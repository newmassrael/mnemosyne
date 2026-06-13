# `causes` edge class — findings (code 0, scratch, gitignored)

Date: 2026-06-13. Scope + pre-registration: `scope.md` (this dir). Method:
PoC discipline — semantic FIT + falsification, existing primitives only,
findings = SSOT. Live runs against `causes.atomic.json`.

## Verdict: REFUTE — do NOT build a new `causes` edge class

Every invariant a `causes` edge would assert decomposes into a mechanism that
already exists (or is the already-deferred rung-3). The one headline invariant
that looked net-new — "cause before effect" — was proven LIVE to false-positive
on flashbacks, which is exactly why the store already surfaces it as data, not a
gate.

## What was tested

An original 4-fact slice ("the funeral / the flashback"): a CAUSE fact
(`c-accident` — the accident that kills Edgar) told in a FLASHBACK at section
`s2-flashback`, and its EFFECT (`e-mourning` — the family mourns) told FIRST at
`s1-funeral`. Discourse order `s1 → s2 → s3 → s4`, so the effect is
canon-BEFORE its cause (a flashback). Plus a clean causal pair (`c-gift` at s3
→ `e-gratitude` at s4, cause before effect). The effect facts `pays_off` their
cause facts (causality modeled with the existing setup/payoff edge).

## Live result (the keystone)

`report-payoff-coverage --order` on the slice:

```
world `main`:
  paid: [ { setup: c-accident, payoffs: [e-mourning] }, { setup: c-gift, payoffs: [e-gratitude] } ]
  payoff_before_setup: [ { payoff: e-mourning, setup: c-accident } ]
```

The flashback (effect `e-mourning` @ s1 canon-before cause `c-accident` @ s2) is
surfaced as **`payoff_before_setup`** — a documented honesty COUNT, "legal
mystery/flashback structure, surfaced never gated" — and the pair is still
`paid` (credited). The clean pair (`c-gift` before `e-gratitude`) is NOT in
`payoff_before_setup`. So the store ALREADY detects "effect before cause in
discourse order" and DELIBERATELY does not gate it, because over discourse order
that is a legitimate flashback. A `causes` edge that GATED on cause-before-effect
would false-positive on exactly this.

## The decomposition (why every invariant is already covered)

1. **Temporal "cause before effect"** — over canon = DISCOURSE order (sec 7.3),
   effect-before-cause is a flashback → already surfaced as `payoff_before_setup`
   (count, never gated, live-proven above). Over STORY time, "cause-day ≤
   effect-day" is an INTERVAL rule — rung-1, already built (R490–R492). Either
   way, no new edge: {interval gate (story-time) | payoff_before_setup
   (discourse)} covers it.
2. **Acyclicity (causal paradox C↔E)** — real and flashback-immune, BUT the
   `SuccessionCycle` scan (R463, unit-tested) already detects cycles on directed
   fact→fact edges over `succession_ancestors`. A `causes` cycle is the identical
   scan; nothing new in the mechanism.
3. **Dangling cause (E holds where C does not)** — the precondition/affordance
   shape = depth-ladder rung 3 (cross-subject), already on the deferred list.

## The one steelman (and why it still does not FIRE)

A causal cycle among NON-succession facts (two facts that cause each other but
are not a belief-succession) would escape `SuccessionCycle` (it scans only
`supersedes_in_frame` edges). That is the only genuinely net-new thing a
`causes` edge + cycle scan would add. But: a "C causes E causes C" paradox is
rare, esoteric, and UNMEASURED — the scale-floor experiment's causal failures
were the locked-room access (rung-3 precondition) and the Surel self-contradiction
(exclusivity), never a causal cycle. No pull → YAGNI until a real causal-cycle
fault is observed.

## So: is 전후 인과 verifiable? YES — but not via a new edge

The STRUCTURE of causality is already deterministically checkable through the
mechanisms that exist:
- story-time precedence → the interval gate (built);
- discourse-time inversion → `payoff_before_setup` (surfaced, correctly not
  gated — flashbacks are legal);
- causal paradox → the succession-cycle scan;
- effect substantiated by a state-change → payoff substantiation (R485, built);
- effect without its cause → rung-3 precondition (the real, measured pull).

A new `causes` edge would mostly re-skin these. The honest causal-coherence
pull the scale-floor actually measured is **rung-3 cross-subject preconditions**
(the locked-room "범인 접근 불가"), not a `causes` edge.

## Honest boundary (the true ceiling, unchanged R476)

Whether a declared cause C is actually SUFFICIENT / plausible to produce E is
unbounded commonsense — never gateable at any depth. The store checks the
STRUCTURE of declared causality (precedence, acyclicity, substantiation,
preconditions), never its plausibility. That stays author/model territory.

## Recommendation

REFUTE the `causes` edge class. If causal-coherence is the goal, the
evidence-backed next candidate is **rung-3 cross-subject preconditions** — the
one the scale-floor measured and the one not yet covered. Re-running the same
"code-0 PoC → pull → design → build" loop on rung-3 is the productive path; a
`causes` edge is redundant substrate.
