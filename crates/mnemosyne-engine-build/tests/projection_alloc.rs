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
//! **The 5.5x is right and the sentence explaining it is wrong** — Round 794
//! measured the cause and it is not mostly `.to_string()`. The step from `owned`
//! to `borrowed` changes two things at once, the strings AND the per-line
//! `vec![..]`, so this trio could not separate them; the arms below can, and the
//! `vec!` is six times the larger half. See the second table.
//!
//! # What Round 794 added, and what it changed
//!
//! None of the three arms above is a shape the shipped type can HOLD.
//! `from_workspace` derives at run time and must own, so one type carrying both a
//! live and a baked projection is a `Cow<'static, str>` — neither `String` nor
//! `&'static str`. Round 785 designed step two on the assumption that this is
//! "exactly the arm Round 782 measured"; two arms were added to check, differing
//! from each other in ONE field, how the entity list is held.
//!
//! | arm | allocs 200 / 800 | bytes built (800) | emitted (800) | rustc user (800) | peak RSS (800) |
//! |---|---|---|---|---|---|
//! | owned | 1,006 / 4,020 | 501,600 | 232,520 | 0.46s | +83 MB |
//! | cow | 206 / 820 | 460,800 | 288,486 | 0.43s | +81 MB |
//! | cow_list | 6 / 20 | 441,600 | 313,354 | 0.20s | +70 MB |
//! | borrowed | 6 / 20 | 312,800 | 194,171 | 0.09s | +59 MB |
//! | static_slice | 0 / 0 | 0 | 193,749 | 0.06s | +22 MB |
//!
//! Compile figures out of band as above, `rustc` at `opt-level = 0`, median of
//! three, against a baseline of 0.01s and 77,312 KB. The setup reproduces Round
//! 782's marginal memory to within a megabyte on all three of its arms, which is
//! what licenses comparing the new rows to the old ones.
//!
//! ON ALLOCATION CALLS THE DESIGN WAS EXACTLY RIGHT. It predicted 80% of the win
//! for a plain `Vec` and 99% for a borrowable list; measured, 80.0% and 100.0%,
//! with `cow_list` landing not near the fully borrowed arm but ON it, 20 and 20.
//!
//! ON COMPILE IT WAS NOT. The 5.5x does not carry to the arm the kernel can take:
//! `cow` is 1.07x, which is nothing, and `cow_list` is 2.3x. What separates them
//! is one `vec![..]` per line — dropping four `.to_string()`s per line moved 0.46s
//! to 0.43s, and dropping the `vec!` moved 0.43s to 0.20s. The per-line
//! CONSTRUCTOR CALL is what rustc charges for, and `.to_string()` was only the
//! most visible instance of it.
//!
//! ON BYTES BUILT it is worse than the trio suggests and for a structural reason:
//! `Cow<'static, str>` is 24 bytes where `&'static str` is 16, so the element the
//! `Vec` copies is larger. Borrowing everything cuts 38% of the bytes; the
//! kernel's reachable arm cuts 12%. It takes the whole call win and under a third
//! of the byte win, which is the axis the first consumer asked about and the axis
//! it did not.
//!
//! And the emitted source GROWS — 232 KB owned to 313 KB for `cow_list` — while
//! compiling 2.3x faster, because `::std::borrow::Cow::Borrowed(..)` is longer
//! text than `.to_string()` and rustc is not charging for text. That is Round
//! 789's finding (per item, not per byte) reappearing on a different axis.
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
use std::borrow::Cow;
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

/// The shape a shipped `LinePart` could actually take (Round 794): every string
/// a `Cow<'static, str>`, the entity list still a `Vec`.
///
/// Neither arm above is that shape. `from_workspace` derives at run time and must
/// OWN, so one type carrying both a live and a baked projection is a `Cow` — and
/// `Cow<'static, str>` is not `&'static str` any more than it is `String`. This
/// arm is here because Round 792 is the round that established what happens when
/// a design reasons about an arm it did not measure.
pub struct CowLine {
    pub fact_id: Cow<'static, str>,
    pub text: Cow<'static, str>,
    pub mode: DisclosureMode,
    pub frame: Cow<'static, str>,
    pub entities: Vec<Cow<'static, str>>,
    pub carrier: Option<Cow<'static, str>>,
    pub typed_predicate: Option<Cow<'static, str>>,
    pub quote: Option<Cow<'static, str>>,
    pub count: Option<i64>,
}

