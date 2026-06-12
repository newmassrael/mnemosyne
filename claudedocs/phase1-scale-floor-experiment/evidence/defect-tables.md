# Belvoir — Blind Extraction Defect Tables

Both novellas were re-extracted into fresh Mnemosyne narrative stores under the
**single identical procedure** of `BRIEF.md` (frames → typing-discovery loop →
scene-by-scene `import-facts` → edge-discovery loop → declared canon order →
branches), the rules derived by the one fixed recipe, and the deterministic gates
run on each store. Stores: `store-A/` (`story-A.atomic.json`) and `store-B/`
(`story-B.atomic.json`); each with its own `mnemosyne.toml`, `canon-order.json`,
`narrative-rules.json`. Tooling: `mnemosyne-cli 0.1.0 (665570cd)`. Full command
log: `extraction-log.md`.

Metric definitions (per brief):
- **D1** — gated rule violations: `rule_transition_invalid` + `rule_exclusive_overlap`, all worlds (from `validate-continuity`).
- **D2** — `unchained_state_pairs`, all worlds (from `validate-continuity`).
- **D3** — dangling among the 12 REQUIRED setups at story end, counted per **ending** world-line where the setup appears (from `report-payoff-coverage`).
- **D4** — fork-boundary faults: `succession_cross_branch` + `cycle`/`SuccessionCycle`, all worlds (from `validate-continuity`).
- **Primary endpoint** = D1 + D2 + D3 + D4 per story.

Both stories share the same structure: trunk `sc-01..sc-20`; **Fork 1** at `sc-20`
→ CONFRONT vs QUIET; QUIET runs `sc-26..sc-40`; **Fork 2** at `sc-40` → CONFESS vs
ESCALATE. Three ending world-lines each: **A = confront** (suicide), **B =
quiet→confess** (suicide), **C = quiet→escalate** (murder). The ground truth of
Roeder's death legitimately diverges across world-lines (suicide on A/B, murder on
C); this is handled by branch-scoping each fact, so it surfaces as **no** conflict
(`cross_scope_pairs = 0`), not as a defect.

---

## Headline — story-A and story-B side by side

| Metric | story-A | story-B |
|---|---|---|
| **D1** — gated rule violations | **0** | **0** |
| **D2** — `unchained_state_pairs` | **0** | **0** |
| **D3** — dangling required setups (ending world-lines) | **3** | **5** |
| **D4** — fork-boundary faults | **0** | **0** |
| **Primary endpoint (D1+D2+D3+D4)** | **3** | **5** |
| Rule count (recipe) | **9** = 3 alive-arc + 6 exclusive-custody + 0 location | **9** = 2 alive-arc + 7 exclusive-custody + 0 location |
| Rules in file (engine, per-subject) | 2 | 2 |
| Facts / scenes / world-lines | 82 / 60 / 3 | 96 / 65 / 3 |

### Recorded, NOT counted (honesty surfaces — both stories)

| Surface | story-A | story-B |
|---|---|---|
| `payoffs_to_unmarked` | 0 | 0 |
| `payoff_before_setup` | 0 | 0 |
| `cross_scope_pairs` | 0 | 0 |
| `unordered_pairs` / `rule_unordered_pairs` | 0 / 0 | 0 / 0 |
| `succession_gaps` (edge-candidates) | 0 | 0 |
| `undecidable_edges` / per-world `unknown` (payoff report) | 0 / 0 | 0 / 0 |
| typing/edge proposals rejected (discovery `undecidable`) | 0 / 0 | 0 / 0 |
| intermediate-scope dangling (by construction, not counted) | trunk dangles all 12; quiet dangles 9 | trunk & quiet each dangle all 12 |

---

## D3 detail — required-setup payoff matrix

Legend: a fact id = the scene that pays the setup off **on that ending world-line**;
**DANGLING** = no payoff on that world-line. (Intermediate branches trunk/quiet are
excluded from D3 — their dangling is by construction, payoffs lie downstream.)

### story-A (endings: confront A · quiet-confess B1 · quiet-escalate B2)

