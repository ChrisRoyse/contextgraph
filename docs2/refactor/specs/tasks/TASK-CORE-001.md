# TASK-CORE-001: Remove Manual North Star Protocol Constants and Handlers

```xml
<task_spec id="TASK-CORE-001" version="3.0">
<metadata>
  <title>Remove Manual North Star Protocol Constants, Handlers, and Dispatch Routes</title>
  <status>completed</status>
  <completed_date>2026-01-09</completed_date>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <requirement_ref>REQ-NORTHSTAR-REMOVE-01</requirement_ref>
    <requirement_ref>ARCH-02: Apples-to-apples comparison only</requirement_ref>
    <requirement_ref>ARCH-03: Autonomous-first (no manual goals)</requirement_ref>
  </implements>
  <depends_on><!-- None - first task --></depends_on>
  <blocks>
    <task_ref>TASK-CORE-005</task_ref><!-- GoalNode structure changes -->
    <task_ref>TASK-CORE-009</task_ref><!-- Remove projection code -->
  </blocks>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<completion_summary>
## What Was Done

This task was completed on 2026-01-09. The following changes were made:

### Files Modified
1. **protocol.rs**: Removed `PURPOSE_NORTH_STAR_ALIGNMENT` and `NORTH_STAR_UPDATE` constants, added `DEPRECATED_METHOD` error code
2. **handlers/core.rs**: Removed dispatch routes, added deprecation comments
3. **handlers/purpose.rs**: Removed `handle_north_star_alignment()` (~180 lines) and `handle_north_star_update()` (~180 lines)
4. **handlers/tools.rs**: Updated tool descriptions
5. **Test files updated**:
   - `tests/north_star.rs`: Converted to deprecation tests
   - `tests/purpose.rs`: Removed references to deprecated methods
   - `tests/full_state_verification_purpose.rs`: Updated FSV tests
   - `tests/manual_fsv_purpose.rs`: Updated manual tests
   - `tests/integration_e2e.rs`: Updated E2E tests

### Verification Status
- **Compilation**: `cargo check -p context-graph-mcp` passes with only unrelated warnings
- **Tests**: All 367 tests pass (`cargo test -p context-graph-mcp`)
- **Grep verification**: No functional references to deprecated constants/handlers remain
</completion_summary>

<context>
## Why This Task Existed

The manual North Star system was architecturally broken. It created single 1024D embeddings that
CANNOT be meaningfully compared to 13-embedder teleological arrays.

### The Mathematical Flaw (from constitution.yaml and 05-NORTH-STAR-REMOVAL.md)

**Manual North Star**: ONE vector (1024D from text-embedding-3-large)

**Teleological Array**: 13 DIFFERENT vectors from 13 DIFFERENT models:
- E1: 1024D semantic (meaning)
- E2-E4: 512D temporal (time patterns)
- E5: 768D causal (ASYMMETRIC cause-effect)
- E6: ~30K sparse (selective activation)
- E7: 1536D code (AST structure)
- E8: 384D graph (connectivity)
- E9: 10K-bit binary (holographic robustness)
- E10: 768D multimodal (cross-modal binding)
- E11: 384D entity (factual grounding)
- E12: 128D/token (late interaction precision)
- E13: ~30K sparse SPLADE (keyword precision)

Comparing a single 1024D vector to these 13 different embeddings via projection is
"apples-to-oranges" and produces meaningless alignment scores.

### The Replacement System

The autonomous system uses `auto_bootstrap_north_star` tool (defined in tools.rs) which:
- Discovers purpose from stored teleological fingerprints
- Works with full 13-embedder arrays
- Never creates single embeddings for goals
- Returns `NORTH_STAR_NOT_CONFIGURED` error (-32021) when no autonomous goal discovered yet
</context>

<final_state>
## Current Codebase State (Verified 2026-01-09)

### Protocol Constants (protocol.rs)

**Line 221**: `DEPRECATED_METHOD` error code added:
```rust
pub const DEPRECATED_METHOD: i32 = -32601;  // Same as METHOD_NOT_FOUND per JSON-RPC spec
```

**Lines 274-287**: Deprecation comments where constants were removed:
```rust
// NOTE: PURPOSE_NORTH_STAR_ALIGNMENT removed per TASK-CORE-001 (ARCH-03)
// Manual North Star creates single 1024D embeddings incompatible with 13-embedder arrays.
// Use auto_bootstrap_north_star tool for autonomous goal discovery instead.
```

**Preserved constants** (still exist):
- `PURPOSE_QUERY: &str = "purpose/query"` (line 278)
- `GOAL_HIERARCHY_QUERY: &str = "goal/hierarchy_query"` (line 280)
- `GOAL_ALIGNED_MEMORIES: &str = "goal/aligned_memories"` (line 282)
- `PURPOSE_DRIFT_CHECK: &str = "purpose/drift_check"` (line 284) - for TASK-LOGIC-010

### Dispatch Routes (handlers/core.rs)

**Lines 779-782**: Deprecation comment block added:
```rust
// NOTE: PURPOSE_NORTH_STAR_ALIGNMENT and NORTH_STAR_UPDATE removed per TASK-CORE-001 (ARCH-03)
// These methods now fall through to the default case returning METHOD_NOT_FOUND (-32601)
// Use auto_bootstrap_north_star tool for autonomous goal discovery instead.
```

Deprecated method strings now fall through to the `_ =>` default match arm, returning
`METHOD_NOT_FOUND (-32601)`.

### Handler Functions (handlers/purpose.rs)

**Lines 221-223**: Stub comment where `handle_north_star_alignment` was removed:
```rust
// NOTE: handle_north_star_alignment REMOVED per TASK-CORE-001 (ARCH-03)
// Computed alignment using broken projection - apples-to-oranges comparison.
// Use auto_bootstrap_north_star tool for autonomous goal discovery.
```

**Lines 844-846**: Stub comment where `handle_north_star_update` was removed:
```rust
// NOTE: handle_north_star_update REMOVED per TASK-CORE-001 (ARCH-03)
// Created manual 1024D embeddings violating ARCH-03 autonomous-first.
// Use auto_bootstrap_north_star tool for autonomous goal discovery.
```

**Preserved handlers** (still exist):
- `handle_purpose_query()` - Works with 13D purpose vectors
- `handle_goal_hierarchy_query()` - Works with goal hierarchy
- `handle_goal_aligned_memories()` - Works with hierarchy
- `handle_purpose_drift_check()` - To be refactored in TASK-LOGIC-010
</final_state>

<verification_evidence>
## Verification Commands Run

### 1. Constant Removal Verification
```bash
$ rg "PURPOSE_NORTH_STAR_ALIGNMENT|NORTH_STAR_UPDATE" --type rust crates/
crates/context-graph-mcp/src/protocol.rs:274:    // NOTE: PURPOSE_NORTH_STAR_ALIGNMENT removed...
crates/context-graph-mcp/src/protocol.rs:285:    // NOTE: NORTH_STAR_UPDATE removed...
crates/context-graph-mcp/src/handlers/core.rs:780:    // NOTE: PURPOSE_NORTH_STAR_ALIGNMENT and NORTH_STAR_UPDATE removed...
```
**Result**: Only comments remain, no functional code.

### 2. Handler Removal Verification
```bash
$ rg "handle_north_star_alignment|handle_north_star_update" --type rust crates/
crates/context-graph-mcp/src/handlers/purpose.rs:221:    // NOTE: handle_north_star_alignment REMOVED...
crates/context-graph-mcp/src/handlers/purpose.rs:844:    // NOTE: handle_north_star_update REMOVED...
```
**Result**: Only stub comments remain, no function definitions.

### 3. DEPRECATED_METHOD Verification
```bash
$ rg "DEPRECATED_METHOD" --type rust crates/context-graph-mcp/src/protocol.rs
    pub const DEPRECATED_METHOD: i32 = -32601;
