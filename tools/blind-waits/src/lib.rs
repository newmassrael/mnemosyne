//! The gate for what a test asks of the machine rather than of the code.
//!
//! # The law
//!
//! **In test code, a wait must end on a condition, and the budget that bounds
//! it must be named.**
//!
//! Two clauses, two defects:
//!
//! - [`Defect::BlindWait`] — a `sleep` that is not the retry step of a loop.
//!   Nothing re-checks anything after it, so the verdict of every assertion
//!   downstream is a function of whether the machine got there in time. That is
//!   a claim about the runner. `bench` shipped three of them and one turned
//!   main red; this repository's root workspace shipped four more, and the
//!   ledger carried the question of whether it did — unasked — for six rounds.
//! - [`Defect::SpelledBudget`] — a wait budget written as a literal at the wait
//!   site. A bound whose only job is to turn a hang into a failure is one
//!   decision; spelled at each site it becomes N decisions, none of them
//!   reviewed, and `500` shows up in six places with nobody able to say which
//!   of them was ever measured against a loaded four-core runner.
//!
//! # Why it is a parser and not a pattern
//!
//! A text search cannot tell a `sleep` inside a poll loop from a `sleep` that
//! IS the synchronisation — and the difference is the entire law. It also
//! cannot tell test code from shipped code, where sleeping is behaviour rather
//! than a claim. `syn` answers both from the same tree the compiler reads.
//!
//! # Why it reports what it reached
//!
//! An empty finding list is what a clean workspace looks like AND what a
//! workspace nobody opened looks like. Round 1078 produced the second while
//! believing the first (a nightly-only flag made rustdoc refuse to run and
//! report zero unresolved links), so a gate here states its own reach and
//! [`Report::verdict`] REFUSES rather than passing when the reach is empty or
//! partial.

use ci_plan::issue::{self, Tree};
use proc_macro2::LineColumn;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned as _;

/// What the gate rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Defect {
    /// A `sleep` in test code that nothing re-checks a condition around —
    /// either outside any loop, or inside one that can only end by running out
    /// of iterations. `for _ in 0..3 { sleep(100ms) }` is the second shape and
    /// it is the same blind three hundred milliseconds as the first.
    BlindWait,
    /// A wait budget spelled as a literal at the wait site.
    SpelledBudget,
}

impl Defect {
    /// The one-line statement of what is wrong, for the gate's own output.
    pub fn why(self) -> &'static str {
        match self {
            Defect::BlindWait => {
                "a wait no condition ends — every assertion after it is a claim \
                 that this machine got there in time"
            }
            Defect::SpelledBudget => {
                "a wait budget spelled at the site — the machine assumption is \
                 written here instead of named once"
            }
        }
    }

    /// What to do instead. Printed with every finding, because a gate that
    /// says only "no" is one people route around.
    pub fn fix(self) -> &'static str {
        match self {
            Defect::BlindWait => {
                "poll: loop { if <condition> { break } sleep(<interval>) } \
                 bounded by a named budget — or establish the happens-after \
                 the system already gives you and drop the wait entirely"
            }
            Defect::SpelledBudget => {
                "name it: `const <NAME>: Duration = ...` beside the tests that \
                 share it, and pass the name here"
            }
        }
    }
}

impl fmt::Display for Defect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Defect::BlindWait => f.write_str("blind wait"),
            Defect::SpelledBudget => f.write_str("spelled budget"),
        }
    }
}

/// One rejected site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Path as the gate was given it — relative to the workspace root when the
    /// caller passed a relative manifest, which is what makes the output
    /// paste-able into an editor.
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    /// The callee as written, e.g. `tokio::time::sleep`.
    pub callee: String,
    pub defect: Defect,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{} {} `{}` — {}",
            self.file.display(),
            self.line,
            self.column,
            self.defect,
            self.callee,
            self.defect.why()
        )
    }
}

