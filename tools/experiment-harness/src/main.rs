//! experiment-harness — fail-loud mechanical steps for a blind A/B authoring
//! experiment.
//!
//! Subcommands:
//!   assemble      story + playthrough + world -> blind reading copy
//!   shuffle       arms -> sealed label map (prints the sha256 seal)
//!   verify-seal   label map + expected sha256 -> match / mismatch (exit 1)
//!
//! Argument parsing is intentionally strict: an unknown flag, a missing value,
//! or a missing required option is a usage error with a non-zero exit. Nothing
//! defaults silently.

mod assemble;
mod playthrough;
mod project;
mod seal;
mod shuffle;
mod splice;
mod story;
mod sustain;
mod util;

use std::process::ExitCode;

use util::{write_file, HResult};

const USAGE: &str = "\
experiment-harness — blind A/B experiment mechanics (fail-loud, reproducible)

USAGE:
  experiment-harness assemble --story <md> --playthrough <json> --world <name> [--titles-from <store.json>] [--out <md>]
  experiment-harness shuffle --experiment <name> [--note <text>] --out <json> <arm> <arm> [arm...]
  experiment-harness verify-seal --map <json> --sha256 <hex>
  experiment-harness cast-sustainment --facts <facts.json> --order <order.json> [--ground-frame <id>] [--principals <n>] [--min-active <n>] [--min-nonprincipal <n>] [--min-frames <n>]
  experiment-harness project-world --store <reextracted.atomic.json> --world <name> --out <store.json> [--main-branch <id>]
  experiment-harness splice --base <manuscript.md> --out <md> --replace <scene.md> [--replace <scene.md>...]

assemble
  Render a world's scenes, in playthrough order, into a blind reading copy.
  Scene bodies are stripped of <!-- --> comments and CHOICE: directives;
  `## sc-NN \u{2014} Title` headings become `## Title`. A world-order scene with no
  prose, a duplicate scene id, or an empty body is a hard error.
  --titles-from <store.json> sources each scene heading from the fact base's
  section titles (neutral, arm-independent headings; also titles a source whose
  headings are the bare `## sc-NN` form). Without --out the manuscript is written
  to stdout.

shuffle
  Assign blind labels A, B, ... to the named arms via a /dev/urandom shuffle,
  write the label map to --out, and print its sha256 (record it in the ledger
  as the seal). At least two distinct arms are required.

verify-seal
  Re-hash --map and compare to --sha256. Match exits 0, mismatch exits 1.

cast-sustainment
  Read a fact base (facts.json + order.json) and report, per world-line, how many
  distinct person-frames stay active in the deep tail (scenes after the fork). The
  thresholds are explicit flags (default: principals 3, min-active 6,
  min-nonprincipal 4, min-frames 10; ground frame `gt`). Exits 1 if the floor fails.

project-world
  Emit the single-world projection of a re-extracted store for validate-render-
  fidelity: keep every narrative_fact on the target world or the spine (--main-branch,
  default `main`), drop sibling-branch facts. A store missing narrative_facts errors.

splice
  Replace named `## sc-NN` scene blocks in --base with the re-rendered --replace
  files (one scene each), leaving every other byte untouched. An unmatched scene id
  is a hard error — the localization of a targeted repair is mechanically guaranteed.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> HResult<ExitCode> {
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    match cmd.as_str() {
        "assemble" => cmd_assemble(&args[1..]),
        "shuffle" => cmd_shuffle(&args[1..]),
        "verify-seal" => cmd_verify_seal(&args[1..]),
        "cast-sustainment" => cmd_cast_sustainment(&args[1..]),
        "project-world" => cmd_project_world(&args[1..]),
        "splice" => cmd_splice(&args[1..]),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown subcommand `{other}` (try --help)")),
    }
}

