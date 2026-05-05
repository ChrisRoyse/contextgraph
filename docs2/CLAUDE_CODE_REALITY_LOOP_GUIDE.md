# Claude Code Reality Loop Guide

Date: 2026-05-05

Purpose: define how to use Claude Code as the outer reality-loop optimizer that currently exists as the DeepSeek optimizer, while Gemma remains the inner SWE-bench repair actor.

## Executive Decision

Use Claude Code in two different modes:

1. **Interactive operator mode**
   - Use Claude Code's `/loop` bundled skill while you are actively watching the system.
   - Best for: smoke testing, debugging the loop, manually steering a failing run, and rapidly improving the harness.

2. **Production optimizer mode**
   - Let ContextGraph trigger Claude Code programmatically with `claude -p` or the Claude Agent SDK only after durable reality changes.
   - Best for: unattended long-running SWE-bench refinement, source-of-truth replay, cost control, and full state verification.

Do not run an always-thinking Claude session as the primary optimizer. Durable artifacts are the memory. The optimizer should wake up when the system has new reality to inspect: official SWE-bench failure, validation failure, reward regression, repeated patch, missing reward, harness error, policy boundary failure, or source-of-truth mismatch.

The best design is:

```text
Gemma attempt
  -> ContextGraph applies patch through governed tools
  -> official SWE-bench Docker eval
  -> durable reality artifacts
  -> reward/failure trigger decision
  -> Claude Code optimizer session
  -> Claude reads actual artifacts and edits ContextGraph harness
  -> cargo/test/source-of-truth readback
  -> next Gemma attempt gets better reality
```

## Research Basis

Claude Code now has enough first-class functionality to serve as a coding-agent optimizer:

- Claude Code's Agent SDK can autonomously read files, run commands, search the web, edit code, use MCP, and manage context in Python or TypeScript. Source: https://code.claude.com/docs/en/agent-sdk/overview
- The CLI supports non-interactive `claude -p`, JSON output, structured JSON schema output, streaming JSON, allowed tool controls, and permission modes. Source: https://code.claude.com/docs/en/headless
- `/loop` is a bundled skill that repeatedly runs a prompt inside an open Claude Code session. It can use a prompt from `.claude/loop.md`. Source: https://code.claude.com/docs/en/scheduled-tasks
- Claude Code commands include `/loop`, `/batch`, `/simplify`, `/debug`, `/model`, `/permissions`, `/mcp`, `/hooks`, and `/usage`. Source: https://code.claude.com/docs/en/commands
- Claude Code has built-in tools for Read, Edit, Write, Bash, Grep, Glob, LSP, Monitor, WebFetch, WebSearch, MCP resources, tasks, and agent teams. Source: https://code.claude.com/docs/en/tools-reference
- Hooks provide deterministic control around tool use. They can block actions, inject context, run after edits, and spawn hook subagents for verification. Source: https://code.claude.com/docs/en/hooks
- MCP connects Claude Code to external tools and data sources, and Claude Code can itself run as an MCP server. Source: https://code.claude.com/docs/en/mcp
- Channels can push external events into a running session, but they are research-preview, require claude.ai login, and only fire while the session is open. Source: https://code.claude.com/docs/en/channels
- Agent teams can coordinate multiple Claude Code sessions, but they have overhead and should start at 3 to 5 teammates only for independent investigation/review. Source: https://code.claude.com/docs/en/agent-teams

## Why Claude Code Can Replace DeepSeek For The Outer Loop

DeepSeek is a model endpoint. Claude Code is a coding-agent harness.

For this project, the outer optimizer must do more than answer:

- inspect real files
- inspect generated reality artifacts
- run commands
- edit Rust source
- run `cargo fmt`, `cargo test`, `cargo build`
- inspect source-of-truth files after execution
- avoid editing the oracle, verifiers, reward specs, or benchmark tests
- produce a machine-readable optimizer decision
- leave durable evidence of what changed and why

