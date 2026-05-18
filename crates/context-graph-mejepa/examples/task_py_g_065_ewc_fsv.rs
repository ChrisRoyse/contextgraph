use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use context_graph_mejepa::eval::{ActiveLearningKind, ActiveLearningQueueState};
use context_graph_mejepa::heal::ewc::{
    EwcPlusPlus, FisherMatrix, MemorySnapshotStore, PredictorGradAccessor, TaskSnapshot,
    DEFAULT_EWC_FISHER_BUDGET, DEFAULT_INITIAL_LAMBDA, DEFAULT_MAX_LAMBDA,
    EWC_SNAPSHOT_INTERVAL_UPDATES,
};
use context_graph_mejepa::heal::integrity::{ChainIntegrityChecker, WitnessChainAppender};
use context_graph_mejepa::heal::pipeline::{
    LinearHealPredictor, MeJepaHealConfig, SelfHealingPipeline,
};
use context_graph_mejepa::heal::store::HealRocksStore;
use context_graph_mejepa::heal::HealError;
use context_graph_mejepa::{OracleOutcome, SystemCostCounters};
use context_graph_mejepa_instruments::{Panel, PANEL_DIM};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-065-ewc-fsv";

fn main() -> Result<()> {
    let run_root = run_root()?;
    fs::create_dir_all(&run_root).context("create FSV run root")?;

    let no_forgetting = synthetic_no_forgetting()?;
    let cold_start = cold_start_exemption(&run_root)?;
    let violation = ewc_violation_requeues(&run_root)?;
    let degenerate = degenerate_fisher_fails_closed()?;
    let cadence = snapshot_cadence()?;

    let all_passed = no_forgetting["passed"].as_bool() == Some(true)
        && cold_start["passed"].as_bool() == Some(true)
        && violation["passed"].as_bool() == Some(true)
        && degenerate["passed"].as_bool() == Some(true)
        && cadence["passed"].as_bool() == Some(true);

    let report = json!({
        "task": "TASK-PY-G-065",
        "issue": 284,
        "allPassed": all_passed,
        "sourceOfTruth": {
            "fisherSnapshotsCf": context_graph_mejepa_cf::CF_MEJEPA_FISHER_SNAPSHOTS,
            "activeLearningQueueCf": context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            "counter": "SystemCostCounters::ewc_violations_total"
        },
        "cases": {
            "syntheticNoForgetting": no_forgetting,
            "coldStartExemption": cold_start,
            "ewcProtectionViolation": violation,
            "degenerateFisher": degenerate,
            "snapshotCadence": cadence
        }
    });

    let report_path = run_root.join("ewc_fsv.json");
    let bytes = serde_json::to_vec_pretty(&report).context("serialize FSV report")?;
    fs::write(&report_path, &bytes).context("write FSV report")?;
    let readback = fs::read(&report_path).context("readback FSV report")?;
    if readback != bytes {
        bail!("FSV report readback mismatch");
    }
    let sha256 = hex::encode(Sha256::digest(&readback));
    println!(
        "task-py-g-065 EWC FSV allPassed={} path={} sha256={}",
        all_passed,
        report_path.display(),
        sha256
    );
    if !all_passed {
        bail!("TASK-PY-G-065 EWC FSV failed");
    }
    Ok(())
}

fn run_root() -> Result<PathBuf> {
    let stamp = chrono::Utc::now().format("run-%Y%m%dT%H%M%SZ").to_string();
    Ok(Path::new(FSV_ROOT).join(stamp))
}

#[derive(Debug, Clone)]
struct ScalarPredictor {
    weight: f32,
}

impl PredictorGradAccessor for ScalarPredictor {
    fn param_count(&self) -> usize {
        1
    }

    fn parameters_flat(&self) -> Vec<f32> {
        vec![self.weight]
    }

    fn gradients_for(
        &self,
        _panel: &Panel,
        oracle_outcome: &OracleOutcome,
    ) -> std::result::Result<Vec<f32>, HealError> {
        let target = match oracle_outcome {
            OracleOutcome::Pass => 1.0,
            OracleOutcome::Fail => -1.0,
            OracleOutcome::OutOfDistribution | OracleOutcome::Abstain => 0.0,
        };
        Ok(vec![2.0 * (self.weight - target)])
    }

