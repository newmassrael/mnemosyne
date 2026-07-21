//! The fail-loud interactive-layer gate — the render->fact enforcement (tide's
//! knots G4/G5/G6) brought into the kernel. Given a world and its authored
//! interactivity, it reports every spot where the interactive layer would break
//! play: an empty spot, a ladder rung that reveals a fact the store does not
//! offer (a leak — the class of the tide field-report bug), an offered fact no
//! door can reach, or a precondition that is never diggable in time.

use std::collections::{HashMap, HashSet};

use crate::{Door, PlayableProjection};

/// A fail-loud gate finding — a spot where the interactive layer would break
/// play. Reported, never silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GateViolation {
    /// G4 — a walked spot shows nothing: no disclosed line AND no door. The
    /// player arrives and there is nothing to read or do.
    EmptySpot {
        /// The world-line walked.
        world: String,
        /// The empty section.
        section: String,
    },
    /// G5 (leak) — a ladder rung reveals a fact the store does NOT offer at this
    /// spot: the consumer's ladder invents a reveal. THE render->fact gate (the
    /// class of the tide field-report bug, now caught in the kernel).
    RungRevealsUnofferedFact {
        /// The world-line walked.
        world: String,
        /// The section whose ladder leaked.
        section: String,
        /// The `fact_id` the rung claimed to reveal but the store never offered.
        fact_id: String,
    },
    /// G5 (unreachable) — a ladder spot offers a fact that no door (ask or
    /// examine) reveals: the ladder closed access, so the player can never dig
    /// it. Only a ladder spot gates access; elsewhere facts are shown directly.
    /// Reported only for a MODAL layer; a PARTIAL layer
    /// ([`Interactivity::free_investigate`](crate::Interactivity::free_investigate))
    /// reveals the remainder freely, so an offered fact is never stranded there.
    OfferedFactUnreachable {
        /// The world-line walked.
        world: String,
        /// The ladder section.
        section: String,
        /// The offered `fact_id` no door reveals.
        fact_id: String,
    },
    /// G6 — a rung's precondition fact is never offered at-or-before this spot on
    /// the walk: a lock that stops the chain before it can open (a typo'd or
    /// time-reversing `needs`).
    PreconditionUnreachable {
        /// The world-line walked.
        world: String,
        /// The section of the rung whose precondition dangles.
        section: String,
        /// The `fact_id` the rung needs but the walk never offers in time.
        needs: String,
    },
}

