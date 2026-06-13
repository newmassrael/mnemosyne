# Runbook — disclosure-craft-experiment/v1

**ORCHESTRATOR-ONLY.** The render author, the extractor, and the judges must
NEVER read this file, the manifest (`disclosure-manifest.json`), the design
doc, or the changelog. This file is the operational glue around the sha-pinned
manifest; the manifest is the SSOT, this is just how to run it with the least
effort.

**What makes this LOW-TOUCH:** two of the three inputs are REUSED from R500 —
the `plain` arm prose and the facts-first fact-base store. The only NEW
authoring is the disclosure-guided RENDER. The human types ONE prompt; the
orchestrator does the rest via blind subagents.

Validity rests on blind sub-roles, not the orchestrator: the render author is
blind to the comparison; the extractor + judges are blind to which story is
which and to the hypothesis. The orchestrator may know the mapping (recorded
honesty bound) — it just never leaks it into a sub-prompt. The orchestrator
also writes the pinned DISCLOSURE BRIEF into the render prompt (specifying the
treatment, exactly as the R500 orchestrator wrote the facts-first method) but
never writes prose, extracts, or judges (the R500/R502 contamination bound:
this lineage designed disclosure).

---

## 0. START — the one prompt (paste into a FRESH session)

> You are the ORCHESTRATOR for disclosure-craft-experiment/v1. Read
> `claudedocs/phase1-disclosure-craft-experiment/runbook.md` and
> `disclosure-manifest.json`, then execute steps 1-8. Spawn the render author,
> the extractor, and each judge as a SEPARATE blind subagent (Agent tool, fresh
> context), passing ONLY the verbatim prompt block from the runbook plus the
> firewall wrapper — never the manifest, the hypothesis, or the other arm. Run
> the harness + gate commands yourself. Do NOT author, render, extract, or
> judge in your own voice. Finish by writing `disclosure-report.md` and
> appending the execution ledger entry. If anything is ambiguous, stop and ask —
> do not improvise the protocol (it is sha-pinned).

That is the only prompt the human types. Everything below is what that
orchestrator session does.

---

## 1. Firewall — wrap EVERY subagent prompt with this

Prepend to every render / extractor / judge prompt:

> Work only inside the directory `{DIR}` I give you. Use ONLY the brief below.
> Do NOT read any other file in this repository — no changelog, no design docs,
> no runbook, no manifest, no other story, no `--list-changelog`. Save your
> output where the brief says. Do not ask why this task exists.

The disclosure render subagent additionally MAY query its own store
(`run/disclosure/ff.atomic.json`) read-only via `mnemosyne-cli --sidecar` — it
is the render's source of facts, nothing else.

---

## 2. Setup (orchestrator) — copy the reused inputs in

```
cd claudedocs/phase1-disclosure-craft-experiment
SRC=../phase1-factsfirst-craft-experiment/run
mkdir -p run/plain run/disclosure run/labels run/extract run/reading run/judges
cp $SRC/plain/story.md          run/plain/story.md          # plain = REUSED baseline
cp $SRC/factsfirst/ff.atomic.json run/disclosure/ff.atomic.json  # fact-base = REUSED (gitignored)
```

`*.atomic.json` under this tree is git-ignored (scratch stores); prose, defect
tables, reading copies, judge verdicts, and the report are tracked evidence.

Directory layout:

```
claudedocs/phase1-disclosure-craft-experiment/
  disclosure-manifest.json   (pinned; orchestrator-only)
  runbook.md                 (this; orchestrator-only)
  run/
    plain/      story.md                          <- REUSED from R500
    disclosure/ story.md  ff.atomic.json           <- NEW render + REUSED store
    labels/     story-A.md  story-B.md  label-map.json
    extract/    A.atomic.json  B.atomic.json  defects-A.md  defects-B.md
    reading/    <world>__A.md  <world>__B.md
    judges/     judge-1.md  judge-2.md  judge-3.md
  disclosure-report.md
```

---

