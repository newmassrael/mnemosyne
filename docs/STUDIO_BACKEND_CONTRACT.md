# Mnemosyne Studio — in-tree backend contract (design)

Status: DESIGN ONLY (nothing built). Defines the seam between the pinion
Studio frontend and the Mnemosyne crates so the two proceed in parallel
and "request to pinion" vs "request to backend" is unambiguous.

Decisions locked by the owner (2026-06-07):
- The Studio (web=WASM and native, both pinion, scene-as-data) is a
  pinion application — not a webview. Avoids the path-A architectural
  violation; the path-B layout cost is accepted because pinion is
  owner-driven.
- Viewer first (read-only). Editor is a later phase.
- The Studio app lives in-repo as a SEPARATE workspace (`studio/`, the same
  pattern as `bench/`): its own `[workspace]`, depending on the core read
  crates (path) + pinion (the GUI framework). Core crates stay GUI-free
  (crate-level invariant); the root workspace's build/CI never compiles pinion.
- Missing frontend capability is filed as a request to pinion.

---

## 1. Scope

This backend exposes Mnemosyne's read projections to the Studio frontend
(L3 "Studio UI" in the North-Star 4-layer hexagonal model).

In scope (Phase 1, viewer): read projections over the whole atomic store
— changelog timeline, section browser + detail, cross-ref graph,
coverage / binding-migration / drift dashboards, spec-map, inventory,
term search.

Out of scope here (later phases, named so the contract stays stable):
- Phase 2 — editor: native goes through the in-process `mnemosyne-atomic`
  primitives (the dogfood write path); web goes through the converged
  `mnemosyne-server` write pipeline (same proposal→gate→audit semantics
  over the wire). Previewed in §9.
- Phase 3 — frame/branch selector, rich EPUB-body structured render,
  narrative multi-EPUB. Forward-compat reserved in §8, not built.

---

## 2. The ground truths this design depends on

Verified in code (rev `04c9143`). The whole design follows from them.

GROUND TRUTH 1 — the real read API is the query/validate lib fns over the
JSON `AtomicStore`. The CLI and MCP both call them directly; there is no
read "service" abstraction.
- CLI: `mnemosyne_query::{build_envelope, section_by_id,
  related_sections_with_atomic, changelog_entries_for_section,
  query_term}` + `mnemosyne_validate::code_refs::classify_coverage`
  (`crates/mnemosyne-cli/src/main.rs`).
- Store load: `mnemosyne_atomic::AtomicStore::load(
  &mnemosyne_ops::cascade::resolve_sidecar(...))`.

GROUND TRUTH 2 — the real write path is the `mnemosyne-atomic` primitives
over the same JSON `AtomicStore`. The CLI and MCP both call them
directly; the frozen-ledger / append-only / supersede invariants live
INSIDE the primitives.
- CLI: `atomic_cli::cmd_append_changelog_entry` / `cmd_set_section_intent`
  / `cmd_add_section_binding` (`main.rs:168/202/229`).
- MCP: `atomic::append_changelog_entry(...)` (`mnemosyne-mcp/src/main.rs`).

GROUND TRUTH 3 — `mnemosyne-server` (`proposal → gate → audit`) is
FOUNDATION, not legacy. ARCHITECTURE.md §3 names that pipeline a Layer-0
CORE property ("every mutation = reviewed txn") + the CQRS write side, and
§6.1 protects the crate by name. It is INCOMPLETE today — gate Tier 2/3 are
stubs (`tier2_phase0_stub_accepts_all` / `tier3_phase0_stub_no_warnings`)
and `commit_storage` still writes RocksDB (the pre-convergence-D state; the
target is log-append, RocksDB-free) — and has no production consumer yet
(CLI/MCP write via the in-process atomic primitives). Those are "converge
it" conditions (convergence C/D), NOT "avoid it." The Studio **web** backend
is exactly convergence C/D's first consumer: the converged `mnemosyne-server`
daemon hosts the warm read-side projections (C) + the log-append write
pipeline (D). §11 details this. (An earlier draft of this contract wrongly
told the Studio to avoid server; corrected after a near-deletion that
violated §6.1.)

---

## 3. Principles

1. **Read = pure projection.** Every read is a function of the current
   `AtomicStore` snapshot. ZERO new authoritative state. SSOT stays the
   JSON atomic store.
2. **Single write model.** Phase 2 mutation bottoms out in the
   `mnemosyne-atomic` primitives over the JSON log — the dogfood SSOT.
   Native calls them in-process (the exact path CLI/MCP use); web calls the
   converged `mnemosyne-server` pipeline (proposal→gate→audit) that wraps
   the same primitives. Either way the editor inherits the identical
   frozen-ledger enforcement and the one SSOT — no second write model, no
   second gate.
3. **Reuse, do not reinvent, do not re-abstract.** Reads wrap existing
   query/validate fns; writes bottom out in existing atomic primitives.
   Native adds no service layer (links them directly, as CLI/MCP do); web
   reuses the existing `mnemosyne-server` pipeline rather than a parallel
   one. The lib fns ARE the read SSOT; the primitives ARE the write SSOT.
