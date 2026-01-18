# Task: TASK-P6-005 - Capture Memory CLI Commands

## CRITICAL: AI Agent Implementation Instructions

**This task document is the SINGLE SOURCE OF TRUTH for implementing capture-memory and capture-response CLI commands.**

### MANDATORY Requirements for Implementation
1. **FAIL FAST** - No workarounds, no fallbacks. Errors must propagate immediately with full context.
2. **NO BACKWARDS COMPATIBILITY** - Break cleanly, don't mask failures.
3. **NO MOCK DATA IN TESTS** - Use real RocksDB, real MemoryCaptureService, real data flows.
4. **MANUAL VERIFICATION REQUIRED** - After implementation, you MUST verify data exists in RocksDB.

---

## 1. Executive Summary

**What This Task Creates:**
- Two new CLI subcommands under `memory`:
  - `memory capture-memory` - Capture hook descriptions from PostToolUse/SessionEnd
  - `memory capture-response` - Capture Claude responses from Stop hook

**Why It's Needed:**
- Currently, the CLI has `memory inject-context` and `memory inject-brief` for retrieval
- Memory CAPTURE from CLI does NOT exist yet
- The `MemoryCaptureService` in context-graph-core IS fully implemented and tested
- This task bridges CLI → core capture service

---

## 2. Current State Analysis (VERIFIED)

### 2.1 CLI Structure (ACTUAL - as of commit a05eae3)

```
crates/context-graph-cli/src/
├── main.rs                           # Entry, routes to Commands enum
├── error.rs                          # CliExitCode (0, 1, 2 per AP-26)
└── commands/
    ├── mod.rs                        # Exports session, hooks, memory
    ├── session/                      # restore-identity, persist-identity
    ├── hooks/                        # session_start, pre_tool_use, post_tool_use, etc.
    └── memory/
        ├── mod.rs                    # MemoryCommands enum (InjectContext, InjectBrief)
        └── inject.rs                 # handle_inject_context, handle_inject_brief
```

**CRITICAL: `commands/capture.rs` DOES NOT EXIST** - This task creates it.

### 2.2 Core Library (ACTUAL - fully implemented)

Location: `crates/context-graph-core/src/memory/`

**MemoryCaptureService** (capture.rs:124-312):
```rust
pub struct MemoryCaptureService {
    store: Arc<MemoryStore>,
    embedder: Arc<dyn EmbeddingProvider>,
}

impl MemoryCaptureService {
    pub fn new(store: Arc<MemoryStore>, embedder: Arc<dyn EmbeddingProvider>) -> Self;

    pub async fn capture_hook_description(
        &self,
        content: String,
        hook_type: HookType,
        session_id: String,
        tool_name: Option<String>,
    ) -> Result<Uuid, CaptureError>;

    pub async fn capture_claude_response(
        &self,
        content: String,
        response_type: ResponseType,
        session_id: String,
    ) -> Result<Uuid, CaptureError>;
}
```

