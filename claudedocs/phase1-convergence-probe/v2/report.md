# convergence-probe/v2 — the fork→join VALIDATION — report

**Question.** Now that the convergence-JOIN substrate is BUILT (R532 declare+visible,
R533 shared-suffix composition, R534 review, R535 per-parent reconciliation gate),
does re-authoring the SAME Harlow Mill convergence premise on the new substrate
REMOVE the v1-measured duplication tax, make the convergence VISIBLE, preserve
fidelity, and exercise the R535 gate correctly?

**Setup.** R536 manifest sha256
`c620aa83c4ad296ee7a95535804159351299b0c877995aa82cad57ea36a4556c` pinned
pre-execution. ONE fresh-context blind author (R469 bound, the 9th use), given ONLY
`premise.md` (Harlow Mill, reused verbatim from v1) + `author-brief.md` (the proven
R520/R524 top-down contract, EXTENDED with the confluence move as ordinary tool
vocabulary alongside fork + the R526 precondition clause). The orchestrator (this
lineage, which built R532–R535) de-risked the tool contract on a throwaway minimal
diamond, then rebuilt the author's store FRESH from `sections.json` + `facts.json` +
`order.json` and ran every gate independently — it authored no fact and trusts no
author claim.

**Authored base.** 17 scenes, 26 facts, 2 frames (`gt` + `town-belief`), 8 entities,
5 predicates, schema 23. Shared spine `sc-01..sc-07`; ONE exclusive fork at `sc-07`
into `sluice` (opens; lower row drowns) and `ride` (holds + rides; mill wrecked);
ONE confluence `dawn` (`converges_from` `sluice@sc-11a`, `ride@sc-11b`) carrying the
shared reckoning `sc-rk` + river's edge `sc-rv`. 5 write→gate→repair iterations.
Rebuilds clean from a pristine schema-23 seed.

## RESULT — OUTCOME-A (validated). The build removes the v1 tax end to end; all three pins hold.

| Measure | v1 (forest, R531) | v2 (confluence, this round) |
|---|---|---|
| **M1 duplication** | the 6-beat shared ending authored as **12 facts** (2×, one set per branch) | the shared ending authored as **5 facts ONCE** on the `dawn` confluence (`f-300`,`f-301`,`f-302`,`f-303` @ `sc-rk`; `f-310` @ `sc-rv`); **0 shared-ending facts duplicated** across the two parents. Tax removed. |
| **M2 convergence visible** | a TREE (the merge invisible) | a **DIAMOND** — `report-fork-tree`: `dawn converges from sluice at sc-11a` + `dawn converges from ride at sc-11b`, both forks placed, 0 unplaced. |
| **M3 reconciliation gate** | n/a (no confluence existed) | **clean on the well-formed store** (validate-continuity 0 structural / 0 interval, `cross_scope_pairs=0`) **AND fires on the negative control**: mutating `f-301`'s evidence to cite `sc-09a` (sluice-only) → exactly `confluence_evidence_unreconciled {fact: f-301, confluence: dawn, parent: ride, evidence: sc-09a}`, exit 1. Non-vacuous, not a false-positive machine. |
| **M4 shared-once semantics** | each branch carries its own copy | `report-playthrough-manuscript --world sluice` and `--world ride` both walk the **SAME fact-ids** at the tail (`f-300`,`f-301`,`f-302`,`f-303` @ `sc-rk`; `f-310` @ `sc-rv`), 0 unplaced / 0 undecidable — one authoring, two readings (R533 forward visibility). |
| **M5 author friction** | the author named duplication **"unavoidable"** | the author: the confluence "handled it **cleanly and naturally** — there was NO point where I was forced to duplicate or compromise the shared ending." The inverse of v1. |

### The pins (pre-committed, orchestrator-verified on a fresh rebuild)

- **PIN-1 (tax removed): HOLDS.** The shared dawn ending is authored ONCE (5 facts on
  `dawn`); 0 shared-ending facts duplicated across `sluice`/`ride`. (v1 = 12 facts.)
- **PIN-2 (fidelity + visible): HOLDS.** validate-continuity 0/0; fork-tree shows the
  confluence merging 2 parents (diamond); both parents' manuscripts carry the shared
  suffix once, 0 unplaced. Terminal payoff dangling = 0 (`sluice` 5/0, `ride` 6/0);
  substantiation unsubstantiated = 0; timeline violated = 0.
- **PIN-3 (gate correct): HOLDS.** Clean on the well-formed store; fires
  `confluence_evidence_unreconciled` on the one-parent-only negative control.

## Why this closes the loop

v1 (R531) measured the cost the forest imposed on a real convergence story — the
entire 6-beat shared ending duplicated to 12 facts, the merge invisible (a tree), the
author independently naming "no native join node … duplication is unavoidable." The
build (R532–R535) targeted exactly that. v2 proves the removal on the SAME story end
to end: a blind author, given the confluence as ordinary tool vocabulary, authored
the shared ending ONCE (5 facts, not 12), the merge shows as a diamond, both parents
share the single authored ending, and the R535 reconciliation gate is both
non-vacuous (passes the clean store) and correct (fires on a one-parent-only
dependency). The author used the confluence corollary exactly as designed —
path-specific reckoning *content* on the FORK branches placed at the shared scene
(`f-110` sluice; `f-210`/`f-211` ride), confluence facts' evidence kept on the shared
spine/ending — so the gate never falsely fired and the content-differs-by-path
requirement was met without duplication.

## Honest notes

- **Convergence-orthogonal modelling observation (carried, not a convergence defect):**
  the author flagged that the dawn belief-break is a SUBJECT change (the town's
  "keeper" belief moves father→Sela), which the same-subject typed payoff-substantiation
  discharge rule does not credit, so it was expressed as an action fact rather than a
  typed-substantiated payoff. This is the same kind of tool-shaping observation v1's
  author made about "Sela IS the keeper" — orthogonal to convergence, a future look.
- **The de-risk vs the experiment.** The orchestrator de-risk (a throwaway minimal
  5-fact diamond) confirmed the mechanics; the experiment confirmed they hold for a
  blind author at story scale on a messier real store (17 scenes, two frames, a
  cross-fork setup→payoff, a belief frame). No complexity-dependent bug surfaced
  (OUTCOME-C did not fire).
- **Blind craft judge: deferred (not required for routing).** The manifest made the
  coherence judge optional; the deterministic pins are the validation core and the
  gate-clean per-world manuscripts both read as one complete story reaching the shared
  reckoning. The R504/R515 craft verdicts stand for the render question; v2 is a
  substrate validation.

## Decision (pre-committed rule) — OUTCOME-A: the convergence-JOIN arc is DONE

All three pins hold ⇒ the convergence-JOIN build (R532–R535) is VALIDATED on a real
authoring attempt: it removes the measured v1 duplication tax (12 → 5 facts, 1×),
makes the convergence visible (diamond), preserves fidelity (shared ending authored
once, present in both parent walks), and the reconciliation gate works (clean on
well-formed, fires on malformed). The fork→join loop is CLOSED end to end. Per the
R536 decision rule, the convergence-JOIN arc is DONE; the next frontier is breadth
(the NPC-breadth probe / the SCXML-seam + L3 concurrency scope rounds / v3 A-1·A-4).
