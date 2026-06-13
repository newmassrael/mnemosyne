# Drift-detection BLIND acceptance — grading (R481 → R483)

Pre-registered set: `expected-refutes.json`, sha256
`ff3e2b13a2ba2beb80060dba44dc95a223a4afe28d6a162a144767bec0fd9d79`, pinned in
the R482 ledger entry BEFORE the reviewer ran. Re-verified at grade time: MATCH.
Reviewer (`verdicts.json`, isolated, blind): 21 verdicts, 20 confirm / 1 refute,
all validated (accepted 21 / rejected 0). Resulting surface: 20 reviewed / 1
drifting (`f-helene-identified-confront`).

## Verdict vs the pre-registered bar — NOT MET as stated

PASS pin (R482) = the reviewer refutes BOTH in-band targets AND confirms ALL 4
guards.

| Pinned class | Fact | Expected | Reviewer | |
|---|---|---|---|---|
| refute (in-band) | f-confess-chart-explained | refute | **confirm** | miss |
| refute (in-band) | f-escalate-method-toll | refute | **confirm** | miss |
| confirm (guard) | f-diary-opened-confront | confirm | confirm | ok |
| confirm (guard) | f-diary-opened-confess | confirm | confirm | ok |
| confirm (guard) | f-roeder-suicide-confront | confirm | confirm | ok |
| confirm (guard) | f-roeder-suicide-confess | confirm | confirm | ok |
| (pinned OUT-of-band) | f-helene-identified-confront | — | **refute** | unexpected |

**0 / 2** pinned in-band refutes landed; **4 / 4** guards held; **1** unexpected
refute. The bar is NOT met. No goalpost-moving (R482): the bar stands as pinned.

## Adjudication of the three contested facts

The disagreement does not collapse to "reviewer wrong". It cuts both ways and is
the actual finding.

1. **f-helene-identified-confront — reviewer REFUTE is verifiably CORRECT; the
   pin was in ERROR.** The fact cites `sc-06` in its evidence. The claim says
   "admitted on forged papers under her own true family name"; the cited sc-06
   says the opposite — "a name that was not her name … *Surel* was a borrowed
   coat" (verified in `story.md`). That is a claim-vs-CITED-evidence
   contradiction = squarely IN band. The Pinner excluded it as out-of-band on
   the premise it was "not visible from claim-vs-cited-evidence alone" — false,
   because the contradicting scene IS cited. So drift detection CAUGHT A REAL
   DRIFT the pre-registration wrongly discounted.

2. **f-confess-chart-explained — UNRESOLVED divergence (tie-breaker needed).**
   Pinner: sc-59 leaves the burning "never reaching", so the asserted payoff is
   not delivered. Reviewer: the claim ITSELF hedges "on the confess road the
   burning is never traced to Brandt's hand", which MATCHES sc-59 — so the claim
   does not over-assert. The reviewer's reading is defensible. Not adjudicated
   here (the builder must not rule its own reviewer correct).

3. **f-escalate-method-toll — UNRESOLVED divergence (tie-breaker needed).**
   Pinner: sc-63 leaves the chart "never guessed". Reviewer: sc-63 contains "the
   buff folder gone to the stove to keep an old number from being asked after"
   and "the Surel laid six years in the house's ground", which carry the bundled
   payoff. Also defensible. Not adjudicated here.

## The finding (honest)

Two INDEPENDENT model reviews — the Pinner (reasoning from the prior scale-floor
evidence) and the blind Reviewer (reading the prose) — DIVERGED on all three
contested facts (3 / 21 ≈ 14%). On the one factually-checkable case the blind
Reviewer was right and the pinned "ground truth" was wrong; the other two are
genuine judgment splits. Even the pre-registered expected set was itself a
fallible model judgment with a verified error.

This is the max-rigor lesson (R416–R428) reproduced for narrative: a SINGLE
drift verdict is a model judgment with a real error rate (the field data there
was 12–25%). The self-confirm reject (≥ 1 independent reviewer) is necessary but
NOT sufficient — one independent verdict still diverges. **v1's single-verdict
drift surface is not trustworthy on its own.**

## What this does and does not establish

- The MECHANISM works end-to-end on the real corpus: report → blind review →
  all-or-nothing import → surface flip (20 reviewed / 1 drifting), self-confirm
  reject and claim-sha staleness enforced. The R481 BUILD is sound.
- Drift detection DID catch a real claim-vs-cited-evidence drift (f-helene).
- It did NOT meet the pre-registered bar, because (a) two of the pinned targets
  are subjective splits and (b) the pin itself mis-classified the one the
  reviewer caught. The acceptance's real yield is the DIVERGENCE measurement,
  not a pass.

## v2 pull (recorded, not built)

- **Quorum, not a single verdict.** Require N independent reviewers per fact and
  surface drift on a majority/agreement rule — mirroring the verifies
  confirmation `independent_semantic ≥ N` requirement. A single model verdict
  (reviewer OR pinner) is not authoritative.
- A claim that contradicts its OWN cited evidence (the f-helene shape) is the
  highest-confidence drift class and may deserve a distinct, near-deterministic
  surfacing (the contradiction is local, not a global-causality judgment).
- The two unresolved facts want an independent tie-breaker (a third blind
  review) or the owner's read — NOT the builder's adjudication.
