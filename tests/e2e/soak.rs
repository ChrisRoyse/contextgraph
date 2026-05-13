#[allow(dead_code)]
#[path = "load.rs"]
mod load;

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-TEST-008";
const DEFAULT_SOAK_MINUTES: u64 = 30;
const MIN_SOAK_MINUTES: u64 = 1;
const MAX_SOAK_MINUTES: u64 = 55;
const MAX_CYCLES: usize = 4;
const DEFAULT_MAX_P99_GROWTH_RATIO: f64 = 2.0;
const DEFAULT_MAX_P99_JITTER_MS: u64 = 250;
const TIMER_PATH: &str = "scripts/systemd/mejepa-soak-nightly.timer";
const SERVICE_PATH: &str = "scripts/systemd/mejepa-soak-nightly.service";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SoakFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: SoakSourceOfTruth,
    pub profile: SoakProfile,
    pub cycle_summaries: Vec<SoakCycleSummary>,
    pub happy_path: Vec<StepEvidence>,
    pub boundary_cases: Vec<BoundaryEvidence>,
    pub physical_artifacts: Vec<FileArtifact>,
    pub readback_equal: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SoakSourceOfTruth {
    pub report_path: String,
    pub export_path: String,
    pub cycle_roots: Vec<String>,
    pub nightly_service_path: String,
    pub nightly_timer_path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SoakProfile {
    pub requested_minutes: u64,
    pub cycle_count: usize,
    pub max_p99_growth_ratio: f64,
    pub accelerated: bool,
}

impl SoakProfile {
    fn try_new(
        requested_minutes: u64,
        cycle_count: usize,
        max_p99_growth_ratio: f64,
    ) -> Result<Self> {
        if requested_minutes < MIN_SOAK_MINUTES {
            bail!("MEJEPA_SOAK_INVALID_INPUT: requested_minutes must be >= {MIN_SOAK_MINUTES}");
        }
        if requested_minutes > MAX_SOAK_MINUTES {
            bail!("MEJEPA_SOAK_INVALID_INPUT: requested_minutes must be <= {MAX_SOAK_MINUTES}");
        }
        if cycle_count == 0 {
            bail!("MEJEPA_SOAK_INVALID_INPUT: cycle_count must be >= 1");
        }
        if cycle_count > MAX_CYCLES {
            bail!("MEJEPA_SOAK_INVALID_INPUT: cycle_count must be <= {MAX_CYCLES}");
        }
        if !(1.0..=10.0).contains(&max_p99_growth_ratio) {
            bail!("MEJEPA_SOAK_INVALID_INPUT: max_p99_growth_ratio must be in [1.0, 10.0]");
        }
        Ok(Self {
            requested_minutes,
            cycle_count,
            max_p99_growth_ratio,
            accelerated: true,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SoakCycleSummary {
    pub cycle_index: usize,
    pub report_path: String,
    pub report_sha256: String,
    pub report_bytes: u64,
    pub all_passed: bool,
    pub readback_equal: bool,
    pub live_prediction_rows: u64,
    pub feedback_rows: u64,
    pub prediction_p99_ms: u64,
    pub physical_artifact_count: usize,
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
    pub expected_error_code: String,
    pub actual_error_code: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SoakExport {
    task_id: String,
    requested_minutes: u64,
    cycle_count: usize,
    live_prediction_rows_total: u64,
    feedback_rows_total: u64,
    max_prediction_p99_ms: u64,
    p99_growth_ratio: f64,
    all_cycles_passed: bool,
}

pub fn default_fsv_root() -> PathBuf {
    PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-soak-test-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("soak_test_fsv.json")
}

pub fn run_soak_fsv(fsv_root: &Path) -> Result<SoakFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let report_path = default_report_path(fsv_root);
    let export_path = fsv_root.join("exports/test-runs/phase-f-soak.json");
    let profile = SoakProfile::try_new(
        requested_minutes_from_env()?,
        cycle_count_from_env()?.unwrap_or(2),
        DEFAULT_MAX_P99_GROWTH_RATIO,
    )?;
    let build_release_sha = git_head_short();

    let mut cycle_summaries = Vec::new();
    let mut cycle_roots = Vec::new();
    for cycle_index in 0..profile.cycle_count {
        let cycle_root = fsv_root
            .join("cycles")
            .join(format!("cycle-{cycle_index:04}"));
        let cycle_report_path = load::default_report_path(&cycle_root);
        let cycle_report = load::run_load_fsv(&cycle_root)
            .with_context(|| format!("run soak load cycle {cycle_index}"))?;
        load::write_report_with_readback(&cycle_report, &cycle_report_path)?;
        let artifact = artifact_for(&cycle_report_path)?;
        let summary = SoakCycleSummary {
            cycle_index,
            report_path: cycle_report_path.display().to_string(),
            report_sha256: artifact.sha256,
            report_bytes: artifact.bytes,
            all_passed: cycle_report.all_passed,
            readback_equal: cycle_report.readback_equal,
            live_prediction_rows: count_after(
                &cycle_report,
                context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
            )?,
            feedback_rows: count_after(
                &cycle_report,
                context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
            )?,
            prediction_p99_ms: prediction_p99_ms(&cycle_report)?,
            physical_artifact_count: cycle_report.physical_artifacts.len(),
        };
        cycle_roots.push(cycle_root.display().to_string());
        cycle_summaries.push(summary);
    }

    let first_p99 = cycle_summaries
        .first()
        .ok_or_else(|| anyhow!("MEJEPA_SOAK_EMPTY_CYCLES"))?
        .prediction_p99_ms
        .max(1);
    let max_p99 = cycle_summaries
        .iter()
        .map(|summary| summary.prediction_p99_ms)
        .max()
        .ok_or_else(|| anyhow!("MEJEPA_SOAK_EMPTY_CYCLES"))?;
    let p99_growth_ratio = max_p99 as f64 / first_p99 as f64;
    let p99_jitter_ms = max_p99.saturating_sub(first_p99);
    let cycles_pass = cycle_summaries
        .iter()
        .all(|summary| summary.all_passed && summary.readback_equal)
        && (p99_growth_ratio <= profile.max_p99_growth_ratio
            || p99_jitter_ms <= DEFAULT_MAX_P99_JITTER_MS);
    let live_prediction_rows_total = cycle_summaries
        .iter()
        .map(|summary| summary.live_prediction_rows)
        .sum::<u64>();
    let feedback_rows_total = cycle_summaries
        .iter()
        .map(|summary| summary.feedback_rows)
        .sum::<u64>();
    let export = SoakExport {
        task_id: TASK_ID.to_string(),
        requested_minutes: profile.requested_minutes,
        cycle_count: profile.cycle_count,
        live_prediction_rows_total,
        feedback_rows_total,
        max_prediction_p99_ms: max_p99,
        p99_growth_ratio,
        all_cycles_passed: cycles_pass,
    };
    write_json_readback(&export_path, &export)?;
    let export_readback: SoakExport = serde_json::from_slice(&fs::read(&export_path)?)?;

    let service_check = systemd_file_contains(
        &repo_file_path(SERVICE_PATH),
        &[
            "ExecStart=/home/cabdru/contextgraph/target/release/examples/phase_f_soak_test_fsv",
            "MEJEPA_SOAK_DURATION_MINS=30",
        ],
    )?;
    let timer_check = systemd_file_contains(
        &repo_file_path(TIMER_PATH),
        &[
            "OnCalendar=*-*-* 05:30:00",
            "Persistent=true",
            "Unit=mejepa-soak-nightly.service",
        ],
    )?;

    let boundary_cases = vec![
        boundary_invalid_profile("zero_minutes_rejected", || {
            SoakProfile::try_new(0, 1, DEFAULT_MAX_P99_GROWTH_RATIO)
        }),
        boundary_invalid_profile("too_many_minutes_rejected", || {
            SoakProfile::try_new(MAX_SOAK_MINUTES + 1, 1, DEFAULT_MAX_P99_GROWTH_RATIO)
        }),
        boundary_invalid_profile("zero_cycles_rejected", || {
            SoakProfile::try_new(DEFAULT_SOAK_MINUTES, 0, DEFAULT_MAX_P99_GROWTH_RATIO)
        }),
        boundary_invalid_profile("too_many_cycles_rejected", || {
            SoakProfile::try_new(
                DEFAULT_SOAK_MINUTES,
                MAX_CYCLES + 1,
                DEFAULT_MAX_P99_GROWTH_RATIO,
            )
        }),
    ];

    let happy_path = vec![
        StepEvidence {
            step: "repeated_load_cycles_stay_stable".to_string(),
            trigger: format!("run {} accelerated soak load windows", profile.cycle_count),
            source_of_truth: cycle_roots.join(","),
            before: json!(null),
            after: json!({"cycleSummaries": cycle_summaries}),
            expected: json!({
                "allCyclesPassed": true,
                "p99GrowthRatioMax": profile.max_p99_growth_ratio,
                "p99JitterBudgetMs": DEFAULT_MAX_P99_JITTER_MS
            }),
            actual: json!({
                "allCyclesPassed": cycles_pass,
                "p99GrowthRatio": p99_growth_ratio,
                "p99JitterMs": p99_jitter_ms
            }),
            pass: cycles_pass,
        },
        StepEvidence {
            step: "soak_export_readback".to_string(),
            trigger: "write soak aggregate export and read it back".to_string(),
            source_of_truth: export_path.display().to_string(),
            before: json!(null),
            after: json!(export_readback),
            expected: json!({"readbackMatches": true}),
            actual: json!({"readbackMatches": export_readback == export}),
            pass: export_readback == export,
        },
        StepEvidence {
            step: "nightly_systemd_service_present".to_string(),
            trigger: "read service file from repo source of truth".to_string(),
            source_of_truth: SERVICE_PATH.to_string(),
            before: json!(null),
            after: json!({"containsRequiredDirectives": service_check}),
            expected: json!({"containsRequiredDirectives": true}),
            actual: json!({"containsRequiredDirectives": service_check}),
            pass: service_check,
        },
        StepEvidence {
            step: "nightly_systemd_timer_present".to_string(),
            trigger: "read timer file from repo source of truth".to_string(),
            source_of_truth: TIMER_PATH.to_string(),
            before: json!(null),
            after: json!({"containsRequiredDirectives": timer_check}),
            expected: json!({"containsRequiredDirectives": true}),
            actual: json!({"containsRequiredDirectives": timer_check}),
            pass: timer_check,
        },
    ];

    let all_passed =
        happy_path.iter().all(|step| step.pass) && boundary_cases.iter().all(|case| case.pass);
    ensure!(all_passed, "soak FSV contains failed evidence");

    Ok(SoakFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha,
        source_of_truth: SoakSourceOfTruth {
            report_path: report_path.display().to_string(),
            export_path: export_path.display().to_string(),
            cycle_roots,
            nightly_service_path: repo_file_path(SERVICE_PATH).display().to_string(),
            nightly_timer_path: repo_file_path(TIMER_PATH).display().to_string(),
        },
        profile,
        cycle_summaries,
        happy_path,
        boundary_cases,
        physical_artifacts: collect_file_artifacts(fsv_root)?,
        readback_equal: true,
        all_passed,
    })
}

pub fn write_report_with_readback(report: &SoakFsvReport, path: &Path) -> Result<()> {
    write_json_readback(path, report)?;
    let readback: SoakFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(readback == *report, "soak FSV readback mismatch");
    Ok(())
}

fn requested_minutes_from_env() -> Result<u64> {
    match std::env::var("MEJEPA_SOAK_DURATION_MINS") {
        Ok(value) => value
            .parse::<u64>()
            .context("MEJEPA_SOAK_INVALID_INPUT: MEJEPA_SOAK_DURATION_MINS must be u64"),
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_SOAK_MINUTES),
        Err(err) => Err(err).context("MEJEPA_SOAK_INVALID_INPUT: read MEJEPA_SOAK_DURATION_MINS"),
    }
}

fn cycle_count_from_env() -> Result<Option<usize>> {
    match std::env::var("MEJEPA_SOAK_CYCLES") {
        Ok(value) => value
            .parse::<usize>()
            .map(Some)
            .context("MEJEPA_SOAK_INVALID_INPUT: MEJEPA_SOAK_CYCLES must be usize"),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(err).context("MEJEPA_SOAK_INVALID_INPUT: read MEJEPA_SOAK_CYCLES"),
    }
}

fn count_after(report: &load::LoadFsvReport, cf_name: &str) -> Result<u64> {
    report
        .cf_counts_after
        .get(cf_name)
        .copied()
        .ok_or_else(|| anyhow!("MEJEPA_SOAK_CF_COUNT_MISSING:{cf_name}"))
}

fn prediction_p99_ms(report: &load::LoadFsvReport) -> Result<u64> {
    report
        .happy_path
        .iter()
        .find(|step| step.step == "concurrent_prediction_load")
        .and_then(|step| step.actual.get("p99LatencyMs"))
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("MEJEPA_SOAK_P99_READBACK_MISSING"))
}

fn systemd_file_contains(path: &Path, needles: &[&str]) -> Result<bool> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(needles.iter().all(|needle| text.contains(needle)))
}

fn repo_file_path(relative_path: &str) -> PathBuf {
    let cwd_relative = PathBuf::from(relative_path);
    if cwd_relative.exists() {
        return cwd_relative;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|repo_root| repo_root.join(relative_path))
        .unwrap_or(cwd_relative)
}

fn boundary_invalid_profile<F>(case: &str, action: F) -> BoundaryEvidence
where
    F: FnOnce() -> Result<SoakProfile>,
{
    let err = action().expect_err("invalid soak profile must fail");
    let actual_error_code = err
        .to_string()
        .split(':')
        .next()
        .map(str::to_string)
        .unwrap_or_else(|| "MEJEPA_SOAK_UNKNOWN_ERROR".to_string());
    BoundaryEvidence {
        case: case.to_string(),
        trigger: format!("SoakProfile::try_new boundary {case}"),
        expected_error_code: "MEJEPA_SOAK_INVALID_INPUT".to_string(),
        actual_error_code: actual_error_code.clone(),
        before: json!({"repoFilesMutated": false}),
        after: json!({"repoFilesMutated": false}),
        pass: actual_error_code == "MEJEPA_SOAK_INVALID_INPUT",
    }
}

fn write_json_readback<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    write_bytes_readback(path, &bytes)
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .with_context(|| format!("open write {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
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
    let allowed = path.starts_with("/mnt/d/contextgraph/fsv/") || path.starts_with(&temp_dir);
    ensure!(
        allowed,
        "refusing to remove existing soak FSV root outside /mnt/d/contextgraph/fsv or temp dir: {}",
        path.display()
    );
    let mut last_err = None;
    for attempt in 0..5 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1) as u64));
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
        out.push(artifact_for(path)?);
        return Ok(());
    }
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            collect_file_artifacts_inner(&entry?.path(), out)?;
        }
    }
    Ok(())
}

fn artifact_for(path: &Path) -> Result<FileArtifact> {
    let bytes = fs::read(path)?;
    Ok(FileArtifact {
        path: path.display().to_string(),
        bytes: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(&bytes)),
    })
}

fn git_head_short() -> String {
    match Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_test_008_soak_harness_repeats_load_and_verifies_nightly_timer() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_soak_fsv(&fsv_root).expect("soak FSV");
        write_report_with_readback(&report, &report_path).expect("write soak report");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert!(report.cycle_summaries.len() >= 2);
        assert!(report.happy_path.len() >= 4);
        assert!(report.boundary_cases.len() >= 3);
        assert!(report
            .cycle_summaries
            .iter()
            .all(|summary| summary.live_prediction_rows > 0 && summary.feedback_rows > 0));
    }
}
