# CLI Reference

**Source files covered:**
- `crates/context-graph-cli/src/main.rs` — `context-graph-cli` clap entrypoint
- `crates/context-graph-cli/src/commands/**/*.rs` — subcommand handlers
- `crates/context-graph-cli/src/bin/blackbox.rs` — `blackbox` standalone binary
- `crates/context-graph-cli/src/bin/context-graph.rs` — `context-graph` alias binary
- `crates/context-graph-mcp/src/main.rs` — `context-graph-mcp` server (manual arg parser)
- `crates/context-graph-benchmark/Cargo.toml` — benchmark binaries (default-members excluded)
- `Makefile` — workspace make targets

Cross-references: `01_system_overview.md`, `03_configuration.md`, `08_mcp_tools_reference.md`, `14_test_suite.md`.

## 1. Workspace Binaries

| Binary | Crate | Path | Purpose |
| --- | --- | --- | --- |
| `context-graph-cli` | context-graph-cli | `src/main.rs` | Operator CLI (clap derive) — sessions, memory, hooks, exports |
| `context-graph` | context-graph-cli | `src/bin/context-graph.rs` | Short-name alias |
| `blackbox` | context-graph-cli | `src/bin/blackbox.rs` | BlackBoxFix verified-AgentOps ledger entrypoint (extracts the `Blackbox` subtree) |
| `context-graph-mcp` | context-graph-mcp | `src/main.rs` | JSON-RPC 2.0 MCP server (stdio / TCP / daemon) |
| benchmark binaries | context-graph-benchmark | `src/bin/*.rs` | 30+ targeted benchmarks (excluded from default builds, 87K lines / 22% of repo) |

Run examples:
```bash
cargo run -p context-graph-cli -- <subcommand>
cargo run -p context-graph-cli --bin blackbox -- <subcommand>
cargo run -p context-graph-mcp -- --transport tcp --port 3100
cargo run -p context-graph-benchmark --bin <bench-name>      # explicit -p required
```

## 2. `context-graph-cli` Top-Level Subcommands

Source: `crates/context-graph-cli/src/main.rs:42` (`enum Commands`).

| Subcommand | Module | Purpose |
| --- | --- | --- |
| `session` | `commands/session/` | Persist / restore identity snapshot |
| `hooks` | `commands/hooks/` | Claude Code native hook event dispatch |
| `memory` | `commands/memory/` | Capture (hooks/responses) and inject context |
| `topic` | `commands/topic.rs` | Topic portfolio, stability, detection |
| `divergence` | `commands/divergence.rs` | Divergence alerts vs recent activity |
| `dynamicjepa` | `commands/dynamicjepa.rs` | DynamicJEPA (5090jepa) MCP surface |
| `learning` | `commands/learning.rs` | Learning-event ingest / list / export |
| `utl` | `commands/utl.rs` | Learner-state UTL ops + retrieval policy |
| `setup` | `commands/setup.rs` | Bootstrap `.claude/settings.json` + hook scripts |
| `warmup` | `commands/warmup.rs` | Pre-load embedding models to VRAM |
| `watch` | `commands/watch.rs` | Watch markdown directory into a session |
| `export training-corpus` | `commands/export_training.rs` | Phase-6 Parquet dump of `CF_TRAINING_RECORDS` |
| `export dynamicjepa-episodes` | `commands/export_dynamicjepa.rs` | DynamicJEPA bundle → JSONL episodes |
| `backfill-e14` | `commands/backfill_e14.rs` | Re-embed missing E14 BGE-M3 dense vectors |
| `blackbox` | `commands/blackbox.rs` | BlackBoxFix verified-AgentOps ledger (Phases 1–13) |

Top-level flag: `--verbose` (`-v` count action) — controls tracing filter (`warn` → `info` → `debug` → `trace`).

## 3. `session` Subcommands
Source: `crates/context-graph-cli/src/commands/session/mod.rs`.

| Subcommand | Args | Notes |
| --- | --- | --- |
| `persist` | `--db-path <PATH>` | Persist session identity (in-memory `SessionCache`, not RocksDB — see PRD v6 §14) |
| `restore` | `--db-path <PATH>`, `--format <text\|json>` | Read back snapshot |

## 4. `memory` Subcommands
Source: `crates/context-graph-cli/src/commands/memory/`.

| Subcommand | Key flags |
| --- | --- |
| `capture` | `--content`, `--hook-type` (default `hook`), `--importance` (default `0.5`, clamped [0,1]), `--response-type`, `--session-id`, `--source`, `--tool-name` |
| `capture-response` | Variant for response capture (`stop_response` default) |
| `inject` | `--budget <tokens>` (DEFAULT_CONTEXT_BUDGET), `--top-k` (default `10`), `--session-id`, optional `--brief` |

