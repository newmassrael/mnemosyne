# Response to SCE external-spec compliance ledger RFC

**Target**: SCE (RFC filer)
**From**: mnemosyne maintainer
**Date**: 2026-05-27
**Disposition**: **Accept use case as Phase 1 candidate; FR-1/FR-2/FR-3 deferred to Phase 1.5+; FR-4 partial (이미 inventory 축에 존재); FR-5 reject (intended shape).**

---

## TL;DR

1. **UC-1 등록 수용.** SCE 의 W3C SCXML + IRP catalog mirror 는 narrative
 adapter 와 평행한 Phase 1 후보로 valid. Roadmap 에 추가.
2. **FR-1 (normative_excerpt) / FR-2 (spec_source) 는 지금 land 불가.**
 README §"Closed-form schema in Phase 0" 의 명시적 정책:
 *user-defined kinds, additional entities, schema extensions are
 explicitly not Phase 0 features — that work belongs to Phase 1.5+
 schema decomposition (a separate spec round)*. RFC 가 가장 critical
 로 분류한 두 개가 정확히 이 조항 영역.
3. **FR-3 (symbol-level enforcement) 도 Phase 1+ scope.** Schema mutation
 은 아니지만 (`Implementation.symbol` 이미 존재 —
 `atomic.rs:138-142`), enforcement 활성화 = citation extraction 이
 *language-aware* 가 됨. 현재 cite extractor 는 regex + comment-only
 filter (`code_refs.rs:705`, language-agnostic). LSP / treesitter
 wiring = Phase 0 paradigm 밖.
4. **FR-4 는 부분적 사실 오류.** `extract_inventory_citations(prefixes:
 &[String], ...)` 는 이미 multi-prefix 지원 (`code_refs.rs:549`).
 `extract_citations` 의 단일 `entry_id_prefix` 는 *의도된 단일성* —
 `Round N` 축은 single audit trail 한 줄로 봉인. SCE 의 W3C / IRP
 두 namespace 는 inventory axis 에 매핑하면 현 substrate 로 동작.
5. **FR-5 (multi-workspace) reject.** 단일 `mnemosyne.toml` = single
 audit boundary = Phase 0 단일 source-of-truth 계약. SCE 의 3
 namespace 는 3 디렉토리 × 3 회 호출이 정공법.
6. **Q-3 가 가장 중요한 question.** Spec rev drift 의 frozen ledger
 의미를 SCE 가 잘못 짚을 가능성 — §3.3 답변 참조.
7. **SCE 가 지금 진행할 수 있는 경로는 §6.** Phase 1.5 land 대기 없이
 현 substrate 로 ~80% 기능 confer 가능.

---

## 1. Phase framing — FR-1/FR-2 가 왜 지금 land 안 되는지

README §"Closed-form schema in Phase 0" (lines 377-392) 가 정확히 RFC 의
FR-1/FR-2 형태의 extension 을 다룸:

> The four entity kinds (Section / ChangelogEntry / FrozenList /
> CrossRef) are closed-form. User-defined kinds, additional entities,
> **and schema extensions are explicitly not Phase 0 features — that
> work belongs to Phase 1.5+ schema decomposition (a separate spec
> round)**.

`AtomicSection.normative_excerpt: Option<NormativeExcerpt>` = AtomicSection
스키마에 새 entity-shaped field 추가. `workspace.spec_source` =
`mnemosyne.toml` 의 새 workspace-level entity. 둘 다 Phase 1.5 scope
정의 한가운데.

이 정책의 carry 근거 3 개:

- **Validator 단순성** — plugin loader path 부재 → `cargo test` 가
 closed graph.
- **Round-trip provability** — closed-form 이므로 5-language emit
 (Rust / Kotlin / Python / C++ / Protobuf) 가 feasibility 안에. Open
 schema 진입하면 emit matrix 폭발.
- **Frozen ledger 정합** — schema 자체의 frozen-ness 가 R294 publishable
 chain (entry 안의 frozen audit half) 과 fractal 일치. 임의 field 추가
 = 이 fractal 깨짐.

따라서 FR-1/FR-2 를 Phase 0 에서 land 하면 *세 가지 stable invariant
동시 위반*. CLAUDE.md anti-patterns §"split atomic store across multiple
files" 의 정신과도 충돌 (single store, single schema).

