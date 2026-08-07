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

// ==========================================================================
// THE AUTHORABLE CORPUS (Round 1061).
//
// The manifests an author writes, held as VALUES so a walk can rebuild any of
// them with one thing changed. Until Round 1061 only the FACT manifest was a
// value here, and the census over the read surface said what that cost: of the
// 74 numbers the surface emits, 65 were proposed and never put to a second
// value, because no edit in the population could move a scene registry or a
// road. Every number that counts sections or road positions was in that 65.
//
// Which manifest an edit lands in is a property OF the edit, so the corpus is
// one value rather than one value and two paths.
// ==========================================================================

/// The manifests a corpus directory holds, as an author wrote them.
///
/// `narrative-rules.json` is deliberately NOT here: it is not in the corruption
/// population, so it is carried by path (see [`workspace_try`]) rather than by
/// value. A datum with two homes is how two callers come to disagree about it.
#[derive(Clone)]
pub struct Manifests {
    pub sections: serde_json::Value,
    pub order: serde_json::Value,
    pub facts: serde_json::Value,
}

impl Manifests {
    /// The manifests as the author left them in `dir`.
    pub fn of_corpus(dir: &Path) -> Self {
        Self {
            sections: read_json(&dir.join("sections.json")),
            order: read_json(&dir.join("order.json")),
            facts: read_json(&dir.join("facts.json")),
        }
    }
}

/// The frozen record's corpus — its scene registry and canon order, with the
/// fact manifest taken from the MIGRATED fixture, because the audit directory's
/// own `facts.json` is the pre-migration file that stopped loading (R857).
pub fn dnd_quest_manifests() -> Manifests {
    Manifests {
        sections: read_json(&audit_dir().join("sections.json")),
        order: read_json(&audit_dir().join("order.json")),
        facts: dnd_quest_facts(),
    }
}

/// Rebuild the store the way the experiment's runbook does — fresh seed, then
/// the manifests — with the corpus taken from the frozen record and the fact
/// manifest supplied by the caller, so a test can author a DEFECT into the
/// store an author could equally have authored (the manifest is the authoring
/// path, and it validates: a corruption the import rejects is a corruption no
/// author could have shipped).
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
/// recipe is specific to which author wrote it.
pub fn corpus_workspace_try(dir: &Path, facts: &serde_json::Value) -> Result<TempDir, String> {
    let manifests = Manifests {
        sections: read_json(&dir.join("sections.json")),
        order: read_json(&dir.join("order.json")),
        facts: facts.clone(),
    };
    workspace_try(&manifests, Some(dir))
}

/// THE recipe, over manifest VALUES: fresh seed, the three manifests written
/// out, then the imports an author would run.
///
/// `rules_from` is the corpus directory whose `narrative-rules.json` sits
/// beside them, if it has one — not every corpus declares rules, and a config
/// naming a file that is not there is a load failure rather than a corpus
/// without rules.
pub fn workspace_try(manifests: &Manifests, rules_from: Option<&Path>) -> Result<TempDir, String> {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).expect("mkdir");

    let rules_src = rules_from
        .map(|dir| dir.join("narrative-rules.json"))
        .filter(|src| src.exists());
    let rules = rules_src.is_some();
    if let Some(src) = &rules_src {
        fs::copy(src, ws.join("narrative-rules.json"))
            .map_err(|e| format!("copy {}: {e}", src.display()))?;
    }
    for (name, value) in [
        ("sections.json", &manifests.sections),
        ("order.json", &manifests.order),
        ("facts.json", &manifests.facts),
    ] {
        fs::write(
            ws.join(name),
            serde_json::to_string(value).expect("manifest serialize"),
        )
        .map_err(|e| format!("write {name}: {e}"))?;
    }

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
    workspace_try(
        &Manifests {
            sections: sections.clone(),
            order: order.clone(),
            facts: facts.clone(),
        },
        None,
    )
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

// ==========================================================================
// WHAT A READ WROTE ABOUT A SUBJECT (Round 1055, shared here in Round 1056).
//
// THE one definition, because two walks need it: the read-agreement population
// asks whether a read's record for a subject MOVED, and the census of lossy
// numbers asks whether everything that moved INSIDE one record was a number. A
// second copy is how the two would come to disagree about what a read said
// about which id — and this module already carries the measured cost of that
// exact disagreement one function up ([`registered_ids`]).
//
// A read's record about a subject is what that read WROTE ABOUT IT, and where
// the read writes the id decides what that is:
//
//   - as a field's VALUE, the minimal enclosing object. The row is the
//     sentence: `{"quest": "q-main", "state": "unknown"}` says `unknown` about
//     `q-main`, lists and all.
//   - as a KEY of a map, the value under that key — not the map, which holds
//     every other id beside it.
//   - as a bare element of a LIST, its MEMBERSHIP of that list, addressed down
//     through the ancestors that key it (`scene_coverage[scene=sc-05].facts[]`).
//
// Round 1055 measured what the third arm is worth: under "the minimal
// enclosing object" for every occurrence, a fact listed in a scene's census
// carried the whole row and so moved whenever a NEIGHBOUR did, and the count of
// subjects two reads both answer about read 210 of 214 where it should read
// 105 of 216. The naive repair (membership and nothing else) is refuted by the
// same census, which lists facts PER SCENE: a record that forgets the scene
// cannot see a fact change scenes.
// ==========================================================================

