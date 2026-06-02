# Commit Message Format Guide (mnemosyne)

## Structure

```
<type>(<scope>): <subject>

- <detail 1>
- <detail 2>
- <detail 3>
```

## Rules

### 1. Subject Line
- Format: `<type>(<scope>): <subject>` (scope is optional but strongly preferred)
- Types: `feat`, `fix`, `refactor`, `test`, `docs`, `build`, `chore`
- Subject: Clear and concise description of the change
- No period at the end
- Max 72 characters

### 2. Scope

Spec documents (current dominant phase — pre-impl):
- `concepts`, `design`, `arch`, `roadmap`, `vision`, `prior-art`

Domain (impl phase, post Phase -1B):
- `schema` (§39 graph_schema), `rule` (§41 datalog_rule)
- `branch` (§1, §51), `agent` (§44), `medium` (§50)
- `tier` (§51), `variant` (§46), `persona` (§57)
- `cascade` (§47, §65), `release-lock` (§54)
- `audit` (audit CF), `protocol` (§60a), `spoiler` (§53)

Harness / tooling:
- `conformance` (§39 harness), `codegen` (schema → rule), `fixture`

Build / infra:
- `build`, `ci`, `chore`

### 3. Body
- One blank line after subject
- Bullet points (`- ` prefix) only — no prose lead paragraph
- Bullets must be **contiguous** — no blank line between bullets
- **1–3 items** — focus on key changes (fewer is better)
- The `commit-msg` hook enforces bullet-only + contiguity + the 1–3
  cap (a prose body line, a blank line between bullets, or a 4th
  bullet is rejected, not just discouraged)
- **One bullet = one line, max 72 bytes total (incl. `- ` prefix)**
  - No continuation / indented wrap lines. If a bullet does not fit in
    72 bytes, rewrite it tighter or split into a separate bullet.
  - Verify with: `git log -1 --format=%B | awk '{print length, $0}'`
- Be specific and reference spec anchors

Reference conventions:
- DESIGN section: `§39`, `§44`, `§60a`
- CONCEPTS cell ID: `cell #1`, `cells #1–6`
- Axis enum: `MetaAgentScope=branch_local`, `VariantMediumScope=all_media`
- ROADMAP phase: `Phase -1A`, `Phase 0`, `Phase 4A`
- Decision source slug: `phase-0-cell-1-analysis`

### 4. Style
- **English only** — subject and body must be written in English so the
  log stays accessible to every collaborator. ASCII printable (U+0020
  to U+007E) plus the whitelist of typographic symbols below are the
  only permitted code points; any character outside this set (Hangul,
  Kana, CJK ideographs, Cyrillic, Greek, etc.) is rejected by the
  commit-msg hook.
  - Typographic whitelist: `§` (U+00A7), `–` (en-dash U+2013), `—`
    (em-dash U+2014), `•` (bullet U+2022), `…` (ellipsis U+2026), `→`
    (rightwards arrow U+2192). These are the only non-ASCII code
    points the hook lets through.
  - Round summaries / progress notes that need Korean phrasing belong
    in `claudedocs/`, auto-memory under `memory/`, or atomic-store
    publishable fields, never in the commit message.
  - Per Round 251 the atomic store ledger uses English; commit
    messages stay English-first by the same rationale.
- **No emojis** (Unicode pictograph ranges U+1F300-U+1FAFF and
  U+1F1E6-U+1F1FF are rejected; the typographic symbols above are
  explicitly allowed)
- **No "Generated with Claude Code"**
- **No "Co-Authored-By" tags**
- Professional and technical tone
- Focus on "what" and "why", not "how"
- Quantify progress when possible (e.g., `entries 52 → 53`, `T3 warn 2 → 3`)

## Type Guidelines

