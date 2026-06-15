# ai-breadth-depth-experiment/v1 — report (R545)

**Manifest:** `manifest.json`, sha256 `4e9498e4c6cdbdf71d27ecd40306fb587666f7da2cc41979572f1f63269116cb`,
pinned in the R544 ledger entry pre-execution.
**Designed:** R543 (sec 7.34). **Executed:** R545 (blind author + 3 blind judges; the
orchestrator rebuilt fresh + measured + assembled; authored no fact, judged nothing — the
R469 contamination bound, the 11th use of the blind-experiment family).

## Question

Can an AI SELF-AUTHOR a gate-clean, causally coherent story that is BOTH DEEP (a long,
consequentially branching arc) AND BROAD (a large individuated knowledge-cast) from a
premise only? R524 proved DEPTH alone (70 scenes), R541 proved BREADTH alone (12 frames,
29 scenes). The open question: does holding BOTH at once degrade either (depth diluting
the cast into omniscience leaks / palette-swaps, or breadth flattening the branches)?

## What the blind author produced (premise-only, its own write-gate-read-repair loop)

"The Ember Winter at Grayloam" — a snowed-in iron settlement; the ironmaster is found
dead. Ground truth: a secretly-dying, guilty ironmaster (skimmed the stores for years)
dosed himself with the herbwife's heart-tincture when the auditing factor cornered him,
and forged the ledger's last page to frame the innocent collier. Not a murder.

- **95 scenes** (`sc-01..sc-95`); shared spine sc-01..sc-20, then **3 sequential/nested
  forks** (sc-20 ledger|ration; sc-35 company|village; sc-48 law|settle) → **4 terminal
  world-lines** (ration / village / law / settle), terminal walks 37–56 scenes.
- **13 frames** = `gt` + **12 individuated people** (coll/POV, factor, widow, clerk,
  collier, keeper, herbwife, carter, soldier, prospector, chaplain, child).
- 22 entities, 7 predicates, **117 facts**; 5+ setup→payoff chains (two single-holder:
  herbwife-illness, widow-saw-medicine). **3 write-gate-repair iterations.**
- **Convergence NOT used** — a natural forest of forks; no shared continuation the plot
  wanted declared once (the R536 confluence tool was available, the author judged it
  unneeded). → NO pull for the R528 series-parallel lattice this round.

## Deterministic pins — ALL HOLD (orchestrator rebuilt the store FRESH from the author's
## sections.json + facts.json + order.json into a clean schema-23 seed; independent of the
## author's store file)

**PIN-D1 (gate-clean convergence at combined scale): HOLDS.**
- import into a fresh empty seed = 95 sections + 13 frames + 6 branches + 22 entities +
  7 predicates + 117 facts created, 0 errors (loads clean).
- `validate-continuity --order`: violations 0 (structural=0, interval=0); conflict_pairs=0,
  cross_scope(data)=0, unordered=0. (Includes R522 evidence-reachability + R488 off-branch.)
- `report-playthrough-manuscript --world W` for every world-line (main/ledger/ration/
  company/village/law/settle): unplaced=0, undecidable=0, undeclared adjacencies=0.
- `report-timeline-gaps --world W`, every branch: 0 interval rules / 0 gaps.

**PIN-D2 (DEPTH floor): HOLDS.**
- 95 scenes (≥60); 6 branches (≥3); 4 terminal world-lines (≥3); `report-fork-tree`:
  3 fork points placed, 0 unplaced (≥2).
- `report-payoff-coverage`: every TERMINAL world dangling=0 (ration paid=7, village paid=7,
  law paid=9, settle paid=9). The dangling on main/ledger/company (5/3/2) is correct —
  intermediate-spine setups paying off downstream in their child branches (the R524 shape).
- `report-payoff-substantiation`: unsubstantiated=0 in every world (the `unverifiable`
  rows are benign advisories — the author declined to fabricate typed values, the R542
  lesson). 5+ payoff chains (≥3).
- `validate-continuity`: 0 off-branch / evidence-unreachable.

**PIN-D3 (BREADTH floor): HOLDS.**
- **12 distinct person-frames (≥10), each holding ≥1 fact** — `report-frame-view` at each
  frame's key scene (run/frame-views/): coll 15, factor 6, widow 4, clerk 4, collier 3,
  keeper 2, herbwife 5, carter 2, soldier 3, prospector 4, chaplain 3, child 3 (holding).
- every frame-view non-empty + three-state honest (non-vacuous: querying coll about the
  death on the ration road shows holding=2/not_holding=3 at sc-86 → holding=3/not_holding=2
  at sc-90 — knowledge GROWS along the road, not-held states honestly reported; the
  acquisition boundary is visible in the gate).
