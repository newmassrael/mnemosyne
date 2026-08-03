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
///
/// `reproduced-output` is Round 973, and it is the answer to the vocabulary
/// question Round 953 left open — sharpened by what a MACHINE can settle rather
/// than by reading prose. Round 953 rejected orchestrator/judge/tool roles
/// because assigning them means inferring from a sampled file, and a record
/// whose value is that its claims are true may not carry inferred ones. That
/// objection does not reach a claim the tree can discharge: this role says the
/// bytes are what a named verb PRINTS at the record's pinned revision, and the
/// replay job runs that verb and compares. Nobody is believed; the claim is
/// earned every time the job runs.
///
/// It therefore carries the constraint the others cannot: a `reproduced_by`
/// verb from [`REPRODUCIBLE_VERBS`], and a unit that pins a revision to run it
/// at. Like `run-artifact` it lives under a run tree and no step feeds it — a
/// transcript is not an input to anything, it is what came back out.
const INPUT_ROLES: &[&str] = &[
    "replay-input",
    "raw-agent-output",
    "run-artifact",
    "reproduced-output",
];

/// The verbs whose output the replay job can regenerate and compare, each with
/// what it needs to run. A closed set, because a role that names a verb nothing
/// runs is a claim in the record with no check behind it — exactly the shape
/// this role exists to avoid.
///
/// The membership is measured, not chosen. Of the 108 tracked run artifacts
/// whose first line is a CLI banner (2026-08-03), these six verbs account for
/// 74. The other 34 cannot be reproduced by any single command and say so in
/// their own records: 14 `import-*` logs are a shell composition (two verbs,
/// with `exit=$?` appended by the runbook), and 20 playthrough manuscripts come
/// from `tools/experiment-harness`, a workspace this job does not build at a
/// pinned revision at all.
///
const REPRODUCIBLE_VERBS: &[&str] = &[
    "describe-schema",
    "report-transition-map",
    "report-authoring-frontier",
    "report-disclosure-coverage",
    "report-frame-view",
    "validate-continuity",
];

/// The subset of [`REPRODUCIBLE_VERBS`] that reads NO store — a marker over the
/// list above rather than a second copy of it, so a verb can never be in one
/// and missing from the other (`storeless_verbs_are_reproducible_verbs` holds
/// that).
///
/// One member, and it is what makes `describe-schema` the cheap case: it is a
/// pure function of the binary, so it runs in the extracted tree with no
/// replay behind it. Every other verb here reads the store a replay rebuilt,
/// so its record says WHICH replay, and it runs in that replay's workspace.
const STORELESS_VERBS: &[&str] = &["describe-schema"];

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
/// v3 was Round 953: the accepted document set grew a case v2 could not
/// describe. A kit whose artifacts no verb can import — `factsfirst-craft`
/// carries 51 of them and not one sections or facts manifest — has an empty
/// `replays`, and must then say WHY in `no_replay` and declare no revision
/// provenance, there being no pin for one to describe. v2 would have accepted
/// such a record silently, which would exempt a whole kit from the replay half
/// and leave nothing to notice it.
///
/// v4 is Round 973, for the same reason and not for a bigger one: an input may
/// now name the verb that reproduces it (`reproduced_by`), and a v3 reader has
/// no field to put that in. The version moves because the DOCUMENT changed
/// shape, so a record written against the old shape is told so instead of
/// having its new field ignored.
const REPLAY_SCHEMAS: &[&str] = &["kit-replay/v4"];

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
    ("reproduced_by", REPRODUCIBLE_VERBS),
];

/// The value the two runbooks taught until Rounds 944 and 948 removed it. Kept
/// as the probe for the scan's own liveness because it is the word that
/// actually recurred; the probes assert it is outside every vocabulary, so it
/// cannot quietly become valid and leave the negative half testing nothing.
const RETIRED_LITERAL: &str = "exact";

/// Number fields the PROGRAM owns, each paired with the constant that owns it.
/// A runbook may never write one of these next to its key.
///
/// This is the other half of the sibling table above, and it needs a different
/// rule rather than more rows. A word has a fixed accepted set, so a runbook can
/// hold the right one. A number does not: a kit is replayed at ITS OWN pinned
/// revision, and the constant there is whatever it was that day. Measured, not
/// argued — the four kits that seeded `schema_version: 23` pin revisions
/// (`9184e6f`, `d92e751`, `b8a3f3c`, `f488298`) whose constant IS 23, so what
/// they typed was right on the day and is wrong now. A typed number is correct
/// at exactly one revision; the recipe `describe-schema | sed …` is correct at
/// all of them. The rule here is therefore not "hold today's value" but "hold
/// no value" — and today's value is not written in this comment either, for the
/// same reason the runbooks may not write it: the cell below is the constant.
///
/// Keyed by FIELD, not by shape, and that is the precision the gate lives or
/// dies by. Measured over every field-adjacent number in the runbooks on disk:
/// `schema_version` is the only program-owned one. The rest — `violations: 0`,
/// `unplaced = 0`, `judges: 3`, `n=2`, `playthrough = 19` — are ORACLES, values
/// an arm asserts about its own run, and deriving one of those from the program
/// is exactly what would void it. A blanket "no numbers in a runbook" would fire
/// on every one of them and be switched off within a round. `the_number_scan_
/// sees_the_key_shape_and_spares_the_oracles` holds that line as a check rather
/// than as this sentence.
const PROGRAM_OWNED_NUMBERS: &[(&str, u32)] =
    &[("schema_version", mnemosyne_atomic::CURRENT_SCHEMA_VERSION)];

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

