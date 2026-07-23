//! Scene-presence world-truth — the authored judgment of WHO is in a scene and
//! HOW their presence is known (Round 757, B0). A Layer-0 domain enum shared by
//! the store (`mnemosyne-atomic`'s `ScenePresence` on a Section) and the engine
//! (`mnemosyne-engine`'s cast projection), with no `atomic`↔`engine` dependency —
//! the same reasoning that put [`ContentAnchor`](crate::ContentAnchor) here.
//!
//! Presence is AUTHORED world-truth (every consumer must agree who is present at a
//! scene; disagreement is a bug), so the modality is a stored authored judgment,
//! not something the engine re-derives from the prose — re-implementing an
//! authored judgment loses fidelity.

/// How a character's presence in a scene is known to the point-of-view — the
/// authored evidentiary stance behind a `ScenePresence`. Serialized as
/// `snake_case` so a consumer supplies it as a plain string
/// (`"observed"`/`"guessed"`/`"told"`/`"remembered"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    /// Directly perceived to be present (seen/heard firsthand).
    Observed,
    /// Inferred to be present without direct perception.
    Guessed,
    /// Reported present by another (hearsay).
    Told,
    /// Recalled as present from memory, not perceived in the moment.
    Remembered,
}
