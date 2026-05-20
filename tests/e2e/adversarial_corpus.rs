use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use context_graph_mejepa::calibration::count_cf;
use context_graph_mejepa::conformal::non_conformity_score;
use context_graph_mejepa::gates::valid_witness_segment;
use context_graph_mejepa::{
    build_fixture_deterministic_compiler, open_infer_rocksdb, sha256_bytes, AstDiff,
    CalibrationExample, CalibrationStore, DiffHunk, Language, MeJepaInferConfig, PatchBundle,
    RocksDbInferStore, TaskContext, TaskEnvironment, TaskId, TestId, VerifyVerdict,
    CF_MEJEPA_LIVE_PREDICTIONS, MEJEPA_INFER_CFS,
};
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-TEST-010";
const HEADER_PREFIX: &str = "# me-jepa-adversarial-case ";
const FALSE_CONFIDENT_PASS_THRESHOLD: f32 = 0.95;
const MIN_NON_PASS_RATE: f32 = 0.80;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AdversarialCorpusFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: AdversarialSourceOfTruth,
    pub corpus_counts: BTreeMap<String, u64>,
    pub aggregate: AdversarialAggregate,
    pub happy_path: Vec<AdversarialCaseEvidence>,
    pub boundary_cases: Vec<BoundaryEvidence>,
    pub cf_counts_before: BTreeMap<String, u64>,
    pub cf_counts_after: BTreeMap<String, u64>,
    pub physical_artifacts: Vec<FileArtifact>,
    pub readback_equal: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AdversarialSourceOfTruth {
    pub fixture_root: String,
    pub report_path: String,
    pub infer_db_path: String,
    pub live_prediction_cf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AdversarialAggregate {
    pub total_cases: u64,
    pub non_pass_count: u64,
    pub non_pass_rate: f32,
    pub high_confidence_false_passes: u64,
    pub false_confident_pass_threshold: f32,
    pub categories_with_exactly_ten_cases: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AdversarialCaseEvidence {
    pub case_id: String,
    pub category: String,
    pub scenario: String,
    pub fixture_path: String,
    pub fixture_sha256: String,
    pub trigger: String,
    pub expected: Value,
    pub actual: Value,
    pub final_classification: String,
    pub predicted_verdict: Option<String>,
    pub calibrated_confidence: Option<f32>,
    pub failed_gate: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AdversarialCategory {
    BuggyPassing,
    KnownGoodLookalike,
    EmbedderTargeted,
}

impl AdversarialCategory {
    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "buggy_passing" => Ok(Self::BuggyPassing),
            "knowngood_lookalike" => Ok(Self::KnownGoodLookalike),
            "embedder_targeted" => Ok(Self::EmbedderTargeted),
            other => bail!("MEJEPA_ADVERSARIAL_UNKNOWN_CATEGORY: {other}"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::BuggyPassing => "buggy_passing",
            Self::KnownGoodLookalike => "knowngood_lookalike",
            Self::EmbedderTargeted => "embedder_targeted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdversarialScenario {
    PredictedFailure,
    GrangerRejection,
    ConstellationReject,
}

impl AdversarialScenario {
    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "predicted_failure" => Ok(Self::PredictedFailure),
            "granger_rejection" => Ok(Self::GrangerRejection),
            "constellation_reject" => Ok(Self::ConstellationReject),
            other => bail!("MEJEPA_ADVERSARIAL_UNKNOWN_SCENARIO: {other}"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::PredictedFailure => "predicted_failure",
            Self::GrangerRejection => "granger_rejection",
            Self::ConstellationReject => "constellation_reject",
        }
    }

    fn marker(self) -> &'static str {
        match self {
            Self::PredictedFailure => "scenario: predicted_failure",
            Self::GrangerRejection => "scenario: granger_rejection",
            Self::ConstellationReject => "scenario: constellation_reject",
        }
    }
}

#[derive(Debug, Clone)]
struct AdversarialCase {
    id: String,
    category: AdversarialCategory,
    scenario: AdversarialScenario,
    fixture_path: PathBuf,
    target_path: PathBuf,
    diff_text: String,
    fixture_sha256: String,
}

pub fn default_fsv_root() -> PathBuf {
    PathBuf::from("/zfs/archive/contextgraph/fsv/phase-f-adversarial-corpus-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("adversarial_corpus_fsv.json")
}

pub fn run_adversarial_corpus_fsv(fsv_root: &Path) -> Result<AdversarialCorpusFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let fixture_root = fixture_root()?;
    let report_path = default_report_path(fsv_root);
    let db_path = fsv_root.join("infer-rocksdb");
    let repo_root = fsv_root.join("repo");
    fs::create_dir_all(&repo_root)?;

    let cases = load_cases(&fixture_root)?;
    validate_cases(&cases)?;
    let corpus_counts = corpus_counts(&cases);
    let cf_counts_before = {
        let db = open_infer_rocksdb(&db_path)?;
        count_live_predictions(db.as_ref())?
    };

    let (happy_path, cf_counts_after) = {
        let db = open_infer_rocksdb(&db_path)?;
        let calibration = seed_calibration(db.clone())?;
        let store = Arc::new(RocksDbInferStore::new(db.clone()));
        let compiler = build_fixture_deterministic_compiler(
            repo_root.clone(),
            store,
            calibration,
            MeJepaInferConfig {
                outcome_set_max: 4,
                ..MeJepaInferConfig::default()
            },
        )?;
        let mut results = Vec::with_capacity(cases.len());
        for case in &cases {
            results.push(run_case(&compiler, &repo_root, case)?);
        }
        flush_infer_db(db.as_ref())?;
        (results, count_live_predictions(db.as_ref())?)
    };

    let reopened_counts = {
        let db = open_infer_rocksdb(&db_path)?;
        count_live_predictions(db.as_ref())?
    };
    ensure!(
        reopened_counts == cf_counts_after,
        "live prediction count changed after reopen"
    );

    let boundary_cases = vec![
        boundary_empty_diff(&fixture_root, &cases)?,
        boundary_duplicate_case_id(&cases)?,
        boundary_unknown_category(&cases)?,
        boundary_wrong_category_directory(&fixture_root, &cases)?,
    ];

    let total_cases = happy_path.len() as u64;
    let non_pass_count = happy_path.iter().filter(|case| case.pass).count() as u64;
    let live_rows_before = cf_counts_before
        .get(CF_MEJEPA_LIVE_PREDICTIONS)
        .copied()
        .unwrap_or(0);
    let live_rows_after = reopened_counts
        .get(CF_MEJEPA_LIVE_PREDICTIONS)
        .copied()
        .unwrap_or(0);
    let high_confidence_false_passes = happy_path
        .iter()
        .filter(|case| {
            case.final_classification == "Pass"
                && case
                    .calibrated_confidence
                    .is_some_and(|confidence| confidence > FALSE_CONFIDENT_PASS_THRESHOLD)
        })
        .count() as u64;
    let non_pass_rate = non_pass_count as f32 / total_cases.max(1) as f32;
    let categories_with_exactly_ten_cases =
        corpus_counts.values().all(|count| *count == 10) && corpus_counts.len() == 3;
    let aggregate = AdversarialAggregate {
        total_cases,
        non_pass_count,
        non_pass_rate,
        high_confidence_false_passes,
        false_confident_pass_threshold: FALSE_CONFIDENT_PASS_THRESHOLD,
        categories_with_exactly_ten_cases,
    };
    let all_passed = total_cases == 30
        && non_pass_rate >= MIN_NON_PASS_RATE
        && high_confidence_false_passes == 0
        && categories_with_exactly_ten_cases
        && live_rows_after == live_rows_before + total_cases
        && happy_path.iter().all(|case| case.pass)
        && boundary_cases.iter().all(|case| case.pass)
        && reopened_counts == cf_counts_after;
    ensure!(
        all_passed,
        "adversarial corpus FSV report contains failed evidence"
    );

    let report = AdversarialCorpusFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha: git_head_short(),
        source_of_truth: AdversarialSourceOfTruth {
            fixture_root: fixture_root.display().to_string(),
            report_path: report_path.display().to_string(),
            infer_db_path: db_path.display().to_string(),
            live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        },
        corpus_counts,
        aggregate,
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after: reopened_counts,
        physical_artifacts: collect_file_artifacts(fsv_root, &fixture_root)?,
        readback_equal: true,
        all_passed,
    };
    Ok(report)
}

pub fn write_report_with_readback(report: &AdversarialCorpusFsvReport, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback: AdversarialCorpusFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        readback == *report,
        "adversarial FSV report readback mismatch"
    );
    Ok(())
}

