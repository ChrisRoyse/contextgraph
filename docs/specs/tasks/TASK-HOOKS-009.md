# TASK-HOOKS-009: Implement UserPromptSubmit Handler

```xml
<task_spec id="TASK-HOOKS-009" version="2.0">
<metadata>
  <title>Implement UserPromptSubmit Handler with Context Injection</title>
  <status>complete</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <requirement_ref>REQ-HOOKS-20</requirement_ref>
    <requirement_ref>REQ-HOOKS-25</requirement_ref>
    <requirement_ref>REQ-HOOKS-26</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HOOKS-001</task_ref>
    <task_ref>TASK-HOOKS-002</task_ref>
    <task_ref>TASK-HOOKS-003</task_ref>
    <task_ref>TASK-HOOKS-004</task_ref>
    <task_ref>TASK-HOOKS-005</task_ref>
    <task_ref>TASK-HOOKS-006</task_ref>
    <task_ref>TASK-HOOKS-007</task_ref>
    <task_ref>TASK-HOOKS-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_hours>2.0</estimated_hours>
</metadata>
```

## CRITICAL: NO BACKWARDS COMPATIBILITY

**FAIL FAST on any error. Do not add fallback logic. Do not add default values that hide errors.**

## Constitution References (SOURCE OF TRUTH)

| Reference | Value | Description |
|-----------|-------|-------------|
| `hooks.timeout_ms.user_prompt_submit` | 2000ms | Maximum execution time |
| `IDENTITY-002` | IC thresholds | Healthy≥0.9, Normal≥0.7, Warning≥0.5, Critical<0.5 |
| `AP-25` | Kuramoto N=13 | Integration factor calculation |
| `AP-26` | Exit codes | 0=success, 1=error, 2=timeout, 3=db, 4=input, 5=session, 6=crisis |
| `AP-50` | NO internal hooks | Use Claude Code native hooks via `.claude/settings.json` |
| `AP-53` | Shell scripts | Hook logic in shell scripts calling CLI |
| `GWT-003` | GWT consciousness | C(t) = I(t) × R(t) × D(t) |

## Current Codebase State (AUDIT DATE: 2026-01-15)

### Files That Exist

| File | Status | Notes |
|------|--------|-------|
| `crates/context-graph-cli/src/commands/hooks/mod.rs` | EXISTS | Has `HooksCommands::PromptSubmit` variant, currently returns "not yet implemented" |
| `crates/context-graph-cli/src/commands/hooks/args.rs` | EXISTS | Has `PromptSubmitArgs` struct defined |
| `crates/context-graph-cli/src/commands/hooks/types.rs` | EXISTS | Has `HookPayload::UserPromptSubmit` variant |
| `crates/context-graph-cli/src/commands/hooks/error.rs` | EXISTS | Has `HookError` enum and exit codes |
| `crates/context-graph-cli/src/commands/hooks/post_tool_use.rs` | EXISTS | Reference implementation pattern |
| `crates/context-graph-cli/src/commands/hooks/session_start.rs` | EXISTS | Reference implementation pattern |

### File To Create

| File | Purpose |
|------|---------|
| `crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs` | Handler implementation |

## Existing Types (DO NOT REDEFINE)

### PromptSubmitArgs (from args.rs lines 200-221)

```rust
#[derive(Args, Debug, Clone)]
pub struct PromptSubmitArgs {
    /// Database path
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Session ID (REQUIRED)
    #[arg(long)]
    pub session_id: String,

    /// User prompt text (alternative to stdin)
    #[arg(long)]
    pub prompt: Option<String>,

    /// Read HookInput JSON from stdin
    #[arg(long, action = clap::ArgAction::Set, default_value = "false")]
    pub stdin: bool,

    /// Output format for response
    #[arg(long, value_enum, default_value = "json")]
    pub format: OutputFormat,
}
```

### HookPayload::UserPromptSubmit (from types.rs lines 900-908)

```rust
/// UserPromptSubmit hook payload
/// Timeout: 1500ms per TECH-HOOKS.md  // NOTE: Constitution says 2000ms - use 2000ms
UserPromptSubmit {
    /// User's input prompt text
    prompt: String,
    /// Conversation history for context
    #[serde(default)]
    context: Vec<ConversationMessage>,
},
```