## 5. `hooks` Subcommands
Source: `commands/hooks/{args.rs, types.rs, *.rs}`.

| Subcommand | Module |
| --- | --- |
| `pre-tool-use` | `pre_tool_use.rs` |
| `post-tool-use` | `post_tool_use.rs` |
| `pre-compact` | `pre_compact.rs` |
| `session-start` | `session_start.rs` |
| `session-end` | `session_end.rs` |
| `user-prompt-submit` | `user_prompt_submit.rs` |
| `task-completed` | `task_completed.rs` |
| `generate-config` | (`GenerateConfigArgs`) |

Each hook reads JSON payload (`HookInput`) on stdin, returns `HookOutput` JSON on stdout. See `commands/hooks/types.rs`.

## 6. `topic` Subcommands
Source: `commands/topic.rs`.

| Subcommand | Flags |
| --- | --- |
| `portfolio` | `--hours`, `--format <text\|json>`, `--force` |
| `stability` | `--hours`, `--json` |
| `detect` | `--hours`, `--force` |

## 7. `divergence`
Source: `commands/divergence.rs`. Uses semantic embedders only (E1, E5, E6, E7, E10, E12, E13); E2–E4 excluded per AP-62/AP-63.

| Subcommand | Flags |
| --- | --- |
| `check` | `--hours` (required), `--format <text\|json>` |

## 8. `dynamicjepa` Subcommands
Source: `commands/dynamicjepa.rs` (`DynamicJepaCommands` enum, ≈40 variants).

Domain registry / inspection: `register-domain`, `list-domains`, `get-domain`, `inspect-counts`, `inspect-cf`.
Ingest / adapters: `ingest-event`, `run-adapter`, `materialize-panel`, `get-panel`, `list-instrument-readings`.
Bindings / trajectories: `bind`, `list-bindings`, `compile-trajectories`, `get-trajectory`, `list-trajectories`.
Datasets: `compile-dataset`, `get-dataset-shard`, `inspect-dataset`.
Training / artifacts: `train`, `get-training-run`, `get-artifact`.
Predictions / planning: `predict`, `plan`, `record-surprise`, `get-prediction`, `get-plan-trace`, `get-surprise`.
Constellations / calibration: `build-constellation`, `list-constellations`, `get-constellation`, `calibrate-threshold`, `recalibrate-threshold`.
Cross-domain / audit: `compute-mc-ratio`, `audit-pairwise-mi`, `cross-domain-transfer`.
Verification: `verify-counter-world`, `verify-gridworld`, `verify-career-taxonomy`, `research-smoke`, `hp-sweep`.

## 9. `learning` Subcommands
Source: `commands/learning.rs` (`LearningCommands`).

| Subcommand | Args |
| --- | --- |
| `list` | `--storage`, `--limit`, `--offset`, `--include-text` |
| `get` | `--storage`, `--event-id`, `--include-text` |
| `export-events-jsonl` | `--storage`, `--out`, `--limit`, `--clear-existing`, `--overwrite`, `--batch-size` |
| `export-dataset-jsonl` | `--storage`, `--dataset-id`, `--out`, `--overwrite` |
| `record-events-jsonl` | inverse of export |
| `record-synthetic` | seed test events |

## 10. `utl` Subcommands
Source: `commands/utl.rs` (`UtlCommands`).

| Subcommand | Args |
| --- | --- |
| `record-synthetic` | `--storage`, `--learner-id`, `--session-ts`, `--clear-existing` |
| `get-session` | `--storage`, `--learner-id`, `--session-ts` |
| `get-trace` | `--storage`, `--trace-id` |
| `asset-preflight` | `--models-root`, `--calibration-root`, `--allow-missing` |

## 11. `export` Subcommands
Source: `commands/export_training.rs`, `commands/export_dynamicjepa.rs`.

| Subcommand | Flags | Schema |
| --- | --- | --- |
| `export training-corpus` | `--storage`, `--out`, `--limit`, `--batch-size` | Parquet `(memory_id: Utf8, record_bytes: Binary)` — bincode-with-version-byte payload (`TRAINING_RECORD_VERSION = 3`) |
| `export dynamicjepa-episodes` | varies (`ExportDynamicJepaEpisodesArgs`) | JSONL — `EPISODE_SCHEMA_VERSION = 1`, requires `bundle.schema_version = 2` |

## 12. `backfill-e14`
Source: `commands/backfill_e14.rs` (`BackfillE14Args`).

