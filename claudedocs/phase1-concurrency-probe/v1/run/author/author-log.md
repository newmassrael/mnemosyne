# Author log — The Backfire at Mara Ridge

## Scope decided up front

- **18 scenes**, three world-lines: a shared spine `main` (sc-01 .. sc-09), and
  two terminal world-lines that fork at sc-09 — the water-cart decision:
  - `hold` (sc-10a .. sc-14a): Bo sends the one cart down to hold the backfire's
    lower edge while a hand-team closes the granite gap by pick and bar — the
    cut closes, the two fires meet at the line, the town lives.
  - `fall` (sc-10b .. sc-13b): Bo keeps the cart with the saws to beat the
    granite faster; dry on its lower edge the backfire jumps the still-open
    draw, crosses the unfinished line, and the wind drives it onto the town.
- Both terminal worlds are fully authored and reach a resolution. The fork at
  sc-09 is the single load-bearing structural choice (premise truths #1, #2, #3
  collide there: the open line, the one cart, the dawn wall).

## Phase 0 — skeleton (tentpoles placed first, gate-clean before any detail)

I imported a skeleton (8 facts) BEFORE the connecting detail, in this order:

1. **Scope + reserved scene-id ranges** (sections.json, all 18 scenes) and the
   two branches forking at sc-09.
2. **Endings first**: f-025 (hold resolution — "ash on the ridge and nothing
   more") and f-033 (fall resolution — "gone by full light"), authored before
   any connective beat.
3. **The structure**: the fork branches `hold`/`fall` registered at sc-09; the
   forced cart-call fact f-014 at sc-09.
4. **Load-bearing entities**: bo, wrenna, tam, cart, cut, backfire, mainfire,
   town, draw, handteam.
5. **The one reveal + its setup, together**: the long-range setup f-008 (sc-06,
   "the granite seam leaves the line open", typed `line_state(cut)=open`,
   `payoff_expectation: expected`) authored in the SAME pass as both its
   payoffs — f-022 (hold, sc-12a, `line_state=closed`, `pays_off f-008`) and
   f-031 (fall, sc-11b, `line_state=breached`, `pays_off f-008`). The belief
   tentpole f-010 (Wrenna believes the line is closed) on the `wrenna-belief`
   frame.

Skeleton gate result (first pass, 0 repairs needed): continuity 0 violations;
fork-tree 2 world-lines, 0 unplaced fork points; payoff-coverage hold/fall
dangling=0, main dangling=1 (by design — see below); payoff-substantiation
substantiated in both terminal worlds; timeline-gaps 0; playthrough 0 unplaced /
0 undecidable in every world. The skeleton was a trustworthy spine.

## Phase 1 — detail fill

Filled the spine (sc-02 .. sc-09) and each branch's connective beats with 16
more facts: the plan (f-002), the two crews starting (f-003/f-005), the
dependence of Wrenna's pace on Bo's actual progress (f-006), Tam's relay
(f-007), the cart argument (f-009), the garbled relay that causes the false
belief (f-011), Wrenna quickening her fire on the bad word (f-012), the midnight
truth reaching Bo (f-013), and the per-branch beats (f-020..f-025, f-030..f-033).

Backreferences are STRUCTURAL (cited in `evidence`), not bare prose, e.g.:
- f-011 (garbled relay) cites [sc-08, sc-06, sc-05] — the granite seam it
  misreports AND the relay mechanism it travels on.
- f-013 (truth reaches Bo) cites [sc-09, sc-06, sc-08] — the seam and the
  quickened fire.
- f-022 / f-031 (the payoffs) cite sc-06 (the planted seam).
- f-023 / f-033 (endings) cite sc-02 / sc-01 (the plan, the original threat).
- f-024 (belief overtaken by truth, hold) cites sc-08 (where the belief was
  planted) — every backref's cited scene is on the shared spine, reachable
  at-or-before the citing fact in BOTH world-lines.

**Iteration count for the detail fill: one write -> gate pass for the prose
detail (0 repairs), then one further deliberate iteration to add the
state-machine modeling described below.** The continuity / fork-tree / payoff /
timeline / playthrough gates were all clean on the first full import; I did not
have to chase a single structural violation. (The schema discipline I read out
of the tool up front — typed subject must be in `entities`, evidence scenes must
exist, branch must be registered — meant the import validator never rejected a
fact, so the repair loop was about MODELING choices, not error-chasing.)

## State-machine pass (deliberate second iteration, not a repair)

To express premise truths #1 (the line must be closed before the backfire
reaches it) and #2 (one water cart, one place at a time) as TRACKED state rather
than only prose, I added a narrative-rules file (`rules.json`) with two
`exclusive` rules keyed `per: subject`:
- `one-cart` over `cart_with` — the cart is with at most one crew-lead at a time.
- `one-line-state` over `line_state` — the western-draw line is in one state at
  a time (open / closed / breached).

This exposed a real modeling requirement and is the one place I had to add
structure to make the story expressible — see Awkwardness #1.

