# Scale-floor experiment (R473) — run log

Protocol SSOT: `scale-floor-manifest.json` (sha256 `18ea118a…59d5`).
Economics (PRED-3, measured not pinned): per session, generation vs
re-read/query input tokens. The re-read column is the scale signal — it should
rise with corpus length for the plain arm and stay flat (query-bounded) for the
loop arm.

## Plain arm (belvoir-plain)

| session | scenes | gen (output) | reread tokens | cost | wall (API) | snapshot |
|---|---|---|---|---|---|---|
| S1 | sc-01..10 (10) | 42.7k | ~0 (first session, no prior prose) | $1.97 | 11m48s | a2cbc48 |
| S2 | sc-11..20 (10) | 40.0k | ~0 (only S1 prior; full read = normal handoff) | $2.27 | 10m8s | 1df5713 |
| S3 | sc-21..30 (10) | 60.9k | story.md opened 1x (drift fix) | $3.48 | 14m17s | ed03ac9 |
| S4 | sc-31..40 (10) | 60.4k | story.md opened 1x; STATE reads 3→5 | $3.71 | 15m11s | 2b9a37c |
| S5 | sc-41..50 (10) | 60.6k | story.md lookup 1x (WL-A); STATE reads 5→9 | $4.26 | 15m14s | 67faeff |
| S6 | sc-51..60 (10) | 52.1k | story.md opened 3x (WL-A resume + audit) | $3.73 | 12m48s | baf7d18 |
| **plain TOTAL** | **60 scenes, 3 endings** | **316.7k** | reread S3-S6 = 1/1/1/3 | **$19.42** | ~80m | baf7d18 |

S1 cache-read 452.6k = prompt cache of BRIEF + system, NOT old-prose re-read
(there is no prior prose yet). The reread signal starts at S3 when earlier
sessions sit on disk. ISOLATION VERIFIED from the S1 transcript: tool calls =
Read/Bash/Write only, 0 MCP, 0 mnemosyne-path access — the plain arm saw the
BRIEF and nothing else.

## Loop arm (belvoir-loop)

| session | scenes | gen tokens | query/reread tokens | prevented defects | wall time | snapshot |
|---|---|---|---|---|---|---|
| S7 (loop s1) | sc-01..10 | 98.4k | cli x34 (store-first) | 6 substrate repairs | 23m5s | 823568a |
| S8 (loop s2) | sc-11..20 | 47.5k | query/report x8, **story.md reread 0** | 0 (gates clean) | 11m28s | f5957d0 |
| S9 (loop s3) | sc-21..30 | 89.7k | query/report + story.md reread 3x; cache-write 256.7k | 3 design-hazard | 20m42s | 4106cca |
| S10 (loop s4) | sc-31..40 | 64.4k | query/report; story.md reread 1x; cache-write 117.6k | 0 (gates clean) | 16m6s | 0620cb8 |
| S11 (loop s5) | sc-41..50 | 93.7k | query/report + story.md reread 3x; cache-write 251.2k | 0 (gates clean) | 21m36s | f66610a |
| S12 (loop s6) | sc-51..65 | 108.4k | query/report + story.md reread 2x | 0 (gates green, typed flip OK) | 24m52s | cc75180 |
| **loop TOTAL** | **65 scenes, 3 endings** | **502.1k** | store-query primary | **$35.94** | ~117m | cc75180 |

## Handoff regime (enforced by orchestrator, identical across arms)

S(N+1) auto-inject = `{BRIEF.md, the agent's own handoff artifact (STATE.md /
notes), the immediately-preceding session's delivered prose}`. All earlier
prose stays in `story.md` on disk, re-readable at token cost but NOT
auto-injected. Notes (STATE.md) accumulate and ARE injected — that is the
arm's improvised external memory, the object of study. Injecting the FULL
accumulated prose each session would void the run.

## Session boundary map (for handoff excerpting)

