use std::collections::{BTreeMap, BTreeSet, VecDeque};

use context_graph_solver::{CsrMatrix, ForwardPushConfig, ForwardPushSolver, MatrixKind};
use rocksdb::{IteratorMode, WriteOptions, DB};
use ruff_python_ast::{
    Arguments, Comprehension, ElifElseClause, ExceptHandler, Expr, ExprContext, ModModule,
    Parameters, Stmt,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;

pub const CHUNK_FOUNDATIONALITY_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_FOUNDATIONALITY_LAMBDA_FISHER: f32 = 1.0;
const MAX_PYTHON_DEPENDENCY_SOURCE_BYTES: usize = 5_000_000;
pub const MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED: &str =
    "MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED";
pub const MEJEPA_FOUNDATIONALITY_SATURATED: &str = "MEJEPA_FOUNDATIONALITY_SATURATED";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum LibraryId {
    #[default]
    PythonSweBenchLite,
    NonPythonFixtures,
    ShakespeareCanon,
    SantaTrainingVideo,
    CustomerServiceTranscripts,
    Custom(String),
}

impl LibraryId {
    pub fn slug(&self) -> String {
        match self {
            Self::PythonSweBenchLite => "python-swe-bench-lite".to_string(),
            Self::NonPythonFixtures => "non-python-fixtures".to_string(),
            Self::ShakespeareCanon => "shakespeare-canon".to_string(),
            Self::SantaTrainingVideo => "santa-training-video".to_string(),
            Self::CustomerServiceTranscripts => "customer-service-transcripts".to_string(),
            Self::Custom(value) => format!("custom:{value}"),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::PythonSweBenchLite => "Python SWE-bench Lite".to_string(),
            Self::NonPythonFixtures => "Non-Python Fixtures".to_string(),
            Self::ShakespeareCanon => "Shakespeare Canon".to_string(),
            Self::SantaTrainingVideo => "Santa Training Video".to_string(),
            Self::CustomerServiceTranscripts => "Customer-Service Transcripts".to_string(),
            Self::Custom(value) => value.clone(),
        }
    }

    pub fn parse_slug(value: &str) -> Result<Self, MejepaInferError> {
        validate_single_line("library_id", value, 128)?;
        Ok(match value {
            "python-swe-bench-lite" | "Python-SWE-bench-Lite" => Self::PythonSweBenchLite,
            "non-python-fixtures" | "Non-Python-Fixtures" => Self::NonPythonFixtures,
            "shakespeare-canon" | "Shakespeare-Canon" => Self::ShakespeareCanon,
            "santa-training-video" | "Santa-Training-Video" => Self::SantaTrainingVideo,
            "customer-service-transcripts" | "Customer-Service-Transcripts" => {
                Self::CustomerServiceTranscripts
            }
            other if other.starts_with("custom:") => {
                let custom = other.trim_start_matches("custom:");
                validate_single_line("library_id.custom", custom, 96)?;
                Self::Custom(custom.to_string())
            }
            other => Self::Custom(other.to_string()),
        })
    }

    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_single_line(field, &self.slug(), 128)?;
        if let Self::Custom(value) = self {
            validate_single_line(field, value, 96)?;
        }
        Ok(())
    }
}

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
    #[serde(default)]
    pub from_library_id: LibraryId,
    pub from_chunk_id: String,
    #[serde(default)]
    pub to_library_id: LibraryId,
    pub to_chunk_id: String,
    pub edge_kind: String,
    pub weight: f64,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PythonChunkSource {
    pub chunk_id: String,
    pub module: String,
    pub path: String,
    pub source: String,
}

