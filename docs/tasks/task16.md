# Task 16: Integration Testing and Verification

## Metadata
- **Task ID**: TASK-GAP-016
- **Phase**: 4 (Integration)
- **Priority**: Critical
- **Complexity**: Medium
- **Estimated Time**: 2 hours
- **Dependencies**: task14 (TASK-GAP-014), task15 (TASK-GAP-015) - all previous tasks must be complete

## Objective

Perform comprehensive integration testing to verify all PRD gaps have been closed. This includes running the full test suite, verifying hooks, skills, and MCP tools, and updating the gap analysis document with completion status.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/PRD_GAP_ANALYSIS.md` - Original gap analysis
- `/home/cabdru/contextgraph/docs/ATOMIC_TASKS_PRD_GAPS.md` - Task tracking
- All files modified in tasks 01-15

## Files to Create/Modify

**Files to Modify:**
- `/home/cabdru/contextgraph/docs/ATOMIC_TASKS_PRD_GAPS.md` - Update task status

## Implementation Steps

### Step 1: Run Full Test Suite

```bash
cd /home/cabdru/contextgraph

# Run all workspace tests
cargo test --workspace

# If any tests fail, investigate and document
```

### Step 2: Run Clippy

```bash
# Run clippy with warnings as errors
cargo clippy --workspace -- -D warnings

# Fix any clippy warnings if present
```

### Step 3: Verify Hooks

```bash
# Count hooks in settings.json (should be 6)
jq '.hooks | keys | length' .claude/settings.json
# Expected: 6

# List all hook names
jq -r '.hooks | keys[]' .claude/settings.json
# Expected: SessionStart, SessionEnd, PreToolUse, PostToolUse, UserPromptSubmit, Stop

# Verify all hook scripts exist and are executable
for hook in session_start session_end pre_tool_use post_tool_use user_prompt_submit stop; do
    test -x ".claude/hooks/${hook}.sh" && echo "${hook}.sh: OK" || echo "${hook}.sh: MISSING"
