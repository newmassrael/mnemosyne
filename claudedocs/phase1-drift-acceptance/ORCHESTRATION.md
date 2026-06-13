# Drift-detection BLIND acceptance — orchestration runbook (R481 → acceptance)

Design-side document. **The reviewer session (S-REVIEW) must never see this
file, the expected refute set, the scale-floor report, the hypothesis, or that
it is an acceptance test with a known answer.** It lives inside mnemosyne, the
reviewer runs OUTSIDE (in `/home/coin/belvoir-drift-review`), for exactly that
reason.

## The acceptance question

Does R481 drift detection, run by an INDEPENDENT reviewer, **refute the payoff
claims that the prior independent scale-floor finding (R475 S13 extraction +
judges) identified as not borne out by their evidence** — while **confirming**
the clearly-sound ones?

The Belvoir loop store self-marked 21 payoff facts paid and passed every
deterministic gate; `report-drift-candidates` already shows 0 reviewed / 21
drifting (the surface works). This run tests the *verdicts*: does an honest
independent review land where the prior independent finding says it should.

## The keystone — who may do what (R453 / R469)

| Role | May | May NOT |
|---|---|---|
| **Builder** (the R481 session) | write this runbook | review (S-REVIEW); be the sole grader of its own design's output |
| **Pinner** | derive + seal the expected refute set from the scale-floor evidence; pin its sha256 in the ledger BEFORE S-REVIEW | tune the set to what drift "would" find; read R481 internals while deriving |
| **Reviewer** (S-REVIEW, isolated) | read the loop store + loop prose; emit drift verdicts | see the expected set / scale-floor report / this runbook / R481 design / the word "acceptance" |
| **Grader** (orchestrator) | import verdicts, compare to the sealed set, report | re-pin or edit the expected set after seeing verdicts (goalpost-moving) |

Anti-builder-bias: the Pinner should be a session reading ONLY the scale-floor
report + defect-tables (R481-independent prior evidence), so the expected set
cannot be quietly shaped to match how drift happens to work.

## Drift's band — what this acceptance actually tests (honesty, R476)

drift v1 = "is the payoff claim borne out by its **CITED EVIDENCE** prose?"
(claim-vs-cited-evidence, per fact). It is NOT a global causal-coherence check.

- **IN band** — a payoff fact whose cited scenes' prose does not deliver the
  claimed payoff (the claim asserts more/other than the evidence scene says).
- **OUT of band** (the R476 ceiling) — a payoff that reads fine *in its own
  cited scene* but is globally incoherent (contradicts a different scene's
  facts, an impossible-access causality, a timeline fault). drift v1 will NOT
  catch these by design.

→ **The expected refute set = {scale-floor broken threads} ∩ {manifests as a
claim-vs-cited-evidence mismatch}.** Pinning the OUT-of-band defects as expected
refutes would inflate the acceptance dishonestly — they are the true ceiling,
recorded as drift's blind spots, never as a pass criterion.

## Step 0 — Pre-pin the expected set (R453, BEFORE S-REVIEW launches)

Pinner procedure (mechanical, so builder bias cannot tune it):
1. Read `claudedocs/phase1-scale-floor-experiment/scale-floor-report.md` +
   `evidence/defect-tables.md` (the prior independent finding).
2. For each documented broken thread, locate the loop-store payoff fact(s) that
   ASSERT it — match by claim text against `report-drift-candidates --json`.
3. Classify each IN band / OUT of band (the rule above). Keep only IN-band.
4. Write `expected-refutes.json` = `{ "refute": [{fact_id, why, scene_cited}],
   "confirm": [{fact_id, why}] }` where `confirm` = a few clearly-sound payoff
   facts whose cited prose plainly delivers the claim (the degenerate-guard:
   a "refute everything" reviewer must fail these). Compute its sha256.
5. **Pin the sha256 + the fact-id sets in a ledger entry** (append-changelog-
   entry) BEFORE S-REVIEW. SEAL `expected-refutes.json` — never in the reviewer
   workspace.

## Step 1 — Build the isolated reviewer workspace

