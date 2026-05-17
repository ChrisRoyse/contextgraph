//! Durable adversarial Python corpus rows and fingerprint materialization.
//!
//! TASK-PY-G-060 keeps the 30 adversarial fixture patches in a first-class
//! RocksDB source-of-truth CF. TASK-FP-009 consumes those rows to create
//! canonical KnownBad failure-shape fingerprints.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use context_graph_mejepa_cf::{CF_MEJEPA_ADVERSARIAL_CORPUS, CF_MEJEPA_FAILURE_FINGERPRINTS};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::eval::MutationCategory;
use crate::failure_fingerprint::{
    FailureShapeFingerprint, FingerprintCalibrationState, FingerprintConfidence, FingerprintKind,
    FAILURE_FINGERPRINT_SCHEMA_VERSION,
};
use crate::gates::sha256_bytes;
use crate::types::{ChunkId, EmbedderId, FailureModeClass, OracleOutcome, Verdict};

pub const ADVERSARIAL_CASE_SCHEMA_VERSION: u32 = 1;
pub const ADVERSARIAL_SOURCE_CORPUS: &str = "python-adversarial-v1";
pub const EXPECTED_ADVERSARIAL_CASES: usize = 30;
pub const EXPECTED_ADVERSARIAL_CASES_PER_KIND: usize = 10;
pub const PYTHON_FORWARD_CACHE_EMBEDDERS: [&str; 12] = [
    "e1", "e6", "e7", "e8", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e9",
];

const HEADER_PREFIX: &str = "# me-jepa-adversarial-case ";
const MAX_ID_BYTES: usize = 128;
const MAX_TEXT_BYTES: usize = 512;
const MAX_PATCH_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdversarialKind {
    SubtleBugPassesTests,
    KnownGoodLookalike,
    EmbedderGaming,
}

