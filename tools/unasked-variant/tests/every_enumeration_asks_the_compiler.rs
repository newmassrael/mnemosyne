//! A `match` that ENUMERATES an enum this workspace owns may not carry a
//! catch-all.
//!
//! # Why the threshold is two-and-a-third, and where those numbers came from
//!
//! Both halves are read off the distribution this crate's binary prints over the
//! repository, and neither was chosen first.
//!
//! THE COUNT. Of the matches naming exactly ONE variant of one of this
//! workspace's enums, 120 of 121 carry a catch-all; of those naming two or more,
//! 185 of 190 do not. The cliff is between one and two, and it is what tells a
//! FILTER from an ENUMERATION: a match that says "this variant, and otherwise"
//! is asking one question, and writing out twenty arms to ask it would be worse
//! code and a worse gate.
//!
//! THE FRACTION, BECAUSE THE COUNT CANNOT SEE SIZE. `2 of 33` and `2 of 5` are
//! the same count and opposite things. Both `ContinuityViolation` matches in this
//! tree name two or three of thirty-three variants — plainly filters — and a
//! count-only law would have called them findings and taught everybody to ignore
//! it. A third of the enum separates them from the three real ones by a wide
//! margin: 6% and 9% on one side, 40%, 57% and 100% on the other.
//!
//! AND A THIRD OF ITS FIRST THREE FINDINGS WAS THE GATE BEING WRONG. The densest
//! of them — `mnemosyne-render::door_label`, naming all three `Door` variants
//! beside a catch-all — could not be repaired: `Door` is `#[non_exhaustive]`, so
//! rustc REQUIRES the wildcard in any crate but the one defining it. The repair
//! the gate licensed did not build, which is the cheapest way a law can be told
//! it is wrong. Such enums are out of the population now, for the reason the
//! attribute exists: it is a decision that adding a variant must not break
//! readers, and a gate cannot overrule it by refusing the code that obeys it.
//!
//! THE TWO THAT REMAINED ARE FIXED RATHER THAN LISTED, which is the whole reason
//! this law has no allow-list. R1277 spent a round on what a list of exceptions
//! costs, and the answer was that a list nobody is trying to empty is a list that
//! grows. If a legitimate enumeration-with-a-catch-all ever appears, the round
//! that writes it decides in the open.
//!
//! # Three spellings, and R1282 read one of them
//!
//! R1283 measured what that cost: of the 475 places in this repository that sort
//! a value of one of its own enums by variant, R1282's law reached 351.
//! `matches!` and `if let` chains are the other 124, and EVERY ONE OF THEM
//! carries a catch-all — not by choice but by construction, since neither can be
//! written exhaustively. The question this law asks is whether adding a variant
//! reaches the reader, and it is a question about any construct that sorts by
//! variant rather than about one keyword.
//!
//! WHAT THE TWO NEW SPELLINGS TURNED UP. Every `if let` chain in this tree names
//! exactly ONE variant — they are filters, and extending the law over them cost
//! nothing today while closing the shape. `matches!` was different: twelve of
//! them named a THIRD or more of their enum, and reading them one by one is what
//! settled that the rule was right rather than merely applicable. `TermScope`
//! decides which halves of the store a query scans, and a fifth scope would have
//! scanned nothing and reported no hits — indistinguishable from a scope that
//! holds none. `Cost` decides whether a schema generation costs its author work,
//! in the table whose purpose is telling a holder what their file will do.
//! `Verdict::is_failure` is the predicate `item-citations` computes its exit code
//! from, and a fifth verdict would have defaulted to PASSING. Each was repaired
//! by naming the negative half, which is the only exhaustive form available.

use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("tools/<name> sits two levels under the root")
        .to_path_buf()
}

/// A match must name at least this many variants of one enum before it is read
/// as an enumeration rather than a filter.
const NAMES_AT_LEAST: usize = 2;

/// …and at least this share of that enum, so a filter over a large enum is not
/// mistaken for an enumeration of a small one. Expressed as a fraction to avoid
/// a float in a gate's arithmetic.
const AT_LEAST_A_THIRD: (usize, usize) = (1, 3);

fn enumerates(named: usize, variants: usize) -> bool {
    let (numerator, denominator) = AT_LEAST_A_THIRD;
    named >= NAMES_AT_LEAST && named * denominator >= variants * numerator
}

