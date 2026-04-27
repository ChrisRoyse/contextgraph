//! DTOs for provenance query tools (Phase P3).
//!
//! MCP-L11: Extracted from provenance_tools.rs inline definitions.

use serde::{Deserialize, Deserializer};

/// Parameters for get_audit_trail tool.
#[derive(Debug, Deserialize)]
pub struct GetAuditTrailParams {
    pub target_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    #[serde(
        default = "default_audit_limit",
        deserialize_with = "deserialize_usize_lenient"
    )]
    pub limit: usize,
}

/// Default audit record limit.
pub fn default_audit_limit() -> usize {
    50
}

/// MCP clients may send integers as JSON strings -- accept both.
pub fn deserialize_usize_lenient<'de, D: Deserializer<'de>>(d: D) -> Result<usize, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    match &v {
        serde_json::Value::Number(n) => n
            .as_u64()
            .map(|n| n as usize)
            .ok_or_else(|| serde::de::Error::custom("limit must be a non-negative integer")),
        serde_json::Value::String(s) => s.parse::<usize>().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom(
            "limit must be an integer or string",
        )),
    }
}

/// Parameters for get_merge_history tool.
#[derive(Debug, Deserialize)]
pub struct GetMergeHistoryParams {
    pub memory_id: String,
    #[serde(default)]
    pub include_source_metadata: bool,
}

/// Parameters for get_provenance_chain tool.
#[derive(Debug, Deserialize)]
pub struct GetProvenanceChainParams {
    pub memory_id: String,
    #[serde(default)]
    pub include_audit: bool,
    #[serde(default)]
    pub include_embedding_version: bool,
    #[serde(default)]
    pub include_importance_history: bool,
    #[serde(default)]
    pub include_merge_history: bool,
    /// When true, includes ALL provenance data (audit, embedding, importance, merge).
    #[serde(default)]
    pub depth_full: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_trail_params_defaults() {
        let params: GetAuditTrailParams = serde_json::from_str("{}").unwrap();
        assert_eq!(params.limit, 50);
        assert!(params.target_id.is_none());
    }

    #[test]
    fn test_audit_trail_params_string_limit() {
        let params: GetAuditTrailParams = serde_json::from_str(r#"{"limit": "25"}"#).unwrap();
        assert_eq!(params.limit, 25);
    }

    #[test]
    fn test_merge_history_params_defaults() {
        let params: GetMergeHistoryParams =
            serde_json::from_str(r#"{"memory_id": "test-uuid"}"#).unwrap();
        assert!(!params.include_source_metadata);
    }

    #[test]
    fn test_provenance_chain_params_depth_full() {
        let params: GetProvenanceChainParams =
            serde_json::from_str(r#"{"memory_id": "test-uuid", "depth_full": true}"#).unwrap();
        assert!(params.depth_full);
        assert!(!params.include_audit); // depth_full is handled in handler, not DTO
    }
}
