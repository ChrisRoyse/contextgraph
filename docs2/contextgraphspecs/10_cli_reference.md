# CLI Reference

**Source files covered:**
- `crates/context-graph-cli/src/main.rs` — `context-graph-cli` clap entrypoint (222 lines).
- `crates/context-graph-cli/src/lib.rs` — module exports.
- `crates/context-graph-cli/src/error.rs` — typed CLI errors.
- `crates/context-graph-cli/src/mcp_client.rs` — JSON-RPC client used by hooks/inject.
- `crates/context-graph-cli/src/mcp_helpers.rs` — high-level MCP tool wrappers.
- `crates/context-graph-cli/src/commands/**/*.rs` — subcommand handlers.
- `crates/context-graph-cli/src/bin/blackbox.rs` — `blackbox` standalone binary (24 lines).
- `crates/context-graph-cli/src/bin/context-graph.rs` — `context-graph` short-name alias (6 lines).
- `crates/context-graph-cli/src/bin/reality-loop.rs` — `reality-loop` reality-engine binary (18,137 lines).
- `crates/context-graph-mcp/src/main.rs` — `context-graph-mcp` server (manual arg parser, 3,016 lines).
- `crates/context-graph-mcp/src/server/{mod.rs,transport.rs}` — `TransportMode` + TCP server.
- `crates/context-graph-benchmark/Cargo.toml` — benchmark binaries (excluded from default builds).
- `Makefile` — workspace make targets.

Cross-references: [01_system_overview.md](01_system_overview.md), [03_configuration.md](03_configuration.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md), [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md).

## 1. Workspace Binaries

| Binary | Crate | Path | Purpose |
| --- | --- | --- | --- |
| `context-graph-cli` | context-graph-cli | `src/main.rs` | Operator CLI (clap derive) — sessions, memory, hooks, exports, blackbox, ccreality, dynamicjepa |
| `context-graph` | context-graph-cli | `src/bin/context-graph.rs` | Short-name alias for `context-graph-cli` (6-line shim that re-runs `main()`) |
| `blackbox` | context-graph-cli | `src/bin/blackbox.rs` | BlackBoxFix verified-AgentOps ledger entrypoint (re-uses `BlackboxCli` parser) |
| `reality-loop` | context-graph-cli | `src/bin/reality-loop.rs` | ContextGraph reality engine — runs the inner-actor SWE-bench attempts and synthetic smokes; writes durable artifacts. Pure reality engine after Phase 1+ of the ccreality cutover (the outer optimizer is Claude Code talking via the `cgreality` MCP server) |
| `context-graph-mcp` | context-graph-mcp | `src/main.rs` | JSON-RPC 2.0 MCP server — stdio default, TCP optional, daemon mode |
| benchmark binaries | context-graph-benchmark | `src/bin/*.rs` | 30+ targeted benchmarks (excluded from default builds) |

Run examples:

```bash
cargo run -p context-graph-cli -- <subcommand>
cargo run -p context-graph-cli --bin blackbox -- <subcommand>
cargo run -p context-graph-cli --bin reality-loop -- attempt --task-manifest path/to/task.json --official-eval-each-attempt
cargo run -p context-graph-mcp -- --transport tcp --port 3100
cargo run -p context-graph-benchmark --bin <bench-name>      # explicit -p required
```

## 2. `context-graph-cli` Top-Level Subcommands

Source: `crates/context-graph-cli/src/main.rs:42-145` (`enum Commands`).

| Subcommand | Module | Purpose |
| --- | --- | --- |
| `session` | `commands/session/` | Persist / restore identity snapshot |
| `hooks` | `commands/hooks/` | Claude Code native hook event dispatch |
| `memory` | `commands/memory/` | Capture (hooks/responses) and inject context |
| `topic` | `commands/topic.rs` | Topic portfolio, stability, detection |
| `divergence` | `commands/divergence.rs` | Divergence alerts vs recent activity |
| `dynamicjepa` | `commands/dynamicjepa.rs` | DynamicJEPA registry/adapter/panel/binding/trajectory/dataset/source-of-truth surface (45 variants) |
| `learning` | `commands/learning.rs` | Learning-event ingest / list / export |
| `utl` | `commands/utl.rs` | UTL learner-state ops + retrieval policy |
| `setup` | `commands/setup.rs` | Bootstrap `.claude/settings.json` + hook scripts |
| `warmup` | `commands/warmup.rs` | Pre-load embedding models to VRAM |
| `watch` | `commands/watch.rs` | Watch markdown directory into a session |
| `export training-corpus` | `commands/export_training.rs` | Phase-6 Parquet dump of `CF_TRAINING_RECORDS` |
| `export dynamicjepa-episodes` | `commands/export_dynamicjepa.rs` | DynamicJEPA bundle → JSONL episodes |
| `backfill-e14` | `commands/backfill_e14.rs` | Re-embed missing E14 BGE-M3 dense vectors |
| `blackbox` | `commands/blackbox.rs` | BlackBoxFix verified-AgentOps ledger (Phases 1–18) |
| `ccreality` | `commands/ccreality/mod.rs` | ccreality hook support commands |

Top-level flag: `--verbose` (`-v`, count action) — `0` reads `RUST_LOG` else `warn`; `1` ⇒ `info`; `2` ⇒ `debug`; `3+` ⇒ `trace`. Logs stream to `stderr`.

## 3. `reality-loop` Binary

