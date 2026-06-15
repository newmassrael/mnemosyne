# npc-dialogue-experiment/v1 — report (R550 design / R551 executed)

**Question (manifest):** can an AI SELF-AUTHOR a gate-clean broad+deep fact base whose
CAST STAYS PEOPLED into the deep branch tails (the R545 blemish fixed), and then RENDER
from it warm NPC DIALOGUE — each person in a distinct, consistent voice, speaking only
what their own frame could know — that stays gate-clean (no premature leak, no
off-branch / cross-world knowledge)? Combines the two owner-chosen R550 candidates:
(1) NPC dialogue render (R529 #9, the first warm render over a broad+deep base on the
R514/R515 warm-render-then-gate discipline) and (2) cast sustainment (the R545 blemish).

**Manifest pinned pre-execution:** sha256
`4a04c53910082ee25b400ddff1b7c28631201cc05302ae1a8ed22f2cbd43275d` (R550 ledger,
committed `d1a3793` before any subagent ran). Premise: *Wreckfall at Cawdy Holm*
(fresh, original). Firewall (R469, 11th use): this lineage authored the
design/manifest/premise/four briefs; six blind fresh-context subagents did the work —
a blind author-A (fact base), a blind author-B (warm dialogue render), a blind
re-extractor, 3 blind judges. The orchestrator rebuilt + ran the gates, authored /
rendered / re-extracted / judged nothing.

## RESULT = REACHES (decision-rule branch: all pins hold AND every judged axis ≥ 4 majority)

A blind author authored a **32-scene / 135-fact / 3-world-line** shut-in coastal
mystery with **12 distinct person-frames** (+ gt), kept peopled into every road's deep
tail; a second blind author rendered it as **~22,000 words of warm NPC-dialogue prose**
across three roads. **All three deterministic pins HOLD** (orchestrator rebuilt the
store FRESH from the author's JSON and ran every gate). **3 blind judges scored
unanimous 5/5 overall**, with cast-sustainment, knowledge-realism, branch-integrity,
and voice-consistency all 5/5/5 and voice-distinctness / naturalness 5/4/5. All three
chose **"living story"** over "arranged information" on every road. No knowledge breaks.

## PIN-1 — authoring gate-clean (HOLD)

Rebuilt fresh from the author's sections.json + facts.json + order.json into a clean
schema-23 seed (135 facts / 32 sections import clean), then (with
`--rules narrative-rules.json` for the R449 exclusive-possession rule):

- `validate-continuity --order --rules`: `facts=135 order_nodes=32 conflict_pairs=0
  cross_scope=0 unordered=0 rules=1 unchained_state_pairs=0` — **violations 0
  (structural=0 interval=0)** (incl. R522 evidence-reachability + R488 off-branch + the
  exclusive possession rule bit clean).
- `report-fork-tree`: **3 registered world-lines, 0 unplaced fork point(s)**; report /
  shelter / confront fork from `main` at sc-15 (≈47%); every world reaches a terminal.
- `report-playthrough-manuscript --world {report,shelter,confront}`: unplaced=0,
  undecidable=0 each.
- `report-timeline-gaps`: 0 gap / 0 unreached scene each world.
- `report-payoff-coverage`: paid=2 dangling=0 every terminal world (f-108 keeper's-tally,
  f-110 boy's-lantern — each a single-person piece of knowledge).
- `report-payoff-substantiation`: substantiated=2 unsubstantiated=0 every world.

## PIN-2 — cast sustainment (HOLD; the R545 blemish did NOT reproduce structurally)

The NEW deterministic metric: deep-tail(W) = scenes strictly after W's fork on W's
chain; active = person-frames (≠ gt) holding ≥1 fact whose canon_from is a deep-tail
scene. Principals (top-3 by total fact count) = halsa / officer / headman.

| road | deep tail | active person-frames | non-principal active | floor (≥6 / ≥4) |
|---|---|---|---|---|
| report | sc-16..21 | **12** | 9 | PASS |
| shelter | sc-22..27 | **12** | 9 | PASS |
| confront | sc-28..32 | **12** | 9 | PASS |

All 12 person-frames hold ≥1 fact in EVERY road's deep tail (far above the ≥6 / ≥4-non-
principal floor). >= 10 populated person-frames: PASS (12). The R545 cast-thinning did
not reproduce with the sustainment clause in the brief. **NECESSARY-NOT-SUFFICIENT
LIMIT (not downplayed):** PIN-2 is a structural floor; felt sustainment is the judged
axis — see the shelter sc-27 blemish below.

## PIN-3 — warm-dialogue render acceptance (HOLD, zero repairs)

Over a BLIND re-extraction (separate firewalled subagent; 171 facts from the prose's
own `## sc-NN` markers, EXPLICIT only; shared VOCABULARY = ids, not facts/plan):

- **leak (R502):** `validate-disclosure-leak --telling holm --truth-frame gt`, every W
  = **leaks=0**, `vocabulary_shared=20` (the R510 F5 non-vacuous guard), 2 targeted
  (the false-light f-002 + the killing f-005); both withheld solutions stayed unspoken
  before their per-road reveal scenes.
- **fidelity (R505/R508, the R488 prose analog):** over a single-world projection of the
  re-extraction (spine + that branch), every W = **off_path=0, unplaced=0,
  reached_terminal=true**. No NPC dialogue asserts off-branch / cross-world knowledge.
- **repairs = 0.** The warm draft was gate-clean on the FIRST blind re-extraction — the
  R515 narration result (zero posture tax) extends to dialogue at breadth.
- AS-BUILT (faithful): the re-extractor built ONE combined store (all 3 roads); run
  whole against one world it flagged the OTHER branches' facts as off-path (a store-
  composition artifact, not a render defect), so fidelity was run per world over the
  single-world projection — the R512 "single-world projection" method. The leak gate is
  world-scoped natively and needed no such filtering.

