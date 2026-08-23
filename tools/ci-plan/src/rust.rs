//! THE SIXTH PLACE A CARGO COMMAND IS WRITTEN: a Rust program's own words.
//!
//! The five sources beside this one are all DATA — a workflow's YAML, a script's
//! text, a declaration's TOML, a sweep manifest's JSON array, a lister's output —
//! and a static reader is the only reader they can have. The sixth is different
//! in kind: the words are COMPUTED, assembled by `.arg()` and `.args()` out of
//! values a static reader cannot evaluate, and Round 1257 wrote that down as a
//! limit of shape rather than of effort.
//!
//! It is a limit on the WORDS and never on the POPULATION, and that distinction
//! is the whole of this module. Spawning is a syntactic act: `Command::new(..)`
//! is written down, in a tracked file, and `git ls-files` plus `syn` enumerate
//! every one of them exactly. So this reader can always say WHERE a program
//! issues a cargo command, and says as much about the WORDS as each site spells.
//!
//! # Why not a record of what was actually run
//!
//! The other answer Round 1257 named was to have each program record the
//! commands it issued and to judge the record. It is the wrong one, for the
//! reason this repository has paid for more than once: a program nobody ran
//! writes an empty record, and an empty record is what a clean program and an
//! unexamined one both look like. Every other source here has a population that
//! is complete on a fresh clone with nothing run at all, and a law whose
//! completeness depends on somebody having exercised the code is a law that goes
//! quiet exactly where the code is rarely exercised. A record would also live
//! under `target/`, which is this machine's and not the repository's (N170).
//!
//! # The one fact the syntax does not hold, and who supplies it
//!
//! `--manifest-path` followed by a value is the same three words in a gate that
//! walks this repository's own workspaces and in a fixture that made a
//! throwaway one two lines earlier, and the two want opposite verdicts. So the
//! call site declares it — [`crate::issue::Tree`], an argument the compiler will
//! not let anybody leave out — and this reader reads the declaration beside the
//! words. That is why every cargo spawn must come through [`crate::issue`]: a
//! second door is a site with no declaration to read, and
//! [`Program::CargoBesideTheDoor`] is what this walk calls one.
//!
//! # What a site is sorted into
//!
//! - **[`RustSpawns::commands`]** — through the door with every argument
//!   readable, carrying what the site declared. A [`CargoCommand`] like any
//!   other, so the three laws over that population judge it with no new
//!   vocabulary at all. An argument decided at runtime is still readable AS ONE
//!   WORD: `.arg(&manifest)` renders `$manifest`, which is the spelling a shell
//!   command assembling a path arrives with, and
//!   [`crate::CargoCommand::manifest`] already reads it — though for these
//!   commands the declaration answers before that reading is needed.
//! - **[`RustSpawns::carried`]** — `.args(expr)` hands over an UNKNOWN NUMBER of
//!   words, so the word list has a hole no rendering can fill: a flag might be
//!   in there. Counted and named rather than judged, because a command this
//!   reader cannot finish reading and reports as compliant is the empty answer
//!   wearing a clean one's clothes. A hole that is a PARAMETER is followed one
//!   hop BACKWARDS first — see below — and what stays here is what that hop
//!   could not finish.
//! - **[`RustSpawns::beside_the_door`]** — a cargo spawn that did not come
//!   through [`crate::issue::cargo`]. The defect.
//! - **[`RustSpawns::unplaceable`]** — the PROGRAM is an expression this reader
//!   cannot name even after following it one hop, so it cannot say whether cargo
//!   is what runs. Counted and named: R1190 named the failure of reporting the
//!   LIMIT and never its SIZE one directory over, where a spawn the walk did not
//!   recognise went missing from the judgement and from every number at once.
//!
//! # How a site says it runs cargo
//!
//! Through [`crate::issue::cargo`], which is the whole of the answer after this
//! round. The spellings below are what the walk still RECOGNISES, because a
//! reader that could not see the second door could not report it:
//!
//! 1. the literal `"cargo"`;
//! 2. a call to a `cargo()`-shaped helper — the `std::env::var("CARGO")` line
//!    five programs under `tools/` each carried a copy of;
//! 3. an expression reading the `CARGO` environment variable inline;
//! 4. a value holding one of those, followed one hop.
//!
//! `env!("CARGO_BIN_EXE_<name>")` is NOT one of them — that is a binary this
//! workspace builds, and it is recognised here precisely so it is never mistaken
//! for cargo by a reader that matched the letters.
//!
//! # The hop in the other direction
//!
//! Everywhere above, a name is followed FORWARDS to the value it holds. A
//! wrapper's words cannot be read that way at all:
//!
//! ```text
//! fn cargo(root: &Path, argv: &[&str]) -> …  { issue::cargo(..).args(argv) }
//! ```
//!
//! `argv` holds nothing here. It holds whatever each CALL SITE writes, and the
//! call sites write literals — `["metadata", "--no-deps", …]`, `["check", "-q",
//! "--locked", …]`. So the words are one hop away in the opposite direction, and
//! R1262 wrote them down as a limit of shape when they were undone work.
//!
//! One spawn becomes one command PER CALL SITE, attributed to the caller,
//! because that is the file a person edits to change what runs. What the hop
//! costs is safety about WHICH function is being called, and three rules buy it:
//! only a plain `fn` is followed (a method's calls are written `x.name(..)`, and
//! no reading of the syntax says which type `x` is); the name and its argument
//! count must belong to exactly one tracked function, which is the forward hop's
//! "a name that means two things means neither" pointed backwards; and a call
//! qualified by a TYPE — `Command::new(..)` — is not a call of a free function
//! of that name. Every refusal is a sentence on the site
//! ([`RustSpawn::unfollowed`]), and a hop that reaches only some of the call
//! sites leaves the site carried with the count of the ones it did not
//! ([`RustSpawn::reach`]).
//!
//! And a call written inside a MACRO invocation is listed as a call site whose
//! words are unreadable. `syn` hands macro tokens over unparsed, so
//! `assert!(run(&["build"]).is_ok())` is invisible to the walk — the one way
//! this hop could report a partly-read site as a finished one, which is the
//! failure this repository keeps paying for in other shapes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use proc_macro2::LineColumn;
use quote::ToTokens as _;
use syn::spanned::Spanned as _;
use syn::visit::Visit;

use crate::{cargo_invocation, tracked_files, tracked_manifests, CargoCommand};

/// What a spawn site declared about the tree it resolves, as the SYNTAX says it.
///
/// A second spelling of [`crate::issue::Tree`] and deliberately so: that one is
/// the value a running program holds, this one is what a reader of the file can
/// see. They are the same fact read from two sides, and the direction of any
/// disagreement is loud — a variant this reader does not know becomes
/// [`Declared::Unreadable`], which no law passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declared {
    /// `Tree::ThisRepository`.
    ThisRepository,
    /// `Tree::MadeByThisRun("…")`, with the reason the site gave.
    MadeByThisRun(String),
    /// `Tree::WhereverTheCallerPoints("…")`. The arm that owes the most: the
    /// command must resolve nothing, and [`crate::lock_verdict`] is where that
    /// is held.
    WhereverTheCallerPoints(String),
    /// `Tree::PinnedWhereverItPoints("…")`. The other way to pay for declining
    /// to name a tree: the command must say `--locked`, so a disagreement is
    /// reported wherever it is met rather than repaired.
    PinnedWhereverItPoints(String),
    /// `Tree::PinnedWhenItIsOurs("…")`. The flag is not the site's to spell
    /// unconditionally: it says `--locked` on the paths where the tree turns out
    /// to be one of this repository's, and the law reads the flag's presence as
    /// the answer to that question — held honest by demanding the flag be
    /// CONDITIONAL, which is a thing a site can fail to do.
    PinnedWhenItIsOurs(String),
    /// A `Tree` expression this reader cannot read — a variable, a call, a
    /// variant it does not know. NOT a pass.
    Unreadable(String),
}

impl Declared {
    /// The arm's own name, so a law about ONE arm can ask how many sites
    /// declared it.
    ///
    /// A NAME RATHER THAN A `matches!` AT EVERY CALLER, for the reason R1268
    /// gave about the list of laws: a fact several places re-derive is a fact
    /// they are free to derive differently.
    #[must_use]
    pub fn arm(&self) -> &'static str {
        match self {
            Self::ThisRepository => "ThisRepository",
            Self::MadeByThisRun(_) => "MadeByThisRun",
            Self::WhereverTheCallerPoints(_) => "WhereverTheCallerPoints",
            Self::PinnedWhereverItPoints(_) => "PinnedWhereverItPoints",
            Self::PinnedWhenItIsOurs(_) => "PinnedWhenItIsOurs",
            Self::Unreadable(_) => "Unreadable",
        }
    }

    /// The arm after this one, and `None` at the last — the chain
    /// [`Declared::arms`] walks.
    ///
    /// EXHAUSTIVE BECAUSE THE COMPILER KEEPS IT SO. A sixth arm added to the
    /// enum and forgotten here does not build, which is the difference between
    /// a list of the arms and a list of the arms somebody remembered. Every
    /// payload is empty: this chain is the arms as VALUES, not as data.
    fn after(&self) -> Option<Self> {
        Some(match self {
            Self::ThisRepository => Self::MadeByThisRun(String::new()),
            Self::MadeByThisRun(_) => Self::WhereverTheCallerPoints(String::new()),
            Self::WhereverTheCallerPoints(_) => Self::PinnedWhereverItPoints(String::new()),
            Self::PinnedWhereverItPoints(_) => Self::PinnedWhenItIsOurs(String::new()),
            Self::PinnedWhenItIsOurs(_) => Self::Unreadable(String::new()),
            Self::Unreadable(_) => return None,
        })
    }

    /// One value of every arm, payloads empty.
    ///
    /// R1277, AND IT IS VALUES RATHER THAN NAMES so a law over "every arm" can
    /// both NAME one ([`Declared::arm`]) and MATCH on it — a law that told the
    /// arms apart by string would go on holding over five of six the day a
    /// variant is renamed, which is R1271's silence one level up. The list
    /// itself is what makes such a law exhaustive: an arm added to the enum
    /// arrives in it without anyone deciding to put it there, and the law then
    /// asks the new arm the same question it asks the other five.
    #[must_use]
    pub fn arms() -> Vec<Self> {
        std::iter::successors(Some(Self::ThisRepository), Self::after).collect()
    }
}

/// The program a spawn site runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// Cargo, through [`crate::issue::cargo`], with what the call declared.
    Cargo(Declared),
    /// Cargo, spawned directly. THE DEFECT: a site with no declaration for a
    /// law to read, and the reason `Command::new` is not how this repository
    /// runs cargo.
    CargoBesideTheDoor(String),
    /// A binary this workspace builds, named through the one spelling cargo
    /// checks at compile time.
    OurBinary(String),
    /// Another program, spelled as a literal — `git`, `bash`, `tar`.
    Named(String),
    /// An expression this reader cannot name a program from, even after
    /// following it one hop. NOT a pass: a spawn whose program is unknown is a
    /// spawn that might be cargo.
    Unplaceable(String),
}

/// How many ways a site's choices may go before [`RustSpawn::variants`] gives
/// up and says so.
///
/// A NUMBER RATHER THAN A TRUNCATION. Sixty-four is more than any site in this
/// repository has and small enough that a report listing them is one a person
/// can read; past it the honest answer is that this reader did not enumerate,
/// because a list cut short reads exactly like a complete one.
const VARIANT_CAP: usize = 64;

/// What one word standing for a LIST OF UNKNOWN LENGTH may hold.
///
/// R1269, and it is the answer to the one thing a hole ruins that reading
/// cannot repair. A hole cannot TAKE a word away, so PRESENCE survives it and
/// only ABSENCE is unclaimable (R1266) — and the two commands this repository
/// issues through one hand over a list built in a loop over cargo's own
/// metadata, which no hop reaches. So the site declares what may be in there
/// ([`crate::issue::runtime_words`]) and the declaration travels with the WORD:
/// a site-level union would be an approximation the moment one way through a
/// site holds a subset of its holes, and this is exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MayHold {
    /// The flags the site declared, and nothing else that starts with `-`. Every
    /// OTHER flag's absence is a claim the words now support.
    OnlyThese(Vec<String>),
    /// The site said nothing, so any flag at all may be in there and no absence
    /// can be claimed. The reading every hole had before R1269, and still the
    /// one an undeclared hole gets.
    Anything,
}

/// One word in a [`Way`] that stands for a list of unknown length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uncounted {
    /// Where in [`Way::words`] it sits.
    pub at: usize,
    /// What the site said may be among the words it stands for.
    pub may_hold: MayHold,
}

/// ONE way a site's choices can go: the words it hands over, and which of them
/// stand for a list this reader could not count.
///
/// The hole is a WORD rather than a gap, and it is rendered the way a shell
/// script's assembled argument is (`$selectors`), because it IS one argument
/// position as far as anything here can tell. What the positions buy is the only
/// two questions a hole changes the answer to: whether the SUBCOMMAND was read
/// before it, and whether a flag's ABSENCE can be claimed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Way {
    /// The words, in order, the hole among them.
    pub words: Vec<String>,
    /// The words in [`Way::words`] that stand for a list of unknown length, each
    /// carrying what its site said it may hold.
    pub uncounted: Vec<Uncounted>,
    /// The positions holding ONE word whose value is decided while the program
    /// runs — `$manifest`, `$subcommand`.
    ///
    /// Kept for one question only: whether the SUBCOMMAND is a word any table
    /// can be keyed on. Everywhere else this repository's reading of a runtime
    /// word is the one [`Word::Runtime`] states — one argument, and not a flag
    /// the site spells — which predates this and is not widened here.
    pub unspelled: Vec<usize>,
}

impl Way {
    /// Was the SUBCOMMAND read as a word a table can be keyed on?
    ///
    /// Two ways it is not. A HOLE before it renders without a leading `-`, so a
    /// reader looking for the first word that is not a flag would take the hole
    /// FOR the subcommand. And the subcommand may itself be decided while the
    /// program runs — `locked_resolution_smoke` loops over every subcommand cargo
    /// has, which is the one site in this repository that does it — and no table
    /// classifies a word nobody spelled.
    #[must_use]
    pub fn subcommand_was_read(&self) -> bool {
        let Some(first) = self.words.iter().position(|word| !word.starts_with('-')) else {
            // No subcommand at all is a reading `lock_verdict` already refuses in
            // its own words, so it is not this method's to hide.
            return true;
        };
        !self.uncounted.iter().any(|hole| hole.at <= first) && !self.unspelled.contains(&first)
    }
}

