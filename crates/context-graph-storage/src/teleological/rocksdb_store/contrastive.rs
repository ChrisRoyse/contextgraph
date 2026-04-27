//! Contrastive pair persistence for the Phase 3 miner.
//!
//! All records land in `CF_CONTRASTIVE_PAIRS` using the canonical
//! `[CONTRASTIVE_PAIR_VERSION: u8][bincode-encoded ContrastivePair]` layout.
//! Two secondary indexes:
//!
//! - `CF_CONTRASTIVE_BY_KIND` — key `[kind: u8][anchor: 16B][neg: 16B]`
//!   (33 bytes), empty value — prefix scan returns every pair of a given
//!   anomaly kind.
//! - `CF_CONTRASTIVE_BY_ANCHOR` — key `anchor_uuid: 16B`, value
//!   `bincode(Vec<Uuid>)` — the list of every negative mined against that
//!   anchor (append-on-write; duplicates are deduped on read).
//!
//! # Atomicity
//!
//! `store_contrastive_pair` writes the primary record and both secondary
//! index entries in a single `WriteBatch`. `delete_contrastive_pair` mirrors
//! the pattern.
//!
//! # Version handling
//!
//! The version byte is [`CONTRASTIVE_PAIR_VERSION`]. Deserialization rejects
//! mismatched versions with `CoreError::SerializationError` — no automatic
//! migration.
//!
//! # FAIL FAST
//!
//! CF handles use `.expect(...)`; RocksDB errors propagate via
//! [`TeleologicalStoreError::rocksdb_op`] with operation + CF + key context.

use std::collections::HashSet;

use context_graph_core::contrastive::{AnomalyKind, ContrastivePair, CONTRASTIVE_PAIR_VERSION};
use context_graph_core::error::{CoreError, CoreResult};
use rocksdb::{ColumnFamily, IteratorMode, ReadOptions, WriteBatch};
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::teleological::column_families::{
    CF_CONTRASTIVE_BY_ANCHOR, CF_CONTRASTIVE_BY_KIND, CF_CONTRASTIVE_PAIRS,
};

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

/// Composite primary-key length: `[anchor:16][neg:16]` = 32 bytes.
const PAIR_KEY_LEN: usize = 32;

/// Secondary by-kind key length: `[kind:1][anchor:16][neg:16]` = 33 bytes.
const BY_KIND_KEY_LEN: usize = 33;

