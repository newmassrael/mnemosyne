//! Round 835/836 predecessor — Round 834's design, built: every place a `String`
//! can live under `AtomicStore`, at ANY depth, must be classified by which gate
//! covers it.
//!
//! # Why this exists
//!
//! `every_side_table_detector_is_wired_into_the_aggregate` already forces a
//! human to classify a new field — by destructuring `AtomicStore` with no `..`,
//! so adding a field stops the build. Round 833 found what that cannot see:
//! `AtomicSection::ladder` (Round 765) carried four registry refs NESTED inside
//! an existing field, so no top-level field was ever added, the destructure
//! never stopped compiling, and the guard reported complete for sixty-eight
//! rounds. A completeness guard that watches one depth reports "complete" about
//! the depth it does not watch.
//!
//! # Why the obvious fix is not the fix
//!
//! "Just recurse the types" is dead on measurement, not on argument. Round 834
//! measured 52 reachable types carrying 91 String-typed fields, and WHICH are
//! registry refs is not written in any type: `SectionLadder::carrier` and
//! `AtomicSection::intent` are both `Option<String>`, one an entity ref and one
//! prose; `LadderRung::needs` and `AtomicSection::rationale_bullets` are both
//! `Vec<String>`, one a list of fact ids and one a list of sentences. No
//! mechanical walk can decide. A human must, once per field — and this makes the
//! moment of deciding unavoidable instead of invisible.
//!
//! # Why the classification is a PATH, not a boolean
//!
//! Coverage is not single-source. `AtomicSection::superseded_by` is guarded, but
//! by neither the write path nor a `*_violations` detector: `project.rs` emits it
//! as a cross-ref and the orphan scan resolves it (measured — a section
//! superseded by a phantom id exits 1 with `atomic orphan new`). A table that
//! only knew about detectors would demand a redundant one. So each field records
//! WHICH gate covers it.
//!
//! # What this does NOT do
//!
//! It catches an UNCLASSIFIED field, never a MISCLASSIFIED one. A field recorded
//! as `NotARef` that later becomes a ref by convention is still silent. The
//! difference from Round 833 is that a line already exists for someone to
//! change; that defect had no such line anywhere.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Which gate covers a String-bearing field — the question the table answers.
///
/// Round 834's design assumed TWO paths. Filling the table found FIVE, which is
/// the strongest argument for having built it: a classification that offered
/// only "detector or not a ref" would have forced three quarters of these fields
/// into a wrong answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Coverage {
    /// Re-checked at the scan boundary by a `*_violations` detector wired into
    /// `store_registry_violations` — the baseline gate.
    Detector,
    /// Projected as a cross-ref by `project.rs` and resolved by the orphan scan
    /// — also the baseline gate, by a different route. `superseded_by` is here,
    /// measured: a section superseded by a phantom id exits 1 with
    /// `atomic orphan new`.
    Projection,
    /// Checked by `scan_continuity` — a DIFFERENT command (`validate-continuity`)
    /// from the baseline gate, so a workspace that runs only the baseline does
    /// not get these. `pays_off`, `conflicts_with` and `supersedes_in_frame` are
    /// here, each with its own violation variant.
    Continuity,
    /// A mirror of an audit field whose drift is gated by the publishable/audit
    /// divergence check, not by a ref scan. The value is a copy, so its
    /// referential integrity is the audit field's.
    Divergence,
    /// The write path validates it and NO re-check was found by inspection.
    ///
    /// This is the Round 833 class, recorded rather than papered over: an
    /// out-of-band edit to one of these is invisible to every gate. Each is a
    /// candidate for a boundary twin, and listing them is what turns this table
    /// from a formality into an inventory. "No re-check found" is a statement
    /// about a reading, not a proof — confirming each is follow-on work.
    WritePathOnly,
    /// Not a registry ref. The reason is the payload: it is what a future reader
    /// re-examines when the field's meaning drifts.
    NotARef,
}

/// The sources the type graph is walked over. `include_str!` rather than a
/// runtime read, so a moved or renamed file is a COMPILE error — the first
/// attempt at this measurement read two `lib.rs` files, silently saw a third of
/// the graph, and reported the missing types as external primitives.
const SOURCES: &[(&str, &str)] = &[
    ("atomic/lib.rs", include_str!("../src/lib.rs")),
    (
        "core/lib.rs",
        include_str!("../../mnemosyne-core/src/lib.rs"),
    ),
    (
        "core/narrative.rs",
        include_str!("../../mnemosyne-core/src/narrative.rs"),
    ),
    (
        "core/fact.rs",
        include_str!("../../mnemosyne-core/src/fact.rs"),
    ),
    (
        "core/content_anchor.rs",
        include_str!("../../mnemosyne-core/src/content_anchor.rs"),
    ),
    (
        "core/scene.rs",
        include_str!("../../mnemosyne-core/src/scene.rs"),
    ),
    (
        "core/section_ref.rs",
        include_str!("../../mnemosyne-core/src/section_ref.rs"),
    ),
];