**HookType** (source.rs:50-63) - 6 variants:
- `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `SessionEnd`

**ResponseType** (source.rs:69-76) - 3 variants:
- `SessionSummary`, `StopResponse`, `SignificantResponse`

**MemorySource** (source.rs:16-38) - 3 variants per ARCH-11:
- `HookDescription { hook_type, tool_name }`
- `ClaudeResponse { response_type }`
- `MDFileChunk { file_path, chunk_index, total_chunks }`

**CaptureError** (capture.rs:58-79):
- `EmptyContent`, `ContentTooLong { max, actual }`, `EmbeddingFailed`, `StorageFailed`, `ValidationFailed`

**MAX_CONTENT_LENGTH** = 10,000 characters (mod.rs)

### 2.3 What Already Works

| Component | Status | Location |
|-----------|--------|----------|
| MemoryCaptureService | ✅ COMPLETE | `context-graph-core/src/memory/capture.rs` |
| HookType enum (6 variants) | ✅ COMPLETE | `context-graph-core/src/memory/source.rs` |
| ResponseType enum (3 variants) | ✅ COMPLETE | `context-graph-core/src/memory/source.rs` |
| MemoryStore (RocksDB) | ✅ COMPLETE | `context-graph-core/src/memory/store.rs` |
| TestEmbeddingProvider | ✅ COMPLETE | `context-graph-core/src/memory/capture.rs` |
| memory inject-context | ✅ COMPLETE | `context-graph-cli/src/commands/memory/inject.rs` |
| memory inject-brief | ✅ COMPLETE | `context-graph-cli/src/commands/memory/inject.rs` |

### 2.4 What This Task Creates

| Component | Status | Location |
|-----------|--------|----------|
| capture-memory command | ❌ MISSING | `context-graph-cli/src/commands/memory/capture.rs` |
| capture-response command | ❌ MISSING | `context-graph-cli/src/commands/memory/capture.rs` |
| CaptureMemory in MemoryCommands | ❌ MISSING | `context-graph-cli/src/commands/memory/mod.rs` |
| CaptureResponse in MemoryCommands | ❌ MISSING | `context-graph-cli/src/commands/memory/mod.rs` |

---

## 3. Technical Specification

### 3.1 CLI Command Structure (Clap)

Add to `MemoryCommands` enum in `commands/memory/mod.rs`:

```rust
pub enum MemoryCommands {
    // Existing
    InjectContext(inject::InjectContextArgs),
    InjectBrief(inject::InjectBriefArgs),

    // NEW - This task
    CaptureMemory(capture::CaptureMemoryArgs),
    CaptureResponse(capture::CaptureResponseArgs),
}
```

### 3.2 Argument Structs

**CaptureMemoryArgs:**
```rust
#[derive(Parser)]
pub struct CaptureMemoryArgs {
    /// Content to capture (or use TOOL_DESCRIPTION/SESSION_SUMMARY env var)
    #[arg(long)]
    pub content: Option<String>,

    /// Memory source type: "hook" or "response"
    #[arg(long, default_value = "hook")]
    pub source: String,

    /// Session ID (or use CLAUDE_SESSION_ID env var)
    #[arg(long)]
    pub session_id: Option<String>,

    /// Hook type: session_start, user_prompt_submit, pre_tool_use, post_tool_use, stop, session_end
    #[arg(long)]
    pub hook_type: Option<String>,

    /// Tool name for PreToolUse/PostToolUse hooks
    #[arg(long)]
    pub tool_name: Option<String>,

    /// Path to RocksDB database
    #[arg(long, env = "CONTEXT_GRAPH_DATA_DIR")]
    pub db_path: Option<String>,
}
```

**CaptureResponseArgs:**
```rust
#[derive(Parser)]
pub struct CaptureResponseArgs {
    /// Response content to capture (or use RESPONSE_SUMMARY env var)
    #[arg(long)]
    pub content: Option<String>,

    /// Session ID (or use CLAUDE_SESSION_ID env var)
    #[arg(long)]
    pub session_id: Option<String>,

    /// Response type: session_summary, stop_response, significant_response
    #[arg(long, default_value = "stop_response")]
    pub response_type: Option<String>,

