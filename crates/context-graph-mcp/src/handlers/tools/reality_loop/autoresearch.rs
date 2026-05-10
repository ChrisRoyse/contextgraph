//! Phase 15: autoresearch MCP tool family.
//!
//! Reads and mutates the durable autoresearch state under
//! `<runtime_root>/<run_id>/reality-optimizer/`:
//!
//! - `experiment-registry.json` — list of harness-change experiments
//! - `champion-state.json`      — best (model, task) result so far
//! - `attempts.jsonl`           — compact per-attempt feed
//! - `reward-series.jsonl`      — scalar reward trace
//! - `decisions.jsonl`          — engine's auto keep/discard verdict
//!
//! Engine writes attempts/reward-series/decisions/champion-state automatically.
//! The optimizer (Claude Code) writes experiment-registry entries via the
//! propose/update tools below; champion promotion remains manual through
//! `champion_state_promote` (Phase 19 gate).

use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::shift_log::{append_shift, ShiftRecord};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u64 = 1;

impl Handlers {
    pub(crate) async fn call_experiment_registry_list(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match experiment_registry_list(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_experiment_registry_get(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match experiment_registry_get(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_champion_state_get(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match champion_state_get(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_attempts_history_query(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match attempts_history_query(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_experiment_registry_propose(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match experiment_registry_propose(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_experiment_registry_update_outcome(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match experiment_registry_update_outcome(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_champion_state_promote(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match champion_state_promote(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

// ============================================================================
// Path resolution
// ============================================================================

async fn reality_optimizer_dir() -> Result<PathBuf> {
    let runtime_root = require_active_runtime_root().await?;
    let run_id = latest_run_id(&runtime_root)?.ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_AUTORESEARCH_NO_ACTIVE_RUN",
            "no active run under the runtime root",
            "active_run",
            "record ME-JEPA outer-loop evidence before querying optimizer state",
            json!({"runtime_root": runtime_root.display().to_string()}),
            None,
        )
    })?;
    Ok(runtime_root.join(run_id).join("reality-optimizer"))
}

fn require_optimizer_dir(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Err(CCRealityError::new(
            "CCREALITY_AUTORESEARCH_DIR_MISSING",
            "<run_root>/reality-optimizer/ does not exist yet",
            "reality_optimizer.dir",
            "record ME-JEPA outer-loop evidence; the evidence capture path creates this dir",
            json!({"path": dir.display().to_string()}),
            Some(file_sot(dir)),
        ));
    }
    Ok(())
}

// ============================================================================
// Analyze: experiment registry
// ============================================================================

pub async fn experiment_registry_list(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let dir = reality_optimizer_dir().await?;
    require_optimizer_dir(&dir)?;
    let path = dir.join("experiment-registry.json");
    if !path.is_file() {
        return Ok(json!({
            "experiments": [],
            "source_of_truth": file_sot(&path)
        }));
    }
    let registry = read_json(&path)?;
    let mut experiments: Vec<Value> =
        registry["experiments"].as_array().cloned().ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_AUTORESEARCH_REGISTRY_INVALID_SHAPE",
                "experiment-registry.json does not contain an array at .experiments",
                "experiment_registry.shape",
                "repair the registry JSON before listing experiments",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?;
    if let Some(filter) = optional_str_strict(&args, "status_filter")? {
        experiments.retain(|e| {
            e.get("outcome")
                .and_then(Value::as_str)
                .map(|s| s == filter)
                .unwrap_or(false)
        });
    }
    let limit = optional_u64_strict(&args, "limit", 100)?.min(500) as usize;
    experiments.truncate(limit);
    Ok(json!({
        "experiments": experiments,
        "source_of_truth": file_sot(&path)
    }))
}

pub async fn experiment_registry_get(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let id = required_str(&args, "experiment_id")?;
    let dir = reality_optimizer_dir().await?;
    require_optimizer_dir(&dir)?;
    let path = dir.join("experiment-registry.json");
    let registry = read_json(&path)?;
    let experiment = registry["experiments"]
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|e| e.get("experiment_id").and_then(Value::as_str) == Some(id.as_str()))
                .cloned()
        })
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_AUTORESEARCH_EXPERIMENT_NOT_FOUND",
                format!("experiment_id '{id}' not in registry"),
                "experiment_id",
                "verify the id with experiment_registry_list",
                json!({"experiment_id": id, "registry": file_sot(&path)}),
                Some(file_sot(&path)),
            )
        })?;
    Ok(json!({
        "experiment": experiment,
        "source_of_truth": file_sot(&path)
    }))
}

// ============================================================================
// Analyze: champion state
// ============================================================================

pub async fn champion_state_get(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let dir = reality_optimizer_dir().await?;
    require_optimizer_dir(&dir)?;
    let path = dir.join("champion-state.json");
    if !path.is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_AUTORESEARCH_CHAMPION_STATE_MISSING",
            "champion-state.json is missing from the optimizer state directory",
            "champion_state.path",
            "record ME-JEPA outer-loop evidence so champion-state.json exists",
            json!({"path": path.display().to_string()}),
            Some(file_sot(&path)),
        ));
    }
    let state = read_json(&path)?;
    let mut champions: Vec<Value> = state["champions"].as_array().cloned().ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_AUTORESEARCH_CHAMPION_INVALID_SHAPE",
            "champion-state.json does not contain an array at .champions",
            "champion_state.shape",
            "repair champion-state.json before reading champions",
            json!({"path": path.display().to_string()}),
            Some(file_sot(&path)),
        )
    })?;
    if let Some(model) = optional_str_strict(&args, "model")? {
        champions.retain(|c| c.get("model").and_then(Value::as_str) == Some(model.as_str()));
    }
    if let Some(task) = optional_str_strict(&args, "task")? {
        champions.retain(|c| c.get("task").and_then(Value::as_str) == Some(task.as_str()));
    }
    Ok(json!({
        "champions": champions,
        "source_of_truth": file_sot(&path)
    }))
}