**대신 무엇이 가능한가**: SCE 가 자체 sidecar JSON 으로 normative_excerpt
\+ spec_source 를 carry → mnemosyne validate 통과 → SCE 자체 audit 도구가
sidecar 활용. Phase 1.5 land 시 마이그레이션 부담 발생 (RFC §6 진행안 2
와 동일). **현재 시점 권장 path.**

---

## 2. FR-by-FR disposition

### FR-1 `normative_excerpt` — DEFER (Phase 1.5)

**Status**: 정당한 요구, Phase 0 land 불가, sidecar workaround 권장.

RFC 평가: *"없으면 Mnemosyne 는 정교한 hallucination 감지기 수준이고, 외부
표준 mirror 로는 작동 불가"* — 사실에 가까움. 단, "외부 표준 mirror" 는
Phase 1+ 적용 surface 이므로 Phase 0 에서 작동할 필요 자체가 없음.
Phase 0 의 dogfood 대상은 design-doc lifecycle.

**SCE 측 carry path** (Phase 1.5 land 전까지):

- 옵션 (a) — SCE 별도 sidecar `docs/.atomic/spec_excerpts.json`. 형식:
 `{ section_id: { text, anchor_url, source_revision } }`. mnemosyne
 미관여, SCE 자체 도구가 read. **권장.**
- 옵션 (b) — `AtomicSection.examples: Vec<ExampleBlock>` 에 `language =
 "normative-excerpt-w3c-scxml-2015-09-01"` convention 으로 잠정 동거.
 의미 strained 하지만 schema 미관여로 carry 가능. *비권장* — Phase 1.5
 land 시 migration 더 복잡해짐.

**Phase 1.5 land 시점의 spec 의도**: `normative_excerpt` 는 **frozen
ledger zone** 으로 land (audit half 패턴 차용). 한 번 anchor 되면 setter
없음; rev bump = 새 Section 추가 + 옛 Section `decision_status =
Superseded` 전이. Spec rev drift 가 자연스러운 supersession event 가
됨 (Q-3 답변과 연계).

### FR-2 `spec_source` / `spec_revision` — DEFER (Phase 1.5)

**Status**: FR-1 의 쌍둥이. 동일 처리.

**부가 관찰**: `workspace` 전체에 single `spec_source` 를 묶는 RFC 제안은
SCE 가 단일 spec mirror 라는 가정에 의존. 그러나 SCE 본인이 FR-5 에서
3 namespace (W3C SCXML / IRP / sce-ledger) 를 요구 — 그러면 spec_source
도 namespace 당 1 개여야 함. 이 모순은 SCE 측 design fork 정리 필요
(single mirror vs multi-namespace 어느 쪽이 진짜인지).

Mnemosyne 측 답변: Phase 1.5 land 시 *워크스페이스 당 1 spec_source* 가
자연스러운 shape. Multi-namespace 시나리오 = multi-디렉토리 (= FR-5
reject 와 정합).

### FR-3 symbol-level binding enforcement — DEFER (Phase 1+)

**Status**: Schema mutation 아니지만 enforcement extension 이 Phase 0
paradigm 밖.

**근거**: `Implementation.symbol: Option<String>` 이미 존재
(`atomic.rs:138-142`). 그러나 cite extractor (`code_refs.rs:1043-1180`)
는 regex + comment-only filter 기반의 *language-agnostic* 파이프라인.
Symbol-aware enforcement = 각 citation site 의 enclosing symbol 을
결정해야 함 → LSP / treesitter / language-specific parser 필수.

이는 README 의 "5-language *emit*" (생산만, parse 안 함) 정책 밖.
Phase 0 paradigm 경계.

**RFC 가 옳게 짚은 점**: opt-in flag 로 default v1 유지 → back-compat
안전. 이 spec shape 은 Phase 1+ land 시 그대로 채택 가능.

**현재 시점 SCE workaround**: `Implementation { file, symbol:
Some("Interpreter::process_event") }` 등록은 이미 가능 (atomic.rs:138).
Set-equality 가 file-only 라도 *기록 자체는 보존됨* → SCE 자체 audit
도구가 symbol 필드 query 해서 review 가능. Mnemosyne enforcement 가
늦더라도 SCE 측 도구는 진행 가능.

