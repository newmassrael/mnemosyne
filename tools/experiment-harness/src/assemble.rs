//! Reading-copy assembly: the blind manuscript a judge reads.
//!
//! Input = a story file (scene-id-delimited prose), a playthrough JSON (the
//! per-world ordered scene walk from `report-playthrough-manuscript --json`),
//! and a world name. Output = the world's scenes, in playthrough order, each
//! rendered as `## Title` + stripped body, joined by `---` rules.
//!
//! The blind reading-copy transform applied to every scene body (1) TRUNCATES
//! the body at the first markdown heading — a heading inside a scene body opens
//! a trailing structural section (a `### CHOICE` fork card or the
//! `## World-line and ending map` appendix), never prose, since the scene's own
//! heading is the delimiter stripped before the body (Round 516); (2) drops
//! `<!-- ... -->` scaffolding comments (an unterminated `<!--` is an error) and
//! the recognized line-level scaffolding vocabulary (a `CHOICE:` directive, a
//! bullet carrying the structural `CHOICE` token, and whole-line `[ ... ]`
//! bracket annotations incl. `*[ ... ]*` answer keys); then collapses blank-line
//! runs and trims the ends. The scene heading is normalized `## sc-NN \u{2014}
//! Title` to `## Title` (the scene id is a structural handle, not prose). R500
//! `A)`/`B)` option text under a `CHOICE:` directive (no heading) is left
//! verbatim.
//!
//! NO-SILENT-FAIL (Round 509/516): the heading-truncation catches the fork forms
//! the CHOICE-token vocabulary misses — token-less option bullets like
//! `- **CONFRONT** … \u{2192} sc-09a` and the world-line map — that Round 509's
//! line-level strip silently passed (it dropped the `### CHOICE` heading but
//! kept the bullets and the map, contaminating a judging round and forcing a
//! manual `.clean` patch). A line that survives the recognized line-level strip
//! but still reads as an editorial marker (an unrecognized `CHOICE` form, an
//! unterminated `[` annotation) is a LOUD reject, never silently kept as prose.
//!
//! Every join is checked: a world-order scene with no prose, or a body that
//! strips to nothing, is a hard error. That is the silent-fail the deleted
//! Python committed when `dict.get(id, "")` emitted an empty scene unnoticed.

use crate::playthrough::Playthrough;
use crate::story::{self, Story};
use crate::util::{read_file, HResult};

/// Strip a raw scene body to its blind-reading form. Errors if the body is
/// empty once scaffolding is removed (a real scene cannot be blank), OR if a
/// line survives the strip that still looks like an editorial marker the
/// vocabulary does not recognize — the no-silent-fail guard (Round 509: the
/// narrow R500 `<!--`/`CHOICE:` vocabulary silently passed the blind
/// experiment's `[...]` / `### CHOICE` / `*[answer key]*` forms through as
/// prose, contaminating the judges; an unrecognized marker now fails loud,
/// never leaks).
pub fn reading_body(scene_id: &str, raw: &str) -> HResult<String> {
    let decommented = strip_html_comments(scene_id, raw)?;

    let mut kept: Vec<&str> = Vec::new();
    let mut blank_run = 0usize;
    for line in decommented.lines() {
        let trimmed = line.trim();
        // A markdown heading inside a scene body opens a trailing structural
        // section — a `### CHOICE` fork card or the `## World-line and ending
        // map` appendix — never prose (the scene's own heading is the delimiter,
        // stripped before the body). Everything from the first such heading to
        // the end of the body is structure: stop. This catches the fork forms
        // the CHOICE-token vocabulary misses (token-less option bullets like
        // `- **CONFRONT** … -> sc-09a`) and the world-line map — the silent-fail
        // Round 509 set out to end (Round 516).
        if is_heading_line(trimmed) {
            break;
        }
        if is_scaffolding_line(trimmed) {
            continue;
        }
        if trimmed.is_empty() {
            blank_run += 1;
            // Collapse: at most one blank line between paragraphs.
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
            // No-silent-fail: a non-blank line that survived the recognized
            // strip but still reads as an editorial marker is a loud reject,
            // never silently kept as prose.
            if let Some(why) = suspected_scaffolding(trimmed) {
                return Err(format!(
                    "scene `{scene_id}`: line {trimmed:?} {why} — reject loud (no silent \
                     pass; extend the strip vocabulary in assemble.rs or reword the source)"
                ));
            }
        }
        kept.push(line);
    }

    let body = kept.join("\n");
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err(format!(
            "scene `{scene_id}` strips to an empty body (only scaffolding present)"
        ));
    }
    Ok(body)
}

