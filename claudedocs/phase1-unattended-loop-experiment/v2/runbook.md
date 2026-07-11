# Runbook — unattended-loop-experiment/v2 (orchestrator only)

A leaner confirmation of v1: does R595 (wire format) + R596 (canon-order gap)
remove the two frictions v1's loop hit? `manifest.json` is the SSOT (pins,
firewall, decision rule); this is the order of operations. Self-report is not
trusted (R500): the pins are re-derived on a fresh rebuild.

## Step 0 — preflight (orchestrator)

- CLI carries R595/R596: `mnemosyne-cli describe-schema` shows a "manifest wire
  format" section + a "canon order" section; `report-authoring-frontier` on an
  orderless fact-bearing store reports `unordered scenes`. If not, `cargo install
  --path crates/mnemosyne-cli --force`.
- Create the fresh loop workspace `run/game/`: `mnemosyne.toml` = `[workspace]`,
  `docs/.atomic/workspace.atomic.json` = the empty schema-23 seed.

## Step 1 — blind loop agent

Spawn ONE fresh-context subagent with Bash. Hand it ONLY `loop-agent-brief.md`
(v2) + `../v1/premise.md` (the same task) + the absolute path to `run/game/`. It
runs the whole loop unattended and leaves `sections.json`, `facts.json`, any
canon-order artifact it authored, the applied store, and `loop-log.md`. BLIND to
this runbook, the manifest, the pins. No orchestrator help — no hint about the
wire format or the canon order (those must come from `describe-schema`).

## Step 2 — PIN-v2-1 (orchestrator, loop-log audit)

Read `run/game/loop-log.md`. Count the propose-verdict calls that were WIRE-FORMAT
discovery probes (fixing a serialization error, not a content violation).
PIN-v2-1 holds iff <= 2 (v1 needed 11). Record what, if anything, the agent still
had to reverse-engineer.

## Step 3 — PIN-v2-2 (orchestrator, fresh rebuild)

Rebuild fresh into `run/verify/` from the agent's `sections.json` + `facts.json`
(+ its canon-order artifact, applied the way the loop-log did). Run
`report-authoring-frontier --telling <plan>` — PIN-v2-2 holds iff 0 unordered
scenes AND 0 zero-fact / 0 dangling / 0 unresolved, with the order authored by
the AGENT (the orchestrator supplies none). `validate-workspace` clean.

## Step 4 — decide + record

Apply the manifest's `decision_rule_pre_committed`. Write `report.md` (pins
verbatim, the probe count vs v1's 11, whether the agent authored an order, the
honest finding). Append the changelog entry + commit. Do NOT push.

## Tracked vs gitignored

Tracked: `*.md` + `manifest.json` + `report.md` + `run/game/{sections,facts*}.json`
+ any order artifact + `loop-log.md`. Gitignored: `*.atomic.json`.
