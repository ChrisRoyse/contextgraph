#![allow(deprecated)] // This FSV intentionally verifies the deprecated bridge.

use context_graph_core::memory::ast::{chunk_path, EntityType as CanonicalEntityType};
use context_graph_core::memory::{AstChunkConfig, AstChunkerError, AstCodeChunker, EntityType};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/legacy-ast-chunker-parity-fsv");
    let sources = root.join("sources");
    let syntax = root.join("syntax");
    reset_secure_dir(&root)?;
    fs::create_dir_all(&sources)?;
    fs::create_dir_all(&syntax)?;

    let rust_path = sources.join("worker.rs");
    let python_path = sources.join("solver.py");
    write_secure_text(
        &rust_path,
        r#"
use std::collections::HashMap;

pub struct Worker {
    count: usize,
}

impl Worker {
    pub fn run(&self) -> usize {
        self.count
    }
}

pub fn build_map() -> HashMap<String, usize> {
    HashMap::from([(String::from("worker"), 1)])
}
"#,
    )?;
    write_secure_text(
        &python_path,
        r#"
import math

class Solver:
    def compute(self, value: int) -> int:
        return value + 1

def test_solver():
    assert Solver().compute(1) == 2
"#,
    )?;

    let rust_syntax = rustc_check(&rust_path, &syntax.join("worker.rmeta"))?;
    ensure(
        "rust syntax source of truth must compile",
        rust_syntax["success"] == Value::Bool(true),
    )?;
    let python_syntax = python_check(&python_path)?;
    ensure(
        "python syntax source of truth must compile",
        python_syntax["success"] == Value::Bool(true),
    )?;

    let happy_path = vec![
        parity_case(
            "rust_bridge_parity",
            &rust_path,
            &[EntityType::Struct, EntityType::Method, EntityType::Function],
        )?,
        parity_case(
            "python_bridge_parity",
            &python_path,
            &[
                EntityType::Class,
                EntityType::Method,
                EntityType::TestFunction,
            ],
        )?,
    ];
    let boundary_cases = vec![
        empty_boundary(&sources.join("empty.rs"))?,
        unsupported_boundary(&sources.join("notes.txt"))?,
        invalid_rust_boundary(&sources.join("invalid.rs"))?,
    ];

    let evidence_path = root.join("legacy-ast-chunker-parity-fsv.json");
    let evidence = json!({
        "fsv_root": root.display().to_string(),
        "task_id": "ISSUE-6",
        "source_of_truth": {
            "source_dir": sources.display().to_string(),
            "syntax_dir": syntax.display().to_string(),
            "evidence_json": evidence_path.display().to_string(),
        },
        "trigger": "legacy AstCodeChunker::default_multi_language + canonical memory::ast::chunk_path",
        "syntax_readback": {
            "rust": rust_syntax,
            "python": python_syntax,
        },
        "happy_path": happy_path,
        "boundary_cases": boundary_cases,
        "all_passed": true,
    });
    write_secure_json(&evidence_path, &evidence)?;

    let readback_text = fs::read_to_string(&evidence_path)?;
    let readback: Value = serde_json::from_str(&readback_text)?;
    ensure(
        "evidence JSON must read back byte-equivalent state",
        readback == evidence,
    )?;
    ensure(
        "all boundary cases must pass",
        readback["boundary_cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case| case["pass"] == Value::Bool(true)),
    )?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fsv": "legacy_ast_chunker_parity",
            "evidence": evidence_path,
            "evidence_bytes": readback_text.len(),
            "happy_cases": readback["happy_path"].as_array().unwrap().len(),
            "boundary_cases": readback["boundary_cases"].as_array().unwrap().len(),
            "all_passed": readback["all_passed"],
        }))?
    );
    Ok(())
}

