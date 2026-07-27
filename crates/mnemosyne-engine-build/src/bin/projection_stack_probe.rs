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

/// Build the named artifact, returning a count so the work cannot be optimized
/// away as dead. `None` = no such fixture.
fn build(fixture: &str) -> Option<usize> {
    Some(match fixture {
        "playable_small" => playable_small::playable_projection().walk("main").len(),
        "playable_big" => playable_big::playable_projection().walk("main").len(),
        "quest_small" => quest_small::quest_projection().quests().len(),
        "quest_big" => quest_big::quest_projection().quests().len(),
        "control_small" => control_small::playable_projection().walk("main").len(),
        "control_big" => control_big::playable_projection().walk("main").len(),
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
