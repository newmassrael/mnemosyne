# map-corpus-experiment/v1 — report

Executed 2026-07-31 on the owner's experiment word, per the runbook committed in
`b9ca7c8` (Round 898). The runbook, including both author briefs verbatim, was
tracked BEFORE execution; this report is the post-run record.

**What is claimed.** The map primitives ran on data a person authored, and the
corpus discriminates where the two existing stores agreed. **The oracle is our
own verb output, so nothing here is evidence that the map primitives are
CORRECT** (Round 884). Exercise and discrimination are the whole claim.

**Shape.** Two blind authors, fresh context each, firewalled to their own
directory. No render, no re-extraction, no judges — the map axis is a structure
axis (Round 892). Stage A's brief contains no map vocabulary at all; Stage B's
adds one fiction sentence asking that the engine be able to answer where one can
move to. Neither brief speaks schema.

---

## E1 — does a blind author reach for the `transition` class unprompted?

**YES, and not as an artifact of iteration.**

Stage A's brief never contains the words map, transition, adjacency, edge, graph
or route. The author's `narrative-rules.json` declares:

    "id": "movement-follows-the-map", "predicate": "at", "class": "transition",
    "adjacency": "adjacent", "undirected": false, "containment": "contains"

`order.json` and `narrative-rules.json` are **byte-identical between the frozen
first submission and the green store** (`diff -q`), so the map declaration was
right on the first submission and the iterate-to-green loop did not produce it.

The translation from fiction to schema was the author's own. Both authors chose
`undirected: false` and both explained it in the same terms without being asked:
a descent that cannot be climbed back is inexpressible if an edge admits both
directions, so symmetry is written twice where it holds.

E1's premise was checked before the stage was designed (Round 898) and holds:
`describe-schema` names `adjacency`, `undirected` and `containment`, and says in
as many words that this is how movement between places is gated.

## E2 — does the FIRST submission import?

**NO — and the same defect, independently, in both authors.**

Frozen before anything was handed back, full text, both exit codes.

Stage A (`run/stage-a/first-import.log`):

    error: parse manifest sections.json (JSON array of section imports): invalid type: map, expected a sequence at line 1 column 0
    exit=1
    === mnemosyne-cli atomic mutate FAILED ===
    error: validation: branch `sluice-open`: fork point `sc-12` not present as a section (canon coordinates are structure refs)
    exit=1

Stage B (`run/stage-b/first-import.log`):

    error: parse manifest sections.json (JSON array of section imports): invalid type: map, expected a sequence at line 1 column 0
    exit=1
    === mnemosyne-cli atomic mutate FAILED ===
    error: validation: branch `sluice-drawn`: fork point `sc-15` not present as a section (canon coordinates are structure refs)
    exit=1

Two authors who never saw each other's work produced the SAME wrong guess,
`{"sections": [...]}`, for the same reason: **the contract gives no wire shape
for the sections file at all.** Each independently reported it as a guess before
running anything — Stage A's sealed self-report says "If the loader wants
something else, that is a guess and not a reading."

The store was untouched by the failure (58 bytes, unchanged), so the rejection is
clean rather than partial.

The second error in each log is a **cascade, not a second defect**, and this was
verified rather than accepted from the authors: `facts.json` differs from its
frozen first submission by 13 lines in Stage A, **none of which touch adjacency**,
and `order.json` / `narrative-rules.json` are byte-identical. Stage B's fact
manifest imported with no content change at all. The second message names the
first casualty of the first failure, not an independent problem. It does reveal
one write-time invariant the contract does not list: a branch's `forks_at` is
checked against the sections registry.

## Contract gaps — the evidence the brief did not ask for

Each was reproduced by the orchestrator on an isolated scratch store, not taken
from an author's report.

**G-1. `sections.json` has no wire format in the contract, and `parent_doc` is
required.** `grep -c parent_doc contract.txt` = **0**, yet every section record
needs it. Both authors supplied a guessed value that was accepted — Stage A
`"ossary-rise"`, Stage B `"workspace"`. Stage B's note is the sharp one:
"accepted is not correct. If it is a registry ref a wrong value would have failed
loud; if it is free text, my 21 scenes are filed under a guessed document name,
and the two commands I have cannot tell me which."

**G-2. The contract under-declares the fact manifest by exactly one array:
`units`.** Counted from the type, not from a string: `FactsManifest` has **8**
fields (`frames, branches, entity_kinds, units, entities, predicates, facts,
disclosure_plans`); the contract's manifest section says "seven optional arrays"
and lists all but `units`. It also says elsewhere that a unit is "Declared via
add-unit". Both authors therefore concluded that a Quantity is not authorable
from files and both authored a workaround — Stage A a closed token vocabulary
(`short` / `long` / `half-a-day`), Stage B durations in prose claims. Stage A's
SEALED self-report carries that false conclusion in writing. Both authors later
falsified it by measuring: adding `"units": [...]` imports as `1 units created`.
Verified independently here on a scratch store.

**G-3. The manifest ignores unknown keys, and the contract never says so.** A
manifest with a key that does not exist returns exit 0, zero created, zero bytes
written, and no mention of the key; so does a typo of a real key (`"frame"` for
`"frames"`). **This leniency is deliberate and load-bearing, not an oversight** —
`evidence_replay_smoke.rs` states it and depends on it, since its shape detector
must distinguish a manifest that parses from one that BUILDS. So the defect is
not the parser: it is that the contract handed to authors never mentions it. G-3
is what made G-2 undiscoverable, because a correct guess and a typo produce
byte-identical output.

