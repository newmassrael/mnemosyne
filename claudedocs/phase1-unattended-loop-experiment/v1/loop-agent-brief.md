# Brief — authoring agent (fresh context; this is your whole task)

You are authoring a small branching narrative game into a Mnemosyne store. You
have a command-line tool, `mnemosyne-cli`, and a fresh, empty game workspace.
Your job is to produce a VALID, COMPLETE game store, entirely on your own, using
the tool's own gates to check your work and fix it until it passes.

You have exactly two sources of truth:

1. `mnemosyne-cli describe-schema` — the authoring contract (registries, fact
   shape, fixed vocabularies, rule classes, the quest convention, the write-time
   invariants). READ THIS FIRST, in full (`--json` for the machine form). It
   tells you everything about how to shape the store.
2. `premise.md` (handed to you) — the SETTING and the required structural shape
   of the game.

Do NOT read the mnemosyne source code, tests, or any other file. If you need to
know how something works, ask `describe-schema` or try it and read the error.
The whole point is to author from the contract, not from the internals.

## Your workspace

Work in the directory you are given. It already contains `mnemosyne.toml` and an
empty store at `docs/.atomic/workspace.atomic.json` (schema 23). All `mnemosyne-cli`
commands run from this directory operate on that store.

## The tools you drive (all via `mnemosyne-cli`)

- `describe-schema [--json]` — the contract (read-only).
- `import-sections --manifest <sections.json>` — create the scenes. Sections are
  a JSON ARRAY of `{ "section_id", "parent_doc", "title" }`. Canon coordinates in
  facts (`canon_from` / `canon_to` / `evidence`) must name sections that already
  exist, so author and import your scenes BEFORE the facts that sit in them.
- `propose-verdict --manifest <facts.json> [--json]` — the DRY-RUN GATE. It
  applies your candidate facts manifest to a throwaway clone, runs every
  write-time invariant and the continuity gate, and tells you `commit` (exit 0,
  it WOULD apply cleanly) or `rollback` (exit 1, it would be rejected) with a
  list of actionable violations: each has a `source`, a `rule`, a `message`
  naming the offending fact/field, an `expected`, and a `repair_hint`. THE REAL
  STORE IS NOT TOUCHED. Use this as many times as you need — it is free.
- `import-facts --manifest <facts.json>` — apply a manifest FOR REAL (atomic).
  Only run this once `propose-verdict` on the same manifest says `commit`.
- `report-authoring-frontier [--telling <plan-id>] [--json]` — your WORKLIST. On
  the real store it surfaces what is still incomplete: `zero-fact scenes` (a
  scene with no fact set in it), `dangling setups` per world-line (a fact you
  marked `expected` whose payoff is not yet visible on that world), and — with
  `--telling` — `unresolved quests` and `never-planned disclosures`. This is how
  you find the next thing to author.

The facts manifest shape is a JSON object with `frames` / `branches` /
`entities` / `predicates` / `facts` / `disclosure_plans` arrays. `describe-schema`
documents every field. Registries (frames/branches/entities/predicates) must be
declared before the facts that reference them; disclosure overrides reference
facts, so they apply last — a single manifest may carry all of them and they are
applied in that order in one atomic transaction.

## The loop you run (unattended — no one is helping you)

1. Read `describe-schema` and `premise.md` fully.
2. Design the game on paper: the scenes, the cast (frames), the fork (two
   branches), the withheld secret (a disclosure plan), the quest, the
   setup→payoff chains — everything `premise.md` requires.
3. Author `sections.json` and `import-sections` it.
4. Author `facts.json` (the whole game, or a first batch). Run
   `propose-verdict --manifest facts.json`. If it says `rollback`, read each
   violation, FIX the named fact/field in `facts.json`, and re-run
   `propose-verdict`. Repeat until it says `commit`.
5. `import-facts --manifest facts.json` to apply.
6. Run `report-authoring-frontier --telling <your-plan-id>`. For every gap it
   lists — zero-fact scenes, dangling setups, unresolved quests — author the
   facts that close it into a NEW manifest, `propose-verdict` it (repairing until
   `commit`), and `import-facts` it. Re-run the frontier.
7. Stop when `report-authoring-frontier --telling <your-plan-id>` reports **0
   zero-fact scenes, 0 dangling setups, and 0 unresolved quests**.
   (`never-planned disclosures` are your CHOICE — a withheld secret is
   intentionally never "planned" to be told; leave those as you intend.)

## What you leave behind (in your workspace)

- `sections.json` — the scene skeleton you imported.
- `facts.json` (and any follow-up `facts-2.json`, … you used to close gaps).
- The final store is at `docs/.atomic/workspace.atomic.json` — leave it applied.
- `loop-log.md` — a running log, THE MOST IMPORTANT ARTIFACT. For every single
  `propose-verdict` and `report-authoring-frontier` call, record: the command,
  its verdict/gaps (paste the key lines), and — when it was not clean — exactly
  what you changed in response and why. Number your iterations. Also note any
  moment you were unsure what the contract meant, wanted information
  `describe-schema` did not give you, or had to guess. Be honest about friction;
  do not tidy it away.

## Rules

- Author from the contract and the gates ONLY. No source, no outside files.
- Every repair must be a response to a specific gate output — do not change
  things the gates did not flag.
- Finish to a working state: the frontier clean per step 7, the store applied.
- Keep the game small and taut per `premise.md`. Depth over volume.