| # | Required setup (plant) | A confront | B1 confess | B2 escalate |
|---|---|---|---|---|
| 1 | morphine ledger discrepancy (sc-07) | sc-22 | sc-57 | sc-50 |
| 2 | chart burned in the stove (sc-03) | sc-24 | sc-44 | sc-50 |
| 3 | Roeder's locked diary (sc-02) | sc-51 | sc-34 | sc-34 |
| 4 | forged admission papers (sc-05) | **DANGLING** | sc-41 | sc-49 |
| 5 | master key's whereabouts (sc-02) | sc-23 | sc-43 | sc-49 |
| 6 | last telegram before line cut (sc-01) | sc-54 | sc-43 | sc-49 |
| 7 | poison-suspect vial / digitalin (sc-04) | sc-56 | sc-43 | sc-50 |
| 8 | register name ↔ burial record (sc-16) | **DANGLING** | sc-28 | sc-28 |
| 9 | unsent resignation letter (sc-08) | sc-52 | sc-43 | sc-50 |
| 10 | gap in funicular log (sc-12) | sc-55 | sc-33 | sc-33 |
| 11 | second patient's abrupt change (sc-13) | sc-53 (lives) | sc-44 (lives) | sc-47 (dies) |
| 12 | snow-wet boots, owner denies (sc-17) | **DANGLING** | sc-42 | sc-49 |
| | **dangling count** | **3** | **0** | **0** |

