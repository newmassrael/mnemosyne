//! The blind-authored corpus these integration tests read, rebuilt in place.
//!
//! ONE home for the rebuild (Round 940). The place axis pinned itself against
//! the arm-D record first (Round 936), and the claim axis reads the same store;
//! a second copy of this loop would be a second site biting the same shape, and
//! two sites diverge — the exact cost the first consumer's own map module
//! records paying.
//!
//! The store is rebuilt from the tracked manifests WHERE THEY ALREADY LIVE
//! rather than copied into a fixtures directory: a copy rots silently, and
//! reading the evidence in place means these tests redden if the frozen record
//! is ever mutated.

use std::fs;
use std::path::{Path, PathBuf};

use mnemosyne_atomic::{AtomicStore, FactsManifest, SectionImport};
use tempfile::TempDir;

/// The arm-D stage-B corpus: a blind author's flooded hill-town, authored
/// against `describe-schema` alone with the words map / transition / adjacency /
/// edge / graph / route withheld from the brief.
pub fn corpus_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/mnemosyne-engine
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .join("claudedocs/phase1-map-corpus-experiment/v4/run/stage-b")
}

/// Rebuild the corpus into a scratch workspace and hand back its root.
pub fn rebuild() -> TempDir {
    let src = corpus_dir();
    let tmp = TempDir::new().expect("scratch workspace");
    let root = tmp.path();
    fs::create_dir_all(root.join("docs/.atomic")).expect("sidecar dir");
    for f in ["mnemosyne.toml", "order.json", "rules.json"] {
        fs::copy(src.join(f), root.join(f)).unwrap_or_else(|e| panic!("copy {f}: {e}"));
    }

    let sidecar = AtomicStore::default_sidecar_path(root);
    let mut store = AtomicStore::default();

    let sections: Vec<SectionImport> = serde_json::from_str(
        &fs::read_to_string(src.join("sections.json")).expect("sections.json"),
    )
    .expect("sections manifest parses");
    mnemosyne_atomic::import_sections(&mut store, &sidecar, &sections).expect("sections import");

    let facts: FactsManifest =
        serde_json::from_str(&fs::read_to_string(src.join("facts.json")).expect("facts.json"))
            .expect("facts manifest parses");
    mnemosyne_atomic::import_facts(&mut store, &sidecar, &facts).expect("facts import");

    tmp
}
