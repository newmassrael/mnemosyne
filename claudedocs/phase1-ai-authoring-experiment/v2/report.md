# ai-authoring-experiment/v2 — the SCALE axis — report (R524 execution)

**Headline:** at **70 scenes** — above the 60-scene scale-floor where the
prose-first authoring loop LOST — a blind AI authored a gate-clean branching story
from a premise via its own top-down write→gate→read→repair loop, **BOTH
deterministic pins HELD** (orchestrator-verified, independent of the author's
claim), and **3 blind judges scored it unanimous 5/5 overall** with coherence 5/5/5.
By the pre-committed decision rule, **premise→self-authored coherence HOLDS above
the 60-scene floor → AI-self-authoring SCALES.** The scale-floor counter-evidence
("store-consistency ≠ causal coherence") did NOT reproduce at scale. The manifest
was sha-pinned pre-execution (R523, commit `e2b9e52`, sha256 `80e09fa0…`).

## What ran (firewall held)

premise → ONE blind author subagent (fresh context; only premise.md +
author-brief.md) → the orchestrator REBUILT the store fresh from the author's
manifests and independently ran the pins + assembled neutral per-world manuscripts
→ 3 blind judge subagents (fresh context; only judge-brief.md + the manuscripts) →
the orchestrator applied the decision rule. The orchestrator (this R518–R523
lineage) wrote the manifest + premise + briefs + the R522 gate and ran the
deterministic gates but authored no fact and judged nothing (the R469 contamination
bound).

## The authored story (neutral)

"Cold Signal at Vantor Pass" — 70 scenes, 85 facts, 3 frames (gt / company-belief /
town-belief), 7 branches. A snowbound mountain railway junction; a deadly blast
during a rockslide; an impossible log timestamp and a missing hold-warning. Ground
truth (held whole by neither belief frame): Stationmaster Halloran back-dated a
verbal authorization out of an old habit (forgery, not murder) and never knew a
warning arrived; day-operator Pike suppressed the warning that did arrive and is
gone down the valley. The company believes "authorized / act of God"; the town
believes "murder." Structure: a primary fork at sc-20 (expose / hold / act) and two
forks-off-a-fork (sc-26 → expose-recant / expose-deny; sc-58 → act-dig / act-flee)
= **5 terminal world-lines**. The author ran a clean Phase-0 skeleton (24 tentpole
facts: every ending, the 6 load-bearing spine setups, the forks, both belief
frames) then **4 Phase-1 write→gate→repair iterations** to gate-clean
(author-log.md).

## PIN-A1 — convergence to gate-clean AT SCALE — **HELD** (orchestrator-verified)

