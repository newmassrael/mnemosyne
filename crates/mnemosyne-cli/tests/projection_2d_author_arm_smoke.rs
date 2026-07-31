//! Round 879, corrected in Round 883 — the 2D-projection author arm rebuilds on
//! today's binary from the record plus ONE derived fixture.
//!
//! R878 measured the class: of 34 tracked experiment arms, 31 could not be
//! rebuilt against today's binary. Thirty died on R708 (the `value`/`scalar`
//! removal), which needs an authorial re-decision per fact. THIS arm died on
//! one purely mechanical thing — R752 widened a disclosure override's
//! `first_at` from a single coordinate to a coordinate SET, and the manifest
//! carried the superseded `[branch, coord]` pair.
//!
//! R879 re-encoded those three pairs IN THE RECORD, arguing the transcription
//! provably preserved content. R883 replayed every manifest at the revision that
//! authored it and the edited record FAILED at `14d7a1e0`, its own revision,
//! where the original bytes import cleanly. The edit had bought readability by
//! today's tool at the price of readability by its own. So the facts manifest
//! now comes from `tests/fixtures/projection-2d-author/` — a copy, labelled as
//! derived — and the record is back to its authored bytes. Everything else this
//! rebuild reads is the record itself, unchanged and uncopied.
//!
//! The gate half is unchanged and is still the point: R873 found the dnd store
//! rotted for ~150 rounds because NOTHING IN THE WORKSPACE LOADED IT. What this
//! asserts is what the experiment's own record already claims — the arm's two
//! world-lines, its three withheld secrets with their per-world first-reveal
//! coordinates and diegetic surfaces, and a continuity scan that comes back
//! clean. Its green is its own claim and does not stand in for the experiment's
//! pre-committed pin, which is re-checked by running `14d7a1e0` on the record.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/mnemosyne-cli
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

/// The tracked experiment record. Read, never written, never edited — R883's
/// correction of R879. Its facts manifest is at the shape `14d7a1e0` wrote and
/// today's binary cannot read it; the sections, order, and rules it holds are
/// read from here directly.
fn arm_dir() -> PathBuf {
    repo_root().join("claudedocs/phase1-2d-projection-experiment/v1/run/author")
}

/// The one DERIVED input: the record's facts manifest transcribed into today's
/// `first_at` shape. A copy, so the record can stay readable by its own
/// revision — see this directory's README for why R879's in-place edit was
/// wrong even though its content argument was right.
fn derived_facts() -> PathBuf {
    repo_root().join("crates/mnemosyne-cli/tests/fixtures/projection-2d-author/facts.json")
}

fn run(ws: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(ws)
        .output()
        .expect("cli exec")
}

fn run_ok(ws: &Path, args: &[&str]) -> String {
    let out = run(ws, args);
    assert!(
        out.status.success(),
        "{args:?} failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8(out.stdout).expect("cli output is utf-8")
}

fn json_report(ws: &Path, args: &[&str]) -> serde_json::Value {
    let mut full = args.to_vec();
    full.push("--json");
    serde_json::from_str(&run_ok(ws, &full)).expect("report json")
}

/// Rebuild the arm the way its runbook does: fresh seed, then the tracked
/// manifests. The order and rules declarations are copied verbatim from the
/// record — a second, editable home for them would be the drift this whole
/// line of work exists to remove.
fn rebuilt_workspace() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).expect("mkdir");
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"order.json\"\n\
         rules_path = \"narrative-rules.json\"\n",
    )
    .expect("write config");
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::json!({
            "schema_version": 23,
            "sections": {},
            "changelog_entries": {}
        })
        .to_string(),
    )
    .expect("seed store");
    for name in ["order.json", "narrative-rules.json"] {
        let src = arm_dir().join(name);
        fs::copy(&src, ws.join(name))
            .unwrap_or_else(|e| panic!("the tracked record must hold {}: {e}", src.display()));
    }
    let sections = arm_dir().join("sections.json");
    let facts = derived_facts();
    run_ok(
        ws,
        &[
            "import-sections",
            "--manifest",
            sections.to_str().expect("utf-8 path"),
        ],
    );
    run_ok(
        ws,
        &[
            "import-facts",
            "--manifest",
            facts.to_str().expect("utf-8 path"),
        ],
    );
    tmp
}

