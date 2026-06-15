# Author brief (Stage 1) — author the Holm story as a gate-checked fact base

You are a story author. You will be given a PREMISE (`premise.md`). Your job is to
invent a complete, coherent branching story with a BROAD CAST OF PEOPLE — each of
whom knows and believes different things — and record it as a **fact base** using
the `mnemosyne-cli` tool, using the tool's consistency gates to check your own work
as you go and fixing whatever they flag.

You author the STRUCTURE OF THE STORY AS FACTS — not prose. (No prose narration is
written or graded in THIS stage.) Think of it as writing the story's bible: who is
here, what is really true, what each person knows and believes, what happens in each
scene, how the branches diverge, how each road's aftermath plays out, how each ends.
A later, separate stage will turn your fact base into spoken scenes — so make each
person's knowledge, agenda, and stake CLEAR and DISTINCT in the facts, but do not
write dialogue here.

**Read ONLY this file and `premise.md`. Do not open any other file under
`claudedocs/phase1-npc-dialogue-experiment/` — those are experiment internals and
reading them would bias your work.** Work in the directory `run/author/` (it exists);
leave your deliverables there.

## The heart of this story: a CAST WITH INDIVIDUATED KNOWLEDGE

A shut-in community plus castaways is the point. The substrate represents "what a
particular person knows / believes" with a FRAME: a named epistemic point of view.
You will use MANY frames — one ground-truth frame plus **a distinct frame for each
significant person on the Holm** (Halsa, the riding-officer, the mate, the passenger,
the headman, the salt-wife, the pilot, the boy, the factor's man, and so on). A fact
placed in a person's frame is part of THAT person's knowledge; the same event can be
known by some frames and unknown to others, and two people can believe contradictory
things — each is a true fact about a different mind.

This is what makes the shut-in island real: not one truth with a crowd of bystanders,
but ten-plus people who each hold a different fragment, several of them wrong about
what the others know. Author the cast that way. A person should know what they were
present for or were told, and should NOT hold knowledge they had no way to come by —
let each person's frame carry exactly their own road through the wreck-night and the
days after.

Check yourself with:

```
mnemosyne-cli report-frame-view --frame <person-frame> --branch <world> --entity <who/what> --at <scene> --order order.json --sidecar store.atomic.json
```

It lists exactly the facts that person holds about that subject at that scene. Use it
to confirm each person knows what you intend and no more.

## The method is TOP-DOWN. This is the contract — follow it.

Do NOT free-associate scene by scene from the start; a multi-thread mystery invented
bottom-up drifts. The tool lets you place a fact at ANY scene coordinate in ANY order
— a late ending can be declared before the opening exists — so the skeleton comes
first.

### Phase 0 — SCOPE + SKELETON (get it gate-clean before Phase 1)

1. **Scope.** Fix the total scene count (≈30–34) and the world-lines: name every
   branch (REPORT / SHELTER / CONFRONT) and the single scene where they fork —
   roughly the 40–50% mark, NOT the end, so each road has a real TAIL after it.
   Reserve scene-id ranges (shared spine `sc-01..sc-NN`, then each branch's own range
   for its aftermath).
2. **The cast as frames.** Register the ground-truth frame and a frame for each
   significant person (aim for TEN-PLUS person-frames). Decide, up front, the SHAPE
   of who-knows-what: who witnessed the wreck-night, who only heard of it, who is
   lying, whose belief is wrong.
3. **The wreck-night truth + endings first.** Author what is REALLY true behind the
   wreck-night (ground-truth frame): accident or false light, drowning or killing,
   cargo all ashore or not. Then, for EACH terminal world-line, author its ENDING as
   facts — how that road resolves, who ends where, what is finally known, done, or
   lost on the Holm. You are writing toward these.
4. **The fork.** Author the fork point: Halsa's choice (REPORT / SHELTER / CONFRONT)
   and enough of each resulting road's identity that the three aftermaths are
   genuinely distinct.
5. **Load-bearing knowledge + objects.** Register the people, objects, and fragments
   of knowledge the plot TURNS on (the thing only the boy saw, the locked chest, the
   false-light lantern, the short cask of cargo). Declaring them here, up front, IS
   you saying "these matter."
6. **Reveals AND their setups, together.** For every reveal (a truth that lands later
   — to Halsa or to the Holm), author its SETUP in the SAME pass: the earlier planted
   fact it depends on, placed at an EARLIER scene, marked as a setup that pays off.
   Never let a reveal exist without the earlier fact it rests on.

Then run the gates over the skeleton and make it clean before filling detail.

### Phase 1 — DETAIL FILL (and keep the Holm peopled to the last scene)

Author the connecting detail: the scenes carrying each world-line from fork to ending,
the ordinary beats, and the belief-frame facts that show people acting on what they
(wrongly) believe. Re-run the gates as you go. Every new consequence must trace to a
cause already placed earlier in the SAME world-line, and every person's knowledge must
trace to a scene they were part of.

**KEEP THE CAST ALIVE INTO EACH AFTERMATH (read this — it is the recurring failure to
beat).** The easiest way a crowded story goes wrong is that the early scenes are full
of people and the LATE scenes — each road's aftermath after the fork — shrink to two
or three principals while everyone else quietly vanishes. Do not let the Holm empty
out. On EACH of the three roads, the secondary people (the salt-wife, the boy, the
pilot, the widow, the factor's man, the headman, and the rest) should still be PRESENT
and still KNOWING and ACTING in that road's tail scenes — each carrying their own
fragment into how the road closes. Give several of them a fact (a thing they do, learn,
or still believe) in the post-fork tail of each world-line, not only on the spine.
Aim for the aftermath of every road to be as peopled as its opening.

## Type the load-bearing POSSESSION, and declare the rule (do this for the plot pivots)

Some objects in this story can be held by only ONE party at a time, and WHO holds them
is the pivot of the mystery (the locked chest; the short cask of cargo; the lantern
that may have shown the false light). For each such load-bearing object:

- give it an entity and a `possession` (or `holds`) predicate, and TYPE the facts that
  move it — `typed:{"subject":"<holder-entity>","predicate":"possession","object":{"kind":"entity","id":"<object>"}}`
  — at each scene where it changes hands;
- declare an EXCLUSIVE rule for that predicate in `narrative-rules.json` so the gate
  enforces one-holder-at-a-time and fires if two parties are shown holding it without a
  transfer between them:
  ```json
  { "rules": [ { "kind": "exclusive", "predicate": "possession", "per": "object" } ] }
  ```
  Pass it to the gate: `validate-continuity --rules narrative-rules.json --order order.json --sidecar store.atomic.json`.

A succeeding holder SUPERSEDES the prior one in that frame at the transfer scene
(`supersedes_in_frame`), so the chain of who-held-what reads cleanly and the gate
accepts the legitimate hand-offs while catching an impossible co-holding.

## A light DISCLOSURE PLAN (so the later render cannot blurt the solution early)

The central solution of the wreck-night (who did the load-bearing thing — the false
light, or the killing, or where the short cargo went) should land at a REVEAL scene on
each road, not be stated from the start. Author ONE disclosure plan ("telling") over
this fact base:

```
mnemosyne-cli add-disclosure-plan --telling holm --default-mode withhold --sidecar store.atomic.json
mnemosyne-cli set-disclosure --telling holm --fact <the-solution-fact> --mode withhold \
    --first-at report=<reveal-scene-on-report> --first-at shelter=<...> --first-at confront=<...> \
    --sidecar store.atomic.json
```

Mark the one or two LOAD-BEARING solution facts `withhold` with a `first-at` per
world-line (the scene on that road where it is finally revealed). These facts must be
TYPED (the disclosure gate matches on the typed claim). Keep the plan SPARSE — only the
true load-bearing reveals. Confirm with `report-disclosure-coverage --telling holm`.

## Seed every NEW precondition a reveal needs (prevents a silent hole)

When a reveal introduces a NEW person, object, or STATE the earlier story never
established, that new thing is a question the story must already have opened. Per
reveal, seed its preconditions in the skeleton:

- Open the question as a SETUP earlier (a fact with `payoff_expectation:"expected"`)
  that the reveal `pays_off`. Then the payoff gate confirms the reveal rests on
  something you planted.
- Corollary: the ground-truth frame must not stay SILENT on a question a character's
  belief turns on. If someone believes X about the wreck-night, the ground truth must
  say something about that event (X or not-X), so the belief has a truth to diverge
  from.

## Structural backreferences (REQUIRED — part of the contract)

When a later scene REFERS BACK to an earlier event — a callback, "the light the pilot
swore he saw", "the same hand that signed the bill of lading" — that backreference
MUST be STRUCTURAL, not a bare phrase in the claim text. Cite the establishing scene
in the fact's `evidence` array.

- RIGHT: a fact at `sc-26` whose claim is "Halsa matches the lantern to the pilot's
  store", with `evidence:["sc-26","sc-07"]` where `sc-07` established the lantern.
- WRONG: the same claim with `evidence:["sc-26"]` and `sc-07` mentioned only in prose.
- A backreference's cited scene must be reachable AT OR BEFORE this fact in this
  fact's OWN world-line. You cannot cite a scene that happens only on a DIFFERENT
  branch, or a later scene. If two world-lines must share an earlier event, that event
  belongs on the SHARED SPINE before the fork. (A gate enforces this —
  `evidence_unreachable`.)

## What to produce (JSON files + one store)

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
     {"section_id":"sc-01","parent_doc":"cawdy","title":"The light goes out on the Sneck","coverage_expectation":"informational"}
   ]
   ```
   Load:  `mnemosyne-cli import-sections --manifest sections.json --sidecar store.atomic.json`

3. **`facts.json`** — frames, branches, entities, predicates, facts. Load:
   `mnemosyne-cli import-facts --manifest facts.json --sidecar store.atomic.json`
   (ONE atomic transaction — if anything is invalid, NOTHING is written and it prints
   the error; fix and re-run. Import is additive; identical rows are no-ops. To CHANGE
   a row, edit facts.json and rebuild from the empty seed + re-import. Keep facts.json
   the source of truth.)

   Schema (only the fields you need):
   ```json
   {
     "frames":   [
       {"frame_id":"gt","description":"the ground truth — what actually happened"},
       {"frame_id":"halsa","description":"what the keeper's child knows/believes"},
       {"frame_id":"officer","description":"what the riding-officer knows/believes"}
     ],
     "branches": [{"branch_id":"report","description":"Halsa lays it before the Crown","forks_from":"main","forks_at":"sc-15"}],
     "entities": [{"entity_id":"halsa","kind":"person","description":"the keeper's child"},
                  {"entity_id":"chest","kind":"item","description":"the passenger's locked sea-chest"}],
     "predicates":[{"predicate_id":"possession","object_kind":"entity","description":"who holds the object"}],
     "facts": [
       {"fact_id":"f-001","frame":"gt","claim":"the lantern on the Ness headland showed a false channel light","canon_from":"sc-02","evidence":["sc-02"],"entities":["lantern"],"payoff_expectation":"expected"},
       {"fact_id":"f-002","frame":"pilot","claim":"the pilot saw a light where no light should be","canon_from":"sc-03","evidence":["sc-03"],"entities":["lantern"]},
       {"fact_id":"f-050","frame":"halsa","branch":"report","claim":"Halsa names the false light to the officer","canon_from":"sc-26","evidence":["sc-26","sc-02"],"entities":["lantern"],"pays_off":["f-001"]}
     ]
   }
   ```

4. **`order.json`** — the canon order (REQUIRED). The main spine is a chain of edges;
   each branch is its own edge chain starting at its fork scene:
   ```json
   {
     "edges": [["sc-01","sc-02"],["...","sc-15"]],
     "branches": {
       "report":  [["sc-15","sc-16"],["sc-16","sc-17"]],
       "shelter": [["sc-15","sc-24"],["sc-24","sc-25"]],
       "confront":[["sc-15","sc-32"],["sc-32","sc-33"]]
     }
   }
   ```
   Pass `order.json` to EVERY gate as `--order order.json`.

5. **`narrative-rules.json`** — the exclusive rule(s) for the load-bearing possession
   (above). Pass with `--rules narrative-rules.json`.

## Field rules (read these — they save loop iterations)

- A fact's `canon_from`, a section's `section_id`, a branch's `forks_at`, and every id
  in `evidence[]` must all be SCENE IDS that exist in `sections.json`.
- `branch`: the world-line a fact belongs to. OMIT it for facts on the shared spine
  before the fork (root branch `main`). After the fork, tag each fact with its branch.
- `frame`: the point of view a fact belongs to. Ground truth goes in `gt`; what a
  person knows/believes goes in THAT person's frame. Never mark a belief fact and a
  truth fact as `conflicts` — they are two true facts on two frames.
- `payoff_expectation`: ONLY the literal `"expected"` or omit. `pays_off`: an ARRAY of
  the setup fact-ids a fact discharges; the paying fact must be reachable after the
  setup in its world-line.
- `evidence`: at minimum the fact's own scene; ADD the establishing scene(s) for any
  backreference.
- `typed` (use for the load-bearing reveals AND the possession pivots):
  `{"subject":"<entity>","predicate":"<predicate>","object":{"kind":"value","value":"false-light"}}`
  or `{"kind":"entity","id":"<entity>"}`. Typed setup+payoff pairs read as
  *substantiated* (stronger) than untyped; the disclosure gate needs the withheld
  reveals typed.

## The gates (run all, with --order and --rules, after each import)

```
mnemosyne-cli validate-continuity            --order order.json --rules narrative-rules.json --sidecar store.atomic.json
mnemosyne-cli report-fork-tree               --order order.json --sidecar store.atomic.json
mnemosyne-cli report-timeline-gaps --world <each-branch> --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-coverage         --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-substantiation   --order order.json --sidecar store.atomic.json
mnemosyne-cli report-disclosure-coverage --telling holm --sidecar store.atomic.json
mnemosyne-cli report-playthrough-manuscript --world <each-branch> --order order.json --sidecar store.atomic.json
```

Read them and FIX until:
- `validate-continuity`: 0 structural + 0 interval violations (includes
  evidence-reachability, off-branch, and — with `--rules` — the exclusive-possession
  rule).
- `report-fork-tree`: your fork point is PLACED; every branch registered; every
  world-line reaches a terminal.
- `report-playthrough-manuscript --world W`: for EVERY branch, 0 unplaced / 0
  undecidable. (An "outside order" count is normal — the OTHER branches' scenes.)
- `report-payoff-coverage`: no setup left dangling in any TERMINAL world.
- `report-timeline-gaps`: no gap / unreached scene in any world.
- `report-disclosure-coverage --telling holm`: your withheld reveals are registered.

## Deliverables (leave these in `run/author/`)

- `sections.json`, `facts.json`, `order.json`, `narrative-rules.json`,
  `store.atomic.json` (final, gate-clean).
- `author-log.md` — the Phase-0 skeleton you laid (the cast of frames + the
  tentpoles + the load-bearing possessions + the disclosure plan), then how many
  write→gate→repair iterations you ran, what the gates flagged each pass, and what you
  changed. Note any time a gate caught a knowledge, possession, or backreference
  problem, and how you kept each road's aftermath peopled.

## Scope reminder

≈30–34 scenes; exactly ONE primary fork (REPORT / SHELTER / CONFRONT) at ~40–50%, into
3 distinct terminal world-lines each with its own real AFTERMATH and distinct ending;
**at least TEN distinct people, each with their own frame** (a real, individuated cast,
not two principals and a crowd) **kept present and knowing into the tail of every
road**; the load-bearing possessions TYPED with an exclusive rule; a SPARSE disclosure
plan withholding the load-bearing solution until each road's reveal; at least two real
setup→payoffs (at least one a piece of knowledge only one person held); several people
whose beliefs diverge from the truth and from each other, each acting on what they
believe and could have learned. Lay the skeleton top-down first, then fill. Tell the
best, most coherent shut-in coastal mystery you can — make every consequence trace to a
placed cause, make each person know exactly their own road and no more, keep the Holm
peopled to the last scene of every road, and leave nothing important dangling.
