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

/// One node in a `map/v1` declaration. `id` must resolve to a registered place
/// entity (G1); `outside` marks the map's ENTRANCE — the BFS root G4 walks from
/// (a node reachable from outside the island). The rest (`name`/`note`/
/// `tide_floods`/…) is the consumer's and tolerated unread.
#[derive(Debug, Clone, Deserialize)]
pub struct MapNode {
    pub id: String,
    /// `true` = an entrance to the graph (the mainland end of the dyke). G4
    /// requires at least one, and reaches every other node from it. Defaults
    /// false (an interior node) when the key is absent.
    #[serde(default)]
    pub outside: bool,
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

/// G4 — the graph is CONNECTED from an entrance: there is at least one
/// `outside: true` node, and every node is reachable from the entrances by
/// walking the (undirected) edges. An unreachable node is a place the story
/// can put a character but no one can walk to — `build-map.py`'s "갈 수 없는
/// 자리다". Findings are in sorted id order (deterministic, mirroring the port).
///
/// Independent of G1 (needs no store, no place_kind): a map can be internally
/// well-connected while mis-referencing the store, and vice versa.
pub fn check_map_g4(map: &MapFile) -> Vec<String> {
    let mut findings = Vec::new();
    let node_ids: std::collections::BTreeSet<&str> =
        map.nodes.iter().map(|n| n.id.as_str()).collect();

    // Undirected adjacency, only between declared nodes (a dangling endpoint is
    // G1's finding, not G4's — here it simply contributes no reachability).
    let mut adj: std::collections::BTreeMap<&str, std::collections::BTreeSet<&str>> = node_ids
        .iter()
        .map(|&n| (n, std::collections::BTreeSet::new()))
        .collect();
    for e in &map.edges {
        let (a, b) = (e.a.as_str(), e.b.as_str());
        if node_ids.contains(a) && node_ids.contains(b) {
            adj.get_mut(a).unwrap().insert(b);
            adj.get_mut(b).unwrap().insert(a);
        }
    }

    let roots: Vec<&str> = map
        .nodes
        .iter()
        .filter(|n| n.outside)
        .map(|n| n.id.as_str())
        .collect();
    if roots.is_empty() {
        findings.push(
            "G4 the map has no entrance — at least one node must be `outside: true` \
             (the BFS root; without it every node is unreachable)"
                .to_string(),
        );
        return findings;
    }

    let mut seen: std::collections::BTreeSet<&str> = roots.iter().copied().collect();
    let mut stack = roots;
    while let Some(n) = stack.pop() {
        for &nb in &adj[n] {
            if seen.insert(nb) {
                stack.push(nb);
            }
        }
    }
    // node_ids is sorted (BTreeSet), so the findings are stable.
    for &n in &node_ids {
        if !seen.contains(n) {
            findings.push(format!(
                "G4 `{n}` is unreachable from the entrance — a place you cannot get to"
            ));
        }
    }
    findings
}

/// G2 — every store PLACE is on the map (the owner's key gate: "소설이 장소를
/// 발명하면 여기서 터진다"). G1 catches a map node with no store entity; G2 is
/// the OTHER direction — a store place entity that no map node names is a place
/// the novel invented without adding it to the map first.
///
/// CONTAINERS are the exception: place entities used as a fact search key but
/// NOT a position (`exclusive(per:subject)` = one person one place), so they
/// are deliberately not nodes. `containers` is the configured id list; a
/// container must be a registered place entity (a typo'd one silently fails to
/// exclude the real place), and a container that DID leak in as a node breaks
/// exclusive and is its own finding.
///
/// Guarded on `place_kind` being registered — G1 owns that finding, so if it is
/// unregistered G2 stays silent rather than double-reporting.
pub fn check_map_g2(
    store: &AtomicStore,
    map: &MapFile,
    place_kind: &str,
    containers: &[String],
) -> Vec<String> {
    let mut findings = Vec::new();
    if !store.entity_kinds.contains_key(place_kind) {
        return findings; // G1 reports the unregistered place_kind.
    }
    let node_ids: std::collections::BTreeSet<&str> =
        map.nodes.iter().map(|n| n.id.as_str()).collect();
    let container_set: std::collections::BTreeSet<&str> =
        containers.iter().map(String::as_str).collect();

    // A configured container must be a registered place entity — else the typo
    // does not exclude the place it names, and G2(1) below fires on the real
    // container with a misleading message.
    for &c in &container_set {
        match store.entities.get(c) {
            None => findings.push(format!(
                "G2 [map].containers names `{c}`, which is not a store entity — fix the id"
            )),
            Some(e) if e.kind != place_kind => findings.push(format!(
                "G2 [map].containers `{c}` is not a `{place_kind}` (kind = `{}`)",
                e.kind
            )),
            Some(_) => {}
        }
    }

    // G2(1): every store place, minus containers, is a map node. `store.entities`
    // is a BTreeMap so iteration is sorted — stable findings.
    for (id, entity) in &store.entities {
        if entity.kind == place_kind
            && !container_set.contains(id.as_str())
            && !node_ids.contains(id.as_str())
        {
            findings.push(format!(
                "G2 store place `{id}` is not on the map — the novel invented a place; \
                 fix map.json first"
            ));
        }
    }

    // G2(2): no container leaked in as a node (that would break exclusive).
    for &c in &container_set {
        if node_ids.contains(c) {
            findings.push(format!(
                "G2 container `{c}` is a map node — exclusive breaks (one person in two places)"
            ));
        }
    }

    findings
}

/// Run every IMPLEMENTED map gate and concatenate their findings (G1 + G2 + G4
/// today; G3/G5/G6 are still on `build-map.py` — see the module's carry). The
/// gates are independent, so all run regardless of each other's findings.
pub fn check_map(
    store: &AtomicStore,
    map: &MapFile,
    place_kind: &str,
    containers: &[String],
) -> Vec<String> {
    let mut findings = check_map_g1(store, map, place_kind);
    findings.extend(check_map_g2(store, map, place_kind, containers));
    findings.extend(check_map_g4(map));
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

    /// The FIRST node is the entrance (`outside: true`) so the map is a valid
    /// G4 shape; the rest are interior. G1-only tests ignore `outside`.
    fn map(nodes: &[&str], edges: &[(&str, &str)]) -> MapFile {
        MapFile {
            nodes: nodes
                .iter()
                .enumerate()
                .map(|(i, id)| MapNode {
                    id: id.to_string(),
                    outside: i == 0,
                })
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
        // All gates: a clean map (every place is a node) has no finding.
        assert!(check_map(&store(), &m, "place", &[]).is_empty());
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

    #[test]
    fn g4_no_entrance_is_caught() {
        // No node is `outside: true` — `map()` marks index 0, so build inline.
        let m = MapFile {
            nodes: vec![
                MapNode {
                    id: "ent-dike".into(),
                    outside: false,
                },
                MapNode {
                    id: "ent-village".into(),
                    outside: false,
                },
            ],
            edges: vec![MapEdge {
                a: "ent-dike".into(),
                b: "ent-village".into(),
            }],
        };
        let f = check_map_g4(&m);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("no entrance") && f[0].contains("outside"));
    }

    #[test]
    fn g4_unreachable_node_is_caught() {
        // ent-well has no edge to the connected {dike, village} component.
        let m = map(
            &["ent-dike", "ent-village", "ent-well"],
            &[("ent-dike", "ent-village")],
        );
        let f = check_map_g4(&m);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("ent-well") && f[0].contains("unreachable"));
    }

    #[test]
    fn g4_reaches_across_a_multi_hop_chain() {
        // dike(entrance) -> village -> well -> shrine: BFS must reach the tail.
        let m = map(
            &["ent-dike", "ent-village", "ent-well", "ent-shrine"],
            &[
                ("ent-dike", "ent-village"),
                ("ent-village", "ent-well"),
                ("ent-well", "ent-shrine"),
            ],
        );
        assert!(check_map_g4(&m).is_empty(), "{:?}", check_map_g4(&m));
    }

    /// The gates are independent: `check_map` runs all three and does not stop
    /// at the first. This map is G2-clean (every store place is a node) and
    /// G4-clean (connected), so only the G1 fault (ent-invented is not a store
    /// entity) surfaces — proving a clean G2/G4 does not suppress G1.
    #[test]
    fn check_map_runs_all_gates_independently() {
        let m = map(
            &["ent-dike", "ent-village", "ent-well", "ent-invented"],
            &[
                ("ent-dike", "ent-village"),
                ("ent-village", "ent-well"),
                ("ent-well", "ent-invented"),
            ],
        );
        let f = check_map(&store(), &m, "place", &[]);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("ent-invented") && f[0].contains("G1"));
    }

