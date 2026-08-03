# Schema Guide — customizing Mnemosyne for your codebase

Mnemosyne ships with a **fixed primitives** schema (Section / CrossRef /
ChangelogEntry / FrozenList / InventoryEntry — five closed-form entity
types) and a **markdown pattern → entity** mapping that is fully
config-driven via `mnemosyne.toml`. The atomic store
(`docs/.atomic/workspace.atomic.json`, path overridable via
`[atomic] sidecar_path`) is the single, directly-validated source of truth
for typed facts; humans read it via `mnemosyne-cli query` (the
markdown-render output `GENERATED.md` was removed in Round 400). This guide
shows the knobs available and the reference presets.

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
[workspace]  # required (root / spec_source only; `docs` and `default_doc`
             # were removed in Round 400 — see "Atomic sidecar path" below)

[workspace.spec_source]  # optional — external-spec mirror adopters only
url = "https://www.w3.org/TR/scxml/"
revision = "2015-09-01"
fetched_sha256 = "..."   # optional 64-char lowercase hex
fetched_at = "2026-05-27T00:00:00Z" # optional ISO-8601

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

[atomic]   # optional — override the default store path
sidecar_path = "doc/.atomic/store.json"  # default: docs/.atomic/workspace.atomic.json

[plugins.set_equality_validator]   # optional — opt into code-citation defense
paths = ["src/"]
severity_missing = "warn"   # | "reject"
severity_binding = "warn"   # | "reject"
severity_inventory = "warn"  # | "reject"  (Phase 1A)
comment_only = true
inventory_prefixes = ["ARP_", "TCP_"]   # multi-prefix for inventory cite axis
external_section_prefixes = ["RFC", "IEEE"]  # `<PREFIX> <NUMERIC> §<id>` skip
external_section_prefixes_bare = ["TR_SOMEIP", "SOMEIPSD"] # `<PREFIX> §<id>` skip (R284)

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
- **`[plugins.set_equality_validator].paths`** — production source paths to scan for spec
 citations (`Round NNN`, `§<id>`). Test paths intentionally excluded —
 traceability anchors in tests carry a different policy contract than
 rationale in production source. Typically `["src/"]` or
 per-crate `["crates/foo/src/", "crates/bar/src/"]`.
 **This list is the ONLY thing that decides what the gate reads.** Every file
 under a listed directory is read, whatever its language or extension; the walk
 skips only hidden directories, `target/` and `node_modules/`. To stop the gate
 reading a subtree — generated output under a source parent, a `build/` or
 `__pycache__/` directory — **narrow `paths` to the source subdirs**. There is
 no subtract-this-subtree key, and `scan_exclusions` is not one (below).
 That skip list holds the two ecosystems this validator is written in and no
 others, so `validate-code-refs` prints a **vcs axis** every run: how many files
 in the read set the tree's own VCS calls build output, in the three distinct
 states `N ignored` / `0 ignored` / `not determined`. It is advisory, and it is
 the only axis that can see generated output which has a comment syntax and is
 valid UTF-8 — the other two name files by those properties and structurally
 cannot. The predicate is untracked AND ignored, so a file committed with
 `git add -f` is not called build output.
- **`[plugins.set_equality_validator].scan_exclusions`** — a DECLARATION for the
 coverage axis, not a filter on the read set. It answers "which Rust source is
 deliberately not covered by `paths`" (so a tree merely absent from the config
 fails while a tree someone wrote down stays out), and it feeds the
 exclusion-integrity check that catches an exclusion swallowing the only copy
 of a citation. **It does not narrow what the citation gate reads.** An entry
 naming a subtree that a `paths` entry already covers therefore matches real
 files, reports no stale exclusion, and changes nothing — config that looks
 like it works. `validate-workspace` names that overlap as an advisory; the
 repair is to narrow `paths`. Reported from the field by a consumer who reached
 for this key first, exactly as its name invites.
 Because the exclusion-integrity axis READS the excluded set to decide what an
 exclusion swallowed, `validate-workspace` prints the same **vcs axis** over that
 set, in the same three states. A large `N of M` there means the `0 swallowed`
 above it was computed from files a fresh clone does not have.
 **A vendored tree is not "swallowing" anything, and you do not have to declare
 that.** If a subtree is a git submodule, the `§N.M` in it belongs to that
 project's numbering, and every axis here skips it: `git ls-files --stage`
 marks a submodule with mode `160000`, and a file under such a path is read as
 another repository's. So `scan_exclusions = ["vendor/"]` over a submodule no
 longer rejects the vendored project's citations of its own sections — which was
 the one case the exclusion-integrity check got wrong, reported from the field by
 two consumers whose section ids collided with a vendored spec's. There is
 deliberately **no key** for this: a declaration would be a claim nothing could
 check, and this codebase's hand-maintained lists have all drifted. Both commands
 print a **numbering origin** line every run, in the same three states, naming the
 subtrees and how many `§`-tokens went with them — read it, because this is the
 one axis in the family that makes citations DISAPPEAR. A monorepo that
 deliberately shares one numbering across a submodule is the shape this cannot
 express; if that is yours, the line will say so and the override is a
 conversation, not a config key.
- **`[plugins.set_equality_validator].severity_missing`** — `warn` or `reject`. Fires when a
 citation's target id is absent from the atomic store (hallucination).
 Start at `warn` to surface the baseline, promote to `reject` once
 clean.
- **`[plugins.set_equality_validator].severity_binding`** — `warn` or `reject`. Fires when a
 citation appears in a file that the section's
 `bindings` (Path B Spec ↔ Code) does *not* list (`citation_unbound`),
 or when a binding's file carries no citation (`binding_unbacked`), or on a
 symbol-set mismatch (`symbol_mismatch`). Presence is **kind-agnostic** —
 a binding of *any* `kind` (`implements` or `references`) defends a cite.
 Bidirectional binding integrity.
- **`[plugins.symbol_resolver.<lang>]`** — what makes `severity_binding` a
 *symbol*-level claim rather than a file-level one. `<lang>` is a language
 the extension table produces: `rust` (`.rs`), `cpp` (`.c` `.cc` `.cpp`
 `.cxx` `.h` `.hh` `.hpp` `.hxx`), `python` (`.py`), `go` (`.go`) — an entry
 keyed to anything else could never be consulted and is **refused** at
 startup, as is an entry naming an in-process `backend` this build has no
 plugin for. Backends shipped: `tree-sitter-rust`, `tree-sitter-cpp`.
 A file the axis cannot reach — an extension in no row above, or a language
 with no entry here — binds at **file** granularity whatever
 `severity_binding` says; `validate-code-refs` prints that as its
 `symbol axis (advisory)` line every run, with the count of unreachable
 files that carry a gated citation. Read that number before believing
 `severity_binding = "reject"` means symbol-level everywhere.
