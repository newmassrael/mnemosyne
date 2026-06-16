# Runbook — dnd-quest-experiment/v1 (orchestrator only)

Operational glue for EXECUTING the experiment defined in `manifest.json`. The
manifest is the SSOT for the design, pins, firewall, and decision rule; this
file is just the order of operations + the bootstrap prompt. The executing
session is the ORCHESTRATOR — it runs the deterministic gates + the quest/map
scans + the slice/briefing assembly, and spawns blind subagents for everything
it may not do itself (author, render, re-extract, judge — the R469 contamination
bound).

## THE ONE PROMPT (paste into a fresh session to execute)

> Execute `claudedocs/phase1-dnd-quest-experiment/v1/` per its `runbook.md` and
> `manifest.json`. You are the ORCHESTRATOR (the R561 lineage may not author,
> render, re-extract, or judge). Run the steps in order: spawn the blind
> author, rebuild the store fresh, run PIN-1/2/3, select the render road and
> assemble the outlines + the player-facing quest briefing, spawn the blind
> render, write the extractor vocab, spawn the blind extractor, run PIN-4,
> spawn the 3 blind judges, then apply the pre-committed decision rule, write
> `report.md`, and append the R562 changelog entry + commit. Do NOT push.

## Step 0 — preflight (orchestrator)

- `git config core.hooksPath .githooks` is set; working tree clean.
- **Reinstall the CLI** so `report-playable-world` (R557) is present:
  `cargo install --path crates/mnemosyne-cli --force` (and restart mnemosyne-mcp
  if used). Confirm: `mnemosyne-cli report-playable-world --telling x` errors on
  the store, NOT on `unknown command`.
- Create `run/{author,manuscripts,briefing,render,extract,judges}/`.

## Step 1 — blind author A

Spawn ONE fresh-context subagent. Hand it ONLY `premise.md` + `author-brief.md`
(absolute paths). It works in `run/author/` and leaves
`sections.json` / `facts.json` / `order.json` / `narrative-rules.json` /
`store.atomic.json` / `author-log.md`. It is BLIND to this runbook, the
manifest, the pins, and the later stages.

## Step 2 — PIN-1 / PIN-2 / PIN-3 (orchestrator, deterministic)

- Rebuild FRESH: empty schema-23 seed -> `import-sections` -> `import-facts`.
  Re-run from author-A's JSON (the JSON is the source of truth, not the store).
- PIN-1: run every gate in the manifest's PIN-1 criteria; record verbatim.
- PIN-2: scan `facts.json` for `kind:quest` entities; for each, find its typed
  `pursues`, its giving fact (`payoff_expectation:expected` + entity listed),
  its per-road completion (`pays_off`); check the `requires` prerequisite is
  order-real (prereq completion precedes dependent completion in every shared
  world-line); cross `report-payoff-coverage --order` per terminal world for the
  discharged-on / open-on divergence; confirm each open quest is author-log
  INTENDED. Record the JOIN + how painful it was (the report-quest-graph pull).
- PIN-3: `report-playable-world --telling delve --order`; confirm every
  quest-giving surface resolves to a MapLocator (0 unresolved) and honesty
  surfaces unplaced/undecidable = 0.

## Step 3 — slice + briefing (orchestrator)

- Pick the render road: the terminal world whose walk has the most quest givings
  and >= 1 withheld-secret reveal.
- Assemble `run/manuscripts/world-trunk.md` + `world-<road>.md` via
  `report-playthrough-manuscript --world W --telling delve --order` (frame-
  labelled, disclosure-annotated — the render's bible).
- Assemble `run/briefing/quest-briefing.md` — player-facing: per quest, its
  objective, where it is taken up (the MapLocator place), its prerequisite, and
  its per-road outcome. (This hand-JOIN's painfulness is the option-B pull.)

## Step 4 — blind render B

Spawn ONE fresh-context subagent with ONLY `render-brief.md` + the two outline
files. It leaves `run/render/world-trunk.md` + `world-<road>.md` +
`render-log.md`. BLIND to the original facts, the plan, the pins.

## Step 5 — blind extractor

Write `run/extract/vocab.md` (entity + predicate + frame IDS + forks_at ONLY).
Spawn ONE fresh-context subagent with ONLY `extractor-brief.md` + the rendered
slice + `vocab.md`. It leaves `run/extract/reextracted.atomic.json` (+ its
sections/facts JSON) + `extract-log.md`. BLIND to the original facts + plan.

## Step 6 — PIN-4 (orchestrator)

- leak: `validate-disclosure-leak --telling delve --against run/extract/reextracted.atomic.json --world <road> --truth-frame gt` (expect 0 leaks, vocab_shared > 0, exit 0).
- fidelity: `validate-render-fidelity` over the re-extraction vs `order.json`,
  the rendered road (expect off_path 0, unplaced 0, reached_terminal true).
- Any flag -> scene-scoped repair in the warm register, record the repair COUNT.

## Step 7 — blind judges

Spawn 3 fresh-context subagents, each with ONLY `judge-brief.md` +
`run/briefing/quest-briefing.md` + the rendered slice. Each leaves
`run/judges/judge-{n}.md`. BLIND to the experiment, each other, the AI-authorship.

## Step 8 — decide + record

- Apply the manifest's `decision_rule_pre_committed`.
- Write `report.md` (the SSOT): all pin verdicts verbatim, the judge verdicts,
  the decision, the report-quest-graph pull finding, honest caveats.
- Append the R562 changelog entry (self-contained) + commit. Do NOT push (the
  push consent gate).

## Tracked vs gitignored

Tracked: this dir's `*.md` + `manifest.json` + `vocab.md` + `report.md` +
`run/author/{sections,facts,order,narrative-rules}.json` + `author-log.md` +
`run/manuscripts/*.md` + `run/briefing/*.md` + `run/render/*.md` +
`run/extract/{sections,facts}.json` + `extract-log.md` + `run/judges/*.md`.
Gitignored (scratch): `*.atomic.json` + `*.playthrough.json` (per `.gitignore`).
