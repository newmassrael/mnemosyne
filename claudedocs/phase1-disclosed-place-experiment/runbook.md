# Runbook — disclosed-place-experiment/v1

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
for d in vN/run/stage-a vN/run/stage-b; do
  mkdir -p $d/docs/.atomic
  printf '[workspace]\n[continuity]\ncanon_order_path = "order.json"\n' > $d/mnemosyne.toml
  printf '{"schema_version":23,"sections":{},"changelog_entries":{}}' > $d/docs/.atomic/workspace.atomic.json
  "$MN" describe-schema > $d/contract.txt
done
"$MN" describe-schema > vN/run/contract.txt   # the tracked copy; the per-stage
                                              # ones are byte-identical and are
                                              # ignored, as in the map arms
```

**The seed names no rules file** — the Round 934 fix, carried. Six authors
across three earlier arms invented the filename and three of them mismatched a
seeded pin they were forbidden to read.

**THE RULES PIN MUST BE PATCHED BEFORE THE RENDER READ, AND THAT IS THIS ARM'S
ONE NEW HARNESS STEP.** `mnemosyne-render` takes `<workspace> <telling>` and
resolves the rules file through `mnemosyne.toml` alone — it has no `--rules`
flag, unlike every gate and report the map arms used. So after the author's
submission is frozen (step 3), and before any read in step 5:

```
printf '[workspace]\n[continuity]\ncanon_order_path = "order.json"\nrules_path = "%s"\n' \
  "<the rules file the author actually wrote>" > $d/mnemosyne.toml
```

This is orchestrator-side and invisible to the author: their sealed self-report
never claims anything about the pin, which is exactly the falsehood the Round
934 seed produced. Patching post-submission keeps ONE variable changed by this
arm. The larger alternative the map runbook left open — letting the author write
`mnemosyne.toml` themselves, making the wiring a measured capability — stays
open and is deliberately NOT taken here, for the same reason it was not taken
there.

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
Then hand the author the verbatim errors and let them iterate to green. Their
`self-report.md` is sealed at first submission and is never revised.

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

Put the sealed `self-report.md` beside the render. For every location the report
says a reader cannot place a character at until scene N:

- the place line must NOT name it in any scene before N;
- it MUST name it at or after N;
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
- `vN/manifest.json` + `vN/replay.json` (`kit-replay/v2`) listing the Stage B
  manifests as inputs, the landing commit as `revision`, and
  `revision_provenance: "exact"`.
- Register the replay so `evidence_replay_smoke` rebuilds it in CI. A corpus
  nothing loads is the rot this corpus exists to end (Round 873, Round 897).
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
