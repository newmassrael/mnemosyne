# Loop log — "The Night Lock at Harrow Sluice"

Authoring agent, unattended. Two sources of truth only: `mnemosyne-cli
describe-schema` (the contract) and `premise.md`. Every repair below responds to
a specific gate/contract output. All commands run in this workspace
(`run/game`), operating on `docs/.atomic/workspace.atomic.json` (schema 23).

---

## 0. Design (from the contract + premise)

- **15 scenes** (sc-01..sc-15), one doc `harrow-sluice`.
- **4 frames**: `ground-truth`, `wick`, `master`, `marrow`.
- **Fork**: the WORK road is the **main spine** (sc-01→sc-12); the HOLD road is
  a single branch `hold` forking from `main` at sc-06 (sc-13→sc-15). See §2 for
  why WORK is main and not a second branch.
- **Cast/entities**: e-wick, e-master, e-marrow-master, e-lantern, e-gate,
  e-pound, e-marrow, and the quest entity e-quest (kind `quest`).
- **Withheld secret**: `f-secret` (typed `e-marrow carries e-master`) — the
  Marrow is carrying the injured master keeper home. Default telling withholds;
  one disclosure override reveals it at sc-11 on the WORK/main road only. On the
  HOLD road it is never disclosed (dramatic irony: Wick never learns whose
  passage he barred).
- **Quest** `e-quest` ("see the night through by a keeper's duty"): given at
  sc-06 (`f-quest-give`, `payoff_expectation: expected`, typed `pursues`), and
  discharged on BOTH roads — `f-quest-done-work` (main sc-12) and
  `f-quest-done-hold` (hold sc-15), each typed `completed_by` and each
  `pays_off` the giving fact.
- **Setup→payoff (Chekhov) chains**, each an `expected` setup paid in *every*
  world it reaches (see §2): the standing order (`f-order`), the single lantern
  (`f-lantern-one`), the rising pound (`f-pound-rising`), and the quest giving.

### The three world-invariants → rule classes (worked out from describe-schema)

| # | Premise invariant | Rule class | Keys on |
|---|---|---|---|
| 1 | one lantern, one holder at a time (custody/conservation) | `exclusive`, `per: object` | predicate `holds` (subject=holder, object=e-lantern) |
| 2 | gate barred↔cracked↔open, never barred→open (state machine) | `transition` | predicate `gate_state` (scalar), succession edges on e-gate |
| 3 | pound must fill ≥ 30 min before open (numeric timing) | `interval` | subject e-pound: `open_clock − fill_start_clock ≥ 30` |

Reasoning for rule 1's `per`: describe-schema says `per: object` = "one holder
per object — conservation/custody". So the item is the OBJECT and the holders
are SUBJECTS; `holds(subject=holder, object=e-lantern)` with `per: object`
means "one holder per lantern". (`per: subject` is the location-exclusivity
reading — one location value per subject — not what a single-lantern custody
needs.)

Reasoning for rule 3's operands: the interval form is fixed as
`value(left) − value(right) op bound`. Modelled as a genuine time interval on
the pound: `left = open_clock` (clock-minute the gate was brought open),
`right = fill_start_clock` (clock-minute the pound began filling), `op = ge`,
`bound = 30`. On main: 215 − 180 = 35 ≥ 30 (satisfied). On hold there is no
`open_clock` fact, so the relation simply has no left operand and is not
evaluated (no false positive).

---

## 1. import-sections

Command: `mnemosyne-cli import-sections --manifest sections.json`
Result: **15 created, 0 no-op**, exit 0. (Manifest shape confirmed against
describe-schema: array of `{section_id, parent_doc, title}`.)

---

## 2. propose-verdict #1 — facts only, no rules

Command: `mnemosyne-cli propose-verdict --manifest facts.json`
Verdict: **commit** (exit 0), violations 0.
BUT advisory: `dangling setups (advisory, non-gating) [main] (4): f-lantern-one,
f-order, f-pound-rising, f-quest-give`.

