//! What the reader must get right about a trace, asked of fixtures rather than
//! of a run — so these cost milliseconds and hold on every machine.
//!
//! THE FIXTURES ARE REAL LINES. Every one below was copied from a
//! `strace -f -qq -e trace=%file,%process,close,close_range,dup,dup2,dup3,
//! fchdir,fcntl -e status=successful` stream this repository actually produced,
//! ids and all — with ONE exception, which says so where it stands. A simplified
//! fixture is what Round 1096 measured as passing on the day the thing it stood
//! for was broken: the spelling is what decides whether a reader is right, and
//! R1233's whole `fcntl` mechanism was found by reading a spelling.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use outside_reach::{
    judge, read_stream, tree_of, Census, DeclaredReach, Ground, Unplaced, Where, THE_DRIVER,
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

/// A trace of a command whose launch directory nobody told the reader — so every
/// bare name in it is unresolvable, which is the honest answer and not a wrong
/// one.
fn census_of(trace: &str) -> Census {
    read_stream(Cursor::new(trace.as_bytes()), &ground(), None)
}

/// A trace of a command launched from a known directory, which is what the
/// workflow passes and what makes `cd … && cat name` legible.
fn census_from(trace: &str, started_in: &str) -> Census {
    read_stream(
        Cursor::new(trace.as_bytes()),
        &ground(),
        Some(Path::new(started_in)),
    )
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

/// A DESCRIPTOR IS A PLACE TO START WALKING FROM, NOT A FLOOR — and this is the
/// case that killed the argument for not modelling the file table (R1233).
///
/// The reason written into this crate for counting descriptor-relative names and
/// dropping them was that it costs the census of FILES but not the census of
/// TREES, "because to read a file inside a tree, something must first open the
/// directory, and that open is absolute and is recorded". These two lines are
/// that claim's counterexample: the only absolute path in them is INSIDE the
/// ground, and the file actually reached is in a tree nothing here names.
#[test]
fn a_descriptor_is_a_place_to_start_walking_from_and_not_a_floor() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 openat(AT_FDCWD, \"/repo\", O_RDONLY|O_CLOEXEC|O_DIRECTORY) = 3\n",
        "100 openat(3, \"../elsewhere/thing/Cargo.toml\", O_RDONLY|O_CLOEXEC) = 4\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![PathBuf::from("/elsewhere/thing/Cargo.toml")]),
        "the descriptor was opened inside the ground and the name walked OUT of \
         it: a census that dropped the second line reports this run as clean, \
         which is what it did until R1233: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved_total(),
        0,
        "and nothing was left unplaced: {:?}",
        census.unresolved
    );
    assert_eq!(census.relative, 1, "one name was given relative");
}

/// A BARE NAME IS MEASURED FROM WHERE THE COMMAND STARTED, and the trace never
/// says where that is.
///
/// EVERY LINE BELOW IS REAL — `strace -f -qq -e
/// trace=%file,%process,close,close_range,dup,dup2,dup3,fchdir -e
/// status=successful /bin/sh -c 'cd /etc && cat hostname'`, ids and all. It is
/// the whole mechanism in one command: the shell `chdir`s, `vfork` hands the
/// working directory to a child that never mentions it again, and the file the
/// run actually read appears in the trace as the word `hostname`.
#[test]
fn a_bare_name_is_measured_from_where_the_command_started() {
    let trace = concat!(
        "4071742 execve(\"/bin/sh\", [\"/bin/sh\", \"-c\", \"cd /etc && cat hostname\"], 0x7ffe /* 99 vars */) = 0\n",
        "4071742 newfstatat(AT_FDCWD, \"/fixture/scratchpad\", {st_mode=S_IFDIR|0700, st_size=4096, ...}, 0) = 0\n",
        "4071742 newfstatat(AT_FDCWD, \".\", {st_mode=S_IFDIR|0700, st_size=4096, ...}, 0) = 0\n",
        "4071742 chdir(\"/etc\")                   = 0\n",
        "4071742 vfork()                         = 4071744\n",
        "4071744 execve(\"/usr/bin/cat\", [\"cat\", \"hostname\"], 0x6125 /* 99 vars */) = 0\n",
        "4071744 openat(AT_FDCWD, \"hostname\", O_RDONLY) = 3\n",
    );
    let census = census_from(trace, "/fixture/scratchpad");
    let reached: Vec<&PathBuf> = census.reaches.values().flatten().collect();
    assert_eq!(
        reached,
        vec![
            // The shell itself, which this ground does not name — the real
            // binary's does, and `/usr/bin/cat` on the sixth line is excused by
            // it here, which is what keeps this from being "everything is a
            // reach".
            &PathBuf::from("/bin/sh"),
            // The directory the shell moved into is a place it reached.
            &PathBuf::from("/etc"),
            // AND THE FILE, which no line of the trace spells.
            &PathBuf::from("/etc/hostname"),
        ],
        "the file this command read is `/etc/hostname`: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved_total(),
        0,
        "the working directory was inherited across the vfork, so nothing here \
         was unplaceable: {:?}",
        census.unresolved
    );

    // AND TOLD NOTHING, THE READER LOSES THE NAMES BEFORE THE `chdir` AND
    // KEEPS THE ONES AFTER IT — measured here rather than assumed. An absolute
    // `chdir` states the working directory as surely as a launch flag does, so
    // the file is still found; the `.` on the third line, made while the
    // directory was unknown, is the residue and is counted rather than guessed.
    let blind = census_of(trace);
    assert!(
        blind
            .reaches
            .values()
            .flatten()
            .any(|path| path == Path::new("/etc/hostname")),
        "an absolute `chdir` re-establishes the working directory whatever the \
         reader was told: {:?}",
        blind.reaches
    );
    assert_eq!(
        blind.unresolved_total(),
        1,
        "and the one name made BEFORE it — `.` — is unplaceable, which is the \
         honest answer rather than a guess about where this command started: \
         {:?}",
        blind.unresolved
    );
}

