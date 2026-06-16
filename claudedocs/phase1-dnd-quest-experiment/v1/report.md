# dnd-quest-experiment/v1 — report (R562 execution)

**Question:** can an AI self-author a real game — a D&D-motif branching
dungeon-delve as a gate-clean fact base that instantiates the R559 fact→quest
contract AND the R557 map/surface seam together, such that the full pipeline
fact→quest→map coheres end-to-end and reads like an adventure a table would want
to play?

**Outcome: REACHES.** All four deterministic pins HOLD (zero render repairs);
3 blind judges score the game-feel axes majority 5/5 with one consistent 4/5
craft dock (agency). The R559 quest contract and the R557 map seam are
instantiable by a blind author and read as a real D&D adventure. Manifest
sha256 `36e6ecf6c2e8b644c0111462aa54ea4b214e7d7ffc29a3cd0eac5cda2daccb33`
(pinned in the R561 ledger before any subagent ran).

## What was authored (blind author A, fresh context, premise + author-brief only)

"The Drowned Hold of Vael Mooren" — 52 scenes; shared spine sc-01..sc-22, one
primary fork at sc-22 into three terminal roads SHATTER / CLAIM / PARLEY; 11
person-frames (gt + fighter, wizard, rogue, cleric, reeve, warden's shade,
shrine-keeper, lantern-boy, rival, wise-woman); 25 entities (incl. 4 of
kind=quest), 9 predicates, 121 facts. 4 gate iterations to converge. Crown + key
typed with an exclusive/per-object possession rule.

## Deterministic pins (orchestrator-run; the firewall lineage authored/judged nothing)

**PIN-1 authoring-clean — PASS.** Fresh rebuild from author-A's JSON loaded
clean (11 frames / 3 branches / 25 entities / 9 predicates / 121 facts, 0
errors). validate-continuity (with --rules): violations=0 (structural=0
interval=0). report-fork-tree: 3 world-lines, fork at sc-22 placed, 0 unplaced,
every road terminal. report-playthrough-manuscript per road: unplaced=0,
undecidable=0 (shatter/claim/parley). report-timeline-gaps per road: 0.
report-payoff-coverage: every dangling on a terminal world is author-log-declared
INTENDED (SHATTER opens f-060 reliquary + f-171 warden-parley; CLAIM opens f-041
delver + f-171; PARLEY opens f-060 reliquary) — no accidental hole.

**PIN-2 quest-contract well-formed — PASS** (the R559 contract made falsifiable
on authored data).
- (1) 4 entities of kind=quest: q-main, q-key, q-delver, q-reliquary.
- (2) each quest carries the contract: a typed `pursues` (q-main←fighter,
  q-key/q-delver←rogue, q-reliquary←cleric), exactly one giving fact
  (payoff_expectation=expected, lists the quest), and ≥1 completion fact
  (pays_off the giving): q-main on all 3 roads, q-key on the spine (f-161/f-180),
  q-delver on shatter (f-316) + parley (f-515), q-reliquary on claim (f-409,
  betrayal f-411).
- (3) prerequisite ORDER-REAL: q-main `requires` q-key (typed fact f-153); the
  key completes on the pre-fork spine (sc-17 recovered / sc-19 vault opens) and
  the main quest completes post-fork (sc-25{s,c,p}), so key < vault-open < fork
  < main on every road — a genuine lock-and-key gate, not a label.
- (4) PER-ROAD DIVERGENCE (the R559 "quest state DERIVED per world-line" claim):
  q-delver is discharged on SHATTER+PARLEY and OPEN on CLAIM; q-reliquary is
  discharged on CLAIM and OPEN on SHATTER+PARLEY — two quests diverging in
  opposite directions across the terminals. Quests are per-world obligations,
  not global flags.
- (5) every quest/pursues/requires/completion fact is gate-clean (PIN-1).

