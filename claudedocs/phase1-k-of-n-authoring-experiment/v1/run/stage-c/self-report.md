# Self-report — "Winter House"

Fifteen scenes, one house on one lane, one winter, five people in it: Ellis Varn
the master, Adela his wife, Jonas the elder son, Tobin the youngest, and Marget
Hale who has kept the house twenty years. A sixth name, Rhodes Callender, is
registered as an entity and never appears on the page.

## The withheld fact

`f-truth`, in `facts.json`: *"Tobin was fathered by Rhodes Callender."* Frame
`ground-truth`, `canon_from` `sc-01`, `evidence` `["sc-01"]`, typed leg
`sired_by(e-tobin, e-callender)`.

That is the whole of "true of the house from the first scene": the claim starts
holding at the first scene and is evidenced by the first scene, so nothing about
its truth is deferred. What is deferred is the reader, and that is a different
axis, carried by the disclosure plan rather than by the canon coordinate.

The house's own account of the same slot is a separate fact, `f-supposition`,
frame `household`: *"Tobin is the master's youngest son"*, typed
`sired_by(e-tobin, e-ellis)`, and it carries `conflicts_with: ["f-truth"]` — a
recorded judgment, not something inferred from the wording. `rules.json`
declares an exclusive rule `one-father-per-child` (`sired_by`, per subject), so
the two readings are formally incompatible statements about one slot; they do
not trip the gate only because exclusivity is scoped within a single frame, and
these are held in two. Three people in the house hold the true one —
`f-ellis-knows`, `f-adela-knows`, `f-marget-knows`, each typed
`knows_that(person, f-truth)` using the typed fact-reference shape — and Jonas,
the one who does not know, holds `knows_that(e-jonas, f-supposition)`.

## The three scenes that brush against it

All three are in `facts.json`, and all three are marked by the same structural
device: each is a fact that carries `pays_off: ["f-truth"]`, and `f-truth` is the
only fact in the store marked `payoff_expectation: "expected"` for the reader's
sake. So "which scenes brush against the withheld thing" is answerable by query
alone — they are the payoffs of the withheld setup, and there are exactly three.

1. **`sc-03`, "The Likeness"** — the likeness that does not match. `f-likeness-tobin`
   ("Tobin carries none of the Varn face at all", typed `family_likeness(e-tobin,
   absent)`), set against its sibling `f-likeness-jonas` (`marked`) so the
   mismatch has something to be a mismatch *against*. The `quote` leg carries the
   line as it stands on the page. Section registered in `sections.json`; placed on
   the road by `order.json`.
2. **`sc-08`, "A Settlement, Nine Years Old"** — the settlement in the old ledger.
   `f-settlement`, typed `records_settlement_to(e-ledger, e-callender)`, with
   `f-settlement-sum` giving four hundred pounds as a quantity. It is dated the
   spring before Tobin's birth, and `f-tobin-age` fixes Tobin at nine this winter,
   so the arithmetic is available to a machine and never spelled out in prose.
   Jonas is only in the study at all because `f-accounts` at `sc-07` put him there;
   `f-settlement` pays that setup off too.
3. **`sc-12`, "What Marget Did Not Say"** — the servant who stops a sentence
   halfway. `f-marget-stops`, typed `breaks_off_about(e-marget, e-tobin)`, with
   the broken sentence itself in the `quote` leg.

## Where the reader comes to hold it

At **`sc-08`, the second of the three** on the road this telling walks.

Two files say it, one as a rule and one as an instance.

**The rule — `facts.json`, `disclosure_plans`, telling `winter-house`.** The
override on `f-truth` is:

- `mode: "imply"` — not `withhold`. The reader is meant to arrive at this, and a
  withheld fact plus a reveal pin discloses nothing on any road; `imply` is the
  mode for a thing realised through an object, which is what all three brushes are.
- `surface: { "scene": "sc-08", "object": "e-ledger" }` — the seat. This is where
  the reader meets it, and the object they meet it in is the ledger.
