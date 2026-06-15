# Author brief — author a CROWDED-HOUSE mystery as a gate-checked fact base

You are a story author. You will be given a PREMISE (`premise.md`). Your job is to
invent a complete, coherent branching story with a BROAD CAST OF PEOPLE — each of
whom knows and believes different things — and record it as a **fact base** using
the `mnemosyne-cli` tool, using the tool's consistency gates to check your own work
as you go and fixing whatever they flag.

You author the STRUCTURE OF THE STORY AS FACTS — not prose. (No prose narration is
required or graded.) Think of it as writing the story's bible: who is here, what is
really true, what each person knows and believes, what happens in each scene, how
the branches diverge, how each branch ends.

**Read ONLY this file and `premise.md`. Do not open any other file under
`claudedocs/phase1-ai-npc-breadth-experiment/` — those are experiment internals and
reading them would bias your work.** Work in the directory `run/author/` (it exists);
leave your deliverables there.

## The heart of this story: a CAST WITH INDIVIDUATED KNOWLEDGE

A waystation full of strangers is the point. The substrate represents "what a
particular person knows / believes" with a FRAME: a named epistemic point of view.
You will use MANY frames — one ground-truth frame plus **a distinct frame for each
significant person in the house** (the deputy, the clerk, the wool-factor, the
soldier, the hooded traveller, and so on). A fact placed in a person's frame is part
of THAT person's knowledge; the same event can be known by some frames and unknown to
others, and two people can believe contradictory things — each is a true fact about a
different mind.

This is what makes the crowded house real: not one truth with a crowd of bystanders,
but eight-plus people who each hold a different fragment, several of them wrong about
what the others know. Author the cast that way. A person should know what they were
present for or were told, and should NOT hold knowledge they had no way to come by —
let each person's frame carry exactly their own road through these nights.

You can ask the tool what any person knows at any point, and use it to check yourself:

```
mnemosyne-cli report-frame-view --frame <person-frame> --branch <world> --entity <who/what> --at <scene> --order order.json --sidecar store.atomic.json
```

It lists exactly the facts that person holds about that subject at that scene (and
counts what they do NOT hold / cannot yet decide). Use it to confirm each person knows
what you intend and no more.

## The method is TOP-DOWN. This is the contract — follow it.

Do NOT free-associate scene by scene from the start; a multi-thread mystery invented
bottom-up drifts (you discover at the reveal that you needed a fact you never
planted). The tool lets you place a fact at ANY scene coordinate in ANY order — a
late ending can be declared before the opening exists — so the skeleton comes first.

### Phase 0 — SCOPE + SKELETON (get it gate-clean before Phase 1)