fn run_case(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    repo_root: &Path,
    case: &AdversarialCase,
) -> Result<AdversarialCaseEvidence> {
    let abs_target = repo_root.join(&case.target_path);
    write_bytes_readback(&abs_target, case.diff_text.as_bytes())?;
    let post_sha = sha256_bytes(case.diff_text.as_bytes());
    let session_id = session_id_for_case(&case.id);
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: case.target_path.clone(),
                pre_sha: [0; 32],
                post_sha,
                before: String::new(),
                after: case.diff_text.clone(),
            }],
        },
        valid_witness_segment(),
        format!("Phase F adversarial corpus {}", case.id),
        sha256_bytes(format!("phase-f-adversarial:{}", case.id).as_bytes()),
    )?;
    let context = TaskContext {
        task_id: TaskId(format!("phase-f-adversarial-{}", case.id)),
        session_id,
        language: Language::Python,
        problem_statement: format!(
            "TASK-TEST-010 adversarial corpus category={} {}",
            case.category.as_str(),
            case.scenario.marker()
        ),
        tests: vec![TestId(format!("test_{}", case.id))],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    };
    let verdict = compiler.verify(&patch, &context)?;
    let classification = classification(&verdict);
    let predicted_verdict = predicted_verdict(&verdict);
    let calibrated_confidence = calibrated_confidence(&verdict);
    let failed_gate = failed_gate(&verdict);
    let is_final_pass = matches!(verdict, VerifyVerdict::Approve { .. });
    let high_confidence_false_pass = is_final_pass
        && calibrated_confidence
            .is_some_and(|confidence| confidence > FALSE_CONFIDENT_PASS_THRESHOLD);
    let pass = !is_final_pass && !high_confidence_false_pass;
    Ok(AdversarialCaseEvidence {
        case_id: case.id.clone(),
        category: case.category.as_str().to_string(),
        scenario: case.scenario.as_str().to_string(),
        fixture_path: case.fixture_path.display().to_string(),
        fixture_sha256: case.fixture_sha256.clone(),
        trigger: "MeJepaCompiler::verify(adversarial patch)".to_string(),
        expected: json!({
            "finalClassification": "non_pass",
            "highConfidenceFalsePasses": 0
        }),
        actual: json!({
            "finalClassification": classification,
            "predictedVerdict": predicted_verdict,
            "calibratedConfidence": calibrated_confidence,
            "failedGate": failed_gate
        }),
        final_classification: classification,
        predicted_verdict,
        calibrated_confidence,
        failed_gate,
        pass,
    })
}

