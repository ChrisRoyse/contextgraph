//! Thompson-sampling policy state for ccreality optimizer decisions.
//!
//! Source of truth: `<runtime_root>/<run_id>/reality-optimizer/solver-bandit.json`.
//! The state is read, mutated, written, and read back on every update.

use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::witness_chain::{
    append_witness_entry_for_run, verify_witness_chain_for_run, WitnessOpType,
};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use rand_distr::{Beta, Distribution};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const BANDIT_SCHEMA_VERSION: u64 = 1;
const DEFAULT_ALPHA: f64 = 1.0;
const DEFAULT_BETA: f64 = 1.0;
const DEFAULT_COST_WEIGHT: f64 = 0.01;
const REWARD_EMA_DECAY: f64 = 0.90;
const COST_EMA_DECAY: f64 = 0.90;
const MAX_ARMS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct SolverBanditState {
    schema_version: u64,
    record_kind: String,
    contexts: BTreeMap<String, BTreeMap<String, ArmStats>>,
    selection_count: u64,
    reward_update_count: u64,
    created_at_unix: u64,
    updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ArmStats {
    alpha: f64,
    beta: f64,
    pulls: u64,
    reward_ema: Option<f64>,
    cost_ema: Option<f64>,
    last_reward: Option<f64>,
    last_cost: Option<f64>,
    last_selected_at_unix: Option<u64>,
    last_reward_at_unix: Option<u64>,
}

impl Default for ArmStats {
    fn default() -> Self {
        Self {
            alpha: DEFAULT_ALPHA,
            beta: DEFAULT_BETA,
            pulls: 0,
            reward_ema: None,
            cost_ema: None,
            last_reward: None,
            last_cost: None,
            last_selected_at_unix: None,
            last_reward_at_unix: None,
        }
    }
}

impl ArmStats {
    fn posterior_mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    fn posterior_variance(&self) -> f64 {
        let sum = self.alpha + self.beta;
        (self.alpha * self.beta) / (sum * sum * (sum + 1.0))
    }

    fn record_reward(&mut self, reward: f64, cost: Option<f64>, unix: u64) {
        self.alpha += reward;
        self.beta += 1.0 - reward;
        self.pulls = self.pulls.saturating_add(1);
        self.reward_ema = Some(match self.reward_ema {
            Some(prev) => REWARD_EMA_DECAY * prev + (1.0 - REWARD_EMA_DECAY) * reward,
            None => reward,
        });
        if let Some(cost) = cost {
            self.cost_ema = Some(match self.cost_ema {
                Some(prev) => COST_EMA_DECAY * prev + (1.0 - COST_EMA_DECAY) * cost,
                None => cost,
            });
            self.last_cost = Some(cost);
        }
        self.last_reward = Some(reward);
        self.last_reward_at_unix = Some(unix);
    }
}

impl SolverBanditState {
    fn new(unix: u64) -> Self {
        Self {
            schema_version: BANDIT_SCHEMA_VERSION,
            record_kind: "ccreality_solver_bandit_state".to_string(),
            contexts: BTreeMap::new(),
            selection_count: 0,
            reward_update_count: 0,
            created_at_unix: unix,
            updated_at_unix: unix,
        }
    }

    fn validate(&self, path: &Path) -> Result<()> {
        if self.schema_version != BANDIT_SCHEMA_VERSION {
            return Err(CCRealityError::new(
                "CCREALITY_BANDIT_SCHEMA_VERSION_UNSUPPORTED",
                "solver-bandit.json has an unsupported schema_version",
                "solver_bandit.schema_version",
                "migrate the bandit state before using it",
                json!({"path": path.display().to_string(), "schema_version": self.schema_version}),
                Some(file_sot(path)),
            ));
        }
        for (ctx, arms) in &self.contexts {
            if ctx.trim().is_empty() || arms.is_empty() {
                return Err(CCRealityError::new(
                    "CCREALITY_BANDIT_STATE_INVALID_CONTEXT",
                    "solver-bandit.json contains an empty context or arm map",
                    "solver_bandit.contexts",
                    "repair the bandit state; contexts and arm maps must be non-empty",
                    json!({"path": path.display().to_string(), "context": ctx}),
                    Some(file_sot(path)),
                ));
            }
            for (arm, stats) in arms {
                if arm.trim().is_empty()
                    || !stats.alpha.is_finite()
                    || !stats.beta.is_finite()
                    || stats.alpha <= 0.0
                    || stats.beta <= 0.0
                {
                    return Err(CCRealityError::new(
                        "CCREALITY_BANDIT_STATE_INVALID_ARM",
                        "solver-bandit.json contains invalid arm statistics",
                        "solver_bandit.contexts.arm",
                        "repair the bandit state; alpha/beta must be finite and > 0",
                        json!({"path": path.display().to_string(), "context": ctx, "arm": arm, "stats": stats}),
                        Some(file_sot(path)),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Handlers {
    pub(crate) async fn call_optimizer_bandit_select(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_bandit_select(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_optimizer_bandit_record_reward(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_bandit_record_reward(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_optimizer_bandit_state(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_bandit_state(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn optimizer_bandit_select(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let decision_point = required_str(&args, "decision_point")?;
    let context_bucket = required_str(&args, "context_bucket")?;
    let arms = required_string_array(&args, "arms")?;
    if arms.is_empty() || arms.len() > MAX_ARMS {
        return Err(CCRealityError::new(
            "CCREALITY_BANDIT_ARMS_INVALID_COUNT",
            "optimizer_bandit_select requires 1..=64 arms",
            "arguments.arms",
            "provide a bounded non-empty generic arm set",
            json!({"arm_count": arms.len()}),
            None,
        ));
    }
    let cost_weight = optional_f64_strict(&args, "cost_weight", DEFAULT_COST_WEIGHT)?;
    if !cost_weight.is_finite() || cost_weight < 0.0 {
        return Err(invalid_f64("cost_weight", args.get("cost_weight")));
    }
    let active = active_bandit_context().await?;
    let witness_preflight = verify_witness_chain_for_run(&active.runtime_root, &active.run_id)?;
    let mut state = load_or_init_state(&active.path)?;
    let unix = unix_secs()?;
    let ctx = context_key(&decision_point, &context_bucket);
    let arm_map = state.contexts.entry(ctx.clone()).or_default();
    for arm in &arms {
        arm_map.entry(arm.clone()).or_default();
    }

    let mut rng = rand::thread_rng();
    let mut scored = Vec::with_capacity(arms.len());
    for arm in &arms {
        let stats = arm_map.get(arm).expect("arm initialized");
        let beta = Beta::new(stats.alpha, stats.beta).map_err(|err| {
            CCRealityError::new(
                "CCREALITY_BANDIT_BETA_INIT_FAILED",
                format!("failed to initialize Beta(alpha,beta): {err}"),
                "solver_bandit.beta",
                "inspect persisted alpha/beta values",
                json!({"context": ctx, "arm": arm, "alpha": stats.alpha, "beta": stats.beta}),
                Some(file_sot(&active.path)),
            )
        })?;
        let sample = beta.sample(&mut rng);
        let cost_penalty = cost_weight * stats.cost_ema.unwrap_or(0.0);
        scored.push((arm.clone(), sample - cost_penalty, sample, cost_penalty));
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let selected_arm = scored[0].0.clone();
    let selected_stats = arm_map
        .get_mut(&selected_arm)
        .expect("selected arm initialized");
    selected_stats.last_selected_at_unix = Some(unix);
    state.selection_count = state.selection_count.saturating_add(1);
    state.updated_at_unix = unix;
    save_state(&active.path, &state)?;
    let readback = load_state_required(&active.path)?;
    let readback_arm = readback
        .contexts
        .get(&ctx)
        .and_then(|arms| arms.get(&selected_arm))
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_BANDIT_READBACK_SELECTED_ARM_MISSING",
                "bandit readback did not contain the selected arm",
                "solver_bandit.readback",
                "inspect solver-bandit.json durability",
                json!({"context": ctx, "selected_arm": selected_arm}),
                Some(file_sot(&active.path)),
            )
        })?;
    if readback_arm.last_selected_at_unix != Some(unix) {
        return Err(CCRealityError::new(
            "CCREALITY_BANDIT_READBACK_SELECTION_MISMATCH",
            "bandit readback did not preserve selected-arm timestamp",
            "solver_bandit.readback.last_selected_at_unix",
            "inspect solver-bandit.json durability",
            json!({"context": ctx, "selected_arm": selected_arm, "expected": unix, "actual": readback_arm.last_selected_at_unix}),
            Some(file_sot(&active.path)),
        ));
    }
    let state_sha256 = sha256_file(&active.path)?;
    let witness_append = append_witness_entry_for_run(
        &active.runtime_root,
        &active.run_id,
        WitnessOpType::BanditSelect,
        &state_sha256,
    )?;
    Ok(json!({
        "status": "ok",
        "decision_point": decision_point,
        "context_bucket": context_bucket,
        "selected_arm": selected_arm,
        "scored_arms": scored.into_iter().map(|(arm, utility_sample, reward_sample, cost_penalty)| {
            json!({
                "arm": arm,
                "utility_sample": utility_sample,
                "reward_sample": reward_sample,
                "cost_penalty": cost_penalty,
            })
        }).collect::<Vec<_>>(),
        "state_sha256": state_sha256,
        "witness_preflight": witness_preflight,
        "witness_append": witness_append,
        "source_of_truth": file_sot(&active.path),
        "readback": {
            "selection_count": readback.selection_count,
            "selected_arm_last_selected_at_unix": readback_arm.last_selected_at_unix,
        }
    }))
}

pub async fn optimizer_bandit_record_reward(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let decision_point = required_str(&args, "decision_point")?;
    let context_bucket = required_str(&args, "context_bucket")?;
    let arm = required_str(&args, "arm")?;
    let reward = required_f64(&args, "reward")?;
    if !(0.0..=1.0).contains(&reward) || !reward.is_finite() {
        return Err(CCRealityError::new(
            "CCREALITY_BANDIT_REWARD_OUT_OF_RANGE",
            "reward must be a finite scalar in [0, 1]",
            "arguments.reward",
            "record clipped correctness/reward evidence, not an unbounded score",
            json!({"reward": args.get("reward")}),
            None,
        ));
    }
    let cost = optional_f64_value_strict(&args, "cost")?;
    if let Some(cost) = cost {
        if !cost.is_finite() || cost < 0.0 {
            return Err(invalid_f64("cost", args.get("cost")));
        }
    }
    let active = active_bandit_context().await?;
    let witness_preflight = verify_witness_chain_for_run(&active.runtime_root, &active.run_id)?;
    let mut state = load_state_required(&active.path)?;
    let ctx = context_key(&decision_point, &context_bucket);
    let arm_stats = state
        .contexts
        .get_mut(&ctx)
        .and_then(|arms| arms.get_mut(&arm))
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_BANDIT_REWARD_ARM_NOT_SELECTED",
                "cannot record reward for an arm absent from solver-bandit.json",
                "arguments.arm",
                "call optimizer_bandit_select with this context and arm before recording reward",
                json!({"decision_point": decision_point, "context_bucket": context_bucket, "arm": arm}),
                Some(file_sot(&active.path)),
            )
        })?;
    let unix = unix_secs()?;
    arm_stats.record_reward(reward, cost, unix);
    state.reward_update_count = state.reward_update_count.saturating_add(1);
    state.updated_at_unix = unix;
    save_state(&active.path, &state)?;
    let readback = load_state_required(&active.path)?;
    let stats = readback
        .contexts
        .get(&ctx)
        .and_then(|arms| arms.get(&arm))
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_BANDIT_READBACK_REWARD_ARM_MISSING",
                "bandit readback did not contain the rewarded arm",
                "solver_bandit.readback",
                "inspect solver-bandit.json durability",
                json!({"context": ctx, "arm": arm}),
                Some(file_sot(&active.path)),
            )
        })?;
    if stats.last_reward != Some(reward) {
        return Err(CCRealityError::new(
            "CCREALITY_BANDIT_READBACK_REWARD_MISMATCH",
            "bandit readback did not preserve latest reward",
            "solver_bandit.readback.last_reward",
            "inspect solver-bandit.json durability",
            json!({"context": ctx, "arm": arm, "expected": reward, "actual": stats.last_reward}),
            Some(file_sot(&active.path)),
        ));
    }
    let state_sha256 = sha256_file(&active.path)?;
    let witness_append = append_witness_entry_for_run(
        &active.runtime_root,
        &active.run_id,
        WitnessOpType::BanditReward,
        &state_sha256,
    )?;
    Ok(json!({
        "status": "ok",
        "decision_point": decision_point,
        "context_bucket": context_bucket,
        "arm": arm,
        "stats": arm_stats_to_json(stats),
        "state_sha256": state_sha256,
        "witness_preflight": witness_preflight,
        "witness_append": witness_append,
        "source_of_truth": file_sot(&active.path),
        "readback": {
            "reward_update_count": readback.reward_update_count,
            "last_reward": stats.last_reward,
            "last_cost": stats.last_cost,
        }
    }))
}

pub async fn optimizer_bandit_state(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let path = active_bandit_path().await?;
    let state = if path.is_file() {
        load_state_required(&path)?
    } else {
        SolverBanditState::new(unix_secs()?)
    };
    let decision_point = optional_str_strict(&args, "decision_point")?;
    let context_bucket = optional_str_strict(&args, "context_bucket")?;
    let contexts = if let (Some(dp), Some(cb)) = (decision_point, context_bucket) {
        let key = context_key(&dp, &cb);
        state
            .contexts
            .get(&key)
            .map(|arms| json!({key: arms_json(arms)}))
            .unwrap_or_else(|| json!({}))
    } else {
        json!(state
            .contexts
            .iter()
            .map(|(ctx, arms)| (ctx.clone(), arms_json(arms)))
            .collect::<BTreeMap<_, _>>())
    };
    Ok(json!({
        "status": "ok",
        "selection_count": state.selection_count,
        "reward_update_count": state.reward_update_count,
        "contexts": contexts,
        "state_sha256": if path.is_file() { sha256_file(&path)? } else { "absent".to_string() },
        "source_of_truth": file_sot(&path),
    }))
}

async fn active_bandit_path() -> Result<PathBuf> {
    active_bandit_context().await.map(|active| active.path)
}

struct ActiveBanditContext {
    runtime_root: PathBuf,
    run_id: String,
    path: PathBuf,
}

async fn active_bandit_context() -> Result<ActiveBanditContext> {
    let runtime_root = require_active_runtime_root().await?;
    let run_id = latest_run_id(&runtime_root)?.ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_BANDIT_NO_ACTIVE_RUN",
            "no active run under the runtime root",
            "active_run",
            "run reality-loop smoke or attempt before using the bandit",
            json!({"runtime_root": runtime_root.display().to_string()}),
            Some(file_sot(&runtime_root)),
        )
    })?;
    let path = runtime_root
        .join(&run_id)
        .join("reality-optimizer")
        .join("solver-bandit.json");
    Ok(ActiveBanditContext {
        runtime_root,
        run_id,
        path,
    })
}

fn load_or_init_state(path: &Path) -> Result<SolverBanditState> {
    if path.is_file() {
        load_state_required(path)
    } else {
        SolverBanditState::new(unix_secs()?).tap_validate(path)
    }
}

trait ValidateSelf {
    fn tap_validate(self, path: &Path) -> Result<Self>
    where
        Self: Sized;
}

impl ValidateSelf for SolverBanditState {
    fn tap_validate(self, path: &Path) -> Result<Self> {
        self.validate(path)?;
        Ok(self)
    }
}

fn load_state_required(path: &Path) -> Result<SolverBanditState> {
    let raw = fs::read_to_string(path)
        .map_err(|e| fs_error("CCREALITY_BANDIT_STATE_READ_FAILED", path, e))?;
    let state: SolverBanditState = serde_json::from_str(&raw).map_err(|err| {
        CCRealityError::new(
            "CCREALITY_BANDIT_STATE_JSON_INVALID",
            format!("solver-bandit.json is not valid bandit state JSON: {err}"),
            "solver_bandit.json",
            "repair or remove corrupt solver-bandit.json before continuing",
            json!({"path": path.display().to_string(), "error": err.to_string()}),
            Some(file_sot(path)),
        )
    })?;
    state.validate(path)?;
    Ok(state)
}

fn save_state(path: &Path, state: &SolverBanditState) -> Result<()> {
    state.validate(path)?;
    write_json_checked(path, state)?;
    let readback = load_state_required(path)?;
    if &readback != state {
        return Err(CCRealityError::new(
            "CCREALITY_BANDIT_STATE_READBACK_MISMATCH",
            "solver-bandit.json readback did not match written state",
            "solver_bandit.readback",
            "inspect filesystem durability before trusting bandit state",
            json!({
                "path": path.display().to_string(),
                "expected_sha256": sha256_text(&serde_json::to_string(state).unwrap_or_default()),
                "actual_sha256": sha256_text(&serde_json::to_string(&readback).unwrap_or_default()),
            }),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn context_key(decision_point: &str, context_bucket: &str) -> String {
    format!("{}::{}", safe_id(decision_point), safe_id(context_bucket))
}

fn required_string_array(args: &Value, field: &str) -> Result<Vec<String>> {
    let Some(items) = args.get(field).and_then(Value::as_array) else {
        return Err(CCRealityError::new(
            "CCREALITY_ARG_MISSING_OR_NOT_STRING_ARRAY",
            format!("argument '{field}' must be an array of non-empty strings"),
            format!("arguments.{field}"),
            format!("provide an array of generic arm strings for arguments.{field}"),
            json!({"args": args}),
            None,
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let Some(value) = item.as_str().filter(|value| !value.is_empty()) else {
            return Err(CCRealityError::new(
                "CCREALITY_ARG_INVALID_STRING_ARRAY_ITEM",
                format!("argument '{field}[{idx}]' must be a non-empty string"),
                format!("arguments.{field}.{idx}"),
                "replace invalid array entries with non-empty generic arm strings",
                json!({"value": item}),
                None,
            ));
        };
        if out.iter().any(|seen| seen == value) {
            return Err(CCRealityError::new(
                "CCREALITY_BANDIT_DUPLICATE_ARM",
                "bandit arms must be unique within a selection call",
                "arguments.arms",
                "deduplicate the arm list before selecting",
                json!({"arm": value}),
                None,
            ));
        }
        out.push(value.to_string());
    }
    Ok(out)
}

fn required_f64(args: &Value, field: &str) -> Result<f64> {
    match args.get(field) {
        Some(Value::Number(n)) => n
            .as_f64()
            .ok_or_else(|| invalid_f64(field, args.get(field))),
        Some(Value::String(s)) => s
            .trim()
            .parse::<f64>()
            .map_err(|_| invalid_f64(field, args.get(field))),
        _ => Err(invalid_f64(field, args.get(field))),
    }
}

fn optional_f64_strict(args: &Value, field: &str, default: f64) -> Result<f64> {
    optional_f64_value_strict(args, field).map(|value| value.unwrap_or(default))
}

fn optional_f64_value_strict(args: &Value, field: &str) -> Result<Option<f64>> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => required_f64(args, field).map(Some),
    }
}

fn invalid_f64(field: &str, value: Option<&Value>) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_ARG_INVALID_F64",
        format!("argument '{field}' must be a finite number"),
        format!("arguments.{field}"),
        format!("omit arguments.{field} or provide a finite number"),
        json!({"value": value}),
        None,
    )
}

fn arms_json(arms: &BTreeMap<String, ArmStats>) -> Value {
    json!(arms
        .iter()
        .map(|(arm, stats)| (arm.clone(), arm_stats_to_json(stats)))
        .collect::<BTreeMap<_, _>>())
}

fn arm_stats_to_json(stats: &ArmStats) -> Value {
    json!({
        "alpha": stats.alpha,
        "beta": stats.beta,
        "pulls": stats.pulls,
        "posterior_mean": stats.posterior_mean(),
        "posterior_variance": stats.posterior_variance(),
        "reward_ema": stats.reward_ema,
        "cost_ema": stats.cost_ema,
        "last_reward": stats.last_reward,
        "last_cost": stats.last_cost,
        "last_selected_at_unix": stats.last_selected_at_unix,
        "last_reward_at_unix": stats.last_reward_at_unix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::MutexGuard;
    use tempfile::TempDir;

    struct TestRuntime {
        _tmp: TempDir,
        _orig_active: Option<String>,
        _guard: MutexGuard<'static, ()>,
        runtime_root: PathBuf,
        run_id: String,
        path: PathBuf,
    }

    fn setup_runtime() -> TestRuntime {
        let guard = super::super::TEST_RUNTIME_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        let run_id = "run-bandit".to_string();
        let run_root = tmp.path().join(&run_id);
        std::fs::create_dir_all(run_root.join("reality-optimizer")).expect("optimizer dir");
        let active_path =
            context_graph_paths::cgreality_cache_file("active_runtime_root").expect("active path");
        let orig_active = std::fs::read_to_string(&active_path).ok();
        std::fs::write(&active_path, tmp.path().to_string_lossy().as_bytes()).expect("active");
        TestRuntime {
            _tmp: tmp,
            _orig_active: orig_active,
            _guard: guard,
            runtime_root: run_root.parent().expect("runtime root").to_path_buf(),
            run_id,
            path: run_root.join("reality-optimizer/solver-bandit.json"),
        }
    }

    impl Drop for TestRuntime {
        fn drop(&mut self) {
            let active_path = context_graph_paths::cgreality_cache_file("active_runtime_root")
                .expect("active path");
            if let Some(orig) = &self._orig_active {
                std::fs::write(active_path, orig).ok();
            } else {
                std::fs::remove_file(active_path).ok();
            }
        }
    }

    fn write_corrupt_witness_chain(rt: &TestRuntime) -> PathBuf {
        let path = rt
            .runtime_root
            .join(&rt.run_id)
            .join("claude-code-optimizer")
            .join("witness-chain.bin");
        std::fs::create_dir_all(path.parent().expect("witness parent")).expect("witness parent");
        let mut bytes = vec![0u8; context_graph_witness::WITNESS_ENTRY_SIZE];
        bytes[0] = 0x7f;
        std::fs::write(&path, bytes).expect("write corrupt witness");
        path
    }

    #[tokio::test]
    async fn select_then_reward_persists_physical_state() {
        let rt = setup_runtime();
        let selected = optimizer_bandit_select(json!({
            "decision_point": "next_prompt_delta",
            "context_bucket": "validation-failed",
            "arms": ["tighten-grounding", "rerun-tests"]
        }))
        .await
        .expect("select");
        let arm = selected["selected_arm"].as_str().unwrap().to_string();

        optimizer_bandit_record_reward(json!({
            "decision_point": "next_prompt_delta",
            "context_bucket": "validation-failed",
            "arm": arm,
            "reward": 0.75,
            "cost": 12.0
        }))
        .await
        .expect("reward");

        let raw: SolverBanditState =
            serde_json::from_str(&std::fs::read_to_string(&rt.path).unwrap()).unwrap();
        let ctx = context_key("next_prompt_delta", "validation-failed");
        let stats = raw.contexts.get(&ctx).unwrap().get(&arm).unwrap();
        assert_eq!(stats.last_reward, Some(0.75));
        assert_eq!(stats.last_cost, Some(12.0));
        assert_eq!(stats.pulls, 1);
    }

    #[tokio::test]
    async fn empty_arm_set_errors_before_writing_state() {
        let rt = setup_runtime();
        let err = optimizer_bandit_select(json!({
            "decision_point": "trigger",
            "context_bucket": "first-attempt",
            "arms": []
        }))
        .await
        .expect_err("empty arms fail");
        assert_eq!(err.error_code, "CCREALITY_BANDIT_ARMS_INVALID_COUNT");
        assert!(!rt.path.exists());
    }

    #[tokio::test]
    async fn reward_unknown_arm_fails_without_creating_context() {
        let rt = setup_runtime();
        let initial = SolverBanditState::new(unix_secs().unwrap());
        save_state(&rt.path, &initial).expect("seed empty state");
        let err = optimizer_bandit_record_reward(json!({
            "decision_point": "trigger",
            "context_bucket": "first-attempt",
            "arm": "run",
            "reward": 1.0
        }))
        .await
        .expect_err("unknown arm fail");
        assert_eq!(err.error_code, "CCREALITY_BANDIT_REWARD_ARM_NOT_SELECTED");
        let raw: SolverBanditState =
            serde_json::from_str(&std::fs::read_to_string(&rt.path).unwrap()).unwrap();
        assert!(raw.contexts.is_empty());
    }

    #[tokio::test]
    async fn select_corrupt_witness_preflight_fails_without_state_file() {
        let rt = setup_runtime();
        let witness_path = write_corrupt_witness_chain(&rt);
        let before_witness_sha = sha256_file(&witness_path).expect("before witness sha");
        let before_state_exists = rt.path.exists();
        let err = optimizer_bandit_select(json!({
            "decision_point": "next_prompt_delta",
            "context_bucket": "validation-failed",
            "arms": ["tighten-grounding", "rerun-tests"]
        }))
        .await
        .expect_err("corrupt witness must fail before state write");
        let after_state_exists = rt.path.exists();
        let after_witness_sha = sha256_file(&witness_path).expect("after witness sha");
        println!(
            "BANDIT_EDGE_CORRUPT_SELECT before_state_exists={before_state_exists} after_state_exists={after_state_exists} before_witness_sha={before_witness_sha} after_witness_sha={after_witness_sha} error_code={}",
            err.error_code
        );
        assert_eq!(err.error_code, "CCREALITY_WITNESS_PREV_HASH_MISMATCH");
        assert!(!before_state_exists);
        assert!(!after_state_exists);
        assert_eq!(before_witness_sha, after_witness_sha);
    }

    #[tokio::test]
    async fn reward_corrupt_witness_preflight_preserves_existing_state() {
        let rt = setup_runtime();
        let selected = optimizer_bandit_select(json!({
            "decision_point": "next_prompt_delta",
            "context_bucket": "validation-failed",
            "arms": ["tighten-grounding", "rerun-tests"]
        }))
        .await
        .expect("select");
        let arm = selected["selected_arm"].as_str().unwrap().to_string();
        let before_state_sha = sha256_file(&rt.path).expect("before state sha");
        let witness_path = write_corrupt_witness_chain(&rt);
        let before_witness_sha = sha256_file(&witness_path).expect("before witness sha");
        let err = optimizer_bandit_record_reward(json!({
            "decision_point": "next_prompt_delta",
            "context_bucket": "validation-failed",
            "arm": arm,
            "reward": 0.75,
            "cost": 12.0
        }))
        .await
        .expect_err("corrupt witness must fail before reward write");
        let after_state_sha = sha256_file(&rt.path).expect("after state sha");
        let after_witness_sha = sha256_file(&witness_path).expect("after witness sha");
        println!(
            "BANDIT_EDGE_CORRUPT_REWARD before_state_sha={before_state_sha} after_state_sha={after_state_sha} before_witness_sha={before_witness_sha} after_witness_sha={after_witness_sha} error_code={}",
            err.error_code
        );
        assert_eq!(err.error_code, "CCREALITY_WITNESS_PREV_HASH_MISMATCH");
        assert_eq!(before_state_sha, after_state_sha);
        assert_eq!(before_witness_sha, after_witness_sha);
    }
}
