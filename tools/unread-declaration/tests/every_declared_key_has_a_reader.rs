//! The law's arms, each killed by a different minimal signature.
//!
//! # Why the cases stand outside this checkout
//!
//! A declaration is a file at a fixed path inside a repository root, so the only
//! way to vary one is to be somewhere else. Every case below builds a throwaway
//! root and points the gate at it.
//!
//! # Why most cases answer with a stub rather than with the installed program
//!
//! The program is machine-wide and lives outside every checkout — that asymmetry
//! is the whole reason this gate exists, and it means CI has no copy of it. A
//! case that used the installed one would be a permanent refusal on a runner,
//! and a refusal that passes is the shape R1188 found printing itself as a check
//! in the gate beside the program. So the arms of the law are exercised against
//! a program that answers from a fixture, and the INSTALLED one is held to a
//! separate case that asserts an answer either way: clean where it exists,
//! refusal where it does not. That case is what keeps the fixture's shape from
//! drifting away from the real seam, and it is asserted rather than skipped.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A repository root that declares `text`, or nothing when `text` is `None`.
///
/// IT IS A REAL REPOSITORY, because the value half of the law asks the VCS what
/// this tree tracks and a directory that is not one has no answer to give. A
/// fixture without `git init` would make every case here a refusal, which is why
/// one case deliberately has none — and asserts the refusal rather than reading
/// it as a clean check.
fn repository_declaring(root: &Path, text: Option<&str>) -> PathBuf {
    tracked_repository_declaring(root, text, &[])
}

/// The same, plus files to CREATE AND TRACK — the population the value half is
/// judged against.
fn tracked_repository_declaring(
    root: &Path,
    text: Option<&str>,
    files: &[(&str, &str)],
) -> PathBuf {
    let repository = root.join("repository");
    std::fs::create_dir_all(repository.join(".claude")).expect("make the throwaway root");
    if let Some(text) = text {
        std::fs::write(repository.join(unread_declaration::DECLARATION), text)
            .expect("write the declaration");
    }
    git(&repository, &["init", "-q", "."]);
    git(&repository, &["config", "user.email", "gate@test"]);
    git(&repository, &["config", "user.name", "gate test"]);
    for (path, body) in files {
        let file = repository.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("the fixture file's directory");
        }
        std::fs::write(&file, body).expect("the fixture file");
        git(&repository, &["add", path]);
    }
    repository
}

/// A root that is NOT a repository — for the one case about what this gate can
/// and cannot be asked.
fn unversioned_declaring(root: &Path, text: &str) -> PathBuf {
    let repository = root.join("unversioned");
    std::fs::create_dir_all(repository.join(".claude")).expect("make the throwaway root");
    std::fs::write(repository.join(unread_declaration::DECLARATION), text)
        .expect("write the declaration");
    repository
}

