#![allow(deprecated)]

use context_graph_core::memory::ast::{
    chunk_with_options, AstChunk, AstChunkOptions, EntityType as CanonicalEntityType, Language,
};
use context_graph_core::memory::{
    AstChunkerError, AstCodeChunker, CodeChunk, EntityType as LegacyEntityType,
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
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/ast-codechunker-convergence-fsv");
    reset_secure_dir(&root)?;
    let sources = root.join("sources");
    let toolchain = root.join("toolchain");
    fs::create_dir_all(&sources)?;
    fs::create_dir_all(&toolchain)?;

    let rust_happy = run_rust_happy_path(&sources, &toolchain)?;
    let python_happy = run_python_happy_path(&sources)?;
    let empty = run_empty_edge(&sources)?;
    let invalid_rust = run_invalid_rust_edge(&sources, &toolchain)?;
    let unsupported = run_unsupported_extension_edge(&sources)?;

    let evidence_path = root.join("ast-codechunker-convergence-fsv.json");
    let evidence = json!({
        "source_of_truth": {
            "root": root.display().to_string(),
            "sources": sources.display().to_string(),
            "toolchain_outputs": toolchain.display().to_string(),
            "evidence_json": evidence_path.display().to_string(),
        },
        "root_cause_verified": {
            "legacy_ast_codechunker": "deprecated compatibility API",
            "canonical_source": "context_graph_core::memory::ast::chunk_with_options",
            "required_behavior": "legacy facade must emit the same source-backed AST chunks and fail-closed errors as the canonical parser",
        },
        "happy_paths": {
            "rust_default_rust": rust_happy,
            "python_default_multi_language": python_happy,
        },
        "edge_cases": {
            "empty_input": empty,
            "invalid_rust": invalid_rust,
            "unsupported_extension": unsupported,
        },
    });
    write_secure_json(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "evidence readback equals written evidence",
        readback == evidence,
    )?;
    require(
        "all happy paths pass",
        readback["happy_paths"]
            .as_object()
            .ok_or("happy_paths must be an object")?
            .values()
            .all(|case| case["passes"] == Value::Bool(true)),
    )?;
    require(
        "all edge cases pass",
        readback["edge_cases"]
            .as_object()
            .ok_or("edge_cases must be an object")?
            .values()
            .all(|case| case["passes"] == Value::Bool(true)),
    )?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fsv": "ast_codechunker_convergence",
            "source_of_truth": readback["source_of_truth"],
            "happy_passes": {
                "rust": readback["happy_paths"]["rust_default_rust"]["passes"],
                "python": readback["happy_paths"]["python_default_multi_language"]["passes"],
            },
            "edge_cases": readback["edge_cases"],
        }))?
    );
    Ok(())
}

fn run_rust_happy_path(sources: &Path, toolchain: &Path) -> FsvResult<Value> {
    let source = r#"use std::collections::HashMap;

pub struct Solver {
    values: HashMap<String, i32>,
}

impl Solver {
    pub fn new() -> Self {
        Self { values: HashMap::new() }
    }

    pub fn compute(&self, x: i32) -> i32 {
        x + 41
    }
}

pub fn helper(x: i32) -> i32 {
    Solver::new().compute(x)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_helper() {
        assert_eq!(42, super::helper(1));
    }
}
"#;
    let path = sources.join("happy_solver.rs");
    write_checked(&path, source.as_bytes())?;
    let before = source_state(&path, source.as_bytes())?;
    println!(
        "before rust_happy_path: {}",
        serde_json::to_string(&before)?
    );

    let rustc = rustc_check(&path, &toolchain.join("happy_solver.rmeta"))?;
    require(
        "happy Rust source must compile",
        rustc["success"] == Value::Bool(true),
    )?;
    let readback = fs::read(&path)?;
    let mut legacy = AstCodeChunker::default_rust()?;
    let options = AstChunkOptions {
        file_path: path.display().to_string(),
        max_non_ws_chars: legacy.config().max_size,
    };
    let canonical_chunks = chunk_with_options(&readback, Language::Rust, &options)?;
    let legacy_chunks =
        legacy.chunk(std::str::from_utf8(&readback)?, &path.display().to_string())?;
    let after = compare_state(
        &canonical_chunks,
        &legacy_chunks,
        &[
            "Import",
            "Struct",
            "Impl",
            "Method",
            "Function",
            "TestFunction",
        ],
    )?;
    println!("after rust_happy_path: {}", serde_json::to_string(&after)?);

    Ok(json!({
        "source_of_truth": {
            "source_file": path.display().to_string(),
            "rustc_metadata": toolchain.join("happy_solver.rmeta").display().to_string(),
        },
        "before_state": before,
        "action": {
            "toolchain": rustc,
            "canonical_api": "context_graph_core::memory::ast::chunk_with_options",
            "legacy_api": "context_graph_core::memory::AstCodeChunker::default_rust().chunk",
        },
        "after_state": after,
        "passes": after["matches_canonical"] == Value::Bool(true)
            && after["expected_entities_present"] == Value::Bool(true)
            && rustc["success"] == Value::Bool(true),
    }))
}

