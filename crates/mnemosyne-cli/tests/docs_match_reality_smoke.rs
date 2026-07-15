//! Tracked docs must describe the software that exists — mechanically.
//!
//! Every defect R620-R622 fixed in the docs was found by a *person reading*:
//! a `[workspace] docs` key removed at Round 400 still shipped in five
//! copy-paste configs ~220 rounds later; both READMEs' quickstart TOML failed
//! to parse; a CI example invoked a verb deleted with GENERATED.md; a
//! `_vN`-banned identifier lived on inside two docs. None of it was caught by
//! anything, because nothing looks. A consumer then read those docs, concluded
//! the narrative half did not exist, and rebuilt it outside the store in
//! ~1,100 lines of Python.
//!
//! R622 made the CLI's own help underivable-from-drift by collapsing dispatch
//! and help onto one table. This gate is that same principle pointed at the
//! docs: a documented verb must dispatch, and a documented config must parse
//! through the real `parse_config`. Both questions are decidable, so they are
//! gated rather than reviewed.
//!
//! Scope: TRACKED markdown that is *instructional* — what a reader is told to
//! run. Deliberately excluded, because a stale verb there is an accurate record
//! rather than drift: the frozen ledger (`docs/.atomic/`), and `claudedocs/`
//! experiment runbooks/findings, which are sha-pinned records of what was run
//! at the time and must not be retro-edited (frozen-ledger ethos, CLAUDE.md).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/mnemosyne-cli
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

/// Tracked instructional `.md`, per git — the gate's scope is exactly what a
/// reader is told to run. See the module doc for what is excluded and why.
fn tracked_markdown(root: &Path) -> Vec<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "*.md"])
        .output()
        .expect("git ls-files");
    assert!(out.status.success(), "git ls-files failed");
    String::from_utf8(out.stdout)
        .expect("git output is utf-8")
        .lines()
        .filter(|l| !l.starts_with("claudedocs/") && !l.starts_with("docs/.atomic/"))
        .map(|l| root.join(l))
        .collect()
}

/// The verb a `mnemosyne-cli …` invocation names, if any.
///
/// `text` is one INSTRUCTION span — a shell fence line, or the inside of an
/// inline-code span. Prose is never passed here: "routes through
/// `mnemosyne-cli` mutate API" must not read `mutate` as a verb, and a CI step
/// titled "Install mnemosyne-cli (pinned revision)" must not read `pinned`.
///
/// Keys on `mnemosyne-cli` as a WHOLE token, which separates an invocation
/// (`mnemosyne-cli add-fact`) from a path argument (`cargo install --path
/// crates/mnemosyne-cli --force` — whose `--force` is cargo's). Handles the
/// `cargo run -p mnemosyne-cli -- <verb>` form. Flags are not verbs, except the
/// two meta verbs spelled as flags.
fn invoked_verb(text: &str) -> Option<&str> {
    let toks: Vec<&str> = text.split_whitespace().collect();
    let i = toks.iter().position(|t| *t == "mnemosyne-cli")?;
    let mut next = *toks.get(i + 1)?;
    if next == "--" {
        next = *toks.get(i + 2)?;
    }
    if next.starts_with('-') && !matches!(next, "--help" | "-h" | "--version" | "-V") {
        return None; // an outer command's flag (cargo test -p mnemosyne-cli --test x)
    }
    let verb = next.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
    (!verb.is_empty()).then_some(verb)
}

/// Split a document into (prose, shell-fence lines).
///
/// `prose` keeps every non-fenced line and BLANKS every fenced one, so line
/// numbers survive and fence backticks can never pair with a prose backtick.
/// `shell` is the copyable lines from ```bash-style fences, with line numbers.
fn split_prose_and_shell(text: &str) -> (String, Vec<(usize, &str)>) {
    let mut prose = String::new();
    let mut shell = Vec::new();
    let mut fence_lang: Option<String> = None;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") {
            fence_lang = match fence_lang {
                None => Some(t.trim_start_matches('`').trim().to_string()),
                Some(_) => None,
            };
            prose.push('\n');
            continue;
        }
        match &fence_lang {
            Some(lang) => {
                if matches!(lang.as_str(), "bash" | "sh" | "shell" | "console") {
                    shell.push((i + 1, line));
                }
                prose.push('\n');
            }
            None => {
                prose.push_str(line);
                prose.push('\n');
            }
        }
    }
    (prose, shell)
}

/// Every inline-code span in PROSE, with the 1-based line its opening backtick
/// sits on.
///
/// Scans the whole prose body, not a line: an inline span may wrap across a
/// newline (`` `mnemosyne-cli\ncommit` `` is one span in rendered markdown),
/// and a per-line scan silently misses those — under-detection, the exact
/// failure class this gate exists to condemn.
fn code_spans(prose: &str) -> Vec<(usize, &str)> {
    let mut spans = Vec::new();
    let mut idx = 0usize;
    while let Some(open_rel) = prose[idx..].find('`') {
        let open = idx + open_rel;
        let Some(close_rel) = prose[open + 1..].find('`') else {
            break;
        };
        let close = open + 1 + close_rel;
        let line = prose[..open].matches('\n').count() + 1;
        spans.push((line, &prose[open + 1..close]));
        idx = close + 1;
    }
    spans
}

