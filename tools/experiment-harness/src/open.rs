//! Round 1248 — the build of this repository that can READ a kit's evidence.
//!
//! A kit is a workspace validated by ONE revision. That is not a preference,
//! it is the decision R878/R880 reached and R1084 put on a clock: 31 of 35
//! tracked experiment manifests no longer import, re-typing them was refused
//! because a kit's pins are pre-committed claims about what a blind author
//! produced, and the alternative is to keep the record byte-identical and move
//! the TOOL. Which leaves the reader with a question nothing answered — WHICH
//! build — and a job nothing did: producing it.
//!
//! Measured on 2026-08-19, which is what made this a verb rather than a note.
//! Thirty of the thirty-two tracked kits declare a revision in their
//! `replay.json`; not one of the thirty-seven tracked kit `mnemosyne.toml`
//! files declares a `[tool] pin`, so standing in a kit workspace and running
//! the CLI gets today's build and a shape error. The pin cannot go in those
//! files either: each is sealed by a sha256 its kit's record holds, and a
//! pre-`[tool]` binary dies at TOML parse on the table — so declaring it would
//! break the very replay it was meant to serve. The answer is a reader that
//! knows where kits keep the revision, which is here.
//!
//! It is also where MATERIALISING a revision now lives, once. Two tests carried
//! byte-similar copies of `git archive` + `cargo build` before this — the
//! replay runner's and the pinned-revision recheck's — and a mechanism with two
//! implementations is one that can answer differently about which tool ran.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ci_plan::issue::{self, Tree};

use crate::declare::git;
use crate::util::{read_file, HResult};

/// The repository the caller is standing in, asked of git rather than derived
/// from this binary's location: the tool runs from a side workspace, from a
/// test's temporary directory and by hand, and only the caller's tree can say
/// which repository a kit lives in.
pub fn repo_root() -> HResult<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("git rev-parse: {e}"))?;
    if !out.status.success() {
        return Err("not inside a git work tree".to_string());
    }
    Ok(PathBuf::from(
        std::str::from_utf8(&out.stdout)
            .map_err(|e| format!("repo root is not utf-8: {e}"))?
            .trim(),
    ))
}

/// One replay and the revision its kit pins it to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    pub replay: String,
    pub revision: String,
}

/// What a kit's record says about the build that reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reads {
    /// Every replay names the same revision, so the kit has one answer.
    One {
        revision: String,
        replays: Vec<String>,
    },
    /// The replays name more than one revision. NOT resolved to the first:
    /// which one reads a given artifact depends on which replay built it, and
    /// a confident wrong revision arrives looking exactly like a right one.
    Several(Vec<Pinned>),
    /// The kit declares no replay, and its record says why. This is an ANSWER
    /// and not a gap — `built-outside-this-repository` means no revision of
    /// this repository ever ran that import, so there is none to pin.
    NoReplay { reason: String, prose: String },
}

/// Read one kit's record and say which build reads it.
pub fn reads(root: &Path, unit: &str) -> HResult<Reads> {
    let path = root.join(unit).join("replay.json");
    let raw = read_file(path.to_str().ok_or("record path is not utf-8")?)?;
    let doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("{unit}/replay.json is not JSON: {e}"))?;
    let replays = doc
        .get("replays")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("{unit}/replay.json declares no `replays` array"))?;

    if replays.is_empty() {
        // The reason and the prose are BOTH required here, because a kit that
        // declares no replay and does not say why is indistinguishable from one
        // whose record was never finished.
        let reason = doc
            .get("no_replay_reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!("{unit}/replay.json declares no replay and no `no_replay_reason`")
            })?;
        let prose = doc
            .get("no_replay")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{unit}/replay.json declares no replay and no `no_replay`"))?;
        return Ok(Reads::NoReplay {
            reason: reason.to_string(),
            prose: prose.to_string(),
        });
    }

    let mut pinned = Vec::new();
    for r in replays {
        let replay = r
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{unit}/replay.json: a replay declares no `name`"))?;
        let revision = r.get("revision").and_then(|v| v.as_str()).ok_or_else(|| {
            format!("{unit}/replay.json: replay `{replay}` declares no `revision`")
        })?;
        pinned.push(Pinned {
            replay: replay.to_string(),
            revision: revision.to_string(),
        });
    }

    let first = pinned[0].revision.clone();
    if pinned.iter().all(|p| p.revision == first) {
        return Ok(Reads::One {
            revision: first,
            replays: pinned.into_iter().map(|p| p.replay).collect(),
        });
    }
    Ok(Reads::Several(pinned))
}

