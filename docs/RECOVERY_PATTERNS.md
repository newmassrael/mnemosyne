# Recovery Patterns for Changelog Entries

This guide is for authors who need to **correct an already-appended
ChangelogEntry** — typo, prefix-format inconsistency, redaction of
sensitive content, post-hoc clarification, etc.

The atomic store treats every ChangelogEntry as part of a frozen ledger
(event-sourcing pattern). Direct edits to the JSON file are rejected at
policy level (Round 251+). What *is* supported is the
**audit-vs-publishable split** (R294-R301): every entry stores two
parallel views, and the publishable view is mutable through specific
primitives with mandatory audit (`reason` + `content_hash` anchor).

This document lists the standard recovery recipes. Each is a complete
end-to-end sequence — read the section that matches your situation and
follow it verbatim.

## 0. Mental model — audit vs. publishable half (R294)

Every ChangelogEntry has two parallel views:

- **Audit half** — `decision_summary`, `changes_bullets`,
 `verification_bullets`, `impact_refs`, `carry_forward_bullets`. Frozen
 after first commit. No primitive mutates these post-append. Permanent
 record of what was originally written.
- **Publishable half** — `publishable_decision_summary` and four matching
 `publishable_*` bullet lists. Initialized to a clone of the audit half
 at append time. Mutable through the R295 setters. `mnemosyne-cli query`
 surfaces the publishable half.

Divergence between the two halves is gated by `[[publishable_override_ledger]]`
rows in `mnemosyne.toml` (R296). Each row supplies a mandatory `reason`
and a SHA256 `content_hash` anchor of the publishable state at the time
of approval. R301 hard-rejects commits whose publishable state drifts
from the ledger.

Net effect: you can correct any published-rendering issue without losing
the original record, and every correction is itself an audit entry.

## 1. Recipe — single-entry typo fix

**Symptom**: one published entry contains a typo, broken markdown,
inconsistent capitalization, or a similar narrow defect.

**Recipe** (2 calls):

```bash
# Step 1 — replace publishable_decision_summary (or use a bullet setter
# for the matching bullet-list field).
cargo run -p mnemosyne-cli -- set-changelog-publishable-decision-summary \
 --entry <entry_id> \
 --value "<corrected decision_summary>"

# Step 2 — emit the matching [[publishable_override_ledger]] row.
cargo run -p mnemosyne-cli -- emit-publishable-override-ledger-draft \
 --entry <entry_id>
```

Step 2 prints a TOML block. Paste it into `mnemosyne.toml` under
`[[publishable_override_ledger]]`. Fill in `reason` (mandatory,
human-readable) and `applied_in` (caller-supplied breadcrumb — round
number, PR number, etc.). The `content_hash` is pre-computed against the
post-setter state, so the row clears the R296 gate on the next commit.

**Audit effect**: the audit half is untouched. The publishable half now
carries the corrected form (surfaced via `mnemosyne-cli query`). The
override ledger row is the audit of the correction itself.

## 2. Recipe — bulk format/redaction across multiple entries

**Symptom**: the same pattern (a misspelled term, a leaked internal name,
a prefix-format inconsistency) appears across several entries.

**Recipe** (1 call):

```bash
cargo run -p mnemosyne-cli -- redact-term \
 --pattern "<literal-or-regex>" \
 --replacement "<corrected text>" \
 --scope decision_summary \
 --reason "<why this redaction is happening>" \
 --applied-in "<round / PR / breadcrumb>" \
 --kind redaction \
 [--regex] [--case-insensitive] \
 [--dry-run]
```

Field reference:
- `--scope` — one of `all` | `decision_summary` | `changes_bullets` |
 `verification_bullets` | `impact_refs` | `carry_forward_bullets`.
- `--regex` — interprets `--pattern` as a Rust `regex` crate pattern.
 Default is literal.
- `--case-insensitive` — applies to either mode.
- `--dry-run` — returns the hit set and the ledger drafts without
 mutating. Always run with `--dry-run` first.
- `--kind` — semantic tag for the ledger row (`redaction`, `typo_fix`,
 etc.). Defaults to `redaction`.

`redact-term` walks the publishable half of every ChangelogEntry,
substitutes the pattern, and emits ledger draft rows inline — no separate
`emit-publishable-override-ledger-draft` call required. Paste the rows
into `mnemosyne.toml` exactly as printed; they include `reason`,
`applied_in`, `kind`, and `content_hash`.

**Use this over recipe §1** when the same correction touches three or
more entries, or when you need pattern matching rather than full-field
replacement.

