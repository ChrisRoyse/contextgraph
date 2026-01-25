//! MCP JSON-RPC protocol types.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC ID (can be string, number, or null).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum JsonRpcId {
    String(String),
    Number(i64),
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    /// Create a success response.
    pub fn success(id: Option<JsonRpcId>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<JsonRpcId>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// Standard JSON-RPC error codes.
///
/// TASK-S001: Added teleological-specific error codes for TeleologicalMemoryStore operations.
#[allow(dead_code)]
pub mod error_codes {
    // Standard JSON-RPC 2.0 error codes
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Context Graph specific error codes (-32001 to -32099)
    pub const FEATURE_DISABLED: i32 = -32001;
    pub const NODE_NOT_FOUND: i32 = -32002;
    pub const PAYLOAD_TOO_LARGE: i32 = -32003;
    pub const STORAGE_ERROR: i32 = -32004;
    pub const EMBEDDING_ERROR: i32 = -32005;
    pub const TOOL_NOT_FOUND: i32 = -32006;
    pub const LAYER_TIMEOUT: i32 = -32007;
    /// Index operation failed (HNSW, inverted index, dimension mismatch) - TASK-CORE-014
    pub const INDEX_ERROR: i32 = -32008;
    /// GPU/CUDA operation failed (memory allocation, kernel execution) - TASK-CORE-014
    pub const GPU_ERROR: i32 = -32009;

    // Teleological-specific error codes (-32010 to -32019) - TASK-S001
    /// TeleologicalFingerprint not found by UUID
    pub const FINGERPRINT_NOT_FOUND: i32 = -32010;
    /// Multi-array embedding provider not ready (13 embedders)
    pub const EMBEDDER_NOT_READY: i32 = -32011;
    /// Purpose vector computation failed
    pub const PURPOSE_COMPUTATION_ERROR: i32 = -32012;
    /// Embedder category classification failed
    pub const CATEGORY_CLASSIFICATION_ERROR: i32 = -32013;
    /// Sparse search (SPLADE E13) failed
    pub const SPARSE_SEARCH_ERROR: i32 = -32014;
    /// Semantic search (13-embedding) failed
    pub const SEMANTIC_SEARCH_ERROR: i32 = -32015;
    /// Purpose alignment search failed
    pub const PURPOSE_SEARCH_ERROR: i32 = -32016;
    /// Checkpoint/restore operation failed
    pub const CHECKPOINT_ERROR: i32 = -32017;
    /// Batch operation failed
    pub const BATCH_OPERATION_ERROR: i32 = -32018;
    /// Tool not yet implemented - FAIL FAST per AP-007
    pub const TOOL_NOT_IMPLEMENTED: i32 = -32019;

    // Goal/alignment specific error codes (-32020 to -32029) - TASK-S003
    /// Goal not found in hierarchy
    pub const GOAL_NOT_FOUND: i32 = -32020;
    /// Insufficient memories for topic detection (< min_cluster_size)
    /// Per constitution clustering.parameters.min_cluster_size: 3
    pub const INSUFFICIENT_MEMORIES: i32 = -32021;
    /// Alignment computation failed
    pub const ALIGNMENT_COMPUTATION_ERROR: i32 = -32022;
    /// Goal hierarchy operation failed
    pub const GOAL_HIERARCHY_ERROR: i32 = -32023;

    // Meta-UTL error codes (-32040 to -32049) - TASK-S005
    /// Prediction not found for validation
    pub const META_UTL_PREDICTION_NOT_FOUND: i32 = -32040;
    /// Meta-UTL not initialized
    pub const META_UTL_NOT_INITIALIZED: i32 = -32041;
    /// Insufficient data for prediction
    pub const META_UTL_INSUFFICIENT_DATA: i32 = -32042;
    /// Invalid outcome format
    pub const META_UTL_INVALID_OUTCOME: i32 = -32043;
    /// Trajectory computation failed
    pub const META_UTL_TRAJECTORY_ERROR: i32 = -32044;
    /// Health metrics failed
    pub const META_UTL_HEALTH_ERROR: i32 = -32045;

    // Monitoring error codes (-32050 to -32059) - TASK-EMB-024
    /// SystemMonitor not configured or returned error
    pub const SYSTEM_MONITOR_ERROR: i32 = -32050;
    /// LayerStatusProvider not configured or returned error
    pub const LAYER_STATUS_ERROR: i32 = -32051;
    /// Pipeline breakdown metrics not yet implemented
    pub const PIPELINE_METRICS_UNAVAILABLE: i32 = -32052;

    // Coherence error codes (-32060 to -32069)
    // Note: Use topic-based coherence per PRD v6
    /// Coherence system not initialized or unavailable
    pub const COHERENCE_NOT_INITIALIZED: i32 = -32060;
    /// Coherence network error (step failed, invalid phase, etc.)
    pub const COHERENCE_ERROR: i32 = -32061;
    /// Workspace selection or broadcast error
    pub const WORKSPACE_ERROR: i32 = -32063;
    /// State machine transition error
    pub const STATE_TRANSITION_ERROR: i32 = -32064;
    /// Meta-cognitive evaluation failed
    pub const META_COGNITIVE_ERROR: i32 = -32065;
    /// Topic profile operation failed
    pub const TOPIC_PROFILE_ERROR: i32 = -32066;
    /// Topic stability check failed
    pub const TOPIC_STABILITY_ERROR: i32 = -32067;

    // GPU monitoring error codes (-32075 to -32079)
    /// GpuMonitor not initialized - use with_gpu_monitor() or with_default_gwt()
    pub const GPU_MONITOR_NOT_INITIALIZED: i32 = -32075;
    /// GPU utilization query failed
    pub const GPU_QUERY_FAILED: i32 = -32076;

    // Neuromodulation error codes (-32080 to -32089) - TASK-NEUROMOD-MCP
    /// Neuromodulation manager not initialized
    pub const NEUROMOD_NOT_INITIALIZED: i32 = -32080;
    /// Neuromodulator adjustment failed
    pub const NEUROMOD_ADJUSTMENT_ERROR: i32 = -32081;
    /// Acetylcholine is read-only
    pub const NEUROMOD_ACH_READ_ONLY: i32 = -32082;

    // Steering error codes (-32090 to -32099) - TASK-STEERING-001
    /// Steering system not initialized
    pub const STEERING_NOT_INITIALIZED: i32 = -32090;
    /// Steering feedback computation failed
    pub const STEERING_FEEDBACK_ERROR: i32 = -32091;
    /// Gardener component error
    pub const GARDENER_ERROR: i32 = -32092;
    /// Curator component error
    pub const CURATOR_ERROR: i32 = -32093;
    /// Assessor component error
    pub const ASSESSOR_ERROR: i32 = -32094;

    // Deprecated method error codes (JSON-RPC standard) - TASK-CORE-001
    /// Deprecated method - functionality removed per ARCH-03 (autonomous-first)
    /// Same as METHOD_NOT_FOUND per JSON-RPC spec.
    pub const DEPRECATED_METHOD: i32 = -32601;

    // Causal inference error codes (-32100 to -32109) - TASK-CAUSAL-001
    /// Causal inference engine not initialized
    pub const CAUSAL_NOT_INITIALIZED: i32 = -32100;
    /// Invalid inference direction
    pub const CAUSAL_INVALID_DIRECTION: i32 = -32101;
    /// Causal inference failed
    pub const CAUSAL_INFERENCE_ERROR: i32 = -32102;
    /// Target node required for this direction
    pub const CAUSAL_TARGET_REQUIRED: i32 = -32103;
    /// Causal graph operation failed
    pub const CAUSAL_GRAPH_ERROR: i32 = -32104;

    // TCP Transport error codes (-32110 to -32119) - TASK-INTEG-018
    /// TCP bind failed - address/port unavailable or permission denied
    /// FAIL FAST: Server cannot start if bind fails
    pub const TCP_BIND_FAILED: i32 = -32110;
    /// TCP connection error - stream read/write failed, client disconnected
    pub const TCP_CONNECTION_ERROR: i32 = -32111;
    /// Maximum concurrent TCP connections reached
    /// Server rejects new connections when at capacity (configurable via max_connections)
    pub const TCP_MAX_CONNECTIONS_REACHED: i32 = -32112;
    /// TCP frame error - invalid NDJSON framing, message too large
    pub const TCP_FRAME_ERROR: i32 = -32113;
    /// TCP client timeout - request processing exceeded request_timeout
    pub const TCP_CLIENT_TIMEOUT: i32 = -32114;

    // Session lifecycle error codes (-32120 to -32129) - TASK-013
    /// Session not found - session_id does not exist
    pub const SESSION_NOT_FOUND: i32 = -32120;
    /// Session expired - session TTL exceeded
    pub const SESSION_EXPIRED: i32 = -32121;
    /// Session already exists - duplicate session_id
    pub const SESSION_EXISTS: i32 = -32122;
    /// No active session - must call session_start first
    pub const NO_ACTIVE_SESSION: i32 = -32123;

    // Drift history error codes (-32130 to -32139) - TASK-FIX-002/NORTH-010
    /// No drift history available for the specified goal
    /// FAIL FAST: Returns error rather than empty array when no history exists
    pub const HISTORY_NOT_AVAILABLE: i32 = -32130;
}

/// MCP method names.
#[allow(dead_code)]
pub mod methods {
    // MCP lifecycle methods
    pub const INITIALIZE: &str = "initialize";
    pub const SHUTDOWN: &str = "shutdown";

    // MCP tools protocol methods
    pub const TOOLS_LIST: &str = "tools/list";
    pub const TOOLS_CALL: &str = "tools/call";

    // Memory operations
    pub const MEMORY_STORE: &str = "memory/store";
    pub const MEMORY_RETRIEVE: &str = "memory/retrieve";
    pub const MEMORY_SEARCH: &str = "memory/search";
    pub const MEMORY_DELETE: &str = "memory/delete";

    // Memory injection and comparison operations (TASK-INTEG-001)
    /// Inject content with automatic 13-embedder fingerprint generation
    pub const MEMORY_INJECT: &str = "memory/inject";
    /// Batch injection with parallel embedding
    pub const MEMORY_INJECT_BATCH: &str = "memory/inject_batch";
    /// Multi-embedder perspective search with RRF fusion
    pub const MEMORY_SEARCH_MULTI_PERSPECTIVE: &str = "memory/search_multi_perspective";
    /// Single pair comparison using TeleologicalComparator
    pub const MEMORY_COMPARE: &str = "memory/compare";
    /// 1-to-N comparison using BatchComparator
    pub const MEMORY_BATCH_COMPARE: &str = "memory/batch_compare";
    /// N×N similarity matrix using BatchComparator::compare_all_pairs
    pub const MEMORY_SIMILARITY_MATRIX: &str = "memory/similarity_matrix";

    // Search operations (TASK-S002)
    pub const SEARCH_MULTI: &str = "search/multi";
    pub const SEARCH_SINGLE_SPACE: &str = "search/single_space";
    pub const SEARCH_BY_PURPOSE: &str = "search/by_purpose";
    pub const SEARCH_WEIGHT_PROFILES: &str = "search/weight_profiles";

    // Graph operations
    pub const GRAPH_CONNECT: &str = "graph/connect";
    pub const GRAPH_TRAVERSE: &str = "graph/traverse";

    // UTL operations
    pub const UTL_COMPUTE: &str = "utl/compute";
    pub const UTL_METRICS: &str = "utl/metrics";

    // System operations
    pub const SYSTEM_STATUS: &str = "system/status";
    pub const SYSTEM_HEALTH: &str = "system/health";

    // Purpose/goal operations (TASK-S003)
    /// Query memories by 13D purpose vector similarity
    pub const PURPOSE_QUERY: &str = "purpose/query";
    /// Navigate goal hierarchy (get_children, get_ancestors, get_subtree, get_aligned_memories)
    pub const GOAL_HIERARCHY_QUERY: &str = "goal/hierarchy_query";
    /// Find memories aligned to a specific goal
    pub const GOAL_ALIGNED_MEMORIES: &str = "goal/aligned_memories";
    /// Detect purpose drift in memories (to be refactored in TASK-LOGIC-010 to use teleological arrays)
    pub const PURPOSE_DRIFT_CHECK: &str = "purpose/drift_check";

    // Meta-UTL operations (TASK-S005)
    /// Get per-embedder learning trajectory and accuracy trends
    pub const META_UTL_LEARNING_TRAJECTORY: &str = "meta_utl/learning_trajectory";
    /// Get system health metrics with constitution.yaml targets
    pub const META_UTL_HEALTH_METRICS: &str = "meta_utl/health_metrics";
    /// Predict storage impact before committing
    pub const META_UTL_PREDICT_STORAGE: &str = "meta_utl/predict_storage";
    /// Predict retrieval quality before querying
    pub const META_UTL_PREDICT_RETRIEVAL: &str = "meta_utl/predict_retrieval";
    /// Validate prediction against actual outcome
    pub const META_UTL_VALIDATE_PREDICTION: &str = "meta_utl/validate_prediction";
    /// Get meta-learned optimized weights
    pub const META_UTL_OPTIMIZED_WEIGHTS: &str = "meta_utl/optimized_weights";

    // Coherence operations
    // Note: Use topic-based coherence per PRD v6
    /// Get workspace status and active memory
    pub const COHERENCE_WORKSPACE_STATUS: &str = "coherence/workspace_status";
    /// Get meta-cognitive loop status
    pub const COHERENCE_META_STATUS: &str = "coherence/meta_status";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(JsonRpcId::Number(1)));
    }

    #[test]
    fn test_success_response() {
        let resp = JsonRpcResponse::success(
            Some(JsonRpcId::Number(1)),
            serde_json::json!({"status": "ok"}),
        );
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_error_response() {
        let resp = JsonRpcResponse::error(
            Some(JsonRpcId::String("req-123".to_string())),
            error_codes::METHOD_NOT_FOUND,
            "Method not found",
        );
        assert!(resp.result.is_none());
        assert_eq!(
            resp.error.as_ref().unwrap().code,
            error_codes::METHOD_NOT_FOUND
        );
    }
}
