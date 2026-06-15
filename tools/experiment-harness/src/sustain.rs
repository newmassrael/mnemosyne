//! cast-sustainment — the R545/R551 deep-tail cast-presence metric, made
//! reproducible and fail-loud (R555). Reads the authored `facts.json` +
//! `order.json` (both tracked experiment inputs) and reports, per world-line,
//! how many distinct person-frames remain active in the deep tail (the scenes
//! strictly after the fork). Replaces the throwaway Python scan that backed
//! R551's PIN-2; every threshold is an explicit parameter (no magic constant —
//! this also addresses the R551 hand-picked-floor caveat).

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::util::{read_file, HResult};

#[derive(Deserialize)]
struct FactsManifest {
    #[serde(default)]
    facts: Vec<FactRow>,
}

#[derive(Deserialize)]
struct FactRow {
    frame: String,
    canon_from: String,
}

#[derive(Deserialize)]
struct OrderFile {
    #[serde(default)]
    branches: BTreeMap<String, Vec<Vec<String>>>,
}

/// The sustainment floor, every field explicit.
#[derive(Debug)]
pub struct Floor {
    pub ground_frame: String,
    pub principals: usize,
    pub min_active: usize,
    pub min_nonprincipal: usize,
    pub min_frames: usize,
}

#[derive(Debug)]
pub struct WorldRow {
    pub world: String,
    pub active: Vec<String>,
    pub nonprincipal: Vec<String>,
    pub ok: bool,
}

#[derive(Debug)]
pub struct Report {
    pub person_frames: usize,
    pub principals: Vec<String>,
    pub worlds: Vec<WorldRow>,
    pub floor: Floor,
    pub hold: bool,
}

/// The deep tail of a world = every node on its branch edge-chain except the
/// fork node (the first edge's `from`). A world with no edges is a hard error.
fn deep_tail(world: &str, edges: &[Vec<String>]) -> HResult<BTreeSet<String>> {
    let first = edges
        .first()
        .ok_or_else(|| format!("world `{world}` has no branch edges"))?;
    if first.len() != 2 {
        return Err(format!(
            "world `{world}`: edge {first:?} is not a [from, to] pair"
        ));
    }
    let fork = first[0].clone();
    let mut tail = BTreeSet::new();
    for e in edges {
        if e.len() != 2 {
            return Err(format!(
                "world `{world}`: edge {e:?} is not a [from, to] pair"
            ));
        }
        for node in e {
            if *node != fork {
                tail.insert(node.clone());
            }
        }
    }
    Ok(tail)
}

fn compute(facts: &FactsManifest, order: &OrderFile, floor: Floor) -> HResult<Report> {
    if order.branches.is_empty() {
        return Err("order has no `branches` — nothing to measure".to_string());
    }
    // Total fact count per person-frame (the ground-truth frame is excluded).
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &facts.facts {
        if f.frame != floor.ground_frame {
            *totals.entry(f.frame.as_str()).or_insert(0) += 1;
        }
    }
    let person_frames = totals.len();
    // Principals = top-K by (count desc, name asc) — a stable, deterministic order.
    let mut ranked: Vec<(&str, usize)> = totals.iter().map(|(k, v)| (*k, *v)).collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let principals: BTreeSet<&str> = ranked
        .iter()
        .take(floor.principals)
        .map(|(k, _)| *k)
        .collect();

    let mut worlds = Vec::new();
    let mut hold = person_frames >= floor.min_frames;
    for (world, edges) in &order.branches {
        let tail = deep_tail(world, edges)?;
        let mut active: BTreeSet<&str> = BTreeSet::new();
        for f in &facts.facts {
            if f.frame != floor.ground_frame && tail.contains(&f.canon_from) {
                active.insert(f.frame.as_str());
            }
        }
        let nonprincipal: Vec<String> = active
            .iter()
            .filter(|fr| !principals.contains(*fr))
            .map(|s| s.to_string())
            .collect();
        let ok = active.len() >= floor.min_active && nonprincipal.len() >= floor.min_nonprincipal;
        hold = hold && ok;
        worlds.push(WorldRow {
            world: world.clone(),
            active: active.iter().map(|s| s.to_string()).collect(),
            nonprincipal,
            ok,
        });
    }
    Ok(Report {
        person_frames,
        principals: principals.iter().map(|s| s.to_string()).collect(),
        worlds,
        floor,
        hold,
    })
}

pub fn run(facts_path: &str, order_path: &str, floor: Floor) -> HResult<Report> {
    let facts: FactsManifest = serde_json::from_str(&read_file(facts_path)?)
        .map_err(|e| format!("cannot parse {facts_path}: {e}"))?;
    let order: OrderFile = serde_json::from_str(&read_file(order_path)?)
        .map_err(|e| format!("cannot parse {order_path}: {e}"))?;
    compute(&facts, &order, floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor() -> Floor {
        Floor {
            ground_frame: "gt".to_string(),
            principals: 1,
            min_active: 2,
            min_nonprincipal: 1,
            min_frames: 2,
        }
    }

    fn facts() -> FactsManifest {
        // a/b/c are person-frames; `a` is the principal (3 facts). The tail of
        // world W is {sc-2, sc-3}; a, b, c each hold a tail fact -> 3 active, 2
        // non-principal.
        let rows: Vec<FactRow> = [
            ("gt", "sc-1"),
            ("a", "sc-1"),
            ("a", "sc-2"),
            ("a", "sc-3"),
            ("b", "sc-2"),
            ("c", "sc-3"),
        ]
        .iter()
        .map(|(fr, sc)| FactRow {
            frame: fr.to_string(),
            canon_from: sc.to_string(),
        })
        .collect();
        FactsManifest { facts: rows }
    }

    fn order() -> OrderFile {
        let mut branches = BTreeMap::new();
        branches.insert(
            "W".to_string(),
            vec![
                vec!["sc-1".to_string(), "sc-2".to_string()],
                vec!["sc-2".to_string(), "sc-3".to_string()],
            ],
        );
        OrderFile { branches }
    }

    #[test]
    fn counts_active_and_nonprincipal_in_the_tail() {
        let r = compute(&facts(), &order(), floor()).unwrap();
        assert_eq!(r.person_frames, 3);
        assert_eq!(r.principals, vec!["a".to_string()]);
        let w = &r.worlds[0];
        assert_eq!(w.active.len(), 3); // a, b, c hold a tail fact (sc-2/sc-3)
        assert_eq!(w.nonprincipal.len(), 2); // b, c
        assert!(w.ok);
        assert!(r.hold);
    }

    #[test]
    fn a_thin_tail_fails_the_floor() {
        // Raise the floor above the cast: requires >= 4 active.
        let f = Floor {
            min_active: 4,
            ..floor()
        };
        let r = compute(&facts(), &order(), f).unwrap();
        assert!(!r.worlds[0].ok);
        assert!(!r.hold);
    }

    #[test]
    fn a_world_with_no_edges_is_a_loud_error() {
        let mut o = order();
        o.branches.insert("empty".to_string(), vec![]);
        let err = compute(&facts(), &o, floor()).unwrap_err();
        assert!(err.contains("no branch edges"));
    }
}
