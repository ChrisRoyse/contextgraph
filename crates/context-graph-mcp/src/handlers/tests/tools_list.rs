//! Tools List Tests (MCP 2024-11-05 compliance)

use crate::protocol::JsonRpcId;

use super::{create_test_handlers, make_request};

#[tokio::test]
async fn test_tools_list_returns_all_44_tools() {
    let handlers = create_test_handlers();
    let request = make_request("tools/list", Some(JsonRpcId::Number(1)), None);

    let response = handlers.dispatch(request).await;

    assert!(
        response.error.is_none(),
        "tools/list should not return an error"
    );
    let result = response.result.expect("tools/list must return a result");

    // MCP REQUIREMENT: tools array MUST exist
    let tools = result
        .get("tools")
        .expect("Response must contain tools array")
        .as_array()
        .expect("tools must be an array");

    // Verify exactly 45 tools returned:
    // Core 6: inject_context, store_memory, get_memetic_status, get_graph_manifest, search_graph, utl_status
    // GWT 8: get_consciousness_state, get_kuramoto_sync, get_workspace_status, get_ego_state, trigger_workspace_broadcast, adjust_coupling, get_coherence_state (TASK-34), get_identity_continuity (TASK-38)
    // ATC 3 (TASK-ATC-001): get_threshold_status, get_calibration_metrics, trigger_recalibration
    // Dream 5 (TASK-DREAM-MCP, TASK-37): trigger_dream, get_dream_status, abort_dream, get_amortized_shortcuts, get_gpu_status
    // Neuromod 2 (TASK-NEUROMOD-MCP): get_neuromodulation_state, adjust_neuromodulator
    // Steering 1 (TASK-STEERING-001): get_steering_feedback
    // Causal 1 (TASK-CAUSAL-001): omni_infer
    // NOTE: Manual North Star tools REMOVED (created incompatible single 1024D embeddings)
    // Teleological 5 (TELEO-007 to TELEO-011): search_teleological, compute_teleological_vector, fuse_embeddings, update_synergy_matrix, manage_teleological_profile
    // Autonomous 7 (TASK-AUTONOMOUS-MCP): auto_bootstrap_north_star, get_alignment_drift, trigger_drift_correction, get_pruning_candidates, trigger_consolidation, discover_sub_goals, get_autonomous_status
    // UTL 1 (TASK-UTL-P1-001): gwt/compute_delta_sc
    // Meta-UTL 3 (TASK-METAUTL-P0-005): get_meta_learning_status, trigger_lambda_recalibration, get_meta_learning_log
    // Epistemic 1 (TASK-MCP-002): epistemic_action
    // Merge 1 (TASK-MCP-004): merge_concepts
    // Johari 1 (TASK-MCP-005): get_johari_classification
    assert_eq!(
        tools.len(),
        45,
        "Must return exactly 45 tools, got {}",
        tools.len()
    );
}

#[tokio::test]
async fn test_tools_list_each_tool_has_required_fields() {
    let handlers = create_test_handlers();
    let request = make_request("tools/list", Some(JsonRpcId::Number(1)), None);

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("tools/list must return a result");
    let tools = result.get("tools").unwrap().as_array().unwrap();

    for tool in tools {
        // MCP REQUIREMENT: each tool MUST have name (string)
        let name = tool
            .get("name")
            .expect("Tool must have name field")
            .as_str()
            .expect("Tool name must be a string");
        assert!(!name.is_empty(), "Tool name must not be empty");

        // MCP REQUIREMENT: each tool MUST have description (string)
        let description = tool
            .get("description")
            .expect("Tool must have description field")
            .as_str()
            .expect("Tool description must be a string");
        assert!(
            !description.is_empty(),
            "Tool description must not be empty"
        );

        // MCP REQUIREMENT: each tool MUST have inputSchema (JSON Schema object)
        let input_schema = tool
            .get("inputSchema")
            .expect("Tool must have inputSchema field");
        assert!(
            input_schema.is_object(),
            "inputSchema must be a JSON object"
        );

        // Verify inputSchema is valid JSON Schema (has type field)
        let schema_type = input_schema
            .get("type")
            .expect("inputSchema must have a type field")
            .as_str()
            .expect("inputSchema type must be a string");
        assert_eq!(schema_type, "object", "inputSchema type must be 'object'");
    }
}

#[tokio::test]
async fn test_tools_list_contains_expected_tool_names() {
    let handlers = create_test_handlers();
    let request = make_request("tools/list", Some(JsonRpcId::Number(1)), None);

    let response = handlers.dispatch(request).await;
    let result = response.result.expect("tools/list must return a result");
    let tools = result.get("tools").unwrap().as_array().unwrap();

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();

    // Verify all expected original tools are present
    assert!(
        tool_names.contains(&"inject_context"),
        "Missing inject_context tool"
    );
    assert!(
        tool_names.contains(&"store_memory"),
        "Missing store_memory tool"
    );
    assert!(
        tool_names.contains(&"get_memetic_status"),
        "Missing get_memetic_status tool"
    );
    assert!(
        tool_names.contains(&"get_graph_manifest"),
        "Missing get_graph_manifest tool"
    );
    assert!(
        tool_names.contains(&"search_graph"),
        "Missing search_graph tool"
    );
    assert!(
        tool_names.contains(&"utl_status"),
        "Missing utl_status tool"
    );

    // Verify all GWT tools are present
    assert!(
        tool_names.contains(&"get_consciousness_state"),
        "Missing get_consciousness_state tool"
    );
    assert!(
        tool_names.contains(&"get_kuramoto_sync"),
        "Missing get_kuramoto_sync tool"
    );
    assert!(
        tool_names.contains(&"get_workspace_status"),
        "Missing get_workspace_status tool"
    );
    assert!(
        tool_names.contains(&"get_ego_state"),
        "Missing get_ego_state tool"
    );
    assert!(
        tool_names.contains(&"trigger_workspace_broadcast"),
        "Missing trigger_workspace_broadcast tool"
    );
    assert!(
        tool_names.contains(&"adjust_coupling"),
        "Missing adjust_coupling tool"
    );

    // Verify UTL tool is present (TASK-UTL-P1-001)
    assert!(
        tool_names.contains(&"gwt/compute_delta_sc"),
        "Missing gwt/compute_delta_sc tool (TASK-UTL-P1-001)"
    );

    // Verify Johari classification tool is present (TASK-MCP-005)
    assert!(
        tool_names.contains(&"get_johari_classification"),
        "Missing get_johari_classification tool (TASK-MCP-005)"
    );

    // Verify coherence state tool is present (TASK-34)
    assert!(
        tool_names.contains(&"get_coherence_state"),
        "Missing get_coherence_state tool (TASK-34)"
    );

    // Verify GPU status tool is present (TASK-37)
    assert!(
        tool_names.contains(&"get_gpu_status"),
        "Missing get_gpu_status tool (TASK-37)"
    );

    // Verify identity continuity tool is present (TASK-38)
    assert!(
        tool_names.contains(&"get_identity_continuity"),
        "Missing get_identity_continuity tool (TASK-38)"
    );
}
