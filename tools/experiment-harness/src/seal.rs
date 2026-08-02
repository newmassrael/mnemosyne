//! Sealing — recording the sha256 of something that must not change afterwards,
//! and re-checking it.
//!
//! Two halves over one idea. `verify` is the reveal/audit half of the shuffle:
//! re-hash a label-map file and compare against the sha256 recorded in the
//! ledger at seal time. `stamp_inputs` is the same move applied to a kit's
//! replay record: write each declared input's digest into the record ONCE, so
//! that the freeze the contamination bound depends on stops being an
//! instruction to an orchestrator and becomes something a gate can check.
//!
//! A mismatch is always a loud, non-zero-exit failure, never a warning, and
//! `stamp_inputs` never overwrites a digest that is already there — a tool that
//! re-seals on demand would let any later edit launder itself.

use std::path::Path;

use crate::util::{read_file, sha256_hex, write_file, HResult};

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

/// What one call to [`stamp_inputs`] did to a kit's record.
#[derive(Debug, Default)]
pub struct Stamped {
    /// Inputs that had no digest and now have one.
    pub sealed: Vec<String>,
    /// Inputs that already carried a digest, re-checked and still matching.
    pub confirmed: usize,
}

/// Write the sha256 of every declared input into a kit's `replay.json`.
///
/// Paths in the record are relative to the unit directory (the one holding the
/// record), and some point back up out of it, so they are resolved against that
/// directory and not against the repo root.
///
/// Three outcomes per input, and none of them is silent. No digest recorded:
/// hash the bytes and write it, reporting the path. A digest recorded that
/// still matches: leave it and count it. A digest recorded that does NOT match:
/// hard error naming both values. That last case is the whole point — it is
/// either evidence that changed after it was sealed or a record that lied, and
/// which one it is cannot be decided by a tool.
pub fn stamp_inputs(record_path: &str) -> HResult<Stamped> {
    let unit = Path::new(record_path)
        .parent()
        .ok_or_else(|| format!("{record_path} has no parent directory"))?
        .to_path_buf();
    let raw = read_file(record_path)?;
    let mut doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("{record_path} is not JSON: {e}"))?;

    let inputs = doc
        .get_mut("inputs")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| format!("{record_path} declares no `inputs` array"))?;
    if inputs.is_empty() {
        return Err(format!("{record_path} declares an empty `inputs` array"));
    }

    let mut out = Stamped::default();
    for input in inputs.iter_mut() {
        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{record_path}: an input declares no `path`"))?
            .to_string();
        let on_disk = unit.join(&path);
        let bytes = std::fs::read(&on_disk)
            .map_err(|e| format!("{record_path}: cannot read declared input {path}: {e}"))?;
        let computed = sha256_hex(&bytes);

        match input.get("sha256").and_then(|v| v.as_str()) {
            Some(recorded) if recorded == computed => out.confirmed += 1,
            Some(recorded) => {
                return Err(format!(
                    "{record_path}: {path} no longer hashes to its recorded digest\n  \
                     recorded {recorded}\n  computed {computed}\n  \
                     this tool will not re-seal it — decide whether the evidence \
                     or the record is wrong"
                ))
            }
            None => {
                let obj = input
                    .as_object_mut()
                    .ok_or_else(|| format!("{record_path}: an input is not an object"))?;
                obj.insert(
                    "sha256".to_string(),
                    serde_json::Value::String(computed.clone()),
                );
                out.sealed.push(path);
            }
        }
    }

    if !out.sealed.is_empty() {
        let mut rendered = serde_json::to_string_pretty(&doc)
            .map_err(|e| format!("{record_path}: cannot render the updated record: {e}"))?;
        rendered.push('\n');
        write_file(record_path, &rendered)?;
    }
    Ok(out)
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

    /// A kit directory holding one input and a record that declares it.
    fn kit(name: &str, body: &str, record: &str) -> (String, String) {
        let mut dir = std::env::temp_dir();
        dir.push(format!("eh-stamp-{name}"));
        fs::create_dir_all(&dir).unwrap();
        let input = dir.join("facts.json");
        fs::write(&input, body).unwrap();
        let record_path = dir.join("replay.json");
        fs::write(&record_path, record).unwrap();
        (
            record_path.to_string_lossy().to_string(),
            dir.to_string_lossy().to_string(),
        )
    }

    #[test]
    fn seals_once_then_confirms() {
        let body = "{\"facts\": []}\n";
        let (record, dir) = kit(
            "once",
            body,
            "{\"inputs\": [{\"path\": \"facts.json\", \"role\": \"raw-agent-output\"}]}\n",
        );

        let first = stamp_inputs(&record).unwrap();
        assert_eq!(first.sealed, vec!["facts.json".to_string()]);
        assert_eq!(first.confirmed, 0);
        assert!(fs::read_to_string(&record)
            .unwrap()
            .contains(&sha256_hex(body.as_bytes())));

        // Re-running is a re-CHECK, not a re-seal: the second call must confirm
        // what the first wrote rather than quietly rewriting it.
        let second = stamp_inputs(&record).unwrap();
        assert!(second.sealed.is_empty());
        assert_eq!(second.confirmed, 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_changed_input_is_an_error_not_a_reseal() {
        let (record, dir) = kit(
            "changed",
            "{\"facts\": []}\n",
            "{\"inputs\": [{\"path\": \"facts.json\", \"role\": \"raw-agent-output\"}]}\n",
        );
        stamp_inputs(&record).unwrap();

        // The evidence moves after it was sealed — the case the whole mechanism
        // exists for. Re-sealing here would launder the edit.
        fs::write(format!("{dir}/facts.json"), "{\"facts\": [1]}\n").unwrap();
        let err = stamp_inputs(&record).unwrap_err();
        assert!(err.contains("no longer hashes"), "{err}");
        assert!(err.contains("will not re-seal"), "{err}");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_declared_input_that_is_not_there_is_an_error() {
        let (record, dir) = kit(
            "absent",
            "{}\n",
            "{\"inputs\": [{\"path\": \"gone.json\", \"role\": \"replay-input\"}]}\n",
        );
        let err = stamp_inputs(&record).unwrap_err();
        assert!(
            err.contains("cannot read declared input gone.json"),
            "{err}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_record_with_no_inputs_is_an_error() {
        let (record, dir) = kit("empty", "{}\n", "{\"inputs\": []}\n");
        let err = stamp_inputs(&record).unwrap_err();
        assert!(err.contains("empty `inputs` array"), "{err}");
        fs::remove_dir_all(&dir).ok();
    }
}
