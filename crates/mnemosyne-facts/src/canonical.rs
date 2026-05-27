//! Canonical identifier set + sha256 + Jaccard inclusion — cross-language emit
//! conformance source.

use crate::schema::GraphSpec;
use std::collections::BTreeSet;

/// SHA-256 hex digest — content-addressable stable hash.
pub fn sha256_hex(s: &str) -> String {
 use sha2::{Digest, Sha256};
 let mut hasher = Sha256::new();
 hasher.update(s.as_bytes());
 let digest = hasher.finalize();
 let mut hex = String::with_capacity(64);
 for byte in digest.iter() {
 hex.push_str(&format!("{:02x}", byte));
 }
 hex
}

/// GraphSpec → canonical identifier set (entity + relation + field names + composite-key fields).
/// 5-language emit must include every identifier in this set.
pub fn canonical_identifier_set(spec: &GraphSpec) -> BTreeSet<String> {
 let mut set = BTreeSet::new();
 for entity in &spec.entities {
 set.insert(entity.name.clone());
 set.insert(entity.key.branch_field.clone());
 set.insert(entity.key.entity_field.clone());
 set.insert(entity.key.valid_from_field.clone());
 for field in &entity.fields {
 set.insert(field.name.clone());
 }
 }
 for relation in &spec.relations {
 set.insert(relation.name.clone());
 for field in &relation.fields {
 set.insert(field.name.clone());
 }
 }
 set
}

/// Substring-presence Jaccard inclusion lower bound. Identifiers are unique
/// alphanumerics with no overlap, so substring check yields the exact ratio in
/// practice. Returns 1.0 for empty canonical sets.
pub fn jaccard_inclusion(emit_text: &str, canonical: &BTreeSet<String>) -> f64 {
 if canonical.is_empty() {
 return 1.0;
 }
 let included = canonical
 .iter()
 .filter(|id| emit_text.contains(id.as_str()))
 .count();
 included as f64 / canonical.len() as f64
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn sha256_hex_known_vector() {
 assert_eq!(
 sha256_hex("hello"),
 "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
 );
 }

 #[test]
 fn jaccard_one_when_all_identifiers_present() {
 let mut canonical = BTreeSet::new();
 canonical.insert("Section".to_string());
 canonical.insert("CrossRef".to_string());
 let text = "data Section + relation CrossRef";
 assert!((jaccard_inclusion(text, &canonical) - 1.0).abs() < f64::EPSILON);
 }

 #[test]
 fn jaccard_below_one_when_identifier_missing() {
 let mut canonical = BTreeSet::new();
 canonical.insert("Section".to_string());
 canonical.insert("CrossRef".to_string());
 let text = "Section only";
 assert!((jaccard_inclusion(text, &canonical) - 0.5).abs() < f64::EPSILON);
 }

 #[test]
 fn empty_canonical_returns_one() {
 let empty = BTreeSet::new();
 assert!((jaccard_inclusion("anything", &empty) - 1.0).abs() < f64::EPSILON);
 }
}
