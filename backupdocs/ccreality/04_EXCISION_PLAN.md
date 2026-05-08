# Excision Plan — DeepSeek/OpenAI Removal from `reality-loop.rs`

Source file: `/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs` (10,873 lines as of 2026-05-05).

Pre-production rule: **no backwards-compat shims**. Wipe cleanly.

## Removal classification

Three classes:

- **DELETE** — code that exists only because of DeepSeek/OpenAI. Gone.
- **REPLACE** — provider-agnostic shape we keep, but the body changes.
- **KEEP** — provider-neutral helper used by the inner the inner actor loop or governed agent tools.

## Constants (lines 22–37)

| Line | Identifier | Action |
|------|-----------|--------|
| 22 | `DEFAULT_OPENAI_OPTIMIZER_MODEL = "deepseek-v4-pro"` | DELETE |
| 23 | `DEFAULT_OPENAI_RESPONSES_URL = "https://api.deepseek.com/chat/completions"` | DELETE |
| 24 | `DEFAULT_OPENAI_API_KEY_ENV = "DEEPSEEK_API_KEY"` | DELETE |
| 25 | `DEFAULT_OPENAI_OPTIMIZER_MAX_OUTPUT_TOKENS` | DELETE |
| 26 | `DEFAULT_OPENAI_OPTIMIZER_TOOL_ROUNDS` | DELETE |
| 27–30 | `DEFAULT_OPENAI_MAX_RETRIES`, `DEFAULT_OPENAI_INITIAL_BACKOFF_MS`, `DEFAULT_OPENAI_MAX_BACKOFF_MS`, `DEFAULT_OPENAI_DEPENDENCY_BLOCK_SLEEP_MS` | DELETE |
| 31 | `DEFAULT_OPTIMIZER_MAX_ESTIMATED_CALL_COST_USD` | DELETE |
| 32 | `DEFAULT_OPTIMIZER_TRIGGER_POLICY` | DELETE (decision moves to MCP) |
| 33 | `DEFAULT_OPTIMIZER_MAX_PARALLEL_AGENTS` | DELETE |
| 34 | `DEFAULT_OPTIMIZER_REWARD_REGRESSION_THRESHOLD` | DELETE |
| 35–37 | `DEEPSEEK_V4_PRO_CACHE_HIT_INPUT_USD_PER_M`, `DEEPSEEK_V4_PRO_CACHE_MISS_INPUT_USD_PER_M`, `DEEPSEEK_V4_PRO_OUTPUT_USD_PER_M` | DELETE |

KEEP: `DEFAULT_RUNTIME_ROOT`, `DEFAULT_CONTEXT_GRAPH_BIN`, `DEFAULT_MODEL`, `DEFAULT_OLLAMA_*`, `DEFAULT_GPU_*`, `DEFAULT_OFFICIAL_*`, `DATASET_NAME`, `LOCAL_PYTHON_BIN`.

## CLI flags (lines 46–139, in `struct Args`)

DELETE these `#[arg(long ...)]` fields:
- `optimizer_api_smoke_test` (line 65)
- `openai_optimizer_each_attempt` (107)
- `openai_optimizer_model` (109)
- `openai_responses_url` (111)
- `openai_api_key_env` (112–114)
- `openai_optimizer_tool_rounds` (115)
- `openai_optimizer_max_output_tokens` (117)
- `openai_optimizer_reasoning_effort` (119)
- `openai_optimizer_timeout` (121)
- `openai_max_retries` (123)
- `openai_initial_backoff_ms` (125)
- `openai_max_backoff_ms` (127)
- `openai_dependency_block_sleep_ms` (129)
- `optimizer_max_estimated_call_cost_usd` (130)
- `optimizer_trigger_policy` (132)
- `optimizer_max_parallel_agents` (133)
- `optimizer_reward_regression_threshold` (135)

KEEP: `model`, `task_manifest`, `runtime_root`, `ollama_url`, `context_graph_bin`, `max_attempts`, `run_until_resolved`, `smoke_test`, `official_eval_each_attempt`, `swebench_python`, `official_timeout`, `official_max_workers`, `official_cache_level`, `official_docker_host`, `official_docker_config`, `gpu_index`, `gpu_min_free_mb`, `gpu_max_used_mb`, `ollama_keep_alive`, `ollama_num_ctx`, `ollama_num_predict`, `ollama_temperature`, `ollama_top_p`, `model_timeout`, `git_timeout`, `agent_tool_rounds`, `targeted_test_timeout`, `sleep_between_attempts_ms`.

