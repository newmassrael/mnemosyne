# Getting Started with Mnemosyne (5-minute setup)

Mnemosyne is a markdown spec/doc management infrastructure for LLM-using
codebases (Claude Code / Cursor / Aider / etc.). It treats your spec
documents (`DESIGN.md`, `ARCHITECTURE.md`, ADRs, READMEs) as a typed
workspace with cross-doc reference resolution, append-only changelog
ledgers, and a typed mutate API over a single directly-validated store.

This guide walks you from a fresh checkout to your first
`validate-workspace` pass in five minutes.

## 1. Install (one-time)

Clone the repo and build the CLI:

```bash
git clone https://github.com/<your-fork>/mnemosyne.git
cd mnemosyne
cargo build --workspace --release
```

The CLI binary lands at `target/release/mnemosyne-cli`. For the rest of
this guide we use the `cargo run` form so you don't need the binary on
PATH.

## 2. Author a `mnemosyne.toml`

Drop this at the root of your project (the directory that will become
your workspace root — typically the repo root):

```toml
[workspace]
docs = [
 "docs/SPEC.md",
 "docs/ARCHITECTURE.md",
 "README.md",
]
default_doc = "docs/SPEC.md"
```

The doc paths are relative to the directory the `mnemosyne.toml` lives
in. `default_doc` is the cross-doc reference target — when one doc
mentions `§3` and `§3` doesn't exist locally, the parser looks it up
under `default_doc` and reclassifies the reference as cross-doc.

Optional sections customize behavior. Skip them on first run; defaults
work for design-doc / spec / RFC / ADR style markdown:

```toml
[schema]
changelog_titles = ["Changelog"] # heading titles that open a ledger
entry_id_prefix = "v"  # e.g. v1.0.0 / v1.1 ...

[style]
locale = "en"
[style.thresholds]
max_sentence_length = 250

[terminology.glossary]
"JWT" = ["jwt", "Jwt"]
"OAuth" = ["oauth", "Oauth"]

# Opt into the code-citation defense — see §7 below.
[code_refs]
paths = ["src/"]
severity_missing = "warn"
severity_binding = "warn"
comment_only = true
```

## 3. Run `validate-workspace`

```bash
cargo run -p mnemosyne-cli -- validate-workspace
```

Output looks like:

```
=== mnemosyne-cli validate-workspace ===
T1 orphan total=0 (ledger=0, new=+0, resolved=-0)
style violations: T3 reject=0 / T3 warn=12 / T4 info=4
atomic ledger: entries=42 / sections=18
```

The store holds the typed records (Section / CrossRef / ChangelogEntry /
FrozenList) with no broken cross-doc references,
and the style summary is informational unless the deterministic
`terminology_consistency` rule fires (which rejects).

## 4. Query a section

The query API lets an AI agent (Claude Code / Cursor / etc.) read your
spec without grepping markdown:

```bash
cargo run -p mnemosyne-cli -- query "3"
cargo run -p mnemosyne-cli -- query "3" --include-related
cargo run -p mnemosyne-cli -- query "3" --include-changelog --json
```

`§3` (or just `3`) returns the section body, parent, status, optional
related-cross-ref subsection, optional changelog entries that mention it,
and JSON envelope output suitable for piping into an agent context.

## 5. Mutate a section through the API

When the AI agent (or you) wants to set a field on a section, append a
changelog entry, or record a code binding, use the **atomic mutate
primitives** — they write to the atomic store and run T1+T2 + the
author-time guards (e.g. T1 rule 4: a section marked Superseded must
name its successor) before persisting:

