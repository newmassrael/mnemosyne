# Author brief (Stage 1) — author the two-age game as a gate-checked fact base

You are a game-adventure author. You will be given a PREMISE (`premise.md`):
**one place that exists in two ages — a Founding Age and a Withering Age — where
things people do in the early age change the late age.** Your job is to invent the
complete, coherent game and record it as a **fact base** using the `mnemosyne-cli`
tool, using the tool's consistency gates to check your own work as you go and
fixing whatever they flag.

You author the STRUCTURE OF THE GAME AS FACTS — not prose. Think of it as the
design bible: the two ages of each place, who lives in each age and what they
know, every scene, the early acts and the late consequences they cause (including
acts in one corner of the hollow that land on a different person in a different
corner, generations later), the one optional act that splits the late age, and
the cause the late age cannot see until the traveller crosses. A later, separate
stage turns a slice of your fact base into played scenes — so make the two ages,
each person's knowledge, and each cause→effect link CLEAR and DISTINCT in the
facts, but do not write prose here.

**Read ONLY this file and `premise.md`. Do not open any other file under
`claudedocs/phase1-time-travel-experiment/` — those are experiment internals and
reading them would bias your work.** Work in the directory `run/author/` (it
exists); leave your deliverables there.

## The four things that make this a TIME-TRAVEL game

### (1) Two ages of one place, ordered early-before-late

The place is the SAME in both ages — but older in the late age. Author the
**Founding-Age scenes FIRST in the canon order, then the Withering-Age scenes
AFTER them** — the early age is literally earlier in time than the late age, and
the canon order must say so (every Founding-Age scene of the main timeline comes
strictly BEFORE every Withering-Age scene of it). The two ages are NOT two
opinions of one moment and NOT two unrelated stories — they are the same ground,
early and late, on one continuous timeline.

Each place (the Cistern Steps, the Loom Quarter, the Wend Fields) gets an
**entity**, and that ONE entity is referenced by Founding-Age facts AND
Withering-Age facts — the same cistern, three generations apart. Give the
load-bearing PROP of each place an entity too (the cistern's lower vent, Halden's
boundary-stone, the gorge crossing-tree): the traveller touches the same prop in
both ages, so the same prop-entity appears in a Founding scene and a Withering
scene.

### (2) Early acts that CAUSE late consequences (make the link real to the tool)

This is the heart. An early-age act and its late-age consequence must be a REAL
LINK the tool can see — not just two facts that happen to rhyme. Encode each
cause→effect chain like this:

- the **early act** is a fact in the Founding Age marked `payoff_expectation:"expected"`
  (a thread the game promises to pay off later in time);
- the **late consequence** is a fact in the Withering Age that DISCHARGES it via
  `pays_off` (the consequence pays off the act, across the generations between
  them).

```json
{"fact_id":"f-seal-vent","frame":"gt","claim":"Sera packs the cistern's lower vent against the creeping rot","canon_from":"sc-04","evidence":["sc-04"],"entities":["cistern","cistern-vent"],"payoff_expectation":"expected"}
{"fact_id":"f-clean-water","frame":"gt","claim":"three generations on the cistern still runs clean — the last good water in the valley","canon_from":"sc-31","evidence":["sc-31","sc-04"],"entities":["cistern","cistern-vent"],"pays_off":["f-seal-vent"]}
```

