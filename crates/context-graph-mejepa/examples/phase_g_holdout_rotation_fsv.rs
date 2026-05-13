use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{ensure, Context, Result};
use clap::Parser;
use context_graph_mejepa::heal::{
    is_holdout_rotation_event_key, write_json_atomic_readback, AbcPromoter, HealRocksStore,
    HoldoutDataset, HoldoutExample, HoldoutRotationEvent, HoldoutRotationPolicy, PromotionGate,
    PromotionLockState, PromotionSplitState, RetrainPromoteRequest, TriggerReason,
    WitnessChainAppender, CF_MEJEPA_HEAL_REPORTS,
};
use context_graph_mejepa::OracleOutcome;
use rocksdb::IteratorMode;
use serde_json::{json, Value};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        default_value = "/mnt/d/contextgraph/fsv/phase-g-holdout-rotation-fsv"
    )]
    output_root: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report = run(&args.output_root)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run(output_root: &Path) -> Result<Value> {
    if output_root.exists() {
        fs::remove_dir_all(output_root)
            .with_context(|| format!("remove {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root).with_context(|| format!("create {}", output_root.display()))?;

    let report_path = output_root.join("holdout_rotation_fsv.json");
    let storage = HealRocksStore::open(output_root.join("rocksdb"))?;
    let lock = Arc::new(Mutex::new(PromotionLockState::default()));
    let mut witness_chain = WitnessChainAppender::new(output_root.join("witness-chain.bin"))?;
    let mut promoter = AbcPromoter::try_new(0.1, PromotionGate::default())?;
    promoter.rotation_policy = HoldoutRotationPolicy::try_new(4, 0.10, 0x5117_2026)?;
    promoter.set_split_state(PromotionSplitState::try_new(
        (0..40).map(|idx| format!("train-{idx:03}")).collect(),
        (0..10).map(|idx| format!("holdout-{idx:03}")).collect(),
    )?);
    let split_before = promoter
        .split_state()
        .cloned()
        .context("missing split state")?;
    let current_weights = vec![0.10; 16];

    let mut promotion_results = Vec::new();
    for attempt in 0..5 {
        let result = promoter.retrain_and_promote(RetrainPromoteRequest {
            trigger_reason: TriggerReason::DriftHard,
            current_weights: &current_weights,
            storage: storage.clone(),
            witness_chain: &mut witness_chain,
            holdout: holdout_with_base_coverage(72, 100)?,
            lock: lock.clone(),
            calibration_version: "phase-g-holdout-rotation-calibration",
        });
        ensure!(
            result.is_ok(),
            "promotion attempt {attempt} failed: {result:?}"
        );
        promotion_results.push(json!({
            "attempt": attempt + 1,
            "promoted": true,
            "rotation_in_snapshot": promoter
                .fsv_snapshot()
                .and_then(|snapshot| snapshot.holdout_rotation.map(|event| event.rotation_index)),
        }));
    }

    let split_after = promoter
        .split_state()
        .cloned()
        .context("missing final split state")?;
    let rotation_events = rotation_events(&storage)?;
    ensure!(
        rotation_events.len() == 1,
        "expected exactly one rotation event"
    );
    let event = &rotation_events[0];
    ensure!(
        event.train_count_before == 40,
        "unexpected train_count_before"
    );
    ensure!(
        event.holdout_count_before == 10,
        "unexpected holdout_count_before"
    );
    ensure!(
        event.train_to_holdout.len() == 4,
        "10% of train did not rotate"
    );
    ensure!(
        event.holdout_to_train.len() == 1,
        "10% of holdout did not rotate"
    );
    ensure!(
        split_after.train_task_ids == event.train_task_ids_after,
        "final train split does not match persisted event"
    );
    ensure!(
        split_after.holdout_task_ids == event.holdout_task_ids_after,
        "final holdout split does not match persisted event"
    );

    let assertions = json!({
        "five_promotions_succeeded": promotion_results.len() == 5,
        "rotation_fired_once": rotation_events.len() == 1,
        "rotation_fired_on_fourth_promotion": promotion_results[3]["rotation_in_snapshot"] == 1,
        "fifth_promotion_did_not_rotate_again": promotion_results[4]["rotation_in_snapshot"].is_null(),
        "train_rotated_10pct": event.train_to_holdout.len() == 4,
        "holdout_rotated_10pct": event.holdout_to_train.len() == 1,
        "new_split_readback_matches_memory": split_after.train_task_ids == event.train_task_ids_after
            && split_after.holdout_task_ids == event.holdout_task_ids_after,
        "rotation_persisted_to_heal_reports": event.source_of_truth_cf == CF_MEJEPA_HEAL_REPORTS,
    });
    ensure!(
        assertions
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.as_bool() == Some(true)),
        "one or more FSV assertions failed: {assertions}"
    );

    let report = json!({
        "fsv": "phase_g_holdout_rotation",
        "all_passed": true,
        "artifact_path": report_path,
        "build_release_sha": git_sha(),
        "source_of_truth_cf": CF_MEJEPA_HEAL_REPORTS,
        "rotation_policy": {
            "successful_promotion_period": promoter.rotation_policy.successful_promotion_period,
            "swap_fraction": promoter.rotation_policy.swap_fraction,
            "seed": promoter.rotation_policy.seed,
        },
        "split_before": {
            "train_count": split_before.train_task_ids.len(),
            "holdout_count": split_before.holdout_task_ids.len(),
        },
        "split_after": {
            "train_count": split_after.train_task_ids.len(),
            "holdout_count": split_after.holdout_task_ids.len(),
            "successful_promotions_since_rotation": split_after.successful_promotions_since_rotation,
            "total_successful_promotions": split_after.total_successful_promotions,
            "rotation_index": split_after.rotation_index,
        },
        "promotion_results": promotion_results,
        "rotation_events": rotation_events,
        "assertions": assertions,
    });
    write_json_atomic_readback(&report_path, &report)?;
    Ok(report)
}

fn holdout_with_base_coverage(covered: usize, total: usize) -> Result<HoldoutDataset> {
    ensure!(covered <= total, "covered count cannot exceed total");
    let examples = (0..total)
        .map(|idx| {
            let is_covered = idx < covered;
            HoldoutExample {
                predicted: vec![OracleOutcome::Pass],
                actual: if is_covered {
                    OracleOutcome::Pass
                } else {
                    OracleOutcome::Fail
                },
                ood_score: if is_covered { 0.05 } else { 0.15 },
                calibration_nonconformity_score: if is_covered { 0.05 } else { 0.95 },
            }
        })
        .collect();
    Ok(HoldoutDataset::try_new(examples, [8; 32])?)
}

fn rotation_events(storage: &HealRocksStore) -> Result<Vec<HoldoutRotationEvent>> {
    let db = storage.db();
    let cf = db
        .cf_handle(CF_MEJEPA_HEAL_REPORTS)
        .context("missing heal reports CF")?;
    let mut events = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if is_holdout_rotation_event_key(key.as_ref()) {
            events.push(bincode::deserialize::<HoldoutRotationEvent>(&value)?);
        }
    }
    Ok(events)
}

fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}
