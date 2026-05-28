//! Derived-index byte codec for the canonical typed facts.
//!
//! The fact *structs* (Section / ChangelogEntry / FrozenList / CrossRef) live
//! in `mnemosyne-core` (Layer 0 — the one canonical fact model). This module
//! owns only the *index encoding*: how those structs are serialized into the
//! RocksDB materialized index. Encoding is a persistence-layer concern, so it
//! stays out of the domain core (Round 328 — Convergence B prerequisite).
//!
//! Persistence layout: composite key encoded by
//! `mnemosyne_store::encode_composite_key`, value encoded as length-prefixed
//! UTF-8 string fields + u64 BE numeric fields.
//!
//! Wire format keeps a deterministic byte layout so identical facts produce
//! identical bytes — required for content-addressable hashing and audit
//! comparison.

use byteorder::{BigEndian, ByteOrder};
use mnemosyne_core::{
    ChangelogEntryFact, CrossRefFact, DecisionStatus, FactKey, FrozenListFact, SectionFact,
    SectionSkeleton,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FactCodecError {
    #[error("buffer too short: expected {expected} bytes, got {got}")]
    Truncated { expected: usize, got: usize },
    #[error("invalid utf-8 in field `{field}`: {source}")]
    InvalidUtf8 {
        field: &'static str,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("unknown discriminator byte 0x{0:02x} for field `{1}`")]
    UnknownDiscriminator(u8, &'static str),
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let mut len_buf = [0u8; 4];
    BigEndian::write_u32(&mut len_buf, bytes.len() as u32);
    out.extend_from_slice(&len_buf);
    out.extend_from_slice(bytes);
}

fn read_string(
    buf: &[u8],
    cursor: &mut usize,
    field: &'static str,
) -> Result<String, FactCodecError> {
    let start = *cursor;
    if buf.len() < start + 4 {
        return Err(FactCodecError::Truncated {
            expected: start + 4,
            got: buf.len(),
        });
    }
    let len = BigEndian::read_u32(&buf[start..start + 4]) as usize;
    let str_start = start + 4;
    let str_end = str_start + len;
    if buf.len() < str_end {
        return Err(FactCodecError::Truncated {
            expected: str_end,
            got: buf.len(),
        });
    }
    let bytes = buf[str_start..str_end].to_vec();
    *cursor = str_end;
    String::from_utf8(bytes).map_err(|source| FactCodecError::InvalidUtf8 { field, source })
}

fn write_u64(out: &mut Vec<u8>, v: u64) {
    let mut buf = [0u8; 8];
    BigEndian::write_u64(&mut buf, v);
    out.extend_from_slice(&buf);
}

fn read_u64(buf: &[u8], cursor: &mut usize) -> Result<u64, FactCodecError> {
    let start = *cursor;
    if buf.len() < start + 8 {
        return Err(FactCodecError::Truncated {
            expected: start + 8,
            got: buf.len(),
        });
    }
    let v = BigEndian::read_u64(&buf[start..start + 8]);
    *cursor = start + 8;
    Ok(v)
}

fn read_u8(buf: &[u8], cursor: &mut usize) -> Result<u8, FactCodecError> {
    let start = *cursor;
    if buf.len() < start + 1 {
        return Err(FactCodecError::Truncated {
            expected: start + 1,
            got: buf.len(),
        });
    }
    let v = buf[start];
    *cursor = start + 1;
    Ok(v)
}

/// `Option<String>` codec: one discriminator byte (0 = None, 1 = Some) then,
/// when present, the length-prefixed string.
fn write_opt_string(out: &mut Vec<u8>, s: Option<&str>) {
    match s {
        None => out.push(0),
        Some(v) => {
            out.push(1);
            write_string(out, v);
        }
    }
}

fn read_opt_string(
    buf: &[u8],
    cursor: &mut usize,
    field: &'static str,
) -> Result<Option<String>, FactCodecError> {
    match read_u8(buf, cursor)? {
        0 => Ok(None),
        1 => Ok(Some(read_string(buf, cursor, field)?)),
        other => Err(FactCodecError::UnknownDiscriminator(other, field)),
    }
}

/// `Option<DecisionStatus>` codec: a single discriminator byte. The typed enum
/// replaces the pre-Round-326 stringly-typed status field.
fn encode_decision_status(status: Option<DecisionStatus>) -> u8 {
    match status {
        None => 0,
        Some(DecisionStatus::Active) => 1,
        Some(DecisionStatus::Superseded) => 2,
        Some(DecisionStatus::Removed) => 3,
    }
}

fn read_decision_status(
    buf: &[u8],
    cursor: &mut usize,
) -> Result<Option<DecisionStatus>, FactCodecError> {
    match read_u8(buf, cursor)? {
        0 => Ok(None),
        1 => Ok(Some(DecisionStatus::Active)),
        2 => Ok(Some(DecisionStatus::Superseded)),
        3 => Ok(Some(DecisionStatus::Removed)),
        other => Err(FactCodecError::UnknownDiscriminator(
            other,
            "decision_status",
        )),
    }
}

/// Derived-index byte codec for a canonical typed fact.
///
/// Implemented in this persistence-layer crate (not in the domain core where
/// the structs live) so Layer 0 carries no byte-layout concern. The three key
/// slots passed to [`IndexCodec::decode_value`] are the composite-key
/// components: for entity facts `(branch_id, entity_id, valid_from)`; for the
/// `CrossRef` relation `(branch_id, from_section, to_section)`.
pub trait IndexCodec: Sized {
    fn encode_value(&self) -> Vec<u8>;
    fn decode_value(k0: u64, k1: u64, k2: u64, buf: &[u8]) -> Result<Self, FactCodecError>;
}

impl IndexCodec for SectionFact {
    fn encode_value(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_string(&mut out, &self.section_id);
        write_string(&mut out, &self.skeleton.parent_doc);
        write_string(&mut out, &self.skeleton.title);
        write_opt_string(&mut out, self.skeleton.parent_section.as_deref());
        out.push(encode_decision_status(self.skeleton.decision_status));
        out
    }

    fn decode_value(
        branch_id: u64,
        entity_id: u64,
        valid_from: u64,
        buf: &[u8],
    ) -> Result<Self, FactCodecError> {
        let mut cursor = 0;
        let section_id = read_string(buf, &mut cursor, "section_id")?;
        let parent_doc = read_string(buf, &mut cursor, "parent_doc")?;
        let title = read_string(buf, &mut cursor, "title")?;
        let parent_section = read_opt_string(buf, &mut cursor, "parent_section")?;
        let decision_status = read_decision_status(buf, &mut cursor)?;
        Ok(Self {
            key: FactKey {
                branch_id,
                entity_id,
                valid_from,
            },
            section_id,
            skeleton: SectionSkeleton {
                title,
                parent_doc,
                parent_section,
                decision_status,
            },
        })
    }
}

impl IndexCodec for ChangelogEntryFact {
    fn encode_value(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 4 + self.summary.len());
        write_u64(&mut out, self.round_number);
        write_string(&mut out, &self.summary);
        write_u64(&mut out, self.appended_at);
        out
    }

    fn decode_value(
        branch_id: u64,
        entity_id: u64,
        valid_from: u64,
        buf: &[u8],
    ) -> Result<Self, FactCodecError> {
        let mut cursor = 0;
        let round_number = read_u64(buf, &mut cursor)?;
        let summary = read_string(buf, &mut cursor, "summary")?;
        let appended_at = read_u64(buf, &mut cursor)?;
        Ok(Self {
            key: FactKey {
                branch_id,
                entity_id,
                valid_from,
            },
            round_number,
            summary,
            appended_at,
        })
    }
}

impl IndexCodec for FrozenListFact {
    fn encode_value(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 4 + self.kind.len());
        write_u64(&mut out, self.owner_section);
        write_u64(&mut out, self.frozen_round);
        write_string(&mut out, &self.kind);
        out
    }

    fn decode_value(
        branch_id: u64,
        entity_id: u64,
        valid_from: u64,
        buf: &[u8],
    ) -> Result<Self, FactCodecError> {
        let mut cursor = 0;
        let owner_section = read_u64(buf, &mut cursor)?;
        let frozen_round = read_u64(buf, &mut cursor)?;
        let kind = read_string(buf, &mut cursor, "kind")?;
        Ok(Self {
            key: FactKey {
                branch_id,
                entity_id,
                valid_from,
            },
            owner_section,
            frozen_round,
            kind,
        })
    }
}

impl IndexCodec for CrossRefFact {
    fn encode_value(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.ref_kind.len());
        write_string(&mut out, &self.ref_kind);
        out
    }

    fn decode_value(
        branch_id: u64,
        from_section: u64,
        to_section: u64,
        buf: &[u8],
    ) -> Result<Self, FactCodecError> {
        let mut cursor = 0;
        let ref_kind = read_string(buf, &mut cursor, "ref_kind")?;
        Ok(Self {
            branch_id,
            from_section,
            to_section,
            ref_kind,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_fact_round_trip() {
        // Exercises both discriminator codecs: parent_section Some + a typed
        // decision_status.
        let fact = SectionFact {
            key: FactKey {
                branch_id: 1,
                entity_id: 42,
                valid_from: 1000,
            },
            section_id: "39".to_string(),
            skeleton: SectionSkeleton {
                title: "Phase 0 design_doc schema".to_string(),
                parent_doc: "docs/DESIGN.md".to_string(),
                parent_section: Some("38".to_string()),
                decision_status: Some(DecisionStatus::Superseded),
            },
        };
        let bytes = fact.encode_value();
        let decoded = SectionFact::decode_value(1, 42, 1000, &bytes).expect("decode");
        assert_eq!(decoded, fact);
    }

    #[test]
    fn section_fact_round_trip_defaults() {
        // None parent_section + None decision_status (the empty-skeleton path).
        let fact = SectionFact {
            key: FactKey {
                branch_id: 1,
                entity_id: 7,
                valid_from: 1,
            },
            section_id: "7".to_string(),
            skeleton: SectionSkeleton::default(),
        };
        let bytes = fact.encode_value();
        let decoded = SectionFact::decode_value(1, 7, 1, &bytes).expect("decode");
        assert_eq!(decoded, fact);
    }

    #[test]
    fn changelog_entry_round_trip() {
        let fact = ChangelogEntryFact {
            key: FactKey {
                branch_id: 1,
                entity_id: 73,
                valid_from: 2026_05_03,
            },
            round_number: 73,
            summary: "OPTION B-2 mnemosyne-store production".to_string(),
            appended_at: 2026_05_03_12_30,
        };
        let bytes = fact.encode_value();
        let decoded = ChangelogEntryFact::decode_value(1, 73, 2026_05_03, &bytes).expect("decode");
        assert_eq!(decoded, fact);
    }

    #[test]
    fn frozen_list_round_trip() {
        let fact = FrozenListFact {
            key: FactKey {
                branch_id: 1,
                entity_id: 100,
                valid_from: 1000,
            },
            owner_section: 39,
            frozen_round: 60,
            kind: "release_lock".to_string(),
        };
        let bytes = fact.encode_value();
        let decoded = FrozenListFact::decode_value(1, 100, 1000, &bytes).expect("decode");
        assert_eq!(decoded, fact);
    }

    #[test]
    fn cross_ref_round_trip() {
        let fact = CrossRefFact {
            branch_id: 1,
            from_section: 66,
            to_section: 39,
            ref_kind: "decision".to_string(),
        };
        let bytes = fact.encode_value();
        let decoded = CrossRefFact::decode_value(1, 66, 39, &bytes).expect("decode");
        assert_eq!(decoded, fact);
    }

    #[test]
    fn truncated_buffer_rejected() {
        let bytes = vec![0u8, 0, 0, 5, b'h', b'e']; // 5-byte string, only 2 bytes follow
        let err = SectionFact::decode_value(1, 1, 1, &bytes).expect_err("truncate");
        assert!(matches!(err, FactCodecError::Truncated { .. }));
    }

    #[test]
    fn deterministic_encoding() {
        let fact = SectionFact {
            key: FactKey {
                branch_id: 1,
                entity_id: 42,
                valid_from: 1000,
            },
            section_id: "39".to_string(),
            skeleton: SectionSkeleton {
                title: "Test".to_string(),
                parent_doc: "docs/DESIGN.md".to_string(),
                parent_section: None,
                decision_status: Some(DecisionStatus::Active),
            },
        };
        let a = fact.encode_value();
        let b = fact.encode_value();
        assert_eq!(a, b);
    }
}
