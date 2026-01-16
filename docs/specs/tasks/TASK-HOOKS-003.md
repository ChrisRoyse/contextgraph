# TASK-HOOKS-003: Create HookPayload Variants

```xml
<task_spec id="TASK-HOOKS-003" version="2.1">
<metadata>
  <title>Create HookPayload Enum with Event-Specific Variants</title>
  <status>COMPLETE</status>
  <completed_date>2026-01-15</completed_date>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <requirement_ref>REQ-HOOKS-10</requirement_ref>
    <requirement_ref>REQ-HOOKS-11</requirement_ref>
    <requirement_ref>REQ-HOOKS-12</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="DONE">TASK-HOOKS-001</task_ref>
    <task_ref status="COMPLETE">TASK-HOOKS-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_hours>1.0</estimated_hours>
  <updated>2026-01-15</updated>
</metadata>

<context>
This task creates the HookPayload enum with typed variants for each hook event type.
Each variant contains the specific data Claude Code passes to that hook type.
This replaces the temporary serde_json::Value payload in HookInput.

CRITICAL CONTEXT:
- TASK-HOOKS-001: COMPLETED 2026-01-15 - HookEventType enum exists at lines 46-71 of types.rs
- TASK-HOOKS-002: COMPLETED - HookInput/HookOutput exist at lines 795-918 of types.rs
- HookInput.payload is currently `serde_json::Value` (line 803) with comment "typed in TASK-HOOKS-003"
- This task CHANGES the payload type from serde_json::Value to HookPayload enum

DISCREPANCY NOTE: The _index.md shows title "CLI Argument Types for Hooks" which is INCORRECT.
This task implements HookPayload variants (REQ-HOOKS-10,11,12). CLI argument types are TASK-HOOKS-004.

Claude Code Hook Input Schema (from docs2/claudehooks.md):
Base fields ALL hooks receive via stdin JSON:
  {session_id, transcript_path, cwd, permission_mode, hook_event_name}

Event-specific additional fields:
- SessionStart: source ("startup"/"resume"/"clear")
- PreToolUse: tool_name, tool_input, tool_use_id?
- PostToolUse: tool_name, tool_input, tool_response, tool_use_id?
- UserPromptSubmit: prompt
- SessionEnd: reason ("exit"/"clear"/"logout"/"prompt_input_exit"/"other")
</context>

<constitution_references>
- IDENTITY-002: IC thresholds (healthy >0.9, warning <0.7, critical <0.5)
- AP-25: Kuramoto N=13 oscillators
- AP-26: Exit codes (0=success, 1=error, 2=corruption)
- AP-50: NO internal hooks (use Claude Code native)
- AP-53: Hook logic in shell scripts calling CLI
</constitution_references>
```

## ⚠️ CRITICAL: NO BACKWARDS COMPATIBILITY

This system MUST work correctly or FAIL FAST with clear error messages. There are NO fallbacks, NO graceful degradation, NO backwards compatibility shims.

**Exit Codes (AP-26):**
- `0` = Success
- `1` = Error (validation, parsing, logic)
- `2` = Corruption (data integrity failure)

**Error Handling Rules:**
1. Parse errors → exit code 1 with `{"success": false, "error": "Parse error: <details>"}`
2. Validation errors → exit code 1 with specific field errors
3. Missing required fields → exit code 1 immediately
4. Type mismatches → exit code 1 with expected vs actual
5. NEVER silently ignore errors or use default values for required data

## 📍 Source of Truth

### Specification Documents
| Document | Location | Relevant Section |
|----------|----------|-----------------|
| Functional Spec | `docs/specs/functional/SPEC-HOOKS.md` | REQ-HOOKS-10,11,12 definitions |
| Technical Spec | `docs/specs/technical/TECH-HOOKS.md` | Section 2.2 (lines 312-371) |
| Constitution | `docs2/constitution.yaml` | claude_code.hooks, gwt.self_ego_node.thresholds |
| Claude Code Hooks Ref | `docs2/claudehooks.md` | Input Schema (lines 63-81) |

### Implementation Files (VERIFIED 2026-01-15)
| File | Current State | This Task Changes |
|------|---------------|-------------------|
| `crates/context-graph-cli/src/commands/hooks/types.rs` | HookInput.payload = `serde_json::Value` (line 803) | Replace with `HookPayload` enum |
| `crates/context-graph-cli/src/commands/hooks/mod.rs` | Exports HookInput (line 22) | Add HookPayload, ConversationMessage, SessionEndStatus exports |

### Database (Source of Session State)
- RocksDB at `CONTEXT_GRAPH_DB_PATH` (default: `.context-graph/data`)
- Session identity stored as `SessionIdentitySnapshot` (see TECH-HOOKS.md Section 2.1)

## 🎯 Scope

### In Scope
- Create `HookPayload` enum with 5 typed variants
- Create `ConversationMessage` struct
- Create `SessionEndStatus` enum
- Update `HookInput` to use typed `HookPayload` instead of `serde_json::Value`
- Update `mod.rs` exports
- Add unit tests with REAL data (no mocks)

### Out of Scope
- CLI argument types (TASK-HOOKS-004)
- Error handling types (TASK-HOOKS-005)
- Command handlers (TASK-HOOKS-006+)

## 📋 Prerequisites Check

