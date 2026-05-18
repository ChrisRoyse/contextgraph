use std::collections::{BTreeMap, BTreeSet};

use context_graph_mejepa_cf::CF_MEJEPA_NORTH_STARS;
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::types::EmbedderId;

pub const NORTH_STAR_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_INTENT_VECTOR_DIM: usize = 16;
pub const MAX_INTENT_VECTOR_DIM: usize = 256;
pub const DEFAULT_INTENT_TAU_MARGIN: f32 = 0.05;
pub const INTENT_OVERLAP_COSINE_THRESHOLD: f32 = 0.95;
pub const INTENT_LOW_DIVERSITY_COSINE_THRESHOLD: f32 = 0.999;
pub const INTENT_DEFAULT_PANEL_ID: &str = "mejepa-intent-panel-v1";
pub const INTENT_DEFAULT_CORPUS_SHARD_HASH: &str = "deterministic-intent-hash-v1";
pub const MEJEPA_INTENT_EMPTY_POSITIVES: &str = "MEJEPA_INTENT_EMPTY_POSITIVES";
pub const MEJEPA_INTENT_NEGATIVES_OVERLAP_POSITIVES: &str =
    "MEJEPA_INTENT_NEGATIVES_OVERLAP_POSITIVES";
pub const MEJEPA_INTENT_DRIFT_DURING_REFINEMENT: &str = "MEJEPA_INTENT_DRIFT_DURING_REFINEMENT";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawIntentInput {
    pub input_id: String,
    pub text: String,
}

