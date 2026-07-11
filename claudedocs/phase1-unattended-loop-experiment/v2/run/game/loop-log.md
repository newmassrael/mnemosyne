# loop-log.md — authoring "The Ban Tower" into a Mnemosyne store

Numbered log of every `propose-verdict` and `report-authoring-frontier`
call, the verdict/gaps, and exactly what changed and why. Honest friction
notes at the end. Author + gates only; the two sources of truth were
`describe-schema` and `premise.md`.

## Game as designed

- 15 scenes (sc-01..sc-15), within the 14–20 required.
- 5 frames: `ground-truth` + 4 character frames (`relief`=Wren the relief
  lookout, `senior`=Aldous the missing senior lookout, `dispatch`=Nadia the
  dispatcher, `packer`=Tomas the packer). 4 characters, each its own frame.
- One fork at sc-07: the descent road continues on `main` (sc-12..sc-15);
  the report road forks to branch `call-it` (sc-08..sc-11). Two canonical
  world-lines, each with its own aftermath scenes.
- Withheld secret: `f-secret-fate` (Aldous deserted — set a small fire to
  burn his falsified log page and walked out) + `f-secret-smoke` (the smoke
  was real but his own burn, not a wildfire). Default telling withholds;
  revealed only late (sc-14) and only on the descent world (`main`);
  withheld on `call-it`.
- One quest `q-account` ("account for the senior lookout"): giving setup
  `f-04-quest` (expected), per-road completion — `main`: done/`accounted`
  @ sc-14; `call-it`: done/`closed-inconclusive` @ sc-11.
- Setup→payoff chains: `f-02-button` (scorched brass button, `expected`)
  pays off in BOTH worlds (call-it sc-10, main sc-14). `f-04-radio` (the
  stray radio click, unmarked) pays off in ONLY one world (main sc-13).
  Plus the torn-page detail `f-02-torn` → paid at sc-10 and sc-14.

## Artifacts

- `sections.json` — 15 scenes, imported.
- `facts.json` — final manifest (43 facts, 5 frames, 1 branch, 13
  entities, 6 predicates, 1 disclosure plan / 9 overrides). Kept
  byte-consistent with the applied store (see call PV-4).
- `canon-order.json` — canon order edge graph; main trunk + descent, plus
  the `call-it` fork edges. Pinned via `[continuity].canon_order_path` in
  `mnemosyne.toml` so the store is renderable without passing `--order`.
- Applied store: `docs/.atomic/workspace.atomic.json`.

---

## Numbered gate calls

### PV-1 — `propose-verdict --manifest facts.json --order canon-order.json`
- Structure at this point: BOTH roads as forks (`call-it` + `go-down`)
  off `main` at sc-07.
- Verdict: **rollback**, 2 violations (both gating):
  - `[continuity] evidence_unreachable` — `f-secret-fate` cites evidence
    `sc-02` not reachable by `sc-01` in branch `main`.
  - `[continuity] evidence_unreachable` — same fact, evidence `sc-03`.
- Cause: I set the ground-truth desertion fact's `canon_from` to `sc-01`
  (true from the start) but its evidence (the torn page sc-02, the last
  entry sc-03) is later; the gate requires every evidence ref reachable
  at-or-before `canon_from` in its world-line.
- Change: moved `f-secret-fate.canon_from` `sc-01` → `sc-03` (the point
  where both cited clues are in hand). Content fix, not a wire-format fix.

### PV-2 — `propose-verdict --manifest facts.json --order canon-order.json`
- Verdict: **commit**, 0 violations.
- Then `import-facts` (IF-1): applied 42 facts, 2 branches
  (`call-it` + `go-down`).

### FR-1 — `report-authoring-frontier --telling default-telling --order canon-order.json`
- 35 gap(s):
  - zero-fact scenes: none
  - unordered scenes: none
  - **dangling setups [call-it] (1): f-04-quest**
  - **dangling setups [main] (2): f-02-button, f-04-quest**
  - unresolved quests: none
  - never-planned disclosures (33) — intentional withhold, allowed.
- Diagnosis: the dangling setups were the real gaps. `[call-it] f-04-quest`
  = the quest giving had no completion on the report road. The `[main]`
  danglers were the structural surprise: with BOTH roads forking off
  `main` at sc-07, the bare `main` world-line is a dead prefix (sc-01..07)
  that never reaches any payoff, so its trunk setups (`f-02-button`,
  `f-04-quest`) dangle on `main` itself.
- Change (structural redesign): make `main` a full world-line — the
  descent road continues on `main` (sc-07 → sc-12 → … → sc-15), and only
  the report road forks (`call-it`). Trunk setups then pay off on `main`
  at sc-14. Added `f-11-quest-callit` (completion on `call-it`, scalar
  discharger `closed-inconclusive`, `pays_off` the giving) so the quest
  resolves on BOTH roads with different discharge. Repointed the secret's
  disclosure `first_at` from `go-down` to `main`.
  - Facts are append-only and the store was already applied, so I reset:
    backed up the store to scratchpad, deleted
    `docs/.atomic/workspace.atomic.json` (gitignored; there is no
    init/reset subcommand), and re-ran `import-sections` — the CLI
    recreates a fresh empty store on import, giving a clean baseline with
    the 15 sections only.
  - Edited `canon-order.json` (extend main edges sc-07→sc-12..sc-15, drop
    the `go-down` branch) and `facts.json` (drop the `go-down` branch reg,
    move its 12 facts to `main`, add `f-11-quest-callit`).

