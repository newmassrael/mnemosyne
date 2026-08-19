//! Round 1253 — REBUILDING the store a kit's record says it built.
//!
//! R1248 moved the first half of "read a kit's evidence" out of a test: given a
//! kit, produce the build that reads it. This is the second half, and the two
//! were always one job split by an accident of where the code sat. A kit's
//! workspace is not tracked — `claudedocs/` is gitignored except for what a kit
//! declares — so the ONLY way to stand in the store a kit's replay produced is
//! to run its steps again from the record. Until this module that was possible
//! exclusively by running this repository's `#[ignore]`d test suite, which is
//! to say: not by a reader, not on another machine, and not by a person holding
//! the kit and asking what it contains.
//!
//! # The two refusals are refusals, and no flag turns them off
//!
//! A reconstruction is worth something only if it is a reconstruction OF the
//! original, and two things decide that:
//!
//! 1. **The record has not moved since it was authored.** Every file a step
//!    feeds is read from the PINNED tree and compared with the same path today;
//!    a difference ends the replay. Without it the run reads a later edit and
//!    reports it as the original's result — the exact failure R883 found in its
//!    own corpus.
//! 2. **A step the record marks `reject` must be rolled back.** Those are the
//!    negative controls. A replay in which they APPLY has not reproduced the
//!    run; it has produced a different store that happens to be built from the
//!    same inputs, and the probes have stopped probing.
//!
//! Neither is offered as an option. A `--force` here would produce a digest
//! that looks exactly like the real one and means something else, and the value
//! of the whole record is that its digest means one thing.
//!
//! # What this does NOT decide
//!
//! Whether the digest MATCHES what the record declares, whether a `blocked`
//! replay is still blocked, and whether two runs agree — those are judgements
//! about the record, and they live with the rest of the record's laws in
//! `crates/mnemosyne-cli/tests/evidence_replay_smoke.rs`. This module answers
//! one question: what store do these steps build at this revision. It prints
//! the declared digest beside the one it computed and leaves the verdict to the
//! caller, because a tool that failed on a mismatch could not be used to FIND
//! the digest of a replay that has never declared one.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::declare::git;
use crate::util::{normalize, read_file, sha256_hex, HResult};

/// What the record says must happen when a step runs.
///
/// AN ENUM AND NOT THE RAW STRING, so an unreadable word is a refusal at the
/// point the record is read rather than a step silently treated as `apply` —
/// which is what a `match` with a fallback arm would do to a typo in the one
/// field that distinguishes a negative control from an import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// The verb must succeed and its change must land.
    Apply,
    /// The verb must FAIL and leave the store as it was. These are the record's
    /// negative controls.
    Reject,
}

impl Expect {
    fn read(raw: Option<&str>) -> HResult<Self> {
        match raw.unwrap_or("apply") {
            "apply" => Ok(Expect::Apply),
            "reject" => Ok(Expect::Reject),
            other => Err(format!(
                "a step expects `{other}`, and a record can only say `apply` or \
                 `reject` — a word nothing reads is a negative control nothing runs"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Expect::Apply => "apply",
            Expect::Reject => "reject",
        }
    }
}

/// One step of a replay: a verb, the input it is fed, and what must happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub verb: String,
    /// Relative to the kit, exactly as the record writes it.
    pub input: String,
    pub expect: Expect,
}

/// One replay declared by a kit's record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub unit: String,
    pub name: String,
    pub revision: String,
    /// Why this replay cannot run here, when the record says so. A blocked
    /// replay is still ATTEMPTED — a note nobody re-checks goes stale the
    /// moment the obstacle is removed, and then it is a reason not to look.
    pub blocked: Option<String>,
    /// The store digest the record declares, when it has measured one.
    pub digest: Option<String>,
    /// The `mnemosyne.toml` the original run worked under, when the kit tracks
    /// one. Not decoration: a config that names the canon order and the
    /// narrative rules is what makes a kit's negative controls roll back.
    pub config: Option<String>,
    pub steps: Vec<Step>,
}

