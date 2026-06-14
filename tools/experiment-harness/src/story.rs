//! Parser for a scene-id-delimited story file.
//!
//! The corpus format (Belvoir, Saltglass): a markdown document whose scenes are
//! delimited by `## sc-NN \u{2014} Title` headings, scenes separated by `---`
//! rules, with a free preamble before the first scene heading. Fork structure is
//! carried by `CHOICE:` directive lines and `<!-- ... -->` scaffolding comments
//! inside scene bodies; both are stripped when a scene is rendered into a blind
//! reading copy (see `reading_body`).
//!
//! The parser is strict: a duplicate scene id is a hard error, not a
//! last-one-wins overwrite. That is the precise silent-fail the deleted Python
//! could commit (`dict[id] = body` clobbering an earlier scene with no warning).

use std::collections::HashMap;

use crate::util::HResult;

/// The id/title separator inside a scene heading: space, em-dash, space.
const HEADING_SEP: &str = " \u{2014} ";

#[derive(Debug, Clone)]
pub struct Scene {
    pub id: String,
    pub title: String,
    /// Raw body lines as authored, between this heading and the next scene
    /// heading, with the trailing `---` rule and surrounding blank lines
    /// removed. Strip rules are applied later by `reading_body`.
    pub raw_body: String,
}

#[derive(Debug)]
pub struct Story {
    scenes: Vec<Scene>,
    index: HashMap<String, usize>,
}

impl Story {
    /// Look a scene up by id, erroring loudly when an ordering references a
    /// scene the story does not contain.
    pub fn scene(&self, id: &str) -> HResult<&Scene> {
        match self.index.get(id) {
            Some(&i) => Ok(&self.scenes[i]),
            None => Err(format!(
                "scene `{id}` is named in the world ordering but absent from the story \
                 ({} scenes parsed)",
                self.len()
            )),
        }
    }

    pub fn len(&self) -> usize {
        self.scenes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenes.is_empty()
    }
}

/// Parse a heading line into `(id, title)`. Two accepted forms:
/// `## sc-NN \u{2014} Title` (the corpus format) and the BARE `## sc-NN`
/// (Round 525 — an arm whose source carries no heading title, e.g. a reused
/// render with `## sc-NN` only; its title is then supplied by `--titles-from`
/// the fact base). Returns `None` for any line that is not a scene heading
/// (preamble prose, body text, `---` rules, a non-`sc-` `##` section heading).
fn parse_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## ")?;
    let (id, title) = match rest.split_once(HEADING_SEP) {
        Some((id, title)) => (id.trim(), title.trim().to_string()),
        // Bare `## sc-NN`: the whole remainder is the id, title empty. Require a
        // single whitespace-free `sc-` token so a non-scene `## Heading Words`
        // is not mis-parsed (the `sc-` guard below + this keep it tight).
        None => (rest.trim(), String::new()),
    };
    if !id.starts_with("sc-") || id.contains(char::is_whitespace) {
        return None;
    }
    Some((id.to_string(), title))
}

/// Parse a whole story document. Everything before the first scene heading is
/// preamble and ignored.
pub fn parse(source: &str) -> HResult<Story> {
    let lines: Vec<&str> = source.lines().collect();

    // Indices of scene-heading lines, in document order.
    let mut heads: Vec<(usize, String, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some((id, title)) = parse_heading(line) {
            heads.push((i, id, title));
        }
    }

    if heads.is_empty() {
        return Err("no `## sc-NN \u{2014} Title` scene headings found in the story".to_string());
    }

    let mut scenes: Vec<Scene> = Vec::with_capacity(heads.len());
    let mut index: HashMap<String, usize> = HashMap::with_capacity(heads.len());

    for (h, (start, id, title)) in heads.iter().enumerate() {
        let body_start = start + 1;
        let body_end = heads
            .get(h + 1)
            .map(|(next, _, _)| *next)
            .unwrap_or(lines.len());
        let body = trim_body(&lines[body_start..body_end]);

        if let Some(&prev) = index.get(id) {
            return Err(format!(
                "duplicate scene id `{id}`: first as \"{}\", again as \"{title}\" \
                 (a story must declare each scene once)",
                scenes[prev].title
            ));
        }
        index.insert(id.clone(), scenes.len());
        scenes.push(Scene {
            id: id.clone(),
            title: title.clone(),
            raw_body: body,
        });
    }

    Ok(Story { scenes, index })
}

/// Drop the trailing `---` rule and surrounding blank lines from a scene's body
/// slice, returning the inner text.
fn trim_body(lines: &[&str]) -> String {
    let mut end = lines.len();
    // Walk back over blank lines and a single closing `---` rule.
    while end > 0 {
        let t = lines[end - 1].trim();
        if t.is_empty() || t == "---" {
            end -= 1;
        } else {
            break;
        }
    }
    let mut start = 0;
    while start < end && lines[start].trim().is_empty() {
        start += 1;
    }
    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Belvoir

Preamble prose that must be ignored.

---

## sc-01 \u{2014} The Line Goes Down

By morning the storm had made an island of them.

---

## sc-02 \u{2014} The Locked Room

The consulting room was on the first floor.

CHOICE: Cendre acts on the morphine ledger.

---
";

    #[test]
    fn parses_scenes_and_ignores_preamble() {
        let story = parse(SAMPLE).expect("sample parses");
        assert_eq!(story.len(), 2);
        assert_eq!(story.scene("sc-01").unwrap().title, "The Line Goes Down");
        assert!(story
            .scene("sc-01")
            .unwrap()
            .raw_body
            .contains("island of them"));
        // CHOICE directive survives raw parsing; it is stripped at render time.
        assert!(story.scene("sc-02").unwrap().raw_body.contains("CHOICE:"));
    }

    #[test]
    fn missing_scene_is_loud() {
        let story = parse(SAMPLE).unwrap();
        let err = story.scene("sc-99").unwrap_err();
        assert!(err.contains("sc-99"));
        assert!(err.contains("absent"));
    }

    #[test]
    fn duplicate_scene_id_rejects() {
        let dup = format!("{SAMPLE}\n## sc-01 \u{2014} A Clashing Reuse\n\nbody\n");
        let err = parse(&dup).unwrap_err();
        assert!(err.contains("duplicate scene id `sc-01`"));
    }

    #[test]
    fn empty_story_rejects() {
        let err = parse("# Title\n\njust prose, no scenes\n").unwrap_err();
        assert!(err.contains("no `## sc-"));
    }

    #[test]
    fn bare_scene_heading_parses_with_empty_title() {
        // Round 525: an arm whose source carries only `## sc-NN` (a reused
        // render with no heading title) parses; the title comes later from
        // `--titles-from`. A non-`sc-` `##` heading is still ignored.
        let src = "## sc-01\n\nBody one.\n\n---\n\n## World-line and ending map\n\n## sc-02\n\nBody two.\n";
        let story = parse(src).expect("bare headings parse");
        assert_eq!(story.len(), 2);
        assert_eq!(story.scene("sc-01").unwrap().title, "");
        assert!(story.scene("sc-01").unwrap().raw_body.contains("Body one."));
        // `## World-line and ending map` is not a scene heading.
        assert!(story.scene("World-line").is_err());
    }
}