/// The registry id types (Round 838/839). A field of one of these is a ref whose
/// TARGET REGISTRY the compiler now holds, so it can no longer be misclassified
/// as to ref-ness — but WHICH gate checks it is still a human answer, so it stays
/// in the table. Dropping a field from here the moment it gains a type would
/// trade one blind spot for another.
///
/// This list grows by one line per migration round; the endgame is that it
/// replaces the `String` scan entirely and `NotARef` disappears.
const REF_ID_TYPES: &[&str] = &[
    "UnitId",
    "ParameterId",
    "PredicateId",
    "EntityKindId",
    "FrameId",
    "EntityId",
    "BranchId",
    "FactId",
    "SectionId",
];

/// Container names that are never a type to walk into.
const CONTAINERS: &[&str] = &[
    "String", "Vec", "Option", "BTreeMap", "BTreeSet", "HashMap", "HashSet", "Cow", "Box",
];

/// One declared type: the attribute text stacked above its header, and its body.
///
/// `body` holds the `(place, type-text)` pairs it declares. `place` is a struct
/// field name, an enum struct-variant field name, or a tuple-variant name. Enums
/// are walked too, because they carry refs — `TypedObject::Entity { id }` is an
/// entity id and `Locator::Prefix(String)` is not, which is the same
/// undecidability one level in.
///
/// `attributes` EXCLUDES comment lines, and that is load-bearing: this crate's
/// doc comments name `deny_unknown_fields` in prose ("the store structs carry no
/// `deny_unknown_fields`"), so a scan that read them would find the attribute on
/// the very types that lacked it.
struct TypeDecl {
    attributes: String,
    body: Vec<(String, String)>,
}

/// Which type headers a scan recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Headers {
    /// `pub struct Name {` / `pub enum Name {` at column 0 — the store's shape,
    /// and the only one the two store laws were measured against.
    TopLevelPub,
    /// Any `struct` / `enum` header at any indentation, public or not, because
    /// an input wire's type is often declared inside the function that parses it.
    Anywhere,
}

/// The indentation and bare name of a type header, or `None` when `line` is not
/// one `headers` recognises. Generic parameters are not part of the name.
fn type_header(line: &str, headers: Headers) -> Option<(&str, String)> {
    let line = line.trim_end();
    let code = line.trim_start();
    let indent = &line[..line.len() - code.len()];
    let code = match headers {
        Headers::TopLevelPub if indent.is_empty() => code.strip_prefix("pub ")?,
        Headers::TopLevelPub => return None,
        Headers::Anywhere => code
            .strip_prefix("pub(crate) ")
            .or_else(|| code.strip_prefix("pub "))
            .unwrap_or(code),
    };
    let rest = code
        .strip_prefix("struct ")
        .or_else(|| code.strip_prefix("enum "))?
        .strip_suffix('{')?;
    let name: String = rest
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some((indent, name))
}

/// Every type declared in `src` whose header `headers` recognises, in order.
fn decls_in(src: &str, headers: Headers) -> Vec<(String, TypeDecl)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((indent, name)) = type_header(lines[i], headers) else {
            i += 1;
            continue;
        };
        let attributes = preamble_attributes(&lines, i);
        let mut body: Vec<(String, String)> = Vec::new();
        i += 1;
        // A type block ends at the `}` standing at its header's own indentation
        // — for a column-0 type the first column-0 `}`, since nested braces are
        // indented (the shape the detector-wiring tripwire relies on).
        while i < lines.len()
            && !lines[i]
                .strip_prefix(indent)
                .is_some_and(|rest| rest.starts_with('}'))
        {
            let t = lines[i].trim();
            if !t.starts_with("//") && !t.starts_with("#[") {
                // Order matters: an inline struct-variant (`Entity { id:
                // String },`) also contains a `:` and would otherwise be read
                // as one malformed field and dropped. It was, on the first run
                // — `TypedObject::Entity.id` is an entity ref and went missing,
                // which is this file's own failure mode appearing inside the
                // file that exists to prevent it.
                let inline = inline_struct_variant(t);
                if !inline.is_empty() {
                    body.extend(inline);
                } else if let Some((place, ty)) = named_field(t) {
                    body.push((place, ty));
                } else if let Some((place, ty)) = tuple_variant(t) {
                    body.push((place, ty));
                }
            }
            i += 1;
        }
        out.push((name, TypeDecl { attributes, body }));
    }
    out
}

/// The store's candidate types, by name, from the files the store laws list.
fn type_decls() -> BTreeMap<String, TypeDecl> {
    SOURCES
        .iter()
        .flat_map(|(_label, src)| decls_in(src, Headers::TopLevelPub))
        .collect()
}

/// The attribute lines directly above line `header` — comment lines skipped (see
/// [`TypeDecl`] for why) — stopping at the first line that is neither. A
/// multi-line attribute is read upward from its closing `]` to its `#[`.
///
/// It stops at CODE, not only at a blank line: a type declared inside a function
/// sits directly under that function's signature, and reading the signature as
/// an attribute would credit the type with whatever the signature mentions.
fn preamble_attributes(lines: &[&str], header: usize) -> String {
    let mut attributes: Vec<&str> = Vec::new();
    let mut inside = false;
    let mut j = header;
    while j > 0 {
        j -= 1;
        let t = lines[j].trim();
        if t.starts_with("//") {
            continue;
        }
        if t.starts_with("#[") {
            attributes.push(t);
            inside = false;
        } else if inside || t.ends_with(']') {
            attributes.push(t);
            inside = true;
        } else {
            break;
        }
    }
    attributes.reverse();
    attributes.join(" ")
}

