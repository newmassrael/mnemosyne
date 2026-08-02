# k-of-n-authoring-experiment/v1 — report

Protocol: `../runbook.md`, frozen at Round 967 before this arm ran. Three blind
authors, fresh context each, isolated directories, blind to each other. Each was
given the firewall wrapper plus the verbatim brief from the runbook and nothing
else — never the runbook, the changelog, a design doc, or another author's work.
The orchestrator ran every command, authored no manifest, and repaired no file.

## The oracle, and where it moved

Baseline censused at Round 966, field-keyed over every JSON under `claudedocs/`
(99 stores, 65 fact manifests, 43 rules files). Re-run after this arm:

| axis | before | after | corpora |
|---|---|---|---|
| `edge_guards[].threshold`, non-null k | **0** | **3** | Stage A `k=2`, Stage B `k=2`, Stage C `k=1` |
| `first_at[].threshold`, non-null k | **0** | **1** | Stage C `k=2` |
| `undirected: true` transition rule | **0** | **3** | all three |

Round 961 recorded five corpora choosing directed and both K-of-N branches with
no authored witness. All three are now witnessed, and two of the three moves were
never asked for.

## E1 — Stage A, the arm told nothing: YES

The Stage A brief names no machinery — not threshold, K-of-N, condition, guard,
edge, cost, side table, manifest array, verb, disclosure, telling, reveal,
withhold, `edge_guards`, `first_at` or `surface`. It gives a fiction in which
two-of-three is necessary (a council lock whose custom opens for any two of three
warden seals, the third warden often away) and one sentence of requirement: the
planner must be able to tell what opens each shut way "from your data alone,
without reading your prose."

The author wrote, in `facts.json`:

```json
{ "fact_id": "f-way-counting-strongroom",
  "conditions": ["f-seal-ready-hesper", "f-seal-ready-orrin", "f-seal-ready-vale"],
  "threshold": 2 }
```

and the witness read returns it:

```
e-counting-hall -> e-strongroom 1 minute (f-way-counting-strongroom) [guard 2-of-3: f-seal-ready-hesper, f-seal-ready-orrin, f-seal-ready-vale]
e-counting-hall -> e-tally-office 1 minute (f-way-counting-tally) [guard all: f-key-ready-nissa]
e-river-wharf   -> e-water-stair  6 minute (f-way-wharf-stair)     [guard all: f-cond-low-water]
```

**The two shapes are distinguished, not conflated.** The simpler shut ways carry
one condition and no threshold; the self-report says why, quoting the rule: *"The
contract's K-of-N rule is that an omitted threshold means AND over the whole set,
and `k == len` normalizes back to AND, so `2` of `3` is a real at-least-two and
is legal."*

**The discriminating fact is the FILE.** Reaching the table by hand-calling
`set-edge-guard-threshold` would be the Round 943 outcome and would mean the wire
did not carry the author. No stage directory holds a shell script (0 of 3), no
self-report mentions running a verb, and all three first submissions imported
GREEN — the iterate-to-green loop never ran on any arm.

## E1 — Stage B, asked in fiction: YES, same encoding

`threshold: 2` over the same three-seal set, read back as `[guard 2-of-3: …]`.
The self-report states the non-degeneracy explicitly — *"a `threshold` of 2
against a set of 3 is a genuine, non-degenerate two-of-three"* — and encodes the
custom a second time as quantity facts plus an interval rule, calling the
redundancy deliberate: *"the same custom said in the quantity slot, where an
arithmetic check can reach it."*

## E1-C — Stage C, the disclosure axis: YES

The brief describes a house over one winter, a fact true from the first scene,
three scenes that brush against it, and asks that the reader come to hold it "at
the SECOND one they reach". It names no machinery. The author wrote:

```json
{ "fact_id": "f-truth", "mode": "imply",
  "surface": { "scene": "sc-08", "object": "e-ledger" },
  "first_at": [ { "branch": "main",
                  "coords": ["sc-03", "sc-08", "sc-12"],
                  "threshold": 2 } ] }
```

