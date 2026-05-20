use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, ensure, Context, Result};
use context_graph_mejepa::heal::{
    apply_promotion_approval, approved_promotions, encode_value, list_operator_alerts,
    pending_promotions, read_status_snapshot, DriftSample, DriftStore, HealRocksStore,
    PendingPromotionKind, PromotionApprovalAction, PromotionApprovalRequest,
    PromotionApprovalState, SchedulerState, SelfOptimConfig, TickName, CF_MEJEPA_HEAL_REPORTS,
    CF_MEJEPA_MODEL_PROMOTIONS,
};
use context_graph_mejepa::{OracleOutcome, TrainCertSummary};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-TEST-004";
const COVERED_SAMPLES: u64 = 720;
const UNCOVERED_SAMPLES: u64 = 280;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CatastrophicEventFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: CatastrophicSourceOfTruth,
    pub happy_path: Vec<StepEvidence>,
    pub boundary_cases: Vec<BoundaryEvidence>,
    pub cf_counts_before: BTreeMap<String, u64>,
    pub cf_counts_after: BTreeMap<String, u64>,
    pub physical_artifacts: Vec<FileArtifact>,
    pub readback_equal: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CatastrophicSourceOfTruth {
    pub rocksdb_path: String,
    pub status_path: String,
    pub witness_chain_path: String,
    pub drift_window_cf: String,
    pub promotion_cf: String,
    pub heal_report_cf: String,
    pub report_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StepEvidence {
    pub step: String,
    pub trigger: String,
    pub source_of_truth: String,
    pub before: Value,
    pub after: Value,
    pub expected: Value,
    pub actual: Value,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoundaryEvidence {
    pub case: String,
    pub trigger: String,
    pub expected: Value,
    pub actual: Value,
    pub before: Value,
    pub after: Value,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileArtifact {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone)]
struct Harness {
    fsv_root: PathBuf,
    db_path: PathBuf,
    status_path: PathBuf,
    witness_chain_path: PathBuf,
    archive_root: PathBuf,
    test_outcome_root: PathBuf,
}

pub fn default_fsv_root() -> PathBuf {
    PathBuf::from("/zfs/archive/contextgraph/fsv/phase-f-catastrophic-event-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("catastrophic_event_fsv.json")
}

pub fn run_catastrophic_event_fsv(fsv_root: &Path) -> Result<CatastrophicEventFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let harness = Harness::new(fsv_root);
    fs::create_dir_all(&harness.archive_root)?;
    fs::create_dir_all(&harness.test_outcome_root)?;
    let report_path = default_report_path(fsv_root);
    let build_release_sha = git_head_short();

    let cf_counts_before = {
        let db = open_db(&harness.db_path)?;
        let store = HealRocksStore::from_db(db.clone())?;
        seed_train_certs(store.as_ref())?;
        flush_all_cfs(db.as_ref())?;
        count_relevant_cfs(db.as_ref())?
    };

    let (happy_path, cf_counts_after) = {
        let db = open_db(&harness.db_path)?;
        let store = HealRocksStore::from_db(db.clone())?;
        let mut state = SchedulerState::open(db.clone(), harness.config())?;
        let mut steps = Vec::new();

        let before_seed = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
        seed_drift_window(store.as_ref(), COVERED_SAMPLES, UNCOVERED_SAMPLES)?;
        let after_seed = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
        let seed_pass = after_seed["cfCounts"][context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW]
            == json!(COVERED_SAMPLES + UNCOVERED_SAMPLES);
        steps.push(StepEvidence {
            step: "seed_catastrophic_drift_window".to_string(),
            trigger: "persist 720 covered and 280 uncovered drift samples".to_string(),
            source_of_truth: context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW.to_string(),
            before: before_seed,
            after: after_seed.clone(),
            expected: json!({"driftWindowRows": 1000, "empiricalCoverage": 0.72}),
            actual: json!({
                "driftWindowRows": after_seed["cfCounts"][context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW],
                "empiricalCoverage": COVERED_SAMPLES as f64 / (COVERED_SAMPLES + UNCOVERED_SAMPLES) as f64
            }),
            pass: seed_pass,
        });

        let before_drift = after_seed;
        state.run_tick(TickName::DriftCheck)?;
        let after_drift = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
        let pending = pending_promotions(store.as_ref())?;
        let alerts = list_operator_alerts(store.as_ref())?;
        let queued = pending
            .first()
            .ok_or_else(|| anyhow!("catastrophic drift did not queue a promotion"))?;
        let drift_pass = pending.len() == 1
            && queued.state == PromotionApprovalState::Pending
            && queued.required_distinct_approvals == 2
            && matches!(
                queued.kind,
                PendingPromotionKind::CatastrophicFullRetrainRequired { .. }
            )
            && alerts.iter().any(|alert| {
                alert.severity == context_graph_mejepa::heal::DriftSeverity::Catastrophic
            });
        let promotion_id = queued.promotion_id.clone();
        steps.push(StepEvidence {
            step: "drift_tick_queues_blocked_promotion".to_string(),
            trigger: "SchedulerState::run_tick(TickName::DriftCheck)".to_string(),
            source_of_truth: format!(
                "{} + {}",
                CF_MEJEPA_MODEL_PROMOTIONS,
                context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW
            ),
            before: before_drift,
            after: after_drift.clone(),
            expected: json!({
                "pendingPromotions": 1,
                "requiredDistinctApprovals": 2,
                "catastrophicAlert": true
            }),
            actual: json!({
                "pendingPromotions": pending.len(),
                "requiredDistinctApprovals": queued.required_distinct_approvals,
                "catastrophicAlerts": alerts.iter().filter(|alert| alert.severity == context_graph_mejepa::heal::DriftSeverity::Catastrophic).count()
            }),
            pass: drift_pass,
        });

        let before_first_approval =
            physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
        let first_approval = apply_promotion_approval(
            store.as_ref(),
            PromotionApprovalRequest {
                promotion_id: promotion_id.clone(),
                operator_id: "phase-f-catastrophic-operator-a".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "Phase F catastrophic E2E first approval".to_string(),
                two_person_rule: true,
            },
        )?;
        let after_first_approval =
            physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
        let still_blocked = approved_promotions(store.as_ref())?.is_empty()
            && first_approval.state_after == PromotionApprovalState::Pending
            && first_approval.distinct_approval_count == 1;
        steps.push(StepEvidence {
            step: "single_approval_remains_blocked".to_string(),
            trigger: "apply_promotion_approval(operator A)".to_string(),
            source_of_truth: CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
            before: before_first_approval,
            after: after_first_approval,
            expected: json!({
                "stateAfter": "pending",
                "approvedPromotions": 0,
                "distinctApprovalCount": 1
            }),
            actual: json!({
                "stateAfter": first_approval.state_after,
                "approvedPromotions": approved_promotions(store.as_ref())?.len(),
                "distinctApprovalCount": first_approval.distinct_approval_count
            }),
            pass: still_blocked,
        });

        let before_second_approval =
            physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
        let second_approval = apply_promotion_approval(
            store.as_ref(),
            PromotionApprovalRequest {
                promotion_id: promotion_id.clone(),
                operator_id: "phase-f-catastrophic-operator-b".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "Phase F catastrophic E2E independent approval".to_string(),
                two_person_rule: true,
            },
        )?;
        state.run_tick(TickName::Promote)?;
        let after_promote = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
        let heal_reports_before = before_second_approval["cfCounts"][CF_MEJEPA_HEAL_REPORTS]
            .as_u64()
            .ok_or_else(|| anyhow!("MEJEPA_CATASTROPHIC_HEAL_REPORT_COUNT_MISSING_BEFORE"))?;
        let heal_reports_after = after_promote["cfCounts"][CF_MEJEPA_HEAL_REPORTS]
            .as_u64()
            .ok_or_else(|| anyhow!("MEJEPA_CATASTROPHIC_HEAL_REPORT_COUNT_MISSING_AFTER"))?;
        let active_pointers_after = after_promote["cfCounts"]
            [context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_POINTERS]
            .as_u64()
            .ok_or_else(|| anyhow!("MEJEPA_CATASTROPHIC_ACTIVE_POINTER_COUNT_MISSING"))?;
        let executed = after_promote["executedPromotions"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .any(|row| row["promotion_id"] == json!(promotion_id))
            })
            .unwrap_or(false);
        let recovery_pass = second_approval.state_after == PromotionApprovalState::Approved
            && heal_reports_after > heal_reports_before
            && active_pointers_after >= 1
            && executed
            && harness.witness_chain_path.is_file();
        steps.push(StepEvidence {
            step: "second_approval_promotes_and_recovers".to_string(),
            trigger: "apply_promotion_approval(operator B) then SchedulerState::run_tick(TickName::Promote)".to_string(),
            source_of_truth: format!(
                "{} + {} + {}",
                CF_MEJEPA_MODEL_PROMOTIONS,
                CF_MEJEPA_HEAL_REPORTS,
                context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_POINTERS
            ),
            before: before_second_approval,
            after: after_promote.clone(),
            expected: json!({
                "stateAfter": "approved",
                "healReportsIncreased": true,
                "activeWeightsPointerRowsAtLeast": 1,
                "promotionExecuted": true,
                "witnessChainExists": true
            }),
            actual: json!({
                "stateAfter": second_approval.state_after,
                "healReportsBefore": heal_reports_before,
                "healReportsAfter": heal_reports_after,
                "activePointerRows": active_pointers_after,
                "promotionExecuted": executed,
                "witnessChainExists": harness.witness_chain_path.is_file()
            }),
            pass: recovery_pass,
        });

        flush_all_cfs(db.as_ref())?;
        let counts_after = count_relevant_cfs(db.as_ref())?;
        drop(state);
        drop(store);
        drop(db);
        (steps, counts_after)
    };

    let reopened_counts = {
        let db = open_db(&harness.db_path)?;
        let store = HealRocksStore::from_db(db.clone())?;
        let counts = count_relevant_cfs(db.as_ref())?;
        let executed = executed_promotions(store.as_ref())?;
        ensure!(
            executed.len() == 1,
            "catastrophic promotion execution did not survive reopen"
        );
        ensure!(
            counts == cf_counts_after,
            "catastrophic CF counts changed after reopen"
        );
        counts
    };

    let boundary_cases = vec![
        boundary_promote_without_approval(
            &harness.fsv_root.join("boundary-promote-without-approval"),
        )?,
        boundary_duplicate_operator_rejected(
            &harness.fsv_root.join("boundary-duplicate-operator"),
        )?,
        boundary_empty_reason_rejected(&harness.fsv_root.join("boundary-empty-reason"))?,
    ];

    let all_passed =
        happy_path.iter().all(|step| step.pass) && boundary_cases.iter().all(|case| case.pass);
    ensure!(
        all_passed,
        "catastrophic event FSV contains failed evidence"
    );

    Ok(CatastrophicEventFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha,
        source_of_truth: CatastrophicSourceOfTruth {
            rocksdb_path: harness.db_path.display().to_string(),
            status_path: harness.status_path.display().to_string(),
            witness_chain_path: harness.witness_chain_path.display().to_string(),
            drift_window_cf: context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW.to_string(),
            promotion_cf: CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
            heal_report_cf: CF_MEJEPA_HEAL_REPORTS.to_string(),
            report_path: report_path.display().to_string(),
        },
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after: reopened_counts,
        physical_artifacts: collect_file_artifacts(fsv_root)?,
        readback_equal: true,
        all_passed,
    })
}

