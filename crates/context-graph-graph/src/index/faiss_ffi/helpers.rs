//! Helper functions for FAISS FFI operations.
//!
//! This module provides utility functions for working with
//! FAISS return codes and error handling.
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004 Section 3.1: FAISS FFI Bindings

use crate::error::{GraphError, GraphResult};
use std::os::raw::c_int;

/// Check FAISS result code and convert to GraphResult.
///
/// # Arguments
///
/// - `code`: FAISS return code (0 = success)
/// - `operation`: Description of operation for error message
///
/// # Returns
///
/// - `Ok(())` if code is 0
/// - `Err(GraphError::FaissIndexCreation)` otherwise
///
/// # Example
///
/// ```ignore
/// let result = unsafe { faiss_Index_train(index, n, x) };
/// check_faiss_result(result, "faiss_Index_train")?;
/// ```
#[inline]
pub fn check_faiss_result(code: c_int, operation: &str) -> GraphResult<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(GraphError::FaissIndexCreation(format!(
            "{} failed with error code: {}",
            operation, code
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_faiss_result_success() {
        let result = check_faiss_result(0, "test_operation");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_faiss_result_failure() {
        let result = check_faiss_result(-1, "test_operation");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, GraphError::FaissIndexCreation(_)));
        let msg = err.to_string();
        assert!(msg.contains("test_operation"));
        assert!(msg.contains("-1"));
    }

    #[test]
    fn test_check_faiss_result_various_codes() {
        // Test multiple error codes
        for code in [1, 2, 10, 100, -100] {
            let result = check_faiss_result(code, "test");
            assert!(result.is_err());
            let msg = result.unwrap_err().to_string();
            assert!(msg.contains(&code.to_string()));
        }
    }
}
