# Schema Guide — customizing Mnemosyne for your codebase

Mnemosyne ships with a **fixed primitives** schema (Section / CrossRef /
ChangelogEntry / FrozenList) and a **markdown pattern → entity** mapping
that is fully config-driven via `mnemosyne.toml`. The atomic store
(`docs/.atomic/workspace.atomic.json`) is the single source of truth for
typed facts; `docs/GENERATED.md` is the deterministic human-readable
view. This guide shows the knobs available and the reference presets.

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

[code_refs]   # optional — opt into code-citation defense
paths = ["src/"]
severity_missing = "warn"  # | "reject"
severity_binding = "warn"  # | "reject"
comment_only = true

[[orphan_ledger]]  # optional — register legacy cross-ref carries
doc = "docs/legacy.md"
from = "12"
to = "99"
kind = "markdown_ref"   # | "atomic_entry_ref" | "atomic_section_ref"
reason = "carried from pre-migration state, see Round 80"
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
- **`[code_refs].paths`** — production source paths to scan for spec
 citations (`Round NNN`, `§<id>`). Test paths intentionally excluded —
 traceability anchors in tests carry a different policy contract than
 rationale in production source. Typically `["src/"]` or
 per-crate `["crates/foo/src/", "crates/bar/src/"]`.
- **`[code_refs].severity_missing`** — `warn` or `reject`. Fires when a
 citation's target id is absent from the atomic store (hallucination).
 Start at `warn` to surface the baseline, promote to `reject` once
 clean.
- **`[code_refs].severity_binding`** — `warn` or `reject`. Fires when a
 citation appears in a file that the section's
 `implementations` (Path B Spec ↔ Code binding) does *not* list as a
 backing implementation, *or* when an Active section has zero
 implementations recorded. Bidirectional binding integrity.
- **`[code_refs].comment_only`** — `true` strips string literals before
 scanning so only comment citations count. Default `true`; flip only if
 your project deliberately puts §id references in user-visible strings.
- **`[[orphan_ledger]]`** — register legitimate cross-ref carries
 (e.g. references to legacy docs you preserved by design). Each entry
 names `doc` / `from` / `to` / `kind` / `reason`. The validator's
 `T1 orphan total` line reports `ledger=N, new=+X, resolved=-Y`:
 ledgered orphans are silent, new orphans bail, and resolved-but-still-
 ledgered entries bail too (forcing you to drop the now-stale ledger
 row).

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

### Code-citation defense (multi-crate workspace)

```toml
[code_refs]
paths = [
 "crates/foo/src/",
 "crates/bar/src/",
]
severity_missing = "reject"
severity_binding = "reject"
comment_only = true
```

Run `mnemosyne-cli validate-code-refs` to scan; wire into pre-commit
via `scripts/install-hooks.sh`. Promote `severity_*` from `warn` to
`reject` once the baseline is clean. To bind a section to its
implementation file (so the binding axis recognizes it as backed), use:

```bash
mnemosyne-cli add-section-implementation \
 --section §3 --file crates/foo/src/auth.rs --symbol Session::validate
```

## What stays fixed

The four entity types — Section / CrossRef / ChangelogEntry / FrozenList
— are **not** configurable. They are the universal primitives the
validator + mutate API + cascade engine all build on. What `[schema]`
configures is *which markdown patterns* the parser maps onto these
primitives. Adding a fifth entity type is a Phase 1+ narrative-product
concern (medium-specific entities like `Character` / `Location` /
`Faction` carry off the four primitives via custom relations).
