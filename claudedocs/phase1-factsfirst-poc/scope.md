# Facts-first authoring — PoC scope (code 0, scratch, gitignored)

Date: 2026-06-13. Method: the agent authors a fact-base FIRST, gate-checks it,
then hand-renders prose AS A PROJECTION. Deterministic gate = objective judge.
Findings = SSOT.

## Provenance

Owner insight (2026-06-13): the self-extraction gap (R494 — implicit facts lost
in prose→facts) is an artifact of the WRONG ORDER. Flip it: author FACTS first,
render prose FROM them. A projection cannot contain a fact the base lacks, so the
extraction gap dissolves. This re-derives the North-Star architecture thesis
(every artifact is a projection of the one fact substrate) and the CYOA-renderer
backlog (facts → narrative).

## The two testable claims (mechanism; prose-craft deferred to blind judge)

1. **The extraction gap is structurally impossible under facts→prose.** With the
   facts authored first and gate-checked, there is no lossy extraction step; the
   gate validates every load-bearing fact by construction, BEFORE any prose.
2. **Truth vs revelation separates cleanly via FRAMES** (the perspectival model):
   the GROUND-TRUTH frame holds what is true (gate-checked); a BELIEF frame holds
   the misdirection; the prose PROJECTS the belief frame (withholds the truth =
   mystery), while cross-frame divergence is dramatic IRONY (data, not a
   contradiction) — and `report-irony-intervals` (R455) surfaces it
   deterministically.

## The slice (author as FACTS first)

The Belvoir letter, but truth-first. Frames `truth` and `household`. The murder
truth: Helene WAS in the study (she burned the letter). The misdirection: the
household BELIEVES her confined to the sickroom.
- `truth`: `helene @ study` (sc-murder) — the hidden fact, authored up front
  because we are building the truth, not extracting it from prose.
- `household`: `helene @ sickroom` (sc-murder), with a recorded cross-frame
  conflict edge to the truth fact (the divergence = irony).

## Procedure (live)

1. Author the fact-base (both frames), gate-check: within each frame consistent;
   cross-frame divergence = DATA (no violation). `report-irony-intervals`
   surfaces the divergence window.
2. CONTRAST: a real same-frame error (two locations for Helene IN the `truth`
   frame) IS caught — facts-first does not weaken the gate.
3. Hand-render two prose projections: one from `household` (withholds the truth =
   mystery prose), one from `truth` (the reveal). Show the truth lives in the
   checked base while the prose reveals selectively.

## Honest boundary

- No automated facts→prose renderer exists (the CYOA backlog is unbuilt); the
  prose here is hand-rendered to demonstrate the WORKFLOW + mechanism.
- Prose QUALITY (does facts-first produce GOOD narrative, or wooden list-prose?
  — the scale-floor's craft tension) is NOT testable by a self-authoring agent;
  it needs the blind, session-separated judge (R469). This PoC tests the
  mechanism + the truth/revelation separation only.
- Fact COMPLETENESS still needs authoring: facts-first FORCES the author to think
  in facts (so implicit facts are likelier declared, and the gate checks the base
  before prose), but it does not invent the relevant facts — the relevance
  ceiling (R476) stands.
