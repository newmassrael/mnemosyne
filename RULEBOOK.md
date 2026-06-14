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
  SSOT (one resolver per semantic, one home per datum).
- **After** — the changelog entry is self-contained (R452); `validate-workspace`
  clean; commit per `COMMIT_FORMAT.md` (no `Co-Authored-By`; ≤ 72-byte lines;
  1–3 contiguous bullets; English); update the RESUME memory + the topic memory
  + `MEMORY.md`; update the scratch design-doc section.
- **Bar** — textbook (cost no object) is the owner standard. A hack, a smell, or
  a silent-fail is a defect, not an acceptable carry.

## Consent gates (STOP, name the gate, await the owner's word)

- **PUSH** — never `git push` (any variant) without an explicit push word in the
  current turn. Autonomous running does NOT authorize it; push waits for the end
  (the autonomous-rounds doctrine).
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
