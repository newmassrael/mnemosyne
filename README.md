# Mnemosyne

> Integrity infrastructure for AI-mutated markdown — spec, code citations, and (eventually) narrative.
> [한국어 README](README.ko.md)

When an AI agent edits markdown directly, three failure modes appear
that no compiler catches:

- A regex meant to fix `§3` matches inside a code fence and corrupts an
  unrelated example.
- A heading rename silently breaks 200 cross-refs scattered across
  other docs.
- An "improvement" rewrites a frozen ledger entry — the decision history
  that explained *why* the system is shaped this way is gone.

These hazards extend *outward* the moment your codebase starts citing
the spec. A comment that reads `// see Round 254 for the rationale` is
load-bearing documentation; once `Round 254` is renamed, deleted, or
superseded, that comment lies — and `git blame` will chase the wrong
rationale forever. The same applies to narrative documents: a character
bible whose eye-color note in chapter 2 contradicts chapter 15 is the
same class of integrity break, just in a different medium.

**Mnemosyne replaces these fragile surfaces with a typed, bi-directional integrity stack.**

- The **atomic store** (`docs/.atomic/workspace.atomic.json`) is the
  single, directly-validated source of truth — typed records (Section /
  ChangelogEntry / FrozenList / CrossRef) with append-only audit semantics.
- Humans read the design history through `mnemosyne-cli query`; for spec
  content the SSOT is a committed **EPUB** (the `normative_excerpt` text is
  a revalidated projection of it). The markdown-render model (the old
  `GENERATED.md`) was removed in Round 400. AI writes through typed
  primitives.
- Every mutation routes through a typed primitive that enforces the atomic
  invariants (cross-ref orphan reject, append-only / frozen-ledger) before
  persisting.
- **Code citations** of spec ids (`§3`, `Round 254`) are scanned at
  commit time; hallucinated or superseded references are rejected
  before they reach git history.
- **Section ↔ Implementation bindings** record which source files own
  each decision. When a spec section is renamed or superseded, the
  citing code locations surface automatically.

**Status:** Phase 0 hardening. 500+ tests green. Mnemosyne dogfoods itself
— its own design history lives in the atomic store at
`docs/.atomic/workspace.atomic.json`, read via `mnemosyne-cli query`.

## What Mnemosyne actually protects

Mnemosyne enforces **three integrity boundaries**. Each one corresponds
to a class of bug that AI-mediated authoring creates and that
hand-written review usually misses.

### 1. Document ↔ document (T1 cross-ref orphan reject)

Cross-references between sections never dangle. If `§3` in
`docs/SPEC.md` references `§42`, but `§42` doesn't exist — neither
intra-doc, nor in the default cross-doc target, nor in the atomic store
— the mutation that introduced that reference is rejected at write
time. Renaming `§3` automatically updates every cross_ref pointing to
it, atomically.

**What this catches:** "I told the AI to rename §3 → §4, it did a regex
replace, and now eight unrelated docs have broken refs."

### 2. Document ↔ history (T2 frozen-ledger jaccard)

Once a `ChangelogEntry` is committed, its `sub_bullets` are append-only.
A subsequent mutation that *removes* a bullet from a frozen entry fails
the jaccard-inclusion check (current ⊇ previous). The audit trail
becomes provably immutable without relying on git history (which file
renames, squash-merges, and cherry-picks routinely break for
decision-tracking purposes).

**What this catches:** "The AI 'improved' the changelog wording and now
I don't know what we actually decided in Round 17."

### 3. Document ↔ code (Path B bidirectional binding + code-citation defense)

Every spec `Section` can record `implementations = [(file, symbol), ...]`
— the source code that *owns* that decision. The
`validate-code-refs` pass then walks the configured production source
paths and extracts `§<id>` / `Round NNN` citations from comments. Three
classes of defect are rejected:

- **`Missing`** — citation references a section/entry id that doesn't
  exist in the atomic store (hallucination).
- **`CitationUnbound`** — citation appears in a file that the
  referenced section's `implementations` list does *not* claim as a
  binding. Either the section's binding list is stale, or the citing
  comment is misplaced — both are real defects, surfaced
  symmetrically.
- **`ImplementationMissing`** — an Active section has zero
  `implementations` recorded. "Active" means "this decision is backed
  by code"; a section with no recorded backing breaks that contract.

Pre-commit hooks wire all three into a reject gate. Renaming or
superseding a spec section runs a cascade scan that prints every citing
code location to stderr — stale citations surface immediately.

**What this catches:** "The agent left a `// see Round 254` comment in
auth.rs after we renamed Round 254 to Round 256 last month, and
nothing flagged it for six weeks."

