//! CLI exit code handling per AP-26 constitution.
//!
//! Exit codes:
//! - 0: Success (stdout to Claude)
//! - 1: Recoverable error (stderr to user)
//! - 2: Blocking failure - corruption ONLY (stderr to Claude)
//!
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.

use context_graph_storage::StorageError;
use std::process::ExitCode;

/// Exit codes for CLI commands per AP-26 constitution.
///
/// # Claude Code Integration
///
/// Claude Code hooks interpret exit codes:
/// - Exit 0: Success, stdout captured
/// - Exit 1: Warning, stderr shown to user
/// - Exit 2: BLOCKING - stderr shown to Claude, action blocked
///
/// Exit 2 is ONLY for corruption scenarios where proceeding would be dangerous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CliExitCode {
    /// Success - stdout to Claude
    Success = 0,
    /// Recoverable error - stderr to user, does not block
    Warning = 1,
    /// Blocking failure - stderr to Claude, blocks action
    /// ONLY for corruption scenarios
    Blocking = 2,
}

impl From<CliExitCode> for ExitCode {
    fn from(code: CliExitCode) -> Self {
        ExitCode::from(code as u8)
    }
}

impl From<CliExitCode> for i32 {
    fn from(code: CliExitCode) -> Self {
        code as i32
    }
}

impl From<&StorageError> for CliExitCode {
    fn from(err: &StorageError) -> Self {
        match err {
            // Blocking errors (corruption) - Exit 2
            StorageError::IndexCorrupted { .. } => CliExitCode::Blocking,
            StorageError::Serialization(msg) if is_corruption_indicator(msg) => {
                CliExitCode::Blocking
            }
            StorageError::ReadFailed(msg) if is_corruption_indicator(msg) => CliExitCode::Blocking,

            // NotFound is NOT an error - fresh install is valid
            StorageError::NotFound { .. } => CliExitCode::Success,

            // All other errors are recoverable
            StorageError::OpenFailed { .. } => CliExitCode::Warning,
            StorageError::ColumnFamilyNotFound { .. } => CliExitCode::Warning,
            StorageError::WriteFailed(_) => CliExitCode::Warning,
            StorageError::ReadFailed(_) => CliExitCode::Warning,
            StorageError::FlushFailed(_) => CliExitCode::Warning,
            StorageError::Serialization(_) => CliExitCode::Warning,
            StorageError::ValidationFailed(_) => CliExitCode::Warning,
            StorageError::Internal(_) => CliExitCode::Warning,
        }
    }
}

/// Check if error message indicates corruption.
///
/// Used for string-based error classification when we don't have
/// typed error variants for specific corruption scenarios.
#[inline]
pub fn is_corruption_indicator(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    CORRUPTION_INDICATORS
        .iter()
        .any(|&indicator| lower.contains(indicator))
}

/// Corruption indicator strings (lowercase).
/// SINGLE SOURCE OF TRUTH for all corruption detection across the crate.
/// H4/M9 FIX: Only RocksDB-specific corruption terms.
/// "invalid" and "malformed" removed — they match normal JSON-RPC errors.
/// "not found" never belonged here — it's a normal condition.
/// TST-H3 FIX: "stale index" added (was only in mcp_helpers, now unified here).
pub const CORRUPTION_INDICATORS: &[&str] = &[
    "corruption",
    "corrupted",
    "stale index",
    "checksum mismatch",
    "bad magic",
    "crc error",
    "truncated block",
    "bad table magic number",
    "block checksum",
];

