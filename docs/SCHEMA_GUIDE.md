# Schema Guide — customizing Mnemosyne for your codebase

Mnemosyne ships with a **fixed primitives** schema (Section / CrossRef /
ChangelogEntry / FrozenList) and a **markdown pattern → entity** mapping
that is fully config-driven via `mnemosyne.toml`. This guide shows the
knobs available and the three reference presets.

## Schema schema

Every Mnemosyne workspace is configured by one TOML file. The top-level
tables:

```toml
[workspace]  # required
docs = [...]   # ordered list of doc paths
default_doc = "docs/SPEC.md" # cross-doc reference target

[schema]   # optional — defaults to mnemosyne preset
changelog_titles = [...]  # which heading titles open a ledger
entry_id_prefix = "Round "  # what opens a ledger entry bullet
anchor_convention = "section_number"
medium_name = "design_doc"

[style]   # optional — defaults to compile-time
locale = "ko"   # sentence-boundary locale tag
[style.thresholds]
max_paragraph_length = 1000
max_sentence_length = 300
max_section_body_length = 5000

[terminology.glossary] # optional — defaults to Mnemosyne preset
"Salsa" = ["salsa"]
"bi-temporal" = ["bitemporal"]
```

Omit any optional table to inherit the Mnemosyne preset.

## The three reference presets

`SchemaSection` exposes three factory presets that cover the common
markdown conventions:

| Preset | `changelog_titles`  | `entry_id_prefix` | `anchor_convention` | Use when    |
| ----------------- | ----------------------------------- | ----------------- | ------------------- | --------------------------------------------- |
| `mnemosyne_preset` | `["Changelog", "changelog"]` | `"Round "` | `"section_number"` | English design-doc style (this repo)  |
| `generic_default` | `["Changelog", "changelog"]` | `""` (disabled) | `"heading_slug"` | English README + ARCHITECTURE.md w/o numbered ledger |
| `adr_preset` | `["Decisions"]`   | `"ADR-"`  | `"adr_id"`  | Architectural Decision Records (`ADR-NNNN`) |

Empty `entry_id_prefix` disables changelog entry capture entirely — useful
when the project never numbers its history rows.

## What each field actually does

- **`changelog_titles`** — heading titles that put the parser in
 changelog-bullet mode. The parser falls back to a case-insensitive
 match against the literal `changelog`, so `## CHANGELOG` always works
 even if you forget to list it.
- **`entry_id_prefix`** — the literal string that opens a changelog
 bullet. The parser captures digits + dot-separator chain immediately
 after the prefix and emits `entry_id = "{prefix}{digits}"`. Empty
 prefix disables capture; bullets in the changelog section become
 free-form text.
- **`anchor_convention`** — diagnostic label only in the current round
 (Round 144). The parser still derives `section_id` from numbered headings
 (`## 7.` → `"7"`) and slugifies unnumbered titles.
- **`medium_name`** — diagnostic label that flows through MutateReceipt
 + tracing spans for cross-medium debugging. No semantic effect.
- **`style.locale`** — selects the sentence-boundary handler. `"ko"` is
 the current default; `"ja"`/`"zh"`/`"en"` are placeholders that
 currently fall back to the `.`/`!`/`?` terminator set. Deeper locale
 wiring (`。` for Japanese / Chinese, abbreviation handling for German
 / French) is scheduled for a follow-up round.
- **`style.thresholds`** — per-rule char count overrides. Keys must
 match `StyleRule.rule_id` (`max_sentence_length`, `max_paragraph_length`,
 `max_section_body_length`). Unrecognized keys are silently ignored.
- **`terminology.glossary`** — canonical → variants map. The
 `terminology_consistency` rule rejects (T3) when a non-canonical
 variant appears in any doc body.

## Common authoring patterns

### English design doc (this repo)

Just author `mnemosyne.toml` and commit. The Mnemosyne preset is the
default — your spec passes through unchanged, and the validator
recognizes `Round N` entries under `## Changelog`.

### English-only README + ARCHITECTURE.md, no numbered ledger

```toml
[workspace]
docs = ["README.md", "ARCHITECTURE.md"]
default_doc = "README.md"

[schema]
changelog_titles = ["Changelog"]
entry_id_prefix = ""  # no numbered entries
medium_name = "generic"
```

### ADR-style decisions (`ADR-NNNN`)

```toml
[workspace]
docs = ["docs/adr/ADR-0001.md", "docs/adr/ADR-0002.md", "README.md"]
default_doc = "docs/adr/ADR-0001.md"

[schema]
changelog_titles = ["Decisions"]
entry_id_prefix = "ADR-"
anchor_convention = "adr_id"
medium_name = "adr"
```

### Project-specific terminology rules

```toml
[terminology.glossary]
"JWT" = ["jwt", "Jwt", "JsonWebToken"]
"OAuth2" = ["oauth2", "OAUTH2"]
"Postgres" = ["postgres", "POSTGRES", "PostgreSQL"]
```

The `terminology_consistency` rule rejects (T3, blocks commit via
pre-commit hook) when any variant appears anywhere a doc body — the
canonical form is the only valid spelling.

## What stays fixed

The four entity types — Section / CrossRef / ChangelogEntry / FrozenList
— are **not** configurable. They are the universal primitives the
validator + mutate API + cascade engine all build on. What `[schema]`
configures is *which markdown patterns* the parser maps onto these
primitives. Adding a fifth entity type is a Phase 1+ narrative-product
concern (medium-specific entities like `Character` / `Location` /
`Faction` carry off the four primitives via custom relations).
