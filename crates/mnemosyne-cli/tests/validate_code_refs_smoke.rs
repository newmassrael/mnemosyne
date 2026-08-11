//! Round 256-260 — `validate-code-refs` subcommand smoke tests.
//!
//! Test scope:
//! (i) `[plugins.set_equality_validator]` omission → skip mode (exit 0 with explicit log line)
//! (ii) clean codebase (citation present in atomic store) → no violations
//! (iii) hallucinated citation → reject (exit 1) under default severity
//! (iv) hallucinated citation → warn (exit 0) under `--severity-missing warn`
//! (v) identifier-shaped incidental hits (`TestRound254Helper`,
//!  `round_254_helper`) → not flagged (word-boundary carve-out)
//! (vi) JSON output shape (Round 256 fields)
//! (vii) `--filter-id` decay scan (Round 258)
//! (viii) Round 260 — `§<id>` hallucination → SectionMissing (reject)
//! (ix) Round 260 — `§<id>` cite without matching impl entry → CitationUnbound
//! (x) Round 260 — impl entry without code cite → BindingUnbacked
//! (xi) Round 260 — `--severity-binding warn` keeps exit 0 on binding violations

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// Set up a minimal workspace with one ChangelogEntry (`Round 1`) and
/// optionally a `[plugins.set_equality_validator]` table pointing at `src/`.
fn write_workspace(workspace: &Path, with_code_refs: bool) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::create_dir_all(workspace.join("src")).unwrap();
    let mut cfg = String::from(
        "[workspace]\n\
 [schema]\nentry_id_prefix = \"Round \"\n",
    );
    if with_code_refs {
        cfg.push_str("[plugins.set_equality_validator]\npaths = [\"src/\"]\n");
    }
    fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();

    // Atomic store with one valid entry: "Round 1".
    let atomic = serde_json::json!({
    "schema_version": 1,
    "sections": {},
    "changelog_entries": {
    "Round 1": {
    "decision_summary": "test entry"
    }
    }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    // GENERATED.md stub so default_doc resolves; subcommand doesn't need
    // workspace doc validation but loading config wants it to exist.
    fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

fn run_cli(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

#[test]
fn case_i_skip_mode_when_code_refs_unconfigured() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), false);

    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("skipped") && stdout.contains("[plugins.set_equality_validator]"),
        "stdout: {}",
        stdout
    );
}

#[test]
fn case_ii_clean_codebase_no_violations() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::write(
        tmp.path().join("src/lib.rs"),
        "// Round 1 — test entry implementation\nfn main() {}\n",
    )
    .unwrap();

    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        out.status.success(),
        "exit code: {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("missing=0"), "stdout: {}", stdout);
}

#[test]
fn case_iii_hallucinated_citation_rejected() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::write(
        tmp.path().join("src/lib.rs"),
        "// see Round 999 for hallucinated reference\nfn main() {}\n",
    )
    .unwrap();

    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        !out.status.success(),
        "expected reject; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Round 999") || stderr.contains("missing"),
        "stderr should mention the missing citation; got: {}",
        stderr
    );
}

#[test]
fn case_iv_hallucinated_warn_severity_exits_zero() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::write(
        tmp.path().join("src/lib.rs"),
        "// see Round 999 for hallucinated reference\nfn main() {}\n",
    )
    .unwrap();

    let out = run_cli(
        tmp.path(),
        &["validate-code-refs", "--severity-missing", "warn"],
    );
    assert!(
        out.status.success(),
        "warn severity should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Round 999"),
        "stdout should list the violation; got: {}",
        stdout
    );
}

#[test]
fn case_v_identifier_shaped_incidental_hits_not_flagged() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::write(
        tmp.path().join("src/lib.rs"),
        "struct TestRound254Helper;\n\
 fn round_254_helper() {}\n\
 // Round 1 — actual citation\n",
    )
    .unwrap();

    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        out.status.success(),
        "identifier-shaped hits should not be citations; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("missing=0"), "stdout: {}", stdout);
}

