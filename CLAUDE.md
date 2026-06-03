# Mnemosyne — AI Agent Workflow Guide

This file is auto-read by Claude Code at every session start. When working
on the Mnemosyne project, this instruction takes precedence.

## Phase 0's *real* goal

**Make AI agents (Claude / future LLM) read + mutate spec efficiently.**

- ≠ human readability
- ≠ writing tutorials for newcomers
- ≠ making spec more *concise*
- ≠ "first-time readers can understand"

The atomic store = *audit trail + AI domain* (the single directly-validated
SSOT post Round 400; GENERATED.md and the markdown-doc model were removed —
humans read the EPUB for spec content + `mnemosyne-cli query` for the
changelog). Density is the essence; it is *not* written for human newcomers.

Human-facing surfaces = `GETTING_STARTED.md` / `SCHEMA_GUIDE.md` (separate
artifacts, already exist).

## Anti-patterns — *never recommend / proceed*

The following are *all* Phase 0 framing violations. Self-check before any
recommendation.

### ❌ "atomic store is dense, let's clean it up"
- atomic store = audit trail genre. Density is the essence.
- AI reads via DB query; humans don't read it from start to finish.
- *Reasonable termination point* = T3 reject = 0 + cross-ref consistency.
 Beyond that is an anti-pattern.

### ❌ "make it readable for first-time readers"
- That's `GETTING_STARTED.md` / `SCHEMA_GUIDE.md` territory; already exist.
- Not the purpose of the atomic store.

### ❌ "rewrite the body of a Round N entry to be shorter"
- Frozen ledger violation.
- Audit trail information loss.
- Round 19 frozen ledger principle carries stable.

### ❌ "split this paragraph because it looks long" (ignoring semantic preservation)
- LLMs can parse dense paragraphs fine.
- Splitting may damage *semantic layers*.

### ❌ "drive T3 warn / T4 info to 0"
- Round 138 tier mobility ratify carry: T3 reject = 0 enforced; T3 warn /
 T4 info = acceptable carry zone (author review discretion).
- Trying to drive to 0 = anti-pattern.

### ❌ "split atomic store across multiple files"
- Cross-ref graph fans out drastically.
- Single source-of-truth contract carries.

### ❌ "unify all Round entries to a standard template" (frozen zone)
- Retroactive change to existing entries = frozen ledger violation.
- Enforcing a template on *new* entries OK (Round 162 schema decomposition
 scope), but body mutation of existing entries = 0.

### ❌ "add a `_v2` / `_v3` postfix on a function/struct when extending"
- API postfix versioning is forbidden in this codebase. Extending a
 function signature, struct, or enum: *modify the existing definition
 in place and update all callers in the same change*. Pre-release
 means no external compat to preserve.
- **The ban covers EVERY `vN` version-postfix identifier (`_v2`, `_v3`,
 `_v4`, `…`, snake `foo_v2`, camel `FooV2`), in ALL code — not just
 production API. Test fixtures, test function names, test data labels,
 local variables, modules: none may carry a `vN` version postfix.**
 The right name describes *what differs* (`section_alt` / `mutated_entry`
 / `bare_external_case`), never *which iteration* (`section_v2`). A
 pre-commit gate (`.githooks/pre-commit` Gate 6) scans staged `.rs`
 added lines for the `[A-Za-z0-9]_v[0-9]` / `[a-z0-9]V[0-9]` patterns and
 rejects the commit; do not bypass it with `--no-verify`.
- NOT banned (these are real version *numbers* in data, not identifier
 postfixes): `schema_version` / `CURRENT_SCHEMA_VERSION` (store schema
 generation), upstream spec revision strings, RFC numbers. The gate's
 `_v[0-9]` pattern deliberately does not match `_version`.
- The legacy `_v2`/`_v3` wrappers that existed in `code_refs.rs` were
 cleaned up in the same change that introduced this rule — keep the
 cleanup, don't recreate the pattern.
- Round NNN annotations in code comments (e.g., `// Round 275 — …`)
 are *audit-trail anchors*, not version postfixes — those are
 acceptable when the annotation cites an actual atomic-store entry.
 Inventing a fresh "Round NNN" label for the current change is *not*:
 the round entry must already exist (via `append_changelog_entry`)
 before the citation lands, per the citation hygiene rule.

