//! Round 286 — embed `git describe` output into the binary as the
//! `BUILD_GIT_HASH` env so `mnemosyne-mcp --version` can identify
//! which round/commit produced this binary. Mirrors the
//! `mnemosyne-cli` build.rs (same format, same fallbacks).

fn main() {
    let hash = std::process::Command::new("git")
        .args(["describe", "--always", "--dirty=-dirty", "--abbrev=8"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_GIT_HASH={}", hash);
    // Round 823 — ABSOLUTE, because a rerun path resolves against the PACKAGE
    // root and `.git` is at the workspace root. See the mnemosyne-cli build
    // script for the full account; the bare paths named files that do not
    // exist, which cargo reads as "changed", so this reran on every build.
    if let Some(git_dir) = std::process::Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
        println!("cargo:rerun-if-changed={git_dir}/index");
    }
}
