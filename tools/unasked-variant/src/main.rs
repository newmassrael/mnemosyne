//! Print the census. The threshold this gate judges on is chosen from what this
//! prints, not the other way round.

use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let root = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let census = unasked_variant::enumerations(&root);
    println!(
        "[unasked-variant] {} tracked Rust file(s), {} enum(s) this workspace \
         defines, {} match expression(s) in all — {} of them name a variant of \
         one of those enums",
        census.files,
        census.enums,
        census.matches,
        census.found.len()
    );

    // THE DISTRIBUTION IS THE POINT OF THIS BINARY. A threshold picked before
    // the shape is known is a number somebody liked.
    let mut by_density: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for found in &census.found {
        let cell = by_density.entry(found.named.len()).or_default();
        if found.catch_all.is_some() {
            cell.0 += 1;
        } else {
            cell.1 += 1;
        }
    }
    println!("[unasked-variant] variants named | with a catch-all | exhaustive");
    for (named, (with, without)) in &by_density {
        println!(
            "[unasked-variant]   {named:>3}            | {with:>4}             | {without:>4}"
        );
    }

    // AND THE SAME POPULATION BY FRACTION, because a count is not the question a
    // reader of one match asks. `2 of 33` and `2 of 5` are the same count and
    // opposite things — the first is a filter over a big enum, the second is
    // most of a small one.
    let mut by_share: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    for found in &census.found {
        if found.variants == 0 {
            continue;
        }
        let tenth = u32::try_from(found.named.len() * 10 / found.variants).unwrap_or(10);
        let cell = by_share.entry(tenth).or_default();
        if found.catch_all.is_some() {
            cell.0 += 1;
        } else {
            cell.1 += 1;
        }
    }
    println!("[unasked-variant] tenths of the enum named | with a catch-all | exhaustive");
    for (tenth, (with, without)) in &by_share {
        println!("[unasked-variant]   {tenth:>3}/10                  | {with:>4}             | {without:>4}");
    }

    // AND BY SPELLING (R1283), because R1282's law read one of the three and the
    // distribution is what says whether that mattered.
    let mut by_shape: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for found in &census.found {
        let cell = by_shape.entry(format!("{:?}", found.shape)).or_default();
        if found.catch_all.is_some() {
            cell.0 += 1;
        } else {
            cell.1 += 1;
        }
    }
    println!(
        "[unasked-variant] spelling | with a catch-all | exhaustive ({} matches! call(s) unread)",
        census.unreadable
    );
    for (shape, (with, without)) in &by_shape {
        println!("[unasked-variant]   {shape:<14} | {with:>4}             | {without:>4}");
    }

    // AND HOW MANY CATCH-ALLS DO NOTHING AT ALL (R1283) — the one part of
    // "direction" a program can settle, split by whether the place is an
    // enumeration or a filter, because the first is already refused and the
    // second is the open question.
    let mut discarding = (0usize, 0usize);
    for found in census.found.iter().filter(|e| e.discards) {
        if found.named.len() >= 2 && found.named.len() * 3 >= found.variants {
            discarding.0 += 1;
        } else {
            discarding.1 += 1;
        }
    }
    println!(
        "[unasked-variant] catch-alls whose body does nothing at all: {} on an \
         enumeration, {} on a filter",
        discarding.0, discarding.1
    );

    for found in census.enumerating_with_a_catch_all(2) {
        println!(
            "[unasked-variant]   {} {:?} — names {} of {} variant(s), catch-all {:?}: {:?}",
            found.origin(),
            found.shape,
            found.named.len(),
            found.variants,
            found.catch_all.expect("filtered on it"),
            found.named
        );
    }
}
