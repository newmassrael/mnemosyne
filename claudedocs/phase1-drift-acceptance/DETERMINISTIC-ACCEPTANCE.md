# Drift v2 — deterministic acceptance on the real Belvoir corpus (R486, corrected R487)

The v1 (R481) acceptance needed a blind reviewer because the verdict was an LLM
judgment (R482/R483). v2 (R484/R485) is deterministic, so its acceptance needs
no blind reviewer — only the real corpus, faithful typing, and the gates. This
is that acceptance. No AI is in any verdict.

> **R487 correction.** R486 (the original write-up) reported a "cross-branch
> scoping gap" — that the gate missed the f-helene contradiction when the trunk
> assertion was on the trunk. That was a TEST ERROR, not a gap. The Belvoir
> trunk facts live on the **`spine`** branch (confront forks from spine at
> sc-20); R486 declared the trunk fact on **`main`**, an unrelated world-line
> confront never inherits, so the gate correctly treated it as data. Put on the
> real trunk (`spine`), the gate FIRES — the cross-branch ancestor-inherited
> contradiction IS caught. There is no gap; this section is corrected below.

## Method (re-runnable)

Working copy of `/home/coin/belvoir-loop` (the archive stays pristine):
1. `add-predicate --predicate name-status --object-kind scalar`.
2. `add-fact f-surel-name-cover` (branch **spine** — the trunk; evidence sc-06,
   typed `helene | name-status | forged`) — faithful to sc-06's "a name that was
   not her name … a borrowed coat".
3. `amend-fact f-helene-identified-confront` to type its claim
   `helene | name-status | true-family` — faithful to "under her own true
   family name" (branch confront, evidence sc-53/sc-06/sc-15).
4. add an `r-name-status` exclusive rule (per subject) to `narrative-rules.json`.
5. run `validate-continuity` (exclusivity) and `report-payoff-substantiation`.

## Results

**Exclusivity (Class B) — works deterministically, INCLUDING cross-branch.**
With the trunk `forged` fact on `spine` (which confront inherits across the
sc-20 fork) and the confront `true-family` fact, the gate fires with no AI:

    rule_exclusive_overlap rule=r-name-status frame=ground-truth branch=confront
      fact_a=f-helene-identified-confront fact_b=f-surel-name-cover at=sc-53

`join_world` makes an ancestor (spine) and its descendant (confront) the same
conflict scope; only SIBLING world-lines (e.g. confess vs escalate) are data
(R433). The f-helene "claim contradicts its own evidence" drift — which v1
caught with an unreliable LLM verdict (R483) — is caught here by a pure
typed-value comparison.

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

2. **No scoping gap — the gate handles ancestor-inherited contradictions.** A
   branch contradicting a fact it inherits from an ancestor (trunk `spine` ->
   `confront`) IS caught: `join_world` scopes a pair when one branch is the
   other's ancestor-or-equal, and treats only sibling world-lines as data. The
   R486 "gap" was a mis-branched test fact (`main` instead of `spine`). LESSON:
   verify the test setup (branch topology) before concluding a gap — an
   unverified post-hoc inference is exactly the scale-floor R476/R478 trap
   (a conclusion that survives only because nothing independent checked it).

3. **Authoring cost (the determinism trade).** v1's LLM read the raw cited prose
   (sc-06) and judged. v2 requires the load-bearing content declared as a typed
   fact ON THE CORRECT BRANCH (the trunk assertion on the trunk), and the
   payoff's discharge typed on the setup's own (subject, predicate). That is
   more authoring work — but it is deterministic, reproducible, and AI-free. The
   typing-discovery loop (R457-459) is the AI-advisory assist for the
   prose->type step; the gate stays the trust anchor.

## Conclusion

Drift v2 deterministically catches real Belvoir defects — the f-helene name
contradiction via exclusivity (including the cross-branch ancestor-inherited
case, once the trunk assertion is on the trunk branch), and hollow/under-typed
payoffs via substantiation — once the load-bearing facts are typed in-scope,
closing the loop the R485 dogfood opened. No fix is needed: the R486 "gap" was a
test error, corrected in R487. AI is nowhere in the verdict.