/// `Variant { a: T, b: U },` — an enum struct-variant written on one line, whose
/// fields are keyed `Variant.a`. Empty when the line is not one.
fn inline_struct_variant(t: &str) -> Vec<(String, String)> {
    let Some((head, rest)) = t.split_once('{') else {
        return Vec::new();
    };
    let head = head.trim();
    if head.is_empty() || !head.starts_with(|c: char| c.is_ascii_uppercase()) {
        return Vec::new();
    }
    let Some((inner, _)) = rest.rsplit_once('}') else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|part| named_field(part.trim()))
        .map(|(f, ty)| (format!("{head}.{f}"), ty))
        .collect()
}

/// `pub field: Type,` (struct) or `field: Type,` (enum struct-variant).
///
/// A field name may carry a DIGIT after its first character. Until that was
/// allowed, `spec_sha256` was not a field, so `ArtifactHashes` parsed as a type
/// with no body at all — see `the_walk_reads_a_field_whose_name_carries_a_digit`.
fn named_field(t: &str) -> Option<(String, String)> {
    let t = t.strip_prefix("pub ").unwrap_or(t);
    let (place, ty) = t.split_once(':')?;
    let place = place.trim();
    if !place.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
        || !place
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return None;
    }
    Some((
        place.to_string(),
        ty.trim().trim_end_matches(',').to_string(),
    ))
}

/// `Variant(Type),` — a tuple variant, keyed by the variant name.
fn tuple_variant(t: &str) -> Option<(String, String)> {
    let (place, rest) = t.split_once('(')?;
    let place = place.trim();
    if place.is_empty() || !place.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    let ty = rest.rsplit_once(')')?.0;
    Some((place.to_string(), ty.to_string()))
}

/// Type names mentioned in a type-text, minus the containers.
fn referenced(ty: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut cur = String::new();
    for c in ty.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            if cur.starts_with(|c: char| c.is_ascii_uppercase()) && !CONTAINERS.contains(&&*cur) {
                out.insert(cur.clone());
            }
            cur.clear();
        }
    }
    if cur.starts_with(|c: char| c.is_ascii_uppercase()) && !CONTAINERS.contains(&&*cur) {
        out.insert(cur);
    }
    out
}

/// Every type of ours reachable from `AtomicStore`, at any depth, the root
/// included — ONE walk, read by both laws in this file, so the population the
/// String classification covers and the population the unknown-key law covers
/// cannot be two different graphs.
fn reachable_types(decls: &BTreeMap<String, TypeDecl>) -> BTreeSet<String> {
    assert!(
        decls.contains_key("AtomicStore"),
        "AtomicStore must parse — the walk has no root otherwise"
    );
    let mut frontier: Vec<String> = vec!["AtomicStore".to_string()];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some(ty_name) = frontier.pop() {
        let Some(decl) = decls.get(&ty_name) else {
            continue; // not one of ours (an external or primitive type)
        };
        if !seen.insert(ty_name) {
            continue;
        }
        for (_, ty) in &decl.body {
            frontier.extend(referenced(ty).into_iter().filter(|t| !seen.contains(t)));
        }
    }
    seen
}

/// Every `(Type, place)` reachable from `AtomicStore` whose declared type
/// mentions `String`, at any depth.
fn derived_pairs() -> BTreeSet<(String, String)> {
    let decls = type_decls();
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    for ty_name in reachable_types(&decls) {
        if ty_name == "AtomicStore" {
            continue; // the root's own String places are map KEYS, not fields
        }
        for (place, ty) in &decls[&ty_name].body {
            // A String place, OR a place already migrated to a ref id type —
            // both must stay classified by covering gate.
            if ty.contains("String") || REF_ID_TYPES.iter().any(|t| ty.contains(t)) {
                pairs.insert((ty_name.clone(), place.clone()));
            }
        }
    }
    pairs
}

/// Whether a type is read FROM A JSON OBJECT — the only shape that can carry a
/// key nobody declared. A struct with named fields is; so is an enum with a
/// struct variant (`TypedObject::Quantity { n, unit }`). An enum of unit
/// variants is a string and an enum of tuple variants wraps one value: neither
/// has a key to refuse.
fn is_map_shaped(decl: &TypeDecl) -> bool {
    decl.body.iter().any(|(place, _)| {
        place.contains('.') || place.starts_with(|c: char| c.is_ascii_lowercase())
    })
}

