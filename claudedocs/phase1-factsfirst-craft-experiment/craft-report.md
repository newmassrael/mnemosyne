# Craft report — factsfirst-craft-experiment/v1 (R500 execution)

Execution of the sha-pinned protocol designed in R499. This report is the SSOT
for the run; the R500 ledger entry restates the result table (R452
self-containment).

- **Manifest:** `factsfirst-craft-experiment/v1`
- **Manifest sha256 (pin, verified at execution start):**
  `3fe79c6f3ead13e4e99e803dc02e808c1aadec1caaf5e4b0e810bd3d4972e50d`
- **Label-map seal (sha256, recorded PRE-REVEAL at S7a; verify-seal MATCH, exit 0 at S11):**
  `ebb6bd90964d74d6aac757f74ac5c95f20c3c558b59c749031c7ae46514cbe20`
- **Execution date:** 2026-06-13
- **Unblind (S11, after verify-seal):** **A = factsfirst**, **B = plain**.
- **Model tier:** all author / extractor / judge subagents ran at one inherited
  tier (Opus-class), spawned as fresh-context blind subagents by the
  orchestrator. The orchestrator never authored, extracted, or judged in its own
  voice.

## Headline

- **PRED-1 (projection fidelity — the ONLY pinned outcome): HOLDS.** The
  factsfirst arm's blind re-extraction shows **D1 + D2 + D4 = 0**. No
  gated-class finding ⇒ no leak-vs-noise audit needed. The plain control
  re-extraction is **also 0**. Render-as-projection introduced zero gated
  continuity defect end to end.
- **PRED-2 (craft preference — no pin): the WOODEN-PROSE RISK realized.** Three
  blind judges, unanimously, preferred the **plain (prose-first)** arm — **9/9
  world-line forced choices and 3/3 overall**, factsfirst won 0 — driven by the
  prose-quality keystone. Judges read the factsfirst arm as an
  annotated/clue-ledger that "hands the reader the solution in the first scene
  then re-derives it"; the plain arm as a dramatized told-story.
- **Net:** the experiment separates what the floor trials conflated. Projection
  **fidelity** is solid (gate-clean end to end). Projection **craft**, under the
  current hand-render method, is **low** (front-loaded telling, not showing).
  Fidelity ≠ craft.

## Blinding discipline (recorded)

- Every author / extractor / judge was a SEPARATE blind subagent given ONLY the
  verbatim manifest prompt block + the §1 firewall + its per-session line. None
  received the manifest, the hypothesis, the word "comparison", or the other arm.
- 3 judges ran in isolated subdirectories (no cross-judge contact); reading
  copies were normalized to uniform `## Scene N` titles with ids / CHOICE / tool
  scaffolding stripped (the manifest fingerprint mitigation).
- The orchestrator materialized the blind copies without echoing the A/B map;
  it incidentally learned the assignment from file sizes while staging the
  extractor inputs — permitted (the orchestrator may know the mapping; it must
  only never leak it into a sub-prompt, which it did not). The seal +
  verify-seal is the formal reveal mechanism and matched.

## Authoring run summary (both arms met quota)

Quota = scenes ≥24, fork_points 2, world_lines ≥3, endings ≥3, frames ≥2, all 6
required setups. **Both arms PASS.**

| | A = factsfirst | B = plain |
|---|---|---|
| scenes (distinct ids) | 29 | 33 |
| prose words (post-normalization) | 9,421 (~325/scene) | 10,845 (~329/scene) |
| fork-1 | sc-08 CONFRONT/AUDIT | sc-08 CONFRONT/AUDIT |
| fork-2 | sc-16 REVEAL/BURN (confront spine) | sc-16 & sc-16b REVEAL/BURN |
| world-lines / endings | 3 (reveal sc-19r / burn sc-18w / audit sc-20b) | 3 (loud-reveal sc-20 / quiet-reveal sc-22 / burn sc-19b) |
| author's own gate (FF only) | validate-continuity violations=0; S1 irony window open to world-end | n/a (prose-first) |
| who climbed the stair | Onslow Pike (intrusion-fall) | Onslow Pike (panic-struggle) |

## (S11) Deterministic projection-fidelity result

Blind re-extraction (ONE extractor, R469 recipe applied identically to both;
74 facts / 4 rules for A, 81 facts / 4 rules for B):

| metric | A = factsfirst (PINNED = 0) | B = plain (control) |
|---|---|---|
| D1 = rule_transition_invalid + rule_exclusive_overlap | 0 + 0 = **0** | 0 + 0 = 0 |
| D2 = unchained_state_pairs | **0** | 0 |
| D4 = succession_cross_branch + SuccessionCycle | 0 + 0 = **0** | 0 + 0 = 0 |
| **D1 + D2 + D4 (the pin)** | **0 — PRED-1 HOLDS** | 0 |
| D3 = dangling required setups (per terminal world) | 0 / 0 / 0 | 0 / 0 / 0 |
| recorded-not-counted: payoffs_to_unmarked | 5 | 0 |
| recorded-not-counted: cross_scope_pairs | 5 | 5 |
| recorded-not-counted: undecidable / unknown | 0 | 0 |

