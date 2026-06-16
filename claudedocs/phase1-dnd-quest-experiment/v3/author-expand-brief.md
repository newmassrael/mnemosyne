# Author-expand brief — add the delver's investigation as an on-page prologue

You are a game-adventure author EXPANDING an existing, gate-clean dungeon-delve
fact base. The base already tells the party's descent; it refers to a delver who
went down before them, vanished, and left a journal that (read at scene sc-14)
names the villain. Right now that journal's account is summarized backstory. Your
job: author the delver's investigation as REAL on-page PROLOGUE scenes the reader
witnesses — WITHOUT spoiling the villain's identity, which must stay hidden until
sc-14.

Read EXACTLY these and NO other file under `claudedocs/phase1-dnd-quest-experiment/`:
- this brief
- the v2 base you expand (copy it as your start):
  `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v2/run/author/sections.json`,
  `.../facts.json`, `.../order.json`, `.../narrative-rules.json`
- to reproduce the disclosure plan, read `disclosure_plans.delve` in
  `.../v2/run/author/store.atomic.json` and re-create it with the same
  `add-disclosure-plan`/`set-disclosure` commands (do NOT change any withhold).

`mnemosyne-cli` is installed (schema 23). Work in
`/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v3/run/author/`.

## The story you are adding (the prologue)

Before the party arrives, the last delver went down TRACKING where the fresh
corpse-tithes came from (the base already plants this at sc-05). Author his hunt
as a short PROLOGUE the reader sees first-hand:
- he notices the new tithes / the dead being added to the floodwater;
- he watches, hidden, as a ROBED FIGURE tips bodies into the sluice by night;
- he follows the figure's trail through the under-ways, toward the upper town /
  the shrine quarter;
- he gets close — close enough to be sure the figure is someone of the town, not
  a monster — and writes down what he has tracked;
- he is discovered / cut off / drowns before he can bring word up.

**THE WITHHOLD RULE — this is the whole point, do not break it.** The villain's
identity (that the figure is Brother Vane, the shrine-keeper) is a WITHHELD
secret in the base (fact `f-004`, first revealed at sc-14). Your prologue shows
the DEED and the TRACKING but must NEVER confirm or name WHO the figure is:
- the figure is a "robed figure" / "a hooded shape" / "someone of the town" —
  never named, never face-shown, never tied to the shrine-keeper;
- the trail may lead TOWARD the shrine quarter, but the delver must NOT confirm
  the figure IS the shrine-keeper — he is caught / cut off just short of it (that
  final identification is what the journal + the party complete at sc-14);
- author NO fact, in ANY frame, that names the figure as Vane / the shrine-keeper
  before sc-14. The prologue's facts are about "a robed figure" — in the new
  `delver` frame (what he sees/believes) and `gt` (what truly happens: a robed
  figure feeds the dead, already public knowledge per the base's Maeve facts).
  Do NOT touch `f-004` or its withhold; do NOT add a typed claim with
  subject = the shrine-keeper before sc-14.

A later gate will RE-EXTRACT your rendered prologue blind and CHECK that the
villain's identity is still first knowable at sc-14, not in the prologue. If your
prologue lets a reader conclude it is the shrine-keeper, that gate FAILS — keep
the trail short of confirmation.

## What to add (mechanically)

1. Copy the v2 base into your working dir as your start.
2. **Add the `delver` frame**: `mnemosyne-cli add-frame --frame delver --description "Coombe, the last delver — his hunt for the tithe's source, before the party"` (or via the facts.json frames array).
3. **Add the prologue scenes** as sections with ids that sort BEFORE sc-01 — use
   `sc-00a`, `sc-00b`, `sc-00c`, `sc-00d` (3-5 scenes). Add them to
   `sections.json`.
4. **Prepend the prologue to the canon order**: in `order.json`, add the edge
   chain `sc-00a -> sc-00b -> ... -> sc-00d -> sc-01` (so the prologue runs before
   the existing spine; the existing `sc-01 -> sc-02 ...` edges stay).
5. **Author the prologue facts** (in `facts.json`): the delver's observations
   (frame `delver`) + the ground-truth deed (frame `gt`, the unnamed robed figure
   feeding the dead — consistent with the already-public "someone feeds the
   water"), at sc-00a..sc-00d. ~10-18 facts.
6. **Re-point the journal to pay off the shown prologue**: the base's journal
   fact `f-130` (sc-14) currently pays off `f-044`. Add a SETUP marker to one
   prologue fact (the delver's recorded tracking, `payoff_expectation:"expected"`)
   and have `f-130` ALSO pay it off — so the sc-14 journal is now the payoff of
   scenes the reader actually watched. Keep `f-044` as-is.
7. Do NOT change the fork, the quests, the map surfaces, the possession rule, any
   road's ending, or any existing fact other than (6)'s payoff re-point. Add the
   delver entity's prologue facts; the delver entity already exists.

## Re-gate (every gate clean, exactly as v2)

```
mnemosyne-cli validate-continuity            --order order.json --rules <abs path narrative-rules.json> --sidecar store.atomic.json
mnemosyne-cli report-fork-tree               --order order.json --sidecar store.atomic.json
mnemosyne-cli report-timeline-gaps --world <each road> --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-coverage         --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-substantiation   --order order.json --sidecar store.atomic.json
mnemosyne-cli report-disclosure-coverage --telling delve --sidecar store.atomic.json
```
All must end clean (continuity 0/0, fork placed, no unplaced/undecidable, the
prologue scenes reached in the order, the intended open quests unchanged, the
delve plan reproduced with f-004 still withheld at sc-14).

**Self leak-check (do this):** confirm NO prologue fact (sc-00a..sc-00d) names or
types the robed figure as the shrine-keeper / Vane. List, in your log, every
prologue fact that mentions the figure and confirm each says "robed figure" not
"the shrine-keeper".

## Deliverables (v3/run/author/)

- `sections.json`, `facts.json`, `order.json`, `narrative-rules.json`,
  `store.atomic.json` (gate-clean, `delve` plan reproduced).
- `author-log.md` — the prologue scenes + facts you added (ids), how the journal
  now pays off the shown tracking, the self leak-check confirming the figure is
  never named before sc-14, and the clean gate runs.

Reply with a SHORT summary: the prologue scene ids + frame added, how many facts,
how the journal re-points, and confirmation the figure is never named before
sc-14 and every gate is clean. Do not paste the full base.