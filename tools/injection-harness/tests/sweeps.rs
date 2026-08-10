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

/// Every tracked `.json` in this repository, from `git ls-files`.
///
/// FROM GIT AND NOT FROM A WALK, for the reason `ci-plan` reads workflows that
/// way: a manifest that is not tracked is one nobody else can run, and one that
/// is tracked is one a reader will believe. A list of sweeps kept beside this
/// law would go stale the first time a crate gained one — which is the shape
/// R777, R783, R1080 and R1082 each closed one level at a time, and it would go
/// stale in the direction that reads as a pass.
///
/// THE WHOLE REPOSITORY, AND NOT `tools/`, WHICH IS WHAT R1134 PAID FOR. This
/// walk was `tools/**/*.json` on the argument that sweeps live there, and the
/// argument stopped being true twice in one session: R1132 put a sweep at
/// `.githooks/injection-sweep.json` — aimed at SHELL, which is the text most
/// likely to move out from under an anchor — and nothing here could see it,
/// while R1130 put three RECORDED API BODIES under `tools/cache-budget/tests/`
/// and this law turned main red for holding a fixture. A population fixed by
/// directory answers both questions wrongly, in both directions at once.
///
/// So the classification below is derived from the files instead, and every
/// tracked `.json` is judged rather than filtered.
fn tracked_json(root: &Path) -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "*.json"])
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

/// The directory a tracked path sits in, as the population is grouped by.
fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(head, _)| head)
}

#[test]
fn every_tracked_sweep_still_applies_to_the_tree_it_names() {
    let root = repository_root();
    let tracked = tracked_json(&root);

    // WHICH OF THEM IS A SWEEP IS ASKED OF THE READER THE HARNESS USES, file by
    // file. A directory rule answered this by where a file sits and was wrong in
    // both directions at once (see `tracked_json`); the file itself is the only
    // thing that knows.
    // AND A MANIFEST IS NOT THE SAME THING AS A SWEEP SOMEBODY RUNS. Two of them
    // are INPUTS TO TESTS: `crates/mnemosyne-cli/tests/` holds a pair whose
    // trees the test itself materialises, so their anchors name files that exist
    // only while that test runs.
    //
    // THE CLASSIFIER IS WHERE THE FILE LIVES, AND IT HAD TO BE. The obvious one
    // — does the tree it names hold the files it edits — is the very question
    // this law asks, so using it as a filter would reclassify every ROTTED sweep
    // as somebody's fixture and pass. `tests/` is cargo's own word for a
    // directory of test inputs, and no sweep this repository runs is in one.
    let a_test_input = |path: &str| path.split('/').any(|part| part == "tests");
    let mut manifests = Vec::new();
    let mut inputs = Vec::new();
    let mut others = Vec::new();
    for path in &tracked {
        match injection_harness::read_manifest(&root.join(path)) {
            Ok(manifest) if !a_test_input(path) => manifests.push((path.clone(), manifest)),
            Ok(_) => inputs.push(path.clone()),
            Err(why) => others.push((path.clone(), why)),
        }
    }

    // AND NOTHING RUNNABLE MAY HIDE AMONG THEM. The location rule keeps a test's
    // input out of this law; the NAME is what stops a sweep walking the other
    // way — `injection-sweep.json` is what this repository calls the ones it
    // runs, and one moved under `tests/` would be skipped above without a word.
    let disguised: Vec<&String> = inputs
        .iter()
        .filter(|path| path.ends_with("/injection-sweep.json"))
        .collect();
    assert!(
        disguised.is_empty(),
        "{} sweep(s) carry the name this repository gives the ones it RUNS while \
         sitting where its test inputs live, so nothing checks their anchors and \
         nothing runs them: {disguised:?}",
        disguised.len()
    );

    // NON-VACUITY FIRST, because a walk that found nothing and a repository
    // whose every anchor holds print the same silence — and this whole law is
    // about a proof that had quietly stopped proving.
    assert!(
        manifests.len() >= 12,
        "this repository tracks {} sweep manifest(s), which is fewer than the \
         crates known to carry one plus the hooks' own — a check over the wrong \
         population is the empty answer that looks like a clean one: {:?}",
        manifests.len(),
        manifests.iter().map(|(path, _)| path).collect::<Vec<_>>()
    );

    // A SWEEP HOME IS A DIRECTORY THAT ALREADY HOLDS ONE, which is how the rule
    // "every file here must read as a manifest" survives a population that also
    // holds fixtures. It is DERIVED and not written down: `.githooks/` became a
    // home the day R1132 put a sweep in it, with nothing to update.
    let homes: std::collections::BTreeSet<&str> = manifests
        .iter()
        .map(|(path, _)| directory_of(path))
        .collect();
    let mut hiding = Vec::new();
    for (path, why) in &others {
        // IN A HOME: a sweep with a JSON typo and some other configuration are
        // the same shape to a reader that skipped what it could not parse, and
        // both would take a sweep out of this population without saying so.
        if homes.contains(directory_of(path)) {
            hiding.push(format!(
                "{path} sits beside a sweep and does not read as one: {why}"
            ));
            continue;
        }
        // OR NAMED LIKE ONE, wherever it sits: the first sweep in a fresh
        // directory has no neighbour to be judged against, and a typo in it
        // would otherwise leave this population in silence.
        if path.rsplit('/').next().is_some_and(|name| {
            name.contains("sweep") || name == "self-check.json" || name == "example.json"
        }) {
            hiding.push(format!(
                "{path} is named like a sweep and does not read as one: {why}"
            ));
        }
    }
    assert!(
        hiding.is_empty(),
        "{} tracked file(s) look like a sweep this repository runs and are not \
         one. A sweep that stopped parsing is a proof nobody is running, and it \
         is indistinguishable from a fixture unless something asks:\n  {}",
        hiding.len(),
        hiding.join("\n  ")
    );

    let mut injections = 0;
    let mut edits = 0;
    let mut broken = Vec::new();
    for (path, manifest) in &manifests {
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
            broken.push(format!("{path}: {why}"));
        }
    }

    // AND THE POPULATION IS COUNTED, so that a manifest read as holding no
    // injections at all cannot pass this as a file whose anchors all hold.
    assert!(
        injections >= 100 && edits >= injections,
        "{injections} injection(s) over {edits} edit(s) across {} sweep(s) — too \
         few to be this repository's",
        manifests.len()
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