impl RocksDbTeleologicalStore {
    #[inline]
    fn cf_contrastive_pairs(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONTRASTIVE_PAIRS)
            .expect("CF_CONTRASTIVE_PAIRS must exist — database initialization failed")
    }

    #[inline]
    fn cf_contrastive_by_kind(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONTRASTIVE_BY_KIND)
            .expect("CF_CONTRASTIVE_BY_KIND must exist — database initialization failed")
    }

    #[inline]
    fn cf_contrastive_by_anchor(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONTRASTIVE_BY_ANCHOR)
            .expect("CF_CONTRASTIVE_BY_ANCHOR must exist — database initialization failed")
    }

    /// Persist a contrastive pair atomically across all three CFs.
    ///
    /// Idempotent for `(anchor, negative)` — re-storing overwrites the
    /// primary record and re-emits the secondary index entries.
    pub async fn store_contrastive_pair(&self, pair: &ContrastivePair) -> CoreResult<()> {
        let primary_key = pair_primary_key(pair.anchor_id, pair.negative_id);
        let by_kind_key = by_kind_key(pair.anomaly_kind, pair.anchor_id, pair.negative_id);
        let payload = encode_pair(pair)?;

        // Merge the new negative into the existing anchor → Vec<Uuid> row.
        // The read happens outside the WriteBatch; concurrent writers can
        // race but the composite primary key provides the correctness
        // guarantee — duplicates on the anchor list are deduplicated on
        // read, not on write.
        let anchor_cf = self.cf_contrastive_by_anchor();
        let anchor_key = pair.anchor_id.as_bytes();
        let existing: Vec<Uuid> = match self.db.get_cf(anchor_cf, anchor_key) {
            Ok(Some(bytes)) => decode_anchor_negatives(&bytes).unwrap_or_default(),
            Ok(None) => Vec::new(),
            Err(e) => {
                error!(
                    anchor = %pair.anchor_id,
                    error = %e,
                    "ROCKSDB ERROR: Failed to read anchor negatives during store"
                );
                return Err(TeleologicalStoreError::rocksdb_op(
                    "read_contrastive_by_anchor",
                    CF_CONTRASTIVE_BY_ANCHOR,
                    Some(pair.anchor_id),
                    e,
                )
                .into());
            }
        };
        let anchor_value = append_anchor_negative(&existing, pair.negative_id)?;

        let primary_cf = self.cf_contrastive_pairs();
        let kind_cf = self.cf_contrastive_by_kind();

        let mut batch = WriteBatch::default();
        batch.put_cf(primary_cf, primary_key, &payload);
        batch.put_cf(kind_cf, by_kind_key, &[] as &[u8]);
        batch.put_cf(anchor_cf, anchor_key, &anchor_value);

        self.db.write(batch).map_err(|e| {
            error!(
                anchor = %pair.anchor_id,
                negative = %pair.negative_id,
                error = %e,
                "ROCKSDB ERROR: Failed to store contrastive pair (atomic batch)"
            );
            TeleologicalStoreError::rocksdb_op(
                "store_contrastive_pair",
                CF_CONTRASTIVE_PAIRS,
                Some(pair.anchor_id),
                e,
            )
        })?;

        debug!(
            anchor = %pair.anchor_id,
            negative = %pair.negative_id,
            kind = ?pair.anomaly_kind,
            bytes = payload.len(),
            "Stored contrastive pair + secondary indexes"
        );
        Ok(())
    }

    /// Retrieve a contrastive pair by composite `(anchor, negative)` key.
    pub async fn get_contrastive_pair(
        &self,
        anchor: Uuid,
        negative: Uuid,
    ) -> CoreResult<Option<ContrastivePair>> {
        let cf = self.cf_contrastive_pairs();
        let key = pair_primary_key(anchor, negative);
        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => decode_pair(&bytes).map(Some),
            Ok(None) => Ok(None),
            Err(e) => {
                error!(
                    anchor = %anchor,
                    negative = %negative,
                    error = %e,
                    "ROCKSDB ERROR: Failed to read contrastive pair"
                );
                Err(TeleologicalStoreError::rocksdb_op(
                    "get_contrastive_pair",
                    CF_CONTRASTIVE_PAIRS,
                    Some(anchor),
                    e,
                )
                .into())
            }
        }
    }

    /// Delete a contrastive pair and its secondary index entries. Returns
    /// `true` when a primary record existed.
    pub async fn delete_contrastive_pair(&self, anchor: Uuid, negative: Uuid) -> CoreResult<bool> {
        // Fetch first so we know which kind to drop from the by-kind index
        // (the kind is stored on the record itself).
        let Some(record) = self.get_contrastive_pair(anchor, negative).await? else {
            return Ok(false);
        };
        let primary_key = pair_primary_key(anchor, negative);
        let by_kind_key_bytes = by_kind_key(record.anomaly_kind, anchor, negative);
        let anchor_key = *anchor.as_bytes();

        let primary_cf = self.cf_contrastive_pairs();
        let kind_cf = self.cf_contrastive_by_kind();
        let anchor_cf = self.cf_contrastive_by_anchor();

        // Rebuild the anchor → Vec<Uuid> row without the dropped negative.
        let new_anchor_value = match self.db.get_cf(anchor_cf, anchor_key) {
            Ok(Some(bytes)) => {
                let mut lst = decode_anchor_negatives(&bytes).unwrap_or_default();
                lst.retain(|u| u != &negative);
                dedupe_uuids(&mut lst);
                if lst.is_empty() {
                    None
                } else {
                    Some(encode_anchor_negatives(&lst)?)
                }
            }
            Ok(None) => None,
            Err(e) => {
                error!(
                    anchor = %anchor,
                    error = %e,
                    "ROCKSDB ERROR: Failed to probe anchor negatives during delete"
                );
                return Err(TeleologicalStoreError::rocksdb_op(
                    "probe_contrastive_by_anchor",
                    CF_CONTRASTIVE_BY_ANCHOR,
                    Some(anchor),
                    e,
                )
                .into());
            }
        };

        let mut batch = WriteBatch::default();
        batch.delete_cf(primary_cf, primary_key);
        batch.delete_cf(kind_cf, by_kind_key_bytes);
        match new_anchor_value {
            Some(v) => batch.put_cf(anchor_cf, anchor_key, &v),
            None => batch.delete_cf(anchor_cf, anchor_key),
        }

        self.db.write(batch).map_err(|e| {
            error!(
                anchor = %anchor,
                negative = %negative,
                error = %e,
                "ROCKSDB ERROR: Failed to delete contrastive pair"
            );
            TeleologicalStoreError::rocksdb_op(
                "delete_contrastive_pair",
                CF_CONTRASTIVE_PAIRS,
                Some(anchor),
                e,
            )
        })?;
        Ok(true)
    }

    /// Enumerate every stored `(anchor, negative)` pair key.
    ///
    /// Returns keys only (no payloads). Caller can follow up with
    /// `get_contrastive_pair` when full records are needed.
    pub async fn list_contrastive_pair_keys(&self) -> CoreResult<Vec<(Uuid, Uuid)>> {
        let cf = self.cf_contrastive_pairs();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            match item {
                Ok((key, _)) => {
                    let Some(parsed) = parse_pair_key(&key) else {
                        warn!(
                            len = key.len(),
                            "Skipping malformed CF_CONTRASTIVE_PAIRS key (expected 32 bytes)"
                        );
                        continue;
                    };
                    out.push(parsed);
                }
                Err(e) => {
                    error!(error = %e, "ROCKSDB ERROR: iteration failed in list_contrastive_pair_keys");
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "iterate_contrastive_pairs",
                        CF_CONTRASTIVE_PAIRS,
                        None,
                        e,
                    )
                    .into());
                }
            }
        }
        Ok(out)
    }

    /// O(n) count of primary-CF rows.
    pub async fn count_contrastive_pairs(&self) -> CoreResult<usize> {
        let cf = self.cf_contrastive_pairs();
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Count rows for a given anomaly kind via the secondary index.
    pub async fn count_contrastive_pairs_by_kind(&self, kind: AnomalyKind) -> CoreResult<usize> {
        let cf = self.cf_contrastive_by_kind();
        let prefix = [kind.as_u8()];
        let mut opts = ReadOptions::default();
        opts.set_prefix_same_as_start(true);
        let iter = self.db.iterator_cf_opt(
            cf,
            opts,
            IteratorMode::From(&prefix, rocksdb::Direction::Forward),
        );
        let mut count = 0usize;
        for item in iter {
            match item {
                Ok((key, _)) => {
                    if key.first().copied() == Some(kind.as_u8()) {
                        count += 1;
                    } else {
                        // Prefix extractor guarantees this doesn't happen, but
                        // guard anyway.
                        break;
                    }
                }
                Err(e) => {
                    error!(error = %e, "ROCKSDB ERROR: iteration failed in count_contrastive_pairs_by_kind");
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "count_contrastive_by_kind",
                        CF_CONTRASTIVE_BY_KIND,
                        None,
                        e,
                    )
                    .into());
                }
            }
        }
        Ok(count)
    }

    /// Return the list of negatives mined against `anchor`. Duplicates are
    /// deduped; order is insertion order on disk (pre-dedup).
    pub async fn pairs_for_anchor(&self, anchor: Uuid) -> CoreResult<Vec<Uuid>> {
        let cf = self.cf_contrastive_by_anchor();
        match self.db.get_cf(cf, anchor.as_bytes()) {
            Ok(Some(bytes)) => {
                let mut v = decode_anchor_negatives(&bytes).unwrap_or_default();
                dedupe_uuids(&mut v);
                Ok(v)
            }
            Ok(None) => Ok(Vec::new()),
            Err(e) => {
                error!(
                    anchor = %anchor,
                    error = %e,
                    "ROCKSDB ERROR: pairs_for_anchor read failed"
                );
                Err(TeleologicalStoreError::rocksdb_op(
                    "pairs_for_anchor",
                    CF_CONTRASTIVE_BY_ANCHOR,
                    Some(anchor),
                    e,
                )
                .into())
            }
        }
    }

    /// Prefix-scan the by-kind secondary index and return up to `limit`
    /// `(anchor, negative)` pairs.
    pub async fn list_pairs_by_kind(
        &self,
        kind: AnomalyKind,
        limit: usize,
    ) -> CoreResult<Vec<(Uuid, Uuid)>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cf = self.cf_contrastive_by_kind();
        let prefix = [kind.as_u8()];
        let mut opts = ReadOptions::default();
        opts.set_prefix_same_as_start(true);
        let iter = self.db.iterator_cf_opt(
            cf,
            opts,
            IteratorMode::From(&prefix, rocksdb::Direction::Forward),
        );
        let mut out = Vec::new();
        for item in iter {
            match item {
                Ok((key, _)) => {
                    if key.len() != BY_KIND_KEY_LEN || key[0] != kind.as_u8() {
                        break;
                    }
                    let mut anchor_buf = [0u8; 16];
                    let mut neg_buf = [0u8; 16];
                    anchor_buf.copy_from_slice(&key[1..17]);
                    neg_buf.copy_from_slice(&key[17..33]);
                    out.push((Uuid::from_bytes(anchor_buf), Uuid::from_bytes(neg_buf)));
                    if out.len() >= limit {
                        break;
                    }
                }
                Err(e) => {
                    error!(error = %e, "ROCKSDB ERROR: iteration failed in list_pairs_by_kind");
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "list_pairs_by_kind",
                        CF_CONTRASTIVE_BY_KIND,
                        None,
                        e,
                    )
                    .into());
                }
            }
        }
        Ok(out)
    }

    /// Wipe every row from all three contrastive CFs. Returns the number of
    /// primary-CF rows deleted.
    pub async fn clear_all_contrastive_pairs(&self) -> CoreResult<usize> {
        let primary_cf = self.cf_contrastive_pairs();
        let kind_cf = self.cf_contrastive_by_kind();
        let anchor_cf = self.cf_contrastive_by_anchor();

        // Drain primary keys first for the count.
        let primary_keys: Vec<Vec<u8>> = self
            .db
            .iterator_cf(primary_cf, IteratorMode::Start)
            .filter_map(|r| r.ok().map(|(k, _)| k.to_vec()))
            .collect();
        let kind_keys: Vec<Vec<u8>> = self
            .db
            .iterator_cf(kind_cf, IteratorMode::Start)
            .filter_map(|r| r.ok().map(|(k, _)| k.to_vec()))
            .collect();
        let anchor_keys: Vec<Vec<u8>> = self
            .db
            .iterator_cf(anchor_cf, IteratorMode::Start)
            .filter_map(|r| r.ok().map(|(k, _)| k.to_vec()))
            .collect();

        let mut batch = WriteBatch::default();
        for k in &primary_keys {
            batch.delete_cf(primary_cf, k);
        }
        for k in &kind_keys {
            batch.delete_cf(kind_cf, k);
        }
        for k in &anchor_keys {
            batch.delete_cf(anchor_cf, k);
        }
        self.db.write(batch).map_err(|e| {
            error!(error = %e, "ROCKSDB ERROR: clear_all_contrastive_pairs failed");
            TeleologicalStoreError::rocksdb_op(
                "clear_all_contrastive_pairs",
                CF_CONTRASTIVE_PAIRS,
                None,
                e,
            )
        })?;
        Ok(primary_keys.len())
    }
}

