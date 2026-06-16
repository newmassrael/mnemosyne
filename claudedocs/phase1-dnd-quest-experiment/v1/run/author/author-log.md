# Author log — "The Drowned Hold of Vael Mooren"

A gate-checked fact base for a branching dungeon-delve. Authored top-down:
skeleton first (scope + scenes, cast of frames, ground truth + three endings,
the fork, the quests, the load-bearing possessions, the disclosure plan), then
the connecting detail, with every road kept peopled to its last scene. The
tool's consistency gates were the self-check at every step.

Final shape: **52 scenes · 11 frames (10 person-frames + `gt`) · 3 branches ·
25 entities (incl. 4 quests + 6 map-locations) · 9 predicates · 121 facts.**
All gates clean.

---

## 1. The skeleton I laid

### Scope and scenes
- **Shared spine** `sc-01 … sc-22` (22 scenes): the Reach, the hiring, the four
  quest-givings, the descent, the people met in the dark, the journal, the
  runes, the locked door, the key, the crown's nature, the vault, the shade,
  and the choice.
- **One primary fork at `sc-22`** (~42% in), into three distinct terminal roads,
  each a 10-scene aftermath that does NOT reconverge:
  - SHATTER `sc-23s … sc-32s`
  - CLAIM   `sc-23c … sc-32c`
  - PARLEY  `sc-23p … sc-32p`

### The cast as frames (point of view)
Ground truth `gt` plus **ten person-frames**, each knowing only their own road:
- Party: `fighter` (Doran Brace, reads a fight), `wizard` (Lysa Quill, reads the
  runes and the crown's nature), `rogue` (Pip Sallow, finds the ways and the
  key, overhears), `cleric` (Mother Henna, senses the dead, parleys the shade).
- Town & hold: `reeve` (Calder, hires), `warden` (Orrek's shade, keeps the
  vault), `shrinekeeper` (Brother Vane, hidden cause), `lanternboy` (Tam),
  `rival` (Skell, the rival sellsword), `wisewoman` (Old Maeve, tithe-lore).

Knowledge is fragmented by frame: the wizard alone knows the crown can be
unmade and the warden can be reasoned with; the rogue alone overhears Maeve's
"shrine-work" hint; the cleric alone reads the single will leashing the dead;
the reeve never learns the warden's true nature. Verified with
`report-frame-view` (e.g. `reeve / warden` at `sc-31p` → `holding=0`; the
warden's reasonable nature stays the wizard's secret).

### The ground truth + the three endings
- GT: the rising is driven by the Hollow Crown and Orrek's drowned shade; the
  living cause is Brother Vane, feeding corpse-tithes to wake the warden and
  raise the hold's wealth to rule the Reach. The crown and the warden's key are
  single-holder relics.
- SHATTER: the wizard unmakes the crown; warden freed into death; rising ends;
  wealth lost; the cheated rival turns and is put down; the shrine-keeper is
  unmasked and answerable; the Reach ends poorer and quiet
  (`reach_rule = free-and-poor`).
- CLAIM: the party seizes the crown; the dead are *commanded*, not ended; the
  reliquary trap can now be fulfilled — they hand the crown to Vane and are
  betrayed; the Reach ends crown-ruled (`reach_rule = crown-ruled`).
- PARLEY: the cleric swears an oath with Orrek; the dead are *bound* not ended;
  the warden keeps the crown; the Reach takes up an eternal tithe-watch
  (`reach_rule = watched`).

### The quests (the heart) — authored to the contract
Each quest = a `quest` entity + a `pursues` fact + a giving fact
(`payoff_expectation:"expected"` + a map surface) + per-road completion
(`pays_off` + typed `completed_by`) or left open by design.

