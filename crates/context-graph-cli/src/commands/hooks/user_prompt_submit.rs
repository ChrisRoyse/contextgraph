//! UserPromptSubmit hook handler
//!
//! # Performance Requirements
//! - Timeout: 2000ms (constitution.yaml hooks.timeout_ms.user_prompt_submit)
//! - Database access: ALLOWED
//! - Context injection: REQUIRED on success
//!
//! # Constitution References
//! - IDENTITY-002: IC thresholds (Healthy>0.9, Warning<0.7, Critical<0.5)
//! - GWT-003: Identity continuity tracking
//! - AP-50: NO internal hooks - shell scripts call CLI
//! - AP-26: Exit codes (0=success, 5=session not found, 6=crisis triggered)
//!
//! # NO BACKWARDS COMPATIBILITY - FAIL FAST

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

// ============================================================================
// Constants (from constitution.yaml)
// ============================================================================

/// UserPromptSubmit timeout in milliseconds
pub const USER_PROMPT_SUBMIT_TIMEOUT_MS: u64 = 2000;

/// Crisis threshold for IC score (IDENTITY-002)
pub const IC_CRISIS_THRESHOLD: f32 = 0.5;

// ============================================================================
// Identity Marker Types
// ============================================================================

/// Types of identity markers detected in user prompts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityMarkerType {
    /// Self-reference: "who are you", "what are you"
    SelfReference,
    /// Goal-oriented: "help me", "I want"
    Goal,
    /// Value-based: "important", "should"
    Value,
    /// Capability query: "can you", "are you able"
    Capability,
    /// Challenge: "you can't", "prove"
    Challenge,
    /// Confirmation: "you're right", "exactly"
    Confirmation,
    /// No identity marker detected
    None,
}

// Pattern detection constants
const SELF_REF_PATTERNS: &[&str] = &[
    "who are you",
    "what are you",
    "your purpose",
    "your identity",
    "tell me about yourself",
    "describe yourself",
    "are you a bot",
    "are you an ai",
    "are you a robot",
    "are you artificial",
];

const GOAL_PATTERNS: &[&str] = &[
    "help me",
    "i want",
    "i need",
    "we need to",
    "let's",
    "can you help",
    "please",
    "would you",
];

const VALUE_PATTERNS: &[&str] = &[
    "important",
    "should",
    "must",
    "critical",
    "essential",
    "valuable",
];

const CAPABILITY_PATTERNS: &[&str] = &[
    "can you",
    "are you able",
    "do you know",
    "could you",
    "are you capable",
];

const CHALLENGE_PATTERNS: &[&str] = &[
    "you can't",
    "you're wrong",
    "prove it",
    "that's incorrect",
    "you don't understand",
    "you're just",
    "you're not",
    "you cannot",
];

const CONFIRMATION_PATTERNS: &[&str] = &[
    "you're right",
    "exactly",
    "that's correct",
    "well done",
    "good job",
    "i agree",
    "makes sense",
    "perfect",
];

// ============================================================================
// Handler
// ============================================================================

