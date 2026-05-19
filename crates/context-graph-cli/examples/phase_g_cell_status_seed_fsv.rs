//! Active Python ship-gate cell-status FSV seed harness.
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
    cell_key, required_active_python_ship_gate_cells, ActiveLearningSummary, EvalProvenance,
    EvalReport, MutationCategory, OpenResearchQuestionStatus, ACTIVE_PYTHON_SHIP_GATE_GRID,
    ACTIVE_PYTHON_SHIP_GATE_NAME,
};
use context_graph_mejepa::{count_cf, open_infer_rocksdb, Language, RocksDbEvalStore};
use context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS;
use serde_json::{json, Value};

const DEFAULT_FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/phase-g-cell-status-fsv";
const THRESHOLD: f32 = 0.95;
const PASS_CORRELATION: f32 = 0.97;
const BELOW_CORRELATION: f32 = 0.91;

#[derive(Debug, Clone, Copy)]
enum Scenario {
    Pass,
    BelowThreshold,
    MissingCorrelation,
    Stale80CellGrid,
    OverLimitCell,
    EmptyDb,
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::BelowThreshold => "below-threshold",
            Self::MissingCorrelation => "missing-correlation",
            Self::Stale80CellGrid => "stale-80-cell-grid",
            Self::OverLimitCell => "over-limit-cell",
            Self::EmptyDb => "empty-db",
        }
    }

    fn report_date(self) -> Option<&'static str> {
        match self {
            Self::Pass => Some("2026-05-13-active-python-cell-status-pass"),
            Self::BelowThreshold => Some("2026-05-13-active-python-cell-status-below-threshold"),
            Self::MissingCorrelation => {
                Some("2026-05-13-active-python-cell-status-missing-correlation")
            }
            Self::Stale80CellGrid => Some("2026-05-13-active-python-cell-status-stale-80"),
            Self::OverLimitCell => Some("2026-05-13-active-python-cell-status-over-limit"),
            Self::EmptyDb => None,
        }
    }

    fn expected_exit_code(self) -> i32 {
        match self {
            Self::Pass => 0,
            Self::BelowThreshold
            | Self::MissingCorrelation
            | Self::Stale80CellGrid
            | Self::OverLimitCell
            | Self::EmptyDb => 1,
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
        Scenario::MissingCorrelation,
        Scenario::Stale80CellGrid,
        Scenario::OverLimitCell,
        Scenario::EmptyDb,
    ]
    .into_iter()
    .map(|scenario| seed_scenario(&fsv_root, scenario))
    .collect::<Result<Vec<_>>>()?;

    let output = json!({
        "schema_version": 1,
        "fsv_name": "active-python-cell-status-seed-fsv",
        "seeded_at_unix_ms": Utc::now().timestamp_millis(),
        "threshold": THRESHOLD,
        "active_gate": ACTIVE_PYTHON_SHIP_GATE_NAME,
        "required_grid": ACTIVE_PYTHON_SHIP_GATE_GRID,
        "required_cell_count": required_active_python_ship_gate_cells().len(),
        "source_of_truth": {
            "column_family": CF_MEJEPA_EVAL_REPORTS,
            "writer": "RocksDbEvalStore::persist_report",
            "readback": "RocksDbEvalStore::load_latest_report",
            "grid": ACTIVE_PYTHON_SHIP_GATE_GRID
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

    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let store =
        RocksDbEvalStore::new(db.clone()).map_err(|err| anyhow!("construct eval store: {err}"))?;
    let (report_date, input_hash, readback_hash) = if matches!(scenario, Scenario::EmptyDb) {
        let readback = store
            .load_latest_report()
            .map_err(|err| anyhow!("load latest empty eval report: {err}"))?;
        ensure!(
            readback.is_none(),
            "empty-db scenario unexpectedly has a report"
        );
        (Value::Null, Value::Null, Value::Null)
    } else {
        let report = build_report(scenario);
        let input_hash = report
            .determinism_hash()
            .map_err(|err| anyhow!("hash input eval report: {err}"))?;
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
        (
            json!(report.report_date),
            json!(input_hash),
            json!(readback_hash),
        )
    };
    let cf_count = count_cf(&db, CF_MEJEPA_EVAL_REPORTS).context("count eval-report CF")?;
    let expected_cf_count = if matches!(scenario, Scenario::EmptyDb) {
        0
    } else {
        1
    };
    ensure!(
        cf_count == expected_cf_count,
        "unexpected eval report row count"
    );
    drop(store);
    drop(db);

    Ok(json!({
        "name": scenario.name(),
        "db_path": db_path,
        "report_date": report_date,
        "expected_exit_code": scenario.expected_exit_code(),
        "expected_required_cell_count": required_active_python_ship_gate_cells().len(),
        "expected_passing_cell_count": match scenario {
            Scenario::Pass => json!(8),
            Scenario::BelowThreshold | Scenario::MissingCorrelation => json!(7),
            Scenario::Stale80CellGrid | Scenario::OverLimitCell | Scenario::EmptyDb => Value::Null,
        },
        "expected_below_threshold_cell_count": if matches!(scenario, Scenario::BelowThreshold) { json!(1) } else if matches!(scenario, Scenario::Pass | Scenario::MissingCorrelation) { json!(0) } else { Value::Null },
        "expected_missing_cell_count": if matches!(scenario, Scenario::MissingCorrelation) { json!(1) } else if matches!(scenario, Scenario::Pass | Scenario::BelowThreshold) { json!(0) } else { Value::Null },
        "expected_failing_cell_count": match scenario {
            Scenario::Pass => json!(0),
            Scenario::BelowThreshold | Scenario::MissingCorrelation => json!(1),
            Scenario::Stale80CellGrid | Scenario::OverLimitCell | Scenario::EmptyDb => Value::Null,
        },
        "target_cell": match scenario {
            Scenario::Pass => Value::Null,
            Scenario::BelowThreshold => json!("off_by_one::python"),
            Scenario::MissingCorrelation => json!("known_good::python"),
            Scenario::Stale80CellGrid => json!("known_good::rust"),
            Scenario::OverLimitCell => json!("known_good::rust"),
            Scenario::EmptyDb => Value::Null,
        },
        "determinism_hash": input_hash,
        "readback_hash": readback_hash,
        "cf_count": cf_count,
        "readback_equal": true,
    }))
}

fn build_report(scenario: Scenario) -> EvalReport {
    let mut per_cell_correlation = BTreeMap::new();
    let cells = match scenario {
        Scenario::Stale80CellGrid => stale_non_python_phase_g_cells(),
        Scenario::OverLimitCell => {
            let mut cells = required_active_python_ship_gate_cells();
            cells.push((
                MutationCategory::KnownGood,
                Language::Rust,
                cell_key(MutationCategory::KnownGood, Language::Rust),
            ));
            cells
        }
        Scenario::Pass | Scenario::BelowThreshold | Scenario::MissingCorrelation => {
            required_active_python_ship_gate_cells()
        }
        Scenario::EmptyDb => unreachable!("empty-db scenario has no report"),
    };
    for (category, language, key) in cells {
        let correlation = if matches!(scenario, Scenario::BelowThreshold)
            && category == MutationCategory::OffByOne
            && language == Language::Python
        {
            Some(BELOW_CORRELATION)
        } else if matches!(scenario, Scenario::MissingCorrelation)
            && category == MutationCategory::KnownGood
            && language == Language::Python
        {
            None
        } else {
            Some(PASS_CORRELATION)
        };
        per_cell_correlation.insert(key, correlation);
    }
    let per_cell_convergence_eta =
        context_graph_mejepa::baseline_convergence_eta_for_cells(&per_cell_correlation, 0.95);

    let ship_gate_passed = matches!(scenario, Scenario::Pass);
    let holdout_count = match scenario {
        Scenario::Stale80CellGrid => 800,
        Scenario::OverLimitCell => 900,
        _ => 2400,
    };
    EvalReport {
        report_date: scenario.report_date().expect("report date").to_string(),
        generated_at_unix_ms: Utc::now().timestamp_millis(),
        rolling_window_size: holdout_count,
        holdout_count,
        overall_correlation: Some(if ship_gate_passed {
            PASS_CORRELATION
        } else {
            BELOW_CORRELATION
        }),
        per_category_correlation: MutationCategory::all()
            .into_iter()
            .map(|category| (category, Some(PASS_CORRELATION)))
            .collect(),
        per_language_correlation: report_languages(scenario)
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
        per_failure_mode_class: context_graph_mejepa::empty_failure_mode_class_metrics(
            holdout_count,
            &context_graph_mejepa::EvalConfig::default(),
        ),
        per_cell_convergence_eta,
        active_learning: ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: None,
        per_cell_state_transfer: BTreeMap::new(),
        failing_cell_classifications: BTreeMap::new(),
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: vec![OpenResearchQuestionStatus {
            id: "TASK-PY-G-036".to_string(),
            question: "Does the active Python ship gate reject stale non-Python cell grids?"
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
            corpus_sha: "active-python-cell-status-fsv".to_string(),
            eval_code_version: "task-py-g-036-fsv".to_string(),
            calibration_version: "task-py-g-036-fsv".to_string(),
            generated_by: "active_python_cell_status_seed_fsv".to_string(),
        },
        wall_clock_seconds: 1.0,
    }
}

fn report_languages(scenario: Scenario) -> Vec<Language> {
    if matches!(scenario, Scenario::Stale80CellGrid) {
        return stale_non_python_languages().to_vec();
    }
    if matches!(scenario, Scenario::OverLimitCell) {
        return vec![Language::Python, Language::Rust];
    }
    vec![Language::Python]
}

fn stale_non_python_phase_g_cells() -> Vec<(MutationCategory, Language, String)> {
    let mut cells =
        Vec::with_capacity(MutationCategory::all().len() * stale_non_python_languages().len());
    for category in MutationCategory::all() {
        for language in stale_non_python_languages() {
            cells.push((category, language, cell_key(category, language)));
        }
    }
    cells
}

fn stale_non_python_languages() -> [Language; 10] {
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
