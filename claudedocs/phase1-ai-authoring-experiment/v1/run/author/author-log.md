# Author log — "The Saltmarsh Bell" (v1 AI-authoring run)

A record of the write -> gate -> read -> fix loop used to author the fact base.

## The story (what was made true)

- **Setting:** tidal-island monastery, marsh coast, age of sail. The dusk bell guides the fishing boats home through the shifting channel before the light fails; a boat that misreads the channel at night is lost.
- **Ground truth (`gt` frame):** Father Crispin's eyes and ears have been failing for two years. He reads the dusk crossing by two tools — a worn tide-stone (depth) and a brass reading-glass (sighting the far marks). On nights his failing senses cannot trust the true dusk, he rings the bell an hour EARLY, while the channel is still certainly passable, to bring every boat in safe. It is caution and penance (years ago he once rang late, misread, and lost a boat), never malice. The one boat that did NOT return on an early-bell night — Maren's cousin Edrick's *Kittiwake* — was lost to Edrick's own greed: he stayed out past the early bell to run contraband. Crispin knows, and keeps silent to spare Edrick's family the shame.
- **Belief divergence (premise requirement):** Aldous (`aldous` frame) comes to BELIEVE the early bell is a snare — that Crispin rings early to wreck boats for salvage — and reads Crispin's straining at the marks as guilty pretence, not failing senses. Maren (`maren` frame) believes the bell killed her cousin. Both act on the belief, not the truth.
- **Setups that pay off (both typed -> substantiated):**
  - `gt-tools` (sc-01: the tide-stone + brass glass) pays off in every world — surfaces as Crispin's failing-sense strain (sc-05), is handed to Aldous in *confront*, and is the very thing Aldous LACKS when he misreads the fog in *act_alone*.
  - `gt-early-is-mercy` (sc-03: the early bell's true meaning) pays off at each world's climax (the harbourmaster's ledger / Crispin's confession / the lost boat).
- **The fork (one primary branch point, at sc-05):** Aldous must choose his road on the eve of the season's worst crossing. Three terminal world-lines:
  - **EXPOSE** -> sc-06e/07e/08e: Aldous denounces Crispin to the village; the harbourmaster's ledger and the recovered contraband prove the early bell SAVED boats and that the *Kittiwake* died smuggling. Belief collapses; Crispin, vindicated but shamed by his failed senses, steps down and names Aldous bellwright.
  - **CONFRONT** -> sc-06c/07c/08c: Aldous accuses Crispin to his face; Crispin breaks his thirty-year silence, confesses the failing senses and the old guilt, and puts the stone and glass in Aldous's hands. On the storm-fog night the rope passes to Aldous, who rings true; every boat comes home and Edrick's secret is kept for Maren's sake.
  - **ACT ALONE** (tragedy) -> sc-06a/07a/08a: certain the bell is the killer, Aldous locks Crispin out and rings on his own judgment at "true dusk." Without the stone and glass he misreads the fog exactly as Crispin once did; a boat reads the wrong bell and is lost — the death the early bell had always prevented. He understands too late.

## Scene/scope shape

14 scenes = 5 shared spine (sc-01..sc-05) + 3 per world-line. Exactly one primary fork (sc-05) into 3 terminal world-lines, each its own ending. 36 facts: 24 `gt`, 9 `aldous`, 3 `maren`.

## The write -> gate -> repair iterations

**Setup (pre-loop):** Read both input files; studied a known gate-clean reference fact base (`phase1-disclosure-craft-experiment`) to confirm the exact `import-facts` manifest shape (frames/branches/entities/predicates/facts, `typed` leg, `pays_off`, `payoff_expectation`), the `order.json` branch-chain shape, and the per-world gate semantics. Discovered the CLI resolves `--sidecar`/`--order`/`--manifest` against the repo root, not cwd — switched to ABSOLUTE paths for every call (one stray `store.atomic.json` written at repo root was removed and the import re-run).

**Iteration 1 — first full import + gates.** Imported 14 sections + 36 facts (atomic, clean). Gates flagged:
- `validate-continuity`: 0 violations (the 3 belief-vs-truth pairs registered correctly as cross-frame DATA, not conflicts) — clean.
- `report-fork-tree`: 3 world-lines, 0 unplaced — clean.
- `report-timeline-gaps` (all worlds): 0 violated / 0 unverifiable — clean.
- `report-playthrough-manuscript` (all worlds): 0 unplaced / 0 undecidable — clean.
- `report-payoff-coverage`: **TWO problems.** (a) `gt-maren-edrick` marked `expected` but DANGLING in `confront`/`act_alone`/`main` (it only pays off in `expose`). (b) THREE `[payoff->unmarked]` warnings: `ex-ledger-proof`, `co-confession`, `ac-boat-lost` all declared `pays_off:[gt-early-is-mercy]`, but `gt-early-is-mercy` itself was not marked as a setup.

**Repair 1.** (a) The *Kittiwake*'s true cause is resolved on-page only in `expose`; in `act_alone` the tragedy deliberately leaves it unresolved, so it cannot be a setup that must pay off in every world — DEMOTED `gt-maren-edrick` to an unmarked planted detail (its typed `kittiwake_cause` arc still returns) and removed the now-dangling `pays_off` link on `ex-contraband-found`. (b) `gt-early-is-mercy` IS a genuine cross-world setup (the central mystery) — MARKED it `payoff_expectation:"expected"`. Rebuilt the store from a clean seed so the edited rows took effect.

**Iteration 2 — re-gate.** `report-payoff-coverage`: all three world-lines now `dangling=0` (`gt-early-is-mercy` + `gt-tools` both paid in each). Only `main` (the bare pre-fork spine, which has no ending) shows the expected single dangling — verified against the reference store, whose gate-clean `main` world likewise dangles by design. `report-payoff-substantiation`: `gt-early-is-mercy` reported `unverifiable` in all three worlds — the setup was typed but its paying facts carried no typed state-change to discharge it.

**Repair 2.** Added a `typed` leg (`the_bell early_bell_meaning -> proven-mercy / confessed-mercy / proven-mercy-too-late`) to each of the three payoff facts so the typed setup+payoff pair reads as substantiated. Rebuilt the store from clean seed.

**Iteration 3 — final re-gate.** `report-payoff-substantiation`: every world-line `substantiated=2, unsubstantiated=0, unverifiable=0`. All other gates re-confirmed clean. Confirmed via `--json` that each world's `sections_outside_order` list contains only the OTHER branches' scenes (correctly excluded from that world's walk) — matching the reference store's behavior — while `unplaced_facts`/`undecidable`/`undeclared_adjacencies` are empty.

**Total: 3 write -> gate -> repair iterations** (iteration 1 surfaced the payoff issues; repair 1 fixed coverage; repair 2 fixed substantiation; iteration 3 confirmed all-clean).

## Final gate output (clean)

```
validate-continuity:        violations: 0 (structural=0 interval=0)   [facts=36, conflict_pairs=3 = the belief/truth pairs as cross-frame data]
report-fork-tree:           3 registered world-line(s), 0 unplaced fork point(s)   [expose/confront/act_alone all fork from main at sc-05]
report-timeline-gaps:       expose / confront / act_alone -> violated=0 unverifiable=0
report-payoff-coverage:     expose paid=2 dangling=0 | confront paid=2 dangling=0 | act_alone paid=2 dangling=0
                            (main paid=1 dangling=1 = the bare pre-fork spine, no terminal — matches the reference store by design)
report-payoff-substantiation: expose / confront / act_alone -> substantiated=2 unsubstantiated=0 unverifiable=0
report-playthrough-manuscript: expose / confront / act_alone -> 8 scene(s), unplaced=0, undecidable=0, undeclared adjacencies=0
                            (outside order=6 = the two non-active branches' scenes, correctly excluded from each world's walk)
```

Every gate the brief lists is clean for every terminal world-line. The lone residual — `main` world `dangling=1` — is the bare shared spine (a non-terminal prefix that holds spine setups paying off only after the fork), structurally identical to the validated reference fact base's gate-clean `main` world.
