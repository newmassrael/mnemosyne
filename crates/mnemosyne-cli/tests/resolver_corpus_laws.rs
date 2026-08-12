//! THE THREE BACKENDS WITH NO PREDECESSOR, PUT TO A CORPUS (Rounds 1157, 1161).
//!
//! Rounds 1151 to 1155 shipped five symbol-resolver backends. Two of them were
//! PORTS, and a port has an oracle that needs no invention: run the old
//! implementation beside the new one over every line of a real tree and require
//! them to agree. Rust's did (313 files, 221787 lines, 0 disagreements) and
//! C++'s did (1469 files, 235962 lines, 0 disagreements), and both harnesses were
//! deleted once they had answered.
//!
//! GO, PYTHON AND KOTLIN HAVE NO OLD ANSWER TO AGREE WITH. They shipped on unit
//! laws over hand-written shapes plus one two-site end-to-end fixture each — a
//! sample chosen by whoever wrote it, which is exactly the population a defect
//! hides outside of. Round 1153's grouped/ungrouped `type` and Round 1154's
//! class-body `block` were both found by a BATCH test, and both were shapes the
//! single-line cases could not see.
//!
//! ROUND 1157 PUT THEM TO THE CONSUMER'S TREE and Round 1160 found what that
//! cost: the tree is on ONE machine, so everywhere else — the build machine, and
//! CI — the laws printed `NOT MEASURED` and PASSED. A measurement that quietly
//! stops measuring is the shape this repository keeps paying for.
//!
//! ROUND 1161 SPLIT THE TWO JOBS THAT WERE TANGLED HERE.
//!
//! - DISCOVERY asks "what shapes exist that nobody imagined?". It needs a real
//!   tree at volume, and it is a ONE-SHOT instrument: Round 1157 ran it over
//!   174000 lines and got 0 disagreements, which is the same thing the two port
//!   harnesses did before being deleted. It stays here, but only when
//!   `MNEMOSYNE_RESOLVER_CORPUS` names a tree — and then it MUST measure.
//! - REGRESSION asks "do the engine's invariants still hold?". It must run
//!   everywhere, and for a metamorphic property a fixed sample is the weaker
//!   instrument. The corpus below is BUILT, from shapes composed in several
//!   adjacencies, and that buys the thing a vendored sample cannot give:
//!
//! THE POPULATION IS DERIVED, SO THE CORPUS CANNOT SILENTLY LAG THE BACKEND.
//! `LanguageSpec::pattern_count` is how many declaration patterns a backend's
//! query DECLARES and `patterns_exercised` is which of them a source reached —
//! both read off the compiled query rather than off a list beside it. Law 0
//! requires every pattern to be reached. Add a pattern to a backend and this
//! goes red naming the index nobody covers; a real tree could only ever report
//! what it happened to contain.
//!
//! THE LAWS
//!
//! 0. EVERY PATTERN THE BACKEND DECLARES IS EXERCISED. Non-vacuity, derived.
//! 1. BATCHING CHANGES NO ANSWER. Every line of a file in one call must equal
//!    the same lines split across two calls that INTERLEAVE them — odds in one,
//!    evens in the other, so every line's neighbours move to the other call.
//! 2. AN ANSWER IS TEXT FROM THE FILE. Every symbol returned must occur
//!    verbatim in the source it was given.
//! 3. THE ANSWER IS THE PLANTED ONE. Over the built corpus this is an ORACLE
//!    and not a floor: the composer knows which declaration covers each line, so
//!    every line is checked by name. Round 1157 could only assert
//!    `answered > lines/20` here, a constant of the kind this repository keeps
//!    catching in other people's code. Over a real tree, where nothing knows the
//!    right answer, that floor is still all there is and it stays.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use mnemosyne_core::SymbolResolver;
use mnemosyne_plugin_tree_sitter_core::LanguageSpec;

/// One declaration shape, as source that exhibits it.
///
/// `{N}` in `body` is replaced by an ordinal unique to each placement, so no two
/// copies of a shape share a name and an answer can never be right by accident.
/// `answers` is one entry per LINE of `body`: the name the resolver must give
/// for that line, or `None` where no declaration covers it.
struct Shape {
    label: &'static str,
    body: &'static str,
    answers: &'static [Option<&'static str>],
}

