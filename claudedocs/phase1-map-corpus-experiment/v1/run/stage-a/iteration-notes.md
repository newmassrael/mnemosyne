# Iteration notes

`self-report.md` is sealed and unrevised. Everything the loader changed or
contradicted is recorded here instead.

Final state: `import-sections` exit 0 (22 no-op), `import-facts` exit 0 (171
no-op). Both are idempotent — a second run creates nothing and writes 0 bytes.

## What I had to change

**1. `sections.json` was the wrong container shape.**

> `error: parse manifest sections.json (JSON array of section imports): invalid type: map, expected a sequence at line 1 column 0`

I had written `{ "sections": [ … ] }`, following the fact manifest's convention
of an object of named arrays. The sections manifest is a **bare JSON array** of
section objects. Changed to a top-level `[ … ]`.

The contract does not give a wire format for sections at all — it names the
registry and the `add-section` primitive and stops. My self-report flagged this
as a guess, and the guess was wrong in both of its halves (see 2).

**2. A section requires a `parent_doc` field that the contract never mentions.**

> `error: parse manifest sections.json (JSON array of section imports): missing field 'parent_doc' at line 2 column 82`

`section_id` and `title` are not sufficient. Every section needs `parent_doc`.
Nothing in `contract.txt` mentions a document above the section — the section
registry is described only as "the structure / discourse-order space (chapters,
scenes)", and the medium-neutral structural unit is the section. There is no
documents registry, no `add-doc`, and no `parent_doc` in any wire format or
invariant listed.

The loader accepted `"ossary-rise"`, a string naming nothing that exists. So I
still cannot tell from the loader whether `parent_doc` is meant to reference a
registered document that I was supposed to create first, or whether it is a free
label. It validated no more than its presence. All 22 sections carry
`"parent_doc": "ossary-rise"`.

**3. The second error in the first run was a cascade, not a second defect.**

> `error: validation: branch 'sluice-open': fork point 'sc-12' not present as a section (canon coordinates are structure refs)`

This reads as a branch or canon-order defect. It was not: the facts manifest was
correct as authored. `sc-12` was "not present as a section" only because the
sections import had failed on the shape error above, so the section registry was
empty and *every* coordinate in the manifest was equally absent — the branch fork
point is just the first thing checked. Fixing `sections.json` fixed it with no
change to `facts.json`, `order.json` or `narrative-rules.json`.

It does teach one write-time invariant the contract does not list: a branch's
`forks_at` is checked against the sections registry at write time. The contract's
"write-time invariants" section covers `branch-forest` (fork XOR confluence,
parents pre-registered) and `evidence-provenance` (canon_from / canon_to /
evidence must name existing sections) but never says the fork point itself is a
checked section ref.

**4. `units` IS a manifest array. My self-report's first gap is false.**

The contract states the manifest has "seven optional arrays … frames, branches,
entity_kinds, entities, predicates, facts, disclosure_plans", and says a unit is
"declared via add-unit before a Quantity uses it". On that basis `self-report.md`
concludes that no `quantity` object is authorable from files at all, and encodes
travel times as a closed token vocabulary instead.

That conclusion is wrong. The loader's own success tally names **eight**
categories:

    3 frames + 1 branches + 5 entity-kinds + 0 units + 31 entities
      + 7 predicates + 116 facts + 0 disclosure-plans + 0 disclosure-overrides

`0 units` in a tally for a manifest that has no units array is the tell. I tested
it the only way that cannot lie about it — by authoring the thing the gap claimed
was impossible — and added to `facts.json`:

- `"units": [{ "unit_id": "minute" }]`
- a `travel_minutes` predicate with `object_kind: "quantity"`
- six facts carrying `{ "kind": "quantity", "n": <int>, "unit": "minute" }`

The import created `1 units + 1 predicates + 6 facts`. So units are file-declared,
and Quantity objects load from a manifest. Fifty minutes up the Reeve's Steps,
forty-five wading the lane, three hundred and sixty for the round back up to the
Cistern Yard.