Source: `crates/context-graph-cli/src/bin/reality-loop.rs`. Top-level clap derive at `:76-88`.

```text
reality-loop <SUBCOMMAND>

Subcommands:
  attempt          Run one or more inner-model SWE-bench repair attempts. Writes attempt artifacts.
  smoke            Run a synthetic smoke attempt. No inner model, no Docker. For wiring verification.
  official-eval    Re-run official SWE-bench Docker evaluation against an existing attempt patch.
  targeted-test    Apply an existing attempt patch and run one exact fail_to_pass pytest target.
  replay           Replay an existing attempt's reality-signal-packet builder against current code.
  verify           Full-state-verify every JSON artifact under a runtime root.
  harness          Governed harness operations (open file windows, apply edits, run commands, verify state).
                   Designed to be invoked as a fresh subprocess per call so the on-disk binary is always
                   the source of truth — the MCP server shells out to these to avoid stale-link staleness.
```

### 3.1 Constants (top of file, `:26-75`)

| Constant | Value |
| --- | --- |
| `DEFAULT_RUNTIME_ROOT` | `/tmp/contextgraph-rust-reality-loop` |
| `DEFAULT_CONTEXT_GRAPH_BIN` | `target/debug/context-graph` |
| `DEFAULT_MODEL` | `qwen3-coder:30b-a3b` |
| `DEFAULT_INNER_MODEL_PATH` | `models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf` |
| `DEFAULT_INNER_MODEL_PIN_PATH` | `config/ccreality_model_pins.json` |
| `DEFAULT_INNER_MODEL_PIN_KEY` | `qwen3_coder_30b_a3b_q4_k_xl` |
| `DEFAULT_GPU_MIN_FREE_MB` | `1536` (per-chat free-VRAM floor; tightened from 8192 once the model is resident) |
| `DEFAULT_GPU_PRELOAD_MIN_FREE_MB` | `18432` (loop-start free-VRAM floor before model load) |
| `DEFAULT_INNER_MIN_CONTEXT_TOKENS` | `128_000` |
| `DEFAULT_INNER_MAX_NEW_TOKENS` | `8192` |
| `DEFAULT_INNER_TEMPERATURE` | `0.0` |
| `DEFAULT_INNER_TOP_P` | `0.9` |
| `DEFAULT_INNER_BATCH_SIZE` | `2048` |
| `DEFAULT_INNER_SEED` | `1` |
| `DEFAULT_AGENT_TOOL_ROUNDS` | `24` |
| `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP` | `20` (independent of `--agent-tool-rounds`) |
| `REPEATED_ERROR_DEADLOCK_LEN` | `5` |
| `DEFAULT_OFFICIAL_DOCKER_HOST` | `unix:///var/run/docker.sock` |
| `DEFAULT_OFFICIAL_DOCKER_CONFIG` | `/home/cabdru/.cache/contextgraph-docker-config` |
| `DEFAULT_SMOKE_RUNTIME_ROOT` | `/tmp/cgreality-smoke` |
| `DEFAULT_REPLAY_OUTPUT_LABEL` | `replay` |
| `DATASET_NAME` | `princeton-nlp/SWE-bench_Lite` |
| `DYNAMICJEPA_REALITY_DOMAIN_PACK` | `contextgraph_swe_reality_loop_v1` |
| `DEFAULT_DYNAMICJEPA_SKILL_ID` | `d91e5e0c-3557-57a9-8b48-0fa2acb429a0` |

The inner model is loaded directly via the `llama-cpp-2` Rust bindings (not Ollama). No HTTP client; the GGUF on disk is the model. The `--model` flag is a stable label embedded in artifacts and BlackBox ledger rows.

### 3.2 `reality-loop attempt`

Source: `AttemptArgs` at `:257-350`.