## Awkwardness recorded honestly

**1. Point-in-time facts "hold forever" with no end, so a state CHANGE needs an
explicit `supersedes_in_frame` edge or the exclusive rule fires a false
overlap.** A fact with no `canon_to` holds open from its `canon_from` forward.
So "the cart was with Bo, then moved to Wrenna" was, naively, two facts
(`cart_with=bo` at sc-07 and `cart_with=wrenna` at sc-10a) that BOTH hold from
sc-10a onward — which the `one-cart` exclusive rule correctly reads as the cart
being in two places at once. The story I wanted (custody changes hands) is not a
contradiction; it is a succession. The fix the tool wants is to mark the moving
fact `supersedes_in_frame` the earlier one (f-020 supersedes f-009; f-022
supersedes f-008; f-030/f-031 likewise), which ends the predecessor's holding at
the successor's scene. That is the correct model and reads well in the
playthrough (it now prints explicit `- f-009 superseded by f-020` end-events).
But it IS a non-obvious step: a first-time author would naturally write the two
state facts and only discover, when the exclusive rule fires, that a state
*change* must be wired as succession, not left as two free-standing facts.
Without the rule turned on, the false overlap is silent (the bare continuity
gate does not flag two co-holding facts unless an exclusive rule keys on the
predicate) — so the modeling discipline is opt-in. I would not have caught the
"cart in two places" looseness if I had not chosen to author the exclusivity
rule that premise truth #2 literally describes.

**2. A long-range setup necessarily shows as DANGLING on the trunk world
(`main`), and that is unavoidable, not an error.** The granite-seam setup f-008
lives on the shared spine (sc-06); its payoffs (f-022 closed / f-031 breached)
live on the post-fork branches. `report-payoff-coverage` walks every world
including `main`, and a fork's payoff "never credits the ancestor's world" — so
on `main` f-008 is reported `[DANGLING]`. There is no way to discharge it on the
trunk without inventing a payoff that does not happen before the choice is made,
which would be false. The brief scopes the no-dangling requirement to TERMINAL
worlds, and on both terminal worlds (`hold`, `fall`) f-008 is paid and
substantiated. So this is correct-by-design, but it means the trunk world's
coverage report always carries the setup as dangling, and an author has to know
to read "dangling on main" as "expected: pays off after the fork" rather than as
a defect.

**3. `validate-continuity` is config-gated and silently disabled from the author
side unless you force a severity.** Run with only `--order`/`--sidecar` the gate
prints `continuity gate: disabled ([continuity] table absent)` and exits 0 —
i.e., it passes vacuously because no `[continuity]` table exists in the
workspace `mnemosyne.toml`. To actually enable the structural + interval +
rule checks from the author side without editing the repo config, I passed
`--severity reject` (the flag overrides the table-absent disabled state). I ran
every continuity check with `--severity reject` so the gate was genuinely
enforcing, not vacuously passing. (One related path detail: `--order` is
CWD-relative but `--rules` resolves workspace-root-relative, so I invoked from
the repo root and gave `rules.json` its repo-root-relative path.)

**4. The "wind turns at dawn" is one event that both endings share in spirit but
must be authored as two separate facts on two scenes (f-025 on sc-14a, f-033 on
sc-13b).** The dawn deadline is a single ground-truth pressure (premise truth
#3), but because what is TRUE at dawn differs per world (town saved vs. town
lost), the deadline cannot be one shared-spine fact — each terminal world states
its own dawn outcome. This is correct (the outcome genuinely diverges), but it
means the single most thematically-unifying beat of the premise — "when the wind
turns, whatever is done is done" — is not expressible as one fact; it is split
across the two endings, with only the original threat (f-001, sc-01) shared on
the spine and cited by both via `evidence`.

None of the four required restructuring of the STORY; #1 and #4 required
choosing the right tool primitive (succession edges; per-world ending facts) to
express a story shape the tool does support, and #2/#3 are reading-the-report
caveats rather than authoring obstacles.

## Final gate state (all clean, all enforcing)

- `validate-continuity --severity reject --rules rules.json`: facts=24,
  conflict_pairs=0, rules=2, unchained_state_pairs=0, **0 violations** (exit 0).
- `report-fork-tree`: 2 registered world-lines, **0 unplaced fork points**;
  both forks placed at sc-09 (exit 0).
- `report-payoff-coverage`: `hold` dangling=0, `fall` dangling=0; `main`
  dangling=1 = the granite setup (by design, paid post-fork) (exit 0).
- `report-payoff-substantiation`: f-008 **substantiated** in both terminal
  worlds (typed state-change open->closed on hold, open->breached on fall).
- `report-timeline-gaps --world {main,hold,fall} --rules rules.json`: **0 gaps**,
  0 unverifiable in every world (exit 0).
- `report-playthrough-manuscript --world {main,hold,fall}`: **0 unplaced,
  0 undecidable, 0 undeclared adjacencies** in every world; each world reads
  start to finish (exit 0).
