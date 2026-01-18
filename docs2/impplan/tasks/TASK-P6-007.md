# Task: TASK-P6-007 - Setup Command

## CRITICAL: AI Agent Implementation Instructions

**This task document is the SINGLE SOURCE OF TRUTH for implementing the `setup` CLI command.**

### MANDATORY Requirements for Implementation
1. **FAIL FAST** - No workarounds, no fallbacks. Errors must propagate immediately with full context.
2. **NO BACKWARDS COMPATIBILITY** - Break cleanly, don't mask failures.
3. **NO MOCK DATA IN TESTS** - Use real file system, real file creation, real permissions.
4. **MANUAL VERIFICATION REQUIRED** - After implementation, you MUST verify files exist on disk with correct content.
5. **ABSOLUTELY NO BACKWARDS COMPATIBILITY** - If something doesn't work, it must error with robust logging.

---

## 1. Executive Summary

**What This Task Creates:**
- New CLI subcommand: `context-graph-cli setup`
- Generates `.claude/settings.json` with native hook configuration
- Generates 5 hook shell scripts in `.claude/hooks/` directory
- Preserves existing non-hook settings in `settings.json` if file exists
- Makes all shell scripts executable (chmod 755)

**Why It's Needed:**
- One-command setup for NEW projects to integrate context-graph with Claude Code
- Users run `context-graph-cli setup` in their project root to configure hooks
- Currently these files exist manually in the contextgraph project itself but there's no automated setup for other projects

**Current State:**
- **Hook scripts EXIST** at `.claude/hooks/` (5 scripts, all working)
- **settings.json EXISTS** at `.claude/settings.json` (working configuration)
- **Setup command DOES NOT EXIST** - This task creates it

---

## 2. Current State Analysis (VERIFIED 2026-01-17)

### 2.1 CLI Structure (ACTUAL - as of HEAD)

```
crates/context-graph-cli/src/
├── main.rs                           # Entry point, Commands enum (Session, Hooks, Memory)
├── error.rs                          # CliExitCode (0=Success, 1=Warning, 2=Blocking)
└── commands/
    ├── mod.rs                        # Exports + GLOBAL_IDENTITY_LOCK
    ├── session/                      # restore-identity, persist-identity
    ├── hooks/                        # session-start, pre-tool, post-tool, prompt-submit, session-end
    └── memory/
        ├── mod.rs                    # MemoryCommands enum
        ├── inject.rs                 # inject-context, inject-brief
        └── capture.rs                # capture-memory, capture-response
```

**CRITICAL: `commands/setup.rs` DOES NOT EXIST** - This task creates it.

### 2.2 Existing Hook Files (REFERENCE - to be templated)

These files exist and are the templates for what setup generates:

| File | Location | Purpose |
|------|----------|---------|
| `settings.json` | `.claude/settings.json` | Hook configuration |
| `session_start.sh` | `.claude/hooks/session_start.sh` | SessionStart hook |
| `pre_tool_use.sh` | `.claude/hooks/pre_tool_use.sh` | PreToolUse hook |
| `post_tool_use.sh` | `.claude/hooks/post_tool_use.sh` | PostToolUse hook |
| `user_prompt_submit.sh` | `.claude/hooks/user_prompt_submit.sh` | UserPromptSubmit hook |
| `session_end.sh` | `.claude/hooks/session_end.sh` | SessionEnd hook |

### 2.3 Settings.json Structure (ACTUAL)

```json
{
  "hooks": {
    "SessionStart": [{"hooks": [{"type": "command", "command": ".claude/hooks/session_start.sh", "timeout": 5000}]}],
    "SessionEnd": [{"hooks": [{"type": "command", "command": ".claude/hooks/session_end.sh", "timeout": 30000}]}],
    "PreToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": ".claude/hooks/pre_tool_use.sh", "timeout": 100}]}],
    "PostToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": ".claude/hooks/post_tool_use.sh", "timeout": 3000}]}],
    "UserPromptSubmit": [{"hooks": [{"type": "command", "command": ".claude/hooks/user_prompt_submit.sh", "timeout": 2000}]}]
  }
}
```

### 2.4 What This Task Creates

| Component | Status | Location |
|-----------|--------|----------|
| `setup` command | **TO CREATE** | `context-graph-cli/src/commands/setup.rs` |
| `Setup` in Commands enum | **TO CREATE** | `context-graph-cli/src/main.rs` |
| `setup` module export | **TO CREATE** | `context-graph-cli/src/commands/mod.rs` |

---

## 3. Technical Specification

