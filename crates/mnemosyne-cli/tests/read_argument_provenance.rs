//! An answer that depends on an argument has to say which argument it was
//! given. (Round 1048.)
//!
//! Round 1040 left a backlog of read PAIRS to declare agreements over, and the
//! pair it ranked first — `report-playable-world <-> report-playthrough-
//! manuscript`, 72 subjects in common — agrees exactly, because one read is
//! literally built on the other. What writing that check found is not a
//! disagreement but something the pair could not have shown on its own: the
//! read UNDERNEATH cannot say which question it answered. One store, one verb,
//! three different manuscripts (bare, and one per declared telling), and
//! nothing in the answer to tell them apart.
//!
//! SO THE UNIT OF WORK WAS WRONG. Declaring the 24 remaining pairs one at a
//! time would have asked "do these two agree" 24 times and never asked "can
//! either of them say what it was asked", which is a property of ONE read and
//! is derivable for the whole shipped surface at once. This walk derives it.
//!
//! THE DERIVATION. For every advertised `report-*` / `validate-*` verb, the
//! flags on its own usage line; for each flag, a value taken from the store's
//! own vocabulary (a telling it declares, a branch it registers, an entity it
//! holds — never an invented one, the R1034 panel rule). Ask the read twice,
//! once with the flag and once without it (or with a second value, when the
//! flag is required). Then:
//!
//! - the answers are equal ONCE THE RECORD IS STRIPPED — the argument is inert
//!   for this corpus, and there is nothing to name. Recorded, never a failure.
//! - the substance DIFFERS — the argument decided the answer, so the answer must
//!   TRACK it: some top-level scalar field whose value is the argument in one
//!   probe and absent/`null`/`false` in the other. Not "the value appears
//!   somewhere": a substring search would pass on the id printed in the data,
//!   which is the datum, not the provenance.
//!
//! THE RECORD IS STRIPPED BEFORE SUBSTANCE IS JUDGED, or the rule would be
//! circular — recording an argument makes two answers differ BY THAT RECORD, so
//! any field that names itself would satisfy "the answer changed and the answer
//! names it". `--reading-walk` is exactly that case and reads INERT here: no
//! authored corpus has a scene the reading prune removes, and the tree carries
//! the class instead (`the_reading_walk_prunes_contentless_scenes_and_the_report_says_it_did`).
//!
//! WHY THIS IS AN ORACLE AND NOT A CONVENTION THIS ROUND INVENTED: eight of the
//! twelve arguments that move an answer's substance ALREADY tracked.
//! `report-frame-view` names all four of its, `report-entity` names its entity,
//! `report-authoring-frontier` and `report-disclosure-coverage` name their
//! tellings. The rule was the repository's before this round measured it; four
//! cells did not follow it, and all four were the world-line filter and the
//! telling on the three narrative projections — the reads a runtime consumes.
//!
//! Asked of every store an author shipped that this tree can ask
//! (`authored_stores()`, the R1042 resolver), because a verdict from one corpus
//! is a verdict about one vocabulary: `--telling` cannot be probed differentially
//! at all on a corpus that declares one, and three corpora declare two.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

mod common;
use common::{advertised_reads, authored_stores, cli_binary, read_sidecar, run, SIDECAR};

/// What one (verb, flag) probe concluded on one corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    /// The two probes answered byte-identically: this corpus cannot say
    /// whether the argument shapes the answer, so it demands nothing.
    Inert,
    /// The answers differ and a top-level field tracks the argument.
    Named,
    /// The answers differ and nothing in the answer says which was asked.
    Unnamed,
}

/// One flag on one verb's usage line, and how to give it a value this corpus
/// can actually supply.
struct Flag {
    name: String,
    /// `None` = a boolean flag (no value token follows it).
    takes_value: bool,
    /// `true` when the usage line lists it OUTSIDE brackets — it must be on
    /// every probe, so the differential is between two values rather than
    /// between present and absent.
    required: bool,
}