fn run_python_happy_path(sources: &Path) -> FsvResult<Value> {
    let source = r#""""Solver module."""
import math

CONSTANT = 41

class Solver:
    label = "solver"

    def compute(self, x: int) -> int:
        return x + CONSTANT

def helper(x: int) -> int:
    return Solver().compute(x)

def test_helper() -> None:
    assert helper(1) == 42
"#;
    let path = sources.join("test_solver.py");
    write_checked(&path, source.as_bytes())?;
    let before = source_state(&path, source.as_bytes())?;
    println!(
        "before python_happy_path: {}",
        serde_json::to_string(&before)?
    );

    let py_compile = py_compile(&path)?;
    require(
        "happy Python source must compile",
        py_compile["success"] == Value::Bool(true),
    )?;
    let readback = fs::read(&path)?;
    let mut legacy = AstCodeChunker::default_multi_language()?;
    let options = AstChunkOptions {
        file_path: path.display().to_string(),
        max_non_ws_chars: legacy.config().max_size,
    };
    let canonical_chunks = chunk_with_options(&readback, Language::Python, &options)?;
    let legacy_chunks =
        legacy.chunk(std::str::from_utf8(&readback)?, &path.display().to_string())?;
    let after = compare_state(
        &canonical_chunks,
        &legacy_chunks,
        &[
            "Import",
            "Module",
            "Class",
            "Method",
            "Function",
            "TestFunction",
        ],
    )?;
    println!(
        "after python_happy_path: {}",
        serde_json::to_string(&after)?
    );

    Ok(json!({
        "source_of_truth": {
            "source_file": path.display().to_string(),
            "python_bytecode_dir": path.parent().unwrap_or(sources).join("__pycache__").display().to_string(),
        },
        "before_state": before,
        "action": {
            "toolchain": py_compile,
            "canonical_api": "context_graph_core::memory::ast::chunk_with_options",
            "legacy_api": "context_graph_core::memory::AstCodeChunker::default_multi_language().chunk",
        },
        "after_state": after,
        "passes": after["matches_canonical"] == Value::Bool(true)
            && after["expected_entities_present"] == Value::Bool(true)
            && py_compile["success"] == Value::Bool(true),
    }))
}

fn run_empty_edge(sources: &Path) -> FsvResult<Value> {
    let path = sources.join("empty.py");
    write_checked(&path, b"")?;
    let before = source_state(&path, b"")?;
    println!("before empty_edge: {}", serde_json::to_string(&before)?);

    let canonical = chunk_with_options(
        b"",
        Language::Python,
        &AstChunkOptions::for_path(path.display().to_string()),
    )
    .unwrap_err();
    let mut legacy = AstCodeChunker::default_multi_language()?;
    let legacy_err = legacy.chunk("", &path.display().to_string()).unwrap_err();
    let after = json!({
        "accepted": false,
        "canonical_error_code": canonical.code(),
        "canonical_error": canonical.to_string(),
        "legacy_error_code": legacy_err.code(),
        "legacy_error": legacy_err.to_string(),
        "source_file_readback": source_state(&path, b"")?,
    });
    println!("after empty_edge: {}", serde_json::to_string(&after)?);
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "expected_code": "MEJEPA_CHUNKER_EMPTY_SOURCE",
        "passes": canonical.code() == "MEJEPA_CHUNKER_EMPTY_SOURCE"
            && legacy_err.code() == "MEJEPA_CHUNKER_EMPTY_SOURCE",
    }))
}

