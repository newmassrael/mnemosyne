//! Registry id types (Round 838 design, Round 839 first migration).
//!
//! # Why these exist
//!
//! Every id in this store was a `String`, and which registry a given `String`
//! keys was written nowhere a compiler could read. Round 833 found four registry
//! refs nested inside a section that no scan checked; Round 837 built a
//! ninety-four-line table naming, per field, which gate covers it. That table
//! catches an UNCLASSIFIED field and can never catch a MISCLASSIFIED one — and
//! it can never catch a check that resolves a ref against the WRONG registry,
//! because to a `String` every registry looks alike.
//!
//! A type says it in the one place that cannot drift from the value.
//!
//! # What this buys, and what it does not
//!
//! It buys ASSIGNMENT and FIELD position: a fact's frame cannot be given an
//! entity id, and a detector cannot resolve a unit against the predicates map
//! without saying so out loud.
//!
//! It does NOT buy LOOKUP. [`Borrow<str>`] is implemented deliberately — of 132
//! `contains_key` calls in this workspace 53 pass a string literal, and removing
//! the impl converts every one into a constructor call that adds noise without
//! adding safety. The consequence is honest: `entities.contains_key(unit.as_str())`
//! still compiles. Closing that is a typed accessor per registry, not the removal
//! of `Borrow` — follow-on work, recorded rather than pretended away.
//!
//! # Why the wire is unchanged
//!
//! `#[serde(transparent)]`, measured on five properties before this landed: the
//! JSON is byte-identical in field position AND as a map key, a newtype map key
//! deserializes, `contains_key("literal")` survives, the emitted JSON Schema for
//! a field is unchanged, and — the one that would have been expensive to miss —
//! the derived `Ord` matches `String`'s, so every `BTreeMap` walk and every
//! deterministic-order claim in this codebase stays where it was.

use std::borrow::Borrow;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Declare a registry id. ONE macro so the trait set cannot diverge between
/// ids — nine of these arrive over the migration, and a set that differs per id
/// is the drift this whole change exists to remove.
macro_rules! ref_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        pub struct $name(String);

        impl $name {
            /// The id as a string slice — the read path for formatting, and for
            /// the `contains_key` calls that still take `&str`.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume into the inner `String`, for the boundaries that still
            /// hand `String` onward (wire DTOs, report rows).
            #[must_use]
            pub fn into_inner(self) -> String {
                self.0
            }

            /// An UNSET id. Several fields spell "no kind declared" as the empty
            /// string and skip it on the wire, so this keeps
            /// `skip_serializing_if` working after the type change — a missing
            /// `is_empty` would otherwise force those fields back to `String`
            /// and reopen exactly the hole this migration closes.
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        /// So `map.contains_key("literal")` and `map.get(s: &str)` keep working
        /// against a map keyed by this type. See the module docs for what this
        /// concedes.
        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        /// Comparison against a bare `&str` without constructing an id — the
        /// error messages and tests compare against literals constantly.
        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        /// And against an owned `String` — a configured rule or a wire DTO
        /// holds one, and comparing it should not need a conversion at the
        /// comparison site (Round 841).
        impl PartialEq<String> for $name {
            fn eq(&self, other: &String) -> bool {
                self.0 == *other
            }
        }
    };
}

ref_id! {
    /// A key of the units registry ([`crate::Unit`]).
    UnitId
}

ref_id! {
    /// A key of the parameters registry ([`crate::Parameter`]).
    ParameterId
}

ref_id! {
    /// A key of the predicates registry ([`crate::Predicate`]) — the rule a
    /// typed claim keys off, so a typo here silently escapes its rule.
    PredicateId
}

ref_id! {
    /// A key of the entity-kinds registry ([`crate::EntityKind`]).
    ///
    /// The first id to appear in FOUR fields across three registries —
    /// `Entity::kind`, `EntityKind::parents`, `Predicate::subject_kind` and
    /// `Predicate::object_entity_kind`. Before this, nothing in the types said
    /// those four hold the same vocabulary.
    EntityKindId
}

ref_id! {
    /// A key of the frames registry ([`crate::Frame`]) — the epistemic axis a
    /// fact is believed on.
    ///
    /// Note what this type does NOT cover: `NarrativeFact::supersedes_in_frame`
    /// holds a FACT id despite its name, so it stays outside this type and
    /// arrives with `FactId`. The name said "frame" and the value never was
    /// one — which is the drift a type removes.
    FrameId
}

ref_id! {
    /// A key of the entities registry ([`crate::Entity`]) — the id every claim
    /// about a character, place or object is retrieved by.
    ///
    /// The widest reach of the arc so far: seven fields across five types hold
    /// one, and two of them (`TypedClaim::subject`, `TypedObject::Entity`) sit
    /// beside a predicate id and an entity-kind id in the same struct.
    EntityId
}

ref_id! {
    /// A key of the branches registry ([`crate::Branch`]) — one world-line of
    /// the playthrough graph.
    ///
    /// [`crate::MAIN_BRANCH`] stays a `&str` const: it is the DEFAULT axis
    /// value, known by construction and never registered, so there is no
    /// registry entry for it to be a key of. `PartialEq<&str>` is what keeps
    /// `branch == MAIN_BRANCH` readable at the ~40 sites that ask it.
    BranchId
}

ref_id! {
    /// A key of the narrative-facts registry ([`crate::NarrativeFact`]).
    ///
    /// The id `NarrativeFact::supersedes_in_frame` has held since Round 434
    /// despite its name saying "frame" — Round 843 documented that on
    /// [`FrameId`] and left the field untyped until this type existed.
    FactId
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// The wire is unchanged — the property the whole migration rests on, kept
    /// as a test rather than only as a measurement in a changelog entry.
    #[test]
    fn a_ref_id_is_wire_identical_to_the_string_it_replaces() {
        #[derive(Serialize, Deserialize)]
        struct Before {
            unit: String,
            by_unit: BTreeMap<String, i64>,
        }
        #[derive(Serialize, Deserialize)]
        struct After {
            unit: UnitId,
            by_unit: BTreeMap<UnitId, i64>,
        }
        let json = r#"{"unit":"day","by_unit":{"day":1,"won":2}}"#;
        let before: Before = serde_json::from_str(json).expect("before parses");
        let after: After = serde_json::from_str(json).expect("after parses the SAME json");
        assert_eq!(
            serde_json::to_string(&before).unwrap(),
            serde_json::to_string(&after).unwrap(),
            "a transparent ref id must serialize byte-identically, in field AND \
             map-key position — this is what makes the migration schema-0"
        );
        // The lookup path 53 call sites depend on.
        assert!(after.by_unit.contains_key("day"));
    }

    /// A different `Ord` would silently reorder every `BTreeMap` walk in the
    /// workspace, and with it every "deterministic order" this project claims.
    #[test]
    fn a_ref_id_sorts_exactly_as_its_string_does() {
        let raw = ["won", "day", "Day", "unit-10", "unit-2", ""];
        let mut strings: Vec<String> = raw.iter().map(|s| (*s).to_string()).collect();
        let mut ids: Vec<UnitId> = raw.iter().map(|s| UnitId::from(*s)).collect();
        strings.sort();
        ids.sort();
        let back: Vec<String> = ids.into_iter().map(UnitId::into_inner).collect();
        assert_eq!(strings, back);
    }
}
