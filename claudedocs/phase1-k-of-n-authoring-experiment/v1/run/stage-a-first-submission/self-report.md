# Saltgate Post — self-report

## What I wrote, and where

| file | what it holds |
| --- | --- |
| `sections.json` | the twenty scenes (`sc-01` … `sc-20`), as a bare JSON array for `import-sections` |
| `order.json` | the canon order — the nineteen discourse edges that put those twenty scenes on `main`'s road (no branches; `"branches": {}`) |
| `facts.json` | the fact manifest: frames, entity kinds, units, entities, predicates, all the facts, the **edge costs**, the **edge guards**, and one disclosure plan |
| `rules.json` | the four narrative rules that the continuity gate enforces |
| `mnemosyne.toml` | I added `[continuity] rules_path = "rules.json"` next to the `canon_order_path` that was already there, because the contract says the rules gate stays off until the file is wired |

Everything below is in `facts.json` unless I name another file.

## How many locations, how many ways

The post has **twelve locations**. They are `e-outer-gate`, `e-gatehouse-yard`,
`e-market-square`, `e-weighbridge`, `e-warehouse-row`, `e-river-wharf`,
`e-water-stair`, `e-wardens-walk`, `e-counting-hall`, `e-tally-office`,
`e-strongroom`, `e-caravanserai`. Each is a registered entity of kind `place` in
the `entities` array of `facts.json`, and each is declared to lie inside the
walls by a `contains(e-saltgate-post, …)` fact (the twelve `f-in-post-…` facts).
The post itself, `e-saltgate-post`, is kind `settlement`, not `place` — it is the
container, not somewhere you stand.

**Fourteen ways** lead between them. Each way is one `adjacent(a, b)` fact, and
the ids read as the way itself: `f-way-gate-yard`, `f-way-yard-market`,
`f-way-yard-walk`, `f-way-market-caravanserai`, `f-way-market-weighbridge`,
`f-way-market-counting`, `f-way-weighbridge-warehouse`, `f-way-warehouse-wharf`,
`f-way-warehouse-caravanserai`, `f-way-counting-tally`,
`f-way-counting-strongroom`, `f-way-wharf-stair`, `f-way-stair-walk`,
`f-way-walk-counting`.

One fact is the whole way, in both directions: the transition rule
`moves-follow-the-ways` in `rules.json` declares `"undirected": true`, so a way
does not have to be written twice. Every way also carries a walking time in the
`edge_costs` array of `facts.json` — a number of `minute`, from one minute
(counting hall to strongroom) to six (wharf to the foot of the stair). Mnemosyne
stores those numbers and never adds them up; adding them up is the route
planner's arithmetic.

## Which ways are shut, and what opens each

Three of the fourteen are shut. Each shut way is declared in the `edge_guards`
array of `facts.json`, keyed by the id of the way it gates, with the condition
facts it requires. A route planner reads the guards, resolves each condition fact
id, and asks whether that fact holds where they are standing in the story. Nothing
in this store evaluates a guard — that is the consumer's job — so the store says
*what is required*, never *whether you may pass now*.

**1. The strongroom — `f-way-counting-strongroom`, counting hall to strongroom.**
Guarded by three conditions with `"threshold": 2`:

- `f-seal-ready-hesper` — Warden Hesper is in the post with her council seal and can set it in the council lock
- `f-seal-ready-orrin` — the same of Warden Orrin
- `f-seal-ready-vale` — the same of Warden Vale

Any two of those three open it. This is the only way into the strongroom;
`f-way-counting-strongroom` is its only edge, so the guard is not a detour but
the door.

**2. The tally office — `f-way-counting-tally`, counting hall to tally office.**
One condition, no threshold, which the contract says means require all:

- `f-key-ready-nissa` — Clerk Nissa is in the post with the tally-office key and can turn the tally-office lock

Nobody else in the post carries a key that will turn it, and this is the office's
only edge.

**3. The water-stair — `f-way-wharf-stair`, river wharf to the water-stair.**
One condition, no threshold:

- `f-cond-low-water` — the river stands at low water and the foot of the stair is dry

Note where the guard sits. The stair's *other* edge, `f-way-stair-walk`, from the
head of the stair up to the wardens' walk, is unguarded — the head of the stair is
above any flood. It is the flight between the wharf and the stair that drowns, so
that is the edge that carries the tide condition. Tesh walks up it at `sc-07`
while `f-cond-low-water` holds, and at `sc-18` she gets four flights down and can
go no further, because `f-tide-high` superseded `f-cond-low-water` at `sc-15`.

