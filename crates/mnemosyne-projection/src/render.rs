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
//! Scope: 2a stood up the warm wholesale build + byte-identity proof + one warm
//! consumer (the MCP `render_projection` tool). 2b (R367) made the re-sync
//! incremental — [`RenderProjectionService::reload`] now applies the minimal
//! Salsa-input delta (the render analogue of R340's `reconcile_branch_index`),
//! so a single-field mutate re-runs only that unit's Tier-1 render plus the
//! cheap Tier-2 concat — and wired the warm render into the mutate write path
//! (the MCP host recomposes + writes `GENERATED.md` through this service,
//! superseding the cold full-render `auto_regenerate`; the cold CLI/CI keeps it).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use mnemosyne_atomic::{
    AtomicChangelogEntry, AtomicSection, AtomicStore, Binding, BindingKind, ExampleBlock,
    NormativeExcerpt, RejectedAlternative,
};
use mnemosyne_query::{
    compose_generated_md, render_changelog_entry, render_section, section_heading,
};
use salsa::Setter;

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
    /// `(file, symbol, kind-tag)` triples. `kind` is lowered to its
    /// canonical tag string because `RenderSectionInput` is a Salsa input
    /// over primitive fields (core's `BindingKind` cannot derive
    /// `salsa::Update`); reconstructed via `BindingKind::from_tag`.
    pub bindings: Vec<(String, Option<String>, String)>,
    /// `(text, anchor_url, source_revision)` of the external-spec mirror
    /// excerpt (RFC-002 FR-1). `None` for ordinary Sections. Lowered to a
    /// primitive tuple because `RenderSectionInput` is a Salsa input over
    /// primitive fields (core is L0 zero-dep, cannot derive `salsa::Update`).
    pub normative_excerpt: Option<(String, String, String)>,
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
        bindings: input
            .bindings(db)
            .into_iter()
            .map(|(file, symbol, kind)| Binding {
                file,
                symbol,
                // The tag was produced one render ago by `BindingKind::as_str`
                // (project_section_input / reconcile_sections), so a parse
                // miss is an internal round-trip break, not bad input — fail
                // loud (R356–R364 discipline) rather than silently coercing to
                // a kind, which on a compliance ledger would be the wrong
                // default in either direction.
                kind: BindingKind::from_tag(&kind)
                    .expect("BindingKind tag round-trips through as_str/from_tag"),
            })
            .collect(),
        superseded_by: input.superseded_by(db),
        normative_excerpt: input.normative_excerpt(db).map(
            |(text, anchor_url, source_revision)| NormativeExcerpt {
                text,
                anchor_url,
                source_revision,
            },
        ),
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
            .bindings
            .iter()
            .map(|b| {
                (
                    b.file.clone(),
                    b.symbol.clone(),
                    b.kind.as_str().to_string(),
                )
            })
            .collect(),
        atomic.normative_excerpt.as_ref().map(|ne| {
            (
                ne.text.clone(),
                ne.anchor_url.clone(),
                ne.source_revision.clone(),
            )
        }),
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

    /// Re-sync from the current log (reusing the retained `source_rel`),
    /// applying the minimal Salsa-input delta against the live index (R367 Step
    /// 2b — the render analogue of R340's `reconcile_branch_index`): unchanged
    /// units keep their input handles, so their memoized Tier-1 renders carry
    /// across the re-sync and only the units that actually changed re-execute on
    /// the next [`render`](Self::render). The `RenderIndex` handle itself is
    /// preserved, so the Tier-2 composition stays memoized when no unit changed.
    pub fn reload(&mut self, atomic: &AtomicStore) {
        reconcile_render_index(&mut self.db, self.index, atomic, &self.source_rel);
    }
}

// --- incremental reconcile (R367 Step 2b) ----------------------------------

/// Reconcile the live `RenderIndex` to `atomic` by applying the minimal set of
/// Salsa-input deltas, reusing unchanged per-unit input handles so the memo
/// cache survives the re-sync. The render analogue of R340's
/// `reconcile_branch_index`: the 2a `build` re-projects wholesale (fresh handle
/// identities → cold cache), whereas this keeps the same `RenderIndex` and every
/// unchanged unit handle and mutates only the fields that changed, so a
/// single-field mutate re-runs only that unit's Tier-1 render plus the cheap
/// Tier-2 concat (size-independent invalidation).
///
/// Keying: Section by `section_id`, ChangelogEntry by `entry_id` — both
/// `BTreeMap`-ordered in the store, so order is stable across re-syncs and a
/// field-only edit never reorders the lists. The `sections` / `entries` Vecs are
/// reset only when *membership* changes; a field-only edit leaves them
/// bit-identical (the same mutated handles), so the Tier-2 composition's
/// dependency on the list is not invalidated.
fn reconcile_render_index(
    db: &mut RenderDbImpl,
    index: RenderIndex,
    atomic: &AtomicStore,
    source_rel: &str,
) {
    reconcile_sections(db, index, atomic);
    reconcile_entries(db, index, atomic);
    if index.source_rel(db) != source_rel {
        index.set_source_rel(db).to(source_rel.to_string());
    }
}

