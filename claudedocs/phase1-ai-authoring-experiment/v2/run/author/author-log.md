# Author log — "Cold Signal at Vantor Pass"

A record of the top-down authoring loop: the Phase-0 skeleton (tentpoles laid
first), then the Phase-1 detail fill, with every write -> gate -> repair
iteration the gates drove.

## The story bible (ground truth + belief frames)

**Ground truth (held whole by no one):** The night of the slide, the valley
company agent **Sefton** wired a standing pressure to reopen the bore on
schedule. The wire went down for ice. Day-operator **Pike** gave Stationmaster
**Halloran** a *verbal* relay that clearance had come. Halloran, under company
pressure and trusting the relay, authorized the blast and — out of a years-old
habit of keeping the company's paperwork clean — **back-dated the log entry to
the valley-office timestamp** so the order would read as properly authorized.
He had paper-cleaned verbal authorizations for years and it had never harmed a
soul. But foreman **Brrecord**'s warning to HOLD the blast until the cut was
clear *did* come up the wire, in the brief window the ice let it flicker back.
**Pike took that warning and never logged it** — because logging it would have
shown the blast was fired against a standing hold. Then the snow closed and Pike
rode the last train down, out of reach.

So the truth is split: Halloran forged the timestamp (a habitual cover of a
verbal order, not murder) and does NOT know a warning ever arrived; Pike
suppressed the warning and is gone. Neither man holds the whole truth.

**Two false accounts (belief frames):**
- `company-belief`: the blast was properly authorized from the valley; the slide
  was an act of God; no warning was ever sent.
- `town-belief`: Halloran deliberately blasted on the gang to reopen the line for
  the company's schedule — murder.
Both are false. The truth (mistake + habitual cover-up, split across two men) is
held by neither frame. Wren reasons toward it from the record.

## Scope + world-lines (Phase 0, step 1)

70 scenes. One primary fork + two downstream forks-off-forks. 5 terminal
world-lines.

```
SHARED SPINE  main             sc-01..sc-20   (20)  -> primary fork at sc-20
BRANCH expose  (from main@20)  sc-21..sc-26   (6)   -> downstream fork at sc-26
  TERMINAL expose-recant (from expose@26) sc-27..sc-32 (6)
  TERMINAL expose-deny   (from expose@26) sc-33..sc-38 (6)
BRANCH hold    (from main@20)  sc-39..sc-50   (12)  TERMINAL
BRANCH act     (from main@20)  sc-51..sc-58   (8)   -> downstream fork at sc-58
  TERMINAL act-dig  (from act@58) sc-59..sc-64 (6)
  TERMINAL act-flee (from act@58) sc-65..sc-70 (6)
```

Forks: primary `sc-20` (expose|hold|act); fork-off-a-fork `sc-26`
(expose-recant|expose-deny); fork-off-a-fork `sc-58` (act-dig|act-flee).
Terminals: expose-recant, expose-deny, hold, act-dig, act-flee = 5.

## Phase 0 — skeleton laid (tentpoles first)

I placed, in one pass, BEFORE any connecting detail:

- **Six load-bearing setups** on the shared spine (all `payoff_expectation:expected`,
  all typed so they read as substantiated):
  - `f-001` the forged timestamp (sc-03) — the central evidence.
  - `f-002` the missing warning (sc-09) — the second key evidence.
  - `f-003` Halloran's years-old back-dating habit (sc-07) — the reveal that
    recasts forgery as mistake, not murder. LONG-RANGE.
  - `f-004` the brass check-tag (sc-12) — physical proof from the face.
  - `f-005` Pike off down the valley (sc-15) — why the truth can't be completed
    on its own. LONG-RANGE.
  - `f-006` the dawn relief train + inspector (sc-18) — the deadline.
- **The buried ground truth** Wren reasons toward (`f-007`..`f-010`): Sefton's
  pressure, Pike's verbal relay, Halloran's back-dated authorization, and the
  fact the warning DID arrive and Pike suppressed it.
- **The two false belief frames** (`f-011` company-belief: authorized/act-of-God;
  `f-012` town-belief: murder) — the two characters acting on a false account.
- **Every terminal's ENDING**, authored first, as typed `fate`/`disposition`
  facts: expose-recant (`f-030`,`f-031`), expose-deny (`f-040`,`f-041`),
  hold (`f-050`,`f-051`), act-dig (`f-070`,`f-071`), act-flee (`f-080`,`f-081`),
  plus the two pre-fork branch identities (`f-020` expose, `f-060` act).

### Phase-0 gate pass (iteration 0)

