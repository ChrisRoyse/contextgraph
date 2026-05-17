//! TASK-FP-010 matched-fingerprint live prediction FSV.
//!
//! Proves `RealityPrediction` rows can carry durable failure-shape fingerprint
//! evidence, including compiler-populated Pass/Fail/Abstain matches and
//! persisted OOD/GuardRejected edge semantics.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::compiler::OracleHead as OracleHeadTrait;
use context_graph_mejepa::{
    open_infer_rocksdb, write_fingerprint_sync_readback, AstDiff, CalibrationRecord,
    CalibrationStore, ConformalInterval, ConformalSet, DeterministicConstellationGuard, EmbedderId,
    FailureModeClass, FailureShapeFingerprint, FingerprintCalibrationState, FingerprintConfidence,
    FingerprintKind, FrozenTarget, Language, MeJepaCompiler, MeJepaInferConfig, MejepaStore,
    MutationCategory, OracleOutcome, OracleScores, PatchBundle, PatchWitnessReader,
    PredictionProvenance, Predictor, RealityPrediction, RealityPredictionBuilder,
    RocksDbInferStore, TaskContext, TaskEnvironment, TaskId, TestId, Verdict, WitnessHash,
    FAILURE_FINGERPRINT_SCHEMA_VERSION,
};
use context_graph_mejepa_instruments::{ExceptionClass, InstrumentSlot, Panel, PANEL_DIM};
use serde::Serialize;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-fp-010-matched-fingerprint-live-fsv";
const EVIDENCE_DIM: usize = 384;

#[derive(Debug, Serialize)]
struct CaseRecord {
    case: &'static str,
    verdict: Verdict,
    matched_kind_slug: Option<String>,
    unknown_candidate_present: bool,
    persisted_readback_equal: bool,
    pass: bool,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    fsv_root: String,
    output_path: String,
    started_at_unix_ms: i64,
    cases: Vec<CaseRecord>,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    fs::create_dir_all(&run_root).context("create FSV root")?;
    let output_path = run_root.join("matched_fingerprint_live_fsv.json");

    let cases = vec![
        compile_case(
            &run_root,
            "known_good_pass",
            vec![1.0, 0.0],
            vec![fingerprint(
                FingerprintKind::KnownGood {
                    repo: Some("django__django".to_string()),
                    gold_patch_count: 2,
                },
                vec![1.0, 0.0],
            )?],
            0.95,
            0.95,
            "return 1",
            Verdict::Pass,
            Some("known_good"),
        )?,
        compile_case(
            &run_root,
            "known_bad_fail",
            vec![0.0, 1.0],
            vec![fingerprint(
                FingerprintKind::KnownBad {
                    repo: "django__django".to_string(),
                    mutation_category: MutationCategory::OffByOne,
                    failure_mode: FailureModeClass::OffByOne,
                    exception_class: Some(ExceptionClass::AssertionError),
                },
                vec![0.0, 1.0],
            )?],
            0.20,
            0.20,
            "return 0",
            Verdict::Fail,
            Some("known_bad"),
        )?,
        compile_case(
            &run_root,
            "conflict_abstain",
            vec![1.0, 0.0],
            vec![
                fingerprint(
                    FingerprintKind::KnownGood {
                        repo: Some("django__django".to_string()),
                        gold_patch_count: 2,
                    },
                    vec![1.0, 0.0],
                )?,
                fingerprint(
                    FingerprintKind::KnownBad {
                        repo: "django__django".to_string(),
                        mutation_category: MutationCategory::SubtleFlip,
                        failure_mode: FailureModeClass::WrongAlgorithm,
                        exception_class: Some(ExceptionClass::AssertionError),
                    },
                    vec![1.0, 0.0],
                )?,
            ],
            0.95,
            0.95,
            "<<<<<<< HEAD\nreturn 1\n=======\nreturn 2\n>>>>>>> branch",
            Verdict::Abstain,
            Some("*"),
        )?,
        compile_case(
            &run_root,
            "catalog_empty",
            vec![1.0, 0.0],
            Vec::new(),
            0.95,
            0.95,
            "return 1",
            Verdict::Pass,
            None,
        )?,
        compile_explicit_observation_case(&run_root)?,
        direct_edge_case(&run_root, "unknown_ood", Verdict::OutOfDistribution, true)?,
        direct_edge_case(&run_root, "guard_rejected", Verdict::GuardRejected, false)?,
    ];

