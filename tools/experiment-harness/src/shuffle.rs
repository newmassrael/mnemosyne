//! Blind arm labeling and sealing.
//!
//! Given the named arms of an experiment (e.g. `plain`, `loop`), assign the
//! blind labels A, B, C, ... by an unbiased shuffle drawn from `/dev/urandom`,
//! write the label map as JSON, and emit its sha256. The hash is the seal: it is
//! recorded in the ledger before reveal, so at reveal time `verify-seal` proves
//! the map was not edited after sealing. The shuffle outcome is deliberately
//! not reproducible (that is the blinding); the seal makes tampering loud.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::util::{random_bytes, sha256_hex, write_file, HResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct LabelMap {
    pub experiment: String,
    pub note: String,
    /// label -> arm name. A BTreeMap keeps labels in A, B, C order so the
    /// serialized bytes (and thus the seal) are stable for a given assignment.
    pub assignment: BTreeMap<String, String>,
}

/// Uppercase blind labels A, B, C, ... There is no reason to run a blind A/B
/// experiment with more than a handful of arms; the alphabet is the ceiling.
fn label_for(i: usize) -> HResult<String> {
    if i >= 26 {
        return Err("more than 26 arms is not a blind A/B experiment".to_string());
    }
    Ok(((b'A' + i as u8) as char).to_string())
}

/// Uniform index in `0..=max` from 8 entropy bytes. For the tiny arm counts here
/// the modulo bias against 2^64 is negligible (< n / 2^64).
fn pick(max: usize) -> HResult<usize> {
    let bytes = random_bytes(8)?;
    let mut v = 0u64;
    for b in bytes {
        v = (v << 8) | b as u64;
    }
    Ok((v % (max as u64 + 1)) as usize)
}

/// Build a sealed label map for the given arms. Returns the pretty JSON (with a
/// trailing newline) and its sha256. Does not touch disk.
pub fn build(experiment: &str, note: &str, arms: &[String]) -> HResult<(String, String)> {
    if arms.len() < 2 {
        return Err(format!(
            "a blind experiment needs at least 2 arms, got {}",
            arms.len()
        ));
    }
    let mut seen = BTreeMap::new();
    for arm in arms {
        if seen.insert(arm.clone(), ()).is_some() {
            return Err(format!("arm `{arm}` is listed twice"));
        }
    }

    // Fisher-Yates over a working copy, drawing fresh entropy per swap.
    let mut order: Vec<String> = arms.to_vec();
    for i in (1..order.len()).rev() {
        let j = pick(i)?;
        order.swap(i, j);
    }

    let mut assignment = BTreeMap::new();
    for (i, arm) in order.into_iter().enumerate() {
        assignment.insert(label_for(i)?, arm);
    }

    let map = LabelMap {
        experiment: experiment.to_string(),
        note: note.to_string(),
        assignment,
    };
    let mut json = serde_json::to_string_pretty(&map)
        .map_err(|e| format!("cannot serialize label map: {e}"))?;
    json.push('\n');
    let hash = sha256_hex(json.as_bytes());
    Ok((json, hash))
}

/// CLI entry: build the map, write it, and return the sha256 to print.
pub fn run(experiment: &str, note: &str, arms: &[String], out_path: &str) -> HResult<String> {
    let (json, hash) = build(experiment, note, arms)?;
    write_file(out_path, &json)?;
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arms() -> Vec<String> {
        vec!["plain".to_string(), "loop".to_string()]
    }

    #[test]
    fn assignment_is_a_bijection_over_the_arms() {
        let (json, hash) = build("belvoir", "reveal at S17", &arms()).unwrap();
        let map: LabelMap = serde_json::from_str(&json).unwrap();
        // Both labels present, both arms covered exactly once.
        assert_eq!(map.assignment.len(), 2);
        let mut got: Vec<&String> = map.assignment.values().collect();
        got.sort();
        assert_eq!(got, vec![&"loop".to_string(), &"plain".to_string()]);
        // Seal is the hash of the exact bytes.
        assert_eq!(hash, sha256_hex(json.as_bytes()));
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn fewer_than_two_arms_rejects() {
        let err = build("x", "n", &["solo".to_string()]).unwrap_err();
        assert!(err.contains("at least 2 arms"));
    }

    #[test]
    fn duplicate_arm_rejects() {
        let err = build("x", "n", &["a".to_string(), "a".to_string()]).unwrap_err();
        assert!(err.contains("listed twice"));
    }
}
