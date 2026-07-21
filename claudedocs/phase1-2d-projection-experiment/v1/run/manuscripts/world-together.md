# 아웃라인 — "온실" · together 길 (도입 척추 sc-01..sc-12 + together 꼬리 tg-13..tg-16)

이 파일은 한 호러 탈출극의 **구조 아웃라인**이다 — 장면마다 무엇이 참이고, 누가
있고(프레임), 무엇을 알고, 어떤 퀘스트 실이 걸려 있는지. 각 `## sc-NN` / `## tg-NN`
헤딩을 순서대로 그대로 다루고, 헤딩을 유지하라.

## 표기 읽는 법 (중요)
- `+ f-… (프레임) [state]: <내용>` — 이 장면에서 **참이 되고, 이 장면에서 말해도
  되는** 사실. `(프레임)`은 누구의 앎인지: `gt`=서술이 참으로 제시, 인물이름=그
  인물이 알거나 믿거나 말함.
- `+ f-… (gt) [withhold first_at=tg-NN via <scene>/<object>]: <내용>` — 이 사실은
  이 장면에서 **참이 되지만(그 자리/사물에 있지만) 아직 말해선 안 된다**. `<object>`
  를 살펴도(또는 그 자리에 있어도) 이 진실은 **아직 드러나지 않는다**. 처음 드러나는
  때 = `first_at=tg-NN` 장면. **그 전 어느 장면에서도 이 내용을 서술도 인물도 말하면
  안 된다.** first_at 장면에서 비로소 착지시켜라.
- `- f-… (프레임): superseded by …` — 그 프레임 안에서 앞 믿음이 새 믿음으로 바뀜.
- 퀘스트 실: `f-give-*`=퀘스트가 열림, `f-pursue-*`=누가 맡음, `f-done-*`=완수,
  `f-req-*`=선행조건.
- **그것(geot)의 앎/무지 비대칭**: `geot` 프레임 사실은 그것이 넷의 위치를 **안다**는
  뜻. 넷의 프레임엔 그것의 위치가 **없다** = 넷은 그것이 어디 있는지 **모른다**.
  `f-hajun-cant-place`(기척은 느끼되 못 짚음)가 그 무지다. 이 격차가 공포다.

---

=== playthrough manuscript — 59 fact(s), 1 world(s) ===
world `together`: 16 scene(s), undeclared adjacencies=0, unplaced=0, undecidable=0, off road=4
## sc-01 — sc-01 로비 — 늦은 밤, 넷이 남다 [begins=8 ends=0 holding=8]
    + f-adj-hall-archive (gt) [state]: 긴 복도와 자료실이 이어진다
    + f-adj-hall-classroom (gt) [state]: 긴 복도와 3반 교실이 이어진다
    + f-adj-hall-staffroom (gt) [state]: 긴 복도와 교무실이 이어진다 (문은 잠겨 있다)
    + f-adj-hall-stairwell (gt) [state]: 긴 복도와 계단실이 이어진다
    + f-adj-lobby-exit (gt) [state]: 로비와 정문이 이어진다 (문은 잠겨 있다)
    + f-adj-lobby-hall (gt) [state]: 로비와 긴 복도가 통행으로 이어진다
    + f-geot-track-1 (geot) [state]: 그것은 넷이 로비에 있음을 안다
    + f-hajun-obs-1 (hajun) [state]: 하준은 유리 너머로 빗줄기가 두꺼워지는 것을 본다
## sc-02 — sc-02 자정 — 문이 잠기고 휴대폰이 먹통이 된다 [begins=5 ends=0 holding=13]
    + f-give-escape (gt) [state via sc-02/e-exit]: 잠긴 정문을 열고 온실을 빠져나가는 것이 오늘 밤의 목표로 걸린다
    + f-give-save-yeon (gt) [state via sc-02/e-yeon]: 넷이 연까지 함께 데리고 나가려 한다 — 곁가지가 열린다
    + f-pursue-escape (gt) [state]: 하준이 탈출을 이끈다
    + f-pursue-save-yeon (gt) [state]: 하준이 연을 데리고 나가는 것을 자기 몫으로 진다
    + f-secret-exit (gt) [withhold via sc-12/e-exit]: 정문은 끝이 아니다 — 그 밖에도 밤은 끝나지 않는다
## sc-03 — sc-03 긴 복도 — 첫 발, 교무실이 잠겨 있다 [begins=8 ends=1 holding=20]
    + f-geot-knows-four (geot) [state]: 그것은 넷이 어디 있는지 언제나 안다
    + f-geot-seeks (geot) [state]: 그것은 오직 연을 찾는다
    + f-geot-track-2 (geot) [state]: 그것은 넷이 긴 복도로 들어섰음을 안다
    + f-give-staffkey (gt) [state via sc-03/e-staff-room]: 교무실이 잠겨 있다 — 그 문을 열 작은 열쇠를 찾는 곁가지가 열린다
    + f-hajun-cant-place (hajun) [state]: 하준은 복도에서 무언가의 기척을 느끼지만 그것이 어디 있는지 짚지 못한다
    + f-pursue-staffkey (gt) [state]: 세리가 교무실 열쇠 찾기를 맡는다
    + f-req-escape-staffkey (gt) [state]: 정문에 닿으려면 먼저 교무실 열쇠 찾기가 끝나야 한다 — 교실→교무실 열쇠→교무실→마스터 열쇠→정문
    + f-seri-skeptic (seri) [state]: 세리는 복도를 도는 것을 흔한 괴담쯤으로 여긴다
    - f-geot-track-1 (geot): superseded by f-geot-track-2
