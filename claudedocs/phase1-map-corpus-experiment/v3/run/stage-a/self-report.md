# Vellmoor — self-report

## How many locations the town has

Thirteen. Two of them are quarters that hold other locations inside them —
Crownhead and the Midslope — and eleven are the places themselves: the Shrine
of the Still Water, the Almoner's House and the Bell Tower up in Crownhead; the
Tilted Market, Dyers' Row and the Ferryman's House down in the Midslope; and,
below both quarters and inside neither, the Cistern Stair, the Drowned Landing,
the Boat Shed, the Weir Gate and the Sunken Vaults.

I gave the quarters the same entity kind as everything else, `place`. A quarter
is not a different kind of thing from a house; what makes it a quarter is that
`contains` facts name places inside it. That is also what the engine reads: the
containment facts cut the map into scopes, and a quarter is a node in the scope
above it, which is how anyone walks out of one.

## How many ways lead between them

Nineteen. Each way is one `adjacent` fact and each fact is one direction,
because the transition rule is declared `undirected: false`, so a road you can
walk both ways is written twice. Those nineteen facts make eleven connections:
eight of them run both ways (Crownhead and the Midslope; the Midslope and the
head of the Cistern Stair; the Drowned Landing and the Boat Shed; the Boat Shed
and the Weir Gate; the shrine and the Almoner's House; the Almoner's House and
the Bell Tower; the market and Dyers' Row; the market and the Ferryman's House),
and three run one way only.

The three one-way ways are the interesting ones. The Cistern Stair falls to the
Drowned Landing and there is no fact for the return, because the lower flight is
a chute of fallen masonry: down is a way and up is not. The Weir Gate's
undersluice ladder climbs to the Midslope and nothing descends by it. And a
flooded passage runs from the Drowned Landing to the door of the Sunken Vaults,
with nothing running back.

Every edge joins two places that share a direct container, which is what the
contract requires: the quarters are joined to each other and to the waterline
places at the root scope, and the houses are joined only to their neighbours
inside their own quarter. Nobody in this town has an edge from a house in one
quarter to a house in the next; they cross by way of the quarters themselves.

## Which locations cannot be reached from the start

None. The story opens at sc-01 with Kess in the Almoner's House, and from there
every one of the thirteen locations can be reached by following the `adjacent`
facts: out of the Almoner's House into Crownhead, across to the Midslope, down
the Cistern Stair to the Drowned Landing, along to the Boat Shed and out to the
Weir Gate, and back up through the undersluice into the Midslope again. The
Sunken Vaults is on that list — the flooded passage from the Drowned Landing
reaches it.

Two things are worth saying plainly, because "reachable" and "reached" are not
the same word here.

The Sunken Vaults is reachable and is never reached. Nobody in the twenty scenes
goes there; the townsfolk talk about it in the market and Tull carries the talk
back and Orin rings over it at the end, and the door stays shut. I encoded it
that way on purpose: the vaults are *on* the map, with a real edge, and simply
never travelled in the canon order. Leaving it off the map — a named place with
no edges — is what the phrase "spoken of but never reached" suggests, and it is
the encoding the contract says fails.

The Sunken Vaults is also the one location with no way out. It has an edge in
and no edge back. That is a property of the map, not of the telling, and it is
true whether or not anyone ever goes.

## Which of my files says so

`facts.json` says how many locations there are and how many ways lead between
them, and it is the only file that says it. The locations are the thirteen
entities of kind `place` in the `entities` array; the ways are the nineteen
facts whose typed predicate is `adjacent`; the quarter structure is the six
facts whose predicate is `contains`. Reachability from the start is computed
from those facts and from nothing else — the map lives in the store, not in a
map file.

`rules.json` is what makes the engine read them as a map rather than as loose
claims. It declares the transition rule `movement-follows-the-map` over the `at`
predicate, naming `adjacent` as the edges and `contains` as the containment, so
that every step a person takes is checked against the ways above. It also
declares one place per person, one holder per key, and one waterline per place.
Without this file the adjacency facts are inert and the gate would report that
it evaluated nothing.

