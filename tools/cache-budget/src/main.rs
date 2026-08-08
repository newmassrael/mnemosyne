//! Ask both sides, print what was reached, and only then judge any of it.

use std::path::{Path, PathBuf};
use std::process::Command;

use cache_budget::{conclude, Held, Refusal, DEFAULT_LIMIT_BYTES};

fn main() {
    let root: PathBuf = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);

    // DECLARED — through `ci-plan`, this repository's one reader of what its CI
    // says, so this gate cannot drift from the gates asking the same files what
    // CI RUNS.
    let declared = ci_plan::workflow_cache_declarations(&root);

    // HELD — from the only thing that knows what a cache actually costs.
    let held = match held_caches(&root) {
        Ok(held) => held,
        Err(why) => {
            // A REFUSAL, NOT A PASS, and exit 2 rather than exit 1: "I could not
            // look" and "these break the law" are different answers, and one
            // message for both mislabels whichever it did not mean.
            eprintln!("cache-budget: {}", Refusal::Unreached(why));
            std::process::exit(2);
        }
    };

    let report = conclude(DEFAULT_LIMIT_BYTES, &declared, &held);

    // WHAT WAS REACHED, first and unconditionally. A gate that never opened
    // anything and a gate that found nothing wrong print the same silence.
    println!(
        "{} cache step(s) across this repository's workflows under {} key(s), {} \
         held by GitHub, budget {:.2} GB",
        declared.len(),
        report.rows.len(),
        held.len(),
        DEFAULT_LIMIT_BYTES as f64 / 1e9
    );
    for row in &report.rows {
        let size = match (&row.held, &row.estimate) {
            (Some(held), _) => format!("{:>8.2} GB held", held.size_in_bytes as f64 / 1e9),
            (None, Some(estimate)) => format!(
                "{:>8.2} GB ABSENT, priced from {}",
                estimate.bytes as f64 / 1e9,
                estimate.from
            ),
            (None, None) => "       ? ABSENT, never observed".to_string(),
        };
        println!(
            "  {size}  {}  [{}]  {}",
            row.prefix,
            row.paths.iter().cloned().collect::<Vec<_>>().join(" "),
            row.owners.join(", ")
        );
        // PRINTED THOUGH NOT COUNTED. These are real bytes GitHub is holding, and
        // a gate that judged one generation while silently dropping the others
        // from its output would be reporting a smaller world than it looked at.
        for old in &row.superseded {
            println!(
                "  {:>8.2} GB held under the same key, superseded on {} and aging \
                 out — not counted, because no workflow can stop a lockfile bump \
                 leaving one behind",
                old.size_in_bytes as f64 / 1e9,
                row.held
                    .as_ref()
                    .map_or("—", |newest| newest.created_at.as_str())
            );
        }
    }
    for orphan in &report.orphans {
        println!(
            "  {:>8.2} GB held, declared by nothing: {}",
            orphan.size_in_bytes as f64 / 1e9,
            orphan.key
        );
    }
    match report.demand() {
        Some(demand) => println!("demand {:.2} GB", demand as f64 / 1e9),
        None => println!("demand UNKNOWN — nothing comparable has been observed"),
    }

    // THEN judge it, and print every refusal. Stopping at the first reports one
    // line of a distribution that is itself the finding.
    let refusals = report.refusals();
    if refusals.is_empty() {
        println!("every cache this repository declares is one it keeps");
        return;
    }
    for refusal in &refusals {
        eprintln!("cache-budget: {refusal}");
    }
    let unreached = refusals
        .iter()
        .any(|refusal| matches!(refusal, Refusal::Unreached(_)));
    std::process::exit(if unreached { 2 } else { 1 });
}

/// Every cache this repository holds, from the GitHub API.
///
/// Through `gh` rather than a hand-rolled HTTP call: it is on every hosted runner
/// and on this project's machine, and it already knows how to authenticate in
/// both — one less credential path to get wrong. `{owner}` and `{repo}` are
/// `gh`'s own placeholders, resolved from the checkout, so this gate never names
/// the repository it is judging.
fn held_caches(root: &Path) -> Result<Vec<Held>, String> {
    let out = Command::new("gh")
        .args([
            "api",
            "--paginate",
            "repos/{owner}/{repo}/actions/caches",
            "--jq",
            r#".actions_caches[] | "\(.size_in_bytes)\t\(.created_at)\t\(.key)""#,
        ])
        .current_dir(root)
        .output()
        .map_err(|e| format!("`gh api` could not be run at all: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`gh api repos/{{owner}}/{{repo}}/actions/caches` failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut caches = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let unreadable = || format!("`gh` printed a row this cannot read: {line:?}");
        let (size, rest) = line.split_once('\t').ok_or_else(unreadable)?;
        let (created_at, key) = rest.split_once('\t').ok_or_else(unreadable)?;
        caches.push(Held {
            key: key.to_string(),
            size_in_bytes: size
                .trim()
                .parse()
                .map_err(|e| format!("{size:?} is not a size: {e}"))?,
            created_at: created_at.trim().to_string(),
        });
    }
    Ok(caches)
}