impl PlayableProjection {
    /// Run the interactive-layer gate over `world` under the projection's
    /// configured interactivity: G4 (empty spot), G5 (leak + unreachable), G6
    /// (precondition timing). Returns every violation in walk order — an empty
    /// vec means the interactive layer is play-clean. Pure read; never mutates.
    #[must_use]
    pub fn gate(&self, world: &str) -> Vec<GateViolation> {
        let walk = self.walk(world);

        // For G6: the earliest walk index at which each fact is offered (a fact
        // may be offered at several spots; the first wins — we scan in walk
        // order and `or_insert` keeps the earliest).
        let mut earliest_offer: HashMap<&str, usize> = HashMap::new();
        for (index, section) in walk.iter().enumerate() {
            for line in self.lines(world, section) {
                earliest_offer.entry(line.fact_id.as_str()).or_insert(index);
            }
        }

        let mut violations = Vec::new();
        for (index, section) in walk.iter().enumerate() {
            let lines = self.lines(world, section);
            let doors = self.doors_at(world, section);

            // G4 — nothing to read AND nothing to do.
            if lines.is_empty() && doors.is_empty() {
                violations.push(GateViolation::EmptySpot {
                    world: world.to_string(),
                    section: section.clone(),
                });
            }

            let offered: HashSet<&str> = lines.iter().map(|l| l.fact_id.as_str()).collect();
            let mut reachable: HashSet<&str> = HashSet::new();
            for door in &doors {
                match door {
                    Door::Examine { reveals, .. } => {
                        reachable.extend(reveals.iter().map(String::as_str));
                    }
                    Door::Ask { reveals, .. } => {
                        reachable.insert(reveals.as_str());
                    }
                    Door::Fork { .. } => {}
                }
            }

            let rungs = self.rungs_at(section);
            for rung in rungs {
                // G5 leak — the rung reveals a fact never offered here.
                if !offered.contains(rung.reveals.as_str()) {
                    violations.push(GateViolation::RungRevealsUnofferedFact {
                        world: world.to_string(),
                        section: section.clone(),
                        fact_id: rung.reveals.clone(),
                    });
                }
                // G6 — each precondition offered at-or-before this spot.
                for need in &rung.needs {
                    let in_time = earliest_offer
                        .get(need.as_str())
                        .is_some_and(|&offered_index| offered_index <= index);
                    if !in_time {
                        violations.push(GateViolation::PreconditionUnreachable {
                            world: world.to_string(),
                            section: section.clone(),
                            needs: need.clone(),
                        });
                    }
                }
            }

            // G5 unreachable — only where a ladder gates access AND the layer is
            // MODAL (no free fallback). A PARTIAL consumer (free_investigate)
            // reveals the remainder freely, so no offered fact is stranded and this
            // check does not apply. Iterate `lines` (deterministic order), not the
            // `offered` set.
            if !rungs.is_empty() && !self.free_investigate() {
                for line in lines {
                    if !reachable.contains(line.fact_id.as_str()) {
                        violations.push(GateViolation::OfferedFactUnreachable {
                            world: world.to_string(),
                            section: section.clone(),
                            fact_id: line.fact_id.clone(),
                        });
                    }
                }
            }
        }
        violations
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use mnemosyne_core::DisclosureMode;
    use mnemosyne_validate::continuity::ForkTreeReport;

    use crate::gate::GateViolation;
    use crate::test_support::{begin, locator, report, rung, scene};
    use crate::{Interactivity, PlayableProjection, Rung, StaticOverrides};

    fn ladder_at(section: &str, rungs: Vec<Rung>) -> StaticOverrides {
        StaticOverrides {
            interactivity: Interactivity {
                objects: HashSet::new(),
                ladders: HashMap::from([(section.to_string(), rungs)]),
                free_investigate: false,
            },
            journal_predicates: Vec::new(),
        }
    }

    fn partial_ladder_at(section: &str, rungs: Vec<Rung>) -> StaticOverrides {
        StaticOverrides {
            interactivity: Interactivity {
                objects: HashSet::new(),
                ladders: HashMap::from([(section.to_string(), rungs)]),
                free_investigate: true,
            },
            journal_predicates: Vec::new(),
        }
    }

    #[test]
    fn a_clean_interactive_layer_has_no_violations() {
        let r = report(
            "main",
            vec![scene(
                "sc-01",
                "Dawn",
                vec![
                    begin("f-a", "spoken", "ground-truth", &[]),
                    begin("f-b", "asked", "ground-truth", &[]),
                ],
            )],
            vec![
                locator("f-a", "sc-01", DisclosureMode::State),
                locator("f-b", "sc-01", DisclosureMode::Hint),
            ],
            ForkTreeReport::default(),
        );
        // A ladder that reveals BOTH offered facts; the second needs the first,
        // which is offered at the same spot (in time).
        let overrides = ladder_at(
            "sc-01",
            vec![rung("q1", "f-a", &[]), rung("q2", "f-b", &["f-a"])],
        );
        let proj = PlayableProjection::from_report(r, &overrides).unwrap();
        assert!(proj.gate("main").is_empty());
    }

    #[test]
    fn g4_an_empty_walked_spot_is_flagged() {
        // sc-02 has neither a line nor a door.
        let r = report(
            "main",
            vec![
                scene(
                    "sc-01",
                    "Dawn",
                    vec![begin("f-a", "x", "ground-truth", &[])],
                ),
                scene("sc-02", "Void", Vec::new()),
            ],
            vec![locator("f-a", "sc-01", DisclosureMode::State)],
            ForkTreeReport::default(),
        );
        let proj = PlayableProjection::from_report(r, &StaticOverrides::default()).unwrap();
        let v = proj.gate("main");
        assert_eq!(
            v,
            vec![GateViolation::EmptySpot {
                world: "main".into(),
                section: "sc-02".into(),
            }]
        );
    }

    #[test]
    fn g5_leak_a_rung_revealing_an_unoffered_fact_is_flagged() {
        // The bug: an interactive reveal names a fact the store never offers.
        let r = report(
            "main",
            vec![scene(
                "sc-01",
                "Dawn",
                vec![begin("f-a", "x", "ground-truth", &[])],
            )],
            vec![locator("f-a", "sc-01", DisclosureMode::State)],
            ForkTreeReport::default(),
        );
        // Rung 1 reveals the real f-a (no unreachable); rung 2 invents f-ghost.
        let overrides = ladder_at(
            "sc-01",
            vec![rung("q", "f-a", &[]), rung("invent?", "f-ghost", &[])],
        );
        let proj = PlayableProjection::from_report(r, &overrides).unwrap();
        let v = proj.gate("main");
        assert_eq!(
            v,
            vec![GateViolation::RungRevealsUnofferedFact {
                world: "main".into(),
                section: "sc-01".into(),
                fact_id: "f-ghost".into(),
            }]
        );
    }

    #[test]
    fn g5_unreachable_an_offered_fact_no_door_reveals_at_a_ladder_spot() {
        let r = report(
            "main",
            vec![scene(
                "sc-01",
                "Dawn",
                vec![
                    begin("f-a", "shown via ladder", "ground-truth", &[]),
                    begin("f-b", "offered but ungated", "ground-truth", &[]),
                ],
            )],
            vec![
                locator("f-a", "sc-01", DisclosureMode::State),
                locator("f-b", "sc-01", DisclosureMode::Hint),
            ],
            ForkTreeReport::default(),
        );
        // The ladder reveals f-a but NOT f-b, and no examine door covers f-b.
        let overrides = ladder_at("sc-01", vec![rung("q", "f-a", &[])]);
        let proj = PlayableProjection::from_report(r, &overrides).unwrap();
        let v = proj.gate("main");
        assert_eq!(
            v,
            vec![GateViolation::OfferedFactUnreachable {
                world: "main".into(),
                section: "sc-01".into(),
                fact_id: "f-b".into(),
            }]
        );
    }

    #[test]
    fn a_partial_ladder_suppresses_unreachable_but_still_flags_leaks() {
        // The SAME store as the modal unreachable test: f-a shown via the ladder,
        // f-b offered but no door reveals it. A PARTIAL layer (free_investigate)
        // reveals f-b via the free fallback, so f-b is NOT unreachable — while a
        // rung revealing an unoffered fact still leaks. Proves the flag suppresses
        // ONLY the modal unreachable check, never leak (non-vacuous suppression).
        let r = report(
            "main",
            vec![scene(
                "sc-01",
                "Dawn",
                vec![
                    begin("f-a", "shown via ladder", "ground-truth", &[]),
                    begin("f-b", "offered, freely investigable", "ground-truth", &[]),
                ],
            )],
            vec![
                locator("f-a", "sc-01", DisclosureMode::State),
                locator("f-b", "sc-01", DisclosureMode::Hint),
            ],
            ForkTreeReport::default(),
        );
        // Rung 1 reveals the real f-a; rung 2 invents f-ghost (a leak).
        let overrides = partial_ladder_at(
            "sc-01",
            vec![rung("q", "f-a", &[]), rung("invent?", "f-ghost", &[])],
        );
        let proj = PlayableProjection::from_report(r, &overrides).unwrap();
        let v = proj.gate("main");
        // f-b is NOT flagged unreachable — the free fallback reveals it.
        assert!(!v.iter().any(|x| matches!(
            x,
            GateViolation::OfferedFactUnreachable { fact_id, .. } if fact_id == "f-b"
        )));
        // ...but the leak still fires: free_investigate never softens leak/needs.
        assert!(v.contains(&GateViolation::RungRevealsUnofferedFact {
            world: "main".into(),
            section: "sc-01".into(),
            fact_id: "f-ghost".into(),
        }));
    }

    #[test]
    fn g6_a_precondition_offered_only_later_is_flagged() {
        let r = report(
            "main",
            vec![
                scene(
                    "sc-01",
                    "Dawn",
                    vec![begin("f-a", "x", "ground-truth", &[])],
                ),
                scene(
                    "sc-02",
                    "Noon",
                    vec![begin("f-late", "arrives later", "ground-truth", &[])],
                ),
            ],
            vec![
                locator("f-a", "sc-01", DisclosureMode::State),
                locator("f-late", "sc-02", DisclosureMode::State),
            ],
            ForkTreeReport::default(),
        );
        // A rung at sc-01 needs f-late, which is offered only at sc-02 (later) —
        // a time-reversing lock.
        let overrides = ladder_at("sc-01", vec![rung("q", "f-a", &["f-late"])]);
        let proj = PlayableProjection::from_report(r, &overrides).unwrap();
        let v = proj.gate("main");
        assert!(v.contains(&GateViolation::PreconditionUnreachable {
            world: "main".into(),
            section: "sc-01".into(),
            needs: "f-late".into(),
        }));
    }
}
