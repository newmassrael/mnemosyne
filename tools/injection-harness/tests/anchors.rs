//! `anchors` put to a repository whose own sweeps a change breaks.
//!
//! R1291 — THE LAW HERE IS ABOUT A DIFFERENCE, so every case below has to make
//! two revisions and not one. A tree with a dead anchor is not the subject: this
//! gate exists because the round that KILLS an anchor is the one holding the
//! information, and a check that reported every dead anchor in the tree would
//! block a commit for somebody else's damage — which, in a working tree with
//! more than one writer, is most of them.
//!
//! THE BINARY IS RUN, not the library called, because the thing being asserted
//! is a three-code contract that lives in an exit status. R1096 measured what
//! testing around an exit code costs: the thing that lied WAS the number.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_injection-harness")
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl TempDir {
    fn path(&self) -> &Path {
        &self.0
    }
}

fn tempdir(name: &str) -> TempDir {
    let base = std::env::temp_dir().join(format!(
        "injection-anchors-{}-{name}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("tempdir");
    TempDir(base)
}

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write(root: &Path, path: &str, body: &str) {
    let at = root.join(path);
    if let Some(parent) = at.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(at, body).expect("write");
}

/// The line every fixture below anchors on, and the only thing that decides
/// whether an anchor holds. Written once so no case can accidentally be about a
/// different string than the manifest it builds.
const ANCHORED: &str = "let answer = 41 + 1;\n";

fn manifest(injection: &str, from: &str) -> String {
    format!(
        r#"{{
  "_": ["a fixture sweep"],
  "repo": "..",
  "test_command": ["true"],
  "logs": "../logs",
  "injections": [
    {{
      "name": "{injection}",
      "why": "a fixture",
      "edits": [{{ "file": "src/lib.rs", "from": {from}, "to": "let answer = 0;\n" }}],
      "expect_red": ["a_law"]
    }}
  ]
}}
"#
    )
}

fn record(injection: &str, from: &str) -> String {
    format!(
        r#"{{
  "complete": false,
  "fired": {{
    "{injection}": {{
      "edits": [{{ "file": "src/lib.rs", "from": {from}, "to": "let answer = 0;\n" }}],
      "expect_red": ["a_law"],
      "tests": ["a_law"]
    }}
  }},
  "forgotten": {{}}
}}
"#
    )
}

/// A committed repository whose one sweep anchors on [`ANCHORED`], with a firing
/// record beside it that was proven against that same definition.
fn seeded(name: &str) -> TempDir {
    let dir = tempdir(name);
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "fixture@example.invalid"]);
    git(root, &["config", "user.name", "fixture"]);
    write(
        root,
        "src/lib.rs",
        &format!("fn f() {{\n    {ANCHORED}}}\n"),
    );
    write(
        root,
        "sweeps/demo.sweep.json",
        &manifest("an-injection", "\"let answer = 41 + 1;\\n\""),
    );
    write(
        root,
        "sweeps/demo.sweep.firings.json",
        &record("an-injection", "\"let answer = 41 + 1;\\n\""),
    );
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "seed"]);
    dir
}

fn anchors(root: &Path) -> (i32, String) {
    let out = Command::new(binary())
        .args(["anchors", "--repo"])
        .arg(root)
        // WHICH CARGO, SAID RATHER THAN INHERITED (R1182's law, R1262's reason).
        // This binary links `ci-plan`, whose one door to a cargo command reads
        // `CARGO`, so a test that spawns it runs a different program on a machine
        // where that is set than on one where it is not. `harness.rs` answers
        // with the cargo running the test, because its cases RUN a suite through
        // it. These cases run none: the `anchors` verb issues `git` and nothing
        // else, so ABSENCE is the honest answer — and asserting it proves the
        // verb does not quietly depend on a cargo it never uses.
        .env_remove("CARGO")
        .current_dir(root)
        .output()
        .expect("run the anchors verb");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), said)
}

/// A tree whose anchors all hold is judged, and says so.
///
/// FIRST, because every case below is a difference against this one — and a gate
/// that reported a finding here would make all of them pass for the wrong reason.
#[test]
fn a_change_that_breaks_nothing_is_judged_and_clean() {
    let dir = seeded("clean");
    let root = dir.path();
    write(root, "README.md", "unrelated\n");
    git(root, &["add", "README.md"]);
    let (code, said) = anchors(root);
    assert_eq!(code, 0, "a change touching no anchor is clean:\n{said}");
    assert!(
        said.contains("1 sweep(s) in the index"),
        "and the population is printed, or a walk over nothing reads like a \
         clean run:\n{said}"
    );
    assert!(
        said.contains("0 finding(s) this change introduces"),
        "{said}"
    );
}

