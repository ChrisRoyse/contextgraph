use serde::{Deserialize, Serialize};

pub const SKILL_SEQUENCE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SkillOutcomeVerdict {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillOutcomeObservation {
    pub outcome_label_id: String,
    pub verdict: SkillOutcomeVerdict,
    pub target_side_supervision_only: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillOutcomeDistribution {
    pub pass: u64,
    pub fail: u64,
    pub unknown: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillStepEvidence {
    pub step_index: u32,
    pub chunk_id: String,
    pub file_path: String,
    pub code_state_key: String,
    pub accepted_label_ids: Vec<String>,
    pub group_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillStepTemplate {
    pub step_index: u32,
    pub accepted_label_ids: Vec<String>,
    pub group_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillTransitionEdge {
    pub from_step_index: u32,
    pub to_step_index: u32,
    pub edge_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillEpisodeRow {
    pub episode_id: String,
    pub proposed_skill_name: Option<String>,
    pub ordered_steps: Vec<SkillStepEvidence>,
    pub outcome: Option<SkillOutcomeObservation>,
    pub failure_evidence_set_ids: Vec<String>,
    pub cell_baseline_fail_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillDiscoveryConfig {
    pub min_support: u64,
    pub min_lift_over_cell_baseline: f64,
    pub min_confidence: f64,
    pub allow_pending_outcome_candidates: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SkillPromotionStatus {
    ActiveLearning,
    PromotionReady,
    OperatorApproved,
    Demoted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Level2SkillRow {
    pub schema_version: u32,
    pub skill_id: String,
    pub skill_name: String,
    pub parent_group_ids: Vec<String>,
    pub parent_skill_ids: Vec<String>,
    pub ordered_steps: Vec<SkillStepTemplate>,
    pub prerequisite_label_ids: Vec<String>,
    pub transition_edges: Vec<SkillTransitionEdge>,
    pub support: u64,
    pub confidence: f64,
    pub lift_over_cell_baseline: f64,
    pub stability: f64,
    pub oracle_outcome_distribution: SkillOutcomeDistribution,
    pub code_state_keys: Vec<String>,
    pub source_episode_ids: Vec<String>,
    pub failure_evidence_set_ids: Vec<String>,
    pub live_input_allowed: bool,
    pub promotion_status: SkillPromotionStatus,
    pub operator_approved: bool,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SkillCandidateKind {
    FailureSkill,
    PassStabilitySkill,
    ContextNegativeEvidence,
    NeutralDiagnostic,
    RejectOverbroadOrLeaky,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillUsefulnessProfile {
    pub pattern_hash: String,
    pub candidate_kind: SkillCandidateKind,
    pub support: u64,
    pub confidence: f64,
    pub lift_over_cell_baseline: f64,
    pub stability: f64,
    pub genericity_score: f64,
    pub split_selection_weight: f64,
    pub oracle_outcome_distribution: SkillOutcomeDistribution,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillCandidateRejection {
    pub pattern_hash: String,
    pub candidate_kind: SkillCandidateKind,
    pub reason: String,
    pub support: u64,
    pub lift_over_cell_baseline: f64,
    pub confidence: f64,
    pub source_episode_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillDiscoveryReport {
    pub candidates: Vec<Level2SkillRow>,
    pub rejections: Vec<SkillCandidateRejection>,
    pub usefulness_profiles: Vec<SkillUsefulnessProfile>,
}
