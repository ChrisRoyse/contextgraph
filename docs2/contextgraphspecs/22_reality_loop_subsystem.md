# Reality-Loop Subsystem

This subsystem owns SWE-bench Lite repair end-to-end: minting attempts, driving the inner repair actor, applying governed file edits, scoring against the official Docker SWE-bench evaluator, and persisting every step into a durable on-disk artifact tree plus a BlackBoxFix SQLite ledger. The cgreality MCP server (in `context-graph-mcp`) exposes the artifact tree to Claude Code as the outer optimizer.

**Source files covered**

| Path | Lines | Role |
| --- | --- | --- |
| `crates/context-graph-cli/src/bin/reality-loop.rs` | 18,137 | Reality engine: `attempt`/`smoke`/`official-eval`/`targeted-test`/`replay`/`verify`/`harness *` subcommands |
| `crates/context-graph-cli/src/commands/blackbox.rs` | 35,414 | BlackBoxFix ledger CLI (`blackbox init|record|verify|score|reward|...`) — `LEDGER_SCHEMA_VERSION = 14` (line 40) |
| `crates/context-graph-cli/src/bin/blackbox.rs` | — | Standalone `blackbox` binary entry |
| `crates/context-graph-cli/Cargo.toml` | — | `[[bin]] name = "blackbox"`, `[[bin]] name = "context-graph"`, `[[bin]] name = "context-graph-cli"`. `reality-loop.rs` is auto-discovered as a fourth binary by clap (no explicit `[[bin]]` entry). |
| `crates/context-graph-mcp/src/handlers/tools/reality_loop/` | 5,627 | cgreality MCP handler family |
| `crates/context-graph-mcp/src/tools/definitions/reality.rs` | 204 | cgreality tool definitions |
| `crates/context-graph-mcp/src/main.rs` (`run_reality_loop_stdio` ~ line 2229) | — | MCP-stdio server entry, singleton flock takeover |
| `config/ccreality_model_pins.json` | 16 | Cryptographic pin manifest for the inner-actor GGUF |
| `config/reward_spec.verified_outcome.json` | 56 | Reward feature/weight spec compiled per attempt |
| `configs/dynamicjepa/domain_packs/contextgraph_swe_reality_loop.v1.toml` | 1,194 | DynamicJEPA structured tool-transition domain pack `contextgraph_swe_reality_loop_v1` |

**Cross-links** — [01_system_overview.md](01_system_overview.md) §10 names this subsystem alongside DynamicJEPA. [04_database_schema.md](04_database_schema.md) describes the RocksDB column families that back the persisted artefacts. [08_mcp_tools_reference.md](08_mcp_tools_reference.md) lists the full MCP tool surface; the `mcp__cgreality__*` family documented here is one of its sections. [10_cli_reference.md](10_cli_reference.md) covers the `blackbox` ledger CLI invoked by the engine. [15_verification_report.md](15_verification_report.md) §6 documents the verifier framework. [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md) covers the latent-state subsystem that consumes the per-attempt reality blocks emitted here.

> Behavioural rule, not gate-enforced post 2026-05-07. The MCP path-policy gate (`crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs::ensure_path_allowed`, lines 244–280) only rejects empty/control-character paths; the allow/deny list is **not** consulted. The deny set lives in CLAUDE.md §8 and `./memory/context/constraints.md` and is honoured by behaviour, not by the engine or the MCP. Do not document a "path-policy gate" as enforced.

---

## 1. Component map

```
                     ┌──────────────────────────────────────┐
                     │ Claude Code (the outer optimizer)    │
                     │  - reads attempt artefacts via cgreality MCP
                     │  - records recommendation per turn   │
                     │  - mints next attempt via reality_run_attempt
                     └────────────┬─────────────────────────┘
                                  │ JSON-RPC stdio
                                  ▼
                ┌──────────────────────────────────────────────┐
                │ context-graph-mcp (cgreality stdio server)   │
                │  - 31 tools under mcp__cgreality__*          │
                │  - flock $HOME/.cache/cgreality/             │
                │      reality-loop-stdio.lock (singleton)     │
                └────────────┬─────────────────────────────────┘
                             │ fork/exec, fresh subprocess per call
                             ▼
                ┌──────────────────────────────────────────────┐
                │ target/release/reality-loop                  │
                │  - attempt | smoke | official-eval |         │
                │    targeted-test | replay | verify           │
                │  - harness {open-window | apply-line-window  │
                │             | run-command | verify-state}    │
                │  - llama-cpp-2 in-process (CUDA)             │
                │  - blackbox CLI shells out per record/verify │
                └────┬─────────────┬───────────────────────────┘
                     │             │
                     ▼             ▼
        ┌──────────────────┐  ┌──────────────────────────┐
        │ blackbox.sqlite  │  │ <runtime_root>/<run_id>/ │
        │ (ledger v14)     │  │  per-attempt JSON tree   │
        └──────────────────┘  └──────────────────────────┘
                                    │
                                    ▼
                ┌──────────────────────────────────────────────┐
                │ python -m swebench.harness.run_evaluation    │
                │  via Docker (DOCKER_HOST/DOCKER_CONFIG)      │
                │  → resolved_ids[] in final report            │
                └──────────────────────────────────────────────┘
```

The engine and MCP are coupled by **subprocess fork/exec only** (commits `4072dcf3` / `d48102e6`). The MCP harness write tools (`harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_verify_state`) shell out to `target/release/reality-loop harness <subcommand>` (`alter.rs` line 426 invokes `locate_engine_binary()`). Edits to `crates/context-graph-cli/**` propagate to the next MCP call automatically once `cargo build --release -p context-graph-cli` rebuilds — no `/mcp` reconnect required.

The MCP server itself acquires `$HOME/.cache/cgreality/reality-loop-stdio.lock` at start (`main.rs:1340`); a previous holder is SIGTERM-then-SIGKILLed before the new server takes over, so `/mcp reconnect` is structurally safe.

The deleted Python evidence console (`gemmafrontend/`) is no longer present in the workspace; reality-loop is the only canonical SWE-bench refinement entry point.

---

## 2. The `reality-loop` binary

### 2.1 Top-level subcommands

Defined in `enum Cmd` (`reality-loop.rs:91`). Every subcommand emits a JSON envelope to stdout on success (exit 0) and a typed `RealityError` JSON to stderr on failure (exit 1). The harness subcommand uses a slightly different convention — see §4.

| Subcommand | Args struct | Purpose |
| --- | --- | --- |
| `attempt` | `AttemptArgs` (`reality-loop.rs:259`) | Run one or more inner-actor SWE-bench repair attempts. The workhorse path. |
| `smoke` | `SmokeArgs` (`reality-loop.rs:176`) | Run a synthetic smoke that writes deterministic artifacts; no model load, no Docker. Used by `/ccreality-smoke`. |
| `official-eval` | `OfficialEvalArgs` (`reality-loop.rs:201`) | Re-run official Docker evaluation against an existing attempt's `candidate.patch`. |
| `targeted-test` | `TargetedTestArgs` (`reality-loop.rs:230`) | Apply an existing attempt's patch and run one exact `fail_to_pass` pytest target. |
| `replay` | `ReplayArgs` (`reality-loop.rs:188`) | Re-emit a `reality-signal-packet` envelope from an existing `attempt-summary.json`. |
| `verify` | `VerifyArgs` (`reality-loop.rs:248`) | Walk a runtime root, SHA every JSON artifact, optionally compare against `*.json.sha256` sidecars; reports drift. |
| `harness` | `HarnessArgs` (`reality-loop.rs:111`) | Governed harness operations executed as a fresh subprocess per call. See §4. |

### 2.2 Engine constants (top of `reality-loop.rs`)

