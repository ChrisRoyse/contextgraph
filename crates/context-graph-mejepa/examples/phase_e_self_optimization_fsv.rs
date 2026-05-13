use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use clap::Parser;
use context_graph_mejepa::eval::{
    ActiveLearningQueueState, ActiveLearningSummary, ConformalHealthEntry, EvalProvenance,
    EvalReport, MutationCategory, RegressionCheck, RocksDbEvalStore,
};
use context_graph_mejepa::heal::{
    apply_promotion_approval, approved_promotions, build_per_cell_drift_report,
    choose_winner_by_phase_e_score, classify_emergency_eviction, detect_dissipation, encode_value,
    evaluate_latest_report_cells, list_operator_alerts, pending_promotions,
    persist_dissipation_signal, persist_emergency_eviction_decision, ramp_lambda_for_head,
    read_lambda, read_status_snapshot, snapshot_per_head_fisher, tick_per_head_dormancy,
    write_json_atomic_readback, ChainIntegrityChecker, DissipationConfig, DissipationSignal,
    DriftCell, DriftSample, DriftStore, EmergencyEvictionAction, EmergencyEvictionDecision,
    HealRocksStore, HoldoutEval, LinearHealPredictor, MeJepaHealConfig, ModeWinner,
    PerCellDriftConfig, PerCellDriftObservation, PerCellIntervention, PromotionApprovalAction,
    PromotionApprovalRequest, SchedulerState, SelfHealingPipeline, SelfOptimConfig, TickName,
    WitnessChainAppender, CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_MODEL_PROMOTIONS,
};
use context_graph_mejepa::{
    HeadId, Language, OracleOutcome, PredictionId, SurpriseSeverity, TaskId,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::{json, Value};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv")]
    output_root: PathBuf,
}

fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();
    let args = Args::parse();
    let report = run(&args.output_root)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run(output_root: &Path) -> Result<Value> {
    if output_root.exists() {
        ensure!(
            output_root.starts_with("/mnt/d/contextgraph/fsv/"),
            "refusing to remove FSV root outside /mnt/d/contextgraph/fsv"
        );
        fs::remove_dir_all(output_root)
            .with_context(|| format!("remove {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root).with_context(|| format!("create {}", output_root.display()))?;

    let soft = soft_drift_promotes(&output_root.join("soft-drift"))?;
    let catastrophic = catastrophic_drift_requires_approval(&output_root.join("catastrophic"))?;
    let per_cell = per_cell_small_window_queues_retrain(&output_root.join("per-cell"))?;
    let per_cell_noise =
        per_cell_small_window_noise_does_not_fire(&output_root.join("per-cell-noise"))?;
    let active_learning =
        active_learning_surprise_fast_path(&output_root.join("active-learning-fast-path"))?;
    let dissipation =
        dissipation_detector_persists_interventions(&output_root.join("dissipation"))?;
    let lambda_fisher = lambda_and_fisher_readback(&output_root.join("lambda-fisher"))?;
    let dormancy = dormancy_reinitializes_only_detected_head(&output_root.join("dormancy"))?;
    let emergency = emergency_exact_threshold_persists_once(&output_root.join("emergency"))?;
    let per_cell_promotion =
        per_cell_promotion_rejects_regressing_cell(&output_root.join("per-cell-promotion"))?;
    let invalid = invalid_approval_fails_without_mutation(&output_root.join("invalid-approval"))?;

    let report = json!({
        "sourceOfTruth": {
            "kind": "rocksdb_plus_scheduler_status_file",
            "policyCf": CF_MEJEPA_MODEL_PROMOTIONS,
            "healReportCf": CF_MEJEPA_HEAL_REPORTS,
            "statusFileName": "self_optimization_status.json"
        },
        "cases": [
            soft,
            catastrophic,
            per_cell,
            per_cell_noise,
            active_learning,
            dissipation,
            lambda_fisher,
            dormancy,
            emergency,
            per_cell_promotion,
            invalid
        ],
    });
    let artifact = output_root.join("phase_e_self_optimization_fsv.json");
    write_json_atomic_readback(&artifact, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&artifact)?)?;
    let canonical_report: Value = serde_json::from_slice(&serde_json::to_vec(&report)?)?;
    ensure!(
        readback == canonical_report,
        "phase E FSV artifact readback mismatch"
    );
    Ok(json!({
        "evidencePath": artifact,
        "report": report,
    }))
}

fn soft_drift_promotes(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "soft")?;
    let store = HealRocksStore::from_db(db.clone())?;
    seed_drift_window(store.as_ref(), 860, 140)?;
    let mut state = SchedulerState::open(db.clone(), config_for(root))?;
    let before = physical_state(db.as_ref(), store.as_ref(), state.status_path())?;
    state.run_tick(TickName::DriftCheck)?;
    let after = physical_state(db.as_ref(), store.as_ref(), state.status_path())?;
    ensure!(
        after["cfCounts"][CF_MEJEPA_HEAL_REPORTS]
            .as_u64()
            .unwrap_or(0)
            > before["cfCounts"][CF_MEJEPA_HEAL_REPORTS]
                .as_u64()
                .unwrap_or(0),
        "soft drift did not persist an A/B/C heal report"
    );
    Ok(json!({
        "case": "soft_drift_860_of_1000",
        "expected": "soft drift triggers autonomous A/B/C promotion and persists CF_MEJEPA_HEAL_REPORTS",
        "before": before,
        "after": after,
    }))
}

fn catastrophic_drift_requires_approval(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "catastrophic")?;
    let store = HealRocksStore::from_db(db.clone())?;
    seed_drift_window(store.as_ref(), 750, 250)?;
    let mut state = SchedulerState::open(db.clone(), config_for(root))?;
    let before = physical_state(db.as_ref(), store.as_ref(), state.status_path())?;
    state.run_tick(TickName::DriftCheck)?;
    let queued = pending_promotions(store.as_ref())?;
    ensure!(
        queued.len() == 1,
        "catastrophic drift did not queue one approval"
    );
    let promotion_id = queued[0].promotion_id.clone();
    let first_approval = apply_promotion_approval(
        store.as_ref(),
        PromotionApprovalRequest {
            promotion_id: promotion_id.clone(),
            operator_id: "phase-e-fsv-operator-a".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "FSV catastrophic drift approval".to_string(),
            two_person_rule: true,
        },
    )?;
    ensure!(
        approved_promotions(store.as_ref())?.is_empty(),
        "single catastrophic approval should not satisfy two-person rule"
    );
    let second_approval = apply_promotion_approval(
        store.as_ref(),
        PromotionApprovalRequest {
            promotion_id: promotion_id.clone(),
            operator_id: "phase-e-fsv-operator-b".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "FSV second independent catastrophic drift approval".to_string(),
            two_person_rule: true,
        },
    )?;
    ensure!(
        !approved_promotions(store.as_ref())?.is_empty(),
        "two approvals did not persist approved state"
    );
    state.run_tick(TickName::Promote)?;
    let after = physical_state(db.as_ref(), store.as_ref(), state.status_path())?;
    ensure!(
        after["cfCounts"][CF_MEJEPA_HEAL_REPORTS]
            .as_u64()
            .unwrap_or(0)
            > before["cfCounts"][CF_MEJEPA_HEAL_REPORTS]
                .as_u64()
                .unwrap_or(0),
        "approved catastrophic promotion did not persist heal report"
    );
    Ok(json!({
        "case": "catastrophic_drift_750_of_1000",
        "expected": "catastrophic drift queues approval; two distinct approval readbacks enable promote tick",
        "promotionId": promotion_id,
        "firstApproval": first_approval,
        "secondApproval": second_approval,
        "before": before,
        "after": after,
    }))
}

fn per_cell_small_window_queues_retrain(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "per-cell")?;
    let store = HealRocksStore::from_db(db.clone())?;
    seed_drift_window(store.as_ref(), 700, 0)?;
    let cell = DriftCell {
        mutation_category: MutationCategory::CompileError,
        language: Language::Rust,
    };
    for idx in 0..10u64 {
        let observation = PerCellDriftObservation::try_new(
            cell,
            false,
            1,
            idx,
            1_778_000_010_000 + idx as i64,
            "phase-e-fsv-real-rocksdb",
        )?;
        context_graph_mejepa::heal::persist_per_cell_observation(store.as_ref(), &observation)?;
    }
    let mut state = SchedulerState::open(db.clone(), config_for(root))?;
    let before = physical_state(db.as_ref(), store.as_ref(), state.status_path())?;
    state.run_tick(TickName::DriftCheck)?;
    let after = physical_state(db.as_ref(), store.as_ref(), state.status_path())?;
    ensure!(
        after["pendingPromotions"].as_array().unwrap().len()
            > before["pendingPromotions"].as_array().unwrap().len(),
        "per-cell catastrophic small window did not queue retrain approval"
    );
    Ok(json!({
        "case": "per_cell_small_window_compile_error_rust",
        "expected": "Jeffreys small-window posterior queues full retrain approval",
        "before": before,
        "after": after,
    }))
}

