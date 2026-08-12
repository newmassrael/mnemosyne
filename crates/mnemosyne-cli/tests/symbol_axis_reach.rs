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
//!    fixture makes the demand real — the census counts the two citations this
//!    axis is supposed to judge — so the `null` is a statement about something,
//!    not decoration on an empty tree.
//!
//! 2. THE CONTROL IS THE SAME TREE WITH THE BLOCK PUT BACK. The same bytes,
//!    one config line different, and the drift is found, priced and rejected
//!    on. Without this the first law is satisfied by a gate that judges
//!    nothing ever, and — the reason it is here rather than in the validate
//!    crate, which already resolves symbols against a hand-built map — nothing
//!    put `[plugins.symbol_resolver.rust]` through the binary to a judgement:
//!    the one test that configured a working backend asserted only exit 0, so
//!    a wire handing `rust` the C++ resolver passed it. Round 1146 had to
//!    rebuild the fixture before this law could say that, and the injection
//!    now reddens it: see `write_workspace`.
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

/// A tree in ONE language that can tell WHICH resolver answered, which the
/// first version of it could not.
///
/// TWO SITES, AND BOTH ARE LOAD-BEARING. `src/drift.<ext>` cites `§sec1`, which
/// records `beta`, from inside a method this language's grammar calls something
/// else — a mismatch. `src/matched.<ext>` cites `§sec2`, which records
/// [`LangFixture::matched_symbol`], from inside a method the grammar calls
/// exactly that — clean. The whole run must therefore report EXACTLY ONE symbol
/// violation, at the first file.
///
/// WHY ONE SITE WAS NOT ENOUGH. Round 1145 injected the C++ resolver into the
/// `rust` key and NOTHING went red across 1870 tests. With only a mismatch site,
/// every possible answer is indistinguishable: `alpha`, `Holder`, or nothing at
/// all each leave the recorded `beta` unmatched, so the count is 1 either way
/// and the assertion holds while the wrong plugin runs. A resolver that answers
/// NOTHING now drops the count to 0, and one that answers DIFFERENT NAMES raises
/// it to 2 by reddening the clean site. Both are one number away from correct
/// and both fail. The violation carries no found symbol — file, line and
/// `§id` only — so the two sites are the only thing that can pin the vocabulary.
///
/// Each fixture is written so that NO OTHER shipped grammar parses it into the
/// same answer: `fn alpha() { … }` at top level is also a valid C++ function
/// definition — return type `fn`, name `alpha` — so the Rust sites sit inside
/// `impl` blocks. That is how a fixture stops being a control (Round 1096: the
/// spelling is what makes it one).
struct LangFixture {
    /// The symbol-axis language id — the `[plugins.symbol_resolver.<lang>]` key
    /// this fixture configures.
    language: &'static str,
    /// An extension the symbol-axis extension table maps to `language`.
    ext: &'static str,
    /// Source whose enclosing symbol at the citation line is NOT `beta`.
    drift: &'static str,
    /// Source whose enclosing symbol at the citation line IS `matched_symbol`.
    matched: &'static str,
    /// The name this language's grammar must answer at the clean site, spelled
    /// in that language's own vocabulary.
    matched_symbol: &'static str,
}

/// The Rust fixture — the one laws 1 to 5 run on.
const RUST: LangFixture = LangFixture {
    language: "rust",
    ext: "rs",
    drift: "struct Holder;\n\nimpl Holder {\n    fn alpha(&self) {\n        \
            // §sec1 — recorded as `beta`, so this citation has drifted\n        \
            let _ = 1;\n    }\n}\n",
    matched: "struct Keeper;\n\nimpl Keeper {\n    fn gamma(&self) {\n        \
              // §sec2 — recorded as `gamma`, so this citation is clean\n        \
              let _ = 2;\n    }\n}\n",
    matched_symbol: "gamma",
};

