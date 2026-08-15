#!/usr/bin/env bash
# fmt — format every package manifest in this repository, from ONE population.
#
# WHY IT EXISTS. `cargo fmt --all` at the root formats the ROOT workspace, and
# this repository has twenty: the other nineteen carry their own `[workspace]`
# precisely so the root's commands never reach them. So the command a person
# types to FIX formatting covered a smaller population than the command that
# CHECKS it, and the gap showed up as a gate rejection with no single command
# behind it — R1205 measured the reading side of that (the check now answers in
# the first 2% of a side-gate run) and named this side as undone work rather
# than as a limit. This is that work.
#
# IT DOES NOT KNOW WHICH WORKSPACES EXIST, and that is the whole design. The
# discovery walk lives in `scripts/check-side-workspaces.sh`, which already
# DECLARES every command it runs (`--list`, the shape `tools/ci-plan` reads).
# This script takes the formatting commands that gate declares and runs the same
# words with `--check` removed, so the set it writes is the set the gate reads
# BY CONSTRUCTION and not by two lists agreeing. A workspace added to this
# repository is added to both at once; the law that says so is
# `formatting_population` in the root suite.
#
# AND IT REFUSES A CHECK THAT IS NOT CHECKING. If a declared formatting command
# does not carry `--check`, the gate is running a formatter and reading its exit
# code as a verdict — this script would then be the only thing that noticed, so
# it says so rather than quietly dropping the flag it never found.
#
# Usage:
#   scripts/fmt.sh          # format the root workspace and every separate one
#   scripts/fmt.sh --list   # print the commands and run nothing
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
lister="$here/scripts/check-side-workspaces.sh"
if [[ ! -x "$lister" ]]; then
  echo "[fmt] the workspace lister is missing at $lister — it owns the" \
    "population this script writes, so there is nothing to derive" >&2
  exit 1
fi

list_only=false
for argument in "$@"; do
  case "$argument" in
    --list) list_only=true ;;
    *) echo "[fmt] unknown argument $argument" >&2; exit 2 ;;
  esac
done

# Print a command in the one shape a reader parses, then run those same words —
# the rule this repository's gates state, so a flag cannot be in the report and
# out of the run.
run() {
  echo "[fmt] COMMAND $*"
  if $list_only; then
    return 0
  fi
  "$@"
}

# THE ROOT WORKSPACE, by cargo's own definition of its members. `--all` is not a
# second spelling of anything: it is cargo answering "which packages are in this
# workspace", which is the one question this script must not answer itself.
cd "$here"
run cargo fmt --all

# EVERY SEPARATE WORKSPACE, from the one program that discovers them. Parsed
# from the gate's own declaration line — `[side-workspaces] COMMAND <ws> fmt
# <words…>` — so the words run here are the words it checks with.
#
# THE ONE THING THIS CANNOT SURVIVE is a manifest path containing a space: the
# declaration is space-separated, so it would be read as two words. This
# repository has no such path and the gate's own `--list` output has the same
# limit, which is why it is stated here rather than worked around with a second
# output format nobody else reads.
formatted=0
while IFS= read -r line; do
  read -ra field <<<"$line"
  [[ ${field[0]:-} == "[side-workspaces]" ]] || continue
  [[ ${field[1]:-} == COMMAND ]] || continue
  [[ ${field[3]:-} == fmt ]] || continue
  words=("${field[@]:4}")
  checking=false
  writing=()
  for word in "${words[@]}"; do
    if [[ $word == --check ]]; then
      checking=true
    else
      writing+=("$word")
    fi
  done
  if ! $checking; then
    echo "[fmt] the gate's formatting command for ${field[2]:-?} does not carry" \
      "--check, so what it reports is a formatter's exit code and not a" \
      "verdict: ${words[*]}" >&2
    exit 1
  fi
  formatted=$((formatted + 1))
  run "${writing[@]}"
done < <("$lister" --list)

if [[ $formatted -eq 0 ]]; then
  echo "[fmt] the gate declared no formatting command at all — a run that" \
    "formatted only the root workspace is the gap this script exists to close," \
    "and it looks exactly like a clean one" >&2
  exit 1
fi

echo "[fmt] the root workspace and $formatted package manifest(s) of the" \
  "separate workspaces the gate checks"
