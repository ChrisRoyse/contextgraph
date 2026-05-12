use context_graph_mejepa_embedders::{
    agent_state_score, digest_manifest_for_embedder, query_vram_budget, routing_coverage,
    verify_all_registration_digests, verify_registration_digest, EmbedError, EmbedderId,
    EmbedderKind, EmbedderRegistration, Ensemble, ModelsConfig, VramBudget,
};
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = context_graph_paths::ensure_subdir("fsv/contextgraph-mejepa-embedders-fsv")?;
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    let config_path = std::env::var("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("/mnt/d/cabdru/contextgraph/models/mejepa_models_config.toml")
        });
    let progress_path = root.join("embedders_fsv_progress.json");
    let mut progress = Vec::new();
    record_progress(
        &progress_path,
        &mut progress,
        "start",
        json!({
            "root": root.display().to_string(),
            "models_config": config_path.display().to_string(),
        }),
    )?;

    let routing_rows = routing_coverage()?;
    let routing_path = root.join("routing_coverage.json");
    write_checked(
        &routing_path,
        serde_json::to_string_pretty(&routing_rows)?.as_bytes(),
    )?;
    let routing_readback: Value = serde_json::from_str(&fs::read_to_string(&routing_path)?)?;
    require(
        "routing row readback count",
        routing_readback.as_array().map(Vec::len) == Some(13 * 11),
    )?;
    record_progress(
        &progress_path,
        &mut progress,
        "routing_coverage_complete",
        json!({
            "source_of_truth": routing_path.display().to_string(),
            "row_count": routing_rows.len(),
        }),
    )?;

    let vram_before = json!({
        "budget": "content_set_rtx5090",
        "required_bytes": VramBudget::content_set_rtx5090().required_bytes,
    });
    let vram_after = match query_vram_budget(VramBudget::content_set_rtx5090()) {
        Ok(report) => json!({"passes": true, "report": report}),
        Err(err) => json!({"passes": false, "error": err_json(&err)}),
    };
    let vram_path = root.join("vram_budget_report.json");
    write_checked(
        &vram_path,
        serde_json::to_string_pretty(&vram_after)?.as_bytes(),
    )?;
    record_progress(
        &progress_path,
        &mut progress,
        "vram_budget_complete",
        json!({
            "source_of_truth": vram_path.display().to_string(),
            "passes": vram_after["passes"],
        }),
    )?;

    let config_before = json!({
        "config_path": config_path.display().to_string(),
        "config_exists": config_path.exists(),
    });
    record_progress(
        &progress_path,
        &mut progress,
        "models_config_digest_start",
        json!({"source_of_truth": config_path.display().to_string()}),
    )?;
    let config_after = match ModelsConfig::load(&config_path) {
        Ok(config) => match verify_all_registration_digests(&config) {
            Ok(loaded) => json!({
                "passes": true,
                "registration_count": config.embedders.len(),
                "digest_verified_count": loaded.len(),
                "learner_state_verified_count": EmbedderId::learner_state()
                    .iter()
                    .filter(|id| loaded.contains_key(id))
                    .count(),
            }),
            Err(err) => json!({
                "passes": false,
                "registration_count": config.embedders.len(),
                "error": err_json(&err),
            }),
        },
        Err(err) => json!({
            "passes": false,
            "error": err_json(&err),
        }),
    };
    record_progress(
        &progress_path,
        &mut progress,
        "models_config_digest_complete",
        json!({
            "passes": config_after["passes"],
            "digest_verified_count": config_after["digest_verified_count"],
            "error": config_after.get("error").cloned().unwrap_or(Value::Null),
        }),
    )?;

    record_progress(
        &progress_path,
        &mut progress,
        "load_content_set_start",
        json!({"source_of_truth": config_path.display().to_string()}),
    )?;
    let load_content_after = match Ensemble::load_content_set(&config_path) {
        Ok(ensemble) => json!({
            "passes": true,
            "status": ensemble.status,
        }),
        Err(err) => json!({
            "passes": false,
            "error": err_json(&err),
        }),
    };
    record_progress(
        &progress_path,
        &mut progress,
        "load_content_set_complete",
        json!({
            "passes": load_content_after["passes"],
            "loaded_count": load_content_after["status"]["loaded_count"],
            "error": load_content_after.get("error").cloned().unwrap_or(Value::Null),
        }),
    )?;

    let edge_cases = json!({
        "invalid_embedder_id": edge_invalid_embedder_id(),
        "missing_config": edge_missing_config(&root),
        "digest_mismatch": edge_digest_mismatch(&root)?,
        "e17_missing_calibration": edge_e17_missing_calibration(&root),
    });
    require(
        "edge cases demonstrate expected failures",
        edge_cases
            .as_object()
            .expect("edge object")
            .values()
            .all(|case| case["observed_expected_failure"] == Value::Bool(true)),
    )?;
    record_progress(
        &progress_path,
        &mut progress,
        "edge_cases_complete",
        edge_cases.clone(),
    )?;

    let phase_ready = config_after["passes"] == Value::Bool(true)
        && load_content_after["passes"] == Value::Bool(true);
    let evidence = json!({
        "source_of_truth": {
            "root": root.display().to_string(),
            "models_config": config_path.display().to_string(),
            "routing_coverage_json": routing_path.display().to_string(),
            "vram_budget_json": vram_path.display().to_string(),
            "progress_json": progress_path.display().to_string(),
        },
        "happy_path": {
            "routing": {
                "before_state": {"languages": 11, "entity_types": 13},
                "after_state": {"row_count": routing_rows.len(), "first_row": routing_rows.first()},
                "passes": true,
            },
            "vram_budget": {
                "before_state": vram_before,
                "after_state": vram_after,
            },
            "models_config": {
                "before_state": config_before,
                "after_state": config_after,
            },
            "load_content_set": {
                "before_state": {"required_content_embedders": 12, "retired": ["e5", "e11"]},
                "after_state": load_content_after,
            }
        },
        "edge_cases": edge_cases,
        "phase_ready": phase_ready,
        "passes": phase_ready,
    });
    let evidence_path = root.join("embedders_fsv_evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    write_checked(&evidence_path, text.as_bytes())?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "evidence readback source path",
        readback["source_of_truth"]["routing_coverage_json"] == routing_path.display().to_string(),
    )?;
    println!(
        "Phase 1b embedders FSV evidence: {}",
        evidence_path.display()
    );
    println!("{}", text);
    Ok(())
}