Before starting, verify these conditions are met:

```bash
# 1. Verify TASK-HOOKS-001 is complete (HookEventType exists at line 46)
grep -n "pub enum HookEventType" crates/context-graph-cli/src/commands/hooks/types.rs
# Expected: 46:pub enum HookEventType {

# 2. Verify TASK-HOOKS-002 is complete (HookInput exists with serde_json::Value payload at line 803)
grep -n "pub payload: serde_json::Value" crates/context-graph-cli/src/commands/hooks/types.rs
# Expected: 803:    pub payload: serde_json::Value,

# 3. Verify serde is available (workspace dependency)
grep "serde" crates/context-graph-cli/Cargo.toml
# Expected: serde = { workspace = true }

# 4. Verify chrono is available in workspace (for timestamps in HookInput::new)
grep "chrono" Cargo.toml
# Expected: chrono = { version = "0.4", features = ["serde"] }
# NOTE: May need to add `chrono = { workspace = true }` to CLI Cargo.toml if not present
```

## 🔧 Implementation

### Step 1: Add SessionEndStatus enum

Insert AFTER the `ICClassification` impl block (after line ~772) and BEFORE the HookInput section header comment (line ~774):

```rust
// =============================================================================
// Session End Status
// Technical Reference: TECH-HOOKS.md Section 2.2
// Implements: REQ-HOOKS-12
// Claude Code Reference: docs2/claudehooks.md line 79
// Valid values: exit, clear, logout, prompt_input_exit, other
// =============================================================================

/// Session end status indicating how the session terminated.
/// Maps to Claude Code's SessionEnd.reason field.
///
/// # Serialization
/// Uses snake_case: Normal -> "normal", UserAbort -> "user_abort"
///
/// # NO BACKWARDS COMPATIBILITY
/// Invalid status values cause parse failure (exit code 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndStatus {
    /// Clean session termination (maps to Claude Code "exit")
    Normal,
    /// Session exceeded timeout limit
    Timeout,
    /// Error occurred during session
    Error,
    /// User explicitly interrupted/aborted session (Ctrl+C, etc.)
    UserAbort,
    /// Clear command issued (maps to Claude Code "clear")
    Clear,
    /// Logout issued (maps to Claude Code "logout")
    Logout,
}

impl Default for SessionEndStatus {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for SessionEndStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Timeout => write!(f, "timeout"),
            Self::Error => write!(f, "error"),
            Self::UserAbort => write!(f, "user_abort"),
            Self::Clear => write!(f, "clear"),
            Self::Logout => write!(f, "logout"),
        }
    }
}

impl SessionEndStatus {
    /// Convert from Claude Code's reason string
    pub fn from_claude_code_reason(reason: &str) -> Self {
        match reason {
            "exit" => Self::Normal,
            "clear" => Self::Clear,
            "logout" => Self::Logout,
            "prompt_input_exit" => Self::UserAbort,
            "other" => Self::Error,
            _ => Self::Error, // Unknown reasons treated as errors
        }
    }
}
```

### Step 2: Add ConversationMessage struct

Insert immediately after SessionEndStatus:

```rust
// =============================================================================
// Conversation Message
// Technical Reference: TECH-HOOKS.md Section 2.2
// Implements: REQ-HOOKS-11 (UserPromptSubmit context)
// =============================================================================

/// A single message in the conversation context.
/// Used in UserPromptSubmit payload to provide recent conversation history.
///
/// # Fields
/// - `role`: "user", "assistant", or "system"
/// - `content`: The message text
///
/// # NO BACKWARDS COMPATIBILITY
/// Missing or empty role/content fields cause parse failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationMessage {
    /// Message role: "user", "assistant", or "system"
    pub role: String,
    /// Message content text
    pub content: String,
}

impl ConversationMessage {
    /// Create a new conversation message.
    ///
    /// # Arguments
    /// * `role` - Message role (user/assistant/system)
    /// * `content` - Message text content
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }
}
```

### Step 3: Add HookPayload enum

Insert immediately after ConversationMessage:

