#!/usr/bin/env bash
# check-side-workspaces — fmt, clippy and test every SEPARATE in-repo workspace.
#
# This repository has more than one workspace. The root one is built, linted and
# tested by CI on every push; the others carry their own `[workspace]` precisely
# so the root gates never compile them (`bench/`, `studio/`, `tools/*`). That
# choice is deliberate and it left a hole nobody was watching: `cargo test
# --workspace` cannot reach them, and the pre-commit hook reaches them only when
# one of their files is STAGED. What nobody edits, nobody checks.
#
# What that cost, measured rather than guessed: of the four separate workspaces,
# TWO were already rotted when this gate first ran. `studio/` did not compile at
# all (17 errors, its source naming a `pinion_gpu` its manifest did not depend
# on). `bench/` had 18 broken tests, 79 clippy findings and had never once been
# through `rustfmt`. Neither had failed anything, because nothing had asked.
#
# THE THREE CHECKS ARE ONE GATE, and that is this round's correction: a gate that
# ran their tests and left their lints to a hook nobody triggers is the same hole
# one notch smaller. Every check the root workspace gets, a separate one gets —
# from one definition, so the two cannot drift. The pre-commit hook calls THIS
# script with `--lint-only`, which is exactly what the root gets before a commit
# (`fmt` and `clippy`, never the suite), and CI calls it whole.
#
# THE RULE IS TOTALITY. Every separate workspace is either checked or named below
# with the reason it is not, and the reason is PRINTED on every run. A workspace
# nobody has thought about is checked by default: adding one to this repository
# adds it to this gate, and taking it out takes writing down why.
#
# Usage:
#   scripts/check-side-workspaces.sh               # discover, skip the named, check the rest
#   scripts/check-side-workspaces.sh --lint-only   # fmt + clippy, no suite (the hook's form)
#   scripts/check-side-workspaces.sh bench …       # check exactly these, skipping nothing
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

# Workspaces this gate does not run, and why. A skip written here is a claim
# somebody has to defend in review; the skip below it is a claim about the
# machine, which the machine answers.
#
# IT IS EMPTY, and that is the state to keep it in. `bench` sat here for one
# round with eleven tests reading documents this project had deleted; they were
# retired rather than tolerated, and every separate workspace is now run. An
# entry added here should read like a decision somebody made, not like a failure
# somebody stepped around.
declare -A ungated=()

discover() {
  # Every Cargo.toml with its own [workspace] that is not the root one — the
  # same walk the pre-commit hook does when it gates fmt and clippy for these.
  find . -name Cargo.toml -not -path '*/target/*' -not -path './Cargo.toml' -print0 |
    sort -z |
    while IFS= read -r -d '' manifest; do
      grep -qE '^\[workspace\]' "$manifest" && dirname "${manifest#./}"
    done
}

# The path dependencies a workspace names OUTSIDE this repository that are not
# on this machine. `studio/` depends on the sibling pinion checkout, so it is
# testable exactly where that checkout is: here, and not on a CI runner. Asking
# the machine beats declaring the answer — a hardcoded "CI cannot build studio"
# would also skip it on the one machine that can, which is where its rot was
# found in the first place.
missing_siblings() {
  local ws=$1 manifest dep target
  while IFS= read -r -d '' manifest; do
    while IFS= read -r dep; do
      target=$(realpath -m "$(dirname "$manifest")/$dep")
      case "$target" in "$root"/*) continue ;; esac
      [[ -d "$target" ]] || echo "$dep"
    done < <(grep -oE 'path[[:space:]]*=[[:space:]]*"[^"]+"' "$manifest" |
      sed -E 's/.*"([^"]+)".*/\1/')
  done < <(find "$ws" -name Cargo.toml -not -path '*/target/*' -print0)
}

lint_only=false
named=()
for argument in "$@"; do
  case "$argument" in
    --lint-only) lint_only=true ;;
    -*) echo "check-side-workspaces: unknown flag $argument" >&2; exit 2 ;;
    *) named+=("$argument") ;;
  esac
done

if [[ ${#named[@]} -gt 0 ]]; then
  workspaces=("${named[@]}")
  declare -A ungated=() # named explicitly means asked for explicitly
else
  mapfile -t workspaces < <(discover)
fi

checked=()
skipped=()
for ws in "${workspaces[@]}"; do
  if [[ -v ungated["$ws"] ]]; then
    echo "[side-workspaces] SKIP $ws — ${ungated[$ws]}"
    skipped+=("$ws")
    continue
  fi
  absent=$(missing_siblings "$ws" | sort -u | tr '\n' ' ')
  if [[ -n "${absent// /}" ]]; then
    echo "[side-workspaces] SKIP $ws — its path dependencies leave this repository" \
      "and are not on this machine: ${absent% }"
    skipped+=("$ws")
    continue
  fi
  echo "[side-workspaces] CHECK $ws — fmt, clippy$($lint_only || echo ', tests')"
  # `--all`, because a VIRTUAL workspace manifest (`bench/Cargo.toml` is one:
  # `[workspace]` and no `[package]`) has no targets of its own, and without it
  # `cargo fmt` exits non-zero printing its own usage — which a caller reads as
  # "unformatted" while nothing was checked at all.
  if ! cargo fmt --all --manifest-path "$ws/Cargo.toml" --check; then
    echo "[side-workspaces] $ws is unformatted —" \
      "fix: cargo fmt --all --manifest-path $ws/Cargo.toml" >&2
    exit 1
  fi
  cargo clippy --manifest-path "$ws/Cargo.toml" --all-targets -- -D warnings
  if ! $lint_only; then
    # `--no-fail-fast`, because a gate that stops at the first failing target
    # reports a smaller number than the truth and somebody fixes to it. This gate
    # did exactly that on its first run: it said `bench` had 6 failures, and the
    # 6 were one target's — there were 18.
    cargo test --manifest-path "$ws/Cargo.toml" --locked --no-fail-fast
  fi
  checked+=("$ws")
done

echo "[side-workspaces] checked ${#checked[@]} (${checked[*]-}), skipped ${#skipped[@]} (${skipped[*]-})"
if [[ ${#checked[@]} -eq 0 ]]; then
  echo "[side-workspaces] this gate reached no workspace at all — a green run that" \
       "checked nothing is the failure it exists to prevent" >&2
  exit 1
fi
