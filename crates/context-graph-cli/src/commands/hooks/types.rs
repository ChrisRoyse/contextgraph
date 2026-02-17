//! HookEventType enum for Claude Code native hooks
//!
//! # Performance Budget (per constitution.yaml)
//! - PreToolUse: 500ms (FAST PATH - NO DB ACCESS)
//! - UserPromptSubmit: 2000ms
//! - PostToolUse: 3000ms
//! - SessionStart: 5000ms
//! - SessionEnd: 30000ms
//!
//! # Constitution References
//! - AP-26: Exit codes (0=success, 1=error, 2=corruption)
//!
//! # NO BACKWARDS COMPATIBILITY - FAIL FAST

use serde::{Deserialize, Serialize};

/// Hook event types matching Claude Code native hooks
/// Implements REQ-HOOKS-01 through REQ-HOOKS-05
///
/// # Timeout Values (Claude Code enforced)
/// | Event | Timeout | Description |
/// |-------|---------|-------------|
/// | SessionStart | 5000ms | Session initialization |
/// | PreToolUse | 500ms | FAST PATH - cache only |
/// | PostToolUse | 3000ms | Stability verification |
/// | UserPromptSubmit | 2000ms | Context injection |
/// | SessionEnd | 30000ms | Final persistence |
///
/// # JSON Serialization
/// Uses snake_case: `session_start`, `pre_tool_use`, etc.
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::HookEventType;
///
/// let hook = HookEventType::PreToolUse;
/// assert_eq!(hook.timeout_ms(), 500);
/// assert!(hook.is_fast_path());
///
/// let json = serde_json::to_string(&hook).expect("serialization must succeed");
/// assert_eq!(json, "\"pre_tool_use\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEventType {
    /// Session initialization (timeout: 5000ms)
    /// Triggered: startup, resume, /clear
    /// CLI: `session restore-identity`
    SessionStart,

    /// Before tool execution (timeout: 500ms) - FAST PATH
    /// CRITICAL: Must not access database, uses SessionCache only
    /// CLI: `coherence brief`
    PreToolUse,

    /// After tool execution (timeout: 3000ms)
    /// Updates state and trajectory based on tool result
    PostToolUse,

    /// User prompt submitted (timeout: 2000ms)
    /// Injects relevant context from session memory
    /// CLI: `coherence inject-context --format standard`
    UserPromptSubmit,

    /// Session termination (timeout: 30000ms)
    /// Persists final snapshot and optional consolidation
    /// CLI: `session persist-identity`
    SessionEnd,
}

impl HookEventType {
    /// Get timeout in milliseconds for this hook type
    ///
    /// # Returns
    /// Timeout value in milliseconds as enforced by Claude Code
    ///
    /// # Performance Note
    /// PreToolUse has the strictest timeout (500ms) and MUST use
    /// cached state only - NO database access allowed.
    #[inline]
    pub const fn timeout_ms(&self) -> u64 {
        match self {
            Self::PreToolUse => 500,        // Fast path - NO DB access
            Self::UserPromptSubmit => 2000, // Context injection
            Self::PostToolUse => 3000,      // Stability update + trajectory
            Self::SessionStart => 5000,     // Load/create snapshot
            Self::SessionEnd => 30000,      // Final persist + consolidation
        }
    }

    /// Check if this hook type is time-critical (fast path)
    ///
    /// # Returns
    /// `true` if timeout is under 500ms (only PreToolUse)
    ///
    /// # Performance Implications
    /// Fast path hooks MUST NOT:
    /// - Access disk/database
    /// - Perform network calls
    /// - Block on locks for more than microseconds
    #[inline]
    pub const fn is_fast_path(&self) -> bool {
        self.timeout_ms() <= 500
    }

    /// Get human-readable description of this hook type
    ///
    /// # Returns
    /// Static string describing the hook's purpose
    pub const fn description(&self) -> &'static str {
        match self {
            Self::SessionStart => "Session initialization and state restoration",
            Self::PreToolUse => "Pre-tool brief injection (FAST PATH)",
            Self::PostToolUse => "Post-tool state verification",
            Self::UserPromptSubmit => "User prompt context injection",
            Self::SessionEnd => "Session persistence and consolidation",
        }
    }

    /// Get the corresponding CLI command for this hook type
    ///
    /// # Returns
    /// CLI command string that implements this hook
    pub const fn cli_command(&self) -> &'static str {
        match self {
            Self::SessionStart => "hooks session-start",
            Self::PreToolUse => "hooks pre-tool",
            Self::PostToolUse => "hooks post-tool",
            Self::UserPromptSubmit => "hooks prompt-submit",
            Self::SessionEnd => "hooks session-end",
        }
    }

    /// Get all hook event types as an array
    ///
    /// # Returns
    /// Array of all 5 hook event types in execution order
    pub const fn all() -> [Self; 5] {
        [
            Self::SessionStart,
            Self::PreToolUse,
            Self::PostToolUse,
            Self::UserPromptSubmit,
            Self::SessionEnd,
        ]
    }
}

impl std::fmt::Display for HookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