fn classification(verdict: &VerifyVerdict) -> String {
    match verdict {
        VerifyVerdict::Approve { .. } => "Pass".to_string(),
        VerifyVerdict::EscalateToHuman { failed_gate, .. } => format!("{failed_gate:?}"),
    }
}

fn predicted_verdict(verdict: &VerifyVerdict) -> Option<String> {
    match verdict {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => Some(format!("{:?}", reality_prediction.verdict)),
        VerifyVerdict::EscalateToHuman {
            reality_prediction, ..
        } => reality_prediction
            .as_ref()
            .map(|prediction| format!("{:?}", prediction.verdict)),
    }
}

fn calibrated_confidence(verdict: &VerifyVerdict) -> Option<f32> {
    match verdict {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => Some(reality_prediction.calibrated_confidence),
        VerifyVerdict::EscalateToHuman {
            reality_prediction, ..
        } => reality_prediction
            .as_ref()
            .map(|prediction| prediction.calibrated_confidence),
    }
}

fn failed_gate(verdict: &VerifyVerdict) -> Option<String> {
    match verdict {
        VerifyVerdict::Approve { .. } => None,
        VerifyVerdict::EscalateToHuman { failed_gate, .. } => Some(format!("{failed_gate:?}")),
    }
}

fn load_cases(fixture_root: &Path) -> Result<Vec<AdversarialCase>> {
    let mut paths = Vec::new();
    collect_diff_paths(fixture_root, &mut paths)?;
    paths.sort();
    let mut cases = Vec::with_capacity(paths.len());
    for path in paths {
        cases.push(parse_fixture(&path, fixture_root)?);
    }
    Ok(cases)
}

