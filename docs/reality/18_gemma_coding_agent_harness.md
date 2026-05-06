# Gemma Coding Agent Harness

This document records the current Gemma-specific coding-agent harness. In ContextGraph, this harness is not a sidecar around the reality loop. The governed coding-agent harness is the inner reality loop interface itself: Gemma sees reality through this tool surface, acts through this tool surface, and every tool result becomes durable evidence for the next iteration.

## Research Conclusions

Karpathy's `autoresearch` pattern is a tight measurable loop: let an agent change a bounded artifact, run a fixed-budget experiment, keep only verified improvements, and repeat. The important transfer into ContextGraph is not "edit one Python file"; it is "every iteration has a comparable source-of-truth metric and a durable state transition."

Ralph/Wiggum loops emphasize fresh context and state externalization. The prompt can stay structurally stable because the filesystem, git history, progress files, and test output carry state between iterations. ContextGraph extends this into a reality loop: the externalized state is not only files, but source-of-truth readbacks, BlackBoxFix evidence, reward, incidents, and reinjection packets.

The coding-agent harness is therefore the loop's actuator and sensorium:

```text
Gemma
  -> governed coding-agent harness
  -> real repo / tests / official SWE-bench / BlackBoxFix
  -> reality-signal packet
  -> next Gemma attempt
```

The outer meta-loop optimizes this harness, but Gemma's actual repair loop is the harness.

Current implementation detail: each governed tool call now emits a `harness-transition.json` artifact. That artifact is the measured reality-loop step. It records before-state, action, observation, outcome, DynamicJEPA projection, Learning-as-UTL signal, constellation tags, and BlackBoxFix boundary refs. The BlackBoxFix verifier checks that the request, result, and transition files all exist and contain the expected record kinds.

SWE-agent's Agent-Computer Interface research is directly relevant to Gemma. Its file viewer and editor are integrated: the agent sees line-numbered source windows, edits bounded ranges, and immediately receives observation/error feedback. This is a better interface for local models than asking for raw diffs.

Gemma 4 supports function calling, but Google is explicit that the model does not execute code. The application must parse, validate, execute, and return tool results. That matches ContextGraph's architecture: Gemma requests tools; ContextGraph validates and executes; BlackBoxFix verifies durable state.

The active implementation is now the Rust Gemma coding harness inside `reality-loop.rs`. It loads a real GGUF model file through `llama-cpp-2`, renders the model chat template with the governed ContextGraph tool schemas, requires a generated tool-call grammar, performs grammar-constrained generation, parses the assistant message through the llama.cpp OpenAI-compatible parser, and only then executes governed tools. There is no model HTTP server in this boundary.

## Current Gemma Runtime Settings

Local model readback:

```text
model label: gemma4:e4b
model source of truth: models/gemma4-e4b/gemma4-e4b.Q4_K_M.gguf
architecture: gemma4
parameters: 8.0B
context length: 131072
quantization: Q4_K_M
capabilities required by harness: chat template, OpenAI-compatible tool calls, GGUF llama.cpp load
```

Reality-loop request settings now override the sampling defaults for coding-agent work:

```json
{
  "provider": "contextgraph_rust_gemma_coding_harness",
  "temperature": 0.0,
  "top_p": 0.9,
  "context_policy": "gguf_n_ctx_train_max",
  "effective_context_tokens": "loaded GGUF n_ctx_train",
  "minimum_context_tokens": 128000,
  "max_new_tokens": 16384,
  "batch_size": 2048,
  "gpu_layers": 4294967295,
  "agent_tool_rounds": 24
}
```

The harness uses the loaded GGUF model's training context by default. An explicit `--gemma-context-tokens` value is allowed only when it is positive and no larger than the model readback. The run writes `gemma-harness-model-readback.json` with the model path, `n_ctx_train`, effective context, generation budget, template hash, and selected GGUF metadata. Each model turn also writes source-of-truth artifacts for the rendered prompt, generated grammar, raw generation, parsed response, and final `gemma-harness-response.json`.

The harness also fails closed before generation if the loaded GGUF does not identify as Gemma 4 or if the GGUF/effective context is below the 128K coding-agent floor. This prevents a substitute local model or a low-context quant from being run under the Gemma SWE label.

## Governed Coding-Agent Tool Surface

Gemma currently receives these governed tools:

- `read_problem_reality`: durable SWE task packet, oracle tests, source/test boundary, root-cause checklist.
- `read_official_failure_reality`: official SWE-bench failure readback from prior attempts.
- `list_files`: real `git ls-files` navigation with production-source filtering.
- `search_repo`: bounded real `git grep`.
- `inspect_symbol`: symbol search with compact source windows.
- `open_file_window`: line-numbered file window with SHA-256 readback.
- `apply_line_window_edit`: preferred final edit tool; replaces line ranges from an observed file SHA-256.
- `apply_structured_edit`: exact find/replace final edit tool for unique copied source spans.
- `apply_unified_diff`: final unified-diff submission tool; requires observed file SHA-256 values and verifies the diff against the real checkout.
- `run_targeted_test`: bounded targeted pytest command in the real checkout.
- `run_official_swe_eval`: reads official SWE-bench state; cannot claim success without official report readback.
- `compare_attempts`: compares prior patches, failures, repeated patch signals, and official outcomes.
- `retrieve_similar_verified_episodes`: retrieves real prior official evidence artifacts from the runtime root.
- `synthesize_helper_tool`: vetted deterministic analyzers only.

