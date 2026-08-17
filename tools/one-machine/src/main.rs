//! `one-machine` — dispatch this tree's census to a machine that is not this
//! one, and judge the answer.
//!
//! | verb | what it does | codes |
//! |---|---|---|
//! | `--send` | ask the placement program where this repository's verification goes, refuse if the answer is HERE, and launch the census there detached | 0 sent or already asked, 2 could not send |
//! | `--read` | fetch what that machine wrote and judge it | 0 clean elsewhere, 1 a finding, 2 not judged |
//! | `--header` | print the line the dispatched machine writes about itself before it starts | 0, 2 |
//! | `--fingerprint` | print the identity of the bytes a dispatch would send | 0, 2 |
//! | `--declared-verify` | print the command this repository declares for verifying itself | 0, 2 |
//!
//! The last three exist so the far side spells nothing of its own: the script
//! that runs there asks this program for the header, and asks `outside-reach`
//! for the trace filter, so a machine's answer is written in the same words the
//! machine reading it uses.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use one_machine::{
    carrying_lines, declared_verify_command, fingerprint, judge, launcher, now, program_under_home,
    read_placement, remote_log, run_bounded, scale_of, started_ago, this_host, Claim, Header, Seen,
    Verdict, CENSUS_SCRIPT, CLAIM,
};

fn usage() -> ExitCode {
    eprintln!(
        "usage: one-machine [--repo <path>] \
         (--send | --read | --header | --fingerprint | --declared-verify)\n\
         \x20      [--bx <path>]   the placement program to ask (default ~/{})\n\
         \x20      [--ssh <path>]  the transport to fetch the answer with (default `ssh`)\n\
         \x20      [--budget-seconds <n>]  how long to wait for either of them \
         before ending the\n\
         \x20                      wait and saying so (default {})",
        one_machine::PROGRAM_UNDER_HOME,
        one_machine::BUDGET.as_secs()
    );
    ExitCode::from(2)
}

struct Arguments {
    repository: PathBuf,
    verb: Option<String>,
    placement_program: Option<PathBuf>,
    transport: PathBuf,
    budget: std::time::Duration,
}