### FR-4 multi-prefix — PARTIAL ACCEPT

**Status**: Inventory axis 는 이미 지원. Entry_id axis 의 단일성은
*의도된* design.

**사실 정정**: `extract_inventory_citations(prefixes: &[String], ...)`
(`code_refs.rs:549`) 는 이미 `Vec` 받음. Round 275 spec.

**Entry_id 축의 단일 prefix 는 의도**: `[schema].entry_id_prefix` 는
*한 줄의 audit trail* 을 표현. 한 워크스페이스에 두 audit trail 공존 =
단일 source-of-truth 정신 위반. SCE 가 이를 원한다면 = 워크스페이스 분리
(= FR-5 reject 답변 참조).

**SCE 즉시 가능 매핑**:

- `W3C SCXML 3.13` 형식 → `inventory_prefixes = ["W3C SCXML "]` 등록.
 단, 현 `extract_inventory_citations` 는 `[A-Z0-9_]+ ending in digit`
 tail 만 매칭 (line 530-535). `3.13` 의 `.` 는 매칭 밖 → 이 부분이
 진짜 gap.
- 우회: section_id 를 `W3C-SCXML-3.13` 로 정규화 (`§W3C-SCXML-3.13`
 형태 인용). Cite 형식 SCE 측 수정 필요 (30 K cite migration).
- 또는: `extract_inventory_citations` 에 *tail char class* 를
 configurable 로 만드는 작은 hardening round 가능. **이건 Phase 0
 hardening scope 안** (Round 275 의 직계 확장, 새 entity 아님).
 별도 micro-RFC 로 제출 환영.

### FR-5 multi-workspace — REJECT

**Status**: 현재 shape 가 의도된 정공법. RFC 가 명명한 "우회" 가 곧
권장 패턴.

**근거 3 개**:

1. **단일 source-of-truth 계약** — 워크스페이스당 1 `mnemosyne.toml` =
 single audit boundary. R301 commit↔ledger drift hard reject (entry
 차원의 봉인) 와 fractal 일치 — 워크스페이스 차원에서도 단일
 boundary.
2. **Cleanup hard limit policy** (CLAUDE.md) — T3 reject = 0,
 T1 cross-ref orphan = 0, round-trip mandatory N/N. 이 3 invariant
 는 *워크스페이스 단위* 로 정의. Multi-workspace 가 한 파일에 들어가면
 invariant scope 가 모호해짐.
3. **Cascade semantics** — `mnemosyne-cascade` 는 워크스페이스 root 를
 가정 (`config.rs:604+`). Multi-workspace 가 들어가면 cascade
 dependency graph 가 cross-workspace edge 가능 → Salsa scope 폭발.
 Round-trip provability 무너짐.

**SCE 의 권장 shape**:

```
sce-repo/
├── docs/spec/scxml/mnemosyne.toml      # workspace 1
├── docs/spec/irp/mnemosyne.toml        # workspace 2
└── docs/sce-ledger/mnemosyne.toml      # workspace 3
```

`mnemosyne-cli validate-workspace` 3 회 호출. Pre-commit hook 에서 3 회
chain. RFC 가 "동작은 하지만 자연스러움이 없음" 으로 평가한 패턴이
*intended* shape — RFC 평가의 "자연스러움" 은 single-tool-invocation 의
UX 선호이지 architectural requirement 아님.

---

## 3. Q-by-Q answers

### Q-1: External-spec primary namespace 가 의도된 사용인가

**답**: 의도된 사용 *맞음*, 단 의미 분리 확인 필요.

- `external_section_prefixes` (Round 277) / `_bare` (Round 281) =
 **외부 참조 skip** axis. False-positive 회피용. RFC 가 정확히 짚음 —
 이건 "우리 store 의 일부가 아닌 외부 표준 흔적은 무시" 의미.
- SCE 의 사용 = **외부 표준이 우리 store 의 *컨텐츠 자체***. 이건 별도
 axis 가 아니라 *AtomicSection 의 정상 사용* — SCE 가 W3C SCXML §3.13
 을 자체 store 의 Section 으로 *등록* 하면 됨.
