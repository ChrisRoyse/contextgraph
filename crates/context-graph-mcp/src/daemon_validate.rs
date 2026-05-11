use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DaemonPaths {
    pub root: PathBuf,
    pub storage_db: PathBuf,
    pub shift_log_dir: PathBuf,
    pub panel_db: PathBuf,
    pub logs_dir: PathBuf,
    pub locks_dir: PathBuf,
    pub fsv_dir: PathBuf,
    pub scheduler_state_dir: PathBuf,
    pub hygiene_archive_dir: PathBuf,
    pub pid_file: PathBuf,
    pub stdout_log: PathBuf,
    pub stderr_log: PathBuf,
}

#[derive(Debug, Error)]
pub enum DaemonValidationError {
    #[error("MEJEPA_D_DRIVE_PATH_INVALID: {path}: {detail}")]
    PathInvalid { path: String, detail: String },
    #[error("MEJEPA_D_DRIVE_UNAVAILABLE: {path}: {source}")]
    DDriveUnavailable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("MEJEPA_D_DRIVE_NOT_WRITABLE: {path}: {source}")]
    DDriveNotWritable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("MEJEPA_D_DRIVE_SUBDIR_CREATE_FAILED: {path}: {source}")]
    DDriveSubdirCreateFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("MEJEPA_DAEMON_LOCK_HELD: {path}: {holder}")]
    DaemonLockHeld { path: PathBuf, holder: String },
}

pub fn validate_d_drive_root(root: &Path) -> Result<DaemonPaths, DaemonValidationError> {
    validate_root_path(root)?;
    let metadata =
        fs::metadata(root).map_err(|source| DaemonValidationError::DDriveUnavailable {
            path: root.to_path_buf(),
            source,
        })?;
    if !metadata.is_dir() {
        return Err(DaemonValidationError::PathInvalid {
            path: root.display().to_string(),
            detail: "root exists but is not a directory".to_string(),
        });
    }

    let root_write_test = root.join(".mejepa-daemon-root-write-test");
    write_remove_test(root, &root_write_test)?;

    let paths = DaemonPaths {
        root: root.to_path_buf(),
        storage_db: root.join("storage/contextgraph-rocksdb"),
        shift_log_dir: root.join("runtime/cgreality-shift-log"),
        panel_db: root.join("storage/mejepa-panels"),
        logs_dir: root.join("logs"),
        locks_dir: root.join("state/locks"),
        fsv_dir: root.join("fsv/mcp-daemon-fsv"),
        scheduler_state_dir: root.join("state/schedulers"),
        hygiene_archive_dir: root.join("storage/witness"),
        pid_file: root.join("state/locks/context-graph-daemon.pid"),
        stdout_log: root.join("logs/mejepa-daemon.stdout.log"),
        stderr_log: root.join("logs/mejepa-daemon.stderr.log"),
    };

    for dir in required_subdirs(root) {
        fs::create_dir_all(&dir).map_err(|source| {
            DaemonValidationError::DDriveSubdirCreateFailed {
                path: dir.clone(),
                source,
            }
        })?;
        let meta =
            fs::metadata(&dir).map_err(|source| DaemonValidationError::DDriveUnavailable {
                path: dir.clone(),
                source,
            })?;
        if !meta.is_dir() {
            return Err(DaemonValidationError::PathInvalid {
                path: dir.display().to_string(),
                detail: "required subpath exists but is not a directory".to_string(),
            });
        }
    }

    let write_test = paths.locks_dir.join(".mejepa-daemon-write-test");
    write_remove_test(&paths.locks_dir, &write_test)?;
    Ok(paths)
}

fn write_remove_test(root: &Path, write_test: &Path) -> Result<(), DaemonValidationError> {
    fs::write(write_test, b"ok").map_err(|source| DaemonValidationError::DDriveNotWritable {
        path: root.to_path_buf(),
        source,
    })?;
    fs::remove_file(write_test).map_err(|source| DaemonValidationError::DDriveNotWritable {
        path: write_test.to_path_buf(),
        source,
    })
}

