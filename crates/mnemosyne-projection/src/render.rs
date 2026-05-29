//! Warm render projection — convergence C/D Step 2a (the render walking
//! skeleton, R345).
//!
//! This stands up the `RenderDb` the validation skeleton deliberately deferred
//! (R339 kept the cascade engine Layer-0-only). Render needs the Layer-1
//! design_doc content (`intent`/`rationale`/.../`publishable_*`), so per R345
//! Decision 1 it gets its *own* Salsa database here rather than widening the
//! validation `SectionRecord` — a content-only edit then cannot invalidate a
//! validation memo (independent inputs, independent memo tables).
//!
//! Per R345 Decision 2 the render engine lives one layer up (here, the
//! projection layer), where it may depend on `mnemosyne-query`'s Tera
//! renderers — `tera`, template I/O, and design_doc-medium knowledge never
//! enter the pure `core + salsa` cascade engine. core is L0 zero-dep, so its
//! types cannot derive `salsa::Update`; the composition layer therefore
//! projects each `AtomicSection` into a *projection-local* render-input record
//! of primitive fields (R345's "medium-specific extraction"), and the Tier-1
//! query reconstructs the atomic view to call the shared renderer.
//!
//! Two memo tiers (R345 Decision 3): Tier 1 = one memoized render per Section
//! / ChangelogEntry; Tier 2 = one memoized document composition that calls the
//! single-source `compose_generated_md` builder (R345 Decision 4) so the warm
//! output stays byte-identical to the cold `render_atomic_store_to_md`.
//!
//! Scope (2a): warm wholesale build + byte-identity proof + one warm consumer
//! (the MCP `render_projection` tool), *without* touching the live write path.
//! Incremental delta-apply on mutate (the render analogue of R340's
//! `reconcile_branch_index`) and superseding `auto_regenerate` in the warm host
//! are 2b.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use mnemosyne_atomic::{
    AtomicChangelogEntry, AtomicSection, AtomicStore, ExampleBlock, Implementation,
    RejectedAlternative,
};
use mnemosyne_query::{
    compose_generated_md, render_changelog_entry, render_section, section_heading,
};

// --- per-unit Salsa inputs (Tier-1 keys) -----------------------------------
//
// Primitive-only fields: core types (`AtomicSection`, `DecisionStatus`) cannot
// derive `salsa::Update` without coupling the L0 zero-dep core to salsa, so the
// medium-specific extraction lowers the renderable content to String / Vec /
// tuple fields salsa accepts. The Tier-1 query reconstructs the atomic view.

/// One Section's renderable content. `title` / `decision_status` are already
/// resolved (id-fallback + `as_str`, via the shared `section_heading`) by the
/// projection so the Tier-1 query is a pure render.
#[salsa::input]
pub struct RenderSectionInput {
    pub section_id: String,
    pub title: String,
    pub decision_status: String,
    pub superseded_by: Option<String>,
    pub intent: Option<String>,
    pub rationale_bullets: Vec<String>,
    pub inputs_bullets: Vec<String>,
    pub outputs_bullets: Vec<String>,
    pub caveats_bullets: Vec<String>,
    /// `(alternative, reason)` pairs.
    pub alternatives: Vec<(String, String)>,
    pub impact_scope: Vec<String>,
    /// `(language, code)` pairs.
    pub examples: Vec<(String, String)>,
    /// `(file, symbol)` pairs.
    pub implementations: Vec<(String, Option<String>)>,
}

/// One ChangelogEntry's renderable (publishable) content.
#[salsa::input]
pub struct RenderEntryInput {
    pub entry_id: String,
    pub decision_summary: Option<String>,
    pub changes_bullets: Vec<String>,
    pub verification_bullets: Vec<String>,
    pub impact_refs: Vec<String>,
    pub carry_forward_bullets: Vec<String>,
}

/// The Tier-2 composition input: the per-unit inputs in store order plus the
/// `Source:` line value. A single-field mutate backdates the unchanged unit
/// inputs, so only the changed Tier-1 render and the cheap Tier-2 concat re-run.
#[salsa::input]
pub struct RenderIndex {
    pub source_rel: String,
    pub sections: Vec<RenderSectionInput>,
    pub entries: Vec<RenderEntryInput>,
}

// --- Tier-1 per-unit render queries ----------------------------------------