4. **Native links the libs directly; only web needs a remote.** See §4.
5. **Frame-ready, single-frame now.** v1 serves the current store (one
   epistemic frame). The contract reserves an optional frame selector
   (§8) so the North-Star multi-frame model slots in additively.

---

## 4. Architecture / the seam

```
                 pinion Studio app (scene-as-data)
                /                                  \
        native target                          web / WASM target
   links the crates in-process,           cannot do file IO in WASM →
   calls lib fns + atomic                 talks over gRPC-web/HTTP to
   primitives DIRECTLY                    the converged mnemosyne-server
   (no new crate, no daemon)              daemon (convergence C/D)
        \                                  /
         mnemosyne-query / mnemosyne-validate (reads)
         mnemosyne-atomic primitives        (writes, Phase 2)
         over the JSON AtomicStore  =  the SSOT
```

- Native pinion Studio = the existing library crates ARE the in-tree
  backend. It opens the store (`AtomicStore::load` via
  `mnemosyne-ops::cascade::resolve_sidecar`), calls query/validate for
  reads and atomic primitives for writes. No backend crate to build.
- Web/WASM pinion Studio needs a network boundary (no local file IO in
  WASM). That daemon is the converged `mnemosyne-server` (convergence
  C/D): it hosts the warm read-side projections (reads) + the
  proposal→gate→audit→log-append write pipeline (writes) behind
  gRPC-web/HTTP. Built when the web target starts — and building it IS
  finishing convergence C/D (§11), not a parallel adapter.
- Value-equality is trivial here (not an elaborate invariant): native and
  web bottom out in the SAME fns over the SAME store, so equal inputs give
  equal outputs by construction. A smoke test pins it once the remote
  adapter exists.

---

## 5. Crate placement

- **Native: a new app crate in the `studio/` workspace; NO new BACKEND crate.**
  The Studio app crate (`studio/`) depends on the existing read crates
  (`mnemosyne-query` / `mnemosyne-validate` / `mnemosyne-atomic` /
  `mnemosyne-ops`, via path) + pinion — it links them exactly as the CLI does,
  so the read surface needs no new backend crate. `studio/` is a separate
  in-repo workspace (like `bench/`): core crates stay GUI-free and the root
  workspace's build/gates never compile pinion.
- **Web (later): the converged `mnemosyne-server` daemon** (convergence
  C/D), NOT a new parallel crate. server is the designated warm daemon host
  (ARCHITECTURE §4) and the Studio web is its first consumer. Building the
  web backend = finishing server: gate Tier 2/3 + switch `commit` to
  log-append (RocksDB-free, convergence D) + add the warm read-side
  projection service (convergence C) + the gRPC-web/HTTP face. Authoring
  stays RocksDB-free (reads project from the log; RocksDB read-index at
  scale, convergence B).

This keeps the read side deliberately minimal (no `StudioRead` trait, no
"BFF" facade): the libs are already the read API, and the write side reuses
the existing server pipeline rather than a parallel one.

---

## 6. The read surface (Phase 1)

There is no new trait — the "surface" is just which existing fns the
frontend calls (native) / the remote adapter exposes (web). Each row maps
to an existing implementation; no projection logic is added or duplicated.

| Studio screen | existing fn (over `AtomicStore`) |
|---|---|
| changelog timeline | iterate `store.changelog_entries` (full ledger) |
| section browser | `mnemosyne-query` section views |
| section detail pane | `build_envelope(store, id)` |
| cross-ref graph | `related_sections_with_atomic(...)` |
| coverage dashboard | `classify_coverage(snapshot)` |
| spec-map (overlay) | `report-spec-map` logic + `epub_locator` |
| binding migration | `report-binding-migration` logic |
| drift status | `scan_content_drift` / `scan_spec_drift` |
| inventory | store inventory projection |
| term search | `query_term(store, q)` |

`list_changelog` is the only read without a per-section equivalent today
(query exposes `changelog_entries_for_section`); it iterates the full
ledger — a trivial wrap, no new logic. If the web daemon needs a stable
aggregate entry point, add a small `mnemosyne-query` fn (one home for the
logic), so native and web share it.

---

## 7. DTO contract (web target only)

Native passes the query/validate view types in-process (no wire). The web
adapter needs serializable wire DTOs.

- Start by reusing the query/validate view types directly (`SectionView`,
  `QueryEnvelope`, `RelatedSections`, `CrossRefView`, `ChangelogEntryView`,
  `TermHit`, `CoverageReport`, the spec-map / binding-migration rows) —
  they are already `serde::Serialize` (the CLI `--json` mode emits them).
- Contract version string `studio-read/v1` travels with the remote schema
  (the `epub-anchor-map/v2` precedent): additive fields bump nothing; a
  breaking shape bumps the version.
- Anti-corruption boundary: because the wire contract is consumed by a
  separate repo (pinion), introduce a thin studio-owned wire-DTO layer the
  moment an internal view type needs to change faster than the published
  contract. Until then, direct reuse avoids a 1:1 mapper. (For a
  lifetime/portfolio project the boundary is the textbook end-state; defer
  only the boilerplate, not the principle.)