## Components

| Crate | Role |
|---|---|
| `mnemosyne-atomic` | Atomic store (the JSON SSOT) + mutate primitives |
| `mnemosyne-query` | Read projections (query / report-*) over the store |
| `mnemosyne-validate` | Citation / coverage / drift validation |
| `mnemosyne-store` | RocksDB CF layout (derived index) |
| `mnemosyne-core` | Typed-fact bridge |
| `mnemosyne-cascade` | Salsa cascade queries |
| `mnemosyne-server` | gRPC + audit append surface |
| `mnemosyne-cli` | Production CLI (validate / mutate / query) |
| `mnemosyne-mcp` | Model Context Protocol server for AI clients |

## Quick start (CLI)

```bash
git clone https://github.com/newmassrael/mnemosyne
cd mnemosyne
cargo install --path crates/mnemosyne-cli --force
cargo install --path crates/mnemosyne-mcp --force
```

In your project root, author `mnemosyne.toml`:

```toml
[workspace]
docs = ["ARCHITECTURE.md", "docs/spec.md"]
default_doc = "ARCHITECTURE.md"

[schema]
changelog_titles = ["Changelog"]
entry_id_prefix = "Round "

[style]
locale = "en"

# Optional — opt into the code-citation defense (rejects hallucinated
# §id / Round-N references in your source comments). R306 renamed this
# table to the plugin substrate namespace; behavior is unchanged.
[plugins.set_equality_validator]
paths = ["src/"]
severity_missing = "warn"   # promote to "reject" once your baseline is clean
severity_binding = "warn"
comment_only = true
```

Then:

```bash
mnemosyne-cli validate-workspace   # T1 orphans + atomic ledger + style
mnemosyne-cli validate-code-refs   # citation defense (if [plugins.set_equality_validator] configured)
```

This surfaces your baseline: T1 orphan total, T3/T4 style violations,
atomic ledger sync, plus any spec-id citations in source that no longer
resolve. From that baseline,
mutations are evaluated incrementally.

See [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) and
[docs/SCHEMA_GUIDE.md](docs/SCHEMA_GUIDE.md) for the full walkthrough.

## Using Mnemosyne with AI agents (MCP)

`mnemosyne-mcp` is a Model Context Protocol server. AI clients
(Claude Code, Cursor, Cline, Continue, Copilot Chat, …) connect over
stdio and gain:

- **16 typed tools** — validate / query / 12 atomic mutate primitives
  (Section + ChangelogEntry typed-field setters). Each tool's args are
  JSONSchema-validated before reaching the validator.
- **7 concept resources** under `mnemosyne://concepts/*` — overview,
  atomic-store, frozen-ledger, tier-rules, anti-patterns,
  schema-guide, workflow. AI clients auto-load these so the agent
  internalizes Mnemosyne's semantics before mutating.

### Register the MCP server in a project

Drop a `.mcp.json` at the project root:

```json
{
  "mcpServers": {
    "mnemosyne": {
      "command": "mnemosyne-mcp",
      "args": ["--workspace", "."]
    }
  }
}
```

Restart your AI client. On first invocation it will prompt to approve
the server; once approved, the agent can call tools and read
concept resources without further setup.

### Onboarding flow for collaborators

When a teammate clones a project that already has `.mcp.json` +
`mnemosyne.toml`, they only need:

```bash
cargo install --path /path/to/mnemosyne/crates/mnemosyne-cli --force
cargo install --path /path/to/mnemosyne/crates/mnemosyne-mcp --force
```

The next time their AI client opens the project, it picks up
`.mcp.json` automatically. Pre-built binaries via `cargo-dist` are
planned for a future release.

## How It Works

The atomic store JSON is the single directly-validated artifact:

```
typed mutate primitive ──► atomic store JSON  (the single SSOT)
                                  │
                  mnemosyne-cli query / report-*  (read projections)
```

A typical mutation flow:

1. The author or AI calls a typed primitive
   (e.g. `set_section_intent`).
2. The primitive enforces the atomic invariants (cross-ref orphan
   reject, append-only / frozen-ledger) before any write.
3. On accept, the atomic store JSON is written via temp file + atomic
   rename.
4. `validate-workspace` rechecks the store invariants (T1 orphans,
   citation hygiene, drift) on every subsequent call.

The markdown-render model (a tera template → `GENERATED.md`, gated by a
`parse(emit) == typed_facts` round-trip) was removed in Round 400: the
store is the single directly-validated SSOT, humans read the design
history via `mnemosyne-cli query`, and spec content lives in a committed
EPUB. Read paths return SectionView JSON straight from the store.

