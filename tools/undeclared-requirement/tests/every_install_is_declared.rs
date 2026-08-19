//! The law's arms, each killed by a different minimal signature.
//!
//! # Why the cases stand outside this checkout
//!
//! Both sides of this law are files at fixed paths inside a repository root —
//! `.github/workflows/` and `.claude/remote-build.toml` — and the population is
//! read from `git ls-files`, so the only way to vary either is to build a
//! throwaway repository and stand in it.
//!
//! # Why one case is the repository itself
//!
//! The rest hold the reading against strings, which is what makes a branch this
//! machine never takes assertable. The live one is what keeps the fixtures from
//! drifting into a shape the real files never have: it runs the built binary
//! over this checkout and asserts the verdict it must give today.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use undeclared_requirement::{
    action_installs, judge, read_command, read_declaration, Install, Site,
};

/// One command's words, split the way the workflows' own reader splits them.
///
/// NOT `split_whitespace`. The production path trims the quotes off a word, and
/// a helper that did not would hand this reader `'apt-get` where the gate sees
/// `apt-get` — the first draft of the `bash -c '…'` case passed on exactly that
/// difference, over a wrapper the gate in fact refuses.
fn words(line: &str) -> Vec<String> {
    let mut commands = ci_plan::shell_commands(line);
    assert_eq!(commands.len(), 1, "one case is one command: {commands:?}");
    commands.remove(0)
}

// --- the reading -------------------------------------------------------------

#[test]
fn the_install_this_repository_actually_writes_is_read_down_to_its_packages() {
    // THE CONTROL, in the exact words `.github/workflows/mnemosyne-validate.yml`
    // has carried since R888. Without it the cases below prove only that this
    // reader can say "refused", which every broken reader also does.
    let read = read_command(&words("sudo apt-get install -y protobuf-compiler"));
    assert_eq!(
        read,
        undeclared_requirement::Command::Read {
            manager: "apt-get".to_owned(),
            packages: vec!["protobuf-compiler".to_owned()],
        },
        "{read:?}"
    );
}

#[test]
fn refreshing_the_index_is_not_installing_anything() {
    assert_eq!(
        read_command(&words("sudo apt-get update")),
        undeclared_requirement::Command::Nothing,
        "`apt-get update` names no package, and a reader that counted it as an \
         install would then have to refuse for having none"
    );
}

#[test]
fn a_manager_behind_a_wrapper_refuses_rather_than_reading_nothing() {
    // THE ARM THAT MATTERS MOST. A reader that only looked at the head word
    // would answer "installs nothing" here, and zero findings is what a clean
    // tree looks like.
    let read = read_command(&words(
        "env DEBIAN_FRONTEND=noninteractive apt-get install -y protobuf-compiler",
    ));
    let undeclared_requirement::Command::Refused { why } = read else {
        panic!("a wrapper this cannot see through must refuse: {read:?}");
    };
    assert!(
        why.contains("apt-get") && why.contains("in front"),
        "the refusal names what it saw and why it could not read it: {why}"
    );
    assert_eq!(
        read_command(&words("bash -c 'apt-get install -y protobuf-compiler'")),
        read_command(&words(
            "env DEBIAN_FRONTEND=noninteractive apt-get install -y protobuf-compiler"
        )),
        "and a shell is a wrapper like any other"
    );
}

#[test]
fn a_package_the_shell_decides_at_run_time_is_refused() {
    let read = read_command(&words("apt-get install -y $PACKAGES"));
    let undeclared_requirement::Command::Refused { why } = read else {
        panic!("a name no file can be held against must refuse: {read:?}");
    };
    assert!(why.contains("$PACKAGES"), "{why}");
}

