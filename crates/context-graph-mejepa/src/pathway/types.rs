use serde::{Deserialize, Serialize};

use super::error::{
    require, validate_hex, validate_id, validate_probability, validate_sha, PathwayError,
    PathwayResult,
};
use super::{MAX_Q5_EVENTS, MAX_TOP_K, PATHWAY_AMBIGUOUS_LEAF_REJECTED, PATHWAY_SCHEMA_VERSION};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathwayLeafKind {
    Q1ClaimExists,
    Q2OraclePass,
    Q5ShiftEvent,
    Ambiguous,
}

impl PathwayLeafKind {
    pub fn is_binary(self) -> bool {
        matches!(
            self,
            Self::Q1ClaimExists | Self::Q2OraclePass | Self::Q5ShiftEvent
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathwayLeafOutcome {
    Yes,
    No,
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathwayLeafEvidence {
    pub accepted_label_ids: Vec<String>,
    pub active_skill_ids: Vec<String>,
    pub higher_ability_ids: Vec<String>,
    pub source_membership_keys: Vec<String>,
    pub skill_signature_hash: Option<String>,
    pub closest_historical_pathway_id: Option<String>,
    pub unknown_signature: bool,
}

impl PathwayLeafEvidence {
    pub fn unknown() -> Self {
        Self {
            accepted_label_ids: Vec::new(),
            active_skill_ids: Vec::new(),
            higher_ability_ids: Vec::new(),
            source_membership_keys: Vec::new(),
            skill_signature_hash: None,
            closest_historical_pathway_id: None,
            unknown_signature: true,
        }
    }

    pub fn from_prediction_label_context(context: &crate::PredictionLabelContext) -> Self {
        let unknown_signature = context.accepted_label_ids.is_empty()
            && context.active_skill_ids.is_empty()
            && context.active_higher_ability_ids.is_empty()
            && context.source_membership_keys.is_empty()
            && context.skill_signature_hash.is_none();
        Self {
            accepted_label_ids: context.accepted_label_ids.clone(),
            active_skill_ids: context.active_skill_ids.clone(),
            higher_ability_ids: context.active_higher_ability_ids.clone(),
            source_membership_keys: context.source_membership_keys.clone(),
            skill_signature_hash: context.skill_signature_hash.clone(),
            closest_historical_pathway_id: None,
            unknown_signature,
        }
    }

    pub fn has_live_context(&self) -> bool {
        !self.accepted_label_ids.is_empty()
            || !self.active_skill_ids.is_empty()
            || !self.higher_ability_ids.is_empty()
            || !self.source_membership_keys.is_empty()
            || self.skill_signature_hash.is_some()
            || self.closest_historical_pathway_id.is_some()
    }

    pub fn validate(&self) -> PathwayResult<()> {
        for label in &self.accepted_label_ids {
            validate_id("accepted_label_id", label)?;
        }
        for skill in &self.active_skill_ids {
            validate_id("active_skill_id", skill)?;
        }
        for ability in &self.higher_ability_ids {
            validate_id("higher_ability_id", ability)?;
        }
        for key in &self.source_membership_keys {
            validate_id("source_membership_key", key)?;
        }
        if let Some(hash) = &self.skill_signature_hash {
            validate_id("skill_signature_hash", hash)?;
        }
        if let Some(pathway) = &self.closest_historical_pathway_id {
            validate_id("closest_historical_pathway_id", pathway)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathwayLeafCalibrationReference {
    pub leaf_id: String,
    pub calibration_source_cf: String,
    pub calibration_row_key: String,
}

impl PathwayLeafCalibrationReference {
    pub fn validate(&self) -> PathwayResult<()> {
        validate_id("leaf_calibration.leaf_id", &self.leaf_id)?;
        validate_id(
            "leaf_calibration.calibration_source_cf",
            &self.calibration_source_cf,
        )?;
        validate_id(
            "leaf_calibration.calibration_row_key",
            &self.calibration_row_key,
        )
    }
}

impl Default for PathwayLeafEvidence {
    fn default() -> Self {
        Self::unknown()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathwayLeaf {
    pub leaf_id: String,
    pub leaf_kind: PathwayLeafKind,
    pub predicted_outcome: PathwayLeafOutcome,
    pub predicted_probability: f32,
    pub conformal_interval: [f32; 2],
    pub event_id: Option<String>,
    pub event_label: Option<String>,
    pub cold_cell_warning: bool,
    pub evidence: PathwayLeafEvidence,
}

impl PathwayLeaf {
    pub fn validate(&self) -> PathwayResult<()> {
        validate_id("leaf_id", &self.leaf_id)?;
        require(
            self.leaf_kind.is_binary(),
            PathwayError::code(
                PATHWAY_AMBIGUOUS_LEAF_REJECTED,
                "pathway leaves must be Q1/Q2/Q5 only",
            ),
        )?;
        validate_probability("predicted_probability", self.predicted_probability)?;
        validate_probability("conformal_interval.lower", self.conformal_interval[0])?;
        validate_probability("conformal_interval.upper", self.conformal_interval[1])?;
        require(
            self.conformal_interval[0] <= self.conformal_interval[1],
            "conformal interval lower must be <= upper",
        )?;
        match self.leaf_kind {
            PathwayLeafKind::Q1ClaimExists | PathwayLeafKind::Q5ShiftEvent => require(
                matches!(
                    self.predicted_outcome,
                    PathwayLeafOutcome::Yes | PathwayLeafOutcome::No
                ),
                "Q1/Q5 leaves must use yes/no outcomes",
            )?,
            PathwayLeafKind::Q2OraclePass => require(
                matches!(
                    self.predicted_outcome,
                    PathwayLeafOutcome::Pass | PathwayLeafOutcome::Fail
                ),
                "Q2 leaves must use pass/fail outcomes",
            )?,
            PathwayLeafKind::Ambiguous => unreachable!("rejected above"),
        }
        if let Some(event_id) = &self.event_id {
            validate_id("event_id", event_id)?;
        }
        if let Some(label) = &self.event_label {
            validate_id("event_label", label)?;
        }
        self.evidence.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathwayNode {
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub depth: u32,
    pub leaf: PathwayLeaf,
    pub cumulative_probability: f32,
}

impl PathwayNode {
    pub fn validate(&self) -> PathwayResult<()> {
        validate_id("node_id", &self.node_id)?;
        if let Some(parent) = &self.parent_node_id {
            validate_id("parent_node_id", parent)?;
        }
        self.leaf.validate()?;
        validate_probability("cumulative_probability", self.cumulative_probability)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathwayTreeRecord {
    pub schema_version: u32,
    pub tree_id: String,
    pub prediction_id_hex: String,
    pub candidate_patch_sha256: String,
    pub nodes: Vec<PathwayNode>,
    pub surfaced_pathway_ids: Vec<String>,
    pub generated_branch_count: u64,
    pub ambiguous_leaves_in_pathway: u64,
    pub created_at_unix_ms: i64,
}

impl PathwayTreeRecord {
    pub fn key(&self) -> &str {
        &self.tree_id
    }

    pub fn validate(&self) -> PathwayResult<()> {
        require(
            self.schema_version == PATHWAY_SCHEMA_VERSION,
            "pathway tree schema version mismatch",
        )?;
        validate_id("tree_id", &self.tree_id)?;
        validate_hex("prediction_id_hex", &self.prediction_id_hex)?;
        validate_sha("candidate_patch_sha256", &self.candidate_patch_sha256)?;
        require(!self.nodes.is_empty(), "pathway tree must contain nodes")?;
        require(
            self.ambiguous_leaves_in_pathway == 0,
            PathwayError::code(
                PATHWAY_AMBIGUOUS_LEAF_REJECTED,
                "pathway tree contains ambiguous leaves",
            ),
        )?;
        for node in &self.nodes {
            node.validate()?;
        }
        for pathway_id in &self.surfaced_pathway_ids {
            validate_id("surfaced_pathway_id", pathway_id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfacedPathwayRecord {
    pub schema_version: u32,
    pub pathway_id: String,
    pub tree_id: String,
    pub prediction_id_hex: String,
    pub rank: u32,
    pub raw_joint_probability: f32,
    pub normalized_probability: f32,
    pub leaf_chain: Vec<PathwayLeaf>,
    pub terminal_node_id: String,
    pub closest_historical_pathway_id: Option<String>,
    pub cold_cell_warning: bool,
    pub unknown_pathway_signature: bool,
    pub created_at_unix_ms: i64,
}

impl SurfacedPathwayRecord {
    pub fn key(&self) -> &str {
        &self.pathway_id
    }

    pub fn validate(&self) -> PathwayResult<()> {
        require(
            self.schema_version == PATHWAY_SCHEMA_VERSION,
            "surfaced pathway schema version mismatch",
        )?;
        validate_id("pathway_id", &self.pathway_id)?;
        validate_id("tree_id", &self.tree_id)?;
        validate_hex("prediction_id_hex", &self.prediction_id_hex)?;
        validate_probability("raw_joint_probability", self.raw_joint_probability)?;
        validate_probability("normalized_probability", self.normalized_probability)?;
        require(
            !self.leaf_chain.is_empty(),
            "pathway leaf chain must not be empty",
        )?;
        for leaf in &self.leaf_chain {
            leaf.validate()?;
        }
        validate_id("terminal_node_id", &self.terminal_node_id)?;
        if let Some(pathway) = &self.closest_historical_pathway_id {
            validate_id("closest_historical_pathway_id", pathway)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorPathwayChoiceRecord {
    pub schema_version: u32,
    pub choice_id: String,
    pub prediction_id_hex: String,
    pub pathway_id: String,
    pub operator_id: String,
    pub rationale_text: Option<String>,
    pub chosen_at_unix_ms: i64,
}

impl OperatorPathwayChoiceRecord {
    pub fn key(&self) -> String {
        format!("choice::{}::{}", self.prediction_id_hex, self.pathway_id)
    }

    pub fn validate(&self) -> PathwayResult<()> {
        require(
            self.schema_version == PATHWAY_SCHEMA_VERSION,
            "operator pathway choice schema version mismatch",
        )?;
        validate_id("choice_id", &self.choice_id)?;
        validate_hex("prediction_id_hex", &self.prediction_id_hex)?;
        validate_id("pathway_id", &self.pathway_id)?;
        validate_id("operator_id", &self.operator_id)?;
        if let Some(rationale) = &self.rationale_text {
            require(
                !rationale.contains('\0'),
                "rationale_text must not contain NUL bytes",
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Q5PathwayEventInput {
    pub event_id: String,
    pub event_label: String,
    pub occurred_probability: f32,
    pub conformal_interval: [f32; 2],
    pub cold_cell_warning: bool,
    pub evidence: PathwayLeafEvidence,
}

impl Q5PathwayEventInput {
    pub fn validate(&self) -> PathwayResult<()> {
        validate_id("event_id", &self.event_id)?;
        validate_id("event_label", &self.event_label)?;
        validate_probability("occurred_probability", self.occurred_probability)?;
        validate_probability("q5_conformal_interval.lower", self.conformal_interval[0])?;
        validate_probability("q5_conformal_interval.upper", self.conformal_interval[1])?;
        require(
            self.conformal_interval[0] <= self.conformal_interval[1],
            "q5 conformal interval lower must be <= upper",
        )?;
        self.evidence.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathwaySurfaceInput {
    pub prediction_id_hex: String,
    pub candidate_patch_sha256: String,
    pub q1_claim_exists_probability: f32,
    pub q1_conformal_interval: [f32; 2],
    #[serde(default)]
    pub q1_claim_evidence: PathwayLeafEvidence,
    pub q2_oracle_pass_probability: f32,
    pub q2_conformal_interval: [f32; 2],
    #[serde(default)]
    pub q2_pass_evidence: PathwayLeafEvidence,
    pub q2_fail_evidence: PathwayLeafEvidence,
    pub q5_events: Vec<Q5PathwayEventInput>,
    #[serde(default)]
    pub leaf_calibration_references: Vec<PathwayLeafCalibrationReference>,
    #[serde(default)]
    pub require_non_cold_calibration: bool,
    pub top_k: usize,
    pub prune_epsilon: f32,
    pub created_at_unix_ms: i64,
}

impl PathwaySurfaceInput {
    pub fn validate(&self) -> PathwayResult<()> {
        validate_hex("prediction_id_hex", &self.prediction_id_hex)?;
        validate_sha("candidate_patch_sha256", &self.candidate_patch_sha256)?;
        validate_probability(
            "q1_claim_exists_probability",
            self.q1_claim_exists_probability,
        )?;
        validate_probability(
            "q2_oracle_pass_probability",
            self.q2_oracle_pass_probability,
        )?;
        validate_probability("q1_conformal_interval.lower", self.q1_conformal_interval[0])?;
        validate_probability("q1_conformal_interval.upper", self.q1_conformal_interval[1])?;
        validate_probability("q2_conformal_interval.lower", self.q2_conformal_interval[0])?;
        validate_probability("q2_conformal_interval.upper", self.q2_conformal_interval[1])?;
        require(
            self.q1_conformal_interval[0] <= self.q1_conformal_interval[1]
                && self.q2_conformal_interval[0] <= self.q2_conformal_interval[1],
            "conformal interval lower must be <= upper",
        )?;
        require(
            (1..=MAX_TOP_K).contains(&self.top_k),
            format!("top_k must be within 1..={MAX_TOP_K}"),
        )?;
        validate_probability("prune_epsilon", self.prune_epsilon)?;
        require(
            self.q5_events.len() <= MAX_Q5_EVENTS,
            format!("q5_events exceeds maximum {MAX_Q5_EVENTS}"),
        )?;
        self.q1_claim_evidence.validate()?;
        self.q2_pass_evidence.validate()?;
        self.q2_fail_evidence.validate()?;
        for calibration in &self.leaf_calibration_references {
            calibration.validate()?;
        }
        for event in &self.q5_events {
            event.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathwayLeafCreditAssignment {
    pub schema_version: u32,
    pub pathway_id: String,
    pub leaf_id: String,
    pub prediction_id_hex: String,
    pub predicted_outcome: PathwayLeafOutcome,
    pub observed_outcome: PathwayLeafOutcome,
    pub mistake_context_key: String,
    pub accepted_label_ids: Vec<String>,
    pub active_skill_ids: Vec<String>,
    pub higher_ability_ids: Vec<String>,
    pub source_membership_keys: Vec<String>,
    pub skill_signature_hash: Option<String>,
    pub closest_historical_pathway_id: Option<String>,
    pub unknown_signature: bool,
}

impl PathwayLeafCreditAssignment {
    pub fn validate(&self) -> PathwayResult<()> {
        require(
            self.schema_version == PATHWAY_SCHEMA_VERSION,
            "pathway leaf credit assignment schema version mismatch",
        )?;
        validate_id("credit.pathway_id", &self.pathway_id)?;
        validate_id("credit.leaf_id", &self.leaf_id)?;
        validate_hex("credit.prediction_id_hex", &self.prediction_id_hex)?;
        validate_id("credit.mistake_context_key", &self.mistake_context_key)?;
        require(
            self.predicted_outcome != self.observed_outcome,
            "credit assignment requires observed outcome to disagree with prediction",
        )?;
        for label in &self.accepted_label_ids {
            validate_id("credit.accepted_label_id", label)?;
        }
        for skill in &self.active_skill_ids {
            validate_id("credit.active_skill_id", skill)?;
        }
        for ability in &self.higher_ability_ids {
            validate_id("credit.higher_ability_id", ability)?;
        }
        for membership in &self.source_membership_keys {
            validate_id("credit.source_membership_key", membership)?;
        }
        if let Some(hash) = &self.skill_signature_hash {
            validate_id("credit.skill_signature_hash", hash)?;
        }
        if let Some(pathway) = &self.closest_historical_pathway_id {
            validate_id("credit.closest_historical_pathway_id", pathway)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathwaySurfaceReport {
    pub schema_version: u32,
    pub tree: PathwayTreeRecord,
    pub surfaced_pathways: Vec<SurfacedPathwayRecord>,
    pub leaf_calibration_references: Vec<PathwayLeafCalibrationReference>,
    pub top_k_probability_sum: f32,
    pub ambiguous_leaves_in_pathway: u64,
    pub source_of_truth_cfs: Vec<String>,
}
