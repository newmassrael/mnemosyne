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

# The path dependencies a workspace names OUTSIDE this repository — whether or
# not the tree they name is on this machine. THIS IS THE OWNERSHIP QUESTION and
# it is not the same as the one below it: a workspace that resolves against a
# tree this repository does not own has a resolution this repository cannot
# pin, on every machine, including the one that can compile it.
# Each line is `<resolved tree><TAB><the dependency as written>`: the resolved
# path is what a caller asks the disk about, the spelling is what a reader is
# shown. Deriving the first from the second a second time would resolve it
# against the wrong directory the moment a member manifest sits below the
# workspace root, which is exactly where a relative path stops meaning the same
# thing from two places.
outside_dependencies() {
  local ws=$1 manifest dep target
  while IFS= read -r -d '' manifest; do
    while IFS= read -r dep; do
      target=$(realpath -m "$(dirname "$manifest")/$dep")
      case "$target" in "$root"/*) continue ;; esac
      printf '%s\t%s\n' "$target" "$dep"
    done < <(grep -oE 'path[[:space:]]*=[[:space:]]*"[^"]+"' "$manifest" |
      sed -E 's/.*"([^"]+)".*/\1/')
  done < <(find "$ws" -name Cargo.toml -not -path '*/target/*' -print0)
}

# THE CHECKABILITY QUESTION: of those, the ones whose tree is not here. `studio/`
# depends on the sibling pinion checkout, so it is testable exactly where that
# checkout is: here, and not on a CI runner. Asking the machine beats declaring
# the answer — a hardcoded "CI cannot build studio" would also skip it on the one
# machine that can, which is where its rot was found in the first place.
#
# The two are different answers and R1115 is what separated them: `studio` was
# UNCHECKABLE on a runner and UNOWNED everywhere, and while one predicate served
# both, the second fact had nowhere to be said. What it cost is below.
missing_siblings() {
  local ws=$1 target dep
  while IFS=$'\t' read -r target dep; do
    [[ -d "$target" ]] || echo "$dep"
  done < <(outside_dependencies "$ws")
}

lint_only=false
list_only=false
named=()
for argument in "$@"; do
  case "$argument" in
    --lint-only) lint_only=true ;;
    --list) list_only=true ;;
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

# Print a command in the one shape a reader parses, then run those same words.
# ONE ARRAY EXPANDED TWICE is the whole point: `${words[*]}` is what a gate reads
# and `"${words[@]}"` is what the shell executes, so a flag cannot be in the
# report and out of the run. `$ws` is the loop's, read when this is called.
declare_and_run() {
  local role=$1
  shift
  echo "[side-workspaces] COMMAND $ws $role $*"
  if $list_only; then
    return 0
  fi
  "$@"
}

