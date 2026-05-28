# GENERATED.md — atomic store derived view

this file `mnemosyne-cli generate-docs` output — direct no edit. atomic store (`docs/.atomic/workspace.atomic.json`) in mutate primitive (`set-section-*` / `append-changelog-entry`) pass and then re-generate.

Source: `docs/.atomic/workspace.atomic.json`

---

## Sections

### §atomic-store-mutate-api. Atomic Store Mutate API










**Implementations**:
- crates/mnemosyne-cli/src/atomic_cli.rs
- crates/mnemosyne-atomic/src/lib.rs



### §code-citation-defense. Code Citation Defense










**Implementations**:
- crates/mnemosyne-cli/src/main.rs
- crates/mnemosyne-validate/src/code_refs.rs



### §code-citation-defense/bidirectional-binding. Bidirectional Binding










**Implementations**:
- crates/mnemosyne-atomic/src/lib.rs
- crates/mnemosyne-validate/src/code_refs.rs



### §markdown-parser. Markdown Parser










**Implementations**:
- crates/mnemosyne-parser/src/lib.rs



### §orphan-ledger. Orphan Ledger










**Implementations**:
- crates/mnemosyne-config/src/lib.rs



## Changelog (atomic ledger)

### Round 252 — Parser CommonMark conformance — recognize indented fenced code blocks (§98) and require whitespace after ATX hash sequence (§62)

**Changes**:
- crates/mnemosyne-validator/src/parser.rs: fenced-code-block detection now allows 0–3 leading spaces (CommonMark §98). Previously `line.starts_with("```")` required the fence at column 0, so indented fences (common inside list items) were ignored and their body lines were re-interpreted as ATX headings.
- crates/mnemosyne-validator/src/parser.rs::parse_heading: require space, tab, or end-of-line after the `#{1,6}` sequence and reject lines with 4+ leading spaces (CommonMark §62). Previously `#1 prose` and `    # x` were lifted to headings, creating spurious numbered H1 sections from inline `#N` prose references.
- crates/mnemosyne-validator/src/parser.rs tests: 5 regression tests added — `atx_heading_requires_space_after_hashes`, `atx_heading_rejects_four_plus_leading_spaces`, `parse_markdown_indented_fence_is_recognized`, `parse_markdown_round_trips_indented_fence_with_hash_lines`, `parse_markdown_inline_hash_number_in_prose_is_not_heading`.
- crates/mnemosyne-cli/src/main.rs::cmd_validate: when round-trip fails, dump the section_id and cross_ref BTreeSet diff (a-only / b-only, capped at 15 / 20 entries) so authors can locate which typed-fact tuples drifted between parse → emit → re-parse. Diagnostic only fires on failure path — happy-path output unchanged.



**Verification**:
- cargo test --release -p mnemosyne-validator: 28 parser tests pass (23 pre-existing + 5 new regression tests).
- watching-zenoh adoption regression: round-trip mandatory 10/13 → 13/13 with parser fix alone, no source-markdown edits required. T1 orphan total 87 → 75 across changelog-config alignment + parser fix.
- mnemosyne self-application validate-workspace unaffected: docs=1/1, T1 orphan=0, round-trip=1/1, style violation counts unchanged from pre-fix baseline.




**Carry forward**:
- list-item-nested fences (CommonMark §49) — where the fence indent matches the enclosing list item's content-indent column rather than 0–3 spaces — remain unhandled; the parser still treats them as plain prose. Full coverage requires list-item state tracking, which is a larger refactor than this conformance round. Acceptable carry: no current Mnemosyne self-doc or known external workspace depends on this case.
- diagnostic dump in cmd_validate could later evolve into a structured `--diff-format=json` flag for tooling consumers; current eprintln-only form is sufficient for human authoring loops.



### Round 253 — External-workspace orphan ledger via mnemosyne.toml [[orphan_ledger]] — Round 80 OPTION D extended to non-self-application carriers

**Changes**:
- crates/mnemosyne-validator/src/config.rs: add `OrphanLedgerEntry` struct + `WorkspaceConfig.orphan_ledger: Vec<OrphanLedgerEntry>` field. Each entry carries `doc / from / to / reason / since`; `reason` is required so suppression cannot be silent. Re-exported from `lib.rs` for downstream consumers.
- crates/mnemosyne-cli/src/main.rs::cmd_validate_workspace: known_orphan_keys now composes (set-union) from two sources — the compile-time `KNOWN_STALE_ORPHANS` const (mnemosyne self-application carry, currently empty) and the workspace's `[[orphan_ledger]]` config rows. Set-equality drift catch (new orphan / resolved entry) preserved across both sources. Ledger output distinguishes `(const)` vs `(config)` rows for tracing.
- crates/mnemosyne-validator/src/config.rs tests: 3 new tests — `orphan_ledger_omitted_yields_empty_vec`, `orphan_ledger_array_of_tables_parses`, `orphan_ledger_missing_required_field_rejected` (chain-format assertion to see through anyhow context wrapping).
- watching-zenoh adoption proof: 75 legacy orphan entries authored as `[[orphan_ledger]]` in watching-zenoh/mnemosyne.toml; validate-workspace post-adoption reports `T1 orphan total=75 (ledger=75, new=+0, resolved=-0)` with exit code 0.



**Verification**:
- cargo test --release -p mnemosyne-validator --lib config::: 14 config tests pass (3 new + 11 pre-existing).
- cargo test --release -p mnemosyne-validator: 159 tests pass total, no regressions.
- mnemosyne self-validate-workspace post-feature: docs=1/1, T1 orphan=0 (ledger=0, new=+0), round-trip=1/1, atomic ledger entries=2 (Round 252 + Round 253), GENERATED.md=sync.
- watching-zenoh validate-workspace post-feature + adoption: exit code 0, T1 orphan=75 (ledger=75, new=+0, resolved=-0), round-trip=13/13.




**Carry forward**:
- Migration of `KNOWN_STALE_ORPHANS` const-based ledger to fully config-based remains optional — the const is empty in current self-application, so the union semantics are no-op in practice. Removing the const entirely is a future cleanup round if no future entry is ever added there.
- External users adopting mnemosyne in legacy multi-doc mode now have a textbook path for legacy orphan carry. Atomic-store-mode workspaces still use the FrozenList primitive for the same goal at the entity level; the two ledgers serve different layers (cross-ref baseline vs. ChangelogEntry membership) and do not conflict.
- Triage path for watching-zenoh's 75 entries: incremental rewrite as `[link](other.md#anchor)` cross-doc form when targets are identified, or author the missing target sections in rfc-sce-protocol-synthesis.md. Each resolved orphan auto-surfaces via drift catch.



### Round 254 — atomic-internal orphan ledger — extends [[orphan_ledger]] to cover ChangelogEntry impact_refs and Section impact_scope dangling refs (kind = atomic_entry_ref / atomic_section_ref); resolves frozen-ledger vs scope-change confusion.

**Changes**:
- config.rs: OrphanKind enum + OrphanLedgerEntry.kind field (default = MarkdownRef for compat)
- main.rs validate-workspace: kind-aware ledger; 3 categories with new/resolved drift catch
- frozen-ledger.md: 'frozen ≠ scope-immutable' carve-out + textbook scope-correction path documented
- anti-patterns.md: silence-bypass vs ratified-scope-change separation as new anti-pattern entry
- schema-guide.md: [[orphan_ledger]] section added with kind field documentation and examples
- config.rs tests: 4 new tests covering kind default, atomic kinds, mixed, unknown reject



**Verification**:
- cargo check --release -p mnemosyne-cli -p mnemosyne-validator: clean compile, no new warnings
- cargo test --release -p mnemosyne-validator: all tests pass (4 new + existing)
- cargo install --path crates/mnemosyne-cli --force: binary replaced
- cargo install --path crates/mnemosyne-mcp --force: binary replaced



**Impact**: §generatedmd--atomic-store-derived-view/changelog-atomic-ledger


**Carry forward**:
- External adoption: watching-zenoh Round 7 — README/SESSION_KICKOFF via kind=atomic_entry_ref
- MCP server reconnect required for external clients to pick up new mnemosyne-mcp build
- Schema documentation alignment: [[orphan_ledger]] section reflects Round 253+254 wire



### Round 255 — Stage 1 LLM citation hygiene wired — agent verifies Round NNN citations via existing list_sections + query_section before writing. Rule documented in project CLAUDE.md + external GETTING_STARTED.md.

**Changes**:
- CLAUDE.md: new "Citation hygiene" section with 3-step verification path
- CLAUDE.md: explicit out-of-scope carve-out (no Stage 2/3 / no semantic / no new tool)
- GETTING_STARTED.md: new section 7 explaining LLM agent citation wire for external users
- No new MCP primitive: existing list_sections + query_section verified sufficient



**Verification**:
- cargo build --release -p mnemosyne-cli: clean compile
- mnemosyne-mcp/src/main.rs read: list_sections + query_section confirmed exposed with decision_status
- Edits to external instruction docs only — atomic store JSON / GENERATED.md untouched




**Carry forward**:
- Round 256: validate-code-refs CLI MVP — foundation for Stage 2 + Stage 3
- Round 257: Stage 2 — pre-commit hook gate using validate-code-refs
- Round 258: Stage 3 — supersede cascade trigger to surface decay
- verify_round_citation MCP tool: add only if two-call dance friction observed
- GETTING_STARTED.md broader staleness (DESIGN.md / ROADMAP.md refs) — separate cleanup round



### Round 256 — validate-code-refs CLI MVP — scans configured [code_refs].paths for <entry_id_prefix><digits> citations, rejects those missing from atomic store changelog_entries. Stage 2 of code-citation defense (Round 255 carry).

**Changes**:
- mnemosyne-validator: new code_refs module (extract_citations + scan_paths + walk_paths + 10 unit tests)
- mnemosyne-validator/config.rs: new CodeRefsSection (paths + severity_missing) + WorkspaceConfig.code_refs Option field
- mnemosyne-cli/main.rs: new validate-code-refs subcommand + help text + cli_schema-derived prefix
- mnemosyne-cli tests: 6 integration smoke tests (skip / clean / reject / warn / identifier-shape / JSON)



**Verification**:
- cargo build --release -p mnemosyne-cli: clean compile, 0 warnings
- cargo test --release -p mnemosyne-validator --lib code_refs: 10/10 PASS
- cargo test --release -p mnemosyne-cli --test validate_code_refs_smoke: 6/6 PASS
- cargo test --release --workspace: all suites PASS, 0 failures, no regressions
- self-application probe: 4 valid entries, 119 historical missing citations (off-main carry)




**Carry forward**:
- Round 257: Stage 2 — wire validate-code-refs into scripts/hooks/pre-commit + GitHub Actions
- Round 258: Stage 3 — supersede cascade trigger (set-section-decision-status auto-runs validate-code-refs)
- Self-application activation deferred — 119 off-main historical citations require bulk orphan_ledger entries
- Section §<id> citation + decision_status check: future round (AtomicChangelogEntry has no status field)
- Tree-sitter language-aware extraction: future round (false-positive precision via [[orphan_ledger]] for now)



### Round 257 — Stage 2 wire — pre-commit hook adds Gate 3 (validate-code-refs always-runs) + GitHub Actions step. Subcommand internally skips when [code_refs] unconfigured (Round 256 carry).

**Changes**:
- scripts/hooks/pre-commit: new Gate 3 (validate-code-refs) always-runs after Gate 2
- scripts/hooks/pre-commit: header comment updated from "Two gates" to "Three gates" with Gate 3 description
- .github/workflows/mnemosyne-validate.yml: new step "validate-code-refs (Round 257 Stage 2)" after validate-workspace
- Hook design: subcommand internally skips when [code_refs] unconfigured (5-min setup carry preserved)



**Verification**:
- bash scripts/hooks/pre-commit (no [code_refs]): Gate 3 prints "skipped", exit 0
- bash scripts/hooks/pre-commit (with [code_refs] paths=nonexistent): missing=0, exit 0
- cargo build --release: clean compile, 0 warnings (no production code touched)
- cargo test --release --workspace: no regressions (hook is shell-only)
- install-hooks.sh: untouched (already copies all scripts/hooks/* files generically)




**Carry forward**:
- Round 258: Stage 3 — supersede cascade trigger (Active → Superseded surfaces decay listing)
- Self-application activation: 119 historical citations require bulk orphan_ledger entries (separate round)
- Hook perf optim: only run Gate 3 when staged files intersect [code_refs].paths (carry, optional)
- External user adoption: GETTING_STARTED.md section 7 (Round 255) documents the agent-side path



### Round 258 — Stage 3 capability layer — validate-code-refs --filter-id flag exposes decay scan (citations of one entry_id surface as Decay kind regardless of valid set membership). Auto-cascade trigger deferred behind AtomicSection.decision_status schema extension.

**Changes**:
- code_refs.rs: scan_paths_filtered + ViolationKind::Decay variant for explicit decay scan
- code_refs.rs: 2 new unit tests (filter_id surfaces decay; filter_id=None reports only missing)
- mnemosyne-cli/main.rs: --filter-id <entry_id> flag on validate-code-refs (decay scan mode)
- mnemosyne-cli/main.rs: text + JSON output now reports both missing_count and decay_count
- mnemosyne-cli/tests: case_vii integration test (filter-id mode, JSON shape, decay surfacing)



**Verification**:
- cargo test --release -p mnemosyne-validator --lib code_refs: 12/12 PASS (10 prior + 2 Round 258)
- cargo test --release -p mnemosyne-cli --test validate_code_refs_smoke: 7/7 PASS (6 prior + 1)
- cargo test --release --workspace: validator unit count 173 → 175 (+2), no regressions
- cargo build --release: clean compile, 0 warnings
- manual probe: validate-code-refs --filter-id "Round 254" --json works, returns decay_count




**Carry forward**:
- Auto-cascade trigger deferred — requires AtomicSection.decision_status schema field (future round)
- Once schema lands: post-mutate hook in set_section_decision_status invokes scan_paths_filtered
- docs/.atomic/code_ref_decay.json audit trail file format: future round when auto-cascade lands
- Section §<id> citation pattern (in addition to Round NNN): future round, parallel extraction path
- Self-application activation of [code_refs] still pending — 119 historical citations need bulk ledger



### Round 259 — AtomicSection.implementations + add_section_implementation mutate primitive — Stage 4 (Path B Spec ↔ Code bidirectional binding) substrate; schema only, validator extension + section seeding + self-application deferred to Round 260-262

**Changes**:
- AtomicSection.implementations: Vec<Implementation> field added (Round 259 schema)
- Implementation struct { file: String, symbol: Option<String> } — opaque language-agnostic binding
- add_section_implementation mutate primitive (append-only, set semantics, validation at trust boundary)
- file path validation: workspace-relative POSIX shape (reject /, ./, .., \, //, trailing /)
- symbol validation: non-empty trimmed, no whitespace edges, no internal newline; no language regex
- duplicate (file, symbol) rejected as Validation error — fail-loud > silent dedup
- synthesize_section_body renders implementations as bullet block (style.rs paragraph filter)
- templates/section.md.tera adds **Implementations** block after **Examples**
- render.rs threads implementations into tera context with {file, symbol} JSON shape
- cmd_add_section_implementation CLI subcommand: --section / --file / --symbol / --sidecar / --json / --no-regenerate
- mnemosyne-cli main.rs dispatch arm + usage line + help text wired
- mnemosyne-mcp AddSectionImplementationArgs + add_section_implementation tool wrapping CLI
- no schema_version bump (additive serde-default-empty, backwards compatible)



**Verification**:
- cargo test --release --workspace PASS (449 tests, no regressions)
- cargo run --release -p mnemosyne-cli -- validate-workspace PASS (T1 orphan=0, round-trip 1/1, T3 reject=0, GENERATED.md=sync)
- new inline tests: round-trip, duplicate rejection, malformed file rejection, malformed symbol rejection, opaque symbol acceptance
- integration test atomic_section_round_trip_full_shape extended with implementations entries — render path verified
- render.rs render_section_full_shape extended; new Implementations block bytes verified
- existing atomic store ledger (7 entries, Rounds 252-258) deserializes unchanged — additive field defaults to empty




**Carry forward**:
- Round 260: extend code_refs.rs to detect §<id> citations + cross-check against implementations (bidirectional set-equality, Round 80 OPTION D pattern)
- Round 261: seed 3-5 atomic sections with implementations entries against real code files to exercise end-to-end check
- Round 262: self-application activation — enable [code_refs] in mnemosyne.toml, bulk-register 119 historical citations via orphan_ledger kind="code_citation" or severity_missing="warn"
- deferred: ranking / dedup heuristics for fuzzy file matches (only if Round 260+ surfaces friction)
- deferred: validator-time enforcement that every Active section have ≥1 implementations entry (consider only after 261 seeding informs realistic coverage)



### Round 260 — validate-code-refs bidirectional Spec ↔ Code binding — §<id> citation extractor + AtomicSection.implementations cross-check (Round 80 OPTION D pattern), Path B Stage 4 close

**Changes**:
- code_refs.rs: CodeRefViolation split into Citation + ImplementationUnbacked enum variants — domain asymmetry modeled honestly (no line=0 sentinel)
- code_refs.rs: ViolationKind extended with SectionMissing + CitationUnbound (Round 260 §<id> axis)
- code_refs.rs: extract_section_citations — §[A-Za-z0-9._/-]+ extractor with trailing-dot carve-out
- code_refs.rs: scan_paths_bidirectional — Round NNN axis + §<id> bidirectional set-equality + ledger suppression
- code_refs.rs: DefectClass enum (Hallucination | Binding | Decay) for semantic severity bucketing
- code_refs.rs: scan_paths / scan_paths_filtered retained as thin wrappers for Round 256/258 legacy callers
- config.rs: OrphanKind::CodeCitation variant added — code-axis suppression shape stable for Round 262 bulk register
- config.rs: CodeRefsSection.severity_binding field added (default reject); severity_missing doc updated to cover SectionMissing
- cli main.rs: cmd_validate_code_refs rewritten on the new shape; --severity-binding flag added parallel to --severity-missing
- cli main.rs: usage banner + help text updated for Round 260 bidirectional semantics
- cli main.rs: JSON shape extended with section_missing_count / citation_unbound_count / impl_unbacked_count / severity_binding
- cli main.rs: validate-workspace OrphanKind match arm extended for CodeCitation (no-op — code-axis handled by validate-code-refs)
- validate_code_refs_smoke.rs: 4 new cases viii/ix/x/xi covering SectionMissing / CitationUnbound / ImplementationUnbacked / severity-binding warn
- code_refs.rs inline tests: 11 new tests for §<id> extractor + bidirectional matrix + ledger suppression



**Verification**:
- cargo test --release --workspace PASS — no regressions across all crates
- cargo test --release -p mnemosyne-validator code_refs:: PASS (26 tests; 11 new for §<id> axis + bidirectional)
- cargo test --release -p mnemosyne-cli --test validate_code_refs_smoke PASS (11 tests; 4 new for Round 260 cases viii-xi)
- cargo run --release -p mnemosyne-cli -- validate-workspace PASS (T1 orphan=0, round-trip 1/1, T3 reject=0, GENERATED.md=sync)
- new bidirectional unit tests: clean codebase / SectionMissing / CitationUnbound / ImplementationUnbacked / orphan-ledger suppression both directions / filter-id silences section axis
- new §<id> extractor tests: basic numeric / fractional / slash slug / trailing dot / brackets-parens / solitary sigil / underscore allowed
- cli smoke cases viii-xi exercise the full CLI surface end-to-end including JSON shape and severity-binding flag




**Carry forward**:
- Round 261: seed 3-5 real Mnemosyne sections with implementations entries to exercise the bidirectional check against the live store
- Round 262: self-application activation — enable [code_refs] in mnemosyne.toml + bulk-register historical Round NNN citations as kind=code_citation orphan ledger rows
- deferred: validator-time enforcement that every Active section have ≥1 implementations entry (consider only after 261 seeding informs realistic coverage)
- deferred: auto-cascade trigger on set-section-decision-status (still blocked behind AtomicSection.decision_status schema extension)
- deferred: ranking / dedup heuristics for fuzzy file matches (only if Round 261+ surfaces friction)



### Round 261 — 5 atomic store sections seeded with 8 file-only implementation bindings — Path B (Round 260) validator gains spec-side coverage; namespace ratified as flat kebab + 1-level hierarchical sub-component (frozen)

**Changes**:
- §code-citation-defense seeded → code_refs.rs + main.rs (Round 255-260 layer)
- §code-citation-defense/bidirectional-binding seeded → code_refs.rs + atomic.rs (Round 259-260)
- §orphan-ledger seeded → config.rs (Round 80/253/254/260 set-equality reject pattern)
- §atomic-store-mutate-api seeded → atomic.rs + atomic_cli.rs (Round 161+ primitives)
- §markdown-parser seeded → parser.rs (Round 252 ATX + indented fence fix carry layer)
- atomic store sections count 0 → 5 explicit (+1 implied parent via Round 250 derivation)
- v1 file-only binding convention ratified — Round 260 matching is file-only, symbol opaque



**Verification**:
- cargo test --release --workspace PASS — no regression
- mnemosyne-cli validate-workspace PASS: sections=5, GENERATED.md=sync, orphan_refs=0+0
- T3 reject=0 carry stable (Round 138 tier mobility ratify)
- pre-commit hook gates 1-3 PASS (gate 3 still skip — [code_refs] unconfigured)



**Impact**: §code-citation-defense, §code-citation-defense/bidirectional-binding, §orphan-ledger, §atomic-store-mutate-api, §markdown-parser


**Carry forward**:
- Round 262: enable [code_refs] in mnemosyne.toml; observe ImplementationUnbacked surface
- Round 262 carry decision: bulk register kind=CodeCitation rows vs severity_binding=warn
- Round 263+: AtomicSection.decision_status atomic field extension (Stage B freshness)
- v1 file-only binding; v2 symbol-level matching deferred until empirical need surfaces



### Round 262 — code-citation-defense comment-only precision layer + [code_refs] permanent activation — strip_to_comments via CommentSyntax dispatch eliminates string-literal noise (1581 → 1107, -30%); severity warn baseline pending Round 263 reject promotion

**Changes**:
- comment_scanner layer in code_refs.rs — CommentSyntax (Slash/Hash/Unknown) + per-extension dispatch
- strip_to_comments preserves line numbers 1:1 (non-comment chars → spaces, line breaks intact)
- CodeRefsSection.comment_only config option (default true), unknown extension passthrough preserved
- scan_paths_bidirectional + scan_paths_filtered gain comment_only flag (legacy thin wrapper carries false)
- 11 unit tests for comment-only: syntax dispatch, line/block comment, string literal exclusion, unknown passthrough
- mnemosyne.toml [code_refs] permanent activation: paths=crates/, severity=warn/warn, comment_only=true
- surface reduction: 1581 → 1107 (-30% noise removal, mostly string-literal fixtures in tests)



**Verification**:
- cargo test --release --workspace PASS — 37 code_refs tests (11 new + 26 existing) all pass
- validate-workspace PASS: entries=10, sections=5, GENERATED.md=sync, orphan_refs=0+0
- pre-commit hook gates 1-3 PASS (gate 3 now active under warn — exits 0, prints surface)
- T3 reject=0 carry stable, no style regression



**Impact**: §code-citation-defense, §code-citation-defense/bidirectional-binding


**Carry forward**:
- Round 263: add // §slug comments to 8 ImplementationUnbacked files; promote severity_binding=reject
- Round 264+: legacy // Round N (N<252) comment handling — atomic absorb vs orphan_ledger vs comment delete
- v1 strip_to_comments limitations: raw strings, triple-quoted, shell heredocs — accepted miss cases
- formatted-marker option (require_marker=true) deferred until empirical need surfaces



### Round 263 — code-citation defense full reject — 8 ImpUnbacked binding markers + 192 src/ hallucination cleanup via comment-only sed transforms + extractor placeholder/backtick/digit-boundary improvements; mnemosyne.toml severity promoted to reject (paths narrowed to crates/*/src/); pre-commit gate 3 now actively blocks new hallucinated citations

**Changes**:
- 8 ImplementationUnbacked clears: §slug binding markers added to atomic.rs/atomic_cli.rs/code_refs.rs/config.rs/main.rs/parser.rs module doc-comments
- extract_section_citations gains placeholder filter (skip §X/§N single uppercase letters) and digit-digit boundary for `.` (so §39.implementations parses as §39 not §39.implementations)
- extract_section_citations gains single-line backtick skip — doc-comment `§39` examples no longer count as citations
- 60+ production src/*.rs files transformed via comment-only sed (Round NNN + legacy §digit tokens removed from line comments; string literals untouched)
- mnemosyne.toml [code_refs] paths narrowed to crates/*/src/; severity_missing + severity_binding promoted to "reject"
- src/ surface 192 → 0; pre-commit gate 3 now blocks any new hallucinated citation
- tests/ scope deferred to Round 264 (intentional Round NNN fixture data needs different handling)



**Verification**:
- cargo test --release --workspace PASS — 478 tests pass, no regression
- validate-code-refs PASS: total=0 across all kinds under severity=reject
- pre-commit hook 3 gates all PASS (gate 3 now active reject — would block any new hallucination)
- mnemosyne-cli validate-workspace PASS: entries=11, sections=5, GENERATED.md=sync, T1 orphan=0



**Impact**: §code-citation-defense, §code-citation-defense/bidirectional-binding


**Carry forward**:
- Round 264: tests/ scope strategy — intentional fixture cites need either kind=CodeCitation orphan_ledger entries or per-file [code_refs] exclusion list
- Round 264: legacy `// Round N (N<252)` retroactive atomic absorb option (versus permanent removal) for design rationale preservation
- v1 comment-only stripper limitations carry — raw strings, triple-quoted, shell heredocs (deferred until empirical bite)
- formatted-marker `[code_refs] require_marker = true` option deferred until empirical need surfaces



### Round 264 — code-citation defense closure — tests/ permanent exclusion (asset-class asymmetry: src/ rationale vs tests/ traceability) + legacy Round 1-251 stay in git log (Option α, time-integrity over external completeness); empirical Option D dry-run measured 373 tests/ violations confirming Option A would dilute orphan_ledger semantics; mnemosyne.toml header rewritten as positive policy, not deferral

