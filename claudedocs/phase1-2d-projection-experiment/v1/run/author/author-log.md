# 저자 로그 — 온실 (긴 복도 끝의 방)

게이트-검증된 fact base 한 편. 산문 0 — 사실만. 대사·독백은 렌더의 몫.
스토어 = `store.atomic.json` (seed schema_version 23 → 쓰기 시 현행 v38로 승격).

---

## 1. 뼈대 (톱다운, 살 채우기 전에 먼저 놓은 것)

### 규모
- 20 장면 · 6 프레임 · 5 인물(+적대자) · 7 장소 · 4 퀘스트 · 정확히 한 갈림.
- 공유 척추 `sc-01..sc-12`, 정문 앞 `sc-12`에서 두 종착 세계선으로 갈림.
- 함께 꼬리 `tg-13..tg-16`, 홀로 꼬리 `al-13..al-16`.

### 프레임 목록 (인물 = 관점)
- `gt` — ground truth (실제 벌어지는 일 + 건물 지도 + 세 비밀의 원본).
- `hajun` — 화자. 관찰·반응 (나중에 독백이 될 재료).
- `seri` — 실용/앞장. 괴담→믿음 (in-frame 신념 변화, supersedes).
- `minu` — 겁많음. **연이 여기 있어선 안 됨을 어렴풋이 앎** (말 못 함).
- `yeon` — 비어 있음. 자신이 무엇인지 모름.
- `geot` — **핵심 시험**. 넷의 위치를 앎(`at` 추적 사실 + 목적 `seeks`),
  넷은 그것의 위치를 모름(넷 프레임에 `at(e-geot,…)` 사실 0 = 부재로 인코딩).
  이 앎의 비대칭이 공포. 하준 쪽엔 `f-hajun-cant-place`(기척은 느끼되 못 짚음).

### 두 결말 (정문 앞 갈림, 끝까지 다른 게임)
- **together** — 하준이 연을 붙잡고 진실을 마주. 연이 남기를 택하고 그것이 멎음.
  셋(하준·세리·민우)이 빗속으로. S1·S2 이 길에서 드러남. `q-save-yeon` 작별로 완료,
  `q-truth` 완료.
- **alone** — 하준이 홀로 나가고 문이 닫힘. 셋(세리·민우·연)이 남음. S3 이 길에서
  드러남. `q-save-yeon`·`q-truth` **열린 채**(건너뜀).
- 갈림 장면 = `sc-12`; 두 branch = `together` / `alone` (둘 다 `main`에서 `sc-12` fork).

### 방-그래프 + 잠긴 변 + 열쇠 사슬
- 7 `kind:place`: `e-lobby` `e-long-hall`(허브) `e-classroom-3` `e-stairwell`
  `e-staff-room`(잠김) `e-archive` `e-exit`(잠김).
- 인접(`adjacent`, typed, place↔place, 공간-지도 게이트) 6 변:
  lobby–hall, hall–classroom-3, hall–stairwell, hall–staff-room*, hall–archive,
  lobby–exit*.  (* = 잠긴 변)
- 잠긴 변 = edge-guard(사실에 조건 사실을 건 것, 평가는 소비자 몫):
  - `f-adj-hall-staffroom` ← 조건 `f-have-staffkey`
  - `f-adj-lobby-exit`     ← 조건 `f-have-masterkey`
- 열쇠 사슬(모든 세계선에서 canon order로 실제로 지켜짐):
  `sc-05` 3반 교실 책상 → 교무실 열쇠(`f-have-staffkey`) → 교무실 변 해제 →
  `sc-06` 마스터 열쇠(`f-have-masterkey`) → 정문 변 해제 → 갈림 후 탈출.
  조건 사실이 변 해제보다, 마스터 열쇠가 정문 통과보다 order상 앞선다.

### 퀘스트 (엔티티 + 주기 + 선행 + 길별 완료/열림 + 지도 위치)
계약 = 주는 setup(`expected`, untyped) + `pursues`(actor→quest) + `completed_by`
(quest→actor, setup을 `pays_off`) + 필요시 `requires`(quest→quest).
- `q-escape`(메인) — 이끔 hajun. **requires q-staffkey**(order-real 선행 사슬).
  giving `f-give-escape`@sc-02 (surface sc-02/e-exit). 완료: together `tg-16`,
  alone `al-14`. (bare main = 미완 = 아래 §4 참조).
- `q-staffkey`(선행) — 이끔 seri. giving `f-give-staffkey`@sc-03 (surface
  sc-03/e-staff-room). 완료: **공유 척추 `sc-05`에서** → 모든 세계선 done.
