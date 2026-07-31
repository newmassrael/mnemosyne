# Harrow Steep — self-report

## How many locations the town has

Twelve. They are the Hill Crown, the Bell Tower, the Keeper's House, the head of
the Forty Steps, the Waterline, the Ferry Stone, the Cistern Gate, the Drowned
Quarter, the Sunken Market, the Boat Yard, the Almshouse and the Under-Church.

Eight of those sit at the top level of the town. The Drowned Quarter is one of
the eight and is also a container: the Sunken Market, the Boat Yard, the
Almshouse and the Under-Church are the four places inside it. That is stated by
the four `contains` facts at the head of the fact list in `facts.json`
(`fc-quarter-holds-market`, `fc-quarter-holds-yard`,
`fc-quarter-holds-almshouse`, `fc-quarter-holds-under-church`). The Quarter is
registered with the entity kind `quarter`, which is declared in the same file as
a subkind of `place`, so it counts as a location as well as a container.

## How many ways lead between them

Thirteen ways, written as twenty-five `adjacent` facts in `facts.json` — the
fact ids all begin `fe-`. The map is directed, one fact per direction, so twelve
of the ways are two-way (twenty-four facts) and one way is one-way (a single
fact).

Nine of the ways are in the town's top-level scope: crown to bell tower, crown to
Keeper's House, bell tower to Keeper's House, crown to stair head, the Forty
Steps from the stair head down to the Waterline, Waterline to Ferry Stone, Ferry
Stone across the water to the Drowned Quarter, Ferry Stone to Cistern Gate, and
the cistern stair from the Cistern Gate up into the Keeper's House. The other
four are inside the Drowned Quarter: market to boat yard, boat yard to
almshouse, market to almshouse, and the flooded stair from the almshouse floor
down to the Under-Church door.

The one-way way is the Forty Steps. `fe-stair-head-waterline` exists and there is
no fact for the reverse, and the map rule in `rules.json`
(`movement-rides-the-map`) is declared with `"undirected": false`, so an
`adjacent(a, b)` fact admits only that direction. Mira goes down the broken stair
in scene 6 and cannot come back up it; she has to come round by boat and up
through the cistern.

Travel time is not in any of the JSON files, because the edge-cost table has no
file wire. It is in `commands.sh`, one `add-edge-cost` call per direction, in
minutes: two minutes across the crown, six down the broken stair, twenty-five for
the crossing out to the Quarter and thirty for the loaded row back, eighteen for
the climb up the cistern against twelve for the way down.

The shut way is also in `commands.sh`. Both directions of the cistern stair carry
an edge guard with two conditions — `f-cond-key-in-keepers-hand` (the sluice key
is in the Keeper's keeping) and `f-cond-slack-low-water` (the flood stands at
slack low water). No threshold is set, so the two conditions are ANDed: the
passage is shut until both hold. That is why the Keeper has to come down at dusk
in scene 18 and why the three of them wait at the barred gate in scene 17.

## Which locations cannot be reached from the start

The story starts on the Hill Crown, in scene 1.

Walking the map edges outward from the Hill Crown reaches eight of the twelve
locations: the Bell Tower, the Keeper's House, the stair head, the Waterline, the
Ferry Stone, the Cistern Gate and the Drowned Quarter, plus the crown itself.
Nothing at the top level is stranded — the one-way stair does not cut the town in
two, because the Cistern Gate and the cistern stair close the loop back up to the
crown.

The four locations inside the Drowned Quarter — the Sunken Market, the Boat Yard,
the Almshouse and the Under-Church — cannot be reached from the Hill Crown by
following edges, and this is by construction rather than by accident. Containment
partitions the map into scopes and an edge may only join places with the same
direct container, so the Quarter's four interior places form their own scope and
no edge crosses into it. A traveller enters by refinement instead: they stand in
the Drowned Quarter (an edge from the Ferry Stone gets them there), and then more
precisely in the Sunken Market, which is the same place stated finer. In
`facts.json` that is `f-at-mira-quarter-sc08` and `f-at-mira-market-sc09`
co-holding, which the `one-place-per-person` rule in `rules.json` accepts because
it is declared refinement-aware, with `"containment": "contains"`.

Within the Quarter's own scope all four interior places are reachable from each
other, the Under-Church included.

So no location is off the map. One location is never reached in the telling: the
Under-Church. It has two edges, up and down the flooded stair from the Almshouse
floor (`fe-almshouse-under-church` and `fe-under-church-almshouse` in
`facts.json`), and it has travel costs in `commands.sh` like every other way, but
no scene is set there and no `at` fact ever puts a person there. The townsfolk
speak of it on the crown in scene 4 (`f-town-says-under-church-rings`, held in
the `townsfolk` frame) and Hesk insists on it in scene 12
(`f-hesk-says-under-church-rings`, held in his own frame), and both of those are
recorded as conflicting with the ground-truth fact that the ringing is Hesk's own
hand-bell. Being unvisited is a property of the telling, not of the map, so the
file that says the Under-Church is never reached is `order.json` together with
`sections.json`: the canon order runs sc-01 through sc-21 and none of those
twenty-one scenes is set at the Under-Church.

## Which of my files says what

- `sections.json` — the twenty-one scenes, in the one document `doc-harrow-steep`.
- `facts.json` — the registries (four frames, four entity kinds, two units,
  nineteen entities, seven predicates) and ninety-seven facts: the four
  `contains` facts, the twenty-five `adjacent` facts that are the map, and the
  story facts scene by scene.
- `order.json` — the canon order, a straight line of twenty edges over the
  twenty-one scenes, main trunk only, no branches.
- `rules.json` — the three rules that turn the gate on: one place per person
  (refinement-aware), movement rides the map (directed, scope-partitioned), one
  holder per thing.
- `commands.sh` — the two keyed side tables that no manifest can carry:
  twenty-five edge costs and the four edge-guard conditions on the cistern stair.