## 3. Disclosure render (S1-S3) — ONE arm, 1-3 sequential blind subagents

`{DIR}` = `run/disclosure/`. The store is PRE-BUILT, so this is render-not-author
— combine into fewer sessions if the budget allows (record the count for the
economics leg). Scene budget if split 3 ways: s1 = sc-01..08 (incl fork-1),
s2 = sc-09..16 (both fork-1 limbs + fork-2), s3 = sc-17..>=24 (remaining limbs
+ endings).

Paste the manifest `arm_disclosure.prompt` with the DISCLOSURE BRIEF
(`arm_disclosure.disclosure_brief_PINNED`) inlined VERBATIM, plus the §1
firewall + the per-session line:

> [manifest arm_disclosure.prompt, with policy_default + focal_frame +
> reader_secrets_S1_S2 + solution_facts + required_setups inlined]
>
> THIS SESSION: render {sc-01..08 incl the fork-1 CHOICE | sc-09..16 incl both
> fork-1 limbs and the fork-2 CHOICE | sc-17 onward to >= 24, all remaining
> limbs and endings}. {On continuation: your prose so far is in story.md below;
> your fact source is ff.atomic.json — continue. <paste prior story.md>}

> ORCHESTRATOR: record render token cost + whether following the hand-brief was
> high-friction (the rung-2-substrate pull signal, PRED-4).

---

## 4. Shuffle + seal (S4a)

```
H=tools/experiment-harness/Cargo.toml
cargo run --manifest-path $H -- shuffle \
  --experiment disclosure-craft --note "reveal at S8" \
  --out claudedocs/phase1-disclosure-craft-experiment/run/labels/label-map.json \
  plain disclosure
# -> prints the sha256 SEAL. Record it verbatim in disclosure-report.md NOW (pre-reveal).
cd claudedocs/phase1-disclosure-craft-experiment/run
for L in A B; do
  arm=$(jq -er ".assignment.$L" labels/label-map.json) || { echo "map key $L missing"; exit 1; }
  cp "$arm/story.md" "labels/story-$L.md"
done
```

From here everything downstream uses **story-A / story-B only**.

**PRE-PIN the withhold list (S4a, before extraction):** from the source store
`run/disclosure/ff.atomic.json`, list the WITHHELD solution facts + each one's
reveal scene, into `disclosure-report.md` NOW (pre-reveal). This is the PIN-2
target; pinning it before re-extraction stops post-hoc target-shifting.

```
mnemosyne-cli --sidecar run/disclosure/ff.atomic.json query ...   # enumerate gt-frame solution facts + reveal coords
```

---

## 5. Blind re-extraction (S4b) — ONE extractor subagent, both stories

`{DIR}` = `run/extract/`. Paste (§1 firewall first) the manifest
`grading.deterministic.blind_reextraction` recipe VERBATIM (R469): per story
`story-A.md` then `story-B.md` into its own fresh store, frames from evident
knowledge structure, entities/predicates via report-typing-candidates ->
import-typing-proposals (dry-run), facts via import-facts from EXPLICIT text
only (scene id = canon coord, quote = evidence), edges via
report-edge-candidates -> import-edge-proposals, canon order from the story's
own fork graph, branches forks_at = last shared scene, <= 8 rules by the fixed
recipe. Save `defects-A.md` / `defects-B.md` with validate-continuity +
report-payoff-coverage + the recorded surfaces; cite scene-id + quote for
EVERY finding; never say which story is which.

Orchestrator then tabulates per story:
`D1 = rule_transition_invalid + rule_exclusive_overlap`,
`D2 = unchained_state_pairs`, `D4 = succession_cross_branch + SuccessionCycle`.

**PIN-2 premature-leak check (deterministic, R502):** for the disclosure story
(known via the seal, but the extractor stayed blind), for EACH pre-pinned
withheld solution fact, find its earliest canon coordinate in the
re-extraction; assert `>= its reveal scene`. Any earlier = a premature LEAK ->
run the leak-vs-noise audit (cited scene-id + quote: a leak is the solution
STATED in pre-reveal prose; an over-read hint or extractor over-inference is
noise). PRED-2b (recorded, not pinned): `report-irony-intervals` on the
re-extraction — do the early-shown secrets open >= 1 reader-knows / Hale-doesn't
window.