/// The arm of one choice point a conditional word was added by.
///
/// WORDS ADDED TOGETHER STAY TOGETHER AND WORDS FROM DIFFERENT ARMS NEVER MEET.
/// `if let Some(m) = … { c.arg("--manifest-path").arg(m) }` is two words and ONE
/// decision — a reading that chose them independently would enumerate a command
/// carrying `--manifest-path` with nothing after it, and a `match` whose three
/// arms each add a word would be read as a command that can carry all three at
/// once. Neither is a command anything runs, and a false verdict about one is
/// worse than no verdict at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    /// The choice point, numbered within the file being walked.
    pub choice: usize,
    /// Which of its arms added the word.
    pub arm: usize,
    /// How many arms the choice has.
    pub arms: usize,
    /// Whether taking NONE of them is possible: an `if` with no `else`, a loop
    /// that may run zero times. A `match` is exhaustive and an `if`/`else` is
    /// too, so for those the answer is no — and an arm that adds no words is
    /// already one of the arms rather than an absence.
    pub may_take_none: bool,
}

/// One argument a spawn site hands to the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Word {
    /// A literal: the word itself.
    Spelled(String),
    /// ONE word whose value is decided at runtime, rendered the way a shell
    /// script's assembled path is rendered — `$manifest`, `$root/Cargo.toml`.
    /// The count is what makes it readable: whatever it turns out to be, it is
    /// one argument and it is not a flag this site spells.
    Runtime(String),
    /// An unknown NUMBER of words, from an `.args(expr)` this reader cannot
    /// evaluate. The hole is the count, not the value: a flag may be inside —
    /// unless the site said which ones, which is what the second half carries.
    Unknown(String, MayHold),
    /// A word added on SOME paths through the function and not others — an
    /// `.arg()` inside an `if`, a `match` arm, a loop.
    ///
    /// Reading it as present is how a gate comes to say a command pins its
    /// lockfile when it pins it every other Tuesday; reading it as absent is a
    /// false alarm on a command that is fine. It is neither — and the ARMS that
    /// added it are carried, so [`RustSpawn::variants`] can hand a law every
    /// command the site actually issues instead of one nobody runs.
    Sometimes(String, Vec<Branch>),
}

impl Word {
    /// The word as a command line reads it, for the rendering the laws print.
    #[must_use]
    pub fn rendered(&self) -> String {
        match self {
            Self::Spelled(word) | Self::Runtime(word) | Self::Unknown(word, _) => word.clone(),
            Self::Sometimes(word, _) => format!("[{word}]?"),
        }
    }
}

/// A place in a site's words where an unknown NUMBER of words was handed over,
/// and the parameter they came from when they came from one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    /// Where in [`RustSpawn::words`] it sits.
    pub at: usize,
    /// The enclosing function's parameter the site hands over WHOLE. `None`
    /// when the expression is a local, a field, a transform — anything a call
    /// site's literal does not answer.
    pub parameter: Option<String>,
    /// The expression as the site wrote it, for a report that has to say what it
    /// could not read.
    pub written: String,
}

/// One call of the function a spawn sits in, and the words it hands over.
///
/// THIS IS THE HOP IN THE OTHER DIRECTION. Everywhere else this reader follows a
/// name FORWARDS to what it holds; here the hole is a parameter, and what fills
/// it is written by whoever calls — one literal per call site, each of them a
/// different cargo command through the same door.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerWords {
    /// The tracked path the call is written in.
    pub source: String,
    /// The line the call sits on.
    pub line: usize,
    /// The function that holds the call.
    pub owner: String,
    /// The words handed over, or `None` when the call hands over something this
    /// reader cannot read — a value built two lines earlier, another command's
    /// arguments, a list assembled in a loop.
    pub words: Option<Vec<Word>>,
}

impl CallerWords {
    /// Where it is, for a gate's own output.
    #[must_use]
    pub fn origin(&self) -> String {
        format!("{}:{} `{}`", self.source, self.line, self.owner)
    }
}

/// One place a tracked Rust source spawns a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSpawn {
    /// The tracked path it is written in.
    pub source: String,
    /// The line the spawn sits on.
    pub line: usize,
    /// The function that holds it — what a reader needs to find it, and what a
    /// test needs to call.
    pub owner: String,
    /// What it runs.
    pub program: Program,
    /// What it hands over, in the order the source hands it.
    pub words: Vec<Word>,
    /// The parameters of the function that holds it, in order. Empty when the
    /// spawn is not inside a plain `fn` — a method, an associated function, the
    /// file itself — and a hole in one of those is not followed, because the
    /// calls this reader can enumerate by name are the ones written as
    /// `name(..)`.
    pub parameters: Vec<String>,
    /// Every `.args(..)` whose word COUNT the site does not spell.
    pub holes: Vec<Hole>,
    /// What following those holes back to the call sites found. Empty when
    /// there was nothing to follow, or when [`RustSpawn::unfollowed`] says why
    /// the hop could not be taken at all.
    pub from_callers: Vec<CallerWords>,
    /// Why the holes could not be followed back, when they could not.
    pub unfollowed: Option<String>,
}

impl RustSpawn {
    /// Where it is, for a gate's own output.
    #[must_use]
    pub fn origin(&self) -> String {
        format!("{}:{} `{}`", self.source, self.line, self.owner)
    }

    /// How the command reads back, with the unreadable parts left visible.
    #[must_use]
    pub fn rendered(&self) -> String {
        let mut out = match &self.program {
            Program::Cargo(Declared::ThisRepository) => "cargo".to_string(),
            Program::Cargo(
                Declared::MadeByThisRun(why)
                | Declared::WhereverTheCallerPoints(why)
                | Declared::PinnedWhereverItPoints(why)
                | Declared::PinnedWhenItIsOurs(why),
            ) => format!("cargo [{why}]"),
            Program::Cargo(Declared::Unreadable(written)) => format!("cargo [{written}?]"),
            Program::CargoBesideTheDoor(how)
            | Program::OurBinary(how)
            | Program::Named(how)
            | Program::Unplaceable(how) => how.clone(),
        };
        for word in &self.words {
            out.push(' ');
            out.push_str(&word.rendered());
        }
        out
    }

    /// The words with no hole in them, or `None` when the site hands over a list
    /// this reader cannot count — or one whose membership depends on the path
    /// taken through the function.
    fn complete_words(&self) -> Option<Vec<String>> {
        without_a_hole(&self.words)
    }

    /// How far the hop back to the call sites got, in one line — the SIZE of
    /// what is still unread beside what became readable.
    ///
    /// R1190's rule about reporting a limit's size, applied to a limit that is
    /// now partly gone: three of five call sites reading and two not is a
    /// different fact from none of them reading, and a site that printed only
    /// "carried" said the same thing for both.
    #[must_use]
    pub fn reach(&self) -> String {
        if let Some(why) = &self.unfollowed {
            return format!("not followed back: {why}");
        }
        if self.from_callers.is_empty() {
            return "nothing to follow".to_string();
        }
        let read = self
            .from_callers
            .iter()
            .filter(|caller| caller.words.is_some())
            .count();
        let unread: Vec<String> = self
            .from_callers
            .iter()
            .filter(|caller| caller.words.is_none())
            .map(CallerWords::origin)
            .collect();
        if unread.is_empty() {
            return format!("{read} call site(s) read");
        }
        format!(
            "{read} of {} call site(s) read; the rest hand over words this reader \
             cannot read: {}",
            self.from_callers.len(),
            unread.join(", ")
        )
    }

    /// Every command this site can issue: one word list per way the choices
    /// enclosing its conditional words can go.
    ///
    /// A SITE WITH A CONDITIONAL WORD IS NOT ONE COMMAND. `cargo metadata
    /// [--locked]? [--no-deps]?` is four, and a law that reads it as one has to
    /// choose between calling the flag present (which says a command pins a
    /// lockfile it pins every other Tuesday) and calling it absent (a false
    /// alarm on a command that is fine). Neither is true; all four are.
    ///
    /// A HOLE AMONG THE WORDS IS KEPT AS A WORD AND COUNTED (R1266), because a
    /// hole cannot take a word away: a flag spelled beside one is definitely
    /// there, and only its ABSENCE is a thing the hole can hide. Refusing the
    /// whole site instead was the earlier reading, and it left two commands whose
    /// `--locked` sits in a literal one statement up unread by any law.
    ///
    /// `None` only when the choices multiply past what a report can be read as,
    /// which is a limit this says out loud rather than truncating.
    #[must_use]
    pub fn variants(&self) -> Option<Vec<Way>> {
        ways_of(&self.words)
    }

    /// The ways this site can go that no table can be keyed on, because the
    /// subcommand is behind a hole or is a word nobody spelled.
    #[must_use]
    pub fn ways_no_table_can_key_on(&self) -> usize {
        self.variants().map_or(0, |ways| {
            ways.iter().filter(|way| !way.subcommand_was_read()).count()
        })
    }
}

/// Every way ONE word list can go: one entry per combination of the arms its
/// conditional words depend on.
///
/// A FREE FUNCTION BECAUSE THERE ARE TWO LISTS TO ENUMERATE. A site's own words
/// are one; the same words with a caller's spliced into their hole are another,
/// and a second spelling of this would be one free to enumerate them differently
/// — which is how a command judged through a wrapper comes to be a different
/// command from the one judged at the spawn.
///
/// `None` when the choices multiply past what a report can be read as, which is
/// a limit this says out loud rather than truncating.
#[must_use]
pub fn ways_of(words: &[Word]) -> Option<Vec<Way>> {
    {
        // ONE ENTRY PER CHOICE POINT, holding how many ways it can go. The arms
        // are read off the words rather than counted separately, because a
        // choice no word depends on is a choice this command does not have.
        let mut choices: BTreeMap<usize, (usize, bool)> = BTreeMap::new();
        for word in words {
            if let Word::Sometimes(_, path) = word {
                for branch in path {
                    choices.insert(branch.choice, (branch.arms, branch.may_take_none));
                }
            }
        }
        let ways: usize = choices
            .values()
            .map(|(arms, none)| arms + usize::from(*none))
            .product();
        if ways > VARIANT_CAP {
            return None;
        }
        let numbered: Vec<(usize, usize, bool)> = choices
            .into_iter()
            .map(|(choice, (arms, none))| (choice, arms, none))
            .collect();
        let mut found: Vec<Way> = Vec::new();
        for way in 0..ways {
            // The way is read as a mixed-radix number, one digit per choice.
            // WHERE TAKING NONE IS POSSIBLE IT IS THE FIRST DIGIT, so the
            // plainest command a site can issue is the first one a report
            // prints — `cargo build` before `cargo build --locked`, which is the
            // order a person reads them in anyway.
            let mut rest = way;
            let mut taken: BTreeMap<usize, usize> = BTreeMap::new();
            for (choice, arms, none) in &numbered {
                let options = arms + usize::from(*none);
                let digit = rest % options;
                let arm = if *none {
                    // Digit 0 is "none of them", which no arm answers to.
                    digit.checked_sub(1).unwrap_or(usize::MAX)
                } else {
                    digit
                };
                taken.insert(*choice, arm);
                rest /= options;
            }
            let mut way = Way {
                words: Vec::new(),
                uncounted: Vec::new(),
                unspelled: Vec::new(),
            };
            for word in words {
                match word {
                    Word::Spelled(text) => way.words.push(text.clone()),
                    Word::Runtime(text) => {
                        way.unspelled.push(way.words.len());
                        way.words.push(text.clone());
                    }
                    Word::Sometimes(text, path) => {
                        if path
                            .iter()
                            .all(|branch| taken.get(&branch.choice) == Some(&branch.arm))
                        {
                            way.words.push(text.clone());
                        }
                    }
                    Word::Unknown(text, may_hold) => {
                        way.uncounted.push(Uncounted {
                            at: way.words.len(),
                            may_hold: may_hold.clone(),
                        });
                        way.words.push(text.clone());
                    }
                }
            }
            if !found.contains(&way) {
                found.push(way);
            }
        }
        Some(found)
    }
}

impl RustSpawn {
    /// Every command this site issues, as the population's own type.
    ///
    /// `None` for a site whose words cannot be enumerated, and for one that does
    /// not run cargo through the door — a spawn with no declaration is not a
    /// command any of these laws is about.
    ///
    /// A WAY WHOSE SUBCOMMAND SITS BEHIND A HOLE IS DROPPED, and that is the one
    /// thing a hole ruins outright. The hole renders as `$selectors`, which does
    /// not start with `-`, so a reader looking for the first word that is not a
    /// flag would take the hole FOR the subcommand and every reading after it
    /// would be about a command nobody wrote. Dropped rather than guessed, and
    /// counted by [`RustSpawn::reach`] so the drop is not silent.
    #[must_use]
    pub fn commands(&self) -> Option<Vec<CargoCommand>> {
        let declared = match &self.program {
            Program::Cargo(
                declared @ (Declared::ThisRepository
                | Declared::MadeByThisRun(_)
                | Declared::WhereverTheCallerPoints(_)
                | Declared::PinnedWhereverItPoints(_)
                | Declared::PinnedWhenItIsOurs(_)),
            ) => declared.clone(),
            _ => return None,
        };
        Some(
            self.variants()?
                .into_iter()
                .filter(|way| way.subcommand_was_read())
                .map(|way| {
                    let uncounted = way
                        .uncounted
                        .into_iter()
                        .map(|hole| hole.may_hold)
                        .collect();
                    self.as_command(way.words, uncounted, declared.clone())
                })
                .collect(),
        )
    }

    /// Did EVERY call site of this wrapper write its words down?
    ///
    /// The question the sorting turns on, and it is `all` rather than `any` on
    /// purpose: a site where four callers wrote literals and a fifth handed over
    /// a value it built is still a site this reader cannot finish, and rounding
    /// that off because most of it became readable is how a partly-read
    /// population comes to be reported as a whole one.
    #[must_use]
    pub fn every_call_read(&self) -> bool {
        !self.from_callers.is_empty() && self.from_callers.iter().all(|call| call.words.is_some())
    }

    /// The whole words of the command ONE call site issues through this site:
    /// the site's own, with its hole filled by what that caller wrote.
    ///
    /// `None` when the caller's words were not read. The hole is single by
    /// construction — [`follow_back`] refuses a site with more than one, because
    /// splitting one caller's literals between two parameters is a second
    /// question and this reader answers the first honestly rather than both
    /// vaguely.
    ///
    /// The WORDS come back rather than strings, because the caller's own list may
    /// hold a hole or a conditional word of its own and those are facts about the
    /// command — [`ways_of`] is what turns the merged list into commands.
    #[must_use]
    pub fn words_with(&self, caller: &CallerWords) -> Option<Vec<Word>> {
        let handed = caller.words.as_ref()?;
        let hole = self.holes.first()?;
        let mut filled = self.words.clone();
        filled.splice(hole.at..=hole.at, handed.iter().cloned());
        Some(filled)
    }