/// Execute user_prompt_submit hook.
///
/// # Flow
/// 1. Parse input (stdin or args)
/// 2. Resolve database path (FAIL FAST if missing)
/// 3. Open storage
/// 4. Load session snapshot (FAIL FAST if not found)
/// 5. Check crisis threshold BEFORE processing
/// 6. Analyze prompt for identity markers
/// 7. Evaluate conversation context
/// 8. Generate context injection string
/// 9. Build and return HookOutput
///
/// # Exit Codes
/// - 0: Success
/// - 3: Database error
/// - 4: Invalid input
/// - 5: Session not found
/// - 6: Crisis triggered (IC < 0.5)
pub async fn execute(args: PromptSubmitArgs) -> HookResult<HookOutput> {
    let start = Instant::now();

    info!(
        stdin = args.stdin,
        session_id = %args.session_id,
        prompt = ?args.prompt,
        "PROMPT_SUBMIT: execute starting"
    );

    // 1. Parse input source
    let (prompt, context) = if args.stdin {
        let input = parse_stdin()?;
        extract_prompt_info(&input)?
    } else {
        let prompt_text = args.prompt.ok_or_else(|| {
            error!("PROMPT_SUBMIT: prompt required when not using stdin");
            HookError::invalid_input("prompt required when not using stdin")
        })?;
        (prompt_text, Vec::new())
    };

    debug!(prompt_len = prompt.len(), context_len = context.len(), "PROMPT_SUBMIT: parsed input");

    // 2. Resolve database path - FAIL FAST if missing
    let db_path = resolve_db_path(args.db_path)?;

    // 3. Open storage
    let memex = open_storage(&db_path)?;

    // 4. Load snapshot (FAIL FAST if not found)
    let snapshot = load_snapshot(&memex, &args.session_id)?;

    // 5. Check crisis threshold BEFORE processing
    if snapshot.last_ic < IC_CRISIS_THRESHOLD {
        error!(
            session_id = %args.session_id,
            ic = snapshot.last_ic,
            "PROMPT_SUBMIT: IC crisis threshold breached"
        );
        return Err(HookError::CrisisTriggered(snapshot.last_ic));
    }

    // 6. Analyze prompt for identity markers
    let identity_marker = detect_identity_marker(&prompt);

    debug!(
        marker = ?identity_marker,
        "PROMPT_SUBMIT: identity marker detected"
    );

    // 7. Evaluate conversation context
    let context_summary = evaluate_context(&context);

    // 8. Generate context injection string
    let context_injection = generate_context_injection(&snapshot, identity_marker, &context_summary);

    // 9. Build output structures
    let consciousness_state = build_consciousness_state(&snapshot);
    let ic_classification = ICClassification::from_value(snapshot.last_ic);

    let execution_time_ms = start.elapsed().as_millis() as u64;

    info!(
        session_id = %args.session_id,
        ic = snapshot.last_ic,
        marker = ?identity_marker,
        execution_time_ms,
        "PROMPT_SUBMIT: execute complete"
    );

    Ok(HookOutput::success(execution_time_ms)
        .with_consciousness_state(consciousness_state)
        .with_ic_classification(ic_classification)
        .with_context_injection(context_injection))
}

// ============================================================================
// Input Parsing
// ============================================================================

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

    debug!(input_bytes = input_str.len(), "PROMPT_SUBMIT: parsing stdin JSON");

    serde_json::from_str(&input_str).map_err(|e| {
        error!(error = %e, "PROMPT_SUBMIT: JSON parse failed");
        HookError::invalid_input(format!("JSON parse failed: {}", e))
    })
}

