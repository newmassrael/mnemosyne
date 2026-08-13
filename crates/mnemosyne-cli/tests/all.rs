//! Every integration test of this crate that CAN share a process, in ONE
//! binary.
//!
//! WHY. Each file under `tests/` is its own crate, its own LINK and its own
//! rustdoc unit, and this crate had seventy-six of them — 76 of the
//! workspace's 130 documentable targets in one directory. Round 1171's commit
//! is what turned that from a number into an event: the pre-commit
//! item-citation gate documents every target `cargo metadata` lists, and it
//! ran past a ten-minute timeout and killed the commit. Measured on this
//! machine with the crate's artifacts cleaned, that gate took 7m21s, and the
//! whole of the wait was one step — `documenting mnemosyne-cli` with
//! seventy-six `--test` flags on it.
//!
//! Round 1148 folded `mnemosyne-server`'s twenty-two files the same way for
//! the link cost. This is that repair applied to the crate that grew the
//! larger pile. The law that keeps it honest sits beside this file in
//! `consolidated_test_isolation.rs`, and it was already walking
//! `crates/*/tests/all.rs` when this file arrived.
//!
//! WHAT IT COSTS. The tests share a process, so anything process-global
//! becomes shared state, and the two ways this repository has already been
//! wrong about that are recorded where the law is:
//!
//!   1. `tracing`'s callsite interest cache is process-global even though
//!      `subscriber::with_default` is thread-local — 2 failures in 30 runs.
//!   2. A test file can say only `init_otlp_tracing_subscriber(…)` while the
//!      process-wide `.try_init()` sits one hop away in `src/`.
//!
//! So a file that touches what the PROCESS owns — working directory,
//! environment, tracing dispatcher, signal disposition, global allocator —
//! keeps its own `[[test]]` target, directly or through a helper. That is not
//! a rule to remember: `consolidated_test_isolation` reads this file's
//! `#[path]` list and rejects a file that breaks it.
//!
//! WHAT ELSE CHANGES, stated rather than left to be found:
//!
//!   - Test NAMES gain their module as a prefix (`fact_ref_smoke::the_thing`).
//!     `cargo test <name>` already matched on that, and the injection
//!     harness's red sets resolve at module boundaries (`a::b::name` answers
//!     to `name` and to `b::name`), so the sweeps in `sweeps/` keep working.
//!   - Cargo runs test BINARIES one after another and the tests inside one
//!     binary in parallel, so folding files together raises concurrency rather
//!     than lowering it. Tests that shared an on-disk resource by luck of
//!     running in different processes no longer do — every workspace here is a
//!     `TempDir`, and the suite was run repeatedly to say so rather than
//!     assumed.
//!   - `autotests = false` in `Cargo.toml` is what makes this the only
//!     automatic target; without it cargo would build each file twice.
//!
//! WHAT STAYS OUT, and it is exactly two reasons. A file the PROCESS forces
//! out (the law above), or a file a CI command NAMES with `--test`, which is a
//! target that exists so it can be built alone. Both are checked by
//! `a_separate_test_target_is_one_the_process_or_ci_forces`, so an exception
//! that has neither reason turns a test red instead of quietly growing the
//! pile back. This crate's two are `evidence_replay_smoke` and
//! `pinned_revision_rechecks_evidence_smoke`, which `evidence-replay.yml`
//! selects by name.

// The shared harness, declared ONCE for the whole binary. The files below say
// `use crate::common` rather than `mod common`, because a `mod` inside a
// `#[path]`-loaded module resolves against that module's own directory.
#[path = "common/mod.rs"]
mod common;

#[path = "add_confirmation_event_smoke.rs"]
mod add_confirmation_event_smoke;

#[path = "answer_addressing.rs"]
mod answer_addressing;

#[path = "atomic_first_validate_smoke.rs"]
mod atomic_first_validate_smoke;

#[path = "authorable_population.rs"]
mod authorable_population;

#[path = "authorial_revision_smoke.rs"]
mod authorial_revision_smoke;

#[path = "authoring_surface.rs"]
mod authoring_surface;

#[path = "blanket_verifies_smoke.rs"]
mod blanket_verifies_smoke;

#[path = "commit_ledger_gate_smoke.rs"]
mod commit_ledger_gate_smoke;

// The law that judges this list is IN it, and that is the point rather than a
// curiosity: it used to be the one file that could not be judged by its own
// rule, because it names the constructs it hunts and matched every one of them
// in its own source. Reading a file's SYNTAX instead of its text ended that —
// a string literal in a table is not a use — so the law now lives under the
// rule it states.
#[path = "consolidated_test_isolation.rs"]
mod consolidated_test_isolation;

#[path = "coordinate_read_answers.rs"]
mod coordinate_read_answers;

#[path = "dark_corpus_accounting.rs"]
mod dark_corpus_accounting;

#[path = "discharge_succession_agreement.rs"]
mod discharge_succession_agreement;

#[path = "dnd_quest_map_seam_smoke.rs"]
mod dnd_quest_map_seam_smoke;

#[path = "docs_match_reality_smoke.rs"]
mod docs_match_reality_smoke;

#[path = "edge_cost_smoke.rs"]
mod edge_cost_smoke;

#[path = "edge_guard_smoke.rs"]
mod edge_guard_smoke;

#[path = "entity_dossier_smoke.rs"]
mod entity_dossier_smoke;