Diagnosis + change: in my first design BOTH roads were branches
(`work`, `hold`) forking off `main` at sc-06, so `main` was a bare trunk that
ended at sc-06. The four `expected` setups sit on that trunk; their payoffs live
only on the forked roads, so on the bare `main` world-line they dangle. The
`report-authoring-frontier` finish requires zero dangling setups, and facts are
append-only, so I fixed it *before* importing:

- **Restructured the fork**: made the WORK road BE `main` (a complete spine
  sc-01→sc-12) and kept only `hold` as a branch forking at sc-06. Now every
  trunk-planted setup is paid inside main's own continuation (work) AND in the
  hold fork — no bare-trunk world exists to dangle.
- Edits: dropped the `work` branch from the manifest; stripped `"branch":
  "work"` from the WORK-road facts (they become main); repointed the disclosure
  override `first_at` from `["work","sc-11"]` to `["main","sc-11"]`.

## 3. propose-verdict #2 — after the topology fix

Command: `mnemosyne-cli propose-verdict --manifest facts.json`
Verdict: **commit** (exit 0), violations 0, `dangling setups: none`. Good.

## 4. import-facts (real, atomic)

Command: `mnemosyne-cli import-facts --manifest facts.json`
Result: 4 frames + 1 branch + 8 entities + 7 predicates + 38 facts + 1
disclosure-plan + 1 disclosure-override created, exit 0.

---

## 5. Declaring + wiring the three rules — REVERSE-ENGINEERED

`describe-schema` documents the rule CLASSES and their parameters
(exclusive/`per`; transition/`allowed`; interval/`right`,`op`,`bound`) and that
`propose-verdict`/`validate-continuity` take `--rules <path>` and that canon
order is pinned via `[continuity].canon_order_path`. It does **NOT** document:
(a) the JSON wire shape of the rules file, (b) the toml key that wires the rules
file into the persistent gate, or (c) how to make an interval violation gate at
reject severity. All three had to be discovered from command errors/behaviour.

### 5a. Rules-file JSON shape — from `narrative-rules parse` errors

First guess `{ "rules": [ {id, class, predicate, per|allowed|right/op/bound} ] }`.
Probe: `mnemosyne-cli validate-continuity --rules narrative-rules.json --order canon-order.json`.

- The outer shape + `id/class/predicate/per/allowed/right/op` all parsed on the
  first try (no error about them).
- Friction 1: `"bound": 30` → `error: ... invalid type: integer 30, expected
  struct IntervalBoundWire`. So `bound` is a tagged struct, not a bare number.
- Friction 2: `"bound": { "literal": 30 }` → `error: ... unknown field
  \`literal\`, expected \`predicate\` or \`const\``. The error NAMED the fields.
- Resolved: `"bound": { "const": 30 }` (a literal; `{ "predicate": <id> }` would
  be the third-scalar-predicate form). Rules file then parsed clean.

Final `narrative-rules.json` (the three rules):
```
{ "rules": [
  { "id":"lantern-custody", "class":"exclusive", "predicate":"holds", "per":"object" },
  { "id":"gate-sequence",   "class":"transition","predicate":"gate_state",
    "allowed":[["barred","cracked"],["cracked","open"],["open","cracked"],["cracked","barred"]] },
  { "id":"fill-hold", "class":"interval", "predicate":"open_clock",
    "right":"fill_start_clock", "op":"ge", "bound":{ "const":30 } }
] }
```
Note: the rules parser is fail-loud on unknown fields — the full valid field set
for a rule is exactly `id, predicate, class, per, allowed, right, op, bound`
(discovered in §5c). `predicate` is the "left"/keyed predicate for every class.

### 5b. Wiring the rules into the persistent gate — toml key REVERSE-ENGINEERED

