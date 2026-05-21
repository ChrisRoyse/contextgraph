use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use context_graph_mejepa_train::ability_resolver::{
    open_ability_resolver_rocksdb, prediction_label_context_from_ability_context,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, read_all_chunk_skill_membership_rows, read_all_level2_skill_rows,
    read_all_skill_lifecycle_audit_rows, read_all_skill_reverse_index_rows,
    write_skill_materialization_sync_readback, SkillMaterialization,
};
use context_graph_mejepa_train::label_bridge::load_label_learning_bridge;
use context_graph_mejepa_train::live_skill_reverse_index::{
    reverse_index_live_skills_sync_readback, LiveSkillChunkInput, LiveSkillReverseIndexRequest,
};
use context_graph_mejepa_train::skill_sequence_discovery::{
    Level2SkillRow, SkillOutcomeDistribution, SkillPromotionStatus, SkillStepTemplate,
    SkillTransitionEdge, SKILL_SEQUENCE_SCHEMA_VERSION,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_779_398_400_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = parse_fsv_root();
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;
    let before_tree = tree_state(&fsv_root)?;
    let db_path = fsv_root.join("live-skill-reverse-index.rocksdb");
    let bridge_root = fsv_root.join("label-bridge");
    fs::create_dir_all(&bridge_root)?;

    let synthetic_request = synthetic_live_project_request(NOW + 100);
    let swebench_request = swebench_wrong_file_replay_request(NOW + 200);
    let partial_only_request = partial_candidate_request(NOW + 300);
    let labels = all_live_labels(&[&synthetic_request, &swebench_request, &partial_only_request]);
    write_label_bridge_fixture(&bridge_root, &labels)?;
    let bridge = load_label_learning_bridge(&bridge_root)?;

    let db = open_ability_resolver_rocksdb(&db_path, true)?;
    let before_counts = cf_counts(&db)?;
    let seeded_skills = seed_skill_catalog(&db)?;
    let after_seed_counts = cf_counts(&db)?;

    let synthetic_report = reverse_index_live_skills_sync_readback(&db, synthetic_request)?;
    let synthetic_prediction_context = prediction_label_context_from_ability_context(
        &bridge,
        &synthetic_report.ability_context,
        Vec::new(),
    )?;
    let swebench_report = reverse_index_live_skills_sync_readback(&db, swebench_request)?;
    let swebench_prediction_context = prediction_label_context_from_ability_context(
        &bridge,
        &swebench_report.ability_context,
        Vec::new(),
    )?;
    let partial_report = reverse_index_live_skills_sync_readback(&db, partial_only_request)?;

    let target_label_rejected = reverse_index_live_skills_sync_readback(
        &db,
        LiveSkillReverseIndexRequest {
            code_state_key: "state:target_label_rejection".to_string(),
            chunks: vec![LiveSkillChunkInput {
                chunk_id: "chunk:target_label_rejected".to_string(),
                file_path: "pkg/live.py".to_string(),
                byte_start: 0,
                byte_end: 10,
                sequence_index: 0,
                accepted_label_ids: vec!["oracle:fail".to_string()],
                group_ids: vec!["group:cross_panel:contract_drift".to_string()],
            }],
            created_at_unix_ms: NOW + 400,
            emit_partial_candidates: true,
        },
    )
    .is_err();
    let missing_order_evidence_rejected = reverse_index_live_skills_sync_readback(
        &db,
        LiveSkillReverseIndexRequest {
            code_state_key: "state:bad_order".to_string(),
            chunks: vec![
                live_chunk(
                    "chunk:bad_order:first",
                    "pkg/live.py",
                    0,
                    10,
                    2,
                    &["ast_surface:function_def"],
                    &["group:cross_panel:contract_drift"],
                ),
                live_chunk(
                    "chunk:bad_order:second",
                    "pkg/live.py",
                    10,
                    20,
                    1,
                    &["ast_surface:return_stmt"],
                    &["group:cross_panel:contract_drift"],
                ),
            ],
            created_at_unix_ms: NOW + 500,
            emit_partial_candidates: true,
        },
    )
    .is_err();
    let file_path_order_rejected = reverse_index_live_skills_sync_readback(
        &db,
        LiveSkillReverseIndexRequest {
            code_state_key: "state:bad_file_order".to_string(),
            chunks: vec![
                live_chunk(
                    "chunk:bad_file_order:z",
                    "pkg/z.py",
                    0,
                    10,
                    1,
                    &["ast_surface:function_def"],
                    &["group:cross_panel:contract_drift"],
                ),
                live_chunk(
                    "chunk:bad_file_order:a",
                    "pkg/a.py",
                    0,
                    10,
                    2,
                    &["ast_surface:return_stmt"],
                    &["group:cross_panel:contract_drift"],
                ),
            ],
            created_at_unix_ms: NOW + 600,
            emit_partial_candidates: true,
        },
    )
    .is_err();
    db.flush()?;
    let after_counts = cf_counts(&db)?;
    drop(db);

    let readback_db = open_ability_resolver_rocksdb(&db_path, false)?;
    let readback_counts = cf_counts(&readback_db)?;
    let readback_skills = read_all_level2_skill_rows(&readback_db)?;
    let readback_memberships = read_all_chunk_skill_membership_rows(&readback_db)?;
    let readback_reverse_indexes = read_all_skill_reverse_index_rows(&readback_db)?;
    let readback_lifecycle = read_all_skill_lifecycle_audit_rows(&readback_db)?;

    let synthetic_expected_skills = vec![
        "skill:api_return_delegation_wrapper_chain:fsv".to_string(),
        "skill:source_site_function_contract_risk_chain:fsv".to_string(),
        "skill:wrong_file_test_oracle_import_chain:fsv".to_string(),
    ];
    let synthetic_matched_skill_set = synthetic_report
        .matched_skills
        .iter()
        .map(|row| row.skill_id.clone())
        .collect::<BTreeSet<_>>();
    let synthetic_live_project_pass = synthetic_report.matched_skills.len()
        == synthetic_expected_skills.len()
        && synthetic_expected_skills
            .iter()
            .all(|skill_id| synthetic_matched_skill_set.contains(skill_id))
        && synthetic_prediction_context.active_skill_ids == synthetic_expected_skills
        && !synthetic_report.partial_candidates.is_empty()
        && synthetic_report.partial_candidates.iter().any(|row| {
            row.previous_skill_id.as_deref() == Some("skill:exception_boundary_repair_chain:fsv")
        });
    let swebench_replay_pass = swebench_report
        .matched_skills
        .iter()
        .map(|row| row.skill_id.as_str())
        .collect::<Vec<_>>()
        == vec!["skill:wrong_file_test_oracle_import_chain:fsv"]
        && swebench_prediction_context.active_skill_ids
            == vec!["skill:wrong_file_test_oracle_import_chain:fsv".to_string()]
        && swebench_prediction_context
            .source_membership_keys
            .iter()
            .all(|key| key.contains("swebench:py300x8:wrong_file_import_chain"));
    let partial_candidate_pass = partial_report.matched_skills.is_empty()
        && partial_report.partial_candidates.len() == 1
        && partial_report.materialization.chunk_memberships.is_empty()
        && partial_report.partial_candidates.iter().all(|row| {
            row.candidate_skill_id
                .starts_with("skill_candidate:live_partial:")
        });
    let edge_cases_pass =
        target_label_rejected && missing_order_evidence_rejected && file_path_order_rejected;
    let readback_pass = readback_counts == after_counts
        && readback_skills.len() == seeded_skills.len()
        && readback_memberships.len()
            == synthetic_report.materialization.chunk_memberships.len()
                + swebench_report.materialization.chunk_memberships.len()
        && readback_reverse_indexes.len()
            == synthetic_report.materialization.reverse_indexes.len()
                + swebench_report.materialization.reverse_indexes.len()
        && readback_lifecycle.len()
            == synthetic_report.materialization.lifecycle_audits.len()
                + swebench_report.materialization.lifecycle_audits.len()
                + partial_report.materialization.lifecycle_audits.len()
        && all_report_rows_read_back(
            &[
                &synthetic_report.materialization,
                &swebench_report.materialization,
                &partial_report.materialization,
            ],
            &readback_memberships,
            &readback_reverse_indexes,
            &readback_lifecycle,
        );
    let invariant_pass = synthetic_report.live_input_allowed
        && synthetic_report.no_new_prediction_head_introduced
        && !synthetic_report.hidden_intent_inference_used
        && !synthetic_report.target_side_labels_used_as_live_inputs
        && !synthetic_report.flat_vector_concat_used
        && synthetic_prediction_context.validate().is_ok()
        && swebench_prediction_context.validate().is_ok()
        && no_forbidden_runtime_policy(&synthetic_prediction_context)
        && no_forbidden_runtime_policy(&swebench_prediction_context);
    let all_passed = synthetic_live_project_pass
        && swebench_replay_pass
        && partial_candidate_pass
        && edge_cases_pass
        && readback_pass
        && invariant_pass;

    let report_path = fsv_root.join("task_skill_003_live_skill_reverse_index_fsv.json");
    let report = json!({
        "artifact_kind": "task_skill_003_live_skill_reverse_index_fsv",
        "schema_version": 1,
        "task_id": "TASK-SKILL-003",
        "issue": "https://github.com/ChrisRoyse/contextgraph/issues/555",
        "passes": all_passed,
        "source_of_truth": {
            "db_path": db_path,
            "label_bridge_root": bridge_root,
            "cfs": [
                CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
                CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
                CF_MEJEPA_SKILL_REVERSE_INDEX,
                CF_MEJEPA_SKILL_LIFECYCLE_AUDIT
            ]
        },
        "before": {"tree": before_tree, "cf_counts": before_counts},
        "after_seed": {"cf_counts": after_seed_counts, "seeded_skill_ids": seeded_skills.iter().map(|row| row.skill_id.clone()).collect::<Vec<_>>()},
        "after": {"tree": tree_state(&fsv_root)?, "cf_counts": after_counts},
        "readback": {
            "cf_counts": readback_counts,
            "level2_skill_rows": readback_skills,
            "chunk_skill_membership_rows": readback_memberships,
            "skill_reverse_index_rows": readback_reverse_indexes,
            "skill_lifecycle_audit_rows": readback_lifecycle
        },
        "synthetic_live_project": {
            "report": synthetic_report,
            "prediction_label_context": synthetic_prediction_context,
            "expected_skill_ids": synthetic_expected_skills,
            "passes": synthetic_live_project_pass
        },
        "swebench_known_pattern_replay": {
            "source": "manual_skill_pattern_from_551_wrong_file_test_oracle_import_chain",
            "report": swebench_report,
            "prediction_label_context": swebench_prediction_context,
            "passes": swebench_replay_pass
        },
        "partial_candidate": {
            "report": partial_report,
            "passes": partial_candidate_pass
        },
        "edge_cases": {
            "target_side_live_label_rejected": target_label_rejected,
            "missing_order_evidence_rejected": missing_order_evidence_rejected,
            "file_path_order_rejected": file_path_order_rejected,
            "passes": edge_cases_pass
        },
        "required_invariants": {
            "no_inner_llm": true,
            "no_hidden_intent_inference": true,
            "flat_vector_concat_used": false,
            "target_side_labels_used_as_live_inputs": false,
            "readback_pass": readback_pass,
            "invariant_pass": invariant_pass
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let final_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if final_report != report || !all_passed {
        return Err(format!("TASK-SKILL-003 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&final_report)?);
    Ok(())
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
    PathBuf::from("/tmp/contextgraph-task-skill-003-live-skill-reverse-index-fsv")
}

fn seed_skill_catalog(db: &rocksdb::DB) -> Result<Vec<Level2SkillRow>, Box<dyn std::error::Error>> {
    let skills = vec![
        skill(
            "skill:wrong_file_test_oracle_import_chain:fsv",
            "wrong_file_test_oracle_import_chain",
            &["group:cross_panel:import_chain"],
            vec![
                template(
                    0,
                    &[
                        "source_site_relation:wrong_file_edit",
                        "ast_surface:import_from",
                        "slot:e2:family:temporal_coordinate",
                    ],
                    &["group:cross_panel:import_chain"],
                ),
                template(
                    1,
                    &[
                        "pair_relation:test_oracle_import_resolution",
                        "ast_surface:import_from",
                        "slot:e3:family:temporal_coordinate",
                    ],
                    &["group:cross_panel:import_chain"],
                ),
            ],
            SkillOutcomeDistribution {
                pass: 0,
                fail: 125,
                unknown: 0,
            },
        ),
        skill(
            "skill:source_site_function_contract_risk_chain:fsv",
            "source_site_function_contract_risk_chain",
            &["group:cross_panel:contract_drift"],
            vec![
                template(
                    0,
                    &[
                        "source_site_relation:near_patch_site",
                        "ast_surface:function_def",
                        "slot:e2:family:temporal_coordinate",
                    ],
                    &["group:cross_panel:contract_drift"],
                ),
                template(
                    1,
                    &[
                        "pair_relation:return_contract_boundary",
                        "ast_surface:return_stmt",
                        "slot:e4:family:temporal_coordinate",
                    ],
                    &["group:cross_panel:contract_drift"],
                ),
            ],
            SkillOutcomeDistribution {
                pass: 44,
                fail: 327,
                unknown: 0,
            },
        ),
        skill(
            "skill:api_return_delegation_wrapper_chain:fsv",
            "api_return_delegation_wrapper_chain",
            &["group:cross_panel:return_delegation"],
            vec![
                template(
                    0,
                    &[
                        "pair_relation:delegation_wrapper",
                        "ast_surface:function_def",
                        "slot:e2:family:temporal_coordinate",
                    ],
                    &["group:cross_panel:return_delegation"],
                ),
                template(
                    1,
                    &[
                        "pair_relation:api_return_value",
                        "ast_surface:return_stmt",
                        "slot:e4:family:temporal_coordinate",
                    ],
                    &["group:cross_panel:return_delegation"],
                ),
            ],
            SkillOutcomeDistribution {
                pass: 151,
                fail: 327,
                unknown: 0,
            },
        ),
        skill(
            "skill:exception_boundary_repair_chain:fsv",
            "exception_boundary_repair_chain",
            &["group:cross_panel:exception_boundary"],
            vec![
                template(
                    0,
                    &[
                        "pair_relation:swallowed_exception",
                        "ast_surface:except_handler",
                        "slot:e2:family:temporal_coordinate",
                    ],
                    &["group:cross_panel:exception_boundary"],
                ),
                template(
                    1,
                    &[
                        "pair_relation:propagated_failure",
                        "ast_surface:raise_stmt",
                        "slot:e3:family:temporal_coordinate",
                    ],
                    &["group:cross_panel:exception_boundary"],
                ),
            ],
            SkillOutcomeDistribution {
                pass: 12,
                fail: 68,
                unknown: 0,
            },
        ),
    ];
    write_skill_materialization_sync_readback(
        db,
        &SkillMaterialization {
            level2_skills: skills.clone(),
            chunk_memberships: Vec::new(),
            reverse_indexes: Vec::new(),
            lifecycle_audits: Vec::new(),
        },
    )?;
    Ok(skills)
}

fn synthetic_live_project_request(created_at_unix_ms: i64) -> LiveSkillReverseIndexRequest {
    LiveSkillReverseIndexRequest {
        code_state_key: "state:live_project:no_oracle_outcomes".to_string(),
        chunks: vec![
            live_chunk(
                "chunk:live:contract_wrapper_function",
                "pkg/api.py",
                0,
                160,
                1,
                &[
                    "source_site_relation:near_patch_site",
                    "pair_relation:delegation_wrapper",
                    "ast_surface:function_def",
                    "slot:e2:family:temporal_coordinate",
                ],
                &[
                    "group:cross_panel:contract_drift",
                    "group:cross_panel:return_delegation",
                ],
            ),
            live_chunk(
                "chunk:live:return_contract_boundary",
                "pkg/api.py",
                160,
                260,
                2,
                &[
                    "pair_relation:return_contract_boundary",
                    "pair_relation:api_return_value",
                    "ast_surface:return_stmt",
                    "slot:e4:family:temporal_coordinate",
                ],
                &[
                    "group:cross_panel:contract_drift",
                    "group:cross_panel:return_delegation",
                ],
            ),
            live_chunk(
                "chunk:live:exception_swallow",
                "pkg/api.py",
                260,
                360,
                3,
                &[
                    "pair_relation:swallowed_exception",
                    "ast_surface:except_handler",
                    "slot:e2:family:temporal_coordinate",
                ],
                &["group:cross_panel:exception_boundary"],
            ),
            live_chunk(
                "chunk:live:wrong_file_import",
                "pkg/tests/test_imports.py",
                0,
                90,
                4,
                &[
                    "source_site_relation:wrong_file_edit",
                    "ast_surface:import_from",
                    "slot:e2:family:temporal_coordinate",
                ],
                &["group:cross_panel:import_chain"],
            ),
            live_chunk(
                "chunk:live:test_oracle_import",
                "pkg/tests/test_imports.py",
                90,
                180,
                5,
                &[
                    "pair_relation:test_oracle_import_resolution",
                    "ast_surface:import_from",
                    "slot:e3:family:temporal_coordinate",
                ],
                &["group:cross_panel:import_chain"],
            ),
        ],
        created_at_unix_ms,
        emit_partial_candidates: true,
    }
}

fn swebench_wrong_file_replay_request(created_at_unix_ms: i64) -> LiveSkillReverseIndexRequest {
    LiveSkillReverseIndexRequest {
        code_state_key: "swebench:py300x8:wrong_file_import_chain".to_string(),
        chunks: vec![
            live_chunk(
                "chunk:swebench:wrong_file_import:0",
                "tests/test_settings.py",
                0,
                84,
                1,
                &[
                    "source_site_relation:wrong_file_edit",
                    "ast_surface:import_from",
                    "slot:e2:family:temporal_coordinate",
                ],
                &["group:cross_panel:import_chain"],
            ),
            live_chunk(
                "chunk:swebench:test_oracle_import:1",
                "tests/test_settings.py",
                84,
                184,
                2,
                &[
                    "pair_relation:test_oracle_import_resolution",
                    "ast_surface:import_from",
                    "slot:e3:family:temporal_coordinate",
                ],
                &["group:cross_panel:import_chain"],
            ),
        ],
        created_at_unix_ms,
        emit_partial_candidates: true,
    }
}

fn partial_candidate_request(created_at_unix_ms: i64) -> LiveSkillReverseIndexRequest {
    LiveSkillReverseIndexRequest {
        code_state_key: "state:live_project:partial_exception_boundary".to_string(),
        chunks: vec![live_chunk(
            "chunk:partial:exception_swallow",
            "pkg/error_boundary.py",
            0,
            120,
            1,
            &[
                "pair_relation:swallowed_exception",
                "ast_surface:except_handler",
                "slot:e2:family:temporal_coordinate",
            ],
            &["group:cross_panel:exception_boundary"],
        )],
        created_at_unix_ms,
        emit_partial_candidates: true,
    }
}

fn skill(
    skill_id: &str,
    skill_name: &str,
    parent_groups: &[&str],
    steps: Vec<SkillStepTemplate>,
    distribution: SkillOutcomeDistribution,
) -> Level2SkillRow {
    Level2SkillRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_id: skill_id.to_string(),
        skill_name: skill_name.to_string(),
        parent_group_ids: parent_groups
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        parent_skill_ids: Vec::new(),
        prerequisite_label_ids: steps[0].accepted_label_ids.clone(),
        transition_edges: steps
            .windows(2)
            .map(|pair| SkillTransitionEdge {
                from_step_index: pair[0].step_index,
                to_step_index: pair[1].step_index,
                edge_label: "sequence:next".to_string(),
            })
            .collect(),
        ordered_steps: steps,
        support: 2,
        confidence: 0.92,
        lift_over_cell_baseline: 0.40,
        stability: 0.88,
        oracle_outcome_distribution: distribution,
        code_state_keys: vec!["state:baseline:skill_catalog_seed".to_string()],
        source_episode_ids: vec![
            format!("episode:{skill_name}:seed_a"),
            format!("episode:{skill_name}:seed_b"),
        ],
        failure_evidence_set_ids: Vec::new(),
        live_input_allowed: true,
        promotion_status: SkillPromotionStatus::OperatorApproved,
        operator_approved: true,
        created_at_unix_ms: NOW,
    }
}

fn template(step_index: u32, labels: &[&str], groups: &[&str]) -> SkillStepTemplate {
    let mut accepted_label_ids = labels
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    accepted_label_ids.sort();
    let mut group_ids = groups
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    group_ids.sort();
    SkillStepTemplate {
        step_index,
        accepted_label_ids,
        group_ids,
    }
}

fn live_chunk(
    chunk_id: &str,
    file_path: &str,
    byte_start: u64,
    byte_end: u64,
    sequence_index: u32,
    labels: &[&str],
    groups: &[&str],
) -> LiveSkillChunkInput {
    let mut accepted_label_ids = labels
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    accepted_label_ids.sort();
    let mut group_ids = groups
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    group_ids.sort();
    LiveSkillChunkInput {
        chunk_id: chunk_id.to_string(),
        file_path: file_path.to_string(),
        byte_start,
        byte_end,
        sequence_index,
        accepted_label_ids,
        group_ids,
    }
}

fn all_live_labels(requests: &[&LiveSkillReverseIndexRequest]) -> Vec<String> {
    let mut labels = BTreeSet::new();
    for request in requests {
        for chunk in &request.chunks {
            labels.extend(chunk.accepted_label_ids.iter().cloned());
        }
    }
    labels.into_iter().collect()
}

fn write_label_bridge_fixture(
    root: &Path,
    labels: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let accepted = root.join("accepted_label_registry.jsonl");
    let usefulness = root.join("label_usefulness_metrics.jsonl");
    let chunks = root.join("chunk_constellation_labels.jsonl");
    let code_states = root.join("code_state_constellations.jsonl");
    let failure_sets = root.join("failure_evidence_sets.jsonl");

    let mut accepted_lines = Vec::new();
    let mut usefulness_lines = Vec::new();
    for label in labels {
        let family = label.split_once(':').map(|(head, _)| head).unwrap_or(label);
        accepted_lines.push(
            json!({
                "artifact_kind": "task_skill_003_accepted_label_registry_row",
                "schema_version": 1,
                "formula_version": "task-skill-003-fsv",
                "label_id": label,
                "label_hash": format!("sha256:{}", short_hash(label)),
                "family": family,
                "status": "accepted_live_input",
                "support": 42,
                "fail_rate": 0.50,
                "lift_over_weighted_category_baseline": 0.10,
                "live_prediction_input_allowed": true,
                "learning_update_policy": {
                    "baseline_seeded_from": "TASK-SKILL-003-FSV",
                    "online_rescore": true,
                    "promote_demote_with_new_outcomes": true,
                    "target_labels_are_supervision_only": true
                }
            })
            .to_string(),
        );
        usefulness_lines.push(
            json!({
                "label_id": label,
                "family": family,
                "status": "accepted_live_input",
                "support": 42,
                "leaky_target": false,
                "lift_over_weighted_category_baseline": 0.10
            })
            .to_string(),
        );
    }
    fs::write(&accepted, format!("{}\n", accepted_lines.join("\n")))?;
    fs::write(&usefulness, format!("{}\n", usefulness_lines.join("\n")))?;
    fs::write(&chunks, "{}\n")?;

    let states = [
        "state:live_project:no_oracle_outcomes",
        "swebench:py300x8:wrong_file_import_chain",
        "state:live_project:partial_exception_boundary",
    ];
    let code_state_lines = states
        .iter()
        .map(|state| {
            json!({
                "group_key": state,
                "labels": labels,
                "live_predictor_labels": labels,
                "target_supervision_labels": [],
                "slot_identity_preserved": true,
                "flat_vector_concat_used": false
            })
            .to_string()
        })
        .collect::<Vec<_>>();
    fs::write(&code_states, format!("{}\n", code_state_lines.join("\n")))?;
    fs::write(&failure_sets, "")?;

    let manifest = json!({
        "artifact_kind": "task_skill_003_learning_bridge_manifest",
        "schema_version": 1,
        "formula_version": "task-skill-003-fsv",
        "purpose": "prove live skill reverse index carries matched skills into PredictionLabelContext",
        "paths": {
            "accepted_label_registry": accepted.display().to_string(),
            "label_usefulness_metrics": usefulness.display().to_string(),
            "chunk_constellation_labels": chunks.display().to_string(),
            "code_state_constellations": code_states.display().to_string(),
            "failure_evidence_sets": failure_sets.display().to_string()
        },
        "consumers": {
            "PredictionLabelContext.active_skill_ids": "live reverse indexed ordered skills"
        },
        "dynamic_learning_policy": {
            "baseline_only": false,
            "new_data_uses_same_compiler": true,
            "usefulness_rescored_over_time": true,
            "target_outcomes_supervise_but_are_not_live_inputs": true,
            "slot_identity_preserved": true,
            "flat_vector_concat_used": false
        }
    });
    fs::write(
        root.join("learning_bridge_manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
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

fn all_report_rows_read_back(
    materializations: &[&SkillMaterialization],
    memberships: &[context_graph_mejepa_train::chunk_skill_membership::ChunkSkillMembershipRow],
    reverse_indexes: &[context_graph_mejepa_train::chunk_skill_membership::SkillReverseIndexRow],
    audits: &[context_graph_mejepa_train::chunk_skill_membership::SkillLifecycleAuditRow],
) -> bool {
    materializations.iter().all(|materialization| {
        materialization
            .chunk_memberships
            .iter()
            .all(|row| memberships.contains(row))
            && materialization
                .reverse_indexes
                .iter()
                .all(|row| reverse_indexes.contains(row))
            && materialization
                .lifecycle_audits
                .iter()
                .all(|row| audits.contains(row))
    })
}

fn no_forbidden_runtime_policy(
    label_context: &context_graph_mejepa::PredictionLabelContext,
) -> bool {
    label_context
        .accepted_label_ids
        .iter()
        .chain(label_context.failure_evidence_set_ids.iter())
        .all(|id| {
            !matches!(
                id.split_once(':').map(|(family, _)| family),
                Some(
                    "oracle"
                        | "docker"
                        | "test_phase"
                        | "test_result"
                        | "ground_truth"
                        | "mutation"
                )
            )
        })
}

fn tree_state(root: &Path) -> Result<BTreeMap<String, u64>, Box<dyn std::error::Error>> {
    let mut out = BTreeMap::new();
    if !root.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let key = entry.file_name().to_string_lossy().to_string();
        let value = if file_type.is_dir() {
            fs::read_dir(entry.path())?.count() as u64
        } else {
            entry.metadata()?.len()
        };
        out.insert(key, value);
    }
    Ok(out)
}

fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)
}
