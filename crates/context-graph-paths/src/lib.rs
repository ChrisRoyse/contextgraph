//! Durable path resolution for ContextGraph.
//!
//! This crate intentionally does not fall back to `/tmp`, the current working
//! directory, or `$HOME`. Durable data must live under the configured data root.

use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const ENV_DATA_ROOT: &str = "CONTEXTGRAPH_DATA_ROOT";
pub const DEFAULT_DATA_ROOT: &str = "/mnt/d/contextgraph";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    pub code: &'static str,
    pub message: String,
    pub remediation: &'static str,
}

impl PathError {
    fn new(code: &'static str, message: impl Into<String>, remediation: &'static str) -> Self {
        Self {
            code,
            message: message.into(),
            remediation,
        }
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}; remediation: {}",
            self.code, self.message, self.remediation
        )
    }
}

impl Error for PathError {}

pub type Result<T> = std::result::Result<T, PathError>;

pub fn data_root() -> Result<PathBuf> {
    let raw = match env::var(ENV_DATA_ROOT) {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) => {
            return Err(PathError::new(
                "CONTEXTGRAPH_DATA_ROOT_EMPTY",
                format!("{ENV_DATA_ROOT} is set but empty"),
                "set CONTEXTGRAPH_DATA_ROOT to /mnt/d/contextgraph or unset it to use the project D: root",
            ))
        }
        Err(env::VarError::NotPresent) => DEFAULT_DATA_ROOT.to_string(),
        Err(err) => {
            return Err(PathError::new(
                "CONTEXTGRAPH_DATA_ROOT_UNREADABLE",
                format!("failed to read {ENV_DATA_ROOT}: {err}"),
                "repair the environment so CONTEXTGRAPH_DATA_ROOT is valid UTF-8",
            ))
        }
    };
    let path = PathBuf::from(raw);
    validate_data_root(&path)?;
    Ok(path)
}

pub fn durable_storage_path() -> Result<PathBuf> {
    ensure_subdir("storage/contextgraph-rocksdb")
}

pub fn cgreality_state_dir() -> Result<PathBuf> {
    ensure_subdir("state/cgreality")
}

pub fn cgreality_cache_file(name: &str) -> Result<PathBuf> {
    ensure_safe_file_name(name, "cgreality cache file")?;
    Ok(cgreality_state_dir()?.join(name))
}

pub fn cgreality_runtime_root() -> Result<PathBuf> {
    ensure_subdir("runtime/cgreality")
}

pub fn cgreality_singleton_lock_path() -> Result<PathBuf> {
    ensure_subdir("state/locks").map(|dir| dir.join("reality-loop-stdio.lock"))
}

pub fn mejepa_corpus_repo_cache_dir() -> Result<PathBuf> {
    ensure_subdir("cache/mejepa-corpus/repos")
}

pub fn mejepa_corpus_source_work_root() -> Result<PathBuf> {
    ensure_subdir("work/mejepa-corpus/source-work")
}

pub fn mejepa_image_prep_log_dir() -> Result<PathBuf> {
    ensure_subdir("logs/mejepa-image-prep")
}

pub fn swebench_harness_lock_path() -> Result<PathBuf> {
    ensure_subdir("state/locks").map(|dir| dir.join("mejepa-corpus-swebench-harness.lock"))
}

pub fn user_data_dir() -> Result<PathBuf> {
    ensure_subdir("user-data")
}

pub fn ensure_subdir(relative: impl AsRef<Path>) -> Result<PathBuf> {
    let relative = relative.as_ref();
    validate_relative_path(relative, "data-root subdirectory")?;
    let path = data_root()?.join(relative);
    fs::create_dir_all(&path).map_err(|err| {
        PathError::new(
            "CONTEXTGRAPH_DATA_SUBDIR_CREATE_FAILED",
            format!("failed to create {}: {err}", path.display()),
            "fix D: permissions or create the directory manually under /mnt/d/contextgraph",
        )
    })?;
    let metadata = fs::metadata(&path).map_err(|err| {
        PathError::new(
            "CONTEXTGRAPH_DATA_SUBDIR_STAT_FAILED",
            format!("failed to stat {}: {err}", path.display()),
            "inspect the D: mount and directory permissions",
        )
    })?;
    if !metadata.is_dir() {
        return Err(PathError::new(
            "CONTEXTGRAPH_DATA_SUBDIR_NOT_DIRECTORY",
            format!("{} exists but is not a directory", path.display()),
            "remove or rename the conflicting file and recreate the expected directory",
        ));
    }
    Ok(path)
}