/// The flags on a verb's usage line, in the order the line lists them.
///
/// Read from the line the shipped `--help` prints, never from a table here: a
/// verb that grows a flag is covered the run it ships, which is the property a
/// hand list cannot have (the R1046 lesson — a population keyed by a hand list
/// is blind to the element nobody wrote down).
fn flags_of(usage: &str) -> Vec<Flag> {
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

/// Why a flag is outside this walk's reach at all — an EARNED exclusion, named
/// and counted rather than filtered away, so the census says what it could not
/// decide instead of quietly shrinking (the R1029 rule: an exception trades
/// evidence away, so it has to be spelled).
fn unprobable(flag: &str) -> Option<&'static str> {
    match flag {
        // Every read here is asked `--json`, so a probe that also passes it
        // compares an answer to itself. The flag is real and shapes the wire;
        // this walk is structurally the wrong instrument for it, and the census
        // says so rather than printing 26 meaningless `inert` rows.
        "--json" => Some("the walk asks every read in `--json`, so it cannot differ"),
        // These name a FILE. A corpus ships one order, one rules file, one
        // sidecar; there is no second one of the corpus's OWN to swap in, and
        // pointing at another corpus's file is the invented argument the panel
        // refuses to make (R1034).
        "--sidecar" | "--order" | "--rules" | "--catalog" | "--against" => {
            Some("names a file, and the corpus ships no second one of its own")
        }
        _ => None,
    }
}