/// Everything one answer says about the store's subjects.
#[derive(Default)]
pub struct Wrote {
    /// subject -> address -> what the read wrote there. A `Null` at an address
    /// ending in `[]` is a MEMBERSHIP record: the address is the statement.
    pub by_subject: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    /// Every OBJECT record by address, the answer's root included (address
    /// `""`), so every number in the answer is inside exactly one innermost
    /// record and none is judged against a place no consumer reads it with.
    pub records: BTreeMap<String, serde_json::Value>,
    /// address -> the numbers directly inside that record and not inside a
    /// nested one, keyed by FIELD path (ids collapsed to `*`, so a per-road map
    /// is one field rather than one finding per road) and kept in document
    /// order rather than summed: rows that trade values have moved, and a total
    /// would say they had not.
    pub numbers: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// The same addressing for what the record NAMES: address -> field -> the
    /// subject ids named there. The parallel of [`Wrote::numbers`], and it has
    /// to come off the SAME walk — a number is accounted for when it counts
    /// things the answer names, and reading the two with different notions of
    /// where a place in an answer is would let an accounting be against names
    /// that sit somewhere the number does not.
    ///
    /// A field that names NOTHING is here too, with an empty set: an empty list
    /// is a place ids appear in other stores, and a walk that only sees the
    /// stores where it is full cannot express an accounting that includes it.
    pub names: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    /// The bare subject ids listed at an address, in document order — what the
    /// ORDER question reads, being the one thing a membership record cannot see.
    pub listed: BTreeMap<String, Vec<String>>,
    /// Rows of an array of objects that NO set of id-valued fields addresses,
    /// so the walk fell back to the position each happens to sit at. Named, not
    /// counted (the R1029 rule): a positional address moves when a row is
    /// inserted ahead of it, which is a record moving for a reason that is not
    /// about its subject.
    pub unaddressed: BTreeSet<String>,
    /// Two places in one answer that produced the same address. Structurally
    /// impossible while row keys are distinct, and measured rather than argued.
    pub collisions: usize,
}

impl Wrote {
    fn wrote(&mut self, subject: &str, address: &str, value: &serde_json::Value) {
        self.by_subject
            .entry(subject.to_string())
            .or_default()
            .insert(address.to_string(), value.clone());
    }

    /// A place in one record where ids are named. Called with `id: None` for a
    /// list or map that is EMPTY here — the place exists whether or not this
    /// store put anything in it.
    fn named(&mut self, record: &str, field: &str, id: Option<&str>) {
        let at = self
            .names
            .entry(record.to_string())
            .or_default()
            .entry(field.to_string())
            .or_default();
        if let Some(id) = id {
            at.insert(id.to_string());
        }
    }
}

/// What one read wrote about the store's subjects, and where.
pub fn wrote_about(answer: &serde_json::Value, subjects: &BTreeSet<String>) -> Wrote {
    let mut out = Wrote::default();
    if answer.is_object() {
        out.records.insert(String::new(), answer.clone());
    }
    walk_wrote(answer, "", "", "", subjects, &mut out);
    out
}

