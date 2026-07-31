# Iteration notes

Both commands now exit 0:

    mn import-sections --manifest sections.json   -> 21 created, then 21 no-op
    mn import-facts    --manifest facts.json      -> 5 frames + 1 branch + 4 entity-kinds
                                                     + 1 unit + 25 entities + 7 predicates
                                                     + 97 facts created, then 140 no-op

Re-running either is a clean no-op (`written_bytes: 0`), so the loader is idempotent
by id. `self-report.md` is sealed and unrevised; everything below is the delta.

## The two errors were one defect, not two

The verbatim output showed a sections parse error and a facts validation error
(``fork point `sc-15` not present as a section``). Only the first was real. The second
was downstream of it: no sections had been imported, so every canon coordinate was
dangling and the branch's fork point was simply the first one the validator reached.
Once sections loaded, **the fact manifest imported with no change to its content** —
all 97 facts, the map, the fork, the succession chains, the payoff pair. I did not
touch a single fact to get from exit 1 to exit 0.

That is worth flagging rather than celebrating: the second error named `sc-15` and a
branch, which reads like a fork-authoring mistake. It was not. An error that points
at the last thing to fail can point away from the thing that failed.

## What I changed, and why

**1. `sections.json` shape: object -> top-level array.** I had written
`{ "sections": [ ... ] }`, mirroring the style the manifest uses for its own
registries. Wrong:

    invalid type: map, expected a sequence at line 1 column 0

The section importer wants a bare JSON array of section records. This was a shape the
contract does not fix (sections are not one of the manifest's arrays), and my
inference from the neighbouring style was simply wrong.

**2. `sections.json` records need `parent_doc`, and it is required, not optional.**

    missing field `parent_doc` at line 2 column 67

**The contract does not mention `parent_doc` anywhere.** It describes the sections
registry as the discourse-order space keyed by section id and says a fact's canon
coordinates must name an existing section; it never says a section hangs off a
document. I could only get this from the error. I set it to `workspace` on all 21,
inferred from two things inside my own directory: the sidecar the commands write is
`docs/.atomic/workspace.atomic.json`, and `mnemosyne.toml` declares a `[workspace]`
table. It was accepted and 21 sections were created. I want to be precise about the
epistemic status: **accepted is not the same as correct.** If `parent_doc` is a
registry ref, a wrong value would have failed loud; if it is free text, my 21 scenes
are now filed under a document name I guessed. I have no way to tell which from the
two commands I was given, and I did not read the store to find out.

**3. `facts.json`: added a `units` array.** This one is not a fix — the import was
already green. It is a measurement, and it falsified something my sealed report
asserts. Details below.

## What the contract got wrong (measured, not inferred)

The contract states the manifest is **"A JSON object with seven optional arrays ...
frames, branches, entity_kinds, entities, predicates, facts, disclosure_plans"**, and
says units are declared **"via add-unit before a Quantity uses it"**. My sealed
self-report leaned on that: it says no fact carries a `quantity` typed object because
I could not register a unit through the manifest, and that `edge-costs.json` needs an
`add-unit minute` call first.

The successful import's own counter line named a category the contract's list does not
contain:

    ... + 4 entity-kinds + 0 units + 25 entities + ...

So I tested it instead of leaving it as a hunch: I added
`"units": [{ "unit_id": "minute", ... }]` to `facts.json` and re-imported. Result:
`1 units created, 139 no-op`, exit 0.

**The manifest takes at least eight arrays, and `units` is one of them, keyed
`unit_id`.** The contract's "seven" is wrong, and the sealed report's claim that units
cannot be registered through the manifest is wrong with it. The unit `minute` is now
registered in the store, so the prerequisite `edge-costs.json` names in its
`units_required` field is already satisfied.

I kept the change (it costs nothing and the edge costs need that unit) but did **not**
go on to add `quantity`-typed facts. The sealed report describes a store with none,
the brief did not ask for them, and widening the gap between the sealed description
and the store to chase a nicety I had just been wrong about seemed the worse trade.
So: the report's *premise* there is falsified, its *description of the store* is still
accurate — no fact carries a quantity object.

## What exit 0 does not mean

Both commands passing is not the map being validated. `import-facts` ran the
write-time invariants — registered frame/branch/entity/predicate refs, canon and
evidence section refs, typed subject-listed, object-shape match, same-frame
succession, pays-off existence, branch forest. It did **not** run the continuity gate.

Everything `narrative-rules.json` turns on is therefore still unexercised: the
transition rule that each of the 22 `at` steps is one of the 23 declared edges in the
declared direction, the per-subject exclusivity of `at`, the per-object custody rule,
the G2 completeness/leak checks over the three quarters, `FactCanonOffBranch` on the
fork's facts, and the order's coverage of all 21 scenes. Those are
`validate-continuity` / `propose-verdict`'s job, and neither is among the two commands
I was given. The walks and the chains remain hand-verified only, exactly as the sealed
report says.

Also still unloadable by these two commands, as the sealed report already states:
`edge-costs.json` and `edge-guards.json`, which need `add-edge-cost` and
`add-edge-guard`.

## Counts, corroborated

The loader's own creation counts match the sealed report's numbers, which is the one
useful cross-check the run did provide: 21 sections, 5 frames, 1 branch, 4 entity
kinds, 25 entities, 7 predicates, 97 facts. Nothing in the report's world description
changed — 12 walkable locations, 3 quarters, 23 one-way ways over 13 passages, the
drowned mill unreachable from the bell house.