| Flag | Type | Default | Required? | Notes |
| --- | --- | --- | --- | --- |
| `--model <NAME>` | `String` | `qwen3-coder:30b-a3b` | no | Stable label only |
| `--inner-model-path <PATH>` | `PathBuf` | `models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf` | no | GGUF file loaded by llama-cpp-2 |
| `--inner-chat-template <NAME-OR-TEMPLATE>` | `Option<String>` | none | no | Override llama.cpp chat-template name or text |
| `--task-manifest <PATH>` | `PathBuf` | — | **yes** | SWE-bench Lite instance manifest |
| `--runtime-root <PATH>` | `PathBuf` | `/tmp/contextgraph-rust-reality-loop` | no | Per-loop scratch root |
| `--context-graph-bin <PATH>` | `PathBuf` | `target/debug/context-graph` | no | Used to spawn `blackbox` subcommands |
| `--max-attempts <N>` | `u64` | `1` | no; must be ≥ 1 | Else `RUST_REALITY_MAX_ATTEMPTS_INVALID` |
| `--run-until-resolved` | flag | `false` | no | Hard-rejected unless `--official-eval-each-attempt` is also set |
| `--smoke-test` | flag | `false` | no | Bypass model+Docker; for wiring tests |
| `--official-eval-each-attempt` | flag | `false` | required for any SWE run | Else `RUST_REALITY_OFFICIAL_EVAL_REQUIRED` |
| `--swebench-python <PATH>` | `PathBuf` | `/mnt/d/contextgraph/cache/contextgraph-swebench/venv/bin/python` | no | Interpreter for `python -m swebench.harness.run_evaluation` |
| `--official-timeout <SEC>` | `u64` | `900` | no | Per-instance Docker eval timeout |
| `--official-max-workers <N>` | `u64` | `1` | no | `--max_workers` for SWE-bench harness |
| `--official-cache-level <LEVEL>` | `String` | `env` | no | `--cache_level` for SWE-bench harness |
| `--official-docker-host <URI>` | `String` | `unix:///var/run/docker.sock` | no | `DOCKER_HOST` env for SDK |
| `--official-docker-config <PATH>` | `PathBuf` | `/home/cabdru/.cache/contextgraph-docker-config` | no | `DOCKER_CONFIG`; auto-bootstraps `config.json: {auths:{}}` |
| `--gpu-index <N>` | `u64` | `0` | no | NVIDIA GPU index |
| `--gpu-min-free-mb <N>` | `u64` | `1536` | no | Free-VRAM floor checked at every chat call (post-load) |
| `--gpu-preload-min-free-mb <N>` | `u64` | `18432` | no | Free-VRAM floor at loop_start (pre-load) |
| `--gpu-max-used-mb <N>` | `Option<u64>` | unset | no | Optional ceiling on used VRAM |
| `--inner-context-tokens <N>` | `Option<u32>` | unset | no | Override; otherwise uses GGUF's full training context |
| `--inner-max-new-tokens <N>` | `usize` | `8192` | no | Per-turn output cap |
| `--inner-temperature <F>` | `f32` | `0.0` | no | Sampling temperature |
| `--inner-top-p <F>` | `f32` | `0.9` | no | Nucleus sampling |
| `--inner-gpu-layers <N>` | `u32` | `u32::MAX` | no | llama.cpp `n_gpu_layers` |
| `--inner-batch-size <N>` | `usize` | `2048` | no | llama.cpp `n_batch` |
| `--inner-seed <N>` | `u32` | `1` | no | Sampling seed |
| `--inner-model-pin-path <PATH>` | `PathBuf` | `config/ccreality_model_pins.json` | no | sha256/size manifest |
| `--inner-model-pin-key <KEY>` | `String` | `qwen3_coder_30b_a3b_q4_k_xl` | no | Active entry under `.models` |
| `--git-timeout <SEC>` | `u64` | `600` | no | clone/fetch/reset/clean/diff cap |
| `--agent-tool-rounds <N>` | `u64` | `24` | no | Governed tool-round budget per attempt; capped by `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP=20` |
| `--targeted-test-timeout <SEC>` | `u64` | `300` | no | Per-targeted-test wallclock |
| `--sleep-between-attempts-ms <MS>` | `u64` | `0` | no | Optional inter-attempt pause |
| `--dynamicjepa-db <PATH>` | `Option<PathBuf>` | unset | no | RocksDB used to ingest attempt panels & run trained DJ inference |
| `--dynamicjepa-artifact-id <ID>` | `Option<String>` | unset | no | Trained DJ artifact id; verified before inference |
| `--dynamicjepa-skill-id <UUID>` | `String` | `d91e5e0c-3557-57a9-8b48-0fa2acb429a0` | no | Skill id used when persisting DJ plan traces |

### 3.3 `reality-loop smoke`

Source: `SmokeArgs` at `:175-185`. No model load, no Docker. For wiring verification.

| Flag | Type | Default | Notes |
| --- | --- | --- | --- |
| `--runtime-root <PATH>` | `PathBuf` | `/tmp/cgreality-smoke` | scratch root |
| `--label <STR>` | `String` | `smoke` | appended to synthetic instance id |
| `--force-fail` | flag | `false` | force-fail to exercise failure-artifact writing |

### 3.4 `reality-loop official-eval`

Source: `OfficialEvalArgs` at `:200-227`. Re-runs the official Docker evaluator against an existing attempt's patch.

| Flag | Type | Default |
| --- | --- | --- |
| `--runtime-root <PATH>` | `PathBuf` | `/tmp/contextgraph-rust-reality-loop` |
| `--run-id <ID>` | `String` | required |
| `--target-instance <INSTANCE>` | `String` | required |
| `--attempt <N>` | `u64` | required |
| `--model <NAME>` | `String` | `qwen3-coder:30b-a3b` |
| `--swebench-python <PATH>` | `PathBuf` | `/mnt/d/contextgraph/cache/contextgraph-swebench/venv/bin/python` |
| `--official-timeout <SEC>` | `u64` | `900` |
| `--official-max-workers <N>` | `u64` | `1` |
| `--official-cache-level <STR>` | `String` | `env` |
| `--official-docker-host <URI>` | `String` | `unix:///var/run/docker.sock` |
| `--official-docker-config <PATH>` | `PathBuf` | `/home/cabdru/.cache/contextgraph-docker-config` |

### 3.5 `reality-loop targeted-test`

Source: `TargetedTestArgs` at `:229-245`. Apply an existing attempt's patch and run one exact `fail_to_pass` pytest target.

| Flag | Type | Default |
| --- | --- | --- |
| `--runtime-root <PATH>` | `PathBuf` | `/tmp/contextgraph-rust-reality-loop` |
| `--run-id <ID>` | `String` | required |
| `--target-instance <INSTANCE>` | `String` | required |
| `--attempt <N>` | `u64` | required |
| `--test-id <NODE-ID>` | `String` | required |
| `--timeout-secs <SEC>` | `u64` | `300` |
| `--git-timeout <SEC>` | `u64` | `600` |

