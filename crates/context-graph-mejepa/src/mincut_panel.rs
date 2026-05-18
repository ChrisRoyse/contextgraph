use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use bincode::Options as BincodeOptions;
use context_graph_mejepa_cf::{CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_MINCUT_REPORTS};
use context_graph_mincut::{stoer_wagner, CutPartition, MincutError, StoerWagnerConfig};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::failure_fingerprint::FailureShapeFingerprint;
use crate::types::{ChunkId, DdaSignals, PanelId};

pub const MINCUT_PANEL_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_MINCUT_ROWS: usize = 10_000;
pub const DEFAULT_MINCUT_FINGERPRINT_LIMIT: usize = 256;
pub const MAX_MINCUT_NODES: usize = 512;
pub const MAX_MINCUT_DIRECTIONS: u32 = 16;
pub const MEJEPA_MINCUT_DEGENERATE: &str = "MEJEPA_MINCUT_DEGENERATE";
pub const MEJEPA_MINCUT_DDA_EMBEDDER_IDS_IMPUTED: &str = "MEJEPA_MINCUT_DDA_EMBEDDER_IDS_IMPUTED";
pub const MEJEPA_MINCUT_PAIRWISE_MI_EPSILON_FLOOR: &str = "MEJEPA_MINCUT_PAIRWISE_MI_EPSILON_FLOOR";
const PAIRWISE_MI_CONNECTIVITY_EPSILON: f32 = 1.0e-6;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelSimilarityWeight {
    #[default]
    Mi,
    Cosine,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MincutAlgorithm {
    #[default]
    StoerWagner,
    SparsestCutApprox,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MincutOptions {
    #[serde(default)]
    pub algorithm: MincutAlgorithm,
    #[serde(default = "default_return_top_k_candidate_directions")]
    pub return_top_k_candidate_directions: u32,
}

impl Default for MincutOptions {
    fn default() -> Self {
        Self {
            algorithm: MincutAlgorithm::StoerWagner,
            return_top_k_candidate_directions: default_return_top_k_candidate_directions(),
        }
    }
}

impl MincutOptions {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.return_top_k_candidate_directions > MAX_MINCUT_DIRECTIONS {
            return invalid(
                "return_top_k_candidate_directions",
                format!(
                    "must be <= {MAX_MINCUT_DIRECTIONS}, got {}",
                    self.return_top_k_candidate_directions
                ),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum PanelGraphSource {
    /// Explicit graph source for FSV and operator-supplied panel snapshots.
    InlineWeightedGraph {
        #[serde(alias = "graphId")]
        graph_id: String,
        #[serde(alias = "nodeIds")]
        node_ids: Vec<String>,
        weights: Vec<Vec<f32>>,
    },
    /// Aggregate persisted DDA rows from CF_MEJEPA_DDA_SIGNALS.
    EmbedderSimilarity {
        #[serde(default, alias = "panelId")]
        panel_id: Option<PanelId>,
        #[serde(default)]
        weight: PanelSimilarityWeight,
        #[serde(default = "default_mincut_rows", alias = "maxRows")]
        max_rows: usize,
    },
    /// Read a persisted pairwise-MI matrix from CF_MEJEPA_PAIRWISE_MI.
    PairwiseMiMatrix {
        #[serde(default, alias = "corpusShardHash")]
        corpus_shard_hash: Option<String>,
        #[serde(default, alias = "createdAtUnixMs")]
        created_at_unix_ms: Option<i64>,
        #[serde(default = "default_mincut_rows", alias = "maxRows")]
        max_rows: usize,
    },
    /// Read a persisted TCT constellation row and compare centroids for one
    /// mutation-category identity across requested embedder modalities.
    ConstellationInternal {
        #[serde(default, alias = "versionId")]
        version_id: Option<String>,
        #[serde(alias = "identityId")]
        identity_id: String,
        modalities: Vec<String>,
    },
    /// Compare persisted failure-shape fingerprints by centroid co-similarity.
    FailureFingerprintGraph {
        #[serde(default, alias = "lookbackWindow")]
        lookback_window: u64,
        #[serde(default = "default_mincut_fingerprint_limit")]
        limit: usize,
    },
}

impl PanelGraphSource {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        match self {
            Self::InlineWeightedGraph {
                graph_id,
                node_ids,
                weights,
            } => {
                validate_text("graph_id", graph_id, 512)?;
                validate_node_ids(node_ids)?;
                validate_weight_rows(node_ids.len(), weights)?;
            }
            Self::EmbedderSimilarity { max_rows, .. } => {
                if *max_rows == 0 || *max_rows > 1_000_000 {
                    return invalid(
                        "max_rows",
                        format!("max_rows must be in [1, 1000000], got {max_rows}"),
                    );
                }
            }
            Self::PairwiseMiMatrix {
                corpus_shard_hash,
                created_at_unix_ms,
                max_rows,
            } => {
                if let Some(hash) = corpus_shard_hash {
                    validate_hex("corpus_shard_hash", hash, 64)?;
                }
                if created_at_unix_ms.is_some_and(|value| value <= 0) {
                    return invalid("created_at_unix_ms", "must be positive");
                }
                if *max_rows == 0 || *max_rows > 1_000_000 {
                    return invalid(
                        "max_rows",
                        format!("max_rows must be in [1, 1000000], got {max_rows}"),
                    );
                }
            }
            Self::ConstellationInternal {
                version_id,
                identity_id,
                modalities,
            } => {
                if let Some(version_id) = version_id {
                    validate_hex("version_id", version_id, 64)?;
                }
                validate_text("identity_id", identity_id, 256)?;
                validate_node_ids(modalities)?;
            }
            Self::FailureFingerprintGraph { limit, .. } => {
                if *limit < 2 || *limit > 10_000 {
                    return invalid("limit", format!("limit must be in [2, 10000], got {limit}"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MincutPartitionReport {
    pub left: Vec<String>,
    pub right: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MincutReport {
    pub schema_version: u32,
    pub report_id: String,
    pub created_at_unix_ms: i64,
    pub graph_source_hash: String,
    pub graph_id: String,
    pub graph_source_kind: String,
    pub algorithm: MincutAlgorithm,
    pub node_ids: Vec<String>,
    pub cut_value: f32,
    pub partition: MincutPartitionReport,
    pub recommended_addition_directions: Vec<Vec<f32>>,
    pub conductance: f32,
    pub edge_count: usize,
    pub source_row_count: usize,
    pub warnings: Vec<String>,
    pub source_of_truth_cf: String,
}

impl MincutReport {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != MINCUT_PANEL_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {MINCUT_PANEL_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_hex("report_id", &self.report_id, 64)?;
        if self.created_at_unix_ms <= 0 {
            return invalid("created_at_unix_ms", "must be positive");
        }
        validate_hex("graph_source_hash", &self.graph_source_hash, 64)?;
        validate_text("graph_id", &self.graph_id, 512)?;
        validate_text("graph_source_kind", &self.graph_source_kind, 128)?;
        validate_node_ids(&self.node_ids)?;
        if !self.cut_value.is_finite() || self.cut_value < 0.0 {
            return invalid(
                "cut_value",
                format!("must be finite and non-negative, got {}", self.cut_value),
            );
        }
        if !self.conductance.is_finite() || self.conductance < 0.0 {
            return invalid(
                "conductance",
                format!("must be finite and non-negative, got {}", self.conductance),
            );
        }
        let nodes = self.node_ids.iter().cloned().collect::<BTreeSet<_>>();
        let left = self.partition.left.iter().cloned().collect::<BTreeSet<_>>();
        let right = self
            .partition
            .right
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if left.is_empty() || right.is_empty() {
            return invalid(
                "partition",
                "left and right partition sides must both be non-empty",
            );
        }
        if !left.is_disjoint(&right) {
            return invalid("partition", "left and right partition sides overlap");
        }
        let union = left.union(&right).cloned().collect::<BTreeSet<_>>();
        if union != nodes {
            return invalid("partition", "partition must cover exactly node_ids");
        }
        for direction in &self.recommended_addition_directions {
            if direction.len() != self.node_ids.len() {
                return Err(MejepaInferError::DimMismatch {
                    expected: self.node_ids.len(),
                    actual: direction.len(),
                    context: "mincut addition direction length must match node count".to_string(),
                });
            }
            validate_unit_or_zero("recommended_addition_directions", direction)?;
        }
        if self.source_of_truth_cf != CF_MEJEPA_MINCUT_REPORTS {
            return invalid(
                "source_of_truth_cf",
                format!("expected {CF_MEJEPA_MINCUT_REPORTS}"),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ResolvedGraph {
    graph_id: String,
    graph_source_kind: String,
    node_ids: Vec<String>,
    weights: Vec<f64>,
    source_row_count: usize,
    warnings: Vec<String>,
}

pub fn open_mincut_rocksdb(path: impl AsRef<Path>) -> Result<Arc<DB>, MejepaInferError> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = context_graph_mejepa_cf::ALL_HYGIENE_REFERENCED_CFS
        .iter()
        .map(|cf| ColumnFamilyDescriptor::new(*cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(
        &opts,
        path.as_ref(),
        descriptors,
    )?))
}

pub fn mejepa_mincut_panel(
    db: Option<&DB>,
    graph_source: PanelGraphSource,
    options: MincutOptions,
    created_at_unix_ms: i64,
) -> Result<MincutReport, MejepaInferError> {
    if created_at_unix_ms <= 0 {
        return invalid("created_at_unix_ms", "must be positive");
    }
    graph_source.validate()?;
    options.validate()?;
    let graph_source_hash = sha256_hex(&serde_json::to_vec(&graph_source)?);
    let resolved = resolve_graph(db, &graph_source)?;
    let node_count = resolved.node_ids.len();
    if node_count > MAX_MINCUT_NODES {
        return invalid(
            "node_ids",
            format!("node count {node_count} exceeds max {MAX_MINCUT_NODES}"),
        );
    }
    let edge_count = count_positive_edges(&resolved.weights, node_count)?;
    let mut warnings = resolved.warnings;

    let cut = if all_off_diagonal_zero(&resolved.weights, node_count)? {
        warnings.push(MEJEPA_MINCUT_DEGENERATE.to_string());
        CutPartition {
            small_side: vec![0],
            large_side: (1..node_count).collect(),
            cut_weight: 0.0,
            phase: 0,
        }
    } else {
        match options.algorithm {
            MincutAlgorithm::StoerWagner => {
                stoer_wagner(&resolved.weights, node_count, StoerWagnerConfig::default())
                    .map_err(mincut_error)?
            }
            MincutAlgorithm::SparsestCutApprox => {
                let _connectivity_probe =
                    stoer_wagner(&resolved.weights, node_count, StoerWagnerConfig::default())
                        .map_err(mincut_error)?;
                sparsest_cut_exact_or_stoer(&resolved.weights, node_count)?
            }
        }
    };
    let conductance = conductance(&resolved.weights, node_count, &cut)?;
    let directions = addition_directions(
        &resolved.weights,
        node_count,
        &cut,
        options.return_top_k_candidate_directions,
    )?;
    let partition = MincutPartitionReport {
        left: cut
            .small_side
            .iter()
            .map(|idx| resolved.node_ids[*idx].clone())
            .collect(),
        right: cut
            .large_side
            .iter()
            .map(|idx| resolved.node_ids[*idx].clone())
            .collect(),
    };
    let report_id = {
        let mut hasher = Sha256::new();
        hasher.update(graph_source_hash.as_bytes());
        hasher.update(created_at_unix_ms.to_be_bytes());
        hasher.update(resolved.graph_id.as_bytes());
        hasher.update((cut.cut_weight.to_bits()).to_be_bytes());
        for node_id in &resolved.node_ids {
            hasher.update(node_id.as_bytes());
        }
        hex::encode(hasher.finalize())
    };
    let report = MincutReport {
        schema_version: MINCUT_PANEL_SCHEMA_VERSION,
        report_id,
        created_at_unix_ms,
        graph_source_hash,
        graph_id: resolved.graph_id,
        graph_source_kind: resolved.graph_source_kind,
        algorithm: options.algorithm,
        node_ids: resolved.node_ids,
        cut_value: cut.cut_weight as f32,
        partition,
        recommended_addition_directions: directions,
        conductance: conductance as f32,
        edge_count,
        source_row_count: resolved.source_row_count,
        warnings,
        source_of_truth_cf: CF_MEJEPA_MINCUT_REPORTS.to_string(),
    };
    report.validate()?;
    Ok(report)
}

pub fn write_mincut_report_sync_readback(
    db: &DB,
    report: &MincutReport,
) -> Result<(), MejepaInferError> {
    report.validate()?;
    let cf = cf(db, CF_MEJEPA_MINCUT_REPORTS)?;
    let value = bincode::serialize(report)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, report.report_id.as_bytes(), &value, &opts)?;
    db.flush_cf(cf)?;
    let readback = db.get_cf(cf, report.report_id.as_bytes())?.ok_or_else(|| {
        MejepaInferError::InvalidInput {
            field: "mincut_report".to_string(),
            detail: "read-after-write could not find persisted mincut report".to_string(),
        }
    })?;
    if readback != value {
        return invalid(
            "mincut_report",
            "read-after-write bytes differ from persisted mincut report row",
        );
    }
    let decoded: MincutReport = bincode::deserialize(&readback)?;
    decoded.validate()?;
    if decoded != *report {
        return invalid(
            "mincut_report",
            "read-after-write decoded report does not match input",
        );
    }
    Ok(())
}

pub fn read_mincut_report(
    db: &DB,
    report_id: &str,
) -> Result<Option<MincutReport>, MejepaInferError> {
    validate_hex("report_id", report_id, 64)?;
    let cf = cf(db, CF_MEJEPA_MINCUT_REPORTS)?;
    let Some(bytes) = db.get_cf(cf, report_id.as_bytes())? else {
        return Ok(None);
    };
    let report: MincutReport = bincode::deserialize(&bytes)?;
    report.validate()?;
    if report.report_id != report_id {
        return invalid("report_id", "mincut report payload id does not match key");
    }
    Ok(Some(report))
}

fn resolve_graph(
    db: Option<&DB>,
    source: &PanelGraphSource,
) -> Result<ResolvedGraph, MejepaInferError> {
    match source {
        PanelGraphSource::InlineWeightedGraph {
            graph_id,
            node_ids,
            weights,
        } => Ok(ResolvedGraph {
            graph_id: graph_id.clone(),
            graph_source_kind: "inline_weighted_graph".to_string(),
            node_ids: node_ids.clone(),
            weights: flatten_weights(weights)?,
            source_row_count: 1,
            warnings: Vec::new(),
        }),
        PanelGraphSource::EmbedderSimilarity {
            panel_id,
            weight,
            max_rows,
        } => resolve_dda_graph(
            db_required(db, "embedder_similarity")?,
            panel_id.as_ref(),
            *weight,
            *max_rows,
        ),
        PanelGraphSource::PairwiseMiMatrix {
            corpus_shard_hash,
            created_at_unix_ms,
            max_rows,
        } => resolve_pairwise_mi_graph(
            db_required(db, "pairwise_mi_matrix")?,
            corpus_shard_hash.as_deref(),
            *created_at_unix_ms,
            *max_rows,
        ),
        PanelGraphSource::ConstellationInternal {
            version_id,
            identity_id,
            modalities,
        } => resolve_constellation_graph(
            db_required(db, "constellation_internal")?,
            version_id.as_deref(),
            identity_id,
            modalities,
        ),
        PanelGraphSource::FailureFingerprintGraph {
            lookback_window,
            limit,
        } => resolve_fingerprint_graph(
            db_required(db, "failure_fingerprint_graph")?,
            *lookback_window,
            *limit,
        ),
    }
}

fn resolve_dda_graph(
    db: &DB,
    panel_id_filter: Option<&PanelId>,
    weight: PanelSimilarityWeight,
    max_rows: usize,
) -> Result<ResolvedGraph, MejepaInferError> {
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS)?;
    let mut rows_seen = 0usize;
    let mut n: Option<usize> = None;
    let mut sums: Vec<f64> = Vec::new();
    let mut counted = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        let (panel_id, _chunk_id): (PanelId, ChunkId) = bincode::deserialize(&key)?;
        if panel_id_filter.is_some_and(|want| want != &panel_id) {
            continue;
        }
        rows_seen += 1;
        if rows_seen > max_rows {
            break;
        }
        let signals: DdaSignals = serde_json::from_slice(&value)?;
        signals.validate()?;
        let row_n = signals.embedder_count();
        if row_n < 2 {
            continue;
        }
        let values = match weight {
            PanelSimilarityWeight::Mi => {
                if signals.pairwise_mi_upper.is_empty() {
                    continue;
                }
                signals.pairwise_mi_upper
            }
            PanelSimilarityWeight::Cosine => signals
                .pairwise_cosine_upper
                .iter()
                .map(|value| ((*value + 1.0) / 2.0).clamp(0.0, 1.0))
                .collect(),
        };
        let expected = row_n * (row_n - 1) / 2;
        if values.len() != expected {
            return Err(MejepaInferError::DimMismatch {
                expected,
                actual: values.len(),
                context: "DDA pairwise vector length while resolving mincut panel".to_string(),
            });
        }
        match n {
            Some(existing) if existing != row_n => {
                return Err(MejepaInferError::DimMismatch {
                    expected: existing,
                    actual: row_n,
                    context: "DDA rows for one mincut graph must share embedder count".to_string(),
                });
            }
            None => {
                n = Some(row_n);
                sums = vec![0.0; expected];
            }
            _ => {}
        }
        for (idx, value) in values.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return invalid(
                    "dda.pairwise",
                    format!("DDA pairwise value at {idx} must be finite and non-negative"),
                );
            }
            sums[idx] += *value as f64;
        }
        counted += 1;
    }
    let n = n.ok_or_else(|| MejepaInferError::InvalidInput {
        field: "CF_MEJEPA_DDA_SIGNALS".to_string(),
        detail: "no DDA rows with usable pairwise signals matched mincut request".to_string(),
    })?;
    if counted == 0 {
        return invalid(
            "CF_MEJEPA_DDA_SIGNALS",
            "no DDA rows with non-empty selected pairwise vector matched mincut request",
        );
    }
    for value in &mut sums {
        *value /= counted as f64;
    }
    Ok(ResolvedGraph {
        graph_id: panel_id_filter
            .map(|id| format!("embedder_similarity:{}", hex::encode(id.0)))
            .unwrap_or_else(|| "embedder_similarity:all_panels".to_string()),
        graph_source_kind: "embedder_similarity".to_string(),
        node_ids: (0..n).map(|idx| format!("embedder_{idx:02}")).collect(),
        weights: matrix_from_upper(n, &sums)?,
        source_row_count: counted,
        warnings: vec![MEJEPA_MINCUT_DDA_EMBEDDER_IDS_IMPUTED.to_string()],
    })
}

fn resolve_pairwise_mi_graph(
    db: &DB,
    corpus_shard_hash: Option<&str>,
    created_at_unix_ms: Option<i64>,
    max_rows: usize,
) -> Result<ResolvedGraph, MejepaInferError> {
    let matrix = crate::pairwise_mi::read_pairwise_mi_matrix(
        db,
        corpus_shard_hash,
        created_at_unix_ms,
        max_rows,
    )?;
    let mut weights_matrix = matrix.values;
    let mut epsilon_edges = 0usize;
    let mut row_idx = 0usize;
    while row_idx < weights_matrix.len() {
        let (head, tail) = weights_matrix.split_at_mut(row_idx + 1);
        let row = &mut head[row_idx];
        for (offset, col_row) in tail.iter_mut().enumerate() {
            let col_idx = row_idx + 1 + offset;
            if row[col_idx] == 0.0 {
                row[col_idx] = PAIRWISE_MI_CONNECTIVITY_EPSILON;
                col_row[row_idx] = PAIRWISE_MI_CONNECTIVITY_EPSILON;
                epsilon_edges += 1;
            }
        }
        row_idx += 1;
    }
    for (idx, row) in weights_matrix.iter_mut().enumerate() {
        row[idx] = 0.0;
    }
    let weights = flatten_weights(&weights_matrix)?;
    let warnings = if epsilon_edges == 0 {
        Vec::new()
    } else {
        vec![format!(
            "{MEJEPA_MINCUT_PAIRWISE_MI_EPSILON_FLOOR}:zero_edges={epsilon_edges}:epsilon={PAIRWISE_MI_CONNECTIVITY_EPSILON}"
        )]
    };
    Ok(ResolvedGraph {
        graph_id: format!(
            "pairwise_mi:{}:{}",
            matrix.corpus_shard_hash, matrix.created_at_unix_ms
        ),
        graph_source_kind: "pairwise_mi_matrix".to_string(),
        node_ids: matrix.slots,
        weights,
        source_row_count: matrix.source_row_count,
        warnings,
    })
}

fn resolve_constellation_graph(
    db: &DB,
    version_id: Option<&str>,
    identity_id: &str,
    modalities: &[String],
) -> Result<ResolvedGraph, MejepaInferError> {
    let constellation = load_constellation_from_cf(db, version_id)?;
    let version = constellation.version_id();
    let mutation = parse_tct_mutation(identity_id)?;
    let mut centroids = Vec::with_capacity(modalities.len());
    for modality in modalities {
        let embedder = context_graph_mejepa_tct::EmbedderId::from_str(modality).map_err(|err| {
            MejepaInferError::InvalidInput {
                field: "modalities".to_string(),
                detail: format!("unknown TCT embedder modality {modality}: {err}"),
            }
        })?;
        let by_embedder = constellation
            .per_category_centroids
            .get(&mutation)
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "identity_id".to_string(),
                detail: format!("no per-category constellation centroids for {identity_id}"),
            })?;
        let centroid =
            by_embedder
                .get(&embedder)
                .ok_or_else(|| MejepaInferError::InvalidInput {
                    field: "modalities".to_string(),
                    detail: format!("no centroid for modality {modality} under {identity_id}"),
                })?;
        centroids.push(centroid.values.clone());
    }
    Ok(ResolvedGraph {
        graph_id: format!("constellation:{}:{}", hex::encode(version), identity_id),
        graph_source_kind: "constellation_internal".to_string(),
        node_ids: modalities.to_vec(),
        weights: weights_from_centroids(&centroids)?,
        source_row_count: centroids.len(),
        warnings: Vec::new(),
    })
}

fn load_constellation_from_cf(
    db: &DB,
    version_id: Option<&str>,
) -> Result<context_graph_mejepa_tct::TctConstellation, MejepaInferError> {
    let requested_version = match version_id {
        Some(hex_id) => Some(parse_32_byte_hex("version_id", hex_id)?),
        None => None,
    };
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION)?;
    let mut selected: Option<context_graph_mejepa_tct::TctConstellation> = None;
    let mut selected_count = 0usize;

    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if key.len() != 64 {
            return Err(MejepaInferError::DimMismatch {
                expected: 64,
                actual: key.len(),
                context: "constellation key must be version_id || corpus_sha".to_string(),
            });
        }
        let mut key_version = [0u8; 32];
        key_version.copy_from_slice(&key[..32]);
        if requested_version.is_some_and(|want| want != key_version) {
            continue;
        }
        let constellation: context_graph_mejepa_tct::TctConstellation =
            tct_bincode_options().deserialize(&value)?;
        constellation.validate_integrity()?;
        if constellation.version_id() != key_version {
            return invalid(
                "CF_MEJEPA_CONSTELLATION",
                "constellation key version_id does not match payload",
            );
        }
        if requested_version.is_some() {
            selected_count += 1;
            if selected_count > 1 {
                return invalid(
                    "version_id",
                    format!(
                        "ambiguous constellation version {} has multiple corpus rows",
                        version_id.unwrap_or_default()
                    ),
                );
            }
            selected = Some(constellation);
        } else {
            let replace = selected
                .as_ref()
                .map(|current| {
                    constellation.frozen_at > current.frozen_at
                        || (constellation.frozen_at == current.frozen_at
                            && constellation.version_id() > current.version_id())
                })
                .unwrap_or(true);
            if replace {
                selected = Some(constellation);
            }
        }
    }

    selected.ok_or_else(|| MejepaInferError::InvalidInput {
        field: "CF_MEJEPA_CONSTELLATION".to_string(),
        detail: requested_version
            .map(|version| format!("no constellation row for version {}", hex::encode(version)))
            .unwrap_or_else(|| "no persisted TCT constellations".to_string()),
    })
}

fn resolve_fingerprint_graph(
    db: &DB,
    lookback_window: u64,
    limit: usize,
) -> Result<ResolvedGraph, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_FAILURE_FINGERPRINTS)?;
    let mut fingerprints = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::End) {
        let (_key, value) = item?;
        let fingerprint: FailureShapeFingerprint =
            match bincode::deserialize::<FailureShapeFingerprint>(&value) {
                Ok(value) => value,
                Err(_) => serde_json::from_slice(&value)?,
            };
        fingerprint.validate()?;
        fingerprints.push(fingerprint);
        if fingerprints.len() == limit {
            break;
        }
    }
    if fingerprints.len() < 2 {
        return invalid(
            "CF_MEJEPA_FAILURE_FINGERPRINTS",
            "at least two persisted fingerprints are required for a fingerprint graph",
        );
    }
    let node_ids = fingerprints
        .iter()
        .map(|fingerprint| fingerprint.fingerprint_id.hex())
        .collect::<Vec<_>>();
    let mut weights = vec![0.0; fingerprints.len() * fingerprints.len()];
    for i in 0..fingerprints.len() {
        for j in (i + 1)..fingerprints.len() {
            let sim = fingerprint_similarity(&fingerprints[i], &fingerprints[j])?;
            weights[i * fingerprints.len() + j] = sim as f64;
            weights[j * fingerprints.len() + i] = sim as f64;
        }
    }
    Ok(ResolvedGraph {
        graph_id: format!("failure_fingerprint_graph:lookback-{lookback_window}:limit-{limit}"),
        graph_source_kind: "failure_fingerprint_graph".to_string(),
        node_ids,
        weights,
        source_row_count: fingerprints.len(),
        warnings: Vec::new(),
    })
}

fn sparsest_cut_exact_or_stoer(
    weights: &[f64],
    n: usize,
) -> Result<CutPartition, MejepaInferError> {
    if n > 20 {
        return stoer_wagner(weights, n, StoerWagnerConfig::default()).map_err(mincut_error);
    }
    let full = (1u64 << n) - 1;
    let mut best: Option<(f64, f64, Vec<usize>, Vec<usize>)> = None;
    for mask in 1..full {
        if (mask & 1) == 0 {
            continue;
        }
        if mask == full {
            continue;
        }
        let small = (0..n)
            .filter(|idx| (mask & (1u64 << idx)) != 0)
            .collect::<Vec<_>>();
        let large = (0..n)
            .filter(|idx| (mask & (1u64 << idx)) == 0)
            .collect::<Vec<_>>();
        let cut_weight = cut_weight_for_sides(weights, n, &small, &large)?;
        let vol_small = volume_for_side(weights, n, &small)?;
        let vol_large = volume_for_side(weights, n, &large)?;
        let denom = vol_small.min(vol_large);
        if denom <= 0.0 {
            continue;
        }
        let conductance = cut_weight / denom;
        let replace = best
            .as_ref()
            .map(|(best_conductance, best_cut, _, _)| {
                conductance < *best_conductance
                    || (conductance == *best_conductance && cut_weight < *best_cut)
            })
            .unwrap_or(true);
        if replace {
            best = Some((conductance, cut_weight, small, large));
        }
    }
    let Some((_conductance, cut_weight, small_side, large_side)) = best else {
        return invalid("weights", "could not derive sparsest cut from graph");
    };
    if cut_weight == 0.0 {
        return Err(mincut_error(MincutError::GraphDisconnected { phase: 0 }));
    }
    Ok(CutPartition {
        small_side,
        large_side,
        cut_weight,
        phase: 0,
    })
}

fn addition_directions(
    weights: &[f64],
    n: usize,
    cut: &CutPartition,
    k: u32,
) -> Result<Vec<Vec<f32>>, MejepaInferError> {
    if k == 0 {
        return Ok(Vec::new());
    }
    let mut residual = vec![0.0f64; n * n];
    let left = cut.small_side.iter().copied().collect::<BTreeSet<_>>();
    let right = cut.large_side.iter().copied().collect::<BTreeSet<_>>();
    for i in &left {
        for j in &right {
            let w = weights[i * n + j];
            residual[i * n + j] = w;
            residual[j * n + i] = w;
        }
    }
    if all_zero_matrix(&residual) {
        return Ok(vec![partition_contrast_direction(n, cut)?]);
    }
    let mut out = Vec::new();
    let mut mat = residual;
    for seed in 0..k as usize {
        let Some((lambda, direction)) = dominant_eigen_direction(&mat, n, seed)? else {
            break;
        };
        let direction = orient_direction(direction);
        out.push(
            direction
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>(),
        );
        if out.len() == k as usize {
            break;
        }
        for i in 0..n {
            for j in 0..n {
                mat[i * n + j] -= lambda * direction[i] * direction[j];
            }
        }
    }
    if out.is_empty() {
        out.push(partition_contrast_direction(n, cut)?);
    }
    Ok(out)
}

fn dominant_eigen_direction(
    matrix: &[f64],
    n: usize,
    seed: usize,
) -> Result<Option<(f64, Vec<f64>)>, MejepaInferError> {
    let mut v = (0..n)
        .map(|idx| {
            let raw = (((idx + 1 + seed) * 1_103_515_245) % 997) as f64 / 997.0;
            raw - 0.5
        })
        .collect::<Vec<_>>();
    normalize_f64(&mut v)?;
    for _ in 0..64 {
        let mut next = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                next[i] += matrix[i * n + j] * v[j];
            }
        }
        let norm = l2_norm_f64(&next);
        if norm <= 1.0e-12 {
            return Ok(None);
        }
        for value in &mut next {
            *value /= norm;
        }
        v = next;
    }
    let mut mv = vec![0.0; n];
    for i in 0..n {
        for j in 0..n {
            mv[i] += matrix[i * n + j] * v[j];
        }
    }
    let lambda = v.iter().zip(&mv).map(|(a, b)| a * b).sum::<f64>();
    if !lambda.is_finite() || lambda.abs() <= 1.0e-12 {
        return Ok(None);
    }
    Ok(Some((lambda, v)))
}

fn partition_contrast_direction(
    n: usize,
    cut: &CutPartition,
) -> Result<Vec<f32>, MejepaInferError> {
    let mut direction = vec![0.0f32; n];
    let left_scale = 1.0 / (cut.small_side.len() as f32).sqrt();
    let right_scale = -1.0 / (cut.large_side.len() as f32).sqrt();
    for idx in &cut.small_side {
        direction[*idx] = left_scale;
    }
    for idx in &cut.large_side {
        direction[*idx] = right_scale;
    }
    let norm = direction
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt();
    if norm <= 0.0 || !norm.is_finite() {
        return invalid(
            "partition_contrast_direction",
            "cannot normalize partition direction",
        );
    }
    for value in &mut direction {
        *value /= norm as f32;
    }
    validate_unit_or_zero("partition_contrast_direction", &direction)?;
    Ok(direction)
}

fn conductance(weights: &[f64], n: usize, cut: &CutPartition) -> Result<f64, MejepaInferError> {
    let small = &cut.small_side;
    let large = &cut.large_side;
    let denom = volume_for_side(weights, n, small)?.min(volume_for_side(weights, n, large)?);
    if denom <= 0.0 {
        return Ok(0.0);
    }
    Ok(cut_weight_for_sides(weights, n, small, large)? / denom)
}

fn volume_for_side(weights: &[f64], n: usize, side: &[usize]) -> Result<f64, MejepaInferError> {
    let mut total = 0.0;
    for i in side {
        if *i >= n {
            return invalid("partition", "partition index out of bounds");
        }
        for j in 0..n {
            total += weights[i * n + j];
        }
    }
    Ok(total)
}

fn cut_weight_for_sides(
    weights: &[f64],
    n: usize,
    left: &[usize],
    right: &[usize],
) -> Result<f64, MejepaInferError> {
    let mut total = 0.0;
    for i in left {
        for j in right {
            if *i >= n || *j >= n {
                return invalid("partition", "partition index out of bounds");
            }
            total += weights[i * n + j];
        }
    }
    Ok(total)
}

fn matrix_from_upper(n: usize, upper: &[f64]) -> Result<Vec<f64>, MejepaInferError> {
    let expected = n * (n - 1) / 2;
    if upper.len() != expected {
        return Err(MejepaInferError::DimMismatch {
            expected,
            actual: upper.len(),
            context: "upper-triangle mincut matrix length".to_string(),
        });
    }
    let mut weights = vec![0.0; n * n];
    let mut idx = 0usize;
    for i in 0..n {
        for j in (i + 1)..n {
            let value = upper[idx];
            if !value.is_finite() || value < 0.0 {
                return invalid("weights", "edge weights must be finite and non-negative");
            }
            weights[i * n + j] = value;
            weights[j * n + i] = value;
            idx += 1;
        }
    }
    Ok(weights)
}

fn weights_from_centroids(centroids: &[Vec<f32>]) -> Result<Vec<f64>, MejepaInferError> {
    if centroids.len() < 2 {
        return invalid("centroids", "at least two centroids are required");
    }
    let n = centroids.len();
    let mut weights = vec![0.0; n * n];
    for i in 0..n {
        for j in (i + 1)..n {
            let cosine = cosine(&centroids[i], &centroids[j])?;
            let weight = ((cosine + 1.0) / 2.0).clamp(0.0, 1.0) as f64;
            weights[i * n + j] = weight;
            weights[j * n + i] = weight;
        }
    }
    Ok(weights)
}

fn fingerprint_similarity(
    left: &FailureShapeFingerprint,
    right: &FailureShapeFingerprint,
) -> Result<f32, MejepaInferError> {
    let mut total = 0.0f32;
    let mut count = 0usize;
    for (embedder, left_vec) in &left.centroid_by_embedder {
        if let Some(right_vec) = right.centroid_by_embedder.get(embedder) {
            total += ((cosine(left_vec, right_vec)? + 1.0) / 2.0).clamp(0.0, 1.0);
            count += 1;
        }
    }
    if count == 0 {
        return invalid(
            "fingerprint.centroid_by_embedder",
            "fingerprints have no shared embedder centroids",
        );
    }
    Ok(total / count as f32)
}

fn cosine(left: &[f32], right: &[f32]) -> Result<f32, MejepaInferError> {
    if left.len() != right.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: left.len(),
            actual: right.len(),
            context: "mincut centroid cosine".to_string(),
        });
    }
    let mut dot = 0.0f64;
    let mut left_norm = 0.0f64;
    let mut right_norm = 0.0f64;
    for (a, b) in left.iter().zip(right) {
        if !a.is_finite() || !b.is_finite() {
            return invalid("centroid", "centroid vector contains non-finite value");
        }
        dot += *a as f64 * *b as f64;
        left_norm += *a as f64 * *a as f64;
        right_norm += *b as f64 * *b as f64;
    }
    if left_norm <= 0.0 || right_norm <= 0.0 {
        return invalid("centroid", "centroid vector has zero norm");
    }
    Ok((dot / (left_norm.sqrt() * right_norm.sqrt())).clamp(-1.0, 1.0) as f32)
}