| Type | When to Use | Examples |
|------|-------------|----------|
| `feat` | New axis, new schema kind, new rule, new HTTP/RPC surface, new harness category | Add §60a protocol verifier, Emit datalog_rule from §39 enum |
| `fix` | Spec inconsistency, codegen bug, semantics violation, ordering bug | Freeze transaction-time per tier on lock acquire |
| `refactor` | Spec reorganization without semantic change, code restructuring | Extract pairwise matrix from CONCEPTS §6 to §6.1 |
| `test` | Conformance fixtures, property tests, regression tests | Add VariantMediumScope axis fixtures |
| `docs` | Spec body update, design decision recording, README/explainer | Finalize 6 axis defaults, Add §60a Specification |
| `build` | Codegen pipeline, harness scaffold, dependency, CI | Scaffold §39 conformance harness skeleton |
| `chore` | Repo hygiene, gitignore, tooling config | Add .gitignore for codegen output dir |

## Examples

### Good: Multi-cell decision recording (docs)
```
docs(concepts): finalize 6 axis defaults (cells #1-6)

- Decide MetaAgentScope=branch_local, CascadeOrdering=global_fifo, ExternalAiProtocol=v1 (others per cell-N analysis)
- Register all 6 axes in DESIGN §39 enum block; remove "unspecified" markers from §6 pairwise table
- Unblock ROADMAP Phase 0 / 2 / 4A entry prerequisites
```

### Good: New spec section (docs)
```
docs(design): add §60a External AI Protocol Specification

- Define metadata schema, 7-step verification, audit fields, forgery model
- Resolve cell #6 (ExternalAiProtocol=v1) — Phase 0 mnemosyne-server prerequisite
```

### Good: Inter-section consistency fix (fix)
```
fix(concepts): align §6 pairwise table with §6.2 cell decisions

- Replace "unspecified" in branch×agent and medium×variant cells with §44 / §46 references
- Cross-link MetaAgentScope and VariantMediumScope axis enums
```

### Good: Conformance fixture addition (test)
```
test(conformance): add §39 axis fixtures for VariantMediumScope

- 1 positive fixture (all_media default round-trip via codegen)
- 1 negative fixture (axis omission → runtime panic, no silent default per CONCEPTS §6 rule #2)
- Cross-axis: VariantMediumScope × relation_mode × TierReleaseLockScope
```

### Good: Cross-axis interaction harness (test)
```
test(conformance): add inter-kind dependency fixture for §39 → §41

- Schema change triggers datalog_rule re-codegen end-to-end
- Reject downstream type mismatch at codegen boundary, not at runtime
- Cover Witcher novel/game cross-medium scenario
```

### Good: Codegen feature (feat)
```
feat(codegen): emit datalog_rule from graph_schema axis enum

- Wire §39 → §41 inter-kind dependency for all 6 axis variants
- Reject type mismatch at codegen, no runtime fallback
- Pass §39 conformance harness (12 positive + 6 negative fixtures)
```

### Good: Semantics bug fix (fix)
```
fix(release-lock): freeze transaction-time per tier on §54 lock acquire

- Capture last_seq at acquire, not release (TierReleaseLockScope=transaction_time_freeze)
- Restore continuity with §64 backup last_seq cursor on restore
- Resolve cross-branch race with §44 meta agent batch update
```

### Good: Spec restructure (refactor)
```
refactor(concepts): split §6 into pairwise matrix and cell decisions

- Move 5×5 interaction matrix to §6.1, cell decisions to §6.2
- Cross-link each cell to its DESIGN body section (§44 / §46 / §47 / §54 / §60a)
```

### Good: Concise (1-2 items when sufficient)
```
docs(roadmap): mark Phase 0 prerequisites as resolved

- Cells #1, #6 decided per phase-0 analysis slugs
- Remaining gate: Phase -1A storage spike + Phase -1B P0 codegen pass
```

### Bad: Too many details
```
docs(concepts): update §6

- Update cell #1
- Update cell #2
- Update cell #3
- Update cell #4
- Update cell #5
- Update cell #6
- Update pairwise table
- Update axis enum
```
**Problem**: 8 items — should be condensed to 2–3 with quantification ("6/6 cells decided") and outcomes ("unblock Phase 0 / 2 / 4A").

### Bad: Too vague
```
feat: Add new feature

- Implement handler
- Update spec
- Fix issues
```
**Problem**: Which axis? Which §? Which phase prerequisite? No spec anchor.

## Common Mistakes to Avoid

