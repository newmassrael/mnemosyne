# Premise — "The Night Lock at Harrow Sluice" (setting + required shape)

A fresh, IP-free small branching game. This file is the SETTING and the
required STRUCTURAL shape. You (the authoring agent) invent the specifics;
you must satisfy every REQUIRED item below.

## Setting

Harrow Sluice is a canal lock on a working waterway. Tonight a lone
apprentice keeper, **Wick**, holds the watch alone through a bad storm while
the master keeper is away. Late in the night a barge, the **Marrow**, signals
out of the dark: it wants passage before the flood crests. Working the lock at
night in a storm is dangerous and against the master's standing order — but
turning the barge away has its own cost. Wick must decide, and live with it.

## The three hard world-invariants (LOAD-BEARING — these define the game)

These are not flavour: the whole point is that the world OBEYS them, and the
substrate's continuity gate should ENFORCE them. Encode each as the world-logic
that it is, and make the continuity gate hold you to it (use `describe-schema`
to learn how the substrate expresses a hard world-rule — that is part of your
authoring task, not given to you here).

1. **The storm-lantern (custody).** There is exactly ONE storm-lantern. Whoever
   works the gate machinery must be the one holding it; it passes hand to hand.
   At any moment, in any single world-line, the lantern has exactly ONE holder —
   never two at once. (A custody / conservation invariant.)

2. **The gate sequence (state machine).** The lock-gate moves through states
   `barred -> cracked -> open` and back `open -> cracked -> barred`. It must
   NEVER jump `barred -> open` (that is how a gate — or a keeper — gets
   destroyed). Only the adjacent steps are legal. (A state-machine invariant on
   the gate's state as it changes over the night.)

3. **The fill hold (timing).** Before the gate may be brought to `open`, the
   upper pound must have been filling for at least a set number of minutes (pick
   one, e.g. 30) — open it too early and the surge swamps the Marrow. The elapsed
   fill time must meet or exceed that minimum. (A numeric-timing invariant.)

## Required structural shape

- **14-20 scenes.** Small and taut; depth over volume.
- **4-6 frames (cast/POV).** At least Wick; the master keeper (absent, via the
  standing order / log); the barge-master of the Marrow; and ground-truth.
- **One two-branch fork:** at the barge's signal, either **WORK THE LOCK** for
  the Marrow (run the gate sequence, honour the fill hold) or **HOLD** it till
  dawn (refuse passage). Each road must reach a real, different aftermath.
- **A withheld secret (a disclosure plan / telling):** something the reader does
  not learn until a chosen point — e.g. WHY the master left, or what the Marrow
  is really carrying. Author it as a withhold with its reveal timing.
- **One quest:** e.g. "get the Marrow through safely" (WORK road) / "log the
  night honestly by the standing order" — with its giving and completion.
- **Setup -> payoff chains:** plant, then pay off (a Chekhov gun or two).

## What makes this game itself

The drama IS the world-logic: the single lantern that can't be in two hands,
the gate that can't be rushed, the pound that must be given its minutes. A good
telling makes the reader FEEL those constraints as tension (Wick fumbling the
lantern hand-off in the dark; the unbearable wait for the pound to fill while the
Marrow drifts toward the weir), not just as bookkeeping. Keep it human and taut.
