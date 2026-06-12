//! Reading-copy assembly: the blind manuscript a judge reads.
//!
//! Input = a story file (scene-id-delimited prose), a playthrough JSON (the
//! per-world ordered scene walk from `report-playthrough-manuscript --json`),
//! and a world name. Output = the world's scenes, in playthrough order, each
//! rendered as `## Title` + stripped body, joined by `---` rules.
//!
//! The blind reading-copy transform (v1) applied to every scene body drops
//! `<!-- ... -->` scaffolding comments (an unterminated `<!--` is an error) and
//! `CHOICE:` fork-directive lines, then collapses blank-line runs and trims the
//! ends. The scene heading is normalized `## sc-NN \u{2014} Title` to
//! `## Title` (the scene id is a structural handle, not prose). Option text and
//! `ENDING` headers are left verbatim — that matches the transform the
//! scale-floor experiment actually used; tightening either is an explicit
//! future flag, not a silent default.
//!
//! Every join is checked: a world-order scene with no prose, or a body that
//! strips to nothing, is a hard error. That is the silent-fail the deleted
//! Python committed when `dict.get(id, "")` emitted an empty scene unnoticed.

use crate::playthrough::Playthrough;
use crate::story::{self, Story};
use crate::util::{read_file, HResult};

/// Strip a raw scene body to its blind-reading form. Errors if the body is
/// empty once scaffolding is removed (a real scene cannot be blank).
pub fn reading_body(scene_id: &str, raw: &str) -> HResult<String> {
    let decommented = strip_html_comments(scene_id, raw)?;

    let mut kept: Vec<&str> = Vec::new();
    let mut blank_run = 0usize;
    for line in decommented.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("CHOICE:") {
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
}