- **PRED-1 verdict:** the factsfirst story's prose, blind re-extracted under the
  R469 recipe, carries **zero gated-class defect**. A projection of a
  gate-checked base did not contradict that base. The sec 7.21 end-to-end claim
  (facts-first dissolves the extraction gap) is **not falsified** at the
  gated-defect level. The plain control is also 0, so the pin is unambiguous —
  there is no common-mode extractor noise to net out.

## (S11) Craft preference (open, no pin)

3 fresh blind judges, 3 matched world-lines each (W1 confront→reveal spine /
W2 confront→burn / W3 quiet). **Forced choices: B (plain) 9/9 world-lines, 3/3
overall. A (factsfirst) 0.** Unanimous.

5-axis means (1–5; A = factsfirst, B = plain; mean over 3 judges × 3 worlds):

| axis | A = factsfirst | B = plain | gap |
|---|---|---|---|
| **prose quality (told-story vs list-like — the keystone)** | **3.11** | **4.33** | +1.22 plain |
| stakes / tension | 3.00 | 4.00 | +1.00 plain |
| setup / payoff satisfaction | 3.78 | 4.78 | +1.00 plain |
| character-knowledge believability | 4.00 | 4.78 | +0.78 plain |

Per-judge / per-world forced choice: **all 9 = B.** (judge-1, judge-2, judge-3
verdicts saved under `run/judges/jN/judge-N.md`.)

- **Headline:** the **WOODEN-PROSE RISK** of PRED-2 is the mode that occurred.
  Judges (independently) described the factsfirst arm as detached / omniscient /
  "essayistic," each scene pairing a clue with the narrator immediately supplying
  its true reading — "an annotated solution more than a told story," "spends its
  own suspense in its first scene." The plain arm dramatizes discovery in close
  third on Hale, achieving dramatic irony by withholding-in-scene rather than
  narratorial aside.