// ============================================================================
// Analyze: attempts history
// ============================================================================

pub async fn attempts_history_query(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let dir = reality_optimizer_dir().await?;
    require_optimizer_dir(&dir)?;
    let path = dir.join("attempts.jsonl");
    if !path.is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_AUTORESEARCH_ATTEMPTS_MISSING",
            "attempts.jsonl is missing from the optimizer state directory",
            "attempts_history.path",
            "record ME-JEPA outer-loop evidence so attempts.jsonl exists",
            json!({"path": path.display().to_string()}),
            Some(file_sot(&path)),
        ));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| fs_error("CCREALITY_AUTORESEARCH_ATTEMPTS_READ_FAILED", &path, e))?;
    let mut rows = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str::<Value>(line).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_AUTORESEARCH_ATTEMPTS_JSON_INVALID",
                format!("invalid attempts.jsonl JSON at line {}: {e}", idx + 1),
                "attempts_history.parse",
                "repair or remove the corrupt attempts.jsonl line",
                json!({"path": path.display().to_string(), "line": idx + 1}),
                Some(file_sot(&path)),
            )
        })?);
    }
    if let Some(model) = optional_str_strict(&args, "model")? {
        rows.retain(|r| r.get("model").and_then(Value::as_str) == Some(model.as_str()));
    }
    if let Some(task) = optional_str_strict(&args, "instance_id")? {
        rows.retain(|r| r.get("instance_id").and_then(Value::as_str) == Some(task.as_str()));
    }
    if let Some(filter) = args.get("metadata_filter") {
        rows = super::reflexion::apply_metadata_filter(rows, filter)?;
    }
    let limit = optional_u64_strict(&args, "limit", 100)?.min(500) as usize;
    let total = rows.len();
    if rows.len() > limit {
        rows.drain(0..rows.len() - limit);
    }
    Ok(json!({
        "attempts": rows,
        "total_filtered_rows": total,
        "source_of_truth": file_sot(&path)
    }))
}

