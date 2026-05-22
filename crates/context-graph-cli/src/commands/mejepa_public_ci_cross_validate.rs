//! Phase G public-CI cross-validation driver.
//!
//! The command consumes a pre-scraped public GitHub Actions corpus in the same
//! `index.json` shape accepted by `mejepa train`, validates that it is a 500+
//! GHALogs-derived subset, ingests it through the Phase G source-of-truth
//! writer, and compares its per-cell prediction/oracle correlations against a
//! SWE-bench Lite baseline.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure, Context, Result};
use clap::Args;
use serde_json::json;

use crate::commands::mejepa_active_learning::DEFAULT_MEJEPA_INFER_DB;
use crate::commands::mejepa_oracle_flakiness::DEFAULT_CORPUS_QUARANTINE_PATH;
use crate::commands::mejepa_train::{run_mejepa_train, MejepaTrainArgs, TrainSplitArg};

#[cfg(test)]
mod tests;
mod types;

use self::types::{
    LiteBaseline, PublicCiCellComparison, PublicCiCrossValidationOutput, PublicCorpusEntry,
    PublicCorpusIndex, PublicObservation,
};

const DEFAULT_MIN_PUBLIC_CI_EXAMPLES: usize = 500;
const GHALOGS_DOI: &str = "10.5281/zenodo.10154920";

#[derive(Args, Debug, Clone)]
pub struct PublicCiCrossValidationArgs {
    /// Pre-scraped public-CI corpus root or index.json.
    #[arg(long = "public-corpus", required = true)]
    pub public_corpus: PathBuf,

    /// Lite baseline JSON containing per-cell correlations.
    #[arg(long)]
    pub lite_baseline: PathBuf,

    /// Split to ingest and compare from the public-CI corpus.
    #[arg(long, value_enum, default_value_t = TrainSplitArg::Holdout)]
    pub split: TrainSplitArg,

    /// Inference RocksDB path receiving ME-JEPA source-of-truth rows.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Minimum public-CI examples required before ingestion.
    #[arg(long, default_value_t = DEFAULT_MIN_PUBLIC_CI_EXAMPLES)]
    pub min_examples: usize,

    /// Optional JSON report path; written with fsync + readback.
    #[arg(long)]
    pub report_out: Option<PathBuf>,
}

pub fn run_public_ci_cross_validation(
    args: PublicCiCrossValidationArgs,
) -> Result<PublicCiCrossValidationOutput> {
    validate_args(&args)?;
    let (index_path, corpus_root, index) = load_public_index(&args.public_corpus)?;
    validate_public_source(&index)?;
    let baseline = load_lite_baseline(&args.lite_baseline)?;
    ensure!(
        !baseline.corpus_name.trim().is_empty(),
        "MEJEPA_PUBLIC_CI_BASELINE_INVALID: corpus_name must be non-empty"
    );

    let observations = selected_public_observations(&index, args.split)?;
    ensure!(
        observations.len() >= args.min_examples,
        "MEJEPA_PUBLIC_CI_TOO_FEW_EXAMPLES: {} selected public-CI rows for split {}, need at least {}",
        observations.len(),
        args.split.public_ci_split_name(),
        args.min_examples
    );

    let public_correlations = per_cell_correlations(&observations);
    let per_cell = compare_cells(&baseline.per_cell_correlation, &public_correlations);
    let mean_delta_public_minus_lite = mean_delta(&per_cell);
    let missing_lite_cells = per_cell
        .iter()
        .filter(|row| row.status == "missing_lite")
        .count();
    let missing_public_cells = per_cell
        .iter()
        .filter(|row| row.status == "missing_public")
        .count();
    let cells_compared = per_cell
        .iter()
        .filter(|row| row.status == "compared")
        .count();

    let ingest = run_mejepa_train(MejepaTrainArgs {
        corpus: vec![index_path.clone()],
        split: args.split,
        db_path: args.db_path.clone(),
        max_tasks: None,
        quarantine_config: PathBuf::from(DEFAULT_CORPUS_QUARANTINE_PATH),
    })?;
    ensure!(
        ingest.input_task_count == observations.len(),
        "MEJEPA_PUBLIC_CI_INGEST_COUNT_MISMATCH: ingest wrote {} rows but comparison selected {}",
        ingest.input_task_count,
        observations.len()
    );

    let mut output = PublicCiCrossValidationOutput {
        public_corpus: index.public_ci_source,
        public_corpus_root: corpus_root.display().to_string(),
        public_example_count: observations.len(),
        min_examples: args.min_examples,
        split: args.split,
        db_path: args.db_path.display().to_string(),
        lite_baseline_path: args.lite_baseline.display().to_string(),
        cells_compared,
        missing_lite_cells,
        missing_public_cells,
        mean_delta_public_minus_lite,
        per_cell,
        readback_equal: ingest.readback_equal,
        ingest,
        report_path: args
            .report_out
            .as_ref()
            .map(|path| path.display().to_string()),
        report_readback_equal: true,
        source_of_truth: json!({
            "public_corpus_index": index_path,
            "lite_baseline": args.lite_baseline,
            "db_path": args.db_path,
            "ingest_writer": "context-graph-cli mejepa train",
            "panel_cf": context_graph_mejepa_cf::CF_MEJEPA_PANELS,
            "dda_cf": context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS,
            "live_prediction_write_policy": "disabled: corpus ingest writes target-side training evidence only",
            "oracle_cf": context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
            "train_cert_cf": context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
        }),
    };

    if let Some(report_out) = &args.report_out {
        output.report_readback_equal = write_report_with_readback(report_out, &output)?;
    }
    Ok(output)
}

