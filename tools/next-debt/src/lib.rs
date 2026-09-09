//! What the outstanding-debt ledger says this round should take, and in what
//! order — the three questions the standing rules ask an instrument for.
//!
//! THE RULES NAME AN INSTRUMENT AND THERE WAS NONE. They say: do not pick the
//! next debt by eye, take from the critical line; a row nobody ranked is not an
//! ordinary row but an undecided one, and that count only shrinks; and a row
//! whose provenance chain runs deeper than the re-aiming cap is registered now
//! and repaid later. Each sentence is about a number, and until this crate every
//! one of those numbers was a thing a session read off the ledger by eye.
//!
//! WHAT MAKES THAT A DEFECT RATHER THAN AN INCONVENIENCE is written in the
//! ledger's own history: the session before this one swapped the standing `NEXT`
//! for a different row and recorded, in the same breath, that it had chosen by
//! reading — because there was nothing to ask. An order chosen that way leaves
//! no trace anybody can re-derive, which is this repository's oldest complaint
//! about prose standing in for a program.
//!
//! THE NOTATION IS THE LEDGER'S AND THIS ONLY READS IT. A row's rank and its
//! provenance are written inside the very parenthetical that already carries its
//! branch, so there is one region a registration is classified by and one crate
//! that decides where that region begins. `open-debts` owns that decision; this
//! asks it.

use std::collections::{BTreeMap, BTreeSet};

use open_debts::Unresolved;

/// Where a row sits in the order the standing rules ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    /// Take from these first, and take nothing else while any is open.
    Critical,
    /// Ranked, and ranked below critical.
    Ordinary,
    /// NOBODY DECIDED, which is not the same as "ordinary" and is the whole
    /// reason this is a third value rather than a default. A row that arrives
    /// unranked has had no judgement passed on it at all; calling that
    /// "ordinary" would record a decision nobody made, and the count of them
    /// would stop being a thing that can be driven down.
    Unranked,
}

/// The word a rank is written with, so the reader and the writer share one
/// spelling rather than two that drift.
const CRITICAL: &str = "critical";
/// The other one.
const ORDINARY: &str = "ordinary";

/// What a row says made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// `@from: none` — met while repaying something else, and therefore a root.
    /// The distinction this carries is the one the standing rule insists on:
    /// what a repayment MADE is its child, what a repayment MET is not, because
    /// filing the second as a child lengthens a chain every time somebody steps
    /// on a defect that was already there, and the oldest debts sink fastest.
    Encountered,
    /// A round made it. A round is not a row, so the chain ends here at the same
    /// depth a row's own repayment would put it: one hop from the work that was
    /// being done.
    Round(String),
    /// The row being repaid made it.
    Row(String),
    /// No `@from` at all, which is what every row registered before the rule
    /// existed looks like. It is a ROOT for the walk and it is COUNTED APART: a
    /// number that can only be driven to zero by rewriting history is a number
    /// no termination condition may contain.
    Unrecorded,
    /// The mark is written and what follows it is not a name. NOT FOLDED INTO
    /// `Unrecorded`, because the two say opposite things about the author: one
    /// row predates the rule and one row tried to follow it and failed, and a
    /// reader that calls the second "no provenance recorded" hides a typo inside
    /// a number that is supposed to be shrinking.
    Unreadable,
}

/// One open row, with what the ledger says about where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    /// 1-based, so a reader can open the ledger at it.
    pub line: usize,
    pub rank: Rank,
    pub origin: Origin,
}