/// What the walk opened, and what it did not. Every `.rs` file under the
/// workspace root lands in exactly one of these counters: a file that is in
/// none of them is a file this gate silently skipped, which is the failure mode
/// it exists to make impossible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Files parsed and walked.
    pub scanned: Vec<PathBuf>,
    /// Files under a `target/` directory — cargo's output, not this
    /// repository's source.
    pub build_artifacts: usize,
    /// Files under a nested directory that declares its own `[workspace]`.
    /// They are checked by pointing this gate at THAT manifest, which is what
    /// `scripts/check-side-workspaces.sh` does; counting them here as skipped
    /// keeps "every file is accounted for" true without checking them twice.
    pub foreign_workspaces: Vec<PathBuf>,
    /// Files under the workspace root that no member package claims. Scanned
    /// anyway — a file nobody owns is exactly where an unwatched wait lives —
    /// and reported so the ownership gap is visible rather than inferred.
    pub unowned: Vec<PathBuf>,
    /// Files that failed to parse. Any of these is a refusal: the gate did not
    /// read them and must not report on them.
    pub unparsed: Vec<(PathBuf, String)>,
}

/// Counts the gate states about itself, so a zero is a measurement rather than
/// an absence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reach {
    /// Files classified as test code (a test/bench target's source, anything
    /// under a `tests/` or `benches/` directory, or a `#[cfg(test)]` module).
    pub test_context_files: usize,
    /// Functions walked while in test context.
    pub test_context_fns: usize,
    /// Wait sites seen in test context, defective or not. The denominator the
    /// findings are a numerator of.
    pub wait_sites: usize,
}

/// The gate's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub workspace_root: PathBuf,
    pub coverage: Coverage,
    pub reach: Reach,
    pub findings: Vec<Finding>,
}

/// Why the gate declined to give a verdict at all.
///
/// Separate from "defects found", because the two demand different actions and
/// look identical in an exit code alone. R1078 shipped a run that reported zero
/// unresolved citations because rustdoc had refused to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The walk opened no Rust at all.
    NothingScanned,
    /// A file could not be parsed, so part of the tree is unread.
    Unparsed(Vec<(PathBuf, String)>),
}

// A workspace holding Rust but NO test code is deliberately not in here. It
// was, for one run, and three of `git_hooks_smoke.rs`'s cases went red: the
// hook rejected commits to fixture trees for a reason that has nothing to do
// with this law. "No test code" is a COMPLETE answer — the law is about test
// code, and where there is none it is satisfied — whereas "no Rust at all" and
// "a file I could not read" are the gate failing to reach the tree. A gate that
// rejects for reasons outside its own law is the shape R1079 named: one that
// gets routed around. What R1054 requires is that the two answers LOOK
// different, and they do — the reach line prints `0 files` and the verdict line
// says so in words.

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NothingScanned => f.write_str(
                "no .rs file was opened — a clean answer and an answer about \
                 nothing are the same answer, so this is not one",
            ),
            Refusal::Unparsed(files) => {
                writeln!(
                    f,
                    "{} file(s) did not parse, so the gate read only part of \
                     the tree:",
                    files.len()
                )?;
                for (path, err) in files {
                    writeln!(f, "    {}: {err}", path.display())?;
                }
                Ok(())
            }
        }
    }
}

impl Report {
    /// `Ok(())` when the workspace obeys the law, `Err(Refusal)` when the gate
    /// could not judge. Findings are read separately: a report can be
    /// judgeable AND defective.
    pub fn verdict(&self) -> Result<(), Refusal> {
        if !self.coverage.unparsed.is_empty() {
            return Err(Refusal::Unparsed(self.coverage.unparsed.clone()));
        }
        if self.coverage.scanned.is_empty() {
            return Err(Refusal::NothingScanned);
        }
        Ok(())
    }

    /// Did the walk find any test code to apply the law to? A `false` here is a
    /// verdict, not a refusal — but it is one a caller has to be able to SAY,
    /// because "nothing to check" and "checked and clean" are the same silence
    /// otherwise.
    pub fn found_test_code(&self) -> bool {
        self.reach.test_context_files > 0
    }
}

// --- the census: which directories this workspace owns ----------------------