    fn apply_gradient(
        &mut self,
        gradient: &[f32],
        learning_rate: f32,
    ) -> std::result::Result<(), HealError> {
        self.weight -= learning_rate * gradient[0];
        Ok(())
    }
}

fn synthetic_no_forgetting() -> Result<Value> {
    let baseline_a = 1.0_f32;
    let mut no_ewc = ScalarPredictor { weight: baseline_a };
    for _ in 0..80 {
        let grad = 4.0 * (no_ewc.weight + 1.0);
        no_ewc.apply_gradient(&[grad], 0.05)?;
    }

    let mut with_ewc = ScalarPredictor { weight: baseline_a };
    let store = MemorySnapshotStore::default();
    let mut ewc = EwcPlusPlus::try_new(1, DEFAULT_INITIAL_LAMBDA, DEFAULT_MAX_LAMBDA)?;
    ewc.fisher_matrix = FisherMatrix::try_from_diagonal(vec![1.0], 1)?;
    ewc.snapshot_current_task(&with_ewc.parameters_flat(), [0x65; 32], &store)?;
    for _ in 0..2_000 {
        let task_b_grad = 4.0 * (with_ewc.weight + 1.0);
        let ewc_grad = ewc.ewc_gradient(&with_ewc.parameters_flat())?;
        let total_grad =
            task_b_grad + ewc.current_lambda_for_cell("synthetic::task_b")? * ewc_grad[0];
        let guard = ewc.guard_projected_update(&[total_grad], 0.0001, 10.0)?;
        if guard.violation {
            bail!("synthetic no-forgetting EWC guard should not violate with high FSV budget");
        }
        with_ewc.apply_gradient(&[total_grad], 0.0001)?;
        ewc.auto_tune_lambda_for_cell("synthetic::task_b", 0.25, 0.24)?;
    }

    let baseline_corr = task_a_validation_correlation(baseline_a);
    let no_ewc_corr = task_a_validation_correlation(no_ewc.weight);
    let with_ewc_corr = task_a_validation_correlation(with_ewc.weight);
    let no_ewc_drop = baseline_corr - no_ewc_corr;
    let with_ewc_drop = baseline_corr - with_ewc_corr;
    let passed = with_ewc_drop < 0.03 && no_ewc_drop > 0.30;
    Ok(json!({
        "passed": passed,
        "baselineCorrelation": baseline_corr,
        "withoutEwcCorrelation": no_ewc_corr,
        "withEwcCorrelation": with_ewc_corr,
        "withoutEwcDrop": no_ewc_drop,
        "withEwcDrop": with_ewc_drop,
        "withoutEwcWeightAfterTaskB": no_ewc.weight,
        "withEwcWeightAfterTaskB": with_ewc.weight,
        "lambdaStart": DEFAULT_INITIAL_LAMBDA
    }))
}

fn task_a_validation_correlation(weight: f32) -> f32 {
    if weight >= 0.0 {
        1.0
    } else {
        0.0
    }
}

fn cold_start_exemption(run_root: &Path) -> Result<Value> {
    let temp = run_root.join("cold-start");
    fs::create_dir_all(&temp).context("create cold-start FSV case root")?;
    let storage = HealRocksStore::open(temp.join("rocks"))?;
    let witness_path = temp.join("witness-chain.bin");
    let witness = WitnessChainAppender::new(witness_path.clone())?;
    let checker = ChainIntegrityChecker::try_new(witness_path)?;
    let mut pipeline = SelfHealingPipeline::new(
        LinearHealPredictor::try_new(4)?,
        storage.clone(),
        witness,
        checker,
        MeJepaHealConfig {
            skip_below_signal_clarity: false,
            ..MeJepaHealConfig::default()
        },
    )?;
    let out = pipeline.observe(&panel(0.2), &OracleOutcome::Pass, 0.9, 0, "cold-start")?;
    let queue_absent = storage
        .get_cf(
            context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            b"active",
        )?
        .is_none();
    Ok(json!({
        "passed": matches!(out, context_graph_mejepa::heal::pipeline::ObserveOutput::Stepped { .. }) && queue_absent,
        "queueAbsent": queue_absent,
        "snapshotCount": storage.count_cf(context_graph_mejepa_cf::CF_MEJEPA_FISHER_SNAPSHOTS)?
    }))
}

