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
log="$logdir/${ts}-${label}.log"

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
      cargo clean -p "$c" >/dev/null 2>&1 || true
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

echo "[verify] exit=$status log=$log"
if [[ "$status" -ne 0 ]]; then
  echo "[verify] --- failure lines (full log retained at $log) ---"
  grep -nE "error\[|error:|panicked|test result: FAILED|FAILED| failed" "$log" | tail -40 || true
fi
exit "$status"
