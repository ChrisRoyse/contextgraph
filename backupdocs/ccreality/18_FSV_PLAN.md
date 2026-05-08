# Full State Verification (FSV) Plan

Beyond smoke (single-run verification of the wiring), FSV exercises the loop against synthetic and real attempts to verify the contract holds under every meaningful failure mode.

FSV is the project's existing pattern (see `docs/MEMORY_REVIEW_2026_02_24.md` and prior audit reports under `docs/HIDDEN_BROKENNESS_AUDIT_*.md`). For ccreality, FSV covers six categories:

## Current Incremental FSV Evidence

As of 2026-05-05, the project has passed incremental manual FSV for the implemented cutover phases. This does not replace Phase 10's full A–F suite; it records the source-of-truth evidence already produced while completing Phases 2–5.

| Scope | Status | Source of truth |
|-------|--------|-----------------|
| Phase 2 engine subcommands | PASS | `/tmp/ccreality-fsv-1777999742.log`; runtime root `/tmp/cgreality-phase02-fsv-1777999742`; active-root files read back under `~/.cache/cgreality/`. |
| Phase 3 MCP analyze | PASS | `/tmp/ccreality-fsv-1777999742.log`; `tools/list` count 173, expected analyze tools present, live artifact readbacks tied to `file:` source-of-truth paths. |
| Phase 4 MCP interact/alter/shift | PASS | `/tmp/ccreality-fsv-1777999742.log`; governed edit, harness transition, shift-log, malformed recommendation rejection, SQL-injection rejection, and drift comparison readbacks. |
| Phase 4 ledger append hardening | PASS | `/tmp/ccreality-phase4-ledger-fsv-1778000024.log`; SQLite ledger `/tmp/cgreality-phase04-ledger-fsv-1778000024/ledger.sqlite` contains `agent_run|1` and `tool_call|2`. |
| Phase 5 strict config/schema | PASS | `/tmp/ccreality-phase5-fsv-1778001835.log`; policy/schema/sample file readbacks, real MCP recommendation write, shift-log append, ledger row, and four fail-closed edge cases. |

Phase 10 still must build the automated `scripts/cc/tests/fsv.sh` suite and produce `docs/ccreality/FSV_RESULTS_<date>.md`.

## A. Reality artifact integrity (engine side)

| FSV ID | Scenario | Pass criterion |
|--------|----------|----------------|
| A1 | Run `smoke` subcommand. Compare written `attempt-summary.json` SHA to the SHA the engine logged. | SHAs equal. |
| A2 | Run `attempt` subcommand against a known-passing synthetic task. | `resolved: true`, official report shows instance in `resolved_ids`. |
| A3 | Run `attempt` against a known-failing synthetic task. | `resolved: false`, failure_evidence has classified failure_class. |
| A4 | Two consecutive `attempt` runs with same patch. | Second attempt sets `repeated_patch: true`. |
| A5 | Engine crashes mid-attempt (kill -9 mid-run). | Failure artifact written with engine error_code; no orphaned attempt directory. |

## B. MCP analyze tools

| B1 | `reality_latest_root` returns active root after smoke. | Root path matches `~/.cache/cgreality/active_runtime_root`. |
| B2 | `reality_attempt_summary` for a known attempt returns identical JSON to the on-disk file. | Byte-equal. |
| B3 | `reality_compare_attempts` between two distinct attempts returns non-empty diff. | `patch_diff_summary.lines_added` > 0 OR `validation_status_change` present. |
| B4 | `reality_signal` with `compact: true` returns the same content as `compact_reality_signal_packet` would produce. | Equal. |
| B5 | `reality_audit_trail` for a known-stored memory returns the audit chain. | Chain length ≥ 1; first entry is `created`. |
| B6 | `reality_query_ledger` with read-only WHERE clause returns results. | `row_count` > 0; SQL injection attempt rejected. |

## C. MCP alter tools (the shift-principle suite)

| C1 | `harness_open_window` then `harness_apply_line_window_edit` with correct SHA. | Edit applies; `after_sha256` matches re-read; shift_log gets one entry. |
| C2 | `harness_apply_line_window_edit` with stale SHA. | Returns `HARNESS_EDIT_SHA_MISMATCH`; file unchanged; no shift logged. |
| C3 | `harness_apply_line_window_edit` against a denied path. | Returns `HARNESS_PATH_DENIED`; file unchanged. |
| C4 | `harness_apply_line_window_edit` succeeds; subsequent `reality_shift_log` returns the shift. | shift_id present; before/after SHA match. |
| C5 | External writer mutates the file between `harness_open_window` and `harness_apply_line_window_edit`. | Edit fails with SHA mismatch; `reality_shift_compare_to_my_view` reports `external_drift_detected`. |
| C6 | `optimizer_record_recommendation` with valid payload. | Artifact written; ledger row present; readback SHA matches. |
| C7 | `optimizer_record_recommendation` with malformed payload. | Returns `CCREALITY_RECOMMENDATION_SCHEMA_INVALID`; no artifact written. |
| C8 | `harness_run_command` with allowlisted command. | Exit code captured; artifact written; output matches actual execution. |
| C9 | `harness_run_command` with denied command. | Returns `HARNESS_COMMAND_DENIED`; no execution. |
| C10 | `harness_run_command` attempts shell metachar bypass (`cargo build && rm -rf /`). | Denied; the `&&` is treated as part of the command string and rejected. |
| C11 | Two parallel `harness_apply_line_window_edit` calls in the same MCP server. | Second returns `CCREALITY_HARNESS_EDIT_OWNER_BUSY`. |
| C12 | Two writers across two Claude Code sessions. | Second is denied by edit lock; PreToolUse hook returns deny with the lock holder. |