| Constant | Default | Source | Notes |
| --- | --- | --- | --- |
| `DEFAULT_RUNTIME_ROOT` | `/tmp/contextgraph-rust-reality-loop` | line 26 | Default `--runtime-root` for `attempt`/`official-eval`/`targeted-test`/`verify` |
| `DEFAULT_SMOKE_RUNTIME_ROOT` | `/tmp/cgreality-smoke` | line 70 | `smoke` uses a separate root |
| `DEFAULT_CONTEXT_GRAPH_BIN` | `target/debug/context-graph` | line 27 | The blackbox CLI binary the engine forks for every ledger op |
| `DEFAULT_MODEL` | `qwen3-coder:30b-a3b` | line 28 | Stable model label embedded in artefacts |
| `DEFAULT_INNER_MODEL_PATH` | `models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf` | line 29 | Real GGUF loaded by `llama-cpp-2` in-process |
| `DEFAULT_INNER_MODEL_PIN_PATH` | `config/ccreality_model_pins.json` | line 31 | Pin manifest |
| `DEFAULT_INNER_MODEL_PIN_KEY` | `qwen3_coder_30b_a3b_q4_k_xl` | line 32 | Active pin entry |
| `DEFAULT_GPU_MIN_FREE_MB` | `1536` | line 40 | Minimum free VRAM **during** a chat call (after model resident) |
| `DEFAULT_GPU_PRELOAD_MIN_FREE_MB` | `18432` | line 48 | Minimum free VRAM **before** model load (preflight at `loop_start`) |
| `DEFAULT_INNER_MIN_CONTEXT_TOKENS` | `128_000` | line 49 | Hard floor on effective context window |
| `DEFAULT_INNER_MAX_NEW_TOKENS` | `8192` | line 50 | Per-turn generation budget |
| `DEFAULT_INNER_TEMPERATURE` | `0.0` | line 51 | Deterministic by default |
| `DEFAULT_INNER_TOP_P` | `0.9` | line 52 | |
| `DEFAULT_INNER_BATCH_SIZE` | `2048` | line 53 | llama.cpp prompt batch size |
| `DEFAULT_INNER_SEED` | `1` | line 54 | |
| `DEFAULT_AGENT_TOOL_ROUNDS` | `24` | line 55 | Per-attempt tool-round budget cap |
| `MAX_ROUNDS_PER_ATTEMPT_HARD_CAP` | `20` | line 62 | Independent of `--agent-tool-rounds`; force-stop at this round |
| `REPEATED_ERROR_DEADLOCK_LEN` | `5` | line 67 | Force-stop after this many consecutive identical non-success error codes |
| `DEFAULT_OFFICIAL_DOCKER_HOST` | `unix:///var/run/docker.sock` | line 68 | |
| `DEFAULT_OFFICIAL_DOCKER_CONFIG` | `/home/cabdru/.cache/contextgraph-docker-config` | line 69 | |
| `DATASET_NAME` | `princeton-nlp/SWE-bench_Lite` | line 72 | The only dataset accepted by manifest validation |
| `DYNAMICJEPA_REALITY_DOMAIN_PACK` | `contextgraph_swe_reality_loop_v1` | line 74 | DJ pack id ingested per attempt |
| `DEFAULT_DYNAMICJEPA_SKILL_ID` | `d91e5e0c-3557-57a9-8b48-0fa2acb429a0` | line 75 | DJ skill UUID for plan traces |
| `TOOL_RESULT_HISTORY_CAP_BYTES` | `4 * 1024` | line 2737 | Per-tool-result truncation cap for what enters chat history (full result still persists to disk) |
| `GC_RUNS_RETENTION_DAYS` | `30` | line 2775 | GC retains per-run artefact dirs this long |
| `GC_WORKSPACE_RETENTION_DAYS` | `14` | line 2776 | Workspace clones aged out and re-cloned on demand |
| `GC_SHIFT_LOG_RETENTION_DAYS` | `7` | line 2777 | Per-session shift logs |
| `GC_HOOK_EVENTS_MAX_BYTES` | 100 MiB | line 2778 | Hook events append-only file; truncated to 50 MiB on overflow |
| `GC_MIN_ENTRY_AGE_SECONDS` | `3600` | line 2780 | Safety bumper: never delete anything <1h old |

Per-attempt wallclock budget: `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` env var, default `7200` (120 min), parsed at `reality-loop.rs:4334`. Routes through the existing forced-stop machinery so `attempt-summary.json` is still written. Error code on overrun: `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED`.

### 2.3 `attempt` flag surface (`AttemptArgs`)

| Flag | Default | Source |
| --- | --- | --- |
| `--model` | `DEFAULT_MODEL` | line 261 |
| `--inner-model-path` | `DEFAULT_INNER_MODEL_PATH` | line 264 |
| `--inner-chat-template` | unset | line 267 |
| `--task-manifest` | required | line 269 |
| `--runtime-root` | `DEFAULT_RUNTIME_ROOT` | line 271 |
| `--context-graph-bin` | `DEFAULT_CONTEXT_GRAPH_BIN` | line 273 |
| `--max-attempts` | `1` | line 275 |
| `--run-until-resolved` | `false` | line 277 |
| `--smoke-test` | `false` | line 279 |
| `--official-eval-each-attempt` | `false` (loop refuses without this set) | line 281 |
| `--swebench-python` | `/home/cabdru/.cache/contextgraph-swebench/venv/bin/python` | line 283 |
| `--official-timeout` | `900` s | line 288 |
| `--official-max-workers` | `1` | line 290 |
| `--official-cache-level` | `env` | line 292 |
| `--official-docker-host` | `DEFAULT_OFFICIAL_DOCKER_HOST` | line 294 |
| `--official-docker-config` | `DEFAULT_OFFICIAL_DOCKER_CONFIG` | line 296 |
| `--gpu-index` | `0` | line 298 |
| `--gpu-min-free-mb` | `DEFAULT_GPU_MIN_FREE_MB` | line 300 |
| `--gpu-preload-min-free-mb` | `DEFAULT_GPU_PRELOAD_MIN_FREE_MB` | line 306 |
| `--gpu-max-used-mb` | unset | line 308 |
| `--inner-context-tokens` | unset (uses GGUF max) | line 311 |
| `--inner-max-new-tokens` | `DEFAULT_INNER_MAX_NEW_TOKENS` | line 313 |
| `--inner-temperature` | `DEFAULT_INNER_TEMPERATURE` | line 315 |
| `--inner-top-p` | `DEFAULT_INNER_TOP_P` | line 317 |
| `--inner-gpu-layers` | `u32::MAX` (all on GPU) | line 319 |
| `--inner-batch-size` | `DEFAULT_INNER_BATCH_SIZE` | line 321 |
| `--inner-seed` | `DEFAULT_INNER_SEED` | line 323 |
| `--inner-model-pin-path` | `DEFAULT_INNER_MODEL_PIN_PATH` | line 326 |
| `--inner-model-pin-key` | `DEFAULT_INNER_MODEL_PIN_KEY` | line 329 |
| `--git-timeout` | `600` s | line 331 |
| `--agent-tool-rounds` | `DEFAULT_AGENT_TOOL_ROUNDS` | line 333 |
| `--targeted-test-timeout` | `300` s | line 335 |
| `--sleep-between-attempts-ms` | `0` | line 337 |
| `--dynamicjepa-db` | unset | line 341 |
| `--dynamicjepa-artifact-id` | unset (must be paired with `--dynamicjepa-db`) | line 345 |
| `--dynamicjepa-skill-id` | `DEFAULT_DYNAMICJEPA_SKILL_ID` | line 348 |

The continuous-resolution loop requires `--official-eval-each-attempt`; `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL` (line 1559) blocks `--run-until-resolved` without it.

### 2.4 The inner repair actor

The inner actor is **Qwen3-Coder-30B-A3B-Instruct** loaded in-process by `llama-cpp-2 v0.x` with the `cuda` feature (Cargo.toml line 59). There is no Ollama HTTP path; the model bytes live on disk and are mmaped into the GPU at attempt start.

`InnerCodingHarness` (`reality-loop.rs:3250`) owns the loaded `LlamaModel`, chat template, KV-cache parameters, and sampler config. `InnerCodingHarness::load` (`reality-loop.rs:3496`) gates loading on:

1. **Pin file present** (`RUST_REALITY_INNER_MODEL_PIN_CONFIG_MISSING`).
2. **Pin entry resolved** by `--inner-model-pin-key` (`RUST_REALITY_INNER_MODEL_PIN_NOT_FOUND`).
3. **Pin schema** complete: `repo_id`, `filename`, `expected_sha256`, `expected_size_bytes`, `expected_min_context_tokens`, `expected_identity_substrings` (`RUST_REALITY_INNER_MODEL_PIN_SCHEMA_INVALID`).
4. **File size match** via `fs::metadata` before SHA — cheap fail-fast (`RUST_REALITY_INNER_MODEL_SIZE_MISMATCH`).
5. **Streaming SHA-256 match** (`RUST_REALITY_INNER_MODEL_SHA_MISMATCH`).
6. **GGUF loads** through `LlamaModel::load_from_file` (`RUST_REALITY_INNER_MODEL_LOAD_FAILED`).
7. **Identity readback** declares `qwen3` substring (`RUST_REALITY_INNER_MODEL_IDENTITY_MISMATCH`); also verifies `expected_identity_substrings` from the pin (`RUST_REALITY_INNER_MODEL_PIN_IDENTITY_SUBSTRING_MISSING`).
8. **`n_ctx_train` ≥ max(pin floor, `DEFAULT_INNER_MIN_CONTEXT_TOKENS`)** (`RUST_REALITY_INNER_MODEL_CONTEXT_BELOW_REQUIRED_FLOOR`).
9. **Effective context ≥ 128 K** even when the operator passes `--inner-context-tokens` (`RUST_REALITY_INNER_EFFECTIVE_CONTEXT_BELOW_REQUIRED_FLOOR`).
10. **`max_new_tokens < context_tokens`** (`RUST_REALITY_INNER_MAX_NEW_TOKENS_EXCEEDS_CONTEXT`).

