//! What the reader must get right about a trace, asked of fixtures rather than
//! of a run — so these cost milliseconds and hold on every machine.
//!
//! THE FIXTURES ARE REAL LINES. Every one below was copied from a
//! `strace -f -qq -e trace=%file,%process -e status=successful` stream this
//! repository actually produced, ids and all. A simplified fixture is what
//! Round 1096 measured as passing on the day the thing it stood for was broken:
//! the spelling is what decides whether a reader is right.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use outside_reach::{
    judge, read_stream, tree_of, Census, DeclaredReach, Ground, Where, THE_DRIVER,
};

fn ground() -> Ground {
    Ground {
        owned: vec![
            PathBuf::from("/repo"),
            PathBuf::from("/build"),
            PathBuf::from("/fixture"),
        ],
        toolchain: vec![PathBuf::from("/usr"), PathBuf::from("/home/u/.cargo")],
    }
}

fn census_of(trace: &str) -> Census {
    read_stream(Cursor::new(trace.as_bytes()), &ground())
}

/// THE SIBLING IS REACHED BY `newfstatat`, NOT BY `openat`, and a reader that
/// watched opens would answer zero.
///
/// Measured rather than assumed (Round 1229): narrowing the trace to
/// `openat,execve,clone` made it five times smaller and found NOTHING, because
/// cargo asks whether a path dependency's directory is THERE and never opens
/// it. Zero findings is what a hermetic tree looks like, so a census that can
/// only see opens is a census that reports clean on the one reach this
/// repository already knew about.
#[test]
fn a_directory_that_is_only_stated_is_still_reached() {
    let census = census_of(
        "156826 newfstatat(AT_FDCWD, \"/elsewhere/crates/thing\", {st_mode=S_IFDIR|0775, st_size=4096, ...}, 0) = 0\n",
    );
    assert_eq!(
        census
            .reaches
            .get(THE_DRIVER)
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![PathBuf::from("/elsewhere/crates/thing")]),
        "a stat is how a sibling checkout is reached; watching opens alone \
         answers zero, which is what a clean tree looks like: {:?}",
        census.reaches
    );
}

/// A REACH BELONGS TO THE TEST BINARY, WHICH IS NEVER THE THREAD THAT MADE IT.
///
/// A Rust test is a thread; its reaches are recorded against its own id, and
/// only the clone lines lead back to the binary. Without that walk the census
/// is a number rather than a finding — nobody can act on "something reached a
/// sibling checkout".
#[test]
fn a_reach_is_attributed_to_the_test_binary_above_the_thread_that_made_it() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/git_hooks_smoke-a1b2c3d4e5\", [\"git_hooks_smoke\"], 0x7f /* 99 vars */) = 0\n",
        "100 clone(child_stack=NULL, flags=CLONE_VM|CLONE_FS, child_tidptr=0x7c00) = 101\n",
        "101 clone(child_stack=NULL, flags=CLONE_VM|CLONE_FS, child_tidptr=0x7c00) = 102\n",
        "102 newfstatat(AT_FDCWD, \"/elsewhere/thing\", {st_mode=S_IFDIR|0775, ...}, 0) = 0\n",
    ));
    assert_eq!(
        census.reaches.keys().collect::<Vec<_>>(),
        vec!["git_hooks_smoke"],
        "the reach was made two clones below the binary and must still be its: \
         {:?}",
        census.reaches
    );
}

