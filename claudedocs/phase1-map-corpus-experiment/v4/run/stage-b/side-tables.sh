#!/bin/sh
# Cairnwell side tables: travel times and the one shut way.
#
# These two things are NOT authorable in any manifest. The contract's
# "side tables (verb-only)" section is explicit: edge_costs and edge_guards are
# reached ONLY through their own verbs, keyed by an already-existing fact, and
# writing an "edge_costs" / "edge_guards" array into a facts manifest is a
# SILENT no-op (unknown manifest keys are ignored, exit 0, zero rows). So the
# world is two artifacts, not one: the files, and then these calls.
#
# Run order (each call needs its target to exist already):
#   1. import-sections --manifest sections.json
#   2. import-facts    --manifest facts.json      (registers the units too)
#   3. this file
#
# Not executed by the authoring run.

set -e

# --- travel times, in minutes -------------------------------------------
# The unit `minute` is registered by the `units` array of facts.json.
# Costs are per DIRECTED edge, which is how the uphill/downhill and
# with-the-current/against-the-current asymmetries get said at all.

# market square <-> High Water quarter (the paved lane)
mnemosyne-cli add-edge-cost --fact f-adj-market-highwater  --n 6  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-highwater-market  --n 6  --unit minute

# market square <-> shrine of the ford (the shrine walk)
mnemosyne-cli add-edge-cost --fact f-adj-market-shrine     --n 4  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-shrine-market     --n 4  --unit minute

# market square <-> head of the cairn stair (downhill out, uphill back)
mnemosyne-cli add-edge-cost --fact f-adj-market-stair      --n 3  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-stair-market      --n 5  --unit minute

# shrine of the ford <-> High Water quarter (the contour footpath)
mnemosyne-cli add-edge-cost --fact f-adj-shrine-highwater  --n 7  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-highwater-shrine  --n 7  --unit minute

# the cairn stair down to the boat landing: quick, and one way only
mnemosyne-cli add-edge-cost --fact f-adj-stair-landing     --n 2  --unit minute

# the punt crossing (out on the ebb, back against the current)
mnemosyne-cli add-edge-cost --fact f-adj-landing-drowned   --n 11 --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-drowned-landing   --n 14 --unit minute

# the roof-plank ladder up out of the Drowned quarter: the long way
mnemosyne-cli add-edge-cost --fact f-adj-drowned-highwater --n 20 --unit minute

# inside the High Water quarter
mnemosyne-cli add-edge-cost --fact f-adj-almshouse-cooper  --n 2  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-cooper-almshouse  --n 2  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-cooper-ropewalk   --n 3  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-ropewalk-cooper   --n 3  --unit minute

# inside the Drowned quarter
mnemosyne-cli add-edge-cost --fact f-adj-eelquay-mill      --n 5  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-mill-eelquay      --n 5  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-eelquay-bell      --n 9  --unit minute
mnemosyne-cli add-edge-cost --fact f-adj-bell-eelquay      --n 9  --unit minute

# --- the way that is shut until something is true ------------------------
# The crossing out of the boat landing into the Drowned quarter is shut while
# the punt is chained. The condition is a real fact (f-punt-unchained, sc-08,
# Ordel beating the padlock off); Mnemosyne only holds the link and checks that
# both ends resolve. Whether it holds at a given moment is the consumer's call.
# One condition, so the set is a bare AND of one and needs no K-of-N threshold.
mnemosyne-cli add-edge-guard --fact f-adj-landing-drowned --condition f-punt-unchained
