//! Phase G cell-status FSV seed harness.
//!
//! Writes schema-valid `EvalReport` rows into real inference RocksDB stores so
//! the shell FSV can exercise the public `mejepa cell-status` CLI against
//! `CF_MEJEPA_EVAL_REPORTS`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use context_graph_mejepa::eval::{
    cell_key, ActiveLearningSummary, EvalProvenance, EvalReport, MutationCategory,
    OpenResearchQuestionStatus,
};
use context_graph_mejepa::{count_cf, open_infer_rocksdb, Language, RocksDbEvalStore};
use context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS;
use serde_json::{json, Value};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-cell-status-fsv";
const THRESHOLD: f32 = 0.95;
const PASS_CORRELATION: f32 = 0.97;
const BELOW_CORRELATION: f32 = 0.91;

#[derive(Debug, Clone, Copy)]
enum Scenario {
    Pass,
    BelowThreshold,
    MissingCell,
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::BelowThreshold => "below-threshold",
            Self::MissingCell => "missing-cell",
        }
    }

    fn report_date(self) -> &'static str {
        match self {
            Self::Pass => "2026-05-13-phase-g-cell-status-pass",
            Self::BelowThreshold => "2026-05-13-phase-g-cell-status-below-threshold",
            Self::MissingCell => "2026-05-13-phase-g-cell-status-missing-cell",
        }
    }

    fn expected_exit_code(self) -> i32 {
        match self {
            Self::Pass => 0,
            Self::BelowThreshold | Self::MissingCell => 1,
        }
    }
}