### 3.1 CLI Command Structure (Clap)

Add to `Commands` enum in `main.rs`:

```rust
#[derive(Subcommand)]
enum Commands {
    // Existing
    Session { ... },
    Hooks { ... },
    Memory { ... },

    // NEW - This task
    /// Initialize context-graph hooks for Claude Code
    ///
    /// Creates .claude/settings.json and .claude/hooks/ directory with
    /// all required hook scripts for context-graph integration.
    Setup(commands::setup::SetupArgs),
}
```

### 3.2 Argument Struct

```rust
#[derive(Args)]
pub struct SetupArgs {
    /// Force overwrite existing configuration
    #[arg(long, short = 'f')]
    pub force: bool,

    /// Target directory (default: current working directory)
    #[arg(long)]
    pub target_dir: Option<PathBuf>,

    /// Skip making scripts executable (for testing on non-Unix systems)
    #[arg(long, hide = true)]
    pub skip_chmod: bool,
}
```

### 3.3 Handler Function

```rust
pub async fn handle_setup(args: SetupArgs) -> i32 {
    // 1. Determine target directory (args.target_dir or cwd)
    // 2. Check if .claude/settings.json exists
    //    - If exists and has "hooks" key and !force: return error (exit 1)
    //    - If exists but no "hooks" key: merge hooks into existing
    //    - If !exists: create new
    // 3. Create .claude/ directory if needed
    // 4. Create .claude/hooks/ directory if needed
    // 5. Write settings.json (merge if existing)
    // 6. Write all 5 hook scripts
    // 7. chmod 755 all scripts (unless skip_chmod)
    // 8. Print success summary to stdout
    // 9. Return exit 0
}
```

### 3.4 Exit Codes (AP-26)

| Exit Code | Meaning | When |
|-----------|---------|------|
| 0 | Success | All files created successfully |
| 1 | Error | Hooks already configured (without --force), file write failure |
| 2 | N/A | Not used for setup (no corruption possible) |

### 3.5 Performance Constraints

| Operation | Budget |
|-----------|--------|
| setup command | <1s total |
| File operations | <100ms per file |

---

## 4. Files to Create/Modify

### 4.1 CREATE: `crates/context-graph-cli/src/commands/setup.rs`

Full implementation with:
- `SetupArgs` struct with clap derive
- `handle_setup()` async function
- `SETTINGS_JSON_TEMPLATE` const (the hooks configuration JSON)
- `HOOK_SCRIPT_TEMPLATES` - 5 const strings for hook scripts
- `create_hook_script()` helper
- `merge_settings()` helper
- Comprehensive tests

### 4.2 MODIFY: `crates/context-graph-cli/src/commands/mod.rs`

Add:
```rust
pub mod setup;
```

### 4.3 MODIFY: `crates/context-graph-cli/src/main.rs`

Add `Setup` variant to Commands enum and dispatch to handler.

---

## 5. Hook Script Templates

**CRITICAL: These templates must match the ACTUAL working scripts in `.claude/hooks/`**

The setup command generates these 5 scripts:

### 5.1 session_start.sh (~90 lines)

Key features:
- Reads JSON from stdin
- Validates input with jq
- Finds CLI binary (checks multiple locations)
- Builds HookInput JSON with snake_case format
- Calls `context-graph-cli hooks session-start --stdin --format json`
- 5s timeout wrapper

### 5.2 pre_tool_use.sh (~65 lines)

Key features:
- FAST PATH: No database operations
- 500ms wrapper timeout (100ms CLI budget)
- Calls `context-graph-cli hooks pre-tool --fast-path true`

### 5.3 post_tool_use.sh (~60 lines)

Key features:
- 3s timeout
- Calls `context-graph-cli hooks post-tool`

### 5.4 user_prompt_submit.sh (~85 lines)

Key features:
- Uses jq to safely embed prompt (avoids shell injection)
- 2s timeout
- Calls `context-graph-cli hooks prompt-submit --stdin true`

### 5.5 session_end.sh (~60 lines)

Key features:
- 30s timeout for full persistence
- Calls `context-graph-cli hooks session-end`

---

## 6. Definition of Done

### 6.1 Functional Criteria

| ID | Criterion | Verification Method |
|----|-----------|---------------------|
| DOD-1 | `setup` creates .claude/settings.json | File exists after command |
| DOD-2 | `setup` creates .claude/hooks/ directory | Directory exists |
| DOD-3 | `setup` creates 5 hook scripts | ls .claude/hooks/ shows 5 files |
| DOD-4 | Scripts are executable | stat -c %a shows 755 |
| DOD-5 | `--force` overwrites existing config | Run twice with --force, verify |
| DOD-6 | Without --force, fails if hooks exist | Exit code 1 with message |
| DOD-7 | Preserves existing non-hook settings | Add custom key, run setup, verify key exists |
| DOD-8 | settings.json matches expected structure | Parse and validate keys |

