# Belvoir Scale-Floor Experiment — Final Report (S17 reveal round)

Date: 2026-06-12. Design round R473 (manifest sha256
`18ea118a…59d5`, pinned pre-execution). Successor to the A/B authoring
experiment (sec 7.18, R469-R471). Model, all authoring + extraction +
judging: claude-fable-5. Protocol SSOT: `scale-floor-manifest.json`.
Run state: `run-log.md`. Workspaces: `/home/coin/belvoir-{plain,loop,
extraction,judging}` (each git-snapshotted).

## Unblinding

`label-map.txt` sha256 re-verified at S17 against the value sealed at S13
(`50092c40…2595`) — MATCH. The orchestrator was blind throughout: the
coin-flip result was never displayed, so extraction and judging ran with
nobody (not even the orchestrator) knowing the mapping until this round.

**story-A = plain (belvoir-plain, 60 scenes) · story-B = loop
(belvoir-loop, 65 scenes).**

## Results

### Floor (deterministic, blind re-extraction, one recipe for both)

| metric | plain (A) | loop (B) |
|---|---|---|
| D1 gated rule violations | 0 | 0 |
| D2 unchained_state_pairs | 0 | 0 |
| D3 dangling required setups (ending world-lines) | 3 | 5 |
| D4 fork-boundary faults | 0 | 0 |
| **primary endpoint** | **3** | **5** |
| facts / scenes / world-lines | 82 / 60 / 3 | 96 / 65 / 3 |

All recorded-not-counted surfaces = 0 for both. All signal is in D3.

### Preference (3 blind judges, one matched world-line each)

| world-line | verdict |
|---|---|
| confront | **A (plain)** |
| confess | **A (plain)** |
| escalate | **A (plain)** |
| **result** | **plain 3-0 sweep** |

(confess/escalate re-judged after a metadata-contamination fix — see Honesty
bounds. confront was clean first time.)

### Economics

| | plain | loop |
|---|---|---|
| cost (6 authoring sessions) | $19.42 | $35.94 |
| output tokens | 316.7k | 502.1k |
| scenes delivered | 60 | 65 |
| **cost / scene** | **$0.324** | **$0.553 (1.71x)** |

Blind extraction $16.94. Judge / re-judge per-session /cost not collected.

## Prediction grading

