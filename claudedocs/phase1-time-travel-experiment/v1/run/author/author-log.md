# Author log — "The Wend-Pipe of Brae Hollow" (two-age time-travel fact base)

A gate-checked fact base authored top-down from `premise.md` and `author-brief.md`.
One place (Brae Hollow), two canonical ages on one continuous timeline, with early
acts that cause late consequences. Authored as facts, not prose.

## The skeleton (laid first, gate-clean, then filled)

### Two ages + their scene bands (54 scenes total)

- **FOUNDING AGE band (early)** — `sc-01 … sc-26`, 26 scenes, the SHARED SPINE.
  The young thriving hollow: Sera at the Cistern Steps, Halden at the Loom
  Quarter, the bearer arriving with the wend-pipe and walking down to the Wend
  crossing. The band ends at `sc-26` = THE CHOICE (the planting fork).
- **WITHERING AGE band (late)** — 28 scenes, split into two futures from the fork:
  - `planted` road: `sc-27p … sc-40p` (14 scenes)
  - `barren` road: `sc-27b … sc-40b` (14 scenes)

Canon order (`order.json`) makes the early age literally earlier: every Founding
scene `sc-01..sc-26` comes strictly BEFORE every Withering scene on the main
timeline; each road is its own edge-chain from the fork at `sc-26`.

### Three places as entities present in BOTH ages

- `cistern-steps` (the water), `loom-quarter` (the work), `wend-fields` (the living).
- Load-bearing PROP per place, each an entity touched in both ages:
  `cistern-vent`, `boundary-stone`, `gorge-sapling` (+ `wend-pipe` the time-toggle).
- Persisting STATE-OBJECTS: `cistern`, `ford` (each holds a `condition` across the ages).

### The cast as frames (8 frames = ground truth + 7 person-frames)

- `gt` — ground truth.
- Founding-age people: `sera` (well-keeper), `halden` (dyer-mason), `bearer`
  (the traveller, the only frame that reaches BOTH ages).
- Withering-age people: `ode` (field-warden), `marn` (well-tender), `pell`
  (weaver), `tamsin` (far-field settler, planted future only).
- **Person-frame count: 7** (≥6 required). Each person knows only their own age
  and corner — verified by `report-frame-view` (Marn at sc-31p holds only her own
  two facts and NOT the Founding cause `f-seal-vent`; Ode at the ford holds only
  "a dyer marked it in an age none of them remember"; the bearer at sc-39p is the
  only frame holding the cross-age chain).

### Early-act → late-consequence chains (each = `expected` setup → `pays_off` payoff)

