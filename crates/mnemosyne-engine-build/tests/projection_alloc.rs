//! What the baked artifact costs to BUILD, in allocations (Round 782).
//!
//! # The question being decided
//!
//! The first consumer's third field report measured 34,825 allocations and
//! 2.78 MB on every startup and asked for projection types that can hold
//! `&'static` instead of owning `String`: the data being built is already in the
//! binary as string literals, so a read side that only reads should point at it
//! rather than copy it. The report was explicit that this is not a speed
//! complaint — it is a few milliseconds — but a claim about the PROPERTY, that a
//! store checked at ingestion and fixed again at bake time should be taken out,
//! not rebuilt.
//!
//! Round 775's lesson is that the emitted shape gets measured before it gets
//! chosen, and nobody has measured what borrowing costs. This file is the
//! measurement, and it deliberately does not change a shipped type: the trio
//! `build.rs` emits holds the literal content, the field count and order, the
//! chunk bound and the two sizes equal, and varies only how the data is held.
//!
//! # What the three arms are for
//!
//! - `owned` — the shape shipped today. It is also the CONTROL: it must grow
//!   with the store, because "did not grow" is equally what a broken counter
//!   says, and every other assertion here is licensed by this one having moved.
//! - `borrowed` — the consumer's literal request, strings pointing at `.rodata`,
//!   with the `Vec`-of-chunks assembly deliberately kept. This is the arm that
//!   prices stopping halfway: the strings go and the spine stays.
//! - `static_slice` — the consumer's own sketch taken all the way. Chunk
//!   `static`s plus a `static` of slices over them, which keeps Round 775's
//!   bound (no item holds the whole store) while materializing nothing. Its
//!   result is a COUNT rather than a ratio, and a count of zero needs no
//!   threshold to be read.
//!
//! # What was measured when this landed
//!
//! Allocations here, at 200 lines and 800; compile figures out of band, `rustc`
//! on the same emitted fixtures at `opt-level = 0`, median of three, against an
//! empty-fixture baseline of 0.01s and 78 MB that is subtracted below.
//!
//! | arm | allocs 200 / 800 | bytes built (800) | emitted (800) | rustc user (200 / 800) | peak RSS (800) |
//! |---|---|---|---|---|---|
//! | owned | 1,006 / 4,020 | 501,600 | 232,520 | 0.16s / 0.60s | +83 MB |
//! | borrowed | 6 / 20 | 312,800 | 194,171 | 0.04s / 0.11s | +58 MB |
//! | static_slice | 0 / 0 | 0 | 193,749 | 0.02s / 0.07s | +21 MB |
//!
//! Two things in that table were not what this round expected.
//!
//! FIRST, borrowing removes 99.5% of the allocation CALLS and only 38% of the
//! BYTES. The strings were never the memory — the element is, and it is still
//! copied into a `Vec`. The consumer's report is about call traffic (43% of
//! instructions in malloc/free), so the middle arm answers what was asked; the
//! 2.78 MB it also reported would mostly survive it. Only the third arm is zero.
//!
//! SECOND, and this is the question the consumer flagged as unmeasured and the
//! reason the round exists: borrowing does not COST compile time, it saves it,
//! by 5.5x at 800 lines, and the static arm by 8.6x on a quarter of the marginal
//! memory. The worry was backwards. `"lit".to_string()` is a method call per
//! literal for rustc to resolve, typecheck and codegen; `"lit"` is the literal.
//! Round 775 was fought over rustc's per-body cost, so the shape that reduces
//! allocation being the same shape that reduces compile cost is worth stating
//! plainly rather than leaving for someone to rediscover.
//!
//! # Why the counter is thread-local
//!
//! A global allocator sees every thread, and the test harness runs tests in
//! parallel, so a process-wide counter would attribute another test's work to
//! this one. The counts here are per-thread and the arms are measured on this
//! test's own thread, which makes the numbers reproducible rather than a
//! function of what else was scheduled. `Cell` with a `const` initializer
//! because a thread-local that ALLOCATES on first touch would be counted by the
//! allocator it is installed in.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use mnemosyne_engine::DisclosureMode;

// `SMALL` and `BIG`, from the build script that emitted the fixtures — the same
// pair the stack gate uses, so "four times the store" cannot mean two things.
include!(concat!(env!("OUT_DIR"), "/stack_build_facts.rs"));

/// The shape shipped today: every string owned.
///
/// Mirrors `LinePart` field for field — five owned strings, a `Vec<String>`, a
/// plain enum and a numeric `Option` — so the count this file reports is the
/// count the real type would pay, not a smaller stand-in's.
pub struct OwnedLine {
    pub fact_id: String,
    pub text: String,
    pub mode: DisclosureMode,
    pub frame: String,
    pub entities: Vec<String>,
    pub carrier: Option<String>,
    pub typed_predicate: Option<String>,
    pub quote: Option<String>,
    pub count: Option<i64>,
}

