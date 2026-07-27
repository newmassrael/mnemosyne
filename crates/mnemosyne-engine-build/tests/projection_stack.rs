//! What the baked artifact costs to BUILD, in stack (Round 780).
//!
//! # The axis this crate was not measuring
//!
//! Round 775 split the emitted source into bounded function bodies and measured
//! the win in compile time. The first consumer then reported a second thing that
//! split had fixed, on an axis nobody here had looked at: before it, compiling
//! the artifact at `opt-level = 0` produced a binary that **aborted at run time**
//! with `fatal runtime error: stack overflow`. The cause is the shape. When the
//! whole store is one function body holding one nested expression, every
//! temporary in it is live until the expression ends, and each gets its own
//! slot — so the frame grows with the store until it exceeds the thread's stack.
//!
//! That matters because `opt-level = 0` is cargo's DEFAULT dev profile. The
//! consumer survived by having set `[profile.dev] opt-level = 1` for unrelated
//! reasons, and died the moment it lowered that value. Anyone building with the
//! defaults met the abort.
//!
//! The optimizer is worth being exact about, because the tempting reading — that
//! `opt-level = 1` fixed it and the default merely exposed it — is wrong, and
//! measurably so. Optimizing the control here HALVED what it wanted (96 KiB to
//! 44 KiB at one size, 344 KiB to 160 KiB at four times it) and left the growth
//! rate untouched: 3.6x for 4x the store, at both settings. So the consumer's
//! `opt-level = 1` never made the pre-Round-775 artifact safe. It doubled the
//! store size at which the abort arrived, on a cost that grows with the store —
//! the crash was scheduled, not avoided. Which is also why this gate does not
//! care what profile it runs at: the shape it weighs survives optimization.
//!
//! Round 775 removed it. Nothing here noticed either way: the fixtures in
//! `build.rs` are a couple of lines long, and the Round 775 regression test
//! weighs the emitted TEXT (the longest line), never a running artifact. A
//! property whose only evidence is that a different property held is not gated —
//! it is inferred, and it stops being true silently.
//!
//! # What is asserted, and why none of it is a threshold
//!
//! A number like "must build within 256 KiB" is fitted to today's store; a store
//! that grows past it turns a real gate into an argument about the constant. So
//! the claim here is a RATIO — the artifact must cost the same stack at four
//! times the size — which is the Round 775 scaling assertion moved from the text
//! to the running artifact.
//!
//! A ratio has its own failure mode: "did not grow" is also what a broken
//! measurement says. So the same test measures a CONTROL — the identical parts
//! emitted with the bound removed, the pre-Round-775 shape — and requires it to
//! grow. One test, not two, because that arm is what licenses the others: split
//! apart, the flat arms could report green while the instrument sat broken in
//! another test's red. This is the Rounds 776-779 lesson in its constructive
//! form. Those rounds removed three gates that answered "clean" for input they
//! could not see; the way not to write a fourth is to make the gate demonstrate,
//! on every run, that it can see.
//!
//! Measured when this landed, on the fixtures below: playable 32 KiB at both
//! sizes, quest 28 KiB at both, control 96 KiB then 344 KiB — 3.6x for 4x the
//! store, against a shipped artifact that did not move a single page. Injecting
//! the removed bound back into the emitter put the shipped artifacts on the
//! control's curve (playable 92 KiB to 332 KiB, quest 76 KiB to 272 KiB) and
//! turned every assertion below red, which is how those assertions are known to
//! be live rather than merely green.
//!
//! Round 786 moved one of those figures and left the rest untouched: with the
//! baked entry point handing back a `&'static` instead of the projection by
//! value, the quest artifact went from 28 KiB at both sizes to under one page at
//! both, while playable stayed at 32 KiB and the control at 96 then 344 KiB.
//! Measured by rebuilding the probe against both emitter shapes with nothing
//! else changed. Why the quest axis moved and the playable one did not is NOT
//! explained here, because it was not measured — only that it did.

use std::process::Command;

// `OPT_LEVEL`, `SMALL` and `BIG`, from the build script that emitted the
// fixtures — the only place that knows them.
include!(concat!(env!("OUT_DIR"), "/stack_build_facts.rs"));

/// Cargo builds this and hands over the path, so the binary under test is this
/// package's, at this profile, guaranteed current. Finding it by searching
/// `target/` would reintroduce exactly the stale-artifact failure `verify.sh`
/// exists to prevent.
const PROBE: &str = env!("CARGO_BIN_EXE_projection_stack_probe");

/// Bisection bounds. `CEILING` is generous — well past any figure a bounded
/// artifact could want — and a fixture that does not build within it is reported
/// as unbuildable rather than pinned to a wrong number.
const CEILING: usize = 8 * 1024 * 1024;
/// One page: finer resolution than this measures allocation granularity rather
/// than the artifact.
const GRAIN: usize = 4 * 1024;
/// What a plain `std::thread::spawn` hands a thread — the stack the artifact
/// actually gets in a consumer, and the one the pre-Round-775 shape overran.
const DEFAULT_THREAD_STACK: usize = 2 * 1024 * 1024;

