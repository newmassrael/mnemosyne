//! Atomic typed fields → markdown rendering (ratify, Phase 0f
//! output axis). Section / ChangelogEntry atomic fields → tera template
//! render → MD bytes.
//!
//! Templates are compiled into the binary via `include_str!` from the
//! workspace `templates/` directory — runtime path-independent (production
//! crate carries 0 hardcoded paths, closure-gate consistency).
//!
//! Round-trip invariant: atomic fields → render →
//! markdown text is deterministic (same input always produces same output).
//! Re-import path () is multi-session migration scope.

use mnemosyne_atomic::{AtomicChangelogEntry, AtomicSection};
use serde_json::json;
use std::sync::OnceLock;
use tera::{Context, Tera};
use thiserror::Error;

const SECTION_TEMPLATE: &str = include_str!("../../../templates/section.md.tera");
const CHANGELOG_ENTRY_TEMPLATE: &str = include_str!("../../../templates/changelog_entry.md.tera");

const SECTION_TPL_NAME: &str = "section.md";
const CHANGELOG_ENTRY_TPL_NAME: &str = "changelog_entry.md";

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("tera: {0}")]
    Tera(#[from] tera::Error),
}

/// Lazily-initialized template engine. tera compilation is non-trivial; we
/// compile once per process and reuse.
fn engine() -> &'static Tera {
    static ENGINE: OnceLock<Tera> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut t = Tera::default();
        t.add_raw_template(SECTION_TPL_NAME, SECTION_TEMPLATE)
            .expect("section.md.tera compile-time template must parse");
        t.add_raw_template(CHANGELOG_ENTRY_TPL_NAME, CHANGELOG_ENTRY_TEMPLATE)
            .expect("changelog_entry.md.tera compile-time template must parse");
        t
    })
}

/// Render a Section's atomic fields to markdown.
///
/// `title` / `decision_status` live in `AtomicSection.skeleton` (the canonical
/// `SectionSkeleton`, R325) and `section_id` is the store's map key. The caller
/// extracts and threads them in as Tera context alongside the atomic block, so
/// this renderer stays a pure context-over-template function.
pub fn render_section(
    section_id: &str,
    title: &str,
    decision_status: &str,
    atomic: &AtomicSection,
) -> Result<String, RenderError> {
    let mut ctx = Context::new();
    ctx.insert("section_id", section_id);
    ctx.insert("title", title);
    ctx.insert("decision_status", decision_status);
    if let Some(superseded_by) = &atomic.superseded_by {
        ctx.insert("superseded_by", superseded_by);
    }
    if let Some(intent) = &atomic.intent {
        ctx.insert("intent", intent);
    }
    if !atomic.rationale_bullets.is_empty() {
        ctx.insert("rationale_bullets", &atomic.rationale_bullets);
    }
    if !atomic.inputs_bullets.is_empty() {
        ctx.insert("inputs_bullets", &atomic.inputs_bullets);
    }
    if !atomic.outputs_bullets.is_empty() {
        ctx.insert("outputs_bullets", &atomic.outputs_bullets);
    }
    if !atomic.caveats_bullets.is_empty() {
        ctx.insert("caveats_bullets", &atomic.caveats_bullets);
    }
    if !atomic.alternatives_rejected.is_empty() {
        let alts: Vec<_> = atomic
            .alternatives_rejected
            .iter()
            .map(|a| json!({ "alternative": a.alternative, "reason": a.reason }))
            .collect();
        ctx.insert("alternatives_rejected", &alts);
    }
    if !atomic.impact_scope.is_empty() {
        ctx.insert("impact_scope", &atomic.impact_scope);
    }
    if !atomic.examples.is_empty() {
        let examples: Vec<_> = atomic
            .examples
            .iter()
            .map(|e| json!({ "language": e.language, "code": e.code }))
            .collect();
        ctx.insert("examples", &examples);
    }
    if !atomic.implementations.is_empty() {
        let impls: Vec<_> = atomic
            .implementations
            .iter()
            .map(|i| json!({ "file": i.file, "symbol": i.symbol }))
            .collect();
        ctx.insert("implementations", &impls);
    }
    Ok(engine().render(SECTION_TPL_NAME, &ctx)?)
}

