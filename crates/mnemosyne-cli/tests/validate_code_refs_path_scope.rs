//! SCE lift-request 4-B — `validate-code-refs --paths <file>...`.
//!
//! A live consumer runs this gate over five workspaces and pays 108.9 seconds
//! for it, so the binding axis lives on their push and violations arrive in
//! batches nobody can attribute to a commit. What they asked for is not speed:
//! it is a run scoped to the files a commit touches, judging the axes that ONE
//! FILE PLUS THE STORE can decide, and saying out loud that it did not judge
//! the rest.
//!
//! Two laws, and the second is why this file exists at all:
//!
//! 1. EQUALITY — a scoped run's answer about the named files is exactly the
//!    whole run's answer about those files. Not "similar", not "a subset that
//!    is usually the same": the same violations, at the same lines. A scoped
//!    mode that judges differently is a second gate wearing the first one's
//!    name, and a consumer would be greened by one and rejected by the other.
//!
//! 2. THE UNJUDGED AXES ARE NAMED — an empty answer is the shape of "clean"
//!    AND the shape of "nobody looked". The spec-side half (`binding_unbacked`,
//!    `impl_missing`, `verification_missing`, `misclassified_coverage`,
//!    `blanket_verifies`) needs the whole tree and is not judged here, so the
//!    report names each one and its count is `null` rather than `0`. Zero is a
//!    measurement; null is the absence of one, and a gate that prints the
//!    former for the latter has told the consumer their tree is clean.
//!
//! The partition between the two halves is not hand-written here. It is read
//! off the scoped run's own `not_judged` list, so a run that misclassifies an
//! axis fails law 1 rather than quietly redefining it — and the non-vacuity
//! assertions below pin the concrete violations, so the derivation cannot be
//! satisfied by an empty answer on both sides.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// A tree carrying BOTH halves of the audit, and the citation-side half in
/// more than one file — the scoped answer has to leave something out for the
/// equality law to have any content.
///
/// - `src/scoped.rs` — three citation-side defects, each decidable from this
///   file plus the store: a hallucinated entry, a cite of a section that binds
///   a different file, and a cite of a section that does not exist.
/// - `src/other.rs` — the same shapes, outside the scope under test.
/// - `src/bound.rs` — clean: it cites the section that names it.
/// - `README.md` — outside the configured `paths`; the gate never reads it,
///   which is a different fact from "it is clean".
fn write_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::create_dir_all(ws.join("src")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n\
         [plugins.set_equality_validator]\npaths = [\"src/\"]\n",
    )
    .unwrap();

    fs::write(
        ws.join("src/scoped.rs"),
        "// Round 999 — an entry this store does not hold\n\
         // §39 — a section that binds a different file\n\
         // §77 — a section this store does not hold\n",
    )
    .unwrap();
    fs::write(
        ws.join("src/other.rs"),
        "// Round 998 — a second entry this store does not hold\n\
         // §39 — cited from a second unbound file\n",
    )
    .unwrap();
    fs::write(
        ws.join("src/bound.rs"),
        "// §39 — the file the section binds\n",
    )
    .unwrap();
    fs::write(ws.join("README.md"), "Round 997 — never read\n").unwrap();

    let atomic = serde_json::json!({
        "schema_version": 11,
        "sections": {
            // Bound to a file that cites it (clean) and to one that does not
            // exist at all (binding_unbacked — spec-side).
            "39": {"title": "Bound", "parent_doc": "docs/GENERATED.md", "bindings": [
                {"file": "src/bound.rs", "kind": "implements"},
                {"file": "src/absent.rs", "kind": "implements"}]},
            // Normative with no implements (impl_missing) and no verifies
            // (verification_missing) — both spec-side.
            "gap": {"title": "Gap", "parent_doc": "docs/GENERATED.md"},
            // Exempt but carrying an implements binding (misclassified_coverage).
            "exempt": {"title": "Exempt", "parent_doc": "docs/GENERATED.md",
                "coverage_expectation": "out_of_scope_here",
                "bindings": [{"file": "src/bound.rs", "kind": "implements"}]},
            // One artifact verifies-bound to two sections (blanket_verifies).
            "t1": {"title": "T1", "parent_doc": "docs/GENERATED.md",
                "bindings": [{"file": "t/Case.h", "symbol": "case1", "kind": "verifies"}]},
            "t2": {"title": "T2", "parent_doc": "docs/GENERATED.md",
                "bindings": [{"file": "t/Case.h", "symbol": "case1", "kind": "verifies"}]}
        },
        "changelog_entries": {"Round 1": {"decision_summary": "the one entry this store holds"}}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(ws.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

/// Every axis ENABLED and every severity pinned to `warn`, so one run emits
/// the whole violation universe and still exits 0 for its JSON to be read.
fn validate(ws: &Path, extra: &[&str]) -> (Output, serde_json::Value) {
    let mut args = vec![
        "validate-code-refs",
        "--json",
        "--severity-missing",
        "warn",
        "--severity-binding",
        "warn",
        "--severity-coverage",
        "warn",
        "--severity-verification",
        "warn",
        "--severity-classification",
        "warn",
        "--severity-blanket",
        "warn",
    ];
    args.extend_from_slice(extra);
    let out = Command::new(cli())
        .args(&args)
        .current_dir(ws)
        .output()
        .expect("cli exec");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout is not json: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out, json)
}

fn violations(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json["violations"]
        .as_array()
        .expect("violations array")
        .clone()
}

fn kinds(vs: &[serde_json::Value]) -> BTreeSet<String> {
    vs.iter()
        .map(|v| v["kind"].as_str().unwrap().to_string())
        .collect()
}

fn not_judged_axes(json: &serde_json::Value) -> BTreeSet<String> {
    json["not_judged"]
        .as_array()
        .expect("not_judged array")
        .iter()
        .map(|e| e["axis"].as_str().expect("axis name").to_string())
        .collect()
}

fn strings(json: &serde_json::Value) -> Vec<String> {
    json.as_array()
        .unwrap_or_else(|| panic!("expected array, got {json}"))
        .iter()
        .map(|v| v.as_str().expect("string").to_string())
        .collect()
}

/// LAW 1 — the scoped answer about a file is the whole run's answer about it.
#[test]
fn a_scoped_run_says_about_its_files_exactly_what_the_whole_run_says() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());

    let (_, full) = validate(tmp.path(), &[]);
    let (_, scoped) = validate(tmp.path(), &["--paths", "src/scoped.rs"]);

    // The partition is the scoped run's own answer about what it did not judge,
    // so this test cannot drift from the implementation's classification. The
    // non-vacuity assertions below are what stop that from being circular.
    let unjudged = not_judged_axes(&scoped);
    let expected: Vec<serde_json::Value> = violations(&full)
        .into_iter()
        .filter(|v| !unjudged.contains(v["kind"].as_str().unwrap()))
        .filter(|v| v["file"].as_str() == Some("src/scoped.rs"))
        .collect();
    let got = violations(&scoped);
    assert_eq!(
        got, expected,
        "scoped answer must equal the whole run's answer restricted to the scope"
    );

    // NON-VACUITY, on both sides of the restriction.
    assert_eq!(
        kinds(&got),
        ["citation_unbound", "missing", "section_missing"]
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>(),
        "the scoped file carries three citation-side defects: {got:#?}"
    );
    let lines: Vec<(String, u64)> = got
        .iter()
        .map(|v| {
            (
                v["kind"].as_str().unwrap().to_string(),
                v["line"].as_u64().unwrap(),
            )
        })
        .collect();
    assert!(
        lines.contains(&("missing".to_string(), 1))
            && lines.contains(&("citation_unbound".to_string(), 2))
            && lines.contains(&("section_missing".to_string(), 3)),
        "each defect is reported at its own line: {lines:?}"
    );
    // Something was left out by the restriction — otherwise the equality above
    // is a statement about the whole tree.
    let full_vs = violations(&full);
    assert!(
        full_vs
            .iter()
            .any(|v| v["file"].as_str() == Some("src/other.rs")),
        "the fixture must carry citation-side defects outside the scope"
    );
    assert!(
        full_vs
            .iter()
            .any(|v| unjudged.contains(v["kind"].as_str().unwrap())),
        "the fixture must carry spec-side violations for the scope to suppress"
    );
}

/// LAW 2 — the axes this mode does not judge are named, and their counts are
/// absent rather than zero.
#[test]
fn a_scoped_run_names_every_axis_it_does_not_judge() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());

    let (_, full) = validate(tmp.path(), &[]);
    let (_, scoped) = validate(tmp.path(), &["--paths", "src/scoped.rs"]);
    let unjudged = not_judged_axes(&scoped);

    // Every spec-side axis the whole run can emit is named as unjudged here.
    let spec_side: BTreeSet<String> = [
        "binding_unbacked",
        "impl_missing",
        "verification_missing",
        "misclassified_coverage",
        "blanket_verifies",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    // ... and each of those really is emitted by the whole run, so the naming
    // is about axes that had something to say.
    let full_kinds = kinds(&violations(&full));
    for axis in &spec_side {
        assert!(
            full_kinds.contains(axis),
            "fixture must make the whole run emit `{axis}`: {full_kinds:?}"
        );
        assert!(
            unjudged.contains(axis),
            "scoped run must name `{axis}` as not judged: {unjudged:?}"
        );
    }
    assert!(
        kinds(&violations(&scoped)).is_disjoint(&spec_side),
        "a scoped run must emit no spec-side violation"
    );

    // A count is null exactly when the axis was not judged. Zero means judged
    // and clean; there is no third reading.
    for axis in &unjudged {
        assert_eq!(
            scoped[format!("{axis}_count")],
            serde_json::Value::Null,
            "`{axis}` was not judged, so its count must be null, not a number"
        );
    }
    for axis in ["missing", "section_missing", "citation_unbound"] {
        assert!(
            scoped[format!("{axis}_count")].is_number(),
            "`{axis}` is judged in this mode and must carry a count"
        );
    }
    // Each naming says WHY, in a value a machine can branch on.
    for entry in scoped["not_judged"].as_array().unwrap() {
        if spec_side.contains(entry["axis"].as_str().unwrap()) {
            assert_eq!(
                entry["reason"], "path_scope",
                "spec-side axes are unjudged because the run was path-scoped: {entry}"
            );
        }
    }
}

