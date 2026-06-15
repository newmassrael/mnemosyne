//! splice — replace named `## sc-NN` scene blocks in a manuscript with
//! re-rendered versions, leaving every other byte untouched (R555).
//!
//! This makes the R553 targeted-repair localization (PIN-R3) a MECHANICAL
//! guarantee rather than a hand-checked diff, and replaces the throwaway Python
//! splice. A replacement whose scene id is absent from the base, or a base block
//! given twice, is a hard error — the repair cannot silently land in the wrong
//! place or smuggle in an unintended scene.

use std::collections::{BTreeMap, BTreeSet};

use crate::util::{read_file, write_file, HResult};

/// The scene id of a `## sc-...` heading line, if it is one.
fn heading_scene_id(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("##")?;
    let token = rest.split_whitespace().next()?;
    token.starts_with("sc-").then(|| token.to_string())
}

/// Split a manuscript into (preamble, ordered [(scene_id, block_text)]). A block
/// runs from its `## sc-` heading to just before the next one (or EOF). Lines are
/// kept verbatim (with newlines) so the non-replaced bytes are preserved exactly.
fn blocks(text: &str) -> (String, Vec<(String, String)>) {
    let mut preamble = String::new();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur: Option<(String, String)> = None;
    for line in text.split_inclusive('\n') {
        if let Some(id) = heading_scene_id(line) {
            if let Some(prev) = cur.take() {
                out.push(prev);
            }
            cur = Some((id, line.to_string()));
        } else if let Some((_, body)) = cur.as_mut() {
            body.push_str(line);
        } else {
            preamble.push_str(line);
        }
    }
    if let Some(prev) = cur.take() {
        out.push(prev);
    }
    (preamble, out)
}

/// Replace each `(scene_id, replacement_block)` in `base`. Returns the spliced
/// manuscript and the count of blocks replaced.
pub fn splice(base: &str, replacements: &[(String, String)]) -> HResult<(String, usize)> {
    let (preamble, base_blocks) = blocks(base);
    let mut reps: BTreeMap<&str, &str> = BTreeMap::new();
    for (id, body) in replacements {
        if reps.insert(id.as_str(), body.as_str()).is_some() {
            return Err(format!("scene `{id}` given as a replacement twice"));
        }
    }
    let base_ids: BTreeSet<&str> = base_blocks.iter().map(|(id, _)| id.as_str()).collect();
    for (id, _) in replacements {
        if !base_ids.contains(id.as_str()) {
            return Err(format!(
                "replacement scene `{id}` not found in the base manuscript"
            ));
        }
    }
    let mut used = 0usize;
    let mut out = String::new();
    out.push_str(&preamble);
    for (id, body) in &base_blocks {
        if let Some(rep) = reps.get(id.as_str()) {
            // Change only the prose: keep the original block's exact trailing
            // whitespace (the inter-scene / end-of-file separator) so a spliced
            // manuscript differs from the base in nothing but the replaced bodies.
            let trailing = &body[body.trim_end().len()..];
            out.push_str(rep.trim_end());
            out.push_str(trailing);
            used += 1;
        } else {
            out.push_str(body);
        }
    }
    Ok((out, used))
}

pub fn run(base_path: &str, replacement_paths: &[String], out_path: &str) -> HResult<usize> {
    let base = read_file(base_path)?;
    let mut reps: Vec<(String, String)> = Vec::new();
    for rp in replacement_paths {
        let body = read_file(rp)?;
        let (pre, b) = blocks(&body);
        if !pre.trim().is_empty() {
            return Err(format!("{rp}: content before the `## sc-` heading"));
        }
        if b.len() != 1 {
            return Err(format!(
                "{rp}: must contain exactly one `## sc-` scene, found {}",
                b.len()
            ));
        }
        reps.push((b[0].0.clone(), body));
    }
    let (out, used) = splice(&base, &reps)?;
    write_file(out_path, &out)?;
    Ok(used)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str =
        "Title preamble\n\n## sc-01\n\nfirst.\n\n## sc-02\n\nsecond.\n\n## sc-03\n\nthird.\n";

    #[test]
    fn replaces_only_the_named_block() {
        let reps = vec![(
            "sc-02".to_string(),
            "## sc-02\n\nSECOND, redone.\n".to_string(),
        )];
        let (out, used) = splice(BASE, &reps).unwrap();
        assert_eq!(used, 1);
        assert!(out.contains("SECOND, redone."));
        assert!(!out.contains("\nsecond.")); // old body gone
        assert!(out.contains("first.")); // sc-01 untouched
        assert!(out.contains("third.")); // sc-03 untouched
        assert!(out.starts_with("Title preamble")); // preamble preserved
    }

    #[test]
    fn preserves_each_block_separator_exactly() {
        // A mid block keeps its blank-line separator; the EOF block keeps its
        // single trailing newline — the splice changes prose, never separators.
        let mid = vec![("sc-02".to_string(), "## sc-02\n\nX.\n".to_string())];
        let (out, _) = splice(BASE, &mid).unwrap();
        assert!(out.contains("X.\n\n## sc-03")); // blank line before next scene kept
        let eof = vec![("sc-03".to_string(), "## sc-03\n\nY.\n".to_string())];
        let (out, _) = splice(BASE, &eof).unwrap();
        assert!(out.ends_with("Y.\n")); // single EOF newline preserved (no added blank)
    }

    #[test]
    fn an_unmatched_replacement_is_a_loud_error() {
        let reps = vec![("sc-99".to_string(), "## sc-99\n\nnope.\n".to_string())];
        let err = splice(BASE, &reps).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn heading_id_parses_h2_scene_lines_only() {
        assert_eq!(heading_scene_id("## sc-16\n").as_deref(), Some("sc-16"));
        assert_eq!(heading_scene_id("### sc-16\n"), None); // h3, not a scene heading
        assert_eq!(heading_scene_id("## The Title\n"), None);
    }
}