/// Determine exit code for any error by inspecting error message.
///
/// This is a fallback for errors that don't have typed variants.
/// Prefer using typed error conversion when possible.
///
/// # Arguments
/// * `e` - Any error implementing std::error::Error + 'static
///
/// # Returns
/// - `CliExitCode::Blocking` if error message indicates corruption
/// - `CliExitCode::Warning` otherwise
pub fn exit_code_for_error(e: &(dyn std::error::Error + 'static)) -> CliExitCode {
    // Try downcasting to StorageError first
    if let Some(storage_err) = e.downcast_ref::<StorageError>() {
        return CliExitCode::from(storage_err);
    }

    // Fallback: check error message for corruption indicators
    let msg = e.to_string();
    if is_corruption_indicator(&msg) {
        CliExitCode::Blocking
    } else {
        CliExitCode::Warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // TC-SESSION-22: Exit Code Value Mapping
    // Source of Truth: AP-26 constitution requirement
    // =========================================================================
    #[test]
    fn tc_session_22_exit_code_values() {
        println!("\n=== TC-SESSION-22: Exit Code Values ===");
        println!("SOURCE OF TRUTH: AP-26 constitution");

        println!("BEFORE: Creating CliExitCode variants");
        let success = CliExitCode::Success;
        let warning = CliExitCode::Warning;
        let blocking = CliExitCode::Blocking;

        println!("AFTER: Checking numeric values");
        assert_eq!(success as u8, 0, "Success must be 0");
        assert_eq!(warning as u8, 1, "Warning must be 1");
        assert_eq!(blocking as u8, 2, "Blocking must be 2");

        println!("EVIDENCE: Success=0, Warning=1, Blocking=2");
        println!("RESULT: PASS - Exit code values match AP-26");
    }

    // =========================================================================
    // TC-SESSION-22b: StorageError to CliExitCode Conversion
    // Source of Truth: StorageError variants
    // =========================================================================
    #[test]
    fn tc_session_22b_storage_error_conversion() {
        println!("\n=== TC-SESSION-22b: StorageError Conversion ===");
        println!("SOURCE OF TRUTH: StorageError variants");

        // Blocking errors (corruption)
        let corruption = StorageError::IndexCorrupted {
            index_name: "test".to_string(),
            details: "test".to_string(),
        };
        println!("BEFORE: IndexCorrupted error");
        let code = CliExitCode::from(&corruption);
        println!("AFTER: exit_code={:?}", code);
        assert_eq!(
            code,
            CliExitCode::Blocking,
            "IndexCorrupted must return Blocking"
        );

        // NotFound is NOT an error
        let not_found = StorageError::NotFound {
            id: "test".to_string(),
        };
        println!("BEFORE: NotFound error");
        let code = CliExitCode::from(&not_found);
        println!("AFTER: exit_code={:?}", code);
        assert_eq!(
            code,
            CliExitCode::Success,
            "NotFound must return Success (fresh install)"
        );

        // Other errors are Warning
        let io_err = StorageError::WriteFailed("disk full".to_string());
        println!("BEFORE: WriteFailed error");
        let code = CliExitCode::from(&io_err);
        println!("AFTER: exit_code={:?}", code);
        assert_eq!(
            code,
            CliExitCode::Warning,
            "WriteFailed must return Warning"
        );

        println!("EVIDENCE: IndexCorrupted=Blocking, NotFound=Success, WriteFailed=Warning");
        println!("RESULT: PASS - StorageError conversion correct");
    }

    // =========================================================================
    // TC-SESSION-22c: Corruption Indicator Detection
    // Source of Truth: CORRUPTION_INDICATORS constant
    // =========================================================================
    #[test]
    fn tc_session_22c_corruption_indicators() {
        println!("\n=== TC-SESSION-22c: Corruption Indicators ===");
        println!("SOURCE OF TRUTH: CORRUPTION_INDICATORS constant");

        let corruption_messages = [
            ("data corruption detected", true),
            ("stale index found", true), // TST-H3: now in unified CORRUPTION_INDICATORS
            ("checksum mismatch", true),
            ("bad magic number", true),
            ("CRC ERROR in block", true),
            ("truncated block at offset 1024", true),
            ("bad table magic number", true),
            ("block checksum failed", true),
        ];

        // H4/M9 FIX: "invalid", "malformed", "not found" are NOT corruption
        let non_corruption_messages = [
            ("connection refused", false),
            ("timeout error", false),
            ("file not found", false),
            ("permission denied", false),
            ("disk full", false),
            ("invalid request", false),
            ("invalid json format", false),
            ("malformed entry", false),
            ("truncated file", false),
            ("memory not found", false),
        ];

        for (msg, expected) in corruption_messages {
            let result = is_corruption_indicator(msg);
            println!("  '{}': corruption={} (expected={})", msg, result, expected);
            assert_eq!(
                result, expected,
                "Message '{}' should be corruption={}",
                msg, expected
            );
        }

        for (msg, expected) in non_corruption_messages {
            let result = is_corruption_indicator(msg);
            println!("  '{}': corruption={} (expected={})", msg, result, expected);
            assert_eq!(
                result, expected,
                "Message '{}' should be corruption={}",
                msg, expected
            );
        }

        println!("RESULT: PASS - Corruption indicators detected correctly");
    }

    // =========================================================================
    // TC-SESSION-22d: ExitCode Conversion
    // Source of Truth: std::process::ExitCode
    // =========================================================================
    #[test]
    fn tc_session_22d_exit_code_conversion() {
        println!("\n=== TC-SESSION-22d: ExitCode Conversion ===");

        // Test i32 conversion
        assert_eq!(i32::from(CliExitCode::Success), 0);
        assert_eq!(i32::from(CliExitCode::Warning), 1);
        assert_eq!(i32::from(CliExitCode::Blocking), 2);

        // Test std::process::ExitCode conversion (can't inspect value, just verify compilation)
        let _exit: ExitCode = CliExitCode::Success.into();
        let _exit: ExitCode = CliExitCode::Warning.into();
        let _exit: ExitCode = CliExitCode::Blocking.into();

        println!("RESULT: PASS - ExitCode conversions compile and work");
    }

    // =========================================================================
    // EDGE CASE 1: Empty error message
    // =========================================================================
    #[test]
    fn edge_case_empty_message() {
        println!("\n=== EDGE CASE 1: Empty Message ===");
        println!("BEFORE: Empty string");
        let result = is_corruption_indicator("");
        println!("AFTER: corruption={}", result);
        assert!(!result, "Empty message should not indicate corruption");
        println!("RESULT: PASS - Empty message handled");
    }

    // =========================================================================
    // EDGE CASE 2: Unicode in error message
    // =========================================================================
    #[test]
    fn edge_case_unicode_message() {
        println!("\n=== EDGE CASE 2: Unicode Message ===");
        let msg = "错误: corruption detected 🔥";
        println!("BEFORE: '{}'", msg);
        let result = is_corruption_indicator(msg);
        println!("AFTER: corruption={}", result);
        assert!(result, "Should detect corruption even with unicode");
        println!("RESULT: PASS - Unicode handled correctly");
    }

    // =========================================================================
    // EDGE CASE 3: Case sensitivity
    // =========================================================================
    #[test]
    fn edge_case_case_sensitivity() {
        println!("\n=== EDGE CASE 3: Case Sensitivity ===");

        let test_cases = [
            "CORRUPTION",
            "Corruption",
            "corruption",
            "CHECKSUM MISMATCH",
            "Checksum Mismatch",
        ];

        for msg in test_cases {
            let result = is_corruption_indicator(msg);
            println!("  '{}': corruption={}", msg, result);
            assert!(result, "Should detect corruption regardless of case");
        }

        println!("RESULT: PASS - Case insensitive detection works");
    }

    // =========================================================================
    // Test exit_code_for_error with StorageError
    // =========================================================================
    #[test]
    fn test_exit_code_for_storage_error() {
        println!("\n=== Test exit_code_for_error with StorageError ===");

        let index_err = StorageError::IndexCorrupted {
            index_name: "test".to_string(),
            details: "corruption".to_string(),
        };
        let code = exit_code_for_error(&index_err);
        assert_eq!(code, CliExitCode::Blocking);

        let not_found = StorageError::NotFound {
            id: "test".to_string(),
        };
        let code = exit_code_for_error(&not_found);
        assert_eq!(code, CliExitCode::Success);

        let write_err = StorageError::WriteFailed("disk full".to_string());
        let code = exit_code_for_error(&write_err);
        assert_eq!(code, CliExitCode::Warning);

        println!("RESULT: PASS - exit_code_for_error works with StorageError");
    }

    // =========================================================================
    // Test exit_code_for_error with generic error (string-based detection)
    // =========================================================================
    #[test]
    fn test_exit_code_for_generic_error() {
        println!("\n=== Test exit_code_for_error with generic error ===");

        // Use std::io::Error as a generic error type
        let corruption_err =
            std::io::Error::new(std::io::ErrorKind::InvalidData, "data corruption detected");
        let code = exit_code_for_error(&corruption_err);
        println!("  'data corruption detected': exit_code={:?}", code);
        assert_eq!(code, CliExitCode::Blocking);

        let normal_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let code = exit_code_for_error(&normal_err);
        println!("  'file not found': exit_code={:?}", code);
        assert_eq!(code, CliExitCode::Warning);

        println!("RESULT: PASS - exit_code_for_error works with generic errors");
    }

    // =========================================================================
    // Test Serialization with corruption indicator in message
    // =========================================================================
    #[test]
    fn test_serialization_with_corruption() {
        println!("\n=== Test Serialization with corruption indicator ===");

        // Serialization with corruption indicator
        let err = StorageError::Serialization("data corruption in record".to_string());
        let code = CliExitCode::from(&err);
        println!(
            "  Serialization('data corruption in record'): exit_code={:?}",
            code
        );
        assert_eq!(code, CliExitCode::Blocking);

        // H4/M9 FIX: "invalid" is NOT a corruption indicator — it's normal for parse errors
        let err = StorageError::Serialization("invalid json format".to_string());
        let code = CliExitCode::from(&err);
        println!(
            "  Serialization('invalid json format'): exit_code={:?}",
            code
        );
        assert_eq!(code, CliExitCode::Warning); // Parse errors are recoverable, not corruption

        // Serialization with no indicators
        let err = StorageError::Serialization("unexpected end of input".to_string());
        let code = CliExitCode::from(&err);
        println!(
            "  Serialization('unexpected end of input'): exit_code={:?}",
            code
        );
        assert_eq!(code, CliExitCode::Warning);

        println!("RESULT: PASS - Serialization errors classified correctly");
    }

    // =========================================================================
    // Test ReadFailed with corruption indicator in message
    // =========================================================================
    #[test]
    fn test_read_failed_with_corruption() {
        println!("\n=== Test ReadFailed with corruption indicator ===");

        // ReadFailed with corruption indicator
        let err = StorageError::ReadFailed("checksum mismatch".to_string());
        let code = CliExitCode::from(&err);
        println!("  ReadFailed('checksum mismatch'): exit_code={:?}", code);
        assert_eq!(code, CliExitCode::Blocking);

        // ReadFailed without corruption indicator
        let err = StorageError::ReadFailed("io error: connection reset".to_string());
        let code = CliExitCode::from(&err);
        println!(
            "  ReadFailed('io error: connection reset'): exit_code={:?}",
            code
        );
        assert_eq!(code, CliExitCode::Warning);

        println!("RESULT: PASS - ReadFailed errors classified correctly");
    }

    // =========================================================================
    // Comprehensive StorageError variant coverage
    // =========================================================================
    #[test]
    fn test_all_storage_error_variants() {
        println!("\n=== Test all StorageError variants ===");

        let variants: Vec<(StorageError, CliExitCode, &str)> = vec![
            (
                StorageError::OpenFailed {
                    path: "test".into(),
                    message: "err".into(),
                },
                CliExitCode::Warning,
                "OpenFailed",
            ),
            (
                StorageError::ColumnFamilyNotFound {
                    name: "test".into(),
                },
                CliExitCode::Warning,
                "ColumnFamilyNotFound",
            ),
            (
                StorageError::WriteFailed("err".into()),
                CliExitCode::Warning,
                "WriteFailed",
            ),
            (
                StorageError::ReadFailed("err".into()),
                CliExitCode::Warning,
                "ReadFailed (normal)",
            ),
            (
                StorageError::FlushFailed("err".into()),
                CliExitCode::Warning,
                "FlushFailed",
            ),
            (
                StorageError::NotFound { id: "test".into() },
                CliExitCode::Success,
                "NotFound",
            ),
            (
                StorageError::Serialization("err".into()),
                CliExitCode::Warning,
                "Serialization (normal)",
            ),
            (
                StorageError::ValidationFailed("err".into()),
                CliExitCode::Warning,
                "ValidationFailed",
            ),
            (
                StorageError::IndexCorrupted {
                    index_name: "test".into(),
                    details: "err".into(),
                },
                CliExitCode::Blocking,
                "IndexCorrupted",
            ),
            (
                StorageError::Internal("err".into()),
                CliExitCode::Warning,
                "Internal",
            ),
        ];

        for (err, expected, name) in variants {
            let code = CliExitCode::from(&err);
            println!("  {}: exit_code={:?} (expected={:?})", name, code, expected);
            assert_eq!(code, expected, "{} should return {:?}", name, expected);
        }

        println!("RESULT: PASS - All StorageError variants classified correctly");
    }
}