/// The rendered authoring contract, from the binary the runbooks invoke.
fn describe_schema() -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
        .arg("describe-schema")
        .output()
        .expect("run mnemosyne-cli describe-schema");
    assert!(
        out.status.success(),
        "describe-schema must exit 0; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("describe-schema output is utf-8")
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
    /// The COMMAND that prints these bytes — verb first, then its arguments —
    /// on a `reproduced-output` and nowhere else (Round 973, widened to a full
    /// argv in Round 974). Present exactly when the role is that one, which is
    /// what makes the role a claim the replay job can settle rather than a
    /// label: the job runs this command at the unit's pinned revision and
    /// compares its stdout against `sha256` above.
    ///
    /// It is an argv and not a verb because the reports need one — measured:
    /// `report-transition-map --rules rules.json` reproduces its recorded
    /// transcript and `report-transition-map` alone does not. The arguments are
    /// constrained to the workspace (no absolute path, no `..`), because the
    /// only thing a replayed verb may read is what the replay put there.
    reproduced_by: Option<Vec<String>>,
    /// The replay whose workspace this command runs in, for every verb that
    /// reads a store. Absent exactly for the storeless ones.
    reproduced_after: Option<String>,
    /// The kit's own files this command needs beside the store, staged into the
    /// replay's workspace under their base names.
    ///
    /// This exists because a first attempt without it FABRICATED one. The 24
    /// frame-views need a canon order to resolve their coordinate, the kits
    /// that hold them track no `mnemosyne.toml` at all, and the measurement
    /// that "proved" they reproduce had written a config the tree does not
    /// contain — so it measured a workspace the gate could never build. Every
    /// path here must itself be a declared input of the same unit, which is
    /// what keeps a staged file sealed rather than borrowed from today's tree.
    reproduced_with: Vec<String>,
    /// The exit status the recorded run ended with. `validate-continuity` exits
    /// 1 when it rejects, and five recorded transcripts ARE the output of a
    /// rejecting run — so a non-zero exit is evidence, not a failure, and the
    /// record says which it was rather than the check assuming zero.
    reproduced_exit: i32,
    /// Why this banner-carrying artifact is NOT reproducible, for the ones no
    /// single command can print. Mutually exclusive with the two fields above:
    /// an artifact is either checked or it says why it cannot be.
    ///
    /// It names a MECHANISM — what stops a command from existing — and never a
    /// count, because a mechanism is true at every revision and a count is
    /// state that something has to measure. The numbers here have fields:
    /// `reproduced_exit` and `store_surplus` below.
    unreproducible: Option<String>,
    /// The ids the recorded run's store held that this unit's replay does not
    /// create — the state half of a store-shaped excuse (Round 975).
    ///
    /// Round 974 excused one transcript in prose, and the sentence was wrong in
    /// both of its halves: it reported the run as placing 23 sections where the
    /// run placed 22 of 23, and it called the bytes "evidence of a store the
    /// record does not describe" when this kit's own sealed iteration notes name
    /// the extra section, its title, and why it exists. Naming the id instead of
    /// describing the store lets the check settle both: the id must be created
    /// by no step of this unit's replay, and it must be found in this unit's own
    /// sealed evidence — so "the record does not describe it" cannot be written
    /// about something the record describes.
    store_surplus: Vec<String>,
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
            let reproduced_by = i["reproduced_by"].as_array().map(|argv| {
                let argv: Vec<String> = argv
                    .iter()
                    .map(|a| {
                        a.as_str()
                            .unwrap_or_else(|| {
                                panic!("{file}: input {path} has a non-string in `reproduced_by`")
                            })
                            .to_string()
                    })
                    .collect();
                let verb = argv.first().unwrap_or_else(|| {
                    panic!("{file}: input {path} declares an empty `reproduced_by`")
                });
                assert!(
                    vocabulary("reproduced_by").contains(&verb.as_str()),
                    "{file}: input {path} names `{verb}` as what reproduces it, \
                     and this job cannot run that verb — a claim with no check \
                     behind it is what this role exists to avoid"
                );
                // The workspace is the only place a replayed verb may read. An
                // absolute path or a `..` would let a record reach outside the
                // one tree the replay controls, and the bytes it then compared
                // would not be evidence of anything.
                for a in &argv[1..] {
                    assert!(
                        !a.starts_with('/') && !a.split('/').any(|s| s == ".."),
                        "{file}: input {path} passes `{a}`, which leaves the \
                         replay's workspace — a reproduced transcript may only \
                         read what the replay put there"
                    );
                }
                argv
            });
            inputs.push(Input {
                unit: unit.clone(),
                path,
                role: i["role"].as_str().expect("input role").to_string(),
                sha256,
                reproduced_by,
                reproduced_after: i["reproduced_after"].as_str().map(str::to_string),
                reproduced_with: i["reproduced_with"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|p| p.as_str().expect("reproduced_with path").to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
                reproduced_exit: i["reproduced_exit"].as_i64().unwrap_or(0) as i32,
                unreproducible: i["unreproducible"].as_str().map(str::to_string),
                store_surplus: i["store_surplus"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|s| s.as_str().expect("store_surplus id").to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
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

/// The banner `describe-schema` prints, with its schema number replaced by the
/// wildcard that makes it match at every revision.
///
/// DERIVED by running the verb, never typed. This is the Round 962 rule at the
/// place it bites hardest: the banner carries the schema version, so a literal
/// copied into this file would be right at today's revision and wrong at the
/// seven pinned ones the recorded contracts were captured at — and the failure
/// would be silence, since a pattern that matches nothing reports no
/// candidates and no violations.
fn contract_banner_prefix() -> String {
    let first = describe_schema()
        .lines()
        .next()
        .expect("describe-schema prints a banner")
        .to_string();
    let cut = first
        .find(char::is_numeric)
        .expect("the banner carries the schema version");
    first[..cut].to_string()
}

/// EVERY RECORDED AUTHORING CONTRACT CLAIMS THE ROLE THAT CHECKS IT.
///
/// The half that keeps `reproduced-output` from being decoration. Without this,
/// the role would be discharged wherever a record happened to claim it and
/// absent wherever nobody bothered — and a reader could not tell a transcript
/// nobody checked from a file nobody could check. The candidate set is taken
/// from the EVIDENCE, the way Round 953 took coverage from the filesystem
/// rather than from the record: a run artifact whose first line is the contract
/// banner is a captured `describe-schema`, and it has to say so.
///
/// WHAT THIS CANNOT DO, measured rather than implied. The banner is the first
/// line, so a contract captured with a wrapper around it — `echo` before the
/// command, an orchestrator's own header — is not seen. That failure mode is
/// real and not hypothetical: the same shape hides two `first-import.log` files
/// whose run errored (the CLI prints no banner on the failure path) and one
/// `gate-output.txt` an author wrapped in `===== validate-continuity =====`. It
/// is why the ROLE's own check is re-execution and not this scan — this only
/// decides who is asked.
#[test]
fn every_recorded_contract_is_declared_as_a_reproduced_transcript() {
    let root = repo_root();
    let d = declarations();
    let banner = contract_banner_prefix();

    let roles: BTreeMap<String, String> = d
        .inputs
        .iter()
        .map(|i| (normalize(&format!("{}/{}", i.unit, i.path)), i.role.clone()))
        .collect();

    let mut candidates = 0usize;
    let mut unclaimed: Vec<String> = Vec::new();
    for path in run_artifacts() {
        let Ok(bytes) = std::fs::read(root.join(&path)) else {
            continue;
        };
        let first = String::from_utf8_lossy(&bytes);
        let Some(first) = first.lines().next() else {
            continue;
        };
        if !first.starts_with(&banner) {
            continue;
        }
        candidates += 1;
        match roles.get(&normalize(&path)) {
            Some(role) if role == "reproduced-output" => {}
            Some(role) => unclaimed.push(format!("{path} is declared `{role}`")),
            None => unclaimed.push(format!("{path} is declared by no record")),
        }
    }

    assert!(
        unclaimed.is_empty(),
        "{} captured authoring contract(s) do not claim the role that checks \
         them. Each is a `describe-schema` transcript, so it is declared \
         `reproduced-output` with `reproduced_by: \"describe-schema\"` — run \
         `experiment-harness set-input-role --record <replay.json> --path <p> \
         --role reproduced-output --reproduced-by describe-schema`:\n{}",
        unclaimed.len(),
        unclaimed.join("\n")
    );
    // Non-vacuity: a banner that matched nothing would satisfy the assertion
    // above while asking nobody anything, which is precisely how a derived
    // pattern fails.
    assert!(
        candidates > 0,
        "no run artifact begins with the contract banner `{banner}` — either \
         the corpus holds no captured contract, or the derivation above stopped \
         matching the verb's output and this test now asks nothing"
    );
    println!("{candidates} captured contract(s), all declared `reproduced-output`");
}

/// EVERY ARTIFACT A TOOL PRINTED IS EITHER REGENERATED OR SAYS WHY IT IS NOT.
///
/// Round 973 closed one shape — the captured contract — and left 92 banner-
/// carrying artifacts under a role that says only "this is in the run tree".
/// Round 974 reproduces 58 more and leaves 34, and THAT is the state this gate
/// exists to keep honest: a class half-checked with nothing marking the other
/// half reads, to the next reader, as a class fully checked.
///
/// The discriminator is a SHAPE and not a content match, which is why it needs
/// no derivation: a first line of `=== … ===` is the banner every CLI report
/// opens with. Measured over the whole corpus (2026-08-03): exactly 108 of 636
/// tracked run-tree files match, and none of them carries `replay-input` or
/// `raw-agent-output`, so this rule reaches only the unclassified class.
///
/// A reason is prose and nothing checks that it is TRUE — what it checks is
/// that somebody had to write one, which is the difference between a bounded
/// scope and a silent one.
#[test]
fn every_tool_printed_artifact_is_reproduced_or_says_why_not() {
    let root = repo_root();
    let d = declarations();
    let by_path: BTreeMap<String, &Input> = d
        .inputs
        .iter()
        .map(|i| (normalize(&format!("{}/{}", i.unit, i.path)), i))
        .collect();

    let mut banners = 0usize;
    let mut reproduced = 0usize;
    let mut excused = 0usize;
    let mut silent: Vec<String> = Vec::new();
    for path in run_artifacts() {
        let Ok(bytes) = std::fs::read(root.join(&path)) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let Some(first) = text.lines().next() else {
            continue;
        };
        if !(first.starts_with("=== ") && first.ends_with(" ===")) {
            continue;
        }
        banners += 1;
        match by_path.get(&normalize(&path)) {
            Some(i) if i.role == "reproduced-output" => reproduced += 1,
            Some(i) if i.unreproducible.is_some() => excused += 1,
            Some(i) => silent.push(format!("{path} is `{}`", i.role)),
            None => silent.push(format!("{path} is declared by no record")),
        }
    }

    assert!(
        silent.is_empty(),
        "{} artifact(s) carry a CLI banner and neither claim \
         `reproduced-output` nor say why they cannot. Declare the command that \
         prints it, or state in `unreproducible` what stops one from \
         existing:\n{}",
        silent.len(),
        silent.join("\n")
    );
    // Non-vacuity on both halves: a corpus with nothing excused would leave the
    // `unreproducible` branch untested, and one with nothing reproduced would
    // make this rule an alias for "everything is excused".
    assert!(
        reproduced > 0 && excused > 0,
        "this rule needs both halves exercised — {reproduced} reproduced, \
         {excused} excused, {banners} banner(s) seen"
    );
    println!("{banners} tool-printed artifact(s): {reproduced} reproduced, {excused} excused");
}

/// An excuse names a MECHANISM, and mechanisms do not carry numbers.
///
/// Measured before this rule was written: 43 excuses, 4 distinct texts. The
/// three that are right are structural — a harness this job does not build at a
/// pinned revision, a runbook that appends an exit code so the verb's stdout is
/// a prefix of the bytes — and each is true at every revision, so none of them
/// needs a count. The fourth carried two numbers and BOTH were wrong, in a
/// sentence nothing reads. That is Round 959's rule (a number no program looks
/// at does not belong in shipped prose) meeting a field that had no other place
/// to put one, and the fix is the places: `reproduced_exit` holds a status and
/// `store_surplus` holds what a store contained.
#[test]
fn an_excuse_names_a_mechanism_and_leaves_the_numbers_to_the_fields() {
    let d = declarations();
    let mut checked = 0usize;
    for i in &d.inputs {
        let Some(why) = &i.unreproducible else {
            continue;
        };
        checked += 1;
        assert!(
            !why.chars().any(|c| c.is_ascii_digit()),
            "{}/{} excuses itself with a number in it:\n  {why}\nAn excuse says \
             what STOPS a command from existing, which is true at every \
             revision. A count is state, and state has fields that settle it — \
             `reproduced_exit` for a status, `store_surplus` for what the run's \
             store held that the replay does not create. Round 974 wrote both of \
             this field's numbers wrong and nothing noticed.",
            i.unit,
            i.path
        );
    }
    // Non-vacuity: with nothing excused this rule reads as satisfied while
    // asserting nothing at all.
    assert!(checked > 0, "no excuse was read — the rule ran on nothing");
    println!("{checked} excuse(s) name a mechanism and no number");
}

/// An excuse that blames the store NAMES what was in it, and what it names is
/// found in this unit's own sealed evidence.
///
/// Round 974 diffed one transcript against the rebuilt store, saw a section it
/// could not account for, and declared the bytes "evidence of a store the record
/// does not describe". The record describes it exactly: the author's own
/// iteration notes — a declared, sha-pinned input of the same unit — name the
/// stray section, its title, the probe that created it, and the fact that no
/// manifest contains it. Reading the manifests and concluding the record is
/// silent is the same error as deriving a command from a banner: the answer was
/// in the kit, in a file the search did not open.
///
/// So the excuse's state half is a list of ids, and the two arms make the false
/// claim unwritable. An id no step creates is what makes a command impossible;
/// an id found in the unit's sealed evidence is what makes "the record does not
/// describe it" false about it. An id that fails either is not a surplus — it is
/// a guess about a store.
#[test]
fn an_excuse_that_blames_the_store_names_what_the_record_describes() {
    let root = repo_root();
    let d = declarations();
    let mut checked = 0usize;
    for i in &d.inputs {
        if i.store_surplus.is_empty() {
            continue;
        }
        let at = format!("{}/{}", i.unit, i.path);
        assert!(
            i.unreproducible.is_some(),
            "{at} declares `store_surplus` and no `unreproducible` — a store \
             difference is the REASON an artifact cannot be reproduced, so on \
             anything else it is a field with no claim behind it"
        );
        // "The replay does not rebuild it" needs a replay to be about.
        assert!(
            d.replays.iter().any(|r| r.unit == i.unit),
            "{at} says its run's store held ids this kit's replay does not \
             create, and {} declares no replay at all",
            i.unit
        );
        let created = ids_a_unit_creates(&root, &d, &i.unit);
        // Every OTHER declared input of this unit, read once: the evidence the
        // record consists of, minus the artifact whose own bytes are what is in
        // question.
        let evidence: Vec<(&str, String)> = d
            .inputs
            .iter()
            .filter(|o| o.unit == i.unit && o.path != i.path)
            .map(|o| {
                let p = normalize(&format!("{}/{}", o.unit, o.path));
                let text = std::fs::read(root.join(&p))
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default();
                (o.path.as_str(), text)
            })
            .collect();
        for id in &i.store_surplus {
            assert!(
                !created.contains(id),
                "{at} calls `{id}` a store surplus, and this kit's replay \
                 CREATES it — the rebuilt store has it, so whatever stops a \
                 command from printing these bytes, it is not this id"
            );
            let found: Vec<&str> = evidence
                .iter()
                .filter(|(_, text)| text.contains(id.as_str()))
                .map(|(p, _)| *p)
                .collect();
            assert!(
                !found.is_empty(),
                "{at} calls `{id}` a store surplus, and no other sealed input of \
                 {} mentions it. Then the record really does not describe that \
                 store, and the honest declaration is what is missing from the \
                 kit — not an id this check cannot resolve against anything",
                i.unit
            );
            checked += 1;
        }
    }
    // Non-vacuity: with no surplus declared anywhere, both arms above are
    // untested and this rule is an alias for "no kit uses the field".
    assert!(
        checked > 0,
        "no store surplus was resolved — the rule ran on nothing"
    );
    println!("{checked} declared store surplus id(s) resolved against sealed evidence");
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
    // The units a verb could actually be run at, and the revision to run it at.
    // A record with an empty `replays` states why (`no_replay`) and pins
    // nothing, so it has no revision to reproduce anything against; a record
    // pinning two would leave which one a transcript came from unsettled, and
    // the check would be choosing rather than reading.
    let mut pinned: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for r in &d.replays {
        pinned
            .entry(r.unit.clone())
            .or_default()
            .insert(r.revision.clone());
    }
    let pinned: BTreeMap<String, BTreeSet<String>> = pinned;

    // A field belongs to the role that gives it meaning. `reproduced_by` on
    // anything else would be a verb named where nothing runs it, which reads to
    // the next author as a checked claim and is not one.
    for i in &d.inputs {
        if i.role != "reproduced-output" {
            assert!(
                i.reproduced_by.is_none() && i.reproduced_after.is_none(),
                "{}/{} is `{}` and names a reproducing command — only \
                 `reproduced-output` is checked against one",
                i.unit,
                i.path,
                i.role
            );
        }
    }

    // Round 974: the marker list may not drift off the list it marks.
    for v in STORELESS_VERBS {
        assert!(
            REPRODUCIBLE_VERBS.contains(v),
            "`{v}` is marked storeless and is not a reproducible verb at all"
        );
    }

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
            // The role that has to name its own check. A transcript is not fed
            // to anything and it lives in the run tree like its sibling, but it
            // additionally claims the bytes are a verb's output — so it names
            // the verb, and the unit has to pin a revision for that verb to be
            // run at. A record with no pin could assert the claim and nothing
            // would ever test it.
            "reproduced-output" => {
                assert!(
                    !fed.contains(&path),
                    "{path} is declared `reproduced-output` but a replay step \
                     feeds it — a transcript is what came out, not what went in"
                );
                assert!(
                    path.starts_with(&format!("{}/", i.unit)) && path.contains("/run/"),
                    "{path} is declared `reproduced-output` but does not sit \
                     under a run tree of {}",
                    i.unit
                );
                let argv = i.reproduced_by.as_ref().unwrap_or_else(|| {
                    panic!(
                        "{path} is declared `reproduced-output` and does not \
                         say what reproduces it — the role IS the command plus \
                         a revision to run it at"
                    )
                });
                let verb = argv[0].as_str();
                match (STORELESS_VERBS.contains(&verb), &i.reproduced_after) {
                    // A verb that reads a store must say which replay built it.
                    // Without that the check would pick a replay, and picking
                    // is what the record exists to stop.
                    (false, None) => panic!(
                        "{path} is reproduced by `{verb}`, which reads a store, \
                         and names no `reproduced_after` — say which replay's \
                         workspace it runs in"
                    ),
                    // And one that reads none must not name a replay it does
                    // not use: a field with no consequence reads as a checked
                    // one.
                    (true, Some(after)) => panic!(
                        "{path} is reproduced by `{verb}`, which reads no \
                         store, yet names `reproduced_after: {after}` — the \
                         replay would have no bearing on the bytes"
                    ),
                    (false, Some(after)) => assert!(
                        d.replays
                            .iter()
                            .any(|r| r.unit == i.unit && &r.name == after),
                        "{path} names `reproduced_after: {after}`, and {} \
                         declares no replay by that name",
                        i.unit
                    ),
                    (true, None) => match pinned.get(&i.unit) {
                        None => panic!(
                            "{path} is declared `reproduced-output` but {} pins \
                             no revision, so nothing can run the verb — the \
                             claim would never be tested",
                            i.unit
                        ),
                        Some(revs) => assert!(
                            revs.len() == 1,
                            "{path} is declared `reproduced-output` and {} pins \
                             {} revisions {revs:?} — which one printed these \
                             bytes is then unsettled, and the check would be \
                             choosing rather than reading",
                            i.unit,
                            revs.len()
                        ),
                    },
                }
                assert!(
                    i.unreproducible.is_none(),
                    "{path} is declared `reproduced-output` AND says why it \
                     cannot be reproduced — one of the two is a lie"
                );
                // A staged file has to be sealed evidence of this same unit,
                // or the bytes it contributes come from today's tree and the
                // comparison stops being about the pinned revision at all.
                let declared: BTreeSet<&str> = d
                    .inputs
                    .iter()
                    .filter(|o| o.unit == i.unit)
                    .map(|o| o.path.as_str())
                    .collect();
                let mut bases: BTreeSet<&str> = BTreeSet::new();
                for w in &i.reproduced_with {
                    assert!(
                        declared.contains(w.as_str()),
                        "{path} stages `{w}`, which {} does not declare as an \
                         input — only sealed evidence may be staged",
                        i.unit
                    );
                    let base = w.rsplit('/').next().expect("a path has a name");
                    assert!(
                        bases.insert(base),
                        "{path} stages two files named `{base}` — one would \
                         silently shadow the other in the workspace"
                    );
                    assert!(
                        argv.iter().any(|a| a == base),
                        "{path} stages `{w}` and its command never names \
                         `{base}` — a file staged and unread is a claim with no \
                         consequence"
                    );
                }
                assert!(
                    i.reproduced_with.is_empty() || i.reproduced_after.is_some(),
                    "{path} stages files and runs no replay — a storeless verb \
                     runs in the extracted tree, where staging has nowhere to go"
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

/// What one replay step puts in the store, by registry.
enum Created {
    Sections(Vec<String>),
    Facts(Vec<String>),
    /// A proposal types or connects what an earlier step created and adds no id
    /// of its own.
    Nothing,
}

/// The ids one replay step creates, read off the manifest that step feeds.
///
/// ONE resolver, because two rules ask this question and would be free to drift
/// apart: the citation walk needs the ids in step order, and the surplus check
/// needs their union. What they would drift about is what a replay puts in the
/// store, which is the whole of what both rules decide.
fn ids_a_step_creates(doc: &serde_json::Value, verb: &str, where_: &str) -> Created {
    match verb {
        "import-sections" => Created::Sections(
            doc.as_array()
                .unwrap_or_else(|| panic!("{where_}: not a JSON array"))
                .iter()
                .filter_map(|e| e.get("section_id").and_then(|v| v.as_str()))
                .map(str::to_string)
                .collect(),
        ),
        "import-facts" | "propose-verdict" => Created::Facts(ids(doc, "facts", "fact_id")),
        "import-typing-proposals" | "import-edge-proposals" => Created::Nothing,
        other => panic!("{where_}: no creation rule for verb `{other}`"),
    }
}

/// Every id this unit's replays leave in a store, unioned across their steps.
///
/// A `reject` step contributes nothing — `propose-verdict` rolls back — so an
/// id only a rejected step names is genuinely absent from the rebuilt store.
fn ids_a_unit_creates(root: &Path, d: &Declarations, unit: &str) -> BTreeSet<String> {
    let mut created = BTreeSet::new();
    for r in d.replays.iter().filter(|r| r.unit == unit) {
        for (n, s) in r.steps.iter().enumerate() {
            if s.expect == "reject" {
                continue;
            }
            let path = normalize(&format!("{}/{}", r.unit, s.input));
            let where_ = format!("{}/{} step {n} ({})", r.unit, r.name, s.input);
            match ids_a_step_creates(&read_json(root, &path), &s.verb, &where_) {
                Created::Sections(v) | Created::Facts(v) => created.extend(v),
                Created::Nothing => {}
            }
        }
    }
    created
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
                "import-sections" => match ids_a_step_creates(&doc, &s.verb, &where_) {
                    Created::Sections(v) => sections.extend(v),
                    _ => unreachable!("import-sections creates sections"),
                },
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
                    match ids_a_step_creates(&doc, &s.verb, &where_) {
                        Created::Facts(v) => visible.extend(v),
                        _ => unreachable!("a fact manifest creates facts"),
                    }
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
/// The workspace is returned alongside the digest (Round 974) because the
/// transcripts a kit records are output of verbs run against exactly this
/// store. Rebuilding a second workspace to run them in would be a second
/// reconstruction free to differ from the one the digest pins.
fn run_replay(
    cli: &Path,
    root: &Path,
    tree: &Path,
    r: &Replay,
) -> Result<(String, TempDir), String> {
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
    Ok((mnemosyne_core::sha256_hex(&store), ws))
}

/// Run one declared transcript's command and compare its stdout to the bytes the
/// seal holds. `cwd` is the extracted tree for a storeless verb and the replay's
/// own workspace for every other, which is the whole reason the workspace is
/// kept alive above.
fn check_transcript(
    cli: &Path,
    cwd: &Path,
    root: &Path,
    tree: &Path,
    i: &Input,
    rev: &str,
) -> Result<(), String> {
    let argv = i.reproduced_by.as_ref().expect("filtered on Some");
    let name = format!("{}/{}", i.unit, i.path);

    // Stage what the command reads besides the store, from the PINNED tree and
    // only after checking it has not moved since — the same rule the config
    // copy follows, for the same reason.
    for w in &i.reproduced_with {
        let rel = normalize(&format!("{}/{}", i.unit, w));
        let then = std::fs::read(tree.join(&rel))
            .map_err(|e| format!("{name}: staged {rel} is not in the pinned tree: {e}"))?;
        if then != std::fs::read(root.join(&rel)).expect("staged file today") {
            return Err(format!(
                "{name}: staged {rel} differs between {rev} and today — the \
                 command would read something the record does not show"
            ));
        }
        let base = rel.rsplit('/').next().expect("a path has a name");
        std::fs::write(cwd.join(base), &then).expect("stage file");
    }

    let out = Command::new(cli)
        .args(argv)
        .current_dir(cwd)
        .output()
        .expect("cli exec");
    let got = out.status.code().unwrap_or(-1);
    if got != i.reproduced_exit {
        return Err(format!(
            "{name}: `{}` exits {got} at {rev} and the record says {} — a gate \
             that rejected is evidence, but only if the record says which it \
             was:\n{}",
            argv.join(" "),
            i.reproduced_exit,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let sha = mnemosyne_core::sha256_hex(&out.stdout);
    if sha == i.sha256 {
        Ok(())
    } else {
        Err(format!(
            "{name}: declared `reproduced-output` of `{}`, but that command at \
             {rev} prints {sha} and the sealed bytes are {} — either the file \
             is not what the record says it is, or the revision it names is not \
             the one that printed it",
            argv.join(" "),
            i.sha256
        ))
    }
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

    // The STORELESS transcripts, keyed by the revision whose binary prints
    // them. Their unit pins exactly one (the cheap gate holds that), so this
    // reads the pin rather than choosing among several.
    let mut storeless: BTreeMap<String, Vec<&Input>> = BTreeMap::new();
    // The rest, keyed by the replay whose workspace they run in.
    let mut after_replay: BTreeMap<(String, String), Vec<&Input>> = BTreeMap::new();
    for i in d.inputs.iter().filter(|i| i.reproduced_by.is_some()) {
        match &i.reproduced_after {
            Some(after) => after_replay
                .entry((i.unit.clone(), after.clone()))
                .or_default()
                .push(i),
            None => {
                let rev = d
                    .replays
                    .iter()
                    .find(|r| r.unit == i.unit)
                    .map(|r| r.revision.clone())
                    .expect("a storeless transcript's unit pins a revision");
                storeless.entry(rev).or_default().push(i);
            }
        }
    }

    let mut ran = 0usize;
    let mut reproduced = 0usize;
    let mut blocked_confirmed = 0usize;
    let mut undeclared: Vec<(String, String)> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    for rev in &revisions {
        let (tree, _target, cli) = build_revision(&root, rev);

        // THE ROLE IS DISCHARGED HERE, not by anything that reads the file's
        // text: the command runs at the revision the record pins, and its
        // stdout is compared byte for byte against the sha the seal carries.
        // A storeless verb needs nothing but the extracted tree.
        for i in storeless.get(rev).into_iter().flatten() {
            match check_transcript(&cli, tree.path(), &root, tree.path(), i, rev) {
                Ok(()) => reproduced += 1,
                Err(why) => wrong.push(why),
            }
        }

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
                (None, Ok((sha, _ws))) => {
                    // Determinism is measured here, not inherited from R881's
                    // sample: a digest from a single run could pin a hash map's
                    // iteration order and reject the same evidence tomorrow.
                    let (again, ws) = run_replay(&cli, &root, tree.path(), r)
                        .expect("the same replay failed on its second run");
                    // The transcripts this replay's store is behind, checked in
                    // the SECOND workspace — the one whose digest was just
                    // confirmed to match the first.
                    for i in after_replay
                        .get(&(r.unit.clone(), r.name.clone()))
                        .into_iter()
                        .flatten()
                    {
                        match check_transcript(&cli, ws.path(), &root, tree.path(), i, rev) {
                            Ok(()) => reproduced += 1,
                            Err(why) => wrong.push(why),
                        }
                    }
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
    // The same standard for the half added in Round 973: a corpus that declared
    // the role nowhere would pass the loop above without running a verb once.
    assert!(
        reproduced > 0,
        "no `reproduced-output` was regenerated — the role is declared nowhere \
         and its check asserted nothing"
    );
    println!(
        "{ran} replays reproduced their declared digest, {blocked_confirmed} blocked as declared, \
         {reproduced} transcript(s) regenerated at their pinned revision"
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

/// How a text wrote a program-owned number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberShape {
    /// `k: 23`, `"k":23`, `"k": 23`, `k = 23` — pasteable as data.
    Key,
    /// The key's first segment carrying the number in prose: `schema-23`,
    /// `schema 23`. Round 962's entry recorded this shape as out of machine
    /// reach and left it as a carry; measuring it one round later found EIGHT
    /// live sites and NOT ONE false positive, every hit an instruction of the
    /// form "empty schema-23 seed -> `import-sections`". The carry was written
    /// without measuring, which writes a defect smaller than it is.
    Prose,
}

/// One program-owned number a text writes out, and the shape it used.
#[derive(Debug)]
struct TypedNumber {
    line: usize,
    field: &'static str,
    value: String,
    shape: NumberShape,
}

/// The key's prose stem: everything before the first `_`.
///
/// Derived from the key rather than listed beside it. A hand-written alias
/// column is a second copy of the same fact, free to disagree with the row it
/// belongs to.
fn prose_stem(field: &str) -> &str {
    field.split('_').next().unwrap_or(field)
}

/// Every program-owned number a text writes beside its key.
///
/// Four spellings, because those are the four the runbooks on disk actually
/// use and each of them is pasteable: `k: 23`, `"k":23`, `"k": 23`, `k = 23`.
/// The `=` form is not JSON and still counts — `convergence-probe/v2` carried
/// the stale version in a "seed schema_version = 23" step, which is copied by
/// reading just as surely as by selecting.
///
/// The boundary rule is the sibling scanner's: a key is only this key when what
/// precedes it is not part of a longer name, so `store_schema_version` is left
/// alone while `"schema_version"` is read. Prose that names the number away
/// from the key — "the seed store's schema version is 23" — is deliberately
/// invisible here. That is the Round 950 correction form, and it is what lets a
/// runbook record what it used to say without teaching it again.
fn typed_numbers(text: &str) -> Vec<TypedNumber> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        for (field, _) in PROGRAM_OWNED_NUMBERS {
            let field = *field;
            let mut from = 0usize;
            while let Some(at) = line[from..].find(field) {
                let start = from + at;
                from = start + field.len();
                if line[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    continue;
                }
                let rest = line[from..].strip_prefix('"').unwrap_or(&line[from..]);
                let rest = rest.trim_start_matches(' ');
                let Some(rest) = rest.strip_prefix([':', '=']) else {
                    continue;
                };
                let digits: String = rest
                    .trim_start_matches(' ')
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                if digits.is_empty() {
                    continue;
                }
                out.push(TypedNumber {
                    line: n + 1,
                    field,
                    value: digits,
                    shape: NumberShape::Key,
                });
            }

            // The prose shape. `-` joins the boundary set here so that
            // `describe-schema 23` is not read as the stem `schema`, and `_`
            // keeps `schema_version: 23` from being counted a second time.
            let stem = prose_stem(field);
            let mut from = 0usize;
            while let Some(at) = line[from..].find(stem) {
                let start = from + at;
                from = start + stem.len();
                if line[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
                {
                    continue;
                }
                let Some(rest) = line[from..].strip_prefix([' ', '-']) else {
                    continue;
                };
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    continue;
                }
                out.push(TypedNumber {
                    line: n + 1,
                    field,
                    value: digits,
                    shape: NumberShape::Prose,
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

/// No runbook writes a number the program owns beside that number's key.
///
/// The sibling above is the WORD half: a value out of a closed vocabulary, which
/// a document can hold correctly forever. This is the NUMBER half, and it cannot
/// be closed the same way. Round 958 found by hand that three runbooks seeded
/// `"schema_version":23` while the constant had reached 44, and named the reason
/// it had gone unseen for so long: the loader MIGRATES a stale version instead
/// of rejecting it, so the wrong seed produces no error, byte-identical
/// receipts, and no signal at all to the arm that inherits it. That round fixed
/// the three it was looking at. Four more were carrying the same literal, one of
/// them under a bold "MUST".
///
/// The remedy the message names is DERIVATION, not correction, and the argument
/// for it is measured rather than aesthetic: each of those kits is replayed at
/// its own pinned revision, where the constant is 23, so `describe-schema`
/// answers 23 there and 44 here while the literal can only ever be one of them.
/// Rewriting a completed kit's runbook to ask the program therefore changes
/// nothing about what its replay seeds — which is why Round 958 could do it to
/// map-corpus and disclosed-place, both long since run, and why it is right for
/// the rest.
#[test]
fn no_runbook_types_a_number_the_program_owns() {
    let root = repo_root();
    let books = runbooks(&root);
    assert!(
        !books.is_empty(),
        "no runbooks found under claudedocs/ — this gate would pass vacuously"
    );
    let mut violations = Vec::new();
    let mut discusses: BTreeMap<&str, usize> = PROGRAM_OWNED_NUMBERS
        .iter()
        .map(|(field, _)| (*field, 0usize))
        .collect();
    for rel in &books {
        let text =
            std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        for t in typed_numbers(&text) {
            let written = match t.shape {
                NumberShape::Key => format!("`{}: {}`", t.field, t.value),
                NumberShape::Prose => format!("`{}-{}` in prose", prose_stem(t.field), t.value),
            };
            violations.push(format!("{rel}:{} writes {written}", t.line));
        }
        for (field, _) in PROGRAM_OWNED_NUMBERS {
            if text.contains(field) {
                *discusses.get_mut(field).expect("seeded above") += 1;
            }
        }
    }
    // The clean state here is ZERO hits, so the count that proves this gate is
    // awake cannot be the hits. It is the corpus: these runbooks DO talk about
    // the key — they seed stores with it — and the claim is that none of them
    // pastes a number beside it. A key no runbook mentions would make the scan
    // above assert nothing while reporting a clean tree.
    for (field, seen) in &discusses {
        assert!(
            *seen > 0,
            "no runbook mentions `{field}` at all, so scanning for it proves \
             nothing. Either the field was renamed in the product and this row \
             is stale, or the corpus stopped being the place the seed is written."
        );
    }
    assert!(
        violations.is_empty(),
        "{} site(s) type a number the program owns:\n  {}\n\
         A runbook is copied from, and a kit is replayed at ITS OWN pinned \
         revision, where the constant is whatever it was that day — so a typed \
         number is right at exactly one revision and wrong at every other, and \
         it is wrong SILENTLY, because the loader migrates a stale version \
         rather than refusing it. Ask the program instead \
         (`SV=\"$(mnemosyne-cli describe-schema | sed -n \
         '1s/.*schema v\\([0-9]\\+\\).*/\\1/p')\"`), which answers correctly at \
         every revision; or, if the point is to record what a past arm used, \
         state the number away from its key, where nothing can paste it.",
        violations.len(),
        violations.join("\n  ")
    );
    println!(
        "{} runbook(s), none types a program-owned number",
        books.len()
    );
}

/// The scan reads every shape it exists to read, and reads nothing else.
///
/// Both halves are load-bearing and the second is the one that keeps this gate
/// alive. A scan that also fired on `violations: 0` would be reporting an arm's
/// own oracle as a defect, and the fix for that false positive is to delete the
/// gate. The probes are built FROM the table, so a row is never checked against
/// a hand-typed copy of itself.
#[test]
fn the_number_scan_sees_the_key_shape_and_spares_the_oracles() {
    for (field, value) in PROGRAM_OWNED_NUMBERS {
        for pasteable in [
            format!("- The seed store is `{field}: {value}`.\n"),
            format!("  printf '{{\"{field}\":{value},\"sections\":{{}}}}' > store.json\n"),
            format!("  `{{ \"{field}\": {value}, \"sections\": {{}} }}`.\n"),
            format!("0. seed {field} = {value}; the ABSOLUTE-path rule documented\n"),
        ] {
            let found = typed_numbers(&pasteable);
            assert_eq!(
                found.len(),
                1,
                "`{field}`: the scan missed a form it exists to read: {pasteable:?} -> {found:?}"
            );
            assert_eq!(found[0].value, value.to_string());
        }

        // The prose shape, which Round 962 called out of reach and Round 963
        // measured at eight live sites with no false positive.
        let stem = prose_stem(field);
        for prose in [
            format!("- Rebuild FRESH: empty {stem}-{value} seed -> `import-sections`\n"),
            format!("- the workspace store = the empty {stem} {value} seed\n"),
        ] {
            let found = typed_numbers(&prose);
            assert_eq!(
                found.len(),
                1,
                "`{field}`: the scan missed the prose shape: {prose:?} -> {found:?}"
            );
            assert_eq!(found[0].shape, NumberShape::Prose);
            assert_eq!(found[0].value, value.to_string());
        }

        // A verb whose name ENDS in the stem is not the stem.
        let verb = format!("- confirm `describe-{stem} {value}` is not a command\n");
        assert!(
            typed_numbers(&verb).is_empty(),
            "`{field}`: the scan reads a hyphenated verb name as the prose \
             shape: {verb:?}"
        );

        // The key shape is counted ONCE, not once per shape.
        let both = format!("- `{field}: {value}`\n");
        let found = typed_numbers(&both);
        assert_eq!(
            found.len(),
            1,
            "`{field}`: the key shape is double-counted by the prose scan: {found:?}"
        );

        // The Round 950 correction form: the number named, the key not fed.
        let recorded = format!("- the seed store's schema version was {value} at that pin\n");
        assert!(
            typed_numbers(&recorded).is_empty(),
            "`{field}`: the scan fires on prose that names the number away from \
             its key, which is the only form left for recording what a past arm \
             used: {recorded:?}"
        );

        // A longer name ending in this key is not this key.
        let longer = format!("- `store_{field}: {value}` is a different field\n");
        assert!(
            typed_numbers(&longer).is_empty(),
            "`{field}`: the scan reads a longer field name as this one: {longer:?}"
        );

        // The key with no number beside it is the derived form, which is the
        // remedy — firing on it would leave nothing to fix.
        let derived =
            format!("  printf '{{\"{field}\":%s,\"sections\":{{}}}}' \"$SV\" > store.json\n");
        assert!(
            typed_numbers(&derived).is_empty(),
            "`{field}`: the scan fires on the DERIVED seed, the very form it \
             asks for: {derived:?}"
        );
    }

    // Values an arm asserts about its own run. Deriving any of these from the
    // program is what would void the experiment, so none of them is a violation
    // and none may be swept in by widening the scan from keys to shapes.
    for oracle in [
        "- `violations: 0` on both stores",
        "- `unplaced = 0`, `off_path = 0`",
        "- leaks=0 across the walk",
        "- judges: 3, blind to each other",
        "- n=2 authors, one premise",
        "- playthrough = 19 scenes",
    ] {
        let found = typed_numbers(&format!("{oracle}\n"));
        assert!(
            found.is_empty(),
            "the scan fires on an arm's own oracle, which the program must never \
             own: {oracle:?} -> {found:?}"
        );
    }
}

/// No runbook tells its orchestrator to install this CLI into the shared slot.
///
/// The third member of the family, and the only one whose failure reaches OUT
/// of this repository. `~/.cargo/bin/mnemosyne-cli` is one slot shared with the
/// consumer checkouts on this machine, and `scripts/mn`'s header records the
/// collision as fact, not risk: on 2026-07-29 an install from this repo replaced
/// a consumer's pinned build with an uncommitted local one. `RULEBOOK.md` has
/// forbidden it since Round 823 — and NINE runbooks went on prescribing it, in
/// every case as the remedy for a CLI that looks stale, which is exactly the
/// moment an orchestrator reaches for it.
///
/// `scripts/mn` is the one resolver and it dissolves the question those bullets
/// were asking: it builds this working tree's source on every call, so a verb is
/// present exactly when the source has it and there is nothing to skew. The
/// existence of that script is asserted here, because a gate whose message names
/// a remedy should fail if the remedy is gone.
///
/// The crate name comes from cargo, not from a string here — this test lives in
/// the crate it is protecting. The `cargo install` part is named literally: it
/// is another tool's command, the way `git ls-files` is named literally above.
#[test]
fn no_runbook_installs_the_cli_into_the_shared_slot() {
    let root = repo_root();
    let resolver = root.join("scripts/mn");
    assert!(
        resolver.is_file(),
        "scripts/mn is missing, so the remedy this gate names does not exist"
    );
    let books = runbooks(&root);
    assert!(
        !books.is_empty(),
        "no runbooks found under claudedocs/ — this gate would pass vacuously"
    );
    let crate_name = env!("CARGO_PKG_NAME");
    let mut violations = Vec::new();
    for rel in &books {
        let text =
            std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            if !line.contains("cargo install") {
                continue;
            }
            // The extent is the CODE SPAN, not a fixed line window. Most of the
            // runbooks that carried this wrapped the command across two lines,
            // so a one-line look misses the crate; but a two-line window was
            // measured wrong in the other direction — injecting an install of
            // an unrelated tool directly above ours reported BOTH as
            // violations, because the window swept in the next line's crate.
            // Markdown's own delimiter answers it: the command runs from
            // `cargo install` to the backtick that closes the span.
            let at = line.find("cargo install").expect("checked above");
            let mut span = String::from(&line[at..]);
            let mut k = n;
            while !span.contains('`') && k + 1 < lines.len() && k < n + 4 {
                k += 1;
                span.push(' ');
                span.push_str(lines[k]);
            }
            let span = span.split('`').next().unwrap_or(&span);
            if span.contains(crate_name) {
                violations.push(format!("{rel}:{}", n + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "{} site(s) tell an orchestrator to install `{crate_name}` into \
         ~/.cargo/bin:\n  {}\n\
         That slot is SHARED with the consumer checkouts on this machine, and \
         writing it has already replaced a sibling's pinned build with an \
         uncommitted local one (scripts/mn's header records the date). Use the \
         one resolver instead: `MN=\"$(git rev-parse --show-toplevel)/scripts/mn\"` \
         builds this tree's source on every call, so there is nothing to skew \
         and nothing to reinstall. If the point is to record that a past arm \
         did install it, describe that in words — this gate reads the command, \
         which is the shape that gets pasted.",
        violations.len(),
        violations.join("\n  ")
    );
    println!(
        "{} runbook(s), none installs into the shared slot",
        books.len()
    );
}

/// The path prefix a kit record must be named by, read OUT of the harness.
///
/// `declare.rs` matches a record's parent directory against the output of
/// `git ls-files <glob>`, and git prints repo-root-relative paths — so the glob
/// IS the rule, and restating it here would be a second copy free to drift from
/// the tool it describes.
fn kit_record_prefix(root: &Path) -> String {
    let src = std::fs::read_to_string(root.join("tools/experiment-harness/src/declare.rs"))
        .expect("read the harness source that owns this rule");
    let glob = src
        .split("\"ls-files\", \"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("declare.rs globs the kit records with `git ls-files <glob>`");
    glob.split('*')
        .next()
        .expect("the glob has a literal prefix")
        .to_string()
}

/// Every `--record` a runbook hands the harness is named from the repo root.
///
/// The fourth member of this family and the last one without a check. Rounds 944
/// and 948 caught a runbook teaching a retired WORD, Round 958 a stale NUMBER,
/// and Round 961 recorded a runbook teaching a PATH the tool rejects — then left
/// it. Measured here rather than repeated: it is three runbooks, and running the
/// tool both ways settles it. The taught shape answers `error: v1/replay.json is
/// not a tracked kit record`; the repo-root-relative shape declares against the
/// real record and leaves the tree unchanged.
///
/// What makes it worth a gate rather than a fix is that the pair DISAGREES.
/// `stamp-inputs` accepts the kit-relative form — it never consults git, so it
/// resolves paths against the record's own parent — while `declare-run-tree`
/// refuses it. A runbook that teaches one shape for both verbs therefore half
/// works, and a half-working instruction is read as a different problem.
#[test]
fn every_record_a_runbook_names_is_rooted_at_the_repo() {
    let root = repo_root();
    let prefix = kit_record_prefix(&root);
    let books = runbooks(&root);
    assert!(
        !books.is_empty(),
        "no runbooks found under claudedocs/ — this gate would pass vacuously"
    );
    let mut checked = 0usize;
    let mut violations = Vec::new();
    for rel in &books {
        let text =
            std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        // Two of the three runbooks that carried this wrapped the flag away
        // from its value, so the argument is looked for on this line and the
        // next. Unlike the shared-slot scan there is no false-positive risk in
        // reading ahead: the token taken is the FIRST after the flag, so a
        // second command on the following line can never be reached unless the
        // flag genuinely had no value.
        let lines: Vec<&str> = text.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            for (i, _) in line.match_indices("--record") {
                let mut rest = String::from(&line[i + "--record".len()..]);
                if n + 1 < lines.len() {
                    rest.push(' ');
                    rest.push_str(lines[n + 1]);
                }
                let Some(arg) = rest.split_whitespace().next() else {
                    continue;
                };
                let arg = arg.trim_matches('`');
                if arg.is_empty() || arg.starts_with('<') {
                    continue; // a `<placeholder>`, not a path
                }
                checked += 1;
                if !arg.starts_with(&prefix) {
                    violations.push(format!("{rel}:{} names `{arg}`", n + 1));
                }
            }
        }
    }
    assert!(
        checked > 0,
        "read {} runbook(s) and found no `--record` argument — the scan asserted \
         nothing",
        books.len()
    );
    assert!(
        violations.is_empty(),
        "{} `--record` argument(s) are not named from the repo root:\n  {}\n\
         The harness matches a record's parent against `git ls-files {prefix}*`, \
         and git prints repo-root-relative paths, so a kit-relative name is not \
         a tracked kit record and `declare-run-tree` refuses it. `stamp-inputs` \
         ACCEPTS the same name — it never consults git — so a runbook teaching \
         one shape for both verbs half works, which reads as a different problem.",
        violations.len(),
        violations.join("\n  ")
    );
    println!("{checked} `--record` argument(s), all rooted at `{prefix}`");
}

/// The recipe the runbooks were given actually yields today's constant.
///
/// Round 958 replaced a stale literal with a derivation, which moves the failure
/// but does not remove it: the recipe greps `describe-schema`'s first line, and
/// that line belongs to the CLI, not to the runbooks. This checks the join from
/// both ends, and reads the marker OUT of the runbooks rather than restating it
/// here — a second copy of the thing being checked would agree with itself while
/// the runbooks drifted.
///
/// The second assertion is the silent one. The runbooks' `sed` is greedy
/// (`.*schema v`), so if the contract's first line ever named a second version
/// the recipe would quietly return the LAST match. That failure prints no error
/// and seeds a plausible number, which is the same shape as the defect this
/// whole family exists to catch.
#[test]
fn the_contract_line_the_runbooks_read_yields_todays_constant() {
    let root = repo_root();
    let mut markers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rel in runbooks(&root) {
        let text =
            std::fs::read_to_string(root.join(&rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        for (n, line) in text.lines().enumerate() {
            if !line.contains("describe-schema") {
                continue;
            }
            let Some(script) = line.split("1s/").nth(1) else {
                continue;
            };
            let Some(marker) = script
                .strip_prefix(".*")
                .and_then(|s| s.split("\\(").next())
            else {
                continue;
            };
            markers
                .entry(marker.to_string())
                .or_default()
                .push(format!("{rel}:{}", n + 1));
        }
    }
    assert_eq!(
        markers.len(),
        1,
        "the runbooks grep for {} different markers, so they cannot all be \
         reading the same line: {markers:?}",
        markers.len()
    );
    let (marker, sites) = markers.into_iter().next().expect("one marker");

    let contract = describe_schema();
    let first = contract
        .lines()
        .next()
        .expect("describe-schema prints at least one line");
    let hits: Vec<&str> = first
        .match_indices(&marker)
        .map(|(i, _)| &first[i..])
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "`{marker}` appears {} time(s) on the contract's first line ({first:?}). \
         The recipe in {sites:?} is greedy, so anything but exactly one match \
         makes it return the wrong number without saying so.",
        hits.len()
    );
    let read: String = hits[0][marker.len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    assert_eq!(
        read,
        mnemosyne_atomic::CURRENT_SCHEMA_VERSION.to_string(),
        "the recipe in {sites:?} reads {read:?} off the contract's first line \
         ({first:?}), but the constant is {}. Every seed those runbooks write \
         is that number.",
        mnemosyne_atomic::CURRENT_SCHEMA_VERSION
    );
    println!("{} deriving site(s) read `{marker}` -> {read}", sites.len());
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

/// THE RECOUNT OF THE RECORDED POPULATION, AND THE ONLY DOOR ONTO IT.
///
/// Every axis here answers one question about what the corpora on record
/// actually did, by walking them. Rounds 970 and 972 built these one at a time
/// as loose functions and each new consumer called whichever it wanted; Round
/// 976 added a second consumer and the four walks had three callers between
/// them. The walks are PRIVATE to this module and [`axes`] is the only thing
/// that leaves it, so "what does the tree say about this axis" has exactly one
/// answer and the compiler is what enforces that — not a convention a later
/// round has to notice.
///
/// WHY THIS IS NOT IN THE HARNESS, which is where a pure data transform over
/// the experiment artifacts otherwise belongs: `tools/experiment-harness`
/// declares its own `[workspace]` precisely so the root workspace's build, CI
/// and pre-commit gates never compile it, and every consumer of these axes is a
/// root-workspace gate. Moving them would make a root gate compile the harness
/// to satisfy a reader that does not exist yet.
mod census {
    use super::run_artifacts;
    use std::path::Path;

    /// One axis of the recorded-corpus population, with the two sides any
    /// universal claim about that axis would have to hold across.
    ///
    /// A universal is refutable EXACTLY WHEN the population is heterogeneous on
    /// its axis, so an axis with an empty side has stopped being evidence —
    /// which is why both sides are carried rather than a single count.
    pub struct PopulationAxis {
        pub id: &'static str,
        pub left_label: &'static str,
        pub left: Vec<String>,
        pub right_label: &'static str,
        pub right: Vec<String>,
    }

    impl PopulationAxis {
        /// The axis as one line, for a report a round reads instead of taking a
        /// baseline from an entry's prose.
        pub fn line(&self) -> String {
            format!(
                "{}: {}={} {}={} (first {}: {})",
                self.id,
                self.left_label,
                self.left.len(),
                self.right_label,
                self.right.len(),
                self.left_label,
                self.left.first().map_or("—", String::as_str),
            )
        }
    }

    /// The whole census as the bytes of the tracked report.
    pub fn report(root: &Path) -> String {
        let mut out = String::from(
            "# population census — what the recorded corpora actually did.\n\
             #\n\
             # GENERATED by `census::report`, checked in so that WHAT AN AXIS SAID\n\
             # AT ROUND N is answerable from this file's history instead of from a\n\
             # frozen sentence. Regenerate with:\n\
             #   MNEMOSYNE_BLESS_CENSUS=1 cargo test -p mnemosyne-cli \\\n\
             #     --test evidence_replay_smoke the_tracked_census\n",
        );
        for axis in axes(root) {
            out.push_str(&axis.line());
            out.push('\n');
        }
        out
    }

    /// Witness paths as REPO-RELATIVE.
    ///
    /// Three of the four walks build their names by joining `root`, and the
    /// fourth asks git, so the census printed absolute and relative paths side
    /// by side and read differently on every machine. That was invisible while
    /// the numbers only ever appeared in a failure message, and it surfaced the
    /// moment the census became bytes something compares.
    fn relative(root: &Path, paths: Vec<String>) -> Vec<String> {
        let prefix = format!("{}/", root.display());
        paths
            .into_iter()
            .map(|p| match p.strip_prefix(&prefix) {
                Some(rest) => rest.to_string(),
                None => p,
            })
            .collect()
    }

    /// Every axis the recorded population can be recounted on today.
    pub fn axes(root: &Path) -> Vec<PopulationAxis> {
        let (undirected, directed) = transition_rule_census(root);
        let (kind_omitted, kind_declared) = map_leg_kind_census(root);
        let (priced_by_direction, priced_one_way_only) = edge_cost_direction_census(root);
        let (scripted, file_only) = authoring_mode_census();
        let (undirected, directed) = (relative(root, undirected), relative(root, directed));
        let (kind_omitted, kind_declared) =
            (relative(root, kind_omitted), relative(root, kind_declared));
        let (priced_by_direction, priced_one_way_only) = (
            relative(root, priced_by_direction),
            relative(root, priced_one_way_only),
        );
        let (scripted, file_only) = (relative(root, scripted), relative(root, file_only));
        vec![
            PopulationAxis {
                id: "transition rules by `undirected`",
                left_label: "undirected",
                left: undirected,
                right_label: "directed",
                right: directed,
            },
            PopulationAxis {
                id: "map corpora by leg-kind declaration",
                left_label: "omitted",
                left: kind_omitted,
                right_label: "declared",
                right: kind_declared,
            },
            PopulationAxis {
                id: "cost-carrying corpora by direction-dependent pricing",
                left_label: "prices a way by direction",
                left: priced_by_direction,
                right_label: "prices no way by direction",
                right: priced_one_way_only,
            },
            PopulationAxis {
                id: "run trees by authoring mode",
                left_label: "hand script",
                left: scripted,
                right_label: "file-only",
                right: file_only,
            },
        ]
    }

    /// The one axis a citation names, or a panic that says it does not exist.
    pub fn axis<'a>(axes: &'a [PopulationAxis], id: &str) -> &'a PopulationAxis {
        axes.iter().find(|a| a.id == id).unwrap_or_else(|| {
            panic!(
                "no axis `{id}` is recounted by this tree, so a claim keyed to \
                 it rests on nothing. Recounted axes: {:?}",
                axes.iter().map(|a| a.id).collect::<Vec<_>>()
            )
        })
    }

    /// Every `class: transition` rule in the recorded corpora, split by
    /// `undirected`.
    ///
    /// Parsed, not grepped: a rules artifact is a JSON object with a `rules`
    /// array, so a store (which has no such array) contributes nothing and
    /// cannot be miscounted as a map. That distinction is the Round 951 lesson
    /// — a map is declared in a rules file and never lives in the store — and
    /// it is exactly what the instrument this test replaces did not have.
    fn transition_rule_census(root: &Path) -> (Vec<String>, Vec<String>) {
        fn walk(dir: &Path, undirected: &mut Vec<String>, directed: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    walk(&path, undirected, directed);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let Some(rules) = v.get("rules").and_then(|r| r.as_array()) else {
                    continue;
                };
                for rule in rules {
                    if rule.get("class").and_then(|c| c.as_str()) != Some("transition") {
                        continue;
                    }
                    let name = path.display().to_string();
                    if rule
                        .get("undirected")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                    {
                        undirected.push(name);
                    } else {
                        directed.push(name);
                    }
                }
            }
        }
        let (mut u, mut d) = (Vec::new(), Vec::new());
        walk(&root.join("claudedocs"), &mut u, &mut d);
        u.sort();
        d.sort();
        (u, d)
    }

    /// Whether an `adjacency` predicate names an entity kind on either leg.
    ///
    /// The kinds are declared on the PREDICATE, in the facts manifest beside the
    /// rules artifact — not in the rules artifact that names the predicate — so
    /// this reads the sibling manifests rather than the rules file that sent it
    /// looking. A corpus that declares neither leg cannot be asked which entities
    /// are places, which is the omission the containment paragraph measured.
    fn map_leg_kind_census(root: &Path) -> (Vec<String>, Vec<String>) {
        fn declared_kinds(dir: &Path) -> std::collections::HashMap<String, bool> {
            let mut out = std::collections::HashMap::new();
            let Ok(entries) = std::fs::read_dir(dir) else {
                return out;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let Some(preds) = v.get("predicates").and_then(|p| p.as_array()) else {
                    continue;
                };
                for p in preds {
                    let Some(id) = p.get("predicate_id").and_then(|i| i.as_str()) else {
                        continue;
                    };
                    let has = p.get("subject_kind").is_some_and(|k| !k.is_null())
                        || p.get("object_entity_kind").is_some_and(|k| !k.is_null());
                    *out.entry(id.to_string()).or_insert(false) |= has;
                }
            }
            out
        }

        fn walk(dir: &Path, omitted: &mut Vec<String>, declared: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    walk(&path, omitted, declared);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let Some(rules) = v.get("rules").and_then(|r| r.as_array()) else {
                    continue;
                };
                let parent = path.parent().expect("a rules file has a directory");
                let kinds = declared_kinds(parent);
                for rule in rules {
                    if rule.get("class").and_then(|c| c.as_str()) != Some("transition") {
                        continue;
                    }
                    // Only a rule that DECLARES an adjacency predicate is in scope:
                    // a transition rule without one has no map, and the claim this
                    // axis refutes is about maps.
                    let Some(adj) = rule.get("adjacency").and_then(|a| a.as_str()) else {
                        continue;
                    };
                    let name = path.display().to_string();
                    if kinds.get(adj).copied().unwrap_or(false) {
                        declared.push(name);
                    } else {
                        omitted.push(name);
                    }
                }
            }
        }
        let (mut o, mut d) = (Vec::new(), Vec::new());
        walk(&root.join("claudedocs"), &mut o, &mut d);
        o.sort();
        d.sort();
        (o, d)
    }

    /// Cost-carrying corpora split by whether ANY way is priced differently in its
    /// two directions.
    ///
    /// Round 972. A manifest whose cost rows cannot be RESOLVED to a fact's two
    /// legs is neither side: it is UNMEASURABLE, and counting it as symmetric would
    /// be the R925 trap — a filter that does not match reading as a result. The
    /// recorded case is real, `phase1-map-corpus-experiment/v1` stage-b, whose costs
    /// live in their own file with no facts beside them.
    fn edge_cost_direction_census(root: &Path) -> (Vec<String>, Vec<String>) {
        fn walk(dir: &Path, asymmetric: &mut Vec<String>, symmetric: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    walk(&path, asymmetric, symmetric);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let Some(costs) = v.get("edge_costs").and_then(|c| c.as_array()) else {
                    continue;
                };
                if costs.is_empty() {
                    continue;
                }
                // fact id -> the pair of places the edge fact joins.
                let mut legs: std::collections::HashMap<&str, (String, String)> =
                    std::collections::HashMap::new();
                for fact in v
                    .get("facts")
                    .and_then(|f| f.as_array())
                    .into_iter()
                    .flatten()
                {
                    let typed = fact.get("typed");
                    let subject = typed
                        .and_then(|t| t.get("subject"))
                        .and_then(|s| s.as_str());
                    let object = typed
                        .and_then(|t| t.get("object"))
                        .and_then(|o| o.get("id"))
                        .and_then(|i| i.as_str());
                    if let (Some(id), Some(s), Some(o)) = (
                        fact.get("fact_id").and_then(|i| i.as_str()),
                        subject,
                        object,
                    ) {
                        legs.insert(id, (s.to_string(), o.to_string()));
                    }
                }
                // Keyed by the UNORDERED pair, valued by each direction's number.
                let mut priced: std::collections::HashMap<(String, String), Vec<i64>> =
                    std::collections::HashMap::new();
                for c in costs {
                    let Some(id) = c.get("fact_id").and_then(|i| i.as_str()) else {
                        continue;
                    };
                    let Some((a, b)) = legs.get(id) else { continue };
                    let Some(n) = c.get("n").and_then(serde_json::Value::as_i64) else {
                        continue;
                    };
                    let key = if a <= b {
                        (a.clone(), b.clone())
                    } else {
                        (b.clone(), a.clone())
                    };
                    priced.entry(key).or_default().push(n);
                }
                if priced.is_empty() {
                    continue; // unmeasurable, and that is not the same as symmetric
                }
                let name = path.display().to_string();
                if priced
                    .values()
                    .any(|ns| ns.len() > 1 && ns.iter().any(|n| n != &ns[0]))
                {
                    asymmetric.push(name);
                } else {
                    symmetric.push(name);
                }
            }
        }
        let (mut a, mut s) = (Vec::new(), Vec::new());
        walk(&root.join("claudedocs"), &mut a, &mut s);
        a.sort();
        s.sort();
        (a, s)
    }

    /// Recorded run trees split by whether the author dropped to a hand-written
    /// shell script, which is what "file-only authoring" is a claim ABOUT.
    fn authoring_mode_census() -> (Vec<String>, Vec<String>) {
        let (mut scripted, mut file_only) = (Vec::new(), Vec::new());
        for f in run_artifacts() {
            if f.ends_with(".sh") {
                scripted.push(f);
            } else if f.ends_with(".json") {
                file_only.push(f);
            }
        }
        scripted.sort();
        file_only.sort();
        (scripted, file_only)
    }
}

/// The nouns that name the recorded-corpus POPULATION.
///
/// Deliberately NOT a list of every plural in the document: this contract
/// addresses a singular "the author" throughout, and the axis being banned is
/// a claim about what the corpora on record DID.
const POPULATION_NOUNS: &[&str] = &[
    "corpus",
    "corpora",
    "author",
    "authors",
    "authoring",
    "authorings",
    "arm",
    "arms",
];

const NUMBER_WORDS: &[&str] = &[
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "half",
];

/// Words a count may run through before it reaches the noun it counts —
/// "three OF THE six corpora", "one author PER arm".
const COUNT_MODIFIERS: &[&str] = &["of", "the", "blind", "recorded", "authored", "per"];

const UNIVERSAL_WORDS: &[&str] = &["every", "all", "each", "no", "none"];

const POPULATION_ADJECTIVES: &[&str] = &["blind", "recorded", "authored"];

fn is_number_word(t: &str) -> bool {
    NUMBER_WORDS.contains(&t) || (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit()))
}

/// A round pin anywhere in the sentence — `R969` or `Round 969`.
///
/// `Round 702/703` is a pin too, so the digits are read as a PREFIX rather than
/// as a whole token.
fn carries_round_pin(words: &[String]) -> bool {
    words.iter().enumerate().any(|(i, w)| {
        let three_digit_prefix =
            |t: &str| t.len() >= 3 && t[..3].chars().all(|c| c.is_ascii_digit());
        if w.starts_with('r') && w.len() >= 4 && three_digit_prefix(&w[1..]) {
            return true;
        }
        w == "round" && words.get(i + 1).is_some_and(|n| three_digit_prefix(n))
    })
}

/// The rendered contract, split into sentences and tokenised.
///
/// A COLON DOES NOT END A SENTENCE, and that is load-bearing rather than
/// incidental: this document's measured claims are written "MEASURED AT ROUND
/// 934, not supposed: three of the six corpora…", so splitting on `:` would
/// file the pin and the count it dates as two different sentences and report
/// every dated census as undated.
fn contract_sentences(doc: &str) -> Vec<Vec<String>> {
    doc.split(['.', ';'])
        .map(|s| {
            s.split_whitespace()
                .map(|w| {
                    w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                        .to_lowercase()
                })
                .filter(|w| !w.is_empty())
                .collect()
        })
        .collect()
}

/// Every census-shaped claim about the recorded corpora in the rendered
/// contract: an undated COUNT of them, or a UNIVERSAL over them.
fn census_claims(doc: &str) -> Vec<String> {
    let mut found = Vec::new();
    let pop = |w: &String| POPULATION_NOUNS.contains(&w.as_str());
    for words in contract_sentences(doc) {
        let dated = carries_round_pin(&words);
        for (i, w) in words.iter().enumerate() {
            if is_number_word(w) && !dated {
                let mut j = i + 1;
                while words
                    .get(j)
                    .is_some_and(|m| COUNT_MODIFIERS.contains(&m.as_str()) && !is_number_word(m))
                {
                    j += 1;
                }
                if words.get(j).is_some_and(pop) {
                    found.push(format!("undated count: `{}`", words[i..=j].join(" ")));
                }
            }
            let mut k = i + 1;
            while words
                .get(k)
                .is_some_and(|m| POPULATION_ADJECTIVES.contains(&m.as_str()))
            {
                k += 1;
            }
            if UNIVERSAL_WORDS.contains(&w.as_str()) && words.get(k).is_some_and(pop) {
                found.push(format!("universal: `{}`", words[i..=k].join(" ")));
            }
            if w == "the"
                && words.get(k).is_some_and(pop)
                && words.get(k + 1).is_some_and(|n| n == "on")
                && words.get(k + 2).is_some_and(|n| n == "record")
            {
                found.push(format!("universal: `{}`", words[i..=k + 2].join(" ")));
            }
        }
    }
    found
}

/// THE RECOUNT IS SOMETHING A ROUND CAN RUN, AND EVERY AXIS STILL REFUTES A
/// UNIVERSAL.
///
/// `cargo test -p mnemosyne-cli --test evidence_replay_smoke population_census
/// -- --nocapture` prints the current census. That command exists because of
/// what the record shows rounds actually do when they need one: Round 968 took
/// its baseline from Round 961's sentence and reported a move from zero when
/// the truth was two to five; Round 915 took one from Round 914's sentence;
/// Rounds 970, 972 and 976 each measured an axis by hand because there was
/// nothing to run. The numbers here are the gate's own — the same
/// [`census::axes`] the contract gate and the ledger bindings read — so a round
/// that runs this cannot get a different answer than CI does.
///
/// It gates as well as reports, and the assertion is the one Round 970's ban
/// rests on: a universal over a population is refutable EXACTLY WHEN that
/// population is heterogeneous on its axis, so an axis with an empty side has
/// stopped being evidence and the ban on that axis would have to be re-argued
/// rather than kept. This is the single home for that invariant; the contract
/// gate states the ban and this states what makes it true.
#[test]
fn the_population_census_runs_and_every_axis_still_refutes_a_universal() {
    let axes = census::axes(&repo_root());
    assert!(
        !axes.is_empty(),
        "no axis is recounted at all, so every census claim in the contract and \
         the ledger rests on nothing a program reads"
    );
    let mut report = String::from("population census (recounted now)\n");
    for axis in &axes {
        report.push_str("  ");
        report.push_str(&axis.line());
        report.push('\n');
    }
    println!("{report}");
    for axis in &axes {
        assert!(
            !axis.left.is_empty() && !axis.right.is_empty(),
            "the recorded corpora are homogeneous on `{}` ({}={}, {}={}), so a \
             universal claim about that axis is no longer refutable by this tree \
             and the ban on it has to be re-argued, not kept\n{report}",
            axis.id,
            axis.left_label,
            axis.left.len(),
            axis.right_label,
            axis.right.len()
        );
    }
}

/// WHAT AN AXIS SAID AT ROUND N IS ANSWERABLE FROM A FILE'S HISTORY, NOT FROM A
/// FROZEN SENTENCE.
///
/// The runnable census answers "what does this axis say NOW". The other half of
/// what the record keeps needing is "what did it say WHEN THAT WAS WRITTEN" —
/// the question that separates a claim which was WRONG WHEN WRITTEN from one
/// that WENT STALE UNDER a growing population. Round 970 had to settle it by
/// reasoning ("Round 934 measured truly; the sentence went stale under it") for
/// three claims, because nothing recorded the value at the time.
///
/// A checked-in report settles it: every change to the population lands in the
/// same commit as the change that caused it, so `git log -p` on this file is the
/// axis's own history. That only holds if the file cannot drift, which is what
/// this gate is — and it is the reason the file may not be hand-edited: its
/// bytes are a program's output, not a claim.
///
/// This is deliberately NOT a store field. A field would carry the same number
/// with no program deriving it — a hand-typed count in a new place, which is the
/// thing Round 959 banned and Round 975 fixed by giving the number a program to
/// come from.
#[test]
fn the_tracked_census_is_what_the_axes_say_now() {
    let root = repo_root();
    let path = root.join("claudedocs/population-census.txt");
    let fresh = census::report(&root);
    if std::env::var_os("MNEMOSYNE_BLESS_CENSUS").is_some() {
        std::fs::write(&path, &fresh).expect("write the census report");
    }
    let tracked = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        tracked, fresh,
        "`claudedocs/population-census.txt` is not what the axes say now, so \
         this file's history has stopped being the record of what each axis \
         said when. Regenerate it in the SAME commit as the change that moved \
         the population: MNEMOSYNE_BLESS_CENSUS=1 cargo test -p mnemosyne-cli \
         --test evidence_replay_smoke the_tracked_census"
    );
}

/// THE CONTRACT MAY NOT STATE AN UNDATED CENSUS OF THE CORPORA IT CANNOT
/// RECOUNT.
///
/// Round 924 wrote "every corpus written against this contract is a DIRECTED
/// map" into the surfaces a blind author reads. It was TRUE when written. Round
/// 943's two blind authors declared `undirected: true`, and the sentence shipped
/// false for twenty-five rounds, into Round 961, which restated it as "every
/// corpus on record has chosen directed", and from there into Round 968, which
/// took its baseline from that entry and reported a move from zero.
///
/// Round 969 repaired that ONE sentence and left as its own carry that nothing
/// recounted the other census-shaped claims. THERE WERE THREE, and the two this
/// gate would not have caught in Round 969's form are the reason it is now over
/// the WHOLE rendered document rather than two struct fields: the containment
/// paragraph's "HALF OF EVERY BLIND AUTHORING ON RECORD — three of six corpora"
/// (three of FIFTEEN when this ran, so the undated form overstated the defect by
/// two and a half times), and the `edge_costs` row's "which is how the corpora on
/// record were written" (the tree records ten hand-written shell scripts).
///
/// THE RULE IS DATE IT OR DROP IT, which is Round 969's own repair generalised:
/// a count of the corpora is fine as HISTORY and never as STANDING FACT, so a
/// count must carry a round pin in its sentence, and a universal — which no
/// date can rescue, since it claims the population entire — may not be written
/// at all.
///
/// The surface is the RENDERED document (Round 957): the section headings are
/// prose an author reads and a library cannot see, and they are not in the
/// struct.
///
/// WHAT THIS CANNOT DO, stated rather than implied (the Round 965 ceiling):
/// no word list separates a census ("every corpus chose directed") from a RULE
/// ("every author must register ids first"), and this bans both. That costs
/// nothing today — measured: this contract writes its rules about "an author"
/// and "the author" and says "every author" zero times — and the failure is
/// loud and rephraseable rather than silent. Prose can also assert a census in
/// words this list does not carry; what is checkable is that the phrasings
/// which ALREADY went stale here cannot come back.
#[test]
fn the_contract_states_no_undated_census_of_its_own_corpora() {
    // The recount comes through the one door, and NON-VACUITY — the reason this
    // ban is a measurement rather than a style preference — is asserted by
    // `the_population_census_runs_and_every_axis_still_refutes_a_universal`,
    // which is its single home. This test states the ban; that one states what
    // makes it true. Two homes for one invariant is what Round 970 deleted the
    // previous gate to avoid.
    let recount = census::axes(&repo_root())
        .iter()
        .map(census::PopulationAxis::line)
        .collect::<Vec<_>>()
        .join("; ");

    let claims = census_claims(&describe_schema());
    assert!(
        claims.is_empty(),
        "the rendered contract states {} census-shaped claim(s) about its own \
         corpora: {}. A count belongs in a report that is re-run, or in a \
         sentence that DATES it; a universal belongs nowhere (Round 959, Round \
         969). The tree recounts, and already refutes a universal on every axis \
         this contract has claimed about — {}",
        claims.len(),
        claims.join(", "),
        recount,
    );
}

/// Every audit-half prose unit of the frozen ledger, as `(entry, field, text)`.
fn ledger_prose() -> Vec<(String, String, String)> {
    let raw = std::fs::read_to_string(repo_root().join("docs/.atomic/workspace.atomic.json"))
        .expect("read the atomic store");
    let store: serde_json::Value = serde_json::from_str(&raw).expect("parse the atomic store");
    let entries = store
        .get("changelog_entries")
        .and_then(|v| v.as_object())
        .expect("changelog_entries");
    let mut out = Vec::new();
    for (id, entry) in entries {
        if let Some(s) = entry.get("decision_summary").and_then(|v| v.as_str()) {
            out.push((id.clone(), "decision_summary".to_string(), s.to_string()));
        }
        for field in [
            "changes_bullets",
            "verification_bullets",
            "carry_forward_bullets",
        ] {
            let Some(arr) = entry.get(field).and_then(|v| v.as_array()) else {
                continue;
            };
            for (i, b) in arr.iter().enumerate() {
                if let Some(s) = b.as_str() {
                    out.push((id.clone(), format!("{field}[{i}]"), s.to_string()));
                }
            }
        }
    }
    out
}

/// The one prose unit of the frozen ledger a citation names, or a panic that
/// says which half of the citation failed.
///
/// An entry id comes in two shapes — bare `Round 568` and titled
/// `Round 293 — <title>` — so a round is matched as the whole id or as its
/// prefix up to the separating space, never by a bare `starts_with` that would
/// read `Round 97` out of `Round 970`.
fn ledger_unit(prose: &[(String, String, String)], round: u64, fragment: &str) -> String {
    let want = format!("Round {round}");
    let hits: Vec<&(String, String, String)> = prose
        .iter()
        .filter(|(id, _, _)| *id == want || id.starts_with(&format!("{want} ")))
        .collect();
    assert!(
        !hits.is_empty(),
        "no entry `{want}` in the ledger, so this citation names nothing"
    );
    let carrying: Vec<&&(String, String, String)> = hits
        .iter()
        .filter(|(_, _, text)| text.contains(fragment))
        .collect();
    assert_eq!(
        carrying.len(),
        1,
        "`{want}` carries the fragment `{fragment}` in {} of its {} prose \
         units; the ledger is frozen, so a citation into it resolves to exactly \
         one or the citation is wrong",
        carrying.len(),
        hits.len()
    );
    carrying[0].2.clone()
}

/// The phrases that scope a claim to the recorded population — the tightening
/// a census gate would reach for once its word list alone proves too loose.
///
/// Matched as WHOLE WORDS. A substring test reads `ever` out of `every` and so
/// declares every universal already scoped, which is the reading that made the
/// first measurement of this rule wrong.
const SCOPE_MARKERS: &[&[&str]] = &[
    &["on", "record"],
    &["in", "this", "tree"],
    &["this", "tree"],
    &["ever"],
    &["yet"],
    &["recorded"],
];

fn scope_markers_in(sentence: &str) -> Vec<String> {
    let words: Vec<String> = sentence
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    SCOPE_MARKERS
        .iter()
        .filter(|m| words.windows(m.len()).any(|w| w == **m))
        .map(|m| m.join(" "))
        .collect()
}

/// The one sentence of `text` that carries `fragment`, split the way the census
/// resolver splits (a colon does not end a sentence — Round 970).
fn sentence_carrying(text: &str, fragment: &str) -> String {
    let hits: Vec<&str> = text
        .split(['.', ';'])
        .filter(|s| s.contains(fragment))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "`{fragment}` occurs in {} sentences of this unit, so naming one is \
         ambiguous",
        hits.len()
    );
    hits[0].split_whitespace().collect::<Vec<_>>().join(" ")
}

/// NO READING OF THE LEDGER'S PROSE IS BOTH COMPLETE AND CLEAN, AND THIS IS THE
/// PAIR THAT PROVES IT.
///
/// Round 970 banned census-shaped prose from the shipping contract and recorded
/// that the LEDGER was unmeasured, that its entries are frozen so a census in
/// one can never be repaired, and that the only possible form would be a gate at
/// APPEND time. Round 972 hit the same wall a second time and left the open
/// question in one sentence: whether an append-time ban is AFFORDABLE, "since
/// ledger prose is where findings live and a finding legitimately says 'four
/// blind authors across two arms'". Round 975 was the third firing.
///
/// It was measured before it was decided. Applying this file's own resolver to
/// all 722 entries — 10561 prose units — reports 97 universals and 148 counts.
/// Five readings of the same question (dropping `arm`, which the ledger uses for
/// Rust match arms and test arms; exempting quoted text, since a repair round
/// QUOTES the sentence it is repairing; requiring a scope marker) move the
/// universal count 97 → 70 → 60 → 58 → 29 and the count arm 148 → 26. Every
/// reading still fires on the oracle the record itself names — Round 961's
/// "every corpus on record has chosen directed", which Rounds 969, 970 and 972
/// each identify as the census that shipped false — so RECALL IS STABLE AND
/// PRECISION IS NOT, which is the Round 973 signature for a question this
/// record cannot settle.
///
/// The tightest reading that still fires on the oracle reports 29 sites, of
/// which 4 state no census. THE PAIR BELOW IS WHAT THAT TIGHTENING COSTS, and
/// it is why the remaining four are not simply worth tuning away: one of these
/// two frozen sentences is a census over the recorded corpora — two step
/// classes have no witness in any of them, which one newly authored corpus
/// would falsify — and the other is a distributive "each" over one run's
/// sessions. The resolver fires on both and NEITHER carries a scope marker, so
/// the scope rule drops them TOGETHER. Its precision is bought with a miss on
/// the same axis it is aimed at, and a miss in the audit trail reads as
/// assurance rather than as silence.
///
/// The COUNT half needs no ban at all, and that is structural rather than a
/// concession: an entry is filed under its own round, so every count in it is
/// dated by the id. Round 970's rule was "date it or drop it"; the ledger dates
/// it by construction, and what that leaves to enforce is the pin —
/// `r976_prefix_without_a_round_number_is_rejected` in `mnemosyne-atomic`.
#[test]
fn no_reading_of_ledger_prose_separates_a_census_from_a_distributive_each() {
    let prose = ledger_prose();
    // A census over the recorded corpora: two step classes have no witness in
    // any of them, which one newly authored corpus would falsify.
    let census = sentence_carrying(
        &ledger_unit(&prose, 935, "are 0 in every corpus"),
        "are 0 in every corpus",
    );
    // Not a census: one run's authoring sessions, distributed over.
    let not_census = sentence_carrying(
        &ledger_unit(&prose, 471, "after every authoring session"),
        "after every authoring session",
    );

    for (label, sentence) in [("census", &census), ("not a census", &not_census)] {
        let claims: Vec<String> = census_claims(sentence)
            .into_iter()
            .filter(|c| c.starts_with("universal"))
            .collect();
        assert!(
            !claims.is_empty(),
            "the resolver no longer fires on the {label} sentence, so this pair \
             no longer demonstrates anything: {sentence}"
        );
        let markers = scope_markers_in(sentence);
        assert!(
            markers.is_empty(),
            "the {label} sentence carries scope marker(s) {markers:?}, so a \
             scope rule WOULD tell this pair apart and the ban is worth \
             re-measuring rather than refused: {sentence}"
        );
    }
}

/// THE CENSUS CLAIMS THE LEDGER SHIPPED THAT THIS TREE CAN RECOUNT ARE FALSE,
/// AND THE FALSITY IS BOUND TO THE PROGRAMS RATHER THAN RESTATED IN PROSE.
///
/// Round 970 repaired the shipping contract's census claims and its two doc
/// comments in `mnemosyne-atomic`, naming three homes for one sentence and
/// calling a repair to only one of them "the half-cleanup this repo bans". Its
/// own carry then records that the LEDGER was not measured. It carries the same
/// sentences. They cannot be repaired — the ledger is append-only — so what a
/// round can do instead is make them un-inheritable: the axis that refutes each
/// one is already a program in this file, and a baseline taken from any of
/// these three sentences now fails a test that names the sentence.
///
/// Each assertion is stated in the DIRECTION of the finding rather than against
/// a fixed number, so it does not rot as the corpora grow: `every` and `no`
/// need one counterexample, and `half` needs the smaller side to stay smaller.
#[test]
fn the_ledger_census_claims_this_tree_can_recount_are_recounted() {
    let prose = ledger_prose();
    let axes = census::axes(&repo_root());

    // "every corpus on record has chosen directed" — Round 961, promoted from
    // Round 936's count of five and inherited by Round 968 as a zero baseline.
    let directedness = census::axis(&axes, "transition rules by `undirected`");
    let (undirected, directed) = (&directedness.left, &directedness.right);
    for (round, fragment) in [
        (961, "every corpus on record has chosen directed"),
        (967, "every recorded corpus chose directed"),
    ] {
        ledger_unit(&prose, round, fragment);
        assert!(
            !undirected.is_empty(),
            "Round {round} states `{fragment}` and the tree recounts {} \
             undirected transition rules against {} directed. With zero \
             undirected on record the sentence would be true again and this \
             binding would be asserting nothing",
            undirected.len(),
            directed.len()
        );
    }

    // "which is how every blind corpus on record was written" — Round 956, the
    // same universal Round 970 removed from the contract's `edge_costs` row and
    // left standing here.
    let mode = census::axis(&axes, "run trees by authoring mode");
    let (scripted, file_only) = (&mode.left, &mode.right);
    let fragment = "which is how every blind corpus on record was written";
    ledger_unit(&prose, 956, fragment);
    assert!(
        !scripted.is_empty(),
        "Round 956 states `{fragment}` and the tree carries {} hand-written \
         shell scripts under the recorded run trees against {} authored files",
        scripted.len(),
        file_only.len()
    );

    // "Half of every blind authoring on record" — Round 934, the sentence Round
    // 970 recounted in the contract as three of FIFTEEN and repaired there only.
    let leg_kind = census::axis(&axes, "map corpora by leg-kind declaration");
    let (omitted, declared) = (&leg_kind.left, &leg_kind.right);
    let fragment = "Half of every blind authoring on record";
    ledger_unit(&prose, 934, fragment);
    assert!(
        omitted.len() < declared.len(),
        "Round 934 states `{fragment}` and the tree recounts {} rules omitting \
         a leg kind against {} declaring one, so the omitting side is no longer \
         the smaller one and the word `half` has to be re-measured rather than \
         left bound here",
        omitted.len(),
        declared.len()
    );
}

/// EVERY COUNT THE LEDGER STATES IS DATED BY THE ENTRY'S OWN KEY.
///
/// This is the fact that lets the count half of Round 970's "date it or drop
/// it" be dropped for the ledger instead of enforced: a count inside `Round 934`
/// is pinned to Round 934 by where it is filed, so it is history and never a
/// standing claim. The property is only worth what the id guarantees, which is
/// why `append_changelog_entry` now demands the number and not just the prefix.
///
/// The resolver is production's — `project::parse_round_number`, what the
/// changelog projection keys on — so this cannot drift from the answer the rest
/// of the store gives.
#[test]
fn every_ledger_entry_is_dated_by_its_own_key() {
    let raw = std::fs::read_to_string(repo_root().join("docs/.atomic/workspace.atomic.json"))
        .expect("read the atomic store");
    let store: serde_json::Value = serde_json::from_str(&raw).expect("parse the atomic store");
    let entries = store
        .get("changelog_entries")
        .and_then(|v| v.as_object())
        .expect("changelog_entries");
    let undated: Vec<&String> = entries
        .keys()
        .filter(|id| mnemosyne_atomic::project::parse_round_number(id).is_none())
        .collect();
    assert!(
        undated.is_empty(),
        "{} of {} ledger entries carry no round number in their key, so every \
         count in their prose is undated and unrepairable: {:?}",
        undated.len(),
        entries.len(),
        undated
    );
    assert!(
        entries.len() > 1,
        "a ledger with one entry cannot demonstrate that the id is what dates it"
    );
}