fn per_cell_small_window_noise_does_not_fire(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "per-cell-noise")?;
    let store = HealRocksStore::from_db(db.clone())?;
    let cell = DriftCell {
        mutation_category: MutationCategory::OffByOne,
        language: Language::Python,
    };
    for idx in 0..10u64 {
        let observation = PerCellDriftObservation::try_new(
            cell,
            idx < 7,
            1,
            idx,
            1_778_000_020_000 + idx as i64,
            "phase-e-fsv-noise-real-rocksdb",
        )?;
        context_graph_mejepa::heal::persist_per_cell_observation(store.as_ref(), &observation)?;
    }
    let before = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    let report = build_per_cell_drift_report(store.as_ref(), &PerCellDriftConfig::default())?;
    let after = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    let cell_report = report
        .cells
        .get(&cell.key())
        .context("missing per-cell noise report")?;
    ensure!(
        cell_report.intervention == PerCellIntervention::NoChange,
        "7/10 small window should not queue intervention"
    );
    ensure!(
        before["pendingPromotions"] == after["pendingPromotions"],
        "noise case mutated pending promotion state"
    );
    Ok(json!({
        "case": "per_cell_small_window_7_of_10_noise",
        "expected": "Jeffreys posterior does not fire on noisy 7/10 small window",
        "cellReport": cell_report,
        "before": before,
        "after": after,
    }))
}

fn active_learning_surprise_fast_path(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "active-learning")?;
    let store = HealRocksStore::from_db(db.clone())?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    let mut queue = ActiveLearningQueueState::new(8)?;
    queue.enqueue_agent_surprise(
        TaskId("phase-e-agent-surprise".to_string()),
        PredictionId([0x22; 16]),
        SurpriseSeverity::Catastrophic,
    )?;
    eval_store.persist_queue(&queue)?;
    let mut state = SchedulerState::open(db.clone(), config_for(root))?;
    let before = physical_state(db.as_ref(), store.as_ref(), state.status_path())?;
    state.run_tick(TickName::ActiveLearning)?;
    let after = physical_state(db.as_ref(), store.as_ref(), state.status_path())?;
    ensure!(
        after["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
            .as_u64()
            .unwrap_or(0)
            > before["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
                .as_u64()
                .unwrap_or(0),
        "agent surprise fast path did not persist policy record"
    );
    let fast_path_records =
        &after["statusFile"]["lastSuccess"]["active_learning"]["details"]["fast_path_records"];
    ensure!(
        fast_path_records[0]["surprise_weight"] == json!(4),
        "catastrophic surprise did not produce 4x drift weight"
    );
    Ok(json!({
        "case": "active_learning_agent_surprise_fast_path",
        "expected": "AgentSurprise ranks first and persists a 4x fast-path reward signal",
        "before": before,
        "after": after,
    }))
}

fn dissipation_detector_persists_interventions(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "dissipation")?;
    let store = HealRocksStore::from_db(db.clone())?;
    let before = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    for idx in 0..4 {
        persist_dissipation_signal(
            store.as_ref(),
            &DissipationSignal::try_new(
                HeadId::Oracle,
                0.4,
                0.8,
                0.8 - idx as f32 * 0.1,
                0.02,
                1_778_000_030_000 + idx,
                "phase-e-fsv-dissipation-real-rocksdb",
            )?,
        )?;
    }
    let report = detect_dissipation(store.as_ref(), &DissipationConfig::default())?;
    let after = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    ensure!(
        report.global_dissipating,
        "dissipation detector did not fire"
    );
    ensure!(
        !report.queued_interventions.is_empty(),
        "dissipation detector did not queue interventions"
    );
    Ok(json!({
        "case": "dissipation_high_l_step_degrading_xi",
        "expected": "per-head dissipation fires and persists replay/EWC/LR interventions",
        "report": report,
        "before": before,
        "after": after,
    }))
}

