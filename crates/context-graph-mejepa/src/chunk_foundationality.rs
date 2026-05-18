use std::collections::{BTreeMap, BTreeSet, VecDeque};

use context_graph_solver::{CsrMatrix, ForwardPushConfig, ForwardPushSolver, MatrixKind};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;

pub const CHUNK_FOUNDATIONALITY_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_FOUNDATIONALITY_LAMBDA_FISHER: f32 = 1.0;
pub const MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED: &str =
    "MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED";
pub const MEJEPA_FOUNDATIONALITY_SATURATED: &str = "MEJEPA_FOUNDATIONALITY_SATURATED";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChunkFoundationalityConfig {
    pub connected_required: bool,
    pub lambda_fisher: f32,
    pub pagerank: ForwardPushConfig,
}

impl Default for ChunkFoundationalityConfig {
    fn default() -> Self {
        Self {
            connected_required: true,
            lambda_fisher: DEFAULT_FOUNDATIONALITY_LAMBDA_FISHER,
            pagerank: ForwardPushConfig::default(),
        }
    }
}

impl ChunkFoundationalityConfig {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_nonnegative_finite("chunk_foundationality.lambda_fisher", self.lambda_fisher)?;
        ForwardPushSolver::new(self.pagerank).map_err(solver_error)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChunkDependencyEdge {
    pub schema_version: u32,
    pub from_chunk_id: String,
    pub to_chunk_id: String,
    pub edge_kind: String,
    pub weight: f64,
    pub evidence_ref: String,
}

impl ChunkDependencyEdge {
    pub fn new(
        from_chunk_id: impl Into<String>,
        to_chunk_id: impl Into<String>,
        edge_kind: impl Into<String>,
        weight: f64,
        evidence_ref: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: CHUNK_FOUNDATIONALITY_SCHEMA_VERSION,
            from_chunk_id: from_chunk_id.into(),
            to_chunk_id: to_chunk_id.into(),
            edge_kind: edge_kind.into(),
            weight,
            evidence_ref: evidence_ref.into(),
        }
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CHUNK_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "chunk_dependency_edge.schema_version",
                format!(
                    "expected {CHUNK_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_single_line(
            "chunk_dependency_edge.from_chunk_id",
            &self.from_chunk_id,
            512,
        )?;
        validate_single_line("chunk_dependency_edge.to_chunk_id", &self.to_chunk_id, 512)?;
        validate_single_line("chunk_dependency_edge.edge_kind", &self.edge_kind, 64)?;
        validate_single_line(
            "chunk_dependency_edge.evidence_ref",
            &self.evidence_ref,
            1024,
        )?;
        if !self.weight.is_finite() || self.weight <= 0.0 {
            return invalid(
                "chunk_dependency_edge.weight",
                "dependency edge weight must be finite and positive",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChunkFoundationalityScore {
    pub schema_version: u32,
    pub chunk_id: String,
    pub foundationality_score: f32,
    pub raw_pagerank: f64,
    pub rank: u32,
    pub upstream_count: u32,
    pub downstream_count: u32,
    pub dependency_graph_sha256: String,
    pub computed_at_unix_ms: i64,
    pub fisher_lambda: f32,
    pub fisher_multiplier: f32,
}

impl ChunkFoundationalityScore {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CHUNK_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "chunk_foundationality.schema_version",
                format!(
                    "expected {CHUNK_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_single_line("chunk_foundationality.chunk_id", &self.chunk_id, 512)?;
        validate_unit(
            "chunk_foundationality.foundationality_score",
            self.foundationality_score,
        )?;
        if !self.raw_pagerank.is_finite() || self.raw_pagerank < 0.0 {
            return invalid(
                "chunk_foundationality.raw_pagerank",
                "raw pagerank must be finite and non-negative",
            );
        }
        if self.rank == 0 {
            return invalid(
                "chunk_foundationality.rank",
                "rank is 1-based and must be non-zero",
            );
        }
        validate_sha256(
            "chunk_foundationality.dependency_graph_sha256",
            &self.dependency_graph_sha256,
        )?;
        if self.computed_at_unix_ms <= 0 {
            return invalid(
                "chunk_foundationality.computed_at_unix_ms",
                "computed_at_unix_ms must be positive",
            );
        }
        validate_nonnegative_finite("chunk_foundationality.fisher_lambda", self.fisher_lambda)?;
        validate_nonnegative_finite(
            "chunk_foundationality.fisher_multiplier",
            self.fisher_multiplier,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChunkFoundationalityReport {
    pub schema_version: u32,
    pub algorithm: String,
    pub dependency_graph_sha256: String,
    pub computed_at_unix_ms: i64,
    pub node_count: usize,
    pub edge_count: usize,
    pub solver_pushes: usize,
    pub solver_total_mass: f64,
    pub saturated_reason_code: Option<String>,
    pub saturated_chunk_ids: Vec<String>,
    pub scores: Vec<ChunkFoundationalityScore>,
}

impl ChunkFoundationalityReport {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CHUNK_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "chunk_foundationality_report.schema_version",
                format!(
                    "expected {CHUNK_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_single_line(
            "chunk_foundationality_report.algorithm",
            &self.algorithm,
            128,
        )?;
        validate_sha256(
            "chunk_foundationality_report.dependency_graph_sha256",
            &self.dependency_graph_sha256,
        )?;
        if self.computed_at_unix_ms <= 0 {
            return invalid(
                "chunk_foundationality_report.computed_at_unix_ms",
                "computed_at_unix_ms must be positive",
            );
        }
        if self.node_count == 0 || self.scores.is_empty() {
            return invalid(
                "chunk_foundationality_report.node_count",
                "report must contain at least one scored node",
            );
        }
        if self.node_count != self.scores.len() {
            return invalid(
                "chunk_foundationality_report.scores",
                "node_count must equal scores length",
            );
        }
        if !self.solver_total_mass.is_finite() || self.solver_total_mass <= 0.0 {
            return invalid(
                "chunk_foundationality_report.solver_total_mass",
                "solver total mass must be finite and positive",
            );
        }
        for score in &self.scores {
            score.validate()?;
            if score.dependency_graph_sha256 != self.dependency_graph_sha256 {
                return invalid(
                    "chunk_foundationality_report.score_graph_hash",
                    "score graph hash must match report graph hash",
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BedrockTouchedChunk {
    pub chunk_id: String,
    pub foundationality_score: f32,
    pub raw_pagerank: f64,
    pub rank: u32,
    pub upstream_count: u32,
    pub downstream_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BedrockConsistencyReport {
    pub threshold: f32,
    pub bedrock_touched: bool,
    pub predicted_foundationality_only: bool,
    pub missing_chunk_ids: Vec<String>,
    pub top_touched_chunks: Vec<BedrockTouchedChunk>,
}

pub fn compute_chunk_foundationality(
    edges: &[ChunkDependencyEdge],
    computed_at_unix_ms: i64,
    config: ChunkFoundationalityConfig,
) -> Result<ChunkFoundationalityReport, MejepaInferError> {
    config.validate()?;
    if computed_at_unix_ms <= 0 {
        return invalid(
            "chunk_foundationality.computed_at_unix_ms",
            "computed_at_unix_ms must be positive",
        );
    }
    if edges.is_empty() {
        return invalid(
            "chunk_dependency_graph.edges",
            "dependency graph requires at least one edge",
        );
    }

    let mut normalized_edges = edges.to_vec();
    for edge in &normalized_edges {
        edge.validate()?;
    }
    normalized_edges.sort_by(|left, right| {
        left.from_chunk_id
            .cmp(&right.from_chunk_id)
            .then_with(|| left.to_chunk_id.cmp(&right.to_chunk_id))
            .then_with(|| left.edge_kind.cmp(&right.edge_kind))
            .then_with(|| left.evidence_ref.cmp(&right.evidence_ref))
    });

    let nodes = graph_nodes(&normalized_edges);
    if config.connected_required && !is_weakly_connected(&nodes, &normalized_edges) {
        return invalid(
            "chunk_dependency_graph.connected",
            MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED,
        );
    }
    let graph_sha256 = dependency_graph_sha256(&nodes, &normalized_edges)?;
    let node_index = nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| (node.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let csr_edges = normalized_edges
        .iter()
        .map(|edge| {
            (
                node_index[&edge.from_chunk_id],
                node_index[&edge.to_chunk_id],
                edge.weight,
            )
        })
        .collect::<Vec<_>>();
    let graph = CsrMatrix::from_edges(
        nodes.len(),
        nodes.len(),
        MatrixKind::NonNegativeAdjacency,
        &csr_edges,
    )
    .map_err(solver_error)?;
    let seeds = (0..nodes.len())
        .map(|idx| (idx, 1.0_f64))
        .collect::<Vec<_>>();
    let solver = ForwardPushSolver::new(config.pagerank).map_err(solver_error)?;
    let solved = solver
        .solve_from_distribution(&graph, &seeds)
        .map_err(solver_error)?;
    let max_raw = solved.estimate.iter().copied().fold(0.0_f64, f64::max);
    if max_raw <= 0.0 || !max_raw.is_finite() {
        return invalid(
            "chunk_foundationality.raw_pagerank",
            "solver produced no positive finite PageRank mass",
        );
    }
    let (upstream, downstream) = dependency_counts(&nodes, &normalized_edges);
    let mut rows = nodes
        .iter()
        .enumerate()
        .map(|(idx, chunk_id)| {
            let raw = solved.estimate[idx];
            let score = (raw / max_raw).clamp(0.0, 1.0) as f32;
            Ok(ChunkFoundationalityScore {
                schema_version: CHUNK_FOUNDATIONALITY_SCHEMA_VERSION,
                chunk_id: chunk_id.clone(),
                foundationality_score: score,
                raw_pagerank: raw,
                rank: 0,
                upstream_count: *upstream.get(chunk_id).unwrap_or(&0),
                downstream_count: *downstream.get(chunk_id).unwrap_or(&0),
                dependency_graph_sha256: graph_sha256.clone(),
                computed_at_unix_ms,
                fisher_lambda: config.lambda_fisher,
                fisher_multiplier: foundationality_fisher_multiplier(score, config.lambda_fisher)?,
            })
        })
        .collect::<Result<Vec<_>, MejepaInferError>>()?;
    rows.sort_by(|left, right| {
        right
            .foundationality_score
            .partial_cmp(&left.foundationality_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    for (rank, row) in rows.iter_mut().enumerate() {
        row.rank = (rank + 1) as u32;
        row.validate()?;
    }
    let saturated_chunk_ids = rows
        .iter()
        .filter(|row| row.foundationality_score >= 0.999_999)
        .map(|row| row.chunk_id.clone())
        .collect::<Vec<_>>();
    let report = ChunkFoundationalityReport {
        schema_version: CHUNK_FOUNDATIONALITY_SCHEMA_VERSION,
        algorithm: "forward_push_uniform_seed_pagerank_dependency_edges".to_string(),
        dependency_graph_sha256: graph_sha256,
        computed_at_unix_ms,
        node_count: nodes.len(),
        edge_count: normalized_edges.len(),
        solver_pushes: solved.pushes,
        solver_total_mass: solved.total_mass,
        saturated_reason_code: (!saturated_chunk_ids.is_empty())
            .then(|| MEJEPA_FOUNDATIONALITY_SATURATED.to_string()),
        saturated_chunk_ids,
        scores: rows,
    };
    report.validate()?;
    Ok(report)
}

pub fn write_chunk_dependency_edge_sync_readback(
    db: &DB,
    edge: &ChunkDependencyEdge,
) -> Result<(), MejepaInferError> {
    edge.validate()?;
    write_value_sync_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH,
        &chunk_dependency_edge_key(edge)?,
        edge,
    )
}

pub fn read_all_chunk_dependency_edges(
    db: &DB,
) -> Result<Vec<ChunkDependencyEdge>, MejepaInferError> {
    read_all_values(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH,
    )
}

pub fn write_chunk_foundationality_score_sync_readback(
    db: &DB,
    score: &ChunkFoundationalityScore,
) -> Result<(), MejepaInferError> {
    score.validate()?;
    write_value_sync_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY,
        &chunk_foundationality_score_key(&score.chunk_id)?,
        score,
    )
}

pub fn read_chunk_foundationality_score(
    db: &DB,
    chunk_id: &str,
) -> Result<Option<ChunkFoundationalityScore>, MejepaInferError> {
    validate_single_line("chunk_foundationality.chunk_id", chunk_id, 512)?;
    read_value(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY,
        &chunk_foundationality_score_key(chunk_id)?,
    )
}

pub fn read_all_chunk_foundationality_scores(
    db: &DB,
) -> Result<Vec<ChunkFoundationalityScore>, MejepaInferError> {
    read_all_values(db, context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY)
}

pub fn persist_chunk_foundationality_report_sync_readback(
    db: &DB,
    edges: &[ChunkDependencyEdge],
    report: &ChunkFoundationalityReport,
) -> Result<(), MejepaInferError> {
    report.validate()?;
    for edge in edges {
        write_chunk_dependency_edge_sync_readback(db, edge)?;
    }
    for score in &report.scores {
        write_chunk_foundationality_score_sync_readback(db, score)?;
    }
    Ok(())
}

pub fn bedrock_consistency_for_chunks(
    db: &DB,
    touched_chunk_ids: &[String],
    threshold: f32,
    top_k: usize,
) -> Result<BedrockConsistencyReport, MejepaInferError> {
    validate_unit("bedrock_consistency.threshold", threshold)?;
    if top_k == 0 {
        return invalid("bedrock_consistency.top_k", "top_k must be positive");
    }
    if touched_chunk_ids.is_empty() {
        return invalid(
            "bedrock_consistency.touched_chunk_ids",
            "at least one touched chunk is required",
        );
    }
    let mut seen = BTreeSet::new();
    let mut touched = Vec::new();
    let mut missing = Vec::new();
    for chunk_id in touched_chunk_ids {
        validate_single_line("bedrock_consistency.chunk_id", chunk_id, 512)?;
        if !seen.insert(chunk_id.clone()) {
            continue;
        }
        match read_chunk_foundationality_score(db, chunk_id)? {
            Some(score) => {
                score.validate()?;
                touched.push(BedrockTouchedChunk {
                    chunk_id: score.chunk_id,
                    foundationality_score: score.foundationality_score,
                    raw_pagerank: score.raw_pagerank,
                    rank: score.rank,
                    upstream_count: score.upstream_count,
                    downstream_count: score.downstream_count,
                });
            }
            None => missing.push(chunk_id.clone()),
        }
    }
    touched.sort_by(|left, right| {
        right
            .foundationality_score
            .partial_cmp(&left.foundationality_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.rank.cmp(&right.rank))
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    let bedrock_touched = touched
        .iter()
        .any(|chunk| chunk.foundationality_score >= threshold);
    touched.truncate(top_k);
    Ok(BedrockConsistencyReport {
        threshold,
        bedrock_touched,
        predicted_foundationality_only: !missing.is_empty(),
        missing_chunk_ids: missing,
        top_touched_chunks: touched,
    })
}

pub fn foundationality_fisher_multiplier(
    foundationality_score: f32,
    lambda_fisher: f32,
) -> Result<f32, MejepaInferError> {
    validate_unit("foundationality_fisher.score", foundationality_score)?;
    validate_nonnegative_finite("foundationality_fisher.lambda", lambda_fisher)?;
    let value = 1.0 + lambda_fisher * foundationality_score;
    validate_nonnegative_finite("foundationality_fisher.multiplier", value)?;
    Ok(value)
}

pub fn apply_foundationality_fisher_multiplier(
    base_fisher: f32,
    foundationality_score: f32,
    lambda_fisher: f32,
) -> Result<f32, MejepaInferError> {
    validate_nonnegative_finite("foundationality_fisher.base_fisher", base_fisher)?;
    let multiplier = foundationality_fisher_multiplier(foundationality_score, lambda_fisher)?;
    let value = base_fisher * multiplier;
    validate_nonnegative_finite("foundationality_fisher.weighted", value)?;
    Ok(value)
}

pub fn compression_aggressiveness_from_foundationality(
    foundationality_score: f32,
) -> Result<f32, MejepaInferError> {
    validate_unit(
        "foundationality_compression.foundationality_score",
        foundationality_score,
    )?;
    Ok((1.0 - 0.75 * foundationality_score).clamp(0.25, 1.0))
}

fn graph_nodes(edges: &[ChunkDependencyEdge]) -> Vec<String> {
    let mut nodes = BTreeSet::new();
    for edge in edges {
        nodes.insert(edge.from_chunk_id.clone());
        nodes.insert(edge.to_chunk_id.clone());
    }
    nodes.into_iter().collect()
}

fn is_weakly_connected(nodes: &[String], edges: &[ChunkDependencyEdge]) -> bool {
    if nodes.is_empty() {
        return false;
    }
    let mut adjacency = nodes
        .iter()
        .map(|node| (node.clone(), BTreeSet::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    for edge in edges {
        adjacency
            .entry(edge.from_chunk_id.clone())
            .or_default()
            .insert(edge.to_chunk_id.clone());
        adjacency
            .entry(edge.to_chunk_id.clone())
            .or_default()
            .insert(edge.from_chunk_id.clone());
    }
    let start = nodes[0].clone();
    let mut queue = VecDeque::from([start.clone()]);
    let mut seen = BTreeSet::from([start]);
    while let Some(node) = queue.pop_front() {
        if let Some(neighbors) = adjacency.get(&node) {
            for neighbor in neighbors {
                if seen.insert(neighbor.clone()) {
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }
    seen.len() == nodes.len()
}

fn dependency_counts(
    nodes: &[String],
    edges: &[ChunkDependencyEdge],
) -> (BTreeMap<String, u32>, BTreeMap<String, u32>) {
    let mut upstream = nodes
        .iter()
        .map(|node| (node.clone(), BTreeSet::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    let mut downstream = upstream.clone();
    for edge in edges {
        upstream
            .entry(edge.to_chunk_id.clone())
            .or_default()
            .insert(edge.from_chunk_id.clone());
        downstream
            .entry(edge.from_chunk_id.clone())
            .or_default()
            .insert(edge.to_chunk_id.clone());
    }
    (
        upstream
            .into_iter()
            .map(|(node, values)| (node, values.len() as u32))
            .collect(),
        downstream
            .into_iter()
            .map(|(node, values)| (node, values.len() as u32))
            .collect(),
    )
}

fn dependency_graph_sha256(
    nodes: &[String],
    edges: &[ChunkDependencyEdge],
) -> Result<String, MejepaInferError> {
    let bytes = serde_json::to_vec(&(nodes, edges))?;
    Ok(sha256_hex(&bytes))
}

fn chunk_dependency_edge_key(edge: &ChunkDependencyEdge) -> Result<Vec<u8>, MejepaInferError> {
    let bytes = serde_json::to_vec(edge)?;
    Ok(format!("edge:{}", sha256_hex(&bytes)).into_bytes())
}

fn chunk_foundationality_score_key(chunk_id: &str) -> Result<Vec<u8>, MejepaInferError> {
    validate_single_line("chunk_foundationality.chunk_id", chunk_id, 512)?;
    Ok(format!("score:{}", sha256_hex(chunk_id.as_bytes())).into_bytes())
}

fn write_value_sync_readback<T>(
    db: &DB,
    cf_name: &str,
    key: &[u8],
    value: &T,
) -> Result<(), MejepaInferError>
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let cf = cf(db, cf_name)?;
    let bytes = bincode::serialize(value)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    db.flush_cf(cf)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: cf_name.to_string(),
            detail: "sync write readback returned no row".to_string(),
        })?;
    if readback != bytes {
        return invalid(
            cf_name,
            "sync write readback bytes differ from encoded input",
        );
    }
    let decoded: T = bincode::deserialize(&readback)?;
    if decoded != *value {
        return invalid(
            cf_name,
            format!("sync write readback decoded value differs: {decoded:?}"),
        );
    }
    Ok(())
}

fn read_value<T>(db: &DB, cf_name: &str, key: &[u8]) -> Result<Option<T>, MejepaInferError>
where
    T: DeserializeOwned,
{
    let cf = cf(db, cf_name)?;
    db.get_cf(cf, key)?
        .map(|bytes| bincode::deserialize(&bytes).map_err(Into::into))
        .transpose()
}

fn read_all_values<T>(db: &DB, cf_name: &str) -> Result<Vec<T>, MejepaInferError>
where
    T: DeserializeOwned,
{
    let cf = cf(db, cf_name)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        rows.push(bincode::deserialize(&value)?);
    }
    Ok(rows)
}

fn validate_single_line(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
    if value.is_empty() || value.len() > max_len || value.chars().any(|c| c == '\n' || c == '\r') {
        return invalid(
            field,
            format!("must be non-empty, single-line, and <= {max_len} bytes"),
        );
    }
    Ok(())
}

fn validate_unit(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(field, format!("{value} outside [0, 1]"));
    }
    Ok(())
}

fn validate_nonnegative_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || value < 0.0 {
        return invalid(field, "must be finite and non-negative");
    }
    Ok(())
}

fn validate_sha256(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return invalid(field, "must be a 64-character sha256 hex digest");
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn solver_error(err: context_graph_solver::SolverError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "chunk_foundationality.solver".to_string(),
        detail: err.to_string(),
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

    #[test]
    fn pagerank_known_hierarchy_scores_core_first() {
        let report = compute_chunk_foundationality(
            &fixture_edges(),
            1,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        assert_eq!(report.scores[0].chunk_id, "core:contract");
        assert_eq!(report.scores[0].foundationality_score, 1.0);
        assert!(
            report.scores[0].upstream_count > report.scores.last().unwrap().upstream_count,
            "core must have more dependents than leaves"
        );
    }

    #[test]
    fn disconnected_graph_rejected_when_required() {
        let err = compute_chunk_foundationality(
            &[
                ChunkDependencyEdge::new("a", "b", "call", 1.0, "fixture"),
                ChunkDependencyEdge::new("c", "d", "call", 1.0, "fixture"),
            ],
            1,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains(MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn persistence_reopens_scores_and_edges() {
        let temp = tempfile::tempdir().unwrap();
        let db = crate::open_infer_rocksdb(temp.path()).unwrap();
        let edges = fixture_edges();
        let report =
            compute_chunk_foundationality(&edges, 1, ChunkFoundationalityConfig::default())
                .unwrap();
        persist_chunk_foundationality_report_sync_readback(db.as_ref(), &edges, &report).unwrap();
        drop(db);
        let reopened = crate::open_infer_rocksdb(temp.path()).unwrap();
        let reopened_edges = read_all_chunk_dependency_edges(reopened.as_ref()).unwrap();
        let reopened_scores = read_all_chunk_foundationality_scores(reopened.as_ref()).unwrap();
        assert_eq!(reopened_edges.len(), edges.len());
        assert_eq!(reopened_scores.len(), report.scores.len());
        assert_eq!(
            read_chunk_foundationality_score(reopened.as_ref(), "core:contract")
                .unwrap()
                .unwrap()
                .foundationality_score,
            1.0
        );
    }

    #[test]
    fn bedrock_report_flags_high_foundationality_touch() {
        let temp = tempfile::tempdir().unwrap();
        let db = crate::open_infer_rocksdb(temp.path()).unwrap();
        let edges = fixture_edges();
        let report =
            compute_chunk_foundationality(&edges, 1, ChunkFoundationalityConfig::default())
                .unwrap();
        persist_chunk_foundationality_report_sync_readback(db.as_ref(), &edges, &report).unwrap();
        let bedrock = bedrock_consistency_for_chunks(
            db.as_ref(),
            &["leaf:test_b".to_string(), "core:contract".to_string()],
            0.75,
            3,
        )
        .unwrap();
        assert!(bedrock.bedrock_touched);
        assert_eq!(bedrock.top_touched_chunks[0].chunk_id, "core:contract");
    }

    #[test]
    fn fisher_and_compression_weight_by_foundationality() {
        let base = apply_foundationality_fisher_multiplier(2.0, 0.0, 1.0).unwrap();
        let bedrock = apply_foundationality_fisher_multiplier(2.0, 1.0, 1.0).unwrap();
        assert_eq!(base, 2.0);
        assert_eq!(bedrock, 4.0);
        assert!(
            compression_aggressiveness_from_foundationality(1.0).unwrap()
                < compression_aggressiveness_from_foundationality(0.0).unwrap()
        );
    }

    fn fixture_edges() -> Vec<ChunkDependencyEdge> {
        vec![
            ChunkDependencyEdge::new("api:handler", "core:contract", "call", 1.0, "fixture"),
            ChunkDependencyEdge::new("api:handler", "core:types", "type", 0.7, "fixture"),
            ChunkDependencyEdge::new("worker:sync", "core:contract", "call", 1.0, "fixture"),
            ChunkDependencyEdge::new("worker:sync", "core:types", "type", 0.7, "fixture"),
            ChunkDependencyEdge::new("leaf:test_a", "api:handler", "test", 1.0, "fixture"),
            ChunkDependencyEdge::new("leaf:test_b", "worker:sync", "test", 1.0, "fixture"),
            ChunkDependencyEdge::new("core:types", "core:contract", "type", 0.5, "fixture"),
        ]
    }
}