/// WHAT THE MODEL CANNOT PLACE IS COUNTED AND ATTRIBUTED, NEVER GUESSED.
///
/// A descriptor opened before the trace began, one handed over a socket, one
/// whose `clone` line `strace` could not render whole: the model has no binding
/// for it, and the only answer it is allowed to give is "unknown". A residue
/// nobody can attribute is a number rather than a finding, so it is carried per
/// binary — the same reason the reaches themselves are.
#[test]
fn a_name_the_model_cannot_place_is_counted_against_the_binary_that_made_it() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 openat(9, \"nested/file.json\", O_RDONLY|O_CLOEXEC) = 4\n",
        "100 newfstatat(4, \"\", {st_mode=S_IFREG|0644, ...}, AT_EMPTY_PATH) = 0\n",
    ));
    assert_eq!(
        census.relative, 2,
        "both names were given relative, and the total says so whatever became \
         of them"
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(2),
        "descriptor 9 was never opened in this trace, so neither the name under \
         it nor the descriptor it produced can be placed — and the binary that \
         made them is named: {:?}",
        census.unresolved
    );
    assert!(
        census.reaches.is_empty(),
        "and nothing is reported as reached on a guess: {:?}",
        census.reaches
    );
}

/// THE RESIDUE SAYS WHAT IT IS MADE OF, because the three kinds are three
/// different pieces of work.
///
/// MEASURED, AND THE NUMBER IS WHY THIS EXISTS: the first whole-suite run under
/// the file table placed 601,965 names and left 391,418, and 391,417 of those
/// were one test binary's. As a single total that is a blind spot somebody has
/// to re-measure before they can act on it — closing a residue of descriptors
/// nobody saw opened is not the same work as closing one of unnamed working
/// directories, and neither is reading an argument this reader cannot parse.
///
/// THE LAST LINE BELOW IS THE ONE CONSTRUCTED FIXTURE IN THIS FILE, and it is
/// named as such because everything else here was copied from a real stream. It
/// stands for a syscall shape this reader has NOT been taught, which is the one
/// case that cannot be copied from a run — it does not exist until `strace`
/// renders something new. That the third count is normally zero is the point of
/// having it: a run that suddenly reports many is a run whose lines this reader
/// is no longer reading, and without the breakdown that is indistinguishable
/// from a run that opened more descriptors.
#[test]
fn what_the_model_could_not_place_says_which_of_the_three_kinds_it_is() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        // No launch directory was given and nothing has said where this stands.
        "100 newfstatat(AT_FDCWD, \"under-an-unnamed-directory\", {st_mode=S_IFREG|0644, ...}, 0) = 0\n",
        // A descriptor this trace never shows being opened.
        "100 openat(9, \"under-an-unseen-descriptor\", O_RDONLY) = 4\n",
        // An argument that is neither `AT_FDCWD`, a descriptor, nor absent.
        "100 somecall(FUTURE_FLAG_THIS_READER_CANNOT_READ, \"after-an-unreadable-argument\", 0) = 0\n",
    ));
    let residue = census.unresolved.get("one_smoke").expect("attributed");
    assert_eq!(
        (residue.working, residue.descriptor, residue.unreadable),
        (1, 1, 1),
        "one of each, and a total of three would say nothing about which work \
         would close any of them: {residue:?}"
    );
    // AND A FEW SPELLINGS, which is what turns the kind into a diagnosis. The
    // whole of R1233's largest residue was closed by reading one of these and
    // finding `fcntl(4, F_DUPFD_CLOEXEC, 3) = 5` behind it.
    assert_eq!(
        residue.examples,
        vec![
            "newfstatat(AT_FDCWD, \"under-an-unnamed-directory\")".to_string(),
            "openat(9, \"under-an-unseen-descriptor\")".to_string(),
            "somecall(?, \"after-an-unreadable-argument\")".to_string(),
        ],
        "each is spelled with the call and the descriptor it was measured from: \
         {residue:?}"
    );
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

