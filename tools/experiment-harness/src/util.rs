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

/// Write a file, attributing the path in any error.
pub fn write_file(path: &str, contents: &str) -> HResult<()> {
    fs::write(path, contents).map_err(|e| format!("cannot write {path}: {e}"))
}
