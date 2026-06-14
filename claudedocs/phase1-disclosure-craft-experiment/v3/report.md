# disclosure-craft-experiment/v3 — warm-render posture, execution report

**Rounds R514 (manifest) + R515 (execution). Lineage R501–R513 (this
orchestrator). Result: a CLEAN FORK-(1) — render POSTURE was the cause. The warm
storytelling render beat the reused R504 compliance render 3-0 (9/9 world-lines)
under fresh blind judges, while BOTH deterministic pins held on the warm final
with ZERO repairs.** fidelity off_path=0 and leak=0 (vocab_shared=7) in all three
worlds; the warm draft was gate-clean on the first pass, so the warmth was not
bought back by any gate-repair tax.

## Integrity

- Manifest: `warm-manifest.json`, sha256
  `bb2a19de69e4a865d401925d5e6fdc5fbe7e599910c3ba1c5ee3c94d0e508433` — the value
  pinned in the R514 ledger entry BEFORE any blind subagent ran (the
  R453/R499/R503 pre-commit discipline). On-disk == pinned: the protocol executed
  is the pinned one.
- Warm brief: `warm-brief.md`, sha256
  `f5d4eadf7566a3032d0670aed51f9512eca9eefcc544e4da80a4167679d08134`.
- Orchestrator ran the harness/gate commands; the warm author, the extractor, and
  the 3 judges were each spawned as separate blind subagents (fresh context) and
  received only their brief block + the firewall — never the manifest, the
  runbook, the hypothesis, the gates, or the other arm. Honesty bound (recorded):
  the orchestrator knew the A/B mapping (needed to attribute the pins and copy the
  labeled files); it never wrote the mapping into any sub-prompt.

## Provenance of reused inputs (the controlled, held-constant substrate)

- `run/compliance/story.md` — arm C = R504's disclosure render, REUSED verbatim
  (sha256 `a2549badbd255b9ef1507738acfcbe22851ae31a012a3086854e7ff46ab9d7b9`). NOT
  re-rendered. Already judged in R504 (keystone 5.00, overall forced-choice 2-1
  loss to plain).
- `authored.atomic.json` (typed fact base + `dc-v2` disclosure plan),
  `meridian-order.json` (branch-scoped canon order), `extractor-brief.md` (the
  fixed blind-extraction vocabulary), `skeleton.atomic.json` — all REUSED from v2
  verbatim. POSTURE is the only varied variable: arm W renders the SAME beats +
  SAME withholds from a storytelling-job brief; arm C from the R504 compliance
  brief.
- Honesty bounds inherited: arm C is a cross-run control (not a same-session
  twin), judge-clean because the v3 judges are fresh; arm W inherits R504's fact
  coverage; this lineage wrote the warm brief (design-stage selection bias the
  sha-pin does not cover); R505's emotion-anchor facts were deliberately held OUT
  to isolate posture.

## The SEAL (recorded PRE-REVEAL)

```
experiment : disclosure-craft-v3
note       : warm vs compliance; reveal at S7
arms       : warm, compliance  ->  shuffled to labels A, B
seal sha256: 28a11e21769fec58c0f185545e85ebba0d21eb98be6d0e4db41ceacb838c6d1c
```
`verify-seal` at S7: **MATCH** (exit 0 — untampered). Unblind:
`A = warm`, `B = compliance`.

## Posture tax (PRED_posture_tax — RECORDED, not pinned)

- **Warm render:** 1 blind author session, ~70.7k subagent tokens, **9,120
  words** (one continuous authorial voice; arm C ~9,417 — comparable scale).
- **Repairs: 0.** The warm draft passed PIN-W1 (fidelity) and PIN-W2 (leak) on
  the FIRST blind re-extraction — no leak, no world-line drift — so the
  gate-and-repair loop terminated with zero scene rewrites. **Repair
  localization = 0/29 scenes.** The warm posture did NOT trade fidelity for
  warmth; generate-freely-then-gate yielded a clean artifact with no gate tax.
- **Blind re-extraction:** 1 session, ~95.8k subagent tokens, 51 gt facts.

## Quota gate (validity_conditions.quota) — PASS

Warm deliverable `run/warm/story.md`: 29 scenes (>=24), 2 forks (sc-08, sc-16),
3 world-lines (CONFRONT->REVEAL, CONFRONT->BURN, QUIET-AUDIT), 3 endings (sc-19r,
sc-18w, sc-20b), the 6 setups planted in the spine, ~9,120 words. All ids match
the canonical scaffold. Quota met -> all legs run.

## PRE-PINNED WITHHOLD LIST (PIN-W2 targets) — recorded PRE-EXTRACTION

The withheld solution conclusions + earliest legitimate reveal, from the source
plan `dc-v2` + the R504 withhold list. Shared-spine invariant: sc-01..sc-08 are
strictly pre-reveal for all three worlds.