fn cmd_assemble(args: &[String]) -> HResult<ExitCode> {
    let mut p = Flags::new(args);
    let story = p.require("--story")?;
    let playthrough = p.require("--playthrough")?;
    let world = p.require("--world")?;
    let titles_from = p.optional("--titles-from")?;
    let out = p.optional("--out")?;
    p.finish()?;

    let manuscript = assemble::run(&story, &playthrough, &world, titles_from.as_deref())?;
    match out {
        Some(path) => {
            write_file(&path, &manuscript)?;
            eprintln!("wrote {} bytes to {path}", manuscript.len());
        }
        None => print!("{manuscript}"),
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_shuffle(args: &[String]) -> HResult<ExitCode> {
    let mut p = Flags::new(args);
    let experiment = p.require("--experiment")?;
    let note = p.optional("--note")?.unwrap_or_default();
    let out = p.require("--out")?;
    let arms = p.positionals();
    p.finish()?;

    let hash = shuffle::run(&experiment, &note, &arms, &out)?;
    eprintln!("sealed {} arms to {out}", arms.len());
    // The seal goes to stdout alone, so it can be captured for the ledger.
    println!("{hash}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_verify_seal(args: &[String]) -> HResult<ExitCode> {
    let mut p = Flags::new(args);
    let map = p.require("--map")?;
    let expected = p.require("--sha256")?;
    p.finish()?;

    let verdict = seal::verify(&map, &expected)?;
    if verdict.matched {
        println!("MATCH {} {map}", verdict.computed);
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!("MISMATCH {map}");
        eprintln!("  computed {}", verdict.computed);
        eprintln!("  expected {}", expected.trim().to_lowercase());
        Ok(ExitCode::from(1))
    }
}

fn cmd_cast_sustainment(args: &[String]) -> HResult<ExitCode> {
    let mut p = Flags::new(args);
    let facts = p.require("--facts")?;
    let order = p.require("--order")?;
    let ground_frame = p
        .optional("--ground-frame")?
        .unwrap_or_else(|| "gt".to_string());
    let principals = parse_usize(p.optional("--principals")?, "--principals", 3)?;
    let min_active = parse_usize(p.optional("--min-active")?, "--min-active", 6)?;
    let min_nonprincipal = parse_usize(p.optional("--min-nonprincipal")?, "--min-nonprincipal", 4)?;
    let min_frames = parse_usize(p.optional("--min-frames")?, "--min-frames", 10)?;
    p.finish()?;

    let floor = sustain::Floor {
        ground_frame,
        principals,
        min_active,
        min_nonprincipal,
        min_frames,
    };
    let report = sustain::run(&facts, &order, floor)?;
    println!(
        "person-frames (!= {}): {}",
        report.floor.ground_frame, report.person_frames
    );
    println!(
        "principals (top-{}): {}",
        report.floor.principals,
        report.principals.join(", ")
    );
    for w in &report.worlds {
        println!(
            "  world {}: active={} (non-principal {}) -> {}",
            w.world,
            w.active.len(),
            w.nonprincipal.len(),
            if w.ok { "PASS" } else { "FAIL" }
        );
    }
    println!(
        "FLOOR >= {} frames, >= {} active & >= {} non-principal per tail: {}",
        report.floor.min_frames,
        report.floor.min_active,
        report.floor.min_nonprincipal,
        if report.hold { "HOLD" } else { "FAIL" }
    );
    if report.hold {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn cmd_project_world(args: &[String]) -> HResult<ExitCode> {
    let mut p = Flags::new(args);
    let store = p.require("--store")?;
    let world = p.require("--world")?;
    let out = p.require("--out")?;
    let main_branch = p
        .optional("--main-branch")?
        .unwrap_or_else(|| "main".to_string());
    p.finish()?;

    let (kept, dropped) = project::run(&store, &world, &main_branch, &out)?;
    eprintln!(
        "world `{world}`: kept {kept} fact(s), dropped {dropped} sibling-branch fact(s) -> {out}"
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_splice(args: &[String]) -> HResult<ExitCode> {
    let mut p = Flags::new(args);
    let base = p.require("--base")?;
    let out = p.require("--out")?;
    let replacements = p.take_all("--replace");
    p.finish()?;
    if replacements.is_empty() {
        return Err("splice needs at least one --replace <scene.md>".to_string());
    }
    let used = splice::run(&base, &replacements, &out)?;
    eprintln!("spliced {used} scene(s) into {out}");
    Ok(ExitCode::SUCCESS)
}

fn parse_usize(value: Option<String>, name: &str, default: usize) -> HResult<usize> {
    match value {
        None => Ok(default),
        Some(s) => s
            .parse::<usize>()
            .map_err(|e| format!("{name} must be a non-negative integer: {e}")),
    }
}

/// A tiny strict flag parser: `--name value` options plus bare positionals.
/// Unknown flags and missing values are loud errors at `finish()`/`require()`.
struct Flags {
    opts: Vec<(String, String)>,
    positionals: Vec<String>,
    unconsumed_flags: Vec<String>,
}

impl Flags {
    fn new(args: &[String]) -> Self {
        let mut opts = Vec::new();
        let mut positionals = Vec::new();
        let mut unconsumed_flags = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            if let Some(name) = a.strip_prefix("--") {
                let name = format!("--{name}");
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    opts.push((name, args[i + 1].clone()));
                    i += 2;
                } else {
                    // Flag with no value; record it so require() can report a
                    // precise "missing value" rather than a generic unknown.
                    unconsumed_flags.push(name);
                    i += 1;
                }
            } else {
                positionals.push(a.clone());
                i += 1;
            }
        }
        Flags {
            opts,
            positionals,
            unconsumed_flags,
        }
    }

    fn take(&mut self, name: &str) -> Option<String> {
        if let Some(pos) = self.opts.iter().position(|(n, _)| n == name) {
            return Some(self.opts.remove(pos).1);
        }
        None
    }

    /// Drain every occurrence of a repeatable option, in order (e.g. `--replace`).
    fn take_all(&mut self, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.opts.len() {
            if self.opts[i].0 == name {
                out.push(self.opts.remove(i).1);
            } else {
                i += 1;
            }
        }
        out
    }

    fn require(&mut self, name: &str) -> HResult<String> {
        if let Some(v) = self.take(name) {
            return Ok(v);
        }
        if self.unconsumed_flags.iter().any(|n| n == name) {
            return Err(format!("flag `{name}` is missing its value"));
        }
        Err(format!("missing required flag `{name}`"))
    }

    fn optional(&mut self, name: &str) -> HResult<Option<String>> {
        if self.unconsumed_flags.iter().any(|n| n == name) {
            return Err(format!("flag `{name}` is missing its value"));
        }
        Ok(self.take(name))
    }

    fn positionals(&mut self) -> Vec<String> {
        std::mem::take(&mut self.positionals)
    }

    /// Error if any flag or positional was left unconsumed — no silent ignores.
    fn finish(self) -> HResult<()> {
        let mut leftovers: Vec<String> = Vec::new();
        leftovers.extend(self.opts.into_iter().map(|(n, _)| n));
        leftovers.extend(self.unconsumed_flags);
        leftovers.extend(self.positionals);
        if leftovers.is_empty() {
            Ok(())
        } else {
            Err(format!("unexpected argument(s): {}", leftovers.join(", ")))
        }
    }
}
