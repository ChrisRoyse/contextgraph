use context_graph_mejepa::{PredictionId, PredictionLabelContext, Verdict};
use context_graph_mejepa_cf::{
    CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS, CF_MEJEPA_MISTAKE_LOG, CF_MEJEPA_ONLINE_HEAD_STATE,
    CF_MEJEPA_REPLAY_BUFFER, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
};
use context_graph_mejepa_train::ability_resolver::{
    open_ability_resolver_rocksdb, record_ability_refutation_sync_readback,
    resolve_ability_context, AbilityContext, AbilityRefutationInput, AbilityResolverRequest,
    LiveChunkInput,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, materialize_skill_memberships, read_all_level2_skill_rows,
    read_all_skill_lifecycle_audit_rows, write_skill_materialization_sync_readback,
    SkillLifecycleDecision,
};
use context_graph_mejepa_train::label_bridge::{
    accepted_label_signature_hash, membership_signature_hash,
};
use context_graph_mejepa_train::mistake_log::{read_all_mistake_log_rows, MistakeTruthSource};
use context_graph_mejepa_train::online_head_state::{
    predict_with_online_head, read_online_head_state_row,
};
use context_graph_mejepa_train::replay_buffer::read_all_replay_rows;
use context_graph_mejepa_train::skill_sequence_discovery::{
    Level2SkillRow, SkillEpisodeRow, SkillOutcomeDistribution, SkillOutcomeObservation,
    SkillOutcomeVerdict, SkillPromotionStatus, SkillStepEvidence, SkillStepTemplate,
    SkillTransitionEdge, SKILL_SEQUENCE_SCHEMA_VERSION,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_779_402_400_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = parse_fsv_root();
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;
    let before_tree = tree_state(&fsv_root)?;
    let db_path = fsv_root.join("skill-lifecycle.rocksdb");
    let db = open_ability_resolver_rocksdb(&db_path, true)?;
    let before_counts = cf_counts(&db)?;

    let seeded = seed_skill_catalog(&db)?;
    let seeded_by_name = seeded
        .iter()
        .map(|row| (row.skill_name.clone(), row.clone()))
        .collect::<BTreeMap<_, _>>();

    let update_report = refute(
        &db,
        "state:update",
        "chunk:update:0",
        "ast_surface:function_def",
        refutation_input(0x11, "contract_break", 1.0, 0.40, NOW + 100),
    )?;
    let split_report = refute(
        &db,
        "state:split",
        "chunk:split:0",
        "ast_surface:return_stmt",
        refutation_input(0x12, "mixed_consequence_contract", 1.0, 0.90, NOW + 200),
    )?;
    let unknown_report = refute(
        &db,
        "state:unknown",
        "chunk:unknown:0",
        "ast_surface:with_stmt",
        refutation_input(0x13, "unknown_ordered_constellation", 1.0, 0.70, NOW + 300),
    )?;
    let demote_report = refute(
        &db,
        "state:background",
        "chunk:background:0",
        "ast_surface:import_from",
        refutation_input(0x14, "background_regressed", 1.0, 0.60, NOW + 400),
    )?;
    let nochange_report = refute(
        &db,
        "state:nochange",
        "chunk:nochange:0",
        "ast_surface:call",
        refutation_input(0x15, "weak_causal_control", 0.01, 0.01, NOW + 500),
    )?;

    db.flush()?;
    let after_counts = cf_counts(&db)?;
    drop(db);

    let readback_db = open_ability_resolver_rocksdb(&db_path, false)?;
    let readback_counts = cf_counts(&readback_db)?;
    let readback_skills = read_all_level2_skill_rows(&readback_db)?;
    let lifecycle_rows = read_all_skill_lifecycle_audit_rows(&readback_db)?;
    let mistake_rows = read_all_mistake_log_rows(&readback_db)?;
    let replay_rows = read_all_replay_rows(&readback_db)?;

    let updated_skill = read_skill_by_name(&readback_db, "contract_update_skill")?;
    let mixed_original = read_skill_by_name(&readback_db, "mixed_consequence_skill")?;
    let background_after = read_skill_by_name(&readback_db, "stable_background_skill")?;
    let nochange_after = read_skill_by_name(&readback_db, "weak_causal_nochange_skill")?;

    let update_prediction_after = predict_with_online_head(
        &readback_db,
        &update_report
            .online_head_update_report
            .head_state_after
            .panel_signature_hash,
        Verdict::Pass,
    )?;
    let update_head_row = read_online_head_state_row(
        &readback_db,
        &update_report
            .online_head_update_report
            .head_state_after
            .head_key,
    )?
    .ok_or("missing update online head row")?;

    let update_pass = has_decision(&update_report, SkillLifecycleDecision::UpdateExistingSkill)
        && update_report.updated_skill_rows.len() == 1
        && updated_skill.oracle_outcome_distribution.fail
            == seeded_by_name["contract_update_skill"]
                .oracle_outcome_distribution
                .fail
                + 1
        && update_prediction_after == Verdict::Fail
        && update_report
            .online_head_update_report
            .unrelated_cell_accuracy_delta
            == 0.0
        && update_head_row == update_report.online_head_update_report.head_state_after;
    let split_pass = has_decision(&split_report, SkillLifecycleDecision::SplitMixedSkill)
        && split_report.candidate_skill_rows.len() == 1
        && split_report.updated_skill_rows.is_empty()
        && mixed_original == seeded_by_name["mixed_consequence_skill"];
    let unknown_pass = has_decision(
        &unknown_report,
        SkillLifecycleDecision::CreateNewCandidateSkill,
    ) && unknown_report.candidate_skill_rows.len() == 1
        && unknown_report.candidate_created_when_no_existing_ability;
    let demote_pass = has_decision(&demote_report, SkillLifecycleDecision::DemoteUnstableSkill)
        && demote_report.updated_skill_rows.len() == 1
        && background_after.promotion_status == SkillPromotionStatus::Demoted
        && background_after.confidence < seeded_by_name["stable_background_skill"].confidence
        && background_after.lift_over_cell_baseline == 0.0;
    let nochange_pass = has_decision(
        &nochange_report,
        SkillLifecycleDecision::NoChangeWithEvidence,
    ) && nochange_report.updated_skill_rows.is_empty()
        && nochange_report.candidate_skill_rows.is_empty()
        && nochange_after == seeded_by_name["weak_causal_nochange_skill"];
    let readback_pass = readback_counts == after_counts
        && mistake_rows.len() == 5
        && replay_rows.len() == 5
        && lifecycle_rows.len() == 9
        && readback_skills.len() == 6;
    let invariant_pass = [
        &update_report,
        &split_report,
        &unknown_report,
        &demote_report,
        &nochange_report,
    ]
    .iter()
    .all(|report| {
        report.label_skill_ability_membership_ids_agree
            && report.no_new_prediction_head_introduced
            && !report.hidden_intent_inference_used
            && !report.target_side_labels_used_as_live_inputs
            && !report.flat_vector_concat_used
            && report
                .online_head_update_report
                .unrelated_cell_accuracy_delta
                == 0.0
            && report
                .mistake_row
                .accepted_label_ids
                .iter()
                .all(|label| !target_side_label(label))
    });
    let all_passed = update_pass
        && split_pass
        && unknown_pass
        && demote_pass
        && nochange_pass
        && readback_pass
        && invariant_pass;

    let report_path = fsv_root.join("task_skill_004_mistake_loop_skill_lifecycle_fsv.json");
    let report = json!({
        "artifact_kind": "task_skill_004_mistake_loop_skill_lifecycle_fsv",
        "schema_version": 1,
        "task_id": "TASK-SKILL-004",
        "issue": "https://github.com/ChrisRoyse/contextgraph/issues/556",
        "passes": all_passed,
        "source_of_truth": {
            "db_path": db_path,
            "cfs": [
                CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
                CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
                CF_MEJEPA_MISTAKE_LOG,
                CF_MEJEPA_REPLAY_BUFFER,
                CF_MEJEPA_ONLINE_HEAD_STATE
            ]
        },
        "before": {"tree": before_tree, "cf_counts": before_counts},
        "after": {"tree": tree_state(&fsv_root)?, "cf_counts": after_counts},
        "readback": {
            "cf_counts": readback_counts,
            "skill_rows": readback_skills,
            "lifecycle_rows": lifecycle_rows,
            "mistake_rows": mistake_rows,
            "replay_rows": replay_rows
        },
        "scenarios": {
            "update_existing_skill": {"passes": update_pass, "report": update_report, "readback_skill": updated_skill, "next_same_episode_prediction": format!("{:?}", update_prediction_after)},
            "split_mixed_skill": {"passes": split_pass, "report": split_report, "original_readback_unchanged": mixed_original},
            "create_unknown_candidate": {"passes": unknown_pass, "report": unknown_report},
            "demote_background_skill": {"passes": demote_pass, "report": demote_report, "readback_skill": background_after},
            "no_change_with_evidence": {"passes": nochange_pass, "report": nochange_report, "readback_skill": nochange_after}
        },
        "required_invariants": {
            "readback_pass": readback_pass,
            "invariant_pass": invariant_pass,
            "no_target_only_live_labels_after_update": invariant_pass,
            "unrelated_control_delta_zero": true,
            "no_hidden_intent_inference": true,
            "flat_vector_concat_used": false
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let readback_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if readback_report != report || !all_passed {
        return Err(format!("TASK-SKILL-004 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&readback_report)?);
    Ok(())
}

fn refute(
    db: &rocksdb::DB,
    code_state_key: &str,
    chunk_id: &str,
    live_label: &str,
    input: AbilityRefutationInput,
) -> Result<
    context_graph_mejepa_train::ability_resolver::AbilityRefutationReport,
    Box<dyn std::error::Error>,
> {
    let context = resolve_ability_context(db, request(code_state_key, chunk_id, live_label))?;
    let label_context = label_context(&context)?;
    Ok(record_ability_refutation_sync_readback(
        db,
        &context,
        &label_context,
        input,
    )?)
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
    PathBuf::from("/tmp/contextgraph-task-skill-004-mistake-loop-skill-lifecycle-fsv")
}

fn seed_skill_catalog(db: &rocksdb::DB) -> Result<Vec<Level2SkillRow>, Box<dyn std::error::Error>> {
    let skills = vec![
        seed_one_skill(
            db,
            "skill:contract_update:fsv",
            "contract_update_skill",
            "state:update",
            "chunk:update",
            [
                "ast_surface:function_def",
                "pair_relation:contract_boundary",
            ],
            SkillOutcomeDistribution {
                pass: 0,
                fail: 2,
                unknown: 0,
            },
            vec!["group:skill_lifecycle:update"],
        )?,
        seed_one_skill(
            db,
            "skill:mixed_consequence:fsv",
            "mixed_consequence_skill",
            "state:split",
            "chunk:split",
            ["ast_surface:return_stmt", "pair_relation:mixed_consequence"],
            SkillOutcomeDistribution {
                pass: 1,
                fail: 1,
                unknown: 0,
            },
            vec!["group:skill_lifecycle:mixed"],
        )?,
        seed_one_skill(
            db,
            "skill:stable_background:fsv",
            "stable_background_skill",
            "state:background",
            "chunk:background",
            [
                "ast_surface:import_from",
                "slot:e2:family:temporal_coordinate",
            ],
            SkillOutcomeDistribution {
                pass: 2,
                fail: 0,
                unknown: 0,
            },
            vec!["group:skill_lifecycle:background"],
        )?,
        seed_one_skill(
            db,
            "skill:weak_causal_nochange:fsv",
            "weak_causal_nochange_skill",
            "state:nochange",
            "chunk:nochange",
            ["ast_surface:call", "pair_relation:weak_context"],
            SkillOutcomeDistribution {
                pass: 0,
                fail: 2,
                unknown: 0,
            },
            vec!["group:skill_lifecycle:nochange"],
        )?,
    ];
    Ok(skills)
}

fn seed_one_skill(
    db: &rocksdb::DB,
    skill_id: &str,
    skill_name: &str,
    code_state_key: &str,
    chunk_prefix: &str,
    labels: [&str; 2],
    outcomes: SkillOutcomeDistribution,
    parent_group_ids: Vec<&str>,
) -> Result<Level2SkillRow, Box<dyn std::error::Error>> {
    let support = outcomes.pass + outcomes.fail + outcomes.unknown;
    let mut episodes = Vec::new();
    let mut source_episode_ids = Vec::new();
    for idx in 0..support {
        let verdict = if idx < outcomes.pass {
            SkillOutcomeVerdict::Pass
        } else if idx < outcomes.pass + outcomes.fail {
            SkillOutcomeVerdict::Fail
        } else {
            SkillOutcomeVerdict::Unknown
        };
        let episode_id = format!("episode:{skill_name}:{idx}");
        source_episode_ids.push(episode_id.clone());
        episodes.push(SkillEpisodeRow {
            episode_id,
            proposed_skill_name: Some(skill_name.to_string()),
            ordered_steps: vec![
                step(0, &format!("{chunk_prefix}:0"), code_state_key, labels[0]),
                step(1, &format!("{chunk_prefix}:1"), code_state_key, labels[1]),
            ],
            outcome: Some(SkillOutcomeObservation {
                outcome_label_id: format!("outcome:{skill_name}:{idx}"),
                verdict,
                target_side_supervision_only: true,
            }),
            failure_evidence_set_ids: vec![format!("evidence:{skill_name}:{idx}")],
            cell_baseline_fail_rate: 0.5,
        });
    }
    let dominant = outcomes.pass.max(outcomes.fail).max(outcomes.unknown);
    let skill = Level2SkillRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_id: skill_id.to_string(),
        skill_name: skill_name.to_string(),
        parent_group_ids: parent_group_ids.into_iter().map(str::to_string).collect(),
        parent_skill_ids: Vec::new(),
        ordered_steps: vec![
            SkillStepTemplate {
                step_index: 0,
                accepted_label_ids: vec![labels[0].to_string()],
                group_ids: vec!["group:skill_lifecycle:seed".to_string()],
            },
            SkillStepTemplate {
                step_index: 1,
                accepted_label_ids: vec![labels[1].to_string()],
                group_ids: vec!["group:skill_lifecycle:seed".to_string()],
            },
        ],
        prerequisite_label_ids: vec![labels[0].to_string()],
        transition_edges: vec![SkillTransitionEdge {
            from_step_index: 0,
            to_step_index: 1,
            edge_label: "sequence:next".to_string(),
        }],
        support,
        confidence: dominant as f64 / support as f64,
        lift_over_cell_baseline: ((dominant as f64 / support as f64) - 0.5).max(0.0),
        stability: 0.9,
        oracle_outcome_distribution: outcomes,
        code_state_keys: vec![code_state_key.to_string()],
        source_episode_ids,
        failure_evidence_set_ids: vec![format!("evidence:{skill_name}:seed")],
        live_input_allowed: true,
        promotion_status: SkillPromotionStatus::PromotionReady,
        operator_approved: false,
        created_at_unix_ms: NOW,
    };
    write_skill_materialization_sync_readback(
        db,
        &materialize_skill_memberships(&skill, &episodes, NOW)?,
    )?;
    Ok(skill)
}

fn step(index: u32, chunk_id: &str, state: &str, label: &str) -> SkillStepEvidence {
    SkillStepEvidence {
        step_index: index,
        chunk_id: chunk_id.to_string(),
        file_path: format!("pkg/{state}.py"),
        code_state_key: state.to_string(),
        accepted_label_ids: vec![label.to_string()],
        group_ids: vec!["group:skill_lifecycle:seed".to_string()],
    }
}

fn request(code_state_key: &str, chunk_id: &str, live_label: &str) -> AbilityResolverRequest {
    AbilityResolverRequest {
        code_state_key: code_state_key.to_string(),
        chunks: vec![LiveChunkInput {
            chunk_id: chunk_id.to_string(),
            file_path: Some(format!("pkg/{code_state_key}.py")),
            accepted_label_ids: vec![live_label.to_string()],
        }],
    }
}

fn label_context(
    context: &AbilityContext,
) -> Result<PredictionLabelContext, Box<dyn std::error::Error>> {
    Ok(PredictionLabelContext {
        code_state_key: Some(context.code_state_key.clone()),
        accepted_label_ids: context.accepted_label_ids.clone(),
        failure_evidence_set_ids: Vec::new(),
        active_skill_ids: context.active_skill_ids.clone(),
        active_higher_ability_ids: context.active_higher_ability_ids.clone(),
        source_membership_keys: context.source_membership_keys.clone(),
        label_signature_hash: Some(accepted_label_signature_hash(&context.accepted_label_ids)?),
        skill_signature_hash: context.skill_signature_hash.clone(),
        ability_signature_hash: context.ability_signature_hash.clone(),
        membership_signature_hash: if context.source_membership_keys.is_empty() {
            None
        } else {
            Some(membership_signature_hash(&context.source_membership_keys)?)
        },
        ..PredictionLabelContext::default()
    })
}

fn refutation_input(
    seed: u8,
    named_failure_mode: &str,
    surprise_z: f32,
    coverage_gap_score: f32,
    created_at_unix_ms: i64,
) -> AbilityRefutationInput {
    AbilityRefutationInput {
        prediction_id: PredictionId([seed; 16]),
        panel_signature_hash: panel_hash(seed),
        predicted_verdict: Verdict::Pass,
        ground_truth_verdict: Verdict::Fail,
        truth_source: MistakeTruthSource::SwebenchDockerOracle,
        language: "python".to_string(),
        mutation_or_live_cell: "skill_lifecycle_fsv".to_string(),
        named_failure_mode: named_failure_mode.to_string(),
        surprise_z,
        coverage_gap_score,
        created_at_unix_ms,
    }
}

fn panel_hash(seed: u8) -> String {
    let mut hasher = Sha256::new();
    hasher.update([seed; 16]);
    format!("panel:{}", &hex::encode(hasher.finalize())[..16])
}

fn read_skill_by_name(
    db: &rocksdb::DB,
    skill_name: &str,
) -> Result<Level2SkillRow, Box<dyn std::error::Error>> {
    read_all_level2_skill_rows(db)?
        .into_iter()
        .find(|row| row.skill_name == skill_name)
        .ok_or_else(|| format!("missing skill {skill_name}").into())
}

fn has_decision(
    report: &context_graph_mejepa_train::ability_resolver::AbilityRefutationReport,
    decision: SkillLifecycleDecision,
) -> bool {
    report
        .lifecycle_audits
        .iter()
        .any(|row| row.decision == decision)
}

fn target_side_label(label: &str) -> bool {
    matches!(
        label.split_once(':').map(|(family, _)| family),
        Some("oracle" | "docker" | "test_result" | "ground_truth" | "truth")
    )
}

fn cf_counts(db: &rocksdb::DB) -> Result<BTreeMap<String, u64>, Box<dyn std::error::Error>> {
    let mut out = BTreeMap::new();
    for cf in [
        CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
        CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
        CF_MEJEPA_MISTAKE_LOG,
        CF_MEJEPA_REPLAY_BUFFER,
        CF_MEJEPA_ONLINE_HEAD_STATE,
    ] {
        out.insert(cf.to_string(), count_cf_rows(db, cf)?);
    }
    Ok(out)
}

fn tree_state(root: &Path) -> Result<BTreeMap<String, u64>, Box<dyn std::error::Error>> {
    let mut out = BTreeMap::new();
    if !root.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let key = entry.file_name().to_string_lossy().to_string();
        let value = if entry.file_type()?.is_dir() {
            fs::read_dir(entry.path())?.count() as u64
        } else {
            entry.metadata()?.len()
        };
        out.insert(key, value);
    }
    Ok(out)
}
