//! Layer-0 narrative fact model — multi-axis (perspectival) facts.
//!
//! ARCHITECTURE.md sec 1.1 (Round 391): facts are multi-axis — the
//! actual/historical fact and each agent's understood-fact are DISTINCT facts
//! on distinct axes, both true, never cross-validated. A [`NarrativeFact`] is
//! one such fact: a claim held within exactly one epistemic [`Frame`] over a
//! canon-time extent. Frames are sparse and non-privileged (`ground-truth` is
//! a registry entry like any other); the absence of a fact in a frame means
//! *unrecorded*, never *false*.
//!
//! Medium-neutral by construction (ARCHITECTURE.md sec 6 invariant 4): a
//! frame, a claim, canon coordinates (structure-section refs), and evidence
//! refs exist for a novel, a TRPG sourcebook, or a spec consumer alike —
//! nothing fiction-shaped lives here, mirroring how [`crate::SectionSkeleton`]
//! carries only medium-neutral scalars. The canon coordinate is a
//! structure-section id (the medium's discourse order); a story-time axis is a
//! recorded revisit trigger, deliberately not pre-built.
//!
//! Frame-divergence queries ("facts of frame F at canon point T") are
//! read-side projections; at index-materialization scale the frame maps onto
//! the bitemporal/branch KV's branch dimension ([`crate::FactKey`] already
//! carries `branch_id` + `valid_from` for exactly this projection). The JSON
//! log stays the SSOT and carries every field the index will ever need.

use serde::{Deserialize, Serialize};

/// One epistemic frame (registry entry). Keyed by frame id in
/// `AtomicStore.frames`; the id is the value every [`NarrativeFact::frame`]
/// must reference (fail-loud at the mutate primitive).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Free-form description of whose epistemic frame this is (e.g. a
    /// character, a faction, the ground-truth axis). Optional prose, not
    /// load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// One multi-axis narrative fact: a claim held within exactly one epistemic
/// frame over a canon-time extent, evidenced by structure sections.
///
/// Append-only by genre: a belief that changes is not edited — a successor
/// fact in the same frame records `supersedes_in_frame`, and the
/// predecessor's effective end is DERIVED from the successor's `canon_from`
/// (never written back). The stored `canon_to` is for beliefs that end
/// without a successor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrativeFact {
    /// Epistemic frame id (registry key in `AtomicStore.frames`). Exactly
    /// one — a believed-fact and the corresponding ground-truth fact are
    /// distinct facts, never one fact with two frames.
    pub frame: String,
    /// The claim held in this frame, per-claim granularity (atomic,
    /// falsifiable — one assertion, not an entity dossier).
    pub claim: String,
    /// Canon coordinate where this claim starts holding: a structure-section
    /// id (the medium's discourse order, e.g. a chapter).
    pub canon_from: String,
    /// Explicit canon end for a belief that ends WITHOUT an in-frame
    /// successor. When a successor exists, the effective end derives from
    /// the successor's `canon_from` and this stays `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canon_to: Option<String>,
    /// Structure-section ids evidencing the claim (≥ 1, fail-loud at the
    /// mutate primitive). Multi-ref by design — a claim's evidence usually
    /// spans sections.
    pub evidence: Vec<String>,
    /// Recorded conflict assertions (fact ids). Contradiction is a semantic
    /// judgment, so edges are recorded — never derived from claim text. The
    /// continuity gate evaluates them frame-scoped: same-frame overlapping
    /// conflict = violation; cross-frame conflict = data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts_with: Vec<String>,
    /// In-frame predecessor this claim replaces (same frame enforced at the
    /// mutate primitive). The mechanism for time-indexed belief change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes_in_frame: Option<String>,
    /// Optional verbatim quote backing the claim (a derived cache of medium
    /// content, EPUB-SSOT symmetric).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// sha256 of `quote`, computed by the mutate primitive at write time
    /// (never caller-supplied) so an out-of-band sidecar edit is detectable
    /// offline — the R404 content-drift pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_sha256: Option<String>,
}
