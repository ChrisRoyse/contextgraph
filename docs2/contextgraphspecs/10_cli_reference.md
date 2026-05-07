# CLI Reference

**Source files covered:**
- `crates/context-graph-cli/src/main.rs` — `context-graph-cli` clap entrypoint (214 lines)
- `crates/context-graph-cli/src/lib.rs` — module exports
- `crates/context-graph-cli/src/error.rs` — typed CLI errors
- `crates/context-graph-cli/src/mcp_client.rs` — JSON-RPC client used by hooks/inject
- `crates/context-graph-cli/src/mcp_helpers.rs` — high-level MCP tool wrappers
- `crates/context-graph-cli/src/commands/**/*.rs` — subcommand handlers
- `crates/context-graph-cli/src/bin/blackbox.rs` — `blackbox` standalone binary (24 lines)
- `crates/context-graph-cli/src/bin/context-graph.rs` — `context-graph` short-name alias (6 lines)
- `crates/context-graph-cli/src/bin/reality-loop.rs` — `reality-loop` SWE-bench repair binary (2,380 lines, NEW commit `cc8e6c33`)
- `crates/context-graph-mcp/src/main.rs` — `context-graph-mcp` server (manual arg parser, 2,301 lines)
- `crates/context-graph-mcp/src/server/{mod.rs,transport.rs}` — TransportMode + TCP server
- `crates/context-graph-benchmark/Cargo.toml` — benchmark binaries (default-members excluded)
- `Makefile` — workspace make targets

Cross-references: `01_system_overview.md`, `03_configuration.md`, `08_mcp_tools_reference.md`, `14_test_suite.md`, `27_reality_loop.md` (if present).

## 1. Workspace Binaries

Source: `crates/context-graph-cli/Cargo.toml` `[[bin]]` entries + `crates/context-graph-mcp/Cargo.toml` + benchmark crate.

| Binary | Crate | Path | Purpose |
| --- | --- | --- | --- |
| `context-graph-cli` | context-graph-cli | `src/main.rs` | Operator CLI (clap derive) — sessions, memory, hooks, exports, blackbox |
| `context-graph` | context-graph-cli | `src/bin/context-graph.rs` | Short-name alias for `context-graph-cli` (`ContextGraphCli::parse().run()`) |
| `blackbox` | context-graph-cli | `src/bin/blackbox.rs` | BlackBoxFix verified-AgentOps ledger entrypoint (extracts the `Blackbox` subtree directly) |
| `reality-loop` | context-graph-cli | `src/bin/reality-loop.rs` | **NEW (commit cc8e6c33)** — Rust-owned SWE-bench Lite repair loop. Fail-closed reality-boundary gates, runtime measurement contract, official Docker SWE-bench eval per attempt. See §22. |
| `context-graph-mcp` | context-graph-mcp | `src/main.rs` | JSON-RPC 2.0 MCP server — stdio default, TCP optional, daemon mode (TASK-INTEG-018) |
| benchmark binaries | context-graph-benchmark | `src/bin/*.rs` | 30+ targeted benchmarks (excluded from default builds, 87K lines / 22% of repo) |

