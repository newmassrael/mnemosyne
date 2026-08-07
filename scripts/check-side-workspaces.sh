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

# THE TREE UNDER CHECK IS THE WORKING DIRECTORY, not this script's own location.
# A git hook runs with the repository root as its working directory and may be
# THIS repository's hook running over ANOTHER tree — which is exactly what the
# hook's own smoke test does, and what broke when this script resolved its root
# from `$BASH_SOURCE`: it walked its own checkout and found none of the caller's
# workspaces. The requirement is stated rather than assumed.
root=$(pwd)
if [[ ! -f "$root/Cargo.toml" ]]; then
  echo "[side-workspaces] $root has no Cargo.toml — run this from the root of the" \
    "tree to check, which is where a git hook and CI both start" >&2
  exit 2
fi

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
  echo "[side-workspaces] CHECK $ws — fmt, clippy, item citations$($lint_only || echo ', tests')"
  # PACKAGE BY PACKAGE, over the manifests that live INSIDE this workspace's
  # directory, which is the same thing as "the packages this repository owns".
  #
  # Not `--all`: R1066 used it because a VIRTUAL manifest (`bench/Cargo.toml` is
  # one: `[workspace]` and no `[package]`) has no targets of its own, and
  # `cargo fmt` without it exits non-zero printing its usage — which a caller
  # reads as "unformatted" while nothing was checked at all. But `--all` walks
  # PATH DEPENDENCIES, and `studio` depends on a sibling checkout: this gate
  # spent three rounds reporting `studio` as unformatted because of files in
  # `../pinion`, which this repository does not own, cannot commit, and had
  # another session live in. A gate that fails on somebody else's file is a gate
  # that gets ignored. Walking this workspace's own manifests keeps the virtual
  # case working (the virtual manifest declares no package and is skipped) and
  # cannot leave the tree.
  formatted=0
  while IFS= read -r -d '' member; do
    grep -qE '^\[package\]' "$member" || continue
    formatted=$((formatted + 1))
    if ! cargo fmt --manifest-path "$member" --check; then
      echo "[side-workspaces] $ws is unformatted —" \
        "fix: cargo fmt --manifest-path $member" >&2
      exit 1
    fi
  done < <(find "$ws" -name Cargo.toml -not -path '*/target/*' -print0 | sort -z)
  if [[ $formatted -eq 0 ]]; then
    echo "[side-workspaces] $ws has no package manifest of its own — a workspace this" \
      "gate formats nothing in is one it is not checking" >&2
    exit 1
  fi
  cargo clippy --manifest-path "$ws/Cargo.toml" --all-targets -- -D warnings
  # R1078 — every item citation in this workspace names an item. The root
  # workspace gets this from the pre-commit hook and its own CI job; ZZ10 named
  # the general hole that a check taking only the root leaves these four out,
  # and this one does not take only the root: it is pointed at a manifest.
  #
  # It found something on its first run here, which is the whole argument for
  # putting it in: two citations in `bench` and one in `studio` that named
  # nothing (`#[tracked]`, `child[i]`, `argv[1]` — brackets are markdown whether
  # or not anyone meant them that way), and NINE bench targets it could not
  # document at all until cargo's omissions were put back.
  # THE PROGRAM comes from THIS script's checkout; the TREE it is pointed at is
  # the working directory, per the rule above. Those are two different things
  # and they are two different trees whenever this repository's gate runs over
  # another one — resolving the program from `$root` looks for it inside the
  # tree under check, where it does not exist.
  citations="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/tools/item-citations/Cargo.toml"
  if [[ ! -f "$citations" ]]; then
    echo "[side-workspaces] the item-citation gate is missing at $citations" >&2
    exit 1
  fi
  if ! cargo run -q --manifest-path "$citations" --bin item-citations -- \
    --workspace "$root/$ws/Cargo.toml"; then
    echo "[side-workspaces] $ws carries a citation that names no item —" \
      "fix: cargo run -q --manifest-path tools/item-citations/Cargo.toml" \
      "--bin item-citations -- --workspace $ws/Cargo.toml" >&2
    exit 1
  fi
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
