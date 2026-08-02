# Ashfell Post — self-report

## What I wrote, and where

`sections.json` holds the structure sections — the scenes. There are twenty-four
of them: a shared trunk of thirteen scenes (`sc-01` to `sc-13`), a main
continuation of seven more (`sc-14` to `sc-20`), and a four-scene branch road
(`sc-14b` to `sc-17b`) for the world-line in which the strongroom never opens.
It is a bare JSON array, as the sections wire requires, and every entry names
the document `ashfell-post`.

`order.json` holds the canon order. Its top-level `edges` are the main road,
`sc-01` through `sc-20` in a line. Under `branches` there is one entry,
`dry-seal`, whose road leaves the trunk at `sc-13` and runs `sc-13` →
`sc-14b` → `sc-15b` → `sc-16b` → `sc-17b`, overriding the inherited
`sc-13` → `sc-14` step. `main` continues as one of the two roads rather than
being left a bare pre-fork prefix.

`facts.json` is the fact manifest: frames, the one branch registration,
entity kinds, units, entities, predicates, the facts themselves, the edge
costs, the edge guards, and one disclosure plan.

`rules.json` holds the narrative rules, and `mnemosyne.toml` now wires it at
`[continuity].rules_path` (the file is inert until it is wired) and sets
`interval_severity = "reject"` so the one interval rule actually gates instead
of only being reported.

## How many locations, and how many ways between them

The post has **twelve** named locations inside its wall: the north gate, the
gate yard, the caravan yard, the stables, the granary, the weigh house, the
tally office, market row, the council hall, the strongroom, the water stair,
and the river quay. Each is an entity of kind `place` in `facts.json`.

There is a thirteenth place entity, `pl-post` (Ashfell Post itself), but it is
of kind `settlement`, not `place`. It is not a location in the post; it is the
container the twelve sit in. Every one of the twelve is named by a `contains`
fact (`f-in-north-gate` and its eleven fellows), which gives the map a single
scope so that every way joins two siblings.

