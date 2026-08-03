//! Round 979 — a round's census reaches the frozen ledger as DATA, and no wire
//! into it accepts a number.
//!
//! The harm this closes is inheritance, measured end to end on this repo's own
//! ledger: Round 924 to Round 936 to Round 961 to Round 968, and Round 914 to
//! Round 915 in the same shape — a round needs a baseline, finds no program to
//! run, and takes the number out of an earlier round's sentence. Round 976
//! measured that no word list can tell a census sentence from a rule at append
//! time and refused to gate the prose; Round 977 built the recount and the
//! tracked report. This is the half that makes what a later round finds be data.
//!
//! WHAT THESE TESTS ARE FOR, and why they run the real binary: a field whose
//! value a caller can type is a hand-written count in a new place, which is the
//! defect wearing the fix. So the load-bearing property is not "the field
//! exists" but "the number came from the report and could not have come from
//! anywhere else", and that is only observable at the wire.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn run(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

fn read_store(ws: &Path) -> serde_json::Value {
    serde_json::from_str(
        &fs::read_to_string(ws.join("docs/.atomic/workspace.atomic.json")).unwrap(),
    )
    .unwrap()
}

/// The report a workspace's own recount would have written. Built through
/// `mnemosyne_ops::render_population_census`, never hand-typed here, so this
/// fixture cannot pass while disagreeing with the bytes the real gate blesses.
fn census_report() -> String {
    mnemosyne_ops::render_population_census(&[
        mnemosyne_atomic::PopulationCensus {
            axis: "transition rules by `undirected`".to_string(),
            left_label: "undirected".to_string(),
            left: 10,
            right_label: "directed".to_string(),
            right: 27,
        },
        mnemosyne_atomic::PopulationCensus {
            axis: "run trees by authoring mode".to_string(),
            left_label: "hand script".to_string(),
            left: 10,
            right_label: "file-only".to_string(),
            right: 239,
        },
    ])
    .expect("render the census report")
}

/// A workspace that declares a census report, with the report on disk.
fn write_workspace(ws: &Path, declare_census: bool) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::create_dir_all(ws.join("reports")).unwrap();
    let mut toml = String::from("[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n");
    if declare_census {
        toml.push_str("[census]\nreport = \"reports/population-census.json\"\n");
    }
    fs::write(ws.join("mnemosyne.toml"), toml).unwrap();
    fs::write(ws.join("reports/population-census.json"), census_report()).unwrap();
    let atomic = serde_json::json!({
        "schema_version": mnemosyne_atomic::CURRENT_SCHEMA_VERSION,
        "sections": {},
        "changelog_entries": {},
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

fn write_prose(ws: &Path) {
    fs::write(ws.join("decision.txt"), "a round that states a census\n").unwrap();
    fs::write(ws.join("changes.txt"), "- changed a thing\n").unwrap();
    fs::write(ws.join("verify.txt"), "- checked the thing\n").unwrap();
}

fn append(ws: &Path, entry: &str, extra: &[&str]) -> std::process::Output {
    let mut args = vec![
        "append-changelog-entry",
        "--entry-id",
        entry,
        "--decision-file",
        "decision.txt",
        "--changes-file",
        "changes.txt",
        "--verification-file",
        "verify.txt",
    ];
    args.extend_from_slice(extra);
    run(ws, &args)
}

/// THE NUMBERS IN THE LEDGER ARE THE REPORT'S, VERBATIM.
///
/// Asserted against the report the fixture wrote rather than against literals
/// spelled in this test: a hand-typed expectation here would pass whenever the
/// wire and the test made the same mistake, which is the whole failure mode.
#[test]
fn record_census_files_exactly_what_the_workspace_report_says() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, true);
    write_prose(ws);

    let out = append(ws, "Round 979", &["--record-census"]);
    assert!(
        out.status.success(),
        "append failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let report: serde_json::Value = serde_json::from_str(&census_report()).unwrap();
    let expected = report.get("axes").unwrap();
    let store = read_store(ws);
    let recorded = store
        .pointer("/changelog_entries/Round 979/population_census")
        .expect("the entry records no census at all");
    assert_eq!(
        recorded, expected,
        "the ledger's census is not the report's — a round reading this entry \
         would inherit a population this tree never counted"
    );
}

/// The field is opt-in, and an entry that did not ask for it carries no key —
/// so "this entry states no census" stays readable as itself rather than as a
/// census of nothing.
#[test]
fn an_append_that_does_not_ask_records_no_census() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, true);
    write_prose(ws);

    assert!(append(ws, "Round 979", &[]).status.success());
    let store = read_store(ws);
    assert!(
        store
            .pointer("/changelog_entries/Round 979/population_census")
            .is_none(),
        "an append that never asked for a census still filed one"
    );
}

/// A WORKSPACE WITH NO REPORT FAILS LOUD AND WRITES NOTHING.
///
/// The silent alternative is worse than the error: an absent
/// `population_census` reads as "this round made no census claim", so a
/// `--record-census` that quietly recorded nothing would leave the ledger
/// stating something false about the round, permanently.
#[test]
fn record_census_without_a_declared_report_refuses_to_append() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, false);
    write_prose(ws);

    let out = append(ws, "Round 979", &["--record-census"]);
    assert!(
        !out.status.success(),
        "an append asked to record a census the workspace does not keep succeeded"
    );
    let said = String::from_utf8_lossy(&out.stderr) + String::from_utf8_lossy(&out.stdout);
    assert!(
        said.contains("[census]"),
        "the refusal does not name the missing declaration: {said}"
    );
    assert!(
        read_store(ws)
            .pointer("/changelog_entries/Round 979")
            .is_none(),
        "the entry landed anyway, so the ledger holds a round that failed to \
         record what it said it would"
    );
}

