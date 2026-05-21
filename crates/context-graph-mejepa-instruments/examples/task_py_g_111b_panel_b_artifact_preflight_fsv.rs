use context_graph_mejepa_instruments::{
    default_panel_b_artifact_specs, require_panel_b_artifacts, resolve_panel_b_artifact_manifest,
    validate_aiwonder_models_root, PanelBArtifactSpec, PanelBSlot,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-111b-model-backed-panel-b-fsv";
const DEFAULT_MODELS_ROOT: &str = "/zfs/hot/contextgraph/models";

fn main() -> Result<(), Box<dyn Error>> {
    let fsv_root = env_path("CG_PANEL_B_PREFLIGHT_FSV_ROOT", DEFAULT_FSV_ROOT);
    let models_root = env_path("CG_PANEL_B_MODELS_ROOT", DEFAULT_MODELS_ROOT);
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;

    let specs = default_panel_b_artifact_specs();
    let manifest = resolve_panel_b_artifact_manifest(&models_root, &specs)?;
    let artifact_gate = require_panel_b_artifacts(&manifest);
    let artifact_contract_valid = if manifest.all_required_available() {
        artifact_gate.is_ok()
    } else {
        artifact_gate.is_err()
    };
    let synthetic_rejected = PanelBArtifactSpec::new(
        PanelBSlot::Ast,
        "synthetic-codebert-placeholder",
        "https://huggingface.co/microsoft/codebert-base",
        "3b0952feddeffad0063f274080e3c23d75e7eb39",
        "hf_transformers_feature_extraction",
        "panel-b/codebert-base",
        vec!["config.json"],
    )
    .validate()
    .is_err();
    let non_aiwonder_root_rejected =
        validate_aiwonder_models_root(Path::new("/tmp/contextgraph/models")).is_err();
    let missing_have_no_synthetic_sha = manifest.missing.is_empty()
        || manifest.artifact_shas_by_slot().len() == manifest.artifacts.len();
    let all_passed = artifact_contract_valid && synthetic_rejected && non_aiwonder_root_rejected;
    let evidence = json!({
        "artifactKind": "task_py_g_111b_panel_b_artifact_preflight_fsv",
        "schemaVersion": 1,
        "coveredGithubIssues": [521],
        "allPassed": all_passed,
        "panelBReady": manifest.all_required_available(),
        "shipGateEligible": manifest.all_required_available(),
        "sourceOfTruth": {
            "modelsRoot": models_root,
            "fsvRoot": fsv_root
        },
        "checks": {
            "requiredArtifactCount": manifest.required_artifact_count,
            "availableArtifactCount": manifest.available_artifact_count,
            "missingArtifactCount": manifest.missing.len(),
            "artifactContractValid": artifact_contract_valid,
            "syntheticArtifactIdsRejected": synthetic_rejected,
            "nonAiwonderModelRootRejected": non_aiwonder_root_rejected,
            "missingArtifactsHaveNoSyntheticSha": missing_have_no_synthetic_sha,
            "activePathPolicy": {
                "mntDActiveReadsOrWrites": false
            }
        },
        "manifest": manifest,
        "blockedReason": if manifest.all_required_available() {
            serde_json::Value::Null
        } else {
            json!("required non-overlapping Panel B artifacts are not installed under the aiwonder models root")
        }
    });
    let report_path = fsv_root.join("panel_b_artifact_preflight_fsv.json");
    write_json_0600(&report_path, &evidence)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    let readback_path = fsv_root.join("panel_b_artifact_preflight_readback.json");
    write_json_0600(
        &readback_path,
        &json!({
            "reportPath": report_path,
            "readbackAllPassed": readback["allPassed"] == true,
            "reportSha256": sha256_hex(&fs::read(&report_path)?)
        }),
    )?;
    if readback["allPassed"] != true {
        return Err(format!(
            "TASK-PY-G-111B preflight FSV failed at {}",
            report_path.display()
        )
        .into());
    }
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var(name)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn write_json_0600(path: &Path, value: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
