//! Round 882/884 — every tracked experiment input declares the revision that
//! can read it and the ordered steps that feed it, and neither declaration can
//! drift from git history or from the CLI's own verb list.
//!
//! The design this enforces, decided across R878–R881: a record is a triple —
//! the INPUT, the REVISION that produced it, and (later) the EXPECTED OUTPUT.
//! Re-checking runs the declared revision against the untouched input. The
//! input is never edited to suit a later tool, because a kit's
//! `deterministic_pins` is a pre-committed claim about what the BLIND AUTHOR
//! produced, and editing that output until it imports would re-establish the
//! pin by editing the thing the pin measures (the R469 contamination bound).
//! R880 proved the revision half is feasible — a June revision still builds and
//! still reads its own bytes. R881 proved the comparison is sound. R882 wrote
//! the revision down. R883 replayed the whole corpus and found the shape wrong.
//!
//! Round 884 widens it on the two axes R883 measured:
//!
//! 1. A replay unit is an ORDERED SEQUENCE, not a `(sections, facts)` pair.
//!    `disclosure-craft` feeds six manifests through four different verbs, and
//!    `continuity-stress` ends with three probes that must be REJECTED. A pair
//!    can express neither, so those inputs were simply absent from the record.
//! 2. The coverage oracle is DERIVED, not a hand-list. R882 recognised an input
//!    by its filename against a `FACTS_NAMES` const — which is a copy of the
//!    very class it was built to kill — and passed green while 14 tracked
//!    inputs sat undeclared, among them a whole re-extraction arm
//!    (`reextracted.manifest.json`, 54 facts). A kit can invent any filename it
//!    likes, so the question is asked of the bytes instead.
//!
//!    Asking "does today's parser accept it" is NOT the same question, and the
//!    difference was measured on identical state: the parse-based oracle found
//!    8 of those 14, the shape-based one all 14. See `classify`.
//!
//! Why this is a TEST and not a `validate-workspace` check: it is a property of
//! THIS repo's evidence, not of the Mnemosyne product. A consumer has no
//! `claudedocs/phase1-*` kits, and exporting a repo-local rule into the
//! validator every consumer runs would be the wrong home for it.
//!
//! Deliberately NOT asserted here: that the declared revision still BUILDS, or
//! that replaying reproduces anything. That is `pinned_revision_rechecks_
//! evidence_smoke` (R880), which compiles a second workspace and is `#[ignore]`d
//! for it. This gate is the cheap always-on half: the declaration exists, it
//! resolves, its verbs are real, and its steps name inputs that are there.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What a declared input IS. A closed vocabulary, because the reason a tracked
/// input is NOT replayed has to be stated somewhere, and this is the one home
/// for it.
///
/// `run-artifact` is the Round 953 member and it deliberately claims LESS than
/// its siblings. The other two name who produced the bytes; this one says only
/// that they are part of the kit's frozen run tree and that this record pins
/// them. That is the whole truth available for most of the tree: a role is a
/// claim, and 425 provenance claims inferred from one sampled file per class
/// would be exactly the false-claims-in-a-record Round 952 warned about when it
/// refused to call `mnemosyne.toml` raw agent output.
///
/// It is not a catch-all. The audit's load-bearing provenance — the frozen
/// first submissions, the sealed self-reports, the authored rules — is already
/// held by the sharper roles and stays there. `run-artifact` may only be used
/// under a `run/` tree, which is a constraint neither sibling carries, and no
/// replay step may feed it.
const INPUT_ROLES: &[&str] = &["replay-input", "raw-agent-output", "run-artifact"];

/// What a step expects the verb to do. `propose-verdict` probes are negative
/// controls: the run recorded them as manifests that MUST be rolled back, so a
/// replay that "succeeded" on one would have reproduced the wrong outcome.
///
/// Measured, not assumed: flipping all three `reject` steps to `apply` leaves
/// every test here green. Nothing downstream cites a probe's fact id, so this
/// file can check the vocabulary and the roll-back's consequence (a rejected
/// step contributes no ids) but has no fixture that separates the two values.
/// Enforcing the outcome needs the replay runner, which actually runs the verb.
const STEP_EXPECTATIONS: &[&str] = &["apply", "reject"];

const PROVENANCE_KINDS: &[&str] = &["derived-upper-bound", "declared-at-run"];

/// The record schemas this gate reads. One member today; it is a set so the
/// version sits in the same table as every other machine-checked literal
/// instead of being a bare string at its one use site.
///
/// v3 is Round 953: the accepted document set grew a case v2 could not
/// describe. A kit whose artifacts no verb can import — `factsfirst-craft`
/// carries 51 of them and not one sections or facts manifest — has an empty
/// `replays`, and must then say WHY in `no_replay` and declare no revision
/// provenance, there being no pin for one to describe. v2 would have accepted
/// such a record silently, which would exempt a whole kit from the replay half
/// and leave nothing to notice it.
const REPLAY_SCHEMAS: &[&str] = &["kit-replay/v3"];

/// Every literal a kit's `replay.json` is checked against, keyed by the field
/// that carries it. The cells ARE the consts above — a second reader of one
/// vocabulary, never a second copy of it.
///
/// Two things read this table, and that is the whole point of it existing.
/// `declarations` consults it to accept a record. `no_runbook_teaches_a_literal_
/// its_own_gate_rejects` consults it to reject a runbook that would tell the
/// next orchestrator to write a word the parser panics on. Round 942 wrote
/// `revision_provenance: "exact"` into a runbook by copying a design instead of
/// the gate; Round 944 fixed that runbook, and Round 948 then found the same
/// word still instructing from a second one. Nobody reasoned their way to the
/// retired value either time — they copied it, so the defence belongs at the
/// shape that gets copied.
///
/// A field that gains a parser check without gaining a row here is caught by
/// `vocabulary`, which panics rather than letting an unscanned field report no
/// violations.
const CHECKED_LITERALS: &[(&str, &[&str])] = &[
    ("schema", REPLAY_SCHEMAS),
    ("revision_provenance", PROVENANCE_KINDS),
    ("role", INPUT_ROLES),
    ("expect", STEP_EXPECTATIONS),
];

/// The value the two runbooks taught until Rounds 944 and 948 removed it. Kept
/// as the probe for the scan's own liveness because it is the word that
/// actually recurred; the probes assert it is outside every vocabulary, so it
/// cannot quietly become valid and leave the negative half testing nothing.
const RETIRED_LITERAL: &str = "exact";

/// The accepted set for one machine-checked field. Panics on a field with no
/// row: a missing vocabulary must never read as an empty one, because nothing
/// would then be scanned for it and the answer would come back "no violations".
fn vocabulary(field: &str) -> &'static [&'static str] {
    CHECKED_LITERALS
        .iter()
        .find(|(name, _)| *name == field)
        .unwrap_or_else(|| {
            panic!("`{field}` is checked by this gate but has no row in CHECKED_LITERALS")
        })
        .1
}

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

fn tracked_evidence_files() -> Vec<String> {
    git(&["ls-files", "claudedocs/phase1-*"])
        .lines()
        .map(str::to_string)
        .collect()
}

/// Every tracked file under a kit's `run/` tree.
///
/// `run/` is the boundary the kits drew themselves, and it is the right one:
/// what sits beside it — `report.md`, `runbook.md`, the design notes — is THIS
/// lineage's analysis, written after the fact and revisable at will. What sits
/// under it is what the run produced and was handed, and that is what the R469
/// contamination bound freezes.
fn run_artifacts() -> Vec<String> {
    tracked_evidence_files()
        .into_iter()
        .filter(|f| f.contains("/run/"))
        .collect()
}

