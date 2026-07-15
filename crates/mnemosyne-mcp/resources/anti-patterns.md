# Anti-Patterns — Things You MUST NOT Do

If you find yourself recommending or attempting any of these, STOP and
reconsider. Each one represents a category violation, not a stylistic
preference.

## ❌ "The atomic store is dense — let me clean it up"

The atomic store is an **audit-trail genre**. Density is the essence;
it is not written for sequential human reading. Cleanup beyond
*reasonable termination point* (T1+T2 = 0 reject) is a category error.

The "reasonable termination point" is:
- T1 cross-ref orphan reject = 0 (within ledger scope)
- T2 frozen-ledger violations = 0
- Round-trip mandatory dimension preserved = N/N

Beyond those three conditions, further "prose tidying" of the atomic
store is wasted effort and risks breaking the audit invariants.

## ❌ "Let me rewrite Round N's body to be clearer"

Frozen ledger violation. Round N is read-only by contract. If clarity
is missing, append a *new* ChangelogEntry that adds the clarification.

## ❌ "Let me make this readable for first-time readers"

The committed EPUB (spec content), `mnemosyne-cli query`, and external
guides are the human-facing surface. The atomic store is not. If a human
reports "I can't navigate this",
the answer is to **create a separate readable artifact** (e.g.
`STATUS.md`, `DECISIONS.md`, `FAQ.md`), not to mutate the atomic store.

## ❌ "Let me drive T3 warn / T4 info to 0"

T3 warn / T4 info are intentionally non-zero. They are the warning
surface that catches *new* style problems. Driving them to 0 by mass
rewrites destroys their signal. Acceptable carry zone is whatever the
project's audit ratify decides; trying to zero it out is the
anti-pattern.

## ❌ "Let me split the atomic store across multiple files"

Single-file source-of-truth contract. Cross-ref graph fans out
drastically when the store is split across files — every traversal
becomes multi-file. The store is meant to be one JSON.

## ❌ "Let me unify all Round entries to a standard template"

Retroactive template enforcement on existing entries = frozen ledger
violation. Templates may be enforced on *new* entries (a project
decision), but body mutation of existing entries is forbidden.

## ❌ "Let me edit the atomic JSON directly because that's faster"

The mutate API exists because direct JSON edits skip:
- T1 prose cross-ref orphan check
- T2 atomic frozen-ledger check
- the typed-primitive audit receipt

The "fast" path produces an inconsistent state that the next
`validate_workspace` call will surface. Use the typed primitive.

## ❌ "Let me add a new feature to the schema"

Schema extensions are out of scope for a *routine* session: they are their
own ratified round, with a decision entry, not a side-effect of authoring.
That is the whole of this rule.

**It does NOT mean the schema is closed.** This page used to say the store
was "4 entity types, closed-form" — that was already wrong by Round 273 (the
fifth), and the store has grown many record types since (the narrative half
landed from Round 430 on; `schema_version` is well past 20). A consumer read
this line, concluded the narrative substrate did not exist, and rebuilt it
outside the store in Python.

So: do not extend the schema mid-session — **and do not infer from that rule
that what you need is missing.** Call `describe_schema` for what the store
actually holds, today, derived from code.

## ❌ "Let me drop the doc from `workspace.docs` to silence its orphans"

Editing `mnemosyne.toml::workspace.docs` to make a problem disappear
is silence-bypass — the orphans become invisible without the project
auditing why. **Distinct from**: a *ratified* scope correction, which
is the textbook path:

✅ DO: append a Round entry whose `decision_summary` records the scope
   change ("Doc X removed from workspace.docs as <reason>"). Edit
   `mnemosyne.toml::workspace.docs`. Register the now-dangling atomic
   refs in `[[orphan_ledger]]` with `kind = atomic_entry_ref` (for
   ChangelogEntry impact_refs) or `kind = atomic_section_ref` (for
   Section impact_scope), `reason` pointing back at the Round entry.
   Run `validate_workspace`; the dangling refs now appear as
   `ledger=N` carry, not `new=+M` reject (Round 254).

❌ DON'T: silently shrink `workspace.docs` and accept the resulting
   orphans without a Round entry recording why. The orphan ledger
   `reason` field is required precisely to prevent this.

The distinction is whether the audit trail records *why* the scope
shrank. Frozen-ledger preserves Round N's body; orphan-ledger absorbs
Round N's now-dangling refs. Together they cleanly separate "history
record" from "current state".

## Self-check questions

Before any non-trivial action on the atomic store, run through these:

1. Does this work improve AI workflow efficiency, mutate-API safety, or
   query efficiency?
2. Is this work distinct from human readability concerns (which belong
   in separate human-facing artifacts)?
3. Does this work avoid touching frozen-ledger zones (existing
   ChangelogEntry bodies, retroactive template enforcement)?

If any answer is "no" — confirm with the user before proceeding.
