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
//!
//! AND THE ROUND BESIDE THAT COMMIT WENT ON BEING BELIEVED (Round 1313). The
//! argument above reached exactly one of the four names a retirement may give,
//! and it was not the one this ledger writes most: a closure citing a round
//! four digits past anything that exists retired its row on sight for the whole
//! life of the rule. THE NUMBER IS DESCRIBED AND NOT SPELLED, because
//! `tools/*/src/` is scanned by the code-citation gate and writing the example
//! here would BE the hallucinated citation — R1306 met the same trap in the
//! hook that scans for it. It is the same hole with a
//! different name in it, and this session ran with it open — at 16:57 the census
//! counted `N263` retired by `R1311` while the store had no such entry, which
//! did not exist until 17:06. So a round is resolved the way a commit is, by
//! asking the one resolver this repository has for the question, and a
//! retirement naming one that does not resolve retires nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use open_debts::Unresolved;

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

/// This repository's one resolver for "the CLI of this checkout".
///
/// NOT `mnemosyne-cli` FROM PATH, and the reason is written at length in the
/// script itself: `~/.cargo/bin` is a shared slot a sibling checkout can own,
/// and a preferred `target/release` artifact can be older than the store it is
/// judging. A census that asked the wrong binary would answer about the wrong
/// store in silence, which is this crate's founding complaint.
const RESOLVER: &str = "scripts/mn";

/// Whether the atomic store in this repository has the round a retirement named.
///
/// SPELLED `Round <digits>` FROM WHATEVER THE LEDGER WROTE, because the ledger
/// writes `R1299` and `Round 1299` for one thing and the store's key is the
/// second. The digits are the identity; the prefix is notation.
///
/// AND THE ANSWER IS ONLY TRUSTED AS "NO" ONCE THE RESOLVER HAS SHOWN IT CAN
/// SAY "YES" — see `resolver_answers`. This call reads exit 0 as "the store has
/// it" and anything else as "it does not", which is safe only downstream of
/// that probe: on its own, a CLI that will not build answers every question the
/// same way a store missing every round would.
fn has_round(repo: &Path, round: &str) -> Result<bool, String> {
    let digits: String = round.chars().filter(char::is_ascii_digit).collect();
    let answer = Command::new(repo.join(RESOLVER))
        .current_dir(repo)
        .args(["query", "--changelog-entry"])
        .arg(format!("Round {digits}"))
        .output()
        .map_err(|why| format!("{} could not be run in {}: {why}", RESOLVER, repo.display()))?;
    Ok(answer.status.success())
}

