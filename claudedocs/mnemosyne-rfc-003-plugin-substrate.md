# RFC 003 — Plugin substrate + medium federation

**Author**: mnemosyne maintainer
**Date**: 2026-05-27
**Status**: **Draft — decision document only; implementation deferred per FR-by-FR disposition below**
**Scope**: Phase 0.5 / Phase 1 architecture substrate
**Supersedes**: nothing
**Relates to**:
- RFC-001 response (changelog amend primitive) — resolved by R294-R301
- RFC-002 response (SCE external-spec adapter) — FR-1/FR-2 landed in R303
- Round 304 — `mutate.rs` retirement + `_atomic` suffix drop
- Round 172 narrative priority audit (Phase 1A carry,
 `project_phase_1_narrative_carry.md`)
- Phase 0 closure conditions in `CLAUDE.md` ("Cleanup hard limit"
 section)

---

## TL;DR

1. **Plugin substrate before more medium adapters.** Phase 0 ships
 design_doc as the single dogfood medium. The next two pressure
 vectors — (a) symbol-level validation (FR-3 from RFC-002), (b)
 behavioral spec checking (SCXML / TLA+ class) — both point at the
 same missing piece: an *extension surface* that lets new
 validators / resolvers / spec layers attach without bloating
 core.
2. **Three transport modes, one trait contract.** Plugins can be
 (T1) in-process Rust crates behind Cargo features, (T2) external
 MCP servers consumed via a new MCP client layer, or (T3)
 shell-out CLI invocations. Each plugin category exposes a
 stable Rust trait; backend choice is `mnemosyne.toml` opt-in.
3. **Medium adapter as the unification.** design_doc / narrative /
 protocol-spec / contract are all *media* sitting on top of a
 medium-agnostic core. The same plugin substrate carries both
 (a) extending the existing design_doc medium with optional
 validators and (b) registering a new medium adapter wholesale.
4. **Five FRs, three phase tiers.** FR-1 (transport) and FR-2
 (spec-layer plugins, FR-3-of-RFC-002 absorbed) are Phase 0.5
 land candidates. FR-3 (spec federation) and FR-4 (external MCP)
 are Phase 1+. FR-5 (medium adapter pattern) is the Phase 1A
 narrative-adapter substrate — decision deferred until Phase 0
 closure conditions hit T3 reject = 0 / orphan = 0 / round-trip
 N/N steady-state.
5. **No code lands from this RFC.** This document freezes
 architectural intent. Implementation rounds happen per-FR after
 user trigger.
6. **Five risks accepted explicitly**, see §7. The most consequential
 is **plugin lifecycle ownership** — the moment third-party
 plugins exist, mnemosyne's "single-binary 5-min setup" promise
 has to be re-stated for the *core*, and the plugin layer needs
 its own contract.

---

## 1. Context — what forced this RFC

Three pressure vectors converged in the May 2026 session:

- **FR-3 from RFC-002 (symbol-level enforcement)** — deferred to
 "Phase 1+" with a "paradigm shift" framing. Honest, but the framing
 implied no plan. A plugin substrate replaces "paradigm shift" with
 "additive plugin layer, opt-in per language".
- **SCXML-class behavioral spec validation** — user-raised in the
 same session. Same shape as FR-3: a validator that the core does
 not own but that should be wirable. Without a plugin substrate,
 each such validator pollutes core with medium-specific code.
- **Narrative adapter (Round 172 priority audit)** — Phase 1A entry
 trigger. The audit ranked fictional-medium adapter first with
 6.00 / 3.00× margin. Attempting to land that adapter into
 today's core would require either (a) hardcoding narrative
 schema into the medium-agnostic layer (anti-pattern) or
 (b) inventing a plugin substrate ad hoc under deadline pressure.

The decision: **freeze the substrate now, before any of the three
trigger** so that all three (and future media) plug into the same
contract.

---

## 2. Goals