/// Tier-1: render one Section to its raw `render_section` markdown block
/// (un-demoted — the `## §`→`### §` demotion is single-sourced in the Tier-2
/// `compose_generated_md`). Reconstructs the atomic view from the primitive
/// input, then calls the shared renderer.
#[salsa::tracked]
pub fn render_section_block<'db>(db: &'db dyn RenderDb, input: RenderSectionInput) -> String {
    let atomic = AtomicSection {
        intent: input.intent(db),
        rationale_bullets: input.rationale_bullets(db),
        inputs_bullets: input.inputs_bullets(db),
        outputs_bullets: input.outputs_bullets(db),
        caveats_bullets: input.caveats_bullets(db),
        alternatives_rejected: input
            .alternatives(db)
            .into_iter()
            .map(|(alternative, reason)| RejectedAlternative {
                alternative,
                reason,
            })
            .collect(),
        impact_scope: input.impact_scope(db),
        examples: input
            .examples(db)
            .into_iter()
            .map(|(language, code)| ExampleBlock { language, code })
            .collect(),
        implementations: input
            .implementations(db)
            .into_iter()
            .map(|(file, symbol)| Implementation { file, symbol })
            .collect(),
        superseded_by: input.superseded_by(db),
        ..Default::default()
    };
    render_section(
        &input.section_id(db),
        &input.title(db),
        &input.decision_status(db),
        &atomic,
    )
    .expect("section template render over a valid context is infallible")
}

/// Tier-1: render one ChangelogEntry to its raw `render_changelog_entry` block.
/// Reconstructs the publishable half (the audit half is never rendered).
#[salsa::tracked]
pub fn render_entry_block<'db>(db: &'db dyn RenderDb, input: RenderEntryInput) -> String {
    let atomic = AtomicChangelogEntry {
        publishable_decision_summary: input.decision_summary(db),
        publishable_changes_bullets: input.changes_bullets(db),
        publishable_verification_bullets: input.verification_bullets(db),
        publishable_impact_refs: input.impact_refs(db),
        publishable_carry_forward_bullets: input.carry_forward_bullets(db),
        ..Default::default()
    };
    render_changelog_entry(&input.entry_id(db), &atomic)
        .expect("changelog entry template render over a valid context is infallible")
}

// --- Tier-2 document composition query -------------------------------------

/// Tier-2: compose the full `GENERATED.md` from the memoized Tier-1 blocks via
/// the single-source builder. On a single-unit change only that unit's Tier-1
/// render re-executes; this concat re-runs but is cheap.
#[salsa::tracked]
pub fn compose_document<'db>(db: &'db dyn RenderDb, index: RenderIndex) -> String {
    let section_blocks: Vec<String> = index
        .sections(db)
        .into_iter()
        .map(|s| render_section_block(db, s))
        .collect();
    let entry_blocks: Vec<String> = index
        .entries(db)
        .into_iter()
        .map(|e| render_entry_block(db, e))
        .collect();
    compose_generated_md(&index.source_rel(db), &section_blocks, &entry_blocks)
}

// --- DB trait + concrete runtime -------------------------------------------

/// Per-DB tracked-body execution counter, wired into Salsa's `WillExecute`
/// event so tests can assert a stable index is served warm (cache hits do not
/// bump). Mirrors the cascade engine's counter.
#[derive(Clone, Default)]
pub struct ExecCounter(Arc<AtomicUsize>);

