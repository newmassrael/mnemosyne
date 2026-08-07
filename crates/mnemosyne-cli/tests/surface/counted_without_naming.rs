//! Which shipped reads answer a change with a NUMBER and nothing else?
//! (Round 1054.)
//!
//! Round 1053 changed a shipped wire because a contract could not be written
//! over it. The authoring frontier's scene census carried a COUNT, so two facts
//! trading the coordinates they are anchored at left the entire report
//! byte-identical, and no test over that read's output could have judged
//! identity. The finding was pinned in the PAIR file that happened to meet it —
//! and what it judges is a property of ONE read. The same shape had been
//! recorded once already, on `report-frame-view`'s `not_holding`, and written
//! down as a limit. A shape met twice by hand is a population nobody has asked
//! the program for; asking the whole read surface a hand-met question is what
//! Round 1049 did, and it came back with shipping defects.
//!
//! THE LAW. Take an edit an author could commit, and one read's answer before
//! and after. If the answer moved and EVERY moving thing in it is a number, that
//! number is the read's whole account of the change: a consumer learns that
//! something moved and cannot learn what. That is the defect. A count that rides
//! beside the list it counts is NOT in the class and this walk must not flag it —
//! `holding_count` moves only when `holding` does — and buying that
//! discrimination from a measurement rather than from a rule about field names is
//! the whole point. Round 1039 already measured what naming rules are worth here:
//! a field's vocabulary "is a fact about its TYPE, and no amount of looking at
//! output recovers it".
//!
//! THE POPULATIONS ARE BOTH DERIVED, AND NEITHER IS THIS FILE'S.
//!
//! - The reads: [`common::panel`], the shipped `--help` asked for every required
//!   argument the corpus can supply — 32 reads over 79 questions (R1051). A
//!   verdict here is about the READ, so the questions one verb answers are
//!   unioned: a number is named if SOME question names it.
//! - The edits: [`common::corruptions`], the store's own legs (R1033). This walk
//!   adds no edit of its own, which is what keeps it from finding the defect it
//!   went looking for.
//!
//! THE FIRST SWEEP CAME BACK EMPTY, AND THE POPULATION WAS WHY. Not one number
//! on the whole surface moved alone, and the reason was not that the surface is
//! clean: `corruptions` says it takes "the legs a fact ACTUALLY carries" and
//! carried only the ones saying what a fact CLAIMS — its typed object, its
//! predicate, its payoff edges, its evidence. Every count a shipped read emits
//! summarizes WHERE a fact is, WHOSE view holds it, or WHICH world-line authored
//! it, and no edit in that population touched any of the three. So the legs that
//! place a fact and attribute it were added where the population lives (41
//! corruptions became 92, none refused by the write path), and the walk was run
//! again. An empty answer from a probe that cannot reach the thing it asks about
//! is the failure this arc has now met three times.
//!
//! THE LAW IS ASKED OF EVERY PAIR OF ANSWERS, not of each answer against the
//! unedited store (Round 1056). What makes a number lossy is that it is NOT A
//! FUNCTION of what the answer names: two answers that name identical things and
//! disagree about it prove the number carries information nothing named carries.
//! Comparing against the baseline asks that of 92 pairs and leaves the escape
//! Round 1054 named — an edit that moves a name for an unrelated reason excuses
//! the number — so answers are bucketed by WHAT THEY NAME and every pair in a
//! bucket is compared. The first sweep under that law found one field the
//! baseline comparison could not:
//! [`payoff_substantiation_names_every_setup_it_counts`] is what it found, why
//! the pair mattered, and the wire this round changed to close it.
//!
//! ROUND 1054 PROPOSED A DIFFERENT NARROWING — scope the comparison to the
//! RECORD, the read's account of one subject — and this round implemented it,
//! swept it, and REFUTED it. Its first flag was the manuscript's per-scene
//! `holding_count` under 34 edits on two reads, and that count is derivable from
//! the events the same answer names in earlier rows:
//! [`the_manuscript_count_is_derivable_from_the_events_it_names`] proves it over
//! every road of every authored corpus. A read's account of a subject is not
//! bounded by one record when the records form a chain, so the record is the
//! wrong scope for this question — while being exactly the right one for "which
//! reads answer about this id" (`common::wrote_about`, whose addressing this
//! walk does use to say where a number sits).
//!
//! WHAT THE SECOND SWEEP FOUND is in [`COUNTED_WITHOUT_NAMING`], and this round
//! did not stop at the census: both wires it named now name what they count. The
//! third member of the class is one this walk CANNOT reach, and
//! [`the_frame_view_names_the_facts_it_calls_not_holding`] both proves it and
//! says why the sweep is blind to it.
//!
//! AND THEN THERE IS THE OTHER HALF OF THE QUESTION (Round 1057). Everything
//! above is DIFFERENTIAL — it compares answers, so it can only speak about a
//! number an edit MOVES, and its kindest verdict is `accompanied`: something
//! named moved alongside. That is not the number being a FUNCTION of the names,
//! which is what a consumer holding one answer actually needs, and Round 1056
//! filed it as undone for ten of its eleven fields. So the same sweep also asks,
//! of each answer on its own, WHICH of the things it names each number counts.
//! An ACCOUNT is a set of places whose names number exactly what the number
//! says; every answer proposes accounts and every answer refutes them, so the 93
//! stores are 93 trials rather than 92 comparisons. It is a gate where the
//! population moves the number and a printed proposal where it does not — an
//! account nothing ever tested is not evidence, and saying so is the difference
//! between a clean surface and a walk that reached nothing.
//!
//! ITS FIRST RUN FOUND A DEFECT IN THIS ARC'S OWN ADDRESSING, and the repair is
//! in `answer_addressing.rs`: a row's KEY is derived from the values, and
//! spelling it into the collapsed FIELD path made one place in one read carry
//! two names. This walk had been printing the quest graph's locator ordinal as
//! two fields since Round 1054 and nothing noticed, because a differential law
//! does not care what a place is called.
//!
//! The four numbers the account cannot explain are in [`NOT_A_COUNT_OF_NAMES`],
//! and none of them counts anything: each is an ordinal or a chain count, and
//! each has the test that states what it IS beside it there.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

use crate::common;
use crate::sweep;
use common::{registered_ids, wrote_about, Wrote, SIDECAR};
use sweep::Answer;

/// Every number in one answer, keyed by the FIELD it sits at.
///
/// The addressing is [`common::wrote_about`], the one derivation this arc has
/// for where a place in an answer IS — ids collapsed to `*` so a per-road map is
/// one field rather than one finding per road, and array rows collapsed to the
/// fields that key them. The values are kept as a list in the order the records
/// come in rather than summed: rows that trade values have moved, and a total
/// would say they had not, which is the very confusion this walk exists to find.
fn numbers_of(wrote: &Wrote) -> Numbers {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for per_record in wrote.numbers.values() {
        for (field, values) in per_record {
            out.entry(field.clone())
                .or_default()
                .extend(values.iter().cloned());
        }
    }
    out
}

/// Every WHOLE number in one answer, as (the field it sits at, the record it
/// sits in, what it says). One field appears once per record it sits in — the
/// manuscript's per-scene count is 823 of these — and an account of it has to
/// hold at every one of them.
///
/// A fraction is not a count of anything nameable and is returned as a tally
/// rather than judged: `branch_owned_density.*.density` is a ratio of two
/// numbers, so "which things is it counting" is the wrong question to ask it.
fn counts_of(wrote: &Wrote) -> (Vec<(&str, &str, usize)>, usize) {
    let mut out = Vec::new();
    let mut fractional = 0usize;
    for (record, fields) in &wrote.numbers {
        for (field, values) in fields {
            // A field carrying SEVERAL numbers in one record is a list of
            // numbers rather than a count, and "which names does it hold as
            // many of" is not a question about it. Counted with the fractions.
            let [value] = values.as_slice() else {
                fractional += values.len();
                continue;
            };
            match value.parse::<usize>() {
                Ok(count) => out.push((field.as_str(), record.as_str(), count)),
                Err(_) => fractional += 1,
            }
        }
    }
    (out, fractional)
}

/// The ids named IN SCOPE of one record: there, and in every record nested
/// inside it. A number is an account of what its own record covers, so a total
/// at the root may be accounted for by names anywhere and a per-scene count may
/// not — otherwise a count in one row could be excused by a list in another.
///
/// The addresses of nested records all begin with this one's, and lexicographic
/// order puts exactly those together, so the scope is one contiguous range.
fn scope_of<'a>(wrote: &'a Wrote, record: &str) -> Scope<'a> {
    let mut out: Scope<'a> = BTreeMap::new();
    // (the field, the field classifying it) -> value -> the ids named at the
    // first in the records where the second holds that value.
    let mut classified: BTreeMap<(&'a str, &'a str), BTreeMap<&'a str, BTreeSet<&'a str>>> =
        BTreeMap::new();
    for (address, fields) in wrote.names.range(record.to_string()..) {
        let nested = record.is_empty()
            || address == record
            || address
                .strip_prefix(record)
                .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['));
        if !nested {
            break;
        }
        let classifiers = classifiers_of(wrote.records.get(address));
        for (field, ids) in fields {
            out.entry(Place::Spelled(field.clone()))
                .or_default()
                .extend(ids.iter().map(String::as_str));
            for (key, value) in &classifiers {
                classified
                    .entry((field.as_str(), key))
                    .or_default()
                    .entry(value)
                    .or_default()
                    .extend(ids.iter().map(String::as_str));
            }
        }
    }
    // EVERY CLASS THIS ANSWER SORTS THOSE NAMES INTO, with none left out. Which
    // of them a search may COMPOSE with is a separate question and a separate
    // function ([`spelled`]), because the two answer to different things: an
    // account is proposed by one answer and put to all 93, and [`accounted`]
    // reads a place this answer lacks as one it names nothing at — the same
    // thing an empty list says. A scope that dropped a place because THIS store
    // happened to sort its rows finely would make that reading false, and would
    // refute an account for the shape of the store refuting it.
    for ((field, key), by_value) in classified {
        for (value, ids) in by_value {
            out.insert(
                Place::Classified {
                    field: field.to_string(),
                    key: key.to_string(),
                    value: value.to_string(),
                },
                ids,
            );
        }
    }
    out
}

