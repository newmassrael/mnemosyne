# Author-repair brief — close the named gaps in an existing quest-game, gate-clean

You are a game-adventure author doing a TARGETED REPAIR on an existing,
gate-clean dungeon-delve fact base. You will be given a REPAIR SPEC listing a
small number of specific deficiencies and the fix each calls for. Make ONLY
those fixes, keep everything else untouched, and re-run the consistency gates so
the repaired base stays clean.

Read EXACTLY these and NO other file under `claudedocs/phase1-dnd-quest-experiment/`:
- `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v2/repair-spec.md` (the deficiencies D1-D4 and the fix each calls for; you handle the AUTHOR fixes D1, D2, D3 — D4 is render-only, not yours)
- the v1 base you are repairing (copy it as your starting point):
  `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v1/run/author/sections.json`,
  `.../facts.json`, `.../order.json`, `.../narrative-rules.json`

`mnemosyne-cli` is installed and on your PATH (schema_version 23). Work in
`/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v2/run/author/`.

## What to do

1. Copy v1's `sections.json`, `facts.json`, `order.json`, `narrative-rules.json`
   into your working dir as your starting base.
2. Make ONLY the three author fixes the spec names:
   - **D1 — earn the journal's naming of the shrine-keeper.** Add an early SETUP
     fact (ground-truth or the lantern-boy's account) that the last delver went
     down TRACKING the corpse-tithes (this is why he vanished). Then make the
     sc-14 journal fact (`f-130`) PAY IT OFF (`pays_off`: the new setup's id), so
     the journal's "tracked the tithes to their source and found the
     shrine-keeper" is an earned conclusion, not a bare assertion. Add the
     setup's scene to `order.json` if it needs a new scene; prefer placing the
     setup at an EXISTING early town scene to avoid shifting ids.
   - **D2 — give Pip's incense clue a hand-off.** v1 `f-052` (sc-06, frame
     rogue): Pip alone overhears the wise-woman mutter the new tithe "smells of
     incense — shrine-work", and never shares it. Add a HAND-OFF fact: Pip tells
     the party what he overheard (frame rogue or gt), placed at a scene at or
     before sc-14 (a natural spot is when the journal is read, sc-13/sc-14).
     Then have the sc-14 deduction fact (`f-132`, Lysa's certainty) EVIDENCE-CITE
     both the journal and Pip's now-shared clue (add the hand-off scene to
     `f-132`'s `evidence`). Place the hand-off at an EXISTING scene if you can,
     to keep ids stable.
   - **D3 — reconcile the ending count.** v1 has the wizard (sc-18, `f-171` +
     `f-172`) frame the oath/parley as a FOURTH option beyond three ends, while
     the warden (sc-20, `f-192`) counts only three (unmake / claim / oath).
     Amend the wizard's belief facts so the wizard counts the SAME three real
     roads — unmake / claim / oath(parley) — with "leave it to him" being the
     do-nothing non-choice, not a fourth road. Edit the claim text of `f-171` /
     `f-172` to match; do not add a road or touch the fork.
3. Re-import from a fresh empty seed and re-run EVERY gate until clean (the same
   gates v1 used):
   ```
   mnemosyne-cli validate-continuity            --order order.json --rules <abs path to narrative-rules.json> --sidecar store.atomic.json
   mnemosyne-cli report-fork-tree               --order order.json --sidecar store.atomic.json
   mnemosyne-cli report-timeline-gaps --world <each road> --order order.json --sidecar store.atomic.json
   mnemosyne-cli report-payoff-coverage         --order order.json --sidecar store.atomic.json
   mnemosyne-cli report-payoff-substantiation   --order order.json --sidecar store.atomic.json
   mnemosyne-cli report-disclosure-coverage --telling delve --sidecar store.atomic.json
   ```
   (The disclosure plan does NOT live in the JSON — re-create it with the same
   `add-disclosure-plan --telling delve --default-mode state` + the v1
   `set-disclosure` surfaces/withholds. Read v1's `store.atomic.json`
   `disclosure_plans.delve` to see exactly which facts were surfaced/withheld and
   reproduce them, plus a surface for any NEW giving fact you add — though you
   are not adding quests, so the four quest surfaces are unchanged.)
   Pass `--rules` an ABSOLUTE path (it resolves from the repo root, not your cwd).

## Hard constraints (this is a SURGICAL repair)

- Change ONLY the facts the three fixes name (add the D1 setup + D2 hand-off
  facts and their legs; amend the D3 wizard belief facts). Do NOT rewrite,
  reword, or re-thread any other fact — a later step DIFFS your facts.json
  against v1 and the repair must be localized.
- Do NOT touch the fork, the quests, the map surfaces, the possession rule, or
  any road's ending. Do NOT renumber existing scenes; if you must add a scene,
  append a new id, do not shift existing ones.
- Every gate must end clean, exactly as v1's did (continuity 0/0, fork placed,
  no unplaced/undecidable, payoff danglings still only the intended open quests).

## Deliverables (in v2/run/author/)

- `sections.json`, `facts.json`, `order.json`, `narrative-rules.json`,
  `store.atomic.json` (final, gate-clean, with the `delve` disclosure plan).
- `author-log.md` — the exact facts you added/amended for D1/D2/D3 (ids +
  claims), why each closes its gap, and the gate runs confirming clean. Note any
  new scene id you had to add.

When done, reply with a SHORT summary only: the fact ids you added/amended per
D1/D2/D3, whether you added any new scene, and confirmation every gate is clean.
Do not paste the full base.