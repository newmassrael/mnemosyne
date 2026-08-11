//! The strings a commit hook actually produces, put to `--paths`.
//!
//! Round 1141 built `validate-code-refs --paths <file>...` for a consumer whose
//! hook hands it the files a commit touched, and pinned what the mode MEANS:
//! the scoped answer equals the whole run's answer restricted to those files,
//! and the axes it cannot judge are named. Six laws state that meaning.
//!
//! NONE OF THEM IS ABOUT THE ARGUMENT. `PathScope::new` accepts an absolute
//! path, strips a `./` prefix, refuses a path outside the tree, refuses an empty
//! string and refuses one naming the workspace root; `selects` takes a directory
//! as everything under it. All of that is promised to the consumer in the reply
//! they have not yet been sent, and exactly one of those branches — the empty
//! LIST — had a test. The rest is code with no reader, on the surface a hook
//! generates mechanically and never by hand.
//!
//! The oracle here is EQUALITY between spellings, not acceptance. A run that
//! takes `./src/a.rs` and quietly selects nothing also "accepts" it, exits 0,
//! and reports a clean tree — the failure this whole mode was built to stop.
//! So each spelling of one file is required to produce the same answer as the
//! plain relative one, and each refusal is required to name what it refused.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// Three citing files, one of them a directory level down, and one file outside
/// the configured `paths` — so "the whole read set" and "the whole tree" are
/// different sets and a scope naming a directory has something to leave out.
fn write_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::create_dir_all(ws.join("src/deep")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n\
         [plugins.set_equality_validator]\npaths = [\"src/\"]\n",
    )
    .unwrap();
    fs::write(
        ws.join("src/a.rs"),
        "// Round 999 — an entry this store does not hold\n",
    )
    .unwrap();
    fs::write(
        ws.join("src/b.rs"),
        "// Round 998 — a second entry this store does not hold\n",
    )
    .unwrap();
    fs::write(
        ws.join("src/deep/c.rs"),
        "// Round 997 — a third, one directory down\n",
    )
    .unwrap();
    fs::write(ws.join("README.md"), "Round 996 — never read\n").unwrap();

    let atomic = serde_json::json!({
        "schema_version": 11,
        "sections": {},
        "changelog_entries": {"Round 1": {"decision_summary": "the one entry this store holds"}}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(ws.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

/// `severity_missing = warn` so a fixture full of hallucinated rounds still
/// exits 0 — the exit code is then free to mean what a REFUSAL means.
fn validate(ws: &Path, extra: &[&str]) -> (Output, serde_json::Value) {
    let mut args = vec!["validate-code-refs", "--json", "--severity-missing", "warn"];
    args.extend_from_slice(extra);
    let out = Command::new(cli())
        .args(&args)
        .current_dir(ws)
        .output()
        .expect("cli exec");
    // A refusal prints no JSON at all; the tests that expect one say so through
    // the exit code first.
    let json = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);
    (out, json)
}

/// What a run judged and what it found — the whole of the answer these tests
/// compare between spellings.
fn answer(json: &serde_json::Value) -> (serde_json::Value, serde_json::Value) {
    (
        json["path_scope"]["matched_files"].clone(),
        json["violations"].clone(),
    )
}

/// LAW 1 — every spelling of one file is the same file.
///
/// `git diff --name-only` gives `src/a.rs`; a hook that resolves it gives an
/// absolute path; one that walks with `find` gives `./src/a.rs`. All three are
/// the file, and the answer must not depend on which the caller had to hand.
#[test]
fn an_absolute_path_and_a_dot_slash_prefix_name_the_same_file_as_the_plain_one() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let root = fs::canonicalize(tmp.path()).unwrap();
    let absolute = root.join("src/a.rs").display().to_string();

    let (plain_out, plain) = validate(tmp.path(), &["--paths", "src/a.rs"]);
    assert!(
        plain_out.status.success(),
        "the control spelling must run: {}",
        String::from_utf8_lossy(&plain_out.stderr)
    );
    // NON-VACUITY: the control judged one file and found the defect in it, so
    // an equal answer below is equal to something.
    assert_eq!(
        answer(&plain).0,
        serde_json::json!(["src/a.rs"]),
        "the control judged exactly the named file: {plain}"
    );
    assert_eq!(
        plain["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .filter(|v| v["kind"] == "missing")
            .count(),
        1,
        "and found the citation in it: {plain}"
    );

    for spelling in [absolute.as_str(), "./src/a.rs"] {
        let (out, json) = validate(tmp.path(), &["--paths", spelling]);
        assert!(
            out.status.success(),
            "`{spelling}` must be accepted: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            answer(&json),
            answer(&plain),
            "`{spelling}` must produce the SAME answer as `src/a.rs`, not merely \
             be accepted — a spelling that selects nothing also exits 0"
        );
    }
}

/// LAW 2 — a directory is everything under it, one level or many.
#[test]
fn a_directory_is_every_file_under_it() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());

    // A nested directory: its one file, and not the two beside it.
    let (out, deep) = validate(tmp.path(), &["--paths", "src/deep"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        answer(&deep).0,
        serde_json::json!(["src/deep/c.rs"]),
        "a directory selects what is under it and nothing else: {deep}"
    );

    // The configured root of the read set: every file the gate reads, so the
    // citation-side answer is the whole run's, which is the equality law of
    // this mode arriving through a directory rather than a file list.
    let (_, whole) = validate(tmp.path(), &[]);
    let (_, all) = validate(tmp.path(), &["--paths", "src/"]);
    assert_eq!(
        answer(&all).0,
        serde_json::json!(["src/a.rs", "src/b.rs", "src/deep/c.rs"]),
        "the configured directory selects the whole read set: {all}"
    );
    assert_eq!(
        all["violations"], whole["violations"],
        "scoping to everything must answer exactly what the unscoped run \
         answers — this fixture has no spec-side violation to be suppressed"
    );
}

/// LAW 3 — a path this tree does not contain is refused, by name.
#[test]
fn a_path_outside_the_workspace_is_refused_and_named() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let outside = TempDir::new().unwrap();
    let stranger = outside.path().join("src/a.rs").display().to_string();

    let (out, _) = validate(tmp.path(), &["--paths", &stranger]);
    assert!(
        !out.status.success(),
        "a path outside the tree must be refused, not silently matched by \
         nothing: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&stranger) && stderr.contains("is outside the workspace"),
        "the refusal must name the path AND say which tree it is outside — a \
         hook handing a path from a sibling checkout needs both: {stderr}"
    );
}

/// LAW 4 — a scope that is not a narrowing is refused, however it is spelled.
///
/// The empty LIST is refused because "everything" and "nothing" are both silent
/// readings of it. A single argument naming the workspace root is the same
/// question asked with one more character, and `.` is the spelling a hook
/// produces by accident — `dirname` of a top-level file, a `${dir:-.}` default,
/// `find . -name '*.rs' -printf '%h\n'`.
#[test]
fn a_scope_that_narrows_nothing_is_refused_in_every_spelling() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let root = fs::canonicalize(tmp.path()).unwrap();

    for spelling in [".", "./", root.display().to_string().as_str()] {
        let (out, _) = validate(tmp.path(), &["--paths", spelling]);
        assert!(
            !out.status.success(),
            "`--paths {spelling}` names the whole tree, which is not a \
             narrowing — it must be refused rather than run as a scope: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("names the workspace root, which is not a narrowing"),
            "`{spelling}` must be refused for BEING THE WHOLE TREE — any other \
             refusal would pass this test while leaving the hole open: {stderr}"
        );
        assert!(
            stderr.contains(spelling),
            "and the refusal must quote what it was handed: {stderr}"
        );
    }
}