fn lambda_and_fisher_readback(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "lambda-fisher")?;
    let store = HealRocksStore::from_db(db.clone())?;
    let before_lambda = read_lambda(store.as_ref(), HeadId::FailureMode)?;
    let lambda_decision =
        ramp_lambda_for_head(store.as_ref(), HeadId::FailureMode, 0.75, 0.30, false)?;
    let after_lambda = read_lambda(store.as_ref(), HeadId::FailureMode)?;
    ensure!(
        (after_lambda - (before_lambda + 0.05)).abs() < 1e-6,
        "lambda ramp did not advance by 0.05"
    );
    let blocked_lambda = ramp_lambda_for_head(store.as_ref(), HeadId::Accuracy, 0.99, 0.30, true)?;
    ensure!(
        !blocked_lambda.updated,
        "critical degraded head should block lambda ramp"
    );
    let before = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    let diagonal = (0..40)
        .map(|idx| 0.01 + idx as f32 * 0.001)
        .collect::<Vec<_>>();
    let fisher = snapshot_per_head_fisher(store.as_ref(), &diagonal, 10, 40)?;
    let after = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    ensure!(
        fisher.len() == HeadId::ALL.len(),
        "missing per-head Fisher snapshots"
    );
    Ok(json!({
        "case": "lambda_ramp_and_per_head_fisher",
        "expected": "lambda ramp writes readback state; Fisher snapshots persist for every head",
        "beforeLambda": before_lambda,
        "lambdaDecision": lambda_decision,
        "afterLambda": after_lambda,
        "blockedLambda": blocked_lambda,
        "fisherSnapshotCount": fisher.len(),
        "before": before,
        "after": after,
    }))
}

fn dormancy_reinitializes_only_detected_head(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "dormancy")?;
    let store = HealRocksStore::from_db(db.clone())?;
    let witness = WitnessChainAppender::new(root.join("witness-chain.bin"))?;
    let checker = ChainIntegrityChecker::try_new(root.join("witness-chain.bin"))?;
    let mut pipeline = SelfHealingPipeline::new(
        LinearHealPredictor::try_new(64)?,
        store.clone(),
        witness,
        checker,
        MeJepaHealConfig::default(),
    )?;
    pipeline.dormant_activation_window = vec![vec![0.2; 16]; 4];
    for row in &mut pipeline.dormant_activation_window {
        row[2] = 0.0;
        row[12] = 0.0;
    }
    let before_weights = pipeline.active_embedders[0].weights.clone();
    let report = tick_per_head_dormancy(&mut pipeline)?;
    let after_weights = pipeline.active_embedders[0].weights.clone();
    let failure_mode = report
        .decisions
        .get("failure_mode")
        .context("missing failure_mode dormancy decision")?;
    ensure!(
        failure_mode.detected,
        "failure_mode head dormancy was not detected"
    );
    ensure!(
        report
            .reinitialized_units_by_head
            .contains_key("failure_mode"),
        "failure_mode head was not reinitialized"
    );
    ensure!(
        before_weights[2] != after_weights[2] && before_weights[12] != after_weights[12],
        "dormant failure_mode units were not changed"
    );
    ensure!(
        before_weights[0] == after_weights[0],
        "non-dormant trunk/shared unit changed unexpectedly"
    );
    Ok(json!({
        "case": "per_head_dormancy_targeted_reinit",
        "expected": "only detected failure_mode projection units are reinitialized",
        "report": report,
        "beforeWeights": {"unit0": before_weights[0], "unit2": before_weights[2], "unit12": before_weights[12]},
        "afterWeights": {"unit0": after_weights[0], "unit2": after_weights[2], "unit12": after_weights[12]},
        "physicalState": physical_state(db.as_ref(), store.as_ref(), &root.join("self_optimization_status.json"))?,
    }))
}