    /// Path to RocksDB database
    #[arg(long, env = "CONTEXT_GRAPH_DATA_DIR")]
    pub db_path: Option<String>,
}
```

### 3.3 Handler Functions

**handle_capture_memory:**
```rust
pub async fn handle_capture_memory(args: CaptureMemoryArgs) -> i32 {
    // 1. Get content from args or env (TOOL_DESCRIPTION, SESSION_SUMMARY)
    // 2. If empty/whitespace, return 0 (silent success per spec)
    // 3. Get session_id from args or env (CLAUDE_SESSION_ID) or use "default"
    // 4. Parse hook_type string to HookType enum
    // 5. Open MemoryStore at db_path
    // 6. Create embedding provider (TestEmbeddingProvider for Phase 1)
    // 7. Create MemoryCaptureService
    // 8. Call capture_hook_description()
    // 9. Return exit code per AP-26 (0=success, 1=error, 2=corruption)
}
```

**handle_capture_response:**
```rust
pub async fn handle_capture_response(args: CaptureResponseArgs) -> i32 {
    // 1. Get content from args or env (RESPONSE_SUMMARY)
    // 2. If empty/whitespace, return 0 (silent success)
    // 3. Get session_id from args or env or "default"
    // 4. Parse response_type string to ResponseType enum
    // 5. Open MemoryStore
    // 6. Create embedding provider
    // 7. Create MemoryCaptureService
    // 8. Call capture_claude_response()
    // 9. Return exit code per AP-26
}
```

### 3.4 Environment Variable Priority

| Env Var | CLI Flag | Used For |
|---------|----------|----------|
| `TOOL_DESCRIPTION` | `--content` | capture-memory content |
| `SESSION_SUMMARY` | `--content` | capture-memory content (fallback) |
| `RESPONSE_SUMMARY` | `--content` | capture-response content |
| `CLAUDE_SESSION_ID` | `--session-id` | Session identifier |
| `CONTEXT_GRAPH_DATA_DIR` | `--db-path` | RocksDB path |

Priority: CLI flag > Env var > Default

### 3.5 Exit Codes (AP-26)

| Exit Code | Meaning | When |
|-----------|---------|------|
| 0 | Success | Memory captured OR empty content (silent ignore) |
| 1 | Error | Capture failed (embedding, validation) |
| 2 | Corruption | Storage corruption detected |

### 3.6 Performance Constraints

| Operation | Budget | Hook Timeout |
|-----------|--------|--------------|
| capture-memory | <2700ms | PostToolUse: 3000ms |
| capture-response | <2700ms | Stop: 3000ms |

---

## 4. Files to Create/Modify

### 4.1 CREATE: `crates/context-graph-cli/src/commands/memory/capture.rs`

Full implementation with:
- `CaptureMemoryArgs` struct
- `CaptureResponseArgs` struct
- `handle_capture_memory()` async function
- `handle_capture_response()` async function
- `get_env_or_arg()` helper function
- `parse_hook_type()` helper function
- `parse_response_type()` helper function
- Comprehensive tests

### 4.2 MODIFY: `crates/context-graph-cli/src/commands/memory/mod.rs`

Add:
```rust
pub mod capture;

pub enum MemoryCommands {
    InjectContext(inject::InjectContextArgs),
    InjectBrief(inject::InjectBriefArgs),
    CaptureMemory(capture::CaptureMemoryArgs),      // NEW
    CaptureResponse(capture::CaptureResponseArgs),  // NEW
}