```rust
// =============================================================================
// Hook Payload (typed payloads for each hook event)
// Technical Reference: TECH-HOOKS.md Section 2.2 (lines 312-371)
// Claude Code Reference: docs2/claudehooks.md Input Schema (lines 63-81)
// Implements: REQ-HOOKS-10, REQ-HOOKS-11, REQ-HOOKS-12
// =============================================================================

/// Event-specific payload data for Claude Code hooks.
/// Maps directly to Claude Code's hook input JSON structure.
///
/// # Claude Code Input Fields (base + event-specific)
/// Base: {session_id, transcript_path, cwd, permission_mode, hook_event_name}
///
/// Event-specific (from docs2/claudehooks.md):
/// - SessionStart: source (startup/resume/clear)
/// - PreToolUse: tool_name, tool_input, tool_use_id?
/// - PostToolUse: tool_name, tool_input, tool_response, tool_use_id?
/// - UserPromptSubmit: prompt
/// - SessionEnd: reason (exit/clear/logout/prompt_input_exit/other)
///
/// # Serialization Format
/// Uses internally-tagged enum: `{"type": "session_start", "data": {...}}`
///
/// # NO BACKWARDS COMPATIBILITY
/// Unknown variant types or malformed payloads cause parse failure (exit code 1).
/// This is intentional per AP-50/AP-53 - we fail fast, not gracefully.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum HookPayload {
    /// SessionStart payload - session initialization
    /// Timeout: 5000ms (allows DB access for session restoration)
    /// Claude Code field: source (startup/resume/clear)
    SessionStart {
        /// Working directory path (absolute path to project root)
        /// Maps to Claude Code's base field: cwd
        cwd: String,
        /// Session start source: "startup", "resume", or "clear"
        /// Maps to Claude Code's SessionStart.source field
        source: String,
        /// Previous session ID for identity continuity linking (optional)
        previous_session_id: Option<String>,
    },

    /// PreToolUse payload - before tool execution
    /// Timeout: 100ms FAST PATH - NO database access allowed
    /// This hook must return quickly to not block Claude Code
    PreToolUse {
        /// Tool name being invoked (e.g., "Read", "Write", "Bash")
        tool_name: String,
        /// Tool input parameters as arbitrary JSON
        /// Kept as serde_json::Value because tool inputs vary by tool
        tool_input: serde_json::Value,
        /// Optional tool use ID for correlation
        tool_use_id: Option<String>,
    },

    /// PostToolUse payload - after tool execution
    /// Timeout: 3000ms (allows DB updates)
    PostToolUse {
        /// Tool name that was executed
        tool_name: String,
        /// Tool input that was provided
        tool_input: serde_json::Value,
        /// Tool execution response (may contain output or error)
        tool_response: serde_json::Value,
        /// Optional tool use ID for correlation
        tool_use_id: Option<String>,
    },

    /// UserPromptSubmit payload - user prompt submitted
    /// Timeout: 2000ms (allows context injection)
    UserPromptSubmit {
        /// User's prompt text
        prompt: String,
        /// Recent conversation messages for context (optional)
        /// Typically last 3-5 messages
        context: Option<Vec<ConversationMessage>>,
    },

    /// SessionEnd payload - session termination
    /// Timeout: 30000ms (allows final state persistence)
    SessionEnd {
        /// Session duration in milliseconds
        duration_ms: u64,
        /// How the session ended (maps from Claude Code reason field)
        status: SessionEndStatus,
        /// Original Claude Code reason string
        reason: String,
    },
}

impl HookPayload {
    // =========================================================================
    // Type Checking Methods (const fn for compile-time optimization)
    // =========================================================================

    /// Check if this is a SessionStart payload
    #[inline]
    pub const fn is_session_start(&self) -> bool {
        matches!(self, Self::SessionStart { .. })
    }

    /// Check if this is a PreToolUse payload
    #[inline]
    pub const fn is_pre_tool_use(&self) -> bool {
        matches!(self, Self::PreToolUse { .. })
    }

    /// Check if this is a PostToolUse payload
    #[inline]
    pub const fn is_post_tool_use(&self) -> bool {
        matches!(self, Self::PostToolUse { .. })
    }

    /// Check if this is a UserPromptSubmit payload
    #[inline]
    pub const fn is_user_prompt_submit(&self) -> bool {
        matches!(self, Self::UserPromptSubmit { .. })
    }

    /// Check if this is a SessionEnd payload
    #[inline]
    pub const fn is_session_end(&self) -> bool {
        matches!(self, Self::SessionEnd { .. })
    }

    // =========================================================================
    // Constructor Methods
    // =========================================================================

    /// Create SessionStart payload
    pub fn session_start(
        cwd: impl Into<String>,
        source: impl Into<String>,
        previous_session_id: Option<String>,
    ) -> Self {
        Self::SessionStart {
            cwd: cwd.into(),
            source: source.into(),
            previous_session_id,
        }
    }

    /// Create PreToolUse payload
    pub fn pre_tool_use(
        tool_name: impl Into<String>,
        tool_input: serde_json::Value,
        tool_use_id: Option<String>,
    ) -> Self {
        Self::PreToolUse {
            tool_name: tool_name.into(),
            tool_input,
            tool_use_id,
        }
    }

    /// Create PostToolUse payload for successful tool execution
    pub fn post_tool_use(
        tool_name: impl Into<String>,
        tool_input: serde_json::Value,
        tool_response: serde_json::Value,
        tool_use_id: Option<String>,
    ) -> Self {
        Self::PostToolUse {
            tool_name: tool_name.into(),
            tool_input,
            tool_response,
            tool_use_id,
        }
    }

    /// Create UserPromptSubmit payload
    pub fn user_prompt(prompt: impl Into<String>, context: Option<Vec<ConversationMessage>>) -> Self {
        Self::UserPromptSubmit {
            prompt: prompt.into(),
            context,
        }
    }

    /// Create SessionEnd payload
    pub fn session_end(duration_ms: u64, reason: impl Into<String>) -> Self {
        let reason_str = reason.into();
        Self::SessionEnd {
            duration_ms,
            status: SessionEndStatus::from_claude_code_reason(&reason_str),
            reason: reason_str,
        }
    }

    // =========================================================================
    // Accessor Methods
    // =========================================================================

    /// Get tool name if this is a tool-related payload (PreToolUse or PostToolUse)
    pub fn tool_name(&self) -> Option<&str> {
        match self {
            Self::PreToolUse { tool_name, .. } => Some(tool_name),
            Self::PostToolUse { tool_name, .. } => Some(tool_name),
            _ => None,
        }
    }

    /// Get the working directory (only valid for SessionStart)
    pub fn cwd(&self) -> Option<&str> {
        match self {
            Self::SessionStart { cwd, .. } => Some(cwd),
            _ => None,
        }
    }

    /// Get the user prompt text (only valid for UserPromptSubmit)
    pub fn prompt(&self) -> Option<&str> {
        match self {
            Self::UserPromptSubmit { prompt, .. } => Some(prompt),
            _ => None,
        }
    }

    /// Get session duration in milliseconds (only valid for SessionEnd)
    pub fn duration_ms(&self) -> Option<u64> {
        match self {
            Self::SessionEnd { duration_ms, .. } => Some(*duration_ms),
            _ => None,
        }
    }
}
```

