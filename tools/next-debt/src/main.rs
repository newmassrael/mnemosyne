//! `next-debt --ledger <path> --repo <path>` — the order the standing rules ask
//! for, printed instead of read off by eye.
//!
//! THE THREE LINES ARE THE THREE RULES. Take from `critical` while it is not
//! empty; `unranked` is the count nobody has decided about and it only shrinks;
//! `deferred` is what the re-aiming cap says to register now and repay later.
//! Every one of them was a sentence in an operating rule that named an
//! instrument this repository did not have — measured across `tools/`,
//! `scripts/`, `.githooks/` and `RULEBOOK.md`, nothing printed any of those
//! words about this ledger.
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and the ledger's steering is sound |
//! | 1 | judged, and the unranked ratchet's pin is not what the ledger holds |
//! | 2 | NOT judged — a document could not be read, or the ledger cannot be walked |
//!
//! THE THIRD IS THE ONE THAT MATTERS, and it is the rule this borrows whole from
//! the census next door: a walk that could not run prints the same silence as a
//! ledger with nothing left in it, so being unable to answer is a refusal and
//! never a pass. A rank that says two things, a provenance mark naming a row
//! nobody registered, a chain that returns to itself, a cap no document
//! declares — each is `2`, with the row named.
//!
//! THE ONE THING IT ENFORCES IS THE RATCHET, and that is deliberate rather than
//! modest. `critical` is an answer, not an invariant, and a non-empty critical
//! line is the normal state of a working ledger; `deferred` is an instruction to
//! the round, and rows sitting under it are the cap doing its job. What can be
//! wrong is the count of rows nobody ranked, because the rule about it is a
//! RATCHET — it may fall and it may not rise — and a ratchet whose pin is never
//! read is a sentence. So the pin lives in the ledger, this reads it, and a
//! measurement either side of it is `1` with the replacement line spelled out.
//!
//! WHAT IT DOES NOT ASK. It runs no git and no store query. The names a
//! retirement gives are `open-debts`'s question — that program's exit code is
//! this arc's termination condition and it blocks on a name that does not
//! resolve — and asking them again here would pay the census's thirty-five
//! seconds twice a round for an answer that already has an owner. How many names
//! went unasked is on the summary line rather than in this comment, so a
//! disagreement between the two counts has somewhere to be read.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use next_debt::{DeclarationFault, Fault, Origin, Steering, REAIMING_CAP, UNRANKED_RATCHET};

/// The process document, inside the repository it belongs to.
///
/// DERIVED FROM `--repo` RATHER THAN DEFAULTED. The ledger gets no default path
/// because it lives outside this repository under a home directory whose name is
/// a machine's; this one is a tracked file at a fixed place in the tree the
/// caller just named, which is the same reasoning that lets the census reach
/// `scripts/mn` without being told where it is.
const RULEBOOK: &str = "RULEBOOK.md";

fn read(path: &Path, what: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|why| format!("{what} {} — {why}", path.display()))
}

