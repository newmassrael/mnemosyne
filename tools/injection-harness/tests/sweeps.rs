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

use std::collections::{BTreeMap, BTreeSet};
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

/// Whether a manifest is an INPUT TO A TEST rather than a sweep somebody runs.
///
/// Declared once for every law in this file, because two laws disagreeing about
/// which files they are about is the same defect as a law over no files at all.
/// The rule, and what it cost to arrive at, is written where it is first used.
fn a_test_input(path: &str) -> bool {
    path.split('/').any(|part| part == "tests")
}

#[test]
fn every_tracked_sweep_still_applies_to_the_tree_it_names() {
    let root = repository_root();
    let tracked = tracked_json(&root);

    // WHICH OF THEM IS A SWEEP IS ASKED OF THE READER THE HARNESS USES, file by
    // file. A directory rule answered this by where a file sits and was wrong in
    // both directions at once (see `tracked_json`); the file itself is the only
    // thing that knows.
    // AND A MANIFEST IS NOT THE SAME THING AS A SWEEP SOMEBODY RUNS. Some are
    // INPUTS TO TESTS, whose trees the test itself materialises, so their
    // anchors name files that exist only while that test runs.
    //
    // THE CLASSIFIER IS WHERE THE FILE LIVES, AND IT HAD TO BE. The obvious one
    // — does the tree it names hold the files it edits — is the very question
    // this law asks, so using it as a filter would reclassify every ROTTED sweep
    // as somebody's fixture and pass. `tests/` is cargo's own word for a
    // directory of test inputs.
    //
    // THE COST OF THAT RULE WAS PAID ONCE AND IS WORTH KEEPING WRITTEN DOWN
    // (Round 1138). It is only sound while nothing runnable is kept in a
    // `tests/` directory, and Round 1088 put a real contract sweep in one —
    // where it was skipped here for its whole life, and where its `repo` of `.`
    // resolved to the tests directory rather than the root, so it could not have
    // run either. Both manifests now live in `crates/mnemosyne-cli/sweeps/`, and
    // the rule below is what would catch the next one that walks back.
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
    // A TEST'S OWN INPUTS SIT IN `fixtures/`, which is the other half of the
    // location rule and the half Round 1088 walked past: a manifest directly
    // under `tests/` is not an input to anything, it is a sweep somebody means to
    // run, and the rule above would skip it in silence — which it did, for the
    // whole life of the file. Naming BOTH shapes here, because the one that got
    // through was not the one the first rule watched for.
    let mut disguised: Vec<&String> = inputs
        .iter()
        .filter(|path| {
            path.ends_with("/injection-sweep.json")
                || !path.split('/').any(|part| part == "fixtures")
        })
        .collect();
    disguised.sort();
    assert!(
        disguised.is_empty(),
        "{} manifest(s) read as a sweep this repository RUNS while sitting where \
         its test inputs live — under `tests/` and not in a `fixtures/` \
         directory — so nothing checks their anchors and nothing runs them: \
         {disguised:?}",
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

/// A SWEEP THAT SAYS `--workspace` MUST RUN THE WHOLE OF IT (Round 1171).
///
/// A sweep's verdict is "which tests went red", so the suite its command
/// reaches is the population every one of its findings is about. Three sweeps
/// here run `cargo test --workspace` with no features, and this repository's
/// features are not decoration: MEASURED on this tree, the same workspace is
/// 1956 tests without them and 1965 with — nine tests in the server's
/// `#![cfg(feature)]` targets, which compile to an EMPTY test binary rather
/// than to an error when the feature is off.
///
/// CI IS NOT WRONG TO SPLIT, AND A SWEEP CANNOT. The workflow runs
/// `cargo test --workspace --locked` in one job and
/// `cargo test -p mnemosyne-server --all-features --locked` in another, so
/// between them every test is run. A sweep has ONE command: whatever those nine
/// tests alone would have caught, it scores as an injection that reddened
/// nothing — the "vacuous law" verdict, which is the one answer a device built
/// to detect vacuity must never invent.
///
/// AND THE CONTROL COUNT IS READ AS THE REPOSITORY'S. Round 1145 recorded
/// "CONTROL 1870 passed, 161 targets" in a manifest header; a reader comparing
/// it against a session's own `--all-features` number sees a discrepancy that
/// is not a defect and can spend an hour on it. That happened, in the session
/// that wrote this law.
///
/// SCOPED TO `--workspace` DELIBERATELY. A sweep aimed at one target claims
/// nothing about the repository's suite, so requiring features of it would be
/// this law over-applying — the direction Round 1144 asks a new refusal to
/// name.
#[test]
fn a_sweep_that_runs_the_whole_workspace_runs_its_features_too() {
    let root = repository_root();
    let here = root.canonicalize().expect("the repository root resolves");
    let mut whole_workspace = Vec::new();
    let mut without_features = Vec::new();
    for path in tracked_json(&root) {
        let Ok(manifest) = injection_harness::read_manifest(&root.join(&path)) else {
            continue;
        };
        let argv = &manifest.test_command;
        if !argv.iter().any(|a| a == "--workspace") {
            continue;
        }
        // WHOSE WORKSPACE — asked of the manifest's own `repo`, already resolved
        // against its directory, and not of where the file sits. A test's INPUT
        // manifest names the tree that test materialises, so `--workspace` in it
        // is a claim about a fixture and none of this law's business; the
        // sibling law above answers the same question by directory, and this one
        // cannot, because a fixture that says `--workspace` about ITS tree is
        // correct while the same words about this one are the defect.
        if manifest.repo.canonicalize().ok().as_deref() != Some(here.as_path()) {
            continue;
        }
        whole_workspace.push(path.clone());
        let enables = argv
            .iter()
            .any(|a| a == "--all-features" || a == "--features" || a.starts_with("--features="));
        if !enables {
            without_features.push(format!("{path}: {}", argv.join(" ")));
        }
    }

    // NON-VACUITY, AND IT IS THE SAME SHAPE THE LAW IS ABOUT: a walk that
    // matched no manifest would pass this in the silence of a clean repository.
    assert!(
        whole_workspace.len() >= 3,
        "only {} sweep(s) claim the whole workspace, which is fewer than this \
         repository has — a law over the wrong population is the empty answer \
         that reads as a clean one: {whole_workspace:?}",
        whole_workspace.len()
    );
    assert!(
        without_features.is_empty(),
        "{} sweep(s) run `cargo test --workspace` with no features, so their \
         control and every red set they report are about a suite nine tests \
         short of this repository's — and an injection that only those nine \
         would catch is scored as a law that cannot break:\n  {}",
        without_features.len(),
        without_features.join("\n  ")
    );
}

/// Which cargo — named rather than inherited, this repository's own law about
/// what a spawned program is allowed to read from the machine (R1182).
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

/// Every flag `cargo test` accepts that NAMES a single target, and the kind of
/// target the name has to be.
///
/// THIS IS CARGO'S SET AND NOT THIS REPOSITORY'S, which is what makes it a
/// closed one rather than a list that goes stale: the four below are all the
/// ways a `cargo test` invocation can name one target. The plural forms
/// (`--tests`, `--benches`, `--all-targets`, `--lib`, `--doc`) select a CLASS
/// and name nothing, so there is nothing in them that can stop resolving.
const NAMES_A_TARGET: [(&str, &str); 4] = [
    ("--test", "test"),
    ("--bench", "bench"),
    ("--bin", "bin"),
    ("--example", "example"),
];

/// What a tree holds, as CARGO answers it: every package, and the kind and name
/// of every target each one declares.
///
/// `--no-deps`, which is a census and not a resolution: it wants no registry, no
/// network and no lockfile, and the separate in-repo workspaces this walks have
/// their own of each.
fn targets_of(tree: &Path) -> BTreeMap<String, BTreeSet<(String, String)>> {
    let out = Command::new(cargo())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .arg("--manifest-path")
        .arg(tree)
        .output()
        .expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "cargo metadata failed for {}: {}",
        tree.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("cargo metadata answers JSON");
    let mut declared = BTreeMap::new();
    for package in metadata["packages"]
        .as_array()
        .expect("cargo metadata lists packages")
    {
        let name = package["name"]
            .as_str()
            .expect("a package cargo lists has a name")
            .to_string();
        let mut targets = BTreeSet::new();
        for target in package["targets"]
            .as_array()
            .expect("a package cargo lists has targets")
        {
            let named = target["name"]
                .as_str()
                .expect("a target cargo lists has a name")
                .to_string();
            // A TARGET CARRIES ITS KINDS AS A LIST and this reads all of them:
            // the same name can be selectable two ways, and a reader that took
            // only the first would answer about whichever cargo printed first.
            for kind in target["kind"]
                .as_array()
                .expect("a target cargo lists has a kind")
            {
                if let Some(kind) = kind.as_str() {
                    targets.insert((kind.to_string(), named.clone()));
                }
            }
        }
        declared.insert(name, targets);
    }
    declared
}

/// The names a `cargo test` command hands cargo to select WHAT IT RUNS, and the
/// manifest it hands it to select WHERE.
///
/// Parsed by walking the argv the manifest actually carries, rather than by
/// matching a shape: a flag and its value are two words, and a reader that
/// searched for the value alone would accept `--test` naming the next flag.
#[derive(Debug, Default)]
struct Selectors {
    packages: Vec<String>,
    /// `(kind, name)`, in the same spelling `cargo metadata` answers with.
    targets: Vec<(String, String)>,
    /// Whether the command selects every member, whatever else it names.
    whole_workspace: bool,
    /// Whether it removes members from that selection — a shape this law
    /// refuses rather than models.
    excludes: bool,
    manifest_path: Option<PathBuf>,
}

fn selectors_of(argv: &[String]) -> Selectors {
    let mut found = Selectors::default();
    let mut words = argv.iter();
    while let Some(word) = words.next() {
        if let Some((_, kind)) = NAMES_A_TARGET.iter().find(|(flag, _)| flag == word) {
            found
                .targets
                .extend(words.next().map(|name| (kind.to_string(), name.clone())));
            continue;
        }
        match word.as_str() {
            "-p" | "--package" => found.packages.extend(words.next().cloned()),
            // `--workspace` OVERRIDES a `-p`, so a command carrying both selects
            // every member and a reader that honoured the narrower one would
            // call a target unreachable that cargo reaches.
            "--workspace" | "--all" => found.whole_workspace = true,
            // AND THE ONE SHAPE THIS CANNOT MODEL IS NAMED RATHER THAN GUESSED
            // AT. `--exclude` removes members from the selection, so a target
            // this law finds in an excluded package is one cargo would not —
            // a wrong answer in the direction that reads as clean. No sweep
            // here carries it; the day one does, this says so instead.
            "--exclude" => found.excludes = true,
            "--manifest-path" => found.manifest_path = words.next().map(PathBuf::from),
            _ => {}
        }
    }
    found
}

/// What a command names and the tree does not hold.
///
/// THE DECISION IS FACTORED OUT SO ITS CONTROL DRIVES THE SAME CODE. A control
/// that re-implemented this rule would prove that a second spelling can fail,
/// which is the one thing nobody needs to know — the same reason the anchor law
/// above calls the harness's own `snapshot_and_dry_run` rather than a copy.
fn unresolved(
    selectors: &Selectors,
    declared: &BTreeMap<String, BTreeSet<(String, String)>>,
) -> Vec<String> {
    if selectors.excludes {
        return vec![
            "its command carries `--exclude`, which this law cannot model — a target found \
             in an excluded package would be reported reachable when cargo would not reach \
             it, and that wrong answer is the one that reads as clean"
                .to_string(),
        ];
    }
    let mut missing = Vec::new();
    for package in &selectors.packages {
        if !declared.contains_key(package) {
            missing.push(format!(
                "`-p {package}` names a package this tree does not build"
            ));
        }
    }
    // A NAME IS LOOKED FOR WHERE THE COMMAND WOULD LOOK: inside the packages it
    // selected, or across the whole tree when it selected none — or when
    // `--workspace` overrode what it selected.
    let mut reachable: BTreeSet<&(String, String)> = BTreeSet::new();
    for (package, targets) in declared {
        let selected = selectors.whole_workspace
            || selectors.packages.is_empty()
            || selectors.packages.contains(package);
        if selected {
            reachable.extend(targets);
        }
    }
    for wanted in &selectors.targets {
        if !reachable.contains(wanted) {
            let (kind, name) = wanted;
            missing.push(format!(
                "`--{kind} {name}` names a target no package it selects declares — cargo \
                 answers `no {kind} target named {name}` and refuses the whole invocation \
                 over it, so every injection in that sweep scores as nothing"
            ));
        }
    }
    missing
}

/// EVERY NAME A SWEEP'S COMMAND HANDS CARGO STILL RESOLVES.
///
/// The law at the top of this file asks whether a sweep's ANCHORS still apply to
/// the tree it edits. This one asks the other half — whether the SUITE it would
/// run is still there — and the two decay separately: a sweep whose every anchor
/// holds perfectly proves nothing if the command that would score it cannot
/// start.
///
/// WHAT IT COST, measured and not argued. Round 1171 folded seventy-six of
/// `mnemosyne-cli`'s test files into one `all` target for a link cost that had
/// grown into a ten-minute pre-commit gate. Two sweeps select a `--test` target
/// out of that pile by name, `.githooks/injection-sweep.json` among them — the
/// one that proves the HOOKS' own gates are not vacuous. From that commit its
/// command was `error: no test target named git_hooks_smoke`, and its eight
/// injections could not run at all.
///
/// THE HARNESS ALREADY REFUSES, and that is not enough. Run by hand it says "the
/// control reached no test target at all" and stops, which is exactly right and
/// reaches nobody: a sweep runs when somebody decides to run one, and the point
/// of tracking these files is that nobody has to decide. The refusal is a reader
/// of last resort; this is a reader on every commit.
///
/// ASKED OF CARGO, PER SWEEP, IN THE TREE THAT SWEEP DECLARES. A list of this
/// repository's test targets written beside this law would be the same
/// hand-kept list that went stale to begin with, one level up.
#[test]
fn every_sweep_names_a_suite_this_tree_can_still_run() {
    let root = repository_root();
    let mut selecting = 0;
    let mut judged = Vec::new();
    let mut broken = Vec::new();
    for path in tracked_json(&root) {
        // The same population and the same classifier as the anchor law: a
        // manifest a test materialises names a tree that exists only while that
        // test runs, so its command is a claim about a fixture.
        let Ok(manifest) = injection_harness::read_manifest(&root.join(&path)) else {
            continue;
        };
        if a_test_input(&path) {
            continue;
        }
        judged.push(path.clone());
        let selectors = selectors_of(&manifest.test_command);
        // WHERE THE COMMAND WOULD RUN is the sweep's own `repo`, already
        // resolved against the manifest's directory by `read_manifest`; a
        // `--manifest-path` in the command is relative to that, because that is
        // the directory the harness starts the command in.
        let tree = match &selectors.manifest_path {
            Some(named) => manifest.repo.join(named),
            None => manifest.repo.join("Cargo.toml"),
        };
        if !tree.is_file() {
            broken.push(format!(
                "{path}: its command runs in {}, where there is no manifest",
                tree.display()
            ));
            continue;
        }
        selecting += usize::from(!selectors.targets.is_empty());
        let declared = if selectors.excludes {
            // Nothing is asked of the tree in this case; the refusal below is
            // about the command, and reading the tree would only make the
            // wrong answer available.
            BTreeMap::new()
        } else {
            targets_of(&tree)
        };
        for missing in unresolved(&selectors, &declared) {
            broken.push(format!("{path}: {missing}"));
        }
    }

    // NON-VACUITY, IN BOTH DIRECTIONS. A walk that read no sweep passes this law
    // in silence, and so does one that read every sweep and found none selecting
    // a target by name — which is the only shape this law can catch.
    assert!(
        judged.len() >= 12,
        "this law judged {} sweep(s), which is fewer than this repository tracks: {judged:?}",
        judged.len()
    );
    assert!(
        selecting >= 3,
        "only {selecting} sweep(s) name a target, and a target named by nobody is a name \
         that cannot go stale — a law over the wrong population is the empty answer that \
         reads as a clean one"
    );
    assert!(
        broken.is_empty(),
        "{} sweep(s) name a suite this tree no longer builds. The anchors of such a \
         sweep can all still apply, and it still proves nothing, because the command \
         that would score it never starts:\n  {}",
        broken.len(),
        broken.join("\n  ")
    );
}

/// Every `#[test]` function a tree holds, and what the walk could not see.
#[derive(Debug, Default)]
struct TestNames {
    /// The function names, unqualified — see the law below for why the module
    /// path is deliberately not part of this answer.
    named: BTreeSet<String>,
    /// Files carrying an item-position macro INVOCATION, which can expand to
    /// tests this walk cannot see: syn does not walk macro bodies, which is what
    /// R1182 paid to learn. COUNTED AND REPORTED rather than treated as a
    /// silence — see the law for why the first draft was wrong to let them
    /// suppress a verdict.
    opaque: BTreeSet<PathBuf>,
    /// Every identifier appearing among the TOKENS of those invocations — the
    /// WEAK half of the answer, and the half R1184 did not have.
    ///
    /// syn will not expand a macro, so a `#[test] fn` it generates is invisible
    /// to `named` however plainly it is written. But the name is not invisible:
    /// `exercised! { report_frame_view_branch_reaches_the_answer: … }` carries
    /// that identifier as a token, and a walk of the token stream finds it
    /// without knowing anything about what the macro does with it.
    ///
    /// IT IS EVIDENCE AND NOT PROOF, which is exactly why it is a second set
    /// rather than more entries in `named`. An identifier among a macro's tokens
    /// may be an argument, a field, a type — anything. What it rules out is the
    /// only verdict this law is entitled to render: that the tree does not have
    /// the test AT ALL. Tokens rather than the invocation's text, so a name that
    /// merely occurs inside a string literal is not mistaken for one.
    generated: BTreeSet<String>,
    /// Files that did not parse — a file genuinely unread, which is the one
    /// thing here that makes a miss unanswerable rather than reportable.
    unparsed: Vec<(PathBuf, String)>,
    /// How many files were opened at all, so a zero is a measurement.
    read: usize,
}

/// Whether an attribute is the one that makes a function a test — by its LAST
/// segment, so `#[tokio::test]` counts and `#[cfg(test)]` on the module around
/// it does not.
fn a_test_attribute(attr: &syn::Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "test")
}

/// Every identifier among a token stream, through nested groups.
///
/// Recursive because the interesting names are never at the top: an invocation's
/// arguments sit inside its braces, and its entries inside those.
fn idents_in(tokens: proc_macro2::TokenStream, into: &mut BTreeSet<String>) {
    for tree in tokens {
        match tree {
            proc_macro2::TokenTree::Ident(ident) => {
                into.insert(ident.to_string());
            }
            proc_macro2::TokenTree::Group(group) => idents_in(group.stream(), into),
            _ => {}
        }
    }
}

fn test_names_in(tree: &Path) -> TestNames {
    struct Collect<'a> {
        into: &'a mut BTreeSet<String>,
        generated: &'a mut BTreeSet<String>,
        opaque: bool,
    }
    impl<'ast> syn::visit::Visit<'ast> for Collect<'_> {
        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            if item.attrs.iter().any(a_test_attribute) {
                self.into.insert(item.sig.ident.to_string());
            }
            syn::visit::visit_item_fn(self, item);
        }
        fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
            // `macro_rules! name { .. }` DEFINES one and carries an ident;
            // `name! { .. }` at item position INVOKES one and does not. Only
            // the second can put items this walk never sees into the crate.
            if item.ident.is_none() {
                self.opaque = true;
                // AND WHAT IT WAS HANDED IS READ, because "I cannot expand this"
                // is not "there is nothing in here".
                idents_in(item.mac.tokens.clone(), self.generated);
            }
            syn::visit::visit_item_macro(self, item);
        }
    }

    let mut found = TestNames::default();
    let mut queue = vec![tree.to_path_buf()];
    while let Some(directory) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // `target/` is cargo's output and `.git/` is not source; both
                // hold `.rs` files that no target compiles.
                let skip = matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("target") | Some(".git")
                );
                if !skip {
                    queue.push(path);
                }
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            found.read += 1;
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    found.unparsed.push((path, e.to_string()));
                    continue;
                }
            };
            match syn::parse_file(&text) {
                Ok(file) => {
                    let mut collect = Collect {
                        into: &mut found.named,
                        generated: &mut found.generated,
                        opaque: false,
                    };
                    syn::visit::visit_file(&mut collect, &file);
                    if collect.opaque {
                        found.opaque.insert(path);
                    }
                }
                Err(e) => found.unparsed.push((path, e.to_string())),
            }
        }
    }
    found
}

