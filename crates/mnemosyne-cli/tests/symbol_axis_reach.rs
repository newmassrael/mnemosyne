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
//!
//! AND WHAT EVERY CITATION AXIS PUBLISHES (Rounds 1167, 1168). The five laws
//! above are about one axis; the two below are about all eight, and they live
//! here because this is where a violation is walked as it left the binary. Law
//! 2b: an axis carries exactly the evidence it declares, on the wire and on the
//! line a person reads. Law 6: `describe-citation-axes` publishes that contract
//! — and the report is checked as the ORACLE FOR A REAL SCAN, not merely
//! against the library it is built from, so a consumer who parses by what the
//! report told them parses what the gate actually emits.

use std::collections::{BTreeMap, BTreeSet};
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

/// The Go fixture. The citation is the first line of each method body, so the
/// doc-comment rule — which Go's spec switches ON — does not fire (the next
/// sibling is a statement, not a declaration) and the answer comes from the
/// smallest covering declaration, as in the other two.
const GO: LangFixture = LangFixture {
    language: "go",
    ext: "go",
    // A GROUPED `var`, WHICH IS A SHAPE C++ CANNOT NAME. The method form this
    // fixture used until Round 1165 read identically under the C++ grammar —
    // `func (h Holder) alpha() {` is a function definition there too, named
    // `alpha` — so the wire that pairs `go` with a backend had no reader.
    drift: "package p\n\nvar (\n\t\
            // §sec1 — recorded as `beta`, so this citation has drifted\n\talpha int\n)\n",
    matched: "package p\n\nvar (\n\t\
              // §sec2 — recorded as `gamma`, so this citation is clean\n\tgamma int\n)\n",
    matched_symbol: "gamma",
};

/// The Python fixture. The citation is a `#` line inside each method body,
/// followed by a statement rather than a definition, so the comment rule —
/// which Python's spec switches ON — does not fire and the answer comes from
/// the smallest covering definition, as in the other three.
const PYTHON: LangFixture = LangFixture {
    language: "python",
    ext: "py",
    drift: "class Holder:\n    def alpha(self):\n        \
            # §sec1 — recorded as `beta`, so this citation has drifted\n        x = 1\n        \
            return x\n",
    matched: "class Keeper:\n    def gamma(self):\n        \
              # §sec2 — recorded as `gamma`, so this citation is clean\n        y = 2\n        \
              return y\n",
    matched_symbol: "gamma",
};

/// The Kotlin fixture. As in the others the citation is a `//` line inside the
/// method body followed by a statement, so the comment rule — which Kotlin's
/// spec switches ON over BOTH comment spellings — does not fire and the answer
/// comes from the smallest covering declaration.
const KOTLIN: LangFixture = LangFixture {
    language: "kotlin",
    ext: "kt",
    // AN `object` HOLDING A PROPERTY, for the same reason Go's changed: the
    // method form read identically under BOTH the C++ and the Python grammars,
    // so two different wrong pairings judged this fixture the same way its own
    // backend does.
    drift: "package p\n\nobject Holder {\n    \
            // §sec1 — recorded as `beta`, so this citation has drifted\n    val alpha: Int = 1\n}\n",
    matched: "package p\n\nobject Keeper {\n    \
              // §sec2 — recorded as `gamma`, so this citation is clean\n    val gamma: Int = 2\n}\n",
    matched_symbol: "gamma",
};

