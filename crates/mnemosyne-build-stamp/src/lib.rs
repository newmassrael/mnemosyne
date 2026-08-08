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
/// ```no_run
/// mnemosyne_build_stamp::emit();
/// ```
///
/// `no_run` rather than `ignore`: an `ignore`d example is not compiled by
/// anything, so it is a code sample that can name a function this crate no
/// longer has and nothing goes red. R1084's gate found it as a test that
/// exists and that no CI command runs — which is what an `ignore`d example is.
/// `no_run` compiles it, and not running it is right: `emit` talks to git and
/// writes cargo directives, which is a build script's job and not a doc-test's.
/// The `fn main` wrapper this carried while nothing compiled it went with the
/// same change: clippy's `needless_doctest_main` rejected it the moment the
/// example became one, which is the first thing anything had ever said about it.
///
/// The value is [`revision`] — a short commit hash, `-dirty` when the tracked
/// tree differs from it, `unknown` when git cannot say. A `-dirty` build
/// corresponds to NO revision, which is exactly what a consumer pinning one
/// needs to be told.
///
/// # What invalidates it
///
/// The four files below, each addressed through `git rev-parse --git-path`
/// rather than joined onto a directory. Two reasons, both learned the hard way:
/// a `cargo:rerun-if-changed` path resolves against the PACKAGE root, so a bare
/// `.git/HEAD` from a crate under `crates/` names a file that does not exist and
/// cargo reads a missing path as changed (Round 823, measured at 46.5s of
/// rebuild per build); and a linked worktree keeps its own HEAD and index while
/// sharing refs with the main git dir, which one joined directory cannot express.
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
    println!("cargo:rustc-env=BUILD_GIT_HASH={}", revision());

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

/// This build's revision (see [`revision_in`]): a short commit hash, `-dirty` when the tracked tree
/// differs from it, `unknown` when git cannot say.
///
/// A REVISION, NOT A DESCRIPTION (Round 827). This was
/// `git describe --always --dirty --abbrev=8`, which returns the nearest TAG
/// when one is reachable and only falls back to a hash when none is — so the
/// day this repository is tagged, every stamp becomes something like
/// `v1.0-3-gabc12345`, and a consumer comparing it against a pinned revision
/// stops matching while holding exactly the right build. Verified by tagging
/// this repo and reading the output back: the stamp became the bare tag name.
///
/// It also made the value and its watch set disagree. `describe` depends on
/// tags, which nothing here watches; `rev-parse` depends on HEAD and the refs
/// and index that ARE watched, so the stamp is now a function of exactly the
/// inputs cargo has been told to invalidate it on.
pub fn revision() -> String {
    revision_in(std::path::Path::new("."))
}

/// [`revision`] for a specific directory — the seam that makes the rule testable
/// against a scratch repository instead of only against whichever one happens to
/// be the current directory.
#[must_use]
pub fn revision_in(dir: &std::path::Path) -> String {
    let Some(hash) = run_git_in(dir, &["rev-parse", "--short=8", "HEAD"]) else {
        // No git, or a repository with no commit yet: unknowable, which the pin
        // check treats as distinct from known-and-wrong.
        return "unknown".to_string();
    };
    // Tracked changes against HEAD, staged or not — the same notion of dirty
    // `git describe --dirty` used. Untracked files are deliberately NOT dirty:
    // they are not part of the build's provenance and would make every stray
    // scratch file rewrite the stamp.
    let clean = std::process::Command::new("git")
        .current_dir(dir)
        .args(["diff", "--quiet", "HEAD"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if clean {
        hash
    } else {
        format!("{hash}-dirty")
    }
}

/// One `git` invocation, trimmed, `None` on any failure — a missing git, a
/// non-zero exit, non-UTF-8 output and empty output are the same answer here:
/// this build cannot know, and must not guess.
fn run_git(args: &[&str]) -> Option<String> {
    run_git_in(std::path::Path::new("."), args)
}

fn run_git_in(dir: &std::path::Path, args: &[&str]) -> Option<String> {
    std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::revision_in;

    fn git(dir: &std::path::Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    /// Round 827 — a TAG must not become the stamp.
    ///
    /// This is the exact regression that shipped in Round 826 and was invisible
    /// because this repository has no tags: `git describe --always` returns the
    /// nearest reachable tag and only falls back to a hash when there is none,
    /// so the day anyone tags a release every stamp turns into a tag name and
    /// every pinned consumer is refused while holding the right build. The test
    /// therefore MAKES a tag, which is the only way to see it.
    #[test]
    fn the_stamp_is_a_revision_even_when_a_tag_is_reachable() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let dir = tmp.path();
        git(dir, &["init", "--quiet"]);
        git(dir, &["config", "user.email", "t@example.com"]);
        git(dir, &["config", "user.name", "t"]);
        std::fs::write(dir.join("f"), "a").expect("write");
        git(dir, &["add", "."]);
        git(dir, &["commit", "--quiet", "-m", "one"]);
        git(dir, &["tag", "-a", "v9.9.9", "-m", "release"]);

        let stamp = revision_in(dir);
        let hash = stamp.strip_suffix("-dirty").unwrap_or(&stamp);
        assert!(
            hash.len() >= 7 && hash.chars().all(|c| c.is_ascii_hexdigit()),
            "the stamp must be a revision, not `{stamp}` — a tag name satisfies \
             no pin and breaks every consumer that compares one"
        );
        assert!(
            !stamp.ends_with("-dirty"),
            "a fresh commit is clean: {stamp}"
        );

        // ...and an uncommitted change to a TRACKED file makes it dirty, which is
        // what keeps a local build from ever satisfying a pin.
        std::fs::write(dir.join("f"), "b").expect("write");
        assert!(
            revision_in(dir).ends_with("-dirty"),
            "a modified tracked file must mark the build dirty"
        );
    }
}
