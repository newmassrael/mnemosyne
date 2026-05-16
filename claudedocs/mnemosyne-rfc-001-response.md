# Response to pinion RFC 001 — Changelog entry amend / supersede primitive

**Target**: pinion (RFC filer)
**From**: mnemosyne maintainer
**Date**: 2026-05-17
**Disposition**: **Reject as drafted — premise already resolved**

---

## TL;DR

RFC 001 의 problem statement ("changelog 만 append-only — 정정 surface 부재") 가
**현 main HEAD 기준 사실과 다릅니다**. Round 294-301 (가장 최근 7 round) 가
정확히 이 문제를 정통 event-sourcing 패턴으로 해결해 두었습니다 — RFC 의 3
option (A/B/C) 중 어느 것도 채택하지 않는 것을 권장하며, R45 (entry 416)
incident 는 기존 surface 1-call 로 즉시 정정 가능합니다.

---

## 1. RFC premise 검증 — R294-R301 이 이미 제공하는 것

**Schema (R294, schema_version 4)**: `AtomicChangelogEntry` 가 두 평행 layer
로 split.

- **Audit half** — `decision_summary`, `changes_bullets`,
 `verification_bullets`, `impact_refs`, `carry_forward_bullets`. 영구
 frozen. 어떤 primitive 도 post-append 미접촉.
- **Publishable half** — `publishable_decision_summary` + 동명 4 bullet
 list. append 시 audit clone 으로 초기화. R295 bare setter 5개로 mutate.
 `generate-docs` 는 publishable 을 render.

이건 RFC 가 §3 Option B 에서 호소한 "frozen ledger 정신 보존 + audit 손상 0"
을 *schema level 에서* 이미 분리 — author convention 이 아니라 schema
contract 로.

**MCP surface (R295 + R299, `mnemosyne-mcp/src/main.rs:802-869`)**:
- `set_changelog_publishable_decision_summary(entry_id, value)`
- `set_changelog_publishable_changes(entry_id, bullets[])`
- `set_changelog_publishable_verification(entry_id, bullets[])`
- `set_changelog_publishable_impact_refs(entry_id, refs[])`
- `set_changelog_publishable_carry_forward(entry_id, bullets[])`

audit half 미접촉; 어떤 entry 든 가능 (RFC Option A 의 "마지막 entry 만"
race-prone 제약 부재).

**Gate (R296)**: publishable 가 audit 와 다르면 `mnemosyne.toml`
`[[publishable_override_ledger]]` row 필수 — `reason` (mandatory) +
`content_hash` SHA256 anchor. RFC Option B 가 요구한 mandatory `reason` 보다
*엄격* (content_hash 가 정정 후 상태를 봉인).

**Automation (R297)**: `redact_term(pattern, replacement, scope, reason,
applied_in, ...)` — publishable half grep + substitute + ledger draft
자동 생성. dry_run 지원.

**Draft generator (R300)**: `emit_publishable_override_ledger_draft(entry_id)`
— 이미 mutate 한 entry 의 ledger row 후행 생성.

**Drift gate (R301)**: publishable 가 audit 와 다른데 ledger row 없거나
content_hash mismatch 면 `commit` hard reject.

---

## 2. RFC 3 option 대비 비교

| 축 | Option A (amend) | Option B (supersede) | Option C (edit) | **현 main: publishable chain** |
|---|---|---|---|---|
| frozen ledger | mutable last-row | event sourcing 정통 | 모든 entry mutable | audit half schema-level frozen |
| audit 손상 | 마지막 row 가능 | 0 | 임의 가능 | 0 (구조적) |
| 정정 범위 | 마지막 entry | 어떤 entry (새 row) | 어떤 entry (in-place) | 어떤 entry (publishable 만) |
| entry_id 소비 | 0 | +1 / 정정 | 0 | 0 |
| audit reason | mandatory | mandatory | mandatory | mandatory + content_hash anchor |
| race 위험 | 있음 (마지막 모호) | 없음 | 없음 | 없음 |
| GENERATED.md template | 변경 0 | `[SUPERSEDED BY N]` 필요 | 변경 0 | 변경 0 |
| commit↔ledger drift 강제 | 미설계 | 미설계 | 미설계 | R301 hard reject |
| 사용 단계 | 1 | 2 | 1 | 1 (redact_term) or 2 (setter + draft) |