// ==========================================================================
// Encoding / decoding / key helpers
// ==========================================================================

/// Encode a `ContrastivePair` with the canonical
/// `[CONTRASTIVE_PAIR_VERSION][bincode]` layout.
pub(crate) fn encode_pair(pair: &ContrastivePair) -> CoreResult<Vec<u8>> {
    let mut bytes = bincode::serialize(pair).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize ContrastivePair: {}", e))
    })?;
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(CONTRASTIVE_PAIR_VERSION);
    out.append(&mut bytes);
    Ok(out)
}

/// Decode a `ContrastivePair`, rejecting empty payloads and version
/// mismatches.
pub(crate) fn decode_pair(bytes: &[u8]) -> CoreResult<ContrastivePair> {
    if bytes.is_empty() {
        return Err(CoreError::SerializationError(
            "contrastive pair payload is empty (missing version byte)".into(),
        ));
    }
    let version = bytes[0];
    if version != CONTRASTIVE_PAIR_VERSION {
        return Err(CoreError::SerializationError(format!(
            "contrastive pair version mismatch: got {}, expected {}. \
             No automatic migration is supported — re-run mine_contrastive_pairs.",
            version, CONTRASTIVE_PAIR_VERSION
        )));
    }
    bincode::deserialize(&bytes[1..]).map_err(|e| {
        CoreError::SerializationError(format!("bincode deserialize ContrastivePair: {}", e))
    })
}

