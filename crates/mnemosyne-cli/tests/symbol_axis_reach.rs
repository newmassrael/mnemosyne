//! The symbol axis says whether it had an instrument.
//!
//! Round 1141 made one rule hold across every mode of `validate-code-refs`:
//! `0` means measured and clean, `null` means not measured, and there is no
//! third reading. It named four silences that had been publishing zeros — a
//! `--filter-id` run's other twelve axes, the decay axis without a filter, the
//! four opt-in axes, and a path-scoped run's spec-side five.
//!
//! THE SYMBOL AXIS WAS NOT AMONG THEM, and it is the one the consumer who
//! asked for all of this actually pays for. `scan` collects the symbol demand
//! whether or not a resolver exists and puts it to nobody when the map is
//! empty, so a run with no `[plugins.symbol_resolver.<lang>]` block emits no
//! symbol violation and the report published `symbol_mismatch_count: 0` — the
//! answer a judged-and-clean axis gives.
//!
//! THAT IS THE STATE THE CONSUMER'S OWN MEASUREMENT PROCEDURE CREATES. SCE
//! priced this axis by removing their resolver blocks and re-running the same
//! command: 108.9 seconds became 0.66, and the runs they measured reported the
//! axis clean. Removing the instrument is the one supported way to make this
//! axis cheap — `severity` has no `off` for it — and until this round it was
//! also the way to make it green.
//!
//! Two answers, deliberately different shapes. Laws 1 to 4 make the state
//! AUDIBLE: the verdict is a fact about the run's configuration, so it does not
//! flicker as the store is edited. Law 5 makes it REFUSABLE: a section that
//! records a symbol for a language nothing covers is a claim nobody checks, and
//! that is a fact about the store and the config together.
//!
//! Five laws, and the last two are where a later round would over-apply the
//! first two:
//!
//! 1. NO RESOLVER, NO COUNT. A run whose resolver map is empty does not
//!    publish a number for this axis; it names the axis and says why. The
//!    fixture makes the demand real — the census counts one citation this axis
//!    is supposed to judge — so the `null` is a statement about something,
//!    not decoration on an empty tree.
//!
//! 2. THE CONTROL IS THE SAME TREE WITH THE BLOCK PUT BACK. The same bytes,
//!    one config line different, and the drift is found, priced and rejected
//!    on. Without this the first law is satisfied by a gate that judges
//!    nothing ever, and — the reason it is here rather than in the validate
//!    crate, which already resolves symbols against a hand-built map — nothing
//!    put `[plugins.symbol_resolver.rust]` through the binary to a judgement:
//!    the one test that configured a working backend asserted only exit 0, so
//!    a wire handing `rust` the C++ resolver passed it.
//!
//! 3. AN UNREACHED LANGUAGE THAT CLAIMS NOTHING COSTS NOTHING. A tree holding
//!    a language no configured resolver covers does NOT lose its count for the
//!    languages that are covered, and is not refused for a claim it never
//!    made. The count is over the population the axis reached, and how far that
//!    reached is the census's answer, not this one's — the same division a
//!    path-scoped run makes between its counts and its `path_scope` block.
//!    Without this law, law 1 grows into "not fully covered = not judged" and
//!    law 5 grows into "an unreached file is a defect", and either one takes
//!    the axis away from a consumer with a second language in the tree.
//!
//! 4. THE REFUSAL CARRIES THE SAME ANSWER. `symbol_mismatch` is itemised in the
//!    binding-class rejection message, which is the one line a gate's operator
//!    cannot avoid reading, and Round 1141 shipped a defect of exactly this
//!    shape one axis over: the message was built from raw violation counts
//!    rather than from the verdict map and priced an unjudged axis at zero.
//!
//! 5. A CLAIM WITH NOTHING TO CHECK IT STOPS THE RUN. Round 855 settled this
//!    shape one step out — a resolver entry that cannot be BUILT is a config
//!    error rather than a warning, because `severity_binding = reject` reads as
//!    symbol-level enforcement while the run performs none. A symbol recorded
//!    for a language no entry covers is the same sentence with the halves
//!    swapped. The refusal is downstream of the report, so a gate parsing this
//!    command's JSON gets a diagnosis rather than a parse error.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// The one config line that separates law 1 from law 2.
const RUST_RESOLVER: &str = "[plugins.symbol_resolver.rust]\n\
                             transport = \"in-process\"\n\
                             backend = \"tree-sitter-rust\"\n";

