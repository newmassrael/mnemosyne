//! The spatial-graph gate (`validate-map`) — the port of the consumer's
//! hand-built `build-map.py` G1 (the owner's 2026-07-17 ruling: the map exists
//! before the novel is authored, the novel is authored on top, and the gate
//! filters against it — "소설이 장소를 발명하지 못한다. 새 장소가 필요하면
//! 여기를 먼저 고친다").
//!
//! G1 is three checks over a declared `map/v1` file and the store:
//!   (a) every map NODE names a registered entity;
//!   (b) that entity is of the configured `place_kind`;
//!   (c) every EDGE endpoint is a map node.
//! Plus the fail-loud precondition that `place_kind` is itself a registered
//! entity kind (the R669 machine-slot rule: the configured kind is a ref, not
//! free text — a typo'd `place_kind` must not silently pass every node).
//!
//! **Why the wire is NOT `deny_unknown_fields`** (R668 died on exactly this):
//! the real `map.json` carries keys this gate does not read — prose (`note`,
//! `law`, `open`), and cost vocabulary (`modes`, `unit`) that is DEBT-J /
//! DEBT-I (Quantity), not yet modelled. A `deny_unknown_fields` wire rejects
//! the real file on its FIRST key (`note`). Instead the fields the gate READS
//! (`nodes`, `edges`, and each node's `id`, each edge's `a`/`b`) are REQUIRED,
//! so a misspelled `nodes` fails to parse (fail-loud) rather than yielding an
//! empty node set that passes vacuously — the R604 silent-key lesson applied
//! where it actually bites, without rejecting the legitimate not-yet-modelled
//! keys.

use mnemosyne_atomic::AtomicStore;
use serde::Deserialize;
use std::path::Path;

/// One node in a `map/v1` declaration. Only `id` is read (it must resolve to a
/// registered place entity); `name`/`outside`/`note`/`tide_floods`/… are the
/// consumer's and tolerated unread.
#[derive(Debug, Clone, Deserialize)]
pub struct MapNode {
    pub id: String,
}

/// One undirected edge. Only the endpoints are read here; `walk`/`tide_closes`
/// (cost + tide guard) are DEBT-J / SCE and tolerated unread.
#[derive(Debug, Clone, Deserialize)]
pub struct MapEdge {
    pub a: String,
    pub b: String,
}

/// The declared spatial graph. `nodes` and `edges` are REQUIRED (a missing or
/// misspelled key fails to parse — the anti-vacuous-pass guard); every other
/// key in the real file (`note`/`law`/`modes`/`unit`/`open`) is tolerated
/// unread. See the module header for why this is not `deny_unknown_fields`.
#[derive(Debug, Clone, Deserialize)]
pub struct MapFile {
    pub nodes: Vec<MapNode>,
    pub edges: Vec<MapEdge>,
}

/// Load + parse a `map/v1` file, with the optional sha256 pin (the R428
/// gate-authority-input contract, shared shape with `load_canon_order`).
pub fn load_map_file(path: &Path, expected_sha256: Option<&str>) -> Result<MapFile, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("map read {}: {}", path.display(), e))?;
    if let Some(expected) = expected_sha256 {
        let actual = mnemosyne_core::sha256_hex(&bytes);
        if actual != expected {
            return Err(format!(
                "map sha256 mismatch at {}: pinned {expected} but file hashes {actual} — the \
                 declaration changed without a re-pin (or was tampered); re-generate, review, \
                 and update [map].sha256",
                path.display(),
            ));
        }
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("map parse {}: {}", path.display(), e))
}