Run examples:
```bash
cargo run -p context-graph-cli -- <subcommand>
cargo run -p context-graph-cli --bin blackbox -- <subcommand>
cargo run -p context-graph-cli --bin reality-loop -- \
  --task-manifest path/to/task.json --official-eval-each-attempt
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
Source: `commands/blackbox.rs` (`BlackboxCommands` enum at line 137). Constants (line 40+):
- `LEDGER_SCHEMA_VERSION = 14` (current ledger schema version on main)
- `EPISODE_EXPORT_SCHEMA_VERSION = 1`
- `DASHBOARD_SCHEMA_VERSION = 1`
- `ENTERPRISE_CONTROL_SCHEMA_VERSION = 1`
- `SIEM_EXPORT_SCHEMA_VERSION = 1`
- `RISK_CONNECTOR_ID = "blackbox_risk_v1"`
- `REWARD_CONNECTOR_ID = "blackbox_reward_compiler_v1"`
- `PILOT_PACKAGE_RULE_VERSION = "pilot_package_v1"`
- `RISK_ASSESSMENT_RULE_VERSION = "risk_register_v1"`
- `REWARD_RULE_VERSION = "reward_compiler_v1"`

| Subcommand | Variant (file:line) | Phase |
| --- | --- | --- |
| `init` | `Init(InitArgs)` (242) | 1 — bootstrap append-only ledger |
| `connector test/list` | `Connector { ConnectorCommands }` (249) | 1 — source-of-truth probes |
| `record run/step/model-decision/tool-call` | `Record { RecordCommands }` (272) | 1–2 — append events |
| `sdk schema/ingest` | `Sdk { SdkCommands }` (286) | SDK contract surface |
| `policy evaluate/registry` | `Policy { PolicyCommands }` (508) | 6 — gate enforcement |
| `mcp recorder/gateway` | `Mcp { McpCommands }` (578) | 6 — MCP server modes |
| `api surface` | `Api { ApiCommands }` (604) | 7 — surface inventory |
| `server serve` | `Server { ServerCommands }` (343) | recorder HTTP daemon |
| `verify` | `Verify(VerifyArgs)` (485) | 1–2 — invariants check |
| `replay` | `Replay(ReplayArgs)` (617) | reconstruct timeline |
| `export evidence/episodes/otel` | `Export { ExportCommands }` (628) | 5 — JSONL/Parquet/OTel evidence |
| `dashboard build/api` | `Dashboard { DashboardCommands }` (683) | 7 — operator dashboard |
| `enterprise configure/sync-directory/access-evaluate/export-audit` | `Enterprise { EnterpriseCommands }` (734) | 8 — multi-tenant + audit log |
| `score run` | `Score { ScoreCommands }` (854) | 12 — deterministic scorecards |
| `privacy evaluate-retention/request-erasure` | `Privacy { PrivacyCommands }` (860) | 13 — retention + erasure |
| `gate evaluate` | `Gate { GateCommands }` (868) | 6/12 — release gates |
| `package pilot-report` | `Package { PackageCommands }` (874) | 17 — buyer pilot report |
| `risk evaluate` | `Risk { RiskCommands }` (880) | 16 — risk + open-question register |
| `reward compile` | `Reward { RewardCommands }` (886) | reward/loss sample compiler |
| `demo first-wedge` | `Demo { DemoCommands }` (892) | 18 — vulnerability-remediation demo |
| `fsv certify` | `Fsv { FsvCommands }` (899) | full-system-verification certificate |
| `inspect ledger/summary/runs/timeline/evidence/verifiers/errors/exports` | `Inspect { InspectCommands }` (1272) | any — read-only views |

`BlackboxCli` parser (line 127) is reused by both `context-graph-cli blackbox …` (via `commands::blackbox::handle_blackbox_command`) and the standalone `blackbox` binary at `src/bin/blackbox.rs`.

## 14. `setup`, `warmup`, `watch`
Source: `commands/setup.rs`, `commands/warmup.rs`, `commands/watch.rs`.

| Command | Args |
| --- | --- |
| `setup` | `--target-dir`, `--force`, `--skip-chmod` |
| `warmup` | `--timeout-secs`, `--skip-if-warm` (~20–30 s on RTX 5090) |
| `watch` | `--path <DIR>`, `--session-id <ID>`, `--db-path` |

## 15. `context-graph-mcp` Server CLI
Source: `crates/context-graph-mcp/src/main.rs:72` (`CliArgs` struct), parser at `:101` (manual `argv` walk — no clap dep). `TransportMode` enum at `crates/context-graph-mcp/src/server/mod.rs:37` (variants `Stdio` (default) and `Tcp`).

| Flag | Effect | Source |
| --- | --- | --- |
| `--config <PATH>` | Load TOML config (overrides defaults) | `main.rs:127` |
| `--transport <stdio\|tcp>` | Transport mode. `sse` is rejected with `[E_CLI_TRANSPORT_001]` (see `determine_transport_mode` `:283`). | `main.rs:133` |
| `--port <u16>` | TCP port (config default `3100`); rejected with `[E_CLI_PORT_001]` if invalid | `main.rs:139` |
| `--bind <ADDR>` | TCP bind address (default `127.0.0.1`) | `main.rs:150` |
| `--warm-first` | Block on warmup (default ON; cancels `--no-warm`) | `main.rs:156` |
| `--no-warm` | Skip blocking warmup; models load in background | `main.rs:160` |
| `--daemon` | Connect to or spawn daemon (TCP, default port `3100`) | `main.rs:164` |
| `--daemon-port <u16>` | Daemon TCP port (rejected with `[E_CLI_PORT_003]` if invalid) | `main.rs:168` |
| `--daemon-server-only` | Hidden — internal flag for the spawned daemon process | `main.rs:181` |
| `-h` / `--help` | Show usage (`print_help`) | `main.rs:202` |

Priority order (TASK-INTEG-019): CLI flag > env var > config file > built-in default.
- `--transport` overrides `CONTEXT_GRAPH_TRANSPORT`, `config.mcp.transport`
- `--port` overrides `CONTEXT_GRAPH_TCP_PORT`, `config.mcp.tcp_port`
- `--bind` overrides `CONTEXT_GRAPH_BIND_ADDRESS`, `config.mcp.bind_address`
- `RUST_LOG` controls tracing-subscriber filter

The default `max_connections` for TCP is `32` (`Config::default_config().mcp.max_connections`, asserted in `server/mod.rs:test_default_max_connections`).

### TCP error codes (`server/mod.rs:test_tcp_error_codes_exist`)
| Code | Constant | Range |
| --- | --- | --- |
| `-32110` | `TCP_BIND_FAILED` | reserved `-32110…-32119` |
| `-32111` | `TCP_CONNECTION_ERROR` | (TASK-INTEG-018) |
| `-32112` | `TCP_MAX_CONNECTIONS_REACHED` | |
| `-32113` | `TCP_FRAME_ERROR` | |
| `-32114` | `TCP_CLIENT_TIMEOUT` | |

### JSON-RPC envelope (protocol module)
Source: `crates/context-graph-mcp/src/protocol.rs`.
- `JsonRpcRequest { jsonrpc: "2.0", id: Option<JsonRpcId>, method: String, params: Option<Value> }`
- `JsonRpcResponse { jsonrpc: "2.0", id: Option<JsonRpcId>, result: Option<Value>, error: Option<JsonRpcError> }`
- `JsonRpcId = Number(i64) | String(String)` (untagged)
- Methods: `initialize`, `shutdown`, `tools/list`, `tools/call`
- Standard codes: `-32700 PARSE_ERROR`, `-32600 INVALID_REQUEST`, `-32601 METHOD_NOT_FOUND`, `-32602 INVALID_PARAMS`, `-32603 INTERNAL_ERROR`
- Extended codes: `-32001 FEATURE_DISABLED`, `-32002 NODE_NOT_FOUND`, `-32004 STORAGE_ERROR`, `-32005 EMBEDDING_ERROR`, `-32006 TOOL_NOT_FOUND`, `-32007 LAYER_TIMEOUT`, `-32021 INSUFFICIENT_MEMORIES`

### Stdio batch + notification handling (`server/mod.rs:813-1054`)
- Batch JSON-RPC supported: `[{...},{...}]` per spec section 6.
- `MAX_BATCH_SIZE` (in `server::transport`) caps batch size; exceeding returns `INVALID_REQUEST` (L7 fix).
- Empty batch array → `INVALID_REQUEST`.
- Per-request timeout via `config.mcp.request_timeout`; on timeout for a notification (no `id`) the response is suppressed (Audit-7 MCP7-M1 fix).
- Notifications (no `id` field) never receive responses (suppressed via `result=None && error=None && id=None`).
- `read_line_bounded(MAX_LINE_BYTES)` enforces per-line size (AGT-04).

## 16. Daemon Control
Source: `crates/context-graph-mcp/src/main.rs` (daemon section), `crates/context-graph-mcp/src/server/`.

| Operation | Mechanism |
| --- | --- |
| Start daemon | `context-graph-mcp --daemon [--daemon-port 3100]` (idempotent — connects if already running) |
| Stop daemon | Send SIGTERM to PID in lock file (`/tmp/context-graph-mcp-daemon.lock`) |
| Health probe | TCP connect to `127.0.0.1:<daemon_port>`, send JSON-RPC `initialize` ping |
| Stale lock | flock + `kill(pid, 0)` + `/proc/pid/status` zombie check (see MEMORY notes) |
| Storage path resolution | `McpServer::resolve_storage_path` (`server/mod.rs:1191`): `CONTEXT_GRAPH_STORAGE_PATH` env > `config.storage.path` > `<exe_dir>/contextgraph_data` |
| Models path resolution | `McpServer::resolve_models_path` (`server/mod.rs:1243`): `CONTEXT_GRAPH_MODELS_PATH` env > `<exe_dir>/models` |
| Background tasks | GC (5 min, retention 7d), HNSW persist + compaction (10 min), checkpoint (6h), graph builder (configurable). All await `JoinHandle` on shutdown (M1/M5 fixes) |
| Shutdown ordering (`server/mod.rs:1073`) | watch-channel signal → GC (5s) → model loader (60s) → HNSW persist (10s) → graph/code/file watchers → final HNSW persist → RocksDB flush |

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

## 22. `reality-loop` Binary (NEW — commit cc8e6c33)
Source: `crates/context-graph-cli/src/bin/reality-loop.rs` (2,380 lines). Standalone Rust binary owning the SWE-bench Lite repair flow that previously lived in Python. **Built by default** as part of `context-graph-cli` (no feature flag).

**Purpose:** Drive a fail-closed reality-boundary loop that (1) compiles a per-task measurement contract, (2) prompts a local Ollama model for a candidate `git diff`, (3) applies it to a clean checkout at `task.base_commit`, (4) **always** runs the official SWE-bench Docker harness, and (5) only reports `solved` when the official report's `resolved_ids` contains the instance id.

### Top-level flags (clap derive `Args` at line 22)
| Flag | Type | Default | Purpose |
| --- | --- | --- | --- |
| `--model <NAME>` | string | `qwen3.6:27b` | Ollama model tag |
| `--task-manifest <PATH>` | PathBuf | required | JSON manifest for the SWE-bench instance |
| `--runtime-root <PATH>` | PathBuf | `/tmp/contextgraph-rust-reality-loop` | Per-loop scratch root |
| `--ollama-url <URL>` | string | `http://127.0.0.1:11434` | Ollama HTTP endpoint |
| `--context-graph-bin <PATH>` | PathBuf | `target/debug/context-graph` | Binary used to invoke `blackbox` subcommands |
| `--max-attempts <u64>` | u64 | `1` | Max repair attempts; rejected if `<1` (`RUST_REALITY_MAX_ATTEMPTS_INVALID`) |
| `--run-until-resolved` | bool | `false` | Loop until official `resolved=true`; **requires `--official-eval-each-attempt`** (else `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL`) |
| `--official-eval-each-attempt` | bool | `false` | **Mandatory** — without it the loop rejects with `RUST_REALITY_OFFICIAL_EVAL_REQUIRED` because measurement scope cannot collapse to local patch validation |
| `--swebench-python <PATH>` | PathBuf | `~/.cache/contextgraph-swebench/venv/bin/python` | Python interpreter used to run `python -m swebench.harness.run_evaluation` |
| `--official-timeout <secs>` | u64 | `900` | Per-instance Docker eval timeout |
| `--official-max-workers <N>` | u64 | `1` | `--max_workers` passed to SWE-bench harness |
| `--official-cache-level <LEVEL>` | string | `env` | `--cache_level` passed to SWE-bench harness |
| `--official-docker-host <URI>` | string | `unix:///var/run/docker.sock` | `DOCKER_HOST` env for Docker SDK |
| `--official-docker-config <PATH>` | PathBuf | `~/.cache/contextgraph-docker-config` | `DOCKER_CONFIG` directory; auto-bootstraps `config.json: {auths:{}}` |
| `--gpu-index <N>` | u64 | `0` | Target NVIDIA GPU index |
| `--gpu-min-free-mb <N>` | u64 | `8192` | Required free VRAM before each Ollama call (`RUST_REALITY_GPU_*`) |
| `--gpu-max-used-mb <N>` | Option<u64> | unset | Optional ceiling on used VRAM |
| `--ollama-keep-alive <DUR>` | string | `30m` | `keep_alive` field in Ollama chat request |
| `--model-timeout <secs>` | u64 | `300` | reqwest timeout for the Ollama HTTP call |
| `--git-timeout <secs>` | u64 | `600` | Timeout for git clone/fetch/reset/clean/diff calls |
| `--sleep-between-attempts-ms <ms>` | u64 | `0` | Optional pause between attempts |

