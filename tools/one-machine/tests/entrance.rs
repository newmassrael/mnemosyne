//! The program, run as a process, with the two things it talks to stubbed.
//!
//! ASKED OF THE PROCESS AND NOT ONLY OF THE LIBRARY. R1127 measured what the
//! other way leaves open in this repository three times over: a refusal path
//! reported as a clean pass, with the whole suite green, because nothing ran the
//! binary. The codes below are the contract every caller has with this gate, and
//! they live in `main`.
//!
//! THE PLACEMENT PROGRAM AND THE TRANSPORT ARE STUBBED, which is the same seam
//! `unread-declaration` uses on the same placement program and the hook suite
//! uses on `gh`. The states that matter most here — a fleet with no host free, a
//! machine that will not answer, a copy that came back different — cannot be
//! reached by waiting for a real one to be in them.
//!
//! ⚠ AND THE STUB IS A BUILT PROGRAM WHOSE ANSWERS ARE DATA, never a script a
//! case writes and runs: `tools/written-executable` refuses that shape, after
//! R1192 measured it as `ETXTBSY` under `check-side-workspaces.sh` — green
//! alone, red beside ten other crates.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use one_machine::{fingerprint, this_host, Claim, Header, CLAIM};

fn git(at: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(at)
        .status()
        .unwrap_or_else(|error| panic!("git {arguments:?} could not be run — {error}"));
    assert!(status.success(), "git {arguments:?} failed in {at:?}");
}

/// A repository that declares how it is verified — the one input `--send` reads
/// out of the tree rather than out of itself.
fn a_repository() -> tempfile::TempDir {
    let tree = tempfile::tempdir().expect("a directory to stand in");
    let at = tree.path();
    git(at, &["init", "--quiet"]);
    git(at, &["config", "user.email", "case@example.invalid"]);
    git(at, &["config", "user.name", "a case"]);
    std::fs::create_dir_all(at.join(".claude")).expect("a declaration directory");
    std::fs::write(
        at.join(".claude/remote-build.toml"),
        "[commands]\nverify = \"scripts/verify.sh -- cargo test --workspace --locked\"\n",
    )
    .expect("a declaration");
    std::fs::write(at.join("tracked"), "one\n").expect("a tracked file");
    git(at, &["add", "-A"]);
    git(at, &["commit", "--quiet", "-m", "one"]);
    std::fs::create_dir_all(at.join("answers")).expect("a directory for the stub's answers");
    tree
}

/// What the stub answers one kind of call with. `which` is `choice`, `send` or
/// `fetch`; see the program's own header for why the call decides.
fn answers(at: &Path, which: &str, exit: u8, body: &str) {
    std::fs::write(
        at.join("answers").join(which),
        format!("exit={exit}\n{body}"),
    )
    .expect("an answer");
}

fn calls(at: &Path) -> String {
    std::fs::read_to_string(at.join("answers/calls")).unwrap_or_default()
}

fn stub() -> &'static str {
    env!("CARGO_BIN_EXE_answering-program")
}

fn run(repository: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_one-machine"))
        .arg("--repo")
        .arg(repository)
        .args(arguments)
        // NAMED ON EVERY SPAWN, because a variable a spawned program reads and
        // its case does not name is one the machine underneath answers.
        .env("ONE_MACHINE_ANSWERS", repository.join("answers"))
        // AND `HOME` IS THE OTHER ONE — `tools/named-environment` named it here
        // before this line existed. It is pointed at the case's own throwaway
        // tree rather than removed, and that is the stronger of the two: every
        // case below hands this program a stub placement program explicitly, so
        // a case that ever stopped doing so would fall through to
        // `~/.claude/remote-build/bin/bx` and reach THE REAL FLEET from a test.
        // Under a `HOME` with no such program it fails loudly instead.
        .env("HOME", repository)
        .output()
        .expect("the program under test")
}

/// `--send`, with the placement program stubbed.
fn send(repository: &Path) -> Output {
    run(repository, &["--send", "--bx", stub()])
}

/// `--read`, with the transport stubbed.
fn read(repository: &Path) -> Output {
    run(repository, &["--read", "--ssh", stub()])
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("an exit status, not a signal")
}