**Changes**:
- tests/ scope = permanent exclusion (policy ratify, not deferral). [code_refs].paths stays at crates/*/src/ only; the asset-class asymmetry between src/ (rationale-bearing) and tests/ (traceability-bearing) makes one-size automation a worse outcome than per-asset judgment.
- Empirical Option D dry-run measured the tests/ surface: 373 violations across 35 test files (248 [missing] Round NNN + 125 [section_missing] §X), all real comment citations under comment_only=true. Top concentration at style_re_audit_full_scale.rs (103) — doc-comment tables citing legacy Round 155-160 design-budget rationale.
- Option A (paths += tests/, register all in orphan_ledger) was rejected because injecting ~100-373 rows into the ledger would dilute its "real exception" semantics — the ledger becomes a residue dump, future readers lose signal vs noise, and config audit value drops.
- Legacy Round 1-251 retroactive-absorb (Track 2): Option α ratified — git log carries 1-251 honestly, atomic store starts at Round 252 (post-MD-DELETION re-anchor). The Round 252 boundary itself is audit information; β/γ would have been external completeness at the cost of time-integrity (post-hoc fabrication of decisions that did not happen at the timestamps they claim).
- mnemosyne.toml [code_refs] header rewritten to document the Round 264 tests/ exclusion as positive policy (not "deferred" wording). The asset-class rationale now lives in the config comment so future auditors can reconstruct the decision.



**Verification**:
- Empirical measurement: tests/ added to [code_refs].paths under warn-mode; validate-code-refs surfaced 373 violations / 35 files. Distribution: top 10 files = 268/373 (72% concentration).
- Sampling confirmed comment_only=true correctly excluded fixture string literals — every surfaced violation is a real comment citation, same nature as src/ pre-Round-263.
- mnemosyne.toml restored to Round 263 baseline post-measurement; src/ surface remains 0 under reject mode.
- cargo run --release -p mnemosyne-cli -- validate-code-refs PASS: total=0 missing=0 section_missing=0 citation_unbound=0 impl_unbacked=0 decay=0.
- cargo run --release -p mnemosyne-cli -- validate-workspace PASS: docs=1/1, T1 orphan=0, round-trip mandatory=1/1, T3 reject=0.
- cargo test --release --workspace PASS — no regression on the 478 baseline tests.



**Impact**: §code-citation-defense


**Carry forward**:
- code-citation defense line closed at Round 264. Subsequent rounds (265+) move to schema/precision tracks: AtomicSection.decision_status atomic-field extension (Stage B freshness, blocks auto-cascade trigger), validator-time enforcement that every Active section has realistic coverage.
- tests/ permanent exclusion is policy, not deferral; future re-evaluation only if asset class equivalence changes (e.g., test doc-comments start carrying production rationale).
- Legacy Round 1-251 stay in git log only; atomic store starts at Round 252. Anyone needing pre-252 rationale grep git history. Do not re-open retroactive absorb without a concrete failure mode of the current α policy.
- v1 comment-only stripper limitations carry — raw strings, triple-quoted, shell heredocs (deferred until empirical bite).
- formatted-marker [code_refs] require_marker = true option deferred until empirical need surfaces.
- Tree-sitter language-aware extraction, fuzzy file match ranking/dedup, verify_round_citation MCP tool — deferred (no friction observed in two-call dance).



### Round 265 — AtomicSection.decision_status atomic field added (Stage B freshness substrate) — Option<DecisionStatus> with serde default None / skip_if_none, set_section_decision_status mutate primitive + CLI surface, query.rs atomic-first override of parser-hardcoded Active; auto-cascade trigger wiring deferred to Round 266+

**Changes**:
- AtomicSection.decision_status field added — Option<DecisionStatus>, serde default = None, skip_serializing_if = Option::is_none. Backward-compatible additive change (no schema_version bump needed; older readers see missing field as None).
- DecisionStatus enum gains Serialize/Deserialize derives with rename_all = lowercase, so JSON shape is "active" / "superseded" / "removed" matching the existing decision_status_str surface.
- New atomic mutate primitive set_section_decision_status (atomic.rs) + CLI surface set-section-decision-status (atomic_cli.rs + main.rs dispatch). Idempotent setter with no cross-doc validation — T1 rule 4 (active → superseded requires superseding cross-ref) carries to validate-workspace gate, not to the atomic write.
- query.rs build_section_view atomic-first decision_status resolution: atomic_store.section(id).decision_status overrides the parser-derived hardcoded Active when present; None falls back to parser status. Atomic-only section branch also reads atomic.decision_status with Active fallback.
- 4 new unit tests: serde round-trip persistence, idempotent overwrite, default-None / skip-serializing, atomic-override resolution across markdown-backed / atomic-only / no-override paths.



**Verification**:
- cargo test --release --workspace PASS — 482 tests (478 baseline + 4 new), 0 failures.
- cargo test --release -p mnemosyne-validator --lib PASS — 209 unit tests (was 205).
- cargo run --release -p mnemosyne-cli -- validate-workspace PASS — entries 13 → 14 after this Round 265 entry, sections=5, GENERATED.md=sync, T1 orphan=0, T3 reject=0.
- cargo run --release -p mnemosyne-cli -- validate-code-refs PASS under reject mode — Round 265 entry presence in atomic store unblocks the new src/ comment citations introduced by this round (validates the citation hygiene pipeline end-to-end).
- pre-commit hook gate 3 reject behavior verified mid-development: missing Round 265 entry caught 7 hallucinated citations from the same change set; entry registration cleared all 7. Demonstrates the Round 263 enforcement working as designed on a real authoring loop.



**Impact**: §atomic-store-mutate-api, §code-citation-defense


**Carry forward**:
- Round 266+ — auto-cascade trigger wiring: when set_section_decision_status transitions a section to Superseded, automatically run validate-code-refs --filter-id against citing entries to surface decay. Substrate (atomic field + setter) is now in place; trigger glue is the missing piece.
- Round 266+ — validator-time T1 rule 4 atomic-side enforcement: extend section_decision_status_transition to also cross-check the atomic override (currently only checks parser snapshot pair). Required when atomic-only sections start carrying Superseded status without a markdown counterpart.
- Legacy set_section_decision_status stub in mutate.rs still rejects with Phase 1+ carry message; deprecation path: route legacy callers to the atomic primitive once cascade trigger lands. Stub kept as-is to preserve §15 mutate API surface registration.
- AtomicStore schema_version stays at 1 (additive Option field is forward/backward compatible). Bump only when a non-additive change lands.



### Round 266 — auto-cascade trigger wired (Stage B freshness) — set-section-decision-status to Superseded/Removed runs scan_section_decay over [code_refs].paths and prints citing locations to stderr; informational only, never alters mutate success; silent no-op when [code_refs] unconfigured

**Changes**:
- code_refs::scan_section_decay function added — targeted §<id> scan returning Vec<Citation> for one section, comment_only honored, walk_paths reused. Public surface for cascade-trigger callers without going through the full bidirectional scanner.
- atomic_cli::print_section_decay_trigger wired into cmd_set_section_decision_status — fires when new status is Superseded or Removed; loads [code_refs] config via discover_config; runs scan_section_decay; prints "[cascade] §X → status — N citing location(s)" + per-line file:line to stderr. Informational only — never alters mutate success.
- Silent no-op when [code_refs] is unconfigured or paths empty (5-min setup promise carry); config-load errors logged to stderr but not propagated; scan io errors logged but not propagated.
- 3 new code_refs unit tests: target-section-only filter, empty-result path, comment_only flag honored. End-to-end smoke verified: §nonexistent-test-section status=superseded surfaces "0 citing locations" trigger output through actual CLI invocation.



**Verification**:
- cargo test --release -p mnemosyne-validator --lib PASS — 212 tests (was 209 after Round 265, +3 scan_section_decay).
- cargo build --release --workspace PASS — clean compile across 7 crates.
- End-to-end smoke: set-section-decision-status --section §nonexistent-test-section --status superseded surfaced "[cascade] §nonexistent-test-section → superseded — 0 citing location(s) in [code_refs].paths" on stderr, mutate succeeded on stdout.
- Smoke-test pollution (single empty atomic-only section) cleaned via direct JSON edit under explicit user override grant; generate-docs regenerated GENERATED.md (sections 6 → 5); validate-workspace PASS post-cleanup with entries=14, sections=5, GENERATED.md=sync, T1 orphan=0.
- validate-code-refs PASS under reject mode after this round's src/ comment additions (Round 265 entry presence keeps the citation hygiene loop closed).



**Impact**: §code-citation-defense, §atomic-store-mutate-api


**Carry forward**:
- Round 267+ — remove-section mutate primitive: smoke test exposed the missing inverse of section_mut. Currently set_section_decision_status (and any other atomic setter) implicitly creates an empty section if the id is absent, with no clean removal path short of direct JSON edit. Authoring loops that touch wrong section_ids cannot self-clean.
- Round 267+ — validate-workspace integration: extend the workspace gate so every atomic Superseded/Removed section auto-runs the decay scan, surfacing decay counts in the workspace report (currently the trigger only fires at mutate time, so a workspace-wide audit needs a separate command).
- Trigger over-fires on idempotent same-status set (Superseded → Superseded re-runs the scan). Acceptable in v1 (informational only, low cost); revisit if operator complaints surface.
- Trigger does not consult §X.implementations binding — purely citation-side. Spec-side ImplementationUnbacked surfacing on transition is a separate concern (likely fits in the validate-workspace integration above).
- Cleanup pollution required CLAUDE.md override grant; remove-section primitive (Round 267+) closes that gap and removes the override exception path for self-introduced test artifacts.



### Round 267 — remove-section mutate primitive added (closes Round 266 carry) — drops section from atomic store, requires --reason audit safeguard, NotFound on missing id; CLI surface + 3 unit tests; eliminates need for CLAUDE.md override-grant exception path on self-introduced authoring pollution

**Changes**:
- atomic::remove_section primitive added — drops a section entry from AtomicStore.sections, requires --reason (audit safeguard, mandatory non-empty trim check), returns NotFound when section_id absent (no silent no-op).
- atomic_cli::cmd_remove_section CLI surface + main.rs dispatch + help text. Mnemosyne mutate API surface count grows by 1 (remove-section); usage line updated.
- 3 new unit tests: drop+persist round-trip, empty reason rejection, NotFound for missing id. 215 validator-lib tests pass (was 212).
- Closes Round 266 carry item 1: smoke-test pollution (or any wrong-section_id authoring loop) now has a clean self-cleanup route without the CLAUDE.md override-grant exception path.



**Verification**:
- cargo test --release -p mnemosyne-validator --lib PASS — 215 tests (was 212 after Round 266, +3 remove_section).
- cargo build --release --workspace PASS across 7 crates.
- validate-code-refs caught 4 self-introduced Round 267 src/ comments under reject mode pre-entry-registration; entry presence clears all 4 — citation hygiene loop validated end-to-end again.
- No referential-integrity check inside the primitive: cross_refs / impact_scope dangling against a removed section_id surface at validate-workspace gate (kind=AtomicSectionRef) or [orphan_ledger] entries — separation of concerns preserved.
- Audit safeguard verified: remove_section with --reason "   " (empty after trim) rejected with Validation error; section unchanged.



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- Round 268+ — validate-workspace integration: extend the workspace gate to also auto-run section_decay scan for every atomic Superseded/Removed section, surfacing decay counts in the workspace report. Currently the trigger only fires at mutate-time (Round 266) — workspace-wide audit needs a separate command path.
- remove_section is unconditional once --reason is present (no referential-integrity pre-check). If a removed section_id is still cited from cross_refs / impact_scope / impact_refs / source-code §X citations, those become orphans visible at validate-workspace. Deferred until empirical bite: a --force / --check-refs split if the no-check default proves footgun-prone.
- Companion remove_changelog_entry primitive NOT added — changelog entries are append-only audit history (frozen ledger). Removal would violate the frozen-ledger invariant. If a changelog entry was authored in error, the correct path is a superseding entry that documents the correction.
- ChangelogEntry-level mutate API still missing: set_decision_summary / append-only changes_bullets edits / etc. for in-progress entries (pre-freeze). Out of scope for the code-citation-defense arc; surface separately if authoring loop friction emerges.



### Round 268 — validate-workspace decay surface integration (Round 266 carry item 2 closed) — print_atomic_decay_surface walks Superseded/Removed atomic sections and runs scan_section_decay against [code_refs].paths; informational only, never affects exit code; smoke loop validated remove-section closure (Round 267) replacing CLAUDE.md override-grant for self-introduced cleanup

**Changes**:
- main.rs::print_atomic_decay_surface added — workspace-wide companion to the Round 266 mutate-time trigger. Reads atomic store, walks sections with decision_status=Some(Superseded|Removed), runs scan_section_decay against [code_refs].paths, prints "atomic decay surface: N citation(s) across M superseded/removed section(s)" + per-section break-down lines (only sections with non-zero hits listed in the break-down).
- Wired into cmd_validate_workspace right before the final OK return; informational only, never affects exit code (matches Round 266 mutate-time trigger semantics). Silent no-op when [code_refs] unconfigured or no Superseded/Removed sections exist.
- End-to-end smoke loop validated through remove_section closure: created §round268-smoke via mutate API, set status Superseded (mutate-time trigger fired with 0 citations), validate-workspace surfaced "atomic decay surface: 0 citation(s) across 1 superseded/removed section(s)", remove-section cleaned up — no CLAUDE.md override grant needed (Round 267 closure proven).



**Verification**:
- cargo build --release --workspace PASS — clean compile.
- cargo test --release --workspace PASS — 488 tests, 0 failures (no new tests this round; behavior is exercised end-to-end via the validate-workspace smoke loop).
- validate-workspace baseline (no Superseded/Removed sections): output unchanged, no decay-surface line emitted.
- validate-workspace with §round268-smoke at status=superseded: emitted "atomic decay surface: 0 citation(s) across 1 superseded/removed section(s)" — gate stayed PASS, exit code 0 (informational semantics confirmed).
- validate-code-refs caught 2 self-introduced Round 268 src/ citations under reject; entry registration cleared both.
- Round 267 remove-section primitive used to clean smoke section without override grant — first end-to-end validation that the new mutate API replaces the JSON-direct-edit exception path for self-introduced authoring artifacts.



**Impact**: §code-citation-defense, §atomic-store-mutate-api


**Carry forward**:
- Round 269+ — code-citation-defense arc closure: with Round 264 (defense closure) + Round 265 (atomic decision_status field) + Round 266 (mutate-time trigger) + Round 267 (remove-section primitive) + Round 268 (validate-workspace decay surface) all landed, the Stage A reject + Stage B freshness substrate is feature-complete for code-side. Subsequent rounds move to spec-side concerns (validator-time enforcement that every Active section has realistic implementation coverage; T1 rule 4 atomic-side cross-check) or to schema/precision tracks (v2 symbol-level matching, raw-string stripper, Tree-sitter language-aware extraction).
- Decay surface count = 0 baseline carries through current self-application (no Superseded/Removed sections exist). First non-zero surface will appear when the spec authoring workflow exercises status transitions in earnest — at that point operator feedback will inform whether the informational-only semantic is sufficient or whether a --reject-on-decay flag is warranted.
- print_atomic_decay_surface is duplicated across mutate-time (atomic_cli) and workspace-time (main.rs) call sites with similar but not identical shapes (mutate-time prints to stderr per-line, workspace-time prints to stdout summary+breakdown). Acceptable v1 — refactor only if a third caller site emerges.



### Round 269 — ImplementationMissing variant added — spec-side coverage axiom (Active = backed by code). Third edge of Path B set-equality complementing CitationUnbound + ImplementationUnbacked. Detection-only this round: severity bucketed under existing severity_binding (C1, YAGNI), Round 270+ carries severity_coverage split decision pending empirical evidence. Option<DecisionStatus> stored raw (not pre-resolved to Active) so audit-trail consumers can distinguish parser-default from explicit override. Removed status tombstone-exempt; Superseded + Active + None all trigger.

**Changes**:
- CodeRefViolation enum gains 3rd variant `ImplementationMissing { section_id, decision_status: Option<DecisionStatus> }` — third edge of the Path B set-equality (CitationUnbound + ImplementationUnbacked + ImplementationMissing) representing the "Active = backed by code" axiom. kind_tag = "impl_missing", defect_class = DefectClass::Binding (joins existing two binding edges in the same severity bucket). Module header doc rewritten 2-variant → 3-variant rationale with explicit shape-asymmetry note (no file/line for section-level absences).
- decision_status field stored as raw `Option<DecisionStatus>` rather than pre-resolved to Active — None → Active fallback is a Round 265 consumer-side convention, so resolving at emission time would discard authoring intent. Audit-trail consumers can distinguish "no atomic override (parser default)" from "atomic override = Active" downstream.
- scan_paths_bidirectional gains step 4 (workspace-wide section enumeration): for each section where `decision_status.unwrap_or(Active) != Removed AND implementations.is_empty()`, emit ImplementationMissing. Skipped under filter_id (decay-scan mode) for surface-narrowing symmetry with steps 2-3. Removed is tombstone-exempt; Superseded triggers (audit gap "marked dead but never recorded where it lived"); Active and None both trigger.
- sort_violations switched from pairwise match to rank-then-sort: Citation < ImplementationUnbacked < ImplementationMissing preserves diff stability for existing tests when the third edge surfaces. Sort key for the new variant is section_id only (no file/line/symbol).
- CLI cmd_validate_code_refs grows impl_missing_count: counts[6], JSON impl_missing_count field, text "violations:" line + per-violation "[impl_missing] §<id> (status=<status>)" lines (status renders "active"/"superseded"/"removed" or "none(default-active)" for raw Option exposure). binding_count now sums citation_unbound + impl_unbacked + impl_missing (C1 placement: severity_binding bucket reused, no new severity_coverage flag — empirical evidence for separate policy not yet observed, mirroring Round 262 → 263 measure-then-promote pattern).
- 6 new unit tests in code_refs.rs::tests covering Active+empty (triggers), None+empty (triggers, raw None preserved), Superseded+empty (triggers, audit gap), Removed+empty (exempt, tombstone), non-empty impls (exempt across all statuses), filter_id silences coverage axis (decay-mode symmetry).



**Verification**:
- cargo build --release --workspace PASS — clean compile, no new warnings.
- cargo test --release --workspace PASS — 488 → 494 tests (+6 coverage_axiom unit tests), 0 failures, no regression.
- validate-workspace PASS: entries 17 → 18, sections=5, orphan_refs=0+0, GENERATED.md=sync (auto-regenerate honored). Atomic decay surface stays at 0 (no Superseded/Removed sections in self-application).
- validate-code-refs PASS under reject mode: impl_missing=0 baseline confirmed (Round 261 seeding gave all 5 self-app sections non-empty implementations; the new axiom adds 0 surface in self-application). Self-introduced Round 269 src/ citations (4 hits across cli main.rs + validator code_refs.rs comments/tests) cleared by entry registration in this round.
- Pre-commit hook 3-gate PASS — gate 3 (validate-code-refs under reject) confirmed clean post-entry-append.
- Citation hygiene (Round 255) honored: Round 269 added to atomic store BEFORE the source files referencing it can pass validate-code-refs reject; entry append re-validated post-write.



**Impact**: §code-citation-defense, §code-citation-defense/bidirectional-binding


**Carry forward**:
- Round 270+ — severity_binding → severity_coverage split decision: this round bucketed ImplementationMissing under severity_binding (C1, YAGNI + Round 262→263 measure-then-promote pattern). Carry the question of whether the two binding edges (ImplementationUnbacked file-grained vs ImplementationMissing section-level) warrant independent policy until empirical evidence emerges (external workspace adoption surface, or self-application authoring friction). Do NOT split preemptively; setting kit are expensive (deprecation cost on external users) and the defect_class already groups them.
- Round 270+ — pre-commit gate 4 / coverage-axiom-specific reject promotion: not needed in this round (severity_binding=reject already gates the surface). Add a dedicated gate only if step 4 surface diverges in behavior from steps 2-3 (e.g. coverage gaps that are intentionally tolerated transitionally during status transitions).
- Round 271+ — Section.implementations append-only state-entry guard for Active transitions: today implementations is append-only at the schema level but no validator-time precondition exists requiring ≥1 impl before set-section-decision-status accepts an Active target. Currently moot (Active is the default and transitions are only Active → Superseded → Removed), but add the guard if a re-activation path is ever introduced.
- Round 272+ — T1 rule 4 atomic-side cross-check: rule 4 currently sees parser snapshots only; cross-checking atomic decision_status Superseded transitions against superseding cross-ref presence requires the validator to read the atomic store during T1 evaluation. Substrate available post-Round 265; wiring deferred.
- Round 268 carry held — print_atomic_decay_surface unification across mutate-time (atomic_cli) and workspace-time (main.rs) still waits for a third caller before refactor (YAGNI).
- Round 267 carry held — ChangelogEntry-level mutate API for in-progress entries (set_decision_summary, etc.) — surface separately if authoring loop friction emerges.
- Deferred until empirical bite: v2 symbol-level matching, raw-string stripper, Tree-sitter language-aware extraction, formatted-marker require_marker option, fuzzy file match ranking/dedup, verify_round_citation MCP tool.



### Round 272 — T1 rule 4 atomic-axis closure. Author-time guard on set_section_decision_status (Superseded target now requires superseding: Option<&str>) plus state-based validate-workspace gate that walks AtomicStore.sections for Some(Superseded) lacking a superseding cross-ref. Symmetric with the markdown-axis dual-layer pattern (mutate guard + parser-pair transition check). SupersedeMissingRef variant reused — no new violation kind, no new schema field, no new severity flag. Removed status tombstone-exempt; --superseding hard-required at CLI; AtomicMutateError::Validation reused over a new ValidatorReject variant. Closes the integrity gap explicitly acknowledged in round-265's atomic.rs doc comment.

**Changes**:
- set_section_decision_status signature gains `superseding: Option<&str>`. Pre-write check rejects Superseded target with None via AtomicMutateError::Validation prefixed "(T1 rule 4, atomic axis)" — symmetric with the markdown-axis guard at mutate::set_section_decision_status. Existence checking is deferred to rule 1 (validate-workspace) on both axes. Removed status tombstone-exempt (asserts finality, not replacement). Closes the integrity gap explicitly acknowledged in the prior round-265 doc comment ("enforced at validate-workspace time, not at this atomic write").
- CLI `set-section-decision-status` grows `--superseding §<id>` flag. Rejected (bail with explicit message) when `--status != superseded` — forward-pointer is only meaningful when the section asserts replacement. Symmetric with the markdown-axis CLI at cmd_set_section_decision_status. `§` prefix stripped before passing into the mutate primitive.
- atomic_section_supersede_state_reject added in validator.rs as the post-condition counterpart to section_decision_status_transition. Walks AtomicStore.sections for Some(Superseded), checks any parsed_doc cross_ref FROM that section_id with RefKind::Decision|Impl. State-based (snapshot) rather than transition-based (prev/curr pair) — catches violations invisible to the parser-pair walk: writes that predate the author-time guard and atomic-only overrides where no markdown prev snapshot ever carried Superseded. Synthesizes prev_status=Active (the only legal predecessor) to reuse SupersedeMissingRef without schema churn.
- cmd_validate_workspace wires the new gate after the GENERATED.md sync check (line 1363) and before print_atomic_decay_surface (line 1377) — placement matches its semantics as a hard reject gate, not informational scan. parsed_docs threaded as `parsed_docs.iter().map(|(_, doc)| doc).collect::<Vec<&ParsedDoc>>()`. Bail message names the rule and the remediation paths explicitly (add-cross-ref, or revert to Active|Removed).
- AtomicMutateError::Validation reused over a new ValidatorReject variant — sibling guards in the same file (add_section_implementation duplicate-reject at atomic.rs:670+, validate_implementation_file boundary check, validate_implementation_symbol) all use Validation for rule-style rejection. Per-rule variants would invent precedent without a consumer dispatching on them. Diagnostic discrimination lives in the message string prefix.
- 7 new unit tests: 3 author-guard tests in atomic.rs (Superseded+None rejects with "(T1 rule 4, atomic axis)" attribution, Active|Removed+None writes cleanly, Superseded+Some writes) + 4 state-gate tests in validator.rs (Superseded+no-ref rejects with SupersedeMissingRef, Superseded+ref passes clean, Removed tombstone-exempt regardless of refs, Active|None skip). Existing call sites in atomic.rs::tests updated to pass `Some("X")` for Superseded targets and `None` for Active.



**Verification**:
- cargo build --release --workspace PASS — clean compile, no new warnings across all 7 crates.
- cargo test --release --workspace PASS — 494 to 501 tests (+7 author-guard + state-gate unit tests), 0 failures, no regression on the prior round-269 baseline.
- validate-workspace PASS: entries 18 to 19, sections=5, T1 orphan=0, round-trip mandatory=1/1, atomic ledger orphan_refs=0+0, GENERATED.md=sync (auto-regenerate honored). Atomic decay surface stays at 0 — no Superseded sections in self-application, so the new state gate adds 0 surface to baseline.
- validate-code-refs PASS under reject mode: total=0 violations across all axes (missing=0 section_missing=0 citation_unbound=0 impl_unbacked=0 impl_missing=0 decay=0). Self-introduced citations cleared by entry registration in this round.
- Pre-commit hook 3-gate PASS — gates 1/2/3 still clean post-mutate.
- Citation hygiene (round 255) honored: Round 272 entry added to atomic store BEFORE src/ files referencing it pass validate-code-refs reject; entry append re-validated post-write. Stray `§2` citation in a test comment caught and rewritten as `section "2"` (the exact failure mode rule 1 + reject-mode pre-commit gate are designed to catch).



**Impact**: §atomic-store-mutate-api, §code-citation-defense/bidirectional-binding


**Carry forward**:
- Round 271 — Section.implementations append-only state-entry guard for Active transitions: still pending. This round's author-time guard pattern (mandatory arg + symmetric CLI gate + state-based post-condition gate) provides a direct template. Currently moot under the today's transition shape (Active is default and transitions only flow Active → Superseded → Removed), but add the guard if a re-activation path is ever introduced.
- Round 270+ — severity_binding to severity_coverage split policy: empirical-evidence-pending, no movement this round. Round 272 reused severity_binding without contributing new evidence — the rule-4 substrate doesn't bucket into either binding or coverage (it's correctness-class, T1 reject only). Split decision still waits for external workspace authoring friction or the C1 placement bucket exceeding its tolerance.
- Round 268 carry held — print_atomic_decay_surface unification across the mutate-time (atomic_cli) and workspace-time (main.rs) sites still waits for a third caller before refactor. YAGNI carry stable.
- Round 267 carry held — ChangelogEntry-level mutate API for in-progress entries (set_decision_summary etc.) — surface separately if authoring loop friction emerges.
- Atomic-store-resident cross_refs: today's state gate consults parsed docs only. If a section is deleted from the markdown axis but the atomic cross_refs survive, the current check might miss the violation. Concrete: AtomicStore today doesn't even carry cross_refs as a top-level field — they live in parsed_doc only. Surface separately as a schema extension if this becomes observable (Removed-then-revived-via-atomic-only would be the symptom).
- Symbol-level matching for code-ref validation: deferred until empirical bite — current file-level granularity matches Round 261 seeding and surfaces 0 self-application false-negatives.
- Deferred until empirical bite: v2 symbol-level matching, raw-string stripper, Tree-sitter language-aware extraction, formatted-marker require_marker option, fuzzy file match ranking/dedup, dedicated verify_round_citation MCP tool.



### Round 273 — Phase 1A 진입 — InventoryEntry 5번째 closed-form 엔티티 schema 추가 (AtomicStore.inventory_entries + schema_v2 back-compat). TC8 외부 dogfood 채택 P0 substrate.

**Changes**:
- InventoryStatus enum (Active/Deprecated/Reserved, Default=Active, serde snake_case rename)
- InventoryEntry struct (status/section_ref/source/reason — body 없음, T2 frozen-ledger 부재)
- AtomicStore.inventory_entries: BTreeMap<String, InventoryEntry> (#[serde(default)] back-compat)
- inventory(id) read-only 헬퍼 + atomic_inventory_id_set() (R275 cite 검증 substrate)
- CURRENT_SCHEMA_VERSION 1→2 bump (v1 store 자동 upgrade on save)
- _mut 헬퍼 의도적 부재 — cite lookup side-effect auto-register 차단



**Verification**:
- cargo test --workspace 0 failure (atomic::tests 46 통과, 신규 4 포함)
- schema_version_1_store_loads_with_empty_inventory: v1 JSON load → save → schema_v2 round-trip
- inventory_entry_round_trip: ARP_07 Active + TCP_RETRANSMISSION_TO_04 Deprecated shape 보존
- atomic_inventory_id_set: ARP_07/TCP_FLAGS_INVALID_02/SOMEIP_ETS_BASICS_01 평행 검증
- validate-workspace 통과 — entries=19 sections=5 T1 orphan=0 round-trip 1/1



**Impact**: §atomic-store-mutate-api, §code-citation-defense


**Carry forward**:
- R274 — mutate primitives 4종 (add/set_status/set_section_ref/remove) for InventoryEntry
- R275 — validator T1 inventory-axis: cite 시 ID 존재 검증 + status=Deprecated reject
- R276 — cascade gate: status 전이 (Active→Deprecated) cite-site scan_inventory_decay 트리거
- R277 — P0: entry_id_prefixes Vec<String> + tail_pattern Numeric/AlphanumericUpper
- R278 — P1: external-standard prefix context (§ 앞 RFC \d+ / IEEE \d+ skip)
- R279 — CLI query --list-inventory + MCP tools + TC8 dogfood baseline self-application
- GENERATED.md round-trip 정책: inventory_entries atomic-only (R273 ratify, GENERATED 비대상)
- R271 carry held — Section.implementations append-only state-entry guard (Phase 1A 와 독립)
- R270+ carry held — severity_binding to severity_coverage split (Phase 1A 비영향)
- R268 carry held — print_atomic_decay_surface unification (third caller 대기 YAGNI)
- R267 carry held — ChangelogEntry-level mutate API for in-progress entries



### Round 274 — InventoryEntry 4 mutate primitives (add/set_status/set_section_ref/remove) + CLI handlers + workspace 517/0 통과. R275 validator inventory-axis substrate.

**Changes**:
- atomic.rs: add_inventory_entry/set_inventory_status/set_inventory_section_ref/remove_inventory_entry
- validate_inventory_id 헬퍼 (non-empty + no whitespace edges + no internal whitespace)
- add: duplicate reject (set semantics), section_ref pre-stripped 강제 (§ 시작 reject)
- set_inventory_status reason: None=preserve, Some("")=clear, Some(non_empty)=overwrite
- remove_inventory_entry: --reason 필수 (remove_section R267 audit-safeguard mirror)
- atomic_cli.rs: 4 CLI handlers (--id/--status/--section/--source/--reason/--clear)
- main.rs: usage line + dispatch arms + help text (Phase 1A inventory mutate API)



**Verification**:
- cargo test --workspace 0 failure (517 통과, 신규 inventory mutate 13개 포함)
- atomic::tests: add_basic/duplicate/invalid_id/sigil_reject + set_status 4종 + section_ref + remove 3종
- CLI smoke: SMOKE_TEST_01 add → deprecated → clear/set ref → duplicate reject → remove cycle
- validate-workspace entries=20 sections=5 GENERATED.md=sync, smoke cleanup 후 git diff 정확



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- R275 — validator T1 inventory-axis: ID cite 존재 검증 + Deprecated cite-time reject
- R276 — cascade gate: set_inventory_status Deprecated 전이 시 scan_inventory_decay 트리거
- R277 — P0: entry_id_prefixes Vec<String> + tail_pattern Numeric/AlphanumericUpper
- R278 — P1: external-standard prefix context (§ 앞 RFC \d+ / IEEE \d+ skip)
- R279 — CLI query --list-inventory + MCP tools + TC8 dogfood baseline
- set_inventory_status 단일 setter: status+reason 묶음, 분리 (set_inventory_reason) 는 friction 측정 carry
- validate_inventory_id 길이/문자클래스 제한 미도입 — empirical bite 까지 자유 형식 carry
- set_inventory_section_ref: API Option None=unset 의미 (superseding의 None 과 다른 결)
- R271 carry held — Section.implementations append-only state-entry guard (Phase 1A 와 독립)
- R270+/R268/R267 carry held — Phase 0 hardening carry, Phase 1A 비영향



### Round 275 — Inventory cite-axis + P0 multi-prefix extractor 통합 (원래 R275+R277). scan_paths_bidirectional_v2 + ViolationKind 2 신규 + DefectClass::Inventory. Active/Reserved silent, Deprecated/Missing reject. 528 tests / 0 failure.

**Changes**:
- config.rs: [code_refs].inventory_prefixes Vec<String> + severity_inventory (default reject)
- code_refs.rs: ViolationKind::InventoryMissing / InventoryDeprecated + DefectClass::Inventory
- code_refs.rs: extract_inventory_citations (multi-prefix, longest-prefix-first, alphanumeric tail)
- tail digit-terminus 규칙 — TCP_BUFFER_SIZE 등 코딩 상수 false-positive 차단
- code_refs.rs: scan_paths_bidirectional_v2 신설 + v1 wrapper 가 back-compat 위임
- scan_v2 inventory axis: Missing/Deprecated reject, Active/Reserved silent (Reserved 도 cite-허용)
- main.rs: --severity-inventory flag + validate-code-refs JSON/text 출력 + reject gate
- Round 275 (R275) 합침: 원래 7라운드의 R275(inventory-axis) + R277(P0 multi-prefix) 통합



**Verification**:
- cargo test --workspace 0 failure (528 통과, 신규 R275 inventory 10 + R273/R274 carry)
- code_refs::tests 신규: extract basic/multi-prefix/digit-terminus/longest-prefix/word-boundary/backtick/empty
- scan v2: inventory_missing_reject/deprecated_reject/active+reserved_silent/v1_wrapper_disables_axis
- Mnemosyne self mnemosyne.toml inventory_prefixes 미설정 → axis silent, validate-workspace 변화 없음
- validate-code-refs 자체 회귀: Round 275 cite 15 + § 4.2.4 backtick 보강 후 통과 (post-changelog)



**Impact**: §code-citation-defense, §atomic-store-mutate-api


**Carry forward**:
- 원래 R277 P0 (multi-prefix) — R275 에 흡수 완료, R277 슬롯 폐기
- R276 — cascade gate: set_inventory_status Deprecated 전이 시 scan_inventory_decay 트리거 (R266 패턴)
- R277 (구 R278) — P1: external-standard prefix context (§ 앞 RFC \d+ / IEEE \d+ 컨텍스트 보존, skip)
- R278 (구 R279) — CLI query --list-inventory + MCP tools + TC8 dogfood baseline self-application
- validate-workspace summary 에 inventory violation count surface — R277 이후 carry (atomic_cli.rs 변경 별 동선)
- inventory orphan_ledger suppression — CodeCitation 와 같은 패턴 가능, empirical bite 까지 carry
- tail digit-terminus 규칙 외 prefix 패턴 확장 (lowercase, mixed-case) — empirical bite 까지 미도입
- R271/R270+/R268/R267 carry held — Phase 0 hardening 독립 트랙



### Round 276 — Inventory mutate-time cascade gate. scan_inventory_decay + print_inventory_decay_trigger + 3 CLI handlers wired (add deprecated / set deprecated / remove). 531 tests / 0 failure. R266 패턴 mirror.

**Changes**:
- code_refs.rs: scan_inventory_decay 함수 — extract_inventory_citations 위 target-id filter
- atomic_cli.rs: print_inventory_decay_trigger 헬퍼 — R266 print_section_decay_trigger 패턴 mirror
- cmd_add_inventory_entry: status=Deprecated 등록 시 cascade ("added(deprecated)" label)
- cmd_set_inventory_status: Deprecated 전이 시 cascade ("deprecated" label); 다른 status 는 silent
- cmd_remove_inventory_entry: 항상 cascade ("removed" label) — entry 가 사라져 모든 cite 가 Missing 됨
- cascade silent 조건: [code_refs] 미설정 OR inventory_prefixes 비어있음 (R275 5-min-setup 일관)
- mutate 성공/실패에 cascade 가 영향 없음 — informational only (R266 패턴 일관)



**Verification**:
- cargo test --workspace 0 failure (531 통과, 신규 scan_inventory_decay 3 + R275 carry)
- code_refs::tests: scan_inventory_decay_surfaces_only_target_id / empty_prefixes / comment_only
- CLI smoke: add R276_SMOKE deprecated + remove → mnemosyne inventory_prefixes 비어 silent (no false output)
- validate-workspace entries=22 sections=5 GENERATED.md=sync (post-smoke cleanup git diff 정확)



**Impact**: §code-citation-defense, §atomic-store-mutate-api


**Carry forward**:
- R277 (구 R278) — P1: external-standard prefix context (§ 앞 RFC \d+ / IEEE \d+ 컨텍스트 보존)
- R278 (구 R279) — CLI query --list-inventory + MCP tools + TC8 dogfood baseline self-application
- cascade DecisionStatus reactivation 미트리거 일관: Section R266 도 Active 로 복귀 시 cascade 안 함
- inventory orphan_ledger suppression — CodeCitation 와 평행, empirical bite 까지 carry
- validate-workspace summary 에 inventory violation count surface — R277 또는 R278 라인 carry
- scan_inventory_decay 의 paths arg: code_refs.paths 만, tests/ asymmetry R263 carry 일관
- R271/R270+/R268/R267 carry held — Phase 0 hardening 독립 트랙



### Round 277 — External-standard § skip (P1). extract_section_citations_v2 + scan_paths_bidirectional_v3. RFC/IEEE/ISO/IEC prefix + numeric + § → skip SectionMissing/CitationUnbound. TC8 854 RFC false-positive 제거 substrate. 540 tests / 0 failure.

**Changes**:
- config.rs: [code_refs].external_section_prefixes Vec<String> (default empty = skip 비활성)
- code_refs.rs: extract_section_citations_v2(content, external_prefixes) — § 앞 token 검사
- is_external_section_cite 헬퍼: prefix + space + numeric + space + § 패턴 매칭
- v1 extract_section_citations 가 v2(empty) 위임 — back-compat 보존
- scan_paths_bidirectional_v3 (9 args) + v2 wrapper 가 v3(empty external) 위임
- main.rs: scan_paths_bidirectional_v3 호출 + JSON/text 에 external_section_prefixes 표시
- single-token prefix 만 v1: ETSI TS 같은 multi-token 은 trailing-token workaround carry
- doc 코멘트의 RFC/IEEE/ISO example backtick 으로 감싸기 (self-scan false-positive 차단)
- test fixture 의 § 를 \u{00a7} escape — source byte 에 § literal 없음



**Verification**:
- cargo test --workspace 0 failure (540 통과, 신규 R277 9 + R276 carry)
- code_refs::tests: extract_v2 skip RFC/IEEE/ISO/IEC, keep internal, empty=v1, whitespace req, mixed
- scan_v3: external RFC skip → no SectionMissing, internal §99 still fires after skip
- self-application: section_missing=0 (이전 false-positive 11 → 0), 남은 6 missing 모두 forward-cite "Round 277"



**Impact**: §code-citation-defense


**Carry forward**:
- R278 (구 R279) — CLI query --list-inventory + MCP tools + TC8 dogfood baseline self-application
- multi-token external prefixes (ETSI TS) — trailing-token workaround carry (v2 carry 까지)
- strip_to_comments 의 string literal 안 `//` 처리 — R263 carry, R277 fixture 회피로 임시 해결
- inventory orphan_ledger suppression — CodeCitation 평행, empirical bite 까지 carry
- validate-workspace summary 에 inventory + external violation count surface — R278 carry
- external prefix 매칭 case-sensitive — case-insensitive 옵션은 friction 측정 후 carry
- R271/R270+/R268/R267 carry held — Phase 0 hardening 독립 트랙



### Round 278 — Phase 1A 클로저 — CLI query --list-inventory + --inventory <id> + MCP 6 tools (list/query + add/set_status/set_section_ref/remove). TC8 외부 dogfood baseline READY: P0+P1+5th entity+cite-axis+cascade+query 완. 540 tests/0 failure.

**Changes**:
- CLI QueryArgs: list_inventory + inventory_id 필드 + flag/value parsing
- cmd_query: --list-inventory (BTreeMap order, JSON/text) + --inventory <id> (single lookup) branches
- main.rs help text: query --list-inventory / --inventory <ID> 노출
- MCP arg structs: InventoryIdArgs/AddInventoryEntryArgs/SetInventoryStatusArgs/SetSectionRef/Remove
- MCP tools 6 신규: list_inventory/query_inventory/add/set_status/set_section_ref/remove
- MCP description: cite-time reject/cascade 동작 명시 — agent 가 author-time 검증 가이드



**Verification**:
- cargo test --workspace 0 failure (540 통과, R278 = CLI/MCP wire 만, validator 변경 없음)
- CLI smoke: add R278_SMOKE active + §atomic-store-mutate-api → list (1 entry) → query --inventory → remove
- MCP cargo build pass — 6 신규 tool macro expansion clean
- self mnemosyne inventory_prefixes 미설정 → list-inventory total=0, query --inventory NOTEXIST NotFound



**Impact**: §atomic-store-mutate-api, §code-citation-defense


**Carry forward**:
- TC8 dogfood READY — Mnemosyne 측 P0+P1+5th entity+cite-axis+cascade+query/MCP 완 (R273-278 6 라운드)
- TC8 측: mnemosyne.toml 작성 (inventory_prefixes=8개 + external_section_prefixes=[RFC,IEEE])
- TC8 측: PDF→case_inventory.json→add-inventory-entry sync script (543 active + 13 deprecated)
- TC8 측: severity_inventory=warn 시작 → baseline 청소 → reject 승격 (R262→R263 패턴)
- Phase 1A 종료 ratify — 5번째 closed-form 엔티티 schema + cite-axis 완전 통합
- multi-token external prefix (ETSI TS) — trailing-token workaround stable, full 처리 carry
- v1/v2/v3 wrapper chain 누적 — 다음 axis 추가 시 ScanOptions 리팩터로 통합 carry
- validate-workspace summary 에 inventory + external count surface — Phase 1B 입구 carry
- R271/R270+/R268/R267 carry held — Phase 0 hardening 독립 트랙 stable



### Round 279 — TC8 dogfood bug fix bundle (4 bugs + regression tests). UTF-8 panic in inventory extractor (P0), [atomic] sidecar_path config wiring (P1), [atomic] output_path explicit knob (P1), CLI help field-cap surfacing (P2). 552 tests / 0 failure.

**Changes**:
- Bug #1 P0 fix: code_refs.rs extract_inventory_citations byte-loop → char_indices peekable
- multi-byte char (em-dash / 한글 / CJK) 가 prefix 앞에 있어도 line[i..] panic 없음
- Bug #2 P1 fix: config.rs AtomicConfigSection (sidecar_path + output_path) — 옵션 B 실제 구현
- atomic_cli.rs resolve_sidecar 가 config 따름; precedence: CLI --sidecar > [atomic].sidecar_path > default
- Bug #3 P1 fix: [atomic] output_path 명시적 knob 신설 — docs[0] 자동 derivation 폐기
- 이유: docs[0] = parse target, output_path = cascade write — 자동 derivation 은 hand-authored content 덮어쓸 위험
- atomic.rs doc-comment 갱신: sidecar_path / output_path 둘 다 config-aware 명시
- Bug #4 P2 fix: CLI help text 에 intent ≤ 200 + bullets ≤ 100 chars cap surfacing



**Verification**:
- cargo test --workspace 552 passed / 0 failed (R278 540 + 12 신규)
- Bug #1 regression: extract_inventory_citations_survives_non_ascii_comment_chars (em-dash + 한글 + CJK)
- Bug #1 regression: scan_v3_survives_non_ascii_comment_chars (full scan + strip_to_comments)
- Bug #2 regression: parse_atomic_sidecar_path / atomic_section_optional_when_absent (config.rs)
- Bug #2 regression: atomic_cli resolve_sidecar 4 case (CLI wins / config / built-in / absolute)
- Bug #3 regression: resolve_output 4 case (CLI wins / atomic.output_path / docs[0] no-derivation / default)
- CLI smoke: tc8 repro panic 사라짐 + [atomic] sidecar + output_path 실 적용 확인
- validate-workspace entries=25 sections=5 sync, self-application 동작 변화 없음



**Impact**: §code-citation-defense, §atomic-store-mutate-api


**Carry forward**:
- TC8 dogfood 차단 해제 — Bug #1 P0 해결, 다국어 코드베이스 첫 mutate 가능
- TC8 측 mnemosyne.toml: [atomic] sidecar_path + output_path 명시 권장 (config-truth)
- README/SCHEMA_GUIDE 에 Field length caps + [atomic] 섹션 문서 추가 — 별도 docs 라운드 carry
- AtomicConfigSection 이름 결정 (vs atomic::AtomicSection 의 namespace 충돌 회피)
- output_path docs[0] 자동 derivation 폐기 — Bug 리포트 옵션 A 보다 옵션 C 가 안전
- validate_atomic_store 의 sidecar_path 도 resolve_sidecar 일관 사용 — 차후 검토 carry
- TC8 측 643 entries seed 완료된 상태에서 Bug #1 unblock → Phase D 진입 가능
- multi-token external prefix (ETSI TS) carry 유지 — Phase 1B
- ScanOptions struct 리팩터 carry 유지 — v1/v2/v3 chain 누적



### Round 280 — Split-brain fix — read/validate 7 사이트가 resolve_sidecar 통과 (mutate 와 일관). validate-workspace / query / validate-code-refs 가 [atomic].sidecar_path 따른다. 555 tests / 0 failure.

**Changes**:
- atomic_cli.rs: resolve_sidecar + resolve_output 둘 다 pub 으로 노출
- main.rs 7 사이트 모두 default_sidecar_path → resolve_sidecar(&root, None) 통일
- cmd_validate / recurse_affected_docs / cmd_validate_workspace / cmd_query / cmd_append_changelog_entry / cmd_style_check / cmd_validate_code_refs
- validate_atomic_store 본체도 resolve_sidecar — staleness 검사가 config-aware sidecar 본다
- split-brain 해소: write 와 read/validate 가 같은 store 본다



**Verification**:
- cargo test --workspace 555 passed / 0 failed (R279 552 + 3 신규)
- 신규 r280_atomic_path_config_smoke.rs: validate-workspace / query / validate-code-refs 모두 sidecar override 따름
- reporter repro 통과: sidecar_path=doc/.atomic/store.json 설정 후 mutate → validate sections=1 sync (이전 0 stale)
- self-application 회귀 영향 없음 — entries=26 sections=5 sync, default 경로 사용자는 fix 효과 보지 못함



**Impact**: §atomic-store-mutate-api, §code-citation-defense


**Carry forward**:
- TC8 외 일반 어댑터 split-brain blocker 해제 — [atomic] 두 knob 완전한 read/write 대칭
- default_sidecar_path / default_output_path API 자체는 유지 — 단순 wrapper 로 backward compat
- resolve_sidecar 가 discover_config 호출 — config 미존재시 default fall back, 영향 없음
- atomic_cli::resolve_sidecar pub 노출 — paths.rs 별도 module 분리는 carry (현재 1 caller 만 사용)
- README/SCHEMA_GUIDE 에 [atomic] 섹션 surface — 별도 docs round carry
- multi-token external prefix (ETSI TS) / ScanOptions struct 리팩터 carry 유지
- Phase 0 carry held — R271/R270+/R268/R267



### Round 281 — Bug #5A fix — external prefix verbatim 비교 전 surrounding punctuation strip. (RFC 791 §3.1) / [RFC 793] / "RFC 2131" 모두 skip 통과. tc8-harness Phase E 의 275 RFC FP 잔여 중 5A subset 해소. 560 tests / 0 failure.

**Changes**:
- is_external_section_cite: prev_token 의 leading non-alphanumeric strip 후 prefixes 와 verbatim 비교
- (RFC 791 §3.1) / [RFC 793 §3.9] / "RFC 2131 §3.4" / «RFC ...» 형태 모두 skip 통과
- bare RFC NNN §X (R277 form) 회귀 영향 없음 — 같은 trim_start_matches 가 변경 없는 case 통과
- 5B (multi-line continuation) / 5C (literal RFC 누락) 는 R281 미포함 — style guidance + Phase 1B carry



**Verification**:
- cargo test --workspace 560 passed / 0 failed (R280 555 + 5 신규)
- code_refs::tests 신규: paren/bracket/quote prefixed + bare 회귀 + unit punctuation strip
- self-application 영향 없음 — Mnemosyne 코멘트는 (RFC ...) form 없음, validate-workspace 동일
- tc8-harness Phase E baseline 275 RFC FP 의 5A subset 해소 예상 — 잔여 5B/5C 는 별 트랙



**Impact**: §code-citation-defense


**Carry forward**:
- Bug #5B (multi-line continuation) — README 의 style guidance 추가 권장 (canonical RFC NNN §X.Y inline)
- Bug #5C (literal RFC 누락) — 5B 변종, code-style 문제, R281 미해소 carry
- Multi-token external prefixes (TR_SOMEIP / AUTOSAR_SWS / ETSI TS) — Phase 1B carry
- trailing punctuation handling (RFC 791) — leading strip 만 함, trailing 은 numeric 검사가 reject
- tc8-harness Phase F (severity reject 승격) 진입 — R281 적용 후 baseline 재측정 권장
- ScanOptions struct 리팩터 carry / Phase 0 carry (R271/R270+/R268/R267) 유지



### Round 282 — SCHEMA_GUIDE 갱신 — 5 primitives + R279/280/281 config knobs (atomic / inventory / external) + Bug #5B/#5C self-contained citation 가이드 + orphan_ledger kind=code_citation 사용 예시. docs-only round, 560 tests / 0 failure 유지.

**Changes**:
- SCHEMA_GUIDE top description: 4 → 5 primitives (InventoryEntry added Phase 1A)
- Schema schema TOML example: [atomic] sidecar_path/output_path + inventory_prefixes + external_section_prefixes + severity_inventory + orphan_ledger kind=code_citation
- 신규 섹션 — Field length caps (intent 200 / bullets 100 char) DX surface
- 신규 섹션 — Self-contained citation rule: scanner 가 prose AI 아님, multi-line/literal-누락 은 carry, orphan_ledger kind=code_citation 사용 예시
- Common authoring patterns 확장: Inventory citation defense (TC8 dogfood 예시) + External adopter directory-layout (R279/R280 fix 반영)
- "What stays fixed" 갱신: 5 entities 확정, Phase 1A 진입 명시



**Verification**:
- cargo test --workspace 560 passed / 0 failed (validator/cli 변경 없음, docs-only round)
- validate-workspace entries=28 sections=5 sync — SCHEMA_GUIDE 변경이 round-trip 영향 없음
- validate-code-refs total=0 — SCHEMA_GUIDE 의 Round 280/281 인용이 R280/R281 entries 매칭
- 외부 어댑터 가이드 cover: 5 primitives + 4 신규 config knobs + Bug #5B/#5C 흡수 메커니즘



**Impact**: §code-citation-defense, §atomic-store-mutate-api


**Carry forward**:
- GETTING_STARTED.md 갱신 — InventoryEntry 도입 mention + Phase 1A 진입 표시, 별도 docs round carry
- README 의 surface 갱신 (5 primitives) 도 별도 docs carry
- Bug #5B/#5C 의 self-contained citation 가이드가 SCHEMA_GUIDE 에 명시 — tc8-harness 측 잔여 cleanup 방향 결정 가능
- Multi-token external prefixes (ETSI TS, TR_SOMEIP) Phase 1B carry — 가이드에 v1 한계 명시
- ScanOptions struct 리팩터 carry / Phase 0 carry (R271/R270+/R268/R267) 유지



### Round 283 — remove_section_implementation primitive — Section.implementations set-element granular remove. R259 add-only 의 missing piece closure. NotFound + (file, symbol) exact + --reason 필수. CLI/MCP 동시 wire. 565 tests / 0 failure.

**Changes**:
- atomic.rs: remove_section_implementation primitive — exact (file, symbol) match, --reason 필수
- NotFound on absent section_id 또는 absent (file, symbol) tuple — silent no-op 없음
- Symbol-aware: (file, None) vs (file, Some("sym")) 별 row, 정확 매칭만 제거
- lib.rs re-export + atomic_cli.rs cmd_remove_section_implementation handler
- main.rs dispatch arm + usage line + help text (Round 283 표기)
- mcp/main.rs: remove_section_implementation MCP tool + RemoveSectionImplementationArgs



**Verification**:
- cargo test --workspace 565 passed / 0 failed (R282 560 + 5 신규)
- atomic::tests 신규 5: basic_round_trip / symbol_aware / section_not_found / impl_not_found / empty_reason
- CLI smoke: add §X impl 2개 → remove (file,symbol) specific → remove ghost 정확 NotFound 에러 → final 1개 row
- MCP build pass, RemoveSectionImplementationArgs schema 정상
- self-application 영향 없음 — Mnemosyne 자체 binding 8개 변동 없음



**Impact**: §atomic-store-mutate-api, §code-citation-defense


**Carry forward**:
- R284 carry: add + remove implementation 양쪽 cascade trigger (R266/R276 패턴) + R268 print_atomic_decay_surface unification 자연 closure
- bulk-replace primitive (set_section_implementations) — script-friendly batch friction 측정 후 carry (Q3 결정)
- atomic_cli.rs handler 14 → 15 → 향후 macro/builder 리팩터 carry
- 어댑터 잔여 cleanup tooling: bulk-register-orphan-ledger CLI — empirical bite 후 carry
- TC8 어댑션 측: R283 install 후 65 impl_unbacked 정리 가능 (typed primitive 경로 확보)
- Phase 0 carry held — R271/R270+/R268/R267 / multi-token external prefix / ScanOptions



### Round 284 — Bug #7 fix — external_section_prefixes_bare 신설 (옵션 D namespace 분리). AUTOSAR family (<PREFIX> §<id>) doc-name 모드. numeric/bare 두 axis 독립, opt-in 명시. extract_v3 + scan_v4 wrapper chain. 573 tests / 0 failure.

**Changes**:
- config.rs: [code_refs].external_section_prefixes_bare Vec<String> default empty
- is_external_section_cite: mode 1 (numeric, R277) + mode 2 (bare, R284) mutually exclusive via last_token shape
- mode 2: <PREFIX> §<id> form (TR_SOMEIP / SOMEIPSD / SWS_SD / AUTOSAR family)
- R281 leading-punct strip 가 양 mode 모두 적용 — (TR_SOMEIP §X.Y) 자연 skip
- extract_section_citations_v3 신설 + v2 가 v3(empty bare) 위임 (R281 caller 무영향)
- scan_paths_bidirectional_v4 신설 + v3 가 v4(empty bare) 위임
- main.rs: cmd_validate_code_refs 가 v4 호출 + JSON/text 출력 양 axis surface
- SCHEMA_GUIDE: 새 섹션 "External standard prefix kinds" + AUTOSAR generic-token risk 경고



**Verification**:
- cargo test --workspace 573 passed / 0 failed (R283 565 + 8 신규 R284)
- code_refs::tests 신규: bare TR_SOMEIP / SOMEIPSD / paren-wrap / negative / numeric+bare 독립 / numeric 회귀 / unit punct strip / scan_v4 통합
- CLI smoke: RFC numeric / TR_SOMEIP bare / SOMEIPSD bare → all skip; (AUTOSAR §) 미등록 정상 fire
- self-application 영향 없음 — Mnemosyne 자체 mnemosyne.toml 변경 없음 (entries=30 동일)
- API surface: v3 wrapper 보존 (R277/R281 callers 무영향), v4 = 본체



**Impact**: §code-citation-defense


**Carry forward**:
- TC8 어댑션 Phase E 의 38 AUTOSAR-family FP 예상 해소 (TR_SOMEIP / SOMEIPSD / SWS_SD bare 등록)
- 옵션 D 채택 정당화: namespace 분리로 generic-token risk 가시화, future-extensibility, axis 일관성
- v1/v2/v3/v4 wrapper chain 길어짐 — Phase 1B ScanOptions struct refactor carry 강화
- Multi-token prefix (ETSI TS, IETF draft-id) — namespace 추가 패턴 사용, empirical bite 후 carry
- Phase F (severity_missing reject 승격) 진입 가능 — Bug #7 마지막 P1 closure
- R284 jaccard: extract_section_citations_v3 + scan_paths_v4 두 신규 fn — symmetric chain



### Round 285 — Bug #8 fix — inventory-axis orphan_ledger suppression. OrphanKind::InventoryCitation variant 추가 + scan_v4 inventory loop suppression branch. §-axis CodeCitation 와 axis-isolated, suppression-only (set-equality drift detection R286+ carry, axis 대칭). R275 carry closure. 577 tests / 0 failure.

**Changes**:
- config.rs: OrphanKind::InventoryCitation variant 추가 (5번째, snake_case "inventory_citation")
- code_refs.rs: inventory_ledger_index 별도 빌드 — CodeCitation/InventoryCitation axis-isolated
- scan_paths_bidirectional_v4 inventory loop: InventoryDeprecated + InventoryMissing 둘 다 suppress
- v4 시그니처 변경 없음 — ledger 인자 안에서 kind filter 만 분리 (v5 wrapper 도입 불필요)
- main.rs validate_atomic_store match arm: InventoryCitation 도 suppression-only (§-axis 와 일관)
- doc convention sentinel: "<inventory-citation>" (CodeCitation의 "<code-citation>" mirror)
- SCHEMA_GUIDE: orphan_ledger field 설명 + Self-contained citation rule 섹션 inventory example 추가



**Verification**:
- cargo test --workspace 577 passed / 0 failed (R284 573 + 4 신규)
- code_refs::tests 신규 4: deprecated_suppress / missing_suppress / unregistered_fires / axis_filter
- axis_filter test: CodeCitation row 가 inventory cite suppress 안 함 (axis 독립 검증)
- self-application 영향 없음 — Mnemosyne 자체 inventory_prefixes 미설정, validate-workspace entries=31 동일



**Impact**: §code-citation-defense


**Carry forward**:
- TC8 어댑션 잔여 2 inventory_deprecated (IPv4_OPTIONS_01) — 본 라운드로 typed surface 확보, ledger 등록 가능
- TC8 expected post-fix: baseline 0 (2877→0, 100% closure), Phase F (severity_inventory=reject) 진입 가능
- R286+ carry: §-axis CodeCitation + inventory-axis InventoryCitation 양 axis 같이 set-equality drift detection 도입 — empirical bite (ledger row 의 referent 해소 후 row 남는 케이스) 발생 시
- R260 부터 §-axis suppression-only 가 ~25 rounds 동안 drift bite 미발생 → low-priority gap, YAGNI carry
- Multi-token external prefix (ETSI TS) / ScanOptions struct refactor / Phase 0 carry 유지



### Round 286 — Universal CLI --version / -V / version surface. build.rs git hash 임베드 (rustc/cargo format mirror). mnemosyne-cli + mnemosyne-mcp 양쪽. watching-zenoh 진단 시 발생한 mtime + strings|grep 우회 폐기. clap migration trigger-bound R287+ carry. 582 tests / 0 failure.

**Changes**:
- crates/mnemosyne-cli/build.rs 신설 — git describe --always --dirty --abbrev=8 호출, BUILD_GIT_HASH env 임베드
- crates/mnemosyne-mcp/build.rs 신설 — 같은 패턴 mirror
- mnemosyne-cli/src/main.rs: --version / -V / version arm + print_help 첫 줄 + meta 섹션 추가
- mnemosyne-mcp/src/main.rs: parse_workspace_arg 에 --version / -V arm + --help 출력에 version 포함
- 출력 format: rustc/cargo 패턴 mirror — "mnemosyne-cli 0.1.0 (a4f00a49-dirty)"
- fallback: git 미가용 시 "unknown" (tarball install / no .git 케이스)
- 5 integration tests (long flag / short flag / subcmd / help first line / 3 forms identical)



**Verification**:
- cargo test --workspace 582 passed / 0 failed (R285 577 + 5 신규 R286)
- ./target/release/mnemosyne-cli --version → "mnemosyne-cli 0.1.0 (a4f00a49-dirty)" 정상
- ./target/release/mnemosyne-mcp --version → "mnemosyne-mcp 0.1.0 (a4f00a49-dirty)" 정상
- --help 첫 줄에도 version 노출 (single call 로 binary 식별 가능)
- dirty marker 동작 — uncommitted changes 있는 빌드 즉시 인식
- self-application 영향 없음 — validate-workspace entries=32 동일



**Impact**: §code-citation-defense


**Carry forward**:
- R287+ clap migration trigger-bound carry: 다음 중 하나 발생 시 진입
- (a) dispatch arm count > 15 (현재 14+ 임계 근접)
- (b) arg-parsing helper 가 3 번째 caller 등장
- (c) build.rs/main.rs boilerplate 가 두 crate 간 3 줄 이상 중복
- YAGNI deferred 가 아닌 empirical-bite-bound carry — Mnemosyne 의 R268/R278 패턴 일관
- 외부 어댑터 (watching-zenoh / tc8-harness) 의 binary 식별 friction 해소 (mtime + strings|grep 우회 폐기)
- 다른 carry 유지: §-axis + inventory-axis set-equality drift detection / ScanOptions struct refactor / multi-token external prefix / Phase 0 carry (R271/R270+/R268/R267)



### Round 287 — AtomicSection outline lift — schema-lift Phase A-D, Round 164+ title-from-workspace-pending carry closure, fail-loud section_mut() refactor with add_section as sole creation path

**Changes**:
- AtomicSection += title / parent_doc / parent_section (3 outline fields, serde-default for v2 back-compat, mirrors schema.rs::Section closed-form)
- schema_version 2 → 3 bump + v2 → v3 load test (back-compat verified)
- atomic add_section primitive (atomic.rs::add_section) pairs with remove_section (R267); section_id duplicate reject + parent_section existence check
- set_section_title / set_section_parent_doc / set_section_parent_section outline setters; self-loop reject on parent_section
- AtomicStore::section_mut() refactored to Option<&mut> (fail-loud); silent or_default() create-on-miss footgun closed
- section_mut_strict helper (atomic.rs free fn); all set_section_* / add_section_* primitives require existing Section (NotFound on missing); add_section is sole creation path
- Test fixtures explicit-seed: atomic.rs 16 tests + validator.rs rule4 4 tests + code_refs.rs 2 helpers + atomic_round_trip 3 integration + cascade_auto_update_smoke 1 + r280_atomic_path_config_smoke 2



**Verification**:
- cargo test --release --workspace: 601 passed / 0 failed (Phase A-D combined)
- Round 269 Option<DecisionStatus> contract preserved — audit distinction explicit-override vs parser-default kept
- schema_version_2_store_loads_with_empty_outline_fields test green (v2 → v3 back-compat)
- Round 286 baseline maintained — docs/.atomic/workspace.atomic.json schema_version=2 stores load cleanly + rewrite to v3 on next save
- Round 164+ atomic-only--title-from-workspace-pending sentinel sections inventoried (5 entries) for Phase I backfill scope



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- Phase E — query.rs ATOMIC_ONLY_PARENT_DOC sentinel + intent→title fallback 제거 (atomic outline 필드 직접 사용)
- Phase F-G — CLI add-section 내부 atomic 교체 + MCP add_section tool 신규 등록 (watching-zenoh outline carry unblock 경로)
- Phase H — legacy mutate.rs::add_section + find_section_end_position / find_changelog_or_eof_position markdown-surgical helpers 일괄 삭제
- Phase I — 기존 atomic store 205 sections backfill migration (5 title-from-workspace-pending sentinel section 실제 outline 채우기)
- Phase J — validate-workspace 전체 통과 + GENERATED.md round-trip (docs 11/11, T1 orphan=0, T3 reject=0 baseline)
- Phase 287+ — AtomicSection.decision_status Option<DecisionStatus> → non-Option 검토 (Round 269 contract 재평가 후 별도 round 결정)



### Round 288 — Phase E+I — Section-axis sentinel removal + 5 sentinel section outline backfill + outline setter CLI surface

**Changes**:
- query.rs synthetic_section uses atomic.title/parent_doc/parent_section directly (ATOMIC_ONLY_PARENT_DOC sentinel + intent→title fallback retired on Section axis)
- generate-docs renders Sections via render_section with real title (placeholder header "atomic-only — title from workspace pending" retired); demoted ## → ### to fit under doc-level outline
- CLI dispatch added: set-section-title / set-section-parent-doc / set-section-parent-section (Round 287 Phase C primitives now surfaced)
- 5 sentinel section outline backfilled via mutate API: atomic-store-mutate-api / code-citation-defense / code-citation-defense/bidirectional-binding (parent=code-citation-defense) / markdown-parser / orphan-ledger — all bound to docs/GENERATED.md
- ATOMIC_ONLY_PARENT_DOC constant retained — changelog axis still uses it (ChangelogEntries are workspace-level, not doc-bound; sentinel semantically correct there)



**Verification**:
- cargo test --release --workspace: all suites pass (312 validator + 22 CLI integration + others)
- validate-workspace baseline maintained: docs=1/1, T1 orphan=0, round-trip=1/1, T3 reject=0, GENERATED.md=sync
- generate-docs output verified: 5 sections render with real titles (Atomic Store Mutate API / Code Citation Defense / Bidirectional Binding / Markdown Parser / Orphan Ledger)
- section_by_id_atomic_only_section_surface test updated — asserts real outline values (no sentinel)



**Impact**: §atomic-store-mutate-api, §code-citation-defense, §code-citation-defense/bidirectional-binding, §markdown-parser, §orphan-ledger


**Carry forward**:
- Phase F — legacy CLI add-section dispatch route through atomic add_section primitive (markdown-surgical insert retirement)
- Phase G — MCP add_section + set_section_title / set_section_parent_doc / set_section_parent_section tools (watching-zenoh outline carry unblock)
- Phase H — legacy mutate.rs::add_section + find_section_end_position / find_changelog_or_eof_position markdown-surgical helpers delete
- ATOMIC_ONLY_PARENT_DOC sentinel — changelog axis keep (entries not doc-bound); revisit if AtomicChangelogEntry gains parent_doc field
- render_section template — top-level §slug. Title format reads oddly for slug-id sections (numeric-id assumption); cosmetic polish carry



### Round 289 — Phase F+G+H — CLI add-section atomic surface + MCP outline tool registration + legacy mutate.rs::add_section retirement

**Changes**:
- CLI add-section dispatch routes through atomic add_section primitive (atomic.rs); legacy markdown-surgical add_section (mutate.rs:543-714) deleted
- CLI add-section flag surface simplified: --section / --parent-doc / --title / --parent (legacy --doc / --numbered-id / --body-file retired — atomic mode has no monolithic body)
- mutate.rs helpers deleted: section_depth + find_changelog_or_eof_position (only used by retired add_section); add_cross_ref / set_section_body keep find_section_end_position + find_section_body_range
- lib.rs re-export cleaned: mutate::add_section removed (atomic::add_section is canonical via module path)
- MCP tools registered (Phase G): add_section + set_section_title + set_section_parent_doc + set_section_parent_section — watching-zenoh outline carry now unblocked
- Legacy tests deleted: 3 add_section_* cases in remaining_mutate_primitives_smoke.rs (atomic.rs unit suite has 8-test atomic add_section coverage)



**Verification**:
- cargo test --release --workspace: 598 passed / 0 failed (601 - 3 legacy add_section tests deleted = 598 expected)
- validate-workspace baseline maintained: docs=1/1, T1 orphan=0, round-trip=1/1, T3 reject=0, GENERATED.md=sync
- mnemosyne-mcp builds clean with 4 new tools registered
- Phase A-D + E+I + F+G+H all land — Round 287 outline-lift carry closure at 9/10 phases (Phase J = ongoing pre-commit verify-generated gate)



**Impact**: §atomic-store-mutate-api, §code-citation-defense


**Carry forward**:
- Round 269 Option<DecisionStatus> tightening — deferred decision; revisit after Phase A-J observation period
- ChangelogEntry axis ATOMIC_ONLY_PARENT_DOC sentinel — kept (entries not doc-bound); revisit if AtomicChangelogEntry gains parent_doc field
- render_section §slug. Title format reads oddly for slug-id sections — cosmetic polish carry
- mutate.rs add_cross_ref / set_section_body / set_section_decision_status — still markdown-surgical; atomic equivalents not yet designed
- AtomicChangelogEntry outline lift (mirror to AtomicSection) — workspace-wide changelog semantics need separate decision before scoping



### Round 290 — terminology_consistency mechanical-citation scope fix — Section.implementations file paths excluded from prose-rule body via synthesize_section_prose_body — synthesize_section_prose_body excludes Section.implementations file-path block from style-rule body; query.rs full-body path unchanged

**Changes**:
- atomic.rs: synthesize_section_prose_body added (skips Section.implementations file-path block)
- atomic.rs: synthesize_section_body_inner DRY helper; public synthesize_section_body unchanged
- style.rs: resolve_section_body switches to prose variant for terminology + length rules
- query.rs unchanged: SectionView.body keeps full body including implementations for consumers
- tests: terminology_ignores_implementation_paths + still_fires_on_prose_variants (regressions)



**Verification**:
- cargo test --workspace: all groups pass / 0 failed (validator lib +2 new terminology tests)
- validate-workspace self-application: T3 reject=0, round-trip=1/1, atomic ledger sync
- Bug repro: TC8/DUT/SOME-IP glossary + lowercase paths in implementations now yields 0 violations
- Companion test: lowercase variant in intent prose still fires terminology_consistency (no over-fix)



**Impact**: §code-citation-defense, §atomic-store-mutate-api


**Carry forward**:
- impact_scope §section-id block still in prose body; slug shape rarely substring-matches glossary
- examples fenced-code block kept; comments inside code can legitimately need terminology flags
- Legacy fallback parsed.bodies (non-atomic sections) — Implementations block lives in markdown body
- Word-boundary tightening of terminology matcher — broader fix deferred; surgical exclusion suffices
- mnemosyne.toml terminology.exempt_patterns config knob — not added; principle = exclude mechanical



### Round 291 — Section.atomic_section_id field + AtomicStore::resolve(&Section) bridge — nested ### §<id> headings find atomic counterpart instead of falling back to raw markdown — closes R290 terminology_consistency false-negative + recovers SectionView body / decision_status override — backfill entry appended retroactively in R293 — Section.atomic_section_id captures heading §<token> verbatim and AtomicStore::resolve(&Section) prefers it over the parser-derived slug, so nested ### §<id> headings under ## Sections find their atomic counterpart. Closes R290 terminology_consistency false-negative on impl paths; recovers SectionView body and decision_status overrides that were silently falling back to raw markdown. (R291 commit 76581f6 — backfill entry appended retroactively in R293.)

**Changes**:
- schema.rs Section adds atomic_section_id: Option<String> populated from heading §<token> verbatim, separate from parser-derived section_id slug
- parser.rs heading parser captures § token before slug derivation so nested ### §<id> headings under ## Sections retain the atomic key shape
- atomic.rs AtomicStore::resolve(&Section) prefers atomic_section_id when present, falls back to parser slug for legacy headings without § token
- query.rs / style.rs / validator.rs / mutate.rs / workspace.rs threaded through the new resolve() path so SectionView body and decision_status overrides land instead of silently bypassing the atomic side
- render → real-parse → atomic-lookup roundtrip regression test added in style.rs covering production GENERATED.md heading shape
- existing R290 same-key bypass test annotated to mark the lookup miss it had been masking before this fix



**Verification**:
- cargo test --release --workspace at commit 76581f6: green (598-tier baseline pre-R292)
- validate-workspace at commit 76581f6: docs=1/1, T1=0, round-trip=1/1, T3 reject=0
- R290 false-negative case (terminology_consistency on Section.implementations path lines) re-exercised: now reads impl-path-stripped body via fixed resolve() path
- new regression test render → real-parse → atomic-lookup roundtrip passes; nested § header round-trip lookup hits atomic store instead of falling back to raw markdown



**Impact**: §atomic-store-mutate-api, §markdown-parser


**Carry forward**:
- This entry appended retroactively in Round 293 (atomic-store key gap closed); original commit 76581f6 was authored Fri 2026-05-15 between R290 (72332cc) and R292 (50f5f2f) without an atomic-store ledger entry, leaving an audit-trail hole until R293 backfill
- Mutate API hardening carry: append-changelog-entry-v2 silently accepted an empty entry body during R293 backfill exploration (entry-id only, no decision/changes/verification/carry args) — separate harden-pass needed to reject null fields at primitive boundary



### Round 292 — query_term read primitive — literal/regex search across atomic Section + ChangelogEntry + Inventory fields; replaces external grep, P1 redact_term preview substrate.

**Changes**:
- query.rs adds query_term() + TermQuery/Mode/Scope/TargetKind/Hit + QueryTermError types
- regex crate added to workspace deps; Literal + Regex modes with case_insensitive toggle on both
- Section scan covers: title, intent, 5 bullet lists, alternatives, examples, implementations
- ChangelogEntry scan: decision_summary + 4 bullet lists (changes/verification/impact_refs/carry)
- Inventory scan: source + reason text fields (Phase 1A axis)
- CLI: mnemosyne-cli query --term <pat> [--regex] [-i] [--scope ...] [--field ...] [--json]
- MCP: query_term tool (read-only) registered; argv delegates to CLI subprocess pattern
- 16 unit tests in query.rs::tests cover literal/regex/case/scope/field/struct-subfield paths



**Verification**:
- cargo test --release --workspace: 617 passed / 0 failed / 47 ignored (Round 289 = 598; +19)
- validate-workspace baseline: docs=1/1, T1=0, round-trip=1/1, T3 reject=0, GENERATED.md=sync
- CLI smoke: --term frozen --scope changelog returns 5 hits across 3 entries
- CLI smoke: --term "Round [0-9]{3}" --regex matches decision_summary fields
- MCP server: cargo build -p mnemosyne-mcp clean; query_term tool registered



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- P1 redact_term mutate primitive — paired with P2 frozen_override_ledger schema (next round)
- P2 frozen_override_ledger config (kind enum + content_hash anchor + T2 ledger-skip integration) — next round
- Legacy parser-side ChangelogEntry.sub_bullets scan deferred — v1 atomic-only scope
- P4 ChangelogEntry body split deferred — re-evaluate after P1+P2 usage data collected
- R291 (commit 76581f6 fix(validator): bridge parser↔atomic section_id key mismatch) atomic-store entry remains unappended — separate carry to close



### Round 293 — R291 backfill entry append + commit↔ledger drift gate (validate-workspace) — audit-trail hole between R290 and R292 closed; warn-only drift surface line wired — R291 atomic-store entry retroactively appended (commit 76581f6 — parser↔atomic section_id key mismatch fix), closing the audit-trail hole between R290 and R292. validator/commit_ledger.rs new module provides a pure BTreeSet<u32> diff (cited / ledger / missing / extra) with no IO or git dep. validate-workspace now prints a commit↔ledger drift surface line at end of run by walking the last 200 git commit subjects, extracting (R<N>) / (Round <N>) project-convention labels, and diffing against ledger entry-id round_numbers. v1 = warn-only (informational, never bails); promotion to hard reject deferred until policy stabilizes.

**Changes**:
- Round 291 atomic-store entry retroactively appended (commit 76581f6 closure — parser↔atomic section_id key mismatch fix); audit-trail hole between R290 and R292 closed
- crates/mnemosyne-validator/src/commit_ledger.rs new module — pure BTreeSet<u32> diff returning CommitLedgerDriftReport { cited_count, ledger_count, missing, extra } with no IO or git dep
- CLI validate-workspace gains commit↔ledger drift surface line at end of run — git log --max-count=200 --pretty=%s subjects parsed for "(R<N>)" / "(Round <N>)" project commit-convention labels and diffed against atomic ledger entry-id round_numbers
- mnemosyne-cli adds regex.workspace = true (workspace dep already present from R292) for the small fixed pattern set covering both label forms
- validator::commit_ledger_diff and CommitLedgerDriftReport exported via lib.rs re-export so future axes (CI, pre-commit, alternative VCS frontends) can reuse the pure diff
- 5 unit tests in commit_ledger.rs::tests cover clean / R291-hole simulation / empty inputs / count fields / ascending sort of missing+extra



**Verification**:
- cargo test --release --workspace: 622 passed / 0 failed / 47 ignored (R292 = 617; +5 from commit_ledger tests)
- validate-workspace baseline: docs=1/1, T1=0, round-trip=1/1, T3 reject=0, GENERATED.md=sync, atomic ledger entries=39 (was 38; R291 backfill +1)
- drift gate live output line: "commit↔ledger drift: cited=26 / ledger=39 / missing=0 (last 200 commits scanned)" — confirms R291 backfill closes the hole
- unit test missing_round_surfaces_when_cited_but_absent reproduces the R291 hole shape and confirms missing list contains the absent round
- existing R292 query_term smoke checks remain green (no regression on read-primitive surface)



**Impact**: §atomic-store-mutate-api, §markdown-parser


**Carry forward**:
- Drift gate severity in v1 = warn-only (informational line, never bails) — promote missing > 0 to a hard reject under a separate axis after policy stabilizes and any legitimate exception classes (e.g. squash-merge artifacts, retroactive backfills) are catalogued
- Cited-pattern set (R<N> / Round <N> in parens) covers project commit convention only — broader free-form mentions in commit body, PR titles, or other surfaces remain out of scope (subject-line scan only); widen if a real false-negative emerges
- Scan window fixed at 200 commits — sufficient for active-window drift catch on the current cadence, but does not retroactively scan deep history; long-tail backfills (commit-only round labels older than the window) need a one-off audit pass before any future window-shrink
- Mutate API hardening carry: append-changelog-entry-v2 silently accepts entry-id-only invocations with empty body (decision/changes/verification/carry args missing) — surfaced during R293 backfill exploration; separate harden-pass needed to reject null fields at the primitive boundary
- P2 frozen_override_ledger config + T2 ledger-skip integration (carry from R292) — next-round substrate for P1
- P1 redact_term mutate primitive (carry from R292) — paired with P2 schema
- Legacy parser-side ChangelogEntry.sub_bullets scan deferred (carry from R292) — v1 atomic-only scope
- P4 ChangelogEntry body split deferred (carry from R292) — re-evaluate after P1+P2 usage data collected



### Round 294 — AtomicChangelogEntry schema split (publishable vs audit body) — schema_version 4 + v3→v4 loader migration + render switch to publishable view + T2 audit-only scope made explicit; R293 catch-up folded as carry — AtomicChangelogEntry gains 5 publishable_* fields paralleling the audit fields (decision_summary, changes_bullets, verification_bullets, impact_refs, carry_forward_bullets); CURRENT_SCHEMA_VERSION bumped 3→4 with a v3→v4 loader migration that clones audit_* into publishable_* per entry. append_changelog_entry default = audit clone (signature unchanged) so newly authored entries are publishable_matches_audit() == true at append time. render_changelog_entry switches to read publishable_* — generate_docs is now the publishable view layer; the audit half stays as the permanent record inside the atomic store. T2 jaccard scope made explicit (audit-only) via comment. Establishes the structural prerequisite for R295 publishable setters and R296 [[publishable_override_ledger]]; closes RFC G4 (body split) ahead of RFC's defer recommendation because schema-evolve-once is cheaper than schema-evolve-twice.

**Changes**:
- AtomicChangelogEntry schema split — 5 publishable_* fields paralleling audit fields (publishable_decision_summary + publishable_changes_bullets + publishable_verification_bullets + publishable_impact_refs + publishable_carry_forward_bullets); audit fields keep names unchanged so existing v3 JSON loads with serde defaults
- clone_audit_into_publishable() and publishable_matches_audit() helper methods on AtomicChangelogEntry — production migration path and R296 ledger-gate substrate
- CURRENT_SCHEMA_VERSION bumped to 4; AtomicStore::load runs v3→v4 migration by cloning audit_* into publishable_* per entry when schema_version < 4 (byte-identical render preserved); v4 stores keep the two halves independent so intentional divergence (redaction, typo fix) survives reload
- append_changelog_entry default = audit clone (signature unchanged so all existing callers stay correct); newly authored entries are publishable_matches_audit() == true at append time
- render_changelog_entry switched to read publishable_* fields — generate_docs surface is the publishable view layer; audit_* half stays as the permanent record inside the atomic store
- T2 jaccard (frozen_ledger_atomic + check_atomic_entry) audit-only scope made explicit via comment — publishable_* changes deliberately bypass T2 (will gain their own gate via [[publishable_override_ledger]] in R296)
- 9 struct-literal fixture sites in query.rs / t2.rs updated with ..Default::default() spread so they continue to compile after schema additions
- 4 new unit tests in atomic.rs::tests cover v3→v4 migration cloning, v4 divergence preservation, append_changelog_entry audit-clone default, and clone_audit_into_publishable idempotency
- 2 new unit tests in render.rs::tests cover publishable-render path and publishable / audit divergence (audit half does not leak into rendered output)
- R293 catch-up: R293 entry body recorded 622 tests (pre-prefix-normalize fix snapshot); current baseline = 629 tests (R293 = 625 + 4 R294 schema-split tests). Frozen-ledger constraint prevented in-place R293 entry update; R294 carry surfaces this as a pattern to revisit when [[publishable_override_ledger]] (R296) lets retroactive publishable updates land without breaking the audit invariant



**Verification**:
- cargo test --release --workspace: 629 passed / 0 failed / 47 ignored (R293 = 625; +4 from atomic schema-split tests + 2 from render publishable-path tests, partially offset by 2 migration test version-bump edits = net +4)
- validate-workspace baseline: docs=1/1, T1=0, round-trip=1/1, T3 reject=0, GENERATED.md=sync, atomic ledger entries=40
- generate-docs round-trip: written_bytes=106551 byte-identical to pre-R294 baseline — confirms publishable_* defaults clone audit_* preserves render shape
- git diff docs/: empty after generate-docs — no GENERATED.md drift on the v3→v4 migration path
- commit↔ledger drift gate: cited=27 / ledger=40 / missing=0 — R293 drift surface still green
- schema_version_3_clones_audit_into_publishable_on_load test: v3 JSON loads, publishable_* matches audit_* per entry, save bumps to v4
- schema_version_4_preserves_publishable_divergence_on_load test: v4 JSON with intentionally diverged publishable_* round-trips without overwrite — invariant for R295 setters and R296 ledger-gate
- render_changelog_entry_publishable_diverges_from_audit test: when publishable_* != audit_*, render emits publishable view; audit half does not leak into GENERATED.md



**Impact**: §atomic-store-mutate-api, §markdown-parser


**Carry forward**:
- R295 carry — publishable mutate primitives: set_changelog_publishable_decision_summary + 4 setters paralleling the publishable_* fields; atomic transaction; pre-write check rejects audit_* mutation attempts (audit invariant enforcement at primitive boundary)
- R296 carry — [[publishable_override_ledger]] config: mnemosyne.toml schema mirroring [[orphan_ledger]] (kind, target_id, fields, reason, applied_in, content_hash_before, content_hash_after); validate-workspace gate that rejects publishable_* != audit_* divergences without a matching ledger entry; cascade emits ledger draft on publishable mutate
- R297 carry — redact_term convenience primitive (RFC P1 variant): pattern + replacement + scope + dry_run + reason; routes only to publishable_* (audit_* is system-immutable after R295 setters land); preview substrate via R292 query_term
- R293 entry body shift carry-over: R293 entry recorded 622 / drift gate v1 only; R294 lands the prefix-normalize aftermath as part of test-count delta; full frozen-ledger-aware retroactive update path waits on R296 [[publishable_override_ledger]]
- mutate API hardening carry (rolled forward from R293): append_changelog_entry silently accepts entry-id-only invocations with empty body — separate harden-pass needed once R295 publishable setters establish the field-required vocabulary at the primitive boundary
- RFC G3 (no internal workspace search) closed by R292 query_term — no further carry on this axis
- RFC G4 (body split) closed by R294 schema split — completes the structural prerequisite that RFC suggested deferring; R295/R296 wire the mutate-side and gate-side
- Drift gate severity promotion (R293 carry) — warn-only → reject decision still pending policy review



### Round 295 — publishable-half setters for ChangelogEntry — 5 primitives + CLI subcommands; audit invariant enforced at primitive boundary — 5 publishable setter primitives in atomic.rs (set_changelog_publishable_decision_summary + 4 bullet variants) modify only publishable_* fields and leave audit_* intact. entry_mut_strict helper enforces entry-must-exist (NotFound on miss); append_changelog_entry remains the sole audit-write path. CLI subcommands wired: set-changelog-publishable-{decision-summary,changes,verification,impact-refs,carry-forward}. Sets up R296 [[publishable_override_ledger]] gate (divergent publishable_* without ledger → reject) and R297 redact_term convenience primitive (P1 variant routing through publishable_* only).

**Changes**:
- 5 publishable setter primitives in atomic.rs: set_changelog_publishable_decision_summary, set_changelog_publishable_changes_bullets, set_changelog_publishable_verification_bullets, set_changelog_publishable_impact_refs, set_changelog_publishable_carry_forward_bullets — each modifies publishable_* only and leaves the audit half intact (audit invariant enforced at primitive boundary)
- entry_mut_strict helper introduced to mirror section_mut_strict (R287 pattern); publishable setters require the entry to exist first (NotFound on miss) because they cannot author the audit half — append_changelog_entry remains the sole audit-write path
- 5 CLI subcommands wired in atomic_cli.rs and main.rs dispatcher: set-changelog-publishable-{decision-summary,changes,verification,impact-refs,carry-forward}; --entry + (--value | --bullets-file) + standard --sidecar/--json/--no-regenerate; usage string in main.rs updated
- 2 generic CLI helpers added (cmd_set_changelog_publishable_string for decision_summary, cmd_set_changelog_publishable_bullets for the 4 vec fields) so subcommand handlers stay one-line wrappers — same shape as cmd_set_section_bullets pre-R295
- 4 new unit tests in atomic.rs::tests cover: all 5 setters touch publishable_* and leave audit_* unchanged; missing entry returns NotFound; save→load round-trip preserves divergence (v4 store, no migration overwrite); per-bullet length validation reuses check_bullet_len
- lib.rs re-exports the 5 publishable setter symbols alongside the existing setter family



**Verification**:
- cargo test --release --workspace: 633 passed / 0 failed / 47 ignored (R294 = 629; +4 from publishable setter tests)
- validator build clean (no warnings) on the new helpers + setters
- CLI build clean after dispatcher + usage-string wire
- publishable_setters_modify_publishable_only test: applies all 5 setters then asserts both halves explicitly — audit untouched, publishable diverged, publishable_matches_audit() = false
- publishable_setter_rejects_missing_entry test: NotFound surfaces at the primitive boundary; entry_mut_strict cannot author entries
- publishable_setter_round_trips_through_save_load test: save then AtomicStore::load preserves divergent publishable_* (v4 path, no clone overwrite)
- publishable_setter_validates_bullet_length test: oversized bullet → AtomicMutateError::Validation, same shape as section setters



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- R296 carry — [[publishable_override_ledger]] config + validate-workspace gate: divergent publishable_* without a ledger entry must reject. mnemosyne.toml schema mirroring [[orphan_ledger]] (kind, target_id, fields, reason, applied_in, content_hash_before, content_hash_after); cascade emits a ledger draft on each publishable mutate so authors fill in the reason
- R297 carry — redact_term convenience primitive (RFC P1 variant): wraps the 5 R295 setters into a single pattern-and-replacement scan; routes only to publishable_*; uses R292 query_term as preview substrate; dry_run + scope filters
- R295 deferred — MCP wiring for the 5 setters: low-friction add via the existing tool-registration pattern, but not required for R296 / R297 closure; defer until usage data shows MCP need
- mutate API hardening carry (rolled forward from R293/R294): append_changelog_entry silently accepts entry-id-only invocations with empty body — separate harden-pass once R296 establishes the publishable / audit boundary at validate-workspace level
- Drift gate severity promotion carry (R293) — warn-only → reject decision still pending policy review



### Round 296 — [[publishable_override_ledger]] config + validate-workspace gate — divergent publishable_* requires SHA256-anchored ledger row (forge-resistant audit trace) — mnemosyne.toml gains [[publishable_override_ledger]] (kind, target_id, fields, reason, applied_in, content_hash_before optional, content_hash_after required) mirroring R254 [[orphan_ledger]]. AtomicChangelogEntry::publishable_hash_hex / audit_hash_hex compute deterministic SHA256 anchors. validate-workspace check_publishable_override_ledger walks changelog_entries; for each entry where publishable_matches_audit() == false, requires a ledger row whose target_id matches and whose content_hash_after equals the current publishable hash — bails on first un-anchored divergence. Editing publishable_* without re-anchoring the ledger row re-surfaces the rejection; pure-ledger carry passes (inert rows surface informationally so authors can prune). Closes RFC G2 (audit-trace structure for legitimate frozen overrides).

**Changes**:
- config.rs adds PublishableOverrideLedgerEntry { kind, target_id, fields, reason, applied_in, content_hash_before (optional), content_hash_after } and a publishable_override_ledger Vec on WorkspaceConfig — TOML schema mirrors [[orphan_ledger]] convention from R254
- atomic.rs adds AtomicChangelogEntry::publishable_hash_hex() and audit_hash_hex() — SHA256 over a serde_json shape that names the fields explicitly so future audit-field additions cannot silently invalidate prior content_hash anchors
- main.rs check_publishable_override_ledger walks changelog_entries, filters publishable_matches_audit() == false, and requires a ledger row whose target_id matches and whose content_hash_after equals the current publishable hash (forge-resistant by construction); inert ledger rows (target no longer divergent) are surfaced informationally so authors can prune
- validate-workspace prints one line ("publishable / audit divergence: entries=N ledger_rows=M") regardless of pass/fail, then bails on the first un-anchored divergence with the exact field needed (`content_hash_after = the printed publishable_hash`)
- gate wires after print_atomic_decay_surface and before the R293 commit↔ledger drift surface; pure-ledger carry passes (rows for entries that no longer diverge are inert, mirroring R254 orphan-axis semantics)
- lib.rs re-exports PublishableOverrideLedgerEntry alongside OrphanLedgerEntry
- 2 new unit tests in atomic.rs::tests cover hash determinism + mutation sensitivity (forge-resistance basis), and the audit/publishable hash separation when the two halves diverge



**Verification**:
- cargo test --release --workspace: 635 passed / 0 failed / 47 ignored (R295 = 633; +2 from hash anchoring tests)
- validate-workspace baseline: docs=1/1, T1=0, round-trip=1/1, T3 reject=0, GENERATED.md=sync, ledger=42; new informational line: "publishable / audit divergence: entries=0 ledger_rows=0"
- publishable_hash_deterministic_and_stable test: same publishable_* → same SHA256 (deterministic), R295 setter → different SHA256 (mutation sensitivity)
- publishable_hash_differs_from_audit_hash_when_diverged test: explicit divergence yields distinct anchors so the ledger row cannot accidentally match the audit half
- gate path verified: with no divergence in production atomic store (40 entries all publishable_matches_audit() == true), gate returns Ok(()) and validate-workspace passes; once R295 setter diverges any entry, gate prints the entry id + current publishable_hash and bails until a [[publishable_override_ledger]] row is registered



**Impact**: §atomic-store-mutate-api, §orphan-ledger


**Carry forward**:
- R297 carry — redact_term convenience primitive (RFC P1 final piece): pattern + replacement + scope (publishable_decision_summary | publishable_changes_bullets | etc.) + dry_run + reason; routes through R295 setters + auto-emits a [[publishable_override_ledger]] draft row for the divergence so authors do not hand-author hashes; uses R292 query_term as preview substrate. Closes RFC G1 once paired with this round's audit-trace
- Cascade auto-emit of [[publishable_override_ledger]] draft on each R295 setter call — ergonomic improvement that removes the manual SHA256 computation step. Defer until R297 lands the redact_term wrapper which already needs the same auto-emit path
- Per-field gate granularity carry: v1 gate matches at entry granularity (all diverged fields pass or fail together via the single content_hash_after anchor); per-field hash anchors (e.g. content_hash_after_changes_bullets) would let authors register partial divergences. Defer until usage shows entry-level granularity is too coarse
- mutate API hardening carry (rolled forward from R293/R294/R295): append_changelog_entry silently accepts entry-id-only invocations with empty body — separate harden-pass once R297 redact_term lands and the publishable boundary is exercised at scale
- Drift gate severity promotion carry (R293) — warn-only → reject decision still pending policy review
- RFC G2 (audit-trace structure for legitimate frozen overrides) closed by this round paired with R294 schema split + R295 setters; G1 (cross-store term replacement primitive) closes when R297 wraps the same path as a single redact_term call



### Round 297 — redact_term convenience primitive (RFC P1) — pattern + replacement over publishable_* with auto-emitted [[publishable_override_ledger]] draft (forge-resistant, audit-half immutable) — redact_term wraps the R295 publishable setters into a single pattern-and-replacement scan over the publishable half of AtomicChangelogEntry — never reads or writes the audit half. Modes: literal / regex / case_insensitive. Scope: All / per-publishable-field. dry_run returns hits + ledger drafts without mutating; non-dry-run applies through R295 setters (validation + invariant preserved) and emits ready-to-paste [[publishable_override_ledger]] blocks whose content_hash_after equals the entry's post-apply publishable_hash_hex (R296 gate accepts as-is). Required reason + applied_in fields make every redaction auditable. CLI subcommand redact-term wired. Closes RFC G1 (no atomic primitive for cross-store term replacement); together with R292 query_term (G3), R294 schema split (G4), R296 ledger gate (G2), the full RFC P1+P2+P3+P4 surface is shipped.

**Changes**:
- crates/mnemosyne-validator/src/redact.rs new module: redact_term(store, sidecar_path, RedactRequest) -> RedactionReport. Walks changelog_entries, scans publishable_* per scope (All / DecisionSummary / ChangesBullets / VerificationBullets / ImpactRefs / CarryForwardBullets), applies literal or regex replace_all (case_insensitive optional), emits hits + auto-generated [[publishable_override_ledger]] draft text per touched entry with the post-mutation publishable hash so authors do not hand-author SHA256 anchors
- RedactError variants: EmptyPattern, InvalidRegex, MissingReason, MissingAppliedIn, Mutate { entry_id, source } — fail-fast on every authoring hole the audit trail needs
- dry_run mode returns the full hits list + ledger drafts without mutating the store; non-dry-run applies field-by-field through the R295 setters so per-bullet length validation and audit-invariant enforcement still apply
- audit half is never read for replacement and never written by this primitive — RFC G1 closure preserves the R294 immutability invariant
- CLI redact-term subcommand wired in atomic_cli.rs + main.rs dispatcher: --pattern + --replacement + (optional --regex / -i / --scope <s> / --dry-run / --kind / --sidecar / --json) + required --reason + --applied-in. Stdout prints per-hit diff + ready-to-paste ledger draft blocks for each touched entry
- 8 unit tests in redact.rs::tests cover dry_run no-mutation guarantee, apply-mutates-publishable-only invariant, scope filter narrowing, regex + case_insensitive composition, idempotency on repeat invocation, EmptyPattern / MissingReason / MissingAppliedIn / InvalidRegex error surfaces, and the contract that the ledger draft's content_hash_after equals the entry's post-apply publishable_hash_hex (R296 gate compatibility)
- lib.rs re-exports the redact module's public surface alongside the existing primitives



**Verification**:
- cargo test --release --workspace: 643 passed / 0 failed / 47 ignored (R296 = 635; +8 from redact tests)
- validate-workspace baseline: docs=1/1, T1=0, round-trip=1/1, T3 reject=0, GENERATED.md=sync, ledger=43, publishable / audit divergence: entries=0 ledger_rows=0
- redact_term_dry_run_does_not_mutate test: dry_run returns full hit list + ledger drafts but the store stays byte-identical (audit invariant + publishable invariant under preview)
- redact_term_apply_mutates_publishable_only test: explicitly asserts both halves — publishable_* takes the redacted value, audit_* keeps the original; publishable_matches_audit() = false post-apply
- redact_term_ledger_draft_hash_matches_post_apply_hash test: the printed content_hash_after equals the entry's publishable_hash_hex() — R296 gate accepts the draft as-is
- redact_term_idempotent_after_apply test: re-running the same redact yields zero hits (publishable_* no longer contains the pattern); safe to re-run as part of CI
- redact_term_regex_mode_with_case_insensitive: regex + -i compose correctly (email-shaped fixture)
- RFC P1 acceptance criteria walked: dry_run returns full hit list ✅; non-dry_run applies single sweep then reports drafts ✅ (mutate path goes through R295 setters); validate-workspace shows publishable / audit divergence + ledger gate from R296 ✅; generate_docs renders publishable view (R294) ✅; idempotent (re-run = no-op) ✅; removing a [[publishable_override_ledger]] row re-surfaces the rejection ✅ (R296 contract)



**Impact**: §atomic-store-mutate-api, §markdown-parser


**Carry forward**:
- RFC closure recap (taken across R292 + R294 + R295 + R296 + R297): G1 cross-store term replacement primitive ✅ R297 redact_term; G2 audit-trace structure for legitimate frozen overrides ✅ R296 [[publishable_override_ledger]]; G3 internal workspace search ✅ R292 query_term; G4 ChangelogEntry body split ✅ R294 schema split. RFC P1 + P2 + P3 + P4 all landed ahead of RFC's defer-P4 recommendation since schema-evolve-once was cheaper than schema-evolve-twice
- Cascade auto-emit of [[publishable_override_ledger]] draft on each R295 setter call (R296 carry) — narrowed: redact_term auto-emits the draft, but a bare set-changelog-publishable-* CLI invocation still requires the author to compute the hash. Defer until usage shows the bare-setter path is exercised at scale outside redact_term
- Per-field gate granularity carry (R296) — entry-level granularity remains; per-field hashes deferred until usage shows it matters
- mutate API hardening carry (rolled forward from R293/R294/R295/R296): append_changelog_entry silently accepts entry-id-only invocations with empty body — separate harden-pass; the boundary is exercised more under redact_term + R295 setters now
- Drift gate severity promotion carry (R293) — warn-only → reject decision still pending policy review
- MCP wire for the publishable setters + redact_term carry (R295/R297) — mechanical add via the existing tool-registration pattern; defer until usage data shows MCP need over CLI subprocess



### Round 298 — append_changelog_entry silent-accept gate: entry-id alone with empty body now rejected at primitive boundary

**Changes**:
- atomicrs check_changelog_entry_v2_required gate added: decision_summary required + non-blank, changes_bullets >=1 non-blank, verification_bullets >=1 non-blank, impact_refs and carry_forward_bullets optional vec but elements non-blank
- entry_id blank reject added ahead of frozen-ledger check so empty key cannot land
- FrozenLedger reject ordering preserved (existing changelog_entry_v2_frozen_after_append test exercises second append with empty body; check sequence unchanged)
- 6 r298_ unit tests in atomic.rs cover blank entry_id, missing decision_summary, empty changes, empty verification, blank bullet element, blank optional element
- 4 integration tests across atomic_first_validate_smoke / generate_docs_smoke / cascade_auto_update_smoke / atomic_round_trip backfilled with --verification-file or verify_bullets so they remain valid bodies



**Verification**:
- cargo test --release --workspace exits 0 with no FAILED or panicked emissions; R298 unit tests 6/6 pass
- validate-workspace baseline unchanged: ledger=44 / T1=0 / T3 reject=0 / round-trip=1/1 / GENERATED.md=sync / divergence=0 / drift=0
- silent-accept hole gated at primitive boundary so CLI append-changelog-entry-v2 with --entry-id alone now exits non-zero with Validation diagnostic




**Carry forward**:
- B: bare set_changelog_publishable_* setters still require manual ledger anchor (only redact_term auto-emits drafts) — carry until usage shows real friction
- E: per-field hash anchor not added; ledger gate remains entry-level. Partial divergence still not registerable
- F: MCP wire for publishable setters and redact_term still CLI-subprocess only
- D: drift gate severity warn-only; exception catalog pre-req before promotion



### Round 299 — MCP wire for publishable setters + redact_term: 6 new MCP tool methods so Claude can author publishable-half overrides without CLI subprocess

**Changes**:
- mnemosyne-mcp gains 6 tool methods: set_changelog_publishable_decision_summary / changes / verification / impact_refs / carry_forward, plus redact_term
- 3 new args structs (SetChangelogPublishableStringArgs, SetChangelogPublishableBulletsArgs, RedactTermArgs) with JsonSchema derives so the tools self-describe in MCP listings
- run_publishable_bullets helper added beside set_section_bullets to factor the temp-bullet-file wiring shared by 4 of the setters
- redact_term forwards --regex / --case-insensitive / --scope / --dry-run / --kind plus the mandatory --reason and --applied-in, with --json always set so the caller receives structured hits + ledger_drafts
- audit-half write-once invariant preserved: every new tool routes through the existing CLI subcommand layer so AtomicMutateError::FrozenLedger and the R296 ledger gate keep their teeth



**Verification**:
- cargo build --release -p mnemosyne-mcp finishes clean (3m 12s, exit 0, no error or unused-import warning)
- validate-workspace baseline unchanged: ledger=45 / T1=0 / T3 reject=0 / round-trip=1/1 / GENERATED.md=sync / divergence=0 / drift=0
- publishable setter and redact_term primitives unchanged on the validator side; MCP wire is a thin CLI-subprocess shim so R295 / R296 / R297 unit and integration coverage transfers verbatim




**Carry forward**:
- B: bare setter ergonomics — MCP tool is now wired but still requires manual [[publishable_override_ledger]] block authoring (redact_term auto-emits drafts; bare setters do not). Carry until usage shows real friction
- D: drift gate severity warn-only; promotion blocked on exception catalog
- E: per-field hash anchor not added; ledger gate remains entry-level



### Round 300 — emit_publishable_override_ledger_draft primitive: bare R295 setter callers now obtain a ready-to-paste ledger block without manual SHA256 work

**Changes**:
- atomicemit_publishable_override_ledger_draft primitive added: read-only render of a [[publishable_override_ledger]] block for a single entry whose publishable half diverges from audit
- AtomicChangelogEntrydivergent_publishable_fields enumerates the 5 publishable_* fields and returns only those that differ from their audit counterpart, in format_ledger_row order
- redactformat_ledger_row promoted from private to pubcrate so atomic.rs can reuse the exact ledger row shape produced by R297 redact_term
- mnemosyne-cli emit-publishable-override-ledger-draft subcommand wired plus matching MCP tool method with EmitPublishableOverrideLedgerDraftArgs schema
- 4 r300_ unit tests cover returns-None when in-sync, NotFound on missing entry_id, fields-list shows only divergent fields, and content_hash_after equals the live publishable_hash_hex



**Verification**:
- cargo test --release -p mnemosyne-validator --lib r300_ passes 4 of 4 with no FAILED or panicked
- cargo build --release -p mnemosyne-mcp -p mnemosyne-cli finishes clean
- smoke test  mnemosyne-cli emit-publishable-override-ledger-draft --entry Round 298 prints status: in sync — no ledger row required (the in-sync path is correctly inert)




**Carry forward**:
- D: drift gate severity still warn-only; exception catalog pre-req carry
- E: per-field hash anchor not added; ledger gate still entry-level so partial-divergence registration remains unavailable
- F+: redact_term-style hits report (which entry/field/index) not exposed for bare setters; emit primitive returns the rendered block but no structured per-field hash anchor



### Round 301 — drift gate severity warn-only → hard reject: validate-workspace bails when any cited round has no atomic-store entry

**Changes**:
- mnemosyne-cli print_commit_ledger_drift_surface promotes missing > 0 from warn-only print to hard bail
- diagnostic surface preserved: header line + per-round "missing R<N>" + backfill hint remain so the user sees what to fix before the gate exits non-zero
- no exception list, no mnemosyne.toml drift_gate section: R293 audit-trail-hole motivation kept intact (silent accept = anti-pattern), the fix is to backfill the missing entry
- doc comment updated; R293 footer "Future round may promote ..." is the R301 trigger condition that is now satisfied



**Verification**:
- cargo build --release -p mnemosyne-cli finishes clean (5m 46s)
- validate-workspace baseline post-promotion: missing=0 → exits 0, surface line "commit↔ledger drift: cited=33 / ledger=47 / missing=0" unchanged
- R293 commit_ledger.rs unit tests (clean_when_cited_subset_of_ledger / missing_round_surfaces_when_cited_but_absent / missing_and_extra_sorted_ascending) still cover the diff math; severity is a CLI-layer concern, no validator-side rewrite




**Carry forward**:
- E: per-field hash anchor (entry-level → field-level) still carry; use-case-driven, hold for actual divergence pattern
- B+: emit primitive returns rendered block but no structured hits report; raise when an RFC actually asks
- F+: MCP→CLI subprocess still in place; in-process direct call is ergonomics-only carry



### Round 302 — append_changelog_entry_v2 rename + legacy v1 CLI dispatch removal — API postfix versioning rule (user feedback): atomic::append_changelog_entry_v2 renamed to atomic::append_changelog_entry across function, MCP tool name, and CLI subcommand. Legacy v1 markdown surgical CLI dispatch removed (R251 left it dead); mutate::append_changelog_entry retained module-qualified for smoke test.

**Changes**:
- atomic::append_changelog_entry_v2 → atomic::append_changelog_entry (function, cmd_*, MCP tool name `append_changelog_entry`, CLI subcommand `append-changelog-entry`)
- removed legacy v1 CLI dispatch `append-changelog-entry` (cmd_append_changelog_entry markdown surgical path); mutate::append_changelog_entry function retained module-qualified for smoke test
- lib.rs top-level re-export: dropped mutate::append_changelog_entry, promoted atomic::append_changelog_entry to the unqualified name
- 17 files updated (validator src + cli src + mcp src + 4 mcp resources + 5 tests); MCP wire name change = breaking for external consumers per pre-release no-compat
- carries the no-`_vN`-postfix rule pinned in CLAUDE.md anti-patterns + global memory feedback-no-postfix-versioning



**Verification**:
- cargo test workspace: 62 test suites 0 fail
- validate-workspace: T1 orphan=0, round-trip 1/1, GENERATED.md sync, publishable/audit divergence=0
- validate-code-refs: 0 violations across 7 crates (severity_missing=reject, severity_binding=reject, severity_inventory=reject)
- pre-commit gate sequence intact (R301 commit↔ledger drift hard reject remains operational)



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- redact_term --scope publishable replays this rename across past entries' publishable views so GENERATED.md renders the unpostfixed name in historical sections; audit half stays frozen with `_v2` references intact
- mnemosyne.toml gains `[[publishable_override_ledger]]` rows (auto-drafted by redact_term) anchoring each transformed entry by content_hash



### Round 303 — external-spec adapter FR-1/FR-2 first-class land (RFC-002 promote from Phase 1.5) — AtomicSection.normative_excerpt + [workspace.spec_source] added as first-class fields. RFC-002 disposition's Phase 1.5 defer reversed — adding 2 fields cost ~half-day; defer label was over-cautious given R265/R275/R287 precedents of Phase 0 schema growth. Frozen-ledger semantic on normative_excerpt mirrors audit-half immutability.

**Changes**:
- AtomicSection.normative_excerpt field added — Option<NormativeExcerpt { text, anchor_url, source_revision }>. Mutate primitive set_section_normative_excerpt is append-only (None→Some allowed, Some→Some rejected with FrozenLedger error); spec rev drift modeled by superseding the Section
- [workspace.spec_source] TOML table added (url + revision + optional fetched_sha256/fetched_at) — single per workspace, validates absolute http(s) URL + non-empty revision + 64-char lowercase hex when present
- CLI subcommand set-section-normative-excerpt + MCP tool set_section_normative_excerpt + lib.rs re-exports wired
- 6 unit tests for the mutate primitive (set/reject-overwrite/blank-text/non-url/missing-host/trailing-newline trim) + 5 config tests for spec_source (minimal/full/non-http reject/blank revision/malformed sha)
- validate-workspace surfaces spec_source line when present



**Verification**:
- cargo test workspace: 86+25 pass (atomic+config), 0 fail across all crates
- validate-workspace: T1 orphan=0, round-trip 1/1, GENERATED.md sync, publishable/audit divergence=0, commit↔ledger drift missing=0
- validate-code-refs: 0 violations across 7 crates
- normative_excerpt field is opt-in (Option default None); existing atomic stores parse unchanged via #[serde(default)]



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- RFC-002 FR-3 (symbol-level binding enforcement) remains Phase 1+ — requires LSP/treesitter wiring outside Phase 0 paradigm
- RFC-002 FR-5 (multi-workspace bundling) remains reject — single spec_source per workspace by design
- Disposition addendum (claudedocs/mnemosyne-rfc-002-sce-response.md) records the Phase 1.5 → Phase 0 promotion of FR-1/FR-2 and the Round 302 wire-name change



### Round 304 — legacy mutate.rs module retired in full + set_section_decision_status_atomic renamed to set_section_decision_status (slot takeover)

**Changes**:
- crates/mnemosyne-validator/src/mutate.rs deleted in full (891 LOC). The module hosted 3 markdown-surgical primitives — add_cross_ref, set_section_decision_status, set_section_body — plus 12 supporting helpers (orphan_key, atomic_write, slug_for_unnumbered_external, find_section_end_position, find_section_heading, is_code_fence_line, predict_section_id_for_heading, parse_leading_section_number, find_section_body_range, find_first_heading_after, validate_general_after_write, finalize_mutate) and the MutateReceipt / MutateError / MutateErrorKind types. All three primitives were dead in production post-R251 source-md deletion: their byte-preserving edits targeted markdown files that no longer exist; the only remaining write target was the auto-emitted docs/GENERATED.md, where any surgical insert would be wiped on the next emitter cycle. Tests only passed because they spun up synthetic temp-dir markdown.
- crates/mnemosyne-cli/tests/remaining_mutate_primitives_smoke.rs deleted in full (234 LOC). The smoke test existed solely to keep the legacy primitives compilable; per no-legacy-carry, a test that only keeps dead code alive is itself dead.
- crates/mnemosyne-cli/src/main.rs: removed dispatch arms for add-cross-ref, set-section-decision-status (markdown variant), set-section-body; removed cmd_add_cross_ref, cmd_set_section_decision_status, cmd_set_section_body handlers and the handle_mutate_result wrapper; removed print_mutate_error, print_mutate_receipt, compute_post_mutate_style_summary helpers (sole callers were the deleted handlers); removed add_cross_ref / set_section_body / set_section_decision_status / MutateError / RefKind imports plus the now-unused schema::DecisionStatus import; trimmed the usage-line subcommand list accordingly; the T1 rule-4 atomic-axis remediation message now points at intent/rationale/impact_scope setters instead of the retired add-cross-ref CLI.
- crates/mnemosyne-validator/src/lib.rs: dropped pub mod mutate; dropped the pub use mutate::{...} re-export; deleted the R287 Phase-H carry comment; renamed the re-export set_section_decision_status_atomic to set_section_decision_status.
- crates/mnemosyne-validator/src/atomic.rs: renamed pub fn set_section_decision_status_atomic to set_section_decision_status (signature unchanged); updated the internal save_with_receipt primitive label from "set_section_decision_status_atomic" to "set_section_decision_status"; updated 4 cross-referencing doc comments in atomic.rs and the 5 unit tests under set_section_decision_status_atomic_* to the new names; the audit-half label change does not break frozen-ledger semantics because the receipt label is operational metadata, not a stored entry field.
- crates/mnemosyne-validator/src/validator.rs and code_refs.rs: doc-comment references updated to the new function and CLI subcommand names.
- crates/mnemosyne-validator/tests/style_audit_full_scale.rs: doc comment on classify_violation no longer cites the retired set_section_body; the SUBSTANTIVELY_DIRTY remediation path now names the atomic body-field setters (set-section-intent / -rationale / etc.).
- crates/mnemosyne-cli/src/atomic_cli.rs: renamed cmd_set_section_decision_status_atomic to cmd_set_section_decision_status; updated the import and the call site; deleted the "symmetric with the markdown-axis CLI" comment that referenced the now-removed cmd_set_section_decision_status sibling.
- crates/mnemosyne-cli/src/main.rs dispatch: set-section-decision-status now routes to atomic_cli::cmd_set_section_decision_status (the slot freed by the markdown-variant retirement).
- crates/mnemosyne-mcp/src/main.rs: the set_section_normative_excerpt tool description's inline CLI hint was updated from set-section-decision-status-atomic to set-section-decision-status.
- docs/GETTING_STARTED.md: the supersede-section example and the cascade-trigger description were updated to the renamed CLI subcommand; the trailing paragraph that advertised the legacy markdown-surgical primitives (append-changelog-entry, set-section-body, add-section) as "still available for ad-hoc edits" was deleted since none of those legacy variants exist anymore.
- docs/SCHEMA_GUIDE.md: the FR-1 spec-revision-drift example was updated to the renamed CLI subcommand.



**Verification**:
- cargo build --workspace clean (zero warnings after the schema::DecisionStatus unused-import was removed).
- cargo test --workspace: all suites pass; ~5 atomic.rs unit tests under set_section_decision_status_atomic_* were renamed via global sed and continue to pass under their new names; no test count regression beyond the 4 deleted smoke tests in remaining_mutate_primitives_smoke.rs (add_cross_ref_intra_doc_decision, add_cross_ref_rejects_orphan_target, set_section_body_replaces_content, set_section_body_rejects_missing_section, set_section_body_preserves_nested_sub_sections, and the two set_section_decision_status Phase-1+ stub tests).
- mnemosyne-cli validate-workspace passes: docs 1/1, T1 orphan total=0, round-trip mandatory=1/1, T3 reject=0, atomic ledger orphans=0+0, commit↔ledger drift cited=34/ledger=50/missing=0.
- Manual cascade-trigger smoke: set-section-decision-status --section §nonexistent --status superseded --superseding §some still surfaces the [cascade] stderr line and rejects via NotFound, matching R266 behavior under the new subcommand name.



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- Past entries R265 / R266 / R267 / R268 / R272 / R275 / R277 / R285 / R290 / R298 / R302 in the audit half (and therefore in docs/GENERATED.md as rendered today) still mention the historical names set_section_decision_status_atomic / set-section-decision-status-atomic / set_section_body / add_cross_ref. The audit half is frozen by design; forward-coherence in the publishable half is available via mnemosyne-cli redact-term + publishable_override_ledger if the divergence becomes user-visible noise. Currently carried per the session-2026-05-27 TODO 4 evaluation — author intent at the original round is the higher-priority audit truth.
- Cross-ref kind enum (RefKind::{Decision, Impl, CrossDoc}) is no longer imported anywhere outside the validator crate. The type itself remains in schema.rs because parser-derived cross_refs and the atomic_store cross_refs view still need it; only the CLI-side import was dropped.
- add-cross-ref as a standalone mutate primitive is no longer available. The atomic-store path adds cross-refs implicitly via the §<id> citations embedded in intent / rationale / impact_scope / changelog impact_refs prose, which the parser extracts into AtomicSection.cross_refs at workspace-load time. If an authoring loop needs an explicit set-element cross-ref setter (e.g., for cross_refs the prose doesn't naturally embed), the atomic side would need a new add_section_cross_ref primitive — not a port of the markdown-surgical predecessor.



### Round 305 — publishable setter field-invariant parity restoration + selective redact campaign for R302/R304 rename forward-coherence — 5 publishable setters dropped check_intent_len / check_bullet_len calls to mirror append_changelog_entry's cap-0 invariant (R295 paste-error closure). Field-invariant parity test pinned in atomic.rs. CLAUDE.md anti-pattern added for half-enforced multi-write-path invariants. R302/R304 rename forward-coherence applied via 3 redact-term passes across 9 historical entries (R265/R266/R269/R272/R294/R295/R296/R297/R298); R302/R304 self-references restored from snapshot since rename narrative ≡ decision content.

**Changes**:
- publishable setter 5개에서 check_intent_len / check_bullet_len call 제거 — set_changelog_publishable_decision_summary / _changes_bullets / _verification_bullets / _impact_refs / _carry_forward_bullets all mirror append_changelog_entry's cap-0 invariant (atomic.rs:1671-1766). check_intent_len / check_bullet_len 함수 자체는 section setter (R161 §41 facts-as-one-liner) 들이 계속 사용하므로 유지.
- CLAUDE.md anti-patterns block 에 §"field 에 두 개의 write path 두면서 invariant 만 다르게" 추가 — R295 paste-error 가 canonical case. 새 setter 추가 시 field-invariant parity test 동반 land 의무 명시.
- audit pass — codebase 위 atomic field × write-path matrix scan: 0 additional paste-error mismatches. add_section vs set_section_title/parent_doc/parent_section/inputs/outputs/caveats invariants 대칭 확인. add_inventory_entry vs set_inventory_status/section_ref invariants 대칭. append_changelog_entry vs publishable setters 비-cap asymmetry (non-blank gate, ≥1 element gate) 는 publishable looser 방향 — design intent (R296 redact substrate), paste-error 아님. R294/R295 cap mismatch 이 isolated incident 로 closure.
- field-invariant parity test 추가 — atomic.rs::tests::field_parity_decision_summary_accepts_uncapped_input (2 KiB summary) + field_parity_bullet_fields_accept_uncapped_elements (10 KiB bullets × 4 fields). 양 path (append + setter) 가 같은 edge-case input 을 accept 하는지 assert. paste-error 재발 시 CI catch.
- 3 redact-term 패턴 live apply: (1) append_changelog_entry_v2 → append_changelog_entry, R302 self-reference + R294/R295/R296/R297/R298 (5 forward-coherence); (2) set_section_decision_status_atomic → set_section_decision_status, R304 self-reference + R265/R266/R272 (3 forward-coherence); (3) set-section-decision-status-atomic → set-section-decision-status, R304 self-reference + R265/R266/R269/R272 (4 forward-coherence). 합 9 unique non-self-reference divergent entries.
- R302/R304 publishable 은 redact 후 /tmp/r302_r304_snapshot.json 의 authentic 값으로 복원 — self-reference entries 의 정체성 보존 (R302 = append_changelog_entry_v2 → append_changelog_entry rename 자체가 decision content; R304 = _atomic suffix drop 자체가 decision content). 복원 후 R302/R304 publishable_matches_audit == true, override ledger row 불필요.
- mnemosyne.toml 에 9 [[publishable_override_ledger]] rows append — R296 SHA256 gate 가 forge-resistance 보장 (content_hash_before/after 매치 must 으로 ledger row 변조 시 즉시 reject).



**Verification**:
- cargo test --workspace 전 suite 통과 (208 tests + 2 new field_parity tests).
- validate-workspace PASS: T1 orphan total=0, round-trip mandatory=1/1, T3 reject=0, atomic ledger entries=51 / sections=5 / orphan_refs=0+0 / GENERATED.md=sync, publishable / audit divergence entries=9 ledger_rows=9 (9-9 match).
- generate-docs PASS: sections rendered=5, changelog entries rendered=51, written_bytes=146986.
- post-redact publishable old-name occurrences: 9 historical forward-coherence entries (R265/R266/R269/R272/R294/R295/R296/R297/R298) eliminated to 0. R302/R304 publishable retains old names by design (rename narrative ≡ decision content).
- audit half old-name occurrences unchanged at 34 across all 51 entries — frozen ledger semantic 유지 확인 (redact-term 이 audit half 절대 mutate 하지 않음, R294 design intent).



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- field-invariant parity test 가 substrate — 향후 신규 publishable setter 추가 시 atomic.rs::tests::field_parity_* 같은 pattern 으로 양 path accept-symmetry assertion 동반 land 의무. CLAUDE.md anti-pattern §"field 에 두 개의 write path" 가 review-time anchor.
- Self-reference rename entries (R302, R304 같은 "rename 자체가 decision content" 인 round) 는 forward-coherence redact 에서 제외 — redact 적용 시 snapshot 복원으로 publishable_matches_audit == true 복원. 향후 추가 rename round 발생 시 같은 selective-restore pattern 적용.
- section setter cap policy 는 R161 §41 facts-as-one-liner 정통으로 유지 — check_intent_len (cap 200) / check_bullet_len (cap 800) 가 set_section_intent / _rationale / _inputs / _outputs / add_section_caveat 에 계속 적용. 본 cap drop 은 publishable setter 만의 fix, section 도메인 전파 아님.



### Round 306 — RFC-003 FR-1/FR-2 plugin substrate proof-first promote + RFC-002 FR-3 symbol-level enforcement first plugin — Plugin substrate (SymbolResolver / Validator trait + PluginCategory + Transport enum + PluginRegistry) landed via new mnemosyne-plugin crate. In-process transport + tree-sitter-rust backend (mnemosyne-plugin-tree-sitter-rust crate) wired as first proof. RFC-002 FR-3 symbol-level enforcement = scan_paths_bidirectional 의 ViolationKind::SymbolMismatch axis 로 production wire (opt-in via [plugins.symbol_resolver.rust]). [code_refs] → [plugins.set_equality_validator] in-place rename 동반 (부채 즉시 상환). MCP/CLI transport variant 는 placeholder NotImplemented (sample backend 미확보, R307+ wire).

**Changes**:
- mnemosyne-plugin crate 신규 (workspace 8th) — SymbolResolver + Validator trait, PluginCategory enum, Transport enum (InProcess / Mcp / Cli), VersionSurface / ValidationContext / ValidationFinding / Severity / ResolverError / ValidatorError types, PluginRegistry (explicit-init pattern, 글로벌 state 0 / inventory crate 의존성 0 / dlopen 0).
- mnemosyne-plugin-tree-sitter-rust crate 신규 (workspace 9th) — TreesitterRustResolver: SymbolResolver impl on tree-sitter 0.26 + tree-sitter-rust 0.24, fn/struct/enum/trait/impl/mod/const/static/type/union/macro_definition 노드 query, register(&mut PluginRegistry) entry point.
- mnemosyne.toml [plugins.*] table 신규 + [code_refs] → [plugins.set_equality_validator] in-place rename. CodeRefsSection struct → SetEqualityValidatorConfig 동반 rename, 모든 doc comment / help text / test fixture / CLI 출력 일괄 변경 (pre-release no-compat, 부채 즉시 상환).
- scan_paths_bidirectional signature 에 symbol_resolvers: Option<&BTreeMap<String, Box<dyn SymbolResolver>>> 인자 추가. CitationUnbound 통과한 file-bound citation 마다 cited section.implementations[file=cited_file].symbol.is_some() 인 entry 위 resolver 호출 + 매치 검증. mismatch → ViolationKind::SymbolMismatch (binding-class severity bucket — severity_binding knob 공동 governance).
- McpResolver / CliResolver placeholder impl 추가 — Transport enum 의 Mcp / Cli variant 가 type / config / registry path 모두 land, resolve_symbol_at 호출 시 ResolverError::NotImplemented 반환. R307+ sample backend 확정 후 production wire.
- mnemosyne-cli/main.rs build_symbol_resolver_map helper — [plugins.symbol_resolver.<lang>] 읽고 InProcess transport 는 tree-sitter-rust 등록, Mcp / Cli 는 placeholder 등록. cmd_validate_code_refs 가 map 전달. 미명시 lang = file-only set-equality 유지 (5-min setup carry).
- validate-code-refs 의 ViolationKind 카운트 array 가 9 slot (R306 SymbolMismatch 추가). binding_count 에 symbol_mismatch_count 포함 (defect_class = Binding).
- 3 RFC-002 FR-3 enforcement smoke tests (happy / mismatch / opt-out / no-symbol-in-impl) — happy_path 0 SymbolMismatch, mismatch 1 SymbolMismatch with citation.line=2, opt-out (resolver 미등록) file-only 유지, no-symbol (Implementation.symbol None) axis silent.
- 3 transport_parity tests (Mcp NotImplemented / Cli NotImplemented / 모든 transport 위 version_surface 존재) — RFC-003 §4.2 transport-abstraction 의 의의를 R307+ wire 시점에 깨지지 않도록 pin.
- 6 TreesitterRustResolver unit tests (fn at definition / fn in body / struct / nested fn in impl / outside-any-item None / register round-trip) — backend 의 R306 capability surface 검증.
- RFC-002 §"Round 306 — FR-3 land (plugin substrate proof-first)" addendum + RFC-003 §"Round 306: FR-1/FR-2 land + FR-3 absorbed first proof" addendum — R303 패턴 따름 (Phase 0.5 defer 라벨 정정 + sustained trigger 명시 + carry forward 7 항목).



**Verification**:
- cargo test --workspace: total 665 passed / 0 failed. 새 추가 tests = mnemosyne-plugin 2 unit + 3 integration (transport_parity), mnemosyne-plugin-tree-sitter-rust 6 unit, mnemosyne-validator 4 integration (symbol_enforcement_smoke). 기존 660 tests regression 0.
- validate-workspace 최종 PASS: T1 orphan total=0, round-trip mandatory=1/1, T3 reject=0, atomic ledger entries=53 / sections=5 / orphan_refs=0+0 / GENERATED.md=sync, publishable / audit divergence entries=9 ledger_rows=9, commit↔ledger drift cited=N / ledger=53 / missing=0.
- cargo build --workspace: 9 crates compile clean (mnemosyne-cascade / cli / core / mcp / plugin / plugin-tree-sitter-rust / server / store / validator). Dependency graph cycle 0.
- mnemosyne-cli 의 validate-code-refs 회귀 0: [code_refs] → [plugins.set_equality_validator] rename 후 모든 11 test cases (case_i_skip / case_ii_clean / case_iii_hallucinated_reject / case_iv_warn_severity / case_v_identifier_shaped / case_vi_json_shape / case_vii_filter_id_decay / case_viii_section_missing / case_ix_citation_unbound / case_x_impl_unbacked / case_xi_severity_binding_warn) 통과.



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- SetEqualityValidator struct extraction — R306 의 scan_paths_bidirectional signature 확장은 minimum proof. 진정 plugin architecture 정합화 = SetEqualityValidator: Validator struct 가 config + symbol_resolvers Box 들 carry, cmd_validate_code_refs 가 PluginRegistry::validator() 통해 dispatch. R307+ cohesive scope.
- AtomicStoreView trait — ValidationContext 가 store reference carry 못해 (mnemosyne-plugin → mnemosyne-validator cycle 방지) Validator trait 의 진정한 호출 미land. R307+ 에 minimum view trait (changelog_keys / section_ids / implementations_by_section 등) 도입으로 validator-class plugin dispatch 정합화.
- MCP transport production wire — McpResolver placeholder NotImplemented → real MCP client wire. Sample backend 후보 (a) Python LSP wrapper (b) mnemosyne-mcp 가 SymbolResolver MCP tool 노출 + 다른 mnemosyne-mcp instance 호출 self-referential dogfood. R307+.
- CLI transport production wire — CliResolver placeholder NotImplemented → real shell-out + output_parser wire. Sample backend 후보: gopls / clangd / pyright. Round 306 시 gopls 시스템 미설치라 placeholder. R307+.
- non-Rust SymbolResolver backends — Python (tree-sitter-python + 별도 crate), Go, TypeScript / 등. 한 라운드 = 한 plugin backend = explicit Cargo edge (R306 의 mnemosyne-plugin-tree-sitter-rust 분리 pattern 따름).
- severity_symbol knob — SymbolMismatch 가 현재 severity_binding bucket 공유 (R269 패턴). 별도 knob 도입은 measurement 발현 시 R307+ (R262 → R263 measure-then-promote precedent).
- [plugins.set_equality_validator] 내부 sub-axis split — inventory / external_ref_skipper 가 별도 ValidatorClass plugin 으로 분리 가능 (R275 inventory + R277 external_section_prefixes lifecycle 분리). R307+ plugin substrate 확장.



### Round 307 — RFC-003 D1+D2 closure — Validator trait dispatch via PluginRegistry production-wires SetEqualityValidator + AtomicStoreView trait lifts atomic-store reads onto a JSON-serializable snapshot for R308 transport prep

**Changes**:
- mnemosyne-plugin grows AtomicStoreView trait + AtomicSnapshot (with SectionView ImplementationRef DecisionStatusView InventoryStatusView closed-form JSON-serializable types) so Validator plugins read the atomic store across the Cargo trust boundary without a reverse edge into mnemosyne-validator
- mnemosyne-plugin ValidationContext gains store reference; ValidationFinding extended with kind Option String + extras BTreeMap String Value for rich payload preservation across trait dispatch
- mnemosyne-validator impl AtomicStoreView for AtomicStore materializes the eager snapshot — changelog ids + section ids with implied parents + per-section impls + inventory status
- mnemosyne-validator SetEqualityValidator struct owns config + entry_id_prefix + orphan_ledger + symbol_resolvers + filter_id; scan_paths_bidirectional free function absorbed into SetEqualityValidator scan method driven from AtomicSnapshot
- mnemosyne-validator impl Validator for SetEqualityValidator + violation_to_finding adapter maps CodeRefViolation kinds across the plugin boundary (kind tag preserved + extras carry entry_id symbol decision_status)
- mnemosyne-cli cmd_validate_code_refs constructs SetEqualityValidator and dispatches via PluginRegistry validator lookup + Validator validate ctx; JSON and TTY output reconstructed from ValidationFinding fields and extras
- Legacy carry retired — scan_paths + scan_paths_filtered + their two dedicated tests removed per no-legacy-carry rule; pre-R260 entry-id-only path superseded by SetEqualityValidator scan



**Verification**:
- cargo build --release green across all 9 workspace crates
- cargo test --release green — validator_trait_dispatch + atomic_store_view_parity new suites pass; symbol_enforcement_smoke migrated to SetEqualityValidator scan with no behavior change
- cargo run mnemosyne-cli validate-workspace baseline clean — T3 reject=0 / T1 orphan=0 / round-trip 1/1 / atomic ledger 53 entries / commit↔ledger drift=0
- cargo run mnemosyne-cli validate-code-refs runs through PluginRegistry dispatch path (zero violations on the workspace)
- validator_trait_dispatch test asserts ValidationFinding kind tag round-trip across missing impl_missing decay axes + extras carry entry_id symbol decision_status
- atomic_store_view_parity test asserts snapshot fields match raw AtomicStore field access (changelog ids section ids with implied parents implementations decision_status inventory status)




**Carry forward**:
- R308 D3 transport abstraction proof — MCP self-ref dogfood (mnemosyne-mcp exposes SymbolResolver tool + McpResolver real client wire + transport_parity asserts InProcess vs MCP equality on same file line)
- R309 D4+D5 medium adapter substrate — MediumAdapter trait lands in mnemosyne-core + DesignDocAdapter refactor (behavior-preserving) + mnemosyne-core owning surface declared
- R310+ D6 external plugin extension — dlopen libloading or external-binary orchestrator path so external users add backends without forking mnemosyne (RFC-003 risk register #5 closure)
- D7 severity_symbol promote — Mnemosyne dogfood activates plugins symbol_resolver rust block + N round measurement evidence before promotion
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger re-measure value × measurability over risk × unmet_deps; confirm fictional adapter is still #1 or accept drift



### Round 308 — D9 closure — workspace lint baseline lifted to deny via -D warnings on pre-commit + pre-push; curated allow list (doc_lazy_continuation + inconsistent_digit_grouping) covers stylistic exemptions, per-site #[allow] covers API-shape exemptions; 230 warnings → 0

**Changes**:
- Root Cargo.toml grows [workspace.lints.clippy] with all warn (priority -1) plus curated allow list — doc_lazy_continuation (rustdoc renders both bullet-continuation styles, 206 stylistic sites) and inconsistent_digit_grouping (YYYY_MM_DD u64 date literals in changelog facts and cascade snapshots preserve human-readable date semantics)
- Per-crate [lints] workspace = true added across all 9 workspace members so the shared lint baseline reaches every crate (and is the only place to bump pedantic deny later)
- Per-site #[allow] annotations for API-shape exemptions where workspace-level allow would be too broad — clippy::too_many_arguments on append_changelog_entry (8-arg public mutate API surface; bundling forces every CLI MCP and test caller to construct a new type with no readability win), clippy::result_large_err on tonic interceptors require_authorization_metadata + with_tracing_span (tonic::Status is the interceptor contract; boxing breaks the trait signature downstream consumers compose against)
- Fixed 18 actionable warnings — 6 field_reassign_with_default in R307 test files and mnemosyne-mcp list_resources converted to struct-literal form; 1 slice_arg_type sort_violations takes &mut [_] not &mut Vec; 1 unnecessary_get_then_is_none in atomic_store_view_parity becomes !contains_key; 1 if_same_then_else in to_github_anchor merges identical whitespace branches; 1 path_statement in handler test replaced with let _dir; 1 unnecessary_unwrap in parse_section_args uses if let Some
- Auto-fix applied for default_constructed_unit_structs (19 sites collapsed to bare unit struct) plus redundant_closure (2 sites) and other machine-applicable lints via cargo clippy --workspace --all-targets --fix
- pre-commit Gate 4 and pre-push clippy gate both lift to -D warnings — every clippy warning at any level becomes a deny gate, allow list and per-site annotations are the only legal exemptions



**Verification**:
- cargo clippy --workspace --all-targets --release -- -D warnings exits 0 (pre-R308 baseline: 230 warnings — 206 doc_lazy_continuation plus 9 inconsistent_digit_grouping plus 18 actionable code issues)
- cargo test --workspace --release all green — every test suite passes after the field_reassign struct-literal conversions and the to_github_anchor branch merge (no behavior regression)
- cargo run mnemosyne-cli validate-workspace baseline clean — entries 54 / sections 5 / T3 reject 0 / T1 orphan 0 / round-trip 1/1 / atomic ledger sync / commit-ledger drift 0
- .githooks/pre-commit Gate 4 + .githooks/pre-push clippy invocation updated to cargo clippy --workspace --all-targets -- -D warnings — pushing a commit with any new warning now fails the hook
- Workspace allow list documents the two stylistic exemptions inline with their justification (rustdoc bullet-continuation parity and YYYY_MM_DD date u64 literal preservation)




**Carry forward**:
- R309 D3 transport abstraction (MCP self-ref dogfood) — original R308 plan deferred; new mnemosyne-plugin-mcp-resolver crate with McpProcessResolver (rmcp client + TokioChildProcess + Runtime::block_on bridge), resolve_symbol_at MCP tool in mnemosyne-mcp, transport_parity integration test asserting InProcess vs MCP equality
- R310 D4+D5 medium adapter substrate — MediumAdapter trait in mnemosyne-core + DesignDocAdapter refactor + mnemosyne-core owning surface declaration
- R311+ D6 external plugin extension mechanism — dlopen libloading or external-binary orchestrator path (RFC-003 risk register #5)
- D7 severity_symbol promote — Mnemosyne dogfood activates plugins.symbol_resolver.rust block; N round measurement evidence before promotion decision
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger re-measure value × measurability over risk × unmet_deps
- Future pedantic tightening — current workspace lint baseline is clippy::all warn; promote to clippy::pedantic warn (or selectively deny on chosen pedantic lints) once the codebase has absorbed a few rounds of D9 baseline



### Round 309 — Textbook unification — DecisionStatus + InventoryStatus enums lifted to mnemosyne-plugin substrate; parallel View enums (DecisionStatusView + InventoryStatusView) and view_to_schema adapter retired; single canonical home across the workspace

**Changes**:
- DecisionStatus enum lifted from mnemosyne-validator::schema to mnemosyne-plugin (substrate-canonical home) — all derives preserved (Debug Clone Copy PartialEq Eq PartialOrd Ord Hash Serialize Deserialize) plus serde rename_all lowercase
- InventoryStatus enum lifted from mnemosyne-validator::atomic to mnemosyne-plugin (substrate-canonical home) — all derives preserved (Debug Clone Copy PartialEq Eq Serialize Deserialize) plus Default Active plus serde rename_all snake_case
- DecisionStatusView and InventoryStatusView parallel View enums retired in full — they existed only as boundary adapters and are no longer needed since SectionView.decision_status and AtomicSnapshot.inventory now carry the canonical types directly
- view_to_schema_decision_status adapter function retired from code_refs.rs — Step 4 coverage axiom emits CodeRefViolation::ImplementationMissing.decision_status directly from snapshot section.decision_status without enum translation
- impl AtomicStoreView for AtomicStore simplified — Inventory and Section iteration paths drop the match-arm view conversion and copy canonical enum values straight into the snapshot
- 17 import sites updated across mnemosyne-validator (lib.rs pub use lines, schema.rs use, atomic.rs use, code_refs.rs lib + 7 test mods, parser.rs, query.rs, validator.rs, workspace.rs tests, style.rs 10 fixture rows) plus mnemosyne-cli (atomic_cli.rs + main.rs 8 sites) plus 2 integration tests
- mnemosyne-validator no longer re-exports DecisionStatus or InventoryStatus from its crate root — single canonical import path is mnemosyne_plugin::DecisionStatus and mnemosyne_plugin::InventoryStatus across the workspace



**Verification**:
- cargo build --release green across all 9 workspace crates after enum lift and 17-site import migration
- cargo test --workspace --release green — atomic_store_view_parity test now imports DecisionStatus + InventoryStatus from mnemosyne_plugin directly, no view-type indirection; style_smoke test migrated similarly
- cargo clippy --workspace --all-targets --release -- -D warnings exits 0 — R308 D9 gate held under the refactor (no new warnings introduced by the enum migration)
- cargo run mnemosyne-cli validate-workspace baseline clean — entries 55 / sections 5 / T3 reject 0 / T1 orphan 0 / round-trip 1/1 / atomic ledger sync / commit-ledger drift 0
- Serde wire format unchanged — both lifted enums preserve their exact serde rename_all attributes (DecisionStatus lowercase / InventoryStatus snake_case) so docs/.atomic/workspace.atomic.json round-trips byte-identically (verified via validate-workspace round-trip mandatory N/N)
- JSON wire-format spot-check on a Deprecated InventoryStatus entry round-trips as deprecated string with single-word variants serializing identically under lowercase and snake_case rules (no on-disk migration needed)




**Carry forward**:
- R310 D3 transport abstraction (MCP self-ref dogfood) — original R308/R309 plan moves forward to R310; mnemosyne-plugin-mcp-resolver crate with McpProcessResolver (rmcp client + TokioChildProcess + Runtime::block_on bridge), resolve_symbol_at MCP tool in mnemosyne-mcp, transport_parity integration test asserting InProcess vs MCP equality
- R311 D4+D5 medium adapter substrate — MediumAdapter trait in mnemosyne-core + DesignDocAdapter refactor + mnemosyne-core owning surface declaration
- R312+ D6 external plugin extension mechanism — dlopen libloading or external-binary orchestrator path (RFC-003 risk register #5)
- D7 severity_symbol promote — Mnemosyne dogfood activates plugins.symbol_resolver.rust block; N round measurement evidence before promotion decision
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger re-measure value × measurability over risk × unmet_deps
- ValidationFinding.extras typing review — re-evaluated and accepted as correct substrate extensibility pattern; analogous to MCP tool result content + GraphQL extensions + OTel attributes; no further work needed
- Future pedantic tightening — current workspace lint baseline is clippy::all warn; promote to clippy::pedantic warn (or selectively deny on chosen pedantic lints) once codebase absorbs a few rounds of D9 baseline



### Round 310 — Plugin substrate rename to mnemosyne-core + legacy core typed-facts layer rename to mnemosyne-facts — D5 closure (substrate role declared by name) and 13-smell #4 closure (layering inversion fix; schema → core dependency direction is now honest since core is the substrate that defines what plugins implement against)

**Changes**:
- mnemosyne-plugin crate renamed to mnemosyne-core — Validator + SymbolResolver + AtomicStoreView traits + DecisionStatus + InventoryStatus enums + PluginRegistry + PluginCategory + Transport enum + ValidationContext + ValidationFinding now live under the substrate-canonical name; previous "plugin" name was a misnomer since this crate hosts the contracts plugins implement against, not a plugin itself
- legacy mnemosyne-core crate renamed to mnemosyne-facts — typed-fact persistence layer binding mnemosyne-store typed put/get for the 4 entity/relation kinds (SectionFact + ChangelogEntryFact + CrossRefFact + FrozenListFact) + GraphSpec + EntityDef + canonical_identifier_set + 5-language code emit (rust kotlin python cpp protobuf); new name accurately describes role and frees mnemosyne-core slot for the substrate
- mnemosyne-plugin-tree-sitter-rust crate kept its name unchanged — it is genuinely a plugin implementation of SymbolResolver so the "plugin" prefix now correctly identifies it relative to the renamed mnemosyne-core substrate
- 71 mnemosyne_plugin import sites migrated to mnemosyne_core across mnemosyne-validator (15 src modules + 18 integration tests) + mnemosyne-cli (atomic_cli.rs + main.rs) + mnemosyne-plugin-tree-sitter-rust deps + transport_parity test
- 9 mnemosyne_core import sites migrated to mnemosyne_facts across mnemosyne-cascade (runtime.rs + fine_grained.rs + snapshot.rs + phase_1_5_measurement.rs test) + mnemosyne-server/src/error.rs + crates/mnemosyne-facts/tests/entity_persist.rs + bench/crates/cascade-measurement/src/lib.rs
- workspace Cargo.toml members list updated — crates/mnemosyne-core directory now houses the substrate and crates/mnemosyne-facts houses the typed-fact persistence layer; 5 consumer Cargo.toml dep paths updated (cli + validator + plugin-tree-sitter-rust + server + cascade + cascade-measurement bench)
- mnemosyne.toml [plugins.set_equality_validator].paths updated — crates/mnemosyne-core/src/ now points at substrate sources and crates/mnemosyne-facts/src/ at typed-fact sources so code-citation defense scans the renamed directories
- doc comments and 5-language emit prologue strings updated — mnemosyne-facts/src/emit.rs ("// Auto-generated by mnemosyne-facts" across rust kotlin python cpp protobuf emitters) + mnemosyne-cascade/src/snapshot.rs cross-crate references + mnemosyne-store/src/store.rs neighbor-crate comments reflect new package names
- 13-smell #4 (layering inversion) closure — mnemosyne-validator::schema::Section.decision_status still depends on mnemosyne-core, but the dependency direction is now honest because mnemosyne-core is by name the substrate; domain depending on substrate is the correct layering
- D5 closure (mnemosyne-core role declaration) — the crate name now declares the role; substrate ownership of plugin contracts + domain enums + registry + validation framework is no longer ambiguous



**Verification**:
- cargo build --workspace green after both renames across all 9 workspace crates (mnemosyne-store + mnemosyne-facts + mnemosyne-core + mnemosyne-cascade + mnemosyne-server + mnemosyne-cli + mnemosyne-mcp + mnemosyne-validator + mnemosyne-plugin-tree-sitter-rust)
- cargo test --workspace --no-fail-fast green — full integration suite passes under new import paths (validator_trait_dispatch + atomic_store_view_parity + symbol_enforcement_smoke + 18 validator integration tests + cascade phase_1_5_measurement + server + facts entity_persist all green)
- cargo clippy --workspace --all-targets -- -D warnings exits 0 — R308 D9 baseline held under the rename refactor without introducing new warnings
- mnemosyne-cli validate-workspace baseline clean — entries 56 / sections 5 / T3 reject 0 / T3 warn 2 / T4 info 7 / T1 orphan 0 / round-trip 1/1 / atomic ledger sync / commit-ledger drift 0
- on-disk atomic store wire format unchanged — both renames are package-level Cargo.toml name + import-path only mutations; no schema field renames + no serde attribute changes + no fact bytes layout touched + docs/.atomic/workspace.atomic.json round-trips byte-identically
- code-citation defense (validate-code-refs) still passes — path scan targets updated in mnemosyne.toml so the renamed crate sources stay covered by the gate



**Impact**: §generatedmd--atomic-store-derived-view/changelog-atomic-ledger/round-306--rfc-003-fr-1fr-2-plugin-substrate-proof-first-promote--rfc-002-fr-3-symbol-level-enforcement-first-plugin--plugin-substrate-symbolresolver--validator-trait--plugincategory--transport-enum--pluginregistry-landed-via-new-mnemosyne-plugin-crate-in-process-transport--tree-sitter-rust-backend-mnemosyne-plugin-tree-sitter-rust-crate-wired-as-first-proof-rfc-002-fr-3-symbol-level-enforcement--scan_paths_bidirectional-의-violationkindsymbolmismatch-axis-로-production-wire-opt-in-via-pluginssymbol_resolverrust-code_refs--pluginsset_equality_validator-in-place-rename-동반-부채-즉시-상환-mcpcli-transport-variant-는-placeholder-notimplemented-sample-backend-미확보-r307-wire, §generatedmd--atomic-store-derived-view/changelog-atomic-ledger/round-307--rfc-003-d1d2-closure--validator-trait-dispatch-via-pluginregistry-production-wires-setequalityvalidator--atomicstoreview-trait-lifts-atomic-store-reads-onto-a-json-serializable-snapshot-for-r308-transport-prep, §generatedmd--atomic-store-derived-view/changelog-atomic-ledger/round-309--textbook-unification--decisionstatus--inventorystatus-enums-lifted-to-mnemosyne-plugin-substrate-parallel-view-enums-decisionstatusview--inventorystatusview-and-view_to_schema-adapter-retired-single-canonical-home-across-the-workspace


**Carry forward**:
- R311 13-smell #5 god-crate decomposition — split mnemosyne-validator (15 src modules + 18 integration tests) into cohesion-driven crates (schema + parser + atomic + validate + style + query + workspace orchestrator); each crate owns one reason to change; substrate naming already in place via this round
- R312 13-smell #1 + #2 typed Validator trait + dedup finding — trait Validator with associated type Finding Serialize plus ErasedValidator object-safe wrapper for dynamic dispatch; retire ValidationFinding stringly-typed extras BTreeMap and CodeRefViolation duplicate representation
- R313 13-smell #8 mnemosyne-mcp library API split — mnemosyne-mcp tool methods call mnemosyne-validator library API directly instead of spawning mnemosyne-cli subprocess (eliminate process fork + arg parsing + JSON round-trip per call)
- R314 13-smell #6 + #7 main.rs decomposition — cli commands module split (validate + query + style + append + each cmd_ function into its own module) plus append_changelog_entry 8-arg builder or request struct to retire too_many_arguments per-site allow
- R315 D3 transport abstraction MCP self-ref dogfood — was originally R309 R310 plan; deferred because transport-on-stringly-typed-boundary would deepen #1 + #2 debt; only enter after R312 typed Validator trait closure
- R316+ D4 MediumAdapter trait plus DesignDocAdapter refactor — Phase 1A prerequisite; medium adapter trait home declared on mnemosyne-core or on a new mnemosyne-medium crate; narrative adapter lands as second impl in Phase 1A
- R317+ D6 external plugin extension mechanism — dlopen libloading dynamic loading or external-binary orchestrator path (RFC-003 section 5 risk register entry 5 plugin lifecycle ownership); large design round
- D7 severity_symbol Mnemosyne self-dogfood — activate plugins.symbol_resolver.rust in mnemosyne.toml plus N round measurement evidence before promotion decision (R263 measure-then-promote pattern)
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger re-measure parameter value times measurability over risk times one plus unmet deps; fictional adapter may no longer be number one
- 13-smell #3 AtomicSnapshot eager allocation lazy iterator GAT — defer until ledger entries cross 10K scale threshold; current 56 entries is well below hot path concern
- 13-smell #9 doc_lazy_continuation 206 sites blanket allow removal — pure stylistic carry; address as continuous-improvement work without blocking on a single round
- 13-smell #10 YyyyMmDd typed newtype — replace inconsistent_digit_grouping blanket allow with strong type at 9 fact sites; mechanical refactor
- 13-smell #11 Box Status tonic interceptor allow — tonic API constraint; remove only when upstream tonic relaxes interceptor trait signature
- 13-smell #12 AtomicSection 14 field data clump analysis — extract Outline title parent_doc parent_section sub-struct candidate; needs cohesion measurement before commit
- 13-smell #13 ValidationContext PluginRegistry reference for multi-validator composition — add when first composition use case materializes (currently no multi-validator scenario)



### Round 311 — Mnemosyne-validator god-crate decomposition first wave — 4 leaf crates (mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic) extracted with full consumer migration; validator shrunk from 15 modules to 10; façade-free per CLAUDE.md no-legacy-carry; 13-smell #5 partial closure (3 more rounds R312/R313 to fully delete mnemosyne-validator crate)

**Changes**:
- mnemosyne-schema crate extracted from mnemosyne-validator src/schema.rs — 4 typed-fact entity/relation types (Section + ChangelogEntry + FrozenList + CrossRef) plus LockKind RefKind ParsedDoc plus sha256_hex canonical helper plus section_by_id traversal helper now live in their own leaf crate (depends only on mnemosyne-core for DecisionStatus)
- mnemosyne-config crate extracted from mnemosyne-validator src/config.rs — mnemosyne.toml loader (LoadedConfig + WorkspaceConfig + SchemaSection + StyleSection + PluginsSection + TerminologySection + AtomicConfigSection + SetEqualityValidatorConfig + SymbolResolverConfig + OrphanKind + OrphanLedgerEntry + PublishableOverrideLedgerEntry) plus discover_config + load_config + parse_config primitives; pure data + serde + toml + anyhow only, no internal deps
- mnemosyne-parser crate extracted from mnemosyne-validator src/parser.rs and src/emitter.rs — markdown bytes ↔ ParsedDoc bidirectional transform; parse_markdown + parse_markdown_with_schema + compare_typed_facts + emit_markdown_with_default + to_github_anchor + RoundTripDiff; emitter is a sub-module since parser and emitter are paired round-trip primitives
- mnemosyne-atomic crate extracted from mnemosyne-validator src/atomic.rs and src/redact.rs — AtomicStore + AtomicSection + AtomicChangelogEntry + InventoryEntry + ExampleBlock + Implementation + NormativeExcerpt + RejectedAlternative types plus all atomic mutate primitives (append_changelog_entry + add_section + set_section_* + add_section_* + remove_section_* + add_inventory_entry + set_inventory_* + remove_inventory_entry + 5 publishable setters + emit_publishable_override_ledger_draft) plus redact_term + RedactRequest + RedactionReport — redact lives in mnemosyne-atomic since publishable redaction is an atomic mutation operation
- emit_markdown no-arg convenience function dropped from mnemosyne-parser — it hardcoded mnemosyne-validator workspace Workspace::MNEMOSYNE_DEFAULT_DOC which was a workspace-specific coupling that did not belong in a generic parser crate; callers now use emit_markdown_with_default(doc, default_doc_or_None) explicitly so external workspaces are not forced into the Mnemosyne self-application default
- 4 new workspace members added to root Cargo.toml — mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic
- mnemosyne-validator/lib.rs no longer hosts pub mod schema + pub mod config + pub mod parser + pub mod emitter + pub mod atomic + pub mod redact and no longer pub use re-exports of those modules' items — façade-free per CLAUDE.md no-legacy-carry policy (lib.rs re-exports of superseded modules count as legacy carry)
- 71 mnemosyne_validator::* consumer use sites migrated across mnemosyne-cli (main.rs + atomic_cli.rs) and mnemosyne-validator/tests (18 integration tests) — each use block split by which extracted crate owns the imported type; sub-module paths (atomic:: schema:: config:: parser:: redact:: emitter::) rewritten to point at the new crate root
- mnemosyne-cli Cargo.toml gained dependencies on mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic so the binary can call the extracted crates directly
- mnemosyne-validator dev-dependencies gained mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic + serde_json so the 18 integration tests retained in mnemosyne-validator/tests/ still compile while the source extraction proceeds (these tests will move to their respective domain crates in a future round once style + query + validate + workspace are also extracted)
- mnemosyne-validator src/ shrunk from 15 modules + lib.rs (16804 lines) to 10 modules + lib.rs (validator + workspace + query + t2 + style + render + code_refs + commit_ledger) — 5 modules + ~7000 lines lifted into the 4 extracted crates



**Verification**:
- cargo build --workspace green across all 13 workspace crates (mnemosyne-store + mnemosyne-facts + mnemosyne-core + mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic + mnemosyne-cascade + mnemosyne-server + mnemosyne-cli + mnemosyne-mcp + mnemosyne-validator + mnemosyne-plugin-tree-sitter-rust)
- cargo test --workspace --no-fail-fast green — 76 test result groups all pass with the 4 extracted crates plus their integration tests (atomic_round_trip + atomic_store_view_parity + changelog_pattern_plugin + 5 style_* + symbol_enforcement_smoke + validator_trait_dispatch + workspace_config_integration + self_application_via_generic + self_validation + external_fixtures_integration + generated_vs_legacy_audit + schema_as_input_integration + phase_1_priority_audit)
- cargo clippy --workspace --all-targets -- -D warnings exits 0 — R308 D9 baseline held under the god-crate decomposition refactor; no new warnings introduced across the 4 new crates or the touched consumer files
- mnemosyne-cli validate-workspace baseline clean — entries 57 / sections 5 / T3 reject 0 / T1 orphan 0 / round-trip 1/1 / atomic ledger sync / commit-ledger drift 0
- atomic store wire format unchanged — the decomposition moved Rust source between crate boundaries only; no schema field renames + no serde attribute changes + no fact bytes layout touched + docs/.atomic/workspace.atomic.json round-trips byte-identically
- code-citation defense gate still passes — mnemosyne.toml [plugins.set_equality_validator].paths covered the validator/src/ tree which now spans crates/mnemosyne-validator/src/ plus crates/mnemosyne-schema/src/ + crates/mnemosyne-config/src/ + crates/mnemosyne-parser/src/ + crates/mnemosyne-atomic/src/; paths updated in mnemosyne.toml to cover the new crate sources
- API surface for downstream consumers narrowed — callers must import from the canonical crate (mnemosyne_schema::Section + mnemosyne_config::LoadedConfig + mnemosyne_parser::parse_markdown + mnemosyne_atomic::AtomicStore) instead of via mnemosyne_validator façade; future-round consumer migration cost is bounded since this round closed the migration for the 4 extracted concerns



**Impact**: §generatedmd--atomic-store-derived-view/changelog-atomic-ledger/round-310--plugin-substrate-rename-to-mnemosyne-core--legacy-core-typed-facts-layer-rename-to-mnemosyne-facts--d5-closure-substrate-role-declared-by-name-and-13-smell-4-closure-layering-inversion-fix-schema--core-dependency-direction-is-now-honest-since-core-is-the-substrate-that-defines-what-plugins-implement-against


**Carry forward**:
- R312 13-smell #5 god-crate decomposition continuation — extract mnemosyne-style (T3/T4 rules) + mnemosyne-workspace (Workspace data type + config orchestrator) + mnemosyne-query (read views + render) from mnemosyne-validator; 5 modules remaining after that round (validator + t2 + code_refs + commit_ledger + lib.rs)
- R313 13-smell #5 final extraction — mnemosyne-validate crate from validator + t2 + code_refs + commit_ledger modules; delete mnemosyne-validator crate entirely at that point; move retained tests to respective domain crates
- R314 13-smell #1 + #2 typed Validator trait + dedup finding — trait Validator with associated type Finding plus ErasedValidator object-safe wrapper for dynamic dispatch through PluginRegistry; retire ValidationFinding stringly-typed extras BTreeMap and CodeRefViolation duplicate representation
- R315 13-smell #8 mnemosyne-mcp library API split — mnemosyne-mcp tool methods call mnemosyne-validate library API directly instead of spawning mnemosyne-cli subprocess
- R316 13-smell #6 + #7 main.rs decomposition — cli commands module split (validate + query + style + append + each cmd_ function into its own module) plus append_changelog_entry 8-arg builder or request struct to retire too_many_arguments per-site allow
- R317 D3 transport abstraction MCP self-ref dogfood — was originally R309 R310 plan; deferred again because transport-on-stringly-typed-boundary would deepen #1 + #2 debt; only enter after R314 typed Validator trait closure
- R318+ D4 MediumAdapter trait plus DesignDocAdapter refactor — Phase 1A prerequisite; medium adapter trait home declared on mnemosyne-core or on a new mnemosyne-medium crate; narrative adapter lands as second impl in Phase 1A
- R319+ D6 external plugin extension mechanism — dlopen libloading dynamic loading or external-binary orchestrator path; large design round
- D7 severity_symbol Mnemosyne self-dogfood — activate plugins.symbol_resolver.rust in mnemosyne.toml plus N round measurement evidence before promotion decision (R263 measure-then-promote pattern)
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger re-measure parameter value times measurability over risk times one plus unmet deps
- 13-smell #4 (layering inversion) full closure — partial closure in R310 via mnemosyne-core rename; full closure requires schema (now in mnemosyne-schema) to not depend on plugin trait surface at all (DecisionStatus from mnemosyne-core is still imported); current dependency direction is honest since mnemosyne-core is by name the substrate
- mnemosyne-validator/tests/ test redistribution — 18 integration tests still hosted in mnemosyne-validator/tests/; redistribute to mnemosyne-schema/tests + mnemosyne-config/tests + mnemosyne-parser/tests + mnemosyne-atomic/tests + mnemosyne-cli/tests (cross-cutting) once R313 deletes mnemosyne-validator crate
- emit_markdown removal API impact — the no-arg convenience function was workspace-coupled to Workspace::MNEMOSYNE_DEFAULT_DOC; callers now use emit_markdown_with_default(doc, None) or pass a workspace-specific default; pre-release no-compat policy applies (no external API to preserve)



### Round 312 — Mnemosyne-validator god-crate decomposition second wave — 3 new crates (mnemosyne-workspace + mnemosyne-style + mnemosyne-query with render submodule) extracted with full consumer migration; validator shrunk from 10 modules to 5; façade-free per CLAUDE no-legacy-carry; 13-smell #5 second-of-three progress (R313 deletes the residual validator crate next)

**Changes**:
- mnemosyne-workspace crate extracted from mnemosyne-validator src/workspace.rs — Workspace data type (multi-doc container with cross-doc resolution + atomic_id_set fallback + MNEMOSYNE_DEFAULT_DOC constant) plus Workspace::from_config + Workspace::mnemosyne + Workspace::insert + Workspace::default_doc_has_section + Workspace::atomic_has_section + Workspace::set_atomic_id_set + Workspace::reclassify_cross_refs methods; depends on mnemosyne-schema (ParsedDoc) plus mnemosyne-config (LoadedConfig)
- mnemosyne-style crate extracted from mnemosyne-validator src/style.rs — T3 plus T4 style rules check_style + default_ruleset + default_ruleset_with_config + glossary_from_config plus StyleRule + StyleScope + StyleSeverity + StyleThreshold + StyleTier + StyleViolation types; depends on mnemosyne-schema (Section + ParsedDoc + ChangelogEntry) plus mnemosyne-atomic (AtomicStore + AtomicSection) plus mnemosyne-config (StyleSection + TerminologySection); style test fixture uses mnemosyne-query render at dev-dep boundary
- mnemosyne-query crate extracted from mnemosyne-validator src/query.rs plus src/render.rs — read-only views (section_by_id (workspace-wide variant) + related_sections + related_sections_with_atomic + changelog_entries_for_section + workspace_section_id_set + query_term + build_envelope + SectionView + ChangelogEntryView + CrossRefView + QueryEnvelope + RelatedSections + TermHit + TermMode + TermQuery + TermScope + TermTargetKind + QueryTermError) plus rendering primitives (render_section + render_changelog_entry + RenderError); render is a sub-module since render is the consumer-facing complement to read views; depends on mnemosyne-schema plus mnemosyne-atomic plus mnemosyne-workspace plus mnemosyne-core (DecisionStatus)
- 3 new workspace members added to root Cargo.toml — mnemosyne-workspace plus mnemosyne-style plus mnemosyne-query
- mnemosyne-validator/lib.rs no longer hosts pub mod workspace + pub mod style + pub mod query + pub mod render and no longer pub use re-exports of those modules items — façade-free per CLAUDE.md no-legacy-carry policy
- mnemosyne-cli Cargo.toml gained dependencies on mnemosyne-workspace plus mnemosyne-style plus mnemosyne-query so cli binary calls the extracted crates directly
- mnemosyne-cli/src/main.rs plus 14 mnemosyne-validator/tests integration tests migrated — 30 use mnemosyne_validator::Workspace + style:: + query:: + render:: sites rewritten to use mnemosyne_workspace::Workspace + mnemosyne_style::* + mnemosyne_query::* + mnemosyne_query::render::* canonical paths
- mnemosyne-validator dev-dependencies gained mnemosyne-workspace plus mnemosyne-style plus mnemosyne-query so 18 retained integration tests still compile in mnemosyne-validator/tests until R313 redistributes them
- mnemosyne.toml [plugins.set_equality_validator].paths gained 3 new crate sources so code-citation defense covers the extracted crates
- mnemosyne-validator src/ shrunk further from 10 modules plus lib.rs to 5 modules plus lib.rs (validator + t2 + code_refs + commit_ledger + lib) — 4 modules plus 2241 lines lifted into the 3 new crates this round; R313 will extract the remaining 4 modules then delete mnemosyne-validator crate entirely
- terminology consistency test in mnemosyne-style/src/lib.rs uses mnemosyne_query::render::render_section at dev-dep boundary — style does not depend on query at runtime



**Verification**:
- cargo build --workspace green across all 16 workspace crates (mnemosyne-store + mnemosyne-facts + mnemosyne-core + mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic + mnemosyne-workspace + mnemosyne-style + mnemosyne-query + mnemosyne-cascade + mnemosyne-server + mnemosyne-cli + mnemosyne-mcp + mnemosyne-validator + mnemosyne-plugin-tree-sitter-rust)
- cargo test --workspace --no-fail-fast green — 82 test result groups all pass with the 3 newly-extracted crates and their dev-dep wiring (mnemosyne-workspace lib tests + mnemosyne-style lib tests with mnemosyne-query render dep + mnemosyne-query lib tests with mnemosyne-parser dev-dep)
- cargo clippy --workspace --all-targets -- -D warnings exits 0 — R308 D9 baseline held under the second wave of god-crate decomposition; no new warnings introduced across the 3 new crates or the touched consumer files
- mnemosyne-cli validate-workspace baseline clean — entries 58 / sections 5 / T3 reject 0 / T1 orphan 0 / round-trip 1/1 / atomic ledger sync / commit-ledger drift 0
- mnemosyne-cli validate-code-refs clean — 0 violations across the 15 scanned paths (now including mnemosyne-workspace/src/ + mnemosyne-style/src/ + mnemosyne-query/src/); §section implementation refs were unaffected because workspace + style + query + render were not bound to §section implementations in the atomic store
- atomic store wire format unchanged — decomposition moved Rust source between crate boundaries only; no schema field renames + no serde attribute changes + no fact bytes layout touched
- runtime dependency graph honored — mnemosyne-query depends on mnemosyne-workspace (read views need a doc-set container) and mnemosyne-workspace depends only on mnemosyne-schema + mnemosyne-config (no cycle); mnemosyne-style depends on mnemosyne-atomic + mnemosyne-schema + mnemosyne-config at runtime and mnemosyne-query only at dev-dep for one terminology test



**Impact**: §generatedmd--atomic-store-derived-view/changelog-atomic-ledger/round-311--mnemosyne-validator-god-crate-decomposition-first-wave--4-leaf-crates-mnemosyne-schema--mnemosyne-config--mnemosyne-parser--mnemosyne-atomic-extracted-with-full-consumer-migration-validator-shrunk-from-15-modules-to-10-faade-free-per-claudemd-no-legacy-carry-13-smell-5-partial-closure-3-more-rounds-r312r313-to-fully-delete-mnemosyne-validator-crate


**Carry forward**:
- R313 13-smell #5 final extraction — mnemosyne-validate crate from validator.rs + t2.rs + code_refs.rs + commit_ledger.rs modules; redistribute 18 integration tests from mnemosyne-validator/tests/ to respective new home crates (atomic_round_trip → mnemosyne-atomic + atomic_store_view_parity → mnemosyne-atomic + symbol_enforcement_smoke + validator_trait_dispatch → mnemosyne-validate + style_* 5 tests → mnemosyne-style + workspace_config_integration → mnemosyne-workspace + cross-cutting tests → mnemosyne-cli); delete crates/mnemosyne-validator/ directory entirely + remove from workspace members
- R314 13-smell #1 + #2 typed Validator trait + dedup finding — trait Validator with associated type Finding plus ErasedValidator object-safe wrapper for dynamic dispatch through PluginRegistry; retire ValidationFinding stringly-typed extras BTreeMap and CodeRefViolation duplicate representation; substrate ready since mnemosyne-validate now hosts the Validator surface
- R315 13-smell #8 mnemosyne-mcp library API split — mnemosyne-mcp tool methods call mnemosyne-validate plus mnemosyne-query plus mnemosyne-atomic library APIs directly instead of spawning mnemosyne-cli subprocess
- R316 13-smell #6 + #7 main.rs decomposition — cli commands module split (validate + query + style + append + each cmd_ function into its own module) plus append_changelog_entry 8-arg builder or request struct to retire too_many_arguments per-site allow
- R317 D3 transport abstraction MCP self-ref dogfood — was originally R309 R310 plan; deferred again because transport-on-stringly-typed-boundary would deepen #1 + #2 debt; only enter after R314 typed Validator trait closure
- R318+ D4 MediumAdapter trait plus DesignDocAdapter refactor — Phase 1A prerequisite; medium adapter trait home declared on mnemosyne-core or on a new mnemosyne-medium crate; narrative adapter lands as second impl in Phase 1A
- R319+ D6 external plugin extension mechanism — dlopen libloading dynamic loading or external-binary orchestrator path; large design round
- D7 severity_symbol Mnemosyne self-dogfood — activate plugins.symbol_resolver.rust in mnemosyne.toml plus N round measurement evidence before promotion decision (R263 measure-then-promote pattern)
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger re-measure parameter value times measurability over risk times one plus unmet deps
- mnemosyne-validator/tests/ test redistribution remains pending — moves to R313 when the validator crate is deleted; current dev-dep wiring (mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic + mnemosyne-workspace + mnemosyne-style + mnemosyne-query) keeps the tests compiling during the transition
- 13-smell #3 AtomicSnapshot eager allocation lazy iterator GAT — defer until ledger entries cross 10K scale threshold; current 58 entries well below hot path concern
- 13-smell #9 doc_lazy_continuation 206 sites blanket allow removal — continuous-improvement; address without blocking on a single round
- 13-smell #10 YyyyMmDd typed newtype — replace inconsistent_digit_grouping blanket allow with strong type at 9 fact sites; mechanical refactor
- 13-smell #11 Box Status tonic interceptor allow — tonic API constraint; remove only when upstream tonic relaxes interceptor trait signature
- 13-smell #12 AtomicSection 14 field data clump analysis — extract Outline title parent_doc parent_section sub-struct candidate; needs cohesion measurement before commit
- 13-smell #13 ValidationContext PluginRegistry reference for multi-validator composition — add when first composition use case materializes



### Round 313 — Mnemosyne-validator god-crate decomposition complete — mnemosyne-validate crate created from final 4 residual modules (validator + t2 + code_refs + commit_ledger); mnemosyne-validator crate deleted entirely; 18 integration tests redistributed to mnemosyne-validate/tests/; 13-smell #5 fully closed (15-module 16804-line god crate replaced by 8 cohesion-driven crates)

**Changes**:
- mnemosyne-validate crate created with the final 4 validator residual modules — validator.rs (T1 cross-ref-orphan + changelog-append-only + frozen-list-membership-delta + section-decision-status-transition + atomic-section-supersede-state) plus t2.rs (T2 frozen-ledger jaccard + atomic frozen check) plus code_refs.rs (R256+ SetEqualityValidator code-citation defense with scan_section_decay + scan_inventory_decay + symbol-mismatch axis + Validator trait impl + tests) plus commit_ledger.rs (commit↔ledger drift report)
- mnemosyne-validator crate deleted entirely — directory crates/mnemosyne-validator/ removed; workspace member entry removed from root Cargo.toml; pre-release no-compat policy applies (no external API to preserve since the residual surface migrated 1-1 into mnemosyne-validate)
- 18 integration tests moved from crates/mnemosyne-validator/tests/ to crates/mnemosyne-validate/tests/ — atomic_round_trip + atomic_store_view_parity + changelog_pattern_plugin + external_fixtures_integration + generated_vs_legacy_audit + phase_1_priority_audit + schema_as_input_integration + self_application_via_generic + self_validation + 6 style_* tests + symbol_enforcement_smoke + validator_trait_dispatch + workspace_config_integration; mnemosyne-validate dev-dependencies span tempfile + mnemosyne-parser + mnemosyne-style + mnemosyne-query + mnemosyne-plugin-tree-sitter-rust so the cross-cutting integration suite still compiles in its new home
- atomic_round_trip.rs path assertion updated — the test's expected file path crates/mnemosyne-validator/src/atomic.rs corrected to crates/mnemosyne-atomic/src/lib.rs (post-R311 atomic crate extraction); two assertion sites fixed
- mnemosyne-validate/src/code_refs.rs SetEqualityValidator plugin_name string updated from "mnemosyne-validator::SetEqualityValidator" to "mnemosyne-validate::SetEqualityValidator" so the plugin registry identifier reflects the canonical crate name
- consumer migration sweep — all mnemosyne_validator (snake) sites replaced with mnemosyne_validate across mnemosyne-cli (main.rs + atomic_cli.rs) + mnemosyne-server + mnemosyne-mcp + the 18 moved tests; all mnemosyne-validator (kebab) Cargo.toml dep entries replaced with mnemosyne-validate
- mnemosyne.toml [plugins.set_equality_validator].paths swap — crates/mnemosyne-validator/src/ entry replaced with crates/mnemosyne-validate/src/ so code-citation defense scans the canonical sources
- atomic-store implementation refs updated — §code-citation-defense plus §code-citation-defense/bidirectional-binding section.implementations rows pointing at crates/mnemosyne-validator/src/code_refs.rs migrated to crates/mnemosyne-validate/src/code_refs.rs via remove-section-implementation plus add-section-implementation primitive pair (preserves audit trail across the move)
- god-crate decomposition #5 fully closed — 8 new cohesion-driven crates (mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic + mnemosyne-workspace + mnemosyne-style + mnemosyne-query + mnemosyne-validate) replace the 15-module 16804-line mnemosyne-validator god crate; workspace now hosts 17 production crates total (3 prior - validator + 8 new = 17 wait 9+8-1=16 actually)
- mnemosyne-cli main.rs reference comment block (the 5-module separation list under the lib.rs doc comment) is now obsolete — the doc-comment description superseded; future R316 cli decomposition round will rewrite the cli docs



**Verification**:
- cargo build --workspace green across all 16 production crates after deleting mnemosyne-validator + creating mnemosyne-validate (mnemosyne-store + mnemosyne-facts + mnemosyne-core + mnemosyne-schema + mnemosyne-config + mnemosyne-parser + mnemosyne-atomic + mnemosyne-workspace + mnemosyne-style + mnemosyne-query + mnemosyne-validate + mnemosyne-cascade + mnemosyne-server + mnemosyne-cli + mnemosyne-mcp + mnemosyne-plugin-tree-sitter-rust)
- cargo test --workspace --no-fail-fast green — 82 test result groups all pass with the 18 redistributed integration tests now running from mnemosyne-validate/tests/; mnemosyne-validator no longer appears in the cargo test target list (deleted)
- cargo clippy --workspace --all-targets -- -D warnings exits 0 — R308 D9 baseline held under the final wave of decomposition; no new warnings introduced across the deleted-validator transition or the consumer name swap
- mnemosyne-cli validate-workspace baseline clean — entries 59 / sections 5 / T3 reject 0 / T1 orphan 0 / round-trip 1/1 / atomic ledger sync / commit-ledger drift 0
- mnemosyne-cli validate-code-refs clean — 0 violations across the 15 scanned paths after §code-citation-defense plus §code-citation-defense/bidirectional-binding implementation refs were migrated from mnemosyne-validator/src/ to mnemosyne-validate/src/ via the atomic-store add-implementation plus remove-implementation primitives
- atomic store wire format unchanged — the validator deletion plus validate creation moved Rust source between crate boundaries only; no schema field renames + no serde attribute changes + no fact bytes layout touched
- mnemosyne-validate dev-dep graph permits the cross-cutting tests to compile — mnemosyne-parser + mnemosyne-style + mnemosyne-query + mnemosyne-plugin-tree-sitter-rust + tempfile dev-deps cover the integration test surface that used to live in mnemosyne-validator/tests/



**Impact**: §generatedmd--atomic-store-derived-view/changelog-atomic-ledger/round-312--mnemosyne-validator-god-crate-decomposition-second-wave--3-new-crates-mnemosyne-workspace--mnemosyne-style--mnemosyne-query-with-render-submodule-extracted-with-full-consumer-migration-validator-shrunk-from-10-modules-to-5-faade-free-per-claude-no-legacy-carry-13-smell-5-second-of-three-progress-r313-deletes-the-residual-validator-crate-next


**Carry forward**:
- R314 13-smell #1 + #2 typed Validator trait + dedup finding — trait Validator with associated type Finding plus ErasedValidator object-safe wrapper for dynamic dispatch through PluginRegistry; retire ValidationFinding stringly-typed extras BTreeMap and CodeRefViolation duplicate representation; substrate ready since mnemosyne-validate now hosts the Validator surface cleanly
- R315 13-smell #8 mnemosyne-mcp library API split — mnemosyne-mcp tool methods call mnemosyne-validate plus mnemosyne-query plus mnemosyne-atomic library APIs directly instead of spawning mnemosyne-cli subprocess (eliminate process fork + arg parsing + JSON round-trip per call)
- R316 13-smell #6 + #7 main.rs decomposition — cli commands module split (cli/commands/{validate + query + style + append + each cmd_ function into its own module}.rs) plus append_changelog_entry 8-arg builder or request struct to retire too_many_arguments per-site allow; mnemosyne-cli/src/main.rs is 1800+ lines and the textbook split is by subcommand
- R317 D3 transport abstraction MCP self-ref dogfood — was originally R309 R310 plan; deferred again because transport-on-stringly-typed-boundary would deepen #1 + #2 debt; only enter after R314 typed Validator trait closure
- R318+ D4 MediumAdapter trait plus DesignDocAdapter refactor — Phase 1A prerequisite; medium adapter trait home declared on mnemosyne-core or on a new mnemosyne-medium crate; narrative adapter lands as second impl in Phase 1A
- R319+ D6 external plugin extension mechanism — dlopen libloading dynamic loading or external-binary orchestrator path; large design round
- D7 severity_symbol Mnemosyne self-dogfood — activate plugins.symbol_resolver.rust in mnemosyne.toml plus N round measurement evidence before promotion decision (R263 measure-then-promote pattern)
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger re-measure parameter value times measurability over risk times one plus unmet deps
- 13-smell #3 AtomicSnapshot eager allocation lazy iterator GAT — defer until ledger entries cross 10K scale threshold; current 59 entries well below hot path concern
- 13-smell #9 doc_lazy_continuation 206 sites blanket allow removal — continuous-improvement; address without blocking on a single round
- 13-smell #10 YyyyMmDd typed newtype — replace inconsistent_digit_grouping blanket allow with strong type at 9 fact sites; mechanical refactor
- 13-smell #11 Box Status tonic interceptor allow — tonic API constraint; remove only when upstream tonic relaxes interceptor trait signature
- 13-smell #12 AtomicSection 14 field data clump analysis — extract Outline title parent_doc parent_section sub-struct candidate; needs cohesion measurement before commit
- 13-smell #13 ValidationContext PluginRegistry reference for multi-validator composition — add when first composition use case materializes (currently no multi-validator scenario)
- post-R313 test redistribution — 18 tests live in mnemosyne-validate/tests/; some are cross-cutting and would conceptually fit other crates (atomic_round_trip → mnemosyne-atomic; style_* → mnemosyne-style; workspace_config_integration → mnemosyne-workspace); leave the current single-location layout pending a future round if test discovery cost shows up in practice
- bench/codegen-prototype/src/query_api.rs doc-comment references mnemosyne-validator by old name twice — pure prose carry from the bench era; not in any scanned path so it does not affect citation defense; clean up if revisiting bench



### Round 314 — Post-decomposition cleanup — 5 residual smells from R311-R313 closed (test redistribution to textbook home crates + mnemosyne-validate dev-dep trim + section_by_id naming collision resolution + cli/main.rs+atomic_cli.rs import reorganization to std/external/internal grouping + bench/codegen-prototype historical doc-rot cleared); repo-wide mnemosyne-validator zero hits; #5 god-crate decomposition now true textbook

**Changes**:
- 16 of 18 cross-cutting integration tests redistributed from mnemosyne-validate/tests/ to their textbook home crate by primary subject — atomic_round_trip + atomic_store_view_parity → mnemosyne-atomic/tests + changelog_pattern_plugin + schema_as_input_integration → mnemosyne-parser/tests + 6 style_* tests → mnemosyne-style/tests + workspace_config_integration → mnemosyne-workspace/tests + 5 cross-cutting orchestrator tests (external_fixtures_integration + generated_vs_legacy_audit + phase_1_priority_audit + self_application_via_generic + self_validation) → mnemosyne-cli/tests; symbol_enforcement_smoke + validator_trait_dispatch remain in mnemosyne-validate/tests since validate is genuinely their primary subject
- mnemosyne-validate dev-dependencies trimmed — mnemosyne-style + mnemosyne-query dropped (their tests moved away); now just tempfile + mnemosyne-parser (for t2.rs internal test fixture) + mnemosyne-plugin-tree-sitter-rust (for symbol_enforcement_smoke); dev-dep count now matches the actual test surface
- mnemosyne-atomic dev-dependencies added mnemosyne-query (atomic_round_trip imports render_changelog_entry + render_section)
- mnemosyne-parser dev-dependencies added tempfile (test fixtures require TempDir)
- mnemosyne-style dev-dependencies added tempfile + mnemosyne-workspace (6 style tests construct a Workspace and use a temp directory) — existing mnemosyne-core + mnemosyne-parser + mnemosyne-query dev-deps retained
- mnemosyne-workspace dev-dependencies added tempfile + mnemosyne-parser + mnemosyne-validate (workspace_config_integration drives parse → workspace → validate path)
- 4 new test directories created — crates/mnemosyne-atomic/tests + crates/mnemosyne-parser/tests + crates/mnemosyne-style/tests + crates/mnemosyne-workspace/tests
- mnemosyne_schema::section_by_id renamed to mnemosyne_schema::sections_by_id_map — the function builds a BTreeMap<section_id, &Section> lookup index and the new name is honest about returning a map; resolves the naming collision with mnemosyne_query::section_by_id which is a workspace-wide find-by-id lookup returning Option<SectionView>; 2 call sites updated (mnemosyne-parser/src/emitter.rs import + use); doc-comment expanded to explain the distinction from the query crate's lookup function
- mnemosyne-cli/src/main.rs import block reorganized into 3 idiomatic Rust groups — std imports first, external-crate imports (anyhow + sha2) second, internal mnemosyne_* imports third in alphabetical crate order with one consolidated use block per crate; duplicate mnemosyne_parser block (originally produced by R311 migration script) collapsed into one
- mnemosyne-cli/src/atomic_cli.rs imports reorganized the same way — std + external + internal-alphabetical with one use block per crate; mnemosyne_atomic 24-symbol import block formatted vertically for readability
- bench/codegen-prototype doc-comment rot cleared — query_api.rs 2 mnemosyne-validator references migrated to mnemosyne-validate (1 site) and mnemosyne-workspace::Workspace (1 site); markdown_import.rs + t1_validator.rs + markdown_full_scale.rs historical mnemosyne-validator mentions updated to mnemosyne-validate
- mnemosyne-core/src/lib.rs doc-comments updated — AtomicStoreView trait doc (line 50) cites mnemosyne-atomic as the producer crate instead of the obsolete mnemosyne-validator; AtomicSnapshot doc (line 69) cites mnemosyne-atomic::AtomicStore instead of mnemosyne-validator::atomic::AtomicStore; DecisionStatus doc (line 95) compares against the schema crate boundary instead of the obsolete validator crate
- mnemosyne-atomic/src/lib.rs in-source test fixture file-path string literals updated from crates/mnemosyne-validator/src/atomic.rs to crates/mnemosyne-atomic/src/lib.rs at 2 assertion sites — these are functional fixture paths the test compares against AtomicSection.implementations entries
- mnemosyne-query/src/render.rs in-source test fixture file paths same correction at 2 sites
- mnemosyne-server/src/gate.rs comment reference to mnemosyne-validator rules updated to mnemosyne-validate rules
- mnemosyne-atomic/tests/atomic_store_view_parity.rs + mnemosyne-cli/tests/self_validation.rs + mnemosyne-style/tests/style_full_scale.rs internal path comments updated to reflect new test home paths



**Verification**:
- cargo build --workspace green across all 16 production crates after test redistribution + dev-dep trim + naming + doc-rot cleanup
- cargo test --workspace --no-fail-fast green — 82 test result groups all pass with the 16 redistributed integration tests now compiling under their textbook home crate dev-dep graphs; mnemosyne-validate retains 2 tests aligned with its actual subject (validator trait dispatch + symbol enforcement)
- cargo clippy --workspace --all-targets -- -D warnings exits 0 — R308 D9 baseline held under the cleanup wave; no new warnings introduced by the test moves + naming + import reorg
- mnemosyne-cli validate-workspace baseline clean — entries 60 / sections 5 / T3 reject 0 / T1 orphan 0 / round-trip 1/1 / atomic ledger sync / commit-ledger drift 0
- mnemosyne-cli validate-code-refs clean — 0 violations; §section implementation refs unaffected (already migrated in R311 + R313 to new crate paths; this round only moved test files which are not in scanned paths)
- repo-wide mnemosyne-validator + mnemosyne_validator grep yields zero hits across .rs + .toml files — all references purged including bench/codegen-prototype historical doc-rot
- atomic store wire format unchanged — cleanup moved Rust source between crate boundaries plus renamed one helper function plus reorganized imports plus updated doc strings; no schema field renames + no serde attribute changes + no fact bytes layout touched
- mnemosyne-validate dev-dep count dropped from 5 to 3 (tempfile + parser + plugin-tree-sitter-rust) reflecting actual test surface; mirrored growth in target crates produces a flat total dev-dep delta but each crate now owns its tests cleanly
- section_by_id naming collision resolved — schema crate function (BTreeMap lookup builder) renamed to sections_by_id_map; query crate function (workspace-wide find-by-id) keeps its original name since it matches the standard find-by-id convention; reader confusion eliminated at the call site
- mnemosyne-cli/src/main.rs + atomic_cli.rs import blocks now follow standard Rust grouping (std + external + internal-alphabetical) with one consolidated use per crate; no duplicate per-crate blocks



**Impact**: §generatedmd--atomic-store-derived-view/changelog-atomic-ledger/round-313--mnemosyne-validator-god-crate-decomposition-complete--mnemosyne-validate-crate-created-from-final-4-residual-modules-validator--t2--code_refs--commit_ledger-mnemosyne-validator-crate-deleted-entirely-18-integration-tests-redistributed-to-mnemosyne-validatetests-13-smell-5-fully-closed-15-module-16804-line-god-crate-replaced-by-8-cohesion-driven-crates


**Carry forward**:
- R315 13-smell #1 + #2 typed Validator trait + dedup finding — trait Validator with associated type Finding plus ErasedValidator object-safe wrapper for dynamic dispatch through PluginRegistry; retire ValidationFinding stringly-typed extras BTreeMap and CodeRefViolation duplicate representation; substrate now hosts Validator trait cleanly in mnemosyne-core and the only consumer is mnemosyne-validate::SetEqualityValidator
- R316 13-smell #8 mnemosyne-mcp library API split — mnemosyne-mcp tool methods call mnemosyne-validate + mnemosyne-query + mnemosyne-atomic library APIs directly instead of spawning mnemosyne-cli subprocess
- R317 13-smell #6 + #7 main.rs decomposition — cli commands module split (cli/commands/{validate + query + style + append + each cmd_ function into its own module}.rs) plus append_changelog_entry 8-arg builder or request struct to retire too_many_arguments per-site allow
- R318 D3 transport abstraction MCP self-ref dogfood — was originally R309 R310 plan; deferred until R315 closure of #1 + #2 to avoid deepening stringly-typed boundary
- R319+ D4 MediumAdapter trait plus DesignDocAdapter refactor — Phase 1A prerequisite
- R320+ D6 external plugin extension mechanism — dlopen libloading or external-binary orchestrator path
- D7 severity_symbol Mnemosyne self-dogfood — activate plugins.symbol_resolver.rust in mnemosyne.toml plus N round measurement evidence
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger
- 13-smell #3 AtomicSnapshot eager allocation lazy iterator GAT — defer until ledger entries cross 10K scale threshold
- 13-smell #9 doc_lazy_continuation 206 sites blanket allow removal — continuous-improvement
- 13-smell #10 YyyyMmDd typed newtype — replace inconsistent_digit_grouping blanket allow with strong type at 9 fact sites
- 13-smell #11 Box Status tonic interceptor allow — tonic API constraint
- 13-smell #12 AtomicSection 14 field data clump analysis — Outline title parent_doc parent_section sub-struct extraction candidate
- 13-smell #13 ValidationContext PluginRegistry reference for multi-validator composition — add when first composition use case materializes



### Round 315 — Typed Validator trait + dedup finding — Validator trait redesigned with associated type Finding (Serialize + Send + Debug) so plugins declare typed payload shapes; ErasedValidator companion trait + blanket impl provide object-safe dispatch through PluginRegistry; ValidationFinding stringly-typed BTreeMap form removed; SetEqualityValidator uses CodeRefViolation as its typed Finding directly with violation_to_finding adapter retired; 13-smell #1 + #2 closed

**Changes**:
- Validator trait redesigned to typed-finding form — added associated type Finding (with Serialize + Send + Debug bounds) so each plugin declares its own rich payload shape (SetEqualityValidator declares Finding = CodeRefViolation; future plugins declare their own typed enums); validate returns Vec<Self::Finding> directly with full static type information preserved across the call boundary
- ErasedValidator companion trait added as object-safe wrapper — fn validate_erased returns Vec<serde_json::Value> by serializing each typed finding at the trait edge; blanket impl<V: Validator> ErasedValidator for V means every typed validator is automatically object-safe-dispatchable
- PluginRegistry storage type changed from Box<dyn Validator> to Box<dyn ErasedValidator> — coercion from Box<V: Validator> is automatic via the blanket impl so register_validator(Box::new(my_validator)) keeps its call syntax; registry.validator(key) now returns &dyn ErasedValidator
- mnemosyne_core::ValidationFinding struct removed entirely — the stringly-typed kind: Option<String> + extras: BTreeMap<String, Value> shape is no longer the trait return value; typed Self::Finding replaces it; pre-release no-compat policy applies (no external API to preserve)
- mnemosyne_core::ValidationFinding-related imports removed from mnemosyne-core src/lib.rs (PathBuf no longer used at substrate level since findings are typed per-plugin)
- SetEqualityValidator impl Validator typed to type Finding = CodeRefViolation — validate returns Vec<CodeRefViolation> directly without the violation_to_finding adapter
- violation_to_finding adapter function removed from mnemosyne-validate/src/code_refs.rs — the typed enum CodeRefViolation already carries entry_id + symbol + decision_status etc as variant payload so the adapter is now redundant; CLAUDE.md no-legacy-carry rule applies
- CodeRefViolation + Citation + ViolationKind gained Serialize derive — default externally-tagged enum form is the auto-derived shape on the ErasedValidator boundary; cli renders a separately-defined flat shape via to_cli_json
- CodeRefViolation gained Display impl — renders the legacy CLI line shape ([<kind>] <file>:<line> <entry_id> for citations; [<kind>] <file>:<no-cite> §<id> (<symbol>) for impl_unbacked; [<kind>] §<id> (status=<status>) for impl_missing) so cli loops can println {} v with no format-string duplication
- CodeRefViolation gained to_cli_json method — produces the stable flat per-violation JSON shape (kind + file + line + section_id + entry_id + symbol + decision_status fields with optionals omitted when absent) that validate-code-refs --json contract emits to external consumers
- mnemosyne-cli/src/main.rs cmd_validate_code_refs refactored to direct typed dispatch — drops the register-then-immediately-retrieve dance with PluginRegistry that was R307 D1 proof and calls validator.validate(&ctx) directly with typed Vec<CodeRefViolation> return; counts loop now uses CodeRefViolation::kind_tag (typed dispatch) instead of f.kind.as_deref string lookup; TTY rendering uses Display via println {} v; JSON rendering uses to_cli_json per violation
- validator_trait_dispatch test rewritten with 3 scenarios — typed_dispatch_yields_typed_findings asserts pattern-match on CodeRefViolation::Citation { kind: Missing, .. } + CodeRefViolation::ImplementationMissing { section_id, decision_status }; erased_dispatch_via_registry_serializes_findings_to_json asserts registry returns Vec<serde_json::Value> with the auto-derived enum shape (Citation discriminator + ImplementationMissing discriminator); typed_dispatch_filter_id_narrows_to_decay_only asserts filter mode returns exactly 1 Citation { kind: Decay }
- registry indirection retained for the cli only at the test layer — production cli path is direct typed; registry exists at substrate level for future dynamic plugin scenarios where dispatcher does not know the concrete validator type (RFC-003 external plugin extension R319+ anchor)



**Verification**:
- cargo build --workspace green across all 16 production crates after typed Validator + ErasedValidator + ValidationFinding removal + cli refactor
- cargo test --workspace --no-fail-fast green — 82 test result groups all pass including the 3 rewritten validator_trait_dispatch tests (typed dispatch + erased dispatch + filter mode) and the unchanged symbol_enforcement_smoke
- cargo clippy --workspace --all-targets -- -D warnings exits 0 — R308 D9 baseline held under the trait redesign; no new warnings introduced by the typed dispatch refactor
- mnemosyne-cli validate-workspace baseline clean — entries 61 / sections 5 / T3 reject 0 / T1 orphan 0 / round-trip 1/1 / atomic ledger sync / commit-ledger drift 0
- mnemosyne-cli validate-code-refs clean — 0 violations across 15 scanned paths; the typed CodeRefViolation dispatch path produces the same per-class counts as the stringly-typed ValidationFinding.kind path that preceded it
- atomic store wire format unchanged — the trait redesign is purely Rust type-level; no schema field renames + no serde attribute changes + no fact bytes layout touched
- two-tier dispatch proof — Validator trait dispatch exercised both via direct typed call (cli production path) and via ErasedValidator object-safe wrapper through PluginRegistry (test path); the registry storage type Box<dyn ErasedValidator> accepts Box<V: Validator> via automatic coercion through the blanket impl
- duplicate finding representation eliminated — CodeRefViolation (rich typed enum) is the single representation; the stringly-typed ValidationFinding parallel form (kind + extras BTreeMap) is gone; substrate has one source of truth for what a SetEqualityValidator finding looks like



**Impact**: §generatedmd--atomic-store-derived-view/changelog-atomic-ledger/round-314--post-decomposition-cleanup--5-residual-smells-from-r311-r313-closed-test-redistribution-to-textbook-home-crates--mnemosyne-validate-dev-dep-trim--section_by_id-naming-collision-resolution--climainrsatomic_clirs-import-reorganization-to-stdexternalinternal-grouping--benchcodegen-prototype-historical-doc-rot-cleared-repo-wide-mnemosyne-validator-zero-hits-5-god-crate-decomposition-now-true-textbook


**Carry forward**:
- R316 13-smell #8 mnemosyne-mcp library API split — mnemosyne-mcp tool methods call mnemosyne-validate + mnemosyne-query + mnemosyne-atomic library APIs directly instead of spawning mnemosyne-cli subprocess (eliminate process fork + arg parsing + JSON round-trip per call)
- R317 13-smell #6 + #7 main.rs decomposition — cli commands module split (cli/commands/{validate + query + style + append + each cmd_ function into its own module}.rs) plus append_changelog_entry 8-arg builder or request struct to retire too_many_arguments per-site allow
- R318 D3 transport abstraction MCP self-ref dogfood — substrate now ready (#1 + #2 closed in R315); mnemosyne-mcp resolve_symbol_at tool + new mnemosyne-plugin-mcp-resolver crate + transport_parity integration test asserting InProcess vs MCP equality
- R319+ D4 MediumAdapter trait plus DesignDocAdapter refactor — Phase 1A prerequisite
- R320+ D6 external plugin extension mechanism — dlopen libloading or external-binary orchestrator path
- D7 severity_symbol Mnemosyne self-dogfood — activate plugins.symbol_resolver.rust in mnemosyne.toml plus N round measurement evidence
- D8 Round 172 priority audit re-validation — at Phase 1 entry trigger
- 13-smell #3 AtomicSnapshot eager allocation lazy iterator GAT — defer until ledger entries cross 10K scale threshold
- 13-smell #9 doc_lazy_continuation 206 sites blanket allow removal — continuous-improvement
- 13-smell #10 YyyyMmDd typed newtype — replace inconsistent_digit_grouping blanket allow with strong type at 9 fact sites
- 13-smell #11 Box Status tonic interceptor allow — tonic API constraint
- 13-smell #12 AtomicSection 14 field data clump analysis — Outline title parent_doc parent_section sub-struct candidate
- 13-smell #13 ValidationContext PluginRegistry reference for multi-validator composition — add when first composition use case materializes
- ErasedValidator default JSON shape — auto-derived externally-tagged enum form is what the trait boundary surface uses; external consumers reading from validate_erased get the verbose shape; cli sticks with to_cli_json flat shape; if a future round wants the boundary shape to match cli flat shape directly, customize CodeRefViolation Serialize impl (trade-off vs auto-derive maintainability)
- Future validator plugins — declare type Finding pointing at the plugin own typed enum (BehavioralFinding for behavioral-checker plugins, NarrativeFinding for narrative-continuity plugins, etc); blanket ErasedValidator impl auto-bridges to PluginRegistry; CLI consumers of new plugins pattern-match on the typed enum directly



### Round 316 — R316 — mnemosyne-mcp library API split: drop CLI subprocess spawn, call mnemosyne-cli ops in-process

**Changes**:
- Added mnemosyne-cli lib target + ops module (mutate via run_atomic_mutate, query, validate, style, docs)
- mnemosyne-mcp links mnemosyne-cli + mnemosyne-atomic; every #[tool] calls a Rust fn, not a forked mnemosyne-cli process
- Deleted mnemosyne-mcp/src/cli.rs subprocess wrapper + the .mnemosyne/tmp write_temp file-passing pattern
- run_atomic_mutate single-sources sidecar resolve + cascade GENERATED.md regenerate for bin and mcp



**Verification**:
- cargo test --workspace green (all suites); cargo clippy --all-targets -D warnings clean
- validate-workspace: T1 orphan=0, round-trip 1/1, T3 reject=0, atomic ledger 62 entries sync
- MCP stdio handshake smoke test: validate_workspace + list_sections return correct data in-process



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- R317: unify cmd_* (cli main.rs + atomic_cli.rs) onto ops, retiring parallel read-path aggregation
- R317: cli main.rs split into commands/ modules + append_changelog_entry 8-arg builder/request struct



### Round 317 — R317 — retire append_changelog_entry 8-arg signature for a named ChangelogEntryDraft struct

**Changes**:
- Replace append_changelog_entry 8 positional args with ChangelogEntryDraft struct
- Retire #[allow(clippy::too_many_arguments)]; named fields kill the swappable-&[String] bug class
- Update all 19 call sites (2 production: atomic_cli + mcp; 17 tests) in same change



**Verification**:
- cargo test --workspace: 670 passed / 0 failed
- cargo clippy --all-targets -D warnings clean (redundant-field-name pass)



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- R317 #6 deferred: cli main.rs (2161 lines) split into commands/ modules
- R316 read-path carry: unify cmd_validate_workspace/cmd_query/cmd_style_check onto ops



### Round 318 — R318 — close MCP validate_workspace gate gap: add supersede + R296 publishable-ledger gates to ops

**Changes**:
- ops::validate_workspace now runs atomic_section_supersede_state_reject (T1 rule 4 atomic axis)
- ops::validate_workspace now runs the R296 publishable/audit divergence ledger gate
- MCP validate_workspace surfaces supersede + publishable-divergence; render_plain prints divergence line



**Verification**:
- MCP stdio validate_workspace prints publishable divergence entries=9 ledger_rows=9 (matches CLI)
- cargo test --workspace 670 pass / clippy -D warnings clean



**Impact**: §atomic-store-mutate-api, §code-citation-defense


**Carry forward**:
- Full single-source: refactor cmd_validate_workspace to delegate to ops (carries CLI-only decay + commit-drift surfaces)
- R317 #6 carry: cli main.rs command-module split still pending



### Round 319 — R319 — extract mnemosyne-ops crate so cli + mcp share one orchestration lib (mcp drops cli/server/store/cascade)

**Changes**:
- New mnemosyne-ops crate: cascade (sidecar/render/regenerate/validate) + query/validate/style/docs ops + run_atomic_mutate
- mnemosyne-mcp depends on mnemosyne-ops not mnemosyne-cli; drops transitive server/store/cascade deps
- atomic_cli sheds 7 moved orchestration helpers, imports them from mnemosyne_ops::cascade; deleted dead RenderedReport



**Verification**:
- cargo test --workspace 670 pass; clippy -D warnings clean; cargo fmt --all --check clean
- cargo tree: mnemosyne-mcp no longer pulls server/store/cascade/cli
- MCP stdio validate_workspace returns correct data in-process post-extraction



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- R320: unify cmd_* onto ops — cmd_validate_workspace still a parallel impl alongside ops::validate_workspace



### Round 320 — R320 — single-source validate-workspace: cmd delegates to ops, delete the 430-line duplicate aggregation

**Changes**:
- cmd_validate_workspace delegates to ops::validate_workspace; keeps only CLI-only decay + commit-drift informational surfaces
- Deleted the duplicate check_publishable_override_ledger gate + dead OrphanKey / KnownStaleOrphan / KNOWN_STALE_ORPHANS
- main.rs 2161 to 1658 lines (-503); validate-workspace stdout byte-identical to pre-R320



**Verification**:
- validate-workspace stdout unchanged; atomic_first_validate_smoke + r280_atomic_path_config_smoke pass
- cargo test --workspace 670 pass; clippy -D warnings clean; cargo fmt --all --check clean



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- Mutate path (cmd_* vs run_atomic_mutate) composes the same single-sourced atomic primitives + cascade::auto_regenerate — not a duplicate algorithm, no unification owed
- D4 MediumAdapter (Phase 1A North-Star gateway) next, on the now-clean ops base



### Round 321 — R321 — drop dead cli commit path (wrote a file-hash to an unread RocksDB store) and retire dead _v2 changelog postfix; substrate crates (store/facts/cascade/server) kept intact, now genuinely orphaned-from-the-live-binary per ARCHITECTURE.md §5

**Changes**:
- Remove cli `commit` subcommand (cmd_commit + dispatch + help + module-doc): it submitted a SHA-256 file-hash through server.submit into RocksDB CfId::Entities that nothing read back — a write-only dead path
- Drop cli deps on mnemosyne-server / mnemosyne-store / sha2 plus their imports; substrate crates (store/facts/cascade/server) kept intact and tested, now built-but-orphaned (zero live callers) matching ARCHITECTURE.md §5
- Rename check_changelog_entry_v2_required to check_changelog_entry_required (def+call) and test fn changelog_entry_v2_frozen_after_append to changelog_entry_frozen_after_append; legacy v1 markdown append already removed so _v2 was a dead postfix
- Fix CLAUDE.md stale `append-changelog-entry-v2` to `append-changelog-entry` (the actual dispatch name)



**Verification**:
- cargo build --workspace + clippy --all-targets (-D warnings) + cargo fmt --all --check all clean
- cargo test --workspace: 86 test groups ok, 0 failed
- validate-workspace: docs 1/1, T1 orphan=0, round-trip 1/1, T3 reject=0, GENERATED.md=sync, commit-ledger drift missing=0



**Impact**: §atomic-store-mutate-api


**Carry forward**:
- extract_v2_* / scan_v2_* test names in code_refs.rs call clean production extract_section_citations (the v2/v3 wrappers were already removed); a test-name purity pass is optional and not a production violation
- ARCHITECTURE.md §5 is now literally accurate (commit stub removed so RocksDB is genuinely built-but-orphaned); foundation convergence A to D remains the next code work



### Round 322 — Convergence A: remove dead Salsa per-entity inputs — Convergence A (fact-model unification per ARCHITECTURE.md §5) begins by removing the three dead salsa::input structs SectionInput/ChangelogEntryInput/FrozenListInput from mnemosyne-cascade: speculative scaffolding with zero constructors anywhere in production, whose presence inflated the documented triplicated-fact-model. Removal follows the no-legacy-carry rule and collapses the Section concept's dead third face without touching the live cascade path.

**Changes**:
- Remove 3 dead per-entity salsa::input structs from mnemosyne-cascade runtime.rs
- Drop their lib.rs re-exports; correct the runtime module doc comment
- Live path (CascadeBranch + BranchSnapshotData) unchanged; no production caller affected



**Verification**:
- cargo test --workspace green (cascade unit + snapshot suites pass)
- validate-workspace green: round-trip 1/1, T3 reject 0, GENERATED.md synced
- grep repo-wide: the 3 inputs had 0 constructors (bench prototype refs independent)




**Carry forward**:
- A1b: hoist bitemporal FactKey envelope into mnemosyne-core; adopt across facts
- A2: canonical Section/ChangelogEntry/FrozenList/CrossRef payloads defined in core
- A3: reconcile live AtomicSection/AtomicChangelogEntry onto canonical core types



### Round 323 — Convergence A: hoist bitemporal FactKey envelope into core — Convergence A continues: introduce a canonical FactKey value object (branch_id, entity_id, valid_from) in mnemosyne-core and adopt it across the three entity facts (SectionFact / ChangelogEntryFact / FrozenListFact) in mnemosyne-facts, replacing the copy-pasted bitemporal-key triple. FactKey is the domain-agnostic composite-identity coordinate the 24-byte RocksDB key encodes; CrossRefFact keeps its distinct relation key (branch_id, from_section, to_section).

**Changes**:
- Add FactKey {branch_id, entity_id, valid_from} value object to mnemosyne-core
- SectionFact/ChangelogEntryFact/FrozenListFact carry key: FactKey (facts -> core edge)
- CrossRefFact unchanged (relation key); byte codec unaffected (key not in value bytes)



**Verification**:
- cargo test --workspace green; facts + cascade fact constructors updated
- validate-workspace green; round-trip 1/1; T3 reject 0; GENERATED.md sync
- clippy --workspace -D warnings + cargo fmt --all --check clean




**Carry forward**:
- A2: core canonical Section skeleton (domain-agnostic) vs design_doc adapter content
- A2 boundary: intent/rationale/inputs/outputs = design_doc-medium fields, not Layer-0
- A3: reconcile live atomic onto canonical core skeleton + adapter content split



### Round 324 — Convergence A2 design: canonical fact-model Layer-0/Layer-1 boundary — Convergence A2 design decision (no code): the canonical core fact is the domain-agnostic SKELETON — FactKey identity, title/parent, decision_status, cross-refs — while the rich design_doc content (intent, rationale, inputs/outputs, caveats, alternatives, examples, normative_excerpt, implementations, publishable_*) is design_doc-MEDIUM-shaped and belongs to the Layer-1 MediumAdapter, not Layer 0. AtomicSection currently conflates both; the A3 code round will split the skeleton into mnemosyne-core and the content into a design_doc adapter, keeping Layer 0 free of medium/spec/code knowledge per the ARCHITECTURE.md §1 invariant and making fiction/ADR media first-class without polluting the core.

**Changes**:
- Decision only: no code this round; defines the A2/A3 canonical-model boundary
- Core skeleton = FactKey + title/parent + decision_status + cross-refs (domain-agnostic)
- design_doc content (intent/rationale/.../implementations) = Layer-1 adapter payload



**Verification**:
- Derived from ARCHITECTURE.md §1 (core domain-agnostic) + §3 (4-layer hexagonal)
- validate-workspace green; round-trip 1/1; GENERATED.md sync
- ARCHITECTURE.md §5 refined with the field-level skeleton/content split




**Carry forward**:
- A3 (code): split AtomicSection into core skeleton + design_doc adapter content
- A3 risk: live workspace.atomic.json serde must stay byte-identical (round-trip)
- B/C/D: RocksDB index from log, cascade incremental projection, unified write path



### Round 325 — Convergence A3: lift Section skeleton into core — Convergence A3 (code): executed the R324 boundary — AtomicSection now embeds the canonical mnemosyne_core::SectionSkeleton (title, parent_doc, parent_section, impact_scope, decision_status) via #[serde(flatten)], so the domain-agnostic Layer-0 skeleton lives in mnemosyne-core while the rich design_doc content stays in the mnemosyne-atomic adapter (Layer 1). FactKey + SectionSkeleton are grouped in a new mnemosyne-core::fact module. On-disk workspace.atomic.json is byte-identical (skeleton flattened first; live sections populate only skeleton + implementations). Still owed: SectionFact (RocksDB index codec) should adopt the same SectionSkeleton (A3b) so the log and the index share one skeleton definition.

**Changes**:
- New mnemosyne-core::fact module: FactKey (moved from lib.rs) + canonical domain-agnostic SectionSkeleton (title, parent_doc, parent_section, impact_scope, decision_status)
- AtomicSection embeds SectionSkeleton via #[serde(flatten)], placed first so flattened skeleton fields serialize ahead of the design_doc content (byte-identical JSON)
- design_doc content (intent/rationale/inputs/outputs/caveats/alternatives/examples/implementations/normative_excerpt) stays in the mnemosyne-atomic adapter = Layer 1
- All skeleton-field call sites across 8 crates (atomic/query/validate/style/cli/ops + 2 test crates) routed through .skeleton



**Verification**:
- cargo test --workspace: 670 passed / 0 failed; cargo clippy --workspace --all-targets clean
- validate-workspace green: docs 1/1, T1 orphan 0, round-trip 1/1, T3 reject 0, GENERATED.md sync
- live workspace.atomic.json sections byte-identical across the split (diff vs pre-split snapshot = empty)




**Carry forward**:
- A3b: SectionFact (RocksDB index codec) should adopt mnemosyne-core::SectionSkeleton so log and index share one skeleton definition; reconcile doc_path/section_id naming
- B/C/D: RocksDB index materialized from the log, cascade incremental projection, unified write path
- Minor cleanup carry: CLI usage string still advertises the removed `commit` subcommand (R321 residue)



### Round 326 — Convergence A: unify Section fact across log and index — Convergence A — the Section fact is now unified across the JSON log and the RocksDB index. SectionFact embeds the canonical mnemosyne_core::SectionSkeleton (R325) behind a full-fidelity byte codec that encodes section_id plus the scalar skeleton (parent_doc, title, parent_section as an Option discriminator, decision_status as a typed-enum discriminator replacing the prior stringly-typed field). Cross-refs were scoped out of the shared skeleton because they are adapter-divergent: the JSON log keeps impact_scope inline on AtomicSection (byte-identical), the index keeps CrossRefFact relation rows. This fulfils convergence A's goal for Section — one SectionSkeleton carries both the serde (log) and the byte codec (index). ChangelogEntry and FrozenList remain on the list.

**Changes**:
- SectionSkeleton scoped to scalars (title/parent_doc/parent_section/decision_status); impact_scope returned to AtomicSection as a direct field, keeping the live JSON byte-identical
- SectionFact = {key, section_id, skeleton: SectionSkeleton}; byte codec encodes the scalars with Option<DecisionStatus> as a discriminator byte (typed enum replaces the prior String)
- Cross-refs left the shared skeleton because they are adapter-divergent: JSON log stores impact_scope inline, index stores CrossRefFact relation rows
- facts re-exports SectionSkeleton/DecisionStatus; cascade fixtures + runtime bridged; fine_grained SectionRecord stays string-typed (bridged) until convergence C



**Verification**:
- cargo test --workspace: 671 passed / 0 failed; cargo clippy --workspace --all-targets -- -D warnings clean
- validate-workspace green: docs 1/1, T1 orphan 0, round-trip 1/1, T3 reject 0, GENERATED.md sync
- live workspace.atomic.json sections byte-identical across the refactor (diff vs pre-refactor snapshot = empty)




**Carry forward**:
- ChangelogEntryFact / FrozenListFact should adopt shared types with their atomic-side counterparts (the remaining A rounds for those entities)
- B/C/D: RocksDB index materialized from the log, cascade incremental projection (SectionRecord adopts the typed DecisionStatus), unified write path



### Round 327 — Convergence A closeout: correct §5 fact-model duplication scope — Correct ARCHITECTURE.md §5 fact-model duplication scope: convergence A is complete (Section was the only entity with genuine cross-face duplication, unified R325/R326); ChangelogEntry and FrozenList reconciliation is B-driven, not a pre-emptive shared skeleton.

**Changes**:
- Rewrote ARCHITECTURE.md §5 table and prose — ChangelogEntry's atomic and fact faces share zero fields, and FrozenList has no atomic representation (frozen-ledger is the FrozenLedger mutate-reject semantic, not a stored entity).
- Recorded that no production code projects the atomic store into the *Fact structs, so the fact model is two unreconciled type definitions, not a live-data duplication.
- Marked convergence A complete — Section was the only entity with genuine cross-face duplication (unified R325/R326) — and elevated B to the active keystone.
- Dropped the prior "ChangelogEntry/FrozenList get the same skeleton treatment" framing as an over-statement; a shared skeleton fits only when both faces already persist identical scalars.



**Verification**:
- Confirmed live atomic store top-level keys are sections, changelog_entries, inventory_entries, schema_version — no frozen-list entity exists.
- Confirmed every SectionFact / ChangelogEntryFact / FrozenListFact construction lives in tests, the persist substrate, or cascade fixtures — no atomic-to-fact projection in production.
- validate-workspace green after the edit — T3 reject=0, round-trip 1/1, GENERATED.md in sync.




**Carry forward**:
- Convergence B is next — write the atomic-to-fact projection that defines ChangelogEntry's canonical scalar shape and wires the orphaned RocksDB index as a derived, rebuildable view.
- ChangelogEntry round_number is currently trapped in the prose entry_id key; B decides whether to surface it as a real field or derive it at projection time.



### Round 328 — Convergence B prerequisite: lift canonical fact structs into core — Lift the 4 canonical fact structs from mnemosyne-facts into mnemosyne-core so Layer 0 owns the one canonical fact model (ARCHITECTURE.md §3); the derived-index byte codec stays in mnemosyne-facts as the new IndexCodec trait, keeping RocksDB out of the canonical model.

**Changes**:
- Moved the 4 fact structs (SectionFact / ChangelogEntryFact / FrozenListFact / CrossRefFact) into mnemosyne-core/src/fact.rs alongside FactKey + SectionSkeleton; core gains no new dependency since the structs are serde-only.
- Converted the inherent encode_value/decode_value methods into a new IndexCodec trait implemented in mnemosyne-facts, keeping the byteorder byte-layout concern in the persistence layer, out of the domain core.
- Re-exported the structs from mnemosyne-facts so cascade and server keep importing the full fact vocabulary from one place; persist.rs imports IndexCodec for the codec calls.



**Verification**:
- cargo check --workspace clean; 671 tests pass (0 failed); cargo clippy --workspace --all-targets -D warnings clean; cargo fmt --all --check clean.
- No production behavior change — the codec byte layout and struct field shapes are identical; only crate residence moved (compiler-driven, byte-for-byte codec preserved).




**Carry forward**:
- B1 next — write the atomic-to-fact projection (AtomicStore to core fact structs), now placeable RocksDB-free since the canonical structs no longer drag mnemosyne-store into the authoring path.
- Remaining facts crate residue (codegen: schema/emit/fixture/canonical = Phase -1A 5-language prototype) is unrelated to the persist binding; flag for a later dead-code review, not in scope here.