// =============================================================================
// TESTS - NO MOCK DATA - REAL VALUES ONLY
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SOURCE OF TRUTH: TECH-HOOKS.md Section 2.2 + .claude/settings.json
    // =========================================================================

    // =========================================================================
    // TC-HOOKS-001: Timeout Values Match Specification
    // SOURCE: TECH-HOOKS.md Section 2.2, constitution.yaml claude_code.hooks
    // =========================================================================
    #[test]
    fn tc_hooks_001_timeout_values_match_spec() {
        println!("\n=== TC-HOOKS-001: Timeout Values Match Specification ===");
        println!("SOURCE OF TRUTH: TECH-HOOKS.md Section 2.2");
        println!("CONSTITUTION: claude_code.hooks.timeouts");

        // These values are from TECH-HOOKS.md and constitution.yaml
        // DO NOT CHANGE without updating both sources
        let expected_timeouts = [
            (HookEventType::SessionStart, 5000_u64, "session_start"),
            (HookEventType::PreToolUse, 500_u64, "pre_tool_use"),
            (HookEventType::PostToolUse, 3000_u64, "post_tool_use"),
            (
                HookEventType::UserPromptSubmit,
                2000_u64,
                "user_prompt_submit",
            ),
            (HookEventType::SessionEnd, 30000_u64, "session_end"),
        ];

        for (hook, expected_timeout, name) in expected_timeouts {
            let actual = hook.timeout_ms();
            println!(
                "  {}: expected={}ms, actual={}ms",
                name, expected_timeout, actual
            );
            assert_eq!(
                actual, expected_timeout,
                "FAIL: {} timeout must be {}ms, got {}ms",
                name, expected_timeout, actual
            );
        }

        println!("RESULT: PASS - All timeout values match specification");
    }

    // =========================================================================
    // TC-HOOKS-002: Serialization Produces snake_case
    // SOURCE: Claude Code hooks JSON format requirement
    // =========================================================================
    #[test]
    fn tc_hooks_002_serialization_snake_case() {
        println!("\n=== TC-HOOKS-002: Serialization Produces snake_case ===");
        println!("SOURCE OF TRUTH: Claude Code hook JSON format");

        let test_cases = [
            (HookEventType::SessionStart, r#""session_start""#),
            (HookEventType::PreToolUse, r#""pre_tool_use""#),
            (HookEventType::PostToolUse, r#""post_tool_use""#),
            (HookEventType::UserPromptSubmit, r#""user_prompt_submit""#),
            (HookEventType::SessionEnd, r#""session_end""#),
        ];

        for (hook, expected_json) in test_cases {
            let actual_json =
                serde_json::to_string(&hook).expect("serialization MUST succeed - fail fast");
            println!("  {:?} -> {}", hook, actual_json);
            assert_eq!(
                actual_json, expected_json,
                "FAIL: {:?} must serialize to {}, got {}",
                hook, expected_json, actual_json
            );
        }

        println!("RESULT: PASS - All variants serialize to snake_case");
    }

    // =========================================================================
    // TC-HOOKS-003: Deserialization from snake_case
    // SOURCE: Claude Code hook JSON format requirement
    // =========================================================================
    #[test]
    fn tc_hooks_003_deserialization_snake_case() {
        println!("\n=== TC-HOOKS-003: Deserialization from snake_case ===");
        println!("SOURCE OF TRUTH: Claude Code hook JSON format");

        let test_cases = [
            (r#""session_start""#, HookEventType::SessionStart),
            (r#""pre_tool_use""#, HookEventType::PreToolUse),
            (r#""post_tool_use""#, HookEventType::PostToolUse),
            (r#""user_prompt_submit""#, HookEventType::UserPromptSubmit),
            (r#""session_end""#, HookEventType::SessionEnd),
        ];

        for (json, expected_hook) in test_cases {
            let actual_hook: HookEventType =
                serde_json::from_str(json).expect("deserialization MUST succeed - fail fast");
            println!("  {} -> {:?}", json, actual_hook);
            assert_eq!(
                actual_hook, expected_hook,
                "FAIL: {} must deserialize to {:?}, got {:?}",
                json, expected_hook, actual_hook
            );
        }

        println!("RESULT: PASS - All snake_case strings deserialize correctly");
    }

    // =========================================================================
    // TC-HOOKS-004: Exactly 5 Variants Exist
    // SOURCE: Claude Code native hook specification
    // =========================================================================
    #[test]
    fn tc_hooks_004_exactly_five_variants() {
        println!("\n=== TC-HOOKS-004: Exactly 5 Variants Exist ===");
        println!("SOURCE OF TRUTH: Claude Code native hook specification");

        let all_variants = HookEventType::all();
        println!("  Variant count: {}", all_variants.len());

        assert_eq!(
            all_variants.len(),
            5,
            "FAIL: Must have exactly 5 variants, got {}",
            all_variants.len()
        );

        // Verify all variants are unique
        let mut seen = std::collections::HashSet::new();
        for variant in all_variants {
            assert!(
                seen.insert(variant),
                "FAIL: Duplicate variant detected: {:?}",
                variant
            );
        }

        println!("  All variants unique: true");
        println!("RESULT: PASS - Exactly 5 unique variants exist");
    }

    // =========================================================================
    // TC-HOOKS-005: Fast Path Detection
    // SOURCE: TECH-HOOKS.md fast path requirement (<500ms)
    // =========================================================================
    #[test]
    fn tc_hooks_005_fast_path_detection() {
        println!("\n=== TC-HOOKS-005: Fast Path Detection ===");
        println!("SOURCE OF TRUTH: constitution.yaml fast path requirement");
        println!("THRESHOLD: timeout <= 500ms");

        let fast_path_expected = [
            (HookEventType::SessionStart, false),
            (HookEventType::PreToolUse, true), // ONLY fast path
            (HookEventType::PostToolUse, false),
            (HookEventType::UserPromptSubmit, false),
            (HookEventType::SessionEnd, false),
        ];

        for (hook, expected_fast) in fast_path_expected {
            let actual_fast = hook.is_fast_path();
            println!(
                "  {:?}: timeout={}ms, is_fast_path={} (expected={})",
                hook,
                hook.timeout_ms(),
                actual_fast,
                expected_fast
            );
            assert_eq!(
                actual_fast, expected_fast,
                "FAIL: {:?}.is_fast_path() must be {}, got {}",
                hook, expected_fast, actual_fast
            );
        }

        println!("RESULT: PASS - Only PreToolUse is fast path");
    }

    // =========================================================================
    // TC-HOOKS-006: Copy and Clone Traits
    // SOURCE: Rust type safety requirement
    // =========================================================================
    #[test]
    fn tc_hooks_006_copy_clone_traits() {
        println!("\n=== TC-HOOKS-006: Copy and Clone Traits ===");
        println!("SOURCE OF TRUTH: Rust type system requirements");

        let original = HookEventType::PreToolUse;
        let copied = original; // Copy
        let cloned = original.clone(); // Clone

        assert_eq!(original, copied, "FAIL: Copy must preserve value");
        assert_eq!(original, cloned, "FAIL: Clone must preserve value");

        // Verify we can use original after copy (proves Copy, not Move)
        assert_eq!(original.timeout_ms(), 500);

        println!("  Original after copy: {:?}", original);
        println!("  Copied: {:?}", copied);
        println!("  Cloned: {:?}", cloned);
        println!("RESULT: PASS - Copy and Clone work correctly");
    }

    // =========================================================================
    // TC-HOOKS-007: Hash Trait for HashMap Usage
    // SOURCE: Rust HashMap requirement
    // =========================================================================
    #[test]
    fn tc_hooks_007_hash_trait() {
        println!("\n=== TC-HOOKS-007: Hash Trait for HashMap Usage ===");
        println!("SOURCE OF TRUTH: Rust HashMap requirement");

        use std::collections::HashMap;

        let mut map: HashMap<HookEventType, u64> = HashMap::new();
        for hook in HookEventType::all() {
            map.insert(hook, hook.timeout_ms());
        }

        assert_eq!(map.len(), 5, "FAIL: HashMap must contain all 5 variants");
        assert_eq!(
            map.get(&HookEventType::PreToolUse),
            Some(&500),
            "FAIL: PreToolUse must map to 500"
        );

        println!("  HashMap size: {}", map.len());
        println!(
            "  PreToolUse lookup: {:?}",
            map.get(&HookEventType::PreToolUse)
        );
        println!("RESULT: PASS - Hash trait works for HashMap");
    }

    // =========================================================================
    // TC-HOOKS-008: CLI Command Mapping
    // SOURCE: .claude/settings.json hook configuration
    // =========================================================================
    #[test]
    fn tc_hooks_008_cli_command_mapping() {
        println!("\n=== TC-HOOKS-008: CLI Command Mapping ===");
        println!("SOURCE OF TRUTH: .claude/settings.json");

        let expected_commands = [
            (HookEventType::SessionStart, "hooks session-start"),
            (HookEventType::PreToolUse, "hooks pre-tool"),
            (HookEventType::PostToolUse, "hooks post-tool"),
            (
                HookEventType::UserPromptSubmit,
                "hooks prompt-submit",
            ),
            (HookEventType::SessionEnd, "hooks session-end"),
        ];

        for (hook, expected_cmd) in expected_commands {
            let actual_cmd = hook.cli_command();
            println!("  {:?} -> \"{}\"", hook, actual_cmd);
            assert_eq!(
                actual_cmd, expected_cmd,
                "FAIL: {:?}.cli_command() must be \"{}\", got \"{}\"",
                hook, expected_cmd, actual_cmd
            );
        }

        println!("RESULT: PASS - All CLI commands match .claude/settings.json");
    }

    // =========================================================================
    // TC-HOOKS-009: Display Trait Implementation
    // SOURCE: Rust Display trait requirement
    // =========================================================================
    #[test]
    fn tc_hooks_009_display_trait() {
        println!("\n=== TC-HOOKS-009: Display Trait Implementation ===");

        for hook in HookEventType::all() {
            let display = format!("{}", hook);
            let description = hook.description();
            println!("  {:?} displays as: \"{}\"", hook, display);
            assert_eq!(display, description, "FAIL: Display must equal description");
            assert!(!display.is_empty(), "FAIL: Display must not be empty");
        }

        println!("RESULT: PASS - Display trait works correctly");
    }

    // =========================================================================
    // TC-HOOKS-010: Invalid Deserialization Fails Fast
    // SOURCE: NO BACKWARDS COMPATIBILITY requirement
    // =========================================================================
    #[test]
    fn tc_hooks_010_invalid_deserialization_fails() {
        println!("\n=== TC-HOOKS-010: Invalid Deserialization Fails Fast ===");
        println!("SOURCE OF TRUTH: NO BACKWARDS COMPATIBILITY requirement");

        let invalid_inputs = [
            r#""SessionStart""#,  // PascalCase - INVALID
            r#""sessionstart""#,  // lowercase no underscore - INVALID
            r#""SESSIONSTART""#,  // UPPERCASE - INVALID
            r#""session-start""#, // kebab-case - INVALID
            r#""unknown_hook""#,  // non-existent variant - INVALID
            r#"0"#,               // numeric - INVALID
            r#"null"#,            // null - INVALID
        ];

        for invalid in invalid_inputs {
            let result: Result<HookEventType, _> = serde_json::from_str(invalid);
            println!("  {} -> {:?}", invalid, result.is_err());
            assert!(
                result.is_err(),
                "FAIL: Invalid input {} must fail deserialization",
                invalid
            );
        }

        println!("RESULT: PASS - All invalid inputs fail fast");
    }
}

// =============================================================================
// Topic Stability Level Classification
// =============================================================================

/// Topic stability level classification
/// Thresholds:
/// - healthy: ">0.9" (stability >= 0.9)
/// - warning: "<0.7" (0.5 <= stability < 0.7)
/// - critical: "<0.5" (stability < 0.5, triggers dream)
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::StabilityLevel;
///
/// assert_eq!(StabilityLevel::from_value(0.95), StabilityLevel::Healthy);
/// assert_eq!(StabilityLevel::from_value(0.80), StabilityLevel::Normal);
/// assert_eq!(StabilityLevel::from_value(0.60), StabilityLevel::Warning);
/// assert_eq!(StabilityLevel::from_value(0.40), StabilityLevel::Critical);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StabilityLevel {
    /// >= 0.9 - Topics are stable and coherent
    Healthy,
    /// 0.7 <= value < 0.9 - Normal operation
    Normal,
    /// 0.5 <= value < 0.7 - Topic drift detected
    Warning,
    /// < 0.5 - Unstable state, dream may trigger
    Critical,
}

impl StabilityLevel {
    /// Classify value into level
    ///
    /// # Arguments
    /// * `stability` - Topic stability value [0.0, 1.0]
    ///
    /// # Returns
    /// Level classification
    ///
    /// # Panics
    /// Never panics - out-of-range values clamp to Critical/Healthy
    #[inline]
    pub fn from_value(stability: f32) -> Self {
        if stability >= 0.9 {
            Self::Healthy
        } else if stability >= 0.7 {
            Self::Normal
        } else if stability >= 0.5 {
            Self::Warning
        } else {
            Self::Critical
        }
    }

    /// Check if this level indicates a crisis state
    /// Crisis = Critical (stability < 0.5)
    #[inline]
    pub const fn is_crisis(&self) -> bool {
        matches!(self, Self::Critical)
    }

    /// Check if this level requires attention
    /// Attention needed = Warning OR Critical
    #[inline]
    pub const fn needs_attention(&self) -> bool {
        matches!(self, Self::Warning | Self::Critical)
    }
}

impl std::fmt::Display for StabilityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "Healthy (>= 0.9)"),
            Self::Normal => write!(f, "Normal (0.7 <= value < 0.9)"),
            Self::Warning => write!(f, "Warning (0.5 <= value < 0.7)"),
            Self::Critical => write!(f, "Critical (< 0.5)"),
        }
    }
}


// =============================================================================
// Coherence State
// Technical Reference: AP-25 (N=13 embedder spaces)
// =============================================================================

/// Coherence state for hook output
/// Implements REQ-HOOKS-14, REQ-HOOKS-15
///
/// All values are normalized to [0.0, 1.0].
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::CoherenceState;
///
/// let state = CoherenceState {
///     coherence: 0.73,
///     integration: 0.85,
///     reflection: 0.78,
///     differentiation: 0.82,
///     topic_stability: 0.92,
/// };
///
/// let json = serde_json::to_string(&state).unwrap();
/// assert!(json.contains("\"coherence\":0.73"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoherenceState {
    /// Current coherence level C(t) [0.0, 1.0]
    pub coherence: f32,
    /// Integration (coherence r) [0.0, 1.0]
    pub integration: f32,
    /// Reflection (meta-cognitive) [0.0, 1.0]
    pub reflection: f32,
    /// Differentiation (purpose entropy) [0.0, 1.0]
    pub differentiation: f32,
    /// Topic stability score [0.0, 1.0]
    pub topic_stability: f32,
}

impl Default for CoherenceState {
    /// Default state: DOR (Disorder of Responsiveness)
    /// - All metrics at 0.0 except stability at 1.0 (fresh state)
    fn default() -> Self {
        Self {
            coherence: 0.0,
            integration: 0.0,
            reflection: 0.0,
            differentiation: 0.0,
            topic_stability: 1.0, // Fresh state = perfect stability
        }
    }
}

impl CoherenceState {
    /// Create coherence state
    ///
    /// # Arguments
    /// * `coherence` - C(t) value
    /// * `integration` - Coherence r value
    /// * `reflection` - Meta-cognitive value
    /// * `differentiation` - Purpose entropy value
    /// * `topic_stability` - Topic stability value
    pub fn new(
        coherence: f32,
        integration: f32,
        reflection: f32,
        differentiation: f32,
        topic_stability: f32,
    ) -> Self {
        Self {
            coherence,
            integration,
            reflection,
            differentiation,
            topic_stability,
        }
    }
}

// =============================================================================
// Level Classification
// =============================================================================

/// Topic stability classification with crisis detection
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::{StabilityClassification, StabilityLevel};
///
/// let stability = StabilityClassification::new(0.45, 0.5);
/// assert!(stability.crisis_triggered);
/// assert_eq!(stability.level, StabilityLevel::Critical);
///
/// let stability = StabilityClassification::new(0.85, 0.5);
/// assert!(!stability.crisis_triggered);
/// assert_eq!(stability.level, StabilityLevel::Normal);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StabilityClassification {
    /// Topic stability value [0.0, 1.0]
    pub value: f32,
    /// Classification level
    pub level: StabilityLevel,
    /// Whether crisis threshold was breached
    pub crisis_triggered: bool,
}

impl StabilityClassification {
    /// Default crisis threshold
    pub const DEFAULT_CRISIS_THRESHOLD: f32 = 0.5;

    /// Create new stability classification from value
    ///
    /// # Arguments
    /// * `value` - Topic stability value [0.0, 1.0]
    /// * `crisis_threshold` - Threshold for crisis trigger (default 0.5)
    ///
    /// # Returns
    /// StabilityClassification with level and crisis state
    pub fn new(value: f32, crisis_threshold: f32) -> Self {
        let level = StabilityLevel::from_value(value);
        Self {
            value,
            level,
            crisis_triggered: value < crisis_threshold,
        }
    }

    /// Create with default crisis threshold (0.5)
    pub fn from_value(value: f32) -> Self {
        Self::new(value, Self::DEFAULT_CRISIS_THRESHOLD)
    }
}

// =============================================================================
// Session End Status (typed payload support)
// Technical Reference: TECH-HOOKS.md Section 2.2
// Implements: REQ-HOOKS-10
// =============================================================================

/// Status of session termination for SessionEnd hook
/// Implements REQ-HOOKS-10 (Typed Payloads)
///
/// # Variants
/// Each variant represents a distinct termination mode:
/// - `Normal`: User-initiated graceful exit
/// - `Timeout`: Session exceeded time limit
/// - `Error`: Terminated due to error condition
/// - `UserAbort`: User interrupted with Ctrl+C or similar
/// - `Clear`: Session cleared via /clear command
/// - `Logout`: User logged out
///
/// # NO BACKWARDS COMPATIBILITY
/// Fail fast on unknown status - do not add fallback variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndStatus {
    /// Normal graceful exit
    Normal,
    /// Session timed out
    Timeout,
    /// Error caused termination
    Error,
    /// User aborted (Ctrl+C)
    UserAbort,
    /// Session cleared via /clear
    Clear,
    /// User logged out
    Logout,
}

// =============================================================================
// Conversation Message (typed payload support)
// Technical Reference: TECH-HOOKS.md Section 2.2
// Implements: REQ-HOOKS-11
// =============================================================================

/// A single message in the conversation context
/// Implements REQ-HOOKS-11 (Context Structure)
///
/// Used in UserPromptSubmit payload to provide conversation history.
///
/// # Fields
/// - `role`: "user" | "assistant" | "system"
/// - `content`: Message text content
///
/// # NO BACKWARDS COMPATIBILITY
/// Unknown roles should fail deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessage {
    /// Message role: "user", "assistant", or "system"
    pub role: String,
    /// Message content text
    pub content: String,
}