#[path = "entity_kind_hierarchy_smoke.rs"]
mod entity_kind_hierarchy_smoke;

#[path = "fact_count_smoke.rs"]
mod fact_count_smoke;

#[path = "fact_ref_smoke.rs"]
mod fact_ref_smoke;

#[path = "feature_coverage_smoke.rs"]
mod feature_coverage_smoke;

#[path = "filtered_read_selects.rs"]
mod filtered_read_selects;

#[path = "frame_view_playable_agreement.rs"]
mod frame_view_playable_agreement;

#[path = "frontier_dangling_agreement.rs"]
mod frontier_dangling_agreement;

#[path = "frontier_manuscript_agreement.rs"]
mod frontier_manuscript_agreement;

#[path = "frontier_playable_agreement.rs"]
mod frontier_playable_agreement;

#[path = "git_hooks_smoke.rs"]
mod git_hooks_smoke;

#[path = "help_registry_smoke.rs"]
mod help_registry_smoke;

#[path = "import_epub_anchors_smoke.rs"]
mod import_epub_anchors_smoke;

#[path = "import_epub_excerpts_smoke.rs"]
mod import_epub_excerpts_smoke;

#[path = "import_facts_smoke.rs"]
mod import_facts_smoke;

#[path = "import_sections_smoke.rs"]
mod import_sections_smoke;

#[path = "locked_resolution_smoke.rs"]
mod locked_resolution_smoke;

#[path = "manuscript_projection_agreement.rs"]
mod manuscript_projection_agreement;

#[path = "map_frontier_smoke.rs"]
mod map_frontier_smoke;

#[path = "mutate_error_output_smoke.rs"]
mod mutate_error_output_smoke;

#[path = "parameter_economy_smoke.rs"]
mod parameter_economy_smoke;

#[path = "path_scope_arguments.rs"]
mod path_scope_arguments;

#[path = "payoff_marking_evidence.rs"]
mod payoff_marking_evidence;

#[path = "payoff_read_agreement.rs"]
mod payoff_read_agreement;

#[path = "phase_1_priority_audit.rs"]
mod phase_1_priority_audit;

#[path = "population_census_smoke.rs"]
mod population_census_smoke;

#[path = "predicate_endpoint_kind_smoke.rs"]
mod predicate_endpoint_kind_smoke;

#[path = "predicate_object_kind_smoke.rs"]
mod predicate_object_kind_smoke;

#[path = "projection_2d_author_arm_smoke.rs"]
mod projection_2d_author_arm_smoke;

#[path = "quantity_unit_smoke.rs"]
mod quantity_unit_smoke;

#[path = "quest_discharge_agreement_smoke.rs"]
mod quest_discharge_agreement_smoke;

#[path = "quest_prerequisite_order_smoke.rs"]
mod quest_prerequisite_order_smoke;

#[path = "quest_state_coverage_agreement.rs"]
mod quest_state_coverage_agreement;

#[path = "r280_atomic_path_config_smoke.rs"]
mod r280_atomic_path_config_smoke;

#[path = "r286_version_smoke.rs"]
mod r286_version_smoke;

#[path = "read_argument_provenance.rs"]
mod read_argument_provenance;

#[path = "report_confirmation_smoke.rs"]
mod report_confirmation_smoke;

#[path = "scale_floor_corpus_carriage.rs"]
mod scale_floor_corpus_carriage;

#[path = "report_coverage_smoke.rs"]
mod report_coverage_smoke;

#[path = "report_frame_view_smoke.rs"]
mod report_frame_view_smoke;

#[path = "report_payoff_coverage_smoke.rs"]
mod report_payoff_coverage_smoke;

#[path = "report_playthrough_manuscript_smoke.rs"]
mod report_playthrough_manuscript_smoke;

#[path = "report_spec_map_smoke.rs"]
mod report_spec_map_smoke;

#[path = "resolver_corpus_laws.rs"]
mod resolver_corpus_laws;

#[path = "side_table_wire_smoke.rs"]
mod side_table_wire_smoke;

#[path = "sigpipe_smoke.rs"]
mod sigpipe_smoke;

#[path = "symbol_axis_reach.rs"]
mod symbol_axis_reach;

#[path = "token_vocabulary_smoke.rs"]
mod token_vocabulary_smoke;

#[path = "tool_pin_smoke.rs"]
mod tool_pin_smoke;

#[path = "transition_map_smoke.rs"]
mod transition_map_smoke;

#[path = "validate_code_refs_path_scope.rs"]
mod validate_code_refs_path_scope;

#[path = "validate_code_refs_smoke.rs"]
mod validate_code_refs_smoke;

#[path = "validate_confirmation_smoke.rs"]
mod validate_confirmation_smoke;

#[path = "validate_content_drift_epub_smoke.rs"]
mod validate_content_drift_epub_smoke;

#[path = "validate_content_drift_smoke.rs"]
mod validate_content_drift_smoke;

#[path = "validate_continuity_smoke.rs"]
mod validate_continuity_smoke;

#[path = "validate_spec_drift_smoke.rs"]
mod validate_spec_drift_smoke;

#[path = "validate_verifies_linkage_smoke.rs"]
mod validate_verifies_linkage_smoke;

#[path = "verification_axis_smoke.rs"]
mod verification_axis_smoke;

#[path = "world_projection_smoke.rs"]
mod world_projection_smoke;
