#!/usr/bin/env bash
# census-elsewhere.sh — take this tree's syscall census HERE, on a machine that
# is not the one where the census was written.
#
# WHY THIS EXISTS AT ALL. `tools/outside-reach` decides what this repository's
# suite reaches outside itself, and that answer is a claim about a MACHINE as
# much as about the source. This repository has caught it being wrong four times
# and every one was found somewhere its author was not sitting: a hosted
# runner's `HOME` (R1229), a job cancelled before the verdict could be read
# (R1231), a gcc driver spelling a library path through its own parent and a
# ground list written from memory (R1232), and a trace filter one `fcntl` short
# of the model it feeds, in a population no workstation run reproduces (R1233).
# The arms that found the last two were scripts a person launched by hand. This
# is the same census with a caller: `one-machine --send` puts it on a machine the
# placement program chose, and `one-machine --read` judges what it wrote.
#
# WHAT THIS FILE SPELLS FOR ITSELF: nothing that another program owns.
#
#   * the header line is `one-machine --header`'s, so the machine writing it and
#     the machine reading it use one spelling of what a header is;
#   * the trace filter is `outside-reach --trace-filter`'s, because which
#     syscalls the stream must carry is a fact about that reader's model — R1233
#     is what a second copy of it costs;
#   * the command under the census is `[commands] verify` in
#     `.claude/remote-build.toml`, which is where this repository already says
#     how it is verified on a build machine, and is in the population of the laws
#     that read every cargo command this repository issues.
#
# WHAT IT DOES SPELL is the census wiring itself, and it is spelled the same way
# the hosted job spells it: `strace -o "|reader"` hands the trace to a program
# through a pipe, so the whole suite costs ZERO bytes on disk, and the reader's
# verdict is written to a file because `strace` returns the WRAPPED command's
# status and says nothing about the program on the other end of its pipe.
#
# THE EXIT CODE IS THE CENSUS'S, three-valued like every gate here: 0 clean,
# 1 a reach nothing declares, 2 could not judge. The suite's own status is
# PRINTED rather than folded in, because a census over a run that died early
# read a shorter run than a green one would have, and the reader of this log has
# to be able to tell those apart.
set -uo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo" || exit 2

# THE LOCK, BECAUSE THIS RUN OUTLIVES THE CONNECTION THAT STARTED IT.
#
# Rule 4 of the machine-wide remote-build protocol: two cargo runs in one tree
# corrupt each other's fingerprint cache, so cargo SKIPS a rebuild and a stale
# binary runs. The program that placed this run holds a lock only while it is
# attached, and it detaches here — so between its exit and this run's end the
# tree would look free. `mode=detached` is the declaration that keeps it
# refused AND keeps it from being reaped: an attached lock whose process has
# been orphaned is provably unreadable and may be ended, and this one is neither.
locks="$HOME/.remote-build/locks"
mkdir -p "$locks" || exit 2
lock="$locks/$(basename "$repo").$$"
printf 'pid=%s start=%s jobs=%s note=%s mode=detached\n' \
    "$$" "$(date -Is)" "${RUST_TEST_THREADS:-unset}" "$(basename "$repo")" > "$lock" || exit 2
trap 'rm -f "$lock"' EXIT

# WHO IS ANSWERING, AND ABOUT WHAT — printed FIRST, so a run that dies before
# the census still says which machine and which bytes it was about. A log whose
# header is missing is not read as a census at all, which is the honest reading
# of a run that never got far enough to say.
cargo run -q --locked --manifest-path tools/one-machine/Cargo.toml --bin one-machine \
    -- --repo "$repo" --header || exit 2

# THE READER, BUILT HERE. `--release` because it reads every line of a
# multi-million-line stream while the suite it watches is running; the hosted
# job builds it the same way and for the same reason.
cargo build --release -q --locked --manifest-path tools/outside-reach/Cargo.toml || exit 2
reader="$repo/target/release/outside-reach"
[ -x "$reader" ] || { echo "one-machine census exit=2  (no reader at $reader)"; exit 2; }

filter="$("$reader" --trace-filter)" || exit 2
suite="$(cargo run -q --locked --manifest-path tools/one-machine/Cargo.toml --bin one-machine \
    -- --repo "$repo" --declared-verify)" || exit 2

out="$repo/target/one-machine/census.txt"
mkdir -p "$(dirname "$out")" || exit 2

# THE BUILD DIRECTORY IS RESOLVED, NOT ASSUMED. On the machine that wrote this
# census `target` is a symlink OUT of the tree, and the syscalls name where it
# LANDS; on a machine a dispatch created it is a real directory inside the tree.
# Both are answered by asking the filesystem rather than by knowing which one
# this is.
build="$(readlink -f target 2>/dev/null || echo "$repo/target")"

# `--cwd` is the one fact about a run its own syscalls never state: without it
# every bare name is unresolvable rather than wrong.
strace -f -qq \
    -e trace="$filter" \
    -e status=successful \
    -o "|$reader --repo $repo --build $build --fixture ${TMPDIR:-/tmp} --cwd $repo > $out 2>&1; echo \$? > $out.rc" \
    bash -c "$suite"
echo "one-machine suite exit=$?"

cat "$out" 2>/dev/null || true

# THE VERDICT IS READ BY THE PROGRAM THAT WROTE IT, not by `cat` and `exit`.
# R1231 measured what the shell form costs: on a job that was cancelled before
# the reader wrote anything, `exit "$(cat …)"` died with a message about `exit`
# and nothing about the census — so "the census answered no" and "the census
# never got to answer" left this step looking identical.
"$reader" --verdict-of "$out.rc"
verdict=$?
echo "one-machine census exit=$verdict"
exit "$verdict"
