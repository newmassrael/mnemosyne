# vocab — id 목록 ONLY (claim/fact/plan 없음)

재추출 시 아래 id만 사용하라. 내용(무엇이 참인지)은 여기 없다 — 대본/로그에서 읽어라.

## 프레임 (누구의 앎)
`gt`, `hajun`, `seri`, `minu`, `yeon`, `geot`

## 갈림 구조
- 갈림 장면(fork): `sc-12`
- 두 길(branch/road): `together`, `alone` (둘 다 `main`에서 sc-12 fork). 이 대본은 `together` 길이다.

## entity-kind
`place`, `character`, `quest`, `item`, `fixture`

## 엔티티 (id : kind)
- `e-lobby` : place
- `e-long-hall` : place
- `e-classroom-3` : place
- `e-stairwell` : place
- `e-staff-room` : place
- `e-archive` : place
- `e-exit` : place
- `e-hajun` : character
- `e-seri` : character
- `e-minu` : character
- `e-yeon` : character
- `e-geot` : character
- `q-escape` : quest
- `q-staffkey` : quest
- `q-save-yeon` : quest
- `q-truth` : quest
- `e-staff-key` : item
- `e-master-key` : item
- `e-record-book` : fixture
- `e-staff-desk` : fixture
- `e-desk-3` : fixture

## 술어 (id : object_kind [subject_kind→object_entity_kind])
- `adjacent` : entity [place→place]
- `pursues` : entity [character→quest]
- `requires` : entity [quest→quest]
- `completed_by` : entity [quest→character]
- `holds` : entity [character→item]
- `at` : entity [character→place]
- `seeks` : entity [character→character]
- `hidden_nature` : token