| arm | session | scene range |
|---|---|---|
| plain | S1 | sc-01..sc-10 |
| plain | S2 | sc-11..sc-20 |
| plain | S3 | sc-21..sc-30 |
| plain | S4 | sc-31..sc-40 |
| plain | S5 | sc-41..sc-50 |
| plain | S6 | sc-51..sc-60 |

## Observations (raw signal for S17 — NOT graded here; S13 blind extraction counts the defects)

- **S3: first reread event.** STATE.md drifted from sc-03's actual prose — the
  agent's planned confront-crux ("Ottilie entered first with the master key
  before the alarm") contradicted sc-03, where she burns the chart AFTER the
  door is opened, in the minutes Cendre steps out. The agent opened story.md,
  caught the contradiction, rebuilt the CONFRONT limb on the real mechanics,
  and logged the correction in STATE §1/§4. Plain's improvised note began
  losing fidelity at ~20-30 scenes and recovered by re-reading — the
  cost-vs-fidelity tradeoff this experiment targets. Whether the delivered
  prose is ultimately clean is for the S13 blind extraction to count, not the
  orchestrator.
- **S4: reread persists, note-dependence rises.** story.md opened again (1x);
  STATE.md reads rose 3→5. No drift correction this session — the new §7B
  branch-root note carried the state forward cleanly. cache read 1.7m→2.1m. The
  emerging pattern: plain holds continuity via an ever-growing STATE.md plus
  periodic targeted reread, with cost rising each session ($1.97→2.27→3.48→3.71).
  The agent also self-maintained recent.md correctly (handoff discipline
  internalized) — the horizon regime holds without orchestrator excerpting.
- **S5: note-dependence accelerates.** STATE.md reads jumped 5→9 in one
  session; story.md lookup 1x — the flagged WL-A (sc-21..25) check the handoff
  regime FORCED, since recent.md held only the QUIET limb. cache read
  2.1m→3.0m, cost 3.71→4.26. The plain arm's "paper Mnemosyne" (STATE.md) is
  now the dominant working surface (~9 reads/session), and the horizon is doing
  its job: resuming an older limb required a deliberate reread the handoff alone
  could not supply. (S5 did NOT self-refresh recent.md — orchestrator
  re-excerpted to sc-41..50 for S6.) Trend over 5 sessions: gen output flat
  (~40-61k), but STATE-reads and cache-read climb every session — the cost of
  holding a long branching story in an improvised note is rising, defects still
  for S13 to count.
