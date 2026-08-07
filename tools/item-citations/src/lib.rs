//! The decisions this gate makes, separated from the subprocesses that feed
//! them: which targets are expected, what cargo's message stream said about
//! each, and what verdict those two together support.
//!
//! Everything here is a function from bytes cargo produced to a verdict, which
//! is what makes the verdicts testable without a toolchain: the orchestration
//! in `main.rs` runs cargo, and this module decides.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// The population — what cargo says this workspace has
// ---------------------------------------------------------------------------

/// The target kinds cargo can be asked to document, one variant per selector
/// flag. `custom-build` is not here: a build script is compiled and run, never
/// documented, and it is reported as a named skip rather than dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetKind {
    Lib,
    Bin,
    Test,
    Bench,
    Example,
}

impl TargetKind {
    /// The `cargo rustdoc` flags that select exactly this target.
    #[must_use]
    pub fn selector(self, name: &str) -> Vec<String> {
        match self {
            TargetKind::Lib => vec!["--lib".to_string()],
            TargetKind::Bin => vec!["--bin".to_string(), name.to_string()],
            TargetKind::Test => vec!["--test".to_string(), name.to_string()],
            TargetKind::Bench => vec!["--bench".to_string(), name.to_string()],
            TargetKind::Example => vec!["--example".to_string(), name.to_string()],
        }
    }

    /// Whether cargo already hands this kind of target its own package's
    /// library on the rustdoc command line.
    ///
    /// It does for binaries and examples; it does NOT for integration tests or
    /// benches. That split is not a guess — `tests/gate.rs` puts all four
    /// kinds to cargo and reads the command lines it builds, because this is a
    /// claim about a tool this repository does not own. The first version of
    /// this function put benches and examples on the same side and cargo
    /// disagreed with both halves of that at once.
    ///
    /// It matters in both directions. A kind cargo already wired that is given
    /// a second `--extern` for the same library gets `E0464`, "multiple
    /// candidates", and rustdoc documents nothing; a kind cargo did not wire
    /// and that is left alone cannot resolve its own crate and rustdoc
    /// documents nothing either. Two ways to reach the same silence.
    #[must_use]
    pub fn is_wired_to_its_library(self) -> bool {
        match self {
            TargetKind::Bin | TargetKind::Example => true,
            TargetKind::Lib | TargetKind::Test | TargetKind::Bench => false,
        }
    }

    /// The cargo kind strings that mean this variant.
    ///
    /// A library target carries its crate types in the same field
    /// (`["lib", "cdylib"]` is one target, not two), so any library-ish kind
    /// collapses to [`TargetKind::Lib`]. An unrecognised kind is an ERROR
    /// rather than a default: a kind nobody here has thought about is exactly
    /// the target that would then be skipped in silence.
    fn classify(kinds: &[String]) -> Result<Option<Self>, String> {
        let mut seen: Option<TargetKind> = None;
        for kind in kinds {
            let mapped = match kind.as_str() {
                "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro" => {
                    Some(TargetKind::Lib)
                }
                "bin" => Some(TargetKind::Bin),
                "test" => Some(TargetKind::Test),
                "bench" => Some(TargetKind::Bench),
                "example" => Some(TargetKind::Example),
                "custom-build" => None,
                other => {
                    return Err(format!(
                        "cargo reports a target kind this gate has no selector for: `{other}`. \
                         Add it to `TargetKind::classify` with the flag that documents it, or \
                         name it as a skip — a kind that falls through here is a target nobody \
                         checks."
                    ))
                }
            };
            match (seen, mapped) {
                (None, next) => seen = next,
                (Some(a), Some(b)) if a == b => {}
                (Some(a), Some(b)) => {
                    return Err(format!(
                        "one target claims two different kinds ({a:?} and {b:?}); this gate \
                         documents one target per selector and cannot choose between them"
                    ))
                }
                (Some(_), None) => {
                    return Err(
                        "a build-script kind is mixed with a documentable kind on one target"
                            .to_string(),
                    )
                }
            }
        }
        Ok(seen)
    }
}

