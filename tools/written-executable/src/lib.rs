//! The law: nothing in this repository creates an executable file.
//!
//! # What goes wrong when something does
//!
//! `ETXTBSY` — "Text file busy" — is the kernel refusing to `exec` a file some
//! process holds open for writing. After `fs::write` returns, THIS process holds
//! nothing; another one does. A sibling test thread's `Command::spawn` forks, the
//! child inherits every open descriptor, and until that child reaches its own
//! `exec` (where `O_CLOEXEC` finally closes ours) the kernel is right. The window
//! is microseconds wide, it opens only when something else is running, and what
//! it produces is a failure in the crate that was minding its own business.
//!
//! Round 1192 met it in `unread-declaration`, whose eleven cases each wrote a
//! shell script and ran it. Every one was green alone. Eleven in flight beside
//! ten other crates is the NORMAL case under `scripts/check-side-workspaces.sh`,
//! and there it died. The repair was not a retry, a lock or a serial attribute —
//! all three treat an ownership problem as a scheduling one. The repair was to
//! stop writing the program: cargo builds it, and what varies per case is DATA it
//! reads, because data cannot be busy.
//!
//! That round closed one crate and wrote down what it could not do: NOTHING ASKS
//! THE OTHERS. This is that.
//!
//! # The two ways to create one
//!
//! A rule about `chmod` alone would be half a law, and the missing half is the
//! one production code was using.
//!
//! 1. **A mode with an executable bit** — `fs::set_permissions`,
//!    `Permissions::from_mode`, `PermissionsExt::set_mode`, `OpenOptionsExt::mode`.
//! 2. **`fs::copy` of a program** — a copy carries the SOURCE's mode, so copying
//!    `std::env::current_exe()` or a `CARGO_BIN_EXE_…` path writes an executable
//!    with no chmod anywhere in the function. `injection-harness` took a copy of
//!    its own binary to supervise with and then ran it.
//!
//! Every other `fs::copy` is counted and printed rather than judged: this walk
//! cannot read the mode of a file it is not looking at, and calling every copy a
//! finding would be a gate nobody could keep green. What it CAN say with no false
//! alarm at all is that a copy whose source this function names as a program is a
//! written program, and that is the arm it fails on.
//!
//! # Why the question is asked of a function
//!
//! The mode and the call that applies it sit on different statements in most of
//! this repository's spellings:
//!
//! ```text
//! let mut perms = fs::metadata(path)?.permissions();
//! perms.set_mode(0o755);
//! fs::set_permissions(path, perms)?;
//! ```
//!
//! An expression-chain rule reads `set_permissions(p, Permissions::from_mode(0o755))`
//! and not that, and a gate that refuses the tidier spelling of the shape it
//! enforces is one people route around (Round 1182). So the unit is the FUNCTION:
//! one that applies a permission mode is read together with the octal literals
//! written in it. Nested functions are judged depth-first on their own bodies, so
//! a helper cannot inherit its parent's literal.
//!
//! A mode this walk cannot read — a function that applies one and writes no octal
//! literal at all — is a REFUSAL rather than a pass. Round 1176's rule: the
//! absence of a spelling in today's corpus is luck, and a gate that answers
//! "clean" for a question it could not ask is worse than one that answers
//! nothing.
//!
//! # Strong evidence and weak, both counted
//!
//! Whether the file is then RUN does not change the verdict — the bit is the
//! hazard, and who execs it (this process, its child, a git hook) does not move
//! the window. But it changes how much the report can claim, so it is measured in
//! three grades and all three are printed:
//!
//! | grade | what the walk saw |
//! |---|---|
//! | [`Ran::ThePath`] | `Command::new` of the SAME expression, in the same function |
//! | [`Ran::SomethingHere`] | a `Command::new` in the same function, of something else |
//! | [`Ran::SomethingInTheFile`] | a `Command::new` elsewhere in the file |
//! | [`Ran::NothingVisible`] | none — the exec is through a helper, a hook, or `PATH` |
//!
//! Round 1182's discipline: print the weak basis and the strong one side by side,
//! so the weaker cannot pass for the stronger in a reader's head.
//!
//! # What it does not see, said rather than hidden
//!
//! - A mode arriving as a PARAMETER is unreadable here and stops the gate, which
//!   is the honest answer rather than a silent pass.
//! - `fs::copy` whose source is named more than ONE hop away — through a `let` in
//!   a caller, a struct field, a helper that takes arguments — reads as an
//!   ordinary copy. It is in the printed copy census. The one hop that IS
//!   followed is a same-file argument-less helper, because `injection-harness`
//!   held exactly that shape while this gate was being written.
//! - A spawn that is not `Command::new` (a helper, `exec`, a shell) is invisible
//!   to the EVIDENCE grade only; it cannot change a verdict.
//! - `symlink` creates no executable and is the repair this gate points at.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use quote::ToTokens;