After excision the binary becomes a clean **engine**: it produces reality artifacts and exits. No outer optimizer code path remains.

## Struct fields (lines 187–243)

In `AttemptRecord` (line 220):
- DELETE `openai_optimizer_recommendation_path` (237)
- DELETE `openai_optimizer_recommendation_compact` (238)

Add nothing in their place. The optimizer recommendation is now a separate artifact under `claude-code-optimizer/`, written by the MCP server, not the engine.

## Top-level functions

DELETE in entirety:
- `validate_optimizer_trigger_config` (2117)
- `decide_deepseek_optimizer_trigger` (2156)
- `reward_scalar_from_reality_signal` (2281) — **MOVE** to a shared module if reused; otherwise delete. Currently used only by `decide_deepseek_optimizer_trigger`. Per-call review: scan for other callers; if none, DELETE.
- `run_openai_loop_optimizer` (2349)
- `run_deepseek_optimizer_api_smoke_test` (2628)
- `openai_response_request_body` (2853)
- `deepseek_chat_request_body` (2881)
- `post_deepseek_chat_completion` (2907)
- `write_deepseek_cost_preflight` (3071)
- `deepseek_actual_usage_cost` (3123)
- `estimate_tokens_from_text` (3180)
- `cost_usd` (3184)
- `round_money` (3188)
- `required_json_u64` (3192) — **REVIEW**: provider-neutral helper. Currently only called from DeepSeek cost helpers. If only DeepSeek call sites: DELETE. (Audit: yes, only used by `deepseek_actual_usage_cost`. DELETE.)
- `post_openai_response` (3206)
- `selected_openai_headers` (3346)
- `selected_deepseek_headers` (3365)
- `openai_api_error_code` (3385)
- `deepseek_api_error_code` (3389)
- `deepseek_error_is_retryable` (3393)
- `deepseek_reality_error_code` (3397)
- `openai_error_is_retryable` (3430)
- `openai_reality_error_code` (3440)
- `is_openai_dependency_error` (3457)
- `exponential_backoff_ms` (3479) — **REVIEW**. Generic helper. If reused elsewhere: KEEP. (Audit: only called from DeepSeek/OpenAI HTTP retry. DELETE.)
- `OpenAiFunctionCall` struct (3487)
- `openai_function_calls` (3494)
- `openai_replayable_output_item` (3558)
- `openai_output_text` (3573)
- `deepseek_chat_assistant_message` (3601)
- `deepseek_chat_function_calls` (3634)
- `deepseek_chat_output_text` (3698)
- `deepseek_optimizer_tool_specs` (3707)
- `openai_optimizer_tool_specs` (3728)
- `execute_openai_optimizer_tool` (3908)
- `openai_optimizer_tool_ok` (4231)
- `openai_optimizer_tool_blocked` (4240)
- `optimizer_project_root` (4258)
- `project_file_window` (4278) — **MOVED in Phase 1** to `crates/context-graph-cli/src/governed_edit.rs`. Re-used by the future `harness_open_window` MCP tool.
- `project_apply_line_window_edits` (4336) — **MOVED in Phase 1** to `crates/context-graph-cli/src/governed_edit.rs`; re-used by `harness_apply_line_window_edit` MCP tool.
- `run_openai_project_command` (4550) — **MOVED/RENAMED in Phase 1** to `run_governed_command`; re-used by `harness_run_command` MCP tool.
- `verify_openai_project_state` (4665) — **MOVED/RENAMED in Phase 1** to `verify_project_state`; re-used by `harness_verify_state` MCP tool.
- `build_openai_self_reality_packet` (4708) — DELETE. Self-reality packet was a workaround for DeepSeek being unable to inspect its own actions; Claude Code's built-in transcript + MCP callbacks make this unnecessary.
- `validate_optimizer_project_path` (4802) — **MOVED/RENAMED in Phase 1** to `validate_governed_path`. Path-policy enforcement is reused by every MCP `harness_*` tool and now reads a `CGREALITY_GOVERNED_PATHS` policy JSON path.
- `required_openai_tool_str` (4862) — DELETE.
- `openai_optimizer_json_schema` (4880) — DELETE. Schema lives in `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md` and as a JSON file `config/ccreality_recommendation_schema.json` consumed by `optimizer_record_recommendation`.
- `validate_openai_optimizer_recommendation` (4949) — DELETE. Validation moves into `optimizer_record_recommendation` MCP tool, using the same JSON Schema file.
- `compact_openai_loop_optimizer` (4986) — DELETE.

