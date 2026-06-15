# concurrency-probe/v1 — report (R549 execution)

**Manifest** sha256 `3b99b5978475ec58760bb282defc54c47b7787e2a2df5108f91737685638c8b3`
(pinned pre-execution in the R548 ledger). **Premise:** The Backfire at Mara Ridge.
**Outcome: OUTCOME-2b (refined) — a told story does NOT need interfering-concurrency
substrate; it renders as a coupled spine + an exclusive pivot, the R547 exclusive-rule
idiom expresses the contention, and the one thinned affordance (continuous live
contention) is pinion/runtime, not Mnemosyne authoring. The R547/R546 layer-split is
CONFIRMED on data. NO Mnemosyne build.**

## What the blind author did (firewall held: it never saw the hypothesis/measurement)

18 scenes, ground-truth + 1 belief frame (`wrenna-belief`), **1 fork at sc-09 → 2
exclusive terminal world-lines** (`hold` = town lives / `fall` = town lost), turning on
ONE decision: where the single water cart goes at midnight. The two crews (Bo's cut,
Wrenna's burn) are narrated together on the shared spine sc-01..sc-09; the premise's
timing interference (the backfire's pace depends on the cut, f-006), the information lag
(Tam the runner, f-007; the garbled relay, f-011/f-012), and the belief divergence
(Wrenna believes the line closed, f-010) all live on the spine and FEED the midnight
pivot. The cart contention and the line state are tracked as **typed mutually-exclusive
state**: predicates `cart_with` / `line_state`, two `exclusive` narrative rules
(`one-cart` per subject, `one-line-state` per subject), and `supersedes_in_frame`
succession edges for the state changes (cart bo→wrenna; line open→closed/breached).

## Deterministic measurement (orchestrator rebuilt FRESH from the author's JSON)

All gates clean on the independent rebuild (not trusting the author's store):
validate-continuity (with `--rules`, `--severity reject`) = 24 facts / 18 nodes /
conflict_pairs=0 / rules=2 / rule_unordered=0 / unchained_state_pairs=0 / **violations
0**; fork-tree = 2 world-lines, 0 unplaced; payoff-coverage = hold/fall dangling 0 (the
granite setup f-008 paid + substantiated in both terminal worlds, dangling-by-design on
the trunk); timeline-gaps 0 both worlds.

- **M1 — interleaving fork-explosion tax: ZERO explosion.** ONE binary fork (2 branches),
  and it encodes a genuine STORY CHOICE (where Bo sends the cart), not an enumeration of
  concurrent interleavings. No N!-style branch blow-up. The duplication is the normal
  two-ending fork cost (the dawn outcome + line resolution authored once per branch), not
  concurrency-specific.
- **M2 — cart contention expressed STRUCTURALLY (the R547 idiom, reached for UNPROMPTED):
  YES.** The author independently discovered the R547 lever — typed possession/state
  (`cart_with` / `line_state`) + the R449 `exclusive` rule + `supersedes_in_frame`
  succession — to make "one cart in one place at a time" and "the line in one state at a
  time" tracked invariants the gate enforces. This is a strong positive for R547: the
  declarative possession/state idiom is natural and discoverable without prompting. NOTE:
  the exclusivity is enforced as a SEQUENCE over time (cart with bo, THEN wrenna via
  succession), i.e. sequential exclusivity, not live simultaneous contention.
- **M3 — AND-join: ORDER expressible, COMBINED-STATE in prose.** The dawn survival is one
  fact per branch (f-025 hold / f-033 fall); the combined-state condition ("cut complete
  AND backfire met AND neither failed for water") lives in the f-025 PROSE CLAIM, not as a
  structural multi-predecessor node fusing two concurrent thread tails — exactly the
  irreducible boundary R547 named (store-consistency ≠ causal coherence). No structural
  AND-join was needed because there are not two concurrent threads to join — there is one
  spine and a fork.
- **M4 — author friction: NO concurrency-loss lament.** The 4 logged awkwardnesses are
  (1) state-change-needs-succession (the exclusive-rule mechanics — used correctly, a
  positive), (2) setup-dangles-on-trunk (expected), (3) validate-continuity config-gated
  (operational; fixed with `--severity reject`), (4) the dawn is two facts on two scenes
  (a fork consequence). NONE is "I wanted to express two concurrent interfering threads
  and could not." The author found spine+fork natural and sufficient.
- **M5 — concurrency rendering: coupled spine + exclusive pivot (the M5(b) shape, but not
  experienced as a loss).** The interference (the contended cart, the timing race) became
  a single decisive CHOICE, which the exclusive-branch substrate already handles.

## Blind judge (1 judge, firewalled, simultaneity-coherence read)

- Q1 two-crews-at-once: **[SIMULTANEOUS]** — "genuinely simultaneous, coupled two-crew
  situation; the interdependence and the costly shared resource land."
- Q2 cart/timing tension: **[MIXED]**, leaning single-choice — "the contention is real but
  collapsed into one binding decision at midnight ... decided in a single stroke rather
  than fought across the night."
- Q3 anything lost: **[PARTIAL]** — coupling, information lag, the either/or cost, both
  outcomes are "fully captured"; what is thinned is the *race texture* — "the minute-by-
  minute attrition of two crews trading time and water across the whole night is more
  implied than enacted ... compressed into one midnight decision."

## Reading (honest, per the don't-downplay discipline)

The interfering-concurrency premise rendered cleanly as a **coupled shared spine + one
exclusive pivot** on the contended resource. The coupling and interference the premise
demanded ARE captured (the judge: SIMULTANEOUS), and the resource exclusivity is
expressed by the R547 idiom UNPROMPTED (M2). What the told story THINS — and the judge
flagged it as a real PARTIAL loss — is the *continuous, live, minute-by-minute
contention* (the cart trading back and forth, the race re-negotiated across the night).
**That thinned affordance is precisely the runtime/gameplay layer** (the player living
through the interleaving), which R527 gap #4 + R546 HALF A + R547 already assign to
pinion/SCE executable behavior — NOT the Mnemosyne authoring substrate. So the probe
CONFIRMS the layer-split on a story designed to stress it: authoring needs the coupling +
sequential exclusivity + the pivot (all expressible today); the live contention is
pinion's.

## Decision (pre-committed rule)

**OUTCOME-2b — NO Mnemosyne build.** Not OUTCOME-1/1b: no fork-explosion (M1=0), no
inexpressible contention (M2 structural), no lamented dodge (M4). The L3-(ii) interfering-
AND-thread build stays YAGNI-DEFERRED for Mnemosyne; the thinned live-contention affordance
is a NAMED pinion/runtime requirement, not a Mnemosyne authoring gap.

## Honest limitations (do not over-read 2b)

- **n=1 premise, n=1 author, n=1 judge.** Suggestive, not proof.
- **Single-contention-point premise.** The premise had ONE decisive contended resource
  (the cart) that collapsed cleanly to one fork. A premise with MANY independent
  simultaneous contention points might fork-explode (untested) — this probe shows THIS
  interfering-concurrency shape does not force a build, NOT that none ever could. A future
  v2 with ≥3 independently-contended simultaneous threads is the trigger for re-testing.
- **The judge's PARTIAL is a real craft loss at the reading level** — it is out of
  Mnemosyne's scope (it is pinion's), not a defect erased by the clean gates.

SSOT = this report + the R548 manifest + the R549 ledger. Tracked evidence:
manifest/premise/brief/runbook + run/author/{sections,facts,order,rules}.json +
author-log.md. Scratch *.atomic.json gitignored.
