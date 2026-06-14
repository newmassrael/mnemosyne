# disclosure-craft-experiment/v2 — tool-run gate validation report

**Round R512. Lineage R501–R511 (this orchestrator). Result: the R507/R508
disclosure render-acceptance gates REPRODUCE R504's manual PIN verdicts on real
blind-re-extracted prose, deterministically, with the R505 manual-mapping step
GONE.**

## Question

v1 (R503 design → R504 execution) demonstrated PIN-1/PIN-2 on the Meridian Vane
disclosure prose **by hand** (manual id-mapping; the gates did not yet exist).
R506 designed and R507/R508 built the typed-tuple gates whose whole point is to
remove that manual step. v2 asks: do the **tool-run** gates, over a fresh blind
re-extraction that shares the authored *vocabulary*, reproduce R504's verdicts —
and does the fidelity gate reject a wrong-world-line render (the R504 footgun)?

## Scope (honest)

v2 here is the **deterministic gate-validation core** — RESUME goals (a) tool
gates reproduce R504's pins, (b) the fidelity negative case, (c) PIN-1/PIN-2
hold. The **craft re-judge over harness-produced reading copies (goal d)** is a
*separable, lower-value robustness leg* and is DEFERRED: R504 already established
the craft verdict (disclosure won the prose-quality keystone 5.00 vs 4.22 but
lost the overall forced choice 2-1), and R505 verified the contamination-repair
fair. Re-judging is robustness on an established result, not new signal — running
it as a 4th-subagent leg risked a half-finished run, so it is left as the next
pull. This round proves the *engineering* claim (the substrate works on real
blind prose); the craft claim stands from R504.

## As-built protocol (mechanized re-validation of R504)

REUSE keeps it low-touch: the disclosure prose is R504's `run/disclosure/story.md`
verbatim (the `## sc-XX` scene markers it already carries are the shared canon
coordinates); the fact base is R504's `ff.atomic.json`. The only NEW work is
structured (the disclosure *treatment*), not prose — the contamination bound
(R469/R502: this lineage may not author/extract/judge its own prose) holds, the
orchestrator only specified the treatment and ran the deterministic gates.

1. **Authoring (orchestrator).** `authored.atomic.json` = a copy of the v1 fact
   base, schema auto-migrated v21→v22. Only 5 of the v1 facts were typed (which
   is *why* R504 needed manual mapping). Faithful typing of the existing claims
   added the two load-bearing solution conclusions as gate-matchable tuples (no
   fabula change — `amend-fact` re-states an existing fact):
   - `gt-climber-pike` → typed `climber / identity / pike` (new entity `climber`
     "the unnamed figure who climbed", new predicate `identity`).
   - `gt-telegram-undelivered` → already typed `crane / knows-dismissal / no`.
   Disclosure plan `dc-v2` (default `withhold`, the sparse R501 ethos): both
   conclusions `state` with per-world reveal pins —
   `gt-climber-pike` first_at `{reveal: sc-17r, audit: sc-19b}` (withheld in
   burn = no pin); `gt-telegram-undelivered` first_at
   `{reveal: sc-11a, burn: sc-11a, audit: sc-19b}`. Coverage: disclosed=2,
   never_planned=50 (sparse).
2. **Canon order (orchestrator).** `meridian-order.json` — the branch-scoped
   declaration (main spine + per-branch edge sets for confront/audit/reveal/burn)
   the gates compose with the store's fork ancestry. Branch scoping is required:
   a burn node must NOT be "named" by the reveal world (else fidelity can't see
   drift).
