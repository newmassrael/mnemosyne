# Self-report — the flooded hill-town

## What I wrote

Five files, in the order the contract says to author them:

- `sections.json` — the twenty scenes, as the bare JSON array the sections wire wants (not an object with a `"sections"` key). Every scene names `parent_doc: "flood-book"`, which the wire requires and does not default.
- `order.json` — the canon order: nineteen edges laying the twenty scenes out in one straight line, with an empty `branches` object. There is one world-line, `main`, and no fork or confluence anywhere in this world.
- `facts.json` — the fact manifest: three frames, six entity kinds, two units, twenty entities, ten predicates, sixty-seven facts, and two disclosure plans.
- `rules.json` — the narrative rules, so the continuity gate actually checks the map instead of merely holding it.
- `mnemosyne.toml` — I added two lines to the file that was already there: `rules_path = "rules.json"`, because the contract says authoring the rules file is not the opt-in, wiring it is; and `interval_severity = "reject"`, because I declared an interval rule and I mean it to gate rather than be reported and shrugged at.

## How many locations the town has

Thirteen.

Three of them are quarters, which are the larger things other places sit inside: Crownhead, the dry quarter on the top of the hill; the Drowned Market, the middle quarter the water took first; and the Waterline, the lowest, standing in the flood.

Nine are sites inside a quarter. Crownhead holds the Shrine of the Still Water, the Bell House, and Vessner's house. The Drowned Market holds the Market Under Awnings, the Salt Granary, and the Lamp Cellar. The Waterline holds the Stair Down to the Water, the Broken Quay, and the Pump House.

The thirteenth is the Causeway, which sits in no quarter. It is the one road that joins them, so it stands at the same level as the quarters themselves rather than inside any of them.

The containment is authored as nine `contains` facts in `facts.json`, and they are what makes the map have scopes at all: a root scope holding the three quarters and the Causeway, and one scope inside each quarter.

## How many ways lead between them

Ten. They are the ten `adjacent` facts in `facts.json` (the ones with ids beginning `f-e-`). Because `rules.json` declares the `town-map` transition rule `undirected: true`, one fact is a two-way road, so ten facts are twenty steps a person could take.

Three of the ten are in the root scope, joining the quarters to each other by way of the Causeway: Crownhead to the Causeway, the Causeway to the Drowned Market, the Drowned Market down to the Waterline. There is no way from Crownhead straight to the Waterline; to get from the top of the town to the bottom you must cross the middle. The other seven are inside the quarters: three joining the Bell House, the shrine yard and Vessner's house into a triangle in Crownhead; two joining the market to the granary and the granary down through its floor-hatch to the Lamp Cellar; two joining the stair to the quay and the quay to the pump house.

Every edge joins two places with the same direct container, which is what the contract requires — no house in one quarter is wired straight to a house in the next. Where a character crosses between a quarter and something inside it, I wrote no edge at all, because the contract says a step between a container and a place inside it is a crossing and needs none. Orin walking from Vessner's house out on to the Causeway is licensed by the Crownhead-to-Causeway edge, lifted to the pair of places that are actually siblings.

I also gave the `adjacent` predicate both leg kinds (`subject_kind: "place"`, `object_entity_kind: "place"`), because the contract is explicit that without them the store cannot be asked which entities are places and the completeness check quietly evaluates nothing. All thirteen locations carry at least one edge, so none of them is a place that exists only in the prose.

Two things I could not write from files: this world has no travel times on its edges and no condition guarding any passage, because edge costs and edge guards are reachable only through their own verbs and a manifest array named after them is a silent no-op. That is a limit of a file-only authoring run, not a decision about the town.

## Which locations a reader cannot place a character at, until which scene

The concealment is entirely in the disclosure plan named `withheld-place`, in the `disclosure_plans` array at the end of **`facts.json`**. That file, and only that file, says what the reader is told and when. Nothing about what is *true* changes between the two readings — the sixty-seven facts are the same facts.

**Sella.** The reader can place her at the shrine in sc-02, on the Causeway in sc-04, and under the awnings of the market in sc-05. Those three position facts are `state`, pinned at their own scenes. After sc-05 the reader is told nothing further about where she is. In the world she moves into the Salt Granary at sc-09 and down into the Lamp Cellar at sc-12, and both of those facts are `withhold` with their reveal pinned to sc-20. So:

- **the Salt Granary** — the reader cannot place Sella there, though she is there from sc-09, until **sc-20**;
- **the Lamp Cellar** — the reader cannot place Sella there, though she is there from sc-12, until **sc-20**.

Across sc-09 to sc-19 the reader has no correct position for her at all. The scenes still happen: she lights a lamp at sc-12, listens to the bell and counts its strokes at sc-13, writes the water's height in the ferry ledger at sc-15, and knocks twice on the boards above her at sc-19. All four of those are `state`, disclosed at their own scenes. Each one says what she is doing without saying where, because each is typed on a predicate whose object is a token for the doing and carries no place in it. That is what lets the middle of the story be told and stay unplaced.

The reader is not merely left blank, either. At sc-08 the story states, in the `town-talk` frame, that the town says she went down to the Broken Quay and did not come up; and at sc-09 it states, in Orin's frame, that Orin believes she went down the stair ahead of him. Both are disclosed. Neither is ground truth, and neither is written as a contradiction of it — they are separate facts in separate frames, which is how the contract says a belief and its ground-truth counterpart must be held. I did record the contradiction explicitly on one edge: the ground-truth fact placing her in the cellar carries a `conflicts_with` pointing at the town's quay claim.

**The Lamp Cellar as a place.** Before any character can be placed there, the reader has to know it is there. The fact that the Drowned Market contains it, the fact that a hatch in the granary floor lets down into it, and the fact that it is cut in rock and stays dry are all `withhold` with their reveal pinned to **sc-07**, the scene at the granary hatch. So for the first six scenes the reader does not know the town has thirteen locations; it has twelve as far as the reader can see. All three of those facts are true from sc-01 — the town is the town from the first page — and it is only the telling that holds them back. That is the axis the contract insists on: `canon_from` is when a thing becomes true, and the reveal is a separate pin. I did not reach for a forward evidence reference anywhere; every evidence ref in `facts.json` names a scene at or before its own fact's `canon_from`.

**Orin.** The other reading, in the same telling. All sixteen of his positions are `state`, each pinned at its own scene, beginning at sc-01 in the Bell House. He is placeable at every scene he appears in, without a gap: Bell House, shrine, Vessner's house, Causeway, market, granary, market, stair, quay, pump house, quay, stair, market, Causeway, granary, and at sc-20 down the hatch into the Lamp Cellar, where he finds her.

**The second reading.** The same facts also carry a plan named `plain-place`, whose default is `state` and which re-pins the five withheld facts to the scenes where they become true — the cellar's three map facts to sc-01, Sella in the granary to sc-09, Sella in the cellar to sc-12. Ask for that telling and the story has no secret in it and no reveal at sc-20; the reader follows her down through the hatch as it happens. Nothing in the fact base differs between the two. Only the telling asked for differs.

## What holds the world together

`rules.json` turns on four rules. One says a person is in at most one place at a time, and declares `contains` so that a coarse statement and a fine statement of one position read as a refinement rather than a contradiction. One says a thing has at most one holder — the ferry ledger passes from Vessner to Orin to Sella, three facts chained by succession, never two at once. One is the map itself: every move a character makes must be a step the ten edges allow, judged at the deepest scope where its two ends are comparable. The fourth says the water must begin to fall on a later day than it rose; both sides of it are quantities with the registered unit `day`, which the units array made authorable from a file at all.

I checked every step of every character against those edges by hand before writing them: Orin's fifteen moves, Sella's five, Vessner's one, Dessa's one. None of them crosses between quarters except by way of the Causeway or the one edge from the Drowned Market down to the Waterline. In the two belief frames I set an explicit `canon_to` on the earlier belief instead of chaining it by succession, because a belief frame carries no map of its own and a step judged against no edges could not be honest.

One deliberate gap: nothing in this store says how the town's belief travelled from the quay to the granary, or how Orin's did. Those pairs are unchained on purpose. The story does not say; it only shows them being replaced by the sight of her.
