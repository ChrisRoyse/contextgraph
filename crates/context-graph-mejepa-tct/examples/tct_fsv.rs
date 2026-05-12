use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use context_graph_mejepa_instruments::{hash_f32s, InstrumentSlot, Panel, PanelBuilder};
use context_graph_mejepa_tct::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

type FsvResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstellationEvidence {
    source_of_truth: serde_json::Value,
    before_state: serde_json::Value,
    after_state: serde_json::Value,
    inspect_summary: ConstellationInspectOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GtauEvidence {
    source_of_truth: serde_json::Value,
    fixtures: Vec<GtauFixtureRecord>,
    replay: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GtauFixtureRecord {
    name: String,
    panel_hash: String,
    output: GtauOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdCalibrationEvidence {
    source_of_truth: serde_json::Value,
    decisions: Vec<CalibrationDecision>,
    summary: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkLevelRejectionEvidence {
    source_of_truth: serde_json::Value,
    before_state: serde_json::Value,
    after_state: serde_json::Value,
    per_chunk: ChunkGtauOutput,
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-mejepa-tct-fsv");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    let real_panel_path = PathBuf::from(
        "/mnt/d/contextgraph/fsv/contextgraph-mejepa-instruments-fsv/phase1-extended/panel-all-slots.json",
    );
    let real_panel = read_real_phase1_panel(&real_panel_path)?;
    let code_version = git_head()?;
    let versions = embedder_versions_from_panel(&real_panel);

    let draft = build_constellation_with_threshold(
        &real_panel,
        &versions,
        &code_version,
        Thresholds::try_new(
            EmbedderId::all()
                .into_iter()
                .map(|embedder| (embedder, 0.0))
                .collect(),
            BTreeMap::new(),
        )?,
        50,
    )?;
    let validation = HeldOutValidation {
        knowngood_samples: (0..50)
            .map(|_| HeldOutSample {
                language: Language::Python,
                entity_type: EntityType::Function,
                mutation: MutationCategory::KnownGood,
                panel: real_panel.clone(),
            })
            .collect(),
    };
    let (thresholds, calibration_decisions) = calibrate(&draft.0, &validation)?;
    let (constellation, shrinkage_decisions) =
        build_constellation_with_threshold(&real_panel, &versions, &code_version, thresholds, 50)?;

    let db_path = root.join("tct-rocksdb");
    let db = open_tct_rocksdb(&db_path)?;
    let store = ConstellationStore::new(db.clone())?;
    let before_count = store.count_constellations()?;
    let version = store.persist(&constellation)?;
    let after_count = store.count_constellations()?;
    drop(store);
    drop(db);
    let reopened = ConstellationStore::new(open_tct_rocksdb(&db_path)?)?;
    let loaded = reopened.load(version, &versions)?;
    let raw_rows = reopened.read_raw_by_version(version)?;
    require(
        "reopened constellation count",
        reopened.count_constellations()? == 1,
    )?;
    require("raw row count", raw_rows.len() == 1)?;
    require(
        "loaded version equals stored",
        loaded.version_id() == version,
    )?;

    let inspect = build_inspect_summary(&loaded, Some(&shrinkage_decisions));
    let constellation_evidence = ConstellationEvidence {
        source_of_truth: json!({
            "rocksdb_path": db_path,
            "constellation_cf": CF_MEJEPA_CONSTELLATION,
            "guard_decisions_cf": CF_MEJEPA_GUARD_DECISIONS,
            "version_id": hex::encode(version),
            "raw_rows_for_version": raw_rows.len(),
        }),
        before_state: json!({
            "constellation_rows_before_persist": before_count,
            "real_panel_path": real_panel_path,
            "real_panel_hash": hash_f32s(real_panel.data()),
        }),
        after_state: json!({
            "constellation_rows_after_persist": after_count,
            "latest_versions": reopened.list_versions(10)?.into_iter().map(hex::encode).collect::<Vec<_>>(),
            "loaded_version_id": hex::encode(loaded.version_id()),
        }),
        inspect_summary: inspect,
    };
    write_and_verify(
        &root.join("constellation-evidence.json"),
        &constellation_evidence,
    )?;

    let accepted = gtau_check(
        &real_panel,
        MutationCategory::KnownGood,
        Language::Python,
        EntityType::Function,
        &loaded,
    )?;
    require("accepted panel passes Gtau", accepted.gtau_satisfied)?;
    let rejected_panel = negate_slot(&real_panel, panel_slot_for_embedder(EmbedderId::E7))?;
    let rejected = gtau_check(
        &rejected_panel,
        MutationCategory::KnownGood,
        Language::Python,
        EntityType::Function,
        &loaded,
    )?;
    require("negated slot rejects Gtau", !rejected.gtau_satisfied)?;
    let replay = gtau_check(
        &real_panel,
        MutationCategory::KnownGood,
        Language::Python,
        EntityType::Function,
        &loaded,
    )?;
    let mut accepted_cmp = accepted.clone();
    let mut replay_cmp = replay.clone();
    accepted_cmp.elapsed_ms = 0.0;
    replay_cmp.elapsed_ms = 0.0;
    require(
        "Gtau replay equality except elapsed",
        accepted_cmp == replay_cmp,
    )?;
    let gtau_evidence = GtauEvidence {
        source_of_truth: json!({
            "panel_json": real_panel_path,
            "constellation_rocksdb": db_path,
            "loaded_version_id": hex::encode(loaded.version_id()),
        }),
        fixtures: vec![
            GtauFixtureRecord {
                name: "happy_path_real_phase1_panel".to_string(),
                panel_hash: hash_f32s(real_panel.data()),
                output: accepted,
            },
            GtauFixtureRecord {
                name: "edge_negated_e7_slot".to_string(),
                panel_hash: hash_f32s(rejected_panel.data()),
                output: rejected,
            },
        ],
        replay: json!({
            "replayed_fixture": "happy_path_real_phase1_panel",
            "equal_except_elapsed_ms": true,
        }),
    };
    write_and_verify(&root.join("gtau-evidence.json"), &gtau_evidence)?;

    let max_fpr = calibration_decisions
        .iter()
        .filter(|decision| decision.strict_enforcement)
        .map(|decision| decision.observed_fpr)
        .fold(0.0f32, f32::max);
    require("calibration max FPR <= target", max_fpr <= TARGET_FPR)?;
    let threshold_evidence = ThresholdCalibrationEvidence {
        source_of_truth: json!({
            "validation_panel_hash": hash_f32s(real_panel.data()),
            "validation_samples": validation.knowngood_samples.len(),
            "target_fpr": TARGET_FPR,
        }),
        decisions: calibration_decisions,
        summary: json!({
            "max_observed_fpr": max_fpr,
            "strict_cells": EmbedderId::all().len(),
            "passes": true,
        }),
    };
    write_and_verify(
        &root.join("threshold-calibration-evidence.json"),
        &threshold_evidence,
    )?;

    let chunk_good = chunk_id([1u8; 32], 10, 20)?;
    let chunk_bad = chunk_id([2u8; 32], 30, 40)?;
    let chunk_output = gtau_check_chunks(
        &[
            (chunk_good.clone(), real_panel.clone()),
            (chunk_bad.clone(), rejected_panel.clone()),
        ],
        MutationCategory::KnownGood,
        &loaded,
    )?;
    require("chunk aggregate rejects", !chunk_output.aggregate_satisfied)?;
    require(
        "exactly one chunk rejected",
        chunk_output.violating_chunks.len() == 1,
    )?;
    require(
        "expected chunk rejected",
        chunk_output.violating_chunks[0] == chunk_bad,
    )?;
    let chunk_evidence = ChunkLevelRejectionEvidence {
        source_of_truth: json!({
            "constellation_version_id": hex::encode(version),
            "chunk_count": 2,
        }),
        before_state: json!({
            "chunk_ids": [chunk_good.clone(), chunk_bad.clone()],
            "failing_slot": panel_slot_for_embedder(EmbedderId::E7).slug(),
        }),
        after_state: json!({
            "aggregate_satisfied": chunk_output.aggregate_satisfied,
            "violating_chunk_count": chunk_output.violating_chunks.len(),
            "violating_chunks": chunk_output.violating_chunks,
        }),
        per_chunk: chunk_output,
    };
    write_and_verify(
        &root.join("chunk-level-rejection-evidence.json"),
        &chunk_evidence,
    )?;

    let edge_states = run_edge_audit(&root, &loaded, &versions)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "tct_fsv": "passed",
            "root": root,
            "source_of_truth": {
                "rocksdb_path": db_path,
                "constellation_rows": reopened.count_constellations()?,
                "guard_decision_rows": reopened.count_guard_decisions()?,
                "version_id": hex::encode(version),
            },
            "edge_case_audit": edge_states,
            "evidence_files": [
                "constellation-evidence.json",
                "gtau-evidence.json",
                "threshold-calibration-evidence.json",
                "chunk-level-rejection-evidence.json"
            ]
        }))?
    );
    Ok(())
}