/// How a source creates an executable file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Made {
    /// A permission mode carrying an executable bit was applied in this function.
    Mode(u32),
    /// A program this function names was copied, and a copy carries the source's
    /// mode.
    CopyOf(String),
}

impl fmt::Display for Made {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Made::Mode(mode) => write!(f, "applies mode 0o{mode:o}, which is executable"),
            Made::CopyOf(program) => write!(
                f,
                "copies {program}, and a copy carries the source's mode — so this writes an \
                 executable with no chmod in sight"
            ),
        }
    }
}

/// What the walk can say about the created file being RUN. Strongest first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ran {
    /// `Command::new` of the same expression, in the same function.
    ThePath(String),
    /// A `Command::new` in the same function, of something this walk cannot tie
    /// to the created file.
    SomethingHere,
    /// A `Command::new` elsewhere in the same file.
    SomethingInTheFile,
    /// None. The exec is through a helper, a git hook, or `PATH`.
    NothingVisible,
}

impl fmt::Display for Ran {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ran::ThePath(path) => write!(f, "and runs `{path}` itself"),
            Ran::SomethingHere => write!(f, "and spawns a program in the same function"),
            Ran::SomethingInTheFile => write!(f, "and this file spawns a program"),
            Ran::NothingVisible => write!(
                f,
                "and nothing in this file spawns it — which does not make it safe, \
                 only harder to see"
            ),
        }
    }
}