### Step 4: Update HookInput struct

**REPLACE** the existing HookInput struct (lines 795-818) with this updated version:

```rust
// =============================================================================
// Hook Input (stdin contract)
// Technical Reference: TECH-HOOKS.md Section 2.2
// Claude Code Reference: docs2/claudehooks.md Input Schema (lines 63-81)
// =============================================================================

/// Input received from Claude Code hook system via stdin.
/// Implements REQ-HOOKS-07, REQ-HOOKS-08, REQ-HOOKS-10
///
/// # Claude Code Input JSON Format
/// Base fields (all hooks): {session_id, transcript_path, cwd, permission_mode, hook_event_name}
/// Plus event-specific fields per docs2/claudehooks.md
///
/// # Example JSON (SessionStart)
/// ```json
/// {
///   "session_id": "session-12345",
///   "transcript_path": "/path/to/transcript.jsonl",
///   "cwd": "/home/user/project",
///   "permission_mode": "default",
///   "hook_event_name": "SessionStart",
///   "source": "startup"
/// }
/// ```
///
/// # NO BACKWARDS COMPATIBILITY
/// - Missing required fields → parse failure (exit code 1)
/// - Empty session_id → validation failure (exit code 1)
/// - Negative timestamp → validation failure (exit code 1)
/// - Unknown hook_type or payload type → parse failure (exit code 1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookInput {
    /// Hook event type (snake_case in JSON)
    pub hook_type: HookEventType,
    /// Session identifier from Claude Code (non-empty required)
    pub session_id: String,
    /// Unix timestamp in milliseconds (positive required)
    pub timestamp_ms: i64,
    /// Event-specific typed payload
    pub payload: HookPayload,
    /// Transcript file path (from Claude Code base fields)
    #[serde(default)]
    pub transcript_path: Option<String>,
    /// Current working directory (from Claude Code base fields)
    #[serde(default)]
    pub cwd: Option<String>,
    /// Permission mode (from Claude Code base fields)
    #[serde(default)]
    pub permission_mode: Option<String>,
}

impl HookInput {
    /// Create new hook input with current timestamp.
    ///
    /// # Arguments
    /// * `hook_type` - The hook event type
    /// * `session_id` - Claude Code session identifier
    /// * `payload` - Event-specific payload data
    pub fn new(
        hook_type: HookEventType,
        session_id: impl Into<String>,
        payload: HookPayload,
    ) -> Self {
        Self {
            hook_type,
            session_id: session_id.into(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            payload,
            transcript_path: None,
            cwd: None,
            permission_mode: None,
        }
    }

    /// Create hook input with explicit timestamp (for testing).
    pub fn with_timestamp(
        hook_type: HookEventType,
        session_id: impl Into<String>,
        timestamp_ms: i64,
        payload: HookPayload,
    ) -> Self {
        Self {
            hook_type,
            session_id: session_id.into(),
            timestamp_ms,
            payload,
            transcript_path: None,
            cwd: None,
            permission_mode: None,
        }
    }

    /// Parse hook input from JSON string (stdin).
    /// Returns Err on parse failure (should exit with code 1).
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Validate that input is well-formed.
    /// Returns error message if invalid, None if valid.
    ///
    /// # Validation Rules
    /// - session_id MUST NOT be empty
    /// - timestamp_ms MUST be positive
    /// - hook_type MUST match payload type
    pub fn validate(&self) -> Option<String> {
        if self.session_id.is_empty() {
            return Some("session_id cannot be empty".into());
        }
        if self.timestamp_ms <= 0 {
            return Some("timestamp_ms must be positive".into());
        }
        // Verify hook_type matches payload type
        let payload_matches = match (&self.hook_type, &self.payload) {
            (HookEventType::SessionStart, HookPayload::SessionStart { .. }) => true,
            (HookEventType::PreToolUse, HookPayload::PreToolUse { .. }) => true,
            (HookEventType::PostToolUse, HookPayload::PostToolUse { .. }) => true,
            (HookEventType::UserPromptSubmit, HookPayload::UserPromptSubmit { .. }) => true,
            (HookEventType::SessionEnd, HookPayload::SessionEnd { .. }) => true,
            _ => false,
        };
        if !payload_matches {
            return Some(format!(
                "hook_type {:?} does not match payload type",
                self.hook_type
            ));
        }
        None
    }

    /// Check if this input is for the fast path (PreToolUse).
    /// Fast path hooks have 100ms timeout and must not access database.
    #[inline]
    pub fn is_fast_path(&self) -> bool {
        self.hook_type.is_fast_path()
    }

    /// Get the timeout for this hook type in milliseconds.
    #[inline]
    pub fn timeout_ms(&self) -> u64 {
        self.hook_type.timeout_ms()
    }
}
```

### Step 5: Update mod.rs exports

**REPLACE** the contents of `crates/context-graph-cli/src/commands/hooks/mod.rs`:

```rust
//! Hook types for Claude Code native integration
//!
//! # Architecture
//! This module defines the data types for hook input/output that match
//! Claude Code's native hook system specification.
//!
//! # Claude Code Hook Reference
//! See docs2/claudehooks.md for the authoritative hook input/output schemas.
//! Base fields: {session_id, transcript_path, cwd, permission_mode, hook_event_name}
//!
//! # Constitution References
//! - IDENTITY-002: IC thresholds and timeout requirements
//! - AP-25: Kuramoto N=13
//! - AP-26: Exit codes (0=success, 1=error, 2=corruption)
//! - AP-50: NO internal hooks (use Claude Code native)
//! - AP-53: Hook logic in shell scripts calling CLI
//!
//! # NO BACKWARDS COMPATIBILITY
//! This module FAILS FAST on any error. Do not add fallback logic.

mod types;

pub use types::{
    ConsciousnessState,
    ConversationMessage,
    HookEventType,
    HookInput,
    HookOutput,
    HookPayload,
    ICClassification,
    ICLevel,
    JohariQuadrant,
    SessionEndStatus,
};
```

## 🧪 Tests

### Unit Tests (add to types.rs)

```rust
// =============================================================================
// HookPayload Tests
// =============================================================================

#[cfg(test)]
mod hook_payload_tests {
    use super::*;

