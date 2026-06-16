# Author-expand log — "The Drowned Hold of Vael Mooren" (v3)

An EXPANSION of the v2 gate-clean fact base: the last delver's investigation is
now an on-page PROLOGUE the reader witnesses first-hand, authored WITHOUT
spoiling the villain's identity (which stays first-knowable at sc-14). The v2
spine, fork, quests, map surfaces, possession rule, and every road's ending are
untouched; the only existing fact changed is `f-130`'s payoff re-point.

Base went from 123 → 136 facts (13 new prologue facts) and 52 → 56 scenes (4 new
prologue scenes). One frame added (`delver`). All v2 gates re-run clean.

---

## Prologue scenes added (sort BEFORE sc-01, prepended to canon order)

Four sections, ids that sort ahead of `sc-01`:

- `sc-00a` — PROLOGUE — Coombe marks the fresh tithes in the floodwater
- `sc-00b` — PROLOGUE — a robed figure tips the dead into the sluice by night
- `sc-00c` — PROLOGUE — Coombe follows the trail toward the upper town
- `sc-00d` — PROLOGUE — cut off in the under-ways, the last entry

`order.json`: prepended the chain `sc-00a -> sc-00b -> sc-00c -> sc-00d -> sc-01`.
The existing `sc-01 -> sc-02 -> ...` spine and the three branch chains are
byte-identical to v2. Confirmed reached in order via the reading walk:
`sc-00a -> sc-00b -> sc-00c -> sc-00d -> sc-01 -> sc-02 -> ...`.

## Frame added

`delver` — "Coombe, the last delver — his hunt for the tithe's source, before the
party". (The `delver` ENTITY already existed in the base; only the FRAME is new.)

## Prologue facts added (13)

