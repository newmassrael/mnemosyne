# Scope investigation — closing the D1 residue by REAL story expansion (R565)

The owner's question: the R564 repair left ONE residual (D1 — the dead delver's
identification of the villain is summarized off-page backstory). A surgical
repair could not fully close it; "writing the story bigger" (a flashback that
shows the investigation on-page) can. **How much effort is that real expansion?**

This is a code-0 effort/scope investigation grounded in the actual v2 base
(`claudedocs/phase1-dnd-quest-experiment/v2/run/author/`: 52 scenes, 121→123
facts, 11 frames, the `delve` disclosure plan). No build, no run. Estimates are
in concrete SUBSTRATE UNITS (frames / scenes / facts / gate re-runs / render +
judge scope), not invented hours — per the no-fake-metrics rule.

## The three tiers (recap), and which the owner asked to scope

- **(1) light** — make the journal's content a CONCRETE observation the party
  reads. ~2-4 facts; no new frame/scenes; no disclosure rework (the journal is
  read at sc-14 = already the reveal). Cost ≈ a surgical repair (what v2 was).
- **(2) medium** — add a LIVING corroborator. ~3-6 facts, 0-1 scene, 0-1 frame.
- **(3) full flashback** — author the delver's investigation as REAL on-page
  scenes. This is the "real expansion" the owner wants scoped.

## What tier (3) concretely requires, against THIS base

### A. The mechanical additions (the substrate + the proven loop already do these)

- **+1 frame: `delver`.** The base has a delver ENTITY (kind=person) but NO
  delver frame — he has no point of view. To show his investigation as lived
  scenes he needs one. (One `add-frame`. Trivial.)
- **+3-5 prologue scenes.** The investigation arc: he notices the fresh
  corpse-tithes → follows the cart of dead → stakes out the sluice → watches a
  robed figure tip bodies into the floodwater → follows the trail toward the
  shrine and writes it down → is caught / drowns. These are new sections placed
  EARLY: a prologue chain `sc-00a → sc-00b → … → sc-01` prepended to the spine.
  The substrate places scenes at any canon coordinate (this is exactly what it
  is for), so this is new sections + a few new `order.json` edges. (Mechanical.)
- **+~10-18 facts.** Per prologue scene: the delver's observations (frame
  `delver`) + the ground-truth of what happened + the trail/evidence. Then the
  EXISTING journal fact `f-130` is re-pointed to PAY OFF these shown scenes (the
  journal becomes the record of what the reader watched), and `f-044` (Tam's
  account that his master went tracking) becomes the town-side echo of a prologue
  the reader has already seen. (Authoring — the loop is proven to converge bases
  this size and far larger.)
- **Scale check: 52 → ~56-57 scenes, 123 → ~135-140 facts.** This is WELL within
  the proven envelope — R524 authored 70 scenes, R545 95 scenes, both gate-clean.
  Volume is NOT the cost driver here.

### B. The ONE genuinely hard part: the withhold/leak interaction

The mystery turns on `f-004` (the cause is the shrine-keeper, Brother Vane)
being WITHHELD until sc-14. A naive flashback that shows Vane's face tipping the
bodies at sc-00c would make the villain's identity re-extractable at sc-00c ≪
sc-14 → **the leak gate FAILS** (the whole point of the v1/v2 pin discipline).

The saving grace, found in the real base: the DEED is ALREADY public and the
IDENTITY is what's withheld. `f-051` (Old Maeve: "someone living must be feeding
the water") is NOT withheld — the town already knows SOMEONE does it. Only `f-004`
(it is VANE) is withheld. So the clean expansion is:

- the prologue shows the delver watching a **robed / unnamed figure** tip the
  bodies and following the trail — realizing the DEED (already public) and
  tracking it toward the shrine;
- the figure's **identity stays unnamed on-page** until sc-14, where the journal
  + the wizard's synthesis name Vane (the existing reveal, now the payoff of
  scenes the reader actually watched).

This keeps `f-004` withheld and ADDS dramatic irony about the deed without
spoiling the who. The cost of this part is NOT volume — it is DESIGN CARE:
- the prologue facts must be TYPED about "a robed figure," never `subject=Vane`,
  so the leak gate's typed-tuple match still sees the identity only at sc-14;
- a RENDER discipline: the flashback prose must not show the face / name (a
  single careless sentence re-leaks);
- a leak-gate RE-PROVE on the expanded base + a blind re-extraction confirming
  the identity is still first-re-extractable at sc-14, not in the prologue.

This is a thinking + iteration task (split deed-vs-identity cleanly, re-time the
withhold, re-prove the gate), not a scale task. It is the actual "공수".

### C. The downstream scope that grows beyond a surgical repair

- **Render:** v2 re-rendered 6 scenes as a patch. Tier (3) adds 3-5 BRAND-NEW
  prologue scenes of prose (plus re-touching sc-13/sc-14 where the journal now
  pays off shown scenes). Bigger render, and it is NEW prose, not a splice.
- **Re-gate:** the full PIN-A/PIN-B/PIN-C suite again, with the leak re-prove as
  the load-bearing check (B above).
- **Re-judge:** a blind judge must assess not just "did the seam close" but
  "is the bigger game BETTER or just longer" — because (D) a prologue changes the
  STORY.

### D. The craft trade-off (not a cost, a creative consequence — flag it honestly)

Opening with the delver's investigation changes the kind of story: the reader now
knows from scene one that a figure is feeding the dead (dramatic irony) before the
party arrives. The mystery shifts from "what is happening?" to "will the party
catch up to what we already saw?". The hooded-figure design keeps the WHO a
mystery, so it adds irony about the DEED while preserving the identity reveal —
but it is still a DIFFERENT opening rhythm than the current cold-open. Whether
that is better is an authoring judgment, not a defect fix. The current v2 already
judged high (5/4/5); tier (3) trades a tighter mystery-open for a fuller,
more-grounded investigation channel. This is the real reason tier (3) is a
"creative choice," not a mechanical patch.

## Honest effort estimate (relative, substrate-grounded)

| tier | new frame | new scenes | new facts | disclosure rework | render | net character |
|---|---|---|---|---|---|---|
| (1) light | 0 | 0 | ~2-4 | none | patch | a surgical repair (≈ v2) |
| (2) medium | 0-1 | 0-1 | ~3-6 | light (witness stays partial) | small patch | a small repair round |
| (3) full | +1 (`delver`) | +3-5 prologue | +~10-18 | **the hard part: split deed/identity, re-time withhold, re-prove leak** | new prologue prose + sc-13/14 | a small NEW-AUTHORING round, not a repair |

**Bottom line.** Tier (3) is feasible and the substrate + the proven authoring
loop already handle the BULK of it (frames, scene placement, gating, render,
judge — all demonstrated, and the scale 52→~56 is trivial vs the proven 95). The
cost is NOT dominated by volume. It is dominated by ONE design task — re-modeling
the withheld secret so the flashback shows the deed without leaking the villain's
identity before sc-14 — plus the larger NEW-prose render and a re-judge that must
weigh a changed story-opening. In project terms it is roughly a **single small
new-authoring round (design + author + gate + render + judge)**, materially more
than the surgical v2 repair but far less than a from-scratch game, and it carries
a genuine CREATIVE trade-off (a flashback prologue is a different story rhythm),
so it is a scope/taste decision, not a forced fix.

The substrate finding worth keeping: the withhold layer (R506) is what makes this
non-trivial AND what makes it possible — because the deed is already public and
only the identity is withheld, the expansion has a clean seam to cut along; a base
that had withheld the DEED itself would have made tier (3) much harder.
