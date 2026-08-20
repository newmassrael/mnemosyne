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
//!   wearing a clean one's clothes.
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
    /// A `Tree` expression this reader cannot read — a variable, a call, a
    /// variant it does not know. NOT a pass.
    Unreadable(String),
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
    /// evaluate. The hole is the count, not the value: a flag may be inside.
    Unknown(String),
    /// A word added on SOME paths through the function and not others — an
    /// `.arg()` inside an `if`, a `match` arm, a loop.
    ///
    /// Reading it as present is how a gate comes to say a command pins its
    /// lockfile when it pins it every other Tuesday; reading it as absent is a
    /// false alarm on a command that is fine. It is neither, and a law that
    /// needs to know whether a flag is there gets told that nobody can say.
    Sometimes(String),
}

impl Word {
    /// The word as a command line reads it, for the rendering the laws print.
    #[must_use]
    pub fn rendered(&self) -> String {
        match self {
            Self::Spelled(word) | Self::Runtime(word) | Self::Unknown(word) => word.clone(),
            Self::Sometimes(word) => format!("[{word}]?"),
        }
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
                Declared::MadeByThisRun(why) | Declared::WhereverTheCallerPoints(why),
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
        self.words
            .iter()
            .map(|word| match word {
                Word::Spelled(text) | Word::Runtime(text) => Some(text.clone()),
                Word::Unknown(_) | Word::Sometimes(_) => None,
            })
            .collect()
    }

    /// This site as one of the population's commands, carrying what it declared.
    fn as_command(&self, words: Vec<String>, declared: Declared) -> CargoCommand {
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
            env: BTreeMap::new(),
            declared: Some(declared),
        }
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
    /// Cargo spawns through the door handing over a list of unknown length.
    pub carried: Vec<RustSpawn>,
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
    /// Every tracked Rust file this walk parsed — the reach, which an empty
    /// finding list alone can never distinguish from a walk that did not run.
    pub files: usize,
    /// Every spawn seen, cargo or not. The denominator that makes the counts
    /// above mean something.
    pub spawns: usize,
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
    for (path, file, named) in parsed {
        found.files += 1;
        let mut walk = Walk::new(&path, &named, &shared, manifest_dir_of(&path, &manifests));
        walk.visit_file(&file);
        for site in walk.sites {
            found.spawns += 1;
            let declared = match &site.program {
                Program::Cargo(
                    declared @ (Declared::ThisRepository
                    | Declared::MadeByThisRun(_)
                    | Declared::WhereverTheCallerPoints(_)),
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
            match site.complete_words() {
                Some(words) => found.commands.push(site.as_command(words, declared)),
                None => found.carried.push(site),
            }
        }
    }
    found
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
    let file = syn::parse_file(text).unwrap_or_else(|why| panic!("{source} does not parse: {why}"));
    let named = named_values(&file);
    let shared = BTreeMap::new();
    let mut walk = Walk::new(source, &named, &shared, manifest_dir.to_string());
    walk.visit_file(&file);
    walk.sites
}

/// One file's walk.
struct Walk<'a> {
    path: &'a str,
    /// What a one-hop follow lands on in THIS file.
    named: &'a Landings,
    /// The names every tracked file agrees about.
    shared: &'a BTreeMap<String, String>,
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
    /// How many branches deep the walk currently is inside the function — an
    /// `if`, a `match` arm, a loop body, a closure. Compared against the depth
    /// the site was OPENED at, so a whole command written inside one `if` reads
    /// as unconditional while a flag added by a second one does not.
    depth: usize,
    /// The depth each site was opened at.
    opened_at: Vec<usize>,
}

impl<'a> Walk<'a> {
    fn new(
        path: &'a str,
        named: &'a Landings,
        shared: &'a BTreeMap<String, String>,
        manifest_dir: String,
    ) -> Self {
        Self {
            path,
            named,
            shared,
            manifest_dir,
            owners: Vec::new(),
            sites: Vec::new(),
            commands: BTreeMap::new(),
            values: BTreeMap::new(),
            depth: 0,
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
    fn in_function(&mut self, name: String, body: &syn::Block) {
        let commands = std::mem::take(&mut self.commands);
        let values = std::mem::take(&mut self.values);
        let depth = std::mem::take(&mut self.depth);
        self.owners.push(name);
        self.visit_block(body);
        self.owners.pop();
        self.commands = commands;
        self.values = values;
        self.depth = depth;
    }

    /// Walk something that only happens on SOME paths.
    fn in_a_branch(&mut self, walk: impl FnOnce(&mut Self)) {
        self.depth += 1;
        walk(self);
        self.depth -= 1;
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
        });
        self.opened_at.push(self.depth);
        self.sites.len() - 1
    }

