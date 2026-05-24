pub mod epoch_witness;
pub mod holdout;
pub mod regression;
pub mod slicing;

use ed25519_dalek::SigningKey;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCategory {
    KnownGood,
    SubtleFlip,
    OffByOne,
    SwapVariable,
    DeleteTestCall,
    WrongFile,
    OverEngineer,
    CompileError,
}

impl MutationCategory {
    pub const ALL: [Self; 8] = [
        Self::KnownGood,
        Self::SubtleFlip,
        Self::OffByOne,
        Self::SwapVariable,
        Self::DeleteTestCall,
        Self::WrongFile,
        Self::OverEngineer,
        Self::CompileError,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
}

impl Lang {
    pub const ALL: [Self; 11] = [
        Self::Rust,
        Self::Python,
        Self::JavaScript,
        Self::TypeScript,
        Self::Go,
        Self::Java,
        Self::C,
        Self::Cpp,
        Self::CSharp,
        Self::Ruby,
        Self::Php,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldoutReport {
    pub step: u64,
    pub prediction_oracle_agreement: f32,
    pub conformal_coverage_calibration: f32,
    pub per_mutation_category_accuracy: HashMap<MutationCategory, f32>,
    pub per_language_accuracy: HashMap<Lang, f32>,
    pub predictor_redundancy_pairwise_mi: f32,
    /// #621: was hardcoded `1.0` regardless of whether Gτ was evaluated.
    /// The honest contract: `Some(rate)` ONLY when the holdout panels were
    /// scored against `context-graph-mejepa-tct::gtau::evaluate_panel`;
    /// `None` otherwise. The TCT wiring is tracked separately — until it
    /// lands, every `HoldoutReport` row is `None` here and no consumer may
    /// fabricate a value.
    pub gtau_pass_rate: Option<f32>,
    pub generic_only_warning: Option<String>,
    pub phase3_dod_passed: Option<bool>,
    pub timestamp_iso8601: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochSummary {
    pub epoch: u32,
    pub mean_l_step: f32,
    pub mean_delta_p: f32,
    pub mean_delta_k: f32,
    pub mean_delta_omega: f32,
    pub mean_delta_xi: f32,
    pub holdout_agreement: f32,
    /// #688: only populated when `epoch_semantics` indicates per-category
    /// accuracy was actually measured. The diagnostic-only training path emits
    /// `None` rather than a hardcoded `MutationCategory::KnownGood`, so a
    /// downstream consumer iterating `CF_MEJEPA_EPOCH_WITNESS` cannot mistake
    /// a stub for a measurement.
    pub best_category: Option<MutationCategory>,
    /// #688: see `best_category`.
    pub worst_category: Option<MutationCategory>,
    /// #688: see `best_category`.
    pub best_language: Option<Lang>,
    /// #688: see `best_category`.
    pub worst_language: Option<Lang>,
    pub total_steps_this_epoch: u64,
    pub skipped_steps_this_epoch: u64,
    pub parent_witness_hash: String,
    pub self_hash: String,
    /// #688: mirrors `TrainingResult::training_semantics` — names the path
    /// that produced this epoch's summary. Required so any consumer of
    /// `CF_MEJEPA_EPOCH_WITNESS` can tell "computed from data" apart from
    /// "diagnostic stub" without inspecting trainer-side context.
    pub epoch_semantics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochWitnessEntry {
    pub bytes: Vec<u8>,
    pub parent_witness_hash: [u8; 32],
    pub self_hash: [u8; 32],
    pub ed25519_signature: Vec<u8>,
    pub layout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochReport {
    pub epoch: u32,
    pub summary: EpochSummary,
    pub witness_entry: EpochWitnessEntry,
    pub phase3_dod_passed_at_this_epoch: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct HoldoutEvaluator {
    pub rocksdb: Arc<DB>,
    pub cf_holdout_reports_name: String,
}

#[derive(Debug, Clone)]
pub struct EpochWitnessChain {
    pub rocksdb: Arc<DB>,
    pub cf_epoch_witness_name: String,
    pub last_epoch_hash: [u8; 32],
    #[allow(dead_code)]
    pub ed25519_signing_key: SigningKey,
}

pub use epoch_witness::{shake256_32, EpochWitnessReplay};
pub use holdout::{
    CalibrationDataset, HoldoutDataset, HoldoutExample, OracleClass, OracleHead, OraclePrediction,
    PredictorForward, PredictorForwardOptions, TrainSplit,
};