/// LAW 3 — a requested path the gate does not read is NAMED, and a scoped run
/// over a clean file reports that it looked.
#[test]
fn a_requested_path_the_gate_never_reads_is_named_rather_than_called_clean() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());

    let (out, json) = validate(
        tmp.path(),
        &["--paths", "src/scoped.rs", "README.md", "src/typo.rs"],
    );
    assert!(
        out.status.success(),
        "warn severities keep this run at exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let scope = &json["path_scope"];
    assert_eq!(
        strings(&scope["requested"]),
        vec!["README.md", "src/scoped.rs", "src/typo.rs"],
        "every requested path is echoed back"
    );
    assert_eq!(strings(&scope["matched_files"]), vec!["src/scoped.rs"]);
    assert_eq!(
        strings(&scope["out_of_read_set"]),
        vec!["README.md"],
        "a file that exists but no configured path covers is its own answer"
    );
    assert_eq!(
        strings(&scope["not_found"]),
        vec!["src/typo.rs"],
        "a path that is not there at all is a different answer again"
    );
    assert_eq!(
        scope["read_set_total"], 3,
        "the narrowing is a number: 3 files would have been read"
    );

    // A clean file inside the scope: no violations, and the report still says
    // the file was read.
    let (clean_out, clean) = validate(tmp.path(), &["--paths", "src/bound.rs"]);
    assert!(clean_out.status.success());
    assert!(
        violations(&clean).is_empty(),
        "src/bound.rs cites the section that binds it"
    );
    assert_eq!(
        strings(&clean["path_scope"]["matched_files"]),
        vec!["src/bound.rs"],
        "silence plus a named matched file is `clean`; silence alone is not"
    );
    // The unscoped run reports no scope at all — the field is not a default.
    let (_, unscoped) = validate(tmp.path(), &[]);
    assert_eq!(unscoped["path_scope"], serde_json::Value::Null);
}

