//! An XML document cites the inventory ids its declared annotation attribute
//! lists — read AS XML, through the real `validate-code-refs`.
//!
//! Round 1322 read the adopter's annotation as a prefix followed by a character
//! class and so read the first id of a list; Round 1323 read it as text between
//! two delimiters, a second grammar for an attribute the document's own XML
//! already defines. The axis now parses the document, so the attribute is found
//! by its namespace under any prefix, its value is decoded as XML decodes it, a
//! comment, another namespace and a Markdown example are not citations, and a
//! document that does not parse is reported with the parser's reason.

use serde_json::Value;
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

/// The declaration every attribute test here uses: `req` in
/// `http://example/ext`, read in `.scxml` documents.
const REQ_ATTRIBUTE: &str = r#"inventory_attributes = [{ namespace = "http://example/ext", name = "req", extensions = ["scxml"] }]"#;

/// A workspace declaring `axis`, with each `(name, text)` of `documents`
/// written under `doc/`, and one active entry (`REQ-4.2.1`) and one deprecated
/// entry (`REQ-7`) registered.
fn workspace(axis: &str, documents: &[(&str, &str)]) -> TempDir {
    let ws = TempDir::new().expect("tempdir");
    fs::write(
        ws.path().join("mnemosyne.toml"),
        format!(
            "[workspace]\n\n[plugins.set_equality_validator]\npaths = [\"doc/\"]\n\
             {axis}\nseverity_inventory = \"reject\"\n"
        ),
    )
    .expect("config");
    for args in [
        &[
            "add-inventory-entry",
            "--id",
            "REQ-4.2.1",
            "--status",
            "active",
        ][..],
        &[
            "add-inventory-entry",
            "--id",
            "REQ-7",
            "--status",
            "deprecated",
            "--reason",
            "superseded",
        ][..],
    ] {
        let out = run(ws.path(), args);
        assert!(
            out.status.success(),
            "fixture: `{}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fs::create_dir_all(ws.path().join("doc")).expect("doc dir");
    for (name, text) in documents {
        fs::write(ws.path().join("doc").join(name), text).expect("document");
    }
    ws
}

/// Whether the gate passed, and the JSON report it printed.
fn report(workspace: &Path) -> (bool, Value) {
    let out = run(workspace, &["validate-code-refs", "--json"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let report: Value = stdout
        .lines()
        .find(|line| line.starts_with('{'))
        .and_then(|line| serde_json::from_str(line).ok())
        .unwrap_or_else(|| {
            panic!(
                "the gate prints one JSON report: stdout {stdout} stderr {}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    (out.status.success(), report)
}

/// Every violation in `report` as `(kind, entry_id, line)`, sorted.
fn violations(report: &Value) -> Vec<(String, String, u64)> {
    let mut found: Vec<(String, String, u64)> = report["violations"]
        .as_array()
        .expect("a violations array")
        .iter()
        .map(|v| {
            (
                v["kind"].as_str().expect("a kind").to_string(),
                v["entry_id"].as_str().expect("an entry_id").to_string(),
                v["line"].as_u64().expect("a line"),
            )
        })
        .collect();
    found.sort();
    found
}

/// The deprecated id is written through a character reference under a prefix
/// other than the adopter's, the missing id sits on the line after its
/// attribute opens, and the same ids in another namespace, in a comment and in
/// a Markdown example are not citations.
#[test]
fn an_attribute_cites_every_id_its_value_lists_as_xml_reads_it() {
    let ws = workspace(
        REQ_ATTRIBUTE,
        &[
            (
                "model.scxml",
                "<scxml xmlns:x=\"http://example/ext\">\n\
                 <state id=\"idle\" x:req=\"REQ-4.2.1 REQ&#45;7\"/>\n\
                 <state id=\"new\" x:req='\n\
                 REQ-999'/>\n\
                 <state xmlns:y=\"urn:other\" y:req=\"REQ-1000\"/>\n\
                 <!-- <state x:req=\"REQ-1001\"/> -->\n\
                 </scxml>\n",
            ),
            (
                "README.md",
                "A model annotates <state x:req=\"REQ-1002\"/>.\n",
            ),
        ],
    );
    let (passed, report) = report(ws.path());
    let found = violations(&report);
    assert!(
        !passed,
        "a deprecated and a missing citation at reject severity fail the gate: {found:?}"
    );
    assert_eq!(
        found,
        vec![
            ("inventory_deprecated".to_string(), "REQ-7".to_string(), 2),
            ("inventory_missing".to_string(), "REQ-999".to_string(), 4),
        ],
        "the decoded id and the id on the line after its attribute are reported where \
         they stand, and nothing another namespace, a comment or a Markdown file holds is"
    );
}

/// A declared document that does not parse is reported for the attribute it
/// was to be read for, with the parser's reason — and the deprecated id inside
/// it is not, because nothing in an unreadable document was read.
#[test]
fn a_document_that_does_not_parse_is_reported_with_the_reason() {
    let ws = workspace(
        REQ_ATTRIBUTE,
        &[(
            "model.scxml",
            "<scxml xmlns:x=\"http://example/ext\">\n<state x:req=\"REQ-7\">\n</scxml>\n",
        )],
    );
    let (passed, report) = report(ws.path());
    let found = violations(&report);
    assert!(
        !passed,
        "an unreadable document at reject severity fails the gate: {found:?}"
    );
    assert_eq!(
        found
            .iter()
            .map(|(kind, entry, _)| (kind.as_str(), entry.as_str()))
            .collect::<Vec<_>>(),
        vec![(
            "inventory_document_unreadable",
            "attribute {http://example/ext}req"
        )],
        "the document is reported for its attribute, not read as citing nothing"
    );
    assert!(
        report["violations"][0]["parse_error"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty()),
        "the report says why the document did not parse: {report}"
    );
}

/// Round 1326 — the adopter writes the annotation as element text instead of an
/// attribute: nothing is cited, so nothing is violated and the gate passes. The
/// REPORT is what must not be silent, because a run that judged nothing prints
/// what a run that found everything in order prints.
#[test]
fn an_attribute_no_document_carries_is_reported_rather_than_silent() {
    let ws = workspace(
        REQ_ATTRIBUTE,
        &[(
            "model.scxml",
            "<scxml xmlns:x=\"http://example/ext\">\n\
             <state><req>REQ-4.2.1</req></state>\n\
             </scxml>\n",
        )],
    );
    let (passed, report) = report(ws.path());
    assert!(passed, "nothing is cited, so nothing is violated: {report}");
    assert_eq!(violations(&report), vec![]);

    let axis = &report["inventory_reader_axis"][0];
    assert_eq!(
        (
            axis["reader"].as_str(),
            axis["documents"].as_u64(),
            axis["carrying"].as_u64(),
            axis["citations"].as_u64(),
        ),
        (
            Some("attribute {http://example/ext}req"),
            Some(1),
            Some(0),
            Some(0)
        ),
        "the report must say the document was read and carries none of it: {report}"
    );
    assert_eq!(
        axis["bare"][0].as_str(),
        Some("doc/model.scxml"),
        "and it must NAME the document, not only count it: {report}"
    );

    let plain = run(ws.path(), &["validate-code-refs"]);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&plain.stdout),
        String::from_utf8_lossy(&plain.stderr)
    );
    assert!(
        text.contains("NO DOCUMENT CARRIES IT") && text.contains("doc/model.scxml"),
        "the line a person reads must carry it too, document named: {text}"
    );
}

/// Round 1328 — THE PORT'S SECOND READER, DECLARED AND RUNNING: the adopter
/// writes the annotation as element text and declares `inventory_elements`, and
/// the same deprecated id is reported that an attribute would have carried. The
/// shape Round 1325 recorded as "not read" costs one reader behind the port.
#[test]
fn an_element_declaration_reads_the_annotation_the_attribute_reader_cannot() {
    let ws = workspace(
        r#"inventory_elements = [{ namespace = "http://example/ext", name = "req", extensions = ["scxml"] }]"#,
        &[(
            "model.scxml",
            "<scxml xmlns:x=\"http://example/ext\">\n\
             <state><x:req>REQ-7</x:req></state>\n\
             </scxml>\n",
        )],
    );
    let (passed, report) = report(ws.path());
    let found = violations(&report);
    assert!(
        !passed,
        "a deprecated citation at reject severity fails the gate: {found:?}"
    );
    assert_eq!(
        found,
        vec![("inventory_deprecated".to_string(), "REQ-7".to_string(), 2)],
        "the element's text is a citation on the line it stands: {report}"
    );
    let axis = &report["inventory_reader_axis"][0];
    assert_eq!(
        (axis["reader"].as_str(), axis["carrying"].as_u64()),
        (Some("element {http://example/ext}req"), Some(1)),
        "and the reach report names the reader that read it: {report}"
    );
}

/// Round 1331 — A PROGRAM THE WORKSPACE DECLARES IS THE READER, end to end: the
/// gate runs it over a document no built-in reader can read, takes the ids it
/// names, and judges them against the store. This is the arc's point made
/// literal — an adopter's own parser can be what reads their documents.
#[test]
fn a_declared_program_is_the_reader_through_the_binary() {
    let ws = workspace(
        r#"citation_readers = [{ name = "fixture", transport = "cli", command = ["sh", "-c", "printf '{\"cites\":[{\"line\":2,\"id\":\"REQ-7\"}]}'"], extensions = ["scxml"] }]"#,
        &[(
            "model.scxml",
            "a document in no format this gate knows\nREQ-7 is cited on this line\n",
        )],
    );
    let (passed, report) = report(ws.path());
    let found = violations(&report);
    assert!(
        !passed,
        "the program named a deprecated id, and reject severity fails the gate: {found:?}"
    );
    assert_eq!(
        found,
        vec![("inventory_deprecated".to_string(), "REQ-7".to_string(), 2)],
        "the ids are the program's and the verdict is the gate's: {report}"
    );
    let axis = &report["inventory_reader_axis"][0];
    assert_eq!(
        (axis["reader"].as_str(), axis["citations"].as_u64()),
        (Some("program fixture"), Some(1)),
        "and the reach report names the program that read it: {report}"
    );
}

/// Round 1334 — A READER THAT NEVER READS ITS INPUT MUST NOT KILL THE GATE.
///
/// The fixture above is such a reader — `printf` answers from nothing — and
/// Round 1331 handed it the document on a PIPE. `mnemosyne-cli` restores
/// `SIG_DFL` for `SIGPIPE` so a piped report dies like `cat` (Round 859), so the
/// gate's own write to a reader that had already exited killed the gate: empty
/// stdout, empty stderr, and a test whose only evidence was that the binary said
/// nothing. It was a RACE — the gate usually won it — so the suite was green on
/// this machine and on one of the two hosted jobs of the same commit.
///
/// This test removes the race rather than reruns it: the document is larger than
/// any pipe buffer, so a write to a pipe could not complete before the reader
/// exits, and the gate would die every time. It passes only because stdin is now
/// a file, which has no reader to close.
#[test]
fn a_reader_that_ignores_its_input_does_not_kill_the_gate() {
    let mut document = String::from("REQ-7 is cited on line one\n");
    // Comfortably past a Linux pipe's 64 KiB, so the write cannot fit.
    while document.len() < 512 * 1024 {
        document.push_str("filler that no reader here parses, only bytes to write\n");
    }
    let ws = workspace(
        r#"citation_readers = [{ name = "deaf", transport = "cli", command = ["sh", "-c", "printf '{\"cites\":[{\"line\":1,\"id\":\"REQ-7\"}]}'"], extensions = ["scxml"] }]"#,
        &[("model.scxml", document.as_str())],
    );
    let (passed, report) = report(ws.path());
    let found = violations(&report);
    assert!(
        !passed,
        "the program named a deprecated id and the gate lived to judge it: {found:?}"
    );
    assert_eq!(
        found,
        vec![("inventory_deprecated".to_string(), "REQ-7".to_string(), 1)],
        "the gate survived handing half a megabyte to a reader that read none of \
         it, and still judged what the reader said: {report}"
    );
}

/// CONTROL: the same annotation under the path axis keeps the attribute's
/// syntax in the id, so even the active entry is reported — the shape the
/// attribute axis exists for.
#[test]
fn the_same_annotation_under_the_path_axis_misses_the_active_entry() {
    let ws = workspace(
        r#"inventory_path_prefixes = ["req=\""]"#,
        &[(
            "model.scxml",
            "<scxml>\n<state id=\"idle\" req=\"REQ-4.2.1\"/>\n</scxml>\n",
        )],
    );
    let (_, report) = report(ws.path());
    assert_eq!(
        violations(&report),
        vec![(
            "inventory_missing".to_string(),
            "req=\"REQ-4.2.1".to_string(),
            2
        )],
        "control: the path axis reads the attribute's syntax into the id"
    );
}