/// A function that creates an executable file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    /// The function it sits in, or `<file>` when it sits outside one.
    pub owner: String,
    pub made: Made,
    pub ran: Ran,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` {} {}", self.owner, self.made, self.ran)
    }
}

/// A function that applies a permission mode this walk cannot read.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unreadable {
    pub file: PathBuf,
    pub line: usize,
    pub owner: String,
}

/// A `fs::copy` whose source this walk cannot name, printed rather than judged.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CopySite {
    pub file: PathBuf,
    pub line: usize,
    pub owner: String,
}

/// What the walk opened, so a zero is a measurement rather than an absence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Files parsed.
    pub scanned: BTreeSet<PathBuf>,
    /// Files under a directory declaring its own `[workspace]`, checked by
    /// pointing this gate at THAT manifest.
    pub foreign_workspaces: usize,
    /// Files under a `target/` directory — cargo's output, not source.
    pub build_artifacts: usize,
    /// Directory symlinks the walk declined to follow. COUNTED RATHER THAN
    /// SKIPPED, for the reason `unowned-scratch` records: in this repository
    /// `target` is a symlink to a shared build cache, and a walk that falls
    /// through to an extension check passes over it without a word.
    pub symlinks_not_followed: BTreeSet<PathBuf>,
    /// Files that failed to parse. Any of these is a refusal.
    pub unparsed: Vec<(PathBuf, String)>,
}

/// The gate's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub workspace_root: PathBuf,
    pub coverage: Coverage,
    /// Functions that apply a permission mode — the population the mode findings
    /// are a numerator of. Without it a clean tree and an unread one print the
    /// same number.
    pub applying: usize,
    /// Functions that call `fs::copy` — the second population, most of which
    /// copy data and are none of this law's business.
    pub copying: usize,
    /// Permission-setting functions whose mode this walk cannot read. Any of
    /// these is a refusal: see the module doc.
    pub unreadable: Vec<Unreadable>,
    /// Copies whose source this walk cannot name. Printed, never failed on.
    pub unnamed_copies: Vec<CopySite>,
    pub findings: Vec<Finding>,
}

impl Report {
    /// Whether the gate read enough to have an opinion at all.
    ///
    /// # Errors
    ///
    /// When nothing was opened, when a file did not parse, or when a function
    /// applies a mode this walk cannot read — each of which makes a clean answer
    /// an answer about something other than this tree.
    pub fn verdict(&self) -> Result<(), String> {
        if self.coverage.scanned.is_empty() {
            return Err(
                "no .rs file was opened — a clean answer and an answer about nothing are \
                 the same answer, so this is not one"
                    .to_owned(),
            );
        }
        if let Some((file, why)) = self.coverage.unparsed.first() {
            return Err(format!(
                "{} did not parse ({why}), so the gate read only part of this tree",
                file.display()
            ));
        }
        if let Some(site) = self.unreadable.first() {
            return Err(format!(
                "`{}` at {}:{} applies a permission mode this walk cannot read — no octal \
                 literal is written in it, so whether it creates an executable is unknown \
                 rather than no",
                site.owner,
                site.file.display(),
                site.line,
            ));
        }
        Ok(())
    }

    /// How many findings carry each grade of run-evidence, strongest first.
    ///
    /// The report prints all four so the weak basis cannot pass for the strong
    /// one — Round 1182's discipline, applied to a law whose verdict rests on
    /// neither.
    #[must_use]
    pub fn evidence(&self) -> [usize; 4] {
        let mut grades = [0usize; 4];
        for finding in &self.findings {
            let at = match finding.ran {
                Ran::ThePath(_) => 0,
                Ran::SomethingHere => 1,
                Ran::SomethingInTheFile => 2,
                Ran::NothingVisible => 3,
            };
            grades[at] += 1;
        }
        grades
    }
}

/// Whether a path ends in the segments given, however it was imported.
fn ends_with(path: &syn::Path, tail: &[&str]) -> bool {
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .rev()
        .collect();
    tail.iter()
        .rev()
        .enumerate()
        .all(|(i, want)| segments.get(i).map(String::as_str) == Some(*want))
}

/// The value of an octal literal as it was WRITTEN, or `None` for any other
/// spelling.
///
/// Octal is how a Unix mode is written and very nearly the only thing it is used
/// for; a decimal `493` would be missed, and that limit is stated rather than
/// papered over with a rule that would read `let n = 7;` as a mode.
fn octal_value(token: &str) -> Option<u32> {
    let digits = token.strip_prefix("0o")?;
    let digits: String = digits
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '_')
        .filter(|c| *c != '_')
        .collect();
    if digits.is_empty() {
        return None;
    }
    u32::from_str_radix(&digits, 8).ok()
}

/// An expression as it was written, normalised enough that `&stub` and `stub`
/// are the same path.
///
/// The strongest thing this walk says is that two expressions are the SAME ONE,
/// and rendering them through `ToTokens` is the only comparison that does not
/// need a describer per expression kind.
fn render(expression: &syn::Expr) -> String {
    let rendered = expression.to_token_stream().to_string();
    let trimmed = rendered
        .trim_start_matches('&')
        .trim()
        .trim_start_matches("mut ")
        .trim();
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether an expression is a reference, which is how a UFCS GETTER spells its
/// receiver: `PermissionsExt::mode(&perms)` reads a mode and
/// `OpenOptionsExt::mode(&mut options, 0o755)` writes one.
fn is_reference(expression: &syn::Expr) -> bool {
    matches!(expression, syn::Expr::Reference(_))
}

fn line_of<T: syn::spanned::Spanned>(node: &T) -> usize {
    node.span().start().line
}

/// One `set_permissions`/`set_mode`/`from_mode`/`mode` call.
#[derive(Debug, Clone)]
struct Applied {
    line: usize,
    /// The path whose permissions are set, when the spelling names one. Only
    /// `set_permissions` does; the others take a `Permissions` or an
    /// `OpenOptions`.
    path: Option<String>,
}

/// One `fs::copy` call.
#[derive(Debug, Clone)]
struct Copied {
    line: usize,
    /// What is being copied. A helper in the same file may be what names the
    /// program, which is the one hop this walk follows.
    source: Option<String>,
    /// Where the copy lands, for the run-evidence comparison.
    destination: Option<String>,
}

/// Everything one body says about this law.
#[derive(Default, Debug)]
struct Body {
    applied: Vec<Applied>,
    copied: Vec<Copied>,
    /// The first executable mode written here, if any.
    executable_mode: Option<u32>,
    /// Whether ANY octal literal is written here — the difference between "the
    /// mode is not executable" and "the mode is not readable".
    wrote_an_octal: bool,
    /// Programs this body can name: `current_exe()`, `env!("CARGO_BIN_EXE_…")`.
    programs: Vec<String>,
    /// `Command::new(<expr>)` arguments, rendered.
    spawns: Vec<String>,
}

impl Body {
    fn saw_a_mode(&mut self, value: u32) {
        self.wrote_an_octal = true;
        if value & 0o111 != 0 && self.executable_mode.is_none() {
            self.executable_mode = Some(value);
        }
    }

    /// The best thing this body can say about the created file being run.
    fn ran(&self, path: Option<&String>, in_the_file: bool) -> Ran {
        if let Some(path) = path {
            if self.spawns.iter().any(|spawn| spawn == path) {
                return Ran::ThePath(path.clone());
            }
        }
        if !self.spawns.is_empty() {
            return Ran::SomethingHere;
        }
        if in_the_file {
            return Ran::SomethingInTheFile;
        }
        Ran::NothingVisible
    }
}

/// The one walk, driven from either entry point below.
///
/// IT DOES NOT DESCEND INTO A NESTED NAMED FUNCTION, and that is the whole
/// reason the unit can be the function. A helper written inside another function
/// has its own body and its own literals; collecting the outer body with the
/// inner one folded in would let a parent's `0o644` and a child's `0o755` decide
/// each other's verdicts — in BOTH directions, one a false alarm and the other a
/// silent pass. Closures are not skipped: they have no name and no body of their
/// own to be judged on, so they belong to the function that writes them.
#[derive(Default)]
struct Walk {
    body: Body,
    /// Whether `OpenOptionsExt` is in scope in this file.
    ///
    /// ⚠ `mode` IS NOT A RARE METHOD NAME, and this gate learned that by being
    /// run: `bench/crates/sled-baseline` calls
    /// `sled::Config::default().mode(sled::Mode::HighThroughput)`, which has
    /// nothing to do with permissions, and the first draft refused the whole
    /// workspace over it. `set_mode`, `from_mode` and `set_permissions` are
    /// unambiguous names; `mode` is the one that needs a second fact, and the
    /// second fact is exactly the condition Rust puts on the call: a trait
    /// method is callable only where the trait is imported.
    open_options_ext_in_scope: bool,
}

impl Walk {
    fn scoped(open_options_ext_in_scope: bool) -> Self {
        Walk {
            body: Body::default(),
            open_options_ext_in_scope,
        }
    }
}

/// Whether a file imports the trait that gives `OpenOptions` a `mode` method.
///
/// A rename (`as Whatever`) still names it on the way in, and a glob over
/// `std::os::unix::fs` brings it too — both are read as the import they are.
fn open_options_ext_in_scope(file: &syn::File) -> bool {
    fn in_tree(tree: &syn::UseTree, under_unix: bool) -> bool {
        match tree {
            // `unix` and not `fs`: a glob is only this trait's home under
            // `std::os::unix::…`, and `use std::fs::*` — which brings neither
            // extension trait — would otherwise read as one that does.
            syn::UseTree::Path(path) => in_tree(&path.tree, under_unix || path.ident == "unix"),
            syn::UseTree::Name(name) => name.ident == "OpenOptionsExt",
            syn::UseTree::Rename(rename) => rename.ident == "OpenOptionsExt",
            syn::UseTree::Glob(_) => under_unix,
            syn::UseTree::Group(group) => group.items.iter().any(|item| in_tree(item, under_unix)),
        }
    }
    struct Imports(bool);
    impl<'ast> syn::visit::Visit<'ast> for Imports {
        fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
            if in_tree(&item.tree, false) {
                self.0 = true;
            }
            syn::visit::visit_item_use(self, item);
        }
    }
    let mut imports = Imports(false);
    syn::visit::visit_file(&mut imports, file);
    imports.0
}

impl<'ast> syn::visit::Visit<'ast> for Walk {
    fn visit_item_fn(&mut self, _nested: &'ast syn::ItemFn) {}

    fn visit_impl_item_fn(&mut self, _nested: &'ast syn::ImplItemFn) {}

    fn visit_trait_item_fn(&mut self, _nested: &'ast syn::TraitItemFn) {}

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref() {
            let path = &path.path;
            let line = line_of(call);
            // `fs::set_permissions(p, perms)` — the one spelling that names the
            // path whose mode is being set.
            if ends_with(path, &["set_permissions"]) {
                self.body.applied.push(Applied {
                    line,
                    path: call.args.first().map(render),
                });
            }
            // `Permissions::from_mode(0o755)` and the UFCS
            // `PermissionsExt::set_mode(&mut perms, 0o755)`.
            if ends_with(path, &["from_mode"]) || ends_with(path, &["set_mode"]) {
                self.body.applied.push(Applied { line, path: None });
            }
            // UFCS `OpenOptionsExt::mode(&mut options, 0o755)`. The TRAIT IS
            // NAMED here, which is what keeps this apart from the getter
            // `PermissionsExt::mode(&perms)` and from every other `mode` a tree
            // may have.
            if ends_with(path, &["OpenOptionsExt", "mode"]) {
                self.body.applied.push(Applied { line, path: None });
            }
            if ends_with(path, &["current_exe"]) {
                self.body.programs.push("current_exe()".to_owned());
            }
            if ends_with(path, &["Command", "new"]) {
                if let Some(argument) = call.args.first() {
                    self.body.spawns.push(render(argument));
                }
            }
            // `fs::copy(from, to)`. `std::io::copy` is EXCLUDED by name: it takes
            // two `&mut` streams and copies bytes between them, which creates no
            // file at all.
            let is_copy = ends_with(path, &["fs", "copy"])
                || (path.segments.len() == 1
                    && ends_with(path, &["copy"])
                    && call.args.len() == 2
                    && !call.args.iter().any(is_reference));
            if is_copy && !ends_with(path, &["io", "copy"]) {
                self.body.copied.push(Copied {
                    line,
                    source: call.args.first().map(render),
                    destination: call.args.get(1).map(render),
                });
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.to_string();
        let line = line_of(call);
        if name == "set_mode" {
            self.body.applied.push(Applied { line, path: None });
        }
        // `OpenOptions::new().mode(0o755)`, and ONLY where the trait that
        // supplies that method is imported — see `open_options_ext_in_scope` for
        // the tree this cost. One argument that is not a reference besides: the
        // zero-argument `permissions().mode()` is a GETTER, which this
        // repository's tests use to assert that a TRACKED hook is executable.
        if self.open_options_ext_in_scope
            && name == "mode"
            && call.args.len() == 1
            && !call.args.iter().any(is_reference)
        {
            self.body.applied.push(Applied { line, path: None });
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_lit_int(&mut self, literal: &'ast syn::LitInt) {
        if let Some(value) = octal_value(&literal.token().to_string()) {
            self.body.saw_a_mode(value);
        }
        syn::visit::visit_lit_int(self, literal);
    }

    // ⚠ `syn` DOES NOT WALK A MACRO'S BODY, and this repository writes modes
    // inside them: an `assert!` comparing one, a `format!` naming a path. Round
    // 1186 met the same blindness from the other side, where a law asserted that
    // macro-generated test names were absent from a tree full of them. The tokens
    // are therefore read — weaker than syntax, and the strongest reading
    // available, since expanding the macro is what a walk cannot do.
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        // `env!("CARGO_BIN_EXE_…")` is a program this body can name, and it is
        // the one spelling cargo checks at compile time.
        if ends_with(&invocation.path, &["env"]) {
            for tree in invocation.tokens.clone() {
                if let proc_macro2::TokenTree::Literal(literal) = tree {
                    let text = literal.to_string();
                    if text.starts_with("\"CARGO_BIN_EXE_") {
                        self.body.programs.push(text.trim_matches('"').to_owned());
                    }
                }
            }
        }
        scan_tokens(invocation.tokens.clone(), &mut self.body);
        syn::visit::visit_macro(self, invocation);
    }
}

/// Read one token stream for the things this law is about that a macro can hide.
///
/// Each group is scanned in its OWN flat sequence rather than a globally
/// flattened one, so `Command :: new` has to be adjacent where it was written
/// instead of across a bracket that happened to sit between them.
///
/// The path a spawn is pointed at is NOT recovered here — an argument list inside
/// a macro is a token stream, not an expression — so a spawn seen this way
/// contributes the weaker [`Ran::SomethingHere`] and never [`Ran::ThePath`].
fn scan_tokens(tokens: proc_macro2::TokenStream, body: &mut Body) {
    use proc_macro2::TokenTree;

    let flat: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, tree) in flat.iter().enumerate() {
        match tree {
            TokenTree::Group(group) => scan_tokens(group.stream(), body),
            TokenTree::Literal(literal) => {
                if let Some(value) = octal_value(&literal.to_string()) {
                    body.saw_a_mode(value);
                }
            }
            TokenTree::Ident(ident) => {
                if ident == "set_permissions" || ident == "set_mode" || ident == "from_mode" {
                    body.applied.push(Applied {
                        line: ident.span().start().line,
                        path: None,
                    });
                }
                if ident == "current_exe" {
                    body.programs.push("current_exe()".to_owned());
                }
                if ident == "new" {
                    // `Command :: new`, by tokens: the two colons are separate
                    // `Punct`s, so the identifier is three places back.
                    let owner = index.checked_sub(3).and_then(|at| flat.get(at));
                    let colon = index.checked_sub(1).and_then(|at| flat.get(at));
                    if matches!(owner, Some(TokenTree::Ident(name)) if name == "Command")
                        && matches!(colon, Some(TokenTree::Punct(p)) if p.as_char() == ':')
                    {
                        body.spawns.push(String::new());
                    }
                }
            }
            TokenTree::Punct(_) => {}
        }
    }
}

fn body_of_block(block: &syn::Block, open_options_ext_in_scope: bool) -> Body {
    let mut walk = Walk::scoped(open_options_ext_in_scope);
    syn::visit::visit_block(&mut walk, block);
    walk.body
}

/// What the file says OUTSIDE every named function — a `const` initialiser, a
/// `static`. The same walk, which skips nested functions, so this is exactly the
/// leftover.
fn body_of_file(file: &syn::File, open_options_ext_in_scope: bool) -> Body {
    let mut walk = Walk::scoped(open_options_ext_in_scope);
    syn::visit::visit_file(&mut walk, file);
    walk.body
}

/// The functions in this file that RETURN a program's path.
///
/// ONE HOP, AND IT IS NOT SPECULATIVE SUBSTRATE. `injection-harness` held
/// exactly this shape while this gate was being written — `fs::copy(binary(),
/// &tool)`, where `binary()` is a helper three lines up returning
/// `env!("CARGO_BIN_EXE_injection-harness")` — and the gate read it as an
/// ordinary copy and said nothing. It was found by a person reading, which is
/// the thing a gate exists to stop being necessary.
///
/// A function counts when its own body names a program and it takes no
/// arguments, so what it returns cannot depend on a caller. Deeper than one hop
/// is a value-tracking walk this does not do, and the copy census is what says
/// so out loud.
fn helpers_naming_a_program(file: &syn::File) -> BTreeSet<String> {
    struct Helpers(BTreeSet<String>);
    impl Helpers {
        fn take(&mut self, name: &str, arguments: usize, block: &syn::Block) {
            if arguments != 0 {
                return;
            }
            // The trait flag is irrelevant here: what is being looked for is a
            // program name, not a permission mode.
            if !body_of_block(block, false).programs.is_empty() {
                self.0.insert(name.to_owned());
            }
        }
    }
    impl<'ast> syn::visit::Visit<'ast> for Helpers {
        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            syn::visit::visit_item_fn(self, item);
            self.take(
                &item.sig.ident.to_string(),
                item.sig.inputs.len(),
                &item.block,
            );
        }
    }
    let mut helpers = Helpers(BTreeSet::new());
    syn::visit::visit_file(&mut helpers, file);
    helpers.0
}