- `validate-continuity`: **0 structural, 0 interval.** Clean from the first
  import — the canon order placed all 24 skeleton facts, evidence backrefs all
  resolved (every backref cites the spine or the fact's own parent branch).
- `report-fork-tree`: **7 world-lines registered, 0 unplaced fork points.**
  Primary fork sc-20 (expose|hold|act); fork-off-fork sc-26
  (expose-recant|expose-deny); fork-off-fork sc-58 (act-dig|act-flee).
- `report-payoff-coverage`: as expected for a bare skeleton, the six spine
  setups still dangle in most terminals — that is the Phase-1 work list. One
  diagnostic to fix: `[payoff->unmarked] f-041 -> f-012` (expose-deny) — I had
  `f-041` claim it `pays_off f-012`, but `f-012` is a belief fact I did not mark
  `payoff_expectation:expected`. Decision: belief frames are not setups; drop
  `f-012` from f-041's `pays_off` in Phase 1.

A clean continuity + fork spine = the trustworthy backbone. Phase 1 fills detail.

## Phase 1 — detail fill (write -> gate -> repair loop)

I filled the connecting beats between the tentpoles, the belief-frame facts, and
— the main driver — the per-terminal payoff facts that discharge every reachable
setup in every terminal world. The gates drove the following repair iterations.

**Iteration 1 (first detail batch — 76 facts).**
- `validate-continuity`: 0 / 0. Clean.
- `report-payoff-coverage`: terminals still dangling: expose-recant `f-004`;
  expose-deny `f-004`; hold `f-001`; act-dig `f-003`,`f-005`; act-flee
  `f-002`,`f-003`,`f-005`. Each is a spine setup with no payoff yet reachable in
  that terminal. FIX: authored one payoff fact per (setup, terminal) gap — e.g.
  the check-tag returned to Margaret in the expose lines, the timestamp left
  standing in hold, the back-dating made moot by the dug-out cut in act-dig, the
  warning staying lost / Pike staying out of reach in act-flee. A negative
  resolution ("the warning stays lost", "Pike beyond reach") is a real payoff:
  the setup returns and matters.

**Iteration 2 (payoff gaps closed — 81 facts).**
- `report-payoff-coverage`: ALL FIVE TERMINALS CLEAN (0 dangling, 0
  payoff-before-setup, 0 payoffs-to-unmarked).
- `report-payoff-substantiation`: lots of UNSUBSTANTIATED / unverifiable. The
  rule (learned from the gate, not the brief): a payoff *substantiates* a setup
  only when its typed leg carries the SAME subject+predicate as the setup, with a
  transitioned value. My multi-setup payoffs each had one typed leg, so they
  substantiated at most one of their setups, and several payoffs used a different
  predicate (`fate`/`location`) than the setup's (`record-standing`/`belief`).

**Iteration 3 (substantiation alignment — 83 facts).** Structural fix, not a
patch: I split every multi-setup payoff so each setup gets its OWN dedicated
substantiating payoff fact carrying the matching subject+predicate typed leg
(e.g. `pike/location` for `f-005`, `warning/record-standing` for `f-002`,
`halloran/belief` for `f-003`, `check-tag/record-standing` for `f-004`,
`timestamp/record-standing` for `f-001`), and typed the deadline setup `f-006`
(`inspector/location`) so it became verifiable. Two facts then failed import:
`f-031` and `f-060` had a typed `subject` not in their `entities` list (the gate:
"the entities list stays THE retrieval key"). FIX: added the subject to each
entities list. Re-import clean.
- `report-payoff-substantiation`: **6/6 substantiated in EVERY terminal**, 0
  unsubstantiated, 0 unverifiable.

**Iteration 4 (empty-scene closure — 85 facts).** `report-playthrough-manuscript`
per world reported unplaced=0 / undecidable=0 everywhere, but a walk audit found
two scenes reached by the walk with NO fact beginning in them — `sc-56` (the
"no word to the valley" beat on the `act` spine, reachable in act-dig+act-flee)
and `sc-60` (the digging beat in act-dig). Those are story holes. FIX: authored
`f-256` (sc-56) and `f-261` (sc-60). Re-audit: 0 empty scenes in any world.

## Evidence-reachability / off-branch backreference

`validate-continuity` includes the evidence-reachability check. Across the whole
authoring loop it **never flagged an off-branch or unreachable backreference in
my real store** — every callback I wrote cites either the fact's own scene, an
ancestor branch's scene, or the shared spine, so all 66 backref-bearing facts
(43 of them long-range, branch facts citing the planted spine setups in
sc-01..sc-20) resolve at-or-before the citing fact in its own world-line. I
designed for this from Phase 0 by placing every shared setup on the spine before
the first fork, so every branch can reach it.

To confirm the clean pass was not vacuous, I ran a deliberate negative control on
a throwaway store: an `expose-recant` fact citing `sc-33` (a scene that exists
only on the sibling `expose-deny` branch). The gate produced exactly
`{"kind":"evidence_unreachable","fact":"f-probe-off","branch":"expose-recant",
"evidence":"sc-33"}` and the violation count rose to 1. So the gate genuinely
enforces the rule; my real store's 0 violations is meaningful. (The throwaway
store was discarded; the real store is untouched.)

## Final gate state (all clean)

Re-ran every gate one final time over the 85-fact store:
- `validate-continuity`: violations 0 (structural 0, interval 0). exit 0.
- `report-fork-tree`: 7 world-lines registered, 0 unplaced fork points; primary
  fork sc-20, forks-off-forks sc-26 and sc-58; every world-line reaches a
  terminal. exit 0.
- `report-payoff-coverage`: 0 dangling in all 5 terminal worlds. exit 0.
- `report-payoff-substantiation`: 6/6 substantiated in all 5 terminals. exit 0.
- `report-timeline-gaps --world W`: 0 violated / 0 unverifiable for every world.
  exit 0.
- `report-playthrough-manuscript --world W`: every terminal unplaced=0,
  undecidable=0, undeclared-adjacencies=0; 0 empty scenes in any world's walk;
  the "outside order" counts are the other branches' scenes correctly excluded.
  exit 0.

## Final tallies

- 70 scenes; 85 facts; 7 branches (3 non-terminal spine segments main/expose/act,
  5 terminal world-lines); 3 frames (gt + 2 belief frames); 5 predicates;
  15 entities.
- 3 fork points (1 primary + 2 forks-off-a-fork); 5 terminal world-lines.
- 6 long-range setups, all planted on the shared spine, every one paying off
  (and substantiated) across the fork in each descendant terminal.
- 4 write -> gate -> repair iterations in Phase 1 (after the clean Phase-0
  skeleton).