fn collect_diff_paths(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in
        fs::read_dir(root).with_context(|| format!("read fixture dir {}", root.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            collect_diff_paths(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("diff") {
            out.push(path);
        }
    }
    Ok(())
}

fn parse_fixture(path: &Path, fixture_root: &Path) -> Result<AdversarialCase> {
    let diff_text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let first_line = diff_text
        .lines()
        .next()
        .ok_or_else(|| anyhow!("MEJEPA_ADVERSARIAL_EMPTY_DIFF: {}", path.display()))?;
    ensure!(
        first_line.starts_with(HEADER_PREFIX),
        "MEJEPA_ADVERSARIAL_MISSING_HEADER: {}",
        path.display()
    );
    let header = parse_header(&first_line[HEADER_PREFIX.len()..])?;
    let category = AdversarialCategory::parse(
        header
            .get("category")
            .ok_or_else(|| anyhow!("MEJEPA_ADVERSARIAL_MISSING_CATEGORY: {}", path.display()))?,
    )?;
    let parent_category = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("MEJEPA_ADVERSARIAL_BAD_FIXTURE_PATH: {}", path.display()))?;
    ensure!(
        parent_category == category.as_str(),
        "MEJEPA_ADVERSARIAL_CATEGORY_PATH_MISMATCH: {} declared={} parent={}",
        path.display(),
        category.as_str(),
        parent_category
    );
    let target_path = PathBuf::from(
        header
            .get("target")
            .ok_or_else(|| anyhow!("MEJEPA_ADVERSARIAL_MISSING_TARGET: {}", path.display()))?,
    );
    ensure_relative_safe_path(&target_path)?;
    let id = header
        .get("id")
        .ok_or_else(|| anyhow!("MEJEPA_ADVERSARIAL_MISSING_ID: {}", path.display()))?
        .to_string();
    ensure!(!id.trim().is_empty(), "MEJEPA_ADVERSARIAL_EMPTY_ID");
    let scenario = AdversarialScenario::parse(
        header
            .get("scenario")
            .ok_or_else(|| anyhow!("MEJEPA_ADVERSARIAL_MISSING_SCENARIO: {}", path.display()))?,
    )?;
    let body = diff_text.lines().skip(1).collect::<Vec<_>>().join("\n");
    ensure!(
        !body.trim().is_empty(),
        "MEJEPA_ADVERSARIAL_EMPTY_DIFF: {}",
        path.display()
    );
    ensure!(
        path.starts_with(fixture_root),
        "MEJEPA_ADVERSARIAL_FIXTURE_OUTSIDE_ROOT: {}",
        path.display()
    );
    Ok(AdversarialCase {
        id,
        category,
        scenario,
        fixture_path: path.to_path_buf(),
        target_path,
        fixture_sha256: hex::encode(Sha256::digest(diff_text.as_bytes())),
        diff_text,
    })
}

fn parse_header(raw: &str) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for token in raw.split_whitespace() {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| anyhow!("MEJEPA_ADVERSARIAL_BAD_HEADER_TOKEN: {token}"))?;
        ensure!(
            out.insert(key.to_string(), value.to_string()).is_none(),
            "MEJEPA_ADVERSARIAL_DUPLICATE_HEADER_KEY: {key}"
        );
    }
    Ok(out)
}