/// LAW 5 — an empty argument is refused.
#[test]
fn an_empty_path_argument_is_refused() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let (out, _) = validate(tmp.path(), &["--paths", ""]);
    assert!(
        !out.status.success(),
        "an empty path is not a file: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--paths was given an empty path"),
        "the refusal must be about the argument, not about the flag being \
         unknown or its value missing: {stderr}"
    );
}

/// LAW 6 — many files, however the caller spells "many", and the flag stops at
/// the next one.
///
/// The reply promises three things in one sentence: `--paths` is variadic, it
/// may be repeated, and it consumes up to the next `--` flag. The third is what
/// makes the first safe — a greedy list would swallow `--severity-missing` and
/// its value, and the run would then be scoped to two paths that are not files
/// and severities that were never applied.
#[test]
fn a_file_list_is_variadic_repeatable_and_stops_at_the_next_flag() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());

    let run = |args: &[&str]| -> (Output, serde_json::Value) {
        let mut argv = vec!["validate-code-refs", "--json"];
        argv.extend_from_slice(args);
        let out = Command::new(cli())
            .args(&argv)
            .current_dir(tmp.path())
            .output()
            .expect("cli exec");
        let json = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);
        (out, json)
    };

    // Variadic, with a flag AFTER the list: the severity must land, which is
    // only observable because this fixture's citations are all hallucinated.
    let (variadic_out, variadic) = run(&[
        "--paths",
        "src/a.rs",
        "src/b.rs",
        "--severity-missing",
        "warn",
    ]);
    assert!(
        variadic_out.status.success(),
        "the flag after the list must be read as a flag, not eaten as a path: {}",
        String::from_utf8_lossy(&variadic_out.stderr)
    );
    assert_eq!(
        answer(&variadic).0,
        serde_json::json!(["src/a.rs", "src/b.rs"]),
        "both named files are judged: {variadic}"
    );
    assert_eq!(
        variadic["violations"].as_array().expect("violations").len(),
        2,
        "and the scope is not decoration — each file carries one: {variadic}"
    );

    // Repeated, same two files, same answer.
    let (_, repeated) = run(&[
        "--severity-missing",
        "warn",
        "--paths",
        "src/a.rs",
        "--paths",
        "src/b.rs",
    ]);
    assert_eq!(
        answer(&repeated),
        answer(&variadic),
        "a repeated flag and one list must be the same scope"
    );
}

/// LAW 7 — the two narrowings compose: `--filter-id` picks the axis, `--paths`
/// picks the files.
///
/// The reply promises this combination, and the two narrowings are applied at
/// different places — the axis by the verdict map, the files by the read set —
/// so nothing about either one implies the other works.
#[test]
fn a_decay_filter_and_a_file_list_narrow_different_things() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());

    // `Round 999` is cited by src/a.rs alone.
    let (_, hit) = validate(
        tmp.path(),
        &["--filter-id", "Round 999", "--paths", "src/a.rs"],
    );
    let (_, miss) = validate(
        tmp.path(),
        &["--filter-id", "Round 999", "--paths", "src/b.rs"],
    );

    assert_eq!(
        hit["decay_count"],
        serde_json::json!(1),
        "the decay axis is judged, over the file the scope named: {hit}"
    );
    assert_eq!(
        miss["decay_count"],
        serde_json::json!(0),
        "and the same axis over a file that does not cite it is a measured \
         zero, not a null: {miss}"
    );
    assert_eq!(
        miss["missing_count"],
        serde_json::Value::Null,
        "the decay filter still un-judges the other axes — the file list did \
         not put them back: {miss}"
    );
}