### PV-3 — `propose-verdict --manifest facts.json --order canon-order.json`
- On the fresh store (sections only). Verdict: **commit**, 0 violations.
- Then `import-facts` (IF-2): applied 43 facts, 1 branch (`call-it`).

### FR-2 — `report-authoring-frontier --telling default-telling --order canon-order.json`
- 34 gap(s):
  - zero-fact scenes: none
  - unordered scenes: none
  - **dangling setups: none**
  - **unresolved quests: none**
  - never-planned disclosures (34) — intentional withhold, allowed.
- All structural axes clean. Then I verified renderability out-of-band
  with the read projections (`report-fork-tree`, `report-quest-graph`
  per world, `report-payoff-coverage`, `report-playthrough-manuscript`).
- Found a defect the frontier does NOT catch: on `call-it`, the render
  brief showed `f-secret-fate [state]` (told at sc-03) — a leak of the
  secret on the road where it should stay withheld. Reason: an override's
  `mode: state` applies globally; `first_at` only localises the reveal
  timing on the worlds it lists. Worlds not in `first_at` still get the
  global `state` (told from `canon_from`).
- Change: re-encode as `mode: withhold` + `first_at main=sc-14` via
  `set-disclosure` for both secret facts. Verified: `main` now renders
  `[withhold first_at=sc-14]` (revealed late at sc-14), `call-it` renders
  `[withhold]` (withheld throughout). Mirrored the change into `facts.json`
  (overrides `state` → `withhold`) so the artifact reproduces the store.

### PV-4 — `propose-verdict --manifest facts.json --order canon-order.json`
- Consistency check: the edited `facts.json` vs the applied store.
- Verdict: **commit**, 78 no-op, 0 created — `facts.json` is now a faithful,
  re-importable image of the final store.

### FR-3 — `report-authoring-frontier --telling default-telling --order canon-order.json`
- 34 gap(s): zero-fact scenes none · unordered scenes none · dangling
  setups none · unresolved quests none · never-planned disclosures (34).
- Every structural axis is `none`; the only remaining "gaps" are the
  never-planned disclosures, which are the intentional withhold choice and
  are explicitly permitted to remain. **Stop condition met.**

---

## Tally

- `propose-verdict` calls: 4 (PV-1..PV-4). Wire-format / JSON-serialization
  fixes: 0 — the manifest deserialized correctly on the first call (the
  contract's worked example + the `object_kind: scalar` → wire tag `value`
  note prevented serialization errors). Content/continuity fixes: 1 (PV-1's
  evidence-reachability rollback). PV-2/3/4 were green confirmations.
- `report-authoring-frontier` calls: 3 (FR-1..FR-3).
- Gap axes seen: zero-fact scenes, unordered scenes, dangling setups,
  unresolved quests, never-planned disclosures. No axis beyond the
  documented set appeared. The one unexpected finding was WITHIN the
  dangling-setups axis: the bare `main` prefix world-line accrues dangling
  setups when both roads fork off it — closed by making `main` a full road.

## Honest friction notes

1. **Biggest friction — the dangling-on-`main` discovery required a store
   reset.** The frontier's dangling/payoff analysis is only observable
   AFTER a real `import-facts`: `propose-verdict`'s dry run covers shape +
   reference-integrity + off-branch continuity, but not payoff/dangling
   coverage, and `report-authoring-frontier` runs only on the applied
   store. So the "bare main prefix dangles" structural flaw was invisible
   until after I had committed the two-fork layout. Correcting it meant
   re-branching already-applied append-only facts, which is not an edit —
   it needs a fresh store. There is no `init`/`reset` subcommand; I had to
   delete the (gitignored) store file and rely on `import-sections`
   recreating an empty store. Discoverable, but a real workflow cost.

2. **The frontier does not catch disclosure leaks.** `report-authoring-
   frontier` treats a fact with any override as "planned" and moves on;
   it never checks whether the planned mode actually withholds on the
   worlds it should. Following the intuitive encoding (`mode: state` to
   mean "revealed") silently leaks the secret on the non-reveal world-line.
   The correct encoding — `mode: withhold` + a `first_at` reveal pin — is
   inferable from the contract but not spelled out, and the leak is only
   visible in the `report-playthrough-manuscript --telling` render brief
   (or, presumably, `validate-disclosure-leak`, which needs re-extracted
   prose I don't have). So "frontier clean" is necessary but not sufficient
   for a correct telling; I had to inspect the render brief to catch it.

3. **A single predicate has one fixed `object_kind`.** The contract
   describes `completed_by` as "entity or scalar value (the discharger),"
   but a registered predicate must pick ONE `object_kind`. I registered
   `completed_by` as `scalar` so both roads could discharge the quest with
   a value (`accounted` / `closed-inconclusive`); an entity discharger
   would have needed a second predicate. Minor, but worth noting that the
   quest-encoding prose reads as more permissive than the registry allows.

4. **`payoff-coverage` advisories vs frontier gaps.** `report-payoff-
   coverage` emits `[payoff->unmarked] ... (forgotten setup marking?)`
   hints for `f-04-radio` and `f-02-torn`. These are intentional: the
   radio is deliberately unmarked so its single-world payoff does not
   dangle on `call-it` (marking it `expected` would re-introduce a real
   frontier gap). They are advisory, not frontier gaps, so they correctly
   remain.