- 즉 비대칭 아님: skip-axis 와 primary-namespace 는 *항상* 별도 였음.
 SCE 가 외부 표준을 primary 로 import 하는 것이 architectural 변경이
 아니라 단순 atomic store 채워넣기.

**Gap 은 cite 형식**: `W3C SCXML 3.13` 의 dotted-numeric tail 이 inventory
축에 안 맞고 (FR-4 답변), `§3.13` 로 cite 하려면 SCE 측 30 K cite
migration 필요. 이게 진짜 friction. Tail char class configurable 화
(FR-4 micro-RFC) 로 해소 가능.

### Q-2: parent_section hierarchical ID 깊이 제한

**답**:

- **깊이 hard limit 없음** — `parent_section: Option<String>`
 (atomic.rs:67) 는 단순 self-referencing FK. 검증은 cycle detection
 (parser 측) 만.
- **Cascade 성능** — T1 cross-ref cascade 는 transitive closure 가
 아니라 *직접 cross-ref* 만 검증. Parent chain traversal 비용은 query
 시 O(depth) 선형. 4-5 단계까지 무리 없음.
- **ID 문자열 규칙** — `section_id` 는 `[A-Za-z0-9./-_]`
 (`code_refs.rs:524-526` `is_section_id_char`). `.` 구분자 OK. SCXML
 Appendix D 의 `D.2.selectTransitions` 같은 ID 가능.
- **권장** — SCE 가 `parent_section` 으로 hierarchy 인코딩 하되,
 `section_id` 자체에도 dotted-path 인코딩 (e.g.,
 `D.2.selectTransitions`). 둘 다 채우면 query 양 axis 가능.

### Q-3: External-source section 의 T2 frozen-ledger 의미 [중요]

**답**: SCE 의 framing 에 미스컨셉션 있음. 정정 필요.

- **T2 frozen scope 는 ChangelogEntry sub_bullets 의 audit half**
 (R294 schema_version 4 → `decision_summary` / `changes_bullets` /
 `verification_bullets` / `impact_refs` / `carry_forward_bullets`).
 `AtomicSection` 의 body / examples / rationale 은 T2 frozen 아님 —
 setter primitive 가 존재 (`set_section_intent` 등).
- 따라서 spec rev 가 바뀌어 mirrored section text 가 변경되는 것은
 **T2 위반 아님**. AtomicSection 은 *mutable* 가 default.
- **그러나 spec rev drift 의 audit trail 은 별도 필요** — 이게 SCE 가
 진짜 원하는 것. 권장 패턴:

```
spec rev bump 이벤트 = ChangelogEntry append
  audit_decision_summary: "W3C SCXML §3.13 rev 2026-03-01 → 2026-05-01 — semantic delta on Y"
  audit_changes_bullets: ["§3.13 normative text updated", "downstream impact: ..."]
  audit_impact_refs: ["§3.13", "§3.14", ...]  # 영향 받은 section
```

ChangelogEntry 는 T2 frozen → spec rev drift 의 *audit trail 자체* 는
frozen. AtomicSection 의 *text* 는 mutable (현재 rev 반영). 이 분리가
그대로 정답.

- **추가 안전망** — AtomicSection 이 *현재 rev* 라는 사실은 워크스페이스
 메타데이터 (Phase 1.5 의 FR-2) 가 land 될 때 명시화. 그 전까지는
 `mnemosyne.toml` 의 별도 sidecar 로 SCE 가 carry.
- **`is_mirrored: bool` lifecycle flag 필요한가**: No. AtomicSection 은
 lifecycle 분기 안 함 (closed-form 정신). Spec rev drift 의 lifecycle
 은 ChangelogEntry stream 으로 표현 — 새 entity 추가하지 않고 기존
 entity stream 으로 표현하는 게 Phase 0 패턴.

### Q-4: extract_inventory_citations (R275) 의 schema 확장 timing

**답**: 부분 yes.

- IRP test catalog 의 구조 필드 (datamodel variant / pass-fail /
 manual-vs-auto / expected outcome 등) = AtomicSection 에 표현하기
 어색. SCE 가 sidecar 로 carry 권장 (FR-1 답변과 동일 패턴).
- **Phase 1.5 schema decomposition 의 motivating example 로 valid**.
 Narrative adapter (캐릭터 stat block / 위치 backstory) 와 동일 design
 pressure — 사용자 정의 entity kind 가 필요. 두 use case 가 양방향에서
 같은 schema-decomposition 에 압력 가하는 셈.