Whether a tool is invoked by the CLI, the MCP server, or a pre-commit
hook, the same code path runs (the typed mutate primitives in
`mnemosyne-atomic` + validation in `mnemosyne-validate`). One
implementation, three entry surfaces.

## CI integration

In CI you don't need MCP — just the CLI:

```yaml
# .github/workflows/mnemosyne.yml
on: [push, pull_request]
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install --git https://github.com/newmassrael/mnemosyne mnemosyne-cli
      - run: mnemosyne-cli validate-workspace
      - run: mnemosyne-cli verify-generated
      - run: mnemosyne-cli validate-code-refs   # optional, requires [plugins.set_equality_validator] in mnemosyne.toml
```

The same three commands plus a `cargo clippy --workspace --all-targets`
gate are wired into the tracked `.githooks/` directory. Install per
clone with:

```bash
git config core.hooksPath .githooks
```

Three hooks then run automatically:
- `pre-commit` — code-citation defense, workspace validate (when the
  atomic sidecar is staged), and fmt + clippy (when `.rs` is staged).
- `commit-msg` — enforces `COMMIT_FORMAT.md` (subject ≤ 72 bytes,
  body ≤ 72 bytes per line, 1–3 bullets, English + typographic
  whitelist).
- `pre-push` — re-runs `validate-workspace` + clippy before
  publishing, catching state drift since the last `pre-commit`.

Once the citation-defense baseline is clean, promote `severity_*`
from `warn` to `reject` in `mnemosyne.toml` and the hook will block
any commit that introduces a hallucinated spec citation.

## Design Considerations

The major shape decisions and the alternatives examined. Useful when
adopting Mnemosyne in a project that has its own opinions about doc
management.

### Why the atomic store, not raw markdown

A pure markdown surface exposes three structural failure modes to AI
agents:

- A regex meant to fix `§3` accidentally matches inside a code fence.
- A heading rename silently invalidates two hundred cross-refs.
- An "improvement" commits a rewrite of a frozen ledger entry and
  history is gone.

The typed atomic store collapses each into a mechanical reject:

- T1 — a non-existent `§N` target is rejected at write time.
- A heading rename routes through `set_section_*` which atomically
  updates every cross_ref pointing to it.
- T2 — a sub_bullet removal is rejected by jaccard inclusion.

### Why a single JSON file instead of a database

Considered: RocksDB, sled, LMDB, XTDB, Datomic. The Phase -1A
measurement spike (under `bench/`) confirmed that RocksDB CF + 24 B
fixed-width composite keys hits the §3 SLA budget for the per-fact
layer.

For the **workspace-scope** atomic store (Section + ChangelogEntry
typed facts), a full database buys nothing — the workspace is small,
and the access pattern is "load whole file → mutate once →
save." A single JSON file written via temp + atomic rename
covers the use case.

RocksDB is still wired in Phase 0 for the **audit-trail layer**:
`mnemosyne-cli commit` records design-doc commit transactions to
RocksDB column families under `.mnemosyne/store/`. The full per-branch
fact layer that exercises the §4 ten-CF schema at the 50K-asset
workload is Phase 1+ scope. The validate / mutate / render paths used
day-to-day touch only the JSON file; RocksDB activates on `commit`.

### Why frozen ledger instead of git history

Git tracks *file* changes. Frozen ledger tracks *decision* changes.
The two are not the same:

- File renames lose the git history of decisions inside the file.
- Squash-merging collapses individual decision commits.
- Cherry-picking re-orders decisions arbitrarily.

The ChangelogEntry sequence is ordered by `entry_id` monotonicity and
re-validated at every mutation. Stronger than git for the audit-trail
use case.

### Why typed primitives instead of LSP-style text edits

LSP edits operate on text ranges. Mnemosyne's primitives operate on
typed fields. The difference matters when one logical change touches
many regions:

- LSP rename `§39 → §40`: author writes a regex and hopes it's
  correct.
- Mnemosyne `set_section_impact_scope(target=§40)`: validator checks
  that §40 exists and atomically updates every relevant cross_ref in
  the store.

Cost: mutations must go through the typed API. Benefit: the
"regex matched the wrong thing" class of bugs is eliminated by
construction.

### Why MCP for the AI integration surface

Considered: custom JSON-RPC, gRPC, vendor-specific extensions, plain
CLI calls. MCP won on three points:

- It is a cross-vendor standard (Claude Code, Cursor, Cline,
  Continue, Copilot Chat all speak it).
- Tool arguments are JSONSchema-validated at the protocol layer.
- Resources auto-load concept docs into the agent's context, so the
  agent learns the rules before mutating.

