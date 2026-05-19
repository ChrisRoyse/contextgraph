#![allow(deprecated)] // This legacy FSV intentionally exercises the deprecated bridge.

//! Manual Full State Verification for the Rust AST chunker.
//!
//! Sources of truth:
//! - `/zfs/archive/contextgraph/fsv/contextgraph-rust-ast-chunker-fsv/sources/*.rs`
//! - `/zfs/archive/contextgraph/fsv/contextgraph-rust-ast-chunker-fsv/rustc/*.rmeta`
//! - `/zfs/archive/contextgraph/fsv/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json`
//!
//! This example fails closed. It writes real Rust source files, validates parse
//! reality with `rustc`, runs the tree-sitter AST chunker, persists evidence,
//! then reads that evidence back before exiting successfully.

use context_graph_core::memory::{AstChunkConfig, AstChunkerError, AstCodeChunker, EntityType};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/zfs/archive/contextgraph/fsv/contextgraph-rust-ast-chunker-fsv");
    let sources = root.join("sources");
    let rustc_out = root.join("rustc");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&sources)?;
    fs::create_dir_all(&rustc_out)?;

    let happy_source = r#"
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
"#;
    let invalid_source = "pub fn broken() {\n    let value = ;\n}\n";
    let method_source = r#"
pub struct Unit {
    value: usize,
}

impl Unit {
    pub fn id(&self) -> usize {
        self.value
    }

    pub fn plus_one(&self) -> usize {
        self.value + 1
    }
}
"#;
    let empty_source = "   \n\t";

    let happy_path = sources.join("happy.rs");
    let invalid_path = sources.join("invalid.rs");
    let method_path = sources.join("method_split.rs");
    write_text_checked(&happy_path, happy_source)?;
    write_text_checked(&invalid_path, invalid_source)?;
    write_text_checked(&method_path, method_source)?;

    let happy_rustc = rustc_check(&happy_path, &rustc_out.join("happy.rmeta"))?;
    ensure(
        "happy source must compile under rustc before chunking",
        happy_rustc["success"].as_bool() == Some(true),
    )?;
    let invalid_rustc = rustc_check(&invalid_path, &rustc_out.join("invalid.rmeta"))?;
    ensure(
        "invalid source must fail rustc as independent syntax source of truth",
        invalid_rustc["success"].as_bool() == Some(false),
    )?;
    let method_rustc = rustc_check(&method_path, &rustc_out.join("method_split.rmeta"))?;
    ensure(
        "method split source must compile under rustc before chunking",
        method_rustc["success"].as_bool() == Some(true),
    )?;

    let happy_case = chunk_success_case(
        "happy_path",
        happy_source,
        &happy_path,
        AstChunkConfig::default(),
    )?;
    let invalid_case = chunk_error_case(
        "invalid_syntax",
        invalid_source,
        &invalid_path,
        AstChunkConfig::default(),
        "parse_failed",
    )?;
    let empty_case = chunk_error_case(
        "empty_input",
        empty_source,
        &sources.join("empty.rs"),
        AstChunkConfig::default(),
        "empty_source",
    )?;
    let method_split_case = chunk_success_case(
        "method_split",
        method_source,
        &method_path,
        AstChunkConfig {
            max_size: 60,
            min_size: 1,
            ..AstChunkConfig::default()
        },
    )?;
    ensure(
        "method split must produce at least one Method chunk",
        method_split_case["after_state"]["method_chunk_count"]
            .as_u64()
            .unwrap_or(0)
            >= 1,
    )?;

    let evidence = json!({
        "source_of_truth": {
            "root": root.display().to_string(),
            "sources": sources.display().to_string(),
            "rustc": rustc_out.display().to_string(),
            "evidence_json": root.join("ast-chunker-evidence.json").display().to_string(),
        },
        "happy_path": happy_case,
        "edge_invalid_syntax": invalid_case,
        "edge_empty_input": empty_case,
        "edge_method_split": method_split_case,
        "rustc_checks": {
            "happy": happy_rustc,
            "invalid": invalid_rustc,
            "method_split": method_rustc,
        },
    });

    let evidence_path = root.join("ast-chunker-evidence.json");
    let evidence_text = serde_json::to_string_pretty(&evidence)?;
    let evidence_sha = write_text_checked(&evidence_path, &evidence_text)?;
    let readback = fs::read_to_string(&evidence_path)?;
    let parsed: Value = serde_json::from_str(&readback)?;
    ensure(
        "read-back JSON must match in-memory evidence",
        parsed == evidence,
    )?;

    println!("Rust AST chunker FSV evidence: {}", evidence_path.display());
    println!("Evidence sha256: {evidence_sha}");
    println!("Read-back size: {} bytes", readback.len());
    println!("{}", evidence_text);
    Ok(())
}

