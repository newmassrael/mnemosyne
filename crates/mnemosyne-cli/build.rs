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

    // Rebuild when HEAD moves (new commit) or the index changes (staged edits
    // flip the dirty state). `HEAD` covers branch switches; `index` covers
    // `git add` / commit movements.
    //
    // Round 823 — these must be ABSOLUTE. A `cargo:rerun-if-changed` path is
    // resolved relative to the PACKAGE root, and `.git` lives at the workspace
    // root, so the bare `.git/HEAD` this emitted named
    // `crates/mnemosyne-cli/.git/HEAD` — a file that does not exist. Cargo
    // treats a missing rerun path as CHANGED, so the script reran on every
    // build and, because it emits `rustc-env`, the crate recompiled every time:
    // 46s of release rebuild with nothing changed (measured, warm). The stamp
    // that tells a consumer WHICH tool they are holding was making the tool
    // expensive to keep fresh, which is why the hooks preferred a possibly
    // stale binary over rebuilding.
    //
    // `--absolute-git-dir` is also correct inside a worktree (it resolves to
    // `…/.git/worktrees/<name>`, whose own HEAD and index are the right files).
    // When git is unavailable there is nothing to watch and no stamp to keep
    // current, so nothing is emitted and cargo falls back to rerunning when a
    // file in this package changes.
    //
    // WHAT THIS DOES NOT WATCH, stated because the always-rerun defect was
    // hiding it: an edit that is never staged moves neither HEAD nor the index,
    // so a binary built after it keeps the previous stamp and can fail to say
    // `-dirty`. That matters only for a LOCALLY built binary, and a local build
    // is no longer something a consumer can pick up — `scripts/mn` keeps it in
    // `target/`, and the pinned path (`cargo install --git --rev`) builds from
    // cargo's own pristine checkout of that revision, where the tree cannot be
    // dirty. If a locally built binary ever has to be trusted AS a pin, this
    // stamp needs an input that sees the worktree, not just git's metadata.
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