- **`[plugins.set_equality_validator].severity_coverage`** — `warn`/`reject`/`info`;
 inherits `severity_binding` when unset. Fires (`impl_missing`) when an
 Active section has **zero `implements` bindings**. Coverage counts only
 `kind = "implements"` (SysML «satisfy»); `references` («trace») links
 satisfy citations but do **not** count as implementation coverage.
 **Before turning this to `reject`, classify.** The gap it reports is only
 meaningful for sections that are supposed to have code, and that is a
 per-section fact in the store, not a config setting — see
 *Coverage classification* below. A consumer who enforced first had 244
 undifferentiated warnings across four ledgers and could not tell a real gap
 from prose; classifying brought it to 163, all of them real.

### Coverage classification — which sections are supposed to have code

`severity_coverage` asks "does this section have an `implements` binding". Whether
it *should* is `Section.coverage_expectation`, set per section with
`set-section-coverage-expectation --section §<id> --expectation <tag> --reason <text>`
(`--reason` is mandatory and is recorded in the store — a year later that is the
difference between an auditable decision and a bare tag). Three values, and they
are not interchangeable:

| tag | means | coverage axiom |
|---|---|---|
| `normative` (default) | a requirement that expects implementing code | **applies** |
| `out_of_scope_here` | in **the document this ledger mirrors**, not implemented *here* — revisitable if scope expands | exempt |
| `informational` | **inherently** non-implementable prose: terminology, overview, references | exempt |

Two readings of `out_of_scope_here` have cost a consumer real time, so both are
stated:

- **"The document this ledger mirrors" is not necessarily an external standard.**
 It is whatever this ledger documents — including your own design doc. A
 consumer whose other ledger *was* an external mirror read "the standard" as
 external, concluded the value was rare and standards-specific, and left 19
 sections `normative` that no binding could ever clear.
- **A deferred or Phase-2 feature is `out_of_scope_here`, not `informational`.**
 `informational` means *inherently* non-implementable. Marking an intended
 feature `informational` is a false declaration, and unlike a missing binding
 nothing later contradicts it.

The orthogonal axis is `Section.verification_expectation`
(`dedicated` / `by_construction`, R413): whether a *normative* section's evidence
is a concrete test artifact. A `by_construction` section is still `normative` for
implements-coverage — folding the two axes into one would force an exempt
mislabel that silently drops the section from coverage.

**What the gates do and do not check here.** `misclassified_coverage` (R423)
rejects an exempt section that carries an `implements`/`verifies` binding — the
contradiction is structural, so it is caught. **Nothing checks whether a tag
matches the prose**: an intended feature tagged `informational` passes every gate.
The consumer above found two such misclassifications only by having four
independent agents classify one ledger each without seeing the written principle,
and their calls disagreed with it in the same place. If you are classifying at
scale, that shape is the check.

**The value set is not two.** Any message that spells it out derives it from the
enum (R870); before that, five surfaces spelled it by hand and three still offered
`informative`, the two-state tag R422 removed with no alias. If a tool ever offers
you `informative`, it is older than R422 — the store will refuse it.
- **`[plugins.set_equality_validator].comment_only`** — `true` strips string literals before
 scanning so only comment citations count. Default `true`; flip only if
 your project deliberately puts §id references in user-visible strings.
- **`[plugins.set_equality_validator].inventory_prefixes`** — multi-prefix list for the
 *opaque-ID* inventory citation axis (Phase 1A, Round 275). Each entry
 is an ASCII word (`"ARP_"`, `"TCP_"`, `"SOMEIP_ETS_"`); the scanner
 walks `<prefix>[A-Z0-9_]+` tokens whose tail ends in a digit (the
 digit-terminus rule suppresses identifier-shaped false positives like
 `TCP_BUFFER_SIZE`). Empty list = axis disabled. Longest-prefix-first
 matching: when both `"SOMEIP_"` and `"SOMEIP_ETS_"` are registered,
 `SOMEIP_ETS_BASICS_01` reports once under the more specific prefix.
- **`[plugins.set_equality_validator].inventory_path_prefixes`** — companion axis with
 *section-path* tail shape (`[A-Za-z0-9./-_]+`, no digit-terminus
 requirement). Each entry is a prefix that may include spaces
 (`"W3C SCXML "`, `"IRP "`); the scanner walks
 `<prefix><section-path>` tokens, so `W3C SCXML 3.13`, `IRP test144`,
 `SCXML-D.2.selectTransitions` all match. Targets external-spec
 mirror adopters (W3C SCXML, IETF RFC, IEEE, AUTOSAR family) who
 would otherwise face a mass cite migration to the sigil-prefixed
 form. Resolution target is the same `InventoryEntry` store as
 `inventory_prefixes` — they are two tail-shape axes feeding the
 same lifecycle (active / deprecated / reserved). A prefix may be
 registered in both axes if both citation shapes coexist; the
 scanner dedups so a matching cite surfaces once.
- **`[plugins.set_equality_validator].external_section_prefixes`** — single-token prefix
 list (`["RFC", "IEEE", "ISO/IEC"]`) for the *numeric-document* form
 of external-standard `§` skip (Round 277). Citation form:
 `<prefix> <numeric> §<id>` — same line, with surrounding punctuation
 like `(RFC 791` stripped (Round 281). For *doc-name* standards
 without a numeric document number, see
 `external_section_prefixes_bare` below. Multi-token prefixes
 (e.g., `"ETSI TS"`) are not v1 — register the trailing token as a
 looser workaround.
- **`[plugins.set_equality_validator].external_section_prefixes_bare`** — single-token
 prefix list for the *doc-name* form of external-standard `§` skip
 (Round 284). Citation form: `<prefix> §<id>` — prefix directly
 before sigil, no numeric between them. Used by AUTOSAR family
 (TR_SOMEIP, SOMEIPSD, SWS_SD) and other doc-name-only standards.
 Kept distinct from `external_section_prefixes` so registration is
 an *explicit opt-in* per prefix — see *External standard prefix
 kinds* below for the FP risk on generic-sounding tokens.
 Both registries key on the CITING PROSE, so they reach a citation only when its
 author named the document. A vendored project's documents cite their own sections
 with the document left implicit (`per §6.2`), and no prefix can reach that — the
 **numbering origin** axis under `scan_exclusions` above is what handles it,
 by where the file lives rather than by what the token says. The two compose:
 register a prefix for `W3C SCXML §5.10` in *your* source, and let the submodule
 boundary answer for the same reference inside the vendored tree.
