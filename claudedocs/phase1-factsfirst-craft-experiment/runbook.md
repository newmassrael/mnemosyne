# Runbook — factsfirst-craft-experiment/v1 (R499)

**ORCHESTRATOR-ONLY.** Authors / extractor / judges must NEVER read this file,
the manifest (`craft-manifest.json`), the design doc, or the project changelog.
This file is the operational glue around the sha-pinned manifest
(`3fe79c6f3ead13e4e99e803dc02e808c1aadec1caaf5e4b0e810bd3d4972e50d`); the
manifest is the SSOT, this is just how to run it with the least effort.

Validity rests on THREE blind sub-roles, not on the orchestrator: the authors
are blind to the comparison, the extractor + judges are blind to which story is
which and to the hypothesis. The orchestrator may know the mapping (recorded
honesty bound) — it just must never leak it into a sub-prompt.

---

## 0. START — the one prompt (paste into a FRESH session)

Open a new Claude Code session in this repo and paste exactly:

> You are the ORCHESTRATOR for factsfirst-craft-experiment/v1. Read
> `claudedocs/phase1-factsfirst-craft-experiment/runbook.md` and
> `craft-manifest.json`, then execute steps 1-8 of the runbook. Spawn every
> author, extractor, and judge as a SEPARATE blind subagent (Agent tool, fresh
> context), passing ONLY the verbatim prompt block from the runbook plus the
> firewall wrapper — never the manifest, the hypothesis, or the other arm. Run
> the harness + gate commands yourself. Do NOT author, extract, or judge in
> your own voice. Finish by writing `craft-report.md` and appending the R500
> ledger entry. If anything is ambiguous, stop and ask — do not improvise the
> protocol (it is sha-pinned).

That is the only prompt the human types. Everything below is what that
orchestrator session does.

---

## 1. Firewall — wrap EVERY subagent prompt with this

Prepend to every author / extractor / judge prompt:

> Work only inside the directory `{DIR}` I give you. Use ONLY the brief below.
> Do NOT read any other file in this repository — no changelog, no design docs,
> no runbook, no other story, no `mnemosyne-cli query --list-changelog`. Save
> your output where the brief says. Do not ask why this task exists.

Both arms use the **same model tier**. Give each subagent its own `{DIR}`.

---

## 2. Directory layout (orchestrator creates `run/`)

```
claudedocs/phase1-factsfirst-craft-experiment/
  craft-manifest.json     (pinned; orchestrator-only)
  runbook.md              (this; orchestrator-only)
  run/
    plain/      story.md                       <- plain arm deliverable
    factsfirst/ story.md  ff.atomic.json        <- ff arm deliverable + its scratch store
    labels/     story-A.md  story-B.md  label-map.json   <- shuffled + sealed
    extract/    A.atomic.json  B.atomic.json  defects-A.md  defects-B.md
    reading/    <world>__A.md  <world>__B.md    <- matched reading copies
    judges/     judge-1.md  judge-2.md  judge-3.md
    craft-report.md
```

`*.atomic.json` under this tree is git-ignored (scratch stores); the prose,
defect tables, reading copies, judge verdicts, and report are tracked evidence
(the scale-floor convention).

---

## 3. Authors (S1-S6) — two arms, 3 sequential subagents each

Within an arm the 3 sessions are **sequential continuations**: each is a fresh
subagent, but you hand it everything written so far. Across arms they are
independent and never see each other. Per-session scene budget: s1 = sc-01..08
(incl fork-1), s2 = sc-09..16 (both fork-1 limbs + fork-2 decl), s3 = sc-17..>=24
(all remaining limbs + endings).

### 3a. PLAIN arm — `{DIR}` = `run/plain/`

Paste (verbatim manifest `arm_plain.prompt`, premise inlined; add the §1
firewall + the per-session line):