/// THE GROUND IS FOUR PLACES AND AN ANCESTOR OF ONE IS NOT A REACH.
///
/// `create_dir_all` walks up to the first parent that exists and stats each
/// one, so a fixture root three levels down makes its own ancestors look like
/// other trees. Measured: reading them as reaches turned 1,652 reaching
/// processes into 3,962 and buried the three real trees in noise.
#[test]
fn the_ground_a_run_stands_on_is_not_a_reach() {
    let census = census_of(concat!(
        "100 openat(AT_FDCWD, \"/repo/crates/x/src/lib.rs\", O_RDONLY|O_CLOEXEC) = 3\n",
        "100 openat(AT_FDCWD, \"/build/debug/deps/libx.rlib\", O_RDONLY|O_CLOEXEC) = 4\n",
        "100 openat(AT_FDCWD, \"/fixture/.tmpAbC123/store.json\", O_RDONLY|O_CLOEXEC) = 5\n",
        "100 openat(AT_FDCWD, \"/usr/lib/x86_64-linux-gnu/libc.so.6\", O_RDONLY|O_CLOEXEC) = 6\n",
        "100 openat(AT_FDCWD, \"/home/u/.cargo/registry/src/serde/lib.rs\", O_RDONLY) = 7\n",
        "100 newfstatat(AT_FDCWD, \"/\", {st_mode=S_IFDIR|0755, ...}, 0) = 0\n",
        "100 newfstatat(AT_FDCWD, \"/fixture\", {st_mode=S_IFDIR|0755, ...}, 0) = 0\n",
    ));
    assert!(
        census.reaches.is_empty(),
        "everything here is somewhere the run may stand, including the \
         ancestors it walked to get there: {:?}",
        census.reaches
    );
    assert_eq!(census.lines, 7, "and every line was read");
}

/// A FAILED CALL REACHED NOTHING, and this repository's `PATH` makes that most
/// of a trace.
///
/// `-e status=successful` drops them at the source — measured at 3.07 MB
/// against 1.17 MB for the same run and the same finding — but the reader must
/// not depend on the flag having been passed, because a caller that forgets it
/// would otherwise be told about every directory this machine's `PATH` does not
/// have.
#[test]
fn a_call_that_failed_reached_nothing() {
    let census = census_of(concat!(
        "100 newfstatat(AT_FDCWD, \"/elsewhere/absent\", 0x7ffd, 0) = -1 ENOENT (No such file or directory)\n",
        "100 openat(AT_FDCWD, \"/elsewhere/also-absent\", O_RDONLY|O_CLOEXEC) = -1 ENOENT (No such file or directory)\n",
    ));
    assert!(
        census.reaches.is_empty(),
        "a path a call FAILED to reach is not a reach: {:?}",
        census.reaches
    );
}

/// WHAT THE READER CANNOT PLACE IS COUNTED, NEVER DROPPED.
///
/// A path named relative to a directory descriptor cannot be resolved from this
/// trace. That is not a hole in the census of TREES — to read inside a tree
/// something must first open the directory, absolutely — but it is a hole in
/// the count of FILES, and a total that quietly omitted some is the shape this
/// repository keeps deleting.
#[test]
fn a_path_this_reader_cannot_place_is_counted_rather_than_ignored() {
    let census = census_of(concat!(
        "100 openat(3, \"nested/file.json\", O_RDONLY|O_CLOEXEC) = 4\n",
        "100 newfstatat(4, \"\", {st_mode=S_IFREG|0644, ...}, AT_EMPTY_PATH) = 0\n",
    ));
    assert_eq!(
        census.relative, 1,
        "a relative path is reported, so no reader is shown a total that \
         silently omitted it"
    );
    assert!(census.reaches.is_empty(), "{:?}", census.reaches);
}

/// A SIGNAL IS NOT A SYSCALL, and a run makes hundreds of them.
///
/// Measured on one lister run: 657 of 14,235 lines are `--- SIGCHLD …` or
/// `+++ exited …`. A reader that took the first quoted string off those would
/// be reading a signal's fields as a path.
#[test]
fn an_event_that_is_not_a_call_is_not_read_as_one() {
    let census = census_of(concat!(
        "158307 --- SIGCHLD {si_signo=SIGCHLD, si_code=CLD_EXITED, si_pid=158308, si_uid=1000} ---\n",
        "158307 +++ exited with 0 +++\n",
    ));
    assert!(census.reaches.is_empty(), "{:?}", census.reaches);
    assert_eq!(census.unparsed, 0, "these are events, not unreadable calls");
}

