# Coldharrow — self-report

## The files

- `sections.json` — the 21 scenes, as the sections wire (a bare JSON array, `parent_doc` = `coldharrow`).
- `order.json` — the canon order: the main trunk runs `sc-01` through `sc-20`, and one branch, `shut-gate`, declares its own road segment `sc-19 → sc-21`.
- `facts.json` — the fact manifest: frames, the branch, entity kinds, the `minute` unit, 22 entities, 5 predicates, 84 facts, and the two keyed side tables `edge_costs` and `edge_guards`.
- `rules.json` — the narrative rules that turn the gate on for this store.
- `mnemosyne.toml` — wires `canon_order_path = "order.json"` and `rules_path = "rules.json"`.

## How many locations the town has

Thirteen. They are entities of kind `place`, declared in the `entities` array of `facts.json`:

- Three terraces, which are containers and also nodes of the map in their own right: `d-upper` (the Upper Terrace), `d-market` (the Market Terrace), `d-harbour` (the Harbour Terrace).
- Three sites on the Upper Terrace: `p-beacon-yard`, `p-cistern-court`, `p-chapel`.
- Three on the Market Terrace: `p-fish-row`, `p-cloth-row`, `p-weigh-house`.
- Three on the Harbour Terrace: `p-quay`, `p-counting-house`, `p-net-loft`.
- One place under no terrace at all: `p-mussel-rock`, the tidal islet outside the sea wall.

Which terrace holds which site is said by nine `contains` facts (`fc-*`) in `facts.json`. Those nine facts are also what partitions the map into scopes, so an edge only ever joins two places on the same terrace, or two terraces (plus Mussel Rock) at the root.

## How many ways lead between them

Twenty. Each way is one `adjacent` fact (`fe-*`) in the `facts` array of `facts.json`, and the map is **directed** — one fact means one direction — so the twenty facts are ten two-way ways:

Between terraces and the rock (root scope): the Long Stair `d-market → d-upper` / `d-upper → d-market`; the Windlass Ramp `d-harbour → d-market` / `d-market → d-harbour`; the sea-gate causeway `d-harbour → p-mussel-rock` / `p-mussel-rock → d-harbour`.

On the Upper Terrace: Beacon Yard ↔ Cistern Court, Cistern Court ↔ Chapel.

On the Market Terrace: Fish Row ↔ Cloth Row, Cloth Row ↔ Weigh House, Fish Row ↔ Weigh House.

On the Harbour Terrace: Quay ↔ Net Loft, Quay ↔ Counting House.

## Which ways cost more than others

Said by the `edge_costs` array of `facts.json`, one row per `adjacent` fact, each a positive number of `minute` (the unit is registered in the same file's `units` array). The engine reads the cost off the edge; the adding-up is the planner's. All twenty ways carry a cost, so a route through the map can be totalled without reading a word of prose.

Costs are not uniform, and because the map is directed the two directions of one way can differ:

- **The Long Stair is the expensive way in the town: 14 minutes up (`fe-stair-up`, Market Terrace to Upper Terrace) and only 6 minutes back down (`fe-stair-down`).** That asymmetry is the climb.
- The sea-gate causeway is the next longest: 12 minutes each way (`fe-gate-out`, `fe-gate-back`).
- The Windlass Ramp is 9 minutes up (`fe-ramp-up`) and 5 down (`fe-ramp-down`).
- **The lane between the two market rows is the cheapest way in the town: 1 minute either way (`fe-fish-cloth`, `fe-cloth-fish`).** Those are the few steps.
- The rest are short: Beacon Yard ↔ Cistern Court 4; Cistern Court ↔ Chapel 3; the other two market ways 2 each; and all three harbour ways 2 each.

So a messenger comparing "up the stair and back" (14 + 6 = 20) against "across the market lane and back" (1 + 1 = 2) gets that difference out of the table alone.

One honest limit: stepping from a site onto its own terrace, or off a terrace into a site on it, is a containment crossing rather than a way, so it has no row in `edge_costs` and adds nothing to a total. Only the twenty ways cost time.

## Which ways are shut, and what opens them

Said by the `edge_guards` array of `facts.json`. Each row names the `adjacent` fact it gates and the set of condition facts that must hold for it to be passable. There are three guarded ways out of twenty, and nothing in the guard rows is prose — a planner reads the fact ids and looks the conditions up in the same file.

- **The sea gate, both directions.** `fe-gate-out` (Harbour Terrace → Mussel Rock) and `fe-gate-back` (Mussel Rock → Harbour Terrace) each require `f-tide-out`. That condition fact is typed `e-tide` / `tide_state` / token `low` — "the tide stands at low water and the Mussel Rock causeway lies bare". What opens the gate is low water, and it shuts in both directions, so the rock strands whoever is on it.
- **The counting-house door, inbound only.** `fe-quay-counting` (Long Quay → Counting House) requires `f-key-verrick`, typed `e-key` / `held_by` / `e-verrick` — "Verrick Sowle carries the counting-house key on his belt". What opens the door is the key, and it is one character's. The way back out, `fe-counting-quay`, carries no guard: you need the key to get in, not to leave.

The two outcomes of the tide are in the store as world-lines, not as prose. `facts.json` registers the branch `shut-gate`, forked from `main` at `sc-19` — the scene at the sea gate — and `order.json` gives it its own road `sc-19 → sc-21`. On `main` the water is low and `f-ev-20` has Mella cross and relight the Mussel Rock lamp, paying off the setup `f-ev-17`. On `shut-gate` the fact `f-tide-high` supersedes `f-tide-out` in the same frame, so the guard's condition no longer holds there, the causeway edges are shut, and the lamp setup stays unpaid on that road. The engine never evaluates the guard; it holds the declaration and the two roads.

## What else the files say

`rules.json` turns the map into something the continuity gate checks. It declares three rules: `coldharrow-map`, a transition rule keyed on the `at` predicate with `adjacency: adjacent` and `containment: contains` and `undirected: false`, so every move a character makes must follow a way or be a crossing into or out of a terrace; `one-place-per-person`, an exclusive rule on `at` keyed per subject and refinement-aware through `contains`; and `one-holder-per-thing`, an exclusive rule on `held_by` keyed per object, which is what makes the letter's passage from Mella to Old Tomsen a succession rather than two holders at once.

`facts.json` also carries a second frame. `ground-truth` holds the town as it stands; `mella` holds what the messenger takes to be so, and there she believes at `sc-03` that nobody holds the counting-house key (`f-m-belief-1`) until `f-m-belief-2` supersedes it at `sc-04`. Her wrong belief and the ground truth are two facts, not one fact with two frames.

Movement itself is in `facts.json` as chains of `at` facts joined by `supersedes_in_frame`: twenty-one for Mella across the whole town, four for Verrick coming down from the Weigh House to the counting house, and one apiece for Ivane in the Net Loft, Serah in the Weigh House and Tomsen in the Beacon Yard. Every one of the thirteen places is the setting of at least one scene, and each scene carries at least one fact of what happened there.
