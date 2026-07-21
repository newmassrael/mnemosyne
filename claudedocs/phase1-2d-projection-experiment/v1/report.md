# Report — 2d-projection-experiment/v1 (executed 2026-07-21, R750)

One blind-authored fact store, projected by two independent blind renders onto a
**visual-novel** axis and an **Ao-Oni-style 2D tsukuru** axis (both
dialogue/monologue-centric), to test the thesis *genre difference = projection
thickness, not substrate difference*. Orchestrated per `runbook.md` under the
R469 contamination bound (this lineage designed only; blind subagents authored /
rendered each axis / re-extracted each / judged). Pins verified by the
orchestrator on a FRESH rebuild from the author's JSON, not the author's
self-report.

## Verdict (one line)

**The store is render-neutral for structure / map / quest / fidelity / game-feel
(PIN-1/2/3 hold; both axes fidelity-clean; judged real games of their genre,
VN 4 / tsukuru 5, same night) — but PIN-4 PROJECTION INVARIANCE FAILS on the
DISCLOSURE-TIMING axis: the pre-registered disclosure-trigger pull landed,
concretely and triangulated five ways.**

## Pins (orchestrator-verified on the fresh rebuild)

| Pin | Verdict | Evidence |
|---|---|---|
| PIN-1 authoring-clean | **HOLD** | fresh rebuild == author store (byte-identical registries + edge_guards); continuity 0/0, fork-tree 2 worlds @sc-12, timeline-gaps 0/0 both roads, manuscript unplaced/undecidable 0 both, disclosure 56/3/0 |
| PIN-2 spatial+quest | **HOLD** | 7 kind:place fully CONNECTED (6 adjacency) + 2 edge-guards (staffkey/masterkey conditions) + key chain order-real (staffkey sc-05 < masterkey sc-06 < escape post-fork) + geot knowledge gap present as data (3 geot-frame `at` facts locate the party; 0 party-frame facts locate geot) + 4 kind:quest full contract + q-escape requires q-staffkey + per-road divergence (q-save-yeon / q-truth done on `together`, open on `alone`) |
| PIN-3 map/surface | **HOLD** | report-playable-world 3 worlds unplaced/undecidable/undeclared-adj = 0; all 7 override surfaces (4 quest-givers + 3 secrets) resolve to a MapLocator |
| PIN-4 invariance | **FAILS (disclosure axis)** | fidelity clean both axes (off_path 0, reached_terminal true); but the two axes DIVERGE on disclosure — see below |

## PIN-4: the divergence (the pull, made concrete)

The substrate expresses disclosure as a **single ordinal** (`first_at
{road=scene}`) plus a **single surface** (`{scene, object}`). The linear **VN**
render honors this trivially — reading order == disclosure order, and there is no
spatial gap between an object and its reveal. The non-linear **walked tsukuru**
render could NOT honor it without straining, in three concrete places its own
(blind) render-log names:

1. **Railroad** — `f-secret-yeon` / `f-secret-geot` are surfaced at
   `sc-08/e-record-book` (the archive) but `first_at=tg-14` (the front door, a
   spatially distant room). "그 사물을 조사하는 순간 드러나라는 규칙을 비선형
   공간에서 그대로 지킬 수가 없었다." Resolution = a **page-carry railroad**: the
   player is half-led to tear out the unreadable page at sc-08 and carry it to
   tg-14. "이 한 번의 가벼운 레일로딩이 없으면 'sc-08 사물 ↔ tg-14 리빌'의 못이
   끊어진다."
