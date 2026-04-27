//! Error types for hook commands
//!
//! # Exit Codes (TECH-HOOKS.md Section 3.2)
//!
//! | Code | Meaning | Description |
//! |------|---------|-------------|
//! | 0 | Success | Hook executed successfully |
//! | 1 | General Error | Unspecified error |
//! | 4 | Invalid Input | Malformed input data |
//!
//! # NO BACKWARDS COMPATIBILITY
//! This module FAILS FAST on any error. Do not add fallback logic.

use thiserror::Error;

/// Hook-specific error types
#[derive(Debug, Error)]
pub enum HookError {
    /// Invalid or malformed input data
    /// Exit code: 4
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// JSON serialization/deserialization error
    /// Exit code: 4
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO operation failed
    /// Exit code: 1
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// General/unspecified error
    /// Exit code: 1
    #[error("{0}")]
    General(String),
}

impl HookError {
    /// Convert to exit code per TECH-HOOKS.md section 3.2
    #[inline]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidInput(_) | Self::Serialization(_) => 4,
            Self::Io(_) | Self::General(_) => 1,
        }
    }

    /// Check if this error is recoverable
    #[inline]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Io(_))
    }

    /// Get error code string (e.g., "ERR_INVALID_INPUT")
    #[inline]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "ERR_INVALID_INPUT",
            Self::Serialization(_) => "ERR_SERIALIZATION",
            Self::Io(_) => "ERR_IO",
            Self::General(_) => "ERR_GENERAL",
        }
    }

    /// Convert to structured JSON error for shell script consumption
    pub fn to_json_error(&self) -> serde_json::Value {
        serde_json::json!({
            "error": true,
            "code": self.error_code(),
            "exit_code": self.exit_code(),
            "message": self.to_string(),
            "recoverable": self.is_recoverable(),
        })
    }

    /// Create invalid input error
    #[inline]
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }
}

#[cfg(test)]
impl HookError {
    /// Create general error (test helper)
    pub fn general(message: impl Into<String>) -> Self {
        Self::General(message.into())
    }
}

impl From<String> for HookError {
    fn from(s: String) -> Self {
        Self::General(s)
    }
}

impl From<&str> for HookError {
    fn from(s: &str) -> Self {
        Self::General(s.to_string())
    }
}

/// Result type for hook operations
pub type HookResult<T> = Result<T, HookError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_codes_match_spec() {
        assert_eq!(HookError::invalid_input("bad data").exit_code(), 4);
        assert_eq!(HookError::general("something").exit_code(), 1);
    }

    #[test]
    fn test_serialization_error_exit_code() {
        let json_err = serde_json::from_str::<String>("invalid json");
        if let Err(e) = json_err {
            let hook_err = HookError::from(e);
            assert_eq!(hook_err.exit_code(), 4);
        } else {
            panic!("Expected JSON parse error");
        }
    }

    #[test]
    fn test_io_error_exit_code() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let hook_err = HookError::from(io_err);
        assert_eq!(hook_err.exit_code(), 1);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            HookError::invalid_input("x").error_code(),
            "ERR_INVALID_INPUT"
        );
        assert_eq!(HookError::general("x").error_code(), "ERR_GENERAL");
    }

    #[test]
    fn test_serialization_error_code() {
        let json_err = serde_json::from_str::<String>("{}");
        if let Err(e) = json_err {
            let hook_err = HookError::from(e);
            assert_eq!(hook_err.error_code(), "ERR_SERIALIZATION");
        }
    }

    #[test]
    fn test_io_error_code() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let hook_err = HookError::from(io_err);
        assert_eq!(hook_err.error_code(), "ERR_IO");
    }

    #[test]
    fn test_is_recoverable() {
        assert!(!HookError::invalid_input("bad").is_recoverable());
        assert!(!HookError::general("x").is_recoverable());
    }

    #[test]
    fn test_io_is_recoverable() {
        let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
        let hook_err = HookError::from(io_err);
        assert!(hook_err.is_recoverable());
    }

    #[test]
    fn test_to_json_error() {
        let err = HookError::invalid_input("bad field");
        let json = err.to_json_error();
        assert_eq!(json["error"], true);
        assert_eq!(json["code"], "ERR_INVALID_INPUT");
        assert_eq!(json["exit_code"], 4);
        assert_eq!(json["recoverable"], false);
    }

    #[test]
    fn test_from_string() {
        let err: HookError = String::from("test error").into();
        assert!(matches!(err, HookError::General(_)));
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn test_from_str() {
        let err: HookError = "test error".into();
        assert!(matches!(err, HookError::General(_)));
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            HookError::invalid_input("missing field").to_string(),
            "Invalid input: missing field"
        );
    }

    #[test]
    fn test_hook_result_type() {
        fn returns_result() -> HookResult<i32> {
            Ok(42)
        }
        fn returns_error() -> HookResult<i32> {
            Err(HookError::general("err"))
        }
        assert_eq!(returns_result().unwrap(), 42);
        assert!(returns_error().is_err());
    }
}
