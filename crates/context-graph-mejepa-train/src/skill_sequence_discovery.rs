//! TASK-PY-G-118 skill-sequence discovery.
//!
//! A skill is an ordered consequence pattern over live-observable chunk labels.
//! Target-side oracle/test labels may score the pattern after reality responds,
//! but they are never allowed in the live inputs that identify the skill.

use crate::error::TrainerError;
use crate::skill_validation;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_STEPS: usize = 64;
const MAX_IDS_PER_STEP: usize = 128;
const MAX_EPISODES_PER_SKILL: usize = 4096;
const SOURCE_FILE: &str = "file:crates/context-graph-mejepa-train/src/skill_sequence_discovery.rs";
const REMEDIATION: &str =
    "skill discovery must use live-observable ordered labels and target outcomes only for post-reality supervision";

pub use crate::skill_sequence_types::*;
pub use crate::skill_validation::is_target_only_live_label;

impl SkillOutcomeDistribution {
    pub fn supervised_total(&self) -> u64 {
        self.pass + self.fail
    }
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
        validate_id_list(
            "code_state_keys",
            &self.code_state_keys,
            MAX_EPISODES_PER_SKILL,
        )?;
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
            return Err(invalid("lift_over_cell_baseline", "must be a finite value"));
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
    let mut usefulness_profiles = Vec::new();
    for (pattern_hash, acc) in groups {
        let metrics = acc.metrics();
        usefulness_profiles.push(acc.usefulness_profile(&pattern_hash, metrics));
        if acc.source_episode_ids.len() >= MAX_EPISODES_PER_SKILL
            && acc.support > MAX_EPISODES_PER_SKILL as u64
        {
            rejections.push(acc.rejection(
                pattern_hash,
                "too_broad_high_support_static_pattern",
                metrics,
            ));
            continue;
        }
        if acc.support < config.min_support {
            rejections.push(acc.rejection(pattern_hash, "low_support_overfit", metrics));
            continue;
        }
        let promotion_status = if acc.outcome_distribution.supervised_total() == 0 {
            if config.allow_pending_outcome_candidates {
                SkillPromotionStatus::ActiveLearning
            } else {
                rejections.push(acc.rejection(pattern_hash, "missing_target_supervision", metrics));
                continue;
            }
        } else if metrics.lift_over_cell_baseline < config.min_lift_over_cell_baseline
            || metrics.confidence < config.min_confidence
        {
            rejections.push(acc.rejection(pattern_hash, "below_usefulness_threshold", metrics));
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
        usefulness_profiles,
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

pub fn skill_candidate_kind_for_row(row: &Level2SkillRow) -> SkillCandidateKind {
    let supervised = row.oracle_outcome_distribution.supervised_total();
    if supervised == 0 {
        return SkillCandidateKind::NeutralDiagnostic;
    }
    let fail_rate = row.oracle_outcome_distribution.fail as f64 / supervised as f64;
    let pass_rate = row.oracle_outcome_distribution.pass as f64 / supervised as f64;
    let fail_lift = if fail_rate >= pass_rate {
        row.lift_over_cell_baseline
    } else {
        0.0
    };
    let pass_lift = if pass_rate > fail_rate {
        row.lift_over_cell_baseline
    } else {
        0.0
    };
    classify_candidate(
        fail_rate,
        pass_rate,
        fail_lift,
        pass_lift,
        skill_genericity_score_for_steps(&row.ordered_steps),
    )
}

pub fn skill_genericity_score_for_steps(steps: &[SkillStepTemplate]) -> f64 {
    genericity_score(steps)
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
        self.support += 1;
        self.baseline_fail_rate_sum += episode.cell_baseline_fail_rate;
        if self.source_episode_ids.len() < MAX_EPISODES_PER_SKILL {
            self.source_episode_ids.push(episode.episode_id.clone());
        }
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
        let genericity_score = genericity_score(&self.ordered_steps);
        if supervised == 0 {
            return CandidateMetrics {
                confidence: 0.0,
                lift_over_cell_baseline: 0.0,
                fail_rate: 0.0,
                pass_rate: 0.0,
                fail_lift: 0.0,
                pass_lift: 0.0,
                stability: stability_score(self.file_paths.len(), self.support),
                genericity_score,
                candidate_kind: SkillCandidateKind::NeutralDiagnostic,
                split_selection_weight: 0.0,
            };
        }
        let fail_rate = self.outcome_distribution.fail as f64 / supervised as f64;
        let pass_rate = self.outcome_distribution.pass as f64 / supervised as f64;
        let baseline_fail_rate = self.baseline_fail_rate_sum / self.support as f64;
        let fail_lift = fail_rate - baseline_fail_rate;
        let pass_lift = pass_rate - (1.0 - baseline_fail_rate);
        let lift_over_cell_baseline = fail_lift.max(pass_lift);
        let confidence = fail_rate.max(pass_rate);
        let candidate_kind =
            classify_candidate(fail_rate, pass_rate, fail_lift, pass_lift, genericity_score);
        let split_selection_weight = selection_weight(
            candidate_kind,
            lift_over_cell_baseline,
            confidence,
            stability_score(self.file_paths.len(), self.support),
            genericity_score,
        );
        CandidateMetrics {
            confidence,
            lift_over_cell_baseline,
            fail_rate,
            pass_rate,
            fail_lift,
            pass_lift,
            stability: stability_score(self.file_paths.len(), self.support),
            genericity_score,
            candidate_kind,
            split_selection_weight,
        }
    }

    fn usefulness_profile(
        &self,
        pattern_hash: &str,
        metrics: CandidateMetrics,
    ) -> SkillUsefulnessProfile {
        SkillUsefulnessProfile {
            pattern_hash: pattern_hash.to_string(),
            candidate_kind: metrics.candidate_kind,
            support: self.support,
            confidence: metrics.confidence,
            lift_over_cell_baseline: metrics.lift_over_cell_baseline,
            stability: metrics.stability,
            genericity_score: metrics.genericity_score,
            split_selection_weight: metrics.split_selection_weight,
            oracle_outcome_distribution: self.outcome_distribution,
            reason: usefulness_reason(metrics),
        }
    }

    fn rejection(
        &self,
        pattern_hash: String,
        reason: &str,
        metrics: CandidateMetrics,
    ) -> SkillCandidateRejection {
        SkillCandidateRejection {
            pattern_hash,
            candidate_kind: metrics.candidate_kind,
            reason: reason.to_string(),
            support: self.support,
            lift_over_cell_baseline: metrics.lift_over_cell_baseline,
            confidence: metrics.confidence,
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
    fail_rate: f64,
    pass_rate: f64,
    fail_lift: f64,
    pass_lift: f64,
    stability: f64,
    genericity_score: f64,
    candidate_kind: SkillCandidateKind,
    split_selection_weight: f64,
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

fn templates_from_steps(
    steps: &[SkillStepEvidence],
) -> Result<Vec<SkillStepTemplate>, TrainerError> {
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
    Ok(format!(
        "skill_pattern:{}",
        &hex::encode(hasher.finalize())[..24]
    ))
}

fn stability_score(file_count: usize, support: u64) -> f64 {
    if support == 0 {
        return 0.0;
    }
    (file_count as f64 / support as f64).clamp(0.0, 1.0)
}

fn genericity_score(steps: &[SkillStepTemplate]) -> f64 {
    let mut labels = Vec::new();
    for step in steps {
        labels.extend(step.accepted_label_ids.iter().map(String::as_str));
        labels.extend(step.group_ids.iter().map(String::as_str));
    }
    if labels.is_empty() {
        return 1.0;
    }
    let has_source_site = labels
        .iter()
        .any(|label| label.starts_with("source_site_relation:"));
    let has_pair_or_cross = labels.iter().any(|label| {
        label.starts_with("pair:")
            || label.starts_with("pair_relation:")
            || label.starts_with("group:cross_panel:")
    });
    let has_temporal = labels.iter().any(|label| {
        label.starts_with("slot:e2:")
            || label.starts_with("slot:e3:")
            || label.starts_with("slot:e4:")
            || label.contains("temporal")
    });
    let has_specific_ast = labels
        .iter()
        .any(|label| label.starts_with("ast_surface:") && !label.ends_with(":present"));
    let mut score = 1.0_f64;
    if has_source_site {
        score -= 0.35;
    }
    if has_pair_or_cross {
        score -= 0.25;
    }
    if has_temporal {
        score -= 0.20;
    }
    if has_specific_ast {
        score -= 0.10;
    }
    score.clamp(0.0, 1.0)
}

fn classify_candidate(
    fail_rate: f64,
    pass_rate: f64,
    fail_lift: f64,
    pass_lift: f64,
    genericity_score: f64,
) -> SkillCandidateKind {
    if genericity_score >= 0.70 && pass_rate >= 0.80 && pass_lift > 0.0 {
        return SkillCandidateKind::ContextNegativeEvidence;
    }
    if fail_rate >= pass_rate && fail_lift > 0.0 {
        return SkillCandidateKind::FailureSkill;
    }
    if pass_rate > fail_rate && pass_lift > 0.0 {
        return SkillCandidateKind::PassStabilitySkill;
    }
    if genericity_score >= 0.85 {
        return SkillCandidateKind::RejectOverbroadOrLeaky;
    }
    SkillCandidateKind::NeutralDiagnostic
}

fn selection_weight(
    kind: SkillCandidateKind,
    lift_over_cell_baseline: f64,
    confidence: f64,
    stability: f64,
    genericity_score: f64,
) -> f64 {
    let kind_weight = match kind {
        SkillCandidateKind::FailureSkill => 1.0,
        SkillCandidateKind::PassStabilitySkill => 0.80,
        SkillCandidateKind::ContextNegativeEvidence => 0.55,
        SkillCandidateKind::NeutralDiagnostic => 0.20,
        SkillCandidateKind::RejectOverbroadOrLeaky => 0.0,
    };
    let genericity_penalty = if matches!(kind, SkillCandidateKind::ContextNegativeEvidence) {
        0.85
    } else {
        1.0 - (genericity_score * 0.60)
    };
    (lift_over_cell_baseline.max(0.0) * confidence * stability * kind_weight * genericity_penalty)
        .max(0.0)
}

fn usefulness_reason(metrics: CandidateMetrics) -> String {
    match metrics.candidate_kind {
        SkillCandidateKind::FailureSkill => format!(
            "failure_skill: fail_rate={:.6}, fail_lift={:.6}, genericity={:.6}",
            metrics.fail_rate, metrics.fail_lift, metrics.genericity_score
        ),
        SkillCandidateKind::PassStabilitySkill => format!(
            "pass_stability_skill: pass_rate={:.6}, pass_lift={:.6}, genericity={:.6}",
            metrics.pass_rate, metrics.pass_lift, metrics.genericity_score
        ),
        SkillCandidateKind::ContextNegativeEvidence => format!(
            "context_negative_evidence: pass_rate={:.6}, pass_lift={:.6}, genericity={:.6}",
            metrics.pass_rate, metrics.pass_lift, metrics.genericity_score
        ),
        SkillCandidateKind::NeutralDiagnostic => format!(
            "neutral_diagnostic: confidence={:.6}, lift={:.6}, genericity={:.6}",
            metrics.confidence, metrics.lift_over_cell_baseline, metrics.genericity_score
        ),
        SkillCandidateKind::RejectOverbroadOrLeaky => format!(
            "reject_overbroad_or_leaky: confidence={:.6}, lift={:.6}, genericity={:.6}",
            metrics.confidence, metrics.lift_over_cell_baseline, metrics.genericity_score
        ),
    }
}

fn derive_skill_name(parent_group_ids: &[String], ordered_steps: &[SkillStepTemplate]) -> String {
    let source = parent_group_ids
        .first()
        .or_else(|| {
            ordered_steps
                .first()
                .and_then(|step| step.accepted_label_ids.first())
        })
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

fn validate_live_label_list(field: &str, values: &[String]) -> Result<(), TrainerError> {
    skill_validation::validate_live_id_list(
        SOURCE_FILE,
        REMEDIATION,
        field,
        values,
        MAX_IDS_PER_STEP,
    )
}

fn validate_id_list(field: &str, values: &[String], max_items: usize) -> Result<(), TrainerError> {
    skill_validation::validate_id_list(SOURCE_FILE, REMEDIATION, field, values, max_items)
}

fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_id(SOURCE_FILE, REMEDIATION, field, value)
}

fn validate_project_relative_path(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_project_relative_path(SOURCE_FILE, REMEDIATION, field, value)
}

fn validate_config(config: SkillDiscoveryConfig) -> Result<(), TrainerError> {
    if config.min_support == 0 {
        return Err(invalid("min_support", "must be positive"));
    }
    if !config.min_lift_over_cell_baseline.is_finite() {
        return Err(invalid("min_lift_over_cell_baseline", "must be finite"));
    }
    validate_finite_unit("min_confidence", config.min_confidence)
}

fn validate_finite_unit(field: &str, value: f64) -> Result<(), TrainerError> {
    skill_validation::validate_finite_unit(SOURCE_FILE, REMEDIATION, field, value)
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    skill_validation::invalid(SOURCE_FILE, REMEDIATION, field, message)
}

#[cfg(test)]
#[path = "skill_sequence_discovery_tests.rs"]
mod skill_sequence_discovery_tests;