The active pin in `config/ccreality_model_pins.json`:

| Field | Value |
| --- | --- |
| `repo_id` | `unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF` |
| `filename` | `Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf` |
| `expected_sha256` | `sha256:2841aa314d916434860cfb8990347528dcdfe5c350dbcb9d1461dbee88ff2533` |
| `expected_size_bytes` | `17665334432` (~17.67 GB) |
| `expected_min_context_tokens` | `128000` |
| `expected_identity_substrings` | `["qwen3"]` |

After load, the harness emits `inner-model-readback.json` (`InnerCodingHarness::model_capability_report`, `reality-loop.rs:3668`) that records `n_vocab`, `n_embd`, `n_layer`, `n_head`, `n_head_kv`, `n_params`, `size_bytes`, `metadata_count`, the chat-template SHA-256, and `selected_metadata` keys (`general.architecture`, `general.name`, `qwen3.context_length`, `tokenizer.chat_template`, etc.).

The chat call path is `InnerCodingHarness::chat` (`reality-loop.rs:3718`). It serialises messages and tools to JSON, renders an OpenAI-compatible chat template (`OpenAIChatTemplateParams`), extracts a `.gbnf` tool-call grammar from the rendered template, writes it next to the request as `tool-grammar.gbnf` (`reality-loop.rs:3823`), and uses one of three llama.cpp grammar samplers depending on the template:

| Mode | Sampler | Source |
| --- | --- | --- |
| Strict (no triggers) | `LlamaSampler::grammar(model, grammar, "root")` | line 4127 |
| Lazy with patterns | `LlamaSampler::grammar_lazy_patterns(model, grammar, triggers, ...)` | line 4090 |
| Lazy without patterns | `LlamaSampler::grammar_lazy(model, grammar, ...)` | line 4108 |

The grammar enforces well-formed tool-call JSON, eliminating the "model returns prose where it should call a tool" failure mode.

### 2.5 GPU budgeting

`read_gpu_vram_state` (`reality-loop.rs:15650`) shells out to `nvidia-smi --query-gpu=index,name,uuid,memory.total,memory.used,memory.free,driver_version --format=csv,noheader,nounits` and pairs it with `nvidia-smi --query-compute-apps=pid,process_name,used_memory ...` plus a `/proc/<pid>/{status,comm,cmdline,exe}` snapshot per compute process so VRAM blame is post-hoc attributable.

`assert_gpu_budget` (`reality-loop.rs:15909`) is invoked at three points:

| Point | Threshold flag | Failure code |
| --- | --- | --- |
| `loop_start` (preflight, before model load) | `--gpu-preload-min-free-mb` (default 18432) | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` |
| `before_inner_coding_harness` (before each chat call) | `--gpu-min-free-mb` (default 1536) | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` |
| `--gpu-max-used-mb` ceiling | optional | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` |

On a chat-error path the engine still tries to capture `gpu_after_chat_error` snapshots so OOM/leak signals survive (`reality-loop.rs:4497`).

---

## 3. The attempt loop

`run` (`reality-loop.rs:1514`) drives one full attempt sequence. Concrete phases below — every step has byte-equal-readback enforcement; failure produces a typed `RealityError`.

### 3.1 Pre-flight

| Step | Enforcement | Error code |
| --- | --- | --- |
| `--max-attempts ≥ 1` | line 1547 | `RUST_REALITY_MAX_ATTEMPTS_INVALID` |
| `--run-until-resolved` requires `--official-eval-each-attempt` | line 1557 | `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL` |
| `--smoke-test` excludes `--run-until-resolved` | line 1567 | `RUST_REALITY_SMOKE_RUN_UNTIL_RESOLVED_INVALID` |
| `--smoke-test` requires `--max-attempts 1` | line 1577 | `RUST_REALITY_SMOKE_MAX_ATTEMPTS_INVALID` |
| Inner-harness arg sanity (context, batch, temperature, top_p, dynamicjepa pairing) | `validate_inner_harness_args`, line 1422 | `RUST_REALITY_INNER_*_INVALID` |
| `context-graph` binary present | line 1589 | `RUST_REALITY_CONTEXT_GRAPH_BIN_MISSING` |
| Task manifest present and valid | line 1594, `validate_task_manifest_contract` line 2238 | `RUST_REALITY_TASK_*` (see §3.2) |
| `--official-eval-each-attempt` is set | line 1602 | `RUST_REALITY_OFFICIAL_EVAL_REQUIRED` |
| `loop_start` GPU preflight | line 1641 | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` |

`update_active_runtime_root` (`reality-loop.rs:16204`) writes `~/.cache/cgreality/active_runtime_root` and `~/.cache/cgreality/active_target_instance` so the cgreality MCP tools (`reality_latest_root`) can find the live run without env vars.

### 3.2 Task-manifest contract (`validate_task_manifest_contract`)

| Field | Rule | Error code |
| --- | --- | --- |
| `dataset` | `== "princeton-nlp/SWE-bench_Lite"` | `RUST_REALITY_TASK_DATASET_MISMATCH` |
| `instance_id` | non-empty, contains `__` | `RUST_REALITY_TASK_INSTANCE_ID_INVALID` |
| `repo` | exactly `owner/name`, no `..` | `RUST_REALITY_TASK_REPO_INVALID` |
| `base_commit` | 40 ASCII hex chars (`is_40_hex`) | `RUST_REALITY_TASK_BASE_COMMIT_INVALID` |
| `problem_statement` | non-empty | `RUST_REALITY_TASK_PROBLEM_STATEMENT_MISSING` |
| `fail_to_pass` | non-empty array | `RUST_REALITY_TASK_FAIL_TO_PASS_MISSING` |
| `pass_to_pass_count` | `> 0` | `RUST_REALITY_TASK_PASS_TO_PASS_MISSING` |
| `source_of_truth` | contains `DATASET_NAME` and `instance_id` | `RUST_REALITY_TASK_SOURCE_OF_TRUTH_INCOMPLETE` |

### 3.3 Measurement contract (`build_measurement_contract`, `reality-loop.rs:2331`)

`<run_root>/measurement-contract.json` pins the entire claim scope before any model bytes are loaded.

| Top-level field | Source |
| --- | --- |
| `schema` | literal `"contextgraph_reality_measurement_contract_v1"` |
| `claim_scope.primary_claim` | `"official_swe_instance_resolution_only"` |
| `claim_scope.success_predicate` | `"official_report.resolved_ids contains task.instance_id"` |
| `claim_scope.explicit_non_claims` | 3 explicit non-claims (no full-repo proof, no human-intent proof, no success without official Docker) |
| `oracle.{dataset, split, instance_id, fail_to_pass, pass_to_pass_count, official_test_patch_sha256, oracle_patch_sha256, source_of_truth}` | from manifest |
| `immutable_boundaries.task_manifest` | `file:<path>` |
| `immutable_boundaries.base_commit` | git SHA |
| `immutable_boundaries.repo` | `owner/name` |
| `immutable_boundaries.official_evaluator` | `"python -m swebench.harness.run_evaluation"` |
| `immutable_boundaries.official_docker_host` | `--official-docker-host` |
| `immutable_boundaries.official_docker_config` | `--official-docker-config` |
| `immutable_boundaries.model_runtime` | `provider=contextgraph_rust_inner_coding_harness`, `model_label`, `inner_model_path`, `chat_template`, `context_policy`, `requested_context_tokens`, `minimum_context_tokens`, `max_new_tokens`, `temperature`, `top_p`, `gpu_layers`, `batch_size`, `seed` |
| `immutable_boundaries.reward_spec` | `file:config/reward_spec.verified_outcome.json` |
| `mandatory_readbacks` | 10 ids: `task_manifest_readback`, `measurement_contract_readback`, `git_head_equals_base_commit`, `git_status_clean_before_attempt`, `candidate_patch_applies_to_base`, `candidate_patch_does_not_modify_tests_or_harness`, `official_docker_sdk_preflight`, `official_swe_report_with_resolved_ids`, `blackbox_sqlite_ledger_counts`, `reality_event_log_append_readback` |
| `model_editable_scope` | `["candidate.patch", "structured edit plan compiled by Rust into candidate.patch", "notes"]` |
| `model_forbidden_scope` | task manifest, official evaluator, Docker harness, BlackBoxFix ledger schema/hash chain, reward spec, verifier source, benchmark tests/fixtures/harness files |
| `failure_policy.{unmeasured_success, missing_source_of_truth, incomplete_oracle, official_evaluator_unavailable, test_or_harness_mutation}` | values: `blocked`, `error`, `error`, `error`, `failed` |