### ❌ "keep the legacy path alive as a carry"
- When a primitive / module / config knob is superseded, *remove it
 in the same change* — function definition, tests, helpers, CLI
 dispatch, lib.rs re-exports, MCP resources. Pre-release no-compat
 means there is no external API to preserve; half-cleanup leaves
 dead code that future agents will be tempted to reanimate.
- Specific carries that **were** removed under this rule (do not
 recreate): the markdown surgical-insert `mutate::append_changelog_entry`
 (pre-Round 162 path, superseded by atomic-store
 `atomic::append_changelog_entry`), its CLI subcommand, its
 `tests/append_changelog_entry_smoke.rs` smoke test, and the
 `parse_append_changelog_args` / `parse_body_file` / `AppendChangelogArgs`
 helpers that supported it.
- If a "legacy carry" justification appears in a comment (`legacy v1
 path`, `pre-R162 carry`, `kept for backward compat`, `superseded but
 retained`, …), that comment is itself an instruction to *delete the
 carry now*, not to preserve it. Audit history lives in the atomic
 store changelog; code lives in code.

### ❌ "field 에 두 개의 write path 두면서 invariant 만 다르게"
- 같은 atomic field 에 작성 권한 있는 primitive 가 둘 이상이면, *모두*
 같은 invariant set 을 강제해야 한다. 더 엄격하게 만들고 싶으면 둘 다
 tighten, 더 느슨하게 두고 싶으면 둘 다 loosen. **half-enforced
 invariant = no invariant + silent broken state** — 한쪽 path 로 들어온
 데이터가 다른 쪽 path 의 invariant 를 어기는 순간 시스템 전체가 invariant
 없이 동작하는 것과 같다.
- R295 가 publishable setter 신규 시 section setter (R161 §41 facts-as-
 one-liner policy) 의 `check_intent_len` / `check_bullet_len` 를 paste
 했다 — `append_changelog_entry` 측 cap 0 인데 setter 만 cap 200. R294
 가 906-char publishable_decision_summary 로 append 됐고 (cap 0 통과)
 R305 redact 시도가 setter cap 200 으로 reject 당하면서 발견. paste-
 error 가 이 anti-pattern 의 canonical case.
- 신규 setter 추가 시 *field-invariant parity test* (multi-write-path
 field 마다 같은 edge-case input 으로 양쪽 호출해 양쪽 다 accept 또는
 양쪽 다 reject 인지 assert) 를 같이 land. R305 가 atomic.rs 에 추가한
 parity test 가 substrate — 새 setter 가 paste-error 를 가져오면 CI 가
 catch.

## ✅ Correct patterns — recommend path

- Improve AI query efficiency (e.g. indexed cache, faster lookup, multi-hop graph)
- Strengthen mutate API safety (e.g. T2 frozen ledger automation, atomic
 field validation)
- Cross-ref graph traversal efficiency (1-hop / multi-hop)
- Enforce frozen ledger consistency on the audit trail
- Add atomic fields to the DB schema (for semantic preservation enforcement)
- Remove hardcoding from production code (config-driven, external user path)
- Create new external-user-facing artifacts (`GETTING_STARTED.md` /
 `SCHEMA_GUIDE.md` etc., as *separate files*, not body mutation)
- Create human-facing dashboards (`STATUS.md` / `DECISIONS.md` etc., as
 *separate artifacts*, auto-generated via DB query)

## Cleanup hard limit

Reasonable termination point for cleanup (store-direct, post Round 400):
- T3 reject = 0
- T1 prose cross-ref orphan = 0 (outside the known-stale ledger)
- atomic orphan refs (entry / section) = 0 (outside the known-stale ledger)

Passing these conditions = cleanup complete. Further *prose tidying*
attempts are anti-patterns.

## Self-check questions (run before any task)

If any of the following is *not* yes — anti-pattern suspected, *confirm
with user before proceeding*:

1. Does this work improve AI workflow efficiency / mutate API safety /
 query efficiency?
2. Is this work *separate* from human readability concerns?
3. Does this work *not touch* the frozen ledger zone (existing Round N
 entry bodies)?

If any answer is no — *confirm with user before starting*. Cleanup-loop
recurrence is a real risk zone.