| # | Withheld conclusion (gt frame) | Reveal scene by world (earliest legitimate) |
|---|---|---|
| W1 | Who climbed the stair = Onslow Pike | REVEAL sc-17r (Junia names); BURN never named; QUIET-AUDIT never named aloud (reasoned at sc-19b). Typed gate target: `gt-climber-pike` first_at {reveal: sc-17r, audit: sc-19b}. |
| W2 | Cause = accidental startle-fall, not murder; clock 3:14 = the jolt, not a set alibi | REVEAL sc-15/sc-17r; QUIET-AUDIT sc-18b; BURN sc-18w. (Audited on coords; the negation/reasoning is not a single typed tuple — the unchanged R500/R504 boundary.) |
| W3 | Telegram lay UNDELIVERED (Crane never knew) + theft reconstructed/proven | REVEAL sc-11a; QUIET-AUDIT sc-09b..sc-12b / sc-19b. Typed gate target: `gt-telegram-undelivered` = `crane/knows-dismissal/no` first_at {reveal: sc-11a, burn: sc-11a, audit: sc-19b}. |

The early reader-secrets (S1 forged-telegram/embezzlement existence via the
half-burnt grate draft; S2 Junia saw a climber and lies) are SHOWN early as
dramatic irony — re-extracting those early is the design, not a leak; W3's
withheld part is specifically the proven investigative resolution.

## PIN-W2 — premature-leak gate (`validate-disclosure-leak`, telling `dc-v2`) — HOLDS

Blind re-extraction `run/extract/W.reextract.atomic.json` (51 gt facts; the typed
conclusions land only at their sanctioned scenes — `climber/identity/pike` @
sc-17r only, `crane/knows-dismissal/no` @ sc-11a + sc-19b; SILENT in burn/audit).

| world  | targeted | **leaks** | unordered (B-1 honesty) | truth_frame_typed | **vocab_shared** | exit |
|--------|----------|-----------|--------------------------|-------------------|------------------|------|
| reveal | 2        | **0**     | 1 (telegram @sc-19b, cross-world)        | 7 | **7** | 0 |
| burn   | 1        | **0**     | 1 (telegram @sc-19b, cross-world)        | 7 | **7** | 0 |
| audit  | 2        | **0**     | 2 (climber @sc-17r; telegram @sc-11a)    | 7 | **7** | 0 |

0 leaks in every world; `vocab_shared=7 > 0` = the R510 F5 non-vacuous guard
(genuine clean pass, not a foreign-id artifact). Cross-world matches surfaced as
`unordered`, never silently dropped. Reproduces R504/R512's PIN-2 on the WARM
prose, tool-run and deterministic.

## PIN-W1 — render<->world-line fidelity gate (`validate-render-fidelity`) — HOLDS