fn under(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// One node of an answer. `path` is its address, `field` the same with every id
/// collapsed to `*`, and `record` the address of the innermost enclosing object
/// record — the one a number here is part of the account of.
fn walk_wrote(
    value: &serde_json::Value,
    path: &str,
    field: &str,
    record: &str,
    subjects: &BTreeSet<String>,
    out: &mut Wrote,
) {
    match value {
        serde_json::Value::Number(n) => {
            out.numbers
                .entry(record.to_string())
                .or_default()
                .entry(field.to_string())
                .or_default()
                .push(n.to_string());
        }
        serde_json::Value::Object(map) if map.is_empty() => {
            // A map with nothing in it: a place ids appear in other stores.
            out.named(record, &format!("{field}{{}}"), None);
        }
        serde_json::Value::Object(map) => {
            // The ids this object names in a FIELD. They make it a record, and
            // the record is this whole object: the row is the sentence.
            let names: Vec<&String> = map
                .values()
                .filter_map(|child| match child {
                    serde_json::Value::String(id) if subjects.contains(id) => Some(id),
                    _ => None,
                })
                .collect();
            let here = if names.is_empty() {
                record.to_string()
            } else {
                out.records.insert(path.to_string(), value.clone());
                for id in &names {
                    out.wrote(id, path, value);
                }
                path.to_string()
            };
            for (key, child) in map {
                let child_path = under(path, key);
                if subjects.contains(key) {
                    // A map KEY is an id: the read wrote the subtree under it
                    // about that id, and that subtree is its own record. The
                    // KEY SET is a place this record names ids — `{}` rather
                    // than `.*`, which is the subtree under one of them.
                    let child_field = under(field, "*");
                    out.named(&here, &format!("{field}{{}}"), Some(key));
                    out.wrote(key, &child_path, child);
                    if child.is_object() {
                        out.records.insert(child_path.clone(), child.clone());
                        walk_wrote(child, &child_path, &child_field, &child_path, subjects, out);
                    } else {
                        walk_wrote(child, &child_path, &child_field, &here, subjects, out);
                    }
                    continue;
                }
                match child.as_str() {
                    // Already claimed above, as one of this object's names.
                    Some(id) if subjects.contains(id) => {
                        out.named(&here, &under(field, key), Some(id));
                    }
                    _ => walk_wrote(child, &child_path, &under(field, key), &here, subjects, out),
                }
            }
        }
        serde_json::Value::Array(items) => {
            let keys = key_fields(items, subjects);
            let listed_at = format!("{path}[]");
            if items.is_empty() {
                // A list with nothing in it: a place ids appear in other
                // stores, and the accounting a full store states has to be
                // expressible against the empty one.
                out.named(record, &format!("{field}[]"), None);
            }
            let mut listed: Vec<String> = Vec::new();
            for (index, item) in items.iter().enumerate() {
                match item {
                    serde_json::Value::String(id) if subjects.contains(id) => {
                        // MEMBERSHIP: the address is the whole of the record,
                        // so a neighbour joining or leaving says nothing here.
                        out.named(record, &format!("{field}[]"), Some(id));
                        out.wrote(id, &listed_at, &serde_json::Value::Null);
                        listed.push(id.clone());
                    }
                    _ => {
                        let row = row_address(path, item, index, &keys);
                        // THE FIELD IS THE PLACE, THE ADDRESS IS THE ROW. Which
                        // columns key a row is derived from the values (see
                        // [`key_fields`]), so spelling the key into the FIELD
                        // makes one place in one read carry different names in
                        // two answers: `report-entity`'s fact rows key on
                        // `branch` for an entity whose facts are one per road
                        // and on `fact_id` for the rest, and the quest graph's
                        // locators key on `world_line` until an edit leaves two
                        // rows sharing one. A shape that moves with the data
                        // cannot be compared across answers, which is the whole
                        // of what a field path is for.
                        let row_field = format!("{field}[]");
                        if item.is_object() && keys.is_empty() {
                            out.unaddressed.insert(row.clone());
                        }
                        walk_wrote(item, &row, &row_field, record, subjects, out);
                    }
                }
            }
            if !listed.is_empty() && out.listed.insert(listed_at, listed).is_some() {
                out.collisions += 1;
            }
        }
        _ => {}
    }
}

/// The fields that ADDRESS a row of this array: the SMALLEST set of fields
/// holding a subject id in every row whose values, taken together, are distinct
/// across the rows.
///
/// Derived from the values rather than named per read — the same discrimination
/// [`road_keying`] makes for roads, widened to every registered id and to
/// composite keys. Distinctness is what makes it a key: a field every row
/// carries but two rows share does not say which row you are at. And one field
/// is not enough on the shipped surface — the disclosure coverage's
/// `inert_reveal_pins` is one row per (fact, world) and NEITHER column is
/// distinct alone, which is how 9 rows came to be addressed by position.
///
/// Smallest because an address should carry no more than what identifies the
/// row: a field that varies for its own reasons would make the address move for
/// a reason that is not about which row this is.
fn key_fields(items: &[serde_json::Value], subjects: &BTreeSet<String>) -> Vec<String> {
    let Some(first) = items.first().and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    let candidates: Vec<String> = first
        .keys()
        .filter(|key| {
            items.iter().all(|item| {
                item.get(key)
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| subjects.contains(id))
            })
        })
        .cloned()
        .collect();
    let keys = |fields: &[String]| {
        let mut seen: BTreeSet<Vec<&str>> = BTreeSet::new();
        items.iter().all(|item| {
            let tuple: Vec<&str> = fields
                .iter()
                .filter_map(|field| item.get(field).and_then(serde_json::Value::as_str))
                .collect();
            seen.insert(tuple)
        })
    };
    for size in 1..=candidates.len() {
        if let Some(found) = combinations(&candidates, size)
            .into_iter()
            .find(|fields| keys(fields))
        {
            return found;
        }
    }
    Vec::new()
}

/// Every `size`-element subset of `of`, in the order the fields come in — so
/// the address a row gets is a fact about the answer and not about which subset
/// happened to be tried first.
fn combinations(of: &[String], size: usize) -> Vec<Vec<String>> {
    if size == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for (index, field) in of.iter().enumerate() {
        for rest in combinations(&of[index + 1..], size - 1) {
            let mut one = vec![field.clone()];
            one.extend(rest);
            out.push(one);
        }
    }
    out
}

fn row_address(path: &str, item: &serde_json::Value, index: usize, keys: &[String]) -> String {
    let addressed: Vec<String> = keys
        .iter()
        .filter_map(|field| {
            item.get(field)
                .and_then(serde_json::Value::as_str)
                .map(|id| format!("{field}={id}"))
        })
        .collect();
    if addressed.is_empty() {
        format!("{path}[{index}]")
    } else {
        format!("{path}[{}]", addressed.join(","))
    }
}

