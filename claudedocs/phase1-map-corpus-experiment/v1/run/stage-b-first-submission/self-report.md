# The flooded hill-town: what is in the store

## How many locations

The town has **twelve locations that a person can stand in**: the bell house, the
shrine of standing water, the dry market, the cistern yard, terrace lane, the vetch
house, the granary, the scour, the water stair, the boat shed, the rope jetty, and
the drowned mill.

There are **three more place-space entities that are not locations in that sense** —
the crown quarter, the terrace quarter and the waterline quarter. They are regions,
not positions: each holds four of the twelve as its nodes, nobody is ever *at* a
quarter, and no way leads into or out of one. So fifteen entities of the place space
in all, twelve of them walkable.

All fifteen are registered in `facts.json` under `entities` (kind `place` for the
twelve, kind `quarter` for the three). The quarter memberships are twelve `contains`
facts in the same file, the ones whose ids begin `f-in-`.

## How many ways lead between them

**Twenty-three ways**, counted the way the engine counts them: twenty-three
`adjacent` facts in `facts.json`, the ones whose ids begin `f-adj-`. Each one is a
single permitted move in a single direction.

Counted as passages rather than as directions, there are thirteen: ten that can be
walked both ways (authored as two facts each, one per direction) and three that go
one way only (one fact each). The three one-way ways are the head of the scour, the
scour's mouth at the boat shed, and the mill race.

I authored them directed rather than undirected on purpose. The contract offers
`undirected: true` on the transition rule, which would make one fact stand for both
directions — but then no way in the town could be one-way, and the descent the town
cannot climb back up would be inexpressible. So `narrative-rules.json` sets
`undirected: false`, and symmetry is something I state twice where it is true rather
than something the rule grants everywhere.

**The descent that cannot be climbed:** terrace lane drops into the scour
(`f-adj-lane-scour`) and the scour lets out at the boat shed's broken wall
(`f-adj-scour-shed`). There is no fact for either reverse, so a mover who goes down
the scour comes out at the waterline and has to walk back up around by the water
stair. Mira does exactly that in scenes 13, 14 and 16. The prose fact
`f-scour-one-way` says the same thing in words, but the prose is not what the engine
reads — the absence of the two reverse facts is.

