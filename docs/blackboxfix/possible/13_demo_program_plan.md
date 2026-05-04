# Demo Program Plan

This document lays out the demonstration program for showing BlackBoxFix as a
Verified AgentOps layer around a real local Gemma model. The point is not that
Gemma can produce plausible text. The point is that Gemma can request actions
and BlackBoxFix can prove what actually persisted.

## Demo Thesis

Gemma can request actions through tools. BlackBoxFix governs, records, verifies,
scores, exports, and certifies the consequences.

Buyer-facing line:

```text
Gemma can take the action. BlackBoxFix proves whether the action actually
persisted correctly.
```

## Why This Demo Works

Most agent demos collapse intent, action, and proof into one trace. This demo
separates them:

1. Gemma states or implies intent.
2. Gemma emits a tool call.
3. The harness executes only an allowlisted tool.
4. BlackBoxFix records the expected delta.
5. BlackBoxFix reads the durable source of truth separately.
6. BlackBoxFix persists verifier/evidence/incident rows.

The differentiator is the expected-delta / actual-readback pair.

## Researched Runtime Facts

Use native Ollama `/api/chat` tool calling where available.

Sources:

- Ollama documents tool calling with the `tools` field and model-returned
  `message.tool_calls`: <https://docs.ollama.com/capabilities/tool-calling>
- Ollama's chat API documents optional tool definitions and returned tool
  calls: <https://docs.ollama.com/api/chat>
- Google's Gemma function-calling guidance states that Gemma does not execute
  code by itself; the application must detect, validate, and execute tool
  calls: <https://ai.google.dev/gemma/docs/capabilities/function-calling>
- OpenTelemetry GenAI conventions include `chat` and `execute_tool` semantics
  that map naturally to BlackBoxFix model/tool/evidence rows:
  <https://opentelemetry.io/docs/specs/semconv/gen-ai/>

Local proof:

- `ollama show gemma4:e4b --modelfile` shows the normal local Gemma tag.
- A direct `/api/chat` probe against `gemma4:e4b` returned a real
  `message.tool_calls` object for `read_deployment_file`.

## Demo Architecture

```text
User task
  -> Ollama /api/chat with Gemma and declared tools
  -> harness validates requested tool
  -> harness executes narrow local operation
  -> harness records CloudEvents into BlackBoxFix
  -> BlackBoxFix verifier reads source of truth independently
  -> ledger stores expected/readback/result/evidence/incident rows
  -> exports/dashboard/scorecard/FSV package prove the result
```

## Implementation Phases

### Phase A: Native Tool-Calling Harness

Build `scripts/blackboxfix_gemma_tool_demo.py`.

It should:

- create a fresh demo workspace;
- create a real Git repo with intentionally broken files;
- call `gemma4:e4b` through Ollama `/api/chat`;
- expose a narrow tool registry;
- validate every tool name and argument;
- record model decisions and tool calls through `blackbox sdk ingest`;
- verify with BlackBoxFix source-of-truth verifiers;
- export evidence, replay, episodes, OpenTelemetry, dashboard, scorecard, and
  FSV packages.

### Phase B: First Five Examples

Start with at least five scenarios from `14_demo_example_catalog.md`:

1. verified deployment artifact fix;
2. scanner finding remediation proof;
3. ticket evidence closure;
4. Kubernetes deployment snapshot proof;
5. Terraform plan guardrail.

### Phase C: Failure Demo

Every scenario must include one deliberate bad claim. A failed verifier is not
a test failure in this context; it is proof that BlackBoxFix catches mismatch
and creates an incident.

### Phase D: Buyer Package

Each scenario should produce:

- `harness-report.json`
- `evidence.json`
- `replay.json`
- `scorecard.json`
- `dashboard/`
- `episodes/`
- `otel/`
- `fsv-certification/`
- manual FSV logs.

## Operating Rules

- No mock data.
- No fallback from native tool calls to hidden app actions.
- No arbitrary shell tools exposed to the model.
- No success claim without verifier evidence.
- No mutation outside the demo workspace.
- Every mutating action must include `expected_delta`.
- Every verifier must read source of truth separately.
- Every failure must include `error_code`, `field_path`, `source_of_truth`, and
  `remediation`.

## Sources Of Truth

- SQLite ledger: `blackbox.sqlite#ledger_records`
- Git repo: `.git` refs and exact `git diff`
- Files: deployment YAML, scanner artifacts, ticket artifacts, cloud artifacts
- Commands: validators and BlackBoxFix verifiers
- Generated exports: manifest-hashed files

## Success Shape

For a complete successful scenario:

```text
agent_run=1
agent_step>=1
model_decision>=1
tool_call>=1
expected_delta>=1
readback_snapshot>=1
verifier_result>=1
evidence_packet>=1
scorecard>=1
full_state_verification>=1
```

For the intentional failure:

```text
verifier_result.status=failed
incident_packet>=1
error_code is non-empty
actual_readback contradicts expected_persisted_delta
```