fn said(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A log the way the far side writes one.
fn log(host: &str, fingerprint: &str, suite: i64, census: i64) -> String {
    format!(
        "{}\none-machine suite exit={suite}\none-machine census exit={census}\n\
         REMOTE_BUILD_EXIT=0\n",
        Header {
            host: host.to_owned(),
            fingerprint: fingerprint.to_owned(),
            started: 1_000,
        }
        .render()
    )
}

fn write_claim(at: &Path, fingerprint: &str) {
    let claim = Claim {
        host: "elsewhere".to_owned(),
        log: "$HOME/.remote-build/one-machine/case.log".to_owned(),
        fingerprint: fingerprint.to_owned(),
        launched: one_machine::now(),
    };
    let path = at.join(CLAIM);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("a claim directory");
    std::fs::write(path, claim.render()).expect("a claim");
}

// ── what this repository itself declares ───────────────────────────────────

/// The census has to be a census OF something, and that something is the
/// declaration's. If this row ever leaves `.claude/remote-build.toml`, the lane
/// loses its command — and it would lose it silently, because a gate with no
/// command to run finds nothing.
#[test]
fn this_repository_declares_the_command_the_census_runs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("tools/<name> sits two levels under the root")
        .to_path_buf();
    let command = one_machine::declared_verify_command(&root)
        .expect("this repository declares how it is verified on a build machine");
    assert!(
        command.contains("cargo test"),
        "the census rides on the suite; this command runs none: {command}"
    );
    let script = root.join(one_machine::CENSUS_SCRIPT);
    assert!(
        script.is_file(),
        "the launcher names {} and a dispatch that launches nothing writes no \
         header, which reads as a machine that never answered",
        one_machine::CENSUS_SCRIPT
    );
    // AND IT IS EXECUTABLE, which is not pedantry: measured in the round that
    // wrote it, the first real dispatch landed a script without the bit and the
    // far side answered `Permission denied`. The lane SAID so — a log with no
    // header is not a census, so the verdict was `not judged` rather than clean
    // — but a whole round's second opinion was gone and only that sentence
    // stood between it and a silent one.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&script)
            .expect("the census script")
            .permissions()
            .mode();
        assert!(
            mode & 0o111 != 0,
            "{} is not executable ({mode:o})",
            script.display()
        );
    }
}

// ── --send ─────────────────────────────────────────────────────────────────

#[test]
fn send_refuses_a_placement_on_this_machine() {
    let tree = a_repository();
    let at = tree.path();
    // A HOST IS NAMED AND IT STILL REFUSES. The word that decides is `where=`,
    // and a gate reading the presence of a host instead would pass this — which
    // is the shape a placement program answers with when it has an alias in hand
    // and has chosen not to use it.
    answers(
        at,
        "choice",
        0,
        "where=local host=this-one budget=- fleet=-\nwhy=no host answered\n",
    );
    let output = send(at);
    assert_eq!(code(&output), 2, "{}", said(&output));
    let words = said(&output);
    assert!(
        words.contains("HERE") && words.contains("no host answered"),
        "the refusal quotes the program that decided: {words}"
    );
    assert!(
        !at.join(CLAIM).exists(),
        "nothing was dispatched, so nothing may later be read as an answer"
    );
}

#[test]
fn send_refuses_a_placement_program_without_the_seam() {
    let tree = a_repository();
    let at = tree.path();
    answers(at, "choice", 0, "");
    let output = send(at);
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(
        said(&output).contains("--explain-choice"),
        "{}",
        said(&output)
    );
}

#[test]
fn send_refuses_a_repository_that_declares_no_command() {
    let tree = a_repository();
    let at = tree.path();
    std::fs::write(at.join(".claude/remote-build.toml"), "send = \"tracked\"\n")
        .expect("a declaration with no commands");
    answers(
        at,
        "choice",
        0,
        "where=remote host=elsewhere budget=8 fleet=x\n",
    );
    let output = send(at);
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(
        said(&output).contains("second definition"),
        "inventing the command would be inventing what verification means: {}",
        said(&output)
    );
    assert!(calls(at).is_empty(), "and nothing was sent anywhere");
}

