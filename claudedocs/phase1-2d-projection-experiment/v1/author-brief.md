# 저자 브리프 (1단계) — 하룻밤 호러를 게이트-검증된 fact base로 짓는다

너는 게임 시나리오 저자다. 프리미스(`premise.md`)를 받는다. 할 일은 짧고 꽉
짜인 **2D 호러 탈출극**을 발명하고 — 넷의 인물, 여섯 방 남짓의 잠긴 건물,
복도를 도는 그것, 이어지는 열쇠, 정문 앞의 한 선택과 두 결말 — 그것을
`mnemosyne-cli`로 **fact base**로 기록하는 것이다. 도구의 일관성 게이트를 네 작업의
자기검증 피드백으로 삼아, 게이트가 짚는 것을 고쳐가며 지어라.

너는 **산문이 아니라 사실**을 짓는다. 누가 여기 있고, 무엇이 참이고, 각자 무엇을
알고, 각 장면이 무엇이고, 방들이 어떻게 이어지고 어디가 잠겼고, 열쇠가 어떻게
이어지고, 비밀이 언제 드러나고, 두 길이 어떻게 갈리는지 — 설계 성경을 쓴다고
생각하라. **나중의 별도 단계가 이 사실을 두 갈래로(각기 다른 화면 형식으로)
플레이 장면으로 바꾼다** — 그러니 각자의 앎과 각 장면·열쇠·비밀을 사실 안에서
**분명하고 뚜렷하게** 만들되, 여기서 산문은 쓰지 마라. 대사도 독백도 여기선
쓰지 않는다 — 그건 렌더의 몫이다. 너는 "누가 무엇을 아는가"를 프레임-사실로
남기고, 렌더가 그것을 대사나 독백으로 목소리 낸다.

**이 파일과 `premise.md`만 읽어라. `claudedocs/phase1-2d-projection-experiment/`
아래 다른 파일은 열지 마라** — 실험 내부라 읽으면 편향된다. `run/author/`에서
작업하고 산출물을 거기 남겨라.

## 게임을 게임으로 만드는 네 가지 — 모두 사실로 지어라

### (1) 인물 = 프레임 (관점)

substrate는 "특정 인물이 무엇을 아는가/믿는가"를 **프레임**(이름 붙은 인식 관점)
으로 표현한다. 프레임을 넉넉히 써라 — 하나의 ground-truth 프레임(`gt`) +
**인물마다 별도 프레임**: `hajun`, `seri`, `minu`, `yeon`, 그리고 적대자 **`geot`**.

- 한 인물 프레임의 사실 = **그 인물의 앎**. 같은 사건을 누구는 알고 누구는
  모른다. 두 사람이 상반되게 믿을 수 있다 — 각기 다른 마음에 관한 참된 사실.
- **`geot`(그것) 프레임이 이 게임의 핵심 시험이다**: 그것은 넷이 **어디 있는지
  안다**(그것의 프레임에 위치-앎 사실), 넷은 그것이 어디 있는지 **모른다**(넷의
  프레임엔 없다). 이 앎의 격차를 사실로 지어라 — 공포는 이 비대칭에서 온다.
- 화자 `hajun`의 프레임엔 **관찰·반응**(나중에 독백이 될 재료)을 담아라.
- 확인: `report-frame-view --frame <person> --branch <world> --entity <who/what>
  --at <scene> --order order.json --sidecar store.atomic.json`

### (2) 공간 = 방-그래프 + 잠긴 문 (이 장르의 심장)

이건 걸어 다니는 2D 게임이다. 공간을 **사실로** 지어라 — 나중에 한 렌더는 이걸
걸어 다니고(쯔꾸르 축), 다른 렌더는 배경으로만 쓴다(VN 축). 둘 다 같은 이
사실을 읽는다.

1. **방 = `kind:place` 엔티티.** 여섯 남짓: `lobby` `long-hall` `classroom-3`
   `stairwell` `staff-room` `archive` `exit`. 먼저 kind를 선언:
   ```
   mnemosyne-cli add-entity-kind --kind place --sidecar store.atomic.json
   mnemosyne-cli add-entity --entity long-hall --kind place --description "긴 복도" --sidecar store.atomic.json
   ```
