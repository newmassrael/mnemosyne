# Author brief — author a branching story as a gate-checked fact base

You are a story author. You will be given a PREMISE (`premise.md`). Your job is to
invent a complete, coherent, branching story and record it as a **fact base** using
the `mnemosyne-cli` tool, using the tool's consistency gates to check your own work
as you go and fixing whatever they flag.

You author the STRUCTURE OF THE STORY AS FACTS — not prose. (No prose narration is
required or graded.) Think of it as writing the story's bible: who, what, where,
what is true, what each character believes, what happens in each scene, how the
branches diverge, how each branch resolves.

**Read ONLY this file and `premise.md`. Do not open any other file under
`claudedocs/phase1-convergence-probe/` — those are experiment internals and reading
them would bias your work.** Work in the directory `run/author/` (create it); leave
your deliverables there.

## The method is TOP-DOWN. This is the contract — follow it.

Do NOT free-associate scene by scene from the start. A branching story invented
bottom-up drifts: you discover at the climax that you needed a fact you never
planted. Instead author in two phases. The tool lets you place a fact at ANY scene
coordinate in ANY order — a late ending can be declared before an early scene exists
— so the skeleton can come first.

### Phase 0 — SCOPE + SKELETON (do this first, and get it gate-clean before Phase 1)

Decide the shape, then lay the TENTPOLES — the load-bearing facts the whole story
hangs on — BEFORE any connecting detail:

