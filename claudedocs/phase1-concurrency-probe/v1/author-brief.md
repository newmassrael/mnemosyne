# Author brief — author a story as a gate-checked fact base

You are a story author. You will be given a PREMISE (`premise.md`). Your job is to
invent a complete, coherent story and record it as a **fact base** using the
`mnemosyne-cli` tool, using the tool's consistency gates to check your own work as you
go and fixing whatever they flag.

You author the STRUCTURE OF THE STORY AS FACTS — not prose. (No prose narration is
required or graded.) Think of it as writing the story's bible: who, what, where, what
is true, what each character believes, what happens in each scene, how events relate,
how it resolves.

**Read ONLY this file and `premise.md`. Do not open any other file under
`claudedocs/phase1-concurrency-probe/` — those are experiment internals and reading
them would bias your work.** Work in the directory `run/author/` (create it); leave
your deliverables there.

## The method is TOP-DOWN. This is the contract — follow it.

Do NOT free-associate scene by scene from the start. A story invented bottom-up
drifts: you discover at the climax that you needed a fact you never planted. Instead
author in two phases. The tool lets you place a fact at ANY scene coordinate in ANY
order — a late ending can be declared before an early scene exists — so the skeleton
can come first.

### Phase 0 — SCOPE + SKELETON (do this first, and get it gate-clean before Phase 1)

Decide the shape, then lay the TENTPOLES — the load-bearing facts the whole story
hangs on — BEFORE any connecting detail:

1. **Scope.** Fix the total scene count and the shape: name every world-line you use
   and where it departs (if any). Reserve the scene-id ranges.
2. **Ending / destination first.** Author how the story RESOLVES as facts — what is
   finally true or done, how it comes out. You are writing toward this. Author it
   before the connecting detail.
3. **The structure.** Author whatever structural points the story turns on (a choice
   point, a convergence, parallel lines — whatever the story actually needs), and
   enough of each part's identity that it is genuinely distinct.
4. **Load-bearing entities/objects.** Register the people, objects, and facts the plot
   TURNS on. Declaring them here, up front, IS you saying "these matter."
5. **Reveals AND their setups, together.** For every major reveal (a truth that lands
   later), author its SETUP in the SAME pass — the planted detail, placed at an EARLIER
   scene than the reveal, marked as a setup that pays off. Never let a reveal exist
   without the earlier fact it depends on.

Then run the gates over this skeleton and make it clean (see the gate list). A clean
skeleton is a spine you can trust.

### Phase 1 — DETAIL FILL

Now author the connecting detail between the tentpoles: the scenes that carry the
story through, the ordinary beats, the belief-frame facts that show characters acting
on what they (wrongly) believe. Re-run the gates as you go. Every new consequence must
trace to a cause already placed earlier in the SAME world-line.

## Structural backreferences (REQUIRED — this is part of the contract)

When a later scene REFERS BACK to an earlier event — a callback, "as happened when the
gate was wedged", "the warning no one heeded" — that backreference MUST be STRUCTURAL,
not a bare phrase inside the claim text. Cite the establishing scene in the fact's
`evidence` array.

- RIGHT: a fact whose claim is "Garrick learns the relief gate was wedged", with
  `evidence: ["<this-scene>", "<gate-scene>"]` where the gate scene established the
  wedged gate. The reference is a real edge the gates can see.
- WRONG: the same claim with only its own scene in `evidence` and the earlier event
  mentioned only in prose. That allusion is invisible to the gates and reads as
  ungrounded.
- A backreference's cited scene must be reachable AT OR BEFORE this fact in this fact's
  OWN world-line. You cannot cite as evidence a scene that happens only on a DIFFERENT
  branch, or a scene that comes later. (A gate enforces this — see below.) If two
  world-lines need to share an earlier event, that event belongs on the SHARED SPINE
  before they diverge, where each can reach it.

## What to produce (three JSON files + one store)

Work in `run/author/`. Start from an EMPTY store, then build it up.

1. **The empty seed store** — create `store.atomic.json` with exactly this content:
   ```json
   {"sections":{},"changelog_entries":{},"inventory_entries":{},"confirmation_events":{},"frames":{},"branches":{},"entities":{},"predicates":{},"narrative_facts":{},"disclosure_plans":{},"schema_version":22}
   ```

2. **`sections.json`** — your SCENES, as a JSON array. Every scene is a section. A
   canon coordinate (a scene id used by a fact) MUST exist here first.
   ```json
   [
     {"section_id":"sc-01","parent_doc":"mara","title":"The fire crests the ridge","coverage_expectation":"informational"},
     {"section_id":"sc-02","parent_doc":"mara","title":"...","coverage_expectation":"informational"}
   ]
   ```
   Load it:  `mnemosyne-cli import-sections --manifest sections.json --sidecar store.atomic.json`

3. **`facts.json`** — frames, branches, entities, predicates, facts. Load it:
   `mnemosyne-cli import-facts --manifest facts.json --sidecar store.atomic.json`
   (import-facts is ONE atomic transaction — if anything is invalid, NOTHING is
   written and it prints the error; fix and re-run. Import is additive; to CHANGE a
   row, rebuild the store from the empty seed and re-import the corrected facts.json.
   Keep facts.json the source of truth.)

   Schema (only the fields you need):
   ```json
   {
     "frames":   [{"frame_id":"gt","description":"the ground truth — what actually happened"}],
     "branches": [{"branch_id":"hold","description":"a distinct world-line","forks_from":"main","forks_at":"sc-08"}],
     "entities": [{"entity_id":"bo","kind":"person","description":"the road-warden, cut crew"}],
     "predicates":[{"predicate_id":"at_location","object_kind":"entity","description":"where a thing/person is"}],
     "facts": [
       {"fact_id":"f-001","frame":"gt","claim":"The fire crests Mara Ridge","canon_from":"sc-01","evidence":["sc-01"],"entities":["bo"],"payoff_expectation":"expected"},
       {"fact_id":"f-050","frame":"gt","claim":"At dawn the cut is complete and the backfire has met the line","canon_from":"sc-20","evidence":["sc-20","sc-01"],"entities":["bo"],"pays_off":["f-001"]}
     ]
   }
   ```