#[test]
fn case_vii_filter_id_surfaces_decay_and_skips_others() {
    // Round 258 — when --filter-id is set, citations matching it are
    // reported as decay; non-matching citations (even if missing) are
    // suppressed. This is the cascade caller's read mode after a
    // supersede mutate.
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::write(
        tmp.path().join("src/lib.rs"),
        "// Round 1 still in atomic but flagged decay\n\
 // Round 999 hallucinated, but filter excludes\n\
 // Round 1 second occurrence\n",
    )
    .unwrap();

    let out = run_cli(
        tmp.path(),
        &["validate-code-refs", "--filter-id", "Round 1", "--json"],
    );
    assert!(
        out.status.success(),
        "filter-id mode does not reject; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    assert_eq!(parsed["filter_id"], "Round 1");
    assert_eq!(parsed["decay_count"], 2);
    // `Round 999` really is missing, and this mode did not look — so the run
    // publishes no `missing` count and NAMES the axis. The assertion here used
    // to be `missing_count == 0`, which is the same number a run that looked
    // and found nothing prints: the mode's whole suppression was invisible.
    assert_eq!(
        parsed["missing_count"],
        serde_json::Value::Null,
        "decay mode judges the decay axis alone: {parsed}"
    );
    let named: Vec<&str> = parsed["not_judged"]
        .as_array()
        .expect("not_judged array")
        .iter()
        .filter(|e| e["reason"] == "decay_filter")
        .map(|e| e["axis"].as_str().expect("axis name"))
        .collect();
    assert!(
        named.contains(&"missing") && named.contains(&"citation_unbound") && named.len() >= 10,
        "every axis but decay is named as suppressed by the filter: {named:?}"
    );
    let violations = parsed["violations"].as_array().expect("violations array");
    assert_eq!(violations.len(), 2);
    for v in violations {
        assert_eq!(v["kind"], "decay");
        assert_eq!(v["entry_id"], "Round 1");
    }
}

#[test]
fn case_vi_json_output_shape() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::write(
        tmp.path().join("src/lib.rs"),
        "// Round 999 missing\n// Round 1 ok\n",
    )
    .unwrap();

    let out = run_cli(
        tmp.path(),
        &["validate-code-refs", "--severity-missing", "warn", "--json"],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["primitive"], "validate-code-refs");
    assert_eq!(parsed["missing_count"], 1);
    // Round 260 — new count fields present in JSON shape.
    assert!(parsed.get("section_missing_count").is_some());
    assert!(parsed.get("citation_unbound_count").is_some());
    assert!(parsed.get("binding_unbacked_count").is_some());
    assert!(parsed.get("severity_binding").is_some());
    let violations = parsed["violations"].as_array().expect("violations array");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["entry_id"], "Round 999");
    assert_eq!(violations[0]["kind"], "missing");
}

// ============ Round 260 — Path B bidirectional smoke tests ============

/// Write a minimal workspace whose atomic store includes one §<id>
/// section with optional `implementations` entries. `with_code_refs` adds
/// `[plugins.set_equality_validator] paths = ["src/"]`.
fn write_workspace_with_section(
    workspace: &Path,
    with_code_refs: bool,
    section_id: &str,
    impls: &[(&str, Option<&str>)],
) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::create_dir_all(workspace.join("src")).unwrap();
    let mut cfg = String::from(
        "[workspace]\n\
 [schema]\nentry_id_prefix = \"Round \"\n",
    );
    if with_code_refs {
        cfg.push_str("[plugins.set_equality_validator]\npaths = [\"src/\"]\n");
    }
    fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();
    let impls_json: Vec<_> = impls
        .iter()
        .map(|(f, s)| match s {
            Some(sym) => serde_json::json!({ "file": f, "symbol": sym }),
            None => serde_json::json!({ "file": f }),
        })
        .collect();
    let mut sections = serde_json::Map::new();
    sections.insert(
        section_id.to_string(),
        serde_json::json!({ "implementations": impls_json }),
    );
    let atomic = serde_json::json!({
    "schema_version": 1,
    "sections": sections,
    "changelog_entries": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

#[test]
fn case_viii_section_missing_rejected_under_default_severity() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::write(
        tmp.path().join("src/lib.rs"),
        "// see §999 hallucinated section ref\nfn main() {}\n",
    )
    .unwrap();
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        !out.status.success(),
        "expected reject; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("SectionMissing")
            || stderr.contains("section_missing")
            || stderr.contains("hallucination"),
        "stderr should mention SectionMissing class; got: {}",
        stderr
    );
}

