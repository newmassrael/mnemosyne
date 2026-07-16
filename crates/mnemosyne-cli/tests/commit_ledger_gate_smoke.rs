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

/// Round 656 — the hint must assert the REMEDY, not merely appear (the R627
/// discipline: a test that pins a message LABEL locks the defect in).
///
/// `missing` has two classes needing OPPOSITE fixes, and the gate cannot tell
/// them apart: it reads commit subjects and resolves them against THIS
/// workspace's ledger, so an upstream's round number is `missing` forever. The
/// first playable consumer hit exactly that (their subject cited `R643`, ours)
/// and reported that following the backfill-only hint would have written
/// `Round 643` into THEIR ledger — a decision they never made.
///
/// This pins both remedies. Reverting to the backfill-only wording FAILS here.
#[test]
fn gate_hint_names_the_upstream_remedy_not_only_backfill() {
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

    // The backfill flow stays named — it is the right remedy for OUR rounds.
    assert!(
        combined.contains("append-changelog-entry"),
        "the backfill remedy must still be named; got: {}",
        combined
    );
    // ...but it must be CONDITIONAL, not the only door offered.
    assert!(
        combined.contains("THIS workspace's round"),
        "the backfill hint must be scoped to this workspace's own rounds; got: {}",
        combined
    );
    // The upstream class must be named, and named as NOT-yours-to-backfill.
    assert!(
        combined.contains("ANOTHER project's round") && combined.contains("NOT yours to"),
        "the hint must tell an upstream-citing consumer NOT to backfill; got: {}",
        combined
    );
    // ...and it must point at the hatch R377 actually built, plus the
    // subject-vs-body fact that makes the cheap fix discoverable.
    assert!(
        combined.contains("severity = warn"),
        "the hint must name the R377 severity hatch; got: {}",
        combined
    );
    assert!(
        combined.contains("SUBJECTS only"),
        "the hint must say the gate reads subjects, so the body is the escape; got: {}",
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