/// The revision a caller asked for, resolved through [`reads`].
///
/// `replay` picks one when the kit names several; without it a kit that names
/// several is an error that LISTS them, because the caller has to choose and a
/// tool that chose for them would be guessing at which evidence they meant.
pub fn revision_of(root: &Path, unit: &str, replay: Option<&str>) -> HResult<String> {
    match reads(root, unit)? {
        Reads::NoReplay { reason, prose } => Err(format!(
            "{unit} declares no replay ({reason}), so no revision of this repository \
             is pinned to read it:\n  {prose}"
        )),
        Reads::One { revision, replays } => match replay {
            None => Ok(revision),
            Some(want) if replays.iter().any(|r| r == want) => Ok(revision),
            Some(want) => Err(format!(
                "{unit} declares no replay `{want}` — it declares: {}",
                replays.join(", ")
            )),
        },
        Reads::Several(pinned) => match replay {
            Some(want) => pinned
                .iter()
                .find(|p| p.replay == want)
                .map(|p| p.revision.clone())
                .ok_or_else(|| {
                    format!(
                        "{unit} declares no replay `{want}` — it declares: {}",
                        pinned
                            .iter()
                            .map(|p| p.replay.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }),
            None => Err(format!(
                "{unit}'s replays name more than one revision, so `--replay` decides \
                 which evidence you mean: {}",
                pinned
                    .iter()
                    .map(|p| format!("{} -> {}", p.replay, p.revision))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        },
    }
}

/// A revision's tree and the CLI built from it.
#[derive(Debug)]
pub struct Materialised {
    pub revision: String,
    /// The extracted source tree.
    pub tree: PathBuf,
    /// The binary that reads this revision's evidence.
    pub cli: PathBuf,
    /// WHICH cargo built it, printed rather than assumed: a machine whose PATH
    /// cargo is a different channel would otherwise have this round's verdict
    /// reported about a toolchain nobody named (Round 1190).
    pub cargo: String,
}

/// Extract `revision` and build its `mnemosyne-cli` under `into`.
///
/// `git archive` and NOT a worktree, which would register state in `.git` and
/// leak if this failed. The build goes to a target directory of its own under
/// `into`, so nothing here can be satisfied by an artifact of today's tree.
pub fn materialise(root: &Path, revision: &str, into: &Path, cargo: &str) -> HResult<Materialised> {
    let tree = into.join("tree");
    let target = into.join("target");
    std::fs::create_dir_all(&tree).map_err(|e| format!("mkdir {}: {e}", tree.display()))?;

    // Fail here rather than inside tar: a revision this clone does not have is
    // a different finding from one that no longer builds.
    let archive = Command::new("git")
        .args(["archive", revision])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git archive {revision}: {e}"))?;
    if !archive.status.success() {
        return Err(format!(
            "git archive {revision} failed — this clone does not hold that revision \
             (a shallow checkout is the usual reason):\n{}",
            String::from_utf8_lossy(&archive.stderr).trim()
        ));
    }

    let mut tar = Command::new("tar")
        .args(["-x", "-C", tree.to_str().ok_or("tree path is not utf-8")?])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("tar spawn: {e}"))?;
    tar.stdin
        .as_mut()
        .ok_or("tar has no stdin")?
        .write_all(&archive.stdout)
        .map_err(|e| format!("write archive to tar: {e}"))?;
    if !tar.wait().map_err(|e| format!("tar wait: {e}"))?.success() {
        return Err(format!("tar could not extract revision {revision}"));
    }

    // THE CARGO IS THE REPLAY'S TO NAME, and the tree is not this working tree:
    // it was extracted from a pinned revision a moment ago, so the lockfile in
    // it is THAT revision's answer. Pinning it here would make reading old
    // evidence fail on a resolution nobody is going to go back and repair, which
    // is the opposite of what a replay is for (R1262).
    let build = issue::named_cargo(
        cargo,
        Tree::MadeByThisRun(
            "a tree extracted at a pinned revision, whose lockfile is that \
             revision's answer rather than this working tree's",
        ),
    )
    .args(["build", "--bin", "mnemosyne-cli"])
    .current_dir(&tree)
    .env("CARGO_TARGET_DIR", &target)
    .output()
    .map_err(|e| format!("{cargo} build: {e}"))?;
    if !build.status.success() {
        return Err(format!(
            "revision {revision} no longer builds — THIS is the finding, and it kills \
             the pin-the-revision design for every replay that names it:\n{}",
            String::from_utf8_lossy(&build.stderr)
        ));
    }

    let cli = target.join("debug/mnemosyne-cli");
    if !cli.is_file() {
        return Err(format!(
            "{cargo} build reported success and there is no binary at {}",
            cli.display()
        ));
    }
    Ok(Materialised {
        revision: revision.to_string(),
        tree,
        cli,
        cargo: cargo.to_string(),
    })
}

/// THE RULE for what a kit unit is: the directory holding a `replay.json`.
///
/// Three places derived this from a tracked-file list before Round 1248 — twice
/// in `declare` and once here — and a fourth reading of "which directories are
/// kits" is a fourth chance for them to answer differently about a nested kit.
/// Pure, so a caller that already has the listing does not pay for a second one.
pub fn kit_units<'a, I: IntoIterator<Item = &'a String>>(tracked: I) -> Vec<String> {
    tracked
        .into_iter()
        .filter(|f| f.ends_with("/replay.json"))
        .map(|f| f.trim_end_matches("/replay.json").to_string())
        .collect()
}

/// THE OVERVIEW: one line per (kit, revision) — unit, revision, the replays that
/// name it — and one line for a kit that declares no replay, carrying its
/// recorded reason.
///
/// A kit is never omitted. What a reader asks before naming one is which kits
/// there are and what reads each, and a listing that quietly skipped the kits
/// with no revision would read as a tree where every kit has one.
pub fn list_lines(root: &Path) -> HResult<Vec<String>> {
    let mut out = Vec::new();
    for unit in tracked_kits(root)? {
        match reads(root, &unit)? {
            Reads::One { revision, replays } => {
                out.push(format!("{unit}\t{revision}\t{}", replays.join(",")))
            }
            Reads::Several(pinned) => out.extend(
                pinned
                    .into_iter()
                    .map(|p| format!("{unit}\t{}\t{}\t(--replay decides)", p.revision, p.replay)),
            ),
            Reads::NoReplay { reason, .. } => {
                out.push(format!("{unit}\t-\t-\tno replay: {reason}"))
            }
        }
    }
    Ok(out)
}

/// Every tracked kit, asked of git.
pub fn tracked_kits(root: &Path) -> HResult<Vec<String>> {
    let tracked: Vec<String> = git(root, &["ls-files", "claudedocs/phase1-*"])?
        .lines()
        .map(str::to_string)
        .collect();
    Ok(kit_units(&tracked))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit(dir: &Path, unit: &str, body: &str) {
        let d = dir.join(unit);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("replay.json"), body).unwrap();
    }

    fn tmp() -> PathBuf {
        let stamp = crate::util::sha256_hex(
            &std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_le_bytes(),
        );
        // The process id is in the name because the temp root is SHARED, and
        // this repository's own gate asks every path built from it to say who
        // owns it.
        let base = std::env::temp_dir().join(format!(
            "experiment-harness-open-{}-{}",
            std::process::id(),
            &stamp[..8]
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    /// The three answers, and each is a DIFFERENT thing to tell a reader.
    #[test]
    fn a_record_says_which_build_reads_it_or_says_why_none_does() {
        let root = tmp();
        kit(
            &root,
            "one",
            r#"{"replays":[{"name":"a","revision":"aaa"},{"name":"b","revision":"aaa"}]}"#,
        );
        kit(
            &root,
            "several",
            r#"{"replays":[{"name":"a","revision":"aaa"},{"name":"b","revision":"bbb"}]}"#,
        );
        kit(
            &root,
            "none",
            r#"{"replays":[],"no_replay_reason":"built-outside-this-repository",
                "no_replay":"an out-of-tree harness built them"}"#,
        );

        assert_eq!(
            reads(&root, "one").unwrap(),
            Reads::One {
                revision: "aaa".to_string(),
                replays: vec!["a".to_string(), "b".to_string()],
            }
        );
        assert_eq!(
            reads(&root, "several").unwrap(),
            Reads::Several(vec![
                Pinned {
                    replay: "a".to_string(),
                    revision: "aaa".to_string()
                },
                Pinned {
                    replay: "b".to_string(),
                    revision: "bbb".to_string()
                },
            ])
        );
        match reads(&root, "none").unwrap() {
            Reads::NoReplay { reason, prose } => {
                assert_eq!(reason, "built-outside-this-repository");
                assert!(prose.contains("out-of-tree"), "{prose}");
            }
            other => panic!("expected NoReplay, got {other:?}"),
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A kit naming two revisions is a QUESTION and not a default. The failure
    /// names both, because the caller cannot choose from a message that hides
    /// what there was to choose between.
    #[test]
    fn two_revisions_are_refused_until_a_replay_is_named() {
        let root = tmp();
        kit(
            &root,
            "several",
            r#"{"replays":[{"name":"a","revision":"aaa"},{"name":"b","revision":"bbb"}]}"#,
        );
        let err = revision_of(&root, "several", None).unwrap_err();
        assert!(
            err.contains("a -> aaa") && err.contains("b -> bbb"),
            "both must be named: {err}"
        );
        assert_eq!(revision_of(&root, "several", Some("b")).unwrap(), "bbb");
        let err = revision_of(&root, "several", Some("c")).unwrap_err();
        assert!(err.contains("a, b"), "the choices must be named: {err}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A kit with no replay refuses with its OWN recorded reason rather than a
    /// generic miss — the reason is the answer, not the absence of one.
    #[test]
    fn a_kit_with_no_replay_refuses_in_its_own_words() {
        let root = tmp();
        kit(
            &root,
            "none",
            r#"{"replays":[],"no_replay_reason":"nothing-a-verb-takes",
                "no_replay":"no verb of this repository takes these bytes"}"#,
        );
        let err = revision_of(&root, "none", None).unwrap_err();
        assert!(
            err.contains("nothing-a-verb-takes") && err.contains("no verb of this repository"),
            "{err}"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A record that says nothing about why it has no replay is a record that
    /// is not finished, and saying "no revision" about it would read as a
    /// decision somebody made.
    #[test]
    fn a_silent_empty_record_is_an_error_and_not_an_answer() {
        let root = tmp();
        kit(&root, "silent", r#"{"replays":[]}"#);
        let err = reads(&root, "silent").unwrap_err();
        assert!(err.contains("no_replay_reason"), "{err}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// EVERY tracked kit answers, and the population is asked of git rather
    /// than listed here.
    ///
    /// The risk this covers is a record SHAPE this reader cannot read — a kit
    /// landing with a replay that names no revision, or an empty `replays` with
    /// no reason — which would surface as "no build reads this kit" for a kit
    /// that names one. Counts are printed rather than asserted: how many kits
    /// exist is the repository's number, and typing it here would make adding a
    /// kit a red build.
    #[test]
    fn every_tracked_kit_says_which_build_reads_it_or_why_none_does() {
        let root = repo_root().expect("this test runs inside the repository");
        let kits = tracked_kits(&root).expect("git ls-files");
        assert!(
            !kits.is_empty(),
            "no tracked kit record found — an empty population is not a clean answer"
        );
        let (mut one, mut several, mut none) = (0usize, 0usize, 0usize);
        for unit in &kits {
            match reads(&root, unit)
                .unwrap_or_else(|e| panic!("{unit}: this reader cannot read its record: {e}"))
            {
                Reads::One { revision, .. } => {
                    assert_eq!(
                        revision.len(),
                        40,
                        "{unit}: `{revision}` is not a full sha and a short one \
                         resolves differently in a bigger clone"
                    );
                    one += 1;
                }
                Reads::Several(pinned) => {
                    assert!(
                        pinned.len() > 1,
                        "{unit}: Several with {} row(s)",
                        pinned.len()
                    );
                    several += 1;
                }
                Reads::NoReplay { reason, prose } => {
                    assert!(
                        !reason.trim().is_empty() && !prose.trim().is_empty(),
                        "{unit}"
                    );
                    none += 1;
                }
            }
        }
        println!(
            "{} tracked kit(s): {one} name one revision, {several} name several, \
             {none} declare no replay",
            kits.len()
        );
        assert_eq!(one + several + none, kits.len(), "a kit fell through");
    }

    /// THE LISTING LEAVES NO KIT OUT, which is the property that makes it an
    /// overview rather than a sample: every unit `tracked_kits` finds appears,
    /// a kit whose replays name two revisions appears once per revision and
    /// says so, and a kit with no replay appears carrying its reason. A listing
    /// that skipped the last kind would read as a tree where every kit has a
    /// revision — which is the thing this round exists to stop being assumed.
    #[test]
    fn the_listing_leaves_no_kit_out_and_says_which_kind_each_is() {
        let root = repo_root().expect("this test runs inside the repository");
        let kits = tracked_kits(&root).expect("git ls-files");
        let lines = list_lines(&root).expect("the listing");
        for unit in &kits {
            assert!(
                lines.iter().any(|l| l.starts_with(&format!("{unit}\t"))),
                "{unit} is in the population and not in the listing"
            );
        }
        assert!(
            lines.len() >= kits.len(),
            "a kit naming two revisions must take two lines"
        );
        // Both of the kinds a plain `unit -> revision` table cannot express are
        // present in this repository's own record, so neither assertion above
        // is judging an empty set.
        assert!(
            lines.iter().any(|l| l.contains("(--replay decides)")),
            "no multi-revision kit in the listing: {lines:#?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("no replay: ")),
            "no kit without a replay in the listing: {lines:#?}"
        );
    }

    /// THE CARGO THE CALLER NAMED is the one that runs. R1190's lesson is that
    /// a build failure here is reported as a finding about the REVISION, so a
    /// build that quietly used a different toolchain would print that sentence
    /// about the wrong thing — and an assertion that merely watched the build
    /// succeed would pass either way.
    ///
    /// Both halves are asked with programs that already exist, for the reason
    /// in the body: writing one and spawning it is what `written-executable`
    /// refuses, and it refused this test's first spelling.
    #[test]
    fn the_cargo_the_caller_named_is_the_one_that_builds() {
        let root = tmp();
        let git_ = |args: &[&str]| {
            let ok = Command::new("git")
                .args(args)
                .current_dir(&root)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            assert!(ok, "git {args:?}");
        };
        git_(&["init", "-q"]);
        std::fs::write(root.join("a.txt"), "one commit is all git archive needs\n").unwrap();
        git_(&["add", "a.txt"]);
        git_(&[
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "seed",
        ]);

        // NOTHING HERE WRITES A PROGRAM, and that is this repository's own
        // `written-executable` gate talking: a file made executable and then
        // spawned in the same function fails with ETXTBSY while some other
        // test's fork still holds it open for writing, and the failure lands in
        // a crate that did nothing. So the question is asked with programs that
        // already exist.
        let into = |name: &str| {
            let d = root.join(name);
            std::fs::create_dir_all(&d).unwrap();
            d
        };

        // (a) A cargo that is not there. The refusal NAMES it, which is the
        //     only way this test can tell "the one the caller named" from
        //     "whatever PATH answers with" — both are absent-or-present in the
        //     same way and only one of them appears in the message.
        let missing = root.join("no-cargo-lives-here");
        let named = missing.to_str().unwrap().to_string();
        let err = materialise(&root, "HEAD", &into("a"), &named).unwrap_err();
        assert!(
            err.contains(&named),
            "the refusal must name the cargo it was given: {err}"
        );

        // (b) A cargo that succeeds and builds nothing — the shape a silently
        //     wrong toolchain has, since the build "passed" and there is no
        //     binary. That must be a refusal and not a returned path.
        let err = materialise(&root, "HEAD", &into("b"), "/bin/true").unwrap_err();
        assert!(
            err.contains("reported success and there is no binary"),
            "a build that produced nothing must refuse: {err}"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A revision this clone does not hold is a DIFFERENT finding from one that
    /// no longer builds, and the two must not arrive in the same sentence: the
    /// first is a shallow checkout, the second kills the pin design.
    #[test]
    fn a_revision_this_clone_does_not_hold_says_so_and_does_not_build() {
        let root = tmp();
        let out = Command::new("git")
            .args(["init"])
            .current_dir(&root)
            .output();
        assert!(out.map(|o| o.status.success()).unwrap_or(false), "git init");
        let into = root.join("into");
        std::fs::create_dir_all(&into).unwrap();
        let err = materialise(
            &root,
            "0000000000000000000000000000000000000000",
            &into,
            "cargo",
        )
        .unwrap_err();
        assert!(
            err.contains("does not hold that revision"),
            "the missing-revision message must not read as a build failure: {err}"
        );
        assert!(
            !err.contains("no longer builds"),
            "a revision that was never extracted cannot have failed to build: {err}"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
