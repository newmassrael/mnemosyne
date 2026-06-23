#!/usr/bin/env bash
# Build the Mnemosyne narrative store for "The Tide That Counts" from scratch.
#
# Idempotent: wipes the sidecar and replays every typed mutate primitive, so
# the store is fully reproducible from this script + MANUSCRIPT.md. Run from
# inside this directory (CWD-relative config discovery finds the novel's
# mnemosyne.toml, never the repo-root self-spec workspace).
set -euo pipefail

MN="${MN:-$(git -C "$(dirname "$0")/../.." rev-parse --show-toplevel)/target/release/mnemosyne-cli}"
cd "$(dirname "$0")"
rm -f .atomic/tide.atomic.json
Q() { "$MN" "$@" >/dev/null; }   # quiet helper
mkdir -p .atomic/tmp
# R <section> <one-line craft note>: route a rationale bullet through a file
R() { printf -- '- %s\n' "$2" > .atomic/tmp/r.txt; Q set-section-rationale --section "$1" --bullets-file .atomic/tmp/r.txt; }

echo "1/8  scenes (discourse structure — one section per scene)"
Q add-section --section s01-arrival       --parent-doc MANUSCRIPT --title "조수가 차오르기 전에"
Q add-section --section s02-the-tally     --parent-doc MANUSCRIPT --title "문설주의 금"
Q add-section --section s03-the-funeral   --parent-doc MANUSCRIPT --title "장례"
Q add-section --section s04-the-empty-beds --parent-doc MANUSCRIPT --title "빈 방들"
Q add-section --section s05-the-letter    --parent-doc MANUSCRIPT --title "할머니의 편지"
Q add-section --section s06-the-shaman    --parent-doc MANUSCRIPT --title "무선의 굿"
Q add-section --section s07-the-lowest-tide --parent-doc MANUSCRIPT --title "그믐 사리"
Q add-section --section s08-the-count     --parent-doc MANUSCRIPT --title "셈"

echo "     scene loglines + craft notes (intent = logline, rationale = why-this-scene)"
Q set-section-intent --section s01-arrival --intent "외지인 지운이 물때에만 열리는 갯들을 건너 무월도에 든다. 소금, 노인뿐인 마을, 밤의 부름."
R s01-arrival "공포의 규칙(대답하지 마라)을 경고로 먼저 심어 둔다 — 의미는 7장에 가서야 회수."
Q set-section-intent --section s02-the-tally --intent "문설주의 칼금과 당집 종소리. 종이 시간이 아니라 '남은 수'를 센다는 첫 단서."
R s02-the-tally "체호프의 총: 셈/종/금 세 가지 소도구를 한 장에 배치하고 의미는 보류(withhold)."
Q set-section-intent --section s03-the-funeral --intent "할머니 장례가 비정상적으로 빠르다. 무당이 시신의 눈·입을 소금으로 막아 '셈에 다시 끼지' 않게 한다."
R s03-the-funeral "지운 프레임의 오인(자연사)을 명시적으로 세워, 5장의 진실 폭로로 supersede할 토대."
Q set-section-intent --section s04-the-empty-beds --intent "아이 없는 마을, 정성껏 보존된 빈 아이 방 셋. 흉년·붉은 물 회상. 종소리가 하루 만에 줄어든다."
R s04-the-empty-beds "독자에게 '셈의 대상은 사람'을 imply. 빈 방=과거의 셈을 후반 회수로 연결."
Q set-section-intent --section s05-the-letter --intent "할머니의 숨긴 편지: 삼십 년 전의 계약, 사리마다 이름 하나. 지운을 살리려 육지로 보냈다는 고백."
R s05-the-letter "핵심 배경을 imply 단계로 공개. 지운 프레임을 진실로 전환(supersede). 이판수가 편지를 태운다."
Q set-section-intent --section s06-the-shaman --intent "무당 무선이 대체 규칙을 알려 준다: 부르는 이름에 다른 사람이 대답하면 그이는 그 사람을 데려간다."
R s06-the-shaman "클라이맥스 트릭(대체)을 명시적 규칙으로 심는다 — 8장의 이름 바꿔치기 회수를 위한 셋업."
Q set-section-intent --section s07-the-lowest-tide --intent "그믐 사리. 지운의 이름이 태어나기 전부터 갯벌에 약속돼 있었음이 드러난다. 물어미가 부른다."
R s07-the-lowest-tide "1장의 경고를 문자 그대로 회수(물 돌아설 때/대답). state 직전의 최대 긴장."
Q set-section-intent --section s08-the-count --intent "지운이 자기 이름 대신 계약의 시초인 이판수의 이름을 외친다. 셈이 바뀌고 길이 바다로 돌아간다."
R s08-the-count "모든 복선의 회수 지점. 진실의 state-공개. 새 동그라미가 이판수 문설주에 그어진다."