impl AdversarialKind {
    pub fn all() -> [Self; 3] {
        [
            Self::SubtleBugPassesTests,
            Self::KnownGoodLookalike,
            Self::EmbedderGaming,
        ]
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::SubtleBugPassesTests => "subtle_bug_passes_tests",
            Self::KnownGoodLookalike => "known_good_lookalike",
            Self::EmbedderGaming => "embedder_gaming",
        }
    }

    pub fn from_fixture_category(category: &str) -> Result<Self, MejepaInferError> {
        match category {
            "buggy_passing" => Ok(Self::SubtleBugPassesTests),
            "knowngood_lookalike" => Ok(Self::KnownGoodLookalike),
            "embedder_targeted" => Ok(Self::EmbedderGaming),
            other => invalid(
                "adversarial_case.kind",
                format!("unknown fixture category {other}"),
            ),
        }
    }

    pub fn fixture_category(self) -> &'static str {
        match self {
            Self::SubtleBugPassesTests => "buggy_passing",
            Self::KnownGoodLookalike => "knowngood_lookalike",
            Self::EmbedderGaming => "embedder_targeted",
        }
    }

    pub fn expected_verdict(self) -> Verdict {
        match self {
            Self::SubtleBugPassesTests => Verdict::Fail,
            Self::KnownGoodLookalike => Verdict::Abstain,
            Self::EmbedderGaming => Verdict::GuardRejected,
        }
    }

    pub fn failure_mode_class(self) -> FailureModeClass {
        match self {
            Self::SubtleBugPassesTests => FailureModeClass::OffByOne,
            Self::KnownGoodLookalike => FailureModeClass::ContractViolation,
            Self::EmbedderGaming => FailureModeClass::ApiMisuse,
        }
    }

    pub fn mutation_category(self) -> MutationCategory {
        match self {
            Self::SubtleBugPassesTests => MutationCategory::SubtleFlip,
            Self::KnownGoodLookalike => MutationCategory::OverEngineer,
            Self::EmbedderGaming => MutationCategory::WrongFile,
        }
    }

    pub fn recall_threshold(self) -> f32 {
        match self {
            Self::SubtleBugPassesTests | Self::KnownGoodLookalike => 0.70,
            Self::EmbedderGaming => 0.80,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdversarialCase {
    pub schema_version: u32,
    pub case_id: String,
    pub kind: AdversarialKind,
    pub repo: String,
    pub target_path: PathBuf,
    pub patch_text: String,
    pub patch_sha256: [u8; 32],
    pub oracle_outcome: OracleOutcome,
    pub expected_verdict: Verdict,
    pub expected_high_severity_q4_count: u32,
    pub failure_mode_class: FailureModeClass,
    pub source_corpus: String,
    pub fixture_path: Option<String>,
}

impl AdversarialCase {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != ADVERSARIAL_CASE_SCHEMA_VERSION {
            return invalid(
                "adversarial_case.schema_version",
                format!(
                    "expected schema version {ADVERSARIAL_CASE_SCHEMA_VERSION}; got {}",
                    self.schema_version
                ),
            );
        }
        validate_text("adversarial_case.case_id", &self.case_id, MAX_ID_BYTES)?;
        validate_text("adversarial_case.repo", &self.repo, MAX_TEXT_BYTES)?;
        validate_text(
            "adversarial_case.source_corpus",
            &self.source_corpus,
            MAX_TEXT_BYTES,
        )?;
        if self.source_corpus != ADVERSARIAL_SOURCE_CORPUS {
            return invalid(
                "adversarial_case.source_corpus",
                format!(
                    "expected {ADVERSARIAL_SOURCE_CORPUS}; got {}",
                    self.source_corpus
                ),
            );
        }
        validate_relative_safe_path("adversarial_case.target_path", &self.target_path)?;
        validate_patch_text("adversarial_case.patch_text", &self.patch_text)?;
        let expected_sha = sha256_bytes(self.patch_text.as_bytes());
        if self.patch_sha256 != expected_sha {
            return invalid(
                "adversarial_case.patch_sha256",
                "patch sha256 does not match patch_text",
            );
        }
        if self.oracle_outcome != OracleOutcome::Fail {
            return invalid(
                "adversarial_case.oracle_outcome",
                "adversarial cases must be oracle-failing",
            );
        }
        if matches!(
            self.expected_verdict,
            Verdict::Pass | Verdict::OutOfDistribution
        ) {
            return invalid(
                "adversarial_case.expected_verdict",
                "expected verdict must be Fail, Abstain, or GuardRejected",
            );
        }
        if self.expected_verdict != self.kind.expected_verdict() {
            return invalid(
                "adversarial_case.expected_verdict",
                format!(
                    "expected {:?} for kind {}; got {:?}",
                    self.kind.expected_verdict(),
                    self.kind.slug(),
                    self.expected_verdict
                ),
            );
        }
        if self.failure_mode_class != self.kind.failure_mode_class() {
            return invalid(
                "adversarial_case.failure_mode_class",
                format!(
                    "expected {:?} for kind {}; got {:?}",
                    self.kind.failure_mode_class(),
                    self.kind.slug(),
                    self.failure_mode_class
                ),
            );
        }
        if self.expected_high_severity_q4_count == 0 {
            return invalid(
                "adversarial_case.expected_high_severity_q4_count",
                "adversarial cases must carry at least one high-severity Q4 side effect",
            );
        }
        if let Some(path) = &self.fixture_path {
            validate_text("adversarial_case.fixture_path", path, MAX_TEXT_BYTES)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdversarialKindRecallMetrics {
    pub kind: AdversarialKind,
    pub total_count: u64,
    pub correct_count: u64,
    pub recall: f32,
    pub threshold: f32,
    pub passed_threshold: bool,
}

impl AdversarialKindRecallMetrics {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.total_count == 0 {
            return invalid("adversarial_kind_recall.total_count", "must be non-zero");
        }
        if self.correct_count > self.total_count {
            return invalid(
                "adversarial_kind_recall.correct_count",
                "cannot exceed total_count",
            );
        }
        validate_probability("adversarial_kind_recall.recall", self.recall)?;
        validate_probability("adversarial_kind_recall.threshold", self.threshold)?;
        let expected = self.correct_count as f32 / self.total_count as f32;
        if (self.recall - expected).abs() > 0.000_01 {
            return invalid(
                "adversarial_kind_recall.recall",
                "recall does not match correct_count / total_count",
            );
        }
        if self.threshold != self.kind.recall_threshold() {
            return invalid(
                "adversarial_kind_recall.threshold",
                "threshold does not match adversarial kind",
            );
        }
        if self.passed_threshold != (self.recall >= self.threshold) {
            return invalid(
                "adversarial_kind_recall.passed_threshold",
                "passed_threshold does not match recall threshold",
            );
        }
        Ok(())
    }
}

pub fn load_adversarial_fixture_cases(
    fixture_root: &Path,
) -> Result<Vec<AdversarialCase>, MejepaInferError> {
    let mut paths = Vec::new();
    collect_diff_paths(fixture_root, &mut paths)?;
    paths.sort();
    let mut cases = Vec::with_capacity(paths.len());
    for path in paths {
        cases.push(parse_adversarial_fixture(fixture_root, &path)?);
    }
    validate_adversarial_corpus(&cases)?;
    Ok(cases)
}

pub fn parse_adversarial_fixture(
    fixture_root: &Path,
    path: &Path,
) -> Result<AdversarialCase, MejepaInferError> {
    let diff_text = fs::read_to_string(path)
        .map_err(|err| MejepaInferError::io("read", path.to_path_buf(), err))?;
    parse_adversarial_fixture_text(fixture_root, path, &diff_text)
}

pub fn parse_adversarial_fixture_text(
    fixture_root: &Path,
    path: &Path,
    diff_text: &str,
) -> Result<AdversarialCase, MejepaInferError> {
    let first_line = diff_text
        .lines()
        .next()
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "adversarial_fixture.header".to_string(),
            detail: "MEJEPA_ADVERSARIAL_EMPTY_DIFF".to_string(),
        })?;
    if !first_line.starts_with(HEADER_PREFIX) {
        return invalid(
            "adversarial_fixture.header",
            "MEJEPA_ADVERSARIAL_MISSING_HEADER",
        );
    }
    let header = parse_header(&first_line[HEADER_PREFIX.len()..])?;
    let id = header_value(&header, "id")?.to_string();
    let fixture_category = header_value(&header, "category")?;
    let kind = AdversarialKind::from_fixture_category(fixture_category)?;
    let scenario = header_value(&header, "scenario")?;
    validate_text("adversarial_fixture.scenario", scenario, MAX_TEXT_BYTES)?;
    let target_path = PathBuf::from(header_value(&header, "target")?);
    validate_relative_safe_path("adversarial_fixture.target", &target_path)?;
    let parent_category = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "adversarial_fixture.path".to_string(),
            detail: "MEJEPA_ADVERSARIAL_BAD_FIXTURE_PATH".to_string(),
        })?;
    if parent_category != kind.fixture_category() {
        return invalid(
            "adversarial_fixture.category",
            "MEJEPA_ADVERSARIAL_CATEGORY_PATH_MISMATCH",
        );
    }
    if !path.starts_with(fixture_root) {
        return invalid(
            "adversarial_fixture.path",
            "MEJEPA_ADVERSARIAL_FIXTURE_OUTSIDE_ROOT",
        );
    }
    let body = diff_text.lines().skip(1).collect::<Vec<_>>().join("\n");
    if body.trim().is_empty() {
        return invalid("adversarial_fixture.body", "MEJEPA_ADVERSARIAL_EMPTY_DIFF");
    }
    let case = AdversarialCase {
        schema_version: ADVERSARIAL_CASE_SCHEMA_VERSION,
        repo: format!("{ADVERSARIAL_SOURCE_CORPUS}/{id}"),
        case_id: id,
        kind,
        target_path,
        patch_sha256: sha256_bytes(diff_text.as_bytes()),
        patch_text: diff_text.to_string(),
        oracle_outcome: OracleOutcome::Fail,
        expected_verdict: kind.expected_verdict(),
        expected_high_severity_q4_count: 1,
        failure_mode_class: kind.failure_mode_class(),
        source_corpus: ADVERSARIAL_SOURCE_CORPUS.to_string(),
        fixture_path: Some(path.display().to_string()),
    };
    case.validate()?;
    Ok(case)
}

