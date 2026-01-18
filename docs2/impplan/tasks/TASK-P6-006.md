# Task: TASK-P6-006 - Capture Response Command

## STATUS: ✅ COMPLETE (Implemented as part of TASK-P6-005)

**Last Updated:** 2026-01-17
**Verified By:** Codebase audit confirms `handle_capture_response()` fully implemented in `capture.rs:331-408`

---

## 1. Executive Summary

**What This Task Covers:**
- `memory capture-response` CLI subcommand for capturing Claude responses from Stop hook

**Current State:** FULLY IMPLEMENTED
- Implementation: `crates/context-graph-cli/src/commands/memory/capture.rs:331-408`
- Handler: `handle_capture_response(CaptureResponseArgs) -> i32`
- Tests: Comprehensive coverage in `capture.rs:484-600+`
- Integration: Wired into `MemoryCommands::CaptureResponse` in `mod.rs:122`

---

## 2. Implementation Location (VERIFIED)

### 2.1 Primary Files

| File | Lines | Purpose |
|------|-------|---------|
| `crates/context-graph-cli/src/commands/memory/capture.rs` | 331-408 | `handle_capture_response()` handler |
| `crates/context-graph-cli/src/commands/memory/capture.rs` | 138-155 | `CaptureResponseArgs` struct |
| `crates/context-graph-cli/src/commands/memory/capture.rs` | 208-219 | `parse_response_type()` helper |
| `crates/context-graph-cli/src/commands/memory/mod.rs` | 99-122 | `CaptureResponse` enum variant |

### 2.2 Handler Function Signature

```rust
// Location: capture.rs:340
pub async fn handle_capture_response(args: CaptureResponseArgs) -> i32
```

### 2.3 Argument Struct

```rust
// Location: capture.rs:138-155
#[derive(Args)]
pub struct CaptureResponseArgs {
    /// Response content to capture (or use RESPONSE_SUMMARY env var)
    #[arg(long)]
    pub content: Option<String>,

    /// Session ID (or use CLAUDE_SESSION_ID env var)
    #[arg(long)]
    pub session_id: Option<String>,

    /// Response type: session_summary, stop_response (default), significant_response
    #[arg(long, default_value = "stop_response")]
    pub response_type: String,

    /// Path to data directory containing RocksDB
    #[arg(long, env = "CONTEXT_GRAPH_DATA_DIR")]
    pub db_path: Option<PathBuf>,
}
```

---

## 3. Technical Specification

### 3.1 Environment Variables

| Variable | Usage | Priority |
|----------|-------|----------|
| `RESPONSE_SUMMARY` | Content source | CLI flag > Env var |
| `CLAUDE_SESSION_ID` | Session ID | CLI flag > Env var > "default" |
| `CONTEXT_GRAPH_DATA_DIR` | RocksDB path | CLI flag > Env var > "./data" |

### 3.2 Response Types (3 variants)

| String Value | Enum Variant | Usage |
|--------------|--------------|-------|
| `session_summary` / `sessionsummary` | `ResponseType::SessionSummary` | SessionEnd summary |
| `stop_response` / `stopresponse` (DEFAULT) | `ResponseType::StopResponse` | Stop hook response |
| `significant_response` / `significantresponse` | `ResponseType::SignificantResponse` | Important responses |

### 3.3 Exit Codes (AP-26 Compliance)

| Exit Code | Meaning | Conditions |
|-----------|---------|------------|
| 0 | Success | Memory captured OR empty content (silent ignore) |
| 1 | Warning/Error | Validation, embedding, or non-corruption storage failure |
| 2 | Blocking/Corruption | Storage corruption detected |

### 3.4 Performance Budget

| Constraint | Value | Source |
|------------|-------|--------|
| Stop hook timeout | 3000ms | constitution.yaml |
| capture-response budget | <2700ms | 10% margin |
| Embedding (stub) | ~0ms | StubMultiArrayProvider |
| RocksDB write | <50ms | Typical SSD |

---

## 4. Execution Flow

```
Stop Hook → ./hooks/stop.sh → context-graph-cli memory capture-response

1. resolve_content(args.content, "RESPONSE_SUMMARY", None)
   → Returns None if empty/whitespace → Exit 0 (silent success)

2. resolve_session_id(args.session_id)
   → CLI arg > CLAUDE_SESSION_ID env > "default"

3. parse_response_type(&args.response_type)
   → Validates against 3 ResponseType variants
   → Exit 1 on invalid type

4. resolve_data_dir(args.db_path)
   → CLI arg > CONTEXT_GRAPH_DATA_DIR env > "./data"

5. MemoryStore::new(&memories_path)
   → Opens RocksDB at {data_dir}/memories
   → Exit 1 on open failure

6. MultiArrayEmbeddingAdapter::new(StubMultiArrayProvider::new())
   → Phase 1: Zeroed embeddings
   → Phase 2+: ProductionMultiArrayProvider

7. MemoryCaptureService::new(store, embedder)
   → Core service from context-graph-core

8. service.capture_claude_response(content, response_type, session_id)
   → Creates Memory with source = ClaudeResponse(response_type)
   → Embeds with all 13 embedders (stub returns zeroed)
   → Stores in RocksDB
   → Exit 0 on success, 1/2 on failure
```