fn run_invalid_rust_edge(sources: &Path, toolchain: &Path) -> FsvResult<Value> {
    let source = b"pub fn broken() {\n    let value = ;\n}\n";
    let path = sources.join("invalid_broken.rs");
    write_checked(&path, source)?;
    let before = source_state(&path, source)?;
    println!(
        "before invalid_rust_edge: {}",
        serde_json::to_string(&before)?
    );

    let rustc = rustc_check(&path, &toolchain.join("invalid_broken.rmeta"))?;
    let canonical = chunk_with_options(
        source,
        Language::Rust,
        &AstChunkOptions::for_path(path.display().to_string()),
    )
    .unwrap_err();
    let mut legacy = AstCodeChunker::default_rust()?;
    let legacy_err = legacy
        .chunk(std::str::from_utf8(source)?, &path.display().to_string())
        .unwrap_err();
    let after = json!({
        "accepted": false,
        "rustc": rustc,
        "canonical": ast_error_state(&canonical),
        "legacy": legacy_error_state(&legacy_err),
        "source_file_readback": source_state(&path, source)?,
    });
    println!(
        "after invalid_rust_edge: {}",
        serde_json::to_string(&after)?
    );
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "expected_code": "MEJEPA_CHUNKER_PARSE_FAILED",
        "passes": after["rustc"]["success"] == Value::Bool(false)
            && after["canonical"]["code"] == Value::String("MEJEPA_CHUNKER_PARSE_FAILED".to_string())
            && after["legacy"]["code"] == Value::String("MEJEPA_CHUNKER_PARSE_FAILED".to_string())
            && after["legacy"]["diagnostic_count"].as_u64().unwrap_or(0) > 0,
    }))
}

fn run_unsupported_extension_edge(sources: &Path) -> FsvResult<Value> {
    let source = b"not source code for a supported language\n";
    let path = sources.join("unsupported.xyz");
    write_checked(&path, source)?;
    let before = source_state(&path, source)?;
    println!(
        "before unsupported_extension_edge: {}",
        serde_json::to_string(&before)?
    );

    let canonical = Language::from_path(&path.display().to_string()).unwrap_err();
    let mut legacy = AstCodeChunker::default_multi_language()?;
    let legacy_err = legacy
        .chunk(std::str::from_utf8(source)?, &path.display().to_string())
        .unwrap_err();
    let after = json!({
        "accepted": false,
        "canonical_error_code": canonical.code(),
        "canonical_error": canonical.to_string(),
        "legacy_error_code": legacy_err.code(),
        "legacy_error": legacy_err.to_string(),
        "source_file_readback": source_state(&path, source)?,
    });
    println!(
        "after unsupported_extension_edge: {}",
        serde_json::to_string(&after)?
    );
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "expected_code": "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
        "passes": canonical.code() == "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE"
            && legacy_err.code() == "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
    }))
}

fn compare_state(
    canonical: &[AstChunk],
    legacy: &[CodeChunk],
    expected_entities: &[&str],
) -> FsvResult<Value> {
    let canonical_rows = canonical
        .iter()
        .map(canonical_summary)
        .collect::<FsvResult<Vec<_>>>()?;
    let legacy_rows = legacy
        .iter()
        .map(legacy_summary)
        .collect::<FsvResult<Vec<_>>>()?;
    let expected_entities_present = expected_entities.iter().all(|expected| {
        canonical_rows
            .iter()
            .any(|row| row["entity_type"] == Value::String((*expected).to_string()))
            && legacy_rows
                .iter()
                .any(|row| row["entity_type"] == Value::String((*expected).to_string()))
    });
    Ok(json!({
        "canonical_chunk_count": canonical_rows.len(),
        "legacy_chunk_count": legacy_rows.len(),
        "canonical_chunks": canonical_rows,
        "legacy_chunks": legacy_rows,
        "matches_canonical": canonical_rows == legacy_rows,
        "expected_entities": expected_entities,
        "expected_entities_present": expected_entities_present,
    }))
}