fn validate_root_path(path: &Path) -> Result<(), DaemonValidationError> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(DaemonValidationError::PathInvalid {
            path: path.display().to_string(),
            detail: "root must be a non-empty absolute path".to_string(),
        });
    }
    let rendered = path.to_string_lossy();
    if rendered.chars().any(|ch| ch.is_control()) {
        return Err(DaemonValidationError::PathInvalid {
            path: rendered.into_owned(),
            detail: "root contains a control character".to_string(),
        });
    }
    Ok(())
}

fn required_subdirs(root: &Path) -> Vec<PathBuf> {
    [
        "storage/contextgraph-rocksdb",
        "runtime/cgreality-shift-log",
        "logs",
        "state/locks",
        "state/schedulers",
        "state/cgreality",
        "state/gold-labels",
        "fsv/mcp-daemon-fsv",
        "storage/replay",
        "storage/witness",
        "storage/cold",
        "storage/train-certs",
        "storage/live-predictions",
        "storage/mejepa-panels",
        "exports/eval",
        "exports/active-learning",
        "exports/pairwise-mi",
    ]
    .iter()
    .map(|relative| root.join(relative))
    .collect()
}

#[derive(Debug)]
pub struct DaemonPidLock {
    path: PathBuf,
    pid: u32,
    _file: File,
}

impl DaemonPidLock {
    pub fn acquire(path: &Path) -> Result<Self, DaemonValidationError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                DaemonValidationError::DDriveSubdirCreateFailed {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|source| DaemonValidationError::DDriveNotWritable {
                path: path.to_path_buf(),
                source,
            })?;
        if let Err(source) = file.try_lock_exclusive() {
            let mut holder = String::new();
            let _ = file.seek(std::io::SeekFrom::Start(0));
            let _ = file.read_to_string(&mut holder);
            let holder = if holder.trim().is_empty() {
                format!("unknown holder; lock error: {source}")
            } else {
                holder.trim().to_string()
            };
            return Err(DaemonValidationError::DaemonLockHeld {
                path: path.to_path_buf(),
                holder,
            });
        }
        let pid = std::process::id();
        file.set_len(0)
            .and_then(|_| file.seek(std::io::SeekFrom::Start(0)).map(|_| ()))
            .and_then(|_| writeln!(file, "{pid}"))
            .and_then(|_| file.flush())
            .map_err(|source| DaemonValidationError::DDriveNotWritable {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(Self {
            path: path.to_path_buf(),
            pid,
            _file: file,
        })
    }
}

impl Drop for DaemonPidLock {
    fn drop(&mut self) {
        let expected = format!("{}\n", self.pid);
        match fs::read_to_string(&self.path) {
            Ok(current) if current == expected => {
                let _ = fs::remove_file(&self.path);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_creates_required_subdirs_and_readback_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let paths = validate_d_drive_root(temp.path()).expect("valid root");
        assert!(paths.storage_db.is_dir());
        assert!(paths.shift_log_dir.is_dir());
        assert!(paths.panel_db.is_dir());
        assert!(paths.fsv_dir.is_dir());
    }

    #[test]
    fn validator_rejects_missing_and_invalid_roots() {
        let missing = PathBuf::from("/tmp/contextgraph-definitely-missing-daemon-root");
        let err = validate_d_drive_root(&missing).expect_err("missing root must fail");
        assert!(matches!(
            err,
            DaemonValidationError::DDriveUnavailable { .. }
        ));

        let err = validate_d_drive_root(Path::new("relative"))
            .expect_err("relative root must fail closed");
        assert!(matches!(err, DaemonValidationError::PathInvalid { .. }));
    }

    #[test]
    fn daemon_pid_lock_rejects_second_owner() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("context-graph-daemon.pid");
        let first = DaemonPidLock::acquire(&path).expect("first lock");
        let err = DaemonPidLock::acquire(&path).expect_err("second lock must fail");
        assert!(matches!(err, DaemonValidationError::DaemonLockHeld { .. }));
        drop(first);
    }
}
