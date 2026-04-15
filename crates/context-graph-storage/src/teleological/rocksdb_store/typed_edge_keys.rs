//! Shared composite-key helpers for the typed-edge column families.
//!
//! Both `CF_TYPED_EDGE_RECORDS` (F1 export corpus) and
//! `CF_TYPED_EDGE_VALIDATIONS` (F4 LLM verdicts) key each row by
//! `[source_uuid: 16B][target_uuid: 16B][edge_type: u8]` = 33 bytes. The
//! layout is kept in a single module so the two CFs stay byte-for-byte
//! compatible and a future migration only needs to change one place.

use uuid::Uuid;

/// Composite key length for the typed-edge CFs (`CF_TYPED_EDGE_RECORDS` and
/// `CF_TYPED_EDGE_VALIDATIONS`): `[source:16][target:16][edge_type:u8]` = 33
/// bytes.
pub(crate) const TYPED_EDGE_KEY_LEN: usize = 33;

/// Build the canonical 33-byte composite key
/// `[source:16][target:16][edge_type:u8]`.
///
/// Used by every CRUD path touching `CF_TYPED_EDGE_RECORDS` or
/// `CF_TYPED_EDGE_VALIDATIONS` so the two CFs are keyed identically.
#[inline]
pub fn typed_edge_record_key(
    source: Uuid,
    target: Uuid,
    edge_type: u8,
) -> [u8; TYPED_EDGE_KEY_LEN] {
    let mut k = [0u8; TYPED_EDGE_KEY_LEN];
    k[..16].copy_from_slice(source.as_bytes());
    k[16..32].copy_from_slice(target.as_bytes());
    k[32] = edge_type;
    k
}

/// Parse a 33-byte composite key back into `(source, target, edge_type)`.
///
/// Returns `None` when the slice length is not [`TYPED_EDGE_KEY_LEN`]; callers
/// are expected to surface that as a structured `SerializationError` with
/// enough context for a human to diagnose (CF name, length, etc.).
#[inline]
pub fn parse_typed_edge_record_key(key: &[u8]) -> Option<(Uuid, Uuid, u8)> {
    if key.len() != TYPED_EDGE_KEY_LEN {
        return None;
    }
    let mut src = [0u8; 16];
    src.copy_from_slice(&key[..16]);
    let mut tgt = [0u8; 16];
    tgt.copy_from_slice(&key[16..32]);
    Some((Uuid::from_bytes(src), Uuid::from_bytes(tgt), key[32]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_parse() {
        let s = Uuid::new_v4();
        let t = Uuid::new_v4();
        let k = typed_edge_record_key(s, t, 7);
        assert_eq!(k.len(), TYPED_EDGE_KEY_LEN);
        let (s2, t2, et) = parse_typed_edge_record_key(&k).expect("parse");
        assert_eq!(s2, s);
        assert_eq!(t2, t);
        assert_eq!(et, 7);
    }

    #[test]
    fn parse_rejects_wrong_length() {
        assert!(parse_typed_edge_record_key(&[0u8; 32]).is_none());
        assert!(parse_typed_edge_record_key(&[0u8; 34]).is_none());
        assert!(parse_typed_edge_record_key(&[]).is_none());
    }

    #[test]
    fn key_layout_is_deterministic() {
        let s = Uuid::new_v4();
        let t = Uuid::new_v4();
        let k = typed_edge_record_key(s, t, 3);
        assert_eq!(&k[..16], s.as_bytes());
        assert_eq!(&k[16..32], t.as_bytes());
        assert_eq!(k[32], 3);
    }
}
