// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{OpsError, OpsResult};
use crate::gc::gc_run_nightly;
use crate::quota::quota_status;
use crate::storage::{open_hygiene_rocksdb, runtime_config, HygieneEnv};
use crate::witness_compress::witness_compress_old_segments;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HygieneMcpRequest {
    pub db_path: PathBuf,
    pub archive_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WitnessCompressMcpRequest {
    pub db_path: PathBuf,
    pub archive_root: PathBuf,
    #[serde(default = "default_segment_size")]
    pub segment_size: usize,
    #[serde(default = "default_min_age_days")]
    pub min_age_days: u32,
}

pub fn mcp_gc_run(request: HygieneMcpRequest) -> OpsResult<serde_json::Value> {
    let env = env_from_paths(request.db_path, request.archive_root)?;
    Ok(serde_json::to_value(gc_run_nightly(&env)?)?)
}

pub fn mcp_quota_status(request: HygieneMcpRequest) -> OpsResult<serde_json::Value> {
    let env = env_from_paths(request.db_path, request.archive_root)?;
    Ok(serde_json::to_value(quota_status(&env)?)?)
}

pub fn mcp_witness_compress(request: WitnessCompressMcpRequest) -> OpsResult<serde_json::Value> {
    if request.segment_size == 0 {
        return Err(OpsError::invalid(
            "segmentSize",
            "segment size must be >= 1",
        ));
    }
    validate_mcp_paths(&request.db_path, &request.archive_root)?;
    let db = open_hygiene_rocksdb(&request.db_path)?;
    let mut config = runtime_config(db, request.archive_root)?;
    config.witness_segment_size = request.segment_size;
    config.witness_min_age_days = request.min_age_days;
    let env = HygieneEnv::try_new(config)?;
    Ok(serde_json::to_value(witness_compress_old_segments(&env)?)?)
}

fn env_from_paths(db_path: PathBuf, archive_root: PathBuf) -> OpsResult<HygieneEnv> {
    validate_mcp_paths(&db_path, &archive_root)?;
    let db = open_hygiene_rocksdb(db_path)?;
    HygieneEnv::try_new(runtime_config(db, archive_root)?)
}

fn validate_mcp_paths(db_path: &PathBuf, archive_root: &PathBuf) -> OpsResult<()> {
    validate_path("dbPath", db_path)?;
    validate_path("archiveRoot", archive_root)?;
    if db_path == archive_root {
        return Err(OpsError::invalid(
            "paths",
            "dbPath and archiveRoot must be different paths",
        ));
    }
    if archive_root.starts_with(db_path) || db_path.starts_with(archive_root) {
        return Err(OpsError::invalid(
            "paths",
            "dbPath and archiveRoot must not be nested inside each other",
        ));
    }
    Ok(())
}

fn validate_path(field: &'static str, path: &Path) -> OpsResult<()> {
    if path.as_os_str().is_empty() {
        return Err(OpsError::invalid(field, "path must be non-empty"));
    }
    if !path.is_absolute() {
        return Err(OpsError::invalid(
            field,
            "MCP hygiene paths must be absolute",
        ));
    }
    let rendered = path.to_string_lossy();
    if rendered.chars().any(char::is_control) {
        return Err(OpsError::invalid(
            field,
            "path must not contain control characters",
        ));
    }
    Ok(())
}

fn default_segment_size() -> usize {
    1024
}

fn default_min_age_days() -> u32 {
    1
}