    // =========================================================================
    // SessionEndStatus Tests
    // =========================================================================

    #[test]
    fn test_session_end_status_serialization() {
        // Normal -> "normal"
        let json = serde_json::to_string(&SessionEndStatus::Normal).unwrap();
        assert_eq!(json, r#""normal""#);

        // UserAbort -> "user_abort" (snake_case)
        let json = serde_json::to_string(&SessionEndStatus::UserAbort).unwrap();
        assert_eq!(json, r#""user_abort""#);
    }

    #[test]
    fn test_session_end_status_deserialization() {
        let status: SessionEndStatus = serde_json::from_str(r#""normal""#).unwrap();
        assert_eq!(status, SessionEndStatus::Normal);

        let status: SessionEndStatus = serde_json::from_str(r#""user_abort""#).unwrap();
        assert_eq!(status, SessionEndStatus::UserAbort);
    }

    #[test]
    fn test_session_end_status_invalid_fails() {
        // Unknown status MUST fail (no backwards compatibility)
        let result: Result<SessionEndStatus, _> = serde_json::from_str(r#""unknown""#);
        assert!(result.is_err());
    }

    #[test]
    fn test_session_end_status_from_claude_code() {
        assert_eq!(SessionEndStatus::from_claude_code_reason("exit"), SessionEndStatus::Normal);
        assert_eq!(SessionEndStatus::from_claude_code_reason("clear"), SessionEndStatus::Clear);
        assert_eq!(SessionEndStatus::from_claude_code_reason("logout"), SessionEndStatus::Logout);
        assert_eq!(SessionEndStatus::from_claude_code_reason("prompt_input_exit"), SessionEndStatus::UserAbort);
        assert_eq!(SessionEndStatus::from_claude_code_reason("other"), SessionEndStatus::Error);
    }

    // =========================================================================
    // ConversationMessage Tests
    // =========================================================================

    #[test]
    fn test_conversation_message_new() {
        let msg = ConversationMessage::new("user", "Hello");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_conversation_message_helpers() {
        let user_msg = ConversationMessage::user("How do I read a file?");
        assert_eq!(user_msg.role, "user");

        let assistant_msg = ConversationMessage::assistant("Use the Read tool");
        assert_eq!(assistant_msg.role, "assistant");

        let system_msg = ConversationMessage::system("Context injection");
        assert_eq!(system_msg.role, "system");
    }

    #[test]
    fn test_conversation_message_serialization() {
        let msg = ConversationMessage::user("Hello");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "Hello");
    }

    // =========================================================================
    // HookPayload SessionStart Tests
    // =========================================================================

    #[test]
    fn test_session_start_payload_serialization() {
        let payload = HookPayload::session_start("/home/user/project", "startup", None);
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["type"], "session_start");
        assert_eq!(json["data"]["cwd"], "/home/user/project");
        assert_eq!(json["data"]["source"], "startup");
        assert!(json["data"]["previous_session_id"].is_null());
    }

    #[test]
    fn test_session_start_with_previous_session() {
        let payload = HookPayload::session_start(
            "/home/user/project",
            "resume",
            Some("prev-session-123".to_string()),
        );
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["type"], "session_start");
        assert_eq!(json["data"]["source"], "resume");
        assert_eq!(json["data"]["previous_session_id"], "prev-session-123");
    }

    #[test]
    fn test_session_start_roundtrip() {
        let original = HookPayload::session_start("/tmp", "startup", Some("prev".into()));
        let json = serde_json::to_string(&original).unwrap();
        let parsed: HookPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    // =========================================================================
    // HookPayload PreToolUse Tests
    // =========================================================================

    #[test]
    fn test_pre_tool_use_payload_serialization() {
        let payload = HookPayload::pre_tool_use(
            "Read",
            serde_json::json!({"file_path": "/tmp/test.rs"}),
            None,
        );
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["type"], "pre_tool_use");
        assert_eq!(json["data"]["tool_name"], "Read");
        assert_eq!(json["data"]["tool_input"]["file_path"], "/tmp/test.rs");
    }

    #[test]
    fn test_pre_tool_use_with_tool_use_id() {
        let payload = HookPayload::pre_tool_use(
            "Bash",
            serde_json::json!({"command": "echo hello"}),
            Some("tool-123".to_string()),
        );
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["data"]["tool_use_id"], "tool-123");
    }

    // =========================================================================
    // HookPayload PostToolUse Tests
    // =========================================================================

    #[test]
    fn test_post_tool_use_success() {
        let payload = HookPayload::post_tool_use(
            "Read",
            serde_json::json!({"file_path": "/tmp/test.rs"}),
            serde_json::json!({"content": "file contents here"}),
            None,
        );
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["type"], "post_tool_use");
        assert_eq!(json["data"]["tool_name"], "Read");
        assert_eq!(json["data"]["tool_response"]["content"], "file contents here");
    }

    // =========================================================================
    // HookPayload UserPromptSubmit Tests
    // =========================================================================

    #[test]
    fn test_user_prompt_submit_simple() {
        let payload = HookPayload::user_prompt("How do I read a file?", None);
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["type"], "user_prompt_submit");
        assert_eq!(json["data"]["prompt"], "How do I read a file?");
        assert!(json["data"]["context"].is_null());
    }