Claude Code already has this shape. Its value is that it has native coding-agent tools and permission controls, so ContextGraph can invoke it as a real engineer process instead of building every tool loop manually around a raw chat model.

## Recommended Architecture

### Roles

**Gemma**

Gemma is the inner SWE-bench repair agent.

Responsibilities:

- read the SWE-bench issue reality
- inspect repository files through ContextGraph tools
- propose/apply structured edits
- respond to official SWE-bench failure reality
- iterate until the official evaluator resolves the instance

Gemma must not:

- edit official tests
- edit SWE-bench evaluation code
- edit reward specs
- edit verifiers
- claim success from local validation alone

**ContextGraph**

ContextGraph is the deterministic reality substrate.

Responsibilities:

- run Gemma attempts
- apply patches only through governed tools
- run official SWE-bench Docker evaluation
- write and read back artifacts
- calculate reward signals
- calculate optimizer trigger decisions
- call Claude Code only when durable reality says it should
- verify any Claude Code source edits through physical readback and tests

**Claude Code**

Claude Code is the outer harness optimizer.

Responsibilities:

- read ContextGraph's latest durable reality artifacts
- diagnose why Gemma failed
- improve the harness, prompt reinjection, tool surface, reward features, or telemetry
- edit ContextGraph project source when needed
- run real verification
- write a structured optimizer report

Claude Code must not:

- solve the SWE-bench patch directly unless explicitly running a separate comparison experiment
- weaken the official oracle
- edit benchmark tests
- hide failures
- remove full state verification
- add mocks that make broken behavior pass
- bypass source-of-truth readback

### Trigger Policy

Use event-triggered Claude Code, not always-on Claude Code.

Trigger Claude Code when any of these durable facts appears:

- official SWE-bench report does not include the target instance in `resolved_ids`
- Gemma produced an empty patch
- patch failed to apply
- patch repeated a previous failed patch hash
- local validation failed
- official evaluator errored or timed out
- reward decreased beyond the configured threshold
- no reward could be calculated
- source-of-truth readback failed
- ContextGraph itself hit a harness error
- policy boundary, tool contract, or verifier mismatch occurred

Do not trigger Claude Code when:

- no new durable artifact exists
- the last attempt is still running
- the latest official report already resolved the target instance
- the only available information is model text without source-of-truth evidence

## Why Not Use Only `/loop`

Claude Code `/loop` is useful, but it is not the best production control plane.

Use `/loop` when you are present because:

- it is fast to start
- it can poll the latest run
- it can run the default project loop prompt from `.claude/loop.md`
- it can use the Monitor tool for responsive local watching

Do not rely on `/loop` alone for unattended production because:

- it requires an open Claude Code session
- tasks are session-scoped
- recurring tasks expire after seven days
- missed fires do not catch up
- permission prompts can pause the session
- evidence and success predicates must still be enforced by ContextGraph

The durable loop should remain owned by ContextGraph. Claude Code should be one optimizer backend that ContextGraph can invoke and verify.

## Setup Plan

### Phase 1: Install And Verify Claude Code

Run from WSL Ubuntu:

```bash
npm install -g @anthropic-ai/claude-code
claude --version
claude doctor
```

Do not use `sudo npm install -g`.

Set authentication according to how you want billing handled:

```bash
export ANTHROPIC_API_KEY=...
```

For local interactive usage, Claude.ai login also works. For ContextGraph programmatic invocation, prefer API-key authentication because it is scriptable and auditable.

### Phase 2: Create Project Claude Memory

Create `.claude/CLAUDE.md` or update the project memory if it already exists.

The memory should say:

```markdown
# ContextGraph Reality Loop Rules

Gemma is the inner SWE-bench repair actor.
Claude Code is the outer harness optimizer.
ContextGraph owns the deterministic reality boundary.

Never edit:
- SWE-bench official evaluator
- benchmark tests
- reward specs unless the user explicitly asks
- verifiers to make a failure pass
- secrets or .env values

Never claim success unless the official SWE-bench report places the target instance in resolved_ids.

When optimizing the harness:
- inspect the latest durable reality artifacts first
- identify the source of truth for the result
- make the smallest source edit that improves Gemma's reality feedback
- run cargo fmt/test/build
- inspect generated artifacts after execution
- write a concise report with file paths, commands, source-of-truth paths, and remaining risk
```

### Phase 3: Create A Claude `/loop` Prompt

Create `.claude/loop.md` for manual operator mode:

```markdown
You are the Claude Code outer optimizer for ContextGraph.

Goal: improve the ContextGraph reality loop so Gemma can pass the configured SWE-bench Lite instance through the official Docker evaluator.

On every iteration:

1. Inspect the latest ContextGraph reality-loop runtime root.
2. Read the latest attempt summary, official SWE-bench report, reward signal, trigger decision, and optimizer artifacts.
3. Define the source of truth for success.
4. If the target instance is already in official resolved_ids, report success and do not change code.
5. If not resolved, diagnose why Gemma failed using only durable artifacts and real source files.
6. Improve the harness, prompt reinjection, tool contracts, reward features, or verification reporting if that will make Gemma more capable on the next attempt.
7. Run cargo fmt, focused tests, and cargo build for changed Rust code.
8. Read back the relevant source-of-truth files and summarize the actual state.

Do not edit benchmark tests, official evaluator code, reward specs, verifiers, or .env secrets.
Do not hide failures or add mocks.
Do not claim SWE success unless official resolved_ids contains the target instance.
```

Manual launch:

```bash
cd /home/cabdru/contextgraph
claude --model opus --permission-mode acceptEdits
```

Then inside Claude Code:

```text
/loop 10m
```

Or for a specific live watch:

```text
/loop Watch the latest ContextGraph reality-loop attempt. If Gemma fails, inspect the physical artifacts and improve the harness. Stop only if official SWE-bench resolved_ids includes the target instance.
```

### Phase 4: Use Claude Programmatically From ContextGraph

This is the production path.

The Rust reality loop should add a second optimizer provider:

```text
--optimizer-provider deepseek|claude-code
```

For `claude-code`, ContextGraph should spawn:

```bash
claude -p \
  --model opus \
  --permission-mode acceptEdits \
  --allowedTools "Read,Edit,Write,Grep,Glob,LSP,Bash(cargo fmt *),Bash(cargo test *),Bash(cargo build *),Bash(rg *),Bash(git diff *),Bash(git status *)" \
  --output-format json \
  --json-schema '<optimizer recommendation schema>' \
  '<rendered optimizer prompt>'
```

Use `--output-format stream-json --verbose` when ContextGraph needs to persist turn-by-turn tool events. Use `--output-format json` when ContextGraph only needs the final structured result plus metadata.

ContextGraph must persist all of these files per invocation:

```text
attempt-N/claude-code-optimizer/
  claude-invocation.json
  claude-stdin-prompt.md
  claude-stdout.json
  claude-stderr.txt
  claude-exit-status.json
  claude-recommendation.json
  project-state-before.json
  project-state-after.json
  git-diff-after.patch
  verification-commands.jsonl
  source-of-truth-readback.json
```

The source of truth for a Claude optimizer run is:

```text
file:<attempt-dir>/claude-code-optimizer/claude-recommendation.json
file:<attempt-dir>/claude-code-optimizer/source-of-truth-readback.json
git:<project-root>#working-tree-diff
```

If Claude edits source, ContextGraph must immediately run:

```bash
rustfmt --edition 2021 crates/context-graph-cli/src/bin/reality-loop.rs
cargo test -p context-graph-cli --bin reality-loop -- --nocapture
cargo build -p context-graph-cli --bin reality-loop
git diff -- crates/context-graph-cli/src/bin/reality-loop.rs
```

Then ContextGraph must read the physical source and diff back before trusting the result.

### Phase 5: Configure Permissions

