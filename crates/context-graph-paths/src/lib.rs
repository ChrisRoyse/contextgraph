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
pub const ENV_LOCAL_SMOKE_ISSUE: &str = "CONTEXTGRAPH_LOCAL_SMOKE_ISSUE";
pub const AIWONDER_DURABLE_ROOT: &str = "/zfs/archive/contextgraph";
pub const AIWONDER_HOT_ROOT: &str = "/zfs/hot/contextgraph";
pub const AIWONDER_EXPLICIT_SCRATCH_ROOT: &str = "/home/croyse/.cache/contextgraph";
pub const LEGACY_LOCAL_DATA_ROOT: &str = "/mnt/d/contextgraph";
pub const DEFAULT_DATA_ROOT: &str = AIWONDER_DURABLE_ROOT;

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
    let path = configured_data_root_path()?;
    validate_data_root(&path)?;
    Ok(path)
}

fn configured_data_root_path() -> Result<PathBuf> {
    let raw = match env::var(ENV_DATA_ROOT) {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) => {
            return Err(PathError::new(
                "CONTEXTGRAPH_DATA_ROOT_EMPTY",
                format!("{ENV_DATA_ROOT} is set but empty"),
                "set CONTEXTGRAPH_DATA_ROOT to /zfs/archive/contextgraph on aiwonder",
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
    Ok(PathBuf::from(raw))
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

pub fn mcp_daemon_pid_marker_path() -> Result<PathBuf> {
    ensure_subdir("state/locks").map(|dir| dir.join("context-graph-daemon.pid"))
}

pub fn mejepa_corpus_repo_cache_dir() -> Result<PathBuf> {
    ensure_subdir("cache/mejepa-corpus/repos")
}

pub fn mejepa_corpus_source_work_root() -> Result<PathBuf> {
    ensure_subdir("work/mejepa-corpus/source-work")
}

pub fn mejepa_corpus_root_dir() -> Result<PathBuf> {
    ensure_subdir("corpus")
}

pub fn require_under_mejepa_corpus_root(path: &Path, field: &'static str) -> Result<PathBuf> {
    let absolute = require_under_data_root(path, field)?;
    let corpus_root = normalize_absolute_lexical(&mejepa_corpus_root_dir()?)?;
    if !absolute.starts_with(&corpus_root) {
        return Err(PathError::new(
            "CONTEXTGRAPH_CORPUS_PATH_OUTSIDE_CORPUS_ROOT",
            format!(
                "{field}={} is outside corpus root {}",
                absolute.display(),
                corpus_root.display()
            ),
            "choose a path under /zfs/archive/contextgraph/corpus on aiwonder so generated corpus data survives reboot and is isolated from hot scratch",
        ));
    }
    Ok(absolute)
}

pub fn require_production_durable_root(path: &Path, field: &'static str) -> Result<PathBuf> {
    require_aiwonder_prefix(
        path,
        field,
        Path::new(AIWONDER_DURABLE_ROOT),
        "durable production root",
    )
}

pub fn require_production_durable_root_or_named_local_smoke(
    path: &Path,
    field: &'static str,
) -> Result<PathBuf> {
    let normalized = normalize_absolute_lexical(&make_absolute(path)?)?;
    if is_under_legacy_local_root(&normalized) {
        if named_local_smoke_issue().is_some() {
            return Ok(normalized);
        }
        return Err(production_root_not_aiwonder(
            field,
            &normalized,
            "local /mnt/d roots are allowed only when CONTEXTGRAPH_LOCAL_SMOKE_ISSUE names the issue-scoped local-smoke run",
        ));
    }
    require_production_durable_root(&normalized, field)
}

pub fn require_production_hot_root(path: &Path, field: &'static str) -> Result<PathBuf> {
    require_aiwonder_prefix(
        path,
        field,
        Path::new(AIWONDER_HOT_ROOT),
        "hot production root",
    )
}

pub fn require_production_hot_or_explicit_scratch_root(
    path: &Path,
    field: &'static str,
) -> Result<PathBuf> {
    let normalized = normalize_absolute_lexical(&make_absolute(path)?)?;
    if normalized.starts_with(AIWONDER_HOT_ROOT)
        || normalized.starts_with(AIWONDER_EXPLICIT_SCRATCH_ROOT)
    {
        return Ok(normalized);
    }
    Err(production_root_not_aiwonder(
        field,
        &normalized,
        "use /zfs/hot/contextgraph for hot production state or /home/croyse/.cache/contextgraph for explicitly budgeted aiwonder scratch",
    ))
}

pub fn production_data_root() -> Result<PathBuf> {
    let root = configured_data_root_path()?;
    let guarded = require_production_durable_root_or_named_local_smoke(&root, ENV_DATA_ROOT)?;
    validate_data_root(&guarded)?;
    Ok(guarded)
}

pub fn named_local_smoke_issue() -> Option<String> {
    match env::var(ENV_LOCAL_SMOKE_ISSUE) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
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
            "fix aiwonder permissions or create the directory manually under /zfs/archive/contextgraph",
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
            "choose a path under the configured durable data root; production roots must be on aiwonder /zfs/archive/contextgraph",
        ));
    }
    Ok(normalized)
}

/// Validator for **ephemeral** paths — caches and work directories whose
/// contents are deterministically rebuildable from durable inputs. Examples:
///
/// - `mejepa-corpus --repo-cache-dir`: bare git clones of public SWE-bench
///   repositories. Rebuildable via `git clone --bare https://github.com/...`.
/// - `mejepa-corpus --source-work-root`: per-task scratch checkouts that
///   live for the duration of a single mutation operation and are deleted
///   after the corpus row commits.
///
/// In the old WSL2 setup the data root `/mnt/d/contextgraph` lived behind the 9P filesystem
/// shim, which is ~100× slower than ext4 for small-file workloads (validated
/// via direct benchmark on 2026-05-11: 50 sequential `echo > file` calls
/// took 0.227 s on 9P vs 0.002 s on ext4). Forcing every ephemeral working
/// path through 9P added ~28 hours to corpus generation. Pinning ephemeral
/// state to ext4 restored native throughput while leaving durable corpus output
/// on the then-current D-drive. Post-migration production hot state belongs on
/// aiwonder `/zfs/hot/contextgraph` or explicitly budgeted aiwonder scratch.
///
/// Contract: the path MUST be absolute, lexically normalizable, and live
/// outside the C-drive policy deny list. It is explicitly allowed OUTSIDE
/// the data root (that is the entire point of this validator). Callers
/// must NEVER use this for durable artifacts — use `require_under_data_root`
/// for any state that must survive a reboot.
pub fn require_ephemeral_path(path: &Path, field: &'static str) -> Result<PathBuf> {
    let absolute = make_absolute(path)?;
    let normalized = normalize_absolute_lexical(&absolute)?;
    // Hard-deny the C-drive (and anything that would route into 9P windows
    // mounts other than /mnt/d/contextgraph). The data-root constraint is
    // intentionally NOT applied here.
    let denied_prefixes = ["/mnt/c/", "/mnt/c"];
    for prefix in &denied_prefixes {
        if normalized.to_string_lossy().starts_with(prefix) {
            return Err(PathError::new(
                "CONTEXTGRAPH_EPHEMERAL_PATH_ON_DENIED_MOUNT",
                format!(
                    "{field}={} lives on a denied mount (C-drive). Use aiwonder hot storage or the configured durable root.",
                    normalized.display()
                ),
                "pick an absolute path on aiwonder hot storage (/zfs/hot/contextgraph/...) or explicitly budgeted scratch",
            ));
        }
    }
    Ok(normalized)
}

fn validate_data_root(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(PathError::new(
            "CONTEXTGRAPH_DATA_ROOT_NOT_ABSOLUTE",
            format!("{} is not absolute", path.display()),
            "set CONTEXTGRAPH_DATA_ROOT to an absolute aiwonder path such as /zfs/archive/contextgraph",
        ));
    }
    let metadata = fs::metadata(path).map_err(|err| {
        PathError::new(
            "CONTEXTGRAPH_DATA_ROOT_MISSING",
            format!("{} is not available: {err}", path.display()),
            "create /zfs/archive/contextgraph on aiwonder or set CONTEXTGRAPH_DATA_ROOT to the correct aiwonder durable root",
        )
    })?;
    if !metadata.is_dir() {
        return Err(PathError::new(
            "CONTEXTGRAPH_DATA_ROOT_NOT_DIRECTORY",
            format!("{} exists but is not a directory", path.display()),
            "replace the conflicting path with the ContextGraph aiwonder durable data directory",
        ));
    }
    Ok(())
}

