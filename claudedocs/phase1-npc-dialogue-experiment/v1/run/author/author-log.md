# Author log — "Wreckfall at Cawdy Holm" (Stage 1 fact base)

A gate-checked branching-story bible authored as facts with `mnemosyne-cli`.
Method was top-down: scope + skeleton (scenes, frames, wreck-night truth,
endings, fork, load-bearing objects, setups+payoffs) made gate-clean first,
then detail fill that keeps every road's aftermath peopled.

## Scope

- 32 scenes. Shared spine `sc-01..sc-15`; fork at **sc-15** (15/32 ≈ 47%, the
  40–50% mark). Three terminal world-lines, each its own tail + ending:
  - **report** `sc-16..sc-21` (6 tail scenes) — Halsa lays it before the Crown.
  - **shelter** `sc-22..sc-27` (6 tail scenes) — the wreck stands as the sea's.
  - **confront** `sc-28..sc-32` (5 tail scenes) — Halsa settles it on the shore.
- 135 facts total: 20 ground-truth (`gt`) + spine person-frame facts +
  27 report / 23 shelter / 24 confront branch facts + 10 typed possession facts.

## The cast of frames (13 frames: ground truth + 12 people)

- `gt` — the ground truth.
- `halsa` — Halsa Crewe, keeper's child, POV.
- `bryde` — the old keeper, abed with the broken leg.
- `officer` — the revenue riding-officer (Crown).
- `mate` — Cass Pellow, the surviving mate.
- `passenger` — Ysolt Marran, with the locked chest.
- `headman` — Orne Veck, net-master, speaks for the Holm.
- `saltwife` — Dunna Quick, first down at the ebb.
- `pilot` — Maon Skerry, the old pilot.
- `boy` — Wick, who found the first thing on the shore.
- `factor` — Pell Garrow, the factor's man, stranded.
- `curer` — Eda Lay, the fish-curer.
- `girl` — Senna, betrothed to Wick.

Twelve person-frames (≥10 required), each carrying exactly its own road.

## The wreck-night truth (ground-truth frame), authored first

- The *Marran* was **lured** onto the Sneck by a **false light** shown from the
  Ness — NOT weather alone (f-001).
- **Orne Veck the headman** carried the lantern to the Ness and showed the false
  light, to bring cargo to a starving Holm (f-002). He also **doused the Holm's
  own true light** so the false one would not be answered (f-003).
- **Bryde Crewe** broke a leg on the light-stair that night climbing to relight
  the doused light (f-004) — the keeper's injury traced to a placed cause.
- The supercargo did **not** simply drown: **the mate Cass Pellow held him under**
  to take the bill of lading he carried (f-005, f-006).
- The chest holds a **contested deed** (inheritance papers), not treasure (f-007);
  the passenger Ysolt is the dead man's kin, sailing to lodge her claim.
- One cask of brandy went quietly up the Holm — the count is **one cask short**
  (f-008). The salt-wife Dunna took it on the headman's word.
- The pilot Maon **did not** show the false light, though he alone could read what
  a Ness light does (f-009, f-010) — a true-but-suspected innocent.

## Endings (authored before the middle)

- **report**: the lantern + short count + chest's deed convict the Holm; the mate
  runs and is caught with the lading-bill (Halsa learns the killing here); the
  cutter takes the headman and the mate to the Crown. The Holm lives on shut and
  shamed (f-216..f-224).
- **shelter**: the wreck is written the sea's; the headman buries the lantern, the
  passenger bargains her chest closed, the children are bound to silence, the
  count is never righted. The Holm whole but no longer clean (f-314..f-322).
- **confront**: Halsa calls the Holm to the bar, names the lantern, the headman
  owns the false light, the passenger opens her own chest, the mate breaks and
  owns the killing, the count is made whole by the Holm's own hand; the island
  answers the cutter having judged its own (f-413..f-423).

## Load-bearing objects + the exclusive possession rule

`narrative-rules.json` declares one rule (corrected to the real wire schema —
`{"id","predicate","class":"exclusive","per":"object"}`, NOT the brief's
illustrative `kind` field):