#[test]
fn case_ix_citation_unbound_rejected_under_default_binding_severity() {
    // §39.bindings = [src/bar.rs] but src/foo.rs cites §39.
    let tmp = TempDir::new().unwrap();
    write_workspace_with_section(tmp.path(), true, "39", &[("src/bar.rs", None)]);
    fs::write(
        tmp.path().join("src/foo.rs"),
        "// §39 cite from unregistered file\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("src/bar.rs"),
        "// §39 cite from authoritative file\n",
    )
    .unwrap();
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        !out.status.success(),
        "expected reject; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("binding") || stderr.contains("CitationUnbound"),
        "stderr should mention binding class; got: {}",
        stderr
    );
}

#[test]
fn case_x_implementation_unbacked_rejected_under_default_binding_severity() {
    // §39.bindings = [src/foo.rs] but src/foo.rs has no §39 cite.
    let tmp = TempDir::new().unwrap();
    write_workspace_with_section(tmp.path(), true, "39", &[("src/foo.rs", Some("Foo"))]);
    fs::write(
        tmp.path().join("src/foo.rs"),
        "// no spec citation\nfn foo() {}\n",
    )
    .unwrap();
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        !out.status.success(),
        "expected reject; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("binding") || stderr.contains("BindingUnbacked"),
        "stderr should mention binding class; got: {}",
        stderr
    );
}

#[test]
fn case_xi_severity_binding_warn_keeps_exit_zero() {
    let tmp = TempDir::new().unwrap();
    write_workspace_with_section(tmp.path(), true, "39", &[("src/foo.rs", None)]);
    fs::write(tmp.path().join("src/foo.rs"), "// no cite\n").unwrap();
    let out = run_cli(
        tmp.path(),
        &["validate-code-refs", "--severity-binding", "warn", "--json"],
    );
    assert!(
        out.status.success(),
        "warn severity should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    assert_eq!(parsed["binding_unbacked_count"], 1);
    assert_eq!(parsed["severity_binding"], "warn");
    let violations = parsed["violations"].as_array().expect("violations array");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["kind"], "binding_unbacked");
    assert_eq!(violations[0]["section_id"], "39");
}

