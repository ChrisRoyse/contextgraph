use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::constellation::TctConstellation;
use crate::error::TctError;

pub const DEFAULT_MAX_AGE_DAYS: u32 = 90;
pub const ENV_ALLOW_STALE: &str = "MEJEPA_TCT_ALLOW_STALE";
pub const ENV_MAX_AGE_DAYS: &str = "MEJEPA_TCT_MAX_AGE_DAYS";
/// Optional explicit override for the directory that receives the
/// `MEJEPA_TCT_ALLOW_STALE=true` audit-log entries. When unset the audit log
/// is sited under `<context-graph-paths::data_root>/runtime/tct/allow-stale-audits/`
/// so it lives on aiwonder durable storage per CLAUDE.md §6.7. The retired
/// `./memory/journal/...` fallback (Sherlock #5 finding, 2026-05-22) has been
/// removed: writing into `./memory/` violates CLAUDE.md §4 (the directory is
/// archived) and §5 (no scratch markdown in the project root).
pub const ENV_ALLOW_STALE_AUDIT_DIR: &str = "MEJEPA_TCT_ALLOW_STALE_AUDIT_DIR";

pub fn read_freshness_config() -> Result<(u32, bool), TctError> {
    let max_age_days = match std::env::var(ENV_MAX_AGE_DAYS) {
        Ok(value) => value.parse::<u32>().map_err(|err| {
            TctError::invalid(
                ENV_MAX_AGE_DAYS,
                format!("must parse as u32 days, got {value:?}: {err}"),
            )
        })?,
        Err(std::env::VarError::NotPresent) => DEFAULT_MAX_AGE_DAYS,
        Err(err) => {
            return Err(TctError::invalid(
                ENV_MAX_AGE_DAYS,
                format!("failed to read env var: {err}"),
            ));
        }
    };
    let allow_stale = match std::env::var(ENV_ALLOW_STALE) {
        Ok(value) if value == "true" => true,
        Ok(value) if value == "false" => false,
        Ok(value) => {
            return Err(TctError::invalid(
                ENV_ALLOW_STALE,
                format!("must be exactly true or false, got {value:?}"),
            ));
        }
        Err(std::env::VarError::NotPresent) => false,
        Err(err) => {
            return Err(TctError::invalid(
                ENV_ALLOW_STALE,
                format!("failed to read env var: {err}"),
            ));
        }
    };
    Ok((max_age_days, allow_stale))
}

pub fn check_freshness(
    constellation: &TctConstellation,
    max_age_days: u32,
    allow_stale: bool,
) -> Result<(), TctError> {
    let now = SystemTime::now();
    let age = now.duration_since(constellation.frozen_at).map_err(|_| {
        TctError::invalid(
            "TctConstellation.frozen_at",
            "frozen_at is in the future relative to system clock",
        )
    })?;
    let age_days = (age.as_secs() / 86_400) as u32;
    if age_days > max_age_days {
        if allow_stale {
            audit_log_override(
                constellation.version_id,
                constellation.frozen_at,
                age_days,
                max_age_days,
            )?;
            tracing::warn!(
                version_id = %hex::encode(constellation.version_id),
                age_days,
                max_age_days,
                "stale TCT constellation override accepted"
            );
            return Ok(());
        }
        let frozen_at_iso =
            chrono::DateTime::<chrono::Utc>::from(constellation.frozen_at).to_rfc3339();
        return Err(TctError::StaleConstellation {
            frozen_at_iso,
            age_days,
            max_age_days,
        });
    }
    Ok(())
}

/// Resolve the directory that receives `MEJEPA_TCT_ALLOW_STALE=true` audit-log
/// entries. Order of precedence:
///
///  1. `MEJEPA_TCT_ALLOW_STALE_AUDIT_DIR` (explicit operator override).
///  2. `context_graph_paths::ensure_subdir("runtime/tct/allow-stale-audits")`
///     (durable aiwonder root per CLAUDE.md §6.7).
///
/// Fails closed if neither resolves: the audit log is the SoT for the override,
/// so an unwritable audit dir means we MUST refuse the override rather than
/// silently lose the record.
pub fn audit_log_override_dir() -> Result<PathBuf, TctError> {
    if let Ok(value) = std::env::var(ENV_ALLOW_STALE_AUDIT_DIR) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(TctError::invalid(
                ENV_ALLOW_STALE_AUDIT_DIR,
                "audit directory env var is set but empty after trimming; \
                 unset it to use the aiwonder durable default, or set it to an \
                 absolute writable path",
            ));
        }
        let path = PathBuf::from(trimmed);
        if !path.is_absolute() {
            return Err(TctError::invalid(
                ENV_ALLOW_STALE_AUDIT_DIR,
                format!("must be an absolute path; got {trimmed:?}"),
            ));
        }
        return Ok(path);
    }
    context_graph_paths::ensure_subdir("runtime/tct/allow-stale-audits").map_err(|err| {
        TctError::invalid(
            "tct.audit_log_dir",
            format!(
                "could not resolve durable audit-log directory: {err}; set \
                 {ENV_ALLOW_STALE_AUDIT_DIR} to an absolute writable path"
            ),
        )
    })
}

pub fn audit_log_override(
    version_id: [u8; 32],
    frozen_at: SystemTime,
    age_days: u32,
    max_age_days: u32,
) -> Result<(), TctError> {
    let dir = audit_log_override_dir()?;
    audit_log_override_at(&dir, version_id, frozen_at, age_days, max_age_days)
}

