---
name: ccreality-smoke
description: Run a synthetic ccreality smoke that verifies the entire engine + MCP + hook + recommendation pipeline without burning Docker time. Use this first when you set up ccreality on a new machine or after harness changes.
---

# ccreality-smoke

You are running the ccreality smoke flow. Goal: confirm every layer of the loop is wired correctly.

Step 1 - Trigger smoke. Call `mcp__cgreality__reality_run_smoke` with `label: "ccreality-skill-smoke"`. Receive the run_id and attempt_summary_path.

Step 2 - Verify analyze tools. Call:
- `mcp__cgreality__reality_attempt_summary` for the new attempt
- `mcp__cgreality__reality_signal` (compact)
- `mcp__cgreality__reality_problem_packet` (compact)
Confirm each returns a non-empty payload with `source_of_truth` pointing to a real file under the runtime root.

Step 3 - Verify alter tools (no-op edit). Call `mcp__cgreality__harness_open_window` on a benign file like `docs/ccreality/00_INDEX.md` lines 1-5. Receive the SHA. Do NOT actually edit; just confirm the open call works.

Step 4 - Verify command tool. Call `mcp__cgreality__harness_run_command` with `command: "cargo --version"`. Confirm exit_code 0 and the artifact path is returned.

Step 5 - Verify state. Call `mcp__cgreality__harness_verify_state` with `scope: "reality_loop_only"`. Confirm rustfmt/cargo check/cargo test all pass.

Step 6 - Record smoke recommendation. Call `mcp__cgreality__optimizer_record_recommendation` with status `no_change`, reason `"smoke flow completed; all layers verified"`, the source-of-truth paths from steps 2-5.

If any step fails, record status `error` with the failing tool name and source-of-truth.
