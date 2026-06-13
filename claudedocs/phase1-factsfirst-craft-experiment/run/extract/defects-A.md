# Defects report — Story A (blind re-extraction)

Store: `A.atomic.json` · canon order: `A-order.json` · rules: `A-rules.json`

Registered: 74 facts · 4 frames (`ground`, `pike`, `hale`, `junia`) ·
4 branches (`confront`, `audit`, `reveal`, `burn`) · 29 sections ·
11 entities · 4 predicates (`is_alive`, `at_location`, `holds_log`, `holds_key`) ·
6 typed facts · 5 conflict edges · 4 rules.

Branch tree (forks_from / forks_at = last shared scene):
- `confront` forks from `main` at **sc-08** (fork-1, CONFRONT limb).
- `audit` forks from `main` at **sc-08** (fork-1, QUIET-AUDIT limb).
- `reveal` forks from `confront` at **sc-16** (fork-2 REVEAL).
- `burn` forks from `confront` at **sc-16** (fork-2 BURN).

---

## 1. validate-continuity

Command: `validate-continuity --order A-order.json --rules A-rules.json`
Result: facts=74 · order_nodes=29 · rules=4 · **violations: 0 (structural=0 interval=0)**.

| surface | count |
|---|---|
| `rule_transition_invalid` | **0** |
| `rule_exclusive_overlap` | **0** |
| `unchained_state_pairs` | **0** |
| `succession_cross_branch` | **0** |
| `SuccessionCycle` | **0** |

Supporting surfaces: `conflict_pairs_checked=5`, `cross_scope_pairs=5`
(all five recorded conflicts are cross-FRAME — belief frame vs ground —
so they are DATA, never gated, under frame-scoped continuity),
`unordered_pairs=0`, `interval_unverifiable=0`.

No continuity defect found. Each evaluated narrative invariant holds:

- **alive-arc (transition, `is_alive`)** — the only character who dies is
  Crane. He is present in the store solely as `dead`
  (sc-01 "the director lay where he had fallen from the dark top of it");
  no typed `alive -> dead` succession edge exists, so the transition rule
  has nothing to evaluate and reports 0 — there is no out-of-allowed step.
