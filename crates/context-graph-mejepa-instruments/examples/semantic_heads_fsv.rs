use context_graph_mejepa_embedders::{
    EmbedderForward, EmbedderId, EmbedderInput, ModelsConfig, PretrainedEmbedderForward,
};
use context_graph_mejepa_instruments::e_embedder_heads::{
    AnalyzerFeatureEvidence, EmbedderBackedInstrumentHead, EmbedderHeadInput,
};
use context_graph_mejepa_instruments::{
    CodeInstrumentInput, DiffInstrumentInput, EAstInstrument, ECfgInstrument, EDataFlowInstrument,
    EDiffInstrument, EReasoningInstrument, ERuntimeInstrument, ETestInstrument, ETraceInstrument,
    ETypeGraphInstrument, Instrument, InstrumentSlot, ReasoningEvent, ReasoningInput, RuntimeInput,
    TextInstrumentInput, TraceEvent, TraceEventKind, TraceInput,
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
    let root =
        PathBuf::from("/zfs/archive/contextgraph/fsv/contextgraph-mejepa-semantic-heads-fsv");
    reset_root(&root)?;
    let output_dir = root.join("head-outputs");
    fs::create_dir_all(&output_dir)?;
    fs::set_permissions(&output_dir, fs::Permissions::from_mode(0o700))?;
    let evidence_path = root.join("semantic-heads-evidence.json");
    let config_path = std::env::var("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("/zfs/hot/contextgraph/models/mejepa_models_config.toml")
        });

    let config = ModelsConfig::load(&config_path)?;
    let e7_forward = PretrainedEmbedderForward::load(config.registration(EmbedderId::E7)?).await?;
    let e1_forward = PretrainedEmbedderForward::load(config.registration(EmbedderId::E1)?).await?;
    let e10_forward =
        PretrainedEmbedderForward::load(config.registration(EmbedderId::E10)?).await?;

    let code_source = code_source();
    let code_source_id = "semantic-head-fsv/code/sample.py".to_string();
    let code_output = e7_forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E7,
            text: code_source.to_string(),
            source_id: code_source_id.clone(),
        })
        .await?;
    let code_input = CodeInstrumentInput {
        language: "python".into(),
        path: code_source_id.clone(),
        source: code_source.to_string(),
    };
    let code_slots = [
        (
            InstrumentSlot::EAst,
            AnalyzerFeatureEvidence::from_instrument(
                code_source_id.clone(),
                "context-graph-mejepa-instruments::EAstInstrument",
                "code-analyzer-v1",
                &EAstInstrument,
                &code_input,
            )?,
        ),
        (
            InstrumentSlot::ECfg,
            AnalyzerFeatureEvidence::from_instrument(
                code_source_id.clone(),
                "context-graph-mejepa-instruments::ECfgInstrument",
                "code-analyzer-v1",
                &ECfgInstrument,
                &code_input,
            )?,
        ),
        (
            InstrumentSlot::EDataFlow,
            AnalyzerFeatureEvidence::from_instrument(
                code_source_id.clone(),
                "context-graph-mejepa-instruments::EDataFlowInstrument",
                "code-analyzer-v1",
                &EDataFlowInstrument,
                &code_input,
            )?,
        ),
        (
            InstrumentSlot::ETypeGraph,
            AnalyzerFeatureEvidence::from_instrument(
                code_source_id.clone(),
                "context-graph-mejepa-instruments::ETypeGraphInstrument",
                "code-analyzer-v1",
                &ETypeGraphInstrument,
                &code_input,
            )?,
        ),
    ];

    let mut head_rows = Vec::new();
    for (slot, evidence) in code_slots {
        head_rows.push(write_head_output(
            &output_dir,
            slot,
            code_output.clone(),
            vec![evidence],
        )?);
    }

    let diff_source_id = "semantic-head-fsv/diff/sample.py".to_string();
    let diff_after = code_source.replace("total -= value", "total -= value + 1");
    let diff_output = e7_forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E7,
            text: diff_after.clone(),
            source_id: diff_source_id.clone(),
        })
        .await?;
    let diff_input = DiffInstrumentInput {
        language: "python".into(),
        path: diff_source_id.clone(),
        before_source: code_source.to_string(),
        after_source: diff_after,
    };
    head_rows.push(write_head_output(
        &output_dir,
        InstrumentSlot::EDiff,
        diff_output,
        vec![AnalyzerFeatureEvidence::from_instrument(
            diff_source_id,
            "context-graph-mejepa-instruments::EDiffInstrument",
            "diff-analyzer-v1",
            &EDiffInstrument,
            &diff_input,
        )?],
    )?);

    let test_source_id = "semantic-head-fsv/test/test.txt".to_string();
    let test_text = "def test_classify_values():\n    assert classify_values([2, -3, 4]) == 4\n";
    let test_output = e1_forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E1,
            text: test_text.to_string(),
            source_id: test_source_id.clone(),
        })
        .await?;
    let test_input = TextInstrumentInput {
        text: test_text.into(),
        source_id: test_source_id.clone(),
        language: Some("python".into()),
    };
    head_rows.push(write_head_output(
        &output_dir,
        InstrumentSlot::ETest,
        test_output,
        vec![AnalyzerFeatureEvidence::from_instrument(
            test_source_id,
            "context-graph-mejepa-instruments::ETestInstrument",
            "test-text-analyzer-v1",
            &ETestInstrument,
            &test_input,
        )?],
    )?);

    let trace_source_id = "semantic-head-fsv/trace/trace.txt".to_string();
    let trace_input = trace_input("semantic_heads_fsv::classify_values");
    let trace_output = e10_forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E10,
            text: "enter classify_values line=2 value_hash=sha256:2 exit line=8 result=4".into(),
            source_id: trace_source_id.clone(),
        })
        .await?;
    head_rows.push(write_head_output(
        &output_dir,
        InstrumentSlot::ETrace,
        trace_output,
        vec![AnalyzerFeatureEvidence::from_instrument(
            trace_source_id,
            "context-graph-mejepa-instruments::ETraceInstrument",
            "trace-analyzer-v1",
            &ETraceInstrument,
            &trace_input,
        )?],
    )?);

    let reasoning_source_id = "semantic-head-fsv/reasoning/reasoning.txt".to_string();
    let reasoning_input = ReasoningInput {
        task_id: "semantic-head-fsv".into(),
        transcript:
            "Verified the analyzer-backed semantic heads against real neural forward outputs."
                .into(),
        events: vec![ReasoningEvent {
            actor: "codex".into(),
            event_type: "fsv_assertion".into(),
            text: "Every persisted head output was read back and hash-checked.".into(),
        }],
    };
    let reasoning_output = e10_forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E10,
            text: reasoning_input.transcript.clone(),
            source_id: reasoning_source_id.clone(),
        })
        .await?;
    head_rows.push(write_head_output(
        &output_dir,
        InstrumentSlot::EReasoning,
        reasoning_output,
        vec![AnalyzerFeatureEvidence::from_instrument(
            reasoning_source_id,
            "context-graph-mejepa-instruments::EReasoningInstrument",
            "reasoning-analyzer-v1",
            &EReasoningInstrument,
            &reasoning_input,
        )?],
    )?);

    let runtime_vector = ERuntimeInstrument.encode(&runtime_input())?;
    let runtime_output_path = output_dir.join("e_runtime-analyzer-output.json");
    let runtime_output = json!({
        "slot": InstrumentSlot::ERuntime.slug(),
        "source": "context-graph-mejepa-instruments::ERuntimeInstrument",
        "vector_len": runtime_vector.len(),
        "vector_l2_norm": l2_norm(&runtime_vector),
        "vector_sha256": vector_sha256(&runtime_vector),
        "first_8": runtime_vector.iter().take(8).copied().collect::<Vec<_>>()
    });
    write_json_checked(&runtime_output_path, &runtime_output)?;
    let runtime_readback: Value = serde_json::from_slice(&fs::read(&runtime_output_path)?)?;
    require(
        "runtime readback hash",
        runtime_readback["vector_sha256"] == runtime_output["vector_sha256"],
    )?;

    let edge_cases = vec![
        edge_missing_analyzer_evidence(&root, &code_output)?,
        edge_analyzer_source_mismatch(&root, &code_output, &code_input)?,
        edge_nonfinite_analyzer_evidence(&root, &code_output, &code_input)?,
        edge_invalid_runtime(&root)?,
    ];
    let evidence = json!({
        "source_of_truth": {
            "evidence_json": evidence_path,
            "head_output_dir": output_dir,
            "models_config": config_path,
            "runtime_output_json": runtime_output_path,
        },
        "happy_path": {
            "before_state": {
                "code_source_id": code_source_id,
                "code_source_bytes": code_source.len(),
                "code_embedder": code_output.embedder.slug(),
                "code_embedder_model_version": code_output.model_version,
                "head_output_files_before": 0,
            },
            "trigger": "real pretrained E7/E1/E10 forward outputs joined with typed analyzer evidence and projected through EmbedderBackedInstrumentHead",
            "after_state": {
                "head_rows": head_rows,
                "runtime_analyzer_output": runtime_output,
                "head_output_file_count": file_count(&output_dir)?,
            },
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
        "evidence readback has 8 embedder-backed rows",
        evidence_readback["happy_path"]["after_state"]["head_rows"]
            .as_array()
            .map(Vec::len)
            == Some(8),
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn write_head_output(
    output_dir: &Path,
    slot: InstrumentSlot,
    embedder_output: context_graph_mejepa_embedders::EmbedderOutput,
    analyzer_evidence: Vec<AnalyzerFeatureEvidence>,
) -> FsvResult<Value> {
    let output_path = output_dir.join(format!("{}-head-output.json", slot.slug()));
    let before = json!({
        "slot": slot.slug(),
        "embedder": embedder_output.embedder.slug(),
        "source_id": embedder_output.source_id,
        "output_exists": output_path.exists(),
        "analyzer_evidence_count": analyzer_evidence.len(),
        "analyzer_vector_lens": analyzer_evidence.iter().map(|e| e.vector.len()).collect::<Vec<_>>(),
    });
    let head = EmbedderBackedInstrumentHead::new(slot)?;
    let vector = head.encode(&EmbedderHeadInput {
        slot,
        source_id: embedder_output.source_id.clone(),
        embedder_output: embedder_output.clone(),
        analyzer_evidence,
    })?;
    let output_json = json!({
        "slot": slot.slug(),
        "embedder": embedder_output.embedder.slug(),
        "source_id": embedder_output.source_id,
        "vector_len": vector.len(),
        "vector_l2_norm": l2_norm(&vector),
        "vector_sha256": vector_sha256(&vector),
        "first_8": vector.iter().take(8).copied().collect::<Vec<_>>(),
        "source_embedder_vector_sha256": vector_sha256(&embedder_output.vector),
        "model_version": embedder_output.model_version,
    });
    write_json_checked(&output_path, &output_json)?;
    let readback: Value = serde_json::from_slice(&fs::read(&output_path)?)?;
    require(
        &format!("{} output readback hash", slot.slug()),
        readback["vector_sha256"] == output_json["vector_sha256"],
    )?;
    require(
        &format!("{} output readback dim", slot.slug()),
        readback["vector_len"] == json!(slot.dim()),
    )?;
    require(
        &format!("{} output readback nonzero", slot.slug()),
        readback["vector_l2_norm"].as_f64().unwrap_or_default() > 0.0,
    )?;
    Ok(json!({
        "before_state": before,
        "trigger": format!("{} analyzer evidence + {} pretrained forward through EmbedderBackedInstrumentHead", slot.slug(), embedder_output.embedder.slug()),
        "after_state": output_json,
        "output_path": output_path,
        "passes": true,
    }))
}

fn edge_missing_analyzer_evidence(
    root: &Path,
    output: &context_graph_mejepa_embedders::EmbedderOutput,
) -> FsvResult<Value> {
    let before =
        json!({"files_before": file_count(root)?, "slot": "e_ast", "analyzer_evidence_count": 0});
    let err = EmbedderBackedInstrumentHead::new(InstrumentSlot::EAst)?
        .encode(&EmbedderHeadInput {
            slot: InstrumentSlot::EAst,
            source_id: output.source_id.clone(),
            embedder_output: output.clone(),
            analyzer_evidence: vec![],
        })
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "missing analyzer evidence fails closed",
        err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
    )?;
    require(
        "missing evidence writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_missing_analyzer_evidence",
        "before_state": before,
        "trigger": "EAst neural head receives real E7 output without analyzer evidence",
        "after_state": after,
        "passes": true,
    }))
}

