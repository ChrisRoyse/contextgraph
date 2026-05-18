use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::chunk_foundationality::{
    compute_chunk_foundationality, foundationality_fisher_multiplier, ChunkDependencyEdge,
    ChunkFoundationalityConfig,
};
use crate::dynamic_embedder::RuntimeEmbedderId;
use crate::dynamic_embedder_vram::DynamicEmbedderEvictionDecision;
use crate::error::MejepaInferError;

pub const EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION: u32 = 1;
pub const MEJEPA_EMBEDDER_FOUNDATIONALITY_EVICTION_BLOCKED: &str =
    "MEJEPA_EMBEDDER_FOUNDATIONALITY_EVICTION_BLOCKED";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EmbedderDependencyEdge {
    pub schema_version: u32,
    pub from_embedder_id: String,
    pub to_embedder_id: String,
    pub degradation_delta: f32,
    pub evidence_ref: String,
}

impl EmbedderDependencyEdge {
    pub fn new(
        from_embedder_id: RuntimeEmbedderId,
        to_embedder_id: RuntimeEmbedderId,
        degradation_delta: f32,
        evidence_ref: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION,
            from_embedder_id: from_embedder_id.slug().into_owned(),
            to_embedder_id: to_embedder_id.slug().into_owned(),
            degradation_delta,
            evidence_ref: evidence_ref.into(),
        }
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "embedder_dependency_edge.schema_version",
                format!(
                    "expected {EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        parse_runtime_embedder_id("embedder_dependency_edge.from", &self.from_embedder_id)?;
        parse_runtime_embedder_id("embedder_dependency_edge.to", &self.to_embedder_id)?;
        validate_positive_finite(
            "embedder_dependency_edge.degradation_delta",
            self.degradation_delta,
        )?;
        validate_single_line(
            "embedder_dependency_edge.evidence_ref",
            &self.evidence_ref,
            1024,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EmbedderFoundationalityScore {
    pub schema_version: u32,
    pub embedder_id: String,
    pub foundationality_score: f32,
    pub raw_pagerank: f64,
    pub rank: u32,
    pub dependency_graph_sha256: String,
    pub computed_at_unix_ms: i64,
    pub fisher_lambda: f32,
    pub fisher_multiplier: f32,
}

impl EmbedderFoundationalityScore {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "embedder_foundationality_score.schema_version",
                format!(
                    "expected {EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        parse_runtime_embedder_id(
            "embedder_foundationality_score.embedder_id",
            &self.embedder_id,
        )?;
        validate_unit(
            "embedder_foundationality_score.foundationality_score",
            self.foundationality_score,
        )?;
        if !self.raw_pagerank.is_finite() || self.raw_pagerank < 0.0 {
            return invalid(
                "embedder_foundationality_score.raw_pagerank",
                "raw_pagerank must be finite and non-negative",
            );
        }
        if self.rank == 0 {
            return invalid(
                "embedder_foundationality_score.rank",
                "rank must be non-zero",
            );
        }
        validate_sha256(
            "embedder_foundationality_score.dependency_graph_sha256",
            &self.dependency_graph_sha256,
        )?;
        if self.computed_at_unix_ms <= 0 {
            return invalid(
                "embedder_foundationality_score.computed_at_unix_ms",
                "computed_at_unix_ms must be positive",
            );
        }
        validate_nonnegative_finite(
            "embedder_foundationality_score.fisher_lambda",
            self.fisher_lambda,
        )?;
        validate_positive_finite(
            "embedder_foundationality_score.fisher_multiplier",
            self.fisher_multiplier,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EmbedderFoundationalityReport {
    pub schema_version: u32,
    pub algorithm: String,
    pub dependency_graph_sha256: String,
    pub computed_at_unix_ms: i64,
    pub node_count: usize,
    pub edge_count: usize,
    pub scores: Vec<EmbedderFoundationalityScore>,
}

impl EmbedderFoundationalityReport {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "embedder_foundationality_report.schema_version",
                format!(
                    "expected {EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_single_line(
            "embedder_foundationality_report.algorithm",
            &self.algorithm,
            128,
        )?;
        validate_sha256(
            "embedder_foundationality_report.dependency_graph_sha256",
            &self.dependency_graph_sha256,
        )?;
        if self.computed_at_unix_ms <= 0 {
            return invalid(
                "embedder_foundationality_report.computed_at_unix_ms",
                "computed_at_unix_ms must be positive",
            );
        }
        if self.node_count == 0 || self.edge_count == 0 || self.scores.len() != self.node_count {
            return invalid(
                "embedder_foundationality_report.scores",
                "node_count, edge_count, and score count must be non-empty and consistent",
            );
        }
        let mut ids = BTreeSet::new();
        for score in &self.scores {
            score.validate()?;
            if !ids.insert(score.embedder_id.clone()) {
                return invalid(
                    "embedder_foundationality_report.scores",
                    format!("duplicate score for {}", score.embedder_id),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EmbedderFoundationalityEvictionGuardReport {
    pub blocked: bool,
    pub reason_code: Option<String>,
    pub threshold: f32,
    pub operator_approved: bool,
    pub protected_eviction_ids: Vec<RuntimeEmbedderId>,
    pub base_plan: DynamicEmbedderEvictionDecision,
}

pub fn compute_embedder_foundationality(
    edges: &[EmbedderDependencyEdge],
    computed_at_unix_ms: i64,
    config: ChunkFoundationalityConfig,
) -> Result<EmbedderFoundationalityReport, MejepaInferError> {
    if edges.is_empty() {
        return invalid(
            "embedder_dependency_graph.edges",
            "embedder dependency graph requires at least one edge",
        );
    }
    let mut chunk_edges = Vec::with_capacity(edges.len());
    for edge in edges {
        edge.validate()?;
        chunk_edges.push(ChunkDependencyEdge::new(
            edge.from_embedder_id.clone(),
            edge.to_embedder_id.clone(),
            "embedder_dependency",
            f64::from(edge.degradation_delta),
            edge.evidence_ref.clone(),
        ));
    }
    let chunk_report = compute_chunk_foundationality(&chunk_edges, computed_at_unix_ms, config)?;
    let scores = chunk_report
        .scores
        .into_iter()
        .map(|score| EmbedderFoundationalityScore {
            schema_version: EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION,
            embedder_id: score.chunk_id,
            foundationality_score: score.foundationality_score,
            raw_pagerank: score.raw_pagerank,
            rank: score.rank,
            dependency_graph_sha256: score.dependency_graph_sha256,
            computed_at_unix_ms: score.computed_at_unix_ms,
            fisher_lambda: score.fisher_lambda,
            fisher_multiplier: score.fisher_multiplier,
        })
        .collect::<Vec<_>>();
    let report = EmbedderFoundationalityReport {
        schema_version: EMBEDDER_FOUNDATIONALITY_SCHEMA_VERSION,
        algorithm: "pagerank/forward-push/embedder-dependency-v1".to_string(),
        dependency_graph_sha256: chunk_report.dependency_graph_sha256,
        computed_at_unix_ms,
        node_count: chunk_report.node_count,
        edge_count: chunk_report.edge_count,
        scores,
    };
    report.validate()?;
    Ok(report)
}

pub fn apply_embedder_foundationality_fisher_multiplier(
    lambda_fisher: f32,
    foundationality_score: f32,
    base_fisher: f32,
) -> Result<f32, MejepaInferError> {
    validate_nonnegative_finite("embedder_foundationality_fisher.base_fisher", base_fisher)?;
    let multiplier = foundationality_fisher_multiplier(foundationality_score, lambda_fisher)?;
    let value = base_fisher * multiplier;
    validate_nonnegative_finite("embedder_foundationality_fisher.weighted", value)?;
    Ok(value)
}

pub fn guard_dynamic_embedder_evictions_by_foundationality(
    base_plan: DynamicEmbedderEvictionDecision,
    scores: &[EmbedderFoundationalityScore],
    protected_threshold: f32,
    operator_approved: bool,
) -> Result<EmbedderFoundationalityEvictionGuardReport, MejepaInferError> {
    validate_unit(
        "embedder_foundationality_eviction.protected_threshold",
        protected_threshold,
    )?;
    let score_by_id = scores
        .iter()
        .map(|score| {
            score.validate()?;
            Ok((score.embedder_id.clone(), score.foundationality_score))
        })
        .collect::<Result<BTreeMap<_, _>, MejepaInferError>>()?;
    let protected_eviction_ids = base_plan
        .evicted_ids
        .iter()
        .filter_map(|id| {
            let score = score_by_id.get(id.slug().as_ref()).copied().unwrap_or(0.0);
            (score >= protected_threshold).then(|| id.clone())
        })
        .collect::<Vec<_>>();
    let blocked = !operator_approved && !protected_eviction_ids.is_empty();
    Ok(EmbedderFoundationalityEvictionGuardReport {
        blocked,
        reason_code: blocked.then(|| MEJEPA_EMBEDDER_FOUNDATIONALITY_EVICTION_BLOCKED.to_string()),
        threshold: protected_threshold,
        operator_approved,
        protected_eviction_ids,
        base_plan,
    })
}

pub fn persist_embedder_foundationality_report_sync_readback(
    db: &DB,
    edges: &[EmbedderDependencyEdge],
    report: &EmbedderFoundationalityReport,
) -> Result<(), MejepaInferError> {
    report.validate()?;
    for edge in edges {
        write_embedder_dependency_edge_sync_readback(db, edge)?;
    }
    for score in &report.scores {
        write_embedder_foundationality_score_sync_readback(db, score)?;
    }
    Ok(())
}

pub fn write_embedder_dependency_edge_sync_readback(
    db: &DB,
    edge: &EmbedderDependencyEdge,
) -> Result<(), MejepaInferError> {
    edge.validate()?;
    write_value_sync_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH,
        &embedder_dependency_edge_key(edge)?,
        edge,
    )
}

pub fn read_all_embedder_dependency_edges(
    db: &DB,
) -> Result<Vec<EmbedderDependencyEdge>, MejepaInferError> {
    read_all_values(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH,
    )
}

pub fn write_embedder_foundationality_score_sync_readback(
    db: &DB,
    score: &EmbedderFoundationalityScore,
) -> Result<(), MejepaInferError> {
    score.validate()?;
    write_value_sync_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_FOUNDATIONALITY,
        &embedder_foundationality_score_key(&score.embedder_id)?,
        score,
    )
}

pub fn read_embedder_foundationality_score(
    db: &DB,
    embedder_id: &str,
) -> Result<Option<EmbedderFoundationalityScore>, MejepaInferError> {
    parse_runtime_embedder_id("embedder_foundationality_score.embedder_id", embedder_id)?;
    read_value(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_FOUNDATIONALITY,
        &embedder_foundationality_score_key(embedder_id)?,
    )
}

pub fn read_all_embedder_foundationality_scores(
    db: &DB,
) -> Result<Vec<EmbedderFoundationalityScore>, MejepaInferError> {
    read_all_values(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_FOUNDATIONALITY,
    )
}

fn embedder_dependency_edge_key(
    edge: &EmbedderDependencyEdge,
) -> Result<Vec<u8>, MejepaInferError> {
    edge.validate()?;
    let mut hasher = Sha256::new();
    hasher.update(edge.from_embedder_id.as_bytes());
    hasher.update([0]);
    hasher.update(edge.to_embedder_id.as_bytes());
    hasher.update([0]);
    hasher.update(edge.evidence_ref.as_bytes());
    Ok(format!(
        "edge/{}/{}/{}",
        edge.from_embedder_id,
        edge.to_embedder_id,
        hex::encode(hasher.finalize())
    )
    .into_bytes())
}

fn embedder_foundationality_score_key(embedder_id: &str) -> Result<Vec<u8>, MejepaInferError> {
    parse_runtime_embedder_id("embedder_foundationality_score.embedder_id", embedder_id)?;
    Ok(format!("score/{embedder_id}").into_bytes())
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
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        out.push(bincode::deserialize(&value)?);
    }
    Ok(out)
}

fn parse_runtime_embedder_id(
    field: &str,
    value: &str,
) -> Result<RuntimeEmbedderId, MejepaInferError> {
    RuntimeEmbedderId::from_str(value).map_err(|err| MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: err.to_string(),
    })
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

fn validate_sha256(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return invalid(
            field,
            "must be a 64-character lowercase/uppercase hex sha256",
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

fn validate_positive_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || value <= 0.0 {
        return invalid(field, "must be finite and positive");
    }
    Ok(())
}

fn validate_nonnegative_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || value < 0.0 {
        return invalid(field, "must be finite and non-negative");
    }
    Ok(())
}

fn invalid<T>(field: impl Into<String>, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic_embedder_vram::DynamicEmbedderEvictionDecision;

    #[test]
    fn embedder_foundationality_scores_known_hierarchy() {
        let report = compute_embedder_foundationality(
            &fixture_edges(),
            1_779_100_000_000,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        assert_eq!(
            report.scores[0].embedder_id,
            dynamic(1, "corpus_rotate_v1").slug()
        );
        assert_eq!(report.scores[0].foundationality_score, 1.0);
    }

    #[test]
    fn foundationality_guard_blocks_unapproved_eviction() {
        let report = compute_embedder_foundationality(
            &fixture_edges(),
            1_779_100_000_000,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        let base_plan = DynamicEmbedderEvictionDecision {
            eviction_required: true,
            budget_satisfied: true,
            reason_code: None,
            before_required_bytes: 1200,
            after_required_bytes: 900,
            budget_bytes: 900,
            freed_vram_bytes: 300,
            evicted_ids: vec![dynamic(1, "corpus_rotate_v1")],
            retained_active_ids: vec![dynamic(2, "syntax_head")],
            missing_utility_ids: Vec::new(),
        };
        let blocked = guard_dynamic_embedder_evictions_by_foundationality(
            base_plan.clone(),
            &report.scores,
            0.75,
            false,
        )
        .unwrap();
        assert!(blocked.blocked);
        assert_eq!(
            blocked.reason_code.as_deref(),
            Some(MEJEPA_EMBEDDER_FOUNDATIONALITY_EVICTION_BLOCKED)
        );
        let approved = guard_dynamic_embedder_evictions_by_foundationality(
            base_plan,
            &report.scores,
            0.75,
            true,
        )
        .unwrap();
        assert!(!approved.blocked);
    }

    fn fixture_edges() -> Vec<EmbedderDependencyEdge> {
        vec![
            EmbedderDependencyEdge::new(
                dynamic(2, "syntax_head"),
                dynamic(1, "corpus_rotate_v1"),
                0.03,
                "heldout:q1",
            ),
            EmbedderDependencyEdge::new(
                dynamic(3, "resource_head"),
                dynamic(1, "corpus_rotate_v1"),
                0.04,
                "heldout:q2",
            ),
            EmbedderDependencyEdge::new(
                dynamic(4, "test_head"),
                dynamic(1, "corpus_rotate_v1"),
                0.02,
                "heldout:q3",
            ),
            EmbedderDependencyEdge::new(
                dynamic(2, "syntax_head"),
                "e7".parse().unwrap(),
                0.01,
                "heldout:q1",
            ),
            EmbedderDependencyEdge::new(
                dynamic(3, "resource_head"),
                "e8".parse().unwrap(),
                0.01,
                "heldout:q2",
            ),
        ]
    }

    fn dynamic(sequence: u32, name: &str) -> RuntimeEmbedderId {
        RuntimeEmbedderId::dynamic(sequence, name).unwrap()
    }
}
