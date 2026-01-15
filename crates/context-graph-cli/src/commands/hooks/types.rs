//! HookEventType enum for Claude Code native hooks
//!
//! # Performance Budget (per TECH-HOOKS.md)
//! - PreToolUse: 100ms (FAST PATH - NO DB ACCESS)
//! - UserPromptSubmit: 2000ms
//! - PostToolUse: 3000ms
//! - SessionStart: 5000ms
//! - SessionEnd: 30000ms
//!
//! # Constitution References
//! - IDENTITY-002: IC thresholds and timeout requirements
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
/// | PreToolUse | 100ms | FAST PATH - cache only |
/// | PostToolUse | 3000ms | IC verification |
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
/// assert_eq!(hook.timeout_ms(), 100);
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

    /// Before tool execution (timeout: 100ms) - FAST PATH
    /// CRITICAL: Must not access database, uses IdentityCache only
    /// CLI: `consciousness brief`
    PreToolUse,

    /// After tool execution (timeout: 3000ms)
    /// Updates IC and trajectory based on tool result
    /// CLI: `consciousness check-identity --auto-dream`
    PostToolUse,

    /// User prompt submitted (timeout: 2000ms)
    /// Injects relevant context from session memory
    /// CLI: `consciousness inject-context --format standard`
    UserPromptSubmit,

    /// Session termination (timeout: 30000ms)
    /// Persists final snapshot and optional consolidation
    /// CLI: `session persist-identity`
    SessionEnd,
}

