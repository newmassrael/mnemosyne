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
//!
//! # Round 1044 — the artifact was still growing, and this test could not see it
//!
//! That "under one page at both" is where the gate went blind. A page is the
//! floor this instrument can report, so the quest arm's ratio was drawn between
//! two readings that were not measurements, and it passed over an artifact whose
//! cost was already stepping: at Round 1043's emitter the quest artifact wanted
//! under a page to 800 quests and 28 KiB at 1,600, and the playable one 32 KiB
//! at 800 against 64 KiB at 3,200 — both outside the sample points. Nothing here
//! said so; it surfaced because an unrelated field added to `QuestWorldPart`
//! moved the quest step down onto the 800 the gate samples.
//!
//! The cause was the reassembly `chunked` handed back: one `v.extend(f_i())`
//! statement per chunk, in one frame, and at `opt-level = 0` every statement's
//! temporaries get their own slot. Round 775 bounded the bodies and left their
//! reassembly unbounded. It is now chunked by the same rule, recursively (see
//! `chunked_over`), and every reading is a resolved one rather than a floor,
//! which is what the assertion below requires of any figure a ratio is drawn
//! from.
//!
//! The four figures that round wrote here — "playable 32 KiB and quest 32 KiB at
//! BOTH sizes with the control at 128 then 512 KiB" — are not what this gate
//! produces, and three of them never were. Run against the very tree that
//! shipped that sentence, at the pair this gate defaults to, the probe answers
//! playable 32 KiB, quest 28 KiB, control 96 KiB then 344 KiB — the figures
//! Round 780 wrote above, a page out on quest and a third out on the control.
//!
//! Round 1046 first wrote that the origin was "likely the oversized pair that
//! round measured at", and Round 1047 measured it instead and found otherwise.
//! Re-running with `MN_FIXTURE_LINES=1200` gives a second pair: playable and
//! quest sit at 32 KiB and 28 KiB at 300 and 1200 items, exactly as at 200 and
//! 800, with the playable control at 136 KiB then 508 KiB. Four measured
//! control points — 96 at 200, 136 at 300, 344 at 800, 508 at 1200 — place 128
//! KiB at roughly 285 items and 512 KiB at roughly 1,210, and those are not
//! four times apart, so NO pair this gate can be run at yields that sentence.
//! Where the figures came from is not recoverable from this tree: that round
//! measured candidate emitter shapes in a worktree it then discarded, and a
//! reading from one of those is the only remaining explanation.
//!
//! Nothing caught any of it because this test asserted booleans and printed
//! nothing: the numbers lived only in prose, where no program could disagree
//! with them. It prints its whole table now, every run, pass or fail — and the
//! correction above is what an unmeasured sentence costs, since the round that
//! set out to fix exactly this shipped one of its own.
//!
//! # Round 1046 — which artifacts this weighed, and which it did not
//!
//! Two of four. `render`, `render_quest`, `render_map` and `render_passages` all
//! reassemble through the `chunked_over` the round above rewrote, and only the
//! first two had a fixture — so the change that moved every emitter's frame
//! shape was measured on half of them. The reason is worth naming because it is
//! the failure this repository keeps finding under different clothes: the
//! population was a LIST, written once in `build.rs`, once in the probe, and
//! once here, and a list cannot report the member nobody wrote into it. Round
//! 774 had already stated the principle for the quest axis — an emitter is free
//! to stop calling the shared chunker, so assuming across that seam is how the
//! Round 769 defect survived to Round 770 — and the two emitters added after it
//! were never brought under it.
//!
//! The population is now the type `Baked`, and every list here is derived from
//! `Baked::ALL`: the fixtures, the probe's dispatch over them, and the loop
//! below. A fifth artifact gets a reading by existing.
//!
//! The CONTROL moved with it, from one to one per artifact, and that is the
//! substantive half. A control is not evidence about an emitter; it is evidence
//! about a FIXTURE. An unbounded playable artifact growing says the instrument
//! can see a playable fixture grow, and says nothing about whether the map
//! fixture grows anything at all — a map fixture that varied nothing would read
//! flat, pass, and mean nothing, which is precisely the vacuous green the Rounds
//! 776-779 lesson above is about. Each artifact now carries its own parts at an
//! unbounded chunk, so every flat reading is licensed by a growing one over the
//! same data.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::process::Command;

