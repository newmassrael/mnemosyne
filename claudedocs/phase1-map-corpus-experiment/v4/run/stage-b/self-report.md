# Cairnwell — self-report

## What is in the town

Cairnwell has twelve places. Ten of them are ordinary locations: the market
square, the shrine of the ford, the cairn stair, the boat landing, the
almshouse, the cooper's yard, the rope-walk, the eel quay, the sunken mill,
and the drowned bellhouse. The other two are the quarters — the High Water
quarter and the Drowned quarter — and in this store a quarter is a place like
any other, not a label: it is a registered entity of kind `quarter`, which is
declared a child of kind `place`, and it stands on the map as a node in its
own right. That is what lets a body leave it. The High Water quarter contains
the almshouse, the cooper's yard and the rope-walk; the Drowned quarter
contains the eel quay, the sunken mill and the drowned bellhouse. The other
four places sit in no quarter.

All of this is in `facts.json`: twelve entries in the `entities` array, and
six `contains` facts (`f-in-almshouse` through `f-in-drowned-bell`) that make
the containment tree.

## How many ways lead between them

Twenty. Each way is one fact — a fact whose typed leg reads
`adjacent(from, to)` — and in this store one such fact means one direction,
because the map rule in `rules.json` sets `"undirected": false`. So a road
you can walk both ways is two facts, and a way you can take only one way is
one fact.

The twenty facts are nine two-way roads (eighteen facts) and two one-way ways:

- `f-adj-stair-landing` — the cairn stair down to the boat landing. The
  lower flight has torn off the rock, so it takes a body down and gives none
  back up. There is no `f-adj-landing-stair`, and that absence is the whole
  statement.
- `f-adj-drowned-highwater` — the roof-plank ladder up out of the Drowned
  quarter into the High Water quarter. It can be climbed and not descended;
  the foot of it hangs over open channel.

Because the map is directed, the two halves of a road can also cost
differently, and some do: three minutes down from the market square to the
head of the stair and five back up, eleven minutes out across the channel on
the ebb and fourteen back against it, twenty for the plank ladder.

Those travel times are not in any of the JSON files, and neither is the shut
way. The contract is explicit that the keyed side tables — `edge_costs` and
`edge_guards` — have no file wire at all and are reached only through their
own verbs, and that writing them into a facts manifest is a silent no-op that
exits zero and builds nothing. So they are written as the calls they have to
be, in `side-tables.sh`, to run after the manifests are imported. The shut way
is one `add-edge-guard` call: the punt crossing out of the boat landing
(`f-adj-landing-drowned`) requires the condition fact `f-punt-unchained`,
which is the moment in scene sc-08 when Ordel beats the padlock off. Mnemosyne
holds that link and checks that both ends resolve; whether it holds at any
given moment is the consumer's to evaluate.

## What the engine reads instead of the prose

`rules.json` is what turns the facts into a map the engine can query without
reading a word of the claims. It declares one rule of class `transition`
(`cairnwell-map`) whose `adjacency` leg names the `adjacent` predicate and
whose `containment` leg names the `contains` predicate. To ask what can be
reached from a place, take the `adjacent` facts whose typed subject is that
place; those are its exits, and the containment tree says which of them are in
scope with it. The rule also makes the gate check every move in the story
against that map, so a scene that walked somebody along a way that does not
exist would be rejected rather than believed.

The adjacency predicate declares `subject_kind` and `object_entity_kind` as
`place` on both legs. That is deliberate: without a leg kind the store cannot
be asked what counts as a place, and the completeness check would have nothing
to evaluate.

Two more rules ride the same facts: `one-place-per-person` (a character is in
at most one place at a time, refinement-aware through `contains`, so being in
the almshouse and being in the High Water quarter are not a contradiction) and
`one-holder-per-thing` (the weir key has one holder, and it changes hands from
Crake to Mirren by succession, not by two facts co-holding).

## What cannot be reached from the start

The story starts at the market square, in scene sc-01. From there, nothing is
cut off: all eleven other places can be reached. The route into the low town
runs market square, cairn stair, boat landing, punt crossing into the Drowned
quarter, and once in the Drowned quarter the eel quay, the sunken mill and the
drowned bellhouse are all in reach; the way back up is the roof-plank ladder
into the High Water quarter and the lane down into the market square. The one
thing you cannot do is go back the way you came: the boat landing has no way
up the cairn stair, so anyone who takes the descent is committed to the whole
loop. That is the point of scene sc-14.

Three qualifications, because "reachable" is not one question:

1. **The drowned bellhouse is reachable and is never reached.** The whole town
   talks about it — the townsfolk hold that its bell still rings itself at
   slack water (`f-rumour-bell`) — and nobody in the twenty scenes sets foot
   there. That is a property of the telling, not of the map: the bellhouse has
   two `adjacent` facts joining it to the eel quay, so it is on the map; it
   simply never appears as anybody's location, and no scene is set there. The
   nearest the story comes is scene sc-17, where a knocking carries over the
   channel from it and Ordel names it the weir chain. Leaving the bellhouse
   off the map instead — no edges at all — would have been the reading the
   phrase invites and would have made it an invented place rather than an
   unvisited one.

2. **If the guard never opens, four places are unreachable.** The punt
   crossing is the only way into the Drowned quarter, since the roof-plank
   ladder runs upward only. While the punt stays chained, the Drowned quarter
   and its eel quay, sunken mill and drowned bellhouse cannot be entered at
   all, and the boat landing becomes somewhere you can arrive and never leave.
   Nothing in the store computes that; the store holds the guard, and the
   consumer evaluates it.

3. **No place is off the map.** Every one of the twelve has at least one
   `adjacent` fact, and every contained place is a node inside its quarter's
   scope, so there is no place that exists in name only.

## Which file says what

- `sections.json` — the twenty scenes, as structure sections of the document
  `doc-cairnwell`. Written first, because a fact naming an unregistered
  section is rejected.
- `order.json` — the canon order, sc-01 through sc-20 in a straight line. This
  is the only place a scene is actually placed on a road, and therefore the
  file that says which places are visited and in what sequence.
- `facts.json` — the registries and the seventy-five facts. The map lives here:
  twenty `adjacent` facts, six `contains` facts. So do the twenty-three `at`
  facts that walk five people through the town, chained by
  `supersedes_in_frame` so that each move is a step the map rule can judge, and
  the rest of what happens at each place.
- `rules.json` — the three narrative rules, wired through
  `[continuity].rules_path` in `mnemosyne.toml`. Authoring this file is the
  opt-in; without it the gate is off and the adjacency facts are inert data.
- `side-tables.sh` — the twenty edge costs and the one edge guard, which no
  manifest can carry. Not run by this authoring pass.
- `mnemosyne.toml` — amended with `rules_path = "rules.json"` alongside the
  canon order path that was already there.

## Two things the store deliberately does not say

The world is one world-line; `main` is the only branch, and no fork is
declared. The guard is a declaration about a passage, not a pair of authored
outcomes.

Facts about belief are sparse on purpose. Mirren holds two facts in her own
frame, the second superseding the first when Crake gives up the key sooner
than she thought he would; the town holds the bell rumour; and the ground
truth about the bellhouse clapper is a separate fact carrying a recorded
`conflicts_with` edge to that rumour. None of the three is a correction of
another. They are three claims held in three frames, and the store keeps them
all.
