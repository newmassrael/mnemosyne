//! SCE lift-request 4-A — one parse per FILE, over the bytes the caller
//! already holds.
//!
//! The consumer measured 108.9 seconds in this gate across five workspaces and
//! found 99.4% of it in symbol resolution, because `resolve_symbol_at(file,
//! line)` re-read and re-parsed the whole file FOR EVERY CITATION. A file with
//! N citations was parsed N times.
//!
//! Three laws, and the second is not about cost at all:
//!
//! 1. ONE CALL PER FILE. However many citations in a file reach the symbol
//!    axis, the resolver is asked once, with all their lines. Measured with a
//!    counting resolver rather than a stopwatch: a timing assertion would be a
//!    statement about this machine, and the property is about the call graph.
//!
//! 2. THE ANSWER COMES FROM THE BYTES THE CITATION CAME FROM. The call site
//!    holds the file's text — it extracted the citations from it — and the old
//!    resolver went back to disk for its own copy. Two reads of one file can
//!    disagree (an editor saving mid-run is the ordinary case), and then the
//!    symbol answer is about a file the citation was never in. The contract now
//!    passes the source; each plugin's own tests pin that it parses THAT, by
//!    resolving against a path which does not exist.
//!
//! 3. THE COST IS REPORTABLE BEFORE IT IS PAID. `symbol_axis_coverage` counts
//!    the citations a scan will put to a resolver and the distinct files they
//!    sit in — the ratio the consumer's payoff estimate needs — and does it in
//!    a tree with no resolver configured, which is the only state the tree
//!    hosting this gate is ever in. The third test holds that prediction
//!    against what the counting resolver actually received, because a census
//!    free to disagree with the gate reports some other tool's cost.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::{SetEqualityValidatorConfig, Severity};
use mnemosyne_core::{AtomicStoreView, ResolverError, SymbolResolver, VersionSurface};
use mnemosyne_validate::code_refs::{
    CitationAttribution, NumberingOriginAxis, SetEqualityValidator,
};
use tempfile::TempDir;

/// One recorded call: the file, the lines it was asked about, and the length of
/// the source it was handed.
type Call = (PathBuf, Vec<u32>, usize);

/// A resolver that answers "drifted" for every line and remembers every call.
struct CountingResolver {
    calls: Arc<Mutex<Vec<Call>>>,
}

impl SymbolResolver for CountingResolver {
    fn version_surface(&self) -> VersionSurface {
        VersionSurface {
            plugin_name: "counting".into(),
            plugin_version: "0".into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn resolve_symbols_at(
        &self,
        file: &Path,
        source: &str,
        lines: &[u32],
    ) -> Result<BTreeMap<u32, String>, ResolverError> {
        self.calls.lock().expect("not poisoned").push((
            file.to_path_buf(),
            lines.to_vec(),
            source.len(),
        ));
        // Every line resolves to a symbol the store does NOT record, so each
        // demanded line also produces a visible judgement — the call count and
        // the judgement count are then independently observable.
        Ok(lines.iter().map(|l| (*l, "drifted".to_string())).collect())
    }
}

/// A workspace whose `src/many.rs` carries THREE citations of a section that
/// records a symbol for it and `src/one.rs` carries one; both files are bound,
/// so those four citations reach the symbol axis.
///
/// Two files are there to make the counting discriminating rather than
/// decorative. `src/silent.rs` carries no citation at all, so a resolver that
/// is handed the read set rather than the demand would show up. And `§40` binds
/// `src/one.rs` with NO symbol recorded: that citation is bound, is judged by
/// the file-level axis, and must NOT become a resolver call — a census that
/// counted "cited and bound" instead of "records a symbol" would say five.
fn workspace() -> (TempDir, AtomicStore) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/many.rs"),
        "// §39 first\nfn a() {}\n// §39 second\nfn b() {}\n// §39 third\nfn c() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/one.rs"),
        "// §39 only\nfn d() {}\n// §40 file-level only\nfn f() {}\n",
    )
    .unwrap();
    std::fs::write(root.join("src/silent.rs"), "fn e() {}\n").unwrap();

    let mut store = AtomicStore::new();
    let mut with_symbol = mnemosyne_atomic::AtomicSection::default();
    for file in ["src/many.rs", "src/one.rs"] {
        with_symbol.bindings.push(mnemosyne_atomic::Binding {
            file: file.to_string(),
            symbol: Some("registered".to_string()),
            kind: mnemosyne_core::BindingKind::Implements,
        });
    }
    store.sections.insert("39".into(), with_symbol);
    let mut file_level_only = mnemosyne_atomic::AtomicSection::default();
    file_level_only.bindings.push(mnemosyne_atomic::Binding {
        file: "src/one.rs".to_string(),
        symbol: None,
        kind: mnemosyne_core::BindingKind::Implements,
    });
    store.sections.insert("40".into(), file_level_only);
    (tmp, store)
}