/// The same fields, pointing at the literals instead of copying them.
pub struct BorrowedLine {
    pub fact_id: &'static str,
    pub text: &'static str,
    pub mode: DisclosureMode,
    pub frame: &'static str,
    pub entities: &'static [&'static str],
    pub carrier: Option<&'static str>,
    pub typed_predicate: Option<&'static str>,
    pub quote: Option<&'static str>,
    pub count: Option<i64>,
}

mod owned_small {
    include!(concat!(env!("OUT_DIR"), "/alloc_owned_small.rs"));
}
mod owned_big {
    include!(concat!(env!("OUT_DIR"), "/alloc_owned_big.rs"));
}
mod borrowed_small {
    include!(concat!(env!("OUT_DIR"), "/alloc_borrowed_small.rs"));
}
mod borrowed_big {
    include!(concat!(env!("OUT_DIR"), "/alloc_borrowed_big.rs"));
}
mod static_slice_small {
    include!(concat!(env!("OUT_DIR"), "/alloc_static_slice_small.rs"));
}
mod static_slice_big {
    include!(concat!(env!("OUT_DIR"), "/alloc_static_slice_big.rs"));
}

// The PROSE shape, which is the opposite one (Round 788+). `PROSE_SMALL` is the
// first consumer's actual spot count and `PROSE_BIG` its quadruple.
include!(concat!(env!("OUT_DIR"), "/prose_build_facts.rs"));

/// A passage as `store_passages` returns it — the anchor's source, its `Prefix`
/// locator, and the text. Three owned strings, one of them long.
pub struct OwnedPassage {
    pub source: String,
    pub prefix: String,
    pub text: String,
}

/// The same three fields pointing at the literals instead of copying them.
pub struct BorrowedPassage {
    pub source: &'static str,
    pub prefix: &'static str,
    pub text: &'static str,
}

mod prose_owned_small {
    include!(concat!(env!("OUT_DIR"), "/prose_owned_small.rs"));
}
mod prose_owned_big {
    include!(concat!(env!("OUT_DIR"), "/prose_owned_big.rs"));
}
mod prose_borrowed_small {
    include!(concat!(env!("OUT_DIR"), "/prose_borrowed_small.rs"));
}
mod prose_borrowed_big {
    include!(concat!(env!("OUT_DIR"), "/prose_borrowed_big.rs"));
}

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
    static BYTES: Cell<usize> = const { Cell::new(0) };
}

/// `System`, plus a tally of what passed through it on this thread.
///
/// `try_with` rather than `with`: a thread-local is unavailable during thread
/// teardown, and an allocator that panicked there would abort a run that had
/// nothing to do with what is being measured.
struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        let _ = BYTES.try_with(|c| c.set(c.get() + layout.size()));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// Allocations and bytes charged to building `f`'s value.
///
/// The value is dropped only AFTER the tally is read, so a shape that frees as
/// it goes cannot report a smaller number than it asked for.
fn cost_of<T>(f: impl FnOnce() -> T) -> (usize, usize) {
    ALLOCS.with(|c| c.set(0));
    BYTES.with(|c| c.set(0));
    let built = f();
    let spent = (ALLOCS.with(Cell::get), BYTES.with(Cell::get));
    drop(built);
    spent
}

#[test]
fn borrowing_removes_the_per_line_allocation_and_static_removes_all_of_it() {
    // FIRST the instrument. The shipped shape must charge more at four times the
    // store, or this run measured nothing and every figure below is vacuous.
    let (owned_small_n, _) = cost_of(owned_small::build);
    let (owned_big_n, owned_big_b) = cost_of(owned_big::build);
    assert!(
        owned_big_n > 3 * owned_small_n,
        "the owned arm charged {owned_big_n} allocations at {BIG} lines against \
         {owned_small_n} at {SMALL}: it did not grow with the store, so the \
         counter is not seeing the work and nothing below can be read"
    );

    // What the owned arm is paying for is PER LINE — it charges at least once for
    // every line at both sizes, which is the property the consumer reported and
    // the one borrowing is supposed to remove.
    assert!(
        owned_small_n >= SMALL && owned_big_n >= BIG,
        "the owned arm charged under one allocation per line ({owned_small_n} at \
         {SMALL}, {owned_big_n} at {BIG}), so it is not the per-line shape this \
         file exists to price"
    );

    // Borrowing removes it. Both arms assemble the same Vec of chunks from the
    // same literals, so the gap between them is the strings and nothing else.
    // Asserted at BOTH sizes rather than only the big one: a reduction that
    // showed up at a single size could be an artifact of that size.
    let (borrowed_small_n, _) = cost_of(borrowed_small::build);
    let (borrowed_big_n, borrowed_big_b) = cost_of(borrowed_big::build);
    for (what, borrowed, owned) in [
        ("small", borrowed_small_n, owned_small_n),
        ("big", borrowed_big_n, owned_big_n),
    ] {
        assert!(
            borrowed * 10 < owned,
            "at the {what} size, borrowing the strings charged {borrowed} \
             allocations against the owned arm's {owned} over identical data: the \
             strings are not what the owned arm is paying for"
        );
    }

    // And what SURVIVES borrowing: the Vec spine, one allocation per chunk rather
    // than per line. Stated as "fewer than one per line" — the complement of the
    // owned arm's assertion above, so the two together say exactly where the cost
    // moved rather than quoting a constant fitted to today's chunk bound.
    assert!(
        borrowed_small_n < SMALL && borrowed_big_n < BIG,
        "the borrowed arm still charged at least one allocation per line \
         ({borrowed_small_n} at {SMALL}, {borrowed_big_n} at {BIG}): its residual \
         is not the per-chunk spine it should be"
    );

    // The whole way: chunk statics and a static of slices over them. No ratio and
    // no threshold — it either builds something or it does not.
    for (what, (n, bytes)) in [
        ("small", cost_of(static_slice_small::build)),
        ("big", cost_of(static_slice_big::build)),
    ] {
        assert_eq!(
            (n, bytes),
            (0, 0),
            "the static_slice arm charged {n} allocations and {bytes} bytes at the \
             {what} size: a projection assembled from statics must build nothing"
        );
    }

    // The finding that decides how far the change has to go. Borrowing removes
    // almost every allocation CALL, and far less of the memory: the elements are
    // still copied into a `Vec`, and the element is most of the bytes. So the
    // byte ratio between the arms must come out much smaller than the count
    // ratio — which is the same statement as "stopping at borrowed buys the
    // malloc traffic and not the footprint", asserted rather than narrated.
    assert!(
        owned_big_b * borrowed_big_n < borrowed_big_b * owned_big_n,
        "the arms' byte ratio ({owned_big_b} to {borrowed_big_b}) is not smaller \
         than their count ratio ({owned_big_n} to {borrowed_big_n}): borrowing \
         removed the copies as thoroughly as it removed the calls, which would \
         make the static arm unnecessary"
    );
}

