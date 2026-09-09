//! Round 1316 — a round's verification reaches the frozen ledger as DATA, and
//! no wire into it accepts a verdict.
//!
//! The harm this closes was measured on this repository's own ledger on
//! 2026-09-03: of 1059 entries, 380 claim a run in their verification bullets —
//! a `verify.sh`, a `check-side-workspaces`, an `exit 0`, an `rc=0` — and 0
//! record one as data. The instance that makes it more than a formality is
//! Round 1313, whose last verification bullet claims a green root suite and a
//! green side gate for "the tree this commit carries", where at the moment that
//! sentence was written the root run had been started before the file under
//! test existed and the side gate had exited 1. The ledger is append-only, so
//! the false sentence is still there; a later session repaired the world to
//! match it, which is not a gate catching anything.
//!
//! WHAT THESE TESTS ARE FOR, and why they run the real binary: a field whose
//! value a caller can type is a claim in a new place, which is the defect
//! wearing the fix. So the load-bearing property is not "the field exists" but
//! "the verdict came out of the wrapper's record and could not have come from
//! anywhere else", and that is only observable at the wire.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn run(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

fn read_store(ws: &Path) -> serde_json::Value {
    serde_json::from_str(
        &fs::read_to_string(ws.join("docs/.atomic/workspace.atomic.json")).unwrap(),
    )
    .unwrap()
}

fn git(ws: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(ws)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .expect("git exec");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn commit_all(ws: &Path, message: &str) {
    git(ws, &["add", "-A"]);
    git(ws, &["commit", "--quiet", "--no-verify", "-m", message]);
}

/// A record shaped exactly as the verification wrapper writes one: the `# cmd:`
/// header, the command's own output, and the `# verify: exit=` trailer.
///
/// The two marker strings come from the reader rather than being spelled here,
/// so this fixture cannot pass while disagreeing with the thing under test. That
/// the REAL wrapper writes those same bytes is a different question, and it is
/// answered by running it:
/// `verify_wrapper_smoke::every_record_the_wrapper_writes_states_the_verdict_it_returned`.
fn record(command: &str, exit_code: i64) -> String {
    format!(
        "# verify.sh 2026-09-09T00:00:00Z\n\
         {}{command}\n\
         # fresh=1\n\
         # cwd: /somewhere\n\
         \n\
         test result: ok. 12 passed; 0 failed\n\
         {}{exit_code}\n",
        mnemosyne_ops::VERIFY_RECORD_COMMAND,
        mnemosyne_ops::VERIFY_RECORD_VERDICT,
    )
}

fn write_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::create_dir_all(ws.join("logs")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": mnemosyne_atomic::CURRENT_SCHEMA_VERSION,
        "sections": {},
        "changelog_entries": {},
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

/// Verification prose that CLAIMS a run, in the shape the gate's predicate
/// recognises — which is the shape 380 entries of the real ledger are in.
fn write_prose(ws: &Path, claiming: bool) {
    fs::write(ws.join("decision.txt"), "a round that ran its suite\n").unwrap();
    fs::write(ws.join("changes.txt"), "- changed a thing\n").unwrap();
    fs::write(
        ws.join("verify.txt"),
        if claiming {
            "- the root suite through scripts/verify.sh: exit 0\n"
        } else {
            "- read the two call sites and the header above them\n"
        },
    )
    .unwrap();
}

fn append(ws: &Path, entry: &str, extra: &[&str]) -> std::process::Output {
    let mut args = vec![
        "append-changelog-entry",
        "--entry-id",
        entry,
        "--decision-file",
        "decision.txt",
        "--changes-file",
        "changes.txt",
        "--verification-file",
        "verify.txt",
    ];
    args.extend_from_slice(extra);
    run(ws, &args)
}

/// THE VERDICT IN THE LEDGER IS THE RECORD'S, VERBATIM.
///
/// Asserted against the record the fixture wrote rather than against literals
/// spelled in the assertion: a hand-typed expectation would pass whenever the
/// wire and the test made the same mistake, which is the whole failure mode.
#[test]
fn record_verification_files_exactly_what_the_wrappers_record_says() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);
    write_prose(ws, true);
    let command = "cargo test --workspace --locked --no-fail-fast";
    fs::write(ws.join("logs/root.log"), record(command, 0)).unwrap();

    let out = append(
        ws,
        "Round 1316",
        &["--record-verification", "logs/root.log"],
    );
    assert!(
        out.status.success(),
        "append failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let store = read_store(ws);
    let filed = store
        .pointer("/changelog_entries/Round 1316/verification_runs")
        .expect("the entry files no verification run at all");
    assert_eq!(
        *filed,
        serde_json::json!([{
            "command": command,
            "exit_code": 0,
            "log": "root.log",
        }]),
        "the ledger's run is not the record's — a reader of this entry would \
         inherit a verdict this tree never reached"
    );
}

/// A ROUND THAT RAN TWO COMMANDS FILES TWO RUNS. The checklist a round follows
/// verifies with the root suite AND the separate-workspace gate, so a flag that
/// took one would make an entry that filed half its verification look exactly
/// like an entry that had run everything.
#[test]
fn record_verification_files_every_run_a_round_hands_it() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);
    write_prose(ws, true);
    fs::write(
        ws.join("logs/root.log"),
        record("cargo test --workspace", 0),
    )
    .unwrap();
    fs::write(
        ws.join("logs/side.log"),
        record("scripts/check-side-workspaces.sh", 1),
    )
    .unwrap();

    let out = append(
        ws,
        "Round 1316",
        &[
            "--record-verification",
            "logs/root.log",
            "--record-verification",
            "logs/side.log",
        ],
    );
    assert!(
        out.status.success(),
        "append failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let store = read_store(ws);
    let filed = store
        .pointer("/changelog_entries/Round 1316/verification_runs")
        .and_then(|v| v.as_array())
        .expect("the entry files no verification run at all")
        .clone();
    assert_eq!(
        filed.len(),
        2,
        "only part of the round's verification landed"
    );
    // The RED one landed as red. A field that quietly dropped or normalised a
    // non-zero status would be a field that agrees with the round's prose by
    // construction, which is the property this whole thing exists to remove.
    assert_eq!(filed[1]["exit_code"], 1, "the red run did not land as red");
}