checked=()
skipped=()
for ws in "${workspaces[@]}"; do
  if [[ -v ungated["$ws"] ]]; then
    echo "[side-workspaces] SKIP $ws — ${ungated[$ws]}"
    skipped+=("$ws")
    continue
  fi
  # WHOSE RESOLUTION IS THIS? R1115. A cargo command that is allowed to resolve
  # freely REWRITES the lockfile it disagrees with — measured, for every
  # subcommand this repository issues, by `locked_resolution_smoke`. So a gate
  # that lints before it tests REPAIRS the evidence the test was going to read,
  # and the `--locked` on the suite below was structurally unable to fail.
  #
  # `--locked` is therefore on every command here, and the one thing that can
  # take it off is the workspace resolving against a tree this repository does
  # not own. `studio` path-depends on the sibling `pinion` checkout: its
  # resolution changes when SOMEBODY ELSE commits, so `--locked` there is a gate
  # that goes red for another repository's work — R1110's defect exactly — and a
  # tracked lockfile there is a file every run of this script rewrites. It had
  # both: `studio/Cargo.lock` was stale in the tree for an unknown number of
  # rounds and was nearly swept into an unrelated commit.
  #
  # The answer is asked of the manifests, not written down: a workspace whose
  # path dependencies all land inside this checkout is one whose lockfile this
  # repository can pin, and nothing else is.
  foreign=$(outside_dependencies "$ws" | cut -f2 | sort -u | tr '\n' ' ')
  if [[ -n "${foreign// /}" ]]; then
    locked=()
    echo "[side-workspaces] LOCK $ws foreign — it resolves against trees this" \
      "repository does not own, so its lockfile is not this repository's to pin:" \
      "${foreign% }"
  else
    locked=(--locked)
    echo "[side-workspaces] LOCK $ws ours"
  fi
  # AFTER the ownership line and not before it. Ownership is the same answer on
  # every machine — `realpath -m` does not need the tree to exist — so a runner
  # that cannot COMPILE `studio` can still say whose resolution it records, and
  # the gate that asks whether an unpinnable workspace tracks a lockfile has to
  # be able to ask it there. Skipping first would make that fact visible only on
  # the one machine holding the sibling checkout.
  absent=$(missing_siblings "$ws" | sort -u | tr '\n' ' ')
  if [[ -n "${absent// /}" ]]; then
    echo "[side-workspaces] SKIP $ws — its path dependencies leave this repository" \
      "and are not on this machine: ${absent% }"
    skipped+=("$ws")
    continue
  fi
  # EVERY COMMAND THIS SCRIPT RUNS, DECLARED ONCE. `--list` prints them and the
  # run below executes the same words, because a reader that has to know what
  # this script runs must not be re-deriving it: R1084's gate asks which tests
  # every CI command executes, and a separate workspace's suite is a command
  # only this file knows. A second spelling drifts the first time a flag changes.
  #
  # R1115 widened this from the suite alone to all five. The suite was declared
  # and the four around it were not, so the gate that reads what this repository
  # runs could see the one command that already carried `--locked` and none of
  # the four that did not — and the four are the ones that run first.
  #
  # `--no-fail-fast`, because a gate that stops at the first failing target
  # reports a smaller number than the truth and somebody fixes to it. This gate
  # did exactly that on its first run: it said `bench` had 6 failures, and the
  # 6 were one target's — there were 18.
  suite=(cargo test --manifest-path "$ws/Cargo.toml" "${locked[@]}" --no-fail-fast)
  # `--list` answers WHICH workspaces are checkable on this machine and stops.
  # R1082's feature gate needs exactly that answer and had written its own: on a
  # CI runner, `cargo metadata` for `studio` dies on the sibling `../pinion`
  # checkout that is not there, and the gate turned main red asking a question
  # this script had already solved twelve lines up. One definition, consumed
  # rather than restated — the same correction R1066 made for fmt and clippy.
  if $list_only; then
    echo "[side-workspaces] CHECKABLE $ws"
  else
    echo "[side-workspaces] CHECK $ws — fmt, clippy, item citations, blind waits$($lint_only || echo ', tests')"
  fi
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
    # NO `--locked` HERE AND THAT IS MEASURED, NOT AN OVERSIGHT: `cargo fmt`
    # rejects the flag outright, and it is the one subcommand this repository
    # issues that leaves a disagreeing lockfile alone. `locked_resolution_smoke`
    # asks cargo which is which rather than trusting this sentence.
    if ! declare_and_run fmt cargo fmt --manifest-path "$member" --check; then
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
  declare_and_run clippy \
    cargo clippy --manifest-path "$ws/Cargo.toml" "${locked[@]}" --all-targets -- -D warnings
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
  # `--locked` UNCONDITIONALLY, and not `"${locked[@]}"`: this command resolves
  # the GATE's own workspace, which is always one of this repository's, whatever
  # the workspace it is pointed at turns out to be.
  # THE SAME THREE CODES AS THE TWO GATES BELOW, and this arm was the one that
  # never got them. `item-citations` has answered 0 / 1 / 2 since it was written
  # — 2 being "a package does not check, so nothing can be said about the
  # citations in it" — and this caller collapsed 1 and 2 into the finding.
  #
  # MEASURED, and that is why it is here rather than in a carry. A concurrent
  # prune of this repository's ONE shared build directory (`.cargo/config.toml`
  # points every workspace at `<repo>/target`) deleted artifacts under a running
  # gate; `librocksdb-sys` then would not compile, the gate correctly answered 2
  # and said so, and this line printed `bench carries a citation that names no
  # item`. A reader sent to hunt a bad citation in `bench` would find none,
  # because there is none. That sentence is the recorded shape of Z15 — a remote
  # red that reads as a defect in the tree.
  declare_and_run citations cargo run -q --manifest-path "$citations" --locked \
    --bin item-citations -- --workspace "$root/$ws/Cargo.toml" || citations_verdict=$?
  case "${citations_verdict:-0}" in
    0) ;;
    1)
      echo "[side-workspaces] $ws carries a citation that names no item —" \
        "fix: cargo run -q --manifest-path tools/item-citations/Cargo.toml" \
        "--bin item-citations -- --workspace $ws/Cargo.toml" >&2
      exit 1
      ;;
    *)
      echo "[side-workspaces] the item-citation gate could not read $ws" \
        "(exit ${citations_verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
  unset citations_verdict
  # R1081 — in test code a wait ends on a condition and its budget is named.
  # The root workspace gets this from its own CI step and the pre-commit hook,
  # and pointing it ONLY at the root is the hole R1080 closed for the citation
  # gate one round earlier. It is pointed at a manifest for the same reason that
  # one is: `bench` is where the class was first found (R1073's red main), and
  # `tools/injection-harness` deliberately uses processes, signals and real
  # time, so it is the likeliest place for the next one.
  #
  # Resolved from THIS SCRIPT's checkout, run against the WORKING DIRECTORY —
  # two different trees whenever this repository's gate runs over another one.
  waits="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/tools/blind-waits/Cargo.toml"
  if [[ ! -f "$waits" ]]; then
    echo "[side-workspaces] the blind-wait gate is missing at $waits" >&2
    exit 1
  fi
  # Exit 1 and exit 2 are different answers — "these sites break the law" and
  # "I could not read enough of this tree to have one" — and one message for
  # both mislabels whichever it did not mean.
  declare_and_run waits cargo run -q --manifest-path "$waits" --locked \
    --bin blind-waits -- --workspace "$root/$ws/Cargo.toml" || waits_verdict=$?
  case "${waits_verdict:-0}" in
    0) ;;
    1)
      echo "[side-workspaces] $ws carries a wait that ends on a clock, or a" \
        "budget spelled where nobody reviews it —" \
        "fix: cargo run -q --manifest-path tools/blind-waits/Cargo.toml" \
        "--bin blind-waits -- --workspace $ws/Cargo.toml" >&2
      exit 1
      ;;
    *)
      echo "[side-workspaces] the blind-wait gate could not read $ws" \
        "(exit ${waits_verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
  unset waits_verdict

  # And every environment a spawned program reads is one its test names (R1182),
  # under the same three-code contract. This is the gate the `main` R1181 found
  # red would have caught at the fixture: a side workspace's own suite is where
  # the stale list lived.
  named="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/tools/named-environment/Cargo.toml"
  if [[ ! -f "$named" ]]; then
    echo "[side-workspaces] the named-environment gate is missing at $named" >&2
    exit 1
  fi
  declare_and_run named cargo run -q --manifest-path "$named" --locked \
    --bin named-environment -- --workspace "$root/$ws/Cargo.toml" || named_verdict=$?
  case "${named_verdict:-0}" in
    0) ;;
    1)
      echo "[side-workspaces] $ws spawns a program whose environment its test" \
        "leaves to the machine —" \
        "fix: cargo run -q --manifest-path tools/named-environment/Cargo.toml" \
        "--bin named-environment -- --workspace $ws/Cargo.toml" >&2
      exit 1
      ;;
    *)
      echo "[side-workspaces] the named-environment gate could not read $ws" \
        "(exit ${named_verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
  unset named_verdict

  # And a path built from the shared temp root names the process (R1193), under
  # the same three-code contract. THIS IS WHERE THAT LAW'S WHOLE POPULATION
  # LIVES: the hook's Gate 5f points at the ROOT workspace, which reaches the
  # temp root nowhere, while all eleven sites are in these crates — the six
  # R1175 repaired among them. A gate wired only at the root would have been
  # green about a tree it never read.
  owner="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/tools/unowned-scratch/Cargo.toml"
  if [[ ! -f "$owner" ]]; then
    echo "[side-workspaces] the scratch-ownership gate is missing at $owner" >&2
    exit 1
  fi
  declare_and_run scratch cargo run -q --manifest-path "$owner" --locked \
    --bin unowned-scratch -- --workspace "$root/$ws/Cargo.toml" || owner_verdict=$?
  case "${owner_verdict:-0}" in
    0) ;;
    1)
      echo "[side-workspaces] $ws builds a path under the shared temp root that names" \
        "no owner, so two runs share it —" \
        "fix: cargo run -q --manifest-path tools/unowned-scratch/Cargo.toml" \
        "--bin unowned-scratch -- --workspace $ws/Cargo.toml" >&2
      exit 1
      ;;
    *)
      echo "[side-workspaces] the scratch-ownership gate could not read $ws" \
        "(exit ${owner_verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
  unset owner_verdict

  if ! $lint_only; then
    declare_and_run suite "${suite[@]}"
  fi
  checked+=("$ws")
done

echo "[side-workspaces] checked ${#checked[@]} (${checked[*]-}), skipped ${#skipped[@]} (${skipped[*]-})"
if [[ ${#checked[@]} -eq 0 ]]; then
  echo "[side-workspaces] this gate reached no workspace at all — a green run that" \
       "checked nothing is the failure it exists to prevent" >&2
  exit 1
fi