/// The case this gate exists for: a change moves the source an anchor names.
#[test]
fn a_change_that_moves_the_source_an_anchor_names_is_refused() {
    let dir = seeded("killed");
    let root = dir.path();
    write(root, "src/lib.rs", "fn f() {\n    let answer = 42;\n}\n");
    git(root, &["add", "src/lib.rs"]);
    let (code, said) = anchors(root);
    assert_eq!(code, 1, "a killed anchor is a JUDGED defect:\n{said}");
    assert!(
        said.contains("an-injection") && said.contains("occurs 0 times, not once"),
        "the finding names the injection and what it found:\n{said}"
    );
    // AND IT NAMES BOTH REPAIRS, because R1289 measured that they are not
    // interchangeable and the harness's own first message offered only one.
    assert!(
        said.contains("forget") && said.contains("--only"),
        "retiring the row and re-anchoring are different repairs and a reader \
         has to be told which is which:\n{said}"
    );
}

/// And damage that was already on `HEAD` is NOT laid at this change's door.
///
/// THE HALF THAT MAKES IT SAFE ON THE COMMIT PATH. This working tree has more
/// than one writer, and a gate that reported every dead anchor would stop a
/// commit for a defect its author never touched — which is how a gate teaches
/// people to reach for `--no-verify`.
#[test]
fn damage_already_on_head_is_not_this_changes() {
    let dir = seeded("carried");
    let root = dir.path();
    write(root, "src/lib.rs", "fn f() {\n    let answer = 42;\n}\n");
    git(root, &["add", "src/lib.rs"]);
    git(root, &["commit", "-q", "-m", "somebody else broke it"]);

    write(root, "README.md", "mine\n");
    git(root, &["add", "README.md"]);
    let (code, said) = anchors(root);
    assert_eq!(
        code, 0,
        "an anchor that was already dead at HEAD is not this change's:\n{said}"
    );
    assert!(
        said.contains("already at HEAD, not this change's"),
        "and it is REPORTED rather than swallowed — a carried finding nobody \
         prints is one nobody ever pays:\n{said}"
    );
}

/// The other half R1289 paid for: the change removes the injection and leaves
/// the proof of it standing.
#[test]
fn a_change_that_orphans_a_firing_record_is_refused() {
    let dir = seeded("orphan");
    let root = dir.path();
    write(
        root,
        "sweeps/demo.sweep.json",
        &manifest("a-different-injection", "\"let answer = 41 + 1;\\n\""),
    );
    git(root, &["add", "sweeps/demo.sweep.json"]);
    let (code, said) = anchors(root);
    assert_eq!(code, 1, "an orphaned proof is a defect:\n{said}");
    assert!(
        said.contains("holds a firing for `an-injection`"),
        "and the finding names the row whose definition is gone:\n{said}"
    );
}

/// A record kept beside an injection whose DEFINITION the change rewrote.
///
/// R1289's re-anchor is exactly this shape, and it is the case where the right
/// answer is "re-run", not "retire" — evidence proven against other text is
/// evidence about another injection.
#[test]
fn a_change_that_rewrites_an_injection_voids_its_evidence() {
    let dir = seeded("rewritten");
    let root = dir.path();
    write(
        root,
        "src/lib.rs",
        "fn f() {\n    let answer = 40 + 2;\n}\n",
    );
    write(
        root,
        "sweeps/demo.sweep.json",
        &manifest("an-injection", "\"let answer = 40 + 2;\\n\""),
    );
    git(root, &["add", "-A"]);
    let (code, said) = anchors(root);
    assert_eq!(
        code, 1,
        "the anchor holds again and the EVIDENCE does not:\n{said}"
    );
    assert!(
        said.contains("was proven against a different definition"),
        "{said}"
    );
}

/// A manifest that stops reading as one leaves the population in silence, and
/// that is the finding no anchor check makes.
#[test]
fn a_sweep_that_stops_parsing_is_refused_rather_than_dropped() {
    let dir = seeded("typo");
    let root = dir.path();
    write(root, "sweeps/demo.sweep.json", "{ this is not json\n");
    git(root, &["add", "sweeps/demo.sweep.json"]);
    let (code, said) = anchors(root);
    assert_eq!(
        code, 1,
        "every injection in it just became invisible, which is not a pass:\n{said}"
    );
    assert!(
        said.contains("read as a sweep at HEAD and does not now"),
        "{said}"
    );
}

/// And a repository this gate cannot be asked about answers with the THIRD code.
///
/// A check that could not run reports zero findings, and zero findings is what a
/// clean tree looks like.
#[test]
fn a_tree_git_cannot_be_asked_about_gets_no_verdict() {
    let dir = tempdir("ungit");
    let (code, said) = anchors(dir.path());
    assert_eq!(code, 2, "not judged is not clean:\n{said}");
    assert!(said.contains("NO VERDICT"), "and it says so:\n{said}");
}
