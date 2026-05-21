use crate::error::TrainerError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::{invalid, validate_id, validate_project_relative_path};

const DEFAULT_MAX_SOURCE_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChunkSourceRow {
    pub chunk_id: String,
    pub file_path: String,
    pub byte_span: [u64; 2],
    pub source_text: Option<String>,
    pub source_text_sha256: Option<String>,
    pub source_row_key: Option<String>,
}

impl ChunkSourceRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_id("chunk_id", &self.chunk_id)?;
        validate_project_relative_path("file_path", &self.file_path)?;
        if self.byte_span[1] < self.byte_span[0] {
            return Err(invalid("byte_span", "end must be >= start"));
        }
        if let Some(value) = &self.source_text_sha256 {
            validate_sha256("source_text_sha256", value)?;
            if let Some(source_text) = &self.source_text {
                let expected = sha256_text(source_text);
                if value != &expected {
                    return Err(invalid(
                        "source_text_sha256",
                        "does not match source_text bytes",
                    ));
                }
            }
        }
        if let Some(value) = &self.source_row_key {
            validate_id("source_row_key", value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChunkSourceIndex {
    pub rows_by_chunk_id: BTreeMap<String, Vec<ChunkSourceRow>>,
}

impl ChunkSourceIndex {
    pub fn insert(&mut self, row: ChunkSourceRow) -> Result<(), TrainerError> {
        row.validate()?;
        self.rows_by_chunk_id
            .entry(row.chunk_id.clone())
            .or_default()
            .push(row);
        Ok(())
    }

    pub fn chunk_ids(&self) -> Vec<String> {
        self.rows_by_chunk_id.keys().cloned().collect()
    }
}

pub fn load_chunk_source_index_jsonl(path: &Path) -> Result<ChunkSourceIndex, TrainerError> {
    load_chunk_source_index_jsonl_with_limit(path, DEFAULT_MAX_SOURCE_BYTES)
}

pub fn load_chunk_source_index_jsonl_with_limit(
    path: &Path,
    max_source_bytes: usize,
) -> Result<ChunkSourceIndex, TrainerError> {
    if max_source_bytes == 0 {
        return Err(invalid("max_source_bytes", "must be positive"));
    }
    let file = File::open(path)?;
    let mut index = ChunkSourceIndex::default();
    for (line_index, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&line)?;
        let chunk_id = string_field(&value, "chunk_id").ok_or_else(|| {
            invalid(
                "chunk_id",
                format!("missing chunk_id at {}:{}", path.display(), line_index + 1),
            )
        })?;
        let file_path = string_field(&value, "relative_path")
            .or_else(|| string_field(&value, "file_path"))
            .unwrap_or_else(|| "unknown.py".to_string());
        let source_text = string_field(&value, "source_text")
            .or_else(|| string_field(&value, "redacted_source_text"));
        if let Some(text) = &source_text {
            if text.len() > max_source_bytes {
                return Err(invalid(
                    "source_text",
                    format!(
                        "chunk {chunk_id} source_text has {} bytes, exceeds {max_source_bytes}",
                        text.len()
                    ),
                ));
            }
        }
        let source_text_sha256 = string_field(&value, "source_text_sha256")
            .or_else(|| source_text.as_ref().map(|text| sha256_text(text)));
        let row = ChunkSourceRow {
            chunk_id,
            file_path,
            byte_span: byte_span(&value),
            source_text,
            source_text_sha256,
            source_row_key: string_field(&value, "row_key"),
        };
        index.insert(row)?;
    }
    Ok(index)
}

fn string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn byte_span(value: &serde_json::Value) -> [u64; 2] {
    if let Some(array) = value.get("byte_span").and_then(serde_json::Value::as_array) {
        if array.len() == 2 {
            let start = array[0].as_u64().unwrap_or(0);
            let end = array[1].as_u64().unwrap_or(start);
            return [start, end];
        }
    }
    let start = value
        .get("byte_start")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let end = value
        .get("byte_end")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(start);
    [start, end]
}

fn sha256_text(value: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(value.as_bytes())))
}

fn validate_sha256(field: &str, value: &str) -> Result<(), TrainerError> {
    validate_id(field, value)?;
    let Some(hex_value) = value.strip_prefix("sha256:") else {
        return Err(invalid(field, "must have sha256: prefix"));
    };
    if hex_value.len() != 64 || !hex_value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid(
            field,
            "must contain 64 lowercase/uppercase hex chars",
        ));
    }
    Ok(())
}