fn read_real_phase1_panel(path: &Path) -> FsvResult<Panel> {
    if !path.is_file() {
        return Err(format!(
            "required real Phase 1 panel missing at {}; run `cargo run --release -p context-graph-mejepa-instruments --example phase1_extended_fsv` first",
            path.display()
        )
        .into());
    }
    let text = fs::read_to_string(path)?;
    let panel: Panel = serde_json::from_str(&text)?;
    for slot in InstrumentSlot::all() {
        require(
            &format!("slot {} filled", slot.slug()),
            panel.is_filled(slot),
        )?;
    }
    Ok(panel)
}

fn build_constellation_with_threshold(
    panel: &Panel,
    versions: &BTreeMap<EmbedderId, [u8; 32]>,
    code_version: &str,
    thresholds: Thresholds,
    sample_count: usize,
) -> Result<(TctConstellation, Vec<ShrinkageDecision>), TctError> {
    let mut builder = ConstellationBuilder::new([4u8; 32], versions.clone(), code_version.into())?;
    let chunk = chunk_id([5u8; 32], 10, 20)?;
    for _ in 0..sample_count {
        builder.ingest_corpus_entry(
            panel,
            MutationCategory::KnownGood,
            OracleOutcome::Pass,
            Language::Python,
            EntityType::Function,
            &[(chunk.clone(), panel.clone())],
        )?;
    }
    builder.finalize(thresholds, SystemTime::now())
}

