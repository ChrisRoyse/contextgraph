use super::errors::Result;
use super::helpers::*;
use super::schema_validator::RecommendationValidator;
use super::shift_log::{append_shift, ShiftRecord};
use super::witness_chain::{
    append_witness_entry_for_run, verify_witness_chain_for_run, WitnessOpType,
};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

impl Handlers {
    pub(crate) async fn call_optimizer_record_decision(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_record_decision(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_optimizer_record_recommendation(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_record_recommendation(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_optimizer_record_harness_transition(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_record_harness_transition(args, true).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn optimizer_record_decision(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let session = required_str(&args, "claude_session_id").or_else(|_| session_id(&args))?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let witness_preflight = verify_witness_chain_for_run(&runtime_root, &run_id)?;
    let attempt_dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let optimizer_dir = attempt_dir.join("claude-code-optimizer");
    fs::create_dir_all(&optimizer_dir)
        .map_err(|e| fs_error("CCREALITY_OPTIMIZER_DIR_CREATE_FAILED", &optimizer_dir, e))?;
    let turn = next_turn(&optimizer_dir, "decision-turn-")?;
    let path = optimizer_dir.join(format!("decision-turn-{turn:02}.json"));
    let trigger_path = attempt_dir.join("trigger-decision.json");
    let payload = json!({
        "record_kind": "ccreality_optimizer_decision",
        "schema_version": 1,
        "run_id": run_id,
        "attempt": attempt,
        "turn_number": turn,
        "policy": args.get("policy"),
        "should_run": args.get("should_run"),
        "reasons": args.get("reasons").cloned().unwrap_or_else(|| json!([])),
        "claude_session_id": session,
        "claude_model": args.get("claude_model"),
        "created_at_unix": unix_secs()?,
        "source_of_truth": file_sot(&path)
    });
    write_json_checked(&path, &payload)?;
    write_json_checked(&trigger_path, &payload)?;
    let mut shift = ShiftRecord::new(
        "optimizer_record_decision",
        payload
            .get("claude_session_id")
            .and_then(Value::as_str)
            .unwrap_or("cgreality-stdio-session"),
        json!({"type": "decision", "policy": payload.get("policy"), "should_run": payload.get("should_run")}),
    )?;
    shift.after = json!({"sha256": sha256_file(&path)?});
    shift.delta_summary = json!({"artifact": file_sot(&path)});
    append_shift(&runtime_root, &shift.session_id, &shift)?;
    let witness_append = append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::Decision,
        &sha256_file(&path)?,
    )?;

    Ok(json!({
        "decision_path": file_sot(&path),
        "trigger_decision_path": file_sot(&trigger_path),
        "sha256": sha256_file(&path)?,
        "witness_preflight": witness_preflight,
        "witness_append": witness_append,
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&path)
    }))
}

pub async fn optimizer_record_recommendation(args: Value) -> Result<Value> {
    // Phase 13 T13.2: tolerate JSON-stringified arguments from JSON-RPC clients that
    // collapse typed values (integers, arrays, booleans) into strings before transport.
    let args = coerce_stringified_args(args);
    let validator = RecommendationValidator::load()?;
    validator.validate(&args)?;
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let session = required_str(&args, "claude_session_id")?;
    let turn = required_u64(&args, "turn_number")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let witness_preflight = verify_witness_chain_for_run(&runtime_root, &run_id)?;
    let attempt_dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let optimizer_dir = attempt_dir.join("claude-code-optimizer");
    fs::create_dir_all(&optimizer_dir)
        .map_err(|e| fs_error("CCREALITY_OPTIMIZER_DIR_CREATE_FAILED", &optimizer_dir, e))?;
    let path = optimizer_dir.join(format!("recommendation-turn-{turn:02}.json"));
    let mut payload = args;
    if let Value::Object(map) = &mut payload {
        map.insert(
            "source_of_truth".to_string(),
            Value::String(file_sot(&path)),
        );
        map.insert("created_at_unix".to_string(), json!(unix_secs()?));
    }
    validator.validate(&payload)?;
    write_json_checked(&path, &payload)?;
    let mut shift = ShiftRecord::new(
        "optimizer_record_recommendation",
        &session,
        json!({"type": "recommendation", "status": payload.get("status")}),
    )?;
    shift.after = json!({"sha256": sha256_file(&path)?});
    shift.delta_summary = json!({"artifact": file_sot(&path)});
    append_shift(&runtime_root, &session, &shift)?;
    let witness_append = append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::Recommendation,
        &sha256_file(&path)?,
    )?;

    Ok(json!({
        "recommendation_path": file_sot(&path),
        "sha256": sha256_file(&path)?,
        "witness_preflight": witness_preflight,
        "witness_append": witness_append,
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&path)
    }))
}

pub async fn optimizer_record_harness_transition(
    args: Value,
    append_external_shift: bool,
) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let witness_preflight = verify_witness_chain_for_run(&runtime_root, &run_id)?;
    let dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let path = write_harness_transition(&dir, &args)?;
    let round = read_json(&path)?
        .get("round")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            super::errors::CCRealityError::new(
                "CCREALITY_HARNESS_TRANSITION_ROUND_MISSING",
                "harness transition readback did not contain a numeric round",
                "harness_transition.round",
                "inspect the transition artifact writer before using this transition",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?;
    let session = session_id(&args)?;
    let transition_sha = sha256_file(&path)?;
    let witness_append = append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::HarnessTransition,
        &transition_sha,
    )?;
    let mut shift_id = Value::Null;
    if append_external_shift {
        let file_path = required_str(&args, "file_path")?;
        let before_sha = required_str(&args, "before_sha256")?;
        let after_sha = required_str(&args, "after_sha256")?;
        let preedit_state_raw = required_str(&args, "preedit_state_path")?;
        let preedit_state_path = file_arg_to_path(&preedit_state_raw);
        let preedit_state = read_json(&preedit_state_path)?;
        let before_text_source_of_truth = preedit_state
            .get("before_text_source_of_truth")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                super::errors::CCRealityError::new(
                    "CCREALITY_PREEDIT_BEFORE_TEXT_SOT_MISSING",
                    "pre-edit state did not include before_text_source_of_truth",
                    "preedit_state.before_text_source_of_truth",
                    "use the pre-tool edit hook so Phase 7 can verify the exact before text",
                    json!({"preedit_state_path": preedit_state_path.display().to_string()}),
                    Some(file_sot(&preedit_state_path)),
                )
            })?
            .to_string();
        let before_text_path = file_arg_to_path(&before_text_source_of_truth);
        if !before_text_path.is_file() {
            return Err(super::errors::CCRealityError::new(
                "CCREALITY_PREEDIT_BEFORE_TEXT_SOT_NOT_FOUND",
                "before_text_source_of_truth does not point to a readable file",
                "preedit_state.before_text_source_of_truth",
                "inspect the pre-tool edit hook output and rerun with a valid before text artifact",
                json!({
                    "preedit_state_path": preedit_state_path.display().to_string(),
                    "before_text_source_of_truth": before_text_source_of_truth
                }),
                Some(file_sot(&preedit_state_path)),
            ));
        }
        let after_source_raw = required_str(&args, "after_source_path")?;
        let after_source = file_arg_to_path(&after_source_raw);
        if !after_source.is_file() {
            return Err(super::errors::CCRealityError::new(
                "CCREALITY_AFTER_SOURCE_NOT_FOUND",
                "after_source_path does not point to a readable file",
                "arguments.after_source_path",
                "pass the edited source file path from the post-tool edit hook",
                json!({"after_source_path": after_source_raw}),
                None,
            ));
        }
        let mut shift = ShiftRecord::new(
            "optimizer_record_harness_transition",
            &session,
            json!({
                "type": "file_edit",
                "task_id": format!("{run_id}:{attempt}:round-{round:02}"),
                "path": file_path,
                "harness_transition": file_sot(&path),
                "tests": ["phase7_harness_transition_replay"],
                "problem_statement": format!("Replay harness transition for {file_path}"),
                "os": std::env::consts::OS
            }),
        )?;
        shift.before = json!({
            "sha256": before_sha,
            "text_source_of_truth": before_text_source_of_truth
        });
        shift.after = json!({
            "sha256": after_sha,
            "source_of_truth": file_sot(&after_source)
        });
        shift.delta_summary = json!({
            "artifact": file_sot(&path),
            "lines_added": args.get("lines_added").cloned().unwrap_or(Value::Null),
            "lines_removed": args.get("lines_removed").cloned().unwrap_or(Value::Null),
            "git_diff_stat": args.get("git_diff_stat").cloned().unwrap_or(Value::Null)
        });
        shift.verification = witness_segment_from_append(&witness_append)?;
        shift.harness_transition_path = Some(file_sot(&path));
        append_shift(&runtime_root, &session, &shift)?;
        shift_id = json!(shift.shift_id);
    }
    Ok(json!({
        "transition_path": file_sot(&path),
        "round": round,
        "shift_id": shift_id,
        "witness_preflight": witness_preflight,
        "witness_append": witness_append,
        "source_of_truth": file_sot(&path)
    }))
}

