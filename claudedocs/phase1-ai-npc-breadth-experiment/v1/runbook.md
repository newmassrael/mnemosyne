# Runbook — ai-npc-breadth-experiment/v1 (orchestrator only)

The owner types ONE word (`실험` / `experiment`) to authorize execution. Everything
below is the orchestrator's deterministic glue around the blind subagents. The
orchestrator NEVER authors a fact or judges (R469 firewall — see manifest.json).

## Tool prerequisites (de-risked at the R540 manifest round)

- `mnemosyne-cli` on PATH is rebuilt to R539-head (schema 23; `cargo install --path
  crates/mnemosyne-cli --force`). Runs require `mnemosyne.toml` as a CWD ancestor, so
  the author works INSIDE the repo at `run/author/` and uses relative `--sidecar
  store.atomic.json` / `--order order.json` (R538: explicit CLI paths are CWD-relative).
- The seed store is `schema_version: 23`. The per-NPC dossier verb
  `report-frame-view --frame <P> --branch <W> --entity <E> --at <S>` is the breadth-floor
  evidence; confirmed working on a throwaway 5-frame diamond before this round.

## Sequence

0. **Pin (R540, done at the manifest round):** `sha256 manifest.json` recorded in the
   R540 ledger entry, committed before any subagent runs.

1. **Blind author (R541):** spawn ONE fresh-context subagent whose entire input is
   `premise.md` + `author-brief.md` (no manifest, no pins, no rubric). It authors
   `sections.json` + `facts.json` + `order.json` + the gate-clean `store.atomic.json`
   + `author-log.md` in `run/author/`. It self-checks with the gates.

2. **Deterministic pins (orchestrator):** rebuild the store FRESH from the author's
   `sections.json` + `facts.json` + `order.json` into a clean schema-23 seed (not the
   author's store file), then run PIN-B1 + PIN-B2 (manifest.json `deterministic_pins`)
   with `--order run/author/order.json`. Record every gate line verbatim in `report.md`.
   Dump each person-frame's `report-frame-view` (at its key scene) to
   `run/frame-views/frame-<P>.txt` — the breadth-floor evidence (>= 8 populated frames).

3. **Assemble manuscripts (orchestrator):** for each registered world-line W,
   `report-playthrough-manuscript --world W --order ... --sidecar ...` to
   `run/manuscripts/world-<W>.md`. Neutral; no pin/judge context in the file. (Facts are
   frame-labelled by the verb, so the judges can read who-knows-what.)

4. **Blind judges (R541):** spawn 3 fresh-context subagents, each given ONLY
   `judge-brief.md` + the `run/manuscripts/world-*.md`. Record verdicts verbatim in
   `run/judges/judge-{1,2,3}.md`.

5. **Decide + report (R541):** apply the pre-committed decision rule (manifest.json
   `decision_rule_pre_committed`), write `report.md` (pins + breadth-floor table + judge
   table + control cross-ref + the routed decision), commit.

## Control cross-reference (orchestrator, judges blind)

The R520 small-scale base (3 frames) was judged coherent 5/5; the R524 scale base (70
scenes) was judged coherent at depth. Cite both in report.md beside the judged breadth
verdict; the judges never see them. The open question is whether judged coherence holds
when the cast is BROADENED to 8+ individuated minds.

## Gate command reference (with --order, per the author-brief)

```
mnemosyne-cli validate-continuity            --order <order> --sidecar <store>
mnemosyne-cli report-fork-tree               --order <order> --sidecar <store>
mnemosyne-cli report-timeline-gaps --world <W> --order <order> --sidecar <store>
mnemosyne-cli report-payoff-coverage         --order <order> --sidecar <store>
mnemosyne-cli report-payoff-substantiation   --order <order> --sidecar <store>
mnemosyne-cli report-frame-view --frame <P> --branch <W> --entity <E> --at <S> --order <order> --sidecar <store>
mnemosyne-cli report-playthrough-manuscript --world <W> --order <order> --sidecar <store>
```
