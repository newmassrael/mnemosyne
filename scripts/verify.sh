#!/usr/bin/env bash
# verify.sh — run a verification command so its result ALWAYS reflects the latest
# source, with the FULL log preserved every time. Built after R743, where two
# problems combined to turn a deterministic regression into a mis-diagnosed
# "flake":
#   (1) a lossy `cargo test | grep panicked` discarded the actual assertion
#       payload (fixed here: full output is tee'd to a retained log);
#   (2) overlapping `cargo` invocations on one target/ corrupted the fingerprint
#       cache, so cargo SKIPPED a rebuild and a STALE binary ran (fixed here: an
#       flock serialises every verify.sh cargo run, and --fresh force-deletes the
#       changed crates' artifacts so cargo MUST rebuild them).
#
# Usage:
#   scripts/verify.sh [--fresh] [--no-fresh] [--label <name>] -- <command...>
#   scripts/verify.sh cargo test --workspace
#
# --fresh (DEFAULT): `cargo clean -p <package>` for every package the working
#   tree has changed against HEAD, in the workspace the wrapped command builds —
#   targeted, so only the changed packages and their dependents rebuild (not a
#   full clean). Guarantees no stale artifact of the code under test survives,
#   even if a past concurrent run corrupted it. `tools/stale-artifacts` is what
#   decides it, for the reason in that program's header: this repository has
#   twenty-four workspaces, so WHICH one a run has to freshen is decided at
#   runtime, and a `cargo clean --manifest-path "$variable"` written here is a
#   command no gate can place. Until R1257 the branch was eight lines of shell
#   asking git about `crates/` — the ROOT workspace's member directory — and
#   `check-side-workspaces.sh` therefore passed --no-fresh for all twenty-three
#   others: R743's RECOVERY half reached none of them.
# --no-fresh: skip the pass (rely on the flock + cargo's own fingerprinting;
#   use when the tree is a fresh clone, which is what the workflows are).
#
# Always: an flock on target/.verify.lock serialises verify.sh runs; the full
# combined stdout+stderr is written to target/verify-logs/<utc-ts>-<label>.log
# (target/ is gitignored) and echoed to the caller as it arrives; the WRAPPED
# command's real exit status is returned — `wait` on the process this script
# started, never a pipeline's — so CI/callers still see a genuine non-zero.
#
# THE LOCK IS RE-ENTRANT ACROSS THE PROCESS TREE (R1196), and that is a
# requirement rather than a convenience. `scripts/check-side-workspaces.sh` runs
# each separate workspace's suite through this script, so wrapping that gate in
# it — `scripts/verify.sh -- scripts/check-side-workspaces.sh`, which is what
# RULEBOOK's checklist asks a round to do — puts one of these inside another. An
# flock is held by the OPEN FILE DESCRIPTION, so the inner run opening the same
# path would wait for a lock its own ancestor holds and never be woken: a
# deadlock, not a slow gate. `VERIFY_LOCKS_HELD` is the exported list of locks
# this process tree already holds, and a lock named in it is not taken again.
# It is a list of PATHS rather than a flag because this repository's gates run
# over ANOTHER tree as well as their own, and a marker that said only "held"
# would skip the lock for a second tree that nothing has locked at all.
#
# AND THE LOCK LIVES IN THIS PROCESS AND IN NOTHING BELOW IT (R1235). The same
# sentence — an flock is held by the open file description — has a second edge:
# a descriptor is COPIED INTO EVERY CHILD unless something closes it, so every
# process this script started held the build lock too, and a process that
# outlived the script went on holding it after the script was gone. Measured
# 2026-08-18: a case in `tools/one-machine` ended a child whose background
# `sleep 600` survived it, the side-workspace gate ran that suite, and the
# `git push` behind it stopped at `acquiring build lock` until the leftover was
# found with `fuser` and killed by hand. Nothing was red. The body below is a
# group with `9>&-` so the descriptor is closed for everything this script
# starts; re-entrancy is untouched, because what a nested run reads is the
# exported VARIABLE and never an inherited descriptor.
#
# AND THE SAME LEFTOVER REACHES THIS WRAPPER'S OUTPUT (R1239), which the lock's
# repair does not touch and which is not the same failure. A leftover holding the
# descriptor it was given for stdout keeps `tee` from ever reaching end of file,
# so the wrapper does not finish until the leftover does — a HANG rather than a
# held lock. So nothing here hands a command a pipe: see `run_into_log` below.
#
# AND, since R1194, every run is judged for what it COVERED: a `cargo test` that
# stopped at the first failing target reports a smaller number than the truth,
# and this wrapper now says so instead of leaving it for CI a round later. A
# green command whose run did not cover every target it compiled exits 1 here; a
# run this cannot be told about (`-q` prints no `Running` line) exits 2, which is
# a refusal rather than a finding. See tools/unreported-targets.
#
# NOTE: this only serialises cargo runs launched THROUGH verify.sh. A subagent
# that runs cargo in the SAME target/ still contends — spawn cargo-running agents
# with worktree isolation (their own target/) so they never race this one.
set -uo pipefail

