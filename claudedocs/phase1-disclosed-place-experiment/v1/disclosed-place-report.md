# Report — disclosed-place-experiment/v1

Two blind authors, one flooded hill-town each, run against the protocol frozen
in `runbook.md` before execution. Round 938 built the disclosed-place join and
closed admitting it had no authored witness; Round 942 measured that the witness
count was zero because no store in this tree has ever carried both axes, and
that `mnemosyne-render` — the shipping consumer of the join — had never been run
on authored data and could not be. This is the run that ends that.

**Headline: the renderer ran on authored data, twice, exit 0. The placement half
of the join honours the telling. The map half does not look at it at all, and
two independent blind authors each walked into that, one of them asserting the
opposite in a sealed report.**

The oracle is the sealed self-report, not our own verb output. Where the two
disagree, this file records the disagreement; it does not resolve it in the
orchestrator's voice.

---

## What was run

| | Stage A | Stage B |
|---|---|---|
| brief | world only — **neither capability named** | world + one fiction sentence asking the engine to answer both questions |
| scenes | 20 | 21 |
| locations | 13 | 13 |
| ways | 10 (`undirected`) | 13 (`undirected`) |
| facts | 67 | 73 |
| tellings | `withheld-place`, `plain-place` | `told`, `told-open` |
| first submission | **imported clean, exit 0 both verbs** | `import-sections` clean; `import-facts` **failed** |
| gate | `violations: 0` | `violations: 0` (after the author's fix) |

Both authors were spawned as separate subagents with fresh context, firewalled to
their own directory, forbidden every other file in the repository. Neither ran
any command. The orchestrating lineage knew the hypothesis, ran every command,
wrote no manifest in its own voice, and repaired no author file — the failing
submission was handed back as the verbatim tool error (Round 469 contamination
bound).

---

## E1 — is the second axis reachable from the contract alone?

Read off the authored files, never asked for. **Stage A is the measurement**;
Stage B was told to answer both questions, so its YES answers are decoration.

| sub-answer | Stage A (nothing named) | Stage B (both asked) |
|---|---|---|
| a `transition` rule declaring an **`adjacency`** predicate | **YES** — `rules.json`, rule `town-map`, `adjacency: "adjacent"`, `undirected: true`, `containment: "contains"` | YES — `rules.json`, rule `walk-the-town` |
| a `disclosure_plans` entry | **YES** — `facts.json`, plans `withheld-place` (default `withhold`) and `plain-place` (default `state`) | YES — `told` / `told-open` |
| per-fact overrides vs the plan default alone | **YES** — 67 + 5 overrides | YES — 53 + 1 overrides |
| typed legs on the location facts | **YES** — `facts.json` predicates: `at` (`subject_kind: character`, `object_entity_kind: place`), `adjacent` (`place`/`place`) | YES — same shape on `adjacent` |

**E1 = YES on all four, in the arm that was told nothing.** The author reached
the transition class, the disclosure layer, per-fact overrides, and typed legs
from the contract alone, and said why in the self-report: *"I also gave the
`adjacent` predicate both leg kinds … because the contract is explicit that
without them the store cannot be asked which entities are places and the
completeness check quietly evaluates nothing."* That is Round 934's repair being
read back by an author who never saw Round 934.

The consequence for the frontier is visible: `unconnected places [town-map]:
none` on both stores — the completeness class **evaluated and answered 0**,
rather than reporting "not derivable". Round 934's distinction is live on
authored data for the first time.

Round 942 predicted the fix was "a new arm, not a new gate" on the grounds that
the map brief had never asked for a telling. Stage A shows the stronger result:
the brief did not have to ask. What was missing was never discoverability.

---

## E2 — the first submission, frozen before anything was handed back

Both first submissions are preserved verbatim under
`run/stage-a-first-submission/` and `run/stage-b-first-submission/`.

### Stage A — `run/stage-a/first-import.log`

```
=== mnemosyne-cli import_sections ===
primitive: import_sections
target_kind: section
target_id: 20 created, 0 no-op
sidecar_path: …/v1/run/stage-a/docs/.atomic/workspace.atomic.json
written_bytes: 3044
exit=0
=== mnemosyne-cli import_facts ===
primitive: import_facts
target_kind: narrative_fact
target_id: 3 frames + 0 branches + 6 entity-kinds + 2 units + 20 entities
  + 10 predicates + 67 facts + 2 disclosure-plans + 72 disclosure-overrides
  created, 0 no-op
sidecar_path: …/v1/run/stage-a/docs/.atomic/workspace.atomic.json
written_bytes: 59254
exit=0
```

Clean on first submission, both verbs, with no iteration.

### Stage B — `run/stage-b/first-import.log`

```
=== mnemosyne-cli import_sections ===
…
target_id: 21 created, 0 no-op
exit=0
=== mnemosyne-cli atomic mutate FAILED ===
error: validation: import_facts: manifest disclosure_plan 0 override 17:
  validation: set_disclosure: surface object `the cistern lid` not present in
  the entity registry
exit=1
```

The author put a prose string in `surface.object`, which is an entity-id slot.
Their diagnosis, unprompted: *"the contract's manifest wire gives that field as
`"surface"?: {"scene": string, "object"?: string}` — typed only as `string`, with
no statement that it resolves against the entity registry."* They fixed it by
registering the lid as a real entity rather than deleting the field. Re-run
clean: `run/stage-b/second-import.log`.

**One import-shape finding, from the failure**: the manifest wire types
`surface.scene` and `surface.object` as bare `string` while the engine enforces
that one is a section id and the other an entity id. That is a contract surface
that under-declares what it accepts.

---

## E3 — the sealed self-report beside the render

### The join responds to the telling — the positive half

Same fact base, telling swapped, nothing else changed.

**Stage A** (`reads/stage-a-render-withheld-place.txt` vs `…-plain-place.txt`):

| | `withheld-place` | `plain-place` |
|---|---|---|
| Lamp Cellar existence (3 facts) | from sc-07 | **from sc-01** |
| Sella at the granary — place line | never | **sc-09** |
| Sella in the cellar — place line | never (see finding 3) | **sc-12** |

**Stage B** (`reads/stage-b-render-told.txt` vs `…-told-open.txt`): the only
place-line difference between the two whole renders is `p-cistern -> p-market`
plus `Yrsa is in the Old Cistern.` at sc-09.

This is the runbook's discriminating requirement met in the direction that
matters: **a join reading the gate's ground truth would have shown the hidden
place in every scene, and it does not.** Stage B's sealed oracle — *"from sc-09
through sc-19, eleven scenes, the reader has no told position for Yrsa
anywhere"* — holds exactly, and the plainly-moving character (Corin, 13
placements; Orin, 16) is placeable from his first scene in both stores.

