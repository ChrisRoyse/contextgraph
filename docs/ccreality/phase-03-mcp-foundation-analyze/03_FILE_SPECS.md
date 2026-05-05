# Phase 3 — File Specifications

Each file's complete or skeletal content. Adapt to the existing rmcp/context-graph-mcp conventions discovered in Task 3.1.

## `crates/context-graph-mcp/src/handlers/tools/reality_loop/mod.rs`

```rust
//! cgreality MCP tool family
//!
//! Reality-loop integration: ANALYZE (this phase), INTERACT + ALTER (Phase 4).
//!
//! Spec: docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md
//! Plan: docs/ccreality/phase-03-mcp-foundation-analyze/

pub mod analyze;
pub mod errors;
pub mod helpers;
pub mod path_policy;

// Phase 4 will add:
// pub mod alter;
// pub mod interact;
// pub mod shift_log;
// pub mod optimizer;

pub use errors::{CCRealityError, Result};

/// Register all cgreality tools with the MCP server.
/// Called by main.rs when --mode reality-loop is set.
pub fn register(server: &mut crate::server::McpServer) {
    // ANALYZE family — read-only artifact access (Phase 3)
    analyze::register(server);
    // ALTER family — Phase 4
    // alter::register(server);
    // INTERACT family — Phase 4
    // interact::register(server);
    // SHIFT-LOG family — Phase 4
    // shift_log::register(server);
    // OPTIMIZER family — Phase 4
    // optimizer::register(server);
}
```

(Adjust `crate::server::McpServer` to whatever the existing crate exposes.)

## `crates/context-graph-mcp/src/handlers/tools/reality_loop/errors.rs`

```rust
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct CCRealityError {
    pub status: &'static str,
    pub error_code: String,
    pub message: String,
    pub field_path: String,
    pub remediation: String,
    pub details: Value,
    pub source_of_truth: Option<String>,
}

impl CCRealityError {
    pub fn new(
        error_code: impl Into<String>,
        message: impl Into<String>,
        field_path: impl Into<String>,
        remediation: impl Into<String>,
        details: Value,
        source_of_truth: Option<String>,
    ) -> Self {
        Self {
            status: "error",
            error_code: error_code.into(),
            message: message.into(),
            field_path: field_path.into(),
            remediation: remediation.into(),
            details,
            source_of_truth,
        }
    }

    pub fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({"status": "error", "error_code": "CCREALITY_ERROR_SERIALIZATION_FAILED"}))
    }
}

impl std::fmt::Display for CCRealityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} ({})", self.error_code, self.message, self.field_path)
    }
}

impl std::error::Error for CCRealityError {}

pub type Result<T> = std::result::Result<T, CCRealityError>;
```

## `crates/context-graph-mcp/src/handlers/tools/reality_loop/helpers.rs`

