# Phase 1 — File Specifications

## `crates/context-graph-cli/Cargo.toml` `[lib]` section

If the manifest does not already declare a library target, add this section:

```toml
[lib]
name = "context_graph_cli"
path = "src/lib.rs"
```

Place it after `[package]` and before any `[[bin]]` entries. Do NOT change any existing `[[bin]]` entries — `reality-loop`, `blackbox`, `context-graph` continue to be binary targets.

## `crates/context-graph-cli/src/lib.rs`

If file does not exist, create with this exact content:

```rust
//! context-graph-cli library crate
//!
//! Houses shared modules used by both the binary entry points
//! (`reality-loop`, `blackbox`, `context-graph`) and external
//! consumers (notably the `context-graph-mcp` crate's
//! `reality_loop` MCP module).

pub mod governed_edit;
```

If file exists, prepend `pub mod governed_edit;` to it (only if not present). Do not remove or reorder existing items.

## `crates/context-graph-cli/src/governed_edit.rs` skeleton

Initial content (functions populated in Task 1.4 by lifting from `reality-loop.rs`):

```rust
//! Governed-edit module — provider-neutral helpers for line-window file edits,
//! path-policy enforcement, command allowlisting, and project-state verification.
//!
//! Lifted out of `bin/reality-loop.rs` during Phase 1 of the ccreality cutover
//! (see docs/ccreality/phase-01-excision/). Used by:
//! - `bin/reality-loop.rs` for the inner Gemma agent's apply_line_window_edit tool
//! - `crates/context-graph-mcp/src/handlers/tools/reality_loop/` (Phase 4) for
//!   the MCP harness_apply_line_window_edit tool

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

// ----- public API -----

#[derive(Debug)]
pub struct GovernedEditError {
    pub error_code: String,
    pub message: String,
    pub field_path: String,
    pub remediation: String,
    pub details: Value,
    pub source_of_truth: Option<String>,
}

pub type Result<T> = std::result::Result<T, GovernedEditError>;

/// Resolve project root from current working directory; fail if Cargo.toml absent.
pub fn project_root() -> Result<PathBuf> { unimplemented!() }

/// Validate a project-relative path against the governed-paths policy.
/// Reads the policy from $CGREALITY_GOVERNED_PATHS and fails closed if the
/// variable, file, or JSON policy is missing.
pub fn validate_governed_path(path: &str) -> Result<String> { unimplemented!() }

/// Open a window of a project file. Returns SHA-256 + window text + total line count.
pub fn project_file_window(
    project_root: &Path,
    relative_path: &str,
    start_line: u64,
    end_line: u64,
) -> Result<Value> { unimplemented!() }

/// Apply a line-window edit batch with sha256 readback verification.
/// Each edit must include observed_sha256 from a prior project_file_window call.
pub fn project_apply_line_window_edits(
    project_root: &Path,
    tool_dir: &Path,
    edits: &[Value],
    timeout_secs: u64,
) -> Result<Value> { unimplemented!() }

/// Run an allowlisted command in the project root.
/// Allowlist: cargo {fmt,check,test,build,clippy}, rustfmt, git {status,diff,log,show},
/// ollama ps, nvidia-smi. Denies shell metachars and chained commands.
pub fn run_governed_command(
    project_root: &Path,
    tool_dir: &Path,
    command: &str,
    timeout_secs: u64,
) -> Result<Value> { unimplemented!() }

/// Verify project state: rustfmt --check, cargo check, cargo test (focused),
/// git status. Returns structured pass/fail per check.
pub fn verify_project_state(
    project_root: &Path,
    tool_dir: &Path,
    label: &str,
    scope: VerifyScope,
) -> Result<Value> { unimplemented!() }

#[derive(Debug, Clone, Copy)]
pub enum VerifyScope {
    /// Limit to reality-loop binary only — fast.
    RealityLoopOnly,
    /// Full workspace — slow but thorough.
    Full,
}

// ----- internal helpers -----

fn governed_tool_blocked(name: &str, code: &str, field: &str, msg: &str, details: Value) -> Value {
    json!({
        "status": "blocked",
        "tool": name,
        "error_code": code,
        "field_path": field,
        "message": msg,
        "details": details
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    // tests moved here in Task 1.5
}
```

When populating `unimplemented!()` bodies in Task 1.4, lift the body from the corresponding function in `reality-loop.rs`, with these mechanical replacements:

