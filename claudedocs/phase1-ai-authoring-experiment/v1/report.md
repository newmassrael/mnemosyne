# ai-authoring-experiment/v1 — report (R520 execution)

**Headline:** BOTH deterministic pins HELD and 3 blind judges found the
AI-self-authored story COHERENT (unanimous 5/5 overall) → by the pre-committed
decision rule, **AI-self-authoring is REACHABLE at small scale → the next axis is
SCALE** (the scale-floor rematch). The manifest was sha-pinned pre-execution
(R519, commit `ac8f5ab`, sha256 `9a095a17…`); this report records execution.

## What ran (firewall held)

premise → ONE blind author subagent (fresh context; only premise.md +
author-brief.md) → the orchestrator independently ran the pins + assembled neutral
per-world manuscripts → 3 blind judge subagents (fresh context; only judge-brief.md
+ the manuscripts) → the orchestrator applied the decision rule. The orchestrator
(this R518/R519 lineage) wrote the manifest + premise + briefs and ran the
deterministic gates but authored no fact and judged nothing (the R469 contamination
bound).

## The authored story (neutral)

"The Saltmarsh Bell" — 14 scenes, 36 facts, 3 frames (gt / aldous / maren), one
primary fork at sc-05 into three terminal world-lines (expose / confront /
act_alone). Ground truth: Crispin's senses are failing; the early bell is mercy +
penance; the Kittiwake was lost to its master's own smuggling, NOT the bell.
Belief frames diverge from it: Aldous believes the bell a snare; Maren believes it
killed her cousin. The author ran **3 write→gate→read→repair iterations** to reach
gate-clean (author-log.md): pass 1 flagged a dangling spine setup + untyped
payoffs; pass 2 added typed state-change legs; pass 3 clean.

## PIN-A1 — convergence to gate-clean — **HELD** (orchestrator-run)

- **Reproduce:** import sections.json + facts.json into a fresh empty seed = 0
  errors, 36 facts reproduced (loads clean).
- **validate-continuity** `--order`: `violations: 0 (structural=0 interval=0)`;
  `unordered=0`, `order_nodes=14`. The 3 belief-vs-truth pairs register as
  `cross_scope(data)=3` (cross-frame data, not conflicts) — the multi-axis frame
  model working.
- **Per-world placement** (expose / confront / act_alone): each
  `unplaced=0, undecidable=0, undeclared adjacencies=0`, 8 scenes.
- **As-built (honest):** the manifest's literal criterion "outside-order = 0" was
  an over-specification. `outside order=6` per world = the OTHER two branches'
  3+3 scenes correctly excluded from that world's walk — a benign cross-branch
  signal, present in the hand-authored reference base too. The substantive
  placement-defect criterion (unplaced / undecidable / undeclared = 0) is what
  PIN-A1 tests, and it holds. (Goalposts not moved: the over-spec is recorded, not
  silently dropped.)

## PIN-A2 — structural completeness floor — **HELD** (orchestrator-run)

- **report-payoff-coverage** `--order`: every TERMINAL world `dangling=0`
  (expose/confront/act_alone each `paid=2 dangling=0`). The pre-fork `main` stub
  shows `dangling=1` (`gt-early-is-mercy`) — a spine setup whose payoff lives in
  the descendant branches (it pays off in all 3 terminal worlds). **Verified
  identical to the hand-authored reference base** (Meridian R504/R514): its `main`
  stub dangles 8 and `confront` intermediate dangles 2, while its terminal worlds
  dangle 0 — the AI base's structural floor is the SAME shape and CLEANER (1 vs 8).
- **report-fork-tree** `--order`: `3 registered world-line(s), 0 unplaced fork
  point(s)`; 1 fork at sc-05 with 3 children (≥2); every world-line terminal.
- **report-timeline-gaps** `--order` per terminal world: `violated=0`.
- **Off-branch:** `validate-continuity structural=0` (no succession cross-branch).

## Causal coherence — JUDGED (no pin; the scale-floor "store-consistency ≠ causal coherence" check)

3 blind judges, R500 5-axis rubric adapted to story-logic (verdicts.md):

| | coherence | completeness | relevance | branch | **overall** |
|---|---|---|---|---|---|
| Judge 1 | 5 | 5 | 5 | 5 | **5** |
| Judge 2 | 5 | 4 | 5 | 5 | **5** |
| Judge 3 | 5 | 4 | 5 | 5 | **5** |

Unanimous **overall 5/5**, coherence 5/5, relevance 5/5, branch-integrity 5/5;
completeness 5/4/4 (mean 4.33). All three independently rendered the decisive
verdict: **"a genuinely authored story, not an internally-consistent pile of
facts."** At 14 scenes, store-consistency AND causal coherence BOTH hold — the
scale-floor's "store-consistency ≠ causal coherence" did NOT reproduce at this
scale.

**The two flagged blemishes are exactly the R476 ceiling, surfacing where R518
predicted.** Both are completeness (not logic breaks) and both are the kind of
semantic gap the gates CANNOT catch by construction: (1) sc-07e introduces the
harbourmaster + contraband at the resolution (a late-but-consistent fact — no gate
flags a fact that IS present and consistent, only one that contradicts or
dangles); (2) sc-07a backreferences a cross-branch event (a narrative reference
the structural gates don't model). This is the R518 decomposition confirmed live:
structural completeness (gated) = clean; semantic completeness (judge-only) = 2
minor blemishes. They did NOT rise to "incoherent" (the alternative fork).

## Control cross-reference (orchestrator; judges blind)

The hand-authored Meridian base (R504/R514) passed its gates and its world-lines
were judged coherent — the small-scale coherence bar. The AI-self-authored base
reaches comparable judged coherence (unanimous 5/5 overall) AND a cleaner
structural floor (main-stub dangles 1 vs 8). At small scale, AI-authored is not
distinguishable from hand-authored on coherence.

## Decision (pre-committed rule, manifest `decision_rule_pre_committed`)

BOTH pins hold AND judges found it coherent (majority overall ≥ 4/5 — here a
unanimous 5/5 — with no major unresolved holes) ⇒ **AI-self-authoring is REACHABLE
at small scale ⇒ the next axis is SCALE** (the scale-floor rematch at 60+ scenes):
does premise→self-authored coherence hold ABOVE the 60-scene floor where the
prose-first loop lost? This run is the small-scale BASELINE the scale test needs.

## Honesty bounds

- **n=1 premise, 14 scenes** — well below the 60-scene scale-floor. This is the
  baseline, NOT a scale claim. The expected result (PRED_coherence) was "plausibly
  coherent at small scale"; it landed there.
- **Store-consistency is deterministic; causal coherence is judge-derived** (one
  model family; 3 sessions independent + blind to each other and the experiment).
- **The 2 completeness blemishes are the R476 ceiling** — judge-only, gate-blind by
  construction. Minor here; they would COMPOUND at scale (the scale axis must watch
  exactly these late-reveal / cross-branch-reference failure modes).
- **The AI ran its own gate loop** (the AI-first self-consistency design); the
  premise + structural constraints were human-supplied (the human's premise /
  direction role). No human supplied a fact.
- The author's deliverables (sections/facts/order JSON) are tracked; the derived
  `store.atomic.json` is gitignored (reproducible via import).
