//! Round 957 — the RENDERED authoring contract states the side-table
//! consequence once, in the paragraph that owns it.
//!
//! Round 956 wired `edge_costs` and `edge_guards` into the fact manifest.
//! Round 957 found that the contract every blind author reads went on teaching
//! the retired model in TWO places: the paragraph, and the section HEADING over
//! it (`-- side tables (verb-only; a manifest array for these is a silent
//! no-op) --`). The paragraph's claim is now derived from the manifest roster,
//! so the library test covers it — but the heading is a `println!` in the CLI
//! and the library cannot see it. Measured, before this file existed: reverting
//! the heading to its false form left the whole workspace suite GREEN.
//!
//! So the gate stands where the copy lands. The invariant is single-home, not a
//! banned word: the consequence is stated exactly ONCE in the whole rendered
//! document, and it is stated inside the side-table paragraph. A second home
//! for one datum is how the heading stayed wrong for a round.
//!
//! Note this deliberately does NOT ban the phrase "verb-only" from the
//! contract: the `edge_guards` manifest row says "same verb-only history as
//! `edge_costs`", which is a true statement about the past and is exactly the
//! sentence that tells an author why the wire is worth trusting.

use std::process::Command;

fn describe_schema() -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
        .arg("describe-schema")
        .output()
        .expect("run mnemosyne-cli describe-schema");
    assert!(
        out.status.success(),
        "describe-schema must exit 0; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The silent-no-op consequence has exactly one home in the rendered contract.
#[test]
fn the_no_op_consequence_is_stated_once_in_the_whole_contract() {
    let contract = describe_schema().to_lowercase();
    let hits = contract.matches("silent no-op").count();
    assert_eq!(
        hits, 1,
        "the consequence is stated {hits} time(s) in the rendered contract. It owns exactly one \
         home — the side-table paragraph, where the claim is DERIVED from the manifest roster. \
         A second statement cannot be derived, so it drifts: the section heading asserted \
         `verb-only` for a full round after Round 956 made it false, and pointed every author \
         who came looking for `edge_costs` back at the verbs."
    );
}

/// The one statement sits WITH the derived list that scopes it.
///
/// A correct count is not enough on its own: the consequence stated apart from
/// the list of tables it applies to is how a reader concludes it applies to all
/// of them, which is the reading that was true until Round 956 and false after.
/// The anchor here is the DERIVED content, not the section heading — an earlier
/// draft of this test anchored on the heading text and, under the injection it
/// was written for, went red because the anchor had moved rather than because
/// the claim had. A red test is not evidence the intended arm fired.
#[test]
fn the_one_statement_sits_with_the_list_that_scopes_it() {
    let contract = describe_schema();
    let line = contract
        .lines()
        .find(|l| l.to_lowercase().contains("silent no-op"))
        .expect("the consequence must be stated somewhere in the contract");
    // The paragraph is printed as one line, so co-location is line identity.
    // These are the tables still outside the fact manifest; the derivation in
    // `mnemosyne-validate` is what decides membership, and this asserts the
    // rendered claim carries that list rather than standing bare.
    for table in [
        "parameters",
        "parameter_deltas",
        "parameter_gates",
        "fact_counts",
    ] {
        assert!(
            line.contains(&format!("`{table}`")),
            "the consequence is stated without naming `{table}`, one of the tables it applies \
             to. Stated bare, it reads as applying to every keyed side table — which is the \
             claim that put `edge_costs` and `edge_guards` out of a file-only authoring's \
             reach, and those two are manifest arrays now (R956)."
        );
    }
    // Which tables belong on that list is NOT decided here — the derivation and
    // its discriminating test live in `mnemosyne-validate`. Asserting membership
    // again from a hand list in a second crate would be the same two-homes-for-
    // one-datum mistake this round is paying off.
}

/// Non-vacuity for both checks above: the tables Round 956 wired must be
/// reachable from a file according to this same document. If the manifest rows
/// vanished, the checks above would still pass over a contract that teaches
/// nothing about them.
#[test]
fn the_wired_side_tables_are_described_as_manifest_arrays() {
    let contract = describe_schema();
    for table in ["edge_costs", "edge_guards"] {
        assert!(
            contract.contains(&format!("  {table}: {{")),
            "`{table}` has no fact-manifest row in the rendered contract, so a file-only author \
             cannot reach it — the exact state Round 956 ended and five corpora sat in (R936)"
        );
    }
}
