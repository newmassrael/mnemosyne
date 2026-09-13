//! THE CITATION GATE IS BUILT IN PLACES THIS FILE NAMES, AND RUN IN ONE.
//!
//! # What the law is for
//!
//! `SetEqualityValidator` has six public fields and no constructor, so building
//! one is a struct literal at whatever call site wants an answer. Until Round
//! 1333 the site that produced the gate's VERDICT was two hundred and forty
//! lines inside `mnemosyne-cli`'s `cmd_validate_code_refs`, between its flag
//! parsing and its two report writers — reachable only by running that binary.
//! A second surface wanting the same answer (the agent-facing one N295 asks
//! for) had to write that sequence again, and a sequence written twice is two
//! answers waiting to disagree about which severities were in force, which store
//! was read, or what a scope selected.
//!
//! Rounds 1322 to 1324 are what a second implementation costs when it is a
//! grammar for someone else's documents: three rounds, each shipping a limit the
//! next one paid for. The shape here is the same one level up, and it is cheaper
//! to refuse than to discover.
//!
//! # Why the population is complete
//!
//! Building a struct and calling a method are syntactic acts written in tracked
//! files, so `git ls-files` plus `syn` enumerates every one exactly. That is
//! what makes this a law rather than a record of what somebody happened to
//! notice: a walk that read nothing writes an empty finding list, and an empty
//! finding list is what a clean tree and an unexamined one look like alike —
//! which is why the reach is asserted before the findings are.
//!
//! # Why it PARSES rather than scans
//!
//! The first draft of this law was a text scan, and it was wrong twice in its
//! first run. `Foo {` matches `pub struct Foo {`, `impl Foo {` and
//! `impl Trait for Foo {` — the type's own declarations, which build nothing —
//! so the gate's definition was reported as three surfaces. Filtering those by
//! their leading words fixed that reading and left the other hole: a scan can
//! only skip a `#[cfg(test)]` module by ASSUMING it is the file's last item,
//! which is this repository's convention and enforced by nothing, so a
//! production construction written below one would be invisible to the law meant
//! to find it. `ci_plan::rust::type_sites` is told which items carry the
//! attribute and skips exactly those, wherever they stand.
//!
//! # What is NOT claimed
//!
//! Not every construction is a gate run, and the list below says which is which.
//! The gate has TWO questions — where citations are, and whether they are valid
//! — and the distinction is the one the port arc turned on (Rounds 1328-1332).
//! Round 1333 held only the verdict to one place and named the CLI's two reading
//! sites as legitimate; Round 1336 gave the reading its own assembly too, and
//! this list is one row shorter for it. A list that only ever grows is a list
//! nobody is using to decide anything.
//!
//! The gate's OWN crate is not on the list and must not be: with declarations no
//! longer mistaken for constructions it builds none outside its tests, so a row
//! for it would be a blanket exemption for the one file most able to grow a
//! second run unseen. The first draft carried that row, and an injection aimed
//! at the declaration filter came back green because of it.

use std::path::{Path, PathBuf};

use ci_plan::rust::{type_sites, TypeSite, TypeSites};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// Where the gate is built and where its verdict is asked for, over every
/// tracked production Rust file.
fn gate_sites() -> TypeSites {
    type_sites(&repository_root(), "SetEqualityValidator", "scan_and_reach")
}

/// Every production site that builds a `SetEqualityValidator`, with what it
/// builds one FOR. A new row is not a failure to route around — it is a place to
/// say which of the two questions the site is asking.
const DECLARED_CONSTRUCTIONS: &[(&str, &str)] = &[(
    "crates/mnemosyne-ops/src/citations.rs",
    "BOTH QUESTIONS, one assembly each. `scan_citations` produces the VERDICT, \
     so every surface asking `what is wrong with this tree` reads one answer; \
     `with_reading` serves the READING that `citation_index` and \
     `propose_implementations` are, which is where citations ARE rather than \
     whether they are valid. Round 1333 declared the CLI here too, for two \
     reading sites it assembled itself; Round 1336 folded them, and the list is \
     shorter by exactly that.",
)];

#[test]
fn every_production_build_of_the_citation_gate_is_one_this_law_names() {
    let found = gate_sites();

    // THE REACH FIRST. This walk's population is every tracked production Rust
    // file in the repository; a walk that stopped would find no sites and read
    // as a tree that builds no validators.
    assert!(
        found.files > 150,
        "this repository tracks more than a hundred and fifty production Rust \
         files and this walk parsed {}, which is a walk that stopped rather than \
         a repository that emptied",
        found.files
    );
    assert!(
        !found.built.is_empty(),
        "no production site builds the citation gate, which means this walk found \
         nothing — the gate is built by `validate-code-refs` on every commit"
    );

    let undeclared: Vec<String> = found
        .built
        .iter()
        .filter(|site| {
            !DECLARED_CONSTRUCTIONS
                .iter()
                .any(|(file, _)| site.file == *file)
        })
        .map(TypeSite::rendered)
        .collect();
    assert!(
        undeclared.is_empty(),
        "these production sites build a `SetEqualityValidator` and this law does \
         not name them: {undeclared:?}\n\
         Building one is how a second surface grows its own gate run. If the site \
         wants a VERDICT it should call `mnemosyne_ops::scan_citations` instead; \
         if it wants the citation INDEX, add it to DECLARED_CONSTRUCTIONS saying \
         so."
    );
}

#[test]
fn the_verdict_is_produced_in_one_place() {
    let found = gate_sites();
    assert!(
        found.files > 150,
        "the walk parsed {} production files, which is a walk that stopped",
        found.files
    );

    // The FILE, not the line: a law pinned to a line number fails on an edit that
    // moves the call and says nothing about what it is there to protect.
    //
    // The gate's OWN crate is not a surface. `scan` delegates to
    // `scan_and_reach` there — the thin wrapper that drops the coverage half —
    // and a caller inside the definition is the type talking to itself. What this
    // law counts is who ASKS the gate for a verdict from outside it.
    let mut callers: Vec<&str> = found
        .called
        .iter()
        .map(|site| site.file.as_str())
        .filter(|file| !file.starts_with("crates/mnemosyne-validate/src/"))
        .collect();
    callers.dedup();
    assert_eq!(
        callers,
        ["crates/mnemosyne-ops/src/citations.rs"],
        "the citation gate's verdict is produced in exactly one production place \
         so that every surface reads one answer; these are the files that call it"
    );
}