/// The record that owns a path: the nearest ancestor directory holding a
/// `replay.json`. Nearest, not outermost, because kits nest — `disclosure-craft`
/// has a record at its root and a `v3/` beneath it — and a rule that reached for
/// the outermost would let a nested kit's artifacts be claimed by a record that
/// never saw them.
fn owning_unit(path: &str, units: &BTreeSet<String>) -> Option<String> {
    let mut dir = Path::new(path).parent();
    while let Some(d) = dir {
        let candidate = d.to_string_lossy().into_owned();
        if units.contains(&candidate) {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// Resolve `a/b/../c` without touching the filesystem — the declarations are
/// relative to their unit directory and some point back up out of it.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// The classes of tracked file a mutate verb will take. Named after the parser
/// that recognises them, so this list cannot describe a shape the product does
/// not actually import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputShape {
    Sections,
    Facts,
    TypingProposals,
    EdgeProposals,
}

/// The `FactsManifest` field names, taken FROM the type: an all-default parse
/// re-serialized is an object holding exactly its keys. A field added to the
/// manifest joins this set on its own; a hand-copied list would not.
fn facts_manifest_keys() -> BTreeSet<String> {
    let empty: mnemosyne_atomic::FactsManifest =
        serde_json::from_str("{}").expect("every FactsManifest field is #[serde(default)]");
    serde_json::to_value(empty)
        .expect("FactsManifest serializes")
        .as_object()
        .expect("a struct serializes to an object")
        .keys()
        .cloned()
        .collect()
}

/// What a file IS, and whether TODAY's parser still accepts it.
///
/// Those are two questions, and the first version of this gate asked only the
/// second — which found 8 of the 14 undeclared inputs and dropped the rest in
/// silence. The reason is the R878 measurement: much of this corpus predates the
/// R708
/// removal of the `{kind:value}` object shape, so today's parser rejects the
/// bytes with a migration work-list. A file the tool can no longer read is
/// still an input; that is the whole premise of pinning the revision that CAN
/// read it. An oracle whose reach shrinks as the tool moves forward would go on
/// reporting full coverage while the corpus fell out from under it.
///
/// So the shape question is answered structurally, and the parse question is
/// answered separately and reported.
fn classify(raw: &str) -> Option<(InputShape, bool)> {
    let doc: serde_json::Value = serde_json::from_str(raw).ok()?;

    if let Some(items) = doc.as_array() {
        // A sections manifest is a bare array of section imports. `section_id`
        // is the field the store keys them by, so its presence is the marker.
        let is_sections = !items.is_empty()
            && items
                .iter()
                .all(|s| s.get("section_id").and_then(|v| v.as_str()).is_some());
        if is_sections {
            let parses = serde_json::from_str::<Vec<mnemosyne_atomic::SectionImport>>(raw).is_ok();
            return Some((InputShape::Sections, parses));
        }
        return None;
    }

    let obj = doc.as_object()?;
    match obj.get("schema").and_then(|v| v.as_str()) {
        Some(mnemosyne_atomic::TYPING_PROPOSALS_SCHEMA) => {
            let parses = serde_json::from_str::<mnemosyne_atomic::TypingProposalsFile>(raw).is_ok();
            return Some((InputShape::TypingProposals, parses));
        }
        Some(mnemosyne_atomic::EDGE_PROPOSALS_SCHEMA) => {
            let parses = serde_json::from_str::<mnemosyne_atomic::EdgeProposalsFile>(raw).is_ok();
            return Some((InputShape::EdgeProposals, parses));
        }
        _ => {}
    }

    // A facts manifest is an object that would BUILD something: at least one of
    // the manifest's own arrays, non-empty. Presence alone is not enough — the
    // manifest is deliberately lenient (every field `#[serde(default)]`, unknown
    // keys ignored), so `{"schema": "canon-order/v1", "edges": [...]}` parses
    // cleanly into a manifest that creates nothing.
    let builds = facts_manifest_keys().iter().any(|k| {
        obj.get(k)
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
    });
    if builds {
        let parses = serde_json::from_str::<mnemosyne_atomic::FactsManifest>(raw).is_ok();
        return Some((InputShape::Facts, parses));
    }
    None
}

/// Every tracked `.json` under the evidence tree that is shaped like a mutate
/// verb's input, keyed by repo-relative path, with whether today's parser still
/// reads it.
fn classified_inputs() -> BTreeMap<String, (InputShape, bool)> {
    let root = repo_root();
    let mut out = BTreeMap::new();
    for file in tracked_evidence_files() {
        if !file.ends_with(".json") || file.ends_with("/replay.json") {
            continue;
        }
        let raw = match std::fs::read_to_string(root.join(&file)) {
            Ok(raw) => raw,
            Err(e) => panic!("read {file}: {e}"),
        };
        if let Some(classified) = classify(&raw) {
            out.insert(file, classified);
        }
    }
    out
}

struct Step {
    verb: String,
    input: String,
    expect: String,
}

struct Replay {
    unit: String,
    name: String,
    revision: String,
    blocked: Option<String>,
    /// sha256 of the store this replay builds at its pinned revision — the
    /// third of the triple, absent until R885's runner measured it.
    digest: Option<String>,
    /// The `mnemosyne.toml` the run itself worked under, when the kit tracked
    /// one. It is not decoration: `continuity-stress`'s config names the canon
    /// order and the narrative rules, and without them its three negative
    /// controls APPLY instead of rolling back — the probes stop probing.
    config: Option<String>,
    steps: Vec<Step>,
}

struct Input {
    unit: String,
    path: String,
    role: String,
    /// sha256 of the declared bytes, written once by `experiment-harness
    /// stamp-inputs` when the kit lands (Round 952).
    ///
    /// Required on EVERY input, including the ones a replay step feeds. Those
    /// are already pinned transitively — change a facts manifest and the store
    /// it builds stops matching `expected_store_sha256` — but that pin is only
    /// paid in the twelve-minute replay job, and it says nothing at all about
    /// an input NO step feeds. Every `raw-agent-output` is in that second
    /// class: the frozen first submissions and the sealed self-reports, which
    /// the R469 contamination bound rests on, were pinned by nothing until this
    /// field existed. One rule with no exceptions beats two with a seam.
    sha256: String,
}

struct Declarations {
    inputs: Vec<Input>,
    replays: Vec<Replay>,
    provenance: BTreeMap<String, String>,
    /// Per unit: why this kit declares no replay at all, when it declares none.
    /// Present exactly for the units whose `replays` is empty.
    no_replay: BTreeMap<String, String>,
}

fn declarations() -> Declarations {
    let root = repo_root();
    let mut inputs = Vec::new();
    let mut replays = Vec::new();
    let mut provenance = BTreeMap::new();
    let mut no_replay = BTreeMap::new();
    for file in tracked_evidence_files() {
        if !file.ends_with("/replay.json") {
            continue;
        }
        let unit = file.trim_end_matches("/replay.json").to_string();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(&file)).expect("read replay"))
                .unwrap_or_else(|e| panic!("{file} is not JSON: {e}"));
        let schema = doc["schema"].as_str().unwrap_or_default();
        assert!(
            vocabulary("schema").contains(&schema),
            "{file}: unknown record schema `{schema}`"
        );
        for i in doc["inputs"]
            .as_array()
            .unwrap_or_else(|| panic!("{file} declares no inputs"))
        {
            let path = i["path"].as_str().expect("input path").to_string();
            let sha256 = i["sha256"]
                .as_str()
                .unwrap_or_else(|| {
                    panic!(
                        "{file}: input {path} declares no sha256 — run \
                         `experiment-harness stamp-inputs --record {file}`"
                    )
                })
                .to_string();
            inputs.push(Input {
                unit: unit.clone(),
                path,
                role: i["role"].as_str().expect("input role").to_string(),
                sha256,
            });
        }
        let declared_replays = doc["replays"]
            .as_array()
            .unwrap_or_else(|| panic!("{file} declares no replays"));

        // A record that pins no revision must not describe where a pin came
        // from, and a record that pins one must. Round 953: the alternative was
        // to make `factsfirst-craft` name a provenance for a revision it does
        // not have, which is a field asserting something about nothing.
        if declared_replays.is_empty() {
            assert!(
                doc.get("revision_provenance").is_none(),
                "{file} declares no replays, so `revision_provenance` describes \
                 no pin — remove it"
            );
            let why = doc["no_replay"].as_str().unwrap_or_else(|| {
                panic!(
                    "{file} declares no replays and does not say why. A kit that \
                     is silently exempt from the replay half is the one thing \
                     nothing else here would notice."
                )
            });
            assert!(
                why.len() > 30,
                "{file}: `no_replay` must name what cannot be replayed, not just \
                 say so: {why:?}"
            );
            no_replay.insert(unit.clone(), why.to_string());
        } else {
            assert!(
                doc.get("no_replay").is_none(),
                "{file} declares {} replay(s) AND a `no_replay` reason — one of \
                 the two is a lie",
                declared_replays.len()
            );
            let prov = doc["revision_provenance"]
                .as_str()
                .unwrap_or_else(|| panic!("{file} declares no revision_provenance"))
                .to_string();
            assert!(
                vocabulary("revision_provenance").contains(&prov.as_str()),
                "{file}: unknown revision_provenance `{prov}` — a record whose pin \
                 does not say where it came from cannot be weighed"
            );
            provenance.insert(unit.clone(), prov);
        }

        for r in declared_replays {
            let steps = r["steps"]
                .as_array()
                .unwrap_or_else(|| panic!("{file}: a replay declares no steps"))
                .iter()
                .map(|s| Step {
                    verb: s["verb"].as_str().expect("step verb").to_string(),
                    input: s["input"].as_str().expect("step input").to_string(),
                    expect: s["expect"].as_str().unwrap_or("apply").to_string(),
                })
                .collect();
            replays.push(Replay {
                unit: unit.clone(),
                name: r["name"].as_str().expect("replay name").to_string(),
                revision: r["revision"].as_str().expect("replay revision").to_string(),
                blocked: r["blocked"].as_str().map(str::to_string),
                digest: r["expected_store_sha256"].as_str().map(str::to_string),
                config: r["config"].as_str().map(str::to_string),
                steps,
            });
        }
    }
    Declarations {
        inputs,
        replays,
        provenance,
        no_replay,
    }
}