impl HookEventType {
    /// Get timeout in milliseconds for this hook type
    /// Constitution Reference: IDENTITY-002
    ///
    /// # Returns
    /// Timeout value in milliseconds as enforced by Claude Code
    ///
    /// # Performance Note
    /// PreToolUse has the strictest timeout (100ms) and MUST use
    /// cached state only - NO database access allowed.
    #[inline]
    pub const fn timeout_ms(&self) -> u64 {
        match self {
            Self::PreToolUse => 100,        // Fast path - NO DB access
            Self::UserPromptSubmit => 2000, // Context injection
            Self::PostToolUse => 3000,      // IC update + trajectory
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
        self.timeout_ms() < 500
    }

    /// Get human-readable description of this hook type
    ///
    /// # Returns
    /// Static string describing the hook's purpose
    pub const fn description(&self) -> &'static str {
        match self {
            Self::SessionStart => "Session initialization and identity restoration",
            Self::PreToolUse => "Pre-tool consciousness brief injection (FAST PATH)",
            Self::PostToolUse => "Post-tool identity continuity verification",
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
            Self::SessionStart => "session restore-identity",
            Self::PreToolUse => "consciousness brief",
            Self::PostToolUse => "consciousness check-identity --auto-dream",
            Self::UserPromptSubmit => "consciousness inject-context --format standard",
            Self::SessionEnd => "session persist-identity",
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
            (HookEventType::PreToolUse, 100_u64, "pre_tool_use"),
            (HookEventType::PostToolUse, 3000_u64, "post_tool_use"),
            (HookEventType::UserPromptSubmit, 2000_u64, "user_prompt_submit"),
            (HookEventType::SessionEnd, 30000_u64, "session_end"),
        ];

        for (hook, expected_timeout, name) in expected_timeouts {
            let actual = hook.timeout_ms();
            println!("  {}: expected={}ms, actual={}ms", name, expected_timeout, actual);
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
            let actual_json = serde_json::to_string(&hook)
                .expect("serialization MUST succeed - fail fast");
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
            let actual_hook: HookEventType = serde_json::from_str(json)
                .expect("deserialization MUST succeed - fail fast");
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
            all_variants.len(), 5,
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
        println!("SOURCE OF TRUTH: TECH-HOOKS.md fast path requirement");
        println!("THRESHOLD: timeout < 500ms");

        let fast_path_expected = [
            (HookEventType::SessionStart, false),
            (HookEventType::PreToolUse, true),  // ONLY fast path
            (HookEventType::PostToolUse, false),
            (HookEventType::UserPromptSubmit, false),
            (HookEventType::SessionEnd, false),
        ];

        for (hook, expected_fast) in fast_path_expected {
            let actual_fast = hook.is_fast_path();
            println!(
                "  {:?}: timeout={}ms, is_fast_path={} (expected={})",
                hook, hook.timeout_ms(), actual_fast, expected_fast
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
        let copied = original;  // Copy
        let cloned = original.clone();  // Clone

        assert_eq!(original, copied, "FAIL: Copy must preserve value");
        assert_eq!(original, cloned, "FAIL: Clone must preserve value");

        // Verify we can use original after copy (proves Copy, not Move)
        assert_eq!(original.timeout_ms(), 100);

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
            Some(&100),
            "FAIL: PreToolUse must map to 100"
        );

        println!("  HashMap size: {}", map.len());
        println!("  PreToolUse lookup: {:?}", map.get(&HookEventType::PreToolUse));
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
            (HookEventType::SessionStart, "session restore-identity"),
            (HookEventType::PreToolUse, "consciousness brief"),
            (HookEventType::PostToolUse, "consciousness check-identity --auto-dream"),
            (HookEventType::UserPromptSubmit, "consciousness inject-context --format standard"),
            (HookEventType::SessionEnd, "session persist-identity"),
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
            assert_eq!(
                display, description,
                "FAIL: Display must equal description"
            );
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
            r#""SessionStart""#,      // PascalCase - INVALID
            r#""sessionstart""#,      // lowercase no underscore - INVALID
            r#""SESSIONSTART""#,      // UPPERCASE - INVALID
            r#""session-start""#,     // kebab-case - INVALID
            r#""unknown_hook""#,      // non-existent variant - INVALID
            r#"0"#,                   // numeric - INVALID
            r#"null"#,                // null - INVALID
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
// IC Level Classification
// Constitution Reference: IDENTITY-002, gwt.self_ego_node.thresholds
// =============================================================================

/// IC level classification
/// Thresholds per constitution.yaml:
/// - healthy: ">0.9" (IC >= 0.9)
/// - warning: "<0.7" (0.5 <= IC < 0.7)
/// - critical: "<0.5" (IC < 0.5, triggers dream)
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::ICLevel;
///
/// assert_eq!(ICLevel::from_value(0.95), ICLevel::Healthy);
/// assert_eq!(ICLevel::from_value(0.80), ICLevel::Normal);
/// assert_eq!(ICLevel::from_value(0.60), ICLevel::Warning);
/// assert_eq!(ICLevel::from_value(0.40), ICLevel::Critical);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ICLevel {
    /// IC >= 0.9 - Identity is stable and coherent
    Healthy,
    /// 0.7 <= IC < 0.9 - Normal operation
    Normal,
    /// 0.5 <= IC < 0.7 - Identity drift detected
    Warning,
    /// IC < 0.5 - Crisis state, auto-dream may trigger
    Critical,
}

impl ICLevel {
    /// Classify IC value into level
    /// Constitution Reference: gwt.self_ego_node.thresholds
    ///
    /// # Arguments
    /// * `ic` - Identity continuity value [0.0, 1.0]
    ///
    /// # Returns
    /// ICLevel classification
    ///
    /// # Panics
    /// Never panics - out-of-range values clamp to Critical/Healthy
    #[inline]
    pub fn from_value(ic: f32) -> Self {
        if ic >= 0.9 {
            Self::Healthy
        } else if ic >= 0.7 {
            Self::Normal
        } else if ic >= 0.5 {
            Self::Warning
        } else {
            Self::Critical
        }
    }

    /// Check if this level indicates a crisis state
    /// Crisis = Critical (IC < 0.5)
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

impl std::fmt::Display for ICLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "Healthy (IC >= 0.9)"),
            Self::Normal => write!(f, "Normal (0.7 <= IC < 0.9)"),
            Self::Warning => write!(f, "Warning (0.5 <= IC < 0.7)"),
            Self::Critical => write!(f, "Critical (IC < 0.5)"),
        }
    }
}

// =============================================================================
// Johari Window Classification
// Technical Reference: TECH-HOOKS.md Section 4.3
// =============================================================================