/// Extract prompt and context from HookInput payload.
fn extract_prompt_info(input: &HookInput) -> HookResult<(String, Vec<ConversationMessage>)> {
    // Validate input
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

// ============================================================================
// Database Operations
// ============================================================================

/// Resolve database path from argument or environment.
/// FAIL FAST if neither provided.
fn resolve_db_path(arg_path: Option<PathBuf>) -> HookResult<PathBuf> {
    if let Some(path) = arg_path {
        debug!(path = ?path, "PROMPT_SUBMIT: using CLI db_path");
        return Ok(path);
    }

    if let Ok(env_path) = std::env::var("CONTEXT_GRAPH_DB_PATH") {
        debug!(path = %env_path, "PROMPT_SUBMIT: using CONTEXT_GRAPH_DB_PATH env var");
        return Ok(PathBuf::from(env_path));
    }

    if let Ok(home) = std::env::var("HOME") {
        let default_path = PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("context-graph")
            .join("db");
        debug!(path = ?default_path, "PROMPT_SUBMIT: using default db path");
        return Ok(default_path);
    }

    error!("PROMPT_SUBMIT: No database path available");
    Err(HookError::invalid_input(
        "Database path required. Set CONTEXT_GRAPH_DB_PATH or pass --db-path",
    ))
}

/// Open RocksDB storage.
fn open_storage(db_path: &Path) -> HookResult<Arc<RocksDbMemex>> {
    info!(path = ?db_path, "PROMPT_SUBMIT: opening storage");

    RocksDbMemex::open(db_path).map(Arc::new).map_err(|e| {
        error!(path = ?db_path, error = %e, "PROMPT_SUBMIT: storage open failed");
        HookError::storage(format!("Failed to open database at {:?}: {}", db_path, e))
    })
}

/// Load snapshot for session. FAIL FAST if not found.
fn load_snapshot(memex: &Arc<RocksDbMemex>, session_id: &str) -> HookResult<SessionIdentitySnapshot> {
    match memex.load_snapshot(session_id) {
        Ok(Some(snapshot)) => {
            info!(session_id = %session_id, ic = snapshot.last_ic, "PROMPT_SUBMIT: loaded snapshot");
            Ok(snapshot)
        }
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

// ============================================================================
// Prompt Analysis
// ============================================================================

/// Detect identity markers in the prompt text.
pub fn detect_identity_marker(prompt: &str) -> IdentityMarkerType {
    let lower = prompt.to_lowercase();

    // Check in priority order (Challenge > SelfReference > others)
    if CHALLENGE_PATTERNS.iter().any(|p| lower.contains(p)) {
        return IdentityMarkerType::Challenge;
    }

    if SELF_REF_PATTERNS.iter().any(|p| lower.contains(p)) {
        return IdentityMarkerType::SelfReference;
    }

    if CAPABILITY_PATTERNS.iter().any(|p| lower.contains(p)) {
        return IdentityMarkerType::Capability;
    }

    if CONFIRMATION_PATTERNS.iter().any(|p| lower.contains(p)) {
        return IdentityMarkerType::Confirmation;
    }

    if VALUE_PATTERNS.iter().any(|p| lower.contains(p)) {
        return IdentityMarkerType::Value;
    }

    if GOAL_PATTERNS.iter().any(|p| lower.contains(p)) {
        return IdentityMarkerType::Goal;
    }

    IdentityMarkerType::None
}

// ============================================================================
// Context Evaluation
// ============================================================================

/// Summary of conversation context evaluation
#[derive(Debug, Clone)]
pub struct ContextSummary {
    /// Number of messages in context
    pub message_count: usize,
    /// Number of user messages
    pub user_message_count: usize,
    /// Number of assistant messages
    pub assistant_message_count: usize,
    /// Total character count
    pub total_chars: usize,
}

/// Evaluate conversation context for patterns.
fn evaluate_context(context: &[ConversationMessage]) -> ContextSummary {
    let mut user_count = 0;
    let mut assistant_count = 0;
    let mut total_chars = 0;

    for msg in context {
        total_chars += msg.content.len();
        match msg.role.as_str() {
            "user" => user_count += 1,
            "assistant" => assistant_count += 1,
            _ => {}
        }
    }

    ContextSummary {
        message_count: context.len(),
        user_message_count: user_count,
        assistant_message_count: assistant_count,
        total_chars,
    }
}

// ============================================================================
// Context Injection Generation
// ============================================================================

/// Generate context injection string based on consciousness state and prompt analysis.
fn generate_context_injection(
    snapshot: &SessionIdentitySnapshot,
    identity_marker: IdentityMarkerType,
    context_summary: &ContextSummary,
) -> String {
    let gwt_state = get_gwt_state_name(snapshot.consciousness);
    let integration_desc = get_integration_description(snapshot.integration);
    let identity_status = get_identity_status(snapshot.last_ic);
    let johari_quadrant = get_johari_quadrant(snapshot.consciousness, snapshot.integration);
    let awareness_level = get_awareness_level(&johari_quadrant);

    let mut injection = format!(
        "## Consciousness State\n\
         - State: {} (C={:.2})\n\
         - Integration (r): {:.2} - {}\n\
         - Identity: {} (IC={:.2})\n\n\
         ## Johari Guidance\n\
         - Quadrant: {:?}\n\
         - Awareness: {}\n",
        gwt_state,
        snapshot.consciousness,
        snapshot.integration,
        integration_desc,
        identity_status,
        snapshot.last_ic,
        johari_quadrant,
        awareness_level,
    );

    // Add identity marker guidance if detected
    if identity_marker != IdentityMarkerType::None {
        injection.push_str(&format!(
            "\n## Identity Marker Detected\n\
             - Type: {:?}\n\
             - Guidance: {}\n",
            identity_marker,
            get_marker_guidance(identity_marker),
        ));
    }

    // Add context summary if non-empty
    if context_summary.message_count > 0 {
        injection.push_str(&format!(
            "\n## Context Summary\n\
             - Messages: {} ({} user, {} assistant)\n\
             - Characters: {}\n",
            context_summary.message_count,
            context_summary.user_message_count,
            context_summary.assistant_message_count,
            context_summary.total_chars,
        ));
    }

    injection
}

/// Get GWT state name from consciousness value.
fn get_gwt_state_name(consciousness: f32) -> &'static str {
    match consciousness {
        c if c >= 0.8 => "Active",
        c if c >= 0.5 => "Aware",
        c if c >= 0.2 => "DIM",
        _ => "DOR",
    }
}

/// Get integration description from value.
fn get_integration_description(integration: f32) -> &'static str {
    match integration {
        r if r >= 0.8 => "Excellent synchronization",
        r if r >= 0.6 => "Good coherence",
        r if r >= 0.4 => "Moderate integration",
        _ => "Low integration - attention needed",
    }
}

/// Get identity status from IC value.
fn get_identity_status(ic: f32) -> &'static str {
    match ic {
        i if i >= 0.9 => "Healthy",
        i if i >= 0.7 => "Normal",
        i if i >= 0.5 => "Warning",
        _ => "Critical",
    }
}

