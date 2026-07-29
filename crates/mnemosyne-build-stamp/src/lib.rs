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

    // WHAT MOVES THE ANSWER (Round 826 — Round 823 watched two of these four
    // and the two it picked do not include the commonest event of all):
    //
    //   HEAD        a checkout or a detach rewrites it — but NOT a commit,
    //               because on a branch HEAD is a symref whose bytes are the
    //               branch NAME and do not change when the branch moves;
    //   the branch  a commit rewrites THIS. Watching only HEAD meant a commit
    //               left the binary stamped with the previous revision, and
    //               with no `-dirty` to betray it on a clean tree — a build
    //               claiming to be a revision it is not, which is the exact lie
    //               the stamp exists to prevent;
    //   packed-refs where that ref lives when it is packed and has no loose file;
    //   index       staging flips the `-dirty` suffix, and a commit rewrites it.
    //
    // Every path comes from `git rev-parse --git-path`, which resolves each one
    // for the CURRENT worktree — a linked worktree keeps its own HEAD and index
    // while sharing refs with the main git dir, so joining them onto one
    // directory would be wrong for exactly the setup that most needs them right.
    for path in ["HEAD", "index", "packed-refs"] {
        watch(path);
    }
    // Empty on a detached HEAD, where there is no branch to move.
    if let Some(branch_ref) = run_git(&["symbolic-ref", "--quiet", "HEAD"]) {
        watch(&branch_ref);
    }
}

/// Register `git_relative_path` as an input, if git can place it and it exists.
///
/// The existence check is not defensive noise: cargo treats a MISSING
/// `rerun-if-changed` path as changed, so naming a file that is not there turns
/// the stamp into an unconditional rebuild of every crate that carries it —
/// which is precisely the defect Round 823 measured at 46.5s per build. A repo
/// with packed refs has no loose ref file, and one with no commits yet has no
/// `packed-refs`; both are ordinary, and neither should cost anything.
fn watch(git_relative_path: &str) {
    if let Some(path) = run_git(&["rev-parse", "--git-path", git_relative_path]) {
        if std::path::Path::new(&path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
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