/// Which flag each verb takes its input under.
///
/// A CLOSED MAP AND A REFUSAL, not a default. The verbs here are the same set
/// the record's own vocabulary allows, so a verb that vanishes is caught where
/// the record is judged; what this catches is a verb whose FLAG changed, and it
/// catches it by name rather than by feeding the old flag to a binary that will
/// then say something else was wrong.
fn input_flag(verb: &str) -> HResult<&'static str> {
    match verb {
        "import-sections" | "import-facts" | "propose-verdict" => Ok("--manifest"),
        "import-typing-proposals" | "import-edge-proposals" => Ok("--proposals"),
        other => Err(format!(
            "no input flag is known for the verb `{other}` — a replay cannot be \
             run against a verb this tool cannot address"
        )),
    }
}

/// Read every replay one kit's record declares.
pub fn replays_of(root: &Path, unit: &str) -> HResult<Vec<Replay>> {
    let path = root.join(unit).join("replay.json");
    let raw = read_file(path.to_str().ok_or("record path is not utf-8")?)?;
    let doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("{unit}/replay.json is not JSON: {e}"))?;
    let declared = match doc.get("replays").and_then(|r| r.as_array()) {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for r in declared {
        let name = r
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| format!("{unit}/replay.json: a replay declares no name"))?;
        let revision = r
            .get("revision")
            .and_then(|n| n.as_str())
            .ok_or_else(|| format!("{unit}/replay.json: replay `{name}` declares no revision"))?;
        let steps_raw = r
            .get("steps")
            .and_then(|s| s.as_array())
            .ok_or_else(|| format!("{unit}/replay.json: replay `{name}` declares no steps"))?;
        let mut steps = Vec::new();
        for s in steps_raw {
            let verb = s
                .get("verb")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{unit}/{name}: a step declares no verb"))?;
            let input = s
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{unit}/{name}: step `{verb}` declares no input"))?;
            let expect = Expect::read(s.get("expect").and_then(|v| v.as_str()))
                .map_err(|e| format!("{unit}/{name}: {e}"))?;
            steps.push(Step {
                verb: verb.to_string(),
                input: input.to_string(),
                expect,
            });
        }
        out.push(Replay {
            unit: unit.to_string(),
            name: name.to_string(),
            revision: revision.to_string(),
            blocked: r
                .get("blocked")
                .and_then(|b| b.as_str())
                .map(str::to_string),
            digest: r
                .get("expected_store_sha256")
                .and_then(|d| d.as_str())
                .map(str::to_string),
            config: r.get("config").and_then(|c| c.as_str()).map(str::to_string),
            steps,
        });
    }
    Ok(out)
}

/// Pick one replay of one kit by name, or say which names there are.
pub fn replay_named(root: &Path, unit: &str, name: &str) -> HResult<Replay> {
    let all = replays_of(root, unit)?;
    all.iter().find(|r| r.name == name).cloned().ok_or_else(|| {
        let names: Vec<&str> = all.iter().map(|r| r.name.as_str()).collect();
        if names.is_empty() {
            format!("{unit} declares no replays at all")
        } else {
            format!("{unit} declares no replay `{name}` — it declares {names:?}")
        }
    })
}

/// What one replay produced, and how it got there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuilt {
    /// sha256 of the store the steps built.
    pub digest: String,
    /// The workspace holding it. The CALLER owns this directory, which is why
    /// the verb takes `--into`: the transcripts a kit records are the output of
    /// verbs run against exactly this store, and rebuilding a second workspace
    /// to run them in would be a second reconstruction free to differ from the
    /// one the digest pins (Round 974).
    pub workspace: PathBuf,
    pub steps: usize,
    /// Steps the record marked `reject` and which duly failed.
    pub rejected: usize,
}