/// One artifact and the four fixtures it is weighed through, as the build script
/// named them.
///
/// The names are carried rather than composed here. This gate used to build them
/// from a tag and a spelling rule of its own — `{tag}_{size}`, `{tag}_control_…`
/// — which is the emitter's convention written down a second time, in the one
/// file whose job is to disagree with the emitter when something drifts.
pub struct Weighed {
    what: &'static str,
    small: &'static str,
    big: &'static str,
    control_small: &'static str,
    control_big: &'static str,
}

// `OPT_LEVEL`, `SMALL` and `BIG`, shared with the allocation gate, and then
// `ARTIFACTS` — the population itself — from the build script that emitted the
// fixtures, the only place that knows either.
include!(concat!(env!("OUT_DIR"), "/stack_build_facts.rs"));
include!(concat!(env!("OUT_DIR"), "/stack_artifacts.rs"));

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

/// Every fixture the probe was compiled with, asked of the probe rather than
/// assumed (Round 1046).
fn fixtures_the_probe_carries() -> BTreeSet<String> {
    let run = Command::new(PROBE)
        .arg("--list")
        .output()
        .expect("ask the probe what it carries");
    assert!(
        run.status.success(),
        "the probe could not list its fixtures: {}",
        String::from_utf8_lossy(&run.stderr).trim()
    );
    String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

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
///
/// ROUND 1044 REFUTED "still discriminates". The arm passed for the whole
/// interval over an artifact that was already stepping — under a page to 800
/// quests, 28 KiB at 1,600 — because `big < 2 * small` against a denominator
/// meaning "at most one page" is a claim about this function's resolution, not
/// about the artifact. A floor reading is now a hard failure where a ratio is
/// drawn from it, so the property Round 780 described is enforced.
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
fn every_baked_artifact_costs_one_stack_however_big_the_store_gets() {
    // Every reading, emitted whatever the verdict is — to stdout, which cargo
    // shows under `--nocapture`, and into the failure message otherwise, so
    // neither outcome is a bare boolean. A gate that reports only its own
    // verdict makes the next round re-measure what this one already knew, and
    // these four numbers per artifact are the whole content of the question.
    // Round 1044 wrote four figures into this file's header that no run
    // produces (see above); numbers a program never emits are the ones that
    // drift.
    let mut table = String::new();
    // Every FAILING claim, rather than the first one. Four artifacts times five
    // claims is twenty answers, and an assertion that stops at the first turns
    // the run into a single bit — the shape that let Round 1044's blindness be
    // described as "the quest arm" when the emitter change was global.
    let mut wrong: Vec<String> = Vec::new();

    // BEFORE ANY OF IT: what this gate weighs against what the probe carries.
    //
    // Everything below is a loop over `ARTIFACTS`, and a loop over an empty list
    // asserts nothing while reporting green — the exact shape of the failure
    // this round exists to repair, one level down from where it repaired it. So
    // the two sets are compared: the fixtures compiled into the probe, and the
    // fixtures named in the table this test iterates. A fixture emitted and
    // weighed by nothing fails here, and so does a population that lost a
    // member.
    let carried: BTreeSet<String> = fixtures_the_probe_carries();
    let weighed: BTreeSet<String> = ARTIFACTS
        .iter()
        .flat_map(|a| [a.small, a.big, a.control_small, a.control_big])
        .map(str::to_string)
        .collect();
    assert_eq!(
        carried,
        weighed,
        "the probe carries {} fixture(s) and this gate weighs {}: a fixture \
         nothing weighs is not measured, and a gate that only names what it \
         already intended to measure cannot say so",
        carried.len(),
        weighed.len()
    );

    for a in ARTIFACTS {
        let what = a.what;
        // FIRST, the instrument, over THIS artifact's own parts. The control is
        // the shipped emitter with a single difference — no bound on the body —
        // over the same data, so if the measurement can see anything at all it
        // sees this.
        let control_small = stack_needed(a.control_small);
        let control_big = stack_needed(a.control_big);
        let small = stack_needed(a.small);
        let big = stack_needed(a.big);
        let _ = writeln!(
            table,
            "{what}: {small} -> {big} bytes at {SMALL} -> {BIG} items, \
             control {control_small} -> {control_big}"
        );

        if control_big <= 2 * control_small {
            wrong.push(format!(
                "the unbounded {what} control needed {control_big} bytes at {BIG} \
                 items against {control_small} at {SMALL}: it did not grow with \
                 the store, so this run measured nothing about {what} and the \
                 flatness below would be vacuous. The emitter no longer \
                 distinguishes its two bounds, the fixture no longer grows, or \
                 something folded the difference away — this build's OPT_LEVEL is \
                 {OPT_LEVEL}, and while an optimizing one was measured to still \
                 show the growth, an unmeasured setting may not."
            ));
        }

        // The bound is what makes the difference, at one size, with everything
        // else held equal — same parts, same values, same types.
        if control_small <= small {
            wrong.push(format!(
                "the bounded {what} artifact needed {small} bytes and the \
                 unbounded one {control_small} over identical parts: the bound is \
                 not what the emitter is doing"
            ));
        }

        // A RATIO DRAWN FROM A FLOOR READING IS NOT A MEASUREMENT (Round 1044).
        //
        // Between Rounds 786 and 1044 both quest figures sat at GRAIN, and the
        // doc above this test argued the ratio "still discriminates" there. It
        // does not. A reading AT GRAIN says "at most one page" — the artifact may
        // want 100 bytes or 4,096 — so `big < 2 * small` against it is a claim
        // about the resolution of the instrument, and it passed for a quest
        // artifact whose cost was ALREADY stepping: measured at Round 1043's
        // tree, the artifact wanted under a page to 800 quests and 28 KiB at
        // 1,600, with both sample points below the step. The blindness was found
        // by a change that moved the step down to 800, not by anything this gate
        // said.
        //
        // So the readings the ratios are drawn from must themselves be above the
        // floor. This is not a threshold fitted to today's store — it does not
        // care what the figure IS, only that the instrument resolved it — and it
        // is the one claim here that would have failed at Round 1043.
        for (items, needed) in [(SMALL, small), (BIG, big)] {
            if needed <= GRAIN {
                wrong.push(format!(
                    "the {what} artifact read {needed} bytes at {items} items, \
                     which is the {GRAIN}-byte resolution floor rather than a cost \
                     — every ratio drawn from this reading compares the instrument \
                     to itself. Raise MN_FIXTURE_LINES until the artifact is \
                     resolved rather than weakening the claim."
                ));
            }
        }

        // The claim. Quadruple the store; the cost must not follow.
        if big >= 2 * small {
            wrong.push(format!(
                "the {what} artifact needed {small} bytes at {SMALL} items and \
                 {big} at {BIG}: its stack cost grows with the store, which is the \
                 pre-Round-775 shape returning"
            ));
        }

        // And the consumer-facing form of it: what a plain spawned thread hands
        // the artifact, against what the artifact wants. This is the figure the
        // first consumer's crash was about.
        if big * 8 > DEFAULT_THREAD_STACK {
            wrong.push(format!(
                "the {what} artifact needed {big} bytes, leaving under 8x headroom \
                 in the {DEFAULT_THREAD_STACK}-byte stack a plain thread gets"
            ));
        }
    }

    print!("{table}");
    assert!(
        wrong.is_empty(),
        "{} of the claims this gate makes about {} baked artifact(s) failed:\n{}\n\
         \nwhat was measured:\n{table}",
        wrong.len(),
        ARTIFACTS.len(),
        wrong.join("\n")
    );
    // The population, printed with the readings. A run that weighed fewer
    // artifacts than the last one should say so where the numbers are read, not
    // only where they are asserted.
    println!("{} baked artifact(s) weighed", ARTIFACTS.len());
}