pub(crate) fn pair_primary_key(anchor: Uuid, negative: Uuid) -> [u8; PAIR_KEY_LEN] {
    let mut out = [0u8; PAIR_KEY_LEN];
    out[..16].copy_from_slice(anchor.as_bytes());
    out[16..].copy_from_slice(negative.as_bytes());
    out
}

pub(crate) fn parse_pair_key(key: &[u8]) -> Option<(Uuid, Uuid)> {
    if key.len() != PAIR_KEY_LEN {
        return None;
    }
    let mut a = [0u8; 16];
    let mut n = [0u8; 16];
    a.copy_from_slice(&key[..16]);
    n.copy_from_slice(&key[16..]);
    Some((Uuid::from_bytes(a), Uuid::from_bytes(n)))
}

pub(crate) fn by_kind_key(
    kind: AnomalyKind,
    anchor: Uuid,
    negative: Uuid,
) -> [u8; BY_KIND_KEY_LEN] {
    let mut out = [0u8; BY_KIND_KEY_LEN];
    out[0] = kind.as_u8();
    out[1..17].copy_from_slice(anchor.as_bytes());
    out[17..].copy_from_slice(negative.as_bytes());
    out
}

fn encode_anchor_negatives(list: &[Uuid]) -> CoreResult<Vec<u8>> {
    bincode::serialize(list).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize anchor negatives: {}", e))
    })
}