pub(crate) fn witness_segment_from_append(witness_append: &Value) -> Result<Value> {
    let source = witness_append
        .get("source_of_truth")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            super::errors::CCRealityError::new(
                "CCREALITY_WITNESS_APPEND_SOURCE_MISSING",
                "witness append result did not include source_of_truth",
                "witness_append.source_of_truth",
                "inspect witness append output before recording a Phase 7 shift",
                json!({"witness_append": witness_append}),
                None,
            )
        })?;
    let offset = witness_append
        .get("offset")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            super::errors::CCRealityError::new(
                "CCREALITY_WITNESS_APPEND_OFFSET_MISSING",
                "witness append result did not include numeric offset",
                "witness_append.offset",
                "inspect witness append output before recording a Phase 7 shift",
                json!({"witness_append": witness_append}),
                Some(source.to_string()),
            )
        })?;
    let entry_size = witness_append
        .get("entry_size")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            super::errors::CCRealityError::new(
                "CCREALITY_WITNESS_APPEND_ENTRY_SIZE_MISSING",
                "witness append result did not include numeric entry_size",
                "witness_append.entry_size",
                "inspect witness append output before recording a Phase 7 shift",
                json!({"witness_append": witness_append}),
                Some(source.to_string()),
            )
        })?;
    let path = file_arg_to_path(source);
    let bytes =
        fs::read(&path).map_err(|e| fs_error("CCREALITY_WITNESS_SEGMENT_READ_FAILED", &path, e))?;
    let start = usize::try_from(offset)
        .ok()
        .and_then(|idx| idx.checked_mul(usize::try_from(entry_size).ok()?))
        .ok_or_else(|| {
            super::errors::CCRealityError::new(
                "CCREALITY_WITNESS_SEGMENT_OFFSET_OVERFLOW",
                "witness offset and entry_size overflowed usize",
                "witness_append.offset",
                "inspect witness append output before recording a Phase 7 shift",
                json!({"offset": offset, "entry_size": entry_size}),
                Some(source.to_string()),
            )
        })?;
    let entry_size = usize::try_from(entry_size).map_err(|err| {
        super::errors::CCRealityError::new(
            "CCREALITY_WITNESS_SEGMENT_SIZE_OVERFLOW",
            format!("witness entry_size does not fit usize: {err}"),
            "witness_append.entry_size",
            "inspect witness append output before recording a Phase 7 shift",
            json!({"entry_size": witness_append.get("entry_size")}),
            Some(source.to_string()),
        )
    })?;
    let end = start.checked_add(entry_size).ok_or_else(|| {
        super::errors::CCRealityError::new(
            "CCREALITY_WITNESS_SEGMENT_RANGE_OVERFLOW",
            "witness segment byte range overflowed usize",
            "witness_append.offset",
            "inspect witness append output before recording a Phase 7 shift",
            json!({"offset": offset, "entry_size": entry_size}),
            Some(source.to_string()),
        )
    })?;
    let segment = bytes.get(start..end).ok_or_else(|| {
        super::errors::CCRealityError::new(
            "CCREALITY_WITNESS_SEGMENT_RANGE_INVALID",
            "witness append range is outside witness-chain.bin",
            "witness_append.offset",
            "inspect witness-chain durability before recording a Phase 7 shift",
            json!({"offset": offset, "entry_size": entry_size, "len": bytes.len()}),
            Some(source.to_string()),
        )
    })?;
    Ok(json!({
        "witness_chain_segment_hex": hex::encode(segment),
        "witness_chain_segment": {
            "source_of_truth": source,
            "offset": offset,
            "entry_size": entry_size
        }
    }))
}

