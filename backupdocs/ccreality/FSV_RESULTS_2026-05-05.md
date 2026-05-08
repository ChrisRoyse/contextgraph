# FSV Results - 2026-05-05

Branch: `ccreality-cutover`

HEAD: `0f12ca530868412ab0ffd1d81e5b802072dbed87`

Run root: `/tmp/cgreality-fullstack-fsv-1778022699`

Master log: `/tmp/cgreality-fullstack-fsv-1778022699/fsv-master.log`

Source of truth: runtime artifact files under each case work directory, `~/.cache/cgreality/active_runtime_root`, shift logs under `cgreality-shift-log`, hook event JSONL files, and BlackBox `ledger_records` rows in each case ledger.

## Per-case Results

| Case | Status | Time (s) | Notes |
|------|--------|----------|-------|
| A1_smoke_sha_match | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/A1_smoke_sha_match.log` |
| A2_passing_resolves | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/A2_passing_resolves.log` |
| A3_failing_does_not_resolve | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/A3_failing_does_not_resolve.log` |
| A4_repeated_patch_detected | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/A4_repeated_patch_detected.log` |
| A5_engine_crash_artifact | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/A5_engine_crash_artifact.log` |
| B1_latest_root | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/B1_latest_root.log` |
| B2_attempt_summary_readback | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/B2_attempt_summary_readback.log` |
| B3_compare_attempts | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/B3_compare_attempts.log` |
| B4_compact_signal | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/B4_compact_signal.log` |
| B5_audit_trail | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/B5_audit_trail.log` |
| B6_ledger_query_read_only | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/B6_ledger_query_read_only.log` |
| C1_open_then_apply | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C1_open_then_apply.log` |
| C2_stale_sha_fails | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C2_stale_sha_fails.log` |
| C3_denied_path_fails | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C3_denied_path_fails.log` |
| C4_shift_log_entry | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C4_shift_log_entry.log` |
| C5_external_drift_detected | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C5_external_drift_detected.log` |
| C6_valid_recommendation | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C6_valid_recommendation.log` |
| C7_malformed_recommendation_rejected | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C7_malformed_recommendation_rejected.log` |
| C8_allowed_command_runs | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C8_allowed_command_runs.log` |
| C9_denied_command_fails | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C9_denied_command_fails.log` |
| C10_metachar_bypass_denied | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C10_metachar_bypass_denied.log` |
| C11_parallel_mcp_lock | PASS | 6 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C11_parallel_mcp_lock.log` |
| C12_stale_lock_recovered | PASS | -6 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/C12_stale_lock_recovered.log` |
| D1_session_start_active_root | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D1_session_start_active_root.log` |
| D2_session_start_no_root | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D2_session_start_no_root.log` |
| D3_user_prompt_fresh_attempt | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D3_user_prompt_fresh_attempt.log` |
| D4_user_prompt_shift_summary | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D4_user_prompt_shift_summary.log` |
| D5_pretool_allow | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D5_pretool_allow.log` |
| D6_pretool_deny | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D6_pretool_deny.log` |
| D7_posttool_records | PASS | 2 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D7_posttool_records.log` |
| D8_stop_blocks_without_recommendation | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D8_stop_blocks_without_recommendation.log` |
| D9_stop_allows_after_recommendation | PASS | 0 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D9_stop_allows_after_recommendation.log` |
| D10_precompact_reinjects | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/D10_precompact_reinjects.log` |
| E1_drip_happy_path | PASS | 51 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/E1_drip_happy_path.log` |
| E2_drift_mid_edit | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/E2_drift_mid_edit.log` |
| E3_loop_three_fires | PASS | 2 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/E3_loop_three_fires.log` |
| F1_claude_cost_limit | PASS | 28 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/F1_claude_cost_limit.log` |
| F2_hook_context_size | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/F2_hook_context_size.log` |
| F3_large_artifact_pointer | PASS | 1 | `log: /tmp/cgreality-fullstack-fsv-1778022699/logs/F3_large_artifact_pointer.log` |

## Boundary & Edge Case Audit

| Edge Case | Case | Source-of-truth proof |
|-----------|------|-----------------------|
| Invalid/stale SHA edit is rejected and file remains unchanged | C2 | `/tmp/cgreality-fullstack-fsv-1778022699/cases/C2_stale_sha_fails/state.log` |
| Denied path access fails without mutating `.env` | C3 | `/tmp/cgreality-fullstack-fsv-1778022699/cases/C3_denied_path_fails/state.log` |
| Maximum-size artifact returns a pointer rather than inline data | F3 | `/tmp/cgreality-fullstack-fsv-1778022699/cases/F3_large_artifact_pointer/source-of-truth.log` |

## Evidence Logs

- Case logs: `/tmp/cgreality-fullstack-fsv-1778022699/logs`
- MCP stderr: `/tmp/cgreality-fullstack-fsv-1778022699/mcp-stderr.log`
- Per-case source-of-truth logs: `/tmp/cgreality-fullstack-fsv-1778022699/cases/<case>/source-of-truth.log`
- Per-case before/after state logs: `/tmp/cgreality-fullstack-fsv-1778022699/cases/<case>/state.log`

## Summary

PASS: 39, FAIL: 0