/// The values this corpus can supply for a flag, read from the store itself.
///
/// An id the corpus never declared is an invented argument, and the panel's
/// rule since R1034 is that the walk supplies none. `main` is the exception the
/// STORE makes rather than this walk: it is a world every store has whether or
/// not it registers a branch.
fn values_for(flag: &str, store: &AtomicStore) -> Vec<String> {
    let ids = |set: Vec<String>| set;
    match flag {
        "--telling" => ids(store
            .disclosure_plans
            .keys()
            .map(ToString::to_string)
            .collect()),
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

/// Every top-level scalar field of an answer, as `(key, rendered value)`.
/// Nested objects are the DATA — an id inside `worlds` is what the read is
/// talking about, not a record of what it was asked.
fn provenance_fields(answer: &serde_json::Value) -> BTreeMap<String, String> {
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
/// answer's RECORD of what it was asked.
///
/// The "something else" matters as much as the match. A field that reads `main`
/// under both probes is a constant, not provenance; a field that reads `main`
/// under `--world main` and `null` without it is the read saying what it was
/// asked. A missing key counts as the absent side: the sibling that has named
/// its telling since R556 spells `null`, and this walk holds the weaker of the
/// two encodings so it measures the surface rather than a house style.
fn record_of(
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
/// This split is what keeps the walk from being circular. RECORDING an argument
/// makes two answers differ by that record, so "if the answers differ the
/// answer must name the argument" would be satisfied by any field that names
/// itself: an argument with no other effect would read as "changes the answer,
/// and is named". Only the tracking fields come out — a scalar like the
/// manuscript's `facts` count is substance and stays in, so an argument that
/// moved only it would still be measured.
fn substance(answer: &serde_json::Value, record: &BTreeSet<String>) -> serde_json::Value {
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

#[test]
fn every_argument_that_decides_an_answer_is_named_in_the_answer() {
    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();

    // (verb, flag) -> the verdicts every corpus reached, and who could not.
    let mut verdicts: BTreeMap<(String, String), BTreeSet<Verdict>> = BTreeMap::new();
    let mut witnesses: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut unprobed: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut outside: BTreeMap<(String, String), &'static str> = BTreeMap::new();
    let mut probes = 0usize;

    for store in &stores {
        let ws = store.ws.path();
        let atomic = match AtomicStore::load(&ws.join(SIDECAR)) {
            Ok(atomic) => atomic,
            Err(_) => continue,
        };
        // The usage line of every advertised read, from the shipped help.
        let help = run(ws, &["--help"]);
        assert!(help.status.success(), "--help must exit 0");
        let help = String::from_utf8(help.stdout).expect("help is utf-8");
        let mut usage_of: BTreeMap<String, String> = BTreeMap::new();
        for line in help.lines() {
            let mut tokens = line.split_whitespace().skip_while(|t| *t != cli_binary());
            if tokens.next().is_none() {
                continue;
            }
            let Some(verb) = tokens.next() else { continue };
            if !(verb.starts_with("report-") || verb.starts_with("validate-")) {
                continue;
            }
            usage_of
                .entry(verb.to_string())
                .or_insert_with(|| tokens.collect::<Vec<_>>().join(" "));
        }
        // The panel derivation is the shared one; this walk only adds the
        // usage line, so the two cannot come to disagree about which verbs the
        // CLI advertises.
        for verb in advertised_reads(ws) {
            let Some(usage) = usage_of.get(&verb) else {
                *unprobed.entry("advertised with no usage line").or_default() += 1;
                continue;
            };
            let mut flags = flags_of(usage);
            flags.retain(|flag| match unprobable(&flag.name) {
                Some(why) => {
                    outside.insert((verb.clone(), flag.name.clone()), why);
                    false
                }
                None => true,
            });
            // The baseline argv: every REQUIRED flag at its first value. A verb
            // whose required flag this corpus cannot supply is unaskable here,
            // which is a measurement about the corpus, not a skip.
            let mut base: Vec<String> = Vec::new();
            let mut unaskable = false;
            for flag in &flags {
                if !flag.required {
                    continue;
                }
                let values = values_for(&flag.name, &atomic);
                match values.first() {
                    Some(first) if flag.takes_value => {
                        base.push(flag.name.clone());
                        base.push(first.clone());
                    }
                    // A required flag with no value this corpus declares (a
                    // file path, an id it does not hold).
                    _ if flag.takes_value => unaskable = true,
                    _ => base.push(flag.name.clone()),
                }
            }
            if unaskable {
                *unprobed
                    .entry("a required flag has no value this corpus declares")
                    .or_default() += 1;
                continue;
            }
            let ask = |extra: &[String]| -> Option<serde_json::Value> {
                let mut argv: Vec<&str> = vec![verb.as_str()];
                argv.extend(base.iter().map(String::as_str));
                argv.extend(extra.iter().map(String::as_str));
                argv.push("--json");
                let out = run(ws, &argv);
                out.status
                    .success()
                    .then(|| serde_json::from_slice(&out.stdout).ok())
                    .flatten()
            };
            let Some(baseline) = ask(&[]) else {
                *unprobed
                    .entry("refuses or answers `--json` in prose")
                    .or_default() += 1;
                continue;
            };
            for flag in &flags {
                let values = values_for(&flag.name, &atomic);
                // The differential: a required flag varies its VALUE, an
                // optional one varies between present and absent.
                let (extra, supplied) = if flag.required {
                    let Some(second) = values.get(1) else {
                        *unprobed
                            .entry("a required flag this corpus gives one value for")
                            .or_default() += 1;
                        continue;
                    };
                    let mut argv = Vec::new();
                    for (index, token) in base.iter().enumerate() {
                        if index >= 1 && base[index - 1] == flag.name {
                            argv.push(second.clone());
                        } else {
                            argv.push(token.clone());
                        }
                    }
                    // Rebuilt whole, so the probe replaces this flag's value
                    // and leaves every sibling required flag as it was.
                    (argv, Some(second.clone()))
                } else if flag.takes_value {
                    let Some(first) = values.first() else {
                        *unprobed
                            .entry("an optional flag with no value this corpus declares")
                            .or_default() += 1;
                        continue;
                    };
                    (vec![flag.name.clone(), first.clone()], Some(first.clone()))
                } else {
                    (vec![flag.name.clone()], None)
                };
                // A required flag's probe REPLACES the baseline argv.
                let probed = if flag.required {
                    let mut argv: Vec<&str> = vec![verb.as_str()];
                    argv.extend(extra.iter().map(String::as_str));
                    argv.push("--json");
                    let out = run(ws, &argv);
                    out.status
                        .success()
                        .then(|| serde_json::from_slice::<serde_json::Value>(&out.stdout).ok())
                        .flatten()
                } else {
                    ask(&extra)
                };
                let Some(probed) = probed else {
                    *unprobed.entry("the probe is refused").or_default() += 1;
                    continue;
                };
                probes += 1;
                let record = record_of(&probed, &baseline, supplied.as_deref());
                let verdict = if substance(&probed, &record) == substance(&baseline, &record) {
                    Verdict::Inert
                } else if record.is_empty() {
                    Verdict::Unnamed
                } else {
                    Verdict::Named
                };
                let key = (verb.clone(), flag.name.clone());
                verdicts.entry(key.clone()).or_default().insert(verdict);
                if verdict == Verdict::Unnamed {
                    witnesses.entry(key).or_default().push(store.name.clone());
                }
            }
        }
    }

    // Print BEFORE asserting — the distribution is the finding, and stopping at
    // the first violation reports one line of a walk over 28 corpora (the R1026
    // lesson).
    let settled = |key: &(String, String)| -> Verdict {
        let seen = &verdicts[key];
        if seen.contains(&Verdict::Unnamed) {
            Verdict::Unnamed
        } else if seen.contains(&Verdict::Named) {
            Verdict::Named
        } else {
            Verdict::Inert
        }
    };
    println!(
        "{asked} authored stores asked, {probes} (verb, flag, corpus) probes ran over \
         {} (verb, flag) cells",
        verdicts.len(),
    );
    for (why, count) in &unprobed {
        println!("  {count:5} probes not run: {why}");
    }
    let mut earned: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for ((verb, flag), why) in &outside {
        earned
            .entry(why)
            .or_default()
            .push(format!("{verb} {flag}"));
    }
    for (why, cells) in &earned {
        println!("  {:5} cells outside the walk: {why}", cells.len());
    }
    for (key, seen) in &verdicts {
        println!(
            "  {:?} {:32} {:20} {:?}",
            settled(key),
            key.0,
            key.1,
            seen.iter().collect::<Vec<_>>()
        );
    }

    let census = |what: Verdict| verdicts.keys().filter(|k| settled(k) == what).count();
    let unnamed: Vec<String> = verdicts
        .keys()
        .filter(|key| settled(key) == Verdict::Unnamed)
        .map(|(verb, flag)| {
            format!(
                "`{verb} {flag}` changes the answer on {} corpora and no field of the \
                 answer says it was given",
                witnesses[&(verb.clone(), flag.clone())].len()
            )
        })
        .collect();

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    // THE POPULATION, asserted rather than printed. A derivation that quietly
    // stops probing passes by asking nothing (the R1036 hole).
    check(
        (asked, unloadable.len()) == (44, 16),
        "POPULATION (stores): every corpus an author shipped is asked, and the \
         ones the R857 rot closed are counted rather than dropped",
    );
    check(
        (verdicts.len(), probes, outside.len()) == (23, 485, 72),
        "POPULATION (cells): the (verb, flag) cells the shipped usage lines and \
         the corpora's own vocabularies put in front of this walk, and the ones \
         no probe of this shape can decide",
    );
    // NON-VACUITY: the walk must reach all three verdicts, or the arm that
    // matters is being carried by nothing. `Named` is the one that makes this
    // an oracle rather than a rule invented here — the surface already tracks
    // its arguments in most cells, and the failures are the minority.
    check(
        (census(Verdict::Named), census(Verdict::Inert)) == (12, 11),
        "CENSUS: the arguments whose record the answer carries, and the ones no \
         authored corpus can make change an answer's substance at all. EIGHT of \
         the twelve were already named before this round — that majority is why \
         the rule below is the repository's own and not one invented here; the \
         four this round repaired are `report-playable-world --world`, \
         `report-quest-graph --world`, and both of \
         `report-playthrough-manuscript`'s",
    );

    check(
        unnamed.is_empty(),
        "CONTRACT: every argument that decides an answer is named in the answer",
    );

    for row in &unnamed {
        println!("    UNNAMED {row}");
    }
    assert_eq!(
        broken,
        Vec::<String>::new(),
        "a shipped read answers a question it cannot state"
    );
}

/// The same claim on the wire a person reads. (The R1045 lesson: an injection
/// that stripped a clause from the human line left the whole workspace green,
/// because every contract above reads `--json`.)
///
/// The three narrative projections print a header, and the header now says what
/// was asked. Checked by DIFFERENCE rather than by substring: the header under
/// `--world <road>` must not be the header without it, and it must name the
/// road — a report that prints the road count alone satisfies neither.
#[test]
fn the_prose_header_says_what_the_projection_was_asked() {
    let mut checked = 0usize;
    let mut wrong: Vec<String> = Vec::new();
    let (stores, _) = authored_stores();

    for store in &stores {
        let ws = store.ws.path();
        let name = &store.name;
        let Ok(atomic) = AtomicStore::load(&ws.join(SIDECAR)) else {
            continue;
        };
        let tellings: Vec<String> = atomic
            .disclosure_plans
            .keys()
            .map(ToString::to_string)
            .collect();
        // `main` is a world every store has, registered or not — the same
        // resolution the gate above uses, so the two walks cannot come to
        // disagree about which roads a corpus can be asked for.
        let mut roads: Vec<String> = read_sidecar(ws)["branches"]
            .as_object()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default();
        roads.push("main".to_string());
        roads.sort();
        roads.dedup();
        let Some(telling) = tellings.first() else {
            continue;
        };
        let road = &roads[0];

        for verb in [
            "report-playthrough-manuscript",
            "report-playable-world",
            "report-quest-graph",
        ] {
            let header = |argv: &[&str]| -> Option<String> {
                let out = run(ws, argv);
                out.status.success().then(|| {
                    String::from_utf8(out.stdout)
                        .expect("the report is utf-8")
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .to_string()
                })
            };
            let all = header(&[verb, "--telling", telling]);
            let one = header(&[verb, "--telling", telling, "--world", road]);
            let (Some(all), Some(one)) = (all, one) else {
                continue;
            };
            checked += 1;
            if !all.contains(&format!("telling `{telling}`")) {
                wrong.push(format!(
                    "{name}: `{verb}` header does not name its telling: {all}"
                ));
            }
            if !one.contains(&format!("world `{road}`")) {
                wrong.push(format!(
                    "{name}: `{verb} --world {road}` header does not name the road: {one}"
                ));
            }
            if !all.contains("(every road)") {
                wrong.push(format!(
                    "{name}: `{verb}` header does not say it was given no road filter: {all}"
                ));
            }
            // The third argument, on the one read that has it. The clause is
            // absent from the siblings because they cannot take a reading walk
            // at all — which is a different fact from having one and not being
            // given it, so it prints nothing rather than `no`.
            let walked = header(&[verb, "--telling", telling, "--reading-walk"]);
            let expected = if verb == "report-playthrough-manuscript" {
                ["reading walk `no`", "reading walk `yes`"]
            } else {
                ["", ""]
            };
            for (said, want) in [
                (&all, expected[0]),
                (walked.as_ref().unwrap_or(&all), expected[1]),
            ] {
                if want.is_empty() {
                    if said.contains("reading walk") {
                        wrong.push(format!(
                            "{name}: `{verb}` names a reading walk it cannot be asked for: {said}"
                        ));
                    }
                } else if !said.contains(want) {
                    wrong.push(format!(
                        "{name}: `{verb}` header does not say `{want}`: {said}"
                    ));
                }
            }
        }
    }

    println!("{checked} prose headers read");
    for row in &wrong {
        println!("    SILENT {row}");
    }
    // Non-vacuity: with nothing read the walk would pass over a wire that
    // prints nothing at all.
    assert_eq!(
        checked, 30,
        "the narrative-projection headers this corpus set puts in front of the walk"
    );
    assert_eq!(
        wrong,
        Vec::<String>::new(),
        "a human-facing header does not say which question it answered"
    );
}
