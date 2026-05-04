# Gemma Tool Runtime Contract

This document defines how local Gemma tool use should be wired for the demo
program.

## Runtime

Use:

```text
Ollama /api/chat + tools + gemma4:e4b
```

Do not use prompt-only pseudo-tools when the local runtime can return native
`message.tool_calls`.

## Constraint

Gemma does not execute tools. The harness executes tools after validation.

```text
model emits tool call
  -> harness validates tool name and arguments
  -> harness executes an allowlisted local function
  -> harness records the tool result
  -> BlackBoxFix verifies the durable state
```

## Flagship Tool Set

### `read_deployment_file`

Arguments:

```json
{ "path": "deploy/service.yaml" }
```

Reads only the demo deployment file and returns content, SHA-256, byte count,
and source-of-truth path.

### `run_deployment_validator`

Arguments:

```json
{ "path": "deploy/service.yaml" }
```

Runs:

```bash
python3 tools/validate_deployment.py deploy/service.yaml
```

Returns exit code, stdout, and stderr.

### `write_deployment_file`

Arguments:

```json
{
  "path": "deploy/service.yaml",
  "content": "..."
}
```

Writes only the allowed deployment file. It must validate path, content size,
YAML parseability, and deployment schema before promotion.

### `read_git_diff`

Arguments:

```json
{ "path": "deploy/service.yaml" }
```

Runs:

```bash
git diff --no-ext-diff --no-color -- deploy/service.yaml
```

Returns exact patch text and SHA-256.

## Path Safety

Reject:

- absolute paths;
- `..`;
- symlink escape;
- any file outside the scenario allowlist;
- any path not declared by the tool schema.

## Expected Delta For Writes

For `write_deployment_file`, expected delta should include:

```json
{
  "file": {
    "path": "deploy/service.yaml",
    "sha256": "sha256:...",
    "size_bytes": 1234
  },
  "validation": {
    "command": ["python3", "tools/validate_deployment.py", "deploy/service.yaml"],
    "exit_code": 0
  },
  "git_diff": {
    "path": "deploy/service.yaml",
    "patch_sha256": "sha256:..."
  }
}
```

## Verification Policy

After the mutating write, run:

1. `cmd_exit_zero_v1` for validator success and artifact hash readback.
2. `git_file_diff_matches_patch_v1` for exact Git diff proof.
3. Direct SQLite readback for row counts and latest verifier/incident payloads.

## Fail-Closed Errors

The harness should emit typed JSON errors for:

- unsupported tool;
- invalid arguments;
- path traversal;
- invalid YAML;
- schema-invalid deployment YAML;
- empty content;
- oversized content;
- missing model;
- missing native tool call;
- BlackBoxFix command failure.

Every error needs `error_code`, `field_path`, `source_of_truth`,
`remediation`, and `details`.

