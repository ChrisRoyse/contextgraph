# OpenAI Outer Optimizer

This document records the new outer-loop architecture.

## Roles

Gemma remains the inner SWE-bench actor:

```text
Gemma -> ContextGraph coding-agent harness -> real repo / official SWE-bench / BlackBoxFix
```

The OpenAI API model is the outer loop optimizer:

```text
verified loop reality -> OpenAI Responses API optimizer -> prompt/tool/reward/measurement recommendations -> next Gemma attempt
```

The OpenAI model does not get to claim SWE success, alter the official evaluator, edit benchmark tests, weaken verifiers, or bypass BlackBoxFix. It receives source-of-truth reality and proposes improvements to the loop.

## Implementation

Source: `crates/context-graph-cli/src/bin/reality-loop.rs`.

New flags:

```text
--openai-optimizer-each-attempt
--openai-optimizer-model gpt-5.5
--openai-responses-url https://api.openai.com/v1/responses
--openai-api-key-env OPENAI_API_KEY
--openai-optimizer-tool-rounds 3
--openai-optimizer-max-output-tokens 4096
--openai-optimizer-reasoning-effort high
--openai-optimizer-timeout 300
--smoke-test
```

The API key is loaded from process env first, then `.env`. The key is never written into optimizer artifacts; only a SHA-256 hash and redacted metadata are persisted.

The optimizer writes:

```text
<attempt_root>/openai-loop-optimizer/
  optimizer-input.json
  openai-tool-round-NN-request.json
  openai-tool-round-NN-response.json
  tool-round-NN-II-<tool>/
    requested-openai-optimizer-tool.json
    openai-optimizer-tool-result.json
  openai-final-request.json
  openai-final-response.json
  openai-loop-optimizer-recommendation.json
```

Every artifact is written with readback verification.

## Optimizer Tools

The outer optimizer receives governed read-only tools:

- `read_problem_reality_packet`
- `read_reality_signal_packet`
- `read_validation_result`
- `read_agent_tool_transcript`
- `read_harness_transition`
- `read_project_docs`
- `inspect_reality_loop_source`

These tools expose loop reality and project design context without giving the API model direct write access.

## Structured Output

The final recommendation must satisfy a strict JSON schema:

- observed loop failures;
- next prompt directives for Gemma;
- harness changes;
- reward-signal changes;
- Gemma capability gaps;
- measurement blindspots;
- source-of-truth checks;
- do-not-change boundaries.

The Rust loop validates the required fields and fails closed if the output is missing, malformed, or has the wrong `record_kind`.

## Smoke Contract

Before any long-running `--run-until-resolved` execution, run the short smoke path:

```bash
target/debug/reality-loop \
  --smoke-test \
  --max-attempts 1 \
  --official-eval-each-attempt \
  --openai-optimizer-each-attempt \
  --model gemma4:e4b \
  --task-manifest <real-swe-task-manifest.json> \
  --runtime-root /tmp/contextgraph-openai-optimizer-smoke
```

The smoke path fails closed if:

- `--run-until-resolved` is also enabled;
- `--max-attempts` is not `1`;
- the OpenAI optimizer is not enabled;
- official evaluation readback is disabled.

Smoke success does not mean the SWE task is solved. Smoke success means the system persisted and read back the Gemma attempt artifacts, harness transitions, BlackBoxFix ledger evidence, reality-signal packet, and OpenAI optimizer recommendation.

## Research Basis

The implementation follows OpenAI's current Responses API pattern: the model may request function tools, the application executes them, and the application returns `function_call_output` before asking for the final structured answer. Structured output is used so the optimizer recommendation is machine-readable and fail-closed.

Sources:

- OpenAI function calling guide: https://platform.openai.com/docs/guides/function-calling
- OpenAI Responses API reference: https://platform.openai.com/docs/api-reference/responses
- OpenAI structured outputs guide: https://platform.openai.com/docs/guides/structured-outputs
