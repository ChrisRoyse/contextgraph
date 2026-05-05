use super::errors::{CCRealityError, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct GovernedPathsConfig {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

#[derive(Debug)]
pub struct GovernedPathsPolicy {
    pub allow: Vec<glob::Pattern>,
    pub deny: Vec<glob::Pattern>,
}

#[derive(Debug, Clone)]
pub enum PolicyVerdict {
    Allow,
    DenyExplicit(String),
    NoMatch,
}

impl GovernedPathsPolicy {
    pub fn load() -> Result<Self> {
        let path = std::env::var("CGREALITY_GOVERNED_PATHS").map_err(|_| {
            CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_ENV_MISSING",
                "CGREALITY_GOVERNED_PATHS is not set",
                "env.CGREALITY_GOVERNED_PATHS",
                "set CGREALITY_GOVERNED_PATHS to the exact governed-paths JSON file",
                json!({}),
                None,
            )
        })?;
        Self::load_from(Path::new(&path))
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_READ_FAILED",
                format!("failed to read governed-paths config: {e}"),
                "config.path",
                "ensure config/ccreality_governed_paths.json exists and is readable",
                json!({"path": path.display().to_string()}),
                Some(format!("file:{}", path.display())),
            )
        })?;
        let value: Value = serde_json::from_str(&raw).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_PARSE_FAILED",
                format!("governed-paths config is not valid JSON: {e}"),
                "config.parse",
                "fix the governed-paths JSON syntax",
                json!({"path": path.display().to_string()}),
                Some(format!("file:{}", path.display())),
            )
        })?;
        Self::from_json(value)
    }

    pub fn from_json(value: Value) -> Result<Self> {
        let cfg: GovernedPathsConfig = serde_json::from_value(value.clone()).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_SHAPE_INVALID",
                format!("governed-paths config shape is invalid: {e}"),
                "config.shape",
                "write JSON as {\"allow\":[...],\"deny\":[...]}",
                json!({"value": value}),
                None,
            )
        })?;
        if cfg.allow.is_empty() {
            return Err(CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_ALLOW_EMPTY",
                "governed-paths allow list is empty",
                "config.allow",
                "add at least one explicit allow pattern",
                json!({}),
                None,
            ));
        }
        let allow = compile_patterns("allow", &cfg.allow)?;
        let deny = compile_patterns("deny", &cfg.deny)?;
        Ok(Self { allow, deny })
    }

    pub fn is_allowed(&self, path: &str) -> PolicyVerdict {
        for pat in &self.deny {
            if pat.matches(path) {
                return PolicyVerdict::DenyExplicit(pat.as_str().to_string());
            }
        }
        for pat in &self.allow {
            if pat.matches(path) {
                return PolicyVerdict::Allow;
            }
        }
        PolicyVerdict::NoMatch
    }
}

fn compile_patterns(field: &str, values: &[String]) -> Result<Vec<glob::Pattern>> {
    values
        .iter()
        .enumerate()
        .map(|(index, pattern)| {
            glob::Pattern::new(pattern).map_err(|e| {
                CCRealityError::new(
                    "CCREALITY_GOVERNED_PATHS_GLOB_INVALID",
                    format!("invalid glob pattern: {e}"),
                    format!("config.{field}[{index}]"),
                    "fix the invalid glob expression",
                    json!({"pattern": pattern}),
                    None,
                )
            })
        })
        .collect()
}

pub fn ensure_path_allowed(path: &str) -> Result<()> {
    if path.trim().is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.contains('\n')
        || std::path::Path::new(path)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(CCRealityError::new(
            "CCREALITY_GOVERNED_PROJECT_PATH_INVALID",
            "project path must be repo-relative without traversal or control characters",
            "arguments.path",
            "use a normalized relative path inside the ContextGraph checkout",
            json!({"path": path}),
            None,
        ));
    }
    let policy = GovernedPathsPolicy::load()?;
    match policy.is_allowed(path) {
        PolicyVerdict::Allow => Ok(()),
        PolicyVerdict::DenyExplicit(rule) => Err(CCRealityError::new(
            "CCREALITY_HARNESS_PATH_DENIED",
            format!("path '{path}' matches deny rule '{rule}'"),
            "arguments.path",
            "choose a path outside protected benchmark, verifier, secret, and reward surfaces",
            json!({"path": path, "rule": rule}),
            None,
        )),
        PolicyVerdict::NoMatch => Err(CCRealityError::new(
            "CCREALITY_HARNESS_PATH_NOT_ALLOWED",
            "path is outside the governed edit allowlist",
            "arguments.path",
            "choose a path explicitly allowed by CGREALITY_GOVERNED_PATHS",
            json!({"path": path}),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_dotenv_and_swebench() {
        let policy_json = serde_json::json!({
            "allow": ["Cargo.toml", "crates/**", "docs/ccreality/**"],
            "deny": [".env", ".env.*", "swebench/**", "tests/oracle_*"]
        });
        let p = GovernedPathsPolicy::from_json(policy_json).expect("valid policy");
        assert!(matches!(
            p.is_allowed(".env"),
            PolicyVerdict::DenyExplicit(_)
        ));
        assert!(matches!(
            p.is_allowed("swebench/eval/runner.py"),
            PolicyVerdict::DenyExplicit(_)
        ));
        assert!(matches!(
            p.is_allowed("crates/context-graph-cli/src/bin/reality-loop.rs"),
            PolicyVerdict::Allow
        ));
        assert!(matches!(
            p.is_allowed("random/path.txt"),
            PolicyVerdict::NoMatch
        ));
    }
}