| Quest | Entity | Giver / surface | Pursued by | Prereq | Completed | Open by design |
|---|---|---|---|---|---|---|
| MAIN — end the rising | `q-main` | reeve, `sc-02/reeve-hall` (`f-012`) | fighter | **requires `q-key`** (`f-153`) | all 3 roads (`f-305`/`f-404`/`f-505`) | — |
| SIDE — the warden's key | `q-key` | vault door, `sc-16/vault-door` (`f-151`) | rogue | — | spine `sc-17` (`f-161`), all roads | — |
| SIDE — the last delver | `q-delver` | lantern-boy, `sc-05/lantern-house` (`f-041`) | rogue | — | SHATTER `f-316`, PARLEY `f-515` | **CLAIM** (party turns ruler, never tells Tam) |
| SIDE/TRAP — the reliquary | `q-reliquary` | shrine, `sc-07/shrine` (`f-060`) | cleric | — | **CLAIM only** `f-409` (then betrayed `f-411`) | **SHATTER & PARLEY** (no crown to give) |

The **key → vault prerequisite is real and ordered**: `q-key` is recovered at
`sc-17`, strictly before the vault opens at `sc-19` (`f-180` `pays_off` the
giving `f-151`); `sc-16→sc-17→sc-18→sc-19` on the spine, inherited by every road.
`q-main requires q-key` is the typed `f-153`.

**Road divergence is genuine:** `q-reliquary` finishes ONLY on CLAIM (it needs
the crown in hand) and stays open on SHATTER/PARLEY; `q-delver` finishes on
SHATTER/PARLEY and stays open on CLAIM. So the three endings are different games,
not the same game with new scenery.

### Load-bearing possessions (typed, exclusive)
- `crown`: warden holds it (`sc-20`, `f-191`) → on CLAIM the fighter takes it
  (`f-401` supersedes `f-191`) → then Vane takes it at the shrine (`f-410`
  supersedes `f-401`). On PARLEY the warden keeps it (`f-506`). On SHATTER it is
  destroyed.
- `key`: the rogue recovers and holds it (`f-162`).
- `narrative-rules.json` declares one `exclusive` rule, `predicate=possession`,
  `per=object` — the gate enforces one holder per relic at a time. The crown's
  `supersedes_in_frame` chain reads cleanly through CLAIM with no co-holding.

### The disclosure plan (sparse — surfaces on givers, withholds on secrets)
One telling `delve` (`default-mode state`):
- **4 surfaces**, one per quest-giver: `f-012`→reeve-hall, `f-041`→lantern-house,
  `f-060`→shrine, `f-151`→vault-door.
- **3 withheld secrets** (each typed, so the gate matches):
  - `f-004` shrine-keeper is the cause → `first_at sc-14` (the journal) on all roads.
  - `f-171` the warden can be reasoned with → `first_at sc-18` (the runes) on all roads.
  - `f-101` the rival's coming betrayal → `first_at` SHATTER `sc-26s` /
    CLAIM `sc-26c` / PARLEY `sc-27p` (where the turn resolves on each road).

---

## 2. Write → gate → repair iterations

**Iteration 1 — sections + facts import.**
`import-sections` accepted all 52 scenes. `import-facts` REJECTED on the first
typed fact: a typed claim's subject (and any entity-shaped object) must be a
member of the fact's `entities` list ("the entities list stays THE retrieval
key"). An audit found **14 such membership gaps** across typed facts (quest
`completed_by` legs naming an entity not yet listed, `rising_state`/`reach_rule`
facts using `deep-vault` as subject without listing it, etc.). Added the missing
typed-leg entities to each fact's `entities` list; re-imported from the empty
seed — clean: 11 frames + 3 branches + 25 entities + 9 predicates + 121 facts.

**Iteration 2 — disclosure plan.**
Added the plan, the 4 surfaces and the 3 withholds via the CLI mutate API.
`report-disclosure-coverage` → `disclosed=118 hidden_by_design=3 never_planned=0`.

