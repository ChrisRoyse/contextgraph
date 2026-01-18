# Task: TASK-P6-008 - Hook Shell Scripts

```xml
<task_spec id="TASK-P6-008" version="2.0">
<metadata>
  <title>Hook Shell Scripts - Validation and Testing</title>
  <phase>6</phase>
  <sequence>50</sequence>
  <layer>surface</layer>
  <estimated_loc>100</estimated_loc>
  <status>IMPLEMENTATION COMPLETE - REQUIRES VALIDATION</status>
  <last_audit>2026-01-17</last_audit>
  <dependencies>
    <dependency task="TASK-P6-007" status="COMPLETE">Setup command creates scripts in setup.rs</dependency>
    <dependency task="TASK-P6-002" status="COMPLETE">Session commands used by scripts</dependency>
    <dependency task="TASK-P6-003" status="COMPLETE">Inject command used by scripts</dependency>
    <dependency task="TASK-P6-005" status="COMPLETE">Capture command used by scripts</dependency>
  </dependencies>
  <produces>
    <artifact type="script" location=".claude/hooks/session_start.sh">Session initialization</artifact>
    <artifact type="script" location=".claude/hooks/user_prompt_submit.sh">User prompt context injection</artifact>
    <artifact type="script" location=".claude/hooks/pre_tool_use.sh">Pre-tool brief context (FAST PATH)</artifact>
    <artifact type="script" location=".claude/hooks/post_tool_use.sh">Post-tool memory capture</artifact>
    <artifact type="script" location=".claude/hooks/session_end.sh">Session persistence</artifact>
    <artifact type="config" location=".claude/settings.json">Native Claude Code hook configuration</artifact>
  </produces>
</metadata>

<current_state_audit>
  <summary>
    Hook shell scripts are FULLY IMPLEMENTED as embedded constants in setup.rs.
    The scripts are already deployed to .claude/hooks/ and configured in .claude/settings.json.
    This task focuses on VALIDATION and TESTING of the existing implementation.
  </summary>

  <implemented_files>
    <file path="crates/context-graph-cli/src/commands/setup.rs" lines="1116">
      Contains all 5 script templates as constants:
      - SESSION_START_SCRIPT (lines 265-367)
      - PRE_TOOL_USE_SCRIPT (lines 370-435)
      - POST_TOOL_USE_SCRIPT (lines 438-499)
      - USER_PROMPT_SUBMIT_SCRIPT (lines 502-586)
      - SESSION_END_SCRIPT (lines 589-649)
      Also contains SETTINGS_JSON_TEMPLATE (lines 202-262)
    </file>
    <file path=".claude/settings.json" lines="61">
      Native Claude Code hook configuration with all 5 hooks registered.
      Matchers: PreToolUse and PostToolUse use ".*" to match all tools.
    </file>
    <file path=".claude/hooks/session_start.sh" lines="103" executable="true"/>
    <file path=".claude/hooks/pre_tool_use.sh" lines="66" executable="true"/>
    <file path=".claude/hooks/post_tool_use.sh" lines="62" executable="true"/>
    <file path=".claude/hooks/user_prompt_submit.sh" lines="86" executable="true"/>
    <file path=".claude/hooks/session_end.sh" lines="59" executable="true"/>
  </implemented_files>

  <discrepancies_from_original_spec>
    <discrepancy id="D1" severity="design_decision">
      <original>6 scripts (includes stop.sh)</original>
      <actual>5 scripts (no stop.sh)</actual>
      <reason>Stop hook functionality merged into session-end.sh. The Stop event in Claude Code is for forcing continuation, not session termination.</reason>
    </discrepancy>
    <discrepancy id="D2" severity="improvement">
      <original>Uses environment variables (USER_PROMPT, TOOL_NAME, TOOL_DESCRIPTION)</original>
      <actual>Uses stdin JSON for all input</actual>
      <reason>stdin JSON is more secure (handles special characters) and recommended by Claude Code docs</reason>
    </discrepancy>
    <discrepancy id="D3" severity="improvement">
      <original>Outputs plain text session ID and context</original>
      <actual>Outputs structured JSON with coherence/stability metrics</actual>
      <reason>Structured output enables richer context injection</reason>
    </discrepancy>
  </discrepancies_from_original_spec>
</current_state_audit>

<context>
  <background>
    The hook shell scripts bridge Claude Code's native hook system to context-graph-cli.
    They receive JSON input via stdin from Claude Code, transform it to the CLI's expected
    HookInput format, invoke the CLI, and pass through output/exit codes.
  </background>
  <business_value>
    Scripts are the integration point with Claude Code. They must be reliable, handle
    errors gracefully, respect timeout constraints, and FAIL FAST with clear error messages.
  </business_value>
  <technical_context>
    Scripts use set -euo pipefail for fail-fast behavior. They read from stdin (JSON),
    transform input using jq, invoke CLI with appropriate flags, and handle timeout/errors.
    Each script has a configured timeout in settings.json matching constitution requirements.
  </technical_context>
</context>

<constitution_references>
  <rule id="ARCH-07">Native Claude Code hooks (.claude/settings.json) control memory lifecycle</rule>
  <rule id="AP-14">No .unwrap() - use map_err, ok_or in Rust code</rule>
  <rule id="AP-26">Exit codes: 0=success, 1=error, 2=timeout, 3=db_error, 4=invalid_input</rule>
  <rule id="AP-50">NATIVE hooks via settings.json ONLY - no internal/built-in hooks</rule>
  <rule id="AP-53">Hook logic MUST be in shell scripts calling context-graph-cli</rule>
</constitution_references>

<timeout_budgets>
  <!-- From constitution: claude_code.performance.hooks -->
  <hook name="session_start" timeout_ms="5000" cli_internal="4500"/>
  <hook name="pre_tool_use" timeout_ms="100" cli_internal="<100" note="FAST PATH - NO DATABASE ACCESS"/>
  <hook name="post_tool_use" timeout_ms="3000" cli_internal="2500" async="true"/>
  <hook name="user_prompt_submit" timeout_ms="2000" cli_internal="1500"/>
  <hook name="session_end" timeout_ms="30000" cli_internal="29000"/>
</timeout_budgets>

<cli_commands>
  <!-- Actual CLI command structure from main.rs and hooks/args.rs -->
  <command>context-graph-cli hooks session-start --stdin --format json</command>
  <command>context-graph-cli hooks pre-tool --session-id ID --tool-name NAME --fast-path true --format json</command>
  <command>context-graph-cli hooks post-tool --session-id ID --tool-name NAME --success BOOL --format json</command>
  <command>context-graph-cli hooks prompt-submit --session-id ID --stdin true --format json</command>
  <command>context-graph-cli hooks session-end --session-id ID --duration-ms MS --generate-summary true --format json</command>
</cli_commands>

<scope>
  <includes>
    <item>Script execution validation (all 5 scripts run without error)</item>
    <item>Input/output format validation (JSON structure compliance)</item>
    <item>Error handling validation (empty input, invalid JSON, missing CLI)</item>
    <item>Timeout compliance validation (scripts complete within budget)</item>
    <item>Exit code passthrough validation (CLI exit codes flow through)</item>
    <item>CLI binary discovery validation (PATH, ./target/debug, ./target/release)</item>
    <item>Synthetic data testing with known inputs/outputs</item>
  </includes>
  <excludes>
    <item>Script generation code (already complete in setup.rs)</item>
    <item>CLI command implementation internals</item>
    <item>Hook handler Rust code</item>
  </excludes>
</scope>

<definition_of_done>
  <criterion id="DOD-1" type="execution">
    <description>All 5 scripts execute without error when given valid JSON input</description>
    <verification>Execute each script with synthetic input, verify exit code 0</verification>
    <source_of_truth>.claude/hooks/*.sh execution result + exit codes</source_of_truth>
  </criterion>

  <criterion id="DOD-2" type="input_validation">
    <description>Scripts handle invalid input gracefully with proper error messages</description>
    <verification>Test with empty stdin, malformed JSON, missing required fields</verification>
    <source_of_truth>stderr output contains JSON error structure, exit code 4</source_of_truth>
  </criterion>

  <criterion id="DOD-3" type="timeout">
    <description>All scripts complete within their timeout budgets</description>
    <verification>time command shows execution under limit</verification>
    <source_of_truth>Wall clock time measurement for each script</source_of_truth>
  </criterion>

  <criterion id="DOD-4" type="cli_discovery">
    <description>Scripts find CLI binary in all expected locations</description>
    <verification>Test with CLI in PATH, ./target/debug, ./target/release</verification>
    <source_of_truth>Script execution succeeds regardless of CLI location</source_of_truth>
  </criterion>

  <criterion id="DOD-5" type="exit_code">
    <description>CLI exit codes pass through correctly</description>
    <verification>Verify exit codes 0, 1, 2, 3, 4 propagate from CLI to script exit</verification>
    <source_of_truth>Script exit code matches CLI exit code</source_of_truth>
  </criterion>

  <criterion id="DOD-6" type="json_output">
    <description>Scripts produce valid JSON output on success</description>
    <verification>Parse stdout with jq, verify JSON structure</verification>
    <source_of_truth>jq parses output without error</source_of_truth>
  </criterion>
</definition_of_done>

<full_state_verification>
  <requirement>
    After completing any logic, you MUST perform Full State Verification.
    Do not rely on return values alone.
  </requirement>

  <step id="1" name="Define Source of Truth">
    <description>Identify where the final result is stored</description>
    <examples>
      - Script execution: exit code + stdout/stderr content
      - Session start: ~/.contextgraph/sessions/{session_id}.json cache file
      - Memory capture: RocksDB storage (check via CLI query)
      - Configuration: .claude/settings.json file content
    </examples>
  </step>

  <step id="2" name="Execute and Inspect">
    <description>Run the logic, then immediately perform a separate Read operation on the Source of Truth</description>
    <examples>
      - After script execution: cat the stdout capture, check exit code
      - After session start: ls ~/.contextgraph/sessions/ to verify cache file created
      - After memory capture: context-graph-cli memory list to verify memory stored
    </examples>
  </step>

  <step id="3" name="Boundary and Edge Case Audit">
    <description>Manually simulate 3+ edge cases, print system state before and after</description>
    <edge_cases>
      - Empty input: echo '' | ./script.sh
      - Invalid JSON: echo 'not json' | ./script.sh
      - Maximum limit: Very long prompt text (10KB+)
      - Missing required fields: echo '{}' | ./script.sh
      - Unicode/special characters: echo '{"prompt":"Hello \n\t \"world\""}' | ./script.sh
    </edge_cases>
  </step>

  <step id="4" name="Evidence of Success">
    <description>Provide a log showing the actual data residing in the system after execution</description>
    <format>
      === TEST: [test_name] ===
      INPUT: [raw input]
      COMMAND: [command executed]
      EXIT_CODE: [code]
      STDOUT: [output]
      STDERR: [error output if any]
      SOURCE_OF_TRUTH_CHECK: [verification of persistent state]
      RESULT: PASS/FAIL
    </format>
  </step>
</full_state_verification>

<manual_test_procedures>
  <procedure id="MT-001" name="Session Start Happy Path">
    <description>Verify session_start.sh creates session with valid input</description>
    <input>{"session_id": "test-session-001"}</input>
    <expected_output>JSON with session_id, coherence_state, stability</expected_output>
    <expected_exit_code>0</expected_exit_code>
    <source_of_truth>~/.contextgraph/sessions/test-session-001.json exists</source_of_truth>
    <commands>
      # Build CLI first
      cargo build --package context-graph-cli

      # Execute test
      echo '{"session_id": "test-session-001"}' | ./.claude/hooks/session_start.sh
      echo "Exit code: $?"

      # Verify source of truth
      ls -la ~/.contextgraph/sessions/ | grep test-session-001
      cat ~/.contextgraph/sessions/test-session-001.json | jq .
    </commands>
  </procedure>

  <procedure id="MT-002" name="Pre-Tool Use FAST PATH">
    <description>Verify pre_tool_use.sh completes in under 500ms</description>
    <input>{"session_id": "test-session-001", "tool_name": "Bash"}</input>
    <expected_output>JSON (may be empty or brief context)</expected_output>
    <expected_exit_code>0</expected_exit_code>
    <timing_constraint>real &lt; 0.5s</timing_constraint>
    <commands>
      time (echo '{"session_id": "test-session-001", "tool_name": "Bash"}' | ./.claude/hooks/pre_tool_use.sh)
      echo "Exit code: $?"
    </commands>
  </procedure>

  <procedure id="MT-003" name="User Prompt Submit with Special Characters">
    <description>Verify user_prompt_submit.sh handles special characters safely</description>
    <input>{"session_id": "test-session-001", "prompt": "Hello\nWorld\t\"quoted\""}</input>
    <expected_output>JSON with context injection</expected_output>
    <expected_exit_code>0</expected_exit_code>
    <security_note>Must not execute shell injection via prompt content</security_note>
    <commands>
      echo '{"session_id": "test-session-001", "prompt": "Hello\nWorld\t\"quoted\""}' | ./.claude/hooks/user_prompt_submit.sh
      echo "Exit code: $?"
    </commands>
  </procedure>

  <procedure id="MT-004" name="Post Tool Use Memory Capture">
    <description>Verify post_tool_use.sh captures tool execution as memory</description>
    <input>{"session_id": "test-session-001", "tool_name": "Write", "success": true}</input>
    <expected_output>JSON confirmation</expected_output>
    <expected_exit_code>0</expected_exit_code>
    <source_of_truth>Memory count increases (verify via memory list command)</source_of_truth>
    <commands>
      # Get before state
      context-graph-cli memory list --limit 1 2>/dev/null || echo "No memories yet"

      # Execute
      echo '{"session_id": "test-session-001", "tool_name": "Write", "success": true}' | ./.claude/hooks/post_tool_use.sh
      echo "Exit code: $?"

      # Verify after state
      context-graph-cli memory list --limit 1 2>/dev/null || echo "Still no memories"
    </commands>
  </procedure>

  <procedure id="MT-005" name="Session End Persistence">
    <description>Verify session_end.sh persists session state</description>
    <input>{"session_id": "test-session-001", "stats": {"duration_ms": 60000}}</input>
    <expected_output>JSON with summary</expected_output>
    <expected_exit_code>0</expected_exit_code>
    <source_of_truth>Session marked as ended in cache, summary generated</source_of_truth>
    <commands>
      echo '{"session_id": "test-session-001", "stats": {"duration_ms": 60000}}' | ./.claude/hooks/session_end.sh
      echo "Exit code: $?"

      # Verify session state
      cat ~/.contextgraph/sessions/test-session-001.json | jq .
    </commands>
  </procedure>

  <procedure id="MT-006" name="Empty Input Handling">
    <description>Verify all scripts reject empty stdin with exit code 4</description>
    <commands>
      for script in session_start pre_tool_use post_tool_use user_prompt_submit session_end; do
        echo "Testing $script with empty input:"
        echo '' | ./.claude/hooks/${script}.sh 2>&amp;1
        echo "Exit code: $?"
        echo "---"
      done
    </commands>
    <expected>All scripts exit with code 4 and JSON error on stderr</expected>
  </procedure>

  <procedure id="MT-007" name="Invalid JSON Handling">
    <description>Verify all scripts reject invalid JSON with exit code 4</description>
    <commands>
      for script in session_start pre_tool_use post_tool_use user_prompt_submit session_end; do
        echo "Testing $script with invalid JSON:"
        echo 'not valid json at all' | ./.claude/hooks/${script}.sh 2>&amp;1
        echo "Exit code: $?"
        echo "---"
      done
    </commands>
    <expected>All scripts exit with code 4 and JSON error on stderr</expected>
  </procedure>

  <procedure id="MT-008" name="CLI Binary Not Found">
    <description>Verify scripts fail gracefully when CLI binary is not available</description>
    <commands>
      # Temporarily hide the CLI
      mv ./target/debug/context-graph-cli ./target/debug/context-graph-cli.bak 2>/dev/null || true
      PATH_BACKUP="$PATH"
      export PATH="/bin:/usr/bin"

      echo '{"session_id": "test"}' | ./.claude/hooks/session_start.sh 2>&amp;1
      echo "Exit code: $?"

      # Restore
      export PATH="$PATH_BACKUP"
      mv ./target/debug/context-graph-cli.bak ./target/debug/context-graph-cli 2>/dev/null || true
    </commands>
    <expected>Exit code 1 with "CLI binary not found" error</expected>
  </procedure>
</manual_test_procedures>

<synthetic_test_data>
  <description>
    Use these synthetic inputs to validate scripts. You know the expected outputs,
    so you can verify correctness by checking if outputs match expectations.
  </description>

  <test_case id="SYN-001">
    <name>New Session Creation</name>
    <input>{"session_id": "synthetic-test-12345"}</input>
    <expected_behavior>
      - Creates new session
      - Returns JSON with session_id matching input
      - Cache file created at ~/.contextgraph/sessions/synthetic-test-12345.json
    </expected_behavior>
  </test_case>

  <test_case id="SYN-002">
    <name>Session Linking</name>
    <input>{"session_id": "synthetic-child-001", "previous_session_id": "synthetic-test-12345"}</input>
    <expected_behavior>
      - Creates child session linked to parent
      - Returns JSON with both session IDs
      - Cache file shows parent reference
    </expected_behavior>
  </test_case>

  <test_case id="SYN-003">
    <name>Prompt with Code Block</name>
    <input>{"session_id": "synthetic-test-12345", "prompt": "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```"}</input>
    <expected_behavior>
      - Handles multiline code block safely
      - No shell injection
      - Returns context JSON
    </expected_behavior>
  </test_case>

  <test_case id="SYN-004">
    <name>Very Long Prompt (Boundary Test)</name>
    <input>Generate: {"session_id": "synthetic-test-12345", "prompt": "A ".repeat(5000)}</input>
    <expected_behavior>
      - Handles 10KB+ input
      - Does not truncate unexpectedly
      - Returns within timeout
    </expected_behavior>
  </test_case>