- **그 전까지의 carry 패턴** — section_id 에 인코딩 (`test144`) +
 sidecar metadata. 인코딩만으로 (e.g., `test144:ecmascript:auto:pass`)
 처리하지 말 것 — query 불가능, brittle.

---

## 4. UC-1 등록 — Phase 1 parallel axis

**수용**. SCE 의 external-spec compliance ledger use case 를 README
Roadmap §"Phase 1" 에 narrative adapter 와 평행한 두 번째 후보로 등록
(별도 commit).

**의의**: Phase 1.5 schema decomposition design 의 양방향 pressure:

- Narrative adapter → Character / Location / Faction / Scene entity 추가
- External-spec adapter → SpecExcerpt / SpecRevision entity 추가 +
 Workspace 메타데이터

두 axis 가 동시에 디자인 압력 가하면 schema-decomposition 메커니즘이
*진짜로* generic 한지 (= 하나의 use case 만 보고 over-fit 하지 않는지)
검증됨. RFC 가 이 점을 잘 짚었음.

**조건**: Phase 1 priority audit (Round 172) 의 6.00 / 3.00× margin
비교가 narrative-first 이므로 *land 순서는 narrative 가 앞*. SCE
adapter 는 narrative adapter 의 schema-decomposition 메커니즘이 land
된 후 *그 위에* 두 번째 schema 로 추가. 둘이 동시에 paradigm 결정에
영향 가하는 design discussion 단계 (Phase 1.5 spec 라운드) 에는 SCE
측 참여 환영.

---

## 5. FB acknowledgment

- **FB-1 (orphan_ledger CodeCitation 의 의도된 일반화)** — 확인.
 `OrphanKind::CodeCitation` 의 (file, id) ledger 는 *모든* legitimate
 carry case 를 노린 일반 surface (legacy debt + 외부 spec drift carry
 \+ 의도적 historical reference 등). SCHEMA_GUIDE.md 에 SCE-style use
 case 를 example 로 추가 (별도 commit).
- **FB-2 (strip_to_comments line 1:1 보존)** — 확인. Markdown 의 cite
 처리 정책 = "코드 펜스 내부만 strip, 산문 텍스트는 그대로 cite-able".
 `code_refs.rs:705` 의 동작이 이와 정합. SCHEMA_GUIDE 명문화 (별도
 commit).
- **FB-3 (5-language emit 의 멀티런타임 정합)** — 확인. SCE 의
 cross-language binding 시나리오 (같은 §6.5 가 .c + .kt + .rs 에
 동시 binding) 는 *valid Phase 0 사용* — `Section.implementations` 에
 세 Implementation entry 등록하면 set-equality 가 정상 동작. 단,
 file-level binding 의 noise (FR-3 deferral 영향) 는 sidecar 도구로
 SCE 측에서 narrow 권장.

---

## 6. SCE 측 즉시 가능한 진행 경로

Phase 1.5 land 대기 불필요. 현 substrate 로 ~80 % 기능 달성 가능.

**A. Atomic store 채워넣기** (mnemosyne 미관여 path):

- W3C SCXML §3.13 → `AtomicSection { section_id: "scxml-3.13", title:
 "<event> element", parent_doc: "docs/spec/scxml.md", ... }` 등록.
- IRP test144 → `AtomicSection { section_id: "irp-test144", title:
 "...", parent_doc: "docs/spec/irp.md", ... }`.
- Code citation form: `§scxml-3.13`, `§irp-test144` (SCE 측 cite
 migration 필요).
- 또는: FR-4 micro-RFC (inventory tail char class configurable) 가 land
 되면 `W3C SCXML 3.13` 원형 cite 가능.

**B. Sidecar metadata** (Phase 1.5 carry path):

- `sce-repo/docs/spec/scxml/.atomic/normative_excerpts.json` —
 `{ section_id: { text, anchor_url, source_revision } }`.
- `sce-repo/docs/spec/scxml/spec_source.json` — `{ url, revision,
 fetched_sha256, fetched_at }`.
- mnemosyne 은 이 sidecar 인식 안 함 (의도). SCE 자체 audit 도구가
 sidecar + mnemosyne atomic store 함께 query.