1. **Preserve the 5-minute setup promise for the design_doc
 dogfood.** A user who installs mnemosyne and runs
 `validate-workspace` against an existing markdown set should see
 zero behavior change. Plugins are *additive opt-in*.
2. **One contract per plugin category.** All symbol resolvers
 implement the same trait; all behavioral spec validators
 implement the same trait. Transport (in-process / MCP / CLI) is
 invisible to the core.
3. **Reproducible validation.** Plugin output participates in the
 atomic store's audit trail; environment-dependent plugins
 (rust-analyzer version, mypy strict mode) must declare their
 version surface so CI / local divergence is detectable.
4. **No core schema mutation per plugin.** Plugin-emitted findings
 attach to existing atomic-store fields (`Implementation.symbol`
 already exists per R259); plugin-specific entities — if any —
 land via the medium adapter pattern, not via core schema
 expansion.
5. **Medium adapter as a first-class plugin category.** A new
 medium (narrative / protocol / contract) is a plugin bundle:
 schema extension + validator set + emitter rules + mutate
 primitive set.

## 3. Non-goals

1. **Not a generic VM / WASM plugin sandbox.** Plugins are
 either trusted Rust crates compiled in, or trusted external
 processes the user explicitly configured. Mnemosyne does not
 attempt to sandbox arbitrary plugin code.
2. **Not a marketplace.** No registry, no discovery service, no
 versioning beyond Cargo semver / OS package manager. Plugin
 ecosystem health is *user's* responsibility.
3. **Not a dynamic-load mechanism.** Adding a plugin requires
 either recompile (T1) or `mnemosyne.toml` edit + new external
 binary install (T2 / T3). No `.so` / `.dll` runtime loading.
4. **Not a replacement for the atomic store as single source of
 truth.** Plugins read from and *advise on* the store; only
 mutate primitives (core-owned) write. Plugin findings become
 store-visible only via core mutate calls.
5. **Not a federation-layer for multi-org cross-repo.** External
 spec federation (FR-3) is git-clone-and-mount; not a real-time
 distributed-graph protocol.

---

## 4. Design