3. **Blind re-extraction (separate blind subagent, fresh context, firewall).**
   The neutral-named prose copy + the fixed vocabulary (`extractor-brief.md` +
   `skeleton.atomic.json` = the registries, zero facts, plan removed). The worker
   was blind to the plan, the hypothesis, and which arm this is. It produced
   `extract/A.reextract.atomic.json` = **45 gt facts**, canon-from = the prose's
   own `## sc-XX` markers, EXPLICIT text only. The three gate-relevant tuples and
   their scenes (the worker's own report):
   - `crane/knows-dismissal/no` @ **sc-11a** (confront path) and @ **sc-19b**
     (audit path)
   - `climber/identity/pike` @ **sc-17r** (reveal limb names Pike)
   - Deliberate SILENCES (correct per "explicit only"): **no** `climber/identity`
     fact in the burn limb (sc-17w, left unnamed) or the audit limb (sc-19b
     "Reasoned, Not Seen" — inferred via the kept key, never named).

## Results — the tool gates

### PIN-2 premature-leak gate (`validate-disclosure-leak`) — HOLDS

| world  | targeted | **leaks** | unordered (honesty) | truth_frame_typed | **vocab_shared** | exit |
|--------|----------|-----------|----------------------|-------------------|------------------|------|
| reveal | 2        | **0**     | 1 (telegram @sc-19b vs pin sc-11a) | 7 | **7** | 0 |
| burn   | 1        | **0**     | 1 (telegram @sc-19b vs pin sc-11a) | 7 | **7** | 0 |
| audit  | 2        | **0**     | 2 (climber @sc-17r; telegram @sc-11a — both cross-world) | 7 | **7** | 0 |

No withheld conclusion was re-extractable before its reveal pin in any world.
`vocab_shared=7 > 0` — the R510 **F5 vacuous-pass guard** confirms this is a
GENUINE clean pass (shared vocabulary), not a foreign-id "0 matches looks clean"
artifact. The `unordered` rows are the B-1 honesty surface: a fact whose coord
belongs to a *different* world-line is incomparable to this world's pin and is
surfaced, never silently dropped (e.g. the audit-path telegram at sc-19b is not
on the reveal/burn timeline). This is the R502 gate (exposed/withheld + first_at
timing), **deterministic, AI out of the gate** — reproducing R504's manual PIN-2
with the manual mapping step removed.

### PIN-1 render↔world-line fidelity gate (`validate-render-fidelity`) — both branches proven

| input | world | off_path | unplaced | reached_terminal | exit | branch |
|---|---|---|---|---|---|---|
| reveal single-world projection | reveal | **0** | 0 | yes | 0 | **ACCEPT** |
| full multi-world re-extraction | burn | 16 (audit + reveal coords) | 0 | yes | 1 | **REJECT = the R504 audit-as-burn footgun** |
| full multi-world re-extraction | reveal | 15 | 0 | yes | 1 | REJECT (cross-world) |
| full multi-world re-extraction | audit | 17 | 0 | yes | 1 | REJECT (cross-world) |

**0 unplaced everywhere** = the blind extractor used canonical scene ids
faithfully (every coord is a real declaration node), so off-path is true
world-line drift, not noise. The `--world burn` run flags the audit scenes
(sc-17b…sc-20b) off the burn line — the R504 footgun ("a confront-burn file that
delivered a quiet-audit ending") **reproduced and rejected on real blind coords**
(exit 1, loud fail). The single-world projection ACCEPTS cleanly (off_path=0),
exercising the accept branch.

## Verdict

- **PIN-2 (leak): HOLDS** — tool-run, deterministic, non-vacuous (vocab_shared=7),
  all three worlds. Reproduces R504's manual PIN-2.
- **PIN-1 (fidelity): both branches proven** on real blind-re-extracted coords —
  accept (single-world off_path=0) and reject (the audit-as-burn footgun).
- The R507/R508 substrate is validated end-to-end on REAL blind prose. The R505
  manual-surgery caveat is discharged: the human id-mapping step is gone, replaced
  by the shared-vocabulary typed-tuple match (soundness exact; completeness still
  rests on extractor recall — the same R500/R504 boundary, unchanged).

## As-built deviations (faithful)

1. **Negative test is the footgun CONDITION, not the literal v1 file.** R504's
   `W2-confront-burn__A.md` reading copy carries anonymized `## Scene` headers
   (built for human judges) — unusable for canonical-coord fidelity. The footgun
   (audit content checked as the burn world-line) was reproduced via
   `--world burn` on the real blind re-extraction, which is arguably stronger
   (real blind coords vs a hand-made file).
2. **Fidelity accept branch used a single-world projection** of the
   re-extraction (facts whose canon-from ∈ the reveal order). Transparent: the
   full store legitimately holds all worlds, so a coherent single-world input is
   the right accept-branch shape; the reject branch uses the genuine cross-world
   condition.
3. **Plain arm not re-extracted.** The leak gate is disclosure-telling-only; v1's
   plain control (D=1) stands. A symmetric fidelity run on plain is a secondary
   check, deferred with goal (d).
4. **Scratch stores gitignored** (`*.atomic.json`, the neutral prose copy); the
   protocol declaration, the blind brief, and this report are the tracked
   evidence (deterministic verdicts inlined above).