fn git(repository: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(repository)
        .output()
        .expect("git, which the value half of the law asks what this tree tracks");
    assert!(
        out.status.success(),
        "git {args:?} failed in the fixture: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A manifest the harness reads as a sweep — the smallest one that does.
fn a_sweep() -> String {
    concat!(
        "{\"repo\": \".\", \"test_command\": [\"cargo\", \"test\"], ",
        "\"logs\": \"target/logs\", \"injections\": [{\"name\": \"n\", ",
        "\"why\": \"w\", \"edits\": [{\"file\": \"f\", \"from\": \"a\", \"to\": \"b\"}], ",
        "\"expect_red\": [\"t\"]}]}"
    )
    .to_owned()
}

/// The seam's wire form, built here so a case states exactly what the program
/// claims to have read.
fn answer(declaration: &Path, present: bool, extracts: &[(&str, &str)]) -> String {
    let mut lines = vec![format!(
        "decl-file\t{}\t{}",
        declaration.display(),
        if present { "present" } else { "absent" }
    )];
    for (key, value) in extracts {
        lines.push(format!("decl\t{key}\t{value}"));
    }
    lines.join("\n")
}

/// The answer one case wants back from the program, written where the built
/// answering program will read it.
///
/// ⚠ THE PROGRAM IS BUILT AND THE ANSWER IS DATA, and the first draft had it the
/// other way round: it wrote a shell script per case and executed it. Under
/// `check-side-workspaces.sh` on the build machine that failed with `Text file
/// busy` (R1192) — a file this process holds open for writing cannot be
/// executed, and with eleven cases in flight another thread's `fork` inherits
/// that descriptor across exactly the window in which this thread runs the file.
/// The repair is structural rather than a retry, which would have treated an
/// ownership problem as a scheduling one: cargo builds the program before any
/// case starts, and what varies per case is a file nobody executes.
fn answer_file(root: &Path, name: &str, answer: &str) -> PathBuf {
    let path = root.join(format!("{name}.answer"));
    std::fs::write(&path, answer).expect("write the answer");
    path
}

/// The program every case points the gate at, except the ones about the
/// installed one.
fn answering_program() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_answering-program"))
}

/// Run the gate.
///
/// `HOME` is named on every spawn — set when the case is about the installed
/// program, removed otherwise — because a variable a spawned program reads and
/// its test does not name is one the machine underneath answers.
/// `UNREAD_DECLARATION_ANSWER` is named for the same reason one level down: the
/// gate hands its environment to the program it runs.
fn gate(
    repository: &Path,
    program: Option<&Path>,
    home: Option<&Path>,
    answer: Option<&Path>,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_unread-declaration"));
    command.arg("--repo").arg(repository);
    if let Some(program) = program {
        command.arg("--program").arg(program);
    }
    match home {
        Some(home) => command.env("HOME", home),
        None => command.env_remove("HOME"),
    };
    match answer {
        Some(answer) => command.env("UNREAD_DECLARATION_ANSWER", answer),
        None => command.env_remove("UNREAD_DECLARATION_ANSWER"),
    };
    command.output().expect("run the gate")
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

#[test]
fn a_declaration_every_key_of_which_is_read_is_clean() {
    // THE CONTROL. Without it the cases below prove only that this gate can say
    // "defect", which every broken gate also does.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(
        root.path(),
        Some("send = \"tracked\"\nneeds = [\"cargo\", \"git\"]\npeak_gb_per_task = 2\n"),
    );
    let says = answer_file(
        root.path(),
        "reads-everything",
        &answer(
            &repository.join(unread_declaration::DECLARATION),
            true,
            &[
                ("send", "tracked"),
                ("needs", "cargo git"),
                ("peak_gb_per_task", "2"),
                ("min_free_gb", ""),
            ],
        ),
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 0, "{}", said(&output));
    assert!(
        said(&output).contains("every top-level key this repository declares is one the program"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_key_the_program_does_not_extract_is_a_finding() {
    // The measured shape: `exclude` is documented in the same block of the same
    // skill as `packages` was, four repositories carry it, and the program has no
    // extractor for it. It reads as a safety measure and imposes nothing.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(
        root.path(),
        Some("send = \"tracked\"\nexclude = [\"/target\"]\n"),
    );
    let says = answer_file(
        root.path(),
        "never-heard-of-exclude",
        &answer(
            &repository.join(unread_declaration::DECLARATION),
            true,
            &[("send", "tracked")],
        ),
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 1, "{}", said(&output));
    assert!(
        said(&output).contains("`exclude` is declared and the program never reads it"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_value_the_two_readings_disagree_about_is_a_finding() {
    // The costliest shape, because the value is neither absent nor what was
    // written: the program's integer pattern stops at the decimal point and the
    // run uses a number nobody wrote.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(root.path(), Some("peak_gb_per_task = 2.5\n"));
    let says = answer_file(
        root.path(),
        "truncates-at-the-point",
        &answer(
            &repository.join(unread_declaration::DECLARATION),
            true,
            &[("peak_gb_per_task", "2")],
        ),
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 1, "{}", said(&output));
    assert!(
        said(&output).contains("declared as `2.5` and the program reads `2`"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_key_inside_a_table_is_named_and_not_judged() {
    // The hole is printed with its size and its names. A key silently skipped is
    // a hole that reads as a clean check.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(
        root.path(),
        Some(
            "send = \"tracked\"\n\n[commands]\nbuild = \"cargo build\"\nverify = \"cargo test\"\n",
        ),
    );
    let says = answer_file(
        root.path(),
        "top-level-only",
        &answer(
            &repository.join(unread_declaration::DECLARATION),
            true,
            &[("send", "tracked")],
        ),
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 0, "{}", said(&output));
    assert!(
        said(&output).contains("2 key(s) inside a table are outside the program's namespace"),
        "{}",
        said(&output)
    );
    assert!(
        said(&output).contains("commands.build") && said(&output).contains("commands.verify"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_declaration_that_is_not_the_language_it_claims_is_a_finding() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(root.path(), Some("send =\nneeds = [\"cargo\"]\n"));
    let says = answer_file(
        root.path(),
        "patterns-read-it-anyway",
        &answer(
            &repository.join(unread_declaration::DECLARATION),
            true,
            &[("send", ""), ("needs", "cargo")],
        ),
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 1, "{}", said(&output));
    assert!(
        said(&output).contains("not valid TOML"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_program_that_cannot_be_run_is_a_refusal_not_a_pass() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(root.path(), Some("send = \"tracked\"\n"));
    let output = gate(
        &repository,
        Some(&root.path().join("not-installed")),
        None,
        None,
    );
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(said(&output).contains("NO VERDICT"), "{}", said(&output));
}

#[test]
fn a_program_without_the_seam_is_a_refusal() {
    // An older copy of the program: it runs, exits 0, and cannot say what it
    // read. Reading that as "no findings" is how a gate reports on a question it
    // never asked.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(root.path(), Some("send = \"tracked\"\n"));
    let says = answer_file(root.path(), "older-copy", "");
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(
        said(&output).contains("does not have `--explain-declaration`"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_program_answering_about_another_file_is_a_refusal() {
    // The program finds the declaration from where it was started, so an answer
    // about a different file would compare one repository's keys against
    // another's values and agree or disagree for a reason neither file explains.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(root.path(), Some("send = \"tracked\"\n"));
    let says = answer_file(
        root.path(),
        "stood-somewhere-else",
        &answer(
            &root.path().join("elsewhere/.claude/remote-build.toml"),
            true,
            &[("send", "tracked")],
        ),
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(
        said(&output).contains("it answered about"),
        "{}",
        said(&output)
    );
}

#[test]
fn a_repository_declaring_nothing_says_so_rather_than_passing_quietly() {
    // The hook that runs this gate also runs over trees that declare nothing —
    // its own smoke test builds one. The law having no population is a complete
    // answer, and it is said in words.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = repository_declaring(root.path(), None);
    let says = answer_file(root.path(), "unasked", "");
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 0, "{}", said(&output));
    assert!(
        said(&output).contains("which is not the same as a clean check"),
        "{}",
        said(&output)
    );
}

#[test]
fn the_installed_program_reads_every_key_this_repository_declares() {
    // THE END-TO-END, and the only case that touches the real seam. It asserts
    // an answer in BOTH worlds rather than stepping aside in one: where the
    // program is installed this repository's own declaration must come back
    // clean, and where it is not the gate must refuse. Neither branch can be
    // reached by a gate that quietly does nothing.
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("this crate sits two directories under the repository root")
        .to_path_buf();
    let home = std::env::var("HOME").expect("HOME is set for the test process");
    let installed = PathBuf::from(&home).join(unread_declaration::PROGRAM_UNDER_HOME);

    let output = gate(&repository, None, Some(Path::new(&home)), None);
    if installed.is_file() {
        assert_eq!(code(&output), 0, "{}", said(&output));
        assert!(
            said(&output).contains("every top-level key this repository declares"),
            "{}",
            said(&output)
        );
    } else {
        assert_eq!(code(&output), 2, "{}", said(&output));
        assert!(said(&output).contains("NO VERDICT"), "{}", said(&output));
    }
}

#[test]
fn the_installed_program_answers_in_the_shape_this_gate_parses() {
    // The fixture above states the wire form; this is what keeps that statement
    // honest where the real program lives. Where it does not, the assertion is
    // that asking is impossible — which is the same fact, said from the other
    // side.
    let home = std::env::var("HOME").expect("HOME is set for the test process");
    let installed = PathBuf::from(&home).join(unread_declaration::PROGRAM_UNDER_HOME);
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("this crate sits two directories under the repository root")
        .to_path_buf();

    match unread_declaration::ask(&installed, &repository) {
        Ok(report) => {
            assert!(
                report.present,
                "it reports its own repository's file absent"
            );
            assert!(
                report.extracts.contains_key("peak_gb_per_task"),
                "the installed program named {:?}",
                report.extracts.keys().collect::<Vec<_>>()
            );
        }
        Err(message) => {
            assert!(
                !installed.is_file(),
                "the program is installed at {} and could not be asked: {message}",
                installed.display()
            );
        }
    }
}

/// What every case about the value half declares — the program reads `writes`,
/// so the KEY half is clean and what is left is the value.
fn reads_writes(root: &Path, name: &str, repository: &Path, value: &str) -> PathBuf {
    answer_file(
        root,
        name,
        &answer(
            &repository.join(unread_declaration::DECLARATION),
            true,
            &[("writes", value)],
        ),
    )
}

#[test]
fn a_writes_token_that_names_nothing_here_is_a_finding() {
    // THE `exclude` SHAPE, one key over and one half down: the program reads the
    // key exactly as declared, and looks for a substring no command of this
    // repository can contain. A requirement that imposes nothing, and the key
    // half is clean while it does.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = tracked_repository_declaring(
        root.path(),
        Some("writes = [\"scripts/fmt.sh\", \"scripts/nope.sh\"]\n"),
        &[("scripts/fmt.sh", "#!/bin/sh\n")],
    );
    let says = reads_writes(
        root.path(),
        "reads-writes",
        &repository,
        "scripts/fmt.sh scripts/nope.sh",
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 1, "{}", said(&output));
    let words = said(&output);
    assert!(
        words.contains("`writes` declares `scripts/nope.sh` and no file this repository tracks"),
        "{words}"
    );
    assert!(
        !words.contains("declares `scripts/fmt.sh` and no file"),
        "the token that DOES name something must not be reported, or the finding \
         says nothing about which token is wrong:\n{words}"
    );
}

#[test]
fn a_token_that_reaches_only_some_of_the_sweeps_is_a_finding() {
    // THE MEASURED DEFECT, and the reason existence is not the law. `.sweep.json`
    // named four of the twenty-three manifests this repository tracks: the token
    // matched something, every key was read as declared, and nineteen sweeps ran
    // on a build machine with their tracked evidence left behind. A gate that
    // asked only "does this token match a path" called that clean.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = tracked_repository_declaring(
        root.path(),
        Some("writes = [\".sweep.json\"]\n"),
        &[
            ("sweeps/alpha.sweep.json", &a_sweep()),
            ("tools/gate/injection-sweep.json", &a_sweep()),
        ],
    );
    let says = reads_writes(
        root.path(),
        "reads-the-narrow-one",
        &repository,
        ".sweep.json",
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 1, "{}", said(&output));
    let words = said(&output);
    assert!(
        words.contains(
            "`tools/gate/injection-sweep.json` is a sweep this repository tracks and no \
             `writes` token names it"
        ),
        "{words}"
    );
    assert!(
        !words.contains("`sweeps/alpha.sweep.json` is a sweep"),
        "the manifest the token DOES name must not be reported, or the finding \
         cannot be acted on:\n{words}"
    );
}

#[test]
fn the_reach_of_every_token_is_counted_and_printed() {
    // WHAT MAKES A PARTIAL REACH VISIBLE AT ALL. Four of twenty-three and
    // twenty-three of twenty-three are both "matches something"; the number is
    // the only thing that tells them apart, so a clean run prints it too. A count
    // published only when a law fails is a count nobody can watch drift.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = tracked_repository_declaring(
        root.path(),
        Some("writes = [\"sweep.json\"]\n"),
        &[
            ("sweeps/alpha.sweep.json", &a_sweep()),
            ("tools/gate/injection-sweep.json", &a_sweep()),
        ],
    );
    let says = reads_writes(root.path(), "reads-the-wide-one", &repository, "sweep.json");
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(code(&output), 0, "{}", said(&output));
    let words = said(&output);
    assert!(
        words.contains("1 `writes` token(s) against 2 tracked sweep(s):"),
        "{words}"
    );
    assert!(
        words.contains("`sweep.json` names 2 tracked path(s), 2 of them sweep(s)"),
        "the count is the finding a round missed, so a clean run has to print \
         it:\n{words}"
    );
}

#[test]
fn a_tree_with_no_vcs_is_no_verdict_rather_than_a_clean_value_half() {
    // THE REFUSAL, ASSERTED. The population comes from the VCS, so a tree without
    // one cannot be judged — and this gate's whole third exit code exists because
    // a check that never ran reports zero findings, which is what a clean tree
    // looks like. The key half being answerable does not make the value half
    // answered.
    let root = tempfile::tempdir().expect("a temporary directory");
    let repository = unversioned_declaring(root.path(), "writes = [\"scripts/fmt.sh\"]\n");
    let says = reads_writes(
        root.path(),
        "reads-writes-unversioned",
        &repository,
        "scripts/fmt.sh",
    );
    let output = gate(&repository, Some(&answering_program()), None, Some(&says));
    assert_eq!(
        code(&output),
        2,
        "a tree this gate cannot ask about must not exit 0:\n{}",
        said(&output)
    );
    assert!(
        said(&output).contains("NO VERDICT"),
        "and it must say so in the words the other gates use:\n{}",
        said(&output)
    );
}
