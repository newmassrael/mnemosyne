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
    /// The corpus directory the recipe read — the `sections.json` / `order.json`
    /// / `narrative-rules.json` half of this store.
    pub dir: PathBuf,
    /// The fact manifest this store was built from, as the author wrote it.
    ///
    /// Carried beside the built workspace so a walk can REBUILD the same corpus
    /// with one thing changed ([`corpus_workspace_try`]) — the authoring path an
    /// author could equally have taken, which is what makes a perturbation
    /// evidence about the store rather than about a hand-edited sidecar.
    pub facts: serde_json::Value,
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
        let facts = read_json(&dir.join("facts.json"));
        match corpus_workspace_try(&dir, &facts) {
            Ok(ws) => loadable.push(AuthoredStore {
                name,
                ws,
                dir,
                facts,
            }),
            Err(_) => unloadable.push(name),
        }
    }
    loadable.push(AuthoredStore {
        name: "the migrated dnd-quest record".to_string(),
        ws: dnd_quest_workspace_from(&dnd_quest_facts()),
        dir: audit_dir(),
        facts: dnd_quest_facts(),
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

/// The SAME corpus recipe over manifests a test writes itself, for the shapes
/// no author shipped (Round 1052).
///
/// A contract asked only of the authored corpora is asked only about what those
/// authors happened to write, and an arm no corpus reaches is a claim with no
/// evidence behind it. The corpora hold no confluence that also declares a
/// telling, no section outside the order, and no scene with nothing in it — so
/// a law about any of those reads as held when nothing exercised it. The R1041
/// precedent is the answer: what the corpora cannot make, the TREE makes, and
/// it goes through the recipe an author would have used rather than a
/// hand-written sidecar, so the store it produces is one an author could ship.
pub fn constructed_corpus(
    sections: &serde_json::Value,
    order: &serde_json::Value,
    facts: &serde_json::Value,
) -> Result<TempDir, String> {
    let manifests = TempDir::new().expect("tempdir");
    for (name, value) in [("sections.json", sections), ("order.json", order)] {
        fs::write(
            manifests.path().join(name),
            serde_json::to_string(value).expect("manifest serialize"),
        )
        .map_err(|e| format!("write {name}: {e}"))?;
    }
    corpus_workspace_try(manifests.path(), facts)
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

/// Every id this store registers — what a read could be talking about.
///
/// Read from the STORE rather than from the reads, so a read inventing an id it
/// never registered is invisible here rather than counted as a subject.
///
/// `main` is in it and that is not a detail. It is a world every store has
/// whether or not it registers a branch (the [`values_for`] rule), and a walk
/// that leaves it out cannot recognise a row keyed by road. Two walks derived
/// this set by hand and they disagreed about exactly that id; Round 1055
/// measured what the disagreement cost, by addressing every row of every array
/// a shipped read emits: 25 rows across three reads — the quest graph's
/// `locators`, the disclosure coverage's `inert_reveal_pins`, the continuity
/// report's `quest_prerequisite_judgements` — are keyed by a world, and a walk
/// blind to `main` fell back to addressing them by the position they happen to
/// sit at.
pub fn registered_ids(store: &AtomicStore) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    out.extend(store.narrative_facts.keys().map(ToString::to_string));
    out.extend(store.entities.keys().map(ToString::to_string));
    out.extend(store.sections.keys().map(ToString::to_string));
    out.extend(store.branches.keys().map(ToString::to_string));
    out.insert(mnemosyne_core::MAIN_BRANCH.to_string());
    out
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

    /// The read AND the question it was asked, as one key.
    ///
    /// A panel keyed by verb alone can hold a read at exactly ONE point of its
    /// argument space, and Round 1051 measured what that costs: asked at a
    /// single frame, `report-frame-view` mentioned 17 subjects and answered
    /// about NONE of them, because the corruption population lives in a frame
    /// the probe did not ask. The label is what lets one verb appear once per
    /// question; callers that want a verdict about the READ aggregate over it.
    pub fn label(&self) -> String {
        if self.args.is_empty() {
            return self.verb.clone();
        }
        format!("{} {}", self.verb, self.args.join(" "))
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

/// One flag on one verb's usage line, and how to give it a value this corpus
/// can actually supply.
pub struct Flag {
    pub name: String,
    /// `false` = a boolean flag (no value token follows it).
    pub takes_value: bool,
    /// `true` when the usage line lists it OUTSIDE brackets — it must be on
    /// every probe, so a differential is between two values rather than between
    /// present and absent.
    pub required: bool,
}

/// Every advertised read's usage line, as `verb -> the tail after the verb`.
///
/// Read from the line the shipped `--help` prints, never from a table here: a
/// verb that grows a flag is covered the run it ships, which is the property a
/// hand list cannot have (the R1046 lesson — a population keyed by a hand list
/// is blind to the element nobody wrote down).
pub fn usage_lines(ws: &Path) -> BTreeMap<String, String> {
    let help = run(ws, &["--help"]);
    assert!(help.status.success(), "--help must exit 0");
    let help = String::from_utf8(help.stdout).expect("help is utf-8");
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for line in help.lines() {
        let mut tokens = line.split_whitespace().skip_while(|t| *t != cli_binary());
        if tokens.next().is_none() {
            continue;
        }
        let Some(verb) = tokens.next() else { continue };
        if !(verb.starts_with("report-") || verb.starts_with("validate-")) {
            continue;
        }
        out.entry(verb.to_string())
            .or_insert_with(|| tokens.collect::<Vec<_>>().join(" "));
    }
    out
}

/// The flags on a verb's usage line, in the order the line lists them.
pub fn flags_of(usage: &str) -> Vec<Flag> {
    let mut out: Vec<Flag> = Vec::new();
    let bytes: Vec<&str> = usage.split_whitespace().collect();
    for (index, token) in bytes.iter().enumerate() {
        let bare = token.trim_start_matches('[').trim_end_matches(']');
        if !bare.starts_with("--") {
            continue;
        }
        // A value token follows when the next token is a `<placeholder>` or an
        // `a|b|c` alternation (the severity flags print the members inline).
        let next = bytes.get(index + 1).copied().unwrap_or_default();
        let next_bare = next.trim_start_matches('[').trim_end_matches(']');
        let takes_value = next_bare.starts_with('<') || next_bare.contains('|');
        // Required = the flag token itself is not bracketed. `[--world <b>]`
        // opens the bracket on the flag; `--telling <id>` does not.
        let required = !token.starts_with('[');
        if out.iter().any(|f| f.name == bare) {
            continue;
        }
        out.push(Flag {
            name: bare.to_string(),
            takes_value,
            required,
        });
    }
    out
}

/// The values this corpus can supply for a flag, read from the store itself.
///
/// An id the corpus never declared is an invented argument, and the panel's
/// rule since R1034 is that the walk supplies none. `main` is the exception the
/// STORE makes rather than any walk: it is a world every store has whether or
/// not it registers a branch.
pub fn values_for(flag: &str, store: &AtomicStore) -> Vec<String> {
    let ids = |set: Vec<String>| set;
    match flag {
        "--telling" => ids(declared_tellings(store)),
        "--world" | "--branch" => {
            let mut out: Vec<String> = store.branches.keys().map(ToString::to_string).collect();
            out.push("main".to_string());
            out.sort();
            out.dedup();
            out
        }
        "--entity" => ids(store.entities.keys().map(ToString::to_string).collect()),
        "--at" | "--target" => ids(store.sections.keys().map(ToString::to_string).collect()),
        "--frame" => {
            let mut out: BTreeSet<String> = BTreeSet::new();
            for fact in store.narrative_facts.values() {
                out.insert(fact.frame.to_string());
            }
            out.into_iter().collect()
        }
        // The declared severity vocabulary the usage lines print themselves.
        "--severity" | "--severity-missing" | "--interval-severity" => {
            vec!["warn".to_string(), "info".to_string()]
        }
        _ => Vec::new(),
    }
}

/// One road as the shipped manuscript reads it.
pub struct RoadLine {
    /// The scenes this road plays through, in order.
    pub scenes: BTreeSet<String>,
    /// The last of them — where the road has all of its history behind it.
    pub end: String,
}

/// Each road's playthrough, read from the shipped manuscript.
///
/// This is the ORACLE FOR THE CANON AXIS, and it reaches what the fork tree
/// cannot: a fork inherits its parent's history only UP TO the fork point, and
/// the parent's later scenes are exactly the ones the child's manuscript does
/// not play. So "which scenes are on this road" is the departure bound, stated
/// by a read rather than recomputed from the membership lattice.
///
/// A road whose manuscript holds no scene has no end and is returned by its
/// absence, counted by the caller.
pub fn road_lines(ws: &Path) -> BTreeMap<String, RoadLine> {
    let mut out = BTreeMap::new();
    let out_bytes = run(ws, &["report-playthrough-manuscript", "--json"]);
    if !out_bytes.status.success() {
        return out;
    }
    let Ok(manuscript) = serde_json::from_slice::<serde_json::Value>(&out_bytes.stdout) else {
        return out;
    };
    for (road, world) in manuscript["worlds"].as_object().into_iter().flatten() {
        let scenes: Vec<String> = world["scenes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|scene| scene["section"].as_str().map(ToString::to_string))
            .collect();
        let Some(end) = scenes.last().cloned() else {
            continue;
        };
        out.insert(
            road.clone(),
            RoadLine {
                scenes: scenes.into_iter().collect(),
                end,
            },
        );
    }
    out
}

/// The argv every probe of a verb starts from: each REQUIRED flag at a value
/// this corpus declares. `None` when a required flag names something the corpus
/// does not supply, which is a measurement about the corpus rather than a skip —
/// the caller counts it by name.
///
/// A required flag whose vocabulary IS the section registry is a CANON
/// COORDINATE and takes the END of the default road rather than the first
/// section id (Round 1051). The two are not interchangeable: the first section
/// is where the story STARTS, and a view asked there holds almost nothing, so a
/// read probed at it answers about a fraction of the subjects it can answer
/// about. The end of the road is where the store has all of its history behind
/// it. If the manuscript cannot say where a road ends — no declared order — the
/// first section is the honest fallback and the caller sees the same `Some` it
/// always did.
pub fn baseline_argv(flags: &[Flag], store: &AtomicStore, ws: &Path) -> Option<Vec<String>> {
    let sections = values_for("--at", store);
    let mut base: Vec<String> = Vec::new();
    let mut default_end: Option<Option<String>> = None;
    for flag in flags.iter().filter(|f| f.required) {
        let values = values_for(&flag.name, store);
        let canon_coordinate = flag.takes_value && !sections.is_empty() && values == sections;
        let chosen = if canon_coordinate {
            let end = default_end
                .get_or_insert_with(|| {
                    road_lines(ws)
                        .get(mnemosyne_core::MAIN_BRANCH)
                        .map(|line| line.end.clone())
                })
                .clone();
            end.or_else(|| values.first().cloned())
        } else {
            values.first().cloned()
        };
        match chosen {
            Some(value) if flag.takes_value => {
                base.push(flag.name.clone());
                base.push(value);
            }
            // A required flag with no value this corpus declares (a file path,
            // an id it does not hold).
            _ if flag.takes_value => return None,
            _ => base.push(flag.name.clone()),
        }
    }
    Some(base)
}

// ==========================================================================
// THE ROAD AXIS (Round 1049, shared here in Round 1050).
//
// Which flags take a road, and whether the ANSWER is keyed by one — the
// discriminator that separates a SELECTOR (a filter picking part of an answer
// that holds one entry per road) from a COORDINATE (a flag that moves the
// point the WHOLE answer is evaluated AT). Two walks now judge those two kinds
// and they must agree about which read is which, so the discriminator is one
// definition rather than a copy in each.
// ==========================================================================

/// The flags on one usage line whose value vocabulary IS this corpus's road
/// registry — derived from the shared [`values_for`] rather than spelled per
/// verb, so `--world` and `--branch` are found the same way and a third
/// road-taking flag joins the run it ships (the R1046 lesson).
pub fn road_filters<'a>(flags: &'a [Flag], store: &AtomicStore) -> Vec<&'a Flag> {
    let roads = values_for("--world", store);
    flags
        .iter()
        .filter(|flag| flag.takes_value && values_for(&flag.name, store) == roads)
        .collect()
}

/// How a structure carries the road it is about — the three encodings the
/// shipped reads use, derived from the value rather than named per verb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keyed {
    /// An object whose every key is a registered road (`worlds`, `per_world`).
    Map,
    /// An array of road ids (the quest graph's sorted per-world key set).
    Ids,
    /// An array of records, each ABOUT one road under this field (the quest
    /// graph's `locators`, one per world where a giving fact is disclosed).
    Records(String),
}

/// How this value carries roads, if it carries them at all.
///
/// Read from the UNFILTERED side by callers that compare two answers: it is the
/// answer that says what shape the read has, and the filtered side is what has
/// to match it.
pub fn road_keying(value: &serde_json::Value, roads: &BTreeSet<String>) -> Option<Keyed> {
    match value {
        serde_json::Value::Object(map) if !map.is_empty() => map
            .keys()
            .all(|key| roads.contains(key))
            .then_some(Keyed::Map),
        serde_json::Value::Array(items) if !items.is_empty() => {
            if items
                .iter()
                .all(|item| item.as_str().is_some_and(|id| roads.contains(id)))
            {
                return Some(Keyed::Ids);
            }
            // A record is about one road when ONE of its fields holds a road
            // id, in every element. Two such fields would leave the caller
            // choosing which one the filter means, which is a judgement it must
            // not make silently — it says so instead.
            let carriers: Vec<&String> = items[0].as_object()?.keys().collect();
            let carriers: Vec<String> = carriers
                .into_iter()
                .filter(|key| {
                    items.iter().all(|item| {
                        item.get(key)
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|id| roads.contains(id))
                    })
                })
                .cloned()
                .collect();
            match carriers.len() {
                1 => Some(Keyed::Records(carriers[0].clone())),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The roads a value holds under a known keying — read WITHOUT re-testing the
/// shape, because a filter that narrows a structure to nothing leaves an empty
/// one, and empty must not read as "no longer that structure".
pub fn roads_in(value: &serde_json::Value, keyed: &Keyed) -> Option<Vec<String>> {
    match (keyed, value) {
        (Keyed::Map, serde_json::Value::Object(map)) => Some(map.keys().cloned().collect()),
        (Keyed::Ids, serde_json::Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(ToString::to_string))
            .collect(),
        (Keyed::Records(field), serde_json::Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.get(field)
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
            })
            .collect(),
        _ => None,
    }
}

/// Whether this answer is ABOUT roads at all — whether a road id appears
/// anywhere in it as a KEY rather than only as a value.
///
/// This is what separates a SELECTOR from a COORDINATE, and it is derived
/// rather than declared. `report-playthrough-manuscript --world` picks a road
/// out of an answer that holds one entry per road; `report-frame-view --branch`
/// takes the same vocabulary and means something else entirely — it moves the
/// coordinate the whole answer is evaluated AT, so its `not_holding` count
/// legitimately RISES on a road where fewer facts hold. Holding a coordinate to
/// "the roads you keep say what they said" would be refuted by the shipped
/// design on the first run, so the two are judged by different contracts and
/// this is the one line that decides which.
pub fn answer_is_keyed_by_road(value: &serde_json::Value, roads: &BTreeSet<String>) -> bool {
    if road_keying(value, roads).is_some() {
        return true;
    }
    match value {
        serde_json::Value::Object(map) => map
            .values()
            .any(|value| answer_is_keyed_by_road(value, roads)),
        serde_json::Value::Array(items) => items
            .iter()
            .any(|value| answer_is_keyed_by_road(value, roads)),
        _ => false,
    }
}

/// Every top-level scalar field of an answer, as `(key, rendered value)`.
/// Nested objects are the DATA — an id inside `worlds` is what the read is
/// talking about, not a record of what it was asked.
pub fn provenance_fields(answer: &serde_json::Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(map) = answer.as_object() {
        for (key, value) in map {
            let rendered = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => "null".to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => continue,
            };
            out.insert(key.clone(), rendered);
        }
    }
    out
}

/// The top-level fields that TRACK the argument — carrying the value in the
/// probe that supplied it, and something else in the probe that did not. The
/// answer's RECORD of what it was asked (Round 1048).
///
/// The "something else" matters as much as the match. A field that reads `main`
/// under both probes is a constant, not provenance; a field that reads `main`
/// under `--world main` and `null` without it is the read saying what it was
/// asked. A missing key counts as the absent side: the sibling that has named
/// its telling since R556 spells `null`, and this resolver holds the weaker of
/// the two encodings so it measures the surface rather than a house style.
///
/// THE one definition, because two walks now need it: the provenance census
/// asks whether the record exists, and the filter walk has to take the record
/// OUT before asking what the answer said — a second copy is how the two would
/// come to disagree about which field is a record and which is substance.
pub fn record_of(
    with: &serde_json::Value,
    without: &serde_json::Value,
    value: Option<&str>,
) -> BTreeSet<String> {
    let taken = provenance_fields(with);
    let bare = provenance_fields(without);
    taken
        .iter()
        .filter(|(key, said)| {
            let matches_value = match value {
                Some(v) => *said == v,
                // A boolean flag: the field is `true` where it was passed.
                None => *said == "true",
            };
            matches_value && bare.get(*key) != Some(said)
        })
        .map(|(key, _)| key.clone())
        .collect()
}

/// The answer with its record of the argument removed — everything the read
/// SAID, as opposed to what it noted about being asked.
///
/// This split is what keeps a differential from being circular. RECORDING an
/// argument makes two answers differ by that record, so "if the answers differ
/// the answer must name the argument" would be satisfied by any field that
/// names itself. Only the tracking fields come out — a scalar like the
/// manuscript's `facts` count is substance and stays in, so an argument that
/// moved only it would still be measured.
pub fn substance(answer: &serde_json::Value, record: &BTreeSet<String>) -> serde_json::Value {
    match answer.as_object() {
        Some(map) => serde_json::Value::Object(
            map.iter()
                .filter(|(key, _)| !record.contains(*key))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        None => answer.clone(),
    }
}

/// Every argv of a verb's REQUIRED arguments the corpus can supply: the
/// cartesian product of each required flag's vocabulary, with a section-valued
/// flag pinned to the canon coordinate [`baseline_argv`] chooses.
///
/// One argv per read is a SAMPLE of its argument space, and Round 1051 measured
/// the cost of sampling: `report-frame-view` asked at one frame mentions 17
/// subjects and answers about NONE, because the corruption population lives in
/// a frame that probe did not ask. A read whose argument selects WHICH facts it
/// can see needs every value the corpus declares, or the census reads "this
/// read answers about nothing" when the truth is "nobody asked it".
///
/// The canon coordinate is deliberately NOT swept: it is a point in the story
/// rather than a choice of subject matter, sweeping it multiplies the panel by
/// every section, and the end of the road is where the store has all of its
/// history behind it (the R1050 rule).
pub fn required_argvs(flags: &[Flag], store: &AtomicStore, ws: &Path) -> Vec<Vec<String>> {
    let Some(base) = baseline_argv(flags, store, ws) else {
        return Vec::new();
    };
    let sections = values_for("--at", store);
    let mut out = vec![base];
    for flag in flags.iter().filter(|f| f.required && f.takes_value) {
        let values = values_for(&flag.name, store);
        if values.len() < 2 || (!sections.is_empty() && values == sections) {
            continue;
        }
        out = out
            .iter()
            .flat_map(|argv| {
                values.iter().map(|value| {
                    let mut next = argv.clone();
                    // The baseline already carries this flag once, at the first
                    // value; replace that value rather than appending a second.
                    if let Some(at) = next.iter().position(|token| *token == flag.name) {
                        next[at + 1] = value.clone();
                    }
                    next
                })
            })
            .collect();
    }
    out.sort();
    out.dedup();
    out
}

/// Ask every advertised read at BASELINE: bare, then with the corpus's own
/// telling, then with EVERY REQUIRED ARGUMENT the corpus can supply. A read that
/// still refuses needs an argument this corpus does not have; it is excluded BY
/// THAT MEASUREMENT, and its refusal is returned to be printed rather than
/// curated away.
///
/// Round 1051 added the third candidate, and it is not a convenience. The first
/// two are a GUESS about what a read needs — "nothing" or "a telling" — and a
/// read needing anything else was reported as an unaskable COUNT, so the four
/// reads that need a frame, a canon coordinate or an entity sat outside every
/// population this panel derives, including the read-agreement backlog. The
/// derivation is the same one the coordinate contract uses: each required flag
/// from the verb's own usage line, at a value read out of the store, with a
/// section-valued flag treated as a canon coordinate ([`baseline_argv`]). A verb
/// that grows a required argument is covered the run it ships.
pub fn panel(ws: &Path, telling: &str) -> (Vec<Read>, Vec<(String, String)>) {
    let mut asked = Vec::new();
    let mut unaskable = Vec::new();
    let store = AtomicStore::load(&ws.join(SIDECAR)).ok();
    let usage_of = usage_lines(ws);
    for verb in advertised_reads(ws) {
        let mut candidates = vec![
            Vec::new(),
            vec!["--telling".to_string(), telling.to_string()],
        ];
        if let (Some(store), Some(usage)) = (store.as_ref(), usage_of.get(&verb)) {
            candidates.extend(required_argvs(&flags_of(usage), store, ws));
        }
        candidates.sort();
        candidates.dedup();
        // EVERY argv the corpus can supply and the read accepts, not the first
        // one that works. Stopping at the first is what kept a swept read down
        // to one question, and one question is what made `report-frame-view`
        // answer about nothing (Round 1051). Two questions to one verb are two
        // different questions — that is the R1048 finding, and the panel now
        // holds both rather than choosing.
        let mut refusals: Vec<String> = Vec::new();
        let mut answered = false;
        for args in candidates {
            let read = Read {
                verb: verb.clone(),
                args,
            };
            let out = run(ws, &read.argv());
            if out.status.success() {
                asked.push(read);
                answered = true;
                continue;
            }
            let reason = String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("(no stderr)")
                .to_string();
            if !refusals.contains(&reason) {
                refusals.push(reason);
            }
        }
        if !answered {
            // Every distinct refusal, not the first: the first is the BARE
            // attempt, which says "--telling arg required" even when the panel
            // went on to supply one and the read refused for another reason.
            unaskable.push((verb, refusals.join(" | ")));
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

/// What every advertised read said about one store, keyed by [`Read::label`] —
/// the read AND the question, since one verb can be asked several.
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
            answers.insert(read.label(), Answer::read(out.stdout));
        } else {
            failed.push(read.label());
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
    // The same, for the legs that PLACE a fact and ATTRIBUTE it. Round 1054
    // added these: this derivation says it takes "the legs a fact actually
    // carries" and carried only the ones that say what a fact CLAIMS, so no
    // edit here ever moved a fact's coordinate, its frame or its world-line.
    // A walk asking which reads count a set they do not name came back EMPTY
    // over the whole read surface — every count on the surface summarizes one
    // of those three axes, and the population could not touch any of them.
    let mut frames: BTreeSet<String> = BTreeSet::new();
    let mut anchors: BTreeSet<String> = BTreeSet::new();
    let mut worlds: BTreeSet<String> = BTreeSet::new();
    for fact in store.narrative_facts.values() {
        frames.insert(fact.frame.to_string());
        anchors.insert(fact.canon_from.to_string());
        worlds.insert(fact.branch.to_string());
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
        // THE PLACEMENT AND ATTRIBUTION LEGS (Round 1054). The four above say
        // what a fact CLAIMS; these say WHERE it becomes true, WHERE it stops,
        // WHOSE view holds it, and WHICH world-line authored it. Every count a
        // shipped read emits summarizes one of those, so without them a walk
        // over the read surface cannot move a single number.
        //
        // The `entities` leg is not here because Leg 1 already carries it: a
        // claim retarget swaps the entity with it (the R446 invariant), so the
        // cast axis is exercised. The prose legs (`claim`, `quote`) are not
        // here either, and that is a different question rather than an
        // omission — a prose edit changes no cardinality, and admitting one
        // would make every read that merely RENDERS a claim "answer about"
        // the fact, which is the distinction this population exists to draw.

        // Leg 6 — the fact becomes true at a coordinate another fact is
        // anchored at. `evidence` moves with it: a backreference left pointing
        // at the scene the fact came from is not a move an author would commit,
        // and the write path is entitled to refuse it.
        let anchor = fact.canon_from.to_string();
        if let Some(alt) = anchors.iter().find(|s| **s != anchor).cloned() {
            out.push(Corruption {
                fact: id.clone(),
                leg: "canon_from",
                apply: Box::new(move |f| {
                    f["canon_from"] = alt.as_str().into();
                    f["evidence"] = serde_json::json!([alt.as_str()]);
                }),
            });
        }
        // Leg 7 — the fact stops being true somewhere else.
        if let Some(to) = &fact.canon_to {
            let to = to.to_string();
            if let Some(alt) = anchors.iter().find(|s| **s != to).cloned() {
                out.push(Corruption {
                    fact: id.clone(),
                    leg: "canon_to",
                    apply: Box::new(move |f| f["canon_to"] = alt.as_str().into()),
                });
            }
        }
        // Leg 8 — a different frame holds the fact.
        let frame = fact.frame.to_string();
        if let Some(alt) = frames.iter().find(|f| **f != frame).cloned() {
            out.push(Corruption {
                fact: id.clone(),
                leg: "frame",
                apply: Box::new(move |f| f["frame"] = alt.as_str().into()),
            });
        }
        // Leg 9 — a different world-line authored it.
        let world = fact.branch.to_string();
        if let Some(alt) = worlds.iter().find(|b| **b != world).cloned() {
            out.push(Corruption {
                fact: id.clone(),
                leg: "branch",
                apply: Box::new(move |f| f["branch"] = alt.as_str().into()),
            });
        }
    }
    out
}
