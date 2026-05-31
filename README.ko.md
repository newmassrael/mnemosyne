# Mnemosyne

> AI가 편집하는 마크다운의 무결성 인프라 — spec, 코드 인용, 그리고 (장기적으로) 서사 매체.
> [English README](README.md)

AI 에이전트가 마크다운을 직접 편집할 때 컴파일러가 잡아내지 못하는 세
가지 실패 모드가 생긴다:

- `§3` 을 고치려는 정규식이 코드 펜스 안에서 잘못 매치되어 무관한
  예시를 망가뜨린다.
- 헤딩 이름 변경 한 번이 다른 문서들에 흩어진 200개의 cross-ref를
  조용히 무효화한다.
- "개선" 한 줄이 frozen ledger 항목을 다시 써서, *왜* 시스템이 이런
  모양인지 설명하던 결정 히스토리가 사라진다.

이 위험은 *코드베이스가 spec을 인용하는 순간 바깥으로 확장된다*. `//
Round 254 의 근거 참조` 같은 주석은 load-bearing 문서다 — Round 254 가
이름이 바뀌거나, 삭제되거나, Superseded 되는 순간 그 주석은 거짓이 되고
`git blame` 은 영원히 잘못된 근거를 추적한다. 서사 문서도 마찬가지다 —
캐릭터 바이블에서 2장의 눈 색깔 메모가 15장과 모순되는 것은 같은 종류의
무결성 파괴, 단지 매체가 다를 뿐이다.

**Mnemosyne는 이 취약한 표면들을 typed, 양방향 무결성 스택으로 대체한다.**

- **atomic store** (`docs/.atomic/workspace.atomic.json`) 가 단일 source
  of truth — 타입드 레코드 (Section / ChangelogEntry / FrozenList /
  CrossRef) + append-only 감사 의미.
- `docs/GENERATED.md` 는 사람이 읽는 유일한 산출물. atomic store에서
  결정론적으로 렌더링됨. *사람은 읽고, AI는 typed primitive 로 쓴다.*
- 모든 mutation 은 typed primitive 통과 — 저장 전에 T1 (cross-ref orphan
  reject) + T2 (frozen-ledger jaccard) 검증.
- **spec id 코드 인용** (`§3`, `Round 254`) 은 커밋 시점에 스캔됨 —
  hallucinated 또는 superseded 참조는 git 히스토리 진입 전 reject.
- **Section ↔ Implementation 바인딩** 으로 각 결정을 어떤 소스 파일이
  소유하는지 기록. spec section 이름이 바뀌거나 superseded 되면, 인용
  중인 코드 위치가 자동으로 surface 된다.

**상태:** Phase 0 hardening (7 crates). 500+ 테스트 그린. Mnemosyne는
자기 자신을 dogfood — 자체 디자인 히스토리는 atomic store
(`docs/.atomic/workspace.atomic.json`) 에 저장되며, `docs/GENERATED.md`
가 사람이 읽는 view 다.

## Mnemosyne 가 실제로 보호하는 것

Mnemosyne는 **세 가지 무결성 경계** 를 강제한다. 각각은 AI 매개 작성이
만드는 버그 클래스이며, 수작업 리뷰가 보통 놓치는 종류다.

### 1. 문서 ↔ 문서 (T1 cross-ref orphan reject)

섹션 간 cross-reference 가 절대 dangling 상태가 되지 않는다. `docs/SPEC.md`
의 `§3` 이 `§42` 를 참조하는데 `§42` 가 doc 내부에도, 기본 cross-doc
target 에도, atomic store 에도 없으면, 그 참조를 도입한 mutation 이
쓰기 시점에 reject 된다. `§3` 이름 변경은 그것을 가리키는 모든 cross_ref 를
원자적으로 자동 갱신한다.

**이게 잡는 것:** "§3 → §4 로 rename 시켰는데 AI 가 regex replace 를
하는 바람에 무관한 문서 8개에 broken ref 가 생겼다."

### 2. 문서 ↔ 히스토리 (T2 frozen-ledger jaccard)

`ChangelogEntry` 가 한 번 커밋되면 그 `sub_bullets` 는 append-only 다.
나중에 frozen 항목에서 bullet 을 *제거하는* mutation 은 jaccard inclusion
체크 (current ⊇ previous) 에서 실패한다. 감사 trail 이 git 히스토리에
의존하지 않고도 *증명 가능하게 immutable* 해진다 (파일 rename / squash-merge /
cherry-pick 은 결정 추적용으로 git 을 일상적으로 깨뜨린다).

**이게 잡는 것:** "AI 가 changelog 표현을 '개선' 했는데 그래서 Round 17
에서 우리가 실제로 뭘 결정했는지 모르겠다."

### 3. 문서 ↔ 코드 (Path B 양방향 binding + code-citation defense)

모든 spec `Section` 은 `implementations = [(file, symbol), ...]` 을 기록할
수 있다 — *그 결정을 소유하는 소스 코드*. `validate-code-refs` 가 설정된
프로덕션 소스 경로를 워크하면서 주석에서 `§<id>` / `Round NNN` 인용을
추출한다. 세 가지 defect 클래스가 reject 된다:

- **`Missing`** — 인용이 atomic store 에 없는 section/entry id 를 참조
  (hallucination).
- **`CitationUnbound`** — 인용이 그 section 의 `implementations` 리스트가
  binding 으로 주장하지 *않는* 파일에 등장. 둘 중 하나가 stale —
  section 의 binding 리스트이거나 그 인용 주석이거나. 어느 쪽이든 실제
  defect, 대칭적으로 surface 된다.
- **`ImplementationMissing`** — Active section 의 `implementations` 가
  비어 있음. "Active" = "이 결정은 코드로 뒷받침된다" 의 의미이므로,
  뒷받침이 기록되지 않은 Active section 은 그 계약을 깬다.

Pre-commit hook 이 셋 모두를 reject gate 로 연결한다. spec section 의
rename 이나 supersede 는 cascade scan 을 트리거하여 모든 인용 중인 코드
위치를 stderr 로 출력 — stale 인용이 즉시 surface 된다.

**이게 잡는 것:** "지난달 Round 254 → Round 256 으로 rename 했는데 AI 가
auth.rs 에 남긴 `// see Round 254` 주석이 6주 동안 어디서도 flag 되지
않았다."

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

# 선택 사항 — code-citation defense 활성화. 소스 주석의 §id / Round-N
# 참조가 hallucination 일 때 reject 한다. R306 에서 plugin substrate
# 네임스페이스로 rename — 동작은 변경 없음.
[plugins.set_equality_validator]
paths = ["src/"]
severity_missing = "warn"   # baseline 깨끗해지면 "reject" 로 승격
severity_binding = "warn"
comment_only = true
```

그 다음:

```bash
mnemosyne-cli validate-workspace   # T1 + round-trip + atomic ledger
mnemosyne-cli validate-code-refs   # citation defense ([plugins.set_equality_validator] 설정 시)
```

이 명령들이 baseline을 뽑는다 — T1 orphan 총합, round-trip mandatory
상태, T3/T4 스타일 위반, atomic ledger sync, 그리고 소스 안의 spec-id
인용 중 해소되지 않는 것. 이 baseline을 기준으로 이후 mutation은
*증분으로* 평가된다.

상세 가이드는 [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) 와
[docs/SCHEMA_GUIDE.md](docs/SCHEMA_GUIDE.md) 참조.

## AI 에이전트와 함께 쓰기 (MCP)

`mnemosyne-mcp` 는 Model Context Protocol 서버. AI 클라이언트 (Claude Code,
Cursor, Cline, Continue, Copilot Chat 등) 가 stdio로 연결하면 다음을 얻는다:

- **16개 typed tool** — validate / query / 12개 atomic mutate primitive
  (Section + ChangelogEntry typed-field setter). 각 tool의 인자는
  validator 도달 전에 JSONSchema로 검증됨.
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
      - run: mnemosyne-cli validate-code-refs   # 선택 사항, mnemosyne.toml 에 [plugins.set_equality_validator] 가 있을 때
```

동일한 세 명령 + `cargo clippy --workspace --all-targets` gate 가
tracked 된 `.githooks/` 디렉토리에 wired. clone 마다 한 번 설치:

```bash
git config core.hooksPath .githooks
```

3 개의 hook 가 자동 실행된다:
- `pre-commit` — atomic-sidecar / GENERATED.md sync, code-citation
  defense, workspace validate (tracked doc 가 staged 일 때), clippy
  (`.rs` 가 staged 일 때).
- `commit-msg` — `COMMIT_FORMAT.md` 강제 (subject ≤ 72 bytes, body
  ≤ 72 bytes / line, 1–3 bullets, English + 타이포그래픽 화이트리스트).
- `pre-push` — push 직전 `validate-workspace` + clippy 재실행, 마지막
  `pre-commit` 이후의 state drift 캐치.

Citation defense baseline 이 깨끗해지면 `mnemosyne.toml` 에서
`severity_*` 를 `warn` → `reject` 로 승격하면, 이후 새 hallucinated
인용을 도입하는 모든 커밋이 hook 단에서 차단된다.

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

Mnemosyne 의 핵심 추상화 — *AI 가 편집하는 마크다운 문서는 안전을 위해
typed invariants 가 필요하다* — 는 디자인 문서를 훌쩍 넘어서 일반화된다.
로드맵은 그 일반화 방향을 따라간다: 동일한 primitive (Section / CrossRef
/ ChangelogEntry / FrozenList), 동일한 무결성 보장 (T1 / T2 / Path B),
그 위에 얹는 schema 만 달라진다.

### Phase 0 — 디자인 문서 라이프사이클 (현재)

프로덕션 dogfood. Mnemosyne 자체 디자인 히스토리가 atomic store 를
통과한다. Round 252-272 hardening arc 가 핵심 무결성 gap 들을 닫았다:

- T1 cross-doc orphan reject + `[[orphan_ledger]]` opt-in carry
  (정당한 legacy 참조용).
- Atomic axis `decision_status` field + author-time + validate-time 가드
  (T1 rule 4 양 축에 걸쳐서).
- Code-citation defense reject 모드 (`severity_missing` /
  `severity_binding` = `reject`) — pre-commit 단에서 hallucinated spec
  참조 차단.
- 양방향 Spec ↔ Code binding — `Section.bindings` (타입 있는 trace-link
  edge: `kind = implements` «satisfy» / `references` «trace») + 세 edge
  set-equality 검출 (`CitationUnbound` + `ImplementationUnbacked` +
  `ImplementationMissing`; 마지막은 `implements`만 coverage 로 카운트).
- Atomic ChangelogEntry mutate API + 모든 성공 write 후 `GENERATED.md`
  auto-cascade 재생성.

### Phase 1 — 서사 매체 어댑터

다음 채택 표면: 장편 픽션, 게임 스크립트, TRPG 캠페인 노트, 월드빌딩
위키, 캐릭터 바이블. 이 매체들은 Phase 0 를 추동한 AI 변형 위험 패턴을
공유한다 — LLM 매개 편집이 컴파일러가 잡지 않는 invariant 를 깬다는 점.
다만 schema 와 primitive 가 달라진다.

구체적 타겟 장르와 Mnemosyne 이 보호할 invariant:

- **장편 픽션 초고 관리.** 2장에서 정립된 캐릭터 눈 색깔이 15장과
  일치해야 한다. rename 된 세력이 무관한 장면 40개에 orphan 참조를
  남기면 안 된다. atomic-store + T1 invariant 가 직접 lift 된다 —
  바뀌는 것은 entity schema (Character / Location / Faction / Scene) 와
  mutate primitive (`set_character_eye_color`,
  `rename_faction_with_cascade`).
- **게임 스크립트 (인터랙티브 픽션, 다이얼로그 트리, 분기 서사).**
  분기 타겟이 resolve 되어야 한다. 장면 간 캐릭터 다이얼로그 schema 가
  일관되어야 한다. 조건 플래그 참조 (`if metPirateKing`) 가 dangling
  되면 안 된다. 동일한 T1 cross-ref orphan reject — section graph 대신
  scene graph 에 적용.
- **TRPG 캠페인 노트.** NPC stat block, location 배경, plot beat 감사
  trail. GM 의 "세 세션 전에 내가 뭘 정했지" 문제는 정확히 frozen-ledger
  문제다 — git history 가 결정 provenance 를 carry 하지 않지만, 세션
  번호로 정렬된 ChangelogEntry 스트림은 한다.
- **월드빌딩 위키.** 세력 관계, 타임라인 일관성, 마법 시스템 제약. 항목
  간 참조는 orphan reject 가 필요하고, "마법 법칙" 변경은 frozen-ledger
  의미가 필요하다 — 그래야 retroactive edit 이 앞선 10개 챕터를 조용히
  모순시키지 않는다.
- **캐릭터 바이블.** 이름 표기 normalization, 나이/타임라인 산수,
  관계 그래프 일관성. 디자인 문서와 동일한 위험, 다른 schema 필드.

Phase 1 priority audit (Round 172) 에서 fictional adapter 는 6.00 /
3.00× margin 으로 Phase 1 첫 진입 대상으로 채택. 선택 사유: (a) AI
매개 작성 workflow 가 이 영역에 이미 존재, (b) 자산당 카운트가
workspace-scope JSON store 에 DB migration 없이 들어맞는 규모, (c)
무결성 파괴 실패 모드가 end user 에게 보인다 — 독자가 캐릭터 눈 색깔이
바이블과 모순되는 걸 알아챈다 — 그래서 validator reject mode 의
calibration 이 잘 유지된다.

Phase 1 은 현재 Phase 0 스택 안정화 뒤로 *deferred* — 폐기 아니라
유보. 로드맵은 그 경계에 솔직하다.

### Phase 1.5 — Cascade-gate 풀스케일 측정

현재 workspace scope 에서 사용 중인 per-record Salsa cascade 패턴이
50K-asset 워크로드에서 발표된 p95 예산을 만족하는지 검증. Phase -1A
측정 spike (`bench/` 워크스페이스, historical baseline 으로 보존) 의
substrate 가 carry 된다. 노벨-규모 (~50K facts) 워크스페이스를 효율적으로
관리할 narrative 매체 어댑터의 인프라 선결 조건.

### 로드맵에 *없는* 것

위 항목들은 audit ledger 의 *registered carry* — 약속이 아니다. Phase 0
스택 안정성이 진입 gating 기준이다. 코드베이스는 "현재 작동하며
dogfood 된 것" 과 "priority audit 에 이름이 등록된 것" 을 의도적으로
분리한다 — registered carry 가 어떤 특정 timeline 에 ship 된다는 함의는
없다.

## 라이선스

MIT 또는 Apache-2.0 (선택).