/// One documentable target, addressed the way cargo addresses it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetId {
    pub package: String,
    pub name: String,
    pub kind: TargetKind,
    /// Cargo's `test` flag for this target: does it build a test harness?
    ///
    /// This is what decides whether the SECOND authority may be consulted for
    /// this target, and it is cargo's answer rather than a rule about file
    /// paths — see [`Verdict`] for what the second authority is for.
    pub harnessed: bool,
}

impl TargetId {
    #[must_use]
    pub fn label(&self) -> String {
        format!("{} {:?} {}", self.package, self.kind, self.name)
    }
}

/// A target cargo has that this gate does not document, with the reason. The
/// rule is totality: every target in the census is either documented or named
/// here, and the names are printed on every run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedTarget {
    pub package: String,
    pub name: String,
    pub reason: String,
}

/// The full census of a workspace: what will be documented, and what will not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Census {
    pub expected: Vec<TargetId>,
    pub skipped: Vec<SkippedTarget>,
}

#[derive(Deserialize)]
struct MetadataDoc {
    packages: Vec<MetadataPackage>,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct MetadataPackage {
    id: String,
    name: String,
    targets: Vec<MetadataTarget>,
}

#[derive(Deserialize)]
struct MetadataTarget {
    kind: Vec<String>,
    name: String,
    test: bool,
}

/// Read `cargo metadata --no-deps` into the census.
///
/// `--no-deps` already restricts the package list to the workspace, and this
/// checks that rather than trusting it: a package outside `workspace_members`
/// would be a dependency's targets silently entering the population.
///
/// # Errors
///
/// If the JSON does not parse, if a package is not a workspace member, or if a
/// target carries a kind with no selector.
pub fn census(metadata_json: &str) -> Result<Census, String> {
    let doc: MetadataDoc = serde_json::from_str(metadata_json)
        .map_err(|e| format!("cargo metadata is not the JSON this gate expects: {e}"))?;
    let members: BTreeSet<&str> = doc.workspace_members.iter().map(String::as_str).collect();

    let mut expected = Vec::new();
    let mut skipped = Vec::new();
    for package in &doc.packages {
        if !members.contains(package.id.as_str()) {
            return Err(format!(
                "`{}` is in the metadata but not in `workspace_members`; this gate documents the \
                 workspace, and a dependency's targets entering the population would be checked \
                 against source this repository does not own",
                package.name
            ));
        }
        for target in &package.targets {
            match TargetKind::classify(&target.kind)? {
                Some(kind) => expected.push(TargetId {
                    package: package.name.clone(),
                    name: target.name.clone(),
                    kind,
                    harnessed: target.test,
                }),
                None => skipped.push(SkippedTarget {
                    package: package.name.clone(),
                    name: target.name.clone(),
                    reason: "build script — cargo compiles and runs it, and documents no target \
                             for it"
                        .to_string(),
                }),
            }
        }
    }
    expected.sort();
    Ok(Census { expected, skipped })
}

// ---------------------------------------------------------------------------
// What cargo said — the message stream
// ---------------------------------------------------------------------------

/// One citation that resolved to nothing, as rustdoc reported it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BrokenLink {
    pub name: String,
    pub file: String,
    pub line: u32,
}

/// Everything one `cargo rustdoc` invocation said, reduced to the three things
/// a verdict needs: which targets rustdoc actually documented, which citations
/// resolved to nothing and in which target, and whether cargo called the build
/// a success.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamReport {
    /// Keyed by (package, target name, kind-as-cargo-spells-it).
    pub documented: BTreeSet<(String, String, String)>,
    pub broken: BTreeMap<(String, String, String), Vec<BrokenLink>>,
    /// Every OTHER error rustdoc reported, rendered, keyed the same way.
    ///
    /// A target can fail to be documented for reasons that are not a citation —
    /// it may not parse, or it may name a crate rustdoc was not handed. Those
    /// errors travel in the JSON stream on stdout, not on stderr, so a gate
    /// that keeps only stderr for its diagnosis holds the summary
    /// ("could not document `x`") and none of the reason. That is exactly what
    /// the first full run of this gate did, and it cost a measurement to
    /// recover what cargo had already said.
    pub other_errors: BTreeMap<(String, String, String), Vec<String>>,
    /// The `.rmeta` cargo produced for each workspace library, keyed by package
    /// name, with the crate name rustdoc knows it by.
    pub libraries: BTreeMap<String, (String, String)>,
    /// `None` when cargo never emitted `build-finished` — which is not the same
    /// as a failure, and is the state where this gate refuses to conclude.
    pub finished: Option<bool>,
    /// Lines the stream carried that were not JSON. Cargo's own errors arrive
    /// on stderr rather than here, so this staying empty is expected; it is
    /// kept because a line this gate could not read is not a line it may drop.
    pub unparsed: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "reason")]
