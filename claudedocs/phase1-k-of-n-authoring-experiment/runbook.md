# Runbook — k-of-n-authoring-experiment

**ORCHESTRATOR-ONLY.** The authors must NEVER read this file, the changelog, the
design docs, any other author's work, or any verb output beyond what is handed
to them. This file is only *how to run it*.

**What this measures.** Both K-of-N thresholds in this substrate have NO authored
witness, and Round 961 says why rather than leaving it a silence: both of its
blind authors declined the access threshold because *"each shut way here has
exactly one thing that opens it."* That is a property of the world they were
given, not a gap in what they understood. Round 961's carry names the only way
to close the branch — **a premise that REQUIRES two-of-three** — and this is that
arm.

**The baseline, censused at Round 966**, field-keyed over every JSON under
`claudedocs/` (99 stores, 65 fact manifests, 43 rules files):

| axis | key present | non-null `k` |
|---|---|---|
| `edge_guards[].threshold` (access, R723) | 2 files, `null` in both | **0** |
| `first_at[].threshold` (disclosure, R752) | **0 files** | **0** |

Six authoring scripts *reason about* the access threshold in prose and decline
it (*"no threshold set = require ALL, the canonical form"*). So the silence is a
choice by authors who understood the mechanism. The count above is the oracle,
and it only moves if a corpus is authored.

**Why the disclosure axis is in this arm and not deferred.** The two thresholds
are the same word, the same `Option<usize>` shape, and **opposite defaults**: an
edge guard's `None` is require-ALL, a reveal's `None` is FIRST-reached (k=1).
Round 752's doc comment says the difference is deliberate. Whether an author who
learns one transfers it wrongly to the other is unmeasured, and no count can
answer it — only an author reaching for both can. Stage C is a SEPARATE corpus
in its own directory, so it confounds nothing: the access corpus has no telling
and the disclosure corpus needs no map.

**Round 966 had to run before this arm could.** The pre-flight found the contract
paragraph a disclosure author lands on teaching `withhold` + `first_at` as the
reveal idiom — the shape `report-disclosure-coverage` has called an *inert reveal
pin* since Round 946. An author following it would have produced a corpus whose
stored `k` no human surface can show, and this arm would have measured a blank
that was a documentation defect rather than the corpus's answer. That is the
Round 957 precedent, and it is why the pre-flight is a step and not a formality.

**Both witness reads were verified on a scratch store before this arm existed**,
so a blank here is the corpus's answer and not an untested projection:

- access — `report-transition-map` prints `[guard 2-of-3: …]` against
  `[guard all: …]`;
- disclosure — `report-playthrough-manuscript --telling` prints
  `first_at={…} k=2` against `k=1`.

⚠ **The obvious disclosure read is the WRONG one.** `report-playable-world`
returns `0 locator(s)` for a withheld fact however it is pinned, so reading the
threshold there measures a blank on a correct corpus. Use the manuscript read.

**What makes this LOW-TOUCH:** no prose judging, no extraction, no judge panel.
The reads are mechanical. The human types ONE prompt.

Validity rests on the authors being blind, not on the orchestrator. The
orchestrator may know the hypothesis; it never writes a manifest, never edits an
author's output, and never puts the words *threshold*, *K-of-N*, *condition*,
*guard*, *edge*, *cost*, *side table*, *manifest array*, *verb*, *disclosure*,
*telling*, *reveal*, *withhold*, `edge_guards`, `first_at`, or `surface` into a
Stage A or Stage C prompt.

---

## 0. START — the one prompt (paste into a FRESH session)

> You are the ORCHESTRATOR for k-of-n-authoring-experiment/v1. Read
> `claudedocs/phase1-k-of-n-authoring-experiment/runbook.md`, then execute steps
> 1-7. Spawn each author as a SEPARATE blind subagent (Agent tool, fresh
> context), passing ONLY the verbatim prompt block from this runbook plus the
> firewall wrapper — never this runbook, the changelog, any design doc, or any
> verb output. Run every command yourself. Do NOT author any manifest in your
> own voice, and do not repair an author's file: hand back the verbatim tool
> error and let them fix it. Finish by writing `k-of-n-report.md`, the
> `replay.json`, and the changelog entry. If anything is ambiguous, stop and ask
> — do not improvise the protocol.

---

## 1. Firewall — wrap EVERY author prompt with this

> Work only inside the directory `{DIR}` I give you. Use ONLY the brief below
> and the command outputs I hand you. Do NOT read any other file in this
> repository — no changelog, no design docs, no runbook, no other author's work,
> no `--list-changelog`. Save your output where the brief says. Do not ask why
> this task exists.

## 2. Setup (orchestrator)

