//! Round 405 — `validate-content-drift` EPUB-file provenance check.
//!
//! When `[workspace.spec_source].epub_path` + `epub_sha256` are pinned, the
//! verb re-hashes the committed EPUB file offline and compares to the pin.
//! Scope:
//! (i) committed EPUB matching the pinned hash → clean (exit 0)
//! (ii) a swapped EPUB → drift; default severity `reject` exits 1
//! (iii) `--severity warn` prints the drift but exits 0
//! (iv) a missing committed EPUB → drift

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

const EPUB_BYTES: &[u8] = b"EPUB-CONTENT-v1";
// sha256("EPUB-CONTENT-v1")
const EPUB_HASH: &str = "177f9d8588e4530b850d8bd77044f631bffeb4c0efe283040872334251c9ce41";

/// Writes a workspace whose spec_source pins an EPUB at `docs/.atomic/epub/spec.epub`.
/// `epub_bytes` is the committed file content (None = do not write the file).
fn write_workspace(ws: &Path, epub_bytes: Option<&[u8]>) {
    fs::create_dir_all(ws.join("docs/.atomic/epub")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        format!(
            "[workspace]\n\
             [workspace.spec_source]\n\
             url = \"https://www.w3.org/TR/scxml/\"\n\
             revision = \"REC-scxml-20150901\"\n\
             epub_path = \"docs/.atomic/epub/spec.epub\"\n\
             epub_sha256 = \"{EPUB_HASH}\"\n\
             [schema]\n\
             entry_id_prefix = \"Round \"\n"
        ),
    )
    .unwrap();
    let store = serde_json::json!({
        "schema_version": 8, "sections": {}, "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
    if let Some(bytes) = epub_bytes {
        fs::write(ws.join("docs/.atomic/epub/spec.epub"), bytes).unwrap();
    }
}

fn run(ws: &Path, extra: &[&str]) -> std::process::Output {
    let mut a = vec!["validate-content-drift"];
    a.extend_from_slice(extra);
    Command::new(cli())
        .args(a)
        .current_dir(ws)
        .output()
        .expect("cli exec")
}

#[test]
fn epub_matching_pinned_hash_is_clean() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), Some(EPUB_BYTES));
    let out = run(tmp.path(), &[]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("epub_file: clean"),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn swapped_epub_drifts_and_reject_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), Some(b"EPUB-CONTENT-v2-swapped"));
    let out = run(tmp.path(), &[]);
    assert!(
        !out.status.success(),
        "swapped EPUB under default reject must exit 1; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("EPUB-file drift"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn swapped_epub_warn_opts_out() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), Some(b"EPUB-CONTENT-v2-swapped"));
    let out = run(tmp.path(), &["--severity", "warn", "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["epub_file"]["checked"], true);
    assert_eq!(v["epub_file"]["status"], "drift");
}

#[test]
fn missing_committed_epub_is_drift() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), None); // pinned but file absent
    let out = run(tmp.path(), &[]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("missing at epub_path"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
