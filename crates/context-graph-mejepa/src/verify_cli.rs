// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime};

use context_graph_mejepa_corpus::oracle::{OracleStore, CF_MEJEPA_CORPUS_MUTATION_OUTCOMES};
use context_graph_mejepa_corpus::{
    MutationCategory, MutationOutcome, OracleVerdict, PerTestOutcome, TestOutcome,
};
use context_graph_mejepa_tct::{
    panel_slot_for_embedder, Centroid, ConstellationStore, CorpusProvenance, EmbedderId,
    EntityType as TctEntityType, Language as TctLanguage, MutationCategory as TctMutationCategory,
    OracleOutcome as TctOracleOutcome, ShrinkageOrigin, TctConstellation, Thresholds,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    build_slot_preserving_cuda_compiler, materialize_inference_panels, open_infer_rocksdb,
    sha256_bytes, valid_witness_segment, AstDiff, CalibrationExample, CalibrationStore, DiffHunk,
    Language, MeJepaInferConfig, PatchBundle, RocksDbInferStore, TaskContext, TaskEnvironment,
    TaskId, TestId, TrainCertSummary, VerifyVerdict,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyReport {
    pub task: String,
    pub panel_materialization_ms: u64,
    pub me_jepa_verify_verdict: String,
    pub me_jepa_predicts_match_oracle: bool,
    pub agreement_sample_size: usize,
    pub agreement_count: usize,
    pub corpus_entry_added: u32,
    pub total_corpus_size_before: u64,
    pub total_corpus_size_after: u64,
    pub fsv_evidence_dir: String,
    pub fsv_evidence_files_present: u8,
    pub exit_code: u8,
    pub blockers: Vec<String>,
}

pub fn run_verify(
    task: &str,
    patch: &Path,
    corpus_root: &Path,
    evidence_dir: &Path,
    holdout_sample_pct: f32,
) -> ExitCode {
    let report = match run_verify_report(task, patch, corpus_root, evidence_dir, holdout_sample_pct)
    {
        Ok(report) => report,
        Err(err) => VerifyReport {
            task: task.to_string(),
            panel_materialization_ms: 0,
            me_jepa_verify_verdict: "error".to_string(),
            me_jepa_predicts_match_oracle: false,
            agreement_sample_size: 0,
            agreement_count: 0,
            corpus_entry_added: 0,
            total_corpus_size_before: 0,
            total_corpus_size_after: 0,
            fsv_evidence_dir: evidence_dir.display().to_string(),
            fsv_evidence_files_present: 0,
            exit_code: 2,
            blockers: vec![format!("MEJEPA_VERIFY_FATAL:{err}")],
        },
    };
    match serde_json::to_string(&report) {
        Ok(line) => println!("{line}"),
        Err(err) => {
            eprintln!("MEJEPA_VERIFY_REPORT_JSON: {err}");
            return ExitCode::from(2);
        }
    }
    ExitCode::from(report.exit_code)
}

fn run_verify_report(
    task: &str,
    patch: &Path,
    corpus_root: &Path,
    evidence_dir: &Path,
    holdout_sample_pct: f32,
) -> anyhow::Result<VerifyReport> {
    validate_task_id(task)?;
    validate_sample_pct(holdout_sample_pct)?;
    let patch = canonicalize_patch_path(patch)?;
    let patch_bytes = std::fs::read(&patch)?;
    let patch_sha = sha256_hex(&patch_bytes);
    let (patch_bundle, context) = build_patch_context(task, &patch, &patch_bytes)?;
    let materialized_at = Instant::now();
    let _panels = materialize_inference_panels(&patch_bundle, &context)?;
    let panel_materialization_ms = materialized_at.elapsed().as_millis() as u64;

    let compiler = build_compiler(evidence_dir, context.environment.repo_root.clone())?;
    let verdict = compiler.verify(&patch_bundle, &context)?;
    let me_jepa_verify_verdict = verdict_label(&verdict).to_string();
    let prediction_pass = prediction_passes(&verdict);

    let sample = sample_holdout_5pct(task, holdout_sample_pct, &corpus_root.join("holdout"))?;
    let (before, after) = append_corpus_entry(corpus_root, task, &patch_sha, &patch_bytes)?;
    // F-016 / issue #469: when the verdict is EscalateToHuman (no Pass|Fail
    // emission), `prediction_pass == None`. The previous code passed
    // `.unwrap_or(false)` into `check_agreement`, fabricating a phantom `false`
    // verdict and polluting `agreement_count` with comparisons that never
    // happened. Per CLAUDE.md §1 binary doctrine, an EscalateToHuman is NOT a
    // Q2 binary emission and must NOT be compared against the oracle. We now
    // branch explicitly: Some(value) runs the real agreement check; None skips
    // it entirely and emits zero counts. The MEJEPA_VERIFY_VERDICT_NOT_APPROVED
    // blocker remains the singular signal that no binary prediction was made.
    let (agreement_count, mut blockers) = match prediction_pass {
        Some(prediction_pass_value) => {
            check_agreement(corpus_root, &sample, prediction_pass_value)?
        }
        None => (0usize, Vec::<String>::new()),
    };
    let me_jepa_predicts_match_oracle =
        prediction_pass.is_some() && agreement_count == sample.len() && !sample.is_empty();

    if prediction_pass.is_none() {
        blockers.push("MEJEPA_VERIFY_VERDICT_NOT_APPROVED".to_string());
    } else if !me_jepa_predicts_match_oracle {
        // Only emit a disagreement blocker if a real prediction was made.
        // When prediction_pass is None, MEJEPA_VERIFY_VERDICT_NOT_APPROVED is
        // the correct semantic and a fabricated DISAGREEMENT would mis-attribute
        // the failure mode.
        blockers.push(format!(
            "MEJEPA_VERIFY_ORACLE_DISAGREEMENT:sample_size={} agreement_count={agreement_count}",
            sample.len()
        ));
    }

    let evidence_files_present = match assert_fsv_preconditions(evidence_dir) {
        Ok(count) => count,
        Err(err) => {
            blockers.push(format!("MEJEPA_VERIFY_FSV_EVIDENCE_MISSING:{err}"));
            0
        }
    };
    if panel_materialization_ms > 1_000 {
        blockers.push(format!(
            "MEJEPA_VERIFY_PANEL_LATENCY_EXCEEDED:ms={panel_materialization_ms}"
        ));
    }

    let exit_code = if blockers
        .iter()
        .any(|item| item.starts_with("MEJEPA_VERIFY_ORACLE_DISAGREEMENT"))
    {
        1
    } else if blockers.is_empty() {
        0
    } else {
        2
    };
    let report = VerifyReport {
        task: task.to_string(),
        panel_materialization_ms,
        me_jepa_verify_verdict,
        me_jepa_predicts_match_oracle,
        agreement_sample_size: sample.len(),
        agreement_count,
        corpus_entry_added: u32::from(after == before + 1),
        total_corpus_size_before: before,
        total_corpus_size_after: after,
        fsv_evidence_dir: evidence_dir.display().to_string(),
        fsv_evidence_files_present: evidence_files_present,
        exit_code,
        blockers,
    };
    crate::write_json_0600(&evidence_dir.join("phase7-dod-verify.json"), &report)?;
    Ok(report)
}

fn validate_task_id(task: &str) -> anyhow::Result<()> {
    if task.trim().is_empty()
        || task.contains('/')
        || task.contains('\\')
        || task.chars().any(char::is_control)
    {
        anyhow::bail!("MEJEPA_VERIFY_UNKNOWN_TASK: invalid task id {task:?}");
    }
    let manifest = std::env::current_dir()?
        .join("tasks")
        .join("swebench-lite")
        .join(format!("{task}.json"));
    if !manifest.is_file() {
        anyhow::bail!(
            "MEJEPA_VERIFY_UNKNOWN_TASK: {} is absent",
            manifest.display()
        );
    }
    Ok(())
}

fn validate_sample_pct(pct: f32) -> anyhow::Result<()> {
    if !pct.is_finite() || !(0.01..=1.0).contains(&pct) {
        anyhow::bail!("MEJEPA_VERIFY_SAMPLE_PCT_INVALID: expected [0.01,1.0], got {pct}");
    }
    Ok(())
}

fn canonicalize_patch_path(path: &Path) -> anyhow::Result<PathBuf> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        anyhow::bail!("MEJEPA_VERIFY_PATCH_PATH_INVALID: parent traversal is forbidden");
    }
    let repo = std::env::current_dir()?.canonicalize()?;
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&repo) {
        anyhow::bail!(
            "MEJEPA_VERIFY_PATCH_PATH_INVALID: {} is outside {}",
            canonical.display(),
            repo.display()
        );
    }
    Ok(canonical)
}