    /// This site as one of the population's commands, carrying what it declared.
    ///
    /// THE HOLES ARE A PARAMETER RATHER THAN A FIELD WRITTEN AFTERWARDS. They
    /// were the second until R1269, and a field two callers set and a third
    /// forgot is the half-enforced invariant `CLAUDE.md` names — the more so now
    /// that what a hole may hold decides whether an absence law may answer at
    /// all. The compiler asks every caller instead.
    fn as_command(
        &self,
        words: Vec<String>,
        uncounted: Vec<MayHold>,
        declared: Declared,
    ) -> CargoCommand {
        let mut all = vec!["cargo".to_string()];
        all.extend(words);
        let Some(invocation) = cargo_invocation(&all) else {
            // `cargo_invocation` answers `None` only for words that run no cargo
            // at all, and the first word here IS cargo. Reachable only if that
            // reader changes under this one, which is a thing to hear about
            // rather than to skip.
            panic!(
                "{} spawns cargo and `cargo_invocation` does not see it: {all:?}",
                self.origin()
            );
        };
        CargoCommand {
            source: self.source.clone(),
            owner: self.owner.clone(),
            carrier: invocation.carrier,
            cargo_args: invocation.cargo_args,
            harness_args: invocation.harness_args,
            uncounted,
            env: BTreeMap::new(),
            declared: Some(declared),
            // THE SITE TRAVELS WITH THE DECLARATION AND SURVIVES `as_command_from`
            // (R1277). A command read at a call site is attributed to the CALLER,
            // which is where a person goes to change its words — so `origin` moves
            // and the spawn that declared the arm is left spelled inside a display
            // string. One site reached through five callers then reads as five
            // sites to anything counting origins, which is measured: that is
            // exactly what `item-citations::cargo` does.
            site: Some(self.origin()),
        }
    }

    /// The command ONE call site issues through this site, attributed where a
    /// person would go to change it.
    ///
    /// THE WORDS ARE THE CALLER'S, so the caller's file is the source: putting
    /// `--locked` into one of these means editing the literal at that call site,
    /// and a verdict that named only the wrapper would send its reader to a
    /// function whose words are a parameter. The door is named in the owner
    /// beside it, because the DECLARATION lives there and is the other thing
    /// such a verdict can be about.
    fn as_command_from(
        &self,
        caller: &CallerWords,
        words: Vec<String>,
        uncounted: Vec<MayHold>,
        declared: Declared,
    ) -> CargoCommand {
        let mut command = self.as_command(words, uncounted, declared);
        command.source = caller.source.clone();
        command.owner = if caller.source == self.source {
            format!("{}:{} → {}", caller.owner, caller.line, self.owner)
        } else {
            format!(
                "{}:{} → {}:{} {}",
                caller.owner, caller.line, self.source, self.line, self.owner
            )
        };
        command
    }
}

/// Every place tracked Rust spawns something, sorted into what can be judged and
/// what can only be counted.
///
/// EVERY HALF IS RETURNED, for the reason [`crate::IssuedCommands`] returns both
/// of its own (R1228): a caller that RECEIVES what was not judged says nothing
/// about it only by deciding to, and a caller that was never handed it cannot
/// say anything at all.
#[derive(Debug, Clone, Default)]
pub struct RustSpawns {
    /// Cargo spawns through the door with every argument readable, each carrying
    /// what its site declared about the tree it resolves. A command over a tree
    /// the run made is IN here rather than set aside: the declaration is what
    /// `lock_verdict` needs, and a command held out of the population is one no
    /// law asks anything of.
    pub commands: Vec<CargoCommand>,
    /// Cargo spawns whose every word is readable but whose word LIST depends on
    /// the path taken — a flag inside an `if`, an argument from a `match` arm.
    ///
    /// A THIRD BUCKET RATHER THAN A HOLE, because a hole and a choice are not
    /// the same ignorance: nobody can enumerate the words behind an
    /// `.args(expr)`, and everybody can enumerate the ways an `if` goes.
    /// [`RustSpawn::variants`] hands over one word list per way, so a law that
    /// wants a verdict gets one per command the site issues instead of picking a
    /// path and being right about a third of the time.
    pub conditional: Vec<RustSpawn>,
    /// Cargo spawns through the door handing over a list of unknown length that
    /// no call site of theirs finished either.
    pub carried: Vec<RustSpawn>,
    /// Ways a site can go that no table can be keyed on: the subcommand is
    /// behind a hole, or is a word decided while the program runs.
    ///
    /// DROPPED AND COUNTED IN ONE PLACE, which is where the enumeration happens.
    /// `locked_resolution_smoke` loops over every subcommand cargo has, so its
    /// own measurement fixture is the site this exists for — and a drop nobody
    /// counted is the silence every number in this struct is here to break.
    pub ways_no_table_can_key_on: usize,
    /// Sites whose choices multiply past what a report can be read as, so this
    /// reader did not enumerate them at all.
    pub ways_beyond_a_report: usize,
    /// Call sites whose words this walk READ and which produced no answer at
    /// all: no command, and no count naming what stopped them.
    ///
    /// A CONSERVATION LAW, AND IT EXISTS BECAUSE THIS ROUND BROKE IT. When the
    /// hop backwards learned to read a caller's local binding, two callers
    /// handing over lists WITH HOLES produced nothing while their site stayed
    /// answered by its three readable callers — so two commands left every
    /// bucket and every number at once, which is the exact silence every other
    /// count here was written to break.
    ///
    /// PER CALLER AND NOT PER SITE, which the first draft got wrong and a sweep
    /// said so: a site with three readable callers and two unreadable ones is
    /// "answered" by any per-site reading, and the two commands are gone anyway.
    pub read_but_unanswered: usize,
    /// Commands in [`RustSpawns::commands`] whose words were read at a CALL SITE
    /// rather than beside the spawn.
    ///
    /// COUNTED APART, because "this reader got further" and "this repository
    /// issues more commands" are different facts about the same growing number,
    /// and one total for both says neither.
    pub through_a_wrapper: usize,
    /// Cargo spawns that did not come through the door. The defect.
    pub beside_the_door: Vec<RustSpawn>,
    /// Spawns whose program this reader cannot name.
    pub unplaceable: Vec<RustSpawn>,
    /// Spawns of a binary this workspace builds, placed through
    /// `env!("CARGO_BIN_EXE_…")` and the helpers that hold one.
    ///
    /// COUNTED BECAUSE IT IS WHAT MAKES `unplaceable` MEAN SOMETHING. These are
    /// most of the spawns in this repository, and if the reading of that spelling
    /// broke they would all become unplaceable — a hundred more sites in the pile
    /// nobody is holding to anything, with no number saying so.
    pub our_binaries: usize,
    /// Spawns of another program, named by a literal — `git`, `bash`, `tar`.
    pub other_programs: usize,
    /// WHICH SITES declared each [`Declared`] arm, whatever became of their
    /// words — see [`Declared::arm`], keyed by [`RustSpawn::origin`].
    ///
    /// R1271, AND IT EXISTS BECAUSE A LAW ABOUT AN ARM ASKS ITS COMMANDS. A site
    /// that issues no readable command contributes none, so a site whose words
    /// stop being readable leaves such a law holding over fewer sites with
    /// nothing saying so. Measured: `stale-artifacts::apply` declares
    /// `PinnedWhenItIsOurs` and issued NO judged command at all until this
    /// round, and the law over that arm was passing on two of the three sites
    /// that declare it — a green that meant "two sites are fine", read as "this
    /// repository's sites are fine".
    ///
    /// R1277 MADE IT THE NAMES RATHER THAN A COUNT, because the count is a floor
    /// of its own once more than one arm asks: a site leaving reach and another
    /// arriving is a total that did not move, and the arm most of this
    /// repository's gates declare has ten of them. The names also say WHICH one
    /// went, which is the difference between a law a reader can act on and one
    /// that reports a number changed. The count is `.len()`.
    ///
    /// R1228's shape once more: the set is handed over rather than left for a
    /// caller to reconstruct, because a caller that has to reconstruct it is a
    /// caller that will not.
    pub declaring: BTreeMap<String, BTreeSet<String>>,
    /// Every tracked Rust file this walk parsed — the reach, which an empty
    /// finding list alone can never distinguish from a walk that did not run.
    pub files: usize,
    /// Every spawn seen, cargo or not. The denominator that makes the counts
    /// above mean something.
    pub spawns: usize,
}

impl RustSpawns {
    /// EVERY command these spawns issue — the ones written whole, and every way
    /// a conditional site can go.
    ///
    /// R1268, and it exists because [`RustSpawns::commands`] stopped being that
    /// list the moment R1265 made a conditional site enumerable. A site with a
    /// conditional word IS a set of commands, and those commands lived nowhere
    /// but in the site: the population `commands_this_repository_issues` handed
    /// over held the twelve of them in a shape only a caller that knew to
    /// expand it could read. One law knew — `locked_resolution_smoke` expanded
    /// them itself — and the other two received a population twelve commands
    /// short and said nothing, which is R1228's defect exactly, one bucket
    /// over: a fact one caller reconstructs is a fact the others never have.
    ///
    /// A site this reader cannot enumerate contributes nothing here and is
    /// counted by [`RustSpawns::ways_beyond_a_report`] instead, so the drop is
    /// a number rather than a silence.
    #[must_use]
    pub fn every_command(&self) -> Vec<CargoCommand> {
        let mut all = self.commands.clone();
        for site in &self.conditional {
            if let Some(commands) = site.commands() {
                all.extend(commands);
            }
        }
        all
    }
}

/// Every cargo command tracked Rust sources issue, and what could not be read.
///
/// # Panics
///
/// When a tracked `.rs` file does not parse. That is deliberate and it is the
/// same stance [`crate::parse_workflow`] takes: a file this walk skips is a file
/// whose spawns are invisible, and the skip would be silent.
#[must_use]
pub fn cargo_commands(root: &Path) -> RustSpawns {
    let mut sources = tracked_files(root, &["ls-files", "*.rs"]);
    sources.sort();
    assert!(
        !sources.is_empty(),
        "this repository tracks no Rust source at all — the empty answer that \
         looks like a clean one"
    );

    // TWO PASSES, because a program is often named one hop away: `cli_binary()`
    // and `BIN` are what the spawn says, and what they ARE is a `fn` or a `const`
    // elsewhere in the file — or, for the helpers a whole test suite shares, in
    // another file entirely. A walk that stopped at the spawn would report a
    // hundred sites as unplaceable and drown the handful that really are.
    let mut parsed = Vec::new();
    let mut everywhere: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in sources {
        let text = std::fs::read_to_string(root.join(&path))
            .unwrap_or_else(|why| panic!("read {path}: {why}"));
        let file = syn::parse_file(&text)
            .unwrap_or_else(|why| panic!("{path} does not parse as Rust: {why}"));
        let named = named_values(&file);
        for (name, expression) in &named.functions {
            everywhere
                .entry(name.clone())
                .or_default()
                .insert(rendered(expression));
        }
        parsed.push((path, file, named));
    }
    // A NAME THAT MEANS TWO THINGS MEANS NEITHER. `cli()` resolves the same way
    // in forty test files and differently in none, which is what makes following
    // it safe; a name two files disagree about is dropped from this map rather
    // than resolved to whichever was read first.
    let shared: BTreeMap<String, String> = everywhere
        .into_iter()
        .filter(|(_, spellings)| spellings.len() == 1)
        .map(|(name, spellings)| (name, spellings.into_iter().next().unwrap_or_default()))
        .collect();

    // `env!("CARGO_MANIFEST_DIR")` IS NOT A RUNTIME VALUE TO THIS READER. It is
    // decided by which crate the file belongs to, and that is the deepest
    // tracked manifest above it — so `concat!(env!("CARGO_MANIFEST_DIR"),
    // "/Cargo.toml")` is a manifest path this walk can spell in full, and the
    // three sites that write it stop being three commands nobody can place.
    let manifests = tracked_manifests(root);

    let mut found = RustSpawns::default();
    // THREE PASSES NOW, and the third is the one that goes BACKWARDS. A site
    // whose words are a parameter cannot be finished where it is written; the
    // words are at the call sites, and those are in files this walk may not have
    // reached yet. So every file is walked first and the holes are followed
    // afterwards, over the whole tree at once.
    // A PASS OF ITS OWN, BEFORE THE ONE THAT READS SPAWNS (R1271). A helper is
    // as often written BELOW the function that calls it as above it, and a
    // reading that collected returned lists as it went would answer for one
    // order of the file and not the other. The lists are gathered over the whole
    // tree first, so a call reads the same wherever its helper sits.
    // ONE COMPUTATION OF WHICH CRATE EACH FILE BELONGS TO, read by both passes.
    let directories: BTreeMap<String, String> = parsed
        .iter()
        .map(|(path, _, _)| (path.clone(), manifest_dir_of(path, &manifests)))
        .collect();
    let returned = returned_word_lists(&parsed, &shared, &directories);

    let mut sites = Vec::new();
    let mut calls = Vec::new();
    let mut walked: Vec<(String, syn::File, String)> = Vec::new();
    for (path, file, named) in parsed {
        found.files += 1;
        let manifest_dir = directories.get(&path).cloned().unwrap_or_default();
        let mut walk = Walk::new(&path, &named, &shared, manifest_dir.clone(), &returned);
        walk.visit_file(&file);
        sites.append(&mut walk.sites);
        calls.append(&mut walk.calls);
        walked.push((path, file, manifest_dir));
    }
    follow_every_hole(&mut sites, &walked, calls);

    {
        for site in sites {
            found.spawns += 1;
            let declared = match &site.program {
                Program::Cargo(
                    declared @ (Declared::ThisRepository
                    | Declared::MadeByThisRun(_)
                    | Declared::WhereverTheCallerPoints(_)
                    | Declared::PinnedWhereverItPoints(_)
                    | Declared::PinnedWhenItIsOurs(_)),
                ) => declared.clone(),
                // THE DOOR'S OWN SPAWN NEEDS NO EXCEPTION HERE, which is a
                // finding rather than an omission. It ends in a `Command::new`,
                // and the first draft excused it by name; the program it spawns
                // is that function's own PARAMETER, so once a bare name stopped
                // being resolved through a function of the same name this reader
                // answers `Unplaceable` there. The injection written to prove the
                // excuse mattered came back with nothing red, and the excuse was
                // deleted rather than kept as a clause nothing exercises.
                Program::Cargo(Declared::Unreadable(_)) | Program::CargoBesideTheDoor(_) => {
                    found.beside_the_door.push(site);
                    continue;
                }
                Program::Unplaceable(_) => {
                    found.unplaceable.push(site);
                    continue;
                }
                Program::OurBinary(_) => {
                    found.our_binaries += 1;
                    continue;
                }
                Program::Named(_) => {
                    found.other_programs += 1;
                    continue;
                }
            };
            // NAMED WHERE THE DECLARATION IS READ AND BEFORE ANY SORTING, so
            // the set is about SITES rather than about whichever bucket their
            // words landed in.
            found
                .declaring
                .entry(declared.arm().to_string())
                .or_default()
                .insert(site.origin());
            found.ways_no_table_can_key_on += site.ways_no_table_can_key_on();
            match site.complete_words() {
                Some(words) => found
                    .commands
                    .push(site.as_command(words, Vec::new(), declared)),
                None => {
                    // ONE COMMAND PER WAY PER CALL SITE THAT WROTE ITS WORDS
                    // DOWN, and the site stays carried while any call site did
                    // not. Both halves are the point: the commands are now
                    // judged, and the remainder is still counted rather than
                    // rounded off by the half that became readable.
                    //
                    // THE CALLER'S WORDS GO THROUGH THE SAME ENUMERATION as the
                    // site's own, holes and choices alike — a caller that built
                    // its list with an `extend` of a runtime value hands over a
                    // hole, and reading that as nothing at all is how two
                    // commands came to be read and then judged by nobody.
                    for caller in &site.from_callers {
                        // A CALLER WHOSE WORDS WERE NOT READ IS ALREADY NAMED by
                        // `reach`, which prints it; this loop is about the ones
                        // that WERE read.
                        if caller.words.is_none() {
                            continue;
                        }
                        let mut answered = false;
                        if let Some(words) = site.words_with(caller) {
                            match ways_of(&words) {
                                None => {
                                    found.ways_beyond_a_report += 1;
                                    answered = true;
                                }
                                Some(ways) => {
                                    for way in ways {
                                        if !way.subcommand_was_read() {
                                            found.ways_no_table_can_key_on += 1;
                                            answered = true;
                                            continue;
                                        }
                                        let uncounted = way
                                            .uncounted
                                            .into_iter()
                                            .map(|hole| hole.may_hold)
                                            .collect();
                                        let command = site.as_command_from(
                                            caller,
                                            way.words,
                                            uncounted,
                                            declared.clone(),
                                        );
                                        found.through_a_wrapper += 1;
                                        found.commands.push(command);
                                        answered = true;
                                    }
                                }
                            }
                        }
                        if !answered {
                            found.read_but_unanswered += 1;
                        }
                    }
                    if !site.every_call_read() {
                        // A CHOICE IS NOT A HOLE. What is left after the hop
                        // back is either a word list nobody can count, or
                        // several lists anybody can — and only the first is
                        // beyond a verdict.
                        if site.variants().is_some() {
                            found.conditional.push(site);
                        } else {
                            found.carried.push(site);
                        }
                    }
                }
            }
        }
    }
    found
}

