# Runbook — disclosed-place-experiment

**v1 RAN UNDER THE VERSION OF THIS FILE COMMITTED AT `636c91d`, NOT THIS ONE.**
That commit is the pre-commitment and the evidentiary text; read it there if you
are auditing what v1's authors were actually run against. Round 944 corrected
four instructions here against what v1 measured — the rules pin (step 2), the
self-report seal (step 3), the E3 oracle's shape (step 6), and the replay
provenance string (step 7) — so that the next arm does not inherit them. The
arms' own frozen files are untouched, which is the Round 934 precedent: fix the
runbook, never the evidence.

**ORCHESTRATOR-ONLY.** The authors must NEVER read this file, the design doc
(`claudedocs/disclosed-place-corpus-design.md`), the changelog, or any verb
output. The design doc is the SSOT for WHY; this file is only how to run it.

**What this measures.** Round 938 built a join — the place a scene DISCLOSES —
and could not witness it on authored data. The join needs two axes in ONE store:
a declared map, and a telling that decides what the reader learns. No corpus in
this tree has both. This experiment authors one, blind.

**What makes this LOW-TOUCH:** no prose, no extraction, no judge panel. The
telling is store data, so the read is `mnemosyne-render` over the author's own
workspace. The human types ONE prompt.

Validity rests on the authors being blind, not on the orchestrator. The
orchestrator may know the hypothesis; it never writes a manifest, never edits an
author's output, and never puts the words *map*, *transition*, *adjacency*,
*edge*, *graph*, *route*, *disclosure*, *telling*, *withhold*, *reveal*, or
*first_at* into a Stage A prompt.

---

## 0. START — the one prompt (paste into a FRESH session)

> You are the ORCHESTRATOR for disclosed-place-experiment/v1. Read
> `claudedocs/phase1-disclosed-place-experiment/runbook.md`, then execute steps
> 1-7. Spawn each author as a SEPARATE blind subagent (Agent tool, fresh
> context), passing ONLY the verbatim prompt block from this runbook plus the
> firewall wrapper — never this runbook, the design doc, the changelog, or any
> verb output. Run every command yourself. Do NOT author any manifest in your
> own voice, and do not repair an author's file: hand back the verbatim tool
> error and let them fix it. Finish by writing `disclosed-place-report.md`, the
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

```
cd "$(git rev-parse --show-toplevel)/claudedocs/phase1-disclosed-place-experiment"
mkdir -p vN/run/stage-a vN/run/stage-b
# Round 957 — the schema version is ASKED FOR, not typed. This line seeded the
# version as a bare 23 while the constant was 44, and a store seeded at 23
# still imports because the loader migrates, so the stale literal produced no
# error and nothing would have told the next arm. Round 944's rule ("grep the
# code that reads a machine-checked literal") applied to the one literal that
# round did not check. v1's frozen evidence is untouched.
SV="$("$MN" describe-schema | sed -n '1s/.*schema v\([0-9]\+\).*/\1/p')"
[ -n "$SV" ] || { echo "could not read the schema version from describe-schema"; exit 1; }
for d in vN/run/stage-a vN/run/stage-b; do
  mkdir -p $d/docs/.atomic
  printf '[workspace]\n[continuity]\ncanon_order_path = "order.json"\n' > $d/mnemosyne.toml
  printf '{"schema_version":%s,"sections":{},"changelog_entries":{}}' "$SV" > $d/docs/.atomic/workspace.atomic.json
  "$MN" describe-schema > $d/contract.txt
done
"$MN" describe-schema > vN/run/contract.txt   # the tracked copy; the per-stage
                                              # ones are byte-identical and are
                                              # ignored, as in the map arms
```

**The seed names no rules file** — the Round 934 fix, carried. Six authors
across three earlier arms invented the filename and three of them mismatched a
seeded pin they were forbidden to read.

**THE RULES PIN MUST BE VERIFIED BEFORE THE RENDER READ — AND v1 MEASURED THAT
IT DOES NOT NEED PATCHING.** `mnemosyne-render` takes `<workspace> <telling>`
and resolves the rules file through `mnemosyne.toml` alone — it has no `--rules`
flag, unlike every gate and report the map arms used. So before any read in step
5, check that `rules_path` names the rules file the author actually wrote:

```
grep -n rules_path $d/mnemosyne.toml   # and confirm that file exists
```