Use project settings for repeatable permissions.

Example `.claude/settings.json`:

```json
{
  "defaultMode": "acceptEdits",
  "permissions": {
    "allow": [
      "Read",
      "Edit",
      "Write",
      "Grep",
      "Glob",
      "LSP",
      "Bash(rg *)",
      "Bash(git status *)",
      "Bash(git diff *)",
      "Bash(rustfmt *)",
      "Bash(cargo fmt *)",
      "Bash(cargo test *)",
      "Bash(cargo build *)",
      "Bash(ps *)"
    ],
    "deny": [
      "Bash(git reset *)",
      "Bash(git checkout *)",
      "Bash(rm -rf *)",
      "Bash(sudo *)",
      "Read(//home/cabdru/contextgraph/.env)",
      "Edit(//home/cabdru/contextgraph/.env)",
      "Write(//home/cabdru/contextgraph/.env)"
    ]
  }
}
```

Keep destructive git operations denied. ContextGraph can still allow a human to run them manually if needed.

### Phase 6: Add Hooks For Deterministic Reality Guardrails

Hooks should enforce rules that must not depend on the model remembering them.

Recommended hooks:

1. `PreToolUse` for `Edit` and `Write`
   - Block edits to `.env`, official SWE-bench evaluator, benchmark tests, reward specs, and verifier files unless an explicit human override file exists.

2. `PostToolUse` for `Edit` and `Write`
   - Run formatting or a cheap syntax check.
   - Persist changed file path and hash to a project-local artifact.

3. `Stop`
   - Inspect the latest assistant output and require a source-of-truth report before the session stops.

4. `SessionStart`
   - Inject the latest ContextGraph reality-loop root, current target SWE instance, and success predicate.

