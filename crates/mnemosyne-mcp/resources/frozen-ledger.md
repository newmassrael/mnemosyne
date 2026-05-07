# Frozen Ledger Semantics

## What "frozen" means

Once a `ChangelogEntry` is committed, its body is immutable. New
information about the same entry must arrive as a *new* changelog entry
that references the original — never as a rewrite.

## Why

Audit trails lose value the moment historical entries can be edited. If
"Round 47" can be silently rewritten in place, the project's decision
history becomes unreliable. Every Mnemosyne deployment treats existing
ChangelogEntry bodies as read-only by contract.

## Mechanical enforcement — T2 frozen-ledger jaccard rule

The validator runs T2 on every doc mutation:

```
jaccard(prev.sub_bullets, curr.sub_bullets) >= threshold
```

- `prev` = ChangelogEntry sub_bullets at transaction T1.
- `curr` = sub_bullets at T2 (where T1 < T2).
- Asymmetric form: `prev.sub_bullets ⊆ curr.sub_bullets` is allowed
  (append-only), removal or modification is rejected.

For the atomic store entries (Round 161+ ratify), the same invariant
extends to all 5 ChangelogEntry atomic fields.

## What you should NEVER do

These are hard violations, not stylistic preferences:

❌ "Round 47 was confusing, let me rewrite that paragraph."
   → T2 reject. Append a new ChangelogEntry that supersedes it.

❌ "This decision was wrong; let me fix Round 12's body."
   → T2 reject. Append a new entry recording the reversal.

❌ "I can clean up the punctuation in this old entry without changing
   meaning."
   → T2 reject. Punctuation differences break jaccard inclusion.

❌ Splitting an old entry into multiple entries.
   → T2 reject. Bullet structure is part of the frozen content.

## What you CAN do

✅ Append new bullets to a recent entry (within the same logical session).
✅ Author entirely new ChangelogEntry records.
✅ Mutate `Section` atomic fields — sections are not frozen, only
   ChangelogEntries.
✅ Add CrossRefs between existing sections.

## Strong-carry sections

Some `Section` records have `decision_status = "Active"` but are
practically frozen because their text is widely cited and edits would
ripple. These are flagged as *strong-carry*. Style rules
(`max_paragraph_length`, etc.) skip them. Body edits are technically
allowed by T2 but should be rare; prefer adding a new section that
supersedes if substantive change is needed.

## Override path

Frozen-ledger violations CANNOT be bypassed via tool flags. If the user
genuinely needs to edit a frozen entry (e.g. fixing a privacy leak),
they must perform the edit themselves outside the Mnemosyne API and
explicitly accept the audit gap.