fn main() -> Result<()> {
    let fsv_root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FSV_ROOT));
    fs::create_dir_all(&fsv_root).context("create FSV root")?;

    let scenarios = [
        Scenario::Pass,
        Scenario::BelowThreshold,
        Scenario::MissingCell,
    ]
    .into_iter()
    .map(|scenario| seed_scenario(&fsv_root, scenario))
    .collect::<Result<Vec<_>>>()?;

    let output = json!({
        "schema_version": 1,
        "fsv_name": "phase-g-cell-status-seed-fsv",
        "seeded_at_unix_ms": Utc::now().timestamp_millis(),
        "threshold": THRESHOLD,
        "required_cell_count": required_phase_g_cells().len(),
        "source_of_truth": {
            "column_family": CF_MEJEPA_EVAL_REPORTS,
            "writer": "RocksDbEvalStore::persist_report",
            "readback": "RocksDbEvalStore::load_latest_report",
            "grid": "8 mutation categories x 10 non-Python fixture languages"
        },
        "scenarios": scenarios,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn seed_scenario(fsv_root: &Path, scenario: Scenario) -> Result<Value> {
    let scenario_root = fsv_root.join(scenario.name());
    let db_path = scenario_root.join("infer-rocksdb");
    fs::create_dir_all(&scenario_root).with_context(|| {
        format!(
            "create scenario root for Phase G cell-status scenario {}",
            scenario.name()
        )
    })?;

    let report = build_report(scenario);
    let input_hash = report
        .determinism_hash()
        .map_err(|err| anyhow!("hash input eval report: {err}"))?;

    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let store =
        RocksDbEvalStore::new(db.clone()).map_err(|err| anyhow!("construct eval store: {err}"))?;
    store
        .persist_report(&report)
        .map_err(|err| anyhow!("persist eval report: {err}"))?;
    let readback = store
        .load_latest_report()
        .map_err(|err| anyhow!("load latest eval report: {err}"))?
        .ok_or_else(|| anyhow!("latest eval report missing after persist"))?;
    let readback_hash = readback
        .determinism_hash()
        .map_err(|err| anyhow!("hash readback eval report: {err}"))?;
    ensure!(
        input_hash == readback_hash,
        "readback hash mismatch for {}",
        scenario.name()
    );
    let cf_count = count_cf(&db, CF_MEJEPA_EVAL_REPORTS).context("count eval-report CF")?;
    ensure!(cf_count == 1, "expected exactly one eval report row");
    drop(store);
    drop(db);

    Ok(json!({
        "name": scenario.name(),
        "db_path": db_path,
        "report_date": report.report_date,
        "expected_exit_code": scenario.expected_exit_code(),
        "expected_required_cell_count": required_phase_g_cells().len(),
        "expected_passing_cell_count": match scenario {
            Scenario::Pass => 80,
            Scenario::BelowThreshold | Scenario::MissingCell => 79,
        },
        "expected_below_threshold_cell_count": if matches!(scenario, Scenario::BelowThreshold) { 1 } else { 0 },
        "expected_missing_cell_count": if matches!(scenario, Scenario::MissingCell) { 1 } else { 0 },
        "expected_failing_cell_count": if matches!(scenario, Scenario::Pass) { 0 } else { 1 },
        "target_cell": match scenario {
            Scenario::Pass => Value::Null,
            Scenario::BelowThreshold => json!("off_by_one::rust"),
            Scenario::MissingCell => json!("known_good::ruby"),
        },
        "determinism_hash": input_hash,
        "cf_count": cf_count,
        "readback_equal": true,
    }))
}

fn build_report(scenario: Scenario) -> EvalReport {
    let mut per_cell_correlation = BTreeMap::new();
    for (category, language, key) in required_phase_g_cells() {
        if matches!(scenario, Scenario::MissingCell)
            && category == MutationCategory::KnownGood
            && language == Language::Ruby
        {
            continue;
        }
        let correlation = if matches!(scenario, Scenario::BelowThreshold)
            && category == MutationCategory::OffByOne
            && language == Language::Rust
        {
            BELOW_CORRELATION
        } else {
            PASS_CORRELATION
        };
        per_cell_correlation.insert(key, Some(correlation));
    }

    let ship_gate_passed = matches!(scenario, Scenario::Pass);
    EvalReport {
        report_date: scenario.report_date().to_string(),
        generated_at_unix_ms: Utc::now().timestamp_millis(),
        rolling_window_size: 800,
        holdout_count: 800,
        overall_correlation: Some(if ship_gate_passed {
            PASS_CORRELATION
        } else {
            BELOW_CORRELATION
        }),
        per_category_correlation: MutationCategory::all()
            .into_iter()
            .map(|category| (category, Some(PASS_CORRELATION)))
            .collect(),
        per_language_correlation: phase_g_languages()
            .into_iter()
            .map(|language| (language, Some(PASS_CORRELATION)))
            .collect(),
        per_cell_correlation,
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::new(),
        conformal_coverage_health: BTreeMap::new(),
        ood_calibration_health: BTreeMap::new(),
        gtau_pass_rate: BTreeMap::new(),
        per_prediction_class_calibration: BTreeMap::new(),
        active_learning: ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: None,
        per_cell_state_transfer: BTreeMap::new(),
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: vec![OpenResearchQuestionStatus {
            id: "TASK-G-002".to_string(),
            question: "Does the per-cell convergence driver bind to eval-report source of truth?"
                .to_string(),
            status: "closed_by_fsv".to_string(),
        }],
        q1_pass_rate: 0.97,
        q2_report_correlation: Some(if ship_gate_passed {
            PASS_CORRELATION
        } else {
            BELOW_CORRELATION
        }),
        q3_side_effect_agreement: Some(0.97),
        ship_gate_passed,
        ship_gate_failures: if ship_gate_passed {
            Vec::new()
        } else {
            vec![format!("{} scenario intentionally fails", scenario.name())]
        },
        provenance: EvalProvenance {
            corpus_sha: "phase-g-cell-status-fsv".to_string(),
            eval_code_version: "task-g-002-fsv".to_string(),
            calibration_version: "task-g-002-fsv".to_string(),
            generated_by: "phase_g_cell_status_seed_fsv".to_string(),
        },
        wall_clock_seconds: 1.0,
    }
}

fn required_phase_g_cells() -> Vec<(MutationCategory, Language, String)> {
    let mut cells = Vec::with_capacity(MutationCategory::all().len() * phase_g_languages().len());
    for category in MutationCategory::all() {
        for language in phase_g_languages() {
            cells.push((category, language, cell_key(category, language)));
        }
    }
    cells
}

fn phase_g_languages() -> [Language; 10] {
    [
        Language::Rust,
        Language::Javascript,
        Language::Typescript,
        Language::Go,
        Language::Java,
        Language::C,
        Language::Cpp,
        Language::CSharp,
        Language::Ruby,
        Language::Php,
    ]
}