#[test]
fn a_flag_that_might_take_the_next_word_is_refused() {
    // `-t bookworm-backports protobuf-compiler` installs ONE package, and a
    // reader that did not know the flag would report a release codename as an
    // undeclared requirement — sending somebody to write it into the
    // declaration, where it would then sit forever looking like a tool.
    let read = read_command(&words(
        "apt-get install -y -t bookworm-backports protobuf-compiler",
    ));
    let undeclared_requirement::Command::Refused { why } = read else {
        panic!("an unknown flag must refuse: {read:?}");
    };
    assert!(why.contains("-t"), "{why}");
    // The flags it does know, and the `--opt=value` form that carries its own
    // value, stay readable — a refusal for every flag would be a gate nobody can
    // keep green.
    assert_eq!(
        read_command(&words(
            "apt-get install -y --no-install-recommends -o=Dpkg::Use-Pty=0 clang"
        )),
        undeclared_requirement::Command::Read {
            manager: "apt-get".to_owned(),
            packages: vec!["clang".to_owned()],
        }
    );
}

#[test]
fn privilege_is_not_the_program_and_a_flag_that_takes_a_word_ends_the_stripping() {
    assert_eq!(
        read_command(&words("sudo -E apt-get install -y jq")),
        undeclared_requirement::Command::Read {
            manager: "apt-get".to_owned(),
            packages: vec!["jq".to_owned()],
        },
        "`sudo` and its own flags are not the program being run"
    );
    let read = read_command(&words("sudo -u root apt-get install -y jq"));
    assert!(
        matches!(read, undeclared_requirement::Command::Refused { .. }),
        "`-u` takes the next word, so the head after stripping is `root` and the \
         manager is no longer first — the strict end of that road: {read:?}"
    );
}

#[test]
fn another_managers_install_is_recognised_and_not_read() {
    assert_eq!(
        read_command(&words("rustup toolchain install 1.88")),
        undeclared_requirement::Command::Recognised {
            what: "rustup".to_owned()
        },
        "judging it would mean joining rustup's names against a declaration \
         written in apt's, which is a table"
    );
    assert_eq!(
        read_command(&words("rustup show")),
        undeclared_requirement::Command::Nothing,
        "and the step this repository actually writes installs nothing"
    );
}

#[test]
fn an_action_that_installs_is_read_from_the_only_thing_this_repository_holds() {
    for action in [
        "arduino/setup-protoc",
        "dtolnay/rust-toolchain",
        "awalsh128/cache-apt-pkgs-action",
    ] {
        assert!(
            action_installs(action),
            "{action} puts a tool on the runner"
        );
    }
    for action in [
        "actions/checkout",
        "actions/cache",
        "actions/upload-artifact",
        "actions/download-artifact",
    ] {
        assert!(
            !action_installs(action),
            "{action} is every `uses:` this repository writes today, and a law \
             that refused on them could never be green"
        );
    }
}

// --- the judgement -----------------------------------------------------------

fn installed(package: &str, job: &str) -> Install {
    Install {
        site: Site {
            source: ".github/workflows/x.yml".to_owned(),
            job: job.to_owned(),
            index: 3,
        },
        manager: "apt-get".to_owned(),
        packages: vec![package.to_owned()],
    }
}

#[test]
fn a_requirement_is_named_under_either_key_because_the_law_is_that_it_is_named() {
    let declared =
        read_declaration("needs = [\"cargo\", \"protoc\"]\npackages = [\"protobuf-compiler\"]\n")
            .expect("a declaration this shape parses");
    assert_eq!(
        declared["protoc"],
        vec!["needs".to_owned()],
        "which key matched is reported: {declared:?}"
    );
    assert!(
        judge(&[installed("protobuf-compiler", "validate")], &declared).is_empty(),
        "named under `packages`"
    );
    assert!(
        judge(&[installed("protoc", "validate")], &declared).is_empty(),
        "named under `needs` — demanding a particular key would force a second \
         spelling of a name that is the same word in both vocabularies"
    );
}

#[test]
fn the_pair_this_repository_actually_had_is_a_defect() {
    // THE HISTORICAL ARM, in the bytes both files really held. The workflow has
    // installed `protobuf-compiler` since 2026-07-31 (R888); the declaration was
    // written on 2026-08-12 with exactly these two keys, and the build machine
    // discovered the gap on 2026-08-14 after compiling 269 crates.
    let declared =
        read_declaration("needs = [\"cargo\"]\npackages = [\"libclang-common-18-dev\"]\n")
            .expect("the declaration as it was written");
    let findings = judge(&[installed("protobuf-compiler", "validate")], &declared);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].package, "protobuf-compiler", "{findings:?}");
}

