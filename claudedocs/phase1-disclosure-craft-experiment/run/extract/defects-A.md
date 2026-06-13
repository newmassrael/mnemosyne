# defects-A — continuity / payoff / irony surfaces (blind re-extraction)

Store: `A.atomic.json`  ·  order: `A-order.json`  ·  rules: `A-rules.json`
Frames: ground, hale, junia, pike  ·  Branches: main, confront, quiet, reveal, burn
Facts: 60  ·  Rules: 3 (1 transition + 2 exclusive)

Every finding cites a scene-id (canon coordinate) + a quote from the page.

---

## 1. validate-continuity surface

`mnemosyne-cli validate-continuity --order A-order.json --rules A-rules.json`

| surface | count |
|---|---|
| rule_transition_invalid | 0 |
| rule_exclusive_overlap | 0 |
| unchained_state_pairs | 0 |
| succession_cross_branch | 0 (not reported as a distinct key; no cross-branch supersession violation surfaced) |
| SuccessionCycle | 0 (no cycle; edge import is cycle-guarded) |
| cross_scope_pairs | 6 (cross-frame conflict edges = data, never a violation) |
| unordered_pairs | 0 |
| rule_unordered_pairs | 0 |
| violation_count | 0 |

**No continuity violations.** The single transition rule (`crane-alive-arc`,
allowed `[[alive,dead]]`) is satisfied: the only alive→dead succession is
`fa-crane-alive` (sc-01, "A blind man on a dark stair at three in the
morning.") → `fa-crane-dead` (sc-01, "Crane had fallen from the top.").
The two exclusive rules (`custody-exclusive` on `held_by`, `location-exclusive`
on `located_at`, both per-subject) show no overlap: the night-log is held only
by Junia (sc-16, "The log is my hand ... he would dictate, and I would write
the figures down for him") and the spare key only by Pike (sc-12a, "the key
warm in his coat-lining all this month"); no object has two distinct holders
at one coordinate.

The 6 `cross_scope_pairs` are the authored cross-frame divergence (conflict)
edges — by design these are recorded as DATA, not violations (frame-scoped
continuity: cross-frame disagreement is legitimate perspectival difference).

---

## 2. report-payoff-coverage — the 6 required setups

`setups_total = 8` marked (the 6 required + the account split into soft/drained
and the clock split into stopped/wound — both halves of two compound setups).
The 6 required setups, all present and marked `payoff_expectation: expected`:

1. **night-log in a hand not Crane's** — `fa-log-strange-hand`, sc-03,
   "the *hand was wrong*".
2. **plate-camera with no exposures** — `fa-plate-blank`, sc-04,
   "the plate was *blank* — clean grey gelatine, never drawn back to the sky".
3. **meridian clock stopped 3:14, wound only hours before** —
   `fa-clock-stopped`, sc-05, "Fourteen minutes past three. The hands held it
   there"; `fa-clock-wound`, sc-05, "the clock had been *wound* ... only hours
   before it stopped".
4. **drained instrument account** — `fa-account-drained`, sc-08,
   "the account had been bled"; soft-tell `fa-account-soft`, sc-06,
   "a place where the figures did not quite carry".
5. **spare dome key reported lost a month before** — `fa-key-bare-hook`,
   sc-07, "*spare dome key mislaid, to replace*".
6. **Crane's unsent letter resigning his post** — `fa-unsent-letter`, sc-06,
   "A resignation. The director's own, private ... set down and not sent."

### Dangling among the 6 required setups (per world)

| world | paid | dangling |
|---|---|---|
| reveal (CONFRONT→REVEAL) | 8 | — none |
| quiet (QUIET-AUDIT) | 8 | — none |
| confront (CONFRONT spine, pre fork-2) | 7 | **fa-unsent-letter** |
| burn (CONFRONT→BURN) | 7 | **fa-unsent-letter** |
| main (shared spine only) | 0 | all 8 (payoffs live in the limbs — expected) |

**DANGLING FINDING — the unsent letter in the BURN world-line.**
`fa-unsent-letter` (sc-06, "A resignation. The director's own ... set down and
not sent.") is set up on the shared spine but receives no discharging payoff
in the CONFRONT→BURN world. In that world the case collapses at the stove:
sc-17w, "she had put the log in ... curl and blacken and fall to nothing", and
sc-18w certifies "*death by fall; cause undetermined; no unlawful act against
the deceased established*" — the irony of the already-written resignation is
never brought to bear. The reveal world discharges it (sc-19r certificate) and
the quiet world discharges it via `fa-crane-meant-leave-quiet` (sc-19b, "The
old astronomer had meant to lay down his post — of his own grief"). The BURN
ending leaves this required setup unpaid — a genuine dangling thread, not a
modelling artefact.

### Recorded-not-counted surfaces (payoff-coverage)

- `payoffs_to_unmarked`: **0** across all worlds.
- `payoff_before_setup`: **0** across all worlds.
- `unknown`: **0** across all worlds.
- `uncredited_edges`: **[]**.
- `undecidable_edges`: **[]**.

---

## 3. report-payoff-substantiation (deterministic, recorded-not-counted)

`setups_total = 8`. The substantiation surface re-checks whether each credited
payoff is discharged by a *typed state-change*. Several payoffs are typed as
beliefs/confessions rather than object state-changes and so register as
`unsubstantiated` (typed setup, hollow payoff — type the payoff as a state
change to discharge). These are recorded, not gated.

| world | substantiated | unsubstantiated | unverifiable |
|---|---|---|---|
| reveal | 3 | 5 | 0 |
| quiet | 5 | 3 | 0 |
| confront | 3 | 4 | 0 |
| burn | 4 | 3 | 0 |
| main | 0 | 0 | 0 |

Representative `unsubstantiated` pairs (recorded, not counted as defects):
- account: setup `fa-account-drained`/`fa-account-soft` → payoff
  `fa-pike-confesses-theft` (sc-09a, "He had taken the money.") — the payoff is
  a confession (object_state `confessed_theft`), not a state-change of the
  account itself, so it does not deterministically discharge the account-state
  setup.
- key: setup `fa-key-bare-hook` → payoff `fa-pike-kept-key` (sc-12a, "the key
  warm in his coat-lining all this month") / `fa-key-found-warm` (sc-11b, "drew
  out a key. ... The lost key was not lost.") — custody change, registers
  unsubstantiated against the object_state setup.
- `unverifiable` (untyped) = 0; every fact carries a typed leg.

---

## 4. report-irony-intervals — reader-knows / character-doesn't windows

`cross_frame_edges = 6`, `same_frame_edges = 0`.

**>= 1 reader-knows / character-doesn't (cross-frame divergence) window EXISTS
in EVERY world-line.** Window counts: reveal 6, confront 4, burn 4, main 3,
quiet 3. All windows are clean (windowless 0, unordered 0, undecidable 0).

Anchoring quotes for the principal divergences (all open from their start
coordinate forward):

- **Junia's false alibi vs. the blank plate.** junia-frame
  `fa-junia-alibi-claim` (sc-02, "She had been at the spectroscope, she would
  say. All night") against ground `fa-plate-blank` (sc-04, "the plate was
  *blank* ... never drawn back to the sky") and ground `fa-junia-on-gallery`
  (sc-02, "she had stood pressed to the curve of the wall with her breath
  held"). The reader knows from sc-02/sc-04 she was on the gallery; the claim
  stands as the alibi Hale receives.
- **Pike's false murderer-belief vs. ground.** pike-frame
  `fa-pike-believes-murderer` (sc-12a, "in his own mind, a murderer") against
  ground `fa-crane-never-read` (sc-11a, "Crane had never held it. Crane had
  never read a word of it.") and reveal-ground `fa-no-murder` (sc-17r, "Pike
  never laid a hand on him. ... it was no murder."). Window opens at sc-12a and
  runs to the certificate.
- **Hale's working frame vs. ground.** hale-frame `fa-hale-writes-fall` (sc-01,
  "the adjuster wrote *fall* in the first clean line of his book") against
  ground `fa-account-drained` (sc-08, "the account had been bled"); and
  confront-frame `fa-hale-pike-cause` (sc-09a, "The bursar was the cause of the
  death.") against ground `fa-no-murder` (sc-17r).

---

## 5. summary counts

| metric | value |
|---|---|
| facts | 60 |
| frames | 4 (ground, hale, junia, pike) |
| branches | 5 (main, confront, quiet, reveal, burn) |
| rules | 3 (crane-alive-arc / custody-exclusive / location-exclusive) |
| rule_transition_invalid | 0 |
| rule_exclusive_overlap | 0 |
| unchained_state_pairs | 0 |
| succession_cross_branch | 0 |
| SuccessionCycle | 0 |
| cross_scope_pairs | 6 |
| violation_count | 0 |
| payoff danglers (6 required) | unsent-letter dangles in the BURN world-line only |
| irony windows | >= 3 per world; present in every world-line |
