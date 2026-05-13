// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::categories::StorageCategory;
use crate::entry::{EntryId, TierTransition};
use crate::tier::Tier;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TierTransitionReport {
    pub transitions: Vec<TierTransition>,
    pub corrupt_entries: Vec<EntryId>,
    pub fidelity_lost_entries: Vec<EntryId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuotaCategoryStatus {
    pub category: StorageCategory,
    pub used_bytes: u64,
    pub budget_bytes: u64,
    pub over_budget: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuotaStatus {
    pub total_used_bytes: u64,
    pub total_quota_bytes: u64,
    pub categories: Vec<QuotaCategoryStatus>,
    pub unrecoverable_categories: Vec<StorageCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvictionRecord {
    pub entry_id: EntryId,
    pub category: StorageCategory,
    pub bytes_deleted: u64,
    pub score: f32,
    pub tier: Tier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuotaEvictionReport {
    pub before: QuotaStatus,
    pub after: QuotaStatus,
    pub evicted: Vec<EvictionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WitnessSegmentMeta {
    pub segment_start: u64,
    pub entry_count: u64,
    pub archive_path: PathBuf,
    pub archive_sha256: [u8; 32],
    pub merkle_root: [u8; 32],
    pub compressed_entry_hash: [u8; 32],
    pub first_key: Vec<u8>,
    pub last_key: Vec<u8>,
    pub compressed_at_unix: i64,
    pub archive_len_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WitnessCompressionReport {
    pub segments_compressed: u64,
    pub entries_archived: u64,
    pub before_live_entries: u64,
    pub after_live_entries: u64,
    pub archive_bytes: u64,
    pub segment_metas: Vec<WitnessSegmentMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WitnessIntegrityReport {
    pub live_entries: u64,
    pub compressed_entries: u64,
    pub last_chain_hash: [u8; 32],
    pub archive_verified_segments: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GcStepReport {
    pub name: String,
    pub ok: bool,
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GcReport {
    pub started_unix: i64,
    pub completed_unix: i64,
    pub steps: Vec<GcStepReport>,
    pub quota_after: QuotaStatus,
    pub witness_after: WitnessIntegrityReport,
    pub source_of_truth_cf: String,
    pub report_key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event_type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GcEvent {
    SessionCleanup {
        session_id_hex: String,
        occurred_unix_ms: i64,
        live_predictions_deleted: u64,
        shift_watermark_deleted: bool,
        deleted_live_prediction_bytes: u64,
        deleted_shift_watermark_bytes: u64,
        deleted_total_bytes: u64,
        quota_category: StorageCategory,
        quota_before_total_used_bytes: u64,
        quota_after_total_used_bytes: u64,
        source_of_truth_cf: String,
        report_key_hex: String,
    },
}
