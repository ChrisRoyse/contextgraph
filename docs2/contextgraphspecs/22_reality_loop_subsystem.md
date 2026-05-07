# Reality-Loop Subsystem

**Source files covered:**
- `crates/context-graph-cli/src/bin/reality-loop.rs` (2,385 lines) — Rust-owned reality loop binary
- `crates/context-graph-cli/src/commands/blackbox.rs` — BlackBoxFix ledger CLI used by the loop
- `crates/context-graph-cli/Cargo.toml` — `[[bin]] name = "context-graph"` (the binary the loop spawns for ledger ops)
- `gemmafrontend/server.py` (3,879 lines) — Python evidence/console server, agent-tool gateway
- `gemmafrontend/model_benchmark.py` (248 lines) — multi-model verified-outcome benchmark runner
- `gemmafrontend/model_prompt_matrix.py` (497 lines) — model × prompt grid with edge cases
- `gemmafrontend/manual_fsv.py` (382 lines) — manual end-to-end FSV harness for the gemmafrontend stack
- `config/reward_spec.verified_outcome.json` (41 lines) — verified-outcome reward spec
- `gemmafrontend/swe_smoke_benchmark_suite.json` (40 lines) — initial smoke curriculum (2 instances)
- `gemmafrontend/README.md` — operator-facing docs