/// The verbs the CLI actually dispatches, read from `--help` — which R622 made
/// a projection of the `COMMANDS` table, so this set cannot drift from dispatch.
fn dispatched_verbs() -> BTreeSet<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
        .arg("--help")
        .output()
        .expect("run mnemosyne-cli --help");
    assert!(out.status.success(), "--help must exit 0");
    let help = String::from_utf8(out.stdout).expect("--help stdout is utf-8");
    let mut verbs = BTreeSet::new();
    for line in help.lines() {
        // Usage lines render as ` {prog} {verb} ...`; the prog path varies by
        // invocation, so key on the token AFTER the one ending in the bin name.
        let mut toks = line.split_whitespace();
        if let Some(first) = toks.next() {
            if first.ends_with("mnemosyne-cli") {
                if let Some(verb) = toks.next() {
                    if !verb.starts_with('-') || verb == "--help" || verb == "--version" {
                        verbs.insert(verb.to_string());
                    }
                }
            }
        }
    }
    verbs
}

/// Every `mnemosyne-cli <verb>` / `cargo run -p mnemosyne-cli -- <verb>`
/// mention in a tracked doc must name a verb that dispatches.
#[test]
fn documented_verbs_all_dispatch() {
    let root = repo_root();
    let verbs = dispatched_verbs();
    assert!(
        verbs.len() > 60,
        "parser regression: only {} verbs read out of --help; the gate would \
         pass vacuously. Got: {:?}",
        verbs.len(),
        verbs
    );

    let mut bad: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for file in tracked_markdown(&root) {
        let rel = file.strip_prefix(&root).unwrap_or(&file).to_path_buf();
        let text = std::fs::read_to_string(&file).expect("read tracked md");
        // A reader copies two things: shell-fence lines, and inline-code spans.
        // Everything else on the page is prose ABOUT the tool.
        let (prose, shell) = split_prose_and_shell(&text);
        let mut spans: Vec<(usize, &str)> = code_spans(&prose);
        spans.extend(shell);
        for (line, span) in spans {
            let Some(verb) = invoked_verb(span) else {
                continue;
            };
            checked += 1;
            if !verbs.contains(verb) {
                bad.push(format!("{}:{} names `{}`", rel.display(), line, verb));
            }
        }
    }
    assert!(
        checked > 20,
        "parser regression: only {} verb mentions found across tracked docs",
        checked
    );
    assert!(
        bad.is_empty(),
        "tracked docs name {} verb(s) the CLI does not dispatch — a reader who \
         types these gets `unknown command`:\n  {}",
        bad.len(),
        bad.join("\n  ")
    );
}

/// Every fenced ```toml block that looks like a `mnemosyne.toml` must parse
/// through the REAL loader. `[workspace]` is `deny_unknown_fields`, so a stale
/// key here is a copy-paste config that fails at the adopter's step one.
#[test]
fn documented_configs_all_parse() {
    let root = repo_root();
    let mut bad: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for file in tracked_markdown(&root) {
        let rel = file.strip_prefix(&root).unwrap_or(&file).to_path_buf();
        let text = std::fs::read_to_string(&file).expect("read tracked md");
        let mut lines = text.lines().enumerate();
        while let Some((start, line)) = lines.next() {
            if line.trim() != "```toml" {
                continue;
            }
            let mut body = String::new();
            for (_, l) in lines.by_ref() {
                if l.trim() == "```" {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            // A whole mnemosyne.toml always carries `[workspace]` (the loader
            // requires it). A block without one is a FRAGMENT showing a single
            // override table — real documentation, but not a config a reader
            // pastes whole, so parsing it would be a false positive.
            if !body.contains("[workspace]") {
                continue;
            }
            // Elided blocks (`docs = [...]`, `# ...`) are illustrative
            // fragments too; a literal `...` marks them.
            if body.contains("...") {
                continue;
            }
            checked += 1;
            if let Err(e) = mnemosyne_config::parse_config(&body) {
                bad.push(format!(
                    "{}:{} does not parse — an adopter pasting this gets: {:#}",
                    rel.display(),
                    start + 1,
                    e
                ));
            }
        }
    }

    assert!(
        checked >= 5,
        "parser regression: only {} mnemosyne.toml blocks found across tracked \
         docs; the gate would pass vacuously",
        checked
    );
    assert!(
        bad.is_empty(),
        "{} documented config(s) do not parse through parse_config:\n  {}",
        bad.len(),
        bad.join("\n  ")
    );
}
