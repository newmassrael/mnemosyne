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

use crate::common;
use common::{
    advertised_reads, authored_stores, baseline_argv, flags_of, record_of, run, substance,
    usage_lines, values_for, SIDECAR,
};

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
        let usage_of = usage_lines(ws);
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
            let Some(base) = baseline_argv(&flags, &atomic, ws) else {
                *unprobed
                    .entry("a required flag has no value this corpus declares")
                    .or_default() += 1;
                continue;
            };
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
                let values = values_for(&flag.name, &atomic, ws);
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
        (asked, unloadable.len()) == (46, 3),
        "POPULATION (stores): every corpus an author shipped is asked, and the \
         ones that no longer load are counted rather than dropped",
    );
    check(
        (verdicts.len(), probes, outside.len()) == (23, 727, 72),
        "POPULATION (cells): the (verb, flag) cells the shipped usage lines and \
         the corpora's own vocabularies put in front of this walk, and the ones \
         no probe of this shape can decide",
    );
    // NON-VACUITY: the walk must reach all three verdicts, or the arm that
    // matters is being carried by nothing. `Named` is the one that makes this
    // an oracle rather than a rule invented here — the surface already tracks
    // its arguments in most cells, and the failures are the minority.
    check(
        (census(Verdict::Named), census(Verdict::Inert)) == (14, 9),
        "CENSUS: the arguments whose record the answer carries, and the ones no \
         authored corpus can make change an answer's substance at all. EIGHT of \
         them were already named before Round 1048 — that majority is why the \
         rule below is the repository's own and not one invented here; the four \
         that round repaired are `report-playable-world --world`, \
         `report-quest-graph --world`, and both of \
         `report-playthrough-manuscript`'s. THE THIRTEENTH ARRIVED BY BEING \
         FIXED: `report-timeline-gaps --world` read Inert here until Round 1049 \
         found that it scoped the PROSE loop alone, so the `--json` wire this \
         walk reads answered every road no matter what it was asked. THE \
         FOURTEENTH ARRIVED BY EVIDENCE: Round 1174 lit thirteen corpora that \
         had gone dark to schema drift, and one argument that read Inert only \
         because no loadable corpus could move it started moving. An argument \
         that decides nothing and an argument that is ignored are the same row \
         in this census, which is why the sibling walk asks the other question \
         — whether the filter SELECTS",
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
/// EVERY READ THAT TAKES A ROAD FILTER, derived from the shipped usage lines
/// rather than listed — Round 1049 repaired a fourth such read and a hand list
/// of three would not have covered it, which is the same blindness R1046 found
/// in the stack gates. For each argument the read HAS, the header says what it
/// was given; for each argument it does NOT have, the header says nothing.
/// That second half is the point: "not given" and "cannot be given" are the
/// same characters on a terminal unless the read declines to print the clause,
/// and `report-timeline-gaps` has no telling to be given.
#[test]
fn the_prose_header_says_what_the_projection_was_asked() {
    let mut checked = 0usize;
    let mut verbs: BTreeSet<String> = BTreeSet::new();
    let mut wrong: Vec<String> = Vec::new();
    let (stores, _) = authored_stores();

    for store in &stores {
        let ws = store.ws.path();
        let name = &store.name;
        let Ok(atomic) = AtomicStore::load(&ws.join(SIDECAR)) else {
            continue;
        };
        let usage_of = usage_lines(ws);
        // `main` is a world every store has, registered or not — the shared
        // resolution, so this walk and the gate above cannot come to disagree
        // about which roads a corpus can be asked for.
        let roads = values_for("--world", &atomic, ws);
        let tellings = values_for("--telling", &atomic, ws);
        let (Some(road), Some(telling)) = (roads.first(), tellings.first()) else {
            continue;
        };

        for verb in advertised_reads(ws) {
            let Some(usage) = usage_of.get(&verb) else {
                continue;
            };
            let flags = flags_of(usage);
            let has = |name: &str| flags.iter().any(|flag| flag.name == name);
            if !has("--world") {
                continue;
            }
            let Some(base) = baseline_argv(&flags, &atomic, ws) else {
                continue;
            };
            let header = |extra: &[&str]| -> Option<String> {
                let mut argv: Vec<&str> = vec![verb.as_str()];
                argv.extend(base.iter().map(String::as_str));
                argv.extend(extra);
                let out = run(ws, &argv);
                out.status.success().then(|| {
                    String::from_utf8(out.stdout)
                        .expect("the report is utf-8")
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .to_string()
                })
            };
            let (Some(all), Some(one)) = (header(&[]), header(&["--world", road])) else {
                continue;
            };
            checked += 1;
            verbs.insert(verb.clone());

            // The road filter, which every read in this population has.
            if !one.contains(&format!("world `{road}`")) {
                wrong.push(format!(
                    "{name}: `{verb} --world {road}` header does not name the road: {one}"
                ));
            }
            if !all.contains("world (every road)") {
                wrong.push(format!(
                    "{name}: `{verb}` header does not say it was given no road filter: {all}"
                ));
            }
            // The two arguments only SOME of them have. A read that cannot be
            // asked must print no clause at all, which is a different fact from
            // having one and not being given it.
            let telling_probe = header(&["--telling", telling]);
            let walk_probe = header(&["--reading-walk"]);
            for (argument, absent, given, probe) in [
                (
                    "--telling",
                    "telling (none)".to_string(),
                    format!("telling `{telling}`"),
                    &telling_probe,
                ),
                (
                    "--reading-walk",
                    "reading walk `no`".to_string(),
                    "reading walk `yes`".to_string(),
                    &walk_probe,
                ),
            ] {
                let clause = argument.trim_start_matches("--").replace('-', " ");
                if !has(argument) {
                    if all.contains(&clause) {
                        wrong.push(format!(
                            "{name}: `{verb}` names a {clause} it cannot be asked for: {all}"
                        ));
                    }
                    continue;
                }
                // A REQUIRED argument is already on the baseline, so the
                // unfiltered header names it; an optional one says so.
                let want = if flags
                    .iter()
                    .any(|flag| flag.name == argument && flag.required)
                {
                    given.clone()
                } else {
                    absent
                };
                if !all.contains(&want) {
                    wrong.push(format!(
                        "{name}: `{verb}` header does not say `{want}`: {all}"
                    ));
                }
                if let Some(probe) = probe {
                    if !probe.contains(&given) {
                        wrong.push(format!(
                            "{name}: `{verb}` given a {clause} does not say `{given}`: {probe}"
                        ));
                    }
                }
            }
        }
    }

    println!("{checked} prose headers read over {} reads", verbs.len());
    for verb in &verbs {
        println!("  {verb}");
    }
    for row in &wrong {
        println!("    SILENT {row}");
    }
    // Non-vacuity: with nothing read the walk would pass over a wire that
    // prints nothing at all.
    assert_eq!(
        (checked, verbs.len()),
        (44, 4),
        "the road-filtering headers this corpus set puts in front of the walk"
    );
    assert_eq!(
        wrong,
        Vec::<String>::new(),
        "a human-facing header does not say which question it answered"
    );
}
