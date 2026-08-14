//! The law: a path derived from the process-wide temp root names the process.
//!
//! # What the root actually is
//!
//! `std::env::temp_dir()` is not this program's directory. It is the machine's,
//! shared by every process on it, and this repository's evidence is produced by
//! runs on a machine three repositories share. Joining a fixed name onto that
//! root yields a path that is not per-run and not per-test — it is per-MACHINE —
//! and the code that builds one almost always removes it too. Two runs then
//! delete each other's fixtures, and the failure arrives as a `NotFound` on a
//! write that follows its own `create_dir_all`, which reads like a flake and is
//! not one: the code is correct alone and wrong the moment anything else runs it.
//!
//! Round 1175 met exactly that, repaired six sites by reading every `temp_dir()`
//! use across the side workspaces, and wrote down what it could not do: NOTHING
//! STOPS THE SEVENTH ONE. This is that.
//!
//! # Why the question is asked of a function
//!
//! The two spellings in this repository put the root and the name on different
//! lines:
//!
//! ```text
//! let dir = std::env::temp_dir().join(format!("thing-{}", std::process::id()));
//!
//! let mut path = std::env::temp_dir();
//! path.push(format!("thing-{name}-{}", std::process::id()));
//! ```
//!
//! An expression-chain rule reads the first and not the second, and a gate that
//! refuses the tidier spelling of the shape it enforces is one people route
//! around. So the unit is the FUNCTION: one that reaches for the shared root
//! must also say `std::process::id()`. That is weaker than "the pid is in this
//! path" and it is the strongest thing checkable without following a value
//! through statements — and its failure is never a false alarm, because a
//! function that never says `process::id` is certainly not putting it in a path.
//!
//! # What it does not see, said rather than hidden
//!
//! A helper that returns the bare root for a caller to name would be reported,
//! because the naming is not in it. No such helper exists here; if one arrives,
//! the honest repair is a hop, not an exemption. `tempfile::TempDir` is not the
//! subject at all: it creates a uniquely-named directory, which is this law's
//! conclusion reached by construction.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// A function that reaches the shared temp root and never names the process.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    /// The function the reach sits in, or `<file>` when it sits outside one.
    pub owner: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` builds a path from the shared temp root and never names the process",
            self.owner
        )
    }
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
    /// SKIPPED: in this repository `target` is a symlink to a shared build
    /// cache, and the first draft of this walk fell through to its extension
    /// check and passed over it without a word — so the artifact count printed
    /// zero, which reads as "there are none" and meant "nobody looked".
    pub symlinks_not_followed: BTreeSet<PathBuf>,
    /// Files that failed to parse. Any of these is a refusal.
    pub unparsed: Vec<(PathBuf, String)>,
}

/// The gate's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub workspace_root: PathBuf,
    pub coverage: Coverage,
    /// Functions that reach the shared root — the population the findings are a
    /// numerator of. Without it a clean tree and an unread one print the same
    /// number.
    pub reaching: usize,
    /// Of those, the ones that name the process.
    pub owning: usize,
    pub findings: Vec<Finding>,
}

impl Report {
    /// Whether the gate read enough to have an opinion at all.
    ///
    /// # Errors
    ///
    /// When nothing was opened, or a file did not parse — a tree read in part is
    /// a tree whose clean answer means nothing.
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
        Ok(())
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

/// Every line in one body that calls the shared temp root, and whether that body
/// says `process::id` anywhere.
#[derive(Default)]
struct Reach {
    roots: Vec<usize>,
    names_the_process: bool,
}

/// The one walk, driven from either entry point below.
#[derive(Default)]
struct Walk(Reach);

impl<'ast> syn::visit::Visit<'ast> for Walk {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref() {
            // `std::env::temp_dir()`, `env::temp_dir()`, `temp_dir()`.
            if ends_with(&path.path, &["temp_dir"]) {
                self.0
                    .roots
                    .push(syn::spanned::Spanned::span(call).start().line);
            }
            // `std::process::id()` — BOTH segments, so a bare `id()` on some
            // other type is not mistaken for the process's.
            if ends_with(&path.path, &["process", "id"]) {
                self.0.names_the_process = true;
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    // ⚠ THE PID IS ALMOST ALWAYS INSIDE A MACRO, and `syn` does not walk a
    // macro's body. Every one of the eleven sites this gate reported on its
    // first run was a FALSE alarm for exactly that reason: they all spell the
    // name `format!("thing-{}", std::process::id())`, and the visitor above
    // sees `format!` as one opaque token stream. Round 1186 met the same
    // blindness from the other side, where a law asserted that test names
    // generated by a macro were absent from a tree that contained them.
    //
    // So the tokens are read. It is weaker than syntax — a `process :: id`
    // sequence anywhere in the invocation counts — and it is the strongest
    // reading available, since expanding the macro is what a walk cannot do.
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        scan_tokens(invocation.tokens.clone(), &mut self.0);
        syn::visit::visit_macro(self, invocation);
    }
}

/// Read one token stream for the two things this law is about.
///
/// Each group is scanned in its OWN flat sequence rather than a globally
/// flattened one, so `process :: id` has to be adjacent where it was written
/// instead of across a bracket that happened to sit between them.
fn scan_tokens(tokens: proc_macro2::TokenStream, reach: &mut Reach) {
    use proc_macro2::TokenTree;

    let flat: Vec<TokenTree> = tokens.into_iter().collect();
    for (i, tree) in flat.iter().enumerate() {
        match tree {
            TokenTree::Group(group) => scan_tokens(group.stream(), reach),
            TokenTree::Ident(ident) => {
                if ident == "temp_dir" {
                    reach.roots.push(ident.span().start().line);
                }
                if ident == "id" {
                    // `process :: id`, by tokens: the two colons are separate
                    // `Punct`s, so the identifier is three places back.
                    let owner = i.checked_sub(3).and_then(|at| flat.get(at));
                    let colon = i.checked_sub(1).and_then(|at| flat.get(at));
                    if matches!(owner, Some(TokenTree::Ident(name)) if name == "process")
                        && matches!(colon, Some(TokenTree::Punct(p)) if p.as_char() == ':')
                    {
                        reach.names_the_process = true;
                    }
                }
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

fn reach_of_block(block: &syn::Block) -> Reach {
    let mut walk = Walk::default();
    syn::visit::visit_block(&mut walk, block);
    walk.0
}

fn reach_of_file(file: &syn::File) -> Reach {
    let mut walk = Walk::default();
    syn::visit::visit_file(&mut walk, file);
    walk.0
}

/// Judge one parsed file.
///
/// The unit is the innermost function each reach sits in; a reach outside every
/// function is owned by the file, which is stated in the finding rather than
/// skipped.
pub fn judge(file: &syn::File, path: &Path, report: &mut Report) {
    struct Functions<'a> {
        path: &'a Path,
        report: &'a mut Report,
        /// Lines already accounted for by an inner function, so a nested one is
        /// not reported twice by the function that contains it.
        claimed: BTreeSet<usize>,
    }
    impl Functions<'_> {
        fn take(&mut self, owner: String, block: &syn::Block) {
            let reach = reach_of_block(block);
            let fresh: Vec<usize> = reach
                .roots
                .iter()
                .copied()
                .filter(|line| !self.claimed.contains(line))
                .collect();
            if fresh.is_empty() {
                return;
            }
            self.report.reaching += 1;
            if reach.names_the_process {
                self.report.owning += 1;
            }
            for line in fresh {
                self.claimed.insert(line);
                if !reach.names_the_process {
                    self.report.findings.push(Finding {
                        file: self.path.to_path_buf(),
                        line,
                        owner: owner.clone(),
                    });
                }
            }
        }
    }
    impl<'ast> syn::visit::Visit<'ast> for Functions<'_> {
        // DEPTH FIRST, so the innermost function claims its own lines before the
        // one containing it is asked. Otherwise a helper nested inside a correct
        // function would inherit that function's `process::id` and its own
        // omission would never be reported.
        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            syn::visit::visit_item_fn(self, item);
            self.take(item.sig.ident.to_string(), &item.block);
        }

        fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
            syn::visit::visit_impl_item_fn(self, item);
            self.take(item.sig.ident.to_string(), &item.block);
        }
    }

    let mut functions = Functions {
        path,
        report,
        claimed: BTreeSet::new(),
    };
    syn::visit::visit_file(&mut functions, file);

    // A reach outside every function — a `const` initialiser, a macro-free
    // static. Owned by the file, and named as such rather than dropped.
    let whole = reach_of_file(file);
    let leftover: Vec<usize> = whole
        .roots
        .iter()
        .copied()
        .filter(|line| !functions.claimed.contains(line))
        .collect();
    if leftover.is_empty() {
        return;
    }
    report.reaching += 1;
    if whole.names_the_process {
        report.owning += 1;
        return;
    }
    for line in leftover {
        report.findings.push(Finding {
            file: path.to_path_buf(),
            line,
            owner: format!("<{}>", path.display()),
        });
    }
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
    // A RELATIVE `Cargo.toml` HAS AN EMPTY PARENT, not a missing one, and
    // walking `""` fails with a message about a path nobody typed. The callers
    // in this repository all pass an absolute manifest; being wrong for the one
    // that does not is a trap, so the empty parent is read as what it means.
    let root = match manifest.parent() {
        Some(parent) if parent.as_os_str().is_empty() => PathBuf::from("."),
        Some(parent) => parent.to_path_buf(),
        None => return Err(format!("{} names no directory", manifest.display())),
    };
    let mut report = Report {
        workspace_root: root.clone(),
        coverage: Coverage::default(),
        reaching: 0,
        owning: 0,
        findings: Vec::new(),
    };
    walk(&root, &root, &mut report)?;
    report.findings.sort();
    report.findings.dedup();
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
        // NOT FOLLOWED, AND NAMED. A symlink can leave the tree entirely or
        // point back into it, and either way what it reaches is judged where it
        // really lives — by the walk over THAT root.
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