/// A KILLED RUN AND A GREEN ONE LOOK IDENTICAL UP TO THE TRAILER.
///
/// The wrapper seals its record on the way out, so a record with no trailer is
/// one whose wrapper never reached its own end. Reading that as anything files
/// a verdict nobody reached — which is the defect, restated in the field meant
/// to close it.
#[test]
fn a_record_whose_wrapper_never_finished_is_refused() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);
    write_prose(ws, true);
    let full = record("cargo test --workspace", 0);
    let truncated = full
        .lines()
        .filter(|l| !l.starts_with(mnemosyne_ops::VERIFY_RECORD_VERDICT))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(ws.join("logs/killed.log"), truncated).unwrap();

    let out = append(
        ws,
        "Round 1316",
        &["--record-verification", "logs/killed.log"],
    );
    let said =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "an unsealed record was filed as a verdict: {said}"
    );
    assert!(
        said.contains("never reached its own end"),
        "the refusal does not say what is wrong with the record: {said}"
    );
    assert!(
        !ws.join("docs/.atomic/workspace.atomic.json").exists()
            || read_store(ws)
                .pointer("/changelog_entries/Round 1316")
                .is_none(),
        "the append was refused and the entry landed anyway"
    );
}

/// A record naming no command is a status with nothing to attach it to.
#[test]
fn a_record_that_names_no_command_is_refused() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);
    write_prose(ws, true);
    fs::write(
        ws.join("logs/headless.log"),
        format!("{}0\n", mnemosyne_ops::VERIFY_RECORD_VERDICT),
    )
    .unwrap();

    let out = append(
        ws,
        "Round 1316",
        &["--record-verification", "logs/headless.log"],
    );
    let said =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a record naming no command was filed: {said}"
    );
}

/// The field is opt-in, and an entry that did not ask for it carries no key —
/// so "this entry files no run" stays readable as itself.
#[test]
fn an_append_that_does_not_ask_files_no_run() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);
    write_prose(ws, false);

    assert!(append(ws, "Round 1316", &[]).status.success());
    let raw = fs::read_to_string(ws.join("docs/.atomic/workspace.atomic.json")).unwrap();
    assert!(
        !raw.contains("verification_runs"),
        "an entry that filed no run still writes the key: {raw}"
    );
}

