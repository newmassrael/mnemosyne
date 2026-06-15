# npc-dialogue-experiment/v2 — report (R552 design / R553 executed)

**Question (manifest):** does a TARGETED, scene-scoped RE-RENDER of only the v1 judge-
flagged weak scenes — a critique → re-render → re-gate stage after the v1 warm render —
RAISE craft without re-cooling (R513) or introducing a leak / off-branch drift (the R508
gates must still pass)? The owner's R551-followup: *can you fix only the deficient parts
rather than redo the whole render, and does it help?*

**Manifest pinned pre-execution:** sha256
`ada6bd39b2626e3dd7f8e15ec4a9ef14f662b2505bdfd77fc0a4df7ae9ff2ad5` (R552 ledger,
committed `d1c3cda` before any subagent ran). Firewall (R469, 12th use): this lineage
selected the weak set FROM the v1 verdicts, wrote the manifest / repair craft-note /
A/B brief / runbook, spliced the arms, ran the gates; a blind repair-renderer, a blind
re-extractor, and 3 blind A/B judges did the rest.

## RESULT = CONFIRMED (decision-rule branch: arm R preferred AND all pins hold)

The 4 judge-flagged weak scenes (report sc-16, confront sc-29, confront sc-30, shelter
sc-27) were re-rendered by a blind subagent and spliced into otherwise-byte-identical v1
manuscripts (arm R). **3 blind A/B judges, with arm→version randomized per judge,
unanimously preferred the REPAIRED version on ALL 4 scenes AND overall — 12/12 scene
choices + 3/3 overall, 3-0.** All three deterministic pins HOLD: the repair introduced
no leak, no drift, and stayed localized to the 4 scenes. The targeted critique →
re-render → re-gate stage is a VALIDATED render-improve loop.

## PIN-R3 — localization (HOLD, by construction + confirmed)

The orchestrator spliced the 4 blind-rendered scenes into copies of the v1 manuscripts.
Verified: in all 3 roads the scene-id sequence is identical to arm C, and the ONLY
changed scene blocks are exactly `report/sc-16`, `confront/sc-29`, `confront/sc-30`,
`shelter/sc-27`. Independently re-confirmed: the blind re-extraction found 66 spine
facts byte-identical to v1. Any judged difference is attributable to the 4 scenes alone.

## PIN-R1 — no new leak (HOLD)

Over a BLIND re-extraction of arm R (separate firewalled subagent, 159 facts from the
prose's own markers): `validate-disclosure-leak --telling holm --truth-frame gt`, every
world = **leaks=0**, `vocabulary_shared=13` (R510 F5 non-vacuous). Critically, the
shelter sc-27 dramatization (the riskiest repair — bringing the cast on-page at the
silence) did NOT voice the withheld murder, which stays unknown on the shelter road.

## PIN-R2 — no new drift (HOLD)

Over arm R's single-world projection (the v1/R512 method): `validate-render-fidelity`,
every world = **off_path=0, unplaced=0, reached_terminal=true**. The repair pulled in no
off-branch / cross-world fact.

## JUDGED — blind A/B, 3 judges (no pin)

Unblind (label-map): arm R = repair, arm C = v1 control.

| scene | judge-1 | judge-2 | judge-3 |
|---|---|---|---|
| report sc-16 | **R** | **R** | **R** |
| confront sc-29 | **R** | **R** | **R** |
| confront sc-30 | **R** | **R** | **R** |
| shelter sc-27 | **R** | **R** | **R** |
| **overall** | **R** | **R** | **R** |

The judges independently named the SAME reasons the repair won: arm R dramatizes the
unsaid (sc-16: the officer tests Halsa under pressure instead of announcing "a hanging
matter… the Holm's neck"); breaks the confession in real time across a hearth instead of
declaiming to the square (sc-29: "We were starving… the winter didn't count"); wrings the
murder out through live resistance and collapse (sc-30); and stages the shelter ending as
a lived curing-shed scene with a closing line for the boy ("I keep feared I'll forget
which telling's the true one") instead of a narrated roll-call of fates.

**The seam check cut FOR the repair, not against it.** Two judges, asked if any version
read "as if spliced in from a different hand," named **arm C's** shelter sc-27 (the
ORIGINAL summary) as the out-of-step one — "a character-by-character epilogue ledger
rather than a scene lived through." No judge flagged a repaired scene as foreign. The
manifest's seam-detection risk resolved in the repair's favour: the repaired scenes read
as belonging; the original's weakest scene was the odd one out.

**Decision rule (manifest, pre-committed):** arm R preferred by a majority AND PIN-R1/R2/
R3 hold ⇒ **targeted craft-repair RAISES completeness without re-cooling or breaking
fidelity ⇒ the critique → re-render → re-gate stage is a VALIDATED render-improve loop**
(the render analog of the author write-gate-read-repair loop). Met decisively (3-0). It
is a prompt/stage, not new substrate — build YAGNI-deferred, recipe recorded.

## Honest caveats + the one real residue (NOT downplayed — feedback_dont_downplay_experiment_flaws)

- **A repair-introduced in-frame grounding question, surfaced by the blind extractor, at
  the repaired confront sc-30.** The extractor flagged that Ysolt names the mate's murder
  by deducing from the missing key/bill — grounded — but "the prose never shows her
  learning that the mate came out of the water carrying the bill." De-orating the reveal
  into cut-and-thrust may have slightly thinned the on-page acquisition channel for one
  premise. The craft judges did NOT flag it (the deduction read as earned), and PIN-R2
  did not catch it (it is an IN-FRAME acquisition question, the R539 YAGNI boundary, not
  an off-branch claim). So the repair won the craft A/B but carries the same store-
  consistency ≠ causal-coherence residue v1 carried — a reminder that craft preference
  and knowledge-grounding are different axes, and the gate covers only the latter's
  cross-world form. This is the one place "patch the weak part" traded a faint new seam
  for a craft gain; honest, and judged-not-gated.
- **n=1** — one repair-render / extractor / 3 judges, one story.
- **The weak-set signal was uneven** (report sc-16 flagged by all 3 v1 judges; shelter
  sc-27 by only judge-1). The repair won all four anyway, including the thin-signal one —
  which strengthens "summary→dramatized is a reliable lift," but the shelter claim rests
  on a single original flag promoted to a clean A/B win.
- **The base already scored 5/5**, so the headroom was small — yet the repair still won
  3-0 blind. This both answers the small-headroom worry (it helped anyway) and bounds the
  claim (the larger value of the stage is on a LOWER-scoring base; untested here).
- **A/B judging was scene-paired** (the 4 scenes shown as pairs), not in full-manuscript
  context; voice-consistency-across-the-book was not re-judged here (v1 covered it 5/5).

## Through-line

The owner's hypothesis is confirmed on this instance: **you do not have to re-roll the
whole render to raise it — patch only the judge-flagged weak scenes, and the deterministic
gates prove the patch broke nothing (no leak, no drift, scoped), while blind A/B judges
confirm it reads better.** Targeted craft-repair is the render-side analog of the AI-first
author's write-gate-read-repair loop; the gate is the safety net that makes an automated
repair stage trustworthy. The one residue (an in-frame deduction grounding at sc-30) marks
the unchanged boundary: gates cover fidelity, not in-frame acquisition, and the blind judge
remains the backstop for the latter.

SSOT = this file + the R552 / R553 ledger entries.
