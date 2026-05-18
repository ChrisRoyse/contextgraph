//! TASK-PY-G-042 (#261) patch-similarity exemplar retrieval.
//!
//! The live prediction surface needs exemplar rows that come from the frozen
//! structural instruments, not from prompt text or a heuristic label lookup.
//! This module owns the schema-versioned HNSW graph, row manifest, and
//! metadata CF readback for that index.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::CF_MEJEPA_PATCH_SIMILARITY_META;
use context_graph_mejepa_instruments::{InstrumentSlot, OracleVerdict, Panel};
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::eval::MutationCategory;
use crate::types::{ExemplarMatch, FailureModeClass, TaskId, WitnessHash};

pub const PATCH_SIMILARITY_SCHEMA_VERSION: u32 = 1;
pub const PATCH_SIMILARITY_STRUCTURAL_DIM: usize = 384 + 256 + 256 + 256;
pub const PATCH_SIMILARITY_META_KEY: &[u8] = b"active";
pub const PATCH_SIMILARITY_GRAPH_FILE: &str = "patch_similarity_hnsw.usearch";
pub const PATCH_SIMILARITY_RECORDS_FILE: &str = "patch_similarity_records.json";
pub const PATCH_SIMILARITY_META_FILE: &str = "patch_similarity_meta.json";
pub const PATCH_SIMILARITY_DEFAULT_K: usize = 5;
const HNSW_M: usize = 16;
const HNSW_EF_CONSTRUCTION: usize = 128;
const HNSW_EF_SEARCH: usize = 64;
const MAX_PATCH_SIMILARITY_RECORDS: usize = 1_000_000;
const MAX_EVIDENCE_PATH_BYTES: usize = 8192;
const NORMALIZED_EPSILON: f32 = 1.0e-3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchExemplarRecord {
    pub exemplar_prediction_id: [u8; 16],
    pub task_id: TaskId,
    pub mutation_kind: MutationCategory,
    pub oracle_outcome: OracleVerdict,
    pub failure_mode_class: FailureModeClass,
    pub witness_hash: WitnessHash,
    pub evidence_path: String,
    pub diff_summary: String,
    pub vector: Vec<f32>,
}