pub fn write_harness_transition(attempt_dir: &Path, payload: &Value) -> Result<PathBuf> {
    let dir = attempt_dir.join("harness-transitions");
    fs::create_dir_all(&dir)
        .map_err(|e| fs_error("CCREALITY_HARNESS_TRANSITION_DIR_CREATE_FAILED", &dir, e))?;
    let round = next_round(&dir)?;
    let path = dir.join(format!("round-{round:02}.json"));
    let mut body = payload.clone();
    if let Value::Object(map) = &mut body {
        map.insert(
            "record_kind".to_string(),
            json!("ccreality_harness_transition"),
        );
        map.insert("schema_version".to_string(), json!(1));
        map.insert("round".to_string(), json!(round));
        map.insert("created_at_unix".to_string(), json!(unix_secs()?));
        map.insert("source_of_truth".to_string(), json!(file_sot(&path)));
        map.entry("outcome".to_string())
            .or_insert(json!("recorded"));
    }
    write_json_checked(&path, &body)?;
    Ok(path)
}

fn next_turn(dir: &Path, prefix: &str) -> Result<u64> {
    let mut max_turn = 0;
    if dir.is_dir() {
        for entry in fs::read_dir(dir)
            .map_err(|e| fs_error("CCREALITY_OPTIMIZER_DIR_READ_FAILED", dir, e))?
        {
            let entry =
                entry.map_err(|e| fs_error("CCREALITY_OPTIMIZER_DIR_ENTRY_FAILED", dir, e))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(num) = name
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(".json"))
                .and_then(|s| s.parse::<u64>().ok())
            {
                max_turn = max_turn.max(num);
            }
        }
    }
    Ok(max_turn + 1)
}

fn next_round(dir: &Path) -> Result<u64> {
    let mut max_round = 0;
    if dir.is_dir() {
        for entry in fs::read_dir(dir)
            .map_err(|e| fs_error("CCREALITY_HARNESS_TRANSITION_DIR_READ_FAILED", dir, e))?
        {
            let entry = entry
                .map_err(|e| fs_error("CCREALITY_HARNESS_TRANSITION_DIR_ENTRY_FAILED", dir, e))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(num) = name
                .strip_prefix("round-")
                .and_then(|s| s.strip_suffix(".json"))
                .and_then(|s| s.parse::<u64>().ok())
            {
                max_round = max_round.max(num);
            }
        }
    }
    Ok(max_round + 1)
}