// ============================================================================
// Alter: experiment registry propose / update
// ============================================================================

pub async fn experiment_registry_propose(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let summary = required_str(&args, "harness_change_summary")?;
    let session = required_str(&args, "claude_session_id").or_else(|_| session_id(&args))?;
    let dir = reality_optimizer_dir().await?;
    require_optimizer_dir(&dir)?;
    let path = dir.join("experiment-registry.json");
    let mut registry = if path.is_file() {
        read_json(&path)?
    } else {
        json!({
            "schema_version": SCHEMA_VERSION,
            "record_kind": "ccreality_experiment_registry",
            "experiments": []
        })
    };
    let unix = unix_secs()?;
    let experiment_id = format!(
        "exp-{unix}-{}",
        safe_id(&summary).chars().take(40).collect::<String>()
    );
    let before_attempt_count = optional_u64_strict(&args, "before_attempt_count", 0)?;
    let entry = json!({
        "experiment_id": experiment_id,
        "harness_change_summary": summary,
        "harness_change_recommendation_path": args.get("harness_change_recommendation_path"),
        "harness_change_shift_ids": args.get("harness_change_shift_ids").cloned().unwrap_or_else(|| json!([])),
        "files_changed": args.get("files_changed").cloned().unwrap_or_else(|| json!([])),
        "before_attempt_count": before_attempt_count,
        "after_attempt_count": 0,
        "outcome": "pending",
        "reasoning": Value::Null,
        "claude_session_id": session,
        "created_at_unix": unix,
        "updated_at_unix": unix,
    });
    registry["experiments"]
        .as_array_mut()
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_AUTORESEARCH_REGISTRY_INVALID_SHAPE",
                "experiment-registry.json does not contain an array at .experiments",
                "experiment_registry.shape",
                "wipe the file so the engine reinitializes it",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?
        .push(entry.clone());
    write_json_checked(&path, &registry)?;
    let runtime_root = require_active_runtime_root().await?;
    let mut shift = ShiftRecord::new(
        "experiment_registry_propose",
        &session,
        json!({"type": "experiment", "experiment_id": experiment_id}),
    )?;
    shift.after = json!({"sha256": sha256_file(&path)?});
    shift.delta_summary = json!({"artifact": file_sot(&path), "experiment_id": experiment_id});
    append_shift(&runtime_root, &session, &shift)?;
    Ok(json!({
        "experiment_id": experiment_id,
        "registry_path": file_sot(&path),
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&path)
    }))
}

