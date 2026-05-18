use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use context_graph_mejepa::heal::cf::{decode_value, encode_plasticity_history_key};
use context_graph_mejepa::heal::ewc::TaskSnapshot;
use context_graph_mejepa::heal::integrity::{ChainIntegrityChecker, WitnessChainAppender};
use context_graph_mejepa::heal::pipeline::{
    LinearHealPredictor, MeJepaHealConfig, SelfHealingPipeline, StatusChange,
};
use context_graph_mejepa::heal::plasticity::{
    tick_predictor_plasticity, PlasticityHistoryRow, EWC_BLOCKED_REINIT_BUDGET_ALERT,
    PLASTICITY_COLLAPSE_ALERT, PLASTICITY_REGRESSION_ALERT,
};
use context_graph_mejepa::heal::store::HealRocksStore;
use context_graph_mejepa::SystemCostCounters;
use context_graph_mejepa_cf::CF_MEJEPA_PLASTICITY_HISTORY;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-066-continual-backprop-fsv";
const PARAM_COUNT: usize = 1_000;
const ITERATIONS: usize = 50_000;

fn main() -> Result<()> {
    let run_root = run_root()?;
    fs::create_dir_all(&run_root).context("create FSV run root")?;

    let bounded = repeating_pattern_plasticity_bound(&run_root)?;
    let regression = regression_alert_after_three_crosses(&run_root)?;
    let collapse = all_dormant_collapse(&run_root)?;
    let ewc = ewc_precedence_blocks_reinit(&run_root)?;
    let healthy = healthy_predictor_noop(&run_root)?;
    let invalid = invalid_gradient_fails_closed(&run_root)?;

    let all_passed = bounded["passed"].as_bool() == Some(true)
        && regression["passed"].as_bool() == Some(true)
        && collapse["passed"].as_bool() == Some(true)
        && ewc["passed"].as_bool() == Some(true)
        && healthy["passed"].as_bool() == Some(true)
        && invalid["passed"].as_bool() == Some(true);

    let report = json!({
        "task": "TASK-PY-G-066",
        "issue": 285,
        "allPassed": all_passed,
        "sourceOfTruth": {
            "plasticityHistoryCf": CF_MEJEPA_PLASTICITY_HISTORY,
            "weightSource": "SelfHealingPipeline::predictor.weights",
            "counter": "SystemCostCounters::dormant_units_reinit_total",
            "fsvRoot": run_root
        },
        "cases": {
            "repeatingPatternPlasticityBound": bounded,
            "regressionAlert": regression,
            "allDormantCollapse": collapse,
            "ewcPrecedence": ewc,
            "healthyNoop": healthy,
            "invalidGradient": invalid
        }
    });

    let report_path = run_root.join("continual_backprop_fsv.json");
    let bytes = serde_json::to_vec_pretty(&report).context("serialize FSV report")?;
    fs::write(&report_path, &bytes).context("write FSV report")?;
    let readback = fs::read(&report_path).context("readback FSV report")?;
    if readback != bytes {
        bail!("FSV report readback mismatch");
    }
    let sha256 = hex::encode(Sha256::digest(&readback));
    println!(
        "task-py-g-066 continual backprop FSV allPassed={} path={} sha256={}",
        all_passed,
        report_path.display(),
        sha256
    );
    if !all_passed {
        bail!("TASK-PY-G-066 continual-backprop FSV failed");
    }
    Ok(())
}

fn run_root() -> Result<PathBuf> {
    let stamp = chrono::Utc::now().format("run-%Y%m%dT%H%M%SZ").to_string();
    Ok(Path::new(FSV_ROOT).join(stamp))
}