**C. Spec rev drift audit trail** (Q-3 답변):

- 각 spec rev bump 시점에 `append_changelog_entry_v2` 호출 → audit half
 에 delta 기록 → publishable half (R295) 로 후일 정정 가능.
- ChangelogEntry stream 이 "이 워크스페이스가 spec rev N → N+1 로
 옮겨갔다" 의 frozen audit trail.

**D. Multi-namespace via multi-디렉토리**:

- 3 워크스페이스 (W3C SCXML / IRP / sce-ledger) = 3 `mnemosyne.toml` =
 3 atomic store. Pre-commit 에서 3 회 validate. FR-5 reject 답변 참조.

**E. Symbol-level audit (SCE 측 도구)**:

- `Implementation { file: "Interpreter.cpp", symbol:
 Some("process_event") }` 등록은 이미 가능 (atomic.rs:138).
- mnemosyne 의 set-equality 는 file-only 검사. SCE 자체 도구가 symbol
 필드 query → 자체 narrow audit 보고서. mnemosyne enforcement 가
 늦더라도 진행 가능.

**F. Mass cite migration**:

- 30,759 W3C citation 의 `W3C SCXML 3.13` → `§scxml-3.13` 변환은 SCE
 측 1 회 sed/awk 작업.
- 또는 FR-4 micro-RFC land 대기 (몇 라인의 hardening 라운드 — 적극
 환영).

---

## 7. Action items

**SCE 측**:

1. §6 A/B/C 즉시 진행 — Phase 1.5 land 대기 불필요.
2. Cite 형식 결정: §-prefix migration 즉시 (SCE 측 30 K 변환) vs FR-4
 micro-RFC 제출 (tail char class configurable).
3. Spec rev drift audit 패턴은 ChangelogEntry stream (Q-3 답변) — 새
 entity 추가 대기 X.
4. Multi-namespace 는 multi-디렉토리 (§6 D). 단일 mnemosyne.toml 에
 묶는 시도 X.
5. FR-1 / FR-2 / FR-3 의 carry 는 sidecar (§6 B / E). Phase 1.5 land
 시 마이그레이션 부담 인지.
6. Phase 1.5 spec discussion 가시화 시점에 SCE 측 design pressure
 입력 환영.

**Mnemosyne 측 (본 response 와 함께 land)**:

1. README Roadmap §"Phase 1" 에 UC-1 (external-spec compliance
 adapter) 를 narrative adapter 와 평행한 후보로 추가.
2. SCHEMA_GUIDE.md 에 external-spec mirror sidecar 패턴 example 추가
 (FB-1 / FB-2 의 명문화).
3. FR-4 micro-RFC 가 제출되면 Round 275 의 직계 확장으로 검토 — Phase 0
 hardening scope 안.
4. Phase 1.5 schema decomposition spec 라운드 진입 시 narrative +
 external-spec 두 use case 의 schema 요구를 *동시에* 검토하여 generic
 mechanism 도출.

---

## 8. References

- `crates/mnemosyne-validator/src/atomic.rs:54-113` — `AtomicSection`
 현재 schema (normative_excerpt 부재 확인).
- `crates/mnemosyne-validator/src/atomic.rs:138-142` — `Implementation
 { file, symbol }` (symbol 이미 존재, enforcement 미적용).
- `crates/mnemosyne-validator/src/code_refs.rs:549-555` —
 `extract_inventory_citations` 이미 `&[String]` multi-prefix.
- `crates/mnemosyne-validator/src/code_refs.rs:1204-1212` — file-only
 set-equality 의 명시적 v1 design choice.
- `crates/mnemosyne-validator/src/code_refs.rs:524-526` —
 `is_section_id_char` (`.` 구분자 허용).
- `crates/mnemosyne-validator/src/config.rs:14-16, 537-555` — workspace
 단일 `mnemosyne.toml` shape.
- README.md:377-392 — Closed-form schema in Phase 0 policy.
- README.md:441-486 — Phase 1 narrative adapter precedent.
- CLAUDE.md "Anti-patterns" §"split atomic store across multiple files"
 — single store / schema 계약.
- `claudedocs/mnemosyne-rfc-001-response.md` — RFC 의 disposition 형식
 precedent.