/// Render a ChangelogEntry's atomic fields to markdown.
///
/// Round 294 — reads the `publishable_*` half (mutable view layer). The
/// `audit_*` half is the permanent record kept inside the atomic store and
/// is never rendered directly. At append time `publishable_* == audit_*`
/// (see `append_changelog_entry`), so this is byte-identical to pre-R294
/// rendering for entries that have not yet diverged. After R295 setters
/// (paired with the R296 `[[publishable_override_ledger]]` gate) the two
/// halves can diverge; the published view still routes through here.
pub fn render_changelog_entry(
    entry_id: &str,
    atomic: &AtomicChangelogEntry,
) -> Result<String, RenderError> {
    let mut ctx = Context::new();
    ctx.insert("entry_id", entry_id);
    ctx.insert(
        "decision_summary",
        atomic.publishable_decision_summary.as_deref().unwrap_or(""),
    );
    if !atomic.publishable_changes_bullets.is_empty() {
        ctx.insert("changes_bullets", &atomic.publishable_changes_bullets);
    }
    if !atomic.publishable_verification_bullets.is_empty() {
        ctx.insert(
            "verification_bullets",
            &atomic.publishable_verification_bullets,
        );
    }
    if !atomic.publishable_impact_refs.is_empty() {
        ctx.insert("impact_refs", &atomic.publishable_impact_refs);
    }
    if !atomic.publishable_carry_forward_bullets.is_empty() {
        ctx.insert(
            "carry_forward_bullets",
            &atomic.publishable_carry_forward_bullets,
        );
    }
    Ok(engine().render(CHANGELOG_ENTRY_TPL_NAME, &ctx)?)
}

