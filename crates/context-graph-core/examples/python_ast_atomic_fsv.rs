use context_graph_core::memory::ast::{
    chunk_with_options, AstChunk, AstChunkOptions, EntityType, Language,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/python-ast-atomic-fsv");
    reset_secure_dir(&root)?;
    let sources = root.join("sources");
    fs::create_dir_all(&sources)?;

    let happy = run_happy_path(&sources)?;
    let empty = run_empty_edge();
    let invalid = run_invalid_syntax_edge(&sources)?;
    let maximum = run_maximum_class_edge(&sources)?;
    let evidence_path = root.join("python-ast-atomic-fsv.json");
    let evidence = json!({
        "source_of_truth": {
            "root": root.display().to_string(),
            "sources": sources.display().to_string(),
            "python_bytecode": sources.join("__pycache__").display().to_string(),
            "evidence_json": evidence_path.display().to_string(),
        },
        "happy_path": happy,
        "edge_cases": {
            "empty_input": empty,
            "invalid_syntax": invalid,
            "maximum_class": maximum,
        },
    });
    write_secure_json(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require("evidence readback must match write", readback == evidence)?;
    require(
        "happy path passes",
        readback["happy_path"]["passes"] == Value::Bool(true),
    )?;
    require(
        "all edges pass",
        readback["edge_cases"]
            .as_object()
            .unwrap()
            .values()
            .all(|edge| edge["passes"] == Value::Bool(true)),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fsv": "python_ast_atomic",
            "evidence_json": evidence_path,
            "happy_passes": readback["happy_path"]["passes"],
            "edge_cases": readback["edge_cases"],
        }))?
    );
    Ok(())
}

fn run_happy_path(sources: &Path) -> FsvResult<Value> {
    let source = r#""""Module contract."""
import os
from pathlib import Path

OFFSET = 41

def traceable(fn):
    return fn

@traceable
class Solver:
    """Solver contract."""
    label = "solver"

    def compute(self, x: int) -> int:
        return x + OFFSET

    def helper(self) -> int:
        return self.compute(1)

@traceable
def top_level() -> int:
    return Solver().helper()

def test_compute() -> None:
    assert Solver().compute(1) == 42
"#;
    let path = sources.join("happy_atomic.py");
    write_checked(&path, source.as_bytes())?;
    let before = json!({
        "path": path.display().to_string(),
        "source_bytes": source.len(),
        "source_sha256": sha256(source.as_bytes()),
        "pyc_files_before": pyc_files(sources)?,
    });
    println!("before happy_path: {}", serde_json::to_string(&before)?);
    let compile = py_compile(&path)?;
    require(
        "happy py_compile succeeds",
        compile["success"] == Value::Bool(true),
    )?;
    let source_readback = fs::read(&path)?;
    let chunks = chunk_with_options(
        &source_readback,
        Language::Python,
        &AstChunkOptions::for_path("tests/test_solver.py"),
    )?;
    let after = python_atomic_state(sources, &chunks)?;
    println!("after happy_path: {}", serde_json::to_string(&after)?);
    let class_source = chunk_source(
        chunks
            .iter()
            .find(|chunk| chunk.entity_type == EntityType::Class)
            .ok_or("missing class chunk")?,
    );
    let module_source = chunk_source(
        chunks
            .iter()
            .find(|chunk| chunk.entity_type == EntityType::Module)
            .ok_or("missing module globals chunk")?,
    );
    let passes = compile["success"] == Value::Bool(true)
        && after["method_count"] == Value::from(2)
        && after["has_top_level_function"] == Value::Bool(true)
        && after["has_test_function"] == Value::Bool(true)
        && class_source.contains("class Solver")
        && class_source.contains("label = \"solver\"")
        && !class_source.contains("def compute")
        && !class_source.contains("return x + OFFSET")
        && module_source.contains("OFFSET = 41")
        && !module_source.contains("class Solver")
        && !module_source.contains("def top_level");
    Ok(json!({
        "before_state": before,
        "action": {
            "py_compile": compile,
            "chunker": "context_graph_core::memory::ast::chunk_with_options",
        },
        "after_state": after,
        "passes": passes,
    }))
}

fn run_empty_edge() -> Value {
    let before = json!({
        "source_bytes": 0,
        "expected_code": "MEJEPA_CHUNKER_EMPTY_SOURCE",
    });
    println!("before empty_input: {}", before);
    let err = chunk_with_options(
        b"",
        Language::Python,
        &AstChunkOptions::for_path("empty.py"),
    )
    .unwrap_err();
    let after = json!({
        "accepted": false,
        "actual_code": err.code(),
        "error": err.to_string(),
    });
    println!("after empty_input: {}", after);
    json!({
        "before_state": before,
        "after_state": after,
        "passes": err.code() == "MEJEPA_CHUNKER_EMPTY_SOURCE",
    })
}

