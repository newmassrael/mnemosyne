# Vocabulary — npc-dialogue-experiment/v1 (ids only; NO claims, NO facts, NO plan)

Use these exact ids when you record facts. Do not invent new ids for things named here.

## Frames (points of view)
- `gt` — the ground truth — what actually happened on the wreck-night and after
- `halsa` — Halsa Crewe, the keeper's child, left to keep the Holm — the point of view
- `bryde` — Bryde Crewe, the old keeper, abed with a broken leg
- `officer` — the revenue riding-officer, Crown, waiting for the cutter
- `mate` — Cass Pellow, the wreck's surviving mate
- `passenger` — Ysolt Marran, the passenger with the locked sea-chest
- `headman` — Orne Veck, the Holm's headman and net-master
- `saltwife` — Dunna Quick, the fisher-widow first down at the ebb
- `pilot` — Maon Skerry, the old pilot who once guided ships off the Sneck
- `boy` — Wick, the boy who found the first thing on the shore
- `factor` — Pell Garrow, the mainland factor's man, stranded by the tide
- `curer` — Eda Lay, the fish-curer
- `girl` — Senna, betrothed to the boy Wick

## Branches (world-lines) and where they fork
- `report` — forks from `main` at `sc-15`
- `shelter` — forks from `main` at `sc-15`
- `confront` — forks from `main` at `sc-15`
- shared spine before the fork = root branch `main` (omit `branch` for spine facts)

## Entities (people / objects / places)
- `halsa` (person) — the keeper's child, tending the light and tide-bell
- `bryde` (person) — the old keeper, Halsa's parent, abed with a broken leg
- `officer` (person) — the revenue riding-officer
- `mate` (person) — the wreck's surviving mate, Cass Pellow
- `passenger` (person) — Ysolt Marran, the passenger off the wreck
- `headman` (person) — Orne Veck, the Holm's headman
- `saltwife` (person) — Dunna Quick, the fisher-widow
- `pilot` (person) — Maon Skerry, the old pilot
- `boy` (person) — Wick, the boy of the Holm
- `factor` (person) — Pell Garrow, the factor's man
- `curer` (person) — Eda Lay, the fish-curer
- `girl` (person) — Senna, betrothed to Wick
- `supercargo` (person) — the wreck's supercargo, the one dead man
- `lantern` (item) — the lantern that showed the false channel light from the Ness
- `chest` (item) — the passenger's locked sea-chest
- `shortcask` (item) — the one cask of cargo that went quietly up the Holm
- `holmlight` (item) — the Holm's true light, doused on the wreck-night
- `deed` (item) — the inheritance papers carried in the chest
- `tally` (item) — the keeper's tally-book logging the lights and tides
- `ladingbill` (item) — the bill of lading the supercargo carried
- `sneck` (place) — the long shoal where ships go aground
- `drang` (place) — the tidal causeway to the mainland
- `ness` (place) — the Ness headland, where the false light was shown

## Predicates (relation types for typed facts)
- `possession` (object: entity) — who holds the object at this scene (one holder at a time)
- `cause` (object: scalar) — the load-bearing wreck-night truth a subject is the cause of
- `whereabouts` (object: scalar) — where a load-bearing object stands, as a state that changes when it is found or moved
- `lit` (object: scalar) — the state of a light — burning, out, or shown to have been doused by a hand
