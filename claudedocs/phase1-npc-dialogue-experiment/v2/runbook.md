# Runbook — npc-dialogue-experiment/v2 (orchestrator only)

Tests the owner's R551-followup: re-render ONLY the deficient parts and measure the lift.
The orchestrator selects the weak set from the v1 verdicts, splices arms, runs the gates;
three blind subagents (repair-render, re-extract, A/B judges x3) do the rest. R469 firewall.

## Sequence

0. **Pin (R552):** `sha256sum manifest.json` in the R552 ledger, committed before any subagent.

1. **Blind repair-render (R553):** spawn ONE fresh subagent (repair-brief.md) -> run/repair/
   {report-sc-16, confront-sc-29, confront-sc-30, shelter-sc-27}.md + repair-log.md.

2. **Splice arms (orchestrator):** arm C = the v1 manuscripts verbatim
   (v1/run/render/world-*.md). arm R = copies of the v1 manuscripts with EXACTLY the 4
   scene blocks (`## sc-NN` ... up to the next `## `) replaced by the repair output ->
   run/arms/R-world-{report,shelter,confront}.md. Record `diff` arm C vs arm R: ONLY the
   4 scene blocks differ (PIN-R3).

3. **Blind re-extract (orchestrator + subagent):** copy v1/run/extract/vocab.md ->
   run/extract/vocab.md; spawn the blind extractor (v1/extractor-brief.md + arm R
   manuscripts + vocab.md) -> run/extract/reextracted.atomic.json.

4. **PIN-R1 + PIN-R2 (orchestrator):** rebuild the authored store fresh (or reuse
   v1/run/author/verify.atomic.json, which carries the disclosure plan `holm`), then:
   - leak: `validate-disclosure-leak --telling holm --against run/extract/reextracted.atomic.json
     --world W --truth-frame gt --order <v1 order> --sidecar <v1 verify>` each W = leaks=0,
     vocab_shared>0.
   - fidelity: per-world single-world projection of the re-extraction (the v1 method:
     keep facts with branch in {W, spine}), `validate-render-fidelity --against <proj>
     --world W --order <v1 order> --sidecar <v1 verify>` = off_path=0, unplaced=0, terminal.
   Record verbatim.

5. **Blind A/B judges (R553):** build 3 shuffled packets (run/judges/packet-{1,2,3}.md) —
   each = the 4 scene-pairs as "Version 1" / "Version 2", with the arm->version mapping
   RANDOMIZED per judge and recorded ONLY in run/judges/label-map.md (orchestrator). Spawn
   3 blind subagents (ab-judge-brief.md + their packet) -> run/judges/judge-{1,2,3}.md.

6. **Decide + report (R553):** unblind via label-map; tally per-scene + overall A/B
   preference; apply the decision rule (manifest decision_rule_pre_committed); write
   report.md; commit. Update RESUME memory. Push only on the owner's explicit push word.

## Notes

- The arm->version randomization removes label bias; the label-map is the only place the
  mapping lives until report.md (judges blind).
- PIN-R3 is guaranteed by the splice (orchestrator replaces only the 4 blocks) and
  CONFIRMED by the recorded diff — any judged difference is attributable to the 4 scenes.
- No tools/ change; reuse v1 verbs + the single-world-projection method. Scratch
  *.atomic.json gitignored (the v1 .gitignore rule covers v2 as a subdir).
