//! TASK-PY-G-118 production skill materialization from #413 label artifacts.
//!
//! This module turns the aiwonder Level-0 label compiler output into ordered
//! skill episodes. It deliberately streams `chunk_constellation_labels.jsonl`
//! because the full artifact is multi-GB, and it rejects target-side labels as
//! live skill identifiers.

use crate::chunk_skill_membership::{materialize_skill_memberships, SkillMaterialization};
use crate::error::{TrainerError, TrainerErrorCode};
use crate::label_bridge::{load_label_learning_bridge, LabelLearningBridge};
use crate::skill_sequence_discovery::{
    discover_skill_candidates, SkillDiscoveryConfig, SkillEpisodeRow, SkillOutcomeObservation,
    SkillOutcomeVerdict, SkillPromotionStatus, SkillStepEvidence,
};
use crate::skill_validation;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const SOURCE_FILE: &str =
    "file:crates/context-graph-mejepa-train/src/skill_corpus_materialization.rs";
const REMEDIATION: &str =
    "production skill materialization must stream aiwonder #413 labels, preserve live/target label boundaries, and avoid /mnt/d";
const MAX_STEP_LABELS: usize = 8;
const MAX_GROUP_IDS: usize = 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillCorpusMaterializationConfig {
    pub max_chunk_rows: Option<usize>,
    pub chunk_window_size: usize,
    pub max_steps_per_failure_episode: usize,
    pub max_materialized_skills: usize,
    pub discovery: SkillDiscoveryConfig,
}