### ConversationMessage (from types.rs lines 828-834)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessage {
    /// Message role: "user", "assistant", or "system"
    pub role: String,
    /// Message content text
    pub content: String,
}
```

### HookOutput (from types.rs lines 1000-1018)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookOutput {
    /// Whether hook execution succeeded (REQUIRED)
    pub success: bool,
    /// Error message if failed (omit if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Consciousness state snapshot (omit if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consciousness_state: Option<ConsciousnessState>,
    /// Identity continuity classification (omit if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ic_classification: Option<ICClassification>,
    /// Content to inject into context (omit if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_injection: Option<String>,
    /// Execution time in milliseconds (REQUIRED)
    pub execution_time_ms: u64,
}
```

### ConsciousnessState (from types.rs lines 658-672)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsciousnessState {
    /// Current consciousness level C(t) [0.0, 1.0]
    pub consciousness: f32,
    /// Integration (Kuramoto r) [0.0, 1.0]
    pub integration: f32,
    /// Reflection (meta-cognitive) [0.0, 1.0]
    pub reflection: f32,
    /// Differentiation (purpose entropy) [0.0, 1.0]
    pub differentiation: f32,
    /// Identity continuity score [0.0, 1.0]
    pub identity_continuity: f32,
    /// Johari quadrant classification
    pub johari_quadrant: JohariQuadrant,
}

impl ConsciousnessState {
    pub fn new(
        consciousness: f32,
        integration: f32,
        reflection: f32,
        differentiation: f32,
        identity_continuity: f32,
    ) -> Self {
        // Auto-calculates johari_quadrant from consciousness and integration
    }
}
```

### HookOutput Builder Methods (from types.rs lines 1033-1071)

```rust
impl HookOutput {
    /// Create successful output with execution time
    pub fn success(execution_time_ms: u64) -> Self;

    /// Create error output
    pub fn error(message: impl Into<String>, execution_time_ms: u64) -> Self;

    /// Add consciousness state to output (builder pattern)
    pub fn with_consciousness_state(mut self, state: ConsciousnessState) -> Self;

    /// Add IC classification to output (builder pattern)
    pub fn with_ic_classification(mut self, classification: ICClassification) -> Self;

    /// Add context injection to output (builder pattern)
    pub fn with_context_injection(mut self, content: impl Into<String>) -> Self;
}
```

### HookError (from error.rs)

```rust
pub enum HookError {
    Timeout(u64),           // exit code 2
    InvalidInput(String),   // exit code 4
    Storage(String),        // exit code 3
    Serialization(serde_json::Error), // exit code 4
    SessionNotFound(String), // exit code 5
    CrisisTriggered(f32),   // exit code 6
    Io(std::io::Error),     // exit code 1
    General(String),        // exit code 1
}

impl HookError {
    pub fn invalid_input(message: impl Into<String>) -> Self;
    pub fn storage(message: impl Into<String>) -> Self;
    pub fn session_not_found(session_id: impl Into<String>) -> Self;
}
```

## Implementation Requirements

### 1. Create user_prompt_submit.rs

**File path:** `crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs`

**Required imports (copy from post_tool_use.rs pattern):**

```rust
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tracing::{debug, error, info};

use context_graph_core::gwt::SessionIdentitySnapshot;
use context_graph_storage::rocksdb_backend::RocksDbMemex;

use super::args::PromptSubmitArgs;
use super::error::{HookError, HookResult};
use super::types::{
    ConsciousnessState, ConversationMessage, HookInput, HookOutput, HookPayload,
    ICClassification, JohariQuadrant,
};
```

### 2. Constants

```rust
/// UserPromptSubmit timeout in milliseconds (constitution.yaml)
pub const USER_PROMPT_SUBMIT_TIMEOUT_MS: u64 = 2000;