- **`[plugins.set_equality_validator].severity_inventory`** — `warn` / `reject` / `info`.
 Fires when an inventory citation's id is absent from the atomic store
 (`InventoryMissing`) or its registered status is `Deprecated`
 (`InventoryDeprecated`). `Active` / `Reserved` ids pass silently.
- **`[atomic].sidecar_path`** — workspace-relative or absolute path
 for the JSON store. Default `docs/.atomic/workspace.atomic.json`. Use
 this to redirect the sidecar into an existing `doc/` tree without
 colliding with `docs/`. CLI `--sidecar` flag wins over this config
 when both are present.
- **`[[publishable_override_ledger]]`** — authorize an entry whose
 *publishable* half intentionally diverges from its frozen *audit* half.
 The audit half is append-only and never mutates (R161 §41), so when a
 redaction or a rename must be reflected outward, `redact-term` /
 the `set-changelog-publishable-*` setters change only the publishable
 side and this ledger anchors the divergence. Each entry names `kind`
 (`"redaction"` / `"typo"` / `"clarification"`), `target_id` (the entry
 key, short `Round N` or long `Round N — title`), `fields` (which
 `publishable_*` fields diverge — informational; the v1 gate matches at
 entry granularity), `reason`, `applied_in` (the round that applied it),
 `content_hash_after` (required SHA256) and optional
 `content_hash_before`. `emit-publishable-override-ledger-draft` prints
 a ready-to-paste block for a diverged entry. Unledgered divergence is a
 validate-workspace failure (R296 gate).
- **`[[orphan_ledger]]`** — register legitimate cross-ref carries
 (e.g. references to legacy docs you preserved by design). Each entry
 names `doc` / `from` / `to` / `kind` / `reason`. The validator's
 `T1 orphan total` line reports `ledger=N, new=+X, resolved=-Y`:
 ledgered orphans are silent, new orphans bail, and resolved-but-still-
 ledgered entries bail too (forcing you to drop the now-stale ledger
 row). `kind = "code_citation"` covers code-side §-axis citation
 suppression (Path B); `kind = "inventory_citation"` (Round 285)
 covers code-side inventory-axis citation suppression (intentional
 historical references to deprecated / deleted test-case ids).
 See *Self-contained citation rule* below for when to use.

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

`validate-code-refs` resolves a citation's external-standard prefix from
a **narrow, explicit scope** — never prose inference. As of Round
379/380 the recognized forms are:

- ✓ `// RFC 2131 §3.5 client retransmits` — prefix + section, same line
- ✓ `// (RFC 791 §3.1) — fragmentation fields` — surrounding `()` stripped (Round 281)
- ✓ `// CSS Color 4 §8.1` / `// UAX #9 §3.3` — multi-word and `#`-number prefixes (Round 379)
- ✓ `// UAX #9 §6.6.8 / §6.6.9 / §6.6.10` — same-line chain, `/`-separated (Round 380)
- ✓ `/// WAI-ARIA 1.2` then `/// §6.6.6` — wrap: prefix at the *end* of the previous comment line (Round 380)
- ✗ `// see RFC 3927 above` then `// §2.2.1 says ...` — prefix is mid-prose, not the previous line's tail, so it does not carry
- ✗ `// 2131 §3.1 lease renew` — numeric only, prefix word missing

The failing forms stay unskipped by design: the wrap carry fires only
when the previous comment line *ends with* the prefix, and the chain
only across `/`-or-whitespace separators (a comma or word breaks it) —
broadening past that would be prose inference and risk false-skips on
internal citations (a `§4.2.4` adjacent to a stray numeric silently
bypassing the spec-side reject gate). Prefer rewriting to canonical
`<PREFIX> §X.Y` form. When mass-rewriting isn't practical (legacy
codebase carry), register the (file, §id) pair in
`[[orphan_ledger]] kind = "code_citation"`:

```toml
[[orphan_ledger]]
doc = "<code-citation>"
from = "src/dhcpv4_client.cpp"
to = "3.1"
kind = "code_citation"
reason = "RFC 2131 §3.1 cited multi-line in DHCPv4 transition prose, retain"
since = "v2.4"   # required — when this orphan was registered
```

The same shape applies to the inventory citation axis (Round 285) —
register `kind = "inventory_citation"` when the code intentionally cites
a deprecated or deleted test-case id (e.g., to document why a code path
deliberately skips a removed scenario):

```toml
[[orphan_ledger]]
doc = "<inventory-citation>"
from = "src/dissect/packet_pipeline.cpp"
to = "IPv4_OPTIONS_01"
kind = "inventory_citation"
reason = "Historical: IPv4_OPTIONS_01..14 deleted V2->V3, documents why dissector skips IP options"
since = "v3.0"   # required — when this orphan was registered
```

Both axes are independent — a `code_citation` row does not suppress an
`inventory_citation` violation, and vice versa. The validator surfaces
the ledger entry under `ledger=N` (silent unless the orphan later
resolves), and the audit-trail records *why* the citation is unmatched
rather than silencing it.

## Citation token grammar (id boundaries)

The `§<id>` extractor reads the id as the run of section-id characters
after the sigil — alphanumerics plus `. - _ /`. Two deterministic
boundary rules are worth knowing when authoring citations (both are
grammar edges, not parser bugs):

- **A `.` continues the id only between two digits.** `§39.bindings`
  parses as id `39` (the `.bindings` prose suffix is dropped),
  while `§3.13` stays whole. A double-dot Appendix form like
  `§scxml-D.2.func` therefore truncates at `§scxml-D` (`.func` is not
  digit-bounded) — use an all-hyphen form (`§scxml-D-2-func`) when the id
  must carry letters after a dot.
- **`-` is an id character, so a glued suffix over-extends.** `§16.5-L3500`
  (a section glued to a line-ref by `-`) parses as the single id
  `16.5-L3500`, not `16.5`. Space-separate a non-id suffix —
  `§16.5 L3500` or `§16.5 (L3500)`. `-` must stay an id char for
  namespaced ids (`scxml-3.13`, `scxml-D-interpret`), so the extractor
  cannot guess the boundary; the author marks it with a space.

Author citations so the id ends where you intend; the orphan_ledger is
the escape hatch when a non-conforming citation must stay.

## External standard prefix kinds

