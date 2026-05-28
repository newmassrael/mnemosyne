//! Typed-fact instance encoding — Section / ChangelogEntry / FrozenList / CrossRef.
//!
//! Each fact carries its composite-key components (branch_id / entity_id /
//! valid_from) plus the entity-specific payload. Persistence layout: composite
//! key encoded by `mnemosyne_store::encode_composite_key`, value encoded as
//! length-prefixed UTF-8 string fields + u64 BE numeric fields.
//!
//! Wire format keeps deterministic byte layout so identical facts produce
//! Identical bytes — required for content-addressable hashing and audit.
//! comparison.

use byteorder::{BigEndian, ByteOrder};
use serde::{Deserialize, Serialize};
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

// ────────────────────────────────────────────────────────────────────────────
// SectionFact — Section entity instance (production typed fact).
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionFact {
    pub branch_id: u64,
    pub entity_id: u64,
    pub valid_from: u64,
    pub doc_path: String,
    pub section_id: String,
    pub title: String,
    pub decision_status: String,
}

impl SectionFact {
    pub fn encode_value(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            16 + self.doc_path.len()
                + self.section_id.len()
                + self.title.len()
                + self.decision_status.len(),
        );
        write_string(&mut out, &self.doc_path);
        write_string(&mut out, &self.section_id);
        write_string(&mut out, &self.title);
        write_string(&mut out, &self.decision_status);
        out
    }

    pub fn decode_value(
        branch_id: u64,
        entity_id: u64,
        valid_from: u64,
        buf: &[u8],
    ) -> Result<Self, FactCodecError> {
        let mut cursor = 0;
        let doc_path = read_string(buf, &mut cursor, "doc_path")?;
        let section_id = read_string(buf, &mut cursor, "section_id")?;
        let title = read_string(buf, &mut cursor, "title")?;
        let decision_status = read_string(buf, &mut cursor, "decision_status")?;
        Ok(Self {
            branch_id,
            entity_id,
            valid_from,
            doc_path,
            section_id,
            title,
            decision_status,
        })
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ChangelogEntryFact.
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangelogEntryFact {
    pub branch_id: u64,
    pub entity_id: u64,
    pub valid_from: u64,
    pub round_number: u64,
    pub summary: String,
    pub appended_at: u64,
}

impl ChangelogEntryFact {
    pub fn encode_value(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 4 + self.summary.len());
        write_u64(&mut out, self.round_number);
        write_string(&mut out, &self.summary);
        write_u64(&mut out, self.appended_at);
        out
    }

    pub fn decode_value(
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
            branch_id,
            entity_id,
            valid_from,
            round_number,
            summary,
            appended_at,
        })
    }
}

// ────────────────────────────────────────────────────────────────────────────
// FrozenListFact.
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrozenListFact {
    pub branch_id: u64,
    pub entity_id: u64,
    pub valid_from: u64,
    /// Owner section entity_id — EntityRef target=Section.
    pub owner_section: u64,
    pub frozen_round: u64,
    pub kind: String,
}

impl FrozenListFact {
    pub fn encode_value(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 4 + self.kind.len());
        write_u64(&mut out, self.owner_section);
        write_u64(&mut out, self.frozen_round);
        write_string(&mut out, &self.kind);
        out
    }

    pub fn decode_value(
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
            branch_id,
            entity_id,
            valid_from,
            owner_section,
            frozen_round,
            kind,
        })
    }
}

// ────────────────────────────────────────────────────────────────────────────
// CrossRefFact — relation (Section→Section).
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrossRefFact {
    pub branch_id: u64,
    /// Source section entity_id.
    pub from_section: u64,
    /// Target section entity_id (carried in `valid_from` slot of the composite key).
    pub to_section: u64,
    pub ref_kind: String,
}

impl CrossRefFact {
    pub fn encode_value(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.ref_kind.len());
        write_string(&mut out, &self.ref_kind);
        out
    }

    pub fn decode_value(
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
        let fact = SectionFact {
            branch_id: 1,
            entity_id: 42,
            valid_from: 1000,
            doc_path: "docs/DESIGN.md".to_string(),
            section_id: "39".to_string(),
            title: "Phase 0 design_doc schema".to_string(),
            decision_status: "Active".to_string(),
        };
        let bytes = fact.encode_value();
        let decoded = SectionFact::decode_value(1, 42, 1000, &bytes).expect("decode");
        assert_eq!(decoded, fact);
    }

    #[test]
    fn changelog_entry_round_trip() {
        let fact = ChangelogEntryFact {
            branch_id: 1,
            entity_id: 73,
            valid_from: 2026_05_03,
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
            branch_id: 1,
            entity_id: 100,
            valid_from: 1000,
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
            branch_id: 1,
            entity_id: 42,
            valid_from: 1000,
            doc_path: "docs/DESIGN.md".to_string(),
            section_id: "39".to_string(),
            title: "Test".to_string(),
            decision_status: "Active".to_string(),
        };
        let a = fact.encode_value();
        let b = fact.encode_value();
        assert_eq!(a, b);
    }
}