2. **인접 = 사실.** 두 방을 잇는 변은 `adjacent` 술어의 **typed 사실**이다. 술어는
   `place↔place`로 게이트(공간-지도 게이트):
   ```
   mnemosyne-cli add-predicate --predicate adjacent --object-kind entity \
       --subject-kind place --object-entity-kind place \
       --description "두 장소가 통행으로 이어짐" --sidecar store.atomic.json
   ```
   각 변을 `add-fact ... --typed-subject A --typed-predicate adjacent --typed-object-entity B`로.
3. **잠긴 문 = 조건이 걸린 변.** 어떤 변은 조건이 채워져야 통행된다. 그 변(인접
   사실)에 **edge-guard**를 걸어라 — 조건은 "무엇을 가졌다"는 또 다른 사실:
   ```
   mnemosyne-cli add-edge-guard --fact f-adj-hall-staffroom --condition f-have-staffkey --sidecar store.atomic.json
   ```
   이게 열쇠-게이트다. 조건 사실(`f-have-staffkey`)은 열쇠를 손에 넣는 장면의
   사실이다. 게이트는 **평가하지 않는다**(소비자/런타임의 몫) — 스토어는 "이 문은
   이 조건을 요구한다"만 선언한다. 정문 변엔 `f-have-masterkey` 조건을 건다.
   변의 비용이 의미 있으면 `add-edge-cost --fact <adj> --n <int> --unit step`도.

프리미스의 선행 사슬을 이 어휘로: `classroom-3`에서 살펴 얻는 교무실 열쇠 →
`staff-room` 변 해제 → 마스터 열쇠 → `exit` 변 해제. **모든 세계선에서 순서가
실제로 지켜지게**(교무실 열쇠 장면이 교무실 진입보다, 마스터 열쇠가 정문보다
canon order에서 앞서게).

### (3) 퀘스트 (목표·선행조건·결과)

퀘스트마다 같은 한 줌의 사실로 기록한다 — 이게 계약이다:

1. **퀘스트 = `kind:quest` 엔티티.** 넷 남짓: `q-escape`(나간다, 메인),
   `q-staffkey`(교무실 열쇠 찾기, 선행), `q-save-yeon`(연을 데리고 나간다, 길마다
   다름), `q-truth`(그것의 정체, 선택).
2. **누가 이끄나** — typed `pursues` 사실(actor → quest).
3. **어디서 주어지나** — 주는 장면의 사실, `--payoff-expectation expected`, 그리고
   **지도 위치**(아래 (4)의 surface)로 못 박는다.
4. **무엇을 선행하나** — typed `requires` 사실 + **실제 순서**: 선행이 canon
   order에서 정말 먼저. `q-escape --requires-> 마스터 열쇠`, `staff-room 접근
   --requires-> 교무실 열쇠`. 최소 이 사슬 하나는 order-real로.
5. **어떻게 완료되나** — 완료하는 세계선에서, 주는 사실을 `--pays-off`로 갚는 사실
   (선택적 typed `completed_by`). **퀘스트는 길마다의 의무지 전역 플래그가
   아니다**: `q-save-yeon`은 **함께-길에서만 작별로 완료**되고 **홀로-길에서는
   갚지 않은 채(열린 채)** 둔다. 이 갈림이 두 결말을 진짜 다른 게임으로 만든다.
   최소 한 퀘스트가 한 종착 길에서 완료·다른 길에서 열림이어야 한다.

### (4) 노출 계획 (disclosure) — 지도 위치와 비밀

퀘스트가 주어지는 **장소**와 비밀이 **처음 드러나는 지점**을 하나의 telling으로:
```
mnemosyne-cli add-disclosure-plan --telling play --default-mode state --sidecar store.atomic.json
mnemosyne-cli set-disclosure --telling play --fact f-give-escape --mode state \
    --surface sc-03,exit --sidecar store.atomic.json
```
- **surface** `--surface <scene>[,<object>]` = 지도 위 그 사실이 있는 장소/사물.
  퀘스트 주는 사실마다 surface를 달아라. surface의 object는 등록된 엔티티여야
  한다(장소·사물에 엔티티를 주라: `exit` `record-book` `staff-desk` 등).