**Iteration 3 — substantiation tightening (craft, read-only gate).**
`report-payoff-substantiation` flagged the most load-bearing payoff,
`f-004` (the buried cause), as **UNSUBSTANTIATED**: the reveal `f-131` restated
the same typed value as the setup, so it read as "no state-change." Re-modelled
`cause_of_rising` as a **scalar disclosure-state** that genuinely flips:
`f-004`/`f-031` = `hidden`, the reveal `f-131` and Vane's open seizure `f-412`
= `exposed`. After rebuild, `f-004` is substantiated on every world.
A second hollow payoff surfaced: `f-171` (warden reasonable) on PARLEY, where
`f-502` restated `reasonable`. Changed `f-502`'s disposition to `sworn` — a real
transition from the latent capacity (set up at `sc-18`) to the realized oath.
After rebuild: **`unsubstantiated=0` on every world.** (The remaining
`unverifiable` entries are the untyped quest-giving setups — that is the brief's
own quest-giving shape, advisory not a gate.)

**Iteration 4 — full sweep, all clean** (see below).

No iteration hit an off-branch / evidence-unreachable error: every `evidence[]`
back-reference cites the establishing scene at-or-before the citing fact in its
own world-line, and any event two roads needed (the journal, the runes, the key,
the choice) lives on the **shared spine** before `sc-22`. The exclusive-possession
rule never flagged a co-holding, because each transfer uses
`supersedes_in_frame` rather than a second live holder.

---

## 3. What each gate reported (final)

- **validate-continuity** (`--order` + `--rules`): `violations: 0
  (structural=0 interval=0)`, `conflict_pairs=0`, `rules=1`. Clean — including
  evidence-reachability, off-branch, and the exclusive-possession rule.
- **report-fork-tree**: `3 registered world-line(s), 0 unplaced fork point(s)`;
  all three fork from `main` at `sc-22`; every road reaches its terminal.
- **report-timeline-gaps** (per road): `violated=0 unverifiable=0` on
  shatter/claim/parley (no interval rules declared, so nothing to violate).
- **report-payoff-coverage**: 8 setups. Per road every setup is paid or open
  BY DESIGN — `world main` dangles the five cross-fork setups (they pay off only
  after the fork); SHATTER leaves `f-060` (reliquary, no crown) + `f-171`
  (warden never parleyed) open; CLAIM leaves `f-041` (delver never told) +
  `f-171` open; PARLEY leaves `f-060` (reliquary refused) open. Every open
  thread is intended.
- **report-payoff-substantiation**: `unsubstantiated=0` on every world after the
  Iteration-3 retyping.
- **report-disclosure-coverage**: `disclosed=118 hidden_by_design=3
  never_planned=0`.
- **report-playthrough-manuscript** (per road): each world `32 scene(s),
  undeclared adjacencies=0, unplaced=0, undecidable=0, outside order=20` (the 20
  scenes of the other two roads — normal).
- **report-playable-world**: 4 worlds, **4 quest-giver locators each**
  (reeve-hall #1, lantern-house #4, shrine #6, vault-door #15), `unplaced=0,
  undecidable=0`. The map the later stage reads resolves cleanly on every road.

---

## 4. How I kept each road peopled

A `report-frame-view` and a tail audit confirm the recurring failure (aftermath
shrinking to two principals) was avoided. Every road's post-fork tail
(`sc-26X … sc-32X`) carries facts in the frames of the **lantern-boy, reeve,
rival, shrine-keeper, and wise-woman** — plus the warden/cleric/fighter where
they act on that road:

- SHATTER tail: fighter, cleric, lanternboy, reeve, rival, shrinekeeper,
  wisewoman (Skell turns and falls; Henna lays the dead to rest; Maeve reads the
  falling water; Tam is told and keeps the journal; Vane is unmasked before
  Calder).
- CLAIM tail: lanternboy, reeve, rival, shrinekeeper, wisewoman (Skell kneels to
  the new power; Calder's authority goes hollow; Maeve names the dark bargain;
  Tam is left under the crown's shadow, never told of his master).
- PARLEY tail: lanternboy, reeve, rival, shrinekeeper, wisewoman, warden (Skell
  is refused and climbs out sour; Maeve takes up the tithe-watch; Calder binds
  the Reach to it; Tam joins the watch; Vane is exposed by the oath's terms).

Each secondary person carries their fragment of the truth into how the road
closes — present, knowing, and acting to the last scene.