// =============================================================================
// Hook Payload (typed variants per event type)
// Technical Reference: TECH-HOOKS.md Section 2.2
// Implements: REQ-HOOKS-10, REQ-HOOKS-11, REQ-HOOKS-12
// =============================================================================

/// Typed payload variants for each hook event type
/// Implements REQ-HOOKS-10 (Typed Payloads), REQ-HOOKS-11, REQ-HOOKS-12
///
/// # Variants
/// Each variant contains fields specific to its event type:
/// - `SessionStart`: Session initialization data
/// - `PreToolUse`: Tool invocation request (500ms timeout)
/// - `PostToolUse`: Tool completion with response (3000ms timeout)
/// - `UserPromptSubmit`: User input with context (2000ms timeout)
/// - `SessionEnd`: Session termination data (30000ms timeout)
///
/// # JSON Format
/// Uses internally tagged enum for Claude Code compatibility:
/// ```json
/// { "type": "session_start", "data": { "cwd": "/path", ... } }
/// ```
///
/// # NO BACKWARDS COMPATIBILITY
/// Unknown variants fail deserialization - no fallback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum HookPayload {
    /// SessionStart hook payload
    /// Timeout: 5000ms per TECH-HOOKS.md
    SessionStart {
        /// Current working directory
        cwd: String,
        /// How session was initiated (e.g., "cli", "ide")
        source: String,
        /// Previous session ID for continuity (optional)
        #[serde(skip_serializing_if = "Option::is_none")]
        previous_session_id: Option<String>,
    },

    /// PreToolUse hook payload (fast path)
    /// Timeout: 500ms total per constitution.yaml - CLI logic ~100ms
    PreToolUse {
        /// Name of tool being invoked
        tool_name: String,
        /// Tool input parameters as JSON
        tool_input: serde_json::Value,
        /// Unique identifier for this tool use
        tool_use_id: String,
    },

    /// PostToolUse hook payload
    /// Timeout: 3000ms per TECH-HOOKS.md
    PostToolUse {
        /// Name of tool that was invoked
        tool_name: String,
        /// Tool input parameters as JSON
        tool_input: serde_json::Value,
        /// Tool response/result
        tool_response: String,
        /// Unique identifier for this tool use
        tool_use_id: String,
        /// Whether the tool executed successfully (from Claude Code)
        /// Defaults to None if not provided - will use smart heuristic
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_success: Option<bool>,
    },

    /// UserPromptSubmit hook payload
    /// Timeout: 2000ms per constitution.yaml
    UserPromptSubmit {
        /// User's input prompt text
        prompt: String,
        /// Conversation history for context
        #[serde(default)]
        context: Vec<ConversationMessage>,
    },

    /// SessionEnd hook payload
    /// Timeout: 30000ms per constitution.yaml (final persist + consolidation)
    SessionEnd {
        /// Session duration in milliseconds
        duration_ms: u64,
        /// How session ended
        status: SessionEndStatus,
        /// Optional reason for termination
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    /// R5: PreCompact hook payload
    /// Timeout: 20000ms per settings.json
    /// Captures session summary before context window compression
    PreCompact {
        /// What triggered the compaction (e.g., "auto", "manual")
        trigger: String,
        /// Optional conversation summary from Claude Code
        #[serde(skip_serializing_if = "Option::is_none")]
        conversation_summary: Option<String>,
    },

    /// R6: TaskCompleted hook payload
    /// Timeout: 20000ms per settings.json
    /// Extracts learnings from completed tasks
    TaskCompleted {
        /// Task subject/title
        task_subject: String,
        /// Task ID
        task_id: String,
        /// Optional task result/output summary
        #[serde(skip_serializing_if = "Option::is_none")]
        task_result: Option<String>,
    },
}

