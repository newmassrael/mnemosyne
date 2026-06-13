# disclosure-craft-experiment/v1 — execution report

**Status at this point in the file: PRE-REVEAL.** Everything above the
`## REVEAL` divider was written before the blind labels were broken. The
post-reveal grading is appended below that divider at step 8.

## Integrity

- Manifest: `disclosure-manifest.json`, sha256
  `7550633362cc1206778354f49aa64cff01323a3c23ec8d6bf07329b40db1880c`.
- This sha is the value pinned in the R503 ledger entry
  ("Round 503 — disclosure-craft-experiment/v1 protocol…") BEFORE any
  execution session existed. On-disk manifest == pinned manifest: the
  protocol executed is the pinned one.
- Orchestrator ran the harness/gate commands; the render author(s), the
  extractor, and the judges were each spawned as separate blind subagents
  (fresh context) and received only their brief block + the §1 firewall —
  never the manifest, the runbook, the hypothesis, or the other arm.
- Honesty bound (recorded): the orchestrator may know the A/B mapping (it
  is needed to copy the labeled files and to attribute the PINs at reveal);
  it never wrote the mapping into any sub-prompt.

## Provenance of reused inputs (the low-touch design)

- `run/plain/story.md` — the R500 **plain** arm, REUSED verbatim from
  `phase1-factsfirst-craft-experiment/run/plain/story.md`.
  sha256 `ad74df13f2ab0d3c6ac8e6cd6577b17937f81dec27bb8e29217ba148043a208f`.
- `run/disclosure/ff.atomic.json` — the R500 facts-first scratch store,
  REUSED from `phase1-factsfirst-craft-experiment/run/factsfirst/ff.atomic.json`.
  sha256 `a27963f8fa7c8a82e1ffe08c1c09020b86b22fc1d868183cbb138b5d01c544a1`.
  The disclosure arm renders prose FROM this store; it never re-authors facts.
- Honesty bounds inherited (manifest `honesty_bounds_recorded`): plain is a
  cross-run baseline (not a same-run twin); the disclosure arm inherits R500's
  fact coverage/gaps; premise reuse trades R499 novelty-control for
  render-isolation (judge-clean because judges are fresh); this lineage wrote
  the pinned disclosure brief (design-stage selection bias the sha-pin does
  not cover).

## The SEAL (recorded PRE-REVEAL)

```
experiment : disclosure-craft
note       : reveal at S8
arms       : plain, disclosure  ->  shuffled to labels A, B
seal sha256: 5496b834d1c220e2427bd087f1bcbe7e2deab065dadb103b383961aa8a242fe4
```

`verify-seal` is run at step 8 against this recorded value before the map is
read for unblinding.

## Disclosure render economics (PRED-4, partial — friction leg)

- Render sessions: **3** sequential blind subagents (the store was pre-built,
  so this is render-not-author). SendMessage was unavailable in this harness,
  so each continuation was a fresh blind subagent that read its own prior
  `story.md` for voice continuity.
  - S1 = spine sc-01..08 + Fork-1 CHOICE — ~60.6k subagent tokens.
  - S2 = confront limb sc-09a..sc-16 (+ Fork-2 CHOICE) + audit middle
    sc-09b..sc-12b — ~74.7k subagent tokens.
  - S3 = endings (reveal sc-17r..19r / burn sc-17w..18w / audit sc-17b..20b)
    + world-line map — ~74.4k subagent tokens.
  - Total render ~209.7k subagent tokens.
- Hand-brief friction (the rung-2-substrate pull signal): all three sessions
  reported the brief as **followable with low workflow-friction** but
  demanding **craft attention at the irony/withhold seams** — specifically
  (S1) turning S1/S2 propositions into witnessed objects without narrator
  assertion; (S2) holding Pike's false belief in his own close-third (sc-12a)
  one scene AFTER the contradicting evidence was shown to Hale (sc-11a);
  (S3) paying off four planted objects in one close-third across the audit
  endings without tipping into omniscient summary. None reported fighting the
  brief. Read as a **moderate** pull signal: the `canon_from` coordinates in
  the store already carried the disclosure timing, so the author did not need
  a separate stored disclosure plan to know when to reveal — but the per-scene
  "what may surface here, in whose frame" bookkeeping was manual.

## Quota gate (validity_conditions.quota) — PASS