/// A `Deserialize` type that reads past an unknown key ON PURPOSE, and why.
///
/// Every row is a decision somebody can defend in review. A row that stops
/// matching a lenient map-shaped type is refused as stale, and a row naming a
/// type the store reaches is refused outright — the store's refusal has no
/// exemption.
const LENIENT_BY_DESIGN: &[(&str, &str, &str)] = &[
    ("crates/mnemosyne-cli/src/atomic_cli.rs", "AnchorEntry", "medium-forge's `epub-anchor-map` is one file read by two verbs, each taking only the keys it needs — `import-epub-anchors` the locators, `import-epub-excerpts` the text — so each reader must pass over the other's"),
    ("crates/mnemosyne-cli/src/atomic_cli.rs", "AnchorMap", "the root of that shared `epub-anchor-map`, as `import-epub-anchors` reads it; `tools/medium-forge/convert.py` also writes a `schema` key there"),
    ("crates/mnemosyne-cli/src/atomic_cli.rs", "EpubExcerptEntry", "the other reader of the same shared `epub-anchor-map` (see `AnchorEntry`)"),
    ("crates/mnemosyne-cli/src/atomic_cli.rs", "ExcerptAnchorMap", "the root of that shared `epub-anchor-map`, as `import-epub-excerpts` reads it; `tools/medium-forge/convert.py` also writes a `schema` key there"),
    ("crates/mnemosyne-server/src/audit.rs", "AuditRecord", "append-only records written by every past build are read back; a field a later build retires must not make that history unreadable, and the log has no migration ladder"),
    ("crates/mnemosyne-validate/src/verifies_linkage.rs", "CatalogEntry", "a consumer-generated `verifies-catalog/v1`, documented lenient on extra fields since its first round"),
    ("crates/mnemosyne-validate/src/verifies_linkage.rs", "VerifiesCatalog", "the root of that consumer-generated catalog"),
];

/// Every Rust source under this workspace's `crates/*/src`, keyed by its
/// repository-relative path.
fn crate_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, into: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .map(|entry| entry.expect("a directory entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, into);
            } else if path.extension().is_some_and(|x| x == "rs") {
                into.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources: Vec<PathBuf> = std::fs::read_dir(root.join("crates"))
        .expect("the workspace has a crates/ directory")
        .map(|entry| entry.expect("a directory entry").path().join("src"))
        .filter(|src| src.is_dir())
        .collect();
    sources.sort();
    let mut files = Vec::new();
    for src in sources {
        walk(&src, &mut files);
    }
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let relative = path
                .strip_prefix(&root)
                .expect("every source is under the root")
                .to_string_lossy()
                .into_owned();
            (relative, text)
        })
        .collect()
}

/// EVERY TYPE READ FROM JSON REFUSES A KEY IT DOES NOT MODEL — not only the store's.
///
/// The store refusing an unknown key closed the loss AT REST. The same loss was
/// still open on every wire into the store: an agent calling
/// `add_inventory_entry` with a `"modality"` got success and the key was dropped
/// before any store saw it, and a manifest key an importer does not model
/// vanished the same way. So the population here is every map-shaped
/// `Deserialize` type in every crate's sources, declared anywhere — including
/// inside the function that parses it — and an exception is a written decision
/// in [`LENIENT_BY_DESIGN`], never an omission.
#[test]
fn every_type_read_from_json_refuses_a_key_it_does_not_model() {
    let store_types = reachable_types(&type_decls());
    let mut map_shaped = 0usize;
    let mut lenient: Vec<String> = Vec::new();
    let mut exempted: BTreeSet<(String, String)> = BTreeSet::new();
    for (file, src) in crate_sources() {
        for (name, decl) in decls_in(&src, Headers::Anywhere) {
            if !decl.attributes.contains("Deserialize") || !is_map_shaped(&decl) {
                continue;
            }
            map_shaped += 1;
            if decl.attributes.contains("deny_unknown_fields") {
                continue;
            }
            if LENIENT_BY_DESIGN
                .iter()
                .any(|(f, t, _)| *f == file && *t == name)
            {
                exempted.insert((file.clone(), name));
            } else {
                lenient.push(format!("{file}: {name}"));
            }
        }
    }

    // NON-VACUITY FLOOR — a scan that stops matching headers finds nothing, and
    // nothing lacks the attribute.
    assert!(
        map_shaped >= 150,
        "only {map_shaped} map-shaped `Deserialize` types across crates/*/src — the \
         scan has stopped reaching the sources rather than the sources having shrunk"
    );
    assert!(
        lenient.is_empty(),
        "{} of {map_shaped} map-shaped `Deserialize` type(s) read past a key they do not \
         model, and none is a written decision. Add `#[serde(deny_unknown_fields)]` — \
         or, only where reading past keys IS the design, a LENIENT_BY_DESIGN row with \
         the reason:\n    {}",
        lenient.len(),
        lenient.join("\n    ")
    );
    let stale: Vec<String> = LENIENT_BY_DESIGN
        .iter()
        .filter(|(f, t, _)| !exempted.contains(&((*f).to_string(), (*t).to_string())))
        .map(|(f, t, _)| format!("{f}: {t}"))
        .collect();
    assert!(
        stale.is_empty(),
        "LENIENT_BY_DESIGN rows that match no lenient map-shaped `Deserialize` type — \
         stale, or strict after all:\n    {}",
        stale.join("\n    ")
    );
    let store_exempt: Vec<&str> = LENIENT_BY_DESIGN
        .iter()
        .map(|(_, t, _)| *t)
        .filter(|t| store_types.contains(*t))
        .collect();
    assert!(
        store_exempt.is_empty(),
        "a type the store reaches cannot be lenient by design: {store_exempt:?}"
    );
    for (file, name, why) in LENIENT_BY_DESIGN {
        assert!(
            !why.trim().is_empty(),
            "{file}: {name} is exempt with no reason; the reason IS the row"
        );
    }
}