The other eleven ways carry no guard, which under this contract means unrecorded,
not forbidden.

### How a planner tells whether a shut way is shut *to them*

Every condition is an ordinary fact with a canon extent, so "is this way open
here" is answered by asking whether the condition fact holds at that canon
coordinate:

- `f-seal-ready-hesper` holds from `sc-01` with no end.
- `f-seal-ready-vale` holds at `sc-01` only — it carries `"canon_to": "sc-01"`, because Vale rides out for the salt fair before the gate is fully open.
- `f-seal-ready-orrin` holds from `sc-14`, when Orrin rides back in from up-country.
- `f-key-ready-nissa` holds from `sc-01` with no end.
- `f-cond-low-water` holds from `sc-01` until `f-tide-high` supersedes it at `sc-15`.

So the strongroom count runs 2 of 3 at `sc-01` (Hesper and Vale, before Vale
leaves), 1 of 3 from `sc-02` to `sc-13`, and 2 of 3 again from `sc-14`. That is
the shape of the day: Hesper sets her one seal at the door at `sc-11` and nothing
turns (`f-lock-refuses`); Orrin's return at `sc-14` makes the second, and at
`sc-16` the lock gives (`f-lock-gives`). I did not author a "Vale cannot seal"
fact — under this contract the absence of a fact in a frame is unrecorded, never
false, so the third condition simply does not hold between `sc-02` and the end,
and there is nothing to leave stale.

## How I said the two-of-three custom in particular

Three ways, all in `facts.json`, and they agree:

1. **The guard, which is the machine-readable one.** In the `edge_guards` array,
   the entry keyed by `f-way-counting-strongroom` lists exactly three conditions —
   the three wardens' seal-readiness facts — and sets `"threshold": 2`. The
   contract's K-of-N rule is that an omitted threshold means AND over the whole
   set, and `k == len` normalizes back to AND, so `2` of `3` is a real
   at-least-two and is legal (`1 <= k < len`). Two named seals of the three, never
   one, and the set is a set — it does not say *which* two.

2. **A typed restatement on the lock itself.** `f-custom-two-of-three` is a fact
   whose typed leg is `opens_for_count(e-council-lock, {quantity, n: 2, unit:
   "seal"})`, using a registered unit `seal`. So the number two is readable from
   the lock as well as from the edge, without going through the guard.

3. **The prose claim on that same fact**, which says why the custom exists: the
   lock gives to any two of the three wardens' seals set together and never to one
   alone, *because the third warden is commonly away*. That fact is marked
   `payoff_expectation: "expected"`, and `f-lock-gives` at `sc-16` pays it off, so
   the custom is a setup that the store can check was discharged.

The disclosure plan `plain-telling` in `facts.json` seats
`f-custom-two-of-three` with `surface.scene = "sc-11"` and
`surface.object = "e-council-lock"` — the custom is stated in the world from
`sc-01`, but the reader meets it at the door where it bites, on the lock. Because
the mode is `state`, not `withhold`, it is actually told; the contract warns that
`withhold` plus a reveal pin discloses nothing anywhere.

## The rest of what the store holds, briefly

`rules.json` turns on four rules: `one-place-per-person` (`at`, exclusive per
subject, refinement-aware through `contains`), `moves-follow-the-ways` (`at`,
transition over the `adjacent` edges, undirected, scoped by `contains`),
`one-holder-per-token` (`holds`, exclusive per **object** — the predicate is
written holder-first, so keying on the object is one holder per thing), and
`one-state-of-the-river` (`water_state`, exclusive per subject). Every move any
character makes is a `supersedes_in_frame` succession whose two ends are joined by
one of the fourteen ways, so the gate can check the walking as well as the map.

Seven people move through the post over the twenty scenes, and something happens
at each of the twelve places. There are two frames: `ground-truth` and
`clerk-nissa`, who believes at `sc-10` that Vale left her seal in the tally-office
strongbox — `f-nissa-thinks-vale-sealed`, recorded as `conflicts_with`
`f-vale-downriver`, and corrected in her own frame at `sc-11`. That belief is a
fact in her frame only; it is not one of the strongroom's conditions, and it
opens nothing.