## Decision flow — when *human readable* gap surfaces

If a human reports they cannot access information:
- → Create a *separate artifact* (`STATUS.md` / `DECISIONS.md` /
 `CHANGELOG_SUMMARY.md` / `FAQ.md` / `TUTORIAL.md` etc.)
- → *Not* body mutation of existing atomic store entries
- → Prefer DB-query auto-generation when possible (cascade auto-update
 Stage 4 alignment)

## Progress history location

The atomic store changelog_entries = single source of truth. GENERATED.md
and the markdown-doc model were removed in Round 400 (the store is the only
directly-validated artifact). On entering a new conversation, read the
changelog via `mnemosyne-cli query` first (Round 127 dogfood proof carries).

## Mutate API enforcement (Round 127 carry)

All spec mutation routes through `mnemosyne-cli` mutate API:
- `append-changelog-entry` (add a Round N entry, atomic)
- `set-section-intent` / `set-section-rationale` / etc. (atomic Section
 primitives)
- `add-section` / `add-cross-ref` (legacy markdown surgical insert,
 pre-251)
- `set-section-decision-status` (Active → Superseded)
- `set-section-body` (legacy markdown body update, T2 frozen ledger gate)

Direct `Edit` / `Write` on the atomic store JSON or generated artifacts =
0 calls enforced. Exception: explicit user *override grant* (Round 126
option (iii) escape hatch).

## Citation hygiene (Round 255 — Stage 1 of code-citation-verification)

Before writing `Round NNN` or `§<id>` references in code / comments /
commit messages, *verify the target exists in the atomic store*. LLM
hallucination of round numbers is silent corruption of the audit trail —
no compiler catches it, and `git blame` chases the wrong rationale.

**Verification path** (existing MCP tools, no new primitives needed):

1. Call `list_sections` once at session start → cache the section_id set.
2. For each cited `Round NNN`, prefix-match `round-NNN--` in the cached
 set:
 - 0 matches = hallucinated. Do NOT write the citation. Find the
 actually-relevant round, or stop and ask the user.
 - ≥ 1 match = exists. Proceed.
3. For decision_status (Active vs Superseded), call
 `query_section(section_id=<full-slug>)` and read `decision_status`.
 Only cite Active entries; Superseded entries require explicit "this is
 a historical reference to a superseded decision" framing.

CLI equivalents (when MCP unavailable): `mnemosyne-cli query
--list-sections` for step 1, `mnemosyne-cli query §<full-section-id>` for
step 3.

**Why**: atomic store is the single source of truth for round-numbered
decisions. Citations that name a non-existent or superseded round break
audit-trail correctness silently. Catching it at the *agent's writing
moment* is dramatically cheaper than catching it later (pre-commit / CI
/ post-merge decay scan).

**Out of scope** (carry forward, future rounds):

- Pre-commit gate that rejects missing/superseded citations (Stage 2)
- Cascade trigger that surfaces decay when an entry transitions to
 Superseded (Stage 3)
- Semantic match check ("Round NNN actually decides *this* code") —
 T3/T4 heuristic territory, not v1
- Dedicated `verify_round_citation(n)` MCP tool — add only if the
 two-call dance shows real friction in practice

## Git hook installation (R306+ — tracked `.githooks/`)

This repo ships its git hooks under `.githooks/` (tracked, source of
truth). The directory contains three hooks:

- `pre-commit` — atomic-sidecar gate, code-citation defense,
 workspace validate (when a doc is staged), clippy (when `.rs` is
 staged).
- `commit-msg` — enforces `COMMIT_FORMAT.md`: subject ≤ 72 bytes,
 body line ≤ 72 bytes, 1–3 bullets, no continuation lines, English
 + typographic whitelist (`§ – — • … →`).
- `pre-push` — re-runs `validate-workspace` + clippy before
 publishing.

Install (one-time per clone):

```bash
git config core.hooksPath .githooks
```

The legacy `scripts/install-hooks.sh` + `scripts/hooks/` copy-based
flow was retired in R306+ (no more sync step; `.githooks/` is the
direct hook directory). Any local `.git/hooks/pre-commit` /
`commit-msg` left over from the copy era is automatically ignored
once `core.hooksPath` is set, and can be deleted.