- 0 within-frame contradiction (conflict_pairs=0).

**Both floors held SIMULTANEOUSLY** — depth did not collapse breadth, breadth did not
collapse depth, at the deterministic level.

## Judged — 3 fresh blind judges (judge-brief.md rubric, over the 4 terminal manuscripts)

| OVERALL axis | J1 | J2 | J3 | majority |
|---|---|---|---|---|
| Causal coherence | 5 | 5 | 5 | **5** |
| Completeness | 5 | 4 | 4 | 4 |
| Knowledge-realism | 5 | 5 | 5 | **5** |
| Cast-distinctness | 4 | 4 | 5 | **4** |
| Branch-integrity | 5 | 5 | 5 | **5** |
| Overall | 5 | 5 | 5 | **5** |
| Knowledge breaks | none | none | none | **none** |

- **The feared cross-tension failure did NOT materialize.** Knowledge-realism = 5/5/5 with
  **zero knowledge breaks** — all three independently named the frame-scoped knowledge
  architecture the biggest STRENGTH ("no one knows across a road they are not on"; "in
  ration Coll reconstructs the suicide WITHOUT reading the ledger, so he never learns the
  collier was framed, and the ending honours that gap"). The omniscience-leak risk (the
  unenforced breadth axis, R539) did not appear even at 95 scenes × 12 frames × 4 roads.
  Branch-integrity = 5/5/5 (breadth did not flatten the branches; the roads "diverge and
  stay diverged" into four distinct motivated endings).

- **The honest blemish (all three judges, the SAME finding — not downplayed):** the broad
  cast is **front-loaded** — vivid through the shared trunk (sc-01..20), it **thins to a
  handful of principals in the deep branch tails** after the midwinter forks (carter, cook,
  smith, keeper, soldier, child, prospector recede to scenery). And several eyewitness
  setups pay off on only SOME roads (soldier sc-16 + prospectors sc-17 close only in
  village; child sc-18 only in ration; the prospectors' undeclared-ore thread sc-17/29
  dangles in law/settle; the bonded servant's fever sc-34 unresolved in law/settle). This
  is **cast-SUSTAINMENT under depth** — breadth proved harder to hold across a long deep
  arc than to establish — and it is the measured texture of the combined ceiling. It is a
  COMPLETENESS/breadth-sustainment slip (cast 4 / completeness 4 majority), NOT a coherence
  or knowledge break (the R476 ceiling / R526-clause family: which dangling threads matter
  is a judgment no deterministic gate makes). The lone fully-sustained road, `village`, drew
  cast 5/5/5 — showing the author CAN sustain breadth on a road, but did not on all four.

## Control cross-reference (orchestrator, judges blind)

R524 (70-scene/7-branch DEPTH) judged coherent 5/5; R541 (29-scene/12-frame BREADTH) judged
5/5 knowledge-realism + 5/5 cast-distinctness. This combined base holds comparable judged
coherence (5/5/5) + knowledge-realism (5/5/5) + branch-integrity (5/5/5) with both floors
pushed at once; cast-distinctness drops one notch (4 majority vs R541's 5) — the
sustainment cost of adding depth to breadth.

## Decision (pre-committed rule: all_pins_hold_and_coherent)

ALL three pins hold AND judges find it coherent with a real cast and distinct branches
(overall 5 ≥4, knowledge-realism 5 ≥4, cast-distinctness 4 ≥4, branch-integrity 5 ≥4, no
major breaks) →

**AI SELF-AUTHORING SCALES ON THE COMBINED AXIS.** A blind author held a 95-scene /
4-terminal / 12-frame deep-AND-broad base gate-clean and judged-coherent from a premise
only. The R529 binding bottleneck (AI self-authoring at scale + breadth + coherence) is met
near AAA scale on all of: depth (R524), breadth (R541), and now **both together**.

**No deferred lever pulled** (the R541-style clean outcome): no omniscience leak → the R539
NPC-knowledge-acquisition gate stays YAGNI; no convergence-duplication (forest, by author
choice) → the R528 series-parallel lattice stays deferred; no possession/prose-ghost error
→ the R527 world-state axis stays deferred. The cast-sustainment blemish is a CRAFT/
authoring-brief observation (the next run's brief can ask for the cast to stay active into
the branch tails — the R526-clause register), not a deterministic-gate lever.

## Tracked evidence

manifest / premise / author-brief / judge-brief / runbook; run/author/{sections,facts,
order}.json + author-log.md; run/manuscripts/world-{ration,village,law,settle}.md;
run/frame-views/frame-*.txt; run/judges/judge-{1,2,3}.md. Scratch `*.atomic.json`
(seed + rebuild + author store) gitignored.
