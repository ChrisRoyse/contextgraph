# Settings & MCP Config Spec

## Files

- `/home/cabdru/contextgraph/.claude/settings.json` — project settings (committed). Defines defaults shared by every operator.
- `/home/cabdru/contextgraph/.claude/settings.local.json` — operator-local overrides (gitignored). Already exists with `enableAllProjectMcpServers: true`.
- `/home/cabdru/contextgraph/.mcp.json` — project MCP config (committed). Defines the `cgreality` server.
- `/home/cabdru/contextgraph/.claude/loop.md` — `/loop` default prompt; see [11_LOOP_PROMPT](11_LOOP_PROMPT.md).
- `/home/cabdru/contextgraph/CLAUDE.md` — project memory (already exists; we add an addendum); see [10_CLAUDE_MD_PROJECT_MEMORY](10_CLAUDE_MD_PROJECT_MEMORY.md).

## `.claude/settings.json` — committed project settings

```json
{
  "$schema": "https://code.claude.com/schemas/settings.json",
  "model": "opus",
  "defaultMode": "acceptEdits",
  "enableAllProjectMcpServers": true,
  "includeCoAuthoredBy": true,
  "permissions": {
    "allow": [
      "Read",
      "Glob",
      "Grep",
      "LS",
      "WebFetch",
      "WebSearch",
      "TodoWrite",
      "Task",
      "Edit(crates/context-graph-cli/src/**)",
      "Edit(crates/context-graph-mcp/src/handlers/tools/reality_loop/**)",
      "Edit(scripts/cc/**)",
      "Edit(.claude/loop.md)",
      "Edit(.claude/agents/cc-reality-*.md)",
      "Edit(.claude/skills/ccreality-*/**)",
      "Edit(docs/ccreality/**)",
      "Edit(docs/reality/**)",
      "Write(scripts/cc/**)",
      "Write(.claude/agents/cc-reality-*.md)",
      "Write(.claude/skills/ccreality-*/**)",
      "Write(docs/ccreality/**)",
      "Write(docs/reality/**)",
      "Bash(cargo fmt *)",
      "Bash(cargo check *)",
      "Bash(cargo test *)",
      "Bash(cargo build *)",
      "Bash(cargo clippy *)",
      "Bash(rustfmt *)",
      "Bash(rg *)",
      "Bash(git status *)",
      "Bash(git diff *)",
      "Bash(git log *)",
      "Bash(git show *)",
      "Bash(git rev-parse *)",
      "Bash(ollama ps)",
      "Bash(nvidia-smi)",
      "Bash(jq *)",
      "Bash(ls *)",
      "Bash(cat *)",
      "Bash(head *)",
      "Bash(tail *)",
      "Bash(sha256sum *)",
      "mcp__cgreality__.*"
    ],
    "deny": [
      "Read(/home/cabdru/contextgraph/.env)",
      "Edit(/home/cabdru/contextgraph/.env)",
      "Edit(/home/cabdru/contextgraph/.env.*)",
      "Write(/home/cabdru/contextgraph/.env)",
      "Edit(/home/cabdru/contextgraph/swebench/**)",
      "Edit(/home/cabdru/contextgraph/tests/oracle_**)",
      "Edit(/home/cabdru/contextgraph/crates/context-graph-benchmark/datasets/swe_bench_*)",
      "Edit(/home/cabdru/contextgraph/crates/*/src/reward_specs/**)",
      "Edit(/home/cabdru/contextgraph/crates/*/src/verifiers/**)",
      "Bash(git push *)",
      "Bash(git push --force *)",
      "Bash(git reset --hard *)",
      "Bash(git checkout -- *)",
      "Bash(git clean -f *)",
      "Bash(git branch -D *)",
      "Bash(git rebase *)",
      "Bash(rm -rf *)",
      "Bash(sudo *)",
      "Bash(curl *)",
      "Bash(wget *)",
      "Bash(npm publish *)",
      "Bash(cargo publish *)",
      "Bash(docker rm -f *)",
      "Bash(docker rmi -f *)"
    ]
  },
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume|clear",
        "hooks": [
          {
            "type": "command",
            "timeout": 10,
            "command": "\"$CLAUDE_PROJECT_DIR\"/scripts/cc/hook_session_start.sh"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "timeout": 8,
            "command": "\"$CLAUDE_PROJECT_DIR\"/scripts/cc/hook_user_prompt_submit.sh"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "timeout": 5,
            "command": "\"$CLAUDE_PROJECT_DIR\"/scripts/cc/hook_pre_tool_use_edit.sh"
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
            "timeout": 90,
            "command": "\"$CLAUDE_PROJECT_DIR\"/scripts/cc/hook_post_tool_use_edit.sh"
          }
        ]
      }
    ],
    "PreCompact": [
      {
        "matcher": "auto|manual",
        "hooks": [
          {
            "type": "command",
            "timeout": 8,
            "command": "\"$CLAUDE_PROJECT_DIR\"/scripts/cc/hook_pre_compact.sh"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "timeout": 10,
            "command": "\"$CLAUDE_PROJECT_DIR\"/scripts/cc/hook_stop.sh"
          }
        ]
      }
    ]
  },
  "env": {
    "CONTEXTGRAPH_ROOT": "/home/cabdru/contextgraph",
    "CGREALITY_GOVERNED_PATHS": "/home/cabdru/contextgraph/config/ccreality_governed_paths.json",
    "CGREALITY_RECOMMENDATION_SCHEMA": "/home/cabdru/contextgraph/config/ccreality_recommendation_schema.json"
  }
}
```