/// Every tracked file a mutate verb would accept is declared exactly once, and
/// every declaration points at a file that is there.
///
/// The coverage direction is the load-bearing one, and it is why the oracle had
/// to stop being a filename list: R882's hand-list said "covered" while seven
/// classified inputs sat undeclared, because it only looked where it had been
/// told to look. Asking the parsers instead means a kit cannot hide an input by
/// naming it something new.
#[test]
fn every_input_a_verb_would_accept_is_declared_exactly_once() {
    let root = repo_root();
    let d = declarations();
    assert!(
        !d.inputs.is_empty(),
        "no input declarations found at all — this gate would pass vacuously"
    );

    let mut declared: BTreeSet<String> = BTreeSet::new();
    let mut declared_replayable: BTreeSet<String> = BTreeSet::new();
    for i in &d.inputs {
        assert!(
            vocabulary("role").contains(&i.role.as_str()),
            "{}: unknown input role `{}`",
            i.unit,
            i.role
        );
        let path = normalize(&format!("{}/{}", i.unit, i.path));
        assert!(
            root.join(&path).is_file(),
            "{}: declares an input that is not there: {path}",
            i.unit
        );
        assert!(
            declared.insert(path.clone()),
            "{path} is declared by more than one replay unit"
        );
        if i.role == "replay-input" {
            declared_replayable.insert(path);
        }
    }

    let classified = classified_inputs();
    let present: BTreeSet<String> = classified.keys().cloned().collect();
    let missing: Vec<String> = present
        .difference(&declared)
        .map(|p| format!("{p} ({:?})", classified[p].0))
        .collect();
    assert!(
        missing.is_empty(),
        "tracked inputs shaped like a mutate verb's, with no declaration — the \
         class this gate exists to close: {missing:#?}"
    );
    // Only `replay-input` has to be shaped like something a verb takes.
    // `raw-agent-output` frequently is NOT, and that is the whole reason a
    // normalized sibling exists: `2d-projection`'s extractor wrote its sections
    // with `id` / `branch` where the store wants `section_id` / `parent_doc`.
    // Demanding that the raw form be importable would deny the record the one
    // thing it is there to say.
    let phantom: Vec<&String> = declared_replayable.difference(&present).collect();
    assert!(
        phantom.is_empty(),
        "inputs declared `replay-input` that are shaped like no verb's input — \
         either the file changed shape or the role is wrong: {phantom:#?}"
    );

    // Reported, never asserted on: how much of the corpus today's parser can no
    // longer read is a MEASUREMENT of where the tool has moved since (R878
    // counted it once by hand), not a property of the record. A threshold here
    // would either freeze the corpus or quietly rot.
    let stale: Vec<&String> = present.iter().filter(|p| !classified[*p].1).collect();
    println!(
        "{} classified inputs, all declared; {} no longer parse with today's \
         binary and are readable only at their pinned revision",
        present.len(),
        stale.len()
    );
    for p in stale {
        println!("  stale: {p}");
    }
}

/// Every declared revision is a real commit, and a DERIVED one still equals what
/// the derivation yields. A pin that has quietly drifted from history would send
/// a future re-check to the wrong tree and look fine doing it.
///
/// The derivation is the newest add-commit across ALL of a replay's inputs, not
/// just its facts manifest (R882's rule). A replay needs a tree that holds every
/// input it feeds, so the max is the earliest revision at which the replay could
/// run at all — the pair rule was a special case that happened to agree.
#[test]
fn every_declared_revision_resolves_and_derived_ones_still_match() {
    // A shallow clone holds none of the pinned commits, so every assertion
    // below would fail with `revision does not resolve` and send the reader
    // hunting for a corrupt declaration. R889 spent that hunt: name the cause
    // instead. Failing — rather than skipping — is deliberate; a gate that
    // quietly stops asserting where the history is thin is worse than a red one.
    assert_eq!(
        git(&["rev-parse", "--is-shallow-repository"]).trim(),
        "false",
        "this is a SHALLOW clone, so no pinned revision can be resolved and \
         these declarations cannot be checked here. Whatever runs this test \
         must check out full history (`fetch-depth: 0` for actions/checkout)."
    );
    let d = declarations();
    assert!(!d.replays.is_empty(), "no replays — nothing asserted");
    let mut checked_derived = 0usize;
    for r in &d.replays {
        assert_eq!(
            r.revision.len(),
            40,
            "{}/{}: not a full sha: {}",
            r.unit,
            r.name,
            r.revision
        );
        let ok = Command::new("git")
            .args(["cat-file", "-e", &format!("{}^{{commit}}", r.revision)])
            .current_dir(repo_root())
            .output()
            .expect("git exec")
            .status
            .success();
        assert!(
            ok,
            "{}/{}: revision does not resolve: {}",
            r.unit, r.name, r.revision
        );

        if d.provenance[&r.unit] == "derived-upper-bound" {
            let paths: Vec<String> = r
                .steps
                .iter()
                .map(|s| normalize(&format!("{}/{}", r.unit, s.input)))
                .collect();
            let mut args: Vec<&str> = vec!["log", "--diff-filter=A", "--format=%H", "-1", "--"];
            args.extend(paths.iter().map(String::as_str));
            let derived = git(&args).trim().to_string();
            assert_eq!(
                derived,
                r.revision,
                "{}/{}: the declared pin no longer matches what the derivation \
                 yields over its {} input(s) — one of the two is wrong and a \
                 re-check would use the wrong tree",
                r.unit,
                r.name,
                paths.len()
            );
            checked_derived += 1;
        }
    }
    // Non-vacuity: the derivation comparison is the substantive half, so at
    // least one replay must actually have taken it.
    assert!(
        checked_derived > 0,
        "no derived pin was re-derived — the comparison above ran on nothing"
    );
    println!("{} replays, {checked_derived} re-derived", d.replays.len());
}

/// Every declared input still hashes to the digest recorded when it landed.
///
/// THE FREEZE WAS AN INSTRUCTION UNTIL THIS TEST. A kit's `deterministic_pins`
/// is a pre-committed claim about what a BLIND AUTHOR produced, and the R469
/// contamination bound forbids editing that output until it suits a later tool
/// — but the only thing that could detect such an edit was
/// `expected_store_sha256`, which covers what a replay step FEEDS. Everything
/// else the record declares was pinned by nobody: the frozen first submissions,
/// the sealed self-reports, and — Round 951 — the authored `rules.json` that is
/// the map half of both map-declaring corpora and was not even declared. Round
/// 943 wrote the same sentence about the self-report seal one axis over: a rule
/// nobody can check is a backup, not a seal.
///
/// The digest is written ONCE, by `experiment-harness stamp-inputs`, which
/// refuses to overwrite one that is already there. A tool that re-sealed on
/// demand would let any later edit launder itself, and this gate would then be
/// asserting that the evidence equals whatever the evidence currently is.
#[test]
fn every_declared_input_still_hashes_to_its_sealed_digest() {
    let root = repo_root();
    let d = declarations();
    assert!(
        !d.inputs.is_empty(),
        "no inputs declared at all — this gate would pass vacuously"
    );

    let mut by_role: BTreeMap<&str, usize> = BTreeMap::new();
    for i in &d.inputs {
        let path = normalize(&format!("{}/{}", i.unit, i.path));
        let bytes = std::fs::read(root.join(&path))
            .unwrap_or_else(|e| panic!("{}: cannot read declared input {path}: {e}", i.unit));
        let computed = mnemosyne_core::sha256_hex(&bytes);
        assert_eq!(
            computed, i.sha256,
            "{path} no longer hashes to the digest sealed with it. If the \
             evidence was edited, that is the R469 bound broken and the edit is \
             what has to be undone — not this digest."
        );
        *by_role.entry(i.role.as_str()).or_default() += 1;
    }
    println!(
        "{} declared input(s) still sealed: {by_role:?}",
        d.inputs.len()
    );
}