    #[test]
    fn test_user_prompt_submit_with_context() {
        let context = vec![
            ConversationMessage::user("Hello"),
            ConversationMessage::assistant("Hi! How can I help?"),
            ConversationMessage::user("How do I read a file?"),
        ];
        let payload = HookPayload::user_prompt("Show me an example", Some(context));
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["type"], "user_prompt_submit");
        let context_array = json["data"]["context"].as_array().unwrap();
        assert_eq!(context_array.len(), 3);
        assert_eq!(context_array[0]["role"], "user");
        assert_eq!(context_array[1]["role"], "assistant");
    }

    // =========================================================================
    // HookPayload SessionEnd Tests
    // =========================================================================

    #[test]
    fn test_session_end_normal() {
        let payload = HookPayload::session_end(3600000, "exit");
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["type"], "session_end");
        assert_eq!(json["data"]["duration_ms"], 3600000);
        assert_eq!(json["data"]["status"], "normal");
        assert_eq!(json["data"]["reason"], "exit");
    }

    #[test]
    fn test_session_end_user_abort() {
        let payload = HookPayload::session_end(1800000, "prompt_input_exit");
        let json = serde_json::to_value(&payload).unwrap();

        assert_eq!(json["data"]["status"], "user_abort");
        assert_eq!(json["data"]["reason"], "prompt_input_exit");
    }

    // =========================================================================
    // HookPayload Type Checking Tests
    // =========================================================================

    #[test]
    fn test_payload_type_checking() {
        let session_start = HookPayload::session_start("/tmp", "startup", None);
        assert!(session_start.is_session_start());
        assert!(!session_start.is_pre_tool_use());
        assert!(!session_start.is_post_tool_use());
        assert!(!session_start.is_user_prompt_submit());
        assert!(!session_start.is_session_end());

        let pre_tool = HookPayload::pre_tool_use("Read", serde_json::Value::Null, None);
        assert!(pre_tool.is_pre_tool_use());
        assert!(!pre_tool.is_session_start());
    }

    // =========================================================================
    // HookPayload Accessor Tests
    // =========================================================================

    #[test]
    fn test_tool_name_accessor() {
        let pre_tool = HookPayload::pre_tool_use("Read", serde_json::Value::Null, None);
        assert_eq!(pre_tool.tool_name(), Some("Read"));

        let post_tool = HookPayload::post_tool_use("Write", serde_json::Value::Null, serde_json::Value::Null, None);
        assert_eq!(post_tool.tool_name(), Some("Write"));

        let session_start = HookPayload::session_start("/tmp", "startup", None);
        assert_eq!(session_start.tool_name(), None);
    }

    #[test]
    fn test_cwd_accessor() {
        let session_start = HookPayload::session_start("/home/user/project", "startup", None);
        assert_eq!(session_start.cwd(), Some("/home/user/project"));

        let pre_tool = HookPayload::pre_tool_use("Read", serde_json::Value::Null, None);
        assert_eq!(pre_tool.cwd(), None);
    }

    #[test]
    fn test_prompt_accessor() {
        let prompt = HookPayload::user_prompt("Hello!", None);
        assert_eq!(prompt.prompt(), Some("Hello!"));

        let session_end = HookPayload::session_end(1000, "exit");
        assert_eq!(session_end.prompt(), None);
    }

    #[test]
    fn test_duration_ms_accessor() {
        let session_end = HookPayload::session_end(3600000, "exit");
        assert_eq!(session_end.duration_ms(), Some(3600000));

        let session_start = HookPayload::session_start("/tmp", "startup", None);
        assert_eq!(session_start.duration_ms(), None);
    }

    // =========================================================================
    // HookInput Tests (with typed payload)
    // =========================================================================

    #[test]
    fn test_hook_input_new() {
        let input = HookInput::new(
            HookEventType::SessionStart,
            "test-session-123",
            HookPayload::session_start("/home/user", "startup", None),
        );

        assert_eq!(input.hook_type, HookEventType::SessionStart);
        assert_eq!(input.session_id, "test-session-123");
        assert!(input.timestamp_ms > 0);
        assert!(input.payload.is_session_start());
    }

    #[test]
    fn test_hook_input_full_roundtrip() {
        let input = HookInput::with_timestamp(
            HookEventType::PreToolUse,
            "session-456",
            1705312345678,
            HookPayload::pre_tool_use("Read", serde_json::json!({"file": "test.rs"}), None),
        );

        let json = serde_json::to_string(&input).unwrap();
        let parsed: HookInput = serde_json::from_str(&json).unwrap();

        assert_eq!(input.hook_type, parsed.hook_type);
        assert_eq!(input.session_id, parsed.session_id);
        assert_eq!(input.timestamp_ms, parsed.timestamp_ms);
        assert_eq!(input.payload, parsed.payload);
    }

    #[test]
    fn test_hook_input_validate_empty_session() {
        let input = HookInput::with_timestamp(
            HookEventType::SessionStart,
            "", // Empty session_id - INVALID
            1705312345678,
            HookPayload::session_start("/tmp", "startup", None),
        );

        let error = input.validate();
        assert!(error.is_some());
        assert!(error.unwrap().contains("session_id"));
    }

    #[test]
    fn test_hook_input_validate_negative_timestamp() {
        let input = HookInput::with_timestamp(
            HookEventType::SessionStart,
            "session-123",
            -1, // Negative timestamp - INVALID
            HookPayload::session_start("/tmp", "startup", None),
        );

        let error = input.validate();
        assert!(error.is_some());
        assert!(error.unwrap().contains("timestamp_ms"));
    }

    #[test]
    fn test_hook_input_validate_type_mismatch() {
        let input = HookInput::with_timestamp(
            HookEventType::SessionStart, // Type says SessionStart
            "session-123",
            1705312345678,
            HookPayload::session_end(1000, "exit"), // But payload is SessionEnd
        );

        let error = input.validate();
        assert!(error.is_some());
        assert!(error.unwrap().contains("does not match"));
    }

    #[test]
    fn test_hook_input_is_fast_path() {
        let pre_tool = HookInput::new(
            HookEventType::PreToolUse,
            "session",
            HookPayload::pre_tool_use("Read", serde_json::Value::Null, None),
        );
        assert!(pre_tool.is_fast_path());
        assert_eq!(pre_tool.timeout_ms(), 100);

        let session_start = HookInput::new(
            HookEventType::SessionStart,
            "session",
            HookPayload::session_start("/tmp", "startup", None),
        );
        assert!(!session_start.is_fast_path());
        assert_eq!(session_start.timeout_ms(), 5000);
    }
}
```

## ✅ Full State Verification

### 1. Source of Truth Definition
| What | Where | Verification Command |
|------|-------|---------------------|
| HookPayload enum | `types.rs` after ICClassification impl | `grep -n "pub enum HookPayload" crates/context-graph-cli/src/commands/hooks/types.rs` |
| HookInput.payload type | `types.rs` line ~803 (updated) | `grep -n "pub payload: HookPayload" crates/context-graph-cli/src/commands/hooks/types.rs` |
| mod.rs exports | `mod.rs` pub use block | `grep -E "HookPayload\|ConversationMessage\|SessionEndStatus" crates/context-graph-cli/src/commands/hooks/mod.rs` |

### 2. Execute & Inspect Protocol

**Build Verification (MUST pass):**
```bash
# Must succeed with 0 errors
cargo build --package context-graph-cli 2>&1 | head -50
echo "Exit code: $?"
# Expected: Exit code: 0