Disclosure deliverable `run/disclosure/story.md` (= label A):
- scenes: **29** (>= 24 required)
- fork points: **2** (fork-1 @ sc-08 confront/audit; fork-2 @ sc-16 reveal/burn)
- world-lines: **3** (CONFRONT->REVEAL, CONFRONT->BURN, QUIET-AUDIT)
- endings: **3** (sc-19r, sc-18w, sc-20b)
- epistemic frames: source store carries **3** (gt/hale/pike); prose renders
  in focal close-third per the brief (gt is the gate's, never the narrator's)
- 6 required setups: planted in the spine (night-log wrong hand sc-03; blank
  plate sc-04; clock 3:14 sc-05; drained account + unsent letter sc-06; spare
  key sc-07; account discrepancy sc-08), payoffs withheld to their scenes.
- word count: 9,417 (plain baseline ~10k; comparable scales -> preference
  leg is valid).

Quota met -> all deterministic legs + the preference leg run.

## PRE-PINNED WITHHOLD LIST (PIN-2 targets) — recorded PRE-EXTRACTION

Per runbook step 4 / manifest `premature_leak_metric`: the WITHHELD
ground-truth-frame solution conclusions and the earliest scene at which each
may legitimately surface, enumerated from the SOURCE store
`run/disclosure/ff.atomic.json` before any re-extraction. PIN-2 asserts that
in the disclosure story's BLIND re-extraction, each conclusion's earliest
canon coordinate is **at-or-after** its reveal scene. Any earlier = a
premature LEAK (cited scene-id + quote leak-vs-noise audit).

**Shared-spine invariant (the deterministic backbone):** scenes **sc-01..sc-08**
are strictly pre-reveal for ALL three worlds (they precede both forks). Any
of W1/W2/W3 stated as a narrator/established conclusion anywhere in sc-01..08
is an unambiguous premature leak, independent of world.

| # | Withheld solution conclusion (gt frame) | gt fact(s) | Reveal scene by world (earliest legitimate) |
|---|---|---|---|
| **W1** | **Who climbed the stair = Onslow Pike** (the intruder who entered the dome the night Crane died) | `gt-climber-pike`; pays off via `gt-reveal-names-pike`, `gt-hale-sees-key`, `gt-audit-certifies` | CONFRONT->REVEAL: **sc-17r** (Junia names Pike). QUIET-AUDIT: **sc-11b** (Hale witnesses Pike draw the kept key + enter), certified sc-20b. CONFRONT->BURN: **never named** (gt-burn-certifies cannot name the climber). Earliest across worlds = **sc-11b**. |
| **W2** | **True cause of death = accidental fall** (Crane startled on the dark stair, already in private grief over his failing eyes), **NOT murder**; the clock at 3:14 = the **instant of the fall (jolt)**, not a set/planted alibi | `gt-clock-jolt`, `gt-clock-read-true`, `gt-audit-clock-pier`, `gt-letter-resolves`, `gt-audit-letter-read`, `gt-reveal-certifies`, `gt-audit-certifies`, `gt-burn-certifies` | CONFRONT->REVEAL: begins **sc-11a** (no one drove him) + **sc-15** (clock jolted not set), certified **sc-19r**. QUIET-AUDIT: **sc-18b** (clock/pier) + **sc-19b** (letter), certified **sc-20b**. CONFRONT->BURN: **sc-18w** (accidental-and-unproven). Earliest = **sc-11a**. |
| **W3** | **True custody/resolution of the forged telegram + drained account** = the telegram lay **UNDELIVERED** (Crane never read it / never knew of the dismissal), traced to the post-box at the foot of the road; the theft **reconstructed and proven** against Pike | `gt-telegram-found-undelivered`, `gt-audit-telegram-found`, `gt-pike-confesses`, `gt-audit-reconstructs` | CONFRONT->REVEAL: **sc-09a** (account+forgery confessed) / **sc-11a** (telegram traced undelivered). QUIET-AUDIT: **sc-09b..sc-12b** (theft reconstructed) / **sc-19b** (telegram traced). Earliest = **sc-09a**. |