/// Every tracked file under a kit's `run/` tree is declared by the record that
/// owns it, and every `run/` tree has a record to own it.
///
/// THE SEAL IS ONLY AS WIDE AS THE DECLARATION. Round 952 gave every declared
/// input a digest a gate re-checks, and then measured how far that reached: 552
/// tracked files live under kit `run/` trees and 142 were declared. The other
/// 410 — the manuscripts the judges read, the judges' own reports, the label map
/// that made them blind, the briefs the authors were handed, the logs the run
/// emitted — were pinned by nothing at all, which is the state Round 952
/// described as the mechanism starting rather than finishing.
///
/// Coverage is asserted from the FILESYSTEM SIDE, not from the record's. A gate
/// that walked the declarations and checked they resolve would pass on a record
/// declaring one file out of fifty; the question worth asking is the other one,
/// and it is the R884 lesson restated (a hand-list only looks where it was told
/// to look).
#[test]
fn every_tracked_run_artifact_is_declared_by_its_kit() {
    let d = declarations();
    let units: BTreeSet<String> = tracked_evidence_files()
        .into_iter()
        .filter(|f| f.ends_with("/replay.json"))
        .map(|f| f.trim_end_matches("/replay.json").to_string())
        .collect();
    let declared: BTreeSet<String> = d
        .inputs
        .iter()
        .map(|i| normalize(&format!("{}/{}", i.unit, i.path)))
        .collect();

    let artifacts = run_artifacts();
    assert!(
        !artifacts.is_empty(),
        "no run artifacts found at all — this gate would pass vacuously"
    );

    // One defect with two remedies, reported TOGETHER. An artifact under a
    // record can be declared in it; an artifact under none needs the record
    // first. Splitting them into two assertions would report the first
    // shortfall and hide the second behind it, which teaches a reader the gap
    // is smaller than it is — the Round 899 failure, one axis over.
    let mut unowned: Vec<String> = Vec::new();
    let mut undeclared: Vec<String> = Vec::new();
    for path in &artifacts {
        match owning_unit(path, &units) {
            None => unowned.push(path.clone()),
            Some(_) if !declared.contains(path) => undeclared.push(path.clone()),
            Some(_) => {}
        }
    }
    assert!(
        unowned.is_empty() && undeclared.is_empty(),
        "{} of {} tracked run artifact(s) are outside the Round 952 seal.\n\
         {} sit under no kit record at all, so nothing can declare them — write \
         that kit's `replay.json` first: {unowned:#?}\n\
         {} have an owning record that does not declare them; run \
         `experiment-harness declare-run-tree --record <replay.json>` and then \
         `stamp-inputs` on the same record: {undeclared:#?}",
        unowned.len() + undeclared.len(),
        artifacts.len(),
        unowned.len(),
        undeclared.len()
    );
    println!(
        "{} tracked run artifact(s) across {} record(s), all declared",
        artifacts.len(),
        units.len()
    );
}

/// A manifest the record marks as raw agent output is NOT a replay input. The
/// distinction is recorded because two kits ship both their agent's raw output
/// and the normalized form, and a census that could not tell them apart
/// reported a working arm as broken.
#[test]
fn roles_are_from_the_declared_vocabulary_and_every_one_is_used() {
    let d = declarations();
    let roles: BTreeSet<&str> = d.inputs.iter().map(|i| i.role.as_str()).collect();
    // Asked of the vocabulary, not of a hand-typed pair: a role added to
    // `INPUT_ROLES` and then used nowhere is a distinction the corpus does not
    // actually draw, and this is the test that says so.
    for role in INPUT_ROLES {
        assert!(
            roles.contains(role),
            "role `{role}` is in the vocabulary and used by no record — an \
             unexercised distinction is untested: {roles:?}"
        );
    }

    // A role is a claim about use, so it has to agree with the steps: anything
    // a replay feeds is a replay input, and anything marked raw is fed by none.
    let fed: BTreeSet<String> = d
        .replays
        .iter()
        .flat_map(|r| {
            r.steps
                .iter()
                .map(move |s| normalize(&format!("{}/{}", r.unit, s.input)))
        })
        .collect();
    for i in &d.inputs {
        let path = normalize(&format!("{}/{}", i.unit, i.path));
        match i.role.as_str() {
            "replay-input" => assert!(
                fed.contains(&path),
                "{path} is declared `replay-input` but no replay step feeds it"
            ),
            "raw-agent-output" => assert!(
                !fed.contains(&path),
                "{path} is declared `raw-agent-output` but a replay step feeds it"
            ),
            // The constraint neither sibling carries, and the reason
            // `run-artifact` is not a catch-all: it claims membership of the
            // kit's run tree, so it may not be used anywhere else. A file
            // outside `run/` has to say what it IS.
            "run-artifact" => {
                assert!(
                    !fed.contains(&path),
                    "{path} is declared `run-artifact` but a replay step feeds \
                     it — a fed input is a `replay-input`"
                );
                // Beneath the unit, and beneath a `run/` — not necessarily the
                // unit's own `run/`, because kits nest: `disclosure-craft`
                // holds the record and `v3/run/` is one of the trees it owns.
                assert!(
                    path.starts_with(&format!("{}/", i.unit)) && path.contains("/run/"),
                    "{path} is declared `run-artifact` but does not sit under a \
                     run tree of {}",
                    i.unit
                );
            }
            other => panic!("unknown role `{other}`"),
        }
    }
}

/// A kit that declares no replay says why, and a kit that declares one does not
/// pretend it also has no replay.
///
/// The check lives in `declarations`, which every test here calls; this one
/// exists to state the rule where a reader looks for it and to keep the
/// replay-less case from becoming vacuous. If the corpus ever holds no
/// replay-less kit, the rule above is untested and this says so rather than
/// passing quietly.
#[test]
fn a_kit_with_no_replay_states_why_and_declares_no_pin() {
    let d = declarations();
    assert!(
        !d.no_replay.is_empty(),
        "no kit declares an empty `replays`, so the rule that such a kit must \
         explain itself is asserted against nothing"
    );
    for unit in d.no_replay.keys() {
        assert!(
            !d.replays.iter().any(|r| &r.unit == unit),
            "{unit} says it has no replay and contributed one"
        );
        assert!(
            !d.provenance.contains_key(unit),
            "{unit} says it has no replay and still declares a revision provenance"
        );
    }
    println!(
        "{} kit(s) declare no replay, each with a stated reason: {:?}",
        d.no_replay.len(),
        d.no_replay.keys().collect::<Vec<_>>()
    );
}

/// Every verb a step names is a verb this CLI actually has. Asked of the binary,
/// so a renamed or removed verb breaks the record loudly instead of waiting for
/// someone to run a replay — the R875 class, where a datum sat in a side file
/// that nothing loaded.
#[test]
fn every_step_verb_is_one_the_cli_has() {
    let help = Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
        .arg("--help")
        .current_dir(repo_root())
        .output()
        .expect("cli exec");
    assert!(help.status.success(), "--help failed");
    let help = String::from_utf8(help.stdout).expect("help is utf-8");

    let d = declarations();
    let verbs: BTreeSet<&str> = d
        .replays
        .iter()
        .flat_map(|r| r.steps.iter().map(|s| s.verb.as_str()))
        .collect();
    assert!(!verbs.is_empty(), "no verbs declared — nothing asserted");
    for v in &verbs {
        assert!(
            help.contains(&format!("mnemosyne-cli {v} ")),
            "step verb `{v}` is not in this CLI's usage — the record names a \
             verb that no longer exists"
        );
    }
    println!(
        "{} distinct step verbs, all present: {verbs:?}",
        verbs.len()
    );
}

/// A replay is an ordered sequence over its own declared inputs, and it starts
/// by creating the sections its facts will cite. The order is the thing the
/// pair schema could not say: `disclosure-craft` adds facts, then types them,
/// then draws edges between them, and every one of those steps rejects if run
/// before the one above it.
#[test]
fn replays_are_ordered_sequences_over_their_own_inputs() {
    let d = declarations();
    let classified = classified_inputs();
    let declared_by_unit: BTreeSet<(String, String)> = d
        .inputs
        .iter()
        .map(|i| (i.unit.clone(), normalize(&format!("{}/{}", i.unit, i.path))))
        .collect();

    let mut blocked = 0usize;
    for r in &d.replays {
        assert!(!r.steps.is_empty(), "{}/{}: no steps", r.unit, r.name);
        for s in &r.steps {
            assert!(
                vocabulary("expect").contains(&s.expect.as_str()),
                "{}/{}: unknown expectation `{}`",
                r.unit,
                r.name,
                s.expect
            );
            let path = normalize(&format!("{}/{}", r.unit, s.input));
            assert!(
                declared_by_unit.contains(&(r.unit.clone(), path.clone())),
                "{}/{}: step feeds {path}, which this unit does not declare",
                r.unit,
                r.name
            );
        }
        // A fact cites a section by id, and the store rejects a citation to a
        // section it does not hold — so the first step of a replay is always
        // the one that creates them. This is the store's rule, restated as the
        // shape of the record rather than discovered again at replay time.
        let first = &r.steps[0];
        let first_path = normalize(&format!("{}/{}", r.unit, first.input));
        assert_eq!(
            classified.get(&first_path).map(|c| c.0),
            Some(InputShape::Sections),
            "{}/{}: first step feeds {first_path}, which is not a sections \
             manifest — facts cannot land before the sections they cite",
            r.unit,
            r.name
        );
        assert_eq!(
            first.verb, "import-sections",
            "{}/{}: first step is not import-sections",
            r.unit, r.name
        );

        if let Some(cfg) = &r.config {
            let path = normalize(&format!("{}/{}", r.unit, cfg));
            assert!(
                path.ends_with("/mnemosyne.toml"),
                "{}/{}: a replay's config is a mnemosyne.toml, not {path}",
                r.unit,
                r.name
            );
            assert!(
                tracked_evidence_files().contains(&path),
                "{}/{}: declares an untracked config: {path}",
                r.unit,
                r.name
            );
        }

        if let Some(reason) = &r.blocked {
            assert!(
                reason.len() > 30,
                "{}/{}: `blocked` must name what is missing, not just say so: \
                 {reason:?}",
                r.unit,
                r.name
            );
            blocked += 1;
        }
    }
    println!(
        "{} replays, {blocked} blocked on an unrecorded step",
        d.replays.len()
    );
}