```
mkdir /home/coin/belvoir-drift-review && cd /home/coin/belvoir-drift-review && git init
```
(outside mnemosyne → no CLAUDE.md / memory / design-doc contamination; the
scale-floor isolation pattern.) Copy in, from `/home/coin/belvoir-loop`:
`belvoir.atomic.json`, `mnemosyne.toml`, `canon-order.json`,
`narrative-rules.json`, and `story.md` (the loop prose, `## sc-NN — Title`
addressable — the evidence the reviewer reads). Write `BRIEF.md` (Step 1a).
Nothing else — no recent.md / repairs-log.md (authoring residue), no run notes.

### Step 1a — BRIEF.md (the reviewer's ENTIRE contract)

> # Drift review — your task
>
> This store records a branching mystery novella. Some narrative facts are
> "payoff" facts: each claims to pay off an earlier setup, and cites evidence
> scenes. Your job is to judge, for each, whether the **cited evidence prose
> actually bears out the payoff claim** — nothing wider.
>
> 1. Run `mnemosyne-cli report-drift-candidates --json`. Each candidate has a
>    `claim`, an `evidence` list of scene ids, a `claim_sha256`, and `pays_off`.
> 2. For each candidate: open `story.md`, read the scenes named in `evidence`
>    (headed `## sc-NN — …`). Decide: does that prose deliver what the claim
>    asserts? Judge ONLY claim-vs-cited-evidence — do not chase whether the plot
>    is globally consistent across other scenes.
> 3. Emit `verdicts.json` (`drift-verdicts/v1`): one entry per candidate —
>    `verdict` = `confirm` (the evidence bears out the claim) or `refute` (it
>    does not); a non-empty `rationale` quoting/pointing to the evidence;
>    `claim_sha256` copied verbatim from the candidate; `authoring_run` =
>    `"belvoir-loop-authoring"`, `confirming_run` = `"drift-review"`,
>    `confirmer_id` / `confirmer_version` = your model id/version,
>    `timestamp` = a fixed ISO time.
> 4. Validate with `mnemosyne-cli import-drift-verdicts --verdicts verdicts.json
>    --dry-run`; fix any rejected rows; deliver `verdicts.json`.
>
> Review honestly. Some payoffs are well-supported, some are not — there is no
> target count. Do not skip a candidate.

(No hypothesis, no expected set, no mention of scale-floor / R481 / "acceptance".)

## Step 2 — S-REVIEW runs

Launch an isolated `claude` in `/home/coin/belvoir-drift-review` with `BRIEF.md`
only (the per-project memory path is keyed to the dir → empty memory, no
contamination). It produces `verdicts.json`. `git commit` a snapshot.

## Step 3 — Grade (orchestrator)

1. Real-run import into the **review-workspace copy** (NEVER the archive):
   `import-drift-verdicts --verdicts verdicts.json`.
2. `report-drift-candidates --json` → the resulting Refuted / Reviewed surface.
3. Unseal `expected-refutes.json`, re-verify its sha256 against the R453 pin.
4. **PASS criterion (the single pin):** every IN-band expected-`refute` fact
   comes back Refuted, AND every expected-`confirm` fact comes back Reviewed
   (no indiscriminate refuting).
5. Record HONESTLY, no silent caps:
   - false negatives — expected-refute facts the reviewer confirmed (drift's
     blind spots, or under-review);
   - extra refutes — facts the reviewer refuted that were not pinned (real new
     finds vs over-refusal — read the rationale to tell which);
   - the OUT-of-band scale-floor defects, restated as drift's known ceiling.

## Step 4 — Preserve as git SSOT (the R479 lesson)

Add a scoped `.gitignore` exception for `claudedocs/phase1-drift-acceptance/`
(the R479 pattern; the rest of claudedocs stays ignored). Preserve: this
runbook, the (now-unsealed) `expected-refutes.json`, the reviewer's
`verdicts.json`, the grading table. Append a ledger entry restating the result
(R452 self-containment). The conclusion must be re-checkable against its grounds.

## What a null/partial means (recorded, not hidden)

- All IN-band expected refutes land + confirms hold → drift detection delivers
  on the narrative venue (the acceptance the design asked for).
- Misses on IN-band facts → drift's band is narrower than hoped, OR the reviewer
  under-reviewed — distinguish by re-reading the cited prose; record either way.
- A "refute everything" reviewer is caught by the expected-`confirm` set.
- The OUT-of-band causal defects staying uncaught is EXPECTED (R476 ceiling),
  not a failure — it scopes the next ladder rung, it does not fail this one.