```
**Result**: Error code properly added.

### 4. Compilation Verification
```bash
$ cargo check -p context-graph-mcp
    Checking context-graph-mcp v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in X.XXs
```
**Result**: Compiles with no errors (only unrelated warnings).

### 5. Test Verification
```bash
$ cargo test -p context-graph-mcp
test result: ok. 367 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
**Result**: All tests pass.
</verification_evidence>

<full_state_verification>
## Full State Verification (FSV) Protocol

This section documents the FSV performed after completion.

### Source of Truth Definition

| Component | Source of Truth | Location |
|-----------|-----------------|----------|
| Protocol Constants | Actual constant definitions | `protocol.rs` lines 260-290 |
| Dispatch Routes | Match arm presence | `core.rs` lines 770-820 |
| Handler Functions | Function definitions | `purpose.rs` full file |
| Error Codes | Error code constant | `protocol.rs` error_codes module |

### Execute & Inspect Results

**Inspection 1: Protocol Constants**
- Verified `PURPOSE_NORTH_STAR_ALIGNMENT` constant does NOT exist as code
- Verified `NORTH_STAR_UPDATE` constant does NOT exist as code
- Verified `DEPRECATED_METHOD = -32601` DOES exist at line 221
- Verified `PURPOSE_DRIFT_CHECK` preserved at line 284

