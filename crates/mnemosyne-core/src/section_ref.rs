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

#[cfg(test)]
mod tests {
    use super::strip_section_marker;

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
