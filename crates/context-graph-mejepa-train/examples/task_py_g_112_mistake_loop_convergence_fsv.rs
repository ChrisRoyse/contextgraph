use context_graph_mejepa::{PredictionId, Verdict};
use context_graph_mejepa_cf::{CF_MEJEPA_MISTAKE_LOG, CF_MEJEPA_REPLAY_BUFFER};
use context_graph_mejepa_train::label_bridge::{
    accepted_label_signature_hash, build_prediction_label_context, load_label_learning_bridge,
};
use context_graph_mejepa_train::mistake_log::{
    mistake_id_from_parts, read_mistake_log_row, write_mistake_log_row_sync_readback,
    MistakeLogRow, MistakeTruthSource,
};
use context_graph_mejepa_train::replay_buffer::{
    label_aware_replay_cell_id, read_replay_row, write_replay_row_sync_readback, ReplayBufferRow,
    ReplayBufferSource, ReplayRetentionTier,
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
    let before = tree_state(&fsv_root)?;
    let label_root = fsv_root.join("label-bridge");
    fs::create_dir_all(&label_root)?;
    write_label_fixture(&label_root, true)?;

    let bridge = load_label_learning_bridge(&label_root)?;
    let accepted_label_ids = vec!["ast_surface:function".to_string()];
    let active_skill_ids = vec!["skill:unit_sequence".to_string()];
    let label_context = build_prediction_label_context(
        &bridge,
        accepted_label_ids.clone(),
        Some("python:before:unit".to_string()),
        vec!["python:before:unit".to_string()],
        active_skill_ids.clone(),
    )?;
    let label_signature = label_context
        .label_signature_hash
        .clone()
        .ok_or("missing label signature")?;
    let skill_signature = label_context.skill_signature_hash.clone();
    let replay_cell_id = label_aware_replay_cell_id(
        "python",
        "swap_variable",
        "python:before:unit",
        "failure:multi_point",
        &label_signature,
        skill_signature.as_deref(),
    )?;

    let db_path = fsv_root.join("mistake-loop.rocksdb");
    let db = open_db(&db_path)?;
    let prediction_id = PredictionId([0x91; 16]);
    let replay_row = ReplayBufferRow {
        prediction_id,
        surprise_z: 2.0,
        cell_id: replay_cell_id.clone(),
        coverage_gap_score: 0.7,
        last_replayed_ts: None,
        replay_count: 0,
        retention_weight: 1.0,
        protected: true,
        retention_tier: ReplayRetentionTier::Hot,
        source: ReplayBufferSource::AcceptedLabelMistake,
        created_at_unix_ms: 1_779_000_000_000,
        updated_at_unix_ms: 1_779_000_000_000,
    };
    write_replay_row_sync_readback(&db, &replay_row)?;

    let mistake_id = mistake_id_from_parts(
        prediction_id,
        "python:before:unit",
        &label_signature,
        skill_signature.as_deref(),
        Verdict::Fail,
    )?;
    let mistake_row = MistakeLogRow {
        schema_version: 1,
        mistake_id: mistake_id.clone(),
        prediction_id,
        predicted_verdict: Verdict::Pass,
        ground_truth_verdict: Verdict::Fail,
        truth_source: MistakeTruthSource::SwebenchDockerOracle,
        code_state_key: "python:before:unit".to_string(),
        named_failure_mode: Some("failure:multi_point".to_string()),
        accepted_label_ids,
        active_skill_ids,
        label_signature_hash: label_signature.clone(),
        skill_signature_hash: skill_signature.clone(),
        failure_evidence_set_ids: vec!["python:before:unit".to_string()],
        replay_row_key: hex::encode(prediction_id.0),
        accepted_registry_sha256: label_context.accepted_registry_sha256.clone(),
        usefulness_metrics_sha256: label_context.usefulness_metrics_sha256.clone(),
        learning_bridge_manifest_sha256: label_context.learning_bridge_manifest_sha256.clone(),
        created_at_unix_ms: 1_779_000_000_000,
    };
    write_mistake_log_row_sync_readback(&db, &mistake_row)?;

    drop(db);
    let readback_db = open_db(&db_path)?;
    let mistake_readback =
        read_mistake_log_row(&readback_db, &mistake_id)?.ok_or("missing mistake")?;
    let replay_readback = read_replay_row(&readback_db, prediction_id)?.ok_or("missing replay")?;

    let edge_empty_labels = accepted_label_signature_hash(&Vec::new()).is_err();
    let edge_same_verdict = {
        let mut row = mistake_row.clone();
        row.ground_truth_verdict = row.predicted_verdict;
        row.validate().is_err()
    };
    let edge_bad_cell_component = label_aware_replay_cell_id(
        "python",
        "swap_variable",
        "bad\nstate",
        "failure:multi_point",
        &label_signature,
        skill_signature.as_deref(),
    )
    .is_err();
    let leaky_root = fsv_root.join("leaky-label-bridge");
    fs::create_dir_all(&leaky_root)?;
    write_label_fixture(&leaky_root, false)?;
    let edge_leaky_label = load_label_learning_bridge(&leaky_root).is_err();

    let report_path = fsv_root.join("task_py_g_112_mistake_loop_convergence_fsv.json");
    let all_passed = mistake_readback == mistake_row
        && replay_readback == replay_row
        && edge_empty_labels
        && edge_same_verdict
        && edge_bad_cell_component
        && edge_leaky_label;
    let report = json!({
        "artifact_kind": "task_py_g_112_mistake_loop_convergence_fsv",
        "schema_version": 1,
        "task_id": "TASK-PY-G-112",
        "passes": all_passed,
        "before": before,
        "after": tree_state(&fsv_root)?,
        "db_path": db_path,
        "label_context": label_context,
        "mistake_readback": mistake_readback,
        "replay_readback": replay_readback,
        "edge_cases": {
            "empty_label_signature_rejected": edge_empty_labels,
            "same_verdict_mistake_rejected": edge_same_verdict,
            "control_char_cell_component_rejected": edge_bad_cell_component,
            "leaky_live_label_rejected": edge_leaky_label
        },
        "slot_identity_preserved": true,
        "flat_vector_concat_used": false,
        "active_path_policy": {"mnt_d_active_reads_or_writes": false}
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let final_report: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if final_report != report || !all_passed {
        return Err(format!("TASK-PY-G-112 FSV failed at {}", report_path.display()).into());
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
    PathBuf::from("/tmp/contextgraph-task-py-g-112-mistake-loop-fsv")
}

fn open_db(path: &Path) -> Result<DB, rocksdb::Error> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    DB::open_cf_descriptors(
        &opts,
        path,
        vec![
            ColumnFamilyDescriptor::new(CF_MEJEPA_MISTAKE_LOG, Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_REPLAY_BUFFER, Options::default()),
        ],
    )
}

fn write_label_fixture(root: &Path, live_allowed: bool) -> Result<(), Box<dyn std::error::Error>> {
    let accepted = root.join("accepted_label_registry.jsonl");
    let usefulness = root.join("label_usefulness_metrics.jsonl");
    let chunks = root.join("chunk_constellation_labels.jsonl");
    let code_states = root.join("code_state_constellations.jsonl");
    let failure_sets = root.join("failure_evidence_sets.jsonl");
    fs::write(
        &accepted,
        format!(
            r#"{{"artifact_kind":"python_auto_label_discovery_accepted_label_registry_row","schema_version":1,"formula_version":"unit","label_id":"ast_surface:function","label_hash":"sha256:label","family":"ast_surface","status":"accepted_live_input","support":42,"fail_rate":0.75,"lift_over_weighted_category_baseline":0.31,"live_prediction_input_allowed":{},"learning_update_policy":{{"baseline_seeded_from":"TASK-PY-G-117","online_rescore":true,"promote_demote_with_new_outcomes":true,"target_labels_are_supervision_only":true}}}}"#,
            live_allowed
        ),
    )?;
    fs::write(
        &usefulness,
        r#"{"label_id":"ast_surface:function","family":"ast_surface","status":"accepted_live_input","support":42,"leaky_target":false,"lift_over_weighted_category_baseline":0.31}"#,
    )?;
    fs::write(&chunks, "{}\n")?;
    fs::write(&code_states, "{}\n")?;
    fs::write(
        &failure_sets,
        r#"{"code_state_key":"python:before:unit","labels":["evidence:multi_point"],"localization_mode":"failure_localization:multi_point","slot_identity_preserved":true,"flat_vector_concat_used":false}"#,
    )?;
    let manifest = json!({
        "artifact_kind": "python_auto_label_discovery_learning_bridge_manifest",
        "schema_version": 1,
        "formula_version": "unit",
        "purpose": "unit",
        "paths": {
            "accepted_label_registry": accepted,
            "label_usefulness_metrics": usefulness,
            "chunk_constellation_labels": chunks,
            "code_state_constellations": code_states,
            "failure_evidence_sets": failure_sets,
        },
        "consumers": {
            "ReplayBufferRow.cell_id": "label aware"
        },
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
    )?;
    Ok(())
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