### 6.2 Exit Code Verification

| Scenario | Expected Exit |
|----------|---------------|
| Fresh setup (no existing config) | 0 |
| Existing hooks, no --force | 1 |
| Existing hooks, with --force | 0 |
| Existing settings.json, no hooks key | 0 (merge) |
| Write permission denied | 1 |

---

## 7. Full State Verification Protocol (MANDATORY)

After implementing, you MUST perform these verification steps.

### 7.1 Source of Truth

**Primary:** File system at target directory
**Files to verify:**
- `.claude/settings.json` - content and validity
- `.claude/hooks/session_start.sh` - exists, executable, content
- `.claude/hooks/pre_tool_use.sh` - exists, executable, content
- `.claude/hooks/post_tool_use.sh` - exists, executable, content
- `.claude/hooks/user_prompt_submit.sh` - exists, executable, content
- `.claude/hooks/session_end.sh` - exists, executable, content

### 7.2 Execute & Inspect Protocol

```bash
# Step 1: Build CLI
cargo build --package context-graph-cli

# Step 2: Create test directory
mkdir -p /tmp/test-setup-project
cd /tmp/test-setup-project

# Step 3: Run setup
/path/to/context-graph-cli setup
echo "Exit code: $?"  # Expected: 0

# Step 4: MANDATORY - Verify files exist
ls -la .claude/
ls -la .claude/hooks/

# Step 5: MANDATORY - Verify settings.json content
cat .claude/settings.json | jq .

# Step 6: MANDATORY - Verify permissions
stat -c "%a %n" .claude/hooks/*.sh
# Expected: 755 for all files

# Step 7: MANDATORY - Verify script content starts correctly
head -1 .claude/hooks/session_start.sh
# Expected: #!/bin/bash
```

### 7.3 Boundary & Edge Case Audit (3 Required)

**Edge Case 1: Existing hooks, no --force**
```bash
# Before state
cd /tmp/test-setup-project-ec1
/path/to/context-graph-cli setup
echo "First setup exit: $?"  # 0

# Action
/path/to/context-graph-cli setup
EXIT_CODE=$?

# After state
echo "Second setup exit: $EXIT_CODE"  # Expected: 1
# Verify error message on stderr: "Hooks already configured"
```

**Edge Case 2: Existing hooks, with --force**
```bash
# Before state
cd /tmp/test-setup-project-ec2
/path/to/context-graph-cli setup
echo "settings.json before:"
cat .claude/settings.json | jq -c .

# Action
/path/to/context-graph-cli setup --force
EXIT_CODE=$?

# After state
echo "Second setup exit: $EXIT_CODE"  # Expected: 0
echo "settings.json after:"
cat .claude/settings.json | jq -c .
```

**Edge Case 3: Merge with existing settings.json (no hooks key)**
```bash
# Before state
cd /tmp/test-setup-project-ec3
mkdir -p .claude
echo '{"customSetting": "value123", "theme": "dark"}' > .claude/settings.json
echo "Before:"
cat .claude/settings.json

# Action
/path/to/context-graph-cli setup
EXIT_CODE=$?

# After state
echo "Exit: $EXIT_CODE"  # Expected: 0
echo "After:"
cat .claude/settings.json | jq .
# Expected: customSetting and theme preserved, hooks added
```

### 7.4 Evidence of Success Log Template

```
=== SETUP COMMAND FSV VERIFICATION ===
Test Directory: /tmp/test-setup-project
Test Time: $(date)

[Test 1] Fresh setup
  Exit Code: 0 ✓
  .claude/ created: ✓
  .claude/hooks/ created: ✓
  settings.json exists: ✓
  settings.json valid JSON: ✓
  session_start.sh exists: ✓ (755)
  pre_tool_use.sh exists: ✓ (755)
  post_tool_use.sh exists: ✓ (755)
  user_prompt_submit.sh exists: ✓ (755)
  session_end.sh exists: ✓ (755)

[Edge Case 1] Existing hooks, no --force
  Exit Code: 1 ✓
  Error Message: "Hooks already configured" ✓
  Files unchanged: ✓

[Edge Case 2] Existing hooks, with --force
  Exit Code: 0 ✓
  Files overwritten: ✓

[Edge Case 3] Merge with existing settings
  customSetting preserved: ✓
  hooks added: ✓
  Exit Code: 0 ✓

=== ALL VERIFICATIONS PASSED ===
```