- `q-save-yeon`(길별) — 이끔 hajun. giving `f-give-save-yeon`@sc-02 (surface
  sc-02/e-yeon). **together `tg-15` 작별로 완료 · alone 열린 채**(건너뜀).
- `q-truth`(선택) — 이끔 hajun. giving `f-give-truth`@sc-07 (surface
  sc-07/e-record-book). **together `tg-14` 완료 · alone 열린 채**.

### 노출 계획 (telling `play`, 성글게)
- default_mode `state`(퀘스트 시작은 알려줌). 4 퀘스트-주는 사실에 surface.
- 세 비밀 = withhold + typed(`hidden_nature` 토큰) + 세계선별 first_at + 벌리는
  사물에 surface:
  - S1 `f-secret-yeon`(연=잔상) — withhold, first_at together=`tg-14`,
    surface sc-08/e-record-book.
  - S2 `f-secret-geot`(그것=관리인의 남은 것, 연을 찾음) — withhold,
    first_at together=`tg-14`, surface sc-08/e-record-book.
  - S3 `f-secret-exit`(정문 밖에도 밤은 안 끝남) — withhold, first_at alone=`al-14`,
    surface sc-12/e-exit.
- 프리미스의 fork 배정을 그대로: S1·S2는 together에서, S3는 alone에서 처음 드러남.
  기록부(sc-08)엔 젖어 번진 한 칸(`f-gt-record-examined`)만 두어 **먼저 살펴도
  진실은 착지하지 않게** 함 — first_at은 갈림 뒤 한 지점에 못 박음.

---

## 2. 빌드 순서 (write → gate)

1. 빈 seed `store.atomic.json` (브리프 그대로, schema_version 23).
2. `import-sections --manifest sections.json` → 20 장면 생성 (0 no-op).
3. `import-facts --manifest facts.json` → 한 원자 트랜잭션:
   6 프레임 + 2 branch + 5 entity-kind + 21 엔티티 + 8 술어 + 59 사실 +
   1 disclosure-plan + 7 override. **첫 실행에 통과** (invalid 0).
4. `add-edge-guard` ×2 (잠긴 변 두 개). manifest에는 edge-guard 슬롯이 없어 별도 CLI.
5. 게이트 전부 실행.

---

## 3. write → gate → repair 반복

- **콘텐츠(스토어) 반복 = 1.** import가 첫 시도에 원자적으로 통과했고, 7 게이트
  전부 첫 통과. 저작 결함으로 인한 repair 0.
