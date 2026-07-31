# Cawl Hill — self-report

## How many locations

Thirteen. Seven of them stand in the open town: the Crown Yard at the summit,
the Shrine Terrace on the seaward shoulder, the Upper Quarter, the Market
Quarter, the Pilgrim Stair, the Water Line where the flood stands against the
drowned doors, and the Lantern House out on the water. The remaining six sit
inside a quarter's wall: the Almshouse, the Rope Walk and the Cistern Yard
inside the Upper Quarter; the Weighhouse, the Salt Market and Tanners' Row
inside the Market Quarter.

All thirteen are entities of kind `place` (the two quarters are kind `quarter`,
which is declared a subkind of `place`) in `facts.json`. Which six sit inside a
quarter is said by the six `contains` facts in the same file — `f-in-almshouse`,
`f-in-rope-walk`, `f-in-cistern-yard`, `f-in-weighhouse`, `f-in-salt-market`,
`f-in-tanners-row`.

## How many ways lead between them

Twenty-one, counted as the engine counts them: one fact per direction. As a
person on the hill would count them, eleven — ten that can be walked both ways
and one that cannot.

Each way is an `adjacent` fact in `facts.json` with a typed leg, so the answer
to "where can I go from here" is a query over the store and never a reading of
the prose: collect every `adjacent` fact whose subject is the place you stand
in, and its objects are your choices. `rules.json` is what makes the engine hold
the story to those ways — the `moves-along-the-ways` rule declares `adjacent`
the adjacency predicate, with `undirected: false`.

That flag is the reason the ways are counted twice. The town is a two-way town
almost everywhere, but the Pilgrim Stair is not: `f-way-stair-water` says the
stair leads down from the stair head to the Water Line, and there is
deliberately no fact saying the reverse. Since edge symmetry is declared once
for the whole map and not per way, the only way to have one one-way descent was
to make every way one-way and write both halves of the other ten. So the ten
walkable connections are twenty facts, plus the one descent, and the asymmetry
is visible again in the travel times: six minutes down the Crown Yard lane and
nine back up.

The travel times and the shut way are not in any of the manifests. They are
keyed side tables with no file wire at all, so they are the `add-edge-cost` and
`add-edge-guard` lines in `commands.sh`. The gate at the foot of the Market
Quarter carries two guard conditions — Hesper holding the second stair key, and
the water standing slack — and the engine never evaluates them; it hands them
back and the consumer ANDs them.

## Which locations cannot be reached from the start

The story starts in the Crown Yard. Following the ways as declared, six
locations cannot be reached from there: the Almshouse, the Rope Walk, the
Cistern Yard, the Weighhouse, the Salt Market and Tanners' Row — that is, every
location inside a quarter's wall.

This is not an oversight and it is worth stating plainly, because it is the
shape the map model imposes. A way may only join two places that share the same
direct container, so the Almshouse can be joined to the Rope Walk but never to
the Crown Yard, and a quarter is not joined to the things inside it either — a
quarter leaves its own scope by being a node in the town's scope, which is
exactly what the Upper Quarter and the Market Quarter are. So the town is really
three maps that no way crosses: the open town of seven places, the Upper
Quarter's three, and the Market Quarter's three. Entering a quarter is not a
move along a way at all; it is a person being at the quarter and at a yard
inside it at the same time, which the `one-place-per-person` rule in
`rules.json` accepts as one place stated twice rather than two places at once,
because it names `contains` as its containment predicate. Hesper's walk in
`facts.json` is written that way throughout: a coarse chain that steps between
the quarters, and finer facts inside each quarter that refine it.

Two further things a reachability read should say. First, the Lantern House
*is* reachable — it has ways to and from the Water Line — but nobody in this
story ever goes there. It is spoken of at the Crown Yard, in the Almshouse and
at the Water Line, and the townsfolk frame holds that its keeper still trims the
wick, and no `at` fact ever puts a person there. Being unvisited is a property
of the telling, not of the map, so it stays on the map. Second, the descent runs
the wrong way for anyone who takes it: once below the gate, the only places
reachable from the Water Line are the Lantern House and the Water Line again.
Everything above — including the Crown Yard the story started in — is
unreachable from there. That is the point of the flood, and `f-treads-gone`
says so in prose while the missing reverse fact says so to the engine.

## Which file says what

- `sections.json` — the twenty scenes, as structure sections.
- `order.json` — which scenes each world travels: the main road runs sc-01 to
  sc-17; the `barred-stair` world-line forks at sc-13, where the bar is tried,
  and runs its own three scenes instead.
- `facts.json` — the registries, the map (`adjacent` and `contains` facts), the
  people, and the story. Both worlds are in here: on the main road the gate
  gives at slack water and two people go down and cannot come back; on
  `barred-stair` the tide turns first, the bar holds, and they go back up to the
  cistern instead.
- `rules.json` — turns the map on: one place per person, one holder per thing,
  movement only along declared ways, and the stair's passability window.
- `commands.sh` — travel times and the guarded gate, which no manifest can hold.
