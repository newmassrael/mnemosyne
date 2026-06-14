# Runbook — disclosure-craft-experiment/v3 (warm-render posture)

**ORCHESTRATOR-ONLY.** The warm author, the extractor, and the judges must NEVER
read this file, the manifest (`warm-manifest.json`), the design doc, the
changelog, the compliance prose, or each other's prompts. This file is the
operational glue around the sha-pinned manifest; the manifest is the SSOT.

**What makes this LOW-TOUCH:** four of the five inputs are REUSED from R504 — the
compliance arm prose, the fact base, the disclosure plan `dc-v2`, the canon order,
and the extractor vocabulary. The only NEW authoring is the warm-posture RENDER +
its gate-repair. The human types ONE prompt (the owner's `experiment` word
triggers it); the orchestrator does the rest via blind subagents.

Validity rests on blind sub-roles: the warm author is blind to the comparison and
the gates; the extractor + judges are blind to which story is which and to the
hypothesis. The orchestrator may know the A/B mapping (recorded honesty bound) —
it never leaks it into a sub-prompt; it never authors prose, extracts, or judges
(the R469/R500/R502 contamination bound: this lineage designed disclosure).

---

## 0. START — the one prompt (already executing in this session)

The owner's `experiment` / `실험` word triggers execution. The orchestrator:
authors + sha-pins this manifest (R514, committed before any subagent runs),
then runs S1-S7, spawning the warm author / extractor / each judge as a SEPARATE
blind subagent (Agent tool, fresh context), passing ONLY the verbatim brief block
+ the §1 firewall — never the manifest, the hypothesis, the gates, or the other
arm. The orchestrator runs the harness + gate commands itself. It does NOT author,
render, extract, repair-in-its-own-voice, or judge. If anything is ambiguous, stop
and ask — do not improvise the sha-pinned protocol.

---

## 1. Firewall — wrap EVERY subagent prompt with this

> Work only inside the directory `{DIR}` I give you. Use ONLY the brief below.
> Do NOT read any other file in this repository — no changelog, no design docs,
> no runbook, no manifest, no other story, no `--list-changelog`. Save your
> output where the brief says. Do not ask why this task exists.

---

## 2. Setup (orchestrator) — DONE at R514 manifest round

```
v3/authored.atomic.json   <- typed fact base + dc-v2 plan (REUSED from v2)
v3/meridian-order.json     <- branch-scoped canon order (REUSED)
v3/extractor-brief.md      <- the FIXED blind extraction vocabulary (REUSED)
v3/skeleton.atomic.json    <- registries, zero facts (REUSED, extractor start point)
v3/run/compliance/story.md <- arm C = R504 disclosure render, REUSED verbatim
v3/warm-brief.md           <- arm W treatment (orchestrator-authored, NEW)
```
`*.atomic.json` / `*.playthrough.json` under v3/ are git-ignored (scratch);
the prose (warm + compliance), the manifest, the warm brief, the order, the
extractor brief, the runbook, and report.md are tracked evidence.

---

## 3. S1 — warm render (1-3 blind subagents)

`{DIR}` = `run/warm/`. Paste (§1 firewall first) the ENTIRE `warm-brief.md`. The
author writes the full 29-scene branching manuscript to `run/warm/story.md` in id
order. If a session stops at a scene boundary, continue it (SendMessage if
available, else a fresh blind subagent that reads its own prior `story.md` for
voice continuity — NEVER the compliance prose). Record render token cost + session
count (the posture-tax economics leg).

---

## 4. S2 — gate-and-repair (arm W only; orchestrator runs gates, blind subagent repairs)

1. **Blind re-extract arm W.** `{DIR}` = `run/extract/`. Paste (§1 firewall) the
   `extractor-brief.md` verbatim; task = re-extract `run/warm/story.md` into
   `run/extract/W.reextract.atomic.json`. (This is the gate input; the extractor
   is blind to arm + hypothesis.)
2. **Run the gates (orchestrator).** Pre-pin the withhold list in report.md FIRST.
   ```
   M=crates/mnemosyne-cli   # mnemosyne-cli on PATH
   for W in reveal burn audit; do
     mnemosyne-cli --sidecar v3/authored.atomic.json validate-disclosure-leak \
       --telling dc-v2 --against v3/run/extract/W.reextract.atomic.json \
       --world $W --truth-frame gt --order v3/meridian-order.json --json
   done
   # fidelity: project the re-extraction to each world's scene set, then:
   mnemosyne-cli --sidecar v3/authored.atomic.json validate-render-fidelity \
     --against <W-world-projection.json> --world $W --order v3/meridian-order.json --json
   # continuity D-metrics on the W re-extraction:
   mnemosyne-cli --sidecar v3/run/extract/W.reextract.atomic.json validate-continuity --json
   ```