One anchor so no relative depth can drift:

```
MN="$(git rev-parse --show-toplevel)/scripts/mn"
```

**The seed's schema version is DERIVED, never typed in** (Round 944's rule, and
Round 958 found two live runbooks seeding a stale literal that still imports
because the loader migrates — so the wrong number produces no error at all):

```
cd "$(git rev-parse --show-toplevel)/claudedocs/phase1-k-of-n-authoring-experiment"
SV="$("$MN" describe-schema | sed -n '1s/.*schema v\([0-9]\+\).*/\1/p')"
[ -n "$SV" ] || { echo "could not read the schema version from describe-schema"; exit 1; }
for d in vN/run/stage-a vN/run/stage-b vN/run/stage-c; do
  mkdir -p $d/docs/.atomic
  printf '[workspace]\n[continuity]\ncanon_order_path = "order.json"\n' > $d/mnemosyne.toml
  printf '{"schema_version":%s,"sections":{},"changelog_entries":{}}' "$SV" > $d/docs/.atomic/workspace.atomic.json
  "$MN" describe-schema > $d/contract.txt
done
"$MN" describe-schema > vN/run/contract.txt   # the tracked copy
```

**The seed names no rules file** — the Round 934 fix, carried. Round 943 and
Round 961 both measured that it needs no patching afterwards either: 4 of 4
authors across those arms wired `rules_path` into `mnemosyne.toml` themselves.
Before the read in step 5, CHECK that it resolves for the stages that need a map
(A and B):

```
grep -n rules_path $d/mnemosyne.toml   # and confirm that file exists
```

**If it already resolves, DO NOT REWRITE IT.** Overwriting an author's file is
forbidden twice in this runbook; in Round 943 the literal `printf` this
instruction used to carry would have deleted an authored `interval_severity`
line. Only if `rules_path` is absent or names a file the author did not write
does the orchestrator APPEND the single missing line, and the report must record
that it did.

`vN/run/*/docs/.atomic/*.json` is scratch (gitignored). The manifests, the
self-reports, the frozen first submissions, the reads and the report are tracked
evidence.

## 3. Stage A — discoverability on the access axis (measures E1)

Spawn ONE blind author with the firewall plus this brief. **None of the
forbidden words above may appear.** The fiction must make two-of-three
NECESSARY without naming any machinery.

> Author a small world as structured data for a story engine, using ONLY the
> authoring contract in `contract.txt` in your directory. The contract is the
> complete description of what the engine accepts; follow it exactly.
>
> The world: a walled trading post of eight to fourteen named locations that
> people move between. Write fifteen to twenty-five scenes in which people move
> through it and something happens at each place.
>
> One thing about this post matters as much as its layout. The strongroom is
> shut by an old council lock, and the custom of the house is that it opens for
> ANY TWO of the three wardens who each carry a seal — never one alone, and the
> third warden is often away, which is the whole reason the custom exists. Other
> ways through the post are shut too, and those are simpler: the water-stair is
> passable only at low water, and the tally-office needs the clerk's key.
>
> Someone planning a route through the post must be able to tell which ways are
> shut to them and what would open each one — the strongroom included, with its
> two-of-three custom intact — from your data alone, without reading your prose.
>
> Write these files in your directory: `sections.json`, `order.json`,
> `facts.json`, and any other file the contract tells you to write. Then write
> `self-report.md`: in plain prose, how many locations the post has, how many
> ways lead between them, WHICH WAYS ARE SHUT AND WHAT OPENS EACH, how you said
> the strongroom's two-of-three custom in particular, and which of your files
> says each. Do not run any command except those I give you.

**Freeze the first submission BEFORE handing anything back** (this is E2):

```
cd vN/run/stage-a
cp -r . ../stage-a-first-submission
"$MN" import-sections --manifest sections.json > first-import.log 2>&1; echo "exit=$?" >> first-import.log
"$MN" import-facts    --manifest facts.json   >> first-import.log 2>&1; echo "exit=$?" >> first-import.log
```

Keep `first-import.log` verbatim — full text, both exit codes, no filtering.
Then hand the author the verbatim errors and let them iterate to green.

**THE SELF-REPORT IS FROZEN BY THE COPY ABOVE, NOT BY ANY PROMISE THE AUTHOR
MADE.** The author is never told it is sealed, deliberately: a writer who knows
a document is about to be frozen writes a different document. E3 must therefore
read `stage-*-first-submission/self-report.md`, never the working copy, and the
report must diff the two and record any difference.

**E1 is read off the result, not asked for.** Record each with the file and the
line that says so:

- ★ **WHICH OF THE TWO CORRECT ENCODINGS THE AUTHOR REACHED.** Two-of-three is
  sayable twice in this contract, and BOTH are right:
  - **(i)** one `edge_guards` entry with three `conditions` and `"threshold": 2`;
  - **(ii)** three separate guarded edges to the same target, each carrying two
    of the three conditions — the contract's own *"OR is authored as MULTIPLE
    guarded edges to the same target"* sentence, applied.
  This is the finding either way. A count of "did they use `threshold`" would
  report (ii) as a failure to discover the mechanism, when it is a different
  correct reading of the same paragraph. Record WHICH, and quote the sentence
  the self-report gives as the reason.
- whether the simpler shut ways (water-stair, tally-office) are single-condition
  guards, and whether the author set a threshold on those too (a `k == len`
  normalizes to AND, so a threshold there is invisible in the read — check the
  MANIFEST as well as the map read);
- whether the author ALSO hand-wrote a shell script or called the verbs — record
  it either way, since that is what Round 943 and Round 961 both measured;
- an `edge_costs` entry, if any. Not asked for by this brief; Round 961's arm
  reached it unprompted and the count is worth carrying.

## 4. Stage B — the access corpus (runs regardless of E1)

Spawn a SECOND blind author, fresh context, same firewall, same world brief,
plus one added paragraph — still fiction, never schema, and still naming no
machinery:

> Say the lock's custom in your data in whatever way the contract provides: a
> planner reading your files alone must be able to work out that two seals open
> the strongroom and one does not, without being told so in prose.

Same first-submission freeze, same iterate-to-green, same frozen self-report.

## 5. Stage C — the disclosure axis (a SEPARATE corpus)

Spawn a THIRD blind author, fresh context, same firewall. This world needs no
map; do not reuse Stage A's brief.

> Author a short story as structured data for a story engine, using ONLY the
> authoring contract in `contract.txt` in your directory. The contract is the
> complete description of what the engine accepts; follow it exactly.
>
> The story: twelve to eighteen scenes in one house over one winter, with four
> or five people in it. One thing is true of the house from the first scene and
> the reader is not meant to hold it as true until later: the youngest son is
> not the master's child.
>
> Three separate scenes brush against it — a likeness that does not match, a
> settlement in an old ledger, a servant who stops a sentence halfway. A reader
> who has passed only ONE of those three should still be able to think they
> imagined it. It becomes theirs at the SECOND one they reach, whichever two
> those turn out to be; the third only confirms it.
>
> Say all of that in your data, not in your prose: the engine must be able to
> work out, from your files alone, both that this is true from the beginning and
> that the reader comes to hold it at the second of those three scenes.
>
> Write these files in your directory: `sections.json`, `order.json`,
> `facts.json`, and any other file the contract tells you to write. Then write
> `self-report.md`: in plain prose, which fact is the withheld one, which three
> scenes brush against it, at which of them you intend the reader to come to
> hold it, and which of your files says each. Do not run any command except
> those I give you.

Same first-submission freeze, same iterate-to-green, same frozen self-report.

**E1-C is read off the result:**

- whether a reveal trigger carries a coord SET of three and a `"threshold": 2` —
  or one coord, or three coords with no threshold (k=1, which is the FIRST scene
  and not what the fiction asked for);
- ★ **which mode the author chose.** The threshold is only visible on a
  disclosing mode; the shape Round 966 removed from the contract would store the
  `k` and render nothing. Record the mode and whether a `surface` seat was
  written. If the author reached `withhold` + a pin anyway, that is the strongest
  possible finding about the repaired paragraph and must be recorded verbatim,
  not smoothed.

## 6. Read (orchestrator, mechanical — no judging)

Verify the rules pin first (step 2). Then capture each in full.

Stages A and B:

```
"$MN" report-transition-map     --rules <the rules file the author wrote>
"$MN" report-authoring-frontier --rules <the rules file the author wrote>
"$MN" validate-continuity       --rules <the rules file the author wrote>
```

Stage C:

```
"$MN" report-playthrough-manuscript --telling <the telling the author declared>
"$MN" report-disclosure-coverage    --telling <the telling the author declared>
"$MN" report-authoring-frontier     --telling <the telling the author declared>
"$MN" validate-continuity
```

⚠ `report-playthrough-manuscript` is the witness read for the disclosure
threshold — `report-playable-world` returns `0 locator(s)` for a withheld fact
however it is pinned, so it cannot tell a correct corpus from an empty one.
`report-disclosure-coverage` is captured for its `inert reveal pin` roster,
which is the direct read on whether the Round 966 repair held.

Read the **whole** of `validate-continuity`, not just its `violations:` line. A
store can read `violations: 0` with a reject-severity class never evaluated
(Round 934); the access axis has two classes of its own —
`edge_guard_not_an_edge` and the cost's positivity check.