#[test]
fn send_dispatches_once_and_records_where() {
    let tree = a_repository();
    let at = tree.path();
    answers(
        at,
        "choice",
        0,
        "where=remote host=elsewhere budget=8 fleet=x\nwhy=this host is busy\n",
    );
    let output = send(at);
    assert_eq!(code(&output), 0, "{}", said(&output));

    let claim = Claim::read(&std::fs::read_to_string(at.join(CLAIM)).expect("a claim"))
        .expect("a claim this program wrote");
    assert_eq!(claim.host, "elsewhere");
    assert_eq!(
        claim.fingerprint,
        fingerprint(at).expect("a repository"),
        "the claim names the bytes that were sent"
    );

    // AND THE LAUNCHER IS THE ONE THE PROTOCOL ASKS FOR. Each of these is a trap
    // this machine's remote-build rules were written after stepping in: a run
    // that dies with the connection that started it, a sentinel the script
    // claims for itself, and a wrapped command that eats the transport's own
    // script off the stdin it inherited.
    let sent = calls(at);
    assert!(sent.contains("setsid nohup"), "{sent}");
    assert!(sent.contains("< /dev/null"), "{sent}");
    assert!(sent.contains("REMOTE_BUILD_EXIT=$?"), "{sent}");
    assert!(sent.contains(one_machine::CENSUS_SCRIPT), "{sent}");
    assert!(sent.contains("--host\nelsewhere"), "{sent}");

    // A SECOND CALL ABOUT THE SAME BYTES SENDS NOTHING. This runs from a hook,
    // so it runs on every commit; a dispatch per invocation would rsync this
    // tree to a shared build machine over and over and leave the previous census
    // answering about bytes nobody is asking about.
    let before = calls(at);
    let again = send(at);
    assert_eq!(code(&again), 0, "{}", said(&again));
    assert!(said(&again).contains("already asked"), "{}", said(&again));
    assert_eq!(calls(at), before, "it asked the program again");
}

#[test]
fn send_dispatches_again_when_the_bytes_have_changed() {
    let tree = a_repository();
    let at = tree.path();
    answers(
        at,
        "choice",
        0,
        "where=remote host=elsewhere budget=8 fleet=x\n",
    );
    assert_eq!(code(&send(at)), 0);
    let first = calls(at);
    std::fs::write(at.join("tracked"), "two\n").expect("an edit");
    assert_eq!(code(&send(at)), 0);
    assert_ne!(calls(at), first, "different bytes are a different question");
}

#[test]
fn send_records_nothing_when_the_dispatch_itself_fails() {
    let tree = a_repository();
    let at = tree.path();
    answers(
        at,
        "choice",
        0,
        "where=remote host=elsewhere budget=8 fleet=x\n",
    );
    answers(at, "send", 1, "bx: a live lock is held by another run\n");
    let output = send(at);
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(
        !at.join(CLAIM).exists(),
        "a claim beside a launch that failed is the worst state this gate has — \
         it sends the reader looking for an answer nobody is writing"
    );
}

// ── --read ─────────────────────────────────────────────────────────────────

#[test]
fn read_without_a_dispatch_is_not_judged() {
    let tree = a_repository();
    let output = read(tree.path());
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(
        said(&output).contains("no machine has been asked"),
        "{}",
        said(&output)
    );
}

#[test]
fn read_answers_zero_for_a_clean_census_taken_elsewhere() {
    let tree = a_repository();
    let at = tree.path();
    let here = fingerprint(at).expect("a repository");
    answers(at, "fetch", 0, &log("a-build-machine", &here, 0, 0));
    write_claim(at, &here);
    let output = read(at);
    assert_eq!(code(&output), 0, "{}", said(&output));
    assert!(
        said(&output).contains("a-build-machine"),
        "{}",
        said(&output)
    );
}

#[test]
fn read_answers_one_when_that_machine_found_something() {
    let tree = a_repository();
    let at = tree.path();
    let here = fingerprint(at).expect("a repository");
    answers(at, "fetch", 0, &log("a-build-machine", &here, 0, 1));
    write_claim(at, &here);
    let output = read(at);
    assert_eq!(code(&output), 1, "{}", said(&output));
    assert!(
        said(&output).contains("one-machine census exit=1"),
        "the other machine's own words are printed, not a summary of them: {}",
        said(&output)
    );
    // AND WHERE THE REST OF IT IS, by the command that reads it. Only the lines
    // that decide are printed — a census log is thousands — so what is left out
    // has to be reachable rather than merely absent.
    let words = said(&output);
    assert!(
        words.contains("the whole log is on `elsewhere`") && words.contains("cat --"),
        "{words}"
    );
}