/// EVERY TEST A SWEEP'S PLAN NAMES IS ONE THIS TREE STILL HAS.
///
/// The law above asks whether a sweep's command still names a SUITE that
/// exists. This is the same question one level in: whether the tests its
/// `expect_red` lists name are still there. Both decay the same way and both
/// leave every anchor applying perfectly to a proof aimed at nothing.
///
/// THE HARNESS ALSO ASKS THIS, EXACTLY, AND THAT IS NOT ENOUGH — the same
/// argument R1183 made one level up. Its pre-flight holds each expectation
/// against the names the CONTROL RAN, which is the harness's own words and
/// cannot be wrong; it fires when somebody runs a sweep. This one fires on the
/// commit that renames the test, which is where the person who can fix it is.
///
/// SO IT ANSWERS A DELIBERATELY WEAKER QUESTION: the LEAF name, and not the
/// module path. Resolving `audit::tests::x` means following `mod` declarations
/// and `#[path]` attributes from each target root — a rule that already exists,
/// in `tools/named-environment`, that cost that round three defects to get
/// right, and that this law would have to spell a second time. A second
/// spelling of a resolver is the defect this repository keeps paying for, so
/// the module half is left to the pre-flight, which gets it exactly and for
/// free. What is left here is total for the decay that actually happens: a test
/// renamed or deleted.
///
/// A FILE THAT DID NOT PARSE IS A REFUSAL; A MACRO IS ANSWERED WEAKLY.
///
/// R1184's first draft made both refusals, and the measurement rejected that:
/// twenty files in this repository invoke an item-position macro, and letting
/// any of them silence the whole tree is the vacuity this device exists to
/// detect. So that round went the other way — report the miss, and MENTION the
/// macro possibility in the message.
///
/// THAT WAS THE OPPOSITE ERROR, AND IT COST TWO TRUE EXPECTATIONS. The message
/// hedged and the verdict did not: `missing` is an assertion failure, phrased
/// "this tree no longer has that test", rendered about names nothing here could
/// see. R1184 read it and deleted
/// `tests::report_frame_view_branch_reaches_the_answer` and
/// `tests::report_frame_view_order_path_reaches_the_answer` from the frame-view
/// sweep. Both tests exist. `macro_rules! exercised!` in `mnemosyne-mcp`
/// generates them, and R1186's sweep of that manifest reddened both — the
/// deletion had quietly weakened an exhaustive red set to prove less.
///
/// The way out is neither refusal nor a hedged verdict: ASK A WEAKER QUESTION
/// THAT CAN BE ANSWERED, and print both grounds. Strong ground is a `#[test] fn`
/// of that name in the syntax. Weak ground is the name appearing among the
/// TOKENS of an item-position macro invocation — which proves nothing about what
/// the macro does, and rules out the only verdict this law is entitled to
/// render. A name on neither ground is missing, and that is still a failure:
/// a test genuinely deleted appears in no syntax and in no invocation's tokens.
#[test]
fn every_test_a_sweep_names_is_one_this_tree_still_has() {
    let root = repository_root();
    let mut trees: BTreeMap<PathBuf, TestNames> = BTreeMap::new();
    let mut judged = 0;
    // THE TWO GROUNDS, COUNTED SEPARATELY AND BOTH PRINTED. A run in which every
    // expectation was carried by the weak ground is a run this law barely
    // answered, and a reader must be able to see that without re-deriving it.
    let (mut by_syntax, mut by_macro) = (0, 0);
    let mut missing = Vec::new();
    let mut unreadable = Vec::new();
    for path in tracked_json(&root) {
        let Ok(manifest) = injection_harness::read_manifest(&root.join(&path)) else {
            continue;
        };
        if a_test_input(&path) {
            continue;
        }
        // THE TREE THE SWEEP DECLARES, not the packages its command selects. A
        // name found in a package the command does not reach is a miss this law
        // does not report — generous in the direction that cannot raise a false
        // alarm, and the pre-flight is exact about the rest.
        let names = trees
            .entry(manifest.repo.clone())
            .or_insert_with(|| test_names_in(&manifest.repo));
        for injection in &manifest.injections {
            for expected in &injection.expect_red {
                let leaf = expected.rsplit("::").next().unwrap_or(expected);
                judged += 1;
                if names.named.contains(leaf) {
                    by_syntax += 1;
                    continue;
                }
                // THE WEAK GROUND. Not proof that the test exists — proof that
                // this walk is not entitled to say it does not.
                if names.generated.contains(leaf) {
                    by_macro += 1;
                    continue;
                }
                let report = format!("{path} / {}: `{expected}`", injection.name);
                if names.unparsed.is_empty() {
                    missing.push(format!(
                        "{report} — no `#[test] fn` of that name in that tree, and none \
                         of its {} item-position macro invocation(s) is even handed \
                         that identifier",
                        names.opaque.len()
                    ));
                } else {
                    unreadable.push(format!(
                        "{report} — and {} file(s) in that tree did not parse, so an \
                         absent name and an unread one look alike here",
                        names.unparsed.len()
                    ));
                }
            }
        }
    }

    // NON-VACUITY. A walk that opened nothing, and a repository whose every
    // expectation holds, print the same silence.
    let read: usize = trees.values().map(|names| names.read).sum();
    let known: usize = trees.values().map(|names| names.named.len()).sum();
    assert!(
        read >= 300 && known >= 500 && judged >= 50,
        "{read} file(s) read, {known} test name(s) found, {judged} expectation(s) judged \
         — too few to be this repository's, and a law over the wrong population is the \
         empty answer that reads as a clean one"
    );
    // AND ON WHICH GROUND EACH WAS CARRIED. A law whose every answer came from
    // the weak half is one that read tokens and no syntax at all, which is a
    // different device from the one this file claims to be.
    assert!(
        by_syntax > by_macro,
        "{by_syntax} expectation(s) carried by a `#[test] fn` in the syntax and \
         {by_macro} only by an identifier inside a macro invocation — the weak ground \
         is a refusal to over-claim, not a way to answer"
    );
    eprintln!(
        "[expectations] {judged} judged: {by_syntax} by syntax, {by_macro} by a macro's \
         tokens alone"
    );
    assert!(
        unreadable.is_empty(),
        "{} expectation(s) this law cannot answer for, because part of the tree they are \
         about would not parse:\n  {}",
        unreadable.len(),
        unreadable.join("\n  ")
    );
    assert!(
        missing.is_empty(),
        "{} expectation(s) name a test this tree no longer has. The sweep's anchors can \
         all still apply and its command can still start, and it proves nothing, because \
         what it says must go red does not exist:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

/// THE CONTROL FOR THE WALK, over a tree this case builds. The law above found
/// two real stale expectations on its first run, which is the evidence that it
/// works — and that evidence is spent the moment they are repaired. What is left
/// afterwards passes whether the walk sees anything or not.
#[test]
fn the_walk_finds_a_test_wherever_it_is_written_and_says_what_it_could_not_see() {
    let at = std::env::temp_dir().join(format!(
        "sweeps-walk-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&at);
    for directory in ["src", "target/debug", "unparseable"] {
        std::fs::create_dir_all(at.join(directory)).expect("mkdir");
    }
    std::fs::write(
        at.join("src/lib.rs"),
        "#[test]\nfn at_the_top() {}\n\
         #[cfg(test)]\nmod tests {\n    #[test]\n    fn inside_a_module() {}\n\
         \n    #[tokio::test]\n    async fn behind_a_path() {}\n}\n\
         fn not_a_test() {}\n",
    )
    .expect("write lib");
    // A DEFINITION carries an ident and generates nothing by itself; an
    // INVOCATION does not, and can put a `#[test]` into the crate that no walk
    // of the syntax will ever see.
    //
    // AND WHAT THE INVOCATION IS HANDED IS READ. `made_by_a_macro` is exactly
    // the shape that cost R1184 two true expectations: a name that is plainly
    // written, that `syn` will never report as a `#[test] fn`, and that this
    // walk must not answer "absent" about. The string literal beside it is why
    // the tokens are walked rather than the invocation's text.
    std::fs::write(
        at.join("src/generated.rs"),
        "macro_rules! declare {\n    () => {};\n}\ndeclare!();\n\
         exercised! { made_by_a_macro: \"only_in_a_literal\" }\n",
    )
    .expect("write macro user");
    // Cargo's output is not source, and it holds `.rs` files that no target
    // compiles — counting them would let a stale name be answered for by a
    // build artifact of the commit that made it stale.
    std::fs::write(
        at.join("target/debug/stale.rs"),
        "#[test]\nfn only_in_the_build_directory() {}\n",
    )
    .expect("write artifact");
    std::fs::write(at.join("unparseable/broken.rs"), "fn ( this is not rust\n")
        .expect("write broken");

    let found = test_names_in(&at);
    assert_eq!(
        found.named,
        BTreeSet::from([
            "at_the_top".to_string(),
            "inside_a_module".to_string(),
            "behind_a_path".to_string(),
        ]),
        "a test is found at any depth, and only a test is: {found:?}"
    );
    assert_eq!(found.opaque.len(), 1, "{found:?}");
    assert!(
        found.opaque.iter().all(|p| p.ends_with("generated.rs")),
        "a `macro_rules!` DEFINITION is not an invocation: {found:?}"
    );
    // THE WEAK GROUND, in both directions. A name handed to an invocation is
    // found, and a name that only occurs inside a string literal is not — which
    // is the whole reason this reads tokens instead of text.
    assert!(
        found.generated.contains("made_by_a_macro"),
        "a name an invocation is handed is not a name this tree lacks: {found:?}"
    );
    assert!(
        !found.generated.contains("only_in_a_literal"),
        "a literal is not an identifier, and treating it as one would carry any \
         expectation whose name a comment or a message happens to contain: {found:?}"
    );
    assert!(
        !found.named.contains("made_by_a_macro"),
        "and it is WEAK ground — the syntax never said this was a test: {found:?}"
    );
    assert_eq!(found.unparsed.len(), 1, "{found:?}");
    assert_eq!(
        found.read, 3,
        "the build directory is not source: {found:?}"
    );
    let _ = std::fs::remove_dir_all(&at);
}

/// THE CONTROL FOR THE LAW ABOVE, and the reason it is not merely a walk over
/// green files. The law's own first run is what proves it can fail — it named
/// four stale selectors across three sweeps on a tree everything else called
/// clean — but that evidence is spent the moment they are repaired, and what is
/// left afterwards passes whether the rule works or not.
///
/// Each case below is one way a command can name something the tree does not
/// hold, driven through the same `unresolved` the law uses.
#[test]
fn a_command_naming_what_a_tree_does_not_build_is_named_rather_than_passed() {
    let declared = BTreeMap::from([
        (
            "server".to_string(),
            BTreeSet::from([
                ("test".to_string(), "all".to_string()),
                ("lib".to_string(), "server".to_string()),
            ]),
        ),
        (
            "cli".to_string(),
            BTreeSet::from([("bin".to_string(), "cli".to_string())]),
        ),
    ]);
    let judge = |argv: &[&str]| {
        let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
        unresolved(&selectors_of(&argv), &declared)
    };

    // THE SHAPE THAT ROTTED: a target folded into another one, named by a
    // command that has no way to know.
    let gone = judge(&["cargo", "test", "-p", "server", "--test", "grpc_mtls_smoke"]);
    assert_eq!(gone.len(), 1, "{gone:?}");
    assert!(gone[0].contains("--test grpc_mtls_smoke"), "{gone:?}");

    // AND THE CONTROL FOR THAT CONTROL: the same command, the name that is
    // there. Without this the case above only says the rule rejects something.
    assert!(
        judge(&["cargo", "test", "-p", "server", "--test", "all"]).is_empty(),
        "a target the tree declares must resolve"
    );

    // A NAME IS THE PACKAGE'S, NOT THE TREE'S. `all` exists — in the other
    // package, which this command does not select.
    let elsewhere = judge(&["cargo", "test", "-p", "cli", "--test", "all"]);
    assert_eq!(elsewhere.len(), 1, "{elsewhere:?}");

    // UNLESS `--workspace` OVERRODE THE SELECTION, which is cargo's rule and
    // not this reader's opinion.
    assert!(
        judge(&["cargo", "test", "--workspace", "-p", "cli", "--test", "all"]).is_empty(),
        "`--workspace` selects every member whatever else the command names"
    );

    // EVERY FLAG THAT NAMES ONE, not only the one this repository happens to
    // use: a `--bin` or a `--bench` passed over in silence is the same rot with
    // nothing watching it.
    for (flag, name) in [("--bin", "cli"), ("--bench", "cli")] {
        let judged = judge(&["cargo", "test", flag, name]);
        let expected = usize::from(flag == "--bench");
        assert_eq!(
            judged.len(),
            expected,
            "`{flag} {name}` against a tree whose only `{name}` is a bin: {judged:?}"
        );
    }

    // A PACKAGE THAT IS NOT THERE, which is the other half of a selection.
    let no_package = judge(&["cargo", "test", "-p", "renamed"]);
    assert_eq!(no_package.len(), 1, "{no_package:?}");

    // AND THE SHAPE THIS LAW REFUSES RATHER THAN MODELS. `--exclude` narrows a
    // selection, so answering at all would risk the wrong answer in the
    // direction that reads as clean.
    let excluded = judge(&["cargo", "test", "--workspace", "--exclude", "cli"]);
    assert_eq!(excluded.len(), 1, "{excluded:?}");
    assert!(excluded[0].contains("--exclude"), "{excluded:?}");
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