echo "2/8  epistemic frames (who-believes-what — the dramatic-irony axes)"
Q add-frame --frame ground-truth   --description "무월도에서 실제로 참인 것 — 계약과 셈의 진실."
Q add-frame --frame frame-jiun     --description "서지운이 매 순간 아는/믿는 것. 1장의 무지에서 진실로 이동."
Q add-frame --frame frame-village  --description "마을 사람들의 공유 믿음 — 물어미는 우리를 '지켜 준다'는 신앙적 위안."
Q add-frame --frame frame-geumrye  --description "할머니 금례가 숨긴 앎 — 계약을 깨려다 먼저 셈에 든 자의 시점."
Q add-frame --frame frame-munseon  --description "무당 무선의 앎 — 셈은 미움이 아니라 대체로만 비껴갈 수 있다."

echo "3/8  entities (characters / the being / places / objects)"
Q add-entity --entity ent-jiun     --kind character --description "서지운. 일곱 살에 떠나 스물일곱에 할머니 장례로 귀향한 마지막 핏줄."
Q add-entity --entity ent-geumrye  --kind character --description "금례. 지운의 할머니. 셈을 깨려고 지운을 육지로 보냈다."
Q add-entity --entity ent-pansu    --kind character --description "이판수. 이장이자 삼십 년 전 계약을 시작한 장본인."
Q add-entity --entity ent-munseon  --kind character --description "무선. 마을 무당. 시신을 소금으로 봉하고 대체 규칙을 안다."
Q add-entity --entity ent-mother   --kind being     --description "물어미. 갯벌이 낳은 어미. 사리마다 부른 이름 하나를 셈해 데려간다."
Q add-entity --entity ent-island   --kind place     --description "무월도. 물때에만 갯들로 육지와 이어지는 조간대 섬."
Q add-entity --entity ent-flat     --kind place     --description "갯벌/물목. 물이 돌아서는 한 순간 셈이 이뤄지는 경계."
Q add-entity --entity ent-bell     --kind object    --description "당집 종. 시간이 아니라 '남은 수'를 셈해 운다."
Q add-entity --entity ent-tally    --kind object    --description "문설주의 칼금. 셈의 장부. 대신 간 집엔 동그라미가 더해진다."
Q add-entity --entity ent-letter   --kind object    --description "할머니의 기름종이 편지. 계약과 지운의 약속된 이름을 담은 고백."

echo "4/8  predicates (typed-claim vocabulary — load-bearing for disclosure gate)"
Q add-predicate --predicate pred-bound-by    --object-kind scalar --description "주어가 어떤 계약/조건에 묶여 있다."
Q add-predicate --predicate pred-collects     --object-kind scalar --description "주어가 무엇을 셈해 거둔다."
Q add-predicate --predicate pred-promised-to  --object-kind entity --description "주어(이름)가 어떤 존재에게 약속되어 있다."
Q add-predicate --predicate pred-defied       --object-kind scalar --description "주어가 무엇을 거역했다."

echo "     facts — Chekhov setups (foreshadow) with payoff expectations"
Q add-fact --fact f-warning-flat --frame frame-village --canon-from s01-arrival \
  --entities ent-flat,ent-island --evidence s01-arrival \
  --claim "물 돌아설 때 갯벌에 있지 마라. 누가 부르거든 대답하지 마라 — 마을의 첫 가르침." \
  --payoff-expectation expected
Q add-fact --fact f-tally-marks --frame ground-truth --canon-from s02-the-tally \
  --entities ent-tally --evidence s02-the-tally \
  --claim "집집 문설주마다 칼금이 빼곡히 늘어 가고, 대신 간 집엔 동그라미가 더해진다." \
  --payoff-expectation expected
Q add-fact --fact f-bell-counts --frame ground-truth --canon-from s02-the-tally \
  --entities ent-bell --evidence s02-the-tally \
  --claim "당집 종은 시각이 아니라 내림수를 울린다 — 어제 일곱, 오늘 여섯." \
  --payoff-expectation expected
Q add-fact --fact f-empty-beds --frame ground-truth --canon-from s04-the-empty-beds \
  --entities ent-island --evidence s04-the-empty-beds \
  --claim "보존된 빈 아이 방 셋. 섬에 아이가 하나도 남지 않았다." \
  --payoff-expectation expected