/// The places the union search composes with, the empty-naming ones first.
///
/// ONLY THE FIELDS THE ANSWER SPELLS, and that bound is the subject of
/// [`classes_matching`]. An empty list goes first because it is where a store
/// puts ids it happens not to have, and an account that omits it is one the very
/// next store refutes for a reason that is not a defect.
fn spelled<'s, 'a>(scope: &'s Scope<'a>) -> Vec<(&'s Place, &'s BTreeSet<&'a str>)> {
    let mut out: Vec<(&'s Place, &'s BTreeSet<&'a str>)> = scope
        .iter()
        .filter(|(place, _)| !place.is_classified())
        .collect();
    out.sort_by(|a, b| (!a.1.is_empty(), a.0).cmp(&(!b.1.is_empty(), b.0)));
    out
}

/// The classified subsets of this scope that hold exactly `value` ids — each one
/// a WHOLE account rather than a term the search may add to others.
///
/// THIS IS THE MEASURED BOUND, and it is a bound on what an account may SAY, not
/// on how hard the walk tries. Round 1076 first let classified places into the
/// union search and ran it: the process reached 23.9 GiB resident and 24.2 GiB
/// of swap without finishing, because [`accountings`] accumulates every account
/// it finds and the count of unions reaching a given size grows with the size of
/// the vocabulary. That is a measurement the machine cannot afford — and the
/// answer it was buying is one nobody can read either, since a number explained
/// by any of a hundred thousand surviving hypotheses is a number nothing has
/// explained. Precision and cost failed in the same direction, which is what
/// says the bound belongs in the vocabulary rather than in the budget.
///
/// SO A SELECTION IS COMPLETE OR IT IS NOT AN ACCOUNT. A consumer holding the
/// answer either filters one list by one field and gets the number, or does not;
/// "these three lists plus the rows of that class" is not a shape any read on
/// this surface emits. If one ever does, it arrives here as a number no account
/// explains — printed, named in [`NOT_A_COUNT_OF_NAMES`], and the round that
/// meets it will have measured the need before paying for it.
///
/// A CLASSIFIER IS NOT A KEY is the one rule here, and the difference is the
/// data's rather than a constant: a key taking fewer values than there are ids
/// it sorts is sorting them into classes, and one taking as many is naming them
/// one apiece — a singleton per row, which accounts for the number 1 and for
/// nothing else.
///
/// "ONE CLASS IS NOT A CLASSIFICATION" WAS ALSO A RULE HERE FOR ONE RUN, and it
/// was wrong in a way worth keeping written down: it is true of one answer and
/// false of a population. A key holding a single value across the unedited store
/// does select everything it sorts THERE, so the subset is the spelled field and
/// proposing it looks redundant — and it is exactly the description that stops
/// being redundant the moment an edit splits the value, which is the only moment
/// this walk is asking about. Refusing to propose it cost the round the two
/// numbers it was built to reach: the account was never made in the baseline, so
/// what refuted the spelled one read as "nothing the answer names".
///
/// The rule that remains is contingent on the proposing answer too, and its
/// failure mode is the honest one: a number nothing proposed an account for
/// stays in [`NOT_A_COUNT_OF_NAMES`], printed beside how many accounts were
/// proposed for it and how many named a class. A search that cannot reach a
/// hypothesis says so; it never says "clean".
fn classes_matching<'s, 'a>(scope: &'s Scope<'a>, value: usize) -> Vec<&'s Place> {
    let mut taken: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for place in scope.keys() {
        if let Place::Classified { field, key, .. } = place {
            *taken.entry((field.as_str(), key.as_str())).or_default() += 1;
        }
    }
    scope
        .iter()
        .filter(|(place, ids)| {
            let Place::Classified { field, key, .. } = place else {
                return false;
            };
            let classes = taken
                .get(&(field.as_str(), key.as_str()))
                .copied()
                .unwrap_or_default();
            let named_here = scope
                .get(&Place::Spelled(field.clone()))
                .map_or(0, BTreeSet::len);
            ids.len() == value && classes < named_here
        })
        .map(|(place, _)| place)
        .collect()
}

/// The scalar fields of one record that could SORT the ids it names, as
/// `(key, value)`.
///
/// Only strings and booleans: a number in a record is the thing this whole walk
/// is trying to account for, and using one as a class would let a count excuse
/// itself with another count.
fn classifiers_of(record: Option<&serde_json::Value>) -> Vec<(&str, &str)> {
    record
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| match value {
            serde_json::Value::String(text) => Some((key.as_str(), text.as_str())),
            serde_json::Value::Bool(true) => Some((key.as_str(), "true")),
            serde_json::Value::Bool(false) => Some((key.as_str(), "false")),
            _ => None,
        })
        .collect()
}

/// Every set of name-fields whose union holds exactly `value` ids — the
/// hypotheses "this number counts THESE places in the answer".
///
/// A union only grows, so a set already holding too many is pruned with
/// everything under it, a set that could not reach the value even by taking
/// every field left is pruned with everything after it, and a set holding
/// exactly enough is not deepened. Which places it composes, and in what order,
/// is [`spelled`].
///
/// PROPOSING IS NOT DECIDING. What one answer can express is bounded by what it
/// holds — a frame whose view is empty cannot propose "the facts you list",
/// because it lists none — so these are hypotheses that every answer then gets
/// to refute ([`Accounts::put`]), never a verdict from the answer they came
/// from.
///
/// `budget` is the node count this search may spend, decremented as it goes. A
/// search that exhausts it is REPORTED rather than silently truncated — the
/// hypotheses it did not reach are hypotheses that cannot exonerate a number.
fn accountings(scope: &Scope, value: usize, budget: &mut usize) -> BTreeSet<Accounting> {
    let fields = spelled(scope);
    // The most the fields from here on could add between them.
    let mut reach: Vec<usize> = vec![0; fields.len() + 1];
    for index in (0..fields.len()).rev() {
        reach[index] = reach[index + 1] + fields[index].1.len();
    }
    let mut found = BTreeSet::new();
    let mut chosen: Accounting = Vec::new();
    search(
        &fields,
        &reach,
        0,
        &BTreeSet::new(),
        &mut chosen,
        value,
        budget,
        &mut found,
    );
    // AND THE SELECTIONS, EACH ONE WHOLE (Round 1076). A classified subset is
    // not a term the search may add to others — [`classes_matching`] is where
    // that bound is stated and what it cost to learn — so it enters here as a
    // one-place account, proposed like any other and refuted by any store whose
    // rows that class does not hold `value` of.
    found.extend(
        classes_matching(scope, value)
            .into_iter()
            .map(|place| vec![place.clone()]),
    );
    found
}

#[allow(clippy::too_many_arguments)]
fn search(
    fields: &[(&Place, &BTreeSet<&str>)],
    reach: &[usize],
    from: usize,
    union: &BTreeSet<&str>,
    chosen: &mut Accounting,
    value: usize,
    budget: &mut usize,
    found: &mut BTreeSet<Accounting>,
) {
    for index in from..fields.len() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        if union.len() + reach[index] < value {
            // Nothing left can reach it, here or at any later field.
            return;
        }
        let (field, ids) = fields[index];
        let mut next = union.clone();
        next.extend(ids.iter().copied());
        if next.len() > value {
            continue;
        }
        chosen.push(field.clone());
        if next.len() == value {
            found.insert(chosen.clone());
        } else {
            search(
                fields,
                reach,
                index + 1,
                &next,
                chosen,
                value,
                budget,
                found,
            );
        }
        chosen.pop();
    }
}

/// How many ids one accounting's fields name between them, here. A field this
/// answer does not carry contributes nothing rather than disqualifying the
/// account: an empty list and an absent one say the same thing to a reader.
fn accounted(scope: &Scope, of: &Accounting) -> usize {
    let mut union: BTreeSet<&str> = BTreeSet::new();
    for place in of {
        if let Some(ids) = scope.get(place) {
            union.extend(ids.iter().copied());
        }
    }
    union.len()
}

/// The last record's scope, kept because [`Wrote::numbers`] comes in record
/// order: every number of one record is judged before the next record's, and
/// gathering the scope of a root record means walking every name in the answer.
#[derive(Default)]
struct Memo<'a> {
    at: Option<(String, Scope<'a>)>,
}

impl<'a> Memo<'a> {
    fn of(&mut self, wrote: &'a Wrote, record: &str) -> &Scope<'a> {
        if self.at.as_ref().is_none_or(|(was, _)| was != record) {
            self.at = Some((record.to_string(), scope_of(wrote, record)));
        }
        &self.at.as_ref().expect("just filled").1
    }
}

/// What was proposed for one number by the unedited store, before any answer
/// had a chance to refute it.
///
/// "NOTHING THE ANSWER NAMES" IS ONE SENTENCE FOR TWO FINDINGS, and Round 1076
/// spent a whole run unable to tell them apart on the two numbers it was built
/// to reach. Either the walk proposed the right account and some store refuted
/// it — a fact about the READ, and the defect this file exists to find — or the
/// walk never proposed it, which is a limit of THIS SEARCH and says nothing
/// about the read at all. These four say which.
#[derive(Default)]
struct Proposed {
    /// How many accounts, over every question this verb was asked.
    accounts: usize,
    /// How many of them named a class — the places Round 1076 added.
    naming_a_class: usize,
    /// The smallest of them, spelled.
    smallest: String,
    /// What the number actually SAID in the unedited store. Zero is the value
    /// that most often explains an unproposed class: a class with no rows is a
    /// place the answer does not hold, and an account can be composed only of
    /// places it does (the rule [`accountings`] already states for the frame
    /// whose view is empty).
    said: BTreeSet<usize>,
}

/// What this walk still believes each number counts.
#[derive(Default)]
struct Accounts {
    /// (verb, field) -> the hypotheses no answer has refuted.
    alive: BTreeMap<(String, String), BTreeSet<Accounting>>,
    /// (verb, field) -> the answer that refuted the last one.
    died: BTreeMap<(String, String), String>,
    /// (verb, field) -> what was PROPOSED for it before any answer refuted it.
    proposed: BTreeMap<(String, String), Proposed>,
    /// Numbers this law does not judge: a fraction is not a count of anything
    /// nameable, and a field carrying several numbers in one record is a list of
    /// numbers rather than a count.
    fractional: usize,
    exhausted: BTreeSet<String>,
    capped: BTreeSet<String>,
    /// Places no unedited answer carries, so nothing ever proposed an account
    /// for them.
    late: BTreeSet<String>,
}

impl Accounts {
    /// Propose what each number in this answer could be counting.
    fn propose(&mut self, verb: &str, wrote: &Wrote) {
        let mut scope = Memo::default();
        for (field, record, value) in counts_of(wrote).0 {
            let mut budget = SEARCH_BUDGET;
            let found = accountings(scope.of(wrote, record), value, &mut budget);
            if budget == 0 {
                self.exhausted.insert(format!("{verb} {field}"));
            }
            if found.len() > HYPOTHESIS_CAP {
                self.capped
                    .insert(format!("{verb} {field} ({} proposed)", found.len()));
            }
            let key = (verb.to_string(), field.to_string());
            let noted = self.proposed.entry(key.clone()).or_default();
            noted.accounts += found.len();
            noted.naming_a_class += found
                .iter()
                .filter(|account| account.iter().any(Place::is_classified))
                .count();
            noted.said.insert(value);
            if noted.smallest.is_empty() {
                if let Some(one) = found.iter().min_by_key(|a| (a.len(), (*a).clone())) {
                    noted.smallest = spell(one);
                }
            }
            self.alive
                .entry(key)
                .or_default()
                .extend(found.into_iter().take(HYPOTHESIS_CAP));
        }
    }