/// The lists that came back holding EXACTLY what they held, in another order —
/// the one thing a membership record cannot see, so a walk asks for it on every
/// edit instead of assuming it never happens.
///
/// A list whose membership changed is not one of these: that move is what the
/// record is for, and it is already counted as an answer.
pub fn permutations(before: &Wrote, after: &Wrote) -> BTreeSet<String> {
    before
        .listed
        .iter()
        .filter(|(address, order)| {
            after.listed.get(*address).is_some_and(|now| {
                now != *order
                    && now.iter().collect::<BTreeSet<_>>() == order.iter().collect::<BTreeSet<_>>()
            })
        })
        .map(|(address, _)| address.clone())
        .collect()
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
/// `--against` / `--truth-frame` are why this takes the WORKSPACE and not only
/// the store (Round 1072). A frame is store vocabulary like any other; a
/// re-extracted store is a FILE, and a file is a fact about what the workspace
/// holds rather than about what the corpus declares. The panel supplies one
/// only where the projection has actually been written ([`project_worlds`]),
/// so a corpus nobody projected still reports the read as unaskable — which is
/// the truth about that corpus rather than a path to nothing.
pub fn values_for(flag: &str, store: &AtomicStore, ws: &Path) -> Vec<String> {
    let ids = |set: Vec<String>| set;
    match flag {
        "--telling" => ids(declared_tellings(store)),
        // The single-world projections this workspace holds, named RELATIVELY:
        // the panel's argv is reused against every corrupted store, and an
        // absolute path into a temp directory would put a different string in
        // the read's label on every run.
        "--against" => projected_worlds(store)
            .into_iter()
            .filter(|name| ws.join(name).exists())
            .collect(),
        "--world" | "--branch" => {
            let mut out: Vec<String> = store.branches.keys().map(ToString::to_string).collect();
            out.push("main".to_string());
            out.sort();
            out.dedup();
            out
        }
        "--entity" => ids(store.entities.keys().map(ToString::to_string).collect()),
        "--at" | "--target" => ids(store.sections.keys().map(ToString::to_string).collect()),
        // A TRUTH FRAME IS A FRAME. The leak gate names the frame it matches
        // the re-extracted prose in; the vocabulary is the store's, exactly as
        // for `--frame`, and until Round 1072 this arm was missing, so that
        // read had no argv the panel could build even once its file existed.
        "--frame" | "--truth-frame" => {
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

/// What a world's projection is called inside a workspace — one name, derived
/// from the world, so the writer and the reader cannot spell it differently.
fn projection_name(world: &str) -> String {
    format!("reextracted-{world}.json")
}

/// Every projection name this store's worlds would have.
fn projected_worlds(store: &AtomicStore) -> Vec<String> {
    let mut out: Vec<String> = store
        .branches
        .keys()
        .map(ToString::to_string)
        .chain(std::iter::once(mnemosyne_core::MAIN_BRANCH.to_string()))
        .collect();
    out.sort();
    out.dedup();
    out.into_iter()
        .map(|world| projection_name(&world))
        .collect()
}

/// ASK THE CORPUS TO PRODUCE THE FILE THE GATES REQUIRE (Round 1072).
///
/// `validate-render-fidelity` and `validate-disclosure-leak` take a
/// re-extracted prose store and are SINGLE-WORLD by contract; Round 1070
/// measured that handing them the authored store refuses at every world, and
/// shipped `project-world` as the operation that produces what they expect.
/// This runs it, once per world, into the workspace — after which
/// [`values_for`] has a value for `--against` and the panel's own machinery
/// supplies the rest.
///
/// It is the reason MM2 could not be closed by naming a path: the value is not
/// something the corpus DECLARES, it is something the corpus must be ASKED to
/// make. A world the projection refuses is left without a file rather than
/// with an empty one, so the read stays unaskable and says why.
pub fn project_worlds(ws: &Path, store: &AtomicStore) {
    for world in store
        .branches
        .keys()
        .map(ToString::to_string)
        .chain(std::iter::once(mnemosyne_core::MAIN_BRANCH.to_string()))
    {
        let out = projection_name(&world);
        let emitted = run(
            ws,
            &["project-world", "--world", &world, "--out", &out, "--json"],
        );
        assert!(
            emitted.status.success(),
            "the corpus registers `{world}` but cannot project it: {}",
            String::from_utf8_lossy(&emitted.stderr)
        );
    }
}

/// Put a workspace's projections into ANOTHER workspace, unchanged.
///
/// The fixture the gates are asked against is the BASELINE's prose: a world's
/// telling does not change because its road did, and that is exactly what
/// leaves the gate something to disagree with. Copying rather than
/// re-projecting is the whole point — a projection of the corrupted store
/// would move with it and the gate would report clean forever.
pub fn carry_projections(from: &Path, to: &Path, store: &AtomicStore) {
    for name in projected_worlds(store) {
        let src = from.join(&name);
        if src.exists() {
            fs::copy(&src, to.join(&name)).unwrap_or_else(|e| panic!("carry {name}: {e}"));
        }
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
    let sections = values_for("--at", store, ws);
    let mut base: Vec<String> = Vec::new();
    let mut default_end: Option<Option<String>> = None;
    for flag in flags.iter().filter(|f| f.required) {
        let values = values_for(&flag.name, store, ws);
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
pub fn road_filters<'a>(flags: &'a [Flag], store: &AtomicStore, ws: &Path) -> Vec<&'a Flag> {
    let roads = values_for("--world", store, ws);
    flags
        .iter()
        .filter(|flag| flag.takes_value && values_for(&flag.name, store, ws) == roads)
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
    let sections = values_for("--at", store, ws);
    let mut out = vec![base];
    for flag in flags.iter().filter(|f| f.required && f.takes_value) {
        let values = values_for(&flag.name, store, ws);
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
    // The two render-acceptance gates need a FILE, which is why they sat
    // outside every population this panel derives until Round 1072. Ask the
    // corpus to produce it before deriving arguments from it.
    if let Some(store) = store.as_ref() {
        project_worlds(ws, store);
    }
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

// ==========================================================================
// THE DERIVED CORRUPTION POPULATION (Round 1033, shared here in Round 1040).
//
// Nothing here names a defect class: the population is the quest layer's facts
// and, per fact, the legs it ACTUALLY carries. Each edit goes through the real
// write path, so what is judged is a move an author could commit.
// ==========================================================================

/// One authorable corruption: which subject it moves, which leg of the corpus
/// carries it, and the edit itself.
///
/// The edit takes the whole corpus, not a fact entry, and it ASSERTS ITS OWN
/// ARITY while applying: a substitution that matched no place, or two, is a
/// mis-aimed injection rather than a corruption, and it stops the walk here
/// rather than producing a store that silently equals the baseline. That
/// assertion used to sit in each walk that consumed the population — three
/// copies of one loop, three places for it to drift.
pub struct Corruption {
    /// The registered id whose corner of the corpus this edit moves.
    pub subject: String,
    /// WHICH KIND of leg moved — one of [`LEGS`], a closed vocabulary, so a
    /// leg the corpus cannot fill shows up as a zero rather than as an absence.
    pub leg: &'static str,
    /// WHERE it moved, when the subject and the leg together do not say. A
    /// scene sits on several roads, so `sc-22/order.from` names three different
    /// edits; without this they share one label, and a walk that keys findings
    /// by label silently merges them (the collision axis R1055 measured).
    pub site: String,
    apply: Box<dyn Fn(&mut Manifests) + Send + Sync>,
}

/// Every leg this derivation knows how to fill — the closed vocabulary the
/// population reports its reach over.
pub const LEGS: &[&str] = &[
    "facts.typed.object",
    "facts.typed.predicate",
    "facts.pays_off",
    "facts.payoff_expectation",
    "facts.evidence",
    "facts.canon_from",
    "facts.canon_to",
    "facts.frame",
    "facts.branch",
    "sections.coverage_expectation",
    "sections.parent_doc",
    "sections.dropped",
    "order.to",
    "order.from",
    "order.dropped",
];

impl Corruption {
    /// How every walk names this edit. THE one spelling: three walks printed
    /// `format!("{fact}/{leg}")` by hand, and a name a reader matches across
    /// three reports is a name that has to be derived in one place.
    pub fn label(&self) -> String {
        if self.site.is_empty() {
            format!("{}/{}", self.subject, self.leg)
        } else {
            format!("{}/{}@{}", self.subject, self.leg, self.site)
        }
    }

    /// The corpus with this one edit in it.
    pub fn applied(&self, base: &Manifests) -> Manifests {
        let mut out = base.clone();
        (self.apply)(&mut out);
        out
    }
}

/// An edit to the ONE fact entry with this id.
fn on_fact(
    id: &str,
    edit: impl Fn(&mut serde_json::Value) + Send + Sync + 'static,
) -> Box<dyn Fn(&mut Manifests) + Send + Sync> {
    let id = id.to_string();
    Box::new(move |m| {
        let mut hits = 0usize;
        for entry in m.facts["facts"].as_array_mut().expect("facts array") {
            if entry["fact_id"] == id.as_str() {
                edit(entry);
                hits += 1;
            }
        }
        assert_eq!(hits, 1, "the edit to fact {id} matched {hits} entries");
    })
}

/// An edit to the ONE scene-registry entry with this id.
fn on_section(
    id: &str,
    edit: impl Fn(&mut serde_json::Value) + Send + Sync + 'static,
) -> Box<dyn Fn(&mut Manifests) + Send + Sync> {
    let id = id.to_string();
    Box::new(move |m| {
        let mut hits = 0usize;
        for entry in m.sections.as_array_mut().expect("sections array") {
            if entry["section_id"] == id.as_str() {
                edit(entry);
                hits += 1;
            }
        }
        assert_eq!(hits, 1, "the edit to section {id} matched {hits} entries");
    })
}

/// An edge of the canon order is the pair `[from, to]`, so its two ends have
/// positions rather than names in the manifest. These give them names back.
const TAIL: usize = 0;
const HEAD: usize = 1;

/// The edge list of one road in the canon order. The unforked road is the
/// manifest's top-level `edges`; a fork is `branches.<name>`. Both are lists of
/// `[from, to]` pairs, so one accessor serves both and the population never
/// learns which kind of road it is editing.
fn road_mut<'a>(
    order: &'a mut serde_json::Value,
    road: Option<&str>,
) -> &'a mut Vec<serde_json::Value> {
    match road {
        None => order["edges"].as_array_mut().expect("order edges array"),
        Some(name) => order["branches"][name]
            .as_array_mut()
            .unwrap_or_else(|| panic!("order branch {name} is an array")),
    }
}

/// How the manifest spells the road — the prefix every order leg is named with.
fn road_path(road: Option<&str>) -> String {
    match road {
        None => "order.edges".to_string(),
        Some(name) => format!("order.branches.{name}"),
    }
}

/// WHERE EACH SCENE SITS in the declaration — one linearisation of the whole
/// canon order, trunk and forks in a single graph, which is how a road that
/// inherits a trunk prefix actually reads.
///
/// This exists so a retarget can stay AUTHORABLE, and it is the canon order's
/// version of [`swap_entity`]: an edge re-pointed at a scene that already
/// reaches its own tail closes a loop, and the CLI's answer to a loop is
/// `cycle through <id> — a cyclic declaration is no order`. Round 1061 measured
/// that before writing this: choosing the first scene BY NAME produced 110
/// corruptions, every one of them a two-cycle back to the road's own root, and
/// twelve of the thirty shipped reads refused all 110 with the same message. A
/// third of the sweep, carrying one bit. Keeping every edge pointing FORWARD in
/// the baseline's own linearisation cannot close a loop, because every other
/// edge already points that way.
///
/// Scenes the order never mentions come last: nothing reaches them and they
/// reach nothing, so an edge to or from one cannot be part of a cycle.
fn topological_index(
    order: &serde_json::Value,
    scenes: &BTreeSet<String>,
) -> BTreeMap<String, usize> {
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut roads: Vec<&serde_json::Value> = vec![&order["edges"]];
    if let Some(branches) = order["branches"].as_object() {
        roads.extend(branches.values());
    }
    for road in roads {
        for edge in road.as_array().into_iter().flatten() {
            let (Some(from), Some(to)) = (edge[TAIL].as_str(), edge[HEAD].as_str()) else {
                continue;
            };
            edges.push((from.to_string(), to.to_string()));
        }
    }

    let mut nodes: BTreeSet<String> = scenes.clone();
    for (from, to) in &edges {
        nodes.insert(from.clone());
        nodes.insert(to.clone());
    }
    let mut indegree: BTreeMap<&str, usize> = nodes.iter().map(|n| (n.as_str(), 0)).collect();
    let mut out_of: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (from, to) in &edges {
        out_of.entry(from.as_str()).or_default().push(to.as_str());
        *indegree.get_mut(to.as_str()).expect("a declared node") += 1;
    }

    // Smallest ready node first, so the linearisation is a property of the
    // corpus rather than of iteration order.
    let mut ready: BTreeSet<&str> = indegree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(node, _)| *node)
        .collect();
    let mut index = BTreeMap::new();
    while let Some(node) = ready.iter().next().copied() {
        ready.remove(node);
        index.insert(node.to_string(), index.len());
        for next in out_of.get(node).into_iter().flatten() {
            let deg = indegree.get_mut(*next).expect("a declared node");
            *deg -= 1;
            if *deg == 0 {
                ready.insert(next);
            }
        }
    }
    assert_eq!(
        index.len(),
        nodes.len(),
        "the authored canon order is cyclic, so there is no baseline order for a \
         corruption to stay inside of"
    );
    index
}

/// The scenes strictly after `from`, nearest first — where a road could skip
/// to, in the order a smaller skip comes before a larger one.
fn forward_of(order_of: &BTreeMap<String, usize>, from: &str, to: &str) -> Vec<String> {
    let Some(at) = order_of.get(from).copied() else {
        return Vec::new();
    };
    let mut out: Vec<(usize, String)> = order_of
        .iter()
        .filter(|(scene, place)| **place > at && scene.as_str() != to)
        .map(|(scene, place)| (*place, scene.clone()))
        .collect();
    out.sort();
    out.into_iter().map(|(_, scene)| scene).collect()
}

/// The scenes strictly before `to`, nearest first — where a road could be
/// entered from instead.
fn backward_of(order_of: &BTreeMap<String, usize>, from: &str, to: &str) -> Vec<String> {
    let Some(at) = order_of.get(to).copied() else {
        return Vec::new();
    };
    let mut out: Vec<(usize, String)> = order_of
        .iter()
        .filter(|(scene, place)| **place < at && scene.as_str() != from)
        .map(|(scene, place)| (*place, scene.clone()))
        .collect();
    out.sort();
    out.into_iter().rev().map(|(_, scene)| scene).collect()
}

/// Whether an edited canon order is still one the system will accept — asked of
/// THE SHIPPED COMPOSER, never re-derived here.
///
/// This is the one judge because the canon order has more than one invariant
/// and a population that hand-lists them learns the next one the expensive way.
/// Round 1061 learned two in one round: an edge chosen by NAME closes a loop
/// (`a cyclic declaration is no order`, 110 corruptions), and the topological
/// choice that fixed it still detached forks from the trunk (`a branch's edge
/// set must ATTACH to the road it rides in on`, 83 more). Both were 12 of the
/// 30 shipped reads refusing the store outright, which is what a corruption no
/// author could ship looks like when the manifest passes no import.
/// Whether re-pointing one end of one edge leaves an order the system accepts.
fn composed_edge(
    manifests: &Manifests,
    store: &AtomicStore,
    road: Option<&str>,
    at: usize,
    end: usize,
    alt: &str,
) -> bool {
    let mut candidate = manifests.order.clone();
    road_mut(&mut candidate, road)[at][end] = alt.into();
    composes(&candidate, store)
}

pub fn composes(order: &serde_json::Value, store: &AtomicStore) -> bool {
    let Ok(decl) =
        serde_json::from_value::<mnemosyne_validate::continuity::CanonOrderFile>(order.clone())
    else {
        return false;
    };
    mnemosyne_validate::continuity::CanonOrder::from_declaration(&decl, &store.branches).is_ok()
}

/// An edit to the ONE edge at this position of this road.
fn on_edge(
    road: Option<&str>,
    at: usize,
    edit: impl Fn(&mut Vec<serde_json::Value>, usize) + Send + Sync + 'static,
) -> Box<dyn Fn(&mut Manifests) + Send + Sync> {
    let road = road.map(str::to_string);
    Box::new(move |m| {
        let edges = road_mut(&mut m.order, road.as_deref());
        assert!(
            at < edges.len(),
            "the edit to {}[{at}] found {} edges",
            road_path(road.as_deref()),
            edges.len()
        );
        edit(edges, at);
    })
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

/// DERIVE the corruption population: the corpus an author writes, and per entry
/// the legs it actually carries. Nothing here names a defect class — the
/// classes are whatever the legs turn out to be.
///
/// Three manifests, because an author writes three (Round 1061). The fact legs
/// came first and for twenty rounds they were the whole population; the census
/// that walks it then reported 65 of the surface's 74 numbers as proposed and
/// never put to a second value, and the reason was not the surface. Every
/// number that counts SCENES or ROAD POSITIONS — the frontier's `road_scenes`,
/// continuity's `order_nodes` and `sections`, the spec map's section totals,
/// coverage's class split — sits behind a manifest no edit could reach.
///
/// The fact leg is scoped to the quest layer and the other two are not, which
/// is a difference rather than an oversight. The quest layer is the runtime
/// seam this arc walks, and the fact manifest holds 136 facts of which it is a
/// part; a scene registry and a canon order are 56 entries and 55 edges, they
/// are wholly about where the story goes, and there is no sub-part of either
/// that a runtime seam would single out.
pub fn corruptions(store: &AtomicStore, manifests: &Manifests) -> Vec<Corruption> {
    let facts_json = &manifests.facts;
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
                        subject: id.clone(),
                        leg: "facts.typed.object",
                        site: String::new(),
                        apply: on_fact(&id, move |f| {
                            f["typed"]["object"]["id"] = alt.as_str().into();
                            swap_entity(f, &from, &alt);
                        }),
                    });
                }
                // Leg 2 — the claim carries a different predicate.
                if let Some(alt) = predicates.iter().find(|p| **p != predicate).cloned() {
                    out.push(Corruption {
                        subject: id.clone(),
                        leg: "facts.typed.predicate",
                        site: String::new(),
                        apply: on_fact(&id, move |f| {
                            f["typed"]["predicate"] = alt.as_str().into();
                        }),
                    });
                }
            }
        }
        // Leg 3 — one payoff edge goes missing.
        if !fact.pays_off.is_empty() {
            out.push(Corruption {
                subject: id.clone(),
                leg: "facts.pays_off",
                site: String::new(),
                apply: on_fact(&id, |f| {
                    f["pays_off"].as_array_mut().expect("pays_off").remove(0);
                }),
            });
        }
        // Leg 4 — a setup stops expecting its payoff.
        if fact.payoff_expectation == mnemosyne_core::PayoffExpectation::Expected {
            out.push(Corruption {
                subject: id.clone(),
                leg: "facts.payoff_expectation",
                site: String::new(),
                apply: on_fact(&id, |f| {
                    f.as_object_mut()
                        .expect("fact object")
                        .remove("payoff_expectation");
                }),
            });
        }
        // Leg 5 — a backreference goes missing.
        if fact.evidence.len() > 1 {
            out.push(Corruption {
                subject: id.clone(),
                leg: "facts.evidence",
                site: String::new(),
                apply: on_fact(&id, |f| {
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
                subject: id.clone(),
                leg: "facts.canon_from",
                site: String::new(),
                apply: on_fact(&id, move |f| {
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
                    subject: id.clone(),
                    leg: "facts.canon_to",
                    site: String::new(),
                    apply: on_fact(&id, move |f| {
                        f["canon_to"] = alt.as_str().into();
                    }),
                });
            }
        }
        // Leg 8 — a different frame holds the fact.
        let frame = fact.frame.to_string();
        if let Some(alt) = frames.iter().find(|f| **f != frame).cloned() {
            out.push(Corruption {
                subject: id.clone(),
                leg: "facts.frame",
                site: String::new(),
                apply: on_fact(&id, move |f| {
                    f["frame"] = alt.as_str().into();
                }),
            });
        }
        // Leg 9 — a different world-line authored it.
        let world = fact.branch.to_string();
        if let Some(alt) = worlds.iter().find(|b| **b != world).cloned() {
            out.push(Corruption {
                subject: id.clone(),
                leg: "facts.branch",
                site: String::new(),
                apply: on_fact(&id, move |f| {
                    f["branch"] = alt.as_str().into();
                }),
            });
        }
    }

    out.extend(registry_corruptions(manifests));
    out.extend(order_corruptions(manifests, store));
    out
}

// ==========================================================================
// THE OTHER TWO MANIFESTS (Round 1061).
// ==========================================================================

/// The scene registry's legs — `sections.json`, one entry per scene.
///
/// A section is a registered id exactly like a fact is ([`registered_ids`]),
/// and until this round nothing in the population moved one. The legs are the
/// ones a registry entry carries, guarded the same way: a retarget goes to a
/// value that is already legal in that role, so the corruption stays
/// type-plausible.
///
/// `coverage_expectation` takes its alternatives FROM THE TYPE rather than from
/// the corpus, and that is the one place this population reads a type. The
/// difference is in the field: `typed.object` holds an ID, so only the store
/// knows which values exist, and inventing one would author a dangling
/// reference. `coverage_expectation` holds a CLOSED VOCABULARY — the type is
/// the registry of everything an author may write there, and every variant is
/// plausible by construction. Deriving it from the corpus instead would have
/// yielded nothing at all: all 56 of this corpus's scenes are `informational`,
/// so "another value the corpus uses in this role" is the empty set, and a leg
/// that silently contributes zero reads exactly like a leg the surface ignores.
///
/// `title` is absent for the reason the fact legs give: a prose edit changes no
/// cardinality, and admitting one would make every read that merely RENDERS a
/// scene "answer about" it. `section_id` is absent because it is the identity,
/// not a leg — the same reason `fact_id` is not a fact leg.
fn registry_corruptions(manifests: &Manifests) -> Vec<Corruption> {
    let entries = manifests
        .sections
        .as_array()
        .expect("the scene registry is an array");
    // The docs this registry already files scenes under.
    let docs: BTreeSet<String> = entries
        .iter()
        .filter_map(|e| e["parent_doc"].as_str().map(str::to_string))
        .collect();

    let mut out = Vec::new();
    for entry in entries {
        let Some(id) = entry["section_id"].as_str() else {
            continue;
        };
        let id = id.to_string();

        // Leg 1 — the scene is classified differently. Absent in the manifest
        // means the serde default, so the value being moved AWAY from is what
        // the import would have stored, not what the JSON happens to spell.
        let expectation = entry["coverage_expectation"]
            .as_str()
            .unwrap_or_else(|| mnemosyne_core::CoverageExpectation::default().as_str())
            .to_string();
        if let Some(alt) = mnemosyne_core::CoverageExpectation::ALL
            .iter()
            .map(|v| v.as_str())
            .find(|v| *v != expectation)
        {
            out.push(Corruption {
                subject: id.clone(),
                leg: "sections.coverage_expectation",
                site: String::new(),
                apply: on_section(&id, move |e| {
                    e["coverage_expectation"] = alt.into();
                }),
            });
        }

        // Leg 2 — the scene is filed under a different document.
        let doc = entry["parent_doc"].as_str().unwrap_or_default().to_string();
        if let Some(alt) = docs.iter().find(|d| **d != doc).cloned() {
            out.push(Corruption {
                subject: id.clone(),
                leg: "sections.parent_doc",
                site: String::new(),
                apply: on_section(&id, move |e| {
                    e["parent_doc"] = alt.as_str().into();
                }),
            });
        }

        // Leg 3 — the author cuts the scene from the registry. The road may
        // still walk through it and facts may still anchor at it; whether that
        // is refused, and by whom, is the measurement rather than the premise.
        let dropped = id.clone();
        out.push(Corruption {
            subject: id.clone(),
            leg: "sections.dropped",
            site: String::new(),
            apply: Box::new(move |m| {
                let entries = m.sections.as_array_mut().expect("sections array");
                let before = entries.len();
                entries.retain(|e| e["section_id"] != dropped.as_str());
                assert_eq!(
                    before - entries.len(),
                    1,
                    "dropping section {dropped} removed {} entries",
                    before - entries.len()
                );
            }),
        });
    }
    out
}

/// The canon order's legs — `order.json`, one entry per edge of one road.
///
/// This is the manifest the frontier's `road_scenes` counts and continuity's
/// `order_nodes` counts, and no edit has ever moved it: `corruptions` derived
/// from the fact leg, so the only edits that reached a road were the ones that
/// moved a fact ONTO a different one. Round 1055 filed that as `QQ3` and Round
/// 1056 filed it again as `RR2` — both said the same thing, that the population
/// stops at the fact manifest.
///
/// Unlike the other two manifests there is no import to refuse an edit here:
/// `order.json` is read at READ time through the workspace config. So the
/// analogue of "the write path rejected this" is "the reads rejected it", which
/// is why every walk over this population counts read failures BY NAME rather
/// than letting them fall in with the answers it could not compare.
fn order_corruptions(manifests: &Manifests, store: &AtomicStore) -> Vec<Corruption> {
    // What a road may point at: the scenes the registry declares. Derived from
    // the corpus, like a fact's anchors — a scene id is an ID, so only the
    // corpus knows which ones exist.
    let scenes: BTreeSet<String> = manifests
        .sections
        .as_array()
        .expect("the scene registry is an array")
        .iter()
        .filter_map(|e| e["section_id"].as_str().map(str::to_string))
        .collect();
    // WHERE EACH SCENE SITS IN THE DECLARATION, so a retarget can stay one.
    let order_of = topological_index(&manifests.order, &scenes);

    // Every road the manifest declares: the unforked one, then each fork by
    // name. Named here from the manifest rather than listed, so a corpus with
    // more forks is walked without this derivation learning about it.
    let mut roads: Vec<Option<String>> = vec![None];
    if let Some(branches) = manifests.order["branches"].as_object() {
        roads.extend(branches.keys().map(|name| Some(name.clone())));
    }

    let mut out = Vec::new();
    for road in roads {
        let name = road.as_deref();
        let edges = manifests.order.clone();
        let Some(edges) = (match name {
            None => edges["edges"].as_array().cloned(),
            Some(branch) => edges["branches"][branch].as_array().cloned(),
        }) else {
            continue;
        };
        let path = road_path(name);
        for (at, edge) in edges.iter().enumerate() {
            let (from, to) = (
                edge[TAIL].as_str().unwrap_or_default().to_string(),
                edge[HEAD].as_str().unwrap_or_default().to_string(),
            );
            // Leg 1 — the road goes somewhere else next: the story skips ahead.
            // Candidates are tried nearest-skip first and the SHIPPED COMPOSER
            // decides, so the corruption is a road an author could ship rather
            // than one every road-reading verb refuses.
            let end = HEAD;
            if let Some(alt) = forward_of(&order_of, &from, &to)
                .into_iter()
                .find(|alt| composed_edge(manifests, store, name, at, end, alt))
            {
                out.push(Corruption {
                    subject: to.clone(),
                    leg: "order.to",
                    site: format!("{path}[{at}]"),
                    apply: on_edge(name, at, move |edges, at| {
                        edges[at][end] = alt.as_str().into();
                    }),
                });
            }
            // Leg 2 — the road arrives from somewhere else: an earlier scene
            // leads here instead.
            let end = TAIL;
            if let Some(alt) = backward_of(&order_of, &from, &to)
                .into_iter()
                .find(|alt| composed_edge(manifests, store, name, at, end, alt))
            {
                out.push(Corruption {
                    subject: from.clone(),
                    leg: "order.from",
                    site: format!("{path}[{at}]"),
                    apply: on_edge(name, at, move |edges, at| {
                        edges[at][end] = alt.as_str().into();
                    }),
                });
            }
            // Leg 3 — the step goes missing, and the road is that much shorter.
            // Only where the rest still composes: dropping the edge a fork
            // hangs off detaches it, and a detached fork is refused, not read.
            let mut without = manifests.order.clone();
            road_mut(&mut without, name).remove(at);
            if composes(&without, store) {
                out.push(Corruption {
                    subject: to.clone(),
                    leg: "order.dropped",
                    site: format!("{path}[{at}]"),
                    apply: on_edge(name, at, |edges, at| {
                        edges.remove(at);
                    }),
                });
            }
        }
    }
    out
}