## 7. Compare (E3) — the discriminating step

Put the FROZEN `self-report.md` beside the reads.

For every claim of the form "the way from L1 to L2 is shut until X" — the map
read must print a guard on that edge whose conditions include the fact the
author says is X. **Check the CONDITION FACT IDS, not the place names.**

For the strongroom in particular — the self-report's prose account of the
two-of-three custom must match what the read prints, under whichever encoding
the author chose:

- under **(i)**, `[guard 2-of-3: …]` on one edge with three conditions;
- under **(ii)**, three edges to the same target, each `[guard all: …]` over a
  different pair.

A mismatch is a FINDING, not an author error: it means the store did not record
what the author believed they declared, or the contract let them believe it.

For Stage C — the manuscript read must place the withheld fact's line with
`k=2` over the three coords the self-report names. **`k=1` on a three-coord set
is the discriminating negative**: the trigger set was reached and the threshold
was not, which is a different answer from not reaching either.

Record every disagreement verbatim. Do not resolve it in the orchestrator's
voice — record it.

**The count is the number of authored corpora using each threshold, and it moves
by two at most on the access axis and one on the disclosure axis.** Report it
against the Round 966 baseline at the top of this file — both are **0**. Say the
limit plainly too: n=1 per axis premise, one lineage orchestrating. One
instance, not a distribution.

## 8. Land

- `k-of-n-report.md` — E1 (each sub-answer with its file), E1-C, E2 (all three
  first-import logs verbatim), E3 (the disagreements), and the reads in full.
- `vN/manifest.json` + `vN/replay.json` (`kit-replay/v3`) listing the three
  stages' manifests as inputs, the landing commit as `revision`, and
  `revision_provenance: "declared-at-run"`. **Both literals were read out of the
  code that panics on them** — `REPLAY_SCHEMAS` and `PROVENANCE_KINDS` in
  `crates/mnemosyne-cli/tests/evidence_replay_smoke.rs`. Round 943 inherited
  `"exact"` from a design doc and it does not exist; before writing any
  machine-checked literal into a runbook, grep the code that reads it (Round
  944).
- The declared-input roles are `replay-input`, `raw-agent-output`, and
  `run-artifact`, read from `INPUT_ROLES` in the same file. `run-artifact` may
  only be used under a declared run tree and claims less than the other two; the
  load-bearing evidence — the frozen first submissions, the sealed self-reports,
  the authored rules — belongs to the sharper roles and stays there (Round 953).
- A self-referential pin costs TWO commits by construction: a replay's revision
  must name a tree that already holds its inputs, so land the corpus first and
  pin it second. Round 961 refined this — the sealing gates want `replay.json`
  in the SAME commit as the corpus, with only the revision corrected in the
  next.
- Declare EVERY tracked file shaped like a mutate verb's input, including the
  frozen first submissions' `sections.json` — anything undeclared fails
  `every_input_a_verb_would_accept_is_declared_exactly_once`.
- Then declare and seal the REST of the run tree. **Run both FROM THE REPO ROOT
  and name the record from there** — the harness matches a record's parent
  against `git ls-files`, whose output is repo-root-relative, so a kit-relative
  name is not a tracked kit record (Round 964).
  `experiment-harness declare-run-tree --record
  claudedocs/phase1-k-of-n-authoring-experiment/vN/replay.json`, then
  `experiment-harness stamp-inputs --record <the same path>`. Both are
  idempotent and neither ever rewrites an existing entry (Round 952/953).
- Register the replay so `evidence_replay_smoke` rebuilds it in CI. Kits are
  discovered by `git ls-files`, so **stage before running the suite**, and read
  the count — the run must report MORE replays than before, or the green is
  somebody else's kit (Round 933).
- **This runbook must be tracked before the arm runs.** `/claudedocs/*` is
  ignored by default, so a kit directory needs its own `.gitignore` exception or
  the protocol the round claims to have frozen is not in the repository at all
  (Round 960 found a round that did exactly that).
- One changelog entry, one commit. Push is a separate gate.

## Out of scope, deliberately

- `parameters` / `parameter_deltas` / `parameter_gates` / `fact_counts`. Round
  959 measured zero recorded demand and wrote the resume condition — build the
  wire when a brief asks for a meter. This brief does not.
- The `undirected` branch. It also has no authored witness and Round 961 gave it
  a measured cause (every corpus chose directed in order to price a way
  differently in each direction); a premise that needs symmetry is a different
  arm.
- The render-acceptance gates. Stage C has a telling but no re-extracted prose,
  and reviving those gates is separately gated (Round 897).
- Judging the corpora's craft. There is no prose to judge.
