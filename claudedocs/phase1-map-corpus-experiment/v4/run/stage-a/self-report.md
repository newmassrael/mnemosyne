# Ondree — self-report

## The town

Ondree is a hill-town with the flood standing in its lower streets. It has
**twelve locations**. Two of them, the Upper Quarter and the Market Quarter,
are walled districts that hold other locations inside them; the other ten are
places a person stands in.

At the top of the hill, and at the root of the map, are six locations:
Crown Rock, the Upper Quarter, the Market Quarter, the head of the Drowned
Stair, the waterline, and Bell Isle out in the channel.

Three locations sit inside the Upper Quarter: the Shrine of Still Water, the
almshouse, and the Cistern House.

Three sit inside the Market Quarter: the fish market, the ropewalk, and the
ferry landing.

**Where this is said:** `facts.json`. The twelve locations are the entries of
kind `place` (or of kind `quarter`, which is declared a subkind of `place`) in
the `entities` array. Which location sits inside which quarter is the six
`contains` facts, `f-con-upper-shrine` through `f-con-market-ferry`.

## The ways between them

There are **nineteen ways**, each one a separate fact, and each one fact means
one direction of travel. That is nine ordinary two-way roads written as
eighteen facts, plus one way that goes one direction only:

- Crown Rock and the Upper Quarter gate, by the cut lane
- the Upper Quarter and the Market Quarter, by the Cross Way
- the Market Quarter and the head of the Drowned Stair, by a short alley
- the Market Quarter and the waterline, by the old towpath round the hill
- the waterline and Bell Isle, by open channel
- the shrine yard and the almshouse court
- the almshouse court and the Cistern House, by the covered walk
- the fish market and the ropewalk
- the ropewalk and the ferry landing

and then the nineteenth, which is the one-way one: **the Drowned Stair goes
down from its head to the waterline and there is no fact for the way back up.**
Its lower treads are weeded and undercut, and in scene thirteen they go from
under Sula as she steps off them. Anyone who goes down it walks the long
towpath to get home.

**Where this is said:** `facts.json` again — the nineteen facts whose typed
predicate is `adjacent`, ids `f-adj-crown-upper` through `f-adj-ferry-ropewalk`.
The map only counts as a map because `rules.json` declares the rule `town-map`,
which names `adjacent` as its adjacency predicate and `contains` as its
containment predicate, and sets `undirected` to false so that one fact stays
one direction.

Every way also has a walking time, and they are not the same: two minutes
across the market, eight minutes down the cut lane but twelve back up it, three
minutes down the Drowned Stair, and forty to forty-eight minutes on the towpath
that is the only way back from the water. And the alley from the Market Quarter
to the stair head is shut until two things are true at once — the sluice key is
in the warden's hand, and the water at the stair foot has gone slack.

**Where this is said: nowhere in the four JSON files, and that is not an
oversight.** The contract's side-tables section says edge costs and edge guards
have no file wire at all, that they are reached only through their own verbs
keyed by an existing fact, and that writing them into a facts manifest is a
silent no-op that parses cleanly and builds nothing. So they are written as the
verb calls that make them, in **`side-tables.sh`** — nineteen `add-edge-cost`
calls and two `add-edge-guard` calls against `f-adj-market-stair`, naming
`f-key-sula` and `f-tide-slack` as its conditions. I did not run that script;
no command was given to me to run. Until it is run, the store has the roads but
not their lengths, and the alley to the stair is not recorded as shut.

## What cannot be reached from the start

**Nothing.** Starting where the story starts, on Crown Rock at first light, all
twelve locations can be reached by following the ways in their allowed
directions: down the cut lane into the Upper Quarter, and from the quarter into
any of the three places inside it; along the Cross Way to the Market Quarter
and its three places; along the alley to the stair head, down the stair or the
towpath to the waterline, and across the channel to Bell Isle.

Two things are worth saying plainly, because they look like unreachability and
are not.

**Bell Isle is reachable but never reached.** The townsfolk say the drowned
bell still hangs in its tower and tolls at the turn of every tide; ground truth
says only that no boat has crossed the channel since the water came up, and is
otherwise silent on the bell, which is the honest record — nobody has been
there to know. Bell Isle is on the map, with a channel crossing in each
direction, and no one in the twenty scenes ever stands on it. Being unvisited
is a property of the telling, not of the map: the contract is explicit that
leaving such a place off the map instead would make it an invented place, and
naming it inside a quarter with no edges would be that plus a contained thing
off the map. So it has its edges and no one takes them.

**The head of the Drowned Stair cannot be regained from the waterline by the
stair.** You can get back to it — down to the waterline, forty-eight minutes up
the towpath to the Market Quarter, four minutes along the alley — but you
cannot climb what you came down. And the alley itself carries the guard, so a
consumer playing this world will find that route shut until the key and the
slack water are both true.

**Where this is said:** nowhere as a stored answer, because reachability is not
a field anyone writes. It is a reading of the nineteen `adjacent` facts and the
six `contains` facts in `facts.json`, taken together with the rule in
`rules.json` that says how those two relations combine — that a way joins two
locations sharing the same quarter, that stepping into or out of a quarter
needs no way at all, and that a step between two places in different quarters
is judged by the way between the quarters themselves. The one thing that *is*
recorded, as an ordinary fact rather than as map data, is why the stair is
one-way: `f-stair-broken`, in scene thirteen, which also carries a recorded
contradiction against `f-sula-trust`, the tide-warden's belief in the scene
before that the stair would carry her back up.

## The rest of the run, for the record

Twenty scenes, `sc-01` to `sc-20`, in `sections.json`; their discourse order is
the straight line of nineteen edges in `order.json`, which places every one of
them. Seventy facts in `facts.json` across three frames — `ground-truth`, what
the `townsfolk` say, and Sula's own belief — all on the default world-line
`main`, with no forks and no confluences, so there is no bare pre-fork trunk
left carrying unpaid setups. Three setups are marked `expected` and all three
are paid: the barred gate by `f-gate-open`, the almshouse roof's want of a rope
by `f-rope-delivered`, and the fouled cistern by the shrine giving up its
still-water bowls. Five people move through the town — Sula walks fifteen steps
of it, Vess and Oona two apiece — and each move is a succession from the fact
before it, so no one is in two places at once. No disclosure plan is declared;
the brief asked for a world, not for a telling of it.

Nothing here has been validated. I ran no command against the store.