    /// G2 — a store place that no map node names is the novel inventing a place
    /// (the owner's key gate). The store has ent-well; a map without it fires.
    #[test]
    fn g2_store_place_not_on_map_is_caught() {
        let m = map(&["ent-dike", "ent-village"], &[("ent-dike", "ent-village")]);
        // ent-well is a store place absent from the map.
        let f = check_map_g2(&store(), &m, "place", &[]);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("ent-well") && f[0].contains("invented a place"));
    }

    /// G2 CONTAINER exception: a place deliberately excluded from the map (a
    /// search key, not a position) does NOT fire G2(1) — but only when it is
    /// actually declared a container.
    #[test]
    fn g2_container_is_excused_but_only_when_declared() {
        let m = map(&["ent-dike", "ent-village"], &[("ent-dike", "ent-village")]);
        // Undeclared: ent-well fires. Declared a container: it is excused.
        assert_eq!(check_map_g2(&store(), &m, "place", &[]).len(), 1);
        assert!(check_map_g2(&store(), &m, "place", &["ent-well".to_string()]).is_empty());
    }

    /// G2(2): a container that leaked in as a node breaks exclusive (one person
    /// in two places), so it is a finding even though it is a declared place.
    #[test]
    fn g2_container_as_node_breaks_exclusive() {
        let m = map(
            &["ent-dike", "ent-village", "ent-well"],
            &[("ent-dike", "ent-village"), ("ent-village", "ent-well")],
        );
        // ent-well is BOTH a declared container AND a node — the leak G2(2) names.
        let f = check_map_g2(&store(), &m, "place", &["ent-well".to_string()]);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("ent-well") && f[0].contains("exclusive breaks"));
    }

    /// A configured container must be a registered place entity — a typo'd id
    /// fails loud rather than silently failing to exclude the real place.
    #[test]
    fn g2_bogus_container_id_is_caught() {
        let m = map(&["ent-dike", "ent-village", "ent-well"], &[]);
        // "ent-typo" is not a store entity; "ent-jiun" is a character, not place.
        let f_missing = check_map_g2(&store(), &m, "place", &["ent-typo".to_string()]);
        assert!(f_missing
            .iter()
            .any(|x| x.contains("ent-typo") && x.contains("not a store entity")));
        let f_wrongkind = check_map_g2(&store(), &m, "place", &["ent-jiun".to_string()]);
        assert!(f_wrongkind
            .iter()
            .any(|x| x.contains("ent-jiun") && x.contains("not a `place`")));
    }
}