Raw shell is intentionally not exposed. The harness gives Gemma coding-agent capabilities without letting the model move the trust boundary. If a future tool is needed, it should be added as a governed harness capability with schema, source-of-truth declaration, readback, and reward signal extraction.

## Edit Contract

Preferred path:

1. Gemma calls `open_file_window`.
2. Gemma copies `path`, line numbers, and `sha256`.
3. Gemma calls `apply_line_window_edit`.
4. ContextGraph verifies the observed SHA-256 matches the current file.
5. ContextGraph applies the line edit, writes `candidate.patch`, reads the patch SHA-256 back from disk, resets the workspace, and proceeds to validation.

Alternate edit path:

1. Gemma copies an exact unique source span from a real source window.
2. Gemma calls `apply_structured_edit`.
3. ContextGraph rejects missing, ambiguous, empty, or tampered patch artifacts.

Unified diff path:

1. Gemma opens every file changed by the diff and copies the observed SHA-256 values.
2. Gemma calls `apply_unified_diff` with a non-empty unified diff and `observed_files`.
3. ContextGraph runs `git apply --check`, applies the diff to the real checkout, rejects unobserved changed files, rejects test/fixture/harness mutations, runs static checks, writes `candidate.patch`, reads the patch SHA-256 back, and resets the workspace.

## Reality Reward Signals

Each attempt now records `tool_reward_signals` in the reality-signal packet. These are not a replacement for official SWE success; they are intermediate shaping signals for the loop:

- problem reality read;
- repository navigation or search;
- source window opened;
- preferred line-window edit used;
- unified diff edit used;
- patch artifact submitted;
- changed files read before edit;
- targeted test used;
- official evaluation read back;
- validation passed;
- repeated patch penalty.

The latest smoke produced:

```json
{
  "used_problem_reality": true,
  "used_navigation_or_search": true,
  "opened_source_window": false,
  "used_preferred_line_window_edit": false,
  "submitted_patch_artifact": false,
  "official_eval_ran": false,
  "validation_passed": false,
  "score_unbounded": 3.0
}
```

That means the loop can now tell Gemma: you successfully read problem reality and navigated the repo, but you still need source-window grounding and a patch submission.

The reality-signal packet also records `harness_reality`, which is derived by reading every `harness-transition.json` file from the attempt transcript. The summary includes:

- transition coverage: executed tool calls with verified transition rows;
- utility delta sum across tool transitions;
- useful, neutral, and harmful transition counts;
- correction-required count;
- DynamicJEPA projection counts for state/action/outcome rows and surprise candidates;
- Learning projection counts for learning-event candidates;
- Constellation selectors by tool family.

This is the concrete point where the coding-agent harness becomes the inner reality loop: the model does not only receive tool output, it receives measured consequences of its own tool-use behavior.

## Full State Verification

Unit FSV:

```text
cargo test -p context-graph-cli --bin reality-loop -- --nocapture
```

Verified:

- `apply_structured_edit` writes a real patch and changes a real git repo file from `return x + 1` to `return x + 2`.
- `apply_line_window_edit` writes a real patch and changes a real git repo file from `return x + 1` to `return x + 3`.
- `apply_unified_diff` writes a real patch from an observed file hash and validates it with `git apply --check`.
- missing candidate patch source-of-truth fails closed.
- empty patch readback fails closed.
- patch SHA mismatch fails closed.
- stale line-window observed SHA fails closed.

Live Gemma smoke:

```text
/tmp/contextgraph-gemma-agent-harness-fsv-01/rust_reality_1777971780544438052_69ff234d/
```

Source-of-truth readbacks:

```text
summary: file:/tmp/contextgraph-gemma-agent-harness-fsv-01/rust_reality_1777971780544438052_69ff234d/reality-loop-summary.json
tool_transcript: file:/tmp/contextgraph-gemma-agent-harness-fsv-01/rust_reality_1777971780544438052_69ff234d/tasks/scikit-learn__scikit-learn-25638/gemma4-e4b/attempts/attempt-000001/agent-tools/agent-tool-transcript.json
ledger: sqlite:/tmp/contextgraph-gemma-agent-harness-fsv-01/rust_reality_1777971780544438052_69ff234d/tasks/scikit-learn__scikit-learn-25638/gemma4-e4b/blackbox.sqlite#ledger_records
```

Observed result:

```json
{
  "status": "unsolved",
  "error_code": "RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED",
  "executed_tool_calls": 2,
  "tools_used": ["read_problem_reality", "inspect_symbol"],
  "ledger_counts": {
    "tool_call": 4,
    "verifier_result": 4,
    "evidence_packet": 4,
    "incident_packet": 1,
    "reward_compilation": 1
  }
}
```

This is not success, and the system does not claim success. It is a verified coding-agent behavior signal that the next loop can optimize.

## Sources

- Karpathy AutoResearch: https://github.com/karpathy/autoresearch
- Ralph loop concept: https://wiggum.dev/concepts/the-loop/
- Ralph Loops portable format: https://ralphloops.io/
- SWE-agent ACI paper: https://openreview.net/pdf/7b9425730150fb166d4e6c77995f67ea38638fca.pdf
- Reflexion: https://arxiv.org/abs/2303.11366
- Gemma 4 function calling: https://ai.google.dev/gemma/docs/capabilities/text/function-calling-gemma4
- llama.cpp grammars: https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md