struct Subject {
    language: &'static str,
    spec: &'static LanguageSpec,
    resolver: fn() -> Box<dyn SymbolResolver>,
    /// Prepended to every built file. Go needs a package clause to parse as one.
    prelude: &'static str,
    shapes: &'static [Shape],
    /// Extensions this backend's language maps to, as `git ls-files` globs —
    /// used only by the discovery pass over a real tree.
    globs: &'static [&'static str],
    /// The fewest FILES a real tree must hold for the discovery pass to be about
    /// something. Unused by the built corpus, which derives its own floor.
    min_files: usize,
}

const GO_SHAPES: &[Shape] = &[
    Shape {
        label: "function_declaration",
        body: "func Fn{N}() int {\n\treturn 0\n}\n",
        answers: &[Some("Fn{N}"), Some("Fn{N}"), Some("Fn{N}")],
    },
    Shape {
        label: "method_declaration on a named receiver",
        body: "type Holder{N} struct{}\n\nfunc (h Holder{N}) Meth{N}() int {\n\treturn 1\n}\n",
        answers: &[
            Some("Holder{N}"),
            None,
            Some("Meth{N}"),
            Some("Meth{N}"),
            Some("Meth{N}"),
        ],
    },
    Shape {
        // ROUND 1153's DEFECT LIVED HERE — a grouped `type` holds several specs
        // and an ungrouped one holds exactly one, and the single-line cases
        // could not see the difference.
        label: "type_spec, grouped",
        body: "type (\n\tAlpha{N} int\n\tBeta{N} string\n)\n",
        answers: &[None, Some("Alpha{N}"), Some("Beta{N}"), None],
    },
    Shape {
        label: "type_spec, ungrouped",
        body: "type Solo{N} float64\n",
        answers: &[Some("Solo{N}")],
    },
    Shape {
        label: "const_spec",
        body: "const C{N} = 1\n",
        answers: &[Some("C{N}")],
    },
    Shape {
        label: "var_spec, grouped",
        body: "var (\n\tV{N} int\n\tW{N} bool\n)\n",
        answers: &[None, Some("V{N}"), Some("W{N}"), None],
    },
];

const PYTHON_SHAPES: &[Shape] = &[
    Shape {
        label: "function_definition",
        body: "def fn_{N}():\n    return 0\n",
        answers: &[Some("fn_{N}"), Some("fn_{N}")],
    },
    Shape {
        // ROUND 1154's DEFECT LIVED HERE — a citation inside a class body used
        // to fall out to the enclosing scope.
        label: "class_definition holding a function_definition",
        body: "class Cls{N}:\n    def meth_{N}(self):\n        return 1\n",
        answers: &[Some("Cls{N}"), Some("meth_{N}"), Some("meth_{N}")],
    },
    Shape {
        label: "decorated_definition",
        body: "@wrapper_{N}\ndef deco_{N}():\n    return 2\n",
        answers: &[Some("deco_{N}"), Some("deco_{N}"), Some("deco_{N}")],
    },
];

const KOTLIN_SHAPES: &[Shape] = &[
    Shape {
        label: "class_declaration with a member property and function",
        body: "class Cls{N} {\n    val prop{N}: Int = 1\n    fun meth{N}(): Int {\n        return 2\n    }\n}\n",
        answers: &[
            Some("Cls{N}"),
            Some("prop{N}"),
            Some("meth{N}"),
            Some("meth{N}"),
            Some("meth{N}"),
            Some("Cls{N}"),
        ],
    },
    Shape {
        label: "object_declaration",
        body: "object Obj{N} {\n    fun inner{N}() {\n    }\n}\n",
        answers: &[
            Some("Obj{N}"),
            Some("inner{N}"),
            Some("inner{N}"),
            Some("Obj{N}"),
        ],
    },
    Shape {
        label: "property_declaration at the top level",
        body: "val top{N}: Int = 3\n",
        answers: &[Some("top{N}")],
    },
    Shape {
        label: "function_declaration at the top level",
        body: "fun free{N}(): Int {\n    return 4\n}\n",
        answers: &[Some("free{N}"), Some("free{N}"), Some("free{N}")],
    },
];