**Inspection 2: Dispatch Routes**
- Verified no match arm for `methods::PURPOSE_NORTH_STAR_ALIGNMENT`
- Verified no match arm for `methods::NORTH_STAR_UPDATE`
- Verified these method strings fall through to default `_ =>` case

**Inspection 3: Handler Functions**
- Verified `handle_north_star_alignment()` function removed (was ~180 lines)
- Verified `handle_north_star_update()` function removed (was ~180 lines)
- Verified `handle_purpose_query()` preserved and functional
- Verified `handle_purpose_drift_check()` preserved for TASK-LOGIC-010

### Boundary & Edge Case Audit

#### Edge Case 1: Client calls deprecated `purpose/north_star_alignment`

**Before removal**:
```json
Request: {"jsonrpc":"2.0","id":1,"method":"purpose/north_star_alignment","params":{"fingerprint_id":"..."}}
Response: {"jsonrpc":"2.0","id":1,"result":{"alignment":0.75,"threshold":"Acceptable",...}}
```

**After removal**:
```json
Request: {"jsonrpc":"2.0","id":1,"method":"purpose/north_star_alignment","params":{"fingerprint_id":"..."}}
Response: {"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}
```

**Test verification**: `tests/north_star.rs::test_deprecated_north_star_alignment_returns_method_not_found`

#### Edge Case 2: Client calls deprecated `purpose/north_star_update`

**Before removal**:
```json
Request: {"jsonrpc":"2.0","id":2,"method":"purpose/north_star_update","params":{"description":"My goal"}}
Response: {"jsonrpc":"2.0","id":2,"result":{"north_star_id":"uuid",...}}
```

**After removal**:
```json
Request: {"jsonrpc":"2.0","id":2,"method":"purpose/north_star_update","params":{"description":"My goal"}}
Response: {"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found"}}
```

**Test verification**: `tests/north_star.rs::test_deprecated_north_star_update_returns_method_not_found`

#### Edge Case 3: Code references removed constants

**Before**: Code using `methods::PURPOSE_NORTH_STAR_ALIGNMENT` would compile but use broken functionality.

**After**: Code referencing removed constant fails compilation with:
```
error[E0599]: no associated item named `PURPOSE_NORTH_STAR_ALIGNMENT` found
```

**Verification**: Compilation succeeds because all references removed.

### Evidence of Success

**1. Grep output showing no functional matches**:
```bash
$ rg "pub const PURPOSE_NORTH_STAR_ALIGNMENT" --type rust crates/
(no output - constant removed)

$ rg "pub const NORTH_STAR_UPDATE" --type rust crates/
(no output - constant removed)

$ rg "async fn handle_north_star_alignment" --type rust crates/
(no output - handler removed)

$ rg "async fn handle_north_star_update" --type rust crates/
(no output - handler removed)
```