/// The same, with the entity LIST borrowable too — the one decision Round 785
/// left to the build round, made measurable.
///
/// The arms differ in this field and nothing else, so the gap between them is the
/// list holding and is not confounded with the scalar one.
pub struct CowListLine {
    pub fact_id: Cow<'static, str>,
    pub text: Cow<'static, str>,
    pub mode: DisclosureMode,
    pub frame: Cow<'static, str>,
    pub entities: Cow<'static, [Cow<'static, str>]>,
    pub carrier: Option<Cow<'static, str>>,
    pub typed_predicate: Option<Cow<'static, str>>,
    pub quote: Option<Cow<'static, str>>,
    pub count: Option<i64>,
}

mod cow_small {
    include!(concat!(env!("OUT_DIR"), "/alloc_cow_small.rs"));
}
mod cow_big {
    include!(concat!(env!("OUT_DIR"), "/alloc_cow_big.rs"));
}
mod cow_list_small {
    include!(concat!(env!("OUT_DIR"), "/alloc_cow_list_small.rs"));
}
mod cow_list_big {
    include!(concat!(env!("OUT_DIR"), "/alloc_cow_list_big.rs"));
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

/// The text borrowed, the ANCHOR still owned (Round 792) — the shape a real
/// `Passage` could take without touching `mnemosyne-core`, where `ContentAnchor`
/// lives with its serde derives and its `String` fields, shared with the store.
pub struct TextBorrowedPassage {
    pub source: String,
    pub prefix: String,
    pub text: &'static str,
}

mod prose_text_only_small {
    include!(concat!(env!("OUT_DIR"), "/prose_text_only_small.rs"));
}
mod prose_text_only_big {
    include!(concat!(env!("OUT_DIR"), "/prose_text_only_big.rs"));
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

    // ARM THREE (Round 792) — text borrowed, anchor still OWNED, which is the
    // shape a real `Passage` can take without touching `mnemosyne-core`. Round
    // 790 argued from arithmetic that this keeps nearly all of arm two's win
    // because the text is ~99% of the bytes. Arithmetic is what this crate keeps
    // being wrong about, so it is measured rather than reasoned.
    let (text_only_small_n, text_only_small_b) = cost_of(prose_text_only_small::build);
    let (text_only_big_n, text_only_big_b) = cost_of(prose_text_only_big::build);

    // It must still pay per spot — two owned anchor strings each — or the arm is
    // not the shape it claims to be and the comparison below is against the
    // wrong thing.
    assert!(
        text_only_big_n >= PROSE_BIG * 2,
        "the text-only arm charged {text_only_big_n} for {PROSE_BIG} spots of two \
         owned anchor strings each: that is not the shape being priced"
    );

    // The claim the design rests on: on BYTES it lands with arm two, not with
    // arm one. Stated as a fraction of the win rather than as a threshold on the
    // count, because the count is where it deliberately differs.
    let full_win = (owned_big_b - borrowed_big_b) as f64;
    let text_only_win = (owned_big_b - text_only_big_b) as f64;
    let kept = text_only_win / full_win;
    assert!(
        kept > 0.95,
        "borrowing the text alone kept only {:.1}% of what borrowing everything \
         buys ({owned_big_b} owned, {text_only_big_b} text-only, \
         {borrowed_big_b} borrowed): the anchor is not the cheap half the design \
         assumed, and the shape decision has to be re-made against these numbers",
        kept * 100.0
    );