fn emergency_exact_threshold_persists_once(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "emergency")?;
    let store = HealRocksStore::from_db(db.clone())?;
    let threshold = 110 * 1024 * 1024 * 1024u64;
    let before = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    let action = classify_emergency_eviction(threshold, None, threshold, None)?;
    ensure!(
        action == EmergencyEvictionAction::ThresholdReached,
        "exact threshold should fire ThresholdReached once"
    );
    let decision = EmergencyEvictionDecision {
        total_used_bytes_before: threshold,
        total_used_bytes_after: None,
        threshold_bytes: threshold,
        action,
        recorded_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        source_of_truth_cf: CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
    };
    persist_emergency_eviction_decision(store.as_ref(), &decision)?;
    let repeat = classify_emergency_eviction(threshold, None, threshold, Some(&decision))?;
    ensure!(
        repeat == EmergencyEvictionAction::SuppressedRepeat,
        "repeat exact-threshold tick should suppress until headroom changes"
    );
    let after = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    ensure!(
        after["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
            .as_u64()
            .unwrap_or(0)
            > before["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
                .as_u64()
                .unwrap_or(0),
        "emergency decision was not persisted"
    );
    Ok(json!({
        "case": "emergency_eviction_exact_110gb",
        "expected": "exact threshold fires once and repeated same usage suppresses",
        "decision": decision,
        "repeatAction": repeat,
        "before": before,
        "after": after,
    }))
}

fn per_cell_promotion_rejects_regressing_cell(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "per-cell-promotion")?;
    let store = HealRocksStore::from_db(db.clone())?;
    let mode_a = HoldoutEval::try_new(0.90, 0.90, 0.05, 10, [0x01; 32])?;
    let mode_b = HoldoutEval::try_new(0.91, 0.94, 0.05, 10, [0x02; 32])?;
    let mode_c = HoldoutEval::try_new(0.91, 0.93, 0.05, 10, [0x03; 32])?;
    let winner = choose_winner_by_phase_e_score(&mode_a, &mode_b, &mode_c, 1.0, 1.05, 1.15)?;
    ensure!(
        winner == ModeWinner::B,
        "Phase E score formula did not pick B"
    );
    let before_report = eval_report_with_cells(BTreeMap::from([
        ("compile_error::rust".to_string(), Some(0.90)),
        ("off_by_one::python".to_string(), Some(0.80)),
    ]));
    let after_report = eval_report_with_cells(BTreeMap::from([
        ("compile_error::rust".to_string(), Some(0.91)),
        ("off_by_one::python".to_string(), Some(0.75)),
    ]));
    let before = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    let decision = evaluate_latest_report_cells(
        store.as_ref(),
        &before_report,
        &after_report,
        0.0,
        winner,
        &mode_a,
        &mode_b,
        &mode_c,
    )?;
    let after = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    ensure!(
        !decision.accepted,
        "regressing per-cell promotion was accepted"
    );
    ensure!(
        decision.regressing_cells == vec!["off_by_one::python".to_string()],
        "wrong regressing cell set"
    );
    Ok(json!({
        "case": "per_cell_promotion_rejects_regression",
        "expected": "A/B/C winner by score is rejected if any category-language cell regresses",
        "decision": decision,
        "before": before,
        "after": after,
    }))
}