**2. Error code verification**:
```bash
$ rg "pub const DEPRECATED_METHOD: i32 = -32601" --type rust crates/
crates/context-graph-mcp/src/protocol.rs:pub const DEPRECATED_METHOD: i32 = -32601;
```

**3. Test pass evidence**:
```
test result: ok. 367 passed; 0 failed; 0 ignored
```
</full_state_verification>

<error_handling_policy>
## Error Handling Policy (NO FALLBACKS)

This task followed the project requirement: **ABSOLUTELY NO BACKWARDS COMPATIBILITY**.

### What Was NOT Done (Forbidden Patterns)

```rust
// BAD - Fallback to autonomous system (NOT IMPLEMENTED)
methods::PURPOSE_NORTH_STAR_ALIGNMENT => {
    self.call_auto_bootstrap_north_star(request.id, None).await
}

// BAD - Wrapper returning deprecation warning but still working (NOT IMPLEMENTED)
methods::NORTH_STAR_UPDATE => {
    warn!("Deprecated method called");
    self.handle_north_star_update(request.id, request.params).await  // Still works
}

// BAD - Silent no-op (NOT IMPLEMENTED)
methods::PURPOSE_NORTH_STAR_ALIGNMENT => {
    JsonRpcResponse::success(id, json!({"deprecated": true}))
}
```

### What WAS Done (Correct Pattern)

```rust
// GOOD - No match arm at all, falls through to default
// In the match statement, there is simply no entry for:
//   - methods::PURPOSE_NORTH_STAR_ALIGNMENT
//   - methods::NORTH_STAR_UPDATE
//
// The default case handles unknown methods:
_ => JsonRpcResponse::error(
    id,
    error_codes::METHOD_NOT_FOUND,  // -32601
    format!("Unknown method: {}", method),
)
```

### Error Propagation

When clients call deprecated methods:
1. Request arrives at `handle_request()` in `core.rs`
2. Method string does NOT match any case in the match statement
3. Falls through to `_ =>` default case
4. Returns `METHOD_NOT_FOUND (-32601)` error
5. Error propagated back to client immediately
6. No retry, no fallback, no workaround
</error_handling_policy>

<related_tasks>
## Related Tasks

### Blocked Tasks (Can Now Proceed)
- **TASK-CORE-005**: GoalNode structure changes - Depends on this task removing the manual North Star
- **TASK-CORE-009**: Remove projection code - Related cleanup for dimension projection

### Uses This Work
- **TASK-LOGIC-010**: Drift Detection refactoring - Will update `PURPOSE_DRIFT_CHECK` to use teleological arrays

### Autonomous System (Already Exists)
- **auto_bootstrap_north_star tool** (tools.rs): Discovers purpose from stored teleological fingerprints
- **handlers/autonomous.rs**: Implements autonomous goal discovery
- **NORTH_STAR_NOT_CONFIGURED (-32021)**: Error code for when no autonomous goal discovered yet
</related_tasks>

<rollback_plan>
## Rollback (If Ever Needed)

**WARNING**: Rollback would reintroduce the architectural flaw. Only use if critical regression discovered.

```bash
git checkout HEAD~1 -- crates/context-graph-mcp/src/protocol.rs
git checkout HEAD~1 -- crates/context-graph-mcp/src/handlers/core.rs
git checkout HEAD~1 -- crates/context-graph-mcp/src/handlers/purpose.rs
git checkout HEAD~1 -- crates/context-graph-mcp/src/handlers/tests/
```
</rollback_plan>

<related_documentation>
## Reference Documentation

- `docs2/constitution.yaml` - ARCH-02, ARCH-03, AP-01 requirements
- `docs2/refactor/05-NORTH-STAR-REMOVAL.md` - Full rationale for removal
- `docs2/refactor/specs/functional/north_star_removal.md` - Functional spec
- `crates/context-graph-mcp/src/handlers/autonomous.rs` - Replacement autonomous system
</related_documentation>
</task_spec>
```
