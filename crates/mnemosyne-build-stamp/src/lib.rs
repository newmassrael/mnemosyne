//! The build-time git stamp every Mnemosyne binary carries (Round 286 for the
//! stamp itself, Round 824 for this one home).
//!
//! A consumer that runs a Mnemosyne binary has to be able to say WHICH tool they
//! are holding: the store is built by this tool, and a different tool is a
//! different store. `--version` answers that from `BUILD_GIT_HASH`, which this
//! crate emits from a build script.
//!
//! IT LIVES HERE BECAUSE THE COPIES PROVED THEY DO NOT GET REVIEWED TOGETHER.
//! `mnemosyne-cli` and `mnemosyne-mcp` each carried their own transcription of
//! this from Round 286, described in a comment as "mirrors the mnemosyne-cli
//! build.rs (same format, same fallbacks)" — and both carried the SAME defect
//! for that whole span: the `cargo:rerun-if-changed` paths were package-relative
//! while `.git` is at the workspace root, so each named a file that does not
//! exist, cargo read a missing path as changed, and the stamp forced a full
//! recompile on every single build (Round 823 measured 46.5s of release rebuild
//! with nothing changed). One rule in two homes drifts; this one did not even
//! need to drift to hurt, because a copy is also a second thing to get wrong.

/// Emit `BUILD_GIT_HASH` for the calling crate and register what invalidates it.
///
/// Call it from a build script and nothing else:
///
/// ```ignore
/// fn main() {
///     mnemosyne_build_stamp::emit();
/// }
/// ```
///
/// The value is `git describe --always --dirty --abbrev=8`: a short hash for a
/// clean tree, the same with a `-dirty` suffix when the tree it was built from
/// had uncommitted changes, and `unknown` when git is unavailable (a tarball
/// build, or no `.git`). A `-dirty` build corresponds to NO revision, which is
/// exactly what a consumer pinning a revision needs to be told.
///
/// # What invalidates it
///
/// `HEAD` and `index`, addressed ABSOLUTELY. A `cargo:rerun-if-changed` path is
/// resolved against the package root, so a bare `.git/HEAD` from a crate under
/// `crates/` names `crates/<crate>/.git/HEAD` — a path that does not exist, and
/// cargo treats a missing rerun path as changed. `--absolute-git-dir` also
/// resolves correctly inside a worktree (`…/.git/worktrees/<name>`), whose own
/// HEAD and index are the right files to watch.
///
/// # What it cannot see
///
/// An edit that is never staged moves neither HEAD nor the index, so a binary
/// built after one keeps the previous stamp and can fail to say `-dirty`. That
/// is contained rather than solved: a locally built binary is not something a
/// consumer picks up (it stays in `target/`, reached through `scripts/mn`), and
/// the pinned path — `cargo install --git --rev` — builds from cargo's own
/// pristine checkout of that revision, where the tree cannot be dirty. If a
/// locally built binary ever has to be trusted AS a pin, this needs an input
/// that watches the worktree rather than git's metadata.
pub fn emit() {
    let hash = run_git(&["describe", "--always", "--dirty=-dirty", "--abbrev=8"])
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_GIT_HASH={hash}");

    // No git dir means there is nothing to watch and no stamp to keep current;
    // emitting nothing lets cargo fall back to rerunning when a file in the
    // calling package changes.
    if let Some(git_dir) = run_git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
        println!("cargo:rerun-if-changed={git_dir}/index");
    }
}

/// One `git` invocation, trimmed, `None` on any failure — a missing git, a
/// non-zero exit, non-UTF-8 output and empty output are the same answer here:
/// this build cannot know, and must not guess.
fn run_git(args: &[&str]) -> Option<String> {
    std::process::Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