### 4.1 Layered architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ 6. CLIENT SURFACE                                                │
│    Claude / Cursor / Aider │ Studio UI (Phase 1B)               │
│    ↕ MCP (mnemosyne-mcp server, existing)                       │
├─────────────────────────────────────────────────────────────────┤
│ 5. MEDIUM ADAPTERS — "어떤 종류의 글이냐"                        │
│  ┌───────────┬──────────┬───────────┬──────────┬─────────────┐ │
│  │design_doc │narrative │ protocol  │ contract │ research    │ │
│  │(Phase 0)  │(Phase 1A)│(Phase 1+) │(plugin)  │ paper(plugin)│ │
│  └───────────┴──────────┴───────────┴──────────┴─────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│ 4. VALIDATOR + BINDING PLUGINS — "정합성을 어떻게 검사하나"      │
│  T1 orphan │ T2 frozen │ T3/T4 style │ code-ref │ external-spec │
│  ─────────────────────────────────────────────────────────────  │
│  symbol-resolver │ behavioral-spec │ continuity │ asset-binding │
│  (FR-2 plugin slot)                                              │
├─────────────────────────────────────────────────────────────────┤
│ 3. MEDIUM-AGNOSTIC CORE — 모든 medium 이 공유                    │
│  • atomic entity store    • mutate API w/ invariants            │
│  • cross-ref graph        • query API                            │
│  • cascade triggers       • frozen-ledger semantic              │
│  • workspace round-trip   • emitter (per-medium pluggable)      │
├─────────────────────────────────────────────────────────────────┤
│ 2. TRANSPORT LAYER (this RFC's FR-1)                            │
│  in-process Rust │ MCP client │ CLI shell-out │ git federation  │
├─────────────────────────────────────────────────────────────────┤
│ 1. STORAGE                                                       │
│  atomic.json (per medium, per branch)                           │
│  GENERATED.md (derived view, per-medium emitter)                │
│  bi-temporal index (Phase 1A) │ remote spec mounts (FR-3)       │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Transport abstraction (FR-1)

A plugin contributes one or more *capabilities*; capability surface
is a Rust trait owned by the validator or binding category. Each
capability has three possible backends:

```rust
// Example: symbol resolver capability surface
trait SymbolResolver: Send + Sync {
    fn version_surface(&self) -> VersionSurface;
    fn resolve_symbol_at(
        &self,
        file: &Path,
        line: u32,
    ) -> Result<Option<String>, ResolverError>;
}

// Backend 1: in-process (Cargo feature `tree-sitter-rust`)
struct TreesitterRustResolver { /* ... */ }
impl SymbolResolver for TreesitterRustResolver { /* ... */ }

// Backend 2: external MCP (no Cargo feature; runtime dispatch)
struct McpResolver { client: McpClient }
impl SymbolResolver for McpResolver { /* ... */ }

// Backend 3: shell-out CLI
struct CliResolver { command: PathBuf, args: Vec<String> }
impl SymbolResolver for CliResolver { /* ... */ }
```

`mnemosyne.toml` selects backend per capability:

```toml
[plugins.symbol_resolver.rust]
transport = "in-process"    # or "mcp" or "cli"
# In-process backends are named by Cargo feature:
backend = "tree-sitter-rust"

[plugins.symbol_resolver.python]
transport = "mcp"
command = ["python", "-m", "mnemosyne_py_resolver", "--stdio"]
# version_surface query is part of MCP handshake

[plugins.symbol_resolver.go]
transport = "cli"
command = ["gopls"]
output_parser = "gopls_v0_15"
```

When a capability has no configured backend for a given language /
context, the core falls back to the existing language-agnostic
behavior (file-level only). **No language is ever blocked by a
missing plugin.**

### 4.3 Validator-class vs binding-class plugins (FR-2)

Two plugin categories cover all foreseen Phase 0.5 / Phase 1
extension surfaces:

**Validator-class** — reads the atomic store + plugin-specific
input, emits zero or more `ValidationFinding` records. Examples:
- `behavioral_spec_validator` — parses SCXML / TLA+ from a
 referenced file, checks reachability + completeness.
- `continuity_validator` — narrative-medium plugin; checks
 cross-scene prop / appearance / timeline consistency.
- `contract_validator` — checks Hoare-style pre/post on
 implementations.

**Binding-class** — extracts (atomic-store-entity, code-or-asset)
relationships from external sources, surfaces them as findings the
core can advise the user to record via existing mutate primitives.
Examples:
- `symbol_resolver` — answers `resolve_symbol_at(file, line)` for
 the existing R260 set-equality validator; what RFC-002 FR-3 became.
- `asset_binding` — narrative-medium plugin; binds a Scene to an
 image / audio file with hash + revision.
- `external_spec_anchor` — federates the R303 normative_excerpt
 mechanism to point at a remote spec store (see FR-3).

### 4.4 External spec federation (FR-3)

A separate-repo atomic store can be *mounted* read-only:

```toml
[external_specs.shared_protocol]
source = "git@github.com:org/shared-spec"
revision = "v2.3.1"     # immutable pin
atomic_store_path = "docs/.atomic/workspace.atomic.json"
prefix = "ups-"        # local citations: §ups-session-establishment
mount_strategy = "git-clone-cache"  # or "submodule" or "local-path"
```

Local `§ups-*` citations resolve via:
1. local atomic store lookup (no `ups-` prefix expected here)
2. fall through to `[external_specs.shared_protocol]` mounted store
3. reject if not found in either

The pin (`revision`) is mandatory and recorded in the audit trail.
Bumping the pin is an atomic-store mutation in its own right
(reuses R296 publishable_override_ledger pattern semantics).

### 4.5 External MCP tool integration (FR-4)

Distinct from FR-3 (which mounts a *spec store*). FR-4 is for
external systems with live data: ticket trackers, observability
dashboards, schema registries.

```toml
[plugins.external_ref.linear]
transport = "mcp"
command = ["linear-mcp", "--api-key-env", "LINEAR_KEY"]
citation_pattern = "LIN-\\d+"   # local code can cite // LIN-1234

[plugins.external_ref.grafana]
transport = "mcp"
command = ["grafana-mcp", "--url", "${GRAFANA_URL}"]
citation_pattern = "GRAF-[\\w-]+"
```

Validator runs the same set-equality pattern as R260 code-refs
but with the external MCP as the "valid set" oracle. Stale
citations (e.g., Linear ticket closed) surface as decay warnings
via the R266 cascade trigger pattern, not rejections.

### 4.6 Medium adapter (FR-5)

A medium adapter is a *bundle* registered at startup:

```rust
trait MediumAdapter {
    fn id(&self) -> &str;              // "design_doc", "narrative"
    fn schema_extensions(&self) -> Vec<SchemaExtension>;
    fn validators(&self) -> Vec<Box<dyn Validator>>;
    fn mutate_primitives(&self) -> Vec<Box<dyn MutatePrimitive>>;
    fn emitter(&self) -> Box<dyn Emitter>;
}
```

The current design_doc behavior is refactored into
`DesignDocAdapter` (no behavior change; pure restructuring).
Future adapters register alongside:

```toml
[mediums]
default = "design_doc"

[mediums.design_doc]
# default Phase 0 behavior, no extra config

[mediums.narrative]
# Phase 1A adapter — opt-in
bi_temporal = true
branch_model = "git-like"
continuity_validators = ["prop", "appearance", "timeline"]
```

Adapter scope includes (per medium):
- schema fields specific to the medium (e.g., `Scene.story_time`,
 `Character.born`, `Character.died`)
- mutate primitives (`add-scene`, `add-character`,
 `set-scene-story-time`)
- validators (continuity, character-arc completeness)
- emitter rules (narrative GENERATED.md formats differently than
 design_doc)

The medium-agnostic core continues to own: atomic store storage,
cross-ref graph, frozen-ledger T2, transactional mutate semantics,
cascade trigger machinery. **Adapters never bypass core mutate
primitives.**

### 4.7 Narrative-scope additions (FR-5 sub-items)

Narrative adapter requires three core extensions that other media
may also benefit from:

- **Bi-temporal index** — every atomic-store mutation already has
 author-time (`applied_at_transaction_time`). Add story-time as a
 second axis. Query API gains `--as-of-story-time` and
 `--as-of-author-time` flags. design_doc medium ignores
 story-time; narrative requires it.
- **Branch model** — current Active/Superseded chain is single-
 line. Branch model = multi-head atomic store with branch
 ancestry. Each branch is a separate sidecar JSON; merge / diff
 / cherry-pick primitives operate on branches. Reuses R296
 publishable_override_ledger pattern for cross-branch term
 redaction audit.
- **Continuity validator framework** — generalization of T1/T2
 to cover cross-scene predicates. Pluggable per-medium per FR-2.

These three lift the core; FR-5 medium adapters consume them.

---

## 5. FR breakdown

| FR | Title | Phase | Trigger |
|---|---|---|---|
| FR-1 | Transport abstraction (in-process / MCP / CLI) | 0.5 | post Phase-0 closure |
| FR-2 | Validator + binding plugin categories | 0.5 | piggybacks on FR-1 |
| FR-3 | External spec federation (git-mount remote atomic store) | 1+ | first external dogfood with shared spec |
| FR-4 | External MCP tool integration (Linear / Grafana / etc) | 1+ | user-requested |
| FR-5 | Medium adapter pattern (design_doc refactor + narrative adapter substrate) | 1A | Round 172 priority audit trigger |
| FR-5.1 | Bi-temporal index in core | 1A | narrative-adapter prerequisite |
| FR-5.2 | Branch model in core | 1A | narrative-adapter prerequisite |
| FR-5.3 | Continuity validator framework | 1A | narrative-adapter; depends on FR-2 |

---

## 6. Disposition

### FR-1 — DEFER to Phase 0.5

**Trigger**: Phase 0 closure conditions (T3 reject = 0, T1 orphan =
0 outside known-stale ledger, round-trip mandatory N/N) reach
*sustained* steady-state across multiple rounds. Today (post R304)
the conditions hold but the run is too recent to declare
sustained.

**First implementation focus**: `SymbolResolver` trait + `Validator`
trait + in-process transport only. MCP / CLI transports added in
follow-up rounds once trait shape proves stable against the
in-process backend.

### FR-2 — DEFER to Phase 0.5

**Trigger**: piggybacks on FR-1. First proof = RFC-002 FR-3
(symbol-level enforcement) shipped as the first validator plugin
under this substrate.

### FR-3 — DEFER to Phase 1+

**Trigger**: first cross-repo dogfood. Not currently in roadmap;
SCE-class use cases (per RFC-002) are the natural pressure
vector. Until then, RFC-002 R303 normative_excerpt sidecar
suffices.

### FR-4 — DEFER to Phase 1+

**Trigger**: user-explicit request for external system citation
(Linear / Grafana / Jira). No current driver; the existing
reference-memory pattern (`reference_design_progress.md`-style)
covers the documentation use case; FR-4 only fires when
*validation* is wanted.

### FR-5 — DEFER to Phase 1A; substrate dependencies (FR-5.1, 5.2, 5.3) deferred to Phase 1A

**Trigger**: Round 172 narrative priority audit recommendation
acted upon. Prerequisites: Phase 0 closure sustained + FR-1/FR-2
landed (so narrative adapter can use plugin substrate from day
one instead of being a one-off carve-out).

---

## 7. Risk register

| # | Risk | Mitigation |
|---|---|---|
| 1 | **Schema instability** — plugins write to atomic store via core primitives; schema changes break plugins | R294 schema_version chain enforced; plugins declare min/max schema_version; mismatch = plugin disabled with diagnostic, not crash |
| 2 | **Trust boundary** — MCP / CLI plugins run with mnemosyne's privileges | Plugin invocation is explicit `mnemosyne.toml` opt-in; no auto-discovery; document supply-chain expectation in `GETTING_STARTED.md` plugin section |
| 3 | **Validation reproducibility** — plugin output may vary by env (LSP version, tool version) | Each plugin's `version_surface()` is recorded in `ValidationFinding`; `validate-workspace` emits a "plugin version manifest" line for CI comparison |
| 4 | **Cascade complexity across federation** — external spec supersession should cascade to local citations | FR-3 mount records `revision` pin; bumping pin runs local R266 decay scan against new spec. Cross-org real-time cascade explicitly out of scope (see §3 non-goal 5) |
| 5 | **Plugin lifecycle ownership** — third-party plugins exist → core "5-min setup" promise must be re-stated | Core (`design_doc` adapter, no plugins) remains the 5-min surface; plugin install is opt-in second step. `mnemosyne --without-plugins` flag forces core-only behavior for debugging |

---

## 8. Carry path — what users do until each FR lands

- **Need symbol-level validation today** (RFC-002 FR-3): record
 `Implementation.symbol` via existing `add-section-implementation
 --symbol`; the file-only set-equality continues to pass; the
 symbol field is *informationally* present for the plugin to
 consume once FR-2 ships.
- **Need behavioral spec validation today**: store spec as
 normative_excerpt (R303) pointing at the spec file; run any
 standalone validator (e.g., `scxml-validator`) as a separate CI
 step. Cross-link to mnemosyne via comment-level `§<section-id>`
 citations.
- **Need external spec reference today**: vendor a copy into the
 local workspace as a normative_excerpt-anchored section.
 Federation (FR-3) is upgrade-only; vendored copies migrate
 trivially.
- **Need narrative-style work today**: not supported. Phase 1A
 substrate must land first; today's atomic store would force
 narrative facts into design_doc-shaped schema = poor fit + future
 migration cost.

---

## 9. Connection to existing carry

- **RFC-001 (changelog amend)** — already resolved by R294-R301.
 This RFC does not revisit; plugin substrate operates on top of
 the publishable/audit split.
- **RFC-002 (external-spec adapter)** — FR-1/FR-2 of RFC-002
 landed in R303. RFC-002 FR-3 (symbol-level enforcement) is
 absorbed into this RFC's FR-2 as the first plugin-substrate
 proof.
- **Round 304** — `mutate.rs` retirement closed the last
 markdown-surgical legacy. The plugin substrate naturally
 inherits the atomic-only world; no plugin will need to operate
 on markdown source files.
- **Round 172 narrative priority audit** — this RFC defines the
 substrate (FR-5) the narrative adapter will plug into, lifting
 the Round 172 carry from "Phase 1+ TBD" to "Phase 1A FR-5
 candidate".
- **`project_phase_1_narrative_carry.md`** memory — update
 recommended: replace "Phase 1+ TBD entry" with "Phase 1A entry
 via RFC-003 FR-5; depends on FR-1/FR-2 landing first".

---

## 10. Open questions

1. **Schema-extension shape for medium adapters** — does each
 adapter get a namespaced schema subtree
 (`AtomicSection.narrative.scene_props`) or a flat extension
 with prefix convention (`AtomicSection.scene_props`)?
 Narrative adapter PoC will force the decision.
2. **Plugin output trust** — when a binding-class plugin claims
 "function `foo` is at `src/a.rs:42`", does the core trust it
 directly, or run a second independent check? In-process Rust
 plugins are trusted; MCP / CLI plugins should arguably be
 trusted-by-config (user explicitly enabled).
3. **Plugin findings in the atomic store** — does
 `ValidationFinding` get persisted to atomic.json (creating an
 audit trail of plugin runs) or stay ephemeral (recomputed on
 each `validate-workspace`)? Probably ephemeral for Phase 0.5;
 persistence is a separate decision.
4. **Federation revision-bump semantics** — should `[external_specs]`
 revision changes auto-run R266 decay scan on local code, or
 surface a "you need to re-validate" warning only? Auto-run
 risks user surprise; warning-only risks decay accumulation.
5. **Narrative bi-temporal storage shape** — bi-temporal indices
 are well-studied in DB literature but mnemosyne's atomic.json
 is a single-file format. Does this remain single-file (with
 indices computed on load) or split per branch / per time
 axis? Decision deferred to FR-5.1 implementation round.

---

## 11. Decision summary

**This RFC decides**:

1. Plugin substrate exists as a Phase 0.5 deliverable, structured
 around (transport abstraction × trait categories).
2. design_doc Phase 0 behavior is the reference floor — no plugin
 changes it.
3. Medium adapter pattern is the unification for
 narrative / protocol / contract / etc. — all future media
 ride the same substrate, no one-off carve-outs.
4. RFC-002 FR-3 (symbol-level enforcement) becomes the first
 plugin-substrate proof, not a separate paradigm shift.
5. Five risks accepted with the mitigations in §7.

**This RFC explicitly does not decide**:

1. When FR-1 implementation starts (waits for Phase 0 closure
 sustained state).
2. Which language gets the first symbol resolver in-process
 backend (Rust most likely, but defer to implementation round).
3. Narrative adapter schema shape (FR-5.1 / 5.2 / 5.3
 dependencies must land first; concrete schema is its own RFC).
4. Plugin lifecycle / registry / distribution model — out of
 scope per §3 non-goal 2.
5. External spec federation transport details (git submodule vs
 git-clone-cache vs local-path) — left to FR-3 implementation
 round.

**Next action**: file as `claudedocs/mnemosyne-rfc-003-plugin-substrate.md`.
Implementation begins per individual FR triggers above; no code
lands from this RFC directly.
