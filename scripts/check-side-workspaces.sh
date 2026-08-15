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
  local began=${EPOCHREALTIME/./} status=0
  "$@" || status=$?
  local ended=${EPOCHREALTIME/./}
  echo "[side-workspaces] TOOK $ws $role $(((ended - began) / 1000))ms"
  return $status
}

# A MEASUREMENT NEEDS A CLOCK, and a clock that is not there must say so. The
# phase order below IS a measurement, so a shell without `EPOCHREALTIME` would
# print a nonsense duration beside every check and the next reader would order
# the phases by it. R1184 is what this line is afraid of: a `2>/dev/null` that
# turned "this rule did not run" into "this rule found nothing".
if [[ -z ${EPOCHREALTIME:-} ]]; then
  echo "[side-workspaces] this gate reports what each check costs and needs a shell" \
    "with EPOCHREALTIME (bash 5) to do it; this is bash ${BASH_VERSION:-unknown}" >&2
  exit 2
fi

# THE PROGRAMS come from THIS script's checkout; the TREE they are pointed at is
# the working directory. Those are two different things and two different trees
# whenever this repository's gate runs over another one — resolving a program
# from `$root` looks for it inside the tree under check, where it does not exist.
# Resolved ONCE rather than per workspace: nineteen identical `cd`s answered the
# same question nineteen times, and a question asked once cannot be asked
# differently the second time.
here=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
verify="$here/scripts/verify.sh"
citation_gate="$here/tools/item-citations/Cargo.toml"
wait_gate="$here/tools/blind-waits/Cargo.toml"
environment_gate="$here/tools/named-environment/Cargo.toml"
scratch_gate="$here/tools/unowned-scratch/Cargo.toml"
executable_gate="$here/tools/written-executable/Cargo.toml"
for program in "$verify" "$citation_gate" "$wait_gate" "$environment_gate" "$scratch_gate" \
  "$executable_gate"; do
  if [[ ! -f "$program" ]]; then
    echo "[side-workspaces] a program this gate runs is missing at $program" >&2
    exit 1
  fi
done
if [[ ! -x "$verify" ]]; then
  echo "[side-workspaces] the run wrapper at $verify is not executable" >&2
  exit 1
fi

# THE PHASE ORDER, AND IT IS A MEASUREMENT RATHER THAN A HABIT. Every check runs
# over EVERY workspace before the next check runs anywhere, cheapest first, and
# "cheapest" was measured on this repository's own 19 separate workspaces —
# twice, because the first two guesses were both wrong.
#
#   role        all 19 workspaces, warm   ONE workspace, after one source edit
#   scratch                      2.17 s   110 ms ->  111 ms
#   waits                        2.58 s   138 ms ->  107 ms
#   fmt         2.81 s (27 manifests)     154 ms ->  117 ms
#   named                       22.97 s  1689 ms -> 1590 ms
#   clippy                       6.38 s   192 ms ->  770 ms
#   citations                   12.32 s   289 ms -> 9158 ms
#
# `executable` arrived after that table and is placed by the SAME key rather
# than by a fresh sweep: it parses `.rs` and compiles nothing, so it belongs in
# the first group whatever its warm total turns out to be. That total, measured
# over 20 workspaces on the build machine when it landed, is 2.84 s — 46 ms for
# a small workspace and 1.7 s for `bench`, which is where the Rust is. The
# second column is not restated for it because the answer is structural: with
# nothing to compile there is nothing for a source edit to invalidate.
#
# THE PRIMARY KEY IS THE SECOND COLUMN, NOT THE FIRST. Four of these read the
# tree and compile nothing, so what they cost does not depend on what the round
# changed. `clippy` and `citations` compile, and `citations` builds
# documentation — which is how a 289 ms check became 9.2 s from a single touched
# file, and 7m21s in Round 1171. A round that runs this gate is by definition a
# round that changed something, so the compile-bound pair goes last whatever
# their warm numbers say; inside each group the order is the warm total, which
# is all that separates them.
#
# WHAT THIS REPLACED was workspace-major, where the cheapest question about the
# LAST workspace waited behind every expensive question about the other
# eighteen: 38.5 s with nothing at all to rebuild, and the whole of a
# multi-minute wait twice in one session (Round 1200, Round 1204).
phases=(scratch executable waits fmt named clippy citations suite)