impl Default for SkillCorpusMaterializationConfig {
    fn default() -> Self {
        Self {
            max_chunk_rows: None,
            chunk_window_size: 2,
            max_steps_per_failure_episode: 8,
            max_materialized_skills: 128,
            discovery: SkillDiscoveryConfig {
                min_support: 8,
                min_lift_over_cell_baseline: 0.05,
                min_confidence: 0.55,
                allow_pending_outcome_candidates: true,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillCorpusMaterializationReport {
    pub artifact_kind: String,
    pub schema_version: u32,
    pub label_root: String,
    pub accepted_registry_sha256: String,
    pub usefulness_metrics_sha256: String,
    pub learning_bridge_manifest_sha256: String,
    pub code_state_rows: usize,
    pub failure_evidence_rows: usize,
    pub chunk_rows_streamed: usize,
    pub chunk_rows_with_live_skill_labels: usize,
    pub chunk_rows_without_live_skill_labels: usize,
    pub generated_episode_count: usize,
    pub failure_episode_count: usize,
    pub chunk_window_episode_count: usize,
    pub candidate_count: usize,
    pub rejection_count: usize,
    pub materialized_skill_count: usize,
    pub materialized_membership_count: usize,
    pub materialized_reverse_index_count: usize,
    pub materialized_lifecycle_audit_count: usize,
    pub pass_episode_count: usize,
    pub fail_episode_count: usize,
    pub slot_identity_preserved: bool,
    pub flat_vector_concat_used: bool,
    pub target_labels_used_as_live_inputs: bool,
    pub mnt_d_active_reads_or_writes: bool,
    pub sample_episodes: Vec<SkillEpisodeSummary>,
    pub sample_skill_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillEpisodeSummary {
    pub episode_id: String,
    pub proposed_skill_name: Option<String>,
    pub step_count: usize,
    pub first_chunk_id: Option<String>,
    pub first_file_path: Option<String>,
    pub code_state_key: Option<String>,
    pub outcome: Option<SkillOutcomeVerdict>,
    pub failure_evidence_set_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillCorpusMaterializationBuild {
    pub report: SkillCorpusMaterializationReport,
    pub materialization: SkillMaterialization,
    pub discovery_rejection_count: usize,
}

pub fn build_skill_corpus_materialization(
    label_root: impl AsRef<Path>,
    config: SkillCorpusMaterializationConfig,
    created_at_unix_ms: i64,
) -> Result<SkillCorpusMaterializationBuild, TrainerError> {
    validate_config(&config)?;
    if created_at_unix_ms <= 0 {
        return Err(invalid("created_at_unix_ms", "must be positive"));
    }
    let label_root = label_root.as_ref();
    validate_path("label_root", label_root)?;

    let bridge = load_label_learning_bridge(label_root)?;
    let code_state_path =
        resolve_bridge_path(label_root, &bridge.manifest.paths.code_state_constellations);
    let failure_path =
        resolve_bridge_path(label_root, &bridge.manifest.paths.failure_evidence_sets);
    let chunk_path = resolve_bridge_path(
        label_root,
        &bridge.manifest.paths.chunk_constellation_labels,
    );
    validate_path("code_state_constellations", &code_state_path)?;
    validate_path("failure_evidence_sets", &failure_path)?;
    validate_path("chunk_constellation_labels", &chunk_path)?;

    let code_states = read_code_state_infos(&code_state_path)?;
    let baselines = baseline_fail_rates(&code_states);
    let mut episodes = Vec::new();
    let mut counts = EpisodeBuildCounts::default();

    let failure_episodes = read_failure_evidence_episodes(
        &bridge,
        &failure_path,
        &code_states,
        &baselines,
        config.max_steps_per_failure_episode,
    )?;
    counts.failure_episode_count = failure_episodes.len();
    episodes.extend(failure_episodes);

    let chunk_build = read_chunk_window_episodes(
        &bridge,
        &chunk_path,
        &code_states,
        &baselines,
        config.chunk_window_size,
        config.max_chunk_rows,
    )?;
    counts.rows_streamed = chunk_build.rows_streamed;
    counts.rows_with_live_skill_labels = chunk_build.rows_with_live_skill_labels;
    counts.rows_without_live_skill_labels = chunk_build.rows_without_live_skill_labels;
    counts.chunk_window_episode_count = chunk_build.episodes.len();
    episodes.extend(chunk_build.episodes);

    if episodes.is_empty() {
        return Err(invalid(
            "episodes",
            "no production skill episodes were generated",
        ));
    }
    let pass_episode_count = episodes
        .iter()
        .filter(|row| {
            row.outcome
                .as_ref()
                .map(|outcome| outcome.verdict == SkillOutcomeVerdict::Pass)
                .unwrap_or(false)
        })
        .count();
    let fail_episode_count = episodes
        .iter()
        .filter(|row| {
            row.outcome
                .as_ref()
                .map(|outcome| outcome.verdict == SkillOutcomeVerdict::Fail)
                .unwrap_or(false)
        })
        .count();
    let sample_episodes = summarize_episodes(&episodes);

    let discovery = discover_skill_candidates(&episodes, config.discovery, created_at_unix_ms)?;
    let mut candidates = discovery.candidates;
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate_rank(candidate)));
    let selected = candidates
        .into_iter()
        .take(config.max_materialized_skills)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(invalid(
            "skill_candidates",
            "no production skill candidates survived support/lift/leakage filters",
        ));
    }

    let mut materialization = SkillMaterialization {
        level2_skills: Vec::new(),
        chunk_memberships: Vec::new(),
        reverse_indexes: Vec::new(),
        lifecycle_audits: Vec::new(),
    };
    for skill in &selected {
        let partial = materialize_skill_memberships(skill, &episodes, created_at_unix_ms)?;
        extend_materialization(&mut materialization, partial);
    }

    let sample_skill_ids = materialization
        .level2_skills
        .iter()
        .take(16)
        .map(|row| row.skill_id.clone())
        .collect::<Vec<_>>();
    let report = SkillCorpusMaterializationReport {
        artifact_kind: "task_py_g_118_aiwonder_skill_corpus_materialization".to_string(),
        schema_version: 1,
        label_root: label_root.display().to_string(),
        accepted_registry_sha256: bridge.accepted_registry_sha256.clone(),
        usefulness_metrics_sha256: bridge.usefulness_metrics_sha256.clone(),
        learning_bridge_manifest_sha256: bridge.manifest_sha256.clone(),
        code_state_rows: code_states.len(),
        failure_evidence_rows: bridge.failure_evidence_sets.len(),
        chunk_rows_streamed: counts.rows_streamed,
        chunk_rows_with_live_skill_labels: counts.rows_with_live_skill_labels,
        chunk_rows_without_live_skill_labels: counts.rows_without_live_skill_labels,
        generated_episode_count: episodes.len(),
        failure_episode_count: counts.failure_episode_count,
        chunk_window_episode_count: counts.chunk_window_episode_count,
        candidate_count: selected.len(),
        rejection_count: discovery.rejections.len(),
        materialized_skill_count: materialization.level2_skills.len(),
        materialized_membership_count: materialization.chunk_memberships.len(),
        materialized_reverse_index_count: materialization.reverse_indexes.len(),
        materialized_lifecycle_audit_count: materialization.lifecycle_audits.len(),
        pass_episode_count,
        fail_episode_count,
        slot_identity_preserved: true,
        flat_vector_concat_used: false,
        target_labels_used_as_live_inputs: false,
        mnt_d_active_reads_or_writes: false,
        sample_episodes,
        sample_skill_ids,
    };
    Ok(SkillCorpusMaterializationBuild {
        report,
        materialization,
        discovery_rejection_count: discovery.rejections.len(),
    })
}

#[derive(Debug, Clone, Default)]
struct EpisodeBuildCounts {
    failure_episode_count: usize,
    chunk_window_episode_count: usize,
    rows_streamed: usize,
    rows_with_live_skill_labels: usize,
    rows_without_live_skill_labels: usize,
}

#[derive(Debug, Clone)]
struct CodeStateInfo {
    mutation_category: String,
    oracle_all_passed: bool,
    docker_label: String,
}

#[derive(Debug, Deserialize)]
struct CodeStateConstellationJsonRow {
    group_key: String,
    mutation_category: String,
    oracle_all_passed: bool,
    docker_label: String,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
}

#[derive(Debug, Deserialize)]
struct FailureEvidenceJsonRow {
    code_state_key: String,
    localization_mode: String,
    mutation_category: String,
    oracle_all_passed: bool,
    docker_label: String,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
    #[serde(default)]
    candidate_sample: Vec<FailureCandidateJsonRow>,
}

#[derive(Debug, Deserialize)]
struct FailureCandidateJsonRow {
    chunk_id: String,
    row_key: String,
    relative_path: String,
    #[serde(default)]
    byte_span: Vec<u64>,
    #[serde(default)]
    labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkConstellationJsonRow {
    row_key: String,
    chunk_id: String,
    task_instance_id: String,
    workspace_state: String,
    mutation_category: String,
    #[serde(default)]
    relative_path: Option<String>,
    #[serde(default)]
    byte_span: Vec<u64>,
    slot_identity_preserved: bool,
    flat_vector_concat_used: bool,
    #[serde(default)]
    live_predictor_labels: Vec<String>,
}

#[derive(Debug, Clone)]
struct StepSeed {
    row_key: String,
    byte_start: u64,
    evidence: SkillStepEvidence,
}

#[derive(Debug)]
struct ChunkEpisodeBuild {
    episodes: Vec<SkillEpisodeRow>,
    rows_streamed: usize,
    rows_with_live_skill_labels: usize,
    rows_without_live_skill_labels: usize,
}

fn read_code_state_infos(path: &Path) -> Result<BTreeMap<String, CodeStateInfo>, TrainerError> {
    let mut out = BTreeMap::new();
    for row in read_jsonl::<CodeStateConstellationJsonRow>(path)? {
        if !row.slot_identity_preserved || row.flat_vector_concat_used {
            return Err(invalid(
                "code_state_constellations",
                "code-state rows must preserve slot identity and reject flat-vector semantics",
            ));
        }
        validate_id("code_state.group_key", &row.group_key)?;
        out.insert(
            row.group_key.clone(),
            CodeStateInfo {
                mutation_category: row.mutation_category,
                oracle_all_passed: row.oracle_all_passed,
                docker_label: row.docker_label,
            },
        );
    }
    if out.is_empty() {
        return Err(invalid(
            "code_state_constellations",
            "must contain at least one code-state row",
        ));
    }
    Ok(out)
}

fn baseline_fail_rates(code_states: &BTreeMap<String, CodeStateInfo>) -> BTreeMap<String, f64> {
    let mut counts = BTreeMap::<String, (u64, u64)>::new();
    for row in code_states.values() {
        let entry = counts
            .entry(row.mutation_category.clone())
            .or_insert((0_u64, 0_u64));
        if row.oracle_all_passed {
            entry.1 += 1;
        } else {
            entry.0 += 1;
        }
    }
    counts
        .into_iter()
        .map(|(category, (fail, pass))| {
            let total = fail + pass;
            let rate = if total == 0 {
                0.5
            } else {
                fail as f64 / total as f64
            };
            (category, rate)
        })
        .collect()
}

fn read_failure_evidence_episodes(
    bridge: &LabelLearningBridge,
    path: &Path,
    code_states: &BTreeMap<String, CodeStateInfo>,
    baselines: &BTreeMap<String, f64>,
    max_steps: usize,
) -> Result<Vec<SkillEpisodeRow>, TrainerError> {
    let mut episodes = Vec::new();
    for row in read_jsonl::<FailureEvidenceJsonRow>(path)? {
        if !row.slot_identity_preserved || row.flat_vector_concat_used {
            return Err(invalid(
                "failure_evidence_sets",
                "failure evidence rows must preserve slot identity and reject flat-vector semantics",
            ));
        }
        let mut seeds = Vec::new();
        for candidate in &row.candidate_sample {
            if let Some(seed) = step_from_candidate(
                bridge,
                &row.code_state_key,
                candidate,
                candidate.labels.as_slice(),
            )? {
                seeds.push(seed);
            }
        }
        seeds.sort_by(|left, right| {
            (
                left.evidence.file_path.as_str(),
                left.byte_start,
                left.row_key.as_str(),
            )
                .cmp(&(
                    right.evidence.file_path.as_str(),
                    right.byte_start,
                    right.row_key.as_str(),
                ))
        });
        if seeds.is_empty() {
            continue;
        }
        let steps = seeds
            .into_iter()
            .take(max_steps)
            .enumerate()
            .map(|(idx, mut seed)| {
                seed.evidence.step_index = idx as u32;
                seed.evidence
            })
            .collect::<Vec<_>>();
        let info = code_states.get(&row.code_state_key);
        let mutation_category = info
            .map(|info| info.mutation_category.as_str())
            .unwrap_or(row.mutation_category.as_str());
        episodes.push(SkillEpisodeRow {
            episode_id: episode_id("failure", &[&row.code_state_key]),
            proposed_skill_name: Some(propose_skill_name(
                "failure",
                &row.localization_mode,
                &steps,
            )),
            ordered_steps: steps,
            outcome: Some(outcome(&row.docker_label, verdict(row.oracle_all_passed))),
            failure_evidence_set_ids: vec![row.code_state_key.clone()],
            cell_baseline_fail_rate: *baselines.get(mutation_category).unwrap_or(&0.5),
        });
    }
    Ok(episodes)
}

fn read_chunk_window_episodes(
    bridge: &LabelLearningBridge,
    path: &Path,
    code_states: &BTreeMap<String, CodeStateInfo>,
    baselines: &BTreeMap<String, f64>,
    window_size: usize,
    max_rows: Option<usize>,
) -> Result<ChunkEpisodeBuild, TrainerError> {
    let file = File::open(path).map_err(map_io_error)?;
    let reader = BufReader::new(file);
    let mut groups = BTreeMap::<String, Vec<StepSeed>>::new();
    let mut rows_streamed = 0_usize;
    let mut rows_with_live_skill_labels = 0_usize;
    let mut rows_without_live_skill_labels = 0_usize;

    for (idx, line) in reader.lines().enumerate() {
        if max_rows
            .map(|limit| rows_streamed >= limit)
            .unwrap_or(false)
        {
            break;
        }
        let line = line.map_err(map_io_error)?;
        if line.trim().is_empty() {
            continue;
        }
        let row: ChunkConstellationJsonRow = serde_json::from_str(&line).map_err(|err| {
            invalid(
                "chunk_constellation_labels",
                format!("{}:{} failed JSON parse: {err}", path.display(), idx + 1),
            )
        })?;
        rows_streamed += 1;
        if !row.slot_identity_preserved || row.flat_vector_concat_used {
            return Err(invalid(
                "chunk_constellation_labels",
                "chunk rows must preserve slot identity and reject flat-vector semantics",
            ));
        }
        let Some(relative_path) = row.relative_path.clone() else {
            rows_without_live_skill_labels += 1;
            continue;
        };
        let code_state_key = code_state_key(
            &row.task_instance_id,
            &row.mutation_category,
            &row.workspace_state,
        );
        let Some(seed) = step_from_chunk_row(bridge, &code_state_key, &relative_path, &row)? else {
            rows_without_live_skill_labels += 1;
            continue;
        };
        rows_with_live_skill_labels += 1;
        groups.entry(code_state_key).or_default().push(seed);
    }

    let mut episodes = Vec::new();
    for (code_state_key, mut seeds) in groups {
        seeds.sort_by(|left, right| {
            (
                left.evidence.file_path.as_str(),
                left.byte_start,
                left.row_key.as_str(),
            )
                .cmp(&(
                    right.evidence.file_path.as_str(),
                    right.byte_start,
                    right.row_key.as_str(),
                ))
        });
        let Some(info) = code_states.get(&code_state_key) else {
            continue;
        };
        if seeds.is_empty() {
            continue;
        }
        let window_count = if seeds.len() == 1 {
            1
        } else {
            seeds.len().saturating_sub(window_size).saturating_add(1)
        };
        for start in 0..window_count {
            let end = (start + window_size).min(seeds.len());
            let mut steps = seeds[start..end]
                .iter()
                .cloned()
                .enumerate()
                .map(|(idx, mut seed)| {
                    seed.evidence.step_index = idx as u32;
                    seed.evidence
                })
                .collect::<Vec<_>>();
            if steps.is_empty() {
                continue;
            }
            if seeds.len() > 1 && steps.len() < 2 {
                continue;
            }
            episodes.push(SkillEpisodeRow {
                episode_id: episode_id(
                    "chunk_window",
                    &[
                        &code_state_key,
                        &steps
                            .iter()
                            .map(|step| step.chunk_id.as_str())
                            .collect::<Vec<_>>()
                            .join("|"),
                    ],
                ),
                proposed_skill_name: Some(propose_skill_name(
                    "chunk_window",
                    &info.mutation_category,
                    &steps,
                )),
                ordered_steps: std::mem::take(&mut steps),
                outcome: Some(outcome(&info.docker_label, verdict(info.oracle_all_passed))),
                failure_evidence_set_ids: if info.oracle_all_passed {
                    Vec::new()
                } else {
                    vec![code_state_key.clone()]
                },
                cell_baseline_fail_rate: *baselines.get(&info.mutation_category).unwrap_or(&0.5),
            });
        }
    }

    Ok(ChunkEpisodeBuild {
        episodes,
        rows_streamed,
        rows_with_live_skill_labels,
        rows_without_live_skill_labels,
    })
}

fn step_from_candidate(
    bridge: &LabelLearningBridge,
    code_state_key: &str,
    candidate: &FailureCandidateJsonRow,
    labels: &[String],
) -> Result<Option<StepSeed>, TrainerError> {
    let (accepted_label_ids, group_ids) = select_live_skill_labels(bridge, labels)?;
    if accepted_label_ids.is_empty() || group_ids.is_empty() {
        return Ok(None);
    }
    Ok(Some(StepSeed {
        row_key: candidate.row_key.clone(),
        byte_start: candidate.byte_span.first().copied().unwrap_or(0),
        evidence: SkillStepEvidence {
            step_index: 0,
            chunk_id: candidate.chunk_id.clone(),
            file_path: candidate.relative_path.clone(),
            code_state_key: code_state_key.to_string(),
            accepted_label_ids,
            group_ids,
        },
    }))
}

fn step_from_chunk_row(
    bridge: &LabelLearningBridge,
    code_state_key: &str,
    relative_path: &str,
    row: &ChunkConstellationJsonRow,
) -> Result<Option<StepSeed>, TrainerError> {
    let (accepted_label_ids, group_ids) =
        select_live_skill_labels(bridge, &row.live_predictor_labels)?;
    if accepted_label_ids.is_empty() || group_ids.is_empty() {
        return Ok(None);
    }
    Ok(Some(StepSeed {
        row_key: row.row_key.clone(),
        byte_start: row.byte_span.first().copied().unwrap_or(0),
        evidence: SkillStepEvidence {
            step_index: 0,
            chunk_id: row.chunk_id.clone(),
            file_path: relative_path.to_string(),
            code_state_key: code_state_key.to_string(),
            accepted_label_ids,
            group_ids,
        },
    }))
}

fn select_live_skill_labels(
    bridge: &LabelLearningBridge,
    labels: &[String],
) -> Result<(Vec<String>, Vec<String>), TrainerError> {
    let mut accepted_by_priority = Vec::<(u8, String)>::new();
    let mut groups = BTreeSet::<String>::new();
    for label in labels {
        validate_id("label", label)?;
        if target_or_corpus_only_label(label) {
            continue;
        }
        if bridge.accepted_label(label).is_none() {
            continue;
        }
        if label.starts_with("group:") {
            groups.insert(label.clone());
        } else if let Some(priority) = skill_label_priority(label) {
            accepted_by_priority.push((priority, label.clone()));
        }
    }
    accepted_by_priority.sort();
    let mut accepted = Vec::new();
    for (_, label) in accepted_by_priority {
        if !accepted.contains(&label) {
            accepted.push(label);
        }
        if accepted.len() >= MAX_STEP_LABELS {
            break;
        }
    }
    let groups = groups.into_iter().take(MAX_GROUP_IDS).collect::<Vec<_>>();
    Ok((accepted, groups))
}

fn skill_label_priority(label: &str) -> Option<u8> {
    if label.starts_with("ast_surface:") {
        Some(0)
    } else if label.starts_with("path_surface:") {
        Some(1)
    } else if label.starts_with("source_site_relation:") {
        Some(2)
    } else if label.starts_with("pair:") {
        Some(3)
    } else if label.starts_with("slot:") && specific_slot_label(label) {
        Some(4)
    } else {
        None
    }
}

fn specific_slot_label(label: &str) -> bool {
    !(label.ends_with(":present")
        || label.contains(":dim:")
        || label.contains(":norm_unit_band")
        || label.contains(":family:"))
}

fn target_or_corpus_only_label(label: &str) -> bool {
    if crate::skill_sequence_discovery::is_target_only_live_label(label) {
        return true;
    }
    matches!(
        label.split_once(':').map(|(family, _)| family),
        Some("mutation" | "mutation_micro" | "mutation_mechanism")
    )
}

fn outcome(label: &str, verdict: SkillOutcomeVerdict) -> SkillOutcomeObservation {
    SkillOutcomeObservation {
        outcome_label_id: format!("docker:{label}"),
        verdict,
        target_side_supervision_only: true,
    }
}

fn verdict(oracle_all_passed: bool) -> SkillOutcomeVerdict {
    if oracle_all_passed {
        SkillOutcomeVerdict::Pass
    } else {
        SkillOutcomeVerdict::Fail
    }
}

fn candidate_rank(skill: &crate::skill_sequence_discovery::Level2SkillRow) -> (u8, u64, i64, i64) {
    let status = match skill.promotion_status {
        SkillPromotionStatus::PromotionReady => 3,
        SkillPromotionStatus::OperatorApproved => 2,
        SkillPromotionStatus::ActiveLearning => 1,
        SkillPromotionStatus::Demoted => 0,
    };
    (
        status,
        skill.support,
        (skill.lift_over_cell_baseline * 1_000_000.0) as i64,
        (skill.confidence * 1_000_000.0) as i64,
    )
}

fn code_state_key(
    task_instance_id: &str,
    mutation_category: &str,
    workspace_state: &str,
) -> String {
    format!("{task_instance_id}|{mutation_category}|{workspace_state}")
}

fn episode_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    format!("episode:{prefix}:{}", &hex::encode(hasher.finalize())[..24])
}

fn propose_skill_name(prefix: &str, context: &str, steps: &[SkillStepEvidence]) -> String {
    let label = steps
        .first()
        .and_then(|step| step.accepted_label_ids.first())
        .map(String::as_str)
        .unwrap_or("unknown");
    format!(
        "{}_{}_{}",
        slugify(prefix),
        slugify(context),
        slugify(label)
    )
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
    out.trim_matches('_').chars().take(72).collect()
}

fn summarize_episodes(episodes: &[SkillEpisodeRow]) -> Vec<SkillEpisodeSummary> {
    let mut selected = Vec::new();
    let mut seen_kinds = BTreeSet::new();
    for episode in episodes {
        let kind = match episode.outcome.as_ref().map(|outcome| outcome.verdict) {
            Some(SkillOutcomeVerdict::Pass) => "pass",
            Some(SkillOutcomeVerdict::Fail) => "fail",
            _ => "unknown",
        };
        let multistep = if episode.ordered_steps.len() > 1 {
            "multi"
        } else {
            "single"
        };
        let key = format!("{kind}:{multistep}");
        if seen_kinds.insert(key) {
            selected.push(SkillEpisodeSummary {
                episode_id: episode.episode_id.clone(),
                proposed_skill_name: episode.proposed_skill_name.clone(),
                step_count: episode.ordered_steps.len(),
                first_chunk_id: episode
                    .ordered_steps
                    .first()
                    .map(|step| step.chunk_id.clone()),
                first_file_path: episode
                    .ordered_steps
                    .first()
                    .map(|step| step.file_path.clone()),
                code_state_key: episode
                    .ordered_steps
                    .first()
                    .map(|step| step.code_state_key.clone()),
                outcome: episode.outcome.as_ref().map(|outcome| outcome.verdict),
                failure_evidence_set_ids: episode.failure_evidence_set_ids.clone(),
            });
        }
        if selected.len() >= 8 {
            break;
        }
    }
    selected
}

fn extend_materialization(target: &mut SkillMaterialization, source: SkillMaterialization) {
    target.level2_skills.extend(source.level2_skills);
    target.chunk_memberships.extend(source.chunk_memberships);
    target.reverse_indexes.extend(source.reverse_indexes);
    target.lifecycle_audits.extend(source.lifecycle_audits);
}

fn resolve_bridge_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, TrainerError> {
    let file = File::open(path).map_err(map_io_error)?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(map_io_error)?;
        if line.trim().is_empty() {
            continue;
        }
        let row = serde_json::from_str(&line).map_err(|err| {
            invalid(
                "jsonl",
                format!("{}:{} failed JSON parse: {err}", path.display(), idx + 1),
            )
        })?;
        rows.push(row);
    }
    Ok(rows)
}

fn validate_config(config: &SkillCorpusMaterializationConfig) -> Result<(), TrainerError> {
    if config.chunk_window_size == 0 || config.chunk_window_size > 4 {
        return Err(invalid("chunk_window_size", "must be in 1..=4"));
    }
    if config.max_steps_per_failure_episode == 0 || config.max_steps_per_failure_episode > 64 {
        return Err(invalid(
            "max_steps_per_failure_episode",
            "must be in 1..=64",
        ));
    }
    if config.max_materialized_skills == 0 {
        return Err(invalid("max_materialized_skills", "must be positive"));
    }
    Ok(())
}

fn validate_path(field: &str, path: &Path) -> Result<(), TrainerError> {
    let value = path.display().to_string();
    if value.contains("/mnt/d") {
        return Err(invalid(
            field,
            "active production paths must not use /mnt/d",
        ));
    }
    validate_id(field, &value)
}

fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_id(SOURCE_FILE, REMEDIATION, field, value)
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field.into(),
        "file": SOURCE_FILE,
        "remediation": REMEDIATION
    }))
}

