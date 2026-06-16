# Runbook — time-travel-experiment/v1 (orchestrator only)

Operational glue for EXECUTING the experiment defined in `manifest.json`. The
manifest is the SSOT for the design, the substrate resolution, the pins, the
firewall, and the decision rule; this file is just the order of operations + the
bootstrap prompt. The executing session is the ORCHESTRATOR — it runs the
deterministic gates + the era/region/causation scans + the negative-control
removes + the slice/briefing assembly, and spawns blind subagents for everything
it may not do itself (author, render, re-extract, judge — the R469 contamination
bound).

## THE ONE PROMPT (paste into a fresh session to execute)

> Execute `claudedocs/phase1-time-travel-experiment/v1/` per its `runbook.md` and
> `manifest.json`. You are the ORCHESTRATOR (the R570 lineage may not author,
> render, re-extract, or judge). Run the steps in order: spawn the blind author,
> rebuild the store fresh, run PIN-1/2; run the PIN-3 + PIN-4 negative-control
> removes (remove a Founding setup, rebuild, confirm the Withering payoff dangles);
> run PIN-5; select the render future and assemble the outlines + the cross-age
> briefing; spawn the blind render; write the extractor vocab; spawn the blind
> extractor; run the render leak+fidelity gate; spawn the 3 blind judges; then
> apply the pre-committed decision rule, write `report.md`, and append the R571
> changelog entry + commit. Do NOT push.

## Step 0 — preflight (orchestrator)

- `git config core.hooksPath .githooks` is set; working tree clean.
- **Reinstall the CLI** so `report-playable-world` (R557) and `report-quest-graph`
  (R568) are present: `cargo install --path crates/mnemosyne-cli --force` (and
  restart mnemosyne-mcp if used). Confirm: `mnemosyne-cli report-quest-graph
  --telling x` errors on the store, NOT on `unknown command`.
- Create `run/{author,manuscripts,briefing,render,extract,judges}/`.

## Step 1 — blind author

Spawn ONE fresh-context subagent. Hand it ONLY `premise.md` + `author-brief.md`
(absolute paths). It works in `run/author/` and leaves `sections.json` /
`facts.json` / `order.json` / `narrative-rules.json` / `store.atomic.json` /
`author-log.md`. It is BLIND to this runbook, the manifest, the pins, the
substrate mapping, and the later stages.

## Step 2 — PIN-1 / PIN-2 (orchestrator, deterministic)

- Rebuild FRESH: empty schema-23 seed -> `import-sections` -> `import-facts`.
  Re-run from the author's JSON (the JSON is the source of truth, not the store).
- PIN-1: run every gate in the manifest's PIN-1 criteria; record verbatim.
- PIN-2 (era-as-order): scan `facts.json` for the shared-place Entities referenced
  by both a Founding-Age and a Withering-Age fact; confirm canon order places the
  Founding band strictly before the Withering band on the spine world-line (CanonOrder
  reachability past->future, never the reverse); confirm >= 1 shared prop `object`
  resolves to a MapLocator in BOTH ages (report-playable-world / report-disclosure-
  coverage); confirm >= 1 Founding `expected` fact is paid off by a Withering fact
  (R442 across time) and >= 1 Founding state supersedes into a Withering state
  (R547). Record the JOIN + how painful it was (the report-era-graph pull).

## Step 3 — PIN-3 + PIN-4 negative-control removes (orchestrator, deterministic)

The deterministic TEETH — proves the causation is structural, not prose-only.

- PIN-3 (same-place past->future): on the unmodified base confirm the chosen
  Founding setup (e.g. the cistern-seal) is paid off (not dangling) in the
  Withering terminal world(s). Then make a SURGICAL copy of `facts.json` with that
  ONE setup fact removed, rebuild fresh, re-run `report-payoff-coverage` +
  `validate-continuity`: confirm the Withering payoff now surfaces as dangling (or
  the succession gate flags the missing predecessor), and that the ONLY new
  violation traces to the removed cause. Record both runs verbatim.