- **비밀 = withhold + first_at.** 세 비밀(S1 연·S2 그것·S3 출구)을 `withhold`로,
  세계선마다 처음 드러나는 장면을 `--first-at`로:
  ```
  mnemosyne-cli set-disclosure --telling play --fact f-secret-yeon --mode withhold \
      --first-at together=sc-14 --first-at alone=sc-15 --surface sc-06,record-book --sidecar store.atomic.json
  ```
  withhold 비밀은 **typed** 사실이어야 한다(게이트가 typed claim으로 매칭).
  **각 비밀의 surface는 그 비밀이 벌리는 사물에 달아라**(예: S1은 자료실의
  `record-book`) — 렌더가 "무엇을 살펴야 드러나나"를 알 수 있게. 계획은
  **성글게**: 퀘스트-주는 사실의 surface + 세 비밀의 withhold만.
  `report-disclosure-coverage --telling play`로 확인.

> **주의(실험이 관찰하는 지점, 저자는 그냥 최선으로 지어라):** `first_at`은
> 세계선마다 **한 장면**만 못 박는다. 한 비밀이 여러 방식으로 벌릴 수 있어도
> (기록을 살펴서 / 그것에게 몰려서 / 민우의 말에서) 각 길에서 **가장 자연스러운 한
> 지점**을 골라 못 박아라. 그 사물에 surface를 달아 두면 된다.

## 갈림길, 두 길, 계속 사람 있게

정문 앞 한 장면(`sc-12` 근처)에서 **함께(together) / 홀로(alone)** 두 세계선으로
갈린다. 갈리는 장면과 두 branch를 이름 붙여라. scene-id를 나눠 잡아라(공유 척추
`sc-01..sc-12`, 이후 각 길의 꼬리 `tg-13..` / `al-13..`). **넷을 각 길의 마지막
장면까지 존재·인지·행동하게** 하라 — 결말에서 조연이 사라지지 않게.

## 백레퍼런스(구조적), 반복 금지

나중 장면이 앞 사건을 되짚으면(콜백), 그 되짚음은 **구조적**이어야 한다: 그
사실의 `--evidence`에 앞 장면을 넣어라(산문이 아니라). 인용된 장면은 이 사실의
**자기 세계선에서** 이 사실 이전에 도달 가능해야 한다(다른 branch만의 장면이나
더 뒤 장면은 못 인용 — 게이트 `evidence_unreachable`가 잡는다). 공유가 필요하면
갈림 전 **공유 척추**에 두라.

## 방법은 톱다운

장면별 자유연상 금지. 뼈대 먼저: 규모+장면, 프레임 목록, ground truth + 두 결말,
갈림, 방-그래프(엔티티+인접+잠긴 변), 열쇠 사슬, 퀘스트(엔티티+주기+선행+길별
완료/열림), 노출 계획(surface + 세 withhold). **그걸 게이트-clean으로** 만든 뒤
잇는 살을 채우고 매 길을 사람 있게 하라.

## 산출물 (`run/author/`)

1. **빈 seed 스토어** `store.atomic.json`:
   ```json
   {"sections":{},"changelog_entries":{},"inventory_entries":{},"confirmation_events":{},"frames":{},"branches":{},"entities":{},"predicates":{},"narrative_facts":{},"disclosure_plans":{},"schema_version":23}
   ```
2. **`sections.json`** — 장면들(각 장면 = 섹션; 어느 사실이 쓰는 canon 좌표는
   여기 먼저 존재해야 함). `import-sections --manifest sections.json`.
3. **`facts.json`** — 프레임·branch·엔티티(장소 `kind:place`, 사물, `kind:quest`)·
   술어(`adjacent` `pursues` `requires` `completed_by`, 소지 이동이 있으면
   `possession`)·사실. `import-facts --manifest facts.json` (한 원자 트랜잭션 —
   하나라도 invalid면 아무것도 안 써짐; 고치고 재실행. 행 수정은 facts.json 고쳐
   빈 seed부터 재빌드). 필드 규칙:
   - `canon_from`/`section_id`/`forks_at`/`evidence[]` = sections.json의 장면 id.
   - `branch`: 갈림 전 공유 척추엔 생략(root `main`); 갈림 후 각 사실에 길 태그.
   - `frame`: `gt`는 ground truth; 인물 앎은 그 인물 프레임. 믿음-사실과 진실-
     사실을 `conflicts`로 묶지 마라 — 두 프레임 위 두 참.
   - `payoff_expectation`: `expected`만 또는 생략. `pays_off`: 갚는 setup id 배열.
   - `typed`: 퀘스트(pursues/requires/completed_by), 인접(adjacent), 비밀에.
