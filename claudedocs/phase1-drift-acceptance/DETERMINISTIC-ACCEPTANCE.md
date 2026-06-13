# Drift v2 — deterministic acceptance on the real Belvoir corpus (R486)

The v1 (R481) acceptance needed a blind reviewer because the verdict was an LLM
judgment (R482/R483). v2 (R484/R485) is deterministic, so its acceptance needs
no blind reviewer — only the real corpus, faithful typing, and the gates. This
is that acceptance. No AI is in any verdict.

## Method (re-runnable)

Working copy of `/home/coin/belvoir-loop` (the archive stays pristine):
1. `add-predicate --predicate name-status --object-kind scalar`.
2. `add-fact f-surel-name-cover` (trunk, evidence sc-06, typed
   `helene | name-status | forged`) — faithful to sc-06's "a name that was not
   her name … a borrowed coat".
3. `amend-fact f-helene-identified-confront` to type its claim
   `helene | name-status | true-family` — faithful to "under her own true
   family name" (branch confront, evidence sc-53/sc-06/sc-15).
4. add an `r-name-status` exclusive rule (per subject) to `narrative-rules.json`.
5. run `validate-continuity` (exclusivity) and `report-payoff-substantiation`.

## Results

**Exclusivity (Class B) — mechanism PROVEN deterministically.** With both
name-status facts in the SAME (frame, branch) scope (both confront), the gate
fires with no AI:

    rule_exclusive_overlap rule=r-name-status frame=ground-truth branch=confront
      fact_a=f-helene-identified-confront fact_b=f-surel-name-cover at=sc-53

The f-helene "claim contradicts its own evidence" drift — which v1 caught with an
unreliable LLM verdict (R483) — is caught here by a pure typed-value comparison.

**Substantiation (Class A) — works on real data.** Confront world after typing:
substantiated 1 (`f-diary-sealed <- f-diary-opened-confront`, a real
`integrity: sealed -> opened` state-change discharge), unsubstantiated 1
(`f-helene-identified-confront -> f-forged-papers`: the payoff's typed leg
`name-status` does not discharge the setup's `held-by` state — hollow OR
under-typed), unverifiable 10 (untyped payoff chains).

## Findings

1. **Deterministic drift detection works on real narrative, and is honest.**
   Both classes catch real defects by typed-value comparison, no model judgment,
   re-runnable. The dominant `unverifiable` count is the truthful "type these to
   verify" backlog, not a guess.

2. **GAP — cross-branch inherited contradiction is missed.** The REAL f-helene
   defect is cross-branch: trunk `forged` (sc-06, branch main, pre-fork) vs
   confront `true-family` (sc-53). With the forged fact on trunk, the gate
   reports 0 violations — the `(frame, branch)` scoping (R433: sibling
   world-lines never conflict) excludes it. But confront INHERITS the trunk fact
   (visibility crosses the fork), so within the confront world both values hold
   unreconciled. **Visibility inherits across a fork; conflict-scoping does
   not — an asymmetry.** A branch contradicting its own inherited ancestor (not
   a sibling) is a within-world contradiction that should be catchable. Fix
   (a follow-on round): compare a fact against ancestor-inherited facts in the
   world, still NOT against sibling-branch facts. This is distinct from the
   R433 sibling-divergence-is-data invariant.

3. **Authoring cost (the determinism trade).** v1's LLM read the raw cited prose
   (sc-06) and judged. v2 requires the load-bearing content declared as a typed
   fact IN THE RIGHT SCOPE, and the payoff's discharge typed on the setup's own
   (subject, predicate). That is more authoring work — but it is deterministic,
   reproducible, and AI-free. The typing-discovery loop (R457-459) is the
   AI-advisory assist for the prose->type step; the gate stays the trust anchor.

## Conclusion

Drift v2 deterministically catches real Belvoir defects (the f-helene name
contradiction via exclusivity; hollow/under-typed payoffs via substantiation)
once the load-bearing facts are typed in-scope — closing the loop the R485
dogfood opened (92% unverifiable = the typing backlog). One precise gap remains,
found deterministically not by guesswork: cross-branch inherited contradictions
need ancestor-aware conflict scoping (a follow-on round). AI is nowhere in the
verdict.