- **호출(invocation) 수정 = 1 (콘텐츠 아님).** `validate-continuity --rules
  narrative-rules.json`가 상대경로를 워크스페이스 루트 기준으로 풀어
  `No such file` → **`--rules`를 절대경로로** 주어 해소(dnd 선례가 이미 "absolute
  --rules"로 기록해 둔 그 지점). 스토어는 손대지 않음.

첫 패스가 clean한 이유는, 쓰기 전에 다음을 스토어가 **구조적으로 위반할 수 없게**
설계했기 때문 — 게이트가 잡기 전에 막았다:
- **증거-도달성**: 모든 백레퍼런스(`--evidence`)를 자기 세계선의 공유 척추
  장면(또는 같은 꼬리의 앞 장면)으로만 걸어, `evidence_unreachable`/off-branch가
  날 수 없게. 공유가 필요한 콜백은 갈림 전 척추에 둠(예: `f-done-escape-tg`가
  sc-02를, `f-hajun-carries-al`이 sc-08을 인용).
- **순서-실재(order-real) 선행**: 열쇠 사슬(sc-05→sc-06)과 `q-escape requires
  q-staffkey`를 canon order로 정말 앞세움. `q-staffkey`를 갈림 전 척추에서 완료해
  세 세계선 모두 done.
- **믿음 vs 진실**: 신념(seri 회의→믿음, geot 위치-추적)은 `supersedes_in_frame`
  in-frame 승계로, 진실과 `conflicts`로 묶지 않음 → same-frame overlap 0.

---

## 4. 각 게이트가 짚은 것 / 최종 판정

- **validate-continuity** (`--order` + 절대 `--rules`): `facts=59
  order_nodes=20/20 conflict_pairs=0 cross_scope=0 unordered=0 rules=1
  rule_unordered=0 unchained_state_pairs=0`; **violations 0 (structural=0
  interval=0)**; `--severity reject --interval-severity reject`로 exit=0.
  exclusive `holds`(per:object, 열쇠 단일 소지) 위반 0.
- **report-fork-tree**: **2 registered world-line, 0 unplaced fork point**;
  together·alone 둘 다 `main`@`sc-12`에서 fork PLACED. 각 길이 종착(tg-16/al-16,
  maximal)에 닿음.
- **report-timeline-gaps** (per road): together `violated=0 unverifiable=0`,
  alone `violated=0 unverifiable=0` (0 interval rule). gap/미도달 0.
- **report-payoff-coverage** — **어느 것이 어디서 열린 채인지 전부 의도됨**:
  - together: paid=4 dangling=0 (전 퀘스트 완료).
  - alone: paid=2, **dangling=2 = `f-give-save-yeon`·`f-give-truth`** (설계상 건너뜀).
  - main(bare prefix): paid=1, dangling=3 (escape·save-yeon·truth) =
    **fork-lineage 귀결**. 두 길을 모두 `main`에서 fork하면 갈림 전 트렁크가
    dead prefix가 되어 cross-fork setup을 dangling으로 안고 감(스키마 문서가
    경고한 그 함정, dnd 선례가 그대로 수용한 형태). 이 setup들은 **실제 플레이
    가능한 두 길(together/alone)에서 전부 갚임** — bare main은 종착이 아니라
    선택 이전의 트렁크. 의도치 않게 열린 것 0.
- **report-disclosure-coverage --telling play**: `59 facts: disclosed=56
  hidden_by_design=3 never_planned=0`. 세 비밀 withhold 등록, 퀘스트-주는 4
  사실 surface, 계획 안 된 사실 0.
- **report-playthrough-manuscript** (per road): together·alone 모두
  `unplaced=0 undecidable=0 undeclared_adjacencies=0` (각 16 장면; "off road=4" =
  다른 길 장면, 정상).
- **report-playable-world --telling play**: 3 world 모두 clean; **퀘스트-주는 4
  surface 전부 각 길 walk에서 장소/좌표로 resolve**(unplaced 0). withhold 비밀은
  플레이 지도의 visible locator로는 안 뜨되(감춘 lore), render-brief 운반체
  (`report-playthrough-manuscript --telling`)가 mode/first_at/surface를 실어 나름.
- (보너스) **report-quest-graph**: 4 퀘스트 파생 상태가 설계와 일치 —
  q-escape done/done/open(main), q-save-yeon done(together)/open(alone),
  q-staffkey 전 세계선 done, q-truth done(together)/open(alone); requires 사슬 표출.
- (보너스) **report-authoring-frontier**: zero-fact/unplaced/unordered 장면 0,
  unresolved quest 0, never-planned disclosure 0 — 남은 gap은 위의 의도된
  dangling뿐.

---

## 5. 게이트가 앎·순서·백레퍼런스를 잡은 순간 / 각 길을 사람 있게 둔 법

- **앎(프레임)**: geot의 위치-앎을 `at` 추적 사실 3개(`f-geot-track-1..3`,
  supersedes 사슬)로, 목적을 `seeks`로 두고, **넷의 프레임엔 geot 위치 사실을 아예
  두지 않음** — 비대칭을 부재로 인코딩. minu만 `f-minu-knows`로 연의 이질을 어렴풋이
  쥠. 이 앎의 배분이 disclosure-coverage·frame 축과 충돌 없이 통과.
- **순서**: 열쇠 사슬과 `requires`를 canon order로 실재화 → timeline-gaps·
  payoff-coverage가 세 세계선에서 q-staffkey를 앞서 done으로 도출. 순서 위반이
  게이트에 걸린 적은 없음(설계로 선차단).
- **백레퍼런스**: 콜백은 산문이 아니라 `--evidence`의 앞 장면으로만
  (`f-done-*-tg`가 sc-02/sc-07/sc-08 인용). 전부 자기 세계선에서 도달 가능 →
  `evidence_unreachable`/off-branch 0.
- **넷을 끝까지 사람 있게**: 두 꼬리의 마지막 장면군(tg-13..16 / al-13..16) 모두에서
  하준·세리·민우·연 넷이 **각자 프레임의 사실로 인지·행동**함(검증: 네 인물 모두
  두 꼬리에 등장). together에선 연이 남기를 택하고(그녀의 agency) 셋이 나감;
  alone에선 셋이 안쪽에 남아 문을 두드리고·부르고·바라봄. 결말에서 조연이 사라지지
  않음.

---

## 6. 산출물

`store.atomic.json`(최종, 게이트-clean) · `sections.json` · `facts.json` ·
`order.json` · `narrative-rules.json` · 이 로그.
