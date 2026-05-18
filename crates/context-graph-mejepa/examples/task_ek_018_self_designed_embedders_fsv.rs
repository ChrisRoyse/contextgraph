use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{current_unix_ms, write_json_0600};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-018-self-designed-embedders-fsv";

const SUBTASKS: &[SubtaskInput] = &[
    SubtaskInput {
        key: "TASK-EK-018A",
        issue: 342,
        path: "/mnt/d/contextgraph/fsv/task-ek-018a-dynamic-registration-fsv/run-1779082972512/dynamic_registration_fsv.json",
        required_top_key: "registry",
    },
    SubtaskInput {
        key: "TASK-EK-018B",
        issue: 344,
        path: "/mnt/d/contextgraph/fsv/task-ek-018b-absence-queue-fsv/run-1779083869949/absence_queue_fsv.json",
        required_top_key: "synthetic_injection",
    },
    SubtaskInput {
        key: "TASK-EK-018C",
        issue: 343,
        path: "/mnt/d/contextgraph/fsv/task-ek-018c-falsification-gate-fsv/run-1779084741976/falsification_gate_fsv.json",
        required_top_key: "positive_candidate",
    },
    SubtaskInput {
        key: "TASK-EK-018D",
        issue: 345,
        path: "/mnt/d/contextgraph/fsv/task-ek-018d-algorithmic-embedder-synthesis-fsv/algorithmic_embedder_synthesis_fsv.json",
        required_top_key: "accepted_identifier_length",
    },
    SubtaskInput {
        key: "TASK-EK-018E",
        issue: 347,
        path: "/mnt/d/contextgraph/fsv/task-ek-018e-learned-head-synthesis-fsv/learned_head_synthesis_fsv.json",
        required_top_key: "accepted_learned_head",
    },
    SubtaskInput {
        key: "TASK-EK-018F",
        issue: 348,
        path: "/mnt/d/contextgraph/fsv/task-ek-018f-entity-kge-embedder-fsv/run-1779086521328/entity_kge_embedder_fsv.json",
        required_top_key: "operator_approval",
    },
    SubtaskInput {
        key: "TASK-EK-018G",
        issue: 346,
        path: "/mnt/d/contextgraph/fsv/task-ek-018g-freeze-audit-fsv/run-1779087475469/freeze_audit_fsv.json",
        required_top_key: "freeze_audit",
    },
    SubtaskInput {
        key: "TASK-EK-018H",
        issue: 350,
        path: "/mnt/d/contextgraph/fsv/task-ek-018h-operator-gates-fsv/run-1779085400986/operator_gates_fsv.json",
        required_top_key: "first_promotion",
    },
    SubtaskInput {
        key: "TASK-EK-018I",
        issue: 353,
        path: "/mnt/d/contextgraph/fsv/task-ek-018i-dynamic-vram-fsv/run-1779086976419/dynamic_vram_fsv.json",
        required_top_key: "promotion_budget",
    },
    SubtaskInput {
        key: "TASK-EK-018J",
        issue: 351,
        path: "/mnt/d/contextgraph/fsv/task-ek-018j-adversarial-injection-fsv/run-1779088294511/adversarial_injection_fsv.json",
        required_top_key: "report",
    },
];

#[derive(Debug, Clone, Copy)]
struct SubtaskInput {
    key: &'static str,
    issue: u32,
    path: &'static str,
    required_top_key: &'static str,
}