/// Field-level Section reconcile keyed by `section_id`. Reuses the handle for a
/// matching key (setting only changed fields — Salsa bumps an input's revision
/// on every `set`, so an unconditional set would defeat incrementality),
/// allocates for a new key, and resets the `sections` Vec only on a membership
/// change.
fn reconcile_sections(db: &mut RenderDbImpl, index: RenderIndex, atomic: &AtomicStore) {
    let old: std::collections::BTreeMap<String, RenderSectionInput> = index
        .sections(db)
        .into_iter()
        .map(|s| (s.section_id(db), s))
        .collect();
    let mut membership_changed = atomic.sections.len() != old.len();
    let mut new_records: Vec<RenderSectionInput> = Vec::with_capacity(atomic.sections.len());
    for (section_id, sec) in &atomic.sections {
        let record = match old.get(section_id) {
            Some(&existing) => {
                // `section_id` is the key (equal by construction), so it is not
                // re-synced; everything the renderer reads is.
                let (title, decision_status) = section_heading(section_id, sec);
                macro_rules! sync {
                    ($get:ident, $set:ident, $val:expr) => {{
                        let v = $val;
                        if existing.$get(db) != v {
                            existing.$set(db).to(v);
                        }
                    }};
                }
                sync!(title, set_title, title.to_string());
                sync!(
                    decision_status,
                    set_decision_status,
                    decision_status.to_string()
                );
                sync!(superseded_by, set_superseded_by, sec.superseded_by.clone());
                sync!(intent, set_intent, sec.intent.clone());
                sync!(
                    rationale_bullets,
                    set_rationale_bullets,
                    sec.rationale_bullets.clone()
                );
                sync!(
                    inputs_bullets,
                    set_inputs_bullets,
                    sec.inputs_bullets.clone()
                );
                sync!(
                    outputs_bullets,
                    set_outputs_bullets,
                    sec.outputs_bullets.clone()
                );
                sync!(
                    caveats_bullets,
                    set_caveats_bullets,
                    sec.caveats_bullets.clone()
                );
                sync!(
                    alternatives,
                    set_alternatives,
                    sec.alternatives_rejected
                        .iter()
                        .map(|a| (a.alternative.clone(), a.reason.clone()))
                        .collect::<Vec<_>>()
                );
                sync!(impact_scope, set_impact_scope, sec.impact_scope.clone());
                sync!(
                    examples,
                    set_examples,
                    sec.examples
                        .iter()
                        .map(|e| (e.language.clone(), e.code.clone()))
                        .collect::<Vec<_>>()
                );
                sync!(
                    bindings,
                    set_bindings,
                    sec.bindings
                        .iter()
                        .map(|b| (
                            b.file.clone(),
                            b.symbol.clone(),
                            b.kind.as_str().to_string()
                        ))
                        .collect::<Vec<_>>()
                );
                sync!(
                    normative_excerpt,
                    set_normative_excerpt,
                    sec.normative_excerpt.as_ref().map(|ne| {
                        (
                            ne.text.clone(),
                            ne.anchor_url.clone(),
                            ne.source_revision.clone(),
                        )
                    })
                );
                existing
            }
            None => {
                membership_changed = true;
                project_section_input(db, section_id, sec)
            }
        };
        new_records.push(record);
    }
    if membership_changed {
        index.set_sections(db).to(new_records);
    }
}