/// `/../lib` AND `/lib` ARE ONE FILE, AND THE GATE MUST NOT REPORT ONE OF THEM
/// (R1232).
///
/// Measured, on the first hosted run that ever carried this census: ELEVEN of
/// its nineteen findings were toolchain paths written the way a gcc driver
/// writes them — `/../lib/gcc/x86_64-linux-gnu/14/crtbegin.o` — and every one
/// was inside the ground already named, because `starts_with("/lib")` is false
/// for a path whose first component after the root is `..`.
///
/// THE GATE WAS DISAGREEING WITH ITSELF, which is how it showed: `tree_of`
/// drops non-`Normal` components, so the same run printed `REACH /lib/gcc` in
/// one list and `reached /../lib/gcc` in the other.
#[test]
fn a_path_written_through_its_own_parent_is_the_file_it_names() {
    // `/lib64` IS A SEPARATE ENTRY AND NOT A CHILD OF `/lib` — the ground the
    // binary declares names both, and a fixture that named only the first would
    // fail here for a reason that is about the fixture. The second line below is
    // why: it normalises to `/lib64`, which starts with `/lib` as a STRING and
    // not as a path, which is the comparison this gate makes.
    let ground = Ground {
        owned: vec![PathBuf::from("/repo")],
        toolchain: vec![PathBuf::from("/lib"), PathBuf::from("/lib64")],
    };
    let census = read_stream(
        Cursor::new(
            "110 newfstatat(AT_FDCWD, \"/../lib/gcc/x86_64-linux-gnu/14/crtbegin.o\", \
             {st_mode=S_IFREG|0644, st_size=3560, ...}, 0) = 0\n\
             110 newfstatat(AT_FDCWD, \"/../lib/gcc/x86_64-linux-gnu/14/../../../../lib64\", \
             {st_mode=S_IFDIR|0755, st_size=4096, ...}, 0) = 0\n"
                .as_bytes(),
        ),
        &ground,
        None,
    );
    assert!(
        census.reaches.is_empty(),
        "a toolchain path spelled through `..` is still the toolchain: {:?}",
        census.reaches
    );
    assert_eq!(census.lines, 2, "and the lines were read, not skipped");

    // THE CONTROL, and it is what stops the repair from being "excuse anything
    // with a `..` in it": a path that STILL leaves the ground once normalised is
    // still a finding, and it is reported under the name it resolves to.
    let census = read_stream(
        Cursor::new(
            "111 openat(AT_FDCWD, \"/repo/../elsewhere/crates/thing/Cargo.toml\", \
             O_RDONLY|O_CLOEXEC) = 3\n"
                .as_bytes(),
        ),
        &ground,
        None,
    );
    let reached: Vec<&PathBuf> = census.reaches.values().flatten().collect();
    assert_eq!(
        reached,
        vec![&PathBuf::from("/elsewhere/crates/thing/Cargo.toml")],
        "a reach that survives normalisation is reported, and by the path it \
         actually names: {:?}",
        census.reaches
    );
    // AND THE TWO HALVES OF THE REPORT AGREE ABOUT IT, which is the property
    // whose absence made this findable at all.
    assert_eq!(
        census.trees(2).keys().collect::<Vec<_>>(),
        vec![&PathBuf::from("/elsewhere/crates")]
    );
}

/// A THREAD SEES WHAT ITS SIBLINGS OPEN NEXT; A FORK SEES A COPY FROZEN AT THE
/// CLONE. The flags on the line are what says which, and a cargo run is full of
/// both.
///
/// Getting this wrong is not a lost resolution but a WRONG one in the sharing
/// direction: a model that always shared would resolve a forked child's
/// `openat(3, …)` against a directory only its parent went on to open, and
/// report a reach that never happened.
#[test]
fn a_thread_shares_the_file_table_and_a_fork_gets_a_copy() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        // The thread and the forked child are both made BEFORE the descriptor
        // exists, which is the whole point: only one of them can see it.
        "100 clone(child_stack=0x7f, flags=CLONE_VM|CLONE_FS|CLONE_FILES|CLONE_SIGHAND|CLONE_THREAD, parent_tid=[101]) = 101\n",
        "100 clone(child_stack=NULL, flags=CLONE_CHILD_CLEARTID|SIGCHLD, child_tidptr=0x7f) = 102\n",
        "100 openat(AT_FDCWD, \"/repo/crates\", O_RDONLY|O_DIRECTORY) = 3\n",
        "101 openat(3, \"../../elsewhere/seen-by-the-thread\", O_RDONLY) = 4\n",
        "102 openat(3, \"../../elsewhere/hidden-from-the-fork\", O_RDONLY) = 5\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![PathBuf::from("/elsewhere/seen-by-the-thread")]),
        "the thread shares the table and its name resolves; the forked child's \
         copy predates the open, so its name is unplaceable rather than resolved \
         against a descriptor it does not hold: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(1),
        "and the fork's name is the residue, counted: {:?}",
        census.unresolved
    );
}