2. **Leak** — `f-secret-exit` ("정문은 끝이 아니다") is withheld on the `together`
   road (its only `first_at` is `alone=al-14`), but its `surface` pins it to
   `sc-12/e-exit`. The walked render, forced to seat a beyond-the-door truth
   *before* the door, voiced it at sc-12 as a **premonition** ("왜 지금 아는지
   모른 채 그냥 안다"). This is a real premature disclosure on the `together`
   telling; the VN render simply omitted it. "via가 sc-12/e-exit로 못 박혀 있어
   그 자리에 앉히는 것 외의 선택지가 없었다."
3. **Content-split** — `f-geot-seeks` ("그것은 오직 연을 찾는다", stateable from
   sc-03) semantically overlaps the withheld `f-secret-geot` ("그것이 찾는 것은
   연이다", withheld to tg-14). The renderer had to hand-split disclosure by
   layer (behaviour stated / identity sealed) — a strain the single-mode
   disclosure model does not carry.

The VN (linear) render needed NONE of these. This is the pre-registered pull:
**disclosure timing needs the condition / first-reached vocabulary the ACCESS
axis already has** (edge-guards: an adjacency edge REQUIRES condition facts,
K-of-N — R717/720/722/723). With it, a walked render could seat a reveal on a
spatial trigger (examine object O, first-of-a-set) and express "withheld on this
road" without railroading or leaking — the same generalization a real-time 3D
consumer would need, surfaced early in 2D exactly as PRED_pull predicted (and a
touch STRONGER than the predicted "mild": a real leak, not just awkwardness).

## Triangulation (five independent confirmations of the leak)

The `f-secret-exit` premature disclosure at sc-12 was surfaced independently by:

1. **Deterministic scan** (orchestrator): f-080 states the exit-secret content at
   sc-12 on `together`; the VN re-extraction has 0 exit-secret statements anywhere.
2. **Blind tsukuru render-log**: names case 2 above as a forced seating.
3. **Blind judge-1**: "sc-12 ... 근거 없는 저자 지식이 하준에게 얹혀 ... 레일성을
   특히 드러낸다."
4. **Blind judge-2**: docked tsukuru knowledge-realism to 4 for "sc-12 ... '아는
   만큼만' 규칙의 작은 흔들림."
5. **Blind judge-3**: docked same-night to 4 because tsukuru "이 문은 끝이
   아니다"(sc-12) adds an ending thread VN does not.

## Judges (game-feel — the open question)

| Axis | J1 | J2 | J3 | majority |
|---|---|---|---|---|
| VN game-feel | 4 | 4 | 4 | **4** |
| tsukuru game-feel | 4 | 5 | 5 | **5** |
| same-night | 5 | 5 | 4 | **≥4** |
| genre-fit | tsukuru | tsukuru | tsukuru | **tsukuru (unanimous)** |

Both axes read as real games of their genre (the thesis' game-feel half holds).
The VN's recurring dock was the mid-game key-collection (sc-05/06) reading thin —
a genre-fit signal (a walked form makes the same beats tactile; the judges
unanimously called the content tsukuru-fit). No judge called either an
"outline in game clothes".

## Harness / gate findings (not downplayed)

The leak GATE (`validate-disclosure-leak`) did NOT produce the leak verdict — two
real limits surfaced, both worth recording:

1. **Vacuity (my harness flaw).** The gate reported `vocabulary_shared=0` and
   FAILED ITS OWN non-vacuity guard ("the gate is blind, not clean") because my
   `vocab.md` omitted the `hidden_nature` token literals (afterimage / revenant /
   no-exit) and did not steer ground-truth to the `gt` frame — so both blind
   extractors invented foreign tokens/frames that could not match. The gate's
   built-in guard catching this is a POSITIVE for the gate design. The leak
   verdict here came from a deterministic manual scan (a derived oracle, proven
   non-vacuous by finding the real divergence), not the vacuous gate.
2. **Typed-only scope.** The actual leak (f-080) is UNTYPED prose; the leak gate
   checks only typed `hidden_nature` tuples, so even a non-vacuous run would MISS
   it. A prose render that states a secret without the typed structure evades the
   gate. The manual keyword scan covered this gap.

Follow-ons: `vocab.md` should carry the token literals + gt-frame guidance for a
non-vacuous machine gate; the leak gate's typed-only scope should be documented
(or the re-extraction required to type secrets). The fidelity gate, by contrast,
worked non-vacuously and clean on both axes.

## Decision rule applied

Pre-committed rule `pin4_invariance_fails`: "the store is NOT fully render-neutral
for this genre pair ... the most likely cause (pre-registered) is the
disclosure-trigger asymmetry ... a real substrate finding worth a BUILD round,
NOT a refutation of the seam." **This is the outcome.** The map_locator seam and
the fact/quest/branch substrate project genre-agnostically; the DISCLOSURE-TIMING
primitive is where genre thickness bites, exactly as the experiment was designed
to find out.

## Honest caveats

- n=1 (1 premise / 1 author / 1 VN render / 1 tsukuru render / 2 extractions /
  3 judges) — one instance, not a distribution.
- Declarative projection only; no played stateful lifecycle (SCE/pinion line).
- The small tight map still bit (leak + railroad + split) — an open map would
  bite harder; the pull's magnitude at scale is a larger follow-on.
- The leak verdict rests on a deterministic manual scan because the machine gate
  went vacuous (harness flaw, above); the finding is triangulated 5 ways, but a
  fixed-vocab re-run for a machine-confirmed typed-secret verdict remains a
  follow-on.

## Artifacts

Design: premise.md, author-brief.md, render-vn-brief.md, render-tsukuru-brief.md,
extractor-brief.md, judge-brief.md, manifest.json, runbook.md.
Run: run/author/ (author-log.md + JSON), run/manuscripts/world-together.md
(the shared outline), run/render-vn/ + run/render-tsukuru/ (world-together.md +
render-log.md each), run/extract-vn/ + run/extract-tsukuru/ (re-extractions +
extract-log.md), run/judges/judge-{1,2,3}.md. Built stores (*.atomic.json) are
gitignored scratch.