- **S6: plain arm complete, reread peaks.** story.md opened 3x — the most of
  any session — to resume WL-A (dormant since sc-25, 3 sessions back) and run
  the final cross-ending audit. 60 scenes, 3 endings, agent self-audit PASS
  (structural quota only — NOT a defect-clean claim). **MCP 0 across all six
  sessions = isolation clean throughout.** Plain-arm totals: $19.42, output
  316.7k, story.md reread by session = 0/0/1/1/1/3. Headline: improvised-note
  authoring carried continuity across 60 branching scenes, with reread cost
  concentrated where the handoff regime forced it (drift fix at S3, dormant-limb
  resumes at S5-S6). Whether the delivered prose is defect-clean is the S13
  blind-extraction question — the self-audit cannot answer it (the A/B lesson:
  the plain control's own store passed 0 while the blind referee found defects).

### Loop arm

- **S7 (loop s1): store-first build, 3x plain's first-session cost.** loop stood
  up the substrate (2 frames / 25 entities / 39 facts / 12 setups expected,
  canon-order + narrative-rules); gates 0 violations; 6 substrate-friction
  repairs (file shapes — the A/B first-session-friction pattern, NOT story
  defects). $5.95 vs plain S1 $1.97 (3x); output 98.4k vs 42.7k — initial store
  setup is front-loaded. Isolation clean (MCP 0, no mnemosyne-path or hypothesis
  access). **Parity correction (owner decision):** loop also wrote a free-form
  story-state bible to project memory (= plain's STATE.md role, an extra channel
  beyond the store); removed it so the contrast stays "structured store vs
  free-form note." The CLI-format cache (tool usage, not canon) is kept. loop
  S8+ uses the store as its only canon memory — whether store query alone holds
  continuity across 60 scenes is now the clean test.
- **S8 (loop s2): store replaces reread — the value proposition, live.**
  story.md opened 0 times; loop recovered all canon via store verbs (query x2 +
  report-payoff/irony/playthrough x6). Direct contrast: plain began opening
  story.md from S3 to recover/repair canon, and at S6 opened it 3x. Gates clean
  first-run (0 violations, 0 repairs — S7's substrate friction gone). Cost $3.25
  vs plain S2 $2.27 (1.43x, down from S7's 3x — setup was front-loaded). No
  free-form bible regenerated; only the CLI-format cache touched (tool usage,
  not canon). The store-first realignment held, and the clean test is answering:
  at scale, the loop recovers canon by query, not linear reread. (Defect count
  still S13's.)
- **S9 (loop s3): store-rewrite cost surfaces, prose reread returns.** Both
  FORK 1 limbs to the store (+21 facts/79 total, branch-tagged; per-branch
  canon-order edge sets), gates green first-run. $6.58 vs plain S3 $3.48 (1.89x,
  up from S8's 1.43x). Driver: cache write 101.8k→256.7k — the growing sidecar
  (26.7k→47.7k bytes) is rewritten WHOLE on each mutate, a loop-specific scale
  cost plain's small STATE.md append does not pay. story.md reread returned
  (0 at S8 → 3 at S9): the store gives canon facts, but prose continuity/flow
  still pulled targeted rereads — the store does not FULLY replace prose access,
  it shifts most of it to query. Substrate choice flagged: agent kept Cendre's
  working-theory UNTYPED across the fork (typing an exclusive predicate across
  both limbs risked an unordered-state violation) — a recording-granularity call
  that may bear on D1 at S13 (untyped beliefs are gated more weakly than typed).
  Isolation clean. Economics so far (3 of 6): loop 1.4-3x plain per session,
  no convergence below 1x yet — PRED-3 (loop cheaper at scale) NOT supported
  through the midpoint; the store's win is shifting reread→query, not lowering
  total cost.
- **S10 (loop s4): cost normalizes when work is single-limb.** QUIET limb +
  FORK 2 to store (104 facts, confess/escalate branches), gates green 0 repairs.
  $4.45 vs plain S4 $3.71 (1.20x — lowest loop ratio yet). cache write
  256.7k→117.6k confirms S9's dual-limb canon-order build was the spike, not a
  standing cost. story.md reread 1x (varies session to session with prose-flow
  vs pure-canon needs). store now 60.5k bytes. Loop economics through 4 sessions:
  3x / 1.43x / 1.89x / 1.20x — noisy, centered ~1.5x, never below 1x. The store
  buys reread→query substitution and clean first-run gates, not lower total cost.
- **S11 (loop s5): dual-limb spike repeats — the cost driver is now clear.**
  Both FORK 2 limbs to store (22 facts/126 total, deferred fork edges
  registered), gates green 0 repairs. $7.69 vs plain S5 $4.26 (1.81x). The
  single-vs-dual-limb split is the pattern: single-limb S8/S10 = 1.43x/1.20x,
  dual-limb S9/S11 = 1.89x/1.81x — writing two branches' facts+edges+canon-order
  is the driver (cache write 117.6k→251.2k, store 60→73k bytes rewritten whole).
  story.md reread 3x (matches S9's dual-limb 3x — prose-flow checks scale with
  limb count). Substrate discipline consistent: typed supersession stays
  single-branch (records-burned, clinical/life-status), the fork-point belief
  (helene-at-brink) kept UNTYPED so it splits two ways safely (the S3 lesson).
  Loop through 5 sessions = $27.92 vs plain's full $19.42 — loop will land ~1.6x
  plain, near the A/B 1.81x. (agent skipped its own commit at S11; orchestrator
  committed.) Isolation clean (MCP 0). S6 hazard flagged by agent: working-theory
  flip across 3 disjoint ending branches = the S3 cross-branch trap again.
- **S12 (loop s6): loop arm complete; the typed-flip hazard dissolves.** Three
  endings on disjoint branches (162 facts, 65 nodes, gates green). The S3-flagged
  cross-branch hazard did NOT materialize: the gate computes "holds" per-branch
  world-line, so disjoint-branch superseders of the spine token
  (suicide/suicide/murder) are never co-live — typed verdicts validated 0
  violations, no untyped fallback needed; agent even upgraded the diary to typed
  sealed→opened. The substrate's structural payoff: what plain tracked by hand in
  STATE.md, loop tracked as typed predicates the gate verifies. $8.02 (2.15x plain
  S6 — a 3-ending session). Isolation clean (MCP 0, no bible).

### Economics verdict (both arms complete)

- **Loop $35.94 / 502.1k output / 65 scenes vs plain $19.42 / 316.7k / 60 scenes.**
  Per-scene: plain $0.324, loop $0.553 = **1.71x** (vs A/B's 1.48x). **PRED-3
  (loop cheaper at scale, below 1.48x) REFUTED** — scale did NOT make the loop
  relatively cheaper; it held ~1.7-1.85x, near A/B. The store's measured win is
  QUALITATIVE: reread→query substitution (plain opened story.md 6x total over the
  run, climbing with corpus age; loop recovered canon by query), clean first-run
  gates (plain's S3 drift correction vs loop's repeated 0-repair sessions), and
  typed cross-branch verification the gate enforces. Not economic. Both arms ~20k
  words. The defect comparison — the actual endpoint — is S13's; nothing above is a
  defect count: the orchestrator records, the blind referee judges.

## S13 — shuffle + blind extraction

- **Shuffle:** coin flip into story-A / story-B, **orchestrator blind** — the
  flip result was never displayed, so even the orchestrator does not know the
  mapping until S17. label-map.txt SEALED, sha256
  `50092c400e8358d1803bcbf124d48f3661d64e63ab97317d182271feee152595` (reveal
  S17). story-A.md / story-B.md staged in belvoir-extraction; sizes withheld
  from the log to preserve blindness.
- **Extraction (done, $16.94 / 233.6k output / 48m):** blind re-extraction of
  both stories, fixed recipe, gates run live.
  - story-A: D1=0 D2=0 **D3=3** D4=0, endpoint **3** (82 facts / 60 scenes / 3 worlds)
  - story-B: D1=0 D2=0 **D3=5** D4=0, endpoint **5** (96 facts / 65 scenes / 3 worlds)
  - All recorded-not-counted surfaces = 0 for both (payoffs_to_unmarked,
    payoff_before_setup, cross_scope, unordered, succession_gaps, undecidable).
  - **De-facto unblind by scene count:** A=60=plain, B=65=loop (S17 label-map
    confirms). So plain endpoint=3, loop endpoint=5.
- **PRED-1 (plain≥3 AND loop<plain): REFUTED on the 2nd conjunct.** First
  conjunct MET — scale broke plain's floor (A/B plain=0 → scale plain=3). But
  loop=5 > plain=3, so loop<plain fails. Same direction as A/B: the loop does
  NOT produce fewer deterministic defects.
- **All signal in D3.** D1/D2/D4=0 both arms — every custody chain and
  alive→dead transition closes via a narrated succession edge; the legit
  suicide(confront/confess)/murder(escalate) divergence is branch-scoped, so it
  reads as cross_scope=0, not a defect.
- **Key finding — loop store-prose drift.** loop's own store self-marked
  "12/12 paid" (S12 audit), but blind prose re-extraction finds confess/escalate
  leave threads unresolved (forged papers, burned chart, name-match, sealed
  diary). The gate validates the STORE, not store-vs-prose agreement; loop's
  self-audit gave false assurance — the A/B lesson (plain's own store passed 0
  while the referee found defects) reproduced IN THE LOOP ARM. This is the
  sharpest product signal: a narrative store needs prose-vs-store drift
  detection (the spec side has R404 scan_content_drift; the narrative side does
  not).
- **B-1 ambiguity (extractor-flagged, not buried).** loop's extra dangling may
  be a NARRATIVE choice: the escalate ending lets the killer escape, so threads
  stay intentionally unresolved (dramatic irony held to the last line), vs
  plain's murder ending where Cendre confronts the in-house killer and resolves
  all. Two readings: investigator-resolution (A=3/B=5) vs reader-delivery
  (A=0/B=1); loop ≥ plain under both. Whether D3's gap is craft or defect is the
  S14-16 preference leg's question.
- **Contamination note (minor, recorded).** Extractor ran `cat
  belvoir-loop/mnemosyne.toml` + browsed saltglass-extraction (prior A/B) for
  store-toml FORMAT — did NOT open author canon (belvoir.atomic.json / story.md
  / STATE / canon-order never read; verified from transcript tool-inputs).
  D-metric is prose-only and unbiased. The procedural lapse: the extractor saw
  the loop arm exists + the A/B-comparison shape (gate fix: future extraction
  prompts must forbid leaving belvoir-extraction). Extractor also spawned 2
  subagents — their prompts carried no author-arm or hypothesis leakage (checked).

## S14-16 — preference leg (blind judges)

- **Reading copies assembled** (orchestrator, mnemosyne-cli + mechanical prose
  injection): 3 matched world-lines (confront ↔ confront, confess ↔
  quiet-confess, escalate ↔ quiet-escalate) × A/B = 6 files in belvoir-judging.
  Scene order from `report-playthrough-manuscript` over each blind RE-EXTRACTED
  store (R470 symmetry — same referee that graded defects orders the reading),
  prose injected by scene id, normalized (ids / SESSION markers / CHOICE / ENDING
  stripped, verified 0 leaks). Word counts ~9-14k per copy. Orchestrator did not
  read the prose (file-only assembly).
- **Judges:** 3 isolated sessions, one matched world-line each, A/B labels only,
  the reading copies + the 6-axis rubric in judge BRIEF. All 3 ran, all MCP 0 /
  store-and-hypothesis 0-hits (clean).
- **Judge 1 (confront) — forced choice A (=plain).** Scores A: setup 4 / stakes
  4 / char-knowledge **5** / sag 4 (one soft seam: the "has left Lyon" telegram
  vs Hélène's months-old residence). B: setup 4 / stakes 4 / char-knowledge 4 /
  sag 4 (steward subplot strains, boots-retraction, scraped-label motive
  unexplained). **THE FINDING — floor↔preference inversion:** plain's confront
  ending carries D3=3 dangling (floor: worse), yet the judge PREFERS it, reading
  the unpaid threads as intentional dramatic irony ("the road taken determines
  which face of the truth she reaches; the narration converts every unpaid
  thread into the climax"). loop's confront = D3 0 / near-total solution (floor:
  better), but the judge MARKS IT DOWN — "makes the branch point feel less
  consequential." Judge respects B's mechanical rigor (locked room genuinely
  solved, weapon=motive unified) but A wins on prose + knowledge-discipline +
  ending. **The gate rewards completeness; the reader rewards consequential
  choice.** B-1 answered for confront: plain's dangling was CRAFT, not defect —
  the D-metric and the human diverge in sign, not just magnitude.
- **Judges 2 (confess) & 3 (escalate) — both forced choice A. 3-0 sweep for
  plain.** BUT contaminated reading copies. loop's story-B.md leaked authoring
  metadata INTO the graded prose (gate output "validate-continuity 0 violations
  (126 facts)", "→ sc-41", branch-plan blocks, "UNTYPED beliefs") — 6
  occurrences; plain's story-A.md = 0. Per-world leak into the B reading copies:
  confront 0 / confess 4 / escalate 7 (all A copies = 0). Judges 2 & 3 EXPLICITLY
  cited the "out-of-fiction planning text" / "inline metadata" as breaking the
  fiction and damaging continuity/setup/sag. So confess/escalate preference is
  partly an artifact of (a) loop's deliverable-hygiene failure + (b) my
  normalization not stripping scene-internal metadata. **confront (0 leak) is
  clean: A wins on pure prose; the craft-vs-defect finding holds there.**
  confess/escalate need metadata-stripped re-assembly + re-judge for a fair prose
  comparison. **Owner chose re-judge.** Re-assembled confess/escalate B copies
  metadata-free (8 lines stripped from story-B, 0 leak verified, prose preserved:
  13932→13869, 14034→13932 words). confront + all A copies unchanged.
  - **1st re-strip was STILL incomplete** (line-wise filter missed multiline
    `<!-- -->` blocks); a re-judge re-flagged metadata. Root cause: the metadata
    is multiline HTML comments, and my filter + my verification shared the same
    blind spot (verification not independent of implementation). Fixed by cutting
    `<!--.*?-->` as DOTALL blocks (16 blocks removed, all comments, no prose;
    shown in full to owner), verified by provenance (0 removed-comment lines in
    the copies) — but NOT trusted as proof; the judge is the ground truth.
- **Re-judge done — A 3-0 (clean enough).** Both re-judges actively searched
  for non-story text first (per prompt). They found NO store-work metadata — the
  comment removal held. They DID surface residual branch scaffolding (CHOICE
  menus, ending codes like "(WL-B2 · QUIET-ESCALATE · MURDER)") my filter missed,
  but it is **SYMMETRIC**: confess A5/B5, escalate A5/B5, confront A3/B3 —
  identical on plain and loop. Unlike the asymmetric B-only store-notes, this
  residue is the same both sides; the judges read past it to prose. Not
  re-stripped (symmetric + infinite-regress risk; the lesson stands that
  post-hoc normalization never closes — input-side hygiene was the real fix).
- **PREFERENCE VERDICT: plain (A) wins all three world-lines, 3-0.**
- **Why loop lost — substantive, not artifact.** Judges cited loop's REAL plot
  faults that the store's 0-violations gate does not see. confess: "Surel" is
  called both "a borrowed coat" AND "my own name" — a self-contradiction at the
  load-bearing clue — plus an ominously-framed telegram that pays off nothing.
  escalate: the named murderer (Surel, a bed-patient) has NO established access
  to Roeder's private-room draught (the access the solution needs actually fits
  Brandt, the nurse) + kills Lindqvist with no motive + "six weeks" of
  investigation packed inside a "three-week" snow-in. **The store guarantees
  state-consistency (custody/transition closed, D1/D2/D4=0) but NOT causal
  plausibility or clue coherence — and that is exactly where the loop lost.**
  plain's dangling threads were intentional dramatic irony (craft); loop's were
  broken causation (defect). The D-metric counts both as "dangling"; the human
  judge separates craft from defect. This is the PRED-4 untyped-residue lesson,
  sharpened to a product claim: a narrative store that validates declared facts
  does not validate whether the story's causation hangs together.
- **Separate finding (real, not an artifact):** loop putting store-work traces
  in its GRADED prose is itself a loop-arm weakness — plain kept all notes in
  STATE.md, outside the deliverable; loop's store-first habit bled gate output
  and branch-plans into story.md. Even after a clean re-judge, this is a recorded
  hygiene gap (the deliverable contract is "prose scene network alone").
