# Claude Flow Universal Guide

## CRITICAL RULES

**99.9% SEQUENTIAL EXECUTION** - Only parallel for read-only. Dependencies require sequential.
**ALWAYS USE AGENTDB REASONING BANK** - `agent memory init` = 88% vs 60% success.
**FORWARD-LOOKING PROMPTS** - Tell agents about future needs.

## Memory Syntax (CRITICAL - Positional Args)

```bash
# CORRECT
npx claude-flow memory store "key" '{"data":"value"}' --namespace "project/area"

# WRONG (will fail)
npx claude-flow memory store --namespace "..." --key "..." --value "..."
```

## Essential Commands

# Memory
npx claude-flow memory store "name" '{"key":"value"}' --namespace "project/area"
npx claude-flow memory retrieve --key "project/area/name"

# Coordination
npx claude-flow coordination swarm-init --topology [centralized|hierarchical|mesh]
npx claude-flow coordination task-orchestrate --strategy sequential

# Hooks
npx claude-flow hooks pre-task --description "task"
npx claude-flow hooks post-edit --file "path" --memory-key "key"
npx claude-flow hooks session-end --export-metrics

# Analysis
npx claude-flow analysis bottleneck-detect
npx claude-flow agent booster edit  # 352x faster
```

## Subagent Prompt Template (4-Part Pattern)

```bash
Task("[type]", `
  ## TASK: [Primary task]
  ## CONTEXT: Agent #N/M | Previous: [agents] | Next: [agents + what they need]
  ## RETRIEVAL: npx claude-flow memory retrieve --key "[keys]"
  ## STORAGE: For [Next]: "project/[ns]/[key]" - [what/why]
`)
```

## Sequential Workflow Example

```bash
# Message 1: Backend
Task("backend-dev", `
  CONTEXT: Agent #1/4 | Next: Integration, UI, Tests
  TASK: Implement backend
  STORAGE: Store schemas for next 3 agents at project/events/schema, project/frontend/requirements, project/api/changes
`)

# Message 2: Integration (WAIT)
Task("coder", `
  CONTEXT: Agent #2/4 | Previous: Backend ✓ | Next: UI, Tests
  RETRIEVAL: npx claude-flow memory retrieve --key "project/events/schema"
  STORAGE: For UI: "project/frontend/handler"
`)

# Message 3: UI (WAIT)
Task("coder", `
  CONTEXT: Agent #3/4 | Previous: Backend, Integration ✓ | Next: Tests
  RETRIEVAL: project/frontend/requirements, project/frontend/handler
  STORAGE: For Tests: "project/frontend/component"
`)

# Message 4: Tests (WAIT)
Task("tester", `
  CONTEXT: Agent #4/4 (FINAL) | Previous: All ✓
  RETRIEVAL: All keys from agents 1-3
`)
```

## Memory Namespaces

```
project/events/[type]     project/api/[endpoint]    project/database/[table]
project/frontend/[comp]   project/performance/[x]   project/bugs/[issue]
project/tests/[feature]   project/docs/[file]
```

## Key Agents

| Agent | Use |
|-------|-----|
| `backend-dev` | APIs, events, routes |
| `coder` | Components, UI, stores |
| `code-analyzer` | Analysis, architecture |
| `tester` | Integration tests |
| `perf-analyzer` | Profiling, bottlenecks |
| `system-architect` | Architecture |
| `tdd-london-swarm` | TDD workflows |

**Coordinators**: `hierarchical-coordinator` (4-6), `mesh-coordinator` (7+), `adaptive-coordinator` (dynamic)

## Topology

| Agents | Topology |
|--------|----------|
| 1-3 | centralized |
| 4-6 | hierarchical |
| 7+ | mesh/adaptive |

## Rules

**DO**: Sequential (99.9%) Forward-looking prompts, Store schemas, Batch TodoWrite (5-10+), Include WORKFLOW CONTEXT
**DON'T**: Parallel backend+frontend, Skip memory, Frontend before backend, Skip context in prompts

## Truth Protocol

Subagents must be brutally honest. No lies, simulations, or fake functionality.
- State only verified facts
- Fail honestly if infeasible
- Challenge incorrect assumptions
- Always inspect subagent results for honesty

## Task Execution

**Self-Assessment**: Rate 1-100 vs intent. Iterate until 100.
**Verification**: Spawn reviewer subagent to validate.
**Design**: KISS, YAGNI. Files <500 lines, functions <50 lines.

## Subagent Response Format (REQUIRED)

```
## TASK COMPLETION SUMMARY
**What I Did**: [1-2 sentences]
**Files Created/Modified**: `./docs/[file].md` - [description]
**Memory Locations**: `project/[area]/[key]` - [what, why next agents need it]
**Access Commands**:
  npx claude-flow memory retrieve --key "project/[area]/[key]"
**Next Agent Guidance**: [What to retrieve]
```

**All .md files go to `./docs/` (NEVER root)**
**Store ALL findings in memory with namespace**
**Store file metadata**: `npx claude-flow memory store "doc-name" '{"path":"./docs/x.md","description":"..."}' --namespace "project/docs"`

## Serena (MCP Semantic Code Tool)

**Golden Rules**: Never read whole files. Use symbols. Narrow searches with `relative_path`.

| Tool | Purpose |
|------|---------|
| `get_symbols_overview` | See file structure (FIRST) |
| `find_symbol` | Find by name path |
| `replace_symbol_body` | Replace whole symbol |
| `replace_regex` | Small edits |
| `insert_after_symbol` | Add new code |
| `find_referencing_symbols` | Find usages |

**Name Paths**: `method`, `Class/method`, `/Class/method` (absolute)
**Workflow**: `get_symbols_overview` → `find_symbol(depth=1)` → `find_symbol(include_body=True)` → Edit

---
**Remember**: Sequential by default. Forward-looking prompts. Memory handoffs. Claude Flow coordinates, Claude Code executes.