/// A FIELD NAME MAY CARRY A DIGIT, AND THE WALK MUST READ IT.
///
/// The walk once accepted only lowercase letters and `_` in a field name, so
/// `ArtifactHashes { spec_sha256, code_sha256, test_sha256 }` parsed as a type
/// with no fields. It was then neither String-bearing (never classified) nor
/// map-shaped (never required to deny unknown fields), and it read past an
/// unknown key inside every confirmation event while both laws in this file
/// passed. A law that under-reads its population reports clean about the part
/// it cannot see, so the population's shape is pinned here by a witness.
#[test]
fn the_walk_reads_a_field_whose_name_carries_a_digit() {
    let decls = type_decls();
    let places: Vec<&str> = decls
        .get("ArtifactHashes")
        .expect("ArtifactHashes must parse — the witness has no subject otherwise")
        .body
        .iter()
        .map(|(place, _)| place.as_str())
        .collect();
    assert_eq!(
        places,
        ["spec_sha256", "code_sha256", "test_sha256"],
        "the walk dropped a field whose name carries a digit"
    );
    assert!(
        reachable_types(&decls).contains("ArtifactHashes"),
        "ArtifactHashes is a field of ConfirmationEvent and must be reached"
    );
}

/// THE STORE REFUSES A KEY IT DOES NOT MODEL, AT EVERY DEPTH.
///
/// Without `deny_unknown_fields` serde reads past a key it has no field for and
/// the next save writes the store back without it. Measured by an adopter: an
/// inventory entry carrying `"modality"` loaded, was invisible to every query,
/// and was erased by an unrelated `set-inventory-status` that exited 0. The
/// version guard cannot see that case — the key was not written by a newer
/// build, it was written by a person — so the refusal has to live in the types.
///
/// A SOURCE LAW AND NOT ONLY A BEHAVIOURAL ONE, because the defect is an
/// ABSENCE: a type added under the store tomorrow without the attribute
/// reopens the hole at exactly one depth, and no test written today injects a
/// key into a type that does not exist yet. The behavioural half — that the
/// attribute does refuse, through `flatten` and through an internal tag — is
/// `AtomicStore::load`'s own tests.
#[test]
fn every_map_shaped_type_under_the_store_denies_unknown_fields() {
    let decls = type_decls();
    let map_shaped: Vec<&String> = reachable_types(&decls)
        .iter()
        .filter(|t| is_map_shaped(&decls[*t]))
        .map(|t| decls.get_key_value(t).expect("reachable ⊆ declared").0)
        .collect();

    // NON-VACUITY FLOOR — the same reason as the classification law's: a walk
    // that stops matching finds nothing, and nothing lacks the attribute.
    assert!(
        map_shaped.len() >= 30,
        "only {} map-shaped types reachable from AtomicStore — the walk has stopped \
         reaching the graph rather than the graph having shrunk",
        map_shaped.len()
    );

    let lenient: Vec<&String> = map_shaped
        .iter()
        .copied()
        .filter(|t| !decls[*t].attributes.contains("deny_unknown_fields"))
        .collect();
    assert!(
        lenient.is_empty(),
        "{} of {} map-shaped type(s) under AtomicStore read past a key they do not \
         model, so the next save erases it. Add `#[serde(deny_unknown_fields)]` to:\n    {}",
        lenient.len(),
        map_shaped.len(),
        lenient
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join("\n    ")
    );
}

