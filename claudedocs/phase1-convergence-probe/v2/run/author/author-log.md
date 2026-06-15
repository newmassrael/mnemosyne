# Author log — The Reckoning at Harlow Mill (branching-and-converging fact base)

Tool: `/home/coin/mnemosyne/target/release/mnemosyne-cli`. All `--sidecar` and
`--order` paths passed absolute. Work dir: this directory.

## Shape decided up front

- **17 scenes** (within the 16–20 scope): shared spine `sc-01..sc-07`, then ONE
  exclusive fork at `sc-07`, then each path's own 4-scene middle, then a 2-scene
  shared dawn.
- **3 world-lines**: two FORK branches off `main` at `sc-07` — `sluice`
  (opens the sluice; dam holds; lower row drowns) and `ride` (holds + rides;
  dam overtops; mill wrecked) — and ONE CONFLUENCE branch `dawn` that
  `converges_from` both (`sluice@sc-11a`, `ride@sc-11b`) and carries the shared
  reckoning + river's edge.
- Scene-id ranges reserved before any fact: spine `sc-01..07`, sluice middle
  `sc-08a..sc-11a`, ride middle `sc-08b..sc-11b`, shared ending `sc-rk`,`sc-rv`.

## Phase 0 — the tentpoles laid first (before any connecting detail)

I authored the load-bearing skeleton in one pass, late facts before early scenes:

1. **Destination first (the shared ending, on `dawn`):**
   - `f-300` the mill-yard reckoning convenes (Garrick must speak keeper-or-criminal).
   - `f-301` the wedged-gate truth surfaces and turns the verdict.
   - `f-302` Garrick's verdict: keeper, not criminal.
   - `f-310` the river's-edge final image.
   Authored ONCE on the confluence — never duplicated onto each path.
2. **The fork point:** `f-030` at `sc-07` — the one exclusive choice she cannot unmake.
3. **Each fork's world-line identity:** `f-100`/`f-103` (sluice: open → lower row
   drowns) and `f-200`/`f-203` (ride: hold+ride → mill wrecked). Genuinely
   different dead, different ruin.
4. **Load-bearing entities/objects:** sela, garrick, father, lower-row,
   relief-gate, sluice, dam, mill — declaring them here IS saying "these matter."
5. **Reveals + setups + preconditions together:**
   - The wedged-gate reveal (`f-301`, late) was authored alongside its setups
     `f-001` (Sela finds it wedged, `sc-03`) AND its precondition `f-002`
     (it was wedged *on purpose* for the wheel, `sc-04`) — the new state the
     reveal answers, seeded as its own earlier `expected` setup. `f-301`
     `pays_off` both.
   - The verdict reveal (`f-302`) was paired with its setup `f-022` (Sela stands
     *accused* until Garrick speaks, `sc-06`).
   - The false belief `f-011` (town holds the father still keeper) seeded against
     the ground truth `f-010` (Sela is keeper in fact), broken at dawn by `f-303`.

I gated this skeleton clean (continuity 0/0, fork-tree diamond placed) before
filling.

## Phase 1 — detail fill: write → gate → repair iterations

The fixes below are the literal gate flags I hit and what I changed. Five
write→gate→repair iterations.

**Iteration 1 — first `import-facts`.** FLAG: `typed object 'sluice' is not a
member of the fact's entities list` (f-011). The typed-entity-object rule
requires the object entity to also appear in `entities[]` (it stays the
retrieval key). FIX: added `sluice` to f-011's entities.