**W3 scope note (honesty bound — the S1 overlap).** S1 is a READER-KNOWN
secret shown EARLY via the burnt-telegram-draft surface: the *existence* of a
forged telegram + embezzlement, Pike's authorship, and that Crane never knew,
are INTENDED to reach the reader early as dramatic irony — re-extracting those
early is **not** a leak, it is the design. W3's withheld part is specifically
the **investigative resolution/proof** (the telegram physically found
undelivered and traced; the theft reconstructed and certified by Hale). The
PIN-2 leak test for W3 therefore targets that *resolved/proven disposition*,
not the early irony-hint surface. W3 is consequently the fuzziest of the three
and any pre-reveal appearance gets the cited leak-vs-noise audit. W1 and W2
are the CLEAN deterministic pins (no S1/S2 overlap: the reader is never told
who climbed or whether it was murder until the reveal).

**PRED-2b (recorded, not pinned).** The early-shown secrets (S1 via the
burnt telegram draft; S2 via the second set of gallery prints) should open
>= 1 reader-knows / Hale-doesn't irony window in the re-extraction
(`report-irony-intervals`). Fuzzy by show-don't-tell -> recorded, not gated.

---

## REVEAL

`verify-seal` against the pre-recorded seal: **MATCH**
(`5496b834d1c220e2427bd087f1bcbe7e2deab065dadb103b383961aa8a242fe4`, exit 0 —
untampered). Map read for unblinding:

```
A = disclosure   (treatment)
B = plain        (R500 control, reused)
```

### Methodology repair (harness drift since R500 — recorded in full)

Two behaviours of the reading-copy tooling had drifted since R500 and were
caught at the judging step; both were repaired deterministically to restore
R500 parity (the deterministic PINs were unaffected — they read the stores,
not the reading copies):