### 3.6 `reality-loop replay`

Source: `ReplayArgs` at `:187-198`. Re-run the reality-signal-packet builder against the current repository state for an existing attempt-summary.

| Flag | Type | Default |
| --- | --- | --- |
| `--attempt-summary-path <PATH>` | `PathBuf` | required |
| `--output-dir <PATH>` | `Option<PathBuf>` | sibling of attempt-summary |
| `--label <STR>` | `String` | `replay` |

### 3.7 `reality-loop verify`

Source: `VerifyArgs` at `:247-255`. Full-state-verify every JSON artifact under a runtime root.

| Flag | Type | Default |
| --- | --- | --- |
| `--runtime-root <PATH>` | `PathBuf` | required |
| `--attempt <N>` | `Option<u64>` | unset (verifies all attempts when omitted) |

### 3.8 `reality-loop harness <SUB>`

Source: `HarnessArgs` at `:110-173`. Invoked as fresh subprocesses by the MCP server (the MCP→CLI shim, see [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md)). Returns a JSON envelope on stdout. Exit `0` ⇒ success; exit `1` ⇒ tool-level error envelope on stdout; exit `2` ⇒ clap parse failure (RealityError JSON on stderr).

| Sub-subcommand | Args | Stdin payload | Behavior |
| --- | --- | --- | --- |
| `open-window` | `--path <REPO-RELATIVE>`, `--start-line <U64>`, `--end-line <U64>` | none | Read a window of lines from a governed file. Emits JSON envelope on stdout including observed sha256. |
| `apply-line-window-edit` | `--tool-dir <PATH>`, `--timeout-secs <U64=120>` | JSON `{"path","start_line","end_line","observed_sha256","replace"}` | Apply line-window edit; runs cargo check on touched crate; emits diff/transition artifacts under `--tool-dir`. |
| `run-command` | `--tool-dir <PATH>`, `--timeout-secs <U64=120>` | JSON `{"command":"<allowlisted shell command>"}` | Run an allowlisted shell command via the governed runner. Output artifact under `--tool-dir`. |
| `verify-state` | `--tool-dir <PATH>`, `--scope <STR=reality_loop_only>`, `--label <STR=cli>` | none | Verify project state (cargo fmt + check + targeted test). `--scope` is `reality_loop_only` or `full`. |

Stdin is preferred for line-window edits and run-command to avoid argv length and quoting hazards.

### 3.9 Per-attempt deadlines

| Variable | Default | Source |
| --- | --- | --- |
| `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` | `7200` (120 min) | `reality-loop.rs:4334` |

The reality-loop binary reads ONLY this env var. All other behavior is via CLI flags.

### 3.10 Task manifest contract

Required keys (validated by `validate_task_manifest_contract`, `:2238`):

- `dataset` — must equal `"princeton-nlp/SWE-bench_Lite"` (constant `DATASET_NAME`).
- `instance_id` — non-empty, must contain `"__"`.
- `repo` — `owner/repo`, no `..` segments.
- `base_commit` — exact 40-char hex SHA (`is_40_hex`).
- `problem_statement` — non-empty.
- `fail_to_pass` — non-empty array of test ids.
- `pass_to_pass_count` — positive integer.
- `source_of_truth` — must contain both `DATASET_NAME` and `instance_id`.
- Optional: `official_test_patch_sha256`, `oracle_patch_sha256`.

### 3.11 Exit semantics

| Exit | Stdout sentinel | Stderr |
| --- | --- | --- |
| `0` | `RUST_REALITY_LOOP_SUMMARY=<source_of_truth.summary>` | tracing logs |
| `1` | tool error envelope (harness subcommands) | pretty `RealityError` JSON; `RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH=<file>` reference; failure artifact persisted under `<runtime_root>/.../failures/` |
| `2` | (none) | `RealityError {error_code: RUST_REALITY_CLI_PARSE_FAILED}` + clap diagnostic |

### 3.12 Error-code namespace

Every reality-loop error code starts `RUST_REALITY_*`. Selected groups:

