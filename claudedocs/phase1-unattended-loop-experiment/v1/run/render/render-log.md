# Render log — world `road-descend`

Honest notes on ambiguities and on scenes where a disclosure label made the
rendering hard. Craft choices flagged where I had to make one.

## Scope: which scenes got rendered

The outline lists 16 scene slots but only 12 carry facts. The four `-h`
scenes — `sc-09h-holding`, `sc-10h-dawn-net`, `sc-11h-first-light`,
`sc-12h-what-the-log-says` — are all `begins=0 ends=0 holding=16`: they hold
the state established through sc-08 and introduce nothing. Read against the
"undeclared adjacency" pairs (each `-d` scene paired with its `-h`
counterpart, "the order does not compare them"), these are the untaken
branch — the world where Wren *holds the tower* instead of descending. This
world-line descends, so those four slots have no events in it.

I did **not** render headings/prose for the four `-h` scenes. Rendering them
would have required inventing the counterfactual "held the tower" plot, which
the outline does not contain for this world-line and the brief forbids
("do not add plot the outline does not contain"). The render file therefore
contains the 12 fact-bearing scenes in outline order: sc-01…sc-08, then the
descent track sc-09d, sc-10d, sc-11d, sc-12d. If the consumer expects all 16
ids present as empty stubs, that's a one-line change — but empty stubs would
be scaffolding, not prose, so I left them out and logged it here instead.

## Interpretation of `[withhold]`

Almost every surface fact (frames: relief / dispatch / senior / ranger /
hiker) is `[withhold]`, yet several *are the event their scene is named for*
— the empty cab (sc-02), the torn page (sc-03), the choice at the overlook
(sc-08). These cannot be omitted without erasing the scene. I read `withhold`
on these as: render obliquely, through scene and inference, and do not let a
narrator *announce* the fact as flat exposition. So the empty cab is shown
via the unlatched door, the cold stove, the unanswered call — the reader
infers "he's gone" rather than being told it as a headline.

I held a sharper line for the three **ground-truth** facts, which are the
real disclosure work (see below): those I concealed genuinely until their
`first_at` scene.

## Ground-truth facts held back until `first_at` — the hard cases

**gt-death (Tomas is dead) — seeded sc-02, tellable only from sc-11d.**
This is the clearest "basic event I was told to withhold." Tomas is already
dead at the foot of the scree *before scene one* — it is the story's premise —
but it cannot be stated until sc-11d. So sc-02 through sc-10d render him only
as **missing / gone**, an open question. In sc-05 I had to actively steer both
the narrator and Wren away from concluding his death even where dread pushes
there; I resolved it in-frame by having Wren catch her own fear and force the
word back to "missing" (frame-accurate: as the relief lookout she genuinely
does not know). The death is stated plainly for the first time in sc-11d, when
she finds the body. This is the scene where withholding a basic event most
forced me to *under-tell*.

**gt-smoke / gt-sella (the smoke was a real distress fire; Sella is alive and
stranded) — seeded sc-04, tellable only from sc-10d.** Through sc-04–sc-09d
the smoke is rendered only as Tomas's *disputed claim* and Wren's *belief* —
never confirmed as a real fire or a real person. sc-06 leans on this: Tomas's
conviction ("a person, not a wildfire; no one else would go") is his belief
and Wren's deduction of it, deliberately not confirmation that someone is
actually there. The reveal lands in sc-10d (the cold fire ring, then Sella
alive).

## Minor craft choice

In sc-10d, `f-10d2-sella-found` (hiker frame, the "signalled two days after a
fall" detail) is `[withhold]`, while `gt-sella` at the same scene is
`[state]`. Since the statable ground-truth already covers "alive / stranded /
injured / lit a signal fire," I told the reveal on that basis, and let the
extra specifics ("two days," "after the fall took her leg") arrive through
Sella's own later account ("the woman would tell her later") rather than as
omniscient narration — keeping the withheld detail sourced to the character
rather than announced.

## Nothing else notably ambiguous

No `hint`/`imply` partial modes appear in this outline (every fact is
`withhold` or `state`), so there were no fractional-disclosure judgement calls
beyond the above.
