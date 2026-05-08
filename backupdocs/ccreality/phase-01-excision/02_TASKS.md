# Phase 1 — Tasks

Eighteen ordered atomic tasks. Each has a verification command. Do NOT proceed past a task if its verification fails — diagnose and fix first (see [05_RECOVERY.md](05_RECOVERY.md)).

The order matters. Tasks 1.1–1.5 lift the governed-edit module FIRST so the deletes in 1.6+ don't accidentally remove code we still need. Tests are deleted last (1.16) because they reference the doomed code and would otherwise break the build mid-phase.

## Task 1.1 — Decide on lib crate target

Check whether `crates/context-graph-cli/` already exposes a library:
```bash
test -f /home/cabdru/contextgraph/crates/context-graph-cli/src/lib.rs && echo "lib exists" || echo "no lib"
grep -A 3 '^\[lib\]' /home/cabdru/contextgraph/crates/context-graph-cli/Cargo.toml || echo "no [lib] in manifest"
```

If lib does not exist, create one (Task 1.2 will do this). If it exists, add the `governed_edit` module to it.

## Task 1.2 — Create or update lib.rs

If `crates/context-graph-cli/src/lib.rs` does NOT exist, create it with content from [03_FILE_SPECS.md](03_FILE_SPECS.md) §"`crates/context-graph-cli/src/lib.rs`".

If it DOES exist, prepend `pub mod governed_edit;` to it (only if not already present).

Add `[lib]` to `Cargo.toml` if missing — see [03_FILE_SPECS.md](03_FILE_SPECS.md) §"`Cargo.toml` `[lib]` section".

Verify:
```bash
cargo check -p context-graph-cli --lib --quiet
echo "exit=$?"
```
Expected: `exit=0`. The lib has no body yet so this just checks the target builds.

## Task 1.3 — Create `governed_edit.rs` skeleton

Write `crates/context-graph-cli/src/governed_edit.rs` with the public API skeleton from [03_FILE_SPECS.md](03_FILE_SPECS.md) §"`crates/context-graph-cli/src/governed_edit.rs` skeleton". Body of each function is `unimplemented!()` for now.

Verify:
```bash
cargo check -p context-graph-cli --lib --quiet
echo "exit=$?"
```
Expected: `exit=0`.

## Task 1.4 — Move five functions verbatim into `governed_edit.rs`

From `reality-loop.rs`, copy (then delete from the original) these five functions:

| Source line | Function |
|-------------|----------|
| 4258 | `optimizer_project_root` → rename to `project_root` |
| 4278 | `project_file_window` → keep name |
| 4336 | `project_apply_line_window_edits` → keep name |
| 4550 | `run_openai_project_command` → rename to `run_governed_command` |
| 4665 | `verify_openai_project_state` → rename to `verify_project_state` |
| 4802 | `validate_optimizer_project_path` → rename to `validate_governed_path` |

The renames remove the `openai_` and `optimizer_` provider hints from the API.

Adjust function bodies:
- Replace any `RUST_REALITY_OPENAI_*` error code with `RUST_REALITY_GOVERNED_*`.
- Replace any `openai_optimizer_tool_blocked` callsite with a generic `governed_tool_blocked` (define this locally in `governed_edit.rs` — see file specs).
- Imports: bring in `serde_json::Value`, `std::path::{Path, PathBuf}`, `std::fs`, `crate::sha256_text`, etc. — see file specs.
- Make the path-policy reader configurable: `validate_governed_path` reads from `CGREALITY_GOVERNED_PATHS` env var (path to JSON file with `{ "allow": [...], "deny": [...] }`). Falls back to a hard-coded conservative deny list if env unset.

Verify:
```bash
cargo build -p context-graph-cli --bin reality-loop --quiet
echo "exit=$?"
```
Expected: `exit=0`. (The bin still has the original copies; the lib has fresh ones — they coexist temporarily.)

