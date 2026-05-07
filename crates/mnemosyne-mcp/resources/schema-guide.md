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

Build content by calling `set_section_*` and `append_changelog_entry_v2`
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
