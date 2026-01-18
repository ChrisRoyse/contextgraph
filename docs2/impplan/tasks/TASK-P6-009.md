# Task: TASK-P6-009 - E2E Integration Tests

## STATUS: ✅ COMPLETE (2026-01-18)

### Completion Summary
- **35 tests passing** (20 E2E + 15 Integration)
- All happy path tests implemented and passing
- All edge case tests implemented and passing
- Manual FSV verification completed
- Code review and refactoring completed

### Test Results
```
cargo test --package context-graph-cli --test e2e --test integration -- --test-threads=1
running 35 tests: 35 passed; 0 failed
```

---

## CRITICAL: AI Agent Implementation Instructions

**This document is the SINGLE SOURCE OF TRUTH for E2E integration testing.**

### MANDATORY Requirements
1. **NO MOCK DATA** - All tests use REAL CLI execution, REAL database operations
2. **FAIL FAST** - Tests must fail immediately on any error with full context
3. **NO BACKWARDS COMPATIBILITY** - Broken code must error, not be masked
4. **PHYSICAL VERIFICATION REQUIRED** - Check actual database/file state after operations
5. **SYNTHETIC DATA WITH KNOWN OUTPUTS** - Use deterministic test data where you know expected results

---

## 1. Executive Summary

**What This Task Validates:**
- Complete session lifecycle: SessionStart → Tools → SessionEnd
- Memory capture and retrieval across sessions
- Context injection with relevant memories
- Divergence detection when topics shift
- Shell script execution exactly as Claude Code would invoke them

**Current State (VERIFIED 2026-01-18):**
- **Integration tests EXIST** at `crates/context-graph-cli/tests/integration/` - 15 tests
- **E2E tests EXIST** at `crates/context-graph-cli/tests/e2e/` - 20 tests
- **Hook shell scripts EXIST** at `.claude/hooks/` (5 scripts, all executable)
- **CLI is COMPLETE** with hooks, memory, session, and setup commands

---

## 2. Current Codebase State (ACTUAL)

### 2.1 CLI Command Structure (from main.rs)

```
context-graph-cli
├── session                         # Session persistence
│   ├── restore-identity           # Restore session from storage
│   └── persist-identity           # Persist session to storage
├── hooks                           # Claude Code native hooks
│   ├── session-start              # Initialize session (5000ms timeout)
│   ├── pre-tool                   # Fast path brief context (100ms timeout)
│   ├── post-tool                  # Memory capture (3000ms timeout)
│   ├── prompt-submit              # Context injection (2000ms timeout)
│   ├── session-end                # Persist and cleanup (30000ms timeout)
│   └── generate-config            # Generate hook config files
├── memory                          # Memory operations
│   ├── inject-context             # Full context injection (~1200 tokens)
│   ├── inject-brief               # Brief context (<200 tokens)
│   ├── capture-memory             # Capture HookDescription memory
│   └── capture-response           # Capture ClaudeResponse memory
└── setup                           # One-command project setup
```

### 2.2 Existing Test Files (COMPLETE)

| File | Purpose | Tests |
|------|---------|-------|
| `tests/e2e/full_session_test.rs` | Full session workflow via shell scripts | 8 tests |
| `tests/e2e/helpers.rs` | E2E helpers (shell execution, JSON generators) | 5 tests |
| `tests/e2e/error_recovery_test.rs` | Error handling scenarios | 7 tests |
| `tests/integration/hook_lifecycle_test.rs` | CLI binary lifecycle tests | 4 tests |
| `tests/integration/helpers.rs` | Integration helpers (CLI invocation) | Utilities |
| `tests/integration/timeout_test.rs` | Timeout compliance tests | 4 tests |
| `tests/integration/exit_code_test.rs` | Exit code validation | 7 tests |

### 2.3 Hook Shell Scripts (ACTUAL)

| Script | Location | Timeout | Function |
|--------|----------|---------|----------|
| `session_start.sh` | `.claude/hooks/` | 5000ms | Initialize session, load state |
| `pre_tool_use.sh` | `.claude/hooks/` | 100ms | FAST PATH - brief context only |
| `post_tool_use.sh` | `.claude/hooks/` | 3000ms | Capture tool description as memory |
| `user_prompt_submit.sh` | `.claude/hooks/` | 2000ms | Inject context for prompt |
| `session_end.sh` | `.claude/hooks/` | 30000ms | Persist state, run consolidation |

