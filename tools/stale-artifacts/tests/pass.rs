//! What the freshness pass decides, and that it actually does it.
//!
//! THE FIXTURES ARE TREES AND NOT CALLS, for the reason the sibling gates give:
//! the answer is the join of two readings — what git says this tree has changed
//! and what cargo says a workspace contains — and neither can be faked into a
//! function call without measuring the fake instead. So every case here is a git
//! repository holding a root workspace and a separate one beside it, and the
//! only thing that varies is which file was touched and which command is about
//! to run.
//!
//! THE DEFECT THESE ARE ABOUT (N139). The shell this replaced asked git for the
//! files changed under `crates/` and cleaned `cargo clean -p <directory name>`
//! for each. Every case below that names a package outside `crates/`, or names a
//! package whose name is not its directory's, or names a workspace that is not
//! the root one, is a case that answered NOTHING before this program existed —
//! while `scripts/check-side-workspaces.sh` ran twenty-three workspaces' suites
//! through that same wrapper.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ci_plan::issue;
use tempfile::TempDir;

use stale_artifacts::{clean_arguments, plan, Plan};

/// A git repository with a root workspace and one separate workspace beside it.
///
/// THE ROOT WORKSPACE HAS A MEMBER OUTSIDE `crates/` ON PURPOSE. This repository
/// has one too — `tools/ci-plan` is a root member living under `tools/` — and it
/// is the cheapest case that the directory-shaped population could never see.
///
/// AND ONE MEMBER'S PACKAGE NAME IS NOT ITS DIRECTORY'S, which is the other
/// thing the shell could not have got right: `cargo clean -p` takes a package
/// name, and the old pass handed it a directory name that happened to match for
/// every crate under `crates/`.
struct Tree {
    dir: TempDir,
    repo: PathBuf,
}

