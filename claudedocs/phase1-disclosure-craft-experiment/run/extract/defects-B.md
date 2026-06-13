# defects-B — continuity / payoff / irony surfaces (blind re-extraction)

Store: `B.atomic.json`  ·  order: `B-order.json`  ·  rules: `B-rules.json`
Frames: ground, hale, junia, pike  ·  Branches: main, confront, quiet,
reveal_confront, reveal_quiet, burn
Facts: 73  ·  Rules: 3 (1 transition + 2 exclusive)

Every finding cites a scene-id (canon coordinate) + a quote from the page.

---

## 1. validate-continuity surface

`mnemosyne-cli validate-continuity --order B-order.json --rules B-rules.json`

| surface | count |
|---|---|
| rule_transition_invalid | 0 |
| rule_exclusive_overlap | **1** |
| unchained_state_pairs | 0 |
| succession_cross_branch | 0 (no distinct key surfaced; no cross-branch supersession violation) |
| SuccessionCycle | 0 (cycle-guarded import) |
| cross_scope_pairs | 6 (cross-frame conflict edges = data, not violations) |
| unordered_pairs | 0 |
| rule_unordered_pairs | 0 |
| violation_count | 1 |

### FINDING — rule_exclusive_overlap (location), reveal_quiet world

```
kind:   rule_exclusive_overlap
rule:   location-exclusive   (predicate located_at, per-subject)
frame:  ground
branch: reveal_quiet
fact_a: fb-pike-goes-study   (located_at = study_in_the_dark)
fact_b: fb-pike-on-stair-rq  (located_at = iron_stair)
at:     sc-17
```

- `fb-pike-goes-study` — sc-09b, "went by Hale's door with a shielded light,
  toward the director's study". (located_at: study_in_the_dark; branch quiet.)
- `fb-pike-on-stair-rq` — sc-17, "The man on the stair was Onslow Pike."
  (located_at: iron_stair; branch reveal_quiet.)

**What it surfaces (kept, not suppressed):** in the AUDIT→REVEAL world-line
(reveal_quiet descends from quiet), the ground frame holds Pike at two distinct
locations that overlap at coordinate sc-17. The two facts belong to two
different story-times collapsed onto the canon coordinate: the audit-present
(Pike walking to the study at ~2 a.m., sc-09b) is still open (no canon_to) when
the death-night testimony (Pike on the stair, recounted at sc-17) is asserted.
The location-exclusive rule correctly catches the un-bounded audit-present
location persisting into the testimony scene. This is the engine doing its job:
an open present-time location-state and a recounted past-night location-state
both claim Pike at sc-17. The confront-priored reveal world (reveal_confront)
does NOT trip it, because its prior-limb does not place Pike at a persisting
ground location before the testimony. Left in as a genuine surface; it would
be silenced by giving `fb-pike-goes-study` an explicit `canon_to`, but that
bound is not stated on the page, so it is not added.

