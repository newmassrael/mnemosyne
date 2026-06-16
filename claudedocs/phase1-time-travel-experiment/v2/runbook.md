# Runbook — time-travel-experiment/v2 (orchestrator only)

Operational glue for EXECUTING the controlled re-render defined in `manifest.json`.
The manifest is the SSOT for the design, the pin, the firewall, and the decision
rule; this file is the order of operations + the bootstrap prompt. v2 is a
CONTROLLED RE-RENDER of v1's FROZEN base (the R553 discipline) — NO new author. The
executing session is the ORCHESTRATOR (R469 bound: may not render, re-extract, or
judge).

## THE ONE PROMPT (paste into a fresh session to execute)

> Execute `claudedocs/phase1-time-travel-experiment/v2/` per its `runbook.md` and
> `manifest.json`. You are the ORCHESTRATOR (the R572 lineage may not render,
> re-extract, or judge). Rebuild v1's FROZEN base fresh; assemble the arm-MYSTERY
> outline (the planted Withering walk sc-27p..sc-40p as the spine, the Founding
> causes revealed only at sc-39p); spawn the blind renderer; spawn the blind
> extractor over the MYSTERY prose; run PIN-M1 (leak + fidelity on MYSTERY, re-confirm
> v1 IRONY leak 1); assemble the A/B (arm IRONY = v1 prose, arm MYSTERY = v2 prose,
> randomized per judge); spawn 3 blind judges; apply the decision rule; write
> report.md; append the R573 changelog entry + commit. Do NOT push.

## Step 0 — preflight
- `git config core.hooksPath .githooks` set; working tree clean. CLI current (post-R568).
- Rebuild v1's frozen base FRESH from `../v1/run/author/{sections,facts,order,narrative-rules}.json`
  (+ carry the disclosure plan from v1's store): empty schema-23 seed -> import-sections
  -> import-facts -> copy `disclosure_plans` over. (This is exactly the R571 Step-2 rebuild.)
- Create `run/{render-mystery,extract-mystery,judges-v2}/`.

## Step 1 — arm-MYSTERY outline (orchestrator)
- Assemble `run/render-mystery/_outline.md`: the planted WITHERING walk
  (`report-playthrough-manuscript --world planted --telling wend --order ...`, scenes
  sc-27p..sc-40p ONLY — NOT the Founding trunk) as `## sc-NN` scenes, disclosure-
  annotated. At sc-39p add the explicit reveal brief: the bearer crosses back; the
  Founding causes (Sera's seal sc-04 content, Halden's stone sc-11 content) are first
  disclosed HERE, under sc-39p, never in any earlier Withering scene.

## Step 2 — blind renderer
Spawn ONE fresh-context subagent with ONLY `render-brief.md` + the arm-MYSTERY
outline. -> `run/render-mystery/world-planted.md` + `render-log.md`. BLIND to v1's
render, the pins, the leak finding, the A/B.

## Step 3 — blind extractor
Write `run/extract-mystery/vocab.md` (reuse v1's `../v1/run/extract/vocab.md`
verbatim). Spawn ONE fresh-context subagent with ONLY `../v1/extractor-brief.md` +
the MYSTERY prose + vocab. -> `run/extract-mystery/reextracted.atomic.json` (+ JSON)
+ `extract-log.md`. BLIND to the base + plan.

## Step 4 — PIN-M1 (orchestrator)
- MYSTERY leak: `validate-disclosure-leak --telling wend --against run/extract-mystery/reextracted.atomic.json --world planted --truth-frame gt` (expect leaks 0, vocab_shared > 0).
- MYSTERY fidelity: `validate-render-fidelity ... --world planted` (expect off_path 0, unplaced 0, reached_terminal true).
- IRONY re-confirm: re-run the leak gate over v1's re-extraction (`../v1/run/extract/reextracted.atomic.json`) = leak 1 (the R571 result) — the leak 1 -> 0 contrast on the SAME base.
- Any MYSTERY leak -> scene-scoped repair, record the COUNT (the (나) pull signal if it cannot clear).

## Step 5 — A/B judges
Assemble the A/B: arm IRONY = `../v1/run/render/{world-trunk,world-planted}.md`,
arm MYSTERY = `run/render-mystery/world-planted.md`. Randomize which is arm-1/arm-2
per judge; record the label map in `run/judges-v2/label-map.md`. Spawn 3 fresh-context
judges with ONLY `judge-brief-ab.md` + both arms + `../v1/run/briefing/cross-age-briefing.md`.
-> `run/judges-v2/judge-{1,2,3}.md`. BLIND to which arm is which, each other.

## Step 6 — decide + record
- Apply `decision_rule_pre_committed`; the PIN-M1 leak verdict is the (가)->(나) input
  (PASS => future-first EXPRESSIBLE on existing substrate => (나) = enforcement Q;
  needs-contortion => (나) substrate addition pull-justified).
- Write `report.md` (SSOT): PIN-M1 verbatim (leak 1 -> 0 contrast), the A/B verdicts,
  the decision, the (나) recommendation, honest caveats.
- Append the R573 changelog entry (self-contained) + commit. Do NOT push.

## Tracked vs gitignored
Tracked: this dir's `*.md` + `manifest.json` + `report.md` + `run/render-mystery/*.md`
+ `run/extract-mystery/{sections,facts}.json` + extract-log + `run/judges-v2/*.md` +
label-map. Gitignored: `*.atomic.json` (per `.gitignore` — the v1 exception covers
the whole `phase1-time-travel-experiment/` tree).
