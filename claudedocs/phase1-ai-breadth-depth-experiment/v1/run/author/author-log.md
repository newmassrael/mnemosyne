# Author log — "The Ember Winter at Grayloam"

A deep-and-crowded branching winter, authored top-down as a gate-checked fact base.

## Phase 0 — scope + skeleton (laid before any detail fill)

### Scope
- **95 scenes** (`sc-01 .. sc-95`). Shared spine `sc-01..sc-20`, then a SEQUENCE of three
  nested forks, each opening genuinely different roads.
- Aim was a genuinely long winter (60+); the branch ranges land at 95 total scenes, with
  each terminal world-line walking 35–56 of them.

### The forks (a sequence across the winter, not one split)
1. **FORK 1 @ sc-20** — open the dead master's accounts and ration by the ledger, or
   ration by need as the collier demands.
   - `ledger` (sc-21..35) | `ration` (sc-79..95, TERMINAL)
2. **FORK 2 @ sc-35** (on the `ledger` road) — trust the factor's company plan, or the
   village's, at the midwinter crisis (furnace fails, fever spreads).
   - `company` (sc-36..48) | `village` (sc-65..78, TERMINAL)
3. **FORK 3 @ sc-48** (on the `company` road) — at the thaw, lay the death before the
   law, or settle it at Grayloam first.
   - `law` (sc-49..56, TERMINAL) | `settle` (sc-57..64, TERMINAL)

**Four distinct terminal world-lines**, each with its own motivated ending:
- `law` — the truth goes down the pass on the coroner's record; the charter held for the law.
- `settle` — Grayloam keeps its own counsel; the death is called illness, one agreed account goes down.
- `village` — the settlement holds the charter itself and runs Grayloam by its own hand.
- `ration` — the books are never opened at Grayloam; the company finds the master's fraud for itself in the lowlands.

(Three consequential decision points producing four terminal endings — exceeds the floor of three/three.)

### The cast as frames (13 frames = ground truth + 12 individuated people)
- `gt` — ground truth.
- `coll` — Coll Brand, the undermaster (POV).
- `factor` — Hewlin Sarne, the company factor sent up to audit.
- `widow` — Anwen Veil, the ironmaster's widow.
- `clerk` — Dob Asher, the assay-clerk.
- `collier` — Garrad Mowe, the master collier.
- `keeper` — Ell Furn, the furnace-keeper.
- `herbwife` — Mother Sela, the herbwife called the night it happened.
- `carter` — Tam Reke, the last carter over the pass.
- `soldier` — Varn, the discharged soldier.
- `prospector` — Quill, the stranded ore-prospector.
- `chaplain` — Father Oran, the chaplain who hears confessions.
- `child` — Nessa, the child who sees what adults miss.

Each frame holds exactly what that person witnessed or was told; several beliefs diverge
from the ground truth and from each other (see below).

### The inciting truth (ground-truth frame, authored first)
Maddox Veil was **secretly dying** of a wasting heart-illness (known only to the herbwife).
For years he had **skimmed ore weights and sold stores down the pass**, hiding a real
shortfall — so the cellar was **already short** when locked. On the first night of deep snow
the factor confronted him over the books; cornered and dying, **Maddox took a fatal dose of
the herbwife's heart-tincture himself** and, before he died, **altered the ledger's last page
in a disguised hand to blame the shortfall on the collier**. No one struck or poisoned him —
the death was his own act, not a murder, and the collier is innocent.

### Diverging beliefs (each person acts on what they could learn)
- `child` believes the tall visitor (the factor) did the master harm — she only saw him leave.
- `clerk` at first suspects the collier over-drew the stores — the altered page points there.
- `widow` believes worry and his weak heart killed him; she saw him take "his medicine" but
  read it as illness, not self-murder.
- `factor` knows only that he left Maddox alive and angry; he does not know the death was self-administered.
- The settlement at large reads the death as ambiguous; only the herbwife (illness) and the
  widow (the dose) each hold a true fragment, and neither holds the whole.

### Load-bearing setups -> payoffs (planted early, paid in every terminal world they reach)
- `f-herbwife-illness` (sc-06, **knowledge only the herbwife held**) -> paid on `company`
  (`f-coll-learns-illness`, reaching `law`+`settle`), `village` (`f-herbwife-tells-collier`),
  `ration` (`f-herbwife-tells-coll-ration`). Clean in all four terminals.
- `f-widow-saw-medicine` (sc-07, **the dose only the widow witnessed**) -> paid on `company`
  (`f-widow-breaks`), `village` (`f-widow-confides-herbwife`), `ration` (`f-widow-confirms-ration`).
- `f-coll-handed-charter` (sc-09) -> `f-coll-opens-ledger` / `f-accounts-sealed`.
- `f-keeper-furnace` (sc-15, the furnace warning) -> the midwinter failure on every road.
- `f-factor-letter-shown` (sc-12) / `f-factor-claims-books` (sc-26) — the company's claim chain.
- `f-coll-pieces-truth` (sc-45) and `f-coll-pieces-no-ledger` (sc-90) — the reveal of the
  true manner of death, paid at each thaw ending.
- `f-carter-up` (sc-02) -> `f-factor-letter-shown` (the sealed letter the carter brought up).

Five-plus setups, two of them knowledge a single person held — exceeds the floor of three.

