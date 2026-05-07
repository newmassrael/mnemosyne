//! 24 B fixed-width BE composite key codec — DESIGN §4 / Phase -1A stage 2C
//! decision carry: `branch_id (u64 BE 8B) || entity_id (u64 BE 8B) || valid_from (u64 BE 8B)`.
//!
//! prototype `bench/codegen-prototype/src/entity_indexer.rs` (`encode_composite_key`,
//! `decode_composite_key`) — pre-production; the bench prototype's codegen emit
//! template source; this production module is the runtime path that uses the 24 B
//! composite key encode/decode source.

use crate::error::{Result, StoreError};
use byteorder::{BigEndian, ByteOrder};

pub const KEY_LEN: usize = 24;
pub const BRANCH_ID_OFFSET: usize = 0;
pub const ENTITY_ID_OFFSET: usize = 8;
pub const VALID_FROM_OFFSET: usize = 16;

/// 24 B composite key encode — branch_id || entity_id || valid_from (all u64 BE).
#[inline]
pub fn encode_composite_key(branch_id: u64, entity_id: u64, valid_from: u64) -> [u8; KEY_LEN] {
 let mut buf = [0u8; KEY_LEN];
 BigEndian::write_u64(&mut buf[BRANCH_ID_OFFSET..ENTITY_ID_OFFSET], branch_id);
 BigEndian::write_u64(&mut buf[ENTITY_ID_OFFSET..VALID_FROM_OFFSET], entity_id);
 BigEndian::write_u64(&mut buf[VALID_FROM_OFFSET..KEY_LEN], valid_from);
 buf
}

/// 24 B composite key decode — returns `KeyLength` error if input length != 24.
#[inline]
pub fn decode_composite_key(buf: &[u8]) -> Result<(u64, u64, u64)> {
 if buf.len() != KEY_LEN {
 return Err(StoreError::KeyLength {
 expected: KEY_LEN,
 got: buf.len(),
 });
 }
 let branch_id = BigEndian::read_u64(&buf[BRANCH_ID_OFFSET..ENTITY_ID_OFFSET]);
 let entity_id = BigEndian::read_u64(&buf[ENTITY_ID_OFFSET..VALID_FROM_OFFSET]);
 let valid_from = BigEndian::read_u64(&buf[VALID_FROM_OFFSET..KEY_LEN]);
 Ok((branch_id, entity_id, valid_from))
}

/// 16 B prefix for `(branch_id, entity_id)` — iter_branch / iter_from prefix scan source.
#[inline]
pub fn encode_branch_entity_prefix(branch_id: u64, entity_id: u64) -> [u8; 16] {
 let mut buf = [0u8; 16];
 BigEndian::write_u64(&mut buf[0..8], branch_id);
 BigEndian::write_u64(&mut buf[8..16], entity_id);
 buf
}

/// 8 B `branch_id` prefix — branch-wide scan key.
#[inline]
pub fn encode_branch_prefix(branch_id: u64) -> [u8; 8] {
 let mut buf = [0u8; 8];
 BigEndian::write_u64(&mut buf, branch_id);
 buf
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn round_trip_extremes() {
 let cases: &[(u64, u64, u64)] = &[
 (0, 0, 0),
 (1, 1, 1),
 (u64::MAX, u64::MAX, u64::MAX),
 (u64::MAX / 2, u64::MAX / 2, u64::MAX / 2),
 (1, 2, 3),
 ];
 for &(b, e, v) in cases {
 let buf = encode_composite_key(b, e, v);
 assert_eq!(buf.len(), KEY_LEN);
 let (b2, e2, v2) = decode_composite_key(&buf).expect("decode");
 assert_eq!((b, e, v), (b2, e2, v2));
 }
 }

 #[test]
 fn lex_order_matches_numeric() {
 let k1 = encode_composite_key(1, 1, 100);
 let k2 = encode_composite_key(1, 1, 200);
 let k3 = encode_composite_key(1, 2, 50);
 let k4 = encode_composite_key(2, 0, 0);
 assert!(k1 < k2);
 assert!(k2 < k3);
 assert!(k3 < k4);
 }

 #[test]
 fn decode_rejects_wrong_length() {
 let too_short = [0u8; 23];
 assert!(matches!(
 decode_composite_key(&too_short),
 Err(StoreError::KeyLength { expected: 24, got: 23 })
 ));
 let too_long = [0u8; 25];
 assert!(matches!(
 decode_composite_key(&too_long),
 Err(StoreError::KeyLength { expected: 24, got: 25 })
 ));
 }

 #[test]
 fn prefix_helpers_match_full_key() {
 let full = encode_composite_key(7, 11, 99);
 let p16 = encode_branch_entity_prefix(7, 11);
 let p8 = encode_branch_prefix(7);
 assert_eq!(&full[..16], &p16[..]);
 assert_eq!(&full[..8], &p8[..]);
 }
}