External standards identify their documents in two ways, and the
validator's `§<id>` skip layer treats them as **independent axes**
(Round 284) so registration is an explicit opt-in per kind:

| Kind | Citation form | Examples | Config key |
|------|---------------|----------|------------|
| Numeric document number | `<PREFIX> <NUMERIC> §<id>` | RFC 791, IEEE 802.3, ISO/IEC 14882 | `external_section_prefixes` |
| Doc-name short identifier | `<PREFIX> §<id>` | TR_SOMEIP, SOMEIPSD, SWS_SD | `external_section_prefixes_bare` |

Both forms are skipped when the prefix word is registered in the
appropriate axis. Leading punctuation (`(`, `[`, `"`, `«`) is stripped
in both modes (Round 281), so `(RFC 791 §3.1)` and `(TR_SOMEIP §X.Y)`
both match.

**Multi-word names and W3C document forms (Round 379).** A registered
prefix may be **multi-word** — it is matched as a token-boundary suffix
of the prose before the document number (numeric mode) or before the
sigil (bare mode). The document-number token may carry a leading `#`
(Unicode Annex form) or trailing letters (versioned standards). So
W3C-family citations are covered by registering the full standard name:

```toml
external_section_prefixes = [          # <PREFIX> <doc-number> §<id>
  "UAX", "UTS", "UTR",                 # UAX #9 §3.3, UTS #51 §16
  "CSS Color", "CSS Fonts",            # CSS Color 4 §8.1  (multi-word)
  "WAI-ARIA", "WCAG", "IEEE",          # IEEE 802.11ax §… (letter-suffixed)
]
external_section_prefixes_bare = [     # <PREFIX> §<id>
  "Unicode Standard", "Web IDL",       # Unicode Standard §3.12 (multi-word)
  "UCD", "WAI-ARIA",                   # WAI-ARIA cited both ways → both axes
]
```

The verbatim token-boundary match means a registered prefix only skips a
citation whose prose actually *ends with* that exact prefix, so
`SCSS 3 §` does not match `"CSS"` and `random Color 4 §` does not match
`"CSS Color"` — prefer the most specific full name.

**Citation scope — chains and comment wraps (Round 380).** The prefix
need not sit immediately before *every* sigil in a group:

- **Chained list** — `UAX #9 §6.6.8 / §6.6.9 / §6.6.10`: the first cite
  carries the prefix; the rest inherit when separated only by `/` or
  whitespace. A comma, `and`, or any other token breaks the chain, so a
  genuinely distinct internal cite (`UAX #9 §3.3, §5.16`) is still
  validated.
- **Comment-block wrap** — a sigil that is the first content on its line
  inherits the prefix from the immediately preceding comment line when
  that line *ends with* the prefix:

  ```rust
  /// WAI-ARIA 1.2
  /// §6.6.6   // inherits WAI-ARIA — not flagged as a missing section
  ```

  Only the immediately previous line carries (an intervening prose line
  breaks it), and the prefix must be the line's trailing content — both
  guards keep an internal citation from being skipped by accident.

**Pick the right axis.** The two-axis design exists so generic-sounding
tokens (e.g., `"AUTOSAR"`) don't silently skip *internal* `§<id>`
citations on prose lines that happen to mention the standard name.
Register the *most specific* form of the prefix you actually cite:

- ✓ `external_section_prefixes_bare = ["TR_SOMEIP", "SOMEIPSD", "SWS_SD"]` — specific document short-names
- ⚠️  `external_section_prefixes_bare = ["AUTOSAR"]` — generic; a prose
 line like `// AUTOSAR cluster startup §2.1` will silently skip the
 internal `§2.1` reference

When a standard supports both citation forms (some IEEE specs are
cited as both `IEEE 802.3 §...` and `IEEE_802_3 §...`), register the
prefix in both axes. The two paths are independent and a citation
matches whichever axis's shape applies.

## Common authoring patterns

### English design doc (this repo)

Just author `mnemosyne.toml` and commit. The Mnemosyne preset is the
default — your spec passes through unchanged, and the validator
recognizes `Round N` entries under `## Changelog`.

### English-only README + ARCHITECTURE.md, no numbered ledger

```toml
[workspace]

[schema]
changelog_titles = ["Changelog"]
entry_id_prefix = ""  # no numbered entries
medium_name = "generic"
```

### ADR-style decisions (`ADR-NNNN`)

```toml
[workspace]

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
[plugins.set_equality_validator]
paths = [
 "crates/foo/src/",
 "crates/bar/src/",
]
severity_missing = "reject"
severity_binding = "reject"
comment_only = true
```

Run `mnemosyne-cli validate-code-refs` to scan; wire that command into
your project's own pre-commit hook to gate every commit. Promote
`severity_*` from `warn` to `reject` once the baseline is clean.

**CI deployment (Round 381).** Mnemosyne ships no prebuilt binaries; an
external consumer installs the CLI from a **pinned revision** so the
validator's behaviour is frozen until you deliberately bump it — a
compliance ledger wants exactly that (a citation grammar that does not
shift under CI without a visible commit). The consumers are Rust
projects, so the toolchain is already present:

```yaml
# consumer-side: .github/workflows/spec-citations.yml
jobs:
  citations:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - name: Install mnemosyne-cli (pinned revision)
        run: cargo install --git https://github.com/newmassrael/mnemosyne --rev <PINNED_SHA> --locked mnemosyne-cli
      - name: Validate spec citations (per workspace)
        run: |
          for ws in docs/spec/scxml docs/spec/irp docs/sce-ledger; do
            ( cd "$ws" && mnemosyne-cli validate-code-refs )
          done
```

Pin `--rev` to a commit, not a branch; `--locked` builds against
Mnemosyne's committed `Cargo.lock` for a reproducible install, and
`cargo install` caches the built binary by rev. Prebuilt release
binaries and a packaged GitHub Action are intentionally deferred — add
them only when a non-Rust consumer appears or CI compile time becomes a
measured cost.

**Local development, where the pin is NOT enforced by anything (Round
823).** The CI recipe above is safe because a CI runner is fresh and has
one consumer. A developer machine is neither: `cargo install` writes
`~/.cargo/bin/mnemosyne-cli`, a slot every checkout on that machine
shares, so the last `cargo install` wins and a project silently runs its
gates with another project's tool. This is not hypothetical — Mnemosyne's
own repo overwrote a consumer's pinned binary with an uncommitted local
build, and the consumer's gates measured with it.

