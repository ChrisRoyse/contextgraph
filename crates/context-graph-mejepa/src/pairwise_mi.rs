use std::collections::{BTreeMap, BTreeSet};

use context_graph_mejepa_cf::CF_MEJEPA_PAIRWISE_MI;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;

pub const PAIRWISE_MI_SCHEMA_VERSION: u32 = 1;
pub const PAIRWISE_MI_ESTIMATOR_PARTITIONED_NMI: &str = "partitioned_histogram_nmi_v1";
pub const DEFAULT_PAIRWISE_MI_READ_MAX_ROWS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairwiseMiPairKey {
    pub schema_version: u32,
    pub corpus_shard_hash: String,
    pub embedder_pair: String,
    pub ts_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairwiseMiPairRow {
    pub schema_version: u32,
    pub corpus_shard_hash: String,
    pub embedder_pair: String,
    pub left_slot: String,
    pub right_slot: String,
    pub ts_unix_ms: i64,
    pub step: u64,
    pub sample_count: usize,
    pub mi: f32,
    pub confidence_low: f32,
    pub confidence_high: f32,
    pub estimator: String,
    pub source_of_truth_cf: String,
}

impl PairwiseMiPairRow {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != PAIRWISE_MI_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {PAIRWISE_MI_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_hex_64("corpus_shard_hash", &self.corpus_shard_hash)?;
        validate_slot("left_slot", &self.left_slot)?;
        validate_slot("right_slot", &self.right_slot)?;
        if self.left_slot >= self.right_slot {
            return invalid(
                "embedder_pair",
                "left_slot must sort before right_slot for canonical pair rows",
            );
        }
        let expected_pair = format_embedder_pair(&self.left_slot, &self.right_slot);
        if self.embedder_pair != expected_pair {
            return invalid(
                "embedder_pair",
                format!("expected canonical pair {expected_pair}"),
            );
        }
        if self.ts_unix_ms <= 0 {
            return invalid("ts_unix_ms", "must be positive");
        }
        if self.sample_count < 2 {
            return invalid("sample_count", "must be at least two");
        }
        validate_unit("mi", self.mi)?;
        validate_unit("confidence_low", self.confidence_low)?;
        validate_unit("confidence_high", self.confidence_high)?;
        if self.confidence_low > self.mi || self.confidence_high < self.mi {
            return invalid(
                "confidence_interval",
                "confidence interval must contain the MI point estimate",
            );
        }
        if self.confidence_low > self.confidence_high {
            return invalid(
                "confidence_interval",
                "confidence_low must be <= confidence_high",
            );
        }
        validate_text("estimator", &self.estimator, 128)?;
        if self.source_of_truth_cf != CF_MEJEPA_PAIRWISE_MI {
            return invalid(
                "source_of_truth_cf",
                format!("expected {CF_MEJEPA_PAIRWISE_MI}"),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairwiseMiRedundancyBin {
    pub lower_inclusive: f32,
    pub upper_exclusive: f32,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairwiseMiAdaptiveWeight {
    pub slot: String,
    pub redundancy_sum: f32,
    pub mean_redundancy: f32,
    pub adaptive_weight: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairwiseMiMatrixHealth {
    pub max_off_diagonal: f32,
    pub mean_off_diagonal: f32,
    pub effective_signal_count: f32,
    pub redundancy_histogram: Vec<PairwiseMiRedundancyBin>,
    pub adaptive_weights: Vec<PairwiseMiAdaptiveWeight>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairwiseMiPersistedMatrix {
    pub schema_version: u32,
    pub corpus_shard_hash: String,
    pub created_at_unix_ms: i64,
    pub step: u64,
    pub sample_count: usize,
    pub slots: Vec<String>,
    pub values: Vec<Vec<f32>>,
    pub pair_rows: Vec<PairwiseMiPairRow>,
    pub health: PairwiseMiMatrixHealth,
    pub source_row_count: usize,
    pub source_of_truth_cf: String,
}

impl PairwiseMiPersistedMatrix {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != PAIRWISE_MI_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {PAIRWISE_MI_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_hex_64("corpus_shard_hash", &self.corpus_shard_hash)?;
        if self.created_at_unix_ms <= 0 {
            return invalid("created_at_unix_ms", "must be positive");
        }
        if self.sample_count < 2 {
            return invalid("sample_count", "must be at least two");
        }
        validate_pairwise_mi_matrix(&self.slots, &self.values)?;
        let expected_rows = self.slots.len() * (self.slots.len() - 1) / 2;
        if self.pair_rows.len() != expected_rows {
            return Err(MejepaInferError::DimMismatch {
                expected: expected_rows,
                actual: self.pair_rows.len(),
                context: "persisted pairwise MI row count must equal N*(N-1)/2".to_string(),
            });
        }
        if self.source_row_count != self.pair_rows.len() {
            return Err(MejepaInferError::DimMismatch {
                expected: self.pair_rows.len(),
                actual: self.source_row_count,
                context: "pairwise MI source_row_count must match row count".to_string(),
            });
        }
        for row in &self.pair_rows {
            row.validate()?;
            if row.corpus_shard_hash != self.corpus_shard_hash {
                return invalid(
                    "pair_rows",
                    "pairwise MI row corpus hash differs from matrix corpus hash",
                );
            }
            if row.ts_unix_ms != self.created_at_unix_ms {
                return invalid(
                    "pair_rows",
                    "pairwise MI row timestamp differs from matrix timestamp",
                );
            }
            if row.step != self.step {
                return invalid("pair_rows", "pairwise MI row step differs from matrix step");
            }
            if row.sample_count != self.sample_count {
                return invalid(
                    "pair_rows",
                    "pairwise MI row sample_count differs from matrix sample_count",
                );
            }
        }
        if self.source_of_truth_cf != CF_MEJEPA_PAIRWISE_MI {
            return invalid(
                "source_of_truth_cf",
                format!("expected {CF_MEJEPA_PAIRWISE_MI}"),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairwiseMiCfWriteSummary {
    pub corpus_shard_hash: String,
    pub created_at_unix_ms: i64,
    pub rows_written: usize,
    pub byte_identical_readback: bool,
    pub matrix_readback_equal: bool,
    pub source_of_truth_cf: String,
}

pub fn pairwise_mi_corpus_hash(
    slots: &[String],
    values: &[Vec<f32>],
) -> Result<String, MejepaInferError> {
    validate_pairwise_mi_matrix(slots, values)?;
    let mut hasher = Sha256::new();
    hasher.update((slots.len() as u64).to_be_bytes());
    for slot in slots {
        hasher.update(slot.as_bytes());
        hasher.update([0]);
    }
    for row in values {
        for value in row {
            hasher.update(value.to_bits().to_be_bytes());
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn pairwise_mi_rows_from_matrix(
    corpus_shard_hash: &str,
    slots: &[String],
    values: &[Vec<f32>],
    step: u64,
    sample_count: usize,
    created_at_unix_ms: i64,
    estimator: &str,
) -> Result<Vec<PairwiseMiPairRow>, MejepaInferError> {
    validate_hex_64("corpus_shard_hash", corpus_shard_hash)?;
    validate_pairwise_mi_matrix(slots, values)?;
    if sample_count < 2 {
        return invalid("sample_count", "must be at least two");
    }
    if created_at_unix_ms <= 0 {
        return invalid("created_at_unix_ms", "must be positive");
    }
    validate_text("estimator", estimator, 128)?;
    let half_width = confidence_half_width(sample_count);
    let mut rows = Vec::with_capacity(slots.len() * (slots.len() - 1) / 2);
    for row_idx in 0..slots.len() {
        for col_idx in (row_idx + 1)..slots.len() {
            let mi = values[row_idx][col_idx];
            let row = PairwiseMiPairRow {
                schema_version: PAIRWISE_MI_SCHEMA_VERSION,
                corpus_shard_hash: corpus_shard_hash.to_string(),
                embedder_pair: format_embedder_pair(&slots[row_idx], &slots[col_idx]),
                left_slot: slots[row_idx].clone(),
                right_slot: slots[col_idx].clone(),
                ts_unix_ms: created_at_unix_ms,
                step,
                sample_count,
                mi,
                confidence_low: (mi - half_width).max(0.0),
                confidence_high: (mi + half_width).min(1.0),
                estimator: estimator.to_string(),
                source_of_truth_cf: CF_MEJEPA_PAIRWISE_MI.to_string(),
            };
            row.validate()?;
            rows.push(row);
        }
    }
    Ok(rows)
}

pub fn write_pairwise_mi_matrix_sync_readback(
    db: &DB,
    corpus_shard_hash: &str,
    slots: &[String],
    values: &[Vec<f32>],
    step: u64,
    sample_count: usize,
    created_at_unix_ms: i64,
    estimator: &str,
) -> Result<PairwiseMiCfWriteSummary, MejepaInferError> {
    let rows = pairwise_mi_rows_from_matrix(
        corpus_shard_hash,
        slots,
        values,
        step,
        sample_count,
        created_at_unix_ms,
        estimator,
    )?;
    let cf = cf(db, CF_MEJEPA_PAIRWISE_MI)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    let mut encoded_by_key = Vec::with_capacity(rows.len());
    for row in &rows {
        let key = pairwise_mi_key(row)?;
        let value = bincode::serialize(row)?;
        db.put_cf_opt(cf, &key, &value, &opts)?;
        encoded_by_key.push((key, value));
    }
    db.flush_cf(cf)?;

    for (key, value) in &encoded_by_key {
        let Some(readback) = db.get_cf(cf, key)? else {
            return invalid(
                "pairwise_mi_readback",
                "sync write readback returned no row",
            );
        };
        if &readback != value {
            return invalid(
                "pairwise_mi_readback",
                "sync write readback bytes differ from encoded input",
            );
        }
        let decoded: PairwiseMiPairRow = bincode::deserialize(&readback)?;
        decoded.validate()?;
    }

    let matrix = read_pairwise_mi_matrix(
        db,
        Some(corpus_shard_hash),
        Some(created_at_unix_ms),
        DEFAULT_PAIRWISE_MI_READ_MAX_ROWS,
    )?;
    let matrix_readback_equal = matrix.slots == slots && matrix.values == values;
    if !matrix_readback_equal {
        return invalid(
            "pairwise_mi_readback",
            "reconstructed persisted matrix differs from input matrix",
        );
    }
    Ok(PairwiseMiCfWriteSummary {
        corpus_shard_hash: corpus_shard_hash.to_string(),
        created_at_unix_ms,
        rows_written: rows.len(),
        byte_identical_readback: true,
        matrix_readback_equal,
        source_of_truth_cf: CF_MEJEPA_PAIRWISE_MI.to_string(),
    })
}

pub fn read_pairwise_mi_matrix(
    db: &DB,
    corpus_shard_hash: Option<&str>,
    created_at_unix_ms: Option<i64>,
    max_rows: usize,
) -> Result<PairwiseMiPersistedMatrix, MejepaInferError> {
    if max_rows == 0 {
        return invalid("max_rows", "must be greater than zero");
    }
    if let Some(hash) = corpus_shard_hash {
        validate_hex_64("corpus_shard_hash", hash)?;
    }
    if created_at_unix_ms.is_some_and(|value| value <= 0) {
        return invalid("created_at_unix_ms", "must be positive");
    }

    let cf = cf(db, CF_MEJEPA_PAIRWISE_MI)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let row: PairwiseMiPairRow = bincode::deserialize(&value)?;
        row.validate()?;
        if corpus_shard_hash.is_some_and(|want| row.corpus_shard_hash != want) {
            continue;
        }
        if created_at_unix_ms.is_some_and(|want| row.ts_unix_ms != want) {
            continue;
        }
        rows.push(row);
        if rows.len() > max_rows {
            return invalid(
                "max_rows",
                format!("pairwise MI read exceeded max_rows {max_rows}"),
            );
        }
    }
    if rows.is_empty() {
        return invalid(
            "CF_MEJEPA_PAIRWISE_MI",
            "no pairwise MI rows matched requested corpus hash/timestamp",
        );
    }

    let selected_ts = created_at_unix_ms.unwrap_or_else(|| {
        rows.iter()
            .map(|row| row.ts_unix_ms)
            .max()
            .expect("non-empty rows")
    });
    let selected_hash = corpus_shard_hash.map(ToOwned::to_owned).unwrap_or_else(|| {
        rows.iter()
            .filter(|row| row.ts_unix_ms == selected_ts)
            .map(|row| row.corpus_shard_hash.clone())
            .max()
            .expect("rows for latest timestamp")
    });
    let mut pair_rows = rows
        .into_iter()
        .filter(|row| row.ts_unix_ms == selected_ts && row.corpus_shard_hash == selected_hash)
        .collect::<Vec<_>>();
    pair_rows.sort_by(|left, right| {
        left.left_slot
            .cmp(&right.left_slot)
            .then(left.right_slot.cmp(&right.right_slot))
    });
    matrix_from_pair_rows(selected_hash, selected_ts, pair_rows)
}

pub fn summarize_pairwise_mi_matrix(
    slots: &[String],
    values: &[Vec<f32>],
) -> Result<PairwiseMiMatrixHealth, MejepaInferError> {
    validate_pairwise_mi_matrix(slots, values)?;
    let n = slots.len();
    let mut off_diag_sum = 0.0f32;
    let mut max_off_diagonal = 0.0f32;
    let mut off_diag_count = 0usize;
    let edges = [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.000001];
    let mut counts = vec![0usize; edges.len() - 1];
    for (row_idx, row) in values.iter().enumerate().take(n) {
        for value in row.iter().skip(row_idx + 1) {
            let value = *value;
            max_off_diagonal = max_off_diagonal.max(value);
            off_diag_sum += value;
            off_diag_count += 1;
            let bin = edges
                .windows(2)
                .position(|window| value >= window[0] && value < window[1])
                .unwrap_or(edges.len() - 2);
            counts[bin] += 1;
        }
    }
    let mut effective_signal_count = 0.0f32;
    let mut raw_weights = Vec::with_capacity(n);
    for (row_idx, row) in values.iter().enumerate().take(n) {
        let redundancy_sum = row
            .iter()
            .enumerate()
            .filter(|(col_idx, _)| *col_idx != row_idx)
            .map(|(_, value)| *value)
            .sum::<f32>();
        let effective_share = 1.0 / (1.0 + redundancy_sum);
        effective_signal_count += effective_share;
        raw_weights.push((redundancy_sum, effective_share));
    }
    let mean_raw_weight = raw_weights.iter().map(|(_, weight)| *weight).sum::<f32>() / n as f32;
    let adaptive_weights = slots
        .iter()
        .cloned()
        .zip(raw_weights)
        .map(
            |(slot, (redundancy_sum, raw_weight))| PairwiseMiAdaptiveWeight {
                slot,
                redundancy_sum,
                mean_redundancy: if n <= 1 {
                    0.0
                } else {
                    redundancy_sum / (n - 1) as f32
                },
                adaptive_weight: if mean_raw_weight <= f32::EPSILON {
                    1.0
                } else {
                    raw_weight / mean_raw_weight
                },
            },
        )
        .collect::<Vec<_>>();
    Ok(PairwiseMiMatrixHealth {
        max_off_diagonal,
        mean_off_diagonal: if off_diag_count == 0 {
            0.0
        } else {
            off_diag_sum / off_diag_count as f32
        },
        effective_signal_count,
        redundancy_histogram: edges
            .windows(2)
            .zip(counts)
            .map(|(window, count)| PairwiseMiRedundancyBin {
                lower_inclusive: window[0],
                upper_exclusive: window[1].min(1.0),
                count,
            })
            .collect(),
        adaptive_weights,
    })
}

pub fn validate_pairwise_mi_matrix(
    slots: &[String],
    values: &[Vec<f32>],
) -> Result<(), MejepaInferError> {
    if slots.len() < 2 {
        return invalid("slots", "pairwise MI matrix requires at least two slots");
    }
    let mut seen = BTreeSet::new();
    for slot in slots {
        validate_slot("slots", slot)?;
        if !seen.insert(slot) {
            return invalid("slots", format!("duplicate slot {slot}"));
        }
    }
    if values.len() != slots.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: slots.len(),
            actual: values.len(),
            context: "pairwise MI matrix row count".to_string(),
        });
    }
    for (row_idx, row) in values.iter().enumerate() {
        if row.len() != slots.len() {
            return Err(MejepaInferError::DimMismatch {
                expected: slots.len(),
                actual: row.len(),
                context: format!("pairwise MI matrix column count for row {row_idx}"),
            });
        }
        for (col_idx, value) in row.iter().enumerate() {
            validate_unit(&format!("values[{row_idx}][{col_idx}]"), *value)?;
            if row_idx == col_idx && (*value - 1.0).abs() > 1e-6 {
                return invalid(
                    "values",
                    format!("pairwise MI diagonal at {row_idx} must be 1.0"),
                );
            }
        }
    }
    for (row_idx, row) in values.iter().enumerate() {
        for (col_idx, col_row) in values.iter().enumerate().skip(row_idx + 1) {
            if (row[col_idx] - col_row[row_idx]).abs() > 1e-6 {
                return invalid(
                    "values",
                    format!("matrix must be symmetric at ({row_idx}, {col_idx})"),
                );
            }
        }
    }
    Ok(())
}

fn matrix_from_pair_rows(
    corpus_shard_hash: String,
    created_at_unix_ms: i64,
    pair_rows: Vec<PairwiseMiPairRow>,
) -> Result<PairwiseMiPersistedMatrix, MejepaInferError> {
    if pair_rows.is_empty() {
        return invalid(
            "pair_rows",
            "cannot reconstruct matrix from empty pair rows",
        );
    }
    let first = &pair_rows[0];
    let step = first.step;
    let sample_count = first.sample_count;
    let mut slot_set = BTreeSet::new();
    for row in &pair_rows {
        row.validate()?;
        if row.corpus_shard_hash != corpus_shard_hash {
            return invalid("pair_rows", "mixed corpus hashes in one persisted matrix");
        }
        if row.ts_unix_ms != created_at_unix_ms {
            return invalid("pair_rows", "mixed timestamps in one persisted matrix");
        }
        if row.step != step {
            return invalid("pair_rows", "mixed training steps in one persisted matrix");
        }
        if row.sample_count != sample_count {
            return invalid("pair_rows", "mixed sample counts in one persisted matrix");
        }
        slot_set.insert(row.left_slot.clone());
        slot_set.insert(row.right_slot.clone());
    }
    let slots = slot_set.into_iter().collect::<Vec<_>>();
    let slot_index = slots
        .iter()
        .enumerate()
        .map(|(idx, slot)| (slot.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let n = slots.len();
    let expected_rows = n * (n - 1) / 2;
    if pair_rows.len() != expected_rows {
        return Err(MejepaInferError::DimMismatch {
            expected: expected_rows,
            actual: pair_rows.len(),
            context: "pairwise MI CF rows required to reconstruct full matrix".to_string(),
        });
    }
    let mut values = vec![vec![0.0f32; n]; n];
    for (idx, row) in values.iter_mut().enumerate() {
        row[idx] = 1.0;
    }
    let mut seen_pairs = BTreeSet::new();
    for row in &pair_rows {
        let left = *slot_index
            .get(&row.left_slot)
            .expect("slot set built from pair rows");
        let right = *slot_index
            .get(&row.right_slot)
            .expect("slot set built from pair rows");
        if !seen_pairs.insert((left, right)) {
            return invalid("pair_rows", format!("duplicate pair {}", row.embedder_pair));
        }
        values[left][right] = row.mi;
        values[right][left] = row.mi;
    }
    validate_pairwise_mi_matrix(&slots, &values)?;
    let health = summarize_pairwise_mi_matrix(&slots, &values)?;
    let matrix = PairwiseMiPersistedMatrix {
        schema_version: PAIRWISE_MI_SCHEMA_VERSION,
        corpus_shard_hash,
        created_at_unix_ms,
        step,
        sample_count,
        slots,
        values,
        source_row_count: pair_rows.len(),
        pair_rows,
        health,
        source_of_truth_cf: CF_MEJEPA_PAIRWISE_MI.to_string(),
    };
    matrix.validate()?;
    Ok(matrix)
}

fn pairwise_mi_key(row: &PairwiseMiPairRow) -> Result<Vec<u8>, MejepaInferError> {
    row.validate()?;
    let key = PairwiseMiPairKey {
        schema_version: PAIRWISE_MI_SCHEMA_VERSION,
        corpus_shard_hash: row.corpus_shard_hash.clone(),
        embedder_pair: row.embedder_pair.clone(),
        ts_unix_ms: row.ts_unix_ms,
    };
    Ok(bincode::serialize(&key)?)
}

fn confidence_half_width(sample_count: usize) -> f32 {
    (1.96 / (sample_count as f32).sqrt()).clamp(0.01, 0.5)
}

fn format_embedder_pair(left: &str, right: &str) -> String {
    format!("{left}::{right}")
}

fn validate_hex_64(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(field, "must be a 64-character lowercase/uppercase hex hash");
    }
    Ok(())
}

fn validate_slot(field: &str, value: &str) -> Result<(), MejepaInferError> {
    validate_text(field, value, 256)?;
    if value.contains("::") {
        return invalid(
            field,
            "slot names may not contain the canonical pair separator ::",
        );
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if value.len() > max_len {
        return invalid(
            field,
            format!("length {} exceeds max {max_len}", value.len()),
        );
    }
    Ok(())
}

fn validate_unit(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(field, format!("must be finite in [0, 1], got {value}"));
    }
    Ok(())
}

fn invalid<T>(field: impl Into<String>, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    })
}
