use crate::calibration::open_infer_rocksdb;
use crate::cli::write_json_0600;
use crate::error::MejepaInferError;
use crate::project_ingest::{
    run_project_ingest, ProjectIngestError, ProjectIngestMode, ProjectIngestRequest,
    ProjectIngestScope,
};
use crate::project_report::{
    run_project_report, ProjectRealityReport, ProjectReportError, ProjectReportRequest,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_PROJECT_REPORTS, CF_MEJEPA_STRESS_RUNS,
};
use rocksdb::{IteratorMode, DB};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const STRESS_TRACE_ROOT: &str = "/zfs/archive/contextgraph/runtime/stress-runs";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStressSize {
    Tiny,
    Small,
    Medium,
    Large,
}

impl ProjectStressSize {
    pub const ALL: [Self; 4] = [Self::Tiny, Self::Small, Self::Medium, Self::Large];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn file_count(self) -> usize {
        match self {
            Self::Tiny => 5,
            Self::Small => 50,
            Self::Medium => 500,
            Self::Large => 5_000,
        }
    }

    pub fn ingest_budget_ms(self) -> u64 {
        match self {
            Self::Tiny => 5_000,
            Self::Small => 60_000,
            Self::Medium => 600_000,
            Self::Large => 6_000_000,
        }
    }

