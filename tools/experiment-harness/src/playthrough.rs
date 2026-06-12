//! Minimal reader for `mnemosyne-cli report-playthrough-manuscript --json`.
//!
//! That verb (R466) emits the per-world ordered scene walk. The harness needs
//! exactly one thing from it: for a named world, the ordered list of scene
//! sections. Only that subset is modelled here; serde ignores every other field
//! (titles, facts, holding_count, …), so this stays robust to schema growth on
//! the producing side without a version handshake.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::util::HResult;

#[derive(Debug, Deserialize)]
pub struct Playthrough {
    pub worlds: BTreeMap<String, World>,
}

#[derive(Debug, Deserialize)]
pub struct World {
    pub scenes: Vec<SceneRef>,
}

#[derive(Debug, Deserialize)]
pub struct SceneRef {
    /// The scene id, e.g. `sc-01`. Matches `Scene::id` in `story.rs`.
    pub section: String,
}

impl Playthrough {
    pub fn parse(json: &str) -> HResult<Self> {
        serde_json::from_str(json)
            .map_err(|e| format!("cannot parse playthrough JSON (expected report-playthrough-manuscript --json): {e}"))
    }

    /// The ordered scene ids for a world, erroring loudly (and listing the
    /// worlds that do exist) when the requested world is absent.
    pub fn world_order(&self, world: &str) -> HResult<Vec<String>> {
        match self.worlds.get(world) {
            Some(w) => Ok(w.scenes.iter().map(|s| s.section.clone()).collect()),
            None => {
                let mut available: Vec<&str> = self.worlds.keys().map(String::as_str).collect();
                available.sort_unstable();
                Err(format!(
                    "world `{world}` not in the playthrough; available worlds: [{}]",
                    available.join(", ")
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "worlds": {
        "confront": { "scenes": [
          {"section": "sc-01", "title": "ignored"},
          {"section": "sc-02"},
          {"section": "sc-21"}
        ]},
        "quiet": { "scenes": [{"section": "sc-01"}, {"section": "sc-26"}] }
      }
    }"#;

    #[test]
    fn extracts_world_order_ignoring_extra_fields() {
        let pt = Playthrough::parse(SAMPLE).unwrap();
        assert_eq!(
            pt.world_order("confront").unwrap(),
            ["sc-01", "sc-02", "sc-21"]
        );
        assert_eq!(pt.world_order("quiet").unwrap(), ["sc-01", "sc-26"]);
    }

    #[test]
    fn missing_world_lists_available() {
        let pt = Playthrough::parse(SAMPLE).unwrap();
        let err = pt.world_order("escalate").unwrap_err();
        assert!(err.contains("escalate"));
        assert!(err.contains("confront"));
        assert!(err.contains("quiet"));
    }

    #[test]
    fn malformed_json_is_loud() {
        let err = Playthrough::parse("{ not json").unwrap_err();
        assert!(err.contains("cannot parse playthrough JSON"));
    }
}
