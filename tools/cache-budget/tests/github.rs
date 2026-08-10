//! What GitHub actually answers, held against what this gate reads out of it.
//!
//! THE BODIES HERE ARE RECORDINGS. `tests/actions-caches.one-page.json` and
//! `tests/actions-caches.paginated.json` are what `gh api
//! repos/{owner}/{repo}/actions/caches` printed for THIS repository, byte for
//! byte, the second with `--paginate` over a page size small enough to make it
//! answer in four. Everything below reads one of those, or a mutation of one
//! that names the drift it is modelling — because a fixture somebody invented
//! proves what that person believed the API sends.
//!
//! WHY THIS FILE HAD TO EXIST. Until R1130 the answer was flattened by a `--jq`
//! expression before this program ever saw it, and that expression is a program
//! in a language nothing here can run: there is no `jq` on this machine, and
//! `gh` needs a network and a credential. So the one seam between this gate and
//! the storage it judges had no oracle of any kind — the injection that renames
//! a field GitHub sends could not be written, let alone go red.
//!
//! AND ONE FAILURE OF THIS READ IS SILENT WHILE THE REST ARE LOUD. A row missing
//! a size does not parse and a body that never arrived is empty, but a read that
//! STOPS EARLY is a well-formed answer describing a smaller repository — and
//! smaller is the direction that passes a budget. `total_count` is the only
//! thing in the answer that can catch it, and the projection threw it away.

use cache_budget::caches_in;

/// This repository's cache storage, as GitHub answered on 2026-08-10.
const ONE_PAGE: &str = include_str!("actions-caches.one-page.json");

/// The same storage, the same minute, answered in four pages instead of one.
const PAGINATED: &str = include_str!("actions-caches.paginated.json");

/// The newest cache in both recordings, spelled as GitHub spells it.
const NEWEST_KEY: &str =
    "Linux-cargo-validate-90ceb24c2cf985e258613d038affb1685bf0ab9134a089fb7a8d3958f97b9856";
const NEWEST_BYTES: u64 = 151_791_181;
const NEWEST_CREATED: &str = "2026-08-10T10:14:11.221132000Z";

/// How many caches this repository held when both recordings were taken.
const HELD: usize = 11;

/// The recorded answer reads as the caches it carries, values and all.
///
/// THE VALUES ARE ASSERTED AND NOT JUST THE COUNT, because the failure this
/// replaces was a projection naming fields by hand: a `size_in_bytes` GitHub
/// renamed would have arrived as the string `null` and a `key` it renamed as an
/// empty one, and a test counting rows agrees with both.
#[test]
fn the_recorded_answer_reads_as_the_caches_it_carries() {
    let held = caches_in(ONE_PAGE).expect("a real answer is one this gate can read");
    assert_eq!(held.len(), HELD, "every row GitHub sent, and no others");
    let newest = &held[0];
    assert_eq!(newest.key, NEWEST_KEY);
    assert_eq!(newest.size_in_bytes, NEWEST_BYTES);
    assert_eq!(newest.created_at, NEWEST_CREATED);
    // AND THE FIELDS THIS GATE DOES NOT WANT DID NOT STOP IT. The recording
    // carries `id`, `ref`, `version` and `last_accessed_at` on every row; a
    // reader that refused unknown fields would go red the day GitHub adds one,
    // which is a gate failing for somebody else's work.
    assert!(
        ONE_PAGE.contains("\"last_accessed_at\""),
        "the recording carries fields this gate ignores — if it stops doing so, \
         this test has stopped proving that ignoring them works"
    );
}

/// Four pages of one storage read as the same caches as one page of it.
///
/// TWO RECORDINGS OF THE SAME THING, taken seconds apart, are the control this
/// gate's pagination handling needs: `gh --paginate` prints one JSON object per
/// page rather than one merged document, and a reader that took only the first
/// or concatenated them wrongly disagrees with the single-page answer here.
#[test]
fn four_pages_of_one_storage_read_as_the_same_caches_as_one_page() {
    let paged = caches_in(PAGINATED).expect("a real paginated answer is readable");
    let single = caches_in(ONE_PAGE).expect("a real single-page answer is readable");
    assert_eq!(
        paged, single,
        "the same storage, answered in four pages and in one"
    );
}

/// An answer that stops early is a read that failed — not a smaller repository.
///
/// THE FIXTURE IS REAL AND SO IS THE FAILURE IT MODELS: this is the first page
/// of the recorded paginated answer, sliced at the byte the stream ends it on,
/// which is EXACTLY what a `gh` without `--paginate` prints. It says the
/// repository holds eleven caches and carries three, and three of eleven caches
/// weigh a third of the budget. Nothing about it is malformed.
#[test]
fn an_answer_that_stops_early_is_a_read_that_failed_and_not_a_smaller_repository() {
    let why = caches_in(first_page_of(PAGINATED))
        .expect_err("a body carrying fewer rows than it counts is not a verdict");
    assert!(
        why.contains("11") && why.contains("3"),
        "and it says what it was told and what arrived: {why}"
    );
}