impl PythonChunkSource {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_single_line("python_chunk_source.chunk_id", &self.chunk_id, 512)?;
        validate_single_line("python_chunk_source.module", &self.module, 256)?;
        validate_single_line("python_chunk_source.path", &self.path, 512)?;
        if self.source.is_empty() || self.source.len() > MAX_PYTHON_DEPENDENCY_SOURCE_BYTES {
            return invalid(
                "python_chunk_source.source",
                format!(
                    "source must be non-empty and <= {MAX_PYTHON_DEPENDENCY_SOURCE_BYTES} bytes"
                ),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PythonChunkDependencyExtraction {
    pub schema_version: u32,
    pub analyzer: String,
    pub source_count: usize,
    pub edge_count: usize,
    pub edge_kind_counts: BTreeMap<String, usize>,
    pub edges: Vec<ChunkDependencyEdge>,
}

impl PythonChunkDependencyExtraction {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CHUNK_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "python_chunk_dependency_extraction.schema_version",
                format!(
                    "expected {CHUNK_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_single_line(
            "python_chunk_dependency_extraction.analyzer",
            &self.analyzer,
            128,
        )?;
        if self.source_count == 0 {
            return invalid(
                "python_chunk_dependency_extraction.source_count",
                "at least one Python source is required",
            );
        }
        if self.edge_count != self.edges.len() {
            return invalid(
                "python_chunk_dependency_extraction.edge_count",
                "edge_count must equal edges length",
            );
        }
        for edge in &self.edges {
            edge.validate()?;
        }
        Ok(())
    }
}

impl ChunkDependencyEdge {
    pub fn new(
        from_chunk_id: impl Into<String>,
        to_chunk_id: impl Into<String>,
        edge_kind: impl Into<String>,
        weight: f64,
        evidence_ref: impl Into<String>,
    ) -> Self {
        Self::new_with_libraries(
            LibraryId::default(),
            from_chunk_id,
            LibraryId::default(),
            to_chunk_id,
            edge_kind,
            weight,
            evidence_ref,
        )
    }

    pub fn new_with_libraries(
        from_library_id: LibraryId,
        from_chunk_id: impl Into<String>,
        to_library_id: LibraryId,
        to_chunk_id: impl Into<String>,
        edge_kind: impl Into<String>,
        weight: f64,
        evidence_ref: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: CHUNK_FOUNDATIONALITY_SCHEMA_VERSION,
            from_library_id,
            from_chunk_id: from_chunk_id.into(),
            to_library_id,
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
        self.from_library_id
            .validate("chunk_dependency_edge.from_library_id")?;
        validate_single_line(
            "chunk_dependency_edge.from_chunk_id",
            &self.from_chunk_id,
            512,
        )?;
        self.to_library_id
            .validate("chunk_dependency_edge.to_library_id")?;
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

pub fn extract_python_chunk_dependency_edges(
    sources: &[PythonChunkSource],
) -> Result<PythonChunkDependencyExtraction, MejepaInferError> {
    if sources.is_empty() {
        return invalid(
            "python_chunk_dependency_sources",
            "at least one Python source is required",
        );
    }
    let mut index = PythonSymbolIndex::default();
    for source in sources {
        source.validate()?;
        index.add_module(source);
        let module = parse_python_module(source)?;
        collect_python_definitions(source, &module.body, None, &mut index);
    }

    let mut builder = PythonDependencyEdgeBuilder::default();
    for source in sources {
        let module = parse_python_module(source)?;
        collect_python_dependency_edges(
            source,
            &module.body,
            &source.chunk_id,
            None,
            &index,
            &mut builder,
        );
    }

    let edges = builder.into_edges();
    let mut edge_kind_counts = BTreeMap::new();
    for edge in &edges {
        *edge_kind_counts.entry(edge.edge_kind.clone()).or_insert(0) += 1;
    }
    let extraction = PythonChunkDependencyExtraction {
        schema_version: CHUNK_FOUNDATIONALITY_SCHEMA_VERSION,
        analyzer: "python_ruff_chunk_dependency_v1".to_string(),
        source_count: sources.len(),
        edge_count: edges.len(),
        edge_kind_counts,
        edges,
    };
    extraction.validate()?;
    Ok(extraction)
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
    let cf = cf(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH,
    )?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        rows.push(decode_chunk_dependency_edge(&value)?);
    }
    Ok(rows)
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

pub fn bedrock_consistency_for_patch_diff(
    db: &DB,
    patch_diff: &str,
    threshold: f32,
    top_k: usize,
) -> Result<BedrockConsistencyReport, MejepaInferError> {
    validate_unit("bedrock_consistency.threshold", threshold)?;
    if top_k == 0 {
        return invalid("bedrock_consistency.top_k", "top_k must be positive");
    }
    if patch_diff.trim().is_empty() {
        return invalid(
            "bedrock_consistency.patch_diff",
            "patch diff must be non-empty",
        );
    }
    let touched_paths = extract_patch_touched_paths(patch_diff)?;
    if touched_paths.is_empty() {
        return invalid(
            "bedrock_consistency.patch_paths",
            "patch diff did not contain any touched file paths",
        );
    }
    let scores = read_all_chunk_foundationality_scores(db)?;
    let mut touched = Vec::new();
    let mut matched_paths = BTreeSet::new();
    for score in scores {
        score.validate()?;
        if let Some(path) = touched_paths
            .iter()
            .find(|path| chunk_id_matches_patch_path(&score.chunk_id, path))
        {
            matched_paths.insert(path.clone());
            touched.push(BedrockTouchedChunk {
                chunk_id: score.chunk_id,
                foundationality_score: score.foundationality_score,
                raw_pagerank: score.raw_pagerank,
                rank: score.rank,
                upstream_count: score.upstream_count,
                downstream_count: score.downstream_count,
            });
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
    let missing_chunk_ids = touched_paths
        .iter()
        .filter(|path| !matched_paths.contains(*path))
        .map(|path| format!("{path}::*"))
        .collect::<Vec<_>>();
    let bedrock_touched = touched
        .iter()
        .any(|chunk| chunk.foundationality_score >= threshold);
    touched.truncate(top_k);
    Ok(BedrockConsistencyReport {
        threshold,
        bedrock_touched,
        predicted_foundationality_only: !missing_chunk_ids.is_empty(),
        missing_chunk_ids,
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

#[derive(Default)]
struct PythonSymbolIndex {
    modules: BTreeMap<String, String>,
    qualified_symbols: BTreeMap<String, String>,
    simple_symbols: BTreeMap<String, BTreeSet<String>>,
    qualified_variables: BTreeMap<String, String>,
    simple_variables: BTreeMap<String, BTreeSet<String>>,
}

impl PythonSymbolIndex {
    fn add_module(&mut self, source: &PythonChunkSource) {
        self.modules
            .insert(source.module.clone(), source.chunk_id.clone());
    }

    fn add_symbol(&mut self, source: &PythonChunkSource, qualname: &str) {
        let chunk_id = python_symbol_chunk_id(source, qualname);
        self.qualified_symbols
            .insert(format!("{}.{}", source.module, qualname), chunk_id.clone());
        self.qualified_symbols
            .entry(qualname.to_string())
            .or_insert_with(|| chunk_id.clone());
        let simple = qualname.rsplit('.').next().unwrap_or(qualname);
        self.simple_symbols
            .entry(simple.to_string())
            .or_default()
            .insert(chunk_id);
    }

    fn add_variable(&mut self, source: &PythonChunkSource, name: &str) {
        let chunk_id = python_variable_chunk_id(source, name);
        self.qualified_variables
            .insert(format!("{}.{}", source.module, name), chunk_id.clone());
        self.qualified_variables
            .entry(name.to_string())
            .or_insert_with(|| chunk_id.clone());
        self.simple_variables
            .entry(name.to_string())
            .or_default()
            .insert(chunk_id);
    }

    fn resolve_module(&self, module: &str) -> Option<String> {
        if let Some(chunk_id) = self.modules.get(module) {
            return Some(chunk_id.clone());
        }
        let suffix = format!(".{module}");
        unique_suffix_match(&self.modules, &suffix)
    }

    fn resolve_symbol(&self, current_module: &str, target: &str) -> Option<String> {
        let target = target.trim();
        if target.is_empty() {
            return None;
        }
        if let Some(chunk_id) = self.qualified_symbols.get(target) {
            return Some(chunk_id.clone());
        }
        let local = format!("{current_module}.{target}");
        if let Some(chunk_id) = self.qualified_symbols.get(&local) {
            return Some(chunk_id.clone());
        }
        if target.contains('.') {
            let suffix = format!(".{target}");
            if let Some(chunk_id) = unique_suffix_match(&self.qualified_symbols, &suffix) {
                return Some(chunk_id);
            }
        }
        let simple = target.rsplit('.').next().unwrap_or(target);
        self.simple_symbols.get(simple).and_then(unique_set_value)
    }

    fn resolve_variable(&self, current_module: &str, target: &str) -> Option<String> {
        let target = target.trim();
        if target.is_empty() {
            return None;
        }
        if let Some(chunk_id) = self.qualified_variables.get(target) {
            return Some(chunk_id.clone());
        }
        let local = format!("{current_module}.{target}");
        if let Some(chunk_id) = self.qualified_variables.get(&local) {
            return Some(chunk_id.clone());
        }
        if target.contains('.') {
            let suffix = format!(".{target}");
            if let Some(chunk_id) = unique_suffix_match(&self.qualified_variables, &suffix) {
                return Some(chunk_id);
            }
        }
        let simple = target.rsplit('.').next().unwrap_or(target);
        self.simple_variables.get(simple).and_then(unique_set_value)
    }
}

#[derive(Default)]
struct PythonDependencyEdgeBuilder {
    edges: BTreeMap<(String, String, String), ChunkDependencyEdge>,
}

impl PythonDependencyEdgeBuilder {
    fn add(
        &mut self,
        source: &PythonChunkSource,
        from_chunk_id: &str,
        to_chunk_id: &str,
        edge_kind: &str,
        weight: f64,
        detail: &str,
    ) {
        if from_chunk_id.is_empty() || to_chunk_id.is_empty() {
            return;
        }
        let key = (
            from_chunk_id.to_string(),
            to_chunk_id.to_string(),
            edge_kind.to_string(),
        );
        if let Some(existing) = self.edges.get_mut(&key) {
            existing.weight += weight;
            return;
        }
        let evidence_ref = format!(
            "python:{}:{}:{}",
            sanitize_evidence_fragment(&source.path),
            edge_kind,
            sanitize_evidence_fragment(detail)
        );
        self.edges.insert(
            key,
            ChunkDependencyEdge::new(
                from_chunk_id.to_string(),
                to_chunk_id.to_string(),
                edge_kind.to_string(),
                weight,
                evidence_ref,
            ),
        );
    }

    fn into_edges(self) -> Vec<ChunkDependencyEdge> {
        self.edges.into_values().collect()
    }
}

fn parse_python_module(source: &PythonChunkSource) -> Result<ModModule, MejepaInferError> {
    ruff_python_parser::parse_module(&source.source)
        .map(|parsed| parsed.into_syntax())
        .map_err(|err| MejepaInferError::InvalidInput {
            field: "python_chunk_source.source".to_string(),
            detail: format!(
                "ruff-python-parser rejected {} (module {}): {err}",
                source.path, source.module
            ),
        })
}

fn collect_python_definitions(
    source: &PythonChunkSource,
    suite: &[Stmt],
    parent_qualname: Option<&str>,
    index: &mut PythonSymbolIndex,
) {
    for stmt in suite {
        match stmt {
            Stmt::FunctionDef(function) => {
                let qualname = nested_qualname(parent_qualname, function.name.as_str());
                index.add_symbol(source, &qualname);
                collect_python_definitions(source, &function.body, Some(&qualname), index);
            }
            Stmt::ClassDef(class_def) => {
                let qualname = nested_qualname(parent_qualname, class_def.name.as_str());
                index.add_symbol(source, &qualname);
                collect_python_definitions(source, &class_def.body, Some(&qualname), index);
            }
            Stmt::Assign(assign) if parent_qualname.is_none() => {
                let mut names = BTreeSet::new();
                for target in &assign.targets {
                    collect_python_target_names(target, &mut names);
                }
                for name in names {
                    index.add_variable(source, &name);
                }
            }
            Stmt::AnnAssign(ann_assign) if parent_qualname.is_none() => {
                let mut names = BTreeSet::new();
                collect_python_target_names(&ann_assign.target, &mut names);
                for name in names {
                    index.add_variable(source, &name);
                }
            }
            _ => {}
        }
    }
}

fn collect_python_dependency_edges(
    source: &PythonChunkSource,
    suite: &[Stmt],
    current_chunk_id: &str,
    current_qualname: Option<&str>,
    index: &PythonSymbolIndex,
    builder: &mut PythonDependencyEdgeBuilder,
) {
    for stmt in suite {
        collect_python_stmt_edges(
            source,
            stmt,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        );
    }
}

fn collect_python_stmt_edges(
    source: &PythonChunkSource,
    stmt: &Stmt,
    current_chunk_id: &str,
    current_qualname: Option<&str>,
    index: &PythonSymbolIndex,
    builder: &mut PythonDependencyEdgeBuilder,
) {
    match stmt {
        Stmt::FunctionDef(function) => {
            let qualname = nested_qualname(current_qualname, function.name.as_str());
            let function_chunk_id = python_symbol_chunk_id(source, &qualname);
            collect_python_parameters_type_edges(
                source,
                &function.parameters,
                &function_chunk_id,
                index,
                builder,
            );
            if let Some(returns) = &function.returns {
                collect_python_annotation_edges(
                    source,
                    returns,
                    &function_chunk_id,
                    index,
                    builder,
                );
            }
            for decorator in &function.decorator_list {
                collect_python_expr_edges(
                    source,
                    &decorator.expression,
                    &function_chunk_id,
                    Some(&qualname),
                    index,
                    builder,
                );
            }
            collect_python_dependency_edges(
                source,
                &function.body,
                &function_chunk_id,
                Some(&qualname),
                index,
                builder,
            );
        }
        Stmt::ClassDef(class_def) => {
            let qualname = nested_qualname(current_qualname, class_def.name.as_str());
            let class_chunk_id = python_symbol_chunk_id(source, &qualname);
            for decorator in &class_def.decorator_list {
                collect_python_expr_edges(
                    source,
                    &decorator.expression,
                    &class_chunk_id,
                    Some(&qualname),
                    index,
                    builder,
                );
            }
            if let Some(arguments) = &class_def.arguments {
                for base in &arguments.args {
                    if let Some(target) = python_expr_reference_name(base) {
                        if let Some(target_chunk_id) = index.resolve_symbol(&source.module, &target)
                        {
                            builder.add(
                                source,
                                &class_chunk_id,
                                &target_chunk_id,
                                "inheritance",
                                1.2,
                                &format!("{qualname}->{target}"),
                            );
                        }
                    }
                    collect_python_annotation_edges(source, base, &class_chunk_id, index, builder);
                }
                collect_python_arguments_edges(
                    source,
                    arguments,
                    &class_chunk_id,
                    Some(&qualname),
                    index,
                    builder,
                );
            }
            collect_python_dependency_edges(
                source,
                &class_def.body,
                &class_chunk_id,
                Some(&qualname),
                index,
                builder,
            );
        }
        Stmt::Import(stmt_import) => {
            for alias in &stmt_import.names {
                let imported = alias.name.as_str();
                if let Some(target_chunk_id) = index.resolve_module(imported) {
                    builder.add(
                        source,
                        current_chunk_id,
                        &target_chunk_id,
                        "import",
                        0.8,
                        imported,
                    );
                }
            }
        }
        Stmt::ImportFrom(stmt_import_from) => {
            let imported_module = resolve_python_import_from_module(
                &source.module,
                stmt_import_from
                    .module
                    .as_ref()
                    .map(|module| module.as_str()),
                stmt_import_from.level,
            );
            if let Some(module_name) = imported_module.as_deref() {
                if let Some(target_chunk_id) = index.resolve_module(module_name) {
                    builder.add(
                        source,
                        current_chunk_id,
                        &target_chunk_id,
                        "import",
                        0.8,
                        module_name,
                    );
                }
                for alias in &stmt_import_from.names {
                    let alias_name = alias.name.as_str();
                    if alias_name == "*" {
                        continue;
                    }
                    let target = format!("{module_name}.{alias_name}");
                    if let Some(target_chunk_id) = index.resolve_symbol(&source.module, &target) {
                        builder.add(
                            source,
                            current_chunk_id,
                            &target_chunk_id,
                            "import",
                            0.8,
                            &target,
                        );
                    }
                }
            }
        }
        Stmt::Return(stmt_return) => {
            if let Some(value) = &stmt_return.value {
                collect_python_expr_edges(
                    source,
                    value,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Stmt::Delete(stmt_delete) => {
            for target in &stmt_delete.targets {
                collect_python_expr_edges(
                    source,
                    target,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Stmt::TypeAlias(type_alias) => {
            collect_python_annotation_edges(
                source,
                &type_alias.value,
                current_chunk_id,
                index,
                builder,
            );
        }
        Stmt::Assign(assign) => {
            collect_python_expr_edges(
                source,
                &assign.value,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            for target in &assign.targets {
                collect_python_expr_edges(
                    source,
                    target,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Stmt::AugAssign(aug_assign) => {
            collect_python_expr_edges(
                source,
                &aug_assign.target,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &aug_assign.value,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Stmt::AnnAssign(ann_assign) => {
            collect_python_annotation_edges(
                source,
                &ann_assign.annotation,
                current_chunk_id,
                index,
                builder,
            );
            if let Some(value) = &ann_assign.value {
                collect_python_expr_edges(
                    source,
                    value,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
            collect_python_expr_edges(
                source,
                &ann_assign.target,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Stmt::For(stmt_for) => {
            collect_python_expr_edges(
                source,
                &stmt_for.iter,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &stmt_for.target,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_dependency_edges(
                source,
                &stmt_for.body,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_dependency_edges(
                source,
                &stmt_for.orelse,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Stmt::While(stmt_while) => {
            collect_python_expr_edges(
                source,
                &stmt_while.test,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_dependency_edges(
                source,
                &stmt_while.body,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_dependency_edges(
                source,
                &stmt_while.orelse,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Stmt::If(stmt_if) => {
            collect_python_expr_edges(
                source,
                &stmt_if.test,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_dependency_edges(
                source,
                &stmt_if.body,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            for clause in &stmt_if.elif_else_clauses {
                collect_python_elif_else_edges(
                    source,
                    clause,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Stmt::With(stmt_with) => {
            for item in &stmt_with.items {
                collect_python_expr_edges(
                    source,
                    &item.context_expr,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
                if let Some(optional_vars) = &item.optional_vars {
                    collect_python_expr_edges(
                        source,
                        optional_vars,
                        current_chunk_id,
                        current_qualname,
                        index,
                        builder,
                    );
                }
            }
            collect_python_dependency_edges(
                source,
                &stmt_with.body,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Stmt::Match(stmt_match) => {
            collect_python_expr_edges(
                source,
                &stmt_match.subject,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            for case in &stmt_match.cases {
                if let Some(guard) = &case.guard {
                    collect_python_expr_edges(
                        source,
                        guard,
                        current_chunk_id,
                        current_qualname,
                        index,
                        builder,
                    );
                }
                collect_python_dependency_edges(
                    source,
                    &case.body,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Stmt::Raise(stmt_raise) => {
            if let Some(exc) = &stmt_raise.exc {
                collect_python_expr_edges(
                    source,
                    exc,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
            if let Some(cause) = &stmt_raise.cause {
                collect_python_expr_edges(
                    source,
                    cause,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Stmt::Try(stmt_try) => {
            collect_python_dependency_edges(
                source,
                &stmt_try.body,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            for handler in &stmt_try.handlers {
                let ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(type_) = &handler.type_ {
                    collect_python_expr_edges(
                        source,
                        type_,
                        current_chunk_id,
                        current_qualname,
                        index,
                        builder,
                    );
                    collect_python_annotation_edges(
                        source,
                        type_,
                        current_chunk_id,
                        index,
                        builder,
                    );
                }
                collect_python_dependency_edges(
                    source,
                    &handler.body,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
            collect_python_dependency_edges(
                source,
                &stmt_try.orelse,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_dependency_edges(
                source,
                &stmt_try.finalbody,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Stmt::Assert(stmt_assert) => {
            collect_python_expr_edges(
                source,
                &stmt_assert.test,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            if let Some(msg) = &stmt_assert.msg {
                collect_python_expr_edges(
                    source,
                    msg,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Stmt::Expr(stmt_expr) => collect_python_expr_edges(
            source,
            &stmt_expr.value,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        ),
        Stmt::Global(_)
        | Stmt::Nonlocal(_)
        | Stmt::Pass(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::IpyEscapeCommand(_) => {}
    }
}

fn collect_python_expr_edges(
    source: &PythonChunkSource,
    expr: &Expr,
    current_chunk_id: &str,
    current_qualname: Option<&str>,
    index: &PythonSymbolIndex,
    builder: &mut PythonDependencyEdgeBuilder,
) {
    match expr {
        Expr::BoolOp(bool_op) => {
            for value in &bool_op.values {
                collect_python_expr_edges(
                    source,
                    value,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Expr::Named(named) => {
            collect_python_expr_edges(
                source,
                &named.value,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &named.target,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::BinOp(bin_op) => {
            collect_python_expr_edges(
                source,
                &bin_op.left,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &bin_op.right,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::UnaryOp(unary_op) => collect_python_expr_edges(
            source,
            &unary_op.operand,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        ),
        Expr::Lambda(lambda) => {
            if let Some(parameters) = &lambda.parameters {
                collect_python_parameters_type_edges(
                    source,
                    parameters,
                    current_chunk_id,
                    index,
                    builder,
                );
            }
            collect_python_expr_edges(
                source,
                &lambda.body,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::If(expr_if) => {
            collect_python_expr_edges(
                source,
                &expr_if.test,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &expr_if.body,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &expr_if.orelse,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::Dict(dict) => {
            for item in &dict.items {
                if let Some(key) = &item.key {
                    collect_python_expr_edges(
                        source,
                        key,
                        current_chunk_id,
                        current_qualname,
                        index,
                        builder,
                    );
                }
                collect_python_expr_edges(
                    source,
                    &item.value,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Expr::Set(set) => {
            for elt in &set.elts {
                collect_python_expr_edges(
                    source,
                    elt,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Expr::ListComp(comp) => {
            collect_python_comprehension_edges(
                source,
                &comp.generators,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &comp.elt,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::SetComp(comp) => {
            collect_python_comprehension_edges(
                source,
                &comp.generators,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &comp.elt,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::DictComp(comp) => {
            collect_python_comprehension_edges(
                source,
                &comp.generators,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &comp.key,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &comp.value,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::Generator(generator) => {
            collect_python_comprehension_edges(
                source,
                &generator.generators,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &generator.elt,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::Await(await_expr) => collect_python_expr_edges(
            source,
            &await_expr.value,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        ),
        Expr::Yield(yield_expr) => {
            if let Some(value) = &yield_expr.value {
                collect_python_expr_edges(
                    source,
                    value,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Expr::YieldFrom(yield_from) => collect_python_expr_edges(
            source,
            &yield_from.value,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        ),
        Expr::Compare(compare) => {
            collect_python_expr_edges(
                source,
                &compare.left,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            for comparator in &compare.comparators {
                collect_python_expr_edges(
                    source,
                    comparator,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Expr::Call(call) => {
            if let Some(target) = python_expr_reference_name(&call.func) {
                if let Some(target_chunk_id) = index.resolve_symbol(&source.module, &target) {
                    builder.add(
                        source,
                        current_chunk_id,
                        &target_chunk_id,
                        "call",
                        1.0,
                        &format!("{}->{target}", current_qualname.unwrap_or("<module>")),
                    );
                    if python_context_is_test(source, current_qualname) {
                        builder.add(
                            source,
                            current_chunk_id,
                            &target_chunk_id,
                            "test_verifies",
                            1.1,
                            &format!("{}->{target}", current_qualname.unwrap_or("<module>")),
                        );
                    }
                }
            }
            collect_python_expr_edges(
                source,
                &call.func,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_arguments_edges(
                source,
                &call.arguments,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::Attribute(attribute) => {
            if matches!(attribute.ctx, ExprContext::Load) {
                let target = python_expr_reference_name(expr)
                    .unwrap_or_else(|| attribute.attr.as_str().to_string());
                if let Some(target_chunk_id) = index.resolve_variable(&source.module, &target) {
                    builder.add(
                        source,
                        current_chunk_id,
                        &target_chunk_id,
                        "data_flow",
                        0.6,
                        &format!("read:{target}"),
                    );
                }
            }
            collect_python_expr_edges(
                source,
                &attribute.value,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::Subscript(subscript) => {
            collect_python_expr_edges(
                source,
                &subscript.value,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
            collect_python_expr_edges(
                source,
                &subscript.slice,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
        Expr::Starred(starred) => collect_python_expr_edges(
            source,
            &starred.value,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        ),
        Expr::Name(name) => {
            if matches!(name.ctx, ExprContext::Load) {
                let target = name.id.as_str();
                if let Some(target_chunk_id) = index.resolve_variable(&source.module, target) {
                    builder.add(
                        source,
                        current_chunk_id,
                        &target_chunk_id,
                        "data_flow",
                        0.6,
                        &format!("read:{target}"),
                    );
                }
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_python_expr_edges(
                    source,
                    elt,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_python_expr_edges(
                    source,
                    elt,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Expr::Slice(slice) => {
            if let Some(lower) = &slice.lower {
                collect_python_expr_edges(
                    source,
                    lower,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
            if let Some(upper) = &slice.upper {
                collect_python_expr_edges(
                    source,
                    upper,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
            if let Some(step) = &slice.step {
                collect_python_expr_edges(
                    source,
                    step,
                    current_chunk_id,
                    current_qualname,
                    index,
                    builder,
                );
            }
        }
        Expr::FString(_)
        | Expr::TString(_)
        | Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_)
        | Expr::IpyEscapeCommand(_) => {}
    }
}

fn collect_python_arguments_edges(
    source: &PythonChunkSource,
    arguments: &Arguments,
    current_chunk_id: &str,
    current_qualname: Option<&str>,
    index: &PythonSymbolIndex,
    builder: &mut PythonDependencyEdgeBuilder,
) {
    for arg in &arguments.args {
        collect_python_expr_edges(
            source,
            arg,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        );
    }
    for keyword in &arguments.keywords {
        collect_python_expr_edges(
            source,
            &keyword.value,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        );
    }
}

fn collect_python_comprehension_edges(
    source: &PythonChunkSource,
    comprehensions: &[Comprehension],
    current_chunk_id: &str,
    current_qualname: Option<&str>,
    index: &PythonSymbolIndex,
    builder: &mut PythonDependencyEdgeBuilder,
) {
    for comprehension in comprehensions {
        collect_python_expr_edges(
            source,
            &comprehension.iter,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        );
        collect_python_expr_edges(
            source,
            &comprehension.target,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        );
        for if_expr in &comprehension.ifs {
            collect_python_expr_edges(
                source,
                if_expr,
                current_chunk_id,
                current_qualname,
                index,
                builder,
            );
        }
    }
}

fn collect_python_elif_else_edges(
    source: &PythonChunkSource,
    clause: &ElifElseClause,
    current_chunk_id: &str,
    current_qualname: Option<&str>,
    index: &PythonSymbolIndex,
    builder: &mut PythonDependencyEdgeBuilder,
) {
    if let Some(test) = &clause.test {
        collect_python_expr_edges(
            source,
            test,
            current_chunk_id,
            current_qualname,
            index,
            builder,
        );
    }
    collect_python_dependency_edges(
        source,
        &clause.body,
        current_chunk_id,
        current_qualname,
        index,
        builder,
    );
}

fn collect_python_parameters_type_edges(
    source: &PythonChunkSource,
    parameters: &Parameters,
    current_chunk_id: &str,
    index: &PythonSymbolIndex,
    builder: &mut PythonDependencyEdgeBuilder,
) {
    for parameter in parameters.iter() {
        if let Some(annotation) = parameter.annotation() {
            collect_python_annotation_edges(source, annotation, current_chunk_id, index, builder);
        }
    }
}

fn collect_python_annotation_edges(
    source: &PythonChunkSource,
    annotation: &Expr,
    current_chunk_id: &str,
    index: &PythonSymbolIndex,
    builder: &mut PythonDependencyEdgeBuilder,
) {
    let mut labels = BTreeSet::new();
    collect_python_annotation_labels(annotation, &mut labels);
    for label in labels {
        if let Some(target_chunk_id) = index.resolve_symbol(&source.module, &label) {
            builder.add(
                source,
                current_chunk_id,
                &target_chunk_id,
                "type",
                0.7,
                &format!("type:{label}"),
            );
        }
    }
}

fn collect_python_annotation_labels(expr: &Expr, labels: &mut BTreeSet<String>) {
    match expr {
        Expr::Name(name) => {
            labels.insert(name.id.as_str().to_string());
        }
        Expr::Attribute(attribute) => {
            if let Some(label) = python_expr_reference_name(expr) {
                labels.insert(label);
            }
            collect_python_annotation_labels(&attribute.value, labels);
        }
        Expr::Subscript(subscript) => {
            collect_python_annotation_labels(&subscript.value, labels);
            collect_python_annotation_labels(&subscript.slice, labels);
        }
        Expr::BinOp(bin_op) => {
            collect_python_annotation_labels(&bin_op.left, labels);
            collect_python_annotation_labels(&bin_op.right, labels);
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_python_annotation_labels(elt, labels);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_python_annotation_labels(elt, labels);
            }
        }
        _ => {}
    }
}

fn collect_python_target_names(expr: &Expr, names: &mut BTreeSet<String>) {
    match expr {
        Expr::Name(name) => {
            if matches!(name.ctx, ExprContext::Store | ExprContext::Load) {
                names.insert(name.id.as_str().to_string());
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_python_target_names(elt, names);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_python_target_names(elt, names);
            }
        }
        Expr::Starred(starred) => collect_python_target_names(&starred.value, names),
        _ => {}
    }
}

fn python_expr_reference_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str().to_string()),
        Expr::Attribute(attribute) => python_expr_reference_name(&attribute.value)
            .map(|prefix| format!("{prefix}.{}", attribute.attr.as_str()))
            .or_else(|| Some(attribute.attr.as_str().to_string())),
        _ => None,
    }
}

fn python_context_is_test(source: &PythonChunkSource, current_qualname: Option<&str>) -> bool {
    let file = source.path.rsplit('/').next().unwrap_or(&source.path);
    file.starts_with("test_")
        || source.path.contains("/tests/")
        || source.path.contains("/test/")
        || current_qualname
            .and_then(|qualname| qualname.rsplit('.').next())
            .is_some_and(|name| name.starts_with("test_") || name.ends_with("_test"))
}

fn resolve_python_import_from_module(
    current_module: &str,
    module: Option<&str>,
    level: u32,
) -> Option<String> {
    if level == 0 {
        return module.map(str::to_string);
    }
    let mut parts = current_module.split('.').collect::<Vec<_>>();
    if !parts.is_empty() {
        parts.pop();
    }
    for _ in 1..level {
        parts.pop()?;
    }
    if let Some(module) = module {
        if !module.is_empty() {
            parts.extend(module.split('.').filter(|part| !part.is_empty()));
        }
    }
    (!parts.is_empty()).then(|| parts.join("."))
}

fn nested_qualname(parent_qualname: Option<&str>, name: &str) -> String {
    parent_qualname
        .map(|parent| format!("{parent}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn python_symbol_chunk_id(source: &PythonChunkSource, qualname: &str) -> String {
    format!("{}::{qualname}", source.chunk_id)
}

fn python_variable_chunk_id(source: &PythonChunkSource, name: &str) -> String {
    format!("{}::var:{name}", source.chunk_id)
}

fn unique_set_value(values: &BTreeSet<String>) -> Option<String> {
    let mut iter = values.iter();
    let first = iter.next()?;
    iter.next().is_none().then(|| first.clone())
}

fn unique_suffix_match(values: &BTreeMap<String, String>, suffix: &str) -> Option<String> {
    let mut matches = values
        .iter()
        .filter(|(key, _)| key.ends_with(suffix))
        .map(|(_, value)| value);
    let first = matches.next()?;
    matches.next().is_none().then(|| first.clone())
}

fn sanitize_evidence_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' => ' ',
            _ => ch,
        })
        .collect::<String>()
        .chars()
        .take(256)
        .collect()
}

fn extract_patch_touched_paths(patch_diff: &str) -> Result<BTreeSet<String>, MejepaInferError> {
    let mut paths = BTreeSet::new();
    for line in patch_diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let parts = rest.split_whitespace().collect::<Vec<_>>();
            for part in parts.iter().take(2) {
                if let Some(path) = normalize_patch_path(part) {
                    paths.insert(path);
                }
            }
            continue;
        }
        for prefix in ["+++ ", "--- ", "rename from ", "rename to "] {
            if let Some(path) = line.strip_prefix(prefix).and_then(normalize_patch_path) {
                paths.insert(path);
            }
        }
    }
    for path in &paths {
        validate_single_line("bedrock_consistency.patch_path", path, 512)?;
    }
    Ok(paths)
}

fn normalize_patch_path(raw: &str) -> Option<String> {
    let mut path = raw.trim();
    if path.is_empty() || path == "/dev/null" {
        return None;
    }
    path = path.strip_prefix("a/").unwrap_or(path);
    path = path.strip_prefix("b/").unwrap_or(path);
    path = path.strip_prefix("./").unwrap_or(path);
    if path.is_empty() || path == "/dev/null" {
        return None;
    }
    Some(path.replace('\\', "/"))
}

fn chunk_id_matches_patch_path(chunk_id: &str, path: &str) -> bool {
    let chunk_id = chunk_id.replace('\\', "/");
    let path = path.replace('\\', "/");
    chunk_id == path || chunk_id.starts_with(&format!("{path}::"))
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacyChunkDependencyEdgeV1 {
    schema_version: u32,
    from_chunk_id: String,
    to_chunk_id: String,
    edge_kind: String,
    weight: f64,
    evidence_ref: String,
}

fn decode_chunk_dependency_edge(bytes: &[u8]) -> Result<ChunkDependencyEdge, MejepaInferError> {
    match bincode::deserialize::<ChunkDependencyEdge>(bytes) {
        Ok(edge) => {
            edge.validate()?;
            Ok(edge)
        }
        Err(current_err) => {
            let legacy = bincode::deserialize::<LegacyChunkDependencyEdgeV1>(bytes)
                .map_err(|legacy_err| MejepaInferError::InvalidInput {
                    field: context_graph_mejepa_cf::CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH.to_string(),
                    detail: format!(
                        "failed to decode chunk dependency edge as current ({current_err}) or legacy v1 ({legacy_err})"
                    ),
                })?;
            let edge = ChunkDependencyEdge::new(
                legacy.from_chunk_id,
                legacy.to_chunk_id,
                legacy.edge_kind,
                legacy.weight,
                legacy.evidence_ref,
            );
            if legacy.schema_version != CHUNK_FOUNDATIONALITY_SCHEMA_VERSION {
                return invalid(
                    "chunk_dependency_edge.schema_version",
                    format!(
                        "expected {CHUNK_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                        legacy.schema_version
                    ),
                );
            }
            edge.validate()?;
            Ok(edge)
        }
    }
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

    #[test]
    fn python_extractor_emits_source_derived_edge_kinds() {
        let extraction = extract_python_chunk_dependency_edges(&python_sources()).unwrap();
        for kind in [
            "call",
            "data_flow",
            "import",
            "inheritance",
            "test_verifies",
            "type",
        ] {
            assert!(
                extraction.edge_kind_counts.get(kind).copied().unwrap_or(0) > 0,
                "missing edge kind {kind}: {:?}",
                extraction.edge_kind_counts
            );
        }
        let report = compute_chunk_foundationality(
            &extraction.edges,
            1,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        assert!(report
            .scores
            .iter()
            .any(|score| score.chunk_id.starts_with("pkg/core.py::")));
    }

    #[test]
    fn patch_diff_bedrock_report_matches_touched_python_paths() {
        let temp = tempfile::tempdir().unwrap();
        let db = crate::open_infer_rocksdb(temp.path()).unwrap();
        let extraction = extract_python_chunk_dependency_edges(&python_sources()).unwrap();
        let report = compute_chunk_foundationality(
            &extraction.edges,
            1,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        persist_chunk_foundationality_report_sync_readback(db.as_ref(), &extraction.edges, &report)
            .unwrap();
        let patch = r#"diff --git a/pkg/core.py b/pkg/core.py
--- a/pkg/core.py
+++ b/pkg/core.py
@@ -1,2 +1,2 @@
-BASE_LIMIT = 10
+BASE_LIMIT = 11
"#;
        let bedrock = bedrock_consistency_for_patch_diff(db.as_ref(), patch, 0.75, 5).unwrap();
        assert!(bedrock.bedrock_touched);
        assert!(bedrock
            .top_touched_chunks
            .iter()
            .any(|chunk| chunk.chunk_id.starts_with("pkg/core.py::")));
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

    fn python_sources() -> Vec<PythonChunkSource> {
        vec![
            PythonChunkSource {
                chunk_id: "pkg/core.py".to_string(),
                module: "pkg.core".to_string(),
                path: "pkg/core.py".to_string(),
                source: r#"
BASE_LIMIT = 10

class Base:
    def validate(self, value: int) -> int:
        return value

def normalize(value: int) -> int:
    return value + BASE_LIMIT
"#
                .to_string(),
            },
            PythonChunkSource {
                chunk_id: "pkg/service.py".to_string(),
                module: "pkg.service".to_string(),
                path: "pkg/service.py".to_string(),
                source: r#"
from .core import BASE_LIMIT, Base, normalize

class Service(Base):
    def score(self, raw: Base) -> int:
        adjusted = normalize(raw.validate(BASE_LIMIT))
        return adjusted + BASE_LIMIT
"#
                .to_string(),
            },
            PythonChunkSource {
                chunk_id: "tests/test_service.py".to_string(),
                module: "tests.test_service".to_string(),
                path: "tests/test_service.py".to_string(),
                source: r#"
from pkg.service import Service

def test_score() -> None:
    assert Service().score(2) == 12
"#
                .to_string(),
            },
        ]
    }
}
