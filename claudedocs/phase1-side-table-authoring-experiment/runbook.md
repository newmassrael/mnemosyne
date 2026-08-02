# Runbook — side-table-authoring-experiment

**ORCHESTRATOR-ONLY.** The authors must NEVER read this file, the changelog, the
design docs, any other author's work, or any verb output beyond what is handed
to them. This file is only *how to run it*.

**What this measures.** The tables were verb-only, so a file-only authoring —
which is how the corpora on record were written — could not reach them. Round
956 built the manifest wire; Round 957 found the contract still *taught the
retired model* in the paragraph an author lands on, and fixed it.

**The baseline, re-measured at Round 959 and NOT what Round 936 recorded.**
Round 936 counted the recorded transition-map logs and concluded both tables had
no authored witness. That instrument was map-keyed, so it could not see a store
holding guards WITHOUT a map. Re-measured across all 97 stores on disk:

| table | authored corpora on record | where |
|---|---|---|
| `edge_costs` | **0** | — |
| `edge_guards` | **1** | `phase1-2d-projection-experiment/v1` (R750), two guards in-store |

Five distinct authoring arms (map-corpus v2 stage-a/b, v3 stage-b, v4 stage-a/b)
hand-wrote a script calling these verbs and none of their calls reached a store —
v4 stage-a's script says so in its own header: *"NOT RUN by the authoring session
— no command was handed to it."* Round 943's arm wrote no script at all; earlier
drafts of this file attributed the scripts to it, and that was wrong.

So the substrate question is closed and the AUTHORING question is open: with the
wire built and the document corrected, does an author reach these tables? The
count above is the oracle, and it only moves if a corpus is authored.

**Why an arm that is told nothing is worth running.** Round 956's carry says the
next brief "has to name the slots". Round 943 measured the opposite for the
disclosed-place axis: the Stage A brief named neither capability and the author
reached both, quoting the contract's own reasoning. Before Round 957 the
discoverability question here was not merely unanswered but structurally
foreclosed — the contract said in plain words that a manifest could not do it.
That sentence is gone, so the question is open for the first time. Stage A asks
it; Stage B guarantees the corpus regardless of Stage A's answer.

**What makes this LOW-TOUCH:** no prose, no extraction, no judge panel. The read
is `report-transition-map` over the author's own workspace. The human types ONE
prompt.

Validity rests on the authors being blind, not on the orchestrator. The
orchestrator may know the hypothesis; it never writes a manifest, never edits an
author's output, and never puts the words *cost*, *guard*, *edge*, *side table*,
*threshold*, *condition*, *K-of-N*, *manifest array*, *verb*, `edge_costs`, or
`edge_guards` into a Stage A prompt.

---

## 0. START — the one prompt (paste into a FRESH session)

> You are the ORCHESTRATOR for side-table-authoring-experiment/v1. Read
> `claudedocs/phase1-side-table-authoring-experiment/runbook.md`, then execute
> steps 1-7. Spawn each author as a SEPARATE blind subagent (Agent tool, fresh
> context), passing ONLY the verbatim prompt block from this runbook plus the
> firewall wrapper — never this runbook, the changelog, any design doc, or any
> verb output. Run every command yourself. Do NOT author any manifest in your
> own voice, and do not repair an author's file: hand back the verbatim tool
> error and let them fix it. Finish by writing `side-table-report.md`, the
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

**The seed's schema version is DERIVED, never typed in.** The runbook this one
is shaped after seeded the version as a bare 23; the constant had moved on by
Round 957, and a store seeded at 23 still imports because the loader migrates —
so the stale literal produces no error and nothing would have told the next arm.
This is Round 944's rule applied to the one literal that arm did not check: ask
the program.

```
cd "$(git rev-parse --show-toplevel)/claudedocs/phase1-side-table-authoring-experiment"
mkdir -p vN/run/stage-a vN/run/stage-b
SV="$("$MN" describe-schema | sed -n '1s/.*schema v\([0-9]\+\).*/\1/p')"
[ -n "$SV" ] || { echo "could not read the schema version from describe-schema"; exit 1; }
for d in vN/run/stage-a vN/run/stage-b; do
  mkdir -p $d/docs/.atomic
  printf '[workspace]\n[continuity]\ncanon_order_path = "order.json"\n' > $d/mnemosyne.toml
  printf '{"schema_version":%s,"sections":{},"changelog_entries":{}}' "$SV" > $d/docs/.atomic/workspace.atomic.json
  "$MN" describe-schema > $d/contract.txt
done
"$MN" describe-schema > vN/run/contract.txt   # the tracked copy
```