fn require_aiwonder_prefix(
    path: &Path,
    field: &'static str,
    prefix: &Path,
    kind: &'static str,
) -> Result<PathBuf> {
    let normalized = normalize_absolute_lexical(&make_absolute(path)?)?;
    if normalized.starts_with(prefix) {
        return Ok(normalized);
    }
    Err(production_root_not_aiwonder(
        field,
        &normalized,
        match kind {
            "durable production root" => {
                "production durable state must live under /zfs/archive/contextgraph on aiwonder"
            }
            _ => "production hot state must live under /zfs/hot/contextgraph on aiwonder",
        },
    ))
}

fn production_root_not_aiwonder(
    field: &'static str,
    path: &Path,
    remediation: &'static str,
) -> PathError {
    PathError::new(
        "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER",
        format!(
            "{field}={} is not an aiwonder production root",
            path.display()
        ),
        remediation,
    )
}

fn is_under_legacy_local_root(path: &Path) -> bool {
    path.starts_with(LEGACY_LOCAL_DATA_ROOT)
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
            "run from a readable working directory or pass an absolute durable path",
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

    #[test]
    fn corpus_paths_must_stay_under_corpus_root() {
        let _guard = env_lock().lock().expect("env lock");
        let prior = env::var(ENV_DATA_ROOT).ok();
        let root = test_root("corpus-root");
        env::set_var(ENV_DATA_ROOT, &root);

        let ok = require_under_mejepa_corpus_root(
            &root.join("corpus/swebench-lite-python-300x8-v1"),
            "corpus.output",
        )
        .expect("corpus path under corpus root");
        assert!(ok.starts_with(root.join("corpus")));

        let err = require_under_mejepa_corpus_root(
            &root.join("fsv/not-a-durable-corpus"),
            "corpus.output",
        )
        .expect_err("corpus path outside corpus root must fail");
        assert_eq!(err.code, "CONTEXTGRAPH_CORPUS_PATH_OUTSIDE_CORPUS_ROOT");

        restore_env(prior);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn production_durable_root_rejects_local_d_drive() {
        let err =
            require_production_durable_root(Path::new("/mnt/d/contextgraph/fsv/run"), "fsv_root")
                .expect_err("local D-drive must not count as production");
        assert_eq!(err.code, "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER");
    }

    #[test]
    fn production_durable_root_accepts_aiwonder_archive() {
        let path = require_production_durable_root(
            Path::new("/zfs/archive/contextgraph/fsv/task-aiwonder-014"),
            "fsv_root",
        )
        .expect("aiwonder archive is production durable root");
        assert_eq!(
            path,
            PathBuf::from("/zfs/archive/contextgraph/fsv/task-aiwonder-014")
        );
    }

    #[test]
    fn production_data_root_checks_env_var_root() {
        let _guard = env_lock().lock().expect("env lock");
        let prior_data = env::var(ENV_DATA_ROOT).ok();
        let prior_smoke = env::var(ENV_LOCAL_SMOKE_ISSUE).ok();
        env::set_var(ENV_DATA_ROOT, "/mnt/d/contextgraph");
        env::remove_var(ENV_LOCAL_SMOKE_ISSUE);

        let err = production_data_root().expect_err("production env root must reject local D");
        assert_eq!(err.code, "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER");

        env::set_var(ENV_LOCAL_SMOKE_ISSUE, "issue-365-local-smoke-test");
        let local = require_production_durable_root_or_named_local_smoke(
            Path::new("/mnt/d/contextgraph"),
            ENV_DATA_ROOT,
        )
        .expect("named local smoke may use local D root without production closeout");
        assert_eq!(local, PathBuf::from("/mnt/d/contextgraph"));

        restore_env(prior_data);
        restore_local_smoke_env(prior_smoke);
    }

    #[test]
    fn production_hot_root_allows_aiwonder_hot_and_named_scratch() {
        let hot = require_production_hot_or_explicit_scratch_root(
            Path::new("/zfs/hot/contextgraph/runtime/run"),
            "hot_root",
        )
        .expect("aiwonder hot root accepted");
        assert_eq!(hot, PathBuf::from("/zfs/hot/contextgraph/runtime/run"));

        let scratch = require_production_hot_or_explicit_scratch_root(
            Path::new("/home/croyse/.cache/contextgraph/run"),
            "hot_root",
        )
        .expect("explicit aiwonder scratch root accepted");
        assert_eq!(
            scratch,
            PathBuf::from("/home/croyse/.cache/contextgraph/run")
        );

        let err = require_production_hot_or_explicit_scratch_root(
            Path::new("/home/cabdru/.cache/contextgraph/run"),
            "hot_root",
        )
        .expect_err("local scratch must not count as production aiwonder scratch");
        assert_eq!(err.code, "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER");
    }

    fn restore_env(prior: Option<String>) {
        if let Some(value) = prior {
            env::set_var(ENV_DATA_ROOT, value);
        } else {
            env::remove_var(ENV_DATA_ROOT);
        }
    }

    fn restore_local_smoke_env(prior: Option<String>) {
        if let Some(value) = prior {
            env::set_var(ENV_LOCAL_SMOKE_ISSUE, value);
        } else {
            env::remove_var(ENV_LOCAL_SMOKE_ISSUE);
        }
    }
}
