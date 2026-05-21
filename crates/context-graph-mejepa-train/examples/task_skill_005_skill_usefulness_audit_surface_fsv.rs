use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, lifecycle_audit_id_from_parts, materialize_skill_memberships,
    read_all_chunk_skill_membership_rows, read_all_level2_skill_rows,
    read_all_skill_lifecycle_audit_rows, write_skill_lifecycle_audit_row_sync_readback,
    write_skill_materialization_sync_readback, SkillLifecycleAuditRow, SkillLifecycleDecision,
};
use context_graph_mejepa_train::skill_linkage::{
    open_skill_linkage_rocksdb, skill_usefulness_audit, ChunkSourceIndex, ChunkSourceRow,
    SkillAuditRejectedPattern, SkillUsefulnessAuditOptions,
};
use context_graph_mejepa_train::skill_sequence_discovery::{
    skill_id_from_parts, Level2SkillRow, SkillCandidateKind, SkillEpisodeRow,
    SkillOutcomeDistribution, SkillOutcomeObservation, SkillOutcomeVerdict, SkillPromotionStatus,
    SkillStepEvidence, SkillStepTemplate, SkillTransitionEdge, SKILL_SEQUENCE_SCHEMA_VERSION,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_779_406_400_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = parse_fsv_root();
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;
    let before_tree = tree_state(&fsv_root)?;
    let db_path = fsv_root.join("skill-usefulness-audit.rocksdb");
    let db = open_skill_linkage_rocksdb(&db_path, true)?;
    let before_counts = cf_counts(&db)?;

    let mut source_index = ChunkSourceIndex::default();
    let mut expected_code_state_keys = BTreeSet::new();
    let seeded = seed_skill_catalog(&db, &mut source_index, &mut expected_code_state_keys)?;
    write_demote_audit(
        &db,
        "skill:blanket_support_expansion_regressed:fsv",
        NOW + 900,
    )?;
    db.flush()?;
    let after_seed_counts = cf_counts(&db)?;
    drop(db);

    let readback_db = open_skill_linkage_rocksdb(&db_path, false)?;
    let expected_code_state_keys = expected_code_state_keys.into_iter().collect::<Vec<_>>();
    let audit = skill_usefulness_audit(
        &readback_db,
        Some(&source_index),
        SkillUsefulnessAuditOptions {
            limit: 20,
            representative_example_limit: 2,
            expected_code_state_keys: {
                let mut keys = expected_code_state_keys.clone();
                keys.push("state:skill_gap:uncovered_code_state".to_string());
                keys
            },
            unknown_ordered_constellation_ids: vec![
                "unknown_constellation:swap_variable_multi_chunk:001".to_string(),
                "unknown_constellation:subtle_flip_wrapper_context:002".to_string(),
            ],
            rejected_patterns: vec![
                SkillAuditRejectedPattern {
                    pattern_id: "blanket_skill_expansion_cap_512".to_string(),
                    candidate_kind: SkillCandidateKind::RejectOverbroadOrLeaky,
                    reason: "blanket coverage expansion regressed #392 correlation and ECE"
                        .to_string(),
                    support: 512,
                    oracle_outcome_distribution: SkillOutcomeDistribution {
                        pass: 256,
                        fail: 256,
                        unknown: 0,
                    },
                    metric_before: Some(0.8497869968),
                    metric_after: Some(0.8055247068),
                    fsv_artifact: Some("/zfs/archive/contextgraph/fsv/task-skill-006-full-corpus-skill-rematerialization-fsv/run-20260520T233926Z/task_py_g_392_temporal_trajectory_features_fsv.json".to_string()),
                },
                SkillAuditRejectedPattern {
                    pattern_id: "generic_background_import_as_failure_skill".to_string(),
                    candidate_kind: SkillCandidateKind::RejectOverbroadOrLeaky,
                    reason: "generic high-support pass-skewed boilerplate rejected as failure skill"
                        .to_string(),
                    support: 558,
                    oracle_outcome_distribution: SkillOutcomeDistribution {
                        pass: 552,
                        fail: 6,
                        unknown: 0,
                    },
                    metric_before: Some(0.8497869968),
                    metric_after: Some(0.8055247068),
                    fsv_artifact: Some("https://github.com/ChrisRoyse/contextgraph/issues/551#issuecomment-4503007223".to_string()),
                },
            ],
            source_fsv_artifact: Some(
                "/zfs/archive/contextgraph/fsv/task-skill-005-skill-usefulness-audit-surface-fsv/"
                    .to_string(),
            ),
        },
    )?;

    let skill_rows = read_all_level2_skill_rows(&readback_db)?;
    let membership_rows = read_all_chunk_skill_membership_rows(&readback_db)?;
    let lifecycle_rows = read_all_skill_lifecycle_audit_rows(&readback_db)?;
    let readback_counts = cf_counts(&readback_db)?;
    let rows_by_name = audit
        .skill_rows
        .iter()
        .map(|row| (row.skill_name.clone(), row))
        .collect::<BTreeMap<_, _>>();

    let five_patterns_visible = [
        "wrong_file_test_oracle_import_chain",
        "source_site_function_contract_risk_chain",
        "syntax_overlap_compile_break",
        "api_return_delegation_wrapper_chain",
        "stable_background_import_and_impl_scaffold",
    ]
    .iter()
    .all(|name| rows_by_name.contains_key(*name));
    let stable_background_context = rows_by_name
        .get("stable_background_import_and_impl_scaffold")
        .is_some_and(|row| {
            row.candidate_kind == SkillCandidateKind::ContextNegativeEvidence
                && row.oracle_outcome_distribution.pass > row.oracle_outcome_distribution.fail
        });
    let blanket_regression_visible = audit
        .gap_view
        .metric_regression_rejections
        .iter()
        .any(|row| row.pattern_id == "blanket_skill_expansion_cap_512")
        && audit
            .gap_view
            .demoted_or_regressing_skill_ids
            .iter()
            .any(|id| id == "skill:blanket_support_expansion_regressed:fsv");
    let source_bytes_visible = rows_by_name.values().take(5).all(|row| {
        row.representative_sequences
            .iter()
            .any(|seq| seq.source.source_text.is_some())
    });
    let live_target_boundary_pass = audit
        .skill_rows
        .iter()
        .flat_map(|row| row.live_input_label_ids.iter())
        .all(|label| !target_side_label(label))
        && audit
            .skill_rows
            .iter()
            .any(|row| !row.target_supervision_label_ids.is_empty())
        && !audit.target_outcomes_used_as_live_inputs;
    let gap_views_pass = audit.gap_view.uncovered_code_state_count == 1
        && audit.gap_view.unknown_ordered_constellation_ids.len() == 2
        && !audit.gap_view.generic_high_support_rejections.is_empty();
    let readback_pass = readback_counts == after_seed_counts
        && skill_rows.len() == seeded.len()
        && membership_rows.len() >= seeded.len() * 2
        && lifecycle_rows.len() == seeded.len() + 1;
    let invariant_pass = audit.audit_buckets_sum_to_persisted_skill_count
        && audit.no_new_prediction_head_introduced
        && !audit.hidden_intent_inference_used
        && !audit.flat_vector_concat_used;
    let all_passed = five_patterns_visible
        && stable_background_context
        && blanket_regression_visible
        && source_bytes_visible
        && live_target_boundary_pass
        && gap_views_pass
        && readback_pass
        && invariant_pass;

    let report_path = fsv_root.join("task_skill_005_skill_usefulness_audit_surface_fsv.json");
    let report = json!({
        "artifact_kind": "task_skill_005_skill_usefulness_audit_surface_fsv",
        "schema_version": 1,
        "task_id": "TASK-SKILL-005",
        "issue": "https://github.com/ChrisRoyse/contextgraph/issues/557",
        "passes": all_passed,
        "source_of_truth": {
            "db_path": db_path,
            "cfs": [
                CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
                CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
                CF_MEJEPA_SKILL_REVERSE_INDEX,
                CF_MEJEPA_SKILL_LIFECYCLE_AUDIT
            ]
        },
        "before": {"tree": before_tree, "cf_counts": before_counts},
        "after_seed": {"cf_counts": after_seed_counts},
        "readback": {
            "cf_counts": readback_counts,
            "skill_rows": skill_rows,
            "membership_row_count": membership_rows.len(),
            "lifecycle_rows": lifecycle_rows
        },
        "audit": audit,
        "acceptance": {
            "five_patterns_visible": five_patterns_visible,
            "stable_background_context_not_failure": stable_background_context,
            "blanket_regression_visible": blanket_regression_visible,
            "source_bytes_visible": source_bytes_visible,
            "live_target_boundary_pass": live_target_boundary_pass,
            "gap_views_pass": gap_views_pass,
            "readback_pass": readback_pass,
            "invariant_pass": invariant_pass
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let readback_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if readback_report != report || !all_passed {
        return Err(format!("TASK-SKILL-005 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&readback_report)?);
    Ok(())
}

fn seed_skill_catalog(
    db: &rocksdb::DB,
    source_index: &mut ChunkSourceIndex,
    expected_code_state_keys: &mut BTreeSet<String>,
) -> Result<Vec<Level2SkillRow>, Box<dyn std::error::Error>> {
    let specs = vec![
        PatternSpec::new(
            "wrong_file_test_oracle_import_chain",
            125,
            0,
            "tests/test_wrong_surface.py",
            &[
                (
                    "ast_surface:import_surface",
                    "source_site_relation:wrong_file_edit",
                    "group:cross_panel:high_disagreement_cluster",
                ),
                (
                    "ast_surface:call_api_surface",
                    "source_site_relation:wrong_file_edit",
                    "slot:e3:temporal_region:stable",
                ),
            ],
        ),
        PatternSpec::new(
            "source_site_function_contract_risk_chain",
            100,
            4,
            "pkg/source.py",
            &[
                (
                    "ast_surface:function_contract_surface",
                    "source_site_relation:near_patch_site_2kb",
                    "group:cross_panel:high_disagreement_cluster",
                ),
                (
                    "ast_surface:control_flow_surface",
                    "source_site_relation:same_file_distant_context",
                    "pair_relation:contract_control_flow_shift",
                ),
            ],
        ),
        PatternSpec::new(
            "syntax_overlap_compile_break",
            32,
            0,
            "pkg/broken.py",
            &[
                (
                    "ast_surface:syntax_error_surface",
                    "source_site_relation:exact_patch_line_overlap",
                    "group:cross_panel:high_disagreement_cluster",
                ),
                (
                    "ast_surface:return_value_surface",
                    "source_site_relation:exact_patch_line_overlap",
                    "pair_relation:syntax_return_boundary_shift",
                ),
            ],
        ),
        PatternSpec::new(
            "api_return_delegation_wrapper_chain",
            48,
            6,
            "pkg/api.py",
            &[
                (
                    "ast_surface:call_api_surface",
                    "source_site_relation:same_file_distant_context",
                    "group:cross_panel:high_disagreement_cluster",
                ),
                (
                    "ast_surface:return_value_surface",
                    "source_site_relation:near_patch_site_2kb",
                    "pair_relation:api_return_delegation_shift",
                ),
            ],
        ),
        PatternSpec::new(
            "stable_background_import_and_impl_scaffold",
            6,
            552,
            "pkg/stable.py",
            &[
                (
                    "ast_surface:import_surface",
                    "path_surface:no_compact_source_site",
                    "group:sparse_lexical_panel:low_entropy",
                ),
                (
                    "ast_surface:function_contract_surface",
                    "path_surface:no_compact_source_site",
                    "group:semantic_content_panel:present",
                ),
            ],
        ),
        PatternSpec::new(
            "blanket_support_expansion_regressed",
            128,
            128,
            "pkg/blanket.py",
            &[
                (
                    "ast_surface:call_api_surface",
                    "path_surface:no_compact_source_site",
                    "group:generic_high_support",
                ),
                (
                    "ast_surface:function_contract_surface",
                    "path_surface:no_compact_source_site",
                    "group:generic_high_support",
                ),
            ],
        )
        .with_skill_id("skill:blanket_support_expansion_regressed:fsv")
        .with_status(SkillPromotionStatus::Demoted),
    ];

    let mut seeded = Vec::new();
    for (idx, spec) in specs.into_iter().enumerate() {
        let (skill, episodes) = skill_and_episodes(&spec, NOW + idx as i64)?;
        for episode in &episodes {
            expected_code_state_keys.insert(episode.ordered_steps[0].code_state_key.clone());
            for step in &episode.ordered_steps {
                source_index.insert(ChunkSourceRow {
                    chunk_id: step.chunk_id.clone(),
                    file_path: step.file_path.clone(),
                    byte_span: [0, 64],
                    source_text: Some(source_text(&spec.name, step.step_index)),
                    source_text_sha256: Some(sha256_text(&source_text(
                        &spec.name,
                        step.step_index,
                    ))),
                    source_row_key: Some(format!("source:{}:{}", spec.name, step.step_index)),
                })?;
            }
        }
        let materialization = materialize_skill_memberships(&skill, &episodes, NOW + idx as i64)?;
        write_skill_materialization_sync_readback(db, &materialization)?;
        seeded.push(skill);
    }
    Ok(seeded)
}

fn skill_and_episodes(
    spec: &PatternSpec,
    created_at_unix_ms: i64,
) -> Result<(Level2SkillRow, Vec<SkillEpisodeRow>), Box<dyn std::error::Error>> {
    let support = spec.fail + spec.pass;
    let templates = spec
        .steps
        .iter()
        .enumerate()
        .map(|(idx, step)| SkillStepTemplate {
            step_index: idx as u32,
            accepted_label_ids: vec![step.ast_label.clone(), step.relation_label.clone()],
            group_ids: vec![step.group_label.clone()],
        })
        .collect::<Vec<_>>();
    let skill_id = spec.skill_id.clone().unwrap_or(skill_id_from_parts(
        &spec.name,
        &["group:manual_skill_patterns".to_string()],
        &templates,
    )?);
    let source_episode_ids = (0..support)
        .map(|idx| format!("episode:{}:{idx}", spec.name))
        .collect::<Vec<_>>();
    let fail_rate = spec.fail as f64 / support as f64;
    let pass_rate = spec.pass as f64 / support as f64;
    let baseline = 0.3796666667_f64;
    let lift = if fail_rate >= pass_rate {
        fail_rate - baseline
    } else {
        pass_rate - (1.0 - baseline)
    }
    .max(0.01);
    let skill = Level2SkillRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_id: skill_id.clone(),
        skill_name: spec.name.clone(),
        parent_group_ids: vec!["group:manual_skill_patterns".to_string()],
        parent_skill_ids: Vec::new(),
        ordered_steps: templates,
        prerequisite_label_ids: vec![
            spec.steps[0].ast_label.clone(),
            spec.steps[0].relation_label.clone(),
        ],
        transition_edges: vec![SkillTransitionEdge {
            from_step_index: 0,
            to_step_index: 1,
            edge_label: "ordered_teleological_constellation_sequence".to_string(),
        }],
        support,
        confidence: fail_rate.max(pass_rate),
        lift_over_cell_baseline: lift,
        stability: 0.90,
        oracle_outcome_distribution: SkillOutcomeDistribution {
            pass: spec.pass,
            fail: spec.fail,
            unknown: 0,
        },
        code_state_keys: source_episode_ids
            .iter()
            .map(|id| {
                format!(
                    "state:{}:{}",
                    spec.name,
                    id.rsplit(':').next().unwrap_or("0")
                )
            })
            .collect(),
        source_episode_ids: source_episode_ids.clone(),
        failure_evidence_set_ids: if spec.fail > 0 {
            vec![format!("failure_evidence:{}", spec.name)]
        } else {
            Vec::new()
        },
        live_input_allowed: true,
        promotion_status: spec.status,
        operator_approved: spec.status == SkillPromotionStatus::OperatorApproved,
        created_at_unix_ms,
    };
    skill.validate()?;
    let mut episodes = Vec::new();
    for idx in 0..support {
        let verdict = if idx < spec.fail {
            SkillOutcomeVerdict::Fail
        } else {
            SkillOutcomeVerdict::Pass
        };
        let code_state_key = format!("state:{}:{idx}", spec.name);
        let ordered_steps = spec
            .steps
            .iter()
            .enumerate()
            .map(|(step_idx, step)| SkillStepEvidence {
                step_index: step_idx as u32,
                chunk_id: format!("chunk:{}:{idx}:{step_idx}", spec.name),
                file_path: spec.file_path.clone(),
                code_state_key: code_state_key.clone(),
                accepted_label_ids: vec![step.ast_label.clone(), step.relation_label.clone()],
                group_ids: vec![step.group_label.clone()],
            })
            .collect::<Vec<_>>();
        episodes.push(SkillEpisodeRow {
            episode_id: source_episode_ids[idx as usize].clone(),
            proposed_skill_name: Some(spec.name.clone()),
            ordered_steps,
            outcome: Some(SkillOutcomeObservation {
                outcome_label_id: match verdict {
                    SkillOutcomeVerdict::Pass => "oracle:pass".to_string(),
                    SkillOutcomeVerdict::Fail => "oracle:fail".to_string(),
                    SkillOutcomeVerdict::Unknown => "oracle:unknown".to_string(),
                },
                verdict,
                target_side_supervision_only: true,
            }),
            failure_evidence_set_ids: if verdict == SkillOutcomeVerdict::Fail {
                vec![format!("failure_evidence:{}", spec.name)]
            } else {
                Vec::new()
            },
            cell_baseline_fail_rate: 0.3796666667,
        });
    }
    Ok((skill, episodes))
}

fn write_demote_audit(
    db: &rocksdb::DB,
    skill_id: &str,
    created_at_unix_ms: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = SkillLifecycleAuditRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_audit_id: lifecycle_audit_id_from_parts(
            Some(skill_id),
            SkillLifecycleDecision::DemoteUnstableSkill,
            created_at_unix_ms,
        )?,
        prediction_id: Some("prediction:skill_audit_negative_control".to_string()),
        mistake_id: Some("mistake:skill_audit_negative_control".to_string()),
        previous_skill_id: Some(skill_id.to_string()),
        decision: SkillLifecycleDecision::DemoteUnstableSkill,
        candidate_skill_id: None,
        evidence_label_ids: vec!["group:generic_high_support".to_string()],
        evidence_chunk_ids: vec!["chunk:blanket_support_expansion_regressed:0:0".to_string()],
        reason: "blanket_support_expansion_regressed_392_metrics".to_string(),
        created_at_unix_ms,
        evidence_skill_ids: vec![skill_id.to_string()],
        evidence_higher_ability_ids: Vec::new(),
        source_membership_keys: Vec::new(),
    };
    write_skill_lifecycle_audit_row_sync_readback(db, &row)?;
    Ok(())
}

#[derive(Clone)]
struct PatternSpec {
    name: String,
    fail: u64,
    pass: u64,
    file_path: String,
    steps: Vec<StepSpec>,
    skill_id: Option<String>,
    status: SkillPromotionStatus,
}

impl PatternSpec {
    fn new(
        name: &str,
        fail: u64,
        pass: u64,
        file_path: &str,
        steps: &[(&str, &str, &str)],
    ) -> Self {
        Self {
            name: name.to_string(),
            fail,
            pass,
            file_path: file_path.to_string(),
            steps: steps
                .iter()
                .map(|(ast, relation, group)| StepSpec {
                    ast_label: (*ast).to_string(),
                    relation_label: (*relation).to_string(),
                    group_label: (*group).to_string(),
                })
                .collect(),
            skill_id: None,
            status: SkillPromotionStatus::PromotionReady,
        }
    }

    fn with_skill_id(mut self, skill_id: &str) -> Self {
        self.skill_id = Some(skill_id.to_string());
        self
    }

    fn with_status(mut self, status: SkillPromotionStatus) -> Self {
        self.status = status;
        self
    }
}

#[derive(Clone)]
struct StepSpec {
    ast_label: String,
    relation_label: String,
    group_label: String,
}

fn parse_fsv_root() -> PathBuf {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--fsv-root" {
            if let Some(value) = args.next() {
                return PathBuf::from(value);
            }
        }
    }
    PathBuf::from("/tmp/contextgraph-task-skill-005-skill-usefulness-audit-surface-fsv")
}

fn cf_counts(db: &rocksdb::DB) -> Result<BTreeMap<String, u64>, Box<dyn std::error::Error>> {
    let mut counts = BTreeMap::new();
    for cf in [
        CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
        CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
        CF_MEJEPA_SKILL_REVERSE_INDEX,
        CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
    ] {
        counts.insert(cf.to_string(), count_cf_rows(db, cf)?);
    }
    Ok(counts)
}

fn source_text(pattern: &str, step_index: u32) -> String {
    format!("def {pattern}_{step_index}():\n    return {step_index}\n")
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(text.as_bytes())))
}

fn target_side_label(label: &str) -> bool {
    label.starts_with("oracle:")
        || label.starts_with("docker:")
        || label.starts_with("test_result:")
        || label.starts_with("ground_truth:")
}

fn tree_state(path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    collect_tree(path, path, &mut rows)?;
    rows.sort();
    Ok(rows)
}

fn collect_tree(
    root: &Path,
    path: &Path,
    rows: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            collect_tree(root, &child, rows)?;
        } else {
            let metadata = fs::metadata(&child)?;
            rows.push(format!(
                "{}:{}",
                child.strip_prefix(root)?.display(),
                metadata.len()
            ));
        }
    }
    Ok(())
}