The `mnemosyne-mcp` server wraps the production CLI, keeping the
validation logic single-source.

### Why Salsa for cascade queries

Considered: Differential Dataflow, Adapton, manual invalidation.
Salsa won on:

- Field-level dependency tracking (the Round 92 fine-grained layer).
- Byte-equal memoization stability across processes.
- Compile-time `#[salsa::input/tracked/db]` integration that keeps
  cascade definitions close to the query bodies.

Phase 1.5 cascade-gate full-scale measurement (50K asset workload)
will validate that the per-record pattern scales to the §11 SLA
budget.

### Closed-form schema in Phase 0

The four entity kinds (Section / ChangelogEntry / FrozenList /
CrossRef) are closed-form. User-defined kinds, additional entities,
and schema extensions are explicitly not Phase 0 features — that
work belongs to Phase 1.5+ schema decomposition (a separate spec
round).

Closing the schema in Phase 0:

- Simplifies the validator (no plugin loader path).
- Keeps the typed-fact model and validation tractable.
- Makes 5-language emit (Rust + Kotlin + Python + C++ + Protobuf)
  feasible. Salsa cascade semantics remain Rust-only because porting
  the incremental-computation guarantees to other languages was
  judged out of paradigm.

## Documentation

- [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) — 5-minute setup walkthrough.
- [docs/SCHEMA_GUIDE.md](docs/SCHEMA_GUIDE.md) — every `mnemosyne.toml` field, with presets.
- `mnemosyne-cli query` — the project's own design history (the atomic
  store changelog); spec content lives in a committed EPUB.
- [CLAUDE.md](CLAUDE.md) — Claude Code guidance for working *on*
  Mnemosyne itself.
- [COMMIT_FORMAT.md](COMMIT_FORMAT.md) — commit message convention.

For AI agents already inside an MCP session, the canonical onboarding
order is:

1. `mnemosyne://concepts/overview`
2. `mnemosyne://concepts/anti-patterns`
3. `mnemosyne://concepts/atomic-store`
4. `mnemosyne://concepts/frozen-ledger`
5. `mnemosyne://concepts/tier-rules`
6. `mnemosyne://concepts/workflow`

## Roadmap

Mnemosyne's core abstraction — *AI-mutated markdown documents need typed
invariants to stay safe* — generalizes well beyond design docs. The
roadmap follows that generalization outward: same primitives (Section /
CrossRef / ChangelogEntry / FrozenList), same integrity guarantees
(T1 / T2 / Path B), different schemas on top.

### Phase 0 — Design-doc lifecycle (current)

Production dogfood. Mnemosyne's own design history runs through the
atomic store; the hardening arc spanning Round 252-272 closed the core
integrity gaps:

- T1 cross-doc orphan reject with `[[orphan_ledger]]` opt-in carries
  for legitimate legacy references.
- Atomic-axis `decision_status` field with author-time + validate-time
  guards (T1 rule 4 across both axes).
- Code-citation defense reject mode (`severity_missing` /
  `severity_binding` = `reject`) gating pre-commit on hallucinated
  spec references.
- Bidirectional Spec ↔ Code binding via `Section.bindings` (typed
  trace-link edges: `kind = implements` «satisfy» / `references` «trace»)
  and three-edged set-equality detection
  (`CitationUnbound` + `ImplementationUnbacked` + `ImplementationMissing`,
  the last counting only `implements` as coverage).
- Atomic ChangelogEntry mutate API (append-only audit half + a separate
  publishable view) — the single directly-validated SSOT.

### Phase 1 — Narrative medium adapter

The next adoption surface: long-form fiction, game scripts, TRPG
campaign notes, worldbuilding wikis, character bibles. These media
share the same AI-mutation hazard pattern that motivated Phase 0 —
LLM-driven editing breaks invariants that no compiler enforces — but
the schema and the primitives change.

Concrete target genres and what Mnemosyne would guard:

- **Long-form fiction draft management.** A character's
  established eye color in chapter 2 must match chapter 15. A renamed
  faction shouldn't leave 40 orphan references in unrelated scenes.
  The atomic-store + T1 invariants lift directly — what changes is
  the entity schema (Character / Location / Faction / Scene) and the
  mutate primitives (`set_character_eye_color`,
  `rename_faction_with_cascade`).
- **Game scripts (interactive fiction, dialog trees, branching
  narrative).** Branch targets must resolve. Character dialog schemas
  must stay consistent across scenes. Conditional flag references
  (`if metPirateKing`) cannot dangle. Same T1 cross-ref orphan reject,
  applied to scene graphs instead of section graphs.