Cargo pins your *library* dependencies (`Cargo.lock`) but it does not pin
a binary you invoke by name. So resolve the binary FROM YOUR PIN rather
than from `PATH`, and install per-rev so several pins coexist:

```bash
# consumer-side: derive the path from the pin, provision it if absent.
MN_PIN="<PINNED_SHA>"
MN_ROOT="${MN_ROOT:-$HOME/.local/mn}/$MN_PIN"
if [[ ! -x "$MN_ROOT/bin/mnemosyne-cli" ]]; then
  cargo install --git https://github.com/newmassrael/mnemosyne \
    --rev "$MN_PIN" --locked mnemosyne-cli --root "$MN_ROOT"
fi
MN="${MN:-$MN_ROOT/bin/mnemosyne-cli}"
```

`$MN` is then independent of `PATH` and of every other checkout, bumping
`MN_PIN` provisions the new binary by itself, and the old one stays put
for whatever still pins it.

Keep an assertion anyway, and run it wherever the tool is used — not only
where the store is built. `mnemosyne-cli --version` prints
`0.1.0 (<short-sha>)`, with a `-dirty` suffix when the tree it was built
from had uncommitted changes; a `-dirty` build corresponds to no
revision, so it must never be accepted as one:

```bash
rev="$("$MN" --version | sed -n 's/.*(\([0-9a-f]\{7,\}\)).*/\1/p')"
[[ "$rev" == "$MN_PIN" ]] || { echo "mnemosyne-cli is $rev, pinned $MN_PIN" >&2; exit 1; }
```

A build script and a test harness that resolve the tool differently will
eventually measure with different tools, and the numbers they report are
then not comparable — put both halves behind one resolver.

### `[tool]` — declare the pin and let the tool enforce it (Round 826)

Everything above is the consumer doing the enforcing, which means every
place that runs the tool has to remember to. `[tool]` moves the pin into
the workspace itself, where the tool reads it:

```toml
[tool]
pin = "75eddce5"   # the Mnemosyne revision this workspace is validated by
```

Every Mnemosyne binary — the CLI and the MCP server alike — checks the
declared pin against the revision it was built from. If the pinned build
is already installed under the per-rev root, **it switches to it and runs
that** (Round 832); if it is not installed, it refuses, and says so with
the revision it *is*, the revision you asked for, and the command that
installs the right one. This reaches surfaces the recipe above cannot: an
MCP server is launched by a host config that is not yours to script, but
`--workspace` already points here.

So one declaration is enough. You do not need a wrapper script per repo,
and the host config that launches an MCP server can keep naming whatever
binary it already names — that binary reads your pin and hands over to
the right one. The switch replaces the process rather than spawning a
child, so stdio, working directory and exit code all survive it; a server
mid-handshake does not notice. It announces itself on stderr, never
stdout, because stdout is where the server's protocol lives.

**Adopt in this order: UPGRADE FIRST, THEN DECLARE (Round 828).** A
binary built before this section existed cannot read a config that has
it. `WorkspaceConfig` denies unknown fields — deliberately, so a typo'd
key is a loud error rather than a silently ignored one — so an older
binary stops at TOML parse:

```
error: mnemosyne.toml parse failed: TOML parse error at line 3, column 2
  |
3 | [tool]
```

Refusing is CORRECT: a tool that cannot enforce a pin has no business
opening a pinned workspace. The message is what fails you — it says
nothing about revisions, so nothing tells you the fix is a newer
Mnemosyne. And the trap is circular in a way no other section's is: the
pin exists to name which binary may act here, and if the revision you
name predates `[tool]`, the very binary you pinned is the one that cannot
read the pin naming it.

Measured on two real pre-`[tool]` binaries, each without and with the
section: a consumer's pinned CLI at `75eddce5` and an installed MCP
server at `25d30d16` both run normally without it and both exit 1 at the
parse error with it.

So: move the pin you intend to declare to a revision at or after the one
that introduced `[tool]`, install that, confirm `--version` reports it,
and only then add the section. If you pin an older revision on purpose,
do not declare it here — keep using the `MN`-style resolution above,
which needs no cooperation from the binary.

**And this is a STANDING property, not an adoption step (Round 861).** The
order above gets you in; it does not keep you in. A consumer adopted the pin
correctly and went red 65 minutes later, when a concurrent session reinstalled
an older binary on PATH — every command that reads a config then said their
*config* was wrong, and no ordering discipline prevents that, because the
older binary arrives after the ordering is done. Two things follow. Resolve
the tool the way you resolve any other build input: a name on PATH is whatever
was installed last, while a submodule, a lockfile rev or an explicit per-rev
path names one revision and keeps naming it. And read `unknown field` on a key
you did not typo as *"this binary is older than this workspace"* — which is
what the error itself now says, on any build from Round 861 on:

```
unknown field `some_new_key`, expected one of `workspace`, …

  this build is `25d30d16`, and a workspace's config is readable only by a
  Mnemosyne at or after the revision that introduced its NEWEST key.
  so: if the field named above is not a typo, this build is older than this
  workspace — install a newer one, or remove the field
```

That hint cannot help the build in the report above, and could not have: a
binary too old to know a key is also too old to carry the sentence explaining
it. It is written for the key that does not exist yet.

**And the floor is not `[tool]` — it is the newest key in your file
(Round 840).** Field-tested by a consumer: their pre-`[tool]` binary never
reached the pin at all, because it died on `scan_exclusions`, a key from an
*earlier* round sitting further up the same config. Any key newer than the
running binary poisons config parsing first, so a pin can only ever protect
against revisions NEWER than the newest key beside it; nothing inside the
config can protect against an older binary. Covering that direction is the
`--version` assertion in the `MN`-style recipe above, which is why that
recipe is not superseded by `[tool]` — the two guard opposite directions.

**That floor stopped moving in Round 863.** The paragraph above described a
consequence of the ORDER, not a property of pinning: the config was validated
in full and only then was the pin looked at, so any unknown key killed the load
before delegation was considered. From Round 863 the pin is read out of a
generic TOML table BEFORE validation, so a binary at or after that revision
hands over to the pinned build no matter how many keys it has never heard of.
Measured on real builds: a pre-863 binary meeting a pinned workspace with an
unknown key dies at the parse error and never prints a switch note; an 863
binary prints the note and hands over.

Three things that does NOT change, so it is not read as more than it is. A
binary older than 863 is still stopped by any key it does not know, and nothing
can reach back and fix that. A workspace with no pin still dies at the parse
error, correctly — there is nothing to delegate to. And an unknown key is still
fatal once the pinned build has it: the loose read decides where to run, never
what is legal.