### 2.4 Exit Codes (AP-26)

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Timeout/corruption |
| 3 | Database error |
| 4 | Invalid input |
| 5 | Session not found |

---

## 3. Test Categories Required

### 3.1 Happy Path Tests (MUST PASS)

| Test ID | Description | Verification Method |
|---------|-------------|---------------------|
| E2E-HP-001 | Full session lifecycle via shell scripts | All 5 scripts return exit 0, JSON valid |
| E2E-HP-002 | Memory capture and retrieval | Captured content appears in inject output |
| E2E-HP-003 | Multi-session continuity | Session 2 retrieves Session 1 memories |
| E2E-HP-004 | Pre-tool fast path timing | Completes in <500ms (100ms + overhead) |
| E2E-HP-005 | Context injection relevance | Semantically similar memories ranked higher |

### 3.2 Edge Case Tests (MUST PASS)

| Test ID | Description | Expected Behavior |
|---------|-------------|-------------------|
| E2E-EC-001 | Empty stdin input | Exit code 4, JSON error on stderr |
| E2E-EC-002 | Invalid JSON input | Exit code 4, JSON error on stderr |
| E2E-EC-003 | Missing session_id | Exit code 1, clear error message |
| E2E-EC-004 | Very long prompt (10KB+) | Processes without truncation, within timeout |
| E2E-EC-005 | Special characters in prompt | No shell injection, proper JSON escaping |
| E2E-EC-006 | Unicode content | Handles UTF-8 correctly |
| E2E-EC-007 | CLI binary not found | Exit code 1 with "CLI not found" error |

### 3.3 Divergence Detection Tests

| Test ID | Description | Expected Behavior |
|---------|-------------|-------------------|
| E2E-DIV-001 | Topic shift detection | Divergence alert when switching topics |
| E2E-DIV-002 | Temporal similarity excluded | Time proximity doesn't trigger divergence |
| E2E-DIV-003 | Semantic similarity only | Only E1, E5, E6, E7, E10, E12, E13 trigger alerts |

---

## 4. Technical Specification

### 4.1 Test Architecture

**E2E Tests** (tests/e2e/):
- Execute REAL shell scripts exactly as Claude Code would
- Pipe JSON to stdin, capture stdout/stderr/exit code
- Verify timing budgets with wall-clock measurement
- Use tempfile for isolated database per test

**Integration Tests** (tests/integration/):
- Execute REAL CLI binary directly
- Test individual hooks without shell script layer
- Verify JSON output structure and content
- Test concurrent hook execution

### 4.2 Key Test Helpers (ACTUAL - from helpers.rs)

```rust
// E2E: Execute shell script with stdin JSON
pub fn execute_hook_script(
    script_name: &str,
    input_json: &str,
    timeout_ms: u64,
    db_path: &Path,
) -> Result<HookScriptResult, E2EError>

// Integration: Execute CLI binary directly
pub fn invoke_hook_with_stdin(
    hook_cmd: &str,
    session_id: &str,
    extra_args: &[&str],
    stdin_input: &str,
    db_path: &Path,
) -> HookInvocationResult

// Input generators
pub fn create_claude_code_session_start_input(session_id: &str) -> String
pub fn create_claude_code_pre_tool_input(session_id: &str, tool_name: &str, tool_input: Value) -> String
pub fn create_claude_code_post_tool_input(session_id: &str, tool_name: &str, tool_input: Value, tool_response: &str, success: bool) -> String
pub fn create_claude_code_prompt_submit_input(session_id: &str, prompt: &str) -> String
pub fn create_claude_code_session_end_input(session_id: &str, reason: &str) -> String
```

### 4.3 Synthetic Test Data

Use these deterministic inputs so you know expected outputs:

```rust
// Clustering topic - for testing semantic retrieval
const CLUSTERING_MEMORIES: [&str; 3] = [
    "Implemented HDBSCAN clustering with min_cluster_size=3",
    "Added BIRCH tree for incremental online clustering",
    "Configured EOM cluster selection method for HDBSCAN",
];

// Database topic - for testing divergence detection
const DATABASE_MEMORIES: [&str; 3] = [
    "Created PostgreSQL migration for users table",
    "Implemented RocksDB storage layer",
    "Configured ScyllaDB for production deployment",
];

// Code topic - for testing code similarity
const CODE_MEMORIES: [&str; 3] = [
    "impl Iterator for ClusterIterator",
    "fn calculate_silhouette_score(clusters: &[Cluster])",
    "struct HDBSCANParams { min_cluster_size: usize }",
];
```

**Expected Behavior:**
- Query "clustering algorithm" → returns CLUSTERING_MEMORIES
- Query "database migration" → returns DATABASE_MEMORIES
- Query "implement iterator" → returns CODE_MEMORIES
- Query "database" after storing CLUSTERING_MEMORIES → triggers divergence alert

---

## 5. Full State Verification Protocol (MANDATORY)

After every test operation, you MUST verify physical state.

### 5.1 Source of Truth Identification

| Operation | Source of Truth | Verification Method |
|-----------|-----------------|---------------------|
| Session start | CLI JSON output | Parse stdout, check `success: true` |
| Memory capture | Topic state in output | Check topic_state field exists |
| Context injection | stdout content | Verify relevant memories present |
| Session end | Persistence state | Check session summary in output |

### 5.2 Execute & Inspect Protocol

```bash
# Step 1: Build CLI
cargo build --package context-graph-cli

# Step 2: Run test with explicit temp directory
TEMP_DIR=$(mktemp -d)
SESSION_ID="test-e2e-$(uuidgen)"

# Step 3: Execute operation
echo '{"session_id":"'$SESSION_ID'"}' | ./.claude/hooks/session_start.sh
EXIT_CODE=$?

# Step 4: VERIFY exit code
echo "Exit code: $EXIT_CODE"  # Expected: 0

# Step 5: VERIFY JSON output is valid
echo "$OUTPUT" | jq .  # Must not error

# Step 6: VERIFY specific fields
echo "$OUTPUT" | jq '.success'  # Expected: true
echo "$OUTPUT" | jq '.session_id'  # Expected: matches input
```

### 5.3 Boundary & Edge Case Audit (3 Required)

**Edge Case 1: Empty Input**
```bash
# Before state
echo "Testing empty input..."

# Action
echo '' | ./.claude/hooks/session_start.sh 2>&1
EXIT_CODE=$?

# After state verification
echo "Exit code: $EXIT_CODE"  # Expected: 4
# Stderr should contain JSON error: {"success":false,"error":"Empty stdin"}
```

**Edge Case 2: Invalid JSON**
```bash
# Before state
echo "Testing invalid JSON..."

# Action
echo 'not valid json' | ./.claude/hooks/session_start.sh 2>&1
EXIT_CODE=$?

# After state verification
echo "Exit code: $EXIT_CODE"  # Expected: 4
# Stderr should contain JSON error
```

**Edge Case 3: Special Characters (Shell Injection Test)**
```bash
# Before state
echo "Testing special characters..."

# Action - attempting shell injection
INPUT='{"session_id":"test","prompt":"$(rm -rf /)","hook_event_name":"UserPromptSubmit"}'
echo "$INPUT" | ./.claude/hooks/user_prompt_submit.sh 2>&1
EXIT_CODE=$?

# After state verification
echo "Exit code: $EXIT_CODE"  # Expected: 0 (processed safely)
# No shell command execution should occur
```

### 5.4 Evidence of Success Log Template

```
=== E2E INTEGRATION TEST FSV VERIFICATION ===
Test: [test_name]
Session ID: [session_id]
Database Path: [temp_dir]
Test Time: [timestamp]

[Phase 1] Session Start
  Exit Code: 0 ✓
  JSON Valid: ✓
  success=true: ✓
  Execution Time: 45ms (budget: 5000ms) ✓

[Phase 2] Memory Capture
  Exit Code: 0 ✓
  Memory Stored: ✓
  Topic State Updated: ✓

[Phase 3] Context Injection
  Exit Code: 0 ✓
  Relevant Memories Found: ✓
  Divergence Alert (if applicable): ✓

[Phase 4] Session End
  Exit Code: 0 ✓
  Summary Generated: ✓
  State Persisted: ✓

[Edge Cases]
  Empty Input (exit 4): ✓
  Invalid JSON (exit 4): ✓
  Special Characters (safe): ✓

=== ALL VERIFICATIONS PASSED ===
```

