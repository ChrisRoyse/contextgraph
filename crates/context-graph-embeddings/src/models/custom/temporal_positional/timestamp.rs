//! Timestamp and sequence position parsing utilities for the Temporal-Positional model (E4).
//!
//! E4 encodes session sequence positions to enable "before/after" queries within a session.
//! This module supports both sequence-based positions (preferred) and timestamp-based positions
//! (fallback for backward compatibility).
//!
//! # Hybrid Mode Parsing
//!
//! For hybrid mode, instructions can include a session identifier:
//! - `"session:abc123 sequence:42"` - Session ID with sequence position
//! - `"session:abc123 timestamp:2024-01-15T10:30:00Z"` - Session ID with timestamp
//! - `"sequence:42"` - Legacy format without session (uses sentinel)

use chrono::{DateTime, Utc};

use crate::types::ModelInput;

/// Position information extracted from input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionInfo {
    /// The position value (sequence number or Unix timestamp)
    pub position: i64,
    /// True if position is a session sequence number, false if Unix timestamp
    pub is_sequence: bool,
}

impl PositionInfo {
    /// Create a new sequence-based position.
    #[must_use]
    pub fn sequence(seq: u64) -> Self {
        Self {
            position: seq as i64,
            is_sequence: true,
        }
    }

    /// Create a new timestamp-based position.
    #[must_use]
    pub fn timestamp(ts: i64) -> Self {
        Self {
            position: ts,
            is_sequence: false,
        }
    }

    /// Create a position from current time.
    #[must_use]
    pub fn now() -> Self {
        Self::timestamp(Utc::now().timestamp())
    }
}

// =============================================================================
// HYBRID MODE POSITION INFO
// =============================================================================

/// Extended position info including session context for hybrid mode.
///
/// In hybrid mode, E4 embeddings combine:
/// - Session signature (256D) - from session_id
/// - Position encoding (256D) - from position/is_sequence
#[derive(Debug, Clone, PartialEq)]
pub struct HybridPositionInfo {
    /// The position value (sequence number or Unix timestamp)
    pub position: i64,
    /// True if position is a session sequence number, false if Unix timestamp
    pub is_sequence: bool,
    /// Optional session identifier for clustering
    pub session_id: Option<String>,
}

impl HybridPositionInfo {
    /// Create a new hybrid position with all components.
    #[must_use]
    pub fn new(position: i64, is_sequence: bool, session_id: Option<String>) -> Self {
        Self {
            position,
            is_sequence,
            session_id,
        }
    }

    /// Create from a PositionInfo with optional session.
    #[must_use]
    pub fn from_position_info(pos: PositionInfo, session_id: Option<String>) -> Self {
        Self {
            position: pos.position,
            is_sequence: pos.is_sequence,
            session_id,
        }
    }

    /// Create a hybrid position with session and sequence.
    #[must_use]
    pub fn with_session_sequence(session_id: &str, seq: u64) -> Self {
        Self {
            position: seq as i64,
            is_sequence: true,
            session_id: Some(session_id.to_string()),
        }
    }

    /// Create a hybrid position with no session context (legacy mode).
    #[must_use]
    pub fn without_session(position: i64, is_sequence: bool) -> Self {
        Self {
            position,
            is_sequence,
            session_id: None,
        }
    }

    /// Create a default hybrid position using current time, no session.
    #[must_use]
    pub fn now() -> Self {
        Self::without_session(Utc::now().timestamp(), false)
    }
}

/// Extract position from ModelInput for E4 embedding.
///
/// Priority order:
/// 1. "sequence:N" - Session sequence number (preferred for E4)
/// 2. "timestamp:ISO8601" - ISO 8601 timestamp
/// 3. "epoch:N" - Unix epoch seconds
/// 4. Falls back to current time
///
/// Returns `PositionInfo` indicating whether position is sequence-based or timestamp-based.
pub fn extract_position(input: &ModelInput) -> PositionInfo {
    match input {
        ModelInput::Text { instruction, .. } => instruction
            .as_ref()
            .and_then(|inst| parse_position(inst))
            .unwrap_or_else(PositionInfo::now),
        // For non-text inputs, use current time
        _ => PositionInfo::now(),
    }
}

/// Extract hybrid position (with session) from ModelInput for E4 embedding.
///
/// This function is used in hybrid mode to extract both session identity
/// and position information from the instruction field.
///
/// # Instruction Format
///
/// The instruction field can contain:
/// - `"session:abc123 sequence:42"` - Session ID with sequence position
/// - `"session:abc123 timestamp:2024-01-15T10:30:00Z"` - Session ID with timestamp
/// - `"sequence:42"` - Legacy format without session
///
/// # Returns
///
/// `HybridPositionInfo` with session_id (if present), position, and is_sequence flag.
pub fn extract_hybrid_position(input: &ModelInput) -> HybridPositionInfo {
    match input {
        ModelInput::Text { instruction, .. } => instruction
            .as_ref()
            .map(|inst| parse_hybrid_position(inst))
            .unwrap_or_else(HybridPositionInfo::now),
        // For non-text inputs, use current time with no session
        _ => HybridPositionInfo::now(),
    }
}

