# Author-repair log — "The Drowned Hold of Vael Mooren" (v2)

A SURGICAL repair of the v1 gate-clean fact base. Only the facts the three
author fixes (D1/D2/D3) name were added or amended; every other fact, and
`order.json` / `sections.json` / `narrative-rules.json`, is byte-identical to
v1. No new scene was added — both new facts sit at existing early scenes.

Base went from 121 → 123 facts (two new). All v1 gates re-run clean.

---

## D1 — earn the journal's naming of the shrine-keeper (sc-14)

**Added** `f-044` (frame `gt`, `canon_from` sc-05, the lantern-house scene,
`payoff_expectation:"expected"`, untyped):

> "Tam recounts that his master went down tracking where the new corpse-tithes
> were coming from — set on naming their source — and never climbed back out"

**Amended** `f-130` (the sc-14 journal read-out): added `pays_off:["f-044"]`.
No other field of f-130 changed.

**Why it closes the gap:** v1 asserted at sc-14 that the delver "tracked the
corpse-tithes to their source and found the shrine-keeper" with no earlier
setup — a bare conclusion. f-044 plants the investigation as an early-town
setup (this is also why the lantern-boy's errand exists: the master vanished
chasing the tithes). f-130 now pays it off, so the journal's naming of Vane is
an EARNED conclusion to a tracked investigation, not a verdict dropped in.

**Setup placement:** existing scene sc-05 (no new scene, no id shift). f-044 is
untyped, matching the existing quest-giving setups (f-012/f-041/f-060/f-151), so
it reports `unverifiable` in substantiation (the allowed shape), never
`unsubstantiated`. Because it sits on the shared spine and f-130 (sc-14, also
spine) pays it off, it is `[paid]` on every world — no new dangling.

## D2 — give Pip's incense clue a hand-off (sc-06 -> sc-13/sc-14)

**Added** `f-122` (frame `rogue`, `canon_from` sc-13, the delver-body scene):

> "Over the body, Pip finally tells the party the thing he overheard at Maeve's
> door — that the new tithe smells of incense, shrine-work — laying the clue
> beside the journal"

**Amended** `f-132` (sc-14, Lysa's certainty): added `sc-13` to its `evidence`
(now `["sc-14","sc-13","sc-06"]`). No other field changed.

**Why it closes the gap:** v1 `f-052` had Pip ALONE overhear Maeve's
"incense — shrine-work" mutter and never share it, yet the rendered sc-14
synthesis used "the incense Pip had heard." f-122 is the missing HAND-OFF:
the private clue becomes a shared one, contributing to the case against Vane.
f-132 now evidence-cites sc-13 (where the hand-off happens), so Lysa's certainty
rests on the journal + the shared tithe-lore + Pip's now-shared incense clue.
sc-13 is on the shared spine, at-or-before the citing fact's scene (sc-14) in
every world-line — evidence reachability holds (continuity violations=0).

**Placement:** existing scene sc-13 (no new scene, no id shift).

## D3 — reconcile the ending count (sc-18 vs sc-20)

**Amended** `f-171` (claim text only — typed leg `warden_disposition=reasonable`
UNCHANGED, as it is load-bearing for the PARLEY payoff f-502):

> "The wizard learns the warden's shade can be reasoned with — Orrek keeps the
> crown under duty, not malice, so a sworn oath is the third road the relic
> allows, not a way out beyond unmaking or claiming it"

**Amended** `f-172` (claim text only):

> "Lysa alone knows the crown can be shattered by the unmaking she has read, and
> tells the party the three roads the relic allows — unmake it, claim it, or
> bind it by a sworn oath — leaving it to him being no road at all"

**Why it closes the gap:** v1 had the wizard (sc-18) frame the oath/parley as a
FOURTH option beyond three ends, while the warden (sc-20, f-192) counts only
three — unmaking, claiming, or a sworn oath. The amended wizard facts now count
the SAME three real roads as the warden (unmake / claim / oath), with the oath
being the THIRD road and "leave it to him" the do-nothing non-choice, not a
fourth road. No road added, fork untouched, f-171's typed leg untouched.

---

## Surgical-scope confirmation

- Facts changed vs v1: exactly `f-044`, `f-122` (new) and `f-130`, `f-132`,
  `f-171`, `f-172` (amended). All 121 other v1 facts byte-identical.
- `order.json`, `sections.json`, `narrative-rules.json`: byte-identical to v1.
- No new scene; no scene renumbered. Fork, quests, map surfaces, possession
  rule, and all road endings untouched.
- Disclosure plan `delve` reproduced byte-identical to v1 (4 surfaces:
  f-012/f-041/f-060/f-151; 3 withholds: f-004@sc-14, f-171@sc-18, f-101
  per-road; default-mode `state`).

---

## Gate runs (final, all clean)

Build: fresh empty seed → `import-sections` (52 created) → `import-facts`
(11 frames + 3 branches + 25 entities + 9 predicates + 123 facts created,
0 membership rejections) → disclosure plan re-created via CLI.

- **validate-continuity** (`--order` + absolute `--rules`):
  `facts=123 conflict_pairs=0 unordered=0 rules=1`,
  `violations: 0 (structural=0 interval=0)`. Clean — incl. evidence
  reachability, off-branch, and the exclusive-possession rule.
- **report-fork-tree**: `3 registered world-line(s), 0 unplaced fork point(s)`;
  claim/parley/shatter all fork from `main` at sc-22. Clean.
- **report-timeline-gaps** (per road): shatter / claim / parley each
  `violated=0 unverifiable=0` (0 interval rules). Clean.
- **report-payoff-coverage**: 9 setups (was 8; new f-044).
  Per world every setup is paid or open BY DESIGN. New f-044 is `[paid] <- f-130`
  on EVERY world. Dangling set per world unchanged from v1 (claim: f-041, f-171;
  main: the 5 cross-fork; parley: f-060; shatter: f-060, f-171). No new dangling.
- **report-payoff-substantiation**: `unsubstantiated=0` on every world.
  f-044 reports `unverifiable` (untyped setup — the allowed quest-giving shape),
  matching f-012/f-041/f-060/f-151. Clean.
- **report-disclosure-coverage --telling delve**:
  `disclosed=120 hidden_by_design=3 never_planned=0` (118→120: the two new
  non-secret facts fall under default-mode `state`; withholds unchanged at 3;
  no fact left unplanned). Clean.
