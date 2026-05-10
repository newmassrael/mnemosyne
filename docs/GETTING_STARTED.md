# Getting Started with Mnemosyne (5-minute setup)

Mnemosyne is a markdown spec/doc management infrastructure for LLM-using
codebases (Claude Code / Cursor / Aider / etc.). It treats your spec
documents (`DESIGN.md`, `ARCHITECTURE.md`, ADRs, READMEs) as a typed
workspace with cross-doc reference resolution, append-only changelog
ledgers, and round-trip-stable mutate API.

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

Two optional sections customize behavior. Skip them on first run; defaults
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
```

## 3. Run `validate-workspace`

```bash
cargo run -p mnemosyne-cli -- validate-workspace
```

Output looks like:

```
=== mnemosyne-cli validate-workspace ===
docs=3/3
T1 orphan total=0 (ledger=0, new=+0, resolved=-0)
round-trip mandatory=3/3
style violations: T3 reject=0 / T3 warn=12 / T4 info=4
```

Every doc parses, round-trips through the mandatory schema (Section /
CrossRef / ChangelogEntry / FrozenList), no broken cross-doc references,
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

When the AI agent (or you) wants to add a section, append a changelog
entry, or rewrite a body, use the mutate primitives — they enforce
atomic round-trip, frozen-ledger jaccard checks, and audit append:

```bash
cargo run -p mnemosyne-cli -- append-changelog-entry \
 --doc docs/SPEC.md --entry-id "v1.1" \
 --title "RELEASE-NOTES" --body-file ./entry.md

cargo run -p mnemosyne-cli -- set-section-body \
 --doc docs/SPEC.md --section "3" --body-file ./section3.md

cargo run -p mnemosyne-cli -- add-section \
 --doc docs/SPEC.md --title "New decision" --numbered-id "12" \
 --body-file ./body.md
```

Every mutate command emits a `MutateReceipt` with affected docs,
validator-path invocations, and round-trip diff count (must be `0`).
Failures roll back automatically — your file is never left half-written.

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

The MCP server already exposes the verification primitives:

- `list_sections` returns every section_id, including changelog entry
 ids like `round-254--<slug>`. Have the agent call this once at
 session start and cache the set, then prefix-match `round-NNN--`
 before writing any `Round NNN` citation.
- `query_section(section_id)` returns the SectionView with
 `decision_status`. Use this to distinguish Active from Superseded
 entries; only Active entries should be cited without explicit
 historical-reference framing.

Add a one-line rule to your project's `CLAUDE.md` (or equivalent agent
instruction file) telling the agent to verify before citing. Mnemosyne's
own project `CLAUDE.md` carries the example pattern under the *Citation
hygiene* section.

This is **Stage 1** of a three-stage defense. Stage 2 (pre-commit gate
that rejects missing or superseded citations) and Stage 3 (cascade
decay scan when an entry transitions to Superseded) ship in subsequent
rounds; until then, agent-side verification at write time is the
primary protection.

## What's next

- **Schema customization**: see [SCHEMA_GUIDE.md](SCHEMA_GUIDE.md).
- **Architecture**: see [DESIGN.md](DESIGN.md) §15 (runtime SDK), §39
 (graph schema), §66 (self-application).
- **Roadmap**: see [ROADMAP.md](ROADMAP.md) — Phase 0e generic library
 extraction (Round 141-151) is the closure round; Phase 1+ adds branch /
 bi-temporal / cascade / saga + the narrative product surface (Novel /
 TRPG / Wiki / Game adapters).