fn record_progress(
    path: &Path,
    progress: &mut Vec<Value>,
    stage: &str,
    detail: Value,
) -> FsvResult<()> {
    progress.push(json!({
        "stage": stage,
        "unix_ms": unix_ms()?,
        "detail": detail,
    }));
    write_checked(path, serde_json::to_string_pretty(progress)?.as_bytes())
}

fn edge_invalid_embedder_id() -> Value {
    match "e99".parse::<EmbedderId>() {
        Ok(_) => json!({"observed_expected_failure": false}),
        Err(err) => json!({
            "before_state": {"input": "e99"},
            "after_state": {"code": err.code(), "message": err.to_string()},
            "observed_expected_failure": err.code() == "MEJEPA_EMBED_INVALID_INPUT",
        }),
    }
}

fn edge_missing_config(root: &Path) -> Value {
    let missing = root.join("missing-models-config.toml");
    match ModelsConfig::load(&missing) {
        Ok(_) => json!({"observed_expected_failure": false}),
        Err(err) => json!({
            "before_state": {"path": missing.display().to_string(), "exists": false},
            "after_state": {"code": err.code(), "message": err.to_string()},
            "observed_expected_failure": err.code() == "MEJEPA_EMBED_CONFIG_READ",
        }),
    }
}

fn edge_digest_mismatch(root: &Path) -> FsvResult<Value> {
    let edge_root = root.join("digest-edge");
    fs::create_dir_all(&edge_root)?;
    let weight_path = edge_root.join("model.safetensors");
    write_checked(&weight_path, b"real deterministic digest edge bytes")?;
    let files = vec!["model.safetensors".to_string()];
    let (actual_manifest, file_digests) =
        digest_manifest_for_embedder(EmbedderId::E1, &edge_root, &files)?;
    let configured_manifest_sha256 = "1".repeat(64);
    let reg = EmbedderRegistration {
        embedder: EmbedderId::E1,
        name: "semantic".into(),
        kind: EmbedderKind::ContentPretrained,
        path: edge_root.display().to_string(),
        repo: Some("local/edge".into()),
        dimension: EmbedderId::E1.dimension(),
        weight_files: files,
        manifest_sha256: configured_manifest_sha256.clone(),
    };
    let result = match verify_registration_digest(&reg) {
        Ok(_) => json!({"observed_expected_failure": false}),
        Err(err) => json!({
            "before_state": {
                "path": weight_path.display().to_string(),
                "actual_manifest_sha256": actual_manifest,
                "file_digests": file_digests,
                "configured_manifest_sha256": configured_manifest_sha256,
            },
            "after_state": {"code": err.code(), "message": err.to_string()},
            "observed_expected_failure": err.code() == "MEJEPA_EMBED_DIGEST_MISMATCH",
        }),
    };
    Ok(result)
}

fn edge_e17_missing_calibration(root: &Path) -> Value {
    let cert = root.join("missing-e17-cert.json");
    match agent_state_score(&cert, 0.2, 0.5) {
        Ok(_) => json!({"observed_expected_failure": false}),
        Err(err) => json!({
            "before_state": {"cert_path": cert.display().to_string(), "exists": false},
            "after_state": {"code": err.code(), "message": err.to_string()},
            "observed_expected_failure": err.code() == "MEJEPA_EMBED_E17_UNCALIBRATED",
        }),
    }
}

fn err_json(err: &EmbedError) -> Value {
    json!({
        "code": err.code(),
        "message": err.to_string(),
    })
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require("byte readback", fs::read(path)? == bytes)?;
    Ok(())
}

fn unix_ms() -> FsvResult<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("embedders FSV invariant failed: {label}").into())
    }
}
