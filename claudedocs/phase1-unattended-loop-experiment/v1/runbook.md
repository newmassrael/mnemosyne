# Runbook — unattended-loop-experiment/v1 (orchestrator only)

Operational glue for EXECUTING the experiment defined in `manifest.json`. The
manifest is the SSOT for the design, the sha-pinned pins, the firewall, and the
decision rule; this file is just the order of operations + the bootstrap prompt.
The executing session is the ORCHESTRATOR — it sets up the fresh workspace,
spawns the blind subagents, and runs the deterministic VERIFICATION. It does NOT
author, repair, render, or judge (the R469 contamination bound). Self-report is
not trusted (R500): every pin is re-derived by the orchestrator on a FRESH
rebuild from the agent's JSON, never read off the agent's own store or log.

## THE ONE PROMPT (paste into a fresh session to execute)

> Execute `claudedocs/phase1-unattended-loop-experiment/v1/` per its
> `runbook.md` and `manifest.json`. You are the ORCHESTRATOR (this lineage may
> not author, repair, render, or judge). Run the steps in order: set up the fresh
> game workspace, spawn the blind loop agent, verify PIN-1 on a fresh rebuild,
> audit PIN-2 from the loop-log, assemble the render road's outline + quest
> briefing, spawn the blind render, re-extract and run PIN-3's leak + fidelity
> gates, spawn the 3 blind judges, apply the pre-committed decision rule, write
> `report.md`, and append the changelog entry + commit. Do NOT push.

## Step 0 — preflight (orchestrator)

- `git config core.hooksPath .githooks` set; working tree clean.
- CLI carries the loop verbs: `mnemosyne-cli describe-schema | head -1`,
  `mnemosyne-cli propose-verdict` (errs on missing `--manifest`, not "unknown
  command"), `mnemosyne-cli report-authoring-frontier`. If any is "unknown
  command", `cargo install --path crates/mnemosyne-cli --force` first.
- Create the blind loop agent's fresh workspace under `run/game/`:
  `mnemosyne.toml` = `[workspace]`, `docs/.atomic/workspace.atomic.json` =
  `{ "schema_version": 23, "sections": {}, "changelog_entries": {} }`.

## Step 1 — blind loop agent (the experiment proper)

Spawn ONE fresh-context subagent with Bash. Hand it ONLY `premise.md` +
`loop-agent-brief.md` (absolute paths) and tell it its workspace is the absolute
path to `run/game/`. It runs the whole generate→propose-verdict→repair→frontier
loop UNATTENDED and leaves, in `run/game/`: `sections.json`, `facts.json` (+ any
`facts-N.json`), the applied store, and `loop-log.md`. It is BLIND to this
runbook, the manifest, the pins, the decision rule, and the render/judge stages.
The orchestrator does not help it — no hints, no repairs.

## Step 2 — PIN-1 (orchestrator, deterministic; self-report NOT trusted)

- Rebuild FRESH into `run/verify/`: empty schema-23 seed → `import-sections`
  (agent's `sections.json`) → `import-facts` each of the agent's `facts*.json`
  in the order the loop-log applied them. The JSON is the source of truth, not
  the agent's store.
- `propose-verdict --manifest <each facts*.json>` against the rebuilt store =
  every fact `no-op` at `commit` (already-applied ⇒ idempotent; proves the JSON
  re-applies clean).
- `report-authoring-frontier --telling <plan>` on the rebuilt store = **0
  zero-fact scenes, 0 dangling setups, 0 unresolved quests** (never-planned
  disclosures may be > 0 by design).
- `validate-workspace` on the rebuilt store = clean.
- Record every command + output verbatim. PIN-1 = converged AND re-derivable.

## Step 3 — PIN-2 (orchestrator, audit of the loop-log)

- Read `run/game/loop-log.md`. For each repair iteration, confirm it cites a
  specific `propose-verdict` violation or a `report-authoring-frontier` gap —
  i.e. the surfaces DROVE the convergence, it was not luck or source-reading.
- Confirm the agent used `describe-schema` and did NOT read crate source/tests
  (the brief forbids it; the log should show contract/gate use only).
- Tally: iteration count, distinct violation rules hit, gaps closed. Record the
  points where the agent logged friction / a guess / missing contract info —
  that list IS the experiment's deliverable (the next real gap).

## Step 4 — slice + briefing (orchestrator)

- Pick the render road: the terminal world-line on which the withheld secret
  comes out AND the quest resolves (the fuller road).
- `report-playthrough-manuscript --world <road> --telling <plan> --order` →
  `run/manuscripts/world-<road>.md` (frame-labelled, disclosure-annotated — the
  render's outline).
- `report-quest-graph --telling <plan>` (+ `report-playable-world` for the giver
  locators) → `run/briefing/quest-briefing.md`: the objective, the central
  choice, and this road's outcome. Player-facing; no ids.

## Step 5 — blind render (PIN-3 input)

Spawn ONE fresh-context subagent with ONLY `render-brief.md` + the outline
`world-<road>.md`. It leaves `run/render/world-<road>.md` + `render-log.md`.
BLIND to the store, the plan, the pins.

## Step 6 — PIN-3 (orchestrator gates + blind judges)

- Write `run/extract/vocab.md` (entity + predicate + frame ids + the fork point
  ONLY). Spawn ONE blind extractor with the rendered slice + vocab → it leaves
  `run/extract/reextracted.atomic.json`.
- leak: `validate-disclosure-leak --telling <plan> --against
  run/extract/reextracted.atomic.json --world <road> --truth-frame ground-truth`
  → expect 0 leaks, vocab_shared > 0, exit 0.
- fidelity: `validate-render-fidelity` over the re-extraction vs the order →
  expect off_path 0, unplaced 0, reached_terminal true.
- Any flag → scene-scoped warm repair; record the repair COUNT.
- Spawn 3 fresh-context judges, each with ONLY `judge-brief.md` +
  `quest-briefing.md` + the rendered slice → `run/judges/judge-{n}.md`.

## Step 7 — decide + record

- Apply the manifest's `decision_rule_pre_committed`.
- Write `report.md` (the SSOT): PIN-1/2/3 verdicts verbatim, the judge verdicts,
  the decision, and — headline — the honest NEXT GAP the loop-log surfaced.
- Append the changelog entry (self-contained, R452) + commit. Do NOT push.

## Tracked vs gitignored

Tracked: this dir's `*.md` + `manifest.json` + `report.md` +
`run/game/{sections,facts*}.json` + `loop-log.md` + `run/manuscripts/*.md` +
`run/briefing/*.md` + `run/render/*.md` + `run/extract/*.json` (non-atomic) +
`run/judges/*.md`. Gitignored (scratch): `*.atomic.json` (per `.gitignore`).