---

## 6. Definition of Done

### 6.1 Functional Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| DOD-1 | All E2E tests pass | `cargo test --package context-graph-cli --test '*' -- --test-threads=1` |
| DOD-2 | Happy path tests (5) complete | All E2E-HP-* tests pass |
| DOD-3 | Edge case tests (7) complete | All E2E-EC-* tests pass |
| DOD-4 | Timing budgets verified | Pre-tool <500ms, all others within budget |
| DOD-5 | No mock data in any test | Code review confirms real CLI execution |
| DOD-6 | FSV evidence logged | Each test outputs verification log |

### 6.2 Test Coverage Requirements (ALL COMPLETE)

- Full session lifecycle: ✅ test_e2e_full_session_workflow
- Memory capture/retrieval: ✅ test_e2e_memory_capture_and_retrieval
- Divergence detection: ✅ test_e2e_hook_error_recovery
- Multi-session continuity: ✅ test_e2e_multi_session_continuity
- Error handling: ✅ 7 edge case tests in error_recovery_test.rs
- Special characters: ✅ test_e2e_special_character_safety (8 attack vectors)
- Unicode: ✅ test_e2e_unicode_handling (8 character sets)
- Long prompts: ✅ test_e2e_long_prompt_handling (10KB+)
- Timing: ✅ test_all_hooks_within_budget, test_pre_tool_use_completes_under_100ms

---

## 7. Implementation Checklist

### 7.1 Tests Implemented (ALL COMPLETE)

- [x] `test_e2e_memory_capture_and_retrieval` - Capture memory, query, verify presence
- [x] `test_e2e_hook_error_recovery` - Error recovery scenarios (includes divergence)
- [x] `test_e2e_multi_session_continuity` - Session 1 stores, Session 2 retrieves
- [x] `test_e2e_empty_stdin_error` - Empty stdin handling
- [x] `test_e2e_invalid_json_error` - Malformed JSON handling
- [x] `test_e2e_special_character_safety` - Shell injection prevention (8 attack vectors)
- [x] `test_e2e_unicode_handling` - UTF-8 content (emoji, CJK, Arabic, Cyrillic)
- [x] `test_e2e_long_prompt_handling` - 10KB+ prompts within timeout
- [x] `test_all_hooks_within_budget` - Timing compliance
- [x] `test_pre_tool_use_completes_under_100ms` - Fast path timing

### 7.2 Test Commands

```bash
# Build CLI (REQUIRED before tests)
cargo build --package context-graph-cli

# Run all E2E tests
cargo test --package context-graph-cli --test e2e_full_session_test -- --nocapture

# Run all integration tests
cargo test --package context-graph-cli --test hook_lifecycle_test -- --nocapture

# Run specific test
cargo test --package context-graph-cli test_e2e_full_session_workflow -- --nocapture

# Run with test isolation (single thread)
cargo test --package context-graph-cli -- --test-threads=1 --nocapture
```

---

## 8. Manual Testing Procedures

### 8.1 Full Session Lifecycle Manual Test