    /// Add one word to a site, saying whether every path through the function
    /// reaches it.
    fn add(&mut self, site: usize, word: Word) {
        let certain = self.depth <= self.opened_at[site];
        self.sites[site].words.push(match (certain, word) {
            (true, word) => word,
            (false, Word::Spelled(text) | Word::Runtime(text) | Word::Sometimes(text)) => {
                Word::Sometimes(text)
            }
            // An unknown COUNT stays an unknown count: which hole it is does not
            // get smaller for being conditional.
            (false, Word::Unknown(text)) => Word::Unknown(text),
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
                self.visit_expr(&call.func);
                for argument in &call.args {
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
                let mut sites = Vec::new();
                self.in_a_branch(|walk| {
                    sites.extend(walk.root_of_block(&node.then_branch));
                    if let Some((_, otherwise)) = &node.else_branch {
                        sites.extend(walk.root(otherwise));
                    }
                });
                sites
            }
            syn::Expr::Match(node) => {
                self.visit_expr(&node.expr);
                let mut sites = Vec::new();
                self.in_a_branch(|walk| {
                    for arm in &node.arms {
                        sites.extend(walk.root(&arm.body));
                    }
                });
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
                    let word = self.read_word(first);
                    self.add(site, word);
                }
            }
            "args" => {
                if let Some(first) = call.args.first() {
                    // AN ARRAY'S LENGTH IS KNOWN even when its elements are not:
                    // `["run", "--manifest-path", path]` is three words, one of
                    // them decided at runtime, and reading it as a hole would
                    // throw away the two flags that are right there.
                    match self.word_list(first) {
                        Some(words) => {
                            for word in words {
                                self.add(site, word);
                            }
                        }
                        None => self.add(site, Word::Unknown(rendered_as_a_value(first))),
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

    /// One argument's word.
    fn read_word(&self, expression: &syn::Expr) -> Word {
        match self.compile_time_text(expression) {
            Some(text) => Word::Spelled(text),
            None => Word::Runtime(rendered_as_a_value(expression)),
        }
    }

    /// The words of an `.args(..)` argument, or `None` when their NUMBER is
    /// decided at runtime.
    fn word_list(&self, expression: &syn::Expr) -> Option<Vec<Word>> {
        match unwrap(expression) {
            syn::Expr::Array(array) => Some(
                array
                    .elems
                    .iter()
                    .map(|item| self.read_word(item))
                    .collect(),
            ),
            _ => None,
        }
    }

    /// The text a word has BEFORE the program runs: a literal, the manifest
    /// directory cargo hands this file's crate, or a `concat!` of those.
    fn compile_time_text(&self, expression: &syn::Expr) -> Option<String> {
        if let Some(literal) = string_literal(expression) {
            return Some(literal);
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
                (name.value() == "CARGO_MANIFEST_DIR").then(|| self.manifest_dir.clone())
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
                    out.push_str(&self.compile_time_text(part)?);
                }
                Some(out)
            }
            _ => None,
        }
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
        self.in_function(item.sig.ident.to_string(), &item.block);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.in_function(item.sig.ident.to_string(), &item.block);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if let Some(body) = &item.default {
            self.in_function(item.sig.ident.to_string(), body);
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let Some(init) = &local.init else {
            return;
        };
        let sites = self.root(&init.expr);
        if let Some(name) = binding_name(&local.pat) {
            if sites.is_empty() {
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

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.visit_expr(&node.cond);
        self.in_a_branch(|walk| walk.visit_block(&node.body));
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.visit_expr(&node.expr);
        self.in_a_branch(|walk| walk.visit_block(&node.body));
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.in_a_branch(|walk| walk.visit_block(&node.body));
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        self.in_a_branch(|walk| walk.visit_expr(&node.body));
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
        _ => Declared::Unreadable(rendered_text),
    }
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