- PIN-4 (cross-region cross-character): repeat the remove on the load-bearing
  Halden(Loom)->Ode(Fields) chain — remove Halden's Founding setup (region X,
  frame A), rebuild, confirm Ode's Withering payoff (region Y, frame B) dangles.
  Record the setup/payoff frame+region+era of the pair, and both runs verbatim.
- These removes are on SCRATCH copies; the canonical `run/author/facts.json` is
  unchanged. Keep the removed-fact copies under `run/author/neg-control/` (scratch,
  gitignored stores; the diff is recorded in report.md).

## Step 4 — PIN-5 optional-fork per-era state (orchestrator)

- `report-fork-tree --order`: confirm the planting fork is the single fork, 2
  Withering futures, each terminal.
- Confirm the far-field obligation is discharged in `planted` and open in `barren`
  (report-payoff-coverage per terminal). If the author registered a kind=quest
  entity for the cross-age objective, run `report-quest-graph --order` and confirm
  per-world Done/Open matches; else the payoff-coverage divergence alone satisfies
  PIN-5. If the optional act does not fork, record PIN-5 N/A (not a failure).

## Step 5 — slice + briefing (orchestrator)

- Pick the render future: the `planted` future (the richer one — most cross-age
  payoffs + the withheld-cause reveal).
- Assemble `run/manuscripts/world-trunk.md` (the Founding-Age trunk) +
  `world-planted.md` via `report-playthrough-manuscript --world W --telling wend
  --order` (frame-labelled, disclosure-annotated — the render's bible).
- Assemble `run/briefing/cross-age-briefing.md` — player-facing: each place in
  both ages; each cause->effect chain as early-act -> late-consequence with
  where/who/when; the optional fork's two futures; assembled from
  report-payoff-coverage + report-playable-world + the entity/claim JSON. (This
  hand-JOIN's painfulness is the tooling-pull signal.)

## Step 6 — blind render

Spawn ONE fresh-context subagent with ONLY `render-brief.md` + the two outline
files. It leaves `run/render/world-trunk.md` + `world-planted.md` + `render-log.md`.
BLIND to the original facts, the plan, the pins.

## Step 7 — blind extractor

Write `run/extract/vocab.md` (entity + predicate + frame IDS + forks_at ONLY).
Spawn ONE fresh-context subagent with ONLY `extractor-brief.md` + the rendered
slice + `vocab.md`. It leaves `run/extract/reextracted.atomic.json` (+ its
sections/facts JSON) + `extract-log.md`. BLIND to the original facts + plan.

## Step 8 — render leak + fidelity gate (orchestrator)

- leak: `validate-disclosure-leak --telling wend --against run/extract/reextracted.atomic.json --world planted --truth-frame gt` (expect 0 leaks of a withheld Founding cause before its first_at, vocab_shared > 0, exit 0).
- fidelity: `validate-render-fidelity` over the re-extraction vs `order.json`, the
  rendered future (expect off_path 0, unplaced 0, reached_terminal true).
- Any flag -> scene-scoped repair in the warm register, record the repair COUNT.

## Step 9 — blind judges

Spawn 3 fresh-context subagents, each with ONLY `judge-brief.md` +
`run/briefing/cross-age-briefing.md` + the rendered slice. Each leaves
`run/judges/judge-{n}.md`. BLIND to the experiment, each other, the AI-authorship.

## Step 10 — decide + record

- Apply the manifest's `decision_rule_pre_committed`.
- Write `report.md` (the SSOT): all pin verdicts verbatim (incl. both
  negative-control runs), the judge verdicts, the decision, the tooling-pull
  finding, honest caveats.
- Append the R571 changelog entry (self-contained) + commit. Do NOT push (the push
  consent gate).

## Tracked vs gitignored

Tracked: this dir's `*.md` + `manifest.json` + `vocab.md` + `report.md` +
`run/author/{sections,facts,order,narrative-rules}.json` + `author-log.md` +
`run/manuscripts/*.md` + `run/briefing/*.md` + `run/render/*.md` +
`run/extract/{sections,facts}.json` + `extract-log.md` + `run/judges/*.md`.
Gitignored (scratch): `*.atomic.json` + `*.playthrough.json` (per `.gitignore`),
incl. the `run/author/neg-control/` rebuild stores.