pub fn write_report_with_readback(report: &CatastrophicEventFsvReport, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback: CatastrophicEventFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        readback == *report,
        "catastrophic event FSV readback mismatch"
    );
    Ok(())
}

impl Harness {
    fn new(fsv_root: &Path) -> Self {
        Self {
            fsv_root: fsv_root.to_path_buf(),
            db_path: fsv_root.join("heal-rocksdb"),
            status_path: fsv_root.join("self_optimization_status.json"),
            witness_chain_path: fsv_root.join("witness-chain.bin"),
            archive_root: fsv_root.join("archive"),
            test_outcome_root: fsv_root.join("test-outcomes"),
        }
    }

    fn config(&self) -> SelfOptimConfig {
        SelfOptimConfig {
            status_path: self.status_path.clone(),
            hygiene_archive_root: self.archive_root.clone(),
            witness_chain_path: self.witness_chain_path.clone(),
            test_outcome_root: self.test_outcome_root.clone(),
            ..SelfOptimConfig::default().with_all_periods(Duration::from_secs(60))
        }
    }
}

fn boundary_promote_without_approval(root: &Path) -> Result<BoundaryEvidence> {
    let harness = Harness::new(root);
    fs::create_dir_all(root)?;
    fs::create_dir_all(&harness.archive_root)?;
    fs::create_dir_all(&harness.test_outcome_root)?;
    let db = open_db(&harness.db_path)?;
    let store = HealRocksStore::from_db(db.clone())?;
    seed_train_certs(store.as_ref())?;
    seed_drift_window(store.as_ref(), COVERED_SAMPLES, UNCOVERED_SAMPLES)?;
    let mut state = SchedulerState::open(db.clone(), harness.config())?;
    state.run_tick(TickName::DriftCheck)?;
    let before = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
    state.run_tick(TickName::Promote)?;
    let after = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
    let pass = before["cfCounts"][CF_MEJEPA_HEAL_REPORTS]
        == after["cfCounts"][CF_MEJEPA_HEAL_REPORTS]
        && !pending_promotions(store.as_ref())?.is_empty()
        && approved_promotions(store.as_ref())?.is_empty();
    Ok(BoundaryEvidence {
        case: "promote_tick_without_approval_does_not_execute".to_string(),
        trigger: "run promote tick with only a pending catastrophic promotion".to_string(),
        expected: json!({
            "healReportCountUnchanged": true,
            "pendingPromotionStillPresent": true,
            "approvedPromotionAbsent": true
        }),
        actual: json!({
            "healReportsBefore": before["cfCounts"][CF_MEJEPA_HEAL_REPORTS],
            "healReportsAfter": after["cfCounts"][CF_MEJEPA_HEAL_REPORTS],
            "pendingPromotions": pending_promotions(store.as_ref())?.len(),
            "approvedPromotions": approved_promotions(store.as_ref())?.len()
        }),
        before,
        after,
        pass,
    })
}