/// A markdown heading line (`#`+ followed by whitespace). Inside a scene body
/// such a line begins a trailing structural section the reading copy must not
/// carry — a `### CHOICE` fork card or the `## World-line and ending map`
/// appendix (Round 516). The `#`-run must be followed by whitespace so prose
/// like `#1 priority` is not mistaken for a heading.
fn is_heading_line(trimmed: &str) -> bool {
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    hashes > 0 && trimmed[hashes..].starts_with([' ', '\t'])
}

/// A recognized line-level scaffolding marker (Round 509): a `CHOICE:` directive
/// line, a bullet whose text carries the structural uppercase `CHOICE` token, or
/// a whole-line bracket annotation (`[ ... ]`, optionally `*`/`_` emphasis-wrapped
/// — the `[Dramatic irony …]` editorial notes and the `*[All six setups paid …]*`
/// answer key). Headings are handled by truncation ([`is_heading_line`]) before
/// this runs. Recognized scaffolding is dropped silently; unrecognized suspects
/// fail loud via [`suspected_scaffolding`].
fn is_scaffolding_line(trimmed: &str) -> bool {
    if trimmed.starts_with("CHOICE:") {
        return true;
    }
    if is_horizontal_rule(trimmed) {
        return true;
    }
    for marker in ["- ", "* ", "+ "] {
        if let Some(bullet) = trimmed.strip_prefix(marker) {
            if has_choice_token(bullet) {
                return true;
            }
        }
    }
    is_bracket_annotation(trimmed)
}

/// A markdown thematic break (`---`, `***`, `___`, 3+ of one rule char,
/// whitespace allowed). It is a structural scene separator the source may carry
/// mid-body (e.g. before a trailing world-line map), never prose; `story.rs`
/// only trims the ONE trailing rule, so an interior rule reaches here (Round
/// 516). `assemble` re-inserts its own `---` joins between scenes afterward.
fn is_horizontal_rule(trimmed: &str) -> bool {
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && (compact.bytes().all(|b| b == b'-')
            || compact.bytes().all(|b| b == b'*')
            || compact.bytes().all(|b| b == b'_'))
}

/// A non-blank line that survived [`is_scaffolding_line`] but still reads as an
/// editorial marker the vocabulary does not cover (Round 509, no-silent-fail):
/// an unrecognized structural `CHOICE` marker form, or an unterminated `[`
/// annotation (e.g. a multi-line bracket the whole-line rule cannot match).
/// Returns the reason so the caller fails loud.
fn suspected_scaffolding(trimmed: &str) -> Option<&'static str> {
    if has_choice_token(trimmed) {
        return Some("carries an unrecognized CHOICE scaffolding marker");
    }
    if trimmed.starts_with('[') && !trimmed.contains(']') {
        return Some("opens an unterminated `[` editorial annotation");
    }
    None
}

/// True iff some whitespace token, stripped of non-alphabetic edges, is the
/// uppercase structural marker `CHOICE` — so prose `choice` / `choices` is
/// never a marker, only the experiment's uppercase directive token.
fn has_choice_token(s: &str) -> bool {
    s.split_whitespace()
        .any(|w| w.trim_matches(|c: char| !c.is_ascii_alphabetic()) == "CHOICE")
}

/// A whole-line bracket annotation: the trimmed line, after dropping any
/// `*`/`_` emphasis wrap, is `[ … ]`.
fn is_bracket_annotation(trimmed: &str) -> bool {
    let inner = trimmed.trim_matches(|c| c == '*' || c == '_').trim();
    inner.len() >= 2 && inner.starts_with('[') && inner.ends_with(']')
}