# Run all hook payload tests
cargo test --package context-graph-cli hook_payload_tests -- --nocapture
echo "Exit code: $?"
# Expected: Exit code: 0, all tests pass
```

**Type Structure Verification:**
```bash
# Verify HookPayload has exactly 5 variants
grep -A 50 "pub enum HookPayload" crates/context-graph-cli/src/commands/hooks/types.rs | grep -c "^\s*[A-Z][a-zA-Z]*\s*{"
# Expected: 5

# Verify SessionEndStatus exists
grep -n "pub enum SessionEndStatus" crates/context-graph-cli/src/commands/hooks/types.rs
# Expected: line number where enum is defined

# Verify ConversationMessage exists
grep -n "pub struct ConversationMessage" crates/context-graph-cli/src/commands/hooks/types.rs
# Expected: line number where struct is defined
```

### 3. Boundary & Edge Case Audit

**JSON Serialization Edge Cases (tests cover these):**
- Empty strings: `""` for session_id → validation fails
- Null values: `null` for optional fields → accepts
- Unknown fields: Extra JSON fields → ignored (serde default)
- Type mismatch: SessionStart hook_type with SessionEnd payload → validation fails

**Timeout Boundary Tests:**
```bash
# Verify PreToolUse timeout is exactly 100ms (fast path)
cargo test --package context-graph-cli test_hook_input_is_fast_path -- --nocapture
```

### 4. Evidence of Success Checklist

- [ ] `cargo build --package context-graph-cli` exits with code 0
- [ ] `cargo test --package context-graph-cli hook_payload_tests` passes ALL tests
- [ ] HookPayload enum has exactly 5 variants matching HookEventType
- [ ] Each variant fields match Claude Code input schema (docs2/claudehooks.md)
- [ ] Tagged enum JSON format: `{"type": "snake_case_variant", "data": {...}}`
- [ ] HookInput.payload type is `HookPayload` (not `serde_json::Value`)
- [ ] mod.rs exports: HookPayload, ConversationMessage, SessionEndStatus
- [ ] No unused code warnings for new types (they're exported and used)

## 🧪 Manual Testing with Synthetic Data

### Test 1: SessionStart payload parsing
```bash
echo '{"hook_type":"session_start","session_id":"manual-test-1","timestamp_ms":1705312345678,"payload":{"type":"session_start","data":{"cwd":"/home/user/project","source":"startup","previous_session_id":null}}}' | cargo run --package context-graph-cli -- hooks session-start --stdin
```
**Expected:** Exit code 0, valid JSON output

### Test 2: PreToolUse fast path (100ms budget)
```bash
time echo '{"hook_type":"pre_tool_use","session_id":"manual-test-2","timestamp_ms":1705312345678,"payload":{"type":"pre_tool_use","data":{"tool_name":"Read","tool_input":{"file_path":"/tmp/test.rs"},"tool_use_id":null}}}' | cargo run --package context-graph-cli -- hooks pre-tool --stdin --fast-path
```
**Expected:** Exit code 0, response within 100ms

### Test 3: Invalid payload type mismatch (MUST FAIL)
```bash
echo '{"hook_type":"session_start","session_id":"test","timestamp_ms":123,"payload":{"type":"session_end","data":{"duration_ms":1000,"status":"normal","reason":"exit"}}}' | cargo run --package context-graph-cli -- hooks session-start --stdin
echo "Exit code: $?"
```
**Expected:** Exit code 1, error message about type mismatch

### Test 4: Empty session_id (MUST FAIL)
```bash
echo '{"hook_type":"session_start","session_id":"","timestamp_ms":123,"payload":{"type":"session_start","data":{"cwd":"/tmp","source":"startup","previous_session_id":null}}}' | cargo run --package context-graph-cli -- hooks session-start --stdin
echo "Exit code: $?"
```
**Expected:** Exit code 1, error about empty session_id

### Test 5: Claude Code reason field mapping
```bash
# Test all Claude Code reason values map correctly
for reason in exit clear logout prompt_input_exit other; do
  echo "Testing reason: $reason"
  echo "{\"hook_type\":\"session_end\",\"session_id\":\"test\",\"timestamp_ms\":123,\"payload\":{\"type\":\"session_end\",\"data\":{\"duration_ms\":1000,\"status\":\"normal\",\"reason\":\"$reason\"}}}" | cargo run --package context-graph-cli -- hooks session-end --stdin 2>&1 | head -5