# What the CHECK line announces is what the loop at the bottom will actually
# run, derived from the one list rather than written out a second time — the
# sentence it replaced said "fmt, clippy, item citations, blind waits" and had
# been missing two checks since R1182 added them.
announced=()
for phase in "${phases[@]}"; do
  if [[ $phase == suite ]] && $lint_only; then
    continue
  fi
  announced+=("$phase")
done

checked=()
skipped=()
declare -A locked_of=()
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
    locked_of["$ws"]=no
    echo "[side-workspaces] LOCK $ws foreign — it resolves against trees this" \
      "repository does not own, so its lockfile is not this repository's to pin:" \
      "${foreign% }"
  else
    locked_of["$ws"]=yes
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
  # `--list` answers WHICH workspaces are checkable on this machine and stops.
  # R1082's feature gate needs exactly that answer and had written its own: on a
  # CI runner, `cargo metadata` for `studio` dies on the sibling `../pinion`
  # checkout that is not there, and the gate turned main red asking a question
  # this script had already solved twelve lines up. One definition, consumed
  # rather than restated — the same correction R1066 made for fmt and clippy.
  if $list_only; then
    echo "[side-workspaces] CHECKABLE $ws"
  else
    echo "[side-workspaces] CHECK $ws — ${announced[*]}"
  fi
  checked+=("$ws")
done

# ── THE CHECKS ───────────────────────────────────────────────────────────────
#
# One function per phase, each pointed at ONE workspace. The loop at the bottom
# calls each of them over EVERY checkable workspace before calling the next —
# the measured order is declared above, and this is why the checks had to leave
# the discovery loop to become functions.
#
# `declare_and_run` reads `$ws` from its caller's frame, so every one of these
# names its parameter `ws` and nothing else.
#
# `${locked_of[$ws]}` was decided ONCE, in the pass above, where the ownership
# question is asked. Deciding it here would ask it six times per workspace and
# give six chances to answer differently.

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
#
# IT COLLECTS RATHER THAN EXITS, and it is the only phase here that does. The
# reason is the measurement above: finishing this one costs 2.8 s over the whole
# repository, so a run that stops at the first unformatted manifest is buying
# nothing and reporting a smaller number than the truth — the very thing
# `--no-fail-fast` is on the suite for. It matters most HERE because these
# failures come in groups: the command a person types to fix formatting
# (`cargo fmt --all` at the root) reaches a SMALLER population than this check
# does, so every side manifest an edit touched is unformatted at once.
run_fmt() {
  local ws=$1 member formatted=0
  while IFS= read -r -d '' member; do
    grep -qE '^\[package\]' "$member" || continue
    formatted=$((formatted + 1))
    # NO `--locked` HERE AND THAT IS MEASURED, NOT AN OVERSIGHT: `cargo fmt`
    # rejects the flag outright, and it is the one subcommand this repository
    # issues that leaves a disagreeing lockfile alone. `locked_resolution_smoke`
    # asks cargo which is which rather than trusting this sentence.
    if ! declare_and_run fmt cargo fmt --manifest-path "$member" --check; then
      unformatted+=("$member")
    fi
  done < <(find "$ws" -name Cargo.toml -not -path '*/target/*' -print0 | sort -z)
  if [[ $formatted -eq 0 ]]; then
    echo "[side-workspaces] $ws has no package manifest of its own — a workspace this" \
      "gate formats nothing in is one it is not checking" >&2
    exit 1
  fi
}