/// Follow every hole this walk left back to the words the call sites wrote.
///
/// The names asked for are the functions holding a hole and nothing else: the
/// definition count is taken over the whole tree because it decides whether a
/// name may be followed at all, but no tree is walked for calls of a name
/// nothing is waiting on.
fn follow_every_hole(
    sites: &mut [RustSpawn],
    walked: &[(String, syn::File, String)],
    calls: Vec<(String, Call)>,
) {
    let wanted: BTreeSet<String> = sites
        .iter()
        .filter(|site| !site.holes.is_empty())
        .map(|site| site.owner.rsplit("::").next().unwrap_or("").to_string())
        .collect();
    if wanted.is_empty() {
        return;
    }
    let other = read_the_other_direction(walked, &wanted, calls);
    for site in sites.iter_mut().filter(|site| !site.holes.is_empty()) {
        follow_back(site, &other);
    }
}

/// The directory cargo would give this file's crate as `CARGO_MANIFEST_DIR`:
/// the deepest tracked manifest above it. Empty when nothing is above it, which
/// is the root workspace's own directory.
fn manifest_dir_of(path: &str, manifests: &[String]) -> String {
    manifests
        .iter()
        .filter_map(|manifest| manifest.strip_suffix("Cargo.toml"))
        .map(|directory| directory.trim_end_matches('/'))
        .filter(|directory| !directory.is_empty() && path.starts_with(&format!("{directory}/")))
        .max_by_key(|directory| directory.len())
        .unwrap_or_default()
        .to_string()
}

/// Every spawn in ONE piece of Rust, read the way [`cargo_commands`] reads it.
///
/// PINNED AGAINST STRINGS IS THE POINT. The branches that matter most here are
/// the ones this repository's tree does not currently contain — a `--locked`
/// added inside an `if`, a helper that resolves to itself, a name that means two
/// things — and a reader whose only test is the tree can only be checked on what
/// the tree happens to hold today. `manifest_dir` is what
/// `env!("CARGO_MANIFEST_DIR")` reads as, which the tree walk derives from the
/// file's own path.
///
/// # Panics
///
/// When the text does not parse as Rust.
#[must_use]
pub fn spawns_in(source: &str, text: &str, manifest_dir: &str) -> Vec<RustSpawn> {
    spawns_across(&[(source, text, manifest_dir)])
}

/// Every spawn in SEVERAL pieces of Rust, read as one tree.
///
/// THE HOP BACKWARDS CROSSES FILES and a one-file reading cannot pin it: a
/// wrapper in a library and the literals its callers write are routinely in
/// different files, and so is the crate whose `CARGO_MANIFEST_DIR` those
/// literals read. Each source is `(tracked path, text, manifest directory)`,
/// which is what the tree walk derives per file and hands the same reader.
///
/// # Panics
///
/// When any of the texts does not parse as Rust.
#[must_use]
pub fn spawns_across(sources: &[(&str, &str, &str)]) -> Vec<RustSpawn> {
    let shared = BTreeMap::new();
    let mut sites = Vec::new();
    let mut calls = Vec::new();
    let mut walked = Vec::new();
    // THE SAME TWO PASSES THE TREE WALK MAKES, so a fixture drives the hop
    // R1271 added rather than only the tree doing so. A helper is as often
    // written below its caller as above it, which is the whole reason the
    // returned lists are gathered before any spawn is read.
    let parsed: Vec<(String, syn::File, Landings)> = sources
        .iter()
        .map(|(source, text, _)| {
            let file = syn::parse_file(text)
                .unwrap_or_else(|why| panic!("{source} does not parse: {why}"));
            let named = named_values(&file);
            ((*source).to_string(), file, named)
        })
        .collect();
    let directories: BTreeMap<String, String> = sources
        .iter()
        .map(|(source, _, manifest_dir)| ((*source).to_string(), (*manifest_dir).to_string()))
        .collect();
    let returned = returned_word_lists(&parsed, &shared, &directories);
    for (source, file, named) in &parsed {
        let manifest_dir = directories.get(source).cloned().unwrap_or_default();
        let mut walk = Walk::new(source, named, &shared, manifest_dir.clone(), &returned);
        walk.visit_file(file);
        sites.append(&mut walk.sites);
        calls.append(&mut walk.calls);
        walked.push((source.clone(), file.clone(), manifest_dir));
    }
    follow_every_hole(&mut sites, &walked, calls);
    sites
}

/// A list a local binding holds, as far as the walk has read it.
///
/// `Poisoned` IS NOT "EMPTY". A binding something did that this reader does not
/// model — `clear`, `retain`, `remove`, an assignment, a hand-off to a function
/// that takes `&mut` — holds words this walk cannot name, and the only sound
/// answer is that its whole list is a hole. Reading the `vec![..]` and ignoring
/// the rest is how a command comes to be reported as carrying words it does not.
#[derive(Debug, Clone)]
enum ListSoFar {
    /// The words so far, and the branch depth the binding was declared at — the
    /// baseline a later `push` inside an `if` is conditional RELATIVE TO.
    Words {
        words: Vec<Word>,
        declared_at: usize,
    },
    /// Something happened to it this reader does not model, and what.
    Poisoned(String),
}

/// One file's walk.
struct Walk<'a> {
    path: &'a str,
    /// What a one-hop follow lands on in THIS file.
    named: &'a Landings,
    /// The names every tracked file agrees about.
    shared: &'a BTreeMap<String, String>,
    /// Every list a tracked plain `fn` hands back — the hop this walk takes when
    /// a binding's initialiser or a `.args(..)` is a CALL rather than a list.
    /// Empty while the returned lists are themselves being read, which is what
    /// stops the reading recursing.
    returned: &'a BTreeMap<String, Result<Vec<Word>, String>>,
    /// What `env!("CARGO_MANIFEST_DIR")` reads as in this file.
    manifest_dir: String,
    /// The function bodies enclosing what is being visited, innermost last.
    owners: Vec<String>,
    sites: Vec<RustSpawn>,
    /// Local bindings holding a `Command`, mapped to the sites they may hold. A
    /// builder written as statements (`let mut c = issue::cargo(..); c.arg(..)`)
    /// is the same command as the same words written as one chain, and a reader
    /// that saw only the chain would report the tidier spelling as argument-free.
    ///
    /// SEVERAL SITES, because a binding can be initialised by a BRANCH:
    /// `let mut spawn = if asked[0] == "cargo" { issue::cargo(..) } else {
    /// Command::new(..) };` holds one of two spawns, and the words added
    /// afterwards go to whichever it turned out to be. Attributing them to both
    /// is what makes each site's word list true of the command it can be; the
    /// first draft attributed them to neither, and the law read the cargo arm as
    /// a command with no subcommand at all.
    commands: BTreeMap<String, Vec<usize>>,
    /// Local bindings holding something a spawn later names as its program.
    values: BTreeMap<String, syn::Expr>,
    /// Local bindings holding a LIST OF WORDS, as it stands at the statement
    /// being visited.
    ///
    /// A list built two lines above the spawn is the commonest way a cargo
    /// command is written in this repository, and reading only the `vec![..]`
    /// would be the false green this module exists to refuse: a `push` after it
    /// adds a word, and a command reported as missing a flag it does add is a
    /// red nobody can act on. So the words accumulate AS THE WALK PASSES THE
    /// STATEMENTS, and a spelling that could take a word away poisons the whole
    /// binding rather than being ignored.
    lists: BTreeMap<String, ListSoFar>,
    /// The parameters of the innermost plain `fn` being walked, in order — empty
    /// inside a method, an associated function, or outside any function at all.
    parameters: Vec<String>,
    /// Names bound by something OTHER than a `let` around what is being visited:
    /// a closure's arguments, a `for` pattern. A hole naming one of these is a
    /// hole naming that binding and not the parameter it shadows, and following
    /// it back to a call site would answer a question nobody asked.
    shadowed: BTreeSet<String>,
    /// How many branches deep the walk currently is inside the function — an
    /// `if`, a `match` arm, a loop body, a closure. Compared against the depth
    /// the site was OPENED at, so a whole command written inside one `if` reads
    /// as unconditional while a flag added by a second one does not.
    depth: usize,
    /// The arms enclosing what is being visited, outermost first — the same
    /// nesting `depth` counts, with WHICH arm of WHICH choice kept so the words
    /// one of them adds can be told from another's.
    branches: Vec<Branch>,
    /// How many choice points this file's walk has met, so the next one gets a
    /// number nothing else has.
    choices: usize,
    /// Every call written `name(..)` this walk passed, with each argument's words
    /// as read HERE — the other half of the hop backwards.
    calls: Vec<(String, Call)>,
    /// The depth each site was opened at.
    opened_at: Vec<usize>,
}

impl<'a> Walk<'a> {
    fn new(
        path: &'a str,
        named: &'a Landings,
        shared: &'a BTreeMap<String, String>,
        manifest_dir: String,
        returned: &'a BTreeMap<String, Result<Vec<Word>, String>>,
    ) -> Self {
        Self {
            path,
            named,
            shared,
            returned,
            manifest_dir,
            owners: Vec::new(),
            sites: Vec::new(),
            commands: BTreeMap::new(),
            values: BTreeMap::new(),
            lists: BTreeMap::new(),
            parameters: Vec::new(),
            shadowed: BTreeSet::new(),
            depth: 0,
            branches: Vec::new(),
            choices: 0,
            calls: Vec::new(),
            opened_at: Vec::new(),
        }
    }

    fn owner(&self) -> String {
        if self.owners.is_empty() {
            "<file>".to_string()
        } else {
            self.owners.join("::")
        }
    }

    /// Walk a function body with its own binding scope. Bindings are per
    /// function: two functions may each hold a `command`, and carrying one scope
    /// across both would file the second one's arguments under the first.
    /// Answers the list this function RETURNS, when it returns one — read while
    /// the function's own scope is still standing, which is the only moment the
    /// binding its tail names still holds anything.
    fn in_function(
        &mut self,
        name: String,
        parameters: Vec<String>,
        body: &syn::Block,
    ) -> Option<Vec<Word>> {
        let commands = std::mem::take(&mut self.commands);
        let values = std::mem::take(&mut self.values);
        let lists = std::mem::take(&mut self.lists);
        let shadowed = std::mem::take(&mut self.shadowed);
        let depth = std::mem::take(&mut self.depth);
        let outer_parameters = std::mem::replace(&mut self.parameters, parameters);
        self.owners.push(name);
        self.visit_block(body);
        let returned = self.list_returned_by(body);
        self.owners.pop();
        self.commands = commands;
        self.values = values;
        self.lists = lists;
        self.shadowed = shadowed;
        self.parameters = outer_parameters;
        self.depth = depth;
        returned
    }

    /// The list this function's TAIL EXPRESSION hands back, when it hands one
    /// back that this walk read.
    ///
    /// A `return` ANYWHERE REFUSES THE READING, and that is the whole of this
    /// reader's caution. A function with two ways out hands back two lists, and
    /// which one a call site gets is a choice between LISTS — [`ways_of`]
    /// enumerates choices within one list and has nothing to say about that. So a
    /// helper that branches its way out is left unread rather than read as
    /// whichever list its tail happens to name, which would be a command nobody
    /// issues reported as one this repository does.
    fn list_returned_by(&self, body: &syn::Block) -> Option<Vec<Word>> {
        if returns_early(body) {
            return None;
        }
        let Some(syn::Stmt::Expr(tail, None)) = body.stmts.last() else {
            return None;
        };
        match self.list_named(tail) {
            Some(Ok(words)) => Some(words),
            // A POISONED BINDING IS NOT A LIST TO HAND ON. Something this walk
            // does not model happened to it, so what it holds afterwards is
            // exactly what a call site must not be told.
            Some(Err(_)) => None,
            None => word_list(&self.manifest_dir, tail),
        }
    }

