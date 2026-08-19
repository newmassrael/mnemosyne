//! Small fail-loud primitives shared across subcommands: sha256, entropy, and
//! a no-frills error string type. No external crate provides randomness here —
//! `/dev/urandom` is read directly so a missing entropy source is a loud error,
//! never a silent fallback.

use std::fmt::Write as _;
use std::fs;
use std::io::Read as _;

use sha2::{Digest, Sha256};

/// Every subcommand returns this. The string is the operator-facing reason; it
/// is printed to stderr and turns into a non-zero exit. No error is swallowed.
pub type HResult<T> = Result<T, String>;

/// Lowercase-hex sha256 of a byte slice. The seal is computed over the exact
/// bytes written to (or read from) disk so an auditor hashing the file by hand
/// gets the same value.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        // write! to a String is infallible; the unwrap documents that.
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

/// Read `n` bytes of entropy from `/dev/urandom`. A platform without it, or a
/// short read, is a hard error — the shuffle must never quietly degrade to a
/// predictable assignment.
pub fn random_bytes(n: usize) -> HResult<Vec<u8>> {
    let mut file = fs::File::open("/dev/urandom")
        .map_err(|e| format!("cannot open /dev/urandom for the shuffle: {e}"))?;
    let mut buf = vec![0u8; n];
    file.read_exact(&mut buf)
        .map_err(|e| format!("cannot read {n} bytes of entropy from /dev/urandom: {e}"))?;
    Ok(buf)
}

/// Read a whole file, attributing the path in any error.
pub fn read_file(path: &str) -> HResult<String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

/// Read a whole file AS BYTES, attributing the path in any error.
///
/// Beside its `String` sibling rather than replacing it, because the two are
/// used for different things and only one of them may lose information.
/// Everything that PARSES a file wants text and should fail loudly on bytes
/// that are not UTF-8. Carrying evidence into the tree wants the bytes
/// themselves: the whole claim a seal makes is that what is here is what was
/// there, and a copy that went through a decode and an encode has already made
/// that claim about something else.
pub fn read_bytes(path: &str) -> HResult<Vec<u8>> {
    fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))
}

/// Write bytes verbatim, attributing the path in any error.
pub fn write_bytes(path: &str, contents: &[u8]) -> HResult<()> {
    fs::write(path, contents).map_err(|e| format!("cannot write {path}: {e}"))
}

/// Write a file, attributing the path in any error.
pub fn write_file(path: &str, contents: &str) -> HResult<()> {
    fs::write(path, contents).map_err(|e| format!("cannot write {path}: {e}"))
}

/// Collapse `.` and `..` in a record-relative path.
///
/// The kit records write paths as they were convenient to write — `v1/../run/x`
/// is how a nested kit reaches a sibling — and two subcommands now resolve
/// them. `declare` needs it to decide which record owns a path, and `replay`
/// needs it to look one file up in TWO trees and compare them; a comparison
/// only compares if both sides resolve the same string. It lives here, once,
/// for the reason this repository keeps saying: a mechanism with two
/// implementations is one that can answer differently.
///
/// Purely textual, deliberately: nothing here touches the filesystem, so a
/// symlink cannot make the answer depend on which machine is asking.
pub fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_resolves_parent_and_current_segments() {
        assert_eq!(normalize("a/b/../c"), "a/c");
        assert_eq!(normalize("a/./b"), "a/b");
        assert_eq!(normalize("kit/v1/../shared/x.json"), "kit/shared/x.json");
        // A doubled separator is the same path, and a record that carries one
        // must resolve to what its neighbours resolve to.
        assert_eq!(normalize("kit//run/facts.json"), "kit/run/facts.json");
    }
}
