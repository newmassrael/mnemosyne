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
//!
//! AND THE ARC'S TERMINATION COULD BE REACHED BY WRITING A SENTENCE (R1298).
//! A row is retired by prose, that prose names the commit that retired it, and
//! nothing asked git whether the commit existed — so `0` was reachable by
//! closing rows against commits that were never made. It had already happened
//! once, unfabricated: R1297's row said `커밋 4a4d0e0` before that commit
//! existed, and this census counted the row retired for several turns. So the
//! shas a retirement names are RESOLVED here, a retirement naming one that does
//! not resolve retires nothing, and `--repo` is required whenever the ledger
//! names any commit at all — being unable to check is `2`, never `0`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Whether this repository has the commit a retirement named.
///
/// `^{commit}` RATHER THAN A BARE NAME, so a sha that happens to name a tree or
/// a tag does not read as the commit a retirement claims.
fn has_commit(repo: &Path, sha: &str) -> Result<bool, String> {
    let answer = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(format!("{sha}^{{commit}}"))
        .output()
        .map_err(|why| format!("git could not be run in {}: {why}", repo.display()))?;
    match answer.status.code() {
        // 0 is "this is a commit", 1 is "no such object" — anything else is git
        // failing to answer, which must not read as either.
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git could not say whether {sha} is a commit in {}: {}",
            repo.display(),
            String::from_utf8_lossy(&answer.stderr).trim()
        )),
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
                    eprintln!("[open-debts] --ledger needs a path to the outstanding-debt ledger");
                    return ExitCode::from(2);
                }
            },
            "--repo" => match arguments.next() {
                Some(path) => repo = Some(PathBuf::from(path)),
                None => {
                    eprintln!("[open-debts] --repo needs the path of the repository whose commits the ledger cites");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: open-debts --ledger <path> [--repo <path>]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!(
                    "[open-debts] unknown argument {other} — usage: open-debts --ledger <path> [--repo <path>]"
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
    // THE NAMES A RETIREMENT GIVES ARE RESOLVED BEFORE ANYTHING IS COUNTED
    // RETIRED (R1298). A closure that names a commit is a checkable claim, and
    // this census believed one that was false for several turns.
    let named = open_debts::commits_named_by_retirements(&text);
    let mut unresolved: BTreeSet<String> = BTreeSet::new();
    if !named.is_empty() {
        let Some(repo) = repo.as_deref() else {
            eprintln!(
                "[open-debts] NO VERDICT — {} retirement(s) name a commit and no --repo was \
                 given to check them against. Being unable to check is not a pass",
                named.len()
            );
            for (sha, line) in &named {
                eprintln!("[open-debts]   unchecked: {sha} (line {line})");
            }
            return ExitCode::from(2);
        };
        for (sha, line) in &named {
            match has_commit(repo, sha) {
                Ok(true) => {}
                Ok(false) => {
                    unresolved.insert(sha.clone());
                }
                Err(why) => {
                    eprintln!("[open-debts] NO VERDICT — {why}");
                    eprintln!("[open-debts]   it was asked about {sha}, named on line {line}");
                    return ExitCode::from(2);
                }
            }
        }
    }
    let retired = open_debts::retired(&text, &unresolved);
    let open = open_debts::open_autonomous(&text, &unresolved);
    println!(
        "[open-debts] {} registration(s) in {}, {} id(s) retired, {} of {} named commit(s) resolved",
        all.len(),
        ledger.display(),
        retired.len(),
        named.len() - unresolved.len(),
        named.len()
    );
    // A RETIREMENT AGAINST A COMMIT THAT IS NOT THERE IS PRINTED WHATEVER BRANCH
    // IT SITS IN, and it alone stops the exit code being 0. The row it fails to
    // retire may be a ② or a ③, which the walk below never looks at — so
    // leaving this to the open set would let the arc terminate on a false
    // closure filed under another branch.
    if !unresolved.is_empty() {
        println!(
            "[open-debts] {} retirement(s) name a commit this repository does not have — \
             they retire NOTHING:",
            unresolved.len()
        );
        for sha in &unresolved {
            let line = named.get(sha).copied().unwrap_or(0);
            println!("[open-debts]   {sha} (line {line}) is not a commit here");
        }
    }
    // AND EVERY RETIREMENT THIS READER REFUSED IS NAMED, whatever branch its row
    // sits in. One of them on the real ledger sits outside the autonomous
    // branch, where the walk below would never have shown it: the count would
    // simply have been different, and a census that changes its answer without
    // saying which row moved is the thing this program was written to replace.
    //
    // PRINTED, NOT BLOCKING, AND THAT ASYMMETRY IS DELIBERATE. A retirement
    // naming a commit that does not exist is a claim this ledger makes that is
    // FALSE, and "the arc is finished" must not be printed over one. A
    // retirement naming nothing is a claim nobody can chase — a gap in the
    // notation, not a lie — and where it matters it already blocks by itself:
    // the row it fails to retire stays in the walk below. A gate that took the
    // whole arc hostage over notation would be the exemption-shaped mistake in
    // the other direction.
    let refused = open_debts::refused_retirements(&text, &unresolved);
    if !refused.is_empty() {
        println!(
            "[open-debts] {} retirement(s) refused — the row is NOT retired anywhere else:",
            refused.len()
        );
        for (id, (line, why)) in &refused {
            let said = match why {
                open_debts::Refusal::NamesNothing => {
                    "names no round, commit, day or owner's word — nothing to chase"
                }
                open_debts::Refusal::NamesAMissingCommit => {
                    "names a commit this repository does not have"
                }
            };
            println!("[open-debts]   {id} (line {line}) {said}");
        }
    }
    if open.is_empty() && unresolved.is_empty() {
        println!(
            "[open-debts] the autonomous branch is EMPTY — everything left is a \
             limit, a cost judgement, an owner's word or history"
        );
        return ExitCode::SUCCESS;
    }
    if open.is_empty() {
        println!(
            "[open-debts] the autonomous branch is empty, but the arc is NOT finished: \
             a closure above names a commit that does not exist"
        );
        return ExitCode::from(1);
    }
    println!("[open-debts] {} open in the autonomous branch:", open.len());
    for row in &open {
        println!("[open-debts]   {} (line {})", row.id, row.line);
    }
    ExitCode::from(1)
}
