# Hardening Analysis

## Summary

This pass reviewed the completed ccreality implementation across phases 0-12, then ran fresh stack verification. Two new root-cause issues were found and fixed during this pass.

## Root Cause 1 - Stale MCP Tool Count In Verification

**Symptom:** `scripts/cc/tests/smoke.sh` failed with:

```text
SMOKE FAIL: expected at least 30 MCP tools, got 28
```

The Phase 0-8 hardening script also still expected the old pre-hardening tool surface.

**Root cause:** Phase 12 correctly hardened `context-graph-mcp --mode reality-loop` to advertise only callable cgreality tools. The test harnesses still expected the older mixed surface that included full-server memory/graph tools. That old expectation would have pushed the system back toward the original bug: tools advertised but not callable.

**Fix:**

- `scripts/cc/tests/smoke.sh` now expects at least 28 cgreality tools.
- `scripts/cc/tests/smoke.sh` explicitly fails if full-server tools such as `search_graph`, `store_memory`, or `get_audit_trail` leak into the lightweight server list.
- `scripts/cc/tests/verify_phase0_8_hardening.sh` now has the same hardened expectation.

**Verification:**

```text
Layer 2 - MCP server :: 28 callable cgreality tools listed; required tools present and full-server tools absent
source_of_truth.phase4.tool_count=28
```

## Root Cause 2 - Parallel Stateful FSV Tests In Governed Verification

**Symptom:** During smoke Layer 5, `harness_verify_state` failed because `cargo test -p context-graph-cli --bin reality-loop -- --skip ignored` had one flaky failure:

```text
RUST_REALITY_STRUCTURED_EDIT_EMPTY_DIFF
```

The failed test passed when run alone.

**Root cause:** The reality-loop binary tests are full-state tests using real git repositories, real patches, and process-level command execution. Running the whole binary test set in parallel created nondeterministic interference for git-diff based source-of-truth assertions.

**Fix:**

- `context_graph_cli::governed_edit::verify_project_state` now runs cargo tests with `--test-threads=1`.
- `scripts/cc/tests/verify_phase0_8_hardening.sh` now also runs the reality-loop binary tests single-threaded.

**Verification:**

```bash
cargo test -p context-graph-cli --bin reality-loop -- --skip ignored --test-threads=1 --nocapture
```

Result: all 11 tests passed, including `tool_mediated_final_edit_submission_fsv`.

## Existing Phase 12 Hardening Revalidated

The fresh smoke and FSV runs revalidated the Phase 12 fixes:

- Lightweight MCP advertises only callable tools.
- SessionStart hook emits bounded compact attempt context.
- Python candidate edits are syntax-probed before `candidate.patch`.
- Invalid Python edits restore the real workspace and write durable failure artifacts.
- Optimizer recommendations get server-owned `created_at_unix` and `source_of_truth`.

## Remaining Risk

The production SWE-bench task `psf__requests-2317` remains unsolved. This is not a harness failure. The source-of-truth evidence shows the inner model submitted invalid edits and the harness rejected them correctly. The next optimization work should improve the inner prompt/tool strategy rather than weakening validation.

## Hardening Principle Going Forward

Every future verifier must track the current source-of-truth contract. If the implementation intentionally removes a capability from a constrained runtime, the verifier should assert absence of that capability, not just lower the expected count.