/// THE TABLE. One line per String-bearing place reachable from the store,
/// naming the gate that covers it — or, for `NotARef`, why it is not one.
///
/// Bootstrapped from the deriver rather than by hand: Round 834 hand-listed 26
/// types and got 68 pairs where the derivation finds 52 and 91, missing a
/// quarter INCLUDING `TypedClaim` and `EvidenceRef`. A hand list restated the
/// tree and drifted from it inside the round about hand lists drifting, so this
/// table was filled in from the failure message this test prints.
#[rustfmt::skip]
const CLASSIFIED: &[(&str, &str, Coverage, &str)] = &[
    ("ArtifactHashes", "code_sha256", Coverage::NotARef, "hashes of the code files a confirmation was checked against, collected by the outside producer — not a store key"),
    ("ArtifactHashes", "spec_sha256", Coverage::NotARef, "the hash of the spec text a confirmation was checked against — not a store key"),
    ("ArtifactHashes", "test_sha256", Coverage::NotARef, "hashes of the test files a confirmation was checked against, collected by the outside producer — not a store key"),
    ("AtomicChangelogEntry", "carry_forward_bullets", Coverage::NotARef, "audit prose"),
    ("AtomicChangelogEntry", "changes_bullets", Coverage::NotARef, "audit prose"),
    ("AtomicChangelogEntry", "decision_summary", Coverage::NotARef, "audit prose"),
    ("AtomicChangelogEntry", "impact_refs", Coverage::Projection, "SectionId (R847) - validate_atomic_store orphan_entry_refs"),
    ("AtomicChangelogEntry", "publishable_carry_forward_bullets", Coverage::Divergence, "mirror of the audit field"),
    ("AtomicChangelogEntry", "publishable_changes_bullets", Coverage::Divergence, "mirror of the audit field"),
    ("AtomicChangelogEntry", "publishable_decision_summary", Coverage::Divergence, "mirror of the audit field"),
    ("AtomicChangelogEntry", "publishable_impact_refs", Coverage::Divergence, "mirror of the audit field's refs"),
    ("AtomicChangelogEntry", "publishable_verification_bullets", Coverage::Divergence, "mirror of the audit field"),
    ("AtomicChangelogEntry", "verification_bullets", Coverage::NotARef, "audit prose"),
    ("AtomicSection", "caveats_bullets", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "impact_scope", Coverage::Projection, "SectionId (R847), projected as cross-refs"),
    ("AtomicSection", "inputs_bullets", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "intent", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "outputs_bullets", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "rationale_bullets", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "resolved_by", Coverage::Projection, "SectionId (R847), projected as a cross-ref — and R1026 measured that projecting it bought nothing, because the orphan scan reads the STORE and had an arm for superseded_by only; the arm exists now (measured: the scan's answer moves)"),
    ("AtomicSection", "superseded_by", Coverage::Projection, "SectionId (R847), projected as a cross-ref (measured: exits 1)"),
    ("Binding", "file", Coverage::NotARef, "a workspace-relative path"),
    ("Binding", "symbol", Coverage::NotARef, "a code symbol name"),
    ("Branch", "description", Coverage::NotARef, "authored prose (the choice label)"),
    ("BranchFork", "at", Coverage::Detector, "SectionId (R847) - branch_ref_violations"),
    ("BranchFork", "branch", Coverage::Detector, "BranchId (R845) - branch_ref_violations"),
    ("ConfirmationClaim", "SectionCompleteness.section_id", Coverage::WritePathOnly, "section ref; no re-check found"),
    ("ConfirmationClaim", "file", Coverage::NotARef, "a workspace-relative path"),
    ("ConfirmationClaim", "section_id", Coverage::WritePathOnly, "section ref; no re-check found"),
    ("ConfirmationClaim", "symbol", Coverage::NotARef, "a code symbol name"),
    // THE MUTATION-REASON LEDGER (R1024) EMITS NO LIVE REF, and that is the
    // point rather than an oversight. Five of the ten primitives that write a
    // row are REMOVALS, so a row naming a target that no longer exists is the
    // normal case — the ledger outlives what it describes, which is what an
    // audit trail is for. A dangling-ref scan over `target_id` would report the
    // ledger working as a violation.
    ("MutationReason", "primitive", Coverage::NotARef, "the primitive's own name, as the receipt spells it"),
    ("MutationReason", "target_kind", Coverage::NotARef, "the receipt's kind word, not a registry key"),
    ("MutationReason", "target_id", Coverage::NotARef, "what changed, recorded even when the change was its removal"),
    ("MutationReason", "reason", Coverage::NotARef, "authored prose"),
    ("MutationReason", "applied_in", Coverage::NotARef, "the round a redaction is filed under, when the caller already knows it"),
    ("ConfirmationEvent", "authoring_run", Coverage::NotARef, "an opaque run identifier"),
    ("ConfirmationEvent", "confirming_run", Coverage::NotARef, "an opaque run identifier"),
    ("ConfirmationEvent", "rationale", Coverage::NotARef, "authored prose"),
    ("ConfirmationEvent", "timestamp", Coverage::NotARef, "an ISO timestamp"),
    ("Confirmer", "id", Coverage::NotARef, "the confirming tool's identity"),
    ("Confirmer", "version", Coverage::NotARef, "the confirming tool's version"),
    ("ConflictAssertion", "target", Coverage::Continuity, "FactId (R846) - ConflictTargetMissing"),
    ("ConflictAssertion", "target_claim_sha256", Coverage::NotARef, "a fingerprint of the target fact's claim at the moment the conflict was judged — a drift witness, not a store key"),
    ("ContentAnchor", "source", Coverage::NotARef, "a document name, not a registry key"),
    ("ContentExcerpt", "text", Coverage::NotARef, "projected prose"),
    ("ContentExcerpt", "text_sha256", Coverage::NotARef, "the hash of `text`, the offline drift anchor the mutate API sets at write time — not a store key"),
    ("DisclosureOverride", "first_at", Coverage::Detector, "BranchId (R845) keys - disclosure_ref_violations"),
    ("DisclosurePlan", "description", Coverage::NotARef, "authored prose"),
    ("DisclosurePlan", "overrides", Coverage::Detector, "FactId (R846) keys - disclosure_ref_violations"),
    ("DisclosureReveal", "coords", Coverage::Detector, "SectionId (R847) - disclosure_ref_violations"),
    ("DisclosureSurface", "object", Coverage::Detector, "EntityId (R844) - disclosure_ref_violations"),
    ("DisclosureSurface", "scene", Coverage::Detector, "SectionId (R847) - disclosure_ref_violations"),
    ("EdgeCost", "unit", Coverage::Detector, "UnitId (R839) - edge_cost_violations"),
    ("EdgeGuard", "conditions", Coverage::Detector, "FactId (R846) - edge_guard_violations"),
    ("Entity", "description", Coverage::NotARef, "authored prose"),
    ("Entity", "kind", Coverage::Detector, "entity-kind ref - unregistered_entity_kinds"),
    ("EntityKind", "description", Coverage::NotARef, "authored prose"),
    ("EntityKind", "parents", Coverage::Detector, "entity-kind refs - entity_kind_parent_violations"),
    ("EpubLocator", "cfi", Coverage::NotARef, "an EPUB coordinate"),
    ("EpubLocator", "fragment", Coverage::NotARef, "an EPUB coordinate"),
    ("EpubLocator", "spine_href", Coverage::NotARef, "an EPUB spine path"),
    ("EvidenceRef", "reviewed_excerpt_sha256", Coverage::NotARef, "a fingerprint of the prose the author affirms having reviewed — authored, never computed, and not a store key"),
    ("EvidenceRef", "section", Coverage::Detector, "SectionId (R847) - fact_registry_refs Evidence facet"),
    ("ExampleBlock", "code", Coverage::NotARef, "authored content"),
    ("ExampleBlock", "language", Coverage::NotARef, "a language tag"),
    ("Frame", "description", Coverage::NotARef, "authored prose"),
    ("InventoryDisposition", "Delegated.to_doc", Coverage::NotARef, "names a document outside this store; no cross-workspace reference exists to resolve it against"),
    ("InventoryDisposition", "Delegated.to_id", Coverage::NotARef, "names a requirement of that other document, not an id of this store"),
    ("InventoryDisposition", "OutOfScope.reason", Coverage::NotARef, "authored prose"),
    ("InventoryDisposition", "SystemLevel.realised_by", Coverage::NotARef, "authored prose naming a deployment or system property"),
    ("InventoryEntry", "reason", Coverage::NotARef, "authored prose"),
    ("ModalityBound", "unit", Coverage::Detector, "UnitId - inventory_axis_violations, the same check both inventory writers use (Round 1321)"),
    ("InventoryEntry", "section_ref", Coverage::WritePathOnly, "SectionId (R847); existence checked at BOTH writers since R1026, which measured that this row's `write path validates it` had been true only of the shape; no re-check found"),
    ("InventoryEntry", "source", Coverage::NotARef, "the declaring artifact"),
    ("LadderRung", "needs", Coverage::Detector, "FactId (R846) - ladder_ref_violations (R833)"),
    ("LadderRung", "object", Coverage::Detector, "EntityId (R844) - ladder_ref_violations (R833)"),
    ("LadderRung", "reveals", Coverage::Detector, "FactId (R846) - ladder_ref_violations (R833)"),
    ("Locator", "Cfi", Coverage::NotARef, "coordinate text"),
    ("Locator", "Prefix", Coverage::NotARef, "coordinate text"),
    ("NarrativeFact", "branch", Coverage::Detector, "BranchId (R845) - fact_registry_refs Branch facet"),
    ("NarrativeFact", "canon_from", Coverage::Detector, "SectionId (R847) - fact_registry_refs CanonFrom facet"),
    ("NarrativeFact", "canon_to", Coverage::Detector, "SectionId (R847) - fact_registry_refs CanonTo facet"),
    ("NarrativeFact", "claim", Coverage::NotARef, "the authored assertion itself"),
    ("NarrativeFact", "entities", Coverage::Detector, "EntityId (R844) - fact_registry_refs Entity facet"),
    ("NarrativeFact", "frame", Coverage::Detector, "FrameId (R843) - fact_registry_refs Frame facet"),
    ("NarrativeFact", "pays_off", Coverage::Continuity, "FactId (R846) - PayoffTargetMissing"),
    ("NarrativeFact", "quote", Coverage::NotARef, "authored prose"),
    ("NarrativeFact", "quote_sha256", Coverage::NotARef, "the hash of `quote`, computed by the mutate primitive for offline drift detection — not a store key"),
    ("NarrativeFact", "supersedes_in_frame", Coverage::Continuity, "FactId (R846) despite the name - SuccessionTargetMissing"),
    ("NormativeExcerpt", "anchor_url", Coverage::NotARef, "upstream provenance, not a store key"),
    ("NormativeExcerpt", "source_revision", Coverage::NotARef, "upstream provenance, not a store key"),
    ("Parameter", "description", Coverage::NotARef, "authored prose"),
    ("ParameterGate", "parameter", Coverage::Detector, "ParameterId (R839) - parameter_gate_violations"),
    ("Predicate", "description", Coverage::NotARef, "authored prose"),
    ("Predicate", "object_entity_kind", Coverage::Detector, "entity-kind ref - predicate_kind_ref_violations"),
    ("Predicate", "object_tokens", Coverage::NotARef, "the declared vocabulary, not a ref into one"),
    ("Predicate", "subject_kind", Coverage::Detector, "entity-kind ref - predicate_kind_ref_violations"),
    ("PopulationCensus", "axis", Coverage::NotARef, "names a question the WORKSPACE's own recount answers, not anything in this store; every recorded axis is checked against that recount by the workspace's gate (R979)"),
    ("PopulationCensus", "left_label", Coverage::NotARef, "what a side counts, in the axis's own words"),
    ("PopulationCensus", "right_label", Coverage::NotARef, "what a side counts, in the axis's own words"),
    ("RejectedAlternative", "alternative", Coverage::NotARef, "authored prose"),
    ("RejectedAlternative", "reason", Coverage::NotARef, "authored prose"),
    ("ScenePresence", "entity", Coverage::WritePathOnly, "EntityId (R844); no re-check found"),
    ("SectionLadder", "carrier", Coverage::Detector, "EntityId (R844) - ladder_ref_violations (R833)"),
    ("SectionSkeleton", "parent_doc", Coverage::NotARef, "a document label, not a registry key"),
    ("SectionSkeleton", "parent_section", Coverage::WritePathOnly, "SectionId (R847); no re-check found"),
    ("SectionSkeleton", "title", Coverage::NotARef, "authored prose"),
    ("TypedClaim", "predicate", Coverage::Detector, "predicate ref - TypedPredicate facet"),
    ("TypedClaim", "subject", Coverage::Detector, "EntityId (R844) - TypedSubject facet"),
    ("TypedObject", "Entity.id", Coverage::Detector, "EntityId (R844) - TypedObject facet"),
    ("TypedObject", "Fact.id", Coverage::WritePathOnly, "FactId (R846), phase-2 (store union staged); excluded from the facets"),
    ("TypedObject", "Quantity.unit", Coverage::Detector, "UnitId (R839) - TypedUnit facet"),
    ("TypedObject", "Token.token", Coverage::WritePathOnly, "checked against the predicate's declared tokens; excluded from the facets"),
    ("Unit", "description", Coverage::NotARef, "authored prose"),
    ("VerificationRun", "command", Coverage::NotARef, "the words a verification wrapper ran, copied out of the record it wrote (R1316); it names a command line, nothing in this store"),
    ("VerificationRun", "log", Coverage::NotARef, "the file name of that record under a gitignored, budget-collected directory (R1316) — a pointer OUT of the store, and deliberately not resolvable: the record is expected to be collected long before the entry is"),
];