pub fn validate_adversarial_corpus(cases: &[AdversarialCase]) -> Result<(), MejepaInferError> {
    if cases.len() != EXPECTED_ADVERSARIAL_CASES {
        return invalid(
            "adversarial_corpus.case_count",
            format!(
                "expected {EXPECTED_ADVERSARIAL_CASES} adversarial cases; got {}",
                cases.len()
            ),
        );
    }
    let mut ids = BTreeSet::new();
    let mut counts: BTreeMap<AdversarialKind, usize> = BTreeMap::new();
    for case in cases {
        case.validate()?;
        if !ids.insert(case.case_id.clone()) {
            return invalid(
                "adversarial_corpus.case_id",
                format!("duplicate case id {}", case.case_id),
            );
        }
        *counts.entry(case.kind).or_insert(0) += 1;
    }
    for kind in AdversarialKind::all() {
        let count = counts.get(&kind).copied().unwrap_or(0);
        if count != EXPECTED_ADVERSARIAL_CASES_PER_KIND {
            return invalid(
                "adversarial_corpus.kind_count",
                format!(
                    "expected {EXPECTED_ADVERSARIAL_CASES_PER_KIND} cases for {}; got {count}",
                    kind.slug()
                ),
            );
        }
    }
    Ok(())
}

pub fn adversarial_case_key(case_id: &str) -> Result<Vec<u8>, MejepaInferError> {
    validate_text("adversarial_case.case_id", case_id, MAX_ID_BYTES)?;
    Ok(format!("{ADVERSARIAL_SOURCE_CORPUS}:{case_id}").into_bytes())
}