The delver's observations/beliefs in frame `delver`; the ground-truth unnamed
deed in frame `gt` (consistent with the base's already-public "someone living
feeds the water" — Maeve's f-051). The robed figure is referenced ONLY as "a
robed figure" / "someone of the town" / "the hand behind the new tithes".

sc-00a (the new tithes):
- `f-pa1` [delver] — Coombe goes down alone to track where the fresh tithes come from
- `f-pa2` [delver] — the dead are too fresh; reckons a living hand is adding corpses
- `f-pa3` [gt] — new corpse-tithes are being given to the floodwater by a living hand

sc-00b (the deed, watched):
- `f-pb1` [gt] — by night a robed figure tips bodies into a sluice, feeding the dead
- `f-pb2` [delver] — hidden, Coombe watches the robed figure and finds the hand behind the tithes
- `f-pb3` [delver] — he keeps to the shadows; the figure moves like a person, not the dead

sc-00c (the trail):
- `f-pc1` [gt] — the robed figure climbs back up by a way toward the upper town
- `f-pc2` [delver] — Coombe follows the trail up, bending toward the upper town / shrine quarter
- `f-pc3` [delver] — sure it is someone of the town, but loses the trail before a face or name

sc-00d (cut off):
- `f-005` [delver, `payoff_expectation:"expected"`] — he records all he tracked in
  his journal (the tithes, the robed figure at the sluice, the trail toward the
  upper town), but NEVER names who the figure is — THE SETUP MARKER
- `f-pd1` [delver] — means to climb up and warn the town
- `f-pd2` [gt] — cut off in the flooded under-ways, he drowns, journal still on him
- `f-pd3` [delver] — his last lines beg whoever finds the journal to finish the tracking

## How the journal now pays off the shown tracking

The base's journal fact `f-130` (sc-14, frame `gt`) is the only existing fact
changed. Its payoff was re-pointed from `pays_off:["f-044"]` to
`pays_off:["f-044","f-005"]`. No other field of `f-130` changed.

So the sc-14 journal read-out now discharges BOTH:
- `f-044` (Tam's hearsay at sc-05 that his master went down tracking the tithes), and
- `f-005` (the delver's OWN recorded tracking, which the reader watched across
  sc-00a..sc-00d).

`f-130` reports `[paid] f-005 <- f-130` on EVERY world (claim / main / parley /
shatter), exactly as it does for `f-044`. The journal is now the payoff of scenes
the reader actually witnessed — the prologue's open thread ("name the hand")
closes at sc-14 where the journal completes the identification Coombe could not.
`f-044` is kept as-is.

---

## Self leak-check — the figure is NEVER named before sc-14

Every prologue fact (sc-00a..sc-00d) that mentions the figure, with the term
used. None names Vane / the shrine-keeper; none carries a typed leg with subject
= `shrinekeeper`:

- `f-pb1` [gt]  — "a robed figure" (tips bodies into the sluice)              — ok
- `f-pb2` [delver] — "a robed figure" / "the hand behind the new tithes"      — ok
- `f-pb3` [delver] — "the robed figure" (moves like a person, not the dead)   — ok
- `f-pc1` [gt]  — "the robed figure" (climbs toward the upper town)           — ok
- `f-pc2` [delver] — "the robed figure's trail" (bends toward the shrine quarter) — ok
- `f-pc3` [delver] — "someone of the town" ("before he can put a face or a name to it") — ok
- `f-005` [delver] — "the robed figure at the sluice" ("he never names who the figure is") — ok

Programmatic scan of all 13 prologue facts for `vane` / `shrine-keeper` /
`shrinekeeper` / `shrine keeper` (case-insensitive) in claim text AND for any
typed leg with subject `shrinekeeper`: **0 hits.** ANY LEAK = False.

The trail bends TOWARD the shrine quarter (allowed by the brief) but Coombe is
caught/cut off and loses the trail (f-pc3) BEFORE confirming who it is, then
drowns (f-pd2). The final identification is left to the journal + party at sc-14.

`f-004` (the withheld secret naming the shrine-keeper) is UNCHANGED — claim text,
`payoff_expectation:"expected"`, and its typed leg all byte-identical to v2 — and
its disclosure override is still `withhold`, first-at sc-14 on all three roads.

---

## Surgical-scope confirmation (diff vs v2)

- Facts: 13 new (`f-005`, `f-pa1..f-pa3`, `f-pb1..f-pb3`, `f-pc1..f-pc3`,
  `f-pd1..f-pd3`); 0 removed; exactly ONE existing fact changed — `f-130`, and
  ONLY its `pays_off` array (rest byte-identical). All 122 other v2 facts
  byte-identical.
- Frames: 1 new (`delver`); the other 11 byte-identical.
- Sections: 4 new prologue scenes; 0 removed; the 52 base scenes unchanged.
- `order.json`: 4 new prologue spine edges; the existing spine + all 3 branch
  chains byte-identical (fork untouched).
- `narrative-rules.json`: byte-identical to v2 (the one-holder-per-relic
  possession rule untouched).
- Fork, quests, map surfaces, possession rule, and every road's ending: untouched.

## Disclosure plan `delve` — reproduced byte-identical, no withhold changed

Built with the same `add-disclosure-plan` / `set-disclosure` commands as v2:
default-mode `state`; 3 withholds (f-004 @sc-14 all roads, f-171 @sc-18 all
roads, f-101 per-road claim=sc-26c/parley=sc-27p/shatter=sc-26s); 4 surfaces
(f-012 @sc-02 reeve-hall, f-041 @sc-05 lantern-house, f-060 @sc-07 shrine,
f-151 @sc-16 vault-door). Verified byte-identical to the v2 `delve` plan;
`f-004` still withheld at sc-14 on every road.

---

## Build recipe

Fresh empty seed -> `import-sections sections.json` (56 created) ->
`import-facts facts.json` (12 frames + 3 branches + 25 entities + 9 predicates +
136 facts created, 0 membership rejections) -> disclosure plan re-created via the
CLI commands above.

## Gate runs (final, all clean)

- **validate-continuity** (`--order order.json` + absolute `--rules`
  narrative-rules.json):
  `facts=136 order_nodes=56 conflict_pairs=0 cross_scope(data)=0 unordered=0
  rules=1 ... violations: 0 (structural=0 interval=0)`. Clean — incl. evidence
  reachability across the new prologue and the exclusive-possession rule.
- **report-fork-tree**: `3 registered world-line(s), 0 unplaced fork point(s)`;
  claim/parley/shatter all fork from `main` at sc-22. Clean (unchanged).
- **report-timeline-gaps** (per road): shatter / claim / parley each
  `violated=0 unverifiable=0` (0 interval rules). Clean.
- **report-payoff-coverage**: the new setup `f-005` is `[paid] <- f-130` on
  EVERY world (claim / main / parley / shatter), like `f-044`. The intended-open
  (dangling) quest sets per world are UNCHANGED from v2 — claim: f-041,f-171;
  main: the 5 cross-fork (f-012,f-041,f-060,f-101,f-171); parley: f-060;
  shatter: f-060,f-171. No new dangling.
- **report-payoff-substantiation**: `unsubstantiated=0` on every world. `f-005`
  reports `unverifiable` (untyped quest-giving setup — the allowed shape,
  matching f-044/f-012/f-060/f-151). Clean.
- **report-disclosure-coverage --telling delve**:
  `136 facts: disclosed=133 hidden_by_design=3 never_planned=0` (120->133: the
  13 new prologue facts fall under default-mode `state`; withholds unchanged at
  3; nothing left unplanned). Clean.