/// Something about the ledger that stops this reader answering.
///
/// EVERY ONE OF THESE IS "NOT JUDGED" AND NOT "NOTHING FOUND", which is the
/// distinction `open-debts` was built around and the one this inherits: a walk
/// that could not run and a ledger with nothing in it print the same silence
/// unless the program refuses out loud.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// A classification says the row is both ranks. Choosing one would be this
    /// reader deciding what the ledger left ambiguous.
    RankIsBoth { id: String, line: usize },
    /// A row's `@from` names something this ledger never registered, so the
    /// chain it belongs to has no depth. Being unable to measure is not a zero.
    OriginNamesNothingRegistered {
        id: String,
        line: usize,
        named: String,
    },
    /// A row's `@from` is written and names nothing at all.
    OriginUnreadable { id: String, line: usize },
    /// A provenance chain that returns to itself. There is no depth to report
    /// and the ids are named so the repair is one edit rather than a search.
    OriginCycles { ids: Vec<String> },
}

/// Why a number a document was supposed to declare could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationFault {
    /// The document declares it nowhere.
    Absent,
    /// More than one line declares it, so there is no single value. TWO LINES
    /// SAYING DIFFERENT THINGS IS THE FAILURE THIS REFUSES, and two lines saying
    /// the same thing is refused with it: a duplicated datum is a datum with two
    /// homes, and this repository has an invariant about that.
    Repeated { lines: Vec<usize> },
    /// The line is there and what follows the marker is not a number.
    Unreadable { line: usize, said: String },
}

/// The marker the ledger pins its unranked ratchet with.
pub const UNRANKED_RATCHET: &str = "UNRANKED RATCHET";

/// The marker the process document pins the re-aiming cap with.
///
/// THE CAP IS NOT WRITTEN IN THIS PROGRAM, deliberately. The standing rule says
/// the instrument prints *the document's* value, so that the rule and the number
/// it turns on cannot be argued about in two places. A constant here would be a
/// second home for a datum whose home is `RULEBOOK.md`.
pub const REAIMING_CAP: &str = "RE-AIMING CAP";

/// The number a document declares on a line of its own.
///
/// A LINE OF ITS OWN, so that prose ABOUT the marker is not mistaken for the
/// marker. That is not a hypothetical: the ledger section pinning this notation
/// is a section that has to spell it, and a reader taking the first match
/// anywhere would take the sentence explaining the rule instead of the rule.
///
/// A LINE MAY BE DECORATED — the ledger writes its rows as bold bullets — so the
/// bullet and the emphasis are stripped before the marker is looked for, and
/// nothing else is.
///
/// # Errors
///
/// Refuses when the marker is absent, when more than one line carries it, or
/// when what follows it is not a number.
pub fn declared(text: &str, marker: &str) -> Result<usize, DeclarationFault> {
    let mut found: Vec<(usize, String)> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let bare = line
            .trim_start()
            .trim_start_matches(['-', '*', '#', ' ', '\t']);
        if let Some(rest) = bare.strip_prefix(marker) {
            found.push((index + 1, rest.to_string()));
        }
    }
    let [(line, rest)] = found.as_slice() else {
        return Err(if found.is_empty() {
            DeclarationFault::Absent
        } else {
            DeclarationFault::Repeated {
                lines: found.iter().map(|(line, _)| *line).collect(),
            }
        });
    };
    let Some(after) = rest.trim_start().strip_prefix('=') else {
        return Err(DeclarationFault::Unreadable {
            line: *line,
            said: rest.trim().to_string(),
        });
    };
    let digits: String = after
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse::<usize>()
        .map_err(|_| DeclarationFault::Unreadable {
            line: *line,
            said: after.trim().to_string(),
        })
}

/// The rank a classification states, or `Unranked` when it states none, or
/// nothing at all when it states both.
fn rank_in(classification: &str) -> Option<Rank> {
    match (
        word_in(classification, CRITICAL),
        word_in(classification, ORDINARY),
    ) {
        (true, true) => None,
        (true, false) => Some(Rank::Critical),
        (false, true) => Some(Rank::Ordinary),
        (false, false) => Some(Rank::Unranked),
    }
}