## Task 1.5 — Move associated tests

From `reality-loop.rs` `#[cfg(test)]` block, move these three tests into `governed_edit.rs`'s own `#[cfg(test)] mod tests`:

| Source line | Test name | New name |
|-------------|-----------|----------|
| 10669 | `openai_project_edit_tool_updates_real_file_and_reads_back_diff` | `project_edit_tool_updates_real_file_and_reads_back_diff` |
| 10755 | `openai_project_edit_rejects_stale_observed_sha256` | `project_edit_rejects_stale_observed_sha256` |
| 10796 | `openai_project_path_policy_blocks_secret_and_traversal` | `governed_path_policy_blocks_secret_and_traversal` |

Adjust:
- Function call sites use the new names from Task 1.4.
- The path policy test must set `CGREALITY_GOVERNED_PATHS` env to a synthetic JSON file in a tempdir.

Verify:
```bash
cargo test -p context-graph-cli --lib governed_edit --quiet
echo "exit=$?"
```
Expected: `exit=0` with all three tests passing.

## Task 1.6 — Delete the 16 OpenAI/DeepSeek constants

In `reality-loop.rs`, delete lines containing these constants (currently lines 22–37):
- `DEFAULT_OPENAI_OPTIMIZER_MODEL`
- `DEFAULT_OPENAI_RESPONSES_URL`
- `DEFAULT_OPENAI_API_KEY_ENV`
- `DEFAULT_OPENAI_OPTIMIZER_MAX_OUTPUT_TOKENS`
- `DEFAULT_OPENAI_OPTIMIZER_TOOL_ROUNDS`
- `DEFAULT_OPENAI_MAX_RETRIES`
- `DEFAULT_OPENAI_INITIAL_BACKOFF_MS`
- `DEFAULT_OPENAI_MAX_BACKOFF_MS`
- `DEFAULT_OPENAI_DEPENDENCY_BLOCK_SLEEP_MS`
- `DEFAULT_OPTIMIZER_MAX_ESTIMATED_CALL_COST_USD`
- `DEFAULT_OPTIMIZER_TRIGGER_POLICY`
- `DEFAULT_OPTIMIZER_MAX_PARALLEL_AGENTS`
- `DEFAULT_OPTIMIZER_REWARD_REGRESSION_THRESHOLD`
- `DEEPSEEK_V4_PRO_CACHE_HIT_INPUT_USD_PER_M`
- `DEEPSEEK_V4_PRO_CACHE_MISS_INPUT_USD_PER_M`
- `DEEPSEEK_V4_PRO_OUTPUT_USD_PER_M`

KEEP: every constant whose name does not contain OPENAI, DEEPSEEK, or OPTIMIZER.

Verify:
```bash
rg -n "DEFAULT_OPENAI|DEEPSEEK_V4_PRO|DEFAULT_OPTIMIZER" crates/context-graph-cli/src/bin/reality-loop.rs
echo "exit=$? (1 = no matches = pass)"
```
Expected: `exit=1`.

## Task 1.7 — Delete the 17 CLI flag fields from `struct Args`

In `reality-loop.rs` `struct Args` (around lines 46–139), delete these `#[arg(long ...)]` fields:
- `optimizer_api_smoke_test`
- `openai_optimizer_each_attempt`
- `openai_optimizer_model`
- `openai_responses_url`
- `openai_api_key_env`
- `openai_optimizer_tool_rounds`
- `openai_optimizer_max_output_tokens`
- `openai_optimizer_reasoning_effort`
- `openai_optimizer_timeout`
- `openai_max_retries`
- `openai_initial_backoff_ms`
- `openai_max_backoff_ms`
- `openai_dependency_block_sleep_ms`
- `optimizer_max_estimated_call_cost_usd`
- `optimizer_trigger_policy`
- `optimizer_max_parallel_agents`
- `optimizer_reward_regression_threshold`

