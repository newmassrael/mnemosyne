//! Round 286 — embed `git describe` output into the binary as the
//! `BUILD_GIT_HASH` env so `mnemosyne-cli --version` can identify
//! which round/commit produced this binary.
//!
//! Format: `<short-hash>` for clean trees, `<short-hash>-dirty` when
//! uncommitted changes exist, `unknown` when git is unavailable
//! (tarball install / no `.git`).

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
    // Rebuild when HEAD moves (new commit) or index changes (staged edits
    // flip dirty state). `.git/HEAD` covers branch switches; `.git/index`
    // covers `git add` / commit movements.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