fn parse_tct_mutation(
    value: &str,
) -> Result<context_graph_mejepa_tct::MutationCategory, MejepaInferError> {
    let raw = value.strip_prefix("mutation_category:").unwrap_or(value);
    serde_json::from_value::<context_graph_mejepa_tct::MutationCategory>(serde_json::json!(raw))
        .map_err(|err| MejepaInferError::InvalidInput {
            field: "identity_id".to_string(),
            detail: format!("identity_id must be a mutation category, got {value}: {err}"),
        })
}

fn parse_32_byte_hex(field: &str, value: &str) -> Result<[u8; 32], MejepaInferError> {
    validate_hex(field, value, 64)?;
    let bytes = hex::decode(value).map_err(|err| MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: format!("invalid hex: {err}"),
    })?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn flatten_weights(weights: &[Vec<f32>]) -> Result<Vec<f64>, MejepaInferError> {
    let n = weights.len();
    let mut out = Vec::with_capacity(n * n);
    for row in weights {
        for value in row {
            out.push(*value as f64);
        }
    }
    validate_dense_weights(&out, n)?;
    Ok(out)
}

fn validate_dense_weights(weights: &[f64], n: usize) -> Result<(), MejepaInferError> {
    if n < 2 {
        return invalid("node_ids", "graph must contain at least two nodes");
    }
    if weights.len() != n * n {
        return Err(MejepaInferError::DimMismatch {
            expected: n * n,
            actual: weights.len(),
            context: "dense mincut weight matrix".to_string(),
        });
    }
    for i in 0..n {
        for j in 0..n {
            let value = weights[i * n + j];
            if !value.is_finite() || value < 0.0 {
                return invalid("weights", "weights must be finite and non-negative");
            }
            if i == j && value != 0.0 {
                return invalid("weights", "self-loop weights must be zero");
            }
            if (value - weights[j * n + i]).abs() > 1.0e-6 {
                return invalid("weights", "weight matrix must be symmetric");
            }
        }
    }
    Ok(())
}