```bash
# Set a typed Section field (intent / rationale / inputs / outputs / etc.)
cargo run -p mnemosyne-cli -- set-section-intent \
 --section §3 --intent "Authentication boundary for browser clients."

cargo run -p mnemosyne-cli -- set-section-rationale \
 --section §3 --bullet "Cookie-based session preferred over JWT for SSR." \
 --bullet "Refresh-token rotation handled server-side."

# Bind §3 to a code file (code-citation defense Path B). --kind is
# explicit: `implements` (= «satisfy», counts as coverage) or
# `references` (= «trace», defends the cite without a fulfillment claim).
cargo run -p mnemosyne-cli -- add-section-binding \
 --section §3 --file src/auth/session.rs --symbol Session::validate \
 --kind implements

# Mark a section Superseded — `--superseding` is mandatory (T1 rule 4).
cargo run -p mnemosyne-cli -- set-section-decision-status \
 --section §3 --status superseded --superseding §12

# Append a structured changelog entry (atomic, audit-trail).
cargo run -p mnemosyne-cli -- append-changelog-entry-v2 \
 --entry-id "Round 8" --decision "Adopt Argon2id over bcrypt" \
 --changes-file ./round8-changes.txt \
 --verification-file ./round8-verification.txt \
 --impact §3,§7 --carry-file ./round8-carry.txt
```

Every mutate command emits a `MutateReceipt` with the primitive name,
target id, sidecar path, and written bytes. The sidecar JSON is the single
directly-validated artifact — read it back with `mnemosyne-cli query`.
Failures roll back atomically — the sidecar JSON is never left
half-written.

## 6. Install pre-commit hook (optional)

```bash
./scripts/install-hooks.sh
```

This drops a generic pre-commit hook that runs `validate-workspace` on
every `git commit` whose staged set touches a tracked doc. The hook reads
the doc list from `mnemosyne.toml` via `mnemosyne-cli list-docs`, so
adopting a new doc only needs a `mnemosyne.toml` edit.

## 7. LLM agent citation hygiene

When you wire an LLM coding agent (Claude Code / Cursor / Aider) to your
mnemosyne workspace via the MCP server, the agent will reference your
spec entries by id (e.g. `Round 254`, `§42`) in the code, comments, and
commit messages it generates. Hallucinated references — entry ids that
do not exist, or that point to a Superseded decision — are silent
corruption of the audit trail. No compiler catches it; `git blame`
chases the wrong rationale.

Mnemosyne ships a **three-stage defense**, all active by default once
`[code_refs]` is configured in `mnemosyne.toml`:

**Stage 1 — agent-side verification at write time** (MCP):

- `list_sections` returns every section_id, including changelog entry
 ids like `round-254--<slug>`. The agent should call this once at
 session start and cache the set, then prefix-match `round-NNN--`
 before writing any `Round NNN` citation.
- `query_section(section_id)` returns the SectionView with
 `decision_status`. Use this to distinguish Active from Superseded
 entries.

**Stage 2 — `validate-code-refs` reject gate**:

```bash
cargo run -p mnemosyne-cli -- validate-code-refs
```

Scans the paths listed in `[code_refs].paths`, extracts `Round NNN` /
`§N` tokens from comments (`comment_only = true`), and rejects any
citation whose target is missing from the atomic store. Wired into the
pre-commit hook via `scripts/install-hooks.sh`. Promote `severity_*`
from `warn` to `reject` once your baseline is clean.

**Stage 3 — cascade decay scan**:

When a section transitions to `Superseded` or `Removed` via
`set-section-decision-status`, the cascade trigger runs a
`§<id>` scan over `[code_refs].paths` and prints citing locations
to stderr. `validate-workspace` reports the workspace-wide decay
surface as an informational line. Stale citations surface immediately;
the agent's next session can refresh them.

Add a one-line rule to your project's `CLAUDE.md` (or equivalent agent
instruction file) telling the agent to verify before citing. Mnemosyne's
own project `CLAUDE.md` carries the example pattern under the *Citation
hygiene* section.

## What's next

- **Schema customization**: see [SCHEMA_GUIDE.md](SCHEMA_GUIDE.md) — every
 `mnemosyne.toml` field, with presets.
- **Design history**: read it with `mnemosyne-cli query` — Mnemosyne's own
 design decisions live in the atomic store changelog (`Round 252+` is the
 Phase 0 hardening arc).
- **Roadmap**: tracked in the atomic store changelog. Phase 1 (narrative
 medium adapter) is registered as a deferred carry behind Phase 0
 stabilization.