fn parity_case(label: &str, path: &Path, expected: &[EntityType]) -> FsvResult<Value> {
    let source = fs::read_to_string(path)?;
    let path_str = path.display().to_string();
    let before = json!({
        "path": path_str,
        "source_bytes": source.len(),
        "source_sha256": sha256_bytes(source.as_bytes()),
    });

    let mut legacy = AstCodeChunker::default_multi_language()?;
    let legacy_chunks = legacy.chunk(&source, &path_str)?;
    let canonical_chunks = chunk_path(source.as_bytes(), &path_str)?;

    let legacy_rows = legacy_chunks
        .iter()
        .map(|chunk| {
            json!({
                "entity_type": format!("{:?}", chunk.metadata.entity_type),
                "language": chunk.metadata.language,
                "scope_chain": chunk.metadata.scope_chain,
                "line_start": chunk.metadata.start_line,
                "line_end": chunk.metadata.end_line,
                "code_sha256": sha256_bytes(chunk.code.as_bytes()),
            })
        })
        .collect::<Vec<_>>();
    let canonical_rows = canonical_chunks
        .iter()
        .map(|chunk| {
            json!({
                "entity_type": format!("{:?}", canonical_to_legacy_entity(chunk.entity_type)),
                "language": chunk.language.slug(),
                "scope_chain": canonical_scope_chain(chunk),
                "line_start": chunk.line_start,
                "line_end": chunk.line_end,
                "code_sha256": sha256_bytes(canonical_source(chunk).as_bytes()),
            })
        })
        .collect::<Vec<_>>();
    let expected_present = expected.iter().all(|entity_type| {
        legacy_chunks
            .iter()
            .any(|chunk| chunk.metadata.entity_type == *entity_type)
    });
    ensure(
        &format!("{label} legacy/canonical rows must match"),
        legacy_rows == canonical_rows,
    )?;
    ensure(
        &format!("{label} expected entity types present"),
        expected_present,
    )?;

    Ok(json!({
        "case": label,
        "sot": path.display().to_string(),
        "before": before,
        "after": {
            "legacy_chunk_count": legacy_chunks.len(),
            "canonical_chunk_count": canonical_chunks.len(),
            "rows": legacy_rows,
        },
        "expected": {
            "legacy_equals_canonical": true,
            "entity_types": expected.iter().map(|entity| format!("{entity:?}")).collect::<Vec<_>>(),
        },
        "actual": {
            "legacy_equals_canonical": true,
            "expected_present": expected_present,
        },
        "pass": true,
    }))
}

fn empty_boundary(path: &Path) -> FsvResult<Value> {
    write_secure_text(path, "   \n\t")?;
    let before = file_state(path)?;
    let legacy_err = legacy_error(path)?;
    let canonical_err = canonical_error(path)?;
    let pass = legacy_err["code"] == "MEJEPA_CHUNKER_EMPTY_SOURCE"
        && canonical_err["code"] == "MEJEPA_CHUNKER_EMPTY_SOURCE";
    Ok(json!({
        "case": "empty_input",
        "sot": path.display().to_string(),
        "before": before,
        "after": {"legacy": legacy_err, "canonical": canonical_err},
        "expected": "both fail closed as MEJEPA_CHUNKER_EMPTY_SOURCE",
        "actual": {"pass": pass},
        "pass": pass,
    }))
}

fn unsupported_boundary(path: &Path) -> FsvResult<Value> {
    write_secure_text(path, "plain text is not a code language\n")?;
    let before = file_state(path)?;
    let legacy_err = legacy_error(path)?;
    let canonical_err = canonical_error(path)?;
    let pass = legacy_err["code"] == "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE"
        && canonical_err["code"] == "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE";
    Ok(json!({
        "case": "unsupported_extension",
        "sot": path.display().to_string(),
        "before": before,
        "after": {"legacy": legacy_err, "canonical": canonical_err},
        "expected": "both fail closed as MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
        "actual": {"pass": pass},
        "pass": pass,
    }))
}

fn invalid_rust_boundary(path: &Path) -> FsvResult<Value> {
    write_secure_text(path, "pub fn broken() {\n    let value = ;\n}\n")?;
    let before = file_state(path)?;
    let legacy_err = legacy_error(path)?;
    let canonical_err = canonical_error(path)?;
    let pass = legacy_err["code"] == "MEJEPA_CHUNKER_PARSE_FAILED"
        && canonical_err["code"] == "MEJEPA_CHUNKER_PARSE_FAILED"
        && legacy_err["diagnostic_count"].as_u64().unwrap_or(0) > 0
        && canonical_err["diagnostic_count"].as_u64().unwrap_or(0) > 0;
    Ok(json!({
        "case": "invalid_rust",
        "sot": path.display().to_string(),
        "before": before,
        "after": {"legacy": legacy_err, "canonical": canonical_err},
        "expected": "both fail closed as MEJEPA_CHUNKER_PARSE_FAILED with diagnostics",
        "actual": {"pass": pass},
        "pass": pass,
    }))
}