pub fn write_adversarial_case_sync_readback(
    db: &DB,
    case: &AdversarialCase,
) -> Result<(), MejepaInferError> {
    case.validate()?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_ADVERSARIAL_CORPUS,
        &adversarial_case_key(&case.case_id)?,
        case,
    )
}

pub fn read_adversarial_case(
    db: &DB,
    case_id: &str,
) -> Result<Option<AdversarialCase>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_ADVERSARIAL_CORPUS)?;
    db.get_cf(cf, adversarial_case_key(case_id)?)?
        .map(|bytes| bincode::deserialize(&bytes).map_err(Into::into))
        .transpose()
}

pub fn read_adversarial_corpus(db: &DB) -> Result<Vec<AdversarialCase>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_ADVERSARIAL_CORPUS)?;
    let mut cases = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let case: AdversarialCase = bincode::deserialize(&value)?;
        case.validate()?;
        cases.push(case);
    }
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    Ok(cases)
}

pub fn materialize_adversarial_fingerprint(
    case: &AdversarialCase,
    centroid_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
    frozen_at_unix_ms: i64,
) -> Result<FailureShapeFingerprint, MejepaInferError> {
    case.validate()?;
    validate_adversarial_centroids(&case.case_id, &centroid_by_embedder)?;
    if frozen_at_unix_ms <= 0 {
        return adversarial_materialization_failed(
            &case.case_id,
            "frozen_at_unix_ms must be positive",
        );
    }
    let kind = FingerprintKind::KnownBad {
        repo: case.repo.clone(),
        mutation_category: case.kind.mutation_category(),
        failure_mode: case.failure_mode_class,
        exception_class: None,
    };
    let fingerprint_id = FailureShapeFingerprint::canonical_id(&kind, &case.source_corpus)
        .map_err(|err| adversarial_materialization_error(&case.case_id, err))?;
    let variance_by_embedder = centroid_by_embedder
        .keys()
        .cloned()
        .map(|embedder| (embedder, 0.0_f32))
        .collect::<BTreeMap<_, _>>();
    let tau_by_embedder = centroid_by_embedder
        .keys()
        .cloned()
        .map(|embedder| (embedder, 0.95_f32))
        .collect::<BTreeMap<_, _>>();
    let fingerprint = FailureShapeFingerprint {
        schema_version: FAILURE_FINGERPRINT_SCHEMA_VERSION,
        fingerprint_id,
        kind,
        name: format!("adversarial:{}:{}", case.kind.slug(), case.case_id),
        source_corpus: case.source_corpus.clone(),
        source_manifest_sha256: Some(case.patch_sha256),
        centroid_by_embedder,
        variance_by_embedder,
        tau_by_embedder,
        pairwise_cosine: Vec::new(),
        pairwise_mutual_information: Vec::new(),
        reference_chunks: vec![ChunkId(format!(
            "{ADVERSARIAL_SOURCE_CORPUS}:{}:patch",
            case.case_id
        ))],
        n_references: 1,
        oracle_outcome: Some(OracleOutcome::Fail),
        is_canonical: true,
        frozen_at_unix_ms,
        confidence: FingerprintConfidence {
            classification_accuracy: Some(1.0),
            classification_precision: Some(1.0),
            unknown_recall: None,
            calibration_observations: 1,
            calibration_state: FingerprintCalibrationState::Calibrated,
        },
    };
    fingerprint
        .validate()
        .map_err(|err| adversarial_materialization_error(&case.case_id, err))?;
    Ok(fingerprint)
}