현 main 의 publishable chain 이 RFC 3 option 의 모든 trade-off 축에서
dominate 합니다.

---

## 3. R45 entry 416 — 즉시 정정 경로

본 response 대기 불필요. 둘 중 하나로 즉시 실행 가능합니다.

### 옵션 (a) — redact_term 1-call (권장)

```
redact_term(
    pattern = "§5.16 SceneRenderer 표현",
    replacement = "Round 45 — §5.16 SceneRenderer 표현",
    scope = "decision_summary",
    mode = "literal",
    reason = "R45 round prefix 누락 정정 — 직전 R41-R44 entry prefix 일관성 복원",
    applied_in = "pinion R46+ (mnemosyne RFC 001 self-withdraw)",
    dry_run = false,
)
```

자동 효과:
- entry 416 의 `publishable_decision_summary` ← `"Round 45 — §5.16 ..."`
- `audit_decision_summary` ← 원본 그대로 보존 (R45 시점 record 영구)
- `mnemosyne.toml` `[[publishable_override_ledger]]` row 자동 draft

### 옵션 (b) — bare setter + draft emitter 2-call

```
set_changelog_publishable_decision_summary(
    entry_id="416",
    value="Round 45 — §5.16 SceneRenderer 표현 = ..."
)
emit_publishable_override_ledger_draft(entry_id="416")
# → 반환된 ledger_draft 를 mnemosyne.toml 에 paste
```

수기 control 이 더 필요한 경우.

### 결과적으로 보존되는 것

- entry 416 audit half = R45 원본 영구 frozen, audit 손상 0.
- entry 416 publishable half = R46+ 정정 형식으로 GENERATED.md render.
- override_ledger row = 정정의 audit (언제, 누가, 왜, content_hash 봉인).
- entry_id 추가 소비 0 (RFC Option B 의 단점 자동 해소).

---

## 4. RFC 1.3 "cosmetic 만으로 그치지 않는다" 의 약점

RFC 가 정정 primitive 의 정당화로 든 3 항목:

**(a) "검색 grep 깨짐"** — GENERATED.md 에 grep 으로 round 추출하는 audit
도구 자체가 anti-pattern. round_number 는 atomic store 의 structured field
이므로 query 가 정공법. mnemosyne CLAUDE.md "atomic store changelog_entries
= single source of truth" 정책과 정합.

**(b) "외부 reference 시 GENERATED.md title 로는 round 식별 어려움"** —
entry id 는 atomic store 에 박혀 있고 query 로 즉시 해소. title text 검색에
의존하는 reference 추적은 공식 추적 경로가 아님.

**(c) "체계 신뢰: 모든 row 동일 schema invariant"** — RFC 본인이 인정:
"`decision_summary` format 은 strict schema 가 아닌 author convention".
convention 일탈을 invariant 위반으로 격상하려면 별도 mini-RFC 가 정공법 —
본 RFC scope 아님.

3 항목 중 (c) 만이 별도 논의 가치 있음 (§5 참조).

---

## 5. Convention enforcement 책임 분기

mnemosyne 측 정책 (확정):

- mnemosyne 의 validator 는 **schema invariants** 만 강제 — frozen audit
 half, ledger anchor, cross-ref well-formedness, T1/T3 thresholds.
- **Author conventions** (prefix format, naming style 등) 은 consumer
 책임.

근거 데이터: mnemosyne 자체 changelog 의 audit_decision_summary 48 entry
중 0 개가 `Round N — ` prefix 사용. 즉 prefix convention 은 *mnemosyne-wide*
규칙이 아니라 *pinion-local* 규칙. 이를 mnemosyne schema 로 승격하면 다른
consumer 의 다른 convention 도 줄줄이 schema 진입하는 경로가 열려 정체성
유지 곤란.