fn validated(ws: &Path) -> (bool, String) {
    let out = run(ws, &["validate-workspace"]);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// A workspace whose COMMITTED ledger has adopted the record: one entry, filed
/// with its run, already in history. That is the state in which the gate binds,
/// and every case below starts from it so that none of them turns out to be
/// testing the adoption rule by accident.
fn adopted(ws: &Path) {
    write_workspace(ws);
    write_prose(ws, true);
    fs::write(
        ws.join("logs/root.log"),
        record("cargo test --workspace", 0),
    )
    .unwrap();
    git(ws, &["init", "--quiet"]);
    commit_all(ws, "baseline");
    assert!(
        append(
            ws,
            "Round 1316",
            &["--record-verification", "logs/root.log"]
        )
        .status
        .success(),
        "the fixture's adopting round did not append"
    );
    commit_all(ws, "the round that adopted the record");
}

/// THE GATE, ON THE ARM THAT NEEDS NO COMMIT: an entry this tree is ADDING that
/// claims a run in prose and files none.
///
/// The two cases run from the SAME fixture state on purpose, and the green one
/// is what makes the red one worth anything — same tree, same prose, same gate,
/// and the only difference is whether the round filed what it claims. A red with
/// no green beside it is a gate that might be rejecting the fixture.
#[test]
fn an_uncommitted_entry_that_claims_a_run_and_files_none_is_rejected() {
    let green = TempDir::new().unwrap();
    adopted(green.path());
    assert!(append(
        green.path(),
        "Round 1317",
        &["--record-verification", "logs/root.log"]
    )
    .status
    .success());
    let (ok, said) = validated(green.path());
    assert!(ok, "an entry that filed its run was rejected: {said}");

    let red = TempDir::new().unwrap();
    adopted(red.path());
    assert!(append(red.path(), "Round 1317", &[]).status.success());
    let (ok, said) = validated(red.path());
    assert!(
        !ok,
        "an entry claiming a run it does not file was accepted: {said}"
    );
    assert!(
        said.contains("Round 1317"),
        "the rejection does not name the entry: {said}"
    );
}

/// THE ARM THAT NEEDS NO HOOK. A consumer who never installed the pre-commit
/// hook still gets every round checked, on the push that lands it — the entry
/// the TIP COMMIT added, judged against the history it was written on top of.
#[test]
fn an_entry_the_tip_commit_added_without_its_run_is_caught_after_the_commit() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    adopted(ws);

    assert!(append(ws, "Round 1317", &[]).status.success());
    commit_all(ws, "a round that claims a run and files none");

    let (ok, said) = validated(ws);
    assert!(
        !ok,
        "the entry landed in a commit and nothing said so: {said}"
    );
    assert!(
        said.contains("Round 1317"),
        "the rejection does not name the entry: {said}"
    );
}

/// The one line `validate-workspace` prints about this check, or empty.
fn record_line(ws: &Path) -> String {
    let out = run(ws, &["validate-workspace"]);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.starts_with("verification records:"))
        .unwrap_or_default()
        .to_string()
}