enum CargoMessage {
    #[serde(rename = "compiler-artifact")]
    Artifact {
        package_id: String,
        target: MessageTarget,
        filenames: Vec<String>,
    },
    #[serde(rename = "compiler-message")]
    Message {
        package_id: String,
        target: MessageTarget,
        message: Diagnostic,
    },
    #[serde(rename = "build-finished")]
    BuildFinished { success: bool },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct MessageTarget {
    kind: Vec<String>,
    name: String,
}

#[derive(Deserialize)]
struct Diagnostic {
    level: String,
    message: String,
    #[serde(default)]
    rendered: Option<String>,
    #[serde(default)]
    code: Option<DiagnosticCode>,
    #[serde(default)]
    spans: Vec<DiagnosticSpan>,
}

#[derive(Deserialize)]
struct DiagnosticCode {
    code: String,
}

#[derive(Deserialize)]
struct DiagnosticSpan {
    file_name: String,
    line_start: u32,
    is_primary: bool,
}

/// The lint whose verdict this gate is. Named once, and compared against the
/// code cargo puts in the diagnostic rather than against the rendered text, so
/// a reworded rustdoc message cannot quietly empty this gate.
pub const LINT: &str = "rustdoc::broken_intra_doc_links";

/// Which package a `package_id` belongs to, for the ids cargo emits in the
/// message stream (`path+file:///…/crates/foo#0.1.0`, and the older
/// `foo 0.1.0 (path+file:///…)`).
fn package_of(package_id: &str, known: &BTreeSet<String>) -> Option<String> {
    known
        .iter()
        .find(|name| {
            package_id
                .split(['#', '/', ' '])
                .any(|segment| segment == name.as_str() || segment.starts_with(&format!("{name}@")))
        })
        .cloned()
}

/// Reduce a `--message-format=json` stream to what the verdicts need.
///
/// A `compiler-artifact` whose filenames include an `index.html` is a
/// DOCUMENTATION unit: cargo emits artifacts for compiled units too, and the
/// two are told apart by what they produced rather than by assuming an order.
/// This is the gate's evidence of reach, and it is cargo's own word — the
/// alternative, deriving `target/doc/<name>/index.html` here and looking for
/// the file, is a second derivation of a path cargo already told us.
#[must_use]
pub fn read_stream(stdout: &str, known_packages: &BTreeSet<String>) -> StreamReport {
    let mut report = StreamReport::default();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let message: CargoMessage = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(_) => {
                report.unparsed.push(line.to_string());
                continue;
            }
        };
        match message {
            CargoMessage::Artifact {
                package_id,
                target,
                filenames,
            } => {
                let Some(package) = package_of(&package_id, known_packages) else {
                    continue;
                };
                if first_kind(&target.kind) == "lib" {
                    if let Some(rmeta) = filenames.iter().find(|f| f.ends_with(".rmeta")) {
                        report
                            .libraries
                            .insert(package.clone(), (target.name.clone(), rmeta.clone()));
                    }
                }
                if !filenames.iter().any(|f| f.ends_with("index.html")) {
                    continue;
                }
                report
                    .documented
                    .insert((package, target.name, first_kind(&target.kind)));
            }
            CargoMessage::Message {
                package_id,
                target,
                message,
            } => {
                if message.level != "error" {
                    continue;
                }
                let Some(package) = package_of(&package_id, known_packages) else {
                    continue;
                };
                if message.code.as_ref().map(|c| c.code.as_str()) != Some(LINT) {
                    report
                        .other_errors
                        .entry((package, target.name, first_kind(&target.kind)))
                        .or_default()
                        .push(message.rendered.unwrap_or(message.message));
                    continue;
                }
                let span = message
                    .spans
                    .iter()
                    .find(|s| s.is_primary)
                    .or_else(|| message.spans.first());
                report
                    .broken
                    .entry((package, target.name, first_kind(&target.kind)))
                    .or_default()
                    .push(BrokenLink {
                        name: backticked(&message.message).unwrap_or(message.message.clone()),
                        file: span.map_or_else(|| "?".to_string(), |s| s.file_name.clone()),
                        line: span.map_or(0, |s| s.line_start),
                    });
            }
            CargoMessage::BuildFinished { success } => report.finished = Some(success),
            CargoMessage::Other => {}
        }
    }
    report
}

