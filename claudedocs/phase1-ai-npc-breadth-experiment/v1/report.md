# ai-npc-breadth-experiment/v1 — report (R541, executed)

**Question (manifest):** can an AI SELF-AUTHOR, from a premise only, a gate-clean
causally-coherent story with a BROAD CAST — many distinct people, each a
knowledge-frame holding what THAT person knows and believes — running its own
write-gate-read-repair loop, with the cast kept consistent by the gates? R524 proved
DEPTH (70 scenes, judged coherent); this isolates BREADTH (cast size + individuation)
at moderate length (~22-30 scenes), forest-only (no convergence).

**Manifest pinned pre-execution:** sha256
`f1f07bfda178a39c6f81798145ac6f87575c5c0eb7d6475c350776c45f9f6bde` (R540 ledger,
committed `c3edbe9` before any subagent ran). Premise: *The Lantern House at Corvath
Ford* (fresh, original). Firewall (R469, 10th use): this lineage authored the
design/manifest/briefs; a blind author authored the facts, the orchestrator ran the
gates, 3 blind judges scored. The orchestrator authored no fact and judged nothing.

## RESULT = breadth REACHES (decision-rule branch 1: both pins hold AND coherent broad cast)

A blind author authored a **29-scene / 80-fact / 3-world-line** crowded-house mystery
with **12 distinct person-frames** (+ the ground-truth frame) from the premise only, in
**2 write-gate-repair iterations**. Both deterministic pins HOLD (orchestrator rebuilt
the store FRESH from the author's sections.json+facts.json+order.json — independent of
the author's store file — and ran every gate); the 3 blind judges scored **unanimous
5/5 overall, 5/5 knowledge-realism, 5/5 cast-distinctness**. The omniscience-leak risk
(the breadth axis's central worry, unenforced by any gate) did NOT materialize.

## PIN-B1 — convergence to gate-clean (HOLD)

Rebuilt fresh from the author's JSON into a clean schema-23 seed (80 facts / 29
sections import clean), then:

- `validate-continuity --severity reject --interval-severity reject`:
  `facts=80 order_nodes=29 conflict_pairs=0 cross_scope(data)=0 unordered=0` —
  **violations: 0 (structural=0 interval=0)** (incl. R522 evidence-reachability + R488
  off-branch).
- `report-fork-tree`: **3 registered world-lines, 0 unplaced fork point(s)**; `name` /
  `hold` / `act` all fork from `main` at `sc-17`; every world reaches a terminal.
- `report-playthrough-manuscript --world {name,hold,act}`: **unplaced=0, undecidable=0**
  each (outside order=8 = the other branches' scenes correctly excluded).
- `report-timeline-gaps --world {name,hold,act}`: **0 interval rules / 0 gaps** each.

## PIN-B2 — breadth floor + structural floor (HOLD)

- **Breadth floor: 12 person-frames, all 12 populated (>= 1 fact)** — far above the
  floor of 8. Each projects to a non-empty, three-state-honest dossier
  (`report-frame-view`, dumped to `run/frame-views/`):

  | frame | holds | | frame | holds | | frame | holds |
  |---|---|---|---|---|---|---|---|
  | wend | 25 | | soldier | 5 | | groom | 2 |
  | hooded(Cray) | 7 | | midwife | 4 | | trader | 2 |
  | clerk | 6 | | tinker | 4 | | warden | 2 |
  | factor | 4 | | bride | 3 | | drover | 2 |

- `report-payoff-coverage`: **dangling=0 in every world** (act/hold/main/name); 8
  setups, all paid.
- `report-payoff-substantiation`: **0 unsubstantiated** (no hollow payoffs); 2 typed
  state-change pairs substantiated, the rest the honest `unverifiable` for untyped
  epistemic reveals.
- off-branch / evidence-unreachable: **0** (from validate-continuity above).

## JUDGED (3 blind judges, frame-labelled per-world manuscripts; no pin)

| axis | judge-1 | judge-2 | judge-3 |
|---|---|---|---|
| coherence (overall) | 5 | 5 | 5 |
| completeness (overall) | 5 | 4 | 5 |
| **knowledge-realism** | **5** | **5** | **5** |
| **cast-distinctness** | **5** | **5** | **5** |
| branch-integrity | 5 | 5 | 5 |
| **overall** | **5** | **5** | **5** |

All three independently named the same strength — a genuinely individuated cast
("a house of separately-wrong minds whose beliefs are exactly earned by their own
paths"; "no one holds a fact from a world-line they are not on … the people who were
'right there' know precisely what proximity would grant"). The night's two real crimes
(a drowning over a forged whole-house deed-bond; a staged robbery masking
embezzlement) are heard by the house as one noise and refracted through twelve minds
that each know only their slice.

**Decision rule (manifest, pre-committed):** both pins hold AND majority overall >= 4
AND knowledge-realism >= 4 AND cast-distinctness >= 4 AND no major knowledge breaks =>
**AI-self-authoring REACHES breadth.** Met decisively (5/5/5 unanimous). The depth
result (R524) extends to a broad, individuated cast.

## Honest caveats + the lone blemish

- **The one judge-flagged blemish (all three, the SAME one):** HOLD's `sc-32` — Wend
  burns "the leaf of the deed-bond he copied from the body's papers," but on the HOLD
  line the body's bond stays on Cray (who crosses away with it, f-hold-03), so the
  "copied leaf" prop is introduced at the reveal rather than planted earlier. Judges 2
  and 3 classified it a minor COMPLETENESS / provenance slip (**knowledge breaks:
  none**); judge 1 read it as a borderline HOLD-only knowledge-access hole (knowledge
  4/5 on HOLD only; 5/5 overall). This is the **R476 ceiling / R526 case-1 type** (a
  missing precondition setup), NOT an omniscience leak: the brief instantiated the R526
  sec-7.28 precondition clause and it REDUCED but did not eliminate the recurring
  missing-setup blemish — the same pattern as R524's act-dig survivor. It does NOT
  trigger the decision-rule's acquisition-gate branch (which requires
  knowledge-realism < 4 majority or major knowledge breaks — neither occurred).
- **The designed NPC-knowledge-acquisition gate (R539) stays YAGNI-deferred** — no
  measured pull (knowledge-realism scored 5/5/5; the omniscience-leak risk did not
  appear). It would not have caught sc-32 anyway (that is a missing-setup ceiling case,
  not an asserting-unlearnable-knowledge case).
- n=1 premise. The three branches share `sc-01..sc-17` verbatim and diverge only at the
  `sc-17` choice (all judges noted the divergence rests on Wend's single decision — a
  property of this premise's fork-at-the-end shape, not a breadth defect).
- Render not tested (R514 answered render); this is an authoring/structure test.

## Control cross-reference (orchestrator; judges blind)

- R520 small-scale base (3 frames) judged coherent 5/5 — the depth-axis bar at small
  scale. R524 scale base (70 scenes) judged coherent at depth. This base reaches the
  same judged coherence (5/5) with the cast BROADENED to 12 individuated minds at
  moderate length — breadth holds where depth held.

## As-built deviations (faithful)

- The author registered 12 person-frames (the brief asked for >= 8) and 29 scenes (the
  brief asked ~22-30). Forest-only as specified (no `converges_from`).
- `report-frame-view` evidence dumped per frame at the world-terminal where its
  knowledge lives (main-only frames at the `name` terminal sc-21; branch frames at
  their branch terminal). Scratch `verify.atomic.json` + the author's `store.atomic.json`
  gitignored; tracked = manifest, premise, briefs, runbook, the authored
  sections/facts/order JSON, author-log.md, the per-world manuscripts, the per-NPC
  frame-views, the 3 judge verdicts, this report.

SSOT = this file + the R540/R541 ledger entries.