Q add-fact --fact f-letter-hidden --frame frame-geumrye --canon-from s05-the-letter \
  --entities ent-letter,ent-jiun --evidence s05-the-letter \
  --claim "장판 밑 할머니의 편지가 지운에게 결코 돌아오지 말라 경고한다." \
  --payoff-expectation expected
Q add-fact --fact f-substitution-rule --frame frame-munseon --canon-from s06-the-shaman \
  --entities ent-mother,ent-munseon --evidence s06-the-shaman \
  --claim "부르는 이름에 다른 사람이 대답하면, 물어미는 그 대답한 사람을 데려간다." \
  --payoff-expectation expected

echo "5/8  facts — ground-truth reveal chain (typed; enrolled in reader telling)"
Q add-fact --fact f-pact-exists --frame ground-truth --canon-from s01-arrival \
  --entities ent-island,ent-mother --evidence s01-arrival \
  --claim "무월도는 삼십 년 전 흉년에 물어미와 맺은 계약으로만 존속한다." \
  --typed-subject ent-island --typed-predicate pred-bound-by \
  --typed-object-value "물어미의 셈 — 한 사리에 이름 하나" \
  --payoff-expectation expected
Q add-fact --fact f-count-is-people --frame ground-truth --canon-from s04-the-empty-beds \
  --entities ent-mother --evidence s04-the-empty-beds \
  --claim "물어미가 거두는 셈은 사람이다 — 마을이 부른 이름 하나씩." \
  --typed-subject ent-mother --typed-predicate pred-collects \
  --typed-object-value "사리마다 약속된 이름 하나" \
  --pays-off f-tally-marks
Q add-fact --fact f-geumrye-defied --frame ground-truth --canon-from s05-the-letter \
  --entities ent-geumrye,ent-jiun --evidence s05-the-letter \
  --claim "금례는 지운을 육지로 보내 셈에서 빼려 함으로써 계약을 거역했다." \
  --typed-subject ent-geumrye --typed-predicate pred-defied \
  --typed-object-value "계약 — 지운을 육지로 보내어"
Q add-fact --fact f-jiun-promised --frame ground-truth --canon-from s07-the-lowest-tide \
  --entities ent-jiun,ent-mother --evidence s07-the-lowest-tide,s05-the-letter \
  --claim "지운의 이름은 그가 태어나기도 전에 갯벌에 약속돼 있었다." \
  --typed-subject ent-jiun --typed-predicate pred-promised-to \
  --typed-object-entity ent-mother \
  --pays-off f-letter-hidden --payoff-expectation expected

echo "     facts — payoffs (회수)"
Q add-fact --fact f-mother-takes-at-turn --frame ground-truth --canon-from s07-the-lowest-tide \
  --entities ent-mother,ent-flat --evidence s07-the-lowest-tide \
  --claim "물이 돌아서는 한 순간, 물어미는 부른 이름에 대답한 자를 데려간다." \
  --pays-off f-warning-flat,f-substitution-rule
Q add-fact --fact f-children-were-counts --frame ground-truth --canon-from s08-the-count \
  --entities ent-island,ent-mother --evidence s08-the-count \
  --claim "사라진 아이들은 흉년기에 셈으로 치러진 이름들이었다." \
  --pays-off f-empty-beds
Q add-fact --fact f-bell-silent --frame ground-truth --canon-from s08-the-count \
  --entities ent-bell --evidence s08-the-count \
  --claim "셈이 치러진 이튿날 종은 울지 않고 새 금도 그어지지 않는다." \
  --pays-off f-bell-counts
Q add-fact --fact f-jiun-names-pansu --frame ground-truth --canon-from s08-the-count \
  --entities ent-jiun,ent-pansu,ent-mother --evidence s08-the-count \
  --claim "지운은 부름에 자기 대신 계약의 시초 이판수의 이름으로 대답한다." \
  --pays-off f-substitution-rule,f-jiun-promised
Q add-fact --fact f-pansu-taken --frame ground-truth --canon-from s08-the-count \
  --entities ent-pansu,ent-mother,ent-tally --evidence s08-the-count \
  --claim "들물이 이판수를 데려가고, 그의 문설주에 새 동그라미가 그어진다." \
  --pays-off f-substitution-rule,f-pact-exists

echo "     facts — belief divergence (dramatic irony) + supersession"
Q add-fact --fact f-village-believes-protect --frame frame-village --canon-from s02-the-tally \
  --entities ent-mother,ent-island --evidence s02-the-tally \
  --claim "마을은 물어미가 자신들을 '지켜 준다'고 믿으며 셈을 신앙으로 견딘다." \
  --conflicts f-count-is-people