/// The arm imports, and the disclosure timing survives the trip. The
/// coordinate SET is the load-bearing part: R752 widened this field, the old
/// pair form bound POSITIVELY into the struct and died on the widened member,
/// and had that member not changed type the removed shape would have imported
/// in silence. So the assertion is on the coordinates themselves, not on the
/// import merely succeeding.
#[test]
fn the_author_arm_rebuilds_from_its_tracked_manifests() {
    let tmp = rebuilt_workspace();
    let ws = tmp.path();

    let world = json_report(ws, &["report-playable-world", "--telling", "play"]);
    let branches: Vec<&str> = world["fork_tree"]["branches"]
        .as_array()
        .expect("fork tree")
        .iter()
        .map(|b| b["branch_id"].as_str().expect("branch id"))
        .collect();
    assert_eq!(branches, ["alone", "together"], "{world}");

    // The three withheld secrets, each with the world-line and coordinate the
    // record authored. Read out of the store rather than the manifest, so this
    // fails if the re-encoding ever stops meaning what the pair meant.
    let store: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(ws.join("docs/.atomic/workspace.atomic.json")).expect("read store"),
    )
    .expect("store json");
    let overrides = &store["disclosure_plans"]["play"]["overrides"];
    let mut pinned: Vec<(String, String, Vec<String>, String)> = Vec::new();
    for (fact_id, ov) in overrides.as_object().expect("overrides") {
        let Some(first_at) = ov.get("first_at").and_then(|v| v.as_object()) else {
            continue;
        };
        assert_eq!(ov["mode"], "withhold", "a timed reveal is withheld: {ov}");
        for (branch, reveal) in first_at {
            let coords: Vec<String> = reveal["coords"]
                .as_array()
                .expect("the reveal carries a coordinate SET, not a bare coord")
                .iter()
                .map(|c| c.as_str().expect("coord").to_string())
                .collect();
            pinned.push((
                fact_id.clone(),
                branch.clone(),
                coords,
                ov["surface"]["scene"]
                    .as_str()
                    .expect("surface")
                    .to_string(),
            ));
        }
    }
    pinned.sort();
    let expected: Vec<(String, String, Vec<String>, String)> = [
        ("f-secret-exit", "alone", "al-14", "sc-12"),
        ("f-secret-geot", "together", "tg-14", "sc-08"),
        ("f-secret-yeon", "together", "tg-14", "sc-08"),
    ]
    .iter()
    .map(|(f, b, c, s)| {
        (
            (*f).to_string(),
            (*b).to_string(),
            vec![(*c).to_string()],
            (*s).to_string(),
        )
    })
    .collect();
    assert_eq!(pinned, expected, "{overrides}");
}

/// The rebuilt store is CLEAN under the arm's own declared rules — the record
/// is not merely parseable, it still says something true. A scan that found
/// violations would mean the round re-encoded the timing into a different
/// story than the one the experiment ran.
#[test]
fn the_rebuilt_arm_scans_clean_under_its_own_rules() {
    let tmp = rebuilt_workspace();
    let report = json_report(tmp.path(), &["validate-continuity"]);
    let violations = report["violations"].as_array().expect("violations array");
    assert!(
        violations.is_empty(),
        "the rebuilt arm must scan clean: {report}"
    );
    // Non-vacuity, and the point is that this half is NOT optional: an empty
    // store under no rules also reports zero violations, so "clean" alone is a
    // sentence about nothing. The scan must have had the arm's rule in hand
    // and the arm's whole store under it. (Written after measuring the report
    // — an earlier draft OR'd three `unwrap_or(0)` guards together, which is
    // the same shape this session criticised in a consumer's gate.)
    assert_eq!(
        report["rules"], 1,
        "the arm's rule was not in force: {report}"
    );
    assert_eq!(report["facts"], 59, "{report}");
    assert_eq!(report["sections"], 20, "{report}");
}
