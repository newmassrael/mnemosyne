# Stand-in programs, tracked rather than written

Every file beside this one stands in for a program a test drives some other
program into calling — `cargo`, `grep`, `gh`, `scripts/mn`, an installed build.
They are TRACKED, with the executable bit git records, and a fixture reaches one
by SYMLINK.

## Why they are files and not string constants

They used to be `const &str` bodies the tests wrote to disk and chmod'ed. That is
a file the process writes and then runs, and `exec` refuses such a file with
`ETXTBSY` for as long as ANY process holds it open for writing — which is a
sibling test's fork inheriting our descriptor, not this thread. Round 1192 met it
in `tools/unread-declaration`, where eleven cases were green alone and one died
the moment eleven ran beside ten other crates. A retry, a lock or a serial
attribute would each treat an ownership problem as a scheduling one.

The rule that came out of it: **nothing here creates an executable file**. The
executable bit belongs to a file cargo built or git tracks, and what varies per
case is DATA the stand-in reads — data cannot be busy. `tools/written-executable`
is the gate that holds the whole repository to it.

## Why they are shell rather than Rust

A cargo-built binary is the sibling repair, and it is what every `tools/*` crate
uses (`src/bin/gh-stub.rs` and friends): those are gate workspaces, where an
extra binary target costs nothing. `mnemosyne-cli` is a product, and a stand-in
for `grep` has no business being installed alongside it — `env!("CARGO_BIN_EXE_…")`
resolves only within a package, so a binary here is a binary that ships.

These stand in for programs that are themselves invoked as shell finds them, and
this repository's hooks and scripts are shell. What matters to the law is that
nothing writes them.

## What varies per case

The environment. Each file's header says which variables it reads.