### Task manifest contract
Required keys (validated by `validate_task_manifest_contract`, line 646):
- `dataset` — must equal `"princeton-nlp/SWE-bench_Lite"` (constant `DATASET_NAME`)
- `instance_id` — non-empty, must contain `"__"`
- `repo` — `owner/repo`, no `..` segments
- `base_commit` — exact 40-char hex SHA (`is_40_hex`)
- `problem_statement` — non-empty
- `fail_to_pass` — non-empty array of test ids
- `pass_to_pass_count` — positive integer
- `source_of_truth` — must contain both `DATASET_NAME` and `instance_id`
- Optional: `official_test_patch_sha256`, `oracle_patch_sha256`

### Measurement contract (`build_measurement_contract`, line 739)
Schema `contextgraph_reality_measurement_contract_v1` written to `<run_root>/measurement-contract.json`:
- `claim_scope.primary_claim = "official_swe_instance_resolution_only"`
- `claim_scope.success_predicate = "official_report.resolved_ids contains task.instance_id"`
- `claim_scope.explicit_non_claims = [...]` — explicit boundaries (no proof of "all repository behavior", no "human product intent" claims, no success when Docker unavailable)
- `oracle = { dataset, split=test, instance_id, fail_to_pass, pass_to_pass_count, official_test_patch_sha256, oracle_patch_sha256, source_of_truth }`
- `immutable_boundaries = { task_manifest, base_commit, repo, official_evaluator, official_docker_host, official_docker_config, reward_spec }`
- `mandatory_readbacks` (10 items): task manifest readback, measurement contract readback, git HEAD = base_commit, git status clean, candidate patch applies, no test/harness mutation, official Docker SDK preflight, official SWE report with `resolved_ids`, BlackBoxFix sqlite ledger counts, reality-event-log append readback
- `model_editable_scope = ["candidate.patch", "notes"]`
- `model_forbidden_scope` — task manifest, official evaluator, Docker harness, ledger schema/hash chain, reward spec, verifier source, benchmark tests/fixtures
- `failure_policy = { unmeasured_success: blocked, missing_source_of_truth: error, incomplete_oracle: error, official_evaluator_unavailable: error, test_or_harness_mutation: failed }`

