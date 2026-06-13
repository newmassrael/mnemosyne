# rung-3 cross-subject preconditions — PoC scope (code 0, scratch, gitignored)

Date: 2026-06-13. Method: PoC discipline — semantic FIT + falsification,
existing primitives only, findings = SSOT, scratch store NOT committed.

## Provenance

Depth-ladder rung 3 (R476): an action fact must carry its PRECONDITION facts;
the author filling them surfaces a contradiction. The measured instance
(scale-floor, graded): the murderer Surel "poisons via the consulting-room
draught" but is a ground-floor bed patient with no key to the locked room — a
causal-coherence defect the loop's gates MISSED. This PoC tests the cheapest
honest mechanism and what is genuinely new.

## The two questions

- **Q1 — substrate FIT**: can the locked-room contradiction be checked by
  existing primitives once the preconditions are typed?
- **Q2 — what is net-new, and is "cross-subject" needed?** (the keystone): is
  rung-3 a NEW gate, or (a) a precondition-DECLARATION/forcing mechanism + (b)
  the EXISTING exclusivity gate? And does the measured static locked-room need a
  cross-subject "must-equal" RELATION, or does a concrete required leg suffice?

## The empirical test (encode next)

Encode the locked-room with typed facts:
- `surel | at-location | ground-ward`, `draught | at-location | consulting-room`,
  `doctor | holds | doctor-key`, and the action `surel | administers | draught`.
- Rules armed: `at-location` exclusive per subject; `holds` exclusive per object.

- **Test 1 (naive — author writes the action, declares NO preconditions):**
  expect `validate-continuity` violations 0. The gate cannot connect "administers
  draught" to "must be co-located + authorized" — that is commonsense the author
  must declare. The locked-room fault is INVISIBLE (matches scale-floor).
- **Test 2 (author declares the preconditions as concrete typed facts):** add
  `surel | at-location | consulting-room` (co-location requirement) and
  `surel | holds | doctor-key` (authority requirement) at the action's point.
  Expect EXISTING exclusivity to fire BOTH: surel in two locations (per-subject),
  and two holders of the key (per-object).

## Pre-registered FIRE vs REFUTE

This is a FIT/falsification PoC (planted faults — tests the mechanism +
decomposition, not a detection rate).

- **FIRE (a NEW rung-3 piece is justified)** iff Test 1 is silent AND the
  net-new value is a mechanism the existing primitives lack — specifically a
  precondition-DECLARATION (a `requires` shape) + a coverage surface for
  UNFILLED preconditions (the forcing function R476 named), without which the
  author never declares the leg and the gate stays silent.
- **REFUTE the CROSS-SUBJECT relation specifically** iff Test 2 shows a concrete
  required leg + existing exclusivity suffices for the measured static case —
  i.e., the "derive surel's required location FROM the draught's location"
  cross-subject "must-equal" relation is a generalization with no pull yet
  (hardcoding the concrete precondition is enough).
- **Full REFUTE** iff Test 1 already catches it (no gap) or the whole thing is
  pure reuse with no new declaration needed.

## Honest boundary

- Self-plant: tests the mechanism + decomposition, not field prevalence.
- The forcing is the hard part: the gate CANNOT derive that "administer draught"
  requires "co-located + authorized" — that is domain commonsense. rung-3 can
  only make the author DECLARE it; declaring surfaces the contradiction. The
  plausibility ceiling (R476) stands: the store checks declared preconditions,
  never invents them.