The rules, so nothing is surprising:

- **Opt-in.** No `[tool]` section means no check, which is every
  workspace that predates this one.
- **Any prefix of at least seven hex characters** — git's own
  abbreviation floor. Shorter stops naming one commit.
- **A `-dirty` build satisfies no pin at all.** It was built from a tree
  with uncommitted changes, so it is not any revision, and saying
  otherwise is the exact lie the pin exists to prevent. A binary built
  without git (`unknown`) is refused for the same reason: unverifiable,
  which is not the same as verified-and-wrong.
- **`MNEMOSYNE_PIN_SKIP=1` waives it**, and warns on every use. A result
  produced under the waiver is not attributable to the pinned revision.
  **It does not rescue a binary that cannot PARSE the file** (Round 861): the
  waiver is read after the config has been parsed, so a build older than any
  key in your file dies before reaching it. This was written here as an
  unconditional escape hatch until a consumer reached for it in exactly that
  situation and found it inert — an escape hatch with a floor of its own is
  not one where the floor is.
- **The build under a pin is verified BEFORE the hand-off** (Round 861).
  `$MN_ROOT/<pin>/bin` is a directory-naming convention, so what sits there
  merely *claims* to be that revision; the tool asks it `--version` and
  refuses when the answer is a different revision, or is missing. Checking
  only afterwards was not enough: a build older than `[tool]` cannot re-check
  a pin at all and died at TOML parse instead, which reports a broken install
  as a broken config.
- **An unidentified build is refused, never switched away from.** A build
  that never declared which revision it is has a defect no installed
  binary repairs, and handing off would hide it in a second process.
- **The switch happens at most once.** Since Round 861 the ordinary
  mis-install is refused before the hand-off, so what this still covers is
  the build that *answers* with the pinned revision and then does not honour
  it — one replaced between the check and the exec, say. The second check
  says so and stops, rather than exec'ing in a circle.

**Using an installed revision is not procuring one, and the difference is
the whole boundary.** The switch resolves a build that is already on
disk: no network, no compile, one `exec`. Downloading and running another
version of itself is what a tool must not do, so an absent build is still
refused with the install line — provisioning stays the per-rev `--root`
recipe above, or an explicit command you run on purpose.

Where a pin resolves to is `$MN_ROOT/<pin>/bin/<binary>`, defaulting to
`~/.local/mn` — the same `MN_ROOT` the shell recipe above uses, and the
same path the refusal prints. The lookup and the advice are one
definition on purpose: two spellings of it would drift, and the drift
would read as the tool searching where its own instructions never
pointed.

`[tool] pin` and `schema_version` do not overlap. The store's schema
version guards the SHAPE of what is written, and its monotone guard
already refuses a backwards step; the pin guards the JUDGEMENT that wrote
it. Rounds 821 and 822 changed no schema and changed what
`validate-continuity` reports — a schema-only pin would not have moved
while the verdicts did.

To bind a section to a code file (so the binding axis recognizes the cite
as backed), use `add-section-binding` with an explicit `--kind`:

```bash
# implements (= SysML «satisfy»): the symbol fulfills the requirement;
# counts as implementation coverage.
mnemosyne-cli add-section-binding \
 --section §3 --file crates/foo/src/auth.rs --symbol Session::validate \
 --kind implements

# references (= SysML «trace»): the symbol relates to / draws meaning from
# the section (a DTO field, a read of a spec concept) without claiming
# fulfillment. Defends the citation; does NOT count as coverage.
mnemosyne-cli add-section-binding \
 --section §3 --file crates/foo/src/dto.rs --symbol Session::token \
 --kind references
```

Reclassify an existing binding (e.g. a data field wrongly recorded as
`implements`) with `set-section-binding-kind --section … --file … --kind
references --reason "<why>"`; remove one with `remove-section-binding`.

### Inventory citation defense (test cases / requirement ids, Phase 1A)

For projects citing stable external ids in code — TC8 test cases, ISO
test specs, IEEE conformance ids, internal requirement ids — declare
the prefix family and let the inventory axis check existence + status
at cite time:

```toml
[plugins.set_equality_validator]
paths = ["src/", "tests/"]
inventory_prefixes = [
 "ARP_", "TCP_", "UDP_", "IPV4_",
 "ICMPV4_", "DHCPV4_", "SOMEIPSRV_", "SOMEIP_ETS_",
]
# External-spec mirror adopters add section-path-shape prefixes here:
inventory_path_prefixes = [
 "W3C SCXML ",     # section refs like `W3C SCXML 3.13`
 "IRP ",           # test catalog refs like `IRP test144`
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

### External adopter — redirect the store to avoid `docs/` collision

Projects with an existing `doc/` (or `documentation/`) tree that want to
add Mnemosyne without renaming directories override the sidecar path:

```toml
[atomic]
sidecar_path = "doc/.atomic/store.json"   # avoid docs/.atomic collision
```

The mutate, read, and validate paths all honor the override; the CLI
`--sidecar` flag wins when supplied. (The pre-R400 `[workspace] docs` /
`default_doc` / `[atomic] output_path` knobs are gone — the store is the
single directly-validated artifact, with no markdown parse/render target.)

### External-spec mirror

For projects vendoring an external standard (W3C / IETF RFC / IEEE /
ISO/IEC / AUTOSAR family) as a workspace mirror so code citations stay
honest against the spec graph. Two first-class fields plus the existing
atomic primitives cover the pattern end-to-end:

- `[workspace.spec_source]` — workspace provenance (RFC-002 FR-2).
- `AtomicSection.normative_excerpt` — vendored quote anchored to each
  mirrored section (RFC-002 FR-1).

```toml
# docs/spec/scxml/mnemosyne.toml — one workspace per external namespace
[workspace]

[workspace.spec_source]
url = "https://www.w3.org/TR/scxml/"
revision = "2015-09-01"
fetched_sha256 = "abcdef0123...64-hex...0123"
fetched_at = "2026-05-27T00:00:00Z"

[schema]
changelog_titles = ["Revision History"]
entry_id_prefix = "Rev "               # spec rev bump → ChangelogEntry append
anchor_convention = "section_number"
medium_name = "spec_mirror"