| Flag | Purpose |
| --- | --- |
| `--storage <PATH>` | Target RocksDB |
| `--models-dir <PATH>` | BAAI/bge-m3 snapshot root (default `./models/`) |
| `--dry-run` | Count candidates, no model load, no writes |
| `--limit <N>` | Cap fingerprints processed |
| `--scan-batch <N>` | Iterator chunk size |
| `--log-every <N>` | Progress cadence |

## 13. `blackbox` Subcommands
Source: `commands/blackbox.rs` (`BlackboxCommands` enum). Constant: `LEDGER_SCHEMA_VERSION = 8` (Phase 13 ships schema v8 in code; docs reference Phase 13 as "v7" historically — current source is 8).

| Subcommand | Variant | Phase |
| --- | --- | --- |
| `init` | `Init(InitArgs)` | 1 — bootstrap append-only ledger |
| `connector test/list` | `Connector { test, list }` | 1 — source-of-truth probes |
| `record run/step/model-decision/tool-call` | `Record { … }` | 1–2 — append events |
| `verify` | `Verify(VerifyArgs)` | 1–2 — invariants check |
| `replay` | `Replay(ReplayArgs)` | reconstruct timeline |
| `policy evaluate/registry` | `Policy { … }` | 6 — gate enforcement |
| `mcp recorder/gateway` | `Mcp { … }` | 6 — MCP server modes |
| `api surface` | `Api { … }` | 7 — surface inventory |
| `export evidence/episodes` | `Export { … }` | 5 — JSONL/Parquet evidence |
| `dashboard build/api` | `Dashboard { … }` | 7 — operator dashboard |
| `enterprise configure/sync-directory/access-evaluate/export-audit` | `Enterprise { … }` | 8 — multi-tenant + audit log |
| `score run` | `Score { Run }` | 12 — deterministic scorecards |
| `privacy evaluate-retention/request-erasure` | `Privacy { … }` | 13 — retention + erasure |
| `gate evaluate` | `Gate { Evaluate }` | 6/12 — release gates |
| `inspect ledger` | `Inspect { Ledger }` | any |
| Inspect read-only views | `Inspect { Summary, Runs, Timeline, Evidence, Verifiers, Errors, Exports }` | any |

## 14. `setup`, `warmup`, `watch`
Source: `commands/setup.rs`, `commands/warmup.rs`, `commands/watch.rs`.

| Command | Args |
| --- | --- |
| `setup` | `--target-dir`, `--force`, `--skip-chmod` |
| `warmup` | `--timeout-secs`, `--skip-if-warm` (~20–30 s on RTX 5090) |
| `watch` | `--path <DIR>`, `--session-id <ID>`, `--db-path` |

## 15. `context-graph-mcp` Server CLI
Source: `crates/context-graph-mcp/src/main.rs:79` (`CliArgs::parse()` — manual parser, no clap dep).

| Flag | Effect |
| --- | --- |
| `--config <PATH>` | Load TOML config (overrides defaults) |
| `--transport <stdio\|tcp>` | Transport mode (`sse` rejected) |
| `--port <u16>` | TCP port (config default `3100`) |
| `--bind <ADDR>` | TCP bind address |
| `--warm-first` | Block on warmup (default ON; cancels `--no-warm`) |
| `--no-warm` | Skip blocking warmup; models load in background |
| `--daemon` | Connect to or spawn daemon (TCP, default port 3100) |
| `--daemon-port <u16>` | Daemon TCP port |
| `--daemon-server-only` | Hidden — internal flag for spawned daemon process |
| `-h` / `--help` | Show usage |

Priority order (TASK-INTEG-019): CLI flag > env var > config file > built-in default.
- `--transport` overrides `CONTEXT_GRAPH_TRANSPORT`, `config.mcp.transport`
- `--port` overrides `CONTEXT_GRAPH_TCP_PORT`, `config.mcp.tcp_port`
- `--bind` overrides `CONTEXT_GRAPH_BIND_ADDRESS`, `config.mcp.bind_address`
- `RUST_LOG` controls tracing-subscriber filter

## 16. Daemon Control
Source: `crates/context-graph-mcp/src/main.rs` (daemon section), `crates/context-graph-mcp/src/server/`.

| Operation | Mechanism |
| --- | --- |
| Start daemon | `context-graph-mcp --daemon [--daemon-port 3100]` (idempotent — connects if already running) |
| Stop daemon | Send SIGTERM to PID in lock file (`/tmp/context-graph-mcp-daemon.lock`) |
| Health probe | TCP connect to `127.0.0.1:<daemon_port>`, send JSON-RPC ping |
| Stale lock | flock + `kill(pid, 0)` + `/proc/pid/status` zombie check (see MEMORY notes) |

