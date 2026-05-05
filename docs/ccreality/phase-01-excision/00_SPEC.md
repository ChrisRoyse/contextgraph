# Phase 1 — Specification

## Objectives

1. Lift provider-neutral governed-edit helpers into a shared module so both the engine (Gemma inner loop) and Phase 3's MCP module can use the same code.
2. Delete every DeepSeek/OpenAI/ChatGPT/gpt-5 code path from `reality-loop.rs`. Pre-production rule: no shims.
3. Leave the engine in a buildable, testable state with all surviving tests passing.

## Scope

In scope:
- `crates/context-graph-cli/src/bin/reality-loop.rs` — primary edit target.
- `crates/context-graph-cli/src/governed_edit.rs` — NEW file (lifted helpers).
- `crates/context-graph-cli/src/lib.rs` — NEW or updated (exports the new module if not already a lib crate).
- `crates/context-graph-cli/Cargo.toml` — add `path = "src/lib.rs"` lib target if needed.

Out of scope:
- Any other crate.
- Documentation updates (Phase 11 handles that).
- New tests beyond moving existing ones.

## Deliverables

| Deliverable | Path | What changes |
|-------------|------|--------------|
| Governed-edit module | `crates/context-graph-cli/src/governed_edit.rs` | NEW: ~600–900 lines |
| Library crate target | `crates/context-graph-cli/src/lib.rs` | NEW or updated to export `governed_edit` |
| Cargo manifest | `crates/context-graph-cli/Cargo.toml` | add `[lib]` section if missing |
| Engine binary | `crates/context-graph-cli/src/bin/reality-loop.rs` | DELETE ~3,400 lines; KEEP everything Gemma/Ollama/SWE-bench/governed-tool/problem-reality/reality-signal/harness-transition/BlackBox related |

## AI-verifiable end-state (full)

See [04_VERIFICATION.md](04_VERIFICATION.md) for the canonical commands. Summary:
- No `deepseek|openai|gpt-5|chatgpt` substring in `reality-loop.rs`.
- `governed_edit.rs` exists with the expected public functions.
- `cargo build -p context-graph-cli --bin reality-loop` succeeds.
- `cargo test -p context-graph-cli --bin reality-loop -- --skip ignored` passes.
- `cargo test -p context-graph-cli --lib governed_edit` passes (the moved tests).
- File size of `reality-loop.rs` is within 6,500–8,500 lines (sanity guard).

## What stays in `reality-loop.rs`

After this phase, `reality-loop.rs` contains exactly:
- CLI args struct (Gemma/Ollama/SWE-bench/GPU only).
- `RealityError`, `ProcessResult`, `TaskManifest`, `GpuState`, `AttemptRecord`, `AgentToolContext`.
- `main()` + `run()` (with the trigger/optimizer block deleted).
- `validate_task_manifest_contract`, `build_measurement_contract`.
- `build_problem_reality_packet` + `compact_problem_reality_packet` + supporting helpers (`test_target_reality`, `repository_file_inventory`, `problem_production_candidates`, `is_40_hex`).
- `compact_failure_evidence`, `compact_official_evidence`.
- `write_failure_artifact` (with deepseek/openai keys removed from its payload).
- `run_cmd`, `run_cmd_env`, `run_required`, `read_process_output`.
- `blackbox` (calls into BlackBoxFix).
- `ollama_chat_json`, `post_ollama_chat`.
- All `agent_*` functions: `agent_tool_names`, `agent_edit_tool_specs`, `is_navigation_or_reality_tool`, `edit_deadline_active`, `compact_editable_window`, `no_tool_repair_prompt`, `agent_tool_specs`, `tool_call_name`, `tool_call_arguments`, `content_json_tool_calls`, `tool_call_id`, `execute_agent_tool_call`, `build_harness_transition`, `harness_transition_outcome`, `harness_learning_signal`, `harness_tool_family`, `harness_constellation_tags`, `record_agent_tool_boundary`.
- All `tool_*` agent tools (read_problem_reality, read_official_failure_reality, list_files, inspect_symbol, search_repo, open_file_window, apply_structured_edit, apply_line_window_edit, run_targeted_test, run_official_swe_eval, compare_attempts, retrieve_similar_verified_episodes, synthesize_helper_tool).
- `materialize_tool_candidate_submission`, `compact_failed_candidate_submission`.
- `tool_ok`, `tool_blocked`, `required_tool_str`, `bounded_tool_u64`, `parse_grep_line`, `file_window`, `collect_evidence_files`, `tail_chars`.
- `patch_from_tool_submission`, `parse_patch_report`, `compile_structured_edits`, `compile_line_window_edits`, `line_spans`, `structured_edit_failure`, `json_candidates`.
- `ensure_workspace`, `reset_workspace`, `validate_attempt`, `run_static_patch_checks`, `is_forbidden_benchmark_mutation`, `verify_attempt`, `run_official_eval`, `verify_official_docker_sdk`, `official_docker_env`, `build_official_evidence`.
- `json_string_array`, `find_json_report`, `find_instance_report`, `find_instance_text_artifact`, `collect_named_files`, `collect_json_files`, `collect_repository_context`, `add_context_candidate`, `task_terms`, `task_symbols`, `production_path_candidates`, `path_candidates`, `is_context_file`, `is_low_value_context_path`, `focused_context_text`, `text_window`.
- `build_reality_signal_packet`, `compact_reality_signal_packet`, `required_next_tool_for_validation`, `derive_tool_reward_signals`, `build_failure_digest`, `classify_failure`, `first_error_location`, `official_test_progress`, `patch_line_stats`, `changed_files_from_validation`, `extract_tool_grounding`, `extract_harness_transition_reality`, `collect_tool_result_paths`, `compact_reward_signal`, `verify_reality_signal_packet`.
- `compile_prompt`, `write_summary`, `append_event`.
- `read_gpu_vram_state`, `assert_gpu_budget`.
- All low-level helpers: `ledger_counts`, `read_json`, `write_json_checked`, `write_text_checked`, `create_dir_checked`, `require_file`, `resolve_path`, `load_required_secret`, `sha256_text`, `sha256_file`, `short_hash`, `unix_secs`, `unix_secs_for_logging`, `unix_ns`, `parse_u64`, `fs_error`, `path_str`, `file_sot`, `optional_file_sot`, `file_sot_to_path`, `sqlite_sot`, `safe_id`, `truncate_chars`.
- All KEEP-listed tests in [../04_EXCISION_PLAN.md](../04_EXCISION_PLAN.md).

## What is removed

See [02_TASKS.md](02_TASKS.md) for the ordered list. In summary: 16 constants, 17 CLI flags, 2 `AttemptRecord` fields, 32 functions, 11 tests, 11 keys from the failure-artifact JSON, the entire trigger+optimizer block in `run()`.

## Time budget

- Tasks 1.1–1.5 (lift): 30–45 min.
- Tasks 1.6–1.10 (delete constants/flags/structs): 15–20 min.
- Task 1.11 (delete `run()` block): 10–15 min.
- Tasks 1.12–1.15 (delete functions in bulk): 30–45 min.
- Tasks 1.16–1.18 (delete tests, fix failure-artifact, cargo fmt+check): 10–15 min.
- Iteration on cargo errors: 0–30 min.

Total: 1.5–2.5 hours typical.