    /// Put every hypothesis to this answer. One that cannot explain a number
    /// here is refuted: the read said `n`, and the places it was proposed to be
    /// counting hold something else.
    fn put(&mut self, verb: &str, wrote: &Wrote, source: &str) {
        let (counts, fractions) = counts_of(wrote);
        self.fractional += fractions;
        let mut scope = Memo::default();
        for (field, record, value) in counts {
            let key = (verb.to_string(), field.to_string());
            if !self.alive.contains_key(&key) {
                // A PLACE ONLY AN EDIT PRODUCES. Nothing proposed an account
                // for it, so it is named and counted rather than passed over —
                // an account first proposed here was refuted by the stores
                // after it and by none of the ones before.
                self.late.insert(format!("{verb} {field}"));
                self.alive.entry(key.clone()).or_default();
            }
            let Some(alive) = self.alive.get_mut(&key) else {
                continue;
            };
            if alive.is_empty() {
                continue;
            }
            let scope = scope.of(wrote, record);
            alive.retain(|hypothesis| accounted(scope, hypothesis) == value);
            if alive.is_empty() {
                // The FIRST answer that left the number unaccounted for: every
                // one after it refutes nothing.
                self.died
                    .entry(key)
                    .or_insert_with(|| format!("{source}, at `{record}`"));
            }
        }
    }
}

/// What one answer NAMES, keyed so a bucket costs two words instead of a report.
type Fingerprint = (u64, u64);

/// The numbers one answer carried, by the field they sit at.
type Numbers = BTreeMap<String, Vec<String>>;

/// Per read-and-question: what an answer named -> the numbers one such answer
/// carried, and the edit that produced it. Two answers in one bucket name
/// identical things, so a number that differs between them is not a function of
/// the names.
type ByNames = BTreeMap<String, BTreeMap<Fingerprint, (Numbers, String)>>;

/// One hypothesis about a number: the places it counts between them.
type Accounting = Vec<Place>;

/// The places one record and everything under it names ids, borrowed from the
/// answer's own walk.
type Scope<'a> = BTreeMap<Place, BTreeSet<&'a str>>;

/// One place in an answer whose names a number could be counting.
///
/// A PLACE IS NOT ALWAYS A FIELD (Round 1076). An account is composed as a
/// union, and until this round the only things it could union were fields the
/// answer spells — so a number counting the rows of ONE CLASS of a
/// classification had no account, however plainly the answer named both the
/// rows and their classes. `report-spec-map` names every section and every
/// section's `coverage_class`, and its `by_class` totals sat in
/// [`NOT_A_COUNT_OF_NAMES`] as a limit of THIS SEARCH rather than of that read.
///
/// A TYPE RATHER THAN A FORMATTED KEY, because the third component is authored
/// content: a classifier is any scalar field of a record, so its value can be a
/// `claim` or a `title` holding whatever words this walk would print around it.
/// Two different triples that printed alike would be ONE key in a [`Scope`],
/// each silently taking the other's ids, and every account naming that place
/// would afterwards be judged against names that are not the ones it names.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Place {
    /// A field the answer spells, addressed as [`common::wrote_about`] does.
    Spelled(String),
    /// The ids `field` names in the records where `key` holds `value`. The
    /// answer spells no list of these, and spells everything one is made of.
    Classified {
        field: String,
        key: String,
        value: String,
    },
}

impl Place {
    /// Whether this place is one the answer does not spell — what makes an
    /// account depend on Round 1076 rather than merely survive it.
    fn is_classified(&self) -> bool {
        matches!(self, Place::Classified { .. })
    }
}

impl std::fmt::Display for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Place::Spelled(field) => write!(f, "{field}"),
            Place::Classified { field, key, value } => write!(f, "{field} where {key}={value}"),
        }
    }
}

/// One accounting as a reader sees it: the places it counts, joined.
fn spell(of: &Accounting) -> String {
    of.iter()
        .map(Place::to_string)
        .collect::<Vec<_>>()
        .join(" + ")
}

/// What an answer NAMES, as a fixed-width key — two independent 64-bit hashes of
/// the number-blanked answer, so answers that name identical things land in one
/// bucket without the walk holding 30 reads x 93 stores of report text in
/// memory (a measurement that runs the machine out of memory is not a
/// measurement).
fn fingerprint_of(named: &serde_json::Value) -> Fingerprint {
    use std::hash::{Hash, Hasher};
    let text = named.to_string();
    let hash = |salt: &str| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        salt.hash(&mut hasher);
        text.hash(&mut hasher);
        hasher.finish()
    };
    (hash("names"), hash("shape"))
}

/// The answer with every number blanked — everything it NAMES, and the shape it
/// named it in. Two answers equal here differ in numbers alone.
fn named(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(_) => serde_json::Value::Null,
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(named).collect())
        }
        serde_json::Value::Object(map) => {
            serde_json::Value::Object(map.iter().map(|(k, v)| (k.clone(), named(v))).collect())
        }
        other => other.clone(),
    }
}

/// The fields this walk reports as counting what their read does not name, as
/// `verb path`. A list, not a total: an exclusion nobody can read is an
/// exclusion nobody removes (the R1029 rule).
///
/// EMPTY IS THE SHIPPED STATE, and it is a gate rather than a green tick: the
/// walk asks every advertised read, so a read that grows a lossy count is caught
/// the run it ships. What the first sweep found, and this round fixed:
///
/// - `report-authoring-frontier branch_owned_density.*.owned_facts` and the
///   `density` derived from it — moving one fact to another world-line moved
///   both and nothing else in the whole report. Rounds 1052 and 1053 recorded
///   this from the other side as a PAIR's limit ("`owned_facts` is what this
///   pair cannot see"); it was never a limit of a pair.
/// - `report-payoff-coverage worlds.*.exempt` — the one class of a four-way
///   classification whose members no field carried. Its own doc comment said
///   so out loud, in the words "counted, not listed".
///
/// And what the all-pairs law found in Round 1056, also fixed here:
///
/// - `report-payoff-substantiation setups_total` — every fact marked `expected`
///   store-wide, against a body that classifies only the CREDITED ones. The
///   setups nobody paid were counted and never named. No pair with the unedited
///   store shows it; two edited stores that name the same things do.
const COUNTED_WITHOUT_NAMING: [&str; 0] = [];

/// The reads that REFUSE a store this population can author, rather than
/// answering about it (Round 1061).
///
/// A read that stops answering has reacted more loudly than any number could,
/// and until this round the walk filed that reaction as "an answer I could not
/// compare" — the same bucket as a read that answers in prose. The distinction
/// only became load-bearing when the population reached the canon order, where
/// there is no import to refuse anything and the READS are the whole gate.
///
/// `report-authoring-frontier` and `report-quest-graph` reject the five
/// predicate retargets that make a quest claim malformed (the R676 role-conflict
/// guard, which rejects the read rather than judging content).
/// `validate-continuity` rejects far more, and that is what a gate is for.
const READS_THAT_REJECT: [&str; 4] = [
    "report-authoring-frontier",
    "report-quest-graph",
    "validate-continuity",
    // Round 1072 — the fidelity gate joined the panel and immediately joined
    // this list: a road edit that hands a scene to a sibling world puts the
    // frozen prose off-path, and the gate exits non-zero rather than reporting.
    "validate-render-fidelity",
];

/// How many hypotheses one search may visit, and how many it may keep. Both are
/// printed when they bite, because a search that stopped early is a search whose
/// silence means nothing (the R1029 rule, applied to a walk's own budget).
const SEARCH_BUDGET: usize = 20_000_000;
const HYPOTHESIS_CAP: usize = 100_000;