### Per-attempt flow (`run`, line 192)
1. Read GPU initial state via `read_gpu_vram_state(gpu_index)`; assert budget (`assert_gpu_budget`) at every Ollama call entry/exit.
2. Bootstrap BlackBoxFix ledger: `blackbox init` + `blackbox record run` (capability `contextgraph_rust_reality_loop`, task-kind `rust_reality_swe_loop`, risk-tier `high`).
3. `ensure_workspace` → clone if missing, fetch, `git reset --hard <base_commit>`, `git clean -fdx`. Verifies `HEAD == base_commit` (else `RUST_REALITY_BASE_COMMIT_READBACK_MISMATCH`) and `git status --porcelain` is empty (else `RUST_REALITY_WORKSPACE_NOT_CLEAN_AFTER_RESET`).
4. `collect_repository_context` — capture file tree + key file snippets.
5. `compile_prompt` — deterministic prompt including task, prior failed-patch hashes, parse-failure reasons.
6. `blackbox record model-decision` (capability `contextgraph.rust.reality.swe.generate_patch`).
7. `ollama_chat_json` — POST `/api/chat` with strict JSON schema `{patch:string, notes:string}`, `temperature: 0`, `num_predict: 4096`, `think: false`, `additionalProperties: false`.
8. `parse_patch_report` — extract patch from raw JSON or balanced-brace fallback; otherwise emit `RUST_REALITY_MODEL_RESPONSE_JSON_INVALID` / `RUST_REALITY_MODEL_PATCH_MISSING`.
9. `validate_attempt` — `git apply --whitespace=nowarn`, then `git diff --name-only` to detect forbidden mutations via `is_forbidden_benchmark_mutation` (paths matching `test_*`, `tests/`, `*_test.py`, `conftest.py`, `tox.ini`, `pytest`).
10. `blackbox record tool-call` (capability `contextgraph.rust.reality.swe.apply_patch`, status `ok`/`blocked`/`failed`).
11. `verify_attempt` — generate per-task `verify_attempt_status.sh`, run it via `blackbox verify` with verifier-id `cmd_exit_zero_v1`.
12. **If patch applied AND `--official-eval-each-attempt`:** `verify_official_docker_sdk` (`docker.from_env().version`) → `python -m swebench.harness.run_evaluation` with `DOCKER_HOST` + `DOCKER_CONFIG` env. Result parsed by `find_json_report` + `build_official_evidence`. Instance must appear in one of `resolved_ids`/`completed_ids`/`incomplete_ids`/`empty_patch_ids`/`error_ids` else `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING`.
13. `blackbox score run`; if `gemmafrontend/reward_spec.verified_outcome.json` exists, also `blackbox reward compile`.
14. Append `attempt-summary.json`, append to `reality-events.jsonl`, refresh `reality-loop-summary.json` (schema `contextgraph_rust_reality_loop_summary_v1`).
15. Continue until solved (`resolved=true`) or `attempt >= max_attempts` (when `run_until_resolved=false`).

