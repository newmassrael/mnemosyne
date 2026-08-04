//! What stands between an authoring slip and the runtime (Rounds 1033-1036).
//!
//! Round 1031 built a gate for ONE play-breaking class and chose that class by
//! reading a doc comment. Round 1033 replaced that hand-list with a walk: the
//! population is DERIVED from the store — the quest layer of the one
//! blind-authored branching corpus, and per fact the legs it ACTUALLY has —
//! and each corruption is applied to the MANIFEST and pushed through the real
//! write path, so what is judged is what an author could commit.
//!
//! That walk answered "how much of the arc's surface is left" and left one
//! question open in so many words: WHICH of the silent corruptions admit a
//! declared contract to gate against, the way `requires` did. This round
//! answers it, and moving the walk here is how. Round 1033 ran three stations
//! it called by hand from the library; the reads a consumer actually has are
//! the verbs the CLI advertises, and there are thirty-two of them. So:
//!
//! - The PANEL is derived from `--help`, not curated. Every advertised
//!   `report-*` / `validate-*` verb is asked. A verb that cannot be asked
//!   without inventing an argument this corpus does not supply fails AT
//!   BASELINE and is excluded by that measurement, with its name and its
//!   refusal printed.
//! - "The system has somewhere to PUT this" is derived too, and against the
//!   store rather than against a list of field names. A read that merely
//!   carries the edited datum reproduces the store's own diff; a read that
//!   CLASSIFIES moves an id between buckets the store never held. Comparing
//!   the read's resize footprint against the STORE's separates the two, so a
//!   rendering read cannot masquerade as a defect detector.
//!
//! Each corruption lands in exactly one bucket:
//!
//! - `REFUSED` — the write path rejects it. The class cannot be authored.
//! - `CAUGHT` — some shipped read EXITS NON-ZERO. A gate rejects it.
//! - `REPORTED` — every read exits 0, but some read RECLASSIFIES: a list the
//!   store does not hold changes size. The system has a place to stand, so
//!   whether to gate is a policy question with a named owner rather than a
//!   missing capability. Necessary for a gate, not sufficient — `dangling` is
//!   computed and deliberately never rejects, and that is a decision already
//!   made rather than a hole.
//! - `CARRIED` — every read exits 0 and nothing derived moves. A rendering
//!   read reproduces the altered datum and no other read has an opinion.
//!   THIS is the arc's floor: the store states the datum once, so there is no
//!   second declaration for a gate to compare it against.
//! - `INERT` — no advertised read distinguishes it at all. The store holds a
//!   datum no consumer can see.
//!
//! The census is pinned. A new leg kind, a new quest fact, a new verb, or a
//! read that starts or stops seeing a class all move it, which is the point:
//! the number is the arc's remaining surface, re-derived on every run.
//!
//! Round 1034 stopped there, and left "which of the REPORTED SHOULD reject" as
//! a policy question. Round 1035 asks it the only way this repository has that
//! is not judgement. Every boundary-crossing resize PROPOSES a rule — a list
//! that filled proposes "always empty", a list that emptied proposes "never
//! empty" — and each proposal is put to the authored stores. A rule an authored
//! corpus already breaks is a rule that would reject its author, which is
//! exactly how Round 1032 disproved the pickup-order reading.
//!
//! Round 1036 fixed the population that refutation is drawn from. Round 1035
//! asked the ONE corpus under corruption; the refuter's population is not the
//! corpus being corrupted but EVERY store an author shipped, and `git ls-files`
//! finds 43 of them. A rule is refuted by any of them. The corpus can still
//! only REFUTE — a surviving rule is un-refuted, never confirmed — so the
//! authored distribution is printed beside each verdict and the reader can see
//! how close the evidence sits to the boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mnemosyne_atomic::AtomicStore;

mod common;
use common::{
    cli_binary, corpus_workspace_try, dnd_quest_facts, dnd_quest_workspace_try, repo_root, run,
};

/// The sidecar the import writes, relative to the workspace root.
const SIDECAR: &str = "docs/.atomic/workspace.atomic.json";

/// The store as the import left it, read back as the store's own serialization.
fn read_sidecar(ws: &Path) -> serde_json::Value {
    read_sidecar_at(&ws.join(SIDECAR))
}

