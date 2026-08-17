//! A stand-in for the two programs this gate talks to, whose answers are DATA.
//!
//! ⚠ THE PROGRAM IS BUILT AND THE ANSWER IS A FILE, and the first draft of this
//! crate's cases had it the other way round: each wrote a shell script and ran
//! it. R1192 measured what that costs under `scripts/check-side-workspaces.sh` —
//! `ETXTBSY`, the kernel refusing to `exec` a file some process holds open for
//! writing, because a sibling thread's `fork` inherited the descriptor across
//! exactly the window in which this thread runs the file. It is green alone and
//! red beside ten other crates, which is the normal case. `tools/written-
//! executable` is the law that now refuses the shape, and the repair it asks for
//! is this one: cargo builds the program before any case starts, and what varies
//! per case is a file nobody executes.
//!
//! # Which answer, decided from the arguments
//!
//! One binary stands in for both seams, because a spawned child inherits its
//! parent's environment and two stubs distinguished by a variable would be one
//! stub. The arguments already say which call this is:
//!
//! | contains | it is | reads |
//! |---|---|---|
//! | `--explain-choice` | the placement program being ASKED where a command goes | `choice` |
//! | `BatchMode=yes` | the transport fetching a log | `fetch` |
//! | neither | the placement program being TOLD to dispatch | `send` |
//!
//! # The file
//!
//! First line `exit=<n>`; everything after it is the output. A file that is not
//! there is exit 0 and silence — the answer a program gives when a case has
//! nothing to say about that call. Every invocation appends its arguments, one
//! per line, to `calls`, so a case can ask what was sent as well as what came
//! back.
//!
//! A CLEAN ANSWER GOES TO STDOUT AND A FAILING ONE TO STDERR, which is how both
//! programs it stands in for behave: a successful `cat` over a connection prints
//! the file, and a connection that cannot be made prints its reason on the other
//! stream. A stub that put everything on stdout would let a gate pass a case by
//! quoting a failure it had not actually read from the right place.

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some(answers) = std::env::var_os("ONE_MACHINE_ANSWERS") else {
        eprintln!("answering-program: ONE_MACHINE_ANSWERS names no directory");
        return ExitCode::from(2);
    };
    let answers = std::path::PathBuf::from(answers);

    if let Ok(mut calls) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(answers.join("calls"))
    {
        for argument in &arguments {
            let _ = writeln!(calls, "{argument}");
        }
    }

    let which = if arguments.iter().any(|word| word == "--explain-choice") {
        "choice"
    } else if arguments.iter().any(|word| word.contains("BatchMode=yes")) {
        "fetch"
    } else {
        "send"
    };
    let Ok(text) = std::fs::read_to_string(answers.join(which)) else {
        return ExitCode::SUCCESS;
    };
    let (head, body) = text.split_once('\n').unwrap_or((text.as_str(), ""));
    let code: u8 = head
        .strip_prefix("exit=")
        .and_then(|number| number.parse().ok())
        .unwrap_or(0);
    if code == 0 {
        print!("{body}");
    } else {
        eprint!("{body}");
    }
    ExitCode::from(code)
}
