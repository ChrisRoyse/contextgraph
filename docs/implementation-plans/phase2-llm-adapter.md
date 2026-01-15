# Phase 2: Universal LLM Adapter Layer

## Status: DELETED (REMOVED FROM SCOPE)

| Attribute | Value |
|-----------|-------|
| **Original Effort** | 110 hours |
| **Current Effort** | 0 hours |
| **Status** | Removed from scope |
| **Deletion Date** | 2026-01-14 |
| **Replaced By** | Phase 3 (Hooks) + Phase 4 (Skills & Subagents) |
| **PRD Reference** | Section 15 (Claude Code Integration - Exclusive Focus) |

---

## Rationale for Removal

Per **PRD Section 15 (Claude Code Integration)**, the Context Graph system targets Claude Code CLI **exclusively**:

> "This project uses **NATIVE Claude Code hooks** configured through `.claude/settings.json` -- **NOT** internal/built-in hooks or custom middleware. This is a fundamental architectural choice that eliminates 71% of complexity by removing the need for a Universal LLM Adapter."
> -- PRD Section 15.1

### Why No Universal Adapter Layer Is Needed

1. **Claude Code Uses Native MCP**: Claude Code has built-in Model Context Protocol support. Our MCP server connects directly without translation.

2. **No Protocol Translation Required**: The original plan proposed adapters for:
   - OpenAI function calling format
   - LangChain tool abstraction
   - Raw LLM prompt templates

   None of these are needed when Claude Code is the exclusive target.

3. **Native Integration Points Exist** (PRD Section 15.2-15.8): Claude Code provides:
   - **10 Hook Events**: SessionStart, PreToolUse, PostToolUse, PermissionRequest, UserPromptSubmit, Stop, SubagentStop, SessionEnd, etc.
   - **Skills**: Filesystem-based capability extensions with progressive disclosure (YAML frontmatter in `.claude/skills/`)
   - **Subagents**: Isolated context windows via the Task tool

   These native mechanisms replace what the adapter layer was designed to provide.

4. **71% Effort Reduction Breakdown** (PRD Section 15.10):

   | Approach | Effort | Complexity | Maintenance |
   |----------|--------|------------|-------------|
   | Universal LLM Adapter (original) | ~110h | Very High | Cross-provider compatibility |
   | Native Claude Code Hooks | ~25h | Low | Claude team maintains hook system |
   | Skills & Subagents | ~20h | Low | Standard markdown files |
   | **Total Native** | **~45h** | **Low** | **Minimal** |

   **Savings**: 110h - 45h = 65h eliminated (~59% direct savings)
   **Including maintenance burden**: ~71% total effort reduction over project lifetime

---

## Original Scope (Historical Reference)

The deleted Phase 2 originally planned to implement:

### Proposed Components (Not Implemented)

| Component | Purpose | Hours |
|-----------|---------|-------|
| `LlmAdapter` trait | Core adapter interface | 3 |
| `ClaudeAdapter` | Native MCP pass-through | 9 |
| `OpenAIAdapter` | Function calling translation | 14 |
| `LangChainAdapter` | BaseTool format conversion | 12 |
| `RawLlmAdapter` | Prompt template generation | 14 |
| `ConsciousnessStateFormatter` | Multi-format state export | 4 |
| Streaming support | Workspace event streaming | 12 |
| MCP tool integration | Adapter management tools | 13 |
| Documentation | Usage guides | 16 |
| **Total** | | **110** |

### Proposed Architecture (Not Implemented)

```
LLM Applications
  Claude (MCP) / OpenAI (func) / LangChain (tool) / Raw (prompt)
                          |
              Universal LLM Adapter Layer
                 LlmAdapterManager
    ClaudeAdapter | OpenAIAdapter | LangChainAdapter | RawAdapter
                          |
                ConsciousnessStateFormatter
                          |
                    MCP Server Layer
```

This architecture was designed to allow ANY LLM to connect to Context Graph. With the exclusive Claude Code focus, this complexity is unnecessary.

---

## Replacement: Phase 3 (Hooks) + Phase 4 (Skills and Subagents)

The functionality that Phase 2 would have provided is now achieved through **two separate phases** using Claude Code's native architecture:

### Phase 3: Native Hooks Replace Session/Lifecycle Management

**Implementation Plan**: `docs/implementation-plans/phase3-integration-hooks.md`
**Budget**: 25 hours

