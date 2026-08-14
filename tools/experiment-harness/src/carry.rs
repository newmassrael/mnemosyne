//! Evidence entering the tree — the one step in this pipeline that was a
//! person's `cp`.
//!
//! WHAT THIS IS FOR. Declaring, sealing, assigning a role and re-verifying a
//! seal are all verbs; the moment evidence CROSSES INTO the tree was the only
//! step with no program, and it is the step that handles bytes nothing can
//! regenerate. Round 1203 measured what that costs on one kit: `scale-floor`'s
//! two graded stores — the artifacts its scores were read off — sat outside
//! version control on one machine, 64 KB and 72 KB, sealed by nothing, and no
//! revision of this repository can rebuild them because four of that kit's own
//! manifests no longer parse here. One disk failure from gone, and the record
//! that describes the run pinned the manifests AROUND them.
//!
//! WHY A VERB AND NOT A COPY. Three things a `cp` cannot do, each of which has
//! already been paid for somewhere in this repository:
//!
//!   - It cannot refuse. Evidence overwritten by a later, different file is the
//!     one edit a seal exists to prevent, and the seal is written afterwards.
//!   - It leaves the record to a second step, so the tree can hold evidence
//!     nothing declares — which is the state R1191 found and named: 193 files
//!     outside every run tree, 15 declared, none of them evidence.
//!   - It forgets where the bytes came from. After a copy the record can say
//!     "this is a run artifact of this kit" and nothing anywhere says these
//!     bytes were carried in, from what, or when — the provenance lives in a
//!     session transcript, which is exactly the kind of claim Round 1202 spent
//!     a round moving out of prose and into something a program reads.
//!
//! SO THE VERB DOES ALL OF IT IN ONE CALL: copy the bytes verbatim, refuse a
//! destination outside a run tree or one holding different bytes already, make
//! the file tracked, declare it through the walk that already owns declaring,
//! seal it through the stamp that already owns sealing, and write `carried_from`
//! beside the entry so the origin is a fact in the record rather than a memory.
//!
//! AND THAT LAST FIELD BUYS A LAW, which is the point of writing it down: where
//! the source still exists on the machine asking, the carried bytes must still
//! hash to what the record sealed. It is a machine-conditional check and says so
//! when it cannot run, in the shape `check-side-workspaces.sh` established for a
//! workspace whose sibling checkout is not present.

use std::collections::BTreeSet;
use std::path::Path;

use crate::declare;
use crate::seal;
use crate::util::{read_bytes, read_file, sha256_hex, write_bytes, write_file, HResult};

/// One artifact this call brought in, or found already here unchanged.
#[derive(Debug, PartialEq, Eq)]
pub struct Carried {
    /// Repo-relative destination.
    pub into: String,
    /// Where the bytes were read from, as the caller named it.
    pub from: String,
    pub sha256: String,
    /// Whether the destination already held exactly these bytes.
    pub already: bool,
}

/// Where a set of sources is allowed to land, and under what name.
///
/// PURE, SO ITS REFUSALS HAVE CASES. Round 1096's lesson is that a decision
/// reachable only through `git` and a real kit tree is a decision nothing asks
/// about; every refusal here is a function of its arguments.
///
/// # Errors
///
/// A destination outside the unit, one that is not under a run tree, a source
/// with no file name, or two sources that would land on the same name.
pub fn destinations(unit: &str, into: &str, sources: &[String]) -> HResult<Vec<String>> {
    let inside = into
        .strip_prefix(&format!("{unit}/"))
        .ok_or_else(|| format!("{into} is not inside {unit}, whose record this is"))?;
    // A RUN TREE, because that is the rule the gate enforces on the role this
    // verb writes: a `run-artifact` claims membership of the kit's run tree and
    // may not be used anywhere else. Asked of the same rule rather than a second
    // copy of it — a file that belongs somewhere else is `declare-evidence`'s,
    // and it needs a role a walk cannot infer.
    if !inside.split('/').any(|part| part == "run") {
        return Err(format!(
            "{into} is not under a run tree of {unit} — a run-artifact claims \
             membership of the kit's run tree, and evidence that lives elsewhere \
             is declared with `declare-evidence` and a role somebody chose"
        ));
    }
    let mut out = Vec::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    for source in sources {
        let name = Path::new(source)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("{source} has no file name to carry in under"))?;
        let landed = format!("{into}/{name}");
        if !taken.insert(landed.clone()) {
            return Err(format!(
                "two sources would both land on {landed} — carrying them in \
                 would leave whichever came second, and the record would seal it \
                 under a name that describes the other"
            ));
        }
        out.push(landed);
    }
    Ok(out)
}