/// A tree whose ONE Rust citation is a symbol-level defect and whose every
/// other axis is clean, so whatever the report says about `symbol_mismatch` is
/// the whole of the news.
///
/// `§sec1` records the symbol `beta` for `src/foo.rs`; the citation sits inside
/// `fn alpha`. A resolver that is asked answers `alpha`, which is not in the
/// recorded set. Nothing else in the tree is wrong: the section is bound to a
/// file that cites it, so neither the file-level axis nor the spec-side one has
/// anything to say.
///
/// `cpp_case` adds `src/bar.cpp`, bound at FILE level with no symbol recorded,
/// for a language no test here configures a resolver for: an unreachable file
/// that makes no symbol-level claim, which is the state law 3 separates from
/// the one law 5 refuses.
fn write_workspace(ws: &Path, resolver: &str, cpp_case: bool) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::create_dir_all(ws.join("src")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        format!(
            "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n\
             [plugins.set_equality_validator]\npaths = [\"src/\"]\ncomment_only = true\n\
             {resolver}"
        ),
    )
    .unwrap();

    // Line 2 is inside the body, so the enclosing symbol is `alpha`.
    fs::write(
        ws.join("src/foo.rs"),
        "fn alpha() {\n    // §sec1 — the one citation this axis judges\n    let _ = 1;\n}\n",
    )
    .unwrap();

    let mut sections = serde_json::json!({
        "sec1": {
            "title": "One",
            "parent_doc": "docs/GENERATED.md",
            "bindings": [
                {"file": "src/foo.rs", "symbol": "beta", "kind": "implements"}
            ]
        }
    });
    if cpp_case {
        fs::write(
            ws.join("src/bar.cpp"),
            "void gamma() {\n    // §sec2 — bound at file level, so nothing is asked of cpp\n}\n",
        )
        .unwrap();
        sections["sec2"] = serde_json::json!({
            "title": "Two",
            "parent_doc": "docs/GENERATED.md",
            "bindings": [
                {"file": "src/bar.cpp", "kind": "implements"}
            ]
        });
    }

    let atomic = serde_json::json!({
        "schema_version": 11,
        "sections": sections,
        "changelog_entries": {"Round 1": {"decision_summary": "the one entry this store holds"}}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(ws.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

/// The gate at its configured severities — `severity_binding` defaults to
/// reject, which is the state a consumer runs it in. The JSON is printed before
/// the refusal, so it is readable at either exit code.
fn validate(ws: &Path) -> (Output, serde_json::Value) {
    let out = Command::new(cli())
        .args(["validate-code-refs", "--json"])
        .current_dir(ws)
        .output()
        .expect("cli exec");
    let json = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout is not json: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out, json)
}

/// Axis name → machine reason, from the report's own `not_judged` block.
fn not_judged(json: &serde_json::Value) -> BTreeMap<String, String> {
    json["not_judged"]
        .as_array()
        .expect("not_judged array")
        .iter()
        .map(|e| {
            (
                e["axis"].as_str().expect("axis name").to_string(),
                e["reason"].as_str().expect("reason tag").to_string(),
            )
        })
        .collect()
}

/// LAW 1 — an axis with no instrument publishes no count.
#[test]
fn an_axis_with_no_resolver_is_named_rather_than_called_clean() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), "", false);
    let (out, json) = validate(tmp.path());

    // NON-VACUITY, from the run's own census: this tree holds exactly one
    // citation the symbol axis is supposed to judge. The `null` below is about
    // that citation, not about an empty tree.
    assert_eq!(
        (
            json["symbol_axis"]["checked_citations"].as_u64(),
            json["symbol_axis"]["checked_files"].as_u64()
        ),
        (Some(1), Some(1)),
        "the fixture must demand the axis for the absence to mean anything: {}",
        json["symbol_axis"]
    );

    assert_eq!(
        json["symbol_mismatch_count"],
        serde_json::Value::Null,
        "no resolver answered any of that demand, so there is no count to \
         publish — `0` is what a judged and clean axis says"
    );
    assert_eq!(
        not_judged(&json).get("symbol_mismatch").map(String::as_str),
        Some("no_resolver"),
        "the axis must be named with a reason a machine can branch on: {:?}",
        not_judged(&json)
    );

    // Nothing was emitted on this axis — which is why, until this round, the
    // count was the ONLY thing that could have said so, and it said `0`. What
    // stops this run is law 5, not a violation here.
    assert!(
        json["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .all(|v| v["kind"] != "symbol_mismatch"),
        "an axis nobody judged cannot emit: {json}"
    );
    // And the document survived the refusal: it is printed BEFORE the run
    // fails, so a consumer's gate reads a diagnosis rather than a parse error.
    assert!(
        !out.status.success(),
        "law 5 refuses a tree in this state; this assertion is here so that \
         reading the report and failing the run stay one behaviour: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// LAW 2 — the same tree with the block put back finds the drift, prices it,
/// and rejects on it.
#[test]
fn the_same_tree_with_a_resolver_judges_and_names_the_drift() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), RUST_RESOLVER, false);
    let (out, json) = validate(tmp.path());

    assert_eq!(
        json["symbol_mismatch_count"],
        serde_json::json!(1),
        "one config line different from law 1, and the axis is measured: {json}"
    );
    assert!(
        !not_judged(&json).contains_key("symbol_mismatch"),
        "an axis that judged must not appear in `not_judged`: {:?}",
        not_judged(&json)
    );

    let violations = json["violations"].as_array().expect("violations array");
    let symbol: Vec<&serde_json::Value> = violations
        .iter()
        .filter(|v| v["kind"] == "symbol_mismatch")
        .collect();
    assert_eq!(symbol.len(), 1, "violations: {violations:?}");
    assert_eq!(
        (
            symbol[0]["file"].as_str(),
            symbol[0]["line"].as_u64(),
            symbol[0]["entry_id"].as_str()
        ),
        (Some("src/foo.rs"), Some(2), Some("§sec1")),
        "the judgement names the citation, not just the file: {}",
        symbol[0]
    );

    // The whole path through the binary: config → plugin registry → the real
    // tree-sitter resolver → a judgement → the exit code a hook reads.
    assert!(
        !out.status.success(),
        "severity_binding defaults to reject, so a symbol drift must fail the run"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("symbol_mismatch=1"),
        "the refusal must price the axis it rejected on: {stderr}"
    );
}