#[test]
fn no_match_that_enumerates_one_of_this_workspaces_enums_carries_a_catch_all() {
    let root = repository_root();
    let census = unasked_variant::enumerations(&root);

    // THE DENOMINATORS FIRST, because an empty finding list and a walk that did
    // not run print the same thing.
    println!(
        "[unasked-variant] {} tracked Rust file(s), {} enum(s) this workspace \
         defines, {} match expression(s), {} of them over one of those enums",
        census.files,
        census.enums,
        census.matches,
        census.found.len()
    );
    assert!(
        census.files > 300 && census.enums > 100 && census.matches > 2000,
        "this repository is far larger than that, so a walk that found so little \
         stopped reading rather than found a clean tree"
    );
    // AND ALL THREE SPELLINGS ARRIVED (R1283). The floor above is a total, and a
    // total is exactly what cannot tell a reader that lost one construct: R1282's
    // law was blind to 124 of 475 places and its denominators looked healthy
    // throughout. So each shape is seen to be present on its own.
    for shape in [
        unasked_variant::Shape::Match,
        unasked_variant::Shape::MatchesMacro,
        unasked_variant::Shape::IfLetChain,
    ] {
        let seen = census.found.iter().filter(|e| e.shape == shape).count();
        println!("[unasked-variant] {shape:?}: {seen} over one of this workspace's enums");
        assert!(
            seen > 20,
            "this repository writes all three, so {seen} {shape:?} is a reader \
             that stopped rather than a tree that does not use it"
        );
    }
    println!(
        "[unasked-variant] {} `matches!` call(s) this reader could not split into \
         scrutinee and pattern, and {} enum name(s) mean more than one enum here \
         — membership unions, size takes the smallest, so those report more \
         rather than fewer",
        census.unreadable, census.ambiguous
    );

    let found: Vec<String> = census
        .found
        .iter()
        .filter(|e| e.catch_all.is_some() && enumerates(e.named.len(), e.variants))
        .map(|e| {
            format!(
                "{} — names {} of {} variant(s) and still ends in {:?}: {:?}",
                e.origin(),
                e.named.len(),
                e.variants,
                e.catch_all.expect("filtered on it"),
                e.named
            )
        })
        .collect();
    assert!(
        found.is_empty(),
        "a match that names several variants of an enum this workspace OWNS is \
         enumerating it, and a catch-all is what stops the compiler asking that \
         reader about the next variant somebody adds. Measured twice before this \
         law existed: a sixth `Declared` arm compiled into `_ => return None` and \
         would have reported every site declaring it as issuing no commands, and \
         a new `LockVerdict` variant compiled at every reader in the tree. Spell \
         the arms out, or narrow the match to the variants it is really about:\n  \
         {}",
        found.join("\n  ")
    );

    // WHAT A CATCH-ALL DOES WITH THE VALUE, AS FAR AS A PROGRAM CAN SETTLE IT
    // (R1283, and this is where N220 ends rather than where it is enforced).
    // Direction is the second half of every finding here — R1278's catch-all
    // answered `return None`, which a caller reads as nothing to judge, and
    // R1279's pushed to a list of failures — but WHICH direction a body points
    // is semantic: `false` refuses in `Verdict::is_failure` and accepts in
    // `Origin::fetched`, and `CLAUDE.md` puts that outside v1. An EMPTY body is
    // the part that is not semantic, and it is measured here rather than
    // legislated: on the enumerations this law refuses it is ZERO by
    // construction, and on filters it is the ordinary "only this variant
    // matters" of thirty-odd places that are not defects. A rule over those
    // would be the gate people learn to ignore, which this crate has already
    // been taught once.
    let (discarding_enumerations, discarding_filters): (Vec<_>, Vec<_>) = census
        .found
        .iter()
        .filter(|e| e.discards)
        .partition(|e| enumerates(e.named.len(), e.variants));
    println!(
        "[unasked-variant] catch-alls whose body does nothing at all: {} on an \
         enumeration, {} on a filter",
        discarding_enumerations.len(),
        discarding_filters.len()
    );
    assert!(
        discarding_filters.len() > 10,
        "this repository is full of `match x {{ A => …, _ => {{}} }}`, so {} of \
         them is a reader that stopped classifying bodies rather than a tree \
         that stopped writing them — and an unread datum reports zero for \
         everything",
        discarding_filters.len()
    );

    // NON-VACUITY, AND IT IS THE FILTER HALF. A law that found nothing because
    // it recognises nothing looks exactly like this one passing, so the shapes it
    // is NOT about have to be seen to exist: this repository is full of matches
    // that name one variant and carry a catch-all, and of matches that name every
    // variant and carry none. Both are correct code and both must be outside the
    // finding set for the law above to mean what it says.
    let filters = census
        .found
        .iter()
        .filter(|e| e.catch_all.is_some() && !enumerates(e.named.len(), e.variants))
        .count();
    let exhaustive = census
        .found
        .iter()
        .filter(|e| e.catch_all.is_none())
        .count();
    println!(
        "[unasked-variant] {filters} filter(s) over one of this workspace's \
         enums carry a catch-all and are not the subject, and {exhaustive} match \
         every variant they name with no catch-all at all"
    );
    assert!(
        filters > 20 && exhaustive > 100,
        "both shapes this law deliberately does not touch have to be present for \
         its silence to be worth anything — {filters} filter(s) and {exhaustive} \
         exhaustive match(es) is too few to have been read"
    );
}