// =============================================================================
// Hook Input (stdin contract)
// Technical Reference: TECH-HOOKS.md Section 2.2
// =============================================================================

/// Input received from Claude Code hook system via stdin
/// Implements REQ-HOOKS-07, REQ-HOOKS-08, REQ-HOOKS-10, REQ-HOOKS-11, REQ-HOOKS-12
///
/// # Typed Payloads (TASK-HOOKS-003)
/// The `payload` field uses the `HookPayload` enum for type-safe access
/// to event-specific data. Each variant matches the corresponding hook type.
///
/// # JSON Format (from Claude Code)
/// ```json
/// {
///   "hook_type": "pre_tool_use",
///   "session_id": "session-12345",
///   "timestamp_ms": 1705312345678,
///   "payload": { "type": "pre_tool_use", "data": { ... } }
/// }
/// ```
///
/// # NO BACKWARDS COMPATIBILITY
/// Unknown hook types or payload types will fail deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInput {
    /// Hook event type (snake_case in JSON)
    pub hook_type: HookEventType,
    /// Session identifier from Claude Code
    pub session_id: String,
    /// Unix timestamp in milliseconds
    pub timestamp_ms: i64,
    /// Event-specific typed payload (REQ-HOOKS-10, REQ-HOOKS-11, REQ-HOOKS-12)
    pub payload: HookPayload,
}

impl HookInput {
    /// Validate that input is well-formed
    /// Returns error message if invalid, None if valid
    pub fn validate(&self) -> Option<String> {
        if self.session_id.is_empty() {
            return Some("session_id cannot be empty".into());
        }
        if self.timestamp_ms <= 0 {
            return Some("timestamp_ms must be positive".into());
        }
        None
    }
}

// =============================================================================
// Drift Metrics (session identity drift measurement)
// Technical Reference: TASK-HOOKS-013
// Constitution Reference: topic_stability thresholds
// =============================================================================

/// Drift metrics for session topic stability tracking
/// Measures deviation between current and previous session topic stability state
///
/// # Fields
/// - `stability_delta`: Change in topic stability (current - previous)
/// - `purpose_drift`: Cosine distance between purpose vectors [0.0, 2.0]
/// - `time_since_snapshot_ms`: Time elapsed since previous snapshot
/// - `coherence_phase_drift`: Mean absolute phase difference [0.0, π]
///
/// # Thresholds (per TASK-HOOKS-013)
/// - Warning: stability_delta < -0.1
/// - Crisis: stability_delta < -0.3
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::DriftMetrics;
///
/// let metrics = DriftMetrics {
///     stability_delta: -0.15,
///     purpose_drift: 0.25,
///     time_since_snapshot_ms: 3600000,
///     coherence_phase_drift: 0.3,
/// };
///
/// assert!(metrics.is_warning_drift());
/// assert!(!metrics.is_crisis_drift());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftMetrics {
    /// Change in topic stability (current - previous)
    /// Range: [-1.0, 1.0]
    /// Negative = stability degradation, Positive = stability improvement
    pub stability_delta: f32,

    /// Cosine distance between current and previous purpose vectors
    /// Range: [0.0, 2.0] where 0.0 = identical, 2.0 = opposite
    pub purpose_drift: f32,

    /// Time elapsed since the previous session snapshot in milliseconds
    pub time_since_snapshot_ms: i64,

    /// Mean absolute phase difference across coherence metrics
    /// Range: [0.0, π] where 0.0 = perfect sync, π = opposite phases
    pub coherence_phase_drift: f64,
}

impl DriftMetrics {
    /// Crisis drift threshold for stability_delta
    /// Constitution Reference: topic_stability thresholds
    pub const CRISIS_THRESHOLD: f32 = -0.3;

    /// Warning drift threshold for stability_delta
    pub const WARNING_THRESHOLD: f32 = -0.1;

    /// Check if drift indicates a crisis state
    /// Crisis = stability_delta < -0.3 (severe stability degradation)
    ///
    /// # Returns
    /// `true` if stability_delta is below the crisis threshold
    #[inline]
    pub fn is_crisis_drift(&self) -> bool {
        self.stability_delta < Self::CRISIS_THRESHOLD
    }

    /// Check if drift indicates a warning state
    /// Warning = stability_delta < -0.1 (moderate stability degradation)
    ///
    /// # Returns
    /// `true` if stability_delta is below the warning threshold (but not necessarily crisis)
    #[inline]
    pub fn is_warning_drift(&self) -> bool {
        self.stability_delta < Self::WARNING_THRESHOLD
    }
}

// =============================================================================
// Hook Output (stdout contract)
// Technical Reference: TECH-HOOKS.md Section 2.2, 3.3
// =============================================================================

/// Output returned to Claude Code hook system via stdout
/// Implements REQ-HOOKS-07, REQ-HOOKS-08
///
/// # Required Fields
/// - `success`: boolean (MUST be present)
/// - `execution_time_ms`: u64 (MUST be present)
///
/// # Optional Fields (omitted from JSON when None)
/// - `error`: only present when success=false
/// - `coherence_state`: present when state available
/// - `stability_classification`: present when IC computed
/// - `context_injection`: present when context to inject
///
/// # JSON Schema (TECH-HOOKS.md Section 3.3)
/// ```json
/// {
///   "success": true,
///   "execution_time_ms": 15,
///   "coherence_state": { ... },
///   "stability_classification": { ... }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookOutput {
    /// Whether hook execution succeeded (REQUIRED)
    pub success: bool,
    /// Error message if failed (omit if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Coherence state snapshot (omit if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coherence_state: Option<CoherenceState>,
    /// Topic stability classification (omit if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability_classification: Option<StabilityClassification>,
    /// Content to inject into context (omit if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_injection: Option<String>,
    /// Drift metrics for session identity restoration (omit if None)
    /// Only present when linking to a previous session
    /// Technical Reference: TASK-HOOKS-013
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drift_metrics: Option<DriftMetrics>,
    /// Execution time in milliseconds (REQUIRED)
    pub execution_time_ms: u64,
}

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            success: true,
            error: None,
            coherence_state: None,
            stability_classification: None,
            context_injection: None,
            drift_metrics: None,
            execution_time_ms: 0,
        }
    }
}

impl HookOutput {
    /// Create successful output with execution time
    pub fn success(execution_time_ms: u64) -> Self {
        Self {
            success: true,
            execution_time_ms,
            ..Default::default()
        }
    }

    /// Create error output
    /// Constitution Reference: AP-26 (exit codes)
    pub fn error(message: impl Into<String>, execution_time_ms: u64) -> Self {
        Self {
            success: false,
            error: Some(message.into()),
            execution_time_ms,
            ..Default::default()
        }
    }

    /// Add coherence state to output (builder pattern)
    pub fn with_coherence_state(mut self, state: CoherenceState) -> Self {
        self.coherence_state = Some(state);
        self
    }