## Tests to remove (lines 9833–10873)

In the `#[cfg(test)] mod tests` block:

- DELETE `openai_error_classification_distinguishes_quota_from_retryable_rate_limit` (10197)
- DELETE `deepseek_tool_schema_uses_chat_completions_function_shape` (10256)
- DELETE `deepseek_tool_call_replay_preserves_reasoning_content_and_arguments` (10279)
- DELETE `deepseek_cost_preflight_and_usage_are_physical_artifacts` (10326)
- DELETE `deepseek_trigger_decision_fsv_runs_on_failed_official_reality` (10385)
- DELETE `deepseek_trigger_decision_fsv_blocks_disabled_success` (10441)
- DELETE `openai_replay_filters_store_false_reasoning_items` (10582)
- DELETE `openai_project_edit_tool_updates_real_file_and_reads_back_diff` (10669) — UNLESS lifted to `crates/context-graph-shared/src/governed_edit.rs` tests, in which case **MOVE**.
- DELETE `openai_project_edit_rejects_stale_observed_sha256` (10755) — same; MOVE if helper is moved.
- DELETE `openai_project_path_policy_blocks_secret_and_traversal` (10796) — same; MOVE if helper is moved.
- DELETE `openai_self_reality_packet_records_outer_agent_reality` (10820)

KEEP these tests (the inner actor/inner-loop/agent-tool tests, all provider-neutral):
- `tool_mediated_final_edit_submission_fsv` (9946)
- `line_window_edit_submission_fsv` (10028)
- `line_window_edit_rejects_stale_observed_sha256` (10111)
- `editable_window_compacts_real_tool_result_payload_shape` (10148)
- `tool_submission_rejects_missing_patch_source_of_truth` (10497)
- `tool_submission_rejects_patch_hash_mismatch` (10516)
- `final_submission_rejects_empty_patch_readback` (10553)
- `edit_deadline_restricts_tool_surface_after_open_window_stall` (10622)
- `local_python_interpreter_is_real_source_of_truth` (10652)

Also update `test_args` (9838) to drop the deleted fields.

## Attempt-loop call sites

In `run()` (line 283):

- Lines 326–344: DELETE the `--optimizer-api-smoke-test` shortcut (`run_deepseek_optimizer_api_smoke_test`). DELETE the smoke-test pre-flight that depends on `optimizer_trigger_policy`.
- Lines 803–960: DELETE the entire `decide_deepseek_optimizer_trigger` block + the `run_openai_loop_optimizer` invocation block + the blocked/retry loop. The trigger decision and the optimizer call now live in MCP, not in the engine.
- Line 967–968: DELETE `openai_optimizer_recommendation_path` and `openai_optimizer_recommendation_compact` from `AttemptRecord` construction.
- Line 979–980: DELETE `deepseek_optimizer_trigger_decision` and `openai_loop_optimizer` from `attempt-summary.json` payload.

After excision the attempt loop becomes:

```
build problem reality → execute the inner actor → validate → official eval → reward → write attempt summary
```

No outer optimizer is invoked from inside the engine.

## Failure-artifact `args` block (lines 1530–1556)

DELETE the OpenAI/DeepSeek-specific keys from the failure JSON:
- `openai_optimizer_each_attempt`
- `openai_optimizer_model`
- `openai_api_key_env`
- `openai_optimizer_tool_rounds`
- `openai_optimizer_max_output_tokens`
- `openai_optimizer_reasoning_effort`
- `optimizer_max_estimated_call_cost_usd`
- `optimizer_trigger_policy`
- `optimizer_max_parallel_agents`
- `optimizer_reward_regression_threshold`

KEEP everything the inner actor/Ollama/GPU/SWE-bench related.

## `AttemptRecord` summary