#[test]
fn one_missing_line_is_one_finding_however_many_jobs_install_it() {
    let findings = judge(
        &[
            installed("protobuf-compiler", "validate"),
            installed("protobuf-compiler", "msrv"),
            installed("protobuf-compiler", "unrun-tests"),
        ],
        &BTreeMap::new(),
    );
    assert_eq!(
        findings.len(),
        1,
        "three findings for one missing line would report the size of the \
         workflow rather than the size of the defect: {findings:?}"
    );
    assert_eq!(
        findings[0].sites.len(),
        3,
        "and every site is named: {findings:?}"
    );
}

#[test]
fn a_requirement_key_that_is_not_a_list_of_names_refuses() {
    // An empty answer here would turn every install into a finding, which is a
    // loud WRONG answer rather than a missing one.
    assert!(read_declaration("packages = \"protobuf-compiler\"\n").is_err());
    assert!(read_declaration("packages = [1]\n").is_err());
    assert!(read_declaration("packages = [\"ok\"]\nthis is not toml\n").is_err());
    assert!(
        read_declaration("send = \"tracked\"\n")
            .expect("a declaration with no requirement key parses")
            .is_empty(),
        "declaring no requirement is a reading, not an error — what it means for \
         the verdict is the caller's question"
    );
}

// --- the whole gate, over a repository ---------------------------------------

/// A throwaway repository whose workflow and declaration are what a case says.
///
/// TRACKED, because the population comes from `git ls-files`: a workflow that is
/// not tracked is one GitHub does not run, and this law must read the same set.
fn repository(root: &Path, workflow: &str, declaration: Option<&str>) -> PathBuf {
    let repository = root.join("repository");
    std::fs::create_dir_all(repository.join(".github/workflows")).expect("make the workflows");
    std::fs::create_dir_all(repository.join(".claude")).expect("make the declaration directory");
    std::fs::write(repository.join(".github/workflows/ci.yml"), workflow).expect("write it");
    if let Some(text) = declaration {
        std::fs::write(repository.join(undeclared_requirement::DECLARATION), text)
            .expect("write the declaration");
    }
    for arguments in [vec!["init", "-q"], vec!["add", "-A"]] {
        let out = Command::new("git")
            .args(&arguments)
            .current_dir(&repository)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {arguments:?}: {out:?}");
    }
    repository
}

fn gate(repository: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_undeclared-requirement"))
        .arg("--repo")
        .arg(repository)
        .output()
        .expect("run the gate")
}

fn code(output: &std::process::Output) -> i32 {
    output.status.code().expect("the gate exited on a signal")
}

fn said(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The shape this repository's own workflow has: a checkout, the install, a
/// build.
const INSTALLS_PROTOC: &str = r#"
jobs:
  build:
    steps:
      - uses: actions/checkout@v7
      - run: sudo apt-get update && sudo apt-get install -y protobuf-compiler
      - run: cargo test --workspace --locked
"#;

#[test]
fn a_repository_that_names_what_its_ci_installs_is_clean() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository(
        root.path(),
        INSTALLS_PROTOC,
        Some("needs = [\"cargo\", \"protoc\"]\npackages = [\"protobuf-compiler\"]\n"),
    );
    let out = gate(&repository);
    assert_eq!(code(&out), 0, "{}", said(&out));
    assert!(
        said(&out).contains("1 install step(s) name 1 package(s)"),
        "the population it judged is printed, so a green answer over zero of \
         them is not mistakable for this one: {}",
        said(&out)
    );
}

#[test]
fn the_declaration_this_repository_had_on_the_twelfth_is_rejected_against_the_workflow_it_had() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository(
        root.path(),
        INSTALLS_PROTOC,
        Some("needs = [\"cargo\"]\npackages = [\"libclang-common-18-dev\"]\n"),
    );
    let out = gate(&repository);
    assert_eq!(code(&out), 1, "{}", said(&out));
    assert!(said(&out).contains("protobuf-compiler"), "{}", said(&out));
    assert!(
        said(&out).contains("libclang-common-18-dev"),
        "and the declared name no job installs is named as NOT a defect, or the \
         report would read as if the two were the same kind of thing: {}",
        said(&out)
    );
}