impl RawIntentInput {
    pub fn try_new(
        input_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<Self, MejepaInferError> {
        let input = Self {
            input_id: input_id.into(),
            text: text.into(),
        };
        input.validate("raw_input")?;
        Ok(input)
    }

    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_bounded_text(&format!("{field}.input_id"), &self.input_id, 256)?;
        validate_bounded_text(&format!("{field}.text"), &self.text, 65_536)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InteractiveRefinementRound {
    pub round_id: String,
    pub critique: String,
    #[serde(default)]
    pub accepted: bool,
}

impl InteractiveRefinementRound {
    pub fn validate(&self, idx: usize) -> Result<(), MejepaInferError> {
        validate_bounded_text(
            &format!("interactive.rounds[{idx}].round_id"),
            &self.round_id,
            256,
        )?;
        validate_bounded_text(
            &format!("interactive.rounds[{idx}].critique"),
            &self.critique,
            65_536,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum IntentSpecification {
    FreeText {
        description: String,
    },
    PositiveAndNegativeExemplars {
        positive: Vec<RawIntentInput>,
        #[serde(default)]
        negative: Vec<RawIntentInput>,
    },
    ConstellationOfOutcome {
        subject: String,
        references: Vec<RawIntentInput>,
    },
    InteractiveRefinement {
        initial: Box<IntentSpecification>,
        #[serde(default)]
        rounds: Vec<InteractiveRefinementRound>,
        max_rounds: u32,
    },
}

impl IntentSpecification {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.validate_with_depth(0)
    }

    fn validate_with_depth(&self, depth: usize) -> Result<(), MejepaInferError> {
        if depth > 8 {
            return Err(invalid(
                "intent_specification",
                "interactive refinement nesting must be <= 8",
            ));
        }
        match self {
            Self::FreeText { description } => {
                validate_bounded_text("description", description, 65_536)?;
            }
            Self::PositiveAndNegativeExemplars { positive, negative } => {
                if positive.is_empty() {
                    return Err(invalid(
                        "positive",
                        format!("{MEJEPA_INTENT_EMPTY_POSITIVES}: positive exemplars required"),
                    ));
                }
                validate_inputs("positive", positive)?;
                validate_inputs("negative", negative)?;
            }
            Self::ConstellationOfOutcome {
                subject,
                references,
            } => {
                validate_bounded_text("subject", subject, 1024)?;
                if references.is_empty() {
                    return Err(invalid(
                        "references",
                        format!("{MEJEPA_INTENT_EMPTY_POSITIVES}: outcome references required"),
                    ));
                }
                validate_inputs("references", references)?;
            }
            Self::InteractiveRefinement {
                initial,
                rounds,
                max_rounds,
            } => {
                initial.validate_with_depth(depth + 1)?;
                if *max_rounds == 0 || *max_rounds > 64 {
                    return Err(invalid(
                        "max_rounds",
                        format!("max_rounds must be in [1, 64], got {max_rounds}"),
                    ));
                }
                if rounds.len() > *max_rounds as usize {
                    return Err(invalid(
                        "rounds",
                        format!(
                            "round count {} exceeds max_rounds {}",
                            rounds.len(),
                            max_rounds
                        ),
                    ));
                }
                for (idx, round) in rounds.iter().enumerate() {
                    round.validate(idx)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElicitationOptions {
    #[serde(default = "default_panel_id")]
    pub panel_id: String,
    #[serde(default = "default_corpus_shard_hash")]
    pub corpus_shard_hash: String,
    #[serde(default = "default_embedder_ids")]
    pub embedder_ids: Vec<EmbedderId>,
    #[serde(default = "default_vector_dim")]
    pub vector_dim: usize,
    #[serde(default = "default_tau_margin")]
    pub tau_margin: f32,
    #[serde(default = "default_convergence_cosine_threshold")]
    pub convergence_cosine_threshold: f32,
    #[serde(default = "default_drift_rollback_cosine_floor")]
    pub drift_rollback_cosine_floor: f32,
}

impl Default for ElicitationOptions {
    fn default() -> Self {
        Self {
            panel_id: default_panel_id(),
            corpus_shard_hash: default_corpus_shard_hash(),
            embedder_ids: default_embedder_ids(),
            vector_dim: default_vector_dim(),
            tau_margin: default_tau_margin(),
            convergence_cosine_threshold: default_convergence_cosine_threshold(),
            drift_rollback_cosine_floor: default_drift_rollback_cosine_floor(),
        }
    }
}

impl ElicitationOptions {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_bounded_text("panel_id", &self.panel_id, 512)?;
        validate_bounded_text("corpus_shard_hash", &self.corpus_shard_hash, 512)?;
        if self.embedder_ids.is_empty() {
            return Err(invalid("embedder_ids", "at least one embedder is required"));
        }
        let mut seen = BTreeSet::new();
        for embedder_id in &self.embedder_ids {
            validate_bounded_text("embedder_id", &embedder_id.0, 128)?;
            if !seen.insert(embedder_id.clone()) {
                return Err(invalid(
                    "embedder_ids",
                    format!("duplicate embedder id {}", embedder_id.0),
                ));
            }
        }
        if !(1..=MAX_INTENT_VECTOR_DIM).contains(&self.vector_dim) {
            return Err(invalid(
                "vector_dim",
                format!(
                    "vector_dim must be in [1, {MAX_INTENT_VECTOR_DIM}], got {}",
                    self.vector_dim
                ),
            ));
        }
        validate_unit_interval("tau_margin", self.tau_margin)?;
        if self.tau_margin >= 0.5 {
            return Err(invalid(
                "tau_margin",
                format!("tau_margin must be < 0.5, got {}", self.tau_margin),
            ));
        }
        validate_unit_interval(
            "convergence_cosine_threshold",
            self.convergence_cosine_threshold,
        )?;
        if !self.drift_rollback_cosine_floor.is_finite()
            || !(-1.0..=1.0).contains(&self.drift_rollback_cosine_floor)
        {
            return Err(invalid(
                "drift_rollback_cosine_floor",
                format!(
                    "drift_rollback_cosine_floor must be finite and in [-1, 1], got {}",
                    self.drift_rollback_cosine_floor
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentElicitationMode {
    FreeText,
    PositiveAndNegativeExemplars,
    ConstellationOfOutcome,
    InteractiveRefinement,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElicitationTrail {
    pub mode: IntentElicitationMode,
    pub input_count: usize,
    pub positive_count: usize,
    pub negative_count: usize,
    pub accepted_round_count: usize,
    pub max_rounds: Option<u32>,
    pub input_sha256: String,
    pub source_input_ids: Vec<String>,
    pub citations: Vec<String>,
    pub warning_codes: Vec<String>,
}

impl ElicitationTrail {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.input_count == 0 {
            return Err(invalid("provenance.input_count", "must be non-zero"));
        }
        if self.input_sha256.len() != 64
            || !self.input_sha256.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(invalid(
                "provenance.input_sha256",
                "input_sha256 must be 64 hex chars",
            ));
        }
        validate_inputs_ids("provenance.source_input_ids", &self.source_input_ids)?;
        for code in &self.warning_codes {
            validate_bounded_text("provenance.warning_codes", code, 128)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NorthStarVector {
    pub schema_version: u32,
    pub intent_id: String,
    pub created_at_unix_ms: i64,
    pub panel_id: String,
    pub corpus_shard_hash: String,
    pub c_intent: BTreeMap<EmbedderId, Vec<f32>>,
    pub tau_intent: BTreeMap<EmbedderId, f32>,
    pub intent_latent: Vec<f32>,
    pub provenance: ElicitationTrail,
    pub low_diversity_warning: bool,
    pub unconverged: bool,
    pub drift_warning_code: Option<String>,
    pub source_of_truth_cf: String,
}

impl NorthStarVector {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != NORTH_STAR_SCHEMA_VERSION {
            return Err(invalid(
                "schema_version",
                format!(
                    "expected schema_version {NORTH_STAR_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            ));
        }
        validate_hex_id("intent_id", &self.intent_id, 32)?;
        if self.created_at_unix_ms <= 0 {
            return Err(invalid(
                "created_at_unix_ms",
                format!("must be positive, got {}", self.created_at_unix_ms),
            ));
        }
        validate_bounded_text("panel_id", &self.panel_id, 512)?;
        validate_bounded_text("corpus_shard_hash", &self.corpus_shard_hash, 512)?;
        if self.c_intent.is_empty() {
            return Err(invalid(
                "c_intent",
                "at least one intent centroid is required",
            ));
        }
        if self.c_intent.keys().ne(self.tau_intent.keys()) {
            return Err(invalid(
                "tau_intent",
                "tau_intent keys must match c_intent keys",
            ));
        }
        for (embedder_id, vector) in &self.c_intent {
            validate_bounded_text("c_intent.embedder_id", &embedder_id.0, 128)?;
            validate_unit_vector(&format!("c_intent.{}", embedder_id.0), vector)?;
            let tau = self
                .tau_intent
                .get(embedder_id)
                .ok_or_else(|| invalid("tau_intent", "missing tau for embedder"))?;
            validate_unit_interval(&format!("tau_intent.{}", embedder_id.0), *tau)?;
        }
        validate_unit_vector("intent_latent", &self.intent_latent)?;
        self.provenance.validate()?;
        if let Some(code) = &self.drift_warning_code {
            validate_bounded_text("drift_warning_code", code, 128)?;
        }
        if self.source_of_truth_cf != CF_MEJEPA_NORTH_STARS {
            return Err(invalid(
                "source_of_truth_cf",
                format!("expected {CF_MEJEPA_NORTH_STARS}"),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum IntentCandidate {
    RawInput {
        candidate_id: String,
        text: String,
    },
    VectorMap {
        candidate_id: String,
        vectors: BTreeMap<EmbedderId, Vec<f32>>,
    },
}

impl IntentCandidate {
    pub fn candidate_id(&self) -> &str {
        match self {
            Self::RawInput { candidate_id, .. } | Self::VectorMap { candidate_id, .. } => {
                candidate_id
            }
        }
    }

    pub fn validate_against(&self, north_star: &NorthStarVector) -> Result<(), MejepaInferError> {
        validate_bounded_text("candidate_id", self.candidate_id(), 256)?;
        match self {
            Self::RawInput { text, .. } => validate_bounded_text("candidate.text", text, 65_536)?,
            Self::VectorMap { vectors, .. } => {
                if vectors.keys().ne(north_star.c_intent.keys()) {
                    return Err(invalid(
                        "candidate.vectors",
                        "candidate vector keys must match north-star embedders",
                    ));
                }
                for (embedder_id, vector) in vectors {
                    let expected = north_star
                        .c_intent
                        .get(embedder_id)
                        .map(|value| value.len())
                        .ok_or_else(|| invalid("candidate.vectors", "unexpected embedder id"))?;
                    if vector.len() != expected {
                        return Err(MejepaInferError::DimMismatch {
                            expected,
                            actual: vector.len(),
                            context: format!(
                                "candidate vector dim mismatch for embedder {}",
                                embedder_id.0
                            ),
                        });
                    }
                    validate_unit_vector(&format!("candidate.vectors.{}", embedder_id.0), vector)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntentEmbedderAlignment {
    pub embedder_id: EmbedderId,
    pub cosine: f32,
    pub alignment_score: f32,
    pub tau: f32,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlignmentReport {
    pub intent_id: String,
    pub candidate_id: String,
    pub passed: bool,
    pub mean_cosine: f32,
    pub mean_alignment_score: f32,
    pub violating_embedders: Vec<EmbedderId>,
    pub per_embedder: Vec<IntentEmbedderAlignment>,
    pub source_of_truth_cf: String,
}

pub fn elicit_intent(
    spec: IntentSpecification,
    options: ElicitationOptions,
    created_at_unix_ms: i64,
) -> Result<NorthStarVector, MejepaInferError> {
    spec.validate()?;
    options.validate()?;
    if created_at_unix_ms <= 0 {
        return Err(invalid(
            "created_at_unix_ms",
            format!("must be positive, got {created_at_unix_ms}"),
        ));
    }
    let built = build_intent_geometry(&spec, &options, 0)?;
    let intent_latent = mean_vector(built.c_intent.values()).ok_or_else(|| {
        invalid(
            "c_intent",
            "could not derive aggregate intent latent from embedder vectors",
        )
    })?;
    let mut id_hasher = Sha256::new();
    id_hasher.update(serde_json::to_vec(&spec)?);
    id_hasher.update(serde_json::to_vec(&options)?);
    id_hasher.update(serde_json::to_vec(&built.c_intent)?);
    id_hasher.update(created_at_unix_ms.to_be_bytes());
    let digest = id_hasher.finalize();
    let intent_id = hex::encode(&digest[..16]);
    let north_star = NorthStarVector {
        schema_version: NORTH_STAR_SCHEMA_VERSION,
        intent_id,
        created_at_unix_ms,
        panel_id: options.panel_id,
        corpus_shard_hash: options.corpus_shard_hash,
        c_intent: built.c_intent,
        tau_intent: built.tau_intent,
        intent_latent,
        provenance: built.provenance,
        low_diversity_warning: built.low_diversity_warning,
        unconverged: built.unconverged,
        drift_warning_code: built.drift_warning_code,
        source_of_truth_cf: CF_MEJEPA_NORTH_STARS.to_string(),
    };
    north_star.validate()?;
    Ok(north_star)
}

pub fn align_to_intent(
    north_star: &NorthStarVector,
    candidate: IntentCandidate,
) -> Result<AlignmentReport, MejepaInferError> {
    north_star.validate()?;
    candidate.validate_against(north_star)?;
    let mut per_embedder = Vec::with_capacity(north_star.c_intent.len());
    let mut violating_embedders = Vec::new();
    for (embedder_id, intent_vector) in &north_star.c_intent {
        let candidate_vector = match &candidate {
            IntentCandidate::RawInput { text, .. } => {
                hashed_unit_vector(embedder_id, text, intent_vector.len())?
            }
            IntentCandidate::VectorMap { vectors, .. } => vectors
                .get(embedder_id)
                .cloned()
                .ok_or_else(|| invalid("candidate.vectors", "missing embedder vector"))?,
        };
        let cosine = cosine(intent_vector, &candidate_vector)?;
        let alignment_score = ((cosine + 1.0) / 2.0).clamp(0.0, 1.0);
        let tau = *north_star
            .tau_intent
            .get(embedder_id)
            .ok_or_else(|| invalid("tau_intent", "missing tau"))?;
        let passed = cosine >= tau;
        if !passed {
            violating_embedders.push(embedder_id.clone());
        }
        per_embedder.push(IntentEmbedderAlignment {
            embedder_id: embedder_id.clone(),
            cosine,
            alignment_score,
            tau,
            passed,
        });
    }
    let mean_cosine = mean(per_embedder.iter().map(|row| row.cosine)).ok_or_else(|| {
        invalid(
            "per_embedder",
            "at least one per-embedder alignment row is required",
        )
    })?;
    let mean_alignment_score = mean(per_embedder.iter().map(|row| row.alignment_score))
        .ok_or_else(|| {
            invalid(
                "per_embedder",
                "at least one per-embedder alignment row is required",
            )
        })?;
    Ok(AlignmentReport {
        intent_id: north_star.intent_id.clone(),
        candidate_id: candidate.candidate_id().to_string(),
        passed: violating_embedders.is_empty(),
        mean_cosine,
        mean_alignment_score,
        violating_embedders,
        per_embedder,
        source_of_truth_cf: CF_MEJEPA_NORTH_STARS.to_string(),
    })
}

pub fn write_north_star_sync_readback(
    db: &DB,
    north_star: &NorthStarVector,
) -> Result<(), MejepaInferError> {
    north_star.validate()?;
    let cf = cf(db, CF_MEJEPA_NORTH_STARS)?;
    let value = bincode::serialize(north_star)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, north_star.intent_id.as_bytes(), &value, &opts)?;
    let readback = db
        .get_cf(cf, north_star.intent_id.as_bytes())?
        .ok_or_else(|| invalid("north_star", "read-after-write could not find row"))?;
    if readback != value {
        return Err(invalid(
            "north_star",
            "read-after-write bytes differ from persisted north-star row",
        ));
    }
    let decoded: NorthStarVector = bincode::deserialize(&readback)?;
    decoded.validate()?;
    if decoded != *north_star {
        return Err(invalid(
            "north_star",
            "read-after-write decoded north-star does not match input",
        ));
    }
    Ok(())
}

pub fn read_north_star(
    db: &DB,
    intent_id: &str,
) -> Result<Option<NorthStarVector>, MejepaInferError> {
    validate_hex_id("intent_id", intent_id, 32)?;
    let cf = cf(db, CF_MEJEPA_NORTH_STARS)?;
    let Some(bytes) = db.get_cf(cf, intent_id.as_bytes())? else {
        return Ok(None);
    };
    let row: NorthStarVector = bincode::deserialize(&bytes)?;
    row.validate()?;
    if row.intent_id != intent_id {
        return Err(invalid(
            "intent_id",
            "north-star payload intent_id does not match key",
        ));
    }
    Ok(Some(row))
}

struct BuiltIntent {
    c_intent: BTreeMap<EmbedderId, Vec<f32>>,
    tau_intent: BTreeMap<EmbedderId, f32>,
    provenance: ElicitationTrail,
    low_diversity_warning: bool,
    unconverged: bool,
    drift_warning_code: Option<String>,
}

fn build_intent_geometry(
    spec: &IntentSpecification,
    options: &ElicitationOptions,
    depth: usize,
) -> Result<BuiltIntent, MejepaInferError> {
    if depth > 8 {
        return Err(invalid(
            "intent_specification",
            "interactive refinement nesting must be <= 8",
        ));
    }
    match spec {
        IntentSpecification::FreeText { description } => {
            let input = RawIntentInput::try_new("free-text", description.clone())?;
            build_from_positive_negative(
                IntentElicitationMode::FreeText,
                &[input],
                &[],
                None,
                &[],
                options,
            )
        }
        IntentSpecification::PositiveAndNegativeExemplars { positive, negative } => {
            build_from_positive_negative(
                IntentElicitationMode::PositiveAndNegativeExemplars,
                positive,
                negative,
                None,
                &[],
                options,
            )
        }
        IntentSpecification::ConstellationOfOutcome {
            subject,
            references,
        } => {
            let mut positives = Vec::with_capacity(references.len() + 1);
            positives.push(RawIntentInput::try_new(
                "desired-outcome-subject",
                subject.clone(),
            )?);
            positives.extend(references.iter().cloned());
            build_from_positive_negative(
                IntentElicitationMode::ConstellationOfOutcome,
                &positives,
                &[],
                None,
                &[],
                options,
            )
        }
        IntentSpecification::InteractiveRefinement {
            initial,
            rounds,
            max_rounds,
        } => {
            let mut current = build_intent_geometry(initial, options, depth + 1)?;
            let mut accepted_round_count = 0usize;
            let mut warning_codes = current.provenance.warning_codes.clone();
            let mut drift_warning_code = current.drift_warning_code.clone();
            let mut last_cosine: Option<f32> = None;
            for round in rounds.iter().filter(|round| round.accepted) {
                let round_input = RawIntentInput::try_new(&round.round_id, &round.critique)?;
                let round_only = build_from_positive_negative(
                    IntentElicitationMode::InteractiveRefinement,
                    std::slice::from_ref(&round_input),
                    &[],
                    Some(*max_rounds),
                    &[],
                    options,
                )?;
                let before = current.c_intent.clone();
                let step_cosine = mean_cosine_maps(&before, &round_only.c_intent)?;
                if step_cosine < options.drift_rollback_cosine_floor {
                    if !warning_codes
                        .iter()
                        .any(|code| code == MEJEPA_INTENT_DRIFT_DURING_REFINEMENT)
                    {
                        warning_codes.push(MEJEPA_INTENT_DRIFT_DURING_REFINEMENT.to_string());
                    }
                    drift_warning_code = Some(MEJEPA_INTENT_DRIFT_DURING_REFINEMENT.to_string());
                    break;
                }
                current.c_intent = average_maps(&current.c_intent, &round_only.c_intent)?;
                current.tau_intent = calibrate_tau_for_vectors(
                    &current.c_intent,
                    options.tau_margin,
                    current.low_diversity_warning || round_only.low_diversity_warning,
                );
                current
                    .provenance
                    .source_input_ids
                    .push(round.round_id.clone());
                current.provenance.input_count += 1;
                current.provenance.positive_count += 1;
                accepted_round_count += 1;
                current.low_diversity_warning |= round_only.low_diversity_warning;
                last_cosine = Some(mean_cosine_maps(&before, &current.c_intent)?);
            }
            current.provenance.mode = IntentElicitationMode::InteractiveRefinement;
            current.provenance.accepted_round_count += accepted_round_count;
            current.provenance.max_rounds = Some(*max_rounds);
            current.provenance.warning_codes = warning_codes;
            current.provenance.input_sha256 =
                sha256_hex(&serde_json::to_vec(&serde_json::json!({
                    "initial": initial,
                    "rounds": rounds,
                    "maxRounds": max_rounds
                }))?);
            current.unconverged = rounds.len() as u32 >= *max_rounds
                && last_cosine
                    .map(|value| value < options.convergence_cosine_threshold)
                    .unwrap_or(true);
            current.drift_warning_code = drift_warning_code;
            Ok(current)
        }
    }
}

fn build_from_positive_negative(
    mode: IntentElicitationMode,
    positive: &[RawIntentInput],
    negative: &[RawIntentInput],
    max_rounds: Option<u32>,
    citations: &[String],
    options: &ElicitationOptions,
) -> Result<BuiltIntent, MejepaInferError> {
    if positive.is_empty() {
        return Err(invalid(
            "positive",
            format!("{MEJEPA_INTENT_EMPTY_POSITIVES}: positive exemplars required"),
        ));
    }
    let mut c_intent = BTreeMap::new();
    let mut tau_intent = BTreeMap::new();
    let mut any_low_diversity = false;
    for embedder_id in &options.embedder_ids {
        let positive_vectors = positive
            .iter()
            .map(|input| hashed_unit_vector(embedder_id, &input.text, options.vector_dim))
            .collect::<Result<Vec<_>, _>>()?;
        let positive_centroid = mean_vector(positive_vectors.iter()).ok_or_else(|| {
            invalid(
                "positive",
                "could not derive positive centroid from positive exemplars",
            )
        })?;
        let negative_centroid = if negative.is_empty() {
            None
        } else {
            let negative_vectors = negative
                .iter()
                .map(|input| hashed_unit_vector(embedder_id, &input.text, options.vector_dim))
                .collect::<Result<Vec<_>, _>>()?;
            let centroid = mean_vector(negative_vectors.iter()).ok_or_else(|| {
                invalid(
                    "negative",
                    "could not derive negative centroid from negative exemplars",
                )
            })?;
            let overlap = cosine(&positive_centroid, &centroid)?;
            if overlap > INTENT_OVERLAP_COSINE_THRESHOLD {
                return Err(invalid(
                    "negative",
                    format!(
                        "{MEJEPA_INTENT_NEGATIVES_OVERLAP_POSITIVES}: mean-positive to mean-negative cosine {overlap:.6} exceeds {INTENT_OVERLAP_COSINE_THRESHOLD:.6}"
                    ),
                ));
            }
            Some(centroid)
        };
        let mut centroid = positive_centroid.clone();
        if let Some(negative_centroid) = negative_centroid {
            for (idx, value) in centroid.iter_mut().enumerate() {
                *value -= negative_centroid[idx];
            }
            centroid = normalize(centroid, "positive-minus-negative centroid")?;
        }
        let low_diversity = low_diversity(&positive_vectors)?;
        any_low_diversity |= low_diversity;
        let positive_min_cosine = positive_vectors
            .iter()
            .map(|vector| cosine(vector, &centroid))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .fold(1.0_f32, f32::min);
        let tau = if low_diversity {
            (positive_min_cosine - (options.tau_margin * 2.0)).clamp(0.50, 0.99)
        } else {
            (positive_min_cosine - options.tau_margin).clamp(0.50, 0.99)
        };
        c_intent.insert(embedder_id.clone(), centroid);
        tau_intent.insert(embedder_id.clone(), tau);
    }
    let mut warning_codes = Vec::new();
    if any_low_diversity {
        warning_codes.push("MEJEPA_INTENT_LOW_DIVERSITY".to_string());
    }
    let source_input_ids = positive
        .iter()
        .chain(negative.iter())
        .map(|input| input.input_id.clone())
        .collect::<Vec<_>>();
    let input_sha256 = sha256_hex(&serde_json::to_vec(&serde_json::json!({
        "positive": positive,
        "negative": negative,
        "mode": mode,
    }))?);
    Ok(BuiltIntent {
        c_intent,
        tau_intent,
        provenance: ElicitationTrail {
            mode,
            input_count: positive.len() + negative.len(),
            positive_count: positive.len(),
            negative_count: negative.len(),
            accepted_round_count: 0,
            max_rounds,
            input_sha256,
            source_input_ids,
            citations: citations.to_vec(),
            warning_codes,
        },
        low_diversity_warning: any_low_diversity,
        unconverged: false,
        drift_warning_code: None,
    })
}

fn calibrate_tau_for_vectors(
    vectors: &BTreeMap<EmbedderId, Vec<f32>>,
    tau_margin: f32,
    low_diversity: bool,
) -> BTreeMap<EmbedderId, f32> {
    vectors
        .keys()
        .cloned()
        .map(|embedder_id| {
            let margin = if low_diversity {
                tau_margin * 2.0
            } else {
                tau_margin
            };
            (embedder_id, (1.0 - margin).clamp(0.50, 0.99))
        })
        .collect()
}

fn hashed_unit_vector(
    embedder_id: &EmbedderId,
    text: &str,
    dim: usize,
) -> Result<Vec<f32>, MejepaInferError> {
    if dim == 0 || dim > MAX_INTENT_VECTOR_DIM {
        return Err(invalid(
            "dim",
            format!("dim must be in [1, {MAX_INTENT_VECTOR_DIM}], got {dim}"),
        ));
    }
    let mut values = Vec::with_capacity(dim);
    for idx in 0..dim {
        let mut hasher = Sha256::new();
        hasher.update(b"mejepa-intent-vector-v1");
        hasher.update(embedder_id.0.as_bytes());
        hasher.update(idx.to_be_bytes());
        hasher.update(text.as_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&digest[..4]);
        let unit = u32::from_be_bytes(bytes) as f32 / u32::MAX as f32;
        values.push((unit * 2.0) - 1.0);
    }
    normalize(values, "hashed intent vector")
}

fn average_maps(
    lhs: &BTreeMap<EmbedderId, Vec<f32>>,
    rhs: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<BTreeMap<EmbedderId, Vec<f32>>, MejepaInferError> {
    if lhs.keys().ne(rhs.keys()) {
        return Err(invalid(
            "intent_maps",
            "intent map embedder keys must match for averaging",
        ));
    }
    let mut out = BTreeMap::new();
    for (embedder_id, left) in lhs {
        let right = rhs
            .get(embedder_id)
            .ok_or_else(|| invalid("intent_maps", "missing right embedder"))?;
        if left.len() != right.len() {
            return Err(MejepaInferError::DimMismatch {
                expected: left.len(),
                actual: right.len(),
                context: format!("average map dim mismatch for embedder {}", embedder_id.0),
            });
        }
        let averaged = left
            .iter()
            .zip(right.iter())
            .map(|(a, b)| (a + b) * 0.5)
            .collect::<Vec<_>>();
        out.insert(
            embedder_id.clone(),
            normalize(averaged, "averaged intent map")?,
        );
    }
    Ok(out)
}

fn mean_cosine_maps(
    lhs: &BTreeMap<EmbedderId, Vec<f32>>,
    rhs: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<f32, MejepaInferError> {
    if lhs.keys().ne(rhs.keys()) {
        return Err(invalid(
            "intent_maps",
            "intent map embedder keys must match for cosine",
        ));
    }
    let mut count = 0usize;
    let mut sum = 0.0_f32;
    for (embedder_id, left) in lhs {
        let right = rhs
            .get(embedder_id)
            .ok_or_else(|| invalid("intent_maps", "missing right embedder"))?;
        sum += cosine(left, right)?;
        count += 1;
    }
    if count == 0 {
        return Err(invalid("intent_maps", "at least one embedder is required"));
    }
    Ok(sum / count as f32)
}

fn mean_vector<'a, I>(vectors: I) -> Option<Vec<f32>>
where
    I: IntoIterator<Item = &'a Vec<f32>>,
{
    let mut iter = vectors.into_iter();
    let first = iter.next()?.clone();
    let mut sum = first;
    let mut count = 1usize;
    for vector in iter {
        if vector.len() != sum.len() {
            return None;
        }
        for (idx, value) in vector.iter().enumerate() {
            sum[idx] += *value;
        }
        count += 1;
    }
    for value in &mut sum {
        *value /= count as f32;
    }
    normalize(sum, "mean vector").ok()
}

fn mean<I>(values: I) -> Option<f32>
where
    I: IntoIterator<Item = f32>,
{
    let mut count = 0usize;
    let mut sum = 0.0_f32;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        sum += value;
        count += 1;
    }
    (count > 0).then_some(sum / count as f32)
}

fn normalize(mut values: Vec<f32>, context: &str) -> Result<Vec<f32>, MejepaInferError> {
    let norm_sq = values.iter().map(|value| value * value).sum::<f32>();
    if !norm_sq.is_finite() || norm_sq <= 0.0 {
        return Err(invalid(
            context,
            format!("cannot normalize zero/non-finite vector; norm_sq={norm_sq}"),
        ));
    }
    let norm = norm_sq.sqrt();
    for value in &mut values {
        *value /= norm;
        if !value.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: context.to_string(),
                detail: "normalized vector contains non-finite value".to_string(),
            });
        }
    }
    Ok(values)
}

fn cosine(lhs: &[f32], rhs: &[f32]) -> Result<f32, MejepaInferError> {
    if lhs.len() != rhs.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: lhs.len(),
            actual: rhs.len(),
            context: "intent cosine vector length mismatch".to_string(),
        });
    }
    if lhs.is_empty() {
        return Err(invalid("cosine", "vectors must be non-empty"));
    }
    let dot = lhs
        .iter()
        .zip(rhs.iter())
        .map(|(left, right)| left * right)
        .sum::<f32>();
    if !dot.is_finite() {
        return Err(MejepaInferError::NanDetected {
            nan_source: "intent_cosine".to_string(),
            detail: "cosine dot product is non-finite".to_string(),
        });
    }
    Ok(dot.clamp(-1.0, 1.0))
}

fn low_diversity(vectors: &[Vec<f32>]) -> Result<bool, MejepaInferError> {
    if vectors.len() <= 1 {
        return Ok(false);
    }
    let mut min_cosine = 1.0_f32;
    for idx in 0..vectors.len() {
        for jdx in (idx + 1)..vectors.len() {
            min_cosine = min_cosine.min(cosine(&vectors[idx], &vectors[jdx])?);
        }
    }
    Ok(min_cosine >= INTENT_LOW_DIVERSITY_COSINE_THRESHOLD)
}

fn validate_inputs(field: &str, inputs: &[RawIntentInput]) -> Result<(), MejepaInferError> {
    let mut seen = BTreeSet::new();
    for (idx, input) in inputs.iter().enumerate() {
        input.validate(&format!("{field}[{idx}]"))?;
        if !seen.insert(input.input_id.clone()) {
            return Err(invalid(
                field,
                format!("duplicate input id {}", input.input_id),
            ));
        }
    }
    Ok(())
}

fn validate_inputs_ids(field: &str, ids: &[String]) -> Result<(), MejepaInferError> {
    if ids.is_empty() {
        return Err(invalid(field, "at least one source input id is required"));
    }
    for id in ids {
        validate_bounded_text(field, id, 256)?;
    }
    Ok(())
}

fn validate_bounded_text(
    field: &str,
    value: &str,
    max_bytes: usize,
) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return Err(invalid(field, "must not be empty"));
    }
    if value.as_bytes().contains(&0) {
        return Err(invalid(field, "must not contain NUL bytes"));
    }
    if value.len() > max_bytes {
        return Err(invalid(
            field,
            format!("must be <= {max_bytes} bytes, got {}", value.len()),
        ));
    }
    Ok(())
}

fn validate_hex_id(field: &str, value: &str, len: usize) -> Result<(), MejepaInferError> {
    if value.len() != len || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid(field, format!("must be {len} hex chars")));
    }
    Ok(())
}

fn validate_unit_interval(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(invalid(
            field,
            format!("must be finite and in [0, 1], got {value}"),
        ));
    }
    Ok(())
}

fn validate_unit_vector(field: &str, vector: &[f32]) -> Result<(), MejepaInferError> {
    if vector.is_empty() {
        return Err(invalid(field, "vector must be non-empty"));
    }
    let norm_sq = vector.iter().try_fold(0.0_f32, |acc, value| {
        if !value.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: field.to_string(),
                detail: "vector contains non-finite value".to_string(),
            });
        }
        Ok(acc + value * value)
    })?;
    if (norm_sq - 1.0).abs() > 1e-3 {
        return Err(invalid(
            field,
            format!("vector must be unit norm; norm_sq={norm_sq}"),
        ));
    }
    Ok(())
}

fn invalid(field: impl Into<String>, detail: impl Into<String>) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    }
}

fn default_panel_id() -> String {
    INTENT_DEFAULT_PANEL_ID.to_string()
}

fn default_corpus_shard_hash() -> String {
    INTENT_DEFAULT_CORPUS_SHARD_HASH.to_string()
}

fn default_embedder_ids() -> Vec<EmbedderId> {
    vec![
        EmbedderId("E1".to_string()),
        EmbedderId("E10".to_string()),
        EmbedderId("E12".to_string()),
    ]
}

fn default_vector_dim() -> usize {
    DEFAULT_INTENT_VECTOR_DIM
}

fn default_tau_margin() -> f32 {
    DEFAULT_INTENT_TAU_MARGIN
}

fn default_convergence_cosine_threshold() -> f32 {
    0.98
}

fn default_drift_rollback_cosine_floor() -> f32 {
    -1.0
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_infer_rocksdb;

    #[test]
    fn elicit_exemplar_intent_aligns_positive_over_negative() {
        let created_at = 1_779_000_000_000;
        let spec = IntentSpecification::PositiveAndNegativeExemplars {
            positive: vec![
                RawIntentInput::try_new("pos-1", "short direct answer").unwrap(),
                RawIntentInput::try_new("pos-2", "concise helpful reply").unwrap(),
            ],
            negative: vec![RawIntentInput::try_new(
                "neg-1",
                "long rambling answer with irrelevant detail",
            )
            .unwrap()],
        };
        let north_star = elicit_intent(spec, ElicitationOptions::default(), created_at).unwrap();
        let positive = align_to_intent(
            &north_star,
            IntentCandidate::RawInput {
                candidate_id: "positive".to_string(),
                text: "short direct answer".to_string(),
            },
        )
        .unwrap();
        let negative = align_to_intent(
            &north_star,
            IntentCandidate::RawInput {
                candidate_id: "negative".to_string(),
                text: "long rambling answer with irrelevant detail".to_string(),
            },
        )
        .unwrap();
        assert!(positive.mean_cosine > negative.mean_cosine);
        assert_eq!(north_star.source_of_truth_cf, CF_MEJEPA_NORTH_STARS);
    }

    #[test]
    fn empty_positive_set_fails_closed() {
        let err = elicit_intent(
            IntentSpecification::PositiveAndNegativeExemplars {
                positive: Vec::new(),
                negative: Vec::new(),
            },
            ElicitationOptions::default(),
            1_779_000_000_000,
        )
        .unwrap_err();
        assert!(err.to_string().contains(MEJEPA_INTENT_EMPTY_POSITIVES));
    }

    #[test]
    fn persists_north_star_with_readback() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let north_star = elicit_intent(
            IntentSpecification::FreeText {
                description: "answer in fewer than three sentences".to_string(),
            },
            ElicitationOptions::default(),
            1_779_000_000_000,
        )
        .unwrap();
        write_north_star_sync_readback(db.as_ref(), &north_star).unwrap();
        let readback = read_north_star(db.as_ref(), &north_star.intent_id)
            .unwrap()
            .unwrap();
        assert_eq!(readback, north_star);
    }
}
