# dnd-quest-experiment/v2 — report (R564 execution)

**Question:** can a TARGETED repair — re-render the deficient scenes plus the
minimal STORY ADDITIONS needed to close the in-frame knowledge-acquisition seams
— lift the v1 D&D quest-game to a genuinely high-completion scenario without
breaking any gate? (The owner's R562-followup; the R553 render-repair loop
extended to author-side additions; it attacks the store-consistency ≠
causal-coherence residue R553/R562 left open.)

**Outcome: REPAIR WINS, 3-0 blind.** All structural pins hold (PIN-A/PIN-B, and
PIN-C gates); 2 of the 3 acquisition seams fully close, the 3rd materially
improves with one named irreducible residual; 3 blind A/B judges UNANIMOUSLY
prefer the repaired version on knowledge-realism AND agency AND overall. Manifest
sha256 `4ea239df21448478f096808734df1e8e4322d2a1bd068c9fa3f3d910da5318d3`
(pinned in R563 before any subagent ran).

## The repair (blind repair-author, surgical)

123 facts (v1 was 121). Diff vs v1: ADDED f-044 (D1: the last delver went down
TRACKING the corpse-tithes — `payoff_expectation:expected`, paid off by the
sc-14 journal f-130) + f-122 (D2: the rogue tells the party the overheard
incense/shrine clue at sc-13); AMENDED f-130 (pays_off f-044), f-132 (evidence
+= sc-13), f-171/f-172 (D3: the wizard counts the same three roads as the warden
— unmake/claim/oath, the oath the third not a fourth). No new scene; no
renumber; sections/order/narrative-rules byte-identical to v1.

## Pins (orchestrator-run)

**PIN-A repair gate-clean — PASS.** Fresh rebuild from the repaired JSON: 123
facts, 0 errors. validate-continuity 0/0; fork-tree 3 placed/terminal;
manuscript unplaced/undecidable 0 all roads; payoff-coverage terminal danglings
IDENTICAL to v1 (shatter {f-060,f-171}, claim {f-041,f-171}, parley {f-060}) —
the D1 setup f-044 is paid on every road, the D2 hand-off orphaned nothing, the
per-road quest divergence (q-delver / q-reliquary) is intact; PIN-2
quest-contract + PIN-3 map (4 surfaces → MapLocators) preserved. Adding story did
not break the game.

**PIN-B localization — PASS.** Fact half: the only facts differing from v1 are
the D1/D2/D3 named set (added f-044, f-122; amended f-130, f-132, f-171, f-172) —
diff-confirmed, nothing else touched. Render half: the spliced repaired render
differs from v1 in EXACTLY the 6 affected scenes (sc-05, sc-13, sc-14, sc-18,
sc-29c, sc-30c) — every other scene byte-identical. Surgical.

**PIN-C seams-closed + gates-hold — PASS (gates) + 2/3 seams closed, 1 residual.**
- leak: leaks=0, vocabulary_shared=18 (non-vacuous), exit 0 — the additions
  introduced no premature reveal.
- fidelity: off_path=0, unplaced=0, reached_terminal=true — the repair stays on
  the claim road.
- seam re-check (blind extractor's NEW extract-log, the independent signal):
  - **D2 (Pip's incense hand-off) — CLOSED.** "sc-13 explicitly closes that gap
    on-page: Pip tells the other three over the body before Lysa deduces. No
    knowledge-from-nowhere."
  - **D3 (ending count) — CLOSED.** "consistently three, stated four times with
    no disagreement; the non-choice explicitly excluded as no fourth road."
  - **D1 (journal naming the shrine-keeper) — IMPROVED, one residual.** Now a
    SHOWN chain (watch → follow → track → name) rather than a bare conclusion,
    BUT it is the dead delver's SUMMARIZED investigation — "the prose never shows
    the specific link tying a corpse-tithe to Vane; the reader trusts the dead
    man's record." This is the irreducible residual, exactly as pre-registered
    (PRED_C): the last delver is dead from scene one, so his identification is
    necessarily off-page backstory delivered through a found journal — closing it
    fully would need a flashback (more story than a targeted repair).

## Judged A/B (3 blind judges, v1 vs repaired, randomized, label-map sealed)

| forced call | J1 | J2 | J3 | result |
|---|---|---|---|---|
| stronger knowledge-realism | repaired | repaired | repaired | **repaired 3-0** |
| stronger agency | repaired | repaired | repaired | **repaired 3-0** |
| more finished overall | repaired | repaired | repaired | **repaired 3-0** |
| overall score (repaired / v1) | 5 / 3 | 4 / 3 | 5 / 3 | **repaired wins** |

All three, blind, independently identified the un-repaired arm's weaknesses as
EXACTLY the repair-spec's targets: the unrouted incense clue at sc-14, the
"three ends + a fourth thing" wobble at sc-18, and the "same end whichever way
you turned it" hedge at sc-30c. Every judge named the repaired arm's sc-13 Pip
confession + sc-14 three-witness synthesis + the sc-30c committed doom ("whose
brow it ruled from") as what made it the finished version. The seam spot-check
cut FOR the repair (each judge flagged the v1 arm's sc-14 unrouted clue; none
found a comparable seam in the repaired arm).

## Decision (pre-committed rule)

`repair_wins` FIRES: PIN-A + PIN-B + PIN-C(gates) hold; the 3 blind A/B judges
unanimously prefer the repaired arm on knowledge-realism AND agency AND overall;
the targeted seams D2/D3 closed and D1 materially improved. ⇒ targeted repair +
minimal story additions close the in-frame acquisition residue and lift the game
to high completion; the critique → repair → re-gate loop extends from render-only
(R553) to AUTHOR+RENDER repair — a validated completion loop (a prompt/stage
recipe, build YAGNI-deferred).

## Honest residue (not downplayed)

- **D1 is the named irreducible limit.** An off-page dead investigator's
  identification of the villain is backstory delivered through a journal; a
  seeded setup makes the journal's chain SHOWN (and the judges scored it the
  winner), but the specific corpse-tithe→Vane observation is never on the page.
  Closing it fully = a flashback / a living witness = more story than a surgical
  repair. This is the store-consistency ≠ causal-coherence boundary realized on a
  concrete case: the gate (leak/fidelity) passed at every stage; the residual is
  a prose-craft acquisition gap only a human-or-judge reads, not a gate.
- **n=1**: one repair-author / renderer / extractor / 3 judges; the same CLAIM
  road. The seam re-check rests on the blind extractor's reading (the R500/R512
  AI-judgment boundary).
- **headroom was small** (v1 already 5/5/5 knowledge): the A/B forced choice
  could have come down to a thin margin, but it did not — the sweep was 3-0 with
  multi-point overall gaps, and the blind judges' independent naming of the exact
  deficiencies is the stronger signal than the scores alone.
- **still a declarative-scenario repair**, NOT a played quest lifecycle (the
  R559/R546 boundary). "High-completion scenario" = a high-completion authored
  SCENARIO, not a running game.

## Bottom line for the owner's question

Yes — targeted re-render plus minimal story additions produced a measurably
higher-completion scenario (3-0 blind, the exact flagged flaws closed), and the
ONE thing it could not fully close (a dead man's off-page deduction) is named
and is a "needs a flashback," not a structural defect. The repair loop is a
real, validated tool for driving a self-authored game toward high completion.