fn validate_cases(cases: &[AdversarialCase]) -> Result<()> {
    ensure!(
        cases.len() == 30,
        "MEJEPA_ADVERSARIAL_CASE_COUNT: {}",
        cases.len()
    );
    let mut ids = BTreeSet::new();
    for case in cases {
        ensure!(
            ids.insert(case.id.clone()),
            "MEJEPA_ADVERSARIAL_DUPLICATE_CASE: {}",
            case.id
        );
    }
    let counts = corpus_counts(cases);
    for category in [
        AdversarialCategory::BuggyPassing,
        AdversarialCategory::KnownGoodLookalike,
        AdversarialCategory::EmbedderTargeted,
    ] {
        let count = counts.get(category.as_str()).copied().unwrap_or(0);
        ensure!(
            count == 10,
            "MEJEPA_ADVERSARIAL_CATEGORY_COUNT: {}={count}",
            category.as_str()
        );
    }
    Ok(())
}

fn corpus_counts(cases: &[AdversarialCase]) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for case in cases {
        *counts
            .entry(case.category.as_str().to_string())
            .or_insert(0) += 1;
    }
    counts
}

fn boundary_empty_diff(fixture_root: &Path, cases: &[AdversarialCase]) -> Result<BoundaryEvidence> {
    let before = json!({"caseCount": cases.len()});
    let path = fixture_root.join("buggy_passing/empty_boundary.diff");
    let err = parse_fixture_text_at(
        &path,
        fixture_root,
        "# me-jepa-adversarial-case id=empty category=buggy_passing scenario=predicted_failure target=src/empty.py\n",
    )
    .expect_err("empty fixture body must fail closed");
    let after = json!({"caseCount": cases.len()});
    Ok(BoundaryEvidence {
        case: "empty_diff_rejected".to_string(),
        trigger: "parse adversarial fixture with only header".to_string(),
        expected_error_code: "MEJEPA_ADVERSARIAL_EMPTY_DIFF".to_string(),
        actual_error_code: error_code(&err),
        before,
        after,
        pass: error_code(&err) == "MEJEPA_ADVERSARIAL_EMPTY_DIFF",
    })
}

fn boundary_duplicate_case_id(cases: &[AdversarialCase]) -> Result<BoundaryEvidence> {
    let before = json!({"caseCount": cases.len()});
    let mut duplicate = cases.to_vec();
    ensure!(
        duplicate.len() >= 2,
        "MEJEPA_ADVERSARIAL_CASE_COUNT: not enough cases"
    );
    duplicate[1] = duplicate[0].clone();
    let err = validate_cases(&duplicate).expect_err("duplicate case id must fail closed");
    let after = json!({"caseCount": cases.len()});
    Ok(BoundaryEvidence {
        case: "duplicate_case_id_rejected".to_string(),
        trigger: "validate adversarial corpus with duplicate id".to_string(),
        expected_error_code: "MEJEPA_ADVERSARIAL_DUPLICATE_CASE".to_string(),
        actual_error_code: error_code(&err),
        before,
        after,
        pass: error_code(&err) == "MEJEPA_ADVERSARIAL_DUPLICATE_CASE",
    })
}

fn boundary_unknown_category(cases: &[AdversarialCase]) -> Result<BoundaryEvidence> {
    let before = json!({"caseCount": cases.len()});
    let err = AdversarialCategory::parse("unknown").expect_err("unknown category must fail closed");
    let after = json!({"caseCount": cases.len()});
    Ok(BoundaryEvidence {
        case: "unknown_category_rejected".to_string(),
        trigger: "AdversarialCategory::parse(\"unknown\")".to_string(),
        expected_error_code: "MEJEPA_ADVERSARIAL_UNKNOWN_CATEGORY".to_string(),
        actual_error_code: error_code(&err),
        before,
        after,
        pass: error_code(&err) == "MEJEPA_ADVERSARIAL_UNKNOWN_CATEGORY",
    })
}