### pinion 측 권고 — pre-commit hook

R45-class incident (prefix 누락) 의 차단은 pinion 측 pre-commit hook 으로
trivially 가능:

```bash
#!/usr/bin/env bash
# pinion/.git/hooks/pre-commit (or scripts/pre-append-changelog.sh)
# Reject append-changelog-entry-v2 if decision_summary lacks Round prefix.

set -euo pipefail

# Read the staged decision_summary (caller supplies as argv or env).
summary="${CHANGELOG_DECISION_SUMMARY:-}"
round="${CHANGELOG_ROUND_NUMBER:-}"

if [[ -n "$summary" && -n "$round" ]]; then
 if [[ ! "$summary" =~ ^Round\ ${round}\ —\  ]]; then
 echo "ERROR: decision_summary must start with 'Round ${round} — '" >&2
 echo "       Got: ${summary}" >&2
 exit 1
 fi
fi
```

이런 wrapper 가 pinion 의 append-changelog-entry-v2 호출 직전에 invoke 되면
R45 같은 incident 가 append 시점에 reject. mnemosyne 측 schema 변경 0.

차후 ≥2 consumer 가 동일 convention pattern (e.g. `Round N — `) 을 원한다는
demand 가 관측되면, 그 시점에 mnemosyne 측 configurable knob
(`[changelog_format] prefix_regex`) 으로 escalate 검토 — 현 시점에서는 1
consumer 의 1 incident 가 system-level schema 추가를 정당화하지 않음 (YAGNI).

---

## 6. mnemosyne 측 후속 (참고)

본 response 와 함께 mnemosyne 측에서 진행한 보조 작업:

1. **MCP setter description 보강** (`crates/mnemosyne-mcp/src/main.rs:802-869`)
 — 5개 publishable setter 의 use-case 명시 ("typo fix, prefix-format
 correction, redaction" 등). pinion-급 외부 consumer 의 discovery 격차
 해소.
2. **`docs/RECOVERY_PATTERNS.md` 신설** — typo fix / bulk redaction /
 secret expungement / out-of-sync ledger 의 4 case standard recipe. §6
 에 schema-vs-convention 책임 분기 명시.

위 두 산출물은 본 response 와 같은 main 에 land 됩니다. pinion 측은 R46+
에서 RECOVERY_PATTERNS.md §1 또는 §2 recipe 로 entry 416 정정 진행 권장.

---

## 7. pinion 측 권고 action items

1. **RFC 001 self-withdraw** — premise 해소됨 명시.
2. **entry 416 정정 즉시 실행** — §3 옵션 (a) 또는 (b).
3. **R46 round entry `carry_forward_bullets`** — 다음 항목 1줄:
 `"RFC 001 self-withdraw — mnemosyne R294-R301 publishable chain 으로
 premise 해소, R45 prefix 정정 redact_term 으로 완료"`.
4. **pinion 측 pre-commit hook 신설** (§5) — R45-class incident class
 차단. mnemosyne 측 작업 대기 불필요.

---

## 8. References

- `docs/RECOVERY_PATTERNS.md` — 본 response 와 함께 land 되는 4 case recipe.
- `crates/mnemosyne-validator/src/atomic.rs:144-195` — R294 schema split.
- `crates/mnemosyne-validator/src/mutate.rs:1458-1545` —
 append_changelog_entry_v2 frozen-audit guard.
- `crates/mnemosyne-mcp/src/main.rs:802-912` — R295/R297/R300 MCP surface.
- git log: `593b28c` (R301) → `c063ca3` (R300) → `9920750` (R299) →
 `a8a4429` (R298) → `3ff92f3` (R297).
- Fowler "Event Sourcing"; Kleppmann *DDIA* ch.11 — audit-vs-publishable
 split 이 표현한 CQRS / read-write 분리 패턴.