- Error model: a typed error enum. Not-found is a typed variant
  (`SectionNotFound(id)`), never an empty success.

---

## 8. Frame-scoping forward-compat (North Star)

facts are multi-axis / perspectival; `branch` is the epistemic-frame
engine; validation is frame-scoped. v1 is single-frame. To stay
frame-ready WITHOUT building frames:

- Each read is specified as reading "the active frame".
- A future optional `frame: Option<FrameId>` (default = main) is added to
  each fn additively — same discipline as serde-default optional fields.
  No v1 caller breaks.
- DTOs gain no frame field in v1; a frame-aware contract bumps to
  `studio-read/v2`.

Reservation, not implementation. Do not build frames until the narrative
consumer arrives (the multi-frame proving ground).

---

## 9. Phase 2 preview — the editor (NOT this contract)

Recorded so the read contract stays stable when the editor lands.

- The editor adds NO new write path. It calls the `mnemosyne-atomic`
  primitives — the exact path CLI and MCP use — so it goes through the
  same frozen-ledger / append-only enforcement and writes to the real
  SSOT.
- Editable surface == the atomic primitive set, mapped to structured
  forms: `append_changelog_entry`, the `set_section_*` setters,
  `add/remove_section_binding`, `set_section_binding_kind`, inventory ops,
  `set_changelog_publishable_*`, `redact_term`, `import_*`.
- Frozen by construction: anything with no primitive (e.g. a frozen
  Round-N entry body) has no form — the UI cannot express it. The frozen
  ledger is enforced by the ABSENCE of a primitive, not a UI check.
- Primitive errors (frozen reject, divergent-manifest reject, invariant
  violations) surface verbatim. A citation-hygiene helper validates
  `Round NNN` existence before submit (R255).
- Web editor: the converged `mnemosyne-server` daemon exposes the write
  pipeline (proposal→gate→audit→log-append) over the wire; enforcement
  happens server-side in the gate + primitives.

---

## 10. pinion-side responsibilities (what to request from pinion)

- Structured widgets — tree, table, node-link graph, timeline, forms.
  pinion's native strength (scene-as-data); the backend feeds DTOs.
- Rich spec-body structured render (full EPUB body with layout) = the
  path-B piece. Deferrable: the viewer starts by rendering
  `normative_excerpt.text` + structured facts; the full EPUB + CFI
  overlay is a later request to pinion once the body renderer matures.

---

## 11. The Studio web backend completes convergence C/D

The web backend is not a side decision to route *around* the persistence
fork — it is the consumer that *completes* the fork's convergence.

- The fork (R161) is ~80% converged into CQRS event-sourcing: log (JSON
  `AtomicStore`) = SSOT, RocksDB = derived read-index, Salsa cascade =
  incremental projection. A done; B landed-half (live query-routing
  scale-gated); C half + designed; D designed. See ARCHITECTURE.md §4-5 +
  the persistence-fork memory.
- The converged `mnemosyne-server` daemon = the web backend: writes go
  proposal→gate→audit→**log-append** (RocksDB-free authoring, convergence
  D — this is where gate Tier 2/3 get finished and `commit` stops writing
  RocksDB); reads project from the log (flat scan at dogfood scale; the
  RocksDB read-index at corpus scale, convergence B). The log stays the
  single SSOT throughout.
- So the web phase is genuinely large (it finishes C/D) and is gated on
  actually needing the web target. **native viewer first** sidesteps all of
  it: native links the libs over the JSON store in-process (Path X),
  fork-independent, buildable today.

---

## 12. Implementation phasing (native-first, demand-driven)

The read surface is speculative until a frontend consumes it. So:

1. **Native viewer first** — needs no new BACKEND crate (the `studio/` app crate
   links the existing read crates). As the first screen
   (changelog timeline + section browser) is built in pinion, it links
   query/atomic and calls `list_changelog` (full-ledger iterate),
   `section_by_id`, `build_envelope`. This is the heart of "the whole
   Mnemosyne" and is unblocked today.
2. Cross-ref graph (`related_sections_with_atomic`).
3. Dashboards (`classify_coverage` + binding-migration + drift).
4. Spec-map overlay tab (reuses the existing spec viewer design).
5. Search + inventory.
6. Web target — only when needed: the converged `mnemosyne-server` daemon
   (convergence C/D) — warm read-side projections + the log-append write
   pipeline + gRPC-web/HTTP; then the Phase-2 editor over it (§11).

Each slice: call the existing fn (native) first; add the remote binding
when the web target reaches that screen.

---

## 13. Open decisions (resolve at build time)

- Web transport: gRPC-web vs plain HTTP/JSON (decide when the web target
  starts; HTTP/JSON is simpler for a WASM client).
- Whether `list_changelog` lives as a new `mnemosyne-query` fn (shared by
  native + web) or stays an inline iterate in each caller (prefer the
  shared fn the moment the web adapter needs it).
- When to introduce the studio-owned wire-DTO anti-corruption layer (§7).
- Convergence C/D scope + timing for the web daemon (§11) — gates only the
  web target; the native viewer is fork-independent.
```
