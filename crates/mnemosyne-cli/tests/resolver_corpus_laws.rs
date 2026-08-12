//! THE THREE BACKENDS WITH NO PREDECESSOR, PUT TO A REAL CORPUS (Round 1157).
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
//! single-line cases could not see; nothing said what the rest of a real file
//! looks like.
//!
//! WHAT A CORPUS CAN SAY WITHOUT A PREDECESSOR — three laws, and each is a
//! statement about the engine rather than about a language's vocabulary:
//!
//! 1. BATCHING CHANGES NO ANSWER. Every line of a file in one call must equal
//!    the same lines split across two calls that INTERLEAVE them — odds in one,
//!    evens in the other, so every line's neighbours move to the other call.
//!    The unit tests assert the per-line form over six-line fixtures; a per-line
//!    call has no neighbours to be confused by, which is why this asks the
//!    sharper question over the whole corpus instead (and at three parses per
//!    file rather than one per line: the first version of this cost 891s).
//!
//! 2. AN ANSWER IS TEXT FROM THE FILE. Every symbol the resolver returns must
//!    occur verbatim in the source it was given. A resolver that invents,
//!    truncates or mis-slices a name fails this and cannot fail law 1.
//!
//! 3. THE REACH IS A NUMBER. How many lines each backend answers, counted and
//!    asserted above a floor derived from the corpus rather than typed in. An
//!    empty answer is what a clean run and a resolver that reached nothing both
//!    look like.
//!
//! THE CORPUS IS THE CONSUMER'S TREE, which is the only real Go / Python /
//! Kotlin on this machine and — not incidentally — the tree these backends were
//! built for. It is read, never written. When it is absent this contract FAILS
//! rather than skipping: a measurement that quietly stops measuring is the
//! shape Round 1156 spent a round on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use mnemosyne_core::SymbolResolver;

/// One backend, its corpus, and the floor its reach must clear.
struct Subject {
    language: &'static str,
    /// Extensions this backend's language maps to, as `git ls-files` globs.
    globs: &'static [&'static str],
    resolver: fn() -> Box<dyn SymbolResolver>,
    /// The fewest FILES the corpus must hold for the laws below to be about
    /// something. Derived from a count taken when this was written, kept well
    /// under it so a tree that legitimately shrinks does not fail the wrong law.
    min_files: usize,
}

const SUBJECTS: &[Subject] = &[
    Subject {
        language: "go",
        globs: &["*.go"],
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_go::resolver()),
        min_files: 100,
    },
    Subject {
        language: "python",
        globs: &["*.py"],
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_python::resolver()),
        min_files: 100,
    },
    Subject {
        language: "kotlin",
        globs: &["*.kt", "*.kts"],
        resolver: || Box::new(mnemosyne_plugin_tree_sitter_kotlin::resolver()),
        min_files: 300,
    },
];

/// The consumer's tree — the only real corpus for these three on this machine.
fn corpus_root() -> PathBuf {
    PathBuf::from("/home/coin/scxml-core-engine")
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
fn every_backend_holds_its_laws_over_a_real_corpus_of_its_own_language() {
    let root = corpus_root();
    assert!(
        root.join(".git").exists(),
        "the corpus is missing at {} — this contract FAILS rather than skipping, \
         because a measurement that stops measuring reads exactly like one that \
         holds",
        root.display()
    );

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

            // LAW 1 — ONE CALL FOR EVERY LINE, AGAINST THE SAME LINES SPLIT
            // ACROSS TWO CALLS THAT INTERLEAVE THEM. Odds in one, evens in the
            // other: every line's NEIGHBOURS move to the other call, so a batch
            // that let one line's resolution path decide another's cannot agree
            // with itself.
            //
            // THE FIRST VERSION OF THIS ASKED PER LINE and cost 891 seconds —
            // one parse per line of 174000 lines. This is three parses per file
            // and asks a sharper question: a per-line call has no neighbours to
            // be confused by, so it could not see an interleaving defect at all.
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

            // LAW 2 — an answer is text from the file it was given.
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

        // LAW 3 — the reach is a number, and it is not zero.
        assert!(
            answered > lines_total / 20,
            "{}: only {answered} of {lines_total} line(s) answered — a resolver \
             that reaches almost nothing satisfies laws 1 and 2 for free",
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