/// The numbers no set of names in their own answer accounts for, as
/// `verb field`. A count here says: the read prints this number, and a consumer
/// holding the answer cannot say which things it counted.
///
/// THIS IS THE STATIC HALF (Round 1057), and it reaches what the differential
/// census above structurally cannot. That one compares answers, so it can only
/// speak about a number that MOVES; Round 1056 filed the rest as a real limit —
/// "a number that is lossy but constant across this population is invisible to
/// it". Asking each answer whether its own body accounts for its own numbers is
/// a question about ONE answer, so a number nothing ever moves is judged like
/// any other, and the 93 stores the sweep builds are 93 trials of the account
/// rather than 92 comparisons.
///
/// A number here is NOT automatically a defect, and the four on the shipped
/// surface are two shapes, neither of which counts anything:
///
/// - AN ORDINAL is a POSITION. A locator says which scene discloses a fact and
///   then where that scene sits on the road, so the thing it is about is named
///   right beside it and the number is not the read's account of anything.
///   `report-playable-world` carries the road it indexes into, and
///   [`the_locator_ordinal_is_where_the_answer_puts_that_scene`] holds the
///   number to it. `report-quest-graph` carries the same field and NOT the road,
///   so its oracle is a SECOND SHIPPED READ (Round 1058) —
///   [`the_quest_locator_ordinal_is_where_the_manuscript_puts_that_scene`].
/// - A CHAIN COUNT is a function of names the same answer carries in EARLIER
///   records. The facts holding at scene N are the ones that began at or before
///   N and have not ended, both of them named events of the same answer, which
///   is what refuted Round 1054's proposed record scoping.
///   [`the_manuscript_count_is_derivable_from_the_events_it_names`] replays it,
///   over both wires that carry a manuscript.
///
/// The one number the law does not reach at all is the frontier's `density`,
/// which is a FRACTION: [`the_density_is_the_facts_it_names_over_the_road_it_names`]
/// is what that one is, and where the loss in it actually sits.
///
/// TWO SPEC-MAP ENTRIES USED TO BE ON THIS LIST and neither was about the read.
/// They arrived in Round 1061 with the scene registry and Round 1057 filed them
/// as a limit of THIS SEARCH: `report-spec-map` names every section AND its
/// `coverage_class`, so `by_class.normative_gap` is exactly the rows whose class
/// is that one — but an account was composed as a UNION of named lists, and a
/// subset selected by a field's value is not a union of anything. Round 1076
/// gave a [`Place`] the second shape, and which numbers NEED it is asserted
/// rather than implied ([`ACCOUNTED_FOR_ONLY_BY_A_CLASS`]) — a wider vocabulary
/// makes leaving this list strictly easier, so the shrink alone would be no
/// evidence at all.
///
/// - A CLASS WITH NO ROWS IS NOT A PLACE, which is why one of the two is still
///   here. Measured rather than argued: the unedited store says
///   `by_class.normative_gap` is 0, and it says so because no section carries
///   that class — so there is nothing in that answer for a [`Place::Classified`]
///   to be built out of, and an account can be composed only of places the
///   answer holds. That is the rule [`accountings`] already states for the frame
///   whose view is empty, met a second time from the other side. The vocabulary
///   that would supply the missing class is either the read's own (`by_class`'s
///   keys are `coverage_class` values, which is knowledge of THIS report and not
///   a derivation) or every string in the answer, which would hand each of the
///   many zeroes on this surface a pile of accounts that are true because
///   nothing is in them. Both are worse than the printed limit.
const NOT_A_COUNT_OF_NAMES: [&str; 5] = [
    "report-playable-world worlds.*.locators[].scene_ordinal",
    "report-playable-world worlds.*.manuscript.scenes[].holding_count",
    "report-playthrough-manuscript worlds.*.scenes[].holding_count",
    "report-quest-graph quests[].locators[].scene_ordinal",
    "report-spec-map summary.by_class.normative_gap",
];

/// The numbers whose ONLY surviving account names a place the answer does not
/// spell, as `verb field = the account`. (Round 1076.)
///
/// THIS IS WHAT THE WIDER VOCABULARY BOUGHT, and stating it is the difference
/// between buying something and moving a name off a list. A number leaves
/// [`NOT_A_COUNT_OF_NAMES`] the moment ANY hypothesis survives every store, and
/// widening what a hypothesis may say makes that strictly easier — so a round
/// that only checked the census shrink could have excused these two numbers with
/// a coincidence and read the same from here. A field is on THIS list when every
/// account that survived names a [`Place::Classified`]: no union of the fields
/// the answer spells reaches the value at all, in any of the 93 stores.
///
/// The one member is `report-spec-map`, and the account is the read's own words:
/// it names every section AND that section's `coverage_class`, so the total for
/// one class is exactly the sections carrying it. Round 1057 filed it as a limit
/// of THIS SEARCH rather than of the read — "an account here is composed as a
/// UNION of named lists, and a subset selected by a field's value is not a union
/// of anything" — which is a sentence describing work, and this is that work.
const ACCOUNTED_FOR_ONLY_BY_A_CLASS: [&str; 1] = [
    "report-spec-map summary.by_class.informative_exempt = sections[].section_id where \
      coverage_class=informative_exempt",
];

