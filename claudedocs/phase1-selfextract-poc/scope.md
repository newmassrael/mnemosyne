# AI self-extraction loop — PoC scope (code 0, scratch, gitignored)

Date: 2026-06-13. Method: the AI (this agent) performs the self-extraction
LIVE; the deterministic gate is the OBJECTIVE judge (not self-grading). Findings
= SSOT.

## Provenance

North Star (owner, 2026-06-13): AI-FIRST — the AI carries the AAA scenario
end-to-end; the store is the AI's external memory, the gates its self-consistency
feedback, discovery the AI extracting its OWN facts from its OWN prose. The
scale-floor (R475) measured the binding constraint as COVERAGE — the AI under-
declares facts, so the gates have nothing to bite. This PoC probes the
self-extraction loop and, critically, its FAILURE MODE.

## The question

When the AI authors prose and self-extracts the load-bearing facts, is the
extraction complete enough that downstream contradictions fire? And WHERE does
it fail — which kinds of facts get under-extracted?

## Design — isolate the variable (extraction completeness, not gate type)

ONE gate (`at-location` exclusivity per subject), TWO contradictions of the SAME
shape, differing ONLY in whether the load-bearing fact is EXPLICITLY STATED in
the prose or IMPLICIT (an entailed precondition):

- **Explicit contradiction**: the prose states Helene is confined to the
  sickroom (ongoing) AND states she is seen at the conservatory. Both locations
  are on the surface → a faithful prose→facts extraction captures both → the
  exclusivity gate catches it.
- **Implicit contradiction**: the prose states the letter was burned in the
  study and that only Helene had motive — but never states Helene's location.
  The contradiction (a confined Helene could not have been in the study to burn
  it) is an ENTAILED precondition, not a stated fact. A faithful extraction
  records `letter @ study` but NOT `helene @ study` → the gate is SILENT.

## Procedure (live)

1. Author `prose.md` (3 scenes), then SELF-EXTRACT faithfully — encode only the
   facts the prose STATES (the honest forward-authoring pass).
2. Run `validate-continuity` with `at-location` exclusivity. Measure: the
   explicit contradiction fires; the implicit one does NOT (the coverage gap,
   made concrete and deterministic).
3. CLOSE THE LOOP: infer the unstated precondition (`helene @ study`, required
   to have burned the letter there), add it, re-run → the gate now catches it.

## What this measures honestly

- The OBJECTIVE result is the gate's deterministic verdict — not self-graded.
  Under-extraction = the gate is silent, and reporting that silence IS the
  finding (no-silent-caps).
- The expected failure mode: faithful extraction covers EXPLICIT stated facts,
  under-covers IMPLICIT/entailed facts (preconditions) — the rung-3 lesson, now
  framed as a self-extraction coverage gap.
- Key probe: can a TOOL surface the gap? `report-typing-candidates` (R458)
  surfaces untyped EXISTING facts — but the implicit precondition is a MISSING
  fact (never recorded), so no deterministic tool can surface it. Closing the
  gap requires INFERENCE (the AI noticing the entailment), not tooling.

## Honest boundary (the self-confirmation ceiling)

A single self-authoring agent CANNOT blind-measure its own coverage RATE — that
needs the scale-floor's independent-judge protocol (separate sessions, R469
discipline). This PoC tests the loop MECHANICS + the failure MODE on one slice;
the coverage-rate measurement at AAA scale stays the deferred blind protocol.
The true ceiling (R476) stands: the gate checks declared facts, never the prose
meaning behind them.