Verify:
```bash
rg -n "openai_|optimizer_trigger_policy|optimizer_max_parallel_agents|optimizer_reward_regression_threshold|optimizer_max_estimated_call_cost_usd|optimizer_api_smoke_test" crates/context-graph-cli/src/bin/reality-loop.rs | grep -v "^//"
echo "exit=$? (non-zero = no match)"
```
Expected: no matches in non-comment lines.

## Task 1.8 — Delete the 2 `AttemptRecord` fields

In `struct AttemptRecord` (around lines 220–242), delete:
- `openai_optimizer_recommendation_path: Option<String>`
- `openai_optimizer_recommendation_compact: Option<Value>`

Verify:
```bash
rg -n "openai_optimizer_recommendation" crates/context-graph-cli/src/bin/reality-loop.rs
echo "exit=$? (1 = pass)"
```

## Task 1.9 — Update `test_args()` to drop deleted fields

In the `#[cfg(test)] mod tests` block, find `fn test_args(...)` (around line 9838). Delete every assignment that mentions a flag deleted in Task 1.7 or a field deleted in Task 1.8.

Verify:
```bash
cargo check -p context-graph-cli --bin reality-loop --quiet 2>&1 | grep -i "error\[" | head -10
```
Expected: no errors. (There may be unused-import warnings from the lift in Task 1.4; ignore for now.)

## Task 1.10 — Delete keys from `write_failure_artifact` payload

In `write_failure_artifact` (around lines 1511–1572), delete these keys from the `args` JSON object:
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

Verify:
```bash
cargo check -p context-graph-cli --bin reality-loop --quiet
echo "exit=$?"
```

## Task 1.11 — Delete the trigger+optimizer block in `run()`

In `run()` (function starts at line 283), find:
- The `optimizer_api_smoke_test` short-circuit (lines 326–344) → delete.
- The trigger-decision + run_openai_loop_optimizer block (lines 803–960) → delete entirely.

Replace the deleted region (around line 803) with this comment:
```rust
        // ccreality: outer optimizer no longer lives in the engine. The engine writes
        // attempt artifacts and exits; Claude Code (via the cgreality MCP server) reads
        // those artifacts and produces optimizer recommendations. See docs/ccreality/.
```

Also remove these from the `attempt-summary.json` JSON construction (around lines 967–984):
- `openai_optimizer_recommendation_path`
- `openai_optimizer_recommendation_compact`
- `deepseek_optimizer_trigger_decision`
- `openai_loop_optimizer`

Verify:
```bash
rg -n "run_openai_loop_optimizer|decide_deepseek_optimizer_trigger|optimizer_api_smoke_test" crates/context-graph-cli/src/bin/reality-loop.rs
echo "exit=$? (1 = pass)"
```

## Task 1.12 — Delete trigger/cost helper functions

Delete (in entirety) from `reality-loop.rs`:
- `validate_optimizer_trigger_config` (line 2117)
- `decide_deepseek_optimizer_trigger` (line 2156)
- `reward_scalar_from_reality_signal` (line 2281) — only if no other caller; the audit says no.

Verify:
```bash
rg -n "fn validate_optimizer_trigger_config|fn decide_deepseek_optimizer_trigger|fn reward_scalar_from_reality_signal" crates/context-graph-cli/src/bin/reality-loop.rs
echo "exit=$? (1 = pass)"
```

## Task 1.13 — Delete OpenAI/DeepSeek HTTP and helpers

Delete (in entirety):
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
- `required_json_u64` (3192)
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
- `exponential_backoff_ms` (3479)
- `struct OpenAiFunctionCall` (3487)
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
- `build_openai_self_reality_packet` (4708)
- `required_openai_tool_str` (4862)
- `openai_optimizer_json_schema` (4880)
- `validate_openai_optimizer_recommendation` (4949)
- `compact_openai_loop_optimizer` (4986)