impl PatchExemplarRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.exemplar_prediction_id.iter().all(|byte| *byte == 0) {
            return Err(MejepaInferError::InvalidInput {
                field: "patch_exemplar_record.exemplar_prediction_id".to_string(),
                detail: "exemplar_prediction_id must be non-zero".to_string(),
            });
        }
        self.task_id.validate("patch_exemplar_record.task_id")?;
        if self.oracle_outcome.per_test.is_empty() && self.oracle_outcome.exception.is_none() {
            return Err(MejepaInferError::InvalidInput {
                field: "patch_exemplar_record.oracle_outcome".to_string(),
                detail: "oracle_outcome must contain at least one test or exception".to_string(),
            });
        }
        validate_bounded_text(
            "patch_exemplar_record.evidence_path",
            &self.evidence_path,
            MAX_EVIDENCE_PATH_BYTES,
        )?;
        validate_bounded_text(
            "patch_exemplar_record.diff_summary",
            &self.diff_summary,
            MAX_EVIDENCE_PATH_BYTES,
        )?;
        validate_patch_vector("patch_exemplar_record.vector", &self.vector)?;
        Ok(())
    }

    fn normalized(mut self) -> Result<Self, MejepaInferError> {
        normalize_l2_in_place("patch_exemplar_record.vector", &mut self.vector)?;
        self.validate()?;
        Ok(self)
    }

    fn to_match(&self, similarity_score: f32) -> ExemplarMatch {
        ExemplarMatch {
            exemplar_prediction_id: Some(self.exemplar_prediction_id),
            task_id: self.task_id.clone(),
            mutation_kind: self.mutation_kind,
            similarity_score,
            diff_summary: self.diff_summary.clone(),
            oracle_outcome: self.oracle_outcome.clone(),
            failure_mode_class: Some(self.failure_mode_class),
            evidence_path: Some(self.evidence_path.clone()),
            witness_hash: self.witness_hash,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchSimilarityIndexMetadata {
    pub schema_version: u32,
    pub corpus_snapshot_hash: String,
    pub row_count: usize,
    pub vector_dim: usize,
    pub graph_sha256: String,
    pub records_sha256: String,
    pub built_at_unix_ms: i64,
}

impl PatchSimilarityIndexMetadata {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != PATCH_SIMILARITY_SCHEMA_VERSION {
            return Err(MejepaInferError::InvalidInput {
                field: "patch_similarity_meta.schema_version".to_string(),
                detail: format!(
                    "expected schema version {PATCH_SIMILARITY_SCHEMA_VERSION}; got {}",
                    self.schema_version
                ),
            });
        }
        validate_sha256_hex(
            "patch_similarity_meta.corpus_snapshot_hash",
            &self.corpus_snapshot_hash,
        )?;
        validate_sha256_hex("patch_similarity_meta.graph_sha256", &self.graph_sha256)?;
        validate_sha256_hex("patch_similarity_meta.records_sha256", &self.records_sha256)?;
        if self.row_count == 0 || self.row_count > MAX_PATCH_SIMILARITY_RECORDS {
            return Err(MejepaInferError::InvalidInput {
                field: "patch_similarity_meta.row_count".to_string(),
                detail: format!(
                    "row_count must be in [1, {MAX_PATCH_SIMILARITY_RECORDS}], got {}",
                    self.row_count
                ),
            });
        }
        if self.vector_dim != PATCH_SIMILARITY_STRUCTURAL_DIM {
            return Err(MejepaInferError::DimMismatch {
                expected: PATCH_SIMILARITY_STRUCTURAL_DIM,
                actual: self.vector_dim,
                context: "patch_similarity_meta.vector_dim".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchSimilarityBuildReport {
    pub index_dir: PathBuf,
    pub metadata: PatchSimilarityIndexMetadata,
    pub metadata_cf: String,
    pub metadata_cf_key_hex: String,
    pub metadata_cf_readback_equal: bool,
    pub graph_file: PathBuf,
    pub records_file: PathBuf,
    pub metadata_file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchSimilaritySearchStatus {
    Found,
    NoNeighborhoodCoverage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchSimilaritySearchOutcome {
    pub status: PatchSimilaritySearchStatus,
    pub reason_code: Option<String>,
    pub requested_k: usize,
    pub returned: usize,
    pub exemplars: Vec<ExemplarMatch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatchSimilarityQuery {
    pub vector: Vec<f32>,
    pub k: usize,
    pub has_neighborhood_coverage: bool,
}

impl PatchSimilarityQuery {
    pub fn new(
        vector: Vec<f32>,
        k: usize,
        has_neighborhood_coverage: bool,
    ) -> Result<Self, MejepaInferError> {
        if k == 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "patch_similarity_query.k".to_string(),
                detail: "k must be >= 1".to_string(),
            });
        }
        validate_patch_vector("patch_similarity_query.vector", &vector)?;
        Ok(Self {
            vector,
            k,
            has_neighborhood_coverage,
        })
    }
}

pub struct PatchSimilarityIndex {
    metadata: PatchSimilarityIndexMetadata,
    records: Vec<PatchExemplarRecord>,
    index: Index,
}

impl PatchSimilarityIndex {
    pub fn metadata(&self) -> &PatchSimilarityIndexMetadata {
        &self.metadata
    }

    pub fn row_count(&self) -> usize {
        self.records.len()
    }
}

pub fn patch_structural_signature_from_panel(panel: &Panel) -> Result<Vec<f32>, MejepaInferError> {
    let slots = [
        InstrumentSlot::EAst,
        InstrumentSlot::ECfg,
        InstrumentSlot::EDataFlow,
        InstrumentSlot::ETypeGraph,
    ];
    let mut out = Vec::with_capacity(PATCH_SIMILARITY_STRUCTURAL_DIM);
    for slot in slots {
        if !panel.is_filled(slot) {
            return Err(MejepaInferError::InvalidInput {
                field: "patch_similarity.panel_slot".to_string(),
                detail: format!("required structural slot {} is not filled", slot.slug()),
            });
        }
        out.extend_from_slice(panel.slot(slot));
    }
    normalize_l2_in_place("patch_similarity.panel_signature", &mut out)?;
    Ok(out)
}

pub fn build_patch_similarity_index(
    records: Vec<PatchExemplarRecord>,
    index_dir: impl AsRef<Path>,
    corpus_snapshot_hash: impl Into<String>,
    db: Option<&DB>,
) -> Result<PatchSimilarityBuildReport, MejepaInferError> {
    let corpus_snapshot_hash = corpus_snapshot_hash.into();
    validate_sha256_hex(
        "patch_similarity_build.corpus_snapshot_hash",
        &corpus_snapshot_hash,
    )?;
    if records.is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: "patch_similarity_records".to_string(),
            detail: "at least one exemplar record is required".to_string(),
        });
    }
    if records.len() > MAX_PATCH_SIMILARITY_RECORDS {
        return Err(MejepaInferError::DimMismatch {
            expected: MAX_PATCH_SIMILARITY_RECORDS,
            actual: records.len(),
            context: "patch_similarity_records exceeds maximum".to_string(),
        });
    }

    let mut normalized_records = Vec::with_capacity(records.len());
    for record in records {
        normalized_records.push(record.normalized()?);
    }

    let index_dir = index_dir.as_ref().to_path_buf();
    fs::create_dir_all(&index_dir)
        .map_err(|err| MejepaInferError::io("create_dir_all", index_dir.clone(), err))?;

    let index = new_hnsw_index()?;
    index
        .reserve(normalized_records.len())
        .map_err(|err| MejepaInferError::InvalidInput {
            field: "patch_similarity_index.reserve".to_string(),
            detail: format!("usearch reserve failed: {err}"),
        })?;
    for (row_idx, record) in normalized_records.iter().enumerate() {
        index.add(row_idx as u64, &record.vector).map_err(|err| {
            MejepaInferError::InvalidInput {
                field: "patch_similarity_index.add".to_string(),
                detail: format!("usearch add failed at row {row_idx}: {err}"),
            }
        })?;
    }
    let graph_bytes = serialize_hnsw_graph(&index)?;
    let graph_sha256 = sha256_hex(&graph_bytes);
    let graph_file = index_dir.join(PATCH_SIMILARITY_GRAPH_FILE);
    write_bytes_atomic(&graph_file, &graph_bytes)?;

    let records_bytes = serde_json::to_vec_pretty(&normalized_records)?;
    let records_sha256 = sha256_hex(&records_bytes);
    let records_file = index_dir.join(PATCH_SIMILARITY_RECORDS_FILE);
    write_bytes_atomic(&records_file, &records_bytes)?;

    let metadata = PatchSimilarityIndexMetadata {
        schema_version: PATCH_SIMILARITY_SCHEMA_VERSION,
        corpus_snapshot_hash,
        row_count: normalized_records.len(),
        vector_dim: PATCH_SIMILARITY_STRUCTURAL_DIM,
        graph_sha256,
        records_sha256,
        built_at_unix_ms: chrono::Utc::now().timestamp_millis(),
    };
    metadata.validate()?;

    let metadata_file = index_dir.join(PATCH_SIMILARITY_META_FILE);
    let metadata_bytes = serde_json::to_vec_pretty(&metadata)?;
    write_bytes_atomic(&metadata_file, &metadata_bytes)?;
    let metadata_cf_readback_equal = if let Some(db) = db {
        persist_metadata_cf(db, &metadata_bytes)?
    } else {
        false
    };

    Ok(PatchSimilarityBuildReport {
        index_dir,
        metadata,
        metadata_cf: CF_MEJEPA_PATCH_SIMILARITY_META.to_string(),
        metadata_cf_key_hex: hex::encode(PATCH_SIMILARITY_META_KEY),
        metadata_cf_readback_equal,
        graph_file,
        records_file,
        metadata_file,
    })
}

pub fn load_patch_similarity_index(
    index_dir: impl AsRef<Path>,
    expected_corpus_snapshot_hash: &str,
) -> Result<PatchSimilarityIndex, MejepaInferError> {
    validate_sha256_hex(
        "patch_similarity_load.expected_corpus_snapshot_hash",
        expected_corpus_snapshot_hash,
    )?;
    let index_dir = index_dir.as_ref();
    let metadata_file = index_dir.join(PATCH_SIMILARITY_META_FILE);
    let metadata_bytes = fs::read(&metadata_file)
        .map_err(|err| MejepaInferError::io("read", metadata_file.clone(), err))?;
    let metadata: PatchSimilarityIndexMetadata = serde_json::from_slice(&metadata_bytes)?;
    metadata.validate()?;
    if metadata.corpus_snapshot_hash != expected_corpus_snapshot_hash {
        return Err(MejepaInferError::PatchSimilarityStaleIndex {
            expected: expected_corpus_snapshot_hash.to_string(),
            actual: metadata.corpus_snapshot_hash,
        });
    }

    let graph_file = index_dir.join(PATCH_SIMILARITY_GRAPH_FILE);
    let graph_bytes = fs::read(&graph_file)
        .map_err(|err| MejepaInferError::io("read", graph_file.clone(), err))?;
    let graph_sha256 = sha256_hex(&graph_bytes);
    if graph_sha256 != metadata.graph_sha256 {
        return Err(MejepaInferError::InvalidInput {
            field: "patch_similarity_graph.sha256".to_string(),
            detail: format!(
                "graph file hash mismatch: metadata={} observed={graph_sha256}",
                metadata.graph_sha256
            ),
        });
    }

    let records_file = index_dir.join(PATCH_SIMILARITY_RECORDS_FILE);
    let records_bytes = fs::read(&records_file)
        .map_err(|err| MejepaInferError::io("read", records_file.clone(), err))?;
    let records_sha256 = sha256_hex(&records_bytes);
    if records_sha256 != metadata.records_sha256 {
        return Err(MejepaInferError::InvalidInput {
            field: "patch_similarity_records.sha256".to_string(),
            detail: format!(
                "records file hash mismatch: metadata={} observed={records_sha256}",
                metadata.records_sha256
            ),
        });
    }
    let records: Vec<PatchExemplarRecord> = serde_json::from_slice(&records_bytes)?;
    if records.len() != metadata.row_count {
        return Err(MejepaInferError::DimMismatch {
            expected: metadata.row_count,
            actual: records.len(),
            context: "patch_similarity_records row count".to_string(),
        });
    }
    for record in &records {
        record.validate()?;
        validate_normalized_vector(&record.vector)?;
    }

    let index = new_hnsw_index()?;
    index
        .load_from_buffer(&graph_bytes)
        .map_err(|err| MejepaInferError::InvalidInput {
            field: "patch_similarity_index.load".to_string(),
            detail: format!("usearch load_from_buffer failed: {err}"),
        })?;
    if index.size() != metadata.row_count {
        return Err(MejepaInferError::DimMismatch {
            expected: metadata.row_count,
            actual: index.size(),
            context: "patch_similarity HNSW graph row count".to_string(),
        });
    }

    Ok(PatchSimilarityIndex {
        metadata,
        records,
        index,
    })
}

pub fn closest_exemplars(
    index: &PatchSimilarityIndex,
    mut query: PatchSimilarityQuery,
) -> Result<PatchSimilaritySearchOutcome, MejepaInferError> {
    if !query.has_neighborhood_coverage {
        return Ok(PatchSimilaritySearchOutcome {
            status: PatchSimilaritySearchStatus::NoNeighborhoodCoverage,
            reason_code: Some("NO_NEIGHBORHOOD_COVERAGE".to_string()),
            requested_k: query.k,
            returned: 0,
            exemplars: Vec::new(),
        });
    }
    normalize_l2_in_place("patch_similarity_query.vector", &mut query.vector)?;
    let request_k = query.k.min(index.records.len());
    let results = index
        .index
        .search(&query.vector, index.records.len())
        .map_err(|err| MejepaInferError::InvalidInput {
            field: "patch_similarity_index.search".to_string(),
            detail: format!("usearch search failed: {err}"),
        })?;

    let mut ranked = Vec::with_capacity(results.keys.len());
    for key in results.keys {
        let row_idx = key as usize;
        if let Some(record) = index.records.get(row_idx) {
            let cosine = exact_cosine(&query.vector, &record.vector)?;
            ranked.push((row_idx, cosine));
        }
    }
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    ranked.truncate(request_k);
    let exemplars = ranked
        .into_iter()
        .map(|(row_idx, score)| index.records[row_idx].to_match(score))
        .collect::<Vec<_>>();

    Ok(PatchSimilaritySearchOutcome {
        status: PatchSimilaritySearchStatus::Found,
        reason_code: None,
        requested_k: query.k,
        returned: exemplars.len(),
        exemplars,
    })
}

fn new_hnsw_index() -> Result<Index, MejepaInferError> {
    let options = IndexOptions {
        dimensions: PATCH_SIMILARITY_STRUCTURAL_DIM,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        connectivity: HNSW_M,
        expansion_add: HNSW_EF_CONSTRUCTION,
        expansion_search: HNSW_EF_SEARCH,
        ..Default::default()
    };
    Index::new(&options).map_err(|err| MejepaInferError::InvalidInput {
        field: "patch_similarity_index.new".to_string(),
        detail: format!("usearch index creation failed: {err}"),
    })
}

fn serialize_hnsw_graph(index: &Index) -> Result<Vec<u8>, MejepaInferError> {
    let len = index.serialized_length();
    if len == 0 {
        return Err(MejepaInferError::InvalidInput {
            field: "patch_similarity_index.serialized_length".to_string(),
            detail: "serialized HNSW graph length must be non-zero".to_string(),
        });
    }
    let mut graph_bytes = vec![0u8; len];
    index
        .save_to_buffer(&mut graph_bytes)
        .map_err(|err| MejepaInferError::InvalidInput {
            field: "patch_similarity_index.save".to_string(),
            detail: format!("usearch save_to_buffer failed: {err}"),
        })?;
    Ok(graph_bytes)
}

fn persist_metadata_cf(db: &DB, metadata_bytes: &[u8]) -> Result<bool, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_PATCH_SIMILARITY_META)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, PATCH_SIMILARITY_META_KEY, metadata_bytes, &opts)?;
    db.flush_cf(cf)?;
    let readback = db.get_cf(cf, PATCH_SIMILARITY_META_KEY)?.ok_or_else(|| {
        MejepaInferError::InvalidInput {
            field: "patch_similarity_meta_cf".to_string(),
            detail: "metadata CF read-after-write did not find active row".to_string(),
        }
    })?;
    Ok(readback.as_slice() == metadata_bytes)
}

