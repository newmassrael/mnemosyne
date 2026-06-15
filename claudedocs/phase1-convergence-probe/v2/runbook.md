# convergence-probe/v2 — orchestrator runbook

Operational glue for the orchestrator. The science is in `manifest.json` (sha-pinned
in the R536 ledger before execution). This file is how to RUN it; it is NOT read by
the blind author.

## Roles (R469 contamination bound — the 9th use of the blind-experiment family)

- **Orchestrator** (this lineage — built R532-R535): writes manifest + brief, reuses
  the v1 premise, de-risks the tool contract, runs gates + measurement + the negative
  control. Authors NO fact of the experiment's story.
- **Blind author** (ONE fresh-context subagent): given ONLY `premise.md` +
  `author-brief.md`, authors the fact base in `run/author/`. Blind to the hypothesis,
  the v1 result, the measurement, the routing.
- **Blind judge** (OPTIONAL, 1 fresh-context subagent): the shared-ending coherence
  read, only if ambiguous.

## Steps

0. **(done in R536)** manifest sha-pinned + committed pre-execution; tool contract
   de-risked on a throwaway minimal diamond (the exact confluence incantation folded
   into the brief); seed schema_version = 23; `--sidecar`/`--order` ABSOLUTE-path rule
   documented.

1. **Spawn the blind author.** Hand it `premise.md` + `author-brief.md` only, pointed
   at `claudedocs/phase1-convergence-probe/v2/run/author/` as its work dir. It runs
   the TOP-DOWN write->gate->repair loop to a gate-clean store and leaves
   `sections.json`, `facts.json`, `order.json`, `store.atomic.json`, `author-log.md`.

2. **Rebuild fresh + measure (orchestrator, independent of the author's claim).**
   From an EMPTY schema-23 seed, re-import the author's `sections.json` + `facts.json`,
   then run every gate with the author's `order.json` (ABSOLUTE paths):
   - `validate-continuity --json` — expect 0 violations (PIN_2 / M3a).
   - `report-fork-tree` — expect the DIAMOND: `converges from <parent> at <coord>` x
     each parent (M2).
   - `report-playthrough-manuscript --world sluice` and `--world ride` — expect the
     shared-ending facts present in BOTH, same fact-ids, 0 unplaced (M4 / PIN_2).
   - `report-payoff-coverage` / `report-payoff-substantiation` /
     `report-timeline-gaps --world <each>` — expect clean.
   - Count the shared-ending facts on the confluence branch; confirm none duplicated
     onto a fork branch (M1 / PIN_1). Record the v1(12) -> v2 ratio.

3. **Negative control (PIN_3 negative leg).** Copy the gate-clean store; mutate ONE
   confluence fact's `evidence` to cite a path-exclusive scene (a sluice-only or
   ride-only middle scene); re-import to a throwaway store; run `validate-continuity
   --json`; confirm it fires exactly `confluence_evidence_unreconciled` naming that
   fact + the unreached parent. Discard the mutated store.

4. **(optional) Blind judge** the per-world manuscripts for the coherence signal.

5. **Decide + report (R537).** Apply the pre-committed decision rule; write
   `report.md` (every M1-M5 verdict + count verbatim, the PIN results, the OUTCOME);
   commit. Push only on the owner's explicit push word.

## Gotchas (from the R536 de-risk)

- The empty seed store MUST be `schema_version: 23` (R532 bumped 22->23 for
  `Branch.converges_from`).
- `--sidecar` and `--order` given as RELATIVE paths resolve relative to the WORKSPACE
  ROOT, not the CWD — always pass ABSOLUTE paths (a stray repo-root `store.atomic.json`
  is the symptom of getting this wrong).
- A confluence branch carries `converges_from: [{branch, at}, ...]` (>= 2 parents,
  each pre-registered) and OMITS `forks_from`/`forks_at` (fork XOR confluence).
- The confluence's shared ending is wired in `order.json` by a merge edge
  `[<parent-last-scene>, <shared-ending-first-scene>]` in EACH parent's branch chain,
  plus the shared ending's own internal chain under the confluence branch.
- You walk playthroughs with `--world <parent>` (e.g. sluice, ride) — NOT
  `--world dawn`; the confluence is the shared continuation, excluded from the
  per-world sweep (R533 query_worlds), its facts surfacing inside each parent's walk.

## Tracked vs scratch

Tracked (committed): manifest.json, premise.md, author-brief.md, runbook.md,
report.md, run/author/{sections.json, facts.json, order.json, author-log.md}.
Gitignored: every `*.atomic.json` (the scratch stores).