/// Whether ANYTHING in this file spawns a program.
///
/// A fact about the file rather than about any one function, so it needs its own
/// pass: the walk above deliberately stops at a function boundary, and the
/// weakest grade of run-evidence is precisely the one that crosses it.
fn file_spawns(file: &syn::File) -> bool {
    #[derive(Default)]
    struct Scan(bool);
    impl<'ast> syn::visit::Visit<'ast> for Scan {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = call.func.as_ref() {
                if ends_with(&path.path, &["Command", "new"]) {
                    self.0 = true;
                }
            }
            syn::visit::visit_expr_call(self, call);
        }
        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            let mut hidden = Body::default();
            scan_tokens(invocation.tokens.clone(), &mut hidden);
            if !hidden.spawns.is_empty() {
                self.0 = true;
            }
            syn::visit::visit_macro(self, invocation);
        }
    }
    let mut scan = Scan::default();
    syn::visit::visit_file(&mut scan, file);
    scan.0
}

/// Judge one parsed file.
///
/// The unit is the innermost function each site sits in; a site outside every
/// function is owned by the file, which is stated in the finding rather than
/// skipped.
pub fn judge(file: &syn::File, path: &Path, report: &mut Report) {
    let spawns_somewhere = file_spawns(file);
    // A FACT ABOUT THE FILE, asked once: `mode` is only the permission setter
    // where the trait that supplies it is imported.
    let mode_is_a_permission = open_options_ext_in_scope(file);
    let helpers = helpers_naming_a_program(file);

    struct Functions<'a> {
        path: &'a Path,
        report: &'a mut Report,
        spawns_somewhere: bool,
        mode_is_a_permission: bool,
        /// The same-file functions that return a program's path.
        helpers: &'a BTreeSet<String>,
        /// Lines already accounted for by an inner function, so a nested one is
        /// not reported twice by the function that contains it.
        claimed: BTreeSet<usize>,
    }
    impl Functions<'_> {
        // ONE FINDING PER FUNCTION PER WAY, and that is a correction this law's
        // own tests made rather than a preference. The ordinary spelling
        // `set_permissions(p, Permissions::from_mode(0o755))` is TWO calls this
        // walk recognises, on ONE line, and the first draft reported it twice —
        // once with the path it could name and once without, so the evidence
        // census counted a single site as both the strongest grade and a weaker
        // one. The unit of this law is the function; the line reported is the
        // first place in it a person can open.
        fn take(&mut self, owner: &str, body: &Body) {
            let fresh_modes: Vec<Applied> = body
                .applied
                .iter()
                .filter(|site| !self.claimed.contains(&site.line))
                .cloned()
                .collect();
            let fresh_copies: Vec<Copied> = body
                .copied
                .iter()
                .filter(|site| !self.claimed.contains(&site.line))
                .cloned()
                .collect();
            if fresh_modes.is_empty() && fresh_copies.is_empty() {
                return;
            }

            if let Some(first) = fresh_modes.iter().map(|site| site.line).min() {
                self.report.applying += 1;
                for site in &fresh_modes {
                    self.claimed.insert(site.line);
                }
                // The path from whichever call names one: only `set_permissions`
                // does, and it is not always on the group's first line.
                let path = fresh_modes.iter().find_map(|site| site.path.clone());
                match body.executable_mode {
                    Some(mode) => self.report.findings.push(Finding {
                        file: self.path.to_path_buf(),
                        line: first,
                        owner: owner.to_owned(),
                        made: Made::Mode(mode),
                        ran: body.ran(path.as_ref(), self.spawns_somewhere),
                    }),
                    // NOT A PASS. A function that applies a mode and writes no
                    // octal literal is one whose mode arrived from somewhere this
                    // walk cannot follow.
                    None if !body.wrote_an_octal => {
                        self.report.unreadable.push(Unreadable {
                            file: self.path.to_path_buf(),
                            line: first,
                            owner: owner.to_owned(),
                        });
                    }
                    None => {}
                }
            }

            if let Some(first) = fresh_copies.iter().map(|site| site.line).min() {
                self.report.copying += 1;
                for site in &fresh_copies {
                    self.claimed.insert(site.line);
                }
                // A program this body names itself, or one a same-file helper
                // returns — the single hop `helpers_naming_a_program` explains.
                let through_a_helper = fresh_copies.iter().find_map(|site| {
                    // `binary()` renders as `binary ()`, so the parentheses come
                    // off one at a time with the spacing `ToTokens` puts between
                    // every pair of tokens.
                    let name = site
                        .source
                        .as_deref()?
                        .strip_suffix(')')?
                        .trim_end()
                        .strip_suffix('(')?
                        .trim_end();
                    self.helpers
                        .contains(name)
                        .then(|| format!("what `{name}()` returns"))
                });
                match body.programs.first().cloned().or(through_a_helper).as_ref() {
                    Some(program) => {
                        let destination = fresh_copies
                            .iter()
                            .find_map(|site| site.destination.clone());
                        self.report.findings.push(Finding {
                            file: self.path.to_path_buf(),
                            line: first,
                            owner: owner.to_owned(),
                            made: Made::CopyOf(program.clone()),
                            ran: body.ran(destination.as_ref(), self.spawns_somewhere),
                        });
                    }
                    // The census stays PER SITE: it counts copies whose source
                    // this walk cannot name, and folding two into one would print
                    // a smaller number than the truth.
                    None => {
                        for site in &fresh_copies {
                            self.report.unnamed_copies.push(CopySite {
                                file: self.path.to_path_buf(),
                                line: site.line,
                                owner: owner.to_owned(),
                            });
                        }
                    }
                }
            }
        }
    }
    impl<'ast> syn::visit::Visit<'ast> for Functions<'_> {
        // DEPTH FIRST, so the innermost function claims its own lines before the
        // one containing it is asked. Otherwise a helper nested inside a function
        // that happens to write `0o644` would be judged by its parent's literal.
        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            syn::visit::visit_item_fn(self, item);
            let body = body_of_block(&item.block, self.mode_is_a_permission);
            self.take(&item.sig.ident.to_string(), &body);
        }

        fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
            syn::visit::visit_impl_item_fn(self, item);
            let body = body_of_block(&item.block, self.mode_is_a_permission);
            self.take(&item.sig.ident.to_string(), &body);
        }

        // A DEFAULT METHOD ON A TRAIT IS A BODY TOO. It is here because the walk
        // above stops at one, so a chmod written in a trait default would be
        // collected by neither pass and dropped in silence — a gate answering
        // "clean" about code it never read.
        fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
            syn::visit::visit_trait_item_fn(self, item);
            if let Some(block) = &item.default {
                let body = body_of_block(block, self.mode_is_a_permission);
                self.take(&item.sig.ident.to_string(), &body);
            }
        }
    }

    let mut functions = Functions {
        path,
        report,
        spawns_somewhere,
        mode_is_a_permission,
        helpers: &helpers,
        claimed: BTreeSet::new(),
    };
    syn::visit::visit_file(&mut functions, file);

    // A site outside every function — a `const` initialiser, a `static`. Owned by
    // the file, and named as such rather than dropped.
    let claimed = functions.claimed.clone();
    let owner = format!("<{}>", path.display());
    let mut file_level = Functions {
        path,
        report,
        spawns_somewhere,
        mode_is_a_permission,
        helpers: &helpers,
        claimed,
    };
    file_level.take(&owner, &body_of_file(file, mode_is_a_permission));
}