pub async fn handle_memory_command(cmd: MemoryCommands) -> i32 {
    match cmd {
        MemoryCommands::InjectContext(args) => inject::handle_inject_context(args).await,
        MemoryCommands::InjectBrief(args) => inject::handle_inject_brief(args).await,
        MemoryCommands::CaptureMemory(args) => capture::handle_capture_memory(args).await,
        MemoryCommands::CaptureResponse(args) => capture::handle_capture_response(args).await,
    }
}
```

---

## 5. Definition of Done

### 5.1 Functional Criteria

| ID | Criterion | Verification Method |
|----|-----------|---------------------|
| DOD-1 | `capture-memory` creates memory in RocksDB | Query `store.count()` before/after |
| DOD-2 | `capture-response` creates memory in RocksDB | Query `store.count()` before/after |
| DOD-3 | No stdout output on success | Verify stdout is empty |
| DOD-4 | TOOL_DESCRIPTION env var works | Set env, run command, verify capture |
| DOD-5 | RESPONSE_SUMMARY env var works | Set env, run command, verify capture |
| DOD-6 | Empty content returns exit 0 (silent) | Run with empty, check exit code |
| DOD-7 | Hook type parsing works for all 6 types | Test each variant |
| DOD-8 | Response type parsing works for all 3 types | Test each variant |

### 5.2 Exit Code Verification

| Scenario | Expected Exit | Verify |
|----------|---------------|--------|
| Valid capture | 0 | `echo $?` |
| Empty content | 0 | Silent success |
| Invalid hook_type | 1 | Error logged to stderr |
| Content too long | 1 | CaptureError::ContentTooLong |
| DB corruption | 2 | StorageError indicator |

---

## 6. Full State Verification (MANDATORY)

After implementing, you MUST perform these verification steps:

### 6.1 Source of Truth

**Primary:** RocksDB database at `CONTEXT_GRAPH_DATA_DIR` (default: `./data/memories`)
**Secondary:** Memory count via `MemoryStore::count()`
**Tertiary:** Memory retrieval via `MemoryStore::get(uuid)` and `MemoryStore::get_by_session(session_id)`

### 6.2 Execute & Inspect Protocol

```bash
# 1. Clean slate - note initial count
cargo run -p context-graph-cli -- memory inject-context "dummy" 2>/dev/null || true

# 2. Execute capture-memory
TOOL_DESCRIPTION="Test capture from PostToolUse" \
cargo run -p context-graph-cli -- memory capture-memory \
  --source hook \
  --hook-type post_tool_use \
  --tool-name "Edit" \
  --session-id "test-session-001"

# 3. Verify exit code
echo "Exit code: $?"  # Must be 0

# 4. MANDATORY: Verify data in RocksDB
# Create a verification script or use Rust test to:
# - Open MemoryStore at same path
# - Call store.get_by_session("test-session-001")
# - Assert memory exists with expected content
# - Assert memory.source is HookDescription with PostToolUse
```

### 6.3 Boundary & Edge Case Audit (3 Required)

**Edge Case 1: Empty Input**
```bash
# Input
TOOL_DESCRIPTION="" cargo run -p context-graph-cli -- memory capture-memory --source hook

# Expected
# - Exit code: 0
# - No new memory created
# - stderr: debug log "No content to capture"
```

**Edge Case 2: Maximum Length (10,000 chars)**
```bash
# Input
TOOL_DESCRIPTION="$(printf 'x%.0s' {1..10000})" \
cargo run -p context-graph-cli -- memory capture-memory --source hook

# Expected
# - Exit code: 0
# - Memory created with 10,000 char content
```

**Edge Case 3: Over Maximum Length (10,001 chars)**
```bash
# Input
TOOL_DESCRIPTION="$(printf 'x%.0s' {1..10001})" \
cargo run -p context-graph-cli -- memory capture-memory --source hook

# Expected
# - Exit code: 1
# - Error: "Content exceeds maximum length of 10000 characters: got 10001"
# - No memory created
```

### 6.4 Evidence of Success Log

After all tests pass, generate evidence showing:
```
=== CAPTURE MEMORY VERIFICATION ===
Database Path: ./data/memories
Initial Memory Count: 0

[Test 1] capture-memory with valid content
  Input: "Test hook description content"
  Hook Type: PostToolUse
  Tool Name: Edit
  Exit Code: 0
  Memory Count After: 1
  Memory UUID: <actual-uuid>
  Verified Content: "Test hook description content" ✓
  Verified Source: HookDescription(PostToolUse, tool=Edit) ✓

[Test 2] capture-response with valid content
  Input: "Session completed successfully"
  Response Type: SessionSummary
  Exit Code: 0
  Memory Count After: 2
  Memory UUID: <actual-uuid>
  Verified Content: "Session completed successfully" ✓
  Verified Source: ClaudeResponse(SessionSummary) ✓

[Edge Case 1] Empty input
  Exit Code: 0
  Memory Count After: 2 (unchanged) ✓

