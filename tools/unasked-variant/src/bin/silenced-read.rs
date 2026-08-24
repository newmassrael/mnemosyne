//! The same question as `unasked-variant`, asked of this repository's SHELL:
//! where does a script turn "could not" into "nothing"?
//!
//! R1283, AND R1281 FOUND THE FIRST ONE BY HAND. `.githooks/pre-commit` read the
//! list of staged files with `|| true`, so a failed read produced the same value
//! as a repository with nothing staged — and four of its gates are keyed on that
//! list, so an unreadable index walked through every one of them and the hook
//! exited 0 on a commit no rule had been applied to. One hook over, `commit-msg`
//! had ended its Unicode rules in `2>/dev/null`, which made every way of going
//! wrong look like "no match", and it took a second machine to see THAT one.
//!
//! # What this counts, and why it counts rather than refuses
//!
//! Three clauses substitute a value for a failure: `|| true` and `|| :` swallow
//! the status, `2>/dev/null` swallows the reason, and `set +e` swallows both for
//! everything after it. Every one of them is sometimes right — a probe whose
//! failure IS the answer wants `|| true`, and a `command -v` that prints to
//! stderr on a machine without the tool wants `2>/dev/null`.
//!
//! WHAT DECIDES IS WHAT THE SUBSTITUTED VALUE IS THEN USED FOR, and that is not
//! a line pattern. So this prints a census with the file and the line, and the
//! judgement stays with whoever reads it. A gate that refused all of them would
//! be the one people learn to ignore — which the sibling crate has been taught
//! once already, by a compiler.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// The clauses that put a value where a failure was.
const SILENCERS: [(&str, &str); 4] = [
    (
        "|| true",
        "the status is swallowed and the value substituted is success",
    ),
    ("|| :", "the same, spelled with the null command"),
    (
        "2>/dev/null",
        "the reason is swallowed; every way of going wrong reads alike",
    ),
    (
        "set +e",
        "every command after it stops being able to stop the script",
    ),
];

fn main() {
    let root = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let scripts = ci_plan::script_files(&root);
    println!(
        "[silenced-read] {} tracked shell script(s) this repository runs",
        scripts.len()
    );

    let mut by_clause: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut lines_read = 0usize;
    for path in &scripts {
        let text = std::fs::read_to_string(root.join(path))
            .unwrap_or_else(|why| panic!("read {path}: {why}"));
        for (number, line) in text.lines().enumerate() {
            lines_read += 1;
            // A COMMENT IS PROSE ABOUT A CLAUSE AND NOT ONE. Several of these
            // files explain the very defect this counts, in sentences that
            // contain the clause.
            if line.trim_start().starts_with('#') {
                continue;
            }
            for (clause, _) in SILENCERS {
                if line.contains(clause) {
                    by_clause
                        .entry(clause)
                        .or_default()
                        .push(format!("{path}:{}", number + 1));
                }
            }
        }
    }

    println!("[silenced-read] {lines_read} line(s) read, comments skipped");
    for (clause, why) in SILENCERS {
        let found = by_clause.get(clause).map_or(0, Vec::len);
        println!("[silenced-read] `{clause}` — {found} — {why}");
        for at in by_clause.get(clause).into_iter().flatten() {
            println!("[silenced-read]     {at}");
        }
    }
}