#[test]
fn the_reads_that_count_what_they_do_not_name() {
    // ONE SWEEP, THREE LAWS (Round 1071): the corpus, the panel and every
    // trial are built once for this binary and read here.
    let sweep = sweep::sweep();
    let ids = registered_ids(&sweep.store);
    let panel = &sweep.panel;
    let unaskable = &sweep.unaskable;
    let baseline = &sweep.baseline;
    let verb_of: BTreeMap<String, String> = panel
        .iter()
        .map(|read| (read.label(), read.verb.clone()))
        .collect();

    // (verb, field) -> the edits under which that number moved ALONE. Keyed by
    // the READ, so a verb asked several questions is judged by all of them: a
    // number is named if SOME question the corpus can ask names it.
    let mut alone: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    // The same, for the numbers that moved WITH something named — the measure of
    // how much of the surface the law could have flagged and did not.
    let mut accompanied: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    // (verb, edit) pairs where the answer did not move at all. A read blind to an
    // edit is out of this walk's reach, and the number says how far out.
    let mut still = 0usize;
    let mut prose_comparisons = 0usize;
    // label -> what an answer NAMES -> the numbers one such answer carried, and
    // the edit that produced it. Two answers in one bucket name identical things,
    // so a number that differs between them is not a function of the names.
    let mut by_names: ByNames = BTreeMap::new();

    // WHAT EACH NUMBER COUNTS — the static half (Round 1057). The census above
    // compares answers, so it can speak only about a number an edit MOVES, and
    // its verdict on one that moves beside a name is `accompanied`: something
    // named moved too. That is not the same as the number being a FUNCTION of
    // the names, which is what a consumer needs in order to act on it. So each
    // answer is also asked, on its own, WHICH of the things it names the number
    // counts — a question about one answer, so all 93 stores are trials of it.
    let mut accounts = Accounts::default();

    // The unedited store is one of the answers the law compares, not a
    // privileged one: it goes into the same buckets, so "the baseline and this
    // edit name the same things and disagree about a number" is the same finding
    // as any other pair.
    for (label, answer) in &baseline.answers {
        if let Answer::Json(json) = answer {
            let wrote = wrote_about(json, &ids);
            by_names.entry(label.clone()).or_default().insert(
                fingerprint_of(&named(json)),
                (numbers_of(&wrote), "(unedited)".to_string()),
            );
            accounts.propose(&verb_of[label], &wrote);
        }
    }
    // PROPOSED BY ALL, THEN PUT TO ALL. A verb asked at several questions holds
    // one account across them, and an answer that could not propose it — a frame
    // whose view is empty lists nothing — must still be allowed to refute it.
    for (label, answer) in &baseline.answers {
        if let Answer::Json(json) = answer {
            accounts.put(
                &verb_of[label],
                &wrote_about(json, &ids),
                "the unedited store",
            );
        }
    }

    let mut applied = 0usize;
    let mut refused = 0usize;
    // The edits no read on the panel answered differently for. A member of the
    // population that moves NOTHING is not evidence of a quiet surface — it is a
    // corruption the store never received, which is what a manifest field the
    // import ignores would look like from here. Named rather than counted.
    let mut moved_nothing: BTreeSet<String> = BTreeSet::new();
    // Reads that stopped answering at all under an edit, by read and by edit.
    // `order.json` has no import to refuse a bad road, so the refusal surfaces
    // HERE, and a walk that let it fall in with the answers it could not compare
    // would read a store it could not ask as a store with nothing to say.
    let mut read_failures: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for trial in &sweep.trials {
        let Ok(seen) = &trial.seen else {
            // The write path refuses it: not a move an author could make.
            refused += 1;
            continue;
        };
        applied += 1;
        let edit = trial.corruption.label();
        // Parsed for this iteration and dropped with it — the sweep holds the
        // bytes, never 312 parsed panels.
        let seen = seen.parsed();
        // A read that answered at baseline and refuses this store MOVED — from
        // an answer to a refusal. Counting it as an answer this walk could not
        // compare would file the loudest reaction on the panel as silence.
        let mut moved_something = !seen.failed.is_empty();
        for label in &seen.failed {
            read_failures
                .entry(verb_of[label].clone())
                .or_default()
                .insert(edit.clone());
        }
        for (label, before) in &baseline.answers {
            let verb = verb_of[label].clone();
            let (Answer::Json(before), Some(Answer::Json(after))) =
                (before, seen.answers.get(label))
            else {
                // A read that answers `--json` in prose holds no fields to key
                // on. Counted rather than curated away (the R1029 rule) — but
                // its TEXT still says whether the edit reached it, which is a
                // different question from whether the law can key on it.
                prose_comparisons += 1;
                if let (Answer::Prose(was), Some(Answer::Prose(now))) =
                    (before, seen.answers.get(label))
                {
                    moved_something |= was != now;
                }
                continue;
            };
            let (was_wrote, now_wrote) = (wrote_about(before, &ids), wrote_about(after, &ids));
            // EVERY STORE IS A TRIAL OF THE ACCOUNT.
            accounts.put(&verb, &now_wrote, &edit);
            let (was, now) = (numbers_of(&was_wrote), numbers_of(&now_wrote));
            let moved: BTreeSet<String> = was
                .keys()
                .chain(now.keys())
                .filter(|path| was.get(*path) != now.get(*path))
                .cloned()
                .collect();
            // OVER ALL PAIRS, NOT ONLY AGAINST THE BASELINE (Round 1056). A
            // number is a read's whole account of something exactly when it is
            // NOT A FUNCTION of what the answer names: two answers that name
            // identical things and disagree about the number prove the number
            // carries information nothing named carries. Comparing each answer
            // to the baseline asks that of 92 pairs; keying by what the answer
            // NAMES asks it of every pair the sweep produces, which is the
            // narrowing Round 1054 filed and reaches the case it named — an edit
            // that moves a name for an unrelated reason no longer excuses the
            // number, because another edit lands on the same names.
            let fingerprint = fingerprint_of(&named(after));
            match by_names
                .entry(label.clone())
                .or_default()
                .entry(fingerprint)
            {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert((now.clone(), edit.clone()));
                }
                std::collections::btree_map::Entry::Occupied(slot) => {
                    let (theirs, other) = slot.get();
                    for path in theirs.keys().chain(now.keys()) {
                        if theirs.get(path) != now.get(path) {
                            alone
                                .entry((verb.clone(), path.clone()))
                                .or_default()
                                .insert(format!("{edit} against {other}, same names"));
                        }
                    }
                }
            }
            if moved.is_empty() {
                if named(before) == named(after) {
                    still += 1;
                } else {
                    moved_something = true;
                }
                continue;
            }
            moved_something = true;
            // THE COVERAGE MEASURE: the numbers one edit moved while moving
            // something named too. Not evidence of loss — evidence of how much
            // of the surface a single pair cannot speak about, which is why the
            // fingerprint above asks all of them.
            if named(before) != named(after) {
                for path in moved {
                    accompanied
                        .entry((verb.clone(), path))
                        .or_default()
                        .insert(edit.clone());
                }
            }
        }
        if !moved_something {
            moved_nothing.insert(edit.clone());
        }
    }

    // Print BEFORE asserting (the R1026 lesson): the distribution is the finding,
    // and a first-violation stop would report one line of it.
    println!(
        "{} reads over {} questions, {} UNASKABLE; {} corruptions applied, {} refused by the \
         write path; {prose_comparisons} comparisons against a prose answer; {still} (read, edit) \
         pairs the read did not move for at all\n",
        verb_of.values().collect::<BTreeSet<_>>().len(),
        panel.len(),
        unaskable.len(),
        applied,
        refused,
    );
    for (verb, reason) in unaskable {
        println!("  UNASKABLE {verb}: {reason}");
    }
    // THE SWEEP ASSERTS ITS OWN REACH (the R1054 rule: an empty answer is the
    // shape of a clean surface AND of a surface nothing touched). These two say
    // the edits arrived and the reads stayed askable.
    println!(
        "\n{} edit(s) moved no answer on the panel; {} read(s) stopped answering under some edit:",
        moved_nothing.len(),
        read_failures.len()
    );
    for (verb, edits) in &read_failures {
        println!("  {verb} could not answer under {} edit(s)", edits.len());
        for edit in edits.iter().take(3) {
            println!("        {edit}");
        }
    }
    for edit in &moved_nothing {
        println!("  MOVED NOTHING {edit}");
    }
    println!("\nCOUNTED WITHOUT NAMING — the number moved and nothing named did:");
    for ((verb, path), edits) in &alone {
        println!("  {verb} {path}");
        for edit in edits {
            println!("        {edit}");
        }
    }
    println!(
        "\nnumbers that moved WITH something named ({} fields) — the law's negative answers:",
        accompanied.len(),
    );
    for ((verb, path), edits) in &accompanied {
        println!("  {:4} edits  {verb} {path}", edits.len());
    }

    // THE POPULATION MOVES THESE NUMBERS, and only there does an account get
    // TESTED: a number no authorable edit ever moves keeps whatever account the
    // unedited store proposed, and nothing has put that account to a second
    // store. Derived from this walk's own two censuses rather than named here.
    let moved_at_all: BTreeSet<&(String, String)> =
        alone.keys().chain(accompanied.keys()).collect();
    let (tested, untested): (Vec<_>, Vec<_>) = accounts
        .alive
        .iter()
        .partition(|(key, _)| moved_at_all.contains(key));
    let unaccounted: Vec<&(String, String)> = tested
        .iter()
        .filter(|(_, alive)| alive.is_empty())
        .map(|(key, _)| *key)
        .collect();
    println!(
        "\nWHAT EACH NUMBER COUNTS — {} fields on the surface; {} of them an authorable edit \
         moves, and {} of those are accounted for by names in their own answer. {} field(s) no \
         edit moves, so their account is proposed and never tested. {} value(s) not judged (a \
         fraction, or several numbers at one field of one record); {} search(es) out of budget; \
         {} field(s) with more hypotheses than kept; {} place(s) no unedited answer carries:",
        accounts.alive.len(),
        tested.len(),
        tested.len() - unaccounted.len(),
        untested.len(),
        accounts.fractional,
        accounts.exhausted.len(),
        accounts.capped.len(),
        accounts.late.len(),
    );
    for line in accounts
        .exhausted
        .iter()
        .chain(accounts.capped.iter())
        .chain(accounts.late.iter())
    {
        println!("  OUT OF REACH {line}");
    }
    let smallest = |alive: &BTreeSet<Accounting>| {
        alive
            .iter()
            .min_by_key(|hypothesis| (hypothesis.len(), (*hypothesis).clone()))
            .map(spell)
    };
    let account = |alive: &BTreeSet<Accounting>| match smallest(alive) {
        Some(spelled) => format!(
            "|{spelled}|{}",
            if alive.len() > 1 {
                format!("   ({} accounts survive)", alive.len())
            } else {
                String::new()
            },
        ),
        None => "NOTHING THE ANSWER NAMES".to_string(),
    };
    for ((verb, field), alive) in &tested {
        println!("  {verb} {field} = {}", account(alive));
        if alive.is_empty() {
            let key = (verb.clone(), field.clone());
            let none = Proposed::default();
            let it = accounts.proposed.get(&key).unwrap_or(&none);
            let said: Vec<String> = it.said.iter().take(4).map(usize::to_string).collect();
            println!(
                "        the unedited store said {}{}; {} account(s) proposed, {} of them naming \
                 a class{}; last died at {}",
                said.join("/"),
                if it.said.len() > 4 { " ..." } else { "" },
                it.accounts,
                it.naming_a_class,
                if it.smallest.is_empty() {
                    String::new()
                } else {
                    format!(" (smallest |{}|)", it.smallest)
                },
                accounts
                    .died
                    .get(&key)
                    .map_or("the unedited store", String::as_str),
            );
        }
    }
    println!("\nproposed but never put to a second value — no edit in this population moves them:");
    for ((verb, field), alive) in &untested {
        println!("  {verb} {field} = {}", account(alive));
    }

    // WHICH ACCOUNTS THE ANSWER DOES NOT SPELL (Round 1076). A number is here
    // when EVERY account that survived every store names a classified subset —
    // so it is not that a class happened to be among the ways to reach the
    // value, it is that a union of spelled fields cannot reach it at all. That
    // is the non-vacuity of this round: without these lines, teaching the
    // search a wider vocabulary and teaching it nothing look the same from
    // here, and both print an empty census.
    let by_a_class: Vec<String> = tested
        .iter()
        .filter(|(_, alive)| {
            !alive.is_empty()
                && alive
                    .iter()
                    .all(|hypothesis| hypothesis.iter().any(Place::is_classified))
        })
        .map(|((verb, field), alive)| {
            format!(
                "{verb} {field} = {}",
                smallest(alive).expect("a surviving account"),
            )
        })
        .collect();
    println!(
        "\nACCOUNTED FOR ONLY BY A CLASS — no union of fields the answer spells reaches these:"
    );
    for line in &by_a_class {
        println!("  {line}");
    }

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        (
            verb_of.values().collect::<BTreeSet<_>>().len(),
            panel.len(),
            applied,
            refused,
        ) == (32, 79, 256, 56),
        "INPUTS: both populations come from `common`, so this walk cannot narrow \
         either to suit itself — 32 reads over 79 questions, and 312 authorable \
         corruptions of which the write path refuses 56. It was 41 until Round \
         1054 and 92 until Round 1061, and both jumps were the same discovery \
         twice: the derivation said it took the legs a thing ACTUALLY carries \
         and was reading a smaller thing than it claimed. First it carried only \
         the legs saying what a fact CLAIMS, never where it sits; then it took \
         the FACT manifest for the corpus, when an author writes three. The \
         refusals are one leg — cutting a scene out of the registry that facts \
         still stand at",
    );
    check(
        alone
            .keys()
            .map(|(verb, path)| format!("{verb} {path}"))
            .collect::<Vec<_>>()
            == COUNTED_WITHOUT_NAMING,
        "THE CENSUS: every field of every shipped read that an authorable edit \
         moves ON ITS OWN, named. Round 1053 met this shape on the frontier's \
         scene census and Round 1050 on `report-frame-view`'s `not_holding`; \
         both were written down where they were met, and neither asked the \
         surface",
    );
    check(
        accompanied
            .keys()
            .any(|(verb, path)| verb == "report-frame-view" && path == "holding_count"),
        "NON-VACUITY: the law says NO as well as yes, and it says no to the \
         canonical shape it must not flag — `holding_count` rides beside the \
         entries it counts (R435) and moves only when they do, which no rule \
         about a field's name could tell apart from the `not_holding` this round \
         had to change",
    );
    check(
        (still, prose_comparisons, accompanied.len()) == (17735, 614, 17),
        "REACH, ASSERTED RATHER THAN IMPLIED: of the 20224 (question, edit) pairs \
         this walk makes, 17735 move the read not at all and 614 are against the \
         one verb that takes `--json` and answers in prose, which holds no fields \
         to key on. The law can speak only about the rest, and it finds numbers \
         moving in 17 fields. An empty census is what a clean surface looks like \
         AND what a walk that stopped reaching anything looks like; these three \
         numbers are what tell them apart. It read 10 fields from Round 1056 to \
         Round 1060 over a population of 92, all of them fact edits. Round 1061 \
         widened that population to the two manifests an author writes beside \
         the facts, and the seven fields that arrived are what a scene registry \
         and a canon order move: the coverage class split, the spec map's class \
         totals, and the road under the frontier's density",
    );
    check(
        accompanied.keys().any(|(verb, path)| {
            verb == "report-authoring-frontier" && path == "branch_owned_density.*.density"
        }),
        "NON-VACUITY, THE FIX: the density is still a number and edits still move \
         it — it left the census because the `owned` list beside it now moves \
         too. Without this line an empty census would also be what a read that \
         stopped answering looks like",
    );
    check(
        unaccounted
            .iter()
            .map(|(verb, field)| format!("{verb} {field}"))
            .collect::<Vec<_>>()
            == NOT_A_COUNT_OF_NAMES,
        "WHAT EACH NUMBER COUNTS: for every number an authorable edit moves, the \
         things in the same answer it holds as many of. `accompanied` says only \
         that something named moved alongside — the upgrade Round 1056 filed as \
         undone for ten of its eleven fields, since a number that moves beside a \
         name can still carry what no name carries",
    );
    check(
        by_a_class == ACCOUNTED_FOR_ONLY_BY_A_CLASS,
        "WHAT THE CLASSES BOUGHT: the numbers a union of spelled fields cannot \
         reach at all, and the account that does reach them. Widening what a \
         hypothesis may SAY makes leaving the census above strictly easier, so \
         the shrink is not evidence on its own — a coincidence in a bigger \
         search space reads exactly the same. This list is the evidence, and it \
         is also the guard the other way: a number that stops needing a class \
         has had its read start naming the subset, and one that starts needing \
         a class is a read that stopped",
    );
    check(
        moved_nothing.is_empty(),
        "EVERY CORRUPTION IS EVIDENCE: no edit in the population leaves the whole \
         panel saying exactly what it said before. One that did would be a store \
         the walk built, asked thirty reads about, and learned nothing from — \
         and the thing it would look like from here is a clean surface. Round \
         1061 met 110 such edits the first time this population reached the \
         canon order, and every one of them was a road no reader could travel",
    );
    check(
        read_failures.keys().cloned().collect::<Vec<_>>() == READS_THAT_REJECT,
        "AND WHICH READS REFUSE, rather than answer: a read that stops answering \
         under an edit has reacted more loudly than any number could, and the \
         walk files it as a refusal instead of as an answer it could not \
         compare. Four of the thirty-two do it; the rest hand back an answer about \
         a store an author should not have shipped",
    );
    check(
        accounts.exhausted.is_empty() && accounts.capped.is_empty() && accounts.late.is_empty(),
        "THE SEARCH REACHED ITS ANSWERS: no hypothesis search ran out of budget, \
         none proposed more accounts than the walk kept, and every place a \
         number sits was carried by an answer that could propose an account for \
         it. Any of the three would make an `accounted for` verdict a statement \
         about what the walk had time to try",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the census of numbers that name nothing no longer holds"
    );
}

