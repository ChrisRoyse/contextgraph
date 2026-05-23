use crate::cert::TrainingCertificate;
use crate::error::{TrainerError, TrainerErrorCode};
use rocksdb::{IteratorMode, DB};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

pub const FSYNC_INTERVAL: u64 = 100;
pub const GENESIS_PARENT_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize)]
pub struct ChainVerificationReport {
    pub from: u64,
    pub to: u64,
    pub verified: u64,
    pub broken_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
}

pub fn body_canonical_json(cert: &TrainingCertificate) -> Result<String, TrainerError> {
    let mut hashable = cert.clone();
    hashable.self_hash.clear();
    let value = serde_json::to_value(hashable)?;
    serde_json::to_string(&sort_value(value)).map_err(TrainerError::from)
}

pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, TrainerError> {
    let value = serde_json::to_value(value)?;
    serde_json::to_string(&sort_value(value)).map_err(TrainerError::from)
}

pub fn compute_self_hash(canonical_body: &str) -> String {
    hex::encode(Sha256::digest(canonical_body.as_bytes()))
}

pub fn compute_parent_hash(prev_self_hash: &str, prev_canonical_body: &str) -> String {
    let mut h = Sha256::new();
    h.update(prev_self_hash.as_bytes());
    h.update(prev_canonical_body.as_bytes());
    hex::encode(h.finalize())
}

pub fn compute_merkle_root(components: &HashMap<String, f32>) -> String {
    if components.is_empty() {
        return GENESIS_PARENT_HASH.to_string();
    }
    let mut leaves = components
        .iter()
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .map(|(key, value)| {
            let mut h = Sha256::new();
            h.update(b"MEJEPA_TRAIN_COMPONENT_LEAF_V1");
            h.update(key.as_bytes());
            h.update(format!("{value:.9}").as_bytes());
            h.finalize().to_vec()
        })
        .collect::<Vec<_>>();
    while leaves.len() > 1 {
        if leaves.len() % 2 == 1 {
            leaves.push(vec![0u8; 32]);
        }
        let mut next = Vec::with_capacity(leaves.len() / 2);
        for pair in leaves.chunks(2) {
            let mut h = Sha256::new();
            h.update(b"MEJEPA_TRAIN_COMPONENT_NODE_V1");
            h.update(&pair[0]);
            h.update(&pair[1]);
            next.push(h.finalize().to_vec());
        }
        leaves = next;
    }
    hex::encode(&leaves[0])
}

pub fn verify_chain(
    rocksdb: &DB,
    cf_name: &str,
    from_step: u64,
    to_step: u64,
) -> Result<ChainVerificationReport, TrainerError> {
    let cf = cf(rocksdb, cf_name)?;
    let mut expected_parent = GENESIS_PARENT_HASH.to_string();
    let mut verified = 0u64;
    for step in from_step..=to_step {
        let Some(bytes) = rocksdb.get_cf(cf, step.to_be_bytes())? else {
            return Err(chain_error(step, format!("missing cert at step {step}")));
        };
        let cert: TrainingCertificate = serde_json::from_slice(&bytes)?;
        if cert.step != step {
            return Err(chain_error(
                step,
                format!("cert step field {} does not match key {step}", cert.step),
            ));
        }
        cert.validate_phase3()?;
        if cert.parent_witness_hash != expected_parent {
            return Ok(ChainVerificationReport {
                from: from_step,
                to: to_step,
                verified,
                broken_at: Some(step),
                failure_reason: Some("parent_witness_hash_mismatch".to_string()),
                expected: Some(expected_parent),
                observed: Some(cert.parent_witness_hash),
            });
        }
        let body = body_canonical_json(&cert)?;
        let recomputed = compute_self_hash(&body);
        if cert.self_hash != recomputed {
            return Ok(ChainVerificationReport {
                from: from_step,
                to: to_step,
                verified,
                broken_at: Some(step),
                failure_reason: Some("self_hash_mismatch".to_string()),
                expected: Some(recomputed),
                observed: Some(cert.self_hash),
            });
        }
        let recomputed_merkle_root = compute_merkle_root(&cert.loss_components);
        if cert.merkle_root != recomputed_merkle_root {
            return Ok(ChainVerificationReport {
                from: from_step,
                to: to_step,
                verified,
                broken_at: Some(step),
                failure_reason: Some("merkle_root_mismatch".to_string()),
                expected: Some(recomputed_merkle_root),
                observed: Some(cert.merkle_root),
            });
        }
        expected_parent = cert.self_hash;
        verified += 1;
    }
    Ok(ChainVerificationReport {
        from: from_step,
        to: to_step,
        verified,
        broken_at: None,
        failure_reason: None,
        expected: None,
        observed: None,
    })
}

pub fn last_cert_hash(rocksdb: &DB, cf_name: &str) -> Result<String, TrainerError> {
    let cf = cf(rocksdb, cf_name)?;
    let mut last_hash = GENESIS_PARENT_HASH.to_string();
    for item in rocksdb.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let cert: TrainingCertificate = serde_json::from_slice(&value)?;
        last_hash = cert.self_hash;
    }
    Ok(last_hash)
}

pub(crate) fn cf<'a>(
    rocksdb: &'a DB,
    cf_name: &str,
) -> Result<&'a rocksdb::ColumnFamily, TrainerError> {
    rocksdb.cf_handle(cf_name).ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("missing RocksDB column family {cf_name}"),
        )
        .with_context(json!({
            "cf": cf_name,
            "remediation": "open the DB with MEJEPA_TRAIN_CFS column-family descriptors"
        }))
    })
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = Map::new();
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                sorted.insert(
                    key.clone(),
                    sort_value(map.get(&key).cloned().expect("key exists")),
                );
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_value).collect()),
        other => other,
    }
}

fn chain_error(step: u64, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainCertChainBroken, message).with_step(step)
}