### Backreferences (structural, in `evidence[]`, not bare prose)
Every callback cites its establishing scene: e.g. `f-coll-matches-hand` cites `sc-22` (where
the altered hand was first seen); `f-collier-denies` cites `sc-14` (the collier's own count);
`f-factor-claims-books` cites `sc-12` (the letter); the thaw reveals cite the fork-3 scene and
the piecing scene. All cited scenes lie at or before the citing fact on its own world-line
(no `evidence_unreachable`).

## Phase 1 — detail fill + write -> gate -> repair iterations

I authored sections, then the full fact manifest (skeleton + connecting detail) in one pass,
then ran the gates and repaired. Iteration count: **3 repair iterations** after the first
gate run.

- **Iteration 1 — payoff coverage.** First run flagged (a) every `gt`-frame fact and several
  spine facts marked `payoff_expectation:"expected"` as `[DANGLING]` in terminal worlds, and
  (b) many `[payoff->unmarked]` warnings (a `pays_off` target not marked as a setup).
  Root cause: I had over-marked setups. A setup marked `expected` must pay off in EVERY
  terminal world it is reachable in; ground-truth facts and several spine facts only pay off
  on a subset of roads (or are simply the truth, not a planted-payoff device).
  Fix: removed `payoff_expectation` from the `gt` facts and from spine facts that resolve on
  only some roads (`f-clerk-weights-wrong`, `f-collier-counts`, `f-coll-cellar-short`,
  `f-coll-letter-partial`, `f-factor-audit-purpose`, `f-factor-confront`, `f-soldier-*`,
  `f-prospector-*`, `f-child-visitor`, `f-chaplain-confession`, `f-coll-sees-altered`,
  `f-clerk-reads-false`, `f-gt-collier-innocent`, `f-collier-village-authority`). Their
  callbacks survive structurally via `evidence[]`. Added the `expected` mark to the genuine
  cross-road setups (`f-factor-letter-shown`, `f-factor-claims-books`, `f-coll-pieces-truth`,
  `f-coll-thaw-near`, `f-coll-pieces-no-ledger`). Result: all four TERMINAL worlds -> dangling=0.

- **Iteration 2 — residual `[payoff->unmarked]`.** A handful of `pays_off` edges still pointed
  at now-unmarked spine setups (`f-prospector-knows-factor`, `f-collier-village-authority`,
  `f-prospector-assay`, `f-soldier-saw-factor`). These spine setups can't be marked (they pay
  off on only one road, so marking would re-dangle them elsewhere). Fix: dropped those
  `pays_off` edges; the backreference is preserved via the `evidence[]` citation, which is the
  brief's required structural-backreference mechanism anyway. Result: 0 `[payoff->unmarked]`.

- **Iteration 3 — substantiation (advisory) over-reach, reverted.** I tried to strengthen the
  two single-holder reveal chains (illness, the dose) to `substantiated` by typing both setup
  and payoff. (a) The first attempt failed the atomic import: `f-widow-saw-medicine`'s typed
  subject `death` was not in its `entities[]` list — fixed by adding `death` to the list.
  (b) Re-running substantiation then reported `unsubstantiated=2` per terminal world: the gate
  requires a typed *state-change* to discharge a typed setup, and my payoff carried the same
  typed value as the setup (no state change) -> "typed setup, hollow payoff". `unsubstantiated`
  is the gate's actual defect signal, whereas the prior `unverifiable` (untyped) is benign
  advisory. Rather than fabricate artificial state-change values to satisfy an advisory metric,
  I reverted the setup-side and widow-payoff-side typing. The load-bearing reveal facts that
  legitimately encode a state-change (`f-coll-learns-illness`, `f-coll-matches-hand`,
  `f-coll-pieces-truth`, `f-herbwife-tells-collier`, `f-herbwife-tells-coll-ration`,
  `f-coll-pieces-no-ledger`, and the per-ending `final-disposition`/`culpability`/`charter-holder`
  facts) keep their typed legs. Final substantiation: `unsubstantiated=0` in all worlds.

## Knowledge / backreference problems a gate caught and how I fixed them

- The payoff-coverage gate caught that I had treated **ground truth as a setup**. Ground truth
  is the bedrock the reveals discover, not a planted-payoff device; unmarking the `gt` facts
  was the correct model (their dramatic function is carried by the typed reveal facts that
  discharge the person-frame setups). This is the clearest case of a gate correcting a
  knowledge-model error.
- The substantiation gate caught a typed subject (`death`) missing from a fact's `entities[]`
  (atomic import refused the whole manifest) — a real grounding error, fixed by listing it.
- I confirmed per-person knowledge with `report-frame-view` across the whole arc, including the
  acquisition boundary: on the `ration` road, Coll does NOT hold the manner-of-death
  (`f-coll-pieces-no-ledger`) at sc-86 (`not_holding=1`), and only holds it at sc-90 after the
  factor (sc-88) and herbwife (sc-89) tell him — so no person knows more than their winter let them.

## Convergence

**No convergence was used.** The plot is a natural forest of forks: each road's thaw and the
company's return play out differently (the law road records it; the settle road agrees one
account; the village road holds the charter; the ration road sends the sealed books down), so
there is no shared later continuation the plot wants to declare once. A plain fork forest is
correct here, and the brief states convergence is an optional tool, not a requirement.

## Final gate state (all clean)

- `validate-continuity`: violations 0 (structural=0, interval=0); conflict_pairs=0, cross_scope=0, unordered=0.
- `report-fork-tree`: 6 registered world-lines, 0 unplaced fork points; every branch terminal.
- `report-payoff-coverage`: dangling=0 in every terminal world (`law`, `settle`, `village`, `ration`).
  (Danglings remain only on the non-terminal spines `main`/`ledger`/`company`, where they correctly pay off downstream.)
- `report-payoff-substantiation`: unsubstantiated=0 in every world (remaining `unverifiable` are benign advisories).
- `report-timeline-gaps` (each branch): violated=0, unverifiable=0.
- `report-playthrough-manuscript` (each branch): unplaced=0, undecidable=0, undeclared adjacencies=0.