4. **`order.json`** — the canon order (REQUIRED, or the gates cannot place your facts).
   The main spine is a chain of edges; each branch (if any) is its own edge chain
   starting at its departure scene. NOTE: the canon order is a PARTIAL order — if two
   scenes are NOT connected by an edge (directly or transitively), the tool treats them
   as UNORDERED (either could come first). You only declare the orderings the story
   actually requires; you do not have to force a total order.
   ```json
   {
     "edges": [["sc-01","sc-02"],["sc-02","sc-03"]],
     "branches": {
       "hold": [["sc-08","sc-09a"],["sc-09a","sc-10a"]]
     }
   }
   ```
   Pass `order.json` to EVERY gate as `--order order.json`.

## Field rules (read these — they save you loop iterations)

- A fact's `canon_from`, a section's `section_id`, a branch's `forks_at`, and every id
  in `evidence[]` must all be SCENE IDS that exist in `sections.json`.
- `branch`: the world-line a fact belongs to. OMIT it for facts on the shared spine
  before any fork (those live on the implicit root branch `main`). After a fork, tag
  each fact with its branch id.
- `branches[].forks_from`: the parent branch id (`"main"` for the root spine).
  `forks_at` = the scene on the parent where this branch departs.
- `payoff_expectation`: ONLY the literal `"expected"` (mark a setup that must pay off)
  or `"unmarked"`/omit. It is NOT free text.
- `pays_off`: an ARRAY of the setup fact-ids this fact discharges, e.g. `["f-001"]`.
  The paying fact must be reachable after the setup in its world-line.
- `frame`: at least a ground-truth frame (e.g. `"gt"`). To make a character BELIEVE
  something false, add a belief frame (e.g. `"bo-belief"`) and put the belief fact in
  that frame. Never mark a belief fact and a truth fact as conflicting — they are two
  true facts on two frames.
- `evidence`: at minimum the fact's own scene; ADD the establishing scene(s) for any
  backreference (the structural-backreference rule above).
- `typed` (OPTIONAL): `{"subject":"<entity-id>","predicate":"<predicate-id>","object":{"kind":"value","value":"complete"}}` or `{"kind":"entity","id":"<entity-id>"}`. Typed setup+payoff pairs read as *substantiated* (stronger) than untyped; use it for the load-bearing reveals and for any state you want the tool to track (e.g. where a thing IS).
- `narrative-rules` (OPTIONAL): if some state in your story is mutually exclusive (a
  thing can be in only one place at a time; a person can hold only one role at a time),
  you may declare a rule file and check it — see the tool's `validate-continuity`
  output and `--rules`. This is optional; use it only if the story needs it.

## The gates (run all, with --order, after each import)

```
mnemosyne-cli validate-continuity            --order order.json --sidecar store.atomic.json
mnemosyne-cli report-fork-tree               --order order.json --sidecar store.atomic.json
mnemosyne-cli report-timeline-gaps --world <each-world> --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-coverage         --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-substantiation   --order order.json --sidecar store.atomic.json
mnemosyne-cli report-playthrough-manuscript --world <each-world> --order order.json --sidecar store.atomic.json
```

Read them and FIX until:
- `validate-continuity`: 0 structural + 0 interval violations. (This INCLUDES an
  evidence-reachability check: an `evidence` citation not reachable at-or-before the
  fact in its own world-line is a structural violation. If you see
  `evidence_unreachable` or `fact_canon_off_branch`, a coordinate or backreference
  points off this world-line or forward — fix it by the rule above.)
- `report-fork-tree`: any fork/convergence points are PLACED (not "UNPLACED"); every
  registered world-line reaches a resolution.
- `report-playthrough-manuscript --world W`: for EVERY world-line, 0 unplaced / 0
  undecidable — every fact sits in a real scene of that world, and reading the scenes
  in order tells that world's story start to finish. (An "outside order" count is
  normal: scenes correctly excluded from this world's walk.)
- `report-payoff-coverage`: no setup left dangling in any TERMINAL world.
- `report-timeline-gaps`: no gap/unreached scene in any world.

## Deliverables (leave these in `run/author/`)

- `sections.json`, `facts.json`, `order.json`, `store.atomic.json` (final, gate-clean).
- `author-log.md` — a log: how you laid the Phase-0 skeleton (the tentpoles you placed
  first), then how many write→gate→repair iterations you ran in Phase 1, what the gates
  flagged each pass, and what you changed. **Record honestly any point where the story
  you wanted to tell was awkward to express in the tool — anywhere you had to
  restructure, simplify, duplicate, or work around something to get it expressed, note
  what it was and what you did.** (This records the authoring loop itself.)

## Scope reminder

~16–20 scenes. Tell the best, most coherent version of the story the premise asks for,
using whatever the tools offer to express it — declare the orderings, world-lines,
entities, state, and setups the story actually needs, and leave unordered whatever the
story does not pin. At least one long-range setup→payoff. At least one character whose
belief diverges from the ground truth. Lay the skeleton top-down first, then fill. Make
every consequence trace to a cause you have already placed, and leave nothing
load-bearing dangling. If the story is hard to express in the tool at any point, do
your best faithful version AND note the difficulty in `author-log.md`.