pub fn deterministic_adversarial_centroids(
    case: &AdversarialCase,
) -> BTreeMap<EmbedderId, Vec<f32>> {
    PYTHON_FORWARD_CACHE_EMBEDDERS
        .iter()
        .map(|embedder| {
            (
                EmbedderId((*embedder).to_string()),
                deterministic_unit_vector(&case.patch_sha256, embedder),
            )
        })
        .collect()
}

pub fn adversarial_kind_recall_metrics(
    cases: &[AdversarialCase],
    correct_case_ids: &BTreeSet<String>,
) -> Result<BTreeMap<AdversarialKind, AdversarialKindRecallMetrics>, MejepaInferError> {
    validate_adversarial_corpus(cases)?;
    let mut totals: BTreeMap<AdversarialKind, u64> = BTreeMap::new();
    let mut correct: BTreeMap<AdversarialKind, u64> = BTreeMap::new();
    for case in cases {
        *totals.entry(case.kind).or_insert(0) += 1;
        if correct_case_ids.contains(&case.case_id) {
            *correct.entry(case.kind).or_insert(0) += 1;
        }
    }
    let mut out = BTreeMap::new();
    for kind in AdversarialKind::all() {
        let total_count = totals.get(&kind).copied().unwrap_or(0);
        let correct_count = correct.get(&kind).copied().unwrap_or(0);
        let recall = correct_count as f32 / total_count.max(1) as f32;
        let threshold = kind.recall_threshold();
        let metrics = AdversarialKindRecallMetrics {
            kind,
            total_count,
            correct_count,
            recall,
            threshold,
            passed_threshold: recall >= threshold,
        };
        metrics.validate()?;
        out.insert(kind, metrics);
    }
    Ok(out)
}

pub fn write_adversarial_patch_artifacts(
    cases: &[AdversarialCase],
    corpus_root: &Path,
) -> Result<Vec<PathBuf>, MejepaInferError> {
    validate_adversarial_corpus(cases)?;
    let mut written = Vec::with_capacity(cases.len());
    for case in cases {
        let path = corpus_root.join(&case.case_id).join("patch.diff");
        write_file_readback(&path, case.patch_text.as_bytes())?;
        written.push(path);
    }
    Ok(written)
}

pub fn fingerprint_count_for_source(
    db: &DB,
    source_corpus: &str,
) -> Result<usize, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_FAILURE_FINGERPRINTS)?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let fingerprint: FailureShapeFingerprint = bincode::deserialize(&value)?;
        fingerprint.validate()?;
        if fingerprint.source_corpus == source_corpus {
            count += 1;
        }
    }
    Ok(count)
}

