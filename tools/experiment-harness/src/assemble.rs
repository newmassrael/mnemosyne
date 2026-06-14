//! Reading-copy assembly: the blind manuscript a judge reads.
//!
//! Input = a story file (scene-id-delimited prose), a playthrough JSON (the
//! per-world ordered scene walk from `report-playthrough-manuscript --json`),
//! and a world name. Output = the world's scenes, in playthrough order, each
//! rendered as `## Title` + stripped body, joined by `---` rules.
//!
//! The blind reading-copy transform applied to every scene body drops
//! `<!-- ... -->` scaffolding comments (an unterminated `<!--` is an error) and
//! the recognized scaffolding-marker vocabulary (Round 509, broadened from the
//! narrow R500 `CHOICE:`-only rule to the forms the blind free-form authors
//! used: a `CHOICE:` directive, a markdown heading/bullet carrying the
//! structural `CHOICE` token, and whole-line `[ ... ]` bracket annotations incl.
//! `*[ ... ]*` answer keys), then collapses blank-line runs and trims the ends.
//! The scene heading is normalized `## sc-NN \u{2014} Title` to `## Title` (the
//! scene id is a structural handle, not prose). Option text and `ENDING`
//! headers are left verbatim.
//!
//! NO-SILENT-FAIL (Round 509): a line that survives the recognized strip but
//! still reads as an editorial marker (an unrecognized `CHOICE` form, an
//! unterminated `[` annotation) is a LOUD reject, never silently kept as prose.
//! The narrow v1 vocabulary silently passed the experiment's `[...]` /
//! `### CHOICE` / `*[answer key]*` through, contaminating a judging round and
//! forcing a manual `.clean` patch — exactly the silent-fail this project bans.
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

/// A recognized scaffolding marker line (Round 509 — broadened from the narrow
/// R500 `<!--`/`CHOICE:` vocabulary to the forms the blind free-form authors
/// actually used): a `CHOICE:` directive line, a markdown heading or bullet
/// whose text carries the structural uppercase `CHOICE` token, or a whole-line
/// bracket annotation (`[ ... ]`, optionally `*`/`_` emphasis-wrapped — the
/// `[Dramatic irony …]` editorial notes and the `*[All six setups paid …]*`
/// answer key). Recognized scaffolding is dropped silently; unrecognized
/// suspects fail loud via [`suspected_scaffolding`].
fn is_scaffolding_line(trimmed: &str) -> bool {
    if trimmed.starts_with("CHOICE:") {
        return true;
    }
    if let Some(after_hash) = trimmed.strip_prefix('#') {
        let heading = after_hash.trim_start_matches('#').trim();
        if has_choice_token(heading) {
            return true;
        }
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

    #[test]
    fn broadened_scaffolding_forms_are_stripped() {
        let raw = "\
[Dramatic irony \u{2014} the reader knows the truth.]
### CHOICE \u{2014} fork-1
- **CHOICE A \u{2014} CONFRONT.**
*[All six setups paid: clock, plate, key.]*
[CONFRONT limb. resolves at sc-19.]
The real prose survives here.";
        let body = reading_body("sc-x", raw).unwrap();
        assert_eq!(body, "The real prose survives here.");
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
        let err = reading_body("sc-x", "Prose.\n[Dramatic irony spanning\nmultiple lines]")
            .unwrap_err();
        assert!(err.contains("unterminated") || err.contains("annotation"), "{err}");
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
