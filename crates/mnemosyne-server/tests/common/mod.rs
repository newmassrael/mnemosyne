//! What every wait in this crate's integration tests goes through, and the one
//! budget that bounds them.
//!
//! Round 1081 found four waits here that ended on a clock instead of on a
//! condition, and five budgets spelled at their own sites. The clock ones are
//! the class that turned this repository's main branch red in Round 1073 from
//! `bench`: an assertion whose subject is how the machine interleaved, wearing
//! the clothes of an assertion about the code. One of the four did not even
//! have a red side — `zero_rate_sampling_drops_all_spans` slept 300ms and then
//! required that the collector had received nothing, so a wait too SHORT passed
//! it, for exactly the reason the test exists to rule out.
//!
//! The law those repairs are shaped by, and which `tools/blind-waits` enforces
//! on every commit: **a wait must end on a condition, and the budget that
//! bounds it must be named.**
//!
//! So there are two things here and only two:
//!
//! - [`LIVENESS`], the single budget. Its job is to turn a hang into a failure
//!   and nothing else — no test's GREEN may depend on its value, only its
//!   ability to fail at all.
//! - [`until_ok`] and [`quiesce`], the two shapes of ending on a condition:
//!   retry until the system answers, and drive the system to rest so there is
//!   nothing left to wait for.

#![allow(dead_code)] // each test binary uses a different part of this module

use std::future::Future;
use std::time::{Duration, Instant};

/// THE liveness budget for this crate's tests — one decision, in one place.
///
/// It is deliberately far larger than anything measured: the slowest of these
/// waits resolves in single-digit milliseconds on this machine, and CI runs on
/// a shared four-vCPU runner where a hundredfold pause is a scheduling event
/// rather than a defect. A budget that is generous cannot be the reason a test
/// is red; a budget that is tight is a performance assertion nobody wrote down
/// as one, which is what `500` meant at each of the five sites it used to be
/// spelled at.
pub const LIVENESS: Duration = Duration::from_secs(30);

/// How often a poll re-asks. Not a budget — shortening it only costs cycles,
/// lengthening it only costs latency, and NO verdict depends on it.
const POLL: Duration = Duration::from_millis(5);

/// Retry `attempt` until it answers, or fail saying what never happened and
/// what the last failure was.
///
/// The shape matters more than the budget: the loop ends when the SYSTEM says
/// so. On a machine ten times slower this polls ten times more and returns the
/// same verdict, which is the property a `sleep` of a fixed length cannot have.
pub async fn until_ok<T, E, F, Fut>(what: &str, mut attempt: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let deadline = Instant::now() + LIVENESS;
    // Declared without a value: every path that reaches the assertion has gone
    // through the `Err` arm below, so an initial string would be a value no
    // reader ever sees — which is what clippy says and what it would be.
    let mut last;
    loop {
        match attempt().await {
            Ok(value) => return value,
            Err(failure) => last = failure.to_string(),
        }
        assert!(
            Instant::now() < deadline,
            "{what} never succeeded within {LIVENESS:?}; last failure: {last}"
        );
        tokio::time::sleep(POLL).await;
    }
}

/// Bring a traced server to rest, so that a test asking what the collector
/// received is asking a question with a settled answer.
///
/// **The chain, in the order it has to happen:**
///
/// 1. Every request the test made is already awaited, so none is in flight.
/// 2. `shutdown` + `server.await` — tonic's graceful shutdown returns only
///    once its connection tasks are done, which is once every handler future
///    has been dropped, which is when `tracing` closes the handler's span and
///    `tracing-opentelemetry` hands it to the batch processor. Joining the
///    server is what makes that a JOIN rather than a race against the response
///    write.
/// 3. `drop(guard)` — the `OtlpTracerGuard` shuts the tracer provider down.
///    The SDK sends `Shutdown` on the SAME channel the spans went down, behind
///    them, and blocks until the worker has exported the flush AND the
///    collector has answered the export RPC (opentelemetry_sdk 0.26:
///    `BatchSpanProcessor::shutdown` blocks on a oneshot the worker resolves
///    after `flush().await`; the mock collector pushes to its capture before
///    returning its response).
///
/// After step 3 the collector holds everything it will ever hold. There is
/// nothing to wait for, and so nothing here waits.
///
/// **The negative assertion's control.** `zero_rate_sampling_drops_all_spans`
/// asserts an ABSENCE, and no amount of waiting can establish one. What makes
/// it non-vacuous is that the two tests asserting a PRESENCE go through this
/// same function: if the chain did not deliver, they would be red. That is why
/// this is a function and not three comments — a shared claim cannot drift
/// between the test that would catch it failing and the test that depends on
/// it holding.
pub async fn quiesce<G>(
    guard: G,
    shutdown: tokio::sync::oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
) {
    shutdown.send(()).ok();
    server.await.ok();
    drop(guard);
}
