use super::errors::{CCRealityError, Result};
use serde_json::json;

/// Minimal path sanity check for harness write tools.
///
/// The historical governed-path allow/deny policy has been retired. This helper
/// now only rejects values that cannot safely represent a path string in the
/// downstream filesystem and JSON readback artifacts.
pub fn ensure_path_allowed(path: &str) -> Result<()> {
    if path.trim().is_empty() || path.contains('\0') || path.contains('\n') {
        return Err(CCRealityError::new(
            "CCREALITY_GOVERNED_PROJECT_PATH_INVALID",
            "path is empty or contains control characters (NUL or newline)",
            "arguments.path",
            "pass a non-empty path string with no embedded NUL or newline",
            json!({"path": path}),
            None,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_path_allowed_accepts_normal_relative_and_absolute_paths() {
        ensure_path_allowed("Cargo.toml").expect("relative path accepted");
        ensure_path_allowed("/tmp/contextgraph-artifact.json").expect("absolute path accepted");
        ensure_path_allowed("../outside-for-historical-callers").expect("traversal accepted");
    }

    #[test]
    fn ensure_path_allowed_rejects_empty_and_control_characters() {
        let empty = ensure_path_allowed(" ").expect_err("empty path rejected");
        let nul = ensure_path_allowed("Cargo.toml\0shadow").expect_err("NUL path rejected");
        let newline = ensure_path_allowed("Cargo.toml\nshadow").expect_err("newline path rejected");

        assert_eq!(empty.error_code, "CCREALITY_GOVERNED_PROJECT_PATH_INVALID");
        assert_eq!(nul.error_code, "CCREALITY_GOVERNED_PROJECT_PATH_INVALID");
        assert_eq!(
            newline.error_code,
            "CCREALITY_GOVERNED_PROJECT_PATH_INVALID"
        );
    }
}