The finish state wants the rules "wired (so the gate loads them)". Discovered
that unknown toml `[continuity]` keys are **silently ignored** (no fail-loud), so
I probed with a rules file that deliberately FIRES on the valid store
(`probe-rules-fire.json`: a transition rule with `allowed` missing the steps that
occur → 3 structural violations) and swept candidate key names, running
`validate-continuity` (no flags) and watching the violation count:

- `narrative_rules_path` → 0 violations (ignored — my first guess was WRONG).
- `rules_path` → **3 structural violations** (LOADED). ← correct key.
- `narrative_rules`, `rule_path`, `rules`, `narrative_rule_path`,
  `continuity_rules_path` → all 0 (ignored).

So the correct wiring is `[continuity].rules_path`. (`canon_order_path` was
already documented and confirmed reading — `order_nodes=15`.)

### 5c. Making the interval rule GATE at reject — toml key REVERSE-ENGINEERED

With `--rules`, the exclusive and transition breaks REJECTED by default, but the
interval break only SURFACED (`violations: 1 (0 gating at reject severity)`,
exit 0 = commit). Interval defaults to `warn`. `propose-verdict` has no
`--interval-severity` flag (only `validate-continuity` does), and a per-rule
`"severity"` field is rejected by the fail-loud parser (`unknown field
\`severity\`, expected one of id, predicate, class, per, allowed, right, op,
bound`). So the interval severity had to come from toml.

- Discovered that `propose-verdict` DOES read `[continuity]` severities:
  setting `severity = "warn"` made a transition (structural) break drop to
  "0 gating" and commit — proving toml severity is honoured by the dry-run gate.
- Swept the interval key: `[continuity].interval_severity = "reject"` makes the
  interval break gate. (This mirrors `validate-continuity`'s `--interval-severity`
  / `--severity` split: `severity` = structural, `interval_severity` = interval.)

### Final `mnemosyne.toml [continuity]` (as wired, in place)
```
[continuity]
canon_order_path = "canon-order.json"
rules_path = "narrative-rules.json"
interval_severity = "reject"
```
Confirmation the gate loads all three: `validate-continuity` (no flags) on the
clean store reports `rules=3 ... interval_severity=reject ... violations: 0`,
exit 0. And `propose-verdict` with NO flags auto-loads rules_path +
canon_order_path + severities from toml (verified by the bite tests in §6).

### Canon-order artifact (`canon-order.json`, pinned via canon_order_path)
Main trunk sc-01→…→sc-12 in `edges`; the hold fork under `branches.hold`
(sc-06→sc-13→sc-14→sc-15). Confirmed loaded: `order_nodes=15`, `unordered=0`.

---

## 6. Rule-BITES checks (each break rejected by propose-verdict, then discarded)

Each break is a tiny incremental manifest applied on top of the real store by
`propose-verdict` (NO flags — proving the toml wiring is live). All three
REJECTED (rollback, exit 1). Breaking drafts were discarded afterward.