- **lantern** (the false light): `gt` possession chain
  `headman (f-501, sc-01) -> boy (f-502, sc-04) -> halsa (f-503, sc-14)`, then
  per road: `-> officer (f-506, report sc-18)` / `-> headman again (f-509,
  shelter sc-23, buried)`. Each transfer uses `supersedes_in_frame` on the prior
  holder, in-frame (`gt`).
- **chest**: `passenger (f-504, sc-07) -> officer (f-508, report sc-17)`.
- **shortcask**: `saltwife (f-505, sc-08) -> officer (f-507, report sc-18)` /
  `-> factor (f-510, confront sc-31, count made whole)`.

`validate-continuity --rules` reports `unchained_state_pairs=0` — no impossible
co-holding; all hand-offs accepted.

## Disclosure plan (sparse "telling" `holm`, default withhold)

Two load-bearing solution facts withheld, both TYPED (gate needs typed reveals):

- **f-002** (false light = the headman): `first-at report=sc-18, shelter=sc-23,
  confront=sc-29`.
- **f-005** (the killing = the mate): `first-at report=sc-19, confront=sc-30`.
  Deliberately **no shelter first-at** — on the shelter road the killing is never
  revealed (it stays a buried truth), which the plan permits.

`report-disclosure-coverage --telling holm` => `hidden_by_design=2,
never_planned=133` — the two reveals registered, the plan sparse.

## Setups -> payoffs (two, both substantiated, one private-knowledge each)

1. **f-108** (the tally's last line logs the Holm light OUT at the wreck hour —
   private knowledge in Bryde's hand, read only by Halsa) -> paid by **f-135**
   (Halsa learns the light was doused by a hand). Typed on predicate `lit`
   (`logged-out-at-wreck-hour` -> `doused-by-a-hand`) => substantiated.
2. **f-110** (the boy Wick found a dry shuttered lantern on the bar — knowledge
   only the boy held) -> paid by **f-141** (Halsa takes it from the net-loft and
   sees it is Holm-work). Typed on predicate `whereabouts`
   (`found-dry-on-the-bar` -> `taken-into-halsa-keeping`) => substantiated.

Both setups pay off in EVERY terminal world (the discharging payoffs f-135/f-141
sit on the shared spine) — `report-payoff-coverage` dangling=0 in all worlds,
`report-payoff-substantiation` substantiated=2 / unverifiable=0 in all worlds.

## Structural backreferences (evidence arrays, not prose)

Every callback cites its establishing scene structurally, e.g. f-135
`evidence:["sc-12","sc-03"]` (the tally line), f-141 `["sc-14","sc-13"]`,
f-201 `["sc-16","sc-14","sc-03"]`, f-404 `["sc-29","sc-14","sc-04"]` (the boy's
find). No backreference cites a later or off-branch scene — `evidence_unreachable`
never fired.

## Keeping the Holm peopled to the last scene of every road

Each road's tail carries multiple distinct frames per scene; the FINAL aftermath
scene of every road holds 5–6 distinct people (verified via the manuscript +
frame greps):

- **sc-21** (report aftermath): bryde, factor, halsa, passenger, saltwife.
- **sc-27** (shelter aftermath): boy, bryde, curer, halsa, headman, passenger.
- **sc-32** (confront aftermath): boy, curer, factor, halsa, officer, passenger.

The secondary people (salt-wife, boy, pilot, curer, factor, headman, girl, bryde)
each carry a fact into each road's tail — the house does not empty out to two
principals.

## Write -> gate -> repair iterations

**5 import/repair iterations** before all gates were clean:

1. **import-facts #1** — REJECTED: I had left `"=== ... ==="` comment strings in
   the `facts` array. The parser fail-louds (`invalid type: string ... expected
   struct FactImport`). Fix: removed all 24 comment strings (atomic import wrote
   nothing, so no partial state).
2. **import-facts #2** — REJECTED: `supersedes_in_frame` given as an array
   (`["f-501"]`); the field is `Option<String>` (one prior per fact). Fix: scalar
   strings. (Confirmed against `continuity.rs` / `main.rs` field type.)
3. **import-facts #3** — REJECTED: f-001/f-005 typed `possession` with a scalar
   `value` object, but `possession` is declared `object_kind=entity` (shape
   mismatch). Fix: added a `cause` predicate (`object_kind=scalar`) for the two
   solution states; possession stays entity-only.
4. **import-facts #4** — REJECTED: branch possession transfers (f-208, f-210,
   f-304, f-412) carried `supersedes_in_frame` into a `gt`-frame chain from a
   person frame — the gate enforces **in-frame succession only** (cross-frame is
   data, not succession). Fix: split possession out of the person-frame narrative
   facts into dedicated `gt`-frame transfer facts (f-506..f-510), each tagged with
   its branch and superseding the spine `gt` holder.