/// Whether a directory declares its own workspace, which is what makes it
/// somebody else's tree.
fn declares_a_workspace(directory: &Path) -> bool {
    let manifest = directory.join("Cargo.toml");
    std::fs::read_to_string(manifest).is_ok_and(|text| {
        text.lines()
            .any(|line| line.trim_start().starts_with("[workspace]"))
    })
}

/// Run the law over one workspace.
///
/// # Errors
///
/// When the manifest names no directory, or the tree cannot be walked.
pub fn run(manifest: &Path) -> Result<Report, String> {
    // A RELATIVE `Cargo.toml` HAS AN EMPTY PARENT, not a missing one, and walking
    // `""` fails with a message about a path nobody typed.
    let root = match manifest.parent() {
        Some(parent) if parent.as_os_str().is_empty() => PathBuf::from("."),
        Some(parent) => parent.to_path_buf(),
        None => return Err(format!("{} names no directory", manifest.display())),
    };
    let mut report = Report {
        workspace_root: root.clone(),
        coverage: Coverage::default(),
        applying: 0,
        copying: 0,
        unreadable: Vec::new(),
        unnamed_copies: Vec::new(),
        findings: Vec::new(),
    };
    walk(&root, &root, &mut report)?;
    report.findings.sort();
    report.findings.dedup();
    report.unreadable.sort();
    report.unreadable.dedup();
    report.unnamed_copies.sort();
    report.unnamed_copies.dedup();
    Ok(report)
}