**PIN-3 map/surface resolves — PASS** (R557 exercised on real authored content
for the first time). report-playable-world --telling delve resolves all 4
quest-giving surfaces to MapLocators on every road, 0 unresolved:
q-main @ sc-02/reeve-hall (#1), q-delver @ sc-05/lantern-house (#4),
q-reliquary @ sc-07/shrine (#6), q-key @ sc-16/vault-door (#15);
undeclared_adjacencies=0, unplaced=0, undecidable=0 on all worlds.

**PIN-4 render-slice acceptance — PASS, ZERO repairs** (warm render = blind
author B over the CLAIM render bible; blind re-extraction = a third subagent).
- leak (validate-disclosure-leak --telling delve --world claim --truth-frame gt):
  leaks=0, vocabulary_shared=12 (the R510 F5 non-vacuous guard satisfied — 12
  typed truth-frame facts genuinely compared), 3 targeted withheld secrets, exit 0.
  No withheld secret (shrine-keeper cause / warden-reasonable / rival-betrayal)
  was re-extractable before its reveal scene.
- fidelity (validate-render-fidelity --world claim): off_path=0, unplaced=0,
  reached_terminal=true (the rendered CLAIM road's 87 re-extracted facts all sit
  on the claim world-line and reach its ending).
The warm-render-then-gate discipline (R515 narration, R551 dialogue) extends to a
QUEST-BEARING slice with no posture tax.

## Judged game-feel (3 blind judges, briefing + CLAIM prose; no pin)

| axis | J1 | J2 | J3 | majority |
|---|---|---|---|---|
| quest legibility | 5 | 5 | 5 | 5 |
| prerequisite / structure | 5 | 5 | 5 | 5 |
| agency / branch integrity | 4 | 4 | 4 | 4 |
| map coherence | 5 | 5 | 5 | 5 |
| knowledge realism | 5 | 5 | 5 | 5 |
| game-feel (overall) | 4 | 5 | 5 | 5 |
| **overall** | 4 | 5 | 5 | 5 |

All three: "a real adventure, not an outline wearing adventure clothes." The
lock-and-key spine (the Warden's Key gates the whole vault), the
Reliquary-trap/fork interlock, and reveals earned through play (Vane's guilt
surfaces in the delver's journal at sc-14, not blurted up top) carried it.

## The report-quest-graph PULL finding (the built-in probe)

PIN-2 and the player-facing quest briefing were a hand-JOIN over multiple
sources: facts.json (kind=quest entities + typed pursues/requires/completed_by +
payoff givings/completions) × report-payoff-coverage per terminal world (4 CLI
calls for discharge/open status) × order.json (the prerequisite ordering) ×
report-playable-world (the surface→MapLocator places). Composing the per-world
quest state required a custom multi-source script, not a single read. **This is a
real, registered pull for R559 option B (`report-quest-graph`)** — a verb that
emits a per-world QuestNode {objective, actor, derived state discharged/open,
prerequisites, completion fact, surface locator} would own exactly this JOIN. The
experiment is the consumer whose pull justifies building it; recommend R563 BUILD
report-quest-graph (the R556→R557 design→build-when-pulled pattern), reusing the
per-world reports verbatim (the R558 no-silent-drop lesson).

## Honest residue (not downplayed — feedback_dont_downplay_experiment_flaws)

- **The agency 4/5 dock is real and unanimous.** All three judges cited the SAME
  line at sc-30c ("came to the same end whichever way you turned it"), which
  flattens CLAIM's INTRA-road sub-choice (keep the crown vs hand it to Vane —
  both end with the crown ruling). This is a RENDER/CRAFT note on one line, NOT a
  structural failure: the three-road fork IS genuinely divergent (PIN-2-gated).
  But it is the honest gap — a correct, divergent quest graph can still be
  rendered with a line that undersells its own agency (the structure-consistency
  ≠ game-feel residue, the analog of store-consistency ≠ causal-coherence).
- **In-frame knowledge-acquisition seams (R539 boundary, judged not gated).** The
  blind extractor flagged three: (1) the delver's journal at sc-14 NAMES the
  shrine-keeper without the prose showing how the delver identified him; (2) Pip
  alone hears the incense clue at sc-06 yet the sc-14 synthesis draws on it with
  no shown hand-off; (3) an ending-count mismatch (a "fourth thing beyond three
  ends" vs the warden's "only three"). PIN-4 fidelity cannot catch in-frame
  acquisition (only cross-world), and the judges rated knowledge-realism 5/5/5 —
  so these rest on the blind judge, the same store-consistency ≠ causal-coherence
  residue. Real, small, named.
- **n=1**: one premise / one author / one render / one extractor / 3 judges.
- **render = a SLICE** (CLAIM road only); PIN-4 + the prose game-feel saw one
  road's prose; the other two roads were judged from the briefing, not played.
- **As-built deviation:** the disclosure plan lives in the store, NOT in the
  import-facts manifest (no `disclosure_plans` in facts.json), so a pure
  rebuild-from-JSON could not reproduce it — the orchestrator merged the author's
  store-resident plan into the fresh-rebuilt facts (the author store was first
  confirmed structurally identical to the rebuild). A minor substrate sharp-edge:
  there is no bulk import for a disclosure plan, only per-fact set-disclosure.
- **Boundary held (the R559/R546 line):** this tested the DECLARATIVE quest
  STRUCTURE (well-formed, gated, per-road-divergent, map-resolvable) — NOT a
  played stateful quest lifecycle (available/active/done/failed), which is
  SCE/pinion's. "The quests are well-formed and projectable" is not "the quests
  play."

## Decision (pre-committed rule, manifest decision_rule_pre_committed)

`all_pins_hold_and_judged_good` FIRES: all 4 pins hold AND judges majority ≥4 on
quest-legibility (5), agency (4), and game-feel-overall (5), with no major
knowledge/coherence break (knowledge 5/5/5). ⇒ the full pipeline fact→quest→map
COHERES end-to-end on a self-authored real game; the R559 quest contract + the
R557 map seam are instantiable by a blind author and READ as a real D&D
adventure. NO Mnemosyne substrate gap surfaced (substrate sufficient, the
R547/R556 refrain). The one open follow-on is the registered report-quest-graph
pull (R563 candidate, pull-justified).