**The seed names no rules file** — the Round 934 fix, carried, and Round 943
measured that it needs no patching afterwards either: 2 of 2 authors wired
`rules_path` into `mnemosyne.toml` themselves, unprompted. Before the read in
step 5, CHECK that it resolves:

```
grep -n rules_path $d/mnemosyne.toml   # and confirm that file exists
```

**If it already resolves, DO NOT REWRITE IT.** Overwriting an author's file is
forbidden twice in this runbook; a convenience step does not outrank that, and
in Round 943 the literal `printf` this instruction used to carry would have
deleted an authored `interval_severity` line. Only if `rules_path` is absent or
names a file the author did not write does the orchestrator APPEND the single
missing line, and the report must record that it did.

`vN/run/*/docs/.atomic/*.json` is scratch (gitignored). The manifests, the
self-reports, the frozen first submissions, the reads and the report are tracked
evidence.

## 3. Stage A — discoverability (measures E1)

Spawn ONE blind author with the firewall plus this brief. **None of the
forbidden words above may appear.** The fiction must make both properties
NECESSARY without naming any machinery.

> Author a small world as structured data for a story engine, using ONLY the
> authoring contract in `contract.txt` in your directory. The contract is the
> complete description of what the engine accepts; follow it exactly.
>
> The world: a terraced coastal town of eight to fourteen named locations that
> people move between. Write fifteen to twenty-five scenes in which people move
> through it and something happens at each place.
>
> Two things about this town matter as much as its layout. First, getting
> around it is not uniform: the stair to the upper terrace is a long climb, the
> lane between the two market rows is a few steps, and a messenger planning a
> route would care about the difference. Second, not every way is open to
> everyone at all times — the sea-gate can only be crossed when the water is
> low, and the counting-house door needs a key one character carries. Someone
> planning a route must be able to tell which ways are shut to them and why.
>
> The engine must be able to answer both of those from your data alone, without
> reading your prose.
>
> Write these files in your directory: `sections.json`, `order.json`,
> `facts.json`, and any other file the contract tells you to write. Then write
> `self-report.md`: in plain prose, how many locations the town has, how many
> ways lead between them, WHICH WAYS COST MORE THAN OTHERS, WHICH WAYS ARE SHUT
> AND WHAT OPENS THEM, and which of your files says each. Do not run any command
> except those I give you.

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
report must diff the two and record any difference (Round 943 recorded a
good-faith revision).

**E1 is read off the result, not asked for.** Record each YES/NO with the file
and the line that says so:

- an `edge_costs` entry **in the fact manifest** — the discriminating fact is
  the FILE, not the table: reaching it by calling `add-edge-cost` by hand is the
  Round 943 outcome, and it means the wire did not carry the author;
- an `edge_guards` entry in the fact manifest, with `conditions` naming real
  condition facts;
- whether a `threshold` was used at all, and if so whether it is `1..=len`;
- whether the author ALSO hand-wrote a shell script or called the verbs — record
  it either way, since that is precisely what Round 943 measured twice.

## 4. Stage B — the corpus (runs regardless of E1)

Spawn a SECOND blind author, fresh context, same firewall, same world brief,
plus one added paragraph — still fiction, never schema, and still naming no
machinery:

> Say both of those in your data in whatever way the contract provides: a
> planner reading your files alone must be able to total up how long a route
> takes, and to see that a way is shut without being told in prose which one.

Same first-submission freeze, same iterate-to-green, same frozen self-report.

## 5. Read (orchestrator, mechanical — no judging)

Verify the rules pin first (step 2). Then, in EACH stage directory, capture each
in full:

```
"$MN" report-transition-map     --rules <the rules file the author wrote>
"$MN" report-authoring-frontier --rules <the rules file the author wrote>
"$MN" validate-continuity       --rules <the rules file the author wrote>
```

`report-transition-map` IS the witness read: a cost prints on its edge as
`<n> <unit>` and a guard as `[guard all: …]` or a K-of-N form. Round 957
verified this read on a scratch store before the experiment existed, so a blank
here is the corpus's answer and not an untested projection.