fn validate_args(args: &PublicCiCrossValidationArgs) -> Result<()> {
    ensure!(
        args.public_corpus.exists(),
        "MEJEPA_PUBLIC_CI_CORPUS_MISSING: {}",
        args.public_corpus.display()
    );
    ensure!(
        args.lite_baseline.exists(),
        "MEJEPA_PUBLIC_CI_BASELINE_MISSING: {}",
        args.lite_baseline.display()
    );
    ensure!(
        args.min_examples > 0,
        "MEJEPA_PUBLIC_CI_MIN_EXAMPLES_INVALID: --min-examples must be >= 1"
    );
    Ok(())
}

fn load_public_index(path: &Path) -> Result<(PathBuf, PathBuf, PublicCorpusIndex)> {
    let index_path = if path.is_dir() {
        path.join("index.json")
    } else {
        path.to_path_buf()
    };
    let text = fs::read_to_string(&index_path).with_context(|| {
        format!(
            "MEJEPA_PUBLIC_CI_INDEX_READ_FAILED: {}",
            index_path.display()
        )
    })?;
    let index: PublicCorpusIndex = serde_json::from_str(&text).with_context(|| {
        format!(
            "MEJEPA_PUBLIC_CI_INDEX_JSON_INVALID: {}",
            index_path.display()
        )
    })?;
    let corpus_root = index_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("MEJEPA_PUBLIC_CI_INDEX_PARENT_MISSING"))?;
    Ok((index_path, corpus_root, index))
}

fn validate_public_source(index: &PublicCorpusIndex) -> Result<()> {
    let version = index.corpus_version.to_ascii_lowercase();
    ensure!(
        version.contains("ghalogs"),
        "MEJEPA_PUBLIC_CI_SOURCE_UNSUPPORTED: corpus_version must identify GHALogs"
    );
    ensure!(
        index
            .public_ci_source
            .name
            .to_ascii_lowercase()
            .contains("ghalogs"),
        "MEJEPA_PUBLIC_CI_SOURCE_UNSUPPORTED: source name must identify GHALogs"
    );
    ensure!(
        index.public_ci_source.doi == GHALOGS_DOI,
        "MEJEPA_PUBLIC_CI_SOURCE_UNSUPPORTED: expected DOI {GHALOGS_DOI}, got {}",
        index.public_ci_source.doi
    );
    ensure!(
        index.public_ci_source.zenodo_url.contains("zenodo.org")
            && index.public_ci_source.zenodo_url.contains("10154920"),
        "MEJEPA_PUBLIC_CI_SOURCE_UNSUPPORTED: zenodo_url must point to the GHALogs record"
    );
    ensure!(
        index
            .public_ci_source
            .repository_url
            .contains("D2KLab/gha-dataset"),
        "MEJEPA_PUBLIC_CI_SOURCE_UNSUPPORTED: repository_url must point to D2KLab/gha-dataset"
    );
    if let Some(corpus_sha256) = &index.corpus_sha256 {
        validate_sha256(corpus_sha256, "corpus_sha256")?;
    }
    Ok(())
}