/// A CALL STRACE COULD NOT RENDER WHOLE IS ANNOUNCED.
///
/// THEY HAPPEN — 82 of them across the whole root suite, and zero on the two
/// smaller runs this was first checked against. The order inside the reader is
/// what this case is really about: the FIRST half of a split call still parses
/// as a syscall name (`clone(child_stack=NULL, … <unfinished ...>`), so a
/// reader that dispatched on the name before checking would hand it to the
/// clone arm, find no return value, and drop a whole subtree's parentage in
/// silence. This test found exactly that in the first draft.
#[test]
fn a_call_split_across_lines_is_announced_rather_than_swallowed() {
    let census = census_of(concat!(
        "100 clone(child_stack=NULL, flags=CLONE_VM <unfinished ...>\n",
        "100 <... clone resumed>) = 101\n",
    ));
    assert_eq!(
        census.unparsed, 2,
        "a reader that silently ignored these would lose the parentage they \
         carry, and a census that cannot attribute is a number"
    );
}

/// THE UNIT OF THE DECLARATION IS A TREE, because a census of files answers
/// "how much" where the question is "what".
///
/// DEPTH IS COUNTED FROM THE ROOT, so what makes a tree depends on how deep it
/// lives: `/home/coin/pinion` is three components and `/elsewhere/crates` is
/// two. That is why the VERDICT never uses this — [`judge`] matches a reach
/// against the declared prefix itself, which is depth-independent — and this
/// grouping exists for the printout alone.
#[test]
fn the_census_speaks_in_trees_and_names_who_reached_each() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 newfstatat(AT_FDCWD, \"/elsewhere/crates/a\", {st_mode=S_IFDIR|0775, ...}, 0) = 0\n",
        "100 newfstatat(AT_FDCWD, \"/elsewhere/crates/b\", {st_mode=S_IFDIR|0775, ...}, 0) = 0\n",
        "200 execve(\"/build/debug/deps/two_smoke-f6e5d4c3b2\", [\"two\"], 0x7f) = 0\n",
        "200 openat(AT_FDCWD, \"/elsewhere/crates/c/Cargo.toml\", O_RDONLY) = 3\n",
    ));
    let trees = census.trees(2);
    assert_eq!(trees.len(), 1, "three files, one tree: {trees:?}");
    let (tree, targets) = trees.into_iter().next().expect("one tree");
    assert_eq!(tree, PathBuf::from("/elsewhere/crates"));
    assert_eq!(
        targets.into_iter().collect::<Vec<_>>(),
        vec!["one_smoke".to_string(), "two_smoke".to_string()],
        "and both binaries that reached it are named, because a tree with no \
         reacher is a fact nobody can act on"
    );
}

/// A REACH NO ROW COVERS IS A FINDING; A ROW NO REACH EXERCISED IS SAID OUT
/// LOUD AND IS NOT ONE.
///
/// The second half is Round 1227's lesson applied to this gate itself: on a
/// hosted runner the sibling checkout is absent, so its row goes unexercised —
/// and a green run there must not read as evidence about a reach it never made,
/// nor may the row be deleted as stale.
#[test]
fn a_row_nothing_exercised_is_announced_and_a_reach_nothing_declares_is_a_finding() {
    const DECLARED: &[DeclaredReach] = &[
        DeclaredReach {
            at: Where::Sibling("elsewhere"),
            why: "a case",
            only_where_the_tree_exists: true,
        },
        DeclaredReach {
            at: Where::Sibling("absent-here"),
            why: "a case whose tree this machine does not have",
            only_where_the_tree_exists: true,
        },
    ];
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 newfstatat(AT_FDCWD, \"/elsewhere/thing\", {st_mode=S_IFDIR|0775, ...}, 0) = 0\n",
        "100 newfstatat(AT_FDCWD, \"/undeclared/thing\", {st_mode=S_IFDIR|0775, ...}, 0) = 0\n",
    ));
    let verdict = judge(&census, DECLARED, Path::new("/repo"), Path::new("/home/u"));
    assert!(
        !verdict.is_clean(),
        "a reach nothing declares must be a finding: {verdict:?}"
    );
    assert_eq!(
        verdict.undeclared,
        vec![("one_smoke".to_string(), PathBuf::from("/undeclared/thing"))],
        "and it names the binary and the path, not a count: {verdict:?}"
    );
    assert_eq!(verdict.exercised, vec![PathBuf::from("/elsewhere")]);
    assert_eq!(
        verdict.unexercised,
        vec![PathBuf::from("/absent-here")],
        "a row this run did not exercise is REPORTED — deleting it as stale is \
         how a gate loses the sibling's row the first time it runs on CI"
    );
}