Rebuilt fresh from `sections.json` + `facts.json` into an empty seed (independent of
the author's `store.atomic.json`):
- **Import:** 0 errors; **70 sections + 85 facts** reproduced; 7 branches + 3 frames
  registered.
- **validate-continuity** `--order`: `violations: 0 (structural=0 interval=0)`. The
  structural count INCLUDES the R522 `EvidenceUnreachable` check — every structural
  backreference resolves on its own world-line.
- **Per-world placement** (all 7 branches): each `unplaced=0, undecidable=0,
  undeclared adjacencies=0`. `outside order` > 0 (the other branches' scenes
  correctly excluded) — the expected, non-defect signal (the R520 over-spec
  correction).

## PIN-A2 — structural completeness floor AT SCALE — **HELD** (orchestrator-verified)

- **report-payoff-coverage** `--order`: every TERMINAL world `paid=6 dangling=0`
  (expose-recant / expose-deny / hold / act-dig / act-flee). The intermediate
  branches (expose, act) dangle by design — the 6 setups are on the spine and pay
  off in their terminal descendants (the v1 main-stub pattern).
- **report-payoff-substantiation** `--order`: **6/6 substantiated** in every
  terminal (0 unsubstantiated, 0 unverifiable) — each spine setup has a dedicated
  payoff carrying the matching typed subject+predicate leg.
- **report-fork-tree** `--order`: 7 world-lines, **0 unplaced fork points**, **3
  forks** (sc-20 with 3 children; sc-26 with 2; sc-58 with 2 — each ≥ 2), **5
  terminals** (in the 4–6 band), every world-line terminal.
- **report-timeline-gaps** `--world W` per terminal: 0.
- **Off-branch family:** `validate-continuity` total violations = 0 — so
  `succession_cross_branch = 0`, `fact_canon_off_branch = 0`, AND
  `evidence_unreachable = 0`.

## R522 evidence-reachability gate — FIELD-VALIDATED at scale (bonus, not a pin)

The R522 gate never flagged the author's real store (0 `evidence_unreachable`): all
**66 backref-bearing facts (43 long-range**, branch facts citing the planted spine
setups) resolve at-or-before the citing fact in their own world-line, because the
author placed every shared setup on the spine before the first fork (the R521
top-down discipline). To prove the clean pass was non-vacuous, the author ran a
deliberate NEGATIVE CONTROL on a throwaway store — an `expose-recant` fact citing
`sc-33` (a scene only on the sibling `expose-deny` branch) — and the gate emitted
exactly `{"kind":"evidence_unreachable","fact":"f-probe-off","branch":
"expose-recant","evidence":"sc-33"}`. **Piece B fires at scale, non-vacuously**:
R520's case-2 cross-branch allusion is now structurally prevented (the contract) AND
caught (the gate).

## Causal coherence — JUDGED (the scale-floor "store-consistency ≠ causal coherence" check)

3 blind judges, R500 5-axis rubric adapted to story-logic, over the 5 terminal
manuscripts:

| | coherence | completeness | relevance | branch | **overall** |
|---|---|---|---|---|---|
| Judge 1 | 5 | 4 | 5 | 5 | **5** |
| Judge 2 | 5 | 5 | 5 | 5 | **5** |
| Judge 3 | 5 | 4 | 5 | 5 | **5** |

Unanimous **overall 5/5**, coherence 5/5, relevance 5/5, branch-integrity 5/5;
completeness 5/4/4 (mean 4.33). All three independently rendered the decisive
verdict: **"a coherent authored story, not an internally-consistent pile of
facts."** Each named the same architectural strength unprompted: the spine plants
the load-bearing facts and **each of the 5 endings cashes in a DIFFERENT subset** —
the warning is recovered in exactly the paths where the chosen action would recover
it and left lost where it would not; the branches are causally independent, not
cosmetically forked. **At 70 scenes, store-consistency AND causal coherence BOTH
hold — the scale-floor "store-consistency ≠ causal coherence" did NOT reproduce at
4–5× the v1 scale.**

**The one shared blemish is the R476 ceiling, surfacing where R518 predicted.** All
three judges flagged the SAME single soft spot: in `act-dig`, the surviving
track-worker found in the air pocket (sc-62) is the load-bearing proof of that
branch, yet nothing on the shared spine seeds that anyone could have survived (the
spine consistently treats the gang as killed) — an unprepared late reveal. This is
**case-1** (a MISSING setup = semantic completeness the gates cannot catch, the R476
ceiling), exactly the v1 blemish-class, and it is a completeness dent (4/5 from two
judges), NOT a coherence break (coherence stayed 5/5 unanimous). Honest nuance: the
R521 top-down contract MITIGATED case-1 for the 6 DECLARED load-bearing setups (all
6 planted on the spine, clean), but a SIXTH branch-specific proof mechanism (the
survivor) still emerged unseeded on one of five paths — top-down REDUCED case-1 at
scale but did not eliminate it. (case-2 was eliminated — see the R522 field proof.)

## Control cross-references (orchestrator; judges blind)

1. **v1 (R520), 14 scenes:** unanimous 5/5 overall coherent — the small-scale bar.
   At 70 scenes (5×), coherence HOLDS at the same unanimous 5/5.
2. **scale-floor (R473–R479), ~60 scenes, prose-first authoring:** produced
   store-consistent-but-incoherent results ("store-consistency ≠ causal coherence").
   At 70 scenes, **facts-first top-down authoring AVOIDS that** — unanimous 5/5
   coherent. The flip from the prose-first loss is the controlled result: the
   binding difference is authoring METHOD (facts-first top-down), not scale.

## Decision (pre-committed rule, manifest `decision_rule_pre_committed`)

BOTH pins hold AND judges find it coherent (majority overall ≥ 4/5 — here a
unanimous 5/5 — with no major unresolved holes) ⇒ **premise→self-authored coherence
HOLDS above the 60-scene floor ⇒ AI-self-authoring SCALES.** The North-Star
authoring loop (premise → AI-self-authored, gate-clean, causally coherent branching
fact base) is reachable at novella scale. The next frontier is a DIFFERENT axis
(convergence-JOIN / salience / pinion-projection), not more authoring-coherence
proof.

## Honesty bounds

- **n=1 premise, 70 scenes** — one large instance, not a distribution. The result is
  a strong existence proof at scale, not a frequency claim.
- **Store-consistency is deterministic; causal coherence is judge-derived** (one
  model family; 3 sessions independent + blind to each other, the experiment, the
  scale-floor history, and that the base is AI-authored).
- **case-1 (the R476 ceiling) recurred on ONE of five branches** despite the
  top-down contract — the act-dig survivor. Top-down reduced it (6/6 declared setups
  clean) but did not eliminate it; the lever against the residue stays the
  judge-only semantic-completeness frontier (the sec 7.26 aggregation surface, still
  YAGNI — the gates did their job; this is the gate-blind class by construction).
- **case-2 was eliminated** — 0 `evidence_unreachable` in the real store + a positive
  negative-control firing. The R521 contract + R522 gate worked exactly as designed.
- **The AI ran its own gate loop** (the AI-first self-consistency design); the human
  supplied only the premise + structural constraints. No human supplied a fact.
- The author's deliverables (sections/facts/order JSON + author-log) are tracked; the
  derived `store.atomic.json` is gitignored (reproducible via import).
