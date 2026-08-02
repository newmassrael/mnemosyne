//! Round 691 (DEBT-DOUBLE-STDERR regression) — the atomic-mutate error prints
//! exactly once, and the `--json` error output stays pure json.
//!
//! R684 fixed the double-print structurally but shipped no test — the
//! cost-no-object review flagged that the CLI crate already has an
//! stderr-asserting subprocess harness. This pins the fix at that layer: a
//! revert to the double-print reddens CI here. The suppression signal is now a
//! typed `CliError` variant `main` matches, not a marker recovered by
//! `downcast_ref`; the behavior this test asserts is invariant across that
//! rework, which is exactly why the test guards it.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn run(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("cli exec")
}

/// A minimal workspace: only `mnemosyne.toml` (no store file — a missing
/// sidecar loads as empty, so the mutate reaches the atomic-mutate error path
/// on the absent section rather than a config/load error).
fn workspace() -> TempDir {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("mnemosyne.toml"), "[workspace]\n").unwrap();
    tmp
}

const FAILING_MUTATE: &[&str] = &[
    "set-section-intent",
    "--section",
    "no-such-section",
    "--intent",
    "x",
];

#[test]
fn atomic_mutate_error_prints_exactly_once() {
    let tmp = workspace();
    let out = run(tmp.path(), FAILING_MUTATE);
    assert!(
        !out.status.success(),
        "the mutate must fail (exit non-zero)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let error_lines = stderr.lines().filter(|l| l.starts_with("error:")).count();
    assert_eq!(
        error_lines, 1,
        "the atomic-mutate error must print exactly once, got {error_lines}:\n{stderr}"
    );
}

#[test]
fn atomic_mutate_json_error_is_pure_json() {
    let tmp = workspace();
    let mut args = FAILING_MUTATE.to_vec();
    args.push("--json");
    let out = run(tmp.path(), &args);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.lines().any(|l| l.starts_with("error:")),
        "the --json error output must not carry a trailing non-json `error:` line:\n{stderr}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(stderr.trim()).is_ok(),
        "the --json error output must be valid json:\n{stderr}"
    );
}

/// A workspace whose first mutate will create `docs/.atomic/workspace.atomic.json`.
fn store_workspace() -> TempDir {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("docs")).unwrap();
    fs::write(tmp.path().join("mnemosyne.toml"), "[workspace]\n").unwrap();
    tmp
}

/// Append one entry whose decision comes from `prose`, and give back the
/// `decision_summary` the store actually holds.
fn append_with_decision_file(dir: &Path, prose: &str) -> (std::process::Output, Option<String>) {
    fs::write(dir.join("decision.txt"), prose).unwrap();
    fs::write(dir.join("changes.txt"), "- x\n").unwrap();
    fs::write(dir.join("verify.txt"), "- v\n").unwrap();
    let out = run(
        dir,
        &[
            "append-changelog-entry",
            "--entry-id",
            "Round 999",
            "--decision-file",
            "decision.txt",
            "--changes-file",
            "changes.txt",
            "--verification-file",
            "verify.txt",
        ],
    );
    let sidecar = dir.join("docs/.atomic/workspace.atomic.json");
    let stored = fs::read_to_string(&sidecar).ok().and_then(|t| {
        serde_json::from_str::<serde_json::Value>(&t)
            .ok()?
            .get("changelog_entries")?
            .get("Round 999")?
            .get("decision_summary")?
            .as_str()
            .map(str::to_string)
    });
    (out, stored)
}

/// A CHANGELOG ENTRY'S PROSE HAS NO INLINE SHELL DOOR (Round 971).
///
/// A backtick inside a double-quoted shell argument is command substitution.
/// What reaches the primitive is the substitution's OUTPUT, so the loss happens
/// BEFORE any invariant this program could enforce — which is why the defence
/// has to be the ABSENCE of the door rather than a check behind it.
///
/// This is not a hypothetical. The same failure is recorded three times:
/// Round 819 caught it at readback and discarded the append; Round 907 caught
/// it at readback, reverted the store to the previous commit and re-appended
/// through a heredoc; Round 969 did NOT catch it, and the ledger is append-only
/// so two backticked terms are missing from that entry's summary permanently.
/// Round 819 even wrote the rule — "an id belongs in a file passed to the
/// primitive, never inline in a shell argument" — and the field had no file to
/// be passed in for another hundred and fifty rounds. The three bullet fields
/// were never reachable this way; `decision_summary` was the only one that was.
///
/// The assertion is over EVERY prose field, not only the one that was fixed:
/// the reason `decision_summary` was the casualty is that it was the only one
/// with an inline form, and a future field growing one would be the same defect
/// under a new name.
#[test]
fn no_prose_field_of_a_changelog_entry_takes_an_inline_shell_argument() {
    let tmp = store_workspace();
    for inline in ["--decision", "--changes", "--verification", "--carry"] {
        let out = run(
            tmp.path(),
            &[
                "append-changelog-entry",
                "--entry-id",
                "Round 999",
                inline,
                "text",
            ],
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success() && stderr.contains("unknown flag"),
            "`append-changelog-entry {inline}` is accepted, so a prose field can \
             be handed to the primitive as a shell argument again. That is the \
             door the shell came through in Round 819, Round 907 and Round 969, \
             and the third one is frozen in the ledger. stderr: {stderr}"
        );
    }
}

/// The file route carries the prose the inline route could not.
///
/// The discriminating content is a BACKTICKED term: that is precisely what the
/// shell ate, so prose that survives with its backticks is the property whose
/// absence caused the damage. Multi-line and interior blank lines are asserted
/// too — a bullets-style reader would silently reshape this field, and it is
/// prose, not bullets.
#[test]
fn a_decision_file_carries_backticked_prose_through_unchanged() {
    let tmp = store_workspace();
    let prose = "R999 — the field keys on `edge_costs` and the rule said \
                 `undirected: true`.\n\nA second paragraph, kept.\n";
    let (out, stored) = append_with_decision_file(tmp.path(), prose);
    assert!(
        out.status.success(),
        "append failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stored = stored.expect("the entry is in the store with a decision_summary");
    assert_eq!(
        stored,
        prose.trim_end(),
        "the file route reshaped the prose it was given; only trailing \
         whitespace may be trimmed, because this field is prose and the terms \
         inside it are the load-bearing part"
    );
    assert!(
        stored.contains("`edge_costs`") && stored.contains("`undirected: true`"),
        "the backticked terms did not survive, which is the exact loss Round \
         969 recorded: {stored}"
    );
}

/// Silent emptiness is the SHAPE the failure takes, so it rejects.
///
/// Command substitution leaves the value empty, not malformed, and an empty
/// prose field is indistinguishable from a deliberate blank once stored. This
/// is the one thing the primitive can still refuse.
#[test]
fn an_all_whitespace_decision_file_rejects_rather_than_storing_silence() {
    let tmp = store_workspace();
    let (out, stored) = append_with_decision_file(tmp.path(), "   \n\n  \n");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("empty after trim"),
        "an all-whitespace decision file was accepted; silent emptiness is what \
         the shell substitution produced in Round 969. stderr: {stderr}"
    );
    assert!(
        stored.is_none(),
        "the rejected append still wrote a decision_summary: {stored:?}"
    );
}
