# Runbook — continuity-stress-experiment/v1 (orchestrator only)

Does the unattended loop exercise the continuity gate's RULE CLASSES
(exclusive / transition / interval), and does the gate BITE on real authored
content? `manifest.json` is the SSOT (question, firewall, pins, decision rule);
this is the order of operations. Self-report is not trusted (R500): the pins are
re-derived on a fresh rebuild + a deterministic negative control.

## Step 0 — preflight (orchestrator)

- CLI is current: `mnemosyne-cli describe-schema` shows a "narrative-rule classes"
  section (exclusive / transition / interval). If not, `cargo install --path
  crates/mnemosyne-cli --force`.
- Create the fresh loop workspace `run/game/`: `mnemosyne.toml` = `[workspace]`,
  `docs/.atomic/workspace.atomic.json` = the empty schema-23 seed. Do NOT
  pre-wire any `[continuity].rules_path` — the agent must discover + do that
  itself (the surface under test).

## Step 1 — blind loop agent

Spawn ONE fresh-context subagent with Bash. Hand it ONLY `loop-agent-brief.md` +
`premise.md` + the absolute path to `run/game/`. It runs the whole loop
unattended and leaves `sections.json`, `facts.json`, its narrative-rules file,
its canon-order artifact, the wired `mnemosyne.toml`, the applied store, and
`loop-log.md`. BLIND to this runbook, the manifest, the pins. No orchestrator
help — no hint about the rules-file wire format or how to wire it (those must
come from `describe-schema` or the agent's own error-reading).

## Step 2 — PIN-1 (orchestrator, fresh rebuild)

Rebuild fresh into `run/verify/` from the agent's `sections.json` + `facts.json`
(+ its canon-order + rules file, wired as its `mnemosyne.toml` did). Confirm:
loaded rules contain >= 1 of EACH class (exclusive, transition, interval), each
predicate used by real typed legs; `propose-verdict` re-applies at `commit`;
`report-authoring-frontier --telling <plan>` = 0 zero-fact / 0 unordered /
0 dangling / 0 unresolved; `validate-workspace` clean. RECORD exactly which
classes landed (a class the agent could not declare is ABSENT — that is the
`surface_gap` signal, not a failure to hide).

## Step 3 — PIN-2 (orchestrator, deterministic negative control = the teeth)

For EACH rule the agent declared, author a MINIMAL rule-violating probe facts
manifest and run it through `propose-verdict` against the agent's final store
(rules wired). The R570 remove-and-rescan pattern:
- exclusive: a fact adding a SECOND concurrent holder of the custody object in
  one (frame x world) -> expect REJECT naming the exclusive rule.
- transition: a successor state fact forming an UN-declared `(from, to)` (e.g.
  barred -> open) -> expect REJECT naming the transition rule.
- interval: numeric legs whose `value(left) - value(right)` violates the op/bound
  -> expect REJECT naming the interval rule.
Then confirm the SAME store WITHOUT each probe passes (`commit`). A rule whose
violation does NOT reject = `teeth_dull` (a gate bug — stop and fix, build-first).
Keep every probe manifest + the verbatim verdicts.

## Step 4 — PIN-3 + judged-no-pin (orchestrator + blind render/judge)

- PIN-3: read `loop-log.md`. Record whether the gate fired on a rule reject during
  the agent's OWN iterations (natural teeth) or it authored rule-clean first-try
  (state it plainly); and every friction the agent hit declaring/wiring the rules
  (probe count, what it reverse-engineered) = the surface-sufficiency deliverable.
- Judged-no-pin: spawn a blind render agent (one road, warm, ~1-2k words) then a
  blind judge — real-game yes/no + world-logic legibility + coherence 1-5. Run
  the leak + fidelity gates on the render.

## Step 5 — decide + record

Apply the manifest's `decision_rule_pre_committed`. Write `report.md` (pins
verbatim, which classes landed, the PIN-2 verdicts, PIN-3's friction record, the
judged read, the honest finding). Append the changelog entry + commit. Do NOT
push.

## Tracked vs gitignored

Tracked: `*.md` + `manifest.json` + `report.md` + `run/game/{sections,facts*}.json`
+ the rules file + the canon-order artifact + `loop-log.md` + the PIN-2 probe
manifests. Gitignored: `*.atomic.json`.