fn validate_patch_vector(field: &str, vector: &[f32]) -> Result<(), MejepaInferError> {
    if vector.len() != PATCH_SIMILARITY_STRUCTURAL_DIM {
        return Err(MejepaInferError::DimMismatch {
            expected: PATCH_SIMILARITY_STRUCTURAL_DIM,
            actual: vector.len(),
            context: field.to_string(),
        });
    }
    let mut norm_sq = 0.0_f32;
    for (idx, value) in vector.iter().enumerate() {
        if !value.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: field.to_string(),
                detail: format!("{field}[{idx}] is {value}"),
            });
        }
        norm_sq += value * value;
    }
    if norm_sq <= f32::EPSILON {
        return Err(MejepaInferError::PatchSimilarityDegenerateQuery {
            detail: format!("{field} has zero L2 norm"),
        });
    }
    Ok(())
}

fn normalize_l2_in_place(field: &str, vector: &mut [f32]) -> Result<(), MejepaInferError> {
    validate_patch_vector(field, vector)?;
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm <= f32::EPSILON {
        return Err(MejepaInferError::PatchSimilarityDegenerateQuery {
            detail: format!("{field} has zero L2 norm"),
        });
    }
    for value in vector.iter_mut() {
        *value /= norm;
    }
    validate_normalized_vector(vector)
}