/// The number a document declares, said in words a reader can act on.
fn declared(text: &str, marker: &str, document: &Path) -> Result<usize, String> {
    next_debt::declared(text, marker).map_err(|why| match why {
        DeclarationFault::Absent => format!(
            "{} declares `{marker}` on no line of its own, so this program has no number \
             to read. It carries none of its own: a value written here as well would be a \
             second home for a datum that has one",
            document.display()
        ),
        DeclarationFault::Repeated { lines } => format!(
            "{} declares `{marker}` on {} lines ({}), so it declares no single value",
            document.display(),
            lines.len(),
            lines
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        DeclarationFault::Unreadable { line, said } => format!(
            "{}:{line} carries `{marker}` and what follows it is not a number: {said}",
            document.display()
        ),
    })
}

/// The census's own order, as one line per row.
fn name(rows: &[&next_debt::Row]) -> Vec<String> {
    rows.iter()
        .map(|row| format!("{} (line {})", row.id, row.line))
        .collect()
}

/// The spread of provenance depths, as the ledger holds it.
fn spread(steering: &Steering) -> String {
    let counted = steering.by_depth();
    if counted.is_empty() {
        return "no row has a depth".to_string();
    }
    counted
        .iter()
        .map(|(depth, rows)| format!("{rows} at {depth}"))
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// Why a fault is a refusal, in the words its repair needs.
fn said(fault: &Fault) -> String {
    match fault {
        Fault::RankIsBoth { id, line } => format!(
            "{id} (line {line}) is written both critical and ordinary — this reader \
             will not choose between them"
        ),
        Fault::OriginNamesNothingRegistered { id, line, named } => format!(
            "{id} (line {line}) says it came from {named}, which this ledger never \
             registers — its chain has no depth"
        ),
        Fault::OriginUnreadable { id, line } => {
            format!("{id} (line {line}) writes the provenance mark and names nothing after it")
        }
        Fault::OriginCycles { ids } => format!(
            "a provenance chain returns to itself: {}",
            ids.join(" \u{2192} ")
        ),
    }
}

fn main() -> ExitCode {
    let mut ledger: Option<PathBuf> = None;
    let mut repo: Option<PathBuf> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--ledger" => match arguments.next() {
                Some(path) => ledger = Some(PathBuf::from(path)),
                None => {
                    eprintln!("[next-debt] --ledger needs a path to the outstanding-debt ledger");
                    return ExitCode::from(2);
                }
            },
            "--repo" => match arguments.next() {
                Some(path) => repo = Some(PathBuf::from(path)),
                None => {
                    eprintln!(
                        "[next-debt] --repo needs the path of the repository whose {RULEBOOK} \
                         declares the re-aiming cap"
                    );
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: next-debt --ledger <path> --repo <path>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!(
                    "[next-debt] unknown argument {other} — usage: next-debt --ledger <path> \
                     --repo <path>"
                );
                return ExitCode::from(2);
            }
        }
    }
    let (Some(ledger), Some(repo)) = (ledger, repo) else {
        eprintln!("[next-debt] usage: next-debt --ledger <path> --repo <path>");
        return ExitCode::from(2);
    };
    let rulebook = repo.join(RULEBOOK);
    let (text, process) = match (
        read(&ledger, "NO VERDICT — the ledger could not be read:"),
        read(
            &rulebook,
            "NO VERDICT — the process document could not be read:",
        ),
    ) {
        (Ok(text), Ok(process)) => (text, process),
        (Err(why), _) | (_, Err(why)) => {
            eprintln!("[next-debt] {why}");
            return ExitCode::from(2);
        }
    };

    let cap = match declared(&process, REAIMING_CAP, &rulebook) {
        Ok(cap) => cap,
        Err(why) => {
            eprintln!("[next-debt] NO VERDICT — {why}");
            return ExitCode::from(2);
        }
    };
    let pin = match declared(&text, UNRANKED_RATCHET, &ledger) {
        Ok(pin) => pin,
        Err(why) => {
            eprintln!("[next-debt] NO VERDICT — {why}");
            return ExitCode::from(2);
        }
    };
    let steering = match next_debt::steering(&text) {
        Ok(steering) => steering,
        Err(faults) => {
            eprintln!(
                "[next-debt] NO VERDICT — {} row(s) this ledger cannot be steered by:",
                faults.len()
            );
            for fault in &faults {
                eprintln!("[next-debt]   {}", said(fault));
            }
            return ExitCode::from(2);
        }
    };
    // AND AN EMPTY WALK IS A REFUSAL, the rule this borrows whole from the census
    // next door: a ledger this could not parse and a ledger with nothing left in
    // it print the same silence, and only one of them is finished.
    if steering.open.is_empty() {
        eprintln!(
            "[next-debt] NO VERDICT — {} holds no open row of the autonomous branch. \
             That is either a finished arc or a walk that read nothing, and this program \
             cannot tell them apart; `open-debts` is the one that judges it",
            ledger.display()
        );
        return ExitCode::from(2);
    }

    let unasked = open_debts::commits_named_by_retirements(&text).len()
        + open_debts::rounds_named_by_retirements(&text).len();
    println!(
        "[next-debt] {} open in the autonomous branch of {}, retirements read as written \
         ({unasked} name(s) they give are resolved by open-debts, not here)",
        steering.open.len(),
        ledger.display()
    );
    let critical = steering.critical();
    if critical.is_empty() {
        println!(
            "[next-debt] critical: none — work the population in whatever order the seams fit"
        );
    } else {
        println!(
            "[next-debt] critical {} — take from these and nothing else:",
            critical.len()
        );
        for row in name(&critical) {
            println!("[next-debt]   {row}");
        }
    }
    let unranked = steering.unranked();
    println!(
        "[next-debt] unranked {unranked}, pinned at {pin} in {}",
        ledger.display()
    );
    println!(
        "[next-debt] depth: {} \u{2014} unrooted {} (registered before the rule; nothing turns on it)",
        spread(&steering),
        steering.unrooted()
    );
    let deferred = steering.deferred(cap);
    println!(
        "[next-debt] deferred {} at depth > {cap} ({REAIMING_CAP}, {})",
        deferred.len(),
        rulebook.display()
    );
    for row in name(&deferred) {
        println!("[next-debt]   {row} — register it, repay it when the cap allows");
    }
    // THE ROWS THE CAP DEFERS ARE NAMED WITH THEIR PARENTS, because "not this
    // round" is only actionable if the reader can see which chain it belongs to.
    for row in &steering.open {
        if let (Origin::Row(parent), Some(depth)) = (&row.origin, steering.depths.get(&row.id)) {
            if *depth > cap {
                println!("[next-debt]     {} came from {parent}", row.id);
            }
        }
    }

    match unranked.cmp(&pin) {
        std::cmp::Ordering::Greater => {
            println!(
                "[next-debt] the ratchet is BROKEN — {} row(s) more than the pin allows. \
                 The repair is to rank the rows, not to raise the pin: a rank is a decision \
                 somebody makes, and the pin is only where the decisions had got to",
                unranked - pin
            );
            ExitCode::from(1)
        }
        // A LOOSE PIN IS A FAILURE TOO, and that is what makes this a ratchet
        // rather than a ceiling. A pin nobody tightens records a state the ledger
        // left behind rounds ago, and every measurement under it passes — which
        // is the shape of a gate that has stopped being read.
        std::cmp::Ordering::Less => {
            println!(
                "[next-debt] the pin is LOOSE — the ledger now holds {unranked}. Tighten it: \
                 the line reading `{UNRANKED_RATCHET} = {pin}` becomes `{UNRANKED_RATCHET} = \
                 {unranked}`"
            );
            ExitCode::from(1)
        }
        std::cmp::Ordering::Equal => {
            println!("[next-debt] the ratchet holds at {pin}");
            ExitCode::SUCCESS
        }
    }
}
