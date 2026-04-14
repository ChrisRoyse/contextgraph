//! Tools List Tests (MCP 2024-11-05 compliance)
//!
//! PERF: tools/list is a pure read of the static tool registry; it does not
//! touch storage. Uses the shared read-only handler fixture.

use crate::protocol::JsonRpcId;

use super::{make_request, shared_readonly_handlers};

#[tokio::test]
async fn test_tools_list_each_tool_has_required_fields() {
    let handlers = shared_readonly_handlers().await;
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