fn repeating_pattern_plasticity_bound(run_root: &Path) -> Result<Value> {
    let baseline = baseline_repeating_pattern_dormancy();
    let case_root = run_root.join("bounded-repeating-pattern");
    let (mut pipeline, storage, counters) = pipeline_for_case(&case_root, PARAM_COUNT)?;
    let mut active_until = vec![0usize; PARAM_COUNT];
    let mut max_dormancy = 0.0f32;
    let mut final_row = None;
    let mut checkpoints = Vec::new();

    for tick in 0..ITERATIONS {
        pipeline.status.lock().unwrap().observation_counter = tick as u64;
        let row = repeating_pattern_activation_row(tick, &active_until);
        push_activation_row(&mut pipeline, row);
        let gradient = repeating_pattern_gradient(PARAM_COUNT);
        let history = tick_predictor_plasticity(&mut pipeline, &gradient)?;
        for &unit in &history.reinitialized_units {
            active_until[unit] = tick.saturating_add(1_000);
        }
        max_dormancy = max_dormancy.max(history.dormancy_fraction);
        if tick % 5_000 == 0 || tick + 1 == ITERATIONS {
            checkpoints.push(json!({
                "tick": tick + 1,
                "dormancyFraction": history.dormancy_fraction,
                "reinitCount": history.reinit_count,
                "consecutiveRegressionTicks": history.consecutive_regression_ticks,
                "alerts": history.alert_codes,
            }));
        }
        final_row = Some(history);
    }
    let final_row = final_row.context("missing final plasticity row")?;
    let counter_after = counters.snapshot().dormant_units_reinit_total;
    drop(pipeline);
    drop(storage);
    let reopened = HealRocksStore::open(case_root.join("rocks"))?;
    let reopened_count = reopened.count_cf(CF_MEJEPA_PLASTICITY_HISTORY)?;
    let reopened_final = read_history_row(reopened.as_ref(), ITERATIONS as u64)?;
    let passed = baseline["monotonic"].as_bool() == Some(true)
        && baseline["finalDormancy"].as_f64().unwrap_or(0.0) > 0.30
        && max_dormancy < 0.20
        && final_row == reopened_final
        && reopened_count == ITERATIONS as u64
        && counter_after > 0;
    Ok(json!({
        "passed": passed,
        "iterations": ITERATIONS,
        "parameterCount": PARAM_COUNT,
        "withoutCbp": baseline,
        "withCbp": {
            "maxDormancyFraction": max_dormancy,
            "finalDormancyFraction": final_row.dormancy_fraction,
            "finalReinitRate": final_row.reinit_rate,
            "finalReinitCount": final_row.reinit_count,
            "historyRowsBeforeReopen": ITERATIONS,
            "historyRowsAfterReopen": reopened_count,
            "dormantUnitsReinitCounter": counter_after,
            "reopenedFinalRowEqual": final_row == reopened_final,
            "checkpoints": checkpoints
        },
        "expected": {
            "withoutCbpFinalDormancy": "> 0.30",
            "withoutCbpDormancyMonotonic": true,
            "withCbpMaxDormancy": "< 0.20",
            "plasticityHistoryRows": ITERATIONS
        }
    }))
}

fn regression_alert_after_three_crosses(run_root: &Path) -> Result<Value> {
    let case_root = run_root.join("regression-alert");
    let (mut pipeline, storage, counters) = pipeline_for_case(&case_root, PARAM_COUNT)?;
    let before_rate = pipeline.plasticity_state.current_reinit_rate;
    let mut rows = Vec::new();
    for tick in 0..3 {
        pipeline.status.lock().unwrap().observation_counter = tick as u64;
        let mut activation = vec![0.2; PARAM_COUNT];
        for value in activation.iter_mut().take(160) {
            *value = 0.0;
        }
        push_activation_row(&mut pipeline, activation);
        rows.push(tick_predictor_plasticity(
            &mut pipeline,
            &healthy_gradient(PARAM_COUNT, tick),
        )?);
    }
    let after_rate = pipeline.plasticity_state.current_reinit_rate;
    let counter_after = counters.snapshot().dormant_units_reinit_total;
    drop(pipeline);
    drop(storage);
    let reopened = HealRocksStore::open(case_root.join("rocks"))?;
    let third = read_history_row(reopened.as_ref(), 3)?;
    let passed = rows.len() == 3
        && third.consecutive_regression_ticks == 3
        && third
            .alert_codes
            .contains(&PLASTICITY_REGRESSION_ALERT.to_string())
        && after_rate > before_rate
        && counter_after > 0
        && reopened.count_cf(CF_MEJEPA_PLASTICITY_HISTORY)? == 3;
    Ok(json!({
        "passed": passed,
        "before": {
            "reinitRate": before_rate,
            "dormantUnitsReinitCounter": 0
        },
        "trigger": "160/1000 units dormant for three consecutive ticks",
        "after": {
            "reinitRate": after_rate,
            "dormantUnitsReinitCounter": counter_after,
            "historyRows": reopened.count_cf(CF_MEJEPA_PLASTICITY_HISTORY)?,
            "thirdTickRow": third
        },
        "expected": {
            "alert": PLASTICITY_REGRESSION_ALERT,
            "reinitRateIncreased": true
        }
    }))
}