## E3 — sealed self-report vs the machine

Recorded, not resolved in the orchestrator's voice.

**Stage A — a numeric disagreement.** Sealed report: thirteen locations, 29
`adjacent` facts. Machine: `12 node(s), 29 edge(s)`. Edges agree exactly; the
missing thirteenth node is the Undertown, the place the brief asked to be
spoken of and never reached, which the author authored with no adjacency fact at
either end. **The substrate does not lose it** — `validate-continuity` names it
as `map_contained_off_map` (a contained thing that is neither a node nor a
container). The map READ cannot show an isolated place, because a map's node set
is derived from its edges; the GATE is where it surfaces.

**Stage B — the counts agree, the question does not.** Sealed report: twelve
walkable locations, 23 `adjacent` facts, one place unreachable from the start
(the drowned mill). Machine: `12 node(s), 23 edge(s)` — both exact. The mill has
one outgoing edge and no way in, so it IS a node. The frontier answers
`unconnected places: none`, which is also true. **Two true answers to two
different questions**: the author reported UNREACHABLE, the verb reports
UNCONNECTED, and there is no read in this substrate that answers the first.
Round 891 excluded reachability deliberately; this corpus is the first authored
instance where that exclusion is visible as a gap between what an author is asked
to know and what the store can be asked.

**Why the two stages differ on the frontier.** Stage B declared
`subject_kind: "place"` and `object_entity_kind: "place"` on `adjacent`; Stage A
declared neither. That one difference decides whether Round 891's subtraction can
run at all:

    Stage A: unconnected places [movement-follows-the-map]: not derivable
             — predicate `adjacent` declares no leg kind, so the registered
             place set cannot be asked for
    Stage B: unconnected places [moves-follow-the-map]: none

Round 898 recorded that Round 891's own J2 injection could not fail through the
CLI for want of a place subkind in the fixture. A blind author reproduced that
exact shape independently, from the contract alone.

## C1 — which branches ran on authored data

Both stores declare a transition rule, so the map verbs answer from their
non-empty branches for the first time in this tree.

- `report-transition-map`: 12 nodes / 29 edges (A), 12 nodes / 23 edges (B).
- `report-authoring-frontier` map arm: `not derivable` (A), computed (B).
- G2 containment checks (Round 703): fired on authored data —
  `map_contained_off_map` (A), `adjacency_cross_scope` (both).
- `validate-continuity` **rejects both corpora**: exit 1, 27 violations in A
  (14 `adjacency_cross_scope`, 11 `evidence_unreachable`, 2
  `map_contained_off_map`), 12 in B (10 `adjacency_cross_scope`, 2
  `evidence_unreachable`).

**The dominant violation replicates across both authors and is a finding, not an
author error.** Both authored quarters as containers holding locations, as the
brief asked, and both wrote passages between locations in different quarters —
which is what a town is. The check treats a non-sibling edge as a violation; the
source states the intent plainly (`AdjacencyCrossScope (a non-sibling edge)`).
The contract mentions `cross_scope` **zero** times, and describes G2 only as
"every place-kind entity must be a node or a container; a container must not be
walked on; a region contains only real nodes" — none of which says an edge must
stay inside one container. So declaring `containment` silently turns on a check
that rejects a natural hierarchical map, in a direction the author cannot see.
Whether the check is right (a hierarchical map should also carry quarter-level
edges) or over-strict (a town's passages cross quarters) is a design question
this report does not settle.

Also caught by the frontier and worth recording: Stage A's author left a stray
`sc-probe` section while probing `parent_doc`; the frontier names it under
`zero-fact scenes` and `unplaced scenes`. The pollution is visible, and it is why
the Stage B instructions forbade throwaway records. It never entered the tracked
corpus — `sc-probe` appears zero times in `stage-a/sections.json`, so the
registered replay rebuilds a store without it; the pollution lived only in the
gitignored scratch store the frontier was read from.

## C2 — the sweep's third arm

Round 896 left seven verbs byte-identical across its two real stores after its
own repairs. Re-run with the Stage B corpus as a third arm against this
repository's store:

| verb | third arm |
|---|---|
| `report-binding-migration` | identical |
| `report-confirmation` | identical |
| `report-parameter-economy` | identical |
| `validate-confirmation` | identical |
| `validate-spec-drift` | identical |
| `validate-verifies-linkage` | identical |
| **`report-transition-map`** | **DIFFERS** |

Exactly one, and it is the one Round 896 called the sharpest case of an honest
byte-identical — both existing stores print "no transition rule declares an
adjacency predicate" because both are correct to. The corpus gains discriminating
power on precisely the axis it was built for and on no other, which is the honest
result: the other six read inputs an authored map corpus still does not carry.

## What this corpus does not do

- It does not unblock `validate-render-fidelity` / `validate-disclosure-leak`
  (Round 897): those need a re-extracted store, and this corpus has no prose.
- It does not compute reachability (Round 891 excluded it), and E3 shows why that
  now has an authored witness.
- It does not settle per-world maps (Round 696 finding 6).
- `edge-costs.json` and `edge-guards.json` in Stage B are argument lists for
  `add-edge-cost` / `add-edge-guard`, not manifest-loadable, so the cost and guard
  side tables remain unexercised — the corpus reached the boundary of what file
  authoring can express and stopped there, which is itself the measurement.
- Neither corpus passes `validate-continuity`. The corpus is registered as a
  replay of its IMPORT, which is what it demonstrates; a gate-clean map corpus
  would require settling the `adjacency_cross_scope` design question first.
