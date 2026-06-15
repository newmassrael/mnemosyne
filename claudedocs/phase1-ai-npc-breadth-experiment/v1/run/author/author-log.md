# Author log — "The Lantern House at Corvath Ford"

A crowded-house mystery authored as a gate-checked fact base, top-down per the
author brief. This log records the Phase-0 skeleton (frames + tentpoles), then
the write -> gate -> repair iterations.

## Phase 0 — scope + skeleton (laid before any detail fill)

### Scope
- 29 scenes. Shared spine `sc-01..sc-17` (17 scenes), single primary fork at
  `sc-17`, then three branch ranges of 4 each: NAME `sc-18..sc-21`,
  HOLD `sc-30..sc-33`, ACT `sc-40..sc-43`.
- Exactly one primary fork (Wend's choice: NAME / HOLD / ACT) -> 3 distinct
  terminal world-lines, each with its own ending (`sc-21` / `sc-33` / `sc-43`).
- Each terminal world walks 21 scenes (17 spine + 4 branch); the other 8
  branch scenes are correctly "outside order" for that world.

### The cast as frames (13 frames: ground truth + 12 people)
- `gt`     — ground truth (what actually happened)
- `wend`   — Wend Aldercott, deputy ford-warden (POV)
- `clerk`  — Halloran Voss, circuit magistrate's clerk
- `factor` — Berdine Quist, wool-factor (strongbox + ledger)
- `soldier`— Tomas Reke, discharged soldier
- `hooded` — Cray Holt, the hooded traveller (Mabon's creditor)
- `midwife`— Senna Cobb, midwife
- `tinker` — Old Pell, tinker who knows the warden
- `trader` — Garrec, horse-trader
- `drover` — Lyle, drover (travelling with Garrec)
- `groom`  — Aldous Frith, the bridegroom (eloping couple)
- `bride`  — Joan Tarry, the bride (eloping couple)
- `warden` — Mabon Aldercott, the old ford-warden (Wend's uncle; absent, then dead)

### The two intertwined threads (decided up front)
1. **The warden thread (the real crime).** Mabon did not go "downriver on
   business." He had pledged the Lantern House against a gambling debt to Cray
   Holt in a deed-bond. The bond is a FORGERY — Cray forged a pledge of the
   whole house, beyond Mabon's small, half-paid true debt. On the first flood
   night Mabon crossed to refuse the bond; they struggled at the millrace and
   Mabon drowned. Cray is the hooded traveller; he took the bond from the body.
2. **The strongbox thread (the loud decoy).** The wool-factor's strongbox was
   already nearly empty on arrival (lead under a skim of coin) because Berdine
   had embezzled his master's money. Berdine broke his OWN box in the night to
   stage a robbery and accused the nameless hooded traveller — which also,
   conveniently for Cray, sent the house hunting a coin-thief instead of a
   killer. The cry/crash heard that night = Berdine's staged break-in; the
   splash that followed = Mabon into the race. Two events the house heard as one.

### The three roads (genuinely distinct terminals)
- **NAME** — Wend lays it all before the clerk. The clerk opens his sealed
  assize papers, matches the genuine county seal, proves the deed-bond a
  forgery; Berdine's staged theft unravels under questioning (a robbed full box
  leaves marks an empty box never had). Cray and Berdine bound for the assize.
- **HOLD** — Wend judges a stranger's word + a torn log won't convict, says
  nothing, lets the house cross, then burns the copied bond-leaf in secret —
  Cray crosses free with the forged bond and his guilt.
- **ACT** — Wend raises the chain-ferry and shuts the ford against Cray; the
  soldier (two-sound memory) and tinker (the debt) back him; Cray confesses,
  Wend burns the bond at the lantern, and Cray drowns at the ford he came to
  take. The rest of the house crosses none the wiser.

### Load-bearing knowledge fragments + objects (declared up front)
- Objects: `strongbox`, `deed_bond`, `ferry_log`, `clerk_papers`, `wet_cloak`,
  `lantern`.
- Knowledge only one traveller held: the BRIDE saw a hooded figure cross the
  yard to the millrace in the dark (`f-bride-01`); the SOLDIER heard TWO sounds,
  not one, on his watch (`f-sol-02`); the MIDWIFE found the hooded cloak hung
  sodden hours after the rain stopped (`f-mid-02`); the TINKER knew Mabon's debt
  (`f-tink-02/03`).

### Reveals and their setups (authored together)
- SETUP `f-gt-12` (Mabon tore the ferry-log leaves) -> PAYOFF `f-wend-10`
  (Wend finds the log torn, knows Mabon kept it 30 years). Knowledge reveal.
- SETUP `f-gt-11` (Cray's cloak soaked at the race) -> PAYOFF `f-mid-02`
  (midwife finds the wet cloak). Only-one-traveller knowledge.
- SETUP `f-bride-01` (bride saw the hooded figure) -> PAYOFF `f-bride-03`
  (bride tells Wend once the body is found). Only-one-traveller knowledge.
- SETUP `f-sol-02` (soldier heard two sounds) -> PAYOFF `f-sol-03` (fixes the
  death-time when the body is found) AND `f-act-02` (damns Cray in ACT).
- SETUP `f-gt-06` (box empty on arrival) -> PAYOFF `f-wend-14` / `f-name-05`.
- SETUP `f-gt-03` (deed-bond forged) -> three road-distinct payoffs: spine
  `f-wend-13`, plus `f-name-03` (proven at law) / `f-hold-04` (burned in
  secret) / `f-act-03` (burned, confessed). The central object resolves three
  distinct ways.
- TYPED state-change setup/payoffs (substantiated): `f-gt-14` (Mabon `fate=alive`)
  -> `f-gt-04` (`fate=dead`); `f-gt-02` (`deed_bond custody=mabon`) ->
  `f-gt-10` (`custody=cray`).

### Backreferences made structural (evidence edges, not prose)
- `f-name-03` cites `sc-07` (the seal) — the seal-match rests on a planted scene.
- `f-wend-10` cites `sc-08` (the night) for the torn log; `f-wend-11`/`f-wend-13`
  cite `sc-13` (the tinker's debt); `f-sol-03`/`f-act-02` cite `sc-12` (the two
  sounds); `f-bride-03` cites `sc-15` (body found). All cited scenes are on the
  shared spine, reachable at-or-before the citing fact in every world-line.

## Phase 0 gate baseline + Phase 1 iterations

**Iteration 1 — first import (79 facts) + full gate pass.**
- `validate-continuity`: 0 structural + 0 interval. Clean.
- `report-fork-tree`: fork PLACED at sc-17, 3 branches registered, all terminal.
- `report-payoff-coverage`: terminals (name/hold/act) all `dangling=0`, BUT the
  trunk world `main` showed `[DANGLING] f-gt-03` and a `[payoff->unmarked]`
  notice (`f-name-03 -> f-gt-13`).
- `report-payoff-substantiation`: several `UNSUBSTANTIATED` — typed setups
  (`f-gt-03` forged, `f-gt-06` empty) whose typed payoffs carried the SAME
  object value (no state change), reading as hollow.

  WHAT THE GATE CAUGHT / HOW FIXED:
  1. Substantiation taught me the model: a typed setup is *substantiated* only
     when a payoff flips its typed object to a DIFFERENT value (a real
     state-change), not a re-assertion. The forged-bond and empty-box are
     EPISTEMIC reveals of a latent truth, not physical state changes. Fix:
     un-typed those two setups (they stay real setups, honestly *unverifiable*
     in substantiation) and added two genuine physical state-change pairs that
     ARE load-bearing: `f-gt-14` (Mabon alive) -> `f-gt-04` (dead), and
     `f-gt-02` (bond custody mabon) -> `f-gt-10` (custody cray). These now read
     `substantiated`.
  2. `f-name-03` paid off `f-gt-13`, which I had un-marked as a setup -> the
     `payoff->unmarked` notice. Fix: dropped `f-gt-13` from `f-name-03.pays_off`
     (the seal stays a structural backreference via `evidence:[sc-07]`).
  3. `f-gt-03` (forged bond) dangled on the `main` trunk because all its
     payoffs lived post-fork. Fix: gave it a spine payoff — `f-wend-13` (sc-16),
     where Wend, holding the tinker's word that the debt was small and
     half-paid, sees the whole-house pledge overreaches and must be forged.
     Cited `sc-13` as evidence for that backreference.

**Iteration 2 — rebuilt from the empty seed + re-import (80 facts) + full gate pass.**
  (Edits changed existing rows, so I rebuilt the store from the empty seed
  rather than re-importing additively onto divergent rows.)
- `validate-continuity` (`--severity reject --interval-severity reject`):
  0 structural + 0 interval, exit 0.
- `report-fork-tree`: 3 world-lines, fork PLACED at sc-17, 0 unplaced.
- `report-payoff-coverage`: `dangling=0` in EVERY world (name/hold/act AND
  main), no `payoff->unmarked`.
- `report-payoff-substantiation`: 2 substantiated, 0 unsubstantiated, rest
  unverifiable (untyped epistemic reveals — the honest classification).
- `report-timeline-gaps` (each of name/hold/act): violated=0, unverifiable=0.
- `report-playthrough-manuscript` (each of name/hold/act): 21 scenes,
  undeclared adjacencies=0, unplaced=0, undecidable=0, outside order=8.

  All brief-required gates clean. No further repairs needed.

## Knowledge-boundary verification (report-frame-view spot checks)

Confirmed each person knows exactly their own road and no more:
- `factor` (Berdine) on `mabon` at sc-16: holds only "does not know the warden
  is dead / no deed-bond" — correctly ignorant of the death thread.
- `clerk` on `deed_bond` at sc-17 (main): holds NOTHING (the forgery is unknown
  to him on the spine); at sc-19 in the NAME world he holds the proven-forgery
  fact — the road-specific reveal lands only where it should.
- `bride` on `cray` at sc-11: holds the hooded-figure sighting (sc-08) and
  `not_holding=1` her own later sc-16 telling (correct temporal scoping).
- `wend` on `cray` at sc-17: holds his three assembled facts and nothing he had
  no way to learn.
- `hooded` (Cray) on `mabon` at sc-09: knows Mabon drowned — knowledge no
  innocent house member holds, because he caused it.

## Iteration count

2 write -> gate -> repair iterations (iteration 1 surfaced the payoff-coverage
trunk dangling, the payoff->unmarked notice, and the substantiation hollow-payoff
smell; iteration 2 verified all required gates clean after the three fixes).
