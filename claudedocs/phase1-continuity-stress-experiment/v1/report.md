# Report — continuity-stress-experiment/v1

**Question (R600 proof gap).** The continuity gate's RULE CLASSES
(exclusive / transition / interval = possession-custody / state-machine /
numeric-timing) were never exercised by the unattended loop — v1/v2's games had
no rule-tripping content. Does a blind loop agent, given a premise whose
world-logic REQUIRES all three classes and a brief to enforce it with the gate,
(a) DECLARE all three from the surface, (b) TYPE the content so the gate keys on
it, (c) does the gate BITE on a real violation, (d) self-repair? Manifest sha256
`184358e1db6a85a4ea8db81aecc39060b8a76107cfb8af3f539aac6cbc441aa6` (R602,
pre-execution pin). Firewall R469: one blind loop agent authored + wired + gated;
this lineage seeded the premise + workspace and re-derived every pin on a FRESH
rebuild (self-report not trusted, R500); the PIN-2 negative controls are
orchestrator VERIFICATION (the R570 remove-and-rescan pattern).

## Outcome: `classes_exercised` FIRES — the R600 proof gap is CLOSED, with a named surface_gap secondary finding.

A blind agent authored **"The Night Lock at Harrow Sluice"** (15 scenes, 4
frames, WORK road = main sc-01..12 + a HOLD fork, 38 facts, a withheld secret, a
quest, setup->payoff chains) and made the continuity gate enforce all three
world-invariants. All pins verified on a FRESH orchestrator rebuild from the
agent's `sections.json` + `facts.json` + `narrative-rules.json` +
`canon-order.json` + `mnemosyne.toml`.

### PIN-1 (declared + typed, from the surface) — HOLDS

On the fresh rebuild: `import-sections` + `import-facts` clean; `propose-verdict`
re-applies at `commit` (60 no-op, violations 0); `report-authoring-frontier
--telling default` = 0 zero-fact / 0 unordered / 0 dangling / 0 unresolved (only
37 never-planned disclosures = the intended withhold); `validate-continuity`
reports `facts=38 order_nodes=15 rules=3 interval_severity=reject violations: 0`;
`validate-workspace` clean. All THREE classes are declared and key on real typed
legs actually used by the facts:

| rule id | class | keys on (predicate) | typed legs |
|---|---|---|---|
| lantern-custody | exclusive `per:object` | `holds` (subject=holder, object=e-lantern) | 4 |
| gate-sequence | transition | `gate_state` (barred↔cracked↔open) | 5 |
| fill-hold | interval | `open_clock − fill_start_clock ≥ 30` (subj e-pound) | 1 + 1 |

### PIN-2 (the teeth BITE — deterministic negative control) — HOLDS

The orchestrator applied ONE minimal rule-violating probe per rule to a throwaway
clone of the agent's final store (rules auto-loaded from its wired toml) via
`propose-verdict`. Every probe REJECTED (rollback) for exactly its rule, and the
same store without the probe commits (PIN-1). The teeth are load-bearing on the
agent's real authored content — the R600 gap's core:

- exclusive: a second concurrent holder of `e-lantern` → `rule_exclusive_overlap`
  (5×, e.g. `f-lantern-wick` and `probe-excl` co-hold `holds` at sc-04).
- transition: a `barred → open` succession step → `rule_transition_invalid`
  (`e-gate` steps `barred -> open` outside the allowed set).
- interval: `open_clock` 200 (200−180=20 < 30) → `rule_interval_violation`
  (`e-pound` 200 - 180 not >= 30 at sc-11).

### PIN-3 (surface friction record) — the `surface_gap` finding

The agent DID declare all three (so PIN-1 held fully), but ONLY by
REVERSE-ENGINEERING an undocumented authoring surface. `describe-schema`
documents the rule CLASSES + parameters and that `--rules` / `canon_order_path`
exist, but NOT: (a) the rules-file JSON wire shape, (b) the toml key that wires
the rules file, (c) how to make the interval rule gate at reject. From the
agent's `loop-log.md` (§5, verified real):

1. **Rules-file JSON shape** — reverse-engineered from fail-loud
   `narrative-rules parse` errors; the friction was `bound`, a tagged struct
   `{ "const": N } | { "predicate": <id> }` (a bare number rejects).
2. **The wiring toml key `[continuity].rules_path`** — undocumented AND
   **silently ignored when wrong** (the agent's first guess `narrative_rules_path`
   was silently dropped; it had to sweep candidate names with a deliberately-
   firing rules file). A silent-fail footgun (the CLAUDE.md no-silent-fail ethos).