#[test]
fn every_string_field_under_the_store_is_classified() {
    let derived = derived_pairs();

    // NON-VACUITY FLOORS. A parser that quietly stops matching reports an empty
    // derivation, and an empty derivation is a subset of any table — it would
    // pass while checking nothing. The floors are set below Round 834's measured
    // 52 types / 91 pairs so ordinary growth does not trip them, and far above
    // zero so a broken parse cannot.
    assert!(
        derived.len() >= 80,
        "the derivation found only {} String-bearing places; Round 834 measured 91, \
         so the parser has stopped matching rather than the tree having shrunk",
        derived.len()
    );
    let types: BTreeSet<&str> = derived.iter().map(|(t, _)| t.as_str()).collect();
    assert!(
        types.len() >= 20,
        "only {} types carry String places — the walk is not reaching the graph",
        types.len()
    );

    let classified: BTreeSet<(String, String)> = CLASSIFIED
        .iter()
        .map(|(t, f, _, _)| ((*t).to_string(), (*f).to_string()))
        .collect();

    // Unclassified: a String place exists that nobody has decided about. This is
    // the Round 833 case — a ref-bearing field arriving at a depth no guard
    // watched.
    let unclassified: Vec<&(String, String)> = derived
        .iter()
        .filter(|p| !classified.contains(*p))
        .collect();
    assert!(
        unclassified.is_empty(),
        "{} String place(s) under AtomicStore are unclassified. Add each to \
         CLASSIFIED with the gate that covers it (Detector / Projection) or with \
         NotARef and the reason:\n{}",
        unclassified.len(),
        unclassified
            .iter()
            .map(|(t, f)| format!("    (\"{t}\", \"{f}\", Coverage::???, \"why\"),"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Stale: a table entry for a place that no longer exists. The Round 783 rule
    // — a declared exclusion matching nothing is folklore, and folklore is what
    // this table would otherwise decay into.
    let stale: Vec<&(String, String)> = classified
        .iter()
        .filter(|p| !derived.contains(*p))
        .collect();
    assert!(
        stale.is_empty(),
        "{} CLASSIFIED entry(ies) name a place that no longer exists — delete them:\n{}",
        stale.len(),
        stale
            .iter()
            .map(|(t, f)| format!("    {t}.{f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The table's own shape: no duplicate keys, and every `NotARef` carries a
/// reason. A blank reason is the classification that will be re-derived wrongly
/// by the next reader, which is the failure this whole file exists to prevent.
#[test]
fn the_classification_table_is_well_formed() {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for (t, f, cov, why) in CLASSIFIED {
        assert!(
            seen.insert((t, f)),
            "{t}.{f} is classified twice — two answers for one place"
        );
        if *cov == Coverage::NotARef {
            assert!(
                !why.trim().is_empty(),
                "{t}.{f} is declared NotARef with no reason; the reason IS the entry"
            );
        }
    }
}