```rust
use super::errors::{CCRealityError, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub async fn read_active_runtime_root() -> Result<Option<PathBuf>> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(None),
    };
    let path = home.join(".cache/cgreality/active_runtime_root");
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| CCRealityError::new(
        "CCREALITY_ACTIVE_ROOT_READ_FAILED",
        format!("failed to read active runtime root: {e}"),
        "active_runtime_root.path",
        "ensure ~/.cache/cgreality/ is readable",
        serde_json::json!({"path": path.display().to_string()}),
        Some(format!("file:{}", path.display())),
    ))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(trimmed)))
}

pub async fn read_active_target_instance() -> Result<Option<String>> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(None),
    };
    let path = home.join(".cache/cgreality/active_target_instance");
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(&path).map_err(|e| CCRealityError::new(
        "CCREALITY_ACTIVE_INSTANCE_READ_FAILED",
        format!("failed to read active target instance: {e}"),
        "active_target_instance.path",
        "ensure ~/.cache/cgreality/ is readable",
        serde_json::json!({"path": path.display().to_string()}),
        Some(format!("file:{}", path.display())),
    ))?.trim().to_string()))
}

pub fn latest_run_id(runtime_root: &Path) -> Option<String> {
    let entries: Vec<_> = std::fs::read_dir(runtime_root).ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.starts_with("run-"))
        .collect();
    let mut sorted = entries;
    sorted.sort();
    sorted.into_iter().last()
}

pub fn latest_attempt_number(runtime_root: &Path, target_instance: Option<&str>) -> Option<u64> {
    let run_id = latest_run_id(runtime_root)?;
    let task = target_instance?;
    let task_dir = runtime_root.join(run_id).join(task);
    let attempts: Vec<u64> = std::fs::read_dir(&task_dir).ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|name| name.strip_prefix("attempt-").and_then(|s| s.parse().ok()))
        .collect();
    attempts.into_iter().max()
}

pub fn attempt_dir(runtime_root: &Path, run_id: &str, task_id: &str, attempt: u64) -> PathBuf {
    runtime_root.join(run_id).join(task_id).join(format!("attempt-{}", attempt))
}

pub fn read_json(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path).map_err(|e| CCRealityError::new(
        "CCREALITY_ARTIFACT_READ_FAILED",
        format!("failed to read artifact at {}: {e}", path.display()),
        "artifact.path",
        "verify the artifact path is correct and readable",
        serde_json::json!({"path": path.display().to_string()}),
        Some(format!("file:{}", path.display())),
    ))?;
    serde_json::from_str(&raw).map_err(|e| CCRealityError::new(
        "CCREALITY_ARTIFACT_PARSE_FAILED",
        format!("artifact at {} is not valid JSON: {e}", path.display()),
        "artifact.parse",
        "inspect the artifact for corruption",
        serde_json::json!({"path": path.display().to_string()}),
        Some(format!("file:{}", path.display())),
    ))
}

pub fn file_sot(path: &Path) -> String { format!("file:{}", path.display()) }

pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Sha256, Digest};
    let bytes = fs::read(path).map_err(|e| CCRealityError::new(
        "CCREALITY_SHA256_READ_FAILED",
        format!("failed to read file for SHA-256: {e}"),
        "sha256.read",
        "ensure file is readable",
        serde_json::json!({"path": path.display().to_string()}),
        Some(format!("file:{}", path.display())),
    ))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}
```

(Add `sha2` to Cargo.toml if missing.)

## `crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs`

```rust
use super::errors::{CCRealityError, Result};
use serde::Deserialize;
use std::path::PathBuf;

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
        // 1. Try $CGREALITY_GOVERNED_PATHS env var
        if let Ok(path) = std::env::var("CGREALITY_GOVERNED_PATHS") {
            return Self::load_from(&PathBuf::from(path));
        }
        // 2. Try $CONTEXTGRAPH_ROOT/config/ccreality_governed_paths.json
        if let Ok(root) = std::env::var("CONTEXTGRAPH_ROOT") {
            let candidate = PathBuf::from(root).join("config/ccreality_governed_paths.json");
            if candidate.is_file() {
                return Self::load_from(&candidate);
            }
        }
        // 3. Fallback: hard-coded conservative
        tracing::warn!("CGREALITY_GOVERNED_PATHS unset; using hard-coded conservative policy");
        Ok(Self::default_conservative())
    }

    pub fn load_from(path: &PathBuf) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| CCRealityError::new(
            "CCREALITY_GOVERNED_PATHS_READ_FAILED",
            format!("failed to read governed-paths config: {e}"),
            "config.path",
            "ensure config/ccreality_governed_paths.json exists or set CGREALITY_GOVERNED_PATHS",
            serde_json::json!({"path": path.display().to_string()}),
            Some(format!("file:{}", path.display())),
        ))?;
        let cfg: GovernedPathsConfig = serde_json::from_str(&raw).map_err(|e| CCRealityError::new(
            "CCREALITY_GOVERNED_PATHS_PARSE_FAILED",
            format!("invalid governed-paths config: {e}"),
            "config.parse",
            "verify JSON syntax",
            serde_json::json!({"path": path.display().to_string()}),
            Some(format!("file:{}", path.display())),
        ))?;
        Ok(Self {
            allow: cfg.allow.iter().map(|s| glob::Pattern::new(s).unwrap()).collect(),
            deny:  cfg.deny.iter().map(|s| glob::Pattern::new(s).unwrap()).collect(),
        })
    }

    pub fn default_conservative() -> Self {
        Self {
            allow: vec!["crates/context-graph-cli/src/**", "scripts/cc/**", "docs/ccreality/**"]
                .into_iter().map(|s| glob::Pattern::new(s).unwrap()).collect(),
            deny: vec![
                ".env", ".env.*", "**/.env",
                "swebench/**", "tests/oracle_**",
                "crates/context-graph-benchmark/datasets/swe_bench_*",
                "crates/*/src/reward_specs/**",
                "crates/*/src/verifiers/**",
            ].into_iter().map(|s| glob::Pattern::new(s).unwrap()).collect(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deny_dotenv_and_swebench() {
        let p = GovernedPathsPolicy::default_conservative();
        assert!(matches!(p.is_allowed(".env"), PolicyVerdict::DenyExplicit(_)));
        assert!(matches!(p.is_allowed("swebench/eval/runner.py"), PolicyVerdict::DenyExplicit(_)));
        assert!(matches!(p.is_allowed("crates/context-graph-cli/src/bin/reality-loop.rs"), PolicyVerdict::Allow));
        assert!(matches!(p.is_allowed("random/path.txt"), PolicyVerdict::NoMatch));
    }
}
```

