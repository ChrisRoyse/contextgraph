use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::entry::HygieneEntryMeta;
use crate::error::{OpsError, OpsResult};

pub const DEFAULT_RETENTION_POLICY_PATH: &str = "config/retention_policy.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionRule {
    pub cf_name: String,
    pub retention_class: String,
    pub minimum_retention_days: u32,
}

impl RetentionRule {
    pub fn validate(&self) -> OpsResult<()> {
        if self.cf_name.trim().is_empty() {
            return Err(OpsError::invalid(
                "retention.cf_name",
                "cf_name must be non-empty",
            ));
        }
        if self.retention_class.trim().is_empty() {
            return Err(OpsError::invalid(
                "retention.retention_class",
                format!("retention class missing for {}", self.cf_name),
            ));
        }
        Ok(())
    }

    pub fn retained_until_unix(&self, created_unix: i64) -> i64 {
        created_unix.saturating_add(self.minimum_retention_days as i64 * 86_400)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionPolicy {
    pub column_families: Vec<RetentionRule>,
}

impl RetentionPolicy {
    pub fn validate(&self) -> OpsResult<()> {
        let expected = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for rule in &self.column_families {
            rule.validate()?;
            if !expected.contains(rule.cf_name.as_str()) {
                return Err(OpsError::invalid(
                    "retention.cf_name",
                    format!("unknown retention CF {}", rule.cf_name),
                ));
            }
            if !seen.insert(rule.cf_name.clone()) {
                return Err(OpsError::invalid(
                    "retention.cf_name",
                    format!("duplicate retention CF {}", rule.cf_name),
                ));
            }
        }
        let seen_refs = seen.iter().map(String::as_str).collect::<BTreeSet<_>>();
        let missing = expected
            .iter()
            .filter(|cf_name| !seen_refs.contains(**cf_name))
            .map(|cf_name| (*cf_name).to_string())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(OpsError::invalid(
                "retention.column_families",
                format!("retention policy missing CFs: {}", missing.join(", ")),
            ));
        }
        Ok(())
    }

    pub fn by_cf(&self) -> OpsResult<BTreeMap<String, RetentionRule>> {
        self.validate()?;
        Ok(self
            .column_families
            .iter()
            .map(|rule| (rule.cf_name.clone(), rule.clone()))
            .collect())
    }
}

pub fn default_retention_policy_path() -> PathBuf {
    std::env::var("CONTEXTGRAPH_RETENTION_POLICY")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_RETENTION_POLICY_PATH))
}

pub fn load_retention_policy(path: impl AsRef<Path>) -> OpsResult<RetentionPolicy> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path)
        .map_err(|err| OpsError::io("read_retention_policy", path, err))?;
    let policy: RetentionPolicy = toml::from_str(&raw).map_err(|err| {
        OpsError::invalid(
            "retention_policy",
            format!("failed to parse {}: {err}", path.display()),
        )
    })?;
    policy.validate()?;
    Ok(policy)
}

pub fn retained_by_rule(meta: &HygieneEntryMeta, rule: &RetentionRule, now_unix: i64) -> bool {
    now_unix < rule.retained_until_unix(meta.created_unix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_must_cover_all_hygiene_referenced_cfs() {
        let policy = RetentionPolicy {
            column_families: Vec::new(),
        };
        let err = policy.validate().unwrap_err();
        assert_eq!(err.code, "MEJEPA_HYGIENE_INVALID_CONFIG");
    }

    #[test]
    fn default_policy_declares_feedback_and_override_retention() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/retention_policy.toml");
        let policy = load_retention_policy(path).unwrap();
        let by_cf = policy.by_cf().unwrap();

        for cf_name in [
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
            context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_OVERRIDES,
        ] {
            let rule = by_cf.get(cf_name).expect("CF must have a retention rule");
            assert_eq!(rule.minimum_retention_days, 365);
        }
    }
}
