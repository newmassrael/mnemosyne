# Corrieholm — self-report

## What I wrote, and where

- `sections.json` — the 21 scenes, as the sections manifest (bare JSON array, `import-sections`).
- `order.json` — the canon order: one straight road, `sc-01` through `sc-21`, 20 edges, no branches. Every scene is placed.
- `facts.json` — the fact manifest: frames, entity kinds, the `minute` unit, 18 entities, 5 predicates, 80 facts, and the two side tables (`edge_costs`, `edge_guards`) as arrays inside the same manifest.
- `rules.json` — the narrative-rules file, wired via `[continuity].rules_path` in `mnemosyne.toml`. Three rules: one place per person, movement must follow the town map, one holder per carried thing.
- `mnemosyne.toml` — I added the one line `rules_path = "rules.json"` under `[continuity]`; `canon_order_path` was already there. The rules gate is off until the file is wired, so the wiring is part of the authoring.

## How many locations the town has

Thirteen, all in the `entities` array of `facts.json`, all of entity kind `place`.

Five of them sit at the town level — the Shell Strand (`p-strand`), the Sea Gate (`p-sea-gate`), and the three terraces: Lower (`p-lower-terrace`), Market (`p-market-terrace`), Upper (`p-upper-terrace`).

Eight sit inside a terrace, and the eight `contains` facts in `facts.json` (ids `f-in-quay` through `f-in-alms`) say which:

- on the Lower Terrace: the Quay, the Net Loft, the Fish Shed
- on the Market Terrace: the North Row, the South Row, the Counting House
- on the Upper Terrace: the Bell Tower, the Almshouse

The terraces are containers *and* nodes people walk between — that is how you get from one shelf of the town to another. A step from a place on one terrace to a place on another is judged between the two terraces, which are the pair that are siblings, so the Cliff Stair and the Long Stair licence every such move.

## How many ways lead between them

Nine ways, written as eighteen one-way edges. Each edge is a `adjacent(a, b)` fact in the `facts` array of `facts.json`, ids `f-adj-*`. I declared the transition rule in `rules.json` with `"undirected": false`, so one fact means one direction and a two-way way is two facts. I did that deliberately, because the stair costs more going up than coming down and an undirected edge cannot say so.

The nine ways:

1. Lower Terrace ↔ Market Terrace — the Cliff Stair (`f-adj-lt-mt`, `f-adj-mt-lt`)
2. Market Terrace ↔ Upper Terrace — the Long Stair (`f-adj-mt-ut`, `f-adj-ut-mt`)
3. Lower Terrace ↔ Sea Gate — the wall walk (`f-adj-lt-sg`, `f-adj-sg-lt`)
4. Sea Gate ↔ Shell Strand — the arch through the wall (`f-adj-sg-st`, `f-adj-st-sg`)
5. Quay ↔ Net Loft — the ladder stair (`f-adj-quay-loft`, `f-adj-loft-quay`)
6. Quay ↔ Fish Shed — the slipway (`f-adj-quay-shed`, `f-adj-shed-quay`)
7. North Row ↔ South Row — the lane between the rows (`f-adj-nrow-srow`, `f-adj-srow-nrow`)
8. North Row ↔ Counting House — the one door (`f-adj-nrow-ch`, `f-adj-ch-nrow`)
9. Bell Tower ↔ Almshouse — the terrace path (`f-adj-tower-alms`, `f-adj-alms-tower`)

## Which ways cost more than others

The `edge_costs` array in `facts.json` — eighteen rows, one per one-way edge, each keyed by that edge's fact id, each a whole number of `minute` (the unit is declared in the `units` array of the same file). Nothing else in my files carries a travel cost; the prose in `f-stair-long` mentions the climb, but a router should read `edge_costs` and not that sentence.

Expensive, in order:

