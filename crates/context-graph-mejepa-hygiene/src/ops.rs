// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use crate::error::{OpsError, OpsErrorKind, OpsResult};
use crate::gc::gc_run_nightly;
use crate::quota::quota_check_and_evict;
use crate::reports::{
    GcReport, QuotaEvictionReport, TierTransitionReport, WitnessCompressionReport,
    WitnessIntegrityReport,
};
use crate::storage::HygieneEnv;
use crate::tier_ops::{tier_demote_all, tier_promote_all};
use crate::witness_compress::{verify_witness_integrity, witness_compress_old_segments};

pub fn heal_detect_drift(env: &HygieneEnv) -> OpsResult<serde_json::Value> {
    env.config
        .self_healing
        .as_ref()
        .ok_or_else(|| {
            OpsError::new(OpsErrorKind::CrossCuttingDeferred {
                operation: "heal_detect_drift".to_string(),
                detail: "no SelfHealingHandle supplied in RuntimeConfig".to_string(),
            })
        })?
        .detect_drift()
}

pub fn heal_retrain_if_needed(env: &HygieneEnv) -> OpsResult<serde_json::Value> {
    env.config
        .self_healing
        .as_ref()
        .ok_or_else(|| {
            OpsError::new(OpsErrorKind::CrossCuttingDeferred {
                operation: "heal_retrain_if_needed".to_string(),
                detail: "no SelfHealingHandle supplied in RuntimeConfig".to_string(),
            })
        })?
        .retrain_if_needed()
}

pub fn tier_demote_ops(env: &HygieneEnv) -> OpsResult<TierTransitionReport> {
    tier_demote_all(env)
}

pub fn tier_promote_ops(env: &HygieneEnv) -> OpsResult<TierTransitionReport> {
    tier_promote_all(env)
}

pub fn quota_check_and_evict_ops(env: &HygieneEnv) -> OpsResult<QuotaEvictionReport> {
    quota_check_and_evict(env)
}

pub fn witness_compress_old_segments_ops(env: &HygieneEnv) -> OpsResult<WitnessCompressionReport> {
    witness_compress_old_segments(env)
}

pub fn witness_verify_integrity(env: &HygieneEnv) -> OpsResult<WitnessIntegrityReport> {
    verify_witness_integrity(env)
}

pub fn gc_run_nightly_ops(env: &HygieneEnv) -> OpsResult<GcReport> {
    gc_run_nightly(env)
}