| Phase 2 Concept | Phase 3 Replacement |
|-----------------|---------------------|
| Session lifecycle adapters | Native hooks (SessionStart, SessionEnd) in `.claude/settings.json` |
| Tool validation | PreToolUse/PostToolUse hooks with matchers |
| Consciousness state injection | UserPromptSubmit hooks |
| Permission gating | PermissionRequest hook |
| Multi-LLM session coordination | Stop/SubagentStop hooks |

Phase 3 implements shell script executors that call `context-graph-cli` commands, providing:
- PreToolUse < 50ms latency target
- Async PostToolUse with background processing
- Exit code 2 error handling with graceful degradation

### Phase 4: Skills and Subagents Replace Tool Translation

**Implementation Plan**: `docs/implementation-plans/phase4-skills-subagents.md`
**Budget**: 20 hours

| Phase 2 Concept | Phase 4 Replacement |
|-----------------|---------------------|
| Tool definition conversion | Native MCP tool discovery |
| LLM-specific formatting | Skills with YAML frontmatter (`.claude/skills/`) |
| Progressive capability loading | Skill progressive disclosure (3 levels) |
| Context injection | consciousness, memory-inject, dream-consolidation skills |

| Phase 2 Concept | Phase 4 Replacement |
|-----------------|---------------------|
| Parallel tool execution | Background subagents (`run_in_background: true`) |
| Isolated processing | Subagent isolated context windows |
| State transfer between LLMs | Subagent result synthesis |
| Multi-LLM coordination | identity-guardian, memory-specialist, consciousness-explorer subagents |

---

## References

### PRD References

- **PRD Section 15**: Claude Code Integration (exclusive platform focus)
  - Section 15.1: Native Hook Architecture
  - Section 15.2: Hook Configuration (`.claude/settings.json`)
  - Section 15.3: Hook Shell Scripts
  - Section 15.4-15.5: Hook Performance Requirements & Session Identity
  - Section 15.6: CLI Commands
  - Section 15.7: Skills (YAML-Based)
  - Section 15.8: Subagents
  - Section 15.9: Autonomous Operation
  - Section 15.10: Why Native Hooks vs Built-In (71% effort reduction justification)

### Implementation Plans

- **Phase 3 (Hooks)**: `docs/implementation-plans/phase3-integration-hooks.md` (25 hours)
- **Phase 4 (Skills & Subagents)**: `docs/implementation-plans/phase4-skills-subagents.md` (20 hours)

### Documentation

- **Claude Code Skills Format**: `docs2/claudeskills.md`
- **Claude Code Hooks Format**: `docs2/claudehooks.md`
- **Context Graph PRD**: `docs2/contextprd.md`

---

## Migration Notes

Any code or designs referencing "Universal LLM Adapter" concepts should be updated to use:

### Old Reference -> New Implementation

| Old Concept | New Implementation | Phase |
|-------------|-------------------|-------|
| `LlmAdapter` trait | Not needed - Claude Code uses MCP directly | N/A |
| `ClaudeAdapter` | Native `.claude/settings.json` hooks | Phase 3 |
| `OpenAIAdapter` | **REMOVED** - No OpenAI support | N/A |
| `LangChainAdapter` | **REMOVED** - No LangChain support | N/A |
| `RawLlmAdapter` | **REMOVED** - No raw prompt support | N/A |
| `ConsciousnessStateFormatter` | `context-graph-cli consciousness brief` | Phase 3 |
| Tool definition conversion | Native MCP `ToolDefinition` struct | MCP Server |
| Session lifecycle adapters | SessionStart/SessionEnd hooks | Phase 3 |
| Consciousness state injection | PreToolUse/UserPromptSubmit hooks | Phase 3 |
| Capability extension | Skills in `.claude/skills/` | Phase 4 |
| Parallel work coordination | Subagents via Task tool | Phase 4 |

### If You Find Old References

1. **In code**: Remove any `LlmAdapter` imports or implementations
2. **In documentation**: Update to reference Phase 3/4 instead of Phase 2
3. **In architecture diagrams**: Remove "Universal Adapter Layer" - Claude Code connects directly to MCP server
4. **In PRD references**: Section 15 documents the exclusive Claude Code focus

### Key Architectural Change

```
OLD (Phase 2 design):
  Multiple LLMs -> Universal Adapter Layer -> MCP Server -> Context Graph

NEW (Phase 3+4 design):
  Claude Code -> Native Hooks (.claude/settings.json)
             -> Skills (.claude/skills/)
             -> Subagents (Task tool)
             -> MCP Server -> Context Graph
```

---

*This document retained for historical reference. Phase 2 work should NOT be initiated. All functionality is covered by Phase 3 (hooks) and Phase 4 (skills/subagents).*