/// The C++ fixture. The citation sits on the LAST line of each method body, so
/// the doc-comment rule (a comment immediately above a declaration binds to
/// that declaration) does not fire and the smallest covering declaration — the
/// method — is the answer, exactly as in the Rust fixture.
const CPP: LangFixture = LangFixture {
    language: "cpp",
    ext: "cpp",
    drift: "struct Holder {\n    void alpha() {\n        int x = 1;\n        (void)x;\n        \
            // §sec1 — recorded as `beta`, so this citation has drifted\n    }\n};\n",
    matched: "struct Keeper {\n    void gamma() {\n        int y = 2;\n        (void)y;\n        \
              // §sec2 — recorded as `gamma`, so this citation is clean\n    }\n};\n",
    matched_symbol: "gamma",
};

/// Every fixture this test file holds, looked up by language. The POPULATION is
/// the binary's own answer (law 7), never this list: a backend this build ships
/// and this table has no fixture for FAILS rather than being skipped, so the
/// round that adds a language cannot add it without a control.
const FIXTURES: &[&LangFixture] = &[&RUST, &CPP];

/// Laws 1 to 5 run on the Rust fixture.
///
/// `cpp_case` adds `src/bar.cpp`, bound at FILE level with no symbol recorded,
/// for a language no test here configures a resolver for: an unreachable file
/// that makes no symbol-level claim, which is the state law 3 separates from
/// the one law 5 refuses.
fn write_workspace(ws: &Path, resolver: &str, cpp_case: bool) {
    write_language_workspace(ws, resolver, &RUST, cpp_case);
}