**The way that is shut until something is true:** terrace lane into the granary
(`f-adj-lane-granary`). The edge exists on the map at all times; what is conditional
is the crossing. `edge-guards.json` puts a guard on that edge fact requiring two
conditions at once — `f-key-held` (Kell has his father's key off the boat shed hook)
and `f-water-below-third` (the flood has fallen below the third step of the granary
stair) — with no K-of-N threshold, which is the AND default. Mnemosyne holds that
declaration and checks only that the edge and both conditions resolve; whether the
guard holds is the runtime's question, not the store's. Both outcomes are in the
store as world-lines instead: on `main` the door stays barred (`f-granary-barred`
never gets a successor there), and on the fork `sluice-drawn` the water falls, the
key turns, and `f-lane-door-opens` supersedes it.

**Ways that take longer than others:** `edge-costs.json` gives every one of the
twenty-three a cost in minutes. Uphill and against the current cost more than the way
back down — terrace lane up to the market is eight minutes where the market down to
the lane is five, the water stair up to the lane is nine where the lane down to the
stair is six — and the scour costs one minute in each of its two legs, because it is
a fall rather than a walk.

## Which locations cannot be reached from the start

The story starts at the bell house (scene `sc-01`). Following the `adjacent` facts
from there, eleven of the twelve are reachable.

**One is not: the drowned mill.** It is the place the townsfolk speak of and nobody
in the story ever reaches. It is a real node of the map — the mill race carries out
of it to the rope jetty (`f-adj-mill-jetty`), which is how a brass lamp arrives at
the jetty in scene 16 — but that is its only way, and it points outward. Nothing in
the town leads in. The town believes it still turns (`f-mill-turning`, held in the
`townsfolk` frame, not in ground truth); ground truth records only that nobody has
stood in it since the water took the second step (`f-mill-unentered`).

Two things worth stating precisely rather than leaving to be inferred:

- The three quarters are not reachable either, but not in the same sense: they are
  containers, no way touches them, and being unreachable is what a region is. Only
  the mill is a walkable place with no way in.
- The granary is reachable *on the map* — one way leads into it from terrace lane —
  but that way is the guarded one, so whether a given playthrough can reach it
  depends on a condition the store declares and does not evaluate. On `main`, nobody
  goes in; on `sluice-drawn`, Kell does.

**Which file says so:** `facts.json`, and it alone. The reachable set is computable
from the twenty-three `f-adj-` facts in it and nothing else — no prose is read, no
other file is consulted. `narrative-rules.json` is what makes those facts binding
rather than decorative: its `moves-follow-the-map` rule names `adjacent` as the
adjacency predicate for the `at` predicate, so every move any character makes has to
be one of the twenty-three, in the declared direction. `edge-guards.json` and
`edge-costs.json` hang extra readings on individual edges; neither adds or removes a
way.

## The rest of what is here, and what I had to decide

**Scenes.** Twenty-one, `sc-01` to `sc-21`, registered in `sections.json` and placed
in `order.json`. Eighteen are on the trunk; `sc-01` through `sc-15` are shared by
both world-lines, `sc-16` to `sc-18` continue `main`, and `sc-19` to `sc-21` are the
fork's own road, declared under `branches` in `order.json` so the fork's ending can
be told from the trunk's. Every one of the twenty-one is placed by the order and
every one carries facts. Five people move through the town — Orrin the bell-keeper,
Sarn who keeps the shrine, Vetch who cuts the tide marks, Kell the boatman, Mira who
listens at the tally stone — and each move is authored as an `at` fact that
supersedes that person's previous one, so the twenty-two `at` facts form five chains
and each step of each chain is one of the twenty-three ways. Something happens at
every scene and at every one of the eleven reachable locations.

**Where I had to choose a shape the contract does not fix.** Three places:

1. `sections.json`. The contract names the sections registry and says a section id
   must exist before a fact cites it, but sections are not one of the seven arrays of
   the fact manifest — they are written with `add-section`, and the contract gives no
   wire shape for them. I mirrored the shape the manifest uses for its own registries:
   an object with one array, each entry `{ "section_id", "title" }`. If the loader
   wants something else, that file is the one to adjust; nothing in `facts.json`
   depends on its shape, only on the ids.

2. `edge-costs.json` and `edge-guards.json`. Same situation, and worth being blunt
   about: `edge_costs` and `edge_guards` are registries written with `add-edge-cost`
   and `add-edge-guard`, keyed by the edge fact id, and neither is one of the seven
   manifest arrays. So those two files are **argument lists for those primitive
   calls, not something `import-facts` can read**. One `add-edge-cost` per entry;
   one `add-edge-guard` per condition, so two calls for the single guard.

3. The costs need a unit registered first. A `quantity` object's unit must be a
   member of the units registry, and units are also declared by a primitive
   (`add-unit`), not by the manifest — so `add-unit minute` has to run before the
   twenty-three costs will take. `edge-costs.json` names that prerequisite in its own
   `units_required` field. This is also why no fact in `facts.json` carries a
   `quantity` typed object: the eleven drowned steps of the water stair and the
   fourth step of the granary stair are stated in prose claims instead, since I could
   not register a unit for them through the manifest.

**What I did not author.** No disclosure plan — the town's one secret, the mill,
is carried by the frame axis (the `townsfolk` frame says it turns; ground truth says
only that nobody has been in it), and a telling would have been a render decision the
brief did not ask for. No quest predicates: nothing here is a tracked obligation. No
interval rules: they would need quantities, and quantities need the unit registry.

**One thing I could not check.** I was asked not to run anything, so nothing here has
been through `validate-continuity`, `propose-verdict`, or a JSON parser. The
reference discipline is hand-verified — every frame, branch, entity, predicate and
section id used by a fact is declared earlier in `facts.json` or in `sections.json`;
every `supersedes_in_frame` target is in the same frame and appears earlier in the
array; both `pays_off` targets are the one setup fact `f-cistern-brackish`, paid once
on each world-line so neither world leaves it dangling; and every fork fact cites
only scenes on the fork's own road (`sc-01`–`sc-15`, `sc-19`–`sc-21`), never `main`'s
exclusive `sc-16`–`sc-18`. Hand-verification is not the gate, and if something in
here is wrong, the likeliest kinds are a mistyped id or a shape I inferred in the
three places named above.