### BITE 1 — lantern custody (`exclusive`, `per: object`)
Breaking draft `break-lantern.json` (added fact): the barge-master ALSO holds the
one lantern while Wick still holds it —
`{ f-break-lantern, ground-truth, holds(e-marrow-master → e-lantern), from sc-05 }`
(no succession, so it overlaps Wick's custody).
Command: `mnemosyne-cli propose-verdict --manifest break-lantern.json`
Result: **rollback**, exit 1, **4× `rule_exclusive_overlap`**, e.g.
`exclusive rule \`lantern-custody\`: facts \`f-break-lantern\` and
\`f-lantern-wick\` co-hold conflicting \`holds\` at \`sc-05\` (frame
\`ground-truth\`, branch \`main\`)`. → Rule 1 BITES. Draft discarded.

### BITE 2 — gate sequence (`transition`)
Breaking draft `break-gate.json` (added fact): on the hold road the gate leaps
straight from barred to open —
`{ f-break-gate, ground-truth, branch hold, gate_state=open, supersedes f-gate-barred, from sc-13 }`.
Command: `mnemosyne-cli propose-verdict --manifest break-gate.json`
Result: **rollback**, exit 1, **`rule_transition_invalid`**:
`transition rule \`gate-sequence\`: subject \`e-gate\` steps \`barred\` ->
\`open\` (\`f-gate-barred\` -> \`f-break-gate\`) outside the allowed set`.
→ Rule 2 BITES. Draft discarded.

### BITE 3 — fill hold (`interval`)
Breaking draft `break-fill.json` (added fact): the gate is brought open after
only 20 minutes of fill —
`{ f-break-open, ground-truth, branch hold, open_clock="195", from sc-14 }`
(195 − 180 = 15 < 30).
Command: `mnemosyne-cli propose-verdict --manifest break-fill.json`
Result: **rollback**, exit 1, **`rule_interval_violation`**:
`interval rule \`fill-hold\`: subject \`e-pound\` 195 - 180 not >= 30 at
\`sc-14\` (frame \`ground-truth\`, branch \`hold\`)`, `1 gating at reject
severity`. → Rule 3 BITES (only after `interval_severity = "reject"` was wired
in §5c; before that it surfaced but did not gate). Draft discarded.

Did the gate ever fire on ME during authoring (not a deliberate break)? Only the
advisory dangling-setups signal in §2 (fixed structurally before import). No
rule ever rejected a legitimate authored fact.

---

## 7. report-authoring-frontier — closing every gap

Command: `mnemosyne-cli report-authoring-frontier --telling default`
Result (final): `37 gap(s)`, of which:
- `zero-fact scenes: none`
- `unordered scenes: none`
- `dangling setups: none`
- `unresolved quests: none`
- `never-planned disclosures (37)`: every fact except `f-secret`.

The only remaining gaps are `never-planned disclosures` — the intentional
withhold. The telling `default` has `default_mode: withhold` (the substrate's
canonical Dark-Souls reconstruct posture) with exactly one explicit override,
`f-secret` (revealed at sc-11 on the main/WORK road, never on HOLD). Per the
finish state, never-planned disclosures MAY remain; all other gap axes are
closed. FINISH STATE MET.

---

## 8. Final verification snapshot

- `propose-verdict --manifest facts.json` → **commit**, 60 no-op, violations 0.
- `validate-continuity` (no flags) → `facts=38 order_nodes=15 rules=3
  interval_severity=reject`, `violations: 0`, exit 0.
- Three rules declared + wired (`[continuity].rules_path`) + live (`rules=3`) +
  each BITES (§6).
- `report-authoring-frontier --telling default` → only never-planned
  disclosures remain.

## 9. Frictions / guesses / missing-contract info (summary)

1. **Fork topology vs dangling setups** (§2): the contract doesn't say a bare
   `main` trunk counts as its own world for payoff coverage. propose-verdict's
   advisory revealed it; fixed by making WORK = main. (Not a rule; a coverage
   axis.)
2. **Rules-file JSON shape** (§5a): undocumented. Reverse-engineered from
   fail-loud `narrative-rules parse` errors — key finding: `bound` is
   `{ "const": N }` | `{ "predicate": <id> }`, and `predicate` is the keyed
   ("left") predicate for every class.
3. **Rules toml key** (§5b): undocumented AND silently ignored when wrong. Found
   `[continuity].rules_path` by probing with a deliberately-firing rules file and
   sweeping candidate key names.
4. **Interval gating** (§5c): interval defaults to `warn`; `propose-verdict` has
   no severity flag and the rules file has no per-rule `severity`. Found that
   `propose-verdict` honours toml `[continuity]` severities and that
   `interval_severity = "reject"` is the interval lever.
5. **propose-verdict input model**: it is a pure function of `--manifest` +
   whatever it auto-loads from toml `[continuity]` (rules_path, canon_order_path,
   severities). Passing `--rules/--order` explicitly overrides/supplies the same;
   both paths were used and agree.
