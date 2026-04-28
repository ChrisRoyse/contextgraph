# CLI Reference

**Source files covered:**
- `crates/context-graph-cli/src/main.rs`
- `crates/context-graph-cli/src/commands/**/*.rs`
- `crates/context-graph-mcp/src/main.rs`

## 1. CLI Binary
`context-graph-cli` uses clap derive. The top-level verbose flag is in `main.rs`; command modules live under `src/commands`.

## 2. Command Module Inventory
| Source | Scanned arg fields |
| --- | --- |
| crates/context-graph-cli/src/commands/backfill_e14.rs | dry_run, limit, log_every, models_dir, scan_batch, storage |
| crates/context-graph-cli/src/commands/divergence.rs | hours, json, verbose |
| crates/context-graph-cli/src/commands/export_training.rs | batch_size, limit, out, storage |
| crates/context-graph-cli/src/commands/hooks/args.rs | duration_ms, fast_path, force, format, generate_summary, hooks, output_dir, previous_session_id, prompt, session_id, shell, stdin, success, tool_name |
| crates/context-graph-cli/src/commands/hooks/error.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/memory_cache.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/mod.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/post_tool_use.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/pre_compact.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/session_end.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/session_start.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/session_state.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/task_completed.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/types.rs | not determined by scan |
| crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs | not determined by scan |
| crates/context-graph-cli/src/commands/learning.rs | clear_existing, dataset_id, event_id, include_text, limit, offset, out, overwrite, storage |
| crates/context-graph-cli/src/commands/memory/capture.rs | content, hook_type, importance, response_type, session_id, source, tool_name |
| crates/context-graph-cli/src/commands/memory/inject.rs | budget, session_id, top_k |
| crates/context-graph-cli/src/commands/memory/mod.rs | not determined by scan |
| crates/context-graph-cli/src/commands/mod.rs | not determined by scan |
| crates/context-graph-cli/src/commands/session/mod.rs | not determined by scan |
| crates/context-graph-cli/src/commands/session/persist.rs | db_path |
| crates/context-graph-cli/src/commands/session/restore.rs | db_path, format |
| crates/context-graph-cli/src/commands/setup.rs | force, skip_chmod, target_dir |
| crates/context-graph-cli/src/commands/topic.rs | force, format, hours, json |
| crates/context-graph-cli/src/commands/utl.rs | allow_missing, calibration_root, clear_existing, learner_id, models_root, session_ts, storage, trace_id |
| crates/context-graph-cli/src/commands/warmup.rs | skip_if_warm, timeout_secs |
| crates/context-graph-cli/src/commands/watch.rs | db_path, path, session_id |

## 3. Notable Commands
| Command | Behavior |
| --- | --- |
| warmup | Loads/waits for embedding warm provider |
| watch | Watches paths into session/data dir |
| export training-corpus | Exports TrainingRecord payloads |
| backfill-e14 | Backfills E14 with dry-run/limit/batch options |
| learning | Learning event read/write commands |
| utl | Learner-state UTL commands |

## 4. MCP Server CLI
Separate manual parser flags: `--config`, `--transport`, `--port`, `--bind`, `--warm-first`, `--no-warm`, `--daemon`, `--daemon-port`, hidden `--daemon-server-only`, `--help`, `-h`.