/// A REPORT THAT IS NOT A REPORT IS A REFUSAL, NOT AN EMPTY CENSUS.
#[test]
fn record_census_refuses_an_unreadable_report() {
    for (what, bytes) in [
        ("not json", "the axes are fine, trust me\n".to_string()),
        (
            "no axes",
            mnemosyne_ops::render_population_census(&[]).unwrap(),
        ),
    ] {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();
        write_workspace(ws, true);
        write_prose(ws);
        fs::write(ws.join("reports/population-census.json"), &bytes).unwrap();

        let out = append(ws, "Round 979", &["--record-census"]);
        assert!(
            !out.status.success(),
            "`{what}` was accepted as this workspace's recorded population"
        );
        assert!(
            read_store(ws)
                .pointer("/changelog_entries/Round 979")
                .is_none(),
            "`{what}`: the entry landed anyway"
        );
    }
}

fn git(ws: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(ws)
        .output()
        .expect("git must be runnable to test the contemporaneity gate");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn commit_all(ws: &Path, message: &str) {
    git(ws, &["add", "-A"]);
    git(
        ws,
        &[
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "user.name=t",
            "commit",
            "--quiet",
            "--no-verify",
            "-m",
            message,
        ],
    );
}

/// Move one count in the report, leaving it a well-formed report that simply
/// says something else — the shape a re-bless after a corpus change produces.
fn move_a_count(ws: &Path) {
    let mut axes: Vec<mnemosyne_atomic::PopulationCensus> = serde_json::from_value(
        serde_json::from_str::<serde_json::Value>(&census_report())
            .unwrap()
            .get("axes")
            .unwrap()
            .clone(),
    )
    .unwrap();
    axes[0].right += 1;
    fs::write(
        ws.join("reports/population-census.json"),
        mnemosyne_ops::render_population_census(&axes).unwrap(),
    )
    .unwrap();
}

/// AN ENTRY THIS COMMIT IS ADDING MAY NOT FREEZE A COUNT THE TREE NO LONGER
/// STATES — AND A COMMITTED ONE IS FREE TO.
///
/// Both halves in one test, because either alone is the wrong gate: a check that
/// only fired would reject every honest older entry the day its axis moved, and
/// a check that only spared would be no check at all. The order of the three
/// phases is the order the defect happens in.
#[test]
fn a_census_being_added_must_match_the_tree_and_a_committed_one_need_not() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, true);
    write_prose(ws);
    git(ws, &["init", "--quiet"]);
    commit_all(ws, "baseline");

    // Phase 1 — appended against the report as it stands: clean.
    assert!(append(ws, "Round 979", &["--record-census"])
        .status
        .success());
    let out = run(ws, &["validate-workspace"]);
    assert!(
        out.status.success(),
        "an entry appended against the current report was rejected: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Phase 2 — the population moves after the entry was written. The entry is
    // still uncommitted, so this is the one moment the disagreement is decidable.
    move_a_count(ws);
    let out = run(ws, &["validate-workspace"]);
    let said =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "an uncommitted entry froze a count the tree no longer states and \
         validate-workspace passed: {said}"
    );
    assert!(
        said.contains("Round 979") && said.contains("transition rules"),
        "the rejection names neither the entry nor the axis: {said}"
    );

    // Phase 3 — once committed, that same entry is history, and history is
    // supposed to disagree with a population that has moved on since.
    commit_all(ws, "the round");
    move_a_count(ws);
    let out = run(ws, &["validate-workspace"]);
    assert!(
        out.status.success(),
        "a COMMITTED entry was rejected for disagreeing with a population that \
         moved after it: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// NEITHER WIRE HAS A DOOR THROUGH WHICH A COUNT COULD ARRIVE.
///
/// The primitive's invariants are shared by construction — one function, called
/// from one place — so what is left to check is the shape above it: a wire that
/// took the counts from its caller would satisfy every invariant in the store
/// and still put a hand-typed population into the frozen ledger. That is the
/// `CLAUDE.md` anti-pattern in its live form (two write paths into one field),
/// and the defence is that both wires take a BOOLEAN and read the numbers from
/// the one resolver.
///
/// Read from the sources of BOTH wires, with the site count asserted, because a
/// check that silently found nothing would report "parity holds" for a tree in
/// which nobody writes the field at all.
#[test]
fn both_wires_take_their_census_from_the_one_resolver() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf();
    let wires = [
        "crates/mnemosyne-cli/src/atomic_cli.rs",
        "crates/mnemosyne-mcp/src/main.rs",
    ];
    let mut sites = 0usize;
    for wire in wires {
        let text = fs::read_to_string(root.join(wire)).unwrap_or_else(|e| panic!("{wire}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        let fed: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.trim_start().starts_with("population_census:"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            fed.len(),
            1,
            "{wire} builds {} changelog drafts carrying a census; this check \
             reads one per wire and would miss the others",
            fed.len()
        );
        // The value handed to the draft is a local; the assertion is that the
        // local's ONLY source in this wire is the shared resolver.
        assert_eq!(
            text.matches("workspace_population_census").count(),
            1,
            "{wire} names the shared census resolver {} times — either it has \
             stopped using it, or it has grown a second way to fill the field",
            text.matches("workspace_population_census").count()
        );
        assert!(
            !text.contains("PopulationCensus {"),
            "{wire} constructs a `PopulationCensus` of its own, so a count can \
             now enter the ledger without passing through the workspace's report"
        );
        sites += fed.len();
    }
    assert_eq!(sites, 2, "expected exactly the two wires to fill the field");
}