/// Every fixture this test file holds, looked up by language. The POPULATION is
/// the binary's own answer (law 7), never this list: a backend this build ships
/// and this table has no fixture for FAILS rather than being skipped, so the
/// round that adds a language cannot add it without a control.
const FIXTURES: &[&LangFixture] = &[&RUST, &CPP, &GO, &PYTHON, &KOTLIN];

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

    // THE JUDGEMENT SAYS WHAT THE CODE ACTUALLY SAYS (Round 1158). Until this
    // round the violation carried file, line and `§id` and DROPPED the symbol the
    // resolver answered, so a consumer learned a citation had drifted but not to
    // what — and Round 1154, which added thirty new bindings in C++ by repairing
    // the doc-comment rule, made that specific: someone meeting a new mismatch
    // could not tell from the report whether their code had moved or this
    // resolver had started answering. Both names are here now.
    assert_eq!(
        (&symbol[0]["found"], &symbol[0]["expected"]),
        (
            &serde_json::json!("alpha"),
            // A SET, because a section legitimately records more than one symbol
            // in one file: printing whichever member the comparison rejected
            // would look definite and be arbitrary.
            &serde_json::json!(["beta"])
        ),
        "the judgement must name the symbol the resolver READ and the ones the \
         store RECORDS — the drift is the pair, and one half of it is not a \
         diagnosis: {}",
        symbol[0]
    );
    // AND EVERY AXIS CARRIES EXACTLY WHAT IT DECLARES (Round 1167, generalising
    // Round 1158's `found`-iff-symbol_mismatch). The expected key set is ASKED
    // OF THE LIBRARY — `AuditAxis::evidence().wire_keys()` — rather than spelled
    // here, so this law is about what the binary printed against the same
    // declaration the serializer is checked against, and a payload added to a
    // fourth axis is covered here the day it is declared rather than the day
    // someone remembers to widen this loop.
    for v in violations {
        let tag = v["kind"].as_str().expect("every violation names its kind");
        let axis = mnemosyne_validate::code_refs::AuditAxis::all()
            .into_iter()
            .find(|a| a.kind_tag() == tag)
            .unwrap_or_else(|| panic!("the binary printed a kind no axis owns: {tag}"));
        let declared: BTreeSet<&str> = axis.evidence().wire_keys().iter().copied().collect();
        // The universe of evidence keys, derived the same way: whatever ANY
        // axis declares is what a violation might wrongly be carrying.
        let carried: BTreeSet<&str> = mnemosyne_validate::code_refs::AuditAxis::all()
            .into_iter()
            .flat_map(|a| a.evidence().wire_keys())
            .copied()
            .filter(|k| v.get(*k).is_some())
            .collect();
        assert_eq!(
            carried, declared,
            "an axis carries the evidence it declares and no other: {v}"
        );
    }

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