3. **`[continuity].interval_severity = "reject"`** — the interval class defaults
   to `warn`, `propose-verdict` has no severity flag, and a per-rule `severity`
   field rejects, so a naively-declared interval rule SURFACES but does NOT gate.
   A second silent-fail footgun (a "declared" interval rule that silently never
   bites).

**Did the gate fire naturally during authoring?** No — the agent authored
rule-consistent content first-try; the only non-deliberate signal was the
advisory dangling-setups on a bare `main` trunk (see below). So PIN-2's
orchestrator injection is the teeth evidence, stated plainly (PRED-3 confirmed) —
R600's "the classes are untested" is closed by proving the teeth load-bearing on
authored content, not by the competent agent tripping its own gate.

**Bonus — R601's fork-lineage trap RE-CONFIRMED a third time, without the doc.**
The agent (on a pre-R601 CLI, so `describe-schema` had no fork-lineage note) hit
the exact dead-prefix dangling trap R601 documented: with both roads forking off
`main`, the bare `main` trunk's `expected` setups dangled. It fixed it
structurally (made WORK = `main`, forked only HOLD) — a THIRD independent agent
hitting the same trap, evidence R601's `branches`-registry documentation is
well-targeted.

## judged-no-pin (coherence / husk backstop) — CLEARS (not a husk)

A blind render agent warm-rendered the WORK/main road (~1,440 words) from the
orchestrator-assembled manuscript beats; a blind judge read only the prose:

- **REAL GAME? yes** — a genuine single-load dilemma (break the standing order to
  save the barge vs obey and let it wreck), stakes resolved through the keeper's
  action, not narration. The `husk` branch does NOT fire.
- **WORLD-LOGIC LEGIBILITY 4/5** — the lantern custody (physically handed to the
  barge-master to free both hands, passed back for the final lift) and the
  fill-time (the counting/waiting dread, a concrete 35-minute payoff) land HARD
  and load-bearingly; the step-by-step gate is enacted correctly but reads as
  procedure rather than felt jeopardy.
- **COHERENCE 5/5** — every beat follows cleanly, no contradiction or dropped
  thread.
- **DISCLOSURE (independent leak read): clean** — the reader learns WHO is aboard
  the barge only at the sc-11 reveal (the injured master keeper); the earlier
  "master away downriver" plants without giving it away. This matches the
  authored plan (f-secret withheld, revealed at sc-11 on main) — a coherence-level
  confirmation the telling did not leak.

Craft note (not downplayed, an echo of store-consistency ≠ coherence): the
transition rule is STRUCTURALLY enforced (PIN-2 proved it bites) yet in the prose
the gate sequence reads as procedure, not tension — the one constraint the gate
GATES but the telling under-dramatizes. The judge's weakest-link: give the beam a
near-miss so all three constraints bite on the page, not just in the store.

The full re-extraction leak + fidelity gates (R502/R505) were NOT run — deferred
as disproportionate for a non-gating rule-stress backstop (the disclosure/warmth
axis was settled R514/R515); the judge's independent disclosure read is the
coherence-level check here.

## Decision (pre-committed rule)

`classes_exercised`: PIN-1 (all 3 declared + typed) AND PIN-2 (all 3 bite + the
paired removal passes) HOLD => the R600 proof gap is CLOSED. The rule classes are
REACHABLE in the unattended loop and the gate BITES on real authored content.

Secondary finding (`surface_gap`, exactly PRED-1): the rules-file AUTHORING
SURFACE is undocumented in `describe-schema` — the rules-file JSON wire, the
`[continuity].rules_path` wiring, and the `interval_severity` reject lever, two of
them silent-fail footguns. This is the next `describe-schema` build target (the
R595 wire-format / R596 canon-order pattern, now for the strongest gates). A
competent agent reverse-engineered it; a less capable one would have shipped a
store whose "declared" interval rule silently never gates.

`teeth_dull` did NOT fire (no declared rule failed its negative control).

## Honest caveats

- n = 1 premise / 1 loop agent / 1 render / 1 judge / one render road.
- PIN-2 is an orchestrator injection, not the agent's own error — it proves the
  gate BITES on the agent's real content, not that the loop naturally trips it
  (PRED-3). Closing R600's "untested" is exactly this: teeth proven load-bearing.
- The premise is engineered rule-dense; a real game need not carry all three at
  once. The experiment tests reachability + teeth, not that rules are common.
