# Runbook — dnd-quest-experiment/v2 (orchestrator only)

Targeted-repair execution. `manifest.json` is the SSOT for the pins + decision
rule; `repair-spec.md` is the deterministic deficiency list (from v1's blind
judges + extractor). The executing session is the ORCHESTRATOR (this lineage may
not author the repair facts, render, re-extract, or judge — R469).

## THE ONE PROMPT (paste into a fresh session to execute)

> Execute `claudedocs/phase1-dnd-quest-experiment/v2/` per its `runbook.md` and
> `manifest.json`. You are the ORCHESTRATOR. Run the steps in order: spawn the
> blind repair-author, rebuild fresh + run PIN-A (gates) and PIN-B (the
> repaired-vs-v1 facts diff + the affected-scene set), assemble the repaired
> outline + affected-scenes.txt, spawn the blind render-repair, splice it onto
> v1's prose + diff-check localization, re-extract blind + run PIN-C (leak +
> fidelity + the seam re-check), spawn 3 blind A/B judges (arm->version
> randomized, label-map sealed), then apply the pre-committed decision rule,
> write report.md, append the R564 changelog entry + commit. Do NOT push.

## Steps (detail in manifest grading_procedure)

0. Preflight: `run/{author,manuscripts,render,render/scenes,extract,judges,briefing}/` dirs; CLI current (report-playable-world present). Copy v1's quest-briefing.md into run/briefing/ (the judges reuse it).
1. Blind repair-author: repair-spec.md + author-repair-brief.md + the v1 base JSON -> run/author/ (repaired JSON + store + author-log). Closes D1/D2/D3, re-gates.
2. PIN-A + PIN-B: rebuild fresh from repaired JSON; run every gate (PIN-A); diff repaired facts.json vs v1 (PIN-B fact half) and compute the affected scene set (the scenes whose fact set changed).
3. Outline + affected list: report-playthrough-manuscript --world claim --telling delve -> run/manuscripts/world-claim.md; write run/render/affected-scenes.txt (the affected scene ids).
4. Blind render-repair: render-repair-brief.md + the repaired outline + v1's world-claim.md + affected-scenes.txt -> run/render/scenes/*.md (or repaired-scenes.md).
5. Splice + PIN-B render half: build run/render/world-claim.md = v1's render with ONLY the affected scenes swapped for the new prose; diff vs v1 confirms localization.
6. Extract + PIN-C: write run/extract/vocab.md (v1 ids + any new entity); spawn the blind extractor (v1 extractor-brief task + the spliced repaired render + vocab.md) -> run/extract/; run leak + fidelity; read the new extract-log for the seam re-check (are D1/D2/D3 still flagged?).
7. Blind A/B judges: 3 subagents, each gets judge-brief-ab.md + quest-briefing.md + arm X + arm Y (X/Y = {v1 render, repaired render} randomized per judge; record the label-map in run/judges/label-map.json, do NOT show judges) -> run/judges/judge-{1,2,3}.md.
8. Decide: unblind label-map, apply manifest decision_rule_pre_committed, write report.md, append R564 + commit. NO push.

## Tracked vs gitignored

Tracked: this dir's *.md + manifest.json + run/author/{sections,facts,order,narrative-rules}.json + author-log.md + run/manuscripts/*.md + run/render/*.md (+ scenes/) + render-log.md + affected-scenes.txt + run/extract/{sections,facts}.json + vocab.md + extract-log.md + run/judges/*.md + label-map.json + report.md. Gitignored: *.atomic.json + *.playthrough.json.