/// The sections, order and facts of a store where two facts of two frames can
/// trade whose belief they are: one frame holds a standing fact and a lapsed
/// one, the other holds a lapsed one, and at the end of the road each frame's
/// view therefore reports exactly one fact as definitively not holding.
fn frame_trade_manifests() -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    let section = |id: &str, title: &str| serde_json::json!({"section_id": id, "parent_doc": "constructed", "title": title});
    let sections = serde_json::json!([
        section("n-01", "the watch begins"),
        section("n-02", "the watch is relieved"),
        section("n-03", "the road goes on"),
    ]);
    let order = serde_json::json!({"edges": [["n-01", "n-02"], ["n-02", "n-03"]]});
    let fact = |id: &str, frame: &str, to: Option<&str>, claim: &str| {
        let mut row = serde_json::json!({
            "fact_id": id,
            "frame": frame,
            "claim": claim,
            "canon_from": "n-01",
            "evidence": ["n-01"],
        });
        if let Some(to) = to {
            row["canon_to"] = serde_json::json!(to);
        }
        row
    };
    let facts = serde_json::json!({
        "frames": [{"frame_id": "f-watcher"}, {"frame_id": "f-rival"}],
        "facts": [
            fact("fx-standing", "f-watcher", None, "the gate is watched"),
            fact("fx-lapsed", "f-watcher", Some("n-02"), "the watcher is at the gate"),
            fact("fx-spent", "f-rival", Some("n-02"), "the rival waits in the lane"),
        ],
    });
    (sections, order, facts)
}

/// A frame's view must be able to say WHICH of its facts stopped holding — and
/// until this round it could not. (Round 1054.)
///
/// `report-frame-view` classifies a frame's facts three ways at a canon point:
/// `holding`, `unknown`, and `not_holding`. The first two shipped as lists and
/// the third as a count, which was recorded once as a limit and left. The
/// residual is not recoverable from the rest of the answer either — it is the
/// frame's population minus the other two, and the population appears nowhere in
/// the report.
///
/// So this is what that cost, measured the way Round 1053 measured the frontier's
/// census: two facts of two frames trade whose belief they are. Each frame still
/// holds one standing fact and one lapsed one, so every count is where it was;
/// with `not_holding` set aside the two reports are byte-equal, so that field is
/// the ONLY place in this read where the trade can ever appear. On the wire this
/// round replaced, the two stores were indistinguishable.
///
/// The census above cannot reach this, and the reason is worth keeping: the panel
/// asks each read at the END of the road (the R1050 rule — the point where the
/// store has all of its history behind it), and there every fact of a frame has
/// already become true, so no single-leg edit moves the residual alone. A walk
/// over the shipped surface is bounded by the questions the corpus can ask it.
#[test]
fn the_frame_view_names_the_facts_it_calls_not_holding() {
    let (sections, order, facts) = frame_trade_manifests();
    let mut traded = facts.clone();
    let mut moved = 0usize;
    for fact in traded["facts"].as_array_mut().expect("facts array") {
        let frame = match fact["fact_id"].as_str() {
            Some("fx-lapsed") => "f-rival",
            Some("fx-spent") => "f-watcher",
            _ => continue,
        };
        fact["frame"] = serde_json::json!(frame);
        moved += 1;
    }
    assert_eq!(moved, 2, "the trade moved {moved} facts, not the two named");

    let build = |manifest: &serde_json::Value| {
        common::constructed_corpus(&sections, &order, manifest)
            .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"))
    };
    let (before, after) = (build(&facts), build(&traded));
    let ask = |ws: &std::path::Path, argv: &[&str]| -> serde_json::Value {
        let out = common::run(ws, argv);
        assert!(
            out.status.success(),
            "{argv:?} on the constructed store: {}",
            String::from_utf8_lossy(&out.stderr),
        );
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{argv:?} is not json: {e}"))
    };
    let watcher = |ws: &std::path::Path| {
        ask(
            ws,
            &[
                "report-frame-view",
                "--frame",
                "f-watcher",
                "--at",
                "n-03",
                "--json",
            ],
        )
    };
    // The oracle is ANOTHER SHIPPED READ (the R1050 rule): the manuscript hands
    // a runtime each fact's frame verbatim, so it is what says the trade is an
    // edit a reader meets rather than a difference of spelling.
    let frames_of = |ws: &std::path::Path| -> BTreeMap<String, String> {
        let manuscript = ask(ws, &["report-playthrough-manuscript", "--json"]);
        let mut out = BTreeMap::new();
        for world in manuscript["worlds"].as_object().into_iter().flatten() {
            for scene in world.1["scenes"].as_array().into_iter().flatten() {
                for event in scene["begins"].as_array().into_iter().flatten() {
                    if let (Some(fact), Some(frame)) =
                        (event["fact_id"].as_str(), event["frame"].as_str())
                    {
                        out.insert(fact.to_string(), frame.to_string());
                    }
                }
            }
        }
        out
    };

    let (was, now) = (watcher(before.path()), watcher(after.path()));
    let residual = |view: &serde_json::Value| -> Vec<String> {
        view["not_holding"]
            .as_array()
            .expect("the residual is a list")
            .iter()
            .filter_map(|id| id.as_str().map(ToString::to_string))
            .collect()
    };
    let counts = |view: &serde_json::Value| -> (usize, usize, usize) {
        (
            view["holding"].as_array().map_or(0, Vec::len),
            residual(view).len(),
            view["unknown"].as_array().map_or(0, Vec::len),
        )
    };
    let without_residual = |view: &serde_json::Value| {
        let mut rest = view.clone();
        rest.as_object_mut()
            .expect("the frame view is an object")
            .remove("not_holding");
        rest
    };

    println!(
        "the trade moves `fx-lapsed` {:?} -> {:?} and `fx-spent` {:?} -> {:?}; f-watcher at \
         `n-03` reports holding/not_holding/unknown {:?} then {:?}, naming {:?} then {:?}",
        frames_of(before.path()).get("fx-lapsed"),
        frames_of(after.path()).get("fx-lapsed"),
        frames_of(before.path()).get("fx-spent"),
        frames_of(after.path()).get("fx-spent"),
        counts(&was),
        counts(&now),
        residual(&was),
        residual(&now),
    );

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    let (seen_before, seen_after) = (frames_of(before.path()), frames_of(after.path()));
    check(
        [
            seen_before.get("fx-lapsed").map(String::as_str),
            seen_after.get("fx-lapsed").map(String::as_str),
            seen_before.get("fx-spent").map(String::as_str),
            seen_after.get("fx-spent").map(String::as_str),
        ] == [
            Some("f-watcher"),
            Some("f-rival"),
            Some("f-rival"),
            Some("f-watcher"),
        ],
        "THE EDIT IS REAL: the manuscript hands the runtime each fact under the \
         other's frame after the trade, so this is a store difference a reader \
         meets and not a difference of spelling",
    );
    check(
        counts(&was) == counts(&now),
        "INVISIBLE TO COUNTS: the frame still holds one fact, still calls one \
         definitively not-holding and still leaves none undecided — the trade is \
         exactly the edit a residual of numbers cannot report",
    );
    check(
        without_residual(&was) == without_residual(&now),
        "AND TO EVERY OTHER FIELD: with the residual set aside the two views are \
         equal, so `not_holding` is the ONLY place in this read where the trade \
         could ever appear",
    );
    check(
        counts(&was) == (1, 1, 0),
        "NON-VACUITY: the frame really does hold one and refuse one at this \
         point. Without this the claims above hold for a view that classifies \
         nothing at all",
    );
    check(
        residual(&was) == ["fx-lapsed"] && residual(&now) == ["fx-spent"],
        "THE LAW: the residual MOVED, and it names the fact it is about — which \
         is what makes `not_holding` an answer a consumer can act on rather than \
         a number that something, somewhere, is no longer true",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the frame view does not name the facts it calls not-holding"
    );
}

/// The tellings this store declares — the argument the reads that project a
/// playable world require, read out of the store rather than named here.
fn tellings_of(store: &common::AuthoredStore) -> Vec<String> {
    AtomicStore::load(&store.ws.path().join(SIDECAR))
        .map(|loaded| common::declared_tellings(&loaded))
        .unwrap_or_default()
}

/// Every shipped wire that hands a reader one road's manuscript, as
/// `(what asked it, the per-road map)`.
///
/// There are two, and that is the point of asking through here: the playable
/// world embeds the same per-scene wire under `worlds.*.manuscript`, so a
/// derivation proved of `report-playthrough-manuscript` says nothing about the
/// answer a runtime actually holds.
fn manuscripts(store: &common::AuthoredStore) -> Vec<(String, serde_json::Value)> {
    let mut out = Vec::new();
    let ask = |argv: &[&str]| -> Option<serde_json::Value> {
        let done = common::run(store.ws.path(), argv);
        done.status
            .success()
            .then(|| serde_json::from_slice(&done.stdout).ok())
            .flatten()
    };
    if let Some(manuscript) = ask(&["report-playthrough-manuscript", "--json"]) {
        out.push((
            "report-playthrough-manuscript".to_string(),
            manuscript["worlds"].clone(),
        ));
    }
    for telling in tellings_of(store) {
        let Some(playable) = ask(&["report-playable-world", "--telling", &telling, "--json"])
        else {
            continue;
        };
        let embedded: serde_json::Map<String, serde_json::Value> = playable["worlds"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(world, road)| (world.clone(), road["manuscript"].clone()))
            .collect();
        out.push((
            "report-playable-world".to_string(),
            serde_json::Value::Object(embedded),
        ));
    }
    out
}