/// Parse hybrid position from instruction string.
///
/// Supports formats:
/// - "session:abc123 sequence:42" (preferred)
/// - "session:abc123 timestamp:2024-01-15T10:30:00Z"
/// - Legacy formats without session (backward compatible)
///
/// # Arguments
///
/// * `instruction` - The instruction string to parse
///
/// # Returns
///
/// `HybridPositionInfo` with extracted session_id and position info.
pub fn parse_hybrid_position(instruction: &str) -> HybridPositionInfo {
    let session_id = extract_session_id(instruction);
    let position_info = parse_position(instruction).unwrap_or_else(PositionInfo::now);

    HybridPositionInfo::from_position_info(position_info, session_id)
}

/// Extract session_id from instruction string.
///
/// Looks for "session:XXXX" anywhere in the instruction string.
/// The session_id is extracted until the next whitespace character.
///
/// # Arguments
///
/// * `instruction` - The instruction string to parse
///
/// # Returns
///
/// `Some(String)` with the session_id if found, `None` otherwise.
fn extract_session_id(instruction: &str) -> Option<String> {
    for part in instruction.split_whitespace() {
        if let Some(id) = part.strip_prefix("session:") {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Parse position from instruction string.
///
/// Supports both standalone and multi-part instructions:
/// - "sequence:123" - Standalone sequence number
/// - "session:abc123 sequence:42" - Multi-part with session
///
/// Priority order (first match wins):
/// 1. "sequence:N" -> (N, is_sequence=true) - Session sequence number
/// 2. "timestamp:ISO8601" -> (unix_secs, is_sequence=false)
/// 3. "epoch:N" -> (N, is_sequence=false)
///
/// # Returns
/// `Some(PositionInfo)` if parsing succeeded, `None` otherwise.
pub fn parse_position(instruction: &str) -> Option<PositionInfo> {
    // Search each whitespace-separated part of the instruction
    for part in instruction.split_whitespace() {
        // Priority 1: Sequence number (e.g., "sequence:123")
        if let Some(seq_str) = part.strip_prefix("sequence:") {
            if let Ok(seq) = seq_str.parse::<u64>() {
                return Some(PositionInfo::sequence(seq));
            }
        }

        // Priority 2: ISO 8601 timestamp (e.g., "timestamp:2024-01-15T10:30:00Z")
        if let Some(ts_str) = part.strip_prefix("timestamp:") {
            if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
                return Some(PositionInfo::timestamp(dt.with_timezone(&Utc).timestamp()));
            }
        }

        // Priority 3: Unix epoch (e.g., "epoch:1705315800")
        if let Some(epoch_str) = part.strip_prefix("epoch:") {
            if let Ok(secs) = epoch_str.parse::<i64>() {
                return Some(PositionInfo::timestamp(secs));
            }
        }
    }

    None
}

/// Extract timestamp from ModelInput (legacy API for backward compatibility).
///
/// Attempts to parse timestamp from the instruction field:
/// - ISO 8601 format: "timestamp:2024-01-15T10:30:00Z"
/// - Unix epoch: "epoch:1705315800"
///
/// Falls back to current time if no valid timestamp found.
///
/// Note: For new code, prefer `extract_position()` which also supports sequence numbers.
pub fn extract_timestamp(input: &ModelInput) -> DateTime<Utc> {
    match input {
        ModelInput::Text { instruction, .. } => instruction
            .as_ref()
            .and_then(|inst| parse_timestamp(inst))
            .unwrap_or_else(Utc::now),
        // For non-text inputs, use current time
        _ => Utc::now(),
    }
}

/// Parse timestamp from instruction string (legacy API for backward compatibility).
///
/// Supports formats:
/// - ISO 8601: "timestamp:2024-01-15T10:30:00Z"
/// - Unix epoch: "epoch:1705315800"
///
/// Note: For new code, prefer `parse_position()` which also supports sequence numbers.
pub fn parse_timestamp(instruction: &str) -> Option<DateTime<Utc>> {
    // Try ISO 8601 format: "timestamp:2024-01-15T10:30:00Z"
    if let Some(ts_str) = instruction.strip_prefix("timestamp:") {
        if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str.trim()) {
            return Some(dt.with_timezone(&Utc));
        }
    }

    // Try Unix epoch: "epoch:1705315800"
    if let Some(epoch_str) = instruction.strip_prefix("epoch:") {
        if let Ok(secs) = epoch_str.trim().parse::<i64>() {
            return DateTime::from_timestamp(secs, 0);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sequence() {
        let pos = parse_position("sequence:42").unwrap();
        assert_eq!(pos.position, 42);
        assert!(pos.is_sequence);
    }

    #[test]
    fn test_parse_sequence_large() {
        let pos = parse_position("sequence:9999999").unwrap();
        assert_eq!(pos.position, 9999999);
        assert!(pos.is_sequence);
    }

    #[test]
    fn test_parse_epoch() {
        let pos = parse_position("epoch:1705315800").unwrap();
        assert_eq!(pos.position, 1705315800);
        assert!(!pos.is_sequence);
    }

    #[test]
    fn test_parse_timestamp_iso() {
        let pos = parse_position("timestamp:2024-01-15T10:30:00Z").unwrap();
        // 2024-01-15T10:30:00Z = 1705314600 Unix seconds
        assert_eq!(pos.position, 1705314600);
        assert!(!pos.is_sequence);
    }

    #[test]
    fn test_sequence_priority_over_timestamp() {
        // If both formats could match (they can't syntactically, but test priority)
        let pos = parse_position("sequence:100").unwrap();
        assert!(pos.is_sequence);
    }

    #[test]
    fn test_invalid_instruction() {
        assert!(parse_position("invalid").is_none());
        assert!(parse_position("sequence:").is_none());
        assert!(parse_position("sequence:abc").is_none());
    }

    #[test]
    fn test_position_info_constructors() {
        let seq = PositionInfo::sequence(100);
        assert_eq!(seq.position, 100);
        assert!(seq.is_sequence);

        let ts = PositionInfo::timestamp(1705315800);
        assert_eq!(ts.position, 1705315800);
        assert!(!ts.is_sequence);
    }

    // ==========================================================================
    // HYBRID POSITION INFO TESTS
    // ==========================================================================

    #[test]
    fn test_parse_hybrid_session_sequence() {
        let hybrid = parse_hybrid_position("session:abc123 sequence:42");
        assert_eq!(hybrid.session_id, Some("abc123".to_string()));
        assert_eq!(hybrid.position, 42);
        assert!(hybrid.is_sequence);
    }

    #[test]
    fn test_parse_hybrid_session_timestamp() {
        let hybrid = parse_hybrid_position("session:sess-456 timestamp:2024-01-15T10:30:00Z");
        assert_eq!(hybrid.session_id, Some("sess-456".to_string()));
        assert_eq!(hybrid.position, 1705314600);
        assert!(!hybrid.is_sequence);
    }

    #[test]
    fn test_parse_hybrid_reversed_order() {
        // Order shouldn't matter
        let hybrid = parse_hybrid_position("sequence:100 session:my-session");
        assert_eq!(hybrid.session_id, Some("my-session".to_string()));
        assert_eq!(hybrid.position, 100);
        assert!(hybrid.is_sequence);
    }

    #[test]
    fn test_parse_hybrid_no_session() {
        // Legacy format without session
        let hybrid = parse_hybrid_position("sequence:42");
        assert_eq!(hybrid.session_id, None);
        assert_eq!(hybrid.position, 42);
        assert!(hybrid.is_sequence);
    }

    #[test]
    fn test_parse_hybrid_uuid_session() {
        let hybrid = parse_hybrid_position("session:a1b2c3d4-e5f6-7890-abcd-ef1234567890 sequence:1");
        assert_eq!(
            hybrid.session_id,
            Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string())
        );
        assert_eq!(hybrid.position, 1);
    }

    #[test]
    fn test_parse_hybrid_empty_session() {
        // Empty session: should be None
        let hybrid = parse_hybrid_position("session: sequence:42");
        assert_eq!(hybrid.session_id, None);
        assert_eq!(hybrid.position, 42);
    }

    #[test]
    fn test_hybrid_position_info_constructors() {
        // Test with_session_sequence
        let hybrid = HybridPositionInfo::with_session_sequence("test-session", 123);
        assert_eq!(hybrid.session_id, Some("test-session".to_string()));
        assert_eq!(hybrid.position, 123);
        assert!(hybrid.is_sequence);

        // Test without_session
        let hybrid = HybridPositionInfo::without_session(100, true);
        assert_eq!(hybrid.session_id, None);
        assert_eq!(hybrid.position, 100);
        assert!(hybrid.is_sequence);

        // Test from_position_info
        let pos = PositionInfo::sequence(50);
        let hybrid = HybridPositionInfo::from_position_info(pos, Some("sess".to_string()));
        assert_eq!(hybrid.session_id, Some("sess".to_string()));
        assert_eq!(hybrid.position, 50);
    }

    #[test]
    fn test_extract_session_id() {
        // Basic extraction
        assert_eq!(extract_session_id("session:abc123"), Some("abc123".to_string()));

        // With other parts
        assert_eq!(
            extract_session_id("session:abc123 sequence:42"),
            Some("abc123".to_string())
        );

        // No session
        assert_eq!(extract_session_id("sequence:42"), None);

        // Empty session
        assert_eq!(extract_session_id("session: something"), None);

        // Multiple spaces
        assert_eq!(
            extract_session_id("  session:test  sequence:1  "),
            Some("test".to_string())
        );
    }
}
