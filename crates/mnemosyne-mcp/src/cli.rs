//! Subprocess wrapper around `mnemosyne-cli`.
//!
//! Every MCP tool delegates to the production CLI binary. This keeps
//! validation logic in one place (the validator crate, exercised by
//! 58 test suites) and lets the MCP layer stay thin.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Resolve the `mnemosyne-cli` binary path. Honors `MNEMOSYNE_CLI`
/// environment variable; otherwise expects `mnemosyne-cli` on PATH.
pub fn cli_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        std::env::var("MNEMOSYNE_CLI").unwrap_or_else(|_| "mnemosyne-cli".to_string())
    })
}

pub struct CliOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    pub fn ok(&self) -> bool {
        self.status == 0
    }

 /// Collapse stdout + stderr into a single human-readable string.
    pub fn combined(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n--- stderr ---\n{}", self.stdout, self.stderr)
        }
    }
}

/// Run `mnemosyne-cli <args>` with the given workspace as CWD.
pub async fn run(workspace: &Path, args: &[&str]) -> Result<CliOutput, std::io::Error> {
    let output = Command::new(cli_path())
        .args(args)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .output()
        .await?;
    Ok(CliOutput {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Run `mnemosyne-cli <args>` and pipe `stdin_payload` to its stdin.
/// Used for tools that accept `--bullets-file -` style input.
#[allow(dead_code)] // reserved for v0.2 stdin-piping mutate primitives
pub async fn run_with_stdin(
    workspace: &Path,
    args: &[&str],
    stdin_payload: &str,
) -> Result<CliOutput, std::io::Error> {
    let mut child = Command::new(cli_path())
        .args(args)
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_payload.as_bytes()).await?;
        stdin.shutdown().await?;
    }
    let output = child.wait_with_output().await?;
    Ok(CliOutput {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Write a temp file under `workspace/.mnemosyne/tmp/` containing `content`.
/// Returns the absolute path. Best-effort cleanup is the caller's
/// responsibility; on Linux the workspace cache directory is normally
/// part of `.gitignore` so transient files there are tolerable.
pub fn write_temp(workspace: &Path, prefix: &str, content: &str) -> std::io::Result<PathBuf> {
    let dir = workspace.join(".mnemosyne").join("tmp");
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = dir.join(format!("{}-{}.tmp", prefix, stamp));
    std::fs::write(&path, content)?;
    Ok(path)
}