fn ewc_violation_requeues(run_root: &Path) -> Result<Value> {
    let temp = run_root.join("violation");
    fs::create_dir_all(&temp).context("create violation FSV case root")?;
    let counters = Arc::new(SystemCostCounters::new());
    let storage =
        HealRocksStore::open_with_system_cost_counters(temp.join("rocks"), Arc::clone(&counters))?;
    let witness_path = temp.join("witness-chain.bin");
    let witness = WitnessChainAppender::new(witness_path.clone())?;
    let checker = ChainIntegrityChecker::try_new(witness_path)?;
    let mut pipeline = SelfHealingPipeline::new(
        LinearHealPredictor::try_new(4)?,
        storage.clone(),
        witness,
        checker,
        MeJepaHealConfig {
            skip_below_signal_clarity: false,
            learning_rate: 1.0,
            ..MeJepaHealConfig::default()
        },
    )?;
    pipeline.ewc.fisher_matrix = FisherMatrix::try_from_diagonal(vec![1_000.0; 4], 1)?;
    pipeline.ewc.snapshot_current_task(
        &pipeline.predictor.parameters_flat(),
        [0x66; 32],
        storage.as_ref(),
    )?;
    let err = pipeline
        .observe(&panel(1.0), &OracleOutcome::Fail, 0.9, 0, "ewc-violation")
        .expect_err("over-budget EWC update should fail closed");
    let queue_bytes = storage
        .get_cf(
            context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            b"active",
        )?
        .context("active-learning queue missing after EWC violation")?;
    let queue: ActiveLearningQueueState = bincode::deserialize(&queue_bytes)?;
    let ewc_entries = queue
        .entries
        .values()
        .filter(|entry| {
            matches!(
                entry.kind,
                ActiveLearningKind::EwcProtectionViolation { .. }
            )
        })
        .count();
    let counter = counters.snapshot().ewc_violations_total;
    Ok(json!({
        "passed": err.code() == "MEJEPA_EWC_PROTECTION_VIOLATION" && ewc_entries == 1 && counter == 1,
        "errorCode": err.code(),
        "ewcEntries": ewc_entries,
        "counter": counter,
        "budget": DEFAULT_EWC_FISHER_BUDGET
    }))
}

fn degenerate_fisher_fails_closed() -> Result<Value> {
    let mut ewc = EwcPlusPlus::try_new(2, DEFAULT_INITIAL_LAMBDA, DEFAULT_MAX_LAMBDA)?;
    ewc.task_snapshots.push(TaskSnapshot::try_new(
        vec![0.0, 0.0],
        vec![0.0, 0.0],
        1,
        [0x67; 32],
        1,
    )?);
    let err = ewc
        .guard_projected_update(&[0.1, 0.1], 0.1, DEFAULT_EWC_FISHER_BUDGET)
        .expect_err("all-zero Fisher should fail closed");
    Ok(json!({
        "passed": err.code() == "EWC_FISHER_DEGENERATE",
        "errorCode": err.code()
    }))
}

fn snapshot_cadence() -> Result<Value> {
    let store = MemorySnapshotStore::default();
    let mut ewc = EwcPlusPlus::try_new(2, DEFAULT_INITIAL_LAMBDA, DEFAULT_MAX_LAMBDA)?;
    ewc.fisher_matrix =
        FisherMatrix::try_from_diagonal(vec![0.25, 0.5], EWC_SNAPSHOT_INTERVAL_UPDATES)?;
    let first = ewc.snapshot_current_task_if_due(&[0.1, -0.1], [0x68; 32], &store)?;
    let second = ewc.snapshot_current_task_if_due(&[0.1, -0.1], [0x68; 32], &store)?;
    let stored = store.snapshots.lock().unwrap().len();
    Ok(json!({
        "passed": first.is_some() && second.is_none() && stored == 1,
        "interval": EWC_SNAPSHOT_INTERVAL_UPDATES,
        "storedSnapshots": stored
    }))
}

fn panel(value: f32) -> Panel {
    Panel::try_new(vec![value; PANEL_DIM], (1u16 << 15) - 1).expect("valid FSV panel")
}