[plugins.set_equality_validator]
paths = ["src/", "include/"]
severity_missing = "reject"
severity_binding = "reject"
comment_only = true
```

**One workspace per external namespace.** A repo that cites W3C SCXML
*and* the IRP test catalog *and* its own design ledger keeps three
separate `mnemosyne.toml` trees (e.g. `docs/spec/scxml/`,
`docs/spec/irp/`, `docs/design/`) and runs `validate-workspace` once
per tree. Single-`mnemosyne.toml` multi-namespace bundling is *not*
supported — `[workspace.spec_source]` is single-valued by design.

**Commit↔ledger drift gate in a multi-workspace repo.**
`validate-workspace` includes a commit↔ledger drift gate (Round
293/301): it scans recent commit subjects for Mnemosyne changelog round
labels `(R<n>)` / `(Round <n>)` and rejects when a cited round has no
backfilled atomic-store entry. In a multi-workspace mono-repo the scan
is **path-scoped to each workspace's own subtree** (Round 377), so a
round label on a commit that only touched a *sibling* workspace does not
false-flag this one. If your repo's commit convention uses `(R<n>)` to
mean something *other* than a Mnemosyne changelog round (e.g. an
adoption-round counter), downgrade the gate per workspace:

```toml
[commit_ledger]
severity = "warn"   # | "info" | "reject" (default)
```

`reject` (the default) fails the exit code on any missing cited round —
correct when the workspace authors its own `append-changelog-entry`
ledger. `warn` / `info` still print the drift line but do not gate the
exit code, for a pure consumer ledger whose `(R<n>)` labels are not
Mnemosyne changelog rounds. The diagnostic is never silenced; only the
gating changes.

**Atomic store populated from the upstream spec.** Each spec section
becomes an `AtomicSection`; each test case in a conformance catalog
becomes an `InventoryEntry` (Phase 1A) when the id shape fits. Use
`section_id` slugs that stay stable across revisions (e.g.,
`scxml-3.13` rather than `3.13`, so a future spec restructure does not
silently re-key 30K citations).

**`parent_doc` is a logical label, not a file path (post-R400, R409).**
`AtomicSection.parent_doc` is a mandatory, non-empty, medium-neutral L0
field naming the section's **logical owning document / namespace** (a
grouping key — e.g. `scxml`, `mesh`, `wire`, or `mnemosyne-design`). Since
R400 retired the markdown render, it is **not** a filesystem path and is
**not** validated against any file; the render-era value `docs/GENERATED.md`
is a harmless stale leftover. It is **not provenance** — which upstream the
section mirrors lives in `normative_excerpt.anchor_url` +
`[workspace.spec_source]`, not here. Re-bind with `set-section-parent-doc`
(cosmetic; nothing cross-validates `parent_doc` between stores, so stale
leftovers may be grandfathered). It cannot be cleared (mandatory non-empty)
and is **not** on a deprecation roadmap.

**Anchor the vendored quote at section creation.** Carry the
`normative_excerpt` inline in the `import-sections` manifest — its
`anchor_url` + `source_revision` are the section's authored upstream
identity (which spec section + revision it mirrors):

```json
[
  { "section_id": "scxml-3.13", "parent_doc": "docs/spec.epub",
    "title": "Selecting Transitions",
    "normative_excerpt": {
      "text": "…verbatim spec text…",
      "anchor_url": "https://www.w3.org/TR/scxml/#event",
      "source_revision": "2015-09-01" } }
]
```

**`text` is an EPUB-projected cache (R403).** It is *not* frozen: the
verbatim section text is a derived cache of the committed EPUB. Extract
it with `medium-forge` (emits an `epub-anchor-map/v2` carrying per-section
`text` + `text_sha256`), then project it into the store:

```bash
mnemosyne-cli import-epub-excerpts --anchors out/anchors.json
```

`import-epub-excerpts` refreshes `text` + `text_sha256` on each section
that already carries an excerpt, **preserving** the authored `anchor_url`
+ `source_revision` (store-side identity, not EPUB content). The hash
lets `report-excerpt-hash-backfill` and (future) content-drift scans
re-hash the cached string offline. To model a *different* spec revision,
still supersede the existing Section (`set-section-decision-status
--status superseded --superseding §<new>`) and create a new Section, so
the audit trail records which revision each excerpt mirrors.

**Spec revision drift as ChangelogEntry stream.** When upstream bumps
the spec, append a ChangelogEntry recording the diff and the impacted
sections:

```bash
mnemosyne-cli append-changelog-entry \
  --entry-id "Rev 2026-05-01" \
  --decision-file /tmp/rev-decision.txt \
  --changes-file /tmp/rev-changes.txt \
  --verification-file /tmp/rev-verify.txt \
  --impact "scxml-3.13,scxml-3.14" \
  --carry-file /tmp/rev-carry.txt
