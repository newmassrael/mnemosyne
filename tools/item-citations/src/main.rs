//! Run the item-citation gate over a cargo workspace.
//!
//!     item-citations [--workspace <path to Cargo.toml or its directory>]
//!
//! Every target cargo lists is documented, every citation rustdoc could not
//! resolve is either excused by the target's own test harness or reported as a
//! defect, and the run says how many targets it REACHED — because "no
//! unresolved citations" and "never opened that target" print the same, and
//! this repository has now shipped the second one four times.
//!
//! Exit 0 clean, 1 a citation names no item, 2 the gate has no verdict — it
//! could not be started, or it started and could not finish reading this
//! workspace. Those last two are ONE code because they are one answer to a
//! caller: there is nothing here to go fix, ask again. See
//! [`item_citations::Answer`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Output;

use ci_plan::issue::{self, Tree};

use item_citations::{
    answer, census, coherence, harness_names, judge, read_stream, Answer, BrokenLink, Census,
    Coherence, DevOnlyDependency, OmittedExterns, StreamReport, TargetId, TargetKind, Verdict,
    LINT,
};

/// The flags every rustdoc invocation in this gate runs under.
///
/// `--document-private-items` because the documentation in this repository is
/// written for an agent reading the internals, not only for a downstream
/// consumer: without it, a citation between two private items resolves against
/// nothing and the gate would report a defect for prose that is correct.
const RUSTDOC_FLAGS: &str = "--document-private-items -D rustdoc::broken_intra_doc_links";

fn main() {
    match run() {
        Ok(Answer::Clean) => {}
        Ok(other) => std::process::exit(other.exit_code()),
        Err(message) => {
            eprintln!("[item-citations] {message}");
            std::process::exit(Answer::CouldNotJudge.exit_code());
        }
    }
}

struct Args {
    manifest: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut manifest = PathBuf::from("Cargo.toml");
    let mut argv = std::env::args().skip(1);
    while let Some(argument) = argv.next() {
        match argument.as_str() {
            "--workspace" => {
                let value = argv
                    .next()
                    .ok_or_else(|| "--workspace needs a path".to_string())?;
                manifest = PathBuf::from(value);
            }
            other => {
                return Err(format!(
                    "unknown argument `{other}`; usage: item-citations [--workspace <path>]"
                ))
            }
        }
    }
    if manifest.is_dir() {
        manifest = manifest.join("Cargo.toml");
    }
    if !manifest.is_file() {
        return Err(format!(
            "{} is not a manifest — this gate is pointed at the workspace to check",
            manifest.display()
        ));
    }
    Ok(Args { manifest })
}

fn run() -> Result<Answer, String> {
    let args = parse_args()?;
    let manifest = args
        .manifest
        .canonicalize()
        .map_err(|e| format!("{}: {e}", args.manifest.display()))?;
    let root = manifest
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", manifest.display()))?
        .to_path_buf();

    println!("[item-citations] workspace {}", manifest.display());
    println!("[item-citations] rustdoc flags: {RUSTDOC_FLAGS}");

    let census = collect_census(&root, &manifest)?;
    report_census(&census);

    let packages: BTreeSet<String> = census
        .expected
        .iter()
        .map(|target| target.package.clone())
        .collect();
    if census.expected.is_empty() {
        return Err(
            "this workspace has no documentable target at all — a green run over an \
                    empty population is the failure this gate exists to prevent"
                .to_string(),
        );
    }

    let mut stream = StreamReport::default();
    let mut logs: BTreeMap<String, String> = BTreeMap::new();
    for package in &packages {
        let targets: Vec<TargetId> = census
            .expected
            .iter()
            .filter(|target| &target.package == package)
            .cloned()
            .collect();
        let dev_only = census.dev_only.get(package).cloned().unwrap_or_default();
        let (report, log) =
            document_package(&root, &manifest, package, &packages, &targets, &dev_only)?;
        merge(&mut stream, report);
        logs.insert(package.clone(), log);
    }

    let verdicts = decide(&root, &manifest, &census, &stream, &logs)?;
    Ok(report_verdicts(&census, &verdicts, &stream, &logs))
}

fn collect_census(root: &Path, manifest: &Path) -> Result<Census, String> {
    let output = cargo(
        root,
        &[
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
            &manifest.display().to_string(),
        ],
        None,
    )?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    census(&String::from_utf8_lossy(&output.stdout))
}