fn legacy_error(path: &Path) -> FsvResult<Value> {
    let source = fs::read_to_string(path)?;
    let mut legacy = AstCodeChunker::new_multi_language(AstChunkConfig::default())?;
    let err = legacy
        .chunk(&source, &path.display().to_string())
        .expect_err("boundary case must fail closed through legacy shim");
    Ok(match err {
        AstChunkerError::ParseFailed {
            ref diagnostics, ..
        } => json!({
            "code": err_code(&err),
            "error": err.to_string(),
            "diagnostic_count": diagnostics.len(),
        }),
        other => json!({
            "code": err_code(&other),
            "error": other.to_string(),
            "diagnostic_count": 0,
        }),
    })
}

fn canonical_error(path: &Path) -> FsvResult<Value> {
    let source = fs::read(path)?;
    let err = chunk_path(&source, &path.display().to_string())
        .expect_err("boundary case must fail closed through canonical chunker");
    let diagnostic_count = match &err {
        context_graph_core::memory::MultiAstChunkerError::ParseFailed { diagnostics, .. } => {
            diagnostics.len()
        }
        _ => 0,
    };
    Ok(json!({
        "code": err.code(),
        "error": err.to_string(),
        "diagnostic_count": diagnostic_count,
    }))
}

fn err_code(err: &AstChunkerError) -> &'static str {
    err.code()
}

fn canonical_to_legacy_entity(entity_type: CanonicalEntityType) -> EntityType {
    match entity_type {
        CanonicalEntityType::Function => EntityType::Function,
        CanonicalEntityType::Method => EntityType::Method,
        CanonicalEntityType::Class => EntityType::Class,
        CanonicalEntityType::Struct => EntityType::Struct,
        CanonicalEntityType::Enum => EntityType::Enum,
        CanonicalEntityType::TraitOrInterface => EntityType::TraitOrInterface,
        CanonicalEntityType::Impl => EntityType::Impl,
        CanonicalEntityType::Module => EntityType::Module,
        CanonicalEntityType::Namespace => EntityType::Namespace,
        CanonicalEntityType::TestFunction => EntityType::TestFunction,
        CanonicalEntityType::Import => EntityType::Import,
        CanonicalEntityType::CommentBlock => EntityType::CommentBlock,
        CanonicalEntityType::Docstring => EntityType::Docstring,
    }
}

fn canonical_scope_chain(chunk: &context_graph_core::memory::AstChunk) -> Vec<String> {
    let mut scope = chunk.parent_chain.clone();
    if let Some(symbol) = &chunk.symbol_name {
        if !symbol.is_empty() {
            scope.push(symbol.clone());
        }
    }
    scope
}

fn canonical_source(chunk: &context_graph_core::memory::AstChunk) -> &str {
    if chunk.raw_source.is_empty() {
        &chunk.content
    } else {
        &chunk.raw_source
    }
}

fn file_state(path: &Path) -> FsvResult<Value> {
    let bytes = fs::read(path)?;
    Ok(json!({
        "path": path.display().to_string(),
        "bytes": bytes.len(),
        "sha256": sha256_bytes(&bytes),
    }))
}

fn rustc_check(source: &Path, out: &Path) -> FsvResult<Value> {
    let output = Command::new("rustc")
        .arg("--crate-type=lib")
        .arg("--emit=metadata")
        .arg(source)
        .arg("-o")
        .arg(out)
        .output()?;
    Ok(json!({
        "command": format!("rustc --crate-type=lib --emit=metadata {} -o {}", source.display(), out.display()),
        "success": output.status.success(),
        "status": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "artifact_exists": out.exists(),
        "artifact_bytes": fs::metadata(out).map(|meta| meta.len()).unwrap_or(0),
    }))
}

fn python_check(source: &Path) -> FsvResult<Value> {
    let output = Command::new("python3")
        .arg("-m")
        .arg("py_compile")
        .arg(source)
        .output()?;
    let pycache_count = source
        .parent()
        .unwrap()
        .join("__pycache__")
        .read_dir()
        .map(|entries| entries.count())
        .unwrap_or(0);
    Ok(json!({
        "command": format!("python3 -m py_compile {}", source.display()),
        "success": output.status.success(),
        "status": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "pycache_count": pycache_count,
    }))
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_secure_text(path: &Path, text: &str) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn write_secure_json(path: &Path, value: &Value) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    let text = serde_json::to_string_pretty(value)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    let readback: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    ensure("secure JSON readback must match value", readback == *value)?;
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure(message: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV assertion failed: {message}").into())
    }
}