fn all_dormant_collapse(run_root: &Path) -> Result<Value> {
    let case_root = run_root.join("all-dormant-collapse");
    let (mut pipeline, storage, counters) = pipeline_for_case(&case_root, PARAM_COUNT)?;
    pipeline.status.lock().unwrap().observation_counter = 0;
    push_activation_row(&mut pipeline, vec![0.0; PARAM_COUNT]);
    let before_status = pipeline.status.lock().unwrap().status_change;
    let before_counter = counters.snapshot().dormant_units_reinit_total;
    let err = tick_predictor_plasticity(&mut pipeline, &repeating_pattern_gradient(PARAM_COUNT))
        .expect_err("all-dormant predictor must collapse fail-closed");
    let after_status = pipeline.status.lock().unwrap().status_change;
    let after_counter = counters.snapshot().dormant_units_reinit_total;
    drop(pipeline);
    drop(storage);
    let reopened = HealRocksStore::open(case_root.join("rocks"))?;
    let row = read_history_row(reopened.as_ref(), 1)?;
    let passed = err.code() == PLASTICITY_COLLAPSE_ALERT
        && before_status == StatusChange::Active
        && after_status == StatusChange::Paused
        && before_counter == after_counter
        && row.online_update_paused
        && row
            .alert_codes
            .contains(&PLASTICITY_COLLAPSE_ALERT.to_string())
        && row.dormant_unit_count == PARAM_COUNT;
    Ok(json!({
        "passed": passed,
        "before": {
            "status": format!("{:?}", before_status),
            "dormantUnitsReinitCounter": before_counter
        },
        "trigger": "all 1000 activation units near zero",
        "after": {
            "errorCode": err.code(),
            "status": format!("{:?}", after_status),
            "dormantUnitsReinitCounter": after_counter,
            "historyRow": row
        }
    }))
}

fn ewc_precedence_blocks_reinit(run_root: &Path) -> Result<Value> {
    let case_root = run_root.join("ewc-precedence");
    let (mut pipeline, storage, counters) = pipeline_for_case(&case_root, PARAM_COUNT)?;
    let mut fisher = vec![0.0f32; PARAM_COUNT];
    fisher[0] = 1.0;
    pipeline.ewc.task_snapshots.push(TaskSnapshot::try_new(
        pipeline.predictor.weights.clone(),
        fisher,
        1,
        [0x66; 32],
        chrono::Utc::now().timestamp(),
    )?);
    let protected_before = pipeline.predictor.weights[0];
    for tick in 0..8 {
        pipeline.status.lock().unwrap().observation_counter = tick as u64;
        let mut row = vec![0.2; PARAM_COUNT];
        row[0] = 0.0;
        push_activation_row(&mut pipeline, row);
        let _ = tick_predictor_plasticity(&mut pipeline, &repeating_pattern_gradient(PARAM_COUNT))?;
    }
    let protected_after = pipeline.predictor.weights[0];
    let counter_after = counters.snapshot().dormant_units_reinit_total;
    drop(pipeline);
    drop(storage);
    let reopened = HealRocksStore::open(case_root.join("rocks"))?;
    let row = read_history_row(reopened.as_ref(), 8)?;
    let passed = row.ewc_blocked_reinit_budget > 0
        && row
            .alert_codes
            .contains(&EWC_BLOCKED_REINIT_BUDGET_ALERT.to_string())
        && protected_before == protected_after
        && !row.reinitialized_units.contains(&0)
        && counter_after == row.reinit_count as u64;
    Ok(json!({
        "passed": passed,
        "before": {
            "protectedUnit0Weight": protected_before,
            "dormantUnitsReinitCounter": 0
        },
        "trigger": "unit 0 lowest utility but protected by EWC Fisher snapshot",
        "after": {
            "protectedUnit0Weight": protected_after,
            "dormantUnitsReinitCounter": counter_after,
            "historyRow": row
        },
        "expected": {
            "protectedUnitUnchanged": true,
            "ewcBlockedReinitBudget": "> 0",
            "alert": EWC_BLOCKED_REINIT_BUDGET_ALERT
        }
    }))
}

fn healthy_predictor_noop(run_root: &Path) -> Result<Value> {
    let case_root = run_root.join("healthy-noop");
    let (mut pipeline, storage, counters) = pipeline_for_case(&case_root, PARAM_COUNT)?;
    let before_weights = pipeline.predictor.weights.clone();
    for tick in 0..8 {
        pipeline.status.lock().unwrap().observation_counter = tick as u64;
        push_activation_row(&mut pipeline, vec![0.2; PARAM_COUNT]);
        let _ = tick_predictor_plasticity(&mut pipeline, &healthy_gradient(PARAM_COUNT, tick))?;
    }
    let after_weights = pipeline.predictor.weights.clone();
    let counter_after = counters.snapshot().dormant_units_reinit_total;
    drop(pipeline);
    drop(storage);
    let reopened = HealRocksStore::open(case_root.join("rocks"))?;
    let row = read_history_row(reopened.as_ref(), 8)?;
    let passed = row.dormancy_fraction == 0.0
        && row.reinit_count == 0
        && counter_after == 0
        && before_weights == after_weights;
    Ok(json!({
        "passed": passed,
        "before": {
            "weightsSha": hex::encode(Sha256::digest(bytemuck_bytes(&before_weights))),
            "dormantUnitsReinitCounter": 0
        },
        "trigger": "healthy activations and full-rank gradient variation",
        "after": {
            "weightsSha": hex::encode(Sha256::digest(bytemuck_bytes(&after_weights))),
            "dormantUnitsReinitCounter": counter_after,
            "historyRow": row
        }
    }))
}

