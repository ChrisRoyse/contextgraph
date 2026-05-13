use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use clap::Parser;
use context_graph_mejepa::eval::MutationCategory;
use context_graph_mejepa::heal::{
    apply_promotion_approval, approved_promotions, encode_value, pending_promotions,
    read_status_snapshot, write_json_atomic_readback, DriftCell, DriftSample, DriftStore,
    HealRocksStore, PerCellDriftObservation, PromotionApprovalAction, PromotionApprovalRequest,
    SchedulerState, SelfOptimConfig, TickName, CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_MODEL_PROMOTIONS,
};
use context_graph_mejepa::{Language, OracleOutcome};
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
    let invalid = invalid_approval_fails_without_mutation(&output_root.join("invalid-approval"))?;

    let report = json!({
        "sourceOfTruth": {
            "kind": "rocksdb_plus_scheduler_status_file",
            "policyCf": CF_MEJEPA_MODEL_PROMOTIONS,
            "healReportCf": CF_MEJEPA_HEAL_REPORTS,
            "statusFileName": "self_optimization_status.json"
        },
        "cases": [soft, catastrophic, per_cell, invalid],
    });
    let artifact = output_root.join("phase_e_self_optimization_fsv.json");
    write_json_atomic_readback(&artifact, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&artifact)?)?;
    ensure!(readback == report, "phase E FSV artifact readback mismatch");
    Ok(json!({
        "evidencePath": artifact,
        "report": report,
    }))
}

fn soft_drift_promotes(root: &Path) -> Result<Value> {
    let db = open_seeded_db(root, "soft")?;
    let store = HealRocksStore::from_db(db.clone())?;
    seed_drift_window(store.as_ref(), 840, 160)?;
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
        "case": "soft_drift_840_of_1000",
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