```bash
# Build CLI
cargo build --package context-graph-cli

# Create isolated test environment
TEMP_DIR=$(mktemp -d)
SESSION_ID="manual-test-$(date +%s)"
export CONTEXT_GRAPH_DB_PATH="$TEMP_DIR"

echo "=== Manual E2E Test ==="
echo "Session: $SESSION_ID"
echo "Database: $TEMP_DIR"

# 1. Session Start
echo "--- Session Start ---"
OUTPUT=$(echo '{"session_id":"'$SESSION_ID'","hook_event_name":"SessionStart"}' | ./.claude/hooks/session_start.sh 2>&1)
echo "Exit: $?"
echo "Output: $OUTPUT" | jq .

# 2. Capture Memory
echo "--- Post Tool (Memory Capture) ---"
OUTPUT=$(echo '{"session_id":"'$SESSION_ID'","tool_name":"Read","success":true,"hook_event_name":"PostToolUse"}' | ./.claude/hooks/post_tool_use.sh 2>&1)
echo "Exit: $?"
echo "Output: $OUTPUT" | jq .

# 3. Context Injection
echo "--- User Prompt Submit ---"
OUTPUT=$(echo '{"session_id":"'$SESSION_ID'","prompt":"test query","hook_event_name":"UserPromptSubmit"}' | ./.claude/hooks/user_prompt_submit.sh 2>&1)
echo "Exit: $?"
echo "Output: $OUTPUT" | jq .

# 4. Session End
echo "--- Session End ---"
OUTPUT=$(echo '{"session_id":"'$SESSION_ID'","reason":"normal","hook_event_name":"SessionEnd"}' | ./.claude/hooks/session_end.sh 2>&1)
echo "Exit: $?"
echo "Output: $OUTPUT" | jq .

# Cleanup
rm -rf "$TEMP_DIR"
echo "=== Manual Test Complete ==="
```

### 8.2 Edge Case Manual Tests

```bash
# Test 1: Empty input (all hooks)
for hook in session_start pre_tool_use post_tool_use user_prompt_submit session_end; do
    echo "Testing $hook with empty input:"
    echo '' | ./.claude/hooks/${hook}.sh 2>&1
    echo "Exit: $?"
done

# Test 2: Invalid JSON (all hooks)
for hook in session_start pre_tool_use post_tool_use user_prompt_submit session_end; do
    echo "Testing $hook with invalid JSON:"
    echo 'not json' | ./.claude/hooks/${hook}.sh 2>&1
    echo "Exit: $?"
done

# Test 3: Timing (pre_tool_use must be fast)
echo "Testing pre_tool_use timing:"
time (echo '{"session_id":"timing-test","tool_name":"Read","hook_event_name":"PreToolUse"}' | ./.claude/hooks/pre_tool_use.sh)
```

---

## 9. Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| TASK-P6-007 (Setup Command) | ✅ COMPLETE | Creates hook scripts |
| TASK-P6-008 (Hook Scripts) | ✅ COMPLETE | Scripts in .claude/hooks/ |
| TASK-P6-003 (Inject Context) | ✅ COMPLETE | memory inject-context |
| TASK-P6-005 (Capture Memory) | ✅ COMPLETE | memory capture-memory |

---

## 10. Constitution Compliance

| Rule | Compliance |
|------|------------|
| ARCH-07 | ✓ Tests use native Claude Code hooks |
| AP-14 | ✓ No .unwrap() in test helpers |
| AP-26 | ✓ Exit codes verified |
| AP-50 | ✓ Native hooks via settings.json |
| AP-53 | ✓ Hook logic in shell scripts |

---

## 11. Anti-Patterns (FORBIDDEN)

1. **NO MOCKS** - No MockStorage, MockEmbedder, FakeMemory
2. **NO STUBS** - verify_snapshot_exists must do real verification
3. **NO FALLBACKS** - If CLI not found, FAIL, don't skip test
4. **NO IGNORED FAILURES** - Every assertion must be meaningful
5. **NO HARDCODED PATHS** - Use tempfile for all test databases
6. **NO TIME-BASED ASSUMPTIONS** - Don't assume test order or timing

---

## 12. Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| "CLI binary not found" | Not built | `cargo build --package context-graph-cli` |
| "jq: command not found" | jq not installed | `apt install jq` or `brew install jq` |
| "Permission denied" | Scripts not executable | `chmod +x .claude/hooks/*.sh` |
| Test hangs | Timeout not working | Check timeout command availability |
| JSON parse error | Debug output mixing | Ensure tracing goes to stderr |

---

## 13. Related Files

| File | Purpose |
|------|---------|
| `crates/context-graph-cli/src/main.rs` | CLI entry point |
| `crates/context-graph-cli/src/commands/hooks/*.rs` | Hook handlers |
| `crates/context-graph-cli/src/commands/memory/*.rs` | Memory commands |
| `crates/context-graph-cli/tests/e2e/*.rs` | E2E tests |
| `crates/context-graph-cli/tests/integration/*.rs` | Integration tests |
| `.claude/hooks/*.sh` | Shell script executors |
| `.claude/settings.json` | Hook configuration |