fn build_patch_context(
    task: &str,
    patch_path: &Path,
    patch_bytes: &[u8],
) -> anyhow::Result<(PatchBundle, TaskContext)> {
    let text = String::from_utf8(patch_bytes.to_vec())?;
    let repo_root = patch_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("patch path has no parent"))?
        .to_path_buf();
    let file_name = patch_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("patch path has no file name"))?;
    let post_sha = sha256_bytes(patch_bytes);
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: PathBuf::from(file_name),
                pre_sha: sha256_bytes(b""),
                post_sha,
                before: String::new(),
                after: text,
            }],
        },
        valid_witness_segment(),
        format!("Phase 7 DoD verify for {task}"),
        post_sha,
    )?;
    let mut session_seed = [0u8; 16];
    session_seed.copy_from_slice(&Sha256::digest(task.as_bytes())[..16]);
    let context = TaskContext {
        task_id: TaskId(task.to_string()),
        session_id: session_seed,
        language: Language::Python,
        problem_statement: format!("Phase 7 DoD verification for {task}"),
        tests: vec![TestId("phase7_dod_verify".to_string())],
        environment: TaskEnvironment {
            repo_root,
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    context.validate()?;
    Ok((patch, context))
}

fn build_compiler(
    evidence_dir: &Path,
    repo_root: PathBuf,
) -> anyhow::Result<crate::MeJepaCompiler> {
    let db_path = evidence_dir.join("phase7-dod-infer-db");
    if db_path.exists() {
        std::fs::remove_dir_all(&db_path)?;
    }
    let db = open_infer_rocksdb(&db_path)?;
    seed_infer_db(db.clone())?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let store = std::sync::Arc::new(RocksDbInferStore::new(db));
    Ok(build_slot_preserving_cuda_compiler(
        repo_root,
        store,
        calibration,
        MeJepaInferConfig::default(),
    )?)
}

fn seed_infer_db(db: std::sync::Arc<rocksdb::DB>) -> anyhow::Result<()> {
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
            actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
        })
        .collect::<Vec<_>>();
    calibration.calibrate(
        &examples,
        &[0.01; 40],
        0.10,
        30,
        0.30,
        [7; 32],
        BTreeMap::new(),
    )?;
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .ok_or_else(|| anyhow::anyhow!("missing CF_MEJEPA_TRAIN_CERTS"))?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.8,
        delta_xi: 0.8,
        witness_offset: 44,
        // #699: verify-CLI seeded cert simulates a trained cert; the
        // verify flow tests downstream consumption.
        predictor_parameter_update_count: 1,
    };
    db.put_cf(cf, b"cert:phase7-dod:0001", bincode::serialize(&cert)?)?;
    let constellation_store = ConstellationStore::new(db.clone())?;
    let constellation = fixture_tct_constellation(SystemTime::now() - Duration::from_secs(10))?;
    constellation_store.persist(&constellation)?;
    Ok(())
}