/// An empty workspace this repository's CLI will accept.
///
/// THE SAME SEED R880 USED, and that is the reason to keep it exactly: a
/// different starting store gives a different digest, so every measurement any
/// kit has ever declared is a measurement of a run that began HERE. Which is
/// also why it is public and why `seed-workspace` is a verb — the definition of
/// what a kit's digest means cannot have two spellings, and it had three until
/// R1253 (this one, the replay runner's, and the pinned-revision recheck's).
pub fn seed_workspace(ws: &Path) -> HResult<()> {
    std::fs::create_dir_all(ws.join("docs/.atomic"))
        .map_err(|e| format!("cannot create the workspace at {}: {e}", ws.display()))?;
    std::fs::write(ws.join("mnemosyne.toml"), "[workspace]\n")
        .map_err(|e| format!("cannot write the workspace config: {e}"))?;
    std::fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        "{\"schema_version\": 1, \"sections\": {}, \"changelog_entries\": {}}\n",
    )
    .map_err(|e| format!("cannot write the seed store: {e}"))
}

/// Read one file out of the pinned tree AND out of the tree today, refusing if
/// they differ.
///
/// THE REFUSAL IS THE POINT and it has no flag. A replay reading a later edit
/// of its own input is not a replay; it is a fresh run whose result would be
/// reported under the original's name.
fn unmoved(root: &Path, tree: &Path, rel: &str, revision: &str, what: &str) -> HResult<Vec<u8>> {
    let then = std::fs::read(tree.join(rel))
        .map_err(|e| format!("{what}: {rel} is not in the tree at {revision}: {e}"))?;
    let now = std::fs::read(root.join(rel))
        .map_err(|e| format!("{what}: {rel} is not in the tree today: {e}"))?;
    if then != now {
        return Err(format!(
            "{what}: {rel} differs between {revision} and today — this replay \
             would not be reading the original"
        ));
    }
    Ok(then)
}

