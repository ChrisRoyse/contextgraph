# TASK-MCP-010: Add parameter validation middleware

```xml
<task_spec id="TASK-MCP-010" version="1.0">
<metadata>
  <title>Add parameter validation middleware</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>36</sequence>
  <implements><requirement_ref>REQ-MCP-010</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
MCP tools need consistent parameter validation with clear error messages.
A middleware layer validates inputs before handlers process them.
Constitution: ISS-012 parameter validation
</context>

<scope>
<in_scope>
- Create validation middleware for MCP tool handlers
- Validate string lengths, numeric ranges, UUIDs
- Generate consistent error messages with field names
- Support schemars JsonSchema validation
</in_scope>
<out_of_scope>
- Individual tool schemas (other tasks)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/middleware/validation.rs
use schemars::JsonSchema;

/// Validation error with field information.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Field '{field}' validation failed: {message}")]
    FieldValidation { field: String, message: String },

    #[error("Required field '{field}' is missing")]
    MissingRequired { field: String },

    #[error("Invalid format for '{field}': expected {expected}")]
    InvalidFormat { field: String, expected: String },
}

/// Validate tool input against its schema.
pub fn validate_input<T: JsonSchema + serde::de::DeserializeOwned>(
    params: serde_json::Value,
) -> Result<T, ValidationError>;

/// String length validation.
pub fn validate_string_length(
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), ValidationError>;

/// Numeric range validation.
pub fn validate_range<N: PartialOrd + std::fmt::Display>(
    field: &str,
    value: N,
    min: N,
    max: N,
) -> Result<(), ValidationError>;

/// UUID format validation.
pub fn validate_uuid(field: &str, value: &str) -> Result<uuid::Uuid, ValidationError>;
```
</signatures>
<constraints>
- Error messages MUST include field name
- MUST support schemars JsonSchema
- MUST validate before handler execution
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp validation
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/middleware/mod.rs
- crates/context-graph-mcp/src/middleware/validation.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/lib.rs (add middleware module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp validation
```
</test_commands>
</task_spec>
```