/// LAW 4 — the refusal is a report too, and it does not borrow the zero.
///
/// `symbol_mismatch` is one of the three axes the binding-class refusal itemises,
/// so a run rejecting on a DIFFERENT binding-class axis prints this one's count
/// in the one line a consumer's gate cannot avoid reading. Round 1141 shipped
/// exactly this defect one axis over — the refusal was built from the raw
/// violation counts rather than from the verdict map, and printed
/// `binding_unbacked=0` for an axis that run never judged.
#[test]
fn the_refusal_names_the_axis_it_did_not_judge_rather_than_pricing_it_at_zero() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), "", false);
    // A citation-side defect on a different binding-class axis: `§sec1` binds
    // `src/foo.rs`, and this file is not it.
    fs::write(
        tmp.path().join("src/unbound.rs"),
        "fn epsilon() {\n    // §sec1 — cited from a file the section does not bind\n}\n",
    )
    .unwrap();

    let (out, json) = validate(tmp.path());
    assert!(
        !out.status.success(),
        "the fixture must reject, or there is no refusal to read: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        json["citation_unbound_count"],
        serde_json::json!(1),
        "the axis being rejected ON is measured: {json}"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("symbol_mismatch=not judged"),
        "the refusal must say the symbol axis was not judged, not price it at \
         zero beside a real count: {stderr}"
    );
    assert!(
        !stderr.contains("symbol_mismatch=0"),
        "the mirror of the assertion above — the borrowed zero must be gone \
         from the line, not merely joined by a truer one: {stderr}"
    );
}

