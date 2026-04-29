# CLI Reference

**Source files covered:**
- `crates/context-graph-cli/src/main.rs`
- `crates/context-graph-cli/src/commands/**/*.rs`
- `crates/context-graph-mcp/src/main.rs`

## 1. Binaries
- `context-graph-cli` — operator CLI (clap derive).
- `context-graph-mcp` — MCP server (manual flag parser).
- `train-causal` — embeddings training (separate bin).

## 2. CLI Top-Level Subcommands
| Subcommand | Module | Purpose |
| --- | --- | --- |
| session | commands/session/ | Session persist/restore |
| hooks | commands/hooks/ | Claude Code hook lifecycle |
| memory | commands/memory/ | capture, inject |
| topic | commands/topic.rs | Topic operations |
| divergence | commands/divergence.rs | Divergence alerts |
| learning | commands/learning.rs | Learning event read/write/export |
| utl | commands/utl.rs | Learner-state UTL ops |
| setup | commands/setup.rs | Bootstrap / install |
| warmup | commands/warmup.rs | Wait for embedding warm provider |
| watch | commands/watch.rs | Watch paths into session |
| export | commands/export_training.rs (subcommand) | `training-corpus` export |
| backfill-e14 | commands/backfill_e14.rs | E14 backfill (dry-run/limit/batch) |

## 3. Hooks Submodule (Claude Code)
`commands/hooks/` defines: pre_tool_use, post_tool_use, pre_compact, session_start, session_end, user_prompt_submit, task_completed, plus session_state, memory_cache, types, args, error.

## 4. Notable Command Args
| Command | Key flags |
| --- | --- |
| memory capture | content, hook_type, importance, response_type, session_id, source, tool_name |
| memory inject | budget, session_id, top_k |
| topic | force, format, hours, json |
| learning | clear_existing, dataset_id, event_id, include_text, limit, offset, out, overwrite, storage |
| utl | allow_missing, calibration_root, clear_existing, learner_id, models_root, session_ts, storage, trace_id |
| export training-corpus | batch_size, limit, out, storage |
| backfill-e14 | dry_run, limit, log_every, models_dir, scan_batch, storage |
| warmup | skip_if_warm, timeout_secs |
| watch | db_path, path, session_id |
| session persist | db_path |
| session restore | db_path, format |
| setup | force, skip_chmod, target_dir |

## 5. MCP Server CLI (`context-graph-mcp`)
| Flag | Effect |
| --- | --- |
| `--config <path>` | Load TOML config |
| `--transport stdio\|tcp` | Override transport (sse rejected) |
| `--port <u16>` | Override TCP port (default 3100) |
| `--bind <addr>` | Override bind address |
| `--warm-first` / `--no-warm` | Force or skip blocking warmup |
| `--daemon` / `--daemon-port <u16>` | Run as daemon |
| `--daemon-server-only` (hidden) | Daemon-only internal mode |
| `-h` / `--help` | Help |

## 6. Verbose Flag
Top-level `--verbose` on `context-graph-cli` controls tracing level.