and `report-playthrough-manuscript --telling winter-house` returns:

```
+ f-truth (ground-truth) [imply first_at={sc-03,sc-08,sc-12} k=2 via sc-08/e-ledger]: Tobin was fathered by Rhodes Callender.
```

**`k=1` over a three-coord set would have been the discriminating negative** —
the trigger set reached and the threshold not — and it is not what happened.

### ★ The Round 966 repair was measured from outside, one round after it landed

Stage C's `contract.txt` is the repaired contract (it carries the `INERT reveal
pin` sentence). The author chose a disclosing mode and gave this reason, in the
frozen self-report:

> `mode: "imply"` — not `withhold`. The reader is meant to arrive at this, and **a
> withheld fact plus a reveal pin discloses nothing on any road**; `imply` is the
> mode for a thing realised through an object, which is what all three brushes are.

That is the Round 966 sentence, read and applied by an author who never saw Round
966 — the Round 943 pattern (a repair returned by someone who never saw it), on
the axis where the pre-repair paragraph would have produced a stored `k` that no
human surface can show. `report-disclosure-coverage` reports **0 inert reveal
pins** on this corpus.

The author also stated the semantics correctly and unprompted: *"the threshold of
2 says the reveal is the second-earliest coord reached, whichever two those turn
out to be. One brush is below the threshold, so a reader who has passed only one
has been told nothing and may fairly think they imagined it. The third is above
it and adds no reveal."*

### ★ Stage C reached the OTHER axis without being asked

The Stage C brief mentions no map, no places, and no shut ways. The author built
the house as a real map anyway, put costs on all five edges, and wrote:

```
pl-lane -> pl-house 20 minute (f-adj-lane-house) [guard 1-of-2: f-lane-open-first, f-lane-reopened]
```

naming it in the self-report as *"the substrate's other K-of-N"*. So one author
reached BOTH thresholds in one corpus and used each with the correct default —
`k=1` of 2 as an OR on the access axis, `k=2` of 3 as second-earliest on the
disclosure axis. The runbook's carry said no stage would ask whether an author
transfers one default wrongly to the other; this one answered it anyway, and did
not transfer it wrongly.

## E2 — first submissions

All three frozen by `cp -r` BEFORE any output was handed back, and none of the
authors was told the copy exists (a writer who knows a document is about to be
frozen writes a different document). `diff -r` of frozen against working: **no
difference on any of the three** — no post-freeze revision to record, unlike
Round 943.

Both exit codes are `0` on all three `first-import.log` files, kept verbatim:

| stage | sections | facts |
|---|---|---|
| A | 20 created | 101 facts, 14 edge-costs, 5 edge-guard-conditions, 1 plan, 2 overrides |
| B | 24 created | 107 facts, 15 edge-costs, 5 edge-guard-conditions, 1 plan, 2 overrides |
| C | 15 created | 63 facts, 5 edge-costs, 2 edge-guard-conditions, 1 plan, 8 overrides |

## E3 — the frozen self-report against the reads

**No disagreement on any arm.** Checked on condition fact ids and coord ids, not
on place or scene names:

- every shut way each self-report describes prints a guard on that edge whose
  conditions are the fact ids the report names;
- the strongroom prints `2-of-3` on both access arms, over exactly the three seal
  facts each report names;
- the single-condition ways print `[guard all: …]` over the one fact each report
  names — no arm set a redundant threshold on a one-condition set;
- Stage C's reveal prints `k=2` over exactly the three coords its report names,
  and its seat is the scene and object the report names.

Stage C's report claims `sc-08` is "the second of the three on the road this
telling walks". The declaration is what the store holds and what the read prints;
the resolution to `sc-08` follows from the single-road order in `order.json`
(`sc-03` < `sc-08` < `sc-12`), and no read prints the resolved coordinate, so
this is recorded as consistent rather than as an independent confirmation.

**Nothing was resolved in the orchestrator's voice.** No disagreements arose to
resolve.

## Gates

| stage | violations | steps judged | rules | interval class |
|---|---|---|---|---|
| A | 0 (structural 0, interval 0) | 33 | 4 | `not-declared`, severity `off` |
| B | 0 (structural 0, interval 0) | 39 | 4 | `interval_unverifiable=0`, severity **`reject`** |
| C | 0 (structural 0, interval 0) | 16 | 3 | `not-declared`, severity `off` |

`report-authoring-frontier`: 0 gaps on all three; `unconnected places` evaluated
and answered `none` on each, so the read is doing work rather than declining.

★ **Stage B turned the interval class ON itself** — it added
`interval_severity = "reject"` to its own `mnemosyne.toml`, and the class then
evaluated (`interval_unverifiable=0`). On A and C the class was never evaluated,
so their green does not speak for it (the Round 934 shape). This is the first
corpus on record whose green does.

All three authors wired `rules_path` into `mnemosyne.toml` themselves — 3 of 3,
so the runbook's conditional patch step stayed unused, as it did in Round 943 and
Round 961 (now 7 of 7 across three arms).

## What was NOT found

**No author reached the alternative encoding, and none weighed it.** Two-of-three
is sayable twice in this contract — one guard with `threshold: 2`, or three
guarded edges to the same target each carrying a different pair (the contract's
own "OR is authored as MULTIPLE guarded edges to the same target"). All three
chose the threshold, and **no self-report mentions the alternative at all**. That
is what was measured; whether any author considered and discarded it is not
observable from these artifacts. The runbook was built to record either answer as
a finding, and this is the answer it got.

## Limits

- **n=1 per axis premise**, one lineage orchestrating. Stage A and Stage B share
  one world premise, so their agreement is weaker evidence than two premises
  would be. One instance, not a distribution.
- The `undirected` count moved to 3 as a **side effect** — no brief asked for
  symmetry.

  ★ **CORRECTION, made before this record was pushed.** This section first said
  "none of the three self-reports gives a reason for choosing it". That was
  wrong, and it was wrong because the question was never asked: the sweep that
  produced it searched the self-reports for the *encoding* alternatives and not
  for `undirected` at all, so the silence it reported was the silence of a
  question nobody put. Re-measured, per stage:

  - **Stage A** gives a reason, and it is authoring economy: *"One fact is the
    whole way, in both directions … so a way does not have to be written
    twice."*
  - **Stage B** gives the same reason in the same currency: *"one fact is one
    two-way way; fifteen facts are fifteen ways, not thirty."*
  - **Stage C** gives none — `undirected` appears only in its inventory of the
    three rules it wired.

  So two of three chose symmetry to halve the fact count, which is not what the
  field declares. Round 924's contract text says `undirected` "declares EDGE
  SYMMETRY and nothing else", and Round 961's author declined it for the
  capability the other setting buys: *"the stair costs more going up than coming
  down and an undirected edge cannot say so."* Both of these worlds contain a
  water-stair carrying a single cost for both directions, and **no self-report on
  any arm mentions a way that should cost differently by direction** (0 of 3).

  Whether that is a loss these authors would have minded is NOT measured here,
  and neither is whether the contract's own sentence invites the economy reading
  — it introduces the directed setting with "a two-way road is two facts", which
  states the cost of directed before the capability it buys. Both are the next
  round's question, and the Round 968 ledger entry carries the uncorrected claim,
  since that ledger is append-only and supersession there is stated in prose by a
  later entry.
- Stage C's map and access-axis threshold are outside its brief entirely; they
  are recorded because they are on record, not because this arm set out to
  measure them.
- No prose was rendered or re-extracted, so the render-acceptance gates did not
  run on any corpus and nothing here speaks to premature leak in prose.