/// LAW 4 — the mode is usable as a commit hook: it rejects on the files it was
/// given and passes when those files are clean, whatever the rest of the tree
/// holds.
#[test]
fn a_scoped_run_rejects_on_its_own_files_and_passes_when_they_are_clean() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let run = |extra: &[&str]| -> Output {
        let mut args = vec!["validate-code-refs"];
        args.extend_from_slice(extra);
        Command::new(cli())
            .args(&args)
            .current_dir(tmp.path())
            .output()
            .expect("cli exec")
    };

    // Default severities: the whole tree rejects (hallucinations, unbound
    // cites, and a coverage gap).
    assert!(
        !run(&[]).status.success(),
        "the fixture tree is not clean under default severities"
    );

    // Scoped to the clean file: exit 0. This is the property the consumer's
    // hook rests on — the spec-side rejects of the whole tree must not fire.
    let clean = run(&["--paths", "src/bound.rs"]);
    assert!(
        clean.status.success(),
        "a clean scope must pass while the tree is dirty; stderr: {}",
        String::from_utf8_lossy(&clean.stderr)
    );

    // Scoped to the dirty file: exit 1, naming the class.
    let dirty = run(&["--paths", "src/scoped.rs"]);
    assert!(!dirty.status.success(), "a dirty scope must reject");
    let stderr = String::from_utf8_lossy(&dirty.stderr);
    assert!(
        stderr.contains("hallucination"),
        "the rejection names the citation-side class it judged: {stderr}"
    );
    // The rejection message is the one line a consumer is guaranteed to read,
    // and the binding class it names has a spec-side member. That member must
    // be named as unjudged rather than printed as a zero — the borrowed zero is
    // no less a lie for being inside an error.
    assert!(
        stderr.contains("binding_unbacked=not judged"),
        "the rejection must not print a measured zero for an axis it skipped: {stderr}"
    );
    assert!(
        !stderr.contains("binding_unbacked=0"),
        "and it must not print the zero in any spelling: {stderr}"
    );
}

