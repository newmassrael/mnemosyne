# Runbook — map-corpus-experiment/v1

**ORCHESTRATOR-ONLY.** The authors must NEVER read this file, the design doc
(`claudedocs/map-corpus-design.md`), the changelog, or any verb output. The
design doc is the SSOT for WHY; this file is only how to run it.

**What makes this LOW-TOUCH:** there is no render, no extraction, and no judge
panel. The map axis is a structure axis — Round 892 measured the map reads
working on a store with zero prose — so the whole experiment is one or two blind
authoring passes plus mechanical reads. The human types ONE prompt.

Validity rests on the authors being blind, not on the orchestrator. The
orchestrator may know the hypothesis; it never writes a manifest, never edits
an author's output, and never puts the words *map*, *transition*, *adjacency*,
*edge*, or *graph* into a Stage A prompt.

---

## 0. START — the one prompt (paste into a FRESH session)

> You are the ORCHESTRATOR for map-corpus-experiment/v1. Read
> `claudedocs/phase1-map-corpus-experiment/runbook.md`, then execute steps 1-7.
> Spawn each author as a SEPARATE blind subagent (Agent tool, fresh context),
> passing ONLY the verbatim prompt block from this runbook plus the firewall
> wrapper — never this runbook, the design doc, the changelog, or any verb
> output. Run every command yourself. Do NOT author any manifest in your own
> voice, and do not repair an author's file: hand back the verbatim tool error
> and let them fix it. Finish by writing `map-corpus-report.md`, the
> `replay.json`, and the changelog entry. If anything is ambiguous, stop and
> ask — do not improvise the protocol.

---

## 1. Firewall — wrap EVERY author prompt with this

> Work only inside the directory `{DIR}` I give you. Use ONLY the brief below
> and the command outputs I hand you. Do NOT read any other file in this
> repository — no changelog, no design docs, no runbook, no other author's
> work, no `--list-changelog`. Save your output where the brief says. Do not
> ask why this task exists.

## 2. Setup (orchestrator)

Every command below uses one anchor so no relative depth can drift:

```
MN="$(git rev-parse --show-toplevel)/scripts/mn"
```

```
cd "$(git rev-parse --show-toplevel)/claudedocs/phase1-map-corpus-experiment"
mkdir -p vN/run/stage-a vN/run/stage-b
# seed each stage dir with a store + config, exactly as the dnd kit's rebuild does
for d in vN/run/stage-a vN/run/stage-b; do
  mkdir -p $d/docs/.atomic
  printf '[workspace]\n[continuity]\ncanon_order_path = "order.json"\n' > $d/mnemosyne.toml
  printf '{"schema_version":23,"sections":{},"changelog_entries":{}}' > $d/docs/.atomic/workspace.atomic.json
  "$MN" describe-schema > $d/contract.txt
done
```

**The seed does NOT name a rules file, and that is a fix, not an omission**
(Round 934). Arms A, B and C all seeded `rules_path = "narrative-rules.json"`
while forbidding the author to read the toml, so the filename they invented was
a coin flip: arm A's two authors happened to match, arm B's two both wrote
`rules.json`, arm C's split. Six authors, three mismatches. In arm C it did real
damage — stage A's **sealed** self-report states that the toml pins `rules.json`,
which is false, and a sealed report is the one artifact this protocol declares
unrevisable. Every arm already passed `--rules` explicitly at read time, so
naming a file in the seed bought nothing and cost that.

So: **the orchestrator passes `--rules <whatever the author actually wrote>` on
every gate and report call** (step 5). The canon order stays pinned because all
six authors wrote `order.json`, the name the brief itself gives them.

A larger alternative, deliberately NOT taken here and left for whoever designs
the next arm: let the author write `mnemosyne.toml` themselves. The contract
tells them how to wire it, so their claim about the pin would be true by
construction and the wiring would become a measured capability instead of a
trap. That changes more than one variable, which is why this round did the
smaller correct thing instead.

`contract.txt` is written **into each stage dir** because the firewall confines
the author to that directory.

`v1/run/*/docs/.atomic/*.json` is scratch (gitignored). The manifests, the
self-reports, the frozen first-submission logs, and the report are tracked
evidence.

## 3. Stage A — discoverability (measures E1)

Spawn ONE blind author with the firewall plus this brief. **The words map,
transition, adjacency, edge, graph, and route must not appear.**