- **TRPG campaign notes.** NPC stat blocks, location backstory, plot
  beat audit trail. The GM's "what did I rule three sessions ago"
  problem is exactly the frozen-ledger problem: git history doesn't
  carry decision provenance, but a ChangelogEntry stream sorted by
  session number does.
- **Worldbuilding wikis.** Faction relations, timeline consistency,
  magic-system constraints. References between articles need orphan
  reject; "law of magic" changes need frozen-ledger semantics so
  retroactive edits don't quietly contradict ten earlier chapters.
- **Character bibles.** Name spelling normalization, age/timeline
  arithmetic, relationship graph consistency. Identical hazards to a
  design doc, different fields on the underlying schema.

The Phase 1 priority audit (Round 172) ranked fictional adapter as the
first Phase 1 entry by a 6.00 / 3.00× margin over alternatives —
chosen because (a) the AI-mediated authoring workflow already exists
in this space, (b) the per-asset count fits the workspace-scope JSON
store without database migration, and (c) the integrity-break failure
modes are visible to end users (a reader notices when a character's
eye color contradicts the bible) which keeps the validator's reject
mode well-calibrated.

Phase 1 is currently *deferred* behind Phase 0 stack stabilization —
not abandoned. The roadmap is honest about the boundary.

### Phase 1 — External-spec compliance adapter (parallel candidate)

A second Phase 1 adoption surface, registered after a 2026-05 RFC from
a statechart-compiler project tracking the W3C SCXML Recommendation +
IRP test catalog. The schema differs from the narrative-medium adapter,
but the substrate is identical: atomic store + T1 cross-ref reject +
Path B bidirectional binding + code-citation defense lift directly.

Concrete target surfaces and what Mnemosyne would guard:

- **External standards conformance tracking.** W3C / IETF RFC / IEEE /
  ISO/IEC / AUTOSAR specs vendored as a workspace mirror, with code
  citations checked against the canonical section graph. Section text
  is mutable (reflects current spec revision); the audit trail of
  revision bumps lives in ChangelogEntry stream.
- **Test catalog hygiene.** W3C SCXML IRP, IETF interop test suites,
  internal conformance test rosters — each test id as a Section with
  status (`active` / `deprecated` / `reserved`) and lifecycle audit,
  reusing the InventoryEntry primitive (Phase 1A) where the test id
  shape fits.
- **Normative excerpt embedding.** The vendored spec quote anchored to
  the section, so reviewers verify citations against the exact text
  the compiler was built against — independent of upstream HTML rot.
- **Spec revision drift detection.** Fetch → hash diff → impact report
  for code citations affected by an upstream rev bump.

Schema requirements identified by the RFC:

- New `AtomicSection.normative_excerpt` field (vendored spec quote with
  anchor URL + source revision pin).
- Workspace-level `spec_source` metadata (origin URL, fetched revision,
  fetched_sha256, fetched_at).
- Symbol-level binding enforcement (`Implementation.symbol` participates
  in set-equality, gated by an opt-in flag — requires language-aware
  citation extraction beyond the current regex/comment-only pipeline).

These are *not* Phase 0 features (closed-form schema policy below).
They land as part of the Phase 1.5 schema-decomposition spec round,
where they share design pressure with the narrative adapter's entity
extensions (Character / Location / Faction / Scene). Two parallel
adoption axes hitting the same decomposition mechanism is the
calibration signal — if the mechanism stays generic under both, it is
not over-fit to either.

The narrative adapter remains the *first* Phase 1 entry by the Round
172 priority audit (6.00 / 3.00× margin). The external-spec adapter
lands second, *on top of* the schema-decomposition mechanism the
narrative adapter introduces.

Until Phase 1.5 ships, external-spec adopters can carry ~80% of the
target functionality via sidecar JSON (normative excerpts + spec
provenance) alongside a vanilla atomic store. The full disposition is
in `claudedocs/mnemosyne-rfc-002-sce-response.md`.

### Phase 1.5 — Cascade-gate full-scale measurement

Validation that the per-record Salsa cascade pattern (currently used
at workspace scope) scales to the 50K-asset workload at the published
p95 budget. Substrate carried from the Phase -1A measurement spike
(under `bench/`, retained as historical baseline). This is the
infrastructure prerequisite for any narrative-medium adapter that
manages a novel-scale (~50K facts) workspace efficiently.

### What's not on the roadmap

These items are *registered carries* in the audit ledger, not
commitments. Phase 0 stack stability is the gating criterion. The
codebase deliberately separates "what works today and is dogfooded"
from "what is named in the priority audit" — there is no implication
that a registered carry will ship on any particular timeline.

## License

Dual-licensed under MIT or Apache-2.0 at your option.