| Old | New |
|-----|-----|
| `RealityError::new(...)` | `GovernedEditError { ... }` (with same fields) |
| `Result<T>` (alias for `RealityError` Result) | local `Result<T>` (alias for `GovernedEditError` Result) |
| `RUST_REALITY_OPENAI_PROJECT_*` | `RUST_REALITY_GOVERNED_PROJECT_*` |
| `openai_optimizer_tool_blocked(...)` | `governed_tool_blocked(...)` |
| `openai_optimizer.project_edits[...]` | `governed_edits[...]` (in field_path strings) |

Imports needed at the top of `governed_edit.rs` (some may need to be moved from `reality-loop.rs`):
- `crate::sha256_text` and `crate::sha256_file` — these are currently file-local; either also lift them to `governed_edit.rs` (recommended; they're trivial) or expose them via `lib.rs` so this module can import them. Recommended: copy `sha256_text` and `sha256_file` definitions into `governed_edit.rs` as private helpers (they're small enough that duplication is fine and the module becomes self-contained).
- `crate::write_text_checked`, `crate::write_json_checked`, `crate::create_dir_checked`, `crate::fs_error`, `crate::path_str`, `crate::file_sot`, `crate::run_required` — same approach; either expose via lib.rs or copy as private. Recommended: expose these via lib.rs because they're used widely.

The cleanest approach: in Task 1.2 (lib.rs creation), also re-export the file-system helpers as a separate small module:

```rust
// lib.rs
pub mod governed_edit;
pub mod fs_helpers;  // contains sha256_text, sha256_file, write_*, create_dir_checked, etc.
```

Move the helpers from `reality-loop.rs` into `fs_helpers.rs`. The bin then `use context_graph_cli::fs_helpers::*;` — minimal disruption.

This is a judgment call during execution. Either way, the verification just checks that builds and tests pass.

## Final shape of `crates/context-graph-cli/src/bin/reality-loop.rs`

After all 18 tasks:
- Approximately 6,500–7,500 lines.
- No `deepseek|openai|gpt-5|chatgpt` substring (case-insensitive).
- `struct Args` has ~25 fields (Gemma/Ollama/SWE-bench/GPU only).
- `struct AttemptRecord` has ~17 fields (none mentioning openai/optimizer).
- `run()` is shorter — the trigger+optimizer block is gone.
- All preserved tests still compile and pass.
- Imports `use context_graph_cli::governed_edit;` (or the equivalent) when the inner Gemma `tool_apply_line_window_edit` needs the lifted helpers (it does — see line 6204 in the original, which uses `compile_line_window_edits`; note that `compile_line_window_edits` is different from `project_apply_line_window_edits` and STAYS in `reality-loop.rs` because it operates on candidate-patch artifacts, not on project source files).

The KEEP/DELETE distinction is sharp:
- **KEEP** (in `reality-loop.rs`): `compile_line_window_edits`, `compile_structured_edits` — these compile candidate patches from Gemma's tool calls. They write to attempt-N's candidate-patch.patch, not to the project tree.
- **LIFT** (to `governed_edit.rs`): `project_apply_line_window_edits` — this writes to the project tree (e.g. `crates/context-graph-cli/src/bin/reality-loop.rs` itself). Different semantics, different policy (path allowlist), different artifact (git diff, cargo check).

## What `attempt-summary.json` looks like after Task 1.11

```json
{
  "attempt": { ... AttemptRecord with 17 fields, no openai_* ... },
  "model_response": { ... },
  "blackbox_verifier": { ... },
  "reality_signal_verifier": { ... },
  "score": { ... },
  "reward": { ... },
  "ledger_counts": { ... }
}
```

(Compared to before, missing `deepseek_optimizer_trigger_decision` and `openai_loop_optimizer` keys.)

## What `failure-{unix}-{code}.json` looks like after Task 1.10

```json
{
  "record_kind": "rust_reality_loop_failure",
  "created_at_unix": 1714896000,
  "args": {
    "model": "gemma4:e4b",
    "task_manifest": "...",
    "runtime_root": "...",
    "max_attempts": 1,
    "run_until_resolved": false,
    "smoke_test": true,
    "official_eval_each_attempt": true,
    "official_docker_host": "...",
    "official_docker_config": "...",
    "gpu_index": 0,
    "gpu_min_free_mb": 8192,
    "gpu_max_used_mb": null,
    "ollama_keep_alive": "30m",
    "ollama_num_ctx": 32768,
    "ollama_num_predict": 4096,
    "ollama_temperature": 0.0,
    "ollama_top_p": 0.9
  },
  "error": { ... RealityError ... },
  "source_of_truth": "file:..."
}
```

(Compared to before: 11 openai_/optimizer_ keys removed from `args`.)
