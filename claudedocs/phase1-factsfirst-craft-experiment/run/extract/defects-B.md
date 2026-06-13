# Defects report — Story B (blind re-extraction)

Store: `B.atomic.json` · canon order: `B-order.json` · rules: `B-rules.json`

Registered: 81 facts · 4 frames (`ground`, `pike`, `hale`, `junia`) ·
5 branches (`confront`, `audit`, `loud-reveal`, `quiet-reveal`, `burn`) ·
33 sections · 11 entities · 4 predicates
(`is_alive`, `at_location`, `holds_log`, `holds_key`) · 8 typed facts ·
5 conflict edges · 4 rules.

Branch tree (forks_from / forks_at = last shared scene):
- `confront` forks from `main` at **sc-08** (fork-1 CONFRONT limb, sc-09..sc-16).
- `audit` forks from `main` at **sc-08** (fork-1 QUIET-AUDIT limb, sc-09b..sc-16b).
- `loud-reveal` (WL-1) forks from `confront` at **sc-16** (sc-17,sc-18,sc-19,sc-20).
- `quiet-reveal` (WL-2) forks from `audit` at **sc-16b** (sc-17,sc-18,sc-21,sc-22).
- `burn` (WL-3) forks from `confront` at **sc-16** (sc-17b,sc-18b,sc-19b).

---

## 1. validate-continuity

Command: `validate-continuity --order B-order.json --rules B-rules.json`
Result: facts=81 · order_nodes=33 · rules=4 · **violations: 0 (structural=0 interval=0)**.

| surface | count |
|---|---|
| `rule_transition_invalid` | **0** |
| `rule_exclusive_overlap` | **0** |
| `unchained_state_pairs` | **0** |
| `succession_cross_branch` | **0** |
| `SuccessionCycle` | **0** |

Supporting surfaces: `conflict_pairs_checked=5`, `cross_scope_pairs=5`
(all five conflicts cross FRAMES — belief vs ground — so they are DATA,
never gated), `unordered_pairs=0`, `interval_unverifiable=0`.

No continuity defect found. Each evaluated invariant holds:

- **alive-arc (transition, `is_alive`)** — the only character who dies is
  Crane, present in the store only as `dead`
  (sc-02 "We found him at the foot of it, beneath the great glass, at first
  light"). No typed `alive -> dead` succession edge exists, so the rule has
  nothing to evaluate; 0.
- **night-log-custody (exclusive, `holds_log` per object)** — the night-log
  is held by exactly one party: Junia, who picks it up to burn it
  (sc-17b "she picked up the night-log ... and she fed it to the fire").
  0 overlaps.
- **spare-key-custody (exclusive, `holds_key` per object)** — the spare key
  is attributed only to Pike (sc-20 "the means by which Pike, who had taken
  it a month before, came and went and locked the dome"). Crane's key is a
  distinct object on his person (sc-12 "Crane's, on his person"). 0 overlaps.
- **location-excl (exclusive, `at_location` per subject)** — the text asserts
  simultaneous presence at ~3:14 (Junia on the gallery, Pike on the stair;
  sc-17 "I was on the gallery ... The man on the stair was Onslow Pike").
  No single subject is placed in two locations at one canon point; 0.

---

## 2. report-payoff-coverage (the six required setups)

Command: `report-payoff-coverage --order B-order.json`
setups_total=9 (the six required, with the night-log, spare-key and
resignation each established once per fork-1 limb — confront and audit —
since each playthrough re-establishes them) · uncredited_edges=**0** ·
undecidable_edges=**0**.

The six required setups and the in-store setup fact(s) each is bound to:

1. night-log in the wrong hand — confront limb `b-s11-log-wrong-hand`
   (sc-11 "it was not in Crane's hand") / audit limb `b-s11b-log-wrong-hand`
   (sc-11b "the last entry, the night of the death, ran in another hand
   entirely").
2. plate-camera with no exposures — `b-s06-plates-blank`
   (sc-06 "Every plate was unexposed. The camera had not photographed a
   star, nor anything, that night.").
3. meridian clock stopped at 3:14, wound hours before — `b-s03-clock-stopped-314`
   (sc-03 "Its hands stood at fourteen minutes past three"; wound the evening
   before, sc-03 "Crane wound it himself. The evening before").
4. drained instrument account — `b-s04-pike-drained-fund`
   (sc-04 "Pike had drained it, a little and a little, over three years, into
   debts of his own").
5. spare dome key reported lost a month before — confront limb
   `b-s12-key-lost-claim` (sc-12 "that one was lost, a month back, and never
   found") / audit limb `b-s12b-key-lost-claim` (sc-12b "the one Pike had
   said was lost a month gone, never found").
6. director's unsent resignation — confront limb `b-s14-resignation-found`
   (sc-14 "It was complete. It was signed. And it had never been sent.") /
   audit limb `b-s13b-resignation-found` (sc-13b "Complete. Signed. Never
   sent.").

### Dangling-required-setup count per world-line

| world-line | dangling among the 6 | note |
|---|---|---|
| `loud-reveal` (WL-1, ending sc-20) | **0** | all six paid |
| `quiet-reveal` (WL-2, ending sc-22) | **0** | all six paid |
| `burn` (WL-3, ending sc-19b) | **0** | all six paid |
| `confront` (intermediate limb) | 3 | clock, plate, log — paid on continuations |
| `audit` (intermediate limb) | 2 | clock, plate — paid on continuations |
| `main` (shared trunk only) | 2 | clock, plate |

The danglings on `confront`, `audit`, `main` are projection artifacts of
intermediate (non-terminal) worlds: those branches stop before the reveal/
burn endings where the clock, plate, log, key and resignation setups are
discharged. Every TERMINAL world-line pays off all six. Crediting evidence:

- `loud-reveal`: clock <- sc-18 ("stopped the pendulum dead ... your clock
  at three-fourteen"); plate <- sc-17 ("The camera never opened. That is why
  the plates are blank"); log <- sc-20 ("written a false night-log in her own
  hand"); key <- sc-20 ("Pike, who had taken it a month before, came and went
  and locked the dome"); resignation <- sc-19/sc-20; account <- sc-09/sc-20.
- `quiet-reveal`: clock <- sc-18 / sc-22; plate <- sc-17 / sc-22;
  log <- sc-11b / sc-22; key <- sc-12b / sc-22; resignation <- sc-13b / sc-22
  ("He never read your wire. He had written this — three days before");
  account <- sc-07 / sc-22.
- `burn`: clock <- sc-18b / sc-19b ("agent unestablished" — clock certified
  as a human hand); plate <- sc-18b / sc-19b; log <- sc-17b ("she fed it to
  the fire") / sc-19b ("the false night-log (destroyed, he wrote, in his
  presence, by the witness who had written it)"); key <- sc-12 / sc-18b /
  sc-19b; resignation <- sc-14 / sc-18b / sc-19b; account <- sc-07 / sc-09 /
  sc-18b / sc-19b.

---

## 3. Recorded-not-counted surfaces

| surface | count | detail |
|---|---|---|
| `payoffs_to_unmarked` | **0** | every `pays_off` targets a setup marked `expected` |
| `payoff_before_setup` | **0** | — |
| `cross_scope_pairs` | **5** | the five cross-frame conflict pairs (belief vs ground) |
| `undecidable` / `unknown` | **0** | every fact resolves In/Out in every world |
| `uncredited_edges` | **0** | every `pays_off` credits a setup in some world |

### Cross-scope (cross-frame) conflict pairs — recorded as data, not gated

1. `b-s04-pike-believes-crane-knew` (pike) vs `b-s04-crane-never-knew`
   (ground) — sc-04 "had come to believe ... that Crane had read his
   dismissal" against sc-04 "Crane never knew."
2. `b-s05-junia-claims-spectroscope` (junia) vs `b-s05-junia-on-gallery`
   (ground) — sc-05 "I was at the spectroscope ... From eleven until dawn"
   against sc-05 "She had been above, on the dark gallery."
3. `b-s05-junia-claims-spectroscope` (junia) vs `b-s06-plates-blank` (ground)
   — sc-05 alibi against sc-06 "Every plate was unexposed."
4. `b-s06-junia-shutter-lie` (junia) vs `b-s06-plates-blank` (ground) —
   sc-06 "The shutter jammed ... I worked, and got nothing" against sc-06
   "the camera not opened to the sky."
5. `b-s10-pike-recall-slip` (pike) vs `b-s04-crane-never-knew` (ground) —
   sc-10 "The recall came, and he read it, and he knew he was finished"
   against sc-04 "Crane never knew."

---

## 4. Adapter decisions (extraction notes, not story defects)

- **Re-converging world-lines forced a single-parent tree choice.** Story B's
  own map (sc-515..sc-517 in story-B.md) shares the reveal testimony scenes
  sc-17/sc-18 between WL-1 (CONFRONT prior) and WL-2 (AUDIT prior), and lets
  WL-3 BURN be reached from EITHER fork-1 limb — a DAG, not a tree. Because a
  fact's `branch` is single-valued, each complete world-line is encoded as
  its own branch and the shared reveal scenes are authored once per world-line
  (loud-reveal carries `b-s17-lr-*`/`b-s18-lr-*`; quiet-reveal carries
  `b-s17-qr-*`/`b-s18-qr-*`), with identical quotes per the text's note
  "The testimony is the same; the listener is not" (sc-389). `burn` is modeled
  as forking from `confront` (its first-listed prior); the canon order
  additionally records the in-edges `sc-16b -> sc-17`, `sc-16b -> sc-17b`
  to keep the declared scene graph honest.
- Each fork-1 limb (confront / audit) establishes its OWN copy of the three
  limb-local setups (night-log, spare-key, resignation), so setups_total=9
  while each terminal world-line still sees exactly its six required setups.
- A single per:subject `at_location` exclusive rule covers all principals;
  custody uses two object-scoped predicates (`holds_log`, `holds_key`) so the
  night-log and spare-key each carry a dedicated exclusive rule. Total = 4
  rules (under 8).
- The undelivered forged telegram is the dramatic-irony mechanism, not one of
  the six required setups; it is stored as ground-truth facts and conflict
  pairs without a `payoff_expectation`.
