# experiment-harness

Fail-loud, reproducible mechanics for a blind A/B authoring experiment:
reading-copy assembly, arm shuffle + seal, and seal verification.

## Why this exists

The A/B (R471) and scale-floor (R475) experiments did three mechanical steps —
assembling the judges' reading copies, coin-flipping the arm labels, and
stripping identifying metadata — with one-off Python/bash that was then deleted.
That left the experiments **non-reproducible** (an auditor cannot re-derive the
blind copies the judges read) and ran the work in a silent-fail language inside a
project whose whole thesis is fail-loud verification. R479 deferred the fix;
this crate is it. Every step errors loudly and lives under version control.

It is a **separate in-repo workspace** (the `bench/` and `studio/` pattern): its
own `[workspace]`, so the root build / CI / pre-commit gates never compile it. It
depends on nothing in the core crates — it is a pure data transform over the
experiment artifacts on disk.

## Build

```
cargo build   --manifest-path tools/experiment-harness/Cargo.toml --release
cargo test    --manifest-path tools/experiment-harness/Cargo.toml
cargo clippy  --manifest-path tools/experiment-harness/Cargo.toml --all-targets -- -D warnings
```

## Subcommands

### `assemble` — build a blind reading copy

```
experiment-harness assemble \
  --story belvoir-extraction/story-A.md \
  --playthrough pt-A.json \
  --world confront \
  --out world-confront-A.md
```

`--playthrough` is the JSON from
`mnemosyne-cli report-playthrough-manuscript --world <w> --json` run over the
**blind re-extracted** store; it supplies the per-world ordered scene walk.
`--story` supplies the titles and prose. The two are joined by scene id.

The blind reading-copy transform (v1), applied per scene body:

- drop `<!-- ... -->` scaffolding comments (an unterminated `<!--` is an error);
- drop `CHOICE:` fork-directive lines;
- collapse blank-line runs and trim the ends;
- normalize the heading `## sc-NN — Title` to `## Title`.

Option text and `ENDING` headers are left verbatim — that matches the transform
the scale-floor experiment actually used. Tightening either is a future flag, not
a silent default.

Loud failures (exit 2): a world-order scene absent from the story, a duplicate
scene id, an empty body after stripping, an unknown world, malformed JSON. These
are exactly the cases the deleted `dict.get(id, "")` swallowed.

Without `--out` the manuscript goes to stdout.

### `shuffle` — assign blind labels and seal

```
experiment-harness shuffle \
  --experiment belvoir-scale-floor \
  --note "reveal only at S17" \
  --out label-map.json \
  plain loop
```

Assigns labels A, B, … to the arms by a `/dev/urandom` shuffle, writes the label
map, and prints its sha256 to stdout (nothing else is on stdout, so it can be
captured). **Record that sha256 in the ledger before reveal** — it is the seal.
At least two distinct arms are required.

### `verify-seal` — the reveal/audit check

```
experiment-harness verify-seal --map label-map.json --sha256 <hex-from-ledger>
```

Re-hashes the map and compares. `MATCH` exits 0; a mismatch (the map was edited
after sealing) prints `MISMATCH` and exits 1.

## Reproducibility note

The shuffle outcome is deliberately **not** reproducible — that is the blinding.
What is reproducible is everything else: given the same story, playthrough, and
world, `assemble` is a pure function; and the seal makes any post-hoc edit to the
label map a loud, non-zero-exit failure. To re-check a past experiment, re-run
`assemble` over the archived blind-re-extracted stores and diff against the
committed reading copies.
