# Runbook — unattended-loop-experiment/v3 (orchestrator only)

The full end-to-end loop again, on today's surfaces, with the half v2 dropped:
does the loop still converge, and does it choose CONTENT a blind reader calls a
game? `manifest.json` is the SSOT (question, pins, firewall, decision rule); this
is the order of operations. Self-report is not trusted (R500): every pin is
re-derived on a fresh rebuild from the agent's own JSON.

## Step 0 — preflight (orchestrator)

- **The CLI is `$MN`, never a PATH copy**: `MN="$(git rev-parse --show-toplevel)/scripts/mn"`
  builds this tree's source on every call, so today's surfaces are carried exactly.
  Never `cargo install` into `~/.cargo/bin` — that slot is shared with the consumer
  checkouts on this machine (Round 823).
- Smoke check that the contract is the one being tested: `"$MN" describe-schema`
  prints the manifest wire format, the canon-order section, and the disclosure
  paragraph that says a clean frontier is necessary-but-not-sufficient.
- Create the fresh loop workspace `run/game/`: `mnemosyne.toml` = `[workspace]`,
  `docs/.atomic/workspace.atomic.json` = an empty seed at the schema version
  `describe-schema` reports. Nothing else — no order file, no rules file.

## Step 1 — blind loop agent

Spawn ONE fresh-context subagent with Bash. Hand it ONLY `loop-agent-brief.md`
(v3) + `../v1/premise.md` (the same premise as v1 and v2) + the absolute path to
`run/game/`. BLIND to this runbook, the manifest, and the pins.

⚠ **The brief must not name the leak gate, the fidelity gate, or
`report-playthrough-manuscript`.** PIN-v3-2b is void if it does — the pin asks
whether `describe-schema`'s own paragraph carries the agent past frontier-clean,
which is exactly R598's repair under test.

## Step 2 — PIN-v3-2 and PIN-v3-2b (orchestrator, loop-log audit)

Read `run/game/loop-log.md`. For each repair, name the gate output it traces to;
any repair with no named output fails PIN-v3-2. Then look for a render-acceptance
check the brief never mentioned: PIN-v3-2b holds iff the agent ran one AND its log
credits the contract. Record the iteration count and every friction, guess and
missing-contract-info point — that list is the deliverable.

## Step 3 — PIN-v3-1 (orchestrator, fresh rebuild)

Rebuild fresh into `run/verify/` from the agent's `sections.json` + `facts*.json`
+ its order artifact, applied the way the loop-log says. Then, on THAT store:
`propose-verdict` each manifest (must re-apply at `commit`),
`report-authoring-frontier --telling <plan>` (no gap of any axis except
never-planned disclosures), `validate-workspace` and `validate-continuity` clean.
The agent's own store is not evidence.

## Step 4 — render road + blind render (orchestrator assembles, agent writes)

Pick ONE road. The orchestrator assembles the outlines and the quest briefing
from the verified store (`report-playthrough-manuscript`, `report-quest-graph`)
and hands them, with `../v1/render-brief.md`, to a fresh blind render agent. The
orchestrator writes no prose.

## Step 5 — blind re-extraction + the render gates

A separate fresh agent re-extracts facts from the rendered prose alone (blind to
the store). Then the orchestrator runs the disclosure leak gate and
`validate-render-fidelity`: PIN-v3-3's first half holds iff leaks = 0 with
`vocab_shared > 0`, and off_path = 0 / unplaced = 0 / reached_terminal = true.

## Step 6 — 3 blind judges

Three fresh agents, `../v1/judge-brief.md`, the rendered road only, no store and
no knowledge of the pins. PIN-v3-3's second half holds iff a MAJORITY return
Overall game-feel >= 4/5 AND Coherence >= 4/5 with 'a real game? yes'.

## Step 7 — decide + record

Apply the manifest's `decision_rule_pre_committed` — as written, not as it reads
after the fact. Write `report.md` with every pin's output pasted verbatim, the
comparison to v1 on the same premise, and the honest next gap. Append the
changelog entry + commit.

## Tracked vs gitignored

Tracked: `*.md` + `manifest.json` + `report.md` + `run/game/{sections,facts*}.json`
+ the order artifact + `loop-log.md` + the rendered road + the re-extraction.
Gitignored: `*.atomic.json`.