done
```

## 📊 Output Verification

### Database State Check (RocksDB)
After running CLI commands, verify state was persisted:
```bash
# Check RocksDB has session data (if commands touched DB)
ls -la .context-graph/data/
# Expected: RocksDB files present if session operations ran

# For SessionStart: verify identity snapshot loaded
cargo run --package context-graph-cli -- session status
```

### JSON Output Structure Check
```bash
# Verify output matches HookOutput schema
cargo run --package context-graph-cli -- hooks session-start --stdin <<< '{"hook_type":"session_start","session_id":"verify-output","timestamp_ms":123,"payload":{"type":"session_start","data":{"cwd":"/tmp","source":"startup","previous_session_id":null}}}' | jq .
# Expected: JSON with success, execution_time_ms fields
```

## ⚠️ Known Issues / Traceability Note

**IMPORTANT:** The `_index.md` file incorrectly shows:
```
| 3 | TASK-HOOKS-003 | CLI Argument Types for Hooks | foundation | 1.0 | 002 |
```

This is an ERROR. The correct mapping is:
- **TASK-HOOKS-003** implements **REQ-HOOKS-10, REQ-HOOKS-11, REQ-HOOKS-12** (HookPayload variants)
- **TASK-HOOKS-004** implements **REQ-HOOKS-17 through REQ-HOOKS-22** (CLI argument types)

The _index.md title should be updated to "HookPayload Enum with Event-Specific Variants".

## 📚 Reference Documents

| Document | Path | Key Sections |
|----------|------|--------------|
| Claude Code Hooks | `docs2/claudehooks.md` | Input Schema (lines 63-81), Exit Codes (lines 134-140) |
| Claude Code Skills | `docs2/claudeskills.md` | Context for skill/hook interaction |
| Constitution | `docs2/constitution.yaml` | AP-26 (exit codes), AP-50/53 (native hooks) |
| Functional Spec | `docs/specs/functional/SPEC-HOOKS.md` | REQ-HOOKS-10,11,12 definitions |
| Technical Spec | `docs/specs/technical/TECH-HOOKS.md` | Section 2.2 payload definitions |