#[derive(Debug, Deserialize)]
struct Metadata {
    workspace_root: PathBuf,
    packages: Vec<MetaPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MetaPackage {
    id: String,
    manifest_path: PathBuf,
    targets: Vec<MetaTarget>,
}

#[derive(Debug, Deserialize)]
struct MetaTarget {
    kind: Vec<String>,
    src_path: PathBuf,
}

/// Where the workspace is and what it owns, straight from cargo. The member
/// list is asked for rather than derived from the directory layout, because a
/// member can live anywhere its manifest says and a walk cannot know that.
#[derive(Debug, Clone)]
pub struct Census {
    pub workspace_root: PathBuf,
    /// Directory of each member package's manifest.
    pub member_dirs: Vec<PathBuf>,
    /// Source roots of every target cargo builds in test mode.
    pub test_target_srcs: BTreeSet<PathBuf>,
}

/// Ask cargo where the workspace is and what belongs to it.
///
/// WHOSE TREE THIS IS, IS THE CALLER'S TO SAY (R1262): the side-workspace gate
/// points this at this repository's own manifests and the hook tests point it at
/// fixture trees they built a moment earlier, so no declaration made here could
/// be true of both. What holds instead is stronger — `--no-deps` means the
/// census RESOLVES NOTHING, measured against cargo in
/// `locked_resolution_smoke`, so there is no lockfile for it to rewrite whoever
/// points it where.
pub fn census(manifest: &Path) -> Result<Census, String> {
    let output = issue::cargo(Tree::WhereverTheCallerPoints(
        "the side gate points this at this repository and the hook tests point \
         it at the fixture trees they build",
    ))
    .args(["metadata", "--format-version", "1", "--no-deps"])
    .arg("--manifest-path")
    .arg(manifest)
    .output()
    .map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed for {}: {}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let meta: Metadata = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata is not the JSON this expects: {e}"))?;

    let members: BTreeSet<&String> = meta.workspace_members.iter().collect();
    let mut member_dirs = Vec::new();
    let mut test_target_srcs = BTreeSet::new();
    for package in &meta.packages {
        if !members.contains(&package.id) {
            continue;
        }
        if let Some(dir) = package.manifest_path.parent() {
            member_dirs.push(dir.to_path_buf());
        }
        for target in &package.targets {
            // `test` and `bench` are the kinds whose whole source is test code.
            // `lib` / `bin` / `example` are shipped code where sleeping is
            // behaviour; their test code is the `#[cfg(test)]` modules, which
            // the walk finds by attribute rather than by target kind.
            if target.kind.iter().any(|k| k == "test" || k == "bench") {
                test_target_srcs.insert(target.src_path.clone());
            }
        }
    }
    Ok(Census {
        workspace_root: meta.workspace_root,
        member_dirs,
        test_target_srcs,
    })
}

// --- the walk ---------------------------------------------------------------

/// Run the gate over the workspace whose manifest this is.
pub fn run(manifest: &Path) -> Result<Report, String> {
    let census = census(manifest)?;
    let mut coverage = Coverage::default();
    collect(&census.workspace_root, &census, &mut coverage, true)?;
    coverage.scanned.sort();
    coverage.foreign_workspaces.sort();
    coverage.unowned.sort();

    let mut reach = Reach::default();
    let mut findings = Vec::new();
    for path in &coverage.scanned {
        let source = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                coverage.unparsed.push((path.clone(), e.to_string()));
                continue;
            }
        };
        match syn::parse_file(&source) {
            Ok(file) => {
                let file_is_test = is_test_source(path, &census);
                let mut walk = Walk::new(path.clone(), file_is_test);
                syn::visit::visit_file(&mut walk, &file);
                if file_is_test || walk.saw_test_context {
                    reach.test_context_files += 1;
                }
                reach.test_context_fns += walk.test_context_fns;
                reach.wait_sites += walk.wait_sites;
                findings.extend(walk.findings);
            }
            Err(e) => coverage.unparsed.push((path.clone(), e.to_string())),
        }
    }
    findings.sort_by(|a, b| {
        (&a.file, a.line, a.column, a.defect).cmp(&(&b.file, b.line, b.column, b.defect))
    });

    Ok(Report {
        workspace_root: census.workspace_root.clone(),
        coverage,
        reach,
        findings,
    })
}