/// LAW 5 — a symbol-level claim with nothing to check it stops the run.
///
/// Laws 1 to 4 make the state AUDIBLE. This one makes it refusable, and the two
/// are deliberately different shapes. The verdict is a fact about the run's
/// configuration, so it does not flicker with the store's contents; the refusal
/// is a fact about the store and the config TOGETHER, and incoherence between
/// them is exactly what a gate is for. Round 855 settled the principle one step
/// out: a resolver entry that cannot be built is a config error rather than a
/// warning, because `severity_binding = reject` reads as symbol-level
/// enforcement while the run performs none. A section that records a symbol for
/// a language nothing covers is the same sentence with the halves swapped.
///
/// It is also the state a consumer reaches by following their own cost
/// measurement: SCE priced this axis by deleting their resolver blocks, which
/// left every symbol binding in their store unchecked and every run green.
#[test]
fn a_symbol_claim_no_configured_resolver_covers_refuses_the_run() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), "", false);
    let (out, _) = validate(tmp.path());

    assert!(
        !out.status.success(),
        "the store records a symbol for src/foo.rs and nothing can check it — \
         that must stop the run, not pass it: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("1 citation(s)") && stderr.contains("plugins.symbol_resolver.rust"),
        "the refusal must say HOW MANY claims are unchecked and NAME the config \
         key that would check them: {stderr}"
    );

    // The control for the refusal is the same tree with the entry added: it is
    // about the missing resolver, not about the fixture.
    let with = TempDir::new().unwrap();
    write_workspace(with.path(), RUST_RESOLVER, false);
    let (ok, _) = validate(with.path());
    assert!(
        !String::from_utf8_lossy(&ok.stderr).contains("plugins.symbol_resolver.rust"),
        "a configured resolver must not be asked for again: {}",
        String::from_utf8_lossy(&ok.stderr)
    );

    // And a tree that records no symbol at all is NOT refused — the demand is
    // what makes the absence a defect. This is the state of the repository
    // hosting this gate, whose citations are all module-level.
    let none = TempDir::new().unwrap();
    write_workspace(none.path(), "", false);
    fs::write(
        none.path().join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 11,
            "sections": {"sec1": {"title": "One", "parent_doc": "docs/GENERATED.md",
                "bindings": [{"file": "src/foo.rs", "kind": "implements"}]}},
            "changelog_entries": {"Round 1": {"decision_summary": "the one entry"}}
        }))
        .unwrap(),
    )
    .unwrap();
    let (bare, json) = validate(none.path());
    assert_eq!(
        json["symbol_axis"]["checked_citations"].as_u64(),
        Some(0),
        "file-level binding only, so nothing demands a resolver: {json}"
    );
    assert!(
        bare.status.success(),
        "a tree that makes no symbol-level claim must not be told to configure \
         a resolver for it: {}",
        String::from_utf8_lossy(&bare.stderr)
    );
}

/// LAW 3 — an unreachable language that CLAIMS NOTHING costs nothing.
///
/// This is the boundary law 5 must not swallow. A file whose language no
/// resolver covers is reported by the census and always has been (Round 855's
/// advisory), and if no section records a symbol for it there is no claim to
/// leave unchecked: the run proceeds and the axis keeps its count for the
/// language that IS wired. Widen law 5 from "an unchecked claim" to "an
/// unreached file" and every adopter with a second language is refused for
/// prose they never wrote.
#[test]
fn an_unreached_language_that_claims_no_symbol_neither_refuses_nor_unjudges() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), RUST_RESOLVER, true);
    let (out, json) = validate(tmp.path());

    assert_eq!(
        json["symbol_axis"]["unchecked_citations"].as_u64(),
        Some(0),
        "the cpp file is bound at file level, so it claims nothing the missing \
         cpp resolver would have checked: {}",
        json["symbol_axis"]
    );
    assert_eq!(
        json["symbol_axis"]["checked_citations"].as_u64(),
        Some(1),
        "one claim, and it is the Rust one: {}",
        json["symbol_axis"]
    );
    assert_eq!(
        json["symbol_mismatch_count"],
        serde_json::json!(1),
        "the count is over what the axis reached: a number, not null: {json}"
    );
    assert!(
        !not_judged(&json).contains_key("symbol_mismatch"),
        "partial reach is not the same fact as no reach: {:?}",
        not_judged(&json)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("symbol_resolver.cpp"),
        "and no cpp resolver is demanded — the run fails on the Rust drift it \
         DID find, not on a language nobody made a claim in: {stderr}"
    );

    // How far it reached is the census's answer, in the same document.
    assert_eq!(
        json["symbol_axis"]["unresolved_languages"]["cpp"]["citing_files"].as_u64(),
        Some(1),
        "the language nothing covers is named with the count that matters — \
         files carrying a citation this ledger gates: {}",
        json["symbol_axis"]
    );
}