/// Field-level ChangelogEntry reconcile keyed by `entry_id` (its entity
/// identity). Mirrors [`reconcile_sections`] for the publishable half.
fn reconcile_entries(db: &mut RenderDbImpl, index: RenderIndex, atomic: &AtomicStore) {
    let old: std::collections::BTreeMap<String, RenderEntryInput> = index
        .entries(db)
        .into_iter()
        .map(|e| (e.entry_id(db), e))
        .collect();
    let mut membership_changed = atomic.changelog_entries.len() != old.len();
    let mut new_records: Vec<RenderEntryInput> = Vec::with_capacity(atomic.changelog_entries.len());
    for (entry_id, e) in &atomic.changelog_entries {
        let record = match old.get(entry_id) {
            Some(&existing) => {
                macro_rules! sync {
                    ($get:ident, $set:ident, $val:expr) => {{
                        let v = $val;
                        if existing.$get(db) != v {
                            existing.$set(db).to(v);
                        }
                    }};
                }
                sync!(
                    decision_summary,
                    set_decision_summary,
                    e.publishable_decision_summary.clone()
                );
                sync!(
                    changes_bullets,
                    set_changes_bullets,
                    e.publishable_changes_bullets.clone()
                );
                sync!(
                    verification_bullets,
                    set_verification_bullets,
                    e.publishable_verification_bullets.clone()
                );
                sync!(
                    impact_refs,
                    set_impact_refs,
                    e.publishable_impact_refs.clone()
                );
                sync!(
                    carry_forward_bullets,
                    set_carry_forward_bullets,
                    e.publishable_carry_forward_bullets.clone()
                );
                existing
            }
            None => {
                membership_changed = true;
                project_entry_input(db, entry_id, e)
            }
        };
        new_records.push(record);
    }
    if membership_changed {
        index.set_entries(db).to(new_records);
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
            bindings: vec![
                Binding {
                    file: "crates/mnemosyne-atomic/src/lib.rs".to_string(),
                    symbol: Some("AtomicSection".to_string()),
                    kind: BindingKind::Implements,
                },
                // symbol: None exercises the Option arm of the tuple lowering.
                Binding {
                    file: "crates/mnemosyne-cli/src/atomic_cli.rs".to_string(),
                    symbol: None,
                    kind: BindingKind::Implements,
                },
            ],
            normative_excerpt: Some(NormativeExcerpt {
                text: "the event descriptor is matched verbatim".to_string(),
                anchor_url: "https://www.w3.org/TR/scxml/#event".to_string(),
                source_revision: "2024-rec".to_string(),
            }),
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

    /// A Section with *every* renderable field set to a value distinct from
    /// [`full_section`], so replacing one with the other exercises every
    /// `reconcile_sections` `sync!` set-arm (the field-parity test).
    fn section_alt() -> AtomicSection {
        AtomicSection {
            skeleton: SectionSkeleton {
                title: "Different heading".to_string(),
                parent_doc: "docs/GENERATED.md".to_string(),
                parent_section: None,
                decision_status: Some(DecisionStatus::Active),
            },
            superseded_by: None,
            intent: Some("alt intent".to_string()),
            rationale_bullets: vec!["alt reason".to_string()],
            inputs_bullets: vec!["alt input".to_string()],
            outputs_bullets: vec!["alt output".to_string()],
            caveats_bullets: vec!["alt caveat".to_string()],
            alternatives_rejected: vec![RejectedAlternative {
                alternative: "alt alternative".to_string(),
                reason: "alt rejection reason".to_string(),
            }],
            impact_scope: vec!["7".to_string()],
            examples: vec![ExampleBlock {
                language: "python".to_string(),
                code: "print(1)".to_string(),
            }],
            // Distinct kind (References) from full_section's Implements, so
            // the field-parity test exercises the `kind` arm of the lowering.
            bindings: vec![Binding {
                file: "crates/mnemosyne-query/src/lib.rs".to_string(),
                symbol: Some("compose_generated_md".to_string()),
                kind: BindingKind::References,
            }],
            normative_excerpt: Some(NormativeExcerpt {
                text: "alt normative wording".to_string(),
                anchor_url: "https://www.w3.org/TR/scxml/#datamodel".to_string(),
                source_revision: "2020-rec".to_string(),
            }),
        }
    }

    /// A ChangelogEntry whose publishable half differs from [`entry`] in every
    /// field, to exercise every `reconcile_entries` `sync!` set-arm.
    fn entry_alt() -> AtomicChangelogEntry {
        let mut e = AtomicChangelogEntry {
            decision_summary: Some("alt decision".to_string()),
            changes_bullets: vec!["alt change".to_string()],
            verification_bullets: vec!["alt verify".to_string()],
            impact_refs: vec!["7".to_string()],
            carry_forward_bullets: vec!["alt carry".to_string()],
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

    /// The 2b incrementality contract: after a single Section field edit,
    /// `reload` + `render` re-executes only that unit's Tier-1 render plus the
    /// Tier-2 compose (+2), regardless of how many other units exist
    /// (size-independent invalidation). A wholesale re-project would re-run all.
    #[test]
    fn reconcile_single_field_edit_reruns_only_changed_tier1() {
        let mut store = AtomicStore::new();
        store.sections.insert("43".to_string(), full_section());
        store.sections.insert(
            "7".to_string(),
            AtomicSection {
                intent: Some("seven".to_string()),
                ..Default::default()
            },
        );
        store.sections.insert(
            "9".to_string(),
            AtomicSection {
                intent: Some("nine".to_string()),
                ..Default::default()
            },
        );
        store
            .changelog_entries
            .insert("Round 162".to_string(), entry());
        let src = "src.json";
        let mut svc = RenderProjectionService::build(&store, src);
        let _ = svc.render();
        let warm = svc.exec_counter();

        // Edit exactly one section's content; membership unchanged.
        store.sections.get_mut("7").unwrap().intent = Some("seven edited".to_string());
        svc.reload(&store);
        let out = svc.render();

        // Only the edited section's Tier-1 render + the Tier-2 compose re-run.
        assert_eq!(
            svc.exec_counter() - warm,
            2,
            "a single-field edit re-runs one Tier-1 render + Tier-2 compose, not all units"
        );
        // ...and the output still byte-matches a cold compose of the new store.
        assert_eq!(out, cold_compose(&store, src));
    }

    /// Unchanged Section content across a reload re-executes nothing on render
    /// (the reconcile sets no input field, so every Tier-1 + the Tier-2 compose
    /// stay memoized).
    #[test]
    fn reconcile_no_change_is_fully_memoized() {
        let mut store = AtomicStore::new();
        store.sections.insert("43".to_string(), full_section());
        store
            .changelog_entries
            .insert("Round 162".to_string(), entry());
        let mut svc = RenderProjectionService::build(&store, "src.json");
        let _ = svc.render();
        let warm = svc.exec_counter();
        svc.reload(&store); // identical store
        let _ = svc.render();
        assert_eq!(
            svc.exec_counter(),
            warm,
            "a no-op reload runs no tracked bodies on the next render"
        );
    }

    /// Removing a unit drops it from the output and only re-runs the Tier-2
    /// compose (the surviving units' Tier-1 renders stay memoized).
    #[test]
    fn reconcile_section_removal_stays_byte_identical() {
        let mut store = AtomicStore::new();
        store.sections.insert("43".to_string(), full_section());
        store.sections.insert(
            "7".to_string(),
            AtomicSection {
                intent: Some("seven".to_string()),
                ..Default::default()
            },
        );
        let src = "src.json";
        let mut svc = RenderProjectionService::build(&store, src);
        assert_eq!(svc.render(), cold_compose(&store, src));

        store.sections.remove("7");
        svc.reload(&store);
        let out = svc.render();
        assert_eq!(out, cold_compose(&store, src));
        assert!(!out.contains("seven"));
    }

    /// Field-invariant parity (CLAUDE.md multi-write-path rule, R368): editing
    /// EVERY renderable field on both a Section and a ChangelogEntry, then
    /// reloading, must still byte-match a cold compose. `project_section_input`
    /// (compiler-checked positional `::new`) and `reconcile_sections` (per-field
    /// `sync!`) are two write paths to the same input; a field added to one and
    /// forgotten in the other would compile clean and silently serve stale bytes
    /// on a warm mutate. This test fires every `sync!` set-arm, so the omission
    /// fails here instead of in production.
    #[test]
    fn reconcile_every_renderable_field_change_propagates() {
        let mut store = AtomicStore::new();
        store.sections.insert("43".to_string(), full_section());
        store
            .changelog_entries
            .insert("Round 162".to_string(), entry());
        let src = "src.json";
        let mut svc = RenderProjectionService::build(&store, src);
        assert_eq!(svc.render(), cold_compose(&store, src));

        // Replace both units with fully-different content (membership unchanged,
        // so the field deltas alone must carry through every sync! set-arm).
        store.sections.insert("43".to_string(), section_alt());
        store
            .changelog_entries
            .insert("Round 162".to_string(), entry_alt());
        svc.reload(&store);
        assert_eq!(
            svc.render(),
            cold_compose(&store, src),
            "every renderable field must propagate through reconcile (sync! parity)"
        );
    }

    /// Count-stable membership change: remove one unit and add a different one
    /// in the same reload. The unit count is unchanged, so the change is caught
    /// only by the new-key (`None`) arm, not the `len()` guard — pin that the
    /// Vec is rebuilt and the result stays byte-identical to cold.
    #[test]
    fn reconcile_count_stable_membership_change_stays_byte_identical() {
        let mut store = AtomicStore::new();
        store.sections.insert(
            "7".to_string(),
            AtomicSection {
                intent: Some("seven".to_string()),
                ..Default::default()
            },
        );
        let src = "src.json";
        let mut svc = RenderProjectionService::build(&store, src);
        assert_eq!(svc.render(), cold_compose(&store, src));

        store.sections.remove("7");
        store.sections.insert(
            "8".to_string(),
            AtomicSection {
                intent: Some("eight".to_string()),
                ..Default::default()
            },
        );
        svc.reload(&store);
        let out = svc.render();
        assert_eq!(out, cold_compose(&store, src));
        assert!(out.contains("eight") && !out.contains("seven"));
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