/// The manuscript's per-scene count IS derivable from the events the same answer
/// names — which is why Round 1056 does not scope the law above to the RECORD.
///
/// That scoping is what Round 1054 filed as this law's narrowing: a number
/// escapes the whole-answer test when the same edit moves a name somewhere else
/// for an unrelated reason, so ask whether anything named moved inside the
/// number's own record instead. It was implemented, swept, and the first thing
/// it flagged was this count — `worlds.*.scenes[section=*].holding_count`, under
/// every `branch` and `canon_from` retarget, 34 edits on two reads.
///
/// The flag is a FALSE POSITIVE, and the reason is the axis rather than the
/// field: a scene row does not name the facts holding at it, but the road's
/// EARLIER rows do. The count at scene N is the facts that began at or before N
/// and have not ended, and both halves are named events in the same answer. A
/// consumer reading one row learns only a number; a consumer reading the road
/// learns which facts, and a read's account of a subject is not bounded by one
/// record when the records form a chain.
///
/// So this test measures the derivation rather than asserting it, over every
/// authored corpus and every road. It also executes a claim that shipped with
/// the wire in Round 466 and had never been run: "the delta story and the holds
/// semantics cross-check each other — a delta reconstruction that disagrees with
/// the count has hit an unplaced coordinate, never a second semantics".
#[test]
fn the_manuscript_count_is_derivable_from_the_events_it_names() {
    let (stores, unloadable) = common::authored_stores();
    let mut roads = 0usize;
    let mut scenes = 0usize;
    let mut mismatches: Vec<String> = Vec::new();
    let mut touched_by_unplaced = 0usize;
    let mut wires: BTreeSet<String> = BTreeSet::new();
    for store in &stores {
        // BOTH SHIPPED WIRES THAT CARRY A MANUSCRIPT (Round 1057). Round 1056
        // replayed one of them, and the accounting law names the other in the
        // same breath: `report-playable-world` hands a runtime the same
        // per-scene count under `worlds.*.manuscript`, and a derivation proved
        // of one wire is not proved of the other — that is the shape Round 1054
        // met when a pin judged one read and the same property held for three.
        for (wire, worlds) in manuscripts(store) {
            wires.insert(wire.clone());
            for (world, road) in worlds.as_object().into_iter().flatten() {
                roads += 1;
                // The facts this road says it CANNOT place — the escape hatch the
                // wire's own claim names. A fact whose end coordinate is outside the
                // order has no `ends` event to replay.
                let unplaced: BTreeSet<&str> = ["unplaced_facts", "undecidable"]
                    .iter()
                    .flat_map(|field| road[*field].as_array().into_iter().flatten())
                    .filter_map(|row| row["fact_id"].as_str())
                    .collect();
                let mut holding: BTreeSet<&str> = BTreeSet::new();
                for scene in road["scenes"].as_array().into_iter().flatten() {
                    scenes += 1;
                    for event in scene["begins"].as_array().into_iter().flatten() {
                        if let Some(id) = event["fact_id"].as_str() {
                            holding.insert(id);
                        }
                    }
                    // THE TWO END KINDS STOP AT DIFFERENT TIMES, and the replay has
                    // to know it — which is a thing the answer says, in each event's
                    // `kind`. A fact still holds AT the scene its `canon_to` names
                    // (the interval is closed: `holds_at` asks `p <= canon_to`), and
                    // it has already stopped at the scene where a SUCCESSOR begins
                    // (that one asks `successor.canon_from <= p`). Replaying both as
                    // "gone here" undercounts 26 of 823 scenes, which is how this
                    // test found the distinction rather than assuming it.
                    for event in scene["ends"].as_array().into_iter().flatten() {
                        let (Some(id), Some(kind)) =
                            (event["fact_id"].as_str(), event["kind"].as_str())
                        else {
                            continue;
                        };
                        if kind == "superseded" {
                            holding.remove(id);
                        }
                    }
                    let replayed: BTreeSet<&str> = holding
                        .iter()
                        .copied()
                        .filter(|id| !unplaced.contains(id))
                        .collect();
                    if holding.len() != replayed.len() {
                        touched_by_unplaced += 1;
                    }
                    let counted = scene["holding_count"].as_u64().unwrap_or_default() as usize;
                    if replayed.len() != counted {
                        mismatches.push(format!(
                            "{wire} {} {world} {}: replayed {} of the events it names, counted \
                         {counted}",
                            store.name,
                            scene["section"].as_str().unwrap_or("?"),
                            replayed.len(),
                        ));
                    }
                    // Now the closed end: a fact whose `canon_to` is THIS scene held
                    // here and is gone from the next one.
                    for event in scene["ends"].as_array().into_iter().flatten() {
                        if let Some(id) = event["fact_id"].as_str() {
                            holding.remove(id);
                        }
                    }
                }
            }
        }
    }
    // Print before asserting (the R1026 rule): the reach is the finding as much
    // as the verdict, and a first-violation stop would report one line of it.
    println!(
        "{} authored stores ({} unloadable), {} wire(s) {:?}, {roads} roads, {scenes} scenes \
         replayed; {touched_by_unplaced} scene(s) where an unplaced coordinate is in flight; \
         {} mismatch(es)",
        stores.len(),
        unloadable.len(),
        wires.len(),
        wires,
        mismatches.len(),
    );
    for line in &mismatches {
        println!("  MISMATCH {line}");
    }
    assert!(
        scenes > 100,
        "the replay reached {scenes} scenes, which is a walk that stopped \
         working rather than a repository that emptied"
    );
    assert_eq!(
        mismatches,
        Vec::<String>::new(),
        "the manuscript's holding count is NOT a function of the events the same \
         answer names, which would make the count lossy after all and the \
         record-scoped flag a true positive"
    );
}

/// A locator's `scene_ordinal` is a POSITION, and this is the position it is.
/// (Round 1057.)
///
/// The accounting law above names this field, and naming it is the right
/// verdict rather than a defect: no set of things the answer holds has
/// `scene_ordinal`-many members, because the number does not count anything. It
/// says WHERE on the road the scene beside it sits, and the scene beside it is
/// named — so a consumer already knows what the number is about and only needs
/// it to be true.
///
/// `report-playable-world` carries both halves of that in ONE answer: the road's
/// manuscript, scene by scene in order, and the locators that index into it. So
/// this is not a pair contract and needs no second read — the answer is held to
/// its own order, which is the strongest form this statement can take.
#[test]
fn the_locator_ordinal_is_where_the_answer_puts_that_scene() {
    let (stores, unloadable) = common::authored_stores();
    let mut answered = 0usize;
    let mut roads = 0usize;
    let mut locators = 0usize;
    let mut wrong: Vec<String> = Vec::new();
    for store in &stores {
        for telling in tellings_of(store) {
            let out = common::run(
                store.ws.path(),
                &["report-playable-world", "--telling", &telling, "--json"],
            );
            if !out.status.success() {
                continue;
            }
            let Ok(playable) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
                continue;
            };
            answered += 1;
            for (world, road) in playable["worlds"].as_object().into_iter().flatten() {
                roads += 1;
                let order: Vec<&str> = road["manuscript"]["scenes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|scene| scene["section"].as_str())
                    .collect();
                for locator in road["locators"].as_array().into_iter().flatten() {
                    locators += 1;
                    let (Some(fact), Some(scene), Some(ordinal)) = (
                        locator["fact_id"].as_str(),
                        locator["scene"].as_str(),
                        locator["scene_ordinal"].as_u64(),
                    ) else {
                        continue;
                    };
                    let at = order.iter().position(|section| *section == scene);
                    if at != Some(ordinal as usize) {
                        wrong.push(format!(
                            "{} {world} {fact}: the locator puts {scene} at {ordinal}, the road \
                             this same answer plays has it at {at:?}",
                            store.name,
                        ));
                    }
                }
            }
        }
    }
    // Print before asserting (the R1026 rule).
    println!(
        "{answered} playable-world answer(s) over {} authored stores ({} unloadable), {roads} \
         roads, {locators} locators; {} that index somewhere else",
        stores.len(),
        unloadable.len(),
        wrong.len(),
    );
    for line in &wrong {
        println!("  ELSEWHERE {line}");
    }
    assert!(
        locators > 100,
        "the walk reached {locators} locators, which is a read that stopped \
         answering rather than a repository that emptied"
    );
    assert_eq!(
        wrong,
        Vec::<String>::new(),
        "a locator's ordinal is not a position on the road the same answer plays"
    );
}