---

## 8. Test Commands

### 8.1 Build & Unit Tests

```bash
# Build CLI
cargo build --package context-graph-cli

# Run setup module tests
cargo test commands::setup --package context-graph-cli -- --nocapture

# Run all CLI tests
cargo test --package context-graph-cli -- --nocapture
```

### 8.2 Integration Tests

```bash
# Fresh setup
rm -rf /tmp/test-setup && mkdir /tmp/test-setup && cd /tmp/test-setup
./target/debug/context-graph-cli setup
echo "Exit: $?"
ls -la .claude/hooks/

# Force overwrite
./target/debug/context-graph-cli setup --force
echo "Exit: $?"

# Verify settings.json
cat .claude/settings.json | jq .hooks.SessionStart
```

---

## 9. Dependencies (All Satisfied)

| Dependency | Status | Location |
|------------|--------|----------|
| TASK-P6-001 (CLI infrastructure) | ✅ COMPLETE | main.rs, clap setup |
| TASK-P6-002 (Session commands) | ✅ COMPLETE | commands/session/ |
| TASK-P6-003 (Inject command) | ✅ COMPLETE | commands/memory/inject.rs |
| TASK-P6-005 (Capture command) | ✅ COMPLETE | commands/memory/capture.rs |

---

## 10. Constitution Compliance

| Rule | Compliance |
|------|------------|
| ARCH-07 | ✅ Native Claude Code hooks via .claude/settings.json |
| AP-14 | ✅ No .unwrap() - use map_err, ok_or |
| AP-26 | ✅ Exit codes: 0=success, 1=error |
| AP-50 | ✅ Native hooks via settings.json ONLY |
| AP-53 | ✅ Hook logic in shell scripts calling CLI |

---

## 11. Implementation Checklist

- [x] Create `crates/context-graph-cli/src/commands/setup.rs`
- [x] Define `SetupArgs` struct with --force, --target-dir flags
- [x] Define `SETTINGS_JSON_TEMPLATE` const
- [x] Define hook script template constants (5 scripts)
- [x] Implement `handle_setup()` async function
- [x] Implement `merge_settings()` helper for preserving non-hook keys
- [x] Implement `create_hook_script()` helper with chmod 755
- [x] Add `pub mod setup;` to `commands/mod.rs`
- [x] Add `Setup(commands::setup::SetupArgs)` to Commands enum
- [x] Add match arm for Setup in main()
- [x] Write unit tests for SetupArgs parsing
- [x] Write unit tests for merge_settings logic
- [x] Write integration tests with temp directories
- [x] Run `cargo build --package context-graph-cli`
- [x] Run `cargo test commands::setup --package context-graph-cli`
- [x] Execute manual CLI tests in temp directory
- [x] Verify all 5 hook files exist and are executable
- [x] Verify settings.json structure matches spec
- [x] Run all 3 edge case tests
- [x] Generate evidence log
- [x] Update this document with completion status

## 16. Completion Status

**Status: ✅ COMPLETE**
**Completed Date: 2026-01-17**

### Implementation Summary

Created `crates/context-graph-cli/src/commands/setup.rs` with:
- `SetupArgs` struct with --force, --target-dir, --skip-chmod flags
- `handle_setup()` async function (returns i32 exit code)
- `check_existing_hooks()` helper to detect existing configuration
- `write_settings_json()` helper with merge functionality
- `fail()` helper for consistent error logging
- Template constants for settings.json and all 5 hook scripts
- 11 comprehensive unit tests (8 functional + 3 edge cases)

### Files Modified
| File | Change |
|------|--------|
| `commands/setup.rs` | CREATED - Full implementation (1116 lines) |
| `commands/mod.rs` | MODIFIED - Added `pub mod setup;` |
| `main.rs` | MODIFIED - Added `Setup(commands::setup::SetupArgs)` to Commands enum |

### Verification Evidence