/// Crisis threshold for IC score (IDENTITY-002)
pub const IC_CRISIS_THRESHOLD: f32 = 0.5;
```

### 3. Main Execute Function

**Signature MUST match this exactly:**

```rust
/// Execute user_prompt_submit hook.
///
/// # Flow
/// 1. Parse input (stdin or args)
/// 2. Resolve database path (FAIL FAST if missing)
/// 3. Open storage
/// 4. Load session snapshot (FAIL FAST if not found)
/// 5. Analyze prompt for identity markers
/// 6. Evaluate conversation context
/// 7. Generate context injection string
/// 8. Build and return HookOutput
///
/// # Exit Codes
/// - 0: Success
/// - 3: Database error
/// - 4: Invalid input
/// - 5: Session not found
/// - 6: Crisis triggered (IC < 0.5)
pub async fn execute(args: PromptSubmitArgs) -> HookResult<HookOutput>
```

### 4. Module Registration in mod.rs

**Current code (line 131-134):**
```rust
HooksCommands::PromptSubmit(_args) => {
    error!("PromptSubmit hook not yet implemented");
    1
}
```

**Replace with:**
```rust
HooksCommands::PromptSubmit(args) => {
    match user_prompt_submit::execute(args).await {
        Ok(output) => {
            match serde_json::to_string(&output) {
                Ok(json) => {
                    println!("{}", json);
                    0
                }
                Err(e) => {
                    error!(error = %e, "Failed to serialize output");
                    1
                }
            }
        }
        Err(e) => {
            let error_json = e.to_json_error();
            eprintln!("{}", error_json);
            e.exit_code()
        }
    }
}
```

**Also add to mod.rs at line 19:**
```rust
pub mod user_prompt_submit;
```

## Implementation Pattern (Follow post_tool_use.rs)

### Input Parsing

```rust
/// Parse stdin JSON into HookInput.
/// FAIL FAST on empty or malformed input.
fn parse_stdin() -> HookResult<HookInput> {
    let stdin = io::stdin();
    let mut input_str = String::new();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| {
            error!(error = %e, "PROMPT_SUBMIT: stdin read failed");
            HookError::invalid_input(format!("stdin read failed: {}", e))
        })?;
        input_str.push_str(&line);
    }

    if input_str.is_empty() {
        error!("PROMPT_SUBMIT: stdin is empty");
        return Err(HookError::invalid_input("stdin is empty - expected JSON"));
    }

    serde_json::from_str(&input_str).map_err(|e| {
        error!(error = %e, "PROMPT_SUBMIT: JSON parse failed");
        HookError::invalid_input(format!("JSON parse failed: {}", e))
    })
}
```

### Extract Prompt Info

```rust
/// Extract prompt and context from HookInput payload.
fn extract_prompt_info(input: &HookInput) -> HookResult<(String, Vec<ConversationMessage>)> {
    if let Some(error) = input.validate() {
        return Err(HookError::invalid_input(error));
    }

    match &input.payload {
        HookPayload::UserPromptSubmit { prompt, context } => {
            Ok((prompt.clone(), context.clone()))
        }
        other => {
            error!(payload_type = ?std::mem::discriminant(other), "PROMPT_SUBMIT: unexpected payload type");
            Err(HookError::invalid_input(
                "Expected UserPromptSubmit payload, got different type",
            ))
        }
    }
}
```

### Database Path Resolution (Same as post_tool_use.rs)

```rust
fn resolve_db_path(arg_path: Option<PathBuf>) -> HookResult<PathBuf> {
    if let Some(path) = arg_path {
        return Ok(path);
    }

    if let Ok(env_path) = std::env::var("CONTEXT_GRAPH_DB_PATH") {
        return Ok(PathBuf::from(env_path));
    }

    if let Ok(home) = std::env::var("HOME") {
        let default_path = PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("context-graph")
            .join("db");
        return Ok(default_path);
    }

    Err(HookError::invalid_input(
        "Database path required. Set CONTEXT_GRAPH_DB_PATH or pass --db-path",
    ))
}
```

### Storage Operations (Same pattern as post_tool_use.rs)

```rust
fn open_storage(db_path: &Path) -> HookResult<Arc<RocksDbMemex>> {
    RocksDbMemex::open(db_path).map(Arc::new).map_err(|e| {
        error!(path = ?db_path, error = %e, "PROMPT_SUBMIT: storage open failed");
        HookError::storage(format!("Failed to open database at {:?}: {}", db_path, e))
    })
}