- **night-log-custody (exclusive, `holds_log` per object)** — the night-log
  is held by exactly one party at a time: Junia, who takes it up to burn it
  (sc-17w "she lifted the book from the desk ... and fed the night-log into
  them"). No two parties hold it simultaneously; 0 overlaps.
- **spare-key-custody (exclusive, `holds_key` per object)** — the spare key
  is held only by Pike (sc-07 "It was warm in the lining of Onslow Pike's
  own coat"; sc-11b he draws it to open the lower door). Crane's key is a
  distinct object on the body (sc-07 "The director's key hung on the dead
  man's ring"). No overlap; 0.
- **location-excl (exclusive, `at_location` per subject)** — the text asserts
  simultaneous presence at ~3:14 (Junia on the gallery, Pike on the stair,
  Crane at the rail; sc-17r "She had been on the dome gallery, twelve feet
  above the dead man's place ... It was Onslow Pike"). No single subject is
  ever placed in two locations at one canon point; 0 overlaps.

---

## 2. report-payoff-coverage (the six required setups)

Command: `report-payoff-coverage --order A-order.json`
setups_total=6 · uncredited_edges=**0** · undecidable_edges=**0**.

The six required setups, with the in-store setup fact each is bound to:

1. night-log in the wrong hand — `a-s03-log-wrong-hand`
   (sc-03 "it was not Crane's hand").
2. plate-camera with no exposures — `a-s04-plate-blank`
   (sc-04 "unmarked, undeveloped, blank as the snow outside. No star had
   been drawn upon it.").
3. meridian clock stopped at 3:14, wound only hours before —
   `a-s05-clock-stopped-314` (sc-05 "The meridian clock had stopped at
   fourteen minutes past three"; wound at midnight, sc-05 "the maintenance
   book showed the clock wound at midnight — Pike's own initials").
4. drained instrument account — `a-s01-pike-drained`
   (sc-01 "He had drawn the money out of the instrument account in careful
   spoonfuls across two years").
5. spare dome key reported lost a month before — `a-s07-spare-key-reported-lost`
   (sc-07 "reported lost, a month before the death, in Pike's careful
   column-hand").
6. director's unsent resignation — `a-s06-resignation-found`
   (sc-06 "it was a resignation. The director laid down his post in it").

### Dangling-required-setup count per world-line

| world-line | dangling among the 6 | note |
|---|---|---|
| `reveal` (terminal ending sc-19r) | **0** | all six paid |
| `burn` (terminal ending sc-18w) | **0** | all six paid |
| `audit` (terminal ending sc-20b) | **0** | all six paid |
| `confront` (intermediate spine) | 0 | (not a terminal ending; sc-09a..sc-16) |
| `main` (shared trunk only) | **4** | clock, plate, log, resignation |

The `main` dangling (4) is a projection artifact of the bare trunk world:
`main` is the shared spine sc-01..sc-08 (before any fork), and four of the
six setups are paid only on the forked limbs that continue past sc-08. Every
TERMINAL world-line (`reveal`, `burn`, `audit`) pays off all six. Sample
crediting evidence:

- `reveal`: log <- sc-16 ("the night-log was her hand because Crane's fingers
  had stiffened"); plate <- sc-14 ("blank as the snow on the dome");
  clock <- sc-15 ("the moment the director fell: fourteen minutes past three");
  key <- sc-18r; resignation <- sc-19r; account <- sc-09a.
- `burn`: account <- sc-18w ("It had left him the fraud, whole and proven");
  key <- sc-18w; plate <- sc-14; clock <- sc-15; log <- sc-17w
  ("fed the night-log into them"); resignation <- sc-11a.
- `audit`: account <- sc-09b; plate <- sc-17b; clock <- sc-18b;
  log <- sc-18b ("a kindness, taken down at the eyepiece by the assistant");
  key <- sc-11b/sc-19b; resignation <- sc-19b.

---

## 3. Recorded-not-counted surfaces

| surface | count | detail |
|---|---|---|
| `payoffs_to_unmarked` | 5 edges (across worlds) | see below |
| `payoff_before_setup` | **0** | — |
| `cross_scope_pairs` | **5** | the five cross-frame conflict pairs (belief vs ground) |
| `undecidable` / `unknown` | **0** | every fact resolves In/Out in every world |
| `uncredited_edges` | **0** | every `pays_off` credits a setup in some world |

`payoffs_to_unmarked` (a `pays_off` edge whose target is a real fact that
carries no `payoff_expectation=expected`). All point at two non-required
beats and are honest surfaces, not contradictions:

- `audit`: `a-s09b-theft-reconstructed` -> `a-s08-account-drained-proven`
  (sc-09b "built the theft back up in his own neat columns"; target sc-08
  "a sum gone out of the instrument account that had bought no instrument").
- `confront` / `reveal` / `burn`: `a-s09a-pike-confesses-theft-forgery` ->
  `a-s08-account-drained-proven` (sc-09a "the drained account and the forged
  telegram both").
- `burn`: `a-s18w-fraud-stands` -> `a-s08-account-drained-proven`
  (sc-18w "It had left him the fraud, whole and proven").
- `reveal`: `a-s18r-account-closes` -> `a-s11a-telegram-found-sealed`
  (sc-18r "no man's deliberate hand had put Crane over the rail"; target
  sc-11a "sealed and addressed and undelivered").

These are payoffs to secondary beats (Hale's proven-account fact `a-s08`,
the sealed-telegram fact `a-s11a`) deliberately NOT marked among the six
required setups; the system records them and counts them apart, as designed.

### Cross-scope (cross-frame) conflict pairs — recorded as data, not gated

All five conflicts the store records sit across frames, so frame-scoped
continuity treats them as DATA (the standing dramatic irony), never a
violation:

1. `a-s01-pike-believes-delivered` (pike) vs `a-s01-telegram-undelivered`
   (ground) — sc-01 "Pike believed the telegram had reached Crane" against
   sc-01 "It lay in the iron post-box at the foot of the snow-cut road".
2. `a-s01-pike-believes-delivered` (pike) vs `a-s01-crane-never-saw` (ground)
   — sc-01 against "Crane had never seen it. Crane never knew ... that any
   man wished him gone."
3. `a-s04-junia-tells-spectroscope` (junia) vs `a-s04-plate-blank` (ground)
   — sc-04 "she had been at the spectroscope through the whole of the night"
   against sc-04 "No exposure had been made the whole night through."
4. `a-s05-hale-reads-clock-told` (hale) vs `a-s05-clock-stopped-by-fall`
   (ground) — sc-05 "a clock that had been told to stop" against sc-05 "It
   had recorded, to the minute, the instant Crane fell."
5. `a-s10a-pike-describes-telegram` (pike) vs `a-s11a-telegram-found-sealed`
   (hale) — sc-10a "Crane had read it that evening" against sc-11a "sealed
   and addressed and undelivered ... never read."

---

## 4. Adapter decisions (extraction notes, not story defects)

- The shared spine sc-01..sc-08 carries on the default `main` branch; the
  two fork-1 limbs (`confront`, `audit`) fork from it at sc-08; the two
  fork-2 world-lines (`reveal`, `burn`) fork from `confront` at sc-16. This
  matches the story's own map (sc-403 in story-A.md) exactly — a clean tree,
  no re-convergence.
- A single per:subject `at_location` exclusive rule enforces location-
  exclusivity for every principal (the rule mechanism keys on predicate+per,
  so one rule already covers all subjects); custody is split into two
  object-scoped predicates (`holds_log`, `holds_key`) so the night-log and
  spare-key each carry a dedicated exclusive rule. Total = 4 rules (under 8).
- The undelivered forged telegram is the central dramatic-irony mechanism
  but is NOT one of the six required setups, so it is stored as ground-truth
  facts and conflict pairs without a `payoff_expectation`.