fn embedder_versions_from_panel(panel: &Panel) -> BTreeMap<EmbedderId, [u8; 32]> {
    EmbedderId::all()
        .into_iter()
        .map(|embedder| {
            let slot = panel_slot_for_embedder(embedder);
            let mut bytes = [0u8; 32];
            let digest = sha2_digest(panel.slot(slot));
            bytes.copy_from_slice(&digest);
            (embedder, bytes)
        })
        .collect()
}

fn sha2_digest(values: &[f32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hasher.finalize().into()
}

fn git_head() -> FsvResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --verify HEAD failed; cannot record code_version".into());
    }
    let sha = String::from_utf8(output.stdout)?.trim().to_string();
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("git HEAD was not a 40-char hex SHA: {sha:?}").into());
    }
    Ok(sha.to_ascii_lowercase())
}

fn negate_slot(panel: &Panel, slot: InstrumentSlot) -> Result<Panel, TctError> {
    let mut builder = PanelBuilder::new();
    for current in InstrumentSlot::all() {
        let mut vector = panel.slot(current).to_vec();
        if current == slot {
            for value in &mut vector {
                *value = -*value;
            }
        }
        builder.set_slot(current, &vector)?;
    }
    builder.require_slots(&InstrumentSlot::all())?;
    Ok(builder.build()?)
}

fn chunk_id(file_sha: [u8; 32], line_start: u32, line_end: u32) -> Result<ChunkId, TctError> {
    ChunkId::try_new(
        file_sha,
        Language::Python,
        EntityType::Function,
        line_start,
        line_end,
        [6u8; 16],
    )
}

fn write_and_verify<T>(path: &Path, value: &T) -> FsvResult<()>
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        OpenOptions::new().read(true).open(parent)?.sync_all()?;
    }
    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    require(&format!("{} mode 0600", path.display()), mode == 0o600)?;
    let readback_bytes = fs::read(path)?;
    let readback: T = serde_json::from_slice(&readback_bytes)?;
    require(
        &format!("{} serde readback equality", path.display()),
        readback == *value,
    )?;
    let canonical = serde_json::to_vec_pretty(&readback)?;
    require(
        &format!("{} canonical bytes equality", path.display()),
        canonical == readback_bytes,
    )?;
    Ok(())
}

fn run_edge_audit(
    root: &Path,
    loaded: &TctConstellation,
    versions: &BTreeMap<EmbedderId, [u8; 32]>,
) -> FsvResult<serde_json::Value> {
    let before_empty_validation = json!({ "validation_samples": 0 });
    let empty_validation = HeldOutValidation {
        knowngood_samples: Vec::new(),
    };
    let empty_error = calibrate(loaded, &empty_validation).unwrap_err();
    let after_empty_validation =
        json!({ "error_code": empty_error.code(), "state_mutated": false });

    let before_stale = json!({
        "max_age_days": 90,
        "frozen_at_before": chrono::DateTime::<chrono::Utc>::from(loaded.frozen_at).to_rfc3339(),
    });
    let mut stale = loaded.clone();
    stale.frozen_at = SystemTime::now() - Duration::from_secs(91 * 86_400);
    let stale_error = stale.check_freshness(90, false).unwrap_err();
    let after_stale = json!({ "error_code": stale_error.code(), "state_mutated": false });

    let mut bad_versions = versions.clone();
    bad_versions.insert(EmbedderId::E1, [0u8; 32]);
    let before_provenance = json!({ "runtime_embedder_versions": versions.len() });
    let provenance_error = loaded.check_provenance(&bad_versions).unwrap_err();
    let after_provenance = json!({ "error_code": provenance_error.code(), "state_mutated": false });

    let audit = json!({
        "empty_validation": {
            "before_state": before_empty_validation,
            "after_state": after_empty_validation,
        },
        "stale_constellation": {
            "before_state": before_stale,
            "after_state": after_stale,
        },
        "provenance_mismatch": {
            "before_state": before_provenance,
            "after_state": after_provenance,
        }
    });
    write_and_verify(&root.join("edge-case-audit.json"), &audit)?;
    Ok(audit)
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV assertion failed: {label}").into())
    }
}