pub async fn experiment_registry_update_outcome(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let id = required_str(&args, "experiment_id")?;
    let outcome = required_str(&args, "outcome")?;
    let reasoning = optional_str_strict(&args, "reasoning")?.unwrap_or_default();
    let session = required_str(&args, "claude_session_id").or_else(|_| session_id(&args))?;
    if !matches!(
        outcome.as_str(),
        "pending" | "kept" | "discarded" | "escalated" | "promoted"
    ) {
        return Err(CCRealityError::new(
            "CCREALITY_AUTORESEARCH_INVALID_OUTCOME",
            format!("outcome '{outcome}' is not one of pending|kept|discarded|escalated|promoted"),
            "outcome",
            "use one of the listed enum values",
            json!({"outcome": outcome}),
            None,
        ));
    }
    let dir = reality_optimizer_dir().await?;
    require_optimizer_dir(&dir)?;
    let path = dir.join("experiment-registry.json");
    let mut registry = read_json(&path)?;
    let experiments = registry["experiments"].as_array_mut().ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_AUTORESEARCH_REGISTRY_INVALID_SHAPE",
            "experiment-registry.json does not contain an array at .experiments",
            "experiment_registry.shape",
            "wipe the file so the engine reinitializes it",
            json!({"path": path.display().to_string()}),
            Some(file_sot(&path)),
        )
    })?;
    let entry = experiments
        .iter_mut()
        .find(|e| e.get("experiment_id").and_then(Value::as_str) == Some(id.as_str()))
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_AUTORESEARCH_EXPERIMENT_NOT_FOUND",
                format!("experiment_id '{id}' not in registry"),
                "experiment_id",
                "verify the id with experiment_registry_list",
                json!({"experiment_id": id, "registry": file_sot(&path)}),
                Some(file_sot(&path)),
            )
        })?;
    if let Value::Object(map) = entry {
        map.insert("outcome".to_string(), Value::String(outcome.clone()));
        map.insert("reasoning".to_string(), Value::String(reasoning));
        map.insert("updated_at_unix".to_string(), json!(unix_secs()?));
    }
    write_json_checked(&path, &registry)?;
    let runtime_root = require_active_runtime_root().await?;
    let mut shift = ShiftRecord::new(
        "experiment_registry_update_outcome",
        &session,
        json!({"type": "experiment_outcome", "experiment_id": id, "outcome": outcome}),
    )?;
    shift.after = json!({"sha256": sha256_file(&path)?});
    shift.delta_summary =
        json!({"artifact": file_sot(&path), "experiment_id": id, "outcome": outcome});
    append_shift(&runtime_root, &session, &shift)?;
    Ok(json!({
        "experiment_id": id,
        "outcome": outcome,
        "registry_path": file_sot(&path),
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&path)
    }))
}

// ============================================================================
// Alter: champion promotion (Phase 19 gate)
// ============================================================================

