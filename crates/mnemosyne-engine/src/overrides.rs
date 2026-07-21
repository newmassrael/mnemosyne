//! The consumer override interface — the WHAT-content contract (ladders /
//! examinable objects / journal-predicate policy) the kernel needs but the store
//! does not carry.
//!
//! This is the DATA override surface. STYLE (color / letter-spacing / theme) is
//! a SEPARATE override surface that lives in the presentation layer, keyed off a
//! [`Line`](crate::Line)'s semantic axes (`mode`, `frame`, `entities`, `quote`,
//! `count`, `typed_predicate`) — never here, because the kernel is
//! presentation-agnostic (it supplies meaning; the renderer supplies looks).

use std::path::Path;

use crate::Interactivity;

/// What a consumer supplies to the kernel beyond the store: the authored
/// interactive layer and the journal-predicate policy. CONTENT only —
/// presentation (chrome labels, localization, colors, spacing) is the renderer's
/// override surface, not this one.
pub trait EngineOverrides {
    /// The authored interactive layer (ladders + examinable objects). The kernel
    /// derives examine/ask doors and runs the gate against it.
    fn interactivity(&self) -> &Interactivity;

    /// Typed predicates whose facts are the game's own JOURNAL (quest legs), not
    /// world narrative — routed OUT of the prose `lines` stream (still available
    /// to a quest layer). Empty = every fact is prose.
    fn journal_predicates(&self) -> &[String];
}

/// The zero-config override: no interactivity, no journal policy. A store plays
/// with every fact shown as prose and only fork doors — the batteries-included
/// default so any store is renderable without consumer wiring.
#[derive(Debug, Clone, Default)]
pub struct DefaultOverrides {
    interactivity: Interactivity,
}

impl EngineOverrides for DefaultOverrides {
    fn interactivity(&self) -> &Interactivity {
        &self.interactivity
    }

    fn journal_predicates(&self) -> &[String] {
        &[]
    }
}

/// An in-memory override a consumer populates however it loads its data — the
/// concrete impl the file loader deserializes into. The canonical JSON:
///
/// ```json
/// {
///   "interactivity": { "ladders": { "sc-01": [] }, "objects": [] },
///   "journal_predicates": ["pursues", "requires", "completed_by"]
/// }
/// ```
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StaticOverrides {
    /// The authored interactive layer.
    #[serde(default)]
    pub interactivity: Interactivity,
    /// The journal-predicate policy.
    #[serde(default)]
    pub journal_predicates: Vec<String>,
}

impl StaticOverrides {
    /// Parse the canonical override JSON.
    ///
    /// # Errors
    ///
    /// The `serde_json` error if the text is not the canonical override shape.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Load the canonical override JSON from a file.
    ///
    /// # Errors
    ///
    /// [`OverrideLoadError::Read`] if the file cannot be read;
    /// [`OverrideLoadError::Parse`] if it is not the canonical shape.
    pub fn load(path: &Path) -> Result<Self, OverrideLoadError> {
        let text = std::fs::read_to_string(path).map_err(OverrideLoadError::Read)?;
        Self::from_json(&text).map_err(OverrideLoadError::Parse)
    }
}

impl EngineOverrides for StaticOverrides {
    fn interactivity(&self) -> &Interactivity {
        &self.interactivity
    }

    fn journal_predicates(&self) -> &[String] {
        &self.journal_predicates
    }
}

/// A failure loading a [`StaticOverrides`] from a file.
#[derive(Debug)]
pub enum OverrideLoadError {
    /// The override file could not be read.
    Read(std::io::Error),
    /// The override file was not the canonical JSON shape.
    Parse(serde_json::Error),
}

impl std::fmt::Display for OverrideLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverrideLoadError::Read(e) => write!(f, "reading override file: {e}"),
            OverrideLoadError::Parse(e) => write!(f, "parsing override JSON: {e}"),
        }
    }
}

impl std::error::Error for OverrideLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OverrideLoadError::Read(e) => Some(e),
            OverrideLoadError::Parse(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DefaultOverrides, EngineOverrides, StaticOverrides};

    #[test]
    fn default_overrides_are_empty() {
        let d = DefaultOverrides::default();
        assert!(d.interactivity().ladders.is_empty());
        assert!(d.interactivity().objects.is_empty());
        assert!(!d.interactivity().free_investigate); // default MODAL
        assert!(d.journal_predicates().is_empty());
    }

    #[test]
    fn static_overrides_parse_the_canonical_json() {
        let json = r#"{
            "interactivity": {
                "ladders": { "sc-01": [ { "question": "who?", "reveals": "f-a", "needs": [] } ] },
                "objects": ["tide-table"]
            },
            "journal_predicates": ["pursues", "requires"]
        }"#;
        let o = StaticOverrides::from_json(json).unwrap();
        assert_eq!(
            o.journal_predicates(),
            &["pursues".to_string(), "requires".to_string()]
        );
        assert!(o.interactivity().objects.contains("tide-table"));
        let rungs = &o.interactivity().ladders["sc-01"];
        assert_eq!(rungs.len(), 1);
        assert_eq!(rungs[0].reveals, "f-a");
    }

    #[test]
    fn static_overrides_default_when_fields_are_omitted() {
        let o = StaticOverrides::from_json("{}").unwrap();
        assert!(o.interactivity().ladders.is_empty());
        assert!(o.journal_predicates().is_empty());
    }
}
