//! MCP tool call handlers.

use serde_json::json;
use tracing::debug;

use context_graph_core::traits::SearchOptions;
use context_graph_core::types::{MemoryNode, UtlContext};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::tools::{get_tool_definitions, tool_names};

use super::Handlers;

impl Handlers {
    /// Handle tools/list request.
    ///
    /// Returns all available MCP tools with their schemas.
    pub(super) async fn handle_tools_list(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling tools/list request");

        let tools = get_tool_definitions();
        JsonRpcResponse::success(id, json!({ "tools": tools }))
    }

    /// Handle tools/call request.
    ///
    /// Dispatches to the appropriate tool handler and returns MCP-compliant result.
    pub(super) async fn handle_tools_call(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing params for tools/call",
                );
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'name' parameter in tools/call",
                );
            }
        };

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        debug!(
            "Calling tool: {} with arguments: {:?}",
            tool_name, arguments
        );

        match tool_name {
            tool_names::INJECT_CONTEXT => self.call_inject_context(id, arguments).await,
            tool_names::STORE_MEMORY => self.call_store_memory(id, arguments).await,
            tool_names::GET_MEMETIC_STATUS => self.call_get_memetic_status(id).await,
            tool_names::GET_GRAPH_MANIFEST => self.call_get_graph_manifest(id).await,
            tool_names::SEARCH_GRAPH => self.call_search_graph(id, arguments).await,
            tool_names::UTL_STATUS => self.call_utl_status(id).await,
            _ => JsonRpcResponse::error(
                id,
                error_codes::TOOL_NOT_FOUND,
                format!("Unknown tool: {}", tool_name),
            ),
        }
    }

    // ========== Tool Call Implementations ==========

    /// MCP-compliant tool result helper.
    ///
    /// Wraps tool output in the required MCP format:
    /// `{content: [{type: "text", text: "..."}], isError: false}`
    pub(super) fn tool_result(
        id: Option<JsonRpcId>,
        data: serde_json::Value,
    ) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
                }],
                "isError": false
            }),
        )
    }

    /// MCP-compliant tool error helper.
    pub(super) fn tool_error(id: Option<JsonRpcId>, message: &str) -> JsonRpcResponse {
        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": message
                }],
                "isError": true
            }),
        )
    }

    /// inject_context tool implementation.
    pub(super) async fn call_inject_context(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return Self::tool_error(id, "Missing 'content' parameter"),
        };

        let rationale = args.get("rationale").and_then(|v| v.as_str()).unwrap_or("");
        let importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        // Compute UTL metrics for the content
        let context = UtlContext::default();
        let metrics = match self.utl_processor.compute_metrics(&content, &context).await {
            Ok(m) => m,
            Err(e) => return Self::tool_error(id, &format!("UTL processing failed: {}", e)),
        };

        // Create and store the memory node
        let embedding = vec![0.1; 1536]; // Stub embedding
        let mut node = MemoryNode::new(content, embedding);
        node.importance = importance as f32;
        let node_id = node.id;

        if let Err(e) = self.memory_store.store(node).await {
            return Self::tool_error(id, &format!("Storage failed: {}", e));
        }

        Self::tool_result(
            id,
            json!({
                "nodeId": node_id.to_string(),
                "rationale": rationale,
                "utl": {
                    "learningScore": metrics.learning_score,
                    "entropy": metrics.entropy,
                    "coherence": metrics.coherence,
                    "surprise": metrics.surprise
                }
            }),
        )
    }

    /// store_memory tool implementation.
    pub(super) async fn call_store_memory(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return Self::tool_error(id, "Missing 'content' parameter"),
        };

        let importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        let embedding = vec![0.1; 1536]; // Stub embedding
        let mut node = MemoryNode::new(content, embedding);
        node.importance = importance as f32;
        let node_id = node.id;

        match self.memory_store.store(node).await {
            Ok(_) => Self::tool_result(id, json!({ "nodeId": node_id.to_string() })),
            Err(e) => Self::tool_error(id, &format!("Storage failed: {}", e)),
        }
    }

    /// get_memetic_status tool implementation.
    pub(super) async fn call_get_memetic_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        let node_count = self.memory_store.count().await.unwrap_or(0);

        Self::tool_result(
            id,
            json!({
                "phase": "ghost-system",
                "nodeCount": node_count,
                "utl": {
                    "entropy": 0.5,
                    "coherence": 0.8,
                    "learningScore": 0.65,
                    "suggestedAction": "continue"
                },
                "layers": {
                    "perception": "active",
                    "memory": "active",
                    "reasoning": "stub",
                    "action": "stub",
                    "meta": "stub"
                }
            }),
        )
    }

    /// get_graph_manifest tool implementation.
    pub(super) async fn call_get_graph_manifest(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        Self::tool_result(
            id,
            json!({
                "architecture": "5-layer-bio-nervous",
                "layers": [
                    {
                        "name": "Perception",
                        "description": "Sensory input processing and feature extraction",
                        "status": "active"
                    },
                    {
                        "name": "Memory",
                        "description": "Episodic and semantic memory storage with vector embeddings",
                        "status": "active"
                    },
                    {
                        "name": "Reasoning",
                        "description": "Inference, planning, and decision making",
                        "status": "stub"
                    },
                    {
                        "name": "Action",
                        "description": "Response generation and motor control",
                        "status": "stub"
                    },
                    {
                        "name": "Meta",
                        "description": "Self-monitoring, learning rate control, and system optimization",
                        "status": "stub"
                    }
                ],
                "utl": {
                    "description": "Universal Transfer Learning - measures learning potential",
                    "formula": "L(x) = H(P) - H(P|x) + alpha * C(x)"
                }
            }),
        )
    }

    /// search_graph tool implementation.
    pub(super) async fn call_search_graph(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return Self::tool_error(id, "Missing 'query' parameter"),
        };

        let top_k = args.get("topK").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let options = SearchOptions::new(top_k);

        match self.memory_store.search_text(query, options).await {
            Ok(results) => {
                let results_json: Vec<_> = results
                    .iter()
                    .map(|(node, score)| {
                        json!({
                            "nodeId": node.id.to_string(),
                            "content": node.content,
                            "score": score,
                            "importance": node.importance
                        })
                    })
                    .collect();

                Self::tool_result(
                    id,
                    json!({ "results": results_json, "count": results_json.len() }),
                )
            }
            Err(e) => Self::tool_error(id, &format!("Search failed: {}", e)),
        }
    }

    /// utl_status tool implementation.
    ///
    /// Returns current UTL system state including lifecycle phase, entropy,
    /// coherence, learning score, Johari quadrant, and consolidation phase.
    pub(super) async fn call_utl_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling utl_status tool call");

        // Get status from UTL processor (returns serde_json::Value)
        let status = self.utl_processor.get_status();

        Self::tool_result(id, status)
    }
}
