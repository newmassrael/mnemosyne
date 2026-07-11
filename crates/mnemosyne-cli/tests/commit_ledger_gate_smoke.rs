//! Round 377 — commit↔ledger drift gate multi-workspace awareness.
//!
//! Two regression axes for the SCE-reported false-positive:
//! - D1 path-scope: a parenthesized round label on a commit that only
//!   touched a *sibling* workspace must not bleed into this workspace's
//!   scan (`git log -- .`).
//! - D2 severity opt-out: `[commit_ledger].severity` defaults to `reject`
//!   (the R301 dogfood hard-reject) but a consumer workspace whose
//!   `(R<n>)` labels are not Mnemosyne changelog rounds downgrades to
//!   `warn`/`info`, which surfaces the drift line without gating.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_workspace(dir: &std::path::Path, commit_ledger_severity: Option<&str>) {
    fs::create_dir_all(dir.join("docs")).unwrap();
    let mut cfg = String::from(
        r#"[workspace]
"#,
    );
    if let Some(sev) = commit_ledger_severity {
        cfg.push_str(&format!("\n[commit_ledger]\nseverity = \"{}\"\n", sev));
    }
    fs::write(dir.join("mnemosyne.toml"), cfg).unwrap();
    fs::write(dir.join("docs/STUB.md"), "# Stub\n\n## 1. Top\n\nbody.\n").unwrap();
}

/// Append one changelog entry so the workspace has a non-empty, in-sync
/// ledger + GENERATED.md (otherwise validate-workspace would still pass,
/// but this lets the test assert the ledger round set explicitly).
fn seed_ledger_entry(dir: &std::path::Path, entry_id: &str) {
    let changes = dir.join("changes.txt");
    fs::write(&changes, "x\n").unwrap();
    let verify = dir.join("verify.txt");
    fs::write(&verify, "v\n").unwrap();
    let out = Command::new(cli_binary())
        .args([
            "append-changelog-entry",
            "--entry-id",
            entry_id,
            "--decision",
            "seed",
            "--changes-file",
            changes.to_str().unwrap(),
            "--verification-file",
            verify.to_str().unwrap(),
        ])
        .current_dir(dir)
        .output()
        .expect("run append-changelog-entry");
    assert!(
        out.status.success(),
        "seed append failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git(repo: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_init(repo: &std::path::Path) {
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "test"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
}

fn validate(dir: &std::path::Path) -> std::process::Output {
    Command::new(cli_binary())
        .arg("validate-workspace")
        .current_dir(dir)
        .output()
        .expect("run validate-workspace")
}

#[test]
fn gate_rejects_missing_round_by_default() {
    // A workspace whose only ledger entry is Round 999, but whose history
    // carries a commit citing (R9999): default severity = reject => bail.
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, None);
    seed_ledger_entry(ws, "Round 999");
    git_init(ws);
    git(ws, &["add", "-A"]);
    git(ws, &["commit", "-q", "-m", "seed workspace (R9999)"]);

    let out = validate(ws);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success(),
        "default severity must gate on a missing cited round; got: {}",
        combined
    );
    assert!(
        combined.contains("commit↔ledger drift gate"),
        "expected the R301 gate diagnostic; got: {}",
        combined
    );
}

#[test]
fn gate_warn_severity_opts_out() {
    // Same history, but [commit_ledger] severity = "warn": the drift line
    // still prints, the exit code is not gated.
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, Some("warn"));
    seed_ledger_entry(ws, "Round 999");
    git_init(ws);
    git(ws, &["add", "-A"]);
    git(ws, &["commit", "-q", "-m", "seed workspace (R9999)"]);

    let out = validate(ws);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "warn severity must not gate the exit code; stdout={}, stderr={}",
        stdout,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("missing R9999") && stdout.contains("not gating this workspace"),
        "warn must still surface the drift line; got: {}",
        stdout
    );
}

#[test]
fn gate_path_scoped_no_sibling_bleed() {
    // One git repo, two workspaces. A (R9999) commit touches only ws_a.
    // ws_b (empty ledger) must NOT see ws_a's label — it passes. ws_a
    // itself DOES own the label and (empty ledger) is rejected, proving
    // the commit is genuinely round-labeled and the scope is the only
    // thing protecting ws_b.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    git_init(root);
    let ws_a = root.join("ws_a");
    let ws_b = root.join("ws_b");
    write_workspace(&ws_a, None);
    write_workspace(&ws_b, None);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "init both workspaces"]);

    // A sibling-only labeled commit: touches ws_a, not ws_b.
    fs::write(
        ws_a.join("docs/STUB.md"),
        "# Stub\n\n## 1. Top\n\nedited.\n",
    )
    .unwrap();
    git(root, &["add", "ws_a"]);
    git(root, &["commit", "-q", "-m", "feat: edit ws_a (R9999)"]);

    // ws_b: the sibling label must not bleed in => passes.
    let out_b = validate(&ws_b);
    let combined_b = format!(
        "{}{}",
        String::from_utf8_lossy(&out_b.stdout),
        String::from_utf8_lossy(&out_b.stderr)
    );
    assert!(
        out_b.status.success(),
        "ws_b must not inherit ws_a's round label; got: {}",
        combined_b
    );

    // ws_a: owns the label, empty ledger => gated (proves the label is real).
    let out_a = validate(&ws_a);
    let combined_a = format!(
        "{}{}",
        String::from_utf8_lossy(&out_a.stdout),
        String::from_utf8_lossy(&out_a.stderr)
    );
    assert!(
        !out_a.status.success() && combined_a.contains("commit↔ledger drift gate"),
        "ws_a owns (R9999) with an empty ledger and must be gated; got: {}",
        combined_a
    );
}