fn load_lite_baseline(path: &Path) -> Result<LiteBaseline> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("MEJEPA_PUBLIC_CI_BASELINE_READ_FAILED: {}", path.display()))?;
    let baseline: LiteBaseline = serde_json::from_str(&text)
        .with_context(|| format!("MEJEPA_PUBLIC_CI_BASELINE_JSON_INVALID: {}", path.display()))?;
    for (cell, value) in &baseline.per_cell_correlation {
        validate_cell_key(cell)?;
        if let Some(correlation) = value {
            ensure!(
                correlation.is_finite() && (-1.0..=1.0).contains(correlation),
                "MEJEPA_PUBLIC_CI_BASELINE_INVALID: correlation for {cell} must be finite in [-1,1]"
            );
        }
    }
    Ok(baseline)
}

fn selected_public_observations(
    index: &PublicCorpusIndex,
    split: TrainSplitArg,
) -> Result<Vec<PublicObservation>> {
    let mut observations = Vec::new();
    for (entry_idx, entry) in index.entries.iter().enumerate() {
        if entry.bucket != split.public_ci_split_name() {
            continue;
        }
        validate_public_entry(entry, entry_idx)?;
        observations.push(PublicObservation {
            cell: format!("{}::{}", entry.category, entry.language),
            predicted: entry.predicted_oracle_pass,
            actual: if entry.oracle_all_passed { 1.0 } else { 0.0 },
        });
    }
    Ok(observations)
}

fn validate_public_entry(entry: &PublicCorpusEntry, entry_idx: usize) -> Result<()> {
    ensure!(
        !entry.task_id.trim().is_empty(),
        "MEJEPA_PUBLIC_CI_ENTRY_INVALID: entry {entry_idx} task_id is empty"
    );
    ensure!(
        !entry.repo.trim().is_empty(),
        "MEJEPA_PUBLIC_CI_ENTRY_INVALID: entry {entry_idx} repo is empty"
    );
    ensure!(
        !entry.patch_path.trim().is_empty(),
        "MEJEPA_PUBLIC_CI_ENTRY_INVALID: entry {entry_idx} patch_path is empty"
    );
    validate_sha256(&entry.patch_sha256, "patch_sha256")?;
    if let Some(verdict_sha256) = &entry.oracle_verdict_sha256 {
        validate_sha256(verdict_sha256, "oracle_verdict_sha256")?;
    }
    if let Some(test_count) = entry.oracle_per_test_count {
        ensure!(
            test_count > 0,
            "MEJEPA_PUBLIC_CI_ENTRY_INVALID: entry {entry_idx} oracle_per_test_count must be >= 1"
        );
    }
    if let Some(note) = &entry.mutation_note {
        ensure!(
            !note.trim().is_empty(),
            "MEJEPA_PUBLIC_CI_ENTRY_INVALID: entry {entry_idx} mutation_note is empty"
        );
    }
    if let Some(exception) = &entry.oracle_exception {
        ensure!(
            !exception.trim().is_empty(),
            "MEJEPA_PUBLIC_CI_ENTRY_INVALID: entry {entry_idx} oracle_exception is empty"
        );
    }
    validate_cell_key(&format!("{}::{}", entry.category, entry.language))?;
    ensure!(
        entry.predicted_oracle_pass.is_finite()
            && (0.0..=1.0).contains(&entry.predicted_oracle_pass),
        "MEJEPA_PUBLIC_CI_ENTRY_INVALID: entry {entry_idx} predicted_oracle_pass must be finite in [0,1]"
    );
    Ok(())
}

fn validate_cell_key(cell: &str) -> Result<()> {
    let Some((category, language)) = cell.split_once("::") else {
        return Err(anyhow!(
            "MEJEPA_PUBLIC_CI_CELL_INVALID: cell key must be category::language, got {cell:?}"
        ));
    };
    ensure!(
        !category.trim().is_empty() && !language.trim().is_empty(),
        "MEJEPA_PUBLIC_CI_CELL_INVALID: cell key has empty category or language: {cell:?}"
    );
    Ok(())
}

fn validate_sha256(value: &str, field: &str) -> Result<()> {
    let raw = value.strip_prefix("sha256:").unwrap_or(value);
    ensure!(
        raw.len() == 64 && raw.bytes().all(|b| b.is_ascii_hexdigit()),
        "MEJEPA_PUBLIC_CI_SHA_INVALID: {field} must be sha256 hex, got {value:?}"
    );
    Ok(())
}