- **The Long Stair, going up** (Market Terrace → Upper Terrace, `f-adj-mt-ut`): **15 minutes**. This is the long climb and it is the most costly move in the town by more than double.
- The Long Stair, coming down (`f-adj-ut-mt`): 6 minutes — the same way, but less than half the price downhill. That asymmetry is the reason the edges are directed.
- The Cliff Stair: 7 minutes up (`f-adj-lt-mt`), 4 down (`f-adj-mt-lt`).
- The Shell Strand back up through the arch: 5 minutes (`f-adj-st-sg`) against 4 going out (`f-adj-sg-st`) — the shingle is harder inbound.
- The wall walk to the Sea Gate: 3 each way. The slipway to the Fish Shed: 3 each way. The ladder to the Net Loft: 2 each way. The terrace path between Bell Tower and Almshouse: 2 each way.

Cheap:

- **The lane between the two market rows** (`f-adj-nrow-srow`, `f-adj-srow-nrow`): **1 minute** each way. A few steps, and the cheapest move on the map alongside the counting-house door.
- The counting-house door (`f-adj-nrow-ch`, `f-adj-ch-nrow`): 1 minute each way — but see below, it is not always open.

So a messenger comparing routes reads it straight off: getting from the market up to the tower costs fifteen minutes and is worth planning around; crossing between the two market rows costs one and is not.

I never sum these anywhere, and no file of mine claims a total journey time. The numbers are carried, not added.

## Which ways are shut, and what opens them

The `edge_guards` array in `facts.json` — three rows. Each row names the edge fact it shuts and the set of condition facts that must hold for it to be passable. Nothing evaluates them here; they are declarations for whoever is walking the world.

**The Sea Gate, both directions.** `f-adj-sg-st` (Sea Gate → Shell Strand) and `f-adj-st-sg` (Shell Strand → Sea Gate) each carry the single condition `f-tide-low`. That fact is "The water stands low at the Sea Gate and Oris draws the gate up", and it becomes true at `sc-20`. Before that, the water at the gate is `high` (`f-tide-high`, from `sc-03`) and then `turning` (`f-tide-turning`, from `sc-12`, superseding the first); the three are one chain of in-frame succession on the predicate `water_state`, so the state of the water at any point in the story is answerable. I guarded both directions on purpose: a tidal arch traps you outside as surely as it keeps you out, so the Shell Strand is only reachable, and only leaveable, at low water. That means the Shell Strand is cut off from the rest of the town whenever `f-tide-low` does not hold — it has no other way in.

**The counting-house door, inbound only.** `f-adj-nrow-ch` (North Row → Counting House) carries the single condition `f-maren-key`, which is "Maren Vask carries the counting-house key on her belt, and no other key to that door exists" — a `holds(e-maren, e-key)` fact from `sc-04`. Maren is the one character who carries it. The way out, `f-adj-ch-nrow`, is deliberately **not** guarded: the door locks against the street, not against the room, so anyone inside can leave. Someone planning a route who is not Maren and is not with her should read the Counting House as unreachable.

Each guard is a set with no threshold, which means every condition in it is required. I used one condition per guard because each shut way here has exactly one thing that opens it.

## The scenes

Twenty-one, `sc-01` to `sc-21`, titled in `sections.json` and sequenced in `order.json`. Every one of the thirteen locations is walked through except the three terraces, which are crossed rather than dwelt in; ten of them host at least one scene outright. Three people move: Jena Corrow the messenger (fourteen positions), Maren Vask the clerk (seven), Oris Bell the tide-caller (seven). Every position after a character's first supersedes the one before it, so each person's route is a chain the map can be checked against rather than a scatter of sightings.

Three setups are marked `expected` and all three are paid off on the one world-line: the sealed writ (`f-writ-sealed` → `f-writ-entered`), the short tally (`f-oris-tally` → `f-tally-short`), and Oris's promise of the ninth hour (`f-gate-hour` → `f-tide-low`).

Two facts sit in a second frame, `frame-jena` — what the messenger wrongly takes to be so about the counting house and about her own margin of time. They are held apart from ground truth rather than contradicting it.