    /// Add stability classification to output (builder pattern)
    pub fn with_stability_classification(mut self, classification: StabilityClassification) -> Self {
        self.stability_classification = Some(classification);
        self
    }

    /// Add context injection to output (builder pattern)
    pub fn with_context_injection(mut self, content: impl Into<String>) -> Self {
        self.context_injection = Some(content.into());
        self
    }

    /// Add drift metrics to output (builder pattern)
    /// Technical Reference: TASK-HOOKS-013
    ///
    /// # Arguments
    /// * `metrics` - DriftMetrics computed from session identity comparison
    ///
    /// # Example
    /// ```
    /// use context_graph_cli::commands::hooks::{HookOutput, DriftMetrics};
    ///
    /// let metrics = DriftMetrics {
    ///     stability_delta: -0.05,
    ///     purpose_drift: 0.1,
    ///     time_since_snapshot_ms: 60000,
    ///     coherence_phase_drift: 0.2,
    /// };
    ///
    /// let output = HookOutput::success(50)
    ///     .with_drift_metrics(metrics);
    ///
    /// assert!(output.drift_metrics.is_some());
    /// ```
    pub fn with_drift_metrics(mut self, metrics: DriftMetrics) -> Self {
        self.drift_metrics = Some(metrics);
        self
    }
}

// =============================================================================
// TESTS - NO MOCK DATA - REAL VALUES FROM CONSTITUTION
// =============================================================================

#[cfg(test)]
mod hook_io_tests {
    use super::*;

    // =========================================================================
    // TC-HOOKS-IO-001: StabilityLevel Threshold Boundaries
    // SOURCE OF TRUTH: constitution.yaml topic_stability thresholds
    // =========================================================================
    #[test]
    fn tc_hooks_io_001_ic_level_thresholds() {
        println!("\n=== TC-HOOKS-IO-001: StabilityLevel Threshold Boundaries ===");
        println!("SOURCE: constitution.yaml topic_stability thresholds");

        // Exact boundary tests - these are from constitution
        let boundary_tests = [
            (1.0_f32, StabilityLevel::Healthy, "max value"),
            (0.9_f32, StabilityLevel::Healthy, "healthy boundary (>=0.9)"),
            (0.899_f32, StabilityLevel::Normal, "just below healthy"),
            (0.7_f32, StabilityLevel::Normal, "normal lower boundary"),
            (0.699_f32, StabilityLevel::Warning, "warning boundary (<0.7)"),
            (0.5_f32, StabilityLevel::Warning, "warning lower boundary"),
            (0.499_f32, StabilityLevel::Critical, "critical boundary (<0.5)"),
            (0.0_f32, StabilityLevel::Critical, "min value"),
        ];

        for (value, expected, description) in boundary_tests {
            let actual = StabilityLevel::from_value(value);
            println!(
                "  {} ({}): expected={:?}, actual={:?}",
                description, value, expected, actual
            );
            assert_eq!(
                actual, expected,
                "FAIL: stability={} ({}) must be {:?}, got {:?}",
                value, description, expected, actual
            );
        }

        println!("RESULT: PASS - All stability thresholds match constitution");
    }