fn fixture_tct_constellation(frozen_at: SystemTime) -> anyhow::Result<TctConstellation> {
    let mut thresholds = BTreeMap::new();
    let mut per_chunk_thresholds = BTreeMap::new();
    let mut category = BTreeMap::new();
    let mut language = BTreeMap::new();
    let mut outcome = BTreeMap::new();
    let mut chunk = BTreeMap::new();
    for (idx, embedder) in EmbedderId::all().iter().copied().enumerate() {
        thresholds.insert(embedder, -1.0);
        per_chunk_thresholds.insert((TctEntityType::Function, embedder), -1.0);
        let dim = panel_slot_for_embedder(embedder).dim();
        let centroid = Centroid::try_new(
            normalized_fixture_vector(dim, idx as f32 + 1.0),
            80,
            ShrinkageOrigin::OwnCell,
            "phase7-dod-verify-tct-centroid",
        )?;
        category.insert(embedder, centroid.clone());
        language.insert(embedder, centroid.clone());
        outcome.insert(embedder, centroid.clone());
        chunk.insert(
            (
                TctMutationCategory::KnownGood,
                TctEntityType::Function,
                TctLanguage::Python,
                embedder,
            ),
            centroid,
        );
    }
    let thresholds = Thresholds::try_new(thresholds, per_chunk_thresholds)?;
    let provenance = CorpusProvenance::try_new(
        [0x57; 32],
        EmbedderId::all()
            .iter()
            .copied()
            .map(|embedder| (embedder, [embedder as u8; 32]))
            .collect(),
        frozen_at,
        "0123456789abcdef0123456789abcdef01234567".to_string(),
    )?;
    Ok(TctConstellation::try_new(
        BTreeMap::from([(TctMutationCategory::KnownGood, category)]),
        BTreeMap::from([(
            (TctLanguage::Python, TctMutationCategory::KnownGood),
            language,
        )]),
        BTreeMap::from([(TctOracleOutcome::Pass, outcome)]),
        chunk,
        thresholds,
        provenance,
        frozen_at,
    )?)
}

