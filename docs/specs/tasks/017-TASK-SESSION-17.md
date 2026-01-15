# TASK-SESSION-17: Implement Exit Code Mapping

```xml
<task_spec id="TASK-SESSION-17" version="1.0">
<metadata>
  <title>Implement Exit Code Mapping</title>
  <status>pending</status>
  <layer>surface</layer>
  <sequence>17</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-17</requirement_ref>
    <constitution_ref>AP-26</constitution_ref>
  </implements>
  <depends_on><!-- None - can be implemented early --></depends_on>
  <estimated_hours>0.5</estimated_hours>
</metadata>
```

## Objective

Implement exit code mapping per AP-26 constitution requirement: exit 2 only for blocking failures (corruption).

## Claude Code Exit Code Semantics

| Exit Code | Claude Code Behavior | Context Graph Usage |
|-----------|---------------------|---------------------|
| `0` | Success, stdout to Claude | Normal operation |
| `2` | Block action, stderr to Claude | Critical failure (corrupt identity) |
| `1` or other | Non-blocking, stderr to user | Recoverable errors, warnings |

## Context

Claude Code hooks interpret exit codes to determine whether to block the action or continue. Exit code 2 is special - it blocks the action and shows stderr to Claude. We must only use exit 2 for truly blocking situations like database corruption.

## Implementation Steps

1. Create or update `error.rs` in CLI module
2. Define CliExitCode enum with Success, Warning, Blocking variants
3. Implement From<CliExitCode> for ExitCode
4. Implement exit_code_for_error() function
5. Document exit code semantics per AP-26

## Input Context Files

```xml
<input_context_files>
  <file purpose="error_types">crates/context-graph-core/src/error.rs</file>
  <file purpose="cli_module">crates/context-graph-mcp/src/cli/</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-mcp/src/cli/error.rs` | Error handling (if not exists) |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-mcp/src/cli/mod.rs` | Export error module |

## Rust Signatures

```rust
// crates/context-graph-mcp/src/cli/error.rs

use std::process::ExitCode;

/// Exit codes for CLI commands per AP-26 constitution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliExitCode {
    /// Success - stdout to Claude
    Success = 0,
    /// Recoverable error - stderr to user, does not block
    Warning = 1,
    /// Blocking failure - stderr to Claude, blocks action
    Blocking = 2,
}

impl From<CliExitCode> for ExitCode {
    fn from(code: CliExitCode) -> Self {
        ExitCode::from(code as u8)
    }
}

impl From<CoreError> for CliExitCode {
    fn from(err: CoreError) -> Self {
        match err {
            // Only truly blocking errors
            CoreError::CorruptedIdentity(_) => CliExitCode::Blocking,
            CoreError::DatabaseCorruption(_) => CliExitCode::Blocking,

            // Everything else is recoverable
            CoreError::NotFound(_) => CliExitCode::Success, // Fresh session
            CoreError::SerializationError(_) => CliExitCode::Warning,
            CoreError::IoError(_) => CliExitCode::Warning,
            _ => CliExitCode::Warning,
        }
    }
}

/// Determine exit code for any error.
pub fn exit_code_for_error(e: &dyn std::error::Error) -> CliExitCode {
    // Check for specific error types
    if let Some(core_err) = e.downcast_ref::<CoreError>() {
        return CliExitCode::from(core_err.clone());
    }

    // Default to warning for unknown errors
    CliExitCode::Warning
}

/// Helper to convert CliExitCode to std::process::ExitCode
#[inline]
pub fn to_exit_code(code: CliExitCode) -> ExitCode {
    code.into()
}
```

## Error Classification Table

| Error Type | Exit Code | Rationale |
|------------|-----------|-----------|
| CorruptedIdentity | 2 | Cannot proceed with corrupt identity |
| DatabaseCorruption | 2 | Cannot proceed with corrupt database |
| NotFound | 0 | Fresh session is valid state |
| SerializationError | 1 | Recoverable, log and continue |
| IoError | 1 | Recoverable, log and continue |
| TimeoutError | 1 | Recoverable, retry possible |
| NetworkError | 1 | Recoverable, retry possible |
| Other | 1 | Default to non-blocking |

## Definition of Done

### Acceptance Criteria

- [ ] CliExitCode::Success maps to 0
- [ ] CliExitCode::Warning maps to 1
- [ ] CliExitCode::Blocking maps to 2
- [ ] CorruptedIdentity returns exit code 2
- [ ] DatabaseCorruption returns exit code 2
- [ ] NotFound returns exit code 0 (fresh session is valid)
- [ ] IoError returns exit code 1
- [ ] SerializationError returns exit code 1
- [ ] Test case TC-SESSION-22 passes (exit code mapping)

### Constraints

- Exit 2 ONLY for corruption
- NotFound is NOT an error (fresh install)
- All unknown errors default to 1 (non-blocking)

### Verification Commands

```bash
cargo build -p context-graph-mcp
cargo test -p context-graph-mcp exit_code
```

## Test Cases

### TC-SESSION-22: Exit Code Mapping
```rust
#[test]
fn test_exit_code_values() {
    assert_eq!(CliExitCode::Success as u8, 0);
    assert_eq!(CliExitCode::Warning as u8, 1);
    assert_eq!(CliExitCode::Blocking as u8, 2);
}

#[test]
fn test_exit_code_from_core_error() {
    // Blocking errors
    assert_eq!(
        CliExitCode::from(CoreError::CorruptedIdentity("test".into())),
        CliExitCode::Blocking
    );
    assert_eq!(
        CliExitCode::from(CoreError::DatabaseCorruption("test".into())),
        CliExitCode::Blocking
    );

    // Non-blocking errors
    assert_eq!(
        CliExitCode::from(CoreError::NotFound("test".into())),
        CliExitCode::Success // Fresh session is OK
    );
    assert_eq!(
        CliExitCode::from(CoreError::IoError("test".into())),
        CliExitCode::Warning
    );
    assert_eq!(
        CliExitCode::from(CoreError::SerializationError("test".into())),
        CliExitCode::Warning
    );
}

#[test]
fn test_exit_code_conversion() {
    let exit: ExitCode = CliExitCode::Success.into();
    // ExitCode doesn't expose value, but we can verify compilation
    let _ = exit;

    let exit: ExitCode = CliExitCode::Warning.into();
    let _ = exit;

    let exit: ExitCode = CliExitCode::Blocking.into();
    let _ = exit;
}
```

### TC-SESSION-22b: Exit Code Helper Function
```rust
#[test]
fn test_exit_code_for_error() {
    let corruption = CoreError::DatabaseCorruption("test".into());
    assert_eq!(exit_code_for_error(&corruption), CliExitCode::Blocking);

    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
    assert_eq!(exit_code_for_error(&io_err), CliExitCode::Warning);
}
```

## Exit Conditions

- **Success**: Exit codes correctly mapped per AP-26
- **Failure**: Wrong exit codes for error types - error out with detailed logging

## Completion Notes

This is the final task in the Session Identity Persistence implementation. After completing this task:

1. Run the full test suite: `cargo test`
2. Verify all hooks work end-to-end with Claude Code
3. Check performance targets are met (PreToolUse < 50ms)
4. Update documentation if needed

```xml
</task_spec>
```