/// Round 788+ — the prose axis, which is the shape Round 782 did NOT measure.
///
/// The third lift request asks the kernel to bake the passages `store_passages`
/// parses at every startup. Round 782's answer for the projection was that
/// borrowing removes nearly all the allocation CALLS and only 38% of the bytes,
/// because the strings were never the memory — the elements were. Prose inverts
/// the proportions: `PROSE_SMALL` spots of `PROSE_CHARS` characters each, where
/// the strings ARE most of the bytes. Whether the same conclusion carries is not
/// something to assume on the way to picking a shape.
///
/// The instrument is checked first, as in the test above: the owned arm must
/// charge more at four times the prose, or every figure here is vacuous.
#[test]
fn borrowing_prose_removes_what_borrowing_lines_could_not() {
    let (owned_small_n, owned_small_b) = cost_of(prose_owned_small::build);
    let (owned_big_n, owned_big_b) = cost_of(prose_owned_big::build);
    assert!(
        owned_big_n > owned_small_n && owned_big_b > owned_small_b,
        "the owned arm charged {owned_big_n} allocations / {owned_big_b} bytes at \
         {PROSE_BIG} spots against {owned_small_n} / {owned_small_b} at \
         {PROSE_SMALL}: it did not grow, so this run measured nothing"
    );
    // Three strings per spot is the shape being priced, so the count must track
    // the spots rather than sit at some constant the Vec spine explains.
    assert!(
        owned_big_n >= PROSE_BIG * 3,
        "the owned arm charged {owned_big_n} for {PROSE_BIG} spots of three \
         strings each: that is not the per-spot shape this claims to weigh"
    );

    let (borrowed_small_n, borrowed_small_b) = cost_of(prose_borrowed_small::build);
    let (borrowed_big_n, borrowed_big_b) = cost_of(prose_borrowed_big::build);

    // The claim: with the text borrowed, what remains is the `Vec` spine — a
    // handful of reallocations, NOT a per-spot cost.
    assert!(
        borrowed_big_n < PROSE_BIG,
        "the borrowed arm still charged {borrowed_big_n} allocations for \
         {PROSE_BIG} spots, so it is still paying per spot"
    );

    // AND THE BYTES, which is where prose parts company with lines. Round 782
    // measured a 38% byte reduction on the line shape and said the strings were
    // never the memory. Here they are: the reduction must be far deeper, or the
    // line result carried over and this axis needed no separate decision.
    let byte_cut = 1.0 - (borrowed_big_b as f64 / owned_big_b as f64);
    assert!(
        byte_cut > 0.90,
        "borrowing prose cut only {:.1}% of the bytes ({owned_big_b} to \
         {borrowed_big_b}); on the line shape Round 782 measured 38%, and a \
         result that close would mean prose needs no decision of its own",
        byte_cut * 100.0
    );

    // Reported so a reader of the log sees the pair the decision rests on.
    println!(
        "prose owned  {PROSE_SMALL}: {owned_small_n} allocs / {owned_small_b} B \
         | {PROSE_BIG}: {owned_big_n} / {owned_big_b}"
    );
    println!(
        "prose borrow {PROSE_SMALL}: {borrowed_small_n} allocs / {borrowed_small_b} B \
         | {PROSE_BIG}: {borrowed_big_n} / {borrowed_big_b}  (bytes -{:.1}%)",
        byte_cut * 100.0
    );
}
