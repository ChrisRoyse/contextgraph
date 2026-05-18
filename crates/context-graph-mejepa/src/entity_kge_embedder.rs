use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::MejepaInferError;

pub const ENTITY_KGE_SCHEMA_VERSION: u32 = 1;
pub const ENTITY_KGE_OUTPUT_DIMENSION: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKgeRelationKind {
    Calls,
    Imports,
    Inherits,
    HasType,
    TestVerifies,
    PatchModifies,
}

impl EntityKgeRelationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Calls => "calls",
            Self::Imports => "imports",
            Self::Inherits => "inherits",
            Self::HasType => "has_type",
            Self::TestVerifies => "test_verifies",
            Self::PatchModifies => "patch_modifies",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityDependencyEdgeInput {
    pub head_entity: String,
    pub relation: EntityKgeRelationKind,
    pub tail_entity: String,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityKgeTriple {
    pub head_entity: String,
    pub relation: EntityKgeRelationKind,
    pub tail_entity: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityDependencyGraph {
    pub schema_version: u32,
    pub source_corpus_ref: String,
    pub entities: Vec<String>,
    pub triples: Vec<EntityKgeTriple>,
}

impl EntityDependencyGraph {
    pub fn from_edges(
        source_corpus_ref: impl Into<String>,
        edges: Vec<EntityDependencyEdgeInput>,
    ) -> Result<Self, MejepaInferError> {
        let source_corpus_ref = source_corpus_ref.into();
        validate_text("entity_kge.source_corpus_ref", &source_corpus_ref, 512)?;
        if edges.is_empty() {
            return invalid("entity_kge.edges", "edge set must be non-empty");
        }

        let mut entities = BTreeSet::new();
        let mut triples: BTreeMap<(String, EntityKgeRelationKind, String), BTreeSet<String>> =
            BTreeMap::new();
        for edge in edges {
            validate_text("entity_kge.head_entity", &edge.head_entity, 512)?;
            validate_text("entity_kge.tail_entity", &edge.tail_entity, 512)?;
            validate_text("entity_kge.evidence_ref", &edge.evidence_ref, 1024)?;
            entities.insert(edge.head_entity.clone());
            entities.insert(edge.tail_entity.clone());
            triples
                .entry((edge.head_entity, edge.relation, edge.tail_entity))
                .or_default()
                .insert(edge.evidence_ref);
        }
        let graph = Self {
            schema_version: ENTITY_KGE_SCHEMA_VERSION,
            source_corpus_ref,
            entities: entities.into_iter().collect(),
            triples: triples
                .into_iter()
                .map(
                    |((head_entity, relation, tail_entity), evidence_refs)| EntityKgeTriple {
                        head_entity,
                        relation,
                        tail_entity,
                        evidence_refs: evidence_refs.into_iter().collect(),
                    },
                )
                .collect(),
        };
        graph.validate()?;
        Ok(graph)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != ENTITY_KGE_SCHEMA_VERSION {
            return invalid(
                "entity_kge.schema_version",
                format!(
                    "expected {ENTITY_KGE_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_text("entity_kge.source_corpus_ref", &self.source_corpus_ref, 512)?;
        if self.entities.len() < 2 {
            return invalid(
                "entity_kge.entities",
                "graph must contain at least two entities",
            );
        }
        if self.triples.is_empty() {
            return invalid(
                "entity_kge.triples",
                "graph must contain at least one triple",
            );
        }
        ensure_sorted_unique("entity_kge.entities", &self.entities)?;
        let entity_set = self.entities.iter().cloned().collect::<BTreeSet<_>>();
        let mut triple_keys = BTreeSet::new();
        for triple in &self.triples {
            validate_text("entity_kge.triple.head_entity", &triple.head_entity, 512)?;
            validate_text("entity_kge.triple.tail_entity", &triple.tail_entity, 512)?;
            ensure_sorted_unique("entity_kge.triple.evidence_refs", &triple.evidence_refs)?;
            if !entity_set.contains(&triple.head_entity) {
                return invalid(
                    "entity_kge.triple.head_entity",
                    format!("{} is not in graph.entities", triple.head_entity),
                );
            }
            if !entity_set.contains(&triple.tail_entity) {
                return invalid(
                    "entity_kge.triple.tail_entity",
                    format!("{} is not in graph.entities", triple.tail_entity),
                );
            }
            if !triple_keys.insert((
                triple.head_entity.clone(),
                triple.relation,
                triple.tail_entity.clone(),
            )) {
                return invalid("entity_kge.triples", "duplicate triple key");
            }
        }
        Ok(())
    }

    pub fn audit(&self) -> Result<EntityDependencyGraphAudit, MejepaInferError> {
        self.validate()?;
        Ok(EntityDependencyGraphAudit {
            schema_version: ENTITY_KGE_SCHEMA_VERSION,
            source_corpus_ref: self.source_corpus_ref.clone(),
            entity_count: self.entities.len(),
            triple_count: self.triples.len(),
            relation_counts: relation_counts_from_triples(&self.triples),
            graph_sha256: sha256_json(self)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityDependencyGraphAudit {
    pub schema_version: u32,
    pub source_corpus_ref: String,
    pub entity_count: usize,
    pub triple_count: usize,
    pub relation_counts: BTreeMap<EntityKgeRelationKind, usize>,
    pub graph_sha256: String,
}

pub fn independent_dependency_graph_audit(
    source_corpus_ref: impl Into<String>,
    edges: &[EntityDependencyEdgeInput],
) -> Result<EntityDependencyGraphAudit, MejepaInferError> {
    let graph = EntityDependencyGraph::from_edges(source_corpus_ref, edges.to_vec())?;
    let mut entities = BTreeSet::new();
    let mut triple_keys =
        BTreeMap::<(String, EntityKgeRelationKind, String), BTreeSet<String>>::new();
    for edge in edges {
        validate_text("entity_kge.audit.head_entity", &edge.head_entity, 512)?;
        validate_text("entity_kge.audit.tail_entity", &edge.tail_entity, 512)?;
        validate_text("entity_kge.audit.evidence_ref", &edge.evidence_ref, 1024)?;
        entities.insert(edge.head_entity.clone());
        entities.insert(edge.tail_entity.clone());
        triple_keys
            .entry((
                edge.head_entity.clone(),
                edge.relation,
                edge.tail_entity.clone(),
            ))
            .or_default()
            .insert(edge.evidence_ref.clone());
    }
    let mut relation_counts = BTreeMap::<EntityKgeRelationKind, usize>::new();
    for (_, relation, _) in triple_keys.keys() {
        *relation_counts.entry(*relation).or_default() += 1;
    }
    let graph_sha256 = sha256_json(&graph)?;
    Ok(EntityDependencyGraphAudit {
        schema_version: ENTITY_KGE_SCHEMA_VERSION,
        source_corpus_ref: graph.source_corpus_ref,
        entity_count: entities.len(),
        triple_count: triple_keys.len(),
        relation_counts,
        graph_sha256,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKgeModelKind {
    TransE,
    RotatE,
}

impl EntityKgeModelKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TransE => "transe",
            Self::RotatE => "rotate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityKgeTrainingConfig {
    pub dimension: usize,
    pub epochs: usize,
    pub learning_rate: f32,
    pub margin: f32,
    pub seed: u64,
}

impl Default for EntityKgeTrainingConfig {
    fn default() -> Self {
        Self {
            dimension: ENTITY_KGE_OUTPUT_DIMENSION,
            epochs: 64,
            learning_rate: 0.16,
            margin: 1.0,
            seed: 0xE1_1E_55_u64,
        }
    }
}

impl EntityKgeTrainingConfig {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.dimension == 0 || self.dimension > 4096 || !self.dimension.is_multiple_of(2) {
            return invalid(
                "entity_kge.training.dimension",
                "dimension must be an even value in [2, 4096]",
            );
        }
        if self.epochs == 0 || self.epochs > 4096 {
            return invalid("entity_kge.training.epochs", "epochs must be in [1, 4096]");
        }
        validate_non_negative_finite("entity_kge.training.learning_rate", self.learning_rate)?;
        if self.learning_rate == 0.0 || self.learning_rate > 1.0 {
            return invalid(
                "entity_kge.training.learning_rate",
                "learning_rate must be in (0, 1]",
            );
        }
        validate_non_negative_finite("entity_kge.training.margin", self.margin)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityKgeModel {
    pub schema_version: u32,
    pub model_kind: EntityKgeModelKind,
    pub dimension: usize,
    pub graph_sha256: String,
    pub entity_vectors: BTreeMap<String, Vec<f32>>,
    pub relation_vectors: BTreeMap<EntityKgeRelationKind, Vec<f32>>,
}

impl EntityKgeModel {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != ENTITY_KGE_SCHEMA_VERSION {
            return invalid(
                "entity_kge_model.schema_version",
                format!(
                    "expected {ENTITY_KGE_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_text("entity_kge_model.graph_sha256", &self.graph_sha256, 64)?;
        if self.dimension == 0 || !self.dimension.is_multiple_of(2) {
            return invalid(
                "entity_kge_model.dimension",
                "dimension must be positive and even",
            );
        }
        if self.entity_vectors.is_empty() || self.relation_vectors.is_empty() {
            return invalid(
                "entity_kge_model.vectors",
                "entity and relation vectors must be non-empty",
            );
        }
        for (entity, vector) in &self.entity_vectors {
            validate_text("entity_kge_model.entity", entity, 512)?;
            validate_vector("entity_kge_model.entity_vector", vector, self.dimension)?;
        }
        for vector in self.relation_vectors.values() {
            validate_vector("entity_kge_model.relation_vector", vector, self.dimension)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityKgeAsymmetryEvidence {
    pub relation: EntityKgeRelationKind,
    pub head_entity: String,
    pub tail_entity: String,
    pub forward_relation_conditioned_cosine: f32,
    pub reverse_relation_conditioned_cosine: f32,
    pub absolute_delta: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityKgeTrainingReport {
    pub schema_version: u32,
    pub model_kind: EntityKgeModelKind,
    pub dimension: usize,
    pub epochs: usize,
    pub graph_sha256: String,
    pub initial_loss: f32,
    pub final_loss: f32,
    pub loss_reduction: f32,
    pub loss_history: Vec<f32>,
    pub converged: bool,
    pub entity_vector_sha256: String,
    pub relation_vector_sha256: String,
    pub directional_asymmetry: EntityKgeAsymmetryEvidence,
}

impl EntityKgeTrainingReport {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != ENTITY_KGE_SCHEMA_VERSION {
            return invalid(
                "entity_kge_report.schema_version",
                format!(
                    "expected {ENTITY_KGE_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        if !self.initial_loss.is_finite()
            || !self.final_loss.is_finite()
            || !self.loss_reduction.is_finite()
        {
            return invalid("entity_kge_report.loss", "loss values must be finite");
        }
        if self.loss_history.is_empty() || self.loss_history.iter().any(|value| !value.is_finite())
        {
            return invalid(
                "entity_kge_report.loss_history",
                "loss history must contain finite values",
            );
        }
        if !self.converged || self.final_loss >= self.initial_loss {
            return invalid(
                "entity_kge_report.converged",
                "training report must prove loss decreased",
            );
        }
        validate_sha256_hex(
            "entity_kge_report.entity_vector_sha256",
            &self.entity_vector_sha256,
        )?;
        validate_sha256_hex(
            "entity_kge_report.relation_vector_sha256",
            &self.relation_vector_sha256,
        )?;
        if !self.directional_asymmetry.absolute_delta.is_finite()
            || self.directional_asymmetry.absolute_delta <= 1e-5
        {
            return invalid(
                "entity_kge_report.directional_asymmetry",
                "relation-conditioned directional cosine delta must be positive",
            );
        }
        Ok(())
    }
}

pub fn train_entity_kge_model(
    graph: &EntityDependencyGraph,
    model_kind: EntityKgeModelKind,
    config: EntityKgeTrainingConfig,
) -> Result<(EntityKgeModel, EntityKgeTrainingReport), MejepaInferError> {
    graph.validate()?;
    config.validate()?;

    let graph_sha256 = graph.audit()?.graph_sha256;
    let entities = graph.entities.clone();
    let entity_index = entities
        .iter()
        .enumerate()
        .map(|(idx, entity)| (entity.clone(), idx))
        .collect::<BTreeMap<_, _>>();
    let relations = graph
        .triples
        .iter()
        .map(|triple| triple.relation)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let relation_index = relations
        .iter()
        .enumerate()
        .map(|(idx, relation)| (*relation, idx))
        .collect::<BTreeMap<_, _>>();
    let indexed_triples = graph
        .triples
        .iter()
        .map(|triple| {
            Ok(IndexedTriple {
                head: *entity_index.get(&triple.head_entity).ok_or_else(|| {
                    MejepaInferError::InvalidInput {
                        field: "entity_kge.index.head".to_string(),
                        detail: format!("missing {}", triple.head_entity),
                    }
                })?,
                relation: *relation_index.get(&triple.relation).ok_or_else(|| {
                    MejepaInferError::InvalidInput {
                        field: "entity_kge.index.relation".to_string(),
                        detail: format!("missing {}", triple.relation.as_str()),
                    }
                })?,
                tail: *entity_index.get(&triple.tail_entity).ok_or_else(|| {
                    MejepaInferError::InvalidInput {
                        field: "entity_kge.index.tail".to_string(),
                        detail: format!("missing {}", triple.tail_entity),
                    }
                })?,
            })
        })
        .collect::<Result<Vec<_>, MejepaInferError>>()?;

    let mut entity_vectors = entities
        .iter()
        .map(|entity| seeded_unit_vector("entity", entity, config.dimension, config.seed))
        .collect::<Vec<_>>();
    let mut loss_history = Vec::with_capacity(config.epochs + 1);

    let relation_vectors = match model_kind {
        EntityKgeModelKind::TransE => train_transe(
            &relations,
            &indexed_triples,
            &mut entity_vectors,
            config,
            &mut loss_history,
        ),
        EntityKgeModelKind::RotatE => train_rotate(
            &relations,
            &indexed_triples,
            &mut entity_vectors,
            config,
            &mut loss_history,
        ),
    };

    let entity_vectors = entities
        .into_iter()
        .zip(entity_vectors)
        .collect::<BTreeMap<_, _>>();
    let relation_vectors = relations
        .into_iter()
        .zip(relation_vectors)
        .collect::<BTreeMap<_, _>>();
    let model = EntityKgeModel {
        schema_version: ENTITY_KGE_SCHEMA_VERSION,
        model_kind,
        dimension: config.dimension,
        graph_sha256: graph_sha256.clone(),
        entity_vectors,
        relation_vectors,
    };
    model.validate()?;
    let initial_loss = *loss_history
        .first()
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "entity_kge.loss_history".to_string(),
            detail: "loss history empty".to_string(),
        })?;
    let final_loss = *loss_history
        .last()
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "entity_kge.loss_history".to_string(),
            detail: "loss history empty".to_string(),
        })?;
    let directional_asymmetry = strongest_directional_asymmetry(&model, graph)?;
    let report = EntityKgeTrainingReport {
        schema_version: ENTITY_KGE_SCHEMA_VERSION,
        model_kind,
        dimension: config.dimension,
        epochs: config.epochs,
        graph_sha256,
        initial_loss,
        final_loss,
        loss_reduction: initial_loss - final_loss,
        loss_history,
        converged: final_loss < initial_loss,
        entity_vector_sha256: sha256_json(&model.entity_vectors)?,
        relation_vector_sha256: sha256_json(&model.relation_vectors)?,
        directional_asymmetry,
    };
    report.validate()?;
    Ok((model, report))
}

pub fn relation_conditioned_cosine(
    model: &EntityKgeModel,
    head_entity: &str,
    relation: EntityKgeRelationKind,
    tail_entity: &str,
) -> Result<f32, MejepaInferError> {
    model.validate()?;
    let head = model
        .entity_vectors
        .get(head_entity)
        .ok_or_else(|| missing_vector("head_entity", head_entity))?;
    let tail = model
        .entity_vectors
        .get(tail_entity)
        .ok_or_else(|| missing_vector("tail_entity", tail_entity))?;
    let relation_vector = model
        .relation_vectors
        .get(&relation)
        .ok_or_else(|| missing_vector("relation", relation.as_str()))?;
    let transformed = match model.model_kind {
        EntityKgeModelKind::TransE => add_vectors(head, relation_vector),
        EntityKgeModelKind::RotatE => rotate_vector(head, relation_vector),
    };
    cosine(&transformed, tail)
}

pub fn strongest_directional_asymmetry(
    model: &EntityKgeModel,
    graph: &EntityDependencyGraph,
) -> Result<EntityKgeAsymmetryEvidence, MejepaInferError> {
    graph.validate()?;
    model.validate()?;
    let mut best: Option<EntityKgeAsymmetryEvidence> = None;
    for triple in &graph.triples {
        let forward = relation_conditioned_cosine(
            model,
            &triple.head_entity,
            triple.relation,
            &triple.tail_entity,
        )?;
        let reverse = relation_conditioned_cosine(
            model,
            &triple.tail_entity,
            triple.relation,
            &triple.head_entity,
        )?;
        let evidence = EntityKgeAsymmetryEvidence {
            relation: triple.relation,
            head_entity: triple.head_entity.clone(),
            tail_entity: triple.tail_entity.clone(),
            forward_relation_conditioned_cosine: forward,
            reverse_relation_conditioned_cosine: reverse,
            absolute_delta: (forward - reverse).abs(),
        };
        if best
            .as_ref()
            .map(|prior| evidence.absolute_delta > prior.absolute_delta)
            .unwrap_or(true)
        {
            best = Some(evidence);
        }
    }
    best.ok_or_else(|| MejepaInferError::InvalidInput {
        field: "entity_kge.directional_asymmetry".to_string(),
        detail: "graph has no triples".to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityKgeForwardArtifact<'a> {
    pub schema_version: u32,
    pub runtime_embedder_id_slug: String,
    pub graph: &'a EntityDependencyGraph,
    pub graph_audit: EntityDependencyGraphAudit,
    pub model: &'a EntityKgeModel,
    pub training_report: &'a EntityKgeTrainingReport,
}

pub fn entity_kge_forward_artifact_bytes(
    runtime_embedder_id_slug: impl Into<String>,
    graph: &EntityDependencyGraph,
    model: &EntityKgeModel,
    training_report: &EntityKgeTrainingReport,
) -> Result<Vec<u8>, MejepaInferError> {
    let artifact = EntityKgeForwardArtifact {
        schema_version: ENTITY_KGE_SCHEMA_VERSION,
        runtime_embedder_id_slug: runtime_embedder_id_slug.into(),
        graph,
        graph_audit: graph.audit()?,
        model,
        training_report,
    };
    Ok(serde_json::to_vec_pretty(&artifact)?)
}

#[derive(Clone, Copy)]
struct IndexedTriple {
    head: usize,
    relation: usize,
    tail: usize,
}

fn train_transe(
    relations: &[EntityKgeRelationKind],
    triples: &[IndexedTriple],
    entities: &mut [Vec<f32>],
    config: EntityKgeTrainingConfig,
    loss_history: &mut Vec<f32>,
) -> Vec<Vec<f32>> {
    let mut relation_vectors = relations
        .iter()
        .map(|relation| {
            let mut vector = seeded_unit_vector(
                "transe-relation",
                relation.as_str(),
                config.dimension,
                config.seed,
            );
            scale_vector(&mut vector, 0.05);
            vector
        })
        .collect::<Vec<_>>();
    loss_history.push(transe_loss(entities, &relation_vectors, triples));
    for _ in 0..config.epochs {
        let mut sums = vec![vec![0.0f32; config.dimension]; relation_vectors.len()];
        let mut counts = vec![0usize; relation_vectors.len()];
        for triple in triples {
            for dim in 0..config.dimension {
                sums[triple.relation][dim] +=
                    entities[triple.tail][dim] - entities[triple.head][dim];
            }
            counts[triple.relation] += 1;
        }
        for (idx, relation_vector) in relation_vectors.iter_mut().enumerate() {
            let count = counts[idx].max(1) as f32;
            for (dim, value) in relation_vector.iter_mut().enumerate() {
                let target = sums[idx][dim] / count;
                *value = (*value * (1.0 - config.learning_rate)) + (target * config.learning_rate);
            }
        }
        for triple in triples {
            let head = entities[triple.head].clone();
            let tail = entities[triple.tail].clone();
            let relation = &relation_vectors[triple.relation];
            for dim in 0..config.dimension {
                let error = head[dim] + relation[dim] - tail[dim];
                entities[triple.head][dim] -= config.learning_rate * 0.03 * error;
                entities[triple.tail][dim] += config.learning_rate * 0.03 * error;
            }
        }
        loss_history.push(transe_loss(entities, &relation_vectors, triples));
    }
    relation_vectors
}

fn train_rotate(
    relations: &[EntityKgeRelationKind],
    triples: &[IndexedTriple],
    entities: &mut [Vec<f32>],
    config: EntityKgeTrainingConfig,
    loss_history: &mut Vec<f32>,
) -> Vec<Vec<f32>> {
    for vector in entities.iter_mut() {
        normalize_complex_pairs(vector);
    }
    let pair_count = config.dimension / 2;
    let mut phases = relations
        .iter()
        .map(|relation| seeded_phase_vector(relation.as_str(), pair_count, config.seed))
        .collect::<Vec<_>>();
    let mut relation_vectors = phases_to_relation_vectors(&phases);
    loss_history.push(rotate_loss(entities, &relation_vectors, triples));
    for _ in 0..config.epochs {
        let mut sin_sums = vec![vec![0.0f32; pair_count]; phases.len()];
        let mut cos_sums = vec![vec![0.0f32; pair_count]; phases.len()];
        let mut counts = vec![0usize; phases.len()];
        for triple in triples {
            for pair in 0..pair_count {
                let head_angle =
                    entities[triple.head][pair * 2 + 1].atan2(entities[triple.head][pair * 2]);
                let tail_angle =
                    entities[triple.tail][pair * 2 + 1].atan2(entities[triple.tail][pair * 2]);
                let diff = tail_angle - head_angle;
                sin_sums[triple.relation][pair] += diff.sin();
                cos_sums[triple.relation][pair] += diff.cos();
            }
            counts[triple.relation] += 1;
        }
        for relation_idx in 0..phases.len() {
            if counts[relation_idx] == 0 {
                continue;
            }
            for pair in 0..pair_count {
                let target = sin_sums[relation_idx][pair].atan2(cos_sums[relation_idx][pair]);
                phases[relation_idx][pair] =
                    blend_angle(phases[relation_idx][pair], target, config.learning_rate);
            }
        }
        relation_vectors = phases_to_relation_vectors(&phases);
        for triple in triples {
            let head = entities[triple.head].clone();
            let tail = entities[triple.tail].clone();
            let rotated_head = rotate_vector(&head, &relation_vectors[triple.relation]);
            let inverse_tail = inverse_rotate_vector(&tail, &relation_vectors[triple.relation]);
            for dim in 0..config.dimension {
                entities[triple.tail][dim] +=
                    config.learning_rate * 0.06 * (rotated_head[dim] - tail[dim]);
                entities[triple.head][dim] +=
                    config.learning_rate * 0.03 * (inverse_tail[dim] - head[dim]);
            }
            normalize_complex_pairs(&mut entities[triple.head]);
            normalize_complex_pairs(&mut entities[triple.tail]);
        }
        loss_history.push(rotate_loss(entities, &relation_vectors, triples));
    }
    relation_vectors
}

fn transe_loss(entities: &[Vec<f32>], relations: &[Vec<f32>], triples: &[IndexedTriple]) -> f32 {
    let mut total = 0.0f32;
    for triple in triples {
        let head = &entities[triple.head];
        let relation = &relations[triple.relation];
        let tail = &entities[triple.tail];
        let mut sum = 0.0f32;
        for dim in 0..head.len() {
            let error = head[dim] + relation[dim] - tail[dim];
            sum += error * error;
        }
        total += sum / head.len() as f32;
    }
    total / triples.len().max(1) as f32
}

fn rotate_loss(entities: &[Vec<f32>], relations: &[Vec<f32>], triples: &[IndexedTriple]) -> f32 {
    let mut total = 0.0f32;
    for triple in triples {
        let rotated = rotate_vector(&entities[triple.head], &relations[triple.relation]);
        let tail = &entities[triple.tail];
        let mut sum = 0.0f32;
        for dim in 0..rotated.len() {
            let error = rotated[dim] - tail[dim];
            sum += error * error;
        }
        total += sum / rotated.len() as f32;
    }
    total / triples.len().max(1) as f32
}

fn seeded_unit_vector(kind: &str, label: &str, dimension: usize, seed: u64) -> Vec<f32> {
    let mut vector = (0..dimension)
        .map(|dim| {
            let mut hasher = Sha256::new();
            hasher.update(kind.as_bytes());
            hasher.update(label.as_bytes());
            hasher.update(seed.to_be_bytes());
            hasher.update((dim as u64).to_be_bytes());
            let digest = hasher.finalize();
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&digest[..8]);
            let raw = u64::from_be_bytes(bytes);
            let unit = (raw as f64 / u64::MAX as f64) as f32;
            (unit * 2.0) - 1.0
        })
        .collect::<Vec<_>>();
    normalize_vector(&mut vector);
    vector
}

fn seeded_phase_vector(label: &str, pair_count: usize, seed: u64) -> Vec<f32> {
    (0..pair_count)
        .map(|idx| {
            let mut hasher = Sha256::new();
            hasher.update(b"rotate-phase");
            hasher.update(label.as_bytes());
            hasher.update(seed.to_be_bytes());
            hasher.update((idx as u64).to_be_bytes());
            let digest = hasher.finalize();
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&digest[..8]);
            let raw = u64::from_be_bytes(bytes);
            let unit = (raw as f64 / u64::MAX as f64) as f32;
            (unit - 0.5) * std::f32::consts::FRAC_PI_2
        })
        .collect()
}

fn phases_to_relation_vectors(phases: &[Vec<f32>]) -> Vec<Vec<f32>> {
    phases
        .iter()
        .map(|phase_vector| {
            let mut out = Vec::with_capacity(phase_vector.len() * 2);
            for phase in phase_vector {
                out.push(phase.cos());
                out.push(phase.sin());
            }
            out
        })
        .collect()
}

fn relation_counts_from_triples(
    triples: &[EntityKgeTriple],
) -> BTreeMap<EntityKgeRelationKind, usize> {
    let mut counts = BTreeMap::new();
    for triple in triples {
        *counts.entry(triple.relation).or_default() += 1;
    }
    counts
}

fn rotate_vector(vector: &[f32], relation_vector: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(vector.len());
    for pair in 0..(vector.len() / 2) {
        let re = vector[pair * 2];
        let im = vector[pair * 2 + 1];
        let cos = relation_vector[pair * 2];
        let sin = relation_vector[pair * 2 + 1];
        out.push((re * cos) - (im * sin));
        out.push((re * sin) + (im * cos));
    }
    out
}

fn inverse_rotate_vector(vector: &[f32], relation_vector: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(vector.len());
    for pair in 0..(vector.len() / 2) {
        let re = vector[pair * 2];
        let im = vector[pair * 2 + 1];
        let cos = relation_vector[pair * 2];
        let sin = -relation_vector[pair * 2 + 1];
        out.push((re * cos) - (im * sin));
        out.push((re * sin) + (im * cos));
    }
    out
}

fn add_vectors(left: &[f32], right: &[f32]) -> Vec<f32> {
    left.iter()
        .zip(right)
        .map(|(left, right)| left + right)
        .collect()
}

fn cosine(left: &[f32], right: &[f32]) -> Result<f32, MejepaInferError> {
    if left.len() != right.len() || left.is_empty() {
        return invalid(
            "entity_kge.cosine",
            "vectors must have matching non-zero dimensions",
        );
    }
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for (left, right) in left.iter().zip(right) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        return invalid("entity_kge.cosine", "zero vector encountered");
    }
    Ok(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

fn normalize_vector(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector {
            *value /= norm;
        }
    }
}

fn normalize_complex_pairs(vector: &mut [f32]) {
    for pair in 0..(vector.len() / 2) {
        let re_idx = pair * 2;
        let im_idx = re_idx + 1;
        let norm = (vector[re_idx] * vector[re_idx] + vector[im_idx] * vector[im_idx]).sqrt();
        if norm > f32::EPSILON {
            vector[re_idx] /= norm;
            vector[im_idx] /= norm;
        }
    }
}

fn scale_vector(vector: &mut [f32], scalar: f32) {
    for value in vector {
        *value *= scalar;
    }
}

fn blend_angle(current: f32, target: f32, alpha: f32) -> f32 {
    let sin = (current.sin() * (1.0 - alpha)) + (target.sin() * alpha);
    let cos = (current.cos() * (1.0 - alpha)) + (target.cos() * alpha);
    sin.atan2(cos)
}

fn validate_vector(
    field: &str,
    vector: &[f32],
    expected_len: usize,
) -> Result<(), MejepaInferError> {
    if vector.len() != expected_len {
        return invalid(
            field,
            format!("expected vector len {expected_len}, got {}", vector.len()),
        );
    }
    if vector.iter().any(|value| !value.is_finite()) {
        return invalid(field, "vector contains non-finite values");
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > max_len
        || value.bytes().any(|byte| byte == 0 || byte == b'\n')
    {
        return invalid(
            field,
            format!("must be non-empty trimmed single-line text up to {max_len} bytes"),
        );
    }
    Ok(())
}

fn validate_non_negative_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || value < 0.0 {
        return invalid(field, "value must be finite and non-negative");
    }
    Ok(())
}

fn validate_sha256_hex(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return invalid(field, "value must be 64 lowercase hex characters");
    }
    Ok(())
}

fn ensure_sorted_unique(field: &str, values: &[String]) -> Result<(), MejepaInferError> {
    if values.is_empty() {
        return invalid(field, "list must be non-empty");
    }
    let mut prior: Option<&str> = None;
    for value in values {
        validate_text(field, value, 1024)?;
        if prior.map(|prior| prior >= value.as_str()).unwrap_or(false) {
            return invalid(field, "list must be sorted and unique");
        }
        prior = Some(value);
    }
    Ok(())
}

fn missing_vector(field: &str, value: &str) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: format!("entity_kge.{field}"),
        detail: format!("missing vector for {value}"),
    }
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String, MejepaInferError> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_audit_matches_independent_recount() {
        let edges = fixture_edges();
        let graph =
            EntityDependencyGraph::from_edges("fixture:python-entity-graph-v1", edges.clone())
                .unwrap();
        let audit = graph.audit().unwrap();
        let independent =
            independent_dependency_graph_audit("fixture:python-entity-graph-v1", &edges).unwrap();

        assert_eq!(audit, independent);
        assert_eq!(audit.entity_count, 10);
        assert_eq!(audit.triple_count, 10);
        assert_eq!(
            audit
                .relation_counts
                .get(&EntityKgeRelationKind::Calls)
                .copied(),
            Some(2)
        );
    }

    #[test]
    fn rotate_training_converges_and_keeps_directional_signal() {
        let graph =
            EntityDependencyGraph::from_edges("fixture:python-entity-graph-v1", fixture_edges())
                .unwrap();
        let (_model, report) = train_entity_kge_model(
            &graph,
            EntityKgeModelKind::RotatE,
            EntityKgeTrainingConfig::default(),
        )
        .unwrap();

        assert!(report.converged);
        assert!(report.final_loss < report.initial_loss);
        assert!(report.directional_asymmetry.absolute_delta > 1e-5);
        assert_eq!(report.dimension, ENTITY_KGE_OUTPUT_DIMENSION);
    }

    #[test]
    fn rejects_unsorted_corrupt_graph() {
        let graph = EntityDependencyGraph {
            schema_version: ENTITY_KGE_SCHEMA_VERSION,
            source_corpus_ref: "fixture".to_string(),
            entities: vec!["b".to_string(), "a".to_string()],
            triples: vec![EntityKgeTriple {
                head_entity: "a".to_string(),
                relation: EntityKgeRelationKind::Calls,
                tail_entity: "b".to_string(),
                evidence_refs: vec!["evidence".to_string()],
            }],
        };
        assert_eq!(
            graph.validate().unwrap_err().code(),
            "MEJEPA_INFER_INVALID_INPUT"
        );
    }

    fn fixture_edges() -> Vec<EntityDependencyEdgeInput> {
        vec![
            edge(
                "module:app.api",
                EntityKgeRelationKind::Imports,
                "module:app.db",
                "ast.import:app/api.py:1",
            ),
            edge(
                "module:app.api",
                EntityKgeRelationKind::Imports,
                "module:app.models",
                "ast.import:app/api.py:2",
            ),
            edge(
                "function:app.api.handle_request",
                EntityKgeRelationKind::Calls,
                "function:app.db.fetch_user",
                "call:app/api.py:42",
            ),
            edge(
                "function:app.api.handle_request",
                EntityKgeRelationKind::Calls,
                "function:app.models.serialize_user",
                "call:app/api.py:44",
            ),
            edge(
                "function:app.api.handle_request",
                EntityKgeRelationKind::HasType,
                "class:app.models.User",
                "type:app/api.py:39",
            ),
            edge(
                "class:app.models.AdminUser",
                EntityKgeRelationKind::Inherits,
                "class:app.models.User",
                "class:app/models.py:18",
            ),
            edge(
                "test:tests.test_api.test_handle_request",
                EntityKgeRelationKind::TestVerifies,
                "function:app.api.handle_request",
                "pytest:tests/test_api.py:9",
            ),
            edge(
                "patch:fix-null-user",
                EntityKgeRelationKind::PatchModifies,
                "function:app.api.handle_request",
                "diff:0001",
            ),
            edge(
                "patch:fix-null-user",
                EntityKgeRelationKind::PatchModifies,
                "test:tests.test_api.test_handle_request",
                "diff:0002",
            ),
            edge(
                "function:app.models.serialize_user",
                EntityKgeRelationKind::HasType,
                "class:app.models.User",
                "type:app/models.py:35",
            ),
        ]
    }

    fn edge(
        head_entity: &str,
        relation: EntityKgeRelationKind,
        tail_entity: &str,
        evidence_ref: &str,
    ) -> EntityDependencyEdgeInput {
        EntityDependencyEdgeInput {
            head_entity: head_entity.to_string(),
            relation,
            tail_entity: tail_entity.to_string(),
            evidence_ref: evidence_ref.to_string(),
        }
    }
}