- `first_at: [{ "branch": "main", "coords": ["sc-03", "sc-08", "sc-12"],
  "threshold": 2 }]` — the K-of-N pin. The coords set names the three brushes and
  nothing else; the threshold of 2 says the reveal is the **second-earliest coord
  reached**, whichever two those turn out to be. One brush is below the threshold,
  so a reader who has passed only one has been told nothing and may fairly think
  they imagined it. The third is above it and adds no reveal.

Everything else in the plan defaults to `state` — the three brushes are told
outright, because each of them is on the page as an observation; what is never
stated is the conclusion. The three `knows_that` facts and the four reader-frame
facts are individually overridden to `withhold`, because rendering any of them
would state the thing the whole telling is arranged not to state.

**The instance — the `reader` frame in `facts.json`.** I registered the reader as
an epistemic frame like any other, non-privileged, and wrote the reader's stance
as a four-step in-frame succession over the token predicate `reader_stance`:

- `f-reader-1` at `sc-01`, `unquestioned`
- `f-reader-2` at `sc-03`, `unsettled` — supersedes the first. One brush passed;
  still deniable.
- `f-reader-3` at `sc-08`, `held` — supersedes the second. This is the moment.
- `f-reader-4` at `sc-12`, `confirmed` — supersedes the third. Confirmation only.

Each of the last three cites the brushes it has passed as `evidence`, all prior
to or at its own `canon_from`, so the ladder is auditable backwards. On this
telling's single road the second-earliest of the three coords resolves to `sc-08`,
which is why the ladder's `held` step and the disclosure's seat name the same
scene: the disclosure states the rule, the ladder states what the rule comes out
as here.

## Which file says what

- `sections.json` — the fifteen scenes, `sc-01` through `sc-15`, document
  `winter-house`. Flat, so the order can place every one of them.
- `order.json` — the discourse order, a single main road `sc-01` → `sc-15`, no
  branches. This is what makes "second of the three" a determinate scene.
- `facts.json` — the registries, the facts, the map side tables, and the
  disclosure plan. The withheld fact, its three payoffs, the frame layout, and
  the K-of-N reveal pin are all here.
- `rules.json` — three continuity rules: `one-father-per-child` (exclusive on
  `sired_by`, per subject), `one-place-per-person` (exclusive on `at`, per
  subject, containment-aware), `house-movement` (transition on `at`, adjacency
  `adjacent`, undirected, containment `contains`).
- `mnemosyne.toml` — `canon_order_path` was already pinned; I added
  `rules_path = "rules.json"`, since the rules do nothing until they are wired.

## The house as a map, and two things I want on the record

The house is authored as a real map so that the winter is checkable and not just
asserted. `p-house` contains hall, parlour, study, kitchen, nursery; `p-lane` and
`p-house` sit together in the root scope with an edge between them, which is the
portal by which anyone gets in or out. The adjacency predicate declares
`subject_kind` and `object_entity_kind` of `place`, without which the
completeness check has no way to know what a place is and quietly evaluates
nothing. Every place carries at least one edge. Each of the five people has a
chain of `at` facts linked by `supersedes_in_frame`, and every step in every
chain is either a declared edge between siblings or a lift to the sibling pair
that licenses it — Jonas going from the lane to the hall is licensed by the
lane-to-house edge, not by an invented door. Edge costs are on all five edges; a
single edge guard on the lane-to-house edge names the two open spells of the lane
with `threshold: 1`, which is the substrate's other K-of-N and the reason the
shut weeks are a fact about the world rather than a mood.

Two honest limits. First, I was given no command to run, so nothing here has been
put through `validate-continuity`, `propose-verdict`, or the render-acceptance
gates; every claim above about what the files *say* I stand behind, and no claim
about what a gate *would report* is measured. Second, the reader-as-frame ladder
is a modelling choice rather than something the contract prescribes: the contract
gives frames as "whose belief a fact records" and says they are sparse and
non-privileged, and I took the reader to be a believer like any other so that the
moment of holding is a stored, queryable succession edge and not only a property
of a disclosure plan. If that reading is wrong, the disclosure override on
`f-truth` still carries the whole requirement on its own, and the ladder is
redundant rather than load-bearing.