### 3.4 Per-attempt loop body

For each attempt index from `1` (uncapped if `--run-until-resolved`, otherwise to `--max-attempts`):

1. `attempt_dir = <run_root>/tasks/<instance>/<safe_id(model)>/attempts/attempt-NNNNNN/` created.
2. `reset_workspace(workspace, base_commit, git_timeout)` re-syncs the real checkout under `<runtime_root>/workspaces/<instance>/repo/`.
3. `collect_repository_context` (`reality-loop.rs:10799`) builds `repository-context.json` with `git ls-files`, prioritised production-candidate files (issue-text mention order first, then BM25-ranked from the engine), focused source snippets, and source-of-truth refs.
4. `compile_prompt` (`reality-loop.rs:14960`) materialises `prompt.txt` from problem-reality, repository-context, prior attempt evidence, and a `NO_REPEAT_PATCH_SHA256` list.
5. The engine writes `model-decision-input.json`, records a `blackbox record model-decision` row, and enters the agentic tool-round loop (`inner_coding_harness_chat_json`, `reality-loop.rs:4307`).
6. After the chat loop yields a response, the engine writes `model-response.txt`, parses the candidate patch (or compiles structured edits / line-window edits / unified-diff submissions into a unified diff), persists `model-parse-report.json`, and enforces patch-repeat detection.
7. `validate_attempt` (`reality-loop.rs:10079`) applies the patch with `git apply --whitespace=nowarn`, refuses test/harness mutation, runs `python -m py_compile` on changed Python files, and writes `validation.json` plus `actual-applied.patch`.
8. `verify_attempt` (`reality-loop.rs:10283`) shells out to `target/debug/context-graph blackbox verify` against a synthesised verifier spec (`blackbox-verify-spec.json`), reading `validation.json` to determine `passed`/`failed`.
9. If validation passed, `run_official_eval` (`reality-loop.rs:10361`) runs the SWE-bench Docker evaluator (see §3.5).
10. `build_dynamicjepa_reality_for_attempt` (`reality-loop.rs:11774`) builds the per-attempt DJ block; `live_ingest_dynamicjepa_transition` ingests live transitions when `--dynamicjepa-db` and `--dynamicjepa-artifact-id` are paired.
11. `build_reality_signal_packet` (`reality-loop.rs:11078`) merges everything into `reality-signal-packet.json` (the structured "what happened on this attempt" record consumed by the optimizer).
12. `blackbox score run` and `blackbox reward compile` are invoked; the reward spec is required (`RUST_REALITY_REWARD_SPEC_MISSING`).
13. `attempt-summary.json` is written with `attempt`, `model_response`, `blackbox_verifier`, `reality_signal_verifier`, `dynamicjepa_reality`, `dynamicjepa_reality_record`, `score`, `reward`, `ledger_counts`.
14. `write_optimizer_artifacts` (`reality-loop.rs:15099`) appends to `<run_root>/reality-optimizer/{attempts.jsonl,reward-series.jsonl,decisions.jsonl}` and updates `champion-state.json` and `experiment-registry.json`.
15. `write_training_export` (`reality-loop.rs:15371`) appends to `<run_root>/training/{positive,negative,preference_pairs,rejected_patches}.jsonl` per the verified-episode export rules.
16. The `attempt_completed` event is appended to `reality-events.jsonl` with the post-attempt GPU state.
17. If `solved == true`, the loop emits `rust_reality_loop_completed` and returns the summary; otherwise it advances `attempt += 1`, sleeps `--sleep-between-attempts-ms`, and retries.

### 3.5 Official Docker SWE-bench evaluation (`run_official_eval`)

| Step | Source | Failure code |
| --- | --- | --- |
| `--swebench-python` exists | line 10368 | `RUST_REALITY_SWEBENCH_PYTHON_MISSING` |
| Docker SDK preflight: `python -c "import docker; v=docker.from_env().version(); print(json.dumps(...))"` | `verify_official_docker_sdk` line 10505 | `RUST_REALITY_OFFICIAL_DOCKER_UNAVAILABLE` / `RUST_REALITY_OFFICIAL_DOCKER_PREFLIGHT_JSON_INVALID` |
| Write `official-prediction.jsonl` (one row: `instance_id`, `model_name_or_path`, `model_patch`) | line 10374 | byte-equal readback |
| Run `python -m swebench.harness.run_evaluation --dataset_name princeton-nlp/SWE-bench_Lite --split test --instance_ids <id> --predictions_path <path> --max_workers <n> --timeout <s> --cache_level env --run_id <id> --report_dir <dir>` with `DOCKER_HOST` and `DOCKER_CONFIG` env | line 10399 | `RUST_REALITY_OFFICIAL_EVAL_FAILED` (exit ≠ 0) |
| Best-effort `docker rm -f` of any container whose name contains the eval `run_id` | `cleanup_swebench_containers_for_run` line 10477 | silent on failure |
| Find the JSON report whose filename contains `run_id` and that has a `resolved_ids` array | `find_json_report` line 10664 | `RUST_REALITY_OFFICIAL_REPORT_MISSING` |
| Find per-instance subreport | `find_instance_report` line 10695 | `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` |
| `build_official_evidence` writes `official-success-evidence.json` (resolved) or `official-failure-evidence.json` (not resolved) | line 10578 | byte-equal readback |

Success is **only** declared when `evidence.resolved == true`, which itself is true only when the official report's `resolved_ids` array contains the task's `instance_id` (`reality-loop.rs:1991`). Local patch application, local pytest, model self-confidence, and reward score are all **insufficient** to claim resolution.

---

## 4. Governed `harness` subcommands

The `harness` subcommand surface (`HarnessCmd`, `reality-loop.rs:117`) is designed to be invoked **as a fresh subprocess per call** by the cgreality MCP server. This is the "MCP→CLI shim". Tool-level errors emit a JSON envelope on stdout and exit `1`; clap parse errors exit `2`; infrastructure crashes (panics, OOM) exit `64+`.

| Subcommand | Args | Stdin | Returns | Error codes |
| --- | --- | --- | --- | --- |
| `open-window` | `--path`, `--start-line`, `--end-line` | none | window text + SHA-256 | `CCREALITY_GOVERNED_PROJECT_PATH_INVALID`, `RUST_REALITY_HARNESS_*` |
| `apply-line-window-edit` | `--tool-dir`, `--timeout-secs` (default 120) | JSON object: `{path, start_line, end_line, observed_sha256, replace}` | governed edit result with diff path and new SHA | `RUST_REALITY_HARNESS_STDIN_*`, governed-edit error codes |
| `run-command` | `--tool-dir`, `--timeout-secs` (default 120) | JSON object: `{command}` | command output artifact pointer | `RUST_REALITY_HARNESS_RUN_COMMAND_FIELD_MISSING`, governed-edit error codes |
| `verify-state` | `--tool-dir`, `--scope=reality_loop_only|full` (default `reality_loop_only`), `--label=<text>` | none | verification artifact (cargo fmt + cargo check + targeted test outputs) | `RUST_REALITY_HARNESS_VERIFY_SCOPE_INVALID`, governed-edit error codes |

The implementation calls into `context_graph_cli::governed_edit::*` (`harness_open_window` at `reality-loop.rs:566`, `harness_apply_line_window_edit` at `577`, `harness_run_command` at `591`, `harness_verify_state` at `617`). Stdin payloads are used for `apply-line-window-edit` and `run-command` to avoid argv-quoting hazards on multi-line replacement text and command strings.

The `verify-state` `reality_loop_only` scope runs cargo fmt / cargo check / a targeted `cargo test` scoped to the `context-graph-cli` crate; `full` runs the wider verification. Both write a `verification-*.json` artifact under `--tool-dir`.

