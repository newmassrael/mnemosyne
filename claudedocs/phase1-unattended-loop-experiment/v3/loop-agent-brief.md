# Brief — authoring agent v3 (fresh context; this is your whole task)

You are authoring a small branching narrative game into a Mnemosyne store,
entirely on your own, using a command-line tool and its gates. Your job is to
produce a VALID, COMPLETE, RENDERABLE game store, using the tool's own contract
and gates to check your work and fix it until it passes.

You have exactly two sources of truth:

1. `mnemosyne-cli describe-schema` — the authoring contract. READ THIS FIRST, in
   full (`--json` for the machine form). It documents the registries, the fact
   shape, the fixed vocabularies, the rule classes, the quest convention, the
   write-time invariants, the JSON WIRE FORMAT of the manifest (with a worked
   example you can copy), the canon-order requirement, and what each report is
   for. It tells you everything about how to shape and serialize the store — and
   it tells you things about what a clean gate does and does not certify. Read
   it as a contract, not as a reference to grep.
2. `premise.md` (handed to you) — the SETTING and required structural shape.

Do NOT read the mnemosyne source code, tests, or any other file. If you need to
know how something works, ask `describe-schema` or try a command and read its
error. Author from the contract and the gates ONLY.

## Your workspace

Work in the directory you are given (absolute path handed to you). It contains
`mnemosyne.toml` and an empty store at `docs/.atomic/workspace.atomic.json`. All
`mnemosyne-cli` commands run there operate on that store. The CLI is invoked as
the absolute path you are handed — do not install it anywhere.

## The tools you drive (all via `mnemosyne-cli`)

- `describe-schema [--json]` — the contract (read-only).
- `import-sections --manifest <sections.json>` — create the scenes (a JSON array
  of `{ "section_id", "parent_doc", "title" }`). Facts' canon coordinates must
  name existing sections, so import scenes before the facts in them.
- `propose-verdict --manifest <facts.json> [--json]` — the DRY-RUN GATE. Applies
  a candidate facts manifest to a throwaway clone and returns `commit` (exit 0)
  or `rollback` (exit 1) with actionable violations (source, rule, message,
  expected, repair_hint). The real store is untouched; use it freely.
- `import-facts --manifest <facts.json>` — apply a manifest for real (atomic),
  once `propose-verdict` says `commit`.
- `report-authoring-frontier [--telling <plan-id>] [--json]` — your WORKLIST. On
  the real store it surfaces what is still incomplete, gap axis by gap axis.
  Read EVERY axis it reports and close each one.
- Anything else the contract tells you exists and says you need. The list above
  is what you are guaranteed, not a fence.

## The loop you run (unattended — no one is helping you)

1. Read `describe-schema` (all of it) and `premise.md`.
2. Design the game: scenes, cast (frames), the fork (two branches), the withheld
   secret (a disclosure plan), the quest, the setup→payoff chains.
3. Author `sections.json` and `import-sections` it.
4. Author `facts.json` (follow the wire format the contract documents — copy its
   worked example's shape). `propose-verdict` it; on `rollback`, fix the named
   fact/field and re-run until `commit`; then `import-facts` it.
5. Run `report-authoring-frontier --telling <your-plan-id>`. Close EVERY gap it
   reports — of EVERY kind, including any it lists that you did not expect —
   authoring facts (or whatever artifact the contract says a gap needs) and
   re-gating until that gap is gone. Re-run the frontier after each fix.
6. STOP when `report-authoring-frontier --telling <your-plan-id>` reports NO
   remaining gaps of any kind — except `never-planned disclosures`, which are
   your intentional withhold choice and may remain — AND when you are satisfied,
   from the contract, that the store is actually finished. If the contract tells
   you a clean report does not certify something you care about, believe it.

## What you leave behind (in your workspace)

- `sections.json`, `facts.json` (+ any follow-ups), and any other artifact the
  contract told you the store needs — leave them applied and in place.
- `loop-log.md` — a numbered log of EVERY gate call: the command, its verdict or
  its gaps, exactly what you changed, and WHY — naming the specific output line
  that made you change it. Where you guessed, say you guessed. Where the
  contract did not tell you something you needed, say what was missing. That log
  is read as carefully as the store.

## Rules

- Author from the contract and the gates ONLY. No source, no outside files.
- Every repair must respond to a specific gate or contract output, and the log
  must name it.
- Finish to a RENDERABLE working state.
- Keep the game small and taut per `premise.md`. Depth over volume.