Author SEVERAL such chains. The premise hands you the cistern chain (same place,
across the ages) and — the important one — a chain that crosses the hollow:
**Halden, in the Loom Quarter, in the Founding Age, sets the boundary-stone that
marks the firm ford; Ode, in the Wend Fields, in the Withering Age, finds it and
the people cross safely.** Different corner, different person, generations apart —
ONE act, ONE consequence. Author that as an early `expected` fact (Halden's, in
the Loom Quarter) paid off by a late fact (Ode's, in the Wend Fields). Author the
early act and the late consequence in the DIFFERENT PEOPLE'S frames (Halden's act
in Halden's frame; Ode's discovery in Ode's frame) — a consequence can land on
someone who never knew the cause.

Where an early act changes the **state** of a thing that persists into the late
age (the cistern goes from rotting to sealed-and-clean; the ford goes from unmarked
to marked), TYPE the state and let the late state SUPERSEDE the early one on the
same object (see (4)) — so the tool carries the state forward through time.

### (3) The cast as frames (point of view), across both ages

The substrate represents "what a particular person knows / believes" with a
FRAME: a named epistemic point of view. Use one ground-truth frame (`gt`) plus **a
distinct frame for each significant person** across BOTH ages — the Founding-Age
people (Sera the well-keeper, Halden the dyer-mason, the bearer) and the
Withering-Age people (Ode the field-warden, and the late-age folk who live with
the consequences). Aim for **SIX-PLUS person-frames**. A person knows only their
own age and their own corner: Ode cannot know WHY the cistern runs clean — only
the bearer, who crosses, sees both causes. Check a person's knowledge with:

```
mnemosyne-cli report-frame-view --frame <person> --branch <world> --entity <who/what> --at <scene> --order order.json --sidecar store.atomic.json
```

### (4) The one optional act that splits the late age

The premise has exactly ONE choice (the rest of the timeline is canonical): at the
Wend crossing in the Founding Age the bearer **plants a gorge-sapling, or does
not**. Make this the single FORK: name a branch for each outcome
(`planted` / `barren`), fork at the planting scene, and give each its own
Withering-Age future. In the `planted` future a living bridge-tree spans the gorge
and the far field is reachable (a late fact pays off the planting); in the
`barren` future the gorge is impassable and the far field is lost (the planting
thread stays OPEN — never paid off — on that road, BY DESIGN). Author the planting
as an `expected` fact paid off only on the `planted` road.

## The cause the late age cannot see (a reveal earned by travelling)

In the Withering Age nobody knows WHY the cistern still runs or WHO marked the
ford — those are Founding-Age secrets. The bearer (and the reader) learns a cause
only by crossing to the Founding Age to witness it. Author ONE disclosure plan
("telling") and mark each load-bearing Founding-Age CAUSE `withhold` with a
`first-at` per world-line = the scene where the bearer crosses and witnesses it:

```
mnemosyne-cli add-disclosure-plan --telling wend --default-mode state --sidecar store.atomic.json
mnemosyne-cli set-disclosure --telling wend --fact f-seal-vent --mode withhold \
    --first-at planted=sc-22 --first-at barren=sc-22 --sidecar store.atomic.json
```

Withheld cause facts must be TYPED (the gate matches on the typed claim). Also put
a **surface** on each place's load-bearing prop so the map knows where it lives —
`--surface <scene>,<object>` names the scene and the prop-entity it surfaces at.
Surface the same prop in BOTH ages (the vent at its Founding scene and at its
Withering scene) so the place is locatable in each age:

```
mnemosyne-cli set-disclosure --telling wend --fact f-clean-water --mode state \
    --surface sc-31,cistern-vent --sidecar store.atomic.json
```

Keep the plan SPARSE — surfaces on the load-bearing props, withholds on the few
real Founding-Age causes. Confirm with `report-disclosure-coverage --telling wend`.

## Type the load-bearing STATE, and declare the rule

The persisting things — the cistern (rotting → sealed-clean), the ford (unmarked →
marked-firm) — can hold only ONE state at a time, and the state the bearer finds in
the late age is the state the early act left. For each: give it an entity, a
`condition` predicate (or `possession` for the prop), and TYPE the facts that set
its state in each age
(`typed:{"subject":"cistern","predicate":"condition","object":{"kind":"literal","value":"sealed-clean"}}`).
Declare an EXCLUSIVE rule in `narrative-rules.json` so the gate enforces
one-state-at-a-time:

```json
{ "rules": [ { "kind": "exclusive", "predicate": "condition", "per": "subject" } ] }
```

The late-age state SUPERSEDES the early-age state on that object in that frame at
the late scene (`supersedes_in_frame`), so the chain of the cistern's state reads
cleanly across time and the gate catches an impossible two-states-at-once.

## Seed every NEW precondition a late consequence needs (prevents a silent hole)

When a Withering-Age fact depends on a Founding-Age act, that act must already be
ON THE PAGE as the earlier `expected` setup it pays off — never a late consequence
whose cause the early age never showed. Corollary: the ground-truth frame must not
stay SILENT on a cause a character's belief turns on.

## Structural backreferences (REQUIRED)

When a Withering-Age scene REFERS BACK to a Founding-Age event (the bearer
recognises "the same vent Sera packed"), the backreference MUST be STRUCTURAL: cite
the Founding scene in the fact's `evidence` array, not just in prose. A cited scene
must be reachable AT OR BEFORE this fact in this fact's OWN world-line — across the
ages is fine (the Founding scene is earlier on the same timeline), but you cannot
cite a scene that happens only on a DIFFERENT branch, or a later scene. Shared
early events belong on the SHARED SPINE before the planting fork. (A gate enforces
this — `evidence_unreachable`.)

## The method is TOP-DOWN

Do NOT free-associate scene by scene. Lay the skeleton first: the two ages and
their scene bands (Founding scenes, then Withering scenes); the three places as
entities present in both ages; the cast of frames across both ages; the
cause→effect chains (each early `expected` act + its late `pays_off` consequence,
including the cross-corner Halden→Ode chain); the persisting states + the exclusive
rule; the one planting fork into two Withering futures; the disclosure plan
(prop-surfaces in both ages + the withheld Founding causes). Get THAT gate-clean,
then fill the connecting detail.

## What to produce (in `run/author/`)

1. **The empty seed store** — create `store.atomic.json` with exactly:
   ```json
   {"sections":{},"changelog_entries":{},"inventory_entries":{},"confirmation_events":{},"frames":{},"branches":{},"entities":{},"predicates":{},"narrative_facts":{},"disclosure_plans":{},"schema_version":23}
   ```
2. **`sections.json`** — your SCENES (each scene is a section; a canon coordinate
   used by any fact MUST exist here first). Order them so the Founding-Age scenes
   come before the Withering-Age scenes. Load:
   `mnemosyne-cli import-sections --manifest sections.json --sidecar store.atomic.json`
3. **`facts.json`** — frames, branches, entities (the place-entities, the
   prop-entities, the state-entities), predicates (`condition`/`possession` plus
   any `pursues`/`completed_by` if you model the cross-age objective as a goal),
   and facts. Load:
   `mnemosyne-cli import-facts --manifest facts.json --sidecar store.atomic.json`
   (ONE atomic transaction; if anything is invalid NOTHING is written — fix and
   re-run. To CHANGE a row, edit facts.json and rebuild from the empty seed.)
   Field rules:
   - `canon_from`, `section_id`, `forks_at`, every `evidence[]` id = SCENE IDS in
     `sections.json`.
   - `branch`: OMIT on the shared spine before the planting fork (root `main`);
     after the fork tag each fact with its road (`planted` / `barren`).
   - `frame`: `gt` for ground truth; a person's knowledge in THAT person's frame.
     Author the cross-corner act and its consequence in the two DIFFERENT people's
     frames. Never mark a belief fact and a truth fact as `conflicts` — they are
     two true facts on two frames.
   - `payoff_expectation`: only `"expected"` or omit. `pays_off`: an ARRAY of the
     setup fact-ids a fact discharges (the payer must be reachable after the setup
     in its world-line — a late-age fact paying off an early-age act is exactly
     this, across the ages).
   - `typed`: use for the persisting states, and for any withheld Founding cause.
4. **`order.json`** — the canon order: the main spine as a chain of edges
   (Founding scenes → Withering scenes), the planting fork's two branches each
   their own edge chain from the fork scene. Pass `--order order.json` to EVERY
   gate.
5. **`narrative-rules.json`** — the exclusive rule(s). Pass with `--rules`.

## The gates (run all after each import, with --order and --rules)

```
mnemosyne-cli validate-continuity            --order order.json --rules narrative-rules.json --sidecar store.atomic.json
mnemosyne-cli report-fork-tree               --order order.json --sidecar store.atomic.json
mnemosyne-cli report-timeline-gaps --world <each-road> --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-coverage         --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-substantiation   --order order.json --sidecar store.atomic.json
mnemosyne-cli report-disclosure-coverage --telling wend --sidecar store.atomic.json
mnemosyne-cli report-playthrough-manuscript --world <each-road> --telling wend --order order.json --sidecar store.atomic.json
mnemosyne-cli report-playable-world --telling wend --order order.json --sidecar store.atomic.json
```

Read them and FIX until:
- `validate-continuity`: 0 structural + 0 interval violations (incl.
  evidence-reachability, off-branch, the exclusive state rule, succession).
- `report-fork-tree`: the planting fork is PLACED; both futures registered; every
  world-line reaches a terminal.
- `report-playthrough-manuscript --world W`: every road, 0 unplaced / 0
  undecidable. (An "outside order" count is normal — the other road's scenes.)
- `report-payoff-coverage`: every early `expected` act is either paid off by a
  late fact on a road, or LEFT OPEN BY DESIGN on a road (the un-planted future's
  lost far-field is INTENDED open, not a bug — the gate lists them so you can
  confirm each open thread is one you meant).
- `report-timeline-gaps`: no gap / unreached scene in any world.
- `report-disclosure-coverage --telling wend`: every load-bearing prop has a
  surface in each age; every withheld Founding cause is registered.
- `report-playable-world --telling wend`: it runs clean and each prop surface
  resolves to a place on a road's walk in each age (this is the map the later
  stage reads).

## Deliverables (leave in `run/author/`)

- `sections.json`, `facts.json`, `order.json`, `narrative-rules.json`,
  `store.atomic.json` (final, gate-clean).
- `author-log.md` — the skeleton you laid (the two ages + their scene bands; the
  three places as entities in both ages; the cast of frames across both ages; each
  cause→effect chain = the early `expected` act + the late `pays_off` consequence,
  naming the cross-corner Halden→Ode chain; the persisting states; the planting
  fork's two futures; the disclosure plan), then how many write→gate→repair
  iterations you ran, what each gate flagged, and what you changed. Note any time a
  gate caught a knowledge, state, ordering, or backreference problem, and how you
  made each early act's consequence trace structurally to its late fact.

## Scope

≈45–55 scenes split across the two ages (a Founding-Age band, then a
Withering-Age band); exactly ONE fork (the planting choice, `planted` / `barren`)
near the end of the Founding Age into 2 distinct Withering futures, each with a
real aftermath; **at least SIX distinct people, each with their own frame**, across
both ages; **several early-act → late-consequence chains**, including the
cross-corner cross-person Halden(Loom Quarter)→Ode(Wend Fields) chain and the
same-place cistern chain, each encoded as an early `expected` act paid off by a
late fact; the persisting cistern/ford STATES typed with an exclusive rule; a
SPARSE disclosure plan (prop-surfaces in both ages, withholds on the few real
Founding causes). Lay the skeleton top-down first, then fill. Tell the best, most
coherent two-age story you can — make every late consequence trace to a placed
early cause, make each person know only their own age and corner, make the same
places live in both ages, and leave nothing OPEN that you did not mean to leave
open.