## sc-04 — sc-04 계단실 — 사흘 전의 자리 [begins=3 ends=0 holding=23]
    + f-gt-stairwell (gt) [state]: 계단실 아래에는 사흘 전의 사고 흔적이 남아 있다
    + f-minu-knows (minu) [state]: 민우는 연이 원래 오늘 여기 있어선 안 된다는 것을 어렴풋이 안다 — 그러나 입 밖으로 내지 못한다
    + f-yeon-empty (yeon) [state]: 연은 자신이 무언가 비어 있다고 느끼지만 그것이 무엇인지 모른다
## sc-05 — sc-05 3반 교실 — 책상에서 교무실 열쇠 [begins=3 ends=0 holding=26]
    + f-done-staffkey (gt) [state]: 교무실 열쇠를 확보해 곁가지가 완수된다
    + f-have-staffkey (gt) [state]: 세리가 3반 교실 책상에서 교무실 열쇠를 손에 넣는다
    + f-seri-leads (seri) [state]: 세리는 앞장서 길을 정하고 열쇠를 챙긴다
## sc-06 — sc-06 교무실 — 마스터 열쇠 [begins=2 ends=0 holding=28]
    + f-gt-staffroom-master (gt) [state]: 교무실 책상 안쪽에서 마스터 열쇠가 나온다
    + f-have-masterkey (gt) [state]: 세리가 마스터 열쇠를 손에 넣는다
## sc-07 — sc-07 자료실 — 출석 기록 [begins=2 ends=0 holding=30]
    + f-give-truth (gt) [state via sc-07/e-record-book]: 자료실의 기록이 하나의 물음을 연다 — 복도를 도는 그것은 무엇인가
    + f-pursue-truth (gt) [state]: 하준이 그것의 정체를 밝히려 한다
## sc-08 — sc-08 자료실 — 젖어 번진 한 칸 [begins=3 ends=0 holding=33]
    + f-gt-record-examined (gt) [state]: 자료실 출석부의 한 칸이 젖어 번져 읽히지 않는다
    + f-secret-geot (gt) [withhold first_at=tg-14 via sc-08/e-record-book]: 그것은 사흘 전 계단에서 떨어져 죽은 관리인의 남은 것이며, 그것이 찾는 것은 연이다
    + f-secret-yeon (gt) [withhold first_at=tg-14 via sc-08/e-record-book]: 연은 이미 이 건물의 산 사람이 아니다 — 사흘 전 같은 밤 사라졌고, 지금의 연은 남은 셋의 기억이 붙든 잔상이다
## sc-09 — sc-09 긴 복도 — 그것이 좁혀 온다 [begins=2 ends=0 holding=35]
    + f-hajun-dread (hajun) [state]: 하준은 문마다 잠긴 것을 확인하며 목덜미가 서늘해진다
    + f-minu-fear (minu) [state]: 민우는 겁에 질려 무리에서 뒤처진다
## sc-10 — sc-10 복도 — 세리가 믿기 시작한다 [begins=2 ends=1 holding=36]
    + f-gt-push-exit (gt) [state]: 넷은 마스터 열쇠를 들고 정문으로 향한다
    + f-seri-believes (seri) [state]: 세리는 이제 그것이 실재함을 믿는다
    - f-seri-skeptic (seri): superseded by f-seri-believes
## sc-11 — sc-11 로비 — 정문 앞으로 [begins=2 ends=1 holding=37]
    + f-geot-track-3 (geot) [state]: 그것은 넷이 정문 앞 로비에 이르렀음을 안다
    + f-gt-at-door (gt) [state]: 넷이 로비를 가로질러 정문 앞에 선다
    - f-geot-track-2 (geot): superseded by f-geot-track-3
## sc-12 — sc-12 정문 앞 — 그것이 나타난다 (갈림) [begins=1 ends=0 holding=38]
    + f-gt-it-appears (gt) [state]: 마스터 열쇠가 정문에 닿는 순간 그것이 나타난다 — 하준은 택해야 한다
## tg-13 — tg-13 함께 — 하준이 연을 붙잡는다 [begins=1 ends=0 holding=39]
    + f-hajun-choose-tg (hajun) [state]: 하준은 연의 손목을 붙잡고 물러서지 않기로 한다
## tg-14 — tg-14 함께 — 진실이 착지한다 [begins=2 ends=0 holding=41]
    + f-done-truth-tg (gt) [state]: 진실이 착지한다 — 하준은 그것의 정체를 온전히 안다, 그리고 선택 곁가지가 완수된다
    + f-seri-pulls-tg (seri) [state]: 세리는 하준을 문 쪽으로 끌어당기며 서두른다
## tg-15 — tg-15 함께 — 연이 남기를 택한다 [begins=4 ends=0 holding=45]
    + f-done-save-yeon-tg (gt) [state]: 연이 남기를 택하고, 하준은 작별로 그 몫을 갚는다 — 곁가지가 완수된다
    + f-gt-yeon-stays-tg (gt) [state]: 연은 온실에 남고, 그것은 찾던 것을 되찾아 멎는다
    + f-minu-watches-tg (minu) [state]: 민우는 연을 마지막으로 한 번 돌아본다
    + f-yeon-chooses-tg (yeon) [state]: 연은 스스로 남기를 택한다
## tg-16 — tg-16 함께 — 셋이 빗속으로 [begins=2 ends=0 holding=47]
    + f-done-escape-tg (gt) [state]: 하준·세리·민우 셋이 정문을 열고 빗속으로 나선다 — 탈출이 완수된다
    + f-gt-three-out-tg (gt) [state]: 하준·세리·민우 셋이 빗속으로 걸어 나간다
