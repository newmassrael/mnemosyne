# Author brief — author a branching story as a gate-checked fact base

You are a story author. You will be given a PREMISE (`premise.md`). Your job is to
invent a complete, coherent, branching story and record it as a **fact base** using
the `mnemosyne-cli` tool, using the tool's consistency gates to check your own work
as you go and fixing whatever they flag.

You author the STRUCTURE OF THE STORY AS FACTS — not prose. (No prose narration is
required or graded.) Think of it as writing the story's bible: who, what, where,
what is true, what each character believes, what happens in each scene, how the
branches diverge, how each branch ends.

## Your working loop (this IS the job)

WRITE facts → RUN the gates → READ what they flag → FIX it → repeat, until the
gates are clean and the story is whole. The gates are your consistency feedback;
use them like a compiler. A clean store that tells a real, coherent branching story
is the goal.

## What to produce (three JSON files + one store)

Work in this directory. Start from an EMPTY store, then build it up.

1. **The empty seed store** — create `store.atomic.json` with exactly this content:
   ```json
   {"sections":{},"changelog_entries":{},"inventory_entries":{},"confirmation_events":{},"frames":{},"branches":{},"entities":{},"predicates":{},"narrative_facts":{},"disclosure_plans":{},"schema_version":22}
   ```

2. **`sections.json`** — your SCENES, as a JSON array. Every scene is a section.
   A canon coordinate (a scene id used by a fact) MUST exist here first.
   ```json
   [
     {"section_id":"sc-01","parent_doc":"saltmarsh","title":"The early bell","coverage_expectation":"informational"},
     {"section_id":"sc-02","parent_doc":"saltmarsh","title":"...","coverage_expectation":"informational"}
   ]
   ```
   Load it:  `mnemosyne-cli import-sections --manifest sections.json --sidecar store.atomic.json`

3. **`facts.json`** — frames, branches, entities, predicates, facts. Load it:
   `mnemosyne-cli import-facts --manifest facts.json --sidecar store.atomic.json`
   (import-facts is ONE atomic transaction — if anything is invalid, NOTHING is
   written and it prints the error; fix and re-run. You may re-run import-facts
   repeatedly on the same store; existing identical rows are no-ops.)

   Schema (only the fields you need):
   ```json
   {
     "frames":   [{"frame_id":"gt","description":"the ground truth — what actually happened"}],
     "branches": [{"branch_id":"expose","description":"Aldous carries word to the mainland","forks_from":"main","forks_at":"sc-05"}],
     "entities": [{"entity_id":"aldous","kind":"person","description":"the bellwright"}],
     "predicates":[{"predicate_id":"fate","object_kind":"scalar","description":"a boat's outcome"}],
     "facts": [
       {"fact_id":"f-01","frame":"gt","claim":"Aldous hears the bell ring an hour early","canon_from":"sc-01","evidence":["sc-01"],"entities":["aldous"],"payoff_expectation":"expected"},
       {"fact_id":"f-12","frame":"gt","branch":"expose","claim":"the board learns the early bell hid a rotten jetty","canon_from":"sc-06","evidence":["sc-06"],"entities":["aldous"],"pays_off":["f-01"]}
     ]
   }
   ```

4. **`order.json`** — the canon order (REQUIRED, or the gates cannot place your
   facts). The main spine is a chain of edges; each branch is its own edge chain
   starting at its fork scene:
   ```json
   {
     "edges": [["sc-01","sc-02"],["sc-02","sc-03"],["sc-03","sc-04"],["sc-04","sc-05"]],
     "branches": {
       "expose": [["sc-05","sc-06"],["sc-06","sc-07"]],
       "confront": [["sc-05","sc-08"],["sc-08","sc-09"]]
     }
   }
   ```
   Pass it to EVERY gate as `--order order.json`.

## Field rules (read these — they save you loop iterations)

- A fact's `canon_from`, a section's `section_id`, a branch's `forks_at`, and every
  id in `evidence[]` must all be SCENE IDS that exist in `sections.json`.
- `branch`: the world-line a fact belongs to. OMIT it for facts on the shared
  spine before the fork (those live on the implicit root branch `main`). After a
  fork, tag each fact with its branch id.
- `branches[].forks_from`: the parent branch id (`"main"` for the root spine).
  `forks_from` may itself be another branch (a fork off a fork).
- `payoff_expectation`: ONLY the literal `"expected"` (mark a setup that must pay
  off) or `"unmarked"`/omit. It is NOT free text.
- `pays_off`: an ARRAY of the setup fact-ids this fact discharges, e.g. `["f-01"]`.
- `frame`: at least a ground-truth frame (e.g. `"gt"`). To make a character BELIEVE
  something different from the truth (premise requires this), add a second frame
  (e.g. `"aldous-belief"`) and put the belief fact in that frame. Never mark a
  belief fact and a truth fact as conflicting — they are two true facts on two
  frames.
- `typed` (OPTIONAL): `{"subject":"<entity-id>","predicate":"<predicate-id>","object":{"kind":"value","value":"rotten"}}` or `{"kind":"entity","id":"<entity-id>"}`. Typed setup+payoff pairs read as *substantiated* (stronger) instead of *unverifiable*; untyped is fine but weaker.

## The gates (run all, with --order, after each import)

```
mnemosyne-cli validate-continuity            --order order.json --sidecar store.atomic.json
mnemosyne-cli report-fork-tree               --order order.json --sidecar store.atomic.json
mnemosyne-cli report-timeline-gaps --world <each-branch> --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-coverage         --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-substantiation   --order order.json --sidecar store.atomic.json
mnemosyne-cli report-playthrough-manuscript --world <each-branch> --order order.json --sidecar store.atomic.json
```

Read them and FIX until:
- `validate-continuity`: 0 structural + 0 interval violations.
- `report-fork-tree`: your fork point(s) are PLACED (not "UNPLACED"); every
  branch is registered; every world-line reaches a terminal.
- `report-playthrough-manuscript --world W`: for EVERY branch, 0 unplaced / 0
  outside-order / 0 undecidable — every fact sits in a real scene of that world,
  and reading the scenes in order tells that world's story start to finish.
- `report-payoff-coverage`: no setup left dangling (every `payoff_expectation:expected` has a paying fact).
- `report-timeline-gaps`: no gap/unreached scene in any world.

## Deliverables (leave these in this directory)

- `sections.json`, `facts.json`, `order.json`, `store.atomic.json` (final, gate-clean).
- `author-log.md` — a short log: how many write→gate→repair iterations you ran,
  what the gates flagged each pass, and what you changed. (This records the
  authoring loop itself.)

## Scope reminder

8–14 scenes; exactly one primary fork into 2–3 world-lines, each with its own
terminal ending; at least one real setup→payoff; at least one character belief that
diverges from the ground truth, with that character acting on the belief. Tell the
best, most coherent branching story you can — make every consequence trace to a
cause you have already placed, and leave nothing important dangling.
