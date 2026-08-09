//! Every injection sweep this repository tracks still applies to the tree.
//!
//! AN ANCHOR IS THE PART OF A PROOF THAT DECAYS. A sweep says a contract is not
//! vacuous by breaking the thing the contract is about, and the break is exact
//! text: `from` must occur EXACTLY once in the file it names. The source it
//! names then moves. Nothing tells anyone, because the only reader of an anchor
//! is a sweep being RUN, and a sweep is run when somebody decides to run one.
//!
//! THE COST IS MEASURED, NOT ARGUED. R1103 inserted one line between the two
//! lines the census gate's `every-job-owes-a-restore-record` matched on. From
//! that commit the injection applied to nothing — and, because the harness
//! refuses the WHOLE sweep at its pre-flight, neither did the twenty-five
//! injections beside it. Four rounds later R1107 re-ran that sweep for an
//! unrelated reason and the harness said so in one line. Between those two
//! commits, `tools/twice-compiled/injection-sweep.json` was a file that looked
//! exactly like a proof and proved nothing.
//!
//! WHAT MAKES THIS A LAW RATHER THAN A SECOND SWEEP: it runs no suite. Whether
//! an anchor still applies is a question about TEXT, and it is answered by the
//! very function the harness itself uses — `snapshot_and_dry_run`, the fused
//! snapshot-and-pre-flight pass — so this and the tool cannot come to different
//! answers about what applies.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("this crate lives two directories below the repository root")
        .to_path_buf()
}

/// Every tracked manifest, from `git ls-files`.
///
/// FROM GIT AND NOT FROM A WALK, for the reason `ci-plan` reads workflows that
/// way: a manifest that is not tracked is one nobody else can run, and one that
/// is tracked is one a reader will believe. A list of sweeps kept beside this
/// law would go stale the first time a crate gained one — which is the shape
/// R777, R783, R1080 and R1082 each closed one level at a time, and it would go
/// stale in the direction that reads as a pass.
///
/// EVERY `.json` UNDER `tools/` AND NOT THE ONES NAMED `injection-sweep.json`.
/// The harness's own sweep is called `self-check.json` and the worked example is
/// `example.json`; a glob on the common name would quietly leave both out, and
/// the harness's own is the one aimed at the tool every other sweep is run by.
/// So the rule is the directory, every file in it must READ as a manifest, and
/// a JSON put here that is not one turns this red on the day it is added rather
/// than being skipped into silence.
fn tracked_sweeps(root: &Path) -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "tools/**/*.json"])
        .current_dir(root)
        .output()
        .expect("git ls-files runs");
    assert!(
        out.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    files.sort();
    files
}

#[test]
fn every_tracked_sweep_still_applies_to_the_tree_it_names() {
    let root = repository_root();
    let sweeps = tracked_sweeps(&root);
    // NON-VACUITY FIRST, because a walk that found nothing and a repository
    // whose every anchor holds print the same silence — and this whole law is
    // about a proof that had quietly stopped proving.
    assert!(
        sweeps.len() >= 11,
        "this repository tracks {} sweep manifest(s), which is fewer than the \
         crates known to carry one — a check over the wrong population is the \
         empty answer that looks like a clean one: {sweeps:?}",
        sweeps.len()
    );

    let mut injections = 0;
    let mut edits = 0;
    let mut broken = Vec::new();
    for sweep in &sweeps {
        // EVERY ONE OF THEM MUST READ AS A MANIFEST. A sweep with a JSON typo
        // and a directory holding some other configuration are the same shape
        // to a reader that skipped what it could not parse, and both would take
        // a sweep out of this population without saying so.
        let manifest = injection_harness::read_manifest(&root.join(sweep)).unwrap_or_else(|why| {
            panic!(
                "{sweep} does not read as a sweep manifest, and `tools/` is \
                 where this repository's sweeps live: {why}"
            )
        });
        injections += manifest.injections.len();
        edits += manifest
            .injections
            .iter()
            .map(|injection| injection.edits.len())
            .sum::<usize>();
        // THE MANIFEST'S OWN `repo`, already absolute: `read_manifest` resolves
        // it against the manifest's directory, so this reader needs to know
        // nothing about where each sweep was meant to be run from — which is the
        // half that did not exist before this law needed it.
        if let Err(why) =
            injection_harness::snapshot_and_dry_run(&manifest.repo, &manifest.injections)
        {
            broken.push(format!("{sweep}: {why}"));
        }
    }

    // AND THE POPULATION IS COUNTED, so that a manifest read as holding no
    // injections at all cannot pass this as a file whose anchors all hold.
    assert!(
        injections >= 100 && edits >= injections,
        "{injections} injection(s) over {edits} edit(s) across {} sweep(s) — too \
         few to be this repository's",
        sweeps.len()
    );
    assert!(
        broken.is_empty(),
        "{} of the sweeps this repository tracks no longer apply to it. An \
         anchor is exact text and the source it names moves; a sweep that \
         refuses is a proof nobody is running, and it looks exactly like one \
         that holds:\n  {}",
        broken.len(),
        broken.join("\n  ")
    );
}

#[test]
fn a_sweep_whose_anchor_has_come_loose_is_named_rather_than_skipped() {
    // THE CONTROL FOR THE LAW ABOVE, and the reason it is not merely a walk. Two
    // ways of missing are the ones that matter: an anchor that matches NOTHING
    // (the source moved out from under it) and one that matches TWICE (the
    // source grew a second copy, and the edit nobody described is applied to
    // whichever came first). Both must be named; neither may be skipped.
    let text = "one\ntwo\nthree\ntwo\n";
    let file = "fixture.rs";
    let gone = injection_harness::Edit {
        file: file.to_string(),
        from: "four".to_string(),
        to: "5".to_string(),
    };
    let doubled = injection_harness::Edit {
        file: file.to_string(),
        from: "two".to_string(),
        to: "2".to_string(),
    };
    for (edit, hits) in [(&gone, 0), (&doubled, 2)] {
        let why = injection_harness::replace_once(text, edit)
            .expect_err("an anchor that does not apply exactly once");
        assert!(
            why.contains(&format!("occurs {hits} times")) && why.contains(file),
            "it says how many times and in which file: {why}"
        );
    }
    let once = injection_harness::Edit {
        file: file.to_string(),
        from: "three".to_string(),
        to: "3".to_string(),
    };
    assert_eq!(
        injection_harness::replace_once(text, &once).expect("exactly once"),
        "one\ntwo\n3\ntwo\n",
        "and the one that does apply is applied"
    );
}