/// LAW 5 — the human report says the same thing the JSON does. The consumer
/// reads a terminal, and a mode whose narrowing is only visible in `--json`
/// is a mode whose narrowing is invisible.
#[test]
fn the_text_report_names_the_scope_and_the_axes_it_did_not_judge() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = Command::new(cli())
        .args([
            "validate-code-refs",
            "--severity-missing",
            "warn",
            "--severity-binding",
            "warn",
            "--severity-coverage",
            "warn",
            "--paths",
            "src/scoped.rs",
            "README.md",
        ])
        .current_dir(tmp.path())
        .output()
        .expect("cli exec");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("path scope"),
        "the report names the mode: {stdout}"
    );
    assert!(
        stdout.contains("README.md"),
        "the report names the requested path it does not read: {stdout}"
    );
    for axis in [
        "binding_unbacked",
        "impl_missing",
        "verification_missing",
        "misclassified_coverage",
        "blanket_verifies",
    ] {
        assert!(
            stdout.contains(axis),
            "the report names `{axis}` as not judged: {stdout}"
        );
    }
    // And the counts line does not print a zero for an axis nobody judged.
    assert!(
        !stdout.contains("binding_unbacked=0"),
        "an unjudged axis must not be printed as a measured zero: {stdout}"
    );
}

/// LAW 6 — `--paths` with nothing after it is refused. An empty scope would
/// read as either "everything" or "nothing", and both are silent lies.
#[test]
fn an_empty_scope_is_refused() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = Command::new(cli())
        .args(["validate-code-refs", "--paths"])
        .current_dir(tmp.path())
        .output()
        .expect("cli exec");
    assert!(!out.status.success(), "an empty scope must be refused");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--paths") && !stderr.contains("unknown flag"),
        "the refusal must be about the missing value, not about an unknown \
         flag — this test passed before the flag existed: {stderr}"
    );
}