/// A hook cannot print a census log, and the finding must survive being in one.
#[test]
fn a_finding_inside_a_log_the_size_of_a_real_one_is_not_buried() {
    let tree = a_repository();
    let at = tree.path();
    let here = fingerprint(at).expect("a repository");
    let mut text = log("a-build-machine", &here, 0, 1);
    for number in 0..2_000 {
        text.push_str(&format!("test case_number_{number} ... ok\n"));
    }
    answers(at, "fetch", 0, &text);
    write_claim(at, &here);
    let output = read(at);
    assert_eq!(code(&output), 1, "{}", said(&output));
    let words = said(&output);
    assert!(
        words.lines().count() < 20,
        "the reader was handed {} lines: a finding printed under two thousand \
         lines of a passing suite is a finding nobody sees",
        words.lines().count()
    );
    assert!(words.contains("one-machine census exit=1"), "{words}");
}

#[test]
fn read_refuses_a_census_taken_on_this_very_machine() {
    let tree = a_repository();
    let at = tree.path();
    let here = fingerprint(at).expect("a repository");
    let mine = this_host().expect("this machine has a name");
    answers(at, "fetch", 0, &log(&mine, &here, 0, 0));
    write_claim(at, &here);
    let output = read(at);
    assert_eq!(
        code(&output),
        2,
        "clean in every other respect, and taken here: {}",
        said(&output)
    );
    assert!(said(&output).contains("THIS machine"), "{}", said(&output));
}

#[test]
fn read_refuses_a_census_about_bytes_that_have_changed() {
    let tree = a_repository();
    let at = tree.path();
    let here = fingerprint(at).expect("a repository");
    answers(at, "fetch", 0, &log("a-build-machine", &here, 0, 0));
    write_claim(at, &here);
    std::fs::write(at.join("tracked"), "changed after the dispatch\n").expect("an edit");
    let output = read(at);
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(said(&output).contains("another tree"), "{}", said(&output));
}

#[test]
fn read_refuses_a_machine_that_will_not_answer() {
    let tree = a_repository();
    let at = tree.path();
    let here = fingerprint(at).expect("a repository");
    answers(at, "fetch", 255, "ssh: connect to host: No route to host\n");
    write_claim(at, &here);
    let output = read(at);
    assert_eq!(code(&output), 2, "{}", said(&output));
    assert!(
        said(&output).contains("No route to host"),
        "the transport's own words: {}",
        said(&output)
    );
}

/// And the fetch asks the machine the claim names, for the path the claim names,
/// with `$HOME` left for the far side to expand — a path built from a local
/// `HOME` names a directory on the wrong machine.
#[test]
fn read_asks_the_machine_the_claim_names() {
    let tree = a_repository();
    let at = tree.path();
    let here = fingerprint(at).expect("a repository");
    answers(at, "fetch", 0, &log("a-build-machine", &here, 0, 0));
    write_claim(at, &here);
    assert_eq!(code(&read(at)), 0);
    let asked = calls(at);
    assert!(asked.contains("\nelsewhere\n"), "{asked}");
    assert!(
        asked.contains("$HOME/.remote-build/one-machine/case.log"),
        "{asked}"
    );
}

// ── what the far side is told to write ─────────────────────────────────────

/// The machine taking the census writes its header with THIS program, so the
/// side writing and the side reading share one spelling of what a header is.
#[test]
fn the_header_it_prints_is_the_header_it_reads() {
    let tree = a_repository();
    let at = tree.path();
    let output = run(at, &["--header"]);
    assert_eq!(code(&output), 0, "{}", said(&output));
    let answer = one_machine::read_log(&String::from_utf8_lossy(&output.stdout));
    let header = answer.header.expect("a header this program just printed");
    assert_eq!(header.host, this_host().expect("this machine has a name"));
    assert_eq!(header.fingerprint, fingerprint(at).expect("a repository"));
}

#[test]
fn declared_verify_prints_the_declarations_command() {
    let tree = a_repository();
    let output = run(tree.path(), &["--declared-verify"]);
    assert_eq!(code(&output), 0, "{}", said(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "scripts/verify.sh -- cargo test --workspace --locked"
    );
}

#[test]
fn a_verb_it_does_not_know_is_a_refusal_rather_than_a_default() {
    let tree = a_repository();
    let output = run(tree.path(), &["--sned"]);
    assert_eq!(code(&output), 2, "{}", said(&output));
    let output = run(tree.path(), &[]);
    assert_eq!(
        code(&output),
        2,
        "no verb is not a clean check either: {}",
        said(&output)
    );
}