/// A PROCESS THAT SPOKE BEFORE ITS OWN CLONE LINE IS ADOPTED, NOT ORPHANED —
/// and this is the NORMAL shape of a spawn rather than an oddity.
///
/// THE LINE NUMBERS ARE REAL AND SO IS THE ORDER. In this repository's own
/// suite, a `posix_spawn` child's `dup2`, `dup2`, `dup2`, `chdir` and `execve`
/// are all printed BEFORE its parent's `clone3(…) = <pid>` — because a `vfork`
/// parent is blocked until the child execs, so it cannot print anything until
/// then. A reader that treats such a child's empty table as its own state gives
/// it nothing to resolve against, and every name it later gives under an
/// inherited descriptor becomes residue. Measured: 391,762 of one whole-suite
/// census's 433,904 unplaced names were exactly this.
#[test]
fn a_process_that_spoke_before_its_clone_line_still_inherits() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 openat(AT_FDCWD, \"/elsewhere/opened-by-the-parent\", O_RDONLY|O_DIRECTORY) = 3\n",
        // The child speaks first — the real order, from a real trace.
        "102 dup2(5, 0)                      = 0\n",
        "102 openat(3, \"under-what-it-inherited\", O_RDONLY) = 6\n",
        // …and only now does the parent's clone return.
        "100 clone3({flags=CLONE_VM|CLONE_VFORK|CLONE_CLEAR_SIGHAND, exit_signal=SIGCHLD, stack=0x763e, stack_size=0x9000}, 88) = 102\n",
        "102 openat(3, \"named-after-the-clone-line\", O_RDONLY) = 7\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![
            PathBuf::from("/elsewhere/opened-by-the-parent"),
            PathBuf::from("/elsewhere/opened-by-the-parent/named-after-the-clone-line"),
        ]),
        "the name the child gives AFTER its clone line resolves against what it \
         inherited, and it is attributed to the binary above it: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(1),
        "the one name it gave BEFORE the clone line stays residue — this reader \
         had not been told the child existed yet, and inventing a parent for it \
         would be a guess: {:?}",
        census.unresolved
    );

    // AND WHAT THE CHILD ALREADY LEARNED IS NEWER THAN WHAT IT WOULD INHERIT.
    // Under `CLONE_FS` the working directory is ONE object, so the child's
    // `chdir` moved the parent too; adopting the parent's answer would put the
    // pair back where the child had already left, which is the model answering
    // WRONG rather than late.
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 chdir(\"/repo\")                     = 0\n",
        "103 chdir(\"/elsewhere/where-the-child-went\") = 0\n",
        "100 clone(child_stack=0x7f, flags=CLONE_VM|CLONE_FS|CLONE_FILES|CLONE_SIGHAND|CLONE_THREAD, parent_tid=[103]) = 103\n",
        "103 openat(AT_FDCWD, \"named-after\", O_RDONLY) = 4\n",
        "100 openat(AT_FDCWD, \"named-by-the-parent\", O_RDONLY) = 5\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![
            PathBuf::from("/elsewhere/where-the-child-went"),
            PathBuf::from("/elsewhere/where-the-child-went/named-after"),
            PathBuf::from("/elsewhere/where-the-child-went/named-by-the-parent"),
        ]),
        "both stand where the LAST `chdir` put them, and that was the child's: \
         {:?}",
        census.reaches
    );
}

/// A DESCRIPTOR CLOSED ON EXEC IS NOT THERE AFTERWARDS, and the kernel is what
/// closed it.
///
/// Nearly every open in a Rust build carries `O_CLOEXEC` — the four lines of a
/// real `/bin/sh` trace above carry it three times — so a model that kept them
/// across `execve` would be holding stale bindings through every spawn in the
/// run. The failure that costs is not the lost resolution: it is that the new
/// program's own descriptor 3 would be read as the old program's file.
#[test]
fn a_descriptor_the_kernel_closed_at_exec_is_not_read_as_the_old_file() {
    let census = census_of(concat!(
        "100 openat(AT_FDCWD, \"/elsewhere/before-the-exec\", O_RDONLY|O_CLOEXEC|O_DIRECTORY) = 3\n",
        "100 openat(AT_FDCWD, \"/elsewhere/kept\", O_RDONLY|O_DIRECTORY) = 4\n",
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 openat(3, \"after\", O_RDONLY) = 5\n",
        "100 openat(4, \"after\", O_RDONLY) = 6\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![
            PathBuf::from("/elsewhere/before-the-exec"),
            PathBuf::from("/elsewhere/kept"),
            PathBuf::from("/elsewhere/kept/after"),
        ]),
        "the descriptor without O_CLOEXEC survives the exec and its name \
         resolves; the one the kernel closed resolves to nothing: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(1),
        "and the name under the closed descriptor is residue, not a path: {:?}",
        census.unresolved
    );
}

/// A NUMBER SOMETHING RE-POINTED IS FORGOTTEN RATHER THAN BELIEVED.
///
/// `dup2(10, 1) = 1` IS A REAL LINE — from `/bin/sh -c 'cat hostname >
/// /dev/null'`, where descriptor 10 is the shell's saved copy of its own stdout
/// and was opened before the trace began. Nothing in the stream says what 10 is,
/// so nothing can say what 1 is afterwards; the binding this reader was holding
/// under 1 is now a lie, and deleting it is the difference between the model
/// answering UNKNOWN and answering WRONG.
#[test]
fn a_descriptor_re_pointed_from_an_unknown_source_is_forgotten() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 openat(AT_FDCWD, \"/elsewhere/first\", O_RDONLY|O_DIRECTORY) = 1\n",
        "100 dup2(10, 1)                        = 1\n",
        "100 openat(1, \"under-whatever-1-is-now\", O_RDONLY) = 3\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![PathBuf::from("/elsewhere/first")]),
        "only the open itself is a reach — the name under the re-pointed \
         descriptor must not be reported against the directory that used to be \
         there: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(1),
        "it is residue: {:?}",
        census.unresolved
    );

    // AND A `dup2` WHOSE SOURCE IS KNOWN CARRIES THE PATH ACROSS, which is what
    // stops the rule above from being "forget every duplicated descriptor".
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 openat(AT_FDCWD, \"/elsewhere/first\", O_RDONLY|O_DIRECTORY) = 3\n",
        "100 dup2(3, 7)                         = 7\n",
        "100 openat(7, \"still-the-same-directory\", O_RDONLY) = 8\n",
    ));
    assert!(
        census.reaches.get("one_smoke").is_some_and(
            |paths| paths.contains(&PathBuf::from("/elsewhere/first/still-the-same-directory"))
        ),
        "a duplicate of a known descriptor names the same directory: {:?}",
        census.reaches
    );
}

