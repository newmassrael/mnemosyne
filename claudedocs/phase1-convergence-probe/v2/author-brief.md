# Author brief — author a branching-and-converging story as a gate-checked fact base

You are a story author. You will be given a PREMISE (`premise.md`). Your job is to
invent a complete, coherent story whose world-lines DIVERGE at a choice and then
CONVERGE on a shared ending, and record it as a **fact base** using the
`mnemosyne-cli` tool, using the tool's consistency gates to check your own work as
you go and fixing whatever they flag.

You author the STRUCTURE OF THE STORY AS FACTS — not prose. (No prose narration is
required or graded.) Think of it as writing the story's bible: who, what, where,
what is true, what each character believes, what happens in each scene, how the
branches diverge, how they converge, how each path resolves.

**Read ONLY this file and `premise.md`. Do not open any other file under
`claudedocs/phase1-convergence-probe/` — those are experiment internals and reading
them would bias your work.** Work in the directory `run/author/` (create it under
the experiment folder you were pointed at); leave your deliverables there.

## The method is TOP-DOWN. This is the contract — follow it.

Do NOT free-associate scene by scene from the start. A branching story invented
bottom-up drifts: you discover at the climax that you needed a fact you never
planted. Instead author in two phases. The tool lets you place a fact at ANY scene
coordinate in ANY order — a late ending can be declared before an early scene exists
— so the skeleton can come first.

### Phase 0 — SCOPE + SKELETON (do this first, and get it gate-clean before Phase 1)

Decide the shape, then lay the TENTPOLES — the load-bearing facts the whole story
hangs on — BEFORE any connecting detail:

1. **Scope.** Fix the total scene count and the world-lines: name every branch, where
   it forks, AND where the paths converge. Reserve the scene-id ranges (the shared
   spine `sc-01..sc-NN`, then each branch's own middle range, then the SHARED ENDING's
   own range).
2. **Endings / destination first.** Author how the story RESOLVES as facts — what is
   finally true or done at the shared dawn reckoning + the river's edge, who ends
   where. You are writing toward this. Author it before the connecting detail.