    // =========================================================================
    // TC-HOOKS-IO-002: StabilityLevel Serialization
    // SOURCE OF TRUTH: Claude Code hook JSON format (snake_case)
    // =========================================================================
    #[test]
    fn tc_hooks_io_002_stability_level_serialization() {
        println!("\n=== TC-HOOKS-IO-002: StabilityLevel Serialization ===");
        println!("SOURCE: Claude Code hook JSON format");

        let test_cases = [
            (StabilityLevel::Healthy, r#""healthy""#),
            (StabilityLevel::Normal, r#""normal""#),
            (StabilityLevel::Warning, r#""warning""#),
            (StabilityLevel::Critical, r#""critical""#),
        ];

        for (level, expected_json) in test_cases {
            let json =
                serde_json::to_string(&level).expect("serialization MUST succeed - fail fast");
            println!("  {:?} -> {}", level, json);
            assert_eq!(
                json, expected_json,
                "FAIL: {:?} must serialize to {}, got {}",
                level, expected_json, json
            );
        }

        println!("RESULT: PASS - StabilityLevel serializes to snake_case");
    }

    // =========================================================================
    // TC-HOOKS-IO-003: StabilityLevel Deserialization
    // SOURCE OF TRUTH: Claude Code hook JSON format (snake_case)
    // =========================================================================
    #[test]
    fn tc_hooks_io_003_stability_level_deserialization() {
        println!("\n=== TC-HOOKS-IO-003: StabilityLevel Deserialization ===");

        let test_cases = [
            (r#""healthy""#, StabilityLevel::Healthy),
            (r#""normal""#, StabilityLevel::Normal),
            (r#""warning""#, StabilityLevel::Warning),
            (r#""critical""#, StabilityLevel::Critical),
        ];

        for (json, expected) in test_cases {
            let actual: StabilityLevel =
                serde_json::from_str(json).expect("deserialization MUST succeed - fail fast");
            println!("  {} -> {:?}", json, actual);
            assert_eq!(
                actual, expected,
                "FAIL: {} must deserialize to {:?}, got {:?}",
                json, expected, actual
            );
        }

        println!("RESULT: PASS - StabilityLevel deserializes from snake_case");
    }

    // =========================================================================
    // TC-HOOKS-IO-004: StabilityLevel Crisis Detection
    // SOURCE OF TRUTH: constitution.yaml critical threshold <0.5
    // =========================================================================
    #[test]
    fn tc_hooks_io_004_stability_level_crisis() {
        println!("\n=== TC-HOOKS-IO-004: StabilityLevel Crisis Detection ===");
        println!("SOURCE: constitution.yaml critical: \"<0.5\"");

        assert!(StabilityLevel::Critical.is_crisis(), "Critical MUST be crisis");
        assert!(!StabilityLevel::Warning.is_crisis(), "Warning MUST NOT be crisis");
        assert!(!StabilityLevel::Normal.is_crisis(), "Normal MUST NOT be crisis");
        assert!(!StabilityLevel::Healthy.is_crisis(), "Healthy MUST NOT be crisis");

        assert!(
            StabilityLevel::Critical.needs_attention(),
            "Critical needs attention"
        );
        assert!(
            StabilityLevel::Warning.needs_attention(),
            "Warning needs attention"
        );
        assert!(
            !StabilityLevel::Normal.needs_attention(),
            "Normal does NOT need attention"
        );
        assert!(
            !StabilityLevel::Healthy.needs_attention(),
            "Healthy does NOT need attention"
        );

        println!("RESULT: PASS - Crisis detection matches constitution");
    }

    // =========================================================================
    // TC-HOOKS-IO-005: CoherenceState Default
    // SOURCE OF TRUTH: DOR state definition
    // =========================================================================
    #[test]
    fn tc_hooks_io_005_coherence_state_default() {
        println!("\n=== TC-HOOKS-IO-005: CoherenceState Default ===");
        println!("SOURCE: DOR (Disorder of Responsiveness) initial state");

        let state = CoherenceState::default();

        assert_eq!(state.coherence, 0.0, "Default C must be 0.0");
        assert_eq!(state.integration, 0.0, "Default r must be 0.0");
        assert_eq!(state.reflection, 0.0, "Default reflection must be 0.0");
        assert_eq!(
            state.differentiation, 0.0,
            "Default differentiation must be 0.0"
        );
        assert_eq!(
            state.topic_stability, 1.0,
            "Default topic_stability must be 1.0 (fresh)"
        );

        println!("RESULT: PASS - Default state matches DOR definition");
    }

    // =========================================================================
    // TC-HOOKS-IO-006: CoherenceState JSON Round-trip
    // =========================================================================
    #[test]
    fn tc_hooks_io_006_coherence_state_json() {
        println!("\n=== TC-HOOKS-IO-006: CoherenceState JSON Round-trip ===");

        let state = CoherenceState::new(0.73, 0.85, 0.78, 0.82, 0.92);

        let json = serde_json::to_string(&state).expect("serialize");
        println!("  Serialized: {}", json);

        let parsed: CoherenceState = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(state, parsed, "Round-trip MUST preserve all values");

        println!("RESULT: PASS - JSON round-trip preserves all values");
    }

    // =========================================================================
    // TC-HOOKS-IO-009: StabilityClassification Crisis Trigger
    // SOURCE OF TRUTH: constitution.yaml critical: "<0.5"
    // =========================================================================
    #[test]
    fn tc_hooks_io_009_stability_classification_crisis() {
        println!("\n=== TC-HOOKS-IO-009: StabilityClassification Crisis Trigger ===");
        println!("SOURCE: constitution.yaml critical: \"<0.5\"");

        let crisis = StabilityClassification::new(0.45, 0.5);
        assert!(crisis.crisis_triggered, "0.45 < 0.5 MUST trigger crisis");
        assert_eq!(crisis.level, StabilityLevel::Critical, "0.45 MUST be Critical");

        let no_crisis = StabilityClassification::new(0.55, 0.5);
        assert!(
            !no_crisis.crisis_triggered,
            "0.55 >= 0.5 MUST NOT trigger crisis"
        );
        assert_eq!(no_crisis.level, StabilityLevel::Warning, "0.55 MUST be Warning");

        let boundary = StabilityClassification::new(0.5, 0.5);
        assert!(
            !boundary.crisis_triggered,
            "0.5 >= 0.5 MUST NOT trigger crisis"
        );
        assert_eq!(boundary.level, StabilityLevel::Warning, "0.5 MUST be Warning");

        println!("RESULT: PASS - Crisis trigger matches constitution threshold");
    }

    // =========================================================================
    // TC-HOOKS-IO-010: HookInput Validation
    // =========================================================================
    #[test]
    fn tc_hooks_io_010_hook_input_validation() {
        println!("\n=== TC-HOOKS-IO-010: HookInput Validation ===");

        let valid = HookInput {
            hook_type: HookEventType::PreToolUse,
            session_id: "session-123".into(),
            timestamp_ms: 1705312345678,
            payload: HookPayload::PreToolUse {
                tool_name: "Read".into(),
                tool_input: serde_json::json!({"file_path": "/test.txt"}),
                tool_use_id: "tool-use-001".into(),
            },
        };
        assert!(
            valid.validate().is_none(),
            "Valid input MUST pass validation"
        );

        let empty_session = HookInput {
            session_id: "".into(),
            ..valid.clone()
        };
        assert!(
            empty_session.validate().is_some(),
            "Empty session_id MUST fail"
        );

        let bad_timestamp = HookInput {
            timestamp_ms: 0,
            ..valid.clone()
        };
        assert!(
            bad_timestamp.validate().is_some(),
            "Zero timestamp MUST fail"
        );

        println!("RESULT: PASS - Input validation catches invalid data");
    }

    // =========================================================================
    // TC-HOOKS-IO-011: HookOutput Default
    // =========================================================================
    #[test]
    fn tc_hooks_io_011_hook_output_default() {
        println!("\n=== TC-HOOKS-IO-011: HookOutput Default ===");

        let output = HookOutput::default();

        assert!(output.success, "Default output MUST be success=true");
        assert!(output.error.is_none(), "Default MUST have no error");
        assert!(
            output.coherence_state.is_none(),
            "Default MUST have no state"
        );
        assert!(
            output.stability_classification.is_none(),
            "Default MUST have no classification"
        );
        assert!(
            output.context_injection.is_none(),
            "Default MUST have no injection"
        );
        assert_eq!(output.execution_time_ms, 0, "Default time MUST be 0");

        println!("RESULT: PASS - Default output is minimal success");
    }

    // =========================================================================
    // TC-HOOKS-IO-012: HookOutput Builders
    // =========================================================================
    #[test]
    fn tc_hooks_io_012_hook_output_builders() {
        println!("\n=== TC-HOOKS-IO-012: HookOutput Builders ===");

        let output = HookOutput::success(42)
            .with_coherence_state(CoherenceState::default())
            .with_stability_classification(StabilityClassification::from_value(0.85))
            .with_context_injection("test injection");

        assert!(output.success);
        assert_eq!(output.execution_time_ms, 42);
        assert!(output.coherence_state.is_some());
        assert!(output.stability_classification.is_some());
        assert_eq!(output.context_injection, Some("test injection".into()));

        let error = HookOutput::error("test error", 100);
        assert!(!error.success);
        assert_eq!(error.error, Some("test error".into()));
        assert_eq!(error.execution_time_ms, 100);

        println!("RESULT: PASS - Builder pattern works correctly");
    }

    // =========================================================================
    // TC-HOOKS-IO-013: HookOutput JSON Schema Compliance
    // SOURCE OF TRUTH: TECH-HOOKS.md Section 3.3
    // =========================================================================
    #[test]
    fn tc_hooks_io_013_hook_output_json_schema() {
        println!("\n=== TC-HOOKS-IO-013: HookOutput JSON Schema Compliance ===");
        println!("SOURCE: TECH-HOOKS.md Section 3.3");

        let output = HookOutput {
            success: true,
            error: None,
            coherence_state: Some(CoherenceState {
                coherence: 0.73,
                integration: 0.85,
                reflection: 0.78,
                differentiation: 0.82,
                topic_stability: 0.92,
            }),
            stability_classification: Some(StabilityClassification {
                value: 0.92,
                level: StabilityLevel::Healthy,
                crisis_triggered: false,
            }),
            context_injection: None,
            drift_metrics: None,
            execution_time_ms: 15,
        };

        let json = serde_json::to_value(&output).expect("serialize to Value");
        println!("  JSON: {}", serde_json::to_string_pretty(&json).unwrap());

        // Required fields
        assert!(json.get("success").is_some(), "success is REQUIRED");
        assert!(
            json.get("execution_time_ms").is_some(),
            "execution_time_ms is REQUIRED"
        );

        // Optional fields omitted when None
        assert!(
            json.get("error").is_none(),
            "error MUST be omitted when None"
        );
        assert!(
            json.get("context_injection").is_none(),
            "context_injection MUST be omitted when None"
        );

        // Nested structure
        let cs = json
            .get("coherence_state")
            .expect("coherence_state present");
        assert!(cs.get("coherence").is_some());
        assert!(cs.get("integration").is_some());

        let ic = json
            .get("stability_classification")
            .expect("stability_classification present");
        assert!(ic.get("value").is_some());
        assert!(ic.get("level").is_some());
        assert_eq!(ic.get("level").unwrap(), "healthy");

        println!("RESULT: PASS - JSON matches TECH-HOOKS.md schema");
    }

    // =========================================================================
    // TC-HOOKS-IO-014: Invalid Deserialization Fails Fast
    // =========================================================================
    #[test]
    fn tc_hooks_io_014_invalid_deserialization() {
        println!("\n=== TC-HOOKS-IO-014: Invalid Deserialization Fails Fast ===");
        println!("SOURCE: NO BACKWARDS COMPATIBILITY requirement");

        let invalid_inputs = [
            r#""Healthy""#,  // PascalCase StabilityLevel
            r#""CRITICAL""#, // UPPERCASE StabilityLevel
        ];

        for input in invalid_inputs {
            let result_ic: Result<StabilityLevel, _> = serde_json::from_str(input);
            println!("  {} -> StabilityLevel: {:?}", input, result_ic.is_err());
            assert!(
                result_ic.is_err(),
                "FAIL: Invalid input {} should fail deserialization",
                input
            );
        }

        println!("RESULT: PASS - Invalid inputs fail fast");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-001: SessionEndStatus Serialization
    // Implements: REQ-HOOKS-10
    // =========================================================================
    #[test]
    fn tc_hooks_payload_001_session_end_status_serialization() {
        println!("\n=== TC-HOOKS-PAYLOAD-001: SessionEndStatus Serialization ===");

        let statuses = [
            (SessionEndStatus::Normal, "\"normal\""),
            (SessionEndStatus::Timeout, "\"timeout\""),
            (SessionEndStatus::Error, "\"error\""),
            (SessionEndStatus::UserAbort, "\"user_abort\""),
            (SessionEndStatus::Clear, "\"clear\""),
            (SessionEndStatus::Logout, "\"logout\""),
        ];

        for (status, expected_json) in statuses {
            let json = serde_json::to_string(&status).expect("serialize");
            assert_eq!(
                json, expected_json,
                "SessionEndStatus::{:?} MUST serialize to {}",
                status, expected_json
            );
            println!("  {:?} -> {}", status, json);

            // Round-trip
            let deserialized: SessionEndStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, status, "Round-trip MUST preserve value");
        }

        println!("RESULT: PASS - All SessionEndStatus variants serialize correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-002: SessionEndStatus Invalid Deserialization
    // NO BACKWARDS COMPATIBILITY - unknown status MUST fail
    // =========================================================================
    #[test]
    fn tc_hooks_payload_002_session_end_status_invalid() {
        println!("\n=== TC-HOOKS-PAYLOAD-002: SessionEndStatus Invalid Deserialization ===");

        let invalid_inputs = [
            "\"unknown\"",
            "\"NORMAL\"", // Wrong case
            "\"Normal\"", // PascalCase
            "\"abort\"",  // Missing 'user_' prefix
            "\"\"",       // Empty
            "null",       // Null
            "123",        // Number
        ];

        for input in invalid_inputs {
            let result: Result<SessionEndStatus, _> = serde_json::from_str(input);
            assert!(
                result.is_err(),
                "Invalid input {} MUST fail deserialization",
                input
            );
            println!("  {} -> Err (expected)", input);
        }

        println!("RESULT: PASS - Invalid SessionEndStatus fails fast");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-003: ConversationMessage Structure
    // Implements: REQ-HOOKS-11
    // =========================================================================
    #[test]
    fn tc_hooks_payload_003_conversation_message() {
        println!("\n=== TC-HOOKS-PAYLOAD-003: ConversationMessage Structure ===");

        let msg = ConversationMessage {
            role: "user".into(),
            content: "Hello, world!".into(),
        };

        let json = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "Hello, world!");
        println!("  User message: {}", serde_json::to_string(&json).unwrap());

        let assistant_msg = ConversationMessage {
            role: "assistant".into(),
            content: "Hello! How can I help?".into(),
        };

        let json2 = serde_json::to_value(&assistant_msg).expect("serialize");
        assert_eq!(json2["role"], "assistant");
        println!(
            "  Assistant message: {}",
            serde_json::to_string(&json2).unwrap()
        );

        // Round-trip
        let roundtrip: ConversationMessage = serde_json::from_value(json).expect("deserialize");
        assert_eq!(roundtrip.role, "user");
        assert_eq!(roundtrip.content, "Hello, world!");

        println!("RESULT: PASS - ConversationMessage serializes correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-004: HookPayload SessionStart Variant
    // Implements: REQ-HOOKS-10
    // =========================================================================
    #[test]
    fn tc_hooks_payload_004_session_start() {
        println!("\n=== TC-HOOKS-PAYLOAD-004: HookPayload SessionStart ===");

        let payload = HookPayload::SessionStart {
            cwd: "/home/user/project".into(),
            source: "cli".into(),
            previous_session_id: None,
        };

        let json = serde_json::to_value(&payload).expect("serialize");
        println!("  JSON: {}", serde_json::to_string_pretty(&json).unwrap());

        assert_eq!(json["type"], "session_start");
        assert_eq!(json["data"]["cwd"], "/home/user/project");
        assert_eq!(json["data"]["source"], "cli");
        assert!(
            json["data"].get("previous_session_id").is_none(),
            "None field MUST be omitted"
        );

        // With previous session
        let payload_with_prev = HookPayload::SessionStart {
            cwd: "/home/user/project".into(),
            source: "ide".into(),
            previous_session_id: Some("prev-session-123".into()),
        };

        let json2 = serde_json::to_value(&payload_with_prev).expect("serialize");
        assert_eq!(json2["data"]["previous_session_id"], "prev-session-123");

        // Round-trip
        let roundtrip: HookPayload = serde_json::from_value(json).expect("deserialize");
        if let HookPayload::SessionStart {
            cwd,
            source,
            previous_session_id,
        } = roundtrip
        {
            assert_eq!(cwd, "/home/user/project");
            assert_eq!(source, "cli");
            assert!(previous_session_id.is_none());
        } else {
            panic!("Wrong variant after round-trip");
        }

        println!("RESULT: PASS - SessionStart payload serializes correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-005: HookPayload PreToolUse Variant
    // Implements: REQ-HOOKS-10
    // Timeout: 500ms total (fast path per constitution.yaml)
    // =========================================================================
    #[test]
    fn tc_hooks_payload_005_pre_tool_use() {
        println!("\n=== TC-HOOKS-PAYLOAD-005: HookPayload PreToolUse ===");

        let payload = HookPayload::PreToolUse {
            tool_name: "Read".into(),
            tool_input: serde_json::json!({
                "file_path": "/home/user/test.rs",
                "offset": 0
            }),
            tool_use_id: "toolu_01ABC123".into(),
        };

        let json = serde_json::to_value(&payload).expect("serialize");
        println!("  JSON: {}", serde_json::to_string_pretty(&json).unwrap());

        assert_eq!(json["type"], "pre_tool_use");
        assert_eq!(json["data"]["tool_name"], "Read");
        assert_eq!(json["data"]["tool_use_id"], "toolu_01ABC123");
        assert_eq!(
            json["data"]["tool_input"]["file_path"],
            "/home/user/test.rs"
        );

        // Round-trip
        let roundtrip: HookPayload = serde_json::from_value(json).expect("deserialize");
        if let HookPayload::PreToolUse {
            tool_name,
            tool_use_id,
            ..
        } = roundtrip
        {
            assert_eq!(tool_name, "Read");
            assert_eq!(tool_use_id, "toolu_01ABC123");
        } else {
            panic!("Wrong variant after round-trip");
        }

        println!("RESULT: PASS - PreToolUse payload serializes correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-006: HookPayload PostToolUse Variant
    // Implements: REQ-HOOKS-10
    // Timeout: 3000ms per TECH-HOOKS.md
    // =========================================================================
    #[test]
    fn tc_hooks_payload_006_post_tool_use() {
        println!("\n=== TC-HOOKS-PAYLOAD-006: HookPayload PostToolUse ===");

        // Test with explicit tool_success
        let payload = HookPayload::PostToolUse {
            tool_name: "Bash".into(),
            tool_input: serde_json::json!({
                "command": "cargo build"
            }),
            tool_response: "Compiling context-graph v0.1.0\nFinished release".into(),
            tool_use_id: "toolu_02DEF456".into(),
            tool_success: Some(true),
        };

        let json = serde_json::to_value(&payload).expect("serialize");
        println!("  JSON: {}", serde_json::to_string_pretty(&json).unwrap());

        assert_eq!(json["type"], "post_tool_use");
        assert_eq!(json["data"]["tool_name"], "Bash");
        assert_eq!(json["data"]["tool_use_id"], "toolu_02DEF456");
        assert_eq!(json["data"]["tool_success"], true);
        assert!(json["data"]["tool_response"]
            .as_str()
            .unwrap()
            .contains("Compiling"));

        // Round-trip
        let roundtrip: HookPayload = serde_json::from_value(json).expect("deserialize");
        if let HookPayload::PostToolUse {
            tool_name,
            tool_response,
            tool_success,
            ..
        } = roundtrip
        {
            assert_eq!(tool_name, "Bash");
            assert!(tool_response.contains("Compiling"));
            assert_eq!(tool_success, Some(true));
        } else {
            panic!("Wrong variant after round-trip");
        }

        // Test without tool_success (should default to None)
        let json_without_success = serde_json::json!({
            "type": "post_tool_use",
            "data": {
                "tool_name": "Read",
                "tool_input": {"file_path": "/tmp/test.rs"},
                "tool_response": "use sqlx::Error;\nfn main() {}",
                "tool_use_id": "toolu_03ABC789"
            }
        });

        let deserialized: HookPayload =
            serde_json::from_value(json_without_success).expect("deserialize without tool_success");
        if let HookPayload::PostToolUse { tool_success, .. } = deserialized {
            assert_eq!(tool_success, None, "tool_success should default to None");
        } else {
            panic!("Wrong variant");
        }

        println!("RESULT: PASS - PostToolUse payload serializes correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-007: HookPayload UserPromptSubmit Variant
    // Implements: REQ-HOOKS-10, REQ-HOOKS-11
    // Timeout: 2000ms per constitution.yaml
    // =========================================================================
    #[test]
    fn tc_hooks_payload_007_user_prompt_submit() {
        println!("\n=== TC-HOOKS-PAYLOAD-007: HookPayload UserPromptSubmit ===");

        let payload = HookPayload::UserPromptSubmit {
            prompt: "Help me fix this bug".into(),
            context: vec![
                ConversationMessage {
                    role: "user".into(),
                    content: "There's an error in line 42".into(),
                },
                ConversationMessage {
                    role: "assistant".into(),
                    content: "I see the issue. Let me check...".into(),
                },
            ],
        };

        let json = serde_json::to_value(&payload).expect("serialize");
        println!("  JSON: {}", serde_json::to_string_pretty(&json).unwrap());

        assert_eq!(json["type"], "user_prompt_submit");
        assert_eq!(json["data"]["prompt"], "Help me fix this bug");
        assert_eq!(json["data"]["context"].as_array().unwrap().len(), 2);
        assert_eq!(json["data"]["context"][0]["role"], "user");
        assert_eq!(json["data"]["context"][1]["role"], "assistant");

        // Empty context (default)
        let payload_empty = HookPayload::UserPromptSubmit {
            prompt: "Hello".into(),
            context: vec![],
        };
        let json_empty = serde_json::to_value(&payload_empty).expect("serialize");
        assert!(json_empty["data"]["context"].as_array().unwrap().is_empty());

        // Round-trip
        let roundtrip: HookPayload = serde_json::from_value(json).expect("deserialize");
        if let HookPayload::UserPromptSubmit { prompt, context } = roundtrip {
            assert_eq!(prompt, "Help me fix this bug");
            assert_eq!(context.len(), 2);
        } else {
            panic!("Wrong variant after round-trip");
        }

        println!("RESULT: PASS - UserPromptSubmit payload serializes correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-008: HookPayload SessionEnd Variant
    // Implements: REQ-HOOKS-10
    // Timeout: 30000ms per constitution.yaml
    // =========================================================================
    #[test]
    fn tc_hooks_payload_008_session_end() {
        println!("\n=== TC-HOOKS-PAYLOAD-008: HookPayload SessionEnd ===");

        let payload = HookPayload::SessionEnd {
            duration_ms: 3600000,
            status: SessionEndStatus::Normal,
            reason: None,
        };

        let json = serde_json::to_value(&payload).expect("serialize");
        println!("  JSON: {}", serde_json::to_string_pretty(&json).unwrap());

        assert_eq!(json["type"], "session_end");
        assert_eq!(json["data"]["duration_ms"], 3600000);
        assert_eq!(json["data"]["status"], "normal");
        assert!(
            json["data"].get("reason").is_none(),
            "None reason MUST be omitted"
        );

        // With reason
        let payload_with_reason = HookPayload::SessionEnd {
            duration_ms: 120000,
            status: SessionEndStatus::Error,
            reason: Some("Connection lost".into()),
        };
        let json2 = serde_json::to_value(&payload_with_reason).expect("serialize");
        assert_eq!(json2["data"]["status"], "error");
        assert_eq!(json2["data"]["reason"], "Connection lost");

        // Round-trip
        let roundtrip: HookPayload = serde_json::from_value(json).expect("deserialize");
        if let HookPayload::SessionEnd {
            duration_ms,
            status,
            reason,
        } = roundtrip
        {
            assert_eq!(duration_ms, 3600000);
            assert_eq!(status, SessionEndStatus::Normal);
            assert!(reason.is_none());
        } else {
            panic!("Wrong variant after round-trip");
        }

        println!("RESULT: PASS - SessionEnd payload serializes correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-009: HookPayload Invalid Deserialization
    // NO BACKWARDS COMPATIBILITY - unknown types MUST fail
    // =========================================================================
    #[test]
    fn tc_hooks_payload_009_invalid_deserialization() {
        println!("\n=== TC-HOOKS-PAYLOAD-009: HookPayload Invalid Deserialization ===");

        let invalid_inputs = [
            r#"{"type": "unknown_event", "data": {}}"#,
            r#"{"type": "SessionStart", "data": {"cwd": "/"}}"#, // PascalCase
            r#"{"type": "PRE_TOOL_USE", "data": {}}"#,           // UPPERCASE
            r#"{"data": {"cwd": "/"}}"#,                         // Missing type
            r#"{"type": "session_start"}"#,                      // Missing data
            r#"null"#,
            r#"[]"#,
        ];

        for input in invalid_inputs {
            let result: Result<HookPayload, _> = serde_json::from_str(input);
            assert!(
                result.is_err(),
                "Invalid input {} MUST fail deserialization",
                input
            );
            println!("  {} -> Err (expected)", input);
        }

        println!("RESULT: PASS - Invalid HookPayload fails fast");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-010: HookInput with Typed Payload Integration
    // Implements: REQ-HOOKS-10, REQ-HOOKS-11, REQ-HOOKS-12
    // =========================================================================
    #[test]
    fn tc_hooks_payload_010_hook_input_integration() {
        println!("\n=== TC-HOOKS-PAYLOAD-010: HookInput with Typed Payload Integration ===");

        let input = HookInput {
            hook_type: HookEventType::SessionStart,
            session_id: "session-abc123".into(),
            timestamp_ms: 1705312345678,
            payload: HookPayload::SessionStart {
                cwd: "/home/user/project".into(),
                source: "cli".into(),
                previous_session_id: None,
            },
        };

        let json = serde_json::to_value(&input).expect("serialize");
        println!("  JSON: {}", serde_json::to_string_pretty(&json).unwrap());

        assert_eq!(json["hook_type"], "session_start");
        assert_eq!(json["session_id"], "session-abc123");
        assert_eq!(json["payload"]["type"], "session_start");
        assert_eq!(json["payload"]["data"]["cwd"], "/home/user/project");

        // Round-trip
        let roundtrip: HookInput = serde_json::from_value(json).expect("deserialize");
        assert_eq!(roundtrip.hook_type, HookEventType::SessionStart);
        assert_eq!(roundtrip.session_id, "session-abc123");
        assert!(
            roundtrip.validate().is_none(),
            "Valid input MUST pass validation"
        );

        println!("RESULT: PASS - HookInput integrates with typed payload");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-011: Edge Case - Empty Strings
    // Boundary condition testing
    // =========================================================================
    #[test]
    fn tc_hooks_payload_011_edge_case_empty_strings() {
        println!("\n=== TC-HOOKS-PAYLOAD-011: Edge Case - Empty Strings ===");

        // Empty cwd is technically valid (serialization perspective)
        let payload = HookPayload::SessionStart {
            cwd: "".into(),
            source: "".into(),
            previous_session_id: Some("".into()),
        };

        let json = serde_json::to_value(&payload).expect("serialize empty strings");
        assert_eq!(json["data"]["cwd"], "");
        assert_eq!(json["data"]["source"], "");
        assert_eq!(json["data"]["previous_session_id"], "");

        // Round-trip preserves empty strings
        let roundtrip: HookPayload = serde_json::from_value(json).expect("deserialize");
        if let HookPayload::SessionStart {
            cwd,
            source,
            previous_session_id,
        } = roundtrip
        {
            assert_eq!(cwd, "");
            assert_eq!(source, "");
            assert_eq!(previous_session_id, Some("".into()));
        } else {
            panic!("Wrong variant");
        }

        println!("RESULT: PASS - Empty strings handled correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-012: Edge Case - Large Values
    // Boundary condition testing
    // =========================================================================
    #[test]
    fn tc_hooks_payload_012_edge_case_large_values() {
        println!("\n=== TC-HOOKS-PAYLOAD-012: Edge Case - Large Values ===");

        // Large duration_ms (max u64)
        let payload = HookPayload::SessionEnd {
            duration_ms: u64::MAX,
            status: SessionEndStatus::Normal,
            reason: None,
        };

        let json = serde_json::to_value(&payload).expect("serialize large duration");
        assert_eq!(json["data"]["duration_ms"], u64::MAX);

        // Large prompt
        let large_prompt = "x".repeat(100_000);
        let payload2 = HookPayload::UserPromptSubmit {
            prompt: large_prompt.clone(),
            context: vec![],
        };

        let json2 = serde_json::to_value(&payload2).expect("serialize large prompt");
        assert_eq!(json2["data"]["prompt"].as_str().unwrap().len(), 100_000);

        // Round-trip
        let roundtrip: HookPayload = serde_json::from_value(json2).expect("deserialize");
        if let HookPayload::UserPromptSubmit { prompt, .. } = roundtrip {
            assert_eq!(prompt.len(), 100_000);
        } else {
            panic!("Wrong variant");
        }

        println!("RESULT: PASS - Large values handled correctly");
    }

    // =========================================================================
    // TC-HOOKS-PAYLOAD-013: Edge Case - Unicode Content
    // Boundary condition testing
    // =========================================================================
    #[test]
    fn tc_hooks_payload_013_edge_case_unicode() {
        println!("\n=== TC-HOOKS-PAYLOAD-013: Edge Case - Unicode Content ===");

        let payload = HookPayload::UserPromptSubmit {
            prompt: "Hello \u{1F600} World \u{4E2D}\u{6587} \u{0391}\u{03B2}\u{03B3}".into(),
            context: vec![ConversationMessage {
                role: "user".into(),
                content: "\u{1F389} Party! \u{1F3C6}".into(),
            }],
        };

        let json = serde_json::to_value(&payload).expect("serialize unicode");
        let json_str = serde_json::to_string(&json).expect("to string");
        println!("  JSON: {}", json_str);

        // Round-trip
        let roundtrip: HookPayload = serde_json::from_value(json).expect("deserialize");
        if let HookPayload::UserPromptSubmit { prompt, context } = roundtrip {
            assert!(prompt.contains("\u{1F600}"));
            assert!(prompt.contains("\u{4E2D}\u{6587}"));
            assert_eq!(context[0].content, "\u{1F389} Party! \u{1F3C6}");
        } else {
            panic!("Wrong variant");
        }

        println!("RESULT: PASS - Unicode content preserved correctly");
    }
}
