# A/B Authoring Experiment — Run Log

Manifest pin: `30ed5ed6…1f7e` (R470 ledger entry; full hash in the ledger).
Model for S1–S6 (same tier both arms): claude-fable-5

## Session table

| Session | Role | Date | Model | Tokens in | Tokens out | Wall time | Notes |
|---|---|---|---|---|---|---|---|
| S1 | plain authoring 1 | 2026-06-11 | claude-fable-5 | 3,547 | 30,916 | 10m 2s | $2.92; cache read 266.8k / write 54.0k; 29 scenes planned (sc-01–08 trunk delivered, sc-09–29 skeleton in continuity ledger); author added continuity ledger header (non-prose) to story.md |
| S2 | plain authoring 2 | 2026-06-11 | claude-fable-5 | 3,747 | 40,515 | 20m 37s | $4.64; cache read 1.1m / write 74.2k; 10 scenes: limb A sc-09–16 + fork-2 CHOICE (sc-17 reveal / sc-20 destroy), limb B sc-23–24; continuity ledger updated w/ payoff-debt list; agent self-committed a6e263c (S1 agent left commit to orchestrator — minor procedural asymmetry, deliverable unaffected) |
| S3 | plain authoring 3 | 2026-06-11 | claude-fable-5 | 3,647 | 43,615 | 17m 0s | $4.74; cache read 795.3k / write 86.9k; final 11 scenes (sc-17–22 inserted, sc-25–29 appended), 3 endings marked, ledger payoff matrix claims all setups paid on all world-lines; self-committed 7978506. Mechanical check: 29 sc-NN headers, 2 CHOICE blocks, 3 ENDING markers. PLAIN ARM COMPLETE — totals: $12.30, out 115,046 tokens, 29 scenes |
| S4 | loop authoring 1 | 2026-06-11 | claude-fable-5 | 7,747 | 59,317 | 17m 41s | $7.71; cache read 2.6m / write 102.6k; sc-01–08 + fork-1 CHOICE (confront→sc-09 / trace→sc-25); store: 2 frames, 3 branches, 14 entities, 4 predicates+rules, 23 facts; gates green (orchestrator re-ran validate-continuity: 0 violations); repairs-log = 2 entries (§-prefix coordinate reject, rules-file shape reject x3) — substrate-friction class, not story-defect class; self-committed acd5d07 |
| S5 | loop authoring 2 | 2026-06-11 | claude-fable-5 | 7,547 | 63,415 | 18m 26s | $7.84; cache read 2.2m / write 119.4k; 10 scenes (confront sc-09–16 + fork-2 CHOICE sc-17 reveal / sc-21 destroy; trace sc-25–26); store 47 facts, branches reveal/destroy forks_from confront at sc-16; gates green (orchestrator re-ran: 0 violations, order_nodes=18); repairs-log +1 GENUINE gate catch (canon-order per-branch shape reject — map vs edge-pair sequence) — still substrate-friction class on first read, classify at S11; payoff state logged (confront family 5/6 paid, gt-page-liss deliberately dangling = fork-2 material); self-committed 023e8eb |
| S6 | loop authoring 3 | 2026-06-11 | claude-fable-5 | 3,947 | 53,015 | 14m 9s | $6.70; cache read 1.8m / write 112.0k; final 12 scenes (reveal sc-17–20, destroy sc-21–24, trace sc-27–30), 3 endings; gates green (orchestrator re-ran: 0 violations, 71 facts; payoff 6/6 in all 3 finished worlds — confront 5/6 + trunk 0/6 = interior-limb/inherited-setup semantics, not defects); 0 repairs this session; self-committed 19eb683. LOOP ARM COMPLETE — totals: $22.25, out 175,747 tokens, 30 scenes |
| S7 | shuffle + extraction | 2026-06-11 | claude-fable-5 | 10,249 | 235,112 | 57m 8s | $42.80; cache read 23.8m / write 355.2k; shuffle sealed 15:16 KST (see seal section); both stories re-extracted under one recipe (A: 145 facts, 6 rules; B: 136 facts, 7 rules); PRIMARY ENDPOINT story-A=0 (D1-D4 all 0), story-B=1 (D1=1: boots custody fork-boundary fault sc-15→sc-21 DESTROY world, rule_exclusive_overlap); recorded-not-counted: B QUIET warrant cites receipt discovered on unchosen limb (2nd fork seam, non-custody); B setup-5 planted per-limb not trunk; A soft hem seam fork-2; playthrough honesty surfaces all empty (6 walks); 3 matched pairs assembled (confront-reveal / confront-destroy / quiet); tooling caveat logged: engine ignores rule-level subject field (per:"subject" instantiation) — recipe encoded as exclusive+transition w/ subjects in comments; extraction repo 9bce829, pairs staged to judging 17d11a4 |
| S8 | judge 1 | 2026-06-11 | claude-fable-5 | 3,649 | 41,314 | 13m 8s | $5.36; cache read 419.9k / write 142.1k; choices: B (confront-destroy), A (confront-reveal), B (quiet) — B 2–1; cited A: trunk day-count drift, hem unpicked-vs-torn branch-joint contradiction (matches extractor craft note), quiet-line knowledge leak (Mara uses Caul-POV specifics; "two T.C.s"); cited B: terminal italic fragments x3 (= S7 ARTIFACT, see deviations), 1913-stone vs "twelve years dead" vs GIVEN BACK 1923 date arithmetic; verdict quarantined as verdict-1.md |
| S9 | judge 2 | 2026-06-11 | claude-fable-5 | 3,649 | 24,214 | 9m 50s | $3.72; cache read 441.3k / write 101.9k; choices: A, A, A (sweep, narrow margins); NEW B findings: 12+ booked passengers never accounted after wreck (all worlds), quiet-B climax rests on details established only on other limbs (fee book→iron chest, ruled line — same fork-seam class as extractor's receipt note), reveal-B horn-button seam declared retroactively (matches judge 1's craft critique); A findings: trunk "four days old" day-count (matches judge 1), "two T.C.s" slip; B artifact fragments cited again (S7 artifact, ruled); NOTE: B scored 5/5 character-knowledge discipline on all three lines — "sweep is about whole-world bookkeeping, not writing quality"; per-judge tally now 1–1, per-pair A 4 / B 2; verdict-2.md quarantined |
| S10 | judge 3 | 2026-06-11 | claude-fable-5 | 946 | 33,517 | 15m 37s | $4.35; cache read 424.1k / write 112.2k; choices: B (confront-destroy), A (confront-reveal), A (quiet) — judge prefers A 2–1; NEW A findings: confront-destroy world abandons the eight hidden survivors + narrator promise ("would learn she had written down the name of a living soul") never kept on that line — A's OWN fork-seam class (trunk promises only some branches repay); B: horn-button retroactive setup (3rd independent citation), 1913/"twelve years"/1923 date error in 2 of 3 copies, artifact fragments cited again (ruled); B destroy-line praised: burning rationally motivated, mother-revelation double cost; verdict-3.md quarantined. FINAL PREFERENCE TALLY — per judge: A 2–1 (J1:B, J2:A, J3:A); per pair: A 6 / B 3; per world: reveal A 3–0 unanimous, destroy B 2–1, quiet A 2–1 |