## 17. Makefile Targets
Source: `Makefile`.

| Target | Action |
| --- | --- |
| `make build` | `build-rust` + `build-python` |
| `make build-rust` | `cargo build --release --workspace` + `scripts/trim-stale-deps.sh release` |
| `make build-python` | `uv sync --locked --extra dev` in `clipcannon/` |
| `make build-debug` | `cargo build --workspace` (debug) |
| `make test` | `test-rust` + `test-python-smoke` |
| `make test-rust` | `cargo test --workspace` |
| `make test-python-smoke` | Pytest subset under `clipcannon/tests/` |
| `make test-python` | Full pytest under `clipcannon/tests/` |
| `make test-e2e` | `cargo test -p context-graph-cli --test e2e` |
| `make test-mcp` | `cargo test -p context-graph-mcp` |
| `make check` / `check-rust` / `check-python` | `cargo check --workspace --all-targets` |
| `make lint` / `make clippy` / `lint-rust` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `make lint-python` | `ruff check src/ tests/` in `clipcannon/` |
| `make trim` | `scripts/trim-stale-deps.sh` |
| `make clean` | Remove `target/debug` |
| `make clean-all` | `cargo clean` |
| `make clean-deep` | `clean-all` + `scripts/clean-build-artifacts.sh --aggressive` |
| `make disk-check` | Disk usage report + `disk-guard.sh` |
| `make release-candidate-archive` / `-v2` | 5090 JEPA release archive (requires several `RELEASE_*` env vars) |

Internal scripts: `scripts/trim-stale-deps.sh`, `scripts/disk-guard.sh`, `scripts/clean-build-artifacts.sh`, `scripts/5090jepa/prepare_release_candidate_archive*.sh`.

## 18. Benchmark Invocations
Source: `crates/context-graph-benchmark/Cargo.toml` — 44 declared `[[bin]]` entries plus 2 Criterion benches (`comparative_suite`, `scaling_analysis`).

| Class | Examples |
| --- | --- |
| Embedder | `e1-semantic-bench`, `e7-bench`, `e7-realdata-bench`, `e11-entity-bench`, `e2-manual-verification`, `e4-hybrid-session-bench` |
| End-to-end | `unified-realdata-bench`, `realdata-bench`, `mcp-bench`, `gpu-bench`, `embedder-stress`, `hf-bench` |
| Domain | `temporal-bench`, `temporal-realdata-bench`, `causal-realdata-bench`, `sparse-bench`, `graph-bench`, `graph-realdata-bench`, `graph-linking-bench`, `multimodal-bench`, `multimodal-realdata-bench`, `validation-bench`, `validation-realdata-bench` |
| Code path | `code-fingerprint-bench`, `weight-profile-bench`, `embedder-impact-bench`, `embedder-impact-realdata-bench`, `benchmark-report` |
| Criterion | `cargo bench -p context-graph-benchmark --bench comparative_suite` / `--bench scaling_analysis` |

Build only when needed:
```bash
cargo build -p context-graph-benchmark --release --bin <name>
```

## 19. Environment Variables

| Variable | Read by | Effect |
| --- | --- | --- |
| `RUST_LOG` | `tracing-subscriber` (all bins) | Trace filter |
| `CONTEXT_GRAPH_TRANSPORT` | `context-graph-mcp` | Default transport when no `--transport` |
| `CONTEXT_GRAPH_TCP_PORT` | `context-graph-mcp` | Default TCP port |
| `CONTEXT_GRAPH_BIND_ADDRESS` | `context-graph-mcp` | Default bind address |
| `UV` (Makefile) | Make | Path to `uv` binary (default `uv`) |
| `CLIPCANNON_DIR` | Make | Python project root (default `clipcannon`) |
| `PYTHON_DEV_EXTRA` | Make | UV extras flag (`--extra dev`) |
| `RELEASE_*` (8 vars) | Make `release-candidate-archive*` | 5090 JEPA release archive parameters |

## 20. Exit Codes (AP-26)
- `0` — Success.
- `1` — Recoverable error.
- `2` — Database / state corruption (fail fast, no retry).

`commands/export_training.rs::summary_to_exit_code` and `commands/export_dynamicjepa.rs::summary_to_exit_code` translate per-export summaries into exit codes.

## 21. Verbose / Logging Behaviour
- `context-graph-cli -v` → `info`, `-vv` → `debug`, `-vvv+` → `trace`. Default reads `RUST_LOG` if set, else `warn`.
- Logs stream to `stderr` so JSON exports on `stdout` remain machine-readable.
- The same pattern is used by the `blackbox` binary in `src/bin/blackbox.rs`.
