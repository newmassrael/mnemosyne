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
# --fresh (DEFAULT): `cargo clean -p <crate>` for every crate with uncommitted
#   changes vs HEAD before running — targeted, so only the changed crates and
#   their dependents rebuild (not a full clean). Guarantees no stale artifact of
#   the code under test survives, even if a past concurrent run corrupted it.
# --no-fresh: skip the clean (rely on the flock + cargo's own fingerprinting;
#   use when nothing changed and you want speed).
#
# Always: an flock on target/.verify.lock serialises verify.sh runs; the full
# combined stdout+stderr is tee'd to target/verify-logs/<utc-ts>-<label>.log
# (target/ is gitignored); the WRAPPED command's real exit status is returned
# (PIPESTATUS[0], never tee's) so CI/callers still see a genuine non-zero.
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
while [[ $# -gt 0 ]]; do
  case "$1" in
    --fresh) fresh=1; shift ;;
    --no-fresh) fresh=0; shift ;;
    --label) label="${2:-}"; shift 2 ;;
    --) shift; break ;;
    -*) echo "verify.sh: unknown flag $1" >&2; exit 2 ;;
    *) break ;;
  esac
done
if [[ $# -eq 0 ]]; then
  echo "usage: scripts/verify.sh [--fresh|--no-fresh] [--label <name>] -- <command...>" >&2
  exit 2
fi

logdir="${VERIFY_LOGDIR:-target/verify-logs}"
mkdir -p "$logdir"
lock="target/.verify.lock"

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

# Serialise: no two verify.sh cargo runs touch target/ concurrently.
exec 9>"$lock"
echo "[verify] acquiring build lock ($lock) ..."
flock 9
echo "[verify] lock held."

changed_crates=""
if [[ "$fresh" == 1 ]]; then
  changed_crates="$(git diff --name-only HEAD -- crates/ 2>/dev/null \
    | sed -n 's#^crates/\([^/]*\)/.*#\1#p' | sort -u | tr '\n' ' ')"
  if [[ -n "${changed_crates// }" ]]; then
    for c in $changed_crates; do
      echo "[verify] fresh: cargo clean -p $c"
      cargo clean -p "$c" --locked >/dev/null 2>&1 || true
    done
  else
    echo "[verify] fresh: no uncommitted crate changes; nothing to clean."
  fi
fi

echo "[verify] cmd: $*"
echo "[verify] log: $log"
{
  echo "# verify.sh $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# cmd: $*"
  echo "# fresh=$fresh cleaned=[${changed_crates}]"
  echo "# cwd: $(pwd)"
  echo
} >>"$log"

"$@" 2>&1 | tee -a "$log"
status="${PIPESTATUS[0]}"

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
# wrapper is used over another one.
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
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo run -q --locked --manifest-path "$here/tools/unreported-targets/Cargo.toml" \
  --bin unreported-targets -- --log "$log" --at . -- "$@" 2>&1 | tee -a "$log"
coverage_verdict="${PIPESTATUS[0]}"
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

echo "[verify] exit=$status log=$log"
if [[ "$status" -ne 0 ]]; then
  echo "[verify] --- failure lines (full log retained at $log) ---"
  grep -nE "error\[|error:|panicked|test result: FAILED|FAILED| failed" "$log" | tail -40 || true
fi
exit "$status"