fn report_census(census: &Census) {
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for target in &census.expected {
        *by_kind.entry(format!("{:?}", target.kind)).or_default() += 1;
    }
    let breakdown = by_kind
        .iter()
        .map(|(kind, count)| format!("{count} {kind}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "[item-citations] census: {} documentable targets ({breakdown})",
        census.expected.len()
    );
    for skip in &census.skipped {
        println!(
            "[item-citations] SKIP {} {} — {}",
            skip.package, skip.name, skip.reason
        );
    }
}

/// Ask cargo where one package's library metadata landed.
///
/// This exists because of a hole measured in this gate's first full run: cargo
/// does NOT hand a package's own library to the documentation unit of that
/// package's INTEGRATION TESTS. The rustdoc command line it builds carries an
/// `--extern` for every dependency and none for the crate under test, so any
/// test that writes `use mnemosyne_atomic::…` fails to resolve its own crate
/// and rustdoc documents nothing — 29 of this workspace's 106 test targets, in
/// silence except for `could not document`.
///
/// The missing `--extern` is added back, and every part of it comes from cargo:
/// the crate name is the library target's name in the metadata (already
/// spelled the way rustdoc knows it), and the path is the `.rmeta` cargo says
/// it produced. Nothing about a target directory layout is derived here.
///
/// The check runs for ONE package rather than the workspace, so the features it
/// resolves are the ones the documentation run resolves. What it cannot match
/// is a feature a DEV-dependency turns on, which is in the test targets' graph
/// and not in this one; if that ever bites, it bites as a citation reported
/// unresolved — loudly, and never as a silent pass.
fn library_of(
    root: &Path,
    manifest: &Path,
    package: &str,
    known: &BTreeSet<String>,
) -> Result<Option<(String, String)>, String> {
    let output = cargo(
        root,
        &[
            "check",
            "-q",
            "--locked",
            "--manifest-path",
            &manifest.display().to_string(),
            "-p",
            package,
            "--message-format=json",
        ],
        None,
    )?;
    if !output.status.success() {
        return Err(format!(
            "`{package}` does not check, so nothing can be said about the citations in it:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let report = read_stream(&String::from_utf8_lossy(&output.stdout), known);
    Ok(report.libraries.get(package).cloned())
}

/// The `--extern` flags for the dependencies a package declares only for
/// development, which cargo omits from a BENCH target's documentation unit.
///
/// Every part comes from cargo. Which dependencies are dev-only is the
/// manifest's answer, read from `cargo metadata`; where each one's metadata
/// landed and what crate name rustdoc knows it by are the artifact records of a
/// `cargo check` that builds the bench graph. A dev-only dependency that
/// resolves to no artifact, or to more than one, stops the gate rather than
/// being guessed at: a wrong `--extern` does not fail loudly, it silently
/// resolves a citation against the wrong crate.
fn dev_only_externs(
    root: &Path,
    manifest: &Path,
    package: &str,
    dev_only: &[DevOnlyDependency],
) -> Result<String, String> {
    let output = cargo(
        root,
        &[
            "check",
            "-q",
            "--locked",
            "--manifest-path",
            &manifest.display().to_string(),
            "-p",
            package,
            "--benches",
            "--message-format=json",
        ],
        None,
    )?;
    if !output.status.success() {
        return Err(format!(
            "`{package}`'s benches do not check, so nothing can be said about the citations in \
             them:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut flags = String::new();
    for dependency in dev_only {
        let found = lib_artifacts_of(&stdout, &dependency.package);
        let (crate_name, rmeta) = match found.as_slice() {
            [one] => one.clone(),
            [] => {
                return Err(format!(
                    "`{package}` declares `{}` only as a dev-dependency, and cargo's check of its \
                     benches produced no library for it — this gate cannot document a bench \
                     target without putting that crate back",
                    dependency.package
                ))
            }
            many => {
                return Err(format!(
                    "`{}` resolves to {} different libraries in `{package}`'s bench graph; this \
                     gate will not choose one, because the wrong choice resolves citations \
                     against the wrong crate in silence",
                    dependency.package,
                    many.len()
                ))
            }
        };
        let spelled = dependency.renamed_to.clone().unwrap_or(crate_name);
        flags.push_str(&format!(" --extern {spelled}={rmeta}"));
    }
    Ok(flags)
}

/// Every distinct (crate name, `.rmeta`) a package produced a library artifact
/// for in one message stream.
fn lib_artifacts_of(stdout: &str, package: &str) -> Vec<(String, String)> {
    let known: BTreeSet<String> = [package.to_string()].into_iter().collect();
    let mut found: BTreeSet<(String, String)> = BTreeSet::new();
    for (_, (crate_name, rmeta)) in read_stream(stdout, &known).libraries {
        found.insert((crate_name, rmeta));
    }
    found.into_iter().collect()
}

/// Document every target of one package.
///
/// TWO invocations, and which target goes in which is the whole of what the
/// first full run of this gate had to learn:
///
///   - Binaries, examples and benches ALREADY receive their package's library
///     from cargo. Handing them a second `--extern` for it is `E0464`, "multiple
///     candidates", and rustdoc then documents nothing — which is how three
///     binaries came back unreached from a run whose repair for the test
///     targets was working perfectly.
///   - Test targets receive nothing, and are the reason the extern exists.
///
/// `--keep-going` on both, because a gate that stops at the first failing
/// target reports a smaller number than the truth and somebody fixes to it —
/// the same correction `scripts/check-side-workspaces.sh` had to make when it
/// said `bench` had 6 failures and there were 18.
///
/// The rustdoc flags travel in `RUSTDOCFLAGS` rather than after `--`, because
/// cargo refuses trailing rustdoc arguments when more than one target is
/// selected — which is what makes any whole-package form possible at all.
///
/// WHAT THE EXTERN COSTS, stated rather than left to be found: it reaches every
/// unit of its invocation, the library's own documentation included. A citation
/// inside the library that names its own crate by its EXTERNAL path
/// (`mnemosyne_atomic::Foo` rather than `crate::Foo`) therefore resolves here
/// where the compiler would not accept it in code. That is a looser rule about
/// PATHS, not about existence: such a citation still names an item that exists,
/// which is the whole of what this gate is.
fn document_package(
    root: &Path,
    manifest: &Path,
    package: &str,
    known: &BTreeSet<String>,
    targets: &[TargetId],
    dev_only: &[DevOnlyDependency],
) -> Result<(StreamReport, String), String> {
    // ONE SELECTOR PER TARGET, never a kind-wide flag. `--tests` does not mean
    // "the test targets": cargo reads it as "every target with `test = true`",
    // which includes the library and the binaries, and `--benches` is the same
    // shape. Selecting a group that way puts binaries in the invocation that
    // carries the extern repair, where they are E0464 — silently, because they
    // had already been documented correctly by the other invocation.
    //
    // ONE INVOCATION PER OMITTED-EXTERN SET, because the repair travels in
    // `RUSTDOCFLAGS` and therefore reaches every target in its invocation. Two
    // targets may share one only when cargo leaves out the same things for
    // both.
    let mut groups: BTreeMap<OmittedExterns, Vec<String>> = BTreeMap::new();
    for target in targets {
        groups
            .entry(target.kind.externs_cargo_omits())
            .or_default()
            .extend(target.kind.selector(&target.name));
    }

    let needs_library = groups.keys().any(|omitted| omitted.own_library);
    let library = if needs_library {
        library_of(root, manifest, package, known)?
    } else {
        None
    };
    let needs_dev = groups.keys().any(|omitted| omitted.dev_dependencies) && !dev_only.is_empty();
    let dev_externs = if needs_dev {
        dev_only_externs(root, manifest, package, dev_only)?
    } else {
        String::new()
    };

    let mut merged = StreamReport::default();
    let mut logs = String::new();
    for (omitted, selectors) in &groups {
        let mut flags = RUSTDOC_FLAGS.to_string();
        // A package with no library has nothing to put back, and the targets in
        // this group are documented without it.
        if let (true, Some((crate_name, rmeta))) = (omitted.own_library, &library) {
            flags.push_str(&format!(" --extern {crate_name}={rmeta}"));
        }
        if omitted.dev_dependencies {
            flags.push_str(&dev_externs);
        }
        let flags = flags.as_str();
        println!(
            "[item-citations] documenting {package} {}",
            selectors.join(" ")
        );
        let manifest_arg = manifest.display().to_string();
        // `--locked` HERE TOO, and this reader cannot see it. The declaration in
        // `cargo` below covers every command through it, and the two whose words
        // are assembled at run time are the ones no law reads: `ci_plan::rust`
        // follows a hole back to a caller's LITERAL, and a list built two lines
        // earlier is not one. The flag is right whether or not anything is
        // watching, which is the whole reason it is written here.
        let mut argv: Vec<&str> = vec![
            "rustdoc",
            "--locked",
            "--manifest-path",
            &manifest_arg,
            "-p",
            package,
        ];
        argv.extend(selectors.iter().map(String::as_str));
        argv.extend(["--keep-going", "--message-format=json"]);
        let output = cargo(root, &argv, Some(flags))?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let report = read_stream(&stdout, known);
        if !report.unparsed.is_empty() {
            return Err(format!(
                "cargo's message stream for {package} carried {} line(s) this gate could not \
                 read; the first is:\n{}",
                report.unparsed.len(),
                report.unparsed[0]
            ));
        }
        merge(&mut merged, report);
        logs.push_str(&format!(
            "--- cargo stderr for {package} {} ---\n{stderr}",
            selectors.join(" ")
        ));
    }
    Ok((merged, logs))
}

fn merge(into: &mut StreamReport, from: StreamReport) {
    into.documented.extend(from.documented);
    into.libraries.extend(from.libraries);
    for (target, links) in from.broken {
        into.broken.entry(target).or_default().extend(links);
    }
    for (target, errors) in from.other_errors {
        into.other_errors.entry(target).or_default().extend(errors);
    }
    // One package's build says nothing about another's, so the run's finish is
    // the conjunction: any package cargo did not finish leaves the whole run
    // unfinished.
    into.finished = match (into.finished, from.finished) {
        (Some(a), Some(b)) => Some(a && b),
        (None, other) | (other, None) => other,
    };
}

/// The key the message stream uses for a target, which is cargo's spelling of
/// the kind rather than this gate's enum.
fn stream_key(target: &TargetId) -> (String, String, String) {
    let kind = match target.kind {
        TargetKind::Lib => "lib",
        TargetKind::Bin => "bin",
        TargetKind::Test => "test",
        TargetKind::Bench => "bench",
        TargetKind::Example => "example",
    };
    (
        target.package.clone(),
        target.name.clone(),
        kind.to_string(),
    )
}

fn decide(
    root: &Path,
    manifest: &Path,
    census: &Census,
    stream: &StreamReport,
    logs: &BTreeMap<String, String>,
) -> Result<Vec<(TargetId, Verdict)>, String> {
    let mut verdicts = Vec::new();
    for target in &census.expected {
        let key = stream_key(target);
        let documented = stream.documented.contains(&key);
        let broken: Vec<BrokenLink> = stream.broken.get(&key).cloned().unwrap_or_default();
        let harness = if !broken.is_empty() && target.harnessed {
            Some(list_harness(root, manifest, target)?)
        } else {
            None
        };
        let mut verdict = judge(documented, &broken, harness.as_ref());
        if let Verdict::Indecisive(why) = &verdict {
            // What rustdoc actually said about THIS target, which arrives in
            // the JSON stream rather than on stderr; the package's stderr is
            // the fallback, and it only ever holds the summary.
            let said = match stream.other_errors.get(&key) {
                Some(errors) => errors.join("\n"),
                None => logs.get(&target.package).cloned().unwrap_or_default(),
            };
            verdict = Verdict::Indecisive(format!("{why}\nrustdoc said:\n{said}"));
        }
        verdicts.push((target.clone(), verdict));
    }
    Ok(verdicts)
}

/// Ask a target's own test harness for the names it knows.
///
/// A failure here is fatal rather than "no excuse available": excusing nothing
/// because the authority could not be reached would turn a broken toolchain
/// into a wall of defects, and refusing to excuse without asking would turn it
/// into a silent pass. Neither is an answer.
fn list_harness(
    root: &Path,
    manifest: &Path,
    target: &TargetId,
) -> Result<BTreeSet<String>, String> {
    let manifest_arg = manifest.display().to_string();
    let mut argv: Vec<String> = vec![
        "test".to_string(),
        "-q".to_string(),
        "--locked".to_string(),
        "--manifest-path".to_string(),
        manifest_arg,
        "-p".to_string(),
        target.package.clone(),
    ];
    argv.extend(target.kind.selector(&target.name));
    argv.push("--".to_string());
    argv.push("--list".to_string());
    println!(
        "[item-citations] asking the harness of {} for the names rustdoc cannot see",
        target.label()
    );
    let borrowed: Vec<&str> = argv.iter().map(String::as_str).collect();
    let output = cargo(root, &borrowed, None)?;
    if !output.status.success() {
        return Err(format!(
            "could not ask the test harness of {} for its names, so no citation in it can be \
             excused or convicted:\n{}",
            target.label(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(harness_names(&String::from_utf8_lossy(&output.stdout)))
}

fn cargo(root: &Path, argv: &[&str], rustdoc_flags: Option<&str>) -> Result<Output, String> {
    let mut command = issue::cargo(Tree::PinnedWhereverItPoints(
        "the gate documents the workspace it was pointed at — this repository \
         under the side gate, a fixture under its own cases — and every \
         resolving command below says `--locked`, so whichever it is, a \
         lockfile that disagrees is reported rather than repaired",
    ));
    command.current_dir(root).args(argv);
    match rustdoc_flags {
        Some(flags) => command.env("RUSTDOCFLAGS", flags),
        // The gate's own flags are the only ones that may decide its verdict:
        // an inherited RUSTDOCFLAGS from the caller's environment could deny a
        // different lint set than the one this gate is.
        None => command.env_remove("RUSTDOCFLAGS"),
    };
    command
        .output()
        .map_err(|e| format!("could not run cargo {}: {e}", argv.join(" ")))
}

fn report_verdicts(
    census: &Census,
    verdicts: &[(TargetId, Verdict)],
    stream: &StreamReport,
    logs: &BTreeMap<String, String>,
) -> Answer {
    let mut clean = 0;
    let mut excused_targets = 0;
    let mut excused_links = 0;
    let mut defects = Vec::new();
    let mut indecisive = Vec::new();

    for (target, verdict) in verdicts {
        match verdict {
            Verdict::Clean => clean += 1,
            Verdict::Excused(links) => {
                excused_targets += 1;
                excused_links += links.len();
                for link in links {
                    println!(
                        "[item-citations] EXCUSED {}:{} `{}` — the harness of {} lists it; \
                         rustdoc cannot see a #[test] item",
                        link.file,
                        link.line,
                        link.name,
                        target.label()
                    );
                }
            }
            Verdict::Defect(links) => {
                for link in links {
                    println!(
                        "[item-citations] DEFECT {}:{} `{}` — no item of that name, in {}",
                        link.file,
                        link.line,
                        link.name,
                        target.label()
                    );
                }
                defects.push(target.clone());
            }
            Verdict::Indecisive(why) => {
                println!("[item-citations] INDECISIVE {} — {why}", target.label());
                indecisive.push(target.clone());
            }
        }
    }

    let reached = clean + excused_targets + defects.len();
    println!(
        "[item-citations] reached {reached}/{} targets — {clean} clean, {} with citations only \
         the harness could resolve ({excused_links} of them), {} with a defect, {} the gate \
         will not answer for",
        census.expected.len(),
        excused_targets,
        defects.len(),
        indecisive.len()
    );

    let links_seen: usize = stream.broken.values().map(Vec::len).sum();
    let coherence = coherence(stream.finished, links_seen, indecisive.len());
    if let Coherence::Unexplained(why) = &coherence {
        println!("[item-citations] UNEXPLAINED — {why}");
        for log in logs.values() {
            println!("{log}");
        }
    }

    // THE RUN'S ANSWER IS A DECISION, so it is made in one place a unit test can
    // put cases to rather than by the shape of the returns here. What it
    // corrects: `defects.is_empty() && indecisive.is_empty()` is TWO answers,
    // and this gate has three — everything below that line left as the
    // FINDING's code whether the run had found something or merely failed to
    // look, and its callers print a sentence about the finding.
    let answer = answer(defects.len(), indecisive.len(), &coherence);
    match answer {
        Answer::Clean => {
            println!("[item-citations] every item citation in this workspace names an item");
        }
        Answer::Finding => {
            println!(
                "[item-citations] fix: give the citation a name that resolves, qualify it with \
                 the path that reaches the item, or — if it was never a link — escape the \
                 brackets. The lint is {LINT}."
            );
        }
        Answer::CouldNotJudge => {
            println!(
                "[item-citations] this run carries no finding and no verdict for the targets \
                 named above — a workspace this gate could not finish reading is neither a \
                 clean one nor a guilty one, and a caller that prints it as either sends a \
                 reader hunting a citation nobody wrote"
            );
        }
    }
    answer
}