After excision:

```rust
struct AttemptRecord {
    attempt: u64,
    status: String,
    error_code: Option<String>,
    message: Option<String>,
    remediation: Option<String>,
    prompt_sha256: String,
    response_sha256: String,
    patch_sha256: Option<String>,
    repeated_patch: bool,
    validation_path: String,
    official_evidence_path: Option<String>,
    resolved: Option<bool>,
    failure_evidence: Value,
    official_reality: Option<Value>,
    reality_signal_packet_path: String,
    reality_signal_compact: Value,
    agent_tool_transcript_path: Option<String>,
    patch_preview: Option<String>,
}
```

Two fields removed.

## Cross-repo touchpoints

These reference DeepSeek/OpenAI but are NOT in `reality-loop.rs`:

| File | What | Action |
|------|------|--------|
| `docs/reality/00_index.md` | Index entries 20, 21 | UPDATE: replace with ccreality entries |
| `docs/reality/20_openai_outer_optimizer.md` | OpenAI optimizer doc | DELETE (superseded by ccreality docs) |
| `docs/reality/21_grand_scale_reality_autoresearch.md` | Two-level meta architecture doc | KEEP but update — the architectural framing is still right; just the model swaps |
| `docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md` | Earlier draft | KEEP for historical context; mark superseded at top |
| `docs2/DEEPSEEK_API_GUIDE.md` | Raw API ref | DELETE (no longer relevant) |
| `docs2/.raw/02_pricing.md` … `docs2/.raw/13_anthropic_api.md` | Already deleted (per `git status`) | n/a — these are already shown deleted in working tree |
| `tasks/INNER_DEMOS_FSV.md`, `tasks/TELEOX_GTM_MASTER.md`, `tasks/newdata/alldata.md` | Mention OpenAI/ChatGPT in unrelated contexts | LEAVE — out of reality-loop scope |
| `docs/PAPER.md`, `docs/alldata.md`, `docs/leapableai_youtube_transcripts.md` | Unrelated | LEAVE |
| `models/`, `models/multimodal/` | Hugging Face references mentioning openai-whisper, etc. | LEAVE — different concern |
| `crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` | Probably an example string mentioning OpenAI | INSPECT before action |
| `crates/context-graph-benchmark/src/datasets/e11_entity.rs`, `stress_corpus.rs` | Example data | LEAVE if just dataset content |
| `crates/context-graph-core/src/types/memory_node/{mod.rs,validation.rs}` | INSPECT |
| `scripts/exp2/strata/S2_code*.json` | Stratified code samples | LEAVE |
| `models/README.md`, `models/models_config.toml`, `models/multimodal/README.md`, `models/multimodal/tokenizer_config.json` | Tokenizer/model configs | LEAVE |
| `config/default.toml` | Possibly has openai key references | INSPECT |
| `tasks/newdata/alldata.md`, `tasks/TELEOX_GTM_MASTER.md` | Unrelated GTM material | LEAVE |

The "INSPECT" entries should be checked during execution but probably don't need changes.

## Order of operations

1. Lift `project_file_window`, `project_apply_line_window_edits`, `run_openai_project_command`, `verify_openai_project_state`, and `validate_optimizer_project_path` (rename to `validate_governed_path`) into a new module `crates/context-graph-shared/src/governed_edit.rs` (or `crates/context-graph-cli/src/governed_edit.rs` if no shared crate exists yet — TBD during implementation).
2. Move and adapt the corresponding tests with them.
3. Delete the constants block at lines 22–37.
4. Delete the CLI flag fields in `struct Args`.
5. Delete the matching fields in `AttemptRecord`, `test_args()`, and the failure-artifact JSON.
6. Delete the `optimizer_api_smoke_test` branch in `run()` and the `decide_deepseek_optimizer_trigger` + `run_openai_loop_optimizer` block in the attempt loop.
7. Delete every function listed under "DELETE in entirety" above.
8. Delete the listed tests.
9. `cargo fmt`, `cargo check -p context-graph-cli`, fix any remaining unused imports.
10. Update `docs/reality/00_index.md` and delete/mark superseded the two related docs.
11. Commit.

Estimated post-excision file size: ~7,500 lines (removing ~3,400 lines of DeepSeek/OpenAI code from 10,873).