fn main() -> Result<()> {
    let started_at = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at}"));
    let fsv_path = run_root.join("self_designed_embedders_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("self_designed_embedders_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018 parent FSV run root")?;

    let mut rows = Vec::new();
    let mut loaded = Vec::new();
    for input in SUBTASKS {
        let path = PathBuf::from(input.path);
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let value: Value =
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
        let all_passed = value
            .get("all_passed")
            .or_else(|| value.get("allPassed"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let required_key_present = value.get(input.required_top_key).is_some();
        rows.push(json!({
            "task": input.key,
            "issue": input.issue,
            "path": input.path,
            "sha256": sha256_hex(&bytes),
            "all_passed": all_passed,
            "required_top_key": input.required_top_key,
            "required_top_key_present": required_key_present
        }));
        loaded.push((input.key, value, all_passed && required_key_present));
    }

    let kge = value_for(&loaded, "TASK-EK-018F")?;
    let algorithmic = value_for(&loaded, "TASK-EK-018D")?;
    let learned = value_for(&loaded, "TASK-EK-018E")?;
    let promoted_candidates = vec![
        promoted_candidate(
            "entity-kge-e11-replacement",
            kge.pointer("/falsification/decision")
                .context("missing TASK-EK-018F falsification decision")?,
            kge.pointer("/operator_approval/executed_promotion/state")
                .and_then(Value::as_str)
                .unwrap_or("missing"),
            kge.pointer("/operator_approval/executed_promotion/promotion_id")
                .and_then(Value::as_str),
        )?,
        promoted_candidate(
            "algorithmic-identifier-length",
            algorithmic
                .pointer("/accepted_identifier_length/report/falsificationDecision")
                .context("missing TASK-EK-018D falsification decision")?,
            "not_required_synthetic_subcase",
            None,
        )?,
        promoted_candidate(
            "learned-head-e7-blind-spot",
            learned
                .pointer("/accepted_learned_head/report/falsificationDecision")
                .context("missing TASK-EK-018E falsification decision")?,
            "not_required_synthetic_subcase",
            None,
        )?,
    ];
    let all_subtasks_passed = loaded.iter().all(|(_, _, passed)| *passed);
    let promoted_candidates_valid = promoted_candidates.iter().all(|candidate| {
        candidate["heldout_global_delta"].as_f64().unwrap_or(0.0) >= 0.005
            && candidate["heldout_min_cell_delta"].as_f64().unwrap_or(-1.0) >= 0.0
            && candidate["accepted"].as_bool().unwrap_or(false)
    });
    let operator_approval_audit_present = promoted_candidates.iter().any(|candidate| {
        candidate["operator_approval_state"]
            .as_str()
            .map(|state| state == "executed")
            .unwrap_or(false)
    });
    let v1_synthesis_complete = has_task(&loaded, "TASK-EK-018D")
        && has_task(&loaded, "TASK-EK-018E")
        && has_task(&loaded, "TASK-EK-018F");
    let all_passed = all_subtasks_passed
        && promoted_candidates_valid
        && operator_approval_audit_present
        && v1_synthesis_complete;

    let evidence = json!({
        "task": "TASK-EK-018",
        "issue": 341,
        "all_passed": all_passed,
        "sot": {
            "run_fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "subtask_count": SUBTASKS.len(),
            "source_of_truth": "aggregate of TASK-EK-018A..J FSV artifacts plus promoted-candidate deltas",
        },
        "subtasks": rows,
        "promoted_candidates": promoted_candidates,
        "checks": {
            "all_subtasks_passed": all_subtasks_passed,
            "promoted_candidates_valid": promoted_candidates_valid,
            "operator_approval_audit_present": operator_approval_audit_present,
            "v1_synthesis_complete": v1_synthesis_complete,
            "dynamic_registration": has_task(&loaded, "TASK-EK-018A"),
            "absence_queue": has_task(&loaded, "TASK-EK-018B"),
            "falsification_gate": has_task(&loaded, "TASK-EK-018C"),
            "algorithmic_loop": has_task(&loaded, "TASK-EK-018D"),
            "learned_head_loop": has_task(&loaded, "TASK-EK-018E"),
            "entity_kge_first_instance": has_task(&loaded, "TASK-EK-018F"),
            "freeze_audit": has_task(&loaded, "TASK-EK-018G"),
            "operator_gates": has_task(&loaded, "TASK-EK-018H"),
            "vram_budget": has_task(&loaded, "TASK-EK-018I"),
            "adversarial_injection": has_task(&loaded, "TASK-EK-018J")
        }
    });

    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018 parent run FSV")?;
    write_json_0600(&canonical_path, &evidence)
        .context("write TASK-EK-018 parent canonical FSV")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018 parent FSV")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "fsv_sha256": fsv_sha256,
            "subtask_count": SUBTASKS.len(),
            "promoted_candidate_count": promoted_candidates.len(),
            "operator_approval_audit_present": operator_approval_audit_present
        }))?
    );
    if !all_passed {
        bail!("TASK-EK-018 parent aggregate FSV failed");
    }
    Ok(())
}

fn value_for<'a>(loaded: &'a [(&'static str, Value, bool)], task: &str) -> Result<&'a Value> {
    loaded
        .iter()
        .find(|(key, _, _)| *key == task)
        .map(|(_, value, _)| value)
        .with_context(|| format!("missing {task} FSV input"))
}

fn has_task(loaded: &[(&'static str, Value, bool)], task: &str) -> bool {
    loaded
        .iter()
        .any(|(key, _, passed)| *key == task && *passed)
}

fn promoted_candidate(
    role: &str,
    decision: &Value,
    operator_approval_state: &str,
    operator_approval_id: Option<&str>,
) -> Result<Value> {
    let candidate_name = decision
        .get("candidateName")
        .and_then(Value::as_str)
        .context("missing candidateName")?;
    let accepted = decision
        .get("accepted")
        .and_then(Value::as_bool)
        .context("missing accepted")?;
    let global_delta = decision
        .get("globalDelta")
        .and_then(Value::as_f64)
        .context("missing globalDelta")?;
    let min_cell_delta = decision
        .get("minCellDelta")
        .and_then(Value::as_f64)
        .context("missing minCellDelta")?;
    let compared_cells = decision
        .get("comparedCells")
        .cloned()
        .context("missing comparedCells")?;
    Ok(json!({
        "role": role,
        "candidate_name": candidate_name,
        "accepted": accepted,
        "heldout_global_delta": global_delta,
        "heldout_min_cell_delta": min_cell_delta,
        "operator_approval_state": operator_approval_state,
        "operator_approval_id": operator_approval_id,
        "compared_cells": compared_cells
    }))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
