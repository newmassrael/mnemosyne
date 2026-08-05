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
//! projection_stack_probe --list
//!   every fixture compiled in, one per line
//! ```
//!
//! The `exit 2` arm is the one worth naming: a platform's minimum thread size is
//! a floor on what can be asked, and a probe that reported "did not build" for
//! "was never allowed to try" would hand the parent a fabricated number. Below
//! the floor there is no measurement, and saying so is the honest answer.
//!
//! `--list` exists so the gate can ask what this binary CARRIES instead of
//! assuming its own list is the whole of it (Round 1046). The two must be equal:
//! a fixture nothing weighs is the defect that round repaired one level up, and
//! a gate that only ever names fixtures it already intends to measure cannot
//! see it — including the degenerate case where the population is empty and
//! every claim below passes by never running.

// The fixture modules, the arms, and `build`'s dispatch over them — GENERATED
// by `build.rs` from the same loop that writes the fixtures (Round 1046).
//
// This file carried all three by hand: a `mod` per fixture, an arm per fixture,
// and a `match` naming each one. That is the fixtures-that-exist list and the
// fixtures-that-can-be-measured list kept equal by review, and they were not —
// two of the four baked artifacts had no fixture here at all, so the emitter
// changes Round 1044 made to `chunked_over` went unweighed on the map and
// passage axes. The population is `Baked::ALL` now; see `write_stack_arms` for
// what the generated text is and why each arm is its own `#[inline(never)]`
// function (the Round 804 pooling defect).
include!(concat!(env!("OUT_DIR"), "/stack_arms.rs"));

fn main() {
    let mut args = std::env::args().skip(1);
    let fixture = args
        .next()
        .expect("usage: <fixture> <stack-bytes> | --list");
    if fixture == "--list" {
        for name in FIXTURES {
            println!("{name}");
        }
        return;
    }
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