/// Single-source the `GENERATED.md` document format (R345 Decision 4).
///
/// Assembles the fixed scaffolding (title preamble, `Source:` line, `---`,
/// `## Sections` with the `## §`→`### §` demotion, `## Changelog (atomic
/// ledger)`, the empty-changelog fallback) around already-rendered per-unit
/// blocks, in store order. Both the cold full-render
/// (`mnemosyne-ops::render_atomic_store_to_md`) and the warm incremental
/// render (the projection-layer `RenderDb` Tier-2 composition) call this one
/// builder, so the byte-exact format has a single definition and cannot drift
/// against the `round-trip 1/1` + `GENERATED.md = sync` gates.
///
/// `section_blocks` are the *raw* `render_section` outputs (the `## §`→`### §`
/// demotion is applied here, so it too is single-sourced); `entry_blocks` are
/// the raw `render_changelog_entry` outputs.
pub fn compose_generated_md(
    source_rel: &str,
    section_blocks: &[String],
    entry_blocks: &[String],
) -> String {
    let mut out = String::new();
    out.push_str("# GENERATED.md — atomic store derived view\n\n");
    out.push_str(
        "this file `mnemosyne-cli generate-docs` output — direct no edit. \
  atomic store (`docs/.atomic/workspace.atomic.json`) in mutate \
  primitive (`set-section-*` / `append-changelog-entry`) pass and then \
  re-generate.\n\n",
    );
    out.push_str(&format!("Source: `{}`\n\n", source_rel));
    out.push_str("---\n\n");

    if !section_blocks.is_empty() {
        out.push_str("## Sections\n\n");
        for raw in section_blocks {
            // render_section emits `## §N. title` at top-level depth; under the
            // doc's `## Sections` heading these demote one level so the outline
            // stays coherent.
            let demoted = raw.replacen("## §", "### §", 1);
            out.push_str(&demoted);
            out.push('\n');
        }
    }

    if !entry_blocks.is_empty() {
        out.push_str("## Changelog (atomic ledger)\n\n");
        for raw in entry_blocks {
            out.push_str(raw);
            out.push('\n');
        }
    } else {
        out.push_str("## Changelog (atomic ledger)\n\n");
        out.push_str("(empty — first atomic entry will populate this section.)\n\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{ExampleBlock, Implementation, RejectedAlternative};

    #[test]
    fn render_section_minimal_intent_only() {
        let atomic = AtomicSection {
            intent: Some("test intent".into()),
            ..Default::default()
        };
        let out = render_section("43", "cascade_query kind", "active", &atomic).unwrap();
        assert!(out.contains("## §43. cascade_query kind"));
        assert!(out.contains("**Intent**: test intent"));
        // No empty sections rendered.
        assert!(!out.contains("**Rationale**"));
    }

    #[test]
    fn render_section_full_shape() {
        let atomic = AtomicSection {
            intent: Some("primary intent text".into()),
            rationale_bullets: vec!["reason A".into(), "reason B".into()],
            inputs_bullets: vec!["input X".into()],
            outputs_bullets: vec!["output Y".into()],
            caveats_bullets: vec!["caveat Z".into()],
            alternatives_rejected: vec![RejectedAlternative {
                alternative: "approach A".into(),
                reason: "doesn't scale".into(),
            }],
            impact_scope: vec!["15".into(), "39".into()],
            examples: vec![ExampleBlock {
                language: "rust".into(),
                code: "fn main() {}".into(),
            }],
            implementations: vec![
                Implementation {
                    file: "crates/mnemosyne-atomic/src/lib.rs".into(),
                    symbol: Some("AtomicSection".into()),
                },
                Implementation {
                    file: "crates/mnemosyne-cli/src/atomic_cli.rs".into(),
                    symbol: None,
                },
            ],
            ..Default::default()
        };
        let out = render_section("43", "test", "active", &atomic).unwrap();
        assert!(out.contains("**Intent**: primary intent text"));
        assert!(out.contains("- reason A"));
        assert!(out.contains("- reason B"));
        assert!(out.contains("- input X"));
        assert!(out.contains("- output Y"));
        assert!(out.contains("- caveat Z"));
        assert!(out.contains("approach A — doesn't scale"));
        assert!(out.contains("**Impact scope**: §15, §39"));
        assert!(out.contains("```rust"));
        assert!(out.contains("fn main() {}"));
        assert!(out.contains("**Implementations**"));
        assert!(out.contains("- crates/mnemosyne-atomic/src/lib.rs:AtomicSection"));
        assert!(out.contains("- crates/mnemosyne-cli/src/atomic_cli.rs"));
    }

    #[test]
    fn render_section_status_omitted_when_active() {
        let atomic = AtomicSection {
            intent: Some("x".into()),
            ..Default::default()
        };
        let out = render_section("43", "test", "active", &atomic).unwrap();
        assert!(!out.contains("**Status**"));
    }

    #[test]
    fn render_section_status_emitted_when_superseded() {
        let atomic = AtomicSection::default();
        let out = render_section("43", "test", "superseded", &atomic).unwrap();
        assert!(out.contains("**Status**: superseded"));
    }

    #[test]
    fn render_section_emits_superseded_by_pointer() {
        // R342: the stored forward-pointer renders as a §M citation so the
        // human surface and the round-trip cross-ref derive from the field.
        let atomic = AtomicSection {
            superseded_by: Some("44".into()),
            ..Default::default()
        };
        let out = render_section("43", "test", "superseded", &atomic).unwrap();
        assert!(out.contains("**Superseded by**: §44"));
    }

    #[test]
    fn render_section_omits_superseded_by_when_absent() {
        let atomic = AtomicSection::default();
        let out = render_section("43", "test", "active", &atomic).unwrap();
        assert!(!out.contains("**Superseded by**"));
    }

    #[test]
    fn render_changelog_entry_full_shape() {
        // Round 294 — render reads publishable_*; production path
        // (`append_changelog_entry`) clones audit_* into publishable_* at
        // append time, so the fixture mirrors that path explicitly.
        let mut atomic = AtomicChangelogEntry {
            decision_summary: Some("test decision summary".into()),
            changes_bullets: vec!["change A".into(), "change B".into()],
            verification_bullets: vec!["verify A".into()],
            impact_refs: vec!["43".into(), "61".into()],
            carry_forward_bullets: vec!["carry A".into()],
            ..Default::default()
        };
        atomic.clone_audit_into_publishable();
        let out = render_changelog_entry("Round 162", &atomic).unwrap();
        assert!(out.contains("### Round 162 — test decision summary"));
        assert!(out.contains("- change A"));
        assert!(out.contains("- change B"));
        assert!(out.contains("- verify A"));
        assert!(out.contains("**Impact**: §43, §61"));
        assert!(out.contains("- carry A"));
    }

    #[test]
    fn render_changelog_entry_publishable_diverges_from_audit() {
        // Round 294 — schema split invariant: when publishable_* is
        // explicitly set to differ from audit_*, render emits the
        // publishable view (the audit half stays out of GENERATED.md).
        let atomic = AtomicChangelogEntry {
            decision_summary: Some("audit summary, never rendered".into()),
            changes_bullets: vec!["audit change A".into()],
            verification_bullets: vec!["audit verify A".into()],
            impact_refs: vec!["43".into()],
            carry_forward_bullets: vec!["audit carry A".into()],
            publishable_decision_summary: Some("redacted summary".into()),
            publishable_changes_bullets: vec!["redacted change A".into()],
            publishable_verification_bullets: vec!["redacted verify A".into()],
            publishable_impact_refs: vec!["43".into()],
            publishable_carry_forward_bullets: vec!["redacted carry A".into()],
        };
        let out = render_changelog_entry("Round 162", &atomic).unwrap();
        assert!(out.contains("redacted summary"), "out: {}", out);
        assert!(out.contains("- redacted change A"));
        assert!(out.contains("- redacted verify A"));
        assert!(out.contains("- redacted carry A"));
        assert!(
            !out.contains("audit summary"),
            "audit half must not leak into render"
        );
        assert!(!out.contains("audit change A"));
        assert!(!out.contains("audit verify A"));
        assert!(!out.contains("audit carry A"));
    }

    #[test]
    fn render_deterministic() {
        let atomic = AtomicSection {
            intent: Some("x".into()),
            rationale_bullets: vec!["a".into(), "b".into()],
            ..Default::default()
        };
        let out1 = render_section("43", "test", "active", &atomic).unwrap();
        let out2 = render_section("43", "test", "active", &atomic).unwrap();
        assert_eq!(out1, out2, "render must be deterministic");
    }
}
