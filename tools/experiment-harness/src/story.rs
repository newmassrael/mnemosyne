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

/// Parse a heading line `## sc-NN \u{2014} Title` into `(id, title)`. Returns
/// `None` for any line that is not a scene heading (preamble prose, body text,
/// `---` rules).
fn parse_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## ")?;
    let (id, title) = rest.split_once(HEADING_SEP)?;
    let id = id.trim();
    if !id.starts_with("sc-") {
        return None;
    }
    Some((id.to_string(), title.trim().to_string()))
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
}
