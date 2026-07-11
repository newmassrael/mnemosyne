# Loop log — authoring "The Ban Tower" into a Mnemosyne store

A numbered log of every `propose-verdict` and `report-authoring-frontier` call,
the verdict/gaps, and what I changed in response and why. Plus honest notes on
friction, guesses, and information the contract did not give me.

Author: driven only by `describe-schema`, `premise.md`, and the gate outputs.

---

## Phase 0 — reading the contract, laying down scenes

- Read `describe-schema` and `describe-schema --json` in full, and both brief
  files. The schema documents registries, the fact shape, fixed vocabularies,
  rule classes, the quest convention, and the write-time invariants.
- Wrote `sections.json` (16 scenes: 8 shared/main `sc-01..sc-08`, a 4-scene
  "hold" aftermath `sc-09h..sc-12h`, a 4-scene "descend" aftermath
  `sc-09d..sc-12d`) and imported it: `16 created, 0 no-op`.

### FRICTION 1 (the single biggest) — the manifest wire format is not in the contract

`describe-schema` documents *field names and types* (e.g. "frame : frame id",
"typed : TypedClaim (subject, predicate, object)") but NOT the exact JSON
serialization the parser expects: the registry-object key names, how the typed
`object` enum is tagged, the tuple shape of disclosure timing pins, or the
fork-point field. None of that is in `describe-schema [--json]`. I recovered it
by sending deliberately-wrong probe manifests through `propose-verdict` (the
dry-run gate — free, never touches the store) and reading each parser error.
`propose-verdict`'s parse errors are precise (they name the line/column and the
expected type), so this worked, but it is reverse-engineering the wire format,
not authoring from a documented contract. What I had to discover this way:

| thing | not documented; discovered value |
|---|---|
| frame object | `{ "frame_id": ... }` |
| entity object | `{ "entity_id", "kind", "name" }` |
| predicate object | `{ "predicate_id", "object_kind": "scalar"|"entity" }` |
| branch fork | `{ "branch_id", "forks_from": "<parent-branch>", "forks_at": "<section>" }` — `forks_from` is a STRING (I first tried a `{parent, at}` map) |
| typed scalar object | `{ "kind": "value", "value": "<str>" }` — internally-tagged enum, variants are `entity`/`value` (NOT `scalar`) |
| typed entity object | `{ "kind": "entity", "id": "<entity-id>" }` |
| disclosure override | `{ "fact_id", "mode", "first_at": [ ["<branch>","<section>"], ... ] }` — `fact_id` not `fact`; `first_at` is a list of 2-tuples `[branch, section]` |

### propose-verdict probe calls (shape discovery)

Each is `propose-verdict --manifest <probe> --json`. Verbatim key error lines:

1. frames only `[{frame_id}]` → `commit`. (frame key = `frame_id`.)
2. + entities/predicates → `commit`. (entity `entity_id/kind/name`; predicate
   `predicate_id/object_kind` accepted.)
3. + branch `forks_from:{parent,at}` (a map) →
   `invalid type: map, expected a string ... (branches)`.
   Change: `forks_from` must be a string.
4. branch `forks_from:"main", forks_at:"sc-08..."` →
   `invalid type: string "dead", expected internally tagged enum TypedObject`.
   Change: the typed `object` is an enum, not a bare string. (Branch shape now OK.)
5. typed object `{kind:"scalar", value:"dead"}` →
   `unknown variant "scalar", expected "entity" or "value"`.
   Change: the scalar variant is spelled `value`, not `scalar`.
6. typed object `{kind:"value", value:"dead"}` →
   `invalid type: string ..., expected a sequence` at `first_at`.
   Change: `first_at` is an array, not a scalar section.
7. `first_at:[{branch,section}]` →
   `invalid type: map, expected an array of length 2`.
   Change: each timing pin is a 2-tuple `[branch, section]`.
8. `first_at:[["road-descend","sc-11d..."]]` →
   `missing field "fact_id"` in the override.
   Change: the override key is `fact_id`, not `fact`.
9. override `{fact_id, mode, first_at}` → `commit`. Full plan shape confirmed.
10. (probe2) typed entity object `{kind:"entity", entity:"e-westdraw"}` →
    `missing field "id"`. Change: entity object uses `id`.
11. (probe2) `{kind:"entity", id:"e-westdraw"}` → `commit`. Entity object confirmed.