## JUDGED (3 blind judges, rendered prose, no frame labels, no plan; no pin)

| axis | judge-1 | judge-2 | judge-3 |
|---|---|---|---|
| voice distinctness | 5 | 4 | 5 |
| voice consistency | 5 | 5 | 5 |
| dialogue naturalness | 5 | 4 | 5 |
| **cast sustainment** | **5** | **5** | **5** |
| knowledge realism | 5 | 5 | 5 |
| branch integrity | 5 | 5 | 5 |
| **overall** | **5** | **5** | **5** |
| forced choice | living story | living story | living story |

All three independently named the SAME strength: voice + knowledge discipline — a
genuinely individuated chorus (the pilot's water-reading cadence, the factor's
accountancy, the headman's stone-laying chapel register that drops to "something older
and lower" only when he confesses, the salt-wife's tumbling self-exculpation, the
child's broken three-tellings) where each person speaks only what their own path could
earn. The proof is the supercargo's murder: named-and-caught in report, **deliberately
never learned in shelter** (Halsa stays permanently ignorant; the curer goes on calling
it weather), forced into the open in confront by the one person (the passenger) who
holds the kin/key/deed — the same buried fact disclosed differently on each road by
exactly what proof reached whom.

**Decision rule (manifest, pre-committed):** all three pins hold AND majority ≥ 4 on
voice-distinctness AND naturalness AND cast-sustainment-feel AND knowledge-realism, no
major knowledge breaks ⇒ **NPC DIALOGUE RENDER over a SUSTAINED broad+deep cast
REACHES.** Met decisively. Candidate (1) and candidate (2) both resolve positive: warm-
render-then-gate extends from narration to dialogue at breadth (zero repairs), and the
R545 cast-thinning is brief-fixable (structural PIN-2 + judged sustainment both held).

## Honest caveats + the named blemishes (NOT downplayed — feedback_dont_downplay_experiment_flaws)

The 5/5/5 does NOT mean flawless; all three judges flagged real, consistent craft
seams, and the experiment has standing limits:

- **The shelter road's ending (sc-27) NARRATES rather than DRAMATIZES.** Judge-1 docked
  shelter sustainment to 4/5 (per-road): the final scene reports the secondary cast
  (Orne "ruled it as before", Wick "grew quiet", Ysolt "did not leave") in third-person
  summary, and the boy gets no closing line of his own. This is the **PIN-2 necessary-
  not-sufficient gap partially realized** — the frame is structurally ACTIVE in the tail
  (it holds a fact) yet reads as accounted-for-from-outside, not living on the page.
  PIN-2 passed; the judge caught the hollow spot PIN-2 cannot.
- **The climactic reveal scenes tidy toward exposition / oration.** Judges 2 and 3 both
  flagged it (the only reason naturalness is 4 from judge-2 and 4 on report+confront from
  judge-3): the officer's stakes-speech (report sc-16), the bar-reckoning oration
  (confront sc-29-30), and the most quotable aphorisms ("It's true. That's better than
  right"; "Buried's not undone") occasionally tip from overheard talk toward staged
  epigram / a clue-list read aloud. A recurring craft seam (the R476 ceiling family),
  not a structural or knowledge failure.
- **Voice-distinctness is not a clean 5:** judge-2 gave 4, noting the officer and the
  factor both reach for a ledger/count idiom and blur at moments (separable by temper,
  not by diction alone). The broad-cast voicing held, but the dialog analog of the
  R545 thinning showed faintly at the edges of two adjacent registers.
- **The store-consistency ≠ coherence residue, surfaced by the blind extractor:** on the
  shelter road the murder is gt-asserted (sc-24) with ZERO in-world knower on that branch
  — a ground-truth narration with no character informant. The judges correctly read this
  as the road's POINT (no one is meant to learn it), not a break, so it is not a render
  defect; but it is the irreducible gt-narration boundary (the narrator knows what no
  character does). The in-frame omniscience check stays JUDGED (the R539 acquisition gate
  is YAGNI); here the judges found none.
- **n=1** — one premise / author-A / author-B / extractor / 3 judges. One instance, not
  a distribution. A weak premise could have depressed both axes at once (the R518
  conflation risk) — mitigated by two separate blind subagents + separable pins, but the
  same premise carried both.

## Control cross-reference (orchestrator; judges blind)

- R515 — warm render beat compliance 3-0 for NARRATION; this extends warm-render-then-
  gate to DIALOGUE at breadth, again zero repairs.
- R541 — a broad 12-frame cast scored 5/5 cast-distinctness as an OUTLINE; here the same
  breadth holds as RENDERED DIALOGUE (voice-distinctness 5/4/5).
- R545 — the broad cast THINNED in deep tails; here, with the sustainment clause, PIN-2
  held (12/12 every tail) and judged sustainment-feel scored 5/5/5 — the blemish was
  brief-fixable, with the one shelter sc-27 epilogue-drift the residue.

## As-built deviations (faithful)

- The author registered 12 person-frames (brief asked ≥10) and 32 scenes (asked 30-34),
  fork at sc-15 (≈47%). `--rules` resolves CWD-from-repo-root, so it was passed absolute
  while `--sidecar`/`--order` stayed relative (a CLI path quirk, no effect on results).
- PIN-2 is a JSON scan of facts × order (no new tool, per the manifest); the per-world
  fidelity projection is a read-only filter of the blind re-extraction (recorded above).
- Tracked: manifest, premise, the four briefs, runbook, the authored
  sections/facts/order/narrative-rules JSON, the re-extraction sections/facts JSON,
  vocab.md, author-log / render-log / extract-log, the 3 judge verdicts, this report.
  Scratch `*.atomic.json` (the rebuilt + author + re-extraction stores) stay gitignored.

SSOT = this file + the R550 / R551 ledger entries.