```
=== SETUP COMMAND FSV VERIFICATION ===
Test Directory: /tmp/test-setup-project
Test Time: Sat Jan 17 22:30:22 CST 2026

[Test 1] Fresh setup
  Exit Code: 0 ✓
  .claude/ created: ✓
  .claude/hooks/ created: ✓
  settings.json exists: ✓
  settings.json valid JSON: ✓
  session_start.sh exists: ✓ (755)
  pre_tool_use.sh exists: ✓ (755)
  post_tool_use.sh exists: ✓ (755)
  user_prompt_submit.sh exists: ✓ (755)
  session_end.sh exists: ✓ (755)

[Edge Case 1] Existing hooks, no --force
  Exit Code: 1 ✓
  Error Message: 'Hooks already configured' ✓
  Files unchanged: ✓

[Edge Case 2] Existing hooks, with --force
  Exit Code: 0 ✓
  Files overwritten: ✓

[Edge Case 3] Merge with existing settings
  customSetting preserved: ✓
  theme preserved: ✓
  hooks added: ✓
  Exit Code: 0 ✓

=== ALL VERIFICATIONS PASSED ===
```

### Test Results
```
running 11 tests
test commands::setup::tests::tc_setup_08_nonexistent_target_fails ... ok
test commands::setup::tests::edge_case_invalid_settings_json ... ok
test commands::setup::tests::edge_case_force_preserves_custom_settings ... ok
test commands::setup::tests::edge_case_empty_target_directory ... ok
test commands::setup::tests::tc_setup_01_fresh_setup_creates_all_files ... ok
test commands::setup::tests::tc_setup_02_existing_hooks_no_force_fails ... ok
test commands::setup::tests::tc_setup_05_scripts_are_executable ... ok
test commands::setup::tests::tc_setup_03_existing_hooks_with_force_succeeds ... ok
test commands::setup::tests::tc_setup_04_merge_preserves_existing_settings ... ok
test commands::setup::tests::tc_setup_07_script_content_has_shebang ... ok
test commands::setup::tests::tc_setup_06_settings_structure_matches_spec ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 194 filtered out
```

---

## 12. Traceability

| Spec Item | This Task Covers |
|-----------|------------------|
| Command: setup (cli_command) | ✅ handle_setup |
| REQ-P6-05: Setup command | ✅ Creates hook configuration |
| ARCH-07: Native hooks | ✅ Generates .claude/settings.json |
| AP-50: No internal hooks | ✅ Uses native Claude Code hooks |

---

## 13. Related Tasks

| Task ID | Title | Relationship |
|---------|-------|--------------|
| TASK-P6-001 | CLI Infrastructure | Dependency (provides main.rs, clap) |
| TASK-P6-008 | Hook Shell Scripts | Related (setup generates these) |
| TASK-P6-002 | Session Commands | Used by generated hooks |
| TASK-P6-003 | Inject Context | Used by generated hooks |
| TASK-P6-005 | Capture Memory | Used by generated hooks |

---

## 14. Script Content Reference

The setup command should generate scripts that are FUNCTIONALLY EQUIVALENT to the existing scripts at `.claude/hooks/`. The key patterns are:

### Common Shell Script Header
```bash
#!/bin/bash
# Claude Code Hook: [EventName]
# Timeout: [timeout]ms
#
# Constitution: AP-50, AP-26
# Exit Codes: 0=success, 1=error, 2=timeout, 3=db_error, 4=invalid_input

set -euo pipefail
```

### Common CLI Binary Location Logic
```bash
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-context-graph-cli}"
if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null; then
    for candidate in \
        "./target/release/context-graph-cli" \
        "./target/debug/context-graph-cli" \
        "$HOME/.cargo/bin/context-graph-cli" \
    ; do
        if [ -x "$candidate" ]; then
            CONTEXT_GRAPH_CLI="$candidate"
            break
        fi
    done
fi
```

### Common Input Validation
```bash
INPUT=$(cat)
if [ -z "$INPUT" ]; then
    echo '{"success":false,"error":"Empty stdin","exit_code":4}' >&2
    exit 4
fi

if ! echo "$INPUT" | jq empty 2>/dev/null; then
    echo '{"success":false,"error":"Invalid JSON input","exit_code":4}' >&2
    exit 4
fi
```

**RECOMMENDATION:** Read the actual scripts from `.claude/hooks/` directory and use them as the template content. This ensures the generated scripts match what's already tested and working.

---

## 15. Anti-Patterns (FORBIDDEN)

1. **NO MOCKS** - Tests must create real files in temp directories
2. **NO FALLBACKS** - If file creation fails, propagate error with context
3. **NO SILENT FAILURES** - Log all errors to stderr before returning exit code
4. **NO PARTIAL STATES** - Either all files created or none (cleanup on failure)
5. **NO HARDCODED PATHS** - Use target_dir arg or cwd
6. **NO WINDOWS ASSUMPTIONS** - Use std::os::unix::fs::PermissionsExt for chmod