run_clippy() {
  local ws=$1 locked=()
  if [[ ${locked_of[$ws]} == yes ]]; then locked=(--locked); fi
  declare_and_run clippy \
    cargo clippy --manifest-path "$ws/Cargo.toml" "${locked[@]}" --all-targets -- -D warnings
}

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
#
# `--locked` UNCONDITIONALLY, and not `"${locked[@]}"`: this command resolves
# the GATE's own workspace, which is always one of this repository's, whatever
# the workspace it is pointed at turns out to be.
#
# THE SAME THREE CODES AS THE GATES BELOW, and this arm was the one that never
# got them. `item-citations` has answered 0 / 1 / 2 since it was written — 2
# being "a package does not check, so nothing can be said about the citations in
# it" — and this caller collapsed 1 and 2 into the finding.
#
# MEASURED, and that is why it is here rather than in a carry. A concurrent
# prune of this repository's ONE shared build directory (`.cargo/config.toml`
# points every workspace at `<repo>/target`) deleted artifacts under a running
# gate; `librocksdb-sys` then would not compile, the gate correctly answered 2
# and said so, and this line printed `bench carries a citation that names no
# item`. A reader sent to hunt a bad citation in `bench` would find none,
# because there is none. That sentence is the recorded shape of Z15 — a remote
# red that reads as a defect in the tree.
run_citations() {
  local ws=$1 verdict=0
  declare_and_run citations cargo run -q --manifest-path "$citation_gate" --locked \
    --bin item-citations -- --workspace "$root/$ws/Cargo.toml" || verdict=$?
  case $verdict in
    0) ;;
    1)
      echo "[side-workspaces] $ws carries a citation that names no item —" \
        "fix: cargo run -q --manifest-path tools/item-citations/Cargo.toml" \
        "--bin item-citations -- --workspace $ws/Cargo.toml" >&2
      exit 1
      ;;
    *)
      echo "[side-workspaces] the item-citation gate could not read $ws" \
        "(exit ${verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
}

# R1081 — in test code a wait ends on a condition and its budget is named.
# The root workspace gets this from its own CI step and the pre-commit hook,
# and pointing it ONLY at the root is the hole R1080 closed for the citation
# gate one round earlier. It is pointed at a manifest for the same reason that
# one is: `bench` is where the class was first found (R1073's red main), and
# `tools/injection-harness` deliberately uses processes, signals and real
# time, so it is the likeliest place for the next one.
#
# Exit 1 and exit 2 are different answers — "these sites break the law" and
# "I could not read enough of this tree to have one" — and one message for
# both mislabels whichever it did not mean.
run_waits() {
  local ws=$1 verdict=0
  declare_and_run waits cargo run -q --manifest-path "$wait_gate" --locked \
    --bin blind-waits -- --workspace "$root/$ws/Cargo.toml" || verdict=$?
  case $verdict in
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
        "(exit ${verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
}

# And every environment a spawned program reads is one its test names (R1182),
# under the same three-code contract. This is the gate the `main` R1181 found
# red would have caught at the fixture: a side workspace's own suite is where
# the stale list lived.
run_named() {
  local ws=$1 verdict=0
  declare_and_run named cargo run -q --manifest-path "$environment_gate" --locked \
    --bin named-environment -- --workspace "$root/$ws/Cargo.toml" || verdict=$?
  case $verdict in
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
        "(exit ${verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
}

# And a path built from the shared temp root names the process (R1193), under
# the same three-code contract. THIS IS WHERE THAT LAW'S WHOLE POPULATION
# LIVES: the hook's Gate 5f points at the ROOT workspace, which reaches the
# temp root nowhere, while all eleven sites are in these crates — the six
# R1175 repaired among them. A gate wired only at the root would have been
# green about a tree it never read.
run_scratch() {
  local ws=$1 verdict=0
  declare_and_run scratch cargo run -q --manifest-path "$scratch_gate" --locked \
    --bin unowned-scratch -- --workspace "$root/$ws/Cargo.toml" || verdict=$?
  case $verdict in
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
        "(exit ${verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
}

# And nothing creates an executable file (R1192), under the same three-code
# contract. THIS IS WHERE THAT LAW'S POPULATION LIVES, as it is for the gate
# above: seven of the ten sites this gate found on its first run are in these
# crates — a `gh` written and chmod'ed by four different fixtures, a recorder
# stub, a suite, and the harness's own copy of itself. `exec` refuses a file some
# process holds open for writing, and the holder is a sibling test's fork rather
# than the thread that wrote it, so every one of them was green alone and a flake
# the moment this script ran them together.
run_executable() {
  local ws=$1 verdict=0
  declare_and_run executable cargo run -q --manifest-path "$executable_gate" --locked \
    --bin written-executable -- --workspace "$root/$ws/Cargo.toml" || verdict=$?
  case $verdict in
    0) ;;
    1)
      echo "[side-workspaces] $ws creates an executable file, which is a program it" \
        "then runs —" \
        "fix: cargo run -q --manifest-path tools/written-executable/Cargo.toml" \
        "--bin written-executable -- --workspace $ws/Cargo.toml" >&2
      exit 1
      ;;
    *)
      echo "[side-workspaces] the written-executable gate could not read $ws" \
        "(exit ${verdict}); its own message is above" >&2
      exit 1
      ;;
  esac
}