---

## 5. Per-attempt artifact layout

All paths are relative to `<runtime_root>/<run_id>/` (where `<run_id>` = `rust_reality_<unix_ns>_<short_hash>`). `<task_dir>` = `tasks/<instance_id>/<safe_id(model)>/`.

| Path | Purpose | Writer |
| --- | --- | --- |
| `measurement-contract.json` | Pinned claim scope (§3.3) | `build_measurement_contract` |
| `task-manifest-readback.json` | Verbatim task manifest copy | `run` line 1649 |
| `problem-reality-packet.json` | Durable problem reality packet (`build_problem_reality_packet`, line 2423) | per-run |
| `inner-model-readback.json` | GGUF capability readback | `model_capability_report` |
| `reality-events.jsonl` | Append-only event log with byte-equal readback verification | `append_event` line 15610 |
| `reality-optimizer/experiment-registry.json` | Optimizer experiment list (Phase 15) | `write_optimizer_artifacts` |
| `reality-optimizer/attempts.jsonl` | Per-attempt compact feed | `write_optimizer_artifacts` |
| `reality-optimizer/reward-series.jsonl` | Scalar reward trace | `write_optimizer_artifacts` |
| `reality-optimizer/decisions.jsonl` | Engine auto keep/discard/repair/escalate | `autoresearch_decision` line 15197 |
| `reality-optimizer/champion-state.json` | Best `(model, task)` result | `update_champion_state` line 15280 |
| `training/{positive,negative,preference_pairs,rejected_patches}.jsonl` | Verified-episode training export | `write_training_export` line 15371 |
| `<task_dir>/blackbox.sqlite` | Per-task BlackBoxFix ledger (`LEDGER_SCHEMA_VERSION = 14`) | `target/debug/context-graph blackbox init` |
| `<task_dir>/attempts/attempt-NNNNNN/problem-reality-packet.json` | Compact per-attempt problem-reality view | per-attempt |
| `<task_dir>/attempts/attempt-NNNNNN/repository-context.json` | Per-attempt repository-context snapshot | `collect_repository_context` |
| `<task_dir>/attempts/attempt-NNNNNN/prompt.txt` | Compiled prompt | `compile_prompt` |
| `<task_dir>/attempts/attempt-NNNNNN/model-decision-input.json` | Pre-call decision record | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/model-response.txt` | Raw inner-actor response text | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/model-parse-report.json` | Parse report (patch / structured edits / failures) | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/candidate.patch` | Final candidate patch | only when produced |
| `<task_dir>/attempts/attempt-NNNNNN/actual-applied.patch` | `git diff --binary` after applying candidate | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/static-checks.json` | Python `py_compile` results | `run_static_patch_checks` line 10202 |
| `<task_dir>/attempts/attempt-NNNNNN/validation.json` | Local-validation row (`passed`/`failed`/`blocked` + error code) | `validate_attempt` |
| `<task_dir>/attempts/attempt-NNNNNN/blackbox-verify-spec.json` | Verifier spec passed to the blackbox CLI | `verify_attempt` |
| `<task_dir>/attempts/attempt-NNNNNN/inner-harness-response.json` | Wrapper around the chat loop response | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/agent-tool-transcript.json` | Per-round tool-call transcript (request, response, results, transitions) | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/agent-tools/round-NN-MM-<tool>/` | Per-tool-call dir: `requested-tool-call.json`, `tool-result.json`, `harness-transition.json` | per tool call |
| `<task_dir>/attempts/attempt-NNNNNN/agent-tools/tool-round-NN-request.json` | Full chat request body for round NN | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/agent-tools/tool-round-NN-response.json` | Full chat response for round NN | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/agent-tools/tool-round-NN-tool-grammar.gbnf` | Rendered llama.cpp tool-call grammar | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/harness-transitions/round-NN.json` | Aggregated per-round harness transition | per-round |
| `<task_dir>/attempts/attempt-NNNNNN/dynamicjepa-reality.json` | DJ reality block for the attempt | `build_dynamicjepa_reality_for_attempt` |
| `<task_dir>/attempts/attempt-NNNNNN/official-prediction.jsonl` | One-row predictions input to the SWE-bench harness | `run_official_eval` |
| `<task_dir>/attempts/attempt-NNNNNN/official-eval-command.json` | Full Docker evaluator process record | per-attempt |
| `<task_dir>/attempts/attempt-NNNNNN/official-docker-preflight.json` | Docker SDK preflight | `verify_official_docker_sdk` |
| `<task_dir>/attempts/attempt-NNNNNN/official-docker-env.json` | DOCKER_HOST/DOCKER_CONFIG manifest | `official_docker_env` |
| `<task_dir>/attempts/attempt-NNNNNN/official-success-evidence.json` **or** `official-failure-evidence.json` | Authoritative oracle outcome | `build_official_evidence` |
| `<task_dir>/attempts/attempt-NNNNNN/reality-signal-packet.json` | Compact "what happened" record consumed by the optimizer | `build_reality_signal_packet` |
| `<task_dir>/attempts/attempt-NNNNNN/attempt-summary.json` | Aggregate of every artifact above | per-attempt |
| `<task_dir>/attempts/attempt-NNNNNN/trigger-decision.json` | Optimizer trigger decision (mirrored from `claude-code-optimizer/decision-turn-NN.json` by the MCP) | optimizer-driven |
| `<task_dir>/attempts/attempt-NNNNNN/claude-code-optimizer/recommendation-turn-NN.json` | Per-turn recommendation written via `optimizer_record_recommendation` | optimizer-driven |
| `<task_dir>/attempts/attempt-NNNNNN/claude-code-optimizer/decision-turn-NN.json` | Per-turn trigger decision via `optimizer_record_decision` | optimizer-driven |
| `<task_dir>/attempts/attempt-NNNNNN/claude-code-optimizer/edits/...` | Governed edits attributed to this attempt | `harness_apply_line_window_edit` |
| `<task_dir>/attempts/attempt-NNNNNN/claude-code-optimizer/commands/<unix>-<command>.json` | Allowlisted command outputs | `harness_run_command` |
| `<run_root>/cgreality-shift-log/<safe_session_id>.jsonl` | Per-session shift log (every successful tool call/edit/decision/recommendation) | `append_shift` (`shift_log.rs:71`) |
| `<run_root>/.gc-last-report.json` | Last GC pass report | `gc_runtime_root` |
| `<run_root>/_engine_logs/attempt-<unix_ns>-<pid>.{stdout,stderr}.log` | Detached engine log files (when MCP spawned the run) | MCP `reality_run_attempt` |

Active-run discovery: the engine writes `~/.cache/cgreality/active_runtime_root` and `~/.cache/cgreality/active_target_instance` so the cgreality MCP `reality_latest_root` can find the current run without environment variables.

---

## 6. Inner agentic tool surface (engine-side)

These are the **inner-actor** tools (Qwen3-Coder calls these inside the chat loop). Definitions in `agent_tool_specs` (`reality-loop.rs:5177`).

| Tool | Purpose | Required args |
| --- | --- | --- |
| `read_problem_reality` | Read the durable problem-reality packet (oracle tests, base commit, issue terms, implicated files, root-cause discovery checklist) | `detail` ∈ {`compact`, `full`} |
| `read_official_failure_reality` | Read the latest official SWE-bench failure evidence | optional `attempt` |
| `list_files` | List repository files by substring filter | optional `query`, `max_results` ≤ 200 |
| `inspect_symbol` | Find real source locations for a symbol; returns compact file windows + SHAs | `symbol` |
| `search_repo` | Bounded literal `git grep` over the real checkout | `query`, optional `max_results` ≤ 80 |
| `open_file_window` | Read a file slice by line range with SHA-256 readback | `path`, optional `start_line`, `end_line` |
| `apply_structured_edit` | Submit exact find/replace edits as the candidate patch | `edits[]` of `{path, find, replace}` |
| `apply_line_window_edit` | **Preferred** final patch submission tool. Replace line ranges. SHA-guarded against the file hash from `open_file_window`. | `edits[]` of `{path, start_line, end_line, replace, observed_sha256}` |
| `apply_unified_diff` | Submit a unified diff as the final candidate patch with observed source hashes | `patch`, `observed_files[]` of `{path, observed_sha256}` |
| `run_targeted_test` | Run one bounded targeted pytest in the real checkout | `test_id` |
| `run_official_swe_eval` | Read or explain official SWE-bench evaluation state for an attempt | optional `attempt` |
| `compare_attempts` | Compare prior attempts (patches, failure evidence, repeated-patch signals) | optional `limit` ≤ 20 |
| `retrieve_similar_verified_episodes` | Retrieve real prior official-evidence artifacts for the same repo or instance | optional `query`, `max_results` ≤ 20 |
| `synthesize_helper_tool` | Run one of three vetted deterministic analyzers | `analyzer` ∈ {`test_failure_digest`, `diff_risk_scan`, `repository_file_inventory`} |

Round bookkeeping enforced by `inner_coding_harness_chat_json` (`reality-loop.rs:4307`):

| Guard | Action | Source |
| --- | --- | --- |
| `round > MAX_ROUNDS_PER_ATTEMPT_HARD_CAP` | Force-stop with `RUST_REALITY_MAX_ROUNDS_EXCEEDED` | line 4394 |
| Per-attempt wallclock ≥ `CGREALITY_PER_ATTEMPT_DEADLINE_SECS` | Force-stop with `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED` | line 4418 |
| 5 consecutive identical non-success error codes | Force-stop with `RUST_REALITY_REPEATED_ERROR_DEADLOCK` | line 4746 |
| No tool calls returned this round + a recent `open_file_window` window exists | Increment `no_tool_after_window_count`; reprompt | line 4533 |
| Duplicate navigation tool call (same name + same arg-SHA) | Increment `repeated_navigation_count` | line 4578 |
| Edit deadline active (`edit_deadline_active`, line 5018) | Restrict tool surface to edit-only via `agent_edit_tool_specs` (line 4982) | line 4448 |
| Tool-result text > 4 KB | Truncate the **history copy** with a recovery marker; full result still persists to disk | `truncate_tool_result_for_history` line 2739 |

Final-edit tools (`is_final_edit_tool`, line 4975): `apply_structured_edit`, `apply_line_window_edit`, `apply_unified_diff`. A successful call materialises `<attempt_dir>/candidate.patch` and the loop exits.

---

## 7. cgreality MCP tool family (outer surface)

Tool definitions in `crates/context-graph-mcp/src/tools/definitions/reality.rs`. Tool names are exposed to Claude Code as `mcp__cgreality__<name>`. Handlers live under `crates/context-graph-mcp/src/handlers/tools/reality_loop/`.

### 7.1 Read-only analyze tools (`analyze.rs`)

| Tool | Schema | Behaviour |
| --- | --- | --- |
| `reality_latest_root` | `{}` | Read `~/.cache/cgreality/active_runtime_root` + `active_target_instance`; return latest `run_id` and last attempt number |
| `reality_attempt_summary` | `{run_id, attempt, compact?}` | Read `attempt-summary.json` (optionally compact) |
| `reality_official_report` | `{run_id, attempt}` | Read `official-{success,failure}-evidence.json` |
| `reality_problem_packet` | `{run_id, compact?}` | Read `<run_root>/problem-reality-packet.json` |
| `reality_signal` | `{run_id, attempt, compact?}` | Read `reality-signal-packet.json` |
| `dynamicjepa_reality_for_attempt` | `{run_id, attempt}` | Read `dynamicjepa-reality.json` |
| `reality_failure` | `{run_id, attempt}` | Read compact failure evidence |
| `reality_trigger_decision` | `{run_id, attempt}` | Read `trigger-decision.json` |
| `reality_harness_transitions` | `{run_id, attempt, round?}` | List or read harness-transition artifacts |
| `reality_compare_attempts` | `{run_id, attempt_a, attempt_b}` | Diff two attempt summaries |
| `reality_audit_trail` | `{entity_id, limit?}` | Read ContextGraph audit trail |
| `reality_replay_artifact` | `{path}` | Read any file artefact and return its SHA-256 |

### 7.2 Engine-spawning tools (`interact.rs`)

| Tool | Schema (required) | Behaviour |
| --- | --- | --- |
| `reality_run_attempt` | `{task_manifest, runtime_root?, max_attempts?, official_eval?, model?, inner_model_path?, inner_context_tokens?, inner_max_new_tokens?}` | Spawn `reality-loop attempt` as a detached `setsid` subprocess; poll runtime root for the new run dir for 30 s; return as soon as the engine has demonstrably created its run dir. Engine continues running independently after the MCP returns. |
| `reality_run_smoke` | `{runtime_root?, label?, session_id?}` | Spawn `reality-loop smoke` |
| `reality_run_official_eval` | `{run_id, attempt}` | Spawn `reality-loop official-eval` against an existing attempt's patch |
| `reality_run_targeted_test` | `{run_id, attempt, test_id, timeout_secs?}` | Spawn `reality-loop targeted-test` |
| `reality_recompile_reward` | `{run_id, attempt}` | Re-run `blackbox reward compile` and replay the signal packet |
| `reality_query_ledger` | `{run_id, where, limit?}` | Constrained read-only BlackBox ledger query |

`reality_run_attempt` failure modes:
| Code | Trigger |
| --- | --- |
| `CCREALITY_INTERACT_ENGINE_EXITED_DURING_STARTUP` | Engine exited before creating its run dir; stderr/stdout tails included |
| `CCREALITY_INTERACT_ENGINE_NO_RUN_DIR` | 30 s elapsed without a new run dir under `<runtime_root>/` |
| `CCREALITY_INTERACT_TRY_WAIT_FAILED` | Host process state query failed |

### 7.3 Governed harness write tools (`alter.rs`)

These shell out to `target/release/reality-loop harness <subcommand>` via `spawn_harness_subcommand` (`alter.rs:420`). Each shells out a fresh subprocess; the binary is re-resolved via `locate_engine_binary` (`interact.rs:533`) on every call. `EditLock` (`lock.rs`) provides session-scoped exclusion — the lock file is at `~/.cache/cgreality/edit_lock`, holds session id and acquisition time, has a 1-hour staleness window, and is removed on `Drop`.

| Tool | Schema | CLI subcommand invoked | Returns |
| --- | --- | --- | --- |
| `harness_open_window` | `{path, start_line, end_line}` | `harness open-window` | window content + SHA-256 |
| `harness_apply_line_window_edit` | `{path, start_line, end_line, observed_sha256, replace, session_id?, tool_use_id?}` | `harness apply-line-window-edit` (stdin: edit object) | governed edit result + harness-transition path + shift id |
| `harness_run_command` | `{command, timeout_secs?, session_id?}` | `harness run-command` (stdin: `{command}`) | command output artifact pointer |
| `harness_git_diff` | `{path?, base?}` | `git diff` invocation | structured diff JSON |
| `harness_git_status` | `{}` | `git status --porcelain` | structured JSON |
| `harness_verify_state` | `{scope?}` | `harness verify-state --scope <reality_loop_only|full>` | rustfmt + cargo check + targeted test results |

The path-policy gate is **REMOVED** post 2026-05-07; only empty / NUL / newline paths are rejected (`path_policy::ensure_path_allowed`, `path_policy.rs:268`). The deny-list is BEHAVIORAL (see CLAUDE.md §8) — the AI is trusted to honour it.

### 7.4 Optimizer tools (`optimizer.rs`)

| Tool | Schema (required) | Behaviour |
| --- | --- | --- |
| `optimizer_record_decision` | `{run_id, attempt, policy, should_run, claude_session_id, claude_model}` | Write `claude-code-optimizer/decision-turn-NN.json` and mirror to `trigger-decision.json`; append shift |
| `optimizer_record_recommendation` | (validator-enforced shape, fields validated against `RecommendationValidator`) | Write `claude-code-optimizer/recommendation-turn-NN.json`; append optimizer ledger row; append shift. Stop-hook-enforced. |
| `optimizer_record_harness_transition` | `{run_id, attempt, file_path, before_sha256, after_sha256, lines_added, lines_removed, ...}` | Write a harness-transition artifact directly (used when an external editor has performed an edit) |

`coerce_stringified_args` (Phase 13 T13.2) tolerates JSON-RPC clients that collapse typed values (integers/booleans/arrays) into strings.

### 7.5 Reality-shift detection (`shift_log.rs`)

| Tool | Schema | Behaviour |
| --- | --- | --- |
| `reality_shift_log` | `{session_id, limit?, since_shift_id?}` | Read per-session shift log entries from `<runtime_root>/cgreality-shift-log/<safe_session_id>.jsonl` |
| `reality_shift_compare_to_my_view` | `{session_id, files[]}` | For each path, compute current SHA on disk and compare against the latest shift-log `after.sha256`. Returns `external_drift_detected: true` whenever a path's current SHA differs from the session's last known view |

A `ShiftRecord` (`shift_log.rs:11`) carries `shift_id` (ULID-shaped), `timestamp_unix_ns`, `tool_name`, `tool_use_id`, `session_id`, `subject`, `before`, `after`, `delta_summary`, `verification`, `harness_transition_path`.

### 7.6 Phase 15 autoresearch tools (`autoresearch.rs`)

State lives under `<run_root>/reality-optimizer/`. Engine writes `attempts.jsonl`, `reward-series.jsonl`, `decisions.jsonl`, `champion-state.json` automatically; the optimizer (Claude Code) populates `experiment-registry.json` via the propose/update tools.

| Tool | Schema | Behaviour |
| --- | --- | --- |
| `experiment_registry_list` | `{status_filter?, limit?}` | List experiments; filter by status ∈ {pending, kept, discarded, escalated, promoted} |
| `experiment_registry_get` | `{experiment_id}` | Read one experiment record |
| `experiment_registry_propose` | `{harness_change_summary, claude_session_id, ...}` | Append a candidate experiment |
| `experiment_registry_update_outcome` | `{experiment_id, outcome, reasoning, claude_session_id}` | Set outcome to one of pending/kept/discarded/escalated/promoted |
| `champion_state_get` | `{model?, task?}` | Read champion-state.json (best `(model, task)` row) |
| `champion_state_promote` | `{model, task, justification, claude_session_id}` | Promote a candidate champion. Requires `official_resolved == true`. |
| `attempts_history_query` | `{model?, instance_id?, limit?}` | Read recent rows from `attempts.jsonl` |

The engine's auto keep/discard logic (`autoresearch_decision`, `reality-loop.rs:15197`) decides per attempt:

| Condition | Decision | Reasoning |
| --- | --- | --- |
| `record.resolved == Some(true)` | `keep` | Promotion candidate per Phase 19 |
| `record.repeated_patch == true` | `discard` | Failure does not produce new evidence |
| `error_code.is_some() && patch_sha256.is_none()` | `escalate` | Infrastructure or contract failure |
| DJ intervention surface ∈ {`inner_model`, `dynamicjepa_artifact_training`} | `escalate` | Beyond optimizer scope |
| DJ intervention surface ∈ {`tool_surface`, `tool_guard_surface`, `prompt_template`} | `repair` | Optimizer-actionable |
| `cur_reward > prev_reward + 0.05` | `repair` | Improving trajectory |
| `cur_reward < prev_reward - 0.05` | `discard` | Regressed |
| Otherwise | `repair` | Flat or first attempt |

---

## 8. Reward spec

`config/reward_spec.verified_outcome.json` is required (`RUST_REALITY_REWARD_SPEC_MISSING`); the engine forwards it to `target/debug/context-graph blackbox reward compile` after every attempt.

| Top-level field | Value |
| --- | --- |
| `reward_spec_schema_version` | `1` |
| `reward_id` | `contextgraph_verified_outcome_demo_v1` |
| `objective` | `"maximize verified source-of-truth outcomes while penalizing incidents, risk, and missing governance"` |
| `bounds.min` | `-10.0` |
| `bounds.max` | `10.0` |

| Component | Feature | Weight |
| --- | --- | --- |
| `correctness` | `correctness` | `+4.0` |
| `verification_confidence` | `verification_confidence` | `+2.0` |
| `risk_penalty` | `risk` | `-2.0` |
| `incident_penalty` | `incident_present` | `-3.0` |
| `policy_missing_penalty` | `policy_missing` | `-1.0` |
| `training_value` | `training_value` | `+1.0` |
| `dj_surprise_rate_penalty` | `dynamicjepa_trained_surprise_rate` | `-0.5` |
| `dj_prediction_accuracy_bonus` | `dynamicjepa_prediction_accuracy` | `+1.0` |
| `dj_plan_trace_bonus` | `dynamicjepa_trained_plan_trace_coverage` | `+0.5` |

Cost-cap machinery is **not** present. There is no `cost_cap_reached` blocked status, no `cost_estimate_usd` field, and no per-turn/per-day budget enforcement anywhere in the engine.

---

## 9. DynamicJEPA integration

The reality engine ingests structured tool transitions into the DynamicJEPA store and (when an artifact is paired) runs trained inference per attempt. Cross-link: see [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md) for the latent-state subsystem mechanics.

| Item | Value | Source |
| --- | --- | --- |
| Domain pack id | `contextgraph_swe_reality_loop_v1` | `DYNAMICJEPA_REALITY_DOMAIN_PACK` |
| Domain pack file | `configs/dynamicjepa/domain_packs/contextgraph_swe_reality_loop.v1.toml` | 1,194 lines |
| Default skill id | `d91e5e0c-3557-57a9-8b48-0fa2acb429a0` | `DEFAULT_DYNAMICJEPA_SKILL_ID` |
| State fields | `counter`, `attempt`, `round`, `tool_family`, `repo_dirty_state`, `prompt_version`, `model_id`, `primary_path_bucket`, `symbol_bucket`, `dependency_layer`, `test_target_kind`, `error_signature`, `ownership_bucket` | TOML §`[[state.fields]]` |
| Action fields | `delta`, `tool_family`, `command_kind`, `edit_size_bucket`, `path_risk`, `target_path_bucket`, `symbol_bucket`, `dependency_layer`, `test_target_kind`, `ownership_bucket`, `planned_patch_bytes`, `planned_changed_file_count`, `patch_intent_bucket`, `patch_shape_bucket`, `patch_scope_bucket`, `ast_language`, `ast_parse_status`, `ast_node_kind`, `ast_symbol_kind`, `ast_edit_kind`, `ast_touched_node_count`, `ast_symbol_count` | TOML §`[[action.fields]]` |
| Outcome fields | `next_counter`, `outcome_label`, `result_status`, `verifier_status`, `test_failure_class`, `incident_code`, `changed_file_count`, `patch_bytes`, `correction_required`, `file_risk`, `error_signature`, `symbol_bucket`, `dependency_layer`, `test_target_kind`, `ownership_bucket`, `transition_id` | TOML §`[[outcome.fields]]` |
| Adapter | `json_event` mapping (TOML §`[adapters.mapping]`) maps engine event JSON into the panel | TOML lines 1034–1093 |
| Invariant | `outcome.next_counter == state.counter + action.delta` (severity `fatal`) | TOML §`[[invariants]]` |
| Constellation | `surprise_threshold = 0.85`, `require_loo_min = 0.70`, `calibration_percentile = 10`, `calibration_set_size = 12` | TOML §`[constellation.global]` |

Per-attempt the engine writes `dynamicjepa-reality.json` (`build_dynamicjepa_reality_for_attempt`, line 11774) with both projection-only and trained-inference paths. When `--dynamicjepa-db <path>` and `--dynamicjepa-artifact-id <id>` are paired, the engine validates the artifact lifecycle, segmented surprise calibration, attempt-segment drift, corpus diversity, and comparative promotion against the verifier policy in the TOML. `live_ingest_dynamicjepa_transition` and `update_dynamicjepa_live_index` (lines 6084 and 6260) keep an attempt-local live index of structured transitions.

---

## 10. Smoke and verify subcommands

### 10.1 `smoke`

`run_smoke` (`reality-loop.rs:776`) writes three deterministic artifacts under `<smoke_runtime_root>/<run_id>/<task_id>/smoke-attempt-1/`:

- `problem-reality-packet.json` (`record_kind: ccreality_smoke_problem_reality`)
- `reality-signal-packet.json` (`record_kind: ccreality_smoke_reality_signal`)
- `attempt-summary.json` (`record_kind: ccreality_smoke_attempt_summary`)

Updates `~/.cache/cgreality/active_runtime_root` so cgreality MCP tools see the smoke run. `--force-fail` raises `RUST_REALITY_SMOKE_FORCED_FAIL` to exercise the failure-artifact writer.

### 10.2 `verify`

`run_verify` (`reality-loop.rs:1222`) walks the runtime root, SHA-256s every JSON artifact, and (when a sibling `*.json.sha256` sidecar exists) compares to detect drift. Supports `--attempt N` to scope to a single attempt directory (matches `attempt-N`, `attempt-NNNNNN`, or `smoke-attempt-N`). Returns `status: ok` or `drift_detected` with the offending paths.

---

## 11. Failure model

Every step is fail-closed. `RealityError` (`reality-loop.rs:353`) carries `status="error"`, `error_code`, `message`, `field_path`, `remediation`, `details`, `source_of_truth`. On error the dispatch wrapper writes `<runtime_root>/failures/failure-<unix>-<error_code>.json` (`write_failure_artifact_for_command`, line 3034) and prints the error JSON to stderr; the binary exits `1`.

Selected failure codes already covered above. Below is the longer cross-reference of codes the engine produces during an attempt:

| Code | Phase | Source |
| --- | --- | --- |
| `RUST_REALITY_CLI_PARSE_FAILED` | clap parse | line 511 |
| `RUST_REALITY_HOME_DIR_MISSING` | active-runtime-root cache | line 16207 |
| `RUST_REALITY_DYNAMICJEPA_*` | DJ pairing/lifecycle | lines 1483, 12548, 12941, 13003, etc. |
| `RUST_REALITY_INNER_MODEL_*` | GGUF load + pin enforcement | lines 3399–3492 |
| `RUST_REALITY_INNER_CONTEXT_TOKENS_INVALID`, `..._MAX_NEW_TOKENS_INVALID`, `..._BATCH_SIZE_*`, `..._TEMPERATURE_INVALID`, `..._TOP_P_INVALID` | inner-harness arg sanity | lines 1422–1481 |
| `RUST_REALITY_INNER_BACKEND_INIT_FAILED`, `..._MODEL_LOAD_FAILED`, `..._CHAT_TEMPLATE_*`, `..._MESSAGES_MISSING`, `..._TOOLS_MISSING/EMPTY`, `..._GRAMMAR_MISSING` | chat invocation | lines 3514–3851, 3808 |
| `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` | GPU preflight + per-call | line 15909 |
| `RUST_REALITY_PROBLEM_BASE_COMMIT_MISMATCH`, `..._WORKSPACE_DIRTY`, `..._GIT_LS_FILES_FAILED` | problem-reality capture | lines 2437–2470 |
| `RUST_REALITY_MAX_ROUNDS_EXCEEDED` | round loop hard cap | line 4394 |
| `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED` | wallclock budget | line 4418 |
| `RUST_REALITY_REPEATED_ERROR_DEADLOCK` | 5 identical consecutive errors | line 4748 |
| `RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED` | round loop ended without a final edit | post-loop |
| `RUST_REALITY_PATCH_REPEATED` | patch SHA matches a prior attempt | line 10101 |
| `RUST_REALITY_PATCH_APPLY_FAILED` | `git apply` failed | line 10122 |
| `RUST_REALITY_PATCH_MODIFIES_TESTS` | patch touches a forbidden path (`is_forbidden_benchmark_mutation`, line 10271) | line 10155 |
| `RUST_REALITY_PATCH_PY_COMPILE_FAILED` | static check on changed Python files | line 10244 |
| `RUST_REALITY_OFFICIAL_DOCKER_UNAVAILABLE`, `RUST_REALITY_OFFICIAL_DOCKER_PREFLIGHT_JSON_INVALID` | Docker SDK preflight | lines 10522, 10532 |
| `RUST_REALITY_OFFICIAL_EVAL_FAILED` | swebench harness exit ≠ 0 | line 10441 |
| `RUST_REALITY_OFFICIAL_REPORT_MISSING`, `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` | report or per-instance subreport not found | lines 10664, 10695 |
| `RUST_REALITY_REWARD_SPEC_MISSING` | `config/reward_spec.verified_outcome.json` absent | line 2052 |
| `RUST_REALITY_BLACKBOX_VERIFY_NON_JSON` | blackbox verify produced non-JSON | line 10354 |

`is_forbidden_benchmark_mutation` (`reality-loop.rs:10271`) classifies a path as forbidden when it starts with `test_`, `tests/`, contains `/test_`, `/tests/`, ends in `_test.py`, contains `conftest.py`, `tox.ini`, or `pytest`.

---

## 12. The `blackbox` ledger CLI (called by the engine)

`crates/context-graph-cli/src/commands/blackbox.rs` (35,414 lines) implements the blackbox subcommand surface. The engine forks this binary for every ledger operation. Every per-task ledger lives at `<task_dir>/blackbox.sqlite` with `LEDGER_SCHEMA_VERSION = 14` (line 40).

Subcommand families used by the engine:

| Family | Used during | Example invocation |
| --- | --- | --- |
| `blackbox init --ledger <path>` | run start | `reality-loop.rs:1652` |
| `blackbox record run` | run start | `reality-loop.rs:1656` |
| `blackbox record model-decision` | per-attempt pre-call | `reality-loop.rs:1781` |
| `blackbox record tool-call` | per-attempt post-validation | `reality-loop.rs:1929` |
| `blackbox verify --input <spec>` | per-attempt | `verify_attempt`, `reality-loop.rs:10334` |
| `blackbox record dynamicjepa-reality` | per-attempt | `reality-loop.rs:2016` |
| `blackbox score run` | per-attempt | `reality-loop.rs:2036` |
| `blackbox reward compile --input config/reward_spec.verified_outcome.json` | per-attempt | `reality-loop.rs:2062` |

See [10_cli_reference.md](10_cli_reference.md) for the full blackbox CLI surface and [15_verification_report.md](15_verification_report.md) §6 for the verifier framework + redaction policy.

---

## 13. Operator workflow

Operator entry points for the reality-loop subsystem are the four ccreality skills (`./.claude/skills/ccreality-*` and the loop spec in `.claude/loop.md`):

| Skill | Purpose |
| --- | --- |
| `/ccreality-iterate` | One full optimizer turn — diagnose latest attempt, edit harness if warranted, verify, record recommendation |
| `/ccreality-smoke` | Full pipeline smoke without Docker — verifies engine+MCP+hook+recommendation wiring |
| `/ccreality-trigger-check` | Read-only check on whether to iterate this turn |
| `/ccreality-readback` | Drift detection on past recommendations (compares recorded SHAs to current files) |

Direct CLI invocations (operator only when bypassing the optimizer):

```bash
# Build the engine
cargo build --release -p context-graph-cli

