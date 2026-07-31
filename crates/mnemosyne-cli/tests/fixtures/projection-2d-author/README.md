# projection-2d-author fixture — the author arm, at the current schema

`facts.json` here is the ONE input `projection_2d_author_arm_smoke.rs` needs that
the frozen experiment record cannot supply. Everything else the rebuild reads —
`sections.json`, `order.json`, `narrative-rules.json` — comes straight out of
`claudedocs/phase1-2d-projection-experiment/v1/run/author/`, unchanged and
uncopied, because a second editable home for a datum is the drift this whole
line of work exists to remove.

## Why a copy exists at all, and why R879 was wrong to avoid making one

R879 needed this arm importable on today's binary — R752 widened a disclosure
override's `first_at` from a single coordinate to a coordinate SET, and the
record carries the superseded `[branch, coord]` pair. It made that change **in
place, in the record**, on the ground that the transcription provably preserved
content: the `(telling, fact, branch, coords)` triples were compared before and
after and were identical.

The content argument was right and the conclusion was wrong. R882 wrote down the
revision each manifest was authored under, and R883 replayed every manifest at
its own revision — at which point the edited record **failed at `14d7a1e0`,
the very revision that produced it**, while the original bytes import there
cleanly (20 sections, 59 facts, 7 disclosure overrides). Editing the record had
bought readability by today's tool at the price of readability by its own.

So the rule is narrower than R879 believed, and it has no exception for
provable transcriptions:

> **The record is never edited. A shape the current tool needs lives in a
> derived copy, here, labelled as derived.**

This file is that copy. Its green is its own claim — that the arm's authored
disclosure timing survives into today's shape — and it does not stand in for the
experiment's pre-committed pin, which is a claim about what the blind author
produced and is re-checked by running `14d7a1e0` against the untouched record.
