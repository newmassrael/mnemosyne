# Hillwater — self-report

## What the town is

The town has **thirteen named locations**. Six of them stand at the top level:
the Ridge Road, the Shrine of Still Water, the Water Stair, the Skiff Mooring,
and the two quarters themselves — the Crown Quarter and the Drowned Quarter.
The other seven sit inside one of those two quarters: the Rain Market, the
Weighhouse, the Toll House and the Old Cistern are inside the Crown Quarter;
the Net Loft, the Drowned Chapel and the Boat Shed are inside the Drowned
Quarter. The quarters are locations in their own right, not labels: each is a
node on the map at the top level, and that is how a person gets out of one —
by the quarter's own way to its neighbours, never by a way running from a
house in one quarter to a house in the next.

**Thirteen ways lead between them.** Each way is written once and can be walked
in both directions, because the transition rule in `rules.json` declares
`undirected: true`; so thirteen facts are twenty-six moves. Seven of the ways
are at the top level (Crown Quarter–Ridge Road, Ridge Road–Drowned Quarter,
Ridge Road–Shrine, Crown Quarter–Shrine, Crown Quarter–Drowned Quarter,
Drowned Quarter–Water Stair, Water Stair–Skiff Mooring), three are inside the
Crown Quarter (Market–Weighhouse, Market–Toll House, Market–Cistern), and three
are inside the Drowned Quarter (Net Loft–Chapel, Net Loft–Boat Shed,
Chapel–Boat Shed). Every location has at least one way to it, and every scope
hangs together — you can get from any place to any other place in its scope.

There are twenty-one scenes and seventy-three facts. Eleven of the thirteen
locations host a scene of their own; the two that do not are the quarters,
which are containers and are passed through rather than stopped in.

## Which file answers which question

**Where can one go from here?** `facts.json`, in the thirteen facts whose typed
predicate is `adjacent` — the map is facts, not a file list. `rules.json` names
that predicate as the map's edge source and names `contains` as the predicate
that partitions those edges into scopes, and `mnemosyne.toml` wires the rule
file through `[continuity].rules_path`. So for a scene, the engine takes the
character's `at` fact holding there, reads that place's `adjacent` facts, and —
because the quarters are declared containers — knows that a step out of a
quarter is judged between that quarter and the place outside it. No prose is
read.

**Where has the reader been told anyone is by this point?** `facts.json`, in
`disclosure_plans`, joined against `order.json`. Every fact that places a
person carries an override in the telling `told` with a `first_at` pin on the
`main` road, and `order.json` is what makes "by this point" mean anything: a
placement is known to the reader at scene S if its pin falls at or before S in
that order, and it is still current if no successor has taken over by S.

## Which locations a reader cannot place a character at, until which scene

**The Old Cistern, for Yrsa, from scene sc-09 until scene sc-20.**

The fact is `at-yrsa-05` in `facts.json`: Yrsa is in the Old Cistern, true from
sc-09, evidenced by the three earlier scenes that put her there (sc-04, where
she sounds the cistern; sc-07, where she takes a weight and a lamp; sc-08,
where she goes before light). It is the only fact in the store whose telling
mode is `withhold`, and its `first_at` is pinned to sc-20. That single override
— in `facts.json`, `disclosure_plans` → telling `told` → override on
`at-yrsa-05` — is what says so.

The consequence, which is the reading I wanted: **from sc-09 through sc-19,
eleven scenes, the reader has no told position for Yrsa anywhere.** Her last
disclosed position is the Boat Shed (`at-yrsa-04`, told at sc-08), and that
fact's extent ends where its successor begins at sc-09 — so a reader knows she
has left the Boat Shed and is told nothing about where she went. She acts in
sc-14, sc-17 and sc-19; those facts are told, and not one of them names a
place, nor lists one among its entities. At sc-20 the pin fires and the whole
middle becomes placeable at once, retroactively, from sc-09 onward.

What the reader IS told in the meantime is what the hill believes, which is
wrong: `tt-yrsa-01` (she went down the Water Stair, told at sc-11) and
`tt-yrsa-02` (she took a boat from the Skiff Mooring out to the sunk mill, told
at sc-15). Both are in the `town-talk` frame, not in `ground-truth`, so they
are machine-readable as *belief* and never as position; both carry a recorded
`conflicts_with` edge to `at-yrsa-05`. Corin's own frame holds the one word
against it: `cond-corin-01`, that she is alive.

**No other location is unplaceable for anyone.** Every other placement in the
store — Corin's thirteen, Yrsa's four early and two late, Brida's three,
Hallam's one, Orla's four — is pinned to the scene it becomes true in, so the
plainest reading in the book is Corin's: his position is told at sc-01 and told
again at every move he makes, and a reader can name where he is at any of the
twenty-one scenes.

## Both readings, one world

Nothing about the concealment lives in what is true. The store holds one
`ground-truth` account in which Yrsa is in the cistern from sc-09, and the
concealment is entirely in the disclosure layer. To prove that, `facts.json`
carries a second telling, `told-open`, over the identical fact base: it tells
everything outright and pins `at-yrsa-05` to sc-09, the scene she goes there.
Reading the world through `told` a reader cannot place her until sc-20; reading
it through `told-open` a reader watches her climb down. Not one fact, entity,
way or scene differs between them.

The map facts themselves carry no disclosure decision, so they ride the
`withhold` default in `told` — the reader is walked through the town rather
than handed it, and reconstructs it. The engine reads them regardless, since
adjacency is a store question and not a telling question.

## One thing the files cannot say

The Water Stair ought to be shut at high water, and the contract has a place
for that — an edge guard, or a travel cost on a way. Both are side tables
reached only through their own verbs, keyed by an existing fact; there is no
file wire for them, and a manifest key naming one is a silent no-op. So no
guard or cost is authored here. The way down is on the map unconditionally, and
the flood's height is carried instead as a plain fact that rises from eleven
steps (sc-01) to thirteen (sc-11).