fresh=1
label=""
print_logdir=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --fresh) fresh=1; shift ;;
    --no-fresh) fresh=0; shift ;;
    --label) label="${2:-}"; shift 2 ;;
    --print-logdir) print_logdir=1; shift ;;
    --) shift; break ;;
    -*) echo "verify.sh: unknown flag $1" >&2; exit 2 ;;
    *) break ;;
  esac
done

logdir="${VERIFY_LOGDIR:-target/verify-logs}"

# WHERE THIS WRITES, ASKED RATHER THAN READ (R1199). The records under
# `target/` are bounded by `tools/scratch-budget`, and the law that every
# directory this repository writes records into is one something collects has to
# know which directory THIS program uses. Matching the expansion above out of
# this file would be a second definition of it, correct until the day somebody
# changes the default — and then answering a path nothing writes to, which is a
# directory that is always within its budget.
#
# BEFORE THE `mkdir` AND BEFORE THE LOCK, deliberately: asking a program where
# it writes must not make it write, and a law that has to take this tree's build
# lock to ask a question is one that cannot be asked while a build is running.
if [[ $print_logdir == 1 ]]; then
  printf '%s\n' "$logdir"
  exit 0
fi

if [[ $# -eq 0 ]]; then
  echo "usage: scripts/verify.sh [--fresh|--no-fresh] [--label <name>] -- <command...>" >&2
  echo "       scripts/verify.sh --print-logdir" >&2
  exit 2
fi

mkdir -p "$logdir"
# ABSOLUTE, because it is also the identity compared against `VERIFY_LOCKS_HELD`
# — two trees have a `target/.verify.lock` each and they are two locks.
lock="$(pwd)/target/.verify.lock"

# THE PROGRAMS THIS SCRIPT RUNS COME FROM THIS SCRIPT'S CHECKOUT and the tree
# they act on is the WORKING DIRECTORY — two different trees whenever this
# repository's wrapper is used over another one. Resolved ONCE, here, because
# three of them need it now: the freshness pass runs BEFORE the command and the
# two gates run after it, and a second `cd`-and-`pwd` is a second chance to
# answer differently.
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -z "$label" ]]; then
  label="$(printf '%s' "$*" | tr -c 'A-Za-z0-9._-' '-' | cut -c1-60)"
fi
ts="$(date -u +%Y%m%dT%H%M%SZ)"
# THE PROCESS IS IN THE NAME (the R1193 law, applied where it also holds). The
# stamp has one-second resolution, so two runs sharing a label and a second
# would APPEND to one file — and the coverage gate below, reading a log holding
# two runs, would find targets in it that this command does not build and
# correctly refuse to judge. A name nobody else can produce costs nothing.
log="$logdir/${ts}-${label}-$$.log"

# Serialise: no two verify.sh cargo runs touch target/ concurrently — unless
# this process tree is already holding this tree's lock, in which case taking it
# again is a deadlock rather than a guarantee (see the header).
if [[ ":${VERIFY_LOCKS_HELD:-}:" == *":$lock:"* ]]; then
  echo "[verify] build lock ($lock) already held by this process tree — not re-taking it."
  outermost=0