fn first_kind(kinds: &[String]) -> String {
    kinds.first().cloned().unwrap_or_default()
}

/// The name between the first pair of backticks, which is where rustdoc puts
/// the citation it could not resolve (`unresolved link to `Foo``).
fn backticked(message: &str) -> Option<String> {
    let open = message.find('`')?;
    let rest = &message[open + 1..];
    let close = rest.find('`')?;
    Some(rest[..close].to_string())
}

// ---------------------------------------------------------------------------
// The second authority — names rustdoc structurally cannot see
// ---------------------------------------------------------------------------

/// The test names a target's harness lists, from `-- --list`.
///
/// This is the gate's SECOND authority, and it exists because rustdoc and the
/// test harness see disjoint halves of one target's item space. A `#[test]`
/// function is expanded away unless the crate is compiled in test mode, which
/// rustdoc never does, so a citation naming a sibling test — the "a law citing
/// the law next to it" idiom this repository's test prose is written in — is
/// unresolved for rustdoc no matter what flags it is given, while it is the
/// harness's whole job to know that name.
///
/// Consulting it does not loosen the law. The law is still "every citation
/// names something that exists"; the machine that can answer for `#[test]`
/// items is simply a different machine, and it is asked only for the names the
/// first one could not resolve.
#[must_use]
pub fn harness_names(list_output: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in list_output.lines() {
        let Some((path, kind)) = line.rsplit_once(": ") else {
            continue;
        };
        if kind != "test" && kind != "benchmark" {
            continue;
        }
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        names.insert(path.to_string());
        if let Some(last) = path.rsplit("::").next() {
            names.insert(last.to_string());
        }
    }
    names
}

// ---------------------------------------------------------------------------
// The verdict
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// rustdoc documented the target and every citation in it resolved.
    Clean,
    /// Every unresolved citation names a test the harness lists.
    Excused(Vec<BrokenLink>),
    /// At least one citation names nothing either authority knows.
    Defect(Vec<BrokenLink>),
    /// The gate will not say. An answer it cannot support is worse than no
    /// answer, because "no unresolved links" and "never opened" print the
    /// same.
    Indecisive(String),
}

impl Verdict {
    #[must_use]
    pub fn is_failure(&self) -> bool {
        matches!(self, Verdict::Defect(_) | Verdict::Indecisive(_))
    }
}

/// Decide one target from the evidence about it.
///
/// `harness` is `None` when the target has no test harness to ask, and
/// `Some(names)` when it was asked — never "not asked because we assumed it
/// would not help".
#[must_use]
pub fn judge(
    documented: bool,
    broken: &[BrokenLink],
    harness: Option<&BTreeSet<String>>,
) -> Verdict {
    match (documented, broken.is_empty()) {
        (true, true) => Verdict::Clean,
        (false, true) => Verdict::Indecisive(
            "rustdoc emitted neither documentation for this target nor an unresolved citation in \
             it — the target was not reached, and an unreached target is not a clean one"
                .to_string(),
        ),
        (true, false) => Verdict::Indecisive(
            "cargo reported this target as documented AND reported unresolved citations in it; \
             the lint is denied, so a documented target cannot carry one"
                .to_string(),
        ),
        (false, false) => {
            let Some(names) = harness else {
                return Verdict::Defect(broken.to_vec());
            };
            let (excused, unknown): (Vec<_>, Vec<_>) = broken
                .iter()
                .cloned()
                .partition(|link| names.contains(&link.name));
            if unknown.is_empty() {
                Verdict::Excused(excused)
            } else {
                Verdict::Defect(unknown)
            }
        }
    }
}

