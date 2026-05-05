# OpenAI Outer Optimizer

This document records the new outer-loop architecture.

## Roles

Gemma remains the inner SWE-bench actor:

```text
Gemma -> ContextGraph coding-agent harness -> real repo / official SWE-bench / BlackBoxFix
```

The OpenAI API model is the outer loop coding agent:

```text
verified loop reality -> OpenAI Responses API coding agent -> governed project edits + verification -> next Gemma attempt
```

The OpenAI model does not get to claim SWE success, alter the official evaluator, edit benchmark tests, weaken verifiers, bypass BlackBoxFix, or read/write secrets. It receives source-of-truth reality, may edit governed project files, and must verify those edits with real command/readback artifacts.

## Implementation

Source: `crates/context-graph-cli/src/bin/reality-loop.rs`.

New flags:

```text
--openai-optimizer-each-attempt
--openai-optimizer-model gpt-5.5-pro
--openai-responses-url https://api.openai.com/v1/responses
--openai-api-key-env OPENAI_API_KEY
--openai-optimizer-tool-rounds 8
--openai-optimizer-max-output-tokens 32768
--openai-optimizer-reasoning-effort high
--openai-optimizer-timeout 300
--smoke-test
```

The API key is loaded from process env first, then `.env`. The key is never written into optimizer artifacts; only a SHA-256 hash and redacted metadata are persisted.

The optimizer writes:

```text
<attempt_root>/openai-loop-optimizer/
  optimizer-input.json
  openai-self-reality-packet.json
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

The outer optimizer receives governed reality-reading tools:

- `read_problem_reality_packet`
- `read_reality_signal_packet`
- `read_validation_result`
- `read_agent_tool_transcript`
- `read_harness_transition`
- `read_project_docs`
- `inspect_reality_loop_source`

The outer optimizer also receives governed project-coding tools:

- `project_search`
- `project_open_file_window`
- `project_apply_line_window_edit`
- `project_run_command`

`project_apply_line_window_edit` requires `observed_sha256` from `project_open_file_window`, writes the real project file, reads it back, and materializes a git diff artifact. `project_run_command` is allowlisted to project-verification commands such as `rustfmt_reality_loop`, `cargo_check_reality_loop`, `cargo_test_reality_loop`, `cargo_build_reality_loop`, `git_status`, `git_diff_stat`, `git_diff_reality_loop`, `ollama_ps`, and `nvidia_smi`.

After optimizer tool rounds, the Rust loop runs `verify_openai_project_state`, which performs `rustfmt`, `cargo check`, git status, and git diff readbacks. If project verification fails, the loop fails closed and writes a failure artifact.

The optimizer also writes `openai-self-reality-packet.json`. This is the OpenAI agent's reality packet: it records the tools it used, whether it edited project files, command outputs, git status, git diff, `cargo check`, and the source-of-truth paths for those readbacks. That packet is fed back to the model before the final recommendation, so the outer agent sees the reality of its own actions instead of operating on intent alone.

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

Smoke success does not mean the SWE task is solved. Smoke success means the system persisted and read back the Gemma attempt artifacts, harness transitions, BlackBoxFix ledger evidence, reality-signal packet, OpenAI optimizer tool artifacts, any OpenAI project edits, project verification artifacts, and the final OpenAI optimizer recommendation.

## Research Basis

The implementation follows OpenAI's current Responses API pattern: the model may request function tools, the application executes them, and the application returns `function_call_output` before asking for the final structured answer. Structured output is used so the optimizer recommendation is machine-readable and fail-closed.

Sources:

- OpenAI function calling guide: https://platform.openai.com/docs/guides/function-calling
- OpenAI Responses API reference: https://platform.openai.com/docs/api-reference/responses
- OpenAI structured outputs guide: https://platform.openai.com/docs/guides/structured-outputs