/// A CLOSED DESCRIPTOR IS GONE, WHICH IS WHY THE TRACE ASKS FOR `close` AT ALL.
///
/// `close` takes no filename, so it is not in `%file` and the census did not see
/// it before R1233. Without it a number stays bound to a directory the process
/// has finished with — and the kernel hands that same number to the NEXT thing
/// that asks, which is how a model with no `close` answers wrong rather than
/// unknown. `close_range` is the same fact for a spawner that closes everything
/// above 2 in one call.
#[test]
fn a_closed_descriptor_is_not_a_directory_any_more() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 openat(AT_FDCWD, \"/elsewhere/closed\", O_RDONLY|O_DIRECTORY) = 3\n",
        "100 close(3)                           = 0\n",
        "100 openat(3, \"under-a-free-number\", O_RDONLY) = 4\n",
        "100 openat(AT_FDCWD, \"/elsewhere/ranged\", O_RDONLY|O_DIRECTORY) = 5\n",
        "100 close_range(3, 4294967295, 0)      = 0\n",
        "100 openat(5, \"under-a-freed-range\", O_RDONLY) = 6\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![
            PathBuf::from("/elsewhere/closed"),
            PathBuf::from("/elsewhere/ranged"),
        ]),
        "the two opens are reaches; neither name under a closed number is: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(2),
        "both are residue: {:?}",
        census.unresolved
    );
}

/// `fcntl` IS HOW A DESCRIPTOR IS USUALLY DUPLICATED, and this is the case the
/// residue found rather than the manual.
///
/// EVERY LINE BELOW IS REAL — `find studio -name Cargo.toml`, traced on this
/// repository. It opens the directory, immediately does
/// `fcntl(4, F_DUPFD_CLOEXEC, 3) = 5`, and then names everything it walks under
/// FIVE. `dup`/`dup2`/`dup3` never appear. Until the trace asked for `fcntl`,
/// 33,791 of one run's 33,792 unplaced names were that one descriptor — the
/// count said "under a descriptor never seen opened" and the example said which
/// call, which is what turned a number into this test.
///
/// `F_SETFD` is here for the same reason `O_CLOEXEC` is: it is how a
/// descriptor's survival across `execve` is decided after the fact.
#[test]
fn a_descriptor_duplicated_by_fcntl_names_the_same_directory() {
    let census = census_from(
        concat!(
            "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
            "100 openat(AT_FDCWD, \"elsewhere\", O_RDONLY|O_NOCTTY|O_NONBLOCK|O_NOFOLLOW|O_CLOEXEC|O_DIRECTORY) = 4\n",
            "100 fcntl(4, F_GETFL)               = 0x38800 (flags O_RDONLY|O_NONBLOCK|O_LARGEFILE|O_NOFOLLOW|O_DIRECTORY)\n",
            "100 fcntl(4, F_SETFD, FD_CLOEXEC)   = 0\n",
            "100 fcntl(4, F_DUPFD_CLOEXEC, 3)    = 5\n",
            "100 newfstatat(5, \"walked\", {st_mode=S_IFDIR|0775, st_size=4096, ...}, AT_SYMLINK_NOFOLLOW) = 0\n",
        ),
        "/",
    );
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![
            PathBuf::from("/elsewhere"),
            PathBuf::from("/elsewhere/walked"),
        ]),
        "the duplicate names the directory the original named, so what the walk \
         reaches is reported rather than counted: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved_total(),
        0,
        "and nothing is left over: {:?}",
        census.unresolved
    );

    // AND `F_SETFD` DECIDES WHAT AN `execve` KEEPS. The same open without
    // `O_CLOEXEC`, marked close-on-exec afterwards, must not survive the exec —
    // otherwise the new program's descriptor 4 reads as the old program's
    // directory.
    let census = census_from(
        concat!(
            "100 openat(AT_FDCWD, \"elsewhere\", O_RDONLY|O_DIRECTORY) = 4\n",
            "100 fcntl(4, F_SETFD, FD_CLOEXEC)   = 0\n",
            "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
            "100 newfstatat(4, \"after\", {st_mode=S_IFDIR|0775, ...}, 0) = 0\n",
        ),
        "/",
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(1),
        "the descriptor the kernel closed at the exec is residue, not the old \
         directory: {:?}",
        census.unresolved
    );
}