fn walk(directory: &Path, root: &Path, report: &mut Report) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|e| format!("cannot read {}: {e}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read an entry of {directory:?}: {e}"))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
        // NOT FOLLOWED, AND NAMED. A symlink can leave the tree entirely or point
        // back into it, and either way what it reaches is judged where it really
        // lives — by the walk over THAT root.
        if kind.is_symlink() {
            if path.is_dir() {
                report.coverage.symlinks_not_followed.insert(path);
            }
            continue;
        }
        if kind.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name == "target" {
                report.coverage.build_artifacts += count_rust(&path);
                continue;
            }
            if name == ".git" {
                continue;
            }
            if path != root && declares_a_workspace(&path) {
                report.coverage.foreign_workspaces += count_rust(&path);
                continue;
            }
            walk(&path, root, report)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        match syn::parse_file(&text) {
            Ok(file) => {
                report.coverage.scanned.insert(path.clone());
                judge(&file, &path, report);
            }
            Err(why) => report
                .coverage
                .unparsed
                .push((path.clone(), why.to_string())),
        }
    }
    Ok(())
}

/// How much Rust sits under a directory this walk declined to open, so the
/// skipped set is a number rather than a silence.
fn count_rust(directory: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += count_rust(&path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            total += 1;
        }
    }
    total
}
