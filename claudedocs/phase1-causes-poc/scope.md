# `causes` edge class — PoC scope (code 0, scratch, gitignored)

Date: 2026-06-13. Method: PoC discipline (exposure-poc / Detroit R445 class
— semantic FIT + falsification, existing primitives only, findings = SSOT,
scratch store NOT committed).

## Provenance

Owner asked (this session): "전후 인과관계도 검증 가능해?" — can before/after
causal relationships be verified? Candidate mechanism from the depth-ladder
discussion: a recorded `causes` edge (fact C causes fact E), deterministically
scannable like the existing `conflicts_with` / `pays_off` / `supersedes_in_frame`
edges. This PoC tests whether such an edge is a REAL new mechanism or
decomposes into what already exists.

## The two questions

- **Q1 — substrate FIT** (analytic, below): does the recorded fact→fact edge
  model + canon order + the succession-cycle scan provide the skeleton a
  `causes` edge needs?
- **Q2 — pull AND distinctness** (the keystone falsification): does a `causes`
  edge catch a causal fault that NO existing mechanism covers, gateable
  without false-positiving legitimate structure?

## Q1 — substrate FIT (analytic, DONE): FITs strongly, ~0 new substrate

The recorded-edge model already exists three times over:
- `conflicts_with` (symmetric contradiction) — target-existence + frame-scope
  + canon-overlap checks (`ConflictTargetMissing`, frame-scoped).
- `pays_off` (setup→payoff) — directional fact→fact, per-world credited, with
  honesty counts including **`payoff_before_setup`**.
- `supersedes_in_frame` (succession) — directional, with **`SuccessionCycle`**
  acyclicity detection (R463) over `succession_ancestors`.

A `causes` edge is the SAME shape: a directional fact→fact ref with
target-existence + frame/world scope + (cause-before-effect) + acyclicity. The
build cost would be near-zero — but that is exactly why the question is
distinctness, not feasibility.

## Q2 — the decomposition test (the keystone)

A `causes` edge would assert three invariants. Each is tested against what
already exists:

1. **Temporal "cause before effect"** — over the store's canon order, which is
   DISCOURSE order (sec 7.3: canon = the chapters' discourse sequence, NOT
   story time). Effect-before-cause in DISCOURSE order is a **flashback** —
   legitimate. The store ALREADY surfaces it: `payoff_before_setup` is a
   documented honesty COUNT, "legal mystery/flashback structure, surfaced
   never gated." So a `causes` temporal GATE over discourse order would
   false-positive on every flashback. Over STORY time, "cause-day ≤ effect-day"
   is an INTERVAL rule (rung-1, already built R490) — not a new edge.
   → the temporal half collapses into {interval gate (story-time) |
   payoff_before_setup count (discourse)}.
2. **Acyclicity (causal paradox: C causes E causes C)** — real, gateable, and
   flashback-immune (a cycle is wrong in any time model). BUT it reuses the
   `SuccessionCycle` scan (R463): a `causes` edge feeding the same cycle
   detector is a thin extension, not a new mechanism.
3. **Dangling cause (E holds in a world where C does not)** — this is the
   precondition/affordance shape (depth-ladder rung 3): the effect requires its
   cause to be visible. Overlaps the deferred cross-subject rung-3.

## Surface-not-gate shape (if anything is built)

- cycle → gateable (a paradox is always wrong, like `SuccessionCycle`).
- temporal → surface only (flashback-legitimate; the existing
  `payoff_before_setup` count is the right model).
- dangling → rung-3 territory.

## Pre-registered FIRE vs REFUTE

This is a FIT/falsification PoC (the planted faults are mine — it tests the
MECHANISM and the decomposition, not a blind detection rate).

- **FIRE** (justify a NEW `causes` edge class) iff the slice exhibits a causal
  fault that (i) NO existing mechanism (pays_off count, interval gate,
  succession-cycle, rung-3) covers, AND (ii) is gateable WITHOUT
  false-positiving flashback/mystery structure, AND (iii) branching amplifies it.
- **REFUTE** (do NOT build a new edge) iff every causal fault decomposes into
  {interval gate (story-time temporal) | payoff_before_setup (discourse
  temporal, surfaced) | succession-cycle (acyclicity) | rung-3 (dangling)} —
  i.e., `causes` is redundant; OR the only net-new gate (temporal over
  discourse) false-positives on flashbacks.

## What the slice demonstrates (encode next)

- A **flashback**: a payoff (effect) canon-BEFORE its setup (cause). Expect
  `payoff_coverage` to report `payoff_before_setup` — the temporal inversion
  surfaced as legal data, NOT a violation. (Live proof the temporal half is
  already handled-as-data and deliberately not gated.)
- A **clean causal pair** (cause before effect) — no `payoff_before_setup`,
  the contrast.
- **Acyclicity** is demonstrated analytically: `SuccessionCycle` (R463) already
  gates cycles on succession edges and is unit-tested; a `causes` cycle is the
  identical scan. (The `causes` field does not exist, so it cannot be stored
  via the CLI — the edge is represented in this doc, not the store.)

## Honest boundary

- Self-plant: the PoC tests the decomposition + the flashback false-positive,
  not field prevalence.
- The true ceiling (R476) stands: whether a declared cause C is actually
  SUFFICIENT/plausible to produce E is unbounded commonsense — never gateable.
  The store checks the STRUCTURE of declared causality, never its plausibility.