```

`AtomicSection.normative_excerpt.text` is an **EPUB-projected cache**
(R403), refreshable via `import-epub-excerpts`; the authored `anchor_url`
+ `source_revision` pin which upstream section + rev it mirrors. The
workspace-wide `[workspace.spec_source].revision` is the *current* rev
label. The ChangelogEntry stream records *when* the workspace moved
between revs; per-Section excerpts capture *what* the section said at the
rev it was anchored at. T2 frozen-ledger semantics apply to the entry
audit half — rev-bump records are permanent.

**Content-integrity drift (`validate-content-drift`, R404).** `text_sha256`
is the offline revalidation anchor: the mutate API guarantees
`sha256(text) == text_sha256` at write time, so a later divergence means
the cache was edited out-of-band (a direct sidecar-JSON edit). The scan
re-hashes every excerpt offline — no EPUB, no re-extraction:

```bash
mnemosyne-cli validate-content-drift            # default [content_drift].severity = reject
mnemosyne-cli validate-content-drift --severity warn --json
```

```toml
[content_drift]
severity = "reject"   # | "warn" | "info" (default reject)
```

Defaults to `reject` (a cache diverging from its own hash is corruption,
never a legitimate intermediate state — contrast `[spec_drift]`'s `warn`,
where a trailing rev during partial migration is expected). Empty-hash
excerpts are *unrevalidatable* (not yet projected from an EPUB): they are
counted for context but never gate — that work-list is owned by
`report-excerpt-hash-backfill`, resolved by `import-epub-excerpts`.

**EPUB is the content source.** `text` + `text_sha256` are projected from a
committed EPUB by `import-epub-excerpts` (which preserves the authored
`anchor_url` + `source_revision`); the EPUB-file pin (`epub_path` /
`epub_sha256`) anchors the source itself. Choose the extraction granularity
with `medium-forge --text-scope` (see EPUB_SSOT_RUNBOOK.md).

**Symbol-level binding (record-only).** `Binding.symbol` accepts
an opaque language-agnostic identifier and is preserved in the store
for project-side audit tooling. The validator's set-equality check is
file-only (RFC-002 FR-3 deferred — language-aware enforcement requires
LSP / treesitter wiring outside Phase 0 paradigm).

**Intentional carries for spec drift.** When an upstream spec *removes*
a section but the workspace must keep citing it (e.g., preserving a
historical compatibility comment), use the existing
`[[orphan_ledger]] kind = "code_citation"` row.

**Markdown prose is cite-able.** The `comment_only` filter strips
code-fenced blocks before scanning, but prose lines outside fences are
scanned the same as code comments — so prose citations in spec
markdown (e.g., a paragraph inside `SCXML.md` referencing `§3.13`)
participate in the validate-code-refs gate. Code fences inside the
prose remain exempt.

**`verifies` binding granularity ≤ source granularity (R426).** Do not
bind a `verifies` edge at finer granularity than your authoritative
test metadata supports. If the test's own declaration (e.g. the W3C
`metadata.txt` `specnum`) is section-granular, bind to that section —
not to its sub-sections. Claiming finer precision than the source
supports is the structural root of *blanket-binding* (one test stamped
across sibling sub-sections it does not exercise; in the originating
field report, 84/126 bindings were wrong this way while every
existence gate stayed green). Two machine fences back this rule, both
opt-in: `severity_blanket` (one artifact `verifies`-bound to >1
section) and `[verifies_catalog]` + `validate-verifies-linkage`
(deterministic check against a consumer-generated catalog of declared
targets; a child-of-declared binding is flagged `finer_than_declared`).
The catalog is consumer-generated — Mnemosyne takes the neutral
`verifies-catalog/v1` JSON (`entries[] = { file, symbol?, section_ids }`),
never format-specific parsers. Optionally pin the catalog file with
`[verifies_catalog].sha256` (R428): every load re-hashes the file and a
mismatch fails loudly — the catalog is the authority input of the
catalog-live confirmed branch, so it carries the same tamper/drift
evidence as `epub_sha256`. Re-pin on each legitimate catalog change.

**Verification method precedence.** Where authoritative metadata
exists, the deterministic linkage check is *primary*; model
(fresh-context LLM) review is for *discovery* and for claims with no
deterministic ground truth (e.g. `implements` bindings, tests without
metadata). Measured in the field: deterministic catalog check 0
errors; single model verdicts ~12–25% error. Model review found the
blanket-binding problem — use it to hunt; let the catalog be the
authority.

## Narrative facts (multi-axis, Round 430)

Schema v12 adds two top-level store collections for perspectival
(multi-axis) facts — the Phase 1 narrative substrate:

- **`frames`** — the epistemic-frame registry, keyed by frame id.
  `ground-truth` is a non-privileged entry like any other frame; a
  believed-fact and the corresponding actual-events fact are *distinct
  facts on distinct axes, both true, never cross-validated*.
- **`narrative_facts`** — append-only claims, keyed by fact id. Each
  fact holds exactly one `frame`, a per-claim `claim`, a canon-time
  extent in structure-section refs (`canon_from` + optional
  `canon_to`), `evidence` (≥ 1 section refs, fail-loud), recorded
  `conflicts_with` assertion edges, and optional in-frame succession
  (`supersedes_in_frame`, same frame enforced — cross-frame
  disagreement is data, never succession). A superseded belief's
  effective end derives from its successor's `canon_from`; nothing is
  written back. An optional `quote` carries `quote_sha256`, computed
  at write time (offline drift detection, the content-drift pattern).

Authoring routes through `import-facts --manifest` (bulk, one atomic
transaction, forward refs within the manifest legal) or `add-frame` /
`add-fact` / `add-fact-conflict` — both fact paths share one builder,
so the invariant set cannot diverge. Frames are sparse: a fact absent
from a frame is *unrecorded*, not false.

### Continuity gate (`[continuity]`, Round 431)

`validate-continuity` evaluates the recorded conflict edges
frame-scoped: a SAME-frame pair whose derived canon extents co-hold at
some point is a violation; a CROSS-frame pair is data and never gates.
A fact's effective end is derived (stored `canon_to`, cut by any
in-frame successor's `canon_from`); a stored end that outlives a
successor's start is a `succession_contradiction`.

Canon order is **declared, never inferred**: point
`canon_order_path` at a consumer/medium-adapter-generated
`canon-order/v1` JSON (`{ "edges": [["ch-1","ch-2"], …] }` — a partial
order; a chapter chain for a linear novel, a quest DAG for a game;
cycles reject at load). Pairs not comparable under the declaration are
surfaced as `unordered_pairs`, never gated; equal coordinates need no
declaration. Without the table the gate is off (opt-in).

```toml
[continuity]
canon_order_path = "canon-order.json"
severity = "reject"            # default; warn | info
# canon_order_sha256 = "<64-hex>"  # optional pin; loud mismatch on load
```

### Recorded-population report (`[census]`, Round 979)

Points at a JSON report your own gate writes when it recounts the
corpora/artifacts your ledger makes claims about. Two things read this
one key, which is why it is declared here and not in either of them:
the gate that blesses the report, and
`append-changelog-entry --record-census`, which fills an entry's
`population_census` from it.

The field exists because of a measured failure mode: a round needs a
baseline, finds no program to run, and takes the number out of an
earlier round's sentence — after which a count that was true when
written is inherited long past the point where it stopped being true.
`--record-census` is a **boolean**; there is deliberately no flag,
anywhere, through which a count can be typed. Without the table the
flag has nothing to read and says so (opt-in).

```toml
[census]
report = "claudedocs/population-census.json"
```

The report is a JSON array of
`{axis, left_label, left, right_label, right}` under an `axes` key —
the serialization of the store's own `PopulationCensus`, so the file
format and the field cannot drift apart. What the axes ARE is yours:
the substrate holds the shape and never decides which questions your
population should be counted by.

## What stays fixed

The store entity types — Section / CrossRef / ChangelogEntry /
FrozenList / InventoryEntry / ConfirmationEvent / Frame /
NarrativeFact — are **not** configurable. They are the universal
primitives the validator + mutate API + cascade engine all build on;
`[schema]` configures naming conventions (citation tokens, entry-id
prefix, section-id shapes) over them, never the shapes themselves.
InventoryEntry was added in Phase 1A (Round 273) for stable-id
citation hygiene; ConfirmationEvent in R416 (max-rigor confirmation);
Frame / NarrativeFact in Round 430 (Phase 1 narrative facts). Further
medium-specific entity types (`Character` / `Location` / `Faction`)
remain consumer-pull scope.