**Fifteen** ways lead between them. Each way is one `adjacent` fact in
`facts.json`, with a fact id beginning `f-way-`. The transition rule in
`rules.json` is declared `"undirected": true`, so one fact is one two-way way;
fifteen facts are fifteen ways, not thirty. Each also carries a travel cost in
map minutes in the `edge_costs` array of `facts.json`, from one minute (the
clerk's door, the strongroom door) to five (the water stair).

The ways are: north gate–gate yard, gate yard–market row, gate yard–caravan
yard, gate yard–council hall, caravan yard–market row, caravan yard–stables,
caravan yard–granary, stables–granary, granary–weigh house, market row–weigh
house, market row–council hall, market row–water stair, weigh house–tally
office, council hall–strongroom, and water stair–river quay.

## Which ways are shut, and what opens each

Three of the fifteen are shut. Each shut way is the sole way to the place
behind it, so the guard on it is the whole of the question for a route
planner. All three are declared in the `edge_guards` array of `facts.json`,
keyed by the fact id of the way they gate, and each condition is a real fact
elsewhere in the same file.

**The strongroom door** (way `f-way-hall-strongroom`, council hall to
strongroom). Its guard lists three conditions — `f-term-seal-oak`,
`f-term-seal-iron`, `f-term-seal-salt` — and a `threshold` of **2**. Any two of
the three seals set in the council lock open it. One does not. This is
described further in its own section below.

**The tally office door** (way `f-way-weigh-tally`, weigh house to tally
office). Its guard lists one condition, `f-term-clerk-key`, and no threshold,
which means the whole set is required. That fact reads "the clerk's house key
is turned in the tally office door", typed `key_turned(it-clerk-key,
pl-tally-office)`. Turning the clerk's key is what opens it.

**The water stair** (way `f-way-stair-quay`, water stair to river quay). Its
guard lists one condition, `f-term-low-water`, and no threshold. That fact
reads "the river stands at low water on the water stair", typed
`water_state(pl-water-stair, low)` — and because `water_state` is a token
predicate with the closed vocabulary `["low", "high"]`, a planner can see from
the predicate declaration alone that the other possible state is `high`. Low
water is what opens it; the flood is what shuts it.

The five condition facts all live in a frame of their own, `house-terms`,
declared in `facts.json` as the standing terms each shut way opens on rather
than a record of any one day. That keeps them distinct from the `ground-truth`
facts that record what actually happened on a given road — for instance
`f-key-turned-today`, where Ansel really does turn the key at `sc-07`, and
`f-low-water`, where the ebb really does come at `sc-14`. Mnemosyne holds the
declaration and checks that every condition resolves; whether a condition holds
at play time is the consumer's to evaluate, so the terms frame is the honest
place for them.

## How I said the strongroom's two-of-three custom

Four ways, of which the first is the one a planner needs and the only one that
is machine-decisive.

**First, and load-bearing: the K-of-N threshold on the edge guard.** In the
`edge_guards` array of `facts.json`, the entry keyed by `f-way-hall-strongroom`
carries a `conditions` list of exactly three fact ids and `"threshold": 2`. A
planner reading only that array learns, with no prose at all, that the way into
the strongroom has three conditions and needs any two of them satisfied — and
therefore that one seal does not open it. The contract's default for a guard is
AND over the whole set, and `k == len` normalises back to AND, so a `threshold`
of 2 against a set of 3 is a genuine, non-degenerate two-of-three.

**Second: the three conditions are the three seals, each a typed fact.**
`f-term-seal-oak`, `f-term-seal-iron`, and `f-term-seal-salt` are each typed
`seal_set(<the seal>, it-council-lock)` — subject a `seal`-kind entity, object
the lock. Following the guard's three ids leads straight to three distinct
seals and one lock, so a planner learns not just "two of three conditions" but
"two of three seals in this lock". `f-lock-fitted` types
`fitted_to(it-council-lock, pl-strongroom)`, which is what joins the lock to
the place behind the door.

**Third: the count is also stated as a quantity.** `f-lock-seals-required` is
typed `seals_required(it-council-lock, {n: 2, unit: seal})` and
`f-lock-seals-cut` is typed `seals_cut(it-council-lock, {n: 3, unit: seal})`.
Two and three, in a registered unit, on the lock itself. This is redundant with
the threshold on purpose: it is the same custom said in the quantity slot,
where an arithmetic check can reach it.

**Fourth: a rule gates the redundancy.** `rules.json` declares an interval rule
`lock-opens-short-of-every-seal`: `seals_cut` minus `seals_required` must be at
least `{ "const": 1 }`, both operands resolved on the lock and both in the unit
`seal`. That is the machine statement of *why* the custom exists — more seals
were cut than the lock demands, so an absent warden cannot shut the post out of
its own strongroom. Three minus two is one, so it holds; and because
`mnemosyne.toml` sets `interval_severity = "reject"`, it is a rule that would
actually fail rather than merely be reported if a later hand made the lock
require all three.

The custom is also said in prose, in the claims of `f-lock-seals-required`,
`f-custom-spoken` and `f-mira-learns`, and it is dramatised: Brannoc is away
downriver with the iron seal for the whole story (`f-brannoc-away`), which is
why the third condition is never satisfiable on either road, and the whole
question of the day becomes whether Imre can climb the water stair to set the
second seal. On `main` the ebb comes, two seals go in, and the door opens
(`f-strongroom-open`). On the branch `dry-seal` the water holds high, Imre
stays on the quay, only the oak seal is set, and the door stays shut
(`f-strongroom-stays-shut`) — the two outcomes of the guard authored as two
world-lines, which is what the contract asks an author to do, since Mnemosyne
declares the guard and never evaluates it.

## Which file says each thing

- The twelve locations, their kinds, and their containment in the post:
  `facts.json` (the `entities` array; the `contains` facts `f-in-*`).
- The fifteen ways between them: `facts.json` (the `adjacent` facts `f-way-*`).
- That those facts *are* the map, and that one fact is a two-way way:
  `rules.json` (the transition rule `movement-follows-the-ways`, with
  `adjacency: adjacent`, `undirected: true`, `containment: contains`).
- Which ways are shut and what opens each: `facts.json`, the `edge_guards`
  array; the conditions themselves are facts in the same file in the frame
  `house-terms`.
- That the strongroom takes any two of three seals: `facts.json`, the
  `edge_guards` entry for `f-way-hall-strongroom` (`threshold: 2` over three
  conditions), reinforced by the quantity facts `f-lock-seals-required` and
  `f-lock-seals-cut` in the same file and by the interval rule in `rules.json`.
- How long each way takes: `facts.json`, the `edge_costs` array, in the unit
  `minute`.
- The scenes: `sections.json`. Their order, and the two roads through them:
  `order.json`.
- The rules the continuity gate enforces, and the wiring that turns them on:
  `rules.json` and `mnemosyne.toml`.

## Two things worth flagging

I was given no commands to run, so nothing here has been through
`validate-continuity`, `propose-verdict`, or any import. The files are written
against the contract as read; they have not been machine-checked.

The declaration that makes the map's completeness check answerable is on the
`adjacent` predicate in `facts.json`: it declares `subject_kind: "place"` and
`object_entity_kind: "place"`. Without those legs, the contract says the
`map_invented_place` class emits nothing at all and a run reads exactly like
one where every place was on the map. Every entity of kind `place` in this
store has at least one way to another, so the check has something true to
find rather than nothing to say.