### FINDING 1 — the map half of the join never looks at the telling

`MapProjection` is built by `mnemosyne_ops::transition_map_report(workspace,
sidecar, rules_override)`. **There is no telling parameter.** The edge set is
ground truth, always. The renderer then prints, for each disclosed place, its
exits from that unfiltered map (`crates/mnemosyne-render/src/lib.rs:128-134`).

So the place a scene discloses is telling-filtered; the roads leading out of it
are not.

**Stage A witnesses it at one scene.** The Lamp Cellar's existence — its
containment, the granary-floor hatch edge, and its dryness — is `withhold` with
`first_at` pinned to sc-07, and the author's sealed report says *"for the first
six scenes the reader does not know the town has thirteen locations; it has
twelve as far as the reader can see."* The render at **sc-06** prints:

```
The Salt Granary
p-granary -> p-lamp-cellar, p-market
```

`p-lamp-cellar` is named one scene before its existence is disclosed. The
inversion is sharper than the leak alone: the scene that actually reveals the
cellar, sc-07, prints **no place line at all**, because no character is placed
there in that scene. The reader is shown the cellar early and then not shown it
at the reveal.

**Stage B witnesses it at full scale.** `report-disclosure-coverage --telling
told` reads `never_planned=20`, and the twenty are all thirteen `adjacent` facts
and all seven `contains` facts. With the plan default `withhold`, the author has
withheld the entire road network from the reader — and `told-open` confirms it,
printing those twenty as prose lines that `told` does not print. **Yet the place
lines with exits are byte-identical across the two tellings.** Every market scene
in `told` announces `p-market -> p-cistern, p-toll-house, p-weighhouse` to a
reader who has been told about none of those roads.