fn map_io_error(err: std::io::Error) -> TrainerError {
    invalid("io", err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn fixture_materializes_realistic_chunk_skill_rows() {
        let temp = tempfile::tempdir().unwrap();
        write_fixture(temp.path());
        let mut config = SkillCorpusMaterializationConfig::default();
        config.discovery.min_support = 2;
        config.discovery.min_lift_over_cell_baseline = 0.0;
        config.discovery.min_confidence = 0.5;
        config.max_materialized_skills = 8;

        let build = build_skill_corpus_materialization(temp.path(), config, 1_779_100_000_000)
            .expect("fixture materialization");

        assert!(build.report.generated_episode_count >= 3);
        assert_eq!(build.report.failure_episode_count, 2);
        assert!(build.report.chunk_window_episode_count >= 1);
        assert!(build.report.materialized_skill_count > 0);
        assert!(build.report.materialized_membership_count > 0);
        assert!(!build.report.target_labels_used_as_live_inputs);
        assert!(!build.report.flat_vector_concat_used);
        assert!(build
            .materialization
            .level2_skills
            .iter()
            .all(|row| row.live_input_allowed));
    }

    fn write_fixture(root: &Path) {
        let accepted = [
            "ast_surface:call_api_surface",
            "ast_surface:function_contract_surface",
            "group:semantic_content_panel:present",
            "group:cross_panel:high_disagreement_cluster:medium",
        ];
        fs::write(
            root.join("accepted_label_registry.jsonl"),
            accepted
                .iter()
                .map(|label| {
                    format!(
                        r#"{{"artifact_kind":"python_auto_label_discovery_accepted_label_registry_row","schema_version":1,"formula_version":"unit","label_id":"{label}","label_hash":"label:{hash}","family":"{family}","status":"accepted_descriptive","support":10,"fail_rate":0.5,"lift_over_weighted_category_baseline":0.1,"live_prediction_input_allowed":true,"learning_update_policy":{{"baseline_seeded_from":"TASK-PY-G-117","online_rescore":true,"promote_demote_with_new_outcomes":true,"target_labels_are_supervision_only":true}}}}"#,
                        hash = &hex::encode(Sha256::digest(label.as_bytes()))[..24],
                        family = label.split(':').next().unwrap()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        fs::write(
            root.join("label_usefulness_metrics.jsonl"),
            accepted
                .iter()
                .map(|label| {
                    format!(
                        r#"{{"label_id":"{label}","family":"{family}","status":"accepted_descriptive","support":10,"leaky_target":false,"lift_over_weighted_category_baseline":0.1}}"#,
                        family = label.split(':').next().unwrap()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        fs::write(
            root.join("code_state_constellations.jsonl"),
            [
                code_state("task1|off_by_one|mutation_off_by_one", "off_by_one", false),
                code_state("task2|off_by_one|mutation_off_by_one", "off_by_one", false),
                code_state("task3|known_good|base", "known_good", true),
            ]
            .join("\n"),
        )
        .unwrap();
        fs::write(
            root.join("failure_evidence_sets.jsonl"),
            [
                failure_set("task1|off_by_one|mutation_off_by_one", "task1", false),
                failure_set("task2|off_by_one|mutation_off_by_one", "task2", false),
            ]
            .join("\n"),
        )
        .unwrap();
        fs::write(
            root.join("chunk_constellation_labels.jsonl"),
            [
                chunk_row(
                    "task1",
                    "mutation_off_by_one",
                    "off_by_one",
                    "chunk:f1",
                    false,
                    10,
                ),
                chunk_row(
                    "task1",
                    "mutation_off_by_one",
                    "off_by_one",
                    "chunk:f2",
                    false,
                    20,
                ),
                chunk_row(
                    "task2",
                    "mutation_off_by_one",
                    "off_by_one",
                    "chunk:f3",
                    false,
                    10,
                ),
                chunk_row(
                    "task2",
                    "mutation_off_by_one",
                    "off_by_one",
                    "chunk:f4",
                    false,
                    20,
                ),
                chunk_row("task3", "base", "known_good", "chunk:p1", true, 10),
                chunk_row("task3", "base", "known_good", "chunk:p2", true, 20),
            ]
            .join("\n"),
        )
        .unwrap();
        let manifest = json!({
            "artifact_kind": "python_auto_label_discovery_learning_bridge_manifest",
            "schema_version": 1,
            "formula_version": "unit",
            "purpose": "unit",
            "paths": {
                "accepted_label_registry": root.join("accepted_label_registry.jsonl"),
                "label_usefulness_metrics": root.join("label_usefulness_metrics.jsonl"),
                "chunk_constellation_labels": root.join("chunk_constellation_labels.jsonl"),
                "code_state_constellations": root.join("code_state_constellations.jsonl"),
                "failure_evidence_sets": root.join("failure_evidence_sets.jsonl")
            },
            "consumers": {"ReplayBufferRow.cell_id": "label aware"},
            "dynamic_learning_policy": {
                "baseline_only": true,
                "new_data_uses_same_compiler": true,
                "usefulness_rescored_over_time": true,
                "target_outcomes_supervise_but_are_not_live_inputs": true,
                "slot_identity_preserved": true,
                "flat_vector_concat_used": false
            }
        });
        fs::write(
            root.join("learning_bridge_manifest.json"),
            manifest.to_string(),
        )
        .unwrap();
    }

    fn code_state(key: &str, mutation: &str, pass: bool) -> String {
        format!(
            r#"{{"group_key":"{key}","mutation_category":"{mutation}","oracle_all_passed":{pass},"docker_label":"{docker}","labels":["group_scope:code_state"],"slot_identity_preserved":true,"flat_vector_concat_used":false}}"#,
            docker = if pass {
                "docker_resolved_clean"
            } else {
                "target_and_regression_tests_failed"
            }
        )
    }

    fn failure_set(key: &str, task: &str, pass: bool) -> String {
        format!(
            r#"{{"code_state_key":"{key}","localization_mode":"localization:multi_point_evidence_set","mutation_category":"off_by_one","oracle_all_passed":{pass},"docker_label":"target_and_regression_tests_failed","labels":["evidence:source_path_match"],"slot_identity_preserved":true,"flat_vector_concat_used":false,"candidate_sample":[{cand1},{cand2}]}}"#,
            cand1 = candidate(task, "a", 10),
            cand2 = candidate(task, "b", 20)
        )
    }

    fn candidate(task: &str, suffix: &str, start: u64) -> String {
        format!(
            r#"{{"chunk_id":"chunk:{task}:{suffix}","row_key":"{task}|row|{suffix}","relative_path":"pkg/{task}.py","byte_span":[{start},{end}],"labels":["ast_surface:call_api_surface","ast_surface:function_contract_surface","group:semantic_content_panel:present","group:cross_panel:high_disagreement_cluster:medium","oracle:fail","mutation_micro:numeric_boundary_shift"]}}"#,
            end = start + 5
        )
    }

    fn chunk_row(
        task: &str,
        workspace: &str,
        mutation: &str,
        chunk_id: &str,
        pass: bool,
        start: u64,
    ) -> String {
        format!(
            r#"{{"row_key":"{task}|{workspace}|{chunk_id}","chunk_id":"{chunk_id}","task_instance_id":"{task}","workspace_state":"{workspace}","mutation_category":"{mutation}","relative_path":"pkg/{task}.py","byte_span":[{start},{end}],"slot_identity_preserved":true,"flat_vector_concat_used":false,"live_predictor_labels":["ast_surface:call_api_surface","ast_surface:function_contract_surface","group:semantic_content_panel:present","group:cross_panel:high_disagreement_cluster:medium","oracle:{oracle}","mutation_micro:numeric_boundary_shift"]}}"#,
            end = start + 5,
            oracle = if pass { "pass" } else { "fail" }
        )
    }
}
