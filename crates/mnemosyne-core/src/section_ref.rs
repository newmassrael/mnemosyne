//! Section-reference syntax primitives.
//!
//! A section id may appear with or without its leading `§` reference
//! marker (`§43` vs `43`). Stripping that optional marker to recover the
//! bare id is a normalization performed at every CLI / MCP / query entry
//! point that accepts a user- or markdown-supplied section reference.
//! Centralizing the marker syntax here keeps the strip operation a single
//! source of truth instead of a `strip_prefix('§').unwrap_or(self)` idiom
//! re-derived per call site.

/// Return `section_id` with its optional leading `§` reference marker
/// removed; ids without the marker are returned unchanged.
///
/// This normalizes a possibly-marked section reference to its bare id and
/// is intentionally tolerant: the marker is optional, so a bare id passes
/// through untouched. Callers that must *branch* on whether the marker was
/// present should match `strip_prefix('§')` directly — that is a different
/// operation (presence test), not this normalization.
pub fn strip_section_marker(section_id: &str) -> &str {
    section_id.strip_prefix('§').unwrap_or(section_id)
}

/// Extract the numeric `§N` / `§N.M` inline references from a single line of
/// prose, returning each target id (the digits after `§`, e.g. `"39"`,
/// `"39.1.2"`) without the marker. A trailing `.` is dropped so `§39.` yields
/// `"39"`. Only numeric references are recognised — slug-form `§foo` mentions
/// in prose are intentionally not treated as cross-references (they are
/// commentary, not graph edges). This is the single source of the `§N` scan
/// shared by the markdown parser's cross-ref extraction and the store-direct
/// cross-ref orphan validator, so the two cannot diverge.
pub fn numeric_section_refs(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if let Some(stripped) = line.get(i..).and_then(|r| r.strip_prefix('§')) {
            let consumed_marker = '§'.len_utf8();
            let stripped_bytes = stripped.as_bytes();
            let mut j = 0usize;
            let mut saw_digit = false;
            while j < stripped_bytes.len() {
                let b = stripped_bytes[j];
                if b.is_ascii_digit() || (b == b'.' && saw_digit) {
                    if b.is_ascii_digit() {
                        saw_digit = true;
                    }
                    j += 1;
                } else {
                    break;
                }
            }
            if saw_digit {
                let mut num_end = j;
                if stripped[..num_end].ends_with('.') {
                    num_end -= 1;
                }
                out.push(stripped[..num_end].to_string());
                i += consumed_marker + j;
                continue;
            }
        }
        let step = line[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        i += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{numeric_section_refs, strip_section_marker};

    #[test]
    fn extracts_numeric_section_refs() {
        assert_eq!(
            numeric_section_refs("see §39 and §40.1.2 here"),
            ["39", "40.1.2"]
        );
    }

    #[test]
    fn drops_trailing_period_and_ignores_slug_refs() {
        assert_eq!(numeric_section_refs("§39. and §code-citation"), ["39"]);
    }

    #[test]
    fn no_refs_yields_empty() {
        assert!(numeric_section_refs("plain prose, no marks").is_empty());
    }

    #[test]
    fn strips_leading_marker() {
        assert_eq!(strip_section_marker("§43"), "43");
    }

    #[test]
    fn passes_bare_id_through_unchanged() {
        assert_eq!(strip_section_marker("43"), "43");
    }

    #[test]
    fn empty_string_is_unchanged() {
        assert_eq!(strip_section_marker(""), "");
    }

    #[test]
    fn marker_only_yields_empty() {
        assert_eq!(strip_section_marker("§"), "");
    }

    #[test]
    fn strips_only_the_first_marker() {
        // A doubled marker is malformed input; only the leading one is
        // removed, leaving the second visible for downstream validation.
        assert_eq!(strip_section_marker("§§43"), "§43");
    }
}