/// Rebuild the store one replay records, in `into`, using the binary at `cli`
/// and the tree at `tree`.
///
/// `root` is the repository today: every input is read from both trees and
/// refused if they disagree, which is why both are parameters.
pub fn rebuild(root: &Path, tree: &Path, cli: &Path, r: &Replay, into: &Path) -> HResult<Rebuilt> {
    let name = format!("{}/{}", r.unit, r.name);
    seed_workspace(into)?;

    // A DECLARED CONFIG ARRIVES WITH THE REST OF ITS DIRECTORY, because the
    // paths inside it are relative to where it sits — which is where the run's
    // CWD was. Reading the toml to chase those paths would be a second parser
    // free to disagree with the config crate's.
    if let Some(cfg) = &r.config {
        let joined = normalize(&format!("{}/{}", r.unit, cfg));
        let dir = joined
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .ok_or_else(|| format!("{name}: the declared config has no directory: {cfg}"))?;
        let siblings: BTreeSet<String> = git(root, &["ls-files", &dir])?
            .lines()
            .map(str::to_string)
            // `git ls-files <dir>` reaches into subdirectories, and the config's
            // neighbours are the files BESIDE it. A nested directory's contents
            // are that directory's business.
            .filter(|f| f.rsplit_once('/').map(|(d, _)| d) == Some(dir.as_str()))
            .collect();
        if siblings.is_empty() {
            return Err(format!(
                "{name}: the declared config {cfg} names {dir}, and git tracks \
                 no file there — the record points at nothing"
            ));
        }
        for f in &siblings {
            let bytes = unmoved(root, tree, f, &r.revision, &name)?;
            let base = f.rsplit_once('/').map(|(_, b)| b).unwrap_or(f.as_str());
            std::fs::write(into.join(base), &bytes)
                .map_err(|e| format!("{name}: cannot place {base} in the workspace: {e}"))?;
        }
        if !into.join("mnemosyne.toml").is_file() {
            return Err(format!(
                "{name}: the declared config {cfg} did not land in the workspace \
                 — a replay under the seed config is a replay of something else"
            ));
        }
    }

    let mut rejected = 0usize;
    for (n, s) in r.steps.iter().enumerate() {
        let rel = normalize(&format!("{}/{}", r.unit, s.input));
        unmoved(root, tree, &rel, &r.revision, &format!("{name} step {n}"))?;
        let flag = input_flag(&s.verb)?;
        let input = root
            .join(&rel)
            .to_str()
            .ok_or_else(|| format!("{name} step {n}: {rel} is not a utf-8 path"))?
            .to_string();
        let out = Command::new(cli)
            .args([&s.verb, flag, &input])
            .current_dir(into)
            .output()
            .map_err(|e| format!("{name} step {n}: cannot run {}: {e}", cli.display()))?;
        match (s.expect, out.status.success()) {
            (Expect::Apply, false) => {
                return Err(format!(
                    "{name} step {n} (`{} {rel}`) was rejected:\n{}",
                    s.verb,
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            (Expect::Reject, true) => {
                return Err(format!(
                    "{name} step {n} (`{} {rel}`) was APPLIED, and the record says \
                     it must be rolled back — the negative control has stopped \
                     controlling",
                    s.verb
                ));
            }
            (Expect::Reject, false) => rejected += 1,
            (Expect::Apply, true) => {}
        }
    }

    let store = std::fs::read(into.join("docs/.atomic/workspace.atomic.json"))
        .map_err(|e| format!("{name}: the store the replay built cannot be read: {e}"))?;
    Ok(Rebuilt {
        digest: sha256_hex(&store),
        workspace: into.to_path_buf(),
        steps: r.steps.len(),
        rejected,
    })
}

/// The human-readable lines one rebuild prints, in the order a reader wants
/// them: what ran, what came out, and what the record expected.
pub fn report(r: &Replay, built: &Rebuilt) -> Vec<String> {
    let mut lines = vec![
        format!("replay: {}/{}", r.unit, r.name),
        format!("revision: {}", r.revision),
        format!("workspace: {}", built.workspace.display()),
        format!(
            "steps: {} ({} declared {})",
            built.steps,
            built.rejected,
            Expect::Reject.as_str()
        ),
        format!("digest: {}", built.digest),
    ];
    match &r.digest {
        Some(d) if d == &built.digest => lines.push(format!("declared: {d} (agrees)")),
        Some(d) => lines.push(format!("declared: {d} (DIFFERS)")),
        None => lines.push("declared: none — this replay has never recorded one".to_string()),
    }
    if let Some(why) = &r.blocked {
        lines.push(format!("blocked: {why}"));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_expectation_is_refused_rather_than_read_as_apply() {
        // THE FALLBACK THIS EXISTS TO NOT HAVE. `expect` is the only field
        // separating a negative control from an import, so a word nothing reads
        // must stop the replay rather than quietly become the permissive one.
        assert_eq!(Expect::read(None), Ok(Expect::Apply));
        assert_eq!(Expect::read(Some("apply")), Ok(Expect::Apply));
        assert_eq!(Expect::read(Some("reject")), Ok(Expect::Reject));
        let err = Expect::read(Some("rollback")).expect_err("an unknown word is a refusal");
        assert!(err.contains("rollback"), "{err}");
    }

    #[test]
    fn a_verb_with_no_known_flag_is_named_rather_than_guessed() {
        assert_eq!(input_flag("import-facts"), Ok("--manifest"));
        assert_eq!(input_flag("import-edge-proposals"), Ok("--proposals"));
        let err = input_flag("import-nothing").expect_err("an unknown verb is a refusal");
        assert!(err.contains("import-nothing"), "{err}");
    }

    /// A throwaway pair of trees plus a workspace, named with the process for
    /// the reason `seal::tests::tmp` gives (Round 1175): a fixture this test
    /// also removes is per-run state.
    fn fixture(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let base =
            std::env::temp_dir().join(format!("eh-replay-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (root, tree, into) = (base.join("root"), base.join("tree"), base.join("ws"));
        for d in [&root, &tree] {
            std::fs::create_dir_all(d.join("kit/run")).expect("fixture dirs");
        }
        std::fs::create_dir_all(&into).expect("workspace dir");
        (root, tree, into)
    }

    fn one_step(expect: Expect) -> Replay {
        Replay {
            unit: "kit".to_string(),
            name: "only".to_string(),
            revision: "0123456789abcdef".to_string(),
            blocked: None,
            digest: None,
            config: None,
            steps: vec![Step {
                verb: "import-facts".to_string(),
                input: "run/facts.json".to_string(),
                expect,
            }],
        }
    }

    /// THE FIRST REFUSAL, and it has no flag. A replay reading a later edit of
    /// its own input is not a replay of anything — it is a fresh run whose
    /// result would be reported under the original's name, which is the exact
    /// failure R883 found in its own corpus.
    #[test]
    fn an_input_that_moved_since_the_record_ends_the_replay() {
        let (root, tree, into) = fixture("moved");
        std::fs::write(root.join("kit/run/facts.json"), "{\"today\": 1}").expect("today");
        std::fs::write(tree.join("kit/run/facts.json"), "{\"then\": 1}").expect("then");
        // The binary is never reached, which is the point: this refusal lands
        // before anything runs, so the path below need not be a program.
        let cli = tree.join("never-run");
        let err = rebuild(&root, &tree, &cli, &one_step(Expect::Apply), &into)
            .expect_err("a moved input is a refusal");
        assert!(err.contains("differs between"), "{err}");
        assert!(err.contains("kit/run/facts.json"), "{err}");
    }

    /// THE SECOND REFUSAL, and it has no flag either. The steps a record marks
    /// `reject` are its negative controls; a replay in which they APPLY has
    /// built a different store from the same inputs, and the probes have
    /// stopped probing.
    #[test]
    fn a_reject_step_that_applies_ends_the_replay() {
        let (root, tree, into) = fixture("applied");
        for d in [&root, &tree] {
            std::fs::write(d.join("kit/run/facts.json"), "{}").expect("input");
        }
        // A PROGRAM THIS TEST DID NOT WRITE (R1250): a file written and then
        // executed in one process is how `ETXTBSY` fails a crate that had
        // nothing to do with it. `/bin/true` succeeds whatever it is handed,
        // which is exactly the shape a negative control must not be allowed.
        let err = rebuild(
            &root,
            &tree,
            Path::new("/bin/true"),
            &one_step(Expect::Reject),
            &into,
        )
        .expect_err("a negative control that applied is a refusal");
        assert!(err.contains("was APPLIED"), "{err}");
    }

    /// And the same edge from the other side: a step the record says must land
    /// and which the binary refuses is the replay failing, not a curiosity.
    #[test]
    fn an_apply_step_the_binary_rejects_ends_the_replay() {
        let (root, tree, into) = fixture("rejected");
        for d in [&root, &tree] {
            std::fs::write(d.join("kit/run/facts.json"), "{}").expect("input");
        }
        let err = rebuild(
            &root,
            &tree,
            Path::new("/bin/false"),
            &one_step(Expect::Apply),
            &into,
        )
        .expect_err("a step that would not apply is a refusal");
        assert!(err.contains("was rejected"), "{err}");
    }

    /// The seed is what every declared digest was measured FROM, so a replay
    /// that ran must leave the store where the CLI writes it — and this is the
    /// assertion that would notice the seed quietly changing shape.
    #[test]
    fn a_replay_with_no_steps_still_leaves_the_seed_store_it_began_from() {
        let (root, tree, into) = fixture("seeded");
        let mut r = one_step(Expect::Apply);
        r.steps.clear();
        let built = rebuild(&root, &tree, Path::new("/bin/true"), &r, &into)
            .expect("a replay with no steps still seeds");
        assert_eq!(built.steps, 0);
        assert!(into.join("mnemosyne.toml").is_file());
        let store = std::fs::read(into.join("docs/.atomic/workspace.atomic.json")).expect("store");
        assert_eq!(built.digest, sha256_hex(&store));
    }
}