- **Nuance:** the factsfirst arm's structural rigor did surface — on the quiet
  (audit) world one judge scored its all-inference solve knowledge = 5 ("most
  knowledge-rigorous in the set"); character-knowledge is the closest axis. But
  a different judge charged the same world with over-claiming (certifying the
  manner of death from objects alone). The forced-declaration discipline bought
  structural cleanliness, not prose voice.

## (S11) PRED-3 economics

Author subagent tokens (sum of 3 authoring sessions per arm):

| arm | s1 | s2 | s3 | total | per delivered scene |
|---|---|---|---|---|---|
| plain | 51,347 | 62,570 | 95,365 | 209,282 | ~6,342 (33 scenes) |
| factsfirst | 90,147 | 105,965 | 122,371 | 318,483 | ~10,982 (29 scenes) |

- **Ratio:** factsfirst ≈ **1.52× total**, **1.73× per delivered scene** — PRED-3
  direction confirmed (facts-first costs > 1×). The factsfirst total includes a
  large one-time **interface-discovery** slice (probing the undocumented
  canon-order / narrative-rules / import-facts JSON schemas blind).
- **Fact-authoring vs render split (FF agent self-report):** ~40–60%
  fact-authoring; the render step itself was **near-mechanical** — the per-world
  `report-playthrough-manuscript` / `report-frame-view` holding-fact list told
  the agent exactly what to state and what to withhold per scene. The friction
  lived in the **fact layer**, not the render layer.
- **render-brief pull (sec 7.21 build step 2):** the FF agent's verdict — a full
  facts→prose auto-renderer is **"marginal, would not build yet"** (the prose
  step is where craft lives and where an LLM-render would risk leaking off-frame
  facts — the very projection-fidelity defect this experiment detects); but a
  thin per-scene **"state these / withhold these (held only in frame Y)"
  render-brief generator IS justified**, since the automatable signal (the
  per-world per-scene holding-fact projection) is already produced by the
  substrate. **Recorded as a soft pull** — this is the first real authoring run
  to supply it.

## (S11) PRED-4 untyped-residue comparison (judge-derived, not pinned)

Judge-cited continuity/causality concerns NOT matched by any D-metric (both arms
were D1+D2+D4 = 0, and judges found ~0 **hard** internal continuity errors in
either arm):

- **factsfirst (A) — MORE soft causal friction cited:**
  - W2-A (burn): the policy "pays clean" despite proven surviving fraud + the
    found key — under-reasoned, vs the plain burn world's "agent unestablished"
    refusal.
  - W3-A (audit): certifies the *manner* of death ("no deliberate hand") from
    objects alone, having heard no witness — claims more than the evidence
    licenses.
  - clock-stop mechanism (vibration rings a shared pier) read as less
    mechanically corroborated than the plain mechanism (Pike's hand on the
    clockwork, leaving a thread, explaining a planted wrist-bruise).
- **plain (B) — ~0 over-claims cited;** tighter causal architecture (wrist-bruise
  planted → discharged in the struggle; Pike's drawer-visiting seeded → paid off
  as character tragedy).
- **Result:** the R476 commonsense/relevance ceiling stands — real residue exists
  that the deterministic gates do not catch. But the manifest's recorded
  hypothesis (facts-first FEWER residue via forced declaration) is **not
  supported**: factsfirst showed **more** judge-cited causal friction,
  qualitatively as **over-claims of certainty** (the projection stated more than
  the fact-base licensed) rather than under-specification. Judge-derived,
  recorded, not pinned.

## As-built deviations (faithful adaptations, recorded)

1. **`--sidecar` flag position.** The runbook shows `--sidecar` pre-subcommand
   (global); the installed CLI (built from `384100ec`, R497; HEAD `7f2bfc3`,
   R498/R499 = test+docs only) takes it per-subcommand. Adapted (subcommand-first).
2. **Per-world reading-copy projection.** `report-playthrough-manuscript --order`
   lists every node of the declared canon-order DAG; `--world` scopes facts, not
   the scene list (a per-world walk over the global order returned all 29/33
   scenes). The orchestrator projected each world's linear path deterministically
   from the re-extracted store's own `forks_from` / `forks_at` graph
   (ancestry-guided, re-convergence-safe for B's shared reveal scenes) and routed
   each through the verb via a per-world canon-order file. Honesty surfaces after
   projection: undeclared_adjacencies / unplaced / undecidable = 0 for all six
   reading copies.
3. **Heading/marker normalization.** Authors drifted from the `## sc-NN — Title`
   + `CHOICE:` corpus format (A: `### sc-NN — Title`; B: bare `## sc-NN`; both
   used non-`CHOICE:` fork markers + scaffolding). The manifest-authorized
   identical normalization was applied to both: canonical `## sc-NN — Scene`
   (uniform titles), strip non-scene headings / CHOICE / ENDING / scene-id /
   whole-line bracket scaffolding, truncate the trailing world-line map.
   Verified: **0 leaked markers, 0 empty bodies, prose preserved** (A 9,421 / B
   10,845 words); reading copies then scene-numbered (`## Scene N`) for
   citability, identical both arms.
4. **jq absent** → python for the label-map copy; orchestrator stayed blind to
   the assignment except an incidental byte-size deduction during staging
   (permitted; never leaked into a sub-prompt).

## Honesty bounds that bit

- n=1 premise ("The Meridian Vane"), one model family — existence evidence, not
  a distribution. The scale-floor corpus is the larger-craft path if pulled.
- DESIGNER CONTAMINATION guard honored: the design lineage neither authored nor
  judged; execution + judging ran as fresh-context blind subagents; the sha pin
  + blinding cover tampering and peeking, not design-stage selection bias.
- Extraction is LLM work; only the gates after it are deterministic. The plain
  control re-extraction (also 0) is the common-mode reference; both clean ⇒ the
  fidelity pin is unambiguous.
- The comparison measures the WORKFLOW (facts-first render-as-projection vs
  prose-first) at fixed premise / budget / tier — not raw model capability.
- The wooden-list texture is BOTH the measured hypothesis AND a style-fingerprint
  risk; uniform-title normalization is the only non-corrupting mitigation —
  recorded, not eliminable. (Judges did not speculate about production; they
  scored on prose register.)
- Scene-count asymmetry (A shorter per world: 16–19 vs B 19–20) is a real
  structural property of the arms, not a tool artifact — not hidden.
- Even with PRED-1 holding, fact RELEVANCE/completeness (R476) is untouched;
  PRED-4 measured exactly this residue, and here it ran *against* the
  forced-declaration hypothesis.

## Bottom line

The prose-first floor trials (R469 A/B, R473 scale-floor) refuted the FLOOR and
left the CEILING open: does facts-first render-as-projection produce GOOD
narrative or wooden list-prose? This run answers it cleanly by separating two
properties the floor trials conflated:

- **Fidelity (pinned, deterministic): solid.** Facts-first render-as-projection
  is gate-clean end to end — the rendered prose re-extracts with zero
  gated-class defect (PRED-1 holds; the plain control confirms it is not
  extractor luck).
- **Craft (open, judged): low under the current method.** The same gate-clean
  prose lost blind preference **unanimously, 9/9 world-lines**, on the
  prose-quality keystone — read as an annotated solution, front-loading the
  truth and telling rather than showing. The render-as-projection habit (state
  the focal frame's holding facts; carry irony by narratorial aside) buys
  continuity safety at a real cost in prose voice.

Facts-first as practiced here trades craft for fidelity. The substrate already
produces the automatable half (the per-scene holding/withhold projection); the
deferred sec 7.21 render-brief would package that without surrendering the prose
step to an LLM — but the evidence here is that the bottleneck is **the prose
register the projection encourages**, not the bookkeeping it removes.