else
  exec 9>"$lock"
  echo "[verify] acquiring build lock ($lock) ..."
  flock 9
  echo "[verify] lock held."
  export VERIFY_LOCKS_HELD="${VERIFY_LOCKS_HELD:+$VERIFY_LOCKS_HELD:}$lock"
  outermost=1
fi

# NOTHING BELOW THIS LINE CAN SEE THE LOCK (R1235 — the header's fourth
# paragraph has the measurement). `9>&-` closes fd 9 for the whole group, so no
# program this script starts inherits the build lock and no process one of them
# leaves behind can hold it after this script is gone.
#
# THE GROUP RATHER THAN THE ONE LINE THAT RUNS THE COMMAND, because the property
# belongs to the LOCK and not to whatever is being verified: a program added
# here later would otherwise re-open the same hole in silence. Bash restores
# fd 9 when the group ends, so the lock is still this process's until it exits,
# and `9>&-` where fd 9 was never opened — the re-entrant branch above — is not
# an error. Both were measured before this was written.
{

# EVERY RUN THIS SCRIPT MAKES REACHES BOTH THE LOG AND THE CALLER, AND NOT
# THROUGH A PIPE (R1239).
#
# `cmd 2>&1 | tee -a "$log"` is the obvious spelling, and it makes this
# wrapper's lifetime somebody else's. `tee` ends when its input reaches END OF
# FILE, which is when the LAST holder of the write end lets go — so a command
# that returns at once while leaving a process behind holds this wrapper open
# for as long as that leftover lives. Measured 2026-08-18, on the same command
# with and without the leftover's stdio redirected away: 4002 ms against 2 ms.
# That is a hang and not a block, and it is the second edge of the leftover the
# lock's repair above is about.
#
# A FILE CANNOT BE HELD OPEN AGAINST ANYBODY. The command writes to the log
# directly, so what is on disk is complete the moment the command exits however
# many descriptors it left behind; a follower started at the log's current end
# echoes those same bytes to the caller as they arrive and stops when the
# command does. The status is then the command's OWN — `wait` answers about the
# process this script started, where `${PIPESTATUS[0]}` answers about a pipeline
# whose other member is `tee`, which is why the pipe could not simply be
# redirected away.
#
# `-s 0.05` BOUNDS HOW LONG THE FOLLOWER OUTLIVES THE COMMAND: it is the
# interval at which `tail` re-checks the pid, and coreutils reads the file once
# more after seeing it dead, so nothing written before the exit is lost.
# Measured: 59 ms end to end for the leftover case that cost 4002 ms, with every
# one of 2002 written lines present on BOTH sides.
#
# STDIN IS HANDED ON DELIBERATELY. A shell with job control off gives an
# asynchronous command /dev/null for input, so an unqualified `&` here would
# quietly take the caller's stdin away from whatever is being verified —
# measured, the same command reads nothing at all. `verify_stdin` is that
# descriptor duplicated BEFORE the `&`, which is the only place it can be: the
# /dev/null assignment happens before any redirection written on the command
# itself, so `<&0` there would duplicate /dev/null onto itself.
#
# THIS ONE IS LEFT OPEN FOR CHILDREN ON PURPOSE, which the paragraph about the
# lock says nothing here should be. It is a DUPLICATE OF THEIR OWN STDIN: every
# child already holds that open file description as fd 0, so nothing is reachable
# through this descriptor that was not already, and nothing waits on it. The
# number is auto-allocated rather than written out so that it cannot collide
# with fd 9 or with a descriptor an injection into this file names.
#
# AND THE COMMAND IS HANDED OVER AFTER A BARE `--`, which is not a convention
# invented here. `tools/ci-plan` reads every cargo command this repository
# writes, and it peels a carrier at the bare marker rather than keeping a list
# of known wrapper names — deliberately, because a stale list is silent where an
# unreadable command is loud. A cargo command run through this function without
# the marker is one no gate can see: measured 2026-08-18, the law that this
# script asks `tools/unreported-targets` what a run covered went red the moment
# the call gained a word in front of `cargo`.
exec {verify_stdin}<&0
run_into_log() {
  local before child
  if [[ "${1:-}" != "--" ]]; then
    echo "verify.sh: run_into_log needs a bare -- before the command it runs" >&2
    exit 2
  fi
  shift
  before="$(wc -c <"$log")"
  "$@" >>"$log" 2>&1 <&"$verify_stdin" &
  child=$!
  tail -c "+$((before + 1))" -f -s 0.05 --pid="$child" "$log"
  wait "$child"
}

echo "[verify] cmd: $*"
echo "[verify] log: $log"
{
  echo "# verify.sh $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# cmd: $*"
  echo "# fresh=$fresh"
  echo "# cwd: $(pwd)"
  echo
} >>"$log"

# NO ARTIFACT OF CODE THIS TREE HAS CHANGED SURVIVES INTO THE RUN THAT JUDGES IT
# (R743's recovery half, reaching every workspace since R1257).
#
# WHAT IT CLEANED IS IN THE LOG BECAUSE THE PASS SAYS SO, which is why the
# header above no longer carries a `cleaned=[…]` of its own. That field existed
# because the shell had to ask the question before opening the log in order to
# record the answer anywhere; the program prints its own numbers — how many
# paths the tree changed, how many packages the workspace has, and each package
# it cleaned — and `run_into_log` puts them in the same log as everything else.
# One record rather than two spellings of one, and the one that is left is the
# one that cannot say something the pass did not do.
#
# A REFUSAL FAILS THE VERIFICATION, exactly as the two gates below do. A
# freshness pass that could not run leaves precisely the artifact it exists to
# remove, and a run that continued past it would then be judged by a binary
# built from source this tree no longer holds — R743, silently, with a green
# line where the warning should be.
#
# THE MANIFEST IS WRITTEN OUT IN THE COMMAND for the reason spelled at the
# coverage gate below: `ci-plan` can place a cargo command only when the literal
# tail of its `--manifest-path` names a directory.
if [[ "$fresh" == 1 ]]; then
  run_into_log -- cargo run -q --locked --manifest-path "$here/tools/stale-artifacts/Cargo.toml" \
    --bin stale-artifacts -- --at . -- "$@"
  fresh_verdict=$?
  if [[ "$fresh_verdict" -ne 0 ]]; then
    echo "[verify] the freshness pass did not run (exit $fresh_verdict); its own" \
      "message is above. The command below would be judged with whatever artifacts" \
      "this tree already had" >&2
    echo "[verify] exit=2 log=$log"
    exit 2
  fi
fi

run_into_log -- "$@"
status=$?

# EVERY TEST TARGET THIS RUN COMPILED IS ONE IT REPORTED A RESULT FOR (R1194).
#
# `cargo test` stops at the first target that fails, so the number a reader sees
# is one target's and the ones behind it were never asked. R1177 shipped three
# defects that way and CI found them a round later; the carry it left named the
# missing shape as "a check on the command a round verifies with", which is this
# script. `--no-fail-fast` is DISCIPLINE — a habit that has to be remembered on
# every invocation, and that says nothing at all when a target fails to COMPILE.
# The gate below asks cargo what the command compiled and holds it against what
# the log says ran, so a run that covered less than it looks like says so here
# rather than in CI.
#
# THE PROGRAM COMES FROM THIS SCRIPT'S CHECKOUT and the run it judges happened
# in the WORKING DIRECTORY — two different trees whenever this repository's
# wrapper is used over another one. `here` is resolved once, near the top.
#
# THE MANIFEST IS WRITTEN OUT IN THE COMMAND, not bound to a variable first, and
# that is a requirement rather than a style: `ci-plan` reads this file's source
# to answer which workspace every cargo command in this repository resolves, and
# it can read a path a shell assembles only when the LITERAL tail names a
# directory. `--manifest-path "$coverage"` has no readable tail at all, so the
# gate answers "cannot say which workspace" — and a command it cannot place is
# one it cannot check for `--locked`. Measured: it turned the root suite red
# (`locked_resolution_smoke`, tally `Unreadable: 1`) the first time this was
# written the other way. Spelling it once, here, is also why there is no
# separate existence check — a missing manifest makes `cargo run` fail, which is
# the `*)` branch below, and it says so.
run_into_log -- cargo run -q --locked --manifest-path "$here/tools/unreported-targets/Cargo.toml" \
  --bin unreported-targets -- --log "$log" --at . -- "$@"
coverage_verdict=$?
case "$coverage_verdict" in
  0) ;;
  1)
    # A COMMAND THAT ALREADY FAILED KEEPS ITS OWN STATUS. Both sentences are
    # printed either way: the hidden targets matter most when the run was red,
    # because that is when the reader is about to fix to a partial number.
    echo "[verify] the run above did not cover every target it compiled —" \
      "re-run with --no-fail-fast, or repair what stopped it reaching them" >&2
    if [[ "$status" -eq 0 ]]; then status=1; fi
    ;;
  2)
    echo "[verify] the target-coverage gate has NO VERDICT about this run" \
      "(its own message is above). That is not the same as a covered one." >&2
    if [[ "$status" -eq 0 ]]; then status=2; fi
    ;;
  *)
    echo "[verify] the target-coverage gate could not be started" \
      "(exit $coverage_verdict); its own message is above" >&2
    if [[ "$status" -eq 0 ]]; then status=2; fi
    ;;
