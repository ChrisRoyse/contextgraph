//! TASK-PY-G-118 skill-sequence discovery.
//!
//! A skill is an ordered consequence pattern over live-observable chunk labels.
//! Target-side oracle/test labels may score the pattern after reality responds,
//! but they are never allowed in the live inputs that identify the skill.

use crate::error::{TrainerError, TrainerErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const SKILL_SEQUENCE_SCHEMA_VERSION: u32 = 1;
const MAX_ID_BYTES: usize = 512;
const MAX_STEPS: usize = 64;
const MAX_IDS_PER_STEP: usize = 128;
const MAX_EPISODES_PER_SKILL: usize = 4096;

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

impl SkillOutcomeDistribution {
    pub fn supervised_total(&self) -> u64 {
        self.pass + self.fail
    }
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

impl Default for SkillDiscoveryConfig {
    fn default() -> Self {
        Self {
            min_support: 2,
            min_lift_over_cell_baseline: 0.10,
            min_confidence: 0.60,
            allow_pending_outcome_candidates: true,
        }
    }
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

impl Level2SkillRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        if self.schema_version != SKILL_SEQUENCE_SCHEMA_VERSION {
            return Err(invalid(
                "schema_version",
                format!(
                    "expected {}, got {}",
                    SKILL_SEQUENCE_SCHEMA_VERSION, self.schema_version
                ),
            ));
        }
        validate_id("skill_id", &self.skill_id)?;
        validate_id("skill_name", &self.skill_name)?;
        validate_id_list("parent_group_ids", &self.parent_group_ids, MAX_IDS_PER_STEP)?;
        validate_id_list("parent_skill_ids", &self.parent_skill_ids, MAX_IDS_PER_STEP)?;
        validate_step_templates(&self.ordered_steps)?;
        validate_id_list(
            "prerequisite_label_ids",
            &self.prerequisite_label_ids,
            MAX_IDS_PER_STEP,
        )?;
        validate_transition_edges(&self.transition_edges, self.ordered_steps.len())?;
        validate_id_list("code_state_keys", &self.code_state_keys, MAX_EPISODES_PER_SKILL)?;
        validate_id_list(
            "source_episode_ids",
            &self.source_episode_ids,
            MAX_EPISODES_PER_SKILL,
        )?;
        validate_id_list(
            "failure_evidence_set_ids",
            &self.failure_evidence_set_ids,
            MAX_EPISODES_PER_SKILL,
        )?;
        validate_finite_unit("confidence", self.confidence)?;
        validate_finite_unit("stability", self.stability)?;
        if !self.lift_over_cell_baseline.is_finite() {
            return Err(invalid(
                "lift_over_cell_baseline",
                "must be a finite value",
            ));
        }
        if self.support == 0 || self.source_episode_ids.len() as u64 != self.support {
            return Err(invalid(
                "support",
                "must be positive and match source_episode_ids length",
            ));
        }
        if !self.live_input_allowed {
            return Err(invalid(
                "live_input_allowed",
                "skill rows with leaky live inputs must not be persisted",
            ));
        }
        if self.created_at_unix_ms <= 0 {
            return Err(invalid("created_at_unix_ms", "must be positive"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillCandidateRejection {
    pub pattern_hash: String,
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
}

pub fn discover_skill_candidates(
    episodes: &[SkillEpisodeRow],
    config: SkillDiscoveryConfig,
    created_at_unix_ms: i64,
) -> Result<SkillDiscoveryReport, TrainerError> {
    validate_config(config)?;
    if episodes.is_empty() {
        return Err(invalid("episodes", "must not be empty"));
    }
    if created_at_unix_ms <= 0 {
        return Err(invalid("created_at_unix_ms", "must be positive"));
    }

    let mut groups: BTreeMap<String, Accumulator> = BTreeMap::new();
    for episode in episodes {
        validate_episode(episode)?;
        let templates = templates_from_steps(&episode.ordered_steps)?;
        let pattern_hash = pattern_hash(&templates)?;
        groups
            .entry(pattern_hash)
            .or_insert_with(|| Accumulator::new(templates, episode.proposed_skill_name.clone()))
            .add_episode(episode)?;
    }

    let mut candidates = Vec::new();
    let mut rejections = Vec::new();
    for (pattern_hash, acc) in groups {
        let metrics = acc.metrics();
        if acc.support < config.min_support {
            rejections.push(acc.rejection(
                pattern_hash,
                "low_support_overfit",
                metrics.lift_over_cell_baseline,
                metrics.confidence,
            ));
            continue;
        }
        let promotion_status = if acc.outcome_distribution.supervised_total() == 0 {
            if config.allow_pending_outcome_candidates {
                SkillPromotionStatus::ActiveLearning
            } else {
                rejections.push(acc.rejection(
                    pattern_hash,
                    "missing_target_supervision",
                    metrics.lift_over_cell_baseline,
                    metrics.confidence,
                ));
                continue;
            }
        } else if metrics.lift_over_cell_baseline < config.min_lift_over_cell_baseline
            || metrics.confidence < config.min_confidence
        {
            rejections.push(acc.rejection(
                pattern_hash,
                "below_usefulness_threshold",
                metrics.lift_over_cell_baseline,
                metrics.confidence,
            ));
            continue;
        } else {
            SkillPromotionStatus::PromotionReady
        };
        let row = acc.into_row(metrics, promotion_status, created_at_unix_ms)?;
        row.validate()?;
        candidates.push(row);
    }
    Ok(SkillDiscoveryReport {
        candidates,
        rejections,
    })
}

pub fn skill_id_from_parts(
    skill_name: &str,
    parent_group_ids: &[String],
    ordered_steps: &[SkillStepTemplate],
) -> Result<String, TrainerError> {
    validate_id("skill_name", skill_name)?;
    validate_id_list("parent_group_ids", parent_group_ids, MAX_IDS_PER_STEP)?;
    validate_step_templates(ordered_steps)?;
    let mut hasher = Sha256::new();
    hasher.update(skill_name.as_bytes());
    for id in parent_group_ids {
        hasher.update([0]);
        hasher.update(id.as_bytes());
    }
    for step in ordered_steps {
        hasher.update([0xff]);
        hasher.update(step.step_index.to_le_bytes());
        for label in &step.accepted_label_ids {
            hasher.update([0]);
            hasher.update(label.as_bytes());
        }
        for group in &step.group_ids {
            hasher.update([1]);
            hasher.update(group.as_bytes());
        }
    }
    Ok(format!(
        "skill:{}:{}",
        slugify(skill_name),
        &hex::encode(hasher.finalize())[..16]
    ))
}

pub fn is_target_only_live_label(label: &str) -> bool {
    matches!(
        label.split_once(':').map(|(family, _)| family),
        Some(
            "oracle"
                | "docker"
                | "test_phase"
                | "test_result"
                | "code_state_outcome"
                | "partition"
                | "leakage_policy"
                | "ground_truth"
                | "truth"
                | "post_tool_use"
        )
    )
}

#[derive(Debug, Clone)]
struct Accumulator {
    proposed_skill_name: Option<String>,
    ordered_steps: Vec<SkillStepTemplate>,
    parent_group_ids: BTreeSet<String>,
    prerequisite_label_ids: BTreeSet<String>,
    code_state_keys: BTreeSet<String>,
    source_episode_ids: Vec<String>,
    failure_evidence_set_ids: BTreeSet<String>,
    file_paths: BTreeSet<String>,
    support: u64,
    baseline_fail_rate_sum: f64,
    outcome_distribution: SkillOutcomeDistribution,
}

impl Accumulator {
    fn new(ordered_steps: Vec<SkillStepTemplate>, proposed_skill_name: Option<String>) -> Self {
        let mut parent_group_ids = BTreeSet::new();
        let mut prerequisite_label_ids = BTreeSet::new();
        for step in &ordered_steps {
            parent_group_ids.extend(step.group_ids.iter().cloned());
            if step.step_index == 0 {
                prerequisite_label_ids.extend(step.accepted_label_ids.iter().cloned());
            }
        }
        Self {
            proposed_skill_name,
            ordered_steps,
            parent_group_ids,
            prerequisite_label_ids,
            code_state_keys: BTreeSet::new(),
            source_episode_ids: Vec::new(),
            failure_evidence_set_ids: BTreeSet::new(),
            file_paths: BTreeSet::new(),
            support: 0,
            baseline_fail_rate_sum: 0.0,
            outcome_distribution: SkillOutcomeDistribution::default(),
        }
    }

    fn add_episode(&mut self, episode: &SkillEpisodeRow) -> Result<(), TrainerError> {
        if self.support as usize >= MAX_EPISODES_PER_SKILL {
            return Err(invalid("episodes", "too many episodes for one skill"));
        }
        self.support += 1;
        self.baseline_fail_rate_sum += episode.cell_baseline_fail_rate;
        self.source_episode_ids.push(episode.episode_id.clone());
        self.failure_evidence_set_ids
            .extend(episode.failure_evidence_set_ids.iter().cloned());
        for step in &episode.ordered_steps {
            self.code_state_keys.insert(step.code_state_key.clone());
            self.file_paths.insert(step.file_path.clone());
        }
        match episode.outcome.as_ref().map(|row| row.verdict) {
            Some(SkillOutcomeVerdict::Pass) => self.outcome_distribution.pass += 1,
            Some(SkillOutcomeVerdict::Fail) => self.outcome_distribution.fail += 1,
            Some(SkillOutcomeVerdict::Unknown) | None => self.outcome_distribution.unknown += 1,
        }
        Ok(())
    }

    fn metrics(&self) -> CandidateMetrics {
        let supervised = self.outcome_distribution.supervised_total();
        if supervised == 0 {
            return CandidateMetrics {
                confidence: 0.0,
                lift_over_cell_baseline: 0.0,
                stability: stability_score(self.file_paths.len(), self.support),
            };
        }
        let fail_rate = self.outcome_distribution.fail as f64 / supervised as f64;
        let pass_rate = self.outcome_distribution.pass as f64 / supervised as f64;
        let baseline_fail_rate = self.baseline_fail_rate_sum / self.support as f64;
        let fail_lift = fail_rate - baseline_fail_rate;
        let pass_lift = pass_rate - (1.0 - baseline_fail_rate);
        CandidateMetrics {
            confidence: fail_rate.max(pass_rate),
            lift_over_cell_baseline: fail_lift.max(pass_lift),
            stability: stability_score(self.file_paths.len(), self.support),
        }
    }

    fn rejection(
        &self,
        pattern_hash: String,
        reason: &str,
        lift_over_cell_baseline: f64,
        confidence: f64,
    ) -> SkillCandidateRejection {
        SkillCandidateRejection {
            pattern_hash,
            reason: reason.to_string(),
            support: self.support,
            lift_over_cell_baseline,
            confidence,
            source_episode_ids: self.source_episode_ids.clone(),
        }
    }

    fn into_row(
        self,
        metrics: CandidateMetrics,
        promotion_status: SkillPromotionStatus,
        created_at_unix_ms: i64,
    ) -> Result<Level2SkillRow, TrainerError> {
        let parent_group_ids = self.parent_group_ids.into_iter().collect::<Vec<_>>();
        if parent_group_ids.is_empty() {
            return Err(invalid(
                "parent_group_ids",
                "skill candidates require at least one Level-1 group",
            ));
        }
        let skill_name = self
            .proposed_skill_name
            .unwrap_or_else(|| derive_skill_name(&parent_group_ids, &self.ordered_steps));
        let skill_id = skill_id_from_parts(&skill_name, &parent_group_ids, &self.ordered_steps)?;
        Ok(Level2SkillRow {
            schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
            skill_id,
            skill_name,
            parent_group_ids,
            parent_skill_ids: Vec::new(),
            transition_edges: transition_edges_for_steps(&self.ordered_steps),
            prerequisite_label_ids: self.prerequisite_label_ids.into_iter().collect(),
            ordered_steps: self.ordered_steps,
            support: self.support,
            confidence: metrics.confidence,
            lift_over_cell_baseline: metrics.lift_over_cell_baseline,
            stability: metrics.stability,
            oracle_outcome_distribution: self.outcome_distribution,
            code_state_keys: self.code_state_keys.into_iter().collect(),
            source_episode_ids: self.source_episode_ids,
            failure_evidence_set_ids: self.failure_evidence_set_ids.into_iter().collect(),
            live_input_allowed: true,
            promotion_status,
            operator_approved: false,
            created_at_unix_ms,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct CandidateMetrics {
    confidence: f64,
    lift_over_cell_baseline: f64,
    stability: f64,
}

fn validate_episode(row: &SkillEpisodeRow) -> Result<(), TrainerError> {
    validate_id("episode_id", &row.episode_id)?;
    if let Some(name) = &row.proposed_skill_name {
        validate_id("proposed_skill_name", name)?;
    }
    validate_ordered_steps(&row.ordered_steps)?;
    validate_id_list(
        "failure_evidence_set_ids",
        &row.failure_evidence_set_ids,
        MAX_IDS_PER_STEP,
    )?;
    validate_finite_unit("cell_baseline_fail_rate", row.cell_baseline_fail_rate)?;
    if let Some(outcome) = &row.outcome {
        validate_id("outcome_label_id", &outcome.outcome_label_id)?;
        if !outcome.target_side_supervision_only {
            return Err(invalid(
                "outcome.target_side_supervision_only",
                "outcome labels must supervise after reality responds, not feed live detection",
            ));
        }
    }
    Ok(())
}

fn validate_ordered_steps(steps: &[SkillStepEvidence]) -> Result<(), TrainerError> {
    if steps.is_empty() || steps.len() > MAX_STEPS {
        return Err(invalid("ordered_steps", "must contain 1..64 steps"));
    }
    for (expected, step) in steps.iter().enumerate() {
        if step.step_index != expected as u32 {
            return Err(invalid(
                "ordered_steps",
                "step_index values must be consecutive and zero-based",
            ));
        }
        validate_id("chunk_id", &step.chunk_id)?;
        validate_project_relative_path("file_path", &step.file_path)?;
        validate_id("code_state_key", &step.code_state_key)?;
        validate_live_label_list("accepted_label_ids", &step.accepted_label_ids)?;
        validate_live_label_list("group_ids", &step.group_ids)?;
        if step.accepted_label_ids.is_empty() {
            return Err(invalid(
                "accepted_label_ids",
                "each skill step needs at least one live label",
            ));
        }
    }
    Ok(())
}

fn validate_step_templates(steps: &[SkillStepTemplate]) -> Result<(), TrainerError> {
    if steps.is_empty() || steps.len() > MAX_STEPS {
        return Err(invalid("ordered_steps", "must contain 1..64 steps"));
    }
    for (expected, step) in steps.iter().enumerate() {
        if step.step_index != expected as u32 {
            return Err(invalid(
                "ordered_steps",
                "template step_index values must be consecutive and zero-based",
            ));
        }
        validate_live_label_list("accepted_label_ids", &step.accepted_label_ids)?;
        validate_live_label_list("group_ids", &step.group_ids)?;
        if step.accepted_label_ids.is_empty() {
            return Err(invalid(
                "accepted_label_ids",
                "each skill step needs at least one live label",
            ));
        }
    }
    Ok(())
}

fn validate_transition_edges(
    edges: &[SkillTransitionEdge],
    step_count: usize,
) -> Result<(), TrainerError> {
    for edge in edges {
        if edge.from_step_index as usize >= step_count || edge.to_step_index as usize >= step_count
        {
            return Err(invalid("transition_edges", "edge step index out of range"));
        }
        if edge.from_step_index >= edge.to_step_index {
            return Err(invalid(
                "transition_edges",
                "edges must move forward in the ordered sequence",
            ));
        }
        validate_id("transition_edge.edge_label", &edge.edge_label)?;
    }
    Ok(())
}

fn templates_from_steps(steps: &[SkillStepEvidence]) -> Result<Vec<SkillStepTemplate>, TrainerError> {
    let mut templates = Vec::with_capacity(steps.len());
    for step in steps {
        let mut accepted_label_ids = step.accepted_label_ids.clone();
        accepted_label_ids.sort();
        let mut group_ids = step.group_ids.clone();
        group_ids.sort();
        templates.push(SkillStepTemplate {
            step_index: step.step_index,
            accepted_label_ids,
            group_ids,
        });
    }
    validate_step_templates(&templates)?;
    Ok(templates)
}

fn transition_edges_for_steps(steps: &[SkillStepTemplate]) -> Vec<SkillTransitionEdge> {
    steps
        .windows(2)
        .map(|pair| SkillTransitionEdge {
            from_step_index: pair[0].step_index,
            to_step_index: pair[1].step_index,
            edge_label: "sequence:next".to_string(),
        })
        .collect()
}

fn pattern_hash(steps: &[SkillStepTemplate]) -> Result<String, TrainerError> {
    validate_step_templates(steps)?;
    let mut hasher = Sha256::new();
    for step in steps {
        hasher.update(step.step_index.to_le_bytes());
        for label in &step.accepted_label_ids {
            hasher.update([0]);
            hasher.update(label.as_bytes());
        }
        for group in &step.group_ids {
            hasher.update([1]);
            hasher.update(group.as_bytes());
        }
    }
    Ok(format!("skill_pattern:{}", &hex::encode(hasher.finalize())[..24]))
}

fn stability_score(file_count: usize, support: u64) -> f64 {
    if support == 0 {
        return 0.0;
    }
    (file_count as f64 / support as f64).clamp(0.0, 1.0)
}

fn derive_skill_name(parent_group_ids: &[String], ordered_steps: &[SkillStepTemplate]) -> String {
    let source = parent_group_ids
        .first()
        .or_else(|| ordered_steps.first().and_then(|step| step.accepted_label_ids.first()))
        .map(String::as_str)
        .unwrap_or("unnamed");
    format!("auto_{}", slugify(source))
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').chars().take(64).collect()
}

fn validate_live_label_list(
    field: &str,
    values: &[String],
) -> Result<(), TrainerError> {
    validate_id_list(field, values, MAX_IDS_PER_STEP)?;
    for value in values {
        if is_target_only_live_label(value) {
            return Err(invalid(
                field,
                format!("{value} is target-side supervision and cannot identify a live skill"),
            ));
        }
    }
    Ok(())
}

fn validate_id_list(field: &str, values: &[String], max_items: usize) -> Result<(), TrainerError> {
    if values.len() > max_items {
        return Err(invalid(field, format!("too many ids: {}", values.len())));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        validate_id(field, value)?;
        if !seen.insert(value) {
            return Err(invalid(field, format!("duplicate id {value}")));
        }
    }
    Ok(())
}

fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    if value.trim().is_empty() {
        return Err(invalid(field, "must be non-empty"));
    }
    if value.len() > MAX_ID_BYTES || value.chars().any(char::is_control) {
        return Err(invalid(field, "must be single-line text up to 512 bytes"));
    }
    if value.contains("/mnt/d") {
        return Err(invalid(field, "must not reference legacy /mnt/d state"));
    }
    Ok(())
}

fn validate_project_relative_path(field: &str, value: &str) -> Result<(), TrainerError> {
    validate_id(field, value)?;
    if value.starts_with('/') || value.split('/').any(|part| part == "..") {
        return Err(invalid(field, "must be a project-relative path"));
    }
    Ok(())
}

fn validate_config(config: SkillDiscoveryConfig) -> Result<(), TrainerError> {
    if config.min_support == 0 {
        return Err(invalid("min_support", "must be positive"));
    }
    if !config.min_lift_over_cell_baseline.is_finite() {
        return Err(invalid(
            "min_lift_over_cell_baseline",
            "must be finite",
        ));
    }
    validate_finite_unit("min_confidence", config.min_confidence)
}

fn validate_finite_unit(field: &str, value: f64) -> Result<(), TrainerError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(invalid(field, "must be finite and within [0, 1]"));
    }
    Ok(())
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field.into(),
        "file": "file:crates/context-graph-mejepa-train/src/skill_sequence_discovery.rs",
        "remediation": "skill discovery must use live-observable ordered labels and target outcomes only for post-reality supervision"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(index: u32, chunk: &str, file: &str, label: &str, group: &str) -> SkillStepEvidence {
        SkillStepEvidence {
            step_index: index,
            chunk_id: chunk.to_string(),
            file_path: file.to_string(),
            code_state_key: format!("python:state:{chunk}"),
            accepted_label_ids: vec![label.to_string()],
            group_ids: vec![group.to_string()],
        }
    }

    fn episode(id: &str, file_suffix: &str, verdict: Option<SkillOutcomeVerdict>) -> SkillEpisodeRow {
        SkillEpisodeRow {
            episode_id: id.to_string(),
            proposed_skill_name: Some("import_contract_drift".to_string()),
            ordered_steps: vec![
                step(
                    0,
                    &format!("chunk:{file_suffix}:caller"),
                    &format!("pkg/{file_suffix}.py"),
                    "ast_surface:import_from",
                    "group:import_contract",
                ),
                step(
                    1,
                    &format!("chunk:{file_suffix}:callee"),
                    &format!("pkg/{file_suffix}.py"),
                    "pair_relation:caller_missing_symbol",
                    "group:import_contract",
                ),
            ],
            outcome: verdict.map(|verdict| SkillOutcomeObservation {
                outcome_label_id: "oracle:fail".to_string(),
                verdict,
                target_side_supervision_only: true,
            }),
            failure_evidence_set_ids: vec![format!("evidence:{id}")],
            cell_baseline_fail_rate: 0.20,
        }
    }

    #[test]
    fn discovery_promotes_ordered_skill_candidate() {
        let report = discover_skill_candidates(
            &[
                episode("episode:a", "a", Some(SkillOutcomeVerdict::Fail)),
                episode("episode:b", "b", Some(SkillOutcomeVerdict::Fail)),
            ],
            SkillDiscoveryConfig::default(),
            1_779_000_000_000,
        )
        .unwrap();

        assert_eq!(report.rejections, Vec::new());
        assert_eq!(report.candidates.len(), 1);
        let row = &report.candidates[0];
        assert_eq!(row.skill_name, "import_contract_drift");
        assert_eq!(row.promotion_status, SkillPromotionStatus::PromotionReady);
        assert_eq!(row.ordered_steps.len(), 2);
        assert_eq!(row.transition_edges[0].from_step_index, 0);
        assert_eq!(row.transition_edges[0].to_step_index, 1);
        assert!(row.lift_over_cell_baseline >= 0.79);
    }

    #[test]
    fn discovery_rejects_target_only_live_label() {
        let mut row = episode("episode:a", "a", Some(SkillOutcomeVerdict::Fail));
        row.ordered_steps[0].accepted_label_ids = vec!["oracle:pass".to_string()];

        let err = discover_skill_candidates(
            &[row],
            SkillDiscoveryConfig::default(),
            1_779_000_000_000,
        )
        .unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn discovery_rejects_missing_order() {
        let mut row = episode("episode:a", "a", Some(SkillOutcomeVerdict::Fail));
        row.ordered_steps[1].step_index = 3;

        let err = discover_skill_candidates(
            &[row],
            SkillDiscoveryConfig::default(),
            1_779_000_000_000,
        )
        .unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn discovery_rejects_low_support_overfit_candidate() {
        let report = discover_skill_candidates(
            &[episode("episode:a", "a", Some(SkillOutcomeVerdict::Fail))],
            SkillDiscoveryConfig::default(),
            1_779_000_000_000,
        )
        .unwrap();

        assert_eq!(report.candidates, Vec::new());
        assert_eq!(report.rejections.len(), 1);
        assert_eq!(report.rejections[0].reason, "low_support_overfit");
    }

    #[test]
    fn discovery_emits_pending_live_candidate_without_oracle_outcome() {
        let report = discover_skill_candidates(
            &[
                episode("episode:a", "a", None),
                episode("episode:b", "b", None),
            ],
            SkillDiscoveryConfig::default(),
            1_779_000_000_000,
        )
        .unwrap();

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].promotion_status, SkillPromotionStatus::ActiveLearning);
        assert_eq!(report.candidates[0].oracle_outcome_distribution.unknown, 2);
    }
}