fn read_sidecar_at(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} unreadable: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not json: {e}", path.display()))
}

/// The corpus's own telling, read from the store rather than named here — the
/// walk supplies no argument the corpus did not declare.
fn telling_of(store: &AtomicStore) -> String {
    let mut plans = store.disclosure_plans.keys();
    let telling = plans
        .next()
        .expect("the corpus declares a disclosure plan")
        .clone();
    assert_eq!(
        plans.next(),
        None,
        "the corpus declares more than one telling, so `the` telling is no \
         longer derivable — the walk would have to choose, which is the \
         invented argument this panel refuses to make"
    );
    telling
}

/// One authorable corruption: which fact, which leg, and the edit itself.
struct Corruption {
    fact: String,
    leg: &'static str,
    apply: Box<dyn Fn(&mut serde_json::Value)>,
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
fn corruptions(store: &AtomicStore, facts_json: &serde_json::Value) -> Vec<Corruption> {
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

/// One shipped read, and the arguments the CORPUS can answer it with.
struct Read {
    verb: String,
    args: Vec<String>,
}

impl Read {
    fn argv(&self) -> Vec<&str> {
        let mut argv = vec![self.verb.as_str()];
        argv.extend(self.args.iter().map(String::as_str));
        argv.push("--json");
        argv
    }
}

/// Every `report-*` / `validate-*` verb the shipped help advertises. Read from
/// the token that FOLLOWS the program path on each usage line, so a verb named
/// in a note's prose is not mistaken for a dispatchable one.
fn advertised_reads(ws: &Path) -> BTreeSet<String> {
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
fn panel(ws: &Path, telling: &str) -> (Vec<Read>, Vec<(String, String)>) {
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

/// Where a read RECLASSIFIED something: the JSON paths whose list holds a
/// different NUMBER of entries than it did at baseline.
///
/// This is the discriminator between a read that merely carries the changed
/// datum and a read that has somewhere to PUT it. A rendering read re-renders
/// the altered prose — leaves change, cardinalities do not. A read that sorts
/// facts into `dangling` / `payoffs_to_unmarked` / `unresolved_quests` /
/// `violations` moves one across, and the path names the bucket, so whether
/// that bucket is a finding or merely content is visible rather than assumed.
///
/// A resized list is NOT descended into: once the length moved, comparing the
/// n-th element against the n-th is comparing two different things, and the
/// index-shift shows up as a phantom finding one level down.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Resize {
    path: String,
    /// The datum's own name — the last path segment with any index stripped.
    field: String,
    before: usize,
    after: usize,
    /// A real list length moved, as opposed to a key appearing or vanishing.
    /// Only a list length can be read as an emptiness rule.
    list: bool,
}

impl Resize {
    fn new(path: &str, before: usize, after: usize, list: bool) -> Self {
        let field = path
            .rsplit('.')
            .next()
            .unwrap_or(path)
            .split('[')
            .next()
            .unwrap_or(path)
            .to_string();
        Resize {
            path: path.to_string(),
            field,
            before,
            after,
            list,
        }
    }

    /// What the STORE's own diff and a READ's diff have to share for the read to
    /// be merely carrying the edited datum: the same name moving the same way.
    fn signature(&self) -> (&str, usize, usize) {
        (self.field.as_str(), self.before, self.after)
    }

    /// The read this resize happened in — the root of the path, which the walk
    /// seeds with the verb. A list name means what its own read means by it, so
    /// the refuter is scoped here rather than across the whole surface.
    fn verb(&self) -> &str {
        self.path.split('.').next().unwrap_or(&self.path)
    }

    fn show(&self) -> String {
        format!("{}({}->{})", self.path, self.before, self.after)
    }
}

fn resized(base: &serde_json::Value, now: &serde_json::Value, path: &str, out: &mut Vec<Resize>) {
    match (base, now) {
        (serde_json::Value::Array(b), serde_json::Value::Array(n)) => {
            if b.len() != n.len() {
                out.push(Resize::new(path, b.len(), n.len(), true));
                return;
            }
            for (i, (b, n)) in b.iter().zip(n).enumerate() {
                resized(b, n, &format!("{path}[{i}]"), out);
            }
        }
        (serde_json::Value::Object(b), serde_json::Value::Object(n)) => {
            let mut moved = false;
            for key in n.keys().filter(|k| !b.contains_key(*k)) {
                out.push(Resize::new(&format!("{path}.{key}"), 0, 1, false));
                moved = true;
            }
            for key in b.keys().filter(|k| !n.contains_key(*k)) {
                out.push(Resize::new(&format!("{path}.{key}"), 1, 0, false));
                moved = true;
            }
            if moved {
                return;
            }
            for (key, bv) in b {
                if let Some(nv) = n.get(key) {
                    resized(bv, nv, &format!("{path}.{key}"), out);
                }
            }
        }
        _ => {}
    }
}

/// The rule a boundary-crossing resize proposes, and nothing else. A list that
/// FILLED proposes "this list is always empty"; a list that EMPTIED proposes
/// "this list is never empty". A move between two non-zero counts proposes
/// nothing — a bucket holding four instead of five is a tally, not a predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rule {
    AlwaysEmpty,
    NeverEmpty,
}

impl Rule {
    fn of(resize: &Resize) -> Option<Rule> {
        match (resize.list, resize.before, resize.after) {
            (true, 0, after) if after > 0 => Some(Rule::AlwaysEmpty),
            (true, before, 0) if before > 0 => Some(Rule::NeverEmpty),
            _ => None,
        }
    }

    fn sentence(self, field: &str) -> String {
        match self {
            Rule::AlwaysEmpty => format!("`{field}` is always empty"),
            Rule::NeverEmpty => format!("`{field}` is never empty"),
        }
    }

    fn refuted_by(self, len: usize) -> bool {
        match self {
            Rule::AlwaysEmpty => len > 0,
            Rule::NeverEmpty => len == 0,
        }
    }
}

/// Every authored corpus this repository TRACKS: a fact manifest with a
/// `sections.json` and an `order.json` beside it. Asked of `git ls-files`, not
/// of the working directory — an untracked stray is not evidence about what an
/// author ships, and a walk over the filesystem would count it.
fn authored_corpora() -> Vec<PathBuf> {
    let listed = std::process::Command::new("git")
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

/// One rule the walk proposes, named by the read it lives in and the list it is
/// about — a list name means what its own read means by it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Proposed {
    rule: Rule,
    verb: String,
    field: String,
}

/// What is known about a proposal: whether the authored corpus already breaks
/// it, and which corruptions asked for it.
struct Evidence {
    refuted: bool,
    proposed_by: Vec<String>,
}

/// Every list the AUTHORED store's reads hold, indexed by the datum's own name.
/// This is the refuter: the one blind-authored corpus is evidence about what an
/// author legitimately ships, so a rule the corpus already violates is a rule
/// that would reject the author. Keyed by name across the whole surface rather
/// than by an exact path, which errs toward refutation — the safe direction.
fn index_lists(v: &serde_json::Value, key: Option<&str>, out: &mut BTreeMap<String, Vec<usize>>) {
    match v {
        serde_json::Value::Array(a) => {
            if let Some(k) = key {
                out.entry(k.to_string()).or_default().push(a.len());
            }
            for element in a {
                index_lists(element, None, out);
            }
        }
        serde_json::Value::Object(o) => {
            for (k, value) in o {
                index_lists(value, Some(k), out);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Bucket {
    Refused,
    Caught,
    Reported,
    Carried,
    Inert,
}

impl Bucket {
    fn as_str(self) -> &'static str {
        match self {
            Bucket::Refused => "REFUSED",
            Bucket::Caught => "CAUGHT",
            Bucket::Reported => "REPORTED",
            Bucket::Carried => "CARRIED",
            Bucket::Inert => "INERT",
        }
    }
}

/// One read's answer about one store. A verb that takes `--json` and answers in
/// prose anyway can still be compared for difference, but it holds no list, so
/// it can never be asked whether it STARTED SAYING SOMETHING — a limit of the
/// panel that is named and printed rather than absorbed.
enum Answer {
    Json(serde_json::Value),
    Prose(String),
}

impl Answer {
    fn read(stdout: Vec<u8>) -> Self {
        let text = String::from_utf8(stdout).expect("cli output is utf-8");
        match serde_json::from_str(&text) {
            Ok(json) => Answer::Json(json),
            Err(_) => Answer::Prose(text),
        }
    }
}

/// What every advertised read said about one store.
struct Panelled {
    failed: Vec<String>,
    answers: BTreeMap<String, Answer>,
}

fn ask_panel(ws: &Path, panel: &[Read]) -> Panelled {
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

#[test]
fn the_walk_says_what_stands_between_an_authoring_slip_and_the_runtime() {
    let facts_json = dnd_quest_facts();
    let baseline_ws = dnd_quest_workspace_try(&facts_json).expect("the authored store must load");
    let baseline_store =
        AtomicStore::load(&baseline_ws.path().join(SIDECAR)).expect("the imported store loads");
    let baseline_sidecar = read_sidecar(baseline_ws.path());
    let telling = telling_of(&baseline_store);

    let (panel, unaskable) = panel(baseline_ws.path(), &telling);
    let baseline = ask_panel(baseline_ws.path(), &panel);
    assert!(
        baseline.failed.is_empty(),
        "the panel is exactly the reads that answered at baseline: {:?}",
        baseline.failed
    );

    let population = corruptions(&baseline_store, &facts_json);
    assert!(
        population.len() >= 30,
        "the derived population collapsed to {} — a walk that finds almost \
         nothing reads exactly like a store with almost no surface",
        population.len()
    );

    // The authored corpus's own lists, which is what a proposed rule is refuted
    // against. Round 1032 learned this the hard way: a stricter reading felt
    // safer and turned out to REJECT the blind author's store, and that
    // rejection was the proof the reading was wrong.
    // Round 1035 asked the ONE corpus under corruption and recorded "a second
    // authored corpus is what would move it". The refuter's population is not
    // the corpus being corrupted, though — it is every store an author actually
    // shipped, and this repository tracks forty-three of them. Each is built and
    // read the same way; a corpus that no longer loads is excluded by that
    // failure and named, never by a judgement about relevance.
    let mut baseline_lists: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    let mut refuters: Vec<String> = Vec::new();
    let mut unloadable: Vec<String> = Vec::new();
    // How much evidence actually reached the refuter. A corpus that loads but
    // whose reads start failing loses evidence SILENTLY — the rules it used to
    // refute quietly become candidates — so the total is asserted rather than
    // printed. Round 1036 found this by aiming an injection at exactly that
    // path and watching nothing turn red.
    let mut answers_total = 0usize;
    // The store under corruption is itself an authored store, and it must be in
    // the refuter's population. Widening the sweep WITHOUT this LOST evidence:
    // the dnd-quest record's own `facts.json` is the pre-migration manifest that
    // stopped loading (the rot R857 found), so the tracked-corpus sweep excludes
    // it and every quest-bearing list went to n=0 — three refutations turned
    // into candidates because the population grew.
    for (verb, answer) in &baseline.answers {
        if let Answer::Json(json) = answer {
            let mut per_read = BTreeMap::new();
            index_lists(json, None, &mut per_read);
            for (field, lens) in per_read {
                baseline_lists
                    .entry((verb.clone(), field))
                    .or_default()
                    .extend(lens);
            }
        }
    }
    refuters.push("the migrated dnd-quest record (the store under corruption)".to_string());
    for dir in authored_corpora() {
        let name = dir
            .strip_prefix(repo_root())
            .unwrap_or(&dir)
            .display()
            .to_string();
        let facts = read_sidecar_at(&dir.join("facts.json"));
        let Ok(ws) = corpus_workspace_try(&dir, &facts) else {
            unloadable.push(name);
            continue;
        };
        let Ok(store) = AtomicStore::load(&ws.path().join(SIDECAR)) else {
            unloadable.push(name);
            continue;
        };
        // Each corpus answers under its OWN telling, or bare when it declares
        // none or more than one — the walk invents no argument for anyone.
        let its_telling = match store.disclosure_plans.keys().collect::<Vec<_>>()[..] {
            [only] => Some(only.clone()),
            _ => None,
        };
        let mut answered = 0usize;
        for read in &panel {
            let mut argv = vec![read.verb.as_str()];
            if !read.args.is_empty() {
                let Some(telling) = its_telling.as_deref() else {
                    continue;
                };
                argv.extend(["--telling", telling]);
            }
            argv.push("--json");
            let out = run(ws.path(), &argv);
            if !out.status.success() {
                continue;
            }
            if let Answer::Json(json) = Answer::read(out.stdout) {
                answered += 1;
                let mut per_read = BTreeMap::new();
                index_lists(&json, None, &mut per_read);
                for (field, lens) in per_read {
                    baseline_lists
                        .entry((read.verb.clone(), field))
                        .or_default()
                        .extend(lens);
                }
            }
        }
        answers_total += answered;
        refuters.push(format!("{name} ({answered} reads)"));
    }

    let mut census: BTreeMap<Bucket, usize> = BTreeMap::new();
    let mut rows: BTreeMap<Bucket, Vec<String>> = BTreeMap::new();
    let mut proposals: BTreeMap<Proposed, Evidence> = BTreeMap::new();
    for (index, corruption) in population.iter().enumerate() {
        let mut mutated = facts_json.clone();
        let mut applied = 0usize;
        for entry in mutated["facts"].as_array_mut().expect("facts array") {
            if entry["fact_id"] == corruption.fact.as_str() {
                (corruption.apply)(entry);
                applied += 1;
            }
        }
        assert_eq!(
            applied, 1,
            "corruption {index} ({}/{}) applied {applied} times",
            corruption.fact, corruption.leg
        );

        let mut note = String::new();
        let bucket = match dnd_quest_workspace_try(&mutated) {
            Err(_) => Bucket::Refused,
            Ok(ws) => {
                let seen = ask_panel(ws.path(), &panel);
                if !seen.failed.is_empty() {
                    note = format!(" <- {}", seen.failed.join(" "));
                    Bucket::Caught
                } else {
                    // What the manifest edit did to the STORE, before any read
                    // had an opinion: the corruption's own echo.
                    let mut store_paths = Vec::new();
                    resized(
                        &baseline_sidecar,
                        &read_sidecar(ws.path()),
                        "store",
                        &mut store_paths,
                    );
                    let echo: BTreeSet<(&str, usize, usize)> =
                        store_paths.iter().map(Resize::signature).collect();
                    let mut growth: Vec<Resize> = Vec::new();
                    let mut differing: Vec<&str> = Vec::new();
                    for read in &panel {
                        match (&baseline.answers[&read.verb], &seen.answers[&read.verb]) {
                            (Answer::Json(before), Answer::Json(after)) => {
                                if before == after {
                                    continue;
                                }
                                differing.push(read.verb.as_str());
                                resized(before, after, &read.verb, &mut growth);
                            }
                            (Answer::Prose(before), Answer::Prose(after)) => {
                                if before != after {
                                    differing.push(read.verb.as_str());
                                }
                            }
                            _ => panic!(
                                "`{}` answered in one shape at baseline and another here",
                                read.verb
                            ),
                        }
                    }
                    let derived: Vec<&Resize> = growth
                        .iter()
                        .filter(|r| !echo.contains(&r.signature()))
                        .collect();
                    // Each boundary-crossing resize proposes a rule; the
                    // AUTHORED corpus is asked whether it already breaks it.
                    for resize in &derived {
                        let Some(rule) = Rule::of(resize) else {
                            continue;
                        };
                        let key = (resize.verb().to_string(), resize.field.clone());
                        let refuted = baseline_lists
                            .get(&key)
                            .is_some_and(|lens| lens.iter().any(|len| rule.refuted_by(*len)));
                        proposals
                            .entry(Proposed {
                                rule,
                                verb: key.0,
                                field: key.1,
                            })
                            .or_insert(Evidence {
                                refuted,
                                proposed_by: Vec::new(),
                            })
                            .proposed_by
                            .push(format!("{}/{}", corruption.fact, corruption.leg));
                    }
                    if !derived.is_empty() {
                        note = format!(
                            " <- {}",
                            derived
                                .iter()
                                .map(|r| r.show())
                                .collect::<Vec<_>>()
                                .join(" ")
                        );
                        Bucket::Reported
                    } else if !differing.is_empty() {
                        note = format!(" <- {}", differing.join(" "));
                        Bucket::Carried
                    } else {
                        Bucket::Inert
                    }
                }
            }
        };
        *census.entry(bucket).or_default() += 1;
        rows.entry(bucket)
            .or_default()
            .push(format!("{}/{}{note}", corruption.fact, corruption.leg));
    }

    // Print BEFORE asserting: a first-violation stop would make the whole walk
    // report one line (the R1026 lesson).
    println!(
        "panel: {} advertised reads asked, {} unaskable by this corpus",
        panel.len(),
        unaskable.len()
    );
    for read in &panel {
        println!("    ask  {}", read.argv().join(" "));
    }
    for (verb, reason) in &unaskable {
        println!("    skip {verb} :: {reason}");
    }
    let prose_only: Vec<&str> = baseline
        .answers
        .iter()
        .filter(|(_, a)| matches!(a, Answer::Prose(_)))
        .map(|(verb, _)| verb.as_str())
        .collect();
    println!(
        "    of those, {} answer `--json` in prose, so they hold no list and \
         cannot be asked whether they started saying something: {}",
        prose_only.len(),
        prose_only.join(" ")
    );
    println!(
        "\nplay-break class census over {} derived corruptions:",
        population.len()
    );
    for (bucket, n) in &census {
        println!("  {:9} {n}", bucket.as_str());
    }
    for (bucket, listed) in &rows {
        println!("\n{} — {}:", bucket.as_str(), listed.len());
        for row in listed {
            println!("    {row}");
        }
    }

    println!(
        "\nrefuters: {} authored corpora loaded, {} unloadable, {} (read, list) \
         names indexed",
        refuters.len(),
        unloadable.len(),
        baseline_lists.len()
    );
    for name in &refuters {
        println!("    load {name}");
    }
    for name in &unloadable {
        println!("    dead {name}");
    }
    println!("\nrules the REPORTED rows propose, put to every authored corpus:");
    for (proposed, evidence) in &mut proposals {
        let Proposed { rule, verb, field } = proposed;
        let Evidence {
            refuted,
            proposed_by: from,
        } = evidence;
        from.sort();
        from.dedup();
        let lens = baseline_lists
            .get(&(verb.clone(), field.clone()))
            .cloned()
            .unwrap_or_default();
        // Print the EVIDENCE, not just the verdict. One corpus can only refute:
        // a rule it does not break is un-refuted, never confirmed, and how close
        // the authored store sits to breaking it is the whole of what is known.
        println!(
            "  {:9} {}: {} [authored: n={} min={} max={}] <- {}",
            if *refuted { "REFUTED" } else { "CANDIDATE" },
            verb,
            rule.sentence(field),
            lens.len(),
            lens.iter().min().copied().unwrap_or(0),
            lens.iter().max().copied().unwrap_or(0),
            from.join(" ")
        );
    }

    assert_eq!(
        census.values().sum::<usize>(),
        population.len(),
        "every corruption lands in exactly one bucket"
    );

    // The panel is the shipped surface, so a new verb has to be LOOKED AT
    // rather than absorbed. The unaskable four are earned, not curated: each
    // was asked at baseline and refused for a stated missing argument this
    // corpus does not supply.
    assert_eq!(
        unaskable
            .iter()
            .map(|(verb, _)| verb.as_str())
            .collect::<Vec<_>>(),
        [
            "report-entity",
            "report-frame-view",
            "validate-disclosure-leak",
            "validate-render-fidelity",
        ],
        "the reads this corpus cannot ask without inventing an argument"
    );
    assert_eq!(
        prose_only,
        ["validate-workspace"],
        "the reads that take `--json` and answer in prose — they hold no list, \
         so this walk can see them REJECT but never see them RECLASSIFY"
    );

    // THE NUMBER THIS ROUND EXISTS TO PRODUCE.
    //
    // REFUSED is 0 and that is a measurement: every corruption carries its own
    // entities list (`swap_entity`), which is what Round 1031 learned the write
    // path demands. An author who keeps the store's own invariants can commit
    // all 41.
    //
    // INERT is 0, and it overturns a claim Round 1033 pinned. That walk ran
    // three stations and found two payoff edges neither runtime projection
    // carried; it scoped the finding honestly to those two reads. Asked of the
    // whole shipped surface, `report-payoff-coverage` and
    // `report-payoff-substantiation` both reclassify them. NOTHING in the quest
    // layer is invisible to every consumer.
    assert_eq!(
        census,
        BTreeMap::from([
            (Bucket::Caught, 6),
            (Bucket::Reported, 19),
            (Bucket::Carried, 16),
        ]),
        "the play-break census over the quest layer"
    );

    // THE ANSWER TO ROUND 1033'S QUESTION, named rather than counted: the
    // corruptions for which no shipped read derives anything at all. Every
    // other row moves some classification the system already computes, so a
    // gate would have a place to stand and the question is policy. These
    // sixteen have no such place — a rendering read carries the altered datum
    // and nothing else in the system has an opinion. Two legs, and the shape of
    // both is that the store states the datum ONCE: a retargeted claim object
    // is a different story rather than a broken one, and a dropped evidence
    // backreference removes the only record that the link was ever claimed.
    // This is the R476 ceiling — author content, not missing enforcement —
    // showing up one layer down, and it is 16 of 41 rather than 33.
    assert_eq!(
        rows[&Bucket::Carried]
            .iter()
            .map(|row| row.split(" <-").next().expect("row head").to_string())
            .collect::<Vec<_>>(),
        [
            "f-161/typed.object",
            "f-161/evidence",
            "f-180/evidence",
            "f-305/typed.object",
            "f-305/evidence",
            "f-316/typed.object",
            "f-316/evidence",
            "f-404/typed.object",
            "f-404/evidence",
            "f-409/typed.object",
            "f-409/evidence",
            "f-411/evidence",
            "f-505/typed.object",
            "f-505/evidence",
            "f-515/typed.object",
            "f-515/evidence",
        ],
        "the corruptions no shipped read derives anything from"
    );

    // THE SECOND ANSWER: of the classifications the REPORTED rows move, which
    // could become a gate. A rule is proposed only by a resize that CROSSES the
    // emptiness boundary — a bucket holding four instead of five is a tally,
    // not a predicate — and it is then put to the blind author's own store.
    //
    // The corpus can only REFUTE (Round 1032). Three of these rules would
    // reject the store an author actually shipped, and one of them,
    // `completions is never empty`, is the debt the previous round filed as its
    // next candidate: "a quest still open on a terminal road". Sixteen authored
    // (quest, world) pairs already sit at zero. The gate this arc was about to
    // build would have rejected the blind author, and a measurement said so
    // rather than a reviewer.
    //
    // Round 1035 asked ONE corpus and let `dangling is never empty` survive,
    // recording that a finished story would refute it and that no second corpus
    // existed to ask. The second premise was wrong: this repository tracks 43
    // authored corpora and 27 of them still load. Asked of all of them,
    // `dangling` IS refuted — some author did finish paying their setups — and
    // the surviving `payoffs_to_unmarked` rule now rests on 41 authored worlds
    // rather than 4, every one of them at zero.
    //
    // Sixteen of the 43 no longer load, including the dnd-quest record's own
    // pre-migration manifest. That is the rot Round 857 found, still live, and
    // it is counted here rather than mentioned: a corpus that stops loading
    // silently shrinks the evidence every rule in this list is judged against.
    assert_eq!(
        (refuters.len(), unloadable.len(), answers_total),
        (28, 16, 660),
        "the refuter population: authored corpora that load, those that no \
         longer do, and the (corpus, read) answers that actually reached the \
         index"
    );
    assert_eq!(
        proposals
            .iter()
            .map(|(proposed, evidence)| format!(
                "{} {}: {}",
                if evidence.refuted {
                    "REFUTED"
                } else {
                    "CANDIDATE"
                },
                proposed.verb,
                proposed.rule.sentence(&proposed.field)
            ))
            .collect::<Vec<_>>(),
        [
            "CANDIDATE report-payoff-coverage: `payoffs_to_unmarked` is always empty",
            "REFUTED report-payoff-coverage: `dangling` is never empty",
            "CANDIDATE report-quest-graph: `actors` is never empty",
            "REFUTED report-quest-graph: `completions` is never empty",
            "REFUTED report-quest-graph: `giving_facts` is never empty",
            "REFUTED report-quest-graph: `locators` is never empty",
        ],
        "the rules the walk's own findings propose, put to the authored corpus"
    );
}