/// Remove `<!-- ... -->` spans. An opening `<!--` with no closing `-->` is a
/// loud error rather than silently swallowing the rest of the scene.
fn strip_html_comments(scene_id: &str, raw: &str) -> HResult<String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(open) = rest.find("<!--") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 4..];
        match after.find("-->") {
            Some(close) => {
                rest = &after[close + 3..];
            }
            None => {
                return Err(format!(
                    "scene `{scene_id}` has an unterminated `<!--` comment"
                ));
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// Assemble the reading copy for one world from already-parsed inputs.
pub fn assemble(story: &Story, playthrough: &Playthrough, world: &str) -> HResult<String> {
    if story.is_empty() {
        return Err("story has no scenes".to_string());
    }
    let order = playthrough.world_order(world)?;
    if order.is_empty() {
        return Err(format!("world `{world}` has an empty scene order"));
    }

    let mut blocks: Vec<String> = Vec::with_capacity(order.len());
    for id in &order {
        let scene = story.scene(id)?;
        let body = reading_body(&scene.id, &scene.raw_body)?;
        blocks.push(format!("## {}\n\n{}", scene.title, body));
    }
    // Trailing newline so the file ends cleanly.
    Ok(format!("{}\n", blocks.join("\n\n---\n\n")))
}

/// CLI entry: read the files, assemble, and return the manuscript text.
pub fn run(story_path: &str, playthrough_path: &str, world: &str) -> HResult<String> {
    let story = story::parse(&read_file(story_path)?)?;
    let playthrough = Playthrough::parse(&read_file(playthrough_path)?)?;
    assemble(&story, &playthrough, world)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story;

    const STORY: &str = "\
## sc-01 \u{2014} The Line Goes Down

<!-- SESSION 1 -->
By morning the storm had made an island of them.

---

## sc-02 \u{2014} The Locked Room

The consulting room was on the first floor.

CHOICE: Cendre acts on the ledger.
  A) She confronts the matron. \u{2192} continues sc-21
  B) She stays quiet. \u{2192} continues sc-26

---

## sc-21 \u{2014} The Confrontation

She laid the ledger on the table.
";

    const PLAYTHROUGH: &str = r#"{ "worlds": {
        "confront": { "scenes": [{"section":"sc-01"},{"section":"sc-02"},{"section":"sc-21"}] }
    } }"#;

    #[test]
    fn assembles_world_in_order_stripping_scaffolding() {
        let story = story::parse(STORY).unwrap();
        let pt = Playthrough::parse(PLAYTHROUGH).unwrap();
        let out = assemble(&story, &pt, "confront").unwrap();

        // Headings normalized: title only, no scene id.
        assert!(out.contains("## The Line Goes Down"));
        assert!(!out.contains("sc-01 \u{2014}"));
        // HTML comment and CHOICE directive gone; option text kept verbatim.
        assert!(!out.contains("<!--"));
        assert!(!out.contains("CHOICE:"));
        assert!(out.contains("She confronts the matron"));
        // Order follows the playthrough, not the story file.
        let confront = out.find("Confrontation").unwrap();
        let locked = out.find("Locked Room").unwrap();
        assert!(locked < confront);
        // Scenes separated by a rule.
        assert_eq!(out.matches("\n---\n").count(), 2);
    }

    #[test]
    fn world_order_scene_missing_from_story_is_loud() {
        let story = story::parse(STORY).unwrap();
        let pt = Playthrough::parse(
            r#"{ "worlds": { "confront": { "scenes": [{"section":"sc-99"}] } } }"#,
        )
        .unwrap();
        let err = assemble(&story, &pt, "confront").unwrap_err();
        assert!(err.contains("sc-99"));
        assert!(err.contains("absent"));
    }

    #[test]
    fn unterminated_comment_is_loud() {
        let err = reading_body("sc-x", "prose <!-- open but never closed\nmore").unwrap_err();
        assert!(err.contains("unterminated"));
    }

    #[test]
    fn scaffolding_only_scene_is_loud() {
        let err = reading_body("sc-x", "<!-- note -->\nCHOICE: pick\n").unwrap_err();
        assert!(err.contains("empty body"));
    }

    // Round 509 — the broadened scaffolding vocabulary + no-silent-fail guard.
    // Round 516 — trailing structural sections (fork cards, the world-line map)
    // are truncated at their heading, catching the token-less option-bullet form
    // the CHOICE-token vocabulary missed.

    #[test]
    fn broadened_scaffolding_forms_are_stripped() {
        // A real fork scene: prose, then a trailing `### CHOICE` card whose
        // option bullets DO NOT carry the `CHOICE` token (the warm-render form
        // that silently leaked in Round 515). The heading truncation removes the
        // card heading, its intro line, the token-less bullets, and the trailing
        // limb note — leaving only the scene prose.
        let raw = "\
[Dramatic irony \u{2014} the reader knows the truth.]
The real prose survives here.
### CHOICE \u{2014} fork-1
The motive is on the table; he chooses how the truth comes out.
- **CONFRONT** \u{2014} lay the ledger before Pike. \u{2192} sc-09a
- **AUDIT QUIETLY** \u{2014} reconstruct it unseen. \u{2192} sc-09b
[CONFRONT limb. resolves at sc-19.]";
        let body = reading_body("sc-x", raw).unwrap();
        assert_eq!(body, "The real prose survives here.");
    }

    #[test]
    fn world_line_map_appendix_is_truncated() {
        // The `## World-line and ending map` appendix follows the last scene's
        // prose and carries no `CHOICE` token; Round 509 silently kept it.
        let raw = "\
He capped the ink and went down the hill.
## World-line and ending map
- World 1 (CONFRONT -> REVEAL): sc-01...sc-08 -> sc-17r.
- World 3 (QUIET-AUDIT): sc-01...sc-08 -> sc-20b.";
        let body = reading_body("sc-20b", raw).unwrap();
        assert_eq!(body, "He capped the ink and went down the hill.");
    }

    #[test]
    fn prose_hash_not_a_heading_is_kept() {
        // A `#` not followed by whitespace is prose, not a heading: no truncation.
        let body = reading_body("sc-x", "Cell #1 was the coldest.").unwrap();
        assert_eq!(body, "Cell #1 was the coldest.");
    }

    #[test]
    fn interior_horizontal_rule_is_stripped() {
        // A `---` rule the source carries before a trailing structural section
        // (story.rs trims only the ONE trailing rule) is a structural separator,
        // not prose.
        let body = reading_body("sc-x", "He went down the hill.\n\n---").unwrap();
        assert_eq!(body, "He went down the hill.");
        // `***word***` emphasis is prose, not a rule.
        let kept = reading_body("sc-x", "***urgent*** he wrote.").unwrap();
        assert!(kept.contains("urgent"));
    }

    #[test]
    fn unrecognized_choice_marker_is_loud() {
        // A blockquote CHOICE form the strip vocabulary does not cover: it must
        // fail loud, not pass through as prose (no-silent-fail).
        let err = reading_body("sc-x", "Prose line.\n> CHOICE A please").unwrap_err();
        assert!(err.contains("CHOICE"), "{err}");
    }

    #[test]
    fn unterminated_bracket_annotation_is_loud() {
        let err =
            reading_body("sc-x", "Prose.\n[Dramatic irony spanning\nmultiple lines]").unwrap_err();
        assert!(
            err.contains("unterminated") || err.contains("annotation"),
            "{err}"
        );
    }

    #[test]
    fn lowercase_choice_in_prose_is_kept() {
        let body = reading_body("sc-x", "She had no choice but to run.").unwrap();
        assert!(body.contains("no choice"));
    }

    #[test]
    fn choice_option_text_is_kept_verbatim() {
        // The existing R500 convention: the CHOICE: directive line goes, the
        // A)/B) option text stays (no uppercase CHOICE token in it).
        let raw = "CHOICE: act now\nA) She confronts the matron.\nB) She stays quiet.";
        let body = reading_body("sc-x", raw).unwrap();
        assert!(body.contains("She confronts the matron"));
        assert!(!body.contains("CHOICE:"));
    }
}