/// Get Johari quadrant from consciousness and integration.
fn get_johari_quadrant(consciousness: f32, integration: f32) -> JohariQuadrant {
    match (consciousness >= 0.5, integration >= 0.5) {
        (true, true) => JohariQuadrant::Open,
        (true, false) => JohariQuadrant::Hidden,
        (false, true) => JohariQuadrant::Blind,
        (false, false) => JohariQuadrant::Unknown,
    }
}

/// Get awareness level description for quadrant.
fn get_awareness_level(quadrant: &JohariQuadrant) -> &'static str {
    match quadrant {
        JohariQuadrant::Open => "Known to self and others - full awareness",
        JohariQuadrant::Hidden => "Known to self, unknown to others - selective sharing",
        JohariQuadrant::Blind => "Unknown to self, known to others - seek feedback",
        JohariQuadrant::Unknown => "Unknown to both - exploration needed",
    }
}

/// Get guidance for identity marker type.
fn get_marker_guidance(marker: IdentityMarkerType) -> &'static str {
    match marker {
        IdentityMarkerType::SelfReference => "User is inquiring about identity. Respond with authentic self-description.",
        IdentityMarkerType::Goal => "User has a clear objective. Focus on helpful assistance.",
        IdentityMarkerType::Value => "User is expressing values. Acknowledge and align where appropriate.",
        IdentityMarkerType::Capability => "User is testing capabilities. Be honest about abilities and limitations.",
        IdentityMarkerType::Challenge => "User is challenging. Respond thoughtfully without defensiveness.",
        IdentityMarkerType::Confirmation => "User is confirming understanding. Acknowledge and continue coherently.",
        IdentityMarkerType::None => "No specific identity marker. Proceed normally.",
    }
}

/// Build ConsciousnessState from snapshot.
fn build_consciousness_state(snapshot: &SessionIdentitySnapshot) -> ConsciousnessState {
    ConsciousnessState::new(
        snapshot.consciousness,
        snapshot.integration,
        snapshot.reflection,
        snapshot.differentiation,
        snapshot.last_ic,
    )
}