    // Reported so a reader of the log sees the pair the decision rests on.
    println!(
        "prose text-only {PROSE_SMALL}: {text_only_small_n} allocs / {text_only_small_b} B \
         | {PROSE_BIG}: {text_only_big_n} / {text_only_big_b}  (keeps {:.1}% of the byte win)",
        kept * 100.0
    );
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

/// Round 794 — the arm the shipped type can actually take, and the list decision
/// Round 785 left open.
///
/// Round 782's `borrowed` arm is `&'static str`, which the shipped `LinePart`
/// cannot be: `from_workspace` derives at run time and must own, so one type
/// carrying both a live and a baked projection is a `Cow<'static, str>`. Round
/// 785's design then wrote that step two "is exactly the arm Round 782 measured".
/// It is not — it is a third holding, and Round 792 is the round that established
/// what this crate gets wrong when it reasons about an arm instead of measuring
/// it. So the arm gets measured.
///
/// It also settles the ONE thing that design deliberately deferred: whether the
/// entity list has to be borrowable too. Both arms here carry identical literals
/// and differ in that field alone.
#[test]
fn the_cow_arm_pays_for_its_lists_and_nothing_else() {
    // The instrument first, and for this test's own license: the shipped shape
    // must charge more at four times the store, or nothing below can be read.
    let (owned_small_n, _) = cost_of(owned_small::build);
    let (owned_big_n, owned_big_b) = cost_of(owned_big::build);
    assert!(
        owned_big_n > 3 * owned_small_n,
        "the owned arm charged {owned_big_n} allocations at {BIG} lines against \
         {owned_small_n} at {SMALL}: it did not grow with the store, so the \
         counter is not seeing the work"
    );

    let (cow_small_n, cow_small_b) = cost_of(cow_small::build);
    let (cow_big_n, cow_big_b) = cost_of(cow_big::build);
    let (list_small_n, list_small_b) = cost_of(cow_list_small::build);
    let (list_big_n, list_big_b) = cost_of(cow_list_big::build);
    let (borrowed_big_n, borrowed_big_b) = cost_of(borrowed_big::build);

    // Each arm asserts its OWN shape before it is compared to anything (the Round
    // 792 discipline). `cow` holds a `Vec` per line, so it must still charge at
    // least once per line — otherwise it is not the arm it is named for and the
    // gap measured below is a gap between something else.
    assert!(
        cow_small_n >= SMALL && cow_big_n >= BIG,
        "the cow arm charged under one allocation per line ({cow_small_n} at \
         {SMALL}, {cow_big_n} at {BIG}), so its per-line `Vec` is not there and \
         this is not the arm being priced"
    );
    // And `cow_list` holds no per-line list at all, so it must charge FEWER than
    // one per line. The complement of the assertion above, so the two together
    // say exactly where the residual sits rather than quoting a fitted constant.
    assert!(
        list_small_n < SMALL && list_big_n < BIG,
        "the cow_list arm still charged at least one allocation per line \
         ({list_small_n} at {SMALL}, {list_big_n} at {BIG}): its residual is not \
         the per-chunk spine it should be"
    );

    // THE FINDING THE SHAPE TURNS ON. A `Cow` costs nothing to hold borrowed, so
    // the arm the kernel can actually take must land on the fully borrowed arm's
    // count — not near it, ON it, because what remains in both is the same `Vec`
    // of chunks over the same number of elements. Equality rather than a ratio:
    // there is no threshold to pick and no room to be approximately right.
    assert_eq!(
        list_big_n, borrowed_big_n,
        "the cow_list arm charged {list_big_n} allocations against the fully \
         borrowed arm's {borrowed_big_n} over identical data: `Cow::Borrowed` is \
         not free after all, and the design's step two does not inherit Round \
         782's result"
    );

    // What each arm keeps of the win, which is the number the deferred decision
    // was deferred FOR. Round 785 predicted 80% for the plain `Vec` and 99% for
    // the borrowable list; both are asserted, so a prediction that missed shows
    // up here rather than in a summary.
    let full_win = (owned_big_n - borrowed_big_n) as f64;
    let cow_kept = (owned_big_n - cow_big_n) as f64 / full_win;
    let list_kept = (owned_big_n - list_big_n) as f64 / full_win;
    assert!(
        cow_kept < 0.90,
        "the plain-`Vec` arm kept {:.1}% of the allocation win, which is not the \
         partial result the design costed the list decision against",
        cow_kept * 100.0
    );
    assert!(
        list_kept > 0.99,
        "the borrowable-list arm kept only {:.1}% of the allocation win: the list \
         is not the whole remainder, so something else is still allocating per \
         line and the shape decision has to be re-made against that",
        list_kept * 100.0
    );

    println!(
        "line owned    {SMALL}: {owned_small_n} allocs | {BIG}: {owned_big_n} / {owned_big_b} B"
    );
    println!(
        "line cow      {SMALL}: {cow_small_n} allocs / {cow_small_b} B | {BIG}: \
         {cow_big_n} / {cow_big_b}  (keeps {:.1}% of the call win)",
        cow_kept * 100.0
    );
    println!(
        "line cow_list {SMALL}: {list_small_n} allocs / {list_small_b} B | {BIG}: \
         {list_big_n} / {list_big_b}  (keeps {:.1}% of the call win)",
        list_kept * 100.0
    );
    println!(
        "line borrow   {BIG}: {borrowed_big_n} allocs / {borrowed_big_b} B  (the \
         `&'static str` arm the shipped type cannot be)"
    );
}