/// Whether the run as a whole holds together.
///
/// This is the invariant the ARM-B measurement of Round 1078 was caught by, and
/// it is the reason it is a type rather than a habit. Trying to refute the
/// claim that rustdoc cannot see `#[test]` items, that measurement ran rustdoc
/// with a nightly-only flag on a stable toolchain: rustdoc refused the flag,
/// documented nothing, reported ZERO unresolved citations, and exited non-zero.
/// Counting the citations alone said "clean". Counting the exit status alone
/// said "failed". Only the two TOGETHER say what happened, which is that the
/// gate never ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Coherence {
    Coherent,
    /// Cargo's verdict and what the gate saw do not explain each other.
    Unexplained(String),
}

/// Check cargo's verdict on the whole build against what the gate could see.
///
/// `links_seen` counts every citation rustdoc refused, whether it was later
/// excused or not: an excused target still made rustdoc fail, so a failed build
/// is EXPLAINED by it.
#[must_use]
pub fn coherence(finished: Option<bool>, links_seen: usize, indecisive: usize) -> Coherence {
    match finished {
        None => Coherence::Unexplained(
            "cargo never reported a finished build; whatever this run measured, it was not this \
             workspace's citations"
                .to_string(),
        ),
        Some(false) if links_seen == 0 && indecisive == 0 => Coherence::Unexplained(
            "cargo failed the build while the gate saw neither a refused citation nor an \
             unreached target to explain it — something stopped rustdoc before it could answer, \
             and 'nothing was reported' is not 'nothing is wrong'"
                .to_string(),
        ),
        Some(true) if links_seen > 0 => Coherence::Unexplained(format!(
            "cargo called the build a success while rustdoc reported {links_seen} refused \
             citation(s); the lint is denied, so those two cannot both be true"
        )),
        Some(_) => Coherence::Coherent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_with(targets: &str) -> String {
        format!(
            r#"{{"packages":[{{"id":"path+file:///w/p#0.1.0","name":"p","targets":[{targets}]}}],
                "workspace_members":["path+file:///w/p#0.1.0"]}}"#
        )
    }

    fn target_json(kind: &str, name: &str, test: bool) -> String {
        format!(r#"{{"kind":["{kind}"],"name":"{name}","test":{test}}}"#)
    }

    #[test]
    fn the_census_documents_every_kind_but_the_build_script_and_names_that_one() {
        let json = metadata_with(&format!(
            "{},{},{},{}",
            target_json("lib", "p", false),
            target_json("bin", "p", true),
            target_json("test", "smoke", true),
            target_json("custom-build", "build-script-build", false),
        ));
        let census = census(&json).expect("census");
        assert_eq!(
            census
                .expected
                .iter()
                .map(|t| (t.kind, t.name.as_str(), t.harnessed))
                .collect::<Vec<_>>(),
            vec![
                (TargetKind::Lib, "p", false),
                (TargetKind::Bin, "p", true),
                (TargetKind::Test, "smoke", true),
            ]
        );
        assert_eq!(census.skipped.len(), 1);
        assert_eq!(census.skipped[0].name, "build-script-build");
        assert!(census.skipped[0].reason.contains("build script"));
    }

    #[test]
    fn only_the_kinds_cargo_already_wires_are_kept_from_the_extern_repair() {
        assert!(TargetKind::Bin.is_wired_to_its_library());
        assert!(TargetKind::Example.is_wired_to_its_library());
        assert!(!TargetKind::Bench.is_wired_to_its_library());
        assert!(!TargetKind::Test.is_wired_to_its_library());
        assert!(!TargetKind::Lib.is_wired_to_its_library());
    }

    #[test]
    fn a_library_target_that_carries_several_crate_types_is_still_one_target() {
        let json = metadata_with(r#"{"kind":["lib","cdylib"],"name":"p","test":false}"#);
        let census = census(&json).expect("census");
        assert_eq!(census.expected.len(), 1);
        assert_eq!(census.expected[0].kind, TargetKind::Lib);
    }

    #[test]
    fn a_kind_with_no_selector_stops_the_gate_rather_than_being_skipped() {
        let json = metadata_with(&target_json("some-future-kind", "x", false));
        let error = census(&json).expect_err("an unknown kind must not fall through");
        assert!(error.contains("some-future-kind"), "{error}");
    }

    #[test]
    fn a_package_outside_the_workspace_members_is_refused() {
        let json = r#"{"packages":[{"id":"registry+x#1.0.0","name":"dep",
            "targets":[{"kind":["lib"],"name":"dep","test":false}]}],
            "workspace_members":["path+file:///w/p#0.1.0"]}"#;
        let error = census(json).expect_err("a dependency must not enter the population");
        assert!(error.contains("workspace_members"), "{error}");
    }

    fn packages() -> BTreeSet<String> {
        ["p".to_string()].into_iter().collect()
    }

    #[test]
    fn a_doc_artifact_is_reach_and_a_compiled_artifact_is_not() {
        let stream = concat!(
            r#"{"reason":"compiler-artifact","package_id":"path+file:///w/p#0.1.0",
                "target":{"kind":["lib"],"name":"p"},
                "filenames":["/w/target/debug/deps/libp.rmeta"]}"#,
            "\n",
            r#"{"reason":"compiler-artifact","package_id":"path+file:///w/p#0.1.0",
                "target":{"kind":["test"],"name":"smoke"},
                "filenames":["/w/target/doc/smoke/index.html"]}"#,
            "\n",
            r#"{"reason":"build-finished","success":true}"#,
        )
        .replace('\n', "\n");
        let report = read_stream(&one_line_json(&stream), &packages());
        assert_eq!(
            report.documented,
            [("p".to_string(), "smoke".to_string(), "test".to_string())]
                .into_iter()
                .collect()
        );
        assert_eq!(report.finished, Some(true));
        assert!(report.unparsed.is_empty(), "{:?}", report.unparsed);
        // The compiled artifact is not reach, but it IS where the library's
        // metadata is, which is the `--extern` cargo does not pass to a test
        // target's documentation unit.
        assert_eq!(
            report.libraries.get("p"),
            Some(&(
                "p".to_string(),
                "/w/target/debug/deps/libp.rmeta".to_string()
            ))
        );
    }

    #[test]
    fn an_error_that_is_not_the_gates_lint_is_kept_as_the_reason_a_target_failed() {
        let stream = one_line_json(&format!(
            "{}\n{}",
            diagnostic("error", LINT, "Missing", 12),
            r#"{"reason":"compiler-message","package_id":"path+file:///w/p#0.1.0",
                "target":{"kind":["test"],"name":"other"},
                "message":{"level":"error","message":"unresolved import `p`",
                "rendered":"error[E0432]: unresolved import `p`\n --> tests/other.rs:1:5\n",
                "code":{"code":"E0432"},"spans":[]}}"#,
        ));
        let report = read_stream(&stream, &packages());
        let key = ("p".to_string(), "other".to_string(), "test".to_string());
        let said = report.other_errors.get(&key).expect("the reason is kept");
        assert!(said[0].contains("E0432"), "{said:?}");
        assert!(
            !report.broken.contains_key(&key),
            "an import error is not a citation defect"
        );
    }

    /// The literals above are written across lines for reading; cargo emits one
    /// JSON object per line, so they are folded back before being parsed.
    fn one_line_json(pretty: &str) -> String {
        pretty
            .lines()
            .map(str::trim)
            .fold(String::new(), |mut acc, line| {
                if line.starts_with('{') && !acc.is_empty() {
                    acc.push('\n');
                }
                acc.push_str(line);
                acc
            })
    }

    fn diagnostic(level: &str, code: &str, name: &str, line: u32) -> String {
        format!(
            r#"{{"reason":"compiler-message","package_id":"path+file:///w/p#0.1.0",
                "target":{{"kind":["test"],"name":"smoke"}},
                "message":{{"level":"{level}","message":"unresolved link to `{name}`",
                "code":{{"code":"{code}"}},
                "spans":[{{"file_name":"tests/smoke.rs","line_start":{line},"is_primary":true}}]}}}}"#
        )
    }

    #[test]
    fn only_the_denied_lint_at_error_level_becomes_a_broken_citation() {
        let stream = one_line_json(&format!(
            "{}\n{}\n{}",
            diagnostic("error", LINT, "Missing", 12),
            diagnostic("warning", "rustdoc::private_intra_doc_links", "Private", 13),
            diagnostic("error", "rustdoc::invalid_html_tags", "id", 14),
        ));
        let report = read_stream(&stream, &packages());
        let links = report
            .broken
            .get(&("p".to_string(), "smoke".to_string(), "test".to_string()))
            .expect("the denied lint");
        assert_eq!(
            links,
            &vec![BrokenLink {
                name: "Missing".to_string(),
                file: "tests/smoke.rs".to_string(),
                line: 12,
            }]
        );
    }

    #[test]
    fn a_line_that_is_not_json_is_kept_rather_than_dropped() {
        let report = read_stream("not json at all\n", &packages());
        assert_eq!(report.unparsed, vec!["not json at all".to_string()]);
        assert_eq!(report.finished, None);
    }

    #[test]
    fn the_harness_list_yields_both_the_path_and_its_last_segment() {
        let names = harness_names(
            "surface::counted::the_density_is_a_fraction: test\n\
             lonely: test\n\
             something_else: benchmark\n\
             3 tests, 0 benchmarks\n",
        );
        assert!(names.contains("surface::counted::the_density_is_a_fraction"));
        assert!(names.contains("the_density_is_a_fraction"));
        assert!(names.contains("lonely"));
        assert!(names.contains("something_else"));
        assert!(!names.contains("3 tests, 0 benchmarks"));
    }

    fn link(name: &str) -> BrokenLink {
        BrokenLink {
            name: name.to_string(),
            file: "tests/smoke.rs".to_string(),
            line: 1,
        }
    }

    #[test]
    fn documented_and_silent_is_the_only_clean_verdict() {
        assert_eq!(judge(true, &[], None), Verdict::Clean);
    }

    #[test]
    fn a_target_that_was_never_reached_is_not_a_clean_one() {
        let verdict = judge(false, &[], None);
        let Verdict::Indecisive(why) = verdict else {
            panic!("an unreached target must not pass: {verdict:?}");
        };
        assert!(why.contains("not reached"), "{why}");
    }

    #[test]
    fn documented_while_carrying_a_denied_lint_is_a_contradiction_not_a_pass() {
        let verdict = judge(true, &[link("Missing")], None);
        assert!(matches!(verdict, Verdict::Indecisive(_)), "{verdict:?}");
    }

    #[test]
    fn without_a_harness_to_ask_every_unresolved_citation_is_a_defect() {
        assert_eq!(
            judge(false, &[link("Missing")], None),
            Verdict::Defect(vec![link("Missing")])
        );
    }

    #[test]
    fn a_failed_build_that_nothing_the_gate_saw_explains_is_the_arm_b_trap() {
        // Zero refused citations and a failed build: rustdoc never answered.
        assert!(matches!(
            coherence(Some(false), 0, 0),
            Coherence::Unexplained(_)
        ));
        // The same failed build IS explained once a refused citation is in hand,
        // including one that goes on to be excused — an excused target still
        // made rustdoc fail.
        assert_eq!(coherence(Some(false), 1, 0), Coherence::Coherent);
        // And explained by an unreached target, which is reported on its own.
        assert_eq!(coherence(Some(false), 0, 1), Coherence::Coherent);
    }

    #[test]
    fn a_run_cargo_never_finished_is_never_coherent() {
        assert!(matches!(coherence(None, 0, 0), Coherence::Unexplained(_)));
        assert!(matches!(coherence(None, 3, 0), Coherence::Unexplained(_)));
    }

    #[test]
    fn a_successful_build_carrying_refused_citations_is_a_contradiction() {
        assert_eq!(coherence(Some(true), 0, 0), Coherence::Coherent);
        assert!(matches!(
            coherence(Some(true), 1, 0),
            Coherence::Unexplained(_)
        ));
    }

    #[test]
    fn the_harness_excuses_the_names_it_lists_and_no_others() {
        let names: BTreeSet<String> = ["a_real_test".to_string()].into_iter().collect();
        assert_eq!(
            judge(false, &[link("a_real_test")], Some(&names)),
            Verdict::Excused(vec![link("a_real_test")])
        );
        assert_eq!(
            judge(
                false,
                &[link("a_real_test"), link("a_stale_name")],
                Some(&names)
            ),
            Verdict::Defect(vec![link("a_stale_name")])
        );
    }
}