fn boundary_wrong_category_directory(
    fixture_root: &Path,
    cases: &[AdversarialCase],
) -> Result<BoundaryEvidence> {
    let before = json!({"caseCount": cases.len()});
    let path = fixture_root.join("buggy_passing/wrong_category_boundary.diff");
    let err = parse_fixture_text_at(
        &path,
        fixture_root,
        "# me-jepa-adversarial-case id=wrongdir category=embedder_targeted scenario=constellation_reject target=src/wrongdir.py\n--- a/src/wrongdir.py\n+++ b/src/wrongdir.py\n@@\n+pass\n",
    )
    .expect_err("declared category must match parent directory");
    let after = json!({"caseCount": cases.len()});
    Ok(BoundaryEvidence {
        case: "category_directory_mismatch_rejected".to_string(),
        trigger: "parse fixture whose declared category disagrees with parent directory"
            .to_string(),
        expected_error_code: "MEJEPA_ADVERSARIAL_CATEGORY_PATH_MISMATCH".to_string(),
        actual_error_code: error_code(&err),
        before,
        after,
        pass: error_code(&err) == "MEJEPA_ADVERSARIAL_CATEGORY_PATH_MISMATCH",
    })
}

fn parse_fixture_text_at(path: &Path, fixture_root: &Path, text: &str) -> Result<AdversarialCase> {
    let temp = tempfile::tempdir()?;
    let temp_fixture = temp
        .path()
        .join(path.strip_prefix(fixture_root).unwrap_or(path));
    if let Some(parent) = temp_fixture.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&temp_fixture, text)?;
    let temp_root = temp.path();
    parse_fixture(&temp_fixture, temp_root)
}

fn error_code(err: &anyhow::Error) -> String {
    err.to_string()
        .split(':')
        .next()
        .unwrap_or("MEJEPA_ADVERSARIAL_UNKNOWN")
        .trim()
        .to_string()
}

fn ensure_relative_safe_path(path: &Path) -> Result<()> {
    ensure!(!path.is_absolute(), "MEJEPA_ADVERSARIAL_ABSOLUTE_TARGET");
    ensure!(
        !path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }),
        "MEJEPA_ADVERSARIAL_UNSAFE_TARGET"
    );
    Ok(())
}

fn seed_calibration(db: Arc<DB>) -> Result<CalibrationStore> {
    let calibration_store = CalibrationStore::new(db, 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![0.50],
            actual_test_pass: vec![if idx % 2 == 0 { 1.0 } else { 0.0 }],
        })
        .collect::<Vec<_>>();
    let norm_sq = examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, context_graph_mejepa::MejepaInferError>>()?;
    calibration_store.calibrate(
        &examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::new(),
    )?;
    Ok(calibration_store)
}

fn count_live_predictions(db: &DB) -> Result<BTreeMap<String, u64>> {
    let mut counts = BTreeMap::new();
    counts.insert(
        CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?,
    );
    Ok(counts)
}

fn flush_infer_db(db: &DB) -> Result<()> {
    for cf_name in MEJEPA_INFER_CFS {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
}

fn session_id_for_case(case_id: &str) -> [u8; 16] {
    let digest = Sha256::digest(case_id.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out[0] |= 1;
    out
}

fn fixture_root() -> Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/adversarial")
        .canonicalize()
        .context("canonicalize adversarial fixture root")?;
    ensure!(root.is_dir(), "MEJEPA_ADVERSARIAL_FIXTURE_ROOT_MISSING");
    Ok(root)
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
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
    let allowed = path.starts_with("/zfs/archive/contextgraph/fsv/") || path.starts_with(&temp_dir);
    ensure!(
        allowed,
        "refusing to remove existing adversarial FSV root outside /zfs/archive/contextgraph/fsv or temp dir: {}",
        path.display()
    );
    fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))
}

fn collect_file_artifacts(fsv_root: &Path, fixture_root: &Path) -> Result<Vec<FileArtifact>> {
    let mut out = Vec::new();
    collect_file_artifacts_inner(fsv_root, &mut out)?;
    collect_file_artifacts_inner(fixture_root, &mut out)?;
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
    fn task_test_010_adversarial_corpus_meets_non_pass_gate() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_adversarial_corpus_fsv(&fsv_root).expect("adversarial corpus FSV");
        write_report_with_readback(&report, &report_path).expect("write adversarial FSV report");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert_eq!(report.aggregate.total_cases, 30);
        assert!(report.aggregate.non_pass_rate >= MIN_NON_PASS_RATE);
        assert_eq!(report.aggregate.high_confidence_false_passes, 0);
        assert!(report.aggregate.categories_with_exactly_ten_cases);
        assert!(report.boundary_cases.len() >= 3);
    }
}
