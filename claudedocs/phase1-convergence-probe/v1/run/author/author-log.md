# Author log — The Reckoning at Harlow Mill

A branching fact base authored top-down with `mnemosyne-cli`, gate-checked
at every step. This log records the authoring loop, the tentpoles laid
first, the write→gate→repair iterations, and (honestly) every point where
the story I wanted to tell was awkward to express in the tool.

## Final shape

- **18 scenes.** Shared spine `sc-01..sc-08` (storm → voiceless keeper →
  jammed relief gate → who wedged it → dam will overtop → the keeper's law
  → race to the sluice → the choice/fork at `sc-08`). One exclusive fork at
  `sc-08` into two world-lines:
  - **`sluice`** (Sela opens the sluice): `sc-09a..sc-12a` — the pond
    bleeds, the lower row floods first in the dark and lives are lost, the
    dam holds, the mill is saved.
  - **`ride`** (Sela holds and rides to wake the lower row): `sc-09b..sc-12b`
    — the lower row is woken and flees, the dam overtops, the valley takes
    the flood unshaped, the mill (the town's living) is wrecked.
- **Convergence (REQUIRED).** BOTH world-lines terminate at the SAME two
  scenes: `sc-reckoning` (the mill-yard reckoning where Garrick must name
  Sela keeper or criminal) and `sc-river` (Sela alone at the river's edge).
  In `order.json` both branch edge-chains run `...→sc-reckoning→sc-river`,
  so the two paths run apart through the night and come back to the one
  dawn that judges them.
- 2 frames (`gt` ground truth, `town-belief`), 8 entities, 6 predicates,
  39 facts.

## Phase 0 — the tentpoles I placed first (before any connecting detail)

I did NOT free-associate scene by scene. I fixed the scope and laid the
load-bearing facts first, in this order:

1. **Scope + world-lines.** 18 scenes; spine `sc-01..sc-08`; two branches
   `sluice` / `ride` both forking at `sc-08`; shared convergence scenes
   `sc-reckoning` and `sc-river` reserved as the destination.
2. **Endings / destination first.** I authored the reckoning verdict and
   the river's-edge close as facts before the connecting middle:
   - `f-300a/b` the town gathers, Sela stands to be judged.
   - `f-301a/b` Garrick must say keeper's-duty-or-crime (the same question
     on both paths, worded for what that night did).
   - `f-303a/b` the verdict: knowing the gate was wedged, Garrick names Sela
     the keeper, not a criminal.
   - `f-400a/b` the river's edge: Sela looks at the water that did what it
     did (drowned the lower row / took the mill).
3. **The fork.** `f-014` at `sc-08` (the one exclusive choice), then each
   branch's distinct identity facts (`f-100`/`f-200`) so the two worlds are
   genuinely different.
4. **Load-bearing entities/objects.** Registered up front: `sela`,
   `ferran` (the keeper of record, voiceless), `garrick`, `bram` (who
   wedged the gate), `lower-row`, `relief-gate`, `sluice`, `dam`.
5. **The reveal AND its setup, together.** The wedged-gate long-range
   setup→payoff that crosses the fork:
   - SETUP `f-006` at `sc-03` (early) — Sela finds the relief gate wedged,
     `payoff_expectation:"expected"`, typed `gate_state=wedged`.
   - PAYOFF `f-302a` / `f-302b` at `sc-reckoning` (late, on EACH branch) —
     the wedged-gate truth surfaces and turns the verdict, typed
     `gate_state=known-wedged` (a real state-change), `pays_off:["f-006"]`,
     with `evidence:["sc-reckoning","sc-03","sc-04"]` so the backreference
     to the establishing scene is STRUCTURAL, not a bare phrase.
   I also placed `f-007`/`f-010` (who wedged it, and the consequence: no
   safe drain) as early payoffs of the same setup on the spine, so the
   setup is grounded before the fork as well as after it.

The second long-range thread is the **keeper-of-record** question: SETUP
`f-005` (Sela keeps in fact but her keeping is contested/unrecognized,
typed `keeper_recognition=contested`) paying off at the verdict via
`f-303a/b` (typed `keeper_recognition=affirmed`).

**Belief frame diverging from ground truth (REQUIRED):** the `town-belief`
frame holds `f-004` (the town believes Ferran is still the keeper and Sela
only plays at it) and `f-009` (the town believes the gate simply failed in
the storm — no one's fault), against the `gt` facts `f-005` / `f-007`. The
town acts on the false belief: at the reckoning Sela is judged as "a girl
playing at her father's work," and per-branch `f-106`/`f-205` carry the
town's wrong reading of her choice (chose the mill over the poor / held out
of spite) — distinct from what the night actually did.

I ran the gates over this skeleton before filling and it was structurally
clean, which is the point of the top-down contract: a spine I could trust.

## Phase 1 — detail fill + the write→gate→repair iterations

I filled the connecting beats (the spine scenes `sc-01,02,05,06,07`, each
branch's middle, the per-branch town-belief reactions) and re-ran the full
gate suite. Four repair iterations:

### Iteration 1 — typed-subject membership
`import-facts` rejected the whole transaction (atomic — nothing written):
`f-104`'s typed subject `dam` was not in the fact's `entities` list. The
gate enforces that a typed subject is also a retrieval-key entity. Fix:
added `dam` to `f-104.entities`. Re-import clean.

### Iteration 2 — the verdict→keeper payoff was an unmarked setup
`report-payoff-coverage` flagged `[payoff->unmarked] f-303a/b -> f-005
(forgotten setup marking?)`. The verdict facts declared `pays_off:["f-005"]`
but `f-005` had no `payoff_expectation:"expected"`. Fix: marked `f-005` as
an expected setup. (Correct call — it IS a deliberate long-range setup.)

### Iteration 3 — substantiation: same typed value isn't a state-change
After marking `f-005`, `report-payoff-substantiation` reported
`[UNSUBSTANTIATED] f-005 <- f-303a/b (typed setup, no typed state-change
discharges it)`. Root cause: I had typed both setup and payoff as
`keeper_of_record(sela)=sela` — identical object, so there was no
*change* of state to discharge the setup. The substantiation gate wants the
payoff to carry a typed state-CHANGE on the same subject+predicate.

This is the one place the story was genuinely awkward to express (see the
workaround note below). Fix: I introduced a scalar predicate
`keeper_recognition` that actually transitions — setup `f-005`
`keeper_recognition=contested`, payoff `f-303a/b` `keeper_recognition=
affirmed`. That gave the gate a real state-change to see. I also split out
`f-304a/b` (the `standing=named-keeper` settlement) as separate facts so
the verdict's two distinct consequences (recognition affirmed; standing
settled) are each their own row rather than overloaded onto one.

### Iteration 4 — final full sweep, all clean
Re-ran every gate with `--order`. Results:
- `validate-continuity`: violations 0 (structural 0, interval 0).
- `report-fork-tree`: 2 world-lines registered, 0 unplaced fork points;
  both fork at `sc-08`; both reach `sc-reckoning`→`sc-river`.
- `report-payoff-coverage`: terminal worlds `sluice` and `ride` both
  `dangling=0` (both setups paid). See the `main`-dangling note below.
- `report-payoff-substantiation`: terminal worlds both `unsubstantiated=0`
  (both setups substantiated by typed state-changes).
- `report-timeline-gaps --world sluice|ride`: violated 0, unverifiable 0.
- `report-playthrough-manuscript --world sluice|ride`: each 14 scenes,
  unplaced 0, undecidable 0, outside-order 4 (the other branch's scenes
  correctly excluded). Each world reads start to finish.

## Awkward / workaround points (recorded honestly)

1. **Convergence is not a first-class merge — it works by SHARING a scene
   id at the tail of both branch chains.** The premise's hard requirement is
   that both world-lines reach the *same* reckoning and the *same* river's
   edge — one destination, not two endings. The fork model is a forest
   (each branch has a single parent), so there is no native "join" node.
   What works cleanly: give both branch edge-chains the same terminal
   scenes (`...→sc-reckoning→sc-river`), and tag each branch's facts at
   those shared scenes with its own `branch`. The shared scene then holds
   `sluice`-tagged facts AND `ride`-tagged facts, and each world's
   playthrough sees only its own. I verified this is legal and clean before
   building the real store. It is not a workaround so much as the idiom the
   tool actually supports — but it is worth flagging that convergence is
   *expressed*, not *declared*: nothing in the registries says "these two
   branches join here"; the convergence lives only in the order file's edge
   chains sharing a tail. A reader of `branches[]` alone would not see it.
   Because of this, the same dawn beat had to be authored as TWO parallel
   fact sets (`f-300a..f-304a` and `f-300b..f-304b`, `f-400a`/`f-400b`) —
   the reckoning and the river's edge are duplicated per branch, differing
   only in what each night made true. That duplication is unavoidable given
   the model: a fact belongs to exactly one branch, so a beat that both
   branches play must be written once per branch.

2. **A setup that pays off across the fork "dangles" on the pre-fork spine
   world.** `report-payoff-coverage` shows `[DANGLING] f-005` (and the
   wedged-gate setup has the same shape) in world `main`. This is correct,
   not a defect: `main` is the shared trunk that ends at the fork — it never
   reaches a branch resolution, so a setup whose payoff lives post-fork
   cannot be discharged inside `main` alone. The brief's clean condition is
   "no setup dangling in any TERMINAL world," and the terminal worlds
   (`sluice`, `ride`) both show `dangling=0`. I left this as-is rather than
   contorting the story to pay the setup off before the fork, because the
   premise explicitly wants the wedged-gate truth to surface at the
   reckoning (post-fork). Worth noting only because the `main`-world line in
   the report looks like a flag until you remember `main` is not terminal.

3. **Payoff substantiation requires a typed VALUE CHANGE, which forced a
   small re-modelling of the keeper-of-record thread (iteration 3 above).**
   The natural way to state the keeper truth is "in ground truth Sela IS
   the keeper" (a constant), and the verdict "confirms" it. But a constant
   gives the substantiation gate nothing to discharge — setup value ==
   payoff value is not a state-change. I had to reframe the thread as a
   transition (`contested → affirmed`) so the verdict is a real change of
   state the gate can see. The story is arguably better for it (the
   reckoning visibly *moves* Sela's recognition rather than just restating a
   fact), but it was a tool-shaped pressure, not a story-shaped one: I
   changed how I modelled a true thing to satisfy the discharge check.

Everything else expressed straightforwardly: the spine, the single
exclusive fork, the two distinct middles, the belief-frame divergence, and
the structural backreferences (evidence arrays citing the establishing
scenes) all mapped cleanly onto the schema and passed on the first try once
the three issues above were repaired.
