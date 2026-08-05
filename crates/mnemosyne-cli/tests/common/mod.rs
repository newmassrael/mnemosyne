//! The dnd-quest authored store, rebuilt the way its runbook does — THE one
//! builder, shared by every test that needs the only blind-authored branching
//! corpus this repo can load.
//!
//! Round 857 found that this fixture had stopped loading and nobody noticed for
//! ~150 rounds; `dnd_quest_map_seam_smoke.rs` exists so CI loads it every run.
//! Round 1031 needed the same store to judge quest prerequisites against real
//! authored roads, and a SECOND copy of the recipe is how the two tests would
//! come to disagree about which store they are talking about — so the recipe
//! lives here and both read it.

#![allow(dead_code)] // each test binary uses a different part of this module

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

pub fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

pub fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/mnemosyne-cli
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

/// The frozen experiment record. Read, never written: it is the blind author's
/// own output and the experiment's sha-pinned evidence.
pub fn audit_dir() -> PathBuf {
    repo_root().join("claudedocs/phase1-dnd-quest-experiment/v3/run/author")
}

pub fn run(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

pub fn run_ok(workspace: &Path, args: &[&str]) -> String {
    let out = run(workspace, args);
    assert!(
        out.status.success(),
        "{args:?} failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8(out.stdout).expect("cli output is utf-8")
}

pub fn json_report(workspace: &Path, args: &[&str]) -> serde_json::Value {
    let mut full = args.to_vec();
    full.push("--json");
    serde_json::from_str(&run_ok(workspace, &full)).expect("report json")
}

/// Any JSON this tree tracks, read as JSON — a manifest, a sidecar, a fixture.
pub fn read_json(path: &Path) -> serde_json::Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("{} unreadable: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not json: {e}", path.display()))
}