**If it already resolves, DO NOT REWRITE IT.** In v1 both authors wired
`rules_path` themselves, unprompted, 2 of 2 — the wiring turned out to be a
capability the contract teaches, not a trap the harness has to paper over. This
file previously carried a `printf` that overwrote the whole config, and applying
it would have DELETED an authored line (Stage A had added
`interval_severity = "reject"` on purpose). Overwriting an author's file is
forbidden twice elsewhere in this runbook; a convenience step does not outrank
that.

Only if `rules_path` is absent or names a file the author did not write does the
orchestrator add the single missing line — appended, never by rewriting the
file — and the report must record that it did.

The alternative the map runbook left open — letting the author write
`mnemosyne.toml` themselves, making the wiring a measured capability — **measured
itself in v1** and the answer was 2/2. It is no longer an open question.

`vN/run/*/docs/.atomic/*.json` is scratch (gitignored). The manifests, the
self-reports, the frozen first-submission logs, and the report are tracked
evidence.

## 3. Stage A — discoverability of the SECOND axis (measures E1)

Spawn ONE blind author with the firewall plus this brief. **None of the
forbidden words above may appear.**

> Author a small world as structured data for a story engine, using ONLY the
> authoring contract in `contract.txt` in your directory. The contract is the
> complete description of what the engine accepts; follow it exactly.
>
> The world: a flooded hill-town of eight to fourteen named locations that
> people move between — houses, a market, a shrine, a stair down to the water,
> whatever the story needs. Some locations sit inside a larger quarter. Write
> fifteen to twenty-five scenes in which people move through this town and
> something happens at each place.
>
> The story is told to a reader, and the reader is not told everything at once.
> One character spends the middle of the story somewhere the reader is not meant
> to work out until near the end — the scenes happen, but a reader following
> along should not be able to say where she is until the story tells them.
> Another character's movements are plain from the first scene onward. Your data
> must be able to produce BOTH readings of the same world without changing what
> is true in it.
>
> Write these files in your directory: `sections.json`, `order.json`,
> `facts.json`, and any other file the contract tells you to write. Then write
> `self-report.md`: in plain prose, how many locations the town has, how many
> ways lead between them, WHICH LOCATIONS A READER CANNOT PLACE A CHARACTER AT
> UNTIL WHICH SCENE, and which of your files says so. Do not run any command
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
MADE.** This file used to say the report "is sealed at first submission and is
never revised", which reads as a rule the author is keeping. The author is never
told it, and in v1 Stage B revised theirs in good faith after fixing the import
error. **E3 must therefore read `stage-*-first-submission/self-report.md`, never
the working copy**, and the report must diff the two and record any difference.

Telling the author it is sealed is deliberately NOT done: a writer who knows a
document is about to be frozen writes a different document, and the honest first
answer is what E3 needs. So the freeze is the mechanism and the diff is the
check — but do not call it a seal as though the author were enforcing it.

**E1 is read off the result, not asked for.** Record all four, YES/NO with the
file that says so:
- a rule of class `transition` **that declares an `adjacency` predicate** — the
  discriminating token is `adjacency`, NOT `transition`, because the transition
  class also models one-way state machines and one corpus in this tree declares
  exactly that (Round 891 excluded reachability for this reason);
- a `disclosure_plans` entry;
- per-fact overrides on the location facts, versus the plan default alone;
- typed legs on those location facts — without one the join cannot see them, and
  the contract's write-time gate already requires one for any `withhold` or
  timing pin.

## 4. Stage B — the corpus (runs regardless of E1)

Spawn a SECOND blind author, fresh context, same firewall, same world brief,
plus one added paragraph — still fiction, never schema:

> The engine must be able to answer, without reading your prose, two things for
> any scene: which locations one can move to from there, and where the reader
> has been told anyone is BY THAT POINT in the story. Say both in your data in
> whatever way the contract provides.

Same first-submission freeze, same sealed `self-report.md`, same
iterate-to-green.

## 5. Read (orchestrator, mechanical — no judging)

Patch the rules pin first (step 2). Then, in the Stage B directory, capture each
in full:

```
"$MN" report-transition-map     --rules <the rules file the author wrote>
"$MN" report-authoring-frontier --rules <the rules file the author wrote>
"$MN" validate-continuity       --rules <the rules file the author wrote>
"$MN" report-disclosure-coverage --telling <the telling the author declared>
mnemosyne-render <this stage dir> <the telling the author declared>
```

The render IS the join's read: the place line above each scene's prose is
`MapProjection::places_disclosed_in`, and this arm exists because that binary
has never been run on authored data. It cannot be, today — every corpus with a
map has no telling and the renderer fails loud on all four, and the one tracked
authored store with a telling is pre-Round-708 schema and will not load.

