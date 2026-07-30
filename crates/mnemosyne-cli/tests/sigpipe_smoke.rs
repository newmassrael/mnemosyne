//! Round 859 — a report piped into a reader that stops early must not panic.
//!
//! Rust sets `SIGPIPE` to `SIG_IGN` before `main`, so a write to a closed pipe
//! returns `EPIPE` and `println!` unwraps it: the process dies at exit code 101
//! with `failed printing to stdout: Broken pipe (os error 32)` on stderr. Every
//! consumer pipes a long report into `head` or a pager, so every report verb
//! carried that. Found in Round 857 while consuming `report-quest-graph` as a
//! projection runtime would — and misread, at first, as a defect in the report.
//!
//! The fix restores the default `SIGPIPE` disposition in `main`, so the process
//! dies by signal 13 like `cat` or `grep` does. This test pins the OUTCOME
//! (no panic, death by signal) rather than the mechanism.

use std::io::{BufRead, BufReader};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Enough sections that the output cannot fit in a pipe buffer (64 KiB on
/// Linux). That is what makes the test deterministic: with the whole report
/// smaller than the buffer, the child can finish every write before the reader
/// closes, and the run passes without ever reaching the defect. The size is
/// ASSERTED below rather than assumed.
const SECTIONS: usize = 9000;

fn workspace() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();
    std::fs::create_dir_all(root.join("docs/.atomic")).expect("atomic dir");
    std::fs::write(
        root.join("mnemosyne.toml"),
        "[workspace]\n\n[atomic]\nsidecar_path = \"docs/.atomic/store.json\"\n",
    )
    .expect("config");
    let mut json = String::from("{\"schema_version\":");
    json.push_str(&mnemosyne_atomic::CURRENT_SCHEMA_VERSION.to_string());
    json.push_str(",\"sections\":{");
    for i in 0..SECTIONS {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "\"sc-{i:05}\":{{\"title\":\"Section number {i}\"}}"
        ));
    }
    json.push_str("}}");
    std::fs::write(root.join("docs/.atomic/store.json"), json).expect("store");
    tmp
}

fn cli(root: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"));
    c.args(["query", "--list-sections"]).current_dir(root);
    c
}

#[test]
fn a_report_whose_reader_stops_early_dies_by_signal_and_never_panics() {
    let tmp = workspace();
    let root = tmp.path();

    // NON-VACUITY, and it is the whole reason this fixture is large: the report
    // must exceed the pipe buffer, or the child never blocks on a write and the
    // assertions below would hold against an unfixed binary too.
    let whole = cli(root).output().expect("full run");
    assert!(whole.status.success(), "the full run must succeed");
    assert!(
        whole.stdout.len() > 65_536,
        "the fixture report is {} bytes, which fits in a pipe buffer — this test \
         cannot reach a closed-pipe write until it is larger",
        whole.stdout.len()
    );

    // Read ONE line — proof the child has started writing — then drop the pipe,
    // which closes the read end while the child is still blocked mid-report.
    let mut child = cli(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    {
        let out = child.stdout.take().expect("piped stdout");
        let mut reader = BufReader::new(out);
        let mut line = String::new();
        reader.read_line(&mut line).expect("first line");
        assert!(
            line.starts_with("sc-"),
            "expected a section line, got {line:?}"
        );
    } // reader dropped here => read end closed
    let done = child.wait_with_output().expect("wait");
    let stderr = String::from_utf8_lossy(&done.stderr);

    assert!(
        !stderr.contains("panicked"),
        "a reader that stopped early is not a defect in the report: {stderr}"
    );
    assert_eq!(
        done.status.code(),
        None,
        "the process must die by signal, not exit with a code (stderr: {stderr})"
    );
    assert_eq!(
        done.status.signal(),
        Some(13),
        "death must be SIGPIPE, the disposition `cat` and `grep` run with"
    );
}

/// Round 859 — the fix narrows the PIPE case and nothing else: a genuine write
/// failure must still be loud.
///
/// The control that separates "a reader stopped, which is not our problem" from
/// "make write errors silent". `/dev/full` accepts an open and fails every write
/// with `ENOSPC`, which is a real failure a caller must hear about — so the
/// panic and the 101 stay, and only the closed-pipe case above changed.
///
/// Linux-scoped at COMPILE time rather than skipped at run time: `/dev/full` is
/// a Linux device, and a runtime skip would read as a pass on a platform where
/// the control never ran.
#[cfg(target_os = "linux")]
#[test]
fn a_real_write_failure_is_still_loud() {
    let tmp = workspace();
    let full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full is a Linux device and this control needs it");
    let done = cli(tmp.path())
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .expect("run against /dev/full");
    assert_eq!(
        done.status.code(),
        Some(101),
        "ENOSPC on stdout is a real failure, not a reader that walked away"
    );
    assert!(
        String::from_utf8_lossy(&done.stderr).contains("failed printing to stdout"),
        "and it must say so: {}",
        String::from_utf8_lossy(&done.stderr)
    );
}