fn boundary_duplicate_operator_rejected(root: &Path) -> Result<BoundaryEvidence> {
    let harness = Harness::new(root);
    fs::create_dir_all(root)?;
    let db = open_db(&harness.db_path)?;
    let store = HealRocksStore::from_db(db.clone())?;
    let promotion_id = queue_boundary_promotion(store.as_ref())?;
    apply_promotion_approval(
        store.as_ref(),
        PromotionApprovalRequest {
            promotion_id: promotion_id.clone(),
            operator_id: "phase-f-duplicate-operator".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "first approval should persist".to_string(),
            two_person_rule: true,
        },
    )?;
    let before = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
    let err = apply_promotion_approval(
        store.as_ref(),
        PromotionApprovalRequest {
            promotion_id,
            operator_id: "phase-f-duplicate-operator".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "duplicate approval should fail".to_string(),
            two_person_rule: true,
        },
    )
    .expect_err("duplicate operator approval must fail closed");
    let after = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
    let queued = pending_promotions(store.as_ref())?;
    let pass = err.code() == "MEJEPA_HEAL_INVALID_STATE"
        && before["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
            == after["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
        && queued.len() == 1
        && queued[0].approvals.len() == 1;
    Ok(BoundaryEvidence {
        case: "duplicate_operator_approval_rejected".to_string(),
        trigger: "same operator approves one pending catastrophic promotion twice".to_string(),
        expected: json!({
            "errorCode": "MEJEPA_HEAL_INVALID_STATE",
            "promotionRowsUnchanged": true,
            "approvalCount": 1
        }),
        actual: json!({
            "errorCode": err.code(),
            "promotionRowsBefore": before["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS],
            "promotionRowsAfter": after["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS],
            "approvalCount": queued
                .first()
                .ok_or_else(|| anyhow!("MEJEPA_CATASTROPHIC_DUPLICATE_BOUNDARY_PROMOTION_MISSING"))?
                .approvals
                .len()
        }),
        before,
        after,
        pass,
    })
}

fn boundary_empty_reason_rejected(root: &Path) -> Result<BoundaryEvidence> {
    let harness = Harness::new(root);
    fs::create_dir_all(root)?;
    let db = open_db(&harness.db_path)?;
    let store = HealRocksStore::from_db(db.clone())?;
    let promotion_id = queue_boundary_promotion(store.as_ref())?;
    let before = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
    let err = apply_promotion_approval(
        store.as_ref(),
        PromotionApprovalRequest {
            promotion_id,
            operator_id: "phase-f-empty-reason-operator".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "   ".to_string(),
            two_person_rule: true,
        },
    )
    .expect_err("empty approval reason must fail closed");
    let after = physical_state(db.as_ref(), store.as_ref(), &harness.status_path)?;
    let queued = pending_promotions(store.as_ref())?;
    let pass = err.code() == "MEJEPA_HEAL_INVALID_STATE"
        && before["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
            == after["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS]
        && queued.len() == 1
        && queued[0].approvals.is_empty();
    Ok(BoundaryEvidence {
        case: "empty_approval_reason_rejected".to_string(),
        trigger: "operator approval request with blank reason".to_string(),
        expected: json!({
            "errorCode": "MEJEPA_HEAL_INVALID_STATE",
            "promotionRowsUnchanged": true,
            "approvalCount": 0
        }),
        actual: json!({
            "errorCode": err.code(),
            "promotionRowsBefore": before["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS],
            "promotionRowsAfter": after["cfCounts"][CF_MEJEPA_MODEL_PROMOTIONS],
            "approvalCount": queued
                .first()
                .ok_or_else(|| anyhow!("MEJEPA_CATASTROPHIC_EMPTY_REASON_BOUNDARY_PROMOTION_MISSING"))?
                .approvals
                .len()
        }),
        before,
        after,
        pass,
    })
}

fn queue_boundary_promotion(store: &HealRocksStore) -> Result<String> {
    Ok(context_graph_mejepa::heal::queue_pending_retrain_request(
        store,
        PendingPromotionKind::CatastrophicFullRetrainRequired {
            cell_key: "global_or_per_cell".to_string(),
            metric_value: catastrophic_empirical_coverage(),
        },
        context_graph_mejepa::heal::TriggerReason::DriftCatastrophic,
        "Phase F catastrophic boundary promotion",
    )?)
}

fn catastrophic_empirical_coverage() -> f32 {
    COVERED_SAMPLES as f32 / (COVERED_SAMPLES + UNCOVERED_SAMPLES) as f32
}

fn seed_train_certs(store: &HealRocksStore) -> Result<()> {
    for step in 0..4u64 {
        let summary = TrainCertSummary {
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
    Ok(())
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
    flush_all_cfs(db)?;
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
        "executedPromotions": executed_promotions(store)?,
        "operatorAlerts": list_operator_alerts(store)?,
    }))
}

fn executed_promotions(
    store: &HealRocksStore,
) -> Result<Vec<context_graph_mejepa::heal::PendingPromotion>> {
    Ok(context_graph_mejepa::heal::scan_policy_records::<
        context_graph_mejepa::heal::PendingPromotion,
    >(store, b"phase_e/pending-promotion/")?
    .into_iter()
    .map(|(_key, value)| value)
    .filter(|value| value.state == PromotionApprovalState::Executed)
    .collect())
}

fn count_relevant_cfs(db: &DB) -> Result<BTreeMap<String, u64>> {
    let mut out = BTreeMap::new();
    for cf_name in [
        context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW,
        CF_MEJEPA_MODEL_PROMOTIONS,
        CF_MEJEPA_HEAL_REPORTS,
        context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_POINTERS,
        context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS,
        context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
    ] {
        out.insert(cf_name.to_string(), cf_count(db, cf_name)?);
    }
    Ok(out)
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

fn flush_all_cfs(db: &DB) -> Result<()> {
    for cf_name in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
        let cf = db
            .cf_handle(cf_name)
            .with_context(|| format!("missing column family {cf_name}"))?;
        db.flush_cf(cf)
            .with_context(|| format!("flush column family {cf_name}"))?;
    }
    Ok(())
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut opts = OpenOptions::new();
    opts.create(true).write(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut file = opts.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let readback = fs::read(path)?;
    ensure!(
        readback == bytes,
        "file readback mismatch for {}",
        path.display()
    );
    Ok(())
}

fn remove_existing_fsv_root(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let temp_dir = std::env::temp_dir();
    let allowed = path.starts_with("/zfs/archive/contextgraph/fsv/") || path.starts_with(&temp_dir);
    ensure!(
        allowed,
        "refusing to remove existing catastrophic-event FSV root outside /zfs/archive/contextgraph/fsv or temp dir: {}",
        path.display()
    );
    let mut last_err = None;
    for attempt in 0..5 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(Duration::from_millis(50 * (attempt + 1) as u64));
            }
        }
    }
    if !path.exists() {
        return Ok(());
    }
    Err(last_err.expect("remove_dir_all attempted at least once"))
        .with_context(|| format!("remove {}", path.display()))
}

fn collect_file_artifacts(root: &Path) -> Result<Vec<FileArtifact>> {
    let mut out = Vec::new();
    collect_file_artifacts_inner(root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_file_artifacts_inner(path: &Path, out: &mut Vec<FileArtifact>) -> Result<()> {
    if path.is_file() {
        let bytes = fs::read(path)?;
        out.push(FileArtifact {
            path: path.display().to_string(),
            bytes: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
        return Ok(());
    }
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            collect_file_artifacts_inner(&entry?.path(), out)?;
        }
    }
    Ok(())
}

fn git_head_short() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_test_004_catastrophic_event_fsv_passes() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_catastrophic_event_fsv(&fsv_root).expect("catastrophic event FSV report");
        write_report_with_readback(&report, &report_path)
            .expect("write catastrophic event FSV report");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert!(report.happy_path.len() >= 4);
        assert!(report.boundary_cases.len() >= 3);
        assert!(report_path.is_file());
    }
}