The single transition rule (`crane-alive-arc`, allowed `[[alive,dead]]`) is
satisfied: `fb-crane-alive` (sc-04, "the old man on the iron stair at three in
the morning was a man with nothing left to lose") → `fb-crane-dead` (sc-02, "We
found him at the foot of it, beneath the great glass, at first light"),
chained, alive→dead. The custody-exclusive rule (held_by, per-subject) shows
no overlap: the night-log is held only by Junia (sc-11, "Junia had written the
night-log ... in her hand"), the spare key has no two-holder fact, the telegram
is held by Pike (sc-10b, "he put it back in the drawer"), the unsent letter by
Crane.

The 6 `cross_scope_pairs` are the authored cross-frame divergence edges — DATA,
never violations.

---

## 2. report-payoff-coverage — the 6 required setups

`setups_total = 7` (the 6 required; the night-log-wrong-hand setup is
instanced twice — once per investigative limb — because the story discovers it
independently in the CONFRONT limb (sc-11) and the QUIET limb (sc-11b)).

The 6 required setups, present and marked `payoff_expectation: expected`:

1. **night-log in a hand not Crane's** — confront-limb `fb-log-wrong-hand`,
   sc-11, "Junia had written the night-log ... in her hand"; quiet-limb
   `fb-log-found-quiet`, sc-11b, "The assistant had written the final
   night-log ... in her own hand".
2. **plate-camera with no exposures** — `fb-plates-blank`, sc-06,
   "Every plate was unexposed. ... It had not been opened to the sky."
3. **meridian clock stopped 3:14, wound only hours before** —
   `fb-clock-stopped`, sc-03, "Its hands stood at fourteen minutes past three.";
   `fb-clock-wound`, sc-03, "Then it had run some six hours and stopped."
4. **drained instrument account** — `fb-pike-drained`, sc-04, "Pike had
   drained it, a little and a little, over three years, into debts of his own".
5. **spare dome key reported lost a month before** — `fb-key-lost`, sc-08,
   "a spare dome key, reported lost a month past, lay even now where it should
   not lie".
6. **Crane's unsent letter resigning his post** — `fb-unsent-letter-exists`,
   sc-08, "a letter, never sent, in which Aurel Crane resigned his own post in
   his own hand".

### Dangling among the required setups (per world)

| world | paid | dangling |
|---|---|---|
| reveal_confront (WL-1 CONFRONT→REVEAL) | 6 | — none |
| reveal_quiet (WL-2 AUDIT→REVEAL) | 6 | — none |
| burn (WL-3 BURN) | 6 | — none |
| confront (CONFRONT pre fork-2) | 5 | **fb-clock-wound** |
| quiet (QUIET pre fork-2) | 5 | **fb-log-found-quiet** |
| main (shared spine only) | 1 | fb-clock-wound, fb-key-lost, fb-pike-drained, fb-unsent-letter-exists |

- All three TERMINAL world-lines (the actual endings sc-20 / sc-22 / sc-19b)
  pay off every required setup — no dangling in any ending.
- **fb-clock-wound dangles in the bare CONFRONT limb** (sc-09…sc-16): the
  clock-as-struck payoff is downstream in the reveal/burn limbs (sc-18,
  "stopped the pendulum dead with his own gripping hand"; sc-18b, "the thread
  of cloth caught in its works: real."), so the limb before fork-2 has it open.
- **fb-log-found-quiet dangles in the bare QUIET limb**: the quiet-limb's own
  night-log discovery (sc-11b) pays off only when the reader reaches the
  reveal_quiet testimony (sc-17, "The camera never opened. That is why the
  plates are blank."), downstream of fork-2.
- main: the four spine setups have their payoffs only in the limbs — expected.

### Recorded-not-counted surfaces (payoff-coverage)

- `payoffs_to_unmarked`: **0** across all worlds.
- `payoff_before_setup`: **0** across all worlds.
- `unknown`: **0** across all worlds.
- `uncredited_edges`: **[]** (after instancing the night-log setup per limb so
  the quiet-world payoff discharges the quiet-limb setup, not the confront-limb
  one).
- `undecidable_edges`: **[]**.

---

## 3. report-payoff-substantiation (deterministic, recorded-not-counted)

`setups_total = 7`. The substantiation surface re-checks whether each credited
payoff is discharged by a typed *state-change*. Many payoffs are typed as
hale/junia/pike beliefs or as relational reveals rather than state-changes of
the setup subject, so they register `unsubstantiated` (typed setup, hollow
payoff — type the payoff as a state-change to discharge). Recorded, not gated.

| world | substantiated | unsubstantiated | unverifiable |
|---|---|---|---|
| reveal_confront | 2 | 4 | 0 |
| reveal_quiet | 2 | 4 | 0 |
| burn | 2 | 4 | 0 |
| quiet | 2 | 3 | 0 |
| confront | 1 | 4 | 0 |
| main | 0 | 1 | 0 |

Representative `unsubstantiated` (recorded, not counted as defects):
- key: setup `fb-key-lost` (sc-08) → payoff `fb-key-behind-clock` (sc-12,
  "found now behind the very clock that a hand had stopped at three-fourteen")
  — a location reveal, not a state-change of the key, so does not
  deterministically discharge the object_state setup.
- night-log: setup `fb-log-wrong-hand` → payoffs `fb-junia-not-annexe-rc`
  (sc-17) + `fb-log-manufactured` (sc-11) — belief/relational reveals.
- plates: setup `fb-plates-blank` → `fb-junia-not-at-camera` (sc-06, "or you
  were not at your camera at all"), a Hale belief.
- `unverifiable` (untyped) = 0; every fact carries a typed leg.

---

## 4. report-irony-intervals — reader-knows / character-doesn't windows

`cross_frame_edges = 6`, `same_frame_edges = 0`.

**>= 1 reader-knows / character-doesn't (cross-frame divergence) window EXISTS
in EVERY world-line.** Window counts: reveal_confront 5, reveal_quiet 5,
burn 4, confront 4, quiet 4, main 4. All windows clean (windowless 0,
unordered 0, undecidable 0).

Anchoring quotes for the principal divergences (this story states them outright
in its preamble as S1 and S2):

- **S1 — Pike's false belief vs. ground.** pike-frame `fb-pike-believes-knew`
  (sc-04, "had come to believe, on the night Crane died, that Crane knew")
  against ground `fb-crane-never-knew` (sc-04, "Crane never knew."); extended
  in the reveal by pike-frame `fb-pike-certain-rc` (sc-18, "Certain that Crane
  had read some recall, some dismissal") against ground `fb-crane-bewildered-rc`
  (sc-18, "Read what, man? Known what?").
- **S2 — Junia's false alibi vs. ground.** junia-frame `fb-junia-alibi`
  (sc-05, "I was at the spectroscope ... From eleven until dawn.") against
  ground `fb-junia-on-gallery` (sc-05, "She had been above, on the dark
  gallery ... and she had seen the whole of it.") and ground `fb-plates-blank`
  (sc-06, "Every plate was unexposed.").
- **Hale's frame vs. ground motive.** hale-frame `fb-hale-distrust` (sc-01,
  "to distrust a thing that arranged itself too neatly into an accident")
  against ground `fb-pike-drained` (sc-04, "Pike had drained it, a little and a
  little, over three years").

---

## 5. summary counts

| metric | value |
|---|---|
| facts | 73 |
| frames | 4 (ground, hale, junia, pike) |
| branches | 6 (main, confront, quiet, reveal_confront, reveal_quiet, burn) |
| rules | 3 (crane-alive-arc / custody-exclusive / location-exclusive) |
| rule_transition_invalid | 0 |
| rule_exclusive_overlap | 1 (Pike: study vs stair, reveal_quiet, at sc-17) |
| unchained_state_pairs | 0 |
| succession_cross_branch | 0 |
| SuccessionCycle | 0 |
| cross_scope_pairs | 6 |
| violation_count | 1 |
| payoff danglers (required setups) | none in any terminal world; limb-local danglers (clock in bare CONFRONT, night-log in bare QUIET) |
| irony windows | >= 4 per world; present in every world-line |
