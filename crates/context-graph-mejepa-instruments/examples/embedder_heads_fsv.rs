use context_graph_mejepa_embedders::{
    EmbedderForward, EmbedderId, EmbedderInput, ModelsConfig, PretrainedEmbedderForward,
};
use context_graph_mejepa_instruments::e_embedder_heads::{
    EmbedderBackedInstrumentHead, EmbedderHeadInput,
};
use context_graph_mejepa_instruments::{Instrument, InstrumentSlot};
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
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-mejepa-instruments-embedder-heads-fsv");
    reset_root(&root)?;
    let output_path = root.join("trace-head-output.json");
    let evidence_path = root.join("embedder-heads-evidence.json");
    let config_path = std::env::var("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("/mnt/d/cabdru/contextgraph/models/mejepa_models_config.toml")
        });
    let config = ModelsConfig::load(&config_path)?;
    let registration = config.registration(EmbedderId::E10)?;
    let forward = PretrainedEmbedderForward::load(registration).await?;
    let embedder_output = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E10,
            text: "trace event enter_compile exit_compile elapsed_ms=42 success=true".to_string(),
            source_id: "embedder-head-fsv/trace/e10.txt".to_string(),
        })
        .await?;

    let before = json!({
        "slot": "e_trace",
        "embedder": embedder_output.embedder.slug(),
        "source_id": embedder_output.source_id,
        "input_vector_len": embedder_output.vector.len(),
        "files_before": file_count(&root)?,
    });
    let head = EmbedderBackedInstrumentHead::new(InstrumentSlot::ETrace)?;
    let encoded = head.encode(&EmbedderHeadInput {
        slot: InstrumentSlot::ETrace,
        source_id: embedder_output.source_id.clone(),
        embedder_output: embedder_output.clone(),
        analyzer_features: vec![0.42, 1.0, 0.0, 7.0],
    })?;
    let output_json = json!({
        "slot": "e_trace",
        "vector_len": encoded.len(),
        "vector_l2_norm": l2_norm(&encoded),
        "vector_sha256": vector_sha256(&encoded),
        "first_8": encoded.iter().take(8).copied().collect::<Vec<_>>(),
        "source_embedder_vector_sha256": vector_sha256(&embedder_output.vector),
        "model_version": embedder_output.model_version,
    });
    write_json_checked(&output_path, &output_json)?;
    let output_readback: Value = serde_json::from_slice(&fs::read(&output_path)?)?;
    require(
        "head output readback hash",
        output_readback["vector_sha256"] == output_json["vector_sha256"],
    )?;
    require(
        "head output readback dimension",
        output_readback["vector_len"] == json!(InstrumentSlot::ETrace.dim()),
    )?;
    require(
        "head output readback non-zero",
        output_readback["vector_l2_norm"]
            .as_f64()
            .unwrap_or_default()
            > 0.0,
    )?;

    let edge_cases = vec![
        edge_source_mismatch(&root, &embedder_output)?,
        edge_slot_mismatch(&root, &embedder_output)?,
        edge_disallowed_embedder_for_slot(&root, &embedder_output)?,
    ];
    let evidence = json!({
        "source_of_truth": {
            "evidence_json": evidence_path,
            "head_output_json": output_path,
            "models_config": config_path,
            "model_registration": registration,
        },
        "happy_path": {
            "before_state": before,
            "trigger": "real E10 PretrainedEmbedderForward output projected by EmbedderBackedInstrumentHead(ETrace)",
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
        "evidence readback head hash",
        evidence_readback["happy_path"]["after_state"]["vector_sha256"]
            == output_json["vector_sha256"],
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn edge_source_mismatch(
    root: &Path,
    output: &context_graph_mejepa_embedders::EmbedderOutput,
) -> FsvResult<Value> {
    let before = json!({"files_before": file_count(root)?, "head_source_id": "other-source", "output_source_id": output.source_id});
    let head = EmbedderBackedInstrumentHead::new(InstrumentSlot::ETrace)?;
    let err = head
        .encode(&EmbedderHeadInput {
            slot: InstrumentSlot::ETrace,
            source_id: "other-source".to_string(),
            embedder_output: output.clone(),
            analyzer_features: vec![0.1],
        })
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "source mismatch fails closed",
        err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
    )?;
    require(
        "source mismatch writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_source_mismatch",
        "before_state": before,
        "trigger": "ETrace head receives real E10 output under a different source_id",
        "after_state": after,
        "passes": true,
    }))
}

fn edge_slot_mismatch(
    root: &Path,
    output: &context_graph_mejepa_embedders::EmbedderOutput,
) -> FsvResult<Value> {
    let before = json!({"files_before": file_count(root)?, "head_slot": "e_trace", "input_slot": "e_reasoning"});
    let head = EmbedderBackedInstrumentHead::new(InstrumentSlot::ETrace)?;
    let err = head
        .encode(&EmbedderHeadInput {
            slot: InstrumentSlot::EReasoning,
            source_id: output.source_id.clone(),
            embedder_output: output.clone(),
            analyzer_features: vec![0.1],
        })
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "slot mismatch fails closed",
        err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
    )?;
    require(
        "slot mismatch writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_slot_mismatch",
        "before_state": before,
        "trigger": "ETrace head receives input tagged EReasoning",
        "after_state": after,
        "passes": true,
    }))
}

fn edge_disallowed_embedder_for_slot(
    root: &Path,
    output: &context_graph_mejepa_embedders::EmbedderOutput,
) -> FsvResult<Value> {
    let before = json!({"files_before": file_count(root)?, "slot": "e_ast", "actual_output_embedder": output.embedder.slug()});
    let head = EmbedderBackedInstrumentHead::new(InstrumentSlot::EAst)?;
    let err = head
        .encode(&EmbedderHeadInput {
            slot: InstrumentSlot::EAst,
            source_id: output.source_id.clone(),
            embedder_output: output.clone(),
            analyzer_features: vec![0.1, 0.2],
        })
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "disallowed embedder fails closed",
        err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
    )?;
    require(
        "disallowed embedder writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_disallowed_embedder_for_slot",
        "before_state": before,
        "trigger": "EAst head receives real E10 neural output instead of required E7",
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
        Err(format!("embedder heads FSV assertion failed: {label}").into())
    }
}