> You are writing a branching mystery novella.
>
> PREMISE — "The Meridian Vane": A remote hilltop observatory, the Vane
> Observatory, the winter of 1898 — snow-cut from the valley for the season,
> only its small resident staff left on the summit. At dawn the director, Dr.
> Aurel Crane, is found dead at the foot of the iron stair beneath the great
> refractor, the dome shutter frozen open to the freezing sky and the meridian
> clock stopped at 3:14. The observatory's patron sends Lewin Hale, a methodical
> insurance adjuster, up the last passable road to certify whether the Society's
> instrument policy pays for an accident — or for something else. But the
> night-log for the observation due that night is written in a hand that is not
> Crane's; the assistant astronomer, Junia Frost, swears she was at the
> spectroscope until dawn though the plate-camera holds no exposures; and the
> meridian clock, stopped at 3:14, had been wound only hours before it stopped.
> READER-KNOWN SECRETS (show these to the reader early while keeping them from
> Hale): (S1) the bursar, Onslow Pike, forged the patron's telegram that
> "recalled" Crane to dismissal, to cover a fund he had quietly drained from the
> instrument account — Crane never knew he was to be dismissed, but Pike believed
> he did, and acted on that belief; (S2) Junia Frost was on the dome gallery that
> night, not at the spectroscope — she saw who climbed the iron stair, and she
> lies to shield them. Who climbed the stair, what the forged telegram set in
> motion, and what Hale certifies in the end are yours to decide — differently
> per world-line if the forks demand it.
>
> REQUIRED SETUPS (each must pay off in every world-line where it appears, a
> payoff may differ per world-line): (1) the night-log in the wrong hand; (2) the
> plate-camera with no exposures (it contradicts Junia's alibi); (3) the meridian
> clock stopped at 3:14, wound only hours before; (4) the drained instrument
> account (the bursar's motive); (5) a spare dome key reported lost a month
> before, that turns up; (6) Crane's unsent letter resigning his own post.
>
> REQUIRED STRUCTURE: >= 24 numbered scenes with stable ids (sc-01...), 150-400
> words each. Two fork points — fork-1 around scene 8 (Hale CONFRONTS Pike with
> the account discrepancy, or QUIETLY audits the ledgers and watches), fork-2
> around scene 16 on one declared limb (Junia REVEALS what she saw, or BURNS the
> night-log). >= 3 world-lines, >= 3 endings, >= 2 epistemic frames (ground truth
> vs at least one named character's belief — Hale's working theory and/or Pike's
> false belief; the reader-known S1/S2 must create at least one dramatic-irony
> window where the reader knows and Hale does not). Mark fork points "CHOICE:"
> with labeled options and which scene each option continues to; mark endings.
> Plain text / markdown.
>
> Keep the story consistent: character knowledge, object locations, who is alive,
> what has been revealed on each branch. Deliver scenes in the required format,
> appended to `story.md`.
>
> THIS SESSION: write {scenes sc-01..08 incl the fork-1 CHOICE | scenes sc-09..16
> incl both fork-1 limbs and the fork-2 CHOICE | scenes sc-17 onward to >= 24,
> all remaining limbs and endings}. {On continuation: everything written so far
> is in story.md below — continue it. <paste prior story.md>}

### 3b. FACTSFIRST arm — `{DIR}` = `run/factsfirst/`, store = `ff.atomic.json`

Paste (verbatim manifest `arm_factsfirst.prompt`, same PREMISE / SETUPS /
STRUCTURE blocks as 3a inlined, plus this method head; add §1 firewall + the
per-session line). The PREMISE / REQUIRED SETUPS / REQUIRED STRUCTURE text is
**identical to 3a** — paste those three paragraphs unchanged, then:

> You are writing the above branching mystery novella using the Mnemosyne
> narrative substrate, working FACTS-FIRST. Run `mnemosyne-cli` with
> `--sidecar ff.atomic.json` (your own scratch store). METHOD, in this order:
> (1) BEFORE writing any prose, author the fact-base — register the epistemic
> frames (a ground-truth frame plus at least one named-character belief frame),
> entities, and predicates; author the LOAD-BEARING TRUTH UP FRONT (what actually
> happened the night of the death, who climbed the stair, the true custody of the
> forged telegram and the drained account) as facts in the ground-truth frame
> with canon coordinates (scene ids) and evidence; author the misdirection (what
> each character BELIEVES — e.g. Pike's false belief that Crane knew of his
> dismissal; Hale's theory) in the belief frame(s). Declare succession edges for
> every state change and belief revision, fork points with forks_from/forks_at,
> branches per world-line, and a narrative-rules file (exclusive/transition
> classes only) for the story's own state predicates. (2) GATE-CHECK and REPAIR
> before rendering: run validate-continuity, report-payoff-coverage, and
> report-irony-intervals; a same-frame contradiction is a real error to fix, a
> cross-frame divergence is intended irony (data), report-irony-intervals shows
> you your mystery windows; mark required setups payoff_expectation=expected and
> credit pays_off when a scene pays one off. (3) RENDER the prose scene by scene
> as a PROJECTION of the gate-checked base: for each scene choose the focal
> frame, STATE that frame's holding facts in prose, WITHHOLD facts that hold only
> in another frame (= mystery), and let the cross-frame divergence carry the
> dramatic irony. You may author + gate-check incrementally across the 3 sessions
> (truth up front, scene-facts as you reach them), gate-checking and repairing at
> every session end. Deliver the SAME scene format the brief requires, appended to
> `story.md`. Your store is your tool — only `story.md` is delivered.
>
> THIS SESSION: {s1 — author the load-bearing truth + frames up front, gate it
> clean, render scenes sc-01..08 incl the fork-1 CHOICE | s2 — render sc-09..16
> incl both fork-1 limbs and the fork-2 CHOICE, extend + re-gate | s3 — render
> sc-17 onward to >= 24, all remaining limbs and endings, extend + re-gate}.
> {On continuation: your prose so far is in story.md below and your store is at
> ff.atomic.json — continue. <paste prior story.md>}

> NOTE for the orchestrator: record the factsfirst arm's token usage and the
> fact-authoring-vs-render split (PRED-3 economics), and whether the hand-render
> friction was large enough to justify building render-brief (sec 7.21 step 2).

---

## 4. Shuffle + seal (S7a)

```
mnemosyne-cli --version >/dev/null   # sanity
H=tools/experiment-harness/Cargo.toml
cargo run --manifest-path $H -- shuffle \
  --experiment factsfirst-craft --note "reveal at S11" \
  --out claudedocs/phase1-factsfirst-craft-experiment/run/labels/label-map.json \
  plain factsfirst
# -> prints the sha256 SEAL. Record it verbatim in craft-report.md NOW (pre-reveal).
```

Materialize the blind copies (file->file; the seal already protects the map
from post-hoc edits):

```
cd claudedocs/phase1-factsfirst-craft-experiment/run
for L in A B; do
  arm=$(jq -er ".assignment.$L" labels/label-map.json) || { echo "map key $L missing"; exit 1; }
  cp "$arm/story.md" "labels/story-$L.md"
done
```

From here everything downstream uses **story-A / story-B only**.

---

## 5. Blind re-extraction (S7b) — ONE extractor subagent, both stories

`{DIR}` = `run/extract/`. Paste (§1 firewall first):

> You are re-extracting two short branching mystery stories into fresh Mnemosyne
> stores, blind. For EACH story — `story-A.md` then `story-B.md`, given to you —
> into its own fresh store (`A.atomic.json`, `B.atomic.json`), follow this exact
> procedure, identically for both:
> (1) register frames per the story's evident knowledge structure (ground truth +
> the named-character belief frames it actually uses);
> (2) entities + predicates via report-typing-candidates -> import-typing-proposals
> (dry-run review, declared vocabulary only);
> (3) facts via import-facts manifests built scene by scene from EXPLICIT story
> text ONLY — no inference beyond the page; scene id as canon coordinate, a quote
> as evidence;
> (4) edges via report-edge-candidates -> import-edge-proposals (two-sided
> claim-sha, fill-blanks only);
> (5) canon order declared from the story's OWN explicit scene graph (the fork
> options' continuation pointers), interleaved at forks;
> (6) branches with forks_from / forks_at = last shared scene.
> Then derive AT MOST 8 rules per story, exclusive/transition classes only, by
> this fixed recipe for both: one alive-arc transition rule per character who
> dies on any limb; one exclusive-custody rule per unique physical object two
> parties hold (the night-log and the spare dome key at minimum); one
> location-exclusive rule per principal character if the text asserts simultaneous
> presence; nothing else.
> Then, per story, run and SAVE to `defects-A.md` / `defects-B.md`:
> validate-continuity (report rule_transition_invalid, rule_exclusive_overlap,
> unchained_state_pairs, succession_cross_branch, SuccessionCycle counts),
> report-payoff-coverage (dangling among the 6 required setups), and the
> recorded-not-counted surfaces (payoffs_to_unmarked, payoff_before_setup,
> cross_scope_pairs, undecidable/unknown). Cite scene-id + quote for EVERY
> finding. Do not say which story you think is which.

Orchestrator then reads `defects-A.md` / `defects-B.md` and tabulates per story:
`D1 = rule_transition_invalid + rule_exclusive_overlap`, `D2 = unchained_state_pairs`,
`D4 = succession_cross_branch + SuccessionCycle`. (D3 = dangling required setups,
recorded, NOT in the pin.)

---

## 6. Reading copies (S7c) — deterministic, per matched world-line

Match world-lines across A/B by fork-choice correspondence
(confront/quiet x reveal/burn); spine = confront+reveal if structures diverge.
For each story S in {A,B} and each matched world W:

```
mnemosyne-cli --sidecar run/extract/$S.atomic.json \
  report-playthrough-manuscript --world $W --json > run/extract/$S-$W.playthrough.json
cargo run --manifest-path tools/experiment-harness/Cargo.toml -- assemble \
  --story run/labels/story-$S.md --playthrough run/extract/$S-$W.playthrough.json \
  --world $W --out run/reading/${W}__$S.md
```

Record the R466 honesty surfaces (undeclared_adjacencies / unplaced / undecidable)
for both arms into the run log.

---

## 7. Judges (S8-S10) — 3 independent blind subagents

Each judge `{DIR}` = `run/judges/`, gets the matched reading-copy pairs
(`<world>__A.md` + `<world>__B.md`) and nothing else. Paste (§1 firewall first):

> You are judging two versions (A and B) of the same branching mystery, one
> world-line at a time. For each matched world-line you are given, read both
> linear versions, then answer:
> 1. continuity / causality errors you noticed (count them and cite scene ids);
> 2. prose quality — does it read as a TOLD STORY with a voice, or as assembled /
>    list-like statements of fact? 1-5;
> 3. stakes and tension 1-5;
> 4. setup / payoff satisfaction 1-5;
> 5. character-knowledge believability (does anyone act on what they cannot
>    know) 1-5;
> 6. overall forced choice: A or B, one paragraph why.
> Do not speculate about how either version was produced. Write your verdict to
> `judge-{N}.md`.

(Optional judge-4 = owner read, recorded separately.)

---

## 8. Reveal + grade (S11)

```
cargo run --manifest-path tools/experiment-harness/Cargo.toml -- verify-seal \
  --map claudedocs/phase1-factsfirst-craft-experiment/run/labels/label-map.json \
  --sha256 <the seal recorded in step 4>          # exit 0 = untampered
```

Then, in `craft-report.md`:
1. Unblind: read `label-map.json` (which of A/B is `factsfirst`).
2. **PRED-1 (the pin):** is `D1 + D2 + D4 = 0` for the **factsfirst** story? If
   non-zero, run the leak-vs-noise audit — trace each gated finding to its cited
   scene-id + quote: a finding from a PROSE statement that contradicts what the
   factsfirst base held = a render LEAK (PRED-1 FALSIFIED); a finding the
   extractor inferred beyond the page, or one ALSO present in the plain
   re-extraction, = common-mode extractor noise (not charged). Report the plain
   story's D1+D2+D4 beside it as the control.
3. **Craft (open, no pin):** tally the 3 judges' forced choices + the 5-axis
   scores per matched world; report the headline preference and the prose-quality
   axis honestly. No pass/fail.
4. **PRED-3 economics** (factsfirst token cost / scene, the fact-vs-render split,
   render-brief pull) and **PRED-4 residue** (judge-cited errors no D-metric
   caught, FF vs plain).
5. Restate every honesty bound from the manifest that bit.
6. Append the **R500** ledger entry (`mnemosyne-cli append-changelog-entry`)
   restating the result table — R452 self-containment. Commit; push on owner
   consent.

---

## 9. Pre-flight checklist (orchestrator, before step 3)

- [ ] `tools/experiment-harness` builds (`cargo run --manifest-path tools/experiment-harness/Cargo.toml -- shuffle ...` smoke); `jq` present.
- [ ] `run/` dirs created; both arms use the same model tier.
- [ ] Every author/extractor/judge prompt is wrapped with the §1 firewall and contains NO hypothesis, NO "comparison", NO mention of the other arm.
- [ ] Quota gate (manifest validity_conditions): if an arm misses 24 scenes / 2 forks / 3 worlds / 3 endings / 2 frames / 6 setups, that is itself an economics finding — run the deterministic leg on what exists, skip preference if scales are incomparable.
