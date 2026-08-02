# side-table-authoring-experiment / v1 — report

**The question.** `edge_costs` and `edge_guards` were verb-only until Round 956
built the manifest wire, and Round 957 removed the contract sentence that told
authors a manifest could not carry them. Does an author reach these tables now?

**Baseline (Round 959, re-measured across all 97 stores on disk).**

| table | authored corpora before this run |
|---|---|
| `edge_costs` | 0 |
| `edge_guards` | 1 — `phase1-2d-projection-experiment/v1` (R750) |

**Result.** Both arms reached both tables **through the fact manifest, on the
first submission, with no iteration and no hand-written script.** The count moves
to `edge_costs` **2** and `edge_guards` **3**.

n=2, one world premise, one lineage orchestrating. One instance, not a
distribution.

---

## E1 — discoverability (Stage A was told nothing)

Stage A's brief named no machinery. The forbidden words — *cost*, *guard*,
*edge*, *side table*, *threshold*, *condition*, *K-of-N*, *manifest array*,
*verb*, `edge_costs`, `edge_guards` — appear nowhere in it. The fiction made both
properties necessary (a long stair against a few steps; a sea-gate at low water
and a door that needs a key) and said only that "the engine must be able to
answer both of those from your data alone, without reading your prose."

| sub-answer | Stage A | Stage B |
|---|---|---|
| `edge_costs` **in the fact manifest** | YES — `run/stage-a/facts.json`, 18 rows | YES — `run/stage-b/facts.json`, 20 rows |
| `edge_guards` in the fact manifest | YES — `facts.json:500-504`, 3 rows | YES — `facts.json:636-641`, 3 rows |
| `conditions` name real condition facts | YES — `f-tide-low`, `f-maren-key` | YES — `f-tide-out`, `f-key-verrick` |
| `threshold` used | NO — 0 occurrences | NO — 0 occurrences |
| hand-written shell script / verb calls | NONE | NONE |

**The discriminating fact is the FILE.** Reaching these tables by calling
`add-edge-cost` by hand is the Round 943 outcome and means the wire did not carry
the author. Neither directory contains a `.sh`, and neither self-report mentions
running a verb. The import receipts are where the tables arrive:

- Stage A: `18 edge-costs + 3 edge-guard-conditions created`
- Stage B: `20 edge-costs + 3 edge-guard-conditions created`

### Three things Stage A's author did that were not asked for

1. **They chose a directed map BECAUSE of the cost table.** Their self-report:
   "I declared the transition rule in `rules.json` with `"undirected": false`, so
   one fact means one direction and a two-way way is two facts. I did that
   deliberately, because the stair costs more going up than coming down and an
   undirected edge cannot say so." The undirected branch was not overlooked here;
   it was rejected for a stated reason.
2. **`threshold` was declined, not missed.** "Each guard is a set with no
   threshold, which means every condition in it is required. I used one condition
   per guard because each shut way here has exactly one thing that opens it." The
   K-of-N branch still has no authored witness, and this run says why: the world
   premise gave every shut way exactly one opener.
3. **They stated the layering line unprompted.** "I never sum these anywhere, and
   no file of mine claims a total journey time. The numbers are carried, not
   added." That is the carriage-not-computation boundary, arrived at from the
   contract alone.

Stage B's author reached the same place independently and added a world-line:
they registered a branch `shut-gate` in which `f-tide-high` supersedes
`f-tide-out`, "so the guard's condition no longer holds there, the causeway edges
are shut … The engine never evaluates the guard; it holds the declaration and the
two roads."

---

## E2 — first submissions, verbatim

Both submissions were frozen by `cp -r` **before** any output was handed back.
Neither author was told the copy exists.

### `run/stage-a/first-import.log`

```
=== mnemosyne-cli import_sections ===
primitive: import_sections
target_kind: section
target_id: 21 created, 0 no-op
sidecar_path: .../v1/run/stage-a/docs/.atomic/workspace.atomic.json
written_bytes: 3073
exit=0
=== mnemosyne-cli import_facts ===
primitive: import_facts
target_kind: narrative_fact
target_id: 2 frames + 0 branches + 3 entity-kinds + 1 units + 18 entities + 5 predicates + 80 facts + 18 edge-costs + 3 edge-guard-conditions + 0 disclosure-plans + 0 disclosure-overrides created, 0 no-op
sidecar_path: .../v1/run/stage-a/docs/.atomic/workspace.atomic.json
written_bytes: 46665
exit=0
```

### `run/stage-b/first-import.log`

```
=== mnemosyne-cli import_sections ===
primitive: import_sections
target_kind: section
target_id: 21 created, 0 no-op
sidecar_path: .../v1/run/stage-b/docs/.atomic/workspace.atomic.json
written_bytes: 3176
exit=0
=== mnemosyne-cli import_facts ===
primitive: import_facts
target_kind: narrative_fact
target_id: 2 frames + 1 branches + 4 entity-kinds + 1 units + 22 entities + 5 predicates + 84 facts + 20 edge-costs + 3 edge-guard-conditions + 0 disclosure-plans + 0 disclosure-overrides created, 0 no-op
sidecar_path: .../v1/run/stage-b/docs/.atomic/workspace.atomic.json
written_bytes: 48313
exit=0
```

