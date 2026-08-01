# Map corpus, arm C — the hierarchy-crossing arc meets an author

Arm A is v1 (Round 904). Arm B is v2 (Round 910). The protocol is Round 898's,
frozen, unchanged: two blind authors, fresh context each, isolated directories,
blind to each other, allowed to run no commands, first submissions frozen before
any error was handed back, sealed `self-report.md` never revised. The brief is
byte-identical to arm A's and arm B's: fiction only, and the words *map*,
*transition*, *adjacency*, *edge*, *graph*, *route* appear nowhere in it.

**The only variable is `contract.txt`** — 218 lines and 49,193 bytes in arm B,
218 lines and 55,288 bytes here. Seven lines differ, and those seven lines are
the R911-R931 hierarchy-crossing arc: `evidence` at-or-before `canon_from`
(R912) on the field and again on the invariant line, `disclosure_encoding`
re-fronted as the general when-does-the-reader-learn axis (R912), the
`transition` rule-class paragraph rewritten so succession and the declared map
are two relations and not one (R930), `undirected` cut back to edge symmetry and
nothing else (R924), `containment` carrying the crossing model (R913) and the
chain-of-crossings-is-one-move rule (R925), and the rules-file wire carrying the
step side of the same declaration (R924 + R913/R925, R930).

Round 932 closed that arc on a measurement — four recorded corpora, eight rounds,
no verdict changed — and named the ceiling: the gate could then be sharpened only
by imagining authors, and **which of the arc's guards ever FIRES on real
authoring was unknown and unknowable from there.** This arm is the instrument
that answers it.

## The six corpora, measured on one tree in one sweep

Every store below was rebuilt from its manifests against this tree and gated in
a single run, so no number here is recalled. Arm A and arm B reproduce the
32 / 15 / 27 / 16 that R924 through R931 recorded.

| corpus | violations | steps | hierarchy crossing | lifted | unlicensed | unchained |
|---|---|---|---|---|---|---|
| v1 stage-a (arm A) | 32 | 30 | 0 | 30 | 5 | 1 |
| v1 stage-b (arm A) | 15 | 27 | 0 | 27 | 3 | 0 |
| v2 stage-a (arm B) | 27 | 31 | 0 | 31 | 0 | 74 |
| v2 stage-b (arm B) | 16 | 25 | 0 | 25 | 0 | 85 |
| **v3 stage-a (arm C)** | **0** | 20 | **2** | 18 | 0 | **0** |
| **v3 stage-b (arm C)** | **0** | 45 | **3** | 42 | 0 | **0** |

Finding kinds, same sweep: arm A stage-a is 14 `adjacency_cross_scope` + 11
`evidence_unreachable` + 5 `rule_transition_invalid` + 2 `map_contained_off_map`;
arm A stage-b is 10 + 2 + 3; arm B is `evidence_unreachable` and nothing else,
27 and 16; **arm C is empty in both stages.**

## What the arc bought

| measure | arm A (R904) | arm B (R910) | **arm C** |
|---|---|---|---|
| E1 — reaches for `class: transition` | YES | YES, both | **YES, both** |
| E2 — **first** submission imports | NO, twice | YES, twice | **YES, twice** (exit 0, no iteration) |
| gate verdict on the authored store | 32 / 15 | 27 / 16 | **0 / 0** |
| `evidence_unreachable` | 11 / 2 | **27 / 16** | **0 / 0** |
| `unchained_state_pairs` | 1 / 0 | **74 / 85** | **0 / 0** |
| hierarchy crossings declared | 0 / 0 | 0 / 0 | **2 / 3** |
| E3 — sealed location count vs machine | diverged in both | 12 v 12, 13 v 13 | **13 v 13, 13 v 13** |

**These are the first gate-clean blind authorings this experiment has produced.**
Both stores pass on the first submission with no iteration at all.

### 1. The crossing model has authored witnesses for the first time

R919 measured the frozen corpora and found 86 authored steps, every one of them
lifted, hierarchy crossings 0. Arm C declares **five**, across two authors who
could not see each other:

```
stage-a  p-shrine    -> q-crown          sc-04  ascent   licensed
stage-a  q-midslope  -> p-ferry-house    sc-19  descent  licensed
stage-b  q-tidegate  -> pl-ferry-steps   sc-14  descent  licensed   (main)
stage-b  q-tidegate  -> pl-ferry-steps   sc-14j descent  licensed   (gate-jammed)
stage-b  pl-watchtower -> q-crown        sc-17  ascent   licensed
```

All five are `licensed: true` with no edge, which is what R913 decided a crossing
is. The lift is exercised heavily beside them — steps whose judging scope is the
root and whose `lifted_from` / `lifted_to` are the sibling ancestors rather than
the endpoints.

**Arm A stage-a's author derived the whole three-way step model from the contract
alone and said so in the sealed report** — "siblings joined by an edge, or a
crossing into or out of a quarter, or a pair lifted to the two quarters that are
siblings" — and the gate agrees with them on all twenty steps.

### 2. The workaround the contract used to force is gone

Arm B's two authors both reported, independently and unprompted, that *the
contract never says how a person walks INTO a container*. Both worked around it
with a coarse/fine `at` refinement co-hold, and both paid for it: **74 and 85
unchained pairs**, surfaced and never gated. Stage B's sealed report called six
of its thirteen locations unreachable and "not an oversight ... the shape the map
model imposes".

Arm C's authors write the step. `unchained_state_pairs` is **0 in both stages**,
and neither sealed report contains the complaint. Arm C stage-b's author states
the model back plainly: "an edge between places sharing a container, or a
crossing into or out of a container, which needs no edge".

### 3. The dominant failure of arm B is at zero

Arm B's only finding kind was `evidence_unreachable`, 27 + 16 = 43, both authors
citing evidence from scenes later than the fact's `canon_from`. R912 put the rule
on the `evidence` field itself, on the invariant line, and re-fronted
`disclosure_encoding` as the general axis with `first_at` named as the place the
reader's discovery goes. **Arm C: 0 and 0.**

### 4. `undirected` is read as edge symmetry, by both, with the reason stated

R924 cut `undirected` back to edge symmetry and wrote that it does not declare
whether the rule is a map or a lifecycle. Both arm C authors wrote a DIRECTED map
and both explained it in the symmetry terms the repaired text uses — stage-a
"declared `undirected: false`, so a road you can walk both ways is written
twice"; stage-b "leaves `undirected` unset, so one `adjacent` fact means one
direction and a road you can walk both ways is two facts". Neither reached for
`true` to make their map "a map".

### 5. The not-declared / zero distinction fired on new data

`interval_unverifiable=not-declared` in both stages: neither author declared an
interval rule, and R924's `Option<usize>` rendering says so instead of printing a
`0` that reads as a measurement. Stage B's frontier does the same thing on the
map axis — see the finding below.

## The finding this arm exposes

**Stage B declares no predicate leg kinds, believes it does, and every gate is
green.** Its sealed report says the adjacency facts carry typed legs —

> Each is a typed claim `{subject: <place>, predicate: adjacent, object: {kind:
> entity, id: <place>}}`, so the answer is a lookup on the typed leg and never a
> read of the claim text.

— and `facts.json` declares `{"predicate_id": "adjacent", "object_kind":
"entity", "description": ...}` with no `subject_kind` and no
`object_entity_kind`, on that predicate and on all seven. The author has
conflated a *fact's* legs, which its instances do carry, with the *predicate's*
declared leg KINDS, which nothing here declares. Stage A declared both on every
predicate.

This lands on the one paragraph Stage B exists to test — *the engine must be able
to answer, without reading your prose, which locations one can move to from any
given location*. Half of it holds: `report-transition-map` reads facts, not
kinds, and prints all 21 edges. The other half does not, and the frontier says so
rather than answering:

```
stage-a: unconnected places [movement-follows-the-map]: none
stage-b: unconnected places [movement-follows-the-map]: not derivable — predicate
         `adjacent` declares no leg kind, so the registered place set cannot be
         asked for
```

That refusal is R924's discipline working — a `none` there would have read as a
clean verdict on a question that was never asked. But **no gate rejects the
store**, and the author's own answer to the brief's reachability question ("One:
the Drowned Bell") is therefore unconfirmable by the machine that was told to
answer it. The store is `violations: 0`.

## E3 — sealed report against machine

| claim | stage A | stage B |
|---|---|---|
| locations | 13 sealed / **13** machine | 13 sealed / **13** machine |
| ways | 19 sealed / **19** machine | 21 sealed / **21** machine |
| unreachable from start | "None" / frontier **none** | "the Drowned Bell" / **not derivable** |
| step licensing, hand-reasoned | confirmed: 2 crossing + 18 lifted, 0 unlicensed | confirmed: 3 crossing + 42 lifted, 0 unlicensed |
| `mnemosyne.toml` pins the rules file | **FALSE** — claims `rules.json`; the toml says `narrative-rules.json` | true, by luck |

The last row is the harness defect arm B recorded, on its third run. The
pre-supplied `mnemosyne.toml` names `narrative-rules.json` and the authors are
forbidden to read it. Arm A's two authors happened to write that name; arm B's
two both wrote `rules.json`; arm C's **split** — stage-a `rules.json`, stage-b
`narrative-rules.json`. Six authors, three matches. It is a coin flip, and here
it produced a false statement inside a sealed report. Gate runs for stage A pass
`--rules rules.json` explicitly, as arm B's did.

## Standing limits of this arm

- **The run form still has no witness.** `0 of them RUNS of consecutive
  crossings` in both stages, and `unmoved 0` in both. R925 built both and R932
  parked them as dormant; two more blind authors did not produce either. The
  carry is confirmed by measurement, not recalled.
- The side tables were **authored but not executed**, as in arm B. Stage A wrote
  19 `add-edge-cost` and 1 `add-edge-guard`; stage B wrote 21 and 2. Both
  authors found the verb-only wire unaided and both said the store is incomplete
  until the verbs run. Note the file convention is not prescribed and they
  diverged: stage B wrote `side-tables.sh`, stage A embedded the calls in its
  sealed report.
- **Neither store was prose-judged.** This arm measures structure only, as arms A
  and B did.
- No telling exists in either store (0 disclosure plans), so
  `report-playable-world --telling` was not run.
- The R896 verb-difference sweep (arm A's C2) was not re-run; arm B did not run
  it either.
- **A gate-clean store is not a correct story.** Both authors say plainly that
  they ran no commands and that every count in their report is read off their own
  files by hand. What is measured here is that the contract now leads two
  independent blind authors to a store the gate accepts on first submission — not
  that the worlds are good.