/// A ROW NAMES NO MACHINE, AND THIS IS THE GATE POINTED AT ITSELF.
///
/// The first draft of the declaration wrote `/home/coin/pinion` and
/// `/home/coin/.gitconfig` — true where it was written and false everywhere
/// else. On a hosted runner `HOME` is `/home/runner`, so the git rows would
/// have matched nothing, git's own configuration would have been reported as an
/// undeclared reach, and a gate whose entire subject is claims that hold only
/// on their author's machine would have shipped one. It was caught by asking
/// where the table resolves on a runner, which is the question Round 1227 paid
/// to learn to ask; this is that question as a test.
#[test]
fn a_declared_tree_resolves_against_the_machine_asking_and_names_none() {
    const DECLARED: &[DeclaredReach] = &[
        DeclaredReach {
            at: Where::Sibling("pinion"),
            why: "a sibling checkout beside the repository",
            only_where_the_tree_exists: true,
        },
        DeclaredReach {
            at: Where::Home(".gitconfig"),
            why: "git's own configuration, wherever this user's home is",
            only_where_the_tree_exists: false,
        },
    ];
    // The same trace, judged as two machines would judge it: a workstation with
    // the checkout at `/home/coin/mnemosyne`, and a runner at
    // `/home/runner/work/mnemosyne/mnemosyne` whose home holds a different
    // `.gitconfig`.
    let workstation = judge(
        &census_of(concat!(
            "100 newfstatat(AT_FDCWD, \"/home/coin/pinion/crates/a\", {st_mode=S_IFDIR|0775, ...}, 0) = 0\n",
            "100 openat(AT_FDCWD, \"/home/coin/.gitconfig\", O_RDONLY|O_CLOEXEC) = 3\n",
        )),
        DECLARED,
        Path::new("/home/coin/mnemosyne"),
        Path::new("/home/coin"),
    );
    assert!(
        workstation.is_clean(),
        "both reaches are declared on the machine this was written on: {workstation:?}"
    );

    let runner = judge(
        &census_of("100 openat(AT_FDCWD, \"/home/runner/.gitconfig\", O_RDONLY|O_CLOEXEC) = 3\n"),
        DECLARED,
        Path::new("/home/runner/work/mnemosyne/mnemosyne"),
        Path::new("/home/runner"),
    );
    assert!(
        runner.is_clean(),
        "the SAME row must cover git's configuration where the home is not the \
         author's — a table naming `/home/coin` would report this as an \
         undeclared reach and turn CI red: {runner:?}"
    );
    assert_eq!(
        runner.unexercised,
        vec![PathBuf::from("/home/runner/work/mnemosyne/pinion")],
        "and the sibling's row resolves beside THAT repository, unexercised \
         because a runner has no sibling checkout — which is reported, not \
         deleted as stale: {runner:?}"
    );
}

/// A TREE IS THE FIRST FEW COMPONENTS, and the root is not swallowed.
#[test]
fn a_tree_is_named_from_the_root_down() {
    assert_eq!(
        tree_of(Path::new("/home/u/pinion/crates/a/Cargo.toml"), 3),
        PathBuf::from("/home/u/pinion")
    );
    assert_eq!(tree_of(Path::new("/a"), 3), PathBuf::from("/a"));
    assert_eq!(tree_of(Path::new("/"), 3), PathBuf::from("/"));
}

/// AN EMPTY TRACE IS NOT A HERMETIC RUN, and the count is what says so.
///
/// This is the gate's own non-vacuity: a census over a trace that never arrived
/// reports no reaches, and no reaches is exactly what a clean suite looks like.
/// The binary refuses on this; here it is asked of the reader, which is where
/// the number comes from.
#[test]
fn a_trace_that_never_arrived_is_distinguishable_from_a_clean_one() {
    let empty = census_of("");
    assert_eq!(empty.lines, 0);
    assert!(empty.reaches.is_empty());

    let clean = census_of("100 openat(AT_FDCWD, \"/repo/src/lib.rs\", O_RDONLY) = 3\n");
    assert_eq!(clean.lines, 1);
    assert!(clean.reaches.is_empty());
    // Same `reaches`, different `lines` — which is the whole reason the count
    // is reported beside the verdict rather than behind it.
}