const SUBJECTS: &[Subject] = &[
    Subject {
        language: "go",
        spec: &mnemosyne_plugin_tree_sitter_go::SPEC,
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_go::resolver()),
        prelude: "package corpus\n\n",
        shapes: GO_SHAPES,
        globs: &["*.go"],
        min_files: 100,
    },
    Subject {
        language: "python",
        spec: &mnemosyne_plugin_tree_sitter_python::SPEC,
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_python::resolver()),
        prelude: "",
        shapes: PYTHON_SHAPES,
        globs: &["*.py"],
        min_files: 100,
    },
    Subject {
        language: "kotlin",
        spec: &mnemosyne_plugin_tree_sitter_kotlin::SPEC,
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_kotlin::resolver()),
        prelude: "",
        shapes: KOTLIN_SHAPES,
        globs: &["*.kt", "*.kts"],
        min_files: 300,
    },
];

/// One built file: its source and the name expected at each 1-based line.
struct Built {
    label: String,
    source: String,
    expected: BTreeMap<u32, String>,
}

/// Place `shapes` one after another, in the order given, into one file.
///
/// EVERY PLACEMENT GETS ITS OWN ORDINAL, so the same shape appearing twice in a
/// file declares two different names — an answer that came from the wrong
/// placement is then a wrong NAME rather than a coincidence.
fn compose(subject: &Subject, order: &[usize], label: String) -> Built {
    let mut source = subject.prelude.to_string();
    let mut expected = BTreeMap::new();
    let mut line = source.lines().count() as u32;
    for (ordinal, &index) in order.iter().enumerate() {
        let shape = &subject.shapes[index];
        let n = ordinal.to_string();
        let body = shape.body.replace("{N}", &n);
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(
            lines.len(),
            shape.answers.len(),
            "{}: shape `{}` has {} line(s) of source and {} expected answer(s) \
             — the composer would then be checking the wrong lines",
            subject.language,
            shape.label,
            lines.len(),
            shape.answers.len()
        );
        for (offset, text) in lines.iter().enumerate() {
            source.push_str(text);
            source.push('\n');
            line += 1;
            if let Some(name) = shape.answers[offset] {
                expected.insert(line, name.replace("{N}", &n));
            }
        }
        // A blank line between placements: covered by no declaration, so it is
        // also the check that a declaration's extent stops where it should.
        source.push('\n');
        line += 1;
    }
    Built {
        label,
        source,
        expected,
    }
}

/// The built corpus for one backend.
///
/// EACH SHAPE ALONE, then all of them in order, then all of them REVERSED. The
/// two whole-file orders are what put every shape next to a different neighbour,
/// which is the adjacency law 1 is about; the isolated files are what make a
/// failure name one shape instead of a file holding six.
fn built_corpus(subject: &Subject) -> Vec<Built> {
    let mut out = Vec::new();
    for (index, shape) in subject.shapes.iter().enumerate() {
        out.push(compose(subject, &[index], format!("{} alone", shape.label)));
    }
    let forward: Vec<usize> = (0..subject.shapes.len()).collect();
    let mut backward = forward.clone();
    backward.reverse();
    out.push(compose(subject, &forward, "every shape, in order".into()));
    out.push(compose(subject, &backward, "every shape, reversed".into()));
    // Every shape twice, so a placement's ordinal is what distinguishes two
    // instances of one form standing next to each other.
    let doubled: Vec<usize> = forward.iter().chain(forward.iter()).copied().collect();
    out.push(compose(subject, &doubled, "every shape, twice".into()));
    out
}