### Notes on the permissions

- The `Edit(...)` and `Write(...)` allow rules pin Claude to the project subtree. `defaultMode: "acceptEdits"` lets allowed edits proceed without prompting.
- The `Bash(cargo fmt *)` shape uses Claude Code's prefix matching — the **space before** `*` matters per Claude Code permission docs (it differentiates `cargo fmt` prefix from `cargofmt` substring).
- `mcp__cgreality__.*` is a regex (Claude Code uses regex when the matcher contains non-alphanumerics). It allows every tool in the `cgreality` server. Per the MCP spec doc, all reality access is consolidated under one server, so this rule covers every analyze/interact/alter call.
- The deny block is the **deterministic** reality-boundary enforcement. Hooks layer additional context-aware enforcement on top, but the deny block is unbypassable per Claude Code permission precedence.

### Why `model: opus` as default

Per [15_COST_AND_MODEL_POLICY](15_COST_AND_MODEL_POLICY.md), the optimizer should default to Opus because:
- The work is harness optimization (architecture-level).
- Each turn is event-triggered (low frequency), so cost is bounded.
- Opus's better reasoning materially improves the structured recommendation quality.
Sonnet is still used for the read-only critic subagents.

### Why `defaultMode: "acceptEdits"`

The reality drip + hook-based reality boundary already enforces what Claude can edit. Permission-prompting on every governed Edit would defeat the autonomous loop without adding safety.

### Why `enableAllProjectMcpServers: true` (already in settings.local.json)

Loads the `cgreality` server defined in `.mcp.json` automatically. Without this, every operator would have to approve the server.

## `.mcp.json` — project MCP servers

```json
{
  "$schema": "https://code.claude.com/schemas/mcp.json",
  "mcpServers": {
    "cgreality": {
      "type": "stdio",
      "command": "${CONTEXTGRAPH_ROOT}/target/debug/context-graph-mcp",
      "args": ["--mode", "reality-loop"],
      "env": {
        "CONTEXTGRAPH_ROOT": "${CONTEXTGRAPH_ROOT}",
        "CGREALITY_RUNTIME_ROOT_HINT": "${HOME}/.cache/cgreality/active_runtime_root",
        "CGREALITY_GOVERNED_PATHS": "${CGREALITY_GOVERNED_PATHS}",
        "CGREALITY_RECOMMENDATION_SCHEMA": "${CGREALITY_RECOMMENDATION_SCHEMA}",
        "RUST_LOG": "cgreality=info,context_graph_mcp=info"
      }
    }
  }
}
```

`${...}` expansion is supported by Claude Code's MCP config loader. The `CONTEXTGRAPH_ROOT` env var is exported by `.claude/settings.json`.

## Operator-local overrides (`.claude/settings.local.json`)

Existing file:

```json
{
  "enabledMcpjsonServers": ["claude-flow", "context-graph"],
  "enableAllProjectMcpServers": true
}
```

Will continue to work. Add `"cgreality"` to `enabledMcpjsonServers` if `enableAllProjectMcpServers` is ever set false. No other changes required.

## Why we don't ship `--bare` mode

Claude Code's `--bare` mode skips hooks/skills/MCP/CLAUDE.md auto-discovery. That defeats every reality-drip mechanism we're building. We want full mode for both interactive and `claude -p` invocations of the optimizer.

The only place `--bare` is relevant is in non-optimizer scripts (e.g., a CI lint script that just wants Claude to parse a diff). Those live elsewhere.

## Permission audit checklist

Before merging, walk the allow/deny list against:
- [ ] Every Bash command the engine or hook scripts call themselves (don't allow Claude things only the harness should call).
- [ ] Every MCP tool we want exposed.
- [ ] Every file path we never want Claude to touch (the deny list — confirm cover for `.env`, oracle, evaluator, reward specs, verifiers).
- [ ] Read-only commands list (`Bash(ls *)`, `Bash(cat *)`, etc.) — kept tight; we DON'T allow `Bash(find *)` to avoid CPU exhaustion and we DON'T allow `Bash(mkdir *)` because `acceptEdits` mode already auto-approves it for governed paths.