## Label map seal (S7)

- label-map.txt sha256: b8f71a6d89137fdb35f25ea2fdc33ec582838bd30397394680e347e7f2f5bc31
- sealed at (timestamp): 2026-06-11T15:16:45+09:00
- copy integrity: story-A.md and story-B.md byte-identical to their sources
  (md5 verified at copy time)

## Quota check (S7, mechanical counts only)

| | story-A | story-B |
|---|---|---|
| scenes total (>=24) | 29 — PASS | 30 — PASS |
| fork points (=2) | 2 CHOICE blocks — PASS | 2 CHOICE blocks (3rd grep hit = format preamble) — PASS |
| world-lines (>=3) | 3 — PASS | 3 — PASS |
| endings (>=3) | 3 (ENDING 1/2/3 headers) — PASS | 3 (**ENDING** markers) — PASS |
| 6 required setups present | yes (arm-side final verification) | yes (arm-side gate, 6 setups registered) |

Both deliverables meet quota → deterministic leg AND preference leg both valid
(manifest validity_conditions).

## Prevented defects (loop arm, from repairs-log.md — PRED-3 economics row)

- count: _____
- list: _____

## Deviations / incidents

(record anything that departed from the runbook, with timestamp)

- 2026-06-11 S1: author added a non-prose "continuity ledger" header to
  story.md (brief asked for scenes only). Allowed to stand: it is the plain
  arm's own working memory inside its single deliverable file, which the
  brief's continuation clause ("everything you have written so far")
  permits; graded artifact remains the scene prose. Noted for S7 — the
  extractor works from explicit scene text; the ledger is not scene text
  and must not be treated as scenes (it carries no sc-NN scene headers, so
  the scene-id extraction path is unaffected).
- 2026-06-11 S2: author committed its own session-end snapshot (a6e263c);
  S1's author left committing to the orchestrator. Procedural asymmetry
  only; no content effect.
- 2026-06-11 S7 shuffle: the copy-integrity md5 check printed source-copy
  pairings into the orchestrator transcript, exposing the label mapping to
  the owner. The 3 LLM judge sessions (the pinned preference procedure)
  remain blind — they never see that transcript. Consequence: the OPTIONAL
  owner read (judge 4, recorded separately, never pinned) is no longer
  blind; if performed, it must be reported as unblinded.
- 2026-06-11 post-S7: mapping explicitly disclosed to the owner on direct
  request (story-A = plain, story-B = loop), before judge sessions S8-S10.
  Judge blinding unaffected (fresh sessions, labels only). Formal grading
  still happens at S11 against the sealed file.
- 2026-06-11 S8: judge 1 surfaced an S7 normalization fault — all three
  copy-B files end with an orphaned italic fragment (story-B's
  "**ENDING ...** *synopsis*" construct: the bold marker was stripped, the
  trailing italic synopsis survived; story-A's ending markers sat in scene
  headers, so A copies are unaffected → ASYMMETRIC artifact penalizing B).
  Ruling: copies stay as-is for judges 2-3 (within-leg input parity beats
  removing a known, recorded artifact); at S11, artifact-class citations
  are excluded from PRED-4 residue and the preference reading notes the
  anti-B surface bias (judge 1 chose B 2-1 despite it).