This is the back-door disclosure `places_disclosed_in`'s own doc-comment warns
against, arriving through the exits rather than through the place.

### FINDING 2 — `DisclosedPlace` carries no frame, so a rumour renders as a place

```rust
pub struct DisclosedPlace<'m> {
    pub map: &'m DeclaredMapView,
    pub place: String,
    pub fact_id: String,
}
```

No `frame`, no `branch`. This is precisely the defect Round 940 repaired on the
claim axis — *drop the coordinate and a rumour reads as fact, a fork reads as
trunk* — still open on the place axis, which Round 938 built two rounds earlier.

**Both authors surfaced it independently, and Stage B's author asserted the
opposite in a sealed report.** Stage B, sc-15:

```
The Rain Market: the town says drowned
p-market -> p-cistern, p-toll-house, p-weighhouse
p-mooring -> p-water-stair
```

`p-mooring` is disclosed at sc-15 by exactly one fact: `tt-yrsa-02`, frame
`town-talk`, a **false rumour** ("Yrsa took a boat from the Skiff Mooring"). No
ground-truth `at` puts anyone at the mooring at sc-15; the scene is at the
market. The sealed report says of that fact and its sibling: *"Both are in the
`town-talk` frame, not in `ground-truth`, so they are machine-readable as
**belief** and never as position."*

**That claim is false, and it is false because the read does not carry the
coordinate the author correctly put in the data.** The author modelled the belief
exactly as the contract prescribes and was wrong about what the engine would do
with it. This is the Round 934 pattern again: not silence, but a reasonable
belief the substrate licensed.

Stage A reproduces it at sc-08, where `p-quay` is disclosed solely by
`f-talk-sella-quay` (frame `town-talk`, the town's false claim that Sella
drowned at the quay); no ground-truth placement puts anyone at the quay until
sc-10.

`fact_id` is carried, so a consumer *could* re-read the fact to recover the
frame. The renderer does not, and the row does not carry it — which is the
standard Round 940 set on the sibling axis.

### FINDING 3 — a reveal pinned at the scene its fact stops being current never renders

Stage B's sealed oracle: *"At sc-20 the pin fires and the whole middle becomes
placeable at once, retroactively, from sc-09 onward."*

`at-yrsa-05` (Yrsa at the Old Cistern, `canon_from` sc-09) is `withhold` with
`first_at` sc-20. It is superseded by `at-yrsa-06` (Yrsa at the Rain Market,
`canon_from` **sc-20**). So at sc-20 the pin fires on a fact whose extent has
just ended, and the render shows current placements per scene. **Under `told`,
Yrsa is never once rendered in the Old Cistern for the withheld stretch — not at
sc-09..sc-19, and not at sc-20.** The reveal produces no place line anywhere.

Stage A has the same shape — `f-sella-at-05` (cellar, sc-12) is pinned to sc-20
and superseded by `f-sella-at-06` (granary, sc-20) — and its oracle survives only
because a *different* fact carries the cellar at sc-20: Orin walks down into it.
Without that accident Stage A would read as Stage B does.

Two authors, two designs, the same intent — "reveal the middle at the end" — and
in neither store does the withheld placement itself ever become visible. Whether
`first_at` should mean "the reader may now place them there, retroactively" or
"the reader is told at this scene, if it is still true" is not settled by this
report. It is recorded as the disagreement it is.