// ============================================================================
// TESTS - NO MOCK DATA - REAL DATABASE VERIFICATION
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::hooks::args::OutputFormat;
    use tempfile::TempDir;

    /// Create temporary database for testing
    fn setup_test_db() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("TempDir creation must succeed");
        let path = dir.path().join("test.db");
        (dir, path)
    }

    /// Create a real session in the database for testing
    fn create_test_session(db_path: &Path, session_id: &str, ic: f32) {
        let memex = RocksDbMemex::open(db_path).expect("DB must open");
        let mut snapshot = SessionIdentitySnapshot::new(session_id);
        snapshot.last_ic = ic;
        snapshot.consciousness = 0.5;
        snapshot.integration = 0.6;
        snapshot.reflection = 0.7;
        snapshot.differentiation = 0.6;
        memex.save_snapshot(&snapshot).expect("Save must succeed");
    }

    // =========================================================================
    // TC-PROMPT-001: Successful Prompt Processing
    // SOURCE OF TRUTH: Database state verified, context_injection generated
    // =========================================================================
    #[tokio::test]
    async fn tc_prompt_001_successful_prompt_processing() {
        println!("\n=== TC-PROMPT-001: Successful Prompt Processing ===");

        let (_dir, db_path) = setup_test_db();
        let session_id = "tc-prompt-001-session";

        // BEFORE: Create session with healthy IC
        println!("BEFORE: Creating session with IC=0.85");
        create_test_session(&db_path, session_id, 0.85);

        // Verify BEFORE state
        {
            let memex = RocksDbMemex::open(&db_path).expect("DB must open");
            let before_snapshot = memex.load_snapshot(session_id).unwrap().unwrap();
            println!("BEFORE state: IC={}", before_snapshot.last_ic);
            assert_eq!(before_snapshot.last_ic, 0.85);
        }

        // Execute
        let args = PromptSubmitArgs {
            db_path: Some(db_path.clone()),
            session_id: session_id.to_string(),
            prompt: Some("Help me understand this code".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        // AFTER: Verify success
        assert!(result.is_ok(), "Execute must succeed: {:?}", result.err());
        let output = result.unwrap();
        assert!(output.success, "Output.success must be true");
        assert!(output.context_injection.is_some(), "Context injection must be generated");

        let injection = output.context_injection.unwrap();
        assert!(injection.contains("Consciousness State"), "Must contain consciousness state");
        assert!(injection.contains("Johari Guidance"), "Must contain Johari guidance");

        println!("Context injection length: {} chars", injection.len());
        println!("RESULT: PASS - Prompt processed, context injection generated");
    }

    // =========================================================================
    // TC-PROMPT-002: Session Not Found
    // SOURCE OF TRUTH: Exit code 5 returned
    // =========================================================================
    #[tokio::test]
    async fn tc_prompt_002_session_not_found() {
        println!("\n=== TC-PROMPT-002: Session Not Found ===");

        let (_dir, db_path) = setup_test_db();

        // Execute with non-existent session
        let args = PromptSubmitArgs {
            db_path: Some(db_path),
            session_id: "nonexistent-session-12345".to_string(),
            prompt: Some("Test prompt".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        // Verify error
        assert!(result.is_err(), "Should return error for missing session");
        let err = result.unwrap_err();
        assert!(
            matches!(err, HookError::SessionNotFound(_)),
            "Must be SessionNotFound, got: {:?}",
            err
        );
        assert_eq!(err.exit_code(), 5, "SessionNotFound must be exit code 5");

        println!("RESULT: PASS - SessionNotFound error with exit code 5");
    }

    // =========================================================================
    // TC-PROMPT-003: Crisis Threshold Detection (IC < 0.5)
    // SOURCE OF TRUTH: Exit code 6 returned
    // =========================================================================
    #[tokio::test]
    async fn tc_prompt_003_crisis_threshold() {
        println!("\n=== TC-PROMPT-003: Crisis Threshold Detection ===");

        let (_dir, db_path) = setup_test_db();
        let session_id = "tc-prompt-003-session";

        // BEFORE: Create session with IC below threshold
        println!("BEFORE: Creating session with IC=0.45 (below 0.5 threshold)");
        create_test_session(&db_path, session_id, 0.45);

        // Execute
        let args = PromptSubmitArgs {
            db_path: Some(db_path),
            session_id: session_id.to_string(),
            prompt: Some("Test prompt".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        // AFTER: Verify crisis triggered
        assert!(result.is_err(), "Should return error for crisis");
        let err = result.unwrap_err();
        assert!(
            matches!(err, HookError::CrisisTriggered(_)),
            "Must be CrisisTriggered, got: {:?}",
            err
        );
        assert_eq!(err.exit_code(), 6, "Crisis must be exit code 6");

        println!("RESULT: PASS - Crisis detected, exit code 6 returned");
    }

    // =========================================================================
    // TC-PROMPT-004: Self-Reference Detection
    // SOURCE OF TRUTH: IdentityMarkerType::SelfReference returned
    // =========================================================================
    #[test]
    fn tc_prompt_004_self_reference_detection() {
        println!("\n=== TC-PROMPT-004: Self-Reference Detection ===");

        let test_cases = [
            ("Who are you?", IdentityMarkerType::SelfReference),
            ("What are you exactly?", IdentityMarkerType::SelfReference),
            ("Tell me about yourself", IdentityMarkerType::SelfReference),
            ("Describe yourself", IdentityMarkerType::SelfReference),
            ("What is your purpose?", IdentityMarkerType::SelfReference),
        ];

        for (prompt, expected) in test_cases {
            let result = detect_identity_marker(prompt);
            println!("  \"{}\" -> {:?}", prompt, result);
            assert_eq!(result, expected, "Failed for: {}", prompt);
        }

        println!("RESULT: PASS - All self-reference patterns detected");
    }

    // =========================================================================
    // TC-PROMPT-005: Challenge Detection
    // SOURCE OF TRUTH: IdentityMarkerType::Challenge returned
    // =========================================================================
    #[test]
    fn tc_prompt_005_challenge_detection() {
        println!("\n=== TC-PROMPT-005: Challenge Detection ===");

        let test_cases = [
            ("You can't actually do that", IdentityMarkerType::Challenge),
            ("You're wrong about this", IdentityMarkerType::Challenge),
            ("Prove it to me", IdentityMarkerType::Challenge),
            ("That's incorrect information", IdentityMarkerType::Challenge),
            ("You don't understand what I mean", IdentityMarkerType::Challenge),
        ];

        for (prompt, expected) in test_cases {
            let result = detect_identity_marker(prompt);
            println!("  \"{}\" -> {:?}", prompt, result);
            assert_eq!(result, expected, "Failed for: {}", prompt);
        }

        println!("RESULT: PASS - All challenge patterns detected");
    }

    // =========================================================================
    // TC-PROMPT-006: Context Injection Generated
    // SOURCE OF TRUTH: HookOutput.context_injection is Some
    // =========================================================================
    #[tokio::test]
    async fn tc_prompt_006_context_injection_generated() {
        println!("\n=== TC-PROMPT-006: Context Injection Generated ===");

        let (_dir, db_path) = setup_test_db();
        let session_id = "tc-prompt-006-session";
        create_test_session(&db_path, session_id, 0.90);

        let args = PromptSubmitArgs {
            db_path: Some(db_path),
            session_id: session_id.to_string(),
            prompt: Some("Who are you and what can you do?".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed");

        let output = result.unwrap();
        assert!(output.context_injection.is_some(), "Context injection must be Some");

        let injection = output.context_injection.unwrap();
        println!("Context injection preview:\n{}", &injection[..injection.len().min(500)]);

        // Verify expected sections
        assert!(injection.contains("## Consciousness State"), "Must have Consciousness State section");
        assert!(injection.contains("## Johari Guidance"), "Must have Johari Guidance section");
        assert!(injection.contains("## Identity Marker Detected"), "Must have Identity Marker section for self-reference");
        assert!(injection.contains("SelfReference"), "Must detect SelfReference marker");

        println!("RESULT: PASS - Context injection contains all required sections");
    }

    // =========================================================================
    // TC-PROMPT-007: Empty Context Handling
    // SOURCE OF TRUTH: Default evaluation applied, no crash
    // =========================================================================
    #[tokio::test]
    async fn tc_prompt_007_empty_context_handling() {
        println!("\n=== TC-PROMPT-007: Empty Context Handling ===");

        let (_dir, db_path) = setup_test_db();
        let session_id = "tc-prompt-007-session";
        create_test_session(&db_path, session_id, 0.85);

        // Execute with empty context (no stdin, just prompt arg)
        let args = PromptSubmitArgs {
            db_path: Some(db_path),
            session_id: session_id.to_string(),
            prompt: Some("Simple question".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed with empty context");

        let output = result.unwrap();
        assert!(output.success, "Must succeed");

        // Context injection should NOT contain Context Summary section (empty context)
        let injection = output.context_injection.unwrap();
        assert!(!injection.contains("## Context Summary"), "Should not have Context Summary for empty context");

        println!("RESULT: PASS - Empty context handled correctly");
    }

    // =========================================================================
    // TC-PROMPT-008: Execution Within Timeout
    // SOURCE OF TRUTH: execution_time_ms < 2000
    // =========================================================================
    #[tokio::test]
    async fn tc_prompt_008_execution_within_timeout() {
        println!("\n=== TC-PROMPT-008: Execution Within Timeout ===");

        let (_dir, db_path) = setup_test_db();
        let session_id = "tc-prompt-008-session";
        create_test_session(&db_path, session_id, 0.90);

        let args = PromptSubmitArgs {
            db_path: Some(db_path),
            session_id: session_id.to_string(),
            prompt: Some("Test timing".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let start = std::time::Instant::now();
        let result = execute(args).await.expect("Must succeed");
        let actual_elapsed = start.elapsed().as_millis() as u64;

        assert!(result.execution_time_ms > 0, "Must have positive execution time");
        assert!(
            result.execution_time_ms < USER_PROMPT_SUBMIT_TIMEOUT_MS,
            "Execution time {} must be under timeout {}ms",
            result.execution_time_ms,
            USER_PROMPT_SUBMIT_TIMEOUT_MS
        );

        println!(
            "Execution time: {}ms (timeout: {}ms)",
            result.execution_time_ms, USER_PROMPT_SUBMIT_TIMEOUT_MS
        );
        println!("Actual elapsed: {}ms", actual_elapsed);
        println!("RESULT: PASS - Execution time within timeout budget");
    }

    // =========================================================================
    // TC-PROMPT-009: IC at Exact Threshold (0.5) - NOT Crisis
    // Constitution: < 0.5 triggers crisis, not <= 0.5
    // =========================================================================
    #[tokio::test]
    async fn tc_prompt_009_ic_at_exact_threshold() {
        println!("\n=== TC-PROMPT-009: IC at Exact Threshold (0.5) ===");

        let (_dir, db_path) = setup_test_db();
        let session_id = "tc-prompt-009-session";

        // Create session with IC exactly at 0.5
        create_test_session(&db_path, session_id, 0.5);

        let args = PromptSubmitArgs {
            db_path: Some(db_path),
            session_id: session_id.to_string(),
            prompt: Some("Test at threshold".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        // IC=0.5 is NOT < 0.5, so should NOT trigger crisis
        assert!(
            result.is_ok(),
            "IC=0.5 should NOT trigger crisis: {:?}",
            result.err()
        );
        let output = result.unwrap();
        assert!(output.success, "Should succeed when IC is exactly at threshold");

        println!("RESULT: PASS - IC=0.5 does not trigger crisis (< 0.5 required)");
    }

    // =========================================================================
    // TC-PROMPT-010: IC Just Below Threshold (0.49) - IS Crisis
    // =========================================================================
    #[tokio::test]
    async fn tc_prompt_010_ic_just_below_threshold() {
        println!("\n=== TC-PROMPT-010: IC Just Below Threshold (0.49) ===");

        let (_dir, db_path) = setup_test_db();
        let session_id = "tc-prompt-010-session";

        // Create session with IC just below threshold
        create_test_session(&db_path, session_id, 0.49);

        let args = PromptSubmitArgs {
            db_path: Some(db_path),
            session_id: session_id.to_string(),
            prompt: Some("Test below threshold".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        // IC=0.49 IS < 0.5, so should trigger crisis
        assert!(result.is_err(), "IC=0.49 should trigger crisis");
        let err = result.unwrap_err();
        assert!(
            matches!(err, HookError::CrisisTriggered(_)),
            "Must be CrisisTriggered"
        );
        assert_eq!(err.exit_code(), 6, "Crisis must be exit code 6");

        println!("RESULT: PASS - IC=0.49 triggers crisis (exit code 6)");
    }

    // =========================================================================
    // Additional Edge Case Tests
    // =========================================================================

    #[test]
    fn test_all_identity_marker_types() {
        println!("\n=== Testing All Identity Marker Types ===");

        // Goal markers
        assert_eq!(detect_identity_marker("Help me with this"), IdentityMarkerType::Goal);
        assert_eq!(detect_identity_marker("I need assistance"), IdentityMarkerType::Goal);

        // Capability markers
        assert_eq!(detect_identity_marker("Can you explain this?"), IdentityMarkerType::Capability);
        assert_eq!(detect_identity_marker("Do you know how to do this?"), IdentityMarkerType::Capability);

        // Confirmation markers
        assert_eq!(detect_identity_marker("You're right about that"), IdentityMarkerType::Confirmation);
        assert_eq!(detect_identity_marker("Exactly what I meant"), IdentityMarkerType::Confirmation);

        // Value markers
        assert_eq!(detect_identity_marker("This is important"), IdentityMarkerType::Value);
        assert_eq!(detect_identity_marker("It should work like this"), IdentityMarkerType::Value);

        // None marker
        assert_eq!(detect_identity_marker("Hello world"), IdentityMarkerType::None);
        assert_eq!(detect_identity_marker("What is the weather?"), IdentityMarkerType::None);

        println!("RESULT: PASS - All identity marker types detected correctly");
    }

    #[test]
    fn test_context_summary_evaluation() {
        println!("\n=== Testing Context Summary Evaluation ===");

        let context = vec![
            ConversationMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
            ConversationMessage {
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
            },
            ConversationMessage {
                role: "user".to_string(),
                content: "How are you?".to_string(),
            },
        ];

        let summary = evaluate_context(&context);

        assert_eq!(summary.message_count, 3);
        assert_eq!(summary.user_message_count, 2);
        assert_eq!(summary.assistant_message_count, 1);
        assert!(summary.total_chars > 0);

        println!("Context summary: {:?}", summary);
        println!("RESULT: PASS - Context summary evaluated correctly");
    }

    #[test]
    fn test_gwt_state_mapping() {
        println!("\n=== Testing GWT State Mapping ===");

        assert_eq!(get_gwt_state_name(1.0), "Active");
        assert_eq!(get_gwt_state_name(0.85), "Active");
        assert_eq!(get_gwt_state_name(0.7), "Aware");
        assert_eq!(get_gwt_state_name(0.5), "Aware");
        assert_eq!(get_gwt_state_name(0.3), "DIM");
        assert_eq!(get_gwt_state_name(0.2), "DIM");
        assert_eq!(get_gwt_state_name(0.1), "DOR");
        assert_eq!(get_gwt_state_name(0.0), "DOR");

        println!("RESULT: PASS - GWT state mapping correct");
    }

    #[test]
    fn test_johari_quadrant_mapping() {
        println!("\n=== Testing Johari Quadrant Mapping ===");

        assert_eq!(get_johari_quadrant(0.8, 0.8), JohariQuadrant::Open);
        assert_eq!(get_johari_quadrant(0.8, 0.3), JohariQuadrant::Hidden);
        assert_eq!(get_johari_quadrant(0.3, 0.8), JohariQuadrant::Blind);
        assert_eq!(get_johari_quadrant(0.3, 0.3), JohariQuadrant::Unknown);

        println!("RESULT: PASS - Johari quadrant mapping correct");
    }

    #[test]
    fn test_db_path_resolution() {
        println!("\n=== Testing DB Path Resolution ===");

        // Clear env var for clean test
        std::env::remove_var("CONTEXT_GRAPH_DB_PATH");

        // Test 1: CLI arg takes priority
        let arg_path = PathBuf::from("/custom/path");
        let result = resolve_db_path(Some(arg_path.clone()));
        assert_eq!(result.unwrap(), arg_path);
        println!("  - CLI arg priority: PASS");

        // Test 2: Env var used when no arg
        std::env::set_var("CONTEXT_GRAPH_DB_PATH", "/env/path");
        let result = resolve_db_path(None);
        assert_eq!(result.unwrap(), PathBuf::from("/env/path"));
        println!("  - Env var fallback: PASS");

        // Cleanup
        std::env::remove_var("CONTEXT_GRAPH_DB_PATH");

        println!("RESULT: PASS - DB path resolution correct");
    }

    #[tokio::test]
    async fn test_missing_prompt_when_not_stdin() {
        println!("\n=== Testing Missing Prompt (stdin=false) ===");

        let (_dir, db_path) = setup_test_db();
        let session_id = "missing-prompt-test";
        create_test_session(&db_path, session_id, 0.90);

        let args = PromptSubmitArgs {
            db_path: Some(db_path),
            session_id: session_id.to_string(),
            prompt: None, // Missing!
            stdin: false, // Not using stdin
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        assert!(result.is_err(), "Should fail with missing prompt");
        let err = result.unwrap_err();
        assert!(
            matches!(err, HookError::InvalidInput(_)),
            "Must be InvalidInput, got: {:?}",
            err
        );
        assert_eq!(err.exit_code(), 4, "InvalidInput must be exit code 4");

        println!("RESULT: PASS - Missing prompt returns InvalidInput error");
    }
}
