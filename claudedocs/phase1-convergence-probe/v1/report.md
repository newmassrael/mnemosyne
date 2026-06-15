# convergence-probe/v1 (TEST-1) — report

**Question.** When a blind author authors a story whose structure REQUIRES
convergence (multiple divergent world-lines reach ONE shared dawn reckoning + a
shared final image) on the CURRENT forest substrate, is the author forced into a
damaging workaround, or can convergence be expressed cleanly with no loss?

**Setup.** R530 manifest sha256 `46df8c997b0661703258f5a47f1dea84e261af21d069e03a10847878692fa2ce`
pinned pre-execution. ONE fresh-context blind author (R469 bound), given ONLY
`premise.md` ("The Reckoning at Harlow Mill") + `author-brief.md` (the proven
R520/R524 top-down contract, NO convergence instruction added). Orchestrator
rebuilt the store fresh from the author's `sections.json`/`facts.json`/`order.json`
and ran the gates independently.

**Authored base.** 18 scenes, 39 facts, 2 frames (`gt` + `town-belief`), 2 terminal
world-lines (`sluice`, `ride`) forking at `sc-08`, both converging at the shared tail
`sc-reckoning → sc-river`. 4 write→gate→repair iterations. Rebuilds clean.

## RESULT — OUTCOME-1 (hard pull). The forest's convergence duplication tax is real and was paid.

| Measure | Finding |
|---|---|
| **M1 duplication tax** | The shared ending = **6 story beats authored as 12 facts — exact 2× duplication.** `f-300a≡f-300b` ("the town gathers in the mill yard"), `f-301a/b`, `f-302a/b` (the wedged-gate truth surfaces), `f-303a/b` (Garrick's verdict), `f-304a/b`, `f-400a≡f-400b` ("Sela walks to the river's edge"). The whole shared continuation is authored once per branch. |
| **M2 gate trips** | **0.** `validate-continuity` = 0 structural / 0 interval. The author AVOIDED cross-branch references (which would trip R488/R522) by DUPLICATING instead — duplication is the workaround the forest funnels you to. |
| **M3 fork-tree vs story shape** | **Tree, not diamond.** `report-fork-tree` shows only the divergence (`sluice`/`ride` fork from `main` at sc-08). It does NOT — cannot — show that both re-merge at `sc-reckoning`. The story is a diamond; the structural graph emits a tree. The convergence is invisible to the registry. |
| **M4 author friction** | **Noticed and named, independently.** The blind author (no knowledge of the hypothesis) wrote: *"The fork model is a forest (each branch has a single parent), so there is no native 'join' node... convergence is expressed, not declared: nothing in the registries says 'these two branches join here'... A reader of `branches[]` alone would not see it... the same dawn beat had to be authored as TWO parallel fact sets... That duplication is unavoidable given the model: a fact belongs to exactly one branch."* |
| **M5 convergence dodge** | **No dodge — convergence was MET** (both paths faithfully reach the shared reckoning + river), via duplication, not by flattening or splitting the ending. |

## Why this is decisive

A blind author, given a convergence-shaped premise and the proven forest contract,
**independently rediscovered the exact R528 finding** ("no native join node",
"convergence expressed not declared", "duplication unavoidable") and **paid the
tax in full**: the entire shared continuation (6 beats) authored twice (12 facts).
The build removes exactly this — author the shared ending ONCE, both incoming lines
inherit it. The tax also has a hidden maintenance cost: editing the reckoning means
editing both copies, and the two copies can silently drift (nothing links f-300a to
f-300b).

## A refinement of the R528 design (field-surfaced, append-only)

The author found that sharing a scene-id at the tail of BOTH branch orders
(`sc-reckoning` in both chains) is "legal and clean" — the canon-ORDER already
TOLERATES a shared tail node, gate-clean. This FIELD-VALIDATES R528's core code
finding: the order algebra (`le()`/`closure_of`) is already DAG-general and did not
resist the shared tail; the gap is exactly where R528 placed it — (a) the BRANCH
REGISTRY cannot DECLARE the merge (so it is invisible to `report-fork-tree`/
`branches[]`), and (b) a FACT carries one branch, so the shared content must be
DUPLICATED. The `confluence` design (multi-parent branch declaration + fact-sharing
at/after the merge + the per-parent reachability gate) targets precisely these two,
not the order algebra. The experiment confirms the design is aimed correctly.

(Secondary, not about convergence: the author noted `report-payoff-substantiation`
required re-modelling "Sela IS the keeper" — a constant — as a transition
`contested→affirmed` to discharge a typed setup→payoff. A tool-shaping observation
worth a future look; orthogonal to convergence.)

## Decision (pre-committed rule) — PULL the convergence-JOIN build

OUTCOME-1 fires: convergence met but via 2× duplication of the shared continuation
(M1), with the convergence structurally invisible (M3), the author funneled to
duplication over gate-tripping cross-refs (M2), and the friction independently named
(M4). Per the R530 decision rule, this PULLS the (a) convergence-JOIN BUILD
(`confluence` multi-parent ancestry + the per-parent "no unreconciled dependency
across a confluence" gate + bounded cross-confluence succession). The measured tax
the build removes = the whole shared ending, 6 beats / 12 facts, duplicated. After
the build, re-run THIS premise on the new substrate = the full fork→join validation.