[Edge Case 2] Max length (10000 chars)
  Exit Code: 0
  Memory Count After: 3 ✓

[Edge Case 3] Over max length (10001 chars)
  Exit Code: 1
  Error: ContentTooLong ✓
  Memory Count After: 3 (unchanged) ✓

=== ALL VERIFICATIONS PASSED ===
```

---

## 7. Test Commands

### 7.1 Build & Unit Tests

```bash
# Build CLI
cargo build --package context-graph-cli

# Run capture module tests
cargo test commands::memory::capture --package context-graph-cli -- --nocapture

# Run all memory command tests
cargo test commands::memory --package context-graph-cli -- --nocapture
```

### 7.2 Integration Tests (CLI)

```bash
# Test capture-memory with content flag
./target/debug/context-graph-cli memory capture-memory \
  --content "Implemented HDBSCAN clustering" \
  --source hook \
  --hook-type post_tool_use \
  --tool-name "Edit"
echo "Exit: $?"

# Test capture-memory with env var
TOOL_DESCRIPTION="Refactored similarity module" \
./target/debug/context-graph-cli memory capture-memory --source hook --hook-type session_end

# Test capture-response
./target/debug/context-graph-cli memory capture-response \
  --content "Session completed with 5 tasks"
echo "Exit: $?"

# Test capture-response with env var
RESPONSE_SUMMARY="All tests passing" \
./target/debug/context-graph-cli memory capture-response
```

### 7.3 Validation Query (Rust)

```rust
// In test or verification script
use context_graph_core::memory::{MemoryStore, MemorySource, HookType};

let store = MemoryStore::new("./data/memories")?;
let memories = store.get_by_session("test-session-001")?;

for memory in &memories {
    println!("ID: {}", memory.id);
    println!("Content: {}", memory.content);
    println!("Source: {:?}", memory.source);
    println!("Session: {}", memory.session_id);
    println!("---");
}

