# Report — unattended-loop-experiment/v1 (R594 EXECUTE)

**Question (R592 residue).** The R585–R592 authoring surfaces (`describe-schema`,
`propose-verdict`, `report-authoring-frontier`, the all-primitive `import-facts`
manifest) are built + reviewed + dogfood-grounded, but the unattended
generate→gate→repair→frontier LOOP that consumes them had never been run end to
end. Infra is not proof (R500). This experiment ran it.

**Verdict: `loop_proven` — with a concrete punch-list.** A blind AI agent, given
only `describe-schema` + a premise + the CLI (no source), authored a coherent,
gate-clean small game unattended, self-repairing from the surfaces alone. All
three pins hold. Running the loop end to end also surfaced four honest gaps (the
deliverable) and refuted one gap I had suspected before rendering.

---

## What the loop produced — "The Ban Tower"

16 scenes · 6 frames (`ground-truth` + 5 character frames) · 14 entities · 4
predicates · 28 facts · one fork (`road-descend` off `main` at `sc-08`) · one
disclosure telling (`default-telling`, default-withhold) · one quest
(`q-westdraw`, done on both roads). A relief fire-lookout finds the tower empty;
the withheld truth (the senior lookout is dead at the scree foot; the unconfirmed
smoke was a stranded hiker's real signal fire) is reconstructed on the hold road
and revealed late on the descend road.

## PIN-1 — convergence (HOLDS; re-derived on a fresh rebuild, R500)

Rebuilt fresh from the agent's `sections.json` + `facts.json` (the JSON is the
source of truth, not the agent's store):

- `import-sections` → 16 created; `import-facts` → 6 frames / 1 branch / 14
  entities / 4 predicates / 28 facts / 1 disclosure-plan / 3 overrides created.
- `propose-verdict` re-applies at **commit, 57 no-op, 0 violations** (idempotent).
- `report-authoring-frontier --telling default-telling` → **0 zero-fact scenes,
  0 dangling setups, 0 unresolved quests** (25 never-planned disclosures =
  intended withhold, allowed by the pin).
- `validate-workspace` → T1 orphan 0 / T3 reject 0.

The self-report matched the independent rebuild exactly.

## PIN-2 — the surfaces DROVE it (HOLDS; audit of `loop-log.md`)

From the loop-log, every repair traces to a specific surface output, and the
agent used `describe-schema` + the gates only (it did not read crate source):

- `propose-verdict`: 13 calls — **11 were wire-format discovery probes** (each
  fixing a serialization error the parser named), **2 gated full manifests, both
  `commit` first try, 0 rollbacks** (no continuity/invariant reject ever fired —
  the agent's content was continuity-clean).
- `report-authoring-frontier`: 2 calls — round 1 flagged 3 `main` dangling
  setups; the agent reworked the fork topology (see Finding 2) and round 2 was
  clean on all three axes.

Honest nuance: the surfaces that *did the work* were the `propose-verdict` PARSE
layer + the FRONTIER. The `propose-verdict` CONTINUITY gate (exclusive / interval
/ transition) never fired — this 28-fact game did not stress it, so the loop did
not exercise it. Do not over-claim the continuity gate from this run.

## PIN-3 — coherence (HOLDS)

The orchestrator supplied a canon ORDER (mechanically derived from the agent's
scene ids — see Finding 4), then rendered the descend road blind, re-extracted it
blind, and ran the gates + 3 blind judges.

- **PIN-3a deterministic.** Leak gate: **leaks=0**, `vocabulary_shared=4`
  (non-vacuous — the R510-F5 guard is satisfied, a real pass), `unmatched=2`
  (gt-sella / gt-smoke re-extracted with a slightly different scalar value —
  benign, not a leak). Independently, the blind re-extractor first states Tomas
  dead at `sc-11d` — exactly the fact's `first_at`, so the mystery did not leak.
  Fidelity gate: **off_path=0, unplaced=0, reached_terminal=true**.
- **PIN-3b judges.** 3 blind judges UNANIMOUS **5/5/5/5/4** (coherence /
  knowledge-realism / choice-and-consequence / quest-legibility / overall), all
  "real game? yes". The lone 4 (overall) is craft headroom: the weakest-link
  notes are the `sc-10d` reveal compressing Sella's two-day backstory (2 judges)
  and the `sc-03` torn-page device (1 judge) — render/pacing notes, not
  structural or knowledge breaks.

## Decision (pre-committed rule)

PIN-1 ∧ PIN-2 ∧ PIN-3 all hold ⇒ **`loop_proven`**: the R585–R592 surfaces enable
a blind agent to author a coherent small game unattended. Infra → proof for this
axis, with the caveats below as the next build targets.

---

## The deliverable — the honest next gaps

1. **`describe-schema` documents the semantic contract but NOT the wire format
   (the headline).** It gives field names + types but not the JSON serialization:
   registry key names, `forks_from` (a string) + separate `forks_at`, the
   internally-tagged typed-object enum, `first_at` as `[branch, section]` tuples,
   the override's `fact_id`. The agent reverse-engineered all of it from
   `propose-verdict` parse errors — 11 wasted probe calls. Worse, there is an
   outright INCONSISTENCY: the contract's vocabulary calls the scalar object-kind
   `scalar`, but the wire enum variant is spelled `value`. `describe-schema`'s
   stated purpose (R587: "self-serve instead of reading source") is undercut by
   the omission. **Build target: add the serialization to `describe-schema`
   (ideally emit a JSON example / schema per kind) and fix the scalar/value
   naming.**
2. **The fork-lineage dangling model is under-documented.** A setup is checked on
   the world-line it is *authored* on; a child-branch payoff does not discharge a
   parent (`main`) setup; pre-fork setups are forced onto `main`, so `main` must
   continue past the fork. The agent inferred this only from the frontier's
   `dangling_setups.main` bucketing, and it drove the one real rework. **Build
   target: document it in `describe-schema`.**
3. **The additive frontier-repair loop cannot fix a TOPOLOGY error.** Append-only
   means a fact cannot be re-branched by a later manifest, so a wrong-branch
   structural error needs a full re-author, not an additive `facts-2.json`. The
   agent correctly reset the regenerable store and re-imported a corrected single
   manifest (it never hand-edited the store). **The loop model is additive for
   CONTENT gaps but not for STRUCTURE errors — a real limit worth naming.**
4. **The loop's definition of "done" omits the canon ORDER (the sharpest gap).**
   `report-authoring-frontier` reported 0/0/0 "complete", but the store was NOT
   renderable: `report-playthrough-manuscript` / `report-fork-tree` produced
   nothing until the orchestrator supplied a canon order. The order is a
   first-class authoring artifact (prior experiments' authors wrote `order.json`),
   but `describe-schema` never says one is required and the frontier never checks
   for it. Here it was trivially mechanical (the scene ids encode the sequence),
   so it is a small gap — but a real one. **Build target: `describe-schema`
   should state the order requirement AND `report-authoring-frontier` should flag
   a missing/incomplete canon order as a gap (or the pipeline should derive a
   linear default and let the author override).**

**Refuted before it became a claim (verify-before-claiming):** on first reading
the manuscript I suspected the disclosure was inverted (25 surface facts
`withhold`, 3 secrets `state`). The blind render + the leak gate refuted it: the
config is proper MYSTERY disclosure — the surface events narrate through the POV,
and the three secrets are correctly withheld until their reveal scene (leak=0).
Not a finding.

## Honest scope / caveats

- n = 1 premise / 1 loop agent / 1 render / 3 judges / one render road.
- "Unattended" = no human and no orchestrator help on the LOOP (author + gate +
  repair). The render / extract / judge pipeline is orchestrator-run by firewall
  design (R469), not a loop capability.
- The orchestrator supplied the canon order for PIN-3 (Finding 4). So the loop
  proved unattended authoring of the FACT + DISCLOSURE layers; the ORDER layer it
  did not produce.
- The continuity gate was not stressed (PIN-2 nuance). A game with typed
  possession / interval constraints would exercise it; this one did not.
- The premise wording ("a disclosure plan whose default withholds") nudged the
  default-withhold telling — the agent followed it faithfully and it rendered
  well, but the wording is a confound to note for a cleaner re-run.

## Firewall (R469) — 6 blind subagents

Loop agent (author + gate + repair, unattended) · render · extractor · 3 judges.
This lineage seeded the premise + empty workspace, ran the deterministic
verification + gates, and assembled the outline/briefing — it authored, repaired,
rendered, and judged nothing.