done
```

### Step 4: Verify Skills

```bash
# List all skill directories (should be 5)
ls -d .claude/skills/*/
# Expected: topic-explorer, memory-inject, semantic-search, dream-consolidation, curation

# Verify all SKILL.md files exist
for skill in topic-explorer memory-inject semantic-search dream-consolidation curation; do
    test -f ".claude/skills/${skill}/SKILL.md" && echo "${skill}/SKILL.md: OK" || echo "${skill}/SKILL.md: MISSING"
done

# Verify all skills have correct model in frontmatter
for skill in topic-explorer memory-inject semantic-search dream-consolidation curation; do
    echo -n "${skill}: "
    head -3 ".claude/skills/${skill}/SKILL.md" | grep "model:"
done
```

### Step 5: Verify MCP Tools

```bash
# Verify tool name constants (should be 12)
grep "^pub const" crates/context-graph-mcp/src/tools/names.rs | wc -l
# Expected: 12

# Verify dispatch handles all tools
grep "tool_names::" crates/context-graph-mcp/src/handlers/tools/dispatch.rs | wc -l
# Expected: 12

# Verify tool definitions exist
grep -c "ToolDefinition {" crates/context-graph-mcp/src/tools/definitions.rs
# Expected: 12
```

### Step 6: Build Release

```bash
# Build release binary
cargo build --release

# Verify binary exists
test -f target/release/context-graph-mcp && echo "MCP binary: OK"
test -f target/release/context-graph-cli && echo "CLI binary: OK"
```

### Step 7: Update Gap Analysis

Update the ATOMIC_TASKS_PRD_GAPS.md file to mark all tasks as complete.

## Code/Content to Implement

### Verification Checklist

Create a verification checklist document or update the task tracking document:

```markdown
## PRD Gap Remediation - Final Verification

### Test Suite
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo build --release` succeeds

### Hooks (6 total)
- [ ] SessionStart hook configured
- [ ] SessionEnd hook configured
- [ ] PreToolUse hook configured
- [ ] PostToolUse hook configured
- [ ] UserPromptSubmit hook configured
- [ ] Stop hook configured
- [ ] All 6 hook scripts exist and are executable

### Skills (5 total)
- [ ] topic-explorer/SKILL.md exists (model: sonnet)
- [ ] memory-inject/SKILL.md exists (model: haiku)
- [ ] semantic-search/SKILL.md exists (model: haiku)
- [ ] dream-consolidation/SKILL.md exists (model: sonnet)
- [ ] curation/SKILL.md exists (model: sonnet)

### MCP Tools (12 total)
- [ ] inject_context
- [ ] store_memory
- [ ] search_graph
- [ ] get_memetic_status
- [ ] trigger_consolidation
- [ ] get_topic_portfolio
- [ ] get_topic_stability
- [ ] detect_topics
- [ ] get_divergence_alerts
- [ ] merge_concepts
- [ ] forget_concept
- [ ] boost_importance

### Documentation
- [ ] CLAUDE.md MCP section accurate (12 tools documented)
- [ ] No references to non-existent tools
```

## Definition of Done

- [ ] `cargo test --workspace` passes with no failures
- [ ] `cargo clippy --workspace -- -D warnings` passes with no warnings
- [ ] `cargo build --release` succeeds
- [ ] 6 hooks configured in settings.json
- [ ] 6 hook scripts exist in .claude/hooks/
- [ ] 5 skill SKILL.md files exist in .claude/skills/
- [ ] 12 MCP tools defined and dispatched
- [ ] CLAUDE.md accurately documents 12 tools
- [ ] All task status updated in ATOMIC_TASKS_PRD_GAPS.md

## Verification

Run the complete verification script:

```bash
#!/bin/bash
# PRD Gap Remediation Verification Script

cd /home/cabdru/contextgraph

echo "=== PRD Gap Remediation Verification ==="
echo ""

# Test Suite
echo "1. Running test suite..."
if cargo test --workspace 2>&1 | tail -5; then
    echo "   Test suite: PASS"
else
    echo "   Test suite: FAIL"
fi
echo ""

# Clippy
echo "2. Running clippy..."
if cargo clippy --workspace -- -D warnings 2>&1 | tail -3; then
    echo "   Clippy: PASS"
else
    echo "   Clippy: FAIL"
fi
echo ""

# Hooks
echo "3. Verifying hooks..."
HOOK_COUNT=$(jq '.hooks | keys | length' .claude/settings.json)
echo "   Hooks configured: $HOOK_COUNT (expected: 6)"

for hook in session_start session_end pre_tool_use post_tool_use user_prompt_submit stop; do
    if test -x ".claude/hooks/${hook}.sh"; then
        echo "   ${hook}.sh: OK"
    else
        echo "   ${hook}.sh: MISSING"
    fi
done
echo ""

# Skills
echo "4. Verifying skills..."
for skill in topic-explorer memory-inject semantic-search dream-consolidation curation; do
    if test -f ".claude/skills/${skill}/SKILL.md"; then
        MODEL=$(head -5 ".claude/skills/${skill}/SKILL.md" | grep "model:" | awk '{print $2}')
        echo "   ${skill}: OK (model: $MODEL)"
    else
        echo "   ${skill}: MISSING"
    fi
done
echo ""

# MCP Tools
echo "5. Verifying MCP tools..."
TOOL_COUNT=$(grep "^pub const" crates/context-graph-mcp/src/tools/names.rs | wc -l)
echo "   Tool constants: $TOOL_COUNT (expected: 12)"

DISPATCH_COUNT=$(grep "tool_names::" crates/context-graph-mcp/src/handlers/tools/dispatch.rs | wc -l)
echo "   Dispatch arms: $DISPATCH_COUNT (expected: 12)"
echo ""

# Build
echo "6. Building release..."
if cargo build --release 2>&1 | tail -3; then
    echo "   Build: PASS"
else
    echo "   Build: FAIL"
fi
echo ""

echo "=== Verification Complete ==="
```

Save as `verify_prd_gaps.sh` and run:

```bash
chmod +x verify_prd_gaps.sh
./verify_prd_gaps.sh
```