assert!(!memories.is_empty(), "VERIFICATION FAILED: No memories found!");
```

---

## 8. Dependencies

### 8.1 Crate Dependencies (already present)

```toml
# context-graph-cli/Cargo.toml
[dependencies]
context-graph-core = { path = "../context-graph-core" }
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
uuid = { version = "1", features = ["v4"] }
```

### 8.2 Core Imports

```rust
use context_graph_core::memory::{
    CaptureError,
    HookType,
    MemoryCaptureService,
    MemoryStore,
    ResponseType,
    TestEmbeddingProvider,  // Phase 1 only
};
```

---

## 9. Constraints (MUST NOT VIOLATE)

| Constraint | Rule | Violation Consequence |
|------------|------|----------------------|
| ARCH-01 | TeleologicalArray is atomic (all 13 embeddings) | Compilation fails |
| ARCH-06 | All memory ops through service layer | Architecture violation |
| AP-14 | No `.unwrap()` in library/CLI code | Code review rejection |
| AP-26 | Exit codes: 0=success, 1=error, 2=corruption | Hook integration breaks |

---

## 10. Anti-Patterns (FORBIDDEN)

1. **NO MOCK DATA** - Tests must use real `MemoryStore` with temp directories
2. **NO FALLBACKS** - If capture fails, propagate error with full context
3. **NO SILENT FAILURES** - Log all errors to stderr before returning exit code
4. **NO PARTIAL STATES** - Memory either fully captures or doesn't exist
5. **NO STDOUT ON SUCCESS** - Capture commands are silent (hooks read stdout)

---

## 11. Implementation Checklist

- [x] Create `crates/context-graph-cli/src/commands/memory/capture.rs`
- [x] Define `CaptureMemoryArgs` with clap derive macros
- [x] Define `CaptureResponseArgs` with clap derive macros
- [x] Implement `resolve_content()` helper (renamed from `get_env_or_arg()`)
- [x] Implement `parse_hook_type()` with all 6 variants
- [x] Implement `parse_response_type()` with all 3 variants
- [x] Implement `handle_capture_memory()` async function
- [x] Implement `handle_capture_response()` async function
- [x] Add error-to-exit-code mapping per AP-26
- [x] Add `pub mod capture;` to `commands/memory/mod.rs`
- [x] Add `CaptureMemory` and `CaptureResponse` to `MemoryCommands` enum
- [x] Add match arms in `handle_memory_command()`
- [x] Write unit tests for argument parsing
- [x] Write unit tests for hook_type parsing
- [x] Write unit tests for response_type parsing
- [x] Write integration tests with real MemoryStore
- [x] Run `cargo build --package context-graph-cli`
- [x] Run `cargo test commands::memory::capture --package context-graph-cli`
- [x] Execute manual CLI tests
- [x] Verify memories exist in RocksDB
- [x] Generate evidence log (FSV test output)
- [x] Update this document with completion status

## 14. Implementation Notes (2026-01-17)

### Completion Status: ✅ COMPLETE

### Key Implementation Details

1. **MultiArrayEmbeddingAdapter**: Created an adapter struct that wraps `MultiArrayEmbeddingProvider` to implement the `EmbeddingProvider` trait required by `MemoryCaptureService`. This allows using `StubMultiArrayProvider` for Phase 1 without modifying the core library.

2. **Tests**: 30 comprehensive tests covering:
   - Hook type parsing (all 6 variants, case-insensitive, invalid input)
   - Response type parsing (all 3 variants, invalid input)
   - Content resolution (arg priority, env vars, fallback, whitespace handling)
   - Session ID resolution (arg, env, default)
   - Exit code values (AP-26 compliance)
   - Integration tests with real RocksDB
   - Edge cases (empty, max length, over max length, special characters)
   - Full State Verification (FSV) test

3. **Manual Verification Results**:
   - capture-memory with valid content: Exit 0 ✓
   - capture-response with valid content: Exit 0 ✓
   - Empty content: Exit 0 (silent success) ✓
   - Max length (10000 chars): Exit 0 ✓
   - Over max length (10001 chars): Exit 1 ✓
   - Invalid hook type: Exit 1 ✓
   - Env var reading (TOOL_DESCRIPTION, RESPONSE_SUMMARY): Works ✓
   - Special characters (Unicode, emoji, HTML): Preserved ✓

4. **RocksDB Verification**: Data confirmed to exist in RocksDB via FSV test and manual inspection of database files.

### Files Created/Modified

| File | Action | Lines |
|------|--------|-------|
| `crates/context-graph-cli/src/commands/memory/capture.rs` | Created | ~1100 |
| `crates/context-graph-cli/src/commands/memory/mod.rs` | Modified | +30 |
| `crates/context-graph-cli/Cargo.toml` | Modified | +3 (async-trait dep) |

---

## 12. Related Tasks

| Task ID | Title | Relationship |
|---------|-------|--------------|
| TASK-P6-001 | CLI Infrastructure | Dependency (provides main.rs, clap setup) |
| TASK-P6-006 | Capture Response Command | This task includes it |
| TASK-P1-007 | MemoryCaptureService | Dependency (core service) |
| TASK-P6-003 | Inject Context Command | Sibling (same module) |
| TASK-P6-004 | Inject Brief Command | Sibling (same module) |

---

## 13. Traceability

| Spec Item | This Task Covers |
|-----------|------------------|
| Command: capture-memory (cli_command) | ✅ handle_capture_memory |
| Command: capture-response (cli_command) | ✅ handle_capture_response |
| REQ-P6-07: Env var reading | ✅ TOOL_DESCRIPTION, RESPONSE_SUMMARY |
| AP-26: Exit codes | ✅ 0/1/2 mapping |
| ARCH-11: Memory sources | ✅ HookDescription, ClaudeResponse |