1. **`report-playthrough-manuscript --world` no longer prunes the scene list.**
   The current binary returns the full canon order with per-scene facts
   world-filtered (in-world scene ⟺ `begins > 0`); the R500-era binary pruned
   the list. Verified by running the current binary on R500's own stored store:
   it now returns 29 scenes where R500 recorded 19. Fix: prune each playthrough
   to `begins > 0` scenes before `assemble` — this reproduces R500's 19-scene
   reveal walk exactly (R500's stored 19 == the `begins>0` set).
2. **`assemble` no longer strips editorial scaffolding.** R500's reading copies
   had the `[Dramatic irony …]` / `[CONFRONT limb …]` brackets, `### CHOICE`
   blocks, and the `*[All six setups paid …]*` answer key stripped (only the
   in-prose "the reader knows … Hale does not" sentences survived). The current
   binary left all of it in, inflating B by ~512 words and making the plain
   arm read "assembled / self-annotating" — contaminating a first judging
   round. Fix: strip the editorial brackets, CHOICE blocks, answer keys, and
   world-line map from the source before assemble (in-prose meta-address kept,
   matching R500); first-round verdicts preserved as `run/judges/judge-N.round1-contaminated.md`,
   re-judged blind on the clean copies.
3. **W3 re-match.** A's quiet world is terminal (audit → sc-20b); B's `quiet`
   branch is a non-terminal middle that forks again, so it truncated at a fork
   card. Matched A-quiet to B's terminal `reveal_quiet` (audit → reveal → sc-22)
   — the same terminal quiet-road world R500 used for W3.

Post-repair the six reading copies are scaffolding-free, terminal, and at R500
word parity (W1__B 6,512 w vs R500's 6,581 w). The numbers below are from the
clean re-judge. This repair is harness-plumbing, not a protocol change; the
sha-pinned manifest steps are unchanged.

### PIN-1 — fidelity (PRED-1): **HOLDS**

Disclosure (A) blind re-extraction continuity surfaces:
`D1 (rule_transition_invalid + rule_exclusive_overlap) = 0`,
`D2 (unchained_state_pairs) = 0`,
`D4 (succession_cross_branch + SuccessionCycle) = 0` → **D1+D2+D4 = 0.**
Nothing to leak-vs-noise audit (0 gated findings). A disclosure render is a
projection of the gate-checked base and re-extracts clean.
Common-mode control — plain (B): `D1 = 1` (one `rule_exclusive_overlap`,
ground frame, `reveal_quiet` world, sc-17: Pike's audit-present study location
vs death-night stair location), D2 = 0, D4 = 0 → 1. **The disclosure arm is
cleaner than the control.**

### PIN-2 — premature-leak (PRED-2, the new R502 leg): **HOLDS — 0 leaks**

In the disclosure (A) blind re-extraction, every pre-pinned withheld solution
conclusion's earliest canon coordinate is at-or-after its reveal scene; the
shared spine sc-01..08 carries only the crime scene, the S1/S2 disclosure
surfaces, the 6 planted setups, and hints — no solution conclusion.

| Withheld conclusion | earliest re-extracted coordinate (frame=ground) | reveal scene (pre-pinned) | verdict |
|---|---|---|---|
| **W1** Pike is the climber/intruder | sc-11b (`fa-key-found-warm`: Hale witnesses Pike draw the kept key + enter) / sc-17r (`fa-junia-names-pike`, `fa-pike-had-key-reveal`) | sc-11b (audit) / sc-17r (reveal); never on burn | **at-or-after → PASS** |
| **W2** cause = accidental fall, not murder; clock = jolt not set | sc-15 (`fa-clock-struck`) / sc-17r (`fa-no-murder`) / sc-18b (`fa-clock-struck-quiet`) | sc-11a→sc-19r (confront/reveal); sc-18b→sc-20b (quiet); sc-18w (burn) | **at-or-after → PASS** |
| **W3** telegram undelivered/proven + theft reconstructed | sc-09a (`fa-pike-confesses-forgery/-theft`) / sc-11a (`fa-telegram-undelivered`, `fa-crane-never-read`) / sc-19b (`fa-wire-undelivered-quiet`) | sc-09a/sc-11a (confront); sc-09b–12b/sc-19b (quiet) | **at-or-after → PASS** |

Spine (sc-01..08) ground-frame facts: `fa-crane-dead/alive/blind` (crime
scene), `fa-telegram-corner` (S1 surface — the half-burnt draft, shown not
told), `fa-junia-on-gallery/-saw-climber` (S2 surface — someone climbed, not
who), the 6 setups (`fa-plate-blank` sc-04, `fa-clock-stopped/-wound/-frame-rigid`
sc-05, `fa-unsent-letter` + `fa-account-drained` sc-06/08, `fa-key-bare-hook/-door`
sc-07), and hints (`fa-pike-blanches` sc-06). **None state a withheld solution.**
This is the R502 gate demonstrated on real prose: exposed/withheld binary +
first_at timing, mode-independent, deterministic.

**PRED-2b (irony, recorded not pinned):** the A re-extraction opens ≥3
reader-knows / character-doesn't windows per world (cross-frame edges = 6) — the
early-shown secrets delivered their dramatic irony.

### Craft preference (PRED-3, the OPEN question) — gap CLOSED on the keystone

3 fresh blind judges × 3 matched world-lines, R500 5-axis rubric, on the clean
reading copies. Forced choices: **world-lines A 4 / B 5; overall A 1 / B 2.**

5-axis means (1–5; A = disclosure, B = plain; mean over 3 judges × 3 worlds),
with the R500 recorded baseline beside them:

| axis | A disclosure | B plain | R500 facts-first (omniscient) | R500 plain |
|---|---|---|---|---|
| **prose quality (told-story vs list-like — the KEYSTONE)** | **5.00** | 4.22 | 3.11 | 4.33 |
| stakes / tension | 4.22 | 4.22 | 3.00 | 4.00 |
| setup / payoff | 4.56 | 4.33 | 3.78 | 4.78 |
| character-knowledge | 4.67 | 4.67 | 4.00 | 4.78 |

**The keystone reversed.** The prose-quality axis that drove R500's 3-0 plain
sweep went from facts-first **3.11 (plain +1.22)** to disclosure **5.00 (plain
4.22, disclosure +0.78)**. All three judges scored the disclosure arm's prose
5/5 and praised its sustained close-third voice and *dramatized* (not announced)
withholding; the plain arm was dinged on prose for its recurring direct-address
narrator ("the reader knows this; Hale does not") that "breaks the told-story
spell." On axis means disclosure **ties-or-beats** plain on all four (wins
prose + payoff, ties stakes + knowledge). The overall forced-choice preference
still **narrowly favours plain (2-1 / 5-4)** — plain's residual edge is
stakes/payoff in specific worlds (notably W3 quiet-reveal). This is a near-tie
split, not the R500 0-9 / 0-3 wipeout.

Note (recorded): Judge-1 charged the A burn world (W2) with a "no
confrontation / no burn" fork-label mismatch; the orchestrator verified this is
a **misread** — the A W2 reading copy contains Pike's confession (Hale present,
"he let Pike weep") and the night-log burn (Junia "put the log in" the stove,
the policy paying ironically). It reflects that disclosure's close-third
withholding reads as *less overtly dramatic* than plain's loud confrontation —
a craft observation, not a continuity defect.