---

## 6. Reading copies (S4c) — deterministic, per matched world-line

Match world-lines across A/B by fork-choice correspondence
(confront/quiet x reveal/burn); spine = confront+reveal. For each story S in
{A,B} and matched world W:

```
mnemosyne-cli --sidecar run/extract/$S.atomic.json \
  report-playthrough-manuscript --world $W --json > run/extract/$S-$W.playthrough.json
cargo run --manifest-path tools/experiment-harness/Cargo.toml -- assemble \
  --story run/labels/story-$S.md --playthrough run/extract/$S-$W.playthrough.json \
  --world $W --out run/reading/${W}__$S.md
```

Record the R466 honesty surfaces (undeclared_adjacencies / unplaced /
undecidable) for both arms.

---

## 7. Judges (S5-S7) — 3 independent blind subagents

Each judge `{DIR}` = `run/judges/`, gets the matched reading-copy pairs
(`<world>__A.md` + `<world>__B.md`) and nothing else. Paste (§1 firewall first)
the manifest `grading.preference.judge_prompt` verbatim. Write each verdict to
`judge-{N}.md`. (Optional judge-4 = owner read, recorded separately.)

---

## 8. Reveal + grade (S8)

```
cargo run --manifest-path tools/experiment-harness/Cargo.toml -- verify-seal \
  --map claudedocs/phase1-disclosure-craft-experiment/run/labels/label-map.json \
  --sha256 <the seal recorded in step 4>          # exit 0 = untampered
```

Then, in `disclosure-report.md`:
1. Unblind: read `label-map.json` (which of A/B is `disclosure`).
2. **PIN-1 fidelity:** is `D1+D2+D4 = 0` for the disclosure story? Else
   leak-vs-noise audit. Plain's count beside it as control.
3. **PIN-2 premature-leak:** 0 withheld solution facts re-extractable before
   their reveal scene? Report the per-fact earliest-coordinate table. (PRED-2b
   irony windows recorded.)
4. **Craft (open, no pin):** tally the 3 judges' forced choices + 5-axis scores
   per matched world; report the headline preference. **Cross-ref R500's
   recorded scores** (omniscient facts-first 3.11 / plain 4.33 prose-quality):
   did disclosure move prose-quality above 3.11 toward 4.33? Did the 3-0 gap
   close? No pass/fail on craft.
5. **PRED-3 decision (manifest `decision_rule`):** gap closes + PIN-1 + PIN-2
   hold -> disclosure VALIDATED -> next round builds rung-2 substrate. Gap does
   not close -> do not build; reconsider.
6. **PRED-4 economics/residue:** render cost, hand-brief friction (rung-2 pull),
   untyped residue per arm.
7. Restate every honesty bound from the manifest that bit (esp. the cross-run
   plain baseline + reused fact-base).
8. Append the execution ledger entry restating the result table (R452
   self-containment). Commit; push on owner consent.

---

## 9. Pre-flight checklist (orchestrator, before step 3)

- [ ] `tools/experiment-harness` builds (shuffle smoke); `jq` present.
- [ ] Reused inputs copied: `run/plain/story.md` + `run/disclosure/ff.atomic.json`.
- [ ] Withhold list (PIN-2 targets) pre-pinned in `disclosure-report.md`.
- [ ] Every render/extractor/judge prompt wrapped with §1 firewall, NO hypothesis, NO "comparison", NO mention of the other arm.
- [ ] Quota gate (manifest validity_conditions): if the disclosure arm misses 24 scenes / 2 forks / 3 worlds / 3 endings / 2 frames / 6 setups, that is an economics finding — run the deterministic legs on what exists, skip preference if scales are incomparable.