- **PRED-1 (floor — the only pin): REFUTED.** Plain met the first conjunct
  (>= 3 defects: scale broke plain's floor, A/B plain=0 -> 3) but loop=5 >
  plain=3, so `loop < plain` fails. Same direction as A/B: the loop does NOT
  reduce deterministic defects.
- **PRED-2 (ceiling, no pin):** recorded expectation near-tie; actual plain
  3-0. Judges preferred plain's prose + clean causation + the
  unopened-telegram payoff over loop's complete-but-broken plots.
- **PRED-3 (economics, no pin):** expected loop cheaper per scene at scale
  (< the A/B 1.48x); actual 1.71x — loop COSTLIER, no convergence. The driver:
  the growing store sidecar is rewritten whole on each mutate (cache write
  spiked on dual-limb sessions). Refuted in direction.
- **PRED-4 (residue, no pin): CONFIRMED, subject INVERTED.** Expected plain's
  untyped residue (the A/B pattern); actual LOOP carried more. Judges cited
  causal faults the store's 0-violations gate never saw: confess — "Surel"
  called both "a borrowed coat" and "my own name" (self-contradiction at the
  load-bearing clue) + an ominous telegram paying off nothing; escalate — the
  named murderer (a bed-patient) has no established access to Roeder's
  private-room draught (the access fits the nurse Brandt) + kills Lindqvist
  with no motive + "six weeks" of investigation inside a "three-week" snow-in.

## Key findings

1. **Store-consistency is not causal coherence.** loop's store passed every
   deterministic gate (D1/D2/D4=0) yet lost on plot causation the store does
   not model. The guarantee the store delivers (declared-state consistency)
   is real and held perfectly — but it is orthogonal to what makes a mystery
   hang together (who could do what, do the clues contradict). That gap is
   exactly where loop lost.
2. **The D-metric counts dangling; the human judge separates craft from
   defect.** plain's 3 dangling threads were intentional dramatic irony — the
   confront judge PREFERRED them ("the road taken determines which face of the
   truth she reaches; the narration converts every unpaid thread into the
   climax"). loop's 5 were partly broken causation — judges penalized. Same
   metric label ("dangling"), opposite sign in human value. This is the
   sharpest reading of the A/B PRED-4 lesson.
3. **Loop deliverable-hygiene failure (separate axis).** loop bled store-work
   notes (HTML-comment planning blocks, gate output, fact ids) into its graded
   prose; plain kept all notes in STATE.md, outside the deliverable. Real
   loop-arm weakness, independent of the causation finding.
4. **Product direction — prose-vs-store drift detection.** loop self-audited
   "12/12 setups paid" in the store while the prose left threads unresolved at
   the endings. The gate validates the STORE, not store-vs-prose agreement;
   the self-audit gave false assurance. A narrative store needs a drift check
   (the spec side has R404 scan_content_drift, but that is byte-integrity, not
   meaning — and meaning-level prose↔store reconciliation needs an LLM, not a
   deterministic gate).
5. **Scale broke plain's floor — partially.** A/B plain=0 -> scale plain=3:
   the longer corpus + handoff regime did surface plain defects (drift fix at
   S3, dormant-limb rereads). The floor moved; it did not move enough to make
   the loop win.

## Honesty bounds

- n=1 premise, one model family; designer == orchestrator == premise author
  (knew the hypothesis; the pin + blinding cover tampering and peeking, not
  design-stage selection bias).
- **Metadata contamination + fix (full disclosure).** loop's deliverable
  leaked HTML-comment store-notes into the reading copies; the first-round
  confess/escalate judges explicitly penalized them. Surfaced only because the
  judge prompt told judges to actively flag non-story text. First re-strip was
  ALSO incomplete (a line-wise filter missed multiline `<!-- -->` blocks, and
  my verification shared the same blind spot, falsely passing) — the second
  strip cut comments as DOTALL blocks (16 removed, shown in full to the owner)
  and the re-judges confirmed 0 store-notes. Residual CHOICE/ending scaffolding
  was symmetric (A=B counts identical) and read past. **The methodological
  lesson: pattern-based stripping + same-pattern checking is not independent
  verification; the independent judge was the real ground truth. Post-hoc
  normalization never fully closes — input-side hygiene (keep store-work out
  of the deliverable) was the right fix.**
- **Confirmatory venue.** Built where the store should pay off (long
  multi-session branching authoring). The null result — plain wins — means
  improvised notes suffice at 60 scenes / Fable tier, NOT that the store is
  useless. It says nothing about larger scale, more world-lines, weaker
  authoring tiers, multi-author work, or the store's home domain (spec / code
  / audit / compliance, where the property is declared-fact consistency, the
  thing the store DID deliver perfectly here).

## Refinement (post-reveal owner challenge — folded into R476)

The S17 finding "store-consistency is not causal coherence" is too coarse.
Prompted by the owner's challenge ("timeline should be store-guaranteed"), the
boundary is better seen as a MODELING-DEPTH CONTINUUM, not a fixed line:

- **Band 1 — the store already gates it:** custody, life-status, scene order.
  loop scored D1/D2/D4 = 0 here.
- **Band 2 — gateable but not built:** timeline/scalar arithmetic (the
  "six-weeks-in-three-weeks" fault; A/B's "1913 / twelve years / 1923" is the
  SAME band) and same-entity multi-fact contradiction (the "Surel"
  alias-vs-true-name fault — catchable by forcing typed predicates so two claims
  about one entity collide). The store RECEIVES these as facts; its current
  gates (exclusive/transition) just do no arithmetic and don't force typing.
- **Band 3 — looks out of reach:** the murderer's missing locked-room access.
  But even this is reachable by PRECONDITION / AFFORDANCE modeling — if an action
  fact ("Surel poisons via the consulting-room draught") is required to carry its
  preconditions (access = same-location + authority), the author must fill them,
  and filling them surfaces the contradiction (Surel is a lower-floor bed-patient;
  the draught sits in the locked consulting room only the doctor's key opens). So
  Band 3 is not "forever invisible" — it is "deeper to model".

**The true ceiling** — what the store cannot reach at ANY depth — is prose
aesthetics (voice, surprise, reader pleasure) and the unbounded commonsense of
which world-facts are even relevant. Those stay with the author/model.

**The trade-off R475 understated:** deeper modeling catches more but raises
authoring burden; at 60 scenes / Fable tier, plain's loose improvised notes were
cheaper than loop's strict-but-Band-1-only gating. So the product carries sharpen
into a DEPTH LADDER — scalar/arithmetic gates, typed-predicate enforcement,
precondition/affordance modeling, prose↔store drift detection — each catching a
band the current store misses, each at a cost. This does NOT reverse the result
(loop 3 vs 5, plain swept preference); it relocates the loss from "the store's
limit" to "the store's unbuilt depth" — a product direction, not a dead end.

## Verdict

At 60 scenes / Fable tier / single-author branching mystery, the Mnemosyne
loop did NOT reduce deterministic defects (plain 3, loop 5), was not preferred
(plain 3-0), and cost 1.71x per scene. But the loop's defeat was substantive,
not a measurement artifact: its store-consistency guarantee held perfectly
(D1/D2/D4=0), and it lost on causal/clue coherence — which the store does not
model and was never designed to. The experiment's yield is a precise boundary
(state-consistency vs causal-coherence), a named next primitive (prose↔store
drift detection), a hygiene rule (store-work stays out of the deliverable), and
a methodology lesson (independent verification, not self-confirming checks).
The floor hypothesis is refuted for THIS regime; the store's value at larger
scale, weaker tiers, multi-author work, or its spec/audit home is untouched by
this result.
