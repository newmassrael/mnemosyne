//! Build one baked projection on a thread of an EXACT size, and let the exit
//! status carry the answer (Round 780).
//!
//! A separate process because a stack overflow is not a `Result` — it aborts,
//! and a test that hosted the attempt in its own process would die with it
//! instead of reporting on it. So the measurement lives here and the verdict
//! lives in the parent: `tests/projection_stack.rs` bisects over runs of this
//! binary to find the smallest stack each artifact builds within.
//!
//! ```text
//! projection_stack_probe <fixture> <stack-bytes>
//!   exit 0    built within that stack
//!   exit 2    the OS refused a thread that small — NOT a measurement
//!   killed    overflowed
//! ```
//!
//! The `exit 2` arm is the one worth naming: a platform's minimum thread size is
//! a floor on what can be asked, and a probe that reported "did not build" for
//! "was never allowed to try" would hand the parent a fabricated number. Below
//! the floor there is no measurement, and saying so is the honest answer.

/// The fixtures `build.rs` emits for this binary. Each is the generator's own
/// output, so what is weighed here is the artifact a consumer compiles.
mod playable_small {
    include!(concat!(env!("OUT_DIR"), "/stack_playable_small.rs"));
}
mod playable_big {
    include!(concat!(env!("OUT_DIR"), "/stack_playable_big.rs"));
}
mod quest_small {
    include!(concat!(env!("OUT_DIR"), "/stack_quest_small.rs"));
}
mod quest_big {
    include!(concat!(env!("OUT_DIR"), "/stack_quest_big.rs"));
}
/// The same parts as `playable_*`, emitted with the bound removed — the shape
/// the emitter had before Round 775, kept compiled so the gate can show that its
/// measurement detects the difference rather than asserting it does.
mod control_small {
    include!(concat!(env!("OUT_DIR"), "/stack_control_small.rs"));
}
mod control_big {
    include!(concat!(env!("OUT_DIR"), "/stack_control_big.rs"));
}

/// One fixture per function, and `#[inline(never)]` to keep it that way
/// (Round 804).
///
/// These were six arms of one `match` until this round, and at `opt-level = 0`
/// that is ONE frame holding every arm's temporaries at once — so the figure
/// reported for any fixture carried what the others wanted. Round 804 found it
/// by changing only the playable projection's accessor types and watching the
/// QUEST reading move from 8 KiB to 28 KiB, past the ratio
/// `tests/projection_stack.rs` asserts, while every compiled frame in the quest
/// artifact stayed byte-identical — the emitted quest source did not change by
/// one character.
///
/// That is the failure mode the test's own header warns about one level up: a
/// gate that cannot say WHICH artifact it is weighing reports on the union and
/// calls it the part. Splitting the arms puts each measurement back on its own
/// artifact, and `#[inline(never)]` is what stops the arms from being pooled
/// again at a profile where the optimizer would.
mod arm {
    #[inline(never)]
    pub fn playable_small() -> usize {
        super::playable_small::playable_projection()
            .walk("main")
            .len()
    }
    #[inline(never)]
    pub fn playable_big() -> usize {
        super::playable_big::playable_projection()
            .walk("main")
            .len()
    }
    #[inline(never)]
    pub fn quest_small() -> usize {
        super::quest_small::quest_projection().quests().len()
    }
    #[inline(never)]
    pub fn quest_big() -> usize {
        super::quest_big::quest_projection().quests().len()
    }
    #[inline(never)]
    pub fn control_small() -> usize {
        super::control_small::playable_projection()
            .walk("main")
            .len()
    }
    #[inline(never)]
    pub fn control_big() -> usize {
        super::control_big::playable_projection().walk("main").len()
    }
}

/// Build the named artifact, returning a count so the work cannot be optimized
/// away as dead. `None` = no such fixture.
///
/// Dispatch only: the work is in [`arm`], one function per fixture, so this
/// frame is a string comparison rather than the union of six artifacts.
fn build(fixture: &str) -> Option<usize> {
    Some(match fixture {
        "playable_small" => arm::playable_small(),
        "playable_big" => arm::playable_big(),
        "quest_small" => arm::quest_small(),
        "quest_big" => arm::quest_big(),
        "control_small" => arm::control_small(),
        "control_big" => arm::control_big(),
        _ => return None,
    })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let fixture = args.next().expect("usage: <fixture> <stack-bytes>");
    let stack: usize = args
        .next()
        .expect("usage: <fixture> <stack-bytes>")
        .parse()
        .expect("stack bytes");

    let probe = std::thread::Builder::new()
        .stack_size(stack)
        .spawn(move || build(&fixture).expect("a fixture this binary was built with"));
    let probe = match probe {
        Ok(probe) => probe,
        Err(err) => {
            eprintln!("the OS refused a {stack}-byte thread: {err}");
            std::process::exit(2);
        }
    };
    let built = probe.join().expect("the probe thread finished");
    println!("built {built}");
}