# EVERY COMMAND THIS SCRIPT RUNS, DECLARED ONCE. `--list` prints them and the
# run executes the same words, because a reader that has to know what this
# script runs must not be re-deriving it: R1084's gate asks which tests every CI
# command executes, and a separate workspace's suite is a command only this file
# knows. A second spelling drifts the first time a flag changes.
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
#
# AND THROUGH THE WRAPPER THAT JUDGES WHAT THE RUN COVERED (R1196). The flag
# above is DISCIPLINE: it has to be remembered on every invocation, and it says
# nothing at all when a target fails to COMPILE, which stops cargo just the
# same. `scripts/verify.sh` runs the command, keeps the whole log and holds it
# against what cargo says that command compiles (`tools/unreported-targets`),
# so a suite whose verdict covered less than it looks like says so HERE rather
# than in CI a round later.
#
# `--no-fresh` is measured rather than lazy: the wrapper's freshness pass
# cleans the crates under `crates/` that differ from HEAD, which belong to the
# ROOT workspace and never to this one, so asking for it here would rebuild
# somebody else's tree and freshen nothing of this one.
run_suite() {
  local ws=$1 locked=()
  if [[ ${locked_of[$ws]} == yes ]]; then locked=(--locked); fi
  declare_and_run suite \
    "$verify" --no-fresh --label "side-${ws//\//-}" -- \
    cargo test --manifest-path "$ws/Cargo.toml" "${locked[@]}" --no-fail-fast
}

# ── PHASE-MAJOR ──────────────────────────────────────────────────────────────
#
# One check, every workspace, then the next check. The order is the measured one
# declared at the top of this file, and the whole point is that no expensive
# question is asked ANYWHERE until every cheap question has been answered
# EVERYWHERE.
unformatted=()
for phase in "${phases[@]}"; do
  if [[ $phase == suite ]] && $lint_only; then
    continue
  fi
  phase_began=${EPOCHREALTIME/./}
  for ws in "${checked[@]}"; do
    "run_$phase" "$ws"
  done
  # WHAT ONE LAW COST OVER THE WHOLE POPULATION, which is the number the order
  # above was chosen by and the number `TOOK` cannot add up for a reader: a
  # phase that fails part way through never prints this line, so its absence
  # says the law did not finish rather than that it was cheap.
  echo "[side-workspaces] PHASE $phase over ${#checked[@]} workspace(s)" \
    "$(((${EPOCHREALTIME/./} - phase_began) / 1000))ms"
  # The one phase that reports after it finishes rather than at its first
  # failure — see `run_fmt` for why it is the only one that can afford to.
  if [[ $phase == fmt && ${#unformatted[@]} -gt 0 ]]; then
    for member in "${unformatted[@]}"; do
      echo "[side-workspaces] UNFORMATTED $member" >&2
    done
    # ONE COMMAND, NAMED ONCE. Until R1206 each line above carried its own
    # `cargo fmt --manifest-path …`, so a run that found nine of them handed a
    # reader nine commands to paste out of a population only this gate knew.
    # `scripts/fmt.sh` derives that population from THIS script's `--list`, so
    # what it writes is what the lines above check, and `formatting_population`
    # in the root suite is what holds the two against each other.
    echo "[side-workspaces] ${#unformatted[@]} unformatted package manifest(s) across" \
      "the ${#checked[@]} workspace(s) this run covered — every one of them is named" \
      "above, because this check compiles nothing and finishing it is what makes the" \
      "count true rather than a first sighting. fix: scripts/fmt.sh" >&2
    exit 1
  fi
done

echo "[side-workspaces] checked ${#checked[@]} (${checked[*]-}), skipped ${#skipped[@]} (${skipped[*]-})"
if [[ ${#checked[@]} -eq 0 ]]; then
  echo "[side-workspaces] this gate reached no workspace at all — a green run that" \
       "checked nothing is the failure it exists to prevent" >&2
  exit 1
fi
