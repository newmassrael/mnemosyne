//! Round 880 — the feasibility proof for re-checking experiment evidence
//! WITHOUT editing it: build the revision a kit was authored under, and let
//! that binary read the kit's original manifest.
//!
//! R878 measured that 31 of 35 tracked experiment manifests no longer import.
//! R879 repaired the one whose failure was a pure transcription. The remaining
//! 30 fail on R708's removal of the `value`/`scalar` object shape, and the
//! obvious repair — re-type them into today's shape — was REJECTED on a
//! correctness ground, not a cost one: every kit's `deterministic_pins` is a
//! pre-committed claim about what the BLIND AUTHOR produced ("the author
//! converges to a base that imports with 0 errors"). Editing the author's
//! output until it imports re-establishes the pin by editing the thing the pin
//! measures, which is the R469 contamination bound this repo already imposes on
//! experiments.
//!
//! The alternative is to keep the record byte-identical and move the TOOL: a
//! kit is a workspace validated by one revision, which is exactly what the
//! `[tool] pin` machinery (R826, hardened R861/R863) already expresses for
//! consumers. This test exists because that whole design rests on ONE unmeasured
//! assumption — that an old revision still builds and still eats the original
//! bytes. Reading the code cannot answer it. So this builds it and runs it.
//!
//! Ignored by default: it extracts a second copy of the workspace and compiles
//! it (~12s warm, longer cold, and needs the crate deps already fetched). Run
//! it explicitly:
//!
//! ```text
//! cargo test -p mnemosyne-cli --test pinned_revision_rechecks_evidence_smoke -- --ignored --nocapture
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::issue::{self, Tree};
use tempfile::TempDir;

/// The kit under test: the OLDEST tracked manifest that today's binary rejects
/// (2026-06-13). The oldest is the honest choice — a recent revision building
/// would prove much less about whether this scales backwards.
const KIT: &str = "claudedocs/phase1-factsfirst-poc";

/// What the authoring revision imported, as it imported it. Asserted so that
/// "the old binary succeeded" cannot pass on an empty or half-read manifest.
const EXPECT_SECTIONS: &str = "3 created";
const EXPECT_FACTS: &str = "2 frames + 0 branches + 2 entities + 1 predicates + 3 facts created";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

fn git(args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("git exec");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("git output is utf-8")
}

/// The revision a kit was authored under, DERIVED rather than declared: the
/// commit that first added its manifest is the tree state the tool had when the
/// manifest was produced. Nothing here is invented — if this derivation is ever
/// wrong, it is wrong in a way git history can settle.
fn authoring_revision(manifest: &str) -> String {
    let rev = git(&[
        "log",
        "--diff-filter=A",
        "--format=%H",
        "-1",
        "--",
        manifest,
    ])
    .trim()
    .to_string();
    assert!(
        !rev.is_empty(),
        "no add-commit found for {manifest} — the derivation has no answer, \
         which is a finding, not a pass"
    );
    rev
}

