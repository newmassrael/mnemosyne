# A/B Authoring Experiment — Final Report (S11 reveal round)

Date: 2026-06-11 (design R469, amended R470; all 11 execution sessions same
day). Model, all of S1–S10: claude-fable-5. Protocol SSOT: `ab-manifest.json`
(sha256 `30ed5ed6…1f7e`, R470 ledger pin, superseding the R469 pin — both
sealed before any execution session). Run state: `run-log.md`. Verdicts:
`verdict-{1,2,3}.md`. Arm workspaces: `/home/coin/saltglass-{novella,studio}`,
extraction `/home/coin/saltglass-extraction`, judging
`/home/coin/saltglass-judging` (all git-snapshotted per session).

## Unblinding

label-map.txt sha256 re-verified at S11 against the value sealed at S7
(2026-06-11T15:16:45+09:00): `b8f71a6d89137fdb35f25ea2fdc33ec582838bd30…`
— match. Mapping: **story-A = plain (novella), story-B = loop (studio)**.

Owner was de facto unblinded earlier (S7 copy-integrity output + explicit
disclosure on request, both logged as deviations); the three judge sessions
were blind throughout.

## Results

### Deterministic leg (blind re-extraction, one recipe for both)

| | story-A = PLAIN | story-B = LOOP |
|---|---|---|
| scenes / facts re-extracted | 29 / 145 | 30 / 136 |
| recipe rules derived | 6 | 7 |
| D1 gated rule violations | 0 | **1** |
| D2 unchained_state_pairs | 0 | 0 |
| D3 required-setup dangling | 0 | 0 |
| D4 fork-boundary faults | 0 | 0 |
| **Primary endpoint** | **0** | **1** |

The loop's one defect: boots custody, sc-15 (shared CONFRONT limb, boots
moved to cottage) vs sc-21 (DESTROY world, boots back at the stair foot,
no narrated return) — rule_exclusive_overlap. The loop author's OWN store
passed 0 violations throughout authoring: it never tracked boots custody.

Recorded-not-counted: loop QUIET warrant cites the receipt discovered on
the unchosen CONFRONT limb (fork-seam, non-custody); loop setup-5 planted
per-limb instead of trunk; plain's soft hem seam across fork-2. All
playthrough honesty surfaces empty on all six walks.

### Preference leg (3 blind judges, per matched world-line)

| world-line | J1 | J2 | J3 | result |
|---|---|---|---|---|
| confront-reveal | A | A | A | **PLAIN 3–0 unanimous** |
| confront-destroy | B | A | B | LOOP 2–1 |
| quiet | B | A | A | PLAIN 2–1 |
| judge's set | B | A | A | **PLAIN 2–1 by judge, 6–3 by pair** |

Texture: J2 scored the loop 5/5 on character-knowledge believability on
ALL three lines and called it the stronger prose — "the sweep is about
whole-world bookkeeping, not writing quality." The unanimous pair (reveal)
turned on plain's planted-before-used evidence vs the loop's horn-button
setup declared retroactively at the moment of need (cited independently by
all three judges).

### Economics

| | PLAIN | LOOP | ratio |
|---|---|---|---|
| cost (3 sessions) | $12.30 | $22.25 | 1.81x |
| output tokens | 115,046 | 175,747 | 1.53x |
| scenes delivered | 29 | 30 | — |
| **output tokens / scene** | **3,967** | **5,858** | **1.48x** |
| wall time | ~48m | ~50m | 1.05x |

Prevented defects (loop session logs): **0 story-class**; 3 repairs, all
substrate-friction (§-prefix coordinate reject, rules-file shape x3,
canon-order branch shape) — friction decayed to 0 by session 3.
Grading-line costs: extraction $42.80 (235k out, 23.8m cache read),
judges $5.36 + $3.72 + $4.35. Experiment total across S1–S10 ≈ **$90.78**.

## Prediction grading (against the sealed manifest)

- **PRED-1 (floor — the only pinned outcome): REFUTED on both conjuncts.**
  Plain showed 0 primary-endpoint defects (pin required >= 3); loop showed
  1 > plain's 0 (pin required loop < plain).
- **PRED-2 (ceiling, no pin):** recorded expectation was near-tie with
  judges weighting prose voice over structural correctness. Outcome: narrow
  plain preference (2–1 / 6–3), and the weighting INVERTED — judges
  explicitly preferred whole-world bookkeeping over admitted prose
  superiority.
- **PRED-3 (economics, 1.5x–4x per delivered scene):** 1.48x — marginally
  BELOW the predicted band. The overhead lives in cache-read (store
  re-reading), not prose generation. Prevented-defect row: 0 story-class.