/// Depth-first over the tree, classifying every `.rs` file into exactly one
/// bucket of [`Coverage`].
fn collect(
    dir: &Path,
    census: &Census,
    coverage: &mut Coverage,
    is_root: bool,
) -> Result<(), String> {
    if !is_root && declares_a_workspace(dir) {
        collect_foreign(dir, coverage)?;
        return Ok(());
    }
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("could not read {}: {e}", dir.display()))?;
    let mut names: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("could not read an entry of {}: {e}", dir.display()))?;
        names.push(entry.path());
    }
    names.sort();
    for path in names {
        if path.is_symlink() {
            // A symlink out of the tree would be counted under a root that
            // does not contain it. Nothing here uses one; if something starts
            // to, this gate says so rather than following it.
            continue;
        }
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" {
                coverage.build_artifacts += count_rust(&path)?;
                continue;
            }
            if name == ".git" {
                continue;
            }
            collect(&path, census, coverage, false)?;
        } else if is_rust(&path) {
            if census
                .member_dirs
                .iter()
                .any(|member| path.starts_with(member))
            {
                coverage.scanned.push(path);
            } else {
                coverage.unowned.push(path.clone());
                coverage.scanned.push(path);
            }
        }
    }
    Ok(())
}

fn collect_foreign(dir: &Path, coverage: &mut Coverage) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("could not read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("could not read an entry of {}: {e}", dir.display()))?;
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" {
                coverage.build_artifacts += count_rust(&path)?;
                continue;
            }
            if name == ".git" {
                continue;
            }
            collect_foreign(&path, coverage)?;
        } else if is_rust(&path) {
            coverage.foreign_workspaces.push(path);
        }
    }
    Ok(())
}

fn count_rust(dir: &Path) -> Result<usize, String> {
    let mut total = 0;
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // A `target/` that vanished under the walk is cargo, not this tree.
        Err(_) => return Ok(0),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            total += count_rust(&path)?;
        } else if is_rust(&path) {
            total += 1;
        }
    }
    Ok(total)
}

fn is_rust(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("rs")
}

/// The same rule `scripts/check-side-workspaces.sh` discovers workspaces with:
/// a manifest whose own line says `[workspace]`. Kept identical on purpose —
/// two definitions of "is this a separate workspace" drift, and the shell one
/// is what decides which manifests this gate is pointed at.
fn declares_a_workspace(dir: &Path) -> bool {
    let manifest = dir.join("Cargo.toml");
    match std::fs::read_to_string(&manifest) {
        Ok(text) => text.lines().any(|line| line.trim_end() == "[workspace]"),
        Err(_) => false,
    }
}

/// Whole-file test code: a `test` or `bench` target's own source, or anything
/// under a `tests/` or `benches/` directory (where the shared `common` modules
/// live — they are not targets, and R1074's sweep moved real computation into
/// one of them).
fn is_test_source(path: &Path, census: &Census) -> bool {
    if census.test_target_srcs.contains(path) {
        return true;
    }
    path.components().any(|c| {
        let name = c.as_os_str();
        name == "tests" || name == "benches"
    })
}

// --- the parse --------------------------------------------------------------

/// Callees that wait without asking anything. `yield_now` is here for the same
/// reason the sleeps are: written straight-line in a test it means "let the
/// other task get somewhere first", which is a claim about the scheduler.
const SLEEPS: &[&str] = &[
    "sleep",
    "sleep_ms",
    "sleep_until",
    "park_timeout",
    "yield_now",
];

/// Callees that wait FOR something, with a budget, and which argument carries
/// that budget. These are the good shape — the budget only has to be named.
const BUDGETED: &[(&str, usize)] = &[
    ("timeout", 0),
    ("timeout_at", 0),
    ("recv_timeout", 0),
    ("wait_timeout", 1),
    ("wait_timeout_while", 1),
    ("set_read_timeout", 0),
    ("set_write_timeout", 0),
];

struct Walk {
    file: PathBuf,
    file_is_test: bool,
    cfg_test_depth: usize,
    test_fn_depth: usize,
    /// One entry per enclosing loop, saying whether that loop can end on
    /// something other than running out of iterations. A `for _ in 0..3 {
    /// sleep(100ms) }` is inside a loop and is still a blind three-hundred
    /// milliseconds: the law says a wait ends on a CONDITION, and a count is
    /// not one. `enclosed_by_a_poll` is what reads this.
    loops: Vec<bool>,
    saw_test_context: bool,
    test_context_fns: usize,
    wait_sites: usize,
    findings: Vec<Finding>,
}