/// Carry named files into a kit's run tree, declare them and seal them.
pub fn run(record: &str, into: &str, sources: &[String]) -> HResult<Vec<Carried>> {
    if sources.is_empty() {
        return Err("--from names the evidence to carry in and was given none".to_string());
    }
    let root_raw = declare::git(Path::new("."), &["rev-parse", "--show-toplevel"])?;
    let root = Path::new(root_raw.trim()).to_path_buf();

    let unit = Path::new(record)
        .parent()
        .ok_or_else(|| format!("{record} has no parent directory"))?
        .to_string_lossy()
        .into_owned();
    let unit = unit
        .strip_prefix(&format!("{}/", root.display()))
        .unwrap_or(&unit)
        .to_string();
    if !root.join(&unit).join("replay.json").is_file() {
        return Err(format!("{record} is not a kit record"));
    }

    let landing = destinations(&unit, into, sources)?;
    std::fs::create_dir_all(root.join(into)).map_err(|e| format!("cannot make {into}: {e}"))?;

    let mut carried = Vec::new();
    for (source, landed) in sources.iter().zip(&landing) {
        let bytes = read_bytes(source)?;
        let sha256 = sha256_hex(&bytes);
        let destination = root.join(landed);
        let destination_str = destination
            .to_str()
            .ok_or_else(|| format!("{landed} is not utf-8"))?
            .to_string();
        // ALREADY HERE IS TWO ANSWERS, NOT ONE. The same bytes make this call a
        // no-op, which is what makes it safe to re-run; different bytes are a
        // refusal, because overwriting evidence is the single edit the seal
        // beside it exists to prevent, and the seal would be rewritten by the
        // very call that destroyed what it described.
        if destination.is_file() {
            let here = read_bytes(&destination_str)?;
            if here == bytes {
                carried.push(Carried {
                    into: landed.clone(),
                    from: source.clone(),
                    sha256,
                    already: true,
                });
                continue;
            }
            return Err(format!(
                "{landed} is already here and holds different bytes \
                 (here {}, {source} {sha256}) — evidence is not overwritten by a \
                 tool, and a seal rewritten by the call that replaced what it \
                 described would say nothing",
                sha256_hex(&here)
            ));
        }
        write_bytes(&destination_str, &bytes)?;
        carried.push(Carried {
            into: landed.clone(),
            from: source.clone(),
            sha256,
            already: false,
        });
    }

    // TRACKED, DECLARED AND SEALED IN THE SAME CALL. Each of the three is what
    // the state before it is missing: bytes nothing tracks are one `git clean`
    // from gone, a tracked file nothing declares is what R1191 found the tree
    // full of, and a declared input with no digest is a claim with no seal.
    // `declare` takes its population from `git ls-files`, so the staging below
    // is not tidiness — it is what makes the new file visible to the walk.
    let paths: Vec<&str> = landing.iter().map(String::as_str).collect();
    let mut add = vec!["add", "--"];
    add.extend(paths.iter().copied());
    declare::git(&root, &add)?;
    declare::run(&[record.to_string()])?;
    for entry in &carried {
        note_origin(record, &unit, &entry.into, &entry.from)?;
    }
    seal::stamp_inputs(record)?;
    Ok(carried)
}

/// Write `carried_from` onto the record's entry for one carried path.
///
/// The record stores paths relative to the unit directory, so the entry is
/// found by the same relativisation `declare` used to write it — deriving it a
/// second way here is how the two would come to disagree about which entry this
/// is.
fn note_origin(record: &str, unit: &str, landed: &str, from: &str) -> HResult<()> {
    let relative = landed
        .strip_prefix(&format!("{unit}/"))
        .ok_or_else(|| format!("{landed} is not inside {unit}"))?;
    let raw = read_file(record)?;
    let mut doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("{record} is not JSON: {e}"))?;
    let inputs = doc
        .get_mut("inputs")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| format!("{record} declares no `inputs` array"))?;
    let entry = inputs
        .iter_mut()
        .find(|i| i.get("path").and_then(|p| p.as_str()) == Some(relative))
        .ok_or_else(|| {
            format!("{record} has no entry for {relative} — the declaring walk did not reach it")
        })?;
    let object = entry
        .as_object_mut()
        .ok_or_else(|| format!("{record}: the entry for {relative} is not an object"))?;
    object.insert(
        "carried_from".to_string(),
        serde_json::Value::String(from.to_string()),
    );
    let mut rendered = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("{record}: cannot render the updated record: {e}"))?;
    rendered.push('\n');
    write_file(record, &rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_destination_outside_a_run_tree_is_refused_and_says_which_verb_takes_it() {
        let unit = "claudedocs/phase1-x";
        let sources = vec!["/elsewhere/store-A.json".to_string()];

        let landed = destinations(unit, "claudedocs/phase1-x/run/store-A", &sources)
            .expect("a run tree is where a run-artifact lives");
        assert_eq!(landed, ["claudedocs/phase1-x/run/store-A/store-A.json"]);

        // Inside the unit and not under a run tree: the role this verb writes
        // would be a lie there, and the sibling verb exists for it.
        let elsewhere = destinations(unit, "claudedocs/phase1-x/reports", &sources)
            .expect_err("a run-artifact may not live outside the run tree");
        assert!(elsewhere.contains("declare-evidence"), "{elsewhere}");

        // Another unit's tree entirely — the record being edited is this one's.
        let foreign = destinations(unit, "claudedocs/phase1-y/run", &sources)
            .expect_err("a record may not carry evidence into another kit");
        assert!(foreign.contains("whose record this is"), "{foreign}");
    }

    #[test]
    fn two_sources_that_would_land_on_one_name_are_refused_before_anything_is_written() {
        // THE SHAPE THAT LOSES EVIDENCE SILENTLY: two stores called
        // `story-A.atomic.json` under different directories are exactly what
        // this repository's extraction trees hold, and carried into one place
        // the second would overwrite the first while the record sealed one
        // digest under a name describing the other.
        let refused = destinations(
            "claudedocs/phase1-x",
            "claudedocs/phase1-x/run",
            &[
                "/a/story-A.atomic.json".to_string(),
                "/b/story-A.atomic.json".to_string(),
            ],
        )
        .expect_err("two sources may not land on one name");
        assert!(refused.contains("would both land on"), "{refused}");
    }

    #[test]
    fn a_source_with_no_file_name_is_refused_rather_than_landing_under_the_directory() {
        let refused = destinations(
            "claudedocs/phase1-x",
            "claudedocs/phase1-x/run",
            &["/a/b/..".to_string()],
        )
        .expect_err("a path with no file name has no name to be carried in under");
        assert!(refused.contains("no file name"), "{refused}");
    }
}
