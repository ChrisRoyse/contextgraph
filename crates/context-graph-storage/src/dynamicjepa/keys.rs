//! DynamicJEPA byte-exact RocksDB key encoders.

use context_graph_core::dynamicjepa::{
    AdapterId, BindingId, BindingRef, DatasetId, DatasetShardId, DomainPackId, EventId,
    InstrumentId,
};
use uuid::Uuid;

pub const DYNAMIC_JEPA_DOMAIN_PACK_NAMESPACE: Uuid =
    Uuid::from_u128(0x5090_0000_0000_4000_8000_000000000002);

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