- **PRED-4 (untyped residue >= 1 on plain): CONFIRMED, with margin.**
  Plain's D-metrics = 0, yet judges cited >= 4 distinct real issues: hem
  unpicked-vs-torn at the branch joint (J1), Mara using Caul-private-POV
  specifics on the quiet line (J1), trunk day-count drift (J1+J2), unpaid
  trunk promises on the destroy line (J3); plus "two T.C.s" (J1+J2). Loop
  residue beyond its 1 gated defect: 1913/"twelve years"/1923 date
  arithmetic (J1+J3), 12+ booked passengers never accounted (J2),
  fee-book/ruled-line fork-seam (J2+J3), receipt fork-seam (extractor).
  S7-artifact citations (terminal italic fragments, all three judges)
  excluded per the S8 deviation ruling.

## Analysis

1. **The floor died because the control self-organized.** The plain agent
   invented an in-file "continuity ledger" (ground truth, custody tables,
   frame lists, payoff-debt list) — a paper Mnemosyne, unprompted. The
   comparison silently became *freeform notes vs typed store + gates*, and
   at 29–30 scenes on a Fable-tier model, freeform notes suffice for every
   deterministically-detectable class. Independent evidence that external
   working memory is NEEDED (the model built one), and that at this scale
   improvised memory is ENOUGH.
2. **Coverage binds, not gate power.** The loop's protection is
   min(store coverage, gate power), and the experiment measured coverage as
   the binding constraint: its one defect sat exactly where the author's
   recording granularity fell short of the grader's fixed recipe (boots
   never custody-tracked), while its 5/5 knowledge-discipline scores sat
   exactly where it DID record (frames). Lesson for the loop protocol:
   author under the grader's recipe — track every object two parties touch,
   every state the prose asserts, scenery included.
3. **Fork-seam is the dominant residue class and it is universal.** Both
   arms leaked across world-line boundaries (loop: receipt, fee-book/ruled
   line; plain: unpaid trunk promises, hem state across fork-2), found
   independently by the extractor and two judges. Neither gates nor notes
   prevented it. This is the sharpest product signal: fork boundaries need
   first-class auditing (R445's boundary lesson, now quantified on free
   prose), and payoff tracking should generalize beyond declared setups to
   selectional promises (R442's expected-payoff leg has the right shape).
4. **Untyped residue is real and dominates the gated count (PRED-4).** The
   deterministic line caught 1 defect across both stories; blind readers
   found ~8 distinct real ones. On free prose, the discovery direction
   (R453's untyped-miss quantification, R457/R458 typing discovery, R461
   edge discovery) now has its corpus-measured pull: the gap between gated
   and felt continuity is where LLM-discovery verbs live.
5. **Economics favor the loop more than predicted** (1.48x/scene, below
   the 1.5x floor of the band), and substrate friction is a first-session
   phenomenon that decays to zero. The cost story is not the blocker.

## Product findings (carry into the ledger)

1. **PAY NEXT — narrative-rules loader strict-parse.** The engine silently
   ignored an unknown rule-level `subject` field (S7 had to encode recipe
   scoping in comments). Same silent-no-op class R450 (padded predicate)
   and R468 (unknown --field) already eliminated elsewhere: unknown keys in
   narrative-rules/v1 must reject loudly. Small, test-pinned.
2. Subjects/objects scoping on rules: deferred — the comment workaround
   kept D1 at one-violation-per-defect; build on second consumer pull.
3. Fork-seam audit + selectional-promise payoff tracking: sec 10 backlog
   candidates, now with quantified pull from BOTH arms; gate on next
   corpus, do not build speculatively.
4. Reading-copy normalization: S7's assembly left story-B's trailing
   synopsis fragments in all three copies (asymmetric anti-B bias, logged;
   B won J1 despite it). If the manuscript→reading-copy path recurs, its
   normalization needs a pinned spec rather than per-session judgment.

## Honesty bounds

Manifest bounds carry verbatim (n=1 premise, one model family; designer ==
orchestrator == premise author; extraction is LLM work, parity + pins
mitigate variance, they do not remove it; loop measures agent+gates as a
system, not model capability; prose may fingerprint the loop to judges).
Run-added: owner unblinded post-S7 (logged twice); owner judge-4 read not
performed; S7 normalization artifact penalized B in all judge inputs
(citations excluded from residue, preference reading caveated); judges read
LINEAR copies — structure-native qualities (the branching object itself)
were only partially visible to the preference leg.

## Verdict

At this scale and model tier, the floor hypothesis is refuted: a plain
agent with self-invented notes authors a 29-scene / 3-world branching
novella with zero deterministically-detectable defects, and the loop's
gates add no defect advantage while costing 1.48x tokens per scene. What
the experiment actually bought: the first quantified map of where ALL
current tooling fails (fork seams, scalar arithmetic, knowledge leaks —
the untyped residue), proof that store coverage rather than gate power is
the binding constraint, live evidence that external working memory is
needed enough that models improvise it, and a cheap-side economics number.
The next floor experiment must move to where improvised notes break:
longer corpora, more worlds, more sessions, or weaker authoring tiers —
with the loop protocol upgraded to author under the grader's recipe.
