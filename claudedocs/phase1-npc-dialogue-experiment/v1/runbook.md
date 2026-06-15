# Runbook — npc-dialogue-experiment/v1 (orchestrator only)

The owner types ONE word (`실험` / `experiment`) to authorize execution. Everything
below is the orchestrator's deterministic glue around the blind subagents. The
orchestrator NEVER authors a fact, renders a scene, re-extracts, or judges (R469
firewall — see manifest.json). Six blind subagents do that work, each fresh-context.

## Tool prerequisites

- `mnemosyne-cli` on PATH at schema 23 (`cargo install --path crates/mnemosyne-cli
  --force` if skewed). Runs require `mnemosyne.toml` as a CWD ancestor; subagents work
  INSIDE the repo under `run/<stage>/` with relative `--sidecar` / `--order`.
- Confirm the render-acceptance verbs exist before Stage 3:
  `mnemosyne-cli validate-disclosure-leak --help` and
  `mnemosyne-cli validate-render-fidelity --help` (R507/R508). Reinstall + restart MCP
  if skewed (the binary-skew lesson).

## Sequence

0. **Pin (R550, the manifest round):** `sha256sum manifest.json` recorded in the R550
   ledger entry, committed before any subagent runs.

1. **Stage 1 — blind author-A (authoring):** spawn ONE fresh-context subagent whose
   entire input is `premise.md` + `author-brief.md` (no manifest, no pins, no rubric,
   no render/judge briefs). It authors `sections.json` + `facts.json` + `order.json` +
   `narrative-rules.json` + the gate-clean `store.atomic.json` + a disclosure plan
   (telling `holm`) + `author-log.md` in `run/author/`.

2. **PIN-1 + PIN-2 (orchestrator):** rebuild the store FRESH from the author's
   `sections.json` + `facts.json` + `order.json` into a clean schema-23 seed (NOT the
   author's store file), import the disclosure plan, then:
   - **PIN-1** — run `validate-continuity --order --rules`, `report-fork-tree`,
     `report-timeline-gaps --world W`, `report-payoff-coverage`,
     `report-payoff-substantiation` (manifest `deterministic_pins.PIN_1`). Record every
     line verbatim. Dump each person-frame's `report-frame-view` to
     `run/frame-views/frame-<P>.txt`.
   - **PIN-2** — compute the deep-tail SUSTAINMENT metric (manifest
     `deterministic_pins.PIN_2`): for each world-line W, the deep tail = the scenes
     strictly after W's fork point on W's order chain; count distinct person-frames
     (≠ gt) holding ≥1 fact whose `canon_from` is a deep-tail scene of W; mark the
     top-3-by-total-fact-count frames as principals. Record the per-world table; check
     the floor (≥6 frames active per tail, ≥4 of them non-principal). Computed from the
     author's `facts.json` × `order.json` via `experiment-harness cast-sustainment`
     (R555 — Rust, fail-loud; replaced the throwaway scan that first ran this).

3. **Assemble structural outlines (orchestrator):** for each registered world-line W,
   `report-playthrough-manuscript --world W --telling holm --order order.json --sidecar
   store.atomic.json` → `run/manuscripts/world-<W>.md` (frame-labelled, disclosure-
   annotated). Neutral; no pin/judge context. These are the "bible" the render reads.

4. **Stage 2 — blind author-B (render):** spawn ONE fresh-context subagent whose input
   is `render-brief.md` + the three `run/manuscripts/world-*.md`. It writes
   `run/render/world-{report,shelter,confront}.md` (warm prose, `## sc-NN` scene
   headings) + `render-log.md`. BLIND to pins, rubric, hypothesis.

5. **Stage 3 — blind extractor:** orchestrator writes `run/extract/vocab.md` = the
   entity / predicate / frame IDS + descriptions + the branch `forks_at` (VOCABULARY
   ONLY — NO claims, NO facts, NO plan). Spawn ONE fresh-context subagent whose input is
   `extractor-brief.md` + `run/render/world-*.md` + `run/extract/vocab.md`. It writes
   `run/extract/reextracted.atomic.json` (+ sections.json/facts.json + extract-log.md).
   BLIND to the original facts/plan.

6. **PIN-3 (orchestrator):** over the re-extraction (manifest
   `deterministic_pins.PIN_3`):
   - **leak** — `validate-disclosure-leak --telling holm --against
     run/extract/reextracted.atomic.json --world W --truth-frame gt --order order.json
     --sidecar store.atomic.json` for each W. HOLD = 0 leaks, vocab_shared > 0 (the F5
     non-vacuous guard), exit 0.
   - **fidelity** — `validate-render-fidelity` over the re-extraction vs `order.json`
     per W. HOLD = off_path = 0, unplaced = 0, reached_terminal = true.
   Record verbatim. (If a gate flags, that is the render-then-gate-repair signal —
   record the repair count as the R515 posture tax; do not silently fix.)

7. **Stage 4 — blind judges:** spawn 3 fresh-context subagents, each given ONLY
   `judge-brief.md` + `run/render/world-*.md` (the rendered prose, no frame labels, no
   plan). Record verdicts verbatim in `run/judges/judge-{1,2,3}.md`. Judges blind to the
   experiment and to each other.

8. **Decide + report (R551):** apply the pre-committed decision rule (manifest
   `decision_rule_pre_committed`), write `report.md` (PIN-1/2/3 + the per-world
   sustainment table + the judge table + the cross-references + the routed decision),
   commit. Update the RESUME memory with the outcome + the new NEXT. Push only on the
   owner's explicit push word.

## Control cross-reference (orchestrator, judges blind)

- R515 — warm render beat the compliance render 3-0 for NARRATION (keystone 5.00 vs
  4.11). This experiment asks whether warm render extends to DIALOGUE at breadth.
- R541 — a broad 12-frame cast judged 5/5 cast-distinctness as an OUTLINE (authoring).
- R545 — depth×breadth combined SCALED, but the broad cast THINNED in deep branch
  tails (the blemish PIN-2 + the sustainment axis here operationalize). Cite all three
  beside the verdict; the judges never see them.

## Harness + CLI (R555 update)

Reuse `mnemosyne-cli` verbs verbatim; the PIN-2 deep-tail metric runs through
`experiment-harness cast-sustainment` and the per-world fidelity projection through
`experiment-harness project-world` (R555 — at execution time these were throwaway
Python, since elevated to the fail-loud Rust harness so every pin reproduces from the
tracked inputs). Scratch `*.atomic.json` / `*.playthrough.json` stay gitignored;
tracked evidence = manifest, premise, the four briefs, runbook, the authored
sections/facts/order/rules JSON, the disclosure plan record, the structural outlines,
the rendered manuscripts, the re-extraction sections/facts JSON, the frame-views, the
extract-log, render-log, author-log, the 3 judge verdicts, report.md.