    /// Walk something a pattern binds names over — a closure body, a `for` body.
    /// The names are the pattern's for the length of it and nobody else's.
    fn under_a_pattern(&mut self, bound: BTreeSet<String>, walk: impl FnOnce(&mut Self)) {
        let outer: Vec<String> = bound
            .iter()
            .filter(|name| self.shadowed.insert((*name).clone()))
            .cloned()
            .collect();
        walk(self);
        for name in outer {
            self.shadowed.remove(&name);
        }
    }

    /// Take a number for a choice point about to be walked.
    fn a_choice(&mut self) -> usize {
        self.choices += 1;
        self.choices - 1
    }

    /// Walk ONE ARM of a choice point — a `then`, an `else`, a `match` arm, a
    /// loop body, a closure body.
    fn in_a_branch(
        &mut self,
        choice: usize,
        arm: usize,
        arms: usize,
        may_take_none: bool,
        walk: impl FnOnce(&mut Self),
    ) {
        self.depth += 1;
        self.branches.push(Branch {
            choice,
            arm,
            arms,
            may_take_none,
        });
        walk(self);
        self.branches.pop();
        self.depth -= 1;
    }

    /// Take one method call on a binding that holds a list of words.
    ///
    /// THREE VOCABULARIES AND THE THIRD ONE IS THE POINT. `push` and `extend`
    /// ADD, and the words they add are read. A handful of spellings READ the list
    /// without changing it, and those are passed over. EVERYTHING ELSE poisons
    /// the binding, naming the method — because a reader that ignored what it did
    /// not recognise would answer about a list the code no longer holds, and this
    /// is the direction where being wrong is silent.
    fn a_method_on_a_list(&mut self, name: &str, call: &syn::ExprMethodCall) {
        let method = call.method.to_string();
        match method.as_str() {
            "push" => {
                let word = call.args.first().map(|first| self.read_word_here(first));
                if let Some(word) = word {
                    self.add_to_list(name, word);
                }
            }
            "extend" => {
                if let Some(first) = call.args.first() {
                    // ONE OF THE TWO PLACES A HOLE IS MADE, and the one the two
                    // commands R1269 is about come through: the site whose
                    // `.args(..)` this list reaches takes it as a PARAMETER, so
                    // the uncountable word is created here, in the caller, and
                    // spliced into that site's hole later by `words_with`. A
                    // declaration attached to the site would never be reached by
                    // either of them.
                    let (handed, may_hold) = self.words_may_hold(first);
                    let handed = handed.clone();
                    match self.words_of(&handed) {
                        Some(words) => {
                            for word in words {
                                self.add_to_list(name, word);
                            }
                        }
                        // AN UNKNOWN NUMBER OF WORDS IS A HOLE IN THE LIST, not
                        // a poisoning: a hole cannot take a word away, so what
                        // was read before it is still read.
                        None => self.add_to_list(
                            name,
                            Word::Unknown(rendered_as_a_value(&handed), may_hold),
                        ),
                    }
                }
            }
            // SPELLINGS THAT ONLY LOOK. `arguments.join(" ")` in an error message
            // is not a change to the command, and poisoning on it would leave
            // this reader unable to read the very shape it was written for.
            "len" | "is_empty" | "iter" | "join" | "contains" | "first" | "last" | "get"
            | "clone" | "as_slice" | "to_vec" => {}
            other => {
                self.lists.insert(
                    name.to_string(),
                    ListSoFar::Poisoned(format!(
                        "`{name}.{other}(..)` is a spelling this reader does not \
                         model, so what the list holds afterwards is unknown"
                    )),
                );
            }
        }
    }

    /// A list handed to somebody who can WRITE to it holds words this walk never
    /// saw, so the binding is poisoned.
    ///
    /// `&mut arguments` is the whole spelling: Rust says what may be written
    /// through, and a reader that took `&arguments` and `&mut arguments` for the
    /// same thing would answer about a list a callee has since appended to. The
    /// direction matters — `&arguments` is a READ and stays readable, which is
    /// how `.args(&arguments)` works at all.
    fn a_list_lent_out(&mut self, expression: &syn::Expr) {
        let syn::Expr::Reference(reference) = expression else {
            return;
        };
        if reference.mutability.is_none() {
            return;
        }
        let syn::Expr::Path(path) = reference.expr.as_ref() else {
            return;
        };
        let Some(name) = path.path.get_ident().map(|name| name.to_string()) else {
            return;
        };
        if self.lists.contains_key(&name) {
            self.lists.insert(
                name.clone(),
                ListSoFar::Poisoned(format!(
                    "`&mut {name}` is handed to something that can write to it, \
                     so what the list holds afterwards is not what this walk read"
                )),
            );
        }
    }

    /// Add one word to a list a binding holds, conditional on the arms entered
    /// since the binding was declared.
    fn add_to_list(&mut self, name: &str, word: Word) {
        let Some(ListSoFar::Words { words, declared_at }) = self.lists.get_mut(name) else {
            return;
        };
        // THE ARMS ENTERED SINCE THE BINDING WAS DECLARED, and an empty list of
        // them is what "certain" means — no second spelling of the same fact. A
        // first draft compared the depths as well, and the injection written to
        // prove that comparison mattered came back with nothing red: a word whose
        // branch path is empty is a word every path reaches, which is exactly
        // what the depths were being asked.
        let path: Vec<Branch> = self.branches.iter().skip(*declared_at).cloned().collect();
        if path.is_empty() {
            words.push(word);
            return;
        }
        words.push(match word {
            // An unknown COUNT stays an unknown count: which hole it is does not
            // get smaller for being conditional, and neither does what its site
            // declared may be in it.
            Word::Unknown(text, may_hold) => Word::Unknown(text, may_hold),
            Word::Spelled(text) | Word::Runtime(text) | Word::Sometimes(text, _) => {
                Word::Sometimes(text, path)
            }
        });
    }

    /// One word, read in the file being walked.
    fn read_word_here(&self, expression: &syn::Expr) -> Word {
        read_word(&self.manifest_dir, expression)
    }

    /// The words an expression hands over: a list written right here, or the one
    /// a tracked function hands back a single call away.
    ///
    /// ONE READER FOR EVERY PLACE A LIST IS TAKEN — the `let` that starts a
    /// binding, the `.args(..)` beside a spawn, the `.extend(..)` that grows a
    /// caller's list. A second spelling would be one free to take the hop in one
    /// of them and not the others, which is how `stale-artifacts::apply` came to
    /// be a site with no readable command while the same shape two files over
    /// was read whole.
    fn words_of(&mut self, expression: &syn::Expr) -> Option<Vec<Word>> {
        if let Some(words) = word_list(&self.manifest_dir, expression) {
            return Some(words);
        }
        self.list_a_call_returns(expression)
    }

    /// The list a CALL hands back, renumbered into this walk's own choices.
    ///
    /// THE CHOICES ARE RENUMBERED AND THAT IS NOT BOOKKEEPING. A `Branch::choice`
    /// is a number within ONE walk, and the returned list was read by another; a
    /// list spliced in with its old numbers would collide with the calling
    /// function's own choices and [`ways_of`] would enumerate two independent
    /// decisions as though taking one arm of the first settled the second. It is
    /// also the right reading on its own terms: the same helper called at two
    /// sites is two decisions, not one.
    fn list_a_call_returns(&mut self, expression: &syn::Expr) -> Option<Vec<Word>> {
        let syn::Expr::Call(call) = through_the_shapes_that_keep_a_list(expression) else {
            return None;
        };
        let name = called_name(call)?;
        let Some(Ok(words)) = self.returned.get(&name) else {
            return None;
        };
        let words = words.clone();
        let mut given: BTreeMap<usize, usize> = BTreeMap::new();
        Some(
            words
                .into_iter()
                .map(|word| match word {
                    Word::Sometimes(text, path) => {
                        let path = path
                            .into_iter()
                            .map(|branch| {
                                let choice = match given.get(&branch.choice) {
                                    Some(mine) => *mine,
                                    None => {
                                        self.choices += 1;
                                        given.insert(branch.choice, self.choices - 1);
                                        self.choices - 1
                                    }
                                };
                                Branch { choice, ..branch }
                            })
                            .collect();
                        Word::Sometimes(text, path)
                    }
                    other => other,
                })
                .collect(),
        )
    }