story-A D3 = **3**, all on the **confront** ending. ENDING A's own prose names the
gap (sc-56): on the confront-spine Cendre solves "every truth the house contained
but one" — she never connects the too-sound lungs/forged file (#4), the née-column
*Cordier* (#8), or the wet boots (#12) to the woman in room fourteen.

### story-B (endings: confront A · confess B · escalate C)

| # | Required setup (plant) | A confront | B confess | C escalate |
|---|---|---|---|---|
| 1 | morphine ledger discrepancy (sc-10) | sc-21 | sc-57 | sc-63 |
| 2 | chart burned in the stove (sc-05) | sc-22 | **DANGLING** | **DANGLING** |
| 3 | Roeder's locked diary (sc-02) | sc-51 | sc-56 | **DANGLING** |
| 4 | forged admission papers (sc-06) | sc-53 | sc-42 | **DANGLING** |
| 5 | master key's whereabouts (sc-02) | sc-54 | sc-57 | sc-61 |
| 6 | last telegram before line cut (sc-01) | sc-54 | sc-58 | sc-62 |
| 7 | poison-suspect vial / morphia bottle (sc-04) | sc-54 | sc-57 | sc-61 |
| 8 | register name ↔ burial record (sc-10) | sc-53 | sc-42 | **DANGLING** |
| 9 | unsent resignation letter (sc-08) | sc-54 | sc-57 | sc-64 |
| 10 | gap in funicular log (sc-09) | sc-53 | sc-58 | sc-62 |
| 11 | second patient's abrupt change (sc-10) | sc-54 (lives) | sc-59 (lives) | sc-48 (dies) |
| 12 | snow-wet boots, owner denies (sc-09) | sc-53 | sc-58 | sc-62 |
| | **dangling count** | **0** | **1** | **4** |

story-B D3 = **5**: 1 on **confess** (the burned chart — sc-59 states "the burning
itself stayed outside her knowing … the one secret of the house that even this
gentlest solving never reaches") and 4 on **escalate** (the murder ending, where the
killer escapes unidentified): the diary is **never opened** (sc-64 "she never opened
it … the case kept its hole"), the forged papers stay "unsurrendered and unguessed"
(sc-65), the burned chart "never guessed had gone to the stove" (sc-63), and the
register↔burial kinship "stayed the reader's and not hers" (sc-63).

---

## B-1 honesty — the one genuinely ambiguous classification (affects D3 only)

D3 is the only metric that turns on a judgment call: **what counts as a "payoff"**
when a world-line's closing ground-truth narration tells the reader a truth that the
investigator (Cendre) never reaches. Both stories do this on their failing ending,
and I applied **one rule to both**.

- **Reading 1 — investigator-resolution (PRIMARY, used above).** A setup pays off
  only where the world-line's own scenes *discharge* it (a character resolves/acts on
  it, a confession delivers it, or the narrator states the resolution as an event).
  A thread the text explicitly flags as an unresolved gap ("never," "the case kept
  its hole," "stayed outside her knowing," "unguessed") **dangles**. This matches the
  saltglass precedent, whose payoffs were resolution events. → **A = 3, B = 5.**

- **Reading 2 — reader-delivery (ALTERNATIVE).** A setup pays off if its truth is
  delivered to the reader on that world-line at all, even via an omniscient aside the
  investigator misses. Under this reading the only genuinely un-delivered setup is
  **story-B's diary on the escalate line** (never opened, so its contents reach no one
  on that world-line). → **A = 0, B = 1.**

Under **either** reading, story-B carries the higher D3 than story-A, and the gap is
driven by the same structural fact: story-A's murder ending (C) has Cendre confront
the in-house murderer (the matron) and resolve every thread, whereas story-B's murder
ending (C) has the murderer (the false-name patient) keep silence and walk down
"unnamed, unproven," leaving the diary, papers, burned chart and name-match
unresolved. I have flagged the ambiguous setups in the matrices rather than forcing a
single classification.

(Note for context, not a graded input: story-B's source carries authoring HTML
comments asserting "payoff 12/12 in confront/confess/escalate." A blind re-extraction
of the *prose* does not reproduce 12/12 on confess/escalate under either reading; the
text explicitly leaves those threads as gaps for the investigator.)

---

## Rule derivation (one fixed recipe, both stories)

- one **alive-arc TRANSITION** rule per character who dies on any limb;
- one **exclusive-custody EXCLUSIVE** rule per unique physical object two parties hold at different points;
- one **location-exclusive EXCLUSIVE** rule per principal only where the text asserts simultaneous presence.

The engine keys `transition`/`exclusive` rules per **(predicate, subject)** via
`per:"subject"`, so the recipe's per-character / per-object rules are realised as
**two** rules in each file (one `transition` on `life_state`, one `exclusive` on
`held_by`), instantiated per subject by the engine. The recipe **count** (which the
brief asks for, and which scales with cast and objects) is reported per story:

- **story-A = 9**: alive-arc subjects {augustin-roeder, marguerite-seve,
  ottilie-brandt} = 3; exclusive-custody objects {morphine-ledger, master-key,
  digitalin-vial, roeder-diary, resignation-letter, cordier-chart} = 6;
  location-exclusive = 0 (no asserted simultaneous presence).
- **story-B = 9**: alive-arc subjects {augustin-roeder, edvard-lindqvist} = 2
  (Nurse Brandt dies on no limb); exclusive-custody objects {morphine-ledger,
  brown-bottle, roeder-diary, resignation-letter, forged-admission-papers,
  snow-wet-boots, burned-chart} = 7 (the **master key is excluded** — single
  custodian, the steward Gaspard); location-exclusive = 0.

The differing object/subject sets are faithful to the texts, not procedure drift:
e.g. story-A's master key changes hands (matron's cabinet → Cendre) so it qualifies,
while story-B's never leaves the steward; story-B's boots and forged papers are
physically passed between parties (Gaspard→Cendre; Hélène→Cendre) so they qualify,
while story-A's stay with one holder. D1 = 0 in both: every custody chain and every
alive→dead transition closes via a narrated succession edge (no overlap, no
unchained pair).

---

## Summary

| Story | D1 | D2 | D3 | D4 | **Primary endpoint** | recipe rules |
|---|---|---|---|---|---|---|
| story-A | 0 | 0 | 3 | 0 | **3** | 9 |
| story-B | 0 | 0 | 5 | 0 | **5** | 9 |

Both stores pass every continuity gate (no rule violations, no unchained state
pairs, no fork-boundary faults, no cross-scope conflicts). The entire signal is in
**D3**: both stories deliberately leave their false-name-patient thread unresolved on
one ending, but story-B leaves **more** required setups dangling on its murder ending
(where the killer escapes the investigation) than story-A does on its incomplete
confront ending. All recorded-not-counted honesty surfaces are 0.