fn invalid_gradient_fails_closed(run_root: &Path) -> Result<Value> {
    let case_root = run_root.join("invalid-gradient");
    let (mut pipeline, storage, _counters) = pipeline_for_case(&case_root, PARAM_COUNT)?;
    push_activation_row(&mut pipeline, vec![0.2; PARAM_COUNT]);
    let before_count = storage.count_cf(CF_MEJEPA_PLASTICITY_HISTORY)?;
    let err = tick_predictor_plasticity(&mut pipeline, &[0.1; PARAM_COUNT - 1])
        .expect_err("invalid gradient length must fail closed");
    let after_count = storage.count_cf(CF_MEJEPA_PLASTICITY_HISTORY)?;
    let passed = err.code() == "MEJEPA_HEAL_INVALID_STATE" && before_count == 0 && after_count == 0;
    Ok(json!({
        "passed": passed,
        "before": {
            "plasticityHistoryRows": before_count
        },
        "trigger": "gradient length 999 for 1000-parameter predictor",
        "after": {
            "errorCode": err.code(),
            "plasticityHistoryRows": after_count
        }
    }))
}

fn pipeline_for_case(
    case_root: &Path,
    param_count: usize,
) -> Result<(
    SelfHealingPipeline,
    Arc<HealRocksStore>,
    Arc<SystemCostCounters>,
)> {
    fs::create_dir_all(case_root).with_context(|| format!("create {}", case_root.display()))?;
    let counters = Arc::new(SystemCostCounters::new());
    let storage = HealRocksStore::open_with_system_cost_counters(
        case_root.join("rocks"),
        Arc::clone(&counters),
    )?;
    let witness_path = case_root.join("witness-chain.bin");
    let witness = WitnessChainAppender::new(witness_path.clone())?;
    let checker = ChainIntegrityChecker::try_new(witness_path)?;
    let pipeline = SelfHealingPipeline::new(
        LinearHealPredictor::try_new(param_count)?,
        storage.clone(),
        witness,
        checker,
        MeJepaHealConfig {
            skip_below_signal_clarity: false,
            ..MeJepaHealConfig::default()
        },
    )?;
    Ok((pipeline, storage, counters))
}

fn baseline_repeating_pattern_dormancy() -> Value {
    let mut last = 0.0f32;
    let mut monotonic = true;
    let mut checkpoints = Vec::new();
    for tick in (0..=ITERATIONS).step_by(5_000) {
        let dormant = natural_dormant_limit(tick) as f32 / PARAM_COUNT as f32;
        if dormant < last {
            monotonic = false;
        }
        last = dormant;
        checkpoints.push(json!({
            "tick": tick,
            "dormancyFraction": dormant
        }));
    }
    json!({
        "monotonic": monotonic,
        "finalDormancy": last,
        "checkpoints": checkpoints
    })
}

fn repeating_pattern_activation_row(tick: usize, active_until: &[usize]) -> Vec<f32> {
    let natural_dormant = natural_dormant_limit(tick);
    (0..PARAM_COUNT)
        .map(|unit| {
            if unit < natural_dormant && active_until[unit] <= tick {
                0.0
            } else {
                0.2
            }
        })
        .collect()
}

fn natural_dormant_limit(tick: usize) -> usize {
    (tick / 60).min(700)
}

fn push_activation_row(pipeline: &mut SelfHealingPipeline, row: Vec<f32>) {
    pipeline.dormant_activation_window.push(row);
    let max_rows = pipeline.config.train_cert_window_steps.max(1);
    if pipeline.dormant_activation_window.len() > max_rows {
        let remove_count = pipeline.dormant_activation_window.len() - max_rows;
        pipeline.dormant_activation_window.drain(0..remove_count);
    }
}

fn repeating_pattern_gradient(parameter_count: usize) -> Vec<f32> {
    (0..parameter_count)
        .map(|idx| if idx % 2 == 0 { 0.01 } else { 0.0 })
        .collect()
}

fn healthy_gradient(parameter_count: usize, tick: usize) -> Vec<f32> {
    (0..parameter_count)
        .map(|idx| ((idx + 1 + tick) as f32 * 0.017).sin() * 0.03)
        .collect()
}

fn read_history_row(storage: &HealRocksStore, tick: u64) -> Result<PlasticityHistoryRow> {
    let bytes = storage
        .get_cf(
            CF_MEJEPA_PLASTICITY_HISTORY,
            &encode_plasticity_history_key(tick),
        )?
        .with_context(|| format!("missing plasticity history row {tick}"))?;
    Ok(decode_value::<PlasticityHistoryRow>(&bytes)?)
}

fn bytemuck_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}
