# rung-3 cross-subject preconditions — findings (code 0, scratch, gitignored)

Date: 2026-06-13. Scope + pre-registration: `scope.md`. Method: FIT +
falsification, existing primitives only, findings = SSOT. Live runs against
`rung3.atomic.json`.

## Verdict: REFUTE new substrate — the cross-subject relation is NOT needed; the
## contradiction-check is EXISTING exclusivity; the only gap is authoring forcing

The measured locked-room fault is fully catchable TODAY with the existing
exclusivity gate, once the action's preconditions are declared as concrete
typed facts. No cross-subject "must-equal" relation, no `requires` field, no new
deterministic gate is justified by the measured case.

## The two live tests

The locked-room, typed: `surel | at-location | ground-ward`,
`draught | at-location | consulting-room`, `doctor | holds | doctor-key`,
action `surel | administers | draught`. Rules armed: `at-location` exclusive
per subject, `holds` exclusive per object.

- **Test 1 — action declared, NO preconditions → `violations: 0`, exit 0.**
  The locked-room fault is INVISIBLE. The gate cannot connect "administers
  draught" to "must be co-located + authorized" — that is domain commonsense the
  store cannot derive. (Matches the scale-floor: the loop's gates were silent.)
- **Test 2 — preconditions declared as concrete typed facts** (`surel @
  consulting-room`, `surel holds doctor-key` at the action point) **→ existing
  exclusivity fires BOTH**, exit 1:
  - `loc-excl`: surel co-holds `ground-ward` and `consulting-room` → in two places.
  - `key-custody`: `doctor` and `surel` both hold `doctor-key` → two holders.

## What this decomposes to

- **The CONTRADICTION check is existing exclusivity** (per-subject location,
  per-object custody) — Test 2. Zero new gate.
- **The GAP is FORCING, not checking** — Test 1. Nothing makes the author
  declare `surel @ consulting-room`; without the declaration there is no
  contradiction to find. The hard part is that the store CANNOT derive an
  action's preconditions (commonsense) — it can only check declared ones.
- **The cross-subject "must-equal" RELATION is NOT needed** for the measured
  static case: a CONCRETE required leg (`surel @ consulting-room`) + exclusivity
  suffices. Deriving surel's required location dynamically FROM the draught's
  location is a generalization with no pull — REFUTE.

## The one path that could add value (future pull, not now)

A forcing/coverage nudge ("this action has unfilled preconditions") would have
to KNOW that "administers requires co-location + authority" — commonsense the
store can't derive. So it would need either declared precondition rules (which
relocates the burden to rule-authoring) or an **AI-advisory precondition-
discovery** (propose an action's likely preconditions as typed legs, the author
confirms, EXISTING exclusivity checks — the R457-459 typing/edge-discovery
pattern reused). That is the only net-new piece, and it is a FUTURE pull: the
scale-floor measured the DEFECT, not a demand for automated precondition
coverage, and a single instance does not justify a discovery pipeline.

## Meta-conclusion (consistent with the `causes` PoC)

BOTH causal-coherence candidates this session — `causes` edge and rung-3
cross-subject — decompose into EXISTING mechanisms (exclusivity / succession-
cycle / interval / payoff) + more authoring (declaring the load-bearing facts).
The store's causal-coherence reach is bounded by AUTHORING BURDEN (typing
preconditions), NOT by missing gates. This is exactly R476's thesis: deeper
modeling catches more but raises the burden, and the regime question (when the
burden pays) stays the open, venue-dependent, untested variable. The deterministic
substrate is already sufficient to catch the measured locked-room; what is
missing is the author declaring the preconditions (or an AI aid that proposes
them).

## Honest boundary (true ceiling, R476, unchanged)

Whether the declared preconditions are the RIGHT/COMPLETE set (does administering
the draught really require only co-location + a key?) is commonsense — never
gateable. The store checks declared preconditions; it never invents or completes
them.

## Recommendation

REFUTE new deterministic substrate for rung-3 cross-subject. Actionable output:
(1) an authoring-discipline note — "declare an action's load-bearing preconditions
as typed facts; existing exclusivity (per-subject / per-object) catches the
contradiction"; (2) a recorded FUTURE-PULL candidate — AI-advisory precondition-
discovery (reuse R457-459), built only if a consumer demonstrates the forcing is
worth a discovery pipeline. The measured locked-room needs no new code.