3. **The fork AND the convergence.** Author the fork point (the choice, and enough of
   each resulting world-line's identity that the branches are genuinely distinct) AND
   the convergence point (the scene where the two paths re-join into the shared
   ending). See "Forks and confluences" below for how to declare each.
4. **Load-bearing entities/objects.** Register the people, objects, and facts the
   plot TURNS on (the wedged gate, whatever else you invent). Declaring them here, up
   front, IS you saying "these matter."
5. **Reveals AND their setups AND their preconditions, together.** For every major
   reveal (a truth that lands later), author its SETUP in the SAME pass — the planted
   detail, placed at an EARLIER scene than the reveal, marked as a setup that pays
   off. Never let a reveal exist without the earlier fact it depends on. **And when a
   reveal turns on a NEW entity or a NEW state that the story had not yet established
   (e.g. "X is actually still alive", "the gate was wedged on purpose by Y"), seed
   that precondition explicitly as its own earlier `payoff_expectation:"expected"`
   setup that the reveal `pays_off` — do not let the ground truth stay silent on a
   question the reveal answers.** A reader (and the gate) must be able to trace the
   reveal back to a planted cause.

Then run the gates over this skeleton and make it clean (see the gate list). A clean
skeleton is a spine you can trust.

### Phase 1 — DETAIL FILL

Now author the connecting detail between the tentpoles: the scenes that carry each
world-line through, the ordinary beats, the belief-frame facts that show characters
acting on what they (wrongly) believe. Re-run the gates as you go. Every new
consequence must trace to a cause already placed earlier in the SAME world-line (or
on the shared spine before the fork, or on the shared ending after the convergence).

## Forks and confluences — the two structural moves (READ THIS)

A world-line is declared as a branch. There are exactly two ways a branch relates to
the rest of the story, and a branch is EITHER one or the other, never both:

- **A FORK (divergence).** `forks_from` a single parent at a `forks_at` scene. The
  child inherits the parent's facts up to the fork point, then goes its own way. Use
  this for the ONE exclusive choice the premise names (the sluice path vs the ride
  path each fork from `main` at the choice scene).

- **A CONFLUENCE (convergence / merge).** `converges_from` TWO OR MORE parent
  world-lines, each at the scene on that parent where it joins. A confluence is the
  SHARED CONTINUATION that the listed parents flow INTO. **A fact authored on the
  confluence branch is authored ONCE and is part of EVERY parent world-line past the
  merge** — you do NOT duplicate the shared ending onto each path. Use this for the
  shared dawn reckoning + river's edge that BOTH paths must reach: declare ONE
  confluence branch (e.g. `dawn`) that converges from both the sluice path and the
  ride path, and author the reckoning/river facts ONCE on it.

This is the natural way to express "two paths come back together at one ending."
Author the shared ending on the confluence; author each path's distinct middle on its
own fork branch; author the common opening on the shared spine (`main`).

## What to produce (three JSON files + one store)

Work in `run/author/`. Start from an EMPTY store, then build it up.

1. **The empty seed store** — create `store.atomic.json` with exactly this content
   (note `schema_version` is **23**):
   ```json
   {"sections":{},"changelog_entries":{},"inventory_entries":{},"confirmation_events":{},"frames":{},"branches":{},"entities":{},"predicates":{},"narrative_facts":{},"disclosure_plans":{},"schema_version":23}
   ```

2. **`sections.json`** — your SCENES, as a JSON array. Every scene is a section. A
   canon coordinate (a scene id used by a fact) MUST exist here first.
   ```json
   [
     {"section_id":"sc-01","parent_doc":"harlow","title":"The rising pond","coverage_expectation":"informational"},
     {"section_id":"sc-02","parent_doc":"harlow","title":"...","coverage_expectation":"informational"}
   ]
   ```
   Load it:  `mnemosyne-cli import-sections --manifest sections.json --sidecar <ABS>/store.atomic.json`

3. **`facts.json`** — frames, branches, entities, predicates, facts. Load it:
   `mnemosyne-cli import-facts --manifest facts.json --sidecar <ABS>/store.atomic.json`
   (import-facts is ONE atomic transaction — if anything is invalid, NOTHING is
   written and it prints the error; fix and re-run. Import is additive; to CHANGE a
   row, rebuild the store from the empty seed and re-import the corrected facts.json.
   Keep facts.json the source of truth.)

   Schema (only the fields you need). Note the TWO branch shapes — a fork and a
   confluence:
   ```json
   {
     "frames":   [{"frame_id":"gt","description":"the ground truth — what actually happened"}],
     "branches": [
       {"branch_id":"sluice","description":"Sela opens the sluice","forks_from":"main","forks_at":"sc-08"},
       {"branch_id":"ride","description":"Sela holds and rides","forks_from":"main","forks_at":"sc-08"},
       {"branch_id":"dawn","description":"the shared dawn both paths reach","converges_from":[{"branch":"sluice","at":"sc-12a"},{"branch":"ride","at":"sc-12b"}]}
     ],
     "entities": [{"entity_id":"sela","kind":"person","description":"the de-facto keeper"}],
     "predicates":[{"predicate_id":"status","object_kind":"scalar","description":"a record's standing"}],
     "facts": [
       {"fact_id":"f-001","frame":"gt","claim":"Sela finds the relief gate wedged shut","canon_from":"sc-03","evidence":["sc-03"],"entities":["sela"],"payoff_expectation":"expected"},
       {"fact_id":"f-300","frame":"gt","branch":"dawn","claim":"the wedged-gate truth surfaces and turns Garrick's verdict","canon_from":"sc-rk","evidence":["sc-rk","sc-03"],"entities":["sela"],"pays_off":["f-001"]}
     ]
   }
   ```
   - For a confluence branch: give `converges_from` (an array of `{branch, at}`, one
     per parent, ≥ 2 parents), and OMIT `forks_from`/`forks_at`. Each parent must
     already be registered (list the fork branches before the confluence). `at` = the
     scene on that parent where it joins the shared ending (the parent's LAST distinct
     scene before the merge).
   - A fact on the shared ending lives on the confluence branch (`"branch":"dawn"`)
     and is authored ONCE — it shows up in every parent's playthrough automatically.

4. **`order.json`** — the canon order (REQUIRED, or the gates cannot place your
   facts). The main spine is a chain of edges; each FORK branch is its own edge chain
   starting at its fork scene; the CONFLUENCE's shared ending is wired by giving, in
   EACH parent's chain, the merge edge `[<parent's last scene>, <shared-ending's first
   scene>]`, and giving the confluence branch its own internal chain for the shared
   ending's scenes:
   ```json
   {
     "edges": [["sc-01","sc-02"],["sc-02","sc-08"]],
     "branches": {
       "sluice": [["sc-08","sc-09a"],["sc-09a","sc-12a"],["sc-12a","sc-rk"]],
       "ride":   [["sc-08","sc-09b"],["sc-09b","sc-12b"],["sc-12b","sc-rk"]],
       "dawn":   [["sc-rk","sc-rv"]]
     }
   }
   ```
   Here both `sc-12a` (last sluice scene) and `sc-12b` (last ride scene) have an edge
   INTO `sc-rk` (the shared reckoning, first scene of the `dawn` confluence), and the
   `dawn` chain carries `sc-rk → sc-rv` (the shared ending's internal order). The tool
   composes each parent world's order as: spine ∪ that parent's middle ∪ the shared
   ending — so reading `--world sluice` walks `…sc-12a → sc-rk → sc-rv`, and
   `--world ride` walks `…sc-12b → sc-rk → sc-rv`, the SAME shared ending in both.
   Pass `order.json` to EVERY gate as `--order order.json`.

## PATHS — use ABSOLUTE paths for `--sidecar` and `--order`

`--manifest` is read relative to your current directory, but **`--sidecar` and
`--order`, when given as a RELATIVE path, resolve relative to the workspace root (the
repo), not your current directory.** To avoid confusion, pass ABSOLUTE paths for
`--sidecar` and `--order` in every command (e.g. `--sidecar
/abs/path/to/run/author/store.atomic.json`). `pwd` once and build the absolute paths.

## Field rules (read these — they save you loop iterations)

- A fact's `canon_from`, a section's `section_id`, a branch's `forks_at`, a
  confluence parent's `at`, and every id in `evidence[]` must all be SCENE IDS that
  exist in `sections.json`.
- `branch`: the world-line a fact belongs to. OMIT it for facts on the shared spine
  before the first fork (those live on the implicit root branch `main`). After a
  fork, tag each fact with its fork branch id. For the shared ending, tag each fact
  with the CONFLUENCE branch id (`"branch":"dawn"`) — authored once there.
- `forks_from`/`forks_at`: the parent branch id (`"main"` for the root spine) and the
  scene on the parent where this branch departs. Mutually exclusive with
  `converges_from`.
- `converges_from`: an array of `{branch, at}` (≥ 2). Mutually exclusive with
  `forks_from`/`forks_at`.
- `payoff_expectation`: ONLY the literal `"expected"` (mark a setup that must pay
  off) or `"unmarked"`/omit. It is NOT free text.
- `pays_off`: an ARRAY of the setup fact-ids this fact discharges, e.g. `["f-001"]`.
  The paying fact must be reachable after the setup in its world-line. A shared-ending
  fact on the confluence CAN pay off a setup planted on the shared spine before the
  fork (the spine is reachable from the confluence through either parent).
- `frame`: at least a ground-truth frame (e.g. `"gt"`). To make a character BELIEVE
  something false, add a belief frame (e.g. `"town-belief"`) and put the belief fact
  in that frame. Never mark a belief fact and a truth fact as conflicting — they are
  two true facts on two frames.
- `evidence`: at minimum the fact's own scene; ADD the establishing scene(s) for any
  backreference (the structural-backreference rule below).
- `typed` (OPTIONAL): `{"subject":"<entity-id>","predicate":"<predicate-id>","object":{"kind":"value","value":"wedged"}}` or `{"kind":"entity","id":"<entity-id>"}`. Typed setup+payoff pairs read as *substantiated* (stronger) than untyped; use it for the load-bearing reveals.

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
  mentioned only in prose. That allusion is invisible to the gates and will read as
  ungrounded.
- A backreference's cited scene must be reachable AT OR BEFORE this fact in this
  fact's OWN world-line. You cannot cite as evidence a scene that happens only on a
  DIFFERENT branch, or a scene that comes later. A gate enforces this.
- **Confluence corollary:** a fact on the shared ending (the confluence) is reached
  through BOTH parents. So any scene it cites as evidence must be reachable from
  EVERY incoming parent — which means it must sit on the SHARED SPINE before the
  fork, or on the shared ending itself. You may NOT cite, from the shared ending, a
  scene that exists only on ONE path (the sluice-only middle or the ride-only
  middle) — that evidence is unreachable from the other parent, and a gate
  (`confluence_evidence_unreconciled`) will flag it. If the shared ending needs to
  reflect what a specific path did, author that as a path-specific fact on the FORK
  branch (the reckoning's content differs by path), and keep the confluence facts'
  evidence on the shared spine / shared ending.

## The gates (run all, with --order, after each import)

```
mnemosyne-cli validate-continuity            --order <ABS>/order.json --sidecar <ABS>/store.atomic.json
mnemosyne-cli report-fork-tree               --order <ABS>/order.json --sidecar <ABS>/store.atomic.json
mnemosyne-cli report-timeline-gaps --world <each-branch> --order <ABS>/order.json --sidecar <ABS>/store.atomic.json
mnemosyne-cli report-payoff-coverage         --order <ABS>/order.json --sidecar <ABS>/store.atomic.json
mnemosyne-cli report-payoff-substantiation   --order <ABS>/order.json --sidecar <ABS>/store.atomic.json
mnemosyne-cli report-playthrough-manuscript --world <each-parent> --order <ABS>/order.json --sidecar <ABS>/store.atomic.json
```

Read them and FIX until:
- `validate-continuity`: 0 structural + 0 interval violations. (This INCLUDES an
  evidence-reachability check: an `evidence` citation not reachable at-or-before the
  fact in its own world-line is a structural violation. `evidence_unreachable` /
  `fact_canon_off_branch` = a coordinate or backreference points off this branch or
  forward; `confluence_evidence_unreconciled` = a shared-ending fact cites a scene
  reachable from only ONE parent — fix per the confluence corollary above.)
- `report-fork-tree`: your fork point(s) are PLACED (not "UNPLACED"); your confluence
  shows as `converges from <parent> at <coord>` for each parent (the diamond is
  visible); every branch is registered; every world-line reaches a resolution.
- `report-playthrough-manuscript --world W`: for EACH PARENT world-line (the fork
  branches, e.g. `sluice` and `ride`), 0 unplaced / 0 undecidable — every fact sits
  in a real scene of that world, and reading the scenes in order tells that world's
  story start to finish INCLUDING the shared ending (you should see the shared
  reckoning + river's edge scenes at the tail of BOTH parents' walks, authored once).
  (An "outside order" count is normal: it is the OTHER path's scenes correctly
  excluded from this world's walk.)
- `report-payoff-coverage`: no setup left dangling in any TERMINAL world.
- `report-timeline-gaps`: no gap/unreached scene in any world.

(Note: you query playthroughs with `--world sluice` and `--world ride` — the two
PARENT paths a reader actually takes. The confluence branch `dawn` is the shared
continuation, not a standalone playthrough, so you don't walk it directly; its facts
appear inside each parent's walk.)

## Deliverables (leave these in `run/author/`)

- `sections.json`, `facts.json`, `order.json`, `store.atomic.json` (final, gate-clean).
- `author-log.md` — a log: how you laid the Phase-0 skeleton (the tentpoles you placed
  first, including the fork and the confluence), then how many write→gate→repair
  iterations you ran in Phase 1, what the gates flagged each pass, and what you
  changed. **Record honestly any point where the story you wanted to tell was awkward
  to express in the tool — anywhere you had to restructure, duplicate, or work around
  something to get the gates clean, note what it was and what you did. In particular,
  note how you handled the shared dawn ending both paths reach: what was natural, what
  was awkward, whether anything forced you to duplicate or compromise the shared
  ending.** (This records the authoring loop itself.)

## Scope reminder

~16–20 scenes; ONE exclusive fork producing the two distinct world-lines the premise
names; the shared opening spine before the fork; each path's distinct middle; and the
shared dawn reckoning + river's edge BOTH paths reach, authored ONCE on a confluence.
At least one long-range setup→payoff that crosses the fork and lands at the shared
reckoning (the wedged gate). At least one character whose belief diverges from the
ground truth. Lay the skeleton top-down first, then fill. Tell the best, most coherent
version of this story you can — make every consequence trace to a cause you have
already placed, and leave nothing important dangling.