/// Read a manifest as JSON without asking today's parser to accept it — 39 of
/// the 81 classified inputs predate the R708 shape removal, and the order they
/// were fed in is a property of the bytes, not of whether the current binary
/// still reads them.
fn read_json(root: &Path, path: &str) -> serde_json::Value {
    let raw =
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{path} is not JSON: {e}"))
}

fn ids(v: &serde_json::Value, array: &str, field: &str) -> Vec<String> {
    v.get(array)
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|e| e.get(field).and_then(|s| s.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The declared order is not decoration: every step cites ids an earlier step
/// created, and the store rejects a citation it cannot resolve. Walking the
/// steps and checking that each reference is already known is the same
/// reasoning that DERIVED the `disclosure-craft` order (its one `-facts-add`
/// fact is the target of a typing proposal and a succession edge, so it has to
/// land between the base facts and the proposals) — asserted here so the order
/// cannot be shuffled by a later editor without a red test, and without paying
/// for a full replay to find out.
///
/// A `reject` step contributes nothing: `propose-verdict` rolls back, so its
/// facts must resolve their citations but must NOT become visible to the steps
/// after it.
#[test]
fn every_step_cites_only_what_an_earlier_step_created() {
    let root = repo_root();
    let d = declarations();
    let mut checked = 0usize;
    for r in &d.replays {
        let mut sections: BTreeSet<String> = BTreeSet::new();
        let mut facts: BTreeSet<String> = BTreeSet::new();
        for (n, s) in r.steps.iter().enumerate() {
            let path = normalize(&format!("{}/{}", r.unit, s.input));
            let doc = read_json(&root, &path);
            let where_ = format!("{}/{} step {n} ({})", r.unit, r.name, s.input);
            match s.verb.as_str() {
                "import-sections" => {
                    for id in doc
                        .as_array()
                        .unwrap_or_else(|| panic!("{where_}: not a JSON array"))
                        .iter()
                        .filter_map(|e| e.get("section_id").and_then(|v| v.as_str()))
                    {
                        sections.insert(id.to_string());
                    }
                }
                "import-facts" | "propose-verdict" => {
                    let entries = doc
                        .get("facts")
                        .and_then(|a| a.as_array())
                        .cloned()
                        .unwrap_or_default();
                    // One manifest is ONE atomic transaction, so a fact may
                    // supersede one declared further down the same file — the
                    // import primitive says so, and `2d-projection`'s author
                    // arm uses it. Only references ACROSS steps are ordered.
                    let mut visible = facts.clone();
                    visible.extend(
                        entries
                            .iter()
                            .filter_map(|f| f.get("fact_id").and_then(|v| v.as_str()))
                            .map(str::to_string),
                    );
                    for f in &entries {
                        for key in ["canon_from", "canon_to"] {
                            if let Some(cite) = f.get(key).and_then(|v| v.as_str()) {
                                assert!(
                                    sections.contains(cite),
                                    "{where_}: fact cites section `{cite}`, which no \
                                     earlier step created"
                                );
                                checked += 1;
                            }
                        }
                        if let Some(sup) = f.get("supersedes_in_frame").and_then(|v| v.as_str()) {
                            assert!(
                                visible.contains(sup),
                                "{where_}: fact supersedes `{sup}`, which neither this \
                                 manifest nor an earlier step creates"
                            );
                            checked += 1;
                        }
                    }
                    // A `reject` step leaves nothing behind — `propose-verdict`
                    // rolls back — so its ids must not become visible to the
                    // steps after it.
                    if s.expect == "apply" {
                        facts = visible;
                    }
                }
                "import-typing-proposals" => {
                    for target in ids(&doc, "proposals", "fact") {
                        assert!(
                            facts.contains(&target),
                            "{where_}: types fact `{target}`, which no earlier step \
                             created"
                        );
                        checked += 1;
                    }
                }
                "import-edge-proposals" => {
                    let refs = [
                        ids(&doc, "succession", "predecessor"),
                        ids(&doc, "succession", "successor"),
                        ids(&doc, "conflicts", "fact"),
                        ids(&doc, "conflicts", "target"),
                    ];
                    for target in refs.iter().flatten() {
                        assert!(
                            facts.contains(target),
                            "{where_}: draws an edge at `{target}`, which no earlier \
                             step created"
                        );
                        checked += 1;
                    }
                }
                other => panic!("{where_}: no reference rule for verb `{other}`"),
            }
        }
    }
    // Non-vacuity: a corpus this walk found nothing to check in would pass while
    // asserting nothing at all.
    assert!(
        checked > 0,
        "no citation was resolved — the walk ran on nothing"
    );
    println!("{checked} citations resolved against earlier steps");
}

// ---------------------------------------------------------------------------
// Round 885 — the third of the triple: the EXPECTED OUTPUT.
//
// Everything above is cheap and always on: it reads declarations. This half
// runs them. For each declared revision it extracts that tree, builds the CLI
// there, and feeds the untouched inputs through the declared steps in order —
// then does it a second time and requires the two stores to be byte-identical
// before believing either. The resulting hash is written back into the record,
// so from here a re-check is a comparison rather than a judgement call.
//
// R883 established the facts this rests on by hand: 23 of 23 revisions build,
// and every replay that ran was deterministic across two processes. What it
// could NOT do was record the result, because the schema had no place to put a
// sequence, and writing digests for the units it could express would have
// claimed a completeness the corpus did not have. R884 gave the record that
// place. This closes it.
//
// Ignored by default — it compiles 23 workspaces. That is the honest cost of
// the design, and hiding it inside `cargo test` would make every unrelated run
// pay it. Run explicitly:
//
// ```text
// cargo test -p mnemosyne-cli --test evidence_replay_smoke -- --ignored --nocapture
// ```

use std::io::Write;
use tempfile::TempDir;

/// Which flag each verb takes its input under. A small closed map, and the
/// verbs in it are the same set `every_step_verb_is_one_the_cli_has` checks
/// against the binary's own usage — so a verb that vanishes is caught by the
/// cheap test, and a verb whose FLAG changed is caught here, loudly, by the
/// step failing.
fn input_flag(verb: &str) -> &'static str {
    match verb {
        "import-sections" | "import-facts" | "propose-verdict" => "--manifest",
        "import-typing-proposals" | "import-edge-proposals" => "--proposals",
        other => panic!("no input flag known for verb `{other}`"),
    }
}

/// An empty workspace the CLI will accept: the config it looks for in CWD or an
/// ancestor, and a store to load. Same seed R880 used, which is why the two
/// tests' results are comparable.
fn seed_workspace(ws: &Path) {
    std::fs::create_dir_all(ws.join("docs/.atomic")).expect("mkdir");
    std::fs::write(ws.join("mnemosyne.toml"), "[workspace]\n").expect("config");
    std::fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        "{\"schema_version\": 1, \"sections\": {}, \"changelog_entries\": {}}\n",
    )
    .expect("seed store");
}