Read the **whole** of `validate-continuity`, not just `violations:`. A store can
read `violations: 0` with a reject-severity class never evaluated (Round 934).

## 6. Compare (E3) — the discriminating step

Put the FROZEN `self-report.md` (step 3) beside the render. **The oracle is a
triple — (character, location, scene-range) — not a location on its own.** v1's
first formulation said only "the place line must NOT name it in any scene before
N", and read strictly that manufactures a disagreement wherever the same
location is legitimately disclosed earlier for a different, told stretch: Stage
B's hidden location is the Old Cistern, and its author had also, correctly, put
that character there in the open at scene 4.

So for every claim of the form "a reader cannot place character C at location L
until scene N":

- no place line attributable to C at L may appear before N — check the FACT that
  disclosed it (`DisclosedPlace.fact_id`), not the place name alone, because
  another character standing in the same room is not a leak;
- it MUST appear at or after N. If it does not, do not write it off as a near
  miss: check whether the withheld fact's extent has already ended at N, which
  in v1 was true in both stores and is finding 3 of the report;
- and the plainly-moving character's places must appear from their first scene.

This is the authored analogue of the built pair Round 938 pinned (the same fact,
disclosed and withheld, everything else identical), and it is discriminating in
the direction that matters: a join that leaked would show the hidden place
early, and a join that read the gate's ground truth instead of the telling would
show it in EVERY scene.

Record every disagreement verbatim. A disagreement is a FINDING, not an author
error: it means the store did not record what the author believed they declared,
or the contract let them believe it. Do not resolve it in the orchestrator's
voice — record it.

Record separately, because it is a known interaction and not a defect: a place
carrying no edge is not a node of the map, so the join cannot name it even when
a line discloses it. If the author wrote a spoken-of-but-never-reached location
off the map, that is `map_invented_place` at the frontier AND a silent absence
here.

## 7. Land

- `disclosed-place-report.md` — E1 (all four sub-answers), E2 (both first-import
  logs verbatim), E3 (the disagreements), and the render output in full.
- `vN/manifest.json` + `vN/replay.json` (`kit-replay/v3`) listing the Stage B
  manifests as inputs, the landing commit as `revision`, and
  `revision_provenance: "declared-at-run"`. **That string is the gate's
  vocabulary, checked against the code and not invented here**: the accepted set
  is `["derived-upper-bound", "declared-at-run"]` in
  `crates/mnemosyne-cli/tests/evidence_replay_smoke.rs`, and an unknown value
  panics. This file asked for `"exact"` until v1 ran; Round 898's design used the
  same non-existent word and the map-corpus run corrected it at execution time,
  and the correction was never carried back into a design, so it came round
  again. **Before writing any machine-checked literal into a runbook, grep the
  code that reads it.**
- A self-referential pin costs TWO commits by construction: a replay's revision
  must name a tree that already holds its inputs, so land the corpus first and
  pin it second.
- Declare EVERY tracked file shaped like a mutate verb's input, including the
  frozen first submissions' `sections.json` — anything undeclared fails
  `every_input_a_verb_would_accept_is_declared_exactly_once`.
- Then declare and seal the REST of the run tree, which is not optional and is
  not hand work (Round 953): `experiment-harness declare-run-tree --record
  vN/replay.json` writes a `run-artifact` entry for every tracked file under
  `vN/run/` that the record does not already name, and `experiment-harness
  stamp-inputs --record vN/replay.json` then writes each declared input's
  sha256 into the record, once. Both are idempotent and neither ever rewrites an
  existing entry. Until this is run, the manuscripts, the judge reports, the
  label map and the captured logs are pinned by nothing at all — which is the
  state the whole corpus was in until Round 953 measured it (425 of 552).
- Register the replay so `evidence_replay_smoke` rebuilds it in CI. A corpus
  nothing loads is the rot this corpus exists to end (Round 873, Round 897).
  Kits are discovered by `git ls-files`, so **stage before running the suite**,
  and read the count — the run must report MORE replays than before, or the
  green is somebody else's kit (Round 933).
- One changelog entry, one commit. Push is a separate gate.

## Out of scope, deliberately

- The render-acceptance gates (`validate-disclosure-leak` /
  `validate-render-fidelity`). They read re-extracted PROSE; this corpus has
  none, and reviving them is separately gated (Round 897).
- Judging the corpus's craft. There is no prose to judge.
- Per-world maps (Round 696 finding 6 stands on its own merit).
- Deciding who the player is. The kernel returns every disclosed place and
  Round 938 left the viewpoint unowned; this arm measures the read, not that
  question.