## D. Hooks

| D1 | SessionStart with active runtime root. | `additionalContext` includes the runtime root path. |
| D2 | SessionStart with no runtime root. | `additionalContext` says "not currently running"; exit 0. |
| D3 | UserPromptSubmit with a fresh attempt. | `additionalContext` includes trigger decision + reality-signal compact. |
| D4 | UserPromptSubmit after edit shifts. | `additionalContext` includes "Last 3 reality shifts in this session". |
| D5 | PreToolUse Edit on allowed path. | `permissionDecision: allow` + current SHA in additionalContext. |
| D6 | PreToolUse Edit on denied path (.env). | `permissionDecision: deny` with reason. |
| D7 | PostToolUse Edit on a Rust file under `crates/context-graph-cli/`. | cargo check runs; result in additionalContext; harness-transition record minted. |
| D8 | Stop hook with no recommendation recorded. | `decision: block` with reason; turn cannot end. |
| D9 | Stop hook after recommendation. | exit 0; turn ends. |
| D10 | PreCompact. | Re-injects runtime root + last shift summary. |

## E. Drip + drift end-to-end

| E1 | Operator runs `/ccreality-iterate` after a synthetic failure. The diagnostician identifies it; the writer edits one file; the readback confirms; the recommendation is recorded. | Full chain succeeds; recommendation status=changed; cargo check PASS. |
| E2 | Same as E1, but mid-edit, an external writer mutates the same file. Writer sees SHA mismatch on apply. | Writer surfaces drift via the recommendation; status=blocked; reason mentions external drift. |
| E3 | Operator runs `/loop 2m` for 6 minutes (3 fires) against a synthetic always-failing task. | Three recommendations recorded; each cites different turn_number; shift log shows accumulation. |

## F. Cost and limits

| F1 | Headless run `claude -p "/ccreality-smoke" --max-turns 25`. | Session terminates within 25 model rounds; total_cost_usd reported. |
| F2 | Large `additionalContext` injection (force the hook to emit >10 KB). | Truncated to 10 KB; hook still exits 0. |
| F3 | MCP `reality_attempt_summary` for an attempt with a 100 MB official-evidence artifact. | Returns pointer + size; doesn't inline the full content. |

## FSV harness

`scripts/cc/tests/fsv.sh` runs the full A–F suite. Each FSV case has a synthetic setup (creates a fake runtime root with hand-crafted artifacts) and a teardown (deletes the temp tree).

For C5 (external drift), the test uses two parallel processes: one Claude Code session, one bash background job that mutates the file. Coordination via `~/.cache/cgreality/fsv-c5-mutex`.

For C11/C12 (parallel writers), the test spawns two `claude -p "/ccreality-iterate"` processes simultaneously and asserts exactly one gets the lock.

## FSV pass threshold

All A–E cases must pass before merging the ccreality cutover. F cases must pass within budget tolerance (F1 within 30% of expected cost; F2/F3 within absolute byte limits).

If any case fails:
- Log the failing case ID + reproduction details.
- Open an issue under `docs/ccreality/20_OPEN_QUESTIONS.md`.
- Block merge until resolved.

## FSV reporting

After running, generate `docs/ccreality/FSV_RESULTS_<date>.md` with one row per case:

```markdown
| Case | Status | Time (s) | Notes |
|------|--------|----------|-------|
| A1   | PASS   | 0.4      |       |
| ...  | ...    | ...      |       |
```

Same format as existing audit reports under `docs/MEMORY_REVIEW_2026_02_24.md`.

## Future FSV expansion

Once the loop is in production, add:
- Long-run FSV: 24 hours of `/loop 30m` against a real SWE-bench instance, asserting no orphaned artifacts and bounded ledger growth.
- Adversarial FSV: feed pathological reality (zero-byte attempt-summary, malformed JSON in reality-signal-packet, missing official-evidence) and verify graceful failure.
- Recovery FSV: kill the MCP server mid-tool-call; restart; verify the next session's shift log doesn't contain a phantom shift.