### On-disk artifacts (under `<runtime_root>/<loop_id>/`)
| Path | Purpose |
| --- | --- |
| `measurement-contract.json` | Schema v1 contract (above) |
| `task-manifest-readback.json` | Verbatim manifest readback |
| `tasks/<instance_id>/<safe_model>/blackbox.sqlite` | Per-task BlackBoxFix ledger |
| `tasks/.../attempts/attempt-NNNNNN/prompt.txt` | Compiled prompt + sha256 |
| `tasks/.../attempts/attempt-NNNNNN/model-response.txt` | Raw Ollama response text |
| `tasks/.../attempts/attempt-NNNNNN/model-parse-report.json` | Parse status |
| `tasks/.../attempts/attempt-NNNNNN/candidate.patch` | Extracted unified diff |
| `tasks/.../attempts/attempt-NNNNNN/actual-applied.patch` | Diff after `git apply` |
| `tasks/.../attempts/attempt-NNNNNN/validation.json` | Patch-application result |
| `tasks/.../attempts/attempt-NNNNNN/blackbox-verify-spec.json` | Verifier spec |
| `tasks/.../attempts/attempt-NNNNNN/official-prediction.jsonl` | Predictions for SWE-bench harness |
| `tasks/.../attempts/attempt-NNNNNN/official-eval-command.json` | Subprocess provenance |
| `tasks/.../attempts/attempt-NNNNNN/official-docker-preflight.json` | Docker SDK preflight evidence |
| `tasks/.../attempts/attempt-NNNNNN/official-docker-env.json` | DOCKER_HOST / DOCKER_CONFIG manifest + sha256 |
| `tasks/.../attempts/attempt-NNNNNN/official-{success,failure}-evidence.json` | Per-attempt official outcome |
| `tasks/.../attempts/attempt-NNNNNN/attempt-summary.json` | Combined summary including ledger counts |
| `reality-events.jsonl` | Append-only event log (validated via readback) |
| `reality-loop-summary.json` | Final summary (schema `contextgraph_rust_reality_loop_summary_v1`) |
| `failures/failure-<unix>-<error_code>.json` | Pretty-printed `RealityError` artifact (readback verified) |

