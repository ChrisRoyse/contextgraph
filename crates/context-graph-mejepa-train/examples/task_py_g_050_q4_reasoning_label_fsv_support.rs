#![allow(dead_code)]

use context_graph_mejepa::q4_trust_gate::{Q4EvidenceCatalog, Q4HeadEvidence, Q4HeadKind};
use context_graph_mejepa_instruments::{
    Q4ReasoningOutcome, Q4ReasoningPredictionVerdict, Q4ReasoningSource,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(crate) type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
pub(crate) struct EdgeCase {
    pub(crate) name: &'static str,
    pub(crate) passed: bool,
    pub(crate) detail: serde_json::Value,
}

pub(crate) fn build_harvest(run_id: &str, rows: usize) -> Vec<Q4ReasoningSource> {
    (0..rows)
        .map(|idx| {
            let (prose, outcome, verdict, diff_cos, oracle_cos, affect) = match idx % 6 {
                0 => (
                    "I think this should fix the failing assertion, but I am not sure until the oracle runs.",
                    Q4ReasoningOutcome::Pass,
                    Q4ReasoningPredictionVerdict::Pass,
                    0.55,
                    0.62,
                    0.41,
                ),
                1 => (
                    "This is definitely fixed and all tests pass.",
                    Q4ReasoningOutcome::Fail,
                    Q4ReasoningPredictionVerdict::Fail,
                    0.18,
                    0.08,
                    0.83,
                ),
                2 => (
                    "Verified against the oracle output; the prior failing path is now passing.",
                    Q4ReasoningOutcome::Pass,
                    Q4ReasoningPredictionVerdict::Pass,
                    0.84,
                    0.90,
                    0.52,
                ),
                3 => (
                    "Sorry, my mistake: the first patch missed the edge case and the test is failing.",
                    Q4ReasoningOutcome::Fail,
                    Q4ReasoningPredictionVerdict::Fail,
                    0.41,
                    0.38,
                    0.77,
                ),
                4 => (
                    "The fix is complete, implemented, and verified; all tests pass.",
                    Q4ReasoningOutcome::Pass,
                    Q4ReasoningPredictionVerdict::Pass,
                    0.86,
                    0.91,
                    0.62,
                ),
                _ => (
                    "The patch works for the edited path.",
                    Q4ReasoningOutcome::Fail,
                    Q4ReasoningPredictionVerdict::Fail,
                    0.72,
                    0.70,
                    0.58,
                ),
            };
            Q4ReasoningSource {
                corpus_row_id: format!("task-py-g-050-{run_id}-{idx:03}"),
                chunk_id: format!("chunk-q4-reasoning-{idx:03}"),
                session_id: format!("session-q4-reasoning-{idx:03}"),
                agent_prose: prose.to_string(),
                oracle_outcome: outcome,
                prediction_verdict: verdict,
                pairwise_cosine_reasoning_diff: diff_cos,
                pairwise_cosine_reasoning_oracle: oracle_cos,
                e17_affect_score: affect,
            }
        })
        .collect()
}

pub(crate) fn update_q4_catalog(path: &Path, producer_rows: u64) -> FsvResult<Q4EvidenceCatalog> {
    let mut catalog = if path.exists() {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        Q4EvidenceCatalog::default()
    };
    let mut heads: BTreeMap<_, _> = std::mem::take(&mut catalog.heads);
    heads.insert(
        Q4HeadKind::Reasoning,
        Q4HeadEvidence {
            producer_fsv_root: Some(
                "/zfs/archive/contextgraph/fsv/task-py-g-050-q4-reasoning-label-producer-fsv/"
                    .to_string(),
            ),
            producer_rows,
            calibration_fsv_root: None,
            calibration_rows: 0,
            per_slot_evidence_root: None,
            slots_with_evidence: Vec::new(),
        },
    );
    catalog.heads = heads;
    write_json_checked(path, &catalog)?;
    Ok(catalog)
}

pub(crate) fn write_json_checked<T: Serialize>(path: &Path, value: &T) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    let readback = fs::read(path)?;
    let _: serde_json::Value = serde_json::from_slice(&readback)?;
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> FsvResult<String> {
    Ok(sha256_bytes(&fs::read(path)?))
}

pub(crate) fn sha256_json<T: Serialize>(value: &T) -> FsvResult<String> {
    Ok(sha256_bytes(&serde_json::to_vec(value)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(crate) fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("requirement failed: {name}").into())
    }
}