fn canonical_summary(chunk: &AstChunk) -> FsvResult<Value> {
    Ok(json!({
        "language": chunk.language.slug(),
        "entity_type": format!("{:?}", chunk.entity_type),
        "parent_chain": chunk.parent_chain,
        "symbol_name": chunk.symbol_name,
        "line_start": chunk.line_start,
        "line_end": chunk.line_end,
        "start_byte": chunk.start_byte,
        "end_byte": chunk.end_byte,
        "non_whitespace_chars": chunk.raw_source.chars().filter(|ch| !ch.is_whitespace()).count(),
        "raw_sha256": sha256(chunk.raw_source.as_bytes()),
        "raw_source": chunk.raw_source,
        "routing_count": chunk.embedder_routing_set.len(),
    }))
}

fn legacy_summary(chunk: &CodeChunk) -> FsvResult<Value> {
    let symbol_name = legacy_symbol_name(chunk.metadata.entity_type, &chunk.metadata.scope_chain);
    let parent_chain = legacy_parent_chain(symbol_name.as_ref(), &chunk.metadata.scope_chain);
    Ok(json!({
        "language": chunk.metadata.language,
        "entity_type": legacy_entity_name(chunk.metadata.entity_type),
        "parent_chain": parent_chain,
        "symbol_name": symbol_name,
        "line_start": chunk.metadata.start_line,
        "line_end": chunk.metadata.end_line,
        "start_byte": chunk.metadata.start_byte,
        "end_byte": chunk.metadata.end_byte,
        "non_whitespace_chars": chunk.metadata.non_whitespace_chars,
        "raw_sha256": sha256(chunk.code.as_bytes()),
        "raw_source": chunk.code,
        "routing_count": routing_count_for_legacy(chunk.metadata.entity_type),
    }))
}

fn legacy_symbol_name(_entity_type: LegacyEntityType, scope_chain: &[String]) -> Option<String> {
    scope_chain.last().cloned().filter(|name| !name.is_empty())
}

fn legacy_parent_chain(symbol_name: Option<&String>, scope_chain: &[String]) -> Vec<String> {
    if symbol_name.is_some() && !scope_chain.is_empty() {
        scope_chain[..scope_chain.len() - 1].to_vec()
    } else {
        scope_chain.to_vec()
    }
}

fn legacy_entity_name(entity_type: LegacyEntityType) -> &'static str {
    match entity_type {
        LegacyEntityType::Function => "Function",
        LegacyEntityType::Method => "Method",
        LegacyEntityType::Class => "Class",
        LegacyEntityType::Struct => "Struct",
        LegacyEntityType::Enum => "Enum",
        LegacyEntityType::Trait => "Trait",
        LegacyEntityType::TraitOrInterface => "TraitOrInterface",
        LegacyEntityType::Impl => "Impl",
        LegacyEntityType::Module => "Module",
        LegacyEntityType::Namespace => "Namespace",
        LegacyEntityType::TestFunction => "TestFunction",
        LegacyEntityType::Import => "Import",
        LegacyEntityType::Const => "Const",
        LegacyEntityType::Static => "Static",
        LegacyEntityType::TypeAlias => "TypeAlias",
        LegacyEntityType::Macro => "Macro",
        LegacyEntityType::Comment => "Comment",
        LegacyEntityType::CommentBlock => "CommentBlock",
        LegacyEntityType::Docstring => "Docstring",
        LegacyEntityType::Mixed => "Mixed",
        LegacyEntityType::Unknown => "Unknown",
    }
}