/// The empty workspace this repository's CLI accepts, asked of the tool that
/// owns it (Round 1253).
///
/// It was transcribed here, and byte-identically in the replay runner, and a
/// third time inside `experiment-harness`. That is not a style point: the seed
/// store is what every declared digest was measured FROM, so "the same seed
/// R880 used" is the only reason two kits' measurements are comparable — and a
/// datum whose value is being one thing had three spellings free to drift.
fn seed_workspace(ws: &Path) {
    // `--locked` AND THE DECLARATION (R1262): the manifest is one this
    // repository tracks and `cargo run` resolves, so a free resolve here would
    // rewrite `tools/experiment-harness/Cargo.lock` instead of reporting it.
    let out = issue::cargo(Tree::ThisRepository)
        .args([
            "run",
            "-q",
            "--locked",
            "--manifest-path",
            "tools/experiment-harness/Cargo.toml",
            "--",
            "seed-workspace",
            "--into",
            ws.to_str().expect("utf-8 path"),
        ])
        .current_dir(repo_root())
        .output()
        .expect("experiment-harness exec");
    assert!(
        out.status.success(),
        "seed-workspace could not write {}:\n{}",
        ws.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn import(binary: &Path, ws: &Path, verb: &str, manifest: &Path) -> std::process::Output {
    Command::new(binary)
        .args([verb, "--manifest", manifest.to_str().expect("utf-8 path")])
        .current_dir(ws)
        // NAMED RATHER THAN INHERITED (Round 1182). This test's whole claim is
        // that ONE revision reads the bytes and another refuses them, so which
        // binary runs is the thing under test — and a workspace declaring
        // `[tool] pin` sends the CLI looking for an installed revision under
        // `$MN_ROOT`, or `$HOME/.local/mn`, and execs it. Both roots point at
        // this test's own workspace, where no install exists, and the two knobs
        // that decide the switch are removed.
        .env("MN_ROOT", ws)
        .env("HOME", ws)
        .env_remove("MNEMOSYNE_PIN_EXEC")
        .env_remove("MNEMOSYNE_PIN_SKIP")
        // AND `CARGO` SINCE R1262: this crate dev-depends on `ci-plan`, whose one
        // door to a cargo command reads it, so the law reads it as a variable the
        // spawned CLI can. Removed — the CLI this imports with runs no cargo, and
        // the build that DOES run one is `open-kit`, handed its cargo by name.
        .env_remove("CARGO")
        .output()
        .expect("cli exec")
}

/// The whole design in one test. The old revision builds, and it reads the
/// original bytes; today's revision, given the SAME bytes, rejects them. Both
/// halves are required: without the second, the test would pass just as well
/// against a manifest that was never stale, and would prove nothing about the
/// revision being what makes the difference.
#[test]
#[ignore = "extracts and compiles a second copy of the workspace; run with --ignored"]
fn an_old_revision_still_builds_and_still_reads_its_own_evidence() {
    let root = repo_root();
    let sections_rel = format!("{KIT}/section-manifest.json");
    let facts_rel = format!("{KIT}/facts-manifest.json");
    let rev = authoring_revision(&facts_rel);
    println!("derived authoring revision: {rev}");

    // Round 1248 — the extract and the build are `experiment-harness open-kit`,
    // which is the one place in this repository that turns a revision into the
    // binary that reads its evidence. This test carried its own copy and so did
    // the replay runner: two implementations of one mechanism, either free to
    // build with a cargo the other did not. The cargo is passed through for the
    // reason R1190 recorded — a failed build here is reported as a finding about
    // the REVISION, so a machine whose PATH cargo is a different channel would
    // have that sentence printed about this repository.
    let into = TempDir::new().expect("tempdir");
    let cargo = issue::program();
    let opened = issue::cargo(Tree::ThisRepository)
        .args([
            "run",
            "-q",
            "--locked",
            "--manifest-path",
            "tools/experiment-harness/Cargo.toml",
            "--",
            "open-kit",
            "--revision",
            &rev,
            "--into",
            into.path().to_str().expect("utf-8 path"),
            "--cargo",
            &cargo,
            "--json",
        ])
        .current_dir(&root)
        .output()
        .expect("experiment-harness exec");
    assert!(
        opened.status.success(),
        "the authoring revision could not be opened — THIS is the finding, and it \
         kills the pin-the-revision design:\n{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    let said: serde_json::Value =
        serde_json::from_slice(&opened.stdout).expect("open-kit --json prints json");
    let tree = PathBuf::from(said["tree"].as_str().expect("open-kit reports a tree"));
    let old_cli = PathBuf::from(said["cli"].as_str().expect("open-kit reports a cli"));
    assert!(old_cli.is_file(), "no binary at {}", old_cli.display());

    // The record has not moved since it was authored. If this ever fails, the
    // test below would be reading something other than the original, and every
    // conclusion drawn from it would be about the wrong bytes.
    for rel in [&sections_rel, &facts_rel] {
        let then = fs::read(tree.join(rel)).expect("manifest at the authoring revision");
        let now = fs::read(root.join(rel)).expect("manifest today");
        assert_eq!(
            then, now,
            "{rel} differs between {rev} and today — this test is not reading the original"
        );
    }

    // HALF ONE — the authoring revision reads its own evidence, unmodified.
    let ws = TempDir::new().expect("ws tempdir");
    seed_workspace(ws.path());
    let sec = import(
        &old_cli,
        ws.path(),
        "import-sections",
        &root.join(&sections_rel),
    );
    assert!(
        sec.status.success(),
        "old binary rejected the original sections: {}",
        String::from_utf8_lossy(&sec.stderr)
    );
    let sec_out = String::from_utf8_lossy(&sec.stdout).into_owned();
    assert!(
        sec_out.contains(EXPECT_SECTIONS),
        "expected `{EXPECT_SECTIONS}`, got:\n{sec_out}"
    );
    let facts = import(&old_cli, ws.path(), "import-facts", &root.join(&facts_rel));
    assert!(
        facts.status.success(),
        "old binary rejected the original facts: {}",
        String::from_utf8_lossy(&facts.stderr)
    );
    let facts_out = String::from_utf8_lossy(&facts.stdout).into_owned();
    assert!(
        facts_out.contains(EXPECT_FACTS),
        "expected `{EXPECT_FACTS}`, got:\n{facts_out}"
    );

    // HALF TWO — today's binary, same bytes, refuses. Without this the test is
    // a sentence about nothing: it must be the REVISION that differs, not the
    // manifest being fine all along.
    let today_cli = Path::new(env!("CARGO_BIN_EXE_mnemosyne-cli"));
    let ws_now = TempDir::new().expect("ws tempdir");
    seed_workspace(ws_now.path());
    let sec_now = import(
        today_cli,
        ws_now.path(),
        "import-sections",
        &root.join(&sections_rel),
    );
    assert!(
        sec_now.status.success(),
        "the sections half is NOT the stale one; if it starts failing this \
         test's contrast has moved: {}",
        String::from_utf8_lossy(&sec_now.stderr)
    );
    let facts_now = import(
        today_cli,
        ws_now.path(),
        "import-facts",
        &root.join(&facts_rel),
    );
    assert!(
        !facts_now.status.success(),
        "today's binary ACCEPTED the manifest — the contrast is gone and this \
         test no longer proves the revision is what differs"
    );
    let err = String::from_utf8_lossy(&facts_now.stderr).into_owned();
    assert!(
        err.contains("removed `value`/`scalar` object shape (Round 708)"),
        "expected the R708 refusal, got:\n{err}"
    );
    println!("today's binary refuses the same bytes: {}", err.trim());
}