/// Extract a revision with `git archive` — NOT a worktree, which would register
/// state in `.git` and leak if this panicked — and build its CLI into a target
/// directory of its own.
fn build_revision(root: &Path, rev: &str) -> (TempDir, TempDir, PathBuf) {
    let tree = TempDir::new().expect("tempdir");
    let archive = Command::new("git")
        .args(["archive", rev])
        .current_dir(root)
        .output()
        .expect("git archive");
    assert!(archive.status.success(), "git archive {rev} failed");
    let mut tar = Command::new("tar")
        .args(["-x", "-C", tree.path().to_str().expect("utf-8 path")])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("tar spawn");
    tar.stdin
        .as_mut()
        .expect("tar stdin")
        .write_all(&archive.stdout)
        .expect("write archive");
    assert!(
        tar.wait().expect("tar wait").success(),
        "tar extract failed"
    );

    let target = TempDir::new().expect("target tempdir");
    let build = Command::new("cargo")
        .args(["build", "--bin", "mnemosyne-cli"])
        .current_dir(tree.path())
        .env("CARGO_TARGET_DIR", target.path())
        .output()
        .expect("cargo exec");
    assert!(
        build.status.success(),
        "revision {rev} no longer builds — THIS is the finding, and it kills \
         the pin-the-revision design for every replay that names it:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let cli = target.path().join("debug/mnemosyne-cli");
    assert!(cli.is_file(), "no binary at {}", cli.display());
    (tree, target, cli)
}

/// Run one replay to completion against `cli`, returning the sha256 of the
/// store it built, or the first step that did not do what the record says it
/// should.
fn run_replay(cli: &Path, root: &Path, tree: &Path, r: &Replay) -> Result<String, String> {
    let ws = TempDir::new().expect("ws tempdir");
    seed_workspace(ws.path());
    // A declared config is copied in WITH the rest of its directory, because the
    // paths inside it are relative to where it sits — which is where the run's
    // CWD was. Reading the toml to chase those paths would be a second parser
    // free to disagree with the config crate's.
    if let Some(cfg) = &r.config {
        let dir = normalize(&format!("{}/{}", r.unit, cfg))
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .expect("a config path has a directory");
        for f in tracked_evidence_files()
            .into_iter()
            .filter(|f| f.rsplit_once('/').map(|(d, _)| d) == Some(dir.as_str()))
        {
            let name = f.rsplit_once('/').expect("has a name").1;
            let then = std::fs::read(tree.join(&f))
                .map_err(|e| format!("config dir file {f} is not in the pinned tree: {e}"))?;
            if then != std::fs::read(root.join(&f)).expect("config dir file today") {
                return Err(format!(
                    "{f} differs between {} and today — the replay would run \
                     under a configuration the record does not show",
                    r.revision
                ));
            }
            std::fs::write(ws.path().join(name), &then).expect("copy config dir file");
        }
        assert!(
            ws.path().join("mnemosyne.toml").is_file(),
            "{}/{}: the declared config did not land in the workspace",
            r.unit,
            r.name
        );
    }
    for (n, s) in r.steps.iter().enumerate() {
        let rel = normalize(&format!("{}/{}", r.unit, s.input));

        // The record has not moved since it was authored. Without this the run
        // below could be reading a later edit and would report it as the
        // original's result — the exact failure R883 found in its own corpus.
        let then = std::fs::read(tree.join(&rel))
            .map_err(|e| format!("step {n}: {rel} is not in the pinned tree: {e}"))?;
        let now = std::fs::read(root.join(&rel)).expect("input today");
        if then != now {
            return Err(format!(
                "step {n}: {rel} differs between {} and today — this replay \
                 would not be reading the original",
                r.revision
            ));
        }

        let out = Command::new(cli)
            .args([
                &s.verb,
                input_flag(&s.verb),
                root.join(&rel).to_str().expect("utf-8 path"),
            ])
            .current_dir(ws.path())
            .output()
            .expect("cli exec");
        let ok = out.status.success();
        match (s.expect.as_str(), ok) {
            ("apply", false) => {
                return Err(format!(
                    "step {n} ({} {rel}) was rejected:\n{}",
                    s.verb,
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            ("reject", true) => {
                return Err(format!(
                    "step {n} ({} {rel}) was APPLIED, and the record says it must \
                     be rolled back — the negative control has stopped controlling",
                    s.verb
                ));
            }
            _ => {}
        }
    }
    let store = std::fs::read(ws.path().join("docs/.atomic/workspace.atomic.json"))
        .expect("the store the replay built");
    Ok(mnemosyne_core::sha256_hex(&store))
}

/// Every replay that is not declared blocked runs at its pinned revision, twice,
/// and produces the store the record says it produces. Every replay that IS
/// declared blocked actually fails there — a `blocked` note nobody re-checks
/// would go stale the moment the obstacle was removed, and would then be a
/// reason not to look.
#[test]
#[ignore = "extracts and compiles one workspace per declared revision; run with --ignored"]
fn every_replay_rebuilds_the_store_its_record_says_it_does() {
    let root = repo_root();
    let d = declarations();
    let mut revisions: Vec<String> = d.replays.iter().map(|r| r.revision.clone()).collect();
    revisions.sort();
    revisions.dedup();
    println!(
        "{} replays across {} revisions",
        d.replays.len(),
        revisions.len()
    );

    let mut ran = 0usize;
    let mut blocked_confirmed = 0usize;
    let mut undeclared: Vec<(String, String)> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    for rev in &revisions {
        let (tree, _target, cli) = build_revision(&root, rev);
        for r in d.replays.iter().filter(|r| &r.revision == rev) {
            let name = format!("{}/{}", r.unit, r.name);
            let first = run_replay(&cli, &root, tree.path(), r);
            match (&r.blocked, first) {
                (Some(_), Err(why)) => {
                    println!(
                        "blocked, confirmed: {name}\n    {}",
                        why.lines().next().unwrap_or("")
                    );
                    blocked_confirmed += 1;
                }
                (Some(reason), Ok(_)) => wrong.push(format!(
                    "{name}: declared BLOCKED but it ran — the note is stale and \
                     is telling readers not to look: {reason}"
                )),
                (None, Err(why)) => wrong.push(format!("{name}: {why}")),
                (None, Ok(sha)) => {
                    // Determinism is measured here, not inherited from R881's
                    // sample: a digest from a single run could pin a hash map's
                    // iteration order and reject the same evidence tomorrow.
                    let again = run_replay(&cli, &root, tree.path(), r)
                        .expect("the same replay failed on its second run");
                    if again != sha {
                        wrong.push(format!(
                            "{name}: two runs of the SAME replay at {rev} disagree \
                             ({sha} vs {again}) — nothing here can be pinned"
                        ));
                        continue;
                    }
                    match &r.digest {
                        Some(declared) if declared == &sha => ran += 1,
                        Some(declared) => wrong.push(format!(
                            "{name}: record says {declared}, replay produced {sha}"
                        )),
                        None => {
                            undeclared.push((name.clone(), sha.clone()));
                            println!("undeclared digest: {name} -> {sha}");
                        }
                    }
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "{} replay(s) disagree with the record:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    assert!(
        undeclared.is_empty(),
        "{} replay(s) reproduce deterministically but declare no digest — the \
         record is not yet holding what it measured:\n{}",
        undeclared.len(),
        undeclared
            .iter()
            .map(|(n, s)| format!("  {n} -> {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Non-vacuity: a corpus where everything was blocked, or where no digest was
    // declared, would satisfy every assertion above while proving nothing ran.
    assert!(
        ran > 0,
        "no replay reproduced a declared digest — this test asserted nothing"
    );
    println!(
        "{ran} replays reproduced their declared digest, {blocked_confirmed} blocked as declared"
    );
}

// ---------------------------------------------------------------------------
// Round 887 — the workflow that runs all of the above is itself unwatched.
//
// R886 put the replay on GitHub and left this as a carry, on the grounds that
// pulling in a YAML parser for one assertion was a dependency decision worth
// making deliberately. That was deferring a real defect on a cost judgement,
// which is the one thing this repo's round discipline does not allow, and the
// defect is worse than "the file might not parse":
//
//   - R583 lost an unknown stretch of CI to a workflow that did not parse.
//     GitHub does not run it and does not say so anywhere the author looks.
//   - If `-- --ignored` is ever dropped from the replay step, the job runs the
//     cheap gates twice, goes GREEN, and replays nothing. A vacuous pass, in
//     the workflow built to stop evidence from rotting unnoticed.
//   - If a future kit is tracked outside `claudedocs/phase1-*`, the push
//     trigger's path filter silently stops covering it. Nothing else would
//     notice: the replay would still pass, just never run on the change.
//
// Substring matching cannot answer any of these — the R886 workflow names
// `fetch-depth: 0` in its own comments, so a text check passes on prose. Hence
// a parser, and hence this living beside the declarations: the path-filter
// check is a cross-check between the record and the CI config, and it needs
// both.

use yaml_rust2::{Yaml, YamlLoader};

const REPLAY_WORKFLOW: &str = ".github/workflows/evidence-replay.yml";

fn workflow_files() -> Vec<String> {
    let mut out: Vec<String> = git(&["ls-files", ".github/workflows"])
        .lines()
        .map(str::to_string)
        .collect();
    out.sort();
    out
}

fn load_workflow(path: &str) -> Yaml {
    let raw = std::fs::read_to_string(repo_root().join(path)).expect("read workflow");
    let docs = YamlLoader::load_from_str(&raw).unwrap_or_else(|e| {
        panic!("{path} is not parseable YAML — GitHub would silently not run it: {e}")
    });
    assert_eq!(docs.len(), 1, "{path}: expected exactly one YAML document");
    docs.into_iter().next().expect("one document")
}

/// A workflow's trigger block. YAML 1.1 reads a bare `on` as the boolean true
/// and YAML 1.2 as the string, so ask for both rather than assume which this
/// parser is.
fn trigger_block(doc: &Yaml) -> Option<&Yaml> {
    if !matches!(doc["on"], Yaml::BadValue) {
        return Some(&doc["on"]);
    }
    doc.as_hash().and_then(|h| h.get(&Yaml::Boolean(true)))
}

/// Every workflow this repo ships parses, and declares at least one trigger and
/// one job. An unparseable workflow is not a loud failure anywhere — it is an
/// absence, which is why it went unnoticed once already.
#[test]
fn every_shipped_workflow_parses_and_declares_work() {
    let files = workflow_files();
    assert!(!files.is_empty(), "no workflows found — nothing asserted");
    for path in &files {
        let doc = load_workflow(path);
        // YAML 1.1 reads a bare `on` as the boolean true and YAML 1.2 as the
        // string; accept whichever this parser produces rather than assuming.
        let triggers = trigger_block(&doc)
            .unwrap_or_else(|| panic!("{path}: declares no triggers, so it can never run"));
        assert!(
            triggers.as_hash().is_some_and(|h| !h.is_empty()),
            "{path}: trigger block is empty"
        );
        assert!(
            doc["jobs"].as_hash().is_some_and(|h| !h.is_empty()),
            "{path}: declares no jobs"
        );
    }
    println!("{} workflow(s) parse: {files:?}", files.len());
}

/// The replay workflow still does the thing it exists to do. Both halves are
/// load-bearing and both were measured, not assumed: a `--depth 1` clone
/// answers `git archive <pinned rev>` with exit 128, and without `--ignored`
/// the replay test is filtered out and the job passes having replayed nothing.
#[test]
fn the_replay_workflow_still_replays() {
    let doc = load_workflow(REPLAY_WORKFLOW);
    let steps = doc["jobs"]["replay"]["steps"]
        .as_vec()
        .expect("the replay job declares steps");

    let checkout = steps
        .iter()
        .find(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/checkout"))
        })
        .expect("the replay job checks the repo out");
    assert_eq!(
        checkout["with"]["fetch-depth"].as_i64(),
        Some(0),
        "the replay job must check out full history: it runs `git archive` on \
         23 historical commits, and a shallow clone answers `not a valid \
         object name` for every one of them"
    );

    let runs: Vec<&str> = steps.iter().filter_map(|s| s["run"].as_str()).collect();
    let replay = runs
        .iter()
        .find(|r| r.contains("--ignored"))
        .unwrap_or_else(|| {
            panic!(
                "no step runs the replay: without `-- --ignored` the heavy test \
                 is filtered out and the job goes green having replayed \
                 nothing. Steps: {runs:#?}"
            )
        });
    // Every test target this job names must exist — not just the replay step's.
    // A first cut checked only the step carrying `--ignored`, and an injection
    // that renamed the target in the OTHER step passed clean: the gate was
    // reading one of the two places a rename can land. R885's rename of this
    // very file is the move that would have done it.
    let mut named = 0usize;
    for step in &runs {
        let mut words = step.split_whitespace();
        while let Some(w) = words.next() {
            if w != "--test" {
                continue;
            }
            let target = words.next().expect("`--test` names a target");
            assert!(
                repo_root()
                    .join(format!("crates/mnemosyne-cli/tests/{target}.rs"))
                    .is_file(),
                "the workflow runs `--test {target}`, and there is no such test file"
            );
            named += 1;
        }
    }
    assert!(
        named > 0,
        "no step names a test target — the check above ran on nothing"
    );
    println!("replay step: `{replay}`; {named} test target(s) named and present");
}

/// Match one GitHub `paths:` pattern. Deliberately supports only the two shapes
/// this workflow uses, and panics on anything else: a matcher that quietly
/// mis-read a pattern it did not understand would report coverage it had not
/// checked, which is the failure this whole test is about.
fn path_filter_matches(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        if let Some((head, tail)) = prefix.split_once('*') {
            assert!(
                !tail.contains('*') && !tail.contains('/'),
                "unsupported paths pattern `{pattern}`"
            );
            return path.starts_with(head)
                && path[head.len()..]
                    .split_once('/')
                    .is_some_and(|(seg, rest)| seg.ends_with(tail) && !rest.is_empty());
        }
        return path.starts_with(&format!("{prefix}/"));
    }
    assert!(
        !pattern.contains('*'),
        "unsupported paths pattern `{pattern}`"
    );
    pattern == path
}

/// Every file the replay actually reads is covered by the push trigger's path
/// filter. A kit tracked outside the filter would still replay correctly and
/// would simply stop being re-checked when it changed — an absence again, with
/// nothing red to show for it.
#[test]
fn the_path_filter_covers_every_file_the_replay_reads() {
    let doc = load_workflow(REPLAY_WORKFLOW);
    let triggers = trigger_block(&doc).expect("the replay workflow declares triggers");
    let patterns: Vec<String> = triggers["push"]["paths"]
        .as_vec()
        .expect("the push trigger is path-filtered")
        .iter()
        .map(|p| p.as_str().expect("pattern is a string").to_string())
        .collect();
    assert!(!patterns.is_empty(), "empty path filter");

    let d = declarations();
    let mut read: BTreeSet<String> = BTreeSet::new();
    for r in &d.replays {
        for s in &r.steps {
            read.insert(normalize(&format!("{}/{}", r.unit, s.input)));
        }
        if let Some(cfg) = &r.config {
            read.insert(normalize(&format!("{}/{}", r.unit, cfg)));
        }
    }
    // The declarations themselves decide what runs, so they are read too.
    read.extend(
        tracked_evidence_files()
            .into_iter()
            .filter(|f| f.ends_with("/replay.json")),
    );
    assert!(!read.is_empty(), "nothing to cover — the check is vacuous");

    let uncovered: Vec<&String> = read
        .iter()
        .filter(|p| !patterns.iter().any(|pat| path_filter_matches(pat, p)))
        .collect();
    assert!(
        uncovered.is_empty(),
        "{} file(s) the replay reads are outside the workflow's path filter, so \
         changing them would not re-run it: {uncovered:#?}",
        uncovered.len()
    );
    println!(
        "{} files read by replays, all covered by {patterns:?}",
        read.len()
    );
}

/// Whether a `cargo test` command could run the tests in THIS file. Defaults to
/// yes: the question is answered from a command string, and the safe direction
/// is to demand full history for a job that might need it. Being wrong the other
/// way produces a gate that cannot run and says nothing, which is how R889
/// happened.
fn could_run_this_file(cmd: &str) -> bool {
    if !cmd.contains("cargo test") {
        return false;
    }
    let words: Vec<&str> = cmd.split_whitespace().collect();
    // An explicit `--test <other>` restricts the run to one target.
    if let Some(i) = words.iter().position(|w| *w == "--test") {
        if words.get(i + 1) != Some(&"evidence_replay_smoke") {
            return false;
        }
    }
    // An explicit `-p <other crate>` restricts it to one package.
    if let Some(i) = words.iter().position(|w| *w == "-p" || *w == "--package") {
        if words.get(i + 1) != Some(&"mnemosyne-cli") {
            return false;
        }
    }
    true
}

/// Any workflow job that could run the tests in this file must check the repo
/// out with full history, because they resolve pinned historical commits.
///
/// This is the check that would have caught R889 at commit time. R887 built the
/// same shape one axis over — the path filter has to cover what a replay reads —
/// and this is its sibling: the requirement the gates place on their runner has
/// to be asserted against the runner's own configuration, not remembered.
#[test]
fn every_job_that_could_run_these_gates_checks_out_full_history() {
    let mut checked = 0usize;
    for path in workflow_files() {
        let doc = load_workflow(&path);
        for (name, job) in doc["jobs"].as_hash().expect("jobs is a mapping") {
            let name = name.as_str().unwrap_or("<unnamed>");
            let steps = job["steps"].as_vec().expect("a job declares steps");
            let runs_them = steps
                .iter()
                .filter_map(|s| s["run"].as_str())
                .any(could_run_this_file);
            if !runs_them {
                continue;
            }
            let depth = steps
                .iter()
                .find(|s| {
                    s["uses"]
                        .as_str()
                        .is_some_and(|u| u.starts_with("actions/checkout"))
                })
                .map(|s| s["with"]["fetch-depth"].as_i64());
            assert_eq!(
                depth,
                Some(Some(0)),
                "{path} job `{name}` runs these gates but does not check out \
                 full history (fetch-depth: {depth:?}). A shallow clone holds \
                 none of the pinned revisions, so the gates cannot run there."
            );
            checked += 1;
        }
    }
    // Non-vacuity: if no job were recognised as running these tests, the loop
    // above would assert nothing while looking thorough.
    assert!(
        checked > 0,
        "no workflow job was recognised as running these gates — either the \
         command shapes changed or `could_run_this_file` stopped matching"
    );
    println!("{checked} job(s) run these gates, all with full history");
}

/// One `field: "value"` a text writes out — the shape a reader copies.
#[derive(Debug)]
struct Taught {
    line: usize,
    field: &'static str,
    value: String,
}

/// Every machine-checked literal a text writes in the copyable `field: "value"`
/// form.
///
/// THE FORM IS THE DISCRIMINATOR, and it is deliberately narrower than the
/// instruction-versus-explanation question Round 948 answered by reading. A
/// runbook may NAME a retired word — both of the ones that carried this defect
/// now do, recording what they used to say — but it may not write it in the
/// shape whose only use is to be pasted into a file. That is what actually
/// happened twice: nobody reasoned their way to `exact`, they copied it.
///
/// It is also why a kit's report is out of scope. The disclosed-place report
/// writes the retired value in exactly this form, on purpose, to record that it
/// does not exist — and evidence is frozen (fix the runbook, never the record,
/// the Round 934 rule), so a check that reached it could only be satisfied by
/// editing the thing it is supposed to preserve.
fn taught_literals(text: &str) -> Vec<Taught> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        for (field, _) in CHECKED_LITERALS {
            let field = *field;
            let mut from = 0usize;
            while let Some(at) = line[from..].find(field) {
                let start = from + at;
                from = start + field.len();
                // `first_role` is not `role`.
                if line[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    continue;
                }
                let Some(rest) = line[from..].strip_prefix(": \"") else {
                    continue;
                };
                let Some(end) = rest.find('"') else { continue };
                out.push(Taught {
                    line: n + 1,
                    field,
                    value: rest[..end].to_string(),
                });
            }
        }
    }
    out
}

/// Every runbook on disk, repo-relative, tracked or not.
///
/// Deliberately NOT `git ls-files`. `/claudedocs/*` is ignored by default, so a
/// runbook is untracked from the moment it is written until someone lands a
/// `.gitignore` exception — Round 898 found its own runbook in exactly that
/// state. Discovery by tracking would be blind for the whole window in which a
/// runbook is being read and copied from, which is the window this check is
/// for.
///
/// How many of them are tracked is NOT a number in this comment. It was one
/// until Round 960 ("23 runbooks on disk, 22 tracked"), it was stale within
/// eight rounds, and no program checked it — the shape Round 958 named. The
/// fact now has one home that cannot go stale:
/// `every_runbook_on_disk_is_in_the_repository` asserts that all of them are.
///
/// CI sees only the tracked ones, so this reaches further locally than there —
/// which is the right direction, because the writing moment is where catching
/// it is cheapest, and a file CI does not have cannot make CI red.
fn runbooks(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        let entries =
            std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|e| panic!("dir entry under {}: {e}", dir.display()));
            let kind = entry
                .file_type()
                .unwrap_or_else(|e| panic!("file type of {}: {e}", entry.path().display()));
            if kind.is_dir() {
                walk(&entry.path(), root, out);
            } else if kind.is_file() && entry.file_name() == "runbook.md" {
                let path = entry.path();
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or_else(|_| panic!("{} is outside the repo", path.display()));
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("claudedocs"), root, &mut out);
    out.sort();
    out
}

/// No runbook tells the next run to write a literal this gate would reject.
///
/// The carry Round 948 left after removing the retired word's third home: the
/// runbooks agreed with the gate on that one literal, and nothing checked that
/// they stayed that way. The word's FIRST home is the Round 898 changelog
/// entry, which the frozen ledger cannot fix and this test therefore cannot
/// cover — the design that seeds the copy is permanent, so the check has to sit
/// at the artifact the copy lands in.
#[test]
fn no_runbook_teaches_a_literal_its_own_gate_rejects() {
    let root = repo_root();
    let books = runbooks(&root);
    assert!(
        !books.is_empty(),
        "no runbooks found under claudedocs/ — this gate would pass vacuously"
    );
    let mut checked = 0usize;
    for rel in &books {
        let text =
            std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        for t in taught_literals(&text) {
            checked += 1;
            assert!(
                vocabulary(t.field).contains(&t.value.as_str()),
                "{rel}:{} instructs `{}: \"{}\"`, which this gate rejects — \
                 accepted: {:?}. A runbook is copied from, so a value the \
                 parser panics on is a run spent to reach a panic.",
                t.line,
                t.field,
                t.value,
                vocabulary(t.field)
            );
        }
    }
    assert!(
        checked > 0,
        "read {} runbook(s) and found no checked literal in any of them — the \
         scan asserted nothing",
        books.len()
    );
    println!(
        "{} runbook(s), {checked} checked literal(s), all in vocabulary",
        books.len()
    );
}

/// Every runbook the scan reads is IN the repository.
///
/// The sibling gate above walks the filesystem on purpose: a runbook is
/// untracked from the moment it is written, and that window is exactly when it
/// is copied from. What nothing asked until Round 960 is whether the window ever
/// CLOSES. Round 958's ledger entry says it FROZE the side-table protocol — the
/// file was never added, so what it froze lived in one working tree, and CI, a
/// fresh clone and any replay would have proceeded without it. Round 950 saw the
/// same state one kit over, counted it, printed it, and moved on; a count that
/// only prints is how a defect becomes a fixture.
///
/// REACH, stated rather than implied: in CI an untracked file does not exist, so
/// the walk finds only tracked runbooks and this passes VACUOUSLY there. It
/// bites where the defect is born — the tree that wrote the runbook, on the
/// `cargo test` a round already runs. CI is structurally unable to catch this
/// one, which is the argument for catching it at the writing moment rather than
/// an argument for a second gate that cannot see further.
#[test]
fn every_runbook_on_disk_is_in_the_repository() {
    let root = repo_root();
    let books = runbooks(&root);
    assert!(
        !books.is_empty(),
        "no runbooks found under claudedocs/ — this gate would pass vacuously"
    );
    let tracked: BTreeSet<String> = git(&["ls-files", "claudedocs"])
        .lines()
        .map(str::to_string)
        .collect();
    let missing: Vec<&str> = books
        .iter()
        .filter(|rel| !tracked.contains(*rel))
        .map(String::as_str)
        .collect();
    assert!(
        missing.is_empty(),
        "{} runbook(s) on disk are not in the repository: {missing:?}. \
         `/claudedocs/*` is ignored by default, so a new kit needs its \
         `.gitignore` exception landed IN THE ROUND THAT WRITES THE RUNBOOK — \
         until then the protocol a future run is told to follow exists in one \
         working tree only, and the round that says it froze the protocol has \
         frozen nothing.",
        missing.len()
    );
    println!("{} runbook(s) on disk, all tracked", books.len());
}

/// The scan can see every vocabulary the parser enforces, and does not fire on
/// a runbook that merely names a retired value.
///
/// Per row, because a table is exactly the thing that grows a cell the scanner
/// cannot reach and then reports zero violations for it. The probes are built
/// FROM the table, so a row is never checked against a hand-typed copy of
/// itself.
#[test]
fn the_runbook_scan_can_see_every_checked_vocabulary() {
    for (field, vocab) in CHECKED_LITERALS {
        assert!(
            !vocab.is_empty(),
            "`{field}` has an empty vocabulary — it would accept nothing"
        );
        let accepted = format!("- write `{field}: \"{}\"` into the record\n", vocab[0]);
        let found = taught_literals(&accepted);
        assert_eq!(
            found.len(),
            1,
            "`{field}`: the scan missed the form it exists to read: {found:?}"
        );
        assert_eq!(found[0].value, vocab[0]);
        assert!(vocabulary(field).contains(&found[0].value.as_str()));

        let rejected = format!("- write `{field}: \"{RETIRED_LITERAL}\"` into the record\n");
        let found = taught_literals(&rejected);
        assert_eq!(found.len(), 1, "`{field}`: the scan missed the bad form");
        assert!(
            !vocabulary(field).contains(&found[0].value.as_str()),
            "`{field}`: the probe value is IN the vocabulary, so the negative \
             half of this check tests nothing — pick another probe"
        );
    }

    // The word is not the defect; the copyable shape is. A runbook recording
    // that it used to ask for a retired value must stay green, which is what
    // both corrected runbooks now do.
    let naming = format!("This line said `\"{RETIRED_LITERAL}\"` until Round 948.\n");
    assert!(
        taught_literals(&naming).is_empty(),
        "the scan fires on a runbook that merely NAMES a retired value, which \
         would force the correction record out of the runbooks that carry it"
    );
}