### Exit semantics
- `0` — success (with `RUST_REALITY_LOOP_SUMMARY=<source_of_truth.summary>` printed to stdout)
- `1` — `RealityError` JSON pretty-printed to stderr; `RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH=<file>` reference written. Failure artifact persisted under `failures/`.

### Error-code namespace (selected — all start `RUST_REALITY_*`)
- Args/contract: `..._MAX_ATTEMPTS_INVALID`, `..._CONTINUOUS_REQUIRES_OFFICIAL_EVAL`, `..._OFFICIAL_EVAL_REQUIRED`, `..._CONTEXT_GRAPH_BIN_MISSING`, `..._TASK_MANIFEST_MISSING`, `..._TASK_DATASET_MISMATCH`, `..._TASK_INSTANCE_ID_INVALID`, `..._TASK_REPO_INVALID`, `..._TASK_BASE_COMMIT_INVALID`, `..._TASK_PROBLEM_STATEMENT_MISSING`, `..._TASK_FAIL_TO_PASS_MISSING`, `..._TASK_PASS_TO_PASS_MISSING`, `..._TASK_SOURCE_OF_TRUTH_INCOMPLETE`
- Workspace/git: `..._WORKSPACE_PARENT_MISSING`, `..._GIT_CLONE_FAILED`, `..._GIT_FETCH_FAILED`, `..._GIT_RESET_FAILED`, `..._GIT_CLEAN_FAILED`, `..._GIT_HEAD_READ_FAILED`, `..._BASE_COMMIT_READBACK_MISMATCH`, `..._GIT_STATUS_READ_FAILED`, `..._WORKSPACE_NOT_CLEAN_AFTER_RESET`, `..._GIT_DIFF_FAILED`, `..._GIT_DIFF_NAME_ONLY_FAILED`, `..._PATCH_APPLY_FAILED`, `..._PATCH_MODIFIES_TESTS`
- Process: `..._PROCESS_SPAWN_FAILED`, `..._PROCESS_WAIT_FAILED`, `..._PROCESS_OUTPUT_FAILED`, `..._CHMOD_VERIFIER_FAILED`
- BlackBoxFix: `..._BLACKBOX_COMMAND_FAILED`, `..._BLACKBOX_JSON_INVALID`, `..._BLACKBOX_RUN_ID_MISSING`, `..._BLACKBOX_VERIFY_NON_JSON`
- Ollama: `..._OLLAMA_REQUEST_FAILED`, `..._OLLAMA_JSON_INVALID`, `..._OLLAMA_ERROR`
- Model: `..._MODEL_RESPONSE_JSON_INVALID`, `..._MODEL_PATCH_MISSING`
- Official eval: `..._SWEBENCH_PYTHON_MISSING`, `..._OFFICIAL_DOCKER_UNAVAILABLE`, `..._OFFICIAL_DOCKER_PREFLIGHT_JSON_INVALID`, `..._OFFICIAL_EVAL_FAILED`, `..._OFFICIAL_REPORT_INSTANCE_MISSING`
- GPU/budget: `..._GPU_*` (free-MB / used-MB violations)