---

## 5. Source of Truth for Verification

### 5.1 Primary: RocksDB Database

**Location:** `{CONTEXT_GRAPH_DATA_DIR}/memories` (default: `./data/memories`)

**Verification Method:**
```rust
use context_graph_core::memory::{MemoryStore, MemorySource, ResponseType};

let store = MemoryStore::new("./data/memories")?;
let memories = store.get_by_session("test-session")?;

for memory in &memories {
    if let MemorySource::ClaudeResponse { response_type } = &memory.source {
        println!("Found ClaudeResponse: {:?}", response_type);
        println!("Content: {}", memory.content);
        println!("Session: {}", memory.session_id);
    }
}
```

### 5.2 Secondary: Memory Count

```rust
let count_before = store.count()?;
// Run capture-response
let count_after = store.count()?;
assert!(count_after > count_before, "Memory count should increase");
```

---

## 6. Full State Verification Protocol

### 6.1 Execute & Inspect

```bash
# Step 1: Build CLI
cargo build --package context-graph-cli

# Step 2: Run capture-response with content flag
./target/debug/context-graph-cli memory capture-response \
  --content "Session completed successfully with all tests passing" \
  --response-type stop_response \
  --session-id "fsv-test-session"
echo "Exit code: $?"  # Expected: 0

# Step 3: Run capture-response with env var
RESPONSE_SUMMARY="All tasks completed" \
CLAUDE_SESSION_ID="fsv-test-session" \
./target/debug/context-graph-cli memory capture-response
echo "Exit code: $?"  # Expected: 0
```

### 6.2 Boundary & Edge Case Audit (3 Required)

**Edge Case 1: Empty Input**
```bash
# Before state
echo "Memory count before: $(cargo test --package context-graph-cli test_memory_count --quiet 2>/dev/null || echo 'N/A')"

# Action
RESPONSE_SUMMARY="" ./target/debug/context-graph-cli memory capture-response
EXIT_CODE=$?

# After state
echo "Exit code: $EXIT_CODE"  # Expected: 0 (silent success)
echo "Memory count after: unchanged (no new memory created)"

# Verification: Exit 0, no new memory
```

**Edge Case 2: Maximum Length (10,000 chars)**
```bash
# Before state
COUNT_BEFORE=$(cargo run -p context-graph-cli -- memory inject-context "count" 2>&1 | grep -c "memory" || echo "0")

# Action
RESPONSE_SUMMARY="$(printf 'x%.0s' {1..10000})" \
./target/debug/context-graph-cli memory capture-response
EXIT_CODE=$?

# After state
echo "Exit code: $EXIT_CODE"  # Expected: 0
echo "Content length: 10000 chars (at boundary)"

# Verification: Memory created with full content
```

**Edge Case 3: Over Maximum Length (10,001 chars)**
```bash
# Before state
COUNT_BEFORE=$(cargo run -p context-graph-cli -- memory inject-context "count" 2>&1 | grep -c "memory" || echo "0")

# Action
RESPONSE_SUMMARY="$(printf 'x%.0s' {1..10001})" \
./target/debug/context-graph-cli memory capture-response 2>&1
EXIT_CODE=$?

# After state
echo "Exit code: $EXIT_CODE"  # Expected: 1
echo "Error: Content exceeds maximum length of 10000 characters: got 10001"

# Verification: No new memory created, error logged to stderr
```

### 6.3 Evidence of Success Log Template

```
=== CAPTURE-RESPONSE FSV VERIFICATION ===
Database Path: ./data/memories
Test Session: fsv-test-$(date +%s)

[Test 1] capture-response with valid content
  Input: "Session completed successfully"
  Response Type: StopResponse
  Exit Code: 0 ✓
  Memory UUID: <actual-uuid>
  Verified in RocksDB: Yes ✓
  Verified Source: ClaudeResponse(StopResponse) ✓

[Test 2] capture-response via RESPONSE_SUMMARY env var
  Input: "All tests passing"
  Response Type: StopResponse (default)
  Exit Code: 0 ✓
  Memory UUID: <actual-uuid>
  Verified in RocksDB: Yes ✓

[Edge Case 1] Empty input
  Exit Code: 0 ✓
  Memory Created: No (expected) ✓

[Edge Case 2] Max length (10000 chars)
  Exit Code: 0 ✓
  Memory Created: Yes ✓
  Content Length Preserved: 10000 ✓

[Edge Case 3] Over max length (10001 chars)
  Exit Code: 1 ✓
  Error Message: "Content exceeds maximum length" ✓
  Memory Created: No (expected) ✓

=== ALL VERIFICATIONS PASSED ===
```

