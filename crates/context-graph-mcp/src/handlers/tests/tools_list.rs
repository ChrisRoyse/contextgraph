//! Tools List Tests (MCP 2024-11-05 compliance)

use crate::protocol::JsonRpcId;

use super::{create_test_handlers, make_request};

#[tokio::test]
async fn test_tools_list_returns_all_6_tools() {
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

    // Verify exactly 6 tools returned (5 original + utl_status)
    assert_eq!(
        tools.len(),
        6,
        "Must return exactly 6 tools, got {}",
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

    // Verify all expected tools are present
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
}