### Constants (top of file)
| Name | Value |
| --- | --- |
| `DEFAULT_RUNTIME_ROOT` | `/tmp/contextgraph-rust-reality-loop` |
| `DEFAULT_CONTEXT_GRAPH_BIN` | `target/debug/context-graph` |
| `DEFAULT_MODEL` | `qwen3.6:27b` |
| `DEFAULT_OLLAMA_URL` | `http://127.0.0.1:11434` |
| `DEFAULT_GPU_MIN_FREE_MB` | `8192` |
| `DEFAULT_OLLAMA_KEEP_ALIVE` | `30m` |
| `DEFAULT_OFFICIAL_DOCKER_HOST` | `unix:///var/run/docker.sock` |
| `DEFAULT_OFFICIAL_DOCKER_CONFIG` | `/home/cabdru/.cache/contextgraph-docker-config` |
| `DATASET_NAME` | `princeton-nlp/SWE-bench_Lite` |

### Cross-references
- BlackBoxFix command surface — §13
- Reward spec contract — `gemmafrontend/reward_spec.verified_outcome.json` (referenced via `immutable_boundaries.reward_spec`)
- See commit cc8e6c33 ("feat(context-graph-cli,reality): runtime measurement contract and reality-boundary fail-closed gates") and 56d9026b ("feat(context-graph-cli,reality): Rust-owned reality-loop binary for SWE-bench repair") for change history
