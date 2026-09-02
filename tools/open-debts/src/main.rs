//! `open-debts --ledger <path>` — what is still this session's to do, and the
//! three-code answer to whether the debt arc is finished.
//!
//! THE EXIT CODE IS THE TERMINATION CONDITION, which is the whole reason this is
//! a program. "The ① branch is empty" was a thing a person read off a pasted
//! one-liner, and a person read it wrong three times. Now:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and the autonomous branch is EMPTY — the arc's condition is met |
//! | 1 | judged, and these rows are still open |
//! | 2 | NOT judged — the ledger could not be read, or holds no registrations |
//!
//! THE THIRD IS THE ONE THAT MATTERS HERE. A ledger this cannot parse produces
//! no rows, and no rows is indistinguishable from a finished arc — which is the
//! failure this repository keeps paying for under the name "a green that means
//! nobody looked". So an empty walk is a REFUSAL, not a celebration.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut ledger: Option<PathBuf> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--ledger" => match arguments.next() {
                Some(path) => ledger = Some(PathBuf::from(path)),
                None => {
                    eprintln!("[open-debts] --ledger needs a path to the outstanding-debt ledger");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: open-debts --ledger <path>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!(
                    "[open-debts] unknown argument {other} — usage: open-debts --ledger <path>"
                );
                return ExitCode::from(2);
            }
        }
    }
    // NO DEFAULT PATH, deliberately. The ledger lives outside this repository and
    // under a home directory whose name is a machine's; a census with a built-in
    // target is one that answers about the wrong file without saying so.
    let Some(ledger) = ledger else {
        eprintln!("[open-debts] usage: open-debts --ledger <path>");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&ledger) {
        Ok(text) => text,
        Err(why) => {
            eprintln!(
                "[open-debts] NO VERDICT — {} could not be read: {why}",
                ledger.display()
            );
            return ExitCode::from(2);
        }
    };

    let all = open_debts::registrations(&text);
    if all.is_empty() {
        eprintln!(
            "[open-debts] NO VERDICT — {} holds no registration of the shape \
             `**<id>**(<branch>)`. A walk that found nothing and a ledger with \
             nothing left in it print the same silence",
            ledger.display()
        );
        return ExitCode::from(2);
    }
    let retired = open_debts::retired(&text);
    let open = open_debts::open_autonomous(&text);
    println!(
        "[open-debts] {} registration(s) in {}, {} id(s) retired",
        all.len(),
        ledger.display(),
        retired.len()
    );
    if open.is_empty() {
        println!(
            "[open-debts] the autonomous branch is EMPTY — everything left is a \
             limit, a cost judgement, an owner's word or history"
        );
        return ExitCode::SUCCESS;
    }
    println!("[open-debts] {} open in the autonomous branch:", open.len());
    for row in &open {
        println!("[open-debts]   {} (line {})", row.id, row.line);
    }
    ExitCode::from(1)
}