No content was being repaired in 1–11; this was purely learning the serializer.

---

## Phase 1 — first full manifest and the frontier's structural lesson

Designed the game on paper (cast/frames, entities, predicates, the fork, the
withheld secret as a disclosure plan, the quest, the setup→payoff chains) and
wrote `facts.json` — first version with TWO registered forks (`road-hold` and
`road-descend`), both forking from `main` at `sc-08`, with the pre-fork setups
`f-03-torn-page`, `f-04-smoke-bearing`, `f-give-westdraw` marked `expected` on
`main`.

### propose-verdict call 12 — first full manifest (v1, two forks)

`propose-verdict --manifest facts.json --json` →
```
verdict: commit — 6 frames + 2 branches + 14 entities + 4 predicates
+ 27 facts + 1 disclosure-plans + 3 disclosure-overrides, 0 violations
```
Clean on the first content try (the probe rounds had already taught me the
shape). Applied it: `import-facts` → 27 facts created.

### report-authoring-frontier call 1 — after v1 import

`report-authoring-frontier --telling default-telling --json` →
```
zero_fact_scenes:   []           (all 16 scenes carry >= 1 fact)
unresolved_quests:  []           (q-westdraw has a completed_by binding)
dangling_setups:    { "main": ["f-03-torn-page","f-04-smoke-bearing","f-give-westdraw"] }
never_planned_disclosures: 24    (my choice — see note below)
```

**The lesson (FRICTION 2 — the dangling model is under-documented).**
`describe-schema` defines a dangling setup as "a fact you marked `expected`
whose payoff is not yet visible on that world," but does not say how
"visible on a world" interacts with fork lineage. The frontier bucketed the
three under `main` — and NOT under `road-hold`/`road-descend`, even though
`f-give-westdraw` is unpaid on `road-hold`. From that I inferred the actual
model: a setup is checked on the world-line it is **authored** on, and a payoff
on a **child** branch does not discharge a **parent** (`main`) setup. Combined
with a hard fact I hit while designing — a branch fact whose `canon_from` is a
**pre-fork** section (`sc-03`, `sc-04`, before the fork at `sc-08`) would be
off-branch — this means: **pre-fork setups must live on `main`, so their
payoffs must also be on `main`, so `main` must continue past the fork.** My v1
put both aftermaths on child branches, leaving `main` with three setups it
could never pay off.

**The repair (a structural topology change, not a new fact).** I made the
"hold the post" ending the continuation of `main` itself (the protocol default
stays on the trunk) and registered ONLY `road-descend` as the fork off it — the
moral deviation forks off the default. Concretely, in `facts.json`:
- dropped the `road-hold` branch registration (kept `road-descend`);
- moved the five hold-aftermath facts (`f-09h..f-12h`) from `branch:"road-hold"`
  to `branch:"main"`, so the three `main` setups now pay off on `main`
  (`f-12h` pays `f-03`, `f-10h` pays `f-04`);
- added `f-11h-discharge` (a `completed_by` fact on `main`) so the quest's giving
  setup `f-give-westdraw` also pays off on the `main`/hold line — the quest is
  now discharged on BOTH world-lines (differently: escalate-and-hand-off on the
  hold line, reach-them-in-person on the descend line). The asymmetry the fork
  carries is instead the withheld secret (revealed only on `road-descend`).

### FRICTION 3 — could not close this gap the additive way the brief suggests

The brief's step 6 says close gaps with a NEW manifest (`facts-2.json`) applied
on top. This gap could not be closed additively: the fix required *changing the
branch* of already-imported facts, and the store is append-only (a fact_id can't
be re-branched by a later manifest). Closing it additively would have meant
adding a redundant third world-line of duplicate hold facts on `main` alongside
the existing `road-hold` facts — which would violate the premise's "exactly two
world-lines." So I reset the store instead: the store file
(`docs/.atomic/workspace.atomic.json`) is a gitignored, regenerable artifact;
I backed it up, deleted it, and confirmed `import-sections` recreates a fresh
empty store (identical 2425-byte sections-only store), then re-imported the
corrected single `facts.json`. I did NOT hand-edit the store JSON at any point.
This is a deviation from the suggested additive loop, made because the gap was a
structural topology error rather than missing content — noting it honestly.

---

## Phase 2 — corrected manifest, clean frontier