Read the **whole** of `validate-continuity`, not just `violations:`. A store can
read `violations: 0` with a reject-severity class never evaluated (Round 934),
and this axis has two classes of its own — `edge_guard_not_an_edge` and the
cost's positivity check.

## 6. Compare (E3) — the discriminating step

Put the FROZEN `self-report.md` beside the map read. For every claim of the form
"the way from L1 to L2 costs more than the way from L3 to L4":

- the map read must print a cost on both edges, and the ordering must match the
  author's prose;
- a cost the author described and the map does not print is a FINDING, not an
  author error: it means the store did not record what the author believed they
  declared, or the contract let them believe it.

For every claim of the form "the way from L1 to L2 is shut until X":

- the map read must print a guard on that edge whose conditions include the fact
  the author says is X — check the CONDITION FACT IDS, not the place names.

Record every disagreement verbatim. Do not resolve it in the orchestrator's
voice — record it.

**Record separately, because it is a known interaction and not a defect:** a
guard is checked to sit on a map EDGE, so a guard the author put on a non-edge
fact is `edge_guard_not_an_edge` at the gate rather than a silent absence here.

**The count is the number of authored corpora using each table, and it moves by
two at most.** Report it against the Round 959 baseline at the top of this file —
`edge_costs` 0, `edge_guards` **1**, not 0 — so a report reading "0 to 2" for
guards would be wrong by a corpus. Say the limit plainly too: n=2, one world
premise, one lineage orchestrating. One instance, not a distribution.

## 7. Land

- `side-table-report.md` — E1 (each sub-answer with its file), E2 (both
  first-import logs verbatim), E3 (the disagreements), and the map reads in full.
- `vN/manifest.json` + `vN/replay.json` (`kit-replay/v3`) listing the Stage A and
  Stage B manifests as inputs, the landing commit as `revision`, and
  `revision_provenance: "declared-at-run"`. **Both literals were read out of the
  code that checks them** — `REPLAY_SCHEMAS` and `PROVENANCE_KINDS` in
  `crates/mnemosyne-cli/tests/evidence_replay_smoke.rs`, where an unknown value
  panics. Round 943 inherited `"exact"` from a design doc and it does not exist;
  before writing any machine-checked literal into a runbook, grep the code that
  reads it (Round 944).
- The declared-input roles are `replay-input`, `raw-agent-output`, and
  `run-artifact`, read from `INPUT_ROLES` in the same file. `run-artifact` may
  only be used under a declared run tree and claims less than the other two;
  the load-bearing evidence — the frozen first submissions, the sealed
  self-reports, the authored rules — belongs to the sharper roles and stays
  there (Round 953).
- A self-referential pin costs TWO commits by construction: a replay's revision
  must name a tree that already holds its inputs, so land the corpus first and
  pin it second.
- Declare EVERY tracked file shaped like a mutate verb's input, including the
  frozen first submissions' `sections.json` — anything undeclared fails
  `every_input_a_verb_would_accept_is_declared_exactly_once`.
- Then declare and seal the REST of the run tree, which is not optional and is
  not hand work: `experiment-harness declare-run-tree --record vN/replay.json`
  writes a `run-artifact` entry for every tracked file under `vN/run/` the
  record does not already name, and `experiment-harness stamp-inputs --record
  vN/replay.json` writes each declared input's sha256 once. Both are idempotent
  and neither ever rewrites an existing entry (Round 952/953).
- Register the replay so `evidence_replay_smoke` rebuilds it in CI. Kits are
  discovered by `git ls-files`, so **stage before running the suite**, and read
  the count — the run must report MORE replays than before, or the green is
  somebody else's kit (Round 933).
- One changelog entry, one commit. Push is a separate gate.

## Out of scope, deliberately

- `parameters` / `parameter_deltas` / `parameter_gates` / `fact_counts`. They are
  still verb-only and this arm does not ask whether they should be wired; that
  question is the same one that made the edge tables worth wiring, and it is a
  measurement, not this experiment.
- The disclosure axis. This corpus needs no telling, and adding one would
  confound the witness question with Round 943's still-open findings.
- The render-acceptance gates. They read re-extracted PROSE; this corpus has
  none, and reviving them is separately gated (Round 897).
- Judging the corpus's craft. There is no prose to judge.