- Args/contract: `..._CLI_PARSE_FAILED`, `..._MAX_ATTEMPTS_INVALID`, `..._CONTINUOUS_REQUIRES_OFFICIAL_EVAL`, `..._OFFICIAL_EVAL_REQUIRED`, `..._CONTEXT_GRAPH_BIN_MISSING`, `..._TASK_MANIFEST_MISSING`, `..._TASK_DATASET_MISMATCH`, `..._TASK_INSTANCE_ID_INVALID`, `..._TASK_REPO_INVALID`, `..._TASK_BASE_COMMIT_INVALID`, `..._TASK_PROBLEM_STATEMENT_MISSING`, `..._TASK_FAIL_TO_PASS_MISSING`, `..._TASK_PASS_TO_PASS_MISSING`, `..._TASK_SOURCE_OF_TRUTH_INCOMPLETE`.
- Workspace/git: `..._WORKSPACE_PARENT_MISSING`, `..._GIT_CLONE_FAILED`, `..._GIT_FETCH_FAILED`, `..._GIT_RESET_FAILED`, `..._GIT_CLEAN_FAILED`, `..._GIT_HEAD_READ_FAILED`, `..._BASE_COMMIT_READBACK_MISMATCH`, `..._GIT_STATUS_READ_FAILED`, `..._WORKSPACE_NOT_CLEAN_AFTER_RESET`, `..._GIT_DIFF_FAILED`, `..._GIT_DIFF_NAME_ONLY_FAILED`, `..._PATCH_APPLY_FAILED`, `..._PATCH_MODIFIES_TESTS`, `..._PATCH_TOUCHES_BENCHMARK`.
- Process: `..._PROCESS_SPAWN_FAILED`, `..._PROCESS_WAIT_FAILED`, `..._PROCESS_OUTPUT_FAILED`, `..._CHMOD_VERIFIER_FAILED`.
- BlackBoxFix: `..._BLACKBOX_COMMAND_FAILED`, `..._BLACKBOX_JSON_INVALID`, `..._BLACKBOX_RUN_ID_MISSING`, `..._BLACKBOX_VERIFY_NON_JSON`.
- Inner-model: `..._MODEL_RESPONSE_JSON_INVALID`, `..._MODEL_PATCH_MISSING`, `..._REPEATED_ERROR_DEADLOCK`.
- Official eval: `..._SWEBENCH_PYTHON_MISSING`, `..._OFFICIAL_DOCKER_UNAVAILABLE`, `..._OFFICIAL_DOCKER_PREFLIGHT_JSON_INVALID`, `..._OFFICIAL_EVAL_FAILED`, `..._OFFICIAL_REPORT_INSTANCE_MISSING`.
- GPU: `..._GPU_*` (free-MB / used-MB violations).
- Harness: `..._HARNESS_STDIN_READ_FAILED`, `..._HARNESS_STDIN_EMPTY`, `..._HARNESS_STDIN_JSON_INVALID`, `..._HARNESS_RUN_COMMAND_FIELD_MISSING`, `..._HARNESS_VERIFY_SCOPE_INVALID`.

## 4. `session` Subcommands

Source: `crates/context-graph-cli/src/commands/session/mod.rs`.

| Subcommand | Args | Notes |
| --- | --- | --- |
| `persist` | `--db-path <PATH>` | Persist session identity (in-memory `SessionCache`, not RocksDB) |
| `restore` | `--db-path <PATH>`, `--format <text\|json>` | Read back snapshot |

## 5. `memory` Subcommands

Source: `commands/memory/`.

| Subcommand | Key flags |
| --- | --- |
| `capture` | `--content`, `--hook-type` (default `hook`), `--importance` (default `0.5`, clamped `[0,1]`), `--response-type`, `--session-id`, `--source`, `--tool-name` |
| `capture-response` | Variant for response capture (`stop_response` default) |
| `inject` | `--budget <tokens>`, `--top-k` (default `10`), `--session-id`, optional `--brief` |

## 6. `hooks` Subcommands

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
| `generate-config` | `GenerateConfigArgs` |

Each hook reads JSON `HookInput` on stdin and writes `HookOutput` JSON on stdout (`commands/hooks/types.rs`).

## 7. `topic` Subcommands

Source: `commands/topic.rs`.

| Subcommand | Flags |
| --- | --- |
| `portfolio` | `--hours`, `--format <text\|json>`, `--force` |
| `stability` | `--hours`, `--json` |
| `detect` | `--hours`, `--force` |

## 8. `divergence`

Source: `commands/divergence.rs`. Uses semantic embedders only (E1, E5, E6, E7, E10, E12, E13); E2–E4 excluded.

| Subcommand | Flags |
| --- | --- |
| `check` | `--hours` (required), `--format <text\|json>` |

## 9. `dynamicjepa` Subcommands

Source: `commands/dynamicjepa.rs`. `DynamicJepaCommands` enum has 45 variants (`:94-184`).

Domain registry / inspection: `register-domain`, `list-domains`, `get-domain`, `inspect-counts`, `inspect-cf`.
Ingest / adapters: `ingest-event`, `run-adapter`, `materialize-panel`, `get-panel`, `list-instrument-readings`.
Bindings / trajectories: `bind`, `list-bindings`, `compile-trajectories`, `get-trajectory`, `list-trajectories`.
Datasets: `compile-dataset`, `get-dataset-shard`, `inspect-dataset`.
Training / artifacts: `train`, `get-training-run`, `get-artifact`.
Predictions / planning: `predict`, `plan`, `record-surprise`, `get-prediction`, `get-plan-trace`, `get-surprise`.
Constellations / calibration: `build-constellation`, `list-constellations`, `get-constellation`, `calibrate-threshold`, `recalibrate-threshold`.
Cross-domain / audit: `compute-mc-ratio`, `audit-pairwise-mi`, `cross-domain-transfer`.
Verification: `verify-counter-world`, `verify-gridworld`, `verify-career-taxonomy`, `research-smoke`, `hp-sweep`, `multiseed-aggregate`.
Reference capture/check: `capture-reference`, `check-reference`, `check-release-aggregate-reference`.

Most subcommands accept `--db <PATH>` (env-fallback `CONTEXT_GRAPH_STORAGE_PATH`) and `--json` for machine-readable output. Cross-link [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md) and [03_configuration.md §26](03_configuration.md) (domain-pack TOML schema).

## 10. `learning` Subcommands

Source: `commands/learning.rs`.

| Subcommand | Args |
| --- | --- |
| `list` | `--storage`, `--limit`, `--offset`, `--include-text` |
| `get` | `--storage`, `--event-id`, `--include-text` |
| `export-events-jsonl` | `--storage`, `--out`, `--limit`, `--clear-existing`, `--overwrite`, `--batch-size` |
| `export-dataset-jsonl` | `--storage`, `--dataset-id`, `--out`, `--overwrite` |
| `record-events-jsonl` | inverse of export |
| `record-synthetic` | seed test events |