#[test]
fn a_repository_that_declares_nothing_at_all_is_the_hiding_case_at_its_widest() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository(root.path(), INSTALLS_PROTOC, None);
    let out = gate(&repository);
    assert_eq!(
        code(&out),
        1,
        "an absent declaration is not an empty one — this repository's CI \
         installs something and nothing here says a far side needs it: {}",
        said(&out)
    );
    assert!(said(&out).contains("does not exist"), "{}", said(&out));
}

#[test]
fn an_action_that_installs_is_a_refusal_rather_than_a_pass() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository(
        root.path(),
        r#"
jobs:
  build:
    steps:
      - uses: arduino/setup-protoc@v3
      - run: sudo apt-get install -y protobuf-compiler
"#,
        Some("packages = [\"protobuf-compiler\"]\n"),
    );
    let out = gate(&repository);
    assert_eq!(
        code(&out),
        2,
        "everything else about this repository is clean, and the one step whose \
         installs cannot be read is the whole verdict: {}",
        said(&out)
    );
    assert!(
        said(&out).contains("arduino/setup-protoc"),
        "{}",
        said(&out)
    );
}

#[test]
fn a_workflow_that_installs_nothing_readable_gives_no_verdict() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository(
        root.path(),
        r#"
jobs:
  build:
    steps:
      - run: sudo apt-get update
      - run: rustup toolchain install 1.88
"#,
        Some("packages = [\"protobuf-compiler\"]\n"),
    );
    let out = gate(&repository);
    assert_eq!(
        code(&out),
        2,
        "the declaration was held against nothing, and a green answer there is \
         indistinguishable from a clean check: {}",
        said(&out)
    );
    assert!(
        said(&out).contains("1 step(s) DO install"),
        "and the recognised-but-unread step is counted in the refusal: {}",
        said(&out)
    );
}

#[test]
fn a_workflow_that_installs_nothing_is_a_reading_rather_than_a_refusal() {
    // THE OTHER ZERO. A population of none is two different answers, and the
    // case above is the one that is an absence: there, something IS installed
    // and this law cannot say what. Here the witness looked and the runner
    // needed nothing added, which is a verdict — and it is the shape the hooks'
    // own fixture repository has, so calling it a refusal would reject every
    // commit in every case of `git_hooks_smoke`.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository(
        root.path(),
        r#"
jobs:
  build:
    steps:
      - uses: actions/checkout@v7
      - run: cargo test --workspace --locked
"#,
        None,
    );
    let out = gate(&repository);
    assert_eq!(code(&out), 0, "{}", said(&out));
    assert!(
        said(&out).contains("nothing a stock machine lacks"),
        "and it says which of the two zeroes it is: {}",
        said(&out)
    );
}

#[test]
fn this_repository_is_judged_and_clean() {
    // THE LIVE SEAM. The fixtures above pin the reading against strings; this
    // one is what fails the day the real files stop having the shape they are
    // pinned in — a workflow that installs through an action, a manager this
    // does not read, or a package nobody declared.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repository root is two above this manifest")
        .to_path_buf();
    let out = gate(&root);
    assert_eq!(code(&out), 0, "{}", said(&out));
    // AND THE POPULATION IT JUDGED IS NOT EMPTY, which is the other half of a
    // live seam: exit 0 is what a clean repository and an unread one both look
    // like, and the case above shows a reading of none is a legitimate verdict
    // with a phrase of its own. So this asks for the verdict that is NOT that.
    //
    // IT USED TO NAME `protobuf-compiler`, and R1252 is why it no longer does.
    // The package the law was built for is the one six jobs installed for a
    // `protoc` the schema compiler no longer needs, and when those steps went
    // this assertion failed — correctly, and in the safe direction. Replacing
    // it with the next package's name would rebuild the same trap one round
    // later; what the seam is actually for is that the witness looked at
    // something, and this repository's own two-zeroes vocabulary already says
    // which of the two zeroes a report is.
    assert!(
        !said(&out).contains("nothing a stock machine lacks"),
        "the live seam must judge a population rather than an absence: {}",
        said(&out)
    );
}