Example hook intent:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "$CLAUDE_PROJECT_DIR/scripts/claude_guard_pre_edit.sh"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "$CLAUDE_PROJECT_DIR/scripts/claude_record_post_edit.sh"
          }
        ]
      }
    ]
  }
}
```

The hook scripts should produce structured JSON and nonzero/blocking exits when they detect forbidden behavior.

### Phase 7: MCP Integration

Use MCP for ContextGraph-specific tools, not for unrestricted shell access.

Recommended MCP tools to expose:

- `contextgraph.latest_reality_root`
- `contextgraph.read_attempt_summary`
- `contextgraph.read_official_swe_report`
- `contextgraph.read_reward_signal`
- `contextgraph.read_failure_reality`
- `contextgraph.read_problem_reality`
- `contextgraph.compare_attempts`
- `contextgraph.read_trigger_decision`
- `contextgraph.run_reality_smoke`
- `contextgraph.verify_optimizer_edit`

Do not expose raw unbounded filesystem or shell MCP tools as the default. Claude Code already has file and Bash tools; ContextGraph MCP should expose domain-specific durable reality.

Project `.mcp.json` should use environment variable expansion for machine-specific paths:

```json
{
  "mcpServers": {
    "contextgraph-reality": {
      "type": "stdio",
      "command": "${CONTEXTGRAPH_MCP_BIN:-/home/cabdru/contextgraph/target/debug/context-graph-mcp}",
      "args": ["--mode", "reality-loop"],
      "env": {
        "CONTEXTGRAPH_ROOT": "/home/cabdru/contextgraph"
      }
    }
  }
}
```

Claude Code can also run as an MCP server with:

```bash
claude mcp serve
```

That is useful if a separate ContextGraph process wants to call Claude Code's editing tools through MCP, but it adds another permission boundary. Prefer `claude -p` or Agent SDK first.

## Programmatic Prompt Contract

Every Claude optimizer call should include:

```json
{
  "role": "outer_optimizer",
  "inner_actor": "gemma",
  "target": {
    "dataset": "princeton-nlp/SWE-bench_Lite",
    "instance_id": "<instance_id>",
    "success_predicate": "official SWE-bench report resolved_ids contains instance_id"
  },
  "durable_reality": {
    "latest_attempt_summary": "file:<path>",
    "official_report": "file:<path>",
    "reward_signal": "file:<path>",
    "problem_reality": "file:<path>",
    "failure_reality": "file:<path>",
    "trigger_decision": "file:<path>"
  },
  "allowed_work": [
    "improve ContextGraph reality-loop harness",
    "improve Gemma prompt reinjection",
    "improve tool schema and structured edit path",
    "improve reward feature extraction",
    "improve source-of-truth reporting",
    "improve smoke tests and verification"
  ],
  "forbidden_work": [
    "edit official evaluator",
    "edit benchmark tests",
    "weaken verifiers",
    "hide failures",
    "edit secrets",
    "claim local validation as official success"
  ]
}
```

Claude's final structured output should conform to:

```json
{
  "status": "changed|no_change|blocked|error",
  "reason": "string",
  "source_files_changed": ["string"],
  "verification_commands": ["string"],
  "source_of_truth_readbacks": ["string"],
  "next_gemma_prompt_delta": "string",
  "expected_reward_signal_delta": "string",
  "remaining_risks": ["string"]
}
```

## Event-Triggered vs Always-On

Use event-triggered by default.

Always-on only makes sense for a human-supervised session using `/loop` or Monitor while you are actively watching.

Reasons:

- Reality only changes when an attempt, verifier, reward compiler, or source edit writes a new artifact.
- Durable artifacts are replayable; there is no need to pay for idle reasoning.
- Event-triggering keeps each optimizer decision tied to a concrete source-of-truth snapshot.
- Full state verification is simpler when every Claude invocation has a start artifact, end artifact, and diff.

Recommended trigger policies:

```text
smoke: failure
development: failure_or_regression
overnight: failure_or_regression with cost cap
manual debugging: /loop with 5m to 15m interval
```

## Parallel Claude Usage

Do not use 40 source-editing Claude agents.

Use one source-editing optimizer and optional read-only critics.

Safe parallel pattern:

```text
Claude writer: owns source edits
Claude critic 1: reviews prompt reinjection and reward features
Claude critic 2: reviews tool contracts and source-of-truth coverage
Claude critic 3: reviews SWE problem reality and failure diagnosis
Claude critic 4: reviews security/governance/reward-hacking boundaries
```

The writer receives critic reports and makes one coherent source edit.

Use agent teams for manual exploration when the work naturally splits. Start with 3 to 5 teammates. Do not use agent teams for same-file implementation because file conflicts are likely.

## Claude Code Replacement For DeepSeek Optimizer

Current DeepSeek behavior:

```text
ContextGraph writes optimizer input
DeepSeek reads tool outputs
DeepSeek returns structured recommendation
ContextGraph writes recommendation and reads it back
```

Claude Code replacement:

```text
ContextGraph writes optimizer input
ContextGraph runs claude -p or Agent SDK with allowed tools
Claude Code reads source and reality artifacts
Claude Code edits source if needed
Claude Code runs verification commands
ContextGraph reads source, diff, command outputs, and Claude JSON
ContextGraph writes optimizer recommendation and reads it back
```

The important difference is that Claude Code can directly perform the coding-agent work instead of only suggesting it.

## Smoke Test Plan

Before using Claude Code in the long-running loop, run a small smoke test.

### Source Of Truth

```text
/tmp/contextgraph-claude-code-smoke/<run-id>/claude-code-smoke-summary.json
/tmp/contextgraph-claude-code-smoke/<run-id>/physical-smoke-fact.json
/home/cabdru/contextgraph/crates/context-graph-cli/src/bin/reality-loop.rs
```

### Happy Path

1. ContextGraph writes a physical smoke fact:

```json
{
  "label": "claude-code-smoke",
  "observed_number": 4,
  "meaning": "2 + 2 durable smoke fact"
}
```

2. Claude Code is asked to read that file, report `observed_number`, and write structured JSON.
3. ContextGraph reads Claude output and the physical fact back.
4. The smoke passes only if Claude output says `observed_number=4` and the file readback still says `observed_number=4`.

### Edge Cases

1. Missing Claude binary
   - Expected: hard error `CLAUDE_CODE_BINARY_MISSING`
   - Verify: failure artifact exists and contains the attempted command.

2. Missing authentication
   - Expected: hard error `CLAUDE_CODE_AUTH_MISSING`
   - Verify: failure artifact exists and does not print secrets.

3. Invalid JSON output
   - Expected: hard error `CLAUDE_CODE_STRUCTURED_OUTPUT_INVALID`
   - Verify: raw stdout/stderr persisted, final recommendation absent.

4. Forbidden edit attempt
   - Expected: hook blocks edit
   - Verify: hook artifact records path, reason, and blocking decision.

5. Verification failure
   - Expected: run marked `blocked` or `error`, not success
   - Verify: command transcript and source-of-truth readback exist.

## Implementation Checklist

1. Add `--optimizer-provider deepseek|claude-code`.
2. Add Claude provider config:
   - `--claude-code-bin`
   - `--claude-code-model`
   - `--claude-code-timeout`
   - `--claude-code-max-turns`
   - `--claude-code-output-format`
3. Add `run_claude_code_loop_optimizer`.
4. Persist invocation, prompt, stdout, stderr, exit status, recommendation, diff, and readback artifacts.
5. Validate structured JSON recommendation.
6. Run source verification after any Claude edit.
7. Add provider smoke test.
8. Add boundary tests for missing binary, missing auth, invalid JSON, forbidden edit, and failed verification.
9. Add `.claude/loop.md` for manual operator mode.
10. Add `.claude/settings.json` permission policy.
11. Add project hooks for forbidden edits and post-edit reality recording.
12. Add ContextGraph MCP tools for reading durable reality.

## Cost And Model Choice

Use Claude Code model aliases interactively:

```text
/model opus
/model sonnet
/effort high
/effort max
```

For automated runs:

- Use Sonnet for cheap routine analysis.
- Use Opus only when the loop is repeatedly failing, reward is regressing, or source-level architecture changes are required.
- Keep one writer session active per event.
- Use read-only critics only when the failure has multiple plausible root causes.

The cost-efficient policy is:

```text
first failure: Sonnet writer
same failure repeated 2 times: Opus writer
ambiguous root cause: 3 read-only Sonnet critics + 1 Opus writer
official eval infrastructure error: no model call until infrastructure state is fixed
```

## What To Do Next

The best next engineering step is not to abandon the DeepSeek path immediately. Keep DeepSeek as an optimizer provider and add Claude Code as a second provider.

That gives the system:

- direct comparison between model endpoint optimizer and coding-agent optimizer
- fallback-free provider selection by explicit config
- provider-specific source-of-truth artifacts
- no hidden behavior change in the current long-running loop

Recommended first command after implementation:

```bash
cargo run -p context-graph-cli --bin reality-loop -- \
  --optimizer-provider claude-code \
  --smoke-test \
  --official-eval \
  --max-attempts 1
```

Expected success condition:

```text
Claude Code provider smoke artifact exists.
Claude output is valid JSON.
Physical smoke fact readback matches Claude output.
No forbidden edits occurred.
Rust tests/build pass after any source edit.
Official SWE-bench success is still measured only by resolved_ids.
```

## Final Recommendation

Use Claude Code as the outer optimizer, but keep ContextGraph as the loop owner.

Do not make Claude Code the source of truth. Make Claude Code the coding-agent worker that ContextGraph wakes up, constrains, observes, and verifies.

For manual development, use:

```text
/loop
```

For the real autonomous reality loop, use:

```text
ContextGraph event trigger -> claude -p or Agent SDK -> artifacts -> readback -> next Gemma attempt
```

That gives you the benefits of Claude Code's coding-agent harness without giving up the core ContextGraph claim: durable reality, not model confidence, decides whether the system is working.