/// LAW 2b — EVERY AXIS THAT READS SOMETHING PUBLISHES IT, THROUGH THE BINARY
/// (Round 1167).
///
/// Law 2's walk holds over whatever violations its tree produces, and that tree
/// produces one kind. The generalised claim — a citation carries exactly the
/// evidence its axis declares — is only as wide as the population it is asked
/// about, so this is the tree that provokes all three: a drifted symbol, a
/// citation of a section that binds somebody else, and a comment that restates
/// a fact instead of pointing at it.
///
/// AND BOTH SURFACES, because a consumer reads one of them: `--json` for the
/// wire and the plain run for the line a person sees. The R1045 defect was a
/// payload that held on the machine wire while the human line said less.
#[test]
fn every_axis_that_reads_something_publishes_it_through_the_binary() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, RUST_RESOLVER, false);
    // The prose axis is opt-in, and this is the one config line that turns it
    // on. Appended rather than rebuilt so the tree stays law 2's tree.
    let toml = fs::read_to_string(ws.join("mnemosyne.toml")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        toml.replace(
            "comment_only = true",
            "comment_only = true\nseverity_prose_fact_assertion = \"reject\"",
        ),
    )
    .unwrap();
    // Cites a section that binds `src/drift.rs` and not this file.
    fs::write(ws.join("src/stray.rs"), "// §sec1 cited from nowhere\n").unwrap();
    // Restates in prose a fact the store homes.
    fs::write(
        ws.join("src/restated.rs"),
        "// supersede §sec1, which the store already records\n",
    )
    .unwrap();

    let (out, json) = validate(ws);
    let violations = json["violations"].as_array().expect("violations array");

    let of_kind = |k: &str| -> Vec<&serde_json::Value> {
        violations.iter().filter(|v| v["kind"] == k).collect()
    };

    // ---- citation_unbound names what the section DOES bind ----
    let unbound = of_kind("citation_unbound");
    assert_eq!(
        unbound.len(),
        2,
        "both new files cite a section that binds neither: {violations:?}"
    );
    for v in &unbound {
        assert_eq!(
            v["bound_files"],
            serde_json::json!(["src/drift.rs"]),
            "the report must name the binding set that decided this violation, \
             or the consumer opens the store to find it: {v}"
        );
    }

    // ---- prose_fact_assertion names the verb it matched ----
    let prose = of_kind("prose_fact_assertion");
    assert_eq!(prose.len(), 1, "one restatement: {violations:?}");
    assert_eq!(
        prose[0]["assertion_verb"],
        serde_json::json!("supersede"),
        "the rule is a list of spellings this repository owns; a reader of the \
         flagged line cannot otherwise say which word tripped it: {}",
        prose[0]
    );

    // ---- symbol_mismatch still carries Round 1158's pair ----
    let drift = of_kind("symbol_mismatch");
    assert_eq!(drift.len(), 1, "one drift: {violations:?}");
    assert_eq!(
        (&drift[0]["found"], &drift[0]["expected"]),
        (&serde_json::json!("alpha"), &serde_json::json!(["beta"])),
        "{}",
        drift[0]
    );

    // ---- and the equality holds over the whole population, derived ----
    let declaring: BTreeSet<&str> = mnemosyne_validate::code_refs::AuditAxis::all()
        .into_iter()
        .filter(|a| a.side() == mnemosyne_validate::code_refs::AuditSide::Citation)
        .filter(|a| !a.evidence().wire_keys().is_empty())
        .map(mnemosyne_validate::code_refs::AuditAxis::kind_tag)
        .collect();
    let reached: BTreeSet<&str> = violations
        .iter()
        .filter_map(|v| v["kind"].as_str())
        .collect();
    assert!(
        declaring.is_subset(&reached),
        "this tree must provoke every axis that declares evidence, or the law is \
         narrower than it reads — missing {:?} from {reached:?}",
        declaring.difference(&reached).collect::<Vec<_>>()
    );
    // AND THE EQUALITY ITSELF, over THIS population. Law 2 runs the same walk
    // over a tree that produces one kind, so it can only ever speak for the
    // symbol axis: renaming another axis's wire key left law 2 green while the
    // declaration and the serializer disagreed, which is how this loop came to
    // be here as well as there. Keys are asked of the library, never spelled.
    for v in violations {
        let tag = v["kind"].as_str().expect("every violation names its kind");
        let axis = mnemosyne_validate::code_refs::AuditAxis::all()
            .into_iter()
            .find(|a| a.kind_tag() == tag)
            .unwrap_or_else(|| panic!("the binary printed a kind no axis owns: {tag}"));
        let declared: BTreeSet<&str> = axis.evidence().wire_keys().iter().copied().collect();
        let carried: BTreeSet<&str> = mnemosyne_validate::code_refs::AuditAxis::all()
            .into_iter()
            .flat_map(|a| a.evidence().wire_keys())
            .copied()
            .filter(|k| v.get(*k).is_some())
            .collect();
        assert_eq!(
            carried, declared,
            "an axis carries the evidence it declares and no other: {v}"
        );
    }

    // ---- THE HUMAN LINE, not only the wire ----
    let plain = Command::new(cli())
        .arg("validate-code-refs")
        .current_dir(ws)
        .output()
        .expect("cli exec");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&plain.stdout),
        String::from_utf8_lossy(&plain.stderr)
    );
    for needle in [
        "the section binds `src/drift.rs`",
        "the prose asserts `supersede`",
        "code says `alpha`, store records `beta`",
    ] {
        assert!(
            text.contains(needle),
            "the line a person reads must carry it too — missing {needle:?} in:\n{text}"
        );
    }

    assert!(
        !out.status.success(),
        "an unbound citation and a drift are both reject-class: {}",
        String::from_utf8_lossy(&out.stderr)
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

/// LAW 0 — EVERY FIXTURE BELOW CAN SAY WHICH GRAMMAR ANSWERED.
///
/// THIS IS THE PROPERTY A 3.5-HOUR SWEEP WAS WAITING ON. Round 1145 ran
/// `crates/mnemosyne-cli/sweeps/citation-path-scope.sweep.json` in full and
/// fifteen of its sixteen injections fired; the sixteenth,
/// `the-wire-hands-rust-the-other-languages-resolver`, reddened NOTHING. Handing
/// the `rust` key the C++ resolver left every test green, because the Rust
/// fixture's `fn gamma(&self) { … }` is also a valid C++ function definition —
/// return type `fn`, name `gamma` — so both grammars answered `gamma` and no
/// assertion could tell them apart. The sweep has failed on that ever since,
/// which is correct and is not progress.
///
/// A SWEEP IS NOT THE RIGHT INSTRUMENT FOR IT EITHER. That injection asks a
/// question about the FIXTURE, and answering it cost a full workspace suite per
/// run. Here it is a fact about two resolvers reading the same bytes: run every
/// other shipped backend over each fixture's source and require the reading to
/// differ. A fixture that reads identically under two grammars is not a control,
/// whatever the run around it reports.
#[test]
fn every_fixture_reads_differently_under_a_grammar_that_is_not_its_own() {
    // EVERY PAIR, NOT THE FIRST ONE. A law that stops at the first collision
    // reports one fixture to repair and hides the rest, and the repair is a
    // respelling per fixture rather than one change.
    let mut indistinguishable: Vec<String> = Vec::new();
    for fx in FIXTURES {
        let own = mnemosyne_cli::backends::IN_PROCESS_BACKENDS
            .iter()
            .find(|b| b.language == fx.language)
            .unwrap_or_else(|| panic!("no backend resolves `{}`", fx.language));
        for source in [fx.drift, fx.matched] {
            // THE CITED LINE AND NOT THE WHOLE FILE, because the judgement reads
            // one line. Comparing whole answer maps passes as soon as the two
            // grammars disagree ANYWHERE — which they do, on lines nothing is
            // recorded against — and that is a weaker claim wearing this law's
            // name.
            let cited = source
                .lines()
                .position(|line| line.contains("§sec"))
                .map(|index| index as u32 + 1)
                .unwrap_or_else(|| panic!("the {} fixture holds no citation", fx.language));
            let read = |backend: &mnemosyne_cli::backends::InProcessBackend| {
                backend
                    .make()
                    .resolve_symbols_at(Path::new("/no/such/file"), source, &[cited])
                    .expect("the resolver answers")
                    .remove(&cited)
            };
            let mine = read(own);
            assert!(
                mine.is_some(),
                "the {} fixture's own grammar answers nothing at line {cited}, \
                 so this law compares two absences\n--- source ---\n{source}",
                fx.language
            );
            for other in mnemosyne_cli::backends::IN_PROCESS_BACKENDS {
                if other.language == fx.language {
                    continue;
                }
                let theirs = read(other);
                if theirs == mine {
                    indistinguishable.push(format!(
                        "{} fixture line {cited}: both {} and {} answer {mine:?}",
                        fx.language, fx.language, other.language
                    ));
                }
            }
        }
    }
    assert!(
        indistinguishable.is_empty(),
        "{} fixture/grammar pair(s) read identically at the cited line, so the \
         wire that pairs a language with a backend has no reader there:\n  {}",
        indistinguishable.len(),
        indistinguishable.join("\n  ")
    );
}

/// LAW 6b — THE REPORT PUBLISHES WHERE A CITATION IN A COMMENT BINDS.
///
/// The consumer's question that this answers is the one Round 1162 changed the
/// answer to: a `§` citation written in a Rust `///` comment used to bind to the
/// enclosing item and now binds to the item below it. Nothing they run could
/// have told them either way — the answer lived in a `documented_kinds` list in
/// a crate of ours, which is the same shape as the prose gap list this whole
/// verb exists to replace.
///
/// THE ORACLE IS THE SPEC ITSELF, through the binary. Comparing the report
/// against a list retyped here would check that two lists in this repository
/// agree; comparing it against `IN_PROCESS_BACKENDS`'s own spec checks that what
/// the report prints is what the resolver will do.
#[test]
fn the_report_publishes_the_doc_comment_rule_each_backend_answers_with() {
    let (_, report) = reach(TempDir::new().unwrap().path());
    let rows = report["in_process_backends"]
        .as_array()
        .expect("in_process_backends array");
    assert!(!rows.is_empty(), "nothing to check: {report}");

    let mut with_markers = 0usize;
    for row in rows {
        let key = row["backend"].as_str().expect("backend key");
        let backend = mnemosyne_cli::backends::find(key)
            .unwrap_or_else(|| panic!("the report names `{key}`, which no row holds"));
        let rule = &backend.spec.doc_comments;

        let published = |field: &str| -> Vec<String> {
            row["doc_comments"][field]
                .as_array()
                .unwrap_or_else(|| panic!("{key}: doc_comments.{field} is not an array: {row}"))
                .iter()
                .map(|v| v.as_str().expect("string").to_string())
                .collect()
        };
        assert_eq!(published("comment_kinds"), rule.comment_kinds);
        assert_eq!(published("inward_markers"), rule.inward_markers);
        assert_eq!(published("documented_kinds"), rule.documented_kinds);
        assert_eq!(
            row["declaration_patterns"].as_u64().map(|n| n as usize),
            backend.spec.pattern_count().ok(),
            "{key}: the report's pattern count is not the compiled query's"
        );

        // THE RULE IS NOT OPTIONAL, and the report is where a consumer would
        // read it as optional if a backend published an empty list.
        assert!(
            !rule.documented_kinds.is_empty(),
            "{key}: publishes no documented kind, so a consumer reads this \
             language as one where a doc comment binds nowhere"
        );
        with_markers += usize::from(!rule.inward_markers.is_empty());
    }

    // ONE BACKEND MUST DIFFER, or the field is decoration: a report where every
    // language answers the same thing tells a consumer nothing they needed a
    // report for.
    assert_eq!(
        with_markers, 1,
        "exactly one shipped backend has an inward spelling (Rust's `//!`); \
         if that changed, this law is what should say so: {report}"
    );
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
    // THE LIST IS EMPTY NOW, AND THE ASSERTION CHANGES SHAPE WITH IT. While it
    // had members this was a shrinking claim about the build; empty, it is the
    // invariant those rounds were walking towards — every language a file can
    // map to has a resolver. An extension row added without a backend fails
    // HERE, at the moment it is added, instead of at the moment a consumer
    // notices their citations went to file-level binding.
    assert!(
        without.is_empty(),
        "every language a file can map to must have a resolver; these have \
         none: {without:?} — either ship the backend in the same change as the \
         extension row, or the axis silently stops there: {json}"
    );
    // NON-VACUITY: an empty difference is also what an empty POPULATION looks
    // like. The languages are real and every one of them is served.
    let languages: Vec<&str> = json["symbol_axis_languages"]
        .as_array()
        .expect("symbol_axis_languages array")
        .iter()
        .map(|l| l.as_str().expect("language"))
        .collect();
    assert!(
        languages.len() >= 5,
        "the table's range collapsed rather than the gap closing: {json}"
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

/// `describe-citation-axes --json` in `dir`, parsed.
fn citation_axes(dir: &Path) -> (Output, serde_json::Value) {
    let out = Command::new(cli())
        .args(["describe-citation-axes", "--json"])
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

/// LAW 6 — THE CONTRACT IS PUBLISHED, AND IT DESCRIBES THE GATE'S REAL OUTPUT
/// (Round 1168).
///
/// Rounds 1158 and 1167 each put evidence on the violation wire and each ended
/// by writing down that a consumer ought to be told, in a reply. This is the
/// same replacement Round 1164 made for the doc-comment rule: the answer stops
/// being a list inside a crate of ours and becomes something the consumer's own
/// copy of the binary will say.
///
/// AND THE ORACLE IS A REAL SCAN, not the library the report is built from.
/// Checking the report against `AuditAxis::evidence` alone would prove that two
/// readings of one table agree — true, and no use to somebody writing a parser.
/// What they need is that the keys the report names are the keys the gate
/// EMITS, so this walks the violations of a tree that provokes all three
/// evidence axes and holds each one against the published contract.
#[test]
fn the_published_axis_contract_is_the_one_a_real_scan_obeys() {
    let tmp = TempDir::new().unwrap();
    let (out, report) = citation_axes(tmp.path());
    assert!(
        out.status.success(),
        "a property of the build needs no workspace: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // ---- the published axis space is the library's, both halves ----
    let published: Vec<&serde_json::Value> = report["citation_axes"]
        .as_array()
        .expect("citation_axes array")
        .iter()
        .collect();
    let published_names: BTreeSet<&str> = published
        .iter()
        .map(|r| r["axis"].as_str().expect("axis name"))
        .collect();
    let library_names: BTreeSet<&str> = mnemosyne_validate::code_refs::AuditAxis::all()
        .into_iter()
        .filter(|a| a.side() == mnemosyne_validate::code_refs::AuditSide::Citation)
        .map(mnemosyne_validate::code_refs::AuditAxis::kind_tag)
        .collect();
    assert_eq!(
        published_names, library_names,
        "the report must name every citation-side axis and no other: {report}"
    );
    let published_spec: BTreeSet<&str> = report["spec_side_axes"]
        .as_array()
        .expect("spec_side_axes array")
        .iter()
        .map(|v| v.as_str().expect("axis name"))
        .collect();
    assert!(
        !published_spec.is_empty() && published_spec.is_disjoint(&published_names),
        "the spec-side half must be named, and named apart — a spec violation \
         carries its own fields, not this contract: {report}"
    );

    // ---- and the keys it names are the keys the LIBRARY declares ----
    let keys_of = |axis: &str| -> BTreeSet<String> {
        published
            .iter()
            .find(|r| r["axis"] == axis)
            .unwrap_or_else(|| panic!("the report does not name `{axis}`: {report}"))
            ["evidence_keys"]
            .as_array()
            .expect("evidence_keys array")
            .iter()
            .map(|v| v.as_str().expect("key").to_string())
            .collect()
    };
    for a in mnemosyne_validate::code_refs::AuditAxis::all()
        .into_iter()
        .filter(|a| a.side() == mnemosyne_validate::code_refs::AuditSide::Citation)
    {
        let declared: BTreeSet<String> = a
            .evidence()
            .wire_keys()
            .iter()
            .map(|k| (*k).to_string())
            .collect();
        assert_eq!(
            keys_of(a.kind_tag()),
            declared,
            "`{}`: the report and the declaration must name the same keys",
            a.kind_tag()
        );
        let shape = published
            .iter()
            .find(|r| r["axis"] == a.kind_tag())
            .expect("named above")["evidence"]
            .as_str()
            .expect("evidence name");
        assert_eq!(
            shape,
            a.evidence().as_str(),
            "`{}`: an axis that reads nothing must SAY `nothing` rather than \
             leave the field out",
            a.kind_tag()
        );
    }

    // ---- THE ORACLE: a real scan, judged against what the report published ----
    let ws = TempDir::new().unwrap();
    write_workspace(ws.path(), RUST_RESOLVER, false);
    let toml = fs::read_to_string(ws.path().join("mnemosyne.toml")).unwrap();
    fs::write(
        ws.path().join("mnemosyne.toml"),
        toml.replace(
            "comment_only = true",
            "comment_only = true\nseverity_prose_fact_assertion = \"reject\"",
        ),
    )
    .unwrap();
    fs::write(
        ws.path().join("src/stray.rs"),
        "// §sec1 cited from nowhere\n",
    )
    .unwrap();
    fs::write(
        ws.path().join("src/restated.rs"),
        "// supersede §sec1, which the store already records\n",
    )
    .unwrap();
    let (_, scan) = validate(ws.path());
    let violations = scan["violations"].as_array().expect("violations array");

    let universe: BTreeSet<String> = published
        .iter()
        .flat_map(|r| r["evidence_keys"].as_array().expect("array"))
        .map(|v| v.as_str().expect("key").to_string())
        .collect();
    let mut reached: BTreeSet<&str> = BTreeSet::new();
    for v in violations {
        let tag = v["kind"].as_str().expect("kind");
        reached.insert(tag);
        let carried: BTreeSet<String> = universe
            .iter()
            .filter(|k| v.get(k.as_str()).is_some())
            .cloned()
            .collect();
        assert_eq!(
            carried,
            keys_of(tag),
            "the gate emitted a violation the published contract does not \
             describe: {v}"
        );
    }
    // NON-VACUOUS, DERIVED FROM THE REPORT ITSELF: every axis the report says
    // carries evidence must be one this tree actually provoked, or the walk
    // above is a walk over axes that carry nothing.
    let declaring: BTreeSet<&str> = published
        .iter()
        .filter(|r| !r["evidence_keys"].as_array().expect("array").is_empty())
        .map(|r| r["axis"].as_str().expect("axis"))
        .collect();
    assert!(
        declaring.is_subset(&reached),
        "this tree must provoke every axis the report says carries evidence — \
         missing {:?} from {reached:?}",
        declaring.difference(&reached).collect::<Vec<_>>()
    );

    // ---- the text form says it too (R1045) ----
    let plain = Command::new(cli())
        .arg("describe-citation-axes")
        .current_dir(tmp.path())
        .output()
        .expect("cli exec");
    let text = String::from_utf8_lossy(&plain.stdout);
    for needle in ["citation_unbound", "bound_files", "assertion_verb", "found"] {
        assert!(
            text.contains(needle),
            "the readable form must carry {needle:?} too:\n{text}"
        );
    }
}
