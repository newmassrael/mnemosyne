#!/usr/bin/env bash
# test-side-workspaces — run the tests of every SEPARATE in-repo workspace.
#
# This repository has more than one workspace. The root one is built and tested
# by CI on every push; the others carry their own `[workspace]` precisely so the
# root gates never compile them (`bench/`, `studio/`, `tools/*`). That choice is
# deliberate and it left a hole nobody was watching: `cargo test --workspace`
# cannot reach them, and the pre-commit hook's separate-workspace gate runs
# `fmt` and `clippy` on them only when one of their files is staged. Their TESTS
# were run when a person remembered to run them.
#
# What that cost, measured on the day this script was written rather than
# guessed: of the four separate workspaces, TWO were already rotted.
#   - `studio/` does not compile at all (17 errors, its source using a
#     `pinion_gpu` its manifest does not depend on).
#   - `bench/` has 6 failing tests in `codegen-prototype`, pinned to the
#     markdown-document model Round 400 removed.
# Neither had failed anything, because nothing had asked.
#
# THE RULE HERE IS TOTALITY. Every separate workspace is either tested or named
# below with the reason it is not, and the reason is PRINTED on every run. A
# workspace nobody has thought about is tested by default: adding one to this
# repository adds it to this gate, and taking it out of the gate takes writing
# down why.
#
# Usage:
#   scripts/test-side-workspaces.sh              # discover, skip the named, test the rest
#   scripts/test-side-workspaces.sh bench …      # test exactly these, skipping nothing
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

if [[ $# -gt 0 ]]; then
  workspaces=("$@")
  declare -A ungated=() # named explicitly means asked for explicitly
else
  mapfile -t workspaces < <(discover)
fi

tested=()
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
  echo "[side-workspaces] TEST $ws"
  # `--no-fail-fast`, because a gate that stops at the first failing target
  # reports a smaller number than the truth and somebody fixes to it. This gate
  # did exactly that on its first run: it said `bench` had 6 failures, and the
  # 6 were one target's — there were 17.
  cargo test --manifest-path "$ws/Cargo.toml" --locked --no-fail-fast
  tested+=("$ws")
done

echo "[side-workspaces] tested ${#tested[@]} (${tested[*]-}), skipped ${#skipped[@]} (${skipped[*]-})"
if [[ ${#tested[@]} -eq 0 ]]; then
  echo "[side-workspaces] this gate reached no workspace at all — a green run that" \
       "tested nothing is the failure it exists to prevent" >&2
  exit 1
fi
