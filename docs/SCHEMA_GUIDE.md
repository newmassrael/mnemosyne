# Schema Guide — customizing Mnemosyne for your codebase

Mnemosyne ships with a **fixed primitives** schema (Section / CrossRef /
ChangelogEntry / FrozenList / InventoryEntry — five closed-form entity
types) and a **markdown pattern → entity** mapping that is fully
config-driven via `mnemosyne.toml`. The atomic store
(`docs/.atomic/workspace.atomic.json`, path overridable via
`[atomic] sidecar_path`) is the single source of truth for typed facts;
the cascade output (`docs/GENERATED.md`, path overridable via
`[atomic] output_path`) is the deterministic human-readable view. This
guide shows the knobs available and the reference presets.

The fifth primitive — **InventoryEntry** — was added in Phase 1A
(Round 273) for stable external IDs with a lifecycle vocabulary distinct
from the audit-trail genre: test case ids (TC8 `ARP_07`,
`TCP_RETRANSMISSION_TO_04`), requirement ids, regulation ids. Lifecycle
states are `active` / `deprecated` / `reserved`; the validator's
inventory citation axis rejects citations of `deprecated` ids and of
unregistered ids.

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

[atomic]   # optional — override default store / cascade paths
sidecar_path = "doc/.atomic/store.json"  # default: docs/.atomic/workspace.atomic.json
output_path = "docs/coverage/SPEC.md"  # default: docs/GENERATED.md

[code_refs]   # optional — opt into code-citation defense
paths = ["src/"]
severity_missing = "warn"   # | "reject"
severity_binding = "warn"   # | "reject"
severity_inventory = "warn"  # | "reject"  (Phase 1A)
comment_only = true
inventory_prefixes = ["ARP_", "TCP_"]   # multi-prefix for inventory cite axis
external_section_prefixes = ["RFC", "IEEE"]  # `(RFC 791 §3.1)` etc. skipped

[[orphan_ledger]]  # optional — register legacy cross-ref carries
doc = "docs/legacy.md"
from = "12"
to = "99"
kind = "markdown_ref"  # | "atomic_entry_ref" | "atomic_section_ref" | "code_citation"
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
- **`[code_refs].inventory_prefixes`** — multi-prefix list for the
 inventory citation axis (Phase 1A, Round 275). Each entry is an ASCII
 word (`"ARP_"`, `"TCP_"`, `"SOMEIP_ETS_"`); the scanner walks
 `<prefix>[A-Z0-9_]+` tokens whose tail ends in a digit (the
 digit-terminus rule suppresses identifier-shaped false positives like
 `TCP_BUFFER_SIZE`). Empty list = axis disabled. Longest-prefix-first
 matching: when both `"SOMEIP_"` and `"SOMEIP_ETS_"` are registered,
 `SOMEIP_ETS_BASICS_01` reports once under the more specific prefix.
- **`[code_refs].external_section_prefixes`** — single-token prefix
 list (`["RFC", "IEEE", "ISO/IEC"]`) for external-standard `§` skip
 (Round 277). When a `§<id>` citation is preceded on the same line by
 `<prefix> <numeric>` (with surrounding punctuation like `(RFC 791`
 stripped, Round 281), it's treated as an external reference and
 ignored by the spec layer. Multi-token prefixes (e.g., `"ETSI TS"`)
 are not v1 — register the trailing token as a looser workaround.
- **`[code_refs].severity_inventory`** — `warn` / `reject` / `info`.
 Fires when an inventory citation's id is absent from the atomic store
 (`InventoryMissing`) or its registered status is `Deprecated`
 (`InventoryDeprecated`). `Active` / `Reserved` ids pass silently.
- **`[atomic].sidecar_path`** — workspace-relative or absolute path
 for the JSON store. Default `docs/.atomic/workspace.atomic.json`. Use
 this to redirect the sidecar into an existing `doc/` tree without
 colliding with `docs/`. CLI `--sidecar` flag wins over this config
 when both are present.
- **`[atomic].output_path`** — workspace-relative or absolute path for
 the cascade write target. Default `docs/GENERATED.md`. This is *not*
 auto-derived from `[workspace] docs[0]` — docs[0] is the parse target
 (markdown the validator reads), while `output_path` is the cascade
 write target (atomic store → md). Keep them independent so cascade
 doesn't overwrite hand-authored content on first mutate.
- **`[[orphan_ledger]]`** — register legitimate cross-ref carries
 (e.g. references to legacy docs you preserved by design). Each entry
 names `doc` / `from` / `to` / `kind` / `reason`. The validator's
 `T1 orphan total` line reports `ledger=N, new=+X, resolved=-Y`:
 ledgered orphans are silent, new orphans bail, and resolved-but-still-
 ledgered entries bail too (forcing you to drop the now-stale ledger
 row). `kind = "code_citation"` covers code-side citation suppression
 (Path B); see *Self-contained citation rule* below for when to use.

