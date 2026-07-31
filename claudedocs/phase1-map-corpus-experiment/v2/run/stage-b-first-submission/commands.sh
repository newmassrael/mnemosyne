#!/usr/bin/env sh
#
# Cawl Hill — the parts of this world that no manifest can carry.
#
# The contract is explicit that the keyed side tables (edge_costs, edge_guards)
# have no file wire: writing an "edge_costs" array into the fact manifest parses
# cleanly, exits 0, and builds nothing. So the travel times and the shut gate
# are these verb calls, and they must run AFTER the facts exist, because each
# one is keyed by an existing fact id.
#
# Nothing here has been run. Run from this directory, in this order.

set -e

# --------------------------------------------------------------------------
# 1. Structure first: every fact names a section, so sections are created first.
# --------------------------------------------------------------------------
mnemosyne-cli import-sections --manifest sections.json

# --------------------------------------------------------------------------
# 2. Registries and facts, one atomic transaction: frames, branches,
#    entity_kinds, units, entities, predicates, facts.
# --------------------------------------------------------------------------
mnemosyne-cli import-facts --manifest facts.json

# --------------------------------------------------------------------------
# 3. Travel times. Keyed by the adjacency fact, in registered `minute` units.
#    The hill is steep, so a way costs more upward than downward, which is
#    only sayable because every way is its own directed fact.
# --------------------------------------------------------------------------

# Root scope — the quarters, the terrace, the stair, the water.
mnemosyne-cli add-edge-cost --fact f-way-crown-upper  --n 6  --unit minute
mnemosyne-cli add-edge-cost --fact f-way-upper-crown  --n 9  --unit minute
mnemosyne-cli add-edge-cost --fact f-way-crown-shrine --n 4  --unit minute
mnemosyne-cli add-edge-cost --fact f-way-shrine-crown --n 5  --unit minute
mnemosyne-cli add-edge-cost --fact f-way-upper-market --n 12 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-market-upper --n 17 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-shrine-market --n 10 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-market-shrine --n 14 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-market-stair --n 8  --unit minute
mnemosyne-cli add-edge-cost --fact f-way-stair-market --n 11 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-stair-water  --n 20 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-water-lantern --n 25 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-lantern-water --n 25 --unit minute

# Inside the Upper Quarter.
mnemosyne-cli add-edge-cost --fact f-way-almshouse-rope --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-rope-almshouse --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-rope-cistern   --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-cistern-rope   --n 3 --unit minute

# Inside the Market Quarter.
mnemosyne-cli add-edge-cost --fact f-way-weigh-salt   --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-salt-weigh   --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-salt-tanners --n 4 --unit minute
mnemosyne-cli add-edge-cost --fact f-way-tanners-salt --n 4 --unit minute

# --------------------------------------------------------------------------
# 4. The shut way. The gate at the foot of the Market Quarter opens on the
#    Pilgrim Stair only when Hesper has the second key AND the water stands
#    slack. Two conditions, no threshold, so the consumer ANDs them.
#
#    Only the downward way is guarded (f-way-market-stair). Coming back up
#    from the stair head into the market is free (f-way-stair-market), and
#    there is no way at all back up from the Water Line — that reverse fact
#    does not exist in facts.json, which is how the descent is made one-way.
# --------------------------------------------------------------------------
mnemosyne-cli add-edge-guard --fact f-way-market-stair --condition f-key-in-hand
mnemosyne-cli add-edge-guard --fact f-way-market-stair --condition f-tide-slack

# --------------------------------------------------------------------------
# 5. Wiring, for the gate to actually evaluate anything. The canon order and
#    the rules file are pinned in mnemosyne.toml under [continuity] --
#    canon_order_path = "order.json" and rules_path = "rules.json" -- or
#    passed per-run:
#
#      mnemosyne-cli validate-continuity --order order.json --rules rules.json
#
#    Left commented because the toml is outside this world's directory, and
#    because an unwired rules file means the gate is off, not passing.
# --------------------------------------------------------------------------
