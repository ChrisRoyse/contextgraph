use super::errors::{CCRealityError, Result};
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernedPathsConfig {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub title: String,
    pub description: String,
    pub version: u64,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

#[derive(Debug)]
pub struct GovernedPathsPolicy {
    allow: Vec<CompiledPathPattern>,
    deny: Vec<CompiledPathPattern>,
}

#[derive(Debug)]
struct CompiledPathPattern {
    source: String,
    regex: Regex,
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
                "write the exact Phase 5 governed-paths JSON contract",
                json!({"value": value}),
                None,
            )
        })?;
        if !cfg.schema.starts_with("https://json-schema.org/draft/") {
            return Err(CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_SCHEMA_INVALID",
                "governed-paths $schema must identify a JSON Schema draft",
                "config.$schema",
                "use the Phase 5 ccreality_governed_paths.json file",
                json!({"schema": cfg.schema}),
                None,
            ));
        }
        if cfg.title != "ccreality governed paths policy" {
            return Err(CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_TITLE_INVALID",
                "governed-paths title is not the expected policy title",
                "config.title",
                "use the Phase 5 ccreality_governed_paths.json file",
                json!({"title": cfg.title}),
                None,
            ));
        }
        if cfg.description.trim().is_empty() {
            return Err(CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_DESCRIPTION_MISSING",
                "governed-paths description must not be empty",
                "config.description",
                "use the Phase 5 ccreality_governed_paths.json file",
                json!({}),
                None,
            ));
        }
        if cfg.version != 1 {
            return Err(CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_VERSION_UNSUPPORTED",
                "governed-paths config version is unsupported",
                "config.version",
                "use version 1 of the governed paths policy",
                json!({"version": cfg.version, "supported": [1]}),
                None,
            ));
        }
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
        if cfg.deny.is_empty() {
            return Err(CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_DENY_EMPTY",
                "governed-paths deny list is empty",
                "config.deny",
                "add protected deny patterns before using the policy",
                json!({}),
                None,
            ));
        }
        let allow = compile_patterns("allow", &cfg.allow)?;
        let deny = compile_patterns("deny", &cfg.deny)?;
        Ok(Self { allow, deny })
    }

    pub fn is_allowed(&self, path: &str) -> PolicyVerdict {
        for rule in &self.deny {
            if rule.regex.is_match(path) {
                return PolicyVerdict::DenyExplicit(rule.source.clone());
            }
        }
        for rule in &self.allow {
            if rule.regex.is_match(path) {
                return PolicyVerdict::Allow;
            }
        }
        PolicyVerdict::NoMatch
    }
}

fn compile_patterns(field: &str, values: &[String]) -> Result<Vec<CompiledPathPattern>> {
    let mut compiled = Vec::with_capacity(values.len());
    for (index, pattern) in values.iter().enumerate() {
        let regex_source = pattern_to_regex(pattern).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_PATTERN_INVALID",
                format!("invalid path pattern: {e}"),
                format!("config.{field}[{index}]"),
                "fix the invalid path expression in the Phase 5 policy",
                json!({"pattern": pattern}),
                None,
            )
        })?;
        let regex = Regex::new(&regex_source).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_GOVERNED_PATHS_PATTERN_COMPILE_FAILED",
                format!("failed to compile path pattern: {e}"),
                format!("config.{field}[{index}]"),
                "inspect the policy pattern and generated regex",
                json!({"pattern": pattern, "regex": regex_source}),
                None,
            )
        })?;
        compiled.push(CompiledPathPattern {
            source: pattern.clone(),
            regex,
        });
    }
    Ok(compiled)
}

fn pattern_to_regex(pattern: &str) -> std::result::Result<String, &'static str> {
    if pattern.is_empty() {
        return Err("pattern must not be empty");
    }
    if pattern.contains('\\') || pattern.contains('\0') || pattern.contains('\n') {
        return Err("pattern must use forward slashes and contain no control characters");
    }

    let mut regex = String::with_capacity(pattern.len() * 2 + 2);
    regex.push('^');
    let mut i = 0;
    while i < pattern.len() {
        let rest = &pattern[i..];
        if rest.starts_with("**/") {
            regex.push_str("(?:.*/)?");
            i += 3;
        } else if rest.starts_with("/**/") {
            regex.push_str("/(?:.*/)?");
            i += 4;
        } else if rest == "/**" {
            regex.push_str("/.*");
            i += 3;
        } else if rest.starts_with("**") {
            regex.push_str(".*");
            i += 2;
        } else {
            let ch = rest.chars().next().ok_or("unexpected empty pattern tail")?;
            match ch {
                '*' => regex.push_str("[^/]*"),
                '?' => regex.push_str("[^/]"),
                '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' => {
                    regex.push('\\');
                    regex.push(ch);
                }
                '/' | '_' | '-' => regex.push(ch),
                c if c.is_ascii_alphanumeric() => regex.push(c),
                _ => return Err("pattern contains an unsupported character"),
            }
            i += ch.len_utf8();
        }
    }
    regex.push('$');
    Ok(regex)
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
    fn real_phase5_policy_denies_protected_paths() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let policy_path = root.join("config/ccreality_governed_paths.json");
        let p = GovernedPathsPolicy::load_from(&policy_path).expect("valid Phase 5 policy");
        assert!(matches!(
            p.is_allowed(".env"),
            PolicyVerdict::DenyExplicit(_)
        ));
        assert!(matches!(
            p.is_allowed("swebench/eval/runner.py"),
            PolicyVerdict::DenyExplicit(_)
        ));
        assert!(matches!(
            p.is_allowed("tests/oracle_case/fixture.json"),
            PolicyVerdict::DenyExplicit(_)
        ));
        assert!(matches!(
            p.is_allowed("crates/context-graph-benchmark/datasets/swe_bench_lite/data.json"),
            PolicyVerdict::DenyExplicit(_)
        ));
        assert!(matches!(
            p.is_allowed("crates/context-graph-cli/src/bin/reality-loop.rs"),
            PolicyVerdict::Allow
        ));
        assert!(matches!(
            p.is_allowed("config/ccreality_governed_paths.json"),
            PolicyVerdict::Allow
        ));
        assert!(matches!(
            p.is_allowed("random/path.txt"),
            PolicyVerdict::NoMatch
        ));
    }

    #[test]
    fn real_phase5_policy_rejects_unsupported_version() {
        let value = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "ccreality governed paths policy",
            "description": "test",
            "version": 999,
            "allow": ["docs/ccreality/**"],
            "deny": [".env"]
        });
        let err = GovernedPathsPolicy::from_json(value).expect_err("version must be rejected");
        assert_eq!(
            err.error_code,
            "CCREALITY_GOVERNED_PATHS_VERSION_UNSUPPORTED"
        );
    }

    #[test]
    fn ccreality_pattern_compiler_matches_documented_globstar_dialect() {
        let regex = Regex::new(&pattern_to_regex("tests/oracle_**/**").expect("regex source"))
            .expect("compiled");
        assert!(regex.is_match("tests/oracle_case/fixture.json"));
        assert!(regex.is_match("tests/oracle_large/nested/fixture.json"));

        let root_secret =
            Regex::new(&pattern_to_regex("**/credentials.json").expect("regex source"))
                .expect("compiled");
        assert!(root_secret.is_match("credentials.json"));
        assert!(root_secret.is_match("service/credentials.json"));

        let single_segment =
            Regex::new(&pattern_to_regex("*.pem").expect("regex source")).expect("compiled");
        assert!(single_segment.is_match("secret.pem"));
        assert!(!single_segment.is_match("nested/secret.pem"));
    }
}
