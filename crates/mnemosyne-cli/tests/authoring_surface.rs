//! THE LAWS OVER ONE SWEEP (Round 1071).
//!
//! Three walks ask the same panel of advertised reads about the same derived
//! corruption population, and until this round each built that population for
//! itself. Measured in one suite run: 220.53s + 191.08s + 139.94s — 9m12s, and
//! the dominant term in a CI job that had reached 26m45s of a 30-minute budget
//! (14m21s at Round 1060, 21m6s at Round 1061). The population is not three
//! populations; it is one substrate with three consumers, and the reason it was
//! built three times is that separate integration-test files are separate
//! BINARIES, which share nothing.
//!
//! So they are one binary now. [`sweep::sweep`] builds the corpus, the panel
//! and every trial ONCE for the process, and the laws below read it:
//!
//! - [`play_break_class_census`] — what stands between an authoring slip and
//!   the runtime: each corruption bucketed REFUSED / CAUGHT / REPORTED /
//!   CARRIED / INERT, and every boundary-crossing resize put to the authored
//!   corpora as a rule they might already break.
//! - [`read_agreement_population`] — which subjects more than one shipped read
//!   answers about, measured by perturbation rather than by a heuristic.
//! - [`counted_without_naming`] — the numbers a read reports without naming
//!   what they count.
//! - [`render_gates_reach`] — that the two render-acceptance gates are on the
//!   panel at all, that the fixture they were handed is not empty, and that
//!   some authorable edit moves what they say (Round 1072).
//!
//! Each keeps its own name, its own printout and its own assertions: the point
//! is one sweep, not one law. What moved is where the population comes from.
//!
//! Round 1074 gave the sweep an OWNER the compiler enforces. While it sat in
//! `tests/common/`, every test binary in this crate could call it, and calling
//! it from a second binary shares nothing — the memoization is per process, so
//! the second one rebuilds all 312 stores and re-asks every advertised read,
//! silently, for the whole cost this round was built to pay once. It now lives
//! in [`sweep`], which refuses to compile into any target but this one.

mod common;

#[path = "surface/sweep.rs"]
mod sweep;

#[path = "surface/counted_without_naming.rs"]
mod counted_without_naming;
#[path = "surface/play_break_class_census.rs"]
mod play_break_class_census;
#[path = "surface/read_agreement_population.rs"]
mod read_agreement_population;
#[path = "surface/render_gates_reach.rs"]
mod render_gates_reach;
