use context_graph_mejepa_instruments::e_code::{
    analyze_python_semantic_facts, CodeInstrumentInput, EAstInstrument, ECfgInstrument,
    EDataFlowInstrument, ETypeGraphInstrument, PythonSemanticFacts,
};
use context_graph_mejepa_instruments::{Instrument, InstrumentSlot};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct SlotReadback {
    slot: String,
    path: PathBuf,
    vector_len: usize,
    vector_non_zero_count: usize,
    vector_sha256: String,
    first_8: Vec<f32>,
    readback_sha256: String,
    mode: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> FsvResult<()> {
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-python-semantic-analyzers-fsv");
    reset_root(&root)?;
    let source_dir = root.join("sources");
    let output_dir = root.join("slot-outputs");
    let tool_dir = root.join("tool-output");
    fs::create_dir_all(&source_dir)?;
    fs::create_dir_all(&output_dir)?;
    fs::create_dir_all(&tool_dir)?;
    fs::set_permissions(&source_dir, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&output_dir, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&tool_dir, fs::Permissions::from_mode(0o700))?;

    let evidence_path = root.join("python-semantic-analyzers-evidence.json");
    let source_path = source_dir.join("solver.py");
    let source = happy_source();
    write_bytes_checked(&source_path, source.as_bytes(), 0o600)?;
    require("source file exists after write", source_path.exists())?;

    let before_files = count_files(&output_dir)?;
    let input = CodeInstrumentInput {
        language: "python".into(),
        path: source_path.to_string_lossy().to_string(),
        source: source.into(),
    };
    let facts = analyze_python_semantic_facts(&input)?;
    assert_happy_facts(&facts)?;
    let py_compile = run_tool_jsonish(
        &tool_dir.join("py-compile.json"),
        "python3",
        &["-m", "py_compile", source_path.to_str().unwrap()],
    )?;
    let ruff = run_tool_jsonish(
        &tool_dir.join("ruff-check.json"),
        "ruff",
        &[
            "check",
            "--output-format",
            "json",
            source_path.to_str().unwrap(),
        ],
    )?;
    let pyright = run_tool_jsonish(
        &tool_dir.join("pyright-output.json"),
        "pyright",
        &["--outputjson", source_path.to_str().unwrap()],
    )?;
    require("python py_compile must pass", py_compile.status_success)?;
    require("ruff check must pass", ruff.status_success)?;
    require("pyright must pass", pyright.status_success)?;

    let slot_rows = vec![
        write_slot_output(&output_dir, InstrumentSlot::EAst, &EAstInstrument, &input)?,
        write_slot_output(&output_dir, InstrumentSlot::ECfg, &ECfgInstrument, &input)?,
        write_slot_output(
            &output_dir,
            InstrumentSlot::EDataFlow,
            &EDataFlowInstrument,
            &input,
        )?,
        write_slot_output(
            &output_dir,
            InstrumentSlot::ETypeGraph,
            &ETypeGraphInstrument,
            &input,
        )?,
    ];
    let after_files = count_files(&output_dir)?;
    require(
        "four happy-path slot outputs are physically present",
        after_files == before_files + 4,
    )?;

    let edge_cases = vec![
        edge_empty_source(&output_dir)?,
        edge_invalid_syntax(&output_dir)?,
        edge_non_python_semantic_entrypoint(&output_dir)?,
        edge_max_function_count(&source_dir, &output_dir)?,
    ];

    let evidence = json!({
        "passes": true,
        "source_of_truth": {
            "source_file": source_path,
            "slot_output_dir": output_dir,
            "tool_output_dir": tool_dir,
            "evidence_json": evidence_path,
        },
        "happy_path": {
            "before_state": {
                "slot_output_files": before_files,
            },
            "trigger": "Run Python Ruff semantic analyzers for E_AST/E_CFG/E_DataFlow/E_TypeGraph, then persist and read back each slot vector plus external analyzer tool evidence",
            "expected": {
                "function_count": 1,
                "class_count": 1,
                "cfg_branch_count_min": 1,
                "cfg_loop_back_edge_count_min": 1,
                "data_undefined_read_count": 0,
                "type_union_optional_count_min": 1,
                "slot_output_files_added": 4,
            },
            "after_state": {
                "facts": facts,
                "slot_output_files": after_files,
                "slot_rows": slot_rows,
                "tool_rows": [py_compile, ruff, pyright],
            }
        },
        "edge_cases": edge_cases,
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require("evidence readback passes", readback["passes"] == true)?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn happy_source() -> &'static str {
    r#"from typing import Any

class Solver:
    def compute(self, values: list[int] | None, meta: Any = None) -> int:
        total: int = 0
        if values is None:
            return 0
        for value in values:
            total += value
        return total

solver = Solver()
answer: int = solver.compute([1, 2], meta=None)
"#
}

fn assert_happy_facts(facts: &PythonSemanticFacts) -> FsvResult<()> {
    require(
        "facts analyzer is Python Ruff semantic v1",
        facts.analyzer == "python_ruff_semantic_v1",
    )?;
    require("facts include one function", facts.function_count == 1)?;
    require("facts include one class", facts.class_count == 1)?;
    require("CFG branch count populated", facts.cfg_branch_count >= 1)?;
    require(
        "CFG loopback count populated",
        facts.cfg_loop_back_edge_count >= 1,
    )?;
    require(
        "Data-flow def-use edges populated",
        facts.data_def_use_edge_count >= 1,
    )?;
    require(
        "No undefined reads in happy source",
        facts.data_undefined_read_count == 0,
    )?;
    require(
        "Type graph has annotations",
        facts.type_annotated_binding_count >= 4,
    )?;
    require(
        "Type graph has return annotation",
        facts.type_return_annotation_count >= 1,
    )?;
    require(
        "Type graph has generic annotation",
        facts.type_generic_count >= 1,
    )?;
    require(
        "Type graph has union/optional annotation",
        facts.type_union_optional_count >= 1,
    )?;
    require(
        "Type graph has call-site evidence",
        facts.type_call_site_count >= 2,
    )?;
    require(
        "Type graph has known typed call-site evidence",
        facts.type_known_call_site_count >= 1 && facts.type_typed_argument_count >= 1,
    )?;
    require(
        "Type graph has return-edge evidence",
        facts.type_return_edge_count >= 1,
    )?;
    Ok(())
}

fn write_slot_output<I>(
    output_dir: &Path,
    slot: InstrumentSlot,
    instrument: &I,
    input: &CodeInstrumentInput,
) -> FsvResult<SlotReadback>
where
    I: Instrument<Input = CodeInstrumentInput>,
{
    write_slot_output_named(output_dir, "", slot, instrument, input)
}

fn write_slot_output_named<I>(
    output_dir: &Path,
    name_prefix: &str,
    slot: InstrumentSlot,
    instrument: &I,
    input: &CodeInstrumentInput,
) -> FsvResult<SlotReadback>
where
    I: Instrument<Input = CodeInstrumentInput>,
{
    let vector = instrument.encode(input)?;
    require(
        "slot vector length matches declared slot",
        vector.len() == slot.dim(),
    )?;
    require(
        "slot vector has finite values",
        vector.iter().all(|value| value.is_finite()),
    )?;
    require(
        "slot vector has non-zero signal",
        vector.iter().any(|value| *value != 0.0),
    )?;
    let vector_sha256 = vector_sha256(&vector);
    let path = if name_prefix.is_empty() {
        output_dir.join(format!("{}-vector.json", slot.slug()))
    } else {
        output_dir.join(format!("{name_prefix}-{}-vector.json", slot.slug()))
    };
    let payload = json!({
        "slot": slot.slug(),
        "vector_len": vector.len(),
        "vector_non_zero_count": vector.iter().filter(|value| **value != 0.0).count(),
        "vector_sha256": vector_sha256,
        "first_8": vector.iter().take(8).copied().collect::<Vec<_>>(),
        "vector": vector,
    });
    write_json_checked(&path, &payload)?;
    let readback: Value = serde_json::from_slice(&fs::read(&path)?)?;
    let mode = format!("{:o}", fs::metadata(&path)?.permissions().mode() & 0o777);
    require("slot vector mode is 0600", mode == "600")?;
    require(
        "slot readback hash matches",
        readback["vector_sha256"] == payload["vector_sha256"],
    )?;
    Ok(SlotReadback {
        slot: slot.slug().to_string(),
        path,
        vector_len: readback["vector_len"].as_u64().unwrap_or(0) as usize,
        vector_non_zero_count: readback["vector_non_zero_count"].as_u64().unwrap_or(0) as usize,
        vector_sha256: vector_sha256.clone(),
        first_8: vector.iter().take(8).copied().collect(),
        readback_sha256: readback["vector_sha256"].as_str().unwrap_or("").to_string(),
        mode,
    })
}

#[derive(Debug, Serialize)]
struct ToolRow {
    command: String,
    args: Vec<String>,
    path: PathBuf,
    status_code: Option<i32>,
    status_success: bool,
    stdout_sha256: String,
    stderr_sha256: String,
    mode: String,
}

fn run_tool_jsonish(path: &Path, command: &str, args: &[&str]) -> FsvResult<ToolRow> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|err| format!("failed to execute required analyzer tool {command}: {err}"))?;
    let row = json!({
        "command": command,
        "args": args,
        "status_code": output.status.code(),
        "status_success": output.status.success(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "stdout_sha256": hex_sha256(&output.stdout),
        "stderr_sha256": hex_sha256(&output.stderr),
    });
    write_json_checked(path, &row)?;
    let readback: Value = serde_json::from_slice(&fs::read(path)?)?;
    require(
        "tool output readback status matches",
        readback["status_success"] == row["status_success"],
    )?;
    Ok(ToolRow {
        command: command.to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        path: path.to_path_buf(),
        status_code: output.status.code(),
        status_success: output.status.success(),
        stdout_sha256: hex_sha256(&output.stdout),
        stderr_sha256: hex_sha256(&output.stderr),
        mode: format!("{:o}", fs::metadata(path)?.permissions().mode() & 0o777),
    })
}

fn edge_empty_source(output_dir: &Path) -> FsvResult<Value> {
    let before = count_files(output_dir)?;
    let input = CodeInstrumentInput {
        language: "python".into(),
        path: "edge/empty.py".into(),
        source: String::new(),
    };
    let err = EAstInstrument.encode(&input).unwrap_err();
    let after = count_files(output_dir)?;
    Ok(json!({
        "name": "empty_source_fails_closed",
        "before_state": {"slot_output_files": before},
        "trigger": "Encode empty Python source",
        "expected_error": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_error": err.code(),
        "after_state": {"slot_output_files": after},
        "passes": err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT" && before == after,
    }))
}

fn edge_invalid_syntax(output_dir: &Path) -> FsvResult<Value> {
    let before = count_files(output_dir)?;
    let input = CodeInstrumentInput {
        language: "python".into(),
        path: "edge/invalid.py".into(),
        source: "def broken(:\n    return 1\n".into(),
    };
    let err = ECfgInstrument.encode(&input).unwrap_err();
    let after = count_files(output_dir)?;
    Ok(json!({
        "name": "invalid_syntax_fails_closed",
        "before_state": {"slot_output_files": before},
        "trigger": "Encode invalid Python syntax",
        "expected_error": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_error": err.code(),
        "after_state": {"slot_output_files": after},
        "passes": err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT" && before == after,
    }))
}

fn edge_non_python_semantic_entrypoint(output_dir: &Path) -> FsvResult<Value> {
    let before = count_files(output_dir)?;
    let input = CodeInstrumentInput {
        language: "rust".into(),
        path: "edge/lib.rs".into(),
        source: "pub fn add(a: i32, b: i32) -> i32 { a + b }\n".into(),
    };
    let err = analyze_python_semantic_facts(&input).unwrap_err();
    let after = count_files(output_dir)?;
    Ok(json!({
        "name": "non_python_semantic_entrypoint_fails_closed",
        "before_state": {"slot_output_files": before},
        "trigger": "Call Python semantic fact entrypoint with Rust source",
        "expected_error": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_error": err.code(),
        "after_state": {"slot_output_files": after},
        "passes": err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT" && before == after,
    }))
}

fn edge_max_function_count(source_dir: &Path, output_dir: &Path) -> FsvResult<Value> {
    let before = count_files(output_dir)?;
    let mut source = String::new();
    for i in 0..64 {
        source.push_str(&format!(
            "def f_{i}(x: int) -> int:\n    return x + {i}\n\n"
        ));
    }
    let source_path = source_dir.join("max_functions.py");
    write_bytes_checked(&source_path, source.as_bytes(), 0o600)?;
    let input = CodeInstrumentInput {
        language: "python".into(),
        path: source_path.to_string_lossy().to_string(),
        source,
    };
    let facts = analyze_python_semantic_facts(&input)?;
    require("max edge has 64 functions", facts.function_count == 64)?;
    let row = write_slot_output_named(
        output_dir,
        "max64",
        InstrumentSlot::EAst,
        &EAstInstrument,
        &input,
    )?;
    let after = count_files(output_dir)?;
    Ok(json!({
        "name": "max_64_functions_edge_succeeds_with_exact_count",
        "before_state": {"slot_output_files": before},
        "trigger": "Encode generated Python source with 64 typed functions",
        "expected": {"function_count": 64, "slot_output_files_added": 1},
        "after_state": {"slot_output_files": after, "facts": facts, "slot_row": row},
        "passes": after == before + 1,
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

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_bytes_checked(path, &bytes, 0o600)
}

fn write_bytes_checked(path: &Path, bytes: &[u8], mode: u32) -> FsvResult<()> {
    let tmp = path.with_extension("tmp");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(mode)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    File::open(parent)?.sync_all()?;
    let readback = fs::read(path)?;
    require("write/readback bytes match", readback == bytes)?;
    Ok(())
}

fn count_files(root: &Path) -> FsvResult<usize> {
    if !root.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    hex_sha256(&bytes)
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(label.to_string().into())
    }
}