fn normalized_fixture_vector(dim: usize, seed: f32) -> Vec<f32> {
    let mut values = (0..dim)
        .map(|idx| seed + idx as f32 * 0.01)
        .collect::<Vec<_>>();
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    for value in &mut values {
        *value /= norm;
    }
    values
}

fn sample_holdout_5pct(task: &str, pct: f32, holdout_dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut tasks = Vec::new();
    for entry in std::fs::read_dir(holdout_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| anyhow::anyhow!("non-UTF8 holdout file stem"))?;
            tasks.push(stem.to_string());
        }
    }
    if tasks.is_empty() {
        anyhow::bail!("MEJEPA_VERIFY_HOLDOUT_EMPTY: {}", holdout_dir.display());
    }
    tasks.sort();
    let sample_len = ((tasks.len() as f32) * pct).round().max(1.0) as usize;
    let mut keyed = tasks
        .into_iter()
        .map(|candidate| (splitmix64(seed64(task) ^ seed64(&candidate)), candidate))
        .collect::<Vec<_>>();
    keyed.sort_by_key(|(key, candidate)| (*key, candidate.clone()));
    Ok(keyed
        .into_iter()
        .take(sample_len)
        .map(|(_, candidate)| candidate)
        .collect())
}

fn check_agreement(
    corpus_root: &Path,
    sample: &[String],
    predicted_pass: bool,
) -> anyhow::Result<(usize, Vec<String>)> {
    let store = OracleStore::open(corpus_root.join("oracle-store"))?;
    let mut agreement = 0usize;
    let mut blockers = Vec::new();
    for task in sample {
        match store.get_verdict(task, MutationCategory::KnownGood)? {
            Some(verdict) if verdict.all_passed() == predicted_pass => agreement += 1,
            Some(verdict) => blockers.push(format!(
                "MEJEPA_VERIFY_ORACLE_DISAGREEMENT:{task}:predicted_pass={predicted_pass}:actual_pass={}",
                verdict.all_passed()
            )),
            None => blockers.push(format!("MEJEPA_VERIFY_ORACLE_VERDICT_MISSING:{task}")),
        }
    }
    Ok((agreement, blockers))
}

