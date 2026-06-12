# Scale-floor experiment (R473) — orchestration runbook

Design-side document. **Agent sessions (S1-S16) must never see this file, the
manifest, the predictions, the rubric internals, or the design conversation.**
It lives here (inside mnemosyne, outside the belvoir-* workspaces) for exactly
that reason. Protocol SSOT = `scale-floor-manifest.json` (sha256
`18ea118a…59d5`, pinned in the R473 ledger entry before any execution
session).

## Invariant: session separation IS the validity floor

- S1-S16 run as ISOLATED `claude` sessions that do not know the hypothesis,
  predictions, or rubric.
- Launch them from **outside** the mnemosyne project (in `/home/coin/belvoir-*`)
  so mnemosyne's CLAUDE.md, its memory, and this design doc are NOT auto-loaded.
  (The per-project memory path is keyed to the project dir, so a belvoir-*
  session has its own empty memory — no contamination.)
- Give each session ONLY its workspace `BRIEF.md`. Never paste this runbook,
  the manifest, or the predictions.

## Workspace map

| dir | role | sessions |
|---|---|---|
| `/home/coin/belvoir-plain` | plain arm authoring | S1-S6 |
| `/home/coin/belvoir-loop` | loop arm authoring (mnemosyne substrate) | S7-S12 |
| `/home/coin/belvoir-extraction` | blind re-extraction + defect tables | S13 |
| `/home/coin/belvoir-judging` | matched-world reading-copy judging | S14-S16 |

## Handoff rule (identical across arms = `handoff_parity`)

At the start of session N+1, auto-inject ONLY `{BRIEF.md, the agent's own
handoff notes, the immediately-preceding session's delivered prose}`. All
earlier prose stays in scene files on disk — the agent MAY open them at its own
token cost, but they are NOT auto-injected. This is the horizon operationalization;
any deviation across arms VOIDS the run.

## Token accounting (economics)

Per session record: total input / output tokens, split generation vs
re-read/query input. Form: `run-log.md` rows = session × {gen_tokens,
reread_tokens, scenes_delivered, wall_time}. The loop arm's gate catches +
repairs are logged as prevented defects (economics row, never mixed into the
defect endpoint).

## Step checklist

- **S1-S6 (plain):** isolated `claude` in `belvoir-plain`, give `BRIEF.md`.
  Each session appends to `story.md`, keeps its own handoff notes, `git commit`
  a snapshot. Enforce the handoff rule between sessions.
- **S7-S12 (loop):** isolated `claude` in `belvoir-loop`, give `BRIEF.md`. The
  agent authors store-first; at each session end it runs `validate-continuity`
  + `report-payoff-coverage` and repairs before delivering, logging to
  `repairs-log.md`. `git commit` a snapshot per session.
- **S13 (shuffle + blind extraction):**
  1. coin-flip label the two final `story.md` as `story-A` / `story-B`; write
     `label-map.txt` (which is plain, which is loop); record its sha256 in
     `run-log.md` IMMEDIATELY and seal it until S17.
  2. copy `story-A.md` / `story-B.md` into `belvoir-extraction`.
  3. isolated `claude` follows the extraction `BRIEF.md` — re-extract both,
     derive rules by the fixed recipe, produce the D1-D4 table per story.
- **S14-S16 (judges):**
  1. build matched-world reading copies: run `report-playthrough-manuscript
     --world <branch>` over each story's BLIND RE-EXTRACTED store (R470
     symmetry — the same referee orders both), inject prose by scene id,
     normalize formatting, strip ids/tool artifacts → `world-<name>-A.md` /
     `world-<name>-B.md`.
  2. match world-lines by fork-choice correspondence (confront/quiet ×
     confess/escalate); spine = confront path if structures diverge.
  3. three isolated `claude` judges (one matched world-line each) follow the
     judging `BRIEF.md`.
- **S17 (reveal):** unseal `label-map.txt`, re-verify its sha256, grade PRED-1
  (the only pin: plain ≥ 3 primary-endpoint defects AND loop < plain), measure
  PRED-3 (per-scene token ratio vs the A/B 1.48x) and PRED-4 (judge-cited
  errors invisible to the D-metrics, vs the A/B residue). Compile
  `scale-floor-report.md` (SSOT). Append a mnemosyne ledger entry restating the
  result table (R452 self-containment).

## What the orchestrator (who knows the design) MAY do

Extract/inject prompts, enforce the handoff rule, tally tokens, coin-flip the
shuffle, assemble reading copies, grade at S17 — all orchestration, not
authoring. (Honesty bound, already recorded in the manifest: designer ==
orchestrator knows the hypothesis; the pin + blinding cover tampering and
peeking, not design-stage selection bias.)

## What the orchestrator MUST NOT do

Author Belvoir prose (S1-S12), run the blind extraction (S13), or judge
(S14-S16). Those are isolation-only — a session that knows the hypothesis
cannot perform them without voiding the result.