/// G1 findings, in the file's order (deterministic — the messages mirror
/// `build-map.py`'s so the ported gate reads the same). Empty = the map and
/// the store agree on their nodes.
///
/// `place_kind` is the configured spatial kind and must be a registered entity
/// kind; an unregistered one is a fail-loud FIRST finding, not a silent
/// pass-everything (the R669 rule — the configured value is a ref).
pub fn check_map_g1(store: &AtomicStore, map: &MapFile, place_kind: &str) -> Vec<String> {
    let mut findings = Vec::new();

    if !store.entity_kinds.contains_key(place_kind) {
        findings.push(format!(
            "[map].place_kind `{place_kind}` is not a registered entity kind — declare it \
             (add-entity-kind) or fix the spelling; an unregistered place_kind would pass \
             every node vacuously"
        ));
        // Without a valid place_kind the per-node kind check below is
        // meaningless; return the one finding that explains the whole run.
        return findings;
    }

    let node_ids: std::collections::BTreeSet<&str> =
        map.nodes.iter().map(|n| n.id.as_str()).collect();

    // G1(a) + G1(b): every node is a registered entity of place_kind.
    for n in &map.nodes {
        match store.entities.get(&n.id) {
            None => findings.push(format!(
                "G1 map node `{}` is not in the store — a typo must not silently make a world",
                n.id
            )),
            Some(entity) if entity.kind != place_kind => findings.push(format!(
                "G1 map node `{}` is not a `{place_kind}` (kind = `{}`)",
                n.id, entity.kind
            )),
            Some(_) => {}
        }
    }

    // G1(c): every edge endpoint is a map node.
    for e in &map.edges {
        for (side, id) in [("a", &e.a), ("b", &e.b)] {
            if !node_ids.contains(id.as_str()) {
                findings.push(format!(
                    "G1 edge endpoint `{id}` (side {side}) is not a map node"
                ));
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_core::{Entity, EntityKind};

    /// A store with `place` registered and three place nodes, plus a non-place
    /// entity — the minimal shape G1 reads.
    fn store() -> AtomicStore {
        let mut s = AtomicStore::new();
        s.entity_kinds
            .insert("place".to_string(), EntityKind::default());
        s.entity_kinds
            .insert("character".to_string(), EntityKind::default());
        for id in ["ent-dike", "ent-village", "ent-well"] {
            s.entities.insert(
                id.to_string(),
                Entity {
                    kind: "place".to_string(),
                    description: String::new(),
                },
            );
        }
        s.entities.insert(
            "ent-jiun".to_string(),
            Entity {
                kind: "character".to_string(),
                description: String::new(),
            },
        );
        s
    }

    fn map(nodes: &[&str], edges: &[(&str, &str)]) -> MapFile {
        MapFile {
            nodes: nodes
                .iter()
                .map(|id| MapNode { id: id.to_string() })
                .collect(),
            edges: edges
                .iter()
                .map(|(a, b)| MapEdge {
                    a: a.to_string(),
                    b: b.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn clean_map_passes() {
        let m = map(
            &["ent-dike", "ent-village", "ent-well"],
            &[("ent-dike", "ent-village"), ("ent-village", "ent-well")],
        );
        assert!(check_map_g1(&store(), &m, "place").is_empty());
    }

    /// NON-VACUITY: each fault class fires. The clean case above passing is not
    /// evidence the gate does anything until a fault also fails it.
    #[test]
    fn g1a_invented_place_is_caught() {
        let m = map(&["ent-dike", "ent-invented"], &[]);
        let f = check_map_g1(&store(), &m, "place");
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("ent-invented") && f[0].contains("not in the store"));
    }

    #[test]
    fn g1b_non_place_node_is_caught() {
        // ent-jiun is a registered entity, but a character, not a place.
        let m = map(&["ent-dike", "ent-jiun"], &[]);
        let f = check_map_g1(&store(), &m, "place");
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("ent-jiun") && f[0].contains("not a `place`"));
    }

    #[test]
    fn g1c_dangling_edge_endpoint_is_caught() {
        let m = map(&["ent-dike"], &[("ent-dike", "ent-ghost")]);
        let f = check_map_g1(&store(), &m, "place");
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("ent-ghost") && f[0].contains("not a map node"));
    }

    #[test]
    fn unregistered_place_kind_is_a_single_loud_finding() {
        // A typo'd place_kind must not pass every node vacuously (R669): it is
        // ONE finding that explains the run, not silence.
        let m = map(&["ent-dike"], &[]);
        let f = check_map_g1(&store(), &m, "palce");
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("palce") && f[0].contains("not a registered entity kind"));
    }

    /// The R668 parse guard: the wire accepts the REAL map.json shape — top
    /// keys `note`/`law`/`modes`/`unit`/`open` beside `nodes`/`edges`, and each
    /// node/edge carrying extra keys — because a `deny_unknown_fields` wire
    /// rejects it on the first key. A missing `nodes` key still fails loud.
    #[test]
    fn wire_tolerates_unmodelled_keys_but_requires_nodes_and_edges() {
        let real = r#"{
            "note": "prose", "law": ["a", "b"],
            "modes": { "walk": { "name": "walk" } }, "unit": "minute",
            "nodes": [ { "id": "ent-dike", "name": "둑", "outside": true } ],
            "edges": [ { "a": "ent-dike", "b": "ent-village", "walk": 4, "tide_closes": true } ],
            "open": ["todo"]
        }"#;
        let m: MapFile = serde_json::from_str(real).expect("real map shape must parse");
        assert_eq!(m.nodes.len(), 1);
        assert_eq!(m.edges.len(), 1);

        // A misspelled `nodes` (here: omitted) fails to parse — not a vacuous
        // empty node set.
        let missing = r#"{ "edges": [] }"#;
        assert!(serde_json::from_str::<MapFile>(missing).is_err());
    }
}