pub fn require_under_data_root(path: &Path, field: &'static str) -> Result<PathBuf> {
    let absolute = make_absolute(path)?;
    let normalized = normalize_absolute_lexical(&absolute)?;
    let root = normalize_absolute_lexical(&data_root()?)?;
    if !normalized.starts_with(&root) {
        return Err(PathError::new(
            "CONTEXTGRAPH_DURABLE_PATH_OUTSIDE_DATA_ROOT",
            format!(
                "{field}={} is outside data root {}",
                normalized.display(),
                root.display()
            ),
            "choose a path under /mnt/d/contextgraph so durable data is not written to C: or /tmp",
        ));
    }
    Ok(normalized)
}

fn validate_data_root(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(PathError::new(
            "CONTEXTGRAPH_DATA_ROOT_NOT_ABSOLUTE",
            format!("{} is not absolute", path.display()),
            "set CONTEXTGRAPH_DATA_ROOT to an absolute D: mount path such as /mnt/d/contextgraph",
        ));
    }
    let metadata = fs::metadata(path).map_err(|err| {
        PathError::new(
            "CONTEXTGRAPH_DATA_ROOT_MISSING",
            format!("{} is not available: {err}", path.display()),
            "mount D: in WSL and create /mnt/d/contextgraph before starting ContextGraph",
        )
    })?;
    if !metadata.is_dir() {
        return Err(PathError::new(
            "CONTEXTGRAPH_DATA_ROOT_NOT_DIRECTORY",
            format!("{} exists but is not a directory", path.display()),
            "replace the conflicting path with the ContextGraph D: data directory",
        ));
    }
    Ok(())
}

fn validate_relative_path(path: &Path, label: &'static str) -> Result<()> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(PathError::new(
            "CONTEXTGRAPH_RELATIVE_PATH_INVALID",
            format!(
                "{label} must be a non-empty relative path: {}",
                path.display()
            ),
            "use a relative path below the ContextGraph data root",
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(PathError::new(
                    "CONTEXTGRAPH_RELATIVE_PATH_TRAVERSAL",
                    format!("{label} contains unsafe component: {}", path.display()),
                    "remove root, prefix, current-dir, and parent-dir components",
                ))
            }
        }
    }
    Ok(())
}

fn ensure_safe_file_name(name: &str, label: &'static str) -> Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name == "."
        || name == ".."
    {
        return Err(PathError::new(
            "CONTEXTGRAPH_FILE_NAME_INVALID",
            format!("{label} is not a safe file name: {name:?}"),
            "use a plain file name with no separators or traversal components",
        ));
    }
    Ok(())
}

fn make_absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    env::current_dir().map(|cwd| cwd.join(path)).map_err(|err| {
        PathError::new(
            "CONTEXTGRAPH_CURRENT_DIR_UNREADABLE",
            format!(
                "failed to resolve current directory for {}: {err}",
                path.display()
            ),
            "run from a readable working directory or pass an absolute D-backed path",
        )
    })
}

fn normalize_absolute_lexical(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(PathError::new(
            "CONTEXTGRAPH_PATH_NOT_ABSOLUTE",
            format!("{} is not absolute", path.display()),
            "pass an absolute path or resolve it before durable path validation",
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(PathError::new(
                        "CONTEXTGRAPH_PATH_TRAVERSAL_INVALID",
                        format!("{} escapes above filesystem root", path.display()),
                        "remove parent-directory traversal from the durable path",
                    ));
                }
            }
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = env::temp_dir().join(format!("contextgraph-paths-{name}-{stamp}"));
        fs::create_dir_all(&path).expect("create test root");
        path
    }

    #[test]
    fn explicit_data_root_must_be_absolute() {
        let _guard = env_lock().lock().expect("env lock");
        let prior = env::var(ENV_DATA_ROOT).ok();
        env::set_var(ENV_DATA_ROOT, "relative-root");
        let err = data_root().expect_err("relative root must fail");
        assert_eq!(err.code, "CONTEXTGRAPH_DATA_ROOT_NOT_ABSOLUTE");
        restore_env(prior);
    }

    #[test]
    fn durable_path_must_stay_under_root() {
        let _guard = env_lock().lock().expect("env lock");
        let prior = env::var(ENV_DATA_ROOT).ok();
        let root = test_root("under-root");
        env::set_var(ENV_DATA_ROOT, &root);
        let ok = require_under_data_root(&root.join("storage/db"), "storage").expect("under root");
        assert!(ok.starts_with(&root));
        let err = require_under_data_root(Path::new("/tmp/not-contextgraph"), "storage")
            .expect_err("outside root must fail");
        assert_eq!(err.code, "CONTEXTGRAPH_DURABLE_PATH_OUTSIDE_DATA_ROOT");
        restore_env(prior);
        let _ = fs::remove_dir_all(root);
    }

    fn restore_env(prior: Option<String>) {
        if let Some(value) = prior {
            env::set_var(ENV_DATA_ROOT, value);
        } else {
            env::remove_var(ENV_DATA_ROOT);
        }
    }
}
