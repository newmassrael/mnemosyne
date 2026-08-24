# RULEBOOK — minimal-prompt round operation

Goal: each work session begins by pasting **one bootstrap prompt** into a fresh
session; that session then runs autonomous, commit-sized rounds with no further
prompting until it hits a consent gate. This is the experiment-runbook precedent
("the human types ONE prompt; the orchestrator does the rest" —
`claudedocs/phase1-*/runbook.md`) applied to the whole track.

This file is the **process SSOT**. It does not restate invariants, preferences,
the live position, or past decisions — those live elsewhere (no duplication):

| Source | Owns | Read at |
|---|---|---|
| `CLAUDE.md` (+ `~/.claude` global) | invariants, anti-patterns | auto, every session |
| auto-memory (`MEMORY.md` + files) | preferences + current state + the live `NEXT` | auto, every session |
| atomic-store changelog | decision history | `mnemosyne-cli query --list-changelog` |
| **this file** | round cadence + consent gates + the bootstrap prompt | pointer from `CLAUDE.md` |

If a line here would duplicate one of those, it is wrong — make it a reference.

## THE ONE PROMPT (paste into a fresh session)

This is the whole prompt budget for a normal session. Everything after it is
what that session does on its own.

> You are continuing the Mnemosyne narrative-authoring track. Read this
> `RULEBOOK.md` and the RESUME memory's `NEXT`, then run autonomous,
> commit-sized rounds per the autonomous-rounds doctrine in memory
> (`feedback_north_star_autonomous_rounds`): north-star value order, pay the
> debt this session creates immediately, no half-finished work, YAGNI defers
> speculation, self-pace to ~80% context. Each round follows the per-round
> checklist below and ends as one atomic-store changelog entry + one commit.
> STOP and ask ONLY at a consent gate (push, experiment-execution,
> irreversible/outward, foundation-deletion, genuine scope-fork) or genuine
> ambiguity — never `git push` without an explicit push word, never improvise a
> sha-pinned protocol. Finish at ~80% / a gate by summarizing what landed and
> leaving the RESUME memory with one unambiguous next `NEXT`.

Run `/load` first only if the session needs to re-orient (git state +
`validate-workspace`); the prompt above presumes the auto-loaded memory + this
file. Mid-run the owner can still interject; otherwise no prompting is needed
until a gate.

## Round types (each round = one changelog entry + one commit)

- **DESIGN** (code 0): the design as a self-contained changelog entry (the R452
  self-containment pattern) + the scratch design-doc section. No code.
- **BUILD** (code): implement; build/link errors are the top priority
  (`CLAUDE.md`); then `cargo test` + `cargo clippy -D warnings` + an end-to-end
  smoke; changelog entry; commit.
- **REVIEW**: an honest self-review — find real smells, evidence-cited, never
  sycophantic; record findings; fix them in this round or the next.
- **DEBT**: pay a debt found in review/build in the same session it surfaced. A
  real defect is never deferred as a "separate item."
- **EXPERIMENT** (gated): a blind acceptance test, run via its own
  `claudedocs/phase1-*/runbook.md` bootstrap prompt — separate blind subagents,
  contamination bound (this lineage may not author or judge its own prose, the
  R469 discipline).

## Per-round checklist (reference, do not restate)

- **Before** — citation hygiene: verify every `Round NNN` / `§id` exists before
  writing it (`CLAUDE.md`, the R255 rule).
- **During** — build-error-first; no `vN` version-postfix; no legacy carry;
  SSOT (one resolver per semantic, one home per datum). Run the CLI as
  `scripts/mn` — never `cargo install` into `~/.cargo/bin`, which is a slot
  shared with the consumer checkouts on this machine (Round 823).
- **Verify with the whole population, not the changed one (R1195)** — the ROOT
  suite and `scripts/check-side-workspaces.sh` are what judge a round, whatever
  the change touched. A round that edits only `tools/`, `scripts/` or `.github/`
  still moves what the root workspace's gates read: `locked_resolution_smoke`
  reads every cargo command in this repository's shell scripts, `ci-plan` reads
  the workflows, and both live in the root suite. R1194 edited no `crates/` file,
  verified the workspace it wrote plus the gates its change touched, and the root
  suite it ran anyway is the only thing that caught the defect.