1. **Scope.** Fix the total scene count and the world-lines: name every branch and
   where it forks. Reserve the scene-id ranges (the shared spine `sc-01..sc-NN`, then
   each branch's own range).
2. **Endings / destination first.** Author how the story RESOLVES as facts — what is
   finally true or done, who ends where, how the dawn lands. You are writing toward
   this. Author it before the connecting detail.
3. **The fork.** Author the fork point: the choice, and enough of each resulting
   world-line's identity that the branches are genuinely distinct.
4. **Load-bearing entities/objects.** Register the people, objects, and facts the
   plot TURNS on (the wedged gate, whatever else you invent). Declaring them here, up
   front, IS you saying "these matter."
5. **Reveals AND their setups, together.** For every major reveal (a truth that lands
   later), author its SETUP in the SAME pass — the planted detail, placed at an
   EARLIER scene than the reveal, marked as a setup that pays off. Never let a reveal
   exist without the earlier fact it depends on.

Then run the gates over this skeleton and make it clean (see the gate list). A clean
skeleton is a spine you can trust.

### Phase 1 — DETAIL FILL

Now author the connecting detail between the tentpoles: the scenes that carry each
world-line through, the ordinary beats, the belief-frame facts that show characters
acting on what they (wrongly) believe. Re-run the gates as you go. Every new
consequence must trace to a cause already placed earlier in the SAME world-line.

## Structural backreferences (REQUIRED — this is part of the contract)

When a later scene REFERS BACK to an earlier event — a callback, "the warning no one
freed the gate", "as happened the night the water rose" — that backreference MUST be
STRUCTURAL, not a bare phrase inside the claim text. Cite the establishing scene in
the fact's `evidence` array.

- RIGHT: a fact at the reckoning whose claim is "Garrick learns the relief gate was
  wedged", with `evidence: ["<reckoning-scene>", "<gate-scene>"]` where the gate
  scene is the one that established the wedged gate. The reference is a real edge the
  gates can see.
- WRONG: the same claim with only its own scene in `evidence` and the earlier event
  mentioned only in prose ("the gate from earlier that night"). That allusion is
  invisible to the gates and will read as ungrounded.
- A backreference's cited scene must be reachable AT OR BEFORE this fact in this
  fact's OWN world-line. You cannot cite as evidence a scene that happens only on a
  DIFFERENT branch, or a scene that comes later. (A gate enforces this — see below.)
  If two world-lines need to share an earlier event, that event belongs on the SHARED
  SPINE before the fork, where every branch can reach it.

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
     {"section_id":"sc-01","parent_doc":"harlow","title":"The rising pond","coverage_expectation":"informational"},
     {"section_id":"sc-02","parent_doc":"harlow","title":"...","coverage_expectation":"informational"}
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
     "branches": [{"branch_id":"sluice","description":"Sela opens the sluice","forks_from":"main","forks_at":"sc-08"}],
     "entities": [{"entity_id":"sela","kind":"person","description":"the de-facto keeper"}],
     "predicates":[{"predicate_id":"status","object_kind":"scalar","description":"a record's standing"}],
     "facts": [
       {"fact_id":"f-001","frame":"gt","claim":"Sela finds the relief gate wedged shut","canon_from":"sc-03","evidence":["sc-03"],"entities":["sela"],"payoff_expectation":"expected"},
       {"fact_id":"f-050","frame":"gt","branch":"sluice","claim":"Garrick learns the gate was wedged when he weighs Sela's choice","canon_from":"sc-15","evidence":["sc-15","sc-03"],"entities":["sela"],"pays_off":["f-001"]}
     ]
   }
   ```

4. **`order.json`** — the canon order (REQUIRED, or the gates cannot place your
   facts). The main spine is a chain of edges; each branch is its own edge chain
   starting at its fork scene:
   ```json
   {
     "edges": [["sc-01","sc-02"],["sc-02","sc-03"],["...","sc-08"]],
     "branches": {
       "sluice": [["sc-08","sc-09a"],["sc-09a","sc-10a"]],
       "ride":   [["sc-08","sc-09b"],["sc-09b","sc-10b"]]
     }
   }
   ```
   Pass `order.json` to EVERY gate as `--order order.json`.

## Field rules (read these — they save you loop iterations)

- A fact's `canon_from`, a section's `section_id`, a branch's `forks_at`, and every
  id in `evidence[]` must all be SCENE IDS that exist in `sections.json`.
- `branch`: the world-line a fact belongs to. OMIT it for facts on the shared spine
  before the first fork (those live on the implicit root branch `main`). After a
  fork, tag each fact with its branch id.
- `branches[].forks_from`: the parent branch id (`"main"` for the root spine).
  `forks_at` = the scene on the parent where this branch departs.
- `payoff_expectation`: ONLY the literal `"expected"` (mark a setup that must pay
  off) or `"unmarked"`/omit. It is NOT free text.
- `pays_off`: an ARRAY of the setup fact-ids this fact discharges, e.g. `["f-001"]`.
  The paying fact must be reachable after the setup in its world-line.
- `frame`: at least a ground-truth frame (e.g. `"gt"`). To make a character BELIEVE
  something false, add a belief frame (e.g. `"town-belief"`) and put the belief fact
  in that frame. Never mark a belief fact and a truth fact as conflicting — they are
  two true facts on two frames.
- `evidence`: at minimum the fact's own scene; ADD the establishing scene(s) for any
  backreference (the structural-backreference rule above).
- `typed` (OPTIONAL): `{"subject":"<entity-id>","predicate":"<predicate-id>","object":{"kind":"value","value":"wedged"}}` or `{"kind":"entity","id":"<entity-id>"}`. Typed setup+payoff pairs read as *substantiated* (stronger) than untyped; use it for the load-bearing reveals.

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
- `validate-continuity`: 0 structural + 0 interval violations. (This INCLUDES an
  evidence-reachability check: an `evidence` citation that is not reachable at-or-
  before the fact in its own world-line is a structural violation. If you see
  `evidence_unreachable` or `fact_canon_off_branch`, a coordinate or backreference
  points off this branch or forward — fix it by the rule above.)
- `report-fork-tree`: your fork point(s) are PLACED (not "UNPLACED"); every branch is
  registered; every world-line reaches a resolution.
- `report-playthrough-manuscript --world W`: for EVERY branch, 0 unplaced / 0
  undecidable — every fact sits in a real scene of that world, and reading the scenes
  in order tells that world's story start to finish. (An "outside order" count is
  normal: it is the OTHER branches' scenes correctly excluded from this world's walk.)
- `report-payoff-coverage`: no setup left dangling in any TERMINAL world.
- `report-timeline-gaps`: no gap/unreached scene in any world.

## Deliverables (leave these in `run/author/`)

- `sections.json`, `facts.json`, `order.json`, `store.atomic.json` (final, gate-clean).
- `author-log.md` — a log: how you laid the Phase-0 skeleton (the tentpoles you placed
  first), then how many write→gate→repair iterations you ran in Phase 1, what the
  gates flagged each pass, and what you changed. **Record honestly any point where the
  story you wanted to tell was awkward to express in the tool — anywhere you had to
  restructure, duplicate, or work around something to get the gates clean, note what
  it was and what you did.** (This records the authoring loop itself.)

## Scope reminder

~16–20 scenes; ONE exclusive fork producing the two distinct world-lines the premise
names; the shared opening spine before the fork; each path's distinct middle; and the
shared dawn resolution both paths reach. At least one long-range setup→payoff that
crosses the fork (the wedged gate). At least one character whose belief diverges from
the ground truth. Lay the skeleton top-down first, then fill. Tell the best, most
coherent version of this story you can — make every consequence trace to a cause you
have already placed, and leave nothing important dangling.
