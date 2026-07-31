# Ossary Rise — self-report

## How many locations the town has

Thirteen. They are the Shrine of Still Water, the Reeve's Hall, the Almshouse and
the Old Lookout on the dry top of the hill; the Bell House, the Wet Market, the
Marl House and the Cistern Yard on the half-drowned middle slope; and the Quay
Stair, Drowned Lane, the Sluice Arch, the Nets Yard and the Undertown below the
waterline.

There are also three named quarters — the Crown, the Market Shelf and Tidefoot —
and every one of the thirteen locations sits inside exactly one of them. The
quarters are containers, not locations: nobody walks from a quarter to a quarter,
and no quarter appears at either end of a way. They are searchable places to be
in rather than positions to move between, which is why one fact can say Sabb is
"up in the Crown" while another says he is at the Reeve's Hall, and the two are
one place stated coarsely and finely rather than a man in two places at once.

## How many ways lead between them

Twenty-nine, counted as the engine counts them: twenty-nine `adjacent` facts,
each one a single step from one location to one other location in one direction.
As journeys a townsperson would name, that is fifteen ways — fourteen that can
be walked both ways, written as a pair of facts each, and one that cannot.

The one that cannot is the Cistern Slip, the cistern's broken overflow channel:
it drops out of the Cistern Yard into Drowned Lane, and the wall below it is
sheer and slimed. There is a fact for going down it and no fact for coming back
up, so the engine will refuse anyone who tries to climb it. Getting back up to
the Cistern Yard from the lane means the whole round by the Quay Stair and the
Wet Market instead, which is half a day; that is one of six named passages that
carry how long they take, alongside the hour's climb of the Reeve's Steps, the
hour's wade up Drowned Lane against the flood, and three short ones.

One way is shut until something is true. The arch through the mill sluice is the
only entrance to the Nets Yard, and while the sluice stands shut and seized the
flood stands to the keystone and nobody passes. Both outcomes are written: on the
main world-line the crank will not turn, the town gives Hesk up for drowned, and
his name goes into the lost tally; on the `sluice-open` world-line Fen's weight
brings the sluice up a foot and she finds him alive on the net-loft roof. The
same setup is paid off differently in each world, so neither world is left with a
promise dangling.

## Which locations cannot be reached from the start

One: the Undertown, the flooded cellars under Drowned Lane. Everybody in the town
talks about it — they say a bell still tolls down there when the water turns, and
that the ways into it are silted shut — and nobody in any scene ever goes in. It
is registered, and Tidefoot contains it, but no `adjacent` fact names it at
either end, so there is no step into it from anywhere and no step out. The story
gets as close as the gratings knocking under Fen's feet in the lane.

Every other location can be reached on foot from where the story starts, the Bell
House. Two qualifications on that, both deliberate:

- The Nets Yard is reachable on the map only through the guarded arch, and only
  the `sluice-open` world-line actually travels there. On the main world-line it
  stays a place the story looks at across the water and never enters. So it is
  reachable-in-principle and unreached-in-fact, which is a different thing from
  the Undertown.
- The three quarters cannot be "reached" at all in the walking sense, by design:
  containers are not walked on.

## Which of my files says so

`facts.json` says all of it. The map is not a separate map file — the locations
and quarters are registered entities in it, each way between two locations is an
`adjacent` fact, and each quarter's membership is a `contains` fact. So the
count of locations is the count of place entities there, the count of ways is the
count of `adjacent` facts there, and the Undertown's unreachability is the
absence of any `adjacent` fact naming it. Travel durations are `crossing_time`
facts on named passage entities in the same file, and the shut way is a
`blocked_by` fact whose object is the fact that the sluice stands shut.

`order.json` says which scenes each world-line travels: the main trunk from
`sc-01` to `sc-18`, and the `sluice-open` segment that leaves the trunk at
`sc-12` and ends at `sc-b4`. That is what makes the Nets Yard reached on one
world-line and not the other, and what gives the two worlds different endings.

`sections.json` registers the twenty-two scenes, eighteen on the trunk and four
on the branch, and `narrative-rules.json` is what turns the checking on: one
place per person (refinement-aware through `contains`, so the coarse and fine
statements of Sabb's position agree), one holder per thing, and movement that has
to follow the map. The movement rule is declared directed on purpose — that is
what makes the missing return fact on the Cistern Slip mean something.

## What I could not put in a file

Three things the contract describes but does not give a file for, recorded here
rather than quietly dropped:

- Travel time properly belongs in the `edge_costs` side-table, keyed by the
  adjacency fact, as a number plus a registered unit. That table is written by
  `add-edge-cost`, and a unit has to be registered by `add-unit` first; neither
  the fact manifest nor the order file nor the rules file has an array for
  either, so a `quantity` object is not authorable from files at all. I wrote
  durations as a closed token vocabulary — `short`, `long`, `half-a-day` — on
  named passage entities instead. The prose claims carry the real figures.
- The shut way properly belongs in the `edge_guards` side-table, keyed by the
  edge fact, listing the condition facts the edge requires. That is
  `add-edge-guard`, also with no file. What I could author is the half the
  contract says is the author's anyway: the condition as a real fact, the two
  world-lines for got-it and didn't, and a `blocked_by` reference from the barred
  place to the condition that bars it. A consumer reading only
  `report-transition-map` would see the edge with no guard on it.
- The contract names `add-section` for the sections registry but never gives the
  file shape for it, so `sections.json` follows the manifest's own naming
  convention — an object with a `sections` array of `{ "section_id", "title" }`.
  If the loader wants something else, that is a guess and not a reading.

The counts above are hand-counted from `facts.json`; I was given no command to
count them with, and nothing here has been through the validator.