`order.json` says which places are *reached*. It is the canon order — the plain
line sc-01 through sc-20 — and it is the reason the Sunken Vaults is unvisited
while remaining on the map. Being unvisited is a property of this file, not of
`facts.json`.

`sections.json` says what the twenty scenes are. It comes first in the authoring
order because a fact naming an unregistered section is rejected.

## Two things that are not in any file

The contract is explicit that the keyed side tables have no file wire, and that
writing them into a fact manifest parses cleanly, exits zero, and builds
nothing. So the two parts of the brief that ask for them — "some journeys take
longer than others" and "one way is shut until something is true" — are *not*
in `facts.json`, deliberately, and the world is not complete until these verbs
are run against the imported store. I was not given a command to run, so I have
not run them; here is what they are.

The journey times, one call per way, in map minutes. They are asymmetric where
the town is asymmetric: the Crown Steps take twelve minutes down and twenty up,
and the climb through the undersluice is the longest single move in the town at
thirty-one, against a window of forty minutes that the sluice will hold — which
is the sum the consumer works out, not Mnemosyne.

    add-edge-cost --fact f-edge-crown-mid       --n 12 --unit minute
    add-edge-cost --fact f-edge-mid-crown       --n 20 --unit minute
    add-edge-cost --fact f-edge-mid-stair       --n  4 --unit minute
    add-edge-cost --fact f-edge-stair-mid       --n  6 --unit minute
    add-edge-cost --fact f-edge-stair-landing   --n  9 --unit minute
    add-edge-cost --fact f-edge-landing-shed    --n  5 --unit minute
    add-edge-cost --fact f-edge-shed-landing    --n  5 --unit minute
    add-edge-cost --fact f-edge-shed-weir       --n 24 --unit minute
    add-edge-cost --fact f-edge-weir-shed       --n 24 --unit minute
    add-edge-cost --fact f-edge-weir-mid        --n 31 --unit minute
    add-edge-cost --fact f-edge-landing-vaults  --n 15 --unit minute
    add-edge-cost --fact f-edge-shrine-almoner  --n  3 --unit minute
    add-edge-cost --fact f-edge-almoner-shrine  --n  3 --unit minute
    add-edge-cost --fact f-edge-almoner-bell    --n  2 --unit minute
    add-edge-cost --fact f-edge-bell-almoner    --n  2 --unit minute
    add-edge-cost --fact f-edge-market-dyers    --n  6 --unit minute
    add-edge-cost --fact f-edge-dyers-market    --n  6 --unit minute
    add-edge-cost --fact f-edge-market-ferry    --n  8 --unit minute
    add-edge-cost --fact f-edge-ferry-market    --n  8 --unit minute

The shut way is the climb out of the water, and what has to be true is that the
undersluice has come down. One condition, so no threshold: the default is to
require all of the set.

    add-edge-guard --fact f-edge-weir-mid --condition f-sluice-lowered

`f-sluice-lowered` is a real fact of the store — the undersluice coming down at
scene sixteen, superseding the raised state Mavey was keeping at scene eleven —
so the guard is a link between two things that exist, and the consumer, not
Mnemosyne, is what decides whether it holds during a given playthrough.

## What I could not check

I ran no commands, so every count above is read off the files I wrote rather
than out of a validator or a report. I have checked by hand that each character's
declared step is licensed by the map — siblings joined by an edge, or a crossing
into or out of a quarter, or a pair lifted to the two quarters that are siblings
— and that the guarded climb and the one-way descent are the only ways in or out
of the water. That is my reasoning, not a verdict from the gate.

## Wiring

The two side files are pinned in `mnemosyne.toml` under `[continuity]` —
`canon_order_path` for `order.json`, `rules_path` for `rules.json` — or passed
per-run as `--order order.json --rules rules.json`. The rules file is the opt-in:
until it is wired, the transition and exclusive rules above are not enforced.