/// Whether a word stands on its own in a piece of text.
///
/// ON ITS OWN, because the ledger writes a rank bold and writes prose around it.
/// Emphasis is punctuation, so the test is that what sits either side of the
/// word is not alphanumeric; that admits the bold form without admitting a rank
/// spelled inside a longer word.
fn word_in(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    let mut at = 0usize;
    while let Some(offset) = text[at..].find(word) {
        let start = at + offset;
        let end = start + word.len();
        let before_is_word = start > 0 && bytes[start - 1].is_ascii_alphanumeric();
        let after_is_word = bytes.get(end).is_some_and(u8::is_ascii_alphanumeric);
        if !before_is_word && !after_is_word {
            return true;
        }
        at = end;
    }
    false
}

/// The provenance mark the ledger writes, spelled once.
const FROM: &str = "@from:";

/// The word an origin uses for "nothing made this; it was already here".
const NONE: &str = "none";

/// What a classification says made this row.
fn origin_in(classification: &str) -> Origin {
    let Some(at) = classification.find(FROM) else {
        return Origin::Unrecorded;
    };
    let rest = &classification[at + FROM.len()..];
    // THE TOKEN ENDS AT THE FIRST THING THAT IS NOT PART OF A NAME. The ledger
    // writes the mark inside a code span and inside a longer parenthetical, so
    // the backtick, the comma, the dash and the closing paren all end it — and
    // taking "to the end of the parenthetical" would read a whole sentence as an
    // id and report every rooted row as dangling.
    let token: String = rest
        .trim_start()
        .trim_start_matches(['`', '*'])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if token.eq_ignore_ascii_case(NONE) {
        return Origin::Encountered;
    }
    if token.is_empty() {
        return Origin::Unreadable;
    }
    let round = token
        .strip_prefix('R')
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()));
    if round {
        return Origin::Round(token);
    }
    Origin::Row(token)
}

/// What every registration in this ledger says made it, retired rows included,
/// and the line each was registered on.
///
/// RETIRED ROWS ARE IN IT because a chain walks through them: the row a
/// repayment created outlives the row that was being repaid, and the ledger's
/// own first chain is exactly that shape. A walk over open rows only would call
/// every such parent missing and refuse a ledger that is correct.
///
/// THE FIRST REGISTRATION OF AN ID WINS, which is the policy the census already
/// applies, and a mention is not a registration.
fn origins(ledger: &str) -> BTreeMap<String, (usize, Origin)> {
    let mut said: BTreeMap<String, (usize, Origin)> = BTreeMap::new();
    for row in open_debts::registrations(ledger) {
        if row.is_a_mention() {
            continue;
        }
        let told = origin_in(&row.classification());
        said.entry(row.id.clone()).or_insert((row.line, told));
    }
    said
}

/// How many hops of provenance stand between this row and the work that was
/// being done when it was opened.
fn depth_of(
    id: &str,
    said: &BTreeMap<String, (usize, Origin)>,
    walked: &mut Vec<String>,
) -> Result<usize, Fault> {
    if walked.iter().any(|seen| seen == id) {
        walked.push(id.to_string());
        return Err(Fault::OriginCycles {
            ids: walked.clone(),
        });
    }
    walked.push(id.to_string());
    let Some((line, told)) = said.get(id) else {
        return Ok(0);
    };
    match told {
        Origin::Unrecorded | Origin::Encountered => Ok(0),
        Origin::Round(_) => Ok(1),
        Origin::Unreadable => Err(Fault::OriginUnreadable {
            id: id.to_string(),
            line: *line,
        }),
        Origin::Row(parent) => {
            if !said.contains_key(parent) {
                return Err(Fault::OriginNamesNothingRegistered {
                    id: id.to_string(),
                    line: *line,
                    named: parent.clone(),
                });
            }
            Ok(1 + depth_of(parent, said, walked)?)
        }
    }
}

/// The order this ledger asks for, measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Steering {
    /// The open rows of the autonomous branch, in the census's own order.
    pub open: Vec<Row>,
    /// Each open row's provenance depth, by id.
    pub depths: BTreeMap<String, usize>,
}

impl Steering {
    /// The rows the standing rule says to take from, in the census's order.
    #[must_use]
    pub fn critical(&self) -> Vec<&Row> {
        self.open
            .iter()
            .filter(|row| row.rank == Rank::Critical)
            .collect()
    }