#[test]
fn case_xii_coverage_split_downgrades_independently_of_binding() {
    // Round 385 — §39 exists (Active) with ZERO implementations =
    // ImplementationMissing (coverage class). `--severity-coverage warn`
    // downgrades it while `--severity-binding` stays reject (default): the
    // split makes this possible — previously impl_missing was in the binding
    // bucket and could not be downgraded without also downgrading binding.
    let tmp = TempDir::new().unwrap();
    write_workspace_with_section(tmp.path(), true, "39", &[]);
    fs::write(tmp.path().join("src/foo.rs"), "// no cite\n").unwrap();
    let out = run_cli(
        tmp.path(),
        &[
            "validate-code-refs",
            "--severity-coverage",
            "warn",
            "--json",
        ],
    );
    assert!(
        out.status.success(),
        "coverage=warn must exit 0 even with binding=reject; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    assert_eq!(parsed["impl_missing_count"], 1);
    assert_eq!(parsed["severity_binding"], "reject");
    assert_eq!(parsed["severity_coverage"], "warn");
}

#[test]
fn case_xiii_coverage_inherits_binding_and_rejects_as_coverage_class() {
    // Round 385 — no severity_coverage set: it inherits severity_binding
    // (reject by default), so §39 with zero implementations still rejects —
    // but as a *coverage*-class violation, not binding (proving the move).
    let tmp = TempDir::new().unwrap();
    write_workspace_with_section(tmp.path(), true, "39", &[]);
    fs::write(tmp.path().join("src/foo.rs"), "// no cite\n").unwrap();
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        !out.status.success(),
        "coverage inherits binding=reject → must reject; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // The axis is named by its kind tag, the same string the JSON and the
    // `not_judged` list use — the rejection message used to spell a variant
    // name of its own, and a class whose members are not all judged can now say
    // so here instead of printing a zero for one of them.
    assert!(
        stderr.contains("coverage-class") && stderr.contains("impl_missing=1"),
        "must reject as coverage-class, not binding; got: {}",
        stderr
    );
}

/// Round 855 — a `[plugins.symbol_resolver.<lang>]` entry that cannot be built
/// is a config error, not a warning and not a silence.
///
/// Two shapes of the same rule, asserted together because fixing one and
/// leaving the other is the half-enforcement this project treats as no
/// enforcement. A key naming a language no extension maps to (`c`, the obvious
/// workaround for a `.c` tree taking no symbol binding) used to parse,
/// register, and never be consulted, with no diagnostic at all. A key naming a
/// backend this build has no plugin for printed to stderr and continued. Both
/// leave `severity_binding = reject` reading as symbol-level enforcement while
/// the run performs file-level.
#[test]
fn case_xiv_an_unbuildable_symbol_resolver_entry_is_refused() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::write(tmp.path().join("src/foo.rs"), "// Round 1\n").unwrap();
    let base = fs::read_to_string(tmp.path().join("mnemosyne.toml")).unwrap();

    // The language key `c` — `.c` maps to the `cpp` resolver, never to `c`.
    fs::write(
        tmp.path().join("mnemosyne.toml"),
        format!(
            "{base}[plugins.symbol_resolver.c]\n\
             transport = \"in-process\"\nbackend = \"tree-sitter-cpp\"\n"
        ),
    )
    .unwrap();
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a resolver nothing can consult must refuse; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        stderr.contains("symbol_resolver.c") && stderr.contains("cpp"),
        "the refusal must name the dead key AND the keys that work: {stderr}"
    );

    // Same file, legal language, backend this build has no plugin for.
    fs::write(
        tmp.path().join("mnemosyne.toml"),
        format!(
            "{base}[plugins.symbol_resolver.cpp]\n\
             transport = \"in-process\"\nbackend = \"tree-sitter-kotlin\"\n"
        ),
    )
    .unwrap();
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a backend with no plugin must refuse rather than degrade to file-level"
    );
    assert!(
        stderr.contains("tree-sitter-kotlin"),
        "the refusal must name the backend it cannot build: {stderr}"
    );

    // CONTROL: the same config with a buildable entry passes, so the two
    // refusals above are about the entries and not about the fixture.
    fs::write(
        tmp.path().join("mnemosyne.toml"),
        format!(
            "{base}[plugins.symbol_resolver.rust]\n\
             transport = \"in-process\"\nbackend = \"tree-sitter-rust\"\n"
        ),
    )
    .unwrap();
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        out.status.success(),
        "a buildable resolver must be accepted; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Round 1080 — a tree named in `paths` is read whatever language it holds, and
/// a tree not named there is not read at all.
///
/// This is the capability the gate's reach rests on. `.githooks/` was in `paths`
/// and the two trees those hooks CALL were not: `scripts/`, which holds the gate
/// scripts they delegate to, and `.github/`, which holds the CI running the same
/// checks. Twenty distinct rounds were cited across the two and nothing read
/// them; one of the twenty resolved to nothing, and a second — two digits, which
/// a hand-written pattern had missed — was found by the gate itself the moment
/// the trees were enrolled.
///
/// Both directions, because a path entry that changes no answer is config that
/// looks like it works (the Round 860 shape).
#[test]
fn case_xv_a_non_rust_tree_is_read_when_paths_names_it_and_not_when_it_does_not() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    fs::create_dir_all(tmp.path().join("ci")).unwrap();
    fs::write(
        tmp.path().join("ci/pipeline.yml"),
        "# Round 4242 — a round this store does not have.\nname: ci\n",
    )
    .unwrap();

    // NOT enrolled: `paths = ["src/"]` from the fixture. The citation is there
    // and nothing reads it.
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        out.status.success(),
        "an unenrolled tree must not be read; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    // Enrolled: the same bytes, now inside the gate's reach.
    fs::write(
        tmp.path().join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n\
         [plugins.set_equality_validator]\npaths = [\"src/\", \"ci/\"]\n\
         scan_exclusions = [\"docs/\"]\n",
    )
    .unwrap();
    let out = run_cli(tmp.path(), &["validate-code-refs"]);
    assert!(
        !out.status.success(),
        "a citation naming no entry must be rejected once its tree is enrolled; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Round 4242") && stdout.contains("ci/pipeline.yml"),
        "the rejection must name the citation and the file it is in: {stdout}"
    );
}
