//! Seal verification — the reveal/audit half of the shuffle.
//!
//! Re-hash a label-map file and compare against the sha256 that was recorded in
//! the ledger at seal time. A mismatch means the map was altered after sealing;
//! it is a loud, non-zero-exit failure, never a warning.

use crate::util::{read_file, sha256_hex, HResult};

#[derive(Debug)]
pub struct Verdict {
    pub matched: bool,
    pub computed: String,
}

/// Compare a file's sha256 to `expected` (case-insensitive hex). Returns the
/// verdict; the caller decides the exit code.
pub fn verify(map_path: &str, expected: &str) -> HResult<Verdict> {
    let expected = expected.trim().to_lowercase();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "expected sha256 must be 64 hex chars, got `{expected}`"
        ));
    }
    let bytes = read_file(map_path)?;
    let computed = sha256_hex(bytes.as_bytes());
    Ok(Verdict {
        matched: computed == expected,
        computed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::sha256_hex;
    use std::fs;

    fn tmp(name: &str, contents: &str) -> String {
        let mut path = std::env::temp_dir();
        path.push(format!("eh-seal-test-{name}"));
        let path = path.to_string_lossy().to_string();
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn match_and_mismatch() {
        let body = "{\n  \"experiment\": \"x\"\n}\n";
        let path = tmp("ok", body);
        let good = sha256_hex(body.as_bytes());

        let v = verify(&path, &good).unwrap();
        assert!(v.matched);

        let bad = "0".repeat(64);
        let v = verify(&path, &bad).unwrap();
        assert!(!v.matched);
        assert_eq!(v.computed, good);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn malformed_expected_hash_rejects() {
        let path = tmp("badhash", "x");
        let err = verify(&path, "not-a-hash").unwrap_err();
        assert!(err.contains("64 hex chars"));
        fs::remove_file(&path).ok();
    }
}