impl Tree {
    fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).expect("the fixture repository");
        let tree = Self { dir, repo };
        tree.write(
            "Cargo.toml",
            "[workspace]\n\
             resolver = \"2\"\n\
             members = [\"crates/in-crates\", \"tools/named-otherwise\"]\n",
        );
        tree.package("crates/in-crates", "in-crates");
        tree.package("tools/named-otherwise", "renamed-package");
        // A WORKSPACE ROOT THAT IS ITSELF A PACKAGE, WITH A MEMBER BELOW IT.
        // `tools/injection-harness` and `tools/experiment-harness` are this
        // shape here, and it is the only shape where "which package owns this
        // file" has two answers: `side/nested/src/lib.rs` is inside BOTH
        // directories, and the shallower one owns every file of the deeper one
        // under any reading that stops at the first match.
        //
        // AND IT PATH-DEPENDS ON TWO PACKAGES OUTSIDE ITSELF, one on each side of
        // the boundary this pass draws. `../crates/in-crates` belongs to ANOTHER
        // workspace of this repository, which is what six `tools/*` crates are to
        // `tools/ci-plan`: the suite about to run compiles it and this tree can
        // change it. `../../outside` is not in the repository at all, which is
        // what `studio`'s sibling `pinion` checkout is: this tree cannot change
        // it and git here never reports it.
        tree.write(
            "side/Cargo.toml",
            "[workspace]\n\
             members = [\"nested\"]\n\
             \n\
             [package]\n\
             name = \"side-crate\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n\
             \n\
             [dependencies]\n\
             in-crates = { path = \"../crates/in-crates\" }\n\
             outside-crate = { path = \"../../outside\" }\n",
        );
        tree.write(
            "side/src/lib.rs",
            "pub fn side() { in_crates::committed(); outside_crate::committed() }\n",
        );
        tree.package("side/nested", "nested-crate");
        tree.beside("outside", "outside-crate");
        tree.write("a-file-no-package-owns.md", "not code\n");
        // AND BOTH WORKSPACES ARE RESOLVED BEFORE THEY ARE COMMITTED, which is
        // what this repository's tracked `Cargo.lock` files are. `--locked`
        // forbids writing a lockfile, so a workspace nobody has resolved cannot
        // be read under a pinned command — and that is the pass declining
        // exactly where the command it precedes declines, which R1273 measured
        // in all four combinations rather than accepting.
        //
        // ⚠ THE REASON WROTE HERE UNTIL R1273 WAS WRONG, and it was wrong the
        // same way twice. R1257 first said `--no-deps` would avoid needing a
        // lockfile, and an injection disproved it; it then said cargo wants one
        // "for any manifest that DECLARES A DEPENDENCY", and R1273 disproved
        // that by running the fixture ROOT — which declares none — and watching
        // it decline too. The condition is the pin and the missing file, and
        // dependencies have nothing to do with it.
        tree.resolve("Cargo.toml");
        tree.resolve("side/Cargo.toml");
        tree.git(&["init", "-q", "."]);
        tree.git(&["config", "user.email", "pass@test"]);
        tree.git(&["config", "user.name", "freshness pass test"]);
        tree.git(&["config", "commit.gpgsign", "false"]);
        tree.git(&["add", "-A"]);
        tree.git(&["commit", "-qm", "the state this pass compares against"]);
        tree
    }

    fn package(&self, directory: &str, name: &str) {
        self.write(
            &format!("{directory}/Cargo.toml"),
            &format!(
                "[package]\n\
                 name = \"{name}\"\n\
                 version = \"0.1.0\"\n\
                 edition = \"2021\"\n"
            ),
        );
        self.write(
            &format!("{directory}/src/lib.rs"),
            "pub fn committed() {}\n",
        );
    }

    /// A package NEXT TO the repository rather than in it — the fixture's
    /// sibling checkout.
    fn beside(&self, directory: &str, name: &str) {
        let at = self.dir.path().join(directory);
        std::fs::create_dir_all(at.join("src")).expect("the sibling package");
        std::fs::write(
            at.join("Cargo.toml"),
            format!(
                "[package]\n\
                 name = \"{name}\"\n\
                 version = \"0.1.0\"\n\
                 edition = \"2021\"\n"
            ),
        )
        .expect("the sibling manifest");
        std::fs::write(at.join("src/lib.rs"), "pub fn committed() {}\n")
            .expect("the sibling source");
    }

    /// The repository — a directory INSIDE the fixture, so that something can
    /// sit beside it.
    fn at(&self) -> &Path {
        &self.repo
    }

    /// Write this workspace's lockfile, offline — the fixture has no registry
    /// dependency and a resolve that reached for one would be measuring this
    /// machine's network.
    fn resolve(&self, manifest: &str) {
        // `issue::Tree` spelled in full: this file has a `Tree` of its own, and
        // importing a second one would make the shorter name mean two things.
        let out = issue::cargo(issue::Tree::MadeByThisRun(
            "the fixture workspace this case wrote, whose lockfile is being \
             created here",
        ))
        .args([
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            manifest,
        ])
        .current_dir(self.at())
        .env("CARGO_TARGET_DIR", self.at().join(BUILD_DIRECTORY))
        .output()
        .expect("cargo resolves the fixture");
        assert!(
            out.status.success(),
            "the fixture workspace {manifest} did not resolve: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(&self, path: &str, body: &str) {
        let full = self.at().join(path);
        std::fs::create_dir_all(full.parent().expect("a file inside a directory"))
            .expect("the fixture directory");
        std::fs::write(&full, body).expect("the fixture file");
    }

    /// `git`, in this tree, refusing to continue when it fails — a fixture built
    /// on a repository that was not created is one whose assertions are about
    /// nothing.
    fn git(&self, arguments: &[&str]) {
        let out = Command::new("git")
            .args(arguments)
            .current_dir(self.at())
            .output()
            .expect("git, which this pass itself asks what changed");
        assert!(
            out.status.success(),
            "git {arguments:?} failed in the fixture: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn words(command: &[&str]) -> Vec<String> {
    command.iter().map(|word| (*word).to_string()).collect()
}

fn freshening(plan: Plan) -> stale_artifacts::Freshen {
    match plan {
        Plan::Freshen(freshen) => freshen,
        other => panic!("this case is about what gets cleaned, and the pass said: {other:?}"),
    }
}

/// Where the fixture's own builds go. NAMED rather than left to cargo's
/// default, because one case reads that directory back: a build and a clean
/// that disagreed about where artifacts live would assert about an empty
/// directory and pass.
const BUILD_DIRECTORY: &str = "build";

/// Run the program itself, the way `scripts/verify.sh` does.
fn program(at: &Path, command: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stale-artifacts"))
        .args(["--at", "."])
        .arg("--")
        .args(command)
        .current_dir(at)
        // NAMED RATHER THAN INHERITED (R1182): this pass asks cargo which
        // packages a workspace has, and which cargo it asks is `$CARGO` — set
        // when a suite runs under cargo and absent when the test binary is run
        // directly, where it would fall back to whatever is on PATH.
        .env("CARGO", env!("CARGO"))
        .env("CARGO_TARGET_DIR", at.join(BUILD_DIRECTORY))
        .output()
        .expect("the pass runs")
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn a_changed_package_of_a_separate_workspace_is_one_this_pass_cleans() {
    // THE WHOLE OF N139 IN ONE CASE. `scripts/check-side-workspaces.sh` runs
    // this repository's twenty-three separate workspaces' suites through the
    // wrapper, and the pass in front of them cleaned nothing at all — its
    // population was `crates/`, which is where the ROOT workspace's members
    // live, and `cargo clean -p` resolved the root workspace whatever the suite
    // was about. So a corrupted artifact of a changed `tools/` crate was read by
    // the suite meant to judge it, which is R743 with nothing watching.
    let tree = Tree::new();
    tree.write("side/src/lib.rs", "pub fn side() { /* changed */ }\n");
    let freshen = freshening(plan(
        tree.at(),
        &words(&[
            "cargo",
            "test",
            "--manifest-path",
            "side/Cargo.toml",
            "--locked",
        ]),
    ));
    assert_eq!(
        freshen.packages,
        vec!["side-crate".to_string()],
        "the pass in front of a separate workspace's suite has to clean that \
         workspace's changed package"
    );
    assert_eq!(
        freshen.manifest,
        tree.at().join("side/Cargo.toml"),
        "and clean it IN that workspace: `cargo clean -p` resolves the workspace \
         it is pointed at, so a clean aimed at the root would fail to find the \
         package and the pass would be a no-op with a green exit"
    );
}

#[test]
fn a_root_member_outside_the_crates_directory_is_in_the_population() {
    // THE DIRECTORY-SHAPED POPULATION, ASKED DIRECTLY. This repository's root
    // workspace has a member under `tools/`, so even the ROOT case was not fully
    // covered by a walk over `crates/` — a changed `tools/ci-plan` was never
    // freshened either, and that crate is a dependency of the gates.
    let tree = Tree::new();
    tree.write(
        "tools/named-otherwise/src/lib.rs",
        "pub fn committed() { /* changed */ }\n",
    );
    let freshen = freshening(plan(tree.at(), &words(&["cargo", "test", "--locked"])));
    assert_eq!(
        freshen.packages,
        vec!["renamed-package".to_string()],
        "a root member that does not live under `crates/` is still a package \
         whose artifacts a changed file invalidates"
    );
}

#[test]
fn the_package_named_is_cargos_name_and_not_the_directorys() {
    // The same case read the other way, because these are two failures and one
    // fixture. `cargo clean -p` takes a PACKAGE name; the shell handed it a path
    // component, and `cargo clean -p named-otherwise` is an error rather than a
    // clean — which the old pass discarded with `|| true`.
    let tree = Tree::new();
    tree.write(
        "tools/named-otherwise/src/lib.rs",
        "pub fn committed() { /* changed */ }\n",
    );
    let freshen = freshening(plan(tree.at(), &words(&["cargo", "test"])));
    assert!(
        !freshen.packages.contains(&"named-otherwise".to_string()),
        "the directory's name is not the package's, and cargo answers about \
         packages: {:?}",
        freshen.packages
    );
}

#[test]
fn a_run_over_one_workspace_does_not_clean_a_package_it_never_compiles() {
    // THE OPPOSITE FAILURE, and it is the one a first repair reaches for:
    // cleaning every changed package everywhere is correct in the sense that
    // nothing stale survives, and it throws away the artifacts of twenty-two
    // workspaces the run will not build. Every suite in the side-workspace gate
    // would then rebuild what the one before it had just built.
    let tree = Tree::new();
    tree.write("side/src/lib.rs", "pub fn side() { /* changed */ }\n");
    tree.write(
        "tools/named-otherwise/src/lib.rs",
        "pub fn committed() { /* changed */ }\n",
    );
    let freshen = freshening(plan(
        tree.at(),
        &words(&["cargo", "test", "--manifest-path", "side/Cargo.toml"]),
    ));
    assert_eq!(
        freshen.packages,
        vec!["side-crate".to_string()],
        "a changed package nothing in this resolve reaches is not this run's to \
         clean"
    );
    assert_eq!(
        freshen.changed, 2,
        "and the pass still reports how many paths the tree has changed, so \
         `this resolve reaches none of them` and `git answered nothing` are not \
         the same line"
    );
}

#[test]
fn a_changed_path_dependency_outside_this_workspace_is_still_code_the_run_compiles() {
    // THE OTHER EDGE OF THE SAME LINE, and it is this repository's own shape:
    // six workspaces under `tools/` path-depend on `tools/ci-plan`, which is a
    // member of the ROOT workspace and of none of theirs. A pass that asked
    // cargo for MEMBERS would report `0 packages changed` for every one of them
    // on a round that edited `ci-plan` — while the suite about to run is the one
    // that compiles it. What this law is about is code the tree has changed, and
    // where the manifest declaring it sits is not the question.
    let tree = Tree::new();
    tree.write(
        "crates/in-crates/src/lib.rs",
        "pub fn committed() { /* changed */ }\n",
    );
    let freshen = freshening(plan(
        tree.at(),
        &words(&["cargo", "test", "--manifest-path", "side/Cargo.toml"]),
    ));
    assert_eq!(
        freshen.packages,
        vec!["in-crates".to_string()],
        "a path dependency inside this checkout is a package this run compiles \
         and this tree can change"
    );
}

#[test]
fn the_package_that_owns_a_changed_file_is_the_deepest_one_holding_it() {
    // TWO PACKAGES CONTAIN THIS FILE and exactly one of them compiles it.
    // `side/nested/src/lib.rs` is inside `side/` as well, so a reading that
    // takes the first member whose directory is a prefix cleans `side-crate`
    // and leaves the artifacts of the package that actually changed — a wrong
    // rebuild AND the stale binary, from one line.
    let tree = Tree::new();
    tree.write(
        "side/nested/src/lib.rs",
        "pub fn committed() { /* changed */ }\n",
    );
    let freshen = freshening(plan(
        tree.at(),
        &words(&["cargo", "test", "--manifest-path", "side/Cargo.toml"]),
    ));
    assert_eq!(
        freshen.packages,
        vec!["nested-crate".to_string()],
        "the deepest package holding a changed file is the one whose artifacts \
         that file invalidates"
    );
}

#[test]
fn an_untracked_file_in_a_package_is_a_change_against_head() {
    // A FILE HEAD DOES NOT HAVE IS A DIFFERENCE FROM HEAD. `git diff HEAD`
    // answers about tracked files only, so a module added to a crate and not yet
    // committed left that crate's artifacts in place — and what those artifacts
    // are of is a version of the crate that no longer exists.
    let tree = Tree::new();
    tree.write("crates/in-crates/src/added.rs", "pub fn added() {}\n");
    let freshen = freshening(plan(tree.at(), &words(&["cargo", "test"])));
    assert_eq!(
        freshen.packages,
        vec!["in-crates".to_string()],
        "an untracked source file is a change this pass has to act on"
    );
}

#[test]
fn a_package_outside_this_repository_is_not_one_this_pass_counts() {
    // THE BOUNDARY, AND THE NUMBER IS WHERE IT SHOWS. Reading a resolve WITH
    // dependencies is what reaches a path dependency of another workspace, and
    // it also reaches every registry crate and — for `studio` — a sibling
    // checkout this repository does not own. None of those is code git here can
    // report changed, so none of them can ever be selected; what they would
    // change is the number this pass prints beside the selection, and a
    // denominator that counted four hundred registry crates would say nothing
    // about whether the pass read anything.
    //
    // ASSERTED ON THE COUNT because that is the only thing the filter moves. A
    // case that asserted on the selection instead would pass with the filter
    // deleted, which is a test measuring nothing.
    let tree = Tree::new();
    tree.write("side/src/lib.rs", "pub fn side() { /* changed */ }\n");
    let freshen = freshening(plan(
        tree.at(),
        &words(&["cargo", "test", "--manifest-path", "side/Cargo.toml"]),
    ));
    assert_eq!(
        freshen.members, 3,
        "`side/`'s resolve holds four packages and one of them lives outside \
         this repository: side-crate, nested-crate and in-crates are this \
         tree's, outside-crate is not"
    );
}

#[test]
fn a_changed_file_no_package_owns_cleans_nothing_and_still_says_what_it_saw() {
    let tree = Tree::new();
    tree.write("a-file-no-package-owns.md", "not code, changed\n");
    let freshen = freshening(plan(tree.at(), &words(&["cargo", "test"])));
    assert!(
        freshen.packages.is_empty(),
        "a file outside every package invalidates no artifact: {:?}",
        freshen.packages
    );
    assert_eq!(
        (freshen.changed, freshen.members),
        (1, 2),
        "and the numbers are still printed, because `nothing to clean out of \
         two packages` and `cargo answered about no packages at all` are the \
         two states an empty list hides"
    );
}

#[test]
fn the_clean_pins_the_lockfile_exactly_when_the_command_it_precedes_does() {
    // WHOSE RESOLUTION THIS IS, answered without a second reading of it.
    // `cargo clean` resolves the workspace it is pointed at and will rewrite a
    // lockfile it disagrees with, so the pass must pin — except for a workspace
    // this repository does not own, where a tracked lockfile is a file every run
    // rewrites and `--locked` is a gate that goes red for somebody else's
    // commit. `check-side-workspaces.sh` already decides that, once, and puts
    // the answer on the command: so the clean copies the command rather than
    // asking again.
    let tree = Tree::new();
    tree.write("side/src/lib.rs", "pub fn side() { /* changed */ }\n");
    let pinned = freshening(plan(
        tree.at(),
        &words(&[
            "cargo",
            "test",
            "--manifest-path",
            "side/Cargo.toml",
            "--locked",
        ]),
    ));
    assert!(
        clean_arguments(&pinned, "side-crate").contains(&"--locked".to_string()),
        "a pinned command's freshness pass has to pin too, or the pass rewrites \
         the lockfile the run was about to be judged against"
    );
    let free = freshening(plan(
        tree.at(),
        &words(&["cargo", "test", "--manifest-path", "side/Cargo.toml"]),
    ));
    assert!(
        !clean_arguments(&free, "side-crate").contains(&"--locked".to_string()),
        "and an unpinned one must NOT: this repository takes `--locked` off the \
         workspaces whose resolution it does not own, and a pass that put it \
         back would fail every one of their runs"
    );
}

#[test]
fn the_joined_spelling_of_the_manifest_flag_is_the_same_flag() {
    // `--manifest-path=side/Cargo.toml` and `--manifest-path side/Cargo.toml`
    // are one flag, and a reader that knew only the second answers "no manifest"
    // for the first — which is not a refusal, it is silently freshening the ROOT
    // workspace for a run that builds another one. `ci_plan::CargoCommand` is
    // where this repository knows both spellings, which is why this pass reads
    // the command through it rather than through a walk written here.
    let tree = Tree::new();
    tree.write("side/src/lib.rs", "pub fn side() { /* changed */ }\n");
    let freshen = freshening(plan(
        tree.at(),
        &words(&["cargo", "test", "--manifest-path=side/Cargo.toml"]),
    ));
    assert_eq!(freshen.manifest, tree.at().join("side/Cargo.toml"));
    assert_eq!(freshen.packages, vec!["side-crate".to_string()]);
}

#[test]
fn a_command_that_runs_no_cargo_is_a_stated_nothing_rather_than_a_silence() {
    // RULEBOOK asks a round to run `scripts/verify.sh -- scripts/check-side-\
    // workspaces.sh`, and that command builds no workspace of its own: every
    // cargo command it runs goes through this same wrapper and is freshened
    // there. So there is nothing to do here, and saying nothing at all is how a
    // pass that had stopped working would look.
    let tree = Tree::new();
    let plan = plan(tree.at(), &words(&["scripts/check-side-workspaces.sh"]));
    let Plan::Nothing(reason) = &plan else {
        panic!("a command that is not cargo builds no workspace: {plan:?}")
    };
    assert!(
        reason.contains("runs no cargo command"),
        "and the reason is printed rather than left as an empty answer: {reason}"
    );
}

/// A COMMAND THAT MEETS NO ARTIFACT NEEDS NO CLEAN IN FRONT OF IT.
///
/// R1272. This pass exists for R743's stale binary, and a subcommand that
/// neither produces nor reads a compiled artifact cannot have that problem —
/// `cargo metadata` and `cargo fmt` are the cases. Cleaning in front of one buys
/// nothing and costs whoever compiles next a full rebuild of every package the
/// tree has changed, which on this repository is most of them.
///
/// THE TABLE IS MEASURED, WHICH IS WHY THE SKIP IS SAFE. `ci_plan::compiles`
/// records what cargo did in a workspace built for the purpose
/// (`compiling_subcommands`), and only a measured `false` skips: `None` means
/// nobody asked, and the conservative reading of that is to clean.
#[test]
fn a_command_that_leaves_no_artifact_is_not_cleaned_in_front_of() {
    let tree = Tree::new();
    tree.write(
        "crates/in-crates/src/lib.rs",
        "pub fn committed() { /* changed */ }\n",
    );
    for line in [
        &["cargo", "metadata", "--format-version", "1"][..],
        &["cargo", "fmt", "--check"][..],
        &["cargo", "tree"][..],
    ] {
        let plan = plan(tree.at(), &words(line));
        let Plan::Nothing(reason) = &plan else {
            panic!("`{}` meets no artifact: {plan:?}", line.join(" "))
        };
        assert!(
            reason.contains("leaves nothing a targeted clean removes"),
            "and the reason says which of the nothings it is: {reason}"
        );
    }

    // AND THE SAME TREE IS STILL FRESHENED FOR A COMMAND THAT DOES COMPILE,
    // which is what says the clause above is about the subcommand and not about
    // the tree having nothing changed.
    let freshen = freshening(plan(tree.at(), &words(&["cargo", "test"])));
    assert!(
        !freshen.packages.is_empty(),
        "the tree has a changed package and a compiling command finds it: {freshen:?}"
    );
}

/// AND A SUBCOMMAND NOBODY MEASURED IS CLEANED IN FRONT OF.
///
/// R1272, the conservative half. An unknown subcommand answers `None`, and the
/// price of the two mistakes is not symmetric: freshening a command that did not
/// need it costs a rebuild, and skipping one that did costs R743's stale binary
/// passing a suite.
#[test]
fn a_subcommand_this_repository_has_not_measured_is_still_cleaned_in_front_of() {
    let tree = Tree::new();
    tree.write(
        "crates/in-crates/src/lib.rs",
        "pub fn committed() { /* changed */ }\n",
    );
    let freshen = freshening(plan(
        tree.at(),
        &words(&["cargo", "not-a-subcommand-anybody-measured"]),
    ));
    assert!(
        !freshen.packages.is_empty(),
        "an unmeasured subcommand is assumed to compile: {freshen:?}"
    );
}

#[test]
fn a_tree_that_is_not_a_repository_is_a_reason_and_not_a_refusal() {
    // What differs from HEAD is a question about a repository, and a tree
    // without one has no HEAD to differ from — the tree IS its own state. That
    // is a pass with nothing to do rather than a pass that could not run, and
    // the difference matters because the caller fails on the second.
    let dir = TempDir::new().expect("tempdir");
    let plan = plan(dir.path(), &words(&["cargo", "test"]));
    let Plan::Nothing(reason) = &plan else {
        panic!("a tree with no repository has nothing to compare: {plan:?}")
    };
    assert!(
        reason.contains("not a git repository"),
        "and it says which of the two it is: {reason}"
    );
}

#[test]
fn a_workspace_cargo_cannot_read_is_a_refusal_the_caller_fails_on() {
    // THE ONE ANSWER THAT MUST NOT BE QUIET. A pass that could not find out
    // which packages a workspace has has not run, and a run that continues after
    // it is a run that can read exactly the artifact the pass exists to remove.
    // The old shell's `2>/dev/null` and `|| true` made this state indis-
    // tinguishable from a clean tree.
    let tree = Tree::new();
    let out = program(
        tree.at(),
        &["cargo", "test", "--manifest-path", "nowhere/Cargo.toml"],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "a workspace cargo cannot read is exit 2:\n{}",
        said(&out)
    );
    assert!(
        said(&out).contains("NOT RUN"),
        "and it says so in words, because the exit code is read by a shell and \
         the sentence is read by whoever has to fix it:\n{}",
        said(&out)
    );
}

#[test]
fn a_workspace_with_no_lockfile_is_a_refusal_under_a_pinned_command() {
    // A WORKSPACE WITH DEPENDENCIES NEEDS A LOCKFILE, and `--locked` forbids
    // writing one. So a workspace this repository has never resolved cannot be
    // freshened under a pinned command.
    //
    // NOT A PRICE OF READING THE RESOLVE, WHICH IS WHAT THIS COMMENT SAID UNTIL
    // AN INJECTION DISPROVED IT. The obvious reading is that reaching a path
    // dependency costs the lockfile, so `--no-deps` would not need one — and the
    // sweep's `the-pass-asks-for-members-rather-than-for-the-resolve` puts
    // `--no-deps` back and this case STAYS GREEN. What `--no-deps` changes is
    // only which packages come back.
    //
    // ⚠ AND THE SECOND EXPLANATION WAS WRONG TOO. This comment then said cargo
    // wants the lockfile "once the manifest declares a dependency", and R1273
    // ran the fixture ROOT — which declares none — and watched it decline just
    // the same. The condition is `--locked` and a missing file; dependencies
    // have nothing to do with it.
    //
    // IT IS THE RIGHT DIRECTION AND THAT IS WHY IT IS ACCEPTED: the command this
    // pass precedes carries the same `--locked` and declines for the same reason
    // one step later. That sentence was an assumption until R1273 ran both
    // commands in all four combinations of pin and dependency —
    // `the_pass_and_the_run_it_precedes_decline_in_the_same_place` is where it
    // is now measured, in both directions.
    let tree = Tree::new();
    std::fs::remove_file(tree.at().join("side/Cargo.lock")).expect("the fixture's lockfile");
    let out = program(
        tree.at(),
        &[
            "cargo",
            "test",
            "--manifest-path",
            "side/Cargo.toml",
            "--locked",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "a resolve this pass cannot read is a refusal, not a tree with nothing \
         to clean:\n{}",
        said(&out)
    );
    assert!(
        said(&out).contains("--locked"),
        "and the reason names the flag that forbade it, because the reader has \
         to know whether to resolve the workspace or drop the pin:\n{}",
        said(&out)
    );
}

/// One cargo command in the fixture, for the measurement below.
fn in_fixture(tree: &Tree, arguments: &[&str]) -> Output {
    issue::cargo(issue::Tree::MadeByThisRun(
        "the fixture workspace this measurement wrote, whose lockfile it removed \
         on purpose to see who declines",
    ))
    .args(arguments)
    .current_dir(tree.at())
    .env("CARGO_TARGET_DIR", tree.at().join(BUILD_DIRECTORY))
    .output()
    .expect("cargo runs")
}

/// THE PASS DECLINES EXACTLY WHERE THE RUN IT PRECEDES WOULD, ASKED OF CARGO.
///
/// R1273, and R1257 asked for this in its own words. That round wrote the
/// no-lockfile refusal down as ACCEPTED rather than designed around, on the
/// grounds that "the command this pass precedes carries the same `--locked` and
/// would decline for the same reason one step later" — and never ran that
/// command. The case above it asserts the PASS declines; nothing asserted the
/// RUN does, so the sentence that made the refusal acceptable was the one part
/// of it nobody had measured.
///
/// WHY IT IS NOT A DETAIL. If the run would have SUCCEEDED, this pass is
/// manufacturing a failure that nothing else in the toolchain would have raised
/// — a gate that turns a working tree red for a reason the work does not have.
/// That is the direction R1257's own comment named as the one to rule out ("a
/// pass that was more permissive than the run it guards would be answering about
/// a resolve the run will not use"), and this measures both directions of it.
///
/// AND THE OTHER HALF OF THE POPULATION IS HERE TOO. R1257 said cargo wants a
/// lockfile for a manifest that DECLARES A DEPENDENCY, so a workspace with none
/// should be the case where the refusal does not happen. `side` declares two path
/// dependencies and the fixture root declares none — and measured, BOTH decline.
/// The condition is the pin and the missing file; that explanation was the second
/// wrong one R1257 gave for this refusal, and an injection had already disproved
/// the first.
///
/// ⚠ WHAT THIS CASE IS STRUCTURALLY BLIND TO, and the harness is what said so. A
/// comparison of two verdicts cannot see a change that moves BOTH: drop the pin
/// the pass copies and it resolves the workspace freely, WRITES the lockfile, and
/// the run then succeeds as well — so the two exit codes agree and this case
/// passes while the pass has just erased the very thing the run was to be held
/// to. That is R1115's original defect one program over, and what catches it is
/// `a_workspace_with_no_lockfile_is_a_refusal_under_a_pinned_command` beside this
/// one, which asks the pass for a refusal rather than for agreement. Recorded
/// here because the injection aimed at this case came back MISSED and the honest
/// answer was that it should have been aimed elsewhere.
#[test]
fn the_pass_and_the_run_it_precedes_decline_in_the_same_place() {
    let tree = Tree::new();
    std::fs::remove_file(tree.at().join("side/Cargo.lock")).expect("the fixture's lockfile");
    std::fs::remove_file(tree.at().join("Cargo.lock")).expect("the fixture root's lockfile");

    // THE PASS IS ASKED THROUGH ITS OWN BINARY AND NOT REBUILT HERE. A case that
    // assembled `cargo metadata --locked` by hand would be a second spelling of
    // the commands this pass issues, free to keep passing on the day the pass
    // stopped copying the pin — which is exactly the failure it is written to
    // catch. The run is the SAME WORDS the pass was handed, run directly.
    for (manifest, declares_a_dependency) in [("side/Cargo.toml", true), ("Cargo.toml", false)] {
        let wrapped = [
            "cargo",
            "test",
            "--no-run",
            "--manifest-path",
            manifest,
            "--locked",
        ];
        let pass = program(tree.at(), &wrapped);
        let the_run = in_fixture(&tree, &wrapped[1..]);
        println!(
            "{manifest} (declares a dependency: {declares_a_dependency}) with no lockfile, \
             under `--locked`: the pass {}, the run it precedes {}",
            if pass.status.success() {
                "SUCCEEDS"
            } else {
                "declines"
            },
            if the_run.status.success() {
                "SUCCEEDS"
            } else {
                "declines"
            }
        );
        // THE HALF THAT MATTERS MOST, stated on its own so its failure reads as
        // itself: the pass may not decline where the run would have gone
        // through. That is a gate turning a tree red for a reason the work does
        // not have, and R1257's own comment named it as the direction to rule
        // out — "a pass that was more permissive than the run it guards would be
        // answering about a resolve the run will not use" is the mirror, and
        // this is the side that costs somebody a red they cannot act on.
        assert!(
            the_run.status.success() || !pass.status.success(),
            "the pass went through and the run it precedes did NOT, so this pass \
             is answering about a resolve the run will not use — {manifest}:\n{}",
            String::from_utf8_lossy(&the_run.stderr)
        );
        assert!(
            !(pass.status.success() ^ the_run.status.success()),
            "the pass and the run disagree about whether {manifest} can be \
             resolved under `--locked` with no lockfile, so R1257's reason for \
             ACCEPTING the refusal — that the run declines one step later — is \
             not true of this cargo:\npass: {}\nrun: {}",
            said(&pass),
            String::from_utf8_lossy(&the_run.stderr)
        );

        // AND THE MIRROR, WHICH IS THE HALF THIS REPOSITORY ACTUALLY MEETS. An
        // UNPINNED run over a workspace with no lockfile goes through — cargo
        // writes one — and `studio` is exactly that shape here: not ours to pin,
        // its lockfile untracked, so a fresh clone has none. The pass in front of
        // it must go through too, and a pass that pinned when the run did not
        // would turn every fresh clone red on a workspace the run is happy with.
        let unpinned = ["cargo", "test", "--no-run", "--manifest-path", manifest];
        let pass_unpinned = program(tree.at(), &unpinned);
        let run_unpinned = in_fixture(&tree, &unpinned[1..]);
        println!(
            "{manifest} with no lockfile, UNPINNED: the pass {}, the run it \
             precedes {}",
            if pass_unpinned.status.success() {
                "SUCCEEDS"
            } else {
                "declines"
            },
            if run_unpinned.status.success() {
                "SUCCEEDS"
            } else {
                "declines"
            }
        );
        assert!(
            !(pass_unpinned.status.success() ^ run_unpinned.status.success()),
            "the pass and the run disagree about {manifest} with no lockfile and \
             no pin — and this is the shape `studio` has in a fresh clone, so a \
             pass stricter than its run here reddens a checkout nothing is wrong \
             with:\npass: {}\nrun: {}",
            said(&pass_unpinned),
            String::from_utf8_lossy(&run_unpinned.stderr)
        );
        // The unpinned resolve WROTE a lockfile, so put the tree back for the
        // next manifest — a case that measured a missing lockfile and then left
        // one behind would measure something else the second time round.
        for lockfile in ["Cargo.lock", "side/Cargo.lock"] {
            let _ = std::fs::remove_file(tree.at().join(lockfile));
        }
    }
}

#[test]
fn the_pass_removes_the_artifact_and_not_merely_the_line_about_it() {
    // THE ACT, ASKED OF CARGO'S OWN BUILD DIRECTORY. Every case above is about
    // the decision; this one is about whether anything happens. A pass that
    // prints `cleaning side-crate` and cleans nothing is indistinguishable from
    // a working one in the only place anybody looks, and what it leaves behind
    // is the stale artifact this whole mechanism exists to remove.
    let tree = Tree::new();
    let built = issue::cargo(issue::Tree::MadeByThisRun(
        "the fixture workspace this case wrote, whose lockfile `resolve` made a \
         moment ago",
    ))
    .args(["build", "--manifest-path", "side/Cargo.toml"])
    .current_dir(tree.at())
    .env("CARGO_TARGET_DIR", tree.at().join(BUILD_DIRECTORY))
    .output()
    .expect("cargo builds the fixture");
    assert!(
        built.status.success(),
        "the fixture has to compile before an artifact of it can be removed: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let fingerprints = tree.at().join(BUILD_DIRECTORY).join("debug/.fingerprint");
    for package in ["side-crate", "in-crates"] {
        assert!(
            fingerprint_of(&fingerprints, package),
            "cargo records a fingerprint per package it builds, and this case \
             reads that directory rather than a message: {package} is absent \
             from {}",
            fingerprints.display()
        );
    }

    // BOTH SIDES OF THE MEMBERSHIP LINE, CHANGED AT ONCE. `side-crate` is the
    // workspace this command names; `in-crates` is a path dependency belonging
    // to ANOTHER workspace, which is what six of this repository's `tools/*`
    // crates are to `tools/ci-plan`. Whether cargo will even accept
    // `-p in-crates --manifest-path side/Cargo.toml` is a question about cargo
    // rather than about this program, so it is measured here rather than
    // asserted in prose: a cargo that refused would make the pass exit 2 and
    // this case red, which is the honest way to find out.
    tree.write(
        "side/src/lib.rs",
        "pub fn side() { in_crates::committed() /* changed */ }\n",
    );
    tree.write(
        "crates/in-crates/src/lib.rs",
        "pub fn committed() { /* changed */ }\n",
    );
    let out = program(
        tree.at(),
        &["cargo", "test", "--manifest-path", "side/Cargo.toml"],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "the pass has to succeed before its effect is about anything:\n{}",
        said(&out)
    );
    for package in ["side-crate", "in-crates"] {
        assert!(
            !fingerprint_of(&fingerprints, package),
            "the pass said it was cleaning and cargo's build directory still \
             holds {package}'s fingerprint, so the run about to be judged can be \
             handed a binary built from source this tree no longer has:\n{}",
            said(&out)
        );
    }
}

/// Does cargo's build directory hold a fingerprint for this package?
///
/// READ FROM CARGO'S OWN DIRECTORY rather than from a message this program
/// prints, for the reason the case above gives. The shape (`<name>-<hash>`) is
/// cargo's, so a cargo that changed it turns this case red rather than leaving
/// it quietly asserting nothing — which is why the absence is asserted only
/// after the presence has been.
fn fingerprint_of(fingerprints: &Path, package: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(fingerprints) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with(&format!("{package}-"))
    })
}