## Field length caps (T3 threshold, surfaced for DX)

The atomic mutate API enforces hard caps on text fields to keep the
audit trail compact and prevent prose drift from creeping into structured
data:

- `intent`: 200 chars max
- `rationale_bullets`, `inputs_bullets`, `outputs_bullets`,
 `caveats_bullets`, `caveat`: 100 chars per bullet

Exceeding the cap rejects the mutate with a clear error; split the text
or move detail into a separate atomic field (`examples`, `rationale`).

## Self-contained citation rule

`validate-code-refs` matches citations on a **single line** with **one
explicit prefix token** in scope. Citations must be self-contained at
their use site:

- ✓ `// RFC 2131 §3.5 client retransmits` — RFC token + section on the same line
- ✓ `// (RFC 791 §3.1) — fragmentation fields` — surrounding `()` stripped (Round 281)
- ✗ `// see RFC 3927 above\n// §2.2.1 says ...` — multi-line context;
 the second line has no RFC token on it
- ✗ `// 2131 §3.1 lease renew` — RFC numeric only, prefix word missing

The two failing forms are *not* fixed by the scanner — broadening
either pattern would push the layer into prose inference and create
false-skips on internal citations (a `§4.2.4` adjacent to a stray
numeric would silently bypass the spec-side reject gate). The
architectural rule is: prefer rewriting the comment to canonical
`RFC NNN §X.Y` form. When mass-rewriting isn't practical (legacy
codebase carry), register the (file, §id) pair in
`[[orphan_ledger]] kind = "code_citation"`:

```toml
[[orphan_ledger]]
doc = "<code-citation>"
from = "src/dhcpv4_client.cpp"
to = "3.1"
kind = "code_citation"
reason = "RFC 2131 §3.1 cited multi-line in DHCPv4 transition prose, retain"
```

The validator surfaces it under `ledger=N` (silent unless the orphan
later resolves), and the audit-trail records *why* the citation is
unmatched rather than silencing it.

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

### Inventory citation defense (test cases / requirement ids, Phase 1A)

For projects citing stable external ids in code — TC8 test cases, ISO
test specs, IEEE conformance ids, internal requirement ids — declare
the prefix family and let the inventory axis check existence + status
at cite time:

```toml
[code_refs]
paths = ["src/", "tests/"]
inventory_prefixes = [
 "ARP_", "TCP_", "UDP_", "IPV4_",
 "ICMPV4_", "DHCPV4_", "SOMEIPSRV_", "SOMEIP_ETS_",
]
severity_inventory = "warn"  # promote to "reject" after baseline clean
external_section_prefixes = ["RFC", "IEEE"]  # ignore `(RFC 791 §3.1)`
```

Register each id via the CLI (or sync from your upstream SSOT):

```bash
mnemosyne-cli add-inventory-entry \
 --id ARP_07 --status active --section §4.2.4 \
 --source "tc8_v3.pdf#row=12"

mnemosyne-cli add-inventory-entry \
 --id TCP_RETRANSMISSION_TO_04 --status deprecated \
 --reason "superseded by TO_05 in TC8 v2.3"
```

`active` and `reserved` ids cite freely; `deprecated` ids reject at
cite time (with an optional cascade scan surfacing existing cite-sites
when a mutate flips status). Lookup via `query --list-inventory` or
`query --inventory <id>`.

### External adopter — redirect store/output to avoid `docs/` collision

Projects with an existing `doc/` (or `documentation/`) tree that wants
to add Mnemosyne without renaming directories:

```toml
[workspace]
docs = ["docs/coverage/SPEC.md"]   # parse target
default_doc = "docs/coverage/SPEC.md"

[atomic]
sidecar_path = "doc/.atomic/store.json"   # avoid docs/.atomic collision
output_path = "docs/coverage/SPEC.md"   # cascade write — explicit
```

The mutate, read, validate, and cascade paths all honor both overrides
(Round 280); there is no split-brain. CLI `--sidecar` / `--output`
flags still win when supplied.

## What stays fixed

The five entity types — Section / CrossRef / ChangelogEntry / FrozenList
/ InventoryEntry — are **not** configurable. They are the universal
primitives the validator + mutate API + cascade engine all build on.
What `[schema]` configures is *which markdown patterns* the parser maps
onto Section / ChangelogEntry. The fifth, InventoryEntry, was added in
Phase 1A (Round 273) for stable-id citation hygiene; further entity
types (medium-specific `Character` / `Location` / `Faction` for
narrative products) remain Phase 1+ scope.