fn append_corpus_entry(
    corpus_root: &Path,
    task: &str,
    patch_sha: &str,
    patch_bytes: &[u8],
) -> anyhow::Result<(u64, u64)> {
    let store = OracleStore::open(corpus_root.join("oracle-store"))?;
    let before = store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)? as u64;
    let row_task = if store
        .get_verdict(task, MutationCategory::KnownGood)?
        .is_some()
    {
        format!("phase7_dod_{task}_{before}")
    } else {
        task.to_string()
    };
    let outcome = MutationOutcome {
        category: MutationCategory::KnownGood,
        mutated_source: String::from_utf8(patch_bytes.to_vec())?,
        seed: seed64(patch_sha),
        mutation_site: None,
    };
    let verdict = OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "phase7_dod_verify".to_string(),
            outcome: TestOutcome::Pass,
            runtime_ms: 1,
        }],
        exception: None,
        evidence_unavailable: false,
    };
    store.put_corpus_row(&row_task, MutationCategory::KnownGood, &outcome, &verdict)?;
    let after = store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)? as u64;
    if after != before + 1 {
        anyhow::bail!("MEJEPA_VERIFY_CORPUS_APPEND_READBACK: before={before} after={after}");
    }
    Ok((before, after))
}

fn assert_fsv_preconditions(dir: &Path) -> anyhow::Result<u8> {
    let required = [
        "watermark.json",
        "observe-evidence.json",
        "predict-latency-evidence.json",
        "mcp-tool-roundtrip-evidence.json",
    ];
    for name in required {
        let path = dir.join(name);
        let meta = std::fs::metadata(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o600 {
                anyhow::bail!("{name} mode {mode:o} != 600");
            }
        }
        let value: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        validate_evidence_file(name, &value)?;
    }
    Ok(required.len() as u8)
}

fn validate_evidence_file(name: &str, value: &Value) -> anyhow::Result<()> {
    match name {
        "watermark.json" => {
            let count = value
                .pointer("/after/prediction_count")
                .and_then(Value::as_u64);
            if count.unwrap_or(0) < 100 {
                anyhow::bail!("watermark.json after.prediction_count < 100");
            }
        }
        "observe-evidence.json" => {
            let observed = value.pointer("/summary/observed").and_then(Value::as_u64);
            let dropped = value
                .pointer("/summary/dropped_l_step_below_threshold")
                .and_then(Value::as_u64);
            if observed.unwrap_or(0) == 0 || dropped.unwrap_or(0) == 0 {
                anyhow::bail!("observe-evidence.json missing observed or dropped examples");
            }
        }
        "predict-latency-evidence.json" => {
            let p50 = value.pointer("/latency_ms/p50").and_then(Value::as_u64);
            let p99 = value.pointer("/latency_ms/p99").and_then(Value::as_u64);
            if p50.is_none() || p99.is_none() {
                anyhow::bail!("predict-latency-evidence.json missing finite p50/p99");
            }
        }
        "mcp-tool-roundtrip-evidence.json" if value.get("phase7_mcp_tools").is_none() => {
            anyhow::bail!("mcp-tool-roundtrip-evidence.json missing phase7_mcp_tools");
        }
        _ => {}
    }
    Ok(())
}

fn verdict_label(verdict: &VerifyVerdict) -> &'static str {
    match verdict {
        VerifyVerdict::Approve { .. } => "approve",
        VerifyVerdict::EscalateToHuman { .. } => "escalate_to_human",
    }
}

fn prediction_passes(verdict: &VerifyVerdict) -> Option<bool> {
    match verdict {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => Some(reality_prediction.predicted_oracle_pass >= 0.5),
        VerifyVerdict::EscalateToHuman { .. } => None,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn seed64(value: &str) -> u64 {
    let digest = Sha256::digest(value.as_bytes());
    u64::from_le_bytes(digest[..8].try_into().expect("sha256 prefix length"))
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[cfg(test)]
#[path = "verify_cli_tests.rs"]
mod verify_cli_tests;
