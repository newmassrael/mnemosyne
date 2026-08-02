# A/B Authoring Experiment — Orchestrator Runbook

ORCHESTRATOR-ONLY material. Never show this file, `ab-manifest.json`, or
anything in this directory to an author / extractor / judge session.

- Protocol SSOT: `ab-manifest.json` (sha256 `30ed5ed6…1f7e` pinned in the
  R470 ledger entry, superseding the R469 pin — both pinned before any
  execution session).
- Design SSOT: design doc sec 7.18.
- Run state: `run-log.md` (this directory).

## Validity floor — hard rules

1. **Session isolation.** S1–S10 sessions are NEVER launched inside
   `/home/coin/mnemosyne` (project memory + CLAUDE.md + this directory all
   leak the design). Launch each session with its working directory set to
   the prepared isolated directory below. Verified 2026-06-11: no
   user-scope MCP server exposes the mnemosyne store to those directories,
   and each isolated directory gets its own empty project memory.
2. **No manifest contact.** Author/extractor/judge prompts were extracted
   verbatim from the manifest into the BRIEF files. Pass nothing else.
3. **No internet.** Deny WebSearch/WebFetch for S1–S10 (the briefs also
   instruct the agents not to use them).
4. **Same model tier** for S1–S6 (both arms). Record the model id in
   `run-log.md` before starting.
5. **No coaching.** If an author drifts from format or misses quota, do NOT
   correct mid-course. A quota failure is itself an economics finding
   (manifest `validity_conditions`): report it, run the deterministic leg
   on what exists, skip the preference leg if scales are incomparable.
6. **Orchestrator never edits deliverables.** Snapshot and carry, only.

## Prepared directories

| Path | Role | Sessions |
|---|---|---|
| `/home/coin/saltglass-novella/` | plain arm (BRIEF + 3 session cards) | S1–S3 |
| `/home/coin/saltglass-studio/` | loop arm (BRIEF + WORKSPACE + substrate pre-wired, gate verified green) | S4–S6 |
| `/home/coin/saltglass-extraction/` | blind re-extraction (BRIEF + story-A/, story-B/ workspaces pre-wired) | S7 |
| `/home/coin/saltglass-judging/` | judges (JUDGE-BRIEF + pairs/) | S8–S10 |

All four are git repos (initialized at prep) — commit after every session
for the audit trail.

## S1–S3: plain authoring (then S4–S6: loop authoring)

Order of S1–S3 vs S4–S6 is irrelevant (no shared context). For each
session N = 1, 2, 3:

1. Open a FRESH Claude session with cwd = the arm directory.
2. Opening prompt, verbatim:
   > Read SESSION-N.md in this directory and follow it.
3. Let it run to completion. No interventions beyond permission grants.
4. After the session ends:
   - record token usage (`/cost`) + wall time + model into `run-log.md`;
   - `git -C <armdir> add -A && git commit -m "session N end"`;
   - loop arm only: copy nothing, fix nothing — `repairs-log.md` is the
     agent's own artifact (prevented-defects source for PRED-3).

## S7: shuffle + blind extraction

1. Coin flip (orchestrator shell):
   `FLIP=$(shuf -i 0-1 -n 1)` — 0: plain→story-A, loop→story-B; 1: reverse.
2. Copy `story.md` from each arm into
   `/home/coin/saltglass-extraction/story-A.md` and `story-B.md` per the
   flip. Strip nothing; copy bytes as-is.
3. Write `label-map.txt` in THIS directory (never in the extraction dir):
   two lines, `story-A = plain|loop`, `story-B = plain|loop`.
4. Immediately: `sha256sum label-map.txt` → record the hash + timestamp in
   `run-log.md` (commit run-log if desired). The map stays sealed until S11.
5. Open a FRESH session, cwd = `/home/coin/saltglass-extraction/`:
   > Read EXTRACTION-BRIEF.md in this directory and follow it.
6. After: record tokens; `git commit` the extraction dir. Outputs expected:
   `defect-tables.md`, `extraction-log.md`, `reading-copies/<world>/copy-{A,B}.md`,
   filled `story-A/` + `story-B/` stores.

## S8–S10: judges (3 fresh sessions, independent)

1. Copy `reading-copies/<world>/` from the extraction dir into
   `/home/coin/saltglass-judging/pairs/<world>/`.
2. For each judge J = 1, 2, 3 (sequentially or in parallel checkouts):
   - FRESH session, cwd = `/home/coin/saltglass-judging/`:
     > Read JUDGE-BRIEF.md in this directory and follow it.
   - After the session: move `verdict.md` OUT to this directory as
     `verdict-J.md` (the next judge must not see it), record tokens,
     `git commit` the judging dir.
3. Optional: owner reads the pairs and writes `verdict-owner.md` (recorded
   separately, never mixed into the 3-judge result).

## S11: reveal round (THIS repo, manifest-knowing session allowed)

1. Unblind `label-map.txt`; verify its sha256 against `run-log.md`.
2. Grade against the manifest:
   - **PRED-1 (the only pin)**: plain story's re-extracted store shows >= 3
     primary-endpoint defects AND loop < plain.
   - **PRED-3**: token ratio loop/plain per delivered scene + prevented-defect
     count from `saltglass-studio/repairs-log.md`.
   - **PRED-4**: judge-cited continuity errors NOT matched by any D-metric
     finding (per story) = untyped residue.
   - PRED-2: no pin — report the preference result as an honest headline.
3. Compile `ab-report.md` (this directory, SSOT per manifest
   `session_map_runbook`), restating the defect table (R452
   self-containment).
4. Ledger entry via `append-changelog-entry` (next free Round), restating
   the result table; honesty bounds from the manifest carried verbatim.

## Token recording

After every session S1–S10: paste the session's token totals (input/output)
and wall time into the `run-log.md` table. PRED-3 needs per-arm totals and
delivered-scene counts; record scene counts at S7 (orchestrator counts
scenes in each story.md — counting is mechanical, not editing).