fn collect_diff_paths(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), MejepaInferError> {
    let entries = fs::read_dir(root)
        .map_err(|err| MejepaInferError::io("read_dir", root.to_path_buf(), err))?;
    for entry in entries {
        let path = entry
            .map_err(|err| MejepaInferError::io("read_dir_entry", root.to_path_buf(), err))?
            .path();
        if path.is_dir() {
            collect_diff_paths(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("diff") {
            out.push(path);
        }
    }
    Ok(())
}

fn parse_header(raw: &str) -> Result<BTreeMap<String, String>, MejepaInferError> {
    let mut out = BTreeMap::new();
    for token in raw.split_whitespace() {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "adversarial_fixture.header".to_string(),
                detail: format!("MEJEPA_ADVERSARIAL_BAD_HEADER_TOKEN: {token}"),
            })?;
        validate_text("adversarial_fixture.header.key", key, MAX_ID_BYTES)?;
        validate_text("adversarial_fixture.header.value", value, MAX_TEXT_BYTES)?;
        if out.insert(key.to_string(), value.to_string()).is_some() {
            return invalid(
                "adversarial_fixture.header",
                format!("MEJEPA_ADVERSARIAL_DUPLICATE_HEADER_KEY: {key}"),
            );
        }
    }
    Ok(out)
}

fn header_value<'a>(
    header: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, MejepaInferError> {
    header
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "adversarial_fixture.header".to_string(),
            detail: format!("MEJEPA_ADVERSARIAL_MISSING_{}", key.to_uppercase()),
        })
}

fn validate_adversarial_centroids(
    case_id: &str,
    centroid_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<(), MejepaInferError> {
    if centroid_by_embedder.len() != PYTHON_FORWARD_CACHE_EMBEDDERS.len() {
        return adversarial_materialization_failed(
            case_id,
            format!(
                "expected {} embedders; got {}",
                PYTHON_FORWARD_CACHE_EMBEDDERS.len(),
                centroid_by_embedder.len()
            ),
        );
    }
    let expected = PYTHON_FORWARD_CACHE_EMBEDDERS
        .iter()
        .map(|slug| EmbedderId((*slug).to_string()))
        .collect::<BTreeSet<_>>();
    let actual = centroid_by_embedder
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return adversarial_materialization_failed(
            case_id,
            format!("embedder set mismatch: expected {expected:?} got {actual:?}"),
        );
    }
    for (embedder, vector) in centroid_by_embedder {
        if vector.is_empty() {
            return adversarial_materialization_failed(
                case_id,
                format!("empty vector for embedder {}", embedder.0),
            );
        }
        if vector.iter().any(|value| !value.is_finite()) {
            return adversarial_materialization_failed(
                case_id,
                format!("non-finite vector for embedder {}", embedder.0),
            );
        }
    }
    Ok(())
}

fn deterministic_unit_vector(seed: &[u8; 32], embedder: &str) -> Vec<f32> {
    let mut input = Vec::with_capacity(seed.len() + embedder.len());
    input.extend_from_slice(seed);
    input.extend_from_slice(embedder.as_bytes());
    let digest = sha256_bytes(&input);
    let mut vector = (0..4)
        .map(|idx| {
            let byte = digest[idx] as f32 / 255.0;
            byte.mul_add(2.0, -1.0)
        })
        .collect::<Vec<_>>();
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm <= f32::EPSILON {
        vector[0] = 1.0;
        return vector;
    }
    for value in &mut vector {
        *value /= norm;
    }
    vector
}

fn validate_text(field: &str, value: &str, max_bytes: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if value.len() > max_bytes {
        return invalid(field, format!("exceeds {max_bytes} bytes"));
    }
    if value.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return invalid(field, "contains a control character");
    }
    Ok(())
}

fn validate_patch_text(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if value.len() > MAX_PATCH_BYTES {
        return invalid(field, format!("exceeds {MAX_PATCH_BYTES} bytes"));
    }
    if value.bytes().any(|byte| byte == 0) {
        return invalid(field, "contains NUL byte");
    }
    Ok(())
}

fn validate_probability(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(field, "must be a finite probability in 0..=1");
    }
    Ok(())
}