fn routing_count_for_legacy(entity_type: LegacyEntityType) -> usize {
    let canonical = match entity_type {
        LegacyEntityType::Function => Some(CanonicalEntityType::Function),
        LegacyEntityType::Method => Some(CanonicalEntityType::Method),
        LegacyEntityType::Class => Some(CanonicalEntityType::Class),
        LegacyEntityType::Struct => Some(CanonicalEntityType::Struct),
        LegacyEntityType::Enum => Some(CanonicalEntityType::Enum),
        LegacyEntityType::Trait | LegacyEntityType::TraitOrInterface => {
            Some(CanonicalEntityType::TraitOrInterface)
        }
        LegacyEntityType::Impl => Some(CanonicalEntityType::Impl),
        LegacyEntityType::Module => Some(CanonicalEntityType::Module),
        LegacyEntityType::Namespace => Some(CanonicalEntityType::Namespace),
        LegacyEntityType::TestFunction => Some(CanonicalEntityType::TestFunction),
        LegacyEntityType::Import => Some(CanonicalEntityType::Import),
        LegacyEntityType::Comment | LegacyEntityType::CommentBlock => {
            Some(CanonicalEntityType::CommentBlock)
        }
        LegacyEntityType::Docstring => Some(CanonicalEntityType::Docstring),
        _ => None,
    };
    canonical
        .map(context_graph_core::memory::ast::route_to_embedders)
        .map(|set| set.len())
        .unwrap_or(0)
}

fn ast_error_state(err: &context_graph_core::memory::ast::AstChunkerError) -> Value {
    match err {
        context_graph_core::memory::ast::AstChunkerError::ParseFailed {
            diagnostics,
            reason,
            ..
        } => json!({
            "code": err.code(),
            "reason": reason,
            "diagnostic_count": diagnostics.len(),
            "diagnostics": diagnostics.iter().map(|diagnostic| {
                json!({
                    "kind": diagnostic.kind,
                    "is_error": diagnostic.is_error,
                    "is_missing": diagnostic.is_missing,
                    "start_byte": diagnostic.start_byte,
                    "end_byte": diagnostic.end_byte,
                    "start_line": diagnostic.start_line,
                    "start_column": diagnostic.start_column,
                    "end_line": diagnostic.end_line,
                    "end_column": diagnostic.end_column,
                    "excerpt": diagnostic.excerpt,
                })
            }).collect::<Vec<_>>(),
        }),
        _ => json!({
            "code": err.code(),
            "error": err.to_string(),
            "diagnostic_count": 0,
        }),
    }
}

fn legacy_error_state(err: &AstChunkerError) -> Value {
    match err {
        AstChunkerError::ParseFailed {
            diagnostics,
            reason,
            ..
        } => json!({
            "code": err.code(),
            "reason": reason,
            "diagnostic_count": diagnostics.len(),
            "diagnostics": diagnostics.iter().map(|diagnostic| {
                json!({
                    "kind": diagnostic.kind,
                    "is_error": diagnostic.is_error,
                    "is_missing": diagnostic.is_missing,
                    "start_byte": diagnostic.start_byte,
                    "end_byte": diagnostic.end_byte,
                    "start_line": diagnostic.start_line,
                    "start_column": diagnostic.start_column,
                    "end_line": diagnostic.end_line,
                    "end_column": diagnostic.end_column,
                    "excerpt": diagnostic.excerpt,
                })
            }).collect::<Vec<_>>(),
        }),
        _ => json!({
            "code": err.code(),
            "error": err.to_string(),
            "diagnostic_count": 0,
        }),
    }
}

fn source_state(path: &Path, expected: &[u8]) -> FsvResult<Value> {
    let readback = fs::read(path)?;
    require(
        &format!("source readback mismatch for {}", path.display()),
        readback == expected,
    )?;
    Ok(json!({
        "path": path.display().to_string(),
        "bytes": readback.len(),
        "sha256": sha256(&readback),
    }))
}

fn rustc_check(source_path: &Path, output_path: &Path) -> FsvResult<Value> {
    let output = Command::new("rustc")
        .arg("--crate-type")
        .arg("lib")
        .arg("--emit")
        .arg("metadata")
        .arg(source_path)
        .arg("-o")
        .arg(output_path)
        .output()?;
    let output_bytes = if output_path.exists() {
        Some(fs::read(output_path)?.len())
    } else {
        None
    };
    Ok(json!({
        "command": format!("rustc --crate-type lib --emit metadata {} -o {}", source_path.display(), output_path.display()),
        "success": output.status.success(),
        "exit_code": output.status.code().unwrap_or(-1),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "output_path": output_path.display().to_string(),
        "output_bytes": output_bytes,
    }))
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
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

fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV requirement failed: {name}").into())
    }
}