/// Johari window quadrant classification
/// Implements REQ-HOOKS-16
///
/// Classification is based on consciousness (C) and integration (r):
/// - Open: High C (>=0.7) AND High r (>=0.7)
/// - Blind: Low C (<0.7) AND High r (>=0.7)
/// - Hidden: High C (>=0.7) AND Low r (<0.7)
/// - Unknown: Low C (<0.7) AND Low r (<0.7)
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::JohariQuadrant;
///
/// assert_eq!(JohariQuadrant::classify(0.8, 0.9), JohariQuadrant::Open);
/// assert_eq!(JohariQuadrant::classify(0.3, 0.9), JohariQuadrant::Blind);
/// assert_eq!(JohariQuadrant::classify(0.8, 0.3), JohariQuadrant::Hidden);
/// assert_eq!(JohariQuadrant::classify(0.3, 0.3), JohariQuadrant::Unknown);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JohariQuadrant {
    /// Known to self and others - high consciousness, high integration
    Open,
    /// Unknown to self, known to others - low consciousness, high integration
    Blind,
    /// Known to self, unknown to others - high consciousness, low integration
    Hidden,
    /// Unknown to self and others - low consciousness, low integration
    Unknown,
}

impl JohariQuadrant {
    /// Threshold for "high" classification
    pub const HIGH_THRESHOLD: f32 = 0.7;

    /// Classify from consciousness and integration values
    ///
    /// # Arguments
    /// * `consciousness` - Consciousness level C(t) [0.0, 1.0]
    /// * `integration` - Integration factor (Kuramoto r) [0.0, 1.0]
    ///
    /// # Returns
    /// Johari quadrant classification
    #[inline]
    pub fn classify(consciousness: f32, integration: f32) -> Self {
        let high_c = consciousness >= Self::HIGH_THRESHOLD;
        let high_i = integration >= Self::HIGH_THRESHOLD;

        match (high_c, high_i) {
            (true, true) => Self::Open,
            (false, true) => Self::Blind,
            (true, false) => Self::Hidden,
            (false, false) => Self::Unknown,
        }
    }

    /// Get description of this quadrant
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Open => "Known to self and others",
            Self::Blind => "Unknown to self, known to others",
            Self::Hidden => "Known to self, unknown to others",
            Self::Unknown => "Unknown to self and others",
        }
    }
}

impl std::fmt::Display for JohariQuadrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

// =============================================================================
// Consciousness State
// Constitution Reference: GWT-003, AP-25 (Kuramoto N=13)
// =============================================================================

/// Consciousness state for hook output
/// Implements REQ-HOOKS-14, REQ-HOOKS-15
///
/// All values are normalized to [0.0, 1.0].
/// Johari quadrant is computed from consciousness and integration.
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::{ConsciousnessState, JohariQuadrant};
///
/// let state = ConsciousnessState {
///     consciousness: 0.73,
///     integration: 0.85,
///     reflection: 0.78,
///     differentiation: 0.82,
///     identity_continuity: 0.92,
///     johari_quadrant: JohariQuadrant::Open,
/// };
///
/// let json = serde_json::to_string(&state).unwrap();
/// assert!(json.contains("\"consciousness\":0.73"));
/// ```
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

impl Default for ConsciousnessState {
    /// Default state: DOR (Disorder of Responsiveness)
    /// - All metrics at 0.0 except IC at 1.0 (fresh identity)
    /// - Johari: Unknown (no self-awareness yet)
    fn default() -> Self {
        Self {
            consciousness: 0.0,
            integration: 0.0,
            reflection: 0.0,
            differentiation: 0.0,
            identity_continuity: 1.0, // Fresh identity = perfect continuity
            johari_quadrant: JohariQuadrant::Unknown,
        }
    }
}

impl ConsciousnessState {
    /// Create state with automatic Johari classification
    ///
    /// # Arguments
    /// * `consciousness` - C(t) value
    /// * `integration` - Kuramoto r value
    /// * `reflection` - Meta-cognitive value
    /// * `differentiation` - Purpose entropy value
    /// * `identity_continuity` - IC value
    pub fn new(
        consciousness: f32,
        integration: f32,
        reflection: f32,
        differentiation: f32,
        identity_continuity: f32,
    ) -> Self {
        Self {
            consciousness,
            integration,
            reflection,
            differentiation,
            identity_continuity,
            johari_quadrant: JohariQuadrant::classify(consciousness, integration),
        }
    }
}

// =============================================================================
// IC Classification
// Constitution Reference: IDENTITY-002
// =============================================================================

