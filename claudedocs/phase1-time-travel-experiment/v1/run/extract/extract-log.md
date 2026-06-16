# Extract log — independent re-reading of the played prose

Read only: `extractor-brief.md`, `vocab.md`, `render/world-trunk.md`,
`render/world-planted.md`. No author fact base, manifest, runbook, or premise
opened.

Counts: 40 scenes (sc-01..sc-26 main + sc-27p..sc-40p planted); 44 facts
(by frame — gt 30, bearer 7, sera 3, halden 1, marn 1, pell 1, ode 1;
by branch — main 27, planted 17). `reextracted.atomic.json` imported clean
(40 sections, 8 frames, 1 registered branch `planted` + implicit `main`,
16 entities, 2 predicates, 44 facts).

## WHEN each Founding cause is first stated by the prose (the point of the re-read)

The prose does NOT withhold the Founding causes until the late age. It states
both causes, plainly and on the page, in the FOUNDING scenes themselves, as
the acts happen — long before the sc-39p crossing:

### (i) The cistern is sealed / runs clean because of a past act
- **sc-03** — the rot is first stated: Sera finds "a creeping rot ... it'll
  foul everything" — cistern condition = rotting. (the problem, on the page)
- **sc-04** — the SEALING ACT is first stated outright: Sera packs the lower
  vent course by course, "sealing the rot off from the running water," and
  says "That holds it." This is where the prose first commits the cistern to
  the `sealed` state, by a named act of a named keeper. **This is the first
  place the prose states that the cistern is sealed because of a past act.**
- **sc-05** — the keeper's MARK is first cut: "three strokes and a notch."
- **sc-06** — cistern condition first stated `clean`: "the cistern runs
  sweet ... the rot is sealed away below."
- The CAUSAL LINK across the ages is then re-stated explicitly at **sc-31p**
  (cistern still runs clean three generations on; bearer finds the same mark)
  and named in so many words at **sc-39p** ("This is why the cistern runs
  clean three generations on. This is the seal under all that good water.").

### (ii) The ford was marked by a past hand / why it holds
- **sc-09** — the firm ford is first found and its REASON stated: Halden reads
  the river, "here the bed's bone under the gravel ... this one stays firm."
  The narration adds "No one has marked it" — i.e. at sc-09 the ford is firm
  but not yet marked.
- **sc-10** — the MARKING ACT is first stated: Halden "cuts it to mark the
  firm ford ... the dispute closed in stone."
- **sc-11** — the stone is bedded "at the head of the firm ford ... the one
  ford that stays, named now in stone at its head." This is where the prose
  first binds boundary-stone -> ford as a placed, standing mark.
  **So the past hand that marks the ford is named (Halden) and the act is on
  the page at sc-10/sc-11 — not later.**
- The CAUSAL LINK is re-stated at **sc-35p** (Ode digs up the stone, reads it
  marks a firm ford, "Somebody knew this water") and named outright at
  **sc-39p** ("This is why the ford holds.").

### Direct answer to the re-read's framing question
The prose does NOT first reveal the Founding causes at the sc-39p crossing.
Both causes are stated as ground-truth acts in the Founding trunk: the cistern
seal at **sc-04** (clean state at sc-06), the ford mark at **sc-10/sc-11**.
sc-39p is a RE-STATEMENT / explicit naming of the causal arrow ("this is
why"), not the first appearance of the cause. The late-age scenes that show
the EFFECTS (sc-31p clean cistern, sc-36p holding ford) are matched by
late-age scenes that explicitly disclaim KNOWLEDGE of the cause
(sc-29p Marn, sc-35p Ode) — see "late-age causal statements" below.

## Late-age statements of a Founding-Age cause — do any "come from nowhere"?

None come from nowhere in the planted reading. Every effect the late age
relies on has its cause already shown in the Founding trunk, and the prose is
careful to mark the late-age characters as NOT knowing the cause:

- **sc-29p (marn frame)** — Marn explicitly does NOT name the cause: "Why
  this cistern, and no other ... he does not know the reason ... The why of
  it is a thing his age has lost." This is the late age VOICING THE ABSENCE
  of the cause, not voicing the cause. Recorded as a marn-frame fact.
- **sc-31p (bearer/gt)** — the cistern condition `clean` is re-stated as a
  present late-age fact, and the bearer (not Marn) finds the mark. The bearer
  recognises it only because the bearer watched the sc-04/sc-05 act; the
  knowledge is grounded, not from nowhere.
- **sc-35p (ode frame)** — Ode reads the stone and says only "Somebody knew
  this water" and "does not know who set it or when." Again the late age
  voices the EFFECT and an explicit non-knowledge of the maker.
- **sc-39p (bearer frame)** — the only place a character (the bearer) STATES
  the Founding cause outright in causal terms ("This is why ..."), and the
  prose grounds it: the bearer is at that moment watching Sera and Halden
  perform the very acts shown in sc-04/sc-05 and sc-09/sc-11. So the causal
  naming at sc-39p is fully earned by the on-page Founding acts; it is a
  payoff of setups, not an unsupported assertion.

Net: there is NO place where a late-age character voices a Founding-Age cause
that the text had not already shown. The "who sealed the cistern / who marked
the ford" causes are visible to the reader from sc-04/sc-05 and sc-10/sc-11.

## Genuine ambiguities / judgement calls in extraction

1. **sc-06 vs sc-31p cistern=clean.** Both scenes state the cistern runs
   clean, on different branches (main / planted), so I recorded BOTH as typed
   `condition:clean` facts at their own canon_from. They are not duplicates:
   one is the Founding result, one is the persisted late-age state.
2. **boundary-stone possession ford at sc-11 and sc-35p.** I used the
   `possession` predicate to express "the stone marks / belongs to the firm
   ford" because the vocab offers only `condition` and `possession`, and
   "marks the firm ford" is a placement/binding relation, not a scalar state.
   Recorded at both sc-11 (set) and sc-35p (found/re-read). This is a vocab-
   fit judgement; the prose says "cuts it to mark the firm ford" (sc-10/11)
   and "a mark of the firm ford" (sc-35p).
3. **sc-09 "no one has marked it."** At sc-09 the ford is firm but explicitly
   NOT yet marked, so I recorded the firmness-finding (sc-09) separately from
   the marking (sc-10/sc-11). The typed boundary-stone->ford fact is bound to
   sc-11 (where it is placed), not sc-09.
4. **sc-04 typed `sealed` vs sc-06 typed `clean`.** The prose distinguishes
   "the rot is sealed off" (the act, sc-04) from "the cistern runs sweet/
   clean" (the result, sc-06). I kept them as two typed steps to preserve the
   rotting -> sealed -> clean progression the prose actually walks.
5. **sc-18, sc-23, sc-25** — transition/beat scenes that restate already-
   recorded facts (the bearer carrying the pipe/sapling, the gorge cutting the
   far field, the dusk decision moment). I did not mint new facts for pure
   restatement to avoid double-counting; the sections exist in sections.json.
6. **Vocab object kind.** vocab.md described typed object as
   `{kind:"literal", value:...}` but the CLI accepts `{kind:"value", value}`
   / `{kind:"entity", id}`; I used the CLI's accepted form (value/entity),
   which is the only one that imports.
7. **`main` branch.** The CLI rejects registering `main` (default world-line,
   known by construction). I dropped it from the branches registry but kept
   `branch:"main"` on the 27 Founding facts per vocab ("Omit branch or use
   `main`"); only `planted` is registered, forking from main at sc-26.
