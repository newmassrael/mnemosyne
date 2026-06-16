# Author brief (Stage 1) — author the dungeon-delve as a gate-checked fact base

You are a game-adventure author. You will be given a PREMISE (`premise.md`).
Your job is to invent a complete, coherent, branching DUNGEON-DELVE ADVENTURE —
a party of adventurers, a town and a flooded hold full of people, a set of
QUESTS that interlock, one moral fork into three distinct endings — and record
it as a **fact base** using the `mnemosyne-cli` tool, using the tool's
consistency gates to check your own work as you go and fixing whatever they flag.

You author the STRUCTURE OF THE ADVENTURE AS FACTS — not prose. Think of it as
writing the adventure's design bible: who is here, what is really true, what
each person knows, what each scene is, what the quests are and how they gate
each other, where on the map each quest is picked up, how the roads diverge,
how each ends. A later, separate stage will turn a slice of your fact base into
played scenes — so make each person's knowledge and each quest's shape CLEAR
and DISTINCT in the facts, but do not write prose here.

**Read ONLY this file and `premise.md`. Do not open any other file under
`claudedocs/phase1-dnd-quest-experiment/` — those are experiment internals and
reading them would bias your work.** Work in the directory `run/author/` (it
exists); leave your deliverables there.

## The three things that make this a GAME, not just a story

A real adventure has (1) a CAST who each know their own road, (2) QUESTS with
objectives, prerequisites, and outcomes, and (3) a MAP where quests are picked
up and resolved. Author all three as facts.

### (1) The cast as frames (point of view)

The substrate represents "what a particular person knows / believes" with a
FRAME: a named epistemic point of view. Use MANY frames — one ground-truth
frame (`gt`) plus **a distinct frame for each significant person** (the four
party members AND the town-and-hold NPCs: the reeve, the warden's shade, the
shrine-keeper, the lantern-boy, the rival, the wise-woman). Aim for **TEN-PLUS
person-frames**. A fact in a person's frame is part of THAT person's knowledge;
the same event can be known by some and unknown to others; two people can
believe contradictory things — each a true fact about a different mind. A
person should know only what they were present for or were told. Check with:

```
mnemosyne-cli report-frame-view --frame <person> --branch <world> --entity <who/what> --at <scene> --order order.json --sidecar store.atomic.json
```

### (2) The quests (THE HEART OF THIS ADVENTURE — author every quest this way)

A quest is a goal someone takes on: it has an OBJECTIVE, it is GIVEN somewhere,
it may REQUIRE something done first, and it is COMPLETED (or left undone). Record
each quest with the same handful of facts every time — this is the contract:

1. **The quest itself = an entity of kind `quest`.** Register it in `entities`:
   ```json
   {"entity_id":"q-main","kind":"quest","description":"Still the rising — reach the deep vault and resolve the crown"}
   ```
   Do this for EACH quest (the main quest and each side quest in the premise —
   author FOUR or so distinct quests, including the shrine-keeper's trap-errand).

2. **Who pursues it** — a TYPED fact saying which adventurer leads the quest:
   ```json
   {"fact_id":"f-pursue-key","frame":"gt","claim":"the rogue takes on recovering the warden's key","canon_from":"sc-12","evidence":["sc-12"],
    "typed":{"subject":"rogue","predicate":"pursues","object":{"kind":"entity","id":"q-key"}}}
   ```
   Register the `pursues` predicate once (`object_kind":"entity"`).

3. **Where it is GIVEN** — a fact at the giving scene that OPENS the obligation,
   marked `payoff_expectation:"expected"`, and pinned to a MAP LOCATION (see (3)).
   "Expected" means: this is a thread the adventure has promised to pay off.
   ```json
   {"fact_id":"f-give-key","frame":"gt","claim":"at the locked vault door the party learns only the warden's key will open it","canon_from":"sc-12","evidence":["sc-12"],"entities":["q-key","vault-door"],"payoff_expectation":"expected"}
   ```

4. **What it REQUIRES first** (for the quest that is gated by another) — a TYPED
   fact, AND a real ordering: the prerequisite must actually be finished EARLIER
   in the canon order than the thing it gates.
   ```json
   {"fact_id":"f-req-main","frame":"gt","claim":"the vault cannot be opened — and the crown cannot be reached — until the key is recovered","canon_from":"sc-13","evidence":["sc-13"],
    "typed":{"subject":"q-main","predicate":"requires","object":{"kind":"entity","id":"q-key"}}}
   ```
   Register the `requires` predicate (`object_kind":"entity"`). The premise has a
   clean prerequisite already: the KEY (a side quest) gates reaching the crown
   (the main quest). Make at least that one real — the key's recovery scene must
   come before the vault-opening scene in EVERY world-line where both happen.

5. **How it is COMPLETED** — in each world-line where the party finishes the
   quest, a fact that DISCHARGES the giving fact via `pays_off`, optionally typed
   `completed_by`:
   ```json
   {"fact_id":"f-done-key","frame":"gt","branch":"main","claim":"the rogue lifts the key from the drowned warden's cell","canon_from":"sc-20","evidence":["sc-20","sc-12"],"pays_off":["f-give-key"],
    "typed":{"subject":"q-key","predicate":"completed_by","object":{"kind":"entity","id":"rogue"}}}
   ```
   **Crucial — quests are per-road obligations, not global flags.** A quest the
   party CAN skip (the missing-delver errand) or a quest that is fulfilled on
   only ONE road (the shrine-keeper's trap-errand, completable only if the party
   CLAIMS the crown) should have its completion fact ONLY on the road(s) where it
   is finished. On the roads where it is NOT finished, leave the giving fact with
   no `pays_off` — the obligation stays OPEN on that road. Author it so that at
   least one quest is finished on some terminal road and left open on another;
   that divergence is what makes the three endings real games rather than the
   same game with different scenery.

### (3) The map (where quests live)

Each quest is PICKED UP at a place — the reeve's hall, the shrine, the
lantern-house, the locked vault door, the warden's cell. Record this as a
DISCLOSURE SURFACE on the quest's GIVING fact: the scene where it is offered and
the object/place it is offered at. Author ONE disclosure plan ("telling") over
the adventure, then mark each giving fact:

```
mnemosyne-cli add-disclosure-plan --telling delve --default-mode state --sidecar store.atomic.json
mnemosyne-cli set-disclosure --telling delve --fact f-give-main --mode state \
    --surface sc-03,reeve-hall --sidecar store.atomic.json
```

`--surface <scene>` — or `--surface <scene>,<object>` to name the place too — is
where on the map the quest is offered. Do this for EVERY quest's giving fact so
each quest has a place. (A surface object must be a registered entity — give the
named locations entities, e.g. `reeve-hall`, `vault-door`, `warden-cell`.)

The SECRETS are the other use of the disclosure plan. The buried truths — that
the shrine-keeper is the cause (the delver's journal), the warden's true nature
(the wizard's reading), the rival's betrayal — should land at a REVEAL, not be
told at the start. Mark each load-bearing secret fact `withhold` with a
`first-at` per world-line:

```
mnemosyne-cli set-disclosure --telling delve --fact f-secret-shrinekeeper --mode withhold \
    --first-at shatter=sc-25 --first-at claim=sc-26 --first-at parley=sc-27 --sidecar store.atomic.json
```

Withheld secret facts must be TYPED (the gate matches on the typed claim). Keep
the plan SPARSE — surfaces on the quest-givers, withholds on the few real
secrets. Confirm with `report-disclosure-coverage --telling delve`.

## The fork, the roads, and keeping it peopled

ONE primary fork at roughly the middle (~40–50%), at the deep vault: SHATTER /
CLAIM / PARLEY, into three distinct terminal world-lines each with its own real
AFTERMATH and ending. Name the branches and the single scene where they fork.
Reserve scene-id ranges (a shared spine `sc-01..sc-NN`, then each road's own
range for its aftermath).

**Keep the Reach and the hold peopled to the last scene of EVERY road.** The
recurring failure: the opening is full of people and the aftermath shrinks to
two principals while everyone else vanishes. On each road's tail, the secondary
people (the lantern-boy, the wise-woman, the rival, the warden, the reeve)
should still be PRESENT, KNOWING, and ACTING — each carrying their fragment into
how the road closes. Give several of them a fact in the post-fork tail of each
world-line, not only on the spine.

## Type the load-bearing POSSESSION, and declare the rule

The crown and the key can be held by only ONE party at a time, and who holds
them is a pivot. For each: give it an entity, a `possession` predicate, and TYPE
the facts that move it
(`typed:{"subject":"<holder>","predicate":"possession","object":{"kind":"entity","id":"crown"}}`)
at each scene it changes hands. Declare an EXCLUSIVE rule in
`narrative-rules.json` so the gate enforces one-holder-at-a-time:

```json
{ "rules": [ { "kind": "exclusive", "predicate": "possession", "per": "object" } ] }
```

A succeeding holder SUPERSEDES the prior one in that frame at the transfer scene
(`supersedes_in_frame`), so the chain of who-held-the-crown reads cleanly and
the gate catches an impossible co-holding.

## Seed every NEW precondition a reveal needs (prevents a silent hole)

When a reveal introduces a NEW person, object, or STATE the earlier story never
established, that new thing is a question the adventure must already have opened.
Open it as a SETUP earlier (`payoff_expectation:"expected"`) that the reveal
`pays_off`. Corollary: the ground-truth frame must not stay SILENT on a question
a character's belief turns on — if someone believes X about the rising, the
ground truth must say X or not-X, so the belief has a truth to diverge from.

## Structural backreferences (REQUIRED)

When a later scene REFERS BACK to an earlier event (a callback — "the same mark
the rogue saw on the cell door", "the rune the wizard copied"), the
backreference MUST be STRUCTURAL: cite the establishing scene in the fact's
`evidence` array, not just in prose. A cited scene must be reachable AT OR
BEFORE this fact in this fact's OWN world-line — you cannot cite a scene that
happens only on a DIFFERENT branch, or a later scene. If two roads must share an
earlier event, it belongs on the SHARED SPINE before the fork. (A gate enforces
this — `evidence_unreachable`.)

## The method is TOP-DOWN

Do NOT free-associate scene by scene. Lay the skeleton first: scope + scenes,
the cast of frames, the ground truth + the three endings, the fork, the quests
(entities + givings + the key→vault prerequisite + each road's completions), the
load-bearing possessions, the disclosure plan (surfaces + withheld secrets).
Get THAT gate-clean, then fill the connecting detail and keep every road peopled.

## What to produce (in `run/author/`)

1. **The empty seed store** — create `store.atomic.json` with exactly:
   ```json
   {"sections":{},"changelog_entries":{},"inventory_entries":{},"confirmation_events":{},"frames":{},"branches":{},"entities":{},"predicates":{},"narrative_facts":{},"disclosure_plans":{},"schema_version":23}
   ```
2. **`sections.json`** — your SCENES (each scene is a section; a canon coordinate
   used by any fact MUST exist here first). Load:
   `mnemosyne-cli import-sections --manifest sections.json --sidecar store.atomic.json`
3. **`facts.json`** — frames, branches, entities (incl. the `quest` entities and
   the named map-location entities), predicates (`pursues`, `requires`,
   `completed_by`, `possession`), and facts. Load:
   `mnemosyne-cli import-facts --manifest facts.json --sidecar store.atomic.json`
   (ONE atomic transaction; if anything is invalid NOTHING is written — fix and
   re-run. To CHANGE a row, edit facts.json and rebuild from the empty seed.)
   Field rules:
   - `canon_from`, `section_id`, `forks_at`, every `evidence[]` id = SCENE IDS in
     `sections.json`.
   - `branch`: OMIT on the shared spine before the fork (root `main`); after the
     fork tag each fact with its road.
   - `frame`: `gt` for ground truth; a person's knowledge in THAT person's frame.
     Never mark a belief fact and a truth fact as `conflicts` — they are two true
     facts on two frames.
   - `payoff_expectation`: only `"expected"` or omit. `pays_off`: an ARRAY of the
     setup fact-ids a fact discharges (the payer must be reachable after the setup
     in its world-line).
   - `typed`: use for quests (pursues/requires/completed_by), the possession
     pivots, and the withheld secrets.
4. **`order.json`** — the canon order: the main spine as a chain of edges, each
   branch its own edge chain starting at the fork scene. Pass `--order order.json`
   to EVERY gate.
5. **`narrative-rules.json`** — the exclusive rule(s). Pass with `--rules`.

## The gates (run all after each import, with --order and --rules)

```
mnemosyne-cli validate-continuity            --order order.json --rules narrative-rules.json --sidecar store.atomic.json
mnemosyne-cli report-fork-tree               --order order.json --sidecar store.atomic.json
mnemosyne-cli report-timeline-gaps --world <each-road> --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-coverage         --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-substantiation   --order order.json --sidecar store.atomic.json
mnemosyne-cli report-disclosure-coverage --telling delve --sidecar store.atomic.json
mnemosyne-cli report-playthrough-manuscript --world <each-road> --telling delve --order order.json --sidecar store.atomic.json
mnemosyne-cli report-playable-world --telling delve --order order.json --sidecar store.atomic.json
```

Read them and FIX until:
- `validate-continuity`: 0 structural + 0 interval violations (incl.
  evidence-reachability, off-branch, the exclusive-possession rule).
- `report-fork-tree`: the fork is PLACED; every road registered; every world-line
  reaches a terminal.
- `report-playthrough-manuscript --world W`: every road, 0 unplaced / 0
  undecidable. (An "outside order" count is normal — the other roads' scenes.)
- `report-payoff-coverage`: every quest-giving setup is either paid off on a road
  or LEFT OPEN BY DESIGN on a road — and you know which is which (a skippable or
  trap quest left open on the roads it is not finished is INTENDED, not a bug;
  the gate lists them so you can confirm each open thread is one you meant).
- `report-timeline-gaps`: no gap / unreached scene in any world.
- `report-disclosure-coverage --telling delve`: every quest-giving fact has a
  surface; every withheld secret is registered.
- `report-playable-world --telling delve`: it runs clean and each quest-giving
  surface resolves to a place on a road's walk (this is the map the later stage
  reads).

## Deliverables (leave in `run/author/`)

- `sections.json`, `facts.json`, `order.json`, `narrative-rules.json`,
  `store.atomic.json` (final, gate-clean).
- `author-log.md` — the skeleton you laid (the cast of frames; the three
  endings; the fork; the quests = each quest's entity + giver + prerequisite +
  per-road completion/open status + map place; the load-bearing possessions; the
  disclosure plan), then how many write→gate→repair iterations you ran, what each
  gate flagged, and what you changed. Note any time a gate caught a knowledge,
  possession, ordering, or backreference problem, and how you kept each road
  peopled.

## Scope

≈45–55 scenes; exactly ONE primary fork (SHATTER / CLAIM / PARLEY) at ~40–50%
into 3 distinct terminal world-lines, each with a real aftermath and distinct
ending; **at least TEN distinct people, each with their own frame**, kept
present and knowing into every road's tail; **FOUR or so interlocking quests**
(a main quest, a prerequisite side quest that GATES it, a skippable side quest,
and the shrine-keeper's trap-errand completable on only one road), each authored
with the entity + pursues + giving(expected) + (where finished) completion +
map-surface contract above; the crown and key TYPED with an exclusive rule; a
SPARSE disclosure plan (surfaces on every quest-giver, withholds on the few real
secrets). Lay the skeleton top-down first, then fill. Tell the best, most
coherent dungeon-delve you can — make every consequence trace to a placed cause,
make each person know exactly their own road, keep the Reach peopled to the last
scene, make the quests interlock, and leave nothing OPEN that you did not mean to
leave open.