fn decode_anchor_negatives(bytes: &[u8]) -> CoreResult<Vec<Uuid>> {
    bincode::deserialize(bytes).map_err(|e| {
        CoreError::SerializationError(format!("bincode deserialize anchor negatives: {}", e))
    })
}

fn append_anchor_negative(existing: &[Uuid], new_neg: Uuid) -> CoreResult<Vec<u8>> {
    // Append-then-dedupe on read, but skip the write entirely when the
    // negative is already present.
    if existing.contains(&new_neg) {
        return encode_anchor_negatives(existing);
    }
    let mut v = existing.to_vec();
    v.push(new_neg);
    encode_anchor_negatives(&v)
}

fn dedupe_uuids(v: &mut Vec<Uuid>) {
    let mut seen: HashSet<Uuid> = HashSet::with_capacity(v.len());
    v.retain(|u| seen.insert(*u));
}

// ==========================================================================
// Unit tests
// ==========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use context_graph_core::contrastive::AnomalyKind;
    use context_graph_core::teleological::types::NUM_EMBEDDERS;

    fn mock_pair(anchor: Uuid, negative: Uuid, kind: AnomalyKind) -> ContrastivePair {
        ContrastivePair {
            anchor_id: anchor,
            negative_id: negative,
            anchor_text: "anchor".into(),
            negative_text: "negative".into(),
            similarity_profile: [0.5; NUM_EMBEDDERS],
            high_embedders: vec![0],
            low_embedders: vec![4],
            disagreement_magnitude: 0.7,
            anomaly_kind: kind,
            mined_at: Utc::now(),
            generator: "cross_embedder_anomaly_v1".into(),
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let p = mock_pair(
            Uuid::new_v4(),
            Uuid::new_v4(),
            AnomalyKind::SemanticButNotCausal,
        );
        let bytes = encode_pair(&p).unwrap();
        assert_eq!(bytes[0], CONTRASTIVE_PAIR_VERSION);
        let back = decode_pair(&bytes).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn decode_rejects_empty() {
        let err = decode_pair(&[]).unwrap_err();
        assert!(format!("{}", err).contains("empty"), "unexpected: {}", err);
    }

    #[test]
    fn decode_rejects_wrong_version() {
        let p = mock_pair(Uuid::new_v4(), Uuid::new_v4(), AnomalyKind::Other);
        let mut bytes = encode_pair(&p).unwrap();
        bytes[0] = CONTRASTIVE_PAIR_VERSION.wrapping_add(1);
        let err = decode_pair(&bytes).unwrap_err();
        assert!(
            format!("{}", err).contains("version mismatch"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn primary_key_round_trips() {
        let a = Uuid::new_v4();
        let n = Uuid::new_v4();
        let k = pair_primary_key(a, n);
        let (pa, pn) = parse_pair_key(&k).unwrap();
        assert_eq!(pa, a);
        assert_eq!(pn, n);
    }

    #[test]
    fn parse_pair_key_rejects_wrong_length() {
        assert!(parse_pair_key(&[]).is_none());
        assert!(parse_pair_key(&[0u8; 31]).is_none());
        assert!(parse_pair_key(&[0u8; 33]).is_none());
    }

    #[test]
    fn by_kind_key_starts_with_kind_byte() {
        let a = Uuid::new_v4();
        let n = Uuid::new_v4();
        let k = by_kind_key(AnomalyKind::KeywordButNotParaphrase, a, n);
        assert_eq!(k[0], AnomalyKind::KeywordButNotParaphrase.as_u8());
        assert_eq!(&k[1..17], a.as_bytes());
        assert_eq!(&k[17..], n.as_bytes());
    }

    #[test]
    fn by_kind_keys_sorted_by_kind_byte() {
        let a = Uuid::new_v4();
        let n = Uuid::new_v4();
        for pair in AnomalyKind::all().windows(2) {
            let k1 = by_kind_key(pair[0], a, n);
            let k2 = by_kind_key(pair[1], a, n);
            assert!(
                k1 < k2,
                "kind bytes must be strictly increasing in as_u8 order"
            );
        }
    }

    #[test]
    fn anchor_negatives_roundtrip() {
        let list = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let bytes = encode_anchor_negatives(&list).unwrap();
        let back = decode_anchor_negatives(&bytes).unwrap();
        assert_eq!(back, list);
    }

    #[test]
    fn append_anchor_negative_is_idempotent() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let existing = vec![a];
        // Appending a fresh uuid grows the list.
        let encoded = append_anchor_negative(&existing, b).unwrap();
        let decoded = decode_anchor_negatives(&encoded).unwrap();
        assert_eq!(decoded, vec![a, b]);
        // Appending an existing uuid does not grow the list.
        let encoded2 = append_anchor_negative(&decoded, a).unwrap();
        let decoded2 = decode_anchor_negatives(&encoded2).unwrap();
        assert_eq!(decoded2, vec![a, b]);
    }

    #[test]
    fn dedupe_preserves_first_occurrence_order() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let mut v = vec![a, b, a, c, b];
        dedupe_uuids(&mut v);
        assert_eq!(v, vec![a, b, c]);
    }
}