4. **`order.json`** — canon order: 주 척추를 변의 사슬로, 각 branch는 갈림 장면에서
   시작하는 자기 사슬. 모든 게이트에 `--order order.json`.
5. **`narrative-rules.json`** — 필요한 규칙(소지 이동을 쓰면 exclusive possession).
   `--rules`로 전달.

## 게이트 (매 import 후 --order [--rules]와 함께 전부)

```
mnemosyne-cli validate-continuity            --order order.json [--rules narrative-rules.json] --sidecar store.atomic.json
mnemosyne-cli report-fork-tree               --order order.json --sidecar store.atomic.json
mnemosyne-cli report-timeline-gaps --world <each-road> --order order.json --sidecar store.atomic.json
mnemosyne-cli report-payoff-coverage         --order order.json --sidecar store.atomic.json
mnemosyne-cli report-disclosure-coverage --telling play --sidecar store.atomic.json
mnemosyne-cli report-playthrough-manuscript --world <each-road> --telling play --order order.json --sidecar store.atomic.json
mnemosyne-cli report-playable-world --telling play --order order.json --sidecar store.atomic.json
```

읽고 **고쳐라** — 아래가 될 때까지:
- `validate-continuity`: 구조 0 + interval 0(증거-도달성·off-branch 포함; 규칙
  쓰면 그 위반도 0).
- `report-fork-tree`: 갈림 PLACED; 두 길 등록; 각 세계선이 종착에 닿음.
- `report-playthrough-manuscript --world W`: 매 길 unplaced 0 / undecidable 0.
  ("outside order"는 정상 — 다른 길 장면).
- `report-payoff-coverage`: 각 퀘스트-주는 setup은 어느 길에서 갚였거나 **설계상
  열린 채**(건너뛰는 `q-save-yeon` 등) — 어느 쪽인지 네가 알고 있어야 함.
- `report-timeline-gaps`: 어느 세계선도 gap / 못 닿은 장면 없음.
- `report-disclosure-coverage --telling play`: 퀘스트-주는 사실마다 surface,
  세 비밀 withhold 등록됨.
- `report-playable-world --telling play`: clean 실행 + 각 퀘스트-주는 surface가
  한 길의 walk에서 장소로 resolve(이게 나중 단계가 읽는 지도).

## 산출물 최종 (`run/author/`)

- `sections.json` `facts.json` `order.json` `narrative-rules.json`
  `store.atomic.json`(최종, 게이트-clean).
- `author-log.md` — 뼈대(프레임 목록; 두 결말; 갈림; 방-그래프+잠긴 변+열쇠
  사슬; 퀘스트 = 엔티티+주기+선행+길별 완료/열림+지도 위치; 노출 계획), 그리고
  write→gate→repair 반복 횟수, 각 게이트가 짚은 것, 무엇을 고쳤는지. 게이트가
  앎·순서·백레퍼런스 문제를 잡은 순간과 각 길을 어떻게 사람 있게 뒀는지 적어라.

## 규모

≈18–22 장면; 정확히 **한 갈림**(together / alone)을 ~한가운데서 두 종착 세계선으로,
각기 진짜 aftermath와 다른 결말; **인물 4 + 적대자 `geot`, 각자 프레임**, 매 길
꼬리까지 존재; **넷 남짓 맞물린 퀘스트**(메인 `q-escape` + 이를 게이트하는 선행
`q-staffkey` + 길마다 갈리는 `q-save-yeon` + 선택 `q-truth`), 각기 위 계약대로;
**여섯 남짓 `kind:place` + 인접 사실 + 잠긴 변(edge-guard)로 지은 열쇠 사슬**; 성근
노출 계획(퀘스트-주는 사실 surface + 세 비밀 withhold). 톱다운으로 뼈대 먼저,
그다음 채워라. 가장 꽉 짜인 하룻밤을 지어라 — 모든 결과가 놓인 원인으로 추적되고,
각자 딱 자기 길만 알고, 넷이 끝까지 있고, 열쇠가 이어지고, 의도하지 않은 건
아무것도 열린 채 두지 마라.