5. **import #5 + first full gate pass** — IMPORT CLEAN. `validate-continuity`
   0/0, fork-tree 3 world-lines / 0 unplaced, timeline-gaps 0 in all worlds,
   playthrough 0 unplaced / 0 undecidable in all worlds. But:
   - `narrative-rules.json` first written with the brief's illustrative
     `{"kind":"exclusive",...}` — REJECTED (`unknown field kind`). Fixed to the
     real wire schema (`id`/`predicate`/`class`/`per`). Note: `--rules` resolves
     from repo root, so it is passed as an absolute path; `--sidecar`/`--order`
     resolve from cwd as the brief says.
   - `report-payoff-coverage` flagged **9–10 DANGLING setups** per terminal world:
     I had over-marked `payoff_expectation:"expected"` on many ground-truth /
     establishing facts that nothing discharges. Fix: stripped the marker from
     f-001..f-008, f-123, f-126; kept it only on the two genuine setups f-108 and
     f-110. Re-coverage: dangling=0 everywhere.
   - A residual `[payoff->unmarked] f-207/f-403 -> f-126` advisory (payoffs still
     pointing at the now-unmarked f-126). Fix: removed those two `pays_off`
     references (the structural `evidence:[...,"sc-09"]` backreference stays).
   - `report-payoff-substantiation` reported the two setups `unverifiable
     (untyped)`. Optional strengthening: typed f-108/f-135 (`lit`) and
     f-110/f-141 (`whereabouts`) so each setup's typed state is discharged by a
     typed state-change => substantiated=2 in all worlds.

## A gate catching a real problem

Iteration 4 is the substantive catch: I had tried to move the lantern/cask
through person-frame facts (officer "takes" the lantern in the officer frame).
The continuity gate rejected cross-frame succession — correctly, because "the
officer holds the lantern" is a fact about the WORLD (ground truth), not the
officer's private belief, and the one-holder-at-a-time chain must live in one
frame to be enforceable. The fix (dedicated `gt` possession-transfer facts)
made the exclusive rule actually bite while leaving each person's BELIEF facts
in their own frame. The possession pivots and the belief frames are now cleanly
separated.

## Final clean status of every gate the brief lists

```
validate-continuity:        violations: 0 (structural=0 interval=0)   rules=1 unchained_state_pairs=0
report-fork-tree:           3 registered world-line(s), 0 unplaced fork point(s); fork at sc-15
report-timeline-gaps:       world report/shelter/confront: violated=0 unverifiable=0
report-payoff-coverage:     report/shelter/confront/main: paid=2 dangling=0 unknown=0
report-payoff-substantiation: report/shelter/confront/main: substantiated=2 unsubstantiated=0 unverifiable=0
report-disclosure-coverage: disclosed=0 hidden_by_design=2 never_planned=133  (f-002, f-005 registered)
report-playthrough-manuscript --world report:   21 scene(s), unplaced=0, undecidable=0, outside order=11
report-playthrough-manuscript --world shelter:  21 scene(s), unplaced=0, undecidable=0, outside order=11
report-playthrough-manuscript --world confront: 20 scene(s), unplaced=0, undecidable=0, outside order=12
```

## Deliverables in run/author/

`sections.json`, `facts.json`, `order.json`, `narrative-rules.json`,
`store.atomic.json` (final, gate-clean), `author-log.md`. The store rebuilds
byte-for-byte from the empty seed + `import-sections` + `import-facts` +
`add-disclosure-plan` + two `set-disclosure` calls; facts.json is the source of
truth.