/// A repository holding no caches says so, and that is a reading rather than a
/// refusal.
///
/// THE CONTROL FOR THE TWO ABOVE. Every other case here is a failure, and a
/// reader that refused whenever it ended up with no rows would be refusing the
/// honest answer a fresh repository gives.
#[test]
fn a_repository_holding_no_caches_says_so_and_is_read_as_none() {
    let held = caches_in(r#"{"total_count":0,"actions_caches":[]}"#)
        .expect("a page saying there is nothing is an answer, not a failure");
    assert!(held.is_empty(), "and it holds nothing: {held:?}");
}

/// Nothing printed is not a repository holding nothing.
///
/// THE TWO USED TO BE ONE ANSWER. Under the projection this replaces, a `gh`
/// whose filter matched nothing and a `gh` that printed nothing both arrived as
/// an empty stdout and were read as a repository storing zero bytes.
#[test]
fn nothing_printed_is_not_a_repository_holding_nothing() {
    let why = caches_in("").expect_err("an empty answer is not a repository");
    assert!(
        why.contains("printed nothing"),
        "and it says which of the two it is: {why}"
    );
}

/// A field GitHub renames is a refusal, and not a zero.
///
/// THE DRIFT THIS SEAM IS FOR, applied to a real body: with the size gone, a
/// reader that shrugged would price every cache in the repository at nothing and
/// report a storage sitting on its limit as using none of it.
#[test]
fn a_size_this_gate_cannot_find_is_a_refusal_rather_than_a_zero() {
    let renamed = ONE_PAGE.replace("\"size_in_bytes\"", "\"bytes\"");
    let why = caches_in(&renamed).expect_err("a row with no size is not a cache of size zero");
    // THE READER'S OWN WORDS, and not this gate's summary of them: the wrapper
    // names all five fields whatever goes wrong, so an oracle reading only that
    // would agree with a body that failed for any other reason.
    assert!(
        why.contains("missing field `size_in_bytes`"),
        "and it names the field GitHub stopped sending: {why}"
    );
}

/// Pages that disagree about the count are storage moving underneath the read.
///
/// A cache saved by another job of the same run lands between two pages, and the
/// rows either side of it are then a sample rather than a census. Summing them
/// produces a total that is nobody's.
#[test]
fn pages_that_disagree_about_the_count_are_refused() {
    let moved = format!(
        "{ONE_PAGE}{}",
        ONE_PAGE.replace("\"total_count\":11", "\"total_count\":12")
    );
    let why = caches_in(&moved).expect_err("two answers about one storage is not a total");
    assert!(
        why.contains("11") && why.contains("12"),
        "and it says both counts: {why}"
    );
}

/// The one spelling of a timestamp this gate's ordering assumes it will get.
///
/// `Held::created_at` says in its own doc comment that string comparison is
/// sound "only because the API returns one fixed-width UTC spelling for every
/// entry" — a claim about the world with, until now, no reader. It is also NOT
/// the spelling a run's start time comes in, which is why the comparison against
/// a run is made to the second; a fixture writing the shorter one would be
/// testing the ordering on inputs the API never produces.
#[test]
fn every_recorded_stamp_carries_the_one_spelling_the_ordering_assumes() {
    let held = caches_in(ONE_PAGE).expect("a real answer is readable");
    let widths: std::collections::BTreeSet<usize> =
        held.iter().map(|cache| cache.created_at.len()).collect();
    assert_eq!(
        widths.len(),
        1,
        "one width for every entry, or comparing them as strings orders them by \
         how they were printed: {widths:?}"
    );
    assert!(
        held.iter()
            .all(|cache| cache.created_at.ends_with('Z') && cache.created_at.contains('.')),
        "UTC, and to the fraction — the spelling `to_the_second` exists to trim: {:?}",
        held.first().map(|cache| &cache.created_at)
    );
}

/// The bytes of the first page of a concatenated answer, sliced where the stream
/// ends it.
///
/// ASKED OF THE PARSER rather than found by searching for `}{`: a key or a
/// timestamp may hold any two characters, and a reader that split on them would
/// be a second, worse parser standing between this test and the recording.
fn first_page_of(body: &str) -> &str {
    let mut pages = serde_json::Deserializer::from_str(body).into_iter::<serde_json::Value>();
    pages
        .next()
        .expect("the recording has a first page")
        .expect("and it is JSON");
    &body[..pages.byte_offset()]
}
