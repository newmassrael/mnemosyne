# Mnemosyne

> LLM 기반 프로젝트를 위한 디자인 문서 라이프사이클 인프라.
> [English README](README.md)

마크다운 디자인 문서는 AI 에이전트가 직접 편집하는 순간 위험해진다. 정규식
하나가 불릿 구조를 무너뜨리고, 헤딩 이름 변경 한 번이 200개의 cross-ref를
조용히 깨뜨리며, "개선" 한 줄이 frozen ledger 항목을 다시 써서 히스토리를
잃는다.

Mnemosyne은 그 취약한 표면을 **atomic-store + GENERATED.md** 구조로 대체한다:

- atomic store (`docs/.atomic/workspace.atomic.json`) 가 source of truth —
  타입을 가진 레코드 (Section / ChangelogEntry / FrozenList / CrossRef) 와
  append-only 감사 의미.
- `docs/GENERATED.md` 는 사람이 읽는 유일한 산출물. atomic store에서
  결정론적으로 렌더링됨.
- 모든 mutation은 typed primitive를 통해서만 가능. 저장 전에 T1
  (cross-ref orphan reject) + T2 (frozen-ledger jaccard) 를 강제 통과.

**상태:** Phase 0 production stack (6 crates). 59 테스트 그린. `main`
브랜치 1 커밋 — Phase 0 동안 squash 히스토리 의도적 유지.

## 구성요소

| 크레이트 | 역할 |
|---|---|
| `mnemosyne-validator` | 파서 / 에미터 / T1+T2 / round-trip |
| `mnemosyne-store` | RocksDB CF 레이아웃 |
| `mnemosyne-core` | 타입드 fact 브리지 |
| `mnemosyne-cascade` | Salsa cascade query |
| `mnemosyne-server` | gRPC + 감사 append 표면 |
| `mnemosyne-cli` | 프로덕션 CLI (validate / mutate / generate-docs) |
| `mnemosyne-mcp` | AI 클라이언트용 Model Context Protocol 서버 |

## 빠른 시작 (CLI)

```bash
git clone https://github.com/newmassrael/mnemosyne
cd mnemosyne
cargo install --path crates/mnemosyne-cli --force
cargo install --path crates/mnemosyne-mcp --force
```

자기 프로젝트 루트에 `mnemosyne.toml` 작성:

```toml
[workspace]
docs = ["ARCHITECTURE.md", "docs/spec.md"]
default_doc = "ARCHITECTURE.md"

[schema]
changelog_titles = ["Changelog"]
entry_id_prefix = "Round "

[style]
locale = "en"
```

그 다음:

```bash
mnemosyne-cli validate-workspace
```

이 한 줄이 baseline을 뽑는다 — T1 orphan 총합, round-trip mandatory 상태,
T3/T4 스타일 위반. 이 baseline을 기준으로 이후 mutation은 *증분으로*
평가된다.

상세 가이드는 [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) 와
[docs/SCHEMA_GUIDE.md](docs/SCHEMA_GUIDE.md) 참조.

## AI 에이전트와 함께 쓰기 (MCP)

`mnemosyne-mcp` 는 Model Context Protocol 서버. AI 클라이언트 (Claude Code,
Cursor, Cline, Continue, Copilot Chat 등) 가 stdio로 연결하면 다음을 얻는다:

- **15개 typed tool** — validate / query / 9개 atomic mutate primitive.
  각 tool의 인자는 validator 도달 전에 JSONSchema로 검증됨.
- **7개 개념 리소스** — `mnemosyne://concepts/*` URI로 노출. overview /
  atomic-store / frozen-ledger / tier-rules / anti-patterns /
  schema-guide / workflow. AI 클라이언트가 자동 로드하므로 에이전트가
  mutation 전에 Mnemosyne 의미를 내재화한다.

### 프로젝트에 MCP 서버 등록

프로젝트 루트에 `.mcp.json` 작성:

```json
{
  "mcpServers": {
    "mnemosyne": {
      "command": "mnemosyne-mcp",
      "args": ["--workspace", "."]
    }
  }
}
```

AI 클라이언트 재시작. 첫 호출 시 서버 승인 프롬프트가 뜨고, 승인 후에는
별도 셋업 없이 에이전트가 tool 호출과 개념 리소스 읽기를 수행할 수 있다.

### 협업자 온보딩 흐름

이미 `.mcp.json` + `mnemosyne.toml` 이 있는 프로젝트를 동료가 clone 받을 때
필요한 것은 한 번의 설치뿐:

```bash
cargo install --path /path/to/mnemosyne/crates/mnemosyne-cli --force
cargo install --path /path/to/mnemosyne/crates/mnemosyne-mcp --force
```

다음에 AI 클라이언트가 그 프로젝트를 열 때 `.mcp.json` 을 자동으로
인식한다. `cargo-dist` 기반 prebuilt binary 배포는 추후 릴리스 예정.

## 작동 원리

라이프사이클은 네 노드로 이루어진다:

```
typed mutate primitive ──► atomic store JSON ──► tera 렌더 ──► GENERATED.md
        │                                                          │
        └──────── round-trip: parse(emit) == typed_facts ──────────┘
```

전형적인 mutation 흐름:

1. 작성자나 AI 가 typed primitive 호출 (예: `set_section_intent`).
2. Primitive 가 쓰기 전에 T1 (cross-ref orphan reject) + T2 (frozen
   ledger jaccard) 검증.
3. 통과 시 atomic store JSON 을 temp 파일 + atomic rename 으로 쓴다.
4. Cascade 자동 갱신: tera 템플릿이 store 를 `docs/GENERATED.md` 로
   다시 렌더.
5. Round-trip 불변식 — `parse(emit(typed_facts)) == typed_facts` —
   이후 매 `validate-workspace` 호출에서 다시 검증.

읽기 경로는 파싱을 건너뛴다 — `query-section` 은 atomic store 에서
SectionView JSON 을 바로 반환.

CLI / MCP 서버 / pre-commit hook — 어디서 호출하든 동일한 코드 경로
(`mnemosyne-validator` 의 parse + emit + T1 + T2) 를 거친다. 한 구현,
세 진입 표면.

## CI 통합

CI에서는 MCP가 필요 없다 — CLI만 있으면 충분:

```yaml
# .github/workflows/mnemosyne.yml
on: [push, pull_request]
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install --git https://github.com/newmassrael/mnemosyne mnemosyne-cli
      - run: mnemosyne-cli validate-workspace
      - run: mnemosyne-cli verify-generated
```

## 설계 고찰

주요 설계 결정과 검토했던 대안들. 자기 의견이 있는 프로젝트에 Mnemosyne
를 도입할 때 참고용.

### 왜 atomic store + GENERATED.md 인가, raw markdown 이 아니라

순수 마크다운 표면은 AI 에이전트에게 세 가지 구조적 실패 모드를 노출한다:

- `§3` 을 고치려는 정규식이 코드 펜스 안에서 잘못 매치된다.
- 헤딩 이름 변경이 200개의 cross-ref 를 조용히 무효화한다.
- "개선" 한 줄이 frozen ledger 항목을 다시 써서 히스토리를 잃는다.

Typed atomic store 는 이 셋을 모두 mechanical reject 로 변환한다:

- T1 — 존재하지 않는 `§N` 타겟이 쓰기 시점에 reject 된다.
- 헤딩 이름 변경은 `set_section_*` 를 통해 모든 cross_ref 를 원자적으로
  갱신한다.
- T2 — sub_bullet 삭제는 jaccard inclusion 으로 reject 된다.

### 왜 단일 JSON 파일인가, 데이터베이스가 아니라