Q add-fact --fact f-jiun-thinks-natural --frame frame-jiun --canon-from s03-the-funeral \
  --entities ent-geumrye --evidence s03-the-funeral \
  --claim "지운은 할머니가 노환으로 자연사했다고 여긴다."
Q add-fact --fact f-jiun-learns-truth --frame frame-jiun --canon-from s05-the-letter \
  --entities ent-geumrye,ent-mother --evidence s05-the-letter \
  --claim "지운은 할머니가 계약을 거역해 먼저 셈에 들었음을 편지로 알게 된다." \
  --supersedes f-jiun-thinks-natural

echo "6/8  disclosure plan — the reader telling (default: withhold the pact)"
Q add-disclosure-plan --telling reader --default-mode withhold \
  --description "독자에게 계약의 진실을 어떻게/언제 흘릴지: 보류→암시→명시."
Q set-disclosure --telling reader --fact f-pact-exists   --mode hint  --first-at "main=s02-the-tally"      --surface s02-the-tally
Q set-disclosure --telling reader --fact f-count-is-people --mode imply --first-at "main=s04-the-empty-beds" --surface s04-the-empty-beds
Q set-disclosure --telling reader --fact f-geumrye-defied --mode imply --first-at "main=s05-the-letter"     --surface s05-the-letter
Q set-disclosure --telling reader --fact f-jiun-promised  --mode state --first-at "main=s08-the-count"      --surface s08-the-count

echo "7/8  changelog — authoring audit trail (the 'writers' room' rounds)"
mkdir -p .atomic/tmp
printf -- '- 8개 장면 섹션 + 로그라인/장작법 노트\n- 5개 인식 프레임(ground-truth/지운/마을/금례/무선)\n- 10개 엔티티(인물/물어미/장소/소도구)\n' > .atomic/tmp/c1.txt
printf -- '- query --list-sections로 8장면 확인\n- 프레임/엔티티 등록 확인\n' > .atomic/tmp/v1.txt
printf -- '- 다음 라운드: 사실 베이스 + 복선-회수 그래프\n' > .atomic/tmp/k1.txt
Q append-changelog-entry --entry-id "Round 1" \
  --decision "무대 세팅: 8장면 골격 + 프레임/엔티티 등기. 산문은 MANUSCRIPT.md, 구조는 스토어." \
  --changes-file .atomic/tmp/c1.txt --verification-file .atomic/tmp/v1.txt \
  --impact s01-arrival,s02-the-tally,s08-the-count --carry-file .atomic/tmp/k1.txt

printf -- '- 진실 체인(계약/셈=사람/지운 약속/금례 거역) typed 사실로\n- 체호프 셋업 6종 + 회수 5종 + pays-off 링크\n- 마을 신앙 vs 진실 conflict, 지운 오인→진실 supersede\n' > .atomic/tmp/c2.txt
printf -- '- report-payoff-coverage로 셋업↔회수 정합 확인\n- validate-continuity 통과\n' > .atomic/tmp/v2.txt
printf -- '- 다음: reader telling 공개 일정(보류→암시→명시) 배선\n' > .atomic/tmp/k2.txt
Q append-changelog-entry --entry-id "Round 2" \
  --decision "사실 베이스 + 복선/회수 + 프레임 분기(극적 아이러니)와 supersession 부설." \
  --changes-file .atomic/tmp/c2.txt --verification-file .atomic/tmp/v2.txt \
  --impact s04-the-empty-beds,s05-the-letter,s06-the-shaman,s07-the-lowest-tide \
  --carry-file .atomic/tmp/k2.txt

printf -- '- reader telling 추가(default withhold)\n- 4개 핵심 진실에 hint/imply/state 공개 시점 지정\n' > .atomic/tmp/c3.txt
printf -- '- validate-disclosure-leak로 조기 누설 0 확인\n- report-disclosure-coverage로 공개 곡선 확인\n' > .atomic/tmp/v3.txt
printf -- '- 후속: 분기(다른 결말 world-line) 실험 여지\n' > .atomic/tmp/k3.txt
Q append-changelog-entry --entry-id "Round 3" \
  --decision "독자 공개 일정 확정: 계약 진실을 보류→암시→명시로 통제(슬로우번 공포의 핵심)." \
  --changes-file .atomic/tmp/c3.txt --verification-file .atomic/tmp/v3.txt \
  --impact s02-the-tally,s05-the-letter,s08-the-count --carry-file .atomic/tmp/k3.txt
rm -rf .atomic/tmp

echo "8/8  done — store at .atomic/tide.atomic.json"
