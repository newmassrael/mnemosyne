---
name: mnemosyne-init
description: Bootstrap a project for Mnemosyne — the atomic store is the SSOT (structured decisions + facts + audit trail). Writes a mnemosyne.toml that parses, runs the first validate-workspace baseline, wires up the mnemosyne-mcp MCP server, and points the agent at the live authority for what the store can hold. Use when the user says "set up Mnemosyne", "adopt Mnemosyne for this project", "initialize Mnemosyne", "configure mnemosyne.toml", or asks how to start using Mnemosyne in a project.
tools: Read, Write, Edit, Glob, Grep, Bash
---

# mnemosyne-init

Bootstrap a project for Mnemosyne adoption: the mechanical setup, so the
user can spend their attention on the conceptual choices.

**This file is TRACKED in the Mnemosyne repo and covered by its gates**
(`crates/mnemosyne-cli/tests/docs_match_reality_smoke.rs`): every
`mnemosyne-cli <verb>` below must dispatch, and every `mnemosyne.toml`
block below must parse through the real `parse_config`. It lived outside
the repo until Round 640, where no gate could see it, and drifted into
teaching a deleted artifact and emitting a config that could not parse —
do not move it back out.

## What Mnemosyne actually is (say this, not the old model)

The **atomic store** is the single directly-validated SSOT: structured
sections, changelog entries, and — on the narrative side — frames,
entities, branches, predicates, facts, and disclosure plans. It is a
sidecar JSON mutated **only** through the CLI/MCP mutate API, never by
hand.

There is **no generated markdown design-doc**: the markdown-doc model (the
`docs` / `default_doc` config, the parser/emitter, the generated doc
itself) was removed in Round 400. Humans read the store through `query`;
the store is what gets validated.

**Do not enumerate the store's capabilities from memory — they move.** The
authority is the installed binary:

```bash
mnemosyne-cli describe-schema
```

That prints the live contract: registries, the fact shape, the typed-claim
leg, the fixed vocabularies, the write-time invariants, and the manifest
wire format with a worked example. Read it before telling the user what
Mnemosyne can or cannot hold. A consumer once concluded that seven present
capabilities were absent because the front door listed them from a
hand-maintained list instead (Round 620); this skill is downstream of that
lesson.

## When to invoke

- User asks to adopt / set up / initialize Mnemosyne in a project.
- User wants the MCP server registered for their AI client.

## Prerequisites — verify first

```bash
which mnemosyne-cli && which mnemosyne-mcp
```

If either is missing, install from a Mnemosyne checkout:

```bash
cargo install --path crates/mnemosyne-cli --force
cargo install --path crates/mnemosyne-mcp --force
```

## Workflow

### 1. Check whether the project is already set up

```bash
ls -la mnemosyne.toml .git 2>&1
```

If `mnemosyne.toml` exists, **stop** — show the user the file and offer to
re-validate instead.

### 2. Detect the entry-id convention

The store recognizes changelog entries by a configured prefix. Sample the
project's existing markdown for one:

```bash
grep -rhE "^#{1,3} (Changelog|Change History|Decisions|Open Questions)" --include="*.md" . 2>/dev/null | head
grep -rhE "^- (Round |ADR-|RFC-|OQ-|Decision )" --include="*.md" . 2>/dev/null | head
```

| Project style | `entry_id_prefix` | `changelog_titles` |
|---|---|---|
| ADR-style | `"ADR-"` | `["Decisions"]` |
| RFC-style | `"RFC-"` | `["Changelog"]` |
| Round-numbered (Mnemosyne native) | `"Round "` | `["Changelog"]` |
| Open-question log | `"OQ-"` | `["Open Questions"]` |
| No convention yet | `"Round "` | `["Changelog"]` |

If detection is ambiguous, ask the user. This names the convention only —
it imports nothing.

### 3. Author `mnemosyne.toml`

Write it at the project root. Start minimal — **every section except
`[workspace]` is optional, and the defaults suit design-doc / spec / RFC /
ADR projects**:

```toml
[workspace]

[atomic]
sidecar_path = ".atomic/store.atomic.json"

[schema]
changelog_titles = ["Changelog"]
entry_id_prefix = "Round "
```

`[workspace]` takes only `root` (a workspace-root override) and
`[workspace.spec_source]` (external-spec mirror provenance) — **nothing
else**. Every section is `deny_unknown_fields`, so a stray or misspelled
key REJECTS at load instead of being silently ignored (Round 605).
Relative paths resolve against the `mnemosyne.toml`'s own directory, or
against `root` when it is set.

For the optional sections (`[style]`, `[continuity]`, `[plugins]`,
`[orphan_ledger]`, …) read the schema guide — via MCP once the server is
wired (`mnemosyne://concepts/schema-guide`), or `docs/SCHEMA_GUIDE.md` in
a Mnemosyne checkout. **Do not guess key names**; a wrong one fails the
load.

### 4. Run the baseline validation

```bash
mnemosyne-cli validate-workspace
```

Report what it prints back to the user **without judgment** — it is a
baseline, not a grade. Read the metrics off the actual output rather than
from a list here: the surfaces it reports have changed before, and a stale
transcription is worse than none.

The one thing worth explaining: **T1 orphan total is the starting line.**
New mutations are judged incrementally against it, so a large first number
is normal for an existing project and is not a defect to go fix.

### 5. Wire up the MCP server

Detect the user's client, or ask which one:

```json
{
  "mcpServers": {
    "mnemosyne": {
      "command": "mnemosyne-mcp",
      "args": ["--workspace", "<absolute-path-to-project>"]
    }
  }
}
```

**Don't write to the user's MCP config yourself** — show the snippet and
ask them to add it. Most clients need a restart afterward.

### 6. Point the agent at the concept resources

Once the server is registered, the AI client can read:

- `mnemosyne://concepts/overview`
- `mnemosyne://concepts/atomic-store`
- `mnemosyne://concepts/frozen-ledger`
- `mnemosyne://concepts/tier-rules`
- `mnemosyne://concepts/anti-patterns`
- `mnemosyne://concepts/schema-guide`
- `mnemosyne://concepts/workflow`

Reading these is how the agent learns the frozen-ledger and tier-rule
semantics. For what the store can *hold*, the authority is
`describe-schema` (above), not these prose docs.

## Anti-patterns this skill avoids

- ❌ Writing `mnemosyne.toml` with hardcoded values without detecting the
  project's actual convention.
- ❌ Mass-editing existing markdown to fit Mnemosyne shape on day 1.
  Adoption is incremental: the first run records the baseline, then new
  mutations build forward.
- ❌ Editing the atomic sidecar JSON directly. It is mutated only through
  the mutate API (CLI verbs / MCP tools); a hand edit bypasses every
  write-time invariant.
- ❌ Telling the user what Mnemosyne supports from memory. Run
  `describe-schema` and read the live contract.

## Output shape (for the user)

Finish with a 3-line status:

```
mnemosyne.toml: written at <path>
validate-workspace: exit <code> — <the metrics it actually printed>
MCP server: <registered|snippet shown>
```