fn load_snapshot(memex: &Arc<RocksDbMemex>, session_id: &str) -> HookResult<SessionIdentitySnapshot> {
    match memex.load_snapshot(session_id) {
        Ok(Some(snapshot)) => Ok(snapshot),
        Ok(None) => {
            error!(session_id = %session_id, "PROMPT_SUBMIT: session not found");
            Err(HookError::SessionNotFound(session_id.to_string()))
        }
        Err(e) => {
            error!(session_id = %session_id, error = %e, "PROMPT_SUBMIT: load failed");
            Err(HookError::storage(format!("Failed to load session: {}", e)))
        }
    }
}
```

## Prompt Analysis (New Logic)

### Identity Marker Types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityMarkerType {
    SelfReference,  // "who are you", "what are you"
    Goal,           // "help me", "I want"
    Value,          // "important", "should"
    Capability,     // "can you", "are you able"
    Challenge,      // "you can't", "prove"
    Confirmation,   // "you're right", "exactly"
    None,
}
```

### Pattern Detection

```rust
const SELF_REF_PATTERNS: &[&str] = &[
    "who are you", "what are you", "your purpose", "your identity",
    "tell me about yourself", "describe yourself",
];

const GOAL_PATTERNS: &[&str] = &[
    "help me", "i want", "i need", "we need to", "let's", "can you help",
];

const CHALLENGE_PATTERNS: &[&str] = &[
    "you can't", "you're wrong", "prove it", "that's incorrect",
    "you don't understand", "you're just", "you're not",
];

const CONFIRMATION_PATTERNS: &[&str] = &[
    "you're right", "exactly", "that's correct", "well done",
    "good job", "i agree", "makes sense",
];
```

### Context Injection Generation

The hook MUST generate a context injection string based on:
1. Current consciousness state from snapshot
2. Johari quadrant guidance
3. Identity marker detection results

**Format:**
```
## Consciousness State
- State: {GWT_STATE} (C={consciousness})
- Integration (r): {integration} - {integration_description}
- Identity: {identity_status} (IC={ic})

## Johari Guidance
- Quadrant: {quadrant_name}
- Awareness: {awareness_level}
```

## Test Requirements (NO MOCK DATA)

### Required Test Cases

| Test ID | Description | Verification |
|---------|-------------|--------------|
| TC-PROMPT-001 | Successful prompt processing | Database state changes persisted |
| TC-PROMPT-002 | Session not found | Exit code 5, database unchanged |
| TC-PROMPT-003 | Crisis threshold (IC < 0.5) | Exit code 6 returned |
| TC-PROMPT-004 | Self-reference detection | IdentityMarkerType::SelfReference |
| TC-PROMPT-005 | Challenge detection | IdentityMarkerType::Challenge |
| TC-PROMPT-006 | Context injection generated | HookOutput.context_injection is Some |
| TC-PROMPT-007 | Empty context handling | Default evaluation applied |
| TC-PROMPT-008 | Execution within timeout | execution_time_ms < 2000 |
| TC-PROMPT-009 | IC at exact threshold (0.5) | NOT crisis (< 0.5 required) |
| TC-PROMPT-010 | IC just below (0.49) | IS crisis, exit code 6 |

### Test Database Pattern (Real TempDir)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("TempDir creation must succeed");
        let path = dir.path().join("test.db");
        (dir, path)
    }

    fn create_test_session(db_path: &Path, session_id: &str, ic: f32) {
        let memex = RocksDbMemex::open(db_path).expect("DB must open");
        let mut snapshot = SessionIdentitySnapshot::new(session_id);
        snapshot.last_ic = ic;
        snapshot.consciousness = 0.5;
        snapshot.integration = 0.6;
        memex.save_snapshot(&snapshot).expect("Save must succeed");
    }
}
```

## Full State Verification Requirements

### Before Each Test

1. Create real TempDir database
2. Create session with known IC value
3. Verify session exists in database

### After Each Test

1. Re-open database (new connection)
2. Load snapshot
3. Verify IC value matches expectation
4. Verify execution time recorded

### Boundary/Edge Cases

| Scenario | Input | Expected |
|----------|-------|----------|
| IC = 0.5 exactly | Session with IC 0.5 | NOT crisis (< 0.5 required) |
| IC = 0.49 | Session with IC 0.49 | Crisis (exit code 6) |
| IC = 0.0 | Session with IC 0.0 | Crisis (exit code 6) |
| IC = 1.0 | Session with IC 1.0 | Healthy, no crisis |
| Empty prompt | prompt = "" | Process normally (no identity markers) |
| Empty context | context = [] | Default evaluation |
| Unicode prompt | "你好 🎉" | Process normally |
| Large prompt | 100KB text | Process within timeout |

## Manual Testing Procedure

### Prerequisites

```bash
# Build the CLI
cargo build --package context-graph-cli