fn invalid_approval_fails_without_mutation(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "invalid-approval")?;
    let store = HealRocksStore::from_db(db.clone())?;
    let promotion_id = context_graph_mejepa::heal::queue_pending_retrain_request(
        store.as_ref(),
        context_graph_mejepa::heal::PendingPromotionKind::CatastrophicFullRetrainRequired {
            cell_key: "compile_error::rust".to_string(),
            metric_value: 0.50,
        },
        context_graph_mejepa::heal::TriggerReason::DriftCatastrophic,
        "FSV invalid approval edge case",
    )?;
    let before = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    let err = apply_promotion_approval(
        store.as_ref(),
        PromotionApprovalRequest {
            promotion_id,
            operator_id: "bad\noperator".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "invalid operator id should fail".to_string(),
            two_person_rule: false,
        },
    )
    .expect_err("invalid operator id must fail closed");
    let after = physical_state(
        db.as_ref(),
        store.as_ref(),
        &root.join("self_optimization_status.json"),
    )?;
    ensure!(
        before["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
            == after["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS],
        "invalid approval changed policy CF row count"
    );
    Ok(json!({
        "case": "invalid_approval_operator_id",
        "expected": "invalid operator id returns MEJEPA_HEAL_INVALID_STATE and leaves policy row count unchanged",
        "errorCode": err.code(),
        "before": before,
        "after": after,
    }))
}

fn eval_report_with_cells(per_cell_correlation: BTreeMap<String, Option<f32>>) -> EvalReport {
    EvalReport {
        report_date: "2026-05-13-phase-e-fsv".to_string(),
        generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        rolling_window_size: 8,
        holdout_count: 8,
        overall_correlation: Some(0.90),
        per_category_correlation: BTreeMap::from([(MutationCategory::CompileError, Some(0.90))]),
        per_language_correlation: BTreeMap::from([(Language::Rust, Some(0.90))]),
        per_cell_correlation,
        bayesian_shrinkage: BTreeMap::from([("compile_error::rust".to_string(), 0.10)]),
        conformal_coverage_health: BTreeMap::from([(
            Language::Rust,
            ConformalHealthEntry {
                expected_coverage: 0.90,
                empirical_coverage: 0.91,
                sample_count: 8,
                within_band: true,
            },
        )]),
        ood_calibration_health: BTreeMap::from([(Language::Rust, Some(0.90))]),
        gtau_pass_rate: BTreeMap::from([(Language::Rust, 0.98)]),
        active_learning: ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: None,
        aux_head_distillation: None,
        regression_checks: vec![RegressionCheck {
            name: "perf_latency".to_string(),
            previous: 1.0,
            current: 0.99,
            drop: 0.01,
            passed: true,
        }],
        open_research_questions: Vec::new(),
        q1_pass_rate: 0.99,
        q2_report_correlation: Some(0.90),
        q3_side_effect_agreement: Some(0.90),
        ship_gate_passed: false,
        ship_gate_failures: vec!["phase-e-fsv-regressing-cell".to_string()],
        provenance: EvalProvenance {
            corpus_sha: "phase-e-fsv-corpus".to_string(),
            eval_code_version: "phase-e-fsv".to_string(),
            calibration_version: "phase-e-fsv-calibration".to_string(),
            generated_by: "phase_e_self_optimization_fsv".to_string(),
        },
        wall_clock_seconds: 1.0,
    }
}

fn open_seeded_db(root: &Path, name: &str) -> Result<Arc<DB>> {
    fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    let db = open_db(&root.join("rocksdb"))?;
    let store = HealRocksStore::from_db(db.clone())?;
    for step in 0..4u64 {
        let summary = context_graph_mejepa::TrainCertSummary {
            step,
            delta_omega: 0.70,
            delta_xi: 0.70,
            witness_offset: step,
        };
        store.put_cf_readback(
            context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
            &step.to_be_bytes(),
            &encode_value(&summary)?,
        )?;
    }
    let _ = name;
    Ok(db)
}

fn seed_drift_window(store: &HealRocksStore, covered: u64, uncovered: u64) -> Result<()> {
    for offset in 0..covered {
        let sample = DriftSample::try_new(
            vec![OracleOutcome::Pass],
            OracleOutcome::Pass,
            offset,
            0.05,
            1.0,
        )?;
        store.persist_drift_sample(offset, &sample)?;
    }
    for idx in 0..uncovered {
        let offset = covered + idx;
        let sample = DriftSample::try_new(
            vec![OracleOutcome::Fail],
            OracleOutcome::Pass,
            offset,
            0.20,
            1.0,
        )?;
        store.persist_drift_sample(offset, &sample)?;
    }
    Ok(())
}

fn config_for(root: &Path) -> SelfOptimConfig {
    SelfOptimConfig {
        status_path: root.join("self_optimization_status.json"),
        hygiene_archive_root: root.join("archive"),
        witness_chain_path: root.join("witness-chain.bin"),
        test_outcome_root: root.join("test-outcomes"),
        ..SelfOptimConfig::default().with_all_periods(Duration::from_secs(60))
    }
}

fn open_db(path: &Path) -> Result<Arc<DB>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(&opts, path, descriptors)?))
}

fn physical_state(db: &DB, store: &HealRocksStore, status_path: &Path) -> Result<Value> {
    let mut counts = serde_json::Map::new();
    for cf_name in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
        counts.insert(cf_name.to_string(), json!(cf_count(db, cf_name)?));
    }
    let status = if status_path.exists() {
        let snapshot = read_status_snapshot(status_path)?;
        json!({
            "exists": true,
            "status": snapshot.status,
            "tickerCounts": snapshot.ticker_counts,
            "lastSuccess": snapshot.last_success,
            "lastError": snapshot.last_error,
        })
    } else {
        json!({"exists": false})
    };
    Ok(json!({
        "cfCounts": Value::Object(counts),
        "statusFile": status,
        "pendingPromotions": pending_promotions(store)?,
        "approvedPromotions": approved_promotions(store)?,
        "operatorAlerts": list_operator_alerts(store)?,
    }))
}

fn cf_count(db: &DB, cf_name: &str) -> Result<u64> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing column family {cf_name}"))?;
    let mut count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}
