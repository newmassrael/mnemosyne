# 물때가 셈한다 · *The Tide That Counts*

무드는 **미드나잇 매스**(고립 공동체 + 신앙으로 포장된 계약·셈)와 **포크호러**
("숲에서 온 것" — 여기선 *갯벌에서 온 것*)의 느린 잠식. 무대는 한국 조간대
어촌 **무월도(無月島)**. 물때에만 갯들로 육지와 이어지는 섬에서, 삼십 년 전
흉년에 맺은 계약의 셈이 마지막 핏줄에게 돌아온다.

이 디렉터리는 두 층으로 되어 있다.

| 층 | 파일 | 역할 |
|---|---|---|
| **산문(인간이 읽는 것)** | `MANUSCRIPT.md` | 8장 중편 본문. "EPUB" 표면. |
| **구조(스토어 = 작가의 방)** | `.atomic/tide.atomic.json` | 프레임·엔티티·사실·복선/회수·공개계획·체인지로그. `mnemosyne-cli query`로 읽는다. |

Mnemosyne는 medium-neutral 서사 엔진이다(`mnemosyne-core/narrative.rs`: *"a frame,
a claim, canon coordinates, and evidence refs exist for a novel … alike"*). 이
워크스페이스는 그 서사 레이어를 공포 소설 저작에 그대로 dogfood한다. 루트의
Mnemosyne 자체-spec 워크스페이스와 **완전히 분리**되어 있다(`mnemosyne.toml`의
CWD-상향 탐색이 이 디렉터리의 설정을 먼저 찾는다).

## 재현

```bash
cargo build --release -p mnemosyne-cli          # 리포지터리 루트에서
cd tales/the-tide-that-counts
./build-store.sh                                # 스토어를 0에서 재구성(idempotent)
```

`build-store.sh`는 모든 변경을 타입드 mutate 프리미티브(`add-section` /
`add-frame` / `add-entity` / `add-predicate` / `add-fact` / `add-disclosure-plan`
/ `set-disclosure` / `append-changelog-entry`)로만 적용한다. 사이드카 JSON을
직접 편집하지 않는다 — 그것이 Mnemosyne의 계약이다.

## 서사 엔진이 검증하는 것 (모두 통과)

```bash
mnemosyne-cli validate-workspace          # T1 orphan 0 / T3 reject 0
mnemosyne-cli validate-continuity         # 게이트 enabled, 위반 0
mnemosyne-cli report-payoff-coverage      # 체호프 셋업 8/8 회수, dangling 0
mnemosyne-cli report-irony-intervals      # 극적 아이러니 윈도(마을 신앙 vs 진실)
mnemosyne-cli report-disclosure-coverage --telling reader   # 보류→암시→명시 곡선
mnemosyne-cli report-playthrough-manuscript --telling reader  # 장면별 사실+공개모드
mnemosyne-cli report-entity --entity ent-mother             # 물어미 도시에
```

- **연속성**: `evidence`는 `canon_from` 이전 장면이어야 한다(아직 등장 안 한
  증거로 사실을 주장할 수 없음). 게이트가 초기 저작 결함 3건을 잡아 교정했다.
- **복선/회수(체호프)**: 8개 셋업(경고·금·종·빈 방·편지·대체 규칙)이 후반
  장면에서 모두 회수된다. `pays_off`는 fact 정체성 링크.
- **극적 아이러니**: `frame-village`(물어미가 "지켜 준다")와 `ground-truth`(셈은
  사람을 거둔다)가 충돌하는 구간이 s04부터 결말까지 열린 윈도로 추적된다.
- **공개 통제(슬로우번의 핵심)**: `reader` telling이 계약의 진실 4종을
  `withhold`(기본)에서 `hint`(s02)→`imply`(s04/s05)→`state`(s08)로 푼다.
- **인식 전환(supersession)**: 지운의 "할머니는 자연사"(s03) 믿음이 편지로
  진실(s05)에 의해 교체된다 — manuscript 리포트가 supersede 지점을 표시한다.

> `validate-disclosure-leak`는 *산문 재추출* 아티팩트(본문이 실제로 무엇을
> 누설하는지 재스캔한 파일)를 `--against`로 요구한다. 그건 prose↔plan 일치
> 검사용 별도 파이프라인으로, 여기선 범위 밖이다.

## 읽는 순서

본문은 `MANUSCRIPT.md`. 구조적 개요는 위 리포트들. 저작 결정의 audit trail은
`mnemosyne-cli query`의 changelog 3개 라운드(무대 → 사실/복선 → 공개 일정).