fn per_cell_correlations(observations: &[PublicObservation]) -> BTreeMap<String, CellStats> {
    let mut grouped: BTreeMap<String, Vec<(f32, f32)>> = BTreeMap::new();
    for observation in observations {
        grouped
            .entry(observation.cell.clone())
            .or_default()
            .push((observation.predicted, observation.actual));
    }
    grouped
        .into_iter()
        .map(|(cell, samples)| {
            let count = samples.len();
            (
                cell,
                CellStats {
                    count,
                    correlation: pearson_correlation(&samples),
                },
            )
        })
        .collect()
}

#[derive(Debug)]
struct CellStats {
    count: usize,
    correlation: Option<f32>,
}

fn compare_cells(
    lite: &BTreeMap<String, Option<f32>>,
    public: &BTreeMap<String, CellStats>,
) -> Vec<PublicCiCellComparison> {
    let mut keys: Vec<String> = lite.keys().chain(public.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|cell| {
            let lite_correlation = lite.get(&cell).copied().flatten();
            let public_stats = public.get(&cell);
            let public_ci_correlation = public_stats.and_then(|stats| stats.correlation);
            let public_example_count = public_stats.map(|stats| stats.count).unwrap_or(0);
            let delta_public_minus_lite = match (lite_correlation, public_ci_correlation) {
                (Some(lite_value), Some(public_value)) => Some(public_value - lite_value),
                _ => None,
            };
            let status = match (lite_correlation, public_stats, public_ci_correlation) {
                (Some(_), Some(_), Some(_)) => "compared",
                (None, Some(_), Some(_)) => "missing_lite",
                (Some(_), None, _) => "missing_public",
                (Some(_), Some(_), None) => "public_unavailable",
                _ => "unavailable",
            };
            PublicCiCellComparison {
                cell,
                lite_correlation,
                public_ci_correlation,
                delta_public_minus_lite,
                public_example_count,
                status: status.to_string(),
            }
        })
        .collect()
}

fn mean_delta(rows: &[PublicCiCellComparison]) -> Option<f32> {
    let deltas: Vec<f32> = rows
        .iter()
        .filter_map(|row| row.delta_public_minus_lite)
        .collect();
    if deltas.is_empty() {
        return None;
    }
    Some(deltas.iter().sum::<f32>() / deltas.len() as f32)
}

fn pearson_correlation(samples: &[(f32, f32)]) -> Option<f32> {
    if samples.len() < 2 {
        return None;
    }
    let n = samples.len() as f64;
    let mean_x = samples
        .iter()
        .map(|(predicted, _)| f64::from(*predicted))
        .sum::<f64>()
        / n;
    let mean_y = samples
        .iter()
        .map(|(_, actual)| f64::from(*actual))
        .sum::<f64>()
        / n;
    let mut numerator = 0.0_f64;
    let mut x_variance = 0.0_f64;
    let mut y_variance = 0.0_f64;
    for (predicted, actual) in samples {
        let dx = f64::from(*predicted) - mean_x;
        let dy = f64::from(*actual) - mean_y;
        numerator += dx * dy;
        x_variance += dx * dx;
        y_variance += dy * dy;
    }
    let denominator = (x_variance * y_variance).sqrt();
    if denominator <= f64::EPSILON {
        return None;
    }
    Some((numerator / denominator).clamp(-1.0, 1.0) as f32)
}

fn write_report_with_readback(path: &Path, output: &PublicCiCrossValidationOutput) -> Result<bool> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "MEJEPA_PUBLIC_CI_REPORT_PARENT_CREATE_FAILED: {}",
                parent.display()
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(output)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .with_context(|| format!("MEJEPA_PUBLIC_CI_REPORT_OPEN_FAILED: {}", path.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("MEJEPA_PUBLIC_CI_REPORT_WRITE_FAILED: {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("MEJEPA_PUBLIC_CI_REPORT_SYNC_FAILED: {}", path.display()))?;
    drop(file);
    let readback = fs::read(path).with_context(|| {
        format!(
            "MEJEPA_PUBLIC_CI_REPORT_READBACK_FAILED: {}",
            path.display()
        )
    })?;
    ensure!(
        readback == bytes,
        "MEJEPA_PUBLIC_CI_REPORT_READBACK_MISMATCH: {}",
        path.display()
    );
    Ok(true)
}

trait TrainSplitPublicCiExt {
    fn public_ci_split_name(self) -> &'static str;
}

impl TrainSplitPublicCiExt for TrainSplitArg {
    fn public_ci_split_name(self) -> &'static str {
        match self {
            TrainSplitArg::Train => "train",
            TrainSplitArg::Calibration => "calibration",
            TrainSplitArg::Holdout => "holdout",
        }
    }
}