/// `fchdir` MOVES THE WORKING DIRECTORY WITHOUT NAMING IT, and to an unknown
/// descriptor it makes the working directory UNKNOWN rather than stale.
#[test]
fn the_working_directory_follows_fchdir_and_becomes_unknown_when_it_cannot() {
    let census = census_from(
        concat!(
            "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
            "100 openat(AT_FDCWD, \"/elsewhere/somewhere\", O_RDONLY|O_DIRECTORY) = 3\n",
            "100 fchdir(3)                          = 0\n",
            "100 openat(AT_FDCWD, \"named-from-there\", O_RDONLY) = 4\n",
        ),
        "/repo",
    );
    assert!(
        census.reaches.get("one_smoke").is_some_and(
            |paths| paths.contains(&PathBuf::from("/elsewhere/somewhere/named-from-there"))
        ),
        "the bare name is measured from the directory the descriptor named: {:?}",
        census.reaches
    );

    let census = census_from(
        concat!(
            "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
            "100 fchdir(9)                          = 0\n",
            "100 openat(AT_FDCWD, \"named-from-nowhere\", O_RDONLY) = 4\n",
        ),
        "/repo",
    );
    assert!(
        census.reaches.is_empty(),
        "a working directory this reader cannot name must not leave the previous \
         one in place — the name would be measured from a directory the process \
         has left: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(1),
        "it is residue: {:?}",
        census.unresolved
    );
}

/// `getcwd` IS THE KERNEL ANSWERING THE QUESTION THIS READER CANNOT ASK.
///
/// It takes no filename — it RETURNS one, into a buffer `strace` renders as the
/// first quoted argument — and it is in `%file` already, so it costs nothing to
/// read. Any process that asks it says where it stands, which repairs a working
/// directory whose descent from the launch directory was broken. Measured:
/// 41,828 of one whole-suite census's unplaced names were the cargo driver's,
/// under a directory nothing had named; the line below is a real one from this
/// repository's own suite.
#[test]
fn a_process_that_asks_where_it_stands_has_told_this_reader() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        // Before it says so, a bare name cannot be placed.
        "100 newfstatat(AT_FDCWD, \"before-it-said\", {st_mode=S_IFREG|0644, ...}, 0) = 0\n",
        "100 getcwd(\"/elsewhere/where-it-stands\", 512)  = 26\n",
        "100 openat(AT_FDCWD, \"after-it-said\", O_RDONLY) = 3\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![
            PathBuf::from("/elsewhere/where-it-stands"),
            PathBuf::from("/elsewhere/where-it-stands/after-it-said"),
        ]),
        "the directory it named is a place it stands, and every bare name after \
         it is measured from there: {:?}",
        census.reaches
    );
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(1),
        "and the one name it gave BEFORE saying so is still residue — this is a \
         reader of a stream, not of a run it can re-question: {:?}",
        census.unresolved
    );

    // EXCEPT WHEN THE ANSWER IS NOT A PATH. A process whose working directory
    // has been removed is told `"/tmp/x (deleted)"`, which is what the kernel
    // says and not where anything is. Taking it would measure every later name
    // from a directory that does not exist under a name nothing has.
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 getcwd(\"/elsewhere/pulled-out-from-under-it (deleted)\", 512) = 44\n",
        "100 openat(AT_FDCWD, \"after-it-said\", O_RDONLY) = 3\n",
    ));
    assert_eq!(
        census.unresolved.get("one_smoke").map(Unplaced::total),
        Some(1),
        "the bare name after it stays unplaced, rather than being reported under \
         a directory spelled `… (deleted)`: {:?}",
        census.unresolved
    );
}

/// THE CONTENT OF A SYMLINK IS NOT A PATH THE RUN REACHED.
///
/// `symlink("../../elsewhere/target", "link")` writes a STRING; the file it
/// touches is the second argument. A reader that took the first quoted string
/// off every line — which is what this one did — would resolve that string
/// against the working directory and report a reach into a tree the run never
/// looked at. A false finding is the one outcome worse than a missing one.
#[test]
fn the_content_of_a_symlink_is_not_a_path_the_run_reached() {
    let census = census_from(
        concat!(
            "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
            "100 symlink(\"../elsewhere/never-touched\", \"/fixture/link\") = 0\n",
            "100 symlinkat(\"../elsewhere/never-touched\", 3, \"link\") = 0\n",
        ),
        "/repo",
    );
    assert!(
        census.reaches.is_empty(),
        "neither line reaches anything outside the ground — the link's content \
         is a string, and where it points is not where anything looked: {:?}",
        census.reaches
    );
}

/// A PROGRAM A RUN EXECUTES IS A FILE IT READ (R1233).
///
/// The `execve` arm took the name to attribute reaches to it and judged the path
/// itself against nothing, so a binary run from outside the ground was a reach
/// this census could not report. A test that shells out to a tool the machine
/// beside it does not have is exactly the finding this gate exists to make
/// visible.
#[test]
fn a_program_a_run_executes_is_a_file_it_read() {
    let census = census_of(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 clone(child_stack=NULL, flags=CLONE_CHILD_CLEARTID|SIGCHLD, child_tidptr=0x7f) = 101\n",
        "101 execve(\"/elsewhere/bin/a-tool-only-this-machine-has\", [\"a-tool\"], 0x7f) = 0\n",
    ));
    assert_eq!(
        census
            .reaches
            .get("one_smoke")
            .map(|paths| paths.iter().cloned().collect::<Vec<_>>()),
        Some(vec![PathBuf::from(
            "/elsewhere/bin/a-tool-only-this-machine-has"
        )]),
        "the program is the file, and the reach belongs to the test binary that \
         spawned it: {:?}",
        census.reaches
    );
}

