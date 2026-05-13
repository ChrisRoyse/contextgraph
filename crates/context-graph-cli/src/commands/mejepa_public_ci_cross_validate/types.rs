use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::commands::mejepa_train::{MejepaTrainOutput, TrainSplitArg};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicCiCrossValidationOutput {
    pub public_corpus: PublicCiSource,
    pub public_corpus_root: String,
    pub public_example_count: usize,
    pub min_examples: usize,
    pub split: TrainSplitArg,
    pub db_path: String,
    pub lite_baseline_path: String,
    pub cells_compared: usize,
    pub missing_lite_cells: usize,
    pub missing_public_cells: usize,
    pub mean_delta_public_minus_lite: Option<f32>,
    pub per_cell: Vec<PublicCiCellComparison>,
    pub ingest: MejepaTrainOutput,
    pub readback_equal: bool,
    pub report_path: Option<String>,
    pub report_readback_equal: bool,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicCiCellComparison {
    pub cell: String,
    pub lite_correlation: Option<f32>,
    pub public_ci_correlation: Option<f32>,
    pub delta_public_minus_lite: Option<f32>,
    pub public_example_count: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PublicCiSource {
    pub name: String,
    pub doi: String,
    #[serde(alias = "zenodo_url")]
    pub zenodo_url: String,
    #[serde(alias = "repository_url")]
    pub repository_url: String,
    pub license: String,
    pub notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicCorpusIndex {
    pub corpus_version: String,
    #[serde(default)]
    pub corpus_sha256: Option<String>,
    pub public_ci_source: PublicCiSource,
    pub entries: Vec<PublicCorpusEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PublicCorpusEntry {
    pub bucket: String,
    pub category: String,
    pub language: String,
    #[serde(default)]
    pub mutation_note: Option<String>,
    pub oracle_all_passed: bool,
    #[serde(default)]
    pub oracle_exception: Option<String>,
    #[serde(default)]
    pub oracle_per_test_count: Option<usize>,
    #[serde(default)]
    pub oracle_verdict_sha256: Option<String>,
    pub patch_path: String,
    pub patch_sha256: String,
    pub predicted_oracle_pass: f32,
    pub repo: String,
    pub task_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LiteBaseline {
    pub corpus_name: String,
    pub per_cell_correlation: BTreeMap<String, Option<f32>>,
}

#[derive(Debug)]
pub(super) struct PublicObservation {
    pub cell: String,
    pub predicted: f32,
    pub actual: f32,
}