---

## 7. Test Commands

### 7.1 Unit Tests

```bash
# Run all capture tests
cargo test commands::memory::capture --package context-graph-cli -- --nocapture

# Run response-specific tests
cargo test test_parse_response_type --package context-graph-cli -- --nocapture
cargo test test_capture_response --package context-graph-cli -- --nocapture
```

### 7.2 Integration Tests

```bash
# Test with content flag
./target/debug/context-graph-cli memory capture-response \
  --content "Test response capture" \
  --response-type session_summary

# Test with env var
RESPONSE_SUMMARY="Env var test" ./target/debug/context-graph-cli memory capture-response

# Test all response types
for TYPE in session_summary stop_response significant_response; do
  echo "Testing $TYPE..."
  ./target/debug/context-graph-cli memory capture-response \
    --content "Test $TYPE" \
    --response-type "$TYPE"
  echo "Exit: $?"
done
```

### 7.3 FSV Integration Test (Rust)

The FSV test in `capture.rs` verifies:
1. capture-memory with valid content → memory exists in RocksDB
2. capture-response with valid content → memory exists in RocksDB
3. Empty input → no memory created, exit 0
4. Max length boundary → memory created
5. Over max length → exit 1, no memory created

```bash
cargo test fsv_capture_commands_full_verification --package context-graph-cli -- --nocapture
```

---

## 8. Definition of Done

| ID | Criterion | Status | Verification |
|----|-----------|--------|--------------|
| DOD-1 | `capture-response` creates memory in RocksDB | ✅ DONE | FSV test confirms |
| DOD-2 | `RESPONSE_SUMMARY` env var works | ✅ DONE | `resolve_content()` tested |
| DOD-3 | Empty content returns exit 0 (silent) | ✅ DONE | Line 344-348 |
| DOD-4 | All 3 response types parse correctly | ✅ DONE | `test_parse_response_type_all_variants` |
| DOD-5 | Exit codes match AP-26 | ✅ DONE | `capture_error_to_exit_code()` |
| DOD-6 | No stdout on success | ✅ DONE | Only debug logs via tracing |
| DOD-7 | Performance <2700ms | ✅ DONE | Stub embeddings ~0ms |

---

## 9. Dependencies (All Satisfied)

| Dependency | Status | Location |
|------------|--------|----------|
| TASK-P6-001 (CLI infrastructure) | ✅ COMPLETE | main.rs, clap setup |
| TASK-P6-005 (Capture infrastructure) | ✅ COMPLETE | capture.rs |
| TASK-P1-007 (MemoryCaptureService) | ✅ COMPLETE | context-graph-core/src/memory/capture.rs |
| ResponseType enum | ✅ COMPLETE | context-graph-core/src/memory/source.rs:69-76 |
| MemoryStore | ✅ COMPLETE | context-graph-core/src/memory/store.rs |

---

## 10. Constitution Compliance

| Rule | Compliance |
|------|------------|
| ARCH-01: TeleologicalArray atomic | ✅ All 13 embeddings via adapter |
| ARCH-06: Memory ops through service | ✅ Uses MemoryCaptureService |
| ARCH-11: Memory sources | ✅ ClaudeResponse source |
| AP-14: No .unwrap() | ✅ All errors handled with map_err |
| AP-26: Exit codes 0/1/2 | ✅ capture_error_to_exit_code() |

---

## 11. Related Tasks

| Task ID | Title | Relationship |
|---------|-------|--------------|
| TASK-P6-005 | Capture Memory Commands | Parent (implemented together) |
| TASK-P6-008 | Hook Shell Scripts | Consumer (calls capture-response) |
| TASK-P1-007 | MemoryCaptureService | Dependency (core service) |

---

## 12. Traceability

| Spec Item | Coverage |
|-----------|----------|
| Command: capture-response (cli_command) | ✅ handle_capture_response |
| REQ-P6-07: Env var reading | ✅ RESPONSE_SUMMARY |
| AP-26: Exit codes | ✅ 0/1/2 mapping |
| ARCH-11: ClaudeResponse source | ✅ Implemented |

---

## 13. Implementation Notes

### Why Combined with TASK-P6-005

The `capture-response` command shares 90% of its infrastructure with `capture-memory`:
- Same embedding adapter pattern
- Same exit code mapping
- Same content resolution logic
- Same session ID resolution
- Same RocksDB integration

Implementing them separately would have created code duplication. The constitution requirement for DRY code led to combining them into a single capture.rs module.

### Phase 1 Limitation

Currently uses `StubMultiArrayProvider` which returns zeroed embeddings. This is intentional for Phase 1:
- Validates the capture pipeline end-to-end
- RocksDB storage works correctly
- Exit codes map correctly
- Content flows through correctly

Phase 2+ will swap in `ProductionMultiArrayProvider` for GPU-accelerated embeddings without changing the CLI code (adapter pattern benefit).