#[test]
fn every_pattern_a_backend_declares_is_one_the_built_corpus_reaches() {
    // LAW 0 — THE NON-VACUITY THIS ROUND EXISTS FOR, and the whole reason the
    // corpus is built rather than sampled. The denominator is the compiled
    // query's own pattern count; the numerator is the set of pattern indices the
    // matcher reports. Neither is a list anybody maintains, so a backend that
    // grows a pattern reddens this until the corpus grows a shape.
    for subject in SUBJECTS {
        let declared = subject
            .spec
            .pattern_count()
            .expect("the backend's query compiles");
        assert!(
            declared > 0,
            "{}: the query declares no pattern at all, so every law below is \
             about nothing",
            subject.language
        );
        let mut reached: BTreeSet<usize> = BTreeSet::new();
        for built in built_corpus(subject) {
            reached.extend(
                subject
                    .spec
                    .patterns_exercised(&built.source)
                    .expect("the backend parses its own corpus"),
            );
        }
        let missing: Vec<usize> = (0..declared).filter(|p| !reached.contains(p)).collect();
        assert!(
            missing.is_empty(),
            "{}: the built corpus reaches {} of {declared} declared pattern(s); \
             nothing exercises pattern index {:?}. Add a shape to this file that \
             exhibits it — a pattern no corpus reaches is a claim nothing checks.",
            subject.language,
            reached.len(),
            missing
        );
    }
}

#[test]
fn the_built_corpus_answers_exactly_what_was_planted_in_it() {
    // LAWS 1, 2 AND 3 OVER THE CORPUS THE REPOSITORY CARRIES — the pass that
    // runs on every machine and in CI.
    for subject in SUBJECTS {
        // A SHAPE THAT NAMES NOTHING IS A SHAPE WHOSE ORACLE SAYS NOTHING, and
        // it would sail through the comparison below by matching an empty
        // answer with an empty expectation. Law 0 cannot catch it either: a
        // shape can exercise a pattern the backend declines to NAME.
        for shape in subject.shapes {
            assert!(
                shape.answers.iter().any(Option::is_some),
                "{}: shape `{}` expects no name on any of its lines, so it \
                 contributes nothing an oracle can be wrong about",
                subject.language,
                shape.label
            );
        }
        let resolver = (subject.resolver)();
        for built in built_corpus(subject) {
            assert!(
                !built.expected.is_empty(),
                "{} / {}: nothing was planted in this file, so comparing its \
                 answers to the plan compares two empty maps",
                subject.language,
                built.label
            );
            let n = built.source.lines().count() as u32;
            let all: Vec<u32> = (1..=n).collect();
            let whole = resolver
                .resolve_symbols_at(Path::new("/no/such/file"), &built.source, &all)
                .expect("the resolver answers");

            // LAW 1 — the same lines split across two calls that INTERLEAVE
            // them. Every line's neighbours move to the other call, so a batch
            // that let one line's resolution decide another's cannot agree.
            let odds: Vec<u32> = all.iter().copied().filter(|l| l % 2 == 1).collect();
            let evens: Vec<u32> = all.iter().copied().filter(|l| l % 2 == 0).collect();
            let mut split = resolver
                .resolve_symbols_at(Path::new("/no/such/file"), &built.source, &odds)
                .expect("the resolver answers");
            split.extend(
                resolver
                    .resolve_symbols_at(Path::new("/no/such/file"), &built.source, &evens)
                    .expect("the resolver answers"),
            );
            assert_eq!(
                whole, split,
                "{} / {}: batching changed the answer\n--- source ---\n{}",
                subject.language, built.label, built.source
            );

            // LAW 2 — an answer is text from the file it was given.
            for (line, name) in &whole {
                assert!(
                    built.source.contains(name.as_str()),
                    "{} / {}: line {line} answered `{name}`, which does not occur \
                     in the source it came from",
                    subject.language,
                    built.label
                );
            }

            // LAW 3 — AN ORACLE AND NOT A FLOOR. The composer planted every
            // declaration and knows which one covers each line, so both
            // directions are checked: a line that should answer and does not,
            // and a line that answers something the composer did not plant.
            let got: BTreeMap<u32, String> = whole.into_iter().collect();
            assert_eq!(
                got, built.expected,
                "{} / {}: the resolver's answers are not the planted ones\n\
                 --- source ---\n{}",
                subject.language, built.label, built.source
            );
        }
    }
}