fn parse() -> Result<Arguments, ExitCode> {
    let mut parsed = Arguments {
        repository: PathBuf::from("."),
        verb: None,
        placement_program: None,
        transport: PathBuf::from("ssh"),
        budget: one_machine::BUDGET,
    };
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            // NOT AN ENVIRONMENT VARIABLE. A case wanting a short budget could
            // set one, and then the budget a HOOK runs under would be whatever
            // the machine underneath happens to say. A flag is named at the
            // call, and the callers that do not name it get the constant.
            "--budget-seconds" => {
                let seconds = arguments
                    .next()
                    .and_then(|value| value.parse().ok())
                    .map(std::time::Duration::from_secs);
                let Some(seconds) = seconds else {
                    eprintln!("one-machine: --budget-seconds needs a whole number of seconds");
                    return Err(ExitCode::from(2));
                };
                parsed.budget = seconds;
            }
            "--repo" | "--bx" | "--ssh" => {
                let Some(value) = arguments.next() else {
                    eprintln!("one-machine: {argument} needs a path");
                    return Err(ExitCode::from(2));
                };
                match argument.as_str() {
                    "--repo" => parsed.repository = PathBuf::from(value),
                    "--bx" => parsed.placement_program = Some(PathBuf::from(value)),
                    _ => parsed.transport = PathBuf::from(value),
                }
            }
            "--send" | "--read" | "--header" | "--fingerprint" | "--declared-verify" => {
                if parsed.verb.is_some() {
                    eprintln!("one-machine: one verb at a time");
                    return Err(ExitCode::from(2));
                }
                parsed.verb = Some(argument);
            }
            "--help" | "-h" => return Err(usage()),
            other => {
                eprintln!("one-machine: unknown argument {other}");
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(parsed)
}

fn main() -> ExitCode {
    let parsed = match parse() {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    let repository = &parsed.repository;
    match parsed.verb.as_deref() {
        Some("--fingerprint") => match fingerprint(repository) {
            Ok(digest) => {
                println!("{digest}");
                ExitCode::SUCCESS
            }
            Err(message) => refuse(&message),
        },
        Some("--declared-verify") => match declared_verify_command(repository) {
            Ok(command) => {
                println!("{command}");
                ExitCode::SUCCESS
            }
            Err(message) => refuse(&message),
        },
        Some("--header") => header(repository),
        Some("--send") => send(
            repository,
            parsed.placement_program.as_deref(),
            parsed.budget,
        ),
        Some("--read") => read(repository, &parsed.transport, parsed.budget),
        _ => usage(),
    }
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("[one-machine] NO VERDICT — {message}");
    ExitCode::from(2)
}

/// The line the machine taking the census writes about itself, before it starts.
fn header(repository: &Path) -> ExitCode {
    let (host, digest) = match (this_host(), fingerprint(repository)) {
        (Ok(host), Ok(digest)) => (host, digest),
        (Err(message), _) | (_, Err(message)) => return refuse(&message),
    };
    println!(
        "{}",
        Header {
            host,
            fingerprint: digest,
            started: now(),
        }
        .render()
    );
    ExitCode::SUCCESS
}

/// The name a remote log is filed under — this repository's directory name.
fn repository_name(repository: &Path) -> Result<String, String> {
    repository
        .canonicalize()
        .map_err(|error| format!("{} could not be resolved — {error}", repository.display()))?
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} has no directory name", repository.display()))
}

fn send(
    repository: &Path,
    placement_program: Option<&Path>,
    budget: std::time::Duration,
) -> ExitCode {
    let here = match fingerprint(repository) {
        Ok(digest) => digest,
        Err(message) => return refuse(&message),
    };
    // ALREADY ASKED IS NOT A FAILURE AND NOT A SECOND SEND. This runs from a
    // hook, so it runs on every commit; a dispatch per invocation would rsync
    // this tree to a shared build machine repeatedly and leave the previous
    // census answering about bytes nobody is asking about any more.
    let claim_path = repository.join(CLAIM);
    if let Ok(text) = std::fs::read_to_string(&claim_path) {
        if let Ok(claim) = Claim::read(&text) {
            if claim.fingerprint == here {
                println!(
                    "[one-machine] `{}` was already asked about this tree ({}), {} second(s) \
                     ago — nothing to send",
                    claim.host,
                    &here[..12.min(here.len())],
                    started_ago(claim.launched)
                );
                return ExitCode::SUCCESS;
            }
        }
    }

    let suite = match declared_verify_command(repository) {
        Ok(command) => command,
        Err(message) => return refuse(&message),
    };
    let program =
        match placement_program.map_or_else(program_under_home, |named| Ok(named.to_path_buf())) {
            Ok(path) => path,
            Err(message) => return refuse(&message),
        };

    // ASKED BEFORE IT IS TOLD. `--explain-choice` runs nothing, so the one
    // outcome this gate must never produce — a census taken on the machine that
    // wrote it, reported as a second opinion — is decided before any tree moves.
    let mut asking = Command::new(&program);
    asking.arg("--explain-choice").arg("--");
    asking.args(suite.split_whitespace());
    asking.current_dir(repository);
    let asked = match run_bounded(&mut asking, budget) {
        Ok(output) => output,
        Err(error) => {
            return refuse(&format!(
                "the placement program `{}` {error}",
                program.display()
            ))
        }
    };
    if !asked.status.success() {
        return refuse(&format!(
            "`{} --explain-choice` exited {} — {}",
            program.display(),
            asked
                .status
                .code()
                .map_or_else(|| "on a signal".to_owned(), |code| code.to_string()),
            String::from_utf8_lossy(&asked.stderr).trim()
        ));
    }
    let placement = match read_placement(&String::from_utf8_lossy(&asked.stdout)) {
        Ok(placement) => placement,
        Err(message) => return refuse(&message),
    };
    let Some(host) = placement.host.filter(|_| placement.remote) else {
        return refuse(&format!(
            "the placement program would run this repository's verification HERE — {}. \
             A census taken on the machine that wrote the gate is the answer the gate \
             already had, so nothing was sent",
            placement.why
        ));
    };

    let name = match repository_name(repository) {
        Ok(name) => name,
        Err(message) => return refuse(&message),
    };
    let log = remote_log(&name);
    let mut sending = Command::new(&program);
    sending
        .arg("--label")
        .arg("one-machine")
        .arg("--host")
        .arg(&host)
        .arg("--")
        .arg("bash")
        .arg("-c")
        .arg(launcher(&log, CENSUS_SCRIPT))
        .current_dir(repository);
    let sent = match run_bounded(&mut sending, budget) {
        Ok(output) => output,
        Err(error) => {
            return refuse(&format!(
                "the dispatch through `{}` {error}",
                program.display()
            ))
        }
    };
    print!("{}", String::from_utf8_lossy(&sent.stdout));
    eprint!("{}", String::from_utf8_lossy(&sent.stderr));
    if !sent.status.success() {
        return refuse(&format!(
            "the dispatch to `{host}` exited {} — its own output is above, and no claim was \
             recorded, so nothing here will later read a log that was never written",
            sent.status
                .code()
                .map_or_else(|| "on a signal".to_owned(), |code| code.to_string())
        ));
    }

    // THE CLAIM IS WRITTEN LAST, and only on a dispatch that returned cleanly.
    // A claim recorded beside a launch that failed is the worst state this gate
    // has: `--read` would go looking for an answer, find a stale log or none,
    // and report "not judged" about a machine that was never asked.
    let claim = Claim {
        host: host.clone(),
        log,
        fingerprint: here,
        launched: now(),
    };
    if let Some(parent) = claim_path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            return refuse(&format!(
                "{} could not be created — {error}",
                parent.display()
            ));
        }
    }
    if let Err(error) = std::fs::write(&claim_path, claim.render()) {
        return refuse(&format!(
            "the dispatch to `{host}` was launched and {} could not be written — {error}. \
             The census is running and nothing here will read it",
            claim_path.display()
        ));
    }
    println!(
        "[one-machine] `{host}` is taking the census of this tree — {}. Its answer is read at \
         the next push",
        placement.why
    );
    ExitCode::SUCCESS
}