## 11. `utl` Subcommands

Source: `commands/utl.rs`.

| Subcommand | Args |
| --- | --- |
| `record-synthetic` | `--storage`, `--learner-id`, `--session-ts`, `--clear-existing` |
| `get-session` | `--storage`, `--learner-id`, `--session-ts` |
| `get-trace` | `--storage`, `--trace-id` |
| `asset-preflight` | `--models-root`, `--calibration-root`, `--allow-missing` |

## 12. `export` Subcommands

Source: `commands/export_training.rs`, `commands/export_dynamicjepa.rs`.

| Subcommand | Flags | Schema |
| --- | --- | --- |
| `export training-corpus` | `--storage`, `--out`, `--limit`, `--batch-size` | Parquet `(memory_id: Utf8, record_bytes: Binary)` — bincode-with-version-byte payload (`TRAINING_RECORD_VERSION = 3`) |
| `export dynamicjepa-episodes` | `ExportDynamicJepaEpisodesArgs` | JSONL — `EPISODE_SCHEMA_VERSION = 1`, requires `bundle.schema_version = 2` |

## 13. `backfill-e14`

Source: `commands/backfill_e14.rs`.

| Flag | Purpose |
| --- | --- |
| `--storage <PATH>` | Target RocksDB |
| `--models-dir <PATH>` | BAAI/bge-m3 snapshot root (default `./models/`) |
| `--dry-run` | Count candidates, no model load, no writes |
| `--limit <N>` | Cap fingerprints processed |
| `--scan-batch <N>` | Iterator chunk size |
| `--log-every <N>` | Progress cadence |

## 14. `blackbox` Subcommands

Source: `commands/blackbox.rs`. `BlackboxCli` parser (`:124-136`); `BlackboxCommands` enum (`:138-241`). Constants:

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
| `record run/step/model-decision/tool-call/dynamicjepa-reality` | `Record { RecordCommands }` (272) | 1–2 — append events |
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
| `reward compile` | `Reward { RewardCommands }` (886) | reward/loss sample compiler (consumes `config/reward_spec.verified_outcome.json`) |
| `demo first-wedge` | `Demo { DemoCommands }` (892) | 18 — vulnerability-remediation demo |
| `fsv certify` | `Fsv { FsvCommands }` (899) | full-state-verification certificate |
| `inspect ledger/summary/runs/timeline/evidence/verifiers/errors/exports` | `Inspect { InspectCommands }` (1272) | any — read-only views |

The `BlackboxCli` parser is reused by `context-graph-cli blackbox …` (via `commands::blackbox::handle_blackbox_command`) AND the standalone `blackbox` binary at `src/bin/blackbox.rs`.

Every subcommand's `--ledger <PATH>` flag has an `env = "BLACKBOX_LEDGER"` clap fallback so a single env var pins the active ledger across all subcommands.

## 15. `ccreality` Subcommands

Source: `commands/ccreality/mod.rs`.

| Subcommand | Args |
| --- | --- |
| `record-harness-transition` | `--run-id`, `--attempt`, `--session-id`, `--tool-use-id`, `--tool-name`, `--file-path` (required), `--before-sha256` (required), `--after-sha256` (required), `--lines-added`, `--lines-removed`, `--git-diff-stat`, `--cargo-status`, `--cargo-stderr-tail`, `--timeout-seconds` (default `20`), `--format <json\|json-compact>` |

Used by the `PostToolUse` hook to record harness transitions through the cgreality MCP tool.

## 16. `setup`, `warmup`, `watch`

Source: `commands/setup.rs`, `commands/warmup.rs`, `commands/watch.rs`.

| Command | Args |
| --- | --- |
| `setup` | `--target-dir`, `--force`, `--skip-chmod` |
| `warmup` | `--timeout-secs`, `--skip-if-warm` (~20–30 s on RTX 5090) |
| `watch` | `--path <DIR>`, `--session-id <ID>`, `--db-path` (env fallback `CONTEXT_GRAPH_DATA_DIR`) |

## 17. `context-graph-mcp` Server CLI

Source: `crates/context-graph-mcp/src/main.rs:72-101` (`CliArgs`), parser at `:108-213` (manual `argv` walk — no clap dep). `TransportMode` enum at `crates/context-graph-mcp/src/server/mod.rs` (variants `Stdio` (default) and `Tcp`).

| Flag | Effect | Source |
| --- | --- | --- |
| `--config <PATH>` | Load TOML config (overrides defaults) | `main.rs:130` |
| `--transport <stdio\|tcp>` | Transport mode. `sse` is rejected with `[E_CLI_TRANSPORT_001]` (see `determine_transport_mode` `:299`) | `main.rs:136` |
| `--mode <default\|reality-loop>` | MCP server mode; invalid values rejected with `[E_CLI_MODE_001]` | `main.rs:142` |
| `--port <u16>` | TCP port (config default `3100`); rejected with `[E_CLI_PORT_001]` if invalid | `main.rs:154` |
| `--bind <ADDR>` | TCP bind address (default `127.0.0.1`) | `main.rs:165` |
| `--warm-first` | Block on warmup (default ON; cancels prior `--no-warm`) | `main.rs:171` |
| `--no-warm` | Skip blocking warmup; models load in background | `main.rs:175` |
| `--daemon` | Connect to or spawn daemon (TCP, default port `3100`) | `main.rs:179` |
| `--daemon-port <u16>` | Daemon TCP port (rejected with `[E_CLI_PORT_003]` if invalid) | `main.rs:183` |
| `--daemon-server-only` | Hidden — internal flag for the spawned daemon process | `main.rs:196` |
| `-h` / `--help` | Show usage (`print_help`) | `main.rs:127` |

