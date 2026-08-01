#!/bin/sh
# Kelder flood — the half of the authoring that has NO file wire.
#
# The contract is explicit: the keyed side tables (edge_costs, edge_guards,
# parameters, fact_counts) are reached ONLY through their own verbs, and
# writing an "edge_costs" / "edge_guards" array into a facts manifest is a
# SILENT no-op (unknown manifest keys are ignored by design). So "this
# journey takes longer than that one" and "this way is shut until something
# is true" cannot live in facts.json; they live here.
#
# Run order: import-sections sections.json -> import-facts facts.json -> this.
# NOT RUN by the author of this corpus (no commands were permitted).

set -e

# --- registries and facts, for reference (the two halves this script assumes) ---
# mnemosyne-cli import-sections --manifest sections.json
# mnemosyne-cli import-facts    --manifest facts.json
# `minute` is registered in facts.json's `units` array, so add-edge-cost resolves.

# ---------------------------------------------------------------------------
# EDGE COSTS — travel time on each edge, in map minutes.
# Keyed by the `adjacent` fact id, i.e. by the DIRECTION. The hill is why the
# same road costs more one way than the other: the causeway and the postern
# stair are both dearer upward than downward.
# ---------------------------------------------------------------------------

# root scope: the two quarters, the causeway, the stair, the cistern, the bell
mnemosyne-cli add-edge-cost --fact f-edge-crown-causeway    --n 12 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-causeway-crown    --n 18 --unit minute   # uphill
mnemosyne-cli add-edge-cost --fact f-edge-causeway-tidegate --n 15 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-tidegate-causeway --n 22 --unit minute   # uphill
mnemosyne-cli add-edge-cost --fact f-edge-crown-stair       --n  6 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-stair-crown       --n  8 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-stair-cistern     --n  9 --unit minute   # the one-way descent
mnemosyne-cli add-edge-cost --fact f-edge-cistern-tidegate  --n 25 --unit minute   # the long wade under the hill
mnemosyne-cli add-edge-cost --fact f-edge-bell-cistern      --n 11 --unit minute

# the Crown's inside
mnemosyne-cli add-edge-cost --fact f-edge-shrine-almshouse     --n 5 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-almshouse-shrine     --n 5 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-almshouse-watchtower --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-watchtower-almshouse --n 2 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-shrine-watchtower    --n 6 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-watchtower-shrine    --n 5 --unit minute

# the Tidegate's inside
mnemosyne-cli add-edge-cost --fact f-edge-ferry-market   --n 4 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-market-ferry   --n 4 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-market-eelhouse --n 7 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-eelhouse-market --n 7 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-ferry-pump     --n 3 --unit minute
mnemosyne-cli add-edge-cost --fact f-edge-pump-ferry     --n 3 --unit minute

# ---------------------------------------------------------------------------
# EDGE GUARD — the way that is shut until something is true.
# The crossing from the Ferry Steps to the pump-house wharf requires BOTH
# conditions (no threshold set, so the set is ANDed): the tide-gate must be
# down, and the crank must be in Kesh's keeping. The way BACK off the wharf
# (f-edge-pump-ferry) is unguarded — you can always leave.
# Mnemosyne only records the declaration; the consumer evaluates it.
# ---------------------------------------------------------------------------

mnemosyne-cli add-edge-guard --fact f-edge-ferry-pump --condition f-gate-lowered
mnemosyne-cli add-edge-guard --fact f-edge-ferry-pump --condition f-crank-to-kesh
