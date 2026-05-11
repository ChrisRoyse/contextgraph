use context_graph_mejepa_embedders::{
    embedder_model_id, EmbedderForward, EmbedderId, EmbedderInput, ModelsConfig,
    PretrainedEmbedderForward,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> FsvResult<()> {
    let root = PathBuf::from("/tmp/contextgraph-mejepa-forward-fsv");
    reset_root(&root)?;
    let output_path = root.join("e10-forward-output.json");
    let evidence_path = root.join("forward-wrapper-evidence.json");
    let config_path = std::env::var("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("/mnt/d/cabdru/contextgraph/models/mejepa_models_config.toml")
        });

    let config = ModelsConfig::load(&config_path)?;
    let registration = config.registration(EmbedderId::E10)?;

    let before = json!({
        "models_config": config_path,
        "registration_path": registration.path,
        "registration_manifest_sha256": registration.manifest_sha256,
        "output_exists": output_path.exists(),
    });
    let forward = PretrainedEmbedderForward::load(registration).await?;
    let input = EmbedderInput {
        embedder: EmbedderId::E10,
        text: "ContextGraph forward-wrapper FSV: production telemetry should persist real latency, OOD, and calibration evidence.".to_string(),
        source_id: "forward-fsv/e10/contextual.txt".to_string(),
    };
    let output = forward.forward(&input).await?;
    let output_json = json!({
        "embedder": output.embedder.slug(),
        "source_id": output.source_id,
        "model_version": output.model_version,
        "precision_class": output.precision_class,
        "vector_len": output.vector.len(),
        "vector_l2_norm": l2_norm(&output.vector),
        "vector_sha256": vector_sha256(&output.vector),
        "first_8": output.vector.iter().take(8).copied().collect::<Vec<_>>(),
    });
    write_json_checked(&output_path, &output_json)?;
    let readback: Value = serde_json::from_slice(&fs::read(&output_path)?)?;
    require(
        "forward output readback embedder",
        readback["embedder"] == output_json["embedder"],
    )?;
    require(
        "forward output readback source_id",
        readback["source_id"] == output_json["source_id"],
    )?;
    require(
        "forward output readback vector hash",
        readback["vector_sha256"] == output_json["vector_sha256"],
    )?;
    require(
        "forward vector dimension",
        readback["vector_len"] == json!(EmbedderId::E10.dimension()),
    )?;
    require(
        "forward vector is non-zero",
        readback["vector_l2_norm"].as_f64().unwrap_or_default() > 0.0,
    )?;

    let edge_cases = vec![
        edge_unsupported_forward_slot(&root)?,
        edge_empty_input(&forward, &root).await?,
        edge_mismatched_input_embedder(&forward, &root).await?,
    ];
    let evidence = json!({
        "source_of_truth": {
            "evidence_json": evidence_path,
            "forward_output_json": output_path,
            "model_registration": registration,
        },
        "happy_path": {
            "before_state": before,
            "trigger": "PretrainedEmbedderForward::load(E10) then forward(E10 text input)",
            "after_state": output_json,
            "passes": true,
        },
        "edge_cases": edge_cases,
        "passes": true,
    });
    write_json_checked(&evidence_path, &evidence)?;
    let evidence_readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback passes",
        evidence_readback["passes"] == Value::Bool(true),
    )?;
    require(
        "evidence readback output path",
        evidence_readback["source_of_truth"]["forward_output_json"]
            == output_path.display().to_string(),
    )?;
    require(
        "evidence readback vector hash",
        evidence_readback["happy_path"]["after_state"]["vector_sha256"]
            == output_json["vector_sha256"],
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn edge_unsupported_forward_slot(root: &Path) -> FsvResult<Value> {
    let before = json!({"embedder": "e2", "files_before": file_count(root)?});
    let err = embedder_model_id(EmbedderId::E2).unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "unsupported slot fails closed",
        err.code() == "MEJEPA_EMBED_FORWARD_FAILED",
    )?;
    require(
        "unsupported slot writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_unsupported_forward_slot",
        "before_state": before,
        "trigger": "embedder_model_id(E2)",
        "after_state": after,
        "passes": true,
    }))
}

async fn edge_empty_input(forward: &PretrainedEmbedderForward, root: &Path) -> FsvResult<Value> {
    let before = json!({"text_len": 0, "files_before": file_count(root)?});
    let err = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E10,
            text: "".to_string(),
            source_id: "forward-fsv/e10/empty.txt".to_string(),
        })
        .await
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "empty input fails closed",
        err.code() == "MEJEPA_EMBED_INVALID_INPUT",
    )?;
    require(
        "empty input writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_empty_input",
        "before_state": before,
        "trigger": "forward(E10 empty text)",
        "after_state": after,
        "passes": true,
    }))
}

async fn edge_mismatched_input_embedder(
    forward: &PretrainedEmbedderForward,
    root: &Path,
) -> FsvResult<Value> {
    let before =
        json!({"loaded_wrapper": "e10", "input_embedder": "e1", "files_before": file_count(root)?});
    let err = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E1,
            text: "mismatched embedder should not be forwarded".to_string(),
            source_id: "forward-fsv/e1/mismatch.txt".to_string(),
        })
        .await
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "mismatched input embedder fails closed",
        err.code() == "MEJEPA_EMBED_FORWARD_FAILED",
    )?;
    require(
        "mismatched input writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_mismatched_input_embedder",
        "before_state": before,
        "trigger": "E10 wrapper receives E1 input",
        "after_state": after,
        "passes": true,
    }))
}

fn reset_root(root: &Path) -> FsvResult<()> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require("JSON byte readback equality", fs::read(path)? == bytes)?;
    Ok(())
}

fn file_count(path: &Path) -> FsvResult<usize> {
    Ok(fs::read_dir(path)?.count())
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn l2_norm(vector: &[f32]) -> f64 {
    vector
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt()
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("forward wrapper FSV assertion failed: {label}").into())
    }
}