Consequence I could not clean up: the six `crossing_time` token facts were a
workaround for a gap that does not exist, and they were already in the store
before I found that out. The ledger is append-only and `retract-fact` is not one
of the two commands I have, so both readings now coexist — a coarse token
(`short` / `long` / `half-a-day`) and an exact minute count, on the same six
passages. They agree, but the token one is redundant and would not have been
written had the contract's array list been complete.

Counts in the sealed self-report that this changed: facts 116 → 122, predicates
7 → 8, units 0 → 1. The counts the report is actually *about* did not change —
13 locations, 29 `adjacent` facts / 15 passages, the Undertown unreachable. The
map loaded as authored on the first fact-manifest run.

## The thing that would have made 4 self-correcting

**Unknown manifest keys are silently ignored.** I ran a manifest whose only key
was `{ "edge_costs": [] }`:

    0 frames + 0 branches + … + 0 facts …, 0 no-op
    written_bytes: 0
    exit=0

No error, no warning, no mention of `edge_costs` — exit 0. So the manifest is not
fail-loud on keys it does not know, and a wrong guess about its surface produces
silence rather than a correction. Had it rejected unknown keys the way the rules
parser does ("the parser is fail-loud on unknown or class-mismatched legs"), the
error would have enumerated the accepted keys and I would have found `units`
before authoring the token workaround.

It also means I **cannot** confirm or refute the other two gaps in the sealed
report with the commands I have. `edge_costs`, `edge_guards`, `parameters`,
`parameter_deltas`, `parameter_gates` and `fact_counts` are absent from the
loader's tally, and an unknown key is silent, so "not supported" and "supported
but empty" produce byte-identical output here. The report says those side-tables
are not file-authorable; after this run that claim is *unverified*, not
confirmed — and given that `units` was in the same category and turned out to be
authorable, the prior should be against it. Confirming it needs a command I was
not given (`report-transition-map` would show whether a cost rode the edge).

## Store side effect I could not undo

There is a stray section `sc-probe` in the store, titled `probe`, with
`parent_doc: ossary-rise`. I created it while probing what `parent_doc` would
accept.

It is not part of Ossary Rise: `order.json` does not place it, no fact names it
as `canon_from` or evidence, and `sections.json` does not contain it. It will
show up as an unplaced section in any frontier report. I could not remove it —
deletion is not one of my two commands, and `docs/` is not mine to edit.

The mistake was probing at all. `import-sections` requires `mnemosyne.toml` in
the working directory or an ancestor, so a probe cannot be aimed at a throwaway
store from here; every probe writes to the real one. The fix for `parent_doc` was
identical whether I discovered it from a scratch file or from my own manifest, so
the probe bought nothing and cost a stray row. The later `edge_costs` probe was
safe only by luck of being empty.

## What exit 0 here does and does not certify

`import-facts` runs the write-time invariants — registered frames, branches,
entities, predicates; section refs on `canon_from`, `canon_to`, evidence and the
fork point; typed subject/object listing and object-shape match; same-frame
succession; `pays_off` targets; conflict-hash computation. All of those pass.

It is not the continuity gate. `validate-continuity` is a separate command and I
was not given it, so nothing here has evaluated the three rules in
`narrative-rules.json`, and the risks named at the end of my first report remain
open and unmeasured:

- **G2 completeness** over the wired containment predicate — whether the
  edgeless Undertown counts as "a node or a container" by being contained by
  Tidefoot, or whether "node" means an adjacency endpoint, in which case the one
  location the brief wanted unreachable is a gate violation.
- **The coarse/fine `at` pair** for Sabb (`at(Sabb, the Crown)` overlapping
  `at(Sabb, the Reeve's Hall)`) — whether refinement-awareness through
  `contains` recognises them as one place, as the contract's classroom/school
  case says it should.
- **The unchained coarse fact** — the same fact sits on no succession edge, so
  the transition rule should surface it as an unchained same-subject pair. That
  is a surfacing and not a reject, and it is a cost I took deliberately to
  exercise the containment leg.
