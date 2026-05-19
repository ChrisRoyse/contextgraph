use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use context_graph_mejepa_train::chunk_skill_membership::{
    count_cf_rows, materialize_skill_memberships, membership_key, read_chunk_skill_membership_row,
    read_level2_skill_row, read_skill_lifecycle_audit_row, read_skill_reverse_index_row,
    reverse_index_key, write_skill_materialization_sync_readback, SkillMaterialization,
};
use context_graph_mejepa_train::skill_sequence_discovery::{
    discover_skill_candidates, SkillDiscoveryConfig, SkillEpisodeRow, SkillOutcomeObservation,
    SkillOutcomeVerdict, SkillStepEvidence,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = parse_fsv_root();
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;
    let before_tree = tree_state(&fsv_root)?;

    let manual_review_path = fsv_root.join("manual_review_examples.json");
    let manual_review = manual_review_examples();
    fs::write(
        &manual_review_path,
        serde_json::to_vec_pretty(&manual_review)?,
    )?;

    let failing_episodes = import_contract_drift_episodes();
    let failing_report = discover_skill_candidates(
        &failing_episodes,
        SkillDiscoveryConfig::default(),
        1_779_000_000_000,
    )?;
    let failing_skill = failing_report
        .candidates
        .first()
        .ok_or("missing failing skill candidate")?
        .clone();
    let failing_materialized =
        materialize_skill_memberships(&failing_skill, &failing_episodes, 1_779_000_000_000)?;

    let pending_episodes = live_pending_refactor_episodes();
    let pending_report = discover_skill_candidates(
        &pending_episodes,
        SkillDiscoveryConfig::default(),
        1_779_000_001_000,
    )?;
    let pending_skill = pending_report
        .candidates
        .first()
        .ok_or("missing pending skill candidate")?
        .clone();
    let pending_materialized =
        materialize_skill_memberships(&pending_skill, &pending_episodes, 1_779_000_001_000)?;

    let mut materialized = failing_materialized;
    extend_materialization(&mut materialized, pending_materialized);

    let db_path = fsv_root.join("skill-sequence.rocksdb");
    let db = open_db(&db_path)?;
    let before_counts = cf_counts(&db)?;
    write_skill_materialization_sync_readback(&db, &materialized)?;
    let after_counts = cf_counts(&db)?;
    drop(db);

    let readback_db = open_db(&db_path)?;
    let failing_skill_readback = read_level2_skill_row(&readback_db, &failing_skill.skill_id)?
        .ok_or("missing failing skill readback")?;
    let pending_skill_readback = read_level2_skill_row(&readback_db, &pending_skill.skill_id)?
        .ok_or("missing pending skill readback")?;
    let sample_membership_key = membership_key(
        "chunk:django:consumer_import",
        "state:django:consumer_import",
        &failing_skill.skill_id,
    )?;
    let sample_reverse_key =
        reverse_index_key(&failing_skill.skill_id, "state:django:consumer_import")?;
    let sample_membership = read_chunk_skill_membership_row(&readback_db, &sample_membership_key)?
        .ok_or("missing membership readback")?;
    let sample_reverse = read_skill_reverse_index_row(&readback_db, &sample_reverse_key)?
        .ok_or("missing reverse index readback")?;
    let sample_audit = read_skill_lifecycle_audit_row(
        &readback_db,
        &materialized.lifecycle_audits[0].skill_audit_id,
    )?
    .ok_or("missing lifecycle audit readback")?;

    let edge_leaky_live_label_rejected = {
        let mut episodes = import_contract_drift_episodes();
        episodes[0].ordered_steps[0].accepted_label_ids = vec!["oracle:fail".to_string()];
        discover_skill_candidates(
            &episodes,
            SkillDiscoveryConfig::default(),
            1_779_000_002_000,
        )
        .is_err()
    };
    let edge_missing_order_rejected = {
        let mut episodes = import_contract_drift_episodes();
        episodes[0].ordered_steps[1].step_index = 4;
        discover_skill_candidates(
            &episodes,
            SkillDiscoveryConfig::default(),
            1_779_000_002_000,
        )
        .is_err()
    };
    let edge_low_support_rejected = {
        let report = discover_skill_candidates(
            &[import_contract_drift_episodes().remove(0)],
            SkillDiscoveryConfig::default(),
            1_779_000_002_000,
        )?;
        report.candidates.is_empty()
            && report
                .rejections
                .iter()
                .any(|row| row.reason == "low_support_overfit")
    };
    let edge_live_pending_outcome_emits_active_learning = pending_skill_readback.promotion_status
        == context_graph_mejepa_train::SkillPromotionStatus::ActiveLearning
        && pending_skill_readback.oracle_outcome_distribution.unknown == 2;

    let expected_level2_count = 2;
    let expected_membership_count = 8;
    let expected_reverse_count = 8;
    let expected_audit_count = 2;
    let counts_pass = after_counts["level2Skills"] == expected_level2_count
        && after_counts["chunkSkillMembership"] == expected_membership_count
        && after_counts["skillReverseIndex"] == expected_reverse_count
        && after_counts["skillLifecycleAudit"] == expected_audit_count;
    let readback_pass = failing_skill_readback == failing_skill
        && sample_membership.skill_id == failing_skill.skill_id
        && sample_reverse.skill_id == failing_skill.skill_id
        && sample_audit.candidate_skill_id == Some(failing_skill.skill_id.clone());
    let all_passed = counts_pass
        && readback_pass
        && edge_leaky_live_label_rejected
        && edge_missing_order_rejected
        && edge_low_support_rejected
        && edge_live_pending_outcome_emits_active_learning;

    let report_path = fsv_root.join("task_py_g_118_skill_sequence_discovery_fsv.json");
    let report = json!({
        "artifact_kind": "task_py_g_118_skill_sequence_discovery_fsv",
        "schema_version": 1,
        "task_id": "TASK-PY-G-118",
        "passes": all_passed,
        "before": {
            "tree": before_tree,
            "cf_counts": before_counts
        },
        "after": {
            "tree": tree_state(&fsv_root)?,
            "cf_counts": after_counts
        },
        "db_path": db_path,
        "manual_review_artifact": {
            "path": manual_review_path,
            "sha256": format!("sha256:{}", hex::encode(Sha256::digest(fs::read(&manual_review_path)?)))
        },
        "skill_candidates": {
            "failing": failing_skill_readback,
            "livePending": pending_skill_readback
        },
        "sample_readback": {
            "membershipKey": sample_membership_key,
            "membership": sample_membership,
            "reverseIndexKey": sample_reverse_key,
            "reverseIndex": sample_reverse,
            "lifecycleAudit": sample_audit
        },
        "edge_cases": {
            "leaky_oracle_label_rejected_as_live_input": edge_leaky_live_label_rejected,
            "non_monotonic_missing_order_rejected": edge_missing_order_rejected,
            "low_support_overfit_rejected": edge_low_support_rejected,
            "live_codebase_without_oracle_emits_active_learning_skill": edge_live_pending_outcome_emits_active_learning
        },
        "proof_scope": "deterministic skill-sequence candidate discovery, chunk-skill reverse membership, lifecycle audit rows, sync write/flush/drop-reopen readback over synthetic known-answer episodes",
        "active_path_policy": {
            "mnt_d_active_reads_or_writes": false,
            "target_outcomes_supervise_only": true,
            "flat_vector_concat_used": false
        }
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let final_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if final_report != report || !all_passed {
        return Err(format!("TASK-PY-G-118 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&final_report)?);
    Ok(())
}

fn import_contract_drift_episodes() -> Vec<SkillEpisodeRow> {
    vec![
        SkillEpisodeRow {
            episode_id: "episode:django:import_contract_drift".to_string(),
            proposed_skill_name: Some("import_contract_drift".to_string()),
            ordered_steps: vec![
                step(
                    0,
                    "chunk:django:consumer_import",
                    "django/app/consumer.py",
                    "state:django:consumer_import",
                    "ast_surface:import_from",
                    "group:import_contract",
                ),
                step(
                    1,
                    "chunk:django:provider_symbol",
                    "django/app/provider.py",
                    "state:django:provider_symbol",
                    "pair_relation:caller_missing_symbol",
                    "group:import_contract",
                ),
            ],
            outcome: Some(outcome("oracle:fail", SkillOutcomeVerdict::Fail)),
            failure_evidence_set_ids: vec!["failure_evidence:django:import_contract".to_string()],
            cell_baseline_fail_rate: 0.20,
        },
        SkillEpisodeRow {
            episode_id: "episode:flask:import_contract_drift".to_string(),
            proposed_skill_name: Some("import_contract_drift".to_string()),
            ordered_steps: vec![
                step(
                    0,
                    "chunk:flask:consumer_import",
                    "flask/ext/consumer.py",
                    "state:flask:consumer_import",
                    "ast_surface:import_from",
                    "group:import_contract",
                ),
                step(
                    1,
                    "chunk:flask:provider_symbol",
                    "flask/ext/provider.py",
                    "state:flask:provider_symbol",
                    "pair_relation:caller_missing_symbol",
                    "group:import_contract",
                ),
            ],
            outcome: Some(outcome("oracle:fail", SkillOutcomeVerdict::Fail)),
            failure_evidence_set_ids: vec!["failure_evidence:flask:import_contract".to_string()],
            cell_baseline_fail_rate: 0.20,
        },
    ]
}

fn live_pending_refactor_episodes() -> Vec<SkillEpisodeRow> {
    vec![
        SkillEpisodeRow {
            episode_id: "episode:live:service_split:a".to_string(),
            proposed_skill_name: Some("state_mutation_wrong_layer".to_string()),
            ordered_steps: vec![
                step(
                    0,
                    "chunk:live:controller_state_write",
                    "app/controller.py",
                    "state:live:controller_state_write",
                    "ast_surface:attribute_write",
                    "group:state_boundary",
                ),
                step(
                    1,
                    "chunk:live:model_invariant",
                    "app/model.py",
                    "state:live:model_invariant",
                    "pair_relation:state_write_bypasses_model",
                    "group:state_boundary",
                ),
            ],
            outcome: None,
            failure_evidence_set_ids: Vec::new(),
            cell_baseline_fail_rate: 0.50,
        },
        SkillEpisodeRow {
            episode_id: "episode:live:service_split:b".to_string(),
            proposed_skill_name: Some("state_mutation_wrong_layer".to_string()),
            ordered_steps: vec![
                step(
                    0,
                    "chunk:live:api_state_write",
                    "app/api.py",
                    "state:live:api_state_write",
                    "ast_surface:attribute_write",
                    "group:state_boundary",
                ),
                step(
                    1,
                    "chunk:live:domain_invariant",
                    "app/domain.py",
                    "state:live:domain_invariant",
                    "pair_relation:state_write_bypasses_model",
                    "group:state_boundary",
                ),
            ],
            outcome: None,
            failure_evidence_set_ids: Vec::new(),
            cell_baseline_fail_rate: 0.50,
        },
    ]
}

fn step(
    step_index: u32,
    chunk_id: &str,
    file_path: &str,
    code_state_key: &str,
    label_id: &str,
    group_id: &str,
) -> SkillStepEvidence {
    SkillStepEvidence {
        step_index,
        chunk_id: chunk_id.to_string(),
        file_path: file_path.to_string(),
        code_state_key: code_state_key.to_string(),
        accepted_label_ids: vec![label_id.to_string()],
        group_ids: vec![group_id.to_string()],
    }
}

fn outcome(label: &str, verdict: SkillOutcomeVerdict) -> SkillOutcomeObservation {
    SkillOutcomeObservation {
        outcome_label_id: label.to_string(),
        verdict,
        target_side_supervision_only: true,
    }
}

fn extend_materialization(target: &mut SkillMaterialization, incoming: SkillMaterialization) {
    target.level2_skills.extend(incoming.level2_skills);
    target.chunk_memberships.extend(incoming.chunk_memberships);
    target.reverse_indexes.extend(incoming.reverse_indexes);
    target.lifecycle_audits.extend(incoming.lifecycle_audits);
}

fn cf_counts(db: &DB) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(json!({
        "level2Skills": count_cf_rows(db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS)?,
        "chunkSkillMembership": count_cf_rows(db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)?,
        "skillReverseIndex": count_cf_rows(db, CF_MEJEPA_SKILL_REVERSE_INDEX)?,
        "skillLifecycleAudit": count_cf_rows(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?
    }))
}

fn open_db(path: &Path) -> Result<DB, rocksdb::Error> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    DB::open_cf_descriptors(
        &opts,
        path,
        vec![
            ColumnFamilyDescriptor::new(CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS, Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_SKILL_REVERSE_INDEX, Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, Options::default()),
        ],
    )
}

fn manual_review_examples() -> serde_json::Value {
    json!({
        "artifact_kind": "task_py_g_118_manual_review_examples",
        "examples": [
            {
                "case": "happy",
                "observation": "ordered guard-check and provider symbol labels align with pass outcome; no skill promoted as failure pressure",
                "live_inputs": ["ast_surface:if_guard", "pair_relation:guard_to_return"],
                "target_supervision": "oracle:pass"
            },
            {
                "case": "failing",
                "observation": "consumer import step followed by missing provider symbol recurs with fail outcome",
                "live_inputs": ["ast_surface:import_from", "pair_relation:caller_missing_symbol"],
                "target_supervision": "oracle:fail"
            },
            {
                "case": "multi_chunk",
                "observation": "one skill spans consumer and provider chunks; neither chunk alone carries the complete consequence pattern"
            },
            {
                "case": "multi_file",
                "observation": "consumer.py and provider.py participate in the same skill through a shared code-state consequence"
            },
            {
                "case": "diffuse_cause",
                "observation": "state mutation wrong-layer candidate has no oracle yet, so it is active-learning evidence, not a promoted failure claim"
            }
        ]
    })
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
    PathBuf::from("/tmp/contextgraph-task-py-g-118-skill-sequence-fsv")
}

fn tree_state(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if !root.exists() {
        return Ok(json!({"exists": false}));
    }
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    Ok(json!({"exists": true, "files": files}))
}

fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut Vec<serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            let bytes = fs::read(&path)?;
            files.push(json!({
                "path": path.strip_prefix(root)?.to_string_lossy(),
                "bytes": bytes.len(),
                "sha256": format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
            }));
        }
    }
    Ok(())
}
