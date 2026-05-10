use crate::embedder_id::{EmbedderId, EmbedderKind};
use crate::error::{EmbedError, EmbedResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const MODELS_CONFIG_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsConfig {
    pub schema_version: u16,
    pub embedders: BTreeMap<String, EmbedderRegistration>,
}

impl ModelsConfig {
    pub fn load(path: impl AsRef<Path>) -> EmbedResult<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|err| EmbedError::ConfigRead {
            path: path.to_path_buf(),
            message: err.to_string(),
            remediation:
                "create config/models_config.toml with the Phase 1b schema and real D: model paths",
        })?;
        let config: Self = toml::from_str(&text).map_err(|err| EmbedError::ConfigParse {
            path: path.to_path_buf(),
            message: err.to_string(),
            remediation:
                "fix models_config.toml to match schema_version=1 with [embedders.eN] tables",
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> EmbedResult<()> {
        if self.schema_version != MODELS_CONFIG_SCHEMA_VERSION {
            return Err(EmbedError::invalid(
                "ModelsConfig.schema_version",
                format!(
                    "unsupported schema version {}; expected {}",
                    self.schema_version, MODELS_CONFIG_SCHEMA_VERSION
                ),
                "regenerate the config with the current Phase 1b schema; no migration fallback exists",
            ));
        }
        let mut seen = BTreeSet::new();
        for id in EmbedderId::all() {
            let key = id.slug();
            let reg = self
                .embedders
                .get(key)
                .ok_or(EmbedError::MissingRegistration {
                    embedder: id,
                    remediation: "add a complete [embedders.eN] entry to models_config.toml",
                })?;
            reg.validate()?;
            if reg.embedder != id {
                return Err(EmbedError::invalid(
                    "EmbedderRegistration.embedder",
                    format!("table {key} declares embedder {}", reg.embedder),
                    "the table name and embedder field must identify the same slot",
                ));
            }
            if !seen.insert(reg.embedder) {
                return Err(EmbedError::invalid(
                    "ModelsConfig.embedders",
                    format!("duplicate registration for {}", reg.embedder),
                    "register each E1-E21 slot exactly once",
                ));
            }
        }
        Ok(())
    }

    pub fn registration(&self, id: EmbedderId) -> EmbedResult<&EmbedderRegistration> {
        self.embedders
            .get(id.slug())
            .ok_or(EmbedError::MissingRegistration {
                embedder: id,
                remediation: "add the missing [embedders.eN] entry to models_config.toml",
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedderRegistration {
    pub embedder: EmbedderId,
    pub name: String,
    pub kind: EmbedderKind,
    pub path: String,
    pub repo: Option<String>,
    pub dimension: usize,
    pub weight_files: Vec<String>,
    pub manifest_sha256: String,
}

impl EmbedderRegistration {
    pub fn validate(&self) -> EmbedResult<()> {
        if self.name.trim().is_empty() {
            return Err(EmbedError::invalid(
                "EmbedderRegistration.name",
                format!("{} registration name is empty", self.embedder),
                "use the canonical model role name from the Phase 1b spec",
            ));
        }
        if self.kind != self.embedder.kind() {
            return Err(EmbedError::invalid(
                "EmbedderRegistration.kind",
                format!(
                    "{} registration kind {:?} does not match expected {:?}",
                    self.embedder,
                    self.kind,
                    self.embedder.kind()
                ),
                "fix the registration kind instead of routing around the slot contract",
            ));
        }
        if self.dimension != self.embedder.dimension() {
            return Err(EmbedError::invalid(
                "EmbedderRegistration.dimension",
                format!(
                    "{} dimension {} does not match expected {}",
                    self.embedder,
                    self.dimension,
                    self.embedder.dimension()
                ),
                "fix the wrapper projection or the config before loading weights",
            ));
        }
        match self.kind {
            EmbedderKind::ContentDeterministic => {
                if !self.weight_files.is_empty() || !self.manifest_sha256.is_empty() {
                    return Err(EmbedError::invalid(
                        "EmbedderRegistration.weight_files",
                        format!(
                            "{} is deterministic but declares weight files",
                            self.embedder
                        ),
                        "remove pretrained weight pins from deterministic embedders E2-E4/E9",
                    ));
                }
            }
            EmbedderKind::ContentPretrained | EmbedderKind::LearnerState => {
                if self.path.trim().is_empty() {
                    return Err(EmbedError::invalid(
                        "EmbedderRegistration.path",
                        format!("{} pretrained registration path is empty", self.embedder),
                        "point to the real model directory on D:",
                    ));
                }
                if self.weight_files.is_empty() {
                    return Err(EmbedError::invalid(
                        "EmbedderRegistration.weight_files",
                        format!("{} has no pinned weight files", self.embedder),
                        "list every safetensors shard needed to load this embedder",
                    ));
                }
                validate_sha256(
                    "EmbedderRegistration.manifest_sha256",
                    &self.manifest_sha256,
                )?;
            }
        }
        Ok(())
    }
}

fn validate_sha256(field: &'static str, value: &str) -> EmbedResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(EmbedError::invalid(
            field,
            format!("expected 64 lowercase hex chars, got {value:?}"),
            "write a real lowercase SHA-256 digest",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_requires_all_slots_and_exact_schema() {
        let config = ModelsConfig {
            schema_version: MODELS_CONFIG_SCHEMA_VERSION,
            embedders: BTreeMap::new(),
        };
        assert_eq!(
            config.validate().unwrap_err().code(),
            "MEJEPA_EMBED_REGISTRATION_MISSING"
        );
    }

    #[test]
    fn deterministic_slots_reject_weight_pins() {
        let reg = EmbedderRegistration {
            embedder: EmbedderId::E2,
            name: "temporal_recent".into(),
            kind: EmbedderKind::ContentDeterministic,
            path: String::new(),
            repo: None,
            dimension: EmbedderId::E2.dimension(),
            weight_files: vec!["model.safetensors".into()],
            manifest_sha256: "a".repeat(64),
        };
        assert_eq!(
            reg.validate().unwrap_err().code(),
            "MEJEPA_EMBED_INVALID_INPUT"
        );
    }
}