- **AND THE VENUE IS CI, NOT THIS WORKSTATION (R1286, owner's word).** The rule
  above is about the POPULATION and it has not moved; where it runs has. The
  hosted workflow already runs the root suite and every separate workspace on
  every push, so running them again here before each commit is the same work
  twice — and the second copy is the expensive one, because it competes with
  whatever else this machine is building. R1284 measured a commit that could not
  finish eleven times while three sibling repositories held the cores.
  - **Locally**: what the hooks run — they gate the commit and the push, so they
    are not optional — plus the sweeps whose manifests the round touched. A
    sweep's evidence (`*.firings.json`) is TRACKED, so it cannot be produced
    anywhere else; that is the one thing on this list CI cannot take.
    - **And what the hooks run is itself decided by a census, not by taste
      (R1287).** `git_hooks_smoke::every_compiling_gate_a_git_hook_runs_is_one_a_hosted_job_runs`
      asks which of the COMPILING gates a hook makes this workstation pay for a
      hosted job also pays, and it refuses a gate that a hook runs and no runner
      does — either it gets a job, or it goes in `CANNOT_LEAVE_THIS_MACHINE`
      with a reason. So "should this be local?" is answered by running that
      test, and the same run prints the gates already paid in BOTH places,
      which is the list to move next.
  - **In CI**: the population. Read its verdict at the START of the next round,
    in ONE call, never by polling. That is the standing rule already, and
    `ci-state` prints the previous run's answer inside every push.
  - **What it costs, stated**: a red now lands on `main` and is read one round
    later. It is the trade this repository already made when it stopped blocking
    on hosted runs, and a red is paid off as its own round.
  - `scripts/verify.sh` is still how the root suite is run when it IS run — it
    refuses a run that did not cover every target it compiled (R1194) — and
    running it before a commit is a choice a round may make, not a ritual it owes.
- **After** — the changelog entry is self-contained (R452); `validate-workspace`
  clean; commit per `COMMIT_FORMAT.md` (no `Co-Authored-By`; ≤ 72-byte lines;
  1–3 contiguous bullets; English); update the RESUME memory + the topic memory
  + `MEMORY.md`; update the scratch design-doc section.
- **Carry test (R1005)** — before appending, read each carry bullet and answer:
  *could I do this now?* If the bullet is concrete enough to describe the work —
  "a manifest whose last row is invalid", "the shape is a table away", "the type
  is now there for them" — then it is not a limit, it is undone work, and the
  round is not finished. Rounds 993, 996, 1000, 1001 and 1004 each shipped a
  carry that the NEXT round could close by reading it, five times in one
  session; the phrasings that gave it away were "a later round could", "the
  failure is loud so it is not a defect", and a test written out in prose.
  A carry that survives this test says WHAT THE WORLD DOES NOT ALLOW, not what
  was not attempted.
- **Bar** — textbook (cost no object) is the owner standard. A hack, a smell, or
  a silent-fail is a defect, not an acceptable carry.

## Consent gates (STOP, name the gate, await the owner's word)

- **PUSH** — never `git push` (any variant) without an explicit push word in the
  current turn. Autonomous running does NOT authorize it; push waits for the end
  (the autonomous-rounds doctrine). **After a push lands, read the run it
  started** and report the result — green is not the assumption, and a green
  conclusion is not the whole of what the run said: read its annotations too
  (R893). The pre-push
  hook reports the state of the commit you are building ON, which by
  construction cannot include the push you just made; that last one is only
  ever seen by a person looking. Round 888 and Round 889 were both defects that
  a glance at `gh run list` found and nothing else did.
- **EXPERIMENT EXECUTION** — running a blind acceptance experiment. Trigger:
  the owner's `실험` / `experiment` word.
- **IRREVERSIBLE / OUTWARD** — anything published, deleted, or hard to reverse.
- **FOUNDATION DELETION** — removing a server / primitive / module: check
  `ARCHITECTURE.md` §6 (anti-drift invariants) first; "unused by dogfood" is not
  grounds.
- **SCOPE FORK** — a genuine choice not derivable from stated values + memory.
  Ask ONCE, concise, recommendation first (derive, don't over-ask).

## Autonomy contract (between gates)

Run autonomously per the autonomous-rounds doctrine in memory
(`feedback_north_star_autonomous_rounds` — the SSOT for the run discipline:
north-star order, pay-debt-now, no-half-finished, ~80% self-pace, ask/push at
the end). This file does not restate it. Derive choices from stated values +
memory, not from A/B/C menus.

## RESUME contract (the steering wheel)

Every session ends by leaving the RESUME memory
(`project_narrative_authoring_resume.md`) with ONE unambiguous `NEXT`: the round
type + the concrete target + any consent gate it will hit. The bootstrap prompt
executes that `NEXT`. If `NEXT` is a scope fork, phrase it as the single
question to ask. Keeping `NEXT` current is what keeps the prompt budget at one
bootstrap prompt.