**Both green on the first submission**, so the iterate-to-green loop never ran
and no error was ever handed back. Each self-report is byte-identical to its
frozen copy (`diff` clean, both stages) — there was no post-freeze revision to
record.

**The rules pin needed no orchestrator patch, 2 of 2.** Both authors wrote
`rules.json` and added `rules_path` to `mnemosyne.toml` themselves; `grep -n
rules_path` resolves in both and the orchestrator rewrote nothing. This is the
third arm in a row (with Round 943's two) in which the runbook's optional patch
step was unnecessary.

---

## E3 — frozen self-report against the map read

Checked on **condition fact ids**, not place names.

### Stage A — `report-transition-map`, in full

```
=== declared maps — 1 transition rule(s) ===
map `movement-follows-the-town-map` (adjacency `adjacent`, directed): 13 node(s), 18 edge(s)
  p-almshouse -> p-bell-tower 2 minute (f-adj-alms-tower)
  p-bell-tower -> p-almshouse 2 minute (f-adj-tower-alms)
  p-counting-house -> p-north-row 1 minute (f-adj-ch-nrow)
  p-fish-shed -> p-quay 3 minute (f-adj-shed-quay)
  p-lower-terrace -> p-market-terrace 7 minute (f-adj-lt-mt)
  p-lower-terrace -> p-sea-gate 3 minute (f-adj-lt-sg)
  p-market-terrace -> p-lower-terrace 4 minute (f-adj-mt-lt)
  p-market-terrace -> p-upper-terrace 15 minute (f-adj-mt-ut)
  p-net-loft -> p-quay 2 minute (f-adj-loft-quay)
  p-north-row -> p-counting-house 1 minute (f-adj-nrow-ch) [guard all: f-maren-key]
  p-north-row -> p-south-row 1 minute (f-adj-nrow-srow)
  p-quay -> p-fish-shed 3 minute (f-adj-quay-shed)
  p-quay -> p-net-loft 2 minute (f-adj-quay-loft)
  p-sea-gate -> p-lower-terrace 3 minute (f-adj-sg-lt)
  p-sea-gate -> p-strand 4 minute (f-adj-sg-st) [guard all: f-tide-low]
  p-south-row -> p-north-row 1 minute (f-adj-srow-nrow)
  p-strand -> p-sea-gate 5 minute (f-adj-st-sg) [guard all: f-tide-low]
  p-upper-terrace -> p-market-terrace 6 minute (f-adj-ut-mt)
```

Every cost claim in the frozen self-report matches, and the ordering matches: the
Long Stair up (15) is the most expensive move, the market lane (1) the cheapest.
The asymmetries the author described are all present — 15/6 on the Long Stair,
7/4 on the Cliff Stair, 4/5 through the arch. All three guards print, on exactly
the edges named, with exactly the condition facts named. The author's deliberate
asymmetry — `f-adj-ch-nrow` (out of the counting house) left unguarded because
"the door locks against the street, not against the room" — is visible as the
absence of a guard on that one edge.

**Disagreements: none.**

### Stage B — `report-transition-map`, in full

```
=== declared maps — 1 transition rule(s) ===
map `coldharrow-map` (adjacency `adjacent`, directed): 13 node(s), 20 edge(s)
  d-harbour -> d-market 9 minute (fe-ramp-up)
  d-harbour -> p-mussel-rock 12 minute (fe-gate-out) [guard all: f-tide-out]
  d-market -> d-harbour 5 minute (fe-ramp-down)
  d-market -> d-upper 14 minute (fe-stair-up)
  d-upper -> d-market 6 minute (fe-stair-down)
  p-beacon-yard -> p-cistern-court 4 minute (fe-yard-cistern)
  p-chapel -> p-cistern-court 3 minute (fe-chapel-cistern)
  p-cistern-court -> p-beacon-yard 4 minute (fe-cistern-yard)
  p-cistern-court -> p-chapel 3 minute (fe-cistern-chapel)
  p-cloth-row -> p-fish-row 1 minute (fe-cloth-fish)
  p-cloth-row -> p-weigh-house 2 minute (fe-cloth-weigh)
  p-counting-house -> p-quay 2 minute (fe-counting-quay)
  p-fish-row -> p-cloth-row 1 minute (fe-fish-cloth)
  p-fish-row -> p-weigh-house 2 minute (fe-fish-weigh)
  p-mussel-rock -> d-harbour 12 minute (fe-gate-back) [guard all: f-tide-out]
  p-net-loft -> p-quay 2 minute (fe-loft-quay)
  p-quay -> p-counting-house 2 minute (fe-quay-counting) [guard all: f-key-verrick]
  p-quay -> p-net-loft 2 minute (fe-quay-loft)
  p-weigh-house -> p-cloth-row 2 minute (fe-weigh-cloth)
  p-weigh-house -> p-fish-row 2 minute (fe-weigh-fish)
```

Every cost and guard claim matches, including the 14/6 Long Stair asymmetry, the
9/5 ramp, the 12-each-way causeway, and the 1-minute market lane. Guards print on
`fe-gate-out`, `fe-gate-back` and `fe-quay-counting` with the named conditions;
`fe-counting-quay` is unguarded, as the author said.

**Cost/guard disagreements: none.**

**One prose slip, recorded because it is the author's prose and not the store's:**
the cost section says "all three harbour ways 2 each" while the layout section
lists two harbour ways (Quay↔Net Loft, Quay↔Counting House). The data is
consistent — ten two-way ways, twenty edges, all four harbour edges cost 2 — so
this is a miscount inside the self-report, not a disagreement between the report
and the map.

---

## Other reads, in full

### `report-authoring-frontier`

Stage A — **0 gap(s)**:

```
zero-fact scenes: none / unplaced scenes: none / unordered scenes: none
dangling setups: none
unconnected places [movement-follows-the-town-map]: none
branch density [main]: 80 owned fact(s) over 21 traversed scene(s) = 3.81
```

Stage B — **1 gap(s)**:

```
dangling setups [shut-gate] (1): f-ev-17
unconnected places [coldharrow-map]: none
branch density [main]: 81 owned fact(s) over 20 traversed scene(s) = 4.05
branch density [shut-gate]: 3 owned fact(s) over 20 traversed scene(s) = 0.15
```

The one gap is the author's stated design: the lamp setup `f-ev-17` is paid off
on `main` and deliberately left unpaid on the `shut-gate` world-line, where the
tide is high and the causeway is shut.

`unconnected places` was EVALUATED and answered `none` on both maps — not "not
derivable" — so the map read is doing work on both corpora.

### `validate-continuity` — read whole, not just `violations:`

Stage A — **violations: 0 (structural=0 interval=0)**, 25 steps judged
(hierarchy crossing 1 / lifted 24 / unmoved 0, 0 unlicensed), rules=3.

Stage B — **violations: 2 (structural=2 interval=0)**, 44 steps judged
(hierarchy crossing 13 / lifted 31 / unmoved 0, 0 unlicensed), rules=3:

```
{"kind":"rule_exclusive_overlap","rule":"one-holder-per-thing","predicate":"held_by","frame":"ground-truth","branch":"main","fact_a":"f-ev-ledger","fact_b":"f-key-verrick","at":"sc-04"}
{"kind":"rule_exclusive_overlap","rule":"one-holder-per-thing","predicate":"held_by","frame":"ground-truth","branch":"shut-gate","fact_a":"f-ev-ledger","fact_b":"f-key-verrick","at":"sc-04"}
```

Recorded, not resolved. The mechanical facts, each read out of the author's own
files:

- `rules.json` declares `{"id":"one-holder-per-thing","predicate":"held_by","class":"exclusive","per":"object"}`.
- `f-key-verrick` is typed `{subject: e-key, predicate: held_by, object: e-verrick}`.
- `f-ev-ledger` is typed `{subject: e-ledger, predicate: held_by, object: e-verrick}`.
- Both facts therefore share the OBJECT `e-verrick`, which is the key the rule is
  scoped by.
- The frozen self-report describes the same rule as "an exclusive rule on
  `held_by` keyed per object, which is what makes the letter's passage from Mella
  to Old Tomsen a succession rather than two holders at once."

The declaration and the description are not the same rule, and this run does not
decide which one the author meant. It is recorded here as a finding for a later
round: with `held_by(thing, person)`, `per: object` scopes exclusivity to the
PERSON, so "one holder per thing" is `per: subject`. Whether the contract leads
an author to read `per:` against the typed leg's direction is a question this arm
did not ask.

**Known interaction, not a defect:** a guard is checked to sit on a map EDGE, so
a guard placed on a non-edge fact surfaces as `edge_guard_not_an_edge` at the
gate rather than as a silent absence in the map read. Neither corpus tripped it —
all six guards sit on `adjacent` facts.

`interval_unverifiable=not-declared interval_severity=off` on both stores: that
class was NOT evaluated, because neither author declared an interval rule. A
green `violations:` line here does not speak for it.

---

## What this run did not measure

- **`threshold` / K-of-N still has no authored witness.** Two authors, both
  declining it for the same stated reason: each shut way in their world had
  exactly one opener. A premise that needs two-of-three would be a different arm.
- **The `undirected` branch still has no authored witness**, and now has a
  measured reason: both authors chose `directed` in order to give a way different
  costs in each direction. Round 936 recorded five corpora choosing directed
  without knowing why; this run has one of them saying so in his own words.
- `parameters` / `parameter_deltas` / `parameter_gates` / `fact_counts` — out of
  scope by the runbook, and answered separately by Round 959's measurement.
- The disclosure axis, the render-acceptance gates, and the craft of the prose.
  There is no prose here to judge.