fn edge_analyzer_source_mismatch(
    root: &Path,
    output: &context_graph_mejepa_embedders::EmbedderOutput,
    code_input: &CodeInstrumentInput,
) -> FsvResult<Value> {
    let before = json!({"files_before": file_count(root)?, "head_source_id": output.source_id, "evidence_source_id": "semantic-head-fsv/code/other.py"});
    let evidence = AnalyzerFeatureEvidence::from_instrument(
        "semantic-head-fsv/code/other.py",
        "context-graph-mejepa-instruments::EAstInstrument",
        "code-analyzer-v1",
        &EAstInstrument,
        code_input,
    )?;
    let err = EmbedderBackedInstrumentHead::new(InstrumentSlot::EAst)?
        .encode(&EmbedderHeadInput {
            slot: InstrumentSlot::EAst,
            source_id: output.source_id.clone(),
            embedder_output: output.clone(),
            analyzer_evidence: vec![evidence],
        })
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "analyzer source mismatch fails closed",
        err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
    )?;
    require(
        "source mismatch writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_analyzer_source_mismatch",
        "before_state": before,
        "trigger": "EAst head joins real E7 output with analyzer evidence from another source_id",
        "after_state": after,
        "passes": true,
    }))
}

fn edge_nonfinite_analyzer_evidence(
    root: &Path,
    output: &context_graph_mejepa_embedders::EmbedderOutput,
    code_input: &CodeInstrumentInput,
) -> FsvResult<Value> {
    let before =
        json!({"files_before": file_count(root)?, "slot": "e_ast", "mutated_feature": "NaN"});
    let mut evidence = AnalyzerFeatureEvidence::from_instrument(
        output.source_id.clone(),
        "context-graph-mejepa-instruments::EAstInstrument",
        "code-analyzer-v1",
        &EAstInstrument,
        code_input,
    )?;
    evidence.vector[0] = f32::NAN;
    let err = EmbedderBackedInstrumentHead::new(InstrumentSlot::EAst)?
        .encode(&EmbedderHeadInput {
            slot: InstrumentSlot::EAst,
            source_id: output.source_id.clone(),
            embedder_output: output.clone(),
            analyzer_evidence: vec![evidence],
        })
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "nonfinite analyzer evidence fails closed",
        err.code() == "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT",
    )?;
    require(
        "nonfinite evidence writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_nonfinite_analyzer_evidence",
        "before_state": before,
        "trigger": "EAst head receives analyzer evidence containing NaN",
        "after_state": after,
        "passes": true,
    }))
}

