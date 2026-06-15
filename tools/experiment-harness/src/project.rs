//! project-world — emit the single-world projection of a (possibly combined)
//! re-extracted store that `validate-render-fidelity` expects (R512/R555).
//!
//! The fidelity gate is single-responsibility and single-world by contract: it
//! classifies every fact in the `--against` store against ONE world's order, so
//! a combined store's sibling-branch facts read as off-path. The textbook fix is
//! not to teach the gate about branch tags (muddying its coord-based job) but to
//! feed it the world it expects. This command does exactly that: keep every
//! narrative_fact whose `branch` is the target world or the spine (`main`); drop
//! the sibling-branch facts. It replaces the throwaway Python filter that backed
//! the R551/R553 fidelity pins. A store missing `narrative_facts` is a hard error.

use serde_json::Value;

use crate::util::{read_file, write_file, HResult};

/// Pure transform: returns (projected store JSON + trailing newline, kept, dropped).
pub fn project(
    store_json: &str,
    world: &str,
    main_branch: &str,
) -> HResult<(String, usize, usize)> {
    let mut v: Value =
        serde_json::from_str(store_json).map_err(|e| format!("cannot parse store JSON: {e}"))?;
    let nf = v
        .get_mut("narrative_facts")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "store is missing a `narrative_facts` object".to_string())?;
    let before = nf.len();
    let drop: Vec<String> = nf
        .iter()
        .filter(|(_, fact)| {
            let b = fact
                .get("branch")
                .and_then(Value::as_str)
                .unwrap_or(main_branch);
            b != world && b != main_branch
        })
        .map(|(k, _)| k.clone())
        .collect();
    for k in &drop {
        nf.remove(k);
    }
    let kept = before - drop.len();
    let mut out = serde_json::to_string(&v).map_err(|e| format!("cannot serialize store: {e}"))?;
    out.push('\n');
    Ok((out, kept, drop.len()))
}

pub fn run(
    store_path: &str,
    world: &str,
    main_branch: &str,
    out_path: &str,
) -> HResult<(usize, usize)> {
    let text = read_file(store_path)?;
    let (out, kept, dropped) = project(&text, world, main_branch)?;
    write_file(out_path, &out)?;
    Ok((kept, dropped))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORE: &str = r#"{
      "schema_version": 23,
      "sections": {"sc-1": {}},
      "narrative_facts": {
        "f-spine": {"branch": "main", "canon_from": "sc-1"},
        "f-report": {"branch": "report", "canon_from": "sc-16"},
        "f-confront": {"branch": "confront", "canon_from": "sc-29"},
        "f-untagged": {"canon_from": "sc-1"}
      }
    }"#;

    #[test]
    fn keeps_world_and_spine_drops_siblings() {
        let (out, kept, dropped) = project(STORE, "report", "main").unwrap();
        assert_eq!(kept, 3); // spine + report + untagged(=spine default)
        assert_eq!(dropped, 1); // confront
        let v: Value = serde_json::from_str(&out).unwrap();
        let nf = v["narrative_facts"].as_object().unwrap();
        assert!(nf.contains_key("f-report"));
        assert!(nf.contains_key("f-spine"));
        assert!(nf.contains_key("f-untagged"));
        assert!(!nf.contains_key("f-confront"));
        // Untouched keys survive the round-trip.
        assert_eq!(v["schema_version"], 23);
        assert!(v["sections"].as_object().unwrap().contains_key("sc-1"));
    }

    #[test]
    fn missing_narrative_facts_is_a_loud_error() {
        let err = project(r#"{"schema_version":23}"#, "report", "main").unwrap_err();
        assert!(err.contains("narrative_facts"));
    }
}