    /// Look through an [`crate::issue::runtime_words`] wrapper: the expression
    /// whose words it stands for, and what the site said may be among them.
    ///
    /// ONE READER FOR BOTH PLACES A HOLE IS MADE — the `.args(..)` beside a
    /// spawn and the `.extend(..)` of a list a caller is building. A second
    /// spelling of this would be one free to recognise the wrapper in one of
    /// them and not the other, and the two commands this exists for come
    /// through the second.
    ///
    /// The words go on to be read exactly as they would have been unwrapped, so
    /// a site that wraps a list this walk CAN count loses nothing: the
    /// declaration is about the ones it cannot.
    fn words_may_hold<'e>(&self, expression: &'e syn::Expr) -> (&'e syn::Expr, MayHold) {
        match declared_runtime_words(&self.manifest_dir, expression) {
            Some(read) => read,
            None => (expression, MayHold::Anything),
        }
    }

    /// The words one argument hands over: a binding this walk has been reading,
    /// or a list written right there. `None` when it is neither.
    ///
    /// A POISONED BINDING IS `None` HERE rather than a hole. At a spawn a hole is
    /// worth keeping — the words around it are still read — but a CALLER handing
    /// over a list it has lost track of tells the site nothing at all, and
    /// `reach` says so by naming the call site.
    fn words_handed_over(&self, expression: &syn::Expr) -> Option<Vec<Word>> {
        match self.list_named(expression) {
            Some(Ok(words)) => Some(words),
            Some(Err(_)) => None,
            None => word_list(&self.manifest_dir, expression),
        }
    }

    /// The list a `.args(..)` argument names, when it names a binding this walk
    /// has been reading. `Err` carries why a binding it knows cannot be used.
    fn list_named(&self, expression: &syn::Expr) -> Option<Result<Vec<Word>, String>> {
        let syn::Expr::Path(path) = through_the_shapes_that_keep_a_list(expression) else {
            return None;
        };
        let name = path.path.get_ident()?.to_string();
        if self.shadowed.contains(&name) {
            return None;
        }
        match self.lists.get(&name)? {
            ListSoFar::Words { words, .. } => Some(Ok(words.clone())),
            ListSoFar::Poisoned(why) => Some(Err(why.clone())),
        }
    }

    /// Open a site, and answer where it sits.
    fn open(&mut self, program: Program, span: proc_macro2::Span) -> usize {
        let at: LineColumn = span.start();
        self.sites.push(RustSpawn {
            source: self.path.to_string(),
            line: at.line,
            owner: self.owner(),
            program,
            words: Vec::new(),
            parameters: self.parameters.clone(),
            holes: Vec::new(),
            from_callers: Vec::new(),
            unfollowed: None,
        });
        self.opened_at.push(self.depth);
        self.sites.len() - 1
    }

    /// Add one word to a site, saying whether every path through the function
    /// reaches it — and when it does not, WHICH arms decide.
    ///
    /// The arms are the ones the site was not already inside: a whole command
    /// written within one `if` is unconditional as a command, and only a word
    /// added by a choice made AFTER it opened depends on anything.
    fn add(&mut self, site: usize, word: Word) {
        let opened_at = self.opened_at[site];
        // THE ARMS THE SITE IS NOT ALREADY INSIDE, and an empty list of them is
        // what "every path reaches this" means. Comparing the depths as well
        // would be a second spelling of the same fact — see `add_to_list`, where
        // an injection proved it redundant.
        let path: Vec<Branch> = self.branches.iter().skip(opened_at).cloned().collect();
        if path.is_empty() {
            self.sites[site].words.push(word);
            return;
        }
        self.sites[site].words.push(match word {
            Word::Spelled(text) | Word::Runtime(text) => Word::Sometimes(text, path),
            // A WORD THAT WAS ALREADY CONDITIONAL KEEPS ITS OWN ARMS AND GAINS
            // THESE. A `push` inside one `if` handed to `.args()` inside another
            // depends on BOTH, and replacing the first path with the second
            // would enumerate a command carrying a word its own branch never
            // reached. A choice in both paths is necessarily the same arm — the
            // branch stack is a stack — so the union is by choice.
            Word::Sometimes(text, own) => {
                let mut all = own;
                for branch in path {
                    if !all.iter().any(|had| had.choice == branch.choice) {
                        all.push(branch);
                    }
                }
                Word::Sometimes(text, all)
            }
            // An unknown COUNT stays an unknown count: which hole it is does not
            // get smaller for being conditional, and neither does what its site
            // declared may be in it.
            Word::Unknown(text, may_hold) => Word::Unknown(text, may_hold),
        });
    }

    /// The sites a chain of method calls can belong to, taking its words along
    /// the way. Empty when the chain is rooted at anything but a spawn; more than
    /// one when a branch decided which spawn the value is.
    ///
    /// IT WALKS WHAT IT DOES NOT CLAIM. Every caller hands an expression here and
    /// none of them walks it afterwards, because a walk that happened twice would
    /// open the same spawn twice — so this is where the recursion lives, and the
    /// arms below either take an expression over or pass it to the default walk.
    fn root(&mut self, expression: &syn::Expr) -> Vec<usize> {
        match expression {
            syn::Expr::MethodCall(call) => {
                // A METHOD ON A LIST-HOLDING BINDING IS READ BEFORE THE CHAIN IS,
                // because `arguments.push("--locked")` is not a spawn and the
                // words it adds belong to a command written further down.
                if let syn::Expr::Path(path) = through_the_shapes_that_keep_a_list(&call.receiver) {
                    if let Some(name) = path.path.get_ident().map(|name| name.to_string()) {
                        if self.lists.contains_key(&name) && !self.shadowed.contains(&name) {
                            self.a_method_on_a_list(&name, call);
                            for argument in &call.args {
                                self.visit_expr(argument);
                            }
                            return Vec::new();
                        }
                    }
                }
                let sites = self.root(&call.receiver);
                if sites.is_empty() {
                    for argument in &call.args {
                        self.visit_expr(argument);
                    }
                } else {
                    for site in &sites {
                        self.take(call, *site);
                    }
                }
                sites
            }
            syn::Expr::Call(call) => {
                if is_command_new(call) {
                    let Some(first) = call.args.first() else {
                        return Vec::new();
                    };
                    let program = self.place(first);
                    return vec![self.open(program, call.span())];
                }
                if is_the_door(call) {
                    let declared = declaration(call);
                    return vec![self.open(Program::Cargo(declared), call.span())];
                }
                // RECORDED HERE AND NOWHERE ELSE, because the words a caller
                // hands over are read by the walk that is standing in its file
                // with its bindings in hand.
                if let Some(name) = called_name(call) {
                    let at: LineColumn = call.span().start();
                    let arguments = call
                        .args
                        .iter()
                        .map(|argument| self.words_handed_over(argument))
                        .collect();
                    self.calls.push((
                        name,
                        Call {
                            source: self.path.to_string(),
                            line: at.line,
                            owner: self.owner(),
                            arguments,
                        },
                    ));
                }
                self.visit_expr(&call.func);
                for argument in &call.args {
                    self.a_list_lent_out(argument);
                    self.visit_expr(argument);
                }
                Vec::new()
            }
            // A binding: `let mut command = issue::cargo(..); command.arg(..)`.
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|name| self.commands.get(&name.to_string()).cloned())
                .unwrap_or_default(),
            // A BRANCH CHOOSING THE SPAWN, which is how a program that runs
            // `cargo` for one word and something else for another is written.
            // Both arms are walked, and the words that follow belong to whichever
            // it was.
            syn::Expr::If(node) => {
                self.visit_expr(&node.cond);
                let choice = self.a_choice();
                // AN `if` WITH NO `else` CAN TAKE NEITHER ARM, and one with an
                // `else` always takes exactly one. That is the whole difference
                // between two possible commands and three.
                let has_else = node.else_branch.is_some();
                let arms = if has_else { 2 } else { 1 };
                let mut sites = Vec::new();
                self.in_a_branch(choice, 0, arms, !has_else, |walk| {
                    sites.extend(walk.root_of_block(&node.then_branch));
                });
                if let Some((_, otherwise)) = &node.else_branch {
                    self.in_a_branch(choice, 1, arms, false, |walk| {
                        sites.extend(walk.root(otherwise));
                    });
                }
                sites
            }
            syn::Expr::Match(node) => {
                self.visit_expr(&node.expr);
                let choice = self.a_choice();
                let arms = node.arms.len();
                let mut sites = Vec::new();
                for (at, arm) in node.arms.iter().enumerate() {
                    // A `match` IS EXHAUSTIVE, so "none of them" is not one of
                    // the ways this can go; an arm that adds no words is already
                    // an arm rather than an absence.
                    self.in_a_branch(choice, at, arms, false, |walk| {
                        sites.extend(walk.root(&arm.body));
                    });
                }
                sites
            }
            syn::Expr::Block(node) => self.root_of_block(&node.block),
            // Spellings that wrap the chain without changing what it is.
            syn::Expr::Try(inner) => self.root(&inner.expr),
            syn::Expr::Paren(inner) => self.root(&inner.expr),
            syn::Expr::Group(inner) => self.root(&inner.expr),
            syn::Expr::Reference(inner) => self.root(&inner.expr),
            syn::Expr::Await(inner) => self.root(&inner.base),
            // NOT A SPAWN, so the default walk takes it — and that walk comes
            // back through `visit_expr`, which is this function again.
            other => {
                syn::visit::visit_expr(self, other);
                Vec::new()
            }
        }
    }

    /// A block's value is its tail expression; everything before it is walked as
    /// itself.
    fn root_of_block(&mut self, block: &syn::Block) -> Vec<usize> {
        let mut sites = Vec::new();
        for (at, statement) in block.stmts.iter().enumerate() {
            match statement {
                syn::Stmt::Expr(tail, None) if at + 1 == block.stmts.len() => {
                    sites = self.root(tail);
                }
                other => self.visit_stmt(other),
            }
        }
        sites
    }

    /// Take one method call's contribution to a site's words.
    fn take(&mut self, call: &syn::ExprMethodCall, site: usize) {
        match call.method.to_string().as_str() {
            "arg" => {
                if let Some(first) = call.args.first() {
                    let word = read_word(&self.manifest_dir, first);
                    self.add(site, word);
                }
            }
            "args" => {
                if let Some(first) = call.args.first() {
                    // THE OTHER PLACE A HOLE IS MADE. Read before anything else,
                    // because the wrapper is a CALL and none of the readings
                    // below look through one — a walk that asked them first
                    // would answer about `runtime_words(..)` rather than about
                    // the words inside it.
                    let (first, may_hold) = self.words_may_hold(first);
                    // A BINDING THIS WALK HAS BEEN READING ANSWERS FIRST, and its
                    // words are the list AS IT STANDS HERE: mutations above this
                    // statement are in it and mutations below are not, which is
                    // what `Command::args` does with the slice it is handed.
                    match self.list_named(first) {
                        Some(Ok(words)) => {
                            for word in words {
                                self.add(site, word);
                            }
                            for argument in &call.args {
                                syn::visit::visit_expr(self, argument);
                            }
                            return;
                        }
                        Some(Err(why)) => {
                            let hole = Hole {
                                at: self.sites[site].words.len(),
                                parameter: None,
                                written: format!("{} — {why}", rendered_as_a_value(first)),
                            };
                            self.sites[site].holes.push(hole);
                            self.add(site, Word::Unknown(rendered_as_a_value(first), may_hold));
                            for argument in &call.args {
                                syn::visit::visit_expr(self, argument);
                            }
                            return;
                        }
                        None => {}
                    }
                    // AN ARRAY'S LENGTH IS KNOWN even when its elements are not:
                    // `["run", "--manifest-path", path]` is three words, one of
                    // them decided at runtime, and reading it as a hole would
                    // throw away the two flags that are right there.
                    let first = first.clone();
                    match self.words_of(&first) {
                        Some(words) => {
                            for word in words {
                                self.add(site, word);
                            }
                        }
                        None => {
                            let hole = Hole {
                                at: self.sites[site].words.len(),
                                parameter: self.parameter_named(&first),
                                written: rendered_as_a_value(&first),
                            };
                            self.sites[site].holes.push(hole);
                            self.add(site, Word::Unknown(rendered_as_a_value(&first), may_hold));
                        }
                    }
                }
            }
            _ => {}
        }
        // A spawn nested inside an argument is still a spawn, and skipping the
        // arguments is how a walk that handles chains loses the ones written
        // inside them.
        for argument in &call.args {
            syn::visit::visit_expr(self, argument);
        }
    }

    /// The parameter a `.args(..)` argument hands over WHOLE, when it hands over
    /// one.
    ///
    /// The name has to still mean the parameter where the spawn is written: a
    /// `let` of the same name, or a closure or loop binding around it, makes the
    /// words somebody else's, and a call site's literal would then answer a
    /// question nobody asked.
    fn parameter_named(&self, expression: &syn::Expr) -> Option<String> {
        let syn::Expr::Path(path) = through_the_shapes_that_keep_a_list(expression) else {
            return None;
        };
        let name = path.path.get_ident()?.to_string();
        (self.parameters.contains(&name)
            && !self.values.contains_key(&name)
            && !self.shadowed.contains(&name))
        .then_some(name)
    }

    /// What program an expression names, following it one hop when it names a
    /// value rather than a program.
    fn place(&self, expression: &syn::Expr) -> Program {
        self.place_within(expression, &mut BTreeSet::new())
    }

    fn place_within(&self, expression: &syn::Expr, seen: &mut BTreeSet<String>) -> Program {
        let direct = read_program(expression);
        if !matches!(direct, Program::Unplaceable(_)) {
            return direct;
        }
        let Some(named) = names_a_value(expression) else {
            return direct;
        };
        // A CYCLE IS A HOP THAT NEVER LANDS. `fn cargo() -> String { cargo() }`
        // is not a program this repository has, and a walk that met one would
        // hang instead of reporting.
        if !seen.insert(named.name.clone()) {
            return direct;
        }
        if let Some(local) = self.values.get(&named.name) {
            return self.place_within(&local.clone(), seen);
        }
        let landings = if named.called {
            &self.named.functions
        } else {
            &self.named.values
        };
        if let Some(defined) = landings.get(&named.name) {
            return self.place_within(&defined.clone(), seen);
        }
        // ACROSS FILES ONLY FOR A CALL, and this is a correction the first run
        // made rather than a caution. A CALL names a function, and a function's
        // name is the same fact in every file that says it. A BARE NAME is a
        // local or a parameter, whose meaning is the enclosing signature's — and
        // `Command::new(target)` in `mnemosyne-config` resolved, through a
        // repository-wide map, to a `fn target()` in a test file three
        // directories away, reporting a spawn as placed that nothing had placed.
        if named.called {
            if let Some(text) = self.shared.get(&named.name) {
                if let Ok(parsed) = syn::parse_str::<syn::Expr>(text) {
                    return self.place_within(&parsed, seen);
                }
            }
        }
        direct
    }
}

impl<'ast> Visit<'ast> for Walk<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.in_function(
            item.sig.ident.to_string(),
            parameters_of(&item.sig),
            &item.block,
        );
    }

    // A METHOD'S PARAMETERS ARE NOT FOLLOWED, and the empty list here is that
    // decision rather than an omission. The calls this reader enumerates are the
    // ones written `name(..)`; `x.name(..)` is a method call whose receiver
    // decides which `name`, and no reading of the syntax says which type `x` is.
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.in_function(item.sig.ident.to_string(), Vec::new(), &item.block);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if let Some(body) = &item.default {
            self.in_function(item.sig.ident.to_string(), Vec::new(), body);
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let Some(init) = &local.init else {
            return;
        };
        let sites = self.root(&init.expr);
        if let Some(name) = binding_name(&local.pat) {
            if sites.is_empty() {
                // A BINDING IS DECLARED ONCE AND READ AFTERWARDS, so a second
                // `let` of the same name starts a new list rather than adding to
                // the old one — and a binding whose initialiser is not a list
                // this reader can spell is poisoned rather than left holding
                // whatever the previous one held.
                // AND A CALL IS A LIST WHEN THE FUNCTION IT NAMES HANDS ONE BACK
                // (R1271). `let arguments = clean_arguments(freshen, package);`
                // is the whole of `stale-artifacts::apply`'s command, and before
                // this hop the binding was removed here and the `.args(..)` two
                // lines down became a hole no law could ask anything of.
                let initialiser = (*init.expr).clone();
                match self.words_of(&initialiser) {
                    Some(words) => {
                        self.lists.insert(
                            name.clone(),
                            ListSoFar::Words {
                                words,
                                declared_at: self.depth,
                            },
                        );
                    }
                    None => {
                        self.lists.remove(&name);
                    }
                }
                self.values.insert(name, (*init.expr).clone());
            } else {
                self.commands.insert(name, sites);
            }
        }
        if let Some((_, diverge)) = &init.diverge {
            self.visit_expr(diverge);
        }
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        let _ = self.root(expression);
    }

    // --- what only happens on some paths ------------------------------------
    //
    // THE CONDITION IS NOT THE BRANCH. `if command.arg("x").status().is_ok()`
    // runs that argument every time, so a condition is walked at the depth the
    // `if` sits at and only the arms are deeper. `if` and `match` are handled in
    // `root`, which has to look at their arms anyway — a value can BE a spawn
    // chosen by a branch — and a second spelling of the rule here would be one
    // free to disagree with that one.

    // A LOOP OR A CLOSURE IS ONE ARM THAT MAY NOT BE TAKEN. A body that runs
    // zero times adds nothing, and one that runs many times adds its words more
    // than once — which is a shape no law here asks about, so the reading is the
    // safe half: the words are there or they are not.

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.visit_expr(&node.cond);
        let choice = self.a_choice();
        self.in_a_branch(choice, 0, 1, true, |walk| walk.visit_block(&node.body));
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.visit_expr(&node.expr);
        let bound = names_bound_by(&node.pat);
        let choice = self.a_choice();
        self.in_a_branch(choice, 0, 1, true, |walk| {
            walk.under_a_pattern(bound, |walk| walk.visit_block(&node.body));
        });
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        let choice = self.a_choice();
        self.in_a_branch(choice, 0, 1, true, |walk| walk.visit_block(&node.body));
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        let bound = node.inputs.iter().flat_map(names_bound_by).collect();
        let choice = self.a_choice();
        self.in_a_branch(choice, 0, 1, true, |walk| {
            walk.under_a_pattern(bound, |walk| walk.visit_expr(&node.body));
        });
    }
}

/// What a one-hop follow can land on in one file.
///
/// THE TWO KINDS ARE KEPT APART, and the first run is why. A bare `program` is a
/// local or a PARAMETER, and resolving it through the functions of its own file
/// found `fn program()` — so the one site that IS the door read as a second one.
/// A call names a function; a bare name does not.
#[derive(Default)]
struct Landings {
    /// `const` and `static` values, which a bare name can be.
    values: BTreeMap<String, syn::Expr>,
    /// `fn` tail expressions, which only a CALL can be.
    functions: BTreeMap<String, syn::Expr>,
}

/// The `fn` tail expressions and `const` values a file defines, by name.
fn named_values(file: &syn::File) -> Landings {
    struct Names(Landings);
    impl<'ast> Visit<'ast> for Names {
        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            if let Some(syn::Stmt::Expr(tail, None)) = item.block.stmts.last() {
                self.0
                    .functions
                    .insert(item.sig.ident.to_string(), tail.clone());
            }
            syn::visit::visit_item_fn(self, item);
        }
        fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
            self.0
                .values
                .insert(item.ident.to_string(), (*item.expr).clone());
        }
        fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
            self.0
                .values
                .insert(item.ident.to_string(), (*item.expr).clone());
        }
    }
    let mut names = Names(Landings::default());
    names.visit_file(file);
    names.0
}

// --- the hop in the other direction -----------------------------------------

/// One call written `name(..)`, kept so a hole in a spawn's words can be
/// followed back to the words its caller wrote.
/// One call written `name(..)`, RECORDED BY THE WALK THAT PASSED IT.
///
/// The words are read where the call is written, by the walk that was already
/// reading that file — which is what lets a caller hand over a list it built two
/// lines earlier. A second visitor over the same files could see the call and
/// not the binding, and that is not a caution: `item-citations` writes
/// `let mut argv = vec![.., "--locked", ..]; argv.extend(..); cargo(root, &argv)`
/// at two of its five call sites, and a reader of the call alone reports the
/// flag as unreadable while it sits in a literal one statement up.
struct Call {
    source: String,
    line: usize,
    owner: String,
    /// Each argument's words as this walk read them where they are written, or
    /// `None` for an argument that is not a list it can read.
    arguments: Vec<Option<Vec<Word>>>,
}

