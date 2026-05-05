---
name: ccreality-smoke
description: Run a synthetic ccreality smoke that verifies the entire engine + MCP + hook + recommendation pipeline without burning Docker time. Use this first when you set up ccreality on a new machine or after harness changes.
---

# ccreality-smoke

You are running the ccreality smoke flow. Goal: confirm every layer of the loop is wired correctly.

For deterministic full-state verification from a shell, run `bash scripts/cc/tests/smoke.sh`. That harness is the Phase 9 source of truth because it performs MCP calls, hook invocations, file readbacks, SQLite ledger inspection, drift simulation, deny checks, invalid-write checks, and result document generation.

When invoked interactively, run the same fail-closed MCP flow. Do not use mock data, inferred state, or a fallback success message. If any required readback is missing, stop and report the exact tool and source-of-truth path that failed.

Step 1 - Trigger smoke. Call `mcp__cgreality__reality_run_smoke` with `label: "ccreality-skill-smoke"` and a real `session_id`. Receive the `run_id`, `runtime_root`, and attempt summary location. The runtime root must contain the generated attempt artifacts.

Step 2 - Verify analyze tools. Call:
- `mcp__cgreality__reality_attempt_summary` for the new attempt
- `mcp__cgreality__reality_signal` (compact)
- `mcp__cgreality__reality_problem_packet` (compact)
Confirm each returns a non-empty payload with `source_of_truth` pointing to a real file under the runtime root.

Step 3 - Verify alter tools (no-op edit). Call `mcp__cgreality__harness_open_window` on a benign file like `docs/ccreality/00_INDEX.md` lines 1-5. Receive the SHA. Do NOT actually edit; just confirm the open call works.

Step 4 - Verify command tool. Call `mcp__cgreality__harness_run_command` with `command: "git status --short"`. Confirm `exit_code` is 0 and the artifact path is returned. Do not use commands outside the governed allowlist.

Step 5 - Verify state. Call `mcp__cgreality__harness_verify_state` with `scope: "reality_loop_only"`. Confirm rustfmt/cargo check/cargo test all pass.

Step 6 - Record smoke recommendation. Before recording, ensure the runtime root has a real BlackBox ledger initialized and an `agent_run` row for the smoke task. The BlackBox CLI mints its own ledger `run_id`; include the ccreality smoke `run_id` in the ledger task text, then let optimizer tools attach tool-call rows to the latest BlackBox run. If the ledger is absent or empty, the recommendation write must fail instead of pretending persistence exists.

Call `mcp__cgreality__optimizer_record_recommendation` with a schema-complete `no_change` payload:
- `schema_version: 1`
- `record_kind: "ccreality_optimizer_recommendation"`
- `claude_session_id`: the real session id
- `claude_model`: one of the schema enum values, such as `"haiku"`, `"sonnet"`, `"opus"`, or `"inherit"`
- `run_id`: the smoke run id
- `attempt: 1`
- `turn_number`: current turn number
- `status: "no_change"`
- `reason`: `"smoke flow completed; all layers verified"`
- `source_files_changed: []`
- `verification_commands`: include `git status --short` and `harness_verify_state reality_loop_only`
- `source_of_truth_readbacks`: the exact `file:` URIs from steps 2-5
- `next_gemma_prompt_delta`: explicit no-change statement
- `expected_reward_signal_delta`: explicit no-change statement
- `remaining_risks: []`
- `memories_created: []`
- `memories_boosted: []`
- `harness_transitions_minted: []`
- `shift_log_excerpt: []`
- `cost_estimate_usd`: numeric estimate
- `created_at_unix`: current unix timestamp
- `source_of_truth`: placeholder file URI; the tool will replace it with the persisted recommendation path

If any step fails, record status `error` with the failing tool name and source-of-truth.
