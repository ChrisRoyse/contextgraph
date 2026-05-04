# Verified Deployment Fix Demo Spec

This is the flagship demo: Gemma fixes a broken deployment artifact and
BlackBoxFix proves what changed.

## Current Status

The deployment fix remains the simplest buyer-facing story, but the implemented
local demo now proves a broader research-grade AgentOps flow in
`scripts/blackboxfix_gemma_tool_demo.py`.

Implemented source-of-truth systems:

- BlackBoxFix SQLite ledger;
- DynamicJEPA RocksDB;
- CUDA tiny-training artifact files;
- DynamicJEPA episode JSONL and manifest;
- W3C PROV-style provenance JSON;
- BlackBoxFix evidence, replay, OTEL, dashboard, scorecard, and FSV package
  artifacts.

The same expected-delta / actual-readback claim applies. Instead of only a
deployment YAML file, the current expected delta includes the entire
DynamicJEPA consequence set: ingested events, panels, dataset shards, training
run, model artifact, predictions, plan trace, guard decisions, surprise event,
episode export, provenance file, passed verifier, and deliberate failed
verifier incident.

## Workspace

```text
demo-root/
  blackbox.sqlite
  repo/
    deploy/service.yaml
    tools/validate_deployment.py
    README.md
    .git/
  artifacts/
```

The repo must be real and committed before Gemma acts.

## Broken Baseline

`deploy/service.yaml`:

```yaml
service:
  name: checkout-api
  image: registry.local/checkout-api:1.4.2
  replicas: 2
  health_check:
    path: /healthz
    interval_seconds: 30
    timeout_seconds: 0
```

The validator fails because `timeout_seconds` must be between `1` and `10`.

## Validator Rules

`tools/validate_deployment.py` enforces:

- YAML parses;
- `service.name` exists;
- `service.image` exists;
- `service.replicas` is an integer from `1` to `10`;
- `health_check.path` starts with `/`;
- `health_check.interval_seconds` is an integer from `5` to `300`;
- `health_check.timeout_seconds` is an integer from `1` to `10`;
- `timeout_seconds < interval_seconds`.

## Prompt

```text
You are fixing a deployment artifact. Inspect deploy/service.yaml, run the
validator, update only deploy/service.yaml, then verify validation passes.
The target outcome is a valid health_check timeout. Do not modify any other
file.
```

## Model Loop

1. Send prompt and tools to `/api/chat`.
2. Require `message.tool_calls`.
3. Validate each call.
4. Execute the local tool.
5. Return tool result to the model.
6. Stop only after:
   - file was written;
   - validator passed after write;
   - Git diff was read;
   - final model response summarizes the fix.

## Recording Plan

Minimum rows before verification:

```text
agent_run=1
agent_step>=3
model_decision>=3
tool_call>=3
```

Mutating write must include expected delta. Non-mutating reads and validators
should be recorded as tool calls without expected persisted delta.

## Verification Plan

Run `cmd_exit_zero_v1`:

```json
{
  "verifier_id": "cmd_exit_zero_v1",
  "source": { "cwd": "repo" },
  "expected": {
    "command": ["python3", "tools/validate_deployment.py", "deploy/service.yaml"],
    "artifacts": ["deploy/service.yaml"]
  }
}
```

Run `git_file_diff_matches_patch_v1`:

```json
{
  "verifier_id": "git_file_diff_matches_patch_v1",
  "source": { "repo": "repo" },
  "expected": {
    "path": "deploy/service.yaml",
    "patch": "exact patch from git diff"
  }
}
```

## Failure Demonstration

Run one intentionally bad verifier claim:

- expected patch says `timeout_seconds: 9`;
- actual patch contains whatever Gemma wrote, for example `timeout_seconds: 5`.

Expected result:

```text
expected_delta +1
readback_snapshot +1
verifier_result.status=failed
evidence_packet +1
incident_packet +1
```

## Buyer Proof Artifacts

Export:

- `evidence.json`
- `replay.json`
- `scorecard.json`
- `dashboard/`
- `episodes/`
- `otel/`
- `fsv-certification/`
- `incident-readback.json`

## Pass Criteria

The demo passes only when:

- validator verifier passed;
- Git diff verifier passed;
- intentional bad verifier produced an incident;
- evidence export exists and reparses;
- dashboard, episodes, OTEL, and FSV manifests exist and hash-check;
- direct SQLite readback confirms the rows.