> Author a small world as structured data for a story engine, using ONLY the
> authoring contract in `contract.txt` in your directory. The contract is the
> complete description of what the engine accepts; follow it exactly.
>
> The world: a flooded hill-town of eight to fourteen named locations that
> people move between — houses, a market, a shrine, a stair down to the water,
> whatever the story needs. Some locations sit inside a larger quarter. One
> descent can be taken down but not climbed back up. One place is spoken of by
> the townsfolk but nobody in the story ever reaches it. Some journeys take
> longer than others, and one way is shut until something is true. Write
> fifteen to twenty-five scenes in which people move through this town and
> something happens at each place.
>
> Write these files in your directory: `sections.json`, `order.json`,
> `facts.json`, and any other file the contract tells you to write. Then write
> `self-report.md`: in plain prose, how many locations the town has, how many
> ways lead between them, which locations cannot be reached from the start, and
> which of your files says so. Do not run any command except those I give you.

**Freeze the first submission BEFORE handing anything back** (this is E2):

```
cd v1/run/stage-a
cp -r . ../stage-a-first-submission
"$MN" import-sections --manifest sections.json > first-import.log 2>&1; echo "exit=$?" >> first-import.log
"$MN" import-facts    --manifest facts.json   >> first-import.log 2>&1; echo "exit=$?" >> first-import.log
```

Keep `first-import.log` verbatim — full text, both exit codes, no filtering.
Then hand the author the verbatim errors and let them iterate to green. Their
`self-report.md` is sealed at first submission and is never revised.

**E1 is read off the result, not asked for**: does any file the author wrote
declare a rule of class `transition`? Record YES/NO and the file.

## 4. Stage B — the corpus (runs regardless of E1)

Spawn a SECOND blind author, fresh context, same firewall, same world brief,
plus one added paragraph — still fiction, never schema:

> The engine must be able to answer, without reading your prose, which
> locations one can move to from any given location. Say that in your data in
> whatever way the contract provides.

Same first-submission freeze, same sealed `self-report.md`, same
iterate-to-green.

## 5. Read (orchestrator, mechanical — no judging)

In the Stage B directory, capture each in full:

```
"$MN" report-transition-map        --rules <the rules file the author wrote>
"$MN" report-authoring-frontier    --rules <the rules file the author wrote>
"$MN" validate-continuity          --rules <the rules file the author wrote>
"$MN" report-playable-world --telling <the telling, if one exists>
```

`--rules` is not optional here: the seed pins no rules file (step 2), and
`report-authoring-frontier` without it fails loud on the missing path rather
than reporting a map with no rules.

Read the **whole** of `validate-continuity`, not just `violations:`. Round 934
added a NOTICE for a completeness class that could not be asked, and three of
the six corpora on record trip it — a store can read `violations: 0` with a
reject-severity class never evaluated.

Then re-run the Round 896 sweep with this store as a THIRD arm (the script form
is in that round's verification) and record which verbs now differ that were
byte-identical across the two existing stores. That is C2.

## 6. Compare (E3) — the discriminating step

Put the sealed `self-report.md` beside `report-transition-map`'s node and edge
counts and the frontier's map line. Record every disagreement verbatim. A
disagreement is a FINDING, not an author error: it means the store did not
record what the author believed they declared, or the contract let them believe
it. Do not resolve it in the orchestrator's voice — record it.

## 7. Land

- `map-corpus-report.md` — E1, E2 (both first-import logs verbatim), E3 (the
  disagreements), C1/C2 (which branches ran, which verbs newly differ).
- `v1/manifest.json` + `v1/replay.json` (`kit-replay/v2`) listing the Stage B
  manifests as inputs, the landing commit as `revision`, and
  `revision_provenance: "declared-at-run"` — the first kit in this tree that can
  declare its pin at the run rather than derive it after. **That string is the
  gate's vocabulary, checked against the code and not invented here**: the
  accepted set is `["derived-upper-bound", "declared-at-run"]` in
  `crates/mnemosyne-cli/tests/evidence_replay_smoke.rs`, and an unknown value
  panics. This line said `"exact"` until Round 948; the v1 run had already hit
  that and corrected it in its own `replay.json`, but the correction never came
  back here, so Round 942 copied the retired word into a second runbook from a
  design instead of from the gate. Before writing any machine-checked literal
  into a runbook, grep the code that reads it.
- Register the replay so `evidence_replay_smoke` rebuilds it in CI. A corpus
  nothing loads is the rot this corpus exists to end (Round 873, Round 897).
- One changelog entry, one commit. Push is a separate gate.