    /// How many open rows nobody has ranked.
    #[must_use]
    pub fn unranked(&self) -> usize {
        self.open
            .iter()
            .filter(|row| row.rank == Rank::Unranked)
            .count()
    }

    /// How many open rows record no provenance at all.
    ///
    /// COUNTED AND NOT GATED. Every row opened before the rule existed is one of
    /// these, and the only way to drive the number down is to write provenance
    /// onto rows whose provenance nobody remembers. A count that cannot reach
    /// zero by working is a count no termination condition may hold, so this one
    /// is printed and nothing turns on it.
    #[must_use]
    pub fn unrooted(&self) -> usize {
        self.open
            .iter()
            .filter(|row| row.origin == Origin::Unrecorded)
            .count()
    }

    /// How many open rows sit at each depth.
    #[must_use]
    pub fn by_depth(&self) -> BTreeMap<usize, usize> {
        let mut spread: BTreeMap<usize, usize> = BTreeMap::new();
        for depth in self.depths.values() {
            *spread.entry(*depth).or_default() += 1;
        }
        spread
    }

    /// The rows the cap says to register now and repay later.
    #[must_use]
    pub fn deferred(&self, cap: usize) -> Vec<&Row> {
        self.open
            .iter()
            .filter(|row| self.depths.get(&row.id).is_some_and(|depth| *depth > cap))
            .collect()
    }
}

/// What the ledger says about the order of its own open work.
///
/// THE POPULATION IS THE CENSUS'S, asked for through the crate that owns it, and
/// this is the one place the two programs could disagree: the names a retirement
/// gives are resolved by `open-debts` against git and the atomic store, and they
/// are not resolved here. Reading them as written can only make this set
/// SMALLER than the truth — a retirement resting on a name that does not exist
/// retires nothing, and `open-debts` refuses such a ledger outright rather than
/// letting the arc terminate on it. So the row that would go missing here is a
/// row that has already reddened the census, in the same round, and the summary
/// line says how many names went unasked so the difference has a reader.
///
/// # Errors
///
/// Refuses a ledger whose ranks are ambiguous or whose provenance chains cannot
/// be walked, naming every such row rather than the first.
pub fn steering(ledger: &str) -> Result<Steering, Vec<Fault>> {
    let said = origins(ledger);
    let mut faults: Vec<Fault> = Vec::new();
    let mut open: Vec<Row> = Vec::new();
    let mut depths: BTreeMap<String, usize> = BTreeMap::new();
    for registration in open_debts::open_autonomous(ledger, &Unresolved::default()) {
        let classification = registration.classification();
        let (id, line) = (registration.id, registration.line);
        let Some(rank) = rank_in(&classification) else {
            faults.push(Fault::RankIsBoth {
                id: id.clone(),
                line,
            });
            continue;
        };
        match depth_of(&id, &said, &mut Vec::new()) {
            Ok(depth) => {
                depths.insert(id.clone(), depth);
            }
            Err(fault) => {
                faults.push(fault);
                continue;
            }
        }
        open.push(Row {
            id,
            line,
            rank,
            origin: origin_in(&classification),
        });
    }
    // A CYCLE IS ONE FINDING HOWEVER MANY ROWS STAND ON IT, and a ledger with one
    // would otherwise print the same ring once per member — DEDUPED ON THE
    // MEMBERS AND NOT ON THE WALK, because the walk from each member of a ring
    // spells the same ring starting somewhere else, and two spellings of one
    // finding is the report shape this repository keeps complaining about.
    let mut rings: BTreeSet<BTreeSet<String>> = BTreeSet::new();
    faults.retain(|fault| match fault {
        Fault::OriginCycles { ids } => rings.insert(ids.iter().cloned().collect()),
        _ => true,
    });
    if faults.is_empty() {
        Ok(Steering { open, depths })
    } else {
        Err(faults)
    }
}