(Add `glob = "0.3"` to Cargo.toml if missing.)

## `crates/context-graph-mcp/src/handlers/tools/reality_loop/analyze.rs`

This file is large. It contains 11 tool implementations. Each follows the same pattern:

```rust
use super::errors::{CCRealityError, Result};
use super::helpers::*;
use serde_json::{json, Value};

pub async fn reality_latest_root(_args: Value) -> Result<Value> {
    let runtime_root = read_active_runtime_root().await?;
    let target = read_active_target_instance().await?;
    let active_run_id = runtime_root.as_ref().and_then(|r| latest_run_id(r));
    let attempt_n = runtime_root.as_ref().and_then(|r| latest_attempt_number(r, target.as_deref()));
    Ok(json!({
        "runtime_root": runtime_root.as_ref().map(|p| p.display().to_string()),
        "active_run_id": active_run_id,
        "active_task_id": target,
        "active_target_instance": target,
        "success_predicate": "official SWE-bench report places target instance in resolved_ids",
        "last_attempt_number": attempt_n,
        "source_of_truth": runtime_root.as_ref().map(|p| file_sot(p))
    }))
}

pub async fn reality_attempt_summary(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let path = attempt_dir(&runtime_root, &run_id, &task, attempt).join("attempt-summary.json");
    let body: Value = read_json(&path)?;
    let mut response = body;
    if let Value::Object(ref mut map) = response {
        map.insert("source_of_truth".to_string(), Value::String(file_sot(&path)));
    }
    Ok(response)
}

// ... 9 more tools with the same shape ...
```

For each remaining tool (`reality_official_report`, `reality_problem_packet`, `reality_signal`, `reality_failure`, `reality_trigger_decision`, `reality_harness_transitions`, `reality_compare_attempts`, `reality_audit_trail`, `reality_replay_artifact`), the implementation is mechanical:

1. Parse args (via `required_str`, `required_u64`, `optional_bool` helpers).
2. Resolve attempt directory or other on-disk path.
3. Read JSON.
4. Optionally project (compact form).
5. Add `source_of_truth`.
6. Return.

Helper functions for arg parsing:

```rust
fn required_str(args: &Value, field: &str) -> Result<String> {
    args.get(field).and_then(Value::as_str).map(str::to_string).ok_or_else(|| CCRealityError::new(
        "CCREALITY_ARG_MISSING_OR_NOT_STRING",
        format!("argument '{}' must be a string", field),
        format!("arguments.{}", field),
        format!("provide a string for arguments.{}", field),
        json!({"args": args}),
        None,
    ))
}

fn required_u64(args: &Value, field: &str) -> Result<u64> {
    args.get(field).and_then(Value::as_u64).ok_or_else(|| CCRealityError::new(
        "CCREALITY_ARG_MISSING_OR_NOT_U64",
        format!("argument '{}' must be a non-negative integer", field),
        format!("arguments.{}", field),
        format!("provide a non-negative integer for arguments.{}", field),
        json!({"args": args}),
        None,
    ))
}

fn optional_bool(args: &Value, field: &str, default: bool) -> bool {
    args.get(field).and_then(Value::as_bool).unwrap_or(default)
}

async fn require_active_runtime_root() -> Result<std::path::PathBuf> {
    read_active_runtime_root().await?.ok_or_else(|| CCRealityError::new(
        "CCREALITY_NO_ACTIVE_RUNTIME_ROOT",
        "no active runtime root; call reality_run_smoke or reality_run_attempt first",
        "active_runtime_root",
        "trigger a run to populate ~/.cache/cgreality/active_runtime_root",
        json!({}),
        None,
    ))
}

async fn require_active_target_instance() -> Result<String> {
    read_active_target_instance().await?.ok_or_else(|| CCRealityError::new(
        "CCREALITY_NO_ACTIVE_TARGET_INSTANCE",
        "no active target instance; the engine must record one on attempt/smoke start",
        "active_target_instance",
        "ensure ~/.cache/cgreality/active_target_instance is written",
        json!({}),
        None,
    ))
}

pub fn register(server: &mut crate::server::McpServer) {
    // Adapt the right-hand side to the existing tool-registration pattern in
    // crates/context-graph-mcp/src/handlers/tools/.
    server.register_tool("reality_latest_root", input_schema_for_no_args(), reality_latest_root);
    server.register_tool("reality_attempt_summary", input_schema_run_attempt(), reality_attempt_summary);
    server.register_tool("reality_official_report", input_schema_run_attempt(), reality_official_report);
    server.register_tool("reality_problem_packet", input_schema_run_compact(), reality_problem_packet);
    server.register_tool("reality_signal", input_schema_run_attempt_compact(), reality_signal);
    server.register_tool("reality_failure", input_schema_run_attempt(), reality_failure);
    server.register_tool("reality_trigger_decision", input_schema_run_attempt(), reality_trigger_decision);
    server.register_tool("reality_harness_transitions", input_schema_run_attempt_round(), reality_harness_transitions);
    server.register_tool("reality_compare_attempts", input_schema_compare(), reality_compare_attempts);
    server.register_tool("reality_audit_trail", input_schema_audit_trail(), reality_audit_trail);
    server.register_tool("reality_replay_artifact", input_schema_replay_artifact(), reality_replay_artifact);
}
```

(The exact `register_tool` signature depends on the existing crate's API.)

## `main.rs` additions

In `crates/context-graph-mcp/src/main.rs`, add:

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "context-graph-mcp")]
#[command(about = "ContextGraph Model Context Protocol server")]
struct Cli {
    /// MCP server mode. `default` registers the existing tool families.
    /// `reality-loop` adds the cgreality tool family on top.
    #[arg(long, default_value = "default")]
    mode: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let mut server = build_default_server();  // existing function or inline
    if cli.mode == "reality-loop" {
        crate::handlers::tools::reality_loop::register(&mut server);
    }
    server.run_stdio().await
}
```

(Adapt to the actual existing main.rs structure.)

## Cargo.toml additions

Add to `crates/context-graph-mcp/Cargo.toml` `[dependencies]` if missing:

```toml
glob = "0.3"
sha2 = "0.10"
dirs = "5"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

`clap`, `serde`, `serde_json`, `tracing`, `tokio`, `anyhow` are presumably already there.

Verify with `cargo build`.
