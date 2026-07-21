# Runbook — 2d-projection-experiment/v1 (orchestrator only)

Operational glue for EXECUTING the experiment defined in `manifest.json`. The
manifest is the SSOT for the design, thesis, pins, firewall, and decision rule;
this file is just the order of operations + the bootstrap prompt. The executing
session is the ORCHESTRATOR — it runs the deterministic gates + the map/quest
scans + the slice assembly, and spawns blind subagents for everything it may not
do itself (author, render x2, re-extract x2, judge — the R469 contamination
bound). EXECUTION IS A CONSENT GATE (the owner's `실험` / `experiment` /
`execute` word); do not run the blind stages without it.

## THE ONE PROMPT (paste into a fresh session to execute)

> Execute `claudedocs/phase1-2d-projection-experiment/v1/` per its `runbook.md`
> and `manifest.json`. You are the ORCHESTRATOR (the R749 lineage may not author,
> render either axis, re-extract, or judge). Run the steps in order: spawn the
> blind author, rebuild the store fresh, run PIN-1/2/3, select the render road
> and assemble the two shared outlines, spawn the TWO separate blind renderers
> (VN + tsukuru) on the SAME outlines, write the extractor vocab, spawn the blind
> extractor twice (one per slice), run PIN-4 (leak + fidelity per axis + the
> cross-axis reveal-order invariance), spawn the 3 blind judges on BOTH slices,
> then apply the pre-committed decision rule, record the disclosure-trigger pull
> finding, write `report.md`, and append the execution changelog entry + commit.
> Do NOT push.

## Step 0 — preflight (orchestrator)

- `git config core.hooksPath .githooks` is set; working tree clean.
- **Reinstall the CLI** so `report-playable-world` (R557), `validate-disclosure-leak`,
  and `validate-render-fidelity` are present:
  `cargo install --path crates/mnemosyne-cli --force` (restart mnemosyne-mcp if
  used). Confirm a verb runs against a store, NOT `unknown command`.
- Create `run/{author,manuscripts,render-vn,render-tsukuru,extract,extract-vn,extract-tsukuru,judges}/`.

## Step 1 — blind author

Spawn ONE fresh-context subagent. Hand it ONLY `premise.md` + `author-brief.md`
(absolute paths). It works in `run/author/` and leaves
`sections.json` / `facts.json` / `order.json` / `narrative-rules.json` /
`store.atomic.json` / `author-log.md`. BLIND to this runbook, the manifest, the
pins, the two-axis projection, and every later stage.

## Step 2 — PIN-1 / PIN-2 / PIN-3 (orchestrator, deterministic)

- Rebuild FRESH: empty schema-23 seed -> `import-sections` -> `import-facts` from
  the author's JSON (the JSON is the source of truth, not the store file).
- PIN-1: run every gate in the manifest's PIN-1 criteria; record verbatim.
- PIN-2: scan `facts.json` — `kind:place` entities + `adjacent` typed facts
  (connected graph) + `>= 2` edge-guards whose condition is a key fact; confirm
  the key chain is order-real (staff-key before staff-room, master-key before
  exit, in every world-line where both occur, via `order.json` reachability);
  confirm the `geot` frame locates the party while no party frame locates geot on
  the matching scene (`report-frame-view`); scan `kind:quest` entities x typed
  `pursues`/`requires`/`completed_by` x `report-payoff-coverage` per terminal
  world for the discharged-on / open-on divergence; confirm each open quest is
  author-log INTENDED. Record verbatim.
- PIN-3: `report-playable-world --telling play --order`; confirm every
  quest-giving surface AND every withheld-secret surface resolves to a MapLocator
  (0 unresolved) and honesty surfaces (unplaced / undecidable) = 0.

## Step 3 — slice (orchestrator)

- Pick the render road: the terminal world whose walk has the most quest givings
  and `>= 1` withheld-secret reveal.
- Assemble `run/manuscripts/world-trunk.md` + `world-<road>.md` via
  `report-playthrough-manuscript --world W --telling play --order` (the structural
  outline per scene: what is true, who is present, what they know, which quest
  threads + which secret first-reveals fall on this road). BOTH renderers receive
  these SAME two files — the only controlled variable is the render form.

## Step 4 — TWO blind renderers (separate, blind to each other)

- (a) VN: spawn ONE fresh subagent with `render-vn-brief.md` + the two outlines
  -> `run/render-vn/world-*.md` + `render-log.md`.
- (b) tsukuru: spawn ONE fresh subagent (separate) with `render-tsukuru-brief.md`
  + the SAME two outlines -> `run/render-tsukuru/world-*.md` + `render-log.md`.
  Its `render-log.md` note (a pinned reveal awkward to seat in non-linear space)
  is the disclosure-trigger pull signal — preserve it verbatim.

## Step 5 — blind extractor x2 (one per slice)

- Write `run/extract/vocab.md`: the entity / predicate / frame ids + the fork
  scene + the two road names ONLY (no claims, no facts, no plan).
- Spawn the blind extractor TWICE (fresh each): VN slice -> `run/extract-vn/`,
  tsukuru slice -> `run/extract-tsukuru/`. Each: `reextracted.atomic.json` +
  `extract-log.md`, blind to the original facts and to the other axis.

## Step 6 — PIN-4 (orchestrator)

- Per axis A in {vn, tsukuru}: `validate-disclosure-leak --telling play --against
  run/extract-A/reextracted.atomic.json --world <road> --truth-frame gt` (0 leaks,
  vocab_shared > 0); `validate-render-fidelity --against ... --world <road>
  --order` (off_path 0, unplaced 0, reached_terminal true). Record per axis (incl.
  any repair count).
- INVARIANCE: for each withheld secret (S1/S2/S3), compare the first-reveal scene
  in the VN re-extraction vs the tsukuru re-extraction on the rendered road; they
  must match. Record any divergence (which axis moved which reveal, and why) — the
  recorded pull.

## Step 7 — 3 blind judges

Spawn 3 fresh subagents (`judge-brief.md` + BOTH `run/render-vn/world-<road>.md`
and `run/render-tsukuru/world-<road>.md`) -> `run/judges/judge-{1,2,3}.md`. No
frame labels, no plan, no internal reports.

## Step 8 — decide (orchestrator)

Apply the manifest's pre-committed decision rule. Record the disclosure-trigger
pull finding (the tsukuru render-log craft note + whether a first_at was
railroaded + any PIN-4 invariance divergence). Write `report.md`. Append the
execution changelog entry (self-contained, R452) + commit per `COMMIT_FORMAT.md`.
**Do NOT push** (the push consent gate).

## Tracked vs scratch

Tracked evidence (commit): the briefs, `premise.md`, `manifest.json`, this
`runbook.md`, `report.md`, `run/author/author-log.md`, both `render-log.md`, both
`extract-log.md`, the 3 `judge-*.md`. Scratch (gitignored): the `*.atomic.json`
stores, the JSON manifests, the rendered `world-*.md` slices (large) — keep them
for the run, reference by path in `report.md`, do not bloat the tree.