/// Whether the resolver can answer a question about this store at all.
///
/// THE COMMIT AXIS GETS THIS FROM GIT FOR FREE and this one has to buy it.
/// `rev-parse` says 0 for "yes", 1 for "no such object" and something else when
/// git itself failed, so `has_commit` can refuse to guess. The CLI has one
/// failure code: `… is not in the atomic store` and `mnemosyne.toml not found`
/// are both exit 1, and so is a checkout where the CLI does not build.
/// Without a probe, a census run on such a tree reports every round the ledger
/// ever named as a hallucinated citation. How many that is, is a number this
/// program prints rather than one written here from memory: its own summary
/// line says `N of N named round(s) resolved`, and on the ledger this was built
/// against N is ninety-one. None of them would be true, and all of them would
/// be printed in the words of a real finding. A gate that reddens on things
/// that are not its subject is one people turn off.
///
/// THE PROBE IS A QUESTION THE STORE MUST BE ABLE TO ANSWER and not a question
/// about any round, so it cannot be confused with the judgement it protects.
fn resolver_answers(repo: &Path) -> Result<(), String> {
    let answer = Command::new(repo.join(RESOLVER))
        .current_dir(repo)
        .args(["query", "--list-changelog", "--limit", "1"])
        .output()
        .map_err(|why| format!("{} could not be run in {}: {why}", RESOLVER, repo.display()))?;
    if answer.status.success() {
        return Ok(());
    }
    Err(format!(
        "{}/{RESOLVER} could not read this repository's changelog at all, so \
         nothing it says about a round is an answer: {}",
        repo.display(),
        String::from_utf8_lossy(&answer.stderr).trim()
    ))
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
                    eprintln!("[open-debts] --repo needs the path of the repository whose commits and whose store the ledger cites");
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
    let commits = open_debts::commits_named_by_retirements(&text);
    // BOTH AXES OR NEITHER (Round 1313). Splitting the requirement — check the
    // commits when a commit is named, check the rounds when a round is — would
    // put back the exact asymmetry this round is paying off, one flag apart.
    let rounds = open_debts::rounds_named_by_retirements(&text);
    let mut unresolved = Unresolved::default();
    if !commits.is_empty() || !rounds.is_empty() {
        let Some(repo) = repo.as_deref() else {
            eprintln!(
                "[open-debts] NO VERDICT — {} retirement name(s) ({} commit(s), {} round(s)) \
                 have nothing to be checked against: no --repo was given. Being unable to \
                 check is not a pass",
                commits.len() + rounds.len(),
                commits.len(),
                rounds.len()
            );
            for (sha, line) in &commits {
                eprintln!("[open-debts]   unchecked commit: {sha} (line {line})");
            }
            for (round, line) in &rounds {
                eprintln!("[open-debts]   unchecked round: {round} (line {line})");
            }
            return ExitCode::from(2);
        };
        for (sha, line) in &commits {
            match has_commit(repo, sha) {
                Ok(true) => {}
                Ok(false) => {
                    unresolved.commits.insert(sha.clone());
                }
                Err(why) => {
                    eprintln!("[open-debts] NO VERDICT — {why}");
                    eprintln!("[open-debts]   it was asked about {sha}, named on line {line}");
                    return ExitCode::from(2);
                }
            }
        }
        if !rounds.is_empty() {
            // BEFORE ANY ROUND IS JUDGED ON A SILENCE, and never after: a
            // resolver that cannot answer says "no" in the same voice as a
            // store that genuinely lacks the round.
            if let Err(why) = resolver_answers(repo) {
                eprintln!("[open-debts] NO VERDICT — {why}");
                eprintln!(
                    "[open-debts]   {} round(s) this ledger's retirements name went unasked",
                    rounds.len()
                );
                return ExitCode::from(2);
            }
            for (round, line) in &rounds {
                match has_round(repo, round) {
                    Ok(true) => {}
                    Ok(false) => {
                        unresolved.rounds.insert(round.clone());
                    }
                    Err(why) => {
                        eprintln!("[open-debts] NO VERDICT — {why}");
                        eprintln!(
                            "[open-debts]   it was asked about {round}, named on line {line}"
                        );
                        return ExitCode::from(2);
                    }
                }
            }
        }
    }
    let retired = open_debts::retired(&text, &unresolved);
    let open = open_debts::open_autonomous(&text, &unresolved);
    println!(
        "[open-debts] {} registration(s) in {}, {} id(s) retired, {} of {} named commit(s) \
         resolved, {} of {} named round(s) resolved",
        all.len(),
        ledger.display(),
        retired.len(),
        commits.len() - unresolved.commits.len(),
        commits.len(),
        rounds.len() - unresolved.rounds.len(),
        rounds.len()
    );
    // A RETIREMENT AGAINST A COMMIT THAT IS NOT THERE IS PRINTED WHATEVER BRANCH
    // IT SITS IN, and it alone stops the exit code being 0. The row it fails to
    // retire may be a ② or a ③, which the walk below never looks at — so
    // leaving this to the open set would let the arc terminate on a false
    // closure filed under another branch.
    if !unresolved.commits.is_empty() {
        println!(
            "[open-debts] {} retirement(s) name a commit this repository does not have — \
             they retire NOTHING:",
            unresolved.commits.len()
        );
        for sha in &unresolved.commits {
            let line = commits.get(sha).copied().unwrap_or(0);
            println!("[open-debts]   {sha} (line {line}) is not a commit here");
        }
    }
    // AND THE SAME FOR THE ROUND (Round 1313), printed apart from the commit
    // because the repair is a different one: a sha that does not resolve is
    // usually a closure written before the commit was made, while a round that
    // does not is a citation to an entry the ledger never got.
    if !unresolved.rounds.is_empty() {
        println!(
            "[open-debts] {} retirement(s) name a round the atomic store does not have — \
             they retire NOTHING:",
            unresolved.rounds.len()
        );
        for round in &unresolved.rounds {
            let line = rounds.get(round).copied().unwrap_or(0);
            println!("[open-debts]   {round} (line {line}) is not an entry in the store");
        }
    }
    // AND EVERY RETIREMENT THIS READER REFUSED IS NAMED, whatever branch its row
    // sits in. One of them on the real ledger sits outside the autonomous
    // branch, where the walk below would never have shown it: the count would
    // simply have been different, and a census that changes its answer without
    // saying which row moved is the thing this program was written to replace.
    //
    // AND A REFUSAL BLOCKS THE ARC FROM FINISHING (R1299), which is a reversal of
    // the line R1298 drew one round earlier. That round printed an unattributed
    // retirement without blocking, arguing that a gate holding the arc hostage
    // over notation would be the exemption-shaped mistake pointing the other
    // way — an argument made WITHOUT KNOWING HOW MANY ROWS IT EXCUSED. Measured
    // on the real ledger it excused exactly one, `N147`, whose closing round is
    // written six lines above it in the same file. A rule that lets a claim
    // through because checking looked expensive, on a population nobody counted,
    // is the escape hatch this repository keeps paying for; and an advisory line
    // printed by a program is prose, which is the thing this crate exists to
    // stop being the answer.
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
                open_debts::Refusal::NamesAMissingRound => {
                    "names a round the atomic store does not have"
                }
            };
            println!("[open-debts]   {id} (line {line}) {said}");
        }
    }
    // THE TERMINATION CONDITION IS ASKED OF THE LIBRARY (R1299), where a law can
    // ask it too. It was three conditions spelled out here, and the one question
    // this whole arc ends on had no reader but a person.
    if open_debts::finished(&text, &unresolved) {
        println!(
            "[open-debts] the autonomous branch is EMPTY — everything left is a \
             limit, a cost judgement, an owner's word or history"
        );
        return ExitCode::SUCCESS;
    }
    if open.is_empty() {
        println!(
            "[open-debts] the autonomous branch is empty, but the arc is NOT finished: \
             a closure above is a claim this ledger cannot support"
        );
        return ExitCode::from(1);
    }
    println!("[open-debts] {} open in the autonomous branch:", open.len());
    for row in &open {
        println!("[open-debts]   {} (line {})", row.id, row.line);
    }
    ExitCode::from(1)
}