fn read(repository: &Path, transport: &Path, budget: std::time::Duration) -> ExitCode {
    let here = match fingerprint(repository) {
        Ok(digest) => digest,
        Err(message) => return refuse(&message),
    };
    let host = match this_host() {
        Ok(host) => host,
        Err(message) => return refuse(&message),
    };
    let claim_path = repository.join(CLAIM);
    let claim = match std::fs::read_to_string(&claim_path) {
        Ok(text) => Some(Claim::read(&text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(Err(format!("{} — {error}", claim_path.display()))),
    };
    let log = match &claim {
        Some(Ok(claim)) => fetch(transport, &claim.host, &claim.log, budget),
        _ => Err("nothing was dispatched".to_owned()),
    };
    let seen = Seen {
        claim,
        log,
        here,
        this_host: host,
    };
    let verdict = judge(&seen);
    match &verdict {
        Verdict::Elsewhere { host, seconds } => {
            println!(
                "[one-machine] `{host}` took the census of this tree {seconds} second(s) ago and \
                 found nothing undeclared — this verdict is not only this machine's"
            );
            // AND HOW MUCH IT LOOKED AT, in the far side's own words: "nothing
            // undeclared" is also what a census over an empty trace says, and a
            // clean line with no size beside it is the shape this repository has
            // mistaken for a clean tree before.
            if let Some(scale) = seen.log.as_deref().ok().and_then(scale_of) {
                println!("{scale}");
            }
        }
        Verdict::Finding(message) => {
            // THE OTHER MACHINE'S OWN WORDS, and only the ones that carry the
            // answer. A census log is thousands of lines — 2,766 measured, of
            // which 17 decide — and this runs from a git hook, so printing all
            // of it does not carry the finding to the person pushing, it buries
            // it. Nothing is paraphrased and nothing is hidden: the lines are
            // the far side's own, in order, and the rest is still on the machine
            // that wrote it, named below by the command that reads it.
            if let Ok(text) = &seen.log {
                for line in carrying_lines(text) {
                    println!("{line}");
                }
            }
            if let Some(Ok(claim)) = &seen.claim {
                println!(
                    "[one-machine] the whole log is on `{}` — ssh {} 'cat -- \"{}\"'",
                    claim.host, claim.host, claim.log
                );
            }
            eprintln!("[one-machine] DEFECT {message}");
        }
        Verdict::NotJudged(message) => {
            eprintln!("[one-machine] NO VERDICT — {message}");
        }
    }
    ExitCode::from(verdict.code())
}

/// Fetch a log from the machine that wrote it.
///
/// `$HOME` in the path is left for the FAR side's shell to expand — this
/// program cannot know the remote account, and a path built from a local `HOME`
/// names a directory on the wrong machine.
fn fetch(
    transport: &Path,
    host: &str,
    log: &str,
    budget: std::time::Duration,
) -> Result<String, String> {
    let mut command = Command::new(transport);
    command
        .arg("-o")
        .arg("BatchMode=yes")
        .arg(host)
        .arg(format!("cat -- \"{log}\""));
    let output = run_bounded(&mut command, budget)
        .map_err(|error| format!("`{}` {error}", transport.display()))?;
    if !output.status.success() {
        return Err(format!(
            "`{} {host}` exited {}: {}",
            transport.display(),
            output
                .status
                .code()
                .map_or_else(|| "on a signal".to_owned(), |code| code.to_string()),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
