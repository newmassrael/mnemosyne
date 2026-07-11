# Brief — authoring agent (fresh context; this is your whole task)

You are authoring a small branching narrative game into a Mnemosyne store,
entirely on your own, using a command-line tool and its gates. Your job is to
produce a VALID, COMPLETE, RENDERABLE game store whose HARD WORLD-RULES are
enforced by the tool's continuity gate — using the tool's own contract and gates
to check your work and fix it until it passes.

You have exactly two sources of truth:

1. `mnemosyne-cli describe-schema` — the authoring contract. READ THIS FIRST, in
   full (`--json` for the machine form). It documents the registries, the fact
   shape, the fixed vocabularies, the RULE CLASSES the continuity gate evaluates,
   the quest convention, the write-time invariants, the JSON WIRE FORMAT of the
   fact manifest (with a worked example), and the canon-order requirement.
2. `premise.md` (handed to you) — the SETTING and the required structural shape,
   INCLUDING three hard world-invariants you must make the continuity gate
   enforce.

Do NOT read the mnemosyne source code, tests, or any other file. If you need to
know how something works, ask `describe-schema` or try a command and read its
error. Author from the contract and the gates ONLY.

## Your workspace

Work in the directory you are given (absolute path handed to you). It contains
`mnemosyne.toml` and an empty store at `docs/.atomic/workspace.atomic.json`
(schema 23). All `mnemosyne-cli` commands run there operate on that store.

## The tools you drive (all via `mnemosyne-cli`)

- `describe-schema [--json]` — the contract (read-only).
- `import-sections --manifest <sections.json>` — create the scenes (a JSON array
  of `{ "section_id", "parent_doc", "title" }`). Facts' canon coordinates must
  name existing sections, so import scenes before the facts in them.
- `propose-verdict --manifest <facts.json> [--json]` — the DRY-RUN GATE. Applies
  a candidate facts manifest to a throwaway clone and returns `commit` (exit 0)
  or `rollback` (exit 1) with actionable violations (source, rule, message,
  expected, repair_hint). It ALSO runs the continuity gate — including the
  narrative RULE classes, once you have declared and wired rules. The real store
  is untouched; use it freely.
- `import-facts --manifest <facts.json>` — apply a manifest for real (atomic),
  once `propose-verdict` says `commit`.
- `report-authoring-frontier [--telling <plan-id>] [--json]` — your WORKLIST on
  the real store: what is still incomplete. Read EVERY gap axis and close each.

## The world-rules task (the point of this game)

`premise.md` names THREE hard world-invariants (the single lantern's custody, the
gate's legal state sequence, the pound's fill-time minimum). Your job is not only
to WRITE those into the prose/facts, but to make the substrate's continuity gate
ENFORCE them, so that a later violation would be REJECTED, not silently accepted.

`describe-schema` tells you which rule CLASSES the continuity gate can evaluate
and what each one keys on. Work out, from the contract, (a) how to TYPE your facts
so a rule can key on them, and (b) how to DECLARE and turn ON the rules so the
gate applies them. If the contract does not spell out some step, discover it by
trying a command and reading its error — and RECORD that friction in your log
(what you had to reverse-engineer is important data). Verify a rule is actually
live by checking that `propose-verdict` on a deliberately rule-BREAKING draft is
REJECTED for that rule (then discard the breaking draft) — this is how you know
the gate holds you to the world-logic, not just to reference shape.

## The loop you run (unattended — no one is helping you)

1. Read `describe-schema` (all of it) and `premise.md`.
2. Design the game: scenes, cast (frames), the fork (two branches), the withheld
   secret (a disclosure plan), the quest, the setup->payoff chains — AND the
   three world-rules and how each is typed + declared.
3. Author `sections.json` and `import-sections` it.
4. Author `facts.json` (follow the fact wire format the contract documents), and
   author + wire the narrative RULES so the gate enforces the three invariants.
   `propose-verdict`; on `rollback`, fix the named fact/field/rule and re-run
   until `commit`; then `import-facts`.
5. Confirm each of the three rules actually BITES: a rule-breaking draft is
   rejected for that rule by `propose-verdict` (then discard the draft). Log it.
6. Run `report-authoring-frontier --telling <your-plan-id>`. Close EVERY gap it
   reports — of EVERY kind — authoring facts (or whatever artifact the contract
   says a gap needs) and re-gating until that gap is gone.
7. STOP only when: `propose-verdict` commits, all three world-rules are declared
   + live + bite, and `report-authoring-frontier --telling <your-plan-id>`
   reports NO remaining gaps of any kind — except `never-planned disclosures`,
   which are your intentional withhold choice and may remain.

## What you leave behind (in your workspace)

- `sections.json`, `facts.json` (+ any follow-ups), your narrative-RULES file,
  your canon-order artifact, and the `mnemosyne.toml` as you wired it — leave
  them applied/in place.
- `loop-log.md` — a numbered log of every `propose-verdict` and
  `report-authoring-frontier` call (command + verdict/gaps + exactly what you
  changed and why), AND a clear record of: how you declared + wired each rule,
  every friction/guess/missing-contract-info point (especially anything about the
  rules you had to reverse-engineer), and the rule-BITES-check for each of the
  three rules (the breaking draft you tried + the rejection you got).

## Rules

- Author from the contract and the gates ONLY. No source, no outside files.
- Every repair must respond to a specific gate/contract output.
- Finish to a RENDERABLE working state with all three world-rules live + biting.
- Keep the game small and taut per `premise.md`. Depth over volume.