## 3. Recipe — secret leak / PII redaction

**Symptom**: an entry contains text that must be expunged from the
rendered output (credential, customer name, internal identifier).

The audit half is **not** redacted — by design. The original record is
preserved in the atomic store JSON, which lives under your repository
root and is subject to your normal git history controls. If the secret
must also be expunged from git history, that is a separate `git filter-repo`
operation outside Mnemosyne's scope.

For the publishable view (surfaced via `mnemosyne-cli query`):

```bash
cargo run -p mnemosyne-cli -- redact-term \
 --pattern "<exact-secret-or-pattern>" \
 --replacement "[REDACTED]" \
 --scope all \
 --reason "Secret leak — <ticket / incident ref>" \
 --applied-in "<incident-response round>" \
 --kind redaction
```

Then commit. R301's drift gate will accept because `redact-term` emitted
the matching ledger rows.

**Caveat**: the publishable half is a *view*, not encryption. Anyone
with read access to the atomic store JSON can read the original. Treat
this primitive as a rendering-layer redaction, not a security-grade
expungement.

## 4. Recipe — publishable half already mutated, need a ledger row

**Symptom**: a caller mutated the publishable half via the bare setters
(recipe §1 Step 1) but forgot to run Step 2. `commit` now hard-rejects
with a publishable-vs-ledger drift error (R301).

**Recipe** (1 call):

```bash
cargo run -p mnemosyne-cli -- emit-publishable-override-ledger-draft \
 --entry <entry_id>
```

Returns either:
- `{ "in_sync": true, "ledger_draft": null }` — nothing diverged, no
 action needed.
- `{ "in_sync": false, "ledger_draft": "<toml-block>" }` — paste the
 block into `mnemosyne.toml` under `[[publishable_override_ledger]]`,
 fill in `reason` / `applied_in`, and commit.

The `content_hash` in the draft is computed against the **current**
publishable state, so the row clears the gate on the next commit. If you
mutate the publishable half again after generating the draft, re-run
this command.

## 5. What you cannot do (by design)

The following are not supported and policy-rejected:

- **Edit the audit half of any entry, ever.** The audit half is the
 frozen ledger. No primitive mutates it post-append. This is the source
 of the system's audit integrity.
- **Delete a ChangelogEntry.** `append-changelog-entry` enforces
 monotonic `entry_id`; there is no `remove-changelog-entry`. If an
 entry was authored in error, the publishable half can be neutralized
 (e.g. replaced with `"[Withdrawn — see entry N+k]"`) and a successor
 entry can carry the corrected decision.
- **Direct `Edit` / `Write` on the atomic store JSON.** Enforced by
 project policy (Round 126 escape hatch is the only exception, and
 requires explicit user override grant). Edits made this way bypass the
 typed invariants and are caught by `validate-workspace` + the pre-commit
 atomic-sidecar gate.
- **Mutate a publishable field without an accompanying ledger row.**
 R301 hard-rejects on commit. Always pair a setter call with the matching
 ledger row; use `emit-publishable-override-ledger-draft` if you forgot.

## 6. Author-convention enforcement is the consumer's responsibility

Mnemosyne's validator enforces **schema invariants** (frozen audit half,
ledger anchor, cross-ref well-formedness, T1/T3 thresholds), not
**author conventions** (e.g. "every decision_summary must begin with
`Round N — `"). The latter varies by consumer:

- Mnemosyne's own changelog uses a `<topic-keywords> — <body>` pattern
 with no round-number prefix.
- Downstream consumers (e.g. `pinion`) may use a `Round N — §X.Y <topic>`
 prefix.

If your consumer needs to enforce its own prefix or formatting
convention, the recommended path is a **consumer-side pre-commit hook**
(or pre-append wrapper around `append-changelog-entry`) that
validates the `decision_summary` against your project's regex before
the primitive is invoked. This keeps schema-level invariants central
(here) and convention-level invariants local (your repo).

If a convention drifts and produces an inconsistent entry anyway, use
recipe §1 or §2 to correct the publishable half — the audit half stays
as the historical record.

## 7. Cross-references

- `docs/SCHEMA_GUIDE.md` §publishable-override-ledger — gate schema and
 enforcement details.
- `docs/GETTING_STARTED.md` §code-citation-defense — `[plugins.set_equality_validator]` and
 the bidirectional binding model.
- `crates/mnemosyne-atomic/src/lib.rs` — the atomic mutate primitives.
- `crates/mnemosyne-mcp/src/main.rs` — MCP tool descriptions for the
 R295/R297/R300 surface.