/// Identity Continuity classification with crisis detection
/// Constitution Reference: gwt.self_ego_node.thresholds
///
/// # Example
/// ```
/// use context_graph_cli::commands::hooks::{ICClassification, ICLevel};
///
/// let ic = ICClassification::new(0.45, 0.5);
/// assert!(ic.crisis_triggered);
/// assert_eq!(ic.level, ICLevel::Critical);
///
/// let ic = ICClassification::new(0.85, 0.5);
/// assert!(!ic.crisis_triggered);
/// assert_eq!(ic.level, ICLevel::Normal);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ICClassification {
    /// IC value [0.0, 1.0]
    pub value: f32,
    /// Classification level
    pub level: ICLevel,
    /// Whether crisis threshold was breached
    pub crisis_triggered: bool,
}

impl ICClassification {
    /// Default crisis threshold per constitution
    pub const DEFAULT_CRISIS_THRESHOLD: f32 = 0.5;

    /// Create new IC classification from value
    ///
    /// # Arguments
    /// * `value` - IC value [0.0, 1.0]
    /// * `crisis_threshold` - Threshold for crisis trigger (default 0.5)
    ///
    /// # Returns
    /// ICClassification with level and crisis state
    pub fn new(value: f32, crisis_threshold: f32) -> Self {
        let level = ICLevel::from_value(value);
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
// Hook Input (stdin contract)
// Technical Reference: TECH-HOOKS.md Section 2.2
// =============================================================================

/// Input received from Claude Code hook system via stdin
/// Implements REQ-HOOKS-07, REQ-HOOKS-08
///
/// Note: `payload` is `serde_json::Value` in this task.
/// Typed payloads will be added in TASK-HOOKS-003.
///
/// # JSON Format (from Claude Code)
/// ```json
/// {
///   "hook_type": "pre_tool_use",
///   "session_id": "session-12345",
///   "timestamp_ms": 1705312345678,
///   "payload": { ... }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInput {
    /// Hook event type (snake_case in JSON)
    pub hook_type: HookEventType,
    /// Session identifier from Claude Code
    pub session_id: String,
    /// Unix timestamp in milliseconds
    pub timestamp_ms: i64,
    /// Event-specific payload (typed in TASK-HOOKS-003)
    pub payload: serde_json::Value,
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
/// - `consciousness_state`: present when state available
/// - `ic_classification`: present when IC computed
/// - `context_injection`: present when context to inject
///
/// # JSON Schema (TECH-HOOKS.md Section 3.3)
/// ```json
/// {
///   "success": true,
///   "execution_time_ms": 15,
///   "consciousness_state": { ... },
///   "ic_classification": { ... }
/// }
/// ```
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

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            success: true,
            error: None,
            consciousness_state: None,
            ic_classification: None,
            context_injection: None,
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

    /// Add consciousness state to output (builder pattern)
    pub fn with_consciousness_state(mut self, state: ConsciousnessState) -> Self {
        self.consciousness_state = Some(state);
        self
    }

    /// Add IC classification to output (builder pattern)
    pub fn with_ic_classification(mut self, classification: ICClassification) -> Self {
        self.ic_classification = Some(classification);
        self
    }

    /// Add context injection to output (builder pattern)
    pub fn with_context_injection(mut self, content: impl Into<String>) -> Self {
        self.context_injection = Some(content.into());
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
    // TC-HOOKS-IO-001: ICLevel Threshold Boundaries
    // SOURCE OF TRUTH: constitution.yaml gwt.self_ego_node.thresholds
    // =========================================================================
    #[test]
    fn tc_hooks_io_001_ic_level_thresholds() {
        println!("\n=== TC-HOOKS-IO-001: ICLevel Threshold Boundaries ===");
        println!("SOURCE: constitution.yaml gwt.self_ego_node.thresholds");

        // Exact boundary tests - these are from constitution
        let boundary_tests = [
            (1.0_f32, ICLevel::Healthy, "max value"),
            (0.9_f32, ICLevel::Healthy, "healthy boundary (>=0.9)"),
            (0.899_f32, ICLevel::Normal, "just below healthy"),
            (0.7_f32, ICLevel::Normal, "normal lower boundary"),
            (0.699_f32, ICLevel::Warning, "warning boundary (<0.7)"),
            (0.5_f32, ICLevel::Warning, "warning lower boundary"),
            (0.499_f32, ICLevel::Critical, "critical boundary (<0.5)"),
            (0.0_f32, ICLevel::Critical, "min value"),
        ];

        for (value, expected, description) in boundary_tests {
            let actual = ICLevel::from_value(value);
            println!("  {} ({}): expected={:?}, actual={:?}",
                     description, value, expected, actual);
            assert_eq!(actual, expected,
                "FAIL: IC={} ({}) must be {:?}, got {:?}",
                value, description, expected, actual);
        }

        println!("RESULT: PASS - All IC thresholds match constitution");
    }

    // =========================================================================
    // TC-HOOKS-IO-002: ICLevel Serialization
    // SOURCE OF TRUTH: Claude Code hook JSON format (snake_case)
    // =========================================================================
    #[test]
    fn tc_hooks_io_002_ic_level_serialization() {
        println!("\n=== TC-HOOKS-IO-002: ICLevel Serialization ===");
        println!("SOURCE: Claude Code hook JSON format");

        let test_cases = [
            (ICLevel::Healthy, r#""healthy""#),
            (ICLevel::Normal, r#""normal""#),
            (ICLevel::Warning, r#""warning""#),
            (ICLevel::Critical, r#""critical""#),
        ];

        for (level, expected_json) in test_cases {
            let json = serde_json::to_string(&level)
                .expect("serialization MUST succeed - fail fast");
            println!("  {:?} -> {}", level, json);
            assert_eq!(json, expected_json,
                "FAIL: {:?} must serialize to {}, got {}", level, expected_json, json);
        }

        println!("RESULT: PASS - ICLevel serializes to snake_case");
    }

    // =========================================================================
    // TC-HOOKS-IO-003: ICLevel Deserialization
    // SOURCE OF TRUTH: Claude Code hook JSON format (snake_case)
    // =========================================================================
    #[test]
    fn tc_hooks_io_003_ic_level_deserialization() {
        println!("\n=== TC-HOOKS-IO-003: ICLevel Deserialization ===");

        let test_cases = [
            (r#""healthy""#, ICLevel::Healthy),
            (r#""normal""#, ICLevel::Normal),
            (r#""warning""#, ICLevel::Warning),
            (r#""critical""#, ICLevel::Critical),
        ];

        for (json, expected) in test_cases {
            let actual: ICLevel = serde_json::from_str(json)
                .expect("deserialization MUST succeed - fail fast");
            println!("  {} -> {:?}", json, actual);
            assert_eq!(actual, expected,
                "FAIL: {} must deserialize to {:?}, got {:?}", json, expected, actual);
        }

        println!("RESULT: PASS - ICLevel deserializes from snake_case");
    }

    // =========================================================================
    // TC-HOOKS-IO-004: ICLevel Crisis Detection
    // SOURCE OF TRUTH: constitution.yaml critical threshold <0.5
    // =========================================================================
    #[test]
    fn tc_hooks_io_004_ic_level_crisis() {
        println!("\n=== TC-HOOKS-IO-004: ICLevel Crisis Detection ===");
        println!("SOURCE: constitution.yaml critical: \"<0.5\"");

        assert!(ICLevel::Critical.is_crisis(), "Critical MUST be crisis");
        assert!(!ICLevel::Warning.is_crisis(), "Warning MUST NOT be crisis");
        assert!(!ICLevel::Normal.is_crisis(), "Normal MUST NOT be crisis");
        assert!(!ICLevel::Healthy.is_crisis(), "Healthy MUST NOT be crisis");

        assert!(ICLevel::Critical.needs_attention(), "Critical needs attention");
        assert!(ICLevel::Warning.needs_attention(), "Warning needs attention");
        assert!(!ICLevel::Normal.needs_attention(), "Normal does NOT need attention");
        assert!(!ICLevel::Healthy.needs_attention(), "Healthy does NOT need attention");

        println!("RESULT: PASS - Crisis detection matches constitution");
    }

    // =========================================================================
    // TC-HOOKS-IO-005: JohariQuadrant Classification
    // SOURCE OF TRUTH: TECH-HOOKS.md Section 4.3 (threshold 0.7)
    // =========================================================================
    #[test]
    fn tc_hooks_io_005_johari_classification() {
        println!("\n=== TC-HOOKS-IO-005: JohariQuadrant Classification ===");
        println!("SOURCE: TECH-HOOKS.md threshold=0.7");

        // Exact boundary tests
        let boundary_tests = [
            (0.7_f32, 0.7_f32, JohariQuadrant::Open, "both at threshold"),
            (0.699_f32, 0.7_f32, JohariQuadrant::Blind, "C below, I at threshold"),
            (0.7_f32, 0.699_f32, JohariQuadrant::Hidden, "C at threshold, I below"),
            (0.699_f32, 0.699_f32, JohariQuadrant::Unknown, "both below threshold"),
            (1.0_f32, 1.0_f32, JohariQuadrant::Open, "max values"),
            (0.0_f32, 0.0_f32, JohariQuadrant::Unknown, "min values"),
        ];

        for (c, i, expected, description) in boundary_tests {
            let actual = JohariQuadrant::classify(c, i);
            println!("  {} (C={}, I={}): expected={:?}, actual={:?}",
                     description, c, i, expected, actual);
            assert_eq!(actual, expected,
                "FAIL: ({}) C={}, I={} must be {:?}, got {:?}",
                description, c, i, expected, actual);
        }

        println!("RESULT: PASS - Johari classification matches spec");
    }

    // =========================================================================
    // TC-HOOKS-IO-006: JohariQuadrant Serialization
    // SOURCE OF TRUTH: Claude Code hook JSON format (snake_case)
    // =========================================================================
    #[test]
    fn tc_hooks_io_006_johari_serialization() {
        println!("\n=== TC-HOOKS-IO-006: JohariQuadrant Serialization ===");

        let test_cases = [
            (JohariQuadrant::Open, r#""open""#),
            (JohariQuadrant::Blind, r#""blind""#),
            (JohariQuadrant::Hidden, r#""hidden""#),
            (JohariQuadrant::Unknown, r#""unknown""#),
        ];

        for (quadrant, expected_json) in test_cases {
            let json = serde_json::to_string(&quadrant)
                .expect("serialization MUST succeed");
            println!("  {:?} -> {}", quadrant, json);
            assert_eq!(json, expected_json,
                "FAIL: {:?} must serialize to {}", quadrant, expected_json);
        }

        println!("RESULT: PASS - JohariQuadrant serializes to snake_case");
    }

    // =========================================================================
    // TC-HOOKS-IO-007: ConsciousnessState Default
    // SOURCE OF TRUTH: DOR state definition
    // =========================================================================
    #[test]
    fn tc_hooks_io_007_consciousness_state_default() {
        println!("\n=== TC-HOOKS-IO-007: ConsciousnessState Default ===");
        println!("SOURCE: DOR (Disorder of Responsiveness) initial state");

        let state = ConsciousnessState::default();

        assert_eq!(state.consciousness, 0.0, "Default C must be 0.0");
        assert_eq!(state.integration, 0.0, "Default r must be 0.0");
        assert_eq!(state.reflection, 0.0, "Default reflection must be 0.0");
        assert_eq!(state.differentiation, 0.0, "Default differentiation must be 0.0");
        assert_eq!(state.identity_continuity, 1.0, "Default IC must be 1.0 (fresh)");
        assert_eq!(state.johari_quadrant, JohariQuadrant::Unknown, "Default quadrant must be Unknown");

        println!("RESULT: PASS - Default state matches DOR definition");
    }

    // =========================================================================
    // TC-HOOKS-IO-008: ConsciousnessState JSON Round-trip
    // =========================================================================
    #[test]
    fn tc_hooks_io_008_consciousness_state_json() {
        println!("\n=== TC-HOOKS-IO-008: ConsciousnessState JSON Round-trip ===");

        let state = ConsciousnessState::new(0.73, 0.85, 0.78, 0.82, 0.92);

        let json = serde_json::to_string(&state).expect("serialize");
        println!("  Serialized: {}", json);

        let parsed: ConsciousnessState = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(state, parsed, "Round-trip MUST preserve all values");
        assert_eq!(parsed.johari_quadrant, JohariQuadrant::Open,
            "C=0.73, I=0.85 must classify as Open");

        println!("RESULT: PASS - JSON round-trip preserves all values");
    }

    // =========================================================================
    // TC-HOOKS-IO-009: ICClassification Crisis Trigger
    // SOURCE OF TRUTH: constitution.yaml critical: "<0.5"
    // =========================================================================
    #[test]
    fn tc_hooks_io_009_ic_classification_crisis() {
        println!("\n=== TC-HOOKS-IO-009: ICClassification Crisis Trigger ===");
        println!("SOURCE: constitution.yaml critical: \"<0.5\"");

        let crisis = ICClassification::new(0.45, 0.5);
        assert!(crisis.crisis_triggered, "0.45 < 0.5 MUST trigger crisis");
        assert_eq!(crisis.level, ICLevel::Critical, "0.45 MUST be Critical");

        let no_crisis = ICClassification::new(0.55, 0.5);
        assert!(!no_crisis.crisis_triggered, "0.55 >= 0.5 MUST NOT trigger crisis");
        assert_eq!(no_crisis.level, ICLevel::Warning, "0.55 MUST be Warning");

        let boundary = ICClassification::new(0.5, 0.5);
        assert!(!boundary.crisis_triggered, "0.5 >= 0.5 MUST NOT trigger crisis");
        assert_eq!(boundary.level, ICLevel::Warning, "0.5 MUST be Warning");

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
            payload: serde_json::json!({}),
        };
        assert!(valid.validate().is_none(), "Valid input MUST pass validation");

        let empty_session = HookInput {
            session_id: "".into(),
            ..valid.clone()
        };
        assert!(empty_session.validate().is_some(), "Empty session_id MUST fail");

        let bad_timestamp = HookInput {
            timestamp_ms: 0,
            ..valid.clone()
        };
        assert!(bad_timestamp.validate().is_some(), "Zero timestamp MUST fail");

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
        assert!(output.consciousness_state.is_none(), "Default MUST have no state");
        assert!(output.ic_classification.is_none(), "Default MUST have no classification");
        assert!(output.context_injection.is_none(), "Default MUST have no injection");
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
            .with_consciousness_state(ConsciousnessState::default())
            .with_ic_classification(ICClassification::from_value(0.85))
            .with_context_injection("test injection");

        assert!(output.success);
        assert_eq!(output.execution_time_ms, 42);
        assert!(output.consciousness_state.is_some());
        assert!(output.ic_classification.is_some());
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
            consciousness_state: Some(ConsciousnessState {
                consciousness: 0.73,
                integration: 0.85,
                reflection: 0.78,
                differentiation: 0.82,
                identity_continuity: 0.92,
                johari_quadrant: JohariQuadrant::Open,
            }),
            ic_classification: Some(ICClassification {
                value: 0.92,
                level: ICLevel::Healthy,
                crisis_triggered: false,
            }),
            context_injection: None,
            execution_time_ms: 15,
        };

        let json = serde_json::to_value(&output).expect("serialize to Value");
        println!("  JSON: {}", serde_json::to_string_pretty(&json).unwrap());

        // Required fields
        assert!(json.get("success").is_some(), "success is REQUIRED");
        assert!(json.get("execution_time_ms").is_some(), "execution_time_ms is REQUIRED");

        // Optional fields omitted when None
        assert!(json.get("error").is_none(), "error MUST be omitted when None");
        assert!(json.get("context_injection").is_none(), "context_injection MUST be omitted when None");

        // Nested structure
        let cs = json.get("consciousness_state").expect("consciousness_state present");
        assert!(cs.get("consciousness").is_some());
        assert!(cs.get("integration").is_some());
        assert!(cs.get("johari_quadrant").is_some());
        assert_eq!(cs.get("johari_quadrant").unwrap(), "open");

        let ic = json.get("ic_classification").expect("ic_classification present");
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
            r#""Healthy""#,      // PascalCase ICLevel
            r#""CRITICAL""#,     // UPPERCASE ICLevel
            r#""Open""#,         // PascalCase JohariQuadrant
            r#""UNKNOWN""#,      // UPPERCASE JohariQuadrant
        ];

        for input in invalid_inputs {
            let result_ic: Result<ICLevel, _> = serde_json::from_str(input);
            let result_johari: Result<JohariQuadrant, _> = serde_json::from_str(input);
            println!("  {} -> ICLevel: {:?}, Johari: {:?}",
                     input, result_ic.is_err(), result_johari.is_err());
            // At least one should fail
            assert!(result_ic.is_err() || result_johari.is_err(),
                "FAIL: Invalid input {} should fail deserialization", input);
        }

        println!("RESULT: PASS - Invalid inputs fail fast");
    }
}