pub fn audit_log_override_at(
    journal_dir: &Path,
    version_id: [u8; 32],
    frozen_at: SystemTime,
    age_days: u32,
    max_age_days: u32,
) -> Result<(), TctError> {
    fs::create_dir_all(journal_dir)
        .map_err(|source| TctError::io("create_dir_all", journal_dir, source))?;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let id = uuid::Uuid::new_v4();
    let path = journal_dir.join(format!("agent-tct-allow-stale-{id}.md"));
    let body = format!(
        "---\nnamespace: journal\nagent_id: agent-tct-allow-stale\ntimestamp: {timestamp}\nstatus: active\ntags: tct,override\n---\n\n# TCT stale constellation override\n\n- version_id: {}\n- frozen_at: {}\n- age_days: {age_days}\n- max_age_days: {max_age_days}\n- override_via: {ENV_ALLOW_STALE}=true\n",
        hex::encode(version_id),
        chrono::DateTime::<chrono::Utc>::from(frozen_at).to_rfc3339(),
    );
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .map_err(|source| TctError::io("open", &path, source))?;
    file.write_all(body.as_bytes())
        .map_err(|source| TctError::io("write", &path, source))?;
    Ok(())
}

pub fn days_ago(days: u64) -> SystemTime {
    SystemTime::now() - Duration::from_secs(days * 86_400)
}

#[cfg(test)]
mod audit_log_dir_tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Serialize env mutations because freshness env vars are process-global.
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn restore_env(key: &str, prior: Option<String>) {
        if let Some(value) = prior {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn explicit_env_var_must_be_absolute() {
        let _guard = env_lock().lock().expect("env lock");
        let prior = std::env::var(ENV_ALLOW_STALE_AUDIT_DIR).ok();
        std::env::set_var(ENV_ALLOW_STALE_AUDIT_DIR, "relative/tct-audit");
        let err = audit_log_override_dir().expect_err("relative dir must fail");
        assert_eq!(err.code(), "MEJEPA_TCT_INVALID_INPUT");
        let detail = format!("{err}");
        assert!(
            detail.contains("absolute"),
            "error must explain the absolute-path requirement: {detail}"
        );
        restore_env(ENV_ALLOW_STALE_AUDIT_DIR, prior);
    }

    #[test]
    fn empty_explicit_env_var_fails_closed() {
        let _guard = env_lock().lock().expect("env lock");
        let prior = std::env::var(ENV_ALLOW_STALE_AUDIT_DIR).ok();
        std::env::set_var(ENV_ALLOW_STALE_AUDIT_DIR, "   ");
        let err = audit_log_override_dir().expect_err("whitespace must fail");
        assert_eq!(err.code(), "MEJEPA_TCT_INVALID_INPUT");
        restore_env(ENV_ALLOW_STALE_AUDIT_DIR, prior);
    }

    #[test]
    fn explicit_env_var_round_trips_absolute_path() {
        let _guard = env_lock().lock().expect("env lock");
        let prior = std::env::var(ENV_ALLOW_STALE_AUDIT_DIR).ok();
        let temp = tempfile::tempdir().expect("tempdir");
        let dir = temp.path().join("tct-audit-explicit");
        std::env::set_var(ENV_ALLOW_STALE_AUDIT_DIR, &dir);
        let resolved = audit_log_override_dir().expect("explicit path must resolve");
        assert_eq!(resolved, dir);
        restore_env(ENV_ALLOW_STALE_AUDIT_DIR, prior);
    }

    #[test]
    fn audit_log_writes_under_resolved_dir_not_memory_journal() {
        // Sherlock #5 regression: refuse to write into the retired
        // ./memory/journal/ directory. The audit log MUST land at the
        // explicit env override (or aiwonder durable root) instead.
        let _guard = env_lock().lock().expect("env lock");
        let prior = std::env::var(ENV_ALLOW_STALE_AUDIT_DIR).ok();
        let temp = tempfile::tempdir().expect("tempdir");
        let dir = temp.path().join("tct-audit-write");
        std::env::set_var(ENV_ALLOW_STALE_AUDIT_DIR, &dir);
        let resolved = audit_log_override_dir().expect("explicit path must resolve");
        assert_eq!(resolved, dir);
        audit_log_override_at(&resolved, [7u8; 32], SystemTime::UNIX_EPOCH, 120, 90)
            .expect("audit log must write into the resolved directory");
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("audit dir exists")
            .filter_map(Result::ok)
            .collect();
        assert_eq!(
            entries.len(),
            1,
            "exactly one audit file expected in {}",
            dir.display()
        );
        let path = entries[0].path();
        assert!(
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("agent-tct-allow-stale-"))
                .unwrap_or(false),
            "audit filename must start with agent-tct-allow-stale-: {}",
            path.display()
        );
        assert!(
            !path.to_string_lossy().contains("memory/journal"),
            "audit log must NOT land in the retired ./memory/journal/ directory: {}",
            path.display()
        );
        let body = std::fs::read_to_string(&path).expect("read audit file");
        assert!(body.contains("agent-tct-allow-stale"));
        assert!(body.contains(&format!("age_days: {}", 120)));
        restore_env(ENV_ALLOW_STALE_AUDIT_DIR, prior);
    }
}