fn edge_invalid_runtime(root: &Path) -> FsvResult<Value> {
    let before = json!({"files_before": file_count(root)?, "wall_time_ms": 0, "timed_out": false});
    let err = ERuntimeInstrument
        .encode(&RuntimeInput {
            wall_time_ms: 0,
            peak_rss_bytes: 64 * 1024 * 1024,
            exit_code: 0,
            timed_out: false,
            coverage_percent: Some(90.0),
            network_events: 0,
            filesystem_writes: 1,
        })
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "invalid runtime fails closed",
        err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
    )?;
    require(
        "invalid runtime writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_invalid_runtime",
        "before_state": before,
        "trigger": "ERuntime analyzer receives zero wall_time_ms for a non-timeout run",
        "after_state": after,
        "passes": true,
    }))
}

fn code_source() -> &'static str {
    "def classify_values(values: list[int]) -> int:\n    total = 0\n    for value in values:\n        if value >= 0:\n            total += value\n        else:\n            total -= value\n    return total\n"
}

fn trace_input(function_id: &str) -> TraceInput {
    TraceInput {
        events: vec![
            TraceEvent {
                function_id: format!("{function_id}::enter"),
                line: 1,
                value_hash: "sha256:trace-enter".into(),
                timestamp_ms: 1,
                kind: TraceEventKind::FunctionEnter,
            },
            TraceEvent {
                function_id: format!("{function_id}::line_hit"),
                line: 4,
                value_hash: "sha256:trace-line".into(),
                timestamp_ms: 3,
                kind: TraceEventKind::LineHit,
            },
            TraceEvent {
                function_id: format!("{function_id}::exit"),
                line: 8,
                value_hash: "sha256:trace-exit".into(),
                timestamp_ms: 7,
                kind: TraceEventKind::FunctionExit,
            },
        ],
    }
}

fn runtime_input() -> RuntimeInput {
    RuntimeInput {
        wall_time_ms: 37,
        peak_rss_bytes: 64 * 1024 * 1024,
        exit_code: 0,
        timed_out: false,
        coverage_percent: Some(93.5),
        network_events: 0,
        filesystem_writes: 2,
    }
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
        Err(format!("semantic heads FSV assertion failed: {label}").into())
    }
}