impl ExecCounter {
    fn bump(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    pub fn get(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

#[salsa::db]
pub trait RenderDb: salsa::Database {}

#[salsa::db]
#[derive(Clone)]
pub struct RenderDbImpl {
    storage: salsa::Storage<Self>,
    exec_counter: ExecCounter,
}

impl Default for RenderDbImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderDbImpl {
    pub fn new() -> Self {
        let exec_counter = ExecCounter::default();
        let counter_for_event = exec_counter.clone();
        let storage = salsa::Storage::new(Some(Box::new(move |event| {
            if matches!(event.kind, salsa::EventKind::WillExecute { .. }) {
                counter_for_event.bump();
            }
        })));
        Self {
            storage,
            exec_counter,
        }
    }

    pub fn exec_counter(&self) -> usize {
        self.exec_counter.get()
    }
}

#[salsa::db]
impl salsa::Database for RenderDbImpl {}

#[salsa::db]
impl RenderDb for RenderDbImpl {}

// --- projection (medium-specific extraction) -------------------------------

fn project_section_input(
    db: &RenderDbImpl,
    section_id: &str,
    atomic: &AtomicSection,
) -> RenderSectionInput {
    // id-fallback + status resolution come from the shared `section_heading`
    // (R366) so the cold and warm paths cannot drift on this extraction.
    let (title, decision_status) = section_heading(section_id, atomic);
    RenderSectionInput::new(
        db,
        section_id.to_string(),
        title.to_string(),
        decision_status.to_string(),
        atomic.superseded_by.clone(),
        atomic.intent.clone(),
        atomic.rationale_bullets.clone(),
        atomic.inputs_bullets.clone(),
        atomic.outputs_bullets.clone(),
        atomic.caveats_bullets.clone(),
        atomic
            .alternatives_rejected
            .iter()
            .map(|a| (a.alternative.clone(), a.reason.clone()))
            .collect(),
        atomic.impact_scope.clone(),
        atomic
            .examples
            .iter()
            .map(|e| (e.language.clone(), e.code.clone()))
            .collect(),
        atomic
            .implementations
            .iter()
            .map(|i| (i.file.clone(), i.symbol.clone()))
            .collect(),
    )
}

fn project_entry_input(
    db: &RenderDbImpl,
    entry_id: &str,
    atomic: &AtomicChangelogEntry,
) -> RenderEntryInput {
    RenderEntryInput::new(
        db,
        entry_id.to_string(),
        atomic.publishable_decision_summary.clone(),
        atomic.publishable_changes_bullets.clone(),
        atomic.publishable_verification_bullets.clone(),
        atomic.publishable_impact_refs.clone(),
        atomic.publishable_carry_forward_bullets.clone(),
    )
}

fn project_index(db: &RenderDbImpl, atomic: &AtomicStore, source_rel: &str) -> RenderIndex {
    let sections: Vec<RenderSectionInput> = atomic
        .sections
        .iter()
        .map(|(section_id, s)| project_section_input(db, section_id, s))
        .collect();
    let entries: Vec<RenderEntryInput> = atomic
        .changelog_entries
        .iter()
        .map(|(entry_id, e)| project_entry_input(db, entry_id, e))
        .collect();
    RenderIndex::new(db, source_rel.to_string(), sections, entries)
}

// --- warm service ----------------------------------------------------------

/// Warm read-side render projection: a live `RenderDb` plus the `RenderIndex`
/// projected from the authoring log. Hold one across calls; repeated
/// [`render`](Self::render) on a stable index is served from the Salsa memo
/// cache (the in-process warmth a one-shot CLI cannot have). Mirrors
/// [`crate::ProjectionService`] for the render axis.
pub struct RenderProjectionService {
    db: RenderDbImpl,
    index: RenderIndex,
    source_rel: String,
}

impl RenderProjectionService {
    /// Project `atomic` into a warm render engine. `source_rel` is the
    /// workspace-relative sidecar path that fills the `Source:` line (the MCP
    /// host computes it the same way `generate-docs` does); it is stable per
    /// workspace, so it is retained for [`reload`](Self::reload).
    pub fn build(atomic: &AtomicStore, source_rel: &str) -> Self {
        let db = RenderDbImpl::new();
        let index = project_index(&db, atomic, source_rel);
        Self {
            db,
            index,
            source_rel: source_rel.to_string(),
        }
    }

    /// The composed `GENERATED.md`, byte-identical to the cold
    /// `render_atomic_store_to_md`. Repeated calls without an intervening
    /// [`reload`](Self::reload) hit the Salsa memo cache.
    pub fn render(&self) -> String {
        compose_document(&self.db, self.index)
    }

    /// Total Tier-1/Tier-2 bodies executed since construction (test/observability).
    pub fn exec_counter(&self) -> usize {
        self.db.exec_counter()
    }

    /// Re-sync from the current log (reusing the retained `source_rel`). 2a
    /// re-projects wholesale (a fresh index over the warm db); the incremental
    /// delta-apply that keeps unchanged Tier-1 renders memoized across a
    /// re-sync is the render analogue of R340's `reconcile_branch_index` and
    /// lands in 2b.
    pub fn reload(&mut self, atomic: &AtomicStore) {
        self.index = project_index(&self.db, atomic, &self.source_rel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_core::{DecisionStatus, SectionSkeleton};

    fn full_section() -> AtomicSection {
        AtomicSection {
            skeleton: SectionSkeleton {
                title: "Render skeleton".to_string(),
                parent_doc: "docs/GENERATED.md".to_string(),
                parent_section: None,
                decision_status: Some(DecisionStatus::Superseded),
            },
            superseded_by: Some("44".to_string()),
            intent: Some("primary intent".to_string()),
            rationale_bullets: vec!["reason A".to_string(), "reason B".to_string()],
            inputs_bullets: vec!["input X".to_string()],
            outputs_bullets: vec!["output Y".to_string()],
            caveats_bullets: vec!["caveat Z".to_string()],
            alternatives_rejected: vec![RejectedAlternative {
                alternative: "approach A".to_string(),
                reason: "doesn't scale".to_string(),
            }],
            impact_scope: vec!["15".to_string(), "39".to_string()],
            examples: vec![ExampleBlock {
                language: "rust".to_string(),
                code: "fn main() {}".to_string(),
            }],
            implementations: vec![
                Implementation {
                    file: "crates/mnemosyne-atomic/src/lib.rs".to_string(),
                    symbol: Some("AtomicSection".to_string()),
                },
                // symbol: None exercises the Option arm of the tuple lowering.
                Implementation {
                    file: "crates/mnemosyne-cli/src/atomic_cli.rs".to_string(),
                    symbol: None,
                },
            ],
            ..Default::default()
        }
    }

    fn entry() -> AtomicChangelogEntry {
        let mut e = AtomicChangelogEntry {
            decision_summary: Some("test decision".to_string()),
            changes_bullets: vec!["change A".to_string()],
            verification_bullets: vec!["verify A".to_string()],
            impact_refs: vec!["43".to_string()],
            carry_forward_bullets: vec!["carry A".to_string()],
            ..Default::default()
        };
        e.clone_audit_into_publishable();
        e
    }

    /// The store the warm engine renders from, plus the cold-path block render
    /// for the same store, so the test compares warm output to a direct
    /// `compose_generated_md` call (the exact builder the cold
    /// `render_atomic_store_to_md` uses — so warm == cold by construction of
    /// the shared builder, and this isolates the Tier-1 reconstruction).
    fn cold_compose(store: &AtomicStore, source_rel: &str) -> String {
        let section_blocks: Vec<String> = store
            .sections
            .iter()
            .map(|(id, atomic)| {
                let (title, status) = section_heading(id, atomic);
                render_section(id, title, status, atomic).unwrap()
            })
            .collect();
        let entry_blocks: Vec<String> = store
            .changelog_entries
            .iter()
            .map(|(id, e)| render_changelog_entry(id, e).unwrap())
            .collect();
        compose_generated_md(source_rel, &section_blocks, &entry_blocks)
    }

    #[test]
    fn warm_render_byte_identical_to_cold_compose_full_shape() {
        let mut store = AtomicStore::new();
        store.sections.insert("43".to_string(), full_section());
        store.sections.insert(
            "7".to_string(),
            AtomicSection {
                intent: Some("minimal".to_string()),
                ..Default::default()
            },
        );
        store
            .changelog_entries
            .insert("Round 162".to_string(), entry());
        let src = "docs/.atomic/workspace.atomic.json";
        let svc = RenderProjectionService::build(&store, src);
        assert_eq!(svc.render(), cold_compose(&store, src));
    }

    #[test]
    fn warm_render_empty_store_matches_cold() {
        let store = AtomicStore::new();
        let src = "docs/.atomic/workspace.atomic.json";
        let svc = RenderProjectionService::build(&store, src);
        let out = svc.render();
        assert_eq!(out, cold_compose(&store, src));
        // The empty-changelog fallback is present, no Sections heading.
        assert!(out.contains("(empty — first atomic entry will populate this section.)"));
        assert!(!out.contains("## Sections"));
    }

    #[test]
    fn reload_re_syncs_and_stays_byte_identical() {
        // reload() is reachable from the live MCP render_projection tool
        // (refresh=true); pin that a re-sync over the warm db reflects the new
        // store AND still byte-matches the cold compose.
        let mut store = AtomicStore::new();
        store.sections.insert("43".to_string(), full_section());
        let src = "docs/.atomic/workspace.atomic.json";
        let mut svc = RenderProjectionService::build(&store, src);
        assert_eq!(svc.render(), cold_compose(&store, src));

        // Out-of-band change: add an entry + a second section.
        store.sections.insert(
            "7".to_string(),
            AtomicSection {
                intent: Some("added".to_string()),
                ..Default::default()
            },
        );
        store
            .changelog_entries
            .insert("Round 162".to_string(), entry());
        // Stale until reload.
        assert_ne!(svc.render(), cold_compose(&store, src));
        svc.reload(&store);
        assert_eq!(svc.render(), cold_compose(&store, src));
    }

    #[test]
    fn repeated_render_on_stable_index_is_served_warm() {
        let mut store = AtomicStore::new();
        store.sections.insert("43".to_string(), full_section());
        store
            .changelog_entries
            .insert("Round 162".to_string(), entry());
        let svc = RenderProjectionService::build(&store, "src.json");
        let first = svc.render();
        let after_first = svc.exec_counter();
        assert!(after_first > 0, "first render executes the tracked bodies");
        let second = svc.render();
        assert_eq!(first, second);
        assert_eq!(
            svc.exec_counter(),
            after_first,
            "a second render on the unchanged index runs no new bodies (warm cache)"
        );
    }
}
