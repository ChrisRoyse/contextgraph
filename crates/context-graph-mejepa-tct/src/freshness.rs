use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::constellation::TctConstellation;
use crate::error::TctError;

pub const DEFAULT_MAX_AGE_DAYS: u32 = 90;
pub const ENV_ALLOW_STALE: &str = "MEJEPA_TCT_ALLOW_STALE";
pub const ENV_MAX_AGE_DAYS: &str = "MEJEPA_TCT_MAX_AGE_DAYS";

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

pub fn audit_log_override(
    version_id: [u8; 32],
    frozen_at: SystemTime,
    age_days: u32,
    max_age_days: u32,
) -> Result<(), TctError> {
    audit_log_override_at(
        Path::new("./memory/journal"),
        version_id,
        frozen_at,
        age_days,
        max_age_days,
    )
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