impl Walk {
    fn new(file: PathBuf, file_is_test: bool) -> Self {
        Walk {
            file,
            file_is_test,
            cfg_test_depth: 0,
            test_fn_depth: 0,
            loops: Vec::new(),
            saw_test_context: false,
            test_context_fns: 0,
            wait_sites: 0,
            findings: Vec::new(),
        }
    }

    /// Is this site the retry step of something that can stop early? ANY
    /// enclosing loop having a way out is enough: a counted inner loop nested
    /// in a real poll is still governed by the poll's condition.
    fn enclosed_by_a_poll(&self) -> bool {
        self.loops.iter().any(|way_out| *way_out)
    }

    fn in_test_context(&self) -> bool {
        self.file_is_test || self.cfg_test_depth > 0 || self.test_fn_depth > 0
    }

    fn record(&mut self, at: LineColumn, callee: String, defect: Defect) {
        self.findings.push(Finding {
            file: self.file.clone(),
            line: at.line,
            column: at.column + 1,
            callee,
            defect,
        });
    }
}

impl<'ast> syn::visit::Visit<'ast> for Walk {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let gated = node.attrs.iter().any(is_cfg_test);
        if gated {
            self.cfg_test_depth += 1;
            self.saw_test_context = true;
        }
        syn::visit::visit_item_mod(self, node);
        if gated {
            self.cfg_test_depth -= 1;
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let marked = node.attrs.iter().any(is_test_attribute);
        if marked {
            self.test_fn_depth += 1;
            self.saw_test_context = true;
        }
        if self.in_test_context() {
            self.test_context_fns += 1;
        }
        // A nested `fn` is not lexically inside the enclosing loop's body for
        // any execution that matters: it cannot capture, and it runs when it is
        // called. Its own loops are its own.
        let outer_loops = std::mem::take(&mut self.loops);
        syn::visit::visit_item_fn(self, node);
        self.loops = outer_loops;
        if marked {
            self.test_fn_depth -= 1;
        }
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let marked = node.attrs.iter().any(is_test_attribute);
        if marked {
            self.test_fn_depth += 1;
            self.saw_test_context = true;
        }
        if self.in_test_context() {
            self.test_context_fns += 1;
        }
        let outer_loops = std::mem::take(&mut self.loops);
        syn::visit::visit_impl_item_fn(self, node);
        self.loops = outer_loops;
        if marked {
            self.test_fn_depth -= 1;
        }
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.loops.push(can_stop_early(&node.body));
        syn::visit::visit_expr_loop(self, node);
        self.loops.pop();
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        // The head IS the condition, so a `while` always has a way out.
        self.loops.push(true);
        syn::visit::visit_expr_while(self, node);
        self.loops.pop();
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.loops.push(can_stop_early(&node.body));
        syn::visit::visit_expr_for_loop(self, node);
        self.loops.pop();
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if self.in_test_context() {
            if let syn::Expr::Path(path) = node.func.as_ref() {
                let name = last_segment(&path.path);
                let written = render_path(&path.path);
                let at = path.path.span().start();
                if SLEEPS.contains(&name.as_str()) {
                    self.wait_sites += 1;
                    if !self.enclosed_by_a_poll() {
                        self.record(at, written.clone(), Defect::BlindWait);
                    }
                }
                if let Some(index) = budget_argument(&name) {
                    self.wait_sites += 1;
                    if let Some(argument) = node.args.iter().nth(index) {
                        if holds_a_literal(argument) {
                            self.record(at, written, Defect::SpelledBudget);
                        }
                    }
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.in_test_context() {
            let name = node.method.to_string();
            if SLEEPS.contains(&name.as_str()) {
                self.wait_sites += 1;
                if !self.enclosed_by_a_poll() {
                    self.record(node.method.span().start(), name.clone(), Defect::BlindWait);
                }
            }
            if let Some(index) = budget_argument(&name) {
                self.wait_sites += 1;
                // A method's receiver is its first argument in spirit but not
                // in `args`, so the table's index counts from the receiver and
                // 0 means the receiver itself (never a budget).
                let index = index.saturating_sub(1);
                if let Some(argument) = node.args.iter().nth(index) {
                    if holds_a_literal(argument) {
                        self.record(node.method.span().start(), name, Defect::SpelledBudget);
                    }
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        // `Instant::now() + <budget>` is a deadline, which is a wait budget
        // wearing a different shape. `tools/injection-harness` computes one.
        if self.in_test_context() && matches!(node.op, syn::BinOp::Add(_)) && calls_now(&node.left)
        {
            self.wait_sites += 1;
            if holds_a_literal(&node.right) {
                self.record(
                    node.op.span().start(),
                    "Instant::now() + …".to_string(),
                    Defect::SpelledBudget,
                );
            }
        }
        syn::visit::visit_expr_binary(self, node);
    }
}

/// Can this loop body end the loop on something other than running out of
/// iterations? A `break`, a `return` or a `?` is a way out; a body with none of
/// them turns `for _ in 0..3 { sleep(100ms) }` into a blind three hundred
/// milliseconds wearing a loop.
///
/// Deliberately permissive about WHICH loop a `break` leaves (an unlabelled one
/// leaves the innermost): a gate that rejects a real poll is a gate people work
/// around, and the shape this is aimed at has no way out at all.
fn can_stop_early(body: &syn::Block) -> bool {
    struct Search {
        found: bool,
    }
    impl<'ast> syn::visit::Visit<'ast> for Search {
        fn visit_expr_break(&mut self, node: &'ast syn::ExprBreak) {
            self.found = true;
            syn::visit::visit_expr_break(self, node);
        }
        fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
            self.found = true;
            syn::visit::visit_expr_return(self, node);
        }
        fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
            self.found = true;
            syn::visit::visit_expr_try(self, node);
        }
    }
    let mut search = Search { found: false };
    syn::visit::visit_block(&mut search, body);
    search.found
}

fn budget_argument(name: &str) -> Option<usize> {
    BUDGETED
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, index)| *index)
}

fn last_segment(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}

fn render_path(path: &syn::Path) -> String {
    let mut rendered = String::new();
    if path.leading_colon.is_some() {
        rendered.push_str("::");
    }
    for (index, segment) in path.segments.iter().enumerate() {
        if index > 0 {
            rendered.push_str("::");
        }
        rendered.push_str(&segment.ident.to_string());
    }
    rendered
}

fn calls_now(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(call) => match call.func.as_ref() {
            syn::Expr::Path(path) => last_segment(&path.path) == "now",
            _ => false,
        },
        syn::Expr::Paren(inner) => calls_now(&inner.expr),
        _ => false,
    }
}

/// Does this expression spell a number anywhere inside it? `Duration::from_millis(500)`
/// does; `LIVENESS` does not; `LIVENESS * 2` does, and that is the intent — a
/// budget derived from a named one by an arithmetic nobody reviewed is the same
/// decision made twice.
fn holds_a_literal(expr: &syn::Expr) -> bool {
    struct Search {
        found: bool,
    }
    impl<'ast> syn::visit::Visit<'ast> for Search {
        fn visit_lit(&mut self, node: &'ast syn::Lit) {
            if matches!(node, syn::Lit::Int(_) | syn::Lit::Float(_)) {
                self.found = true;
            }
            syn::visit::visit_lit(self, node);
        }
    }
    let mut search = Search { found: false };
    syn::visit::visit_expr(&mut search, expr);
    search.found
}

fn is_cfg_test(attr: &syn::Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    let mut names_test = false;
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("test") {
            names_test = true;
        }
        // `cfg(all(test, feature = "x"))` nests; walking into it is what makes
        // the common form work rather than only the bare one.
        if meta.input.peek(syn::token::Paren) {
            let _ = meta.parse_nested_meta(|inner| {
                if inner.path.is_ident("test") {
                    names_test = true;
                }
                Ok(())
            });
        }
        Ok(())
    });
    names_test
}

fn is_test_attribute(attr: &syn::Attribute) -> bool {
    let path = attr.path();
    let last = path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();
    last == "test" || last == "bench"
}
