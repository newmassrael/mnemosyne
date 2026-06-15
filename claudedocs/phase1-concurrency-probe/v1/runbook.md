# Runbook — concurrency-probe/v1 (orchestrator-only operational glue)

One human bootstrap prompt → blind subagents. The orchestrator (this lineage) runs
the deterministic gates + measurement and NEVER authors a fact or judges the prose
(the R469 contamination bound). The manifest (`manifest.json`) is sha-pinned in the
R548 ledger BEFORE the author runs; do not edit it after the pin.

## Step 0 — pre-execution (R548, done)
- manifest.json sha256-pinned in the R548 changelog entry, committed.

## Step 1 — blind author (R549)
Spawn ONE fresh-context subagent with NO access to this runbook or the manifest:
> Read `claudedocs/phase1-concurrency-probe/v1/author-brief.md` and
> `claudedocs/phase1-concurrency-probe/v1/premise.md`. Do exactly what the brief
> says. Work in `claudedocs/phase1-concurrency-probe/v1/run/author/`. Leave
> sections.json, facts.json, order.json, store.atomic.json, author-log.md there.

The author is BLIND to: the concurrency hypothesis, the measurement (M1-M5), the
routing, and the R546/R547/R528 design. It just authors the story the premise asks
for, on the tools it is given.

## Step 2 — deterministic measurement (orchestrator)
Rebuild the store fresh from the author's sections.json + facts.json + order.json,
then run, recording every verdict/count verbatim into report.md:
```
mnemosyne-cli validate-continuity          --order order.json --sidecar <fresh>
mnemosyne-cli report-fork-tree             --order order.json --sidecar <fresh>
mnemosyne-cli report-timeline-gaps --world <each> --order order.json --sidecar <fresh>
mnemosyne-cli report-payoff-coverage       --order order.json --sidecar <fresh>
mnemosyne-cli report-payoff-substantiation --order order.json --sidecar <fresh>
mnemosyne-cli report-playthrough-manuscript --world <each> --order order.json --sidecar <fresh>
```
Compute M1 (interleaving fork tax), M2 (cart contention expressed?), M3 (AND-join
expressed?), M4 (author friction, quoted), M5 (concurrency dodge, which kind).

## Step 3 — optional blind judge
If simultaneity-coherence is ambiguous, spawn 1-3 blind judges over the assembled
manuscript(s): "do the two crews read as genuinely working the same night at once,
contending and interfering, or as two separate stories / one fixed sequence?"

## Step 4 — decide (orchestrator)
Apply the pre-committed decision rule, distinguishing OUTCOME-2a (clean concurrency
idiom found) from 2b (concurrency not needed by a told story = the R547/R546
layer-split confirmed). Write report.md; commit (R549).
