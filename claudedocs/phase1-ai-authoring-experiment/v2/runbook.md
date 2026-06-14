# Runbook — ai-authoring-experiment/v2 (SCALE; orchestrator only)

The owner types ONE word (`실험` / `experiment`) to authorize execution. Everything
below is the orchestrator's deterministic glue around the blind subagents. The
orchestrator NEVER authors a fact or judges (R469 firewall — see manifest.json).

## Sequence

0. **Pin (R523, done at the manifest round):** `sha256 manifest.json` recorded in
   the R523 ledger entry, committed before any subagent runs.

1. **Blind author (R524):** spawn ONE fresh-context subagent whose entire input is
   `premise.md` + `author-brief.md` (no manifest, no pins, no rubric, no v1
   baseline). It authors `sections.json` + `facts.json` + `order.json` + the
   gate-clean `store.atomic.json` + `author-log.md` in `run/author/`, working
   TOP-DOWN (Phase 0 skeleton then Phase 1 detail) and using structural
   backreferences. It self-checks with the gates, including the R522
   evidence-reachability check.

2. **Deterministic pins (orchestrator):** run PIN-A1 + PIN-A2 (manifest.json
   `deterministic_pins`) over the author's final store, with `--order
   run/author/order.json`. Record every gate line verbatim in `report.md`. Note in
   the report whether the R522 `evidence_unreachable` gate fired during authoring
   (read `author-log.md`).

3. **Assemble manuscripts (orchestrator):** for each registered world-line W,
   `report-playthrough-manuscript --world W --order ... --sidecar ...` to
   `run/manuscripts/world-<W>.md`. Neutral; no pin/judge context in the file.

4. **Blind judges (R524):** spawn 3 fresh-context subagents, each given ONLY
   `judge-brief.md` + the `run/manuscripts/world-*.md`. Record verdicts verbatim in
   `run/judges/judge-{1,2,3}.md`.

5. **Decide + report (R524):** apply the pre-committed decision rule
   (manifest.json `decision_rule_pre_committed`), write `report.md` (pins +
   judge table + the two control cross-refs + the routed decision), commit.

## Control cross-references (orchestrator, judges blind)

1. The R520 v1 base (14 scenes) was judged unanimous 5/5 coherent — the small-scale
   bar. Does coherence hold at 4-5x the scale?
2. The scale-floor experiment (R473-R479) found prose-first authoring at ~60 scenes
   produced store-consistent-but-incoherent results. Does facts-first top-down
   authoring at the same scale avoid that? Cite both beside the judged coherence;
   the judges never see them.

## Gate command reference (with --order, per the author-brief)

```
mnemosyne-cli validate-continuity          --order <order> --sidecar <store>
mnemosyne-cli report-fork-tree             --order <order> --sidecar <store>
mnemosyne-cli report-timeline-gaps --world <W> --order <order> --sidecar <store>
mnemosyne-cli report-payoff-coverage       --order <order> --sidecar <store>
mnemosyne-cli report-payoff-substantiation --order <order> --sidecar <store>
mnemosyne-cli report-playthrough-manuscript --world <W> --order <order> --sidecar <store>
```