/// What the hop backwards needs to know about the tree it is taken in.
#[derive(Default)]
struct TheOtherDirection {
    /// How many tracked functions answer to each name and argument count. More
    /// than one, and a call of that name names none of them in particular — the
    /// same rule the forward hop draws for a value two files disagree about,
    /// pointed the other way.
    definitions: BTreeMap<(String, usize), usize>,
    /// Every call written `name(..)`, by name.
    calls: BTreeMap<String, Vec<Call>>,
    /// Calls of a wanted name written INSIDE A MACRO'S TOKENS, as (path, line).
    ///
    /// THE ONE WAY THIS HOP COULD REPORT A PARTLY-READ SITE AS FINISHED. `syn`
    /// hands a macro invocation over as tokens and does not parse them, so
    /// `assert!(run(&["build"]).is_ok())` is a call of `run` that the walk above
    /// cannot see — and a wrapper whose other callers all wrote literals would
    /// then read as one whose every call site was read. These are counted as
    /// call sites whose words are unreadable, which is what they are.
    in_macros: BTreeMap<String, Vec<(String, usize)>>,
}

/// Read a tree's function definitions and the calls of the names asked for.
///
/// THE DEFINITIONS ARE COUNTED OVER EVERYTHING and the calls only over what was
/// asked, because the two answer different questions: the count decides whether
/// a name may be followed at all, and it is wrong the moment it is taken over a
/// subset.
fn read_the_other_direction(
    files: &[(String, syn::File, String)],
    wanted: &BTreeSet<String>,
    seen: Vec<(String, Call)>,
) -> TheOtherDirection {
    let mut found = TheOtherDirection::default();
    for (name, call) in seen {
        if wanted.contains(&name) {
            found.calls.entry(name).or_default().push(call);
        }
    }
    // THE FILES ARE READ FOR TWO THINGS ONLY NOW — how many functions answer to
    // each name, and which calls hide inside macro tokens. The WORDS come from
    // the walk, which is the whole of R1266's change here: a second visitor over
    // these files could see a call and not the binding one statement above it.
    for (path, file, _) in files {
        struct Definitions<'a>(&'a mut BTreeMap<(String, usize), usize>);
        impl<'ast> Visit<'ast> for Definitions<'_> {
            fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
                *self
                    .0
                    .entry((item.sig.ident.to_string(), item.sig.inputs.len()))
                    .or_default() += 1;
                syn::visit::visit_item_fn(self, item);
            }
            fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
                *self
                    .0
                    .entry((item.sig.ident.to_string(), item.sig.inputs.len()))
                    .or_default() += 1;
                syn::visit::visit_impl_item_fn(self, item);
            }
            fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
                *self
                    .0
                    .entry((item.sig.ident.to_string(), item.sig.inputs.len()))
                    .or_default() += 1;
                syn::visit::visit_trait_item_fn(self, item);
            }
        }
        Definitions(&mut found.definitions).visit_file(file);

        struct InMacros<'a> {
            wanted: &'a BTreeSet<String>,
            found: Vec<(String, usize)>,
        }
        impl<'ast> Visit<'ast> for InMacros<'_> {
            fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
                calls_inside(invocation.tokens.clone(), self.wanted, &mut self.found);
                syn::visit::visit_macro(self, invocation);
            }
        }
        let mut in_macros = InMacros {
            wanted,
            found: Vec::new(),
        };
        in_macros.visit_file(file);
        for (name, line) in in_macros.found {
            found
                .in_macros
                .entry(name)
                .or_default()
                .push((path.to_string(), line));
        }
    }
    found
}

/// Every `name(..)` written inside a macro's tokens, for the names asked about.
///
/// TOKENS, NOT SYNTAX, because that is all a macro invocation is until it is
/// expanded — and expanding it is a different program. So the shape is matched:
/// an identifier this reader is looking for, not preceded by a `.`, followed by
/// a parenthesised group of the right number of arguments. Groups are walked
/// into, because `assert!(v.contains(&run(&["a"])))` puts the call two
/// delimiters deep.
///
/// Over-matching here is SAFE and under-matching is not: a call this finds
/// becomes a call site whose words are unreadable, which leaves its site
/// carried; one it misses is a command nobody counted.
fn calls_inside(
    tokens: proc_macro2::TokenStream,
    wanted: &BTreeSet<String>,
    into: &mut Vec<(String, usize)>,
) {
    let mut previous: Option<proc_macro2::Ident> = None;
    let mut after_a_dot = false;
    for token in tokens {
        match token {
            proc_macro2::TokenTree::Ident(ident) => {
                previous = (!after_a_dot).then_some(ident);
                after_a_dot = false;
            }
            proc_macro2::TokenTree::Group(group) => {
                if group.delimiter() == proc_macro2::Delimiter::Parenthesis {
                    if let Some(ident) = previous.take() {
                        let name = ident.to_string();
                        if wanted.contains(&name) {
                            into.push((name, ident.span().start().line));
                        }
                    }
                }
                calls_inside(group.stream(), wanted, into);
                previous = None;
                after_a_dot = false;
            }
            proc_macro2::TokenTree::Punct(punct) => {
                previous = None;
                after_a_dot = punct.as_char() == '.';
            }
            proc_macro2::TokenTree::Literal(_) => {
                previous = None;
                after_a_dot = false;
            }
        }
    }
}

/// The function a call names, when the call names one this reader can follow.
///
/// A TYPE BEFORE THE NAME MEANS THE RECEIVER DECIDES. `Command::new(..)` and
/// `Path::new(..)` are not calls of any `fn new` written elsewhere in this
/// repository, and a reader that matched the last segment alone would file every
/// one of them under whichever `fn new` happened to be unique — words handed to
/// a function that never took them.
fn called_name(call: &syn::ExprCall) -> Option<String> {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let (last, before) = segments.split_last()?;
    if let Some(qualifier) = before.last() {
        if qualifier.chars().next().is_some_and(char::is_uppercase) {
            return None;
        }
    }
    Some(last.clone())
}

/// Fill a site's hole from the call sites of the function it sits in, or say why
/// that hop cannot be taken.
///
/// EVERY REFUSAL IS NAMED AND NONE OF THEM IS SILENT. A site this returns
/// without callers stays exactly as carried as it was, and the sentence it
/// carries is what distinguishes "nobody calls it" from "its name means two
/// things" from "the words are not a parameter at all" — three different pieces
/// of work, and a report that said "carried" for all three would name none.
fn follow_back(site: &mut RustSpawn, other: &TheOtherDirection) {
    let mut refuse = |why: String| site.unfollowed = Some(why);
    if site.holes.len() > 1 {
        refuse(format!(
            "{} lists this reader cannot count, and splitting one call site's \
             literals between them is a second question",
            site.holes.len()
        ));
        return;
    }
    let Some(hole) = site.holes.first() else {
        return;
    };
    if let Some(sometimes) = site
        .words
        .iter()
        .find(|word| matches!(word, Word::Sometimes(..)))
    {
        refuse(format!(
            "`{}` is added on some paths through the function only, which no \
             caller's literal answers",
            sometimes.rendered()
        ));
        return;
    }
    let Some(parameter) = hole.parameter.clone() else {
        refuse(format!(
            "the words handed over are `{}`, which is not a parameter of \
             `{}` this reader can follow",
            hole.written, site.owner
        ));
        return;
    };
    let Some(at) = site.parameters.iter().position(|name| *name == parameter) else {
        refuse(format!(
            "`{parameter}` is not among the parameters of `{}`",
            site.owner
        ));
        return;
    };
    let name = site
        .owner
        .rsplit("::")
        .next()
        .unwrap_or(&site.owner)
        .to_string();
    let arity = site.parameters.len();
    match other.definitions.get(&(name.clone(), arity)).copied() {
        Some(1) => {}
        other_count => {
            refuse(format!(
                "`{name}` is the name of {} function(s) of {arity} argument(s) in \
                 this repository, so a call of it names none of them in particular",
                other_count.unwrap_or(0)
            ));
            return;
        }
    }
    let calls: Vec<&Call> = other
        .calls
        .get(&name)
        .map(|calls| {
            calls
                .iter()
                .filter(|call| call.arguments.len() == arity)
                .collect()
        })
        .unwrap_or_default();
    // A CALL INSIDE A MACRO IS A CALL SITE THIS READER CANNOT READ, and it is
    // listed rather than left out. Leaving it out is the one way this hop could
    // report a partly-read site as finished: `syn` hands macro invocations over
    // as tokens, so `assert!(run(&["build"]).is_ok())` is invisible to the walk,
    // and a wrapper whose other callers all wrote literals would read as one
    // every call site of which was read.
    let in_a_macro = other.in_macros.get(&name).map_or(&[][..], Vec::as_slice);
    if calls.is_empty() && in_a_macro.is_empty() {
        refuse(format!(
            "nothing in this repository calls `{name}` with {arity} argument(s)"
        ));
        return;
    }
    site.from_callers = calls
        .into_iter()
        .map(|call| CallerWords {
            source: call.source.clone(),
            line: call.line,
            owner: call.owner.clone(),
            words: call.arguments.get(at).cloned().flatten(),
        })
        .chain(in_a_macro.iter().map(|(source, line)| CallerWords {
            source: source.clone(),
            line: *line,
            owner: "a call inside a macro invocation".to_string(),
            words: None,
        }))
        .collect();
}

/// Does this body leave by any route other than falling off its end?
///
/// A `return` inside a CLOSURE leaves the closure and not the function, so
/// reading one as a second way out of the function is wrong — and reading it as
/// harmless is the direction that hands a caller a list it may never get. This
/// answers the loud way for the same reason every other refusal here does.
fn returns_early(body: &syn::Block) -> bool {
    struct AnyReturn(bool);
    impl<'ast> syn::visit::Visit<'ast> for AnyReturn {
        fn visit_expr_return(&mut self, _: &'ast syn::ExprReturn) {
            self.0 = true;
        }
    }
    let mut found = AnyReturn(false);
    syn::visit::visit_block(&mut found, body);
    found.0
}

/// Every list a tracked plain `fn` hands back, by the name a call site writes.
///
/// R1271, AND THE TECHNIQUE IS ALREADY IN THIS REPOSITORY TWICE — R1266 named
/// this as undone work rather than as a limit, and `named_environment::
/// returned_lists` follows a function's returned list for its own question. What
/// this reader could not do was read `stale-artifacts::apply`, whose whole
/// command is `clean_arguments(freshen, package)`: a `vec![..]` and a
/// conditional push, one call away, in a shape R1266 already reads when it is
/// written beside the spawn.
///
/// A NAME THAT MEANS TWO FUNCTIONS MEANS NEITHER (R1263's rule, and the reason
/// this is a whole-tree pass rather than a per-file one). The `Err` carries why,
/// so a site that hands over such a call is a hole with a reason rather than a
/// silence.
///
/// METHODS ARE NOT HERE. `visit_item_fn` sees free functions, which are the ones
/// a call site writes as `name(..)` — the same boundary [`read_the_other_direction`]
/// draws for the hop in the other direction, and for the same reason.
fn returned_word_lists(
    parsed: &[(String, syn::File, Landings)],
    shared: &BTreeMap<String, String>,
    directories: &BTreeMap<String, String>,
) -> BTreeMap<String, Result<Vec<Word>, String>> {
    let mut found: BTreeMap<String, Result<Vec<Word>, String>> = BTreeMap::new();
    // NOTHING TO HOP THROUGH WHILE THE HOPS ARE BEING READ. A helper whose own
    // list came from a second helper is a chain, and a reader that followed one
    // would have to decide what to do about a cycle; one hop is what R1266 named
    // and one hop is what this is. The bound is here rather than in a comment
    // somewhere: the walk that reads a returned list is handed no returned lists.
    let no_further_hop = BTreeMap::new();
    for (path, file, named) in parsed {
        let manifest_dir = directories.get(path).cloned().unwrap_or_default();
        for item in &file.items {
            let syn::Item::Fn(function) = item else {
                continue;
            };
            let name = function.sig.ident.to_string();
            let mut walk = Walk::new(path, named, shared, manifest_dir.clone(), &no_further_hop);
            let Some(words) =
                walk.in_function(name.clone(), parameters_of(&function.sig), &function.block)
            else {
                continue;
            };
            match found.entry(name.clone()) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(Ok(words));
                }
                std::collections::btree_map::Entry::Occupied(mut slot) => {
                    *slot.get_mut() = Err(format!(
                        "`{name}` names a list-returning function in more than one \
                         tracked file, so a call of it names no one list"
                    ));
                }
            }
        }
    }
    found
}

/// Is this call `Command::new(..)`, however the path is written?
fn is_command_new(call: &syn::ExprCall) -> bool {
    call.args.len() == 1 && ends_with(call, &["Command", "new"])
}

/// Is this call the one door — `issue::cargo(..)` or `issue::named_cargo(..)`?
///
/// TWO SPELLINGS, AND THE SECOND CLOSES A HOLE THE FIRST RUN LEFT. Reading only
/// the qualified path means `use ci_plan::issue::cargo;` followed by
/// `cargo(Tree::ThisRepository)` is a cargo command no law can see — invisible
/// rather than refused, which is the one direction this whole module exists to
/// rule out. So a bare `cargo(..)` counts too WHEN AN ARGUMENT DECLARES A TREE:
/// that is what tells it apart from the `cargo()` helper five programs used to
/// carry, which took none.
fn is_the_door(call: &syn::ExprCall) -> bool {
    // ONE GATE AND TWO WAYS THROUGH IT, rather than two rules where the second
    // silently subsumed the first: an injection aimed at the qualified spelling
    // came back with nothing red, because a call ending in `cargo` was already
    // being recognised by the other clause whatever came before it.
    if !(ends_with(call, &["cargo"]) || ends_with(call, &["named_cargo"])) {
        return false;
    }
    ends_with(call, &["issue", "cargo"])
        || ends_with(call, &["issue", "named_cargo"])
        || !matches!(declaration(call), Declared::Unreadable(_))
}

/// Does the called path end with these segments, in order?
fn ends_with(call: &syn::ExprCall, tail: &[&str]) -> bool {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    let written: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    written.len() >= tail.len()
        && written[written.len() - tail.len()..]
            .iter()
            .zip(tail)
            .all(|(written, wanted)| written == wanted)
}

/// What a door call declares.
///
/// THE ARGUMENT IS FOUND BY WHAT IT IS, not by where it sits: `issue::cargo`
/// takes the declaration first and `issue::named_cargo` takes it second, and a
/// reader keyed on position would answer "unreadable" for one of them the day it
/// was written. The first argument that reads as a `Tree` is the declaration.
fn declaration(call: &syn::ExprCall) -> Declared {
    let mut refused = Vec::new();
    for argument in &call.args {
        match one_declaration(argument) {
            Declared::Unreadable(written) => refused.push(written),
            read => return read,
        }
    }
    Declared::Unreadable(if refused.is_empty() {
        "no argument at all".to_string()
    } else {
        refused.join(", ")
    })
}