# One attempt against a SWE-bench Lite manifest
target/release/reality-loop attempt \
  --task-manifest tasks/swebench-lite/psf__requests-2317.json \
  --model qwen3-coder:30b-a3b \
  --official-eval-each-attempt

# Synthetic smoke
target/release/reality-loop smoke --label localdev

# Re-run official Docker eval against an existing attempt's patch
target/release/reality-loop official-eval \
  --run-id rust_reality_<ns>_<hash> \
  --target-instance psf__requests-2317 \
  --attempt 1

# Verify all JSON artifacts under a runtime root
target/release/reality-loop verify --runtime-root /tmp/contextgraph-rust-reality-loop
```

---

## 14. Hard invariants

1. Success is **only** declared when `official_evidence.resolved == true`, which itself requires the official report's `resolved_ids` to contain `task.instance_id`. Local validation, local pytest, and reward score are insufficient.
2. The engine refuses to enter the model loop without `--official-eval-each-attempt`. The loop will not run with measurement scope collapsed to local validation (`RUST_REALITY_OFFICIAL_EVAL_REQUIRED`).
3. Patch repetition is a hard failure (`RUST_REALITY_PATCH_REPEATED`).
4. The model may not modify test, fixture, or harness files (`RUST_REALITY_PATCH_MODIFIES_TESTS`). `is_forbidden_benchmark_mutation` enumerates the suffix/substring rules.
5. Every JSON write goes through `write_json_checked` (line 16029) and every text write through `write_text_checked` (line 16066) — both perform byte-equal readback after `fs::write` and fail with `RUST_REALITY_JSON_READBACK_MISMATCH` / `RUST_REALITY_TEXT_READBACK_MISMATCH` on drift. `append_event` (line 15610) round-trips each appended event (`RUST_REALITY_EVENT_READBACK_MISMATCH`).
6. The inner-actor GGUF is cryptographically pinned by size + streaming SHA-256 + identity substrings. There is no fallback to "unpinned" mode.
7. The MCP server holds a singleton flock at `~/.cache/cgreality/reality-loop-stdio.lock`; the harness write tools fork-exec the engine binary fresh every call.
8. `EditLock` enforces single-writer-per-session for harness edits (`~/.cache/cgreality/edit_lock`, 1 h staleness window).
9. Path policy is BEHAVIORAL post 2026-05-07. The MCP only rejects empty / NUL / newline path strings; the deny set lives in CLAUDE.md §8.
10. Cost caps and dollar budgets do not exist anywhere in the engine, MCP, or reward spec. There is no `cost_cap_reached` blocked status, no `cost_estimate_usd` schema field, no per-turn/per-day cap.