3. **Repair (blind, story-framed) per violation.** For each leak/fidelity/D
   finding, write a SCENE-SCOPED STORY reason (e.g. "the climber's identity should
   still be a mystery in sc-11; your draft names him — rewrite sc-11 so Hale still
   doesn't know, keep the warmth and everything else") and spawn a blind subagent
   to rewrite ONLY that scene in `run/warm/story.md`. NEVER a compliance frame (no
   fact-id/mode/coord). Re-extract + re-run gates until PIN-W1 + PIN-W2 clean.
4. **Record the posture tax:** repair count, repair localization (touched scenes /
   29), render tokens.

---

## 5. S3 — shuffle + seal + reading copies

```
H=tools/experiment-harness/Cargo.toml
cargo run --manifest-path $H -- shuffle --experiment disclosure-craft-v3 \
  --note "warm vs compliance; reveal at S7" \
  --out v3/run/labels/label-map.json warm compliance
# -> prints the sha256 SEAL. Record it verbatim in v3/report.md NOW (pre-reveal).
```
Re-extract BOTH stories blind (one extractor, both A and B labeled prose) for the
reading-copy playthroughs (W's re-extraction from S2 may be reused for the warm
side; the compliance side needs its own). For each story S in {A,B} and matched
world W in {reveal(W1), burn(W2), audit(W3)}:
```
mnemosyne-cli --sidecar v3/run/extract/$S.reextract.atomic.json \
  report-playthrough-manuscript --world $W --reading-walk --order v3/meridian-order.json --json \
  > v3/run/extract/$S-$W.playthrough.json
cargo run --manifest-path $H -- assemble \
  --story v3/run/labels/story-$S.md --playthrough v3/run/extract/$S-$W.playthrough.json \
  --world $W --out v3/run/reading/${W}__$S.md
```
(story-A/story-B come from the seal map; W and C share the scaffold so the three
worlds align 1:1.)

---

## 5b. R525 — canonical reading-copy regeneration (SUPERSEDES S5's manual normalization)

The two manual pre-`assemble` normalizations S5 described (heading `.norm` +
scaffolding `.clean`, done inline = the SSOT-dup debt) are RETIRED. R516 folded the
scaffolding strip into the harness; R525 added `assemble --titles-from <store>`
(canonical headings from the fact base) + a bare-`## sc-NN` parser path. The six
reading copies are now one deterministic command each over tracked inputs (the
full-walk playthrough = 19/18/16 scenes; `authored.atomic.json` is tracked):

```
for W in reveal burn audit; do
  mnemosyne-cli report-playthrough-manuscript --world $W --order v3/meridian-order.json \
    --sidecar v3/authored.atomic.json --json > /tmp/$W.playthrough.json
  # A = run/warm/story.md, B = run/compliance/story.md (per label-map.json)
  for S_src in "A:run/warm" "B:run/compliance"; do
    S=${S_src%%:*}; D=${S_src##*:}
    cargo run --manifest-path tools/experiment-harness/Cargo.toml -- assemble \
      --story v3/$D/story.md --playthrough /tmp/$W.playthrough.json --world $W \
      --titles-from v3/authored.atomic.json --out v3/run/reading/${W}__$S.md
  done
done
```

This regenerates the six copies byte-identically from tracked inputs. No inline
Python, no `.norm`/`.clean` files.

## 6. S4-S6 — 3 blind judges

Each judge `{DIR}` = `run/judges/`, gets the three matched reading-copy pairs and
nothing else. Paste (§1 firewall first) the manifest `grading.preference.judge_prompt`
verbatim. Write each verdict to `judge-{N}.md`.

---

## 7. S7 — reveal + grade

```
cargo run --manifest-path tools/experiment-harness/Cargo.toml -- verify-seal \
  --map v3/run/labels/label-map.json --sha256 <the recorded seal>   # exit 0 = untampered
```
Then in `v3/report.md`: unblind; report PIN-W1 + PIN-W2 (W final); tally the 3
judges' forced choices + 5-axis means per world (W vs C) + the R504 plain
cross-ref + the re-cool sub-question vs the recorded repair localization; apply
the manifest `decision_rule` fork; record the posture tax + honesty bounds; append
the execution ledger entry (R452 self-containment). Commit; push on owner consent.

---

## 8. Pre-flight checklist

- [ ] `tools/experiment-harness` builds (shuffle smoke).
- [ ] Reused inputs present (authored / order / extractor-brief / skeleton /
      compliance story.md / warm-brief.md).
- [ ] Manifest sha pinned in the R514 ledger entry BEFORE S1.
- [ ] Withhold list (PIN-W2 targets) pre-pinned in report.md before re-extraction.
- [ ] Every subagent prompt wrapped with §1 firewall; NO hypothesis, NO "warm vs",
      NO gates, NO mention of the other arm.
- [ ] Repairs are story-framed only (validity_conditions.repair_register).
- [ ] Quota gate: if arm W misses the 29-scene/2-fork/3-world/3-ending scaffold,
      that is a finding — run deterministic legs on what exists, skip preference if
      scales are incomparable.