**Iteration 2 — skeleton gates.** Continuity 0/0 and fork-tree placed, but:
- `report-payoff-coverage`: `f-020` DANGLING in both terminal worlds — I'd
  marked the dam-threat fact `expected` but never paid it. Also two
  `[payoff->unmarked]` advisories (f-110→f-103, f-210→f-203: paying off setups
  I hadn't marked `expected`).
- `report-payoff-substantiation`: `f-002 <- f-301` and `f-010 <- f-302`
  UNSUBSTANTIATED — the substantiation rule credits a payoff only when it
  carries the SAME subject+predicate with a DIFFERENT value (a real
  state-change). My typed legs re-asserted the same value, or used different
  predicates, so nothing discharged.

FIX (a redesign of the typed legs into genuine state-changes):
  - Modelled the wedged-gate reveal as a single `gate-state` track: `wedged`
    (f-001) → `wedged-on-purpose` (f-002) → `surfaced` (f-301). f-301 now
    discharges both with a real value-change.
  - Modelled the verdict track: `accused` (f-022, new) → `keeper` (f-302).
  - Made the dam-threat fact `f-020` untyped and dropped its `expected` mark
    (it is a stakes-statement, not a planted setup that must visibly pay off);
    the dam outcome is carried by the world-line facts instead.
  - Added `f-021` (lower row `asleep`) as the setup whose fate changes per path:
    `drowned` (f-103, sluice) / `woken` (f-202, ride).

**Iteration 3.** FLAG: f-303 hit the same typed-entity-object rule (object
`sluice` missing from entities). FIX: added it.

**Iteration 4.** `report-payoff-coverage`: `f-202` DANGLING in `ride` (marked
`expected`, never paid). `report-payoff-substantiation`: `f-011 <- f-303`
UNSUBSTANTIATED — the belief-break is a *subject* change (father→sela), which
the discharge rule (same-subject value change) does not credit (see "awkward
points" below). FIX: (1) made `f-210` pay off f-203, added a dedicated
`f-211` (lower row `saved`) to pay off f-202 with a matching `lower-row/fate`
state-change; (2) un-marked f-011 as a setup and added `f-012` (a millhand
acting on the false belief — tries to stop Sela's hand) so the belief is
load-bearing *in action* without forcing a hollow payoff edge; `f-303` remains
the belief-break fact.

**Iteration 5 — connective fill.** The playthrough showed content-empty scenes
(`begins=0`): sc-01, sc-10a, sc-11a, sc-11b. Added `f-000` (storm crests the
dam, sc-01), `f-105` (dam holds/mill spared, sc-10a), `f-106` (drowned counted,
town blames the hand not the unseen gate, sc-11a), `f-206` (no body pulled but
mill broken; grief not blame, sc-11b). Re-ran: every scene now carries a beat.

After iteration 5 ALL gates are clean (see final outputs in `gate-output.txt`).

## How I handled the shared dawn ending both paths reach

This was the core of the assignment and the confluence mechanism handled it
**cleanly and naturally** — there was NO point where I was forced to duplicate
or compromise the shared ending.

- The reckoning convening (`f-300`), the wedged-gate surfacing (`f-301`), the
  verdict (`f-302`), the belief-break (`f-303`), and the river's-edge close
  (`f-310`) are each authored ONCE, on `branch:"dawn"`. The
  `report-playthrough-manuscript` walks confirm they appear in BOTH `sluice`
  and `ride` at the tail — one authoring, two readings.
- The premise wants the *content* of the reckoning to differ by what the night
  did (different dead, different ruin). The brief's confluence corollary
  forbids a confluence fact from citing a scene that exists on only one path.
  I resolved this exactly as the brief prescribes: the **path-specific**
  reckoning facts live on the FORK branches but are placed at the shared scene
  `sc-rk` — `f-110` (sluice: the drowned mourned, cites `sc-09a`), `f-210` +
  `f-211` (ride: wrecked mill witnessed / lower row saved, cite `sc-10b`,
  `sc-09b`). Because they are tagged with their fork branch, the off-path
  evidence is reachable in that branch's own world-line; the gates accept it.
- The **confluence facts' own evidence** stays strictly on the shared spine /
  shared ending: `f-301` cites `sc-03`,`sc-04` (the wedged-gate scenes, both on
  the spine before the fork — reachable from BOTH parents), `f-302` cites
  `sc-02`,`sc-06` (spine), `f-310` cites only `sc-rv`. No
  `confluence_evidence_unreconciled` flag ever fired. This split — "shared
  truth on the confluence, path-flavoured content on the fork branch, both at
  the same scene" — is the one structural subtlety, and once understood it is
  the natural expression, not a workaround.

## Awkward points worth recording (honest notes)

- **Belief-break is a subject change, not a value change.** The town's false
  belief is "the *father* is keeper"; it breaks to "*Sela* is keeper." Modelled
  with `keeper-of`, that is a change of *subject* (father→sela), which
  `report-payoff-substantiation`'s discharge rule (same subject+predicate,
  different value) does not credit. Rather than distort the belief into a
  same-subject value-flip just to satisfy a craft report that is NOT in the
  hard FIX list, I expressed the belief honestly and made it load-bearing
  through an *action* fact (`f-012`, the millhand acting on it) plus the
  break fact (`f-303`). The belief still exists and is acted upon, as required;
  it simply isn't a typed-substantiated setup, which is the accurate reading.
- **`main` shows DANGLING setups; that is correct.** The spine setups (f-001,
  f-002, f-021, f-022) read DANGLING in the non-terminal root world `main`
  because they pay off only *after* the fork. The brief's bar is "no dangling
  in any TERMINAL world" — in both `sluice` and `ride` every one of them is
  `paid`. So nothing is actually left dangling; `main` is a partial walk that
  stops at the fork.
- **Typed legs had to be designed as state TRACKS, not labels.** To get clean
  substantiation I had to think of each load-bearing thread as a value that
  changes over the walk (gate-state: wedged → wedged-on-purpose → surfaced;
  lower-row fate: asleep → drowned|woken → mourned|saved; verdict: accused →
  keeper). That is a discipline the tool rewards, not a workaround — but it is a
  modelling commitment you discover by reading the substantiation flags, not
  one obvious up front.

## Final gate status (all clean)

- `validate-continuity`: violations 0 (structural 0, interval 0).
- `report-fork-tree`: 3 world-lines, 0 unplaced fork points; `dawn` converges
  from `sluice@sc-11a` and `ride@sc-11b` (diamond visible).
- `report-playthrough-manuscript --world sluice` / `--world ride`: each
  unplaced=0, undecidable=0; both walks reach `sc-rk`+`sc-rv` (the shared ending
  authored once); `outside order=4` = the other path's middle correctly excluded.
- `report-payoff-coverage`: terminal worlds `sluice`/`ride` dangling=0.
- `report-payoff-substantiation`: terminal worlds 0 unsubstantiated, 0
  unverifiable (every credited setup is a substantiated typed state-change).
- `report-timeline-gaps` (sluice/ride/dawn): 0 violations.