| chain | setup fact (frame / region / era) | payoff fact (frame / region / era) |
|---|---|---|
| **Cistern (same place, across ages)** | `f-seal-vent` — `gt` / Cistern Steps / Founding `sc-04` (`expected`, typed `cistern condition=sealed`) | `f-clean-water-p` & `f-clean-water-b` — `gt` / Cistern Steps / Withering `sc-31p`,`sc-31b` (typed `cistern condition=clean-holds`, supersedes `f-seal-vent`) |
| **Halden → Ode (CROSS-CORNER, cross-person, the load-bearing chain)** | `f-set-stone` — `gt` (act in `halden`'s frame: `f-cut-stone` sc-10, `f-halden-sets` sc-12) / **Loom Quarter** / Founding `sc-11` (`expected`, typed `ford condition=marked-firm`) | `f-ode-crosses-p` & `f-ode-crosses-b` — **`ode`'s frame** / **Wend Fields** / Withering `sc-36p`,`sc-36b` (typed `ford condition=crossed-firm`) |
| **Keeper's mark (same place, across ages)** | `f-keeper-mark` — `gt` / Cistern Steps / Founding `sc-05` (`expected`, untyped recognition) | `f-mark-found-p` & `f-mark-found-b` — `bearer` / Cistern Steps / Withering `sc-31p`,`sc-31b` |
| **Planting (the fork chain, planted road ONLY)** | `f-plant-sapling` — `gt` / Wend Fields / Founding `sc-26` (`expected`, typed `gorge-sapling condition=planted`, branch `planted`) | `f-bridge-tree-p` — `gt` / Wend Fields / Withering `sc-37p` (typed `gorge-sapling condition=bridged`, supersedes `f-plant-sapling`) |

The **cross-corner Halden→Ode chain** is the spine: ONE act (Halden cuts and sets
the boundary-stone in the Loom Quarter, for a craft-row dispute, never imagining
the drought) lands as ONE consequence on a different person (Ode), in a different
corner (Wend Fields), three generations later (Withering Age) — and Ode never
knows who Halden was. The act and the consequence are in DIFFERENT people's frames
(act recorded in `halden`'s frame; discovery in `ode`'s frame; the typed-state
truth-anchor `f-set-stone` is `gt`, exactly the brief's own cistern example shape).

### The persisting STATES + the exclusive rule

- `cistern` `condition`: `rotting` (`f-cistern-rot` sc-03) → `sealed`
  (`f-seal-vent` sc-04, supersedes rot) → `clean-holds` (`f-clean-water-{p,b}`,
  Withering, supersedes the seal across the ages). A single per-subject chain.
- `ford` `condition`: `unmarked` (`f-ford-unmarked` sc-09) → `marked-firm`
  (`f-set-stone` sc-11, supersedes unmarked) → `crossed-firm` (`f-ode-crosses-{p,b}`,
  Withering, in `ode`'s frame — cross-frame, so it's data not conflict).
- `gorge-sapling` `condition`: `planted` → `bridged` (planted road only).
- **Exclusive rule** (`narrative-rules.json`):
  `{ "id":"one-condition-per-object", "predicate":"condition", "class":"exclusive", "per":"subject" }`
  — enforces one-state-at-a-time on each persisting object; the late state
  supersedes the early state in-frame (`supersedes_in_frame`) so the chain reads
  cleanly across time and a two-states-at-once would be caught.

### The one planting FORK into two Withering futures

- Single fork at `sc-26` (the Wend crossing, end of the Founding Age). Branches:
  - **`planted`** — the bearer plants the gorge-sapling. Late: a living
    bridge-tree spans the gorge (`f-bridge-tree-p` pays off `f-plant-sapling`);
    Tamsin reaches and works the far field (`f-tamsin-far-p`); ending
    `f-end-p` — "the far field carries it: water, ford, and bridge hold."
  - **`barren`** — the bearer does not plant (`f-no-plant`). Late: nothing grew,
    bare rock (`f-bare-rock-b`); the gorge is impassable and the far field is LOST
    (`f-far-lost-b`); ending `f-end-b` — "withers on the near field alone."
- **Divergence**: the far field. On `planted` it is reachable (the planting thread
  is PAID); on `barren` the planting thread does not exist on that road, so the far
  field stays lost — OPEN BY DESIGN (no fact pays off planting on barren). The
  cistern + ford + keeper-mark chains pay off on BOTH roads (shared Founding causes).

### The disclosure plan `wend` (sparse) — the cause the late age cannot see

- `add-disclosure-plan --telling wend --default-mode state`.
- **Withholds** (the few real Founding-age causes, both TYPED so the gate matches):
  - `f-seal-vent` (WHY the cistern runs) — `withhold`, `first-at planted=sc-39p`,
    `barren=sc-39b` (the scene where the bearer crosses to witness it).
  - `f-set-stone` (WHO marked the ford) — `withhold`, same first-at scenes.
- **Surfaces** on each load-bearing prop in BOTH ages (the map locators):
  - `cistern-vent`: Founding `sc-06` (`f-cistern-sweet`); Withering `sc-31p`/`sc-31b`.
  - `boundary-stone`: Founding `sc-12` (`f-halden-sets`); Withering `sc-35p`/`sc-35b`.
  - `gorge-sapling`: Founding `sc-20` (`f-elder-gift`); Withering `sc-37p` (planted —
    the only road where the prop persists as the bridge-tree).
- Coverage: `disclosed=63 hidden_by_design=2 never_planned=0` — sparse, nothing
  left unplanned, exactly the 2 Founding causes hidden.

## Structural backreferences (how each late fact traces to its early cause)

Every cross-age backreference is STRUCTURAL — the Founding scene is cited in the
late fact's `evidence` array, not just in prose, and is reachable at-or-before the
late fact on its own world-line (the Founding spine `sc-01..sc-26` is earlier than
every Withering scene on each road):

- `f-clean-water-{p,b}` cite `sc-04` (Sera's seal); pay off `f-seal-vent`.
- `f-ode-crosses-{p,b}` cite `sc-11` (Halden's stone); pay off `f-set-stone`.
- `f-mark-found-{p,b}` cite `sc-05` (Sera's mark); pay off `f-keeper-mark`.
- `f-bridge-tree-p` cites `sc-26` (the planting); pays off `f-plant-sapling`.
- `f-bearer-recog-vent-{p,b}` / `f-bearer-witness-{p,b}` cite `sc-04`,`sc-11` (the
  bearer recognising "the same vent Sera packed" / witnessing both causes).

Shared early events (Sera's seal sc-04, Halden's stone sc-11, the elder's gift
sc-20, the planting scene sc-26) live on the SHARED SPINE before the fork, so both
roads can cite them without an off-branch reference.

## Write → gate → repair iterations

**2 iterations.**

- **Iteration 1** — imported 54 sections + 8 frames / 2 branches / 16 entities /
  2 predicates / 65 facts (one atomic transaction, 0 rejections). Ran all gates.
  - `validate-continuity`: clean (0 structural / 0 interval; `unchained_state_pairs=0`).
  - `report-fork-tree`: clean (2 world-lines, fork placed at sc-26, 0 unplaced).
  - `report-timeline-gaps` planted/barren: clean (0 violated).
  - `report-payoff-coverage`: clean (planted 4 paid / barren 3 paid / 0 dangling
    on either road; the un-planted far field is correctly absent on barren).
  - **`report-payoff-substantiation`: FLAGGED — `f-seal-vent <- f-clean-water`
    UNSUBSTANTIATED on both roads ("typed setup, hollow payoff").** Root cause:
    the cistern setup and its payoff carried the SAME typed value (`sealed-clean`),
    so the gate saw no state TRANSITION to discharge the setup. (Note: a gate config
    note "continuity gate: disabled ([continuity] table absent)" appears — that is
    the workspace-toml continuity table being absent for a standalone sidecar; the
    continuity gate itself still ran and reported 0 violations. Also: the gates
    require ABSOLUTE `--rules`/`--order`/`--sidecar` paths, per the brief — a
    relative `--rules` resolved against the repo root and errored; fixed by using
    absolute paths.)
  - **Repair**: re-typed the persisting chains as real transitions on the same
    subject+predicate (the dnd-substantiation pattern: setup value ≠ payoff value):
    cistern `sealed → clean-holds`; ford `marked-firm → crossed-firm`; sapling
    `planted → bridged`. Added `supersedes_in_frame` on the late same-frame `gt`
    facts (`f-clean-water-{p,b}` supersede `f-seal-vent`; `f-bridge-tree-p`
    supersedes `f-plant-sapling`) so the exclusive `condition`-per-subject rule
    stays clean (one state at a time, late supersedes early in-frame). The
    keeper-mark chain left untyped (an allowed `unverifiable`/clean shape — it is a
    recognition, not a physical state-change).

- **Iteration 2** — rebuilt the store from the empty seed, re-imported, then built
  the disclosure plan `wend` (1 plan + 2 withholds + 8 surfaces), re-ran every gate.
  - **`report-payoff-substantiation`: clean — `unsubstantiated=0` on every world**
    (planted: 3 substantiated + 1 unverifiable[keeper-mark]; barren: 2 + 1; main: 0).
  - All other gates re-confirmed clean.

## Final state of EVERY gate in the brief's checklist

1. **`validate-continuity`** — CLEAN. `facts=65 order_nodes=54 conflict_pairs=0
   cross_scope(data)=0 unordered=0 rules=1 rule_unordered=0 unchained_state_pairs=0
   violations: 0 (structural=0 interval=0)`. (Evidence-reachability, off-branch,
   the exclusive state rule, and succession all pass.)
2. **`report-fork-tree`** — CLEAN. 2 registered world-lines, 0 unplaced fork points;
   `planted` and `barren` both fork from `main` at sc-26; both futures registered
   and each reaches its terminal (`sc-40p` / `sc-40b`).
3. **`report-timeline-gaps`** (planted & barren) — CLEAN. `violated=0
   unverifiable=0` on each road (0 interval rules declared — no gap / unreached scene).
4. **`report-payoff-coverage`** — CLEAN. planted `paid=4 dangling=0`; barren
   `paid=3 dangling=0`; main `dangling=3` = the 3 shared-spine setups whose payoffs
   live past the fork on the roads (the normal main-spine shape; they ARE paid on
   each road). The barren far-field is intentionally OPEN by design (the planting
   setup does not exist on barren, so it is not even a dangling thread there).
5. **`report-payoff-substantiation`** — CLEAN. `unsubstantiated=0` on every world
   (planted substantiated=3; barren substantiated=2; the keeper-mark setup reports
   `unverifiable` = untyped recognition, the allowed clean shape).
6. **`report-disclosure-coverage --telling wend`** — CLEAN.
   `disclosed=63 hidden_by_design=2 never_planned=0` — every load-bearing prop has a
   surface in each age; both withheld Founding causes registered; nothing unplanned.
7. **`report-playthrough-manuscript`** (planted & barren) — CLEAN. Each road:
   `unplaced=0 undecidable=0 undeclared_adjacencies=0`, `outside order=14` (the
   other road's 14 scenes — the normal/expected "outside order" count).
8. **`report-playable-world --telling wend`** — CLEAN. 3 worlds run clean
   (`undeclared_adjacencies=0 unplaced=0 undecidable=0`); every prop surface
   resolves to a place on each road's walk in each age (planted 6 locators / barren
   5 / main 3). This is the map the later stage reads.

**No gate is left unclear.** All checklist gates are clean.

## Scope check

54 scenes (26 Founding + 14 planted + 14 barren); within the ≈45–55 band, two age
bands ordered early-before-late. Exactly ONE fork (planting, `planted`/`barren`) at
sc-26 into 2 distinct Withering futures with real aftermaths. 7 person-frames (≥6).
4 early→late chains including the cross-corner Halden(Loom Quarter)→Ode(Wend Fields)
chain and the same-place cistern chain. Persisting cistern/ford/sapling states typed
with one exclusive `condition`-per-subject rule. Sparse disclosure plan (8 prop
surfaces across both ages, 2 withholds on the real Founding causes). Nothing left
OPEN that was not meant to be open (only the barren far-field, by design).