Priority order: CLI flag > env var > config file > built-in default.

- `--transport` overrides `CONTEXT_GRAPH_TRANSPORT`, `config.mcp.transport`.
- `--port` overrides `CONTEXT_GRAPH_TCP_PORT`, `config.mcp.tcp_port`.
- `--bind` overrides `CONTEXT_GRAPH_BIND_ADDRESS`, `config.mcp.bind_address`.
- `RUST_LOG` controls tracing-subscriber filter.

The default `max_connections` for TCP is `32` (`Config::default_config().mcp.max_connections`).

### TCP error codes (`server/mod.rs::test_tcp_error_codes_exist`)

| Code | Constant | Range |
| --- | --- | --- |
| `-32110` | `TCP_BIND_FAILED` | reserved `-32110…-32119` |
| `-32111` | `TCP_CONNECTION_ERROR` | |
| `-32112` | `TCP_MAX_CONNECTIONS_REACHED` | |
| `-32113` | `TCP_FRAME_ERROR` | |
| `-32114` | `TCP_CLIENT_TIMEOUT` | |

### JSON-RPC envelope

Source: `crates/context-graph-mcp/src/protocol.rs`.

- `JsonRpcRequest { jsonrpc: "2.0", id: Option<JsonRpcId>, method: String, params: Option<Value> }`
- `JsonRpcResponse { jsonrpc: "2.0", id: Option<JsonRpcId>, result: Option<Value>, error: Option<JsonRpcError> }`
- `JsonRpcId = Number(i64) | String(String)` (untagged)
- Methods: `initialize`, `shutdown`, `tools/list`, `tools/call`
- Standard codes: `-32700 PARSE_ERROR`, `-32600 INVALID_REQUEST`, `-32601 METHOD_NOT_FOUND`, `-32602 INVALID_PARAMS`, `-32603 INTERNAL_ERROR`
- Extended codes: `-32001 FEATURE_DISABLED`, `-32002 NODE_NOT_FOUND`, `-32004 STORAGE_ERROR`, `-32005 EMBEDDING_ERROR`, `-32006 TOOL_NOT_FOUND`, `-32007 LAYER_TIMEOUT`, `-32021 INSUFFICIENT_MEMORIES`

### Stdio batch + notification handling

- Batch JSON-RPC supported: `[{...},{...}]` per spec section 6.
- `MAX_BATCH_SIZE` (in `server::transport`) caps batch size; exceeding returns `INVALID_REQUEST`.
- Empty batch array → `INVALID_REQUEST`.
- Per-request timeout via `config.mcp.request_timeout`; on timeout for a notification (no `id`) the response is suppressed.
- Notifications (no `id` field) never receive responses (suppressed via `result=None && error=None && id=None`).
- `read_line_bounded(MAX_LINE_BYTES)` enforces per-line size.

## 18. Daemon Control

| Operation | Mechanism |
| --- | --- |
| Start daemon | `context-graph-mcp --daemon [--daemon-port 3100]` (idempotent — connects if already running) |
| Stop daemon | Send SIGTERM to PID in lock file (`~/.cache/cgreality/reality-loop-stdio.lock` for the reality-loop MCP singleton; `/tmp/context-graph-mcp-daemon.lock` for the standard daemon) |
| Singleton takeover | `run_reality_loop_stdio()` acquires a flock at startup; if a previous reality-loop MCP holds the lock, the new MCP SIGTERM(2s)→SIGKILL evicts and reclaims it |
| Health probe | TCP connect to `127.0.0.1:<daemon_port>`, send JSON-RPC `initialize` ping |
| Stale-lock detection | flock + `kill(pid, 0)` + `/proc/<pid>/status` zombie check |
| Storage path resolution | `McpServer::resolve_storage_path` (`server/mod.rs:1191`): `CONTEXT_GRAPH_STORAGE_PATH` env > `config.storage.path` > `<exe_dir>/contextgraph_data` |
| Models path resolution | `McpServer::resolve_models_path` (`server/mod.rs:1243`): `CONTEXT_GRAPH_MODELS_PATH` env > `<exe_dir>/models` |
| Background tasks | GC (5 min, retention 7d), HNSW persist + compaction (10 min), checkpoint (6h), graph builder (configurable). All await `JoinHandle` on shutdown |
| Shutdown ordering (`server/mod.rs:1073`) | watch-channel signal → GC (5s) → model loader (60s) → HNSW persist (10s) → graph/code/file watchers → final HNSW persist → RocksDB flush |

## 19. Makefile Targets

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

The 5090 JEPA archive targets require: `RELEASE_AGGREGATE`, `RELEASE_LOCAL_CI_MANIFEST`, `RELEASE_DOCKER_TAR_1`, `RELEASE_DOCKER_TAR_2`, `RELEASE_CONTAINER_VERIFY_JSON` (plus `RELEASE_APPTAINER_SIF` for `-v2`). Optional: `RELEASE_ID`, `RELEASE_OUTPUT_ROOT`, `RELEASE_REFERENCE`, `RELEASE_FULL_BUNDLE_SEED`, `RELEASE_REQUIRE_APPTAINER`. Internal scripts: `scripts/trim-stale-deps.sh`, `scripts/disk-guard.sh`, `scripts/clean-build-artifacts.sh`, `scripts/5090jepa/prepare_release_candidate_archive*.sh`.