/// The discovery pass — a real tree, when this machine has one.
///
/// OPT-IN AND NEVER SILENT. Round 1157 defaulted this to a checkout that exists
/// on one machine and let its absence pass with a printed line, so the laws were
/// guarded there and nowhere else; Round 1160 registered that as a debt and this
/// is its repair. The regression job now belongs to the built corpus above,
/// which runs everywhere. What is left here is the job a built corpus cannot do:
/// meet shapes nobody thought to write. When the variable names a tree, that
/// tree must be there — an unset variable is a pass because the laws above
/// already ran, and a WRONG one is a failure because it means somebody asked for
/// a measurement and did not get it.
fn discovery_corpus() -> Option<PathBuf> {
    let named = std::env::var("MNEMOSYNE_RESOLVER_CORPUS").ok()?;
    let root = PathBuf::from(named);
    assert!(
        root.join(".git").exists(),
        "MNEMOSYNE_RESOLVER_CORPUS names {} and there is no checkout there — a \
         measurement was asked for and cannot be taken, which is not the same \
         as one nobody asked for",
        root.display()
    );
    Some(root)
}

fn tracked(root: &Path, globs: &[&str]) -> Vec<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root).arg("ls-files");
    for g in globs {
        cmd.arg(g);
    }
    let out = cmd.output().expect("git ls-files");
    assert!(out.status.success(), "git ls-files failed in {root:?}");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_real_tree_meets_the_same_laws_when_this_machine_has_one() {
    let Some(root) = discovery_corpus() else {
        return;
    };
    for subject in SUBJECTS {
        let files = tracked(&root, subject.globs);
        assert!(
            files.len() >= subject.min_files,
            "{}: {} file(s) in the corpus, below the floor of {} — the corpus \
             derivation broke, not the resolver",
            subject.language,
            files.len(),
            subject.min_files
        );
        let resolver = (subject.resolver)();

        let mut read = 0usize;
        let mut lines_total = 0usize;
        let mut answered = 0usize;
        let mut batch_disagreements: Vec<String> = Vec::new();
        let mut not_in_source: Vec<String> = Vec::new();

        for rel in &files {
            let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
                continue;
            };
            let n = text.lines().count() as u32;
            if n == 0 {
                continue;
            }
            read += 1;
            let all: Vec<u32> = (1..=n).collect();
            lines_total += all.len();

            let batched = resolver
                .resolve_symbols_at(Path::new("/no/such/file"), &text, &all)
                .expect("the resolver answers");
            let odds: Vec<u32> = all.iter().copied().filter(|l| l % 2 == 1).collect();
            let evens: Vec<u32> = all.iter().copied().filter(|l| l % 2 == 0).collect();
            let mut split: BTreeMap<u32, String> = resolver
                .resolve_symbols_at(Path::new("/no/such/file"), &text, &odds)
                .expect("the resolver answers");
            split.extend(
                resolver
                    .resolve_symbols_at(Path::new("/no/such/file"), &text, &evens)
                    .expect("the resolver answers"),
            );
            answered += batched.len();
            if batched != split {
                for line in &all {
                    let (a, b) = (batched.get(line), split.get(line));
                    if a != b {
                        batch_disagreements.push(format!("{rel}:{line} whole={a:?} split={b:?}"));
                    }
                }
            }

            for (line, name) in &batched {
                if !text.contains(name.as_str()) {
                    not_in_source.push(format!("{rel}:{line} answered `{name}`"));
                }
            }
        }

        println!(
            "{}: {read} file(s), {lines_total} line(s), {answered} answered, \
             {} batch disagreement(s), {} answer(s) absent from their source",
            subject.language,
            batch_disagreements.len(),
            not_in_source.len()
        );

        // A FLOOR AND NOT AN ORACLE, and only here: nothing knows what a real
        // tree's right answers are, which is exactly why the built corpus exists.
        assert!(
            answered > lines_total / 20,
            "{}: only {answered} of {lines_total} line(s) answered — a resolver \
             that reaches almost nothing satisfies the other laws for free",
            subject.language
        );
        assert!(
            batch_disagreements.is_empty(),
            "{}: {} line(s) where batching changed the answer, first 20:\n  {}",
            subject.language,
            batch_disagreements.len(),
            batch_disagreements
                .iter()
                .take(20)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n  ")
        );
        assert!(
            not_in_source.is_empty(),
            "{}: {} answer(s) do not occur in the source they came from, first \
             20:\n  {}",
            subject.language,
            not_in_source.len(),
            not_in_source
                .iter()
                .take(20)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n  ")
        );
    }
}
