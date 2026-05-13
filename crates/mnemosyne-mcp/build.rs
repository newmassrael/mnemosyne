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
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