1. **Scope.** Fix the total scene count (≈22–30) and the world-lines: name every
   branch (NAME / HOLD / ACT) and the single scene where they fork. Reserve scene-id
   ranges (shared spine `sc-01..sc-NN`, then each branch's own range).
2. **The cast as frames.** Register the ground-truth frame and a frame for each
   significant person. Decide, up front, the SHAPE of who-knows-what: who witnessed
   the inciting event, who only heard of it, who is lying, whose belief is wrong.
3. **The inciting truth + endings first.** Author what is REALLY true behind the
   inciting event (ground-truth frame), then, for EACH terminal world-line, author its
   ENDING as facts — how that road resolves, who ends where, what is finally known or
   done. You are writing toward these.
4. **The fork.** Author the fork point: Wend's choice (NAME / HOLD / ACT) and enough
   of each resulting world-line's identity that the three roads are genuinely distinct.
5. **Load-bearing knowledge + objects.** Register the people, objects, and the
   fragments of knowledge the plot TURNS on (the thing only one traveller saw, the
   missing paper, the lie). Declaring them here, up front, IS you saying "these matter."
6. **Reveals AND their setups, together.** For every reveal (a truth that lands
   later — to Wend or to the house), author its SETUP in the SAME pass: the earlier
   planted fact it depends on, placed at an EARLIER scene, marked as a setup that pays
   off. Never let a reveal exist without the earlier fact it rests on.

Then run the gates over the skeleton and make it clean before filling detail.

### Phase 1 — DETAIL FILL

Author the connecting detail: the scenes carrying each world-line from fork to ending,
the ordinary beats, and the belief-frame facts that show people acting on what they
(wrongly) believe. Re-run the gates as you go. Every new consequence must trace to a
cause already placed earlier in the SAME world-line, and every person's knowledge must
trace to a scene they were part of.

## Seed every NEW precondition a reveal needs (read this — it prevents a silent hole)

When a reveal introduces a NEW person, object, or STATE that the earlier story never
established, that new thing is a question the story must already have opened. Per
reveal, seed its preconditions in the skeleton:

- A reveal that "the wool-factor's strongbox was already empty when he arrived" needs
  an earlier established fact that opens the question of the strongbox's contents — not
  silence broken only at the reveal.
- Concretely: open the question as a SETUP earlier (a fact with
  `payoff_expectation:"expected"`) that the reveal `pays_off`. Then the payoff gate
  confirms the reveal rests on something you planted.
- Corollary: the ground-truth frame must not stay SILENT on a question a character's
  belief turns on. If someone believes X about an event, the ground truth must say
  something about that event (X or not-X), so the belief has a truth to diverge from.

## Structural backreferences (REQUIRED — part of the contract)

When a later scene REFERS BACK to an earlier event — a callback, "the warning the
tinker swore he gave", "the same hand that signed the ledger" — that backreference
MUST be STRUCTURAL, not a bare phrase in the claim text. Cite the establishing scene
in the fact's `evidence` array.

- RIGHT: a fact at `sc-24` whose claim is "Wend matches the seal to the clerk's
  papers", with `evidence:["sc-24","sc-07"]` where `sc-07` established the seal. A real
  edge the gates can see.
- WRONG: the same claim with `evidence:["sc-24"]` and `sc-07` mentioned only in prose.
  Invisible to the gates; reads as ungrounded.
- A backreference's cited scene must be reachable AT OR BEFORE this fact in this
  fact's OWN world-line. You cannot cite a scene that happens only on a DIFFERENT
  branch, or a later scene. If two world-lines must share an earlier event, that event
  belongs on the SHARED SPINE before the fork. (A gate enforces this — `evidence_unreachable`.)

## What to produce (three JSON files + one store)

Work in `run/author/`. Run every command from there; relative `--sidecar` / `--order`
resolve to that directory.

1. **The empty seed store** — create `store.atomic.json` with exactly this content
   (note `schema_version` is **23**):
   ```json
   {"sections":{},"changelog_entries":{},"inventory_entries":{},"confirmation_events":{},"frames":{},"branches":{},"entities":{},"predicates":{},"narrative_facts":{},"disclosure_plans":{},"schema_version":23}
   ```

2. **`sections.json`** — your SCENES, a JSON array. Every scene is a section; a canon
   coordinate (a scene id used by a fact) MUST exist here first.
   ```json
   [
     {"section_id":"sc-01","parent_doc":"corvath","title":"The river rising","coverage_expectation":"informational"},
     {"section_id":"sc-02","parent_doc":"corvath","title":"...","coverage_expectation":"informational"}
   ]
   ```
   Load:  `mnemosyne-cli import-sections --manifest sections.json --sidecar store.atomic.json`

3. **`facts.json`** — frames, branches, entities, predicates, facts. Load:
   `mnemosyne-cli import-facts --manifest facts.json --sidecar store.atomic.json`
   (ONE atomic transaction — if anything is invalid, NOTHING is written and it prints
   the error; fix and re-run. Re-running on the same store is fine; identical rows are
   no-ops. To CHANGE a row, edit facts.json and rebuild from the empty seed + re-import
   — import is additive. Keep facts.json the source of truth.)

   Schema (only the fields you need):
   ```json
   {
     "frames":   [
       {"frame_id":"gt","description":"the ground truth — what actually happened"},
       {"frame_id":"wend","description":"what the deputy ford-warden knows/believes"},
       {"frame_id":"clerk","description":"what the magistrate's clerk knows/believes"}
     ],
     "branches": [{"branch_id":"name","description":"Wend lays it before the law","forks_from":"main","forks_at":"sc-18"}],
     "entities": [{"entity_id":"wend","kind":"person","description":"the deputy ford-warden"},
                  {"entity_id":"strongbox","kind":"item","description":"the wool-factor's box"}],
     "predicates":[{"predicate_id":"culpability","object_kind":"scalar","description":"who did the thing"}],
     "facts": [
       {"fact_id":"f-001","frame":"gt","claim":"the strongbox was empty before it ever reached the ford","canon_from":"sc-04","evidence":["sc-04"],"entities":["strongbox"],"payoff_expectation":"expected"},
       {"fact_id":"f-002","frame":"clerk","claim":"the clerk believes the soldier robbed the box in the night","canon_from":"sc-09","evidence":["sc-09"],"entities":["strongbox"]},
       {"fact_id":"f-040","frame":"wend","branch":"name","claim":"Wend tells the clerk the box was empty on arrival","canon_from":"sc-22","evidence":["sc-22","sc-04"],"entities":["strongbox"],"pays_off":["f-001"]}
     ]
   }
   ```

4. **`order.json`** — the canon order (REQUIRED, or the gates cannot place facts). The
   main spine is a chain of edges; each branch is its own edge chain starting at its
   fork scene:
   ```json
   {
     "edges": [["sc-01","sc-02"],["sc-02","sc-03"],["...","sc-18"]],
     "branches": {
       "name": [["sc-18","sc-19"],["sc-19","sc-20"]],
       "hold": [["sc-18","sc-30"],["sc-30","sc-31"]],
       "act":  [["sc-18","sc-40"],["sc-40","sc-41"]]
     }
   }
   ```
   Pass `order.json` to EVERY gate as `--order order.json`.

## Field rules (read these — they save loop iterations)

- A fact's `canon_from`, a section's `section_id`, a branch's `forks_at`, and every id
  in `evidence[]` must all be SCENE IDS that exist in `sections.json`.
- `branch`: the world-line a fact belongs to. OMIT it for facts on the shared spine
  before the fork (those live on the implicit root branch `main`). After the fork, tag
  each fact with its branch id.
- `frame`: the point of view a fact belongs to. Ground truth goes in `gt`; what a
  person knows/believes goes in THAT person's frame. Never mark a belief fact and a
  truth fact as `conflicts` — they are two true facts on two frames (one about the
  world, one about a mind).
- `payoff_expectation`: ONLY the literal `"expected"` (mark a setup that must pay off)
  or omit. `pays_off`: an ARRAY of the setup fact-ids a fact discharges, e.g. `["f-001"]`;
  the paying fact must be reachable after the setup in its world-line.
- `evidence`: at minimum the fact's own scene; ADD the establishing scene(s) for any
  backreference (the rule above).
- `typed` (OPTIONAL, use for the load-bearing reveals): `{"subject":"<entity>","predicate":"<predicate>","object":{"kind":"value","value":"forged"}}` or `{"kind":"entity","id":"<entity>"}`. Typed setup+payoff pairs read as *substantiated* (stronger) than untyped.

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
- `validate-continuity`: 0 structural + 0 interval violations. (Includes
  evidence-reachability: an `evidence` citation not reachable at-or-before the fact in
  its own world-line is a structural violation `evidence_unreachable` — move the cited
  event onto the shared spine, or cite a scene that actually precedes this fact here.)
- `report-fork-tree`: your fork point is PLACED (not "UNPLACED"); every branch is
  registered; every world-line reaches a terminal.
- `report-playthrough-manuscript --world W`: for EVERY branch, 0 unplaced / 0
  undecidable — every fact sits in a real scene of that world and the scenes in order
  tell that world's story start to finish. (An "outside order" count is normal: the
  OTHER branches' scenes correctly excluded from this world's walk.)
- `report-payoff-coverage`: no setup left dangling in any TERMINAL world.
- `report-timeline-gaps`: no gap / unreached scene in any world.

## Deliverables (leave these in `run/author/`)

- `sections.json`, `facts.json`, `order.json`, `store.atomic.json` (final, gate-clean).
- `author-log.md` — a log: the Phase-0 skeleton you laid (the cast of frames + the
  tentpoles), then how many write→gate→repair iterations you ran in Phase 1, what the
  gates flagged each pass, and what you changed. Record any time a gate caught a
  knowledge or backreference problem and how you fixed it.

## Scope reminder

≈22–30 scenes; exactly ONE primary fork (NAME / HOLD / ACT) into 3 distinct terminal
world-lines, each with its own distinct ending; **at least EIGHT distinct people, each
with their own frame holding what they know and believe** (a real, individuated cast,
not two principals and a crowd); at least two real setup→payoffs (at least one a piece
of knowledge only one traveller held); several characters whose beliefs diverge from
the ground truth and from each other, each acting on what they believe and could have
learned. Lay the skeleton top-down first, then fill. Tell the best, most coherent
crowded-house mystery you can — make every consequence trace to a placed cause, make
each person know exactly their own road and no more, and leave nothing important
dangling.
