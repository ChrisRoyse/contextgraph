//! DynamicJEPA byte-exact RocksDB key encoders.

use context_graph_core::dynamicjepa::{
    AdapterId, BindingId, BindingRef, DatasetId, DatasetShardId, DomainPackId, EventId,
    InstrumentId,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const DYNAMIC_JEPA_DOMAIN_PACK_NAMESPACE: Uuid =
    Uuid::from_u128(0x5090_0000_0000_4000_8000_0000_0000_0002);

pub fn blake3_128(bytes: &[u8]) -> [u8; 16] {
    let hash = blake3::hash(bytes);
    let mut out = [0u8; 16];
    out.copy_from_slice(&hash.as_bytes()[..16]);
    out
}

pub fn domain_pack_storage_uuid(id: &DomainPackId, version: &str) -> Uuid {
    Uuid::new_v5(
        &DYNAMIC_JEPA_DOMAIN_PACK_NAMESPACE,
        format!("{}:{version}", id.as_str()).as_bytes(),
    )
}

pub fn uuid_key(uuid: Uuid) -> [u8; 16] {
    *uuid.as_bytes()
}

pub fn domain_pack_key(id: &DomainPackId, version: &str) -> [u8; 16] {
    uuid_key(domain_pack_storage_uuid(id, version))
}

pub fn name_version_key(id: &DomainPackId, version: &str) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(&blake3_128(id.as_str().as_bytes()));
    key[16..].copy_from_slice(&blake3_128(version.as_bytes()));
    key
}

pub fn registry_key(domain_pack_uuid: Uuid, item_id: &str) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(domain_pack_uuid.as_bytes());
    key[16..].copy_from_slice(&blake3_128(item_id.as_bytes()));
    key
}

pub fn instrument_registry_key(domain_pack_uuid: Uuid, instrument_id: &InstrumentId) -> [u8; 32] {
    registry_key(domain_pack_uuid, instrument_id.as_str())
}

pub fn adapter_registry_key(domain_pack_uuid: Uuid, adapter_id: &AdapterId) -> [u8; 32] {
    registry_key(domain_pack_uuid, adapter_id.as_str())
}

pub fn instrument_reading_key(event_id: EventId, instrument_id: &InstrumentId) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(&event_id.into_bytes());
    key[16..].copy_from_slice(&blake3_128(instrument_id.as_str().as_bytes()));
    key
}

pub fn pairwise_reading_key(event_uuid: &Uuid, instrument_j: u32, instrument_k: u32) -> [u8; 24] {
    let mut key = [0u8; 24];
    key[..16].copy_from_slice(event_uuid.as_bytes());
    key[16..20].copy_from_slice(&instrument_j.to_le_bytes());
    key[20..24].copy_from_slice(&instrument_k.to_le_bytes());
    key
}

pub fn constellation_key(domain_uuid: &Uuid, subject_id: &str, modality_id: u32) -> [u8; 36] {
    let mut key = [0u8; 36];
    key[..16].copy_from_slice(domain_uuid.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(subject_id.as_bytes());
    let hash = hasher.finalize();
    key[16..32].copy_from_slice(&hash[..16]);
    key[32..36].copy_from_slice(&modality_id.to_le_bytes());
    key
}

pub fn threshold_calibration_key(
    domain_uuid: &Uuid,
    subject_id: &str,
    modality_id: u32,
    supersede_seq: u32,
) -> [u8; 40] {
    let mut key = [0u8; 40];
    key[..36].copy_from_slice(&constellation_key(domain_uuid, subject_id, modality_id));
    key[36..40].copy_from_slice(&supersede_seq.to_le_bytes());
    key
}

pub fn dataset_shard_key(dataset_id: DatasetId, shard_id: DatasetShardId) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(&dataset_id.into_bytes());
    key[16..].copy_from_slice(&shard_id.into_bytes());
    key
}

pub fn binding_entity_key(entity_ref: &BindingRef, binding_id: BindingId) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(entity_ref.cf.as_bytes());
    hasher.update(&[0]);
    hasher.update(&entity_ref.key_bytes);
    let entity_hash = hasher.finalize();

    let mut key = [0u8; 32];
    key[..16].copy_from_slice(&entity_hash.as_bytes()[..16]);
    key[16..].copy_from_slice(&binding_id.into_bytes());
    key
}

pub fn audit_key(timestamp_unix_nanos: u64, audit_id: Uuid) -> [u8; 24] {
    let mut key = [0u8; 24];
    key[..8].copy_from_slice(&timestamp_unix_nanos.to_be_bytes());
    key[8..].copy_from_slice(audit_id.as_bytes());
    key
}

pub fn parse_uuid_key(
    key: &[u8],
    cf: &str,
) -> context_graph_core::dynamicjepa::DynamicJepaResult<Uuid> {
    if key.len() != 16 {
        return Err(
            context_graph_core::dynamicjepa::DynamicJepaError::validation(
                format!("{cf}.key"),
                format!("expected 16-byte UUID key, got {} bytes", key.len()),
                "inspect the RocksDB key writer; DynamicJEPA UUID CFs use exactly 16 bytes",
            ),
        );
    }
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(key);
    Ok(Uuid::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairwise_reading_key_format_is_24_bytes_fixed() {
        let event = Uuid::from_u128(0x1111_2222_3333_4444_5555_6666_7777_8888);
        let key = pairwise_reading_key(&event, 7, 11);
        assert_eq!(key.len(), 24);
        assert_eq!(&key[..16], event.as_bytes());
        assert_eq!(&key[16..20], &7u32.to_le_bytes());
        assert_eq!(&key[20..24], &11u32.to_le_bytes());
    }

    #[test]
    fn constellation_key_format_is_36_bytes_fixed() {
        let domain = Uuid::from_u128(0x1111_2222_3333_4444_5555_6666_7777_8888);
        let key = constellation_key(&domain, "subject.alpha", 3);
        assert_eq!(key.len(), 36);
        assert_eq!(&key[..16], domain.as_bytes());
        assert_eq!(&key[32..36], &3u32.to_le_bytes());
    }

    #[test]
    fn threshold_calibration_key_format_is_40_bytes_fixed() {
        let domain = Uuid::from_u128(0x1111_2222_3333_4444_5555_6666_7777_8888);
        let prefix = constellation_key(&domain, "subject.alpha", 3);
        let key = threshold_calibration_key(&domain, "subject.alpha", 3, 9);
        assert_eq!(key.len(), 40);
        assert_eq!(&key[..36], &prefix);
        assert_eq!(&key[36..40], &9u32.to_le_bytes());
    }

    #[test]
    fn subject_id_hash_is_stable_across_calls() {
        let domain = Uuid::from_u128(0x1111_2222_3333_4444_5555_6666_7777_8888);
        assert_eq!(
            constellation_key(&domain, "subject.alpha", 3),
            constellation_key(&domain, "subject.alpha", 3)
        );
        assert_ne!(
            constellation_key(&domain, "subject.alpha", 3),
            constellation_key(&domain, "subject.beta", 3)
        );
    }
}