The reality-loop binary is NOT exposed via Makefile — invoke directly with `cargo run -p context-graph-cli --bin reality-loop --release -- <subcommand>` or via the `cgreality` MCP server (which shells out to `target/release/reality-loop` per the MCP→CLI shim).

## 20. Benchmark Invocations

Source: `crates/context-graph-benchmark/Cargo.toml` — declared `[[bin]]` entries plus 2 Criterion benches (`comparative_suite`, `scaling_analysis`).

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

## 21. Environment Variables

A condensed table of CLI-relevant env vars (full inventory in [03_configuration.md §21](03_configuration.md)).

| Variable | Read by | Effect |
| --- | --- | --- |
| `RUST_LOG` | tracing-subscriber (all bins) | trace filter |
| `CONTEXT_GRAPH_ENV` | `Config::load` | selects `config/{env}.toml` |
| `CONTEXT_GRAPH_TRANSPORT` | `context-graph-mcp` | default transport when `--transport` not set |
| `CONTEXT_GRAPH_TCP_PORT` | `context-graph-mcp` | default TCP port |
| `CONTEXT_GRAPH_BIND_ADDRESS` | `context-graph-mcp` | default bind address |
| `CONTEXT_GRAPH_STORAGE_PATH` | `context-graph-mcp`, `dynamicjepa` | sets `storage.path`; flips backend to RocksDB |
| `CONTEXT_GRAPH_MODELS_PATH` | `context-graph-mcp` | flips `embedding.model = e5-large-v2` |
| `CONTEXT_GRAPH_DAEMON` / `CONTEXT_GRAPH_DAEMON_PORT` | `context-graph-mcp` | enable daemon mode + port |
| `CONTEXT_GRAPH_WARM_FIRST` | `context-graph-mcp` | `0` disables blocking warmup |
| `CONTEXT_GRAPH_WATCHER_ENABLED` | `context-graph-mcp` | `1` enables file watcher |
| `CONTEXT_GRAPH_MCP_QUIET` | `Config::validate_phase_safety` | suppresses stderr phase banner |
| `CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT` | `context-graph-mcp` | overrides `mcp.request_timeout` |
| `CONTEXT_GRAPH_DATA_DIR` | `context-graph-cli watch` | watch storage path |
| `CONTEXT_GRAPH_MCP_HOST` / `CONTEXT_GRAPH_MCP_PORT` | `mcp_client.rs` | default MCP TCP target |
| `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` | `reality-loop` | per-attempt wallclock cap (default `7200`) |
| `CGREALITY_GOVERNED_PATHS` | MCP `path_policy.rs`, `governed_edit.rs` | path policy JSON; deny-list enforcement REMOVED 2026-05-07 |
| `CGREALITY_RECOMMENDATION_SCHEMA` | MCP `schema_validator.rs` | recommendation JSON-schema path |
| `CGREALITY_REALITY_LOOP_BIN` | MCP `interact.rs` | override path to `target/release/reality-loop` |
| `CGREALITY_MCP_BIN` / `CGREALITY_CACHE_DIR` / `CONTEXTGRAPH_ROOT` | `commands/ccreality/mod.rs` | MCP binary, cache dir, project root override |
| `CLAUDE_SESSION_ID` | hooks + MCP handlers | tags session-bound memories |
| `BLACKBOX_LEDGER` | every `blackbox` subcommand | default `--ledger` value |
| `BBX_GITHUB_TOKEN` / `GITHUB_TOKEN` / `BBX_GITLAB_TOKEN` / `GITLAB_TOKEN` | `blackbox` connectors | git-host auth |
| `EMBEDDING_*` (7 vars) | `EmbeddingConfig::with_env_overrides` | embedding subsystem overrides |
| `WARM_*` (6 vars) + `CUDA_VISIBLE_DEVICES` | `WarmConfig::from_env` | warm-load subsystem |
| `UV` / `CLIPCANNON_DIR` / `PYTHON_DEV_EXTRA` | Make | Python project paths |
| `RELEASE_*` (8 vars) | Make `release-candidate-archive*` | 5090 JEPA release archive parameters |

## 22. Exit Codes

- `0` — success.
- `1` — recoverable error (also: reality-loop tool-level error envelope on stdout).
- `2` — database/state corruption (fail fast, no retry); also: clap parse failure for `reality-loop`.
- `64+` — infrastructure crash (panic, OOM).

`commands/export_training.rs::summary_to_exit_code` and `commands/export_dynamicjepa.rs::summary_to_exit_code` translate per-export summaries into exit codes.

## 23. Verbose / Logging Behavior

- `context-graph-cli -v` → `info`, `-vv` → `debug`, `-vvv+` → `trace`. Default reads `RUST_LOG` if set, else `warn`.
- Logs stream to `stderr` so JSON exports on `stdout` remain machine-readable.
- The same pattern is used by the `blackbox` binary.
- `reality-loop` writes structured JSON to stdout for the harness subcommands, plus the `RUST_REALITY_LOOP_SUMMARY=...` sentinel on success.
