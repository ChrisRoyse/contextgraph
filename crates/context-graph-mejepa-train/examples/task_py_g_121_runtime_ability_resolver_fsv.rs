use context_graph_mejepa::{PredictionId, PredictionLabelContext, Verdict};
use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS, CF_MEJEPA_MISTAKE_LOG,
    CF_MEJEPA_REPLAY_BUFFER, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use context_graph_mejepa_train::ability_resolver::{
    open_ability_resolver_rocksdb, prediction_label_context_from_ability_context,
    record_ability_refutation_sync_readback, resolve_ability_context, AbilityRefutationInput,
    AbilityRefutationReport, AbilityResolverRequest, LiveChunkInput,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, materialize_skill_memberships, membership_key,
    read_all_skill_lifecycle_audit_rows, write_skill_materialization_sync_readback,
    ChunkSkillMembershipRow, SkillLifecycleDecision,
};
use context_graph_mejepa_train::label_bridge::load_label_learning_bridge;
use context_graph_mejepa_train::mistake_log::{read_all_mistake_log_rows, MistakeTruthSource};
use context_graph_mejepa_train::replay_buffer::read_all_replay_rows;
use context_graph_mejepa_train::skill_sequence_discovery::{
    Level2SkillRow, SkillEpisodeRow, SkillOutcomeDistribution, SkillOutcomeObservation,
    SkillOutcomeVerdict, SkillPromotionStatus, SkillStepEvidence, SkillStepTemplate,
    SkillTransitionEdge, SKILL_SEQUENCE_SCHEMA_VERSION,
};
use rocksdb::WriteOptions;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const NOW: i64 = 1_779_312_000_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = parse_fsv_root();
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;
    let before_tree = tree_state(&fsv_root)?;
    let db_path = fsv_root.join("runtime-ability-resolver.rocksdb");
    let bridge_root = fsv_root.join("label-bridge");
    fs::create_dir_all(&bridge_root)?;
    write_label_bridge_fixture(&bridge_root)?;
    let bridge = load_label_learning_bridge(&bridge_root)?;
    let db = open_ability_resolver_rocksdb(&db_path, true)?;
    let before_counts = cf_counts(&db)?;

    let (skill_a, skill_b) = seed_skill_rows(&db)?;
    let higher_row = write_higher_ability_membership(&db, &skill_a.skill_id)?;
    let resolver_request = request("state:shared", &["chunk:shared"]);
    let ability_context = resolve_ability_context(&db, resolver_request)?;
    let prediction_label_context =
        prediction_label_context_from_ability_context(&bridge, &ability_context, Vec::new())?;
    let refutation_report = record_ability_refutation_sync_readback(
        &db,
        &ability_context,
        &prediction_label_context,
        refutation_input(0x61, NOW + 100),
    )?;

    let candidate_context = resolve_ability_context(&db, request("state:shared", &["chunk:new"]))?;
    let candidate_label_context =
        prediction_label_context_from_ability_context(&bridge, &candidate_context, Vec::new())?;
    let candidate_report = record_ability_refutation_sync_readback(
        &db,
        &candidate_context,
        &candidate_label_context,
        refutation_input(0x62, NOW + 200),
    )?;

    let left = resolve_ability_context(
        &db,
        request("state:other", &["chunk:a_only", "chunk:b_only"]),
    )?;
    let right = resolve_ability_context(
        &db,
        request("state:other", &["chunk:b_only", "chunk:a_only"]),
    )?;
    let edge_ordered_signature_distinguishes_ab =
        left.skill_signature_hash != right.skill_signature_hash;
    let edge_empty_chunk_request_rejected = resolve_ability_context(
        &db,
        AbilityResolverRequest {
            code_state_key: "state:shared".to_string(),
            chunks: Vec::new(),
        },
    )
    .is_err();
    let edge_target_label_rejected = resolve_ability_context(
        &db,
        AbilityResolverRequest {
            code_state_key: "state:shared".to_string(),
            chunks: vec![LiveChunkInput {
                chunk_id: "chunk:bad".to_string(),
                file_path: Some("pkg/bad.py".to_string()),
                accepted_label_ids: vec!["oracle:fail".to_string()],
            }],
        },
    )
    .is_err();
    let edge_mntd_db_rejected =
        open_ability_resolver_rocksdb(Path::new("/zfs/archive/contextgraph/bad.rocksdb"), true)
            .is_err();
    let edge_leaky_membership_rejected = leaky_membership_rejected(&fsv_root, &skill_a.skill_id)?;

    db.flush()?;
    let after_counts = cf_counts(&db)?;
    drop(db);

    let readback_db = open_ability_resolver_rocksdb(&db_path, false)?;
    let mistake_rows = read_all_mistake_log_rows(&readback_db)?;
    let replay_rows = read_all_replay_rows(&readback_db)?;
    let lifecycle_rows = read_all_skill_lifecycle_audit_rows(&readback_db)?;
    let readback_counts = cf_counts(&readback_db)?;

    let active_report_pass = refutation_report.label_skill_ability_membership_ids_agree
        && refutation_report.lifecycle_audits.len() == 3
        && refutation_report
            .lifecycle_audits
            .iter()
            .all(|row| row.decision == SkillLifecycleDecision::UpdateExistingSkill)
        && prediction_label_context.active_skill_ids == vec![skill_a.skill_id, skill_b.skill_id]
        && prediction_label_context.active_higher_ability_ids
            == vec!["ability:shared_runtime_sequence:fsv".to_string()]
        && prediction_label_context
            .source_membership_keys
            .contains(&higher_row.membership_key);
    let candidate_pass = candidate_report.candidate_created_when_no_existing_ability
        && candidate_report.lifecycle_audits.len() == 1
        && candidate_report.lifecycle_audits[0].decision
            == SkillLifecycleDecision::CreateNewCandidateSkill;
    let readback_pass = mistake_rows.len() == 2
        && replay_rows.len() == 2
        && lifecycle_rows.len() == 6
        && readback_counts == after_counts
        && joined_rows_agree(
            &refutation_report,
            &mistake_rows,
            &replay_rows,
            &lifecycle_rows,
        )
        && joined_rows_agree(
            &candidate_report,
            &mistake_rows,
            &replay_rows,
            &lifecycle_rows,
        );
    let edge_pass = edge_ordered_signature_distinguishes_ab
        && edge_empty_chunk_request_rejected
        && edge_target_label_rejected
        && edge_mntd_db_rejected
        && edge_leaky_membership_rejected;
    let invariant_pass = refutation_report.no_new_prediction_head_introduced
        && !refutation_report.hidden_intent_inference_used
        && !refutation_report.target_side_labels_used_as_live_inputs
        && !refutation_report.flat_vector_concat_used
        && prediction_label_context.validate().is_ok()
        && no_forbidden_runtime_policy(&prediction_label_context);
    let all_passed =
        active_report_pass && candidate_pass && readback_pass && edge_pass && invariant_pass;

    let report_path = fsv_root.join("task_py_g_121_runtime_ability_resolver_fsv.json");
    let report = json!({
        "artifact_kind": "task_py_g_121_runtime_ability_resolver_fsv",
        "schema_version": 1,
        "task_id": "TASK-PY-G-121",
        "passes": all_passed,
        "source_of_truth": {
            "db_path": db_path,
            "label_bridge_root": bridge_root,
            "cfs": [
                CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
                CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
                CF_MEJEPA_SKILL_REVERSE_INDEX,
                CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
                CF_MEJEPA_MISTAKE_LOG,
                CF_MEJEPA_REPLAY_BUFFER
            ]
        },
        "before": {"tree": before_tree, "cf_counts": before_counts},
        "after": {"tree": tree_state(&fsv_root)?, "cf_counts": after_counts},
        "readback": {
            "cf_counts": readback_counts,
            "mistake_rows": mistake_rows,
            "replay_rows": replay_rows,
            "lifecycle_rows": lifecycle_rows
        },
        "ability_context": ability_context,
        "prediction_label_context": prediction_label_context,
        "refutation_report": refutation_report,
        "candidate_report": candidate_report,
        "checks": {
            "active_report_writes_one_lifecycle_decision_per_skill_or_ability": active_report_pass,
            "candidate_created_when_no_existing_ability_explains_evidence": candidate_pass,
            "drop_reopen_readback_rows_agree": readback_pass,
            "edge_cases_pass": edge_pass,
            "invariants_pass": invariant_pass
        },
        "edge_cases": {
            "ordered_signature_distinguishes_a_then_b_from_b_then_a": edge_ordered_signature_distinguishes_ab,
            "empty_chunk_request_rejected": edge_empty_chunk_request_rejected,
            "target_side_live_label_rejected": edge_target_label_rejected,
            "legacy_mnt_d_db_rejected": edge_mntd_db_rejected,
            "leaky_membership_row_rejected": edge_leaky_membership_rejected
        },
        "required_invariants": {
            "no_new_prediction_head_introduced": true,
            "hidden_intent_inference_used": false,
            "target_side_labels_used_as_live_inputs": false,
            "flat_vector_concat_used": false
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let final_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if final_report != report || !all_passed {
        return Err(format!("TASK-PY-G-121 FSV failed at {}", report_path.display()).into());
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
    PathBuf::from("/tmp/contextgraph-task-py-g-121-runtime-ability-resolver-fsv")
}

fn seed_skill_rows(
    db: &rocksdb::DB,
) -> Result<(Level2SkillRow, Level2SkillRow), Box<dyn std::error::Error>> {
    let skill_a = skill(
        "skill:import_contract_drift:fsv",
        "import_contract_drift",
        vec!["episode:a:shared", "episode:a:other"],
        ["ast_surface:import_from", "pair_relation:provider_symbol"],
    );
    let episodes_a = vec![
        episode(
            "episode:a:shared",
            "chunk:shared",
            "pkg/shared.py",
            "state:shared",
            ["ast_surface:import_from", "pair_relation:provider_symbol"],
        ),
        episode(
            "episode:a:other",
            "chunk:a_only",
            "pkg/a.py",
            "state:other",
            ["ast_surface:import_from", "pair_relation:provider_symbol"],
        ),
    ];
    write_skill_materialization_sync_readback(
        db,
        &materialize_skill_memberships(&skill_a, &episodes_a, NOW)?,
    )?;
    let skill_b = skill(
        "skill:mock_only_change:fsv",
        "mock_only_change",
        vec!["episode:b:shared", "episode:b:other"],
        ["ast_surface:import_from", "pair_relation:mock_patch"],
    );
    let episodes_b = vec![
        episode(
            "episode:b:shared",
            "chunk:shared",
            "pkg/shared.py",
            "state:shared",
            ["ast_surface:import_from", "pair_relation:mock_patch"],
        ),
        episode(
            "episode:b:other",
            "chunk:b_only",
            "pkg/b.py",
            "state:other",
            ["ast_surface:import_from", "pair_relation:mock_patch"],
        ),
    ];
    write_skill_materialization_sync_readback(
        db,
        &materialize_skill_memberships(&skill_b, &episodes_b, NOW + 1)?,
    )?;
    Ok((skill_a, skill_b))
}

fn write_higher_ability_membership(
    db: &rocksdb::DB,
    source_skill_id: &str,
) -> Result<ChunkSkillMembershipRow, Box<dyn std::error::Error>> {
    let source =
        context_graph_mejepa_train::chunk_skill_membership::read_all_chunk_skill_membership_rows(
            db,
        )?
        .into_iter()
        .find(|row| row.skill_id == source_skill_id && row.chunk_id == "chunk:shared")
        .ok_or("missing source membership")?;
    let mut row = source;
    row.skill_id = "ability:shared_runtime_sequence:fsv".to_string();
    row.hierarchy_level = 3;
    row.membership_score = 0.77;
    row.membership_key = membership_key(&row.chunk_id, &row.code_state_key, &row.skill_id)?;
    row.provenance_hashes = vec!["sha256:higherabilityfsv".to_string()];
    row.validate()?;
    let cf = db
        .cf_handle(CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)
        .ok_or("missing membership cf")?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(
        cf,
        row.membership_key.as_bytes(),
        bincode::serialize(&row)?,
        &opts,
    )?;
    db.flush_cf(cf)?;
    Ok(row)
}

fn leaky_membership_rejected(
    root: &Path,
    source_skill_id: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let db_path = root.join("leaky-membership.rocksdb");
    let db = open_ability_resolver_rocksdb(&db_path, true)?;
    let (skill_a, _skill_b) = seed_skill_rows(&db)?;
    let source_id = if source_skill_id.is_empty() {
        &skill_a.skill_id
    } else {
        source_skill_id
    };
    let mut row =
        context_graph_mejepa_train::chunk_skill_membership::read_all_chunk_skill_membership_rows(
            &db,
        )?
        .into_iter()
        .find(|row| row.skill_id == source_id && row.chunk_id == "chunk:shared")
        .ok_or("missing source membership")?;
    row.live_input_allowed = false;
    let cf = db
        .cf_handle(CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)
        .ok_or("missing membership cf")?;
    db.put_cf(cf, row.membership_key.as_bytes(), bincode::serialize(&row)?)?;
    db.flush_cf(cf)?;
    Ok(resolve_ability_context(&db, request("state:shared", &["chunk:shared"])).is_err())
}

fn request(state: &str, chunks: &[&str]) -> AbilityResolverRequest {
    AbilityResolverRequest {
        code_state_key: state.to_string(),
        chunks: chunks
            .iter()
            .map(|chunk_id| LiveChunkInput {
                chunk_id: (*chunk_id).to_string(),
                file_path: Some(path_for_chunk(chunk_id).to_string()),
                accepted_label_ids: vec!["ast_surface:function".to_string()],
            })
            .collect(),
    }
}

fn path_for_chunk(chunk_id: &str) -> &'static str {
    if chunk_id.starts_with("chunk:a_only") {
        "pkg/a.py"
    } else if chunk_id.starts_with("chunk:b_only") {
        "pkg/b.py"
    } else {
        "pkg/shared.py"
    }
}

fn refutation_input(byte: u8, created_at_unix_ms: i64) -> AbilityRefutationInput {
    AbilityRefutationInput {
        prediction_id: PredictionId([byte; 16]),
        predicted_verdict: Verdict::Pass,
        ground_truth_verdict: Verdict::Fail,
        truth_source: MistakeTruthSource::ShiftLogReplay,
        language: "python".to_string(),
        mutation_or_live_cell: "live_project".to_string(),
        named_failure_mode: "failure:runtime_ability".to_string(),
        surprise_z: 2.5,
        coverage_gap_score: 0.7,
        created_at_unix_ms,
    }
}

fn no_forbidden_runtime_policy(label_context: &PredictionLabelContext) -> bool {
    label_context
        .accepted_label_ids
        .iter()
        .chain(label_context.failure_evidence_set_ids.iter())
        .all(|id| {
            !matches!(
                id.split_once(':').map(|(family, _)| family),
                Some("oracle" | "docker" | "test_phase" | "test_result" | "ground_truth")
            )
        })
}

fn joined_rows_agree(
    report: &AbilityRefutationReport,
    mistake_rows: &[context_graph_mejepa_train::mistake_log::MistakeLogRow],
    replay_rows: &[context_graph_mejepa_train::replay_buffer::ReplayBufferRow],
    lifecycle_rows: &[context_graph_mejepa_train::chunk_skill_membership::SkillLifecycleAuditRow],
) -> bool {
    let prediction_id = report.mistake_row.prediction_id;
    let Some(mistake) = mistake_rows
        .iter()
        .find(|row| row.mistake_id == report.mistake_row.mistake_id)
    else {
        return false;
    };
    let Some(replay) = replay_rows
        .iter()
        .find(|row| row.prediction_id == prediction_id)
    else {
        return false;
    };
    let joined_lifecycle = lifecycle_rows
        .iter()
        .filter(|row| row.mistake_id.as_deref() == Some(report.mistake_row.mistake_id.as_str()))
        .collect::<Vec<_>>();
    mistake == &report.mistake_row
        && replay == &report.replay_row
        && joined_lifecycle.len() == report.lifecycle_audits.len()
        && report.lifecycle_audits.iter().all(|expected| {
            joined_lifecycle.contains(&expected)
                && expected.evidence_label_ids == mistake.accepted_label_ids
                && expected.evidence_skill_ids == mistake.active_skill_ids
                && expected.evidence_higher_ability_ids == mistake.active_higher_ability_ids
                && expected.source_membership_keys == mistake.source_membership_keys
        })
        && replay.accepted_label_ids == mistake.accepted_label_ids
        && replay.active_skill_ids == mistake.active_skill_ids
        && replay.active_higher_ability_ids == mistake.active_higher_ability_ids
        && replay.source_membership_keys == mistake.source_membership_keys
        && replay.label_signature_hash.as_deref() == Some(mistake.label_signature_hash.as_str())
        && replay.skill_signature_hash == mistake.skill_signature_hash
        && replay.ability_signature_hash == mistake.ability_signature_hash
        && replay.membership_signature_hash == mistake.membership_signature_hash
}

fn step(
    index: u32,
    chunk_id: &str,
    file_path: &str,
    state: &str,
    label: &str,
) -> SkillStepEvidence {
    SkillStepEvidence {
        step_index: index,
        chunk_id: chunk_id.to_string(),
        file_path: file_path.to_string(),
        code_state_key: state.to_string(),
        accepted_label_ids: vec![label.to_string()],
        group_ids: vec!["group:runtime".to_string()],
    }
}

fn episode(
    episode_id: &str,
    first_chunk: &str,
    first_path: &str,
    state: &str,
    labels: [&str; 2],
) -> SkillEpisodeRow {
    SkillEpisodeRow {
        episode_id: episode_id.to_string(),
        proposed_skill_name: None,
        ordered_steps: vec![
            step(0, first_chunk, first_path, state, labels[0]),
            step(
                1,
                &format!("{first_chunk}:next"),
                first_path,
                state,
                labels[1],
            ),
        ],
        outcome: Some(SkillOutcomeObservation {
            outcome_label_id: "oracle:fail".to_string(),
            verdict: SkillOutcomeVerdict::Fail,
            target_side_supervision_only: true,
        }),
        failure_evidence_set_ids: vec![format!("evidence:{episode_id}")],
        cell_baseline_fail_rate: 0.25,
    }
}

fn skill(
    skill_id: &str,
    skill_name: &str,
    source_episode_ids: Vec<&str>,
    labels: [&str; 2],
) -> Level2SkillRow {
    Level2SkillRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_id: skill_id.to_string(),
        skill_name: skill_name.to_string(),
        parent_group_ids: vec!["group:runtime".to_string()],
        parent_skill_ids: Vec::new(),
        ordered_steps: vec![
            SkillStepTemplate {
                step_index: 0,
                accepted_label_ids: vec![labels[0].to_string()],
                group_ids: vec!["group:runtime".to_string()],
            },
            SkillStepTemplate {
                step_index: 1,
                accepted_label_ids: vec![labels[1].to_string()],
                group_ids: vec!["group:runtime".to_string()],
            },
        ],
        prerequisite_label_ids: vec![labels[0].to_string()],
        transition_edges: vec![SkillTransitionEdge {
            from_step_index: 0,
            to_step_index: 1,
            edge_label: "transition:next".to_string(),
        }],
        support: source_episode_ids.len() as u64,
        confidence: 0.9,
        lift_over_cell_baseline: 0.65,
        stability: 0.8,
        oracle_outcome_distribution: SkillOutcomeDistribution {
            pass: 0,
            fail: source_episode_ids.len() as u64,
            unknown: 0,
        },
        code_state_keys: vec!["state:shared".to_string(), "state:other".to_string()],
        source_episode_ids: source_episode_ids.into_iter().map(str::to_string).collect(),
        failure_evidence_set_ids: vec!["evidence:unit".to_string()],
        live_input_allowed: true,
        promotion_status: SkillPromotionStatus::PromotionReady,
        operator_approved: false,
        created_at_unix_ms: NOW,
    }
}

fn write_label_bridge_fixture(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let accepted = root.join("accepted_label_registry.jsonl");
    let usefulness = root.join("label_usefulness_metrics.jsonl");
    let chunks = root.join("chunk_constellation_labels.jsonl");
    let code_states = root.join("code_state_constellations.jsonl");
    let failure_sets = root.join("failure_evidence_sets.jsonl");
    let labels = [
        "ast_surface:function",
        "ast_surface:import_from",
        "pair_relation:provider_symbol",
        "pair_relation:mock_patch",
    ];
    fs::write(
        &accepted,
        labels
            .iter()
            .map(|label| {
                serde_json::to_string(&json!({
                    "artifact_kind": "python_auto_label_discovery_accepted_label_registry_row",
                    "schema_version": 1,
                    "formula_version": "task-py-g-121-fsv",
                    "label_id": label,
                    "label_hash": format!("sha256:{}", sha256_text(label)),
                    "family": label.split_once(':').map(|(family, _)| family).unwrap_or("unknown"),
                    "status": "accepted_live_input",
                    "support": 42,
                    "fail_rate": 0.5,
                    "lift_over_weighted_category_baseline": 0.2,
                    "live_prediction_input_allowed": true,
                    "learning_update_policy": {
                        "baseline_seeded_from": "TASK-PY-G-121-FSV",
                        "online_rescore": true,
                        "promote_demote_with_new_outcomes": true,
                        "target_labels_are_supervision_only": true
                    }
                }))
                .unwrap()
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )?;
    fs::write(
        &usefulness,
        labels
            .iter()
            .map(|label| {
                serde_json::to_string(&json!({
                    "label_id": label,
                    "family": label.split_once(':').map(|(family, _)| family).unwrap_or("unknown"),
                    "status": "accepted_live_input",
                    "support": 42,
                    "leaky_target": false,
                    "lift_over_weighted_category_baseline": 0.2
                }))
                .unwrap()
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )?;
    fs::write(&chunks, "{}\n")?;
    fs::write(
        &code_states,
        [
            json!({
                "group_key": "state:shared",
                "labels": ["group_scope:code_state", "dominant_surface:function", "code_state_outcome:fail"],
                "live_predictor_labels": ["ast_surface:function"],
                "target_supervision_labels": ["code_state_outcome:fail", "oracle:fail"],
                "slot_identity_preserved": true,
                "flat_vector_concat_used": false
            }),
            json!({
                "group_key": "state:other",
                "labels": ["group_scope:code_state", "dominant_surface:import", "code_state_outcome:pass"],
                "live_predictor_labels": ["ast_surface:import_from"],
                "target_supervision_labels": ["code_state_outcome:pass", "oracle:pass"],
                "slot_identity_preserved": true,
                "flat_vector_concat_used": false
            }),
        ]
        .into_iter()
        .map(|value| serde_json::to_string(&value).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
            + "\n",
    )?;
    fs::write(&failure_sets, "")?;
    let manifest = json!({
        "artifact_kind": "learning_bridge_manifest",
        "schema_version": 1,
        "formula_version": "task-py-g-121-fsv",
        "purpose": "runtime ability resolver FSV",
        "paths": {
            "accepted_label_registry": "accepted_label_registry.jsonl",
            "label_usefulness_metrics": "label_usefulness_metrics.jsonl",
            "chunk_constellation_labels": "chunk_constellation_labels.jsonl",
            "code_state_constellations": "code_state_constellations.jsonl",
            "failure_evidence_sets": "failure_evidence_sets.jsonl"
        },
        "consumers": {"runtime_ability_resolver": "PredictionLabelContext"},
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

fn cf_counts(
    db: &rocksdb::DB,
) -> Result<BTreeMap<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let mut counts = BTreeMap::new();
    for (key, cf) in [
        ("level2Skills", CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS),
        ("chunkSkillMembership", CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP),
        ("skillReverseIndex", CF_MEJEPA_SKILL_REVERSE_INDEX),
        ("skillLifecycleAudit", CF_MEJEPA_SKILL_LIFECYCLE_AUDIT),
        ("mistakeLog", CF_MEJEPA_MISTAKE_LOG),
        ("replayBuffer", CF_MEJEPA_REPLAY_BUFFER),
    ] {
        counts.insert(key.to_string(), json!(count_cf_rows(db, cf)?));
    }
    Ok(counts)
}

fn tree_state(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    if root.exists() {
        collect_files(root, root, &mut files)?;
    }
    files.sort();
    Ok(json!({"root": root, "file_count": files.len(), "files": files}))
}

fn collect_files(
    root: &Path,
    path: &Path,
    files: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_files(root, &entry_path, files)?;
        } else {
            let relative = entry_path.strip_prefix(root)?.display().to_string();
            files.push(relative);
        }
    }
    Ok(())
}

fn sha256_text(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