/// The tracked fact manifest — the migrated half of the frozen record.
pub fn dnd_quest_facts() -> serde_json::Value {
    read_json(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dnd-quest/facts.json"))
}

/// Every authored corpus this repository TRACKS: a fact manifest with a
/// `sections.json` and an `order.json` beside it. Asked of `git ls-files`, not
/// of the working directory — an untracked stray is not evidence about what an
/// author ships, and a walk over the filesystem would count it.
///
/// Round 1036 built this to widen a refuter's population from one corpus to
/// forty-three; it lives here because a second copy is how two tests come to
/// disagree about which stores an author shipped.
pub fn authored_corpora() -> Vec<PathBuf> {
    let listed = Command::new("git")
        .args(["ls-files", "claudedocs"])
        .current_dir(repo_root())
        .output()
        .expect("git ls-files");
    assert!(listed.status.success(), "git ls-files must exit 0");
    let root = repo_root();
    let mut out: Vec<PathBuf> = String::from_utf8(listed.stdout)
        .expect("git output is utf-8")
        .lines()
        .filter(|path| path.ends_with("/facts.json"))
        .filter_map(|path| root.join(path).parent().map(Path::to_path_buf))
        .filter(|dir| dir.join("sections.json").exists() && dir.join("order.json").exists())
        .collect();
    out.sort();
    out.dedup();
    assert!(
        out.len() > 20,
        "the corpus sweep found {} authored corpora, which is a listing that \
         stopped working rather than a repository that emptied",
        out.len()
    );
    out
}

/// One authored store, built and ready to be asked.
pub struct AuthoredStore {
    pub name: String,
    pub ws: TempDir,
}

/// Every authored store this repository can actually ASK, and the names of the
/// ones it cannot: the tracked corpora, PLUS the migrated dnd-quest record.
///
/// The migrated record has to be named separately and that is not a detail. It
/// is the richest store this tree holds — four roads, quests, dangling setups
/// on every road — and its own TRACKED manifest is the pre-migration file that
/// stopped loading (the rot R857 found), so a sweep of tracked corpora alone
/// EXCLUDES it. Round 1036 lost three refutations to exactly that omission and
/// had to add it back by hand; this is that lesson as a shared resolver rather
/// than as a line each walk remembers to write.
pub fn authored_stores() -> (Vec<AuthoredStore>, Vec<String>) {
    let mut loadable = Vec::new();
    let mut unloadable = Vec::new();
    for dir in authored_corpora() {
        let name = dir
            .strip_prefix(repo_root())
            .unwrap_or(&dir)
            .display()
            .to_string();
        match corpus_workspace_try(&dir, &read_json(&dir.join("facts.json"))) {
            Ok(ws) => loadable.push(AuthoredStore { name, ws }),
            Err(_) => unloadable.push(name),
        }
    }
    loadable.push(AuthoredStore {
        name: "the migrated dnd-quest record".to_string(),
        ws: dnd_quest_workspace_from(&dnd_quest_facts()),
    });
    (loadable, unloadable)
}

/// Rebuild the store the way the experiment's runbook does — fresh seed, then
/// the manifests — with the three unchanged manifests taken from the frozen
/// record and the fact manifest supplied by the caller, so a test can author a
/// DEFECT into the store an author could equally have authored (the manifest is
/// the authoring path, and it validates: a corruption the import rejects is a
/// corruption no author could have shipped).
pub fn dnd_quest_workspace_from(facts: &serde_json::Value) -> TempDir {
    dnd_quest_workspace_try(facts).unwrap_or_else(|e| panic!("the fact manifest must import: {e}"))
}

/// The same recipe, with the fact import's REFUSAL handed back rather than
/// asserted. A walk that corrupts the manifest needs "the write path rejected
/// this" as a VERDICT about the corruption, not as a failure of the walk.
pub fn dnd_quest_workspace_try(facts: &serde_json::Value) -> Result<TempDir, String> {
    for name in ["sections.json", "order.json", "narrative-rules.json"] {
        let src = audit_dir().join(name);
        assert!(
            src.exists(),
            "the frozen record must hold {}",
            src.display()
        );
    }
    corpus_workspace_try(&audit_dir(), facts)
}

/// The same recipe over ANY authored corpus directory this tree tracks. The
/// dnd-quest builder is this one with the frozen record's path: a corpus is a
/// `sections.json` + `order.json` beside a fact manifest, and nothing about the
/// recipe is specific to which author wrote it. `narrative-rules.json` is
/// optional — not every corpus declares rules, and a config naming a file that
/// is not there is a load failure rather than a corpus without rules.
pub fn corpus_workspace_try(dir: &Path, facts: &serde_json::Value) -> Result<TempDir, String> {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).expect("mkdir");

    let mut rules = false;
    for name in ["sections.json", "order.json", "narrative-rules.json"] {
        let src = dir.join(name);
        if !src.exists() {
            continue;
        }
        fs::copy(&src, ws.join(name)).map_err(|e| format!("copy {}: {e}", src.display()))?;
        rules |= name == "narrative-rules.json";
    }
    fs::write(
        ws.join("facts.json"),
        serde_json::to_string(facts).expect("facts serialize"),
    )
    .expect("write facts");

    fs::write(
        ws.join("mnemosyne.toml"),
        format!(
            "[workspace]\n[continuity]\ncanon_order_path = \"order.json\"\n{}",
            if rules {
                "rules_path = \"narrative-rules.json\"\n"
            } else {
                ""
            }
        ),
    )
    .expect("write config");
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::json!({
            "schema_version": 23,
            "sections": {},
            "changelog_entries": {}
        })
        .to_string(),
    )
    .expect("write seed");

    for import in [
        ["import-sections", "--manifest", "sections.json"],
        ["import-facts", "--manifest", "facts.json"],
    ] {
        let out = run(ws, &import);
        if !out.status.success() {
            return Err(format!(
                "{}: {}",
                import[0],
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
    }
    Ok(tmp)
}

/// The authored store exactly as the blind author left it.
pub fn dnd_quest_workspace() -> TempDir {
    dnd_quest_workspace_from(&dnd_quest_facts())
}

// ==========================================================================
// THE SHIPPED READ PANEL (Round 1034, shared here in Round 1039).
//
// The panel is DERIVED from `--help`, never curated: every advertised
// `report-*` / `validate-*` verb is asked, and one that cannot be asked
// without inventing an argument the corpus does not supply is excluded BY
// THAT MEASUREMENT, with its refusal returned rather than filtered away.
//
// It lives here because two walks now need it — the play-break class census
// and the read-agreement population — and a second copy is how they would
// come to disagree about which surface a consumer actually has.
// ==========================================================================

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

/// The sidecar the import writes, relative to the workspace root.
pub const SIDECAR: &str = "docs/.atomic/workspace.atomic.json";

/// The store as the import left it, read back as the store's own serialization.
pub fn read_sidecar(ws: &Path) -> serde_json::Value {
    read_json(&ws.join(SIDECAR))
}

/// Every telling the corpus declares — the only `--telling` arguments a walk
/// may supply, since an id the corpus never declared is an invented argument.
/// THE one definition; [`telling_of`] is this for a caller that needs exactly
/// one and treats the ambiguity as a panic.
pub fn declared_tellings(store: &AtomicStore) -> Vec<String> {
    store
        .disclosure_plans
        .keys()
        .map(ToString::to_string)
        .collect()
}

/// The corpus's own telling, read from the store rather than named here — the
/// walk supplies no argument the corpus did not declare.
pub fn telling_of(store: &AtomicStore) -> String {
    let mut declared = declared_tellings(store).into_iter();
    let telling = declared
        .next()
        .expect("the corpus declares a disclosure plan");
    assert_eq!(
        declared.next(),
        None,
        "the corpus declares more than one telling, so `the` telling is no \
         longer derivable — the walk would have to choose, which is the \
         invented argument this panel refuses to make"
    );
    telling
}

/// One shipped read, and the arguments the CORPUS can answer it with.
pub struct Read {
    pub verb: String,
    pub args: Vec<String>,
}

impl Read {
    pub fn argv(&self) -> Vec<&str> {
        let mut argv = vec![self.verb.as_str()];
        argv.extend(self.args.iter().map(String::as_str));
        argv.push("--json");
        argv
    }
}

/// Every `report-*` / `validate-*` verb the shipped help advertises. Read from
/// the token that FOLLOWS the program path on each usage line, so a verb named
/// in a note's prose is not mistaken for a dispatchable one.
pub fn advertised_reads(ws: &Path) -> BTreeSet<String> {
    let help = run(ws, &["--help"]);
    assert!(help.status.success(), "--help must exit 0");
    let help = String::from_utf8(help.stdout).expect("help is utf-8");
    let bin = cli_binary();
    let verbs: BTreeSet<String> = help
        .lines()
        .filter_map(|line| {
            let mut tokens = line.split_whitespace().skip_while(|t| *t != bin);
            tokens.next()?;
            tokens.next().map(str::to_string)
        })
        .filter(|verb| verb.starts_with("report-") || verb.starts_with("validate-"))
        .collect();
    assert!(
        verbs.len() > 20,
        "the panel derivation read {} verbs out of the shipped help, which is \
         a parse that stopped working rather than a CLI that shrank",
        verbs.len()
    );
    verbs
}

/// Ask every advertised read at BASELINE, bare first and then with the corpus's
/// own telling. A read that still refuses needs an argument this corpus does
/// not supply; it is excluded BY THAT MEASUREMENT, and its refusal is returned
/// to be printed rather than curated away.
pub fn panel(ws: &Path, telling: &str) -> (Vec<Read>, Vec<(String, String)>) {
    let mut asked = Vec::new();
    let mut unaskable = Vec::new();
    for verb in advertised_reads(ws) {
        let candidates = [
            Vec::new(),
            vec!["--telling".to_string(), telling.to_string()],
        ];
        let mut refusal = None;
        for args in candidates {
            let read = Read {
                verb: verb.clone(),
                args,
            };
            let out = run(ws, &read.argv());
            if out.status.success() {
                asked.push(read);
                refusal = None;
                break;
            }
            refusal.get_or_insert_with(|| {
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .unwrap_or("(no stderr)")
                    .to_string()
            });
        }
        if let Some(reason) = refusal {
            unaskable.push((verb, reason));
        }
    }
    (asked, unaskable)
}

/// One read's answer about one store. A verb that takes `--json` and answers in
/// prose anyway can still be compared for difference, but it holds no list, so
/// it can never be asked whether it STARTED SAYING SOMETHING — a limit of the
/// panel that is named and printed rather than absorbed.
pub enum Answer {
    Json(serde_json::Value),
    Prose(String),
}

impl Answer {
    pub fn read(stdout: Vec<u8>) -> Self {
        let text = String::from_utf8(stdout).expect("cli output is utf-8");
        match serde_json::from_str(&text) {
            Ok(json) => Answer::Json(json),
            Err(_) => Answer::Prose(text),
        }
    }
}

/// What every advertised read said about one store.
pub struct Panelled {
    pub failed: Vec<String>,
    pub answers: BTreeMap<String, Answer>,
}

pub fn ask_panel(ws: &Path, panel: &[Read]) -> Panelled {
    let mut failed = Vec::new();
    let mut answers = BTreeMap::new();
    for read in panel {
        let out = run(ws, &read.argv());
        if out.status.success() {
            answers.insert(read.verb.clone(), Answer::read(out.stdout));
        } else {
            failed.push(read.verb.clone());
        }
    }
    Panelled { failed, answers }
}

// ==========================================================================
// THE DERIVED CORRUPTION POPULATION (Round 1033, shared here in Round 1040).
//
// Nothing here names a defect class: the population is the quest layer's facts
// and, per fact, the legs it ACTUALLY carries. Each edit goes through the real
// write path, so what is judged is a move an author could commit.
// ==========================================================================

/// One authorable corruption: which fact, which leg, and the edit itself.
pub struct Corruption {
    pub fact: String,
    pub leg: &'static str,
    pub apply: Box<dyn Fn(&mut serde_json::Value)>,
}

/// Swap an entity out of a fact's `entities` list and the replacement in — the
/// R446 invariant the write path enforces, so a claim retarget carries it.
/// Without this the corruption is refused, and a refused corruption is not a
/// move an author could make.
fn swap_entity(fact: &mut serde_json::Value, from: &str, to: &str) {
    let list = fact["entities"].as_array_mut().expect("entities array");
    list.retain(|e| e != from);
    let to_value = serde_json::Value::from(to);
    if !list.contains(&to_value) {
        list.push(to_value);
    }
}

/// DERIVE the corruption population: the quest layer's facts, and per fact the
/// legs it actually carries. Nothing here names a defect class — the classes
/// are whatever the legs turn out to be.
pub fn corruptions(store: &AtomicStore, facts_json: &serde_json::Value) -> Vec<Corruption> {
    let structural =
        mnemosyne_validate::continuity::structural_fact_ids(store).expect("quest plumbing derives");
    // The quest layer = the plumbing plus whatever pays it off (a completion
    // fact credits a giving setup, and both are the runtime's business).
    let mut population: BTreeSet<String> = structural.clone();
    for (id, fact) in &store.narrative_facts {
        if fact
            .pays_off
            .iter()
            .any(|t| structural.contains(t.as_str()))
        {
            population.insert(id.to_string());
        }
    }

    // The alternatives a retarget can choose, derived from what the store
    // already uses in that role — so the corruption stays type-plausible.
    let mut objects_of: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut predicates: BTreeSet<String> = BTreeSet::new();
    for fact in store.narrative_facts.values() {
        let Some(claim) = &fact.typed else { continue };
        if let mnemosyne_core::TypedObject::Entity { id } = &claim.object {
            predicates.insert(claim.predicate.to_string());
            objects_of
                .entry(claim.predicate.to_string())
                .or_default()
                .insert(id.to_string());
        }
    }

    let mut out = Vec::new();
    for entry in facts_json["facts"].as_array().expect("facts array") {
        let id = entry["fact_id"].as_str().expect("fact_id").to_string();
        if !population.contains(&id) {
            continue;
        }
        let fact = &store.narrative_facts[&id.as_str().into()];

        // The legs, each guarded by whether this fact HAS it.
        if let Some(claim) = &fact.typed {
            if let mnemosyne_core::TypedObject::Entity { id: object } = &claim.object {
                let object = object.to_string();
                let predicate = claim.predicate.to_string();
                // Leg 1 — the claim points at a different entity of the same role.
                if let Some(alt) = objects_of[&predicate]
                    .iter()
                    .find(|c| **c != object)
                    .cloned()
                {
                    let from = object.clone();
                    out.push(Corruption {
                        fact: id.clone(),
                        leg: "typed.object",
                        apply: Box::new(move |f| {
                            f["typed"]["object"]["id"] = alt.as_str().into();
                            swap_entity(f, &from, &alt);
                        }),
                    });
                }
                // Leg 2 — the claim carries a different predicate.
                if let Some(alt) = predicates.iter().find(|p| **p != predicate).cloned() {
                    out.push(Corruption {
                        fact: id.clone(),
                        leg: "typed.predicate",
                        apply: Box::new(move |f| f["typed"]["predicate"] = alt.as_str().into()),
                    });
                }
            }
        }
        // Leg 3 — one payoff edge goes missing.
        if !fact.pays_off.is_empty() {
            out.push(Corruption {
                fact: id.clone(),
                leg: "pays_off",
                apply: Box::new(|f| {
                    f["pays_off"].as_array_mut().expect("pays_off").remove(0);
                }),
            });
        }
        // Leg 4 — a setup stops expecting its payoff.
        if fact.payoff_expectation == mnemosyne_core::PayoffExpectation::Expected {
            out.push(Corruption {
                fact: id.clone(),
                leg: "payoff_expectation",
                apply: Box::new(|f| {
                    f.as_object_mut()
                        .expect("fact object")
                        .remove("payoff_expectation");
                }),
            });
        }
        // Leg 5 — a backreference goes missing.
        if fact.evidence.len() > 1 {
            out.push(Corruption {
                fact: id.clone(),
                leg: "evidence",
                apply: Box::new(|f| {
                    f["evidence"].as_array_mut().expect("evidence").remove(0);
                }),
            });
        }
    }
    out
}