</synthetic_test_data>

<validation_criteria>
  <criterion type="execution">All 5 scripts execute without error with valid input</criterion>
  <criterion type="timeout">Scripts complete within timeout limits:
    - session_start.sh: &lt;5s
    - pre_tool_use.sh: &lt;0.5s (CRITICAL)
    - post_tool_use.sh: &lt;3s
    - user_prompt_submit.sh: &lt;2s
    - session_end.sh: &lt;30s
  </criterion>
  <criterion type="error_handling">Empty/invalid input returns exit code 4 with JSON error</criterion>
  <criterion type="cli_discovery">Scripts find CLI in PATH or ./target/{debug,release}</criterion>
  <criterion type="json_compliance">All output is valid JSON parseable by jq</criterion>
</validation_criteria>

<test_commands>
  <!-- Quick validation commands for CI/CD -->
  <command description="Build CLI">cargo build --package context-graph-cli</command>
  <command description="Run setup tests">cargo test --package context-graph-cli setup -- --nocapture</command>
  <command description="Verify settings.json valid">jq . .claude/settings.json</command>
  <command description="Verify all scripts executable">ls -la .claude/hooks/*.sh</command>
  <command description="Test session_start happy path">echo '{"session_id":"test"}' | .claude/hooks/session_start.sh</command>
  <command description="Test pre_tool_use timing">time (echo '{"session_id":"test","tool_name":"Bash"}' | .claude/hooks/pre_tool_use.sh)</command>
  <command description="Test error handling">echo '' | .claude/hooks/session_start.sh; echo "exit: $?"</command>
</test_commands>

<troubleshooting>
  <issue name="jq not found">
    <symptoms>Scripts fail with "jq: command not found"</symptoms>
    <solution>Install jq: apt-get install jq (Ubuntu) or brew install jq (macOS)</solution>
  </issue>

  <issue name="Permission denied">
    <symptoms>Scripts fail with "Permission denied"</symptoms>
    <solution>chmod +x .claude/hooks/*.sh</solution>
  </issue>

  <issue name="CLI not found">
    <symptoms>Exit code 1 with "CLI binary not found"</symptoms>
    <solution>
      1. cargo build --package context-graph-cli
      2. Ensure ./target/debug/context-graph-cli exists
      3. Or add to PATH: export PATH="$PATH:./target/debug"
    </solution>
  </issue>

  <issue name="Timeout on pre_tool_use">
    <symptoms>pre_tool_use.sh times out with exit code 2</symptoms>
    <solution>
      Pre-tool is FAST PATH - must not access database.
      Check that --fast-path true is being passed.
      Verify CLI handler respects fast path flag.
    </solution>
  </issue>

  <issue name="Invalid JSON output">
    <symptoms>Output not parseable by jq</symptoms>
    <solution>
      1. Check CLI is outputting JSON format (--format json flag)
      2. Ensure no debug output mixing with JSON
      3. Verify tracing is going to stderr not stdout
    </solution>
  </issue>
</troubleshooting>

<implementation_notes>
  <note type="architecture">
    The scripts are embedded as Rust string constants in setup.rs (lines 265-649).
    Any changes to scripts must be made there, then regenerated via:
    context-graph-cli setup --force
  </note>

  <note type="security">
    Scripts use jq for JSON parsing to prevent shell injection attacks.
    User input (prompts, tool descriptions) never goes into shell variables directly.
    The user_prompt_submit.sh uses jq -n with --arg to safely embed user text.
  </note>

  <note type="performance">
    pre_tool_use.sh has CRITICAL timing constraint of 100ms CLI execution.
    The wrapper allows 500ms total for shell startup overhead.
    This hook MUST NOT access the database - cache/memory only.
  </note>

  <note type="error_propagation">
    Exit codes from CLI pass through the scripts unchanged.
    This allows Claude Code to receive proper error signals.
    Timeout (124 from `timeout` command) is mapped to exit code 2.
  </note>
</implementation_notes>

<files_to_modify>
  <file path="None - Implementation Complete">
    This task is for VALIDATION only. The scripts are already implemented.
    If issues are found during validation, update setup.rs then run setup --force.
  </file>
</files_to_modify>
</task_spec>
```

## Quick Reference

### Script Locations
| Script | Path | Timeout |
|--------|------|---------|
| session_start.sh | .claude/hooks/session_start.sh | 5000ms |
| pre_tool_use.sh | .claude/hooks/pre_tool_use.sh | 100ms (FAST) |
| post_tool_use.sh | .claude/hooks/post_tool_use.sh | 3000ms |
| user_prompt_submit.sh | .claude/hooks/user_prompt_submit.sh | 2000ms |
| session_end.sh | .claude/hooks/session_end.sh | 30000ms |

### Exit Codes
| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error / CLI not found |
| 2 | Timeout |
| 3 | Database error (CLI passthrough) |
| 4 | Invalid input |

### CLI Command Reference
```bash
# Session start (reads JSON from stdin)
echo '{"session_id":"X"}' | context-graph-cli hooks session-start --stdin --format json

# Pre-tool (FAST PATH - no DB)
context-graph-cli hooks pre-tool --session-id X --tool-name Y --fast-path true --format json

# Post-tool
context-graph-cli hooks post-tool --session-id X --tool-name Y --success true --format json

# User prompt (reads from stdin for safety)
echo '{"..."}' | context-graph-cli hooks prompt-submit --session-id X --stdin true --format json

# Session end
context-graph-cli hooks session-end --session-id X --duration-ms 60000 --generate-summary true --format json
```

### Source Code Location
- Script templates: `crates/context-graph-cli/src/commands/setup.rs` (lines 265-649)
- Hook handlers: `crates/context-graph-cli/src/commands/hooks/*.rs`
- Settings template: `crates/context-graph-cli/src/commands/setup.rs` (lines 202-262)