pub async fn champion_state_promote(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let model = required_str(&args, "model")?;
    let task = required_str(&args, "task")?;
    let justification = required_str(&args, "justification")?;
    let session = required_str(&args, "claude_session_id").or_else(|_| session_id(&args))?;
    let dir = reality_optimizer_dir().await?;
    require_optimizer_dir(&dir)?;
    let path = dir.join("champion-state.json");
    let mut state = read_json(&path)?;
    let champions = state["champions"].as_array_mut().ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_AUTORESEARCH_CHAMPION_INVALID_SHAPE",
            "champion-state.json does not contain an array at .champions",
            "champion_state.shape",
            "wipe the file so the engine reinitializes it",
            json!({"path": path.display().to_string()}),
            Some(file_sot(&path)),
        )
    })?;
    let entry = champions
        .iter_mut()
        .find(|c| {
            c.get("model").and_then(Value::as_str) == Some(model.as_str())
                && c.get("task").and_then(Value::as_str) == Some(task.as_str())
        })
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_AUTORESEARCH_CHAMPION_NOT_FOUND",
                format!("no candidate champion for ({model}, {task})"),
                "champion.lookup",
                "verify with champion_state_get",
                json!({"model": model, "task": task}),
                Some(file_sot(&path)),
            )
        })?;
    if entry.get("official_resolved").and_then(Value::as_bool) != Some(true) {
        return Err(CCRealityError::new(
            "CCREALITY_AUTORESEARCH_CHAMPION_NOT_RESOLVED",
            "candidate champion must have official_resolved=true before promotion",
            "champion.official_resolved",
            "wait for an attempt that lands resolved_ids",
            json!({"model": model, "task": task}),
            Some(file_sot(&path)),
        ));
    }
    if let Value::Object(map) = entry {
        map.insert("promoted_at_unix".to_string(), json!(unix_secs()?));
        map.insert(
            "promotion_justification".to_string(),
            Value::String(justification.clone()),
        );
        map.insert(
            "promoted_by_session".to_string(),
            Value::String(session.clone()),
        );
    }
    write_json_checked(&path, &state)?;
    let runtime_root = require_active_runtime_root().await?;
    let mut shift = ShiftRecord::new(
        "champion_state_promote",
        &session,
        json!({"type": "champion_promotion", "model": model, "task": task}),
    )?;
    shift.after = json!({"sha256": sha256_file(&path)?});
    shift.delta_summary = json!({"artifact": file_sot(&path), "model": model, "task": task});
    append_shift(&runtime_root, &session, &shift)?;
    Ok(json!({
        "model": model,
        "task": task,
        "champion_state_path": file_sot(&path),
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&path)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::MutexGuard;
    use tempfile::TempDir;

    // These tests mutate process-wide cgreality cache files, so they share the
    // reality_loop-wide lock with bandit/optimizer FSV tests.
    fn lock_runtime() -> MutexGuard<'static, ()> {
        super::super::TEST_RUNTIME_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    struct TestRuntime {
        _tmp: TempDir,
        _orig_active: Option<String>,
        _orig_target: Option<String>,
        _guard: MutexGuard<'static, ()>,
        run_root: PathBuf,
    }

    fn setup_runtime() -> TestRuntime {
        let _guard = lock_runtime();
        let tmp = tempfile::tempdir().expect("tmpdir");
        let runtime_root = tmp.path().to_path_buf();
        let run_id = "test-run-1";
        let run_root = runtime_root.join(run_id);
        let optimizer_dir = run_root.join("reality-optimizer");
        std::fs::create_dir_all(&optimizer_dir).expect("optimizer dir");

        // Write minimal champion + registry seeds
        std::fs::write(
            optimizer_dir.join("experiment-registry.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": 1,
                "record_kind": "ccreality_experiment_registry",
                "experiments": []
            }))
            .unwrap(),
        )
        .expect("write registry");
        std::fs::write(
            optimizer_dir.join("champion-state.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": 1,
                "record_kind": "ccreality_champion_state",
                "champions": [{
                    "model": "candidate-model-a",
                    "task": "psf__requests-2317",
                    "repo": "psf/requests",
                    "attempt": 1,
                    "official_resolved": true,
                    "best_reward": 4.31,
                    "patch_sha256": "sha256:abc",
                    "promoted_at_unix": Value::Null
                }]
            }))
            .unwrap(),
        )
        .expect("write champion");
        std::fs::write(
            optimizer_dir.join("attempts.jsonl"),
            r#"{"model":"candidate-model-a","instance_id":"psf__requests-2317","attempt":1,"reward_scalar":4.31}
{"model":"candidate-model-a","instance_id":"psf__requests-2317","attempt":2,"reward_scalar":4.78}
"#,
        ).expect("write attempts");

        // Point cgreality cache files at this runtime
        let active_path =
            context_graph_paths::cgreality_cache_file("active_runtime_root").expect("active path");
        let target_path = context_graph_paths::cgreality_cache_file("active_target_instance")
            .expect("target path");
        let orig_active = std::fs::read_to_string(&active_path).ok();
        let orig_target = std::fs::read_to_string(&target_path).ok();
        std::fs::write(&active_path, runtime_root.to_string_lossy().as_bytes()).expect("active");
        std::fs::write(&target_path, b"psf__requests-2317").expect("target");

        TestRuntime {
            _tmp: tmp,
            _orig_active: orig_active,
            _orig_target: orig_target,
            _guard,
            run_root,
        }
    }

    impl Drop for TestRuntime {
        fn drop(&mut self) {
            let active_path = context_graph_paths::cgreality_cache_file("active_runtime_root")
                .expect("active path");
            let target_path = context_graph_paths::cgreality_cache_file("active_target_instance")
                .expect("target path");
            if let Some(orig) = &self._orig_active {
                std::fs::write(&active_path, orig).ok();
            } else {
                std::fs::remove_file(&active_path).ok();
            }
            if let Some(orig) = &self._orig_target {
                std::fs::write(&target_path, orig).ok();
            } else {
                std::fs::remove_file(&target_path).ok();
            }
        }
    }

    #[tokio::test]
    async fn champion_state_get_filters_by_model() {
        let _rt = setup_runtime();
        let result = champion_state_get(json!({"model": "candidate-model-a"}))
            .await
            .expect("ok");
        let champions = result["champions"].as_array().unwrap();
        assert_eq!(champions.len(), 1);
        assert_eq!(champions[0]["task"].as_str(), Some("psf__requests-2317"));
    }

    #[tokio::test]
    async fn champion_state_get_returns_empty_when_no_match() {
        let _rt = setup_runtime();
        let result = champion_state_get(json!({"model": "no-such-model"}))
            .await
            .expect("ok");
        let champions = result["champions"].as_array().unwrap();
        assert!(champions.is_empty());
    }

    #[tokio::test]
    async fn attempts_history_query_filters_and_limits() {
        let _rt = setup_runtime();
        let result = attempts_history_query(json!({
            "instance_id": "psf__requests-2317",
            "limit": 10
        }))
        .await
        .expect("ok");
        assert_eq!(result["attempts"].as_array().unwrap().len(), 2);
        assert_eq!(result["total_filtered_rows"].as_u64().unwrap(), 2);
    }

    #[tokio::test]
    async fn experiment_registry_propose_then_update_round_trip() {
        let rt = setup_runtime();
        let propose = experiment_registry_propose(json!({
            "harness_change_summary": "added open_test_file tool",
            "claude_session_id": "test-session-1",
            "files_changed": ["crates/context-graph-cli/src/bin/reality-loop.rs"]
        }))
        .await
        .expect("propose");
        let exp_id = propose["experiment_id"].as_str().unwrap().to_string();

        let list_after_propose = experiment_registry_list(json!({})).await.expect("list");
        assert_eq!(
            list_after_propose["experiments"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            list_after_propose["experiments"][0]["outcome"].as_str(),
            Some("pending")
        );

        experiment_registry_update_outcome(json!({
            "experiment_id": exp_id,
            "outcome": "kept",
            "reasoning": "reward improved by 0.27 across next 3 attempts",
            "claude_session_id": "test-session-1"
        }))
        .await
        .expect("update");

        let list_after_update = experiment_registry_list(json!({})).await.expect("list2");
        assert_eq!(
            list_after_update["experiments"][0]["outcome"].as_str(),
            Some("kept")
        );

        // verify the change is durable on disk
        let registry_path = rt
            .run_root
            .join("reality-optimizer/experiment-registry.json");
        let raw: Value =
            serde_json::from_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
        assert_eq!(raw["experiments"][0]["outcome"].as_str(), Some("kept"));
    }

    #[tokio::test]
    async fn champion_state_promote_requires_official_resolved() {
        let rt = setup_runtime();
        // First add a non-resolved candidate
        let path = rt.run_root.join("reality-optimizer/champion-state.json");
        let mut state: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        state["champions"].as_array_mut().unwrap().push(json!({
            "model": "candidate-model-a",
            "task": "django__django-12908",
            "official_resolved": false,
            "best_reward": 1.0
        }));
        std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

        let err = champion_state_promote(json!({
            "model": "candidate-model-a",
            "task": "django__django-12908",
            "justification": "trying to short-circuit",
            "claude_session_id": "test"
        }))
        .await
        .expect_err("must reject");
        assert_eq!(
            err.error_code,
            "CCREALITY_AUTORESEARCH_CHAMPION_NOT_RESOLVED"
        );
    }

    #[tokio::test]
    async fn experiment_registry_update_rejects_invalid_outcome() {
        let _rt = setup_runtime();
        let propose = experiment_registry_propose(json!({
            "harness_change_summary": "x",
            "claude_session_id": "s"
        }))
        .await
        .unwrap();
        let exp_id = propose["experiment_id"].as_str().unwrap().to_string();
        let err = experiment_registry_update_outcome(json!({
            "experiment_id": exp_id,
            "outcome": "garbage",
            "claude_session_id": "s"
        }))
        .await
        .expect_err("invalid");
        assert_eq!(err.error_code, "CCREALITY_AUTORESEARCH_INVALID_OUTCOME");
    }
}