/// EVERY WAY OF FINDING NOTHING HAS ITS OWN NAME IN THE OUTPUT.
///
/// A verdict of "no violations" is worth what the reach behind it is worth, and
/// this check has three ways of finding nothing — two of which are not "nothing
/// is wrong". Round 979 shipped the census check reporting only violations, so
/// a workspace that had never installed a hook read exactly like one the gate
/// had just cleared; that silence is what this asserts against here, before it
/// can happen a second time.
#[test]
fn each_state_of_the_check_says_which_one_it_is() {
    let mut said = Vec::new();

    // Not adopted: nothing in the committed ledger files a run.
    let bare = TempDir::new().unwrap();
    write_workspace(bare.path());
    write_prose(bare.path(), true);
    git(bare.path(), &["init", "--quiet"]);
    commit_all(bare.path(), "baseline");
    assert!(append(bare.path(), "Round 1316", &[]).status.success());
    said.push(("not adopted", record_line(bare.path())));

    // Undecidable: no repository at all, so which entries this commit adds
    // cannot be known and nothing is judged.
    let nogit = TempDir::new().unwrap();
    write_workspace(nogit.path());
    write_prose(nogit.path(), true);
    fs::write(
        nogit.path().join("logs/root.log"),
        record("cargo test --workspace", 0),
    )
    .unwrap();
    assert!(append(
        nogit.path(),
        "Round 1316",
        &["--record-verification", "logs/root.log"]
    )
    .status
    .success());
    said.push(("no git", record_line(nogit.path())));

    // Measured: adopted, with an entry in reach.
    let live = TempDir::new().unwrap();
    adopted(live.path());
    assert!(append(
        live.path(),
        "Round 1317",
        &["--record-verification", "logs/root.log"]
    )
    .status
    .success());
    let measured = record_line(live.path());
    assert!(
        measured.contains("1 uncommitted"),
        "the measured state does not report the population it read: {measured}"
    );
    said.push(("measured", measured));

    for (what, line) in &said {
        assert!(
            !line.is_empty(),
            "`{what}`: validate-workspace says nothing about this check, so a \
             reader cannot tell a clean tree from one where it never ran"
        );
    }
    for (i, (what_a, a)) in said.iter().enumerate() {
        for (what_b, b) in said.iter().skip(i + 1) {
            assert_ne!(
                a, b,
                "`{what_a}` and `{what_b}` are different states and say the same \
                 thing, so the line does not distinguish them"
            );
        }
    }
}

/// A RULE CANNOT BIND A ROUND THAT RAN BEFORE IT EXISTED, and the ledger is
/// append-only, so a gate that reached backwards would state a violation nobody
/// is able to fix. The 380 entries this debt was measured on claim runs in prose
/// and predate the field entirely.
///
/// Two ways backwards, both closed here: the round that ADOPTS the record must
/// not be judged by the rule it is introducing, and neither must whatever the
/// tip commit happened to be carrying when it did.
#[test]
fn adoption_does_not_reach_back_to_what_was_already_written() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);
    write_prose(ws, true);
    fs::write(
        ws.join("logs/root.log"),
        record("cargo test --workspace", 0),
    )
    .unwrap();
    git(ws, &["init", "--quiet"]);
    commit_all(ws, "baseline");

    // A round from before the record existed: claims a run, files none, and is
    // now frozen in the tip commit.
    assert!(append(ws, "Round 1315", &[]).status.success());
    commit_all(ws, "a round from before the record existed");
    let (ok, said) = validated(ws);
    assert!(
        ok,
        "a ledger that has never filed a run was failed for not filing one: {said}"
    );
    assert!(
        said.contains("has not adopted the record"),
        "the stand-down is silent, so 'nothing wrong' and 'nothing checked' print \
         the same clean: {said}"
    );

    // The adopting round, in the working tree. It files its own run — and the
    // frozen entry the tip commit carries must not become a violation for it.
    assert!(append(
        ws,
        "Round 1316",
        &["--record-verification", "logs/root.log"]
    )
    .status
    .success());
    let (ok, said) = validated(ws);
    assert!(
        ok,
        "adopting the record made the previous round's frozen entry a violation, \
         which is a finding nobody can act on: {said}"
    );
    assert!(
        !said.contains("Round 1315:"),
        "the gate named a frozen entry written before the rule: {said}"
    );

    // AND THE OTHER ARM, which is the one a single-armed rule gets wrong. An
    // entry that rides along in the SAME COMMIT as the adoption was also written
    // before the record existed — HEAD~1 is what it was authored on top of, and
    // HEAD~1 had adopted nothing. A gate that asked only "does the ledger file a
    // run now" would answer yes here and turn a frozen entry into a finding.
    assert!(append(ws, "Round 1317", &[]).status.success());
    commit_all(ws, "the adoption, and a round that rode along with it");
    let (ok, said) = validated(ws);
    assert!(
        ok,
        "an entry that landed in the adopting commit itself was judged by the \
         rule that commit introduced: {said}"
    );
    assert!(
        !said.contains("Round 1317:"),
        "the gate named an entry written before its own arm had adopted the \
         record: {said}"
    );
}