    let all_passed = cases.iter().all(|case| case.pass);
    let report = FsvReport {
        fsv_root: FSV_ROOT.to_string(),
        output_path: output_path.display().to_string(),
        started_at_unix_ms,
        cases,
        all_passed,
    };
    fs::write(&output_path, serde_json::to_vec_pretty(&report)?)
        .with_context(|| format!("write {}", output_path.display()))?;
    println!("{}", output_path.display());
    if !all_passed {
        bail!("matched fingerprint live FSV failed");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compile_case(
    run_root: &Path,
    case: &'static str,
    observation2: Vec<f32>,
    catalog: Vec<FailureShapeFingerprint>,
    oracle_pass: f32,
    test_pass: f32,
    patch_after: &str,
    expected_verdict: Verdict,
    expected_kind_slug: Option<&str>,
) -> Result<CaseRecord> {
    let db_path = run_root.join(case).join("rocks");
    let db = open_infer_rocksdb(&db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    calibration.persist(&calibration_record())?;
    for fingerprint in &catalog {
        write_fingerprint_sync_readback(db.as_ref(), fingerprint)?;
    }

    let store = RocksDbInferStore::new(db.clone());
    let compiler = MeJepaCompiler::new(
        MeJepaInferConfig {
            pause_state_path: None,
            bootstrap_delta_omega: 1.0,
            bootstrap_delta_xi: 1.0,
            p_test_threshold: 0.70,
            ..MeJepaInferConfig::default()
        },
        Arc::new(FixedPredictor(panel_with_evidence(observation2, 0)?)),
        Arc::new(FixedTarget(panel_with_evidence(vec![1.0, 0.0], 0)?)),
        Arc::new(FixedOracle {
            oracle_pass,
            test_pass,
        }),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        Arc::new(store.clone()),
        calibration,
        run_root.to_path_buf(),
    )?;
    let (patch, context) = patch_context(run_root, case, patch_after)?;
    let prediction = compiler.compile(&patch, &context)?;
    MejepaStore::write_live_prediction(&store, &prediction)?;
    let readback = MejepaStore::read_live_predictions(&store, prediction.session_id, 1)?
        .into_iter()
        .next()
        .context("missing persisted prediction readback")?;

    let matched_kind_slug = prediction
        .matched_fingerprint
        .as_ref()
        .map(|matched| matched.kind_slug.clone());
    let expected_kind_matches = match expected_kind_slug {
        Some("*") => matched_kind_slug.is_some(),
        Some(expected) => matched_kind_slug.as_deref() == Some(expected),
        None => matched_kind_slug.is_none(),
    };
    let pass = prediction.verdict == expected_verdict
        && expected_kind_matches
        && prediction.unknown_candidate_id.is_none()
        && readback == prediction;
    Ok(CaseRecord {
        case,
        verdict: prediction.verdict,
        matched_kind_slug,
        unknown_candidate_present: prediction.unknown_candidate_id.is_some(),
        persisted_readback_equal: readback == prediction,
        pass,
    })
}

fn direct_edge_case(
    run_root: &Path,
    case: &'static str,
    verdict: Verdict,
    unknown_candidate: bool,
) -> Result<CaseRecord> {
    let db_path = run_root.join(case).join("rocks");
    let db = open_infer_rocksdb(&db_path)?;
    let store = RocksDbInferStore::new(db);
    let mut prediction = base_prediction(case, verdict)?;
    if unknown_candidate {
        prediction.unknown_candidate_id = Some([case.len() as u8; 16]);
    }
    let prediction = RealityPrediction::try_new(prediction)?;
    MejepaStore::write_live_prediction(&store, &prediction)?;
    let readback = MejepaStore::read_live_predictions(&store, prediction.session_id, 1)?
        .into_iter()
        .next()
        .context("missing persisted direct-edge readback")?;
    let pass = readback == prediction
        && prediction.matched_fingerprint.is_none()
        && prediction.unknown_candidate_id.is_some() == unknown_candidate;
    Ok(CaseRecord {
        case,
        verdict: prediction.verdict,
        matched_kind_slug: None,
        unknown_candidate_present: prediction.unknown_candidate_id.is_some(),
        persisted_readback_equal: readback == prediction,
        pass,
    })
}

fn compile_explicit_observation_case(run_root: &Path) -> Result<CaseRecord> {
    let case = "explicit_forward_cache_observation";
    let db_path = run_root.join(case).join("rocks");
    let db = open_infer_rocksdb(&db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    calibration.persist(&calibration_record())?;
    let fingerprint = fingerprint_with_embedder(
        FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        },
        vec![1.0, 0.0],
        "e1",
    )?;
    write_fingerprint_sync_readback(db.as_ref(), &fingerprint)?;

    let store = RocksDbInferStore::new(db.clone());
    let compiler = MeJepaCompiler::new(
        MeJepaInferConfig {
            pause_state_path: None,
            bootstrap_delta_omega: 1.0,
            bootstrap_delta_xi: 1.0,
            p_test_threshold: 0.70,
            ..MeJepaInferConfig::default()
        },
        Arc::new(FixedPredictor(panel_with_evidence(vec![1.0, 0.0], 0)?)),
        Arc::new(FixedTarget(panel_with_evidence(vec![1.0, 0.0], 0)?)),
        Arc::new(FixedOracle {
            oracle_pass: 0.95,
            test_pass: 0.95,
        }),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        Arc::new(store.clone()),
        calibration,
        run_root.to_path_buf(),
    )?;
    let (patch, context) = patch_context(run_root, case, "return 1")?;
    let observation_by_embedder = BTreeMap::from([(
        EmbedderId("e1".to_string()),
        expand_evidence(vec![1.0, 0.0]),
    )]);
    let prediction = compiler.compile_with_fingerprint_observation(
        &patch,
        &context,
        &observation_by_embedder,
    )?;
    MejepaStore::write_live_prediction(&store, &prediction)?;
    let readback = MejepaStore::read_live_predictions(&store, prediction.session_id, 1)?
        .into_iter()
        .next()
        .context("missing persisted explicit-observation readback")?;
    let matched_kind_slug = prediction
        .matched_fingerprint
        .as_ref()
        .map(|matched| matched.kind_slug.clone());
    let pass = prediction.verdict == Verdict::Pass
        && matched_kind_slug.as_deref() == Some("known_good")
        && prediction.unknown_candidate_id.is_none()
        && readback == prediction;
    Ok(CaseRecord {
        case,
        verdict: prediction.verdict,
        matched_kind_slug,
        unknown_candidate_present: prediction.unknown_candidate_id.is_some(),
        persisted_readback_equal: readback == prediction,
        pass,
    })
}

#[derive(Clone)]
struct FixedPredictor(Panel);

impl Predictor for FixedPredictor {
    fn predict(
        &self,
        _panel_t0: &Panel,
        _panel_t1: &Panel,
    ) -> Result<Panel, context_graph_mejepa::MejepaInferError> {
        Ok(self.0.clone())
    }
}

#[derive(Clone)]
struct FixedTarget(Panel);

impl FrozenTarget for FixedTarget {
    fn target(&self, _panel_t2: &Panel) -> Result<Panel, context_graph_mejepa::MejepaInferError> {
        Ok(self.0.clone())
    }
}

struct FixedOracle {
    oracle_pass: f32,
    test_pass: f32,
}

impl OracleHeadTrait for FixedOracle {
    fn score(
        &self,
        _predicted_panel: &Panel,
    ) -> Result<OracleScores, context_graph_mejepa::MejepaInferError> {
        Ok(OracleScores {
            predicted_oracle_pass: self.oracle_pass,
            predicted_test_pass: vec![self.test_pass],
            predicted_runtime_trace: [0.001; 32],
            granger_attestations: BTreeMap::from([("task_fp_010_fsv".to_string(), 1.0)]),
        })
    }
}

fn panel_with_evidence(observation2: Vec<f32>, scenario: u8) -> Result<Panel> {
    let mut data = vec![0.0; PANEL_DIM];
    let e_ast_offset = InstrumentSlot::EAst.offset();
    for (idx, value) in expand_evidence(observation2).into_iter().enumerate() {
        data[e_ast_offset + idx] = value;
    }
    data[InstrumentSlot::EDiff.offset()] = scenario as f32;
    let filled_mask = (1u16 << InstrumentSlot::all().len()) - 1;
    Ok(Panel::try_new(data, filled_mask)?)
}

fn expand_evidence(values: Vec<f32>) -> Vec<f32> {
    let mut out = vec![0.0; EVIDENCE_DIM];
    for (idx, value) in values.into_iter().enumerate() {
        out[idx] = value;
    }
    out
}

fn fingerprint(kind: FingerprintKind, centroid2: Vec<f32>) -> Result<FailureShapeFingerprint> {
    fingerprint_with_embedder(kind, centroid2, "e_ast")
}

fn fingerprint_with_embedder(
    kind: FingerprintKind,
    centroid2: Vec<f32>,
    embedder_id: &str,
) -> Result<FailureShapeFingerprint> {
    let source_corpus = "task-fp-010-fsv";
    let fingerprint_id = FailureShapeFingerprint::canonical_id(&kind, source_corpus)?;
    let oracle_outcome = match &kind {
        FingerprintKind::KnownGood { .. } => Some(OracleOutcome::Pass),
        FingerprintKind::KnownBad { .. } => Some(OracleOutcome::Fail),
        FingerprintKind::Unknown { .. } => None,
    };
    let embedder = EmbedderId(embedder_id.to_string());
    let fingerprint = FailureShapeFingerprint {
        schema_version: FAILURE_FINGERPRINT_SCHEMA_VERSION,
        fingerprint_id,
        kind,
        name: format!("task-fp-010 {}", fingerprint_id.hex()),
        source_corpus: source_corpus.to_string(),
        source_manifest_sha256: Some([7; 32]),
        centroid_by_embedder: BTreeMap::from([(embedder.clone(), expand_evidence(centroid2))]),
        variance_by_embedder: BTreeMap::from([(embedder.clone(), 0.0)]),
        tau_by_embedder: BTreeMap::from([(embedder, 0.90)]),
        pairwise_cosine: Vec::new(),
        pairwise_mutual_information: Vec::new(),
        reference_chunks: vec![context_graph_mejepa::ChunkId("src/lib.py#0".to_string())],
        n_references: 1,
        oracle_outcome,
        is_canonical: true,
        frozen_at_unix_ms: unix_ms()?,
        confidence: FingerprintConfidence {
            classification_accuracy: Some(1.0),
            classification_precision: Some(1.0),
            unknown_recall: None,
            calibration_observations: 8,
            calibration_state: FingerprintCalibrationState::Calibrated,
        },
    };
    fingerprint.validate()?;
    Ok(fingerprint)
}

fn patch_context(
    run_root: &Path,
    case: &str,
    patch_after: &str,
) -> Result<(PatchBundle, TaskContext)> {
    let repo_root = run_root.join("repo");
    fs::create_dir_all(&repo_root)?;
    let hunks = (0..4)
        .map(|idx| {
            let rel_path = PathBuf::from(format!("{case}_{idx}.py"));
            context_graph_mejepa::DiffHunk {
                path: rel_path,
                pre_sha: [1; 32],
                post_sha: sha32(format!("{case}:{idx}:{patch_after}").as_bytes()),
                before: "return 0".to_string(),
                after: patch_after.to_string(),
            }
        })
        .collect();
    let patch = PatchBundle::try_new(
        AstDiff { hunks },
        valid_witness_segment(),
        format!("task-fp-010 {case}"),
        sha32(case.as_bytes()),
    )?;
    let context = TaskContext {
        task_id: TaskId(format!("task-fp-010-{case}")),
        session_id: sha16(case.as_bytes()),
        language: Language::Python,
        problem_statement: format!("TASK-FP-010 FSV {case}"),
        tests: vec![TestId("test_task_fp_010".to_string())],
        environment: TaskEnvironment {
            repo_root,
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    Ok((patch, context))
}

fn base_prediction(case: &str, verdict: Verdict) -> Result<RealityPrediction> {
    RealityPredictionBuilder::from_parts(
        TaskId(format!("task-fp-010-{case}")),
        sha16(case.as_bytes()),
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::Abstain], 0.1, 1.0)?,
    )
    .prediction_id(sha16(format!("prediction-{case}").as_bytes()))
    .witness_hash(WitnessHash([3; 32]))
    .covered_chunks(vec![context_graph_mejepa::ChunkId(format!("{case}.py#0"))])
    .verdict(verdict)
    .confidence_interval(ConformalInterval::default())
    .predicted_oracle_pass(0.0)
    .predicted_test_pass(vec![0.0])
    .predicted_runtime_trace([0.0; 32])
    .ood_score(if verdict == Verdict::OutOfDistribution {
        1.0
    } else {
        0.0
    })
    .calibrated_confidence(0.0)
    .degraded_status(true)
    .provenance(PredictionProvenance {
        predictor_version: "task-fp-010-fsv".to_string(),
        constellation_version: "task-fp-010-fsv".to_string(),
        calibration_version: "task-fp-010-fsv".to_string(),
        active_pointer: case.to_string(),
    })
    .source_panel_sha([4; 32])
    .calibration_version("task-fp-010-fsv")
    .build()
    .context("build base edge prediction")
}

fn calibration_record() -> CalibrationRecord {
    CalibrationRecord {
        version: "task-fp-010-fsv".to_string(),
        alpha: 0.1,
        target_coverage: 0.9,
        tau: 0.20,
        sigma_squared: 100.0,
        empirical_coverage: 0.95,
        min_samples_per_stratum: 1,
        sample_count: 1,
        per_language_counts: BTreeMap::from([(Language::Python, 1)]),
        corpus_sha: [8; 32],
        embedder_versions: BTreeMap::from([(EmbedderId("e_ast".to_string()), "fsv".to_string())]),
        frozen_at: chrono::Utc::now().timestamp(),
    }
}

fn valid_witness_segment() -> Vec<u8> {
    vec![0u8; context_graph_witness::WITNESS_ENTRY_SIZE]
}

fn sha32(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn sha16(bytes: &[u8]) -> [u8; 16] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn unix_ms() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