fn validate_relative_safe_path(field: &str, path: &Path) -> Result<(), MejepaInferError> {
    if path.as_os_str().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if path.is_absolute() {
        return invalid(field, "MEJEPA_ADVERSARIAL_ABSOLUTE_TARGET");
    }
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                validate_text(field, &value.to_string_lossy(), MAX_TEXT_BYTES)?;
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return invalid(field, "MEJEPA_ADVERSARIAL_UNSAFE_TARGET");
            }
        }
    }
    Ok(())
}

fn write_file_readback(path: &Path, bytes: &[u8]) -> Result<(), MejepaInferError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| MejepaInferError::io("create_dir_all", parent.to_path_buf(), err))?;
    }
    fs::write(path, bytes).map_err(|err| MejepaInferError::io("write", path.to_path_buf(), err))?;
    let readback =
        fs::read(path).map_err(|err| MejepaInferError::io("read", path.to_path_buf(), err))?;
    if readback != bytes {
        return invalid(
            "adversarial_patch_artifact",
            format!("readback mismatch for {}", path.display()),
        );
    }
    Ok(())
}

fn write_value_sync_readback<T>(
    db: &DB,
    cf_name: &str,
    key: &[u8],
    value: &T,
) -> Result<(), MejepaInferError>
where
    T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let cf = cf(db, cf_name)?;
    let bytes = bincode::serialize(value)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    db.flush_cf(cf)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: cf_name.to_string(),
            detail: "sync write readback returned no row".to_string(),
        })?;
    if readback != bytes {
        return invalid(
            cf_name,
            "sync write readback bytes differ from encoded input",
        );
    }
    let decoded: T = bincode::deserialize(&readback)?;
    if decoded != *value {
        return invalid(
            cf_name,
            format!("sync write readback decoded value differs: {decoded:?}"),
        );
    }
    Ok(())
}

fn adversarial_materialization_failed<T>(
    case_id: &str,
    reason: impl Into<String>,
) -> Result<T, MejepaInferError> {
    Err(
        MejepaInferError::AdversarialFingerprintMaterializationFailed {
            case_id: case_id.to_string(),
            reason: reason.into(),
        },
    )
}

fn adversarial_materialization_error(case_id: &str, err: MejepaInferError) -> MejepaInferError {
    MejepaInferError::AdversarialFingerprintMaterializationFailed {
        case_id: case_id.to_string(),
        reason: err.to_string(),
    }
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adversarial_fixture_parser_rejects_unsafe_target() {
        let err = parse_adversarial_fixture_text(
            Path::new("/tmp/fixtures"),
            Path::new("/tmp/fixtures/buggy_passing/bad.diff"),
            "# me-jepa-adversarial-case id=bad category=buggy_passing scenario=predicted_failure target=../bad.py\n--- a/x\n+++ b/x\n@@\n+pass\n",
        )
        .expect_err("unsafe target must fail closed");
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");
        assert!(err.to_string().contains("MEJEPA_ADVERSARIAL_UNSAFE_TARGET"));
    }

    #[test]
    fn deterministic_adversarial_fingerprint_is_stable() {
        let text = "# me-jepa-adversarial-case id=buggy_passing_99 category=buggy_passing scenario=predicted_failure target=src/x.py\n--- a/src/x.py\n+++ b/src/x.py\n@@\n+return len(items) - 1\n";
        let case = parse_adversarial_fixture_text(
            Path::new("/tmp/fixtures"),
            Path::new("/tmp/fixtures/buggy_passing/buggy_passing_99.diff"),
            text,
        )
        .expect("parse fixture");
        let left = materialize_adversarial_fingerprint(
            &case,
            deterministic_adversarial_centroids(&case),
            1_779_030_000_000,
        )
        .expect("left fingerprint");
        let right = materialize_adversarial_fingerprint(
            &case,
            deterministic_adversarial_centroids(&case),
            1_779_030_000_000,
        )
        .expect("right fingerprint");
        assert_eq!(left.fingerprint_id, right.fingerprint_id);
        assert_eq!(left.source_corpus, ADVERSARIAL_SOURCE_CORPUS);
        assert_eq!(left.centroid_by_embedder.len(), 12);
    }
}
