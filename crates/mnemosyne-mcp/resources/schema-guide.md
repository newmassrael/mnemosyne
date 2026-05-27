# `mnemosyne.toml` Schema Guide

A project adopts Mnemosyne by placing `mnemosyne.toml` at the workspace
root. Discovery walks upward from CWD looking for the file, identical
to git's pattern.

## Minimal config

```toml
[workspace]
docs = ["docs/GENERATED.md"]
default_doc = "docs/GENERATED.md"

[schema]
changelog_titles = ["Changelog"]
entry_id_prefix = "Round "

[style]
locale = "en"
```

This is the form a typical Mnemosyne-native project uses (atomic store
+ GENERATED.md as sole source of truth).

## Multi-doc config (legacy markdown carry)

```toml
[workspace]
docs = [
    "ARCHITECTURE.md",
    "README.md",
    "docs/spec.md",
    "docs/protocol.md",
]
default_doc = "ARCHITECTURE.md"

[schema]
changelog_titles = ["Changelog", "Change History"]
entry_id_prefix = "RFC-"
```

For projects that have multiple existing markdown docs and want to
adopt Mnemosyne incrementally without collapsing them into one
GENERATED.md.

## Sections

### `[workspace]`

| Key | Type | Required | Meaning |
|---|---|---|---|
| `docs` | list of paths | yes | docs to validate, relative to workspace root |
| `default_doc` | path | yes | cross-doc fallback target for `§N` lookups |
| `root` | path | no | override workspace root (relative to config file) |

### `[schema]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `changelog_titles` | list of strings | `["Changelog"]` | heading titles that open a `## Changelog` block |
| `entry_id_prefix` | string | `"Round "` | prefix that opens a ChangelogEntry top bullet |

Common prefix presets:

- `"Round "` — Mnemosyne self-application
- `"ADR-"` — Architectural Decision Records
- `"RFC-"` — RFC-style projects
- `"OQ-"` — Open Questions logs

### `[style]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `locale` | string | `"en"` | sentence-boundary handler |
| `thresholds.max_paragraph_length` | int | 1000 | T3 warn |
| `thresholds.max_sentence_length` | int | 200 | T3 warn |
| `thresholds.max_section_body_length` | int | 5000 | T4 info |
| `thresholds.boilerplate_repetition_jaccard` | float | 0.7 | T3 |

### `[terminology]`

```toml
[terminology.glossary]
"Salsa" = ["salsa"]
"bi-temporal" = ["bitemporal"]
```

Maps a canonical form to a list of non-canonical variants for the
`terminology_consistency` rule. Empty section disables the rule.

### `[atomic]` (optional)

| Key | Type | Default | Meaning |
|---|---|---|---|
| `sidecar_path` | path | `"docs/.atomic/workspace.atomic.json"` | atomic store JSON location |

### `[[orphan_ledger]]` (Round 253 + 254)

Per-workspace registration of known-stale cross-refs. Each row is a
table of arrays. `reason` is required — silent suppression is not
allowed.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `kind` | enum | `"markdown_ref"` | `markdown_ref` / `atomic_entry_ref` / `atomic_section_ref` (Round 254) |
| `doc` | string | (required) | source doc path (workspace-relative); `"<atomic-changelog>"` or `"<atomic-section>"` for atomic kinds |
| `from` | string | (required) | section_id (or entry_id for `atomic_entry_ref`) authoring the ref |
| `to` | string | (required) | section_id the ref points to (without leading `§`) |
| `reason` | string | (required) | why this orphan is acceptable; for scope-correction carry, point at the Round entry |
| `since` | string | (required) | when registered (free-form date or round id) |

**Round 253** introduced `markdown_ref` kind — markdown body cross-ref
orphans (e.g. cross-doc placeholder targets pending authoring). The
ledger composes (set-union) with the binary's `KNOWN_STALE_ORPHANS`
const for self-application.

**Round 254** added `atomic_entry_ref` and `atomic_section_ref` kinds
to cover atomic-internal orphans introduced by Round 169 dogfood-switch
ratify. Use these when a doc/section removal from `workspace.docs`
leaves prior `ChangelogEntry.impact_refs` or `Section.impact_scope`
pointing at now-missing atomic IDs. See `frozen-ledger` and
`anti-patterns` concepts for the textbook scope-correction path.

```toml
# Markdown body orphan (Round 253 default)
[[orphan_ledger]]
doc = "ARCHITECTURE.md"
from = "11/11.5"
to = "6.2.6"
reason = "Cross-doc to RFC §6.2.6, target pending authoring"
since = "2026-05-08"

# Atomic-internal ChangelogEntry impact_ref orphan (Round 254)
[[orphan_ledger]]
kind = "atomic_entry_ref"
doc = "<atomic-changelog>"
from = "Round 1"
to = "watching-zenoh"
reason = "Round 7 scope correction: README removed from workspace.docs"
since = "Round 7"

# Atomic-internal Section impact_scope orphan (Round 254)
[[orphan_ledger]]
kind = "atomic_section_ref"
doc = "<atomic-section>"
from = "some-section/sub-id"
to = "removed-target-id"
reason = "Round N scope correction; target was in removed doc"
since = "Round N"
```

Set-equality drift catch: `validate-workspace` reports `new=+M` when a
new orphan appears (must be ledgered or fixed) and `resolved=-K` when
a ledgered ref is later fixed in source (entry should be deleted from
the ledger).

## Conventions for new projects

If you're standing up a new Mnemosyne project, these defaults work:

```toml
[workspace]
docs = ["docs/GENERATED.md"]
default_doc = "docs/GENERATED.md"

[schema]
changelog_titles = ["Changelog"]
entry_id_prefix = "Round "

[style]
locale = "en"
```

Build content by calling `set_section_*` and `append_changelog_entry`
tools — never by hand-editing the JSON or GENERATED.md.

## Heading convention

The parser recognizes two numbered forms for top-level sections:

- `## 1. Title` (numeric prefix + dot)
- `## §1 Title` (section symbol + numeric)

Both produce `section_id = "1"`. Pick one and stay consistent within a
doc. Existing docs that use `## §1` form parse correctly without
config changes.

## Validation

After authoring `mnemosyne.toml`, run `validate_workspace`. The first
run reports your baseline (existing orphans, style warnings). From
that baseline, mutations are evaluated incrementally — only *new*
violations cause failures.
