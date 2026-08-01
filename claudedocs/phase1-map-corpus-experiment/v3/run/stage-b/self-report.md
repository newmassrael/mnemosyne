# Kelder in flood — self-report

## What was written

Five files, plus this one.

- `sections.json` — the 21 scenes, as the bare JSON array the sections wire asks for.
- `facts.json` — the fact manifest: 3 frames, the one branch, 4 entity kinds, 2 units, the
  19 entities, the 7 predicates, and 100 facts (28 of them the map itself: 21 edges and 7
  containments; 33 of them people's positions; 39 of them what happens).
- `order.json` — the canon order: `main`'s 17 scene edges, and the `gate-jammed` fork's
  own three-edge road segment under `branches`.
- `narrative-rules.json` — the three rules that turn the continuity gate on. Pinned by the
  existing `mnemosyne.toml` as `rules_path`, as `order.json` is pinned as
  `canon_order_path`.
- `side-tables.sh` — the half of the authoring that has no file wire (see the last
  section).

## How many locations the town has

Thirteen. Eleven ordinary places — the Long Causeway, the Water Stair, the Old Cistern,
the Drowned Bell, the Shrine of Still Water, the Almshouse, the Watchtower, the Floating
Market, the Ferry Steps, the Eel House, the Pump House — and two quarters, the Crown and
the Tidegate, which are locations in their own right because a container in this model is
not a label over its contents but a node you can stand in and walk through. Obed stands in
the Crown at scene seventeen; Kesh comes out of the hill into the Tidegate at scene
thirteen.

Seven of the thirteen sit inside a quarter: the shrine, the almshouse and the watchtower
are in the Crown; the market, the ferry steps, the eel house and the pump house are in the
Tidegate. That is said by seven `contains` facts in `facts.json`. The remaining six — the
two quarters and the causeway, stair, cistern and bell — sit in the root scope, with no
container.

## How many ways lead between them

Twenty-one, counting each way as one way, which is how this map is built: the transition
rule leaves `undirected` unset, so one `adjacent` fact means one direction and a road you
can walk both ways is two facts. Nine of the twenty-one are pairs, making nine two-way
roads; the other three are single, and each is single on purpose.

- The Water Stair goes down into the Old Cistern and there is no fact for the way back.
  That is the descent that can be taken and not undone. Below the ninth tread the stair
  has fallen into the flood, and the town says nobody who went down it came back up it.
  (You can still get back to the Crown eventually, by wading out under the hill and taking
  the long causeway round; you cannot get back up the stair.)
- The cistern's overflow channel runs out into the Tidegate, and not back in.
- The drowned tower spills east into the cistern, and nothing runs the other way.

## Which locations cannot be reached from the start

One: the Drowned Bell.

The story starts on the Long Causeway at scene one. From there the engine's step model —
an edge between places sharing a container, or a crossing into or out of a container,
which needs no edge — reaches twelve of the thirteen locations. The Drowned Bell has an
edge, `f-edge-bell-cistern`, so it is a real place on the real map and not an invented one;
but that edge points outward only, and nothing in the town points at the tower. The
current runs out of it and there is no way in against it. So it is spoken of and never
reached: the townsfolk say the bell still rings when the water turns, Mira and Obed see its
top course of stone from the watchtower, Kesh hears it struck slow beyond the cistern's
east wall — and no scene in either world-line puts anybody there. Being unvisited is a
property of the telling; being unreachable is a property of the map; here it happens to be
both, and neither was achieved by leaving the tower off the map.

(If you ask the same question of the adjacency edges alone, ignoring the container
crossings, you also lose the seven places inside the two quarters, because the way into a
quarter is the crossing and not an edge. The bell is unreachable under either reading.)

## Which file says so

`facts.json` says all of it, and says it in data, not in prose.

- Which locations one can move to from a given location is the set of `adjacent` facts
  whose typed subject is that location. Each is a typed claim
  `{subject: <place>, predicate: adjacent, object: {kind: entity, id: <place>}}`, so the
  answer is a lookup on the typed leg and never a read of the claim text. The twenty-one of
  them all hold from `sc-01` onward, so they answer for every point in the story.
- Which locations sit inside which quarter is the seven `contains` facts, same shape.
- `narrative-rules.json` is what makes the store enforce this rather than merely record it:
  the rule `movement-follows-the-map` is a transition rule over `at`, with `adjacency:
  adjacent` and `containment: contains`, so every declared move a person makes is checked
  against those edges, and an edge that joined two places in different quarters would be
  rejected as cross-scope. `one-place-per-person` is an exclusive rule over `at`
  per subject (refinement-aware, so standing in the Tidegate and standing at its ferry
  steps are one position and not two), and `one-keeper-per-thing` is an exclusive rule over
  `holds` per object, which is what keeps the pump-house crank in one pair of hands.

## The scenes, and the two ways it can end

Twenty-one scenes. Eighteen on `main`, and three on a fork.

Kesh walks in along the causeway, goes down to the ferry steps and finds the tide-gate
jammed open on a bar of silt and the pump house — with the town's winter grain in its loft
— sealed under two metres of water. He is rowed to the market, goes on to Var's eel house
and comes away with the crank Var saved off the works. He climbs back to the Crown, hears
at the shrine what the town says about the Water Stair, and goes down it anyway. Meanwhile
Obed counts an empty bed in the almshouse ward and finds the child on the watchtower stair.
In the cistern Kesh frees the tide-gate's lower catch; the channel carries him out under
the hill; Var brings the gate down; the water falls; they cross to the wharf and the grain
is dry. Obed walks the Crown after dark calling the Tidegate families by name, and Mira
adds no name to the tally.

The fork, `gate-jammed`, leaves `main` at scene eleven — at the head of the stair, before
the catch is freed, because a fork inherits everything at or before its departure point and
a road that denies the freeing must not inherit it. On that road the catch will not turn,
the crank goes out of Kesh's numb hand into the silt, he comes out empty-handed, the gate
stays up and the grain moulds behind the water. `main` continues as one of the two roads
rather than being abandoned at the fork, so no trunk setup is left dangling on a bare
prefix; and each of the three setups marked `expected` is paid on both roads — the empty
bed before the fork, the crank and the sealed pump house once on each side of it.

## The two halves that are not one artifact

Travel times and the shut way are **not** in `facts.json`. The contract is explicit that
the keyed side tables are reached only through their own verbs and that writing an
`edge_costs` or `edge_guards` array into a fact manifest is a silent no-op — it parses,
exits zero and builds nothing. So they are written as the verb calls themselves, in
`side-tables.sh`:

- Twenty-one `add-edge-cost` calls in map minutes, keyed by the direction rather than by
  the road, which is what lets the hill cost more upward than down: the causeway is twelve
  minutes down from the Crown and eighteen back up, the climb from the Tidegate is
  twenty-two, the wade out of the cistern is twenty-five, and a boat across the market is
  four.
- Two `add-edge-guard` calls on `f-edge-ferry-pump`, the crossing to the pump-house wharf.
  With no threshold set the set is ANDed, so that way is shut until both conditions hold:
  the tide-gate lowered, and the crank in Kesh's keeping. The way back off the wharf is
  unguarded. Mnemosyne records the declaration and never evaluates it; the two conditions
  are real facts, and the two outcomes of evaluating them are the two world-lines above.

## What has not been checked

Nothing here has been run. No import, no `validate-continuity`, no
`report-transition-map`, no `propose-verdict` — I was permitted no commands, so every claim
in this report is read off the files by hand and none of it has been confirmed by the tool.
