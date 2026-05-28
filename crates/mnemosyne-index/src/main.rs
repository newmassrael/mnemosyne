//! Admin driver for the materialized RocksDB fact index (Convergence B
//! read-side, Round 332).
//!
//! This is the RocksDB-side operational entry point that makes the index
//! load-bearing: it drives [`rebuild_index`] (materialize the index from the
//! atomic log) and [`IndexReader`] (serve point queries from it). It lives on
//! the RocksDB subgraph on purpose — the authoring binaries (`mnemosyne-cli`,
//! `mnemosyne-mcp`) carry no `store`/`facts` edge, so the write path never pays
//! the RocksDB cost (ARCHITECTURE.md persistence model; the point of Round 328).
//!
//! The atomic log is the single source of truth; the index this tool writes is
//! a derived, rebuildable view (ARCHITECTURE.md anti-drift invariant #2), so
//! `rebuild` is always safe to re-run.
//!
//! Usage:
//!   mnemosyne-index rebuild --atomic <log.json> --index <dir>
//!   mnemosyne-index get-section <section_id> --index <dir>
//!   mnemosyne-index get-entry <round_number> --index <dir>

use std::path::PathBuf;
use std::process::ExitCode;

use mnemosyne_atomic::{AtomicStore, MAIN_BRANCH_ID};
use mnemosyne_index::{rebuild_index, IndexReader};
use mnemosyne_store::MnemosyneStore;

const USAGE: &str = "\
mnemosyne-index — materialize and query the RocksDB fact index from the atomic log.

USAGE:
    mnemosyne-index rebuild      --atomic <log.json> --index <dir>
    mnemosyne-index get-section  <section_id>        --index <dir>
    mnemosyne-index get-entry    <round_number>      --index <dir>
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("mnemosyne-index: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let cmd = args
        .next()
        .ok_or_else(|| format!("missing subcommand\n\n{USAGE}"))?;
    let rest: Vec<String> = args.collect();
    match cmd.as_str() {
        "rebuild" => cmd_rebuild(&rest),
        "get-section" => cmd_get_section(&rest),
        "get-entry" => cmd_get_entry(&rest),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown subcommand '{other}'\n\n{USAGE}")),
    }
}

/// Value of a required `--<name> <value>` flag.
fn flag_value(args: &[String], name: &str) -> Result<PathBuf, String> {
    let flag = format!("--{name}");
    let pos = args
        .iter()
        .position(|a| *a == flag)
        .ok_or_else(|| format!("missing required flag {flag}\n\n{USAGE}"))?;
    let value = args
        .get(pos + 1)
        .ok_or_else(|| format!("flag {flag} requires a value"))?;
    Ok(PathBuf::from(value))
}

/// First non-flag argument (a flag and its value are both skipped).
fn positional(args: &[String]) -> Option<&String> {
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg.starts_with("--") {
            skip_next = true;
            continue;
        }
        return Some(arg);
    }
    None
}

fn cmd_rebuild(args: &[String]) -> Result<(), String> {
    let atomic_path = flag_value(args, "atomic")?;
    let index_path = flag_value(args, "index")?;

    let atomic = AtomicStore::load(&atomic_path)
        .map_err(|e| format!("load atomic log {}: {e}", atomic_path.display()))?;
    let store = MnemosyneStore::open(&index_path)
        .map_err(|e| format!("open index {}: {e}", index_path.display()))?;
    let stats =
        rebuild_index(&atomic, &store, MAIN_BRANCH_ID).map_err(|e| format!("rebuild: {e}"))?;

    println!(
        "rebuilt index at {}: {} sections, {} changelog entries, {} cross-refs ({} rows total)",
        index_path.display(),
        stats.sections,
        stats.changelog_entries,
        stats.cross_refs,
        stats.total(),
    );
    Ok(())
}

fn cmd_get_section(args: &[String]) -> Result<(), String> {
    let section_id = positional(args)
        .ok_or_else(|| format!("get-section requires a <section_id>\n\n{USAGE}"))?;
    let index_path = flag_value(args, "index")?;
    let store = MnemosyneStore::open(&index_path)
        .map_err(|e| format!("open index {}: {e}", index_path.display()))?;
    let reader = IndexReader::new(&store);

    match reader
        .section(section_id, MAIN_BRANCH_ID)
        .map_err(|e| format!("read: {e}"))?
    {
        Some(fact) => {
            println!("section_id: {}", fact.section_id);
            println!("title: {}", fact.skeleton.title);
            println!("parent_doc: {}", fact.skeleton.parent_doc);
            match &fact.skeleton.parent_section {
                Some(p) => println!("parent_section: {p}"),
                None => println!("parent_section: <none>"),
            }
            match &fact.skeleton.decision_status {
                Some(s) => println!("decision_status: {s:?}"),
                None => println!("decision_status: <none>"),
            }
        }
        None => println!("section '{section_id}' not found in index"),
    }
    Ok(())
}

fn cmd_get_entry(args: &[String]) -> Result<(), String> {
    let round_raw = positional(args)
        .ok_or_else(|| format!("get-entry requires a <round_number>\n\n{USAGE}"))?;
    let round: u64 = round_raw
        .parse()
        .map_err(|_| format!("invalid round number '{round_raw}'"))?;
    let index_path = flag_value(args, "index")?;
    let store = MnemosyneStore::open(&index_path)
        .map_err(|e| format!("open index {}: {e}", index_path.display()))?;
    let reader = IndexReader::new(&store);

    match reader
        .changelog_entry(round, MAIN_BRANCH_ID)
        .map_err(|e| format!("read: {e}"))?
    {
        Some(fact) => {
            println!("round_number: {}", fact.round_number);
            println!("summary: {}", fact.summary);
        }
        None => println!("round {round} not found in index"),
    }
    Ok(())
}