**Cross-links:** [04_database_schema.md](04_database_schema.md), [05_embedding_subsystem.md](05_embedding_subsystem.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [10_cli_reference.md](10_cli_reference.md), [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md), [15_verification_report.md](15_verification_report.md). [01_system_overview.md](01_system_overview.md) section 10 lists this subsystem alongside DynamicJEPA.

## 1. Summary
The reality loop is the SWE-bench Lite repair pipeline that uses ContextGraph as durable memory and the official Docker SWE-bench evaluator as the **only** success oracle. It is now owned by one implementation:

| Variant | Process | Source-of-truth driver |
| --- | --- | --- |
| Rust reality loop | `cargo run --release --bin reality-loop` | `crates/context-graph-cli/src/bin/reality-loop.rs` |

The Rust loop:
1. Validate a SWE-bench task manifest against an immutable measurement contract.
2. Compile a deterministic prompt with verified failure evidence.
3. Drive a local Ollama model through the prompt under a GPU VRAM budget.
4. Apply the candidate patch to the real repository checked out at the exact base commit.
5. Validate fail-closed (no test/harness mutation, no patch-repeat).
6. Run the official Docker SWE-bench harness and read `resolved_ids` back from the JSON report.
7. Persist every step into a BlackBoxFix SQLite ledger and a JSONL reality event stream.

Success is **only** declared when the official report's `resolved_ids` array contains the task's `instance_id`. Local patch application, local pytest results, and model self-confidence are explicitly insufficient.

## 2. Architecture
```
+----------------------+      +--------------------+      +-------------------------+
| reality-loop.rs      |--->  | target/debug/      |--->  | blackbox.sqlite         |
| (Rust)               |      | context-graph      |      | (LEDGER_SCHEMA_VERSION=14)
|                      |      |  blackbox CLI      |      +-------------------------+
|                      |--->  | reality-events.jsonl (append-only with readback verify)
+----------+-----------+
           |
           v
   Ollama /api/chat (qwen3.6:27b, --keep-alive 30m)
           |
           v
   git apply --whitespace=nowarn  (--> repo workspace)
           |
           v
   python -m swebench.harness.run_evaluation (Docker Desktop / WSL2 socket)
           |
           v
   official report (resolved_ids[]) + per-instance run logs
           |
           v
   official-success-evidence.json | official-failure-evidence.json (+ reward + score rows)
```

Components and their concrete roles:

| Component | Role | Key files |
| --- | --- | --- |
| `reality-loop.rs` | Rust binary that owns the entire loop in one process | `crates/context-graph-cli/src/bin/reality-loop.rs` |
| `gemmafrontend/server.py` | Long-lived HTTP console / agent-tool gateway with prevalidated tool calls | `server.py` |
| `target/debug/context-graph` | BlackBoxFix ledger CLI used for `blackbox init`, `record`, `verify`, `score`, `reward compile` | `crates/context-graph-cli/src/main.rs` |
| ContextGraph DB | Memory + reality-event durable store | RocksDB CFs in [04_database_schema.md](04_database_schema.md) |

The Rust loop is the canonical and only SWE-bench refinement entry point. The old Python loop, Python official-eval bridge, and Python prompt compiler have been deleted to prevent split ownership.

## 3. Runtime Measurement Contract (commit cc8e6c33)
`build_measurement_contract` (reality-loop.rs:744) writes a JSON object to `<run_root>/measurement-contract.json` that pins the entire claim scope **before any model is called**.

| Field | Value | Source |
| --- | --- | --- |
| `schema` | `"contextgraph_reality_measurement_contract_v1"` | reality-loop.rs:750 |
| `claim_scope.primary_claim` | `"official_swe_instance_resolution_only"` | reality-loop.rs:757 |
| `claim_scope.success_predicate` | `"official_report.resolved_ids contains task.instance_id"` | reality-loop.rs:758 |
| `claim_scope.explicit_non_claims` | 3 explicit non-claims (no full repo proof, no human-intent proof, no success without official Docker) | reality-loop.rs:759-763 |
| `oracle.dataset` | `"princeton-nlp/SWE-bench_Lite"` | constant `DATASET_NAME` (reality-loop.rs:20) |
| `oracle.split` | `"test"` | reality-loop.rs:767 |
| `oracle.instance_id` | task.instance_id | reality-loop.rs:768 |
| `oracle.fail_to_pass` | full FAIL_TO_PASS list | reality-loop.rs:769 |
| `oracle.pass_to_pass_count` | integer count | reality-loop.rs:770 |
| `oracle.official_test_patch_sha256` | optional | reality-loop.rs:771 |
| `oracle.oracle_patch_sha256` | optional | reality-loop.rs:772 |
| `oracle.source_of_truth` | task.source_of_truth | reality-loop.rs:773 |
| `immutable_boundaries.task_manifest` | `file:<path>` | reality-loop.rs:776 |
| `immutable_boundaries.base_commit` | git SHA | reality-loop.rs:777 |
| `immutable_boundaries.official_evaluator` | `"python -m swebench.harness.run_evaluation"` | reality-loop.rs:779 |
| `immutable_boundaries.official_docker_host` | default `unix:///var/run/docker.sock` | reality-loop.rs:18 |
| `immutable_boundaries.official_docker_config` | default `/home/cabdru/.cache/contextgraph-docker-config` | reality-loop.rs:19 |
| `immutable_boundaries.reward_spec` | `file:config/reward_spec.verified_outcome.json` | reality-loop.rs:782 |
| `mandatory_readbacks` | 10 required readback IDs | reality-loop.rs:784-795 |
| `model_editable_scope` | `["candidate.patch", "notes"]` | reality-loop.rs:797 |
| `model_forbidden_scope` | 7 explicit names (manifest, harness, ledger, reward spec, verifier, tests, fixtures) | reality-loop.rs:800-808 |
| `failure_policy.unmeasured_success` | `"blocked"` | reality-loop.rs:810 |
| `failure_policy.missing_source_of_truth` | `"error"` | reality-loop.rs:811 |
| `failure_policy.official_evaluator_unavailable` | `"error"` | reality-loop.rs:813 |
| `failure_policy.test_or_harness_mutation` | `"failed"` | reality-loop.rs:814 |

The 10 mandatory readbacks (reality-loop.rs:784-795) are: `task_manifest_readback`, `measurement_contract_readback`, `git_head_equals_base_commit`, `git_status_clean_before_attempt`, `candidate_patch_applies_to_base`, `candidate_patch_does_not_modify_tests_or_harness`, `official_docker_sdk_preflight`, `official_swe_report_with_resolved_ids`, `blackbox_sqlite_ledger_counts`, `reality_event_log_append_readback`.

## 4. Reality-Boundary Fail-Closed Gates
Every step is fail-closed with a typed `RealityError` (reality-loop.rs:73-103) carrying `error_code`, `field_path`, `remediation`, `details`, `source_of_truth`. Failure is logged to `<runtime_root>/failures/failure-<ts>-<code>.json` with readback verification before exit (reality-loop.rs:837-883).

| Gate | Error code | Source |
| --- | --- | --- |
| `--max-attempts` ≥ 1 | `RUST_REALITY_MAX_ATTEMPTS_INVALID` | reality-loop.rs:197 |
| `--run-until-resolved` requires `--official-eval-each-attempt` | `RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL` | reality-loop.rs:206 |
| `context-graph` binary present | `RUST_REALITY_CONTEXT_GRAPH_BIN_MISSING` | reality-loop.rs:217 |
| Task manifest present | `RUST_REALITY_TASK_MANIFEST_MISSING` | reality-loop.rs:223 |
| `--official-eval-each-attempt` mandatory for SWE loops | `RUST_REALITY_OFFICIAL_EVAL_REQUIRED` | reality-loop.rs:231 |
| Task dataset == `princeton-nlp/SWE-bench_Lite` | `RUST_REALITY_TASK_DATASET_MISMATCH` | reality-loop.rs:654 |
| `instance_id` non-empty + contains `__` | `RUST_REALITY_TASK_INSTANCE_ID_INVALID` | reality-loop.rs:664 |
| `repo` is `owner/name` and contains no `..` | `RUST_REALITY_TASK_REPO_INVALID` | reality-loop.rs:674 |
| `base_commit` is 40-hex SHA | `RUST_REALITY_TASK_BASE_COMMIT_INVALID` | reality-loop.rs:684 |
| `problem_statement` non-empty | `RUST_REALITY_TASK_PROBLEM_STATEMENT_MISSING` | reality-loop.rs:694 |
| `fail_to_pass` non-empty | `RUST_REALITY_TASK_FAIL_TO_PASS_MISSING` | reality-loop.rs:704 |
| `pass_to_pass_count` > 0 | `RUST_REALITY_TASK_PASS_TO_PASS_MISSING` | reality-loop.rs:714 |
| `source_of_truth` binds dataset+instance | `RUST_REALITY_TASK_SOURCE_OF_TRUTH_INCOMPLETE` | reality-loop.rs:724 |
| `git rev-parse HEAD == base_commit` after reset | `RUST_REALITY_BASE_COMMIT_READBACK_MISMATCH` | reality-loop.rs:1190 |
| `git status --porcelain` empty after reset | `RUST_REALITY_WORKSPACE_NOT_CLEAN_AFTER_RESET` | reality-loop.rs:1212 |
| Patch parses as JSON `{patch, notes}` | `RUST_REALITY_MODEL_RESPONSE_JSON_INVALID` / `RUST_REALITY_MODEL_PATCH_MISSING` | reality-loop.rs:1100-1124 |
| Patch differs from previous attempts | `RUST_REALITY_PATCH_REPEATED` | reality-loop.rs:1250 |
| Patch applies via `git apply` | `RUST_REALITY_PATCH_APPLY_FAILED` | reality-loop.rs:1271 |
| Patch does not touch test/fixture/harness paths | `RUST_REALITY_PATCH_MODIFIES_TESTS` | reality-loop.rs:1303 |
| Docker SDK preflight (`docker.from_env().version()`) returns JSON | `RUST_REALITY_OFFICIAL_DOCKER_UNAVAILABLE` / `RUST_REALITY_OFFICIAL_DOCKER_PREFLIGHT_JSON_INVALID` | reality-loop.rs:1536, 1546 |
| Official harness exits 0 | `RUST_REALITY_OFFICIAL_EVAL_FAILED` | reality-loop.rs:1495 |
| Final report contains `resolved_ids` and instance is classified | `RUST_REALITY_OFFICIAL_REPORT_MISSING` / `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` | reality-loop.rs:1610, 1697 |
| Free GPU VRAM ≥ `--gpu-min-free-mb` (default 8192) | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` | reality-loop.rs:2076 |
| Used GPU VRAM ≤ `--gpu-max-used-mb` (when set) | `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` | reality-loop.rs:2087 |
| All JSON writes do byte-equal readback | `RUST_REALITY_JSON_READBACK_MISMATCH` | reality-loop.rs:2208 |
| All text writes do byte-equal readback | `RUST_REALITY_TEXT_READBACK_MISMATCH` | reality-loop.rs:2228 |
| Each appended event must round-trip | `RUST_REALITY_EVENT_READBACK_MISMATCH` | reality-loop.rs:1989 |

`is_forbidden_benchmark_mutation` (reality-loop.rs:1332) considers a path forbidden when it starts with `test_`, `tests/`, contains `/test_`, `/tests/`, ends in `_test.py`, contains `conftest.py`, `tox.ini`, or `pytest`.

## 5. Reality Branches & Governed Agent Tools
The Rust `reality-loop` owns the governed SWE agent-tool surface. Tools are validated in-process and persisted into attempt-scoped tool transcripts with readback verification.

| Tool name | Mode | Args | Source-of-truth |
| --- | --- | --- | --- |
| `list_files` | read_only | `path?`, `pattern?`, `max_results?` (1-200, default 60) | git workspace filesystem |
| `read_file` | read_only | `path` (required), `start_line?`, `end_line?` | file bytes + sha256 |
| `search_repo` | read_only | `query` (required), `max_results?` (1-40) | scanned files + sha256 |
| `inspect_repository_context` | read_only | none | pre-materialised `repository-context.json` |
| `read_failure_evidence` | read_only | `kind?` | persisted validation/official evidence files |

Every tool call is rejected with a typed `tool_result_blocked` row (line 1351) for: unknown tool (`SWE_AGENT_TOOL_UNKNOWN`), missing path (`SWE_AGENT_TOOL_PATH_REQUIRED`), absolute or `..` path (`SWE_AGENT_TOOL_PATH_OUTSIDE_REPO`), resolved escape (`SWE_AGENT_TOOL_PATH_ESCAPE`), missing list dir (`SWE_AGENT_TOOL_LIST_PATH_MISSING`), missing file (`SWE_AGENT_TOOL_FILE_MISSING`), empty query (`SWE_AGENT_TOOL_QUERY_REQUIRED`).

**Reality branch IDs** namespace each event so the loop is multi-layer auditable:
| Function | Pattern |
| --- | --- |
| `root_reality_branch_id(loop_id)` | `<loop_id>:root` |
| `task_reality_branch_id(loop_id, model, instance)` | `<loop_id>:<model>:<instance>:task-root` |
| `attempt_reality_branch_id(loop_id, model, instance, attempt)` | `<loop_id>:<model>:<instance>:attempt-NNN` |
| `prompt_candidate_branch_id(loop_id, model, instance, attempt)` | `<loop_id>:<model>:<instance>:prompt-candidate-NNN` |

Every reality event has `schema_version = 1` and includes `loop_id`, `loop_layer`, `event_type`, `status`, `model`, `instance_id`, `attempt`, `reality_branch_id`, `parent_reality_branch_id`, `promotion_status`, `payload`, and `payload_sha256`.

## 6. Deterministic Prompt Compiler (Rust)
Prompt compilation is now part of `crates/context-graph-cli/src/bin/reality-loop.rs`; the deleted Python compiler must not be used or recreated.

The Rust prompt contract supports three prompt kinds:

| `prompt_kind` | Template ID | Required inputs |
| --- | --- | --- |
| `initial` | `swe_repair_initial_v2` | task + repo_context (with `source_of_truth`) |
| `local_failure_refinement` | `swe_repair_local_failure_v2` | + `validation` row with `source_of_truth` |
| `official_failure_refinement` | `swe_repair_official_failure_v2` | + `official_evidence` with `source_of_truth.official_report` |

The compiled prompt always carries:
- `instance_id`, `repo`, `base_commit`
- `problem_statement` wrapped between `<<<PROBLEM_STATEMENT … PROBLEM_STATEMENT>>>` markers
- A `PREVIOUS_ATTEMPT_JSON` block (source attempt #, prior prompt SHA, prior output SHA, hashes)
- A `VERIFIED_EVIDENCE_JSON` block (compact failure summary)
- An `EVIDENCE_SOURCE_REFS_JSON` block listing every source-of-truth ref
- A `NO_REPEAT_PATCH_SHA256` list to forbid re-emitting prior patches
- A `REPOSITORY_CONTEXT` block built from durable on-disk file snippets

The compiler returns hard typed Rust errors when required source-of-truth inputs are missing, when prompt quality gates fail, or when hidden solution/test-patch leakage is detected.

| Quality check | Trigger |
| --- | --- |
| `failed_test_included` | every `failed_fail_to_pass` + `failed_pass_to_pass` test ID is mentioned |
| `official_error_code_included` | `official_error_code` text is in the prompt |
| `assertion_excerpt_included` | first 80 chars of `test_output_excerpt` is in the prompt |
| `source_snippet_included` | `"Repository context"` literal is present |
| `previous_patch_hash_included` | every previous patch hash appears |
| `no_repeat_constraint_included` | `"Do not repeat an unchanged patch"` literal present |
| `source_of_truth_refs_included` | every short source-of-truth ref appears |
| `hidden_solution_leak_absent` | gold patch / test patch text absent (hard gate) |

Outputs include prompt text, prompt manifest, request/response artifacts, parse report, and prompt-quality metadata. Each is byte-equal-verified on readback.

## 7. Continuous Run-Until-Resolved + GPU VRAM Budgeting (commit b7c4c5a2)
Both loops accept `--run-until-resolved`. When set, the attempt counter is uncapped (`while args.run_until_resolved || attempt <= args.max_attempts`, reality-loop.rs:342) and the loop only terminates on:

1. `solved == true` (an attempt's official evidence has `resolved_ids` containing the instance), OR
2. A fail-closed error (any of the gates in §4).

`--run-until-resolved` requires `--official-eval-each-attempt` (reality-loop.rs:206). Without official Docker readback, the loop refuses to enter continuous mode because there would be no oracle.

GPU VRAM is read from `nvidia-smi --query-gpu=index,name,uuid,memory.total,memory.used,memory.free,driver_version` (reality-loop.rs:2002) before and after every Ollama call (`assert_gpu_budget`, line 2069). Defaults:

| Flag | Default | Purpose |
| --- | --- | --- |
| `--gpu-index` | 0 | which GPU to read |
| `--gpu-min-free-mb` | 8192 | refuses to call model when free < this |
| `--gpu-max-used-mb` | unset | optional ceiling on used VRAM |
| `--ollama-keep-alive` | `"30m"` | passed to Ollama `keep_alive` so the model isn't reloaded each attempt |

A `compute_processes_raw` snapshot (`nvidia-smi --query-compute-apps=pid,process_name,used_memory`) is captured alongside, so VRAM blame can be assigned post-hoc.

## 8. Per-Attempt Official SWE-Bench Eval & Reprompt (commit 94ca398d)
`run_official_eval` (reality-loop.rs:1422) is invoked **inside** the attempt loop whenever validation passes locally. Steps:

1. Verify Docker SDK preflight via `python -c "import docker; ..."` returning JSON.
2. Write `official-prediction.jsonl` with one row `{instance_id, model_name_or_path, model_patch}`.
3. Run `python -m swebench.harness.run_evaluation --dataset_name princeton-nlp/SWE-bench_Lite --split test --instance_ids <id> --predictions_path <path> --max_workers <n> --timeout <s> --cache_level <env> --run_id <id> --report_dir <dir>` with `DOCKER_HOST` and `DOCKER_CONFIG` env vars (reality-loop.rs:1559).
4. Find the JSON report whose filename contains `<run_id>` and that has a `resolved_ids` array (reality-loop.rs:1676).
5. Build `official-success-evidence.json` or `official-failure-evidence.json` carrying `resolved`, `completed`, `incomplete`, `empty_patch`, `errored`, `previous_patch_sha256`, `source_of_truth`, and the full `official_report` payload (reality-loop.rs:1592).
6. Persist a BlackBoxFix `score` row and (when `config/reward_spec.verified_outcome.json` is present) a `reward compile` row for the attempt.

If the report exits 0 but the target `instance_id` is not in any of `completed_ids | incomplete_ids | empty_patch_ids | error_ids`, the loop raises `RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING` rather than silently treating the run as a non-resolution (reality-loop.rs:1610). Failure evidence becomes the next prompt's `official_evidence` input (deterministic prompt compiler §6).

## 9. Reward Spec (`reward_spec.verified_outcome.json`)
| Field | Value |
| --- | --- |
| `reward_spec_schema_version` | 1 |
| `reward_id` | `contextgraph_verified_outcome_demo_v1` |
| `objective` | "maximize verified source-of-truth outcomes while penalizing incidents, risk, and missing governance" |
| `bounds.min` | -10.0 |
| `bounds.max` | 10.0 |

| Component id | Feature | Weight |
| --- | --- | --- |
| `correctness` | `correctness` | +4.0 |
| `verification_confidence` | `verification_confidence` | +2.0 |
| `risk_penalty` | `risk` | -2.0 |
| `incident_penalty` | `incident_present` | -3.0 |
| `policy_missing_penalty` | `policy_missing` | -1.0 |
| `training_value` | `training_value` | +1.0 |

The Rust loop forwards this file to `target/debug/context-graph blackbox reward compile --input <reward_spec>` after every attempt (reality-loop.rs:530-552).

## 10. Smoke Curriculum & Scoreboard
The initial smoke curriculum (`gemmafrontend/swe_smoke_benchmark_suite.json`) is intentionally tiny:

| Field | Value |
| --- | --- |
| `suite_id` | `contextgraph_swe_lite_smoke_v1` |
| `dataset` | `princeton-nlp/SWE-bench_Lite` |
| `split` | `test` |
| `limits.max_models_per_batch` | 1 |
| `limits.max_workers` | 1 |
| `limits.max_attempts_per_model_task_run` | 1 |
| `limits.official_timeout_seconds` | 900 |
| `limits.expand_only_after_manual_review` | true |

Selected instances:

| `instance_id` | `repo` | FAIL_TO_PASS count | Baseline status |
| --- | --- | --- | --- |
| `psf__requests-2317` | `psf/requests` | 8 | `official_unresolved` |
| `django__django-12908` | `django/django` | 2 | `not_yet_run` |

Promotion rules (`promotion_rules`): more instances may be added only after (a) official Docker readback is persisted into BlackBoxFix for every selected instance, (b) failure evidence from official logs is compacted into the next model prompt, (c) at least one task shows reward improvement across attempts, and (d) Docker cache stays manually inspected. The `do_not_claim_success_until` clause re-asserts the official-resolved-ids predicate.

## 11. CLI Usage and Flags

### Rust `reality-loop` binary
Built as part of `crates/context-graph-cli` via `[[bin]] name = "reality-loop"` (Cargo.toml). Invoked:

```
cargo run --release --bin reality-loop -- \
  --task-manifest <path>.json \
  --model qwen3.6:27b \
  --official-eval-each-attempt \
  --max-attempts 1
```

| Flag | Default | Source |
| --- | --- | --- |
| `--model` | `qwen3.6:27b` | reality-loop.rs:14, 26 |
| `--task-manifest` | (required) | reality-loop.rs:28 |
| `--runtime-root` | `/tmp/contextgraph-rust-reality-loop` | reality-loop.rs:12, 30 |
| `--ollama-url` | `http://127.0.0.1:11434` | reality-loop.rs:15, 32 |
| `--context-graph-bin` | `target/debug/context-graph` | reality-loop.rs:13, 34 |
| `--max-attempts` | 1 | reality-loop.rs:36 |
| `--run-until-resolved` | off | reality-loop.rs:38 |
| `--official-eval-each-attempt` | off (loop refuses to run without it) | reality-loop.rs:40 |
| `--swebench-python` | `/home/cabdru/.cache/contextgraph-swebench/venv/bin/python` | reality-loop.rs:42-46 |
| `--official-timeout` | 900 s | reality-loop.rs:47 |
| `--official-max-workers` | 1 | reality-loop.rs:49 |
| `--official-cache-level` | `env` | reality-loop.rs:51 |
| `--official-docker-host` | `unix:///var/run/docker.sock` | reality-loop.rs:18, 53 |
| `--official-docker-config` | `/home/cabdru/.cache/contextgraph-docker-config` | reality-loop.rs:19, 55 |
| `--gpu-index` | 0 | reality-loop.rs:57 |
| `--gpu-min-free-mb` | 8192 | reality-loop.rs:16, 59 |
| `--gpu-max-used-mb` | unset | reality-loop.rs:61 |
| `--ollama-keep-alive` | `30m` | reality-loop.rs:17, 63 |
| `--model-timeout` | 300 s | reality-loop.rs:65 |
| `--git-timeout` | 600 s | reality-loop.rs:67 |
| `--sleep-between-attempts-ms` | 0 | reality-loop.rs:69 |

The loop exits with stdout `RUST_REALITY_LOOP_SUMMARY=<summary-source-of-truth>` on success or stderr `RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH=<failure-artifact-path>` on failure; both line prefixes are stable for shell consumers.

The previous Python SWE-bench loop and its Python official-eval wrapper were deleted. Operator workflows must use the Rust `reality-loop` command above.


### Python `model_benchmark.py`
Multi-model verified-outcome benchmark. Operator passes `--base-url` and one or more `--model`; the script POSTs each through `gemmafrontend/server.py`, reads the persisted snapshot back, and validates the BlackBoxFix ledger (`schema_version=14`) directly via SQLite (`direct_ledger_counts`, line 79).

### Python `model_prompt_matrix.py`
Same shape as `model_benchmark.py` but extends to a model × prompt grid plus 4 edge cases (`run_edge_cases`, line 308). Writes a Markdown summary (`write_markdown`, line 374).

### Python `manual_fsv.py`
End-to-end FSV harness: starts the local `gemmafrontend/server.py` against a `--run-root`, exercises the agent-tool gateway in read-only mode, and reads `blackbox.sqlite` back as source of truth.

## 12. Cross-References
- BlackBoxFix ledger schema, verifier framework, score, reward, redaction policy: see [10_cli_reference.md](10_cli_reference.md) `blackbox` subcommand reference and [15_verification_report.md](15_verification_report.md) §6 (`LEDGER_SCHEMA_VERSION = 14`).
- Reality-loop ledger entries are also indexed into the durable RocksDB store via the same MCP tools surfaced in [08_mcp_tools_reference.md](08_mcp_tools_reference.md).
- The DynamicJEPA subsystem ([21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md)) sits beside the reality loop: DJ owns latent-state predictions, while the reality loop owns SWE-bench oracle evidence; both share BlackBoxFix as their audit substrate.
- For column-family and storage details that back the reality loop's persisted artefacts, see [04_database_schema.md](04_database_schema.md) and [09_storage_layer.md](09_storage_layer.md).

## 13. Source Notes & Lessons
- The Rust loop refuses to enter the model loop without `--official-eval-each-attempt`. This is the structural enforcement of the "no measurement, no claim" boundary added by commit cc8e6c33.
- Every `write_*_checked` helper performs a byte-equal readback after `fs::write`. JSON readbacks parse back to a `Value` and compare structurally; text readbacks compare bytes. This catches both filesystem and serialization bugs before they propagate.
- Repeated patch detection compares `sha256_text(patch)` against `previous_patch_hashes` (reality-loop.rs:402-413). Repetition is treated as a hard failure (`RUST_REALITY_PATCH_REPEATED`) so the model's reprompt pressure is real.
- The Python loop's `agent_tool_rounds` defaults to 2 read-only tool rounds per attempt; setting it to 0 short-circuits to a single `ollama_generate_patch` call (line 1597).
- `--official-docker-config` defaults to a project-local cache dir so the WSL Docker SDK does not depend on the operator's `~/.docker/config.json`.
