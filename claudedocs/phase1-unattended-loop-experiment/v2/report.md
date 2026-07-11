# Report — unattended-loop-experiment/v2 (R598 EXECUTE)

**Question.** Did R595 (describe-schema wire format) + R596 (frontier
canon-order gap) REMOVE the two top frictions v1's unattended loop hit — the 11
wasted wire-format probes and the unrenderable no-order store? A leaner re-run
(no render / judge; coherence was proven at v1): one blind loop agent, the same
"The Ban Tower" premise, the updated surfaces.

**Verdict: `fixes_confirmed`.** Both pins hold, re-derived independently. And —
the point of re-running — the loop surfaced the NEXT layer of gaps.

---

## PIN-v2-1 — wire friction removed (HOLDS; loop-log audit)

The blind agent made **4 propose-verdict calls, ZERO of them wire-format fixes**
(v1 needed **11**). The one non-commit (PV-1) was a CONTENT fix — an
`evidence_unreachable` continuity rollback, not serialization. The agent's
loop-log credits the fix by name: "the contract's worked example + the
`object_kind: scalar` → wire tag `value` note prevented serialization errors."
Non-vacuous: the agent names R595's additions as the reason it never
reverse-engineered the serializer. Finding 1 fix confirmed on a live run.

## PIN-v2-2 — renderable store authored unattended (HOLDS; fresh rebuild)

The agent AUTHORED a canon order on its own — `canon-order.json` (main trunk +
descent, the `call-it` fork edges) pinned via `[continuity].canon_order_path` in
`mnemosyne.toml` — with NO orchestrator help; it discovered the requirement from
`describe-schema`'s canon-order section and the frontier's `unordered scenes`
gap. Re-derived on a FRESH rebuild from the agent's `sections.json` +
`facts.json` + `canon-order.json`: `report-authoring-frontier` = **0 zero-fact /
0 unordered / 0 dangling / 0 unresolved** (34 never-planned = intended withhold);
`validate-workspace` clean. Finding 4 fix confirmed — the surfaces steered the
agent to a RENDERABLE store.

Bonus (deterministic, not a pin): the disclosure is correctly calibrated — the
secret renders `[withhold first_at=sc-14]` on `main` (revealed late) and stays
withheld on `call-it`, with surface facts `[state]`. Better-calibrated than v1's
all-withhold default; the v2 agent set surface facts to `state` and secrets to
`withhold` deliberately.

## Decision (pre-committed): `fixes_confirmed`

PIN-v2-1 ∧ PIN-v2-2 ⇒ R595 + R596 removed both top frictions; the unattended loop
now self-serves the wire format AND authors a renderable store. The infra→proof
arc for the two headline gaps is closed on a live re-run, not just asserted.

---

## The re-run's deliverable — the NEXT layer of gaps

Running the loop again both confirmed the fixes AND found the next friction — the
self-improving pattern. Not downplayed:

1. **The dry-run gate and the worklist check DIFFERENT things, and the
   expensive-to-fix category is not in the cheap gate (the sharpest new gap).**
   `propose-verdict`'s dry run covers shape + reference-integrity + off-branch
   continuity, but NOT payoff/dangling coverage — that is only in
   `report-authoring-frontier`, which runs only on the APPLIED store. So a
   structural flaw (here: with BOTH roads forking off `main`, the bare `main`
   prefix is a dead world-line whose trunk setups dangle) was invisible until
   AFTER `import-facts`, and fixing it meant re-branching already-applied
   append-only facts — a full store reset, with no `init`/`reset` subcommand (the
   agent deleted the gitignored store and re-imported). **Build target:
   `propose-verdict` could run the payoff/dangling analysis in its dry run (it
   already has the throwaway clone), so the loop sees dangling BEFORE it commits.**

2. **`report-authoring-frontier` does not catch disclosure leaks — "frontier
   clean" ≠ "correct telling".** The frontier treats any fact with an override as
   "planned" and moves on; it never checks whether the mode actually withholds on
   the worlds it should. The intuitive `mode: state` leaked the secret on the
   non-reveal road; the agent caught it only by inspecting
   `report-playthrough-manuscript --telling` (a tool outside its brief), then
   re-encoded as `mode: withhold` + a `first_at` reveal pin. **Build target: a
   loop-visible leak signal (fold a leak check into the frontier, or have
   `describe-schema` spell out the withhold+first_at reveal encoding).** This is
   the concrete form of v1's "the loop's worklist omits the render gates" — here
   the agent actually leaked and recovered out-of-band.

3. **v1's Finding 2 RECURRED — it is not "smaller".** Both the v1 and the v2
   agents independently hit the fork-lineage dangling trap (pre-fork setups force
   `main` to continue past the fork; two symmetric forks leave `main` a dead
   prefix). Two independent agents stumbling on the same undocumented model is
   evidence it is a real, recurring friction worth documenting in
   `describe-schema`, not deferring.

4. **Minor:** `completed_by`'s prose says "entity or scalar value" but a
   registered predicate picks ONE `object_kind` (the agent used `scalar` for both
   roads) — a doc-precision note. `report-payoff-coverage` advisories were
   correctly distinguished from frontier gaps (the deliberately-unmarked
   single-world payoff).

## Honest scope / caveats

- n = 1 premise (same as v1, controlling difficulty) / 1 loop agent. No render /
  judge — v2 is a friction-removal confirmation, not a coherence re-proof.
- The probe-count bar (≤ 2) is a threshold on a noisy quantity; the loop-log's
  narrative (the agent naming R595's example as why it never probed) is the real
  evidence.
- The agent used `report-playthrough-manuscript` (outside its brief's tool list)
  to catch the leak — good initiative, but it means the loop's DEFINED stop
  condition (frontier clean) was necessary-not-sufficient, which is finding 2
  above.

## Firewall (R469)

One blind loop agent (author + gate + repair, unattended); this lineage seeded
the workspace + premise and re-derived every pin on a fresh rebuild. No hint
about the wire format or the canon order was given — both came from
`describe-schema`, the point of the fixes.