/// What one expression declares, if it declares anything.
fn one_declaration(expression: &syn::Expr) -> Declared {
    let rendered_text = rendered(expression);
    match unwrap(expression) {
        syn::Expr::Path(path) => {
            match path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .as_deref()
            {
                Some("ThisRepository") => Declared::ThisRepository,
                _ => Declared::Unreadable(rendered_text),
            }
        }
        syn::Expr::Call(call) if ends_with(call, &["MadeByThisRun"]) => {
            call.args.first().and_then(string_literal).map_or_else(
                || Declared::Unreadable(rendered_text),
                Declared::MadeByThisRun,
            )
        }
        syn::Expr::Call(call) if ends_with(call, &["WhereverTheCallerPoints"]) => {
            call.args.first().and_then(string_literal).map_or_else(
                || Declared::Unreadable(rendered_text),
                Declared::WhereverTheCallerPoints,
            )
        }
        syn::Expr::Call(call) if ends_with(call, &["PinnedWhereverItPoints"]) => {
            call.args.first().and_then(string_literal).map_or_else(
                || Declared::Unreadable(rendered_text),
                Declared::PinnedWhereverItPoints,
            )
        }
        syn::Expr::Call(call) if ends_with(call, &["PinnedWhenItIsOurs"]) => {
            call.args.first().and_then(string_literal).map_or_else(
                || Declared::Unreadable(rendered_text),
                Declared::PinnedWhenItIsOurs,
            )
        }
        _ => Declared::Unreadable(rendered_text),
    }
}

/// A plain `fn`'s parameters, in the order it takes them.
///
/// A `self` receiver ends the reading: a signature with one is a method, whose
/// calls are written `x.name(..)` and are not the ones this reader enumerates.
/// A parameter bound by a pattern rather than a name keeps its POSITION — the
/// list has to stay as long as the signature is, or every parameter after it
/// would be followed to the wrong argument.
fn parameters_of(signature: &syn::Signature) -> Vec<String> {
    let mut taken = Vec::new();
    for input in &signature.inputs {
        match input {
            syn::FnArg::Receiver(_) => return Vec::new(),
            syn::FnArg::Typed(typed) => taken.push(binding_name(&typed.pat).unwrap_or_default()),
        }
    }
    taken
}

/// Every name a pattern binds — a closure's argument, a `for` loop's, however
/// deeply it destructures.
fn names_bound_by(pattern: &syn::Pat) -> BTreeSet<String> {
    struct Bound(BTreeSet<String>);
    impl<'ast> Visit<'ast> for Bound {
        fn visit_pat_ident(&mut self, ident: &'ast syn::PatIdent) {
            self.0.insert(ident.ident.to_string());
            syn::visit::visit_pat_ident(self, ident);
        }
    }
    let mut bound = Bound(BTreeSet::new());
    bound.visit_pat(pattern);
    bound.0
}

/// The name a `let` binds, when it binds one plainly. A destructuring pattern
/// binds no single value and is not followed.
fn binding_name(pattern: &syn::Pat) -> Option<String> {
    match pattern {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Type(typed) => binding_name(&typed.pat),
        _ => None,
    }
}

/// A name a spawn site gives its program, and whether it CALLED it.
struct Names {
    name: String,
    /// `cli_binary()` rather than `cli` — a function, whose name means the same
    /// thing in every file that writes it.
    called: bool,
}

/// The single name an expression is, when it is one: `cli`, `cli_binary()`,
/// `common::cli_binary()`, `BIN`.
fn names_a_value(expression: &syn::Expr) -> Option<Names> {
    let last = |path: &syn::ExprPath| {
        path.path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
    };
    match unwrap(expression) {
        syn::Expr::Path(path) => last(path).map(|name| Names {
            name,
            called: false,
        }),
        syn::Expr::Call(call) if call.args.is_empty() => match call.func.as_ref() {
            syn::Expr::Path(path) => last(path).map(|name| Names { name, called: true }),
            _ => None,
        },
        _ => None,
    }
}

/// What program a spawn site runs, read off the expression it names it with.
fn read_program(expression: &syn::Expr) -> Program {
    if let Some(literal) = string_literal(expression) {
        return if literal == "cargo" {
            Program::CargoBesideTheDoor(literal)
        } else {
            Program::Named(literal)
        };
    }
    let rendered_text = rendered(expression);
    // `env!("CARGO_BIN_EXE_<name>")` FIRST, because it contains the letters the
    // test below looks for. A binary this workspace builds is not cargo, and a
    // reader that matched `CARGO` before this line would file every one of them
    // under the wrong program.
    if let Some((_, after)) = rendered_text.split_once("CARGO_BIN_EXE_") {
        let binary: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        return Program::OurBinary(format!("env!(CARGO_BIN_EXE_{binary})"));
    }
    if rendered_text.contains("\"CARGO\"") || names_cargo(&rendered_text) {
        return Program::CargoBesideTheDoor(rendered_text);
    }
    Program::Unplaceable(format!("${rendered_text}"))
}

/// Does this expression's spelling name cargo — `cargo()`, `&cargo`,
/// `the_cargo_running_this()`?
fn names_cargo(rendered_text: &str) -> bool {
    rendered_text
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|word| word == "cargo" || word.ends_with("_cargo") || word.starts_with("cargo_"))
}

/// A string literal's value, through the references and parentheses a call site
/// writes around it.
fn string_literal(expression: &syn::Expr) -> Option<String> {
    match unwrap(expression) {
        syn::Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Str(text) => Some(text.value()),
            _ => None,
        },
        _ => None,
    }
}

/// The [`crate::issue::runtime_words`] wrapper an expression is written in, when
/// it is written in one: the words it stands for, and the flags the site
/// declared may be among them.
///
/// BY POSITION, unlike [`declaration`], and the difference is that there is one
/// spelling of this function rather than two. `runtime_words(words, may_hold,
/// why)` takes the words FIRST, and a reader that hunted instead for "the
/// argument shaped like a list of literals" would find the words themselves the
/// day a site wrapped a list it could have counted.
///
/// A wrapper whose declaration this reader cannot read is [`MayHold::Anything`]
/// — the same answer an unwrapped hole gets. That is the conservative direction
/// and not the convenient one: it leaves every absence unclaimable, which is
/// what a law wants from a declaration it could not read.
fn declared_runtime_words<'a>(
    manifest_dir: &str,
    expression: &'a syn::Expr,
) -> Option<(&'a syn::Expr, MayHold)> {
    let syn::Expr::Call(call) = unwrap(expression) else {
        return None;
    };
    if !ends_with(call, &["runtime_words"]) {
        return None;
    }
    let mut arguments = call.args.iter();
    let words = arguments.next()?;
    let may_hold = arguments
        .next()
        .and_then(|declared| word_list(manifest_dir, declared))
        .map(|declared| {
            declared
                .iter()
                .map(|word| match word {
                    Word::Spelled(text) => Some(text.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<String>>>()
        });
    Some(match may_hold {
        Some(Some(flags)) => (words, MayHold::OnlyThese(flags)),
        _ => (words, MayHold::Anything),
    })
}

/// A word list with no hole in it, or `None` when one is still there.
///
/// ONE SPELLING FOR TWO READERS. The site's own words and the words a call site
/// finishes are the same question asked of two lists, and a second copy of this
/// match is a copy free to answer differently the day a fifth kind of word is
/// added — the shape `CLAUDE.md` calls a half-enforced invariant.
fn without_a_hole(words: &[Word]) -> Option<Vec<String>> {
    words
        .iter()
        .map(|word| match word {
            Word::Spelled(text) | Word::Runtime(text) => Some(text.clone()),
            Word::Unknown(..) | Word::Sometimes(..) => None,
        })
        .collect()
}

/// One argument's word.
///
/// A FREE FUNCTION AND NOT A METHOD, because it is read from two sides now: at
/// the spawn, where the words are written beside the `Command`, and at a CALL
/// SITE one hop back, where the same literal array is written as an argument.
/// `manifest_dir` is what `env!("CARGO_MANIFEST_DIR")` reads as in the file the
/// words are written in — the caller's file for a call site, which is the whole
/// reason this takes it rather than holding one.
fn read_word(manifest_dir: &str, expression: &syn::Expr) -> Word {
    match compile_time_text(manifest_dir, expression) {
        Some(text) => Word::Spelled(text),
        None => Word::Runtime(rendered_as_a_value(expression)),
    }
}

/// The words of a list-shaped expression, or `None` when their NUMBER is decided
/// at runtime.
fn word_list(manifest_dir: &str, expression: &syn::Expr) -> Option<Vec<Word>> {
    match through_the_shapes_that_keep_a_list(expression) {
        syn::Expr::Array(array) => Some(
            array
                .elems
                .iter()
                .map(|item| read_word(manifest_dir, item))
                .collect(),
        ),
        syn::Expr::Macro(invocation) if invocation.mac.path.is_ident("vec") => {
            let elements: syn::punctuated::Punctuated<syn::Expr, syn::Token![,]> = invocation
                .mac
                .parse_body_with(
                    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
                )
                .ok()?;
            Some(
                elements
                    .iter()
                    .map(|item| read_word(manifest_dir, item))
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Through the spellings that hand a list on WITHOUT changing which words are in
/// it: `&`, `(..)`, a macro's invisible group, `.iter()`, `.into_iter()`,
/// `.as_slice()`, `.to_vec()`, `.clone()`.
///
/// A METHOD THAT RESHAPES THE LIST IS NOT ONE OF THESE. `.skip(1)`, `.map(..)`,
/// `[1..]` all answer a different list, and reading them as their receiver would
/// credit a command with words it drops — so they fall through to the refusal
/// rather than being followed. `named_environment::positions_of` draws the same
/// line for the same reason.
fn through_the_shapes_that_keep_a_list(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Reference(inner) => through_the_shapes_that_keep_a_list(&inner.expr),
        syn::Expr::Paren(inner) => through_the_shapes_that_keep_a_list(&inner.expr),
        syn::Expr::Group(inner) => through_the_shapes_that_keep_a_list(&inner.expr),
        syn::Expr::MethodCall(call)
            if call.args.is_empty()
                && matches!(
                    call.method.to_string().as_str(),
                    "iter" | "into_iter" | "as_slice" | "as_ref" | "to_vec" | "clone"
                ) =>
        {
            through_the_shapes_that_keep_a_list(&call.receiver)
        }
        other => other,
    }
}

/// The text a word has BEFORE the program runs: a literal, the manifest
/// directory cargo hands the file's crate, or a `concat!` of those.
fn compile_time_text(manifest_dir: &str, expression: &syn::Expr) -> Option<String> {
    if let Some(literal) = string_literal(expression) {
        return Some(literal);
    }
    // A CONVERSION IS NOT A COMPUTATION, and this is the spelling every
    // `Vec<String>` argument list in this repository is written with:
    // `vec!["metadata".to_string(), …]`. Read as a runtime value it renders as
    // `$"metadata".to_string()`, which is not a word cargo has ever been handed
    // — and the law that asks which subcommands have been measured caught
    // exactly that (R1266). `.display().to_string()` is NOT one of these: its
    // receiver is not compile-time text, so it falls through as it should.
    if let syn::Expr::MethodCall(call) = unwrap(expression) {
        if call.args.is_empty()
            && matches!(
                call.method.to_string().as_str(),
                "to_string" | "to_owned" | "into"
            )
        {
            return compile_time_text(manifest_dir, &call.receiver);
        }
    }
    // `String::from("x")` is the same word by another route.
    if let syn::Expr::Call(call) = unwrap(expression) {
        if ends_with(call, &["String", "from"]) && call.args.len() == 1 {
            return compile_time_text(manifest_dir, call.args.first()?);
        }
    }
    let syn::Expr::Macro(macro_call) = unwrap(expression) else {
        return None;
    };
    match macro_call
        .mac
        .path
        .segments
        .last()?
        .ident
        .to_string()
        .as_str()
    {
        "env" => {
            let name: syn::LitStr = macro_call.mac.parse_body().ok()?;
            (name.value() == "CARGO_MANIFEST_DIR").then(|| manifest_dir.to_string())
        }
        "concat" => {
            let parts: syn::punctuated::Punctuated<syn::Expr, syn::Token![,]> = macro_call
                .mac
                .parse_body_with(
                    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
                )
                .ok()?;
            let mut out = String::new();
            for part in &parts {
                out.push_str(&compile_time_text(manifest_dir, part)?);
            }
            Some(out)
        }
        _ => None,
    }
}

/// Through `&`, `(..)`, and the invisible groups a macro expansion leaves.
fn unwrap(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Reference(inner) => unwrap(&inner.expr),
        syn::Expr::Paren(inner) => unwrap(&inner.expr),
        syn::Expr::Group(inner) => unwrap(&inner.expr),
        _ => expression,
    }
}

/// An expression as its source reads, with the spacing taken out so two
/// spellings of one expression are one string.
fn rendered(expression: &syn::Expr) -> String {
    expression
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// How a value decided at runtime is written into a rendered command line.
///
/// THE SAME SPELLING A SHELL SCRIPT ARRIVES WITH, deliberately:
/// `--manifest-path "$ws/Cargo.toml"` and `.arg(format!("{ws}/Cargo.toml"))` are
/// the same fact about the same command, and [`crate::CargoCommand::manifest`]
/// already knows how to read the first. A second rendering would be a second
/// reader of the same thing.
fn rendered_as_a_value(expression: &syn::Expr) -> String {
    if let Some(shape) = interpolated_shape(expression) {
        return shape;
    }
    format!("${}", rendered(expression).trim_start_matches('&'))
}

/// A `format!` whose literal part names a path keeps that literal part: the
/// interpolations become `$`, and what surrounds them is as real as any word.
fn interpolated_shape(expression: &syn::Expr) -> Option<String> {
    let syn::Expr::Macro(macro_call) = unwrap(expression) else {
        return None;
    };
    let name = macro_call.mac.path.segments.last()?.ident.to_string();
    if name != "format" {
        return None;
    }
    let tokens = macro_call.mac.tokens.to_string();
    let literal = tokens.split('"').nth(1)?;
    let mut out = String::new();
    let mut rest = literal;
    while let Some((before, after)) = rest.split_once('{') {
        out.push_str(before);
        let (inside, tail) = after.split_once('}')?;
        out.push('$');
        out.push_str(inside.split(':').next().unwrap_or_default());
        rest = tail;
    }
    out.push_str(rest);
    // A shape with no literal part at all says nothing a bare rendering does not.
    (out != "$" && !out.is_empty()).then_some(out)
}
