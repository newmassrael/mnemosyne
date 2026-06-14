# Runbook — ai-authoring-experiment/v1 (orchestrator only)

The owner types ONE word (`실험` / `experiment`) to authorize execution. Everything
below is the orchestrator's deterministic glue around the blind subagents. The
orchestrator NEVER authors a fact or judges (R469 firewall — see manifest.json).

## Sequence

0. **Pin (R519, done at the manifest round):** `sha256 manifest.json` recorded in
   the R519 ledger entry, committed before any subagent runs.

1. **Blind author (R520):** spawn ONE fresh-context subagent whose entire input is
   `premise.md` + `author-brief.md` (no manifest, no pins, no rubric). It authors
   `sections.json` + `facts.json` + `order.json` + the gate-clean `store.atomic.json`
   + `author-log.md` in `run/author/`. It self-checks with the gates.

2. **Deterministic pins (orchestrator):** run PIN-A1 + PIN-A2 (manifest.json
   `deterministic_pins`) over the author's final store, with `--order
   run/author/order.json`. Record every gate line verbatim in `report.md`.

3. **Assemble manuscripts (orchestrator):** for each registered world-line W,
   `report-playthrough-manuscript --world W --order ... --sidecar ...` to
   `run/manuscripts/world-<W>.md`. Neutral; no pin/judge context in the file.

4. **Blind judges (R520):** spawn 3 fresh-context subagents, each given ONLY
   `judge-brief.md` + the `run/manuscripts/world-*.md`. Record verdicts verbatim in
   `run/judges/judge-{1,2,3}.md`.

5. **Decide + report (R520):** apply the pre-committed decision rule
   (manifest.json `decision_rule_pre_committed`), write `report.md` (pins +
   judge table + control cross-ref + the routed decision), commit.

## Control cross-reference (orchestrator, judges blind)

The hand-authored Meridian Vane base (R504/R514) passed its gates and its
world-lines were judged coherent — the small-scale coherence bar. Cite it in
report.md beside the judged coherence; the judges never see it.

## Gate command reference (with --order, per the author-brief)

```
mnemosyne-cli validate-continuity          --order <order> --sidecar <store>
mnemosyne-cli report-fork-tree             --order <order> --sidecar <store>
mnemosyne-cli report-timeline-gaps --world <W> --order <order> --sidecar <store>
mnemosyne-cli report-payoff-coverage       --order <order> --sidecar <store>
mnemosyne-cli report-payoff-substantiation --order <order> --sidecar <store>
mnemosyne-cli report-playthrough-manuscript --world <W> --order <order> --sidecar <store>
```