fn run_invalid_syntax_edge(sources: &Path) -> FsvResult<Value> {
    let source = b"def broken(:\n    return 1\n";
    let path = sources.join("invalid_syntax.py");
    write_checked(&path, source)?;
    let before = json!({
        "path": path.display().to_string(),
        "source_bytes": source.len(),
        "source_sha256": sha256(source),
        "expected_code": "MEJEPA_CHUNKER_PARSE_FAILED",
    });
    println!("before invalid_syntax: {}", serde_json::to_string(&before)?);
    let compile = py_compile(&path)?;
    let err = chunk_with_options(
        source,
        Language::Python,
        &AstChunkOptions::for_path("invalid_syntax.py"),
    )
    .unwrap_err();
    let after = json!({
        "py_compile": compile,
        "accepted": false,
        "actual_code": err.code(),
        "error": err.to_string(),
    });
    println!("after invalid_syntax: {}", serde_json::to_string(&after)?);
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "passes": err.code() == "MEJEPA_CHUNKER_PARSE_FAILED"
            && after["py_compile"]["success"] == Value::Bool(false),
    }))
}

fn run_maximum_class_edge(sources: &Path) -> FsvResult<Value> {
    let mut source = String::from("class Bulk:\n    label = \"bulk\"\n");
    for index in 0..40 {
        source.push_str(&format!(
            "\n    def method_{index}(self) -> int:\n        return {index}\n"
        ));
    }
    let path = sources.join("maximum_class.py");
    write_checked(&path, source.as_bytes())?;
    let before = json!({
        "path": path.display().to_string(),
        "source_bytes": source.len(),
        "source_sha256": sha256(source.as_bytes()),
        "expected_methods": 40,
    });
    println!("before maximum_class: {}", serde_json::to_string(&before)?);
    let compile = py_compile(&path)?;
    require(
        "maximum py_compile succeeds",
        compile["success"] == Value::Bool(true),
    )?;
    let source_readback = fs::read(&path)?;
    let chunks = chunk_with_options(
        &source_readback,
        Language::Python,
        &AstChunkOptions::for_path("pkg/maximum_class.py"),
    )?;
    let after = python_atomic_state(sources, &chunks)?;
    println!("after maximum_class: {}", serde_json::to_string(&after)?);
    let class_source = chunk_source(
        chunks
            .iter()
            .find(|chunk| chunk.entity_type == EntityType::Class)
            .ok_or("missing maximum class chunk")?,
    );
    Ok(json!({
        "before_state": before,
        "action": { "py_compile": compile },
        "after_state": after,
        "passes": after["method_count"] == Value::from(40)
            && class_source.contains("class Bulk")
            && class_source.contains("label = \"bulk\"")
            && !class_source.contains("def method_0")
            && !class_source.contains("return 39"),
    }))
}

fn python_atomic_state(sources: &Path, chunks: &[AstChunk]) -> FsvResult<Value> {
    let rows = chunks
        .iter()
        .map(|chunk| {
            json!({
                "entity_type": chunk.entity_type,
                "parent_chain": chunk.parent_chain,
                "line_start": chunk.line_start,
                "line_end": chunk.line_end,
                "start_byte": chunk.start_byte,
                "end_byte": chunk.end_byte,
                "sha256": chunk.sha256,
                "source": chunk_source(chunk),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "pyc_files_after": pyc_files(sources)?,
        "chunk_count": chunks.len(),
        "method_count": chunks.iter().filter(|chunk| chunk.entity_type == EntityType::Method).count(),
        "class_count": chunks.iter().filter(|chunk| chunk.entity_type == EntityType::Class).count(),
        "module_count": chunks.iter().filter(|chunk| chunk.entity_type == EntityType::Module).count(),
        "has_top_level_function": chunks.iter().any(|chunk| chunk.entity_type == EntityType::Function && chunk_source(chunk).contains("def top_level")),
        "has_test_function": chunks.iter().any(|chunk| chunk.entity_type == EntityType::TestFunction),
        "method_parent_chains": chunks.iter()
            .filter(|chunk| chunk.entity_type == EntityType::Method)
            .map(|chunk| chunk.parent_chain.clone())
            .collect::<Vec<_>>(),
        "chunks": rows,
    }))
}

fn chunk_source(chunk: &AstChunk) -> String {
    chunk
        .content
        .split_once("--- source ---\n")
        .map(|(_, source)| source.to_string())
        .unwrap_or_default()
}

fn py_compile(path: &Path) -> FsvResult<Value> {
    let output = Command::new("python3")
        .arg("-m")
        .arg("py_compile")
        .arg(path)
        .output()?;
    Ok(json!({
        "command": format!("python3 -m py_compile {}", path.display()),
        "success": output.status.success(),
        "exit_code": output.status.code().unwrap_or(-1),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
    }))
}

fn pyc_files(sources: &Path) -> FsvResult<Vec<Value>> {
    let cache = sources.join("__pycache__");
    if !cache.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(cache)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("pyc") {
            let bytes = fs::read(&path)?;
            files.push(json!({
                "path": path.display().to_string(),
                "bytes": bytes.len(),
                "sha256": sha256(&bytes),
            }));
        }
    }
    files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    Ok(files)
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let readback = fs::read(path)?;
    require(
        &format!("readback mismatch for {}", path.display()),
        readback == bytes,
    )?;
    Ok(())
}

fn write_secure_json(path: &Path, value: &Value) -> FsvResult<()> {
    let text = serde_json::to_string_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    require(
        &format!("secure JSON readback mismatch for {}", path.display()),
        readback == *value,
    )?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("Python AST atomic FSV invariant failed: {label}").into())
    }
}