검토 대상: RocksDB, sled, LMDB, XTDB, Datomic. Phase -1A 측정 스파이크
(`bench/` 워크스페이스) 에서 RocksDB CF + 24 B 고정폭 composite key 가
per-fact 레이어의 §3 SLA 예산을 만족함을 확인했다.

그러나 **workspace-scope** atomic store (Section + ChangelogEntry typed
facts) 에서는 풀-DB 오버헤드가 얻는 게 없다 — 워크스페이스는 작고, 접근
패턴은 "전체 파일 로드 → 한 번 mutate → 재렌더" 일 뿐이다. 단일 JSON 을
temp + atomic rename 으로 쓰면 충분하다.

RocksDB 는 Phase 0 에서도 **audit-trail 레이어**용으로 살아있다 —
`mnemosyne-cli commit` 이 디자인 문서 커밋 트랜잭션을 `.mnemosyne/store/`
아래 RocksDB column family 에 기록한다. §4 10-CF 스키마를 50K-asset
워크로드에서 본격 활용하는 per-branch fact 레이어는 Phase 1+ 스코프다.
일상적으로 쓰는 validate / mutate / render 경로는 JSON 파일만 건드리고,
RocksDB 는 `commit` 때 깨어난다.

### 왜 frozen ledger 인가, git 히스토리가 아니라

Git 은 *파일* 변경을 추적한다. Frozen ledger 는 *결정* 변경을 추적한다.
같지 않다:

- 파일 이름 변경이 그 안의 결정 히스토리를 잃는다.
- Squash-merge 가 개별 결정 커밋을 합쳐 버린다.
- Cherry-pick 이 결정 순서를 임의로 재배치한다.

ChangelogEntry 시퀀스는 `entry_id` monotonicity 로 정렬되며, 매 mutation
마다 다시 검증된다. Audit 용도에서 git 보다 강한 보장이다.

### 왜 typed primitive 인가, LSP 식 텍스트 편집이 아니라

LSP 편집은 텍스트 범위 단위로 동작한다. Mnemosyne primitive 는 typed
field 단위다. 한 논리적 변경이 여러 영역을 건드릴 때 차이가 드러난다:

- LSP "§39 → §40 rename": 작성자가 정규식을 쓰고 정확성을 기도한다.
- Mnemosyne `set_section_impact_scope(target=§40)`: validator 가 §40
  존재를 확인하고, 모든 cross_ref 를 원자적으로 갱신하고, GENERATED.md
  를 다시 렌더한다.

비용: mutation 은 typed API 를 통과해야 한다. 효익: "정규식이 엉뚱한
곳을 잡았다" 류의 버그가 구조적으로 0 이다.

### 왜 AI 통합 표면에 MCP 인가

검토 대상: custom JSON-RPC, gRPC, vendor-specific extension, 평범한
CLI 호출. MCP 채택 사유:

- 벤더 횡단 표준 (Claude Code, Cursor, Cline, Continue, Copilot Chat
  모두 지원).
- Tool 인자가 프로토콜 레이어에서 JSONSchema 로 검증된다.
- Resource 표면이 개념 문서를 에이전트 컨텍스트에 자동 로드하므로,
  에이전트가 mutate 전에 규칙을 학습한다.

`mnemosyne-mcp` 서버는 프로덕션 CLI 를 래핑한다. Validation 로직은
단일 source 로 유지된다.

### 왜 cascade query 에 Salsa 인가

검토 대상: Differential Dataflow, Adapton, 수동 invalidation. Salsa
채택 사유:

- Field-level 의존성 추적 (Round 92 fine-grained 레이어).
- Byte-equal memoization 안정성 — 프로세스 간에도 동일.
- `#[salsa::input/tracked/db]` 컴파일 타임 통합으로 cascade 정의가
  query body 옆에 머문다.

Phase 1.5 cascade-gate full-scale 측정 (50K asset 워크로드) 이
per-record 패턴이 §11 SLA 예산까지 스케일하는지 검증할 예정이다.