    pub fn parse(value: &str) -> Result<Self, ProjectStressError> {
        match value {
            "tiny" => Ok(Self::Tiny),
            "small" => Ok(Self::Small),
            "medium" => Ok(Self::Medium),
            "large" => Ok(Self::Large),
            other => Err(ProjectStressError::InvalidInput {
                field: "size".to_string(),
                detail: format!("expected tiny|small|medium|large; got {other}"),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StressFaultInjection {
    #[default]
    None,
    OomBeforeIngest,
    NetworkLatencySpike,
    ColdAllAbstain,
    ForcePerformanceRegression,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectStressRequest {
    pub size: ProjectStressSize,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_trace_root")]
    pub trace_root: PathBuf,
    #[serde(default)]
    pub fault: StressFaultInjection,
}

impl ProjectStressRequest {
    pub fn new(size: ProjectStressSize) -> Self {
        Self {
            size,
            run_id: None,
            seed: default_seed(),
            trace_root: default_trace_root(),
            fault: StressFaultInjection::None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StressDefectCounts {
    pub clean_files: usize,
    pub files_with_bugs: usize,
    pub files_with_security_smells: usize,
    pub files_with_perf_issues: usize,
    pub dependency_version_drift_instances: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StressRecallStatus {
    Satisfied,
    Missed,
    CappedByReport,
    InsufficientSupport,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StressDefectRecall {
    pub seeded: usize,
    pub detected: usize,
    pub status: StressRecallStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StressRunVerdict {
    Completed,
    CompletedWithFlags,
    AbortedOom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StressRun {
    pub schema_version: u32,
    pub run_id: String,
    pub project_id: String,
    pub project_size: ProjectStressSize,
    pub seed: u64,
    pub source_of_truth_cf: String,
    pub live_prediction_cf: String,
    pub project_report_cf: String,
    pub trace_dir: String,
    pub project_dir: String,
    pub stress_db_path: String,
    pub project_manifest_path: String,
    pub project_report_json_path: String,
    pub project_report_markdown_path: String,
    pub generated_file_count: usize,
    pub seeded_counts: StressDefectCounts,
    pub ingest_time_ms: u64,
    pub reingest_time_ms: u64,
    pub report_time_ms: u64,
    pub ingest_budget_ms: u64,
    pub incremental_budget_ms: u64,
    pub defect_recall_per_class: BTreeMap<String, StressDefectRecall>,
    pub total_predictions: usize,
    pub memory_peak_mb: u64,
    pub flags: Vec<String>,
    pub verdict: StressRunVerdict,
    pub acceptance_passed: bool,
}

#[derive(Debug, Error)]
pub enum ProjectStressError {
    #[error("MEJEPA_STRESS_INVALID_INPUT: {field}: {detail}")]
    InvalidInput { field: String, detail: String },
    #[error("MEJEPA_STRESS_IO: {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("MEJEPA_STRESS_JSON: {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Path(#[from] context_graph_paths::PathError),
    #[error("{0}")]
    Ingest(#[from] ProjectIngestError),
    #[error("{0}")]
    Report(#[from] ProjectReportError),
    #[error("MEJEPA_STRESS_ROCKSDB: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("MEJEPA_STRESS_BINCODE: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
    #[error("{0}")]
    Infer(#[from] MejepaInferError),
}

impl ProjectStressError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "MEJEPA_STRESS_INVALID_INPUT",
            Self::Io { .. } => "MEJEPA_STRESS_IO",
            Self::Json { .. } => "MEJEPA_STRESS_JSON",
            Self::Path(err) => err.code,
            Self::Ingest(err) => err.code(),
            Self::Report(err) => err.code(),
            Self::RocksDb(_) => "MEJEPA_STRESS_ROCKSDB",
            Self::Bincode(_) => "MEJEPA_STRESS_BINCODE",
            Self::Infer(err) => err.code(),
        }
    }
}

pub fn run_project_stress(request: ProjectStressRequest) -> Result<StressRun, ProjectStressError> {
    let run_id = request
        .run_id
        .clone()
        .unwrap_or_else(|| default_run_id(request.size));
    validate_run_id(&run_id)?;
    let project_id = project_id_for_run(&run_id, request.size);
    let trace_root = require_stress_trace_root(&request.trace_root)?;
    let trace_dir = trace_root.join(&run_id);
    remove_dir_if_exists(&trace_dir)?;
    fs::create_dir_all(&trace_dir).map_err(|source| ProjectStressError::Io {
        path: trace_dir.display().to_string(),
        source,
    })?;
    let generated = generate_project(&trace_dir, request.size, request.seed)?;
    let stress_db_path = trace_dir.join("stress-runs.rocksdb");

    if request.fault == StressFaultInjection::OomBeforeIngest {
        let run = StressRun {
            schema_version: 1,
            run_id,
            project_id,
            project_size: request.size,
            seed: request.seed,
            source_of_truth_cf: CF_MEJEPA_STRESS_RUNS.to_string(),
            live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            project_report_cf: CF_MEJEPA_PROJECT_REPORTS.to_string(),
            trace_dir: trace_dir.display().to_string(),
            project_dir: generated.project_dir.display().to_string(),
            stress_db_path: stress_db_path.display().to_string(),
            project_manifest_path: String::new(),
            project_report_json_path: String::new(),
            project_report_markdown_path: String::new(),
            generated_file_count: generated.file_count,
            seeded_counts: generated.seeded_counts,
            ingest_time_ms: 0,
            reingest_time_ms: 0,
            report_time_ms: 0,
            ingest_budget_ms: request.size.ingest_budget_ms(),
            incremental_budget_ms: 5_000,
            defect_recall_per_class: BTreeMap::new(),
            total_predictions: 0,
            memory_peak_mb: peak_rss_mb(),
            flags: vec!["STRESS_RUN_OOM".to_string()],
            verdict: StressRunVerdict::AbortedOom,
            acceptance_passed: false,
        };
        write_trace_json(&trace_dir, &run)?;
        persist_stress_run_path(&stress_db_path, &run)?;
        return Ok(run);
    }

    let ingest_start = Instant::now();
    let ingest = run_project_ingest(ProjectIngestRequest {
        repo_path: generated.project_dir.clone(),
        project_id: Some(project_id.clone()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::All,
        overwrite: true,
        changed_paths: Vec::new(),
    })?;
    let mut ingest_time_ms = elapsed_ms(ingest_start);

    let changed_file =
        generated
            .incremental_file
            .clone()
            .ok_or_else(|| ProjectStressError::InvalidInput {
                field: "generated_project.incremental_file".to_string(),
                detail: "generated project did not contain an editable clean source file"
                    .to_string(),
            })?;
    let changed_rel_path =
        generated
            .incremental_rel_path
            .clone()
            .ok_or_else(|| ProjectStressError::InvalidInput {
                field: "generated_project.incremental_rel_path".to_string(),
                detail: "generated project did not contain an editable clean relative path"
                    .to_string(),
            })?;
    append_incremental_edit(&changed_file, request.seed)?;
    let reingest_start = Instant::now();
    let reingest = run_project_ingest(ProjectIngestRequest {
        repo_path: generated.project_dir.clone(),
        project_id: Some(project_id.clone()),
        mode: ProjectIngestMode::Incremental,
        scope: ProjectIngestScope::All,
        overwrite: false,
        changed_paths: vec![changed_rel_path],
    })?;
    let reingest_time_ms = elapsed_ms(reingest_start);

    let report_start = Instant::now();
    let project_report = run_project_report(ProjectReportRequest {
        project_id: project_id.clone(),
        section: None,
        offset: None,
    })?;
    let report_time_ms = elapsed_ms(report_start);

    if request.fault == StressFaultInjection::ForcePerformanceRegression {
        ingest_time_ms = request.size.ingest_budget_ms().saturating_add(1);
    }
    let mut defect_recall = defect_recall(
        &project_report.report,
        &generated.seeded_counts,
        request.fault,
    );
    if request.fault == StressFaultInjection::ColdAllAbstain {
        for row in defect_recall.values_mut() {
            row.status = StressRecallStatus::InsufficientSupport;
        }
    }

    let mut run = StressRun {
        schema_version: 1,
        run_id,
        project_id,
        project_size: request.size,
        seed: request.seed,
        source_of_truth_cf: CF_MEJEPA_STRESS_RUNS.to_string(),
        live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        project_report_cf: CF_MEJEPA_PROJECT_REPORTS.to_string(),
        trace_dir: trace_dir.display().to_string(),
        project_dir: generated.project_dir.display().to_string(),
        stress_db_path: stress_db_path.display().to_string(),
        project_manifest_path: ingest.manifest_path,
        project_report_json_path: project_report.report_json_path,
        project_report_markdown_path: project_report.report_md_path,
        generated_file_count: generated.file_count,
        seeded_counts: generated.seeded_counts,
        ingest_time_ms,
        reingest_time_ms,
        report_time_ms,
        ingest_budget_ms: request.size.ingest_budget_ms(),
        incremental_budget_ms: 5_000,
        defect_recall_per_class: defect_recall,
        total_predictions: reingest.manifest.prediction_count,
        memory_peak_mb: peak_rss_mb(),
        flags: Vec::new(),
        verdict: StressRunVerdict::Completed,
        acceptance_passed: false,
    };
    if request.fault == StressFaultInjection::NetworkLatencySpike {
        run.flags
            .push("NETWORK_FILESYSTEM_LATENCY_SPIKE".to_string());
    }
    finalize_stress_run(&mut run);
    write_trace_json(&trace_dir, &run)?;
    persist_stress_run_path(&stress_db_path, &run)?;
    Ok(run)
}

pub fn finalize_stress_run(run: &mut StressRun) {
    if run.ingest_time_ms > run.ingest_budget_ms {
        push_flag(&mut run.flags, "PERFORMANCE_REGRESSION");
    }
    if run.reingest_time_ms > run.incremental_budget_ms {
        push_flag(&mut run.flags, "INCREMENTAL_PERFORMANCE_REGRESSION");
    }
    if matches!(run.verdict, StressRunVerdict::AbortedOom) {
        run.acceptance_passed = false;
        return;
    }
    if !run.flags.is_empty() {
        run.verdict = StressRunVerdict::CompletedWithFlags;
    }
    let recall_ok = run.defect_recall_per_class.values().all(|row| {
        matches!(
            row.status,
            StressRecallStatus::Satisfied
                | StressRecallStatus::CappedByReport
                | StressRecallStatus::InsufficientSupport
                | StressRecallStatus::NotApplicable
        )
    });
    run.acceptance_passed = run.generated_file_count == run.project_size.file_count()
        && run.total_predictions == run.generated_file_count
        && run.reingest_time_ms <= run.incremental_budget_ms
        && recall_ok;
}

pub fn persist_stress_run_path(db_path: &Path, run: &StressRun) -> Result<(), ProjectStressError> {
    let db = open_infer_rocksdb(db_path)?;
    persist_stress_run(db.as_ref(), run)
}

pub fn persist_stress_run(db: &DB, run: &StressRun) -> Result<(), ProjectStressError> {
    let cf =
        db.cf_handle(CF_MEJEPA_STRESS_RUNS)
            .ok_or_else(|| ProjectStressError::InvalidInput {
                field: "rocksdb.column_family".to_string(),
                detail: format!("missing column family {CF_MEJEPA_STRESS_RUNS}"),
            })?;
    let key = stress_run_key(&run.run_id, run.project_size);
    let bytes = bincode::serialize(run)?;
    db.put_cf(cf, &key, &bytes)?;
    let readback = db
        .get_cf(cf, &key)?
        .ok_or_else(|| ProjectStressError::InvalidInput {
            field: "stress_run.readback".to_string(),
            detail: "missing stress run after put_cf".to_string(),
        })?;
    if readback != bytes {
        return Err(ProjectStressError::InvalidInput {
            field: "stress_run.readback".to_string(),
            detail: "readback bytes differ from written stress run".to_string(),
        });
    }
    Ok(())
}

pub fn load_stress_run(
    db: &DB,
    run_id: &str,
    size: ProjectStressSize,
) -> Result<Option<StressRun>, ProjectStressError> {
    let cf =
        db.cf_handle(CF_MEJEPA_STRESS_RUNS)
            .ok_or_else(|| ProjectStressError::InvalidInput {
                field: "rocksdb.column_family".to_string(),
                detail: format!("missing column family {CF_MEJEPA_STRESS_RUNS}"),
            })?;
    db.get_cf(cf, stress_run_key(run_id, size))?
        .map(|bytes| bincode::deserialize(&bytes).map_err(ProjectStressError::from))
        .transpose()
}

pub fn count_stress_runs(db: &DB) -> Result<usize, ProjectStressError> {
    let cf =
        db.cf_handle(CF_MEJEPA_STRESS_RUNS)
            .ok_or_else(|| ProjectStressError::InvalidInput {
                field: "rocksdb.column_family".to_string(),
                detail: format!("missing column family {CF_MEJEPA_STRESS_RUNS}"),
            })?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

pub fn stress_run_key(run_id: &str, size: ProjectStressSize) -> Vec<u8> {
    format!("stress_run:{run_id}:{}", size.as_str()).into_bytes()
}

struct GeneratedProject {
    project_dir: PathBuf,
    file_count: usize,
    seeded_counts: StressDefectCounts,
    incremental_file: Option<PathBuf>,
    incremental_rel_path: Option<String>,
}

#[derive(Clone, Copy)]
struct StressProfile {
    files_with_bugs: usize,
    files_with_security_smells: usize,
    files_with_perf_issues: usize,
    dependency_version_drift_instances: usize,
    test_files: usize,
}

fn generate_project(
    trace_dir: &Path,
    size: ProjectStressSize,
    seed: u64,
) -> Result<GeneratedProject, ProjectStressError> {
    let project_dir = trace_dir.join(format!("synthetic-{}-project", size.as_str()));
    remove_dir_if_exists(&project_dir)?;
    fs::create_dir_all(&project_dir).map_err(|source| ProjectStressError::Io {
        path: project_dir.display().to_string(),
        source,
    })?;
    let profile = stress_profile(size);
    let target = size.file_count();
    let mut rows = Vec::<(String, String, bool)>::new();
    let mut dependency_file_count = 0usize;

    if profile.dependency_version_drift_instances > 0 {
        dependency_file_count = 1;
        let mut requirements = String::new();
        for idx in 0..profile.dependency_version_drift_instances {
            let package = match idx % 3 {
                0 => "django==1.2",
                1 => "requests==2.18.0",
                _ => "urllib3==1.25.0",
            };
            requirements.push_str(&format!(
                "{package}  # STRESS_DEP_DRIFT seeded={seed} idx={idx}\n"
            ));
        }
        rows.push(("requirements.txt".to_string(), requirements, false));
    }
    if target >= 10 {
        rows.push((
            "pyproject.toml".to_string(),
            "[project]\nname = \"contextgraph-stress\"\nversion = \"0.0.0\"\n".to_string(),
            true,
        ));
        rows.push((
            "README.md".to_string(),
            "# Synthetic stress project\n\nGenerated for TASK-PY-G-072.\n".to_string(),
            true,
        ));
    }
    for idx in 0..profile.test_files {
        rows.push((
            format!("tests/test_smoke_{idx:04}.py"),
            format!(
                "from pkg.clean_{idx:04} import value_{idx}\n\n\
                 def test_value_{idx}():\n    assert value_{idx}() == {idx}\n"
            ),
            true,
        ));
    }
    for idx in 0..profile.files_with_bugs {
        rows.push((
            format!("pkg/bugs/divide_{idx:04}.py"),
            format!("def divide_{idx}(a, b):\n    return a / b\n"),
            false,
        ));
    }
    for idx in 0..profile.files_with_security_smells {
        rows.push((
            format!("pkg/security/eval_{idx:04}.py"),
            format!("def run_{idx}(expr):\n    return eval(expr)\n"),
            false,
        ));
    }
    for idx in 0..profile.files_with_perf_issues {
        rows.push((
            format!("pkg/perf/quadratic_{idx:04}.py"),
            format!(
                "def pair_sum_{idx}(items):\n    total = 0\n    # STRESS_PERF_QUADRATIC\n    for left in items:\n        for right in items:\n            total += left * right\n    return total\n"
            ),
            false,
        ));
    }
    if rows.len() > target {
        return Err(ProjectStressError::InvalidInput {
            field: "stress_profile".to_string(),
            detail: format!("profile generated {} rows for target {target}", rows.len()),
        });
    }
    let mut clean_idx = 0usize;
    while rows.len() < target {
        rows.push((
            format!("pkg/clean_{clean_idx:04}.py"),
            format!("def value_{clean_idx}():\n    return {clean_idx}\n"),
            true,
        ));
        clean_idx += 1;
    }

    let mut clean_files = 0usize;
    let mut incremental_file = None;
    let mut incremental_rel_path = None;
    for (path, content, clean) in &rows {
        write_project_file(&project_dir.join(path), content)?;
        if *clean {
            clean_files += 1;
            if incremental_file.is_none() && path.ends_with(".py") && path.starts_with("pkg/") {
                incremental_file = Some(project_dir.join(path));
                incremental_rel_path = Some(path.clone());
            }
        }
    }
    let seeded_counts = StressDefectCounts {
        clean_files,
        files_with_bugs: profile.files_with_bugs,
        files_with_security_smells: profile.files_with_security_smells,
        files_with_perf_issues: profile.files_with_perf_issues,
        dependency_version_drift_instances: profile.dependency_version_drift_instances,
    };
    if dependency_file_count > target {
        return Err(ProjectStressError::InvalidInput {
            field: "dependency_file_count".to_string(),
            detail: "dependency file count exceeded project size".to_string(),
        });
    }
    Ok(GeneratedProject {
        project_dir,
        file_count: rows.len(),
        seeded_counts,
        incremental_file,
        incremental_rel_path,
    })
}

fn stress_profile(size: ProjectStressSize) -> StressProfile {
    match size {
        ProjectStressSize::Tiny => StressProfile {
            files_with_bugs: 1,
            files_with_security_smells: 1,
            files_with_perf_issues: 0,
            dependency_version_drift_instances: 1,
            test_files: 1,
        },
        ProjectStressSize::Small => StressProfile {
            files_with_bugs: 4,
            files_with_security_smells: 3,
            files_with_perf_issues: 3,
            dependency_version_drift_instances: 2,
            test_files: 5,
        },
        ProjectStressSize::Medium => StressProfile {
            files_with_bugs: 8,
            files_with_security_smells: 6,
            files_with_perf_issues: 6,
            dependency_version_drift_instances: 4,
            test_files: 20,
        },
        ProjectStressSize::Large => StressProfile {
            files_with_bugs: 8,
            files_with_security_smells: 6,
            files_with_perf_issues: 6,
            dependency_version_drift_instances: 5,
            test_files: 100,
        },
    }
}

fn defect_recall(
    report: &ProjectRealityReport,
    seeded: &StressDefectCounts,
    fault: StressFaultInjection,
) -> BTreeMap<String, StressDefectRecall> {
    let insufficient = fault == StressFaultInjection::ColdAllAbstain;
    let bug_detected = report
        .top_likely_broken_areas
        .iter()
        .filter(|row| row.explanation.contains("DivisionByZero"))
        .count();
    let perf_detected = report
        .top_likely_broken_areas
        .iter()
        .filter(|row| row.explanation.contains("QuadraticPerf"))
        .count();
    let dependency_detected = report
        .top_likely_broken_areas
        .iter()
        .filter(|row| row.explanation.contains("DependencyVersionDrift"))
        .count();
    let security_detected = report.top_vulnerability_slices.len();
    let clean_detected = report.predicted_works.len();

    BTreeMap::from([
        (
            "clean_files".to_string(),
            recall_row(seeded.clean_files, clean_detected, insufficient, true),
        ),
        (
            "files_with_bugs".to_string(),
            recall_row(seeded.files_with_bugs, bug_detected, insufficient, false),
        ),
        (
            "files_with_security_smells".to_string(),
            recall_row(
                seeded.files_with_security_smells,
                security_detected,
                insufficient,
                false,
            ),
        ),
        (
            "files_with_perf_issues".to_string(),
            recall_row(
                seeded.files_with_perf_issues,
                perf_detected,
                insufficient,
                false,
            ),
        ),
        (
            "dependency_version_drift_instances".to_string(),
            recall_row(
                seeded.dependency_version_drift_instances,
                dependency_detected,
                insufficient,
                false,
            ),
        ),
    ])
}

fn recall_row(
    seeded: usize,
    detected: usize,
    insufficient: bool,
    capped_report_section: bool,
) -> StressDefectRecall {
    let status = if insufficient {
        StressRecallStatus::InsufficientSupport
    } else if seeded == 0 {
        StressRecallStatus::NotApplicable
    } else if detected >= seeded {
        StressRecallStatus::Satisfied
    } else if capped_report_section && detected >= 20 {
        StressRecallStatus::CappedByReport
    } else {
        StressRecallStatus::Missed
    };
    StressDefectRecall {
        seeded,
        detected,
        status,
    }
}

fn write_project_file(path: &Path, content: &str) -> Result<(), ProjectStressError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ProjectStressError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| ProjectStressError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn append_incremental_edit(path: &Path, seed: u64) -> Result<(), ProjectStressError> {
    let mut content = fs::read_to_string(path).map_err(|source| ProjectStressError::Io {
        path: path.display().to_string(),
        source,
    })?;
    content.push_str(&format!("\n# incremental-stress-edit-{seed}\n"));
    fs::write(path, content).map_err(|source| ProjectStressError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn write_trace_json(trace_dir: &Path, run: &StressRun) -> Result<(), ProjectStressError> {
    let trace_path = trace_dir.join("stress_run.json");
    write_json_0600(&trace_path, run)?;
    let readback: StressRun = serde_json::from_slice(&fs::read(&trace_path).map_err(|source| {
        ProjectStressError::Io {
            path: trace_path.display().to_string(),
            source,
        }
    })?)
    .map_err(|source| ProjectStressError::Json {
        path: trace_path.display().to_string(),
        source,
    })?;
    if readback.run_id != run.run_id || readback.generated_file_count != run.generated_file_count {
        return Err(ProjectStressError::InvalidInput {
            field: "stress_run_json.readback".to_string(),
            detail: "trace JSON readback did not match written stress run".to_string(),
        });
    }
    Ok(())
}

fn validate_run_id(value: &str) -> Result<(), ProjectStressError> {
    if !value.is_empty()
        && value.len() <= 96
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(())
    } else {
        Err(ProjectStressError::InvalidInput {
            field: "run_id".to_string(),
            detail: format!("invalid stress run id {value:?}"),
        })
    }
}

fn project_id_for_run(run_id: &str, size: ProjectStressSize) -> String {
    let digest = blake3::hash(format!("{run_id}\0{}", size.as_str()).as_bytes());
    let digest_hex = digest.to_hex().to_string();
    format!("task-py-g-072-{}-{}", size.as_str(), &digest_hex[..12])
}

fn default_run_id(size: ProjectStressSize) -> String {
    format!("task-py-g-072-{}-{}", size.as_str(), now_ms())
}

fn default_seed() -> u64 {
    72
}

fn default_trace_root() -> PathBuf {
    PathBuf::from(STRESS_TRACE_ROOT)
}

fn require_stress_trace_root(path: &Path) -> Result<PathBuf, ProjectStressError> {
    let normalized =
        context_graph_paths::require_production_durable_root(path, "project_stress.trace_root")?;
    let production_runtime = Path::new(context_graph_paths::AIWONDER_DURABLE_ROOT).join("runtime");
    if normalized.starts_with(&production_runtime) {
        Ok(normalized)
    } else {
        Err(ProjectStressError::InvalidInput {
            field: "trace_root".to_string(),
            detail: format!(
                "{} must live under /zfs/archive/contextgraph/runtime in production",
                normalized.display()
            ),
        })
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn remove_dir_if_exists(path: &Path) -> Result<(), ProjectStressError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|source| ProjectStressError::Io {
            path: path.display().to_string(),
            source,
        })?;
    }
    Ok(())
}

fn peak_rss_mb() -> u64 {
    let Ok(status) = fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    status
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix("VmHWM:")?;
            let kb = rest.split_whitespace().next()?.parse::<u64>().ok()?;
            Some(kb.div_ceil(1024))
        })
        .unwrap_or(0)
}

fn push_flag(flags: &mut Vec<String>, flag: &str) {
    if !flags.iter().any(|value| value == flag) {
        flags.push(flag.to_string());
    }
}
