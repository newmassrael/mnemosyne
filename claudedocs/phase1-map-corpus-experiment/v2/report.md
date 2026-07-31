# Map corpus, arm B — the same brief against a repaired contract

Arm A is v1 (Round 904). The protocol is Round 898's, frozen, unchanged. The
brief is byte-identical to arm A's: fiction only, and the words *map*,
*transition*, *adjacency*, *edge*, *graph*, *route* appear nowhere in it.

**The only variable is `contract.txt`** — 211 lines in arm A, 218 after R906
(derived manifest roster + `units` + `sections_wire`, containment restated to the
R716 scope/portal model), R908 (the interval bound), and R909 (`side_table_wire`,
and "the map is possibility, not itinerary").

Two blind authors, fresh context each, isolated directories, blind to each other,
allowed to run no commands. First submissions frozen before any error was handed
back.

## What the repairs bought

| measure | arm A (R904) | arm B |
|---|---|---|
| E1 — reaches for `class: transition` | YES | YES, both |
| E2 — **first** submission imports | **NO, twice** (`sections.json` as an object) | **YES, twice** (exit 0, no iteration) |
| `adjacency_cross_scope` findings | **24 of 39** | **0** |
| Quantity authored from a file | "impossible", in both sealed reports | **2 and 3 authored** |
| edge costs / guards | unreachable — no file wire, no documented verb | **29 and 23 calls in `commands.sh`** |
| E3 — sealed location count vs machine | diverged in both stages | **12 vs 12, 13 vs 13** |

Every gap R904 measured is closed, and closed in the way the repair predicted.
The sections wire ended E2's double failure. The `units` roster ended a false
conclusion two independent authors had put in sealed reports. `side_table_wire`
made two brief requirements authorable that previously had no path at all.

## What it cost, and what it exposed

**A new dominant failure: `evidence_unreachable`, 27 + 16 = 43 across the two
arms, and nothing else.** Both authors cite evidence from scenes LATER than the
fact's `canon_from` (24 such facts in stage A alone). The contract states the
rule exactly once, buried inside the `branches` registry description and framed
throughout as a fork-and-sibling concern — "citing a sibling's exclusive scene is
rejected". Neither author has meaningful branching, and both hit it on `main`.
Whether the check is right or over-strict is **open**, and is deliberately not
resolved here: R909 was written because reading an entry is not a demonstration.

**★ The finding that lands on R909.** Both authors, independently and without
seeing each other, reported the same missing thing in their own words: *the
contract never says how a person walks INTO a container.* Stage B's sealed
self-report states the consequence outright — six of its thirteen locations
"cannot be reached from there ... that is, every location inside a quarter's
wall", and calls it "not an oversight ... the shape the map model imposes".

Verified mechanically across all three corpora, including my own:

```
stage-a: containers=1 places-inside=4  parent-child edges=0  edges from outside INTO an inside-place=0
stage-b: containers=2 places-inside=6  parent-child edges=0  edges from outside INTO an inside-place=0
R909 town:                                                    edges from outside INTO an inside-place=0
```

A parent-child edge is cross-scope, so it is structurally unavailable; nothing
inside a container is reachable by walking. Both authors worked around it with a
coarse/fine `at` refinement co-hold, and both paid for it in unchained pairs
(74 and 85, surfaced but never gated).

**R909 authored a town with this exact property and called the contract
sufficient.** Its own carry named the blind spot that hid it: `undirected: false`
means `MapDisconnected` never runs, and that corpus "exercises the MAP axis and
nothing else" — no one walks in it. Two authors who did put people in motion hit
the wall in their first attempt.

## Standing limits of this arm

- The pre-supplied `mnemosyne.toml` names `narrative-rules.json`; both authors
  wrote `rules.json`, which they were not permitted to read the toml to learn.
  The gate is fail-loud about the missing file, and this is a **harness defect,
  not an author error** — arm A's toml was identical and its authors happened to
  match. Gate runs here pass `--rules rules.json` explicitly.
- `undirected: false` is forced by the brief's one-way descent, so
  `MapDisconnected` never ran in either arm. Neither "0 cross-scope" nor
  "violations N" includes a connectivity verdict.
- The authors' prose was never judged. This arm measures structure only.