### 왜 round-trip 등식이 시스템의 척추인가

계약: `parse(emit(typed_facts)) == typed_facts`.

이 등식이 무너지면 atomic store 와 `GENERATED.md` 가 drift 하고,
pre-commit hook 이 결국 잘못 분류한다. Round 67 의 sub-section prefix
버그는 정확히 이 방식으로 잡혔다 — 파서가 nested numbered heading 의
section_id 를 `60/1` 로 만들었는데, 에미터가 bare `1.` 만 출력해서
재파싱 시 다른 id 가 나오면서 diff 가 깨졌다. 마지막 segment 에 부모
prefix 를 보존하는 것으로 수정. 손으로 쓰는 테스트가 좀처럼 못 잡는
종류의 위생 문제를 mechanical 하게 잡는 장치다.

### 닫힌 형 스키마 (Phase 0)

네 entity kind (Section / ChangelogEntry / FrozenList / CrossRef) 는
closed-form 이다. 사용자 정의 kind, 추가 entity, 스키마 확장은
명시적으로 Phase 0 의 *비*기능 — Phase 1.5+ 스키마 decomposition (별도
spec round) 의 작업이다.

Phase 0 에서 스키마를 닫음으로써:

- Validator 가 단순해진다 (플러그인 로더 경로 없음).
- Round-trip 의 증명 가능성이 유지된다.
- 5-language emit (Rust + Kotlin + Python + C++ + Protobuf) 이 처리
  가능해진다. Salsa cascade 의미는 Rust-only 로 남는다 — 점진적 계산
  보장을 다른 언어로 옮기는 것은 out of paradigm 으로 판단했다.

## 문서

- [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) — 5분 셋업 가이드.
- [docs/SCHEMA_GUIDE.md](docs/SCHEMA_GUIDE.md) — `mnemosyne.toml` 모든
  필드 + preset.
- [docs/GENERATED.md](docs/GENERATED.md) — atomic store에서 생성됨.
  Mnemosyne 자체 디자인 문서 dogfood.
- [CLAUDE.md](CLAUDE.md) — Mnemosyne *자체*를 작업할 때의 Claude Code
  가이드.
- [COMMIT_FORMAT.md](COMMIT_FORMAT.md) — 커밋 메시지 규약.

이미 MCP 세션 안에 들어온 AI 에이전트의 canonical 온보딩 순서:

1. `mnemosyne://concepts/overview`
2. `mnemosyne://concepts/anti-patterns`
3. `mnemosyne://concepts/atomic-store`
4. `mnemosyne://concepts/frozen-ledger`
5. `mnemosyne://concepts/tier-rules`
6. `mnemosyne://concepts/workflow`

## 로드맵

Mnemosyne은 Phase 0 — 디자인 문서 라이프사이클 단계다. 장기적으로는 동일한
atomic-store + frozen-ledger 보장을 다른 마크다운 형태 매체로 확장하는 방향:

- **Phase 1 (deferred): 서사 매체 어댑터.** 픽션 / 창작 영역 확장 — 게임
  스크립트, 캐릭터 바이블, 월드빌딩 로그 — 동일한 AI 변형 안전성 계약
  아래에서. 우선순위 감사에서 Phase 1 첫 진입 대상으로 채택. 현재 legacy
  markdown migration carry 완주 이전까지 deferred.
- **Phase 1.5: cascade-gate full-scale 측정.** Per-record Salsa cascade
  패턴이 §11 50K-asset 워크로드에서 발표된 p95 예산을 만족하는지 검증.

위 항목은 *registered carry* — 약속이 아니다. Phase 0 stack 안정화가 선결
조건이며, 코드베이스는 "현재 작동하는 것" 과 "audit ledger에 이름이
등록된 것" 을 구분해서 기술한다.

## 라이선스

MIT 또는 Apache-2.0 (선택).
