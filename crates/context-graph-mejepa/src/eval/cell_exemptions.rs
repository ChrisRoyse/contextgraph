use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::error::{EvalError, EvalErrorCode};

pub const DEFAULT_CELL_EXEMPTIONS_PATH: &str = "config/cell_exemptions.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellExemption {
    pub cell: String,
    pub reason: String,
    pub operator_id: String,
    pub expires_unix_ms: Option<i64>,
}

impl CellExemption {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.cell.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "cell exemption cell must be non-empty",
            ));
        }
        if self.reason.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("cell exemption {} missing operator reason", self.cell),
            ));
        }
        if self.operator_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("cell exemption {} missing operator_id", self.cell),
            ));
        }
        if self.expires_unix_ms == Some(0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "cell exemption {} expires_unix_ms must be non-zero",
                    self.cell
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellExemptionConfig {
    pub exemptions: Vec<CellExemption>,
}

impl CellExemptionConfig {
    pub fn validate(&self) -> Result<(), EvalError> {
        let mut seen = BTreeMap::new();
        for exemption in &self.exemptions {
            exemption.validate()?;
            if seen.insert(exemption.cell.clone(), ()).is_some() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("duplicate cell exemption {}", exemption.cell),
                ));
            }
        }
        Ok(())
    }

    pub fn active_map(
        &self,
        now_unix_ms: i64,
    ) -> Result<BTreeMap<String, CellExemption>, EvalError> {
        self.validate()?;
        Ok(self
            .exemptions
            .iter()
            .filter(|exemption| {
                exemption
                    .expires_unix_ms
                    .map(|expires| now_unix_ms < expires)
                    .unwrap_or(true)
            })
            .map(|exemption| (exemption.cell.clone(), exemption.clone()))
            .collect())
    }
}

pub fn load_cell_exemptions(
    path: impl AsRef<Path>,
    now_unix_ms: i64,
) -> Result<BTreeMap<String, CellExemption>, EvalError> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = std::fs::read_to_string(path).map_err(|err| {
        EvalError::new(
            EvalErrorCode::Store,
            format!("failed to read cell exemptions {}: {err}", path.display()),
        )
    })?;
    let parsed: CellExemptionConfig = toml::from_str(&raw).map_err(|err| {
        EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("failed to parse cell exemptions {}: {err}", path.display()),
        )
    })?;
    parsed.active_map(now_unix_ms)
}

pub fn default_cell_exemptions_path() -> PathBuf {
    std::env::var("CONTEXTGRAPH_CELL_EXEMPTIONS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_CELL_EXEMPTIONS_PATH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_exemptions_do_not_enter_active_map() {
        let config = CellExemptionConfig {
            exemptions: vec![
                CellExemption {
                    cell: "compile_error::python".to_string(),
                    reason: "operator approved temporary blind cell".to_string(),
                    operator_id: "operator".to_string(),
                    expires_unix_ms: Some(100),
                },
                CellExemption {
                    cell: "off_by_one::rust".to_string(),
                    reason: "operator approved active blind cell".to_string(),
                    operator_id: "operator".to_string(),
                    expires_unix_ms: None,
                },
            ],
        };
        let active = config.active_map(1_000).unwrap();
        assert!(!active.contains_key("compile_error::python"));
        assert!(active.contains_key("off_by_one::rust"));
    }

    #[test]
    fn repository_cell_exemptions_config_parses() {
        let raw = include_str!("../../../../config/cell_exemptions.toml");
        let config: CellExemptionConfig = toml::from_str(raw).unwrap();
        let active = config.active_map(1_800_000_000_000).unwrap();
        let exemption = active.get("delete_test_call::python").unwrap();
        assert_eq!(exemption.operator_id, "github-issue-810");
        assert!(exemption.reason.contains("#810"));
    }
}
