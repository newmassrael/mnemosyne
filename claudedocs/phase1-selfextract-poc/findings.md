# AI self-extraction loop — findings (code 0, scratch, gitignored)

Date: 2026-06-13. Scope + pre-registration: `scope.md`. The AI (this agent)
performed the self-extraction live; the deterministic gate was the OBJECTIVE
judge. Live runs against `selfextract.atomic.json`.

## Result: the self-extraction failure mode is IMPLICIT/entailed facts — and no
## deterministic tool can close it; only inference can

The gap that bounds AI-first coverage (scale-floor R475) is located precisely
here: faithful prose→facts extraction reliably captures EXPLICIT stated facts
(the gate catches their contradictions), but UNDER-captures IMPLICIT entailed
facts (preconditions, transitions) — and because the missing fact is absent from
the store entirely, no read-tool can surface it. Closing the gap requires the AI
to INFER the unstated fact, which is an authoring-loop/inference problem, not a
substrate or discovery-tool gap.

## The three live results

The slice (`prose.md`, frame gt, 3 scenes): Helene is confined to the sickroom
(stated, ongoing); seen at the conservatory (stated); the incriminating letter
is found burned in the study, only Helene having motive (the letter's location
stated, Helene's NOT). One gate: `at-location` exclusive per subject. Two
contradictions of the SAME shape, differing only in explicit vs implicit.

1. **Run 1 — faithful extraction (stated facts only): 1 violation.** The
   EXPLICIT contradiction fires: `f-confined` (sickroom) vs `f-conservatory`
   co-hold at s2 → Helene in two places. The IMPLICIT contradiction is SILENT: I
   extracted `letter @ study` but never `helene @ study` (the prose never places
   Helene in the study — "it was never explained"), so a confined Helene burning
   the letter has no facts to fire on. The coverage gap, made deterministic.
2. **Probe — `report-typing-candidates`: 0 untyped facts.** The discovery tool
   surfaces UNTYPED EXISTING facts; here every fact is typed and the load-bearing
   fact is MISSING entirely. No deterministic tool can surface a fact that was
   never recorded. The gap is invisible to tooling.
3. **Run 2 — close the loop by INFERRING the precondition.** I add
   `helene @ study` (entailed: to have burned the letter in the study she must
   have been there). The implicit contradiction now FIRES (`f-confined` vs
   `f-helene-study` at s3). The loop closes — but only because I inferred the
   unstated fact.

## Secondary observation (same failure mode, second facet)

Run 2 produced a THIRD violation (`f-conservatory` vs `f-helene-study`) because
my faithful extraction recorded no MOVEMENT transitions — the prose states three
locations but never "she went from X to Y", so all three location facts stay
open and co-hold. Un-extracted transitions are the same implicit-fact gap in a
second guise: the prose (and a faithful extraction) carries the states, not the
moves between them.

## What this locates (the North-Star leverage)

- **Explicit stated facts → extracted reliably → gate catches.** The substrate
  and the gate are sufficient (consistent with R493).
- **Implicit/entailed facts (preconditions, transitions) → under-extracted → gate
  silent.** This is the coverage bottleneck the scale-floor measured, now
  isolated to a single cause: the AI does not CREATE the implicit load-bearing
  facts as it writes.
- **No deterministic tool closes it** (Probe): a missing fact is invisible. The
  only closer is INFERENCE — the AI noticing the entailment. So the AI-first
  end-to-end leverage is the authoring loop's inference of implicit facts, an
  AI-advisory step (propose entailed preconditions/transitions from prose, the
  R457-459 pattern but AI-driven), NOT a new gate or verb.

## Convergence — this unifies the session's three PoCs

- causes edge (R493): REFUTE — decomposes into existing mechanisms.
- rung-3 cross-subject (R493): REFUTE new substrate — the check is exclusivity;
  the gap is the author declaring the precondition.
- self-extraction (here): the gap is the AI INFERRING + recording the implicit
  precondition.

All three point to ONE conclusion: **the gates are sufficient; AI-first
end-to-end coverage is bound by the AI's inference of IMPLICIT load-bearing
facts.** The product leverage is not more substrate — it is the authoring loop
reliably surfacing what the prose entails but does not state.

## Honest boundary (the self-confirmation ceiling)

A single self-authoring agent cannot blind-measure its own coverage RATE — I
know the contradictions I planted, so "what I missed" here is a MECHANISM
demonstration (faithful extraction structurally omits unstated entailments), not
a coverage statistic. The rate at AAA scale needs the scale-floor's
independent-judge, session-separated protocol (R469). The true ceiling (R476)
stands: the gate checks declared facts; whether the inferred preconditions are
the RIGHT/COMPLETE set is commonsense — author/model territory.

## Recommendation

The AI-first leverage is an AI-advisory IMPLICIT-FACT inference step in the
authoring loop: from each scene's prose, propose the entailed preconditions and
transitions (not just the typed legs of stated facts), record them, let the
existing gates check. This is testable only under a blind, session-separated
protocol (does AI inference of implicit facts measurably raise coverage without
costing prose quality — the scale-floor's open regime question). It is an
inference/loop-design problem, confirmed here to be the binding one, not a
substrate gap.