Verify:
```bash
rg -n "openai|deepseek|gpt-5|chatgpt" crates/context-graph-cli/src/bin/reality-loop.rs | head -20
echo "exit=$? (1 = no matches = pass)"
```

## Task 1.14 — Delete the lifted-and-now-orphaned project_*/optimizer_project helpers

After Task 1.4 lifted `project_file_window`, `project_apply_line_window_edits`, `run_openai_project_command`, `verify_openai_project_state`, `validate_optimizer_project_path`, `optimizer_project_root` into `governed_edit.rs`, the originals in `reality-loop.rs` are duplicates. Delete them now (they live around lines 4258–4801).

Verify:
```bash
rg -n "fn project_file_window|fn project_apply_line_window_edits|fn run_openai_project_command|fn verify_openai_project_state|fn validate_optimizer_project_path|fn optimizer_project_root" crates/context-graph-cli/src/bin/reality-loop.rs
echo "exit=$? (1 = pass)"
```

## Task 1.15 — Run cargo check and fix unused imports

```bash
cargo check -p context-graph-cli --bin reality-loop 2>&1 | tee /tmp/cargo-check-phase1.log
```

Look for `unused import` warnings. Remove dead `use` lines. Common candidates after the deletes:
- `use std::collections::BTreeMap;` (if no longer used)
- `use reqwest::header::HeaderMap;` (only used by the deleted HTTP helpers)
- imports for `OpenAiFunctionCall`, `serde_json::json` patterns specific to deleted code.

Iterate until `cargo check` is clean (no warnings beyond the ones present at base SHA).

Verify:
```bash
cargo check -p context-graph-cli --bin reality-loop --quiet 2>&1 | grep -c "^warning"
```
Should match the warning count from the base SHA (run `git stash && cargo check` then `git stash pop` to compare; alternatively run on `main`).

## Task 1.16 — Delete obsolete tests

In the `#[cfg(test)] mod tests` block of `reality-loop.rs`, delete these tests (keep their preceding `#[test]` attribute removed too):
- `openai_error_classification_distinguishes_quota_from_retryable_rate_limit` (10197)
- `deepseek_tool_schema_uses_chat_completions_function_shape` (10256)
- `deepseek_tool_call_replay_preserves_reasoning_content_and_arguments` (10279)
- `deepseek_cost_preflight_and_usage_are_physical_artifacts` (10326)
- `deepseek_trigger_decision_fsv_runs_on_failed_official_reality` (10385)
- `deepseek_trigger_decision_fsv_blocks_disabled_success` (10441)
- `openai_replay_filters_store_false_reasoning_items` (10582)
- `openai_self_reality_packet_records_outer_agent_reality` (10820)

(The three project_*/path_policy tests are already moved in Task 1.5.)

Verify:
```bash
rg -n "fn openai_|fn deepseek_" crates/context-graph-cli/src/bin/reality-loop.rs
echo "exit=$? (1 = pass)"
```

## Task 1.17 — Run cargo fmt + cargo test

```bash
cargo fmt -p context-graph-cli
cargo test -p context-graph-cli --bin reality-loop --quiet -- --skip ignored
echo "exit=$?"
```
Expected: `exit=0` with all surviving tests passing.

If a test fails, diagnose:
- Did we accidentally delete a function the test calls? Check git diff.
- Did the test reference a removed `Args` field? Update the test (or delete if it was OpenAI-specific).

## Task 1.18 — Final size check

```bash
wc -l /home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs
```
Expected: between 6,500 and 8,500 lines. (10,873 - ~3,400 deleted - ~900 lifted = ~6,500–7,000.)

If outside that range, something was either over-deleted or under-deleted. Inspect.

## Phase complete

Run [04_VERIFICATION.md](04_VERIFICATION.md). When the full verification block returns exit 0, Phase 1 is complete. Do not commit partial Phase 1 work before verification passes. Current branch reality: Phase 1 has since been committed as part of incremental cutover commits recorded in the implementation plan.