// --------------------------------------------------------------- the verdict

/// THE CENSUS'S OWN THREE ANSWERS, ASKED OF THE PROCESS (R1233).
///
/// Every case above asks the LIBRARY, and R1127 measured what that leaves open
/// in this repository's gates: a refusal path reported as a clean pass with the
/// whole suite green, because nothing ran the binary. `--verdict-of` has had a
/// process-level reader since R1230; the census itself — the path that decides
/// what a run reached and whether that is a finding — had none.
///
/// The fourth case is this round's mechanism at the level that matters: a reach
/// made through a descriptor must reach the VERDICT, not only the census, or the
/// model resolves paths nothing acts on.
#[test]
fn the_census_answers_empty_clean_and_finding_as_three_different_statuses() {
    // THE ENVIRONMENT IS NAMED RATHER THAN INHERITED. This binary reads five
    // variables to place the toolchain and the home the declaration resolves
    // against, and a test that left them to the machine would run a different
    // program on a machine that sets them. All five are declared ABSENT, which
    // makes the ground exactly `--repo`, `--build`, `--fixture` and the
    // operating system's own roots.
    let run = |trace: &str| {
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_outside-reach"))
            .env_remove("CARGO_HOME")
            .env_remove("RUSTUP_HOME")
            .env_remove("SCCACHE_DIR")
            .env_remove("CCACHE_DIR")
            .env_remove("HOME")
            .args([
                "--repo",
                "/repo",
                "--build",
                "/build",
                "--fixture",
                "/fixture",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the reader runs");
        {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .expect("stdin")
                .write_all(trace.as_bytes())
                .expect("the trace goes in");
        }
        let out = child.wait_with_output().expect("it finishes");
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (
            out.status.code().expect("it exits rather than signals"),
            said,
        )
    };

    // NO TRACE AT ALL — which is what a run this gate never saw looks like, and
    // is indistinguishable from a hermetic one by the reaches alone.
    let (code, said) = run("");
    assert_eq!(code, 2, "an empty trace is `could not judge`:\n{said}");
    assert!(said.contains("EMPTY"), "and it says so:\n{said}");

    // A RUN THAT STAYED HOME.
    let (code, said) = run("100 openat(AT_FDCWD, \"/repo/src/lib.rs\", O_RDONLY|O_CLOEXEC) = 3\n");
    assert_eq!(code, 0, "everything it read is ground:\n{said}");
    assert!(
        said.contains("1 line(s)"),
        "and the count is printed beside the verdict, which is what makes the \
         case above readable at all:\n{said}"
    );

    // A REACH NOTHING DECLARES.
    let (code, said) = run(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 newfstatat(AT_FDCWD, \"/elsewhere/thing\", {st_mode=S_IFDIR|0775, ...}, 0) = 0\n",
    ));
    assert_eq!(code, 1, "a reach no row covers is a finding:\n{said}");
    assert!(
        said.contains("one_smoke reached /elsewhere/thing"),
        "named, with the binary that made it:\n{said}"
    );

    // AND THE SAME REACH MADE THROUGH A DESCRIPTOR (R1233), which no absolute
    // path in this trace names. Before the file table this exited 0.
    let (code, said) = run(concat!(
        "100 execve(\"/build/debug/deps/one_smoke-a1b2c3d4e5\", [\"one\"], 0x7f) = 0\n",
        "100 openat(AT_FDCWD, \"/repo\", O_RDONLY|O_CLOEXEC|O_DIRECTORY) = 3\n",
        "100 openat(3, \"../elsewhere/through-a-descriptor\", O_RDONLY) = 4\n",
    ));
    assert_eq!(
        code, 1,
        "the model must reach the VERDICT and not only the census:\n{said}"
    );
    assert!(
        said.contains("one_smoke reached /elsewhere/through-a-descriptor"),
        "and the path it names is the one the descriptor resolves to:\n{said}"
    );
}

/// THE THREE ANSWERS OF `--verdict-of`, ASKED OF THE PROCESS (R1230).
///
/// The census cannot fail the step it rides on — `strace` returns the wrapped
/// command's status and says nothing about the program on the other end of its
/// pipe — so it writes a status to a file and a LATER step exits with it. That
/// step was four lines of shell until the first hosted run measured what shell
/// costs here: the job hit its budget and was cancelled before the reader wrote
/// anything, `cat` on a file that does not exist left an empty string, and
/// `exit ""` failed with a message about `exit`. A census that NEVER ANSWERED
/// and a census that answered `no` came out of that step identical — and the
/// first is the answer to the question this gate has open, whether `ptrace`
/// attaches on a hosted runner at all.
///
/// Run as a PROCESS, because an exit code is the whole of what this mode
/// produces: R1127 measured this crate's sibling reporting a refusal path as a
/// clean pass with the entire suite green, because nothing ran the binary.
#[test]
fn a_verdict_that_was_never_written_is_the_third_answer_and_not_the_first() {
    let at = std::env::temp_dir().join(format!("outside-reach-verdict-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).expect("fixture directory");

    // THE ENVIRONMENT THIS SPAWN GIVES IS NAMED, NOT INHERITED, and the gate
    // that insisted is right to: this binary reads five variables to place the
    // toolchain, and a test that leaves them to the machine runs a different
    // program on a machine that sets them. `--verdict-of` reads none of the five
    // — it opens one file and exits with what is in it — so what this case
    // declares is their ABSENCE, which is a claim rather than a shrug.
    let run = |file: &Path| {
        std::process::Command::new(env!("CARGO_BIN_EXE_outside-reach"))
            .env_remove("CARGO_HOME")
            .env_remove("RUSTUP_HOME")
            .env_remove("SCCACHE_DIR")
            .env_remove("CCACHE_DIR")
            .env_remove("HOME")
            .arg("--verdict-of")
            .arg(file)
            .output()
            .expect("the reader runs")
    };
    let code =
        |out: &std::process::Output| out.status.code().expect("it exits rather than signals");
    let said = |out: &std::process::Output| {
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };

    // NO FILE AT ALL — the run was killed before the census could answer.
    let missing = at.join("never-written.rc");
    let out = run(&missing);
    assert_eq!(
        code(&out),
        2,
        "a verdict nobody wrote is `could not judge`, never `clean`:\n{}",
        said(&out)
    );
    assert!(
        said(&out).contains("never answered") && said(&out).contains("never-written.rc"),
        "and it says which of the two it is, with the file it looked for:\n{}",
        said(&out)
    );

    // A FILE HOLDING SOMETHING THAT IS NOT A STATUS — the same third answer, and
    // not the silent pass `exit "$verdict"` would have made of it.
    let garbage = at.join("garbage.rc");
    std::fs::write(&garbage, "strace: Operation not permitted\n").expect("write");
    let out = run(&garbage);
    assert_eq!(code(&out), 2, "{}", said(&out));
    assert!(
        said(&out).contains("is not a status"),
        "and it says what it found rather than the file's name alone:\n{}",
        said(&out)
    );

    // AND THE REAL VERDICTS PASS THROUGH UNCHANGED, which is what stops all of
    // the above from holding for a reader that simply always refuses.
    for (recorded, expected) in [("0\n", 0), ("1\n", 1), ("2", 2)] {
        let file = at.join(format!("verdict-{expected}.rc"));
        std::fs::write(&file, recorded).expect("write");
        let out = run(&file);
        assert_eq!(
            code(&out),
            expected,
            "a recorded `{recorded}` is the status this step exits with:\n{}",
            said(&out)
        );
        if expected == 0 {
            // The clean one says so out loud: a step that printed nothing on the
            // path that matters cannot be told from a step that never ran.
            assert!(
                said(&out).contains("verdict=0"),
                "the verdict is printed, not only returned:\n{}",
                said(&out)
            );
        }
    }

    let _ = std::fs::remove_dir_all(&at);
}

/// The filter every caller builds a census command with is THIS reader's, asked
/// of it rather than copied.
///
/// R1233 is the argument, and it is not a hypothesis: the `-e trace=` list was
/// one `fcntl` short of the model it feeds for as long as the model existed, and
/// what that cost was 433,904 names unplaced in a single whole-suite run. Which
/// syscalls the stream must carry is a fact about the resolution above — a
/// narrower filter resolves FEWER names, and fewer names is what a hermetic run
/// looks like, so the drift is silent in the direction that reads as clean.
///
/// The hosted job cannot ask a program for its filter — it is YAML, and the
/// value is in the command it hands `strace`. So the agreement is asked HERE, of
/// the file, and the shell that CAN ask does (`scripts/census-elsewhere.sh`).
///
/// WHAT THIS CANNOT SEE, SAID RATHER THAN HIDDEN: it reads the workflow's text,
/// so a filter assembled at run time out of a variable would not be compared.
/// That state is not silent either — the count below would be zero, and a law
/// that finds nothing to judge fails rather than passes.
#[test]
fn every_census_command_in_this_repository_asks_this_reader_for_its_filter() {
    let program = std::process::Command::new(env!("CARGO_BIN_EXE_outside-reach"))
        .arg("--trace-filter")
        .output()
        .expect("the reader under test");
    assert!(program.status.success());
    assert_eq!(
        String::from_utf8_lossy(&program.stdout).trim(),
        outside_reach::TRACE_FILTER,
        "the seam has to answer with the constant the model is written against"
    );

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("tools/<name> sits two levels under the root");
    let workflows = root.join(".github/workflows");
    let mut compared = 0;
    for entry in std::fs::read_dir(&workflows).expect("this repository has workflows") {
        let path = entry.expect("a directory entry").path();
        let text = std::fs::read_to_string(&path).expect("a workflow");
        for line in text.lines() {
            let Some(at) = line.find("-e trace=") else {
                continue;
            };
            let spelled = line[at + "-e trace=".len()..]
                .split_whitespace()
                .next()
                .unwrap_or_default();
            assert_eq!(
                spelled,
                outside_reach::TRACE_FILTER,
                "{} builds a census with a filter this reader did not give it — a \
                 second copy of the model's requirements, free to drift the way \
                 R1233 measured",
                path.display()
            );
            compared += 1;
        }
    }
    assert!(
        compared > 0,
        "no workflow in {} names a trace filter at all — this law found nothing \
         to judge, which is not the same as finding nothing wrong",
        workflows.display()
    );
}