/// Did `fixture` build within `stack` bytes?
///
/// # Panics
///
/// If the OS refused a thread that small: that is not a failed build, and
/// scoring it as one would fabricate a measurement out of a platform floor.
fn builds_within(fixture: &str, stack: usize) -> bool {
    let run = Command::new(PROBE)
        .args([fixture, &stack.to_string()])
        .output()
        .expect("run the probe");
    match run.status.code() {
        Some(0) => true,
        Some(2) => panic!(
            "the OS refused a {stack}-byte thread, so {fixture} was never allowed \
             to try: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        ),
        _ => false,
    }
}

/// The smallest stack `fixture` builds within, to [`GRAIN`].
///
/// Measured twice and required to agree. A bisection that does not repeat is not
/// a measurement, and a gate built on one would fail intermittently — which is
/// how a gate gets called flaky and then gets ignored.
///
/// [`GRAIN`] is the smallest figure this can report, and the OS rounds a
/// too-small request up to its own minimum on top of that — so a result AT
/// `GRAIN` means "at most one page", not "one page". It is a real upper bound
/// (the probe genuinely ran within that request and returned 0), and it is not a
/// measurement of the cost.
///
/// Round 780 wrote that every assertion here is an upper bound or a ratio
/// between figures well above the floor. That stopped being true of the quest
/// arm in Round 786: handing back a `&'static` rather than the projection by
/// value took the quest artifact from 28 KiB at both sizes to under one page at
/// both, so its ratio is now drawn between two floor readings. The ratio still
/// discriminates — a quest artifact wanting 8 KiB at `BIG` fails it, and that is
/// four times MORE sensitive than the same assertion was at 28 KiB — but it is
/// blind to growth that stays entirely under a page, and the honest name for a
/// 4096 in that arm is a resolution limit rather than a cost.
fn stack_needed(fixture: &str) -> usize {
    let measure = || {
        assert!(
            builds_within(fixture, CEILING),
            "{fixture} did not build within {CEILING} bytes — the bisection has no \
             upper bound to start from"
        );
        let (mut small, mut big) = (0, CEILING);
        while big - small > GRAIN {
            let mid = (small + big) / 2 / GRAIN * GRAIN;
            if builds_within(fixture, mid) {
                big = mid;
            } else {
                small = mid;
            }
        }
        big
    };
    let first = measure();
    let second = measure();
    assert_eq!(
        first, second,
        "{fixture} needed {first} bytes and then {second}: the measurement is not \
         reproducible, so nothing below it can be trusted"
    );
    first
}

#[test]
fn the_baked_artifact_costs_one_stack_however_big_the_store_gets() {
    // FIRST, the instrument. The control is the shipped emitter with a single
    // difference — no bound on the body — over the same parts, so if the
    // measurement can see anything at all it sees this.
    let control_small = stack_needed("control_small");
    let control_big = stack_needed("control_big");
    assert!(
        control_big > 2 * control_small,
        "the unbounded control needed {control_big} bytes at {BIG} lines against \
         {control_small} at {SMALL}: it did not grow with the store, so this run \
         measured nothing and the flatness below would be vacuous. The emitter no \
         longer distinguishes its two bounds, or something folded the difference \
         away — this build's OPT_LEVEL is {OPT_LEVEL}, and while an optimizing \
         one was measured to still show the growth, an unmeasured setting may \
         not."
    );

    // The bound is what makes the difference, at one size, with everything else
    // held equal — same parts, same values, same types.
    let playable_small = stack_needed("playable_small");
    assert!(
        control_small > playable_small,
        "the bounded artifact needed {playable_small} bytes and the unbounded one \
         {control_small} over identical parts: the bound is not what the emitter \
         is doing"
    );

    // The claim. Quadruple the store; the cost must not follow.
    let playable_big = stack_needed("playable_big");
    assert!(
        playable_big < 2 * playable_small,
        "the playable artifact needed {playable_small} bytes at {SMALL} lines and \
         {playable_big} at {BIG}: its stack cost grows with the store, which is \
         the pre-Round-775 shape returning"
    );

    // Repeated on the quest axis rather than inferred from the playable one.
    // `render_quest` is free to stop calling the shared chunker while `render`
    // still calls it, and assuming across that seam is how the Round 769 defect
    // survived to Round 770.
    //
    // Both figures sit at GRAIN since Round 786 — see `stack_needed` for what a
    // floor reading is and is not. The ratio is kept rather than swapped for
    // `quest_big <= GRAIN`, because that would be a threshold fitted to today's
    // store, which is the thing this test refuses to assert anywhere else.
    let quest_small = stack_needed("quest_small");
    let quest_big = stack_needed("quest_big");
    assert!(
        quest_big < 2 * quest_small,
        "the quest artifact needed {quest_small} bytes at {SMALL} quests and \
         {quest_big} at {BIG}: its stack cost grows with the store"
    );

    // And the consumer-facing form of it: what a plain spawned thread hands the
    // artifact, against what the artifact wants. This is the figure the first
    // consumer's crash was about.
    for (what, needed) in [("playable", playable_big), ("quest", quest_big)] {
        assert!(
            needed * 8 <= DEFAULT_THREAD_STACK,
            "the {what} artifact needed {needed} bytes, leaving under 8x headroom \
             in the {DEFAULT_THREAD_STACK}-byte stack a plain thread gets"
        );
    }
}
