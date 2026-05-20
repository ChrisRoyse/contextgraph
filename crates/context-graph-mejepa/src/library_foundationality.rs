use std::collections::{BTreeMap, BTreeSet};

use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::chunk_foundationality::{
    compute_chunk_foundationality, write_chunk_dependency_edge_sync_readback, ChunkDependencyEdge,
    ChunkFoundationalityConfig, LibraryId,
};
use crate::error::MejepaInferError;

pub const LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LibraryRegistration {
    pub schema_version: u32,
    pub library_id: LibraryId,
    pub display_name: String,
    pub description: String,
    pub registered_at_unix_ms: i64,
    pub source_ref: String,
}

impl LibraryRegistration {
    pub fn new(
        library_id: LibraryId,
        description: impl Into<String>,
        registered_at_unix_ms: i64,
        source_ref: impl Into<String>,
    ) -> Self {
        let display_name = library_id.display_name();
        Self {
            schema_version: LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION,
            library_id,
            display_name,
            description: description.into(),
            registered_at_unix_ms,
            source_ref: source_ref.into(),
        }
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "library_registration.schema_version",
                format!(
                    "expected {LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        self.library_id
            .validate("library_registration.library_id")?;
        validate_single_line("library_registration.display_name", &self.display_name, 128)?;
        validate_single_line("library_registration.description", &self.description, 1024)?;
        if self.registered_at_unix_ms <= 0 {
            return invalid(
                "library_registration.registered_at_unix_ms",
                "registered_at_unix_ms must be positive",
            );
        }
        validate_single_line("library_registration.source_ref", &self.source_ref, 1024)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CrossLibraryReferenceCount {
    pub schema_version: u32,
    pub from_library_id: LibraryId,
    pub to_library_id: LibraryId,
    pub edge_count: u32,
    pub total_weight: f64,
    pub dependency_graph_sha256: String,
    pub computed_at_unix_ms: i64,
}

impl CrossLibraryReferenceCount {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "cross_library_reference.schema_version",
                format!(
                    "expected {LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        self.from_library_id
            .validate("cross_library_reference.from_library_id")?;
        self.to_library_id
            .validate("cross_library_reference.to_library_id")?;
        if self.from_library_id == self.to_library_id {
            return invalid(
                "cross_library_reference.library_pair",
                "cross-library reference pair must contain two different libraries",
            );
        }
        if self.edge_count == 0 {
            return invalid(
                "cross_library_reference.edge_count",
                "edge_count must be positive",
            );
        }
        if !self.total_weight.is_finite() || self.total_weight <= 0.0 {
            return invalid(
                "cross_library_reference.total_weight",
                "total_weight must be finite and positive",
            );
        }
        validate_sha256(
            "cross_library_reference.dependency_graph_sha256",
            &self.dependency_graph_sha256,
        )?;
        if self.computed_at_unix_ms <= 0 {
            return invalid(
                "cross_library_reference.computed_at_unix_ms",
                "computed_at_unix_ms must be positive",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LibraryChunkFoundationalityScore {
    pub schema_version: u32,
    pub library_id: LibraryId,
    pub library_slug: String,
    pub chunk_id: String,
    pub foundationality_score_within_library: f32,
    pub raw_pagerank_within_library: f64,
    pub rank_within_library: u32,
    pub foundationality_score_cross_library: f32,
    pub raw_pagerank_cross_library: f64,
    pub rank_cross_library: u32,
    pub within_upstream_count: u32,
    pub within_downstream_count: u32,
    pub cross_library_upstream_count: u32,
    pub cross_library_downstream_count: u32,
    pub within_library_graph_sha256: String,
    pub cross_library_graph_sha256: String,
    pub computed_at_unix_ms: i64,
}

impl LibraryChunkFoundationalityScore {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "library_foundationality_score.schema_version",
                format!(
                    "expected {LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        self.library_id
            .validate("library_foundationality_score.library_id")?;
        if self.library_slug != self.library_id.slug() {
            return invalid(
                "library_foundationality_score.library_slug",
                "library_slug must equal library_id.slug()",
            );
        }
        validate_single_line(
            "library_foundationality_score.chunk_id",
            &self.chunk_id,
            512,
        )?;
        validate_unit(
            "library_foundationality_score.foundationality_score_within_library",
            self.foundationality_score_within_library,
        )?;
        validate_unit(
            "library_foundationality_score.foundationality_score_cross_library",
            self.foundationality_score_cross_library,
        )?;
        validate_nonnegative_finite(
            "library_foundationality_score.raw_pagerank_within_library",
            self.raw_pagerank_within_library,
        )?;
        validate_nonnegative_finite(
            "library_foundationality_score.raw_pagerank_cross_library",
            self.raw_pagerank_cross_library,
        )?;
        if self.rank_within_library == 0 || self.rank_cross_library == 0 {
            return invalid(
                "library_foundationality_score.rank",
                "ranks are 1-based and must be non-zero",
            );
        }
        validate_sha256(
            "library_foundationality_score.within_library_graph_sha256",
            &self.within_library_graph_sha256,
        )?;
        validate_sha256(
            "library_foundationality_score.cross_library_graph_sha256",
            &self.cross_library_graph_sha256,
        )?;
        if self.computed_at_unix_ms <= 0 {
            return invalid(
                "library_foundationality_score.computed_at_unix_ms",
                "computed_at_unix_ms must be positive",
            );
        }
        Ok(())
    }
}

/// Per-chunk scoring failure surfaced when `library_score_from_parts` returned
/// an error during bedrock assembly. F-013 (#466) replaced the previous
/// `filter_map(.ok())` silent-drop with explicit accumulation so operators can
/// see WHICH chunks were dropped and WHY rather than only observing a smaller
/// `top_chunks` list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LibraryBedrockScoringFailure {
    pub chunk_id: String,
    pub reason: String,
}

impl LibraryBedrockScoringFailure {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_single_line(
            "library_bedrock_scoring_failure.chunk_id",
            &self.chunk_id,
            512,
        )?;
        validate_single_line("library_bedrock_scoring_failure.reason", &self.reason, 2048)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LibraryBedrockSummary {
    pub library_id: LibraryId,
    pub library_slug: String,
    pub top_chunks: Vec<LibraryChunkFoundationalityScore>,
    /// F-013 (#466): per-library list of chunks whose bedrock scoring failed
    /// during assembly. Empty when all chunks scored successfully. Surfaces
    /// dropped rows so operators do not silently see an incomplete top-K.
    #[serde(default)]
    pub scoring_failures: Vec<LibraryBedrockScoringFailure>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LibraryFoundationalityReport {
    pub schema_version: u32,
    pub algorithm: String,
    pub computed_at_unix_ms: i64,
    pub library_count: usize,
    pub node_count: usize,
    pub edge_count: usize,
    pub cross_library_edge_count: usize,
    pub cross_library_graph_sha256: String,
    pub registrations: Vec<LibraryRegistration>,
    pub scores: Vec<LibraryChunkFoundationalityScore>,
    pub cross_library_references: Vec<CrossLibraryReferenceCount>,
    pub per_library_bedrock: Vec<LibraryBedrockSummary>,
    pub cross_library_bedrock: Vec<LibraryChunkFoundationalityScore>,
    /// F-013 (#466): cross-library bedrock scoring failures. Same semantic as
    /// `LibraryBedrockSummary.scoring_failures` but for the cross-library
    /// top-K assembly. Empty when all chunks scored successfully.
    #[serde(default)]
    pub cross_library_scoring_failures: Vec<LibraryBedrockScoringFailure>,
}

impl LibraryFoundationalityReport {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION {
            return invalid(
                "library_foundationality_report.schema_version",
                format!(
                    "expected {LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_single_line(
            "library_foundationality_report.algorithm",
            &self.algorithm,
            160,
        )?;
        if self.computed_at_unix_ms <= 0 {
            return invalid(
                "library_foundationality_report.computed_at_unix_ms",
                "computed_at_unix_ms must be positive",
            );
        }
        if self.library_count == 0 || self.node_count == 0 || self.scores.is_empty() {
            return invalid(
                "library_foundationality_report.scores",
                "report must contain at least one library and one scored chunk",
            );
        }
        validate_sha256(
            "library_foundationality_report.cross_library_graph_sha256",
            &self.cross_library_graph_sha256,
        )?;
        for registration in &self.registrations {
            registration.validate()?;
        }
        for score in &self.scores {
            score.validate()?;
            if score.cross_library_graph_sha256 != self.cross_library_graph_sha256 {
                return invalid(
                    "library_foundationality_report.score_cross_hash",
                    "score cross-library hash must match report hash",
                );
            }
        }
        for reference in &self.cross_library_references {
            reference.validate()?;
            if reference.dependency_graph_sha256 != self.cross_library_graph_sha256 {
                return invalid(
                    "library_foundationality_report.reference_cross_hash",
                    "reference hash must match report hash",
                );
            }
        }
        // F-013 (#466): validate any per-library and cross-library scoring
        // failures attached to the report so a malformed failure row (empty
        // chunk_id, oversized reason) cannot slip through silently.
        for bedrock in &self.per_library_bedrock {
            for failure in &bedrock.scoring_failures {
                failure.validate()?;
            }
        }
        for failure in &self.cross_library_scoring_failures {
            failure.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LibraryFoundationalityQueryReport {
    pub requested_library_id: Option<LibraryId>,
    pub requested_library_slug: Option<String>,
    pub top_k: usize,
    pub registered_library_count: usize,
    pub persisted_score_count: usize,
    pub persisted_cross_library_reference_count: usize,
    pub library_bedrock: Vec<LibraryChunkFoundationalityScore>,
    pub cross_library_bedrock: Vec<LibraryChunkFoundationalityScore>,
    pub cross_library_references: Vec<CrossLibraryReferenceCount>,
}

pub fn compute_library_foundationality(
    edges: &[ChunkDependencyEdge],
    registrations: &[LibraryRegistration],
    computed_at_unix_ms: i64,
    config: ChunkFoundationalityConfig,
) -> Result<LibraryFoundationalityReport, MejepaInferError> {
    if computed_at_unix_ms <= 0 {
        return invalid(
            "library_foundationality.computed_at_unix_ms",
            "computed_at_unix_ms must be positive",
        );
    }
    if edges.is_empty() {
        return invalid(
            "library_foundationality.edges",
            "at least one dependency edge is required",
        );
    }
    if registrations.is_empty() {
        return invalid(
            "library_foundationality.registrations",
            "at least one library registration is required",
        );
    }
    config.validate()?;

    let mut registration_by_id = BTreeMap::new();
    for registration in registrations {
        registration.validate()?;
        if registration_by_id
            .insert(registration.library_id.clone(), registration.clone())
            .is_some()
        {
            return invalid(
                "library_foundationality.registrations",
                format!(
                    "duplicate registration for {}",
                    registration.library_id.slug()
                ),
            );
        }
    }

    let mut normalized_edges = edges.to_vec();
    for edge in &normalized_edges {
        edge.validate()?;
    }
    normalized_edges.sort_by(|left, right| {
        left.from_library_id
            .slug()
            .cmp(&right.from_library_id.slug())
            .then_with(|| left.from_chunk_id.cmp(&right.from_chunk_id))
            .then_with(|| left.to_library_id.slug().cmp(&right.to_library_id.slug()))
            .then_with(|| left.to_chunk_id.cmp(&right.to_chunk_id))
            .then_with(|| left.edge_kind.cmp(&right.edge_kind))
            .then_with(|| left.evidence_ref.cmp(&right.evidence_ref))
    });

    let library_by_chunk = library_by_chunk(&normalized_edges)?;
    let used_libraries = library_by_chunk.values().cloned().collect::<BTreeSet<_>>();
    for library_id in &used_libraries {
        if !registration_by_id.contains_key(library_id) {
            return invalid(
                "library_foundationality.registrations",
                format!("missing registration for {}", library_id.slug()),
            );
        }
    }

    let cross_report =
        compute_chunk_foundationality(&normalized_edges, computed_at_unix_ms, config)?;
    let cross_score_by_chunk = cross_report
        .scores
        .iter()
        .map(|score| (score.chunk_id.clone(), score.clone()))
        .collect::<BTreeMap<_, _>>();
    let cross_counts = cross_library_chunk_counts(&normalized_edges);

    let mut within_score_by_chunk = BTreeMap::new();
    let mut per_library_bedrock = Vec::new();
    for library_id in &used_libraries {
        let intra_edges = normalized_edges
            .iter()
            .filter(|edge| edge.from_library_id == *library_id && edge.to_library_id == *library_id)
            .cloned()
            .collect::<Vec<_>>();
        if intra_edges.is_empty() {
            return invalid(
                "library_foundationality.intra_library_edges",
                format!(
                    "library {} has no internal dependency edges",
                    library_id.slug()
                ),
            );
        }
        let within_report =
            compute_chunk_foundationality(&intra_edges, computed_at_unix_ms, config)?;
        for within_score in within_report.scores {
            within_score_by_chunk.insert(
                within_score.chunk_id.clone(),
                (
                    library_id.clone(),
                    within_report.dependency_graph_sha256.clone(),
                    within_score,
                ),
            );
        }
        // F-013 (#466): replace the previous `filter_map(.ok())` silent drop
        // with explicit failure accumulation so operators see WHICH chunks
        // were dropped and WHY. The top_chunks list rank ordering is now
        // explicitly auditable: any chunk missing from top_chunks either lost
        // the rank race or has a corresponding entry in scoring_failures.
        let mut library_scores: Vec<LibraryChunkFoundationalityScore> = Vec::new();
        let mut library_scoring_failures: Vec<LibraryBedrockScoringFailure> = Vec::new();
        for (_, within_hash, within_score) in within_score_by_chunk
            .values()
            .filter(|(score_library_id, _, _)| score_library_id == library_id)
        {
            match library_score_from_parts(
                library_id,
                within_hash,
                within_score,
                &cross_score_by_chunk,
                &cross_counts,
                &cross_report.dependency_graph_sha256,
                computed_at_unix_ms,
            ) {
                Ok(score) => library_scores.push(score),
                Err(err) => library_scoring_failures.push(LibraryBedrockScoringFailure {
                    chunk_id: within_score.chunk_id.clone(),
                    reason: format!("{}: {err}", err.code()),
                }),
            }
        }
        library_scores.sort_by(sort_within_library_scores);
        library_scores.truncate(10);
        library_scoring_failures.sort_by(|left, right| left.chunk_id.cmp(&right.chunk_id));
        per_library_bedrock.push(LibraryBedrockSummary {
            library_id: library_id.clone(),
            library_slug: library_id.slug(),
            top_chunks: library_scores,
            scoring_failures: library_scoring_failures,
        });
    }

    let mut scores = Vec::new();
    for (library_id, within_hash, within_score) in within_score_by_chunk.values() {
        scores.push(library_score_from_parts(
            library_id,
            within_hash,
            within_score,
            &cross_score_by_chunk,
            &cross_counts,
            &cross_report.dependency_graph_sha256,
            computed_at_unix_ms,
        )?);
    }
    scores.sort_by(sort_cross_library_scores);

    let mut cross_library_references = cross_library_reference_counts(
        &normalized_edges,
        &cross_report.dependency_graph_sha256,
        computed_at_unix_ms,
    )?;
    cross_library_references.sort_by(|left, right| {
        right
            .edge_count
            .cmp(&left.edge_count)
            .then_with(|| {
                left.from_library_id
                    .slug()
                    .cmp(&right.from_library_id.slug())
            })
            .then_with(|| left.to_library_id.slug().cmp(&right.to_library_id.slug()))
    });

    let mut registered = registration_by_id
        .into_values()
        .filter(|registration| used_libraries.contains(&registration.library_id))
        .collect::<Vec<_>>();
    registered.sort_by_key(|registration| registration.library_id.slug());
    let cross_library_bedrock = scores.iter().take(10).cloned().collect::<Vec<_>>();
    let report = LibraryFoundationalityReport {
        schema_version: LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION,
        algorithm: "per_library_and_cross_library_forward_push_pagerank".to_string(),
        computed_at_unix_ms,
        library_count: used_libraries.len(),
        node_count: library_by_chunk.len(),
        edge_count: normalized_edges.len(),
        cross_library_edge_count: normalized_edges
            .iter()
            .filter(|edge| edge.from_library_id != edge.to_library_id)
            .count(),
        cross_library_graph_sha256: cross_report.dependency_graph_sha256,
        registrations: registered,
        scores,
        cross_library_references,
        per_library_bedrock,
        cross_library_bedrock,
        // F-013 (#466): cross-library bedrock scoring is currently strict
        // (errors propagate via `?` at the cross-library `scores` build above),
        // so this vector is empty under the current implementation. The field
        // is present in the schema so future relaxation of the cross-library
        // path (matching the per-library failure-accumulation pattern) can
        // populate it without a breaking schema change.
        cross_library_scoring_failures: Vec::new(),
    };
    report.validate()?;
    Ok(report)
}

pub fn persist_library_foundationality_report_sync_readback(
    db: &DB,
    edges: &[ChunkDependencyEdge],
    report: &LibraryFoundationalityReport,
) -> Result<(), MejepaInferError> {
    report.validate()?;
    for registration in &report.registrations {
        write_library_registration_sync_readback(db, registration)?;
    }
    for edge in edges {
        write_chunk_dependency_edge_sync_readback(db, edge)?;
    }
    for score in &report.scores {
        write_library_foundationality_score_sync_readback(db, score)?;
    }
    for reference in &report.cross_library_references {
        write_cross_library_reference_count_sync_readback(db, reference)?;
    }
    Ok(())
}

pub fn write_library_registration_sync_readback(
    db: &DB,
    registration: &LibraryRegistration,
) -> Result<(), MejepaInferError> {
    registration.validate()?;
    write_value_sync_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_LIBRARY_REGISTRY,
        &library_registration_key(&registration.library_id)?,
        registration,
    )
}

pub fn read_library_registration(
    db: &DB,
    library_id: &LibraryId,
) -> Result<Option<LibraryRegistration>, MejepaInferError> {
    library_id.validate("library_registration.library_id")?;
    read_value(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_LIBRARY_REGISTRY,
        &library_registration_key(library_id)?,
    )
}

pub fn read_all_library_registrations(
    db: &DB,
) -> Result<Vec<LibraryRegistration>, MejepaInferError> {
    read_all_values(db, context_graph_mejepa_cf::CF_MEJEPA_LIBRARY_REGISTRY)
}

pub fn write_library_foundationality_score_sync_readback(
    db: &DB,
    score: &LibraryChunkFoundationalityScore,
) -> Result<(), MejepaInferError> {
    score.validate()?;
    write_value_sync_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_LIBRARY_FOUNDATIONALITY,
        &library_foundationality_score_key(&score.library_id, &score.chunk_id)?,
        score,
    )
}

pub fn read_all_library_foundationality_scores(
    db: &DB,
) -> Result<Vec<LibraryChunkFoundationalityScore>, MejepaInferError> {
    read_all_values(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_LIBRARY_FOUNDATIONALITY,
    )
}

pub fn write_cross_library_reference_count_sync_readback(
    db: &DB,
    reference: &CrossLibraryReferenceCount,
) -> Result<(), MejepaInferError> {
    reference.validate()?;
    write_value_sync_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_CROSS_LIBRARY_REFERENCES,
        &cross_library_reference_key(&reference.from_library_id, &reference.to_library_id)?,
        reference,
    )
}

pub fn read_all_cross_library_reference_counts(
    db: &DB,
) -> Result<Vec<CrossLibraryReferenceCount>, MejepaInferError> {
    read_all_values(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_CROSS_LIBRARY_REFERENCES,
    )
}

pub fn read_library_foundationality_report(
    db: &DB,
    library_id: Option<&LibraryId>,
    top_k: usize,
) -> Result<LibraryFoundationalityQueryReport, MejepaInferError> {
    if top_k == 0 {
        return invalid("library_foundationality.top_k", "top_k must be positive");
    }
    if let Some(id) = library_id {
        id.validate("library_foundationality.library_id")?;
    }
    let registrations = read_all_library_registrations(db)?;
    let mut scores = read_all_library_foundationality_scores(db)?;
    let mut references = read_all_cross_library_reference_counts(db)?;
    for registration in &registrations {
        registration.validate()?;
    }
    for score in &scores {
        score.validate()?;
    }
    for reference in &references {
        reference.validate()?;
    }
    let persisted_cross_library_reference_count = references.len();

    let mut library_bedrock = scores
        .iter()
        .filter(|score| library_id.map(|id| &score.library_id == id).unwrap_or(true))
        .cloned()
        .collect::<Vec<_>>();
    library_bedrock.sort_by(sort_within_library_scores);
    library_bedrock.truncate(top_k);

    scores.sort_by(sort_cross_library_scores);
    let mut cross_library_bedrock = scores
        .iter()
        .filter(|score| library_id.map(|id| &score.library_id == id).unwrap_or(true))
        .take(top_k)
        .cloned()
        .collect::<Vec<_>>();
    cross_library_bedrock.sort_by(sort_cross_library_scores);

    if let Some(id) = library_id {
        references
            .retain(|reference| &reference.from_library_id == id || &reference.to_library_id == id);
    }
    references.truncate(top_k);

    Ok(LibraryFoundationalityQueryReport {
        requested_library_id: library_id.cloned(),
        requested_library_slug: library_id.map(LibraryId::slug),
        top_k,
        registered_library_count: registrations.len(),
        persisted_score_count: scores.len(),
        persisted_cross_library_reference_count,
        library_bedrock,
        cross_library_bedrock,
        cross_library_references: references,
    })
}

fn library_score_from_parts(
    library_id: &LibraryId,
    within_hash: &str,
    within_score: &crate::ChunkFoundationalityScore,
    cross_score_by_chunk: &BTreeMap<String, crate::ChunkFoundationalityScore>,
    cross_counts: &BTreeMap<String, (u32, u32)>,
    cross_hash: &str,
    computed_at_unix_ms: i64,
) -> Result<LibraryChunkFoundationalityScore, MejepaInferError> {
    let cross_score = cross_score_by_chunk
        .get(&within_score.chunk_id)
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "library_foundationality.cross_score".to_string(),
            detail: format!("missing cross-library score for {}", within_score.chunk_id),
        })?;
    let (cross_upstream, cross_downstream) = cross_counts
        .get(&within_score.chunk_id)
        .copied()
        .unwrap_or((0, 0));
    let row = LibraryChunkFoundationalityScore {
        schema_version: LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION,
        library_id: library_id.clone(),
        library_slug: library_id.slug(),
        chunk_id: within_score.chunk_id.clone(),
        foundationality_score_within_library: within_score.foundationality_score,
        raw_pagerank_within_library: within_score.raw_pagerank,
        rank_within_library: within_score.rank,
        foundationality_score_cross_library: cross_score.foundationality_score,
        raw_pagerank_cross_library: cross_score.raw_pagerank,
        rank_cross_library: cross_score.rank,
        within_upstream_count: within_score.upstream_count,
        within_downstream_count: within_score.downstream_count,
        cross_library_upstream_count: cross_upstream,
        cross_library_downstream_count: cross_downstream,
        within_library_graph_sha256: within_hash.to_string(),
        cross_library_graph_sha256: cross_hash.to_string(),
        computed_at_unix_ms,
    };
    row.validate()?;
    Ok(row)
}

fn library_by_chunk(
    edges: &[ChunkDependencyEdge],
) -> Result<BTreeMap<String, LibraryId>, MejepaInferError> {
    let mut library_by_chunk = BTreeMap::new();
    for edge in edges {
        insert_chunk_library(
            &mut library_by_chunk,
            &edge.from_chunk_id,
            &edge.from_library_id,
        )?;
        insert_chunk_library(
            &mut library_by_chunk,
            &edge.to_chunk_id,
            &edge.to_library_id,
        )?;
    }
    Ok(library_by_chunk)
}

fn insert_chunk_library(
    library_by_chunk: &mut BTreeMap<String, LibraryId>,
    chunk_id: &str,
    library_id: &LibraryId,
) -> Result<(), MejepaInferError> {
    match library_by_chunk.insert(chunk_id.to_string(), library_id.clone()) {
        Some(previous) if previous != *library_id => invalid(
            "library_foundationality.chunk_library",
            format!(
                "chunk {chunk_id:?} is assigned to both {} and {}",
                previous.slug(),
                library_id.slug()
            ),
        ),
        _ => Ok(()),
    }
}

fn cross_library_chunk_counts(edges: &[ChunkDependencyEdge]) -> BTreeMap<String, (u32, u32)> {
    let mut upstream = BTreeMap::<String, BTreeSet<String>>::new();
    let mut downstream = BTreeMap::<String, BTreeSet<String>>::new();
    for edge in edges {
        if edge.from_library_id == edge.to_library_id {
            continue;
        }
        upstream
            .entry(edge.to_chunk_id.clone())
            .or_default()
            .insert(edge.from_chunk_id.clone());
        downstream
            .entry(edge.from_chunk_id.clone())
            .or_default()
            .insert(edge.to_chunk_id.clone());
    }
    let mut out = BTreeMap::new();
    for chunk_id in upstream.keys().chain(downstream.keys()) {
        out.insert(
            chunk_id.clone(),
            (
                upstream.get(chunk_id).map(BTreeSet::len).unwrap_or(0) as u32,
                downstream.get(chunk_id).map(BTreeSet::len).unwrap_or(0) as u32,
            ),
        );
    }
    out
}

fn cross_library_reference_counts(
    edges: &[ChunkDependencyEdge],
    graph_sha256: &str,
    computed_at_unix_ms: i64,
) -> Result<Vec<CrossLibraryReferenceCount>, MejepaInferError> {
    let mut counts = BTreeMap::<(LibraryId, LibraryId), (u32, f64)>::new();
    for edge in edges {
        if edge.from_library_id == edge.to_library_id {
            continue;
        }
        let entry = counts
            .entry((edge.from_library_id.clone(), edge.to_library_id.clone()))
            .or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += edge.weight;
    }
    let mut rows = Vec::new();
    for ((from_library_id, to_library_id), (edge_count, total_weight)) in counts {
        let row = CrossLibraryReferenceCount {
            schema_version: LIBRARY_FOUNDATIONALITY_SCHEMA_VERSION,
            from_library_id,
            to_library_id,
            edge_count,
            total_weight,
            dependency_graph_sha256: graph_sha256.to_string(),
            computed_at_unix_ms,
        };
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

fn sort_within_library_scores(
    left: &LibraryChunkFoundationalityScore,
    right: &LibraryChunkFoundationalityScore,
) -> std::cmp::Ordering {
    right
        .foundationality_score_within_library
        .partial_cmp(&left.foundationality_score_within_library)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| left.library_slug.cmp(&right.library_slug))
        .then_with(|| left.rank_within_library.cmp(&right.rank_within_library))
        .then_with(|| left.chunk_id.cmp(&right.chunk_id))
}

fn sort_cross_library_scores(
    left: &LibraryChunkFoundationalityScore,
    right: &LibraryChunkFoundationalityScore,
) -> std::cmp::Ordering {
    right
        .foundationality_score_cross_library
        .partial_cmp(&left.foundationality_score_cross_library)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            right
                .cross_library_upstream_count
                .cmp(&left.cross_library_upstream_count)
        })
        .then_with(|| left.rank_cross_library.cmp(&right.rank_cross_library))
        .then_with(|| left.library_slug.cmp(&right.library_slug))
        .then_with(|| left.chunk_id.cmp(&right.chunk_id))
}

fn library_registration_key(library_id: &LibraryId) -> Result<Vec<u8>, MejepaInferError> {
    library_id.validate("library_registration.library_id")?;
    Ok(format!("library:{}", sha256_hex(library_id.slug().as_bytes())).into_bytes())
}

fn library_foundationality_score_key(
    library_id: &LibraryId,
    chunk_id: &str,
) -> Result<Vec<u8>, MejepaInferError> {
    library_id.validate("library_foundationality_score.library_id")?;
    validate_single_line("library_foundationality_score.chunk_id", chunk_id, 512)?;
    Ok(format!(
        "score:{}",
        sha256_hex(format!("{}:{chunk_id}", library_id.slug()).as_bytes())
    )
    .into_bytes())
}

fn cross_library_reference_key(
    from_library_id: &LibraryId,
    to_library_id: &LibraryId,
) -> Result<Vec<u8>, MejepaInferError> {
    from_library_id.validate("cross_library_reference.from_library_id")?;
    to_library_id.validate("cross_library_reference.to_library_id")?;
    Ok(format!(
        "cross-ref:{}",
        sha256_hex(format!("{}->{}", from_library_id.slug(), to_library_id.slug()).as_bytes())
    )
    .into_bytes())
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

fn validate_nonnegative_finite(field: &str, value: f64) -> Result<(), MejepaInferError> {
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

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{open_infer_rocksdb, read_all_chunk_dependency_edges};

    #[test]
    fn library_foundationality_identifies_cross_library_bedrock() {
        let edges = fixture_edges();
        let report = compute_library_foundationality(
            &edges,
            &fixture_registrations(),
            42,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        assert_eq!(report.library_count, 2);
        assert!(report.cross_library_edge_count >= 2);
        assert_eq!(report.cross_library_bedrock[0].chunk_id, "py:core");
        assert_eq!(
            report.cross_library_bedrock[0].library_id,
            LibraryId::PythonSweBenchLite
        );
        assert!(
            report.cross_library_bedrock[0].cross_library_upstream_count
                > report.scores.last().unwrap().cross_library_upstream_count
        );
    }

    #[test]
    fn happy_path_emits_empty_scoring_failures() {
        // F-013 (#466) regression: on a well-formed corpus, all chunk rows
        // score successfully, so every LibraryBedrockSummary.scoring_failures
        // must be empty AND the report cross_library_scoring_failures must be
        // empty. This guarantees the new sibling field is wired and serializes,
        // and that we haven't accidentally elevated benign empty rows into
        // failure entries.
        let edges = fixture_edges();
        let report = compute_library_foundationality(
            &edges,
            &fixture_registrations(),
            42,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        for bedrock in &report.per_library_bedrock {
            assert!(
                bedrock.scoring_failures.is_empty(),
                "library {} should have no scoring failures on the happy path; got {:?}",
                bedrock.library_slug,
                bedrock.scoring_failures
            );
        }
        assert!(report.cross_library_scoring_failures.is_empty());
    }

    #[test]
    fn library_bedrock_scoring_failure_validates_chunk_id_and_reason() {
        // F-013 (#466) regression: ensure the validate() implementation
        // rejects empty / oversized fields so a malformed failure row cannot
        // slip past the persistence path.
        let ok = LibraryBedrockScoringFailure {
            chunk_id: "py:core".to_string(),
            reason: "MEJEPA_INFER_INVALID_INPUT: deterministic test reason".to_string(),
        };
        ok.validate().expect("well-formed failure must validate");

        let empty_chunk = LibraryBedrockScoringFailure {
            chunk_id: String::new(),
            reason: "any".to_string(),
        };
        assert!(empty_chunk.validate().is_err());

        let huge_reason = LibraryBedrockScoringFailure {
            chunk_id: "py:core".to_string(),
            reason: "x".repeat(5_000),
        };
        assert!(huge_reason.validate().is_err());
    }

    #[test]
    fn report_validate_rejects_invalid_scoring_failure_attached() {
        // F-013 (#466) regression: validate() must traverse into the new
        // scoring_failures fields. A report with a malformed failure must not
        // survive validation.
        let edges = fixture_edges();
        let mut report = compute_library_foundationality(
            &edges,
            &fixture_registrations(),
            42,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        assert!(report.validate().is_ok());
        // Inject a malformed failure on the first per-library bedrock.
        report.per_library_bedrock[0].scoring_failures = vec![LibraryBedrockScoringFailure {
            chunk_id: String::new(),
            reason: "synthetic injection".to_string(),
        }];
        let err = report.validate().unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");
    }

    #[test]
    fn library_foundationality_round_trips_rows() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let edges = fixture_edges();
        let report = compute_library_foundationality(
            &edges,
            &fixture_registrations(),
            42,
            ChunkFoundationalityConfig::default(),
        )
        .unwrap();
        persist_library_foundationality_report_sync_readback(db.as_ref(), &edges, &report).unwrap();
        drop(db);

        let reopened = open_infer_rocksdb(temp.path()).unwrap();
        let reopened_edges = read_all_chunk_dependency_edges(reopened.as_ref()).unwrap();
        assert!(reopened_edges
            .iter()
            .any(|edge| edge.from_library_id != edge.to_library_id));
        let query = read_library_foundationality_report(
            reopened.as_ref(),
            Some(&LibraryId::PythonSweBenchLite),
            3,
        )
        .unwrap();
        assert_eq!(query.library_bedrock.len(), 3);
        assert_eq!(query.cross_library_bedrock[0].chunk_id, "py:core");
    }

    fn fixture_registrations() -> Vec<LibraryRegistration> {
        vec![
            LibraryRegistration::new(
                LibraryId::PythonSweBenchLite,
                "Python corpus",
                42,
                "test:python",
            ),
            LibraryRegistration::new(
                LibraryId::ShakespeareCanon,
                "Shakespeare canon",
                42,
                "test:shakespeare",
            ),
        ]
    }

    fn fixture_edges() -> Vec<ChunkDependencyEdge> {
        let py = LibraryId::PythonSweBenchLite;
        let shakespeare = LibraryId::ShakespeareCanon;
        vec![
            ChunkDependencyEdge::new_with_libraries(
                py.clone(),
                "py:api",
                py.clone(),
                "py:core",
                "call",
                1.0,
                "fixture:py-api-core",
            ),
            ChunkDependencyEdge::new_with_libraries(
                py.clone(),
                "py:test",
                py.clone(),
                "py:api",
                "test",
                1.0,
                "fixture:py-test-api",
            ),
            ChunkDependencyEdge::new_with_libraries(
                shakespeare.clone(),
                "shake:scene",
                shakespeare.clone(),
                "shake:canon",
                "quote",
                1.0,
                "fixture:shake-scene-canon",
            ),
            ChunkDependencyEdge::new_with_libraries(
                shakespeare.clone(),
                "shake:commentary",
                shakespeare.clone(),
                "shake:scene",
                "quote",
                1.0,
                "fixture:shake-commentary-scene",
            ),
            ChunkDependencyEdge::new_with_libraries(
                shakespeare.clone(),
                "shake:canon",
                py.clone(),
                "py:core",
                "cross_quote",
                2.0,
                "fixture:shake-canon-py-core",
            ),
            ChunkDependencyEdge::new_with_libraries(
                shakespeare,
                "shake:scene",
                py,
                "py:core",
                "cross_quote",
                1.0,
                "fixture:shake-scene-py-core",
            ),
        ]
    }
}