fn chunk_success_case(
    label: &str,
    source: &str,
    path: &Path,
    config: AstChunkConfig,
) -> FsvResult<Value> {
    let before = before_state(label, source, path);
    let mut chunker = AstCodeChunker::new_rust(config)?;
    let chunks = chunker.chunk(source, &path.display().to_string())?;
    ensure(
        &format!("{label} must produce at least one chunk"),
        !chunks.is_empty(),
    )?;
    let chunk_records = chunks
        .iter()
        .map(|chunk| {
            json!({
                "entity_type": format!("{:?}", chunk.metadata.entity_type),
                "start_line": chunk.metadata.start_line,
                "end_line": chunk.metadata.end_line,
                "start_byte": chunk.metadata.start_byte,
                "end_byte": chunk.metadata.end_byte,
                "non_whitespace_chars": chunk.metadata.non_whitespace_chars,
                "code_sha256": sha256_text(&chunk.code),
                "signature": chunk.metadata.entity_signature.clone(),
                "scope_chain": chunk.metadata.scope_chain.clone(),
            })
        })
        .collect::<Vec<_>>();
    let method_chunk_count = chunks
        .iter()
        .filter(|chunk| chunk.metadata.entity_type == EntityType::Method)
        .count();
    Ok(json!({
        "before_state": before,
        "after_state": {
            "result": "ok",
            "chunk_count": chunks.len(),
            "method_chunk_count": method_chunk_count,
            "chunks": chunk_records,
        },
    }))
}

fn chunk_error_case(
    label: &str,
    source: &str,
    path: &Path,
    config: AstChunkConfig,
    expected: &str,
) -> FsvResult<Value> {
    let before = before_state(label, source, path);
    let mut chunker = AstCodeChunker::new_rust(config)?;
    let err = chunker
        .chunk(source, &path.display().to_string())
        .expect_err("edge case must fail closed");
    let after = match err {
        AstChunkerError::EmptySource => {
            ensure(
                &format!("{label} must fail as empty_source"),
                expected == "empty_source",
            )?;
            json!({"result": "error", "kind": "empty_source"})
        }
        AstChunkerError::ParseFailed {
            file_path,
            reason,
            diagnostics,
        } => {
            ensure(
                &format!("{label} must fail as parse_failed"),
                expected == "parse_failed",
            )?;
            ensure(
                &format!("{label} parse failure must include diagnostics"),
                !diagnostics.is_empty(),
            )?;
            json!({
                "result": "error",
                "kind": "parse_failed",
                "file_path": file_path,
                "reason": reason,
                "diagnostics": diagnostics.iter().map(|d| json!({
                    "node_kind": d.kind.clone(),
                    "is_error": d.is_error,
                    "is_missing": d.is_missing,
                    "start_byte": d.start_byte,
                    "end_byte": d.end_byte,
                    "start_line": d.start_line,
                    "start_column": d.start_column,
                    "end_line": d.end_line,
                    "end_column": d.end_column,
                    "excerpt": d.excerpt,
                })).collect::<Vec<_>>(),
            })
        }
        other => {
            return Err(format!("{label} returned unexpected error: {other}").into());
        }
    };
    Ok(json!({
        "before_state": before,
        "after_state": after,
    }))
}

fn before_state(label: &str, source: &str, path: &Path) -> Value {
    json!({
        "label": label,
        "path": path.display().to_string(),
        "byte_length": source.len(),
        "sha256": sha256_text(source),
    })
}

fn rustc_check(source_path: &Path, output_path: &Path) -> FsvResult<Value> {
    if output_path.exists() {
        fs::remove_file(output_path)?;
    }
    let output = Command::new("rustc")
        .arg("--edition=2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--emit=metadata")
        .arg("-o")
        .arg(output_path)
        .arg(source_path)
        .output()?;
    let metadata_exists = output_path.is_file();
    Ok(json!({
        "source_path": source_path.display().to_string(),
        "metadata_path": output_path.display().to_string(),
        "success": output.status.success(),
        "exit_code": output.status.code().unwrap_or(-1),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "metadata_exists": metadata_exists,
        "metadata_sha256": if metadata_exists {
            Value::String(sha256_file(output_path)?)
        } else {
            Value::Null
        },
    }))
}

fn write_text_checked(path: &Path, text: &str) -> FsvResult<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    let readback = fs::read_to_string(path)?;
    ensure(
        &format!("durable readback must match write for {}", path.display()),
        readback == text,
    )?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    sha256_file(path)
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

fn ensure(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("Rust AST chunker FSV invariant failed: {label}").into())
    }
}