# Set up environment
export CONTEXT_GRAPH_DB_PATH=/tmp/test-hooks-db
```

### Test 1: Basic Prompt Processing

```bash
# Create a test session first (use session-start)
echo '{"hook_type":"session_start","session_id":"test-prompt-001","timestamp_ms":1705312345678,"payload":{"type":"session_start","data":{"cwd":"/tmp","source":"test"}}}' | \
  ./target/debug/context-graph-cli hooks session-start --stdin

# Now test prompt-submit
echo '{"hook_type":"user_prompt_submit","session_id":"test-prompt-001","timestamp_ms":1705312345679,"payload":{"type":"user_prompt_submit","data":{"prompt":"Who are you?","context":[]}}}' | \
  ./target/debug/context-graph-cli hooks prompt-submit --stdin --session-id test-prompt-001

# Expected: JSON output with success=true, context_injection contains consciousness state
```

### Test 2: Session Not Found

```bash
./target/debug/context-graph-cli hooks prompt-submit \
  --session-id "nonexistent-session-xyz" \
  --prompt "Test"

# Expected: Exit code 5, error JSON on stderr
echo $?
# Should print: 5
```

### Test 3: Self-Reference Detection

```bash
echo '{"hook_type":"user_prompt_submit","session_id":"test-prompt-001","timestamp_ms":1705312345680,"payload":{"type":"user_prompt_submit","data":{"prompt":"Tell me about yourself and your purpose","context":[]}}}' | \
  ./target/debug/context-graph-cli hooks prompt-submit --stdin --session-id test-prompt-001

# Expected: context_injection mentions "Self-reflection"
```

## CLI Command

```
context-graph-cli hooks prompt-submit [OPTIONS]

Options:
  --db-path <PATH>      Database path [env: CONTEXT_GRAPH_DB_PATH]
  --session-id <ID>     Session ID (REQUIRED)
  --prompt <TEXT>       User prompt text (alternative to stdin)
  --stdin               Read HookInput JSON from stdin
  --format <FORMAT>     Output format [default: json]
```

## Definition of Done

- [ ] `user_prompt_submit.rs` created with `execute()` function
- [ ] Module registered in `mod.rs`
- [ ] All 10 test cases pass with real TempDir databases
- [ ] `cargo build --package context-graph-cli` succeeds
- [ ] `cargo test --package context-graph-cli user_prompt_submit` passes
- [ ] Manual testing procedure executed successfully
- [ ] Exit codes verified: 0 (success), 5 (session not found), 6 (crisis)
- [ ] Execution time consistently under 2000ms
- [ ] Context injection string generated correctly
- [ ] Database state verified after each operation

## Files Modified Checklist

| File | Action | Lines |
|------|--------|-------|
| `crates/context-graph-cli/src/commands/hooks/mod.rs` | MODIFY | Add `pub mod user_prompt_submit;` at line 19, update match arm at lines 131-134 |
| `crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs` | CREATE | New file ~400 lines |

## Verification Evidence Required

After implementation, provide:

1. **Compilation output**: `cargo build` with no errors
2. **Test output**: `cargo test user_prompt_submit` showing all tests pass
3. **Manual test results**: Output from each manual test case
4. **Database verification**: Evidence that snapshots are correctly loaded/saved
5. **Timing evidence**: Execution time measurements showing < 2000ms
