use super::errors::{CCRealityError, Result};
use super::helpers::{file_sot, unix_secs};
use serde_json::json;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

const STALE_LOCK_AGE_SECS: u64 = 3600;

#[derive(Debug)]
pub struct EditLock {
    path: PathBuf,
    session_id: String,
}

impl EditLock {
    pub fn try_acquire(session_id: &str) -> Result<Self> {
        let path = lock_path()?;
        let parent = path.parent().ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_LOCK_PARENT_MISSING",
                "edit lock path has no parent directory",
                "edit_lock.path",
                "fix the edit lock path",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        std::fs::create_dir_all(parent).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_LOCK_DIR_FAILED",
                format!("failed to create edit-lock directory: {e}"),
                "edit_lock.parent",
                "ensure the cgreality cache directory is writable",
                json!({"path": parent.display().to_string()}),
                Some(file_sot(parent)),
            )
        })?;

        if let Some((holder, acquired_at)) = Self::current_holder()? {
            let now = unix_secs()?;
            if now.saturating_sub(acquired_at) > STALE_LOCK_AGE_SECS {
                std::fs::remove_file(&path).map_err(|e| {
                    CCRealityError::new(
                        "CCREALITY_HARNESS_STALE_LOCK_REMOVE_FAILED",
                        format!("failed to remove stale edit lock: {e}"),
                        "edit_lock.remove_stale",
                        "inspect and remove the stale lock manually",
                        json!({"path": path.display().to_string(), "holder": holder}),
                        Some(file_sot(&path)),
                    )
                })?;
            } else {
                return Err(CCRealityError::new(
                    "CCREALITY_HARNESS_EDIT_OWNER_BUSY",
                    "edit lock is held by another session",
                    "edit_lock",
                    "wait for the other writer to finish or remove the lock if stale",
                    json!({"holder": holder, "acquired_at_unix": acquired_at, "now_unix": now}),
                    Some(file_sot(&path)),
                ));
            }
        }

        let acquired_at = unix_secs()?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| {
                CCRealityError::new(
                    "CCREALITY_HARNESS_EDIT_LOCK_FAILED",
                    format!("failed to acquire edit lock: {e}"),
                    "edit_lock.create",
                    "another writer may have acquired the lock simultaneously; retry after re-reading state",
                    json!({"path": path.display().to_string()}),
                    Some(file_sot(&path)),
                )
            })?;
        write!(file, "{}:{}", session_id, acquired_at).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_LOCK_WRITE_FAILED",
                format!("failed to write edit lock: {e}"),
                "edit_lock.write",
                "ensure the cgreality cache directory is writable",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        file.sync_all().map_err(|e| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_LOCK_SYNC_FAILED",
                format!("failed to sync edit lock: {e}"),
                "edit_lock.sync",
                "ensure the filesystem accepts fsync for the lock path",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        Ok(Self {
            path,
            session_id: session_id.to_string(),
        })
    }

    pub fn current_holder() -> Result<Option<(String, u64)>> {
        let path = lock_path()?;
        if !path.is_file() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_LOCK_READ_FAILED",
                format!("failed to read edit lock: {e}"),
                "edit_lock.read",
                "inspect the cgreality cache directory",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        let Some((holder, ts)) = raw.trim().split_once(':') else {
            return Ok(None);
        };
        let Ok(ts) = ts.parse::<u64>() else {
            return Ok(None);
        };
        Ok(Some((holder.to_string(), ts)))
    }
}

impl Drop for EditLock {
    fn drop(&mut self) {
        if let Ok(raw) = std::fs::read_to_string(&self.path) {
            if raw.starts_with(&format!("{}:", self.session_id)) {
                if let Err(e) = std::fs::remove_file(&self.path) {
                    tracing::warn!("failed to remove edit lock {}: {}", self.path.display(), e);
                }
            }
        }
    }
}

fn lock_path() -> Result<PathBuf> {
    context_graph_paths::cgreality_cache_file("edit_lock").map_err(|e| {
        CCRealityError::new(
            e.code,
            e.message,
            "cgreality.edit_lock",
            e.remediation,
            json!({"data_root_env": context_graph_paths::ENV_DATA_ROOT}),
            None,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_blocks_second_owner_and_releases() {
        let _ = std::fs::remove_file(lock_path().expect("lock path"));
        let first = EditLock::try_acquire("phase4-lock-test-a").expect("first lock");
        let second = EditLock::try_acquire("phase4-lock-test-b").expect_err("second lock denied");
        assert_eq!(second.error_code, "CCREALITY_HARNESS_EDIT_OWNER_BUSY");
        drop(first);
        EditLock::try_acquire("phase4-lock-test-c").expect("lock released");
        let _ = std::fs::remove_file(lock_path().expect("lock path"));
    }
}
