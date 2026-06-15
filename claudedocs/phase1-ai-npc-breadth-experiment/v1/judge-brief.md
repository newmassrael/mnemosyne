# Judge brief — is this crowded-house story coherent, and is its CAST real?

You are reading a branching story presented as a **scene-by-scene factual outline**
(one section per world-line: the events of that path in order). This is an OUTLINE,
not finished prose — so judge the STORY'S LOGIC, COMPLETENESS, COHERENCE, and the
REALITY OF ITS CAST, NOT the writing style, word choice, or polish. A bare sentence
is fine; a plot hole or a person who knows the impossible is not.

You will be given each world-line in turn. Read them all, then answer.

Do not speculate about how the outline was produced.

## How to read the outline (important — the frame labels carry the cast)

Each scene lists the facts that come into play, and every fact is tagged with a
**point of view** in parentheses:

- `(gt)` = the GROUND TRUTH — what actually happened, whether or not anyone in the
  story knows it.
- `(wend)`, `(clerk)`, `(soldier)`, `(hooded)`, … = what THAT PERSON knows or
  believes at that point. A fact in a person's frame is part of that person's mind.

So `+ x (clerk): the clerk believes the soldier robbed the box` means the clerk
BELIEVES that — it need not be true (check `(gt)`). Two people can hold opposite
beliefs; each is a true fact about a different mind. A `- y (wend): superseded by …`
line means Wend's earlier belief was replaced (they learned better).

Use these labels to track WHO KNOWS WHAT as the nights unfold.

## For each world-line and for the story as a whole, rate 1–5 and cite scene ids

1. **Causal coherence** — does every consequence have a cause already present earlier
   in the SAME world-line? (5 = airtight; 1 = events happen for no reason.) Count the
   breaks and cite scene ids.

2. **Completeness** — are there HOLES a reader notices: a missing step, an unanswered
   question the story raises, a setup that never returns, a person who acts without
   being established? List each with the scene id near it. (5 = nothing missing.)

3. **Knowledge realism** — read the person-frames against the ground truth. Does each
   person know ONLY what their own path through these nights would let them know — what
   they witnessed, were told, or could infer? Flag anyone who HOLDS a fact they had no
   way to come by: a person "knowing" something only others witnessed, knowing an event
   from a world-line they are not on, or believing something the story gave them no
   occasion to form. Also flag the opposite: a person who SHOULD plainly know something
   (they were right there) but is shown ignorant of it. Count and cite. (5 = every
   person's knowledge is exactly earned; 1 = people know the impossible or are blind to
   the obvious.)

4. **Cast distinctness** — is this a real crowd of individuated people, or
   interchangeable figures? Do the travellers hold genuinely DIFFERENT knowledge,
   agendas, and beliefs — several of them wrong about each other in different ways — or
   are they palette-swaps who all know roughly the same thing? (5 = a real, varied
   house of minds; 1 = one truth and a set of name-tags.)

5. **Branch integrity** — does each world-line reach its OWN real, distinct ending, and
   does that ending follow from the choice (NAME / HOLD / ACT) that branched it?
   (5 = each road earns a distinct, motivated ending; 1 = branches collapse together.)

6. **Overall** — taken whole, is this a COHERENT AUTHORED STORY WITH A LIVING CAST, or
   an internally-consistent pile of facts that does not add up? Give 1–5 and ONE
   paragraph naming the single biggest strength and the single biggest weakness.

## Output format

```
World-line <name>: coherence X/5, completeness X/5, knowledge X/5, cast X/5, branch X/5
  - <flaws with scene ids, or "none">
... (repeat per world-line) ...
OVERALL: coherence X/5, completeness X/5, knowledge X/5, cast X/5, branch-integrity X/5, overall X/5
Biggest strength: ...
Biggest weakness: ...
Knowledge breaks (scene-id : who knows/ignores what, and why it's wrong), or "none": ...
One paragraph: ...
```
