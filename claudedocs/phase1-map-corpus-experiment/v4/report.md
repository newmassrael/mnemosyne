# Map corpus, arm D — the corrected contract, and a trap removed

Arm A is v1 (Round 904), arm B is v2 (Round 910), arm C is v3 (Round 933). The
protocol is Round 898's, unchanged: two blind authors, fresh context each,
isolated directories, blind to each other, permitted no commands, first
submissions frozen before any error was handed back, sealed `self-report.md`
never revised. The brief is byte-identical to all three earlier arms.

**Two author-observable inputs changed, and they answer different questions.**

1. `contract.txt` — 218 lines in both arms, 55,288 bytes in arm C and 56,509
   here, **four lines differing**: the two surfaces Round 934 corrected, which
   had credited every completeness check to wiring `containment` when
   `map_invented_place` additionally needs the adjacency predicate's leg kind.
2. The seeded `mnemosyne.toml` — Round 934 removed `rules_path`. Arms A, B and C
   all pinned `narrative-rules.json` while the protocol treated the toml as
   something the author must not consult, so the filename they invented was a
   coin flip that went wrong three times in six and, in arm C, put a false
   sentence inside a **sealed** report.

Keep the two apart when reading what follows: the leg-kind result is the
contract's, the toml result is the harness's.

## The eight corpora, measured on one tree in one sweep

| corpus | violations | steps | crossing | lifted | unmoved | runs | completeness NOTICE |
|---|---|---|---|---|---|---|---|
| v1 stage-a (A) | 32 | 30 | 0 | 30 | 0 | 0 | 1 |
| v1 stage-b (A) | 15 | 27 | 0 | 27 | 0 | 0 | 0 |
| v2 stage-a (B) | 27 | 31 | 0 | 31 | 0 | 0 | 1 |
| v2 stage-b (B) | 16 | 25 | 0 | 25 | 0 | 0 | 0 |
| v3 stage-a (C) | 0 | 20 | 2 | 18 | 0 | 0 | 0 |
| v3 stage-b (C) | 0 | 45 | 3 | 42 | 0 | 0 | 1 |
| **v4 stage-a (D)** | **0** | 19 | **3** | 16 | 0 | 0 | **0** |
| **v4 stage-b (D)** | **0** | 18 | **0** | 18 | 0 | 0 | **0** |

Arms A–C reproduce their recorded numbers exactly. **Arm D is the second
consecutive gate-clean arm: four blind authorings in a row now pass on the first
submission with no iteration at all** (E1 YES twice, E2 YES twice, exit 0).

## What arm D measures

### 1. The contract correction, and an author who names it

Before Round 934 the contract attributed the completeness checks to
`containment`, which all six authors wired; **three of the six declared no leg
kind and were never told the class had not run.** Arm D is **2 of 2**, taking
the record to 5 of 8, and the completeness NOTICE fires on neither.

Two authors is not a rate, and this report does not claim one. What it does hold
is the mechanism, in arm D stage-b's own sealed words:

> The adjacency predicate declares `subject_kind`/`object_entity_kind` = `place`
> on both legs, which the contract says is separately required or the
> `map_invented_place` completeness class emits nothing.

That is the sentence Round 934 added, read back by an author who could not see
the round that wrote it.

### 2. The harness trap is gone, because the guess is gone

**Both arm D authors wrote `mnemosyne.toml` themselves**, byte-identically:

```
[workspace]
[continuity]
canon_order_path = "order.json"
rules_path = "rules.json"
```

No filename was invented against a hidden pin, so no sealed report can be wrong
about one. Worth recording precisely, because arm B's report overstated the
constraint: **the firewall never forbade files inside the author's own
directory** — it forbids reading *elsewhere in the repository*. Arms A–C simply
did not touch the toml. With nothing pinned, both arm D authors read it, saw the
contract's instruction to wire the rules file, and did so. The wiring is now a
measured capability rather than a coin flip, which is the larger alternative
Round 934 named and declined to build; the authors took it themselves.

### 3. The crossing model keeps firing — and the lift is doing most of the work

Arm D adds 3 crossings (stage-a), for **8 across the four post-contract
authors** where all four pre-contract corpora had zero. All are `licensed` with
no edge.

Arm D stage-b declares **zero crossings and eight lifted-to-ancestor steps**,
which is worth stating plainly because its sealed report says the opposite:

> Movement in and out relies on the Round 913 crossing rule (a
> container-to-contained step needs no edge)

Every one of its in-and-out movements is in fact a *place-to-place* step lifted
to the two sibling ancestors — `loc-market-square -> loc-cooper-yard` judged as
`loc-market-square -> loc-highwater-quarter`, and seven more like it. Both
clauses are R913 and both are correct; the author used one and described the
other. **This is not a gate hole** — `validate-continuity` separates them in its
own summary line ("hierarchy crossing 0 / lifted 18") — it is the standing limit
that these authors run no commands, and it is the first time the two clauses
have been distinguishable in authored data at all.

### 4. Two classes still have no witness, in eight authorings

`unmoved` and RUNS of consecutive crossings are **0 in every one of the eight
corpora**. Round 925 built both; Round 932 parked them as dormant; Round 933
measured the absence a second time; this is the third. The reopen condition
stays what R932 set — authored data that touches them — and eight blind authors
have not.

## E3 — sealed report against machine

| claim | stage A | stage B |
|---|---|---|
| locations | 12 sealed / **12** machine | 12 sealed / **12** machine |
| ways | 19 sealed / **19** machine | 20 sealed / **20** machine |
| unreachable from start | "Nothing" / frontier **none** | "nothing is cut off" / frontier **none** |
| place set derivable | yes | yes |
| which R913 clause carries the world | crossing + lift, as reported | **reported as crossing; measured as lift** |

No count diverged in either stage — the first arm in which that is true of both.
Arm A diverged in both stages, arms B and C matched on counts.

## Standing limits of this arm

- **Side tables authored, not executed**, as in arms B and C: stage A wrote 19
  `add-edge-cost` and 2 `add-edge-guard`, stage B wrote 20 and 1. Both again
  found the verb-only wire unaided and both stated the store is incomplete until
  the verbs run.
- **Neither store was prose-judged.** Structure only, as in every prior arm.
- No telling exists in either store, so `report-playable-world --telling` was not
  run. The R896 verb-difference sweep was not re-run; no arm since A has.
- **Two inputs changed, not one.** Attributions are kept per-claim above, but a
  reader should not treat arm D as a single-variable arm the way arm B and arm C
  were.
- Arm D stage-a declared `quarter` as a **subkind** of `place` and it worked;
  arm C stage-a considered exactly that and chose against it because the
  contract does not say whether a predicate's declared leg kind accepts a
  subkind. Nothing in Round 934 addressed that, so this is author variance, not
  a repair taking effect — and the contract's silence on it is still there.
- **A gate-clean store is not a correct story.** Both authors state they ran no
  commands and that every count in their report is read off their own files by
  hand.