### propose-verdict call 13 — corrected manifest (v2, main-continuation + one fork)

`propose-verdict --manifest facts.json --json` (against the fresh store) →
```
verdict: commit — 6 frames + 1 branches + 14 entities + 4 predicates
+ 28 facts + 1 disclosure-plans + 3 disclosure-overrides, 0 violations
```
Applied: `import-facts` → 28 facts created (15609 bytes).

### report-authoring-frontier call 2 — after v2 import (FINAL)

`report-authoring-frontier --telling default-telling` →
```
=== authoring frontier — telling default-telling — 25 gap(s) ===
zero-fact scenes: none
dangling setups:  none
unresolved quests: none
never-planned disclosures (25): f-01-arrival, f-02-empty-cab, f-03-torn-page,
  f-04-smoke-bearing, f-05-dispatch, f-05b-crank, f-06-gear, f-06b-tomas-belief,
  f-07-full-dark, f-07b-ranger-expects, f-08-overlook, f-09d-descent, f-09d-rope,
  f-09h-holding, f-10d-cold-ring, f-10d2-sella-found, f-10h-no-fire,
  f-10h2-ranger-search, f-11d-body, f-11h-discharge, f-11h-first-light,
  f-12d-carry-out, f-12h-torn-inference, f-give-westdraw, f-pursue-westdraw
```

**Stop condition met (brief step 7):** 0 zero-fact scenes, 0 dangling setups,
0 unresolved quests.

The 25 never-planned disclosures are intentional and left as-is, per the brief
("a withheld secret is intentionally never 'planned' to be told; leave those as
you intend") and premise item 3, which asks for "a disclosure plan whose default
withholds." I chose the hidden-lore default the schema itself describes
(`default_mode: withhold` — "the Dark-Souls hidden-lore extreme"): the reader of
the default telling reconstructs the surface, and only the three load-bearing
secret facts carry explicit reveal plans. Verified the three secrets are NOT in
the never-planned list (they are planned via overrides):
`gt-death`, `gt-smoke`, `gt-sella` → planned (state, first told on
`road-descend` at `sc-11d`/`sc-10d`).

---

## Design summary (what got built)

- **16 scenes**, **6 frames** (`ground-truth` + relief/senior/dispatch/ranger/
  hiker = 5 characters, each an epistemic frame), **14 entities**,
  **4 predicates** (`life_state`, `signal_kind`, `pursues`, `completed_by`).
- **The fork** (`sc-08-west-draw`): hold the post (stays on `main`) vs descend
  into the west draw (`road-descend` fork). Two canonical world-lines.
- **The withheld secret** (disclosure plan `default-telling`, default withhold):
  `gt-death` (Tomas fell to his death on the scree), `gt-smoke` (the west-draw
  smoke was a real distress signal fire, not a wildfire), `gt-sella` (a stranded
  hiker lit it and is alive). Revealed ONLY on `road-descend`, late (`sc-10d`,
  `sc-11d`). On the hold line it stays reconstructed-only.
- **The quest** `q-westdraw` (kind `quest`): given at `sc-04` (`f-give-westdraw`,
  `expected`), pursued by Wren (`f-pursue-westdraw`), discharged on BOTH roads
  (`f-11h-discharge` on the hold/main line, `f-12d-carry-out` on the descend
  line), each `pays_off` the giving setup → resolved, per-world done.
- **Setup→payoff chains:** the torn log page (`f-03`, physical) pays off in BOTH
  world-lines (`f-12h` on hold, `f-11d` on descend); the unconfirmed smoke
  bearing (`f-04`, radio) pays off in BOTH (`f-10h` on hold, `f-10d` on descend);
  the anchored climbing rope / handline (`f-09d-rope`, physical) pays off in ONE
  world only (`f-11d`, on `road-descend`).

## Gate-call tally

- `propose-verdict`: 13 calls total — 11 were shape-discovery probes, 1 gated the
  first full manifest (`commit`), 1 gated the corrected manifest (`commit`). Zero
  content-repair rounds were needed (both full manifests passed first try; the
  only rework was the topology change driven by the frontier, not by a
  `propose-verdict` rollback).
- `report-authoring-frontier`: 2 calls — round 1 surfaced the 3 `main`
  dangling setups; round 2 (after the topology repair) was clean on all three
  stop-condition axes.