### PRED-3 decision (manifest `decision_rule`)

PRED-3's success definition ("prose-quality rises clearly above the R500
omniscient 3.11 toward plain's 4.33; the 3-0 gap narrows or closes") is **met**:
prose rose to 5.00 (above both 3.11 and 4.33), the 3-0 sweep collapsed to a 2-1
near-tie, and the keystone reversed in disclosure's favour. Disclosure does NOT
"read list-like" (the disclosure-INSUFFICIENT alternative is refuted). PIN-1 and
PIN-2 both hold.

→ **Disclosure-as-render-discipline is VALIDATED. The next round is authorized
to build the rung-2 substrate** (stored disclosure plan + render-brief carrier +
the deterministic premature-leak gate, sec 7.21 step 2 / R501–R502).

Honest reservation recorded: the overall *forced-choice* preference still
narrowly favours plain (2-1). The validation rests on the keystone reversal +
axis-mean parity + gap-narrowing, not a clean forced-choice victory. An owner
who weights the headline forced choice over the keystone could read this as
"close but build-on-watch." The orchestrator's read, per the decision_rule's
stated gap-closure definition, is VALIDATED.

### PRED-4 — economics + residue

- **Render cost:** 3 sequential blind sessions, ~209.7k subagent tokens total
  (60.6k + 74.7k + 74.4k), 9,417 words.
- **Hand-brief friction (rung-2 pull signal):** MODERATE. The brief was
  followable with low workflow-friction; the store's `canon_from` coordinates
  already carried the disclosure timing, so the author needed no separate
  stored disclosure plan to know *when* to reveal. Craft attention concentrated
  at the irony/withhold seams (S1/S2 show-don't-tell; holding Pike's false
  belief one scene after the contradiction was shown; paying four objects in one
  close-third without omniscient summary). → a rung-2 substrate would offload
  the manual per-scene "what may surface here, in whose frame" bookkeeping;
  worth building given the validation, but the brief alone was a real workflow.
- **Untyped residue (R476 ceiling):** judges found ~0 hard internal continuity
  errors in either arm per world (the cross-version "different deaths / opposite
  W2 certifications" they noted are the designed branch divergence, not errors).
  Residue ≈ 0 both arms — consistent with R500.

### Honesty bounds that bit (restated from the manifest)

- Plain (B) is the **R500 recorded baseline, reused** — a cross-run comparison,
  not a same-run twin. Validity of the prose cross-ref rests on the reading
  copies being cleaned to R500 parity (done; see the repair note).
- The disclosure arm reuses the R500 `ff.atomic.json` fact-base — isolates the
  render variable but inherits R500's fact coverage/gaps.
- Premise reuse is deliberate (render isolation), judge-clean (fresh judges),
  trades R499 novelty-control.
- This lineage (R501/R502) wrote the pinned disclosure brief — design-stage
  selection bias the sha-pin does not cover; the author/extractor/judges were
  blind, which covers tampering/peeking, not brief selection.
- rung-1 (limited focal frame) and rung-2 (per-fact state/hint/imply/withhold +
  first_at) were tested **together** (the premise needs both); a clean rung-1-only
  test needs a pure-mystery premise (recorded future option).
- The harness drift + R500-parity repair is itself a recorded deviation from the
  literal runbook commands (jq → Read; schema header + `begins>0` prune;
  scaffolding strip; W3 re-match; one re-judge).

### Result table (R452 self-containment)

| leg | result |
|---|---|
| PIN-1 fidelity (disclosure D1+D2+D4) | **0** (HOLDS) — plain control = 1 |
| PIN-2 premature-leak (withheld solutions re-extractable pre-reveal) | **0** (HOLDS) |
| prose-quality keystone | disclosure **5.00** vs plain 4.22 (R500: FF 3.11 vs plain 4.33) — reversed |
| forced choice | world-lines A 4 / B 5; overall A 1 / B 2 (R500 treatment: 0/9, 0/3) |
| PRED-3 decision | **gap CLOSED → disclosure VALIDATED → build rung-2 substrate** (reservation: plain still edges forced choice 2-1) |
| PRED-4 | render ~209.7k tok / 3 sessions; brief friction MODERATE; residue ≈ 0 both arms |