fn validate_normalized_vector(vector: &[f32]) -> Result<(), MejepaInferError> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if (norm - 1.0).abs() > NORMALIZED_EPSILON {
        return Err(MejepaInferError::InvalidInput {
            field: "patch_similarity_vector.norm".to_string(),
            detail: format!("vector must be L2-normalized; observed norm={norm}"),
        });
    }
    Ok(())
}

fn exact_cosine(lhs: &[f32], rhs: &[f32]) -> Result<f32, MejepaInferError> {
    if lhs.len() != rhs.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: lhs.len(),
            actual: rhs.len(),
            context: "patch_similarity exact cosine".to_string(),
        });
    }
    let dot = lhs.iter().zip(rhs.iter()).map(|(a, b)| a * b).sum::<f32>();
    Ok(dot.clamp(0.0, 1.0))
}

fn validate_sha256_hex(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "value must be 64 hex chars".to_string(),
        });
    }
    Ok(())
}

fn validate_bounded_text(
    field: &str,
    value: &str,
    max_bytes: usize,
) -> Result<(), MejepaInferError> {
    if value.is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "value must be non-empty".to_string(),
        });
    }
    if value.len() > max_bytes {
        return Err(MejepaInferError::DimMismatch {
            expected: max_bytes,
            actual: value.len(),
            context: field.to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "value must not contain control characters".to_string(),
        });
    }
    Ok(())
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), MejepaInferError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| MejepaInferError::io("create_dir_all", parent.to_path_buf(), err))?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("json")
    ));
    fs::write(&tmp, bytes).map_err(|err| MejepaInferError::io("write", tmp.clone(), err))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
            .map_err(|err| MejepaInferError::io("chmod", tmp.clone(), err))?;
    }
    fs::rename(&tmp, path)
        .map_err(|err| MejepaInferError::io("rename", path.to_path_buf(), err))?;
    let readback =
        fs::read(path).map_err(|err| MejepaInferError::io("read", path.to_path_buf(), err))?;
    if readback != bytes {
        return Err(MejepaInferError::InvalidInput {
            field: "patch_similarity_file_readback".to_string(),
            detail: format!("atomic write readback mismatch for {}", path.display()),
        });
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa_instruments::{ExceptionClass, PerTestOutcome, TestOutcome};

    fn snapshot_hash() -> String {
        sha256_hex(b"patch-similarity-test-corpus")
    }

    fn oracle() -> OracleVerdict {
        OracleVerdict {
            per_test: vec![PerTestOutcome {
                test_id: "tests/test_example.py::test_case".to_string(),
                outcome: TestOutcome::Fail,
                runtime_ms: 12,
            }],
            exception: None::<ExceptionClass>,
        }
    }

    fn vector(axis: usize) -> Vec<f32> {
        let mut out = vec![0.0_f32; PATCH_SIMILARITY_STRUCTURAL_DIM];
        out[axis] = 1.0;
        out
    }

    fn record(idx: u8, axis: usize, failure_mode_class: FailureModeClass) -> PatchExemplarRecord {
        PatchExemplarRecord {
            exemplar_prediction_id: [idx; 16],
            task_id: TaskId(format!("task-{idx}")),
            mutation_kind: MutationCategory::OffByOne,
            oracle_outcome: oracle(),
            failure_mode_class,
            witness_hash: WitnessHash([idx; 32]),
            evidence_path: format!("/mnt/d/contextgraph/fsv/task-{idx}.json"),
            diff_summary: format!("synthetic exemplar {idx}"),
            vector: vector(axis),
        }
    }

    #[test]
    fn build_load_and_search_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let records = vec![
            record(1, 0, FailureModeClass::OffByOne),
            record(2, 1, FailureModeClass::WrongAlgorithm),
        ];
        let report =
            build_patch_similarity_index(records, temp.path(), snapshot_hash(), None).unwrap();
        assert_eq!(report.metadata.row_count, 2);
        let index = load_patch_similarity_index(temp.path(), &snapshot_hash()).unwrap();
        let query = PatchSimilarityQuery::new(vector(0), 5, true).unwrap();
        let outcome = closest_exemplars(&index, query).unwrap();
        assert_eq!(outcome.status, PatchSimilaritySearchStatus::Found);
        assert_eq!(outcome.returned, 2);
        assert_eq!(
            outcome.exemplars[0].failure_mode_class,
            Some(FailureModeClass::OffByOne)
        );
        assert_eq!(outcome.exemplars[0].exemplar_prediction_id, Some([1; 16]));
    }

    #[test]
    fn stale_snapshot_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        build_patch_similarity_index(
            vec![record(1, 0, FailureModeClass::OffByOne)],
            temp.path(),
            snapshot_hash(),
            None,
        )
        .unwrap();
        let stale = sha256_hex(b"stale-corpus");
        let err = match load_patch_similarity_index(temp.path(), &stale) {
            Ok(_) => panic!("stale snapshot unexpectedly loaded"),
            Err(err) => err,
        };
        assert_eq!(err.code(), "MEJEPA_PATCH_SIMILARITY_STALE_INDEX");
    }

    #[test]
    fn zero_norm_query_fails_closed() {
        let err =
            PatchSimilarityQuery::new(vec![0.0_f32; PATCH_SIMILARITY_STRUCTURAL_DIM], 1, true)
                .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_PATCH_SIMILARITY_DEGENERATE_QUERY");
    }

    #[test]
    fn no_neighborhood_returns_empty_status() {
        let temp = tempfile::tempdir().unwrap();
        build_patch_similarity_index(
            vec![record(1, 0, FailureModeClass::OffByOne)],
            temp.path(),
            snapshot_hash(),
            None,
        )
        .unwrap();
        let index = load_patch_similarity_index(temp.path(), &snapshot_hash()).unwrap();
        let query = PatchSimilarityQuery::new(vector(0), 1, false).unwrap();
        let outcome = closest_exemplars(&index, query).unwrap();
        assert_eq!(
            outcome.status,
            PatchSimilaritySearchStatus::NoNeighborhoodCoverage
        );
        assert!(outcome.exemplars.is_empty());
    }
}
