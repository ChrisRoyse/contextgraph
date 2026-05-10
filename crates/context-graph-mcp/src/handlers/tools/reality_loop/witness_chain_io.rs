//! Durable file helpers for optimizer witness-chain operations.

use super::errors::{CCRealityError, Result};
use super::helpers::{file_sot, fs_error};
use fs2::FileExt;
use serde_json::json;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

pub fn with_chain_lock<T>(path: &Path, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| fs_error("CCREALITY_WITNESS_DIR_CREATE_FAILED", parent, e))?;
        let lock_path = parent.join("witness-chain.lock");
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|e| fs_error("CCREALITY_WITNESS_LOCK_OPEN_FAILED", &lock_path, e))?;
        lock_file
            .lock_exclusive()
            .map_err(|e| fs_error("CCREALITY_WITNESS_LOCK_ACQUIRE_FAILED", &lock_path, e))?;
        let result = operation();
        let unlock = lock_file
            .unlock()
            .map_err(|e| fs_error("CCREALITY_WITNESS_LOCK_RELEASE_FAILED", &lock_path, e));
        return match (result, unlock) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
        };
    }
    operation()
}

pub fn read_chain_bytes(path: &Path) -> Result<Vec<u8>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file =
        fs::File::open(path).map_err(|e| fs_error("CCREALITY_WITNESS_READ_FAILED", path, e))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| fs_error("CCREALITY_WITNESS_READ_FAILED", path, e))?;
    Ok(bytes)
}

pub fn write_bytes_checked(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| fs_error("CCREALITY_WITNESS_DIR_CREATE_FAILED", parent, e))?;
    }
    let mut file = fs::File::create(path)
        .map_err(|e| fs_error("CCREALITY_WITNESS_FILE_CREATE_FAILED", path, e))?;
    file.write_all(bytes)
        .map_err(|e| fs_error("CCREALITY_WITNESS_FILE_WRITE_FAILED", path, e))?;
    file.sync_all()
        .map_err(|e| fs_error("CCREALITY_WITNESS_FILE_SYNC_FAILED", path, e))?;
    let readback =
        fs::read(path).map_err(|e| fs_error("CCREALITY_WITNESS_FILE_READBACK_FAILED", path, e))?;
    if readback != bytes {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_FILE_READBACK_MISMATCH",
            "witness file readback did not match written bytes",
            "witness_chain.file.readback",
            "inspect filesystem durability before trusting witness-chain state",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        ));
    }
    sync_parent_dir(path, "CCREALITY_WITNESS_PARENT_SYNC_FAILED")?;
    Ok(())
}

pub fn replace_bytes_checked(path: &Path, bytes: &[u8], temp_name: &str) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_WITNESS_PARENT_MISSING",
            "witness-chain.bin has no parent directory",
            "witness_chain.path",
            "use an absolute path with a parent directory",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|e| fs_error("CCREALITY_WITNESS_DIR_CREATE_FAILED", parent, e))?;
    let tmp_path = parent.join(temp_name);
    write_bytes_checked(&tmp_path, bytes)?;
    fs::rename(&tmp_path, path)
        .map_err(|e| fs_error("CCREALITY_WITNESS_RENAME_FAILED", path, e))?;
    sync_parent_dir(path, "CCREALITY_WITNESS_PARENT_SYNC_FAILED")?;
    let readback =
        fs::read(path).map_err(|e| fs_error("CCREALITY_WITNESS_FILE_READBACK_FAILED", path, e))?;
    if readback != bytes {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_REPLACE_READBACK_MISMATCH",
            "replaced witness-chain.bin readback did not match expected canonical bytes",
            "witness_chain.replace.readback",
            "restore from the backup and inspect filesystem durability",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

pub fn sync_parent_dir(path: &Path, code: &'static str) -> Result<()> {
    if let Some(parent) = path.parent() {
        let dir = fs::File::open(parent).map_err(|e| fs_error(code, parent, e))?;
        dir.sync_all().map_err(|e| fs_error(code, parent, e))?;
    }
    Ok(())
}