/// Write the two-site tree of `fx` into `ws`, with `resolver` as the only
/// `[plugins]` content.
fn write_language_workspace(ws: &Path, resolver: &str, fx: &LangFixture, cpp_case: bool) {
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

    let drift_file = format!("src/drift.{}", fx.ext);
    let matched_file = format!("src/matched.{}", fx.ext);
    fs::write(ws.join(&drift_file), fx.drift).unwrap();
    fs::write(ws.join(&matched_file), fx.matched).unwrap();

    let mut sections = serde_json::json!({
        "sec1": {
            "title": "One",
            "parent_doc": "docs/GENERATED.md",
            "bindings": [
                {"file": drift_file, "symbol": "beta", "kind": "implements"}
            ]
        },
        "sec2": {
            "title": "Two",
            "parent_doc": "docs/GENERATED.md",
            "bindings": [
                {"file": matched_file, "symbol": fx.matched_symbol, "kind": "implements"}
            ]
        }
    });
    if cpp_case {
        fs::write(
            ws.join("src/bar.cpp"),
            "void delta() {\n    // §sec3 — bound at file level, so nothing is asked of cpp\n}\n",
        )
        .unwrap();
        sections["sec3"] = serde_json::json!({
            "title": "Three",
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

    // NON-VACUITY, from the run's own census: this tree holds two citations the
    // symbol axis is supposed to judge, in two files. The `null` below is about
    // those citations, not about an empty tree.
    assert_eq!(
        (
            json["symbol_axis"]["checked_citations"].as_u64(),
            json["symbol_axis"]["checked_files"].as_u64()
        ),
        (Some(2), Some(2)),
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

    // EXACTLY ONE, AND AT THE DRIFT SITE. Both halves say which resolver ran:
    // an answer of nothing leaves this at zero, and a different vocabulary of
    // names reddens the matched site too and leaves it at two.
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
        (Some("src/drift.rs"), Some(5), Some("§sec1")),
        "the judgement names the citation, not just the file: {}",
        symbol[0]
    );
    assert!(
        violations.iter().all(|v| v["file"] != "src/matched.rs"),
        "the site whose recorded symbol is what the Rust grammar answers must be \
         CLEAN — that is the half of this fixture a wrong resolver cannot fake: \
         {violations:?}"
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
    // `src/drift.rs`, and this file is not it.
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
        "the store records symbols for two files and nothing can check them — \
         that must stop the run, not pass it: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("2 citation(s)") && stderr.contains("plugins.symbol_resolver.rust"),
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
            "sections": {
                "sec1": {"title": "One", "parent_doc": "docs/GENERATED.md",
                    "bindings": [{"file": "src/drift.rs", "kind": "implements"}]},
                "sec2": {"title": "Two", "parent_doc": "docs/GENERATED.md",
                    "bindings": [{"file": "src/matched.rs", "kind": "implements"}]}
            },
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
        Some(2),
        "two claims, and both are the Rust ones: {}",
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

// ---------------------------------------------------------------------------
// Laws 6 to 9 — HOW FAR THIS BUILD CAN REACH AT ALL.
//
// Laws 1 to 5 are about one run's configuration. These four are one question
// further out, and it is the question the consumer had to answer by reading our
// source: WHICH LANGUAGES CAN THIS BINARY JUDGE? SCE's spec ledger enrols five
// backend runtimes; two of them take symbol-level binding and three take
// file-level, and their own test names the reason for each in prose it had to
// copy out of this repository — "Mnemosyne maps .go to the `go` language but
// ships no resolver plugin for it", "`.kt` is absent from Mnemosyne's
// symbol-axis extension table entirely". Prose in their tree about the inside
// of ours is a restatement that decays silently the moment we ship a resolver:
// nothing they run can tell them the gap closed, so the gap outlives itself.
//
// The answer is a fact about the BUILD, not about a tree, so it is available
// without a workspace and it is the same everywhere the binary runs.
// ---------------------------------------------------------------------------

/// `describe-symbol-axis-reach --json` in `dir`, parsed.
fn reach(dir: &Path) -> (Output, serde_json::Value) {
    let out = Command::new(cli())
        .args(["describe-symbol-axis-reach", "--json"])
        .current_dir(dir)
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

/// The backends the report names, as `(backend key, language)`.
fn reported_backends(json: &serde_json::Value) -> Vec<(String, String)> {
    json["in_process_backends"]
        .as_array()
        .expect("in_process_backends array")
        .iter()
        .map(|b| {
            (
                b["backend"].as_str().expect("backend key").to_string(),
                b["language"].as_str().expect("language").to_string(),
            )
        })
        .collect()
}

/// LAW 6 — THE BUILD NAMES THE BACKENDS IT SHIPS, ANYWHERE.
///
/// Which plugins are compiled in is a property of the binary, so the answer
/// cannot depend on standing in a workspace: a consumer deciding whether to
/// enrol a tree has no workspace of ours to stand in, and the CI job that would
/// check the answer runs in their checkout.
#[test]
fn the_build_names_the_backends_it_ships_without_a_workspace() {
    let empty = TempDir::new().unwrap();
    let (out, json) = reach(empty.path());
    assert!(
        out.status.success(),
        "naming what this build contains is not a judgement and cannot fail: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let backends = reported_backends(&json);
    assert!(
        backends.len() >= 2,
        "a build that names no backend would satisfy every law below vacuously: \
         {json}"
    );
    // Every backend names a language the extension table can actually produce.
    // A backend keyed to a language no file maps to is unreachable config, the
    // shape Round 855 made a hard error one step out.
    let languages: Vec<&str> = json["symbol_axis_languages"]
        .as_array()
        .expect("symbol_axis_languages array")
        .iter()
        .map(|l| l.as_str().expect("language"))
        .collect();
    for (backend, language) in &backends {
        assert!(
            languages.contains(&language.as_str()),
            "backend `{backend}` claims language `{language}`, which no file \
             extension maps to: {json}"
        );
    }
    // The language set is the library's, not a list retyped into the command.
    let from_lib: Vec<&str> = mnemosyne_validate::code_refs::symbol_axis_languages()
        .into_iter()
        .collect();
    assert_eq!(
        languages, from_lib,
        "the report must print the extension table's own answer: {json}"
    );

    // Every extension is printed with the language it maps to, because the
    // consumer's third question is not about a language at all — it is whether
    // a FILE of theirs is on the table (`.kt` was not).
    let exts: BTreeMap<String, String> = json["extensions"]
        .as_array()
        .expect("extensions array")
        .iter()
        .map(|e| {
            (
                e["extension"].as_str().expect("extension").to_string(),
                e["language"].as_str().expect("language").to_string(),
            )
        })
        .collect();
    assert_eq!(
        exts.get("rs").map(String::as_str),
        Some("rust"),
        "extensions: {exts:?}"
    );
    for language in &languages {
        assert!(
            exts.values().any(|l| l == language),
            "language `{language}` is in the table's range but no extension \
             reaches it: {exts:?}"
        );
    }
}

/// LAW 7 — EVERY BACKEND THIS BUILD NAMES RESOLVES ITS OWN LANGUAGE.
///
/// The population is the binary's answer from law 6, so this cannot be a list
/// of the languages someone remembered to test. A backend with no fixture here
/// FAILS rather than being skipped: the round that adds a language cannot add
/// it without a control that says which grammar answered.
///
/// Each fixture goes the whole way through the binary — config parse, plugin
/// registry, the real tree-sitter resolver, a judgement, the exit code a hook
/// reads — and the two-site shape is what makes the answer attributable: a
/// resolver that answers nothing leaves the count at 0 and one that answers a
/// different vocabulary raises it to 2.
#[test]
fn every_backend_this_build_ships_resolves_its_own_language() {
    let (_, report) = reach(TempDir::new().unwrap().path());
    let backends = reported_backends(&report);
    assert!(!backends.is_empty(), "nothing to check: {report}");

    for (backend, language) in &backends {
        let fx = FIXTURES
            .iter()
            .find(|f| f.language == language)
            .unwrap_or_else(|| {
                panic!(
                    "this build ships backend `{backend}` for language \
                     `{language}` and this test has no fixture for it — add the \
                     two-site control, do not widen the population"
                )
            });

        let tmp = TempDir::new().unwrap();
        let resolver = format!(
            "[plugins.symbol_resolver.{language}]\ntransport = \"in-process\"\n\
             backend = \"{backend}\"\n"
        );
        write_language_workspace(tmp.path(), &resolver, fx, false);
        let (out, json) = validate(tmp.path());

        assert_eq!(
            json["symbol_mismatch_count"],
            serde_json::json!(1),
            "`{backend}` on its own language must judge both sites and find \
             exactly the one drift — 0 means it answered nothing, 2 means it \
             answered a vocabulary that is not {language}'s: {json}"
        );
        let violations = json["violations"].as_array().expect("violations array");
        let symbol: Vec<&serde_json::Value> = violations
            .iter()
            .filter(|v| v["kind"] == "symbol_mismatch")
            .collect();
        assert_eq!(
            symbol
                .first()
                .map(|v| (v["file"].as_str(), v["entry_id"].as_str())),
            Some((
                Some(format!("src/drift.{}", fx.ext).as_str()),
                Some("§sec1")
            )),
            "the one judgement must name the drifted citation: {violations:?}"
        );
        assert!(
            !not_judged(&json).contains_key("symbol_mismatch"),
            "a configured backend leaves nothing unjudged: {:?}",
            not_judged(&json)
        );
        assert!(
            !out.status.success(),
            "`severity_binding` defaults to reject, so the drift must fail the \
             run for {language} as it does for every other: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// LAW 8 — A BACKEND FOR ANOTHER LANGUAGE IS REFUSED, NOT WIRED.
///
/// `[plugins.symbol_resolver.rust] backend = "tree-sitter-cpp"` parsed,
/// registered and ran, and what came back was whatever the C++ grammar made of
/// Rust source. Round 855 already settled the two neighbouring shapes — a
/// language key nothing maps to, and a backend name this build has no plugin
/// for — as config errors rather than warnings, for the reason that
/// `severity_binding = reject` reads as symbol-level enforcement while the run
/// performs none. A backend wired to the WRONG language is worse than either:
/// enforcement does happen, against answers from a grammar that never saw this
/// language, and the count it publishes is a number rather than a `null`.
///
/// The oracle is the exit code on a tree with nothing else to fail on, so this
/// cannot pass by agreeing with some other refusal's wording.
#[test]
fn a_backend_for_another_language_is_refused_rather_than_wired() {
    let write = |ws: &Path, backend: &str| {
        fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::write(
            ws.join("mnemosyne.toml"),
            format!(
                "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n\
                 [plugins.set_equality_validator]\npaths = [\"src/\"]\ncomment_only = true\n\
                 [plugins.symbol_resolver.rust]\ntransport = \"in-process\"\n\
                 backend = \"{backend}\"\n"
            ),
        )
        .unwrap();
        fs::write(
            ws.join("docs/.atomic/workspace.atomic.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 11,
                "sections": {},
                "changelog_entries": {"Round 1": {"decision_summary": "the one entry"}}
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(ws.join("docs/GENERATED.md"), "# Stub\n").unwrap();
    };

    // CONTROL FIRST: the same empty tree with the right backend passes, so the
    // failure below is about the pairing and not about the fixture.
    let ok = TempDir::new().unwrap();
    write(ok.path(), "tree-sitter-rust");
    let out = Command::new(cli())
        .args(["validate-code-refs"])
        .current_dir(ok.path())
        .output()
        .expect("cli exec");
    assert!(
        out.status.success(),
        "the control must pass or the assertion below says nothing: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let bad = TempDir::new().unwrap();
    write(bad.path(), "tree-sitter-cpp");
    let out = Command::new(cli())
        .args(["validate-code-refs"])
        .current_dir(bad.path())
        .output()
        .expect("cli exec");
    assert!(
        !out.status.success(),
        "a C++ resolver registered under `rust` must stop the run: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("tree-sitter-cpp") && stderr.contains("cpp") && stderr.contains("rust"),
        "the refusal must name the backend, the language it resolves, and the \
         language it was asked to resolve: {stderr}"
    );
}

/// LAW 9 — THE LANGUAGES THIS BUILD CANNOT RESOLVE ARE NAMED AND COUNTED.
///
/// A language the extension table maps files to, with no backend to resolve
/// them, is where the axis stops. Until now that boundary existed only as an
/// absence — the config for it could not be written, and the only way to learn
/// which languages were affected was to read this repository's Cargo files.
/// SCE did exactly that and wrote the answer into their tree as prose.
///
/// The list below is a CLAIM ABOUT THIS BUILD that shrinks, and the equality is
/// deliberate: a round that ships a resolver must delete its language from here
/// in the same change, which is what stops a gap list from outliving its gap.
#[test]
fn the_languages_this_build_cannot_resolve_are_named_and_counted() {
    let (_, json) = reach(TempDir::new().unwrap().path());

    let without: Vec<&str> = json["languages_without_backend"]
        .as_array()
        .expect("languages_without_backend array")
        .iter()
        .map(|l| l.as_str().expect("language"))
        .collect();
    assert_eq!(
        without,
        vec!["go", "python"],
        "the symbol axis stops at exactly these languages; shipping a resolver \
         deletes one from this list in the same change: {json}"
    );

    // The report's own arithmetic, recomputed from the other two fields it
    // prints: what is missing is the extension table's range minus the
    // languages the shipped backends cover.
    let served: Vec<String> = reported_backends(&json)
        .into_iter()
        .map(|(_, language)| language)
        .collect();
    let recomputed: Vec<&str> = json["symbol_axis_languages"]
        .as_array()
        .expect("symbol_axis_languages array")
        .iter()
        .map(|l| l.as_str().expect("language"))
        .filter(|l| !served.iter().any(|s| s == l))
        .collect();
    assert_eq!(
        without, recomputed,
        "the published gap must be the difference between the two published \
         sets, not a third answer: {json}"
    );
}