fn validator(calls: Option<Arc<Mutex<Vec<Call>>>>) -> SetEqualityValidator {
    let mut symbol_resolvers: BTreeMap<String, Box<dyn SymbolResolver>> = BTreeMap::new();
    if let Some(calls) = calls {
        symbol_resolvers.insert("rust".to_string(), Box::new(CountingResolver { calls }));
    }
    SetEqualityValidator {
        config: SetEqualityValidatorConfig {
            paths: vec!["src/".to_string()],
            comment_only: true,
            severity_binding: Severity::Reject,
            ..Default::default()
        },
        entry_id_prefix: "Round ".to_string(),
        orphan_ledger: vec![],
        symbol_resolvers,
        filter_id: None,
        path_scope: None,
    }
}

fn run(v: &SetEqualityValidator, root: &Path, store: &AtomicStore) -> Vec<String> {
    let attribution = CitationAttribution::new(root, &v.config, NumberingOriginAxis::derive(root));
    let snapshot = AtomicStoreView::snapshot(store);
    v.scan(&attribution, &snapshot)
        .expect("scan")
        .iter()
        .map(|v| v.kind_tag().to_string())
        .collect()
}

/// LAWS 1 and 2 — one call per file, carrying every demanded line and the
/// caller's own bytes.
#[test]
fn a_file_with_three_citations_is_put_to_the_resolver_once() {
    let (tmp, store) = workspace();
    let root = tmp.path();
    let calls: Arc<Mutex<Vec<Call>>> = Arc::default();
    let v = validator(Some(Arc::clone(&calls)));

    let kinds = run(&v, root, &store);

    let recorded = calls.lock().expect("not poisoned").clone();
    let mut per_file: BTreeMap<PathBuf, Vec<Vec<u32>>> = BTreeMap::new();
    for (file, lines, _) in &recorded {
        per_file
            .entry(file.clone())
            .or_default()
            .push(lines.clone());
    }
    assert_eq!(
        per_file.len(),
        2,
        "only the two files carrying a checked citation are asked about: {per_file:?}"
    );
    for (file, calls_for_file) in &per_file {
        assert_eq!(
            calls_for_file.len(),
            1,
            "{} was put to the resolver {} time(s), not once",
            file.display(),
            calls_for_file.len()
        );
    }
    let many = per_file
        .iter()
        .find(|(f, _)| f.ends_with("many.rs"))
        .expect("many.rs was asked about")
        .1[0]
        .clone();
    assert_eq!(
        many,
        vec![1, 3, 5],
        "the one call carries EVERY demanded line of that file"
    );

    // LAW 2 — the caller hands over the bytes it read, so the resolver never
    // needs the disk. Length is the observable here; that the ANSWER comes from
    // those bytes is pinned in each plugin's own suite, against a path that
    // does not exist.
    for (file, _, source_len) in &recorded {
        let on_disk = std::fs::read_to_string(file).expect("the fixture file is readable");
        assert_eq!(
            *source_len,
            on_disk.len(),
            "the resolver was handed {}'s own bytes",
            file.display()
        );
    }

    // NON-VACUITY: every demanded line was judged, not just the first per file.
    assert_eq!(
        kinds.iter().filter(|k| *k == "symbol_mismatch").count(),
        4,
        "all four checked citations are judged: {kinds:?}"
    );
}

/// LAW 3 — the census predicts exactly the demand a scan makes, with NO
/// resolver configured.
#[test]
fn the_census_prices_the_axis_and_the_scan_pays_exactly_that() {
    let (tmp, store) = workspace();
    let root = tmp.path();
    let snapshot = AtomicStoreView::snapshot(&store);

    let dry = validator(None);
    let attribution =
        CitationAttribution::new(root, &dry.config, NumberingOriginAxis::derive(root));
    let cov = dry
        .symbol_axis_coverage(&attribution, &snapshot)
        .expect("census");
    assert_eq!(
        (cov.checked_citations, cov.checked_files),
        (4, 2),
        "four citations over two files — the ratio a one-parse-per-file change \
         buys. FIVE would mean the count is `cited and bound` rather than \
         `records a symbol`: §40's citation is bound and is judged, and no \
         resolver is ever asked about it"
    );

    let calls: Arc<Mutex<Vec<Call>>> = Arc::default();
    let wet = validator(Some(Arc::clone(&calls)));
    run(&wet, root, &store);
    let recorded = calls.lock().expect("not poisoned").clone();
    let lines_asked: usize = recorded.iter().map(|(_, lines, _)| lines.len()).sum();
    let files_asked: BTreeSet<PathBuf> = recorded.iter().map(|(f, _, _)| f.clone()).collect();
    assert_eq!(
        (lines_asked, files_asked.len()),
        (cov.checked_citations, cov.checked_files),
        "the census and the scan must not be able to disagree about the cost"
    );
}