### Bad
```
feat: Add cool new axis enum! 🎯

- Add MetaAgentScope axis 🚀
- Update §39 ✨

🤖 Generated with Claude Code

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
```
**Problems**: Emojis, attribution tags, exclamation marks, no scope, no decision source.

### Good
```
feat(schema): register MetaAgentScope axis in §39 enum

- Add branch_local / player_level variants per cell #1 analysis
- Wire §44 meta agent body to consume axis at codegen
```

## Domain-Specific Guidelines

### Spec edits (`concepts`, `design`, `arch`, `roadmap`)
- Always cite the section being changed (`§6`, `§39`, `§60a`)
- For axis decisions, name the axis enum and chosen default
- For cell decisions, cite `cell #N` and the `phase-N-cell-N-analysis` slug
- Note phase entry impact when relevant

**Example**:
```
docs(design): add §44 MetaAgentScope semantics body

- Specify branch_local default: per-branch NG+ tree, no cross-branch carry
- Cite cell #1 decision (phase-0-cell-1-analysis) and §39 axis enum entry
- Cross-reference §47 cascade ordering for meta agent batch update
```

### Conformance harness (`conformance`, `fixture`)
- Quantify fixtures: "6 positive + 6 negative" or "12 axis × candidate × cross-axis"
- Cite the CONCEPTS §6 rule being enforced (silent-default rejection, axis-omission panic)
- For inter-kind dependencies, name both kinds (`§39 → §41`)

**Example**:
```
test(conformance): add cross-axis fixture for MultiPersonaExposureMode

- Cover MultiPersonaExposureMode × VariantMediumScope (cross-medium spoiler shape)
- Negative case: axis omission must panic per CONCEPTS §6 rule #2
- 1 positive + 1 negative; total harness count 14 → 16
```

### Codegen / impl (`codegen`, `schema`, `rule`)
- Reference both the source spec section (`§39`) and the codegen target (`§41 datalog_rule`)
- For determinism guarantees, cite §45 deterministic gate
- Note rejection at codegen vs runtime (codegen-boundary rejection is preferred per spec)

**Example**:
```
feat(codegen): reject §39 axis type mismatch at codegen boundary

- Validate axis enum against §41 datalog_rule consumer signatures
- Fail-fast at compile, not at query runtime (no silent fallback)
- Pass full §39 conformance harness for all 6 axes
```

### Protocol / verifier (`protocol`, `audit`)
- Reference §60a verification step number when relevant ("step 4: HMAC verify")
- Include cryptographic primitive when introducing one (HMAC-SHA256, deterministic registry)
- Note audit CF mapping for forgery-model coverage

**Example**:
```
feat(protocol): add §60a HMAC verifier reference impl

- Implement deterministic 7-step verification per §60a
- Map verification failures to audit CF (created_by integrity field)
- Reject unregistered actor keys at step 2 (template registry lookup)
```

### Storage / measurement (`build`, `chore`)
- For Phase -1A storage spike work, cite which axis-storage assumption is measured
- Quantify with concrete numbers (rows, bytes, query latency p50/p99)
- Link result back to the decision it informs (e.g., `transaction_time_freeze` viability)

**Example**:
```
build(harness): scaffold Phase -1A storage measurement spike

- Bench bi-temporal CoW overlay at 10⁶ branch × 10⁴ canon-time grid
- Target metric: ancestor walk p99 < 5ms for §54 release_lock check
- Result feeds TierReleaseLockScope viability for Phase 4A
```

## Quantification Guidelines

Always quantify when possible:

- **Cell progress**: "6/6 cells decided", "cells #1, #6 resolved"
- **Axis coverage**: "all 6 axes registered in §39 enum"
- **Fixture count**: "12 positive + 6 negative", "+2 fixtures (14 → 16)"
- **Phase gates**: "unblock Phase 0", "Phase -1B P0 prerequisite"
- **Section diffs**: "+§60a / +§6.1", "§44 body completed"

**Key Points**:
- 1–3 items (use fewer when sufficient)
- No emojis, no attribution tags
- Specific § sections, axis enums, cell IDs, phase slugs
- Quantify cell / axis / fixture / phase-gate progress
- Distinguish spec edits (`docs`) from harness/codegen (`feat`/`test`/`build`)