esac

# AND THE RECORD THIS RUN JUST WROTE IS ONE SOMETHING COLLECTS (R1199).
#
# Nothing reclaimed these until this round. `scripts/gc` delegates the whole of
# `target/` to cargo-sweep, which knows what an ARTIFACT is and nothing else:
# measured, `cargo sweep --installed --dry-run` answered `Would clean: nothing`
# on a tree holding 1537 record files and 86 MB, the oldest of them sixteen days
# old. A collector nobody runs is the state that produced that, so the caller is
# here — the one program every verification in this repository goes through —
# rather than left to a person remembering the gc.
#
# ONLY THE OUTERMOST RUN IN A TREE COLLECTS. `check-side-workspaces.sh` puts
# eighteen of these inside one another, and each of the inner ones would survey
# the same directories again for nothing. The lock already answers which one
# this is: the run that TOOK it is the one whose end is the end of the whole
# verification, and a nested run over ANOTHER tree takes that tree's lock and
# correctly collects there.
#
# THE PROGRAM COMES FROM THIS SCRIPT'S CHECKOUT and the tree it collects in is
# the WORKING DIRECTORY — the same two-tree rule as the gate above, and the
# manifest path is spelled out for the same reason: `ci-plan` can place a cargo
# command only when the literal tail names a directory.
if [[ "$outermost" == 1 ]]; then
  run_into_log -- cargo run -q --locked --manifest-path "$here/tools/scratch-budget/Cargo.toml" \
    --bin scratch-budget -- --at .
  scratch_verdict=$?
  if [[ "$scratch_verdict" -ne 0 ]]; then
    # A REFUSAL IS NOT A FINDING ABOUT THE RUN, and it is not nothing either: a
    # collector that stops working leaves growth nobody sees, because `target/`
    # is gitignored and the build machine prunes its own copy. It fails the
    # verification for the same reason the coverage gate's refusal does — a
    # green line is what this defect looked like for sixteen days.
    echo "[verify] the record collector did not run (exit $scratch_verdict); its own" \
      "message is above. Nothing else bounds what this run wrote to $logdir" >&2
    if [[ "$status" -eq 0 ]]; then status=2; fi
  fi
fi

echo "[verify] exit=$status log=$log"
if [[ "$status" -ne 0 ]]; then
  echo "[verify] --- failure lines (full log retained at $log) ---"
  grep -nE "error\[|error:|panicked|test result: FAILED|FAILED| failed" "$log" | tail -40 || true
fi
exit "$status"

} 9>&-