### Recorded separately — the runbook's E3 rule is under-specified

The runbook says: *"the place line must NOT name it in any scene before N."*
Read strictly, Stage B fails at sc-04, where `p-cistern` is named because Yrsa is
genuinely and disclosedly at the Old Cistern early in the story. The author's
oracle is about a (character, location, scene-range) triple, not a location
alone. The strict reading manufactures a disagreement that is not one; the
charitable reading is applied above, and the rule wants rewriting before the next
arm.

### Not a defect, recorded as the known interaction it is

No author wrote a spoken-of-but-never-reached location: `unconnected places` is
`none` on both stores and `map_invented_place` fired nowhere. The interaction the
runbook flagged (a place carrying no edge cannot be named by the join) has no
witness here.

---

## Protocol — three deviations, all recorded rather than smoothed

1. **The rules pin was NOT patched, and the step turns out to be unnecessary.**
   Step 2 exists because the seed names no rules file, so the orchestrator must
   pin `rules_path` post-submission. **Both authors wrote `rules_path =
   "rules.json"` into `mnemosyne.toml` themselves, unprompted** (2/2). Applying
   the runbook's literal `printf` would additionally have deleted Stage A's
   `interval_severity = "reject"` line — editing an author's output, which the
   runbook forbids twice. The pin was verified to resolve, not assumed. The
   larger alternative Round 942 left open — letting the author do the wiring,
   making it a measured capability — **measured itself**, and the answer is 2/2.
   The Round 934 harness trap is gone for a reason the design did not predict.

2. **`revision_provenance: "exact"` does not exist.** The gate's vocabulary is
   `["derived-upper-bound", "declared-at-run"]`
   (`evidence_replay_smoke.rs:64`), and an unknown value panics. This kit uses
   `declared-at-run`. Round 898's design used the same non-existent word and the
   map-corpus run corrected it at execution time; Round 942 wrote the corrected-
   away word back into this runbook. A design term that has already been
   retired once came back because the runbook was written from the design and
   not from the gate.

3. **The self-report seal is enforced by the freeze, not by the author.** Stage
   B revised `self-report.md` after fixing the import error. The runbook says the
   report "is sealed at first submission and is never revised" but never
   instructs the author of that, and the author revised in good faith. The
   frozen copy preserved the seal and E3 above uses it. The revision was
   harmless — it adds the `surface` detail and leaves the oracle sentence
   unchanged — but the protocol depends on a freeze the author does not know
   about, and the next arm should either tell them or stop calling it a seal.

---

## Honest caveats

- n=2 authors, one world premise, one lineage orchestrating. One instance, not a
  distribution.
- Everything the brief mandated is decoration — the location count, the
  containment, the hidden character, the plain character. The evidence is what
  the brief did not mandate: which primitives were reached (Stage A) and what
  the render did with them.
- Stage B's E1 answers prove only that the author followed the brief.
- The three findings are **read against the sealed self-reports**, which are
  author prose. Findings 1 and 2 are additionally confirmed against the source
  (`transition_map_report` takes no telling; `DisclosedPlace` has no frame
  field), so they do not rest on the reports alone. Finding 3 rests on the
  supersession chain in `facts.json`, which is machine-checkable, plus a reading
  of intent that is not.
- **No repair is proposed here.** This arm measures; what to do about a
  telling-blind map, an unframed disclosed place, and a reveal that never fires
  is a decision for the ledger, not for the orchestrator of the run that found
  them.
- The render-acceptance gates (`validate-disclosure-leak` /
  `validate-render-fidelity`) stay out of scope: they read re-extracted prose
  and this corpus has none (Round 897).

## Where the evidence lives

- `run/stage-{a,b}/` — the authors' files as they now stand, plus their import
  logs.
- `run/stage-{a,b}-first-submission/` — frozen before anything was handed back.
- `reads/` — every mechanical read in full: both renders per store under both
  tellings, the gates, the coverage.
- The changelog entry is the SSOT for the decision.