fn validate_weight_rows(n: usize, weights: &[Vec<f32>]) -> Result<(), MejepaInferError> {
    if weights.len() != n {
        return Err(MejepaInferError::DimMismatch {
            expected: n,
            actual: weights.len(),
            context: "mincut weight row count".to_string(),
        });
    }
    for row in weights {
        if row.len() != n {
            return Err(MejepaInferError::DimMismatch {
                expected: n,
                actual: row.len(),
                context: "mincut weight column count".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_node_ids(node_ids: &[String]) -> Result<(), MejepaInferError> {
    if node_ids.len() < 2 {
        return invalid("node_ids", "at least two node ids are required");
    }
    if node_ids.len() > MAX_MINCUT_NODES {
        return invalid(
            "node_ids",
            format!(
                "node count {} exceeds max {MAX_MINCUT_NODES}",
                node_ids.len()
            ),
        );
    }
    let mut seen = BTreeSet::new();
    for node_id in node_ids {
        validate_text("node_id", node_id, 256)?;
        if !seen.insert(node_id.clone()) {
            return invalid("node_ids", format!("duplicate node id {node_id}"));
        }
    }
    Ok(())
}

fn validate_text(
    field: &str,
    value: impl AsRef<str>,
    max_len: usize,
) -> Result<(), MejepaInferError> {
    let value = value.as_ref();
    if value.trim().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if value.len() > max_len {
        return invalid(
            field,
            format!("length {} exceeds max {max_len}", value.len()),
        );
    }
    if value.as_bytes().contains(&0) {
        return invalid(field, "must not contain NUL bytes");
    }
    Ok(())
}

fn validate_hex(field: &str, value: &str, len: usize) -> Result<(), MejepaInferError> {
    if value.len() != len || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return invalid(field, format!("must be {len} hex chars"));
    }
    Ok(())
}

fn validate_unit_or_zero(field: &str, vector: &[f32]) -> Result<(), MejepaInferError> {
    let mut norm = 0.0f64;
    for value in vector {
        if !value.is_finite() {
            return invalid(field, "direction contains non-finite value");
        }
        norm += *value as f64 * *value as f64;
    }
    if norm == 0.0 {
        return Ok(());
    }
    if (norm.sqrt() - 1.0).abs() > 1.0e-3 {
        return invalid(
            field,
            format!("direction must have unit norm, got {}", norm.sqrt()),
        );
    }
    Ok(())
}

fn count_positive_edges(weights: &[f64], n: usize) -> Result<usize, MejepaInferError> {
    validate_dense_weights(weights, n)?;
    let mut count = 0usize;
    for i in 0..n {
        for j in (i + 1)..n {
            if weights[i * n + j] > 0.0 {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn all_off_diagonal_zero(weights: &[f64], n: usize) -> Result<bool, MejepaInferError> {
    validate_dense_weights(weights, n)?;
    for i in 0..n {
        for j in 0..n {
            if i != j && weights[i * n + j] != 0.0 {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn all_zero_matrix(matrix: &[f64]) -> bool {
    matrix.iter().all(|value| value.abs() <= 1.0e-12)
}

fn l2_norm_f64(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum::<f64>().sqrt()
}

fn normalize_f64(values: &mut [f64]) -> Result<(), MejepaInferError> {
    let norm = l2_norm_f64(values);
    if !norm.is_finite() || norm <= 0.0 {
        return invalid("direction", "cannot normalize zero/non-finite vector");
    }
    for value in values {
        *value /= norm;
    }
    Ok(())
}

fn orient_direction(mut values: Vec<f64>) -> Vec<f64> {
    if let Some(first) = values.iter().find(|value| value.abs() > 1.0e-12) {
        if *first < 0.0 {
            for value in &mut values {
                *value = -*value;
            }
        }
    }
    values
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn default_mincut_rows() -> usize {
    DEFAULT_MINCUT_ROWS
}

fn default_mincut_fingerprint_limit() -> usize {
    DEFAULT_MINCUT_FINGERPRINT_LIMIT
}

fn default_return_top_k_candidate_directions() -> u32 {
    1
}

fn tct_bincode_options() -> impl BincodeOptions {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
}

fn db_required<'a>(db: Option<&'a DB>, source: &str) -> Result<&'a DB, MejepaInferError> {
    db.ok_or_else(|| MejepaInferError::InvalidInput {
        field: "db".to_string(),
        detail: format!("{source} mincut graph source requires a RocksDB handle"),
    })
}

fn mincut_error(err: MincutError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "weights".to_string(),
        detail: format!("{}: {err}", err.code()),
    }
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mincut::weights_from_edges;

    #[test]
    fn inline_stoer_wagner_paper_graph_reports_cut_four() {
        let weights = weights_from_edges(
            8,
            [
                (0, 1, 2.0),
                (0, 4, 3.0),
                (1, 2, 3.0),
                (1, 4, 2.0),
                (1, 5, 2.0),
                (2, 3, 4.0),
                (2, 6, 2.0),
                (3, 6, 2.0),
                (3, 7, 2.0),
                (4, 5, 3.0),
                (5, 6, 1.0),
                (6, 7, 3.0),
            ],
        )
        .unwrap();
        let report = mejepa_mincut_panel(
            None,
            PanelGraphSource::InlineWeightedGraph {
                graph_id: "paper-fig-1".to_string(),
                node_ids: (0..8).map(|idx| format!("v{idx}")).collect(),
                weights: weights
                    .chunks(8)
                    .map(|row| row.iter().map(|value| *value as f32).collect())
                    .collect(),
            },
            MincutOptions::default(),
            1,
        )
        .unwrap();
        assert_eq!(report.cut_value, 4.0);
        assert_eq!(report.recommended_addition_directions.len(), 1);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn all_zero_graph_returns_degenerate_warning_not_connected_error() {
        let report = mejepa_mincut_panel(
            None,
            PanelGraphSource::InlineWeightedGraph {
                graph_id: "zero".to_string(),
                node_ids: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                weights: vec![
                    vec![0.0, 0.0, 0.0],
                    vec![0.0, 0.0, 0.0],
                    vec![0.0, 0.0, 0.0],
                ],
            },
            MincutOptions::default(),
            1,
        )
        .unwrap();
        assert_eq!(report.cut_value, 0.0);
        assert!(report
            .warnings
            .contains(&MEJEPA_MINCUT_DEGENERATE.to_string()));
    }

    #[test]
    fn disconnected_nonzero_graph_fails_closed() {
        let weights = weights_from_edges(4, [(0, 1, 1.0), (2, 3, 1.0)]).unwrap();
        let err = mejepa_mincut_panel(
            None,
            PanelGraphSource::InlineWeightedGraph {
                graph_id: "disconnected".to_string(),
                node_ids: (0..4).map(|idx| format!("v{idx}")).collect(),
                weights: weights
                    .chunks(4)
                    .map(|row| row.iter().map(|value| *value as f32).collect())
                    .collect(),
            },
            MincutOptions::default(),
            1,
        )
        .unwrap_err();
        assert!(err.to_string().contains("MINCUT_GRAPH_DISCONNECTED"));
    }
}
