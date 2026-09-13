//! THE BACKENDS THIS BUILD SHIPS ARE LISTED ONCE, AND A CONFIG BECOMES
//! RESOLVERS IN ONE PLACE.
//!
//! # What the law is for
//!
//! `[plugins.symbol_resolver.<lang>]` is refused on three grounds — a language
//! no extension maps to (Round 855), a backend this build has no plugin for
//! (Round 855), and a backend registered under a language it does not resolve
//! (Round 1151). The third one PARSED, REGISTERED AND RAN for hundreds of
//! rounds, checking every citation in a tree against whatever the C++ grammar
//! made of Rust source and publishing the result as a judged count.
//!
//! Those three refusals are a behaviour of THE BUILD, not of a binary. Until
//! Round 1335 they lived in `mnemosyne-cli`'s `main.rs`, reachable only by
//! running that binary, and the table they read lived in its library. A second
//! surface — the agent-facing one N295 asks for — would have had to write both
//! again, and a surface that wrote them differently would accept a declaration
//! this one refuses. Two runs over one tree would then disagree about whether
//! the symbol axis was judged at all, which is the distinction Round 1141 spent
//! a round making legible.
//!
//! # Why the population is complete
//!
//! Both are syntactic: a `static` table is an item and a config-to-resolvers
//! walk is a function. `git ls-files` plus `syn` enumerates every construction
//! of the table's row type and every call of the wiring exactly, which is what
//! makes this a law rather than a note. A walk that read nothing writes an empty
//! finding list, and an empty finding list is what a clean tree and an
//! unexamined one look like alike — so the reach is asserted before the
//! findings.

use std::path::{Path, PathBuf};

use ci_plan::rust::{type_sites, TypeSite};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// The one crate that may build a backend row, and the one that may turn a
/// config into resolvers. A surface that needs either depends on this crate;
/// it does not grow its own.
const THE_ONE_HOME: &str = "crates/mnemosyne-backends/src/lib.rs";

#[test]
fn the_backend_table_is_written_in_one_place() {
    let found = type_sites(&repository_root(), "InProcessBackend", "resolver_map");

    // THE REACH FIRST.
    assert!(
        found.files > 150,
        "this repository tracks more than a hundred and fifty production Rust \
         files and this walk parsed {}, which is a walk that stopped rather than \
         a repository that emptied",
        found.files
    );
    assert!(
        found.built.len() >= 5,
        "this build ships five in-process backends and the walk found {} row(s), \
         which is a walk that read the wrong thing — every row is a struct \
         literal of `InProcessBackend`",
        found.built.len()
    );

    let elsewhere: Vec<String> = found
        .built
        .iter()
        .filter(|site| site.file != THE_ONE_HOME)
        .map(TypeSite::rendered)
        .collect();
    assert!(
        elsewhere.is_empty(),
        "these production sites build a backend row outside {THE_ONE_HOME}: \
         {elsewhere:?}\n\
         A second table is a second answer to `which grammars does this build \
         ship`, and the refusals that read it would then differ per surface."
    );
}

#[test]
fn a_config_becomes_resolvers_in_one_place() {
    let found = type_sites(&repository_root(), "InProcessBackend", "resolver_map");
    assert!(
        found.files > 150,
        "the walk parsed {} production files, which is a walk that stopped",
        found.files
    );

    // Callers are expected — that is the point of moving it — so what this law
    // holds is that the FUNCTION has one definition, which `type_sites` cannot
    // see directly. What it can see is that nobody has copied the wiring back
    // into a caller: a call site is fine, a second `resolver_map` is not, and a
    // second one would have to be defined in a file that also calls it.
    let defining_callers: Vec<String> = found
        .called
        .iter()
        .filter(|site| site.file == THE_ONE_HOME)
        .map(TypeSite::rendered)
        .collect();
    assert!(
        defining_callers.is_empty(),
        "{THE_ONE_HOME} both defines `resolver_map` and calls it: {defining_callers:?}\n\
         That is how a crate grows a private second wiring beside the public one."
    );

    let callers: Vec<&str> = found.called.iter().map(|site| site.file.as_str()).collect();
    assert!(
        !callers.is_empty(),
        "nothing calls `resolver_map`, so the wiring this law protects is \
         reachable by nobody and the citation gate runs with no resolvers at all"
    );
}
