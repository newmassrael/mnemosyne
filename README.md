# Mnemosyne

> Design-doc lifecycle infrastructure for LLM-driven projects.
> [한국어 README](README.ko.md)

Markdown design docs become unsafe when an AI agent edits them
directly: a regex collapses bullet structure, a heading rename silently
breaks 200 cross-refs, an "improvement" rewrites a frozen ledger entry
and history is lost.

Mnemosyne replaces that fragile surface with **atomic-store + GENERATED.md**:

- The atomic store (`docs/.atomic/workspace.atomic.json`) is the source
  of truth — typed records (Section / ChangelogEntry / FrozenList /
  CrossRef) with append-only audit semantics.
- `docs/GENERATED.md` is the sole human-readable artifact, deterministically
  rendered from the store.
- Every mutation routes through a typed primitive that runs T1
  (cross-ref orphan reject) and T2 (frozen-ledger jaccard) before
  persisting.

**Status:** Phase 0 production stack (6 crates). 59 test suites green.
1 commit on `main` — squashed history is intentional during Phase 0.

## Components

| Crate | Role |
|---|---|
| `mnemosyne-validator` | Parser / emitter / T1+T2 / round-trip |
| `mnemosyne-store` | RocksDB CF layout |
| `mnemosyne-core` | Typed-fact bridge |
| `mnemosyne-cascade` | Salsa cascade queries |
| `mnemosyne-server` | gRPC + audit append surface |
| `mnemosyne-cli` | Production CLI (validate / mutate / generate-docs) |
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
```

Then:

```bash
mnemosyne-cli validate-workspace
```

This surfaces your baseline: T1 orphan total, round-trip mandatory
status, T3/T4 style violations. From that baseline, mutations are
evaluated incrementally.

See [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) and
[docs/SCHEMA_GUIDE.md](docs/SCHEMA_GUIDE.md) for the full walkthrough.

## Using Mnemosyne with AI agents (MCP)

`mnemosyne-mcp` is a Model Context Protocol server. AI clients
(Claude Code, Cursor, Cline, Continue, Copilot Chat, …) connect over
stdio and gain:

- **15 typed tools** — validate / query / 9 atomic mutate primitives.
  Each tool's args are JSONSchema-validated before reaching the
  validator.
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

The lifecycle has four nodes:

```
typed mutate primitive ──► atomic store JSON ──► tera render ──► GENERATED.md
        │                                                             │
        └────────── round-trip: parse(emit) == typed_facts ───────────┘
```

A typical mutation flow:

1. The author or AI calls a typed primitive
   (e.g. `set_section_intent`).
2. The primitive runs T1 (cross-ref orphan reject) and T2 (frozen
   ledger jaccard) before any write.
3. On accept, the atomic store JSON is written via temp file + atomic
   rename.
4. Cascade auto-update: a tera template renders the store back to
   `docs/GENERATED.md`.
5. The round-trip invariant — `parse(emit(typed_facts)) ==
   typed_facts` — is rechecked on every subsequent
   `validate-workspace` call.

Read paths skip parsing entirely — `query-section` returns SectionView
JSON straight from the atomic store.

Whether a tool is invoked by the CLI, the MCP server, or a pre-commit
hook, the same code path runs (parse + emit + T1 + T2 in
`mnemosyne-validator`). One implementation, three entry surfaces.

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
```

## Design Considerations

The major shape decisions and the alternatives examined. Useful when
adopting Mnemosyne in a project that has its own opinions about doc
management.

### Why atomic store + GENERATED.md, not raw markdown

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
re-render." A single JSON file written via temp + atomic rename
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
  that §40 exists, atomically updates every relevant cross_ref,
  re-renders GENERATED.md.

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

### Why round-trip equality is the spine

The contract: `parse(emit(typed_facts)) == typed_facts`.

Without it, the atomic store and `GENERATED.md` drift, and any
pre-commit hook eventually misclassifies. The Round 67 sub-section
prefix bug surfaced exactly this way: the parser produced section_id
`60/1` for a nested numbered heading, but the emitter wrote bare
`1.`, so re-parsing yielded a different id and the diff broke. The
fix preserved the parent prefix on the last segment. Mechanical
hygiene that hand-written tests rarely catch.

### Closed-form schema in Phase 0

The four entity kinds (Section / ChangelogEntry / FrozenList /
CrossRef) are closed-form. User-defined kinds, additional entities,
and schema extensions are explicitly not Phase 0 features — that
work belongs to Phase 1.5+ schema decomposition (a separate spec
round).

Closing the schema in Phase 0:

- Simplifies the validator (no plugin loader path).
- Keeps round-trip provability tractable.
- Makes 5-language emit (Rust + Kotlin + Python + C++ + Protobuf)
  feasible. Salsa cascade semantics remain Rust-only because porting
  the incremental-computation guarantees to other languages was
  judged out of paradigm.

## Documentation

- [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) — 5-minute setup walkthrough.
- [docs/SCHEMA_GUIDE.md](docs/SCHEMA_GUIDE.md) — every `mnemosyne.toml` field, with presets.
- [docs/GENERATED.md](docs/GENERATED.md) — generated from the atomic
  store; the project's own design-doc dogfood.
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

Mnemosyne is in Phase 0 — design-doc lifecycle. The longer arc extends
the same atomic-store + frozen-ledger guarantees to other
markdown-shaped media:

- **Phase 1 (deferred): narrative medium adapter.** A fictional /
  creative-writing extension — game scripts, character bibles,
  worldbuilding logs — under the same AI-mutation safety contract.
  The priority audit ranked this as the first Phase 1 entry.
  Currently deferred behind completion of the legacy markdown
  migration carry.
- **Phase 1.5: cascade-gate full-scale measurement.** Validation that
  the per-record Salsa cascade pattern scales to the §11 50K-asset
  workload at the published p95 budget.

These items are *registered carries*, not commitments. Phase 0 stack
stability is the gating criterion; the codebase is honest about what
works today versus what is named in the audit ledger.

## License

Dual-licensed under MIT or Apache-2.0 at your option.
