#!/bin/sh
# Ondree — the side-table half of the authoring run.
#
# The contract's "side tables (verb-only)" section is explicit: edge_costs and
# edge_guards have NO file wire, and writing an "edge_costs" / "edge_guards"
# array into facts.json is a silent no-op (unknown manifest keys are ignored,
# exit 0, zero rows). So the two things the world needs that a manifest cannot
# carry — "some journeys take longer than others" and "one way is shut until
# something is true" — are these calls, and they run AFTER import-facts,
# because each is keyed by an existing fact id.
#
# Order of the whole run:
#   1. import-sections --manifest sections.json
#   2. import-facts    --manifest facts.json
#   3. this script
# (order.json and rules.json are wired in mnemosyne.toml under [continuity].)
#
# NOT RUN by the authoring session — no command was handed to it.

set -eu

# --- travel times: one cost per one-way edge, in registered unit `minute` ----
# The unit is declared in facts.json's `units` array (add-unit equivalent).
# Every n is POSITIVE; 0 would be a free teleport and is refused.

# root scope — the hill
mnemosyne-cli add-edge-cost --fact f-adj-crown-upper       --n 8  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-upper-crown       --n 12 --unit minute   # the climb costs more than the descent
mnemosyne-cli add-edge-cost --fact f-adj-upper-market      --n 6  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-market-upper      --n 9  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-market-stair      --n 4  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-stair-market      --n 5  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-stair-waterline   --n 3  --unit minute   # the one-way descent: quick, and only downward
mnemosyne-cli add-edge-cost --fact f-adj-market-waterline  --n 40 --unit minute   # the towpath, the long way round the hill
mnemosyne-cli add-edge-cost --fact f-adj-waterline-market  --n 48 --unit minute   # and longer coming back up it
mnemosyne-cli add-edge-cost --fact f-adj-waterline-isle    --n 20 --unit minute   # the channel crossing nobody makes
mnemosyne-cli add-edge-cost --fact f-adj-isle-waterline    --n 20 --unit minute

# inside the Upper Quarter
mnemosyne-cli add-edge-cost --fact f-adj-shrine-almshouse    --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-almshouse-shrine    --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-almshouse-cistern   --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-cistern-almshouse   --n 3 --unit minute

# inside the Market Quarter
mnemosyne-cli add-edge-cost --fact f-adj-fishmarket-ropewalk --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-ropewalk-fishmarket --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-ropewalk-ferry      --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-ferry-ropewalk      --n 4 --unit minute

# --- the shut way ------------------------------------------------------------
# The alley from the Market Quarter to the head of the Drowned Stair is barred.
# Two conditions, ANDed (no threshold set = require ALL, the canonical form):
#   f-key-sula   — the sluice key is in the warden's hand   (sc-10)
#   f-tide-slack — the water at the stair foot has gone slack (sc-11)
# Mnemosyne only holds the declaration and checks that the edge and both
# conditions resolve; the consumer evaluates them at play time.
mnemosyne-cli add-edge-guard --fact f-adj-market-stair --condition f-key-sula
mnemosyne-cli add-edge-guard --fact f-adj-market-stair --condition f-tide-slack

# No set-edge-guard-threshold call: threshold None is AND over the whole set,
# which is what "the key AND slack water" means.