Per-world single-world projection of the warm re-extraction (facts whose
canon_from is in the world's scene set), checked against that world's order (the
v2 accept-branch shape):

| world | re-extracted facts | off_path | unplaced | reached_terminal | exit |
|---|---|---|---|---|---|
| reveal | 33 | **0** | 0 | yes | 0 |
| burn   | 33 | **0** | 0 | yes | 0 |
| audit  | 30 | **0** | 0 | yes | 0 |

off_path=0 (no world-line drift), unplaced=0 (every coord is a real declared node
= the blind extractor used canonical scene ids faithfully), reached_terminal=true
(each world reaches its maximal scene). The R488 prose analog, clean on the warm
final.

**As-built deviation (faithful).** The held-constant v2 extractor-brief produces
typed gt facts for the leak + fidelity gates, NOT the rule/state substrate that
`validate-continuity` needs, so the D1/D2/D4 continuity metrics are not separately
computed (`validate-continuity` reports the continuity table absent) — exactly as
in v2, which also realized fidelity via the render-fidelity gate. PIN-W1 is the
render<->world-line fidelity gate (off_path=0 + unplaced=0 + reached_terminal),
the tool-gate-era mechanism; the manifest's "D1+D2+D4=0 AND off_path=0" conflated
the v1 (continuity-metric) and v2/v3 (typed-tuple gate) eras.

## Reading copies (deterministic) + the harness deviation (recorded)

The per-world reading copies were assembled with the R509 harness from each arm's
prose over a deterministic per-world playthrough built from the canonical
`meridian-order.json` order (identical for both arms; no begins>0 prune of the
two fact-silent scenes). TWO content-preserving, symmetric normalizations were
applied to both arms before assemble:

1. **Heading titles.** `assemble` requires `## sc-NN — Title` headings; arm C
   (R504) has bare `## sc-NN`, arm W has its own titles. Both arms' headings were
   normalized to the SAME canonical titles from the fact base (assemble then
   renders them as neutral `## Title`), so judges see identical headings — the v1
   `*.norm.md` precedent.
2. **Scaffolding strip.** The R509 assemble strips fork bullets carrying the
   literal `CHOICE` token (arm C's format) but NOT arm W's `- **CONFRONT**`
   bullets (no token), and neither catches the trailing `## World-line and ending
   map` (which leaks into the audit terminal scene for BOTH arms). Per the tool's
   own guidance ("extend the strip vocabulary OR reword the source"), the `###
   CHOICE` blocks and the world-line map were stripped from BOTH sources
   structurally (prose untouched, 29 scenes preserved) before assemble. Residual
   check after re-assemble = 0 in all six copies. This is the v1/.clean
   scaffolding-strip deviation, made symmetric and structural (not blind regex on
   prose). NOTE: that arm W's CHOICE-bullet form slipped both the R509 strip AND
   its loud-reject is a residual harness gap worth a future assemble.rs broaden
   (recorded; out of scope this round — no new harness code per the manifest).

Six reading copies, residual=0, comparable scale (warm ~5% shorter, matching 9,120
vs 9,417): reveal A 5,664 / B 5,960; burn A 5,382 / B 5,662; audit A 4,649 /
B 4,914.

## REVEAL — craft (PRED_craft, the OPEN question): a CLEAN 3-0 FLIP to warm

3 fresh blind judges x 3 matched world-lines, R500 5-axis rubric. Forced choices:
**every world-line A; overall A — world-lines A 9 / B 0; overall A 3 / B 0.**

5-axis means (1-5; A = warm, B = compliance; mean over 3 judges x 3 worlds), with
the R504 RECORDED scores beside them (cross-run, uncontrolled — different judge
panel):

| axis | **A warm** | **B compliance** | R504 compliance | R504 plain |
|---|---|---|---|---|
| **prose quality (told-story vs list-like — KEYSTONE)** | **5.00** | 4.11 | 5.00 | 4.22 |
| stakes / tension | 4.78 | 4.00 | 4.22 | 4.22 |
| setup / payoff | 5.00 | 4.11 | 4.56 | 4.33 |
| character-knowledge | 5.00 | 4.89 | 4.67 | 4.67 |

Warm wins the forced choice in all 9 world-line judgements and all 3 overall, and
ties-or-beats compliance on every axis (wins prose, stakes, payoff; near-ties
knowledge). All three judges independently described the SAME contrast: A
"dramatizes / plays the scene / a closer, inhabited voice"; B "steps back into
expository/legal-summary register," "cools into a recap-of-clues, list-like
register at each payoff," "states the ruling where A plays the scene." This is the
R504 plain-vs-compliance contrast, now reproduced with WARM beating COMPLIANCE.

**Re-cool sub-question (vs repair localization = 0):** the judges flagged
cooler/flatter passages almost EXCLUSIVELY in arm B (compliance) — its
certificate/recap scenes. Arm W drew only ONE mild flag (judge-3, the audit
world's "Quiet Reconstruction", inherent method-stating in the reasoning-heavy
audit road), and since arm W had ZERO repairs, NO re-cooling is repair-induced.
The warm arm reads consistently warm; the cool prose is the compliance arm's.

**Honest reading.** The PRIMARY result is the WITHIN-run, same-panel, blind
forced choice: warm beats compliance 3-0. The R504 absolute numbers are a
cross-run reference only (R504's judges scored the SAME compliance prose 5.00 on
the keystone; this panel scored it 4.11 — judge-panel variance, so absolute
cross-run numbers are not comparable; only the within-run direction is
controlled). The flip is clean and large: in R504 plain beat compliance 2-1 on
warmth; in v3 warm beats compliance 3-0, with both pins holding and zero repairs.

## DECISION (manifest `decision_rule`) — FORK (1): POSTURE WAS THE CAUSE

W beats C on the forced choice (3-0) **AND** PIN-W1 + PIN-W2 hold **AND**
localization is bounded (0 repairs, no repair-induced re-cooling). This is
exactly fork (1): **render POSTURE was the cause of R504's loss, not missing data
and not the R476 model-craft ceiling.** The same model, same fact base, same
disclosure plan, same beats + withholds, same scaffold and word budget produced
prose that beats the compliance render decisively when the JOB is re-registered
from "project this fact list under this plan, do not leak" to "tell the best
story." Relocating fidelity OUT of the author's prompt INTO the R508 gates worked:
the warm draft was free AND verifiable — both deterministic gates passed on the
first pass.

-> **warm-render-then-gate(-repair) is validated as the render discipline.** Build
implication = YAGNI-DEFERRED (a `--warm-brief` projection / repair-loop helper
builds ONLY when a real authoring run pulls it; here zero repairs were needed, so
the pull is even weaker than R504 estimated). The R476 ceiling is NOT the binding
constraint on warmth; posture is the lever, and it is a prompt-level lever, not a
substrate one.

## PRED — economics + residue

- **Render cost:** 1 warm session ~70.7k tokens / 9,120 words; re-extraction
  ~95.8k tokens; 3 judges ~106k each. 0 repair sessions.
- **Posture tax:** 0 repairs, 0/29 localization, no judge-flagged warm re-cooling.
- **Untyped residue (R476):** judges found ~0 internal continuity errors in either
  arm (knowledge-believability 5.00 warm / 4.89 compliance); residue ~ 0 both arms.

## Honesty bounds that bit (restated from the manifest)

- Arm C is the R504 disclosure render REUSED verbatim — a cross-run control, not a
  same-session twin; judge-clean (fresh judges), but the cross-run R504 absolute
  scores are uncontrolled references, not a same-panel comparison.
- Arm W reuses R504's fact base + `dc-v2` plan UNCHANGED (deliberate posture
  isolation); R505's emotion-anchor belief-frame facts were held OUT — so this
  result attributes warmth to POSTURE alone, not to added interiority facts.
- This lineage wrote the warm brief (design-stage selection bias the sha-pin does
  not cover); the author/extractor/judges were blind (covers tampering/peeking,
  not brief selection).
- The two heading/scaffolding normalizations are recorded deviations from the
  literal harness path (symmetric, content-preserving); the arm-W CHOICE-bullet
  form slipping the R509 strip+loud-reject is a residual harness gap (future
  assemble.rs broaden).
- PIN-W2's typed-tuple gate covers the two load-bearing conclusions; the broader
  W1/W2/W3 list is audited on re-extraction coordinates (soundness exact;
  completeness rests on extractor recall — the unchanged R500/R504/R512 boundary).
- D1/D2/D4 not separately computed (lean v2 extraction); PIN-W1 realized via the
  render-fidelity gate (see deviation above).

## Result table (R452 self-containment)

| leg | result |
|---|---|
| PIN-W1 fidelity (warm final, per world) | off_path **0** / unplaced 0 / terminal yes — **HOLDS** (3/3 worlds) |
| PIN-W2 premature-leak (warm final, per world) | leaks **0**, vocab_shared 7 — **HOLDS** (3/3 worlds) |
| posture tax | repairs **0**, localization **0/29**, no warm re-cooling |
| craft forced choice | warm **9/9** world-lines, **3/3** overall (R504: plain beat compliance 2-1) |
| prose keystone (within-run, same panel) | warm **5.00** vs compliance 4.11 |
| **DECISION** | **FORK (1): POSTURE WAS THE CAUSE** — warm-render-then-gate validated; build YAGNI-deferred |

## R525 — reading copies made deterministically regenerable (the A-2/A-3 debt closed)

The R515 reading copies relied on TWO manual pre-`assemble` normalizations done
inline (the recorded SSOT-duplication debt): a heading-title `.norm` step and a
scaffolding `.clean` step. R516 already folded the scaffolding strip into the
harness; R525 folds the heading norm in too, via a new `assemble --titles-from
<store>` flag that sources each scene heading from the fact base's section titles
(neutral, arm-independent), plus a parser generalization so an arm whose source
carries the BARE `## sc-NN` form (the reused compliance render) assembles without a
pre-norm. The six reading copies are now produced by ONE deterministic command per
(arm × world) over tracked inputs:

```
mnemosyne-cli report-playthrough-manuscript --world <W> --order meridian-order.json \
  --sidecar authored.atomic.json --json > <W>.playthrough.json        # full walk = 19/18/16
cargo run --manifest-path tools/experiment-harness/Cargo.toml -- assemble \
  --story run/warm/story.md|run/compliance/story.md \
  --playthrough <W>.playthrough.json --world <W> \
  --titles-from authored.atomic.json --out run/reading/<W>__<A|B>.md   # A=warm, B=compliance
```

`authored.atomic.json` (the typed fact base + `dc-v2` plan) is now tracked (a
`.gitignore` exception — it is a source, not a derivable store), so all six copies
regenerate **byte-identically from tracked inputs alone** (verified: 0 differing
lines, all six). No inline normalization remains; the `.norm`/`.clean`
intermediates are gone.

**Effect on the judged copies (honest):** the regenerated `reveal__A` and `burn__A`
each dropped 4 immaterial trailing `---` rules the old manual `.clean` left in (the
exact "modulo 4 immaterial `---`" R516 recorded); the other four copies were already
byte-identical. The prose the judges read is unchanged, so the **3-0 warm verdict
stands**. R525 is a reproducibility/SSOT cleanup, not a re-judge — the A-1/A-4
blind-fidelity + 3-blind re-judge remain separately EXPERIMENT-gated.