/// The quest graph's `scene_ordinal` is a position too — and the answer it sits
/// in cannot say a position in WHAT, so its oracle is a second shipped read.
/// (Round 1058.)
///
/// This is the one number the accounting law names that Round 1057 left with
/// neither an account nor a derivation, and the reason it was left is the reason
/// it needs this shape. `report-playable-world` carries the road its locators
/// index into, so that ordinal is held to its own answer; the quest graph
/// carries `world_line` and `scene` and no scene sequence anywhere, so a reader
/// holding this answer alone cannot check the number and neither can a test.
///
/// The oracle is therefore the read that DOES state the sequence — the R1050
/// rule, an oracle that is another shipped read rather than a recomputation of
/// the projection under test. A recomputed order would be the same thought
/// written twice (the R1041 shape); the manuscript is what a reader would
/// actually open to find out where scene `sc-12` falls on the road `parley`.
///
/// A locator naming a road the manuscript does not walk is NOT a failure of this
/// law — it is the oracle declining to answer, and it is counted and named
/// rather than passed over.
#[test]
fn the_quest_locator_ordinal_is_where_the_manuscript_puts_that_scene() {
    let (stores, unloadable) = common::authored_stores();
    let mut answered = 0usize;
    let mut quests = 0usize;
    let mut locators = 0usize;
    let mut unreached: Vec<String> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    for store in &stores {
        // THE ORACLE, asked first: the roads this store's manuscript walks, each
        // one the sequence of scenes a reader plays through.
        let told = common::run(
            store.ws.path(),
            &["report-playthrough-manuscript", "--json"],
        );
        if !told.status.success() {
            continue;
        }
        let Ok(manuscript) = serde_json::from_slice::<serde_json::Value>(&told.stdout) else {
            continue;
        };
        let roads: BTreeMap<&str, Vec<&str>> = manuscript["worlds"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(road, walk)| {
                let scenes: Vec<&str> = walk["scenes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|scene| scene["section"].as_str())
                    .collect();
                (road.as_str(), scenes)
            })
            .collect();

        for telling in tellings_of(store) {
            let out = common::run(
                store.ws.path(),
                &["report-quest-graph", "--telling", &telling, "--json"],
            );
            if !out.status.success() {
                continue;
            }
            let Ok(graph) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
                continue;
            };
            answered += 1;
            for quest in graph["quests"].as_array().into_iter().flatten() {
                quests += 1;
                for locator in quest["locators"].as_array().into_iter().flatten() {
                    locators += 1;
                    let (Some(quest_id), Some(fact), Some(road), Some(scene), Some(ordinal)) = (
                        quest["quest_id"].as_str(),
                        locator["fact_id"].as_str(),
                        locator["world_line"].as_str(),
                        locator["scene"].as_str(),
                        locator["scene_ordinal"].as_u64(),
                    ) else {
                        continue;
                    };
                    let Some(walk) = roads.get(road) else {
                        // The oracle declines: no road by that name is walked.
                        unreached.push(format!("{} {quest_id} {fact}: {road}", store.name));
                        continue;
                    };
                    let at = walk.iter().position(|section| *section == scene);
                    if at != Some(ordinal as usize) {
                        wrong.push(format!(
                            "{} {quest_id} {fact}: the quest graph puts {scene} at {ordinal} on \
                             {road}, the manuscript walks it at {at:?}",
                            store.name,
                        ));
                    }
                }
            }
        }
    }
    // Print before asserting (the R1026 rule).
    println!(
        "{answered} quest-graph answer(s) over {} authored stores ({} unloadable), {quests} \
         quests, {locators} locators; {} the manuscript walks no road for; {} that index \
         somewhere else",
        stores.len(),
        unloadable.len(),
        unreached.len(),
        wrong.len(),
    );
    for line in unreached.iter().chain(wrong.iter()) {
        println!("  ELSEWHERE {line}");
    }
    assert!(
        locators > 5,
        "the walk reached {locators} quest locators, which is a corpus that \
         declares no quest rather than a law with nothing to say — the migrated \
         dnd-quest record alone carries twelve"
    );
    assert_eq!(
        (unreached, wrong),
        (Vec::<String>::new(), Vec::<String>::new()),
        "a quest locator's ordinal is not a position on the road the manuscript \
         walks under that name"
    );
}

/// The frontier's `density` is a FRACTION, so the accounting law does not judge
/// it at all — and this says what it is, from the inside. (Round 1057; the
/// denominator opened in Round 1061.)
///
/// Numerator and denominator are both in the answer and until this round they
/// were not alike. The numerator is `owned`, a list of facts the read NAMES
/// since Round 1054; the denominator was `road_scenes`, a count of coordinates
/// the answer named none of — recorded from the outside by Rounds 1052, 1054
/// and 1055 as something no walk had judged, because no corruption in the
/// population could move a canon order. Round 1061's population could, the
/// number moved with nothing named moving beside it, and the road is named now.
/// So the ratio is a statement about two LISTS.
#[test]
fn the_density_is_the_facts_it_names_over_the_road_it_names() {
    let (stores, unloadable) = common::authored_stores();
    let mut answered = 0usize;
    let mut roads = 0usize;
    let mut roadless = 0usize;
    let mut wrong: Vec<String> = Vec::new();
    for store in &stores {
        let out = common::run(store.ws.path(), &["report-authoring-frontier", "--json"]);
        if !out.status.success() {
            continue;
        }
        let Ok(frontier) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
            continue;
        };
        answered += 1;
        for (world, row) in frontier["branch_owned_density"]
            .as_object()
            .into_iter()
            .flatten()
        {
            roads += 1;
            let owned = row["owned"].as_array().map_or(0, Vec::len);
            let Some(road) = row["road"].as_array() else {
                wrong.push(format!(
                    "{} {world}: the answer carries no road for the density to be over",
                    store.name,
                ));
                continue;
            };
            let scenes = road.len();
            let density = row["density"].as_f64();
            if scenes == 0 {
                // A world that travels no scene has no density to state, and
                // says so rather than dividing (`Option<f64>`, R619).
                roadless += 1;
                if density.is_some() {
                    wrong.push(format!(
                        "{} {world}: {owned} owned over a road of no scenes, and a density anyway",
                        store.name,
                    ));
                }
                continue;
            }
            let derived = owned as f64 / scenes as f64;
            if density.is_none_or(|said| (said - derived).abs() > 1e-9) {
                wrong.push(format!(
                    "{} {world}: {owned} named over {scenes} scenes is {derived}, the read says \
                     {density:?}",
                    store.name,
                ));
            }
        }
    }
    // Print before asserting (the R1026 rule).
    println!(
        "{answered} frontier answer(s) over {} authored stores ({} unloadable), {roads} \
         world-lines ({roadless} travelling no scene); {} whose density is not the facts it names \
         over the road it counts",
        stores.len(),
        unloadable.len(),
        wrong.len(),
    );
    for line in &wrong {
        println!("  NOT THE RATIO {line}");
    }
    assert!(
        roads > 20,
        "the walk reached {roads} world-lines, which is a read that stopped \
         answering rather than a repository that emptied"
    );
    assert_eq!(
        wrong,
        Vec::<String>::new(),
        "the density is not the facts the answer names over the road it counts"
    );
}

/// `report-payoff-substantiation` must NAME every setup it counts, and until
/// Round 1056 it did not. (Round 1056.)
///
/// The census above found this one by asking its question of every PAIR of
/// answers rather than of each answer against the unedited store: retargeting
/// `f-041`'s world-line and dropping `f-041`'s payoff expectation produce two
/// answers that NAME identical things and disagree about `setups_total`. A number
/// that two answers naming the same things disagree about is carrying information
/// nothing named carries — which is this law's whole subject, and the pair that
/// proves it is a pair neither answer forms with the baseline.
///
/// The cause is a class this read is silent about. `setups_total` is every fact
/// marked `expected`, store-wide; `worlds` classifies only the setups some payoff
/// CREDITED, three ways. A setup no payoff credits — the author's todo, which the
/// coverage sibling names as `dangling` — appears in this report only as a
/// contribution to the total. So a consumer reads "12 setups" against a body
/// naming nine and cannot learn which three are missing, or that they are the
/// ones substantiation cannot even be asked about.
///
/// This test states the property rather than the fix: every setup the total
/// counts is named somewhere in the same answer.
///
/// IT ASKS A CONSTRUCTED STORE AS WELL, and that is not decoration. All 28
/// authored corpora satisfy the property as shipped — an author who marks a setup
/// goes on to pay it — so a sweep of what authors wrote reports a clean surface
/// and the census's finding reads as noise. The shape that breaks it is a setup
/// NO payoff credits, which the census reached by dropping one expectation from
/// an authored store. What the corpora cannot make, the tree makes, through the
/// same import recipe an author would use (the R1052 rule).
#[test]
fn payoff_substantiation_names_every_setup_it_counts() {
    let section = |id: &str, title: &str| serde_json::json!({"section_id": id, "parent_doc": "constructed", "title": title});
    let sections = serde_json::json!([
        section("n-01", "the gun on the wall"),
        section("n-02", "the gun goes off"),
    ]);
    let order = serde_json::json!({"edges": [["n-01", "n-02"]]});
    let fact = |id: &str, at: &str, claim: &str| {
        serde_json::json!({
            "fact_id": id,
            "frame": "f-teller",
            "claim": claim,
            "canon_from": at,
            "evidence": [at],
        })
    };
    let mut paid_setup = fact("fx-paid-setup", "n-01", "a rifle hangs over the hearth");
    paid_setup["payoff_expectation"] = serde_json::json!("expected");
    let mut payoff = fact("fx-payoff", "n-02", "the rifle is fired");
    payoff["pays_off"] = serde_json::json!(["fx-paid-setup"]);
    // THE SHAPE NO AUTHOR SHIPPED: marked as a setup, credited by nothing.
    let mut dangling_setup = fact("fx-dangling-setup", "n-01", "a locked case sits beneath it");
    dangling_setup["payoff_expectation"] = serde_json::json!("expected");
    let facts = serde_json::json!({
        "frames": [{"frame_id": "f-teller"}],
        "facts": [paid_setup, payoff, dangling_setup],
    });
    let constructed = common::constructed_corpus(&sections, &order, &facts)
        .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"));

    let (stores, unloadable) = common::authored_stores();
    let mut targets: Vec<(String, &std::path::Path)> = stores
        .iter()
        .map(|store| (store.name.clone(), store.ws.path()))
        .collect();
    targets.push((
        "a constructed store with one uncredited setup".to_string(),
        constructed.path(),
    ));
    let mut asked = 0usize;
    let mut shortfalls: Vec<String> = Vec::new();
    for (name, path) in &targets {
        let out = common::run(path, &["report-payoff-substantiation", "--json"]);
        if !out.status.success() {
            continue;
        }
        asked += 1;
        let report: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("substantiation json");
        let mut named: BTreeSet<&str> = BTreeSet::new();
        for road in report["worlds"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(_, w)| w)
        {
            for class in ["substantiated", "unsubstantiated", "unverifiable"] {
                for row in road[class].as_array().into_iter().flatten() {
                    if let Some(setup) = row["setup"].as_str() {
                        named.insert(setup);
                    }
                }
            }
            for class in ["dangling", "unknown"] {
                for row in road[class].as_array().into_iter().flatten() {
                    if let Some(setup) = row.as_str() {
                        named.insert(setup);
                    }
                }
            }
        }
        let counted = report["setups_total"].as_u64().unwrap_or_default() as usize;
        if named.len() != counted {
            shortfalls.push(format!(
                "{name}: counts {counted} setups, names {}",
                named.len(),
            ));
        }
    }
    // Print before asserting (the R1026 rule).
    println!(
        "{asked} stores answered ({} unloadable); {} name fewer setups than they count",
        unloadable.len(),
        shortfalls.len(),
    );
    for line in &shortfalls {
        println!("  SHORTFALL {line}");
    }
    assert!(asked > 20, "the sweep asked {asked} stores");
    assert_eq!(
        shortfalls,
        Vec::<String>::new(),
        "a setup counted and never named is a number a consumer cannot account for"
    );
}
