# Runbook — dnd-quest-experiment/v3 (orchestrator only)

Story-EXPANSION execution (the R565 tier-3 / owner choice A0). `manifest.json` is
the SSOT for pins + decision. The executing session is the ORCHESTRATOR (may not
author/render/extract/judge — R469). The LEAK RE-PROVE is the load-bearing gate.

## Steps (detail in manifest grading_procedure)

0. Preflight: `run/{author,manuscripts,render/scenes,extract,judges,briefing}/`; CLI current; copy v2's quest-briefing.md into run/briefing/.
1. Blind expansion-author: author-expand-brief.md + the v2 base JSON -> run/author/. Adds the delver frame + sc-00x prologue + facts; re-points the journal; re-gates + self leak-check (figure never named before sc-14).
2. PIN-1 gates: rebuild fresh from expanded JSON; run continuity/fork/timeline/payoff/playable-world; confirm the prologue is ordered, the quest contract/map/divergence preserved, no new dangling. (Leak re-prove deferred to step 6.)
3. Outline + affected list: report-playthrough-manuscript --world claim --telling delve -> run/manuscripts/world-claim.md; affected-scenes.txt = the prologue scene ids + any touched existing scene (sc-13/sc-14).
4. Blind render-expand: render-expand-brief.md + the expanded outline + v2's world-claim.md + affected-scenes.txt -> run/render/scenes/*.md (robed-figure discipline).
5. Splice: v3 render = v2's render with the prologue PREPENDED + touched scenes swapped; diff confirms only the affected set changed (the existing body byte-identical).
6. Extract + PIN-1 leak re-prove + PIN-2: vocab.md (v2 ids + delver frame); blind extractor (v1 extractor-brief task + the spliced v3 render + vocab.md) -> run/extract/; run validate-disclosure-leak (f-004 must still first-re-extract at sc-14 — the load-bearing check) + fidelity; read the extract-log for the D1 seam re-check (is the delver's investigation now shown? does the naming stay at sc-14?).
7. Blind A/B judges: 3 subagents, judge-brief-ab.md + quest-briefing.md + v2-render (arm) + v3-render (arm), randomized per judge, label-map sealed -> run/judges/.
8. Decide: unblind, apply manifest decision_rule_pre_committed (close_proven / leak_breaks / d1_persists / just_longer), write report.md, append R567 + commit. NO push.

## Tracked vs gitignored

Tracked: *.md + manifest.json + run/author/{sections,facts,order,narrative-rules}.json + author-log.md + run/manuscripts/*.md + run/render/scenes/*.md + render-log.md + affected-scenes.txt + run/render/world-claim.md + run/extract/{sections,facts}.json + vocab.md + extract-log.md + run/judges/*.md + label-map.json + report.md. Gitignored: *.atomic.json + *.playthrough.json.
