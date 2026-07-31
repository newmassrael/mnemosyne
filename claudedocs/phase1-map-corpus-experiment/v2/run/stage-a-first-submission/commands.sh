#!/usr/bin/env bash
#
# Harrow Steep - the half of the world that has no file wire.
#
# The keyed side tables (edge_costs, edge_guards) are verb-only: writing an
# "edge_costs" or "edge_guards" array into facts.json is a silent no-op, because
# the manifest ignores unknown keys. So the travel times and the shut gate are
# authored here, keyed by the adjacency fact ids in facts.json.
#
# Prerequisites, in this order (file-authored, not part of this script):
#   mnemosyne-cli import-sections --manifest sections.json
#   mnemosyne-cli import-facts    --manifest facts.json
#   # then wire the canon order and the rules, either in mnemosyne.toml
#   #   [continuity] canon_order_path = "order.json"
#   #   [continuity] rules_path       = "rules.json"
#   # or pass them per call: --order order.json --rules rules.json
#
# The unit `minute` is registered by the units array of facts.json.

set -euo pipefail

# --- edge costs: how long each way takes, one direction at a time ---------
# On the crown, everything is a short walk.
mnemosyne-cli add-edge-cost --fact fe-crown-bell --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact fe-bell-crown --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact fe-crown-keeper-house --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact fe-keeper-house-crown --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact fe-bell-keeper-house --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact fe-keeper-house-bell --n 2 --unit minute

# The stair head is downhill from the crown, so the climb back costs more.
mnemosyne-cli add-edge-cost --fact fe-crown-stair-head --n 4 --unit minute
mnemosyne-cli add-edge-cost --fact fe-stair-head-crown --n 6 --unit minute

# The broken descent. There is no cost for the reverse because there is no
# reverse edge: the Forty Steps go down only.
mnemosyne-cli add-edge-cost --fact fe-stair-head-waterline --n 6 --unit minute

# The shore.
mnemosyne-cli add-edge-cost --fact fe-waterline-ferry-stone --n 5 --unit minute
mnemosyne-cli add-edge-cost --fact fe-ferry-stone-waterline --n 5 --unit minute
mnemosyne-cli add-edge-cost --fact fe-ferry-stone-cistern-gate --n 7 --unit minute
mnemosyne-cli add-edge-cost --fact fe-cistern-gate-ferry-stone --n 7 --unit minute

# The crossing is the long journey of the story; the return is longer still,
# loaded and against the draw of the emptying cistern.
mnemosyne-cli add-edge-cost --fact fe-ferry-stone-quarter --n 25 --unit minute
mnemosyne-cli add-edge-cost --fact fe-quarter-ferry-stone --n 30 --unit minute

# The cistern stair: a long climb up, a quicker way down.
mnemosyne-cli add-edge-cost --fact fe-cistern-gate-keeper-house --n 18 --unit minute
mnemosyne-cli add-edge-cost --fact fe-keeper-house-cistern-gate --n 12 --unit minute

# Inside the Drowned Quarter, everything is worked by plank walk or boat.
mnemosyne-cli add-edge-cost --fact fe-market-yard --n 4 --unit minute
mnemosyne-cli add-edge-cost --fact fe-yard-market --n 4 --unit minute
mnemosyne-cli add-edge-cost --fact fe-yard-almshouse --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact fe-almshouse-yard --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact fe-market-almshouse --n 6 --unit minute
mnemosyne-cli add-edge-cost --fact fe-almshouse-market --n 6 --unit minute

# The drowned stair to the Under-Church is on the map and is never travelled.
mnemosyne-cli add-edge-cost --fact fe-almshouse-under-church --n 9 --unit minute
mnemosyne-cli add-edge-cost --fact fe-under-church-almshouse --n 9 --unit minute

# --- edge guard: the way that is shut until something is true -------------
# The cistern stair is passable only when the sluice key is in the Keeper's
# hand AND the water stands at slack low water. Two conditions, one call each,
# on each direction of the passage. No threshold is set, so the set is ANDed:
# both conditions are required. Mnemosyne holds the declaration; the consumer
# evaluates it during a playthrough.
mnemosyne-cli add-edge-guard --fact fe-cistern-gate-keeper-house --condition f-cond-key-in-keepers-hand
mnemosyne-cli add-edge-guard --fact fe-cistern-gate-keeper-house --condition f-cond-slack-low-water
mnemosyne-cli add-edge-guard --fact fe-keeper-house-cistern-gate --condition f-cond-key-in-keepers-hand
mnemosyne-cli add-edge-guard --fact fe-keeper-house-cistern-gate --condition f-cond-slack-low-water
