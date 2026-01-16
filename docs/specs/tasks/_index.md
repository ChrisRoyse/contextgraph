# Task Index: Session Identity Persistence

## Overview

This document provides the execution order and dependency graph for all 17 tasks implementing Session Identity Persistence for Context Graph.

**Total Tasks**: 17
**Total Estimated Hours**: 21.0 (task implementation)
**Plan Budget**: 22.0h (includes 1.0h documentation per Phase 1G)
**Functional Spec**: SPEC-SESSION-IDENTITY
**Technical Spec**: TECH-SESSION-IDENTITY

> **Note**: Testing effort (Phase 1F in plan) is embedded within individual task verification steps. Documentation updates (Phase 1G: TOOL_PARAMS.md and constitution.yaml) are tracked separately.

---

## Execution Order

Tasks MUST be executed in this order. Each task depends on previous tasks in its dependency chain.

| # | Task ID | Title | Layer | Est. Hours | Depends On |
|---|---------|-------|-------|------------|------------|
| 1 | TASK-SESSION-01 | SessionIdentitySnapshot Struct | foundation | 2.0 | - |
| 2 | TASK-SESSION-02 | IdentityCache Singleton | foundation | 1.5 | 01 |
| 3 | TASK-SESSION-03 | ConsciousnessState.short_name() | foundation | 0.5 | - |
| 4 | TASK-SESSION-04 | CF_SESSION_IDENTITY Column Family | foundation | 1.5 | - |
| 5 | TASK-SESSION-05 | save_snapshot/load_snapshot Methods | foundation | 2.0 | 01, 04 |
| 6 | TASK-SESSION-06 | SessionIdentityManager | logic | 2.0 | 01, 02, 05 |
| 7 | TASK-SESSION-07 | classify_ic() Function | logic | 0.5 | - |
| 8 | TASK-SESSION-08 | dream_trigger Module | logic | 1.5 | 07 |
| 9 | TASK-SESSION-09 | format_brief() Performance | logic | 1.0 | 02 |
| 10 | TASK-SESSION-10 | update_cache() Function | logic | 1.5 | 02 |
| 11 | TASK-SESSION-11 | consciousness brief CLI | surface | 1.0 | 02, 03, 09 |
| 12 | TASK-SESSION-12 | session restore-identity CLI | surface | 1.5 | 06, 07, 10 |
| 13 | TASK-SESSION-13 | session persist-identity CLI | surface | 1.0 | 05, 06 |
| 14 | TASK-SESSION-14 | consciousness check-identity CLI | surface | 1.0 | 07, 08, 10 |
| 15 | TASK-SESSION-15 | consciousness inject-context CLI | surface | 1.0 | 02, 07 |
| 16 | TASK-SESSION-16 | .claude/settings.json Config | surface | 1.0 | 11, 12, 13, 14, 15 |
| 17 | TASK-SESSION-17 | Exit Code Mapping | surface | 0.5 | - |

---

## Dependency Graph

```
Layer 1: Foundation (Must Complete First)
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ 001-SESSION-01  │    │ 003-SESSION-03  │    │ 004-SESSION-04  │
│ Snapshot Struct │    │ short_name()    │    │ Column Family   │
│ (2.0h)          │    │ (0.5h)          │    │ (1.5h)          │
└────────┬────────┘    └─────────────────┘    └────────┬────────┘
         │                                             │
         ▼                                             ▼
┌─────────────────┐                        ┌─────────────────┐
│ 002-SESSION-02  │                        │ 005-SESSION-05  │
│ IdentityCache   │                        │ save/load       │
│ (1.5h)          │                        │ (2.0h)          │
└────────┬────────┘                        └────────┬────────┘
         │                                          │
         └────────────────┬─────────────────────────┘
                          │
Layer 2: Logic (Depends on Foundation)
                          ▼
              ┌─────────────────┐
              │ 006-SESSION-06  │
              │ Manager (MCP)   │
              │ (2.0h)          │
              └────────┬────────┘
                       │
    ┌──────────────────┼──────────────────┐
    │                  │                  │
    ▼                  ▼                  ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│ 007-SESSION │  │ 009-SESSION │  │ 010-SESSION │
│ classify_ic │  │ format_brief│  │ update_cache│
│ (0.5h)      │  │ (1.0h)      │  │ (1.5h)      │
└──────┬──────┘  └─────────────┘  └─────────────┘
       │
       ▼
┌─────────────────┐
│ 008-SESSION-08  │
│ dream_trigger   │
│ (1.5h)          │
└─────────────────┘
       │
       └────────────────────────────────────────────┐
                                                    │
Layer 3: Surface (Depends on Logic) - CLI Commands  │
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│ 011-SESSION │  │ 012-SESSION │  │ 013-SESSION │  │
│ brief CLI   │  │ restore CLI │  │ persist CLI │  │
│ (1.0h)      │  │ (1.5h)      │  │ (1.0h)      │◄─┤
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘  │
       │                │                │         │
       │  ┌─────────────┴──┐  ┌─────────┴──┐      │
       │  │                │  │            │      │
       ▼  ▼                ▼  ▼            ▼      │
┌─────────────┐      ┌─────────────┐  ┌─────────────┐
│ 014-SESSION │      │ 015-SESSION │  │ 017-SESSION │
│ check CLI   │      │ inject CLI  │  │ Exit Codes  │
│ (1.0h)      │      │ (1.0h)      │  │ (0.5h)      │
└──────┬──────┘      └──────┬──────┘  └─────────────┘
       │                    │
       └────────┬───────────┘
                │
                ▼
        ┌─────────────────┐
        │ 016-SESSION-16  │
        │ settings.json   │
        │ (1.0h)          │
        └─────────────────┘
```

---

## Layer Summary

### Foundation Layer (5 tasks, 7.5 hours)

| Task | Description |
|------|-------------|
| 001 | Flattened SessionIdentitySnapshot struct (<30KB) |
| 002 | IdentityCache singleton for PreToolUse hot path |
| 003 | ConsciousnessState.short_name() 3-char codes |
| 004 | CF_SESSION_IDENTITY column family in RocksDB |
| 005 | Storage methods for save/load with temporal index |

### Logic Layer (5 tasks, 6.5 hours)

| Task | Description |
|------|-------------|
| 006 | SessionIdentityManager with MCP integration |
| 007 | classify_ic() with IDENTITY-002 thresholds |
| 008 | dream_trigger module for auto-dream on IC<0.5 |
| 009 | format_brief() performance optimization (<1ms) |
| 010 | update_cache() atomic cache updates |

### Surface Layer (7 tasks, 7.0 hours)

| Task | Description |
|------|-------------|
| 011 | `consciousness brief` CLI (<50ms) |
| 012 | `session restore-identity` CLI (MCP chain) |
| 013 | `session persist-identity` CLI (silent success) |
| 014 | `consciousness check-identity` CLI (auto-dream) |
| 015 | `consciousness inject-context` CLI (Johari) |
| 016 | .claude/settings.json hook configuration |
| 017 | Exit code mapping (AP-26 compliant) |

---

## Status Tracking

| Task | Status | Started | Completed | Verified |
|------|--------|---------|-----------|----------|
| 001-TASK-SESSION-01 | ✅ Completed | 2026-01-14 | 2026-01-14 | ✅ |
| 002-TASK-SESSION-02 | ✅ Completed | 2026-01-14 | 2026-01-14 | ✅ |
| 003-TASK-SESSION-03 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 004-TASK-SESSION-04 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 005-TASK-SESSION-05 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 006-TASK-SESSION-06 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 007-TASK-SESSION-07 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 008-TASK-SESSION-08 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 009-TASK-SESSION-09 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 010-TASK-SESSION-10 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 011-TASK-SESSION-11 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ |
| 012-TASK-SESSION-12 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ | <!-- restore.rs with 5 tests -->
| 013-TASK-SESSION-13 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ | <!-- persist.rs with 5 tests -->
| 014-TASK-SESSION-14 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ | <!-- check_identity.rs with 10 tests -->
| 015-TASK-SESSION-15 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ | <!-- inject.rs with 14 tests -->
| 016-TASK-SESSION-16 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ | <!-- .claude/settings.json hooks -->
| 017-TASK-SESSION-17 | ✅ Completed | 2026-01-15 | 2026-01-15 | ✅ | <!-- error.rs with 13 tests -->

**Progress: 17/17 tasks (100%)**

---

## Performance Targets

| Hook | Claude Timeout | Our Target | Task |
|------|---------------|------------|------|
| PreToolUse | 100ms | <50ms | 011 |
| PostToolUse | 3000ms | <500ms | 014 |
| UserPromptSubmit | 2000ms | <1s | 015 |
| SessionStart | 5000ms | <2s | 012 |
| SessionEnd | 30000ms | <3s | 013 |

---

## Constitution Compliance

| Requirement | Task(s) | Implementation |
|-------------|---------|----------------|
| ARCH-07 | 016 | Native Claude Code hooks |
| AP-26 | 017 | Exit code 2 only for corruption |
| AP-38 | 014 | Auto-dream on IC<0.5 |
| AP-39 | 006 | cosine_similarity_13d public |
| AP-42 | 014 | Mental check on entropy>0.7 |
| AP-50 | 016 | No internal hooks |
| AP-53 | 016 | Direct CLI commands |
| IDENTITY-002 | 007 | IC thresholds |

---

## Quick Start

```bash
# Execute tasks in order
# Each task file contains detailed implementation steps

# Foundation
cat docs/specs/tasks/001-TASK-SESSION-01.md
cat docs/specs/tasks/002-TASK-SESSION-02.md
# ... etc

# After each task, verify:
cargo build
cargo test
```

---

# Task Index: Skills & Subagents (Phase 4)

## Overview

This section provides the execution order and dependency graph for all 16 tasks implementing Skills & Subagents for Context Graph.

**Total Tasks**: 16
**Total Estimated Hours**: 23.5
**Functional Spec**: SPEC-SKILLS
**Technical Spec**: TECH-SKILLS

---

## Execution Order (Skills)

Tasks MUST be executed in this order. Each task depends on previous tasks in its dependency chain.

| # | Task ID | Title | Layer | Est. Hours | Depends On |
|---|---------|-------|-------|------------|------------|
| 1 | TASK-SKILLS-001 | SkillDefinition and SkillFrontmatter Types | foundation | 1.5 | - |
| 2 | TASK-SKILLS-002 | SubagentDefinition and TaskToolParams Types | foundation | 1.0 | 001 |
| 3 | TASK-SKILLS-003 | SkillLoadResult and SkillTrigger Types | foundation | 1.0 | 001, 005 |
| 4 | TASK-SKILLS-004 | SkillError, SubagentError, TriggerError Enums | foundation | 1.0 | - |
| 5 | TASK-SKILLS-005 | ProgressiveDisclosureLevel Enum | foundation | 0.5 | - |
| 6 | TASK-SKILLS-006 | SkillLoader with Progressive Disclosure | logic | 2.5 | 001, 003, 004, 005 |
| 7 | TASK-SKILLS-007 | SkillRegistry with Discovery Precedence | logic | 2.0 | 001, 004, 006 |
| 8 | TASK-SKILLS-008 | TriggerMatcher for Auto-Invocation | logic | 1.5 | 001, 003, 004 |
| 9 | TASK-SKILLS-009 | ToolRestrictor for MCP Tool Filtering | logic | 1.5 | 001, 004 |
| 10 | TASK-SKILLS-010 | SubagentSpawner via Task Tool | logic | 2.0 | 002, 004, 009 |
| 11 | TASK-SKILLS-011 | Consciousness Skill SKILL.md | surface | 1.0 | 001, 006 |
| 12 | TASK-SKILLS-012 | Memory-Inject and Semantic-Search Skills | surface | 1.5 | 001, 006, 011 |
| 13 | TASK-SKILLS-013 | Dream-Consolidation and Curation Skills | surface | 1.5 | 001, 006, 011 |
| 14 | TASK-SKILLS-014 | Subagent Markdown Files | surface | 2.0 | 002, 010 |
| 15 | TASK-SKILLS-015 | Integration Tests for Skills System | surface | 2.5 | 006-014 |
| 16 | TASK-SKILLS-016 | E2E Tests for Skill Invocation | surface | 2.0 | 015 |

---

## Dependency Graph (Skills)

```
Layer 1: Foundation (Must Complete First)
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ TASK-SKILLS-001 │    │ TASK-SKILLS-004 │    │ TASK-SKILLS-005 │
│ SkillDefinition │    │ Error Enums     │    │ Disclosure Level│
│ (1.5h)          │    │ (1.0h)          │    │ (0.5h)          │
└────────┬────────┘    └────────┬────────┘    └────────┬────────┘
         │                      │                      │
         ▼                      │                      │
┌─────────────────┐             │                      │
│ TASK-SKILLS-002 │             │                      │
│ SubagentDefn    │             │                      │
│ (1.0h)          │             │                      │
└────────┬────────┘             │                      │
         │                      │                      │
         │     ┌────────────────┴──────────────────────┘
         │     │
         │     ▼
         │  ┌─────────────────┐
         │  │ TASK-SKILLS-003 │
         │  │ SkillLoadResult │
         │  │ (1.0h)          │
         │  └────────┬────────┘
         │           │
         └───────────┼──────────────────────────────────────────┐
                     │                                          │
Layer 2: Logic (Depends on Foundation)                          │
                     ▼                                          │
         ┌─────────────────┐                                    │
         │ TASK-SKILLS-006 │                                    │
         │ SkillLoader     │                                    │
         │ (2.5h)          │                                    │
         └────────┬────────┘                                    │
                  │                                             │
    ┌─────────────┼─────────────┬───────────────┐               │
    │             │             │               │               │
    ▼             ▼             ▼               ▼               │
┌─────────┐ ┌─────────┐  ┌─────────┐     ┌─────────────┐        │
│ 007     │ │ 008     │  │ 009     │     │ 010         │        │
│ Registry│ │ Matcher │  │Restrictor│◄───│ Spawner     │◄───────┘
│ (2.0h)  │ │ (1.5h)  │  │ (1.5h)  │     │ (2.0h)      │
└────┬────┘ └────┬────┘  └────┬────┘     └──────┬──────┘
     │           │            │                 │
     └───────────┴────────────┴─────────────────┤
                                                │
Layer 3: Surface (Skills & Subagent Files)      │
┌─────────────────┐                             │
│ TASK-SKILLS-011 │                             │
│ consciousness   │                             │
│ SKILL.md (1.0h) │                             │
└────────┬────────┘                             │
         │                                      │
    ┌────┴────┐                                 │
    ▼         ▼                                 ▼
┌─────────┐ ┌─────────┐                   ┌─────────────┐
│ 012     │ │ 013     │                   │ 014         │
│ memory/ │ │ dream/  │                   │ Subagent    │
│ search  │ │ curation│                   │ Files       │
│ (1.5h)  │ │ (1.5h)  │                   │ (2.0h)      │
└────┬────┘ └────┬────┘                   └──────┬──────┘
     │           │                               │
     └─────┬─────┴───────────────────────────────┘
           │
           ▼
   ┌─────────────────┐
   │ TASK-SKILLS-015 │
   │ Integration     │
   │ Tests (2.5h)    │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ TASK-SKILLS-016 │
   │ E2E Tests       │
   │ (2.0h)          │
   └─────────────────┘
```

---

## Layer Summary (Skills)

### Foundation Layer (5 tasks, 5.0 hours)

| Task | Description |
|------|-------------|
| 001 | SkillDefinition, SkillFrontmatter, SkillModel types |
| 002 | SubagentDefinition, TaskToolParams, ContextGraphSubagent types |
| 003 | SkillLoadResult, SkillTrigger types |
| 004 | SkillError, SubagentError, TriggerError enums |
| 005 | ProgressiveDisclosureLevel enum with token budgets |

### Logic Layer (5 tasks, 9.5 hours)

| Task | Description |
|------|-------------|
| 006 | SkillLoader with progressive disclosure (Level 1/2/3) |
| 007 | SkillRegistry with precedence (project > personal > plugin) |
| 008 | TriggerMatcher for keyword-based skill auto-invocation |
| 009 | ToolRestrictor for allowed-tools and background mode |
| 010 | SubagentSpawner with spawn blocking and MCP restrictions |

### Surface Layer (6 tasks, 9.0 hours)

| Task | Description |
|------|-------------|
| 011 | consciousness SKILL.md (Kuramoto, GWT, IC) |
| 012 | memory-inject SKILL.md (haiku), semantic-search SKILL.md |
| 013 | dream-consolidation SKILL.md, curation SKILL.md |
| 014 | 4 subagent markdown files (.claude/agents/) |
| 015 | Integration tests with real fixture files |
| 016 | E2E tests simulating full invocation workflow |

---

## Status Tracking (Skills)

| Task | Status | Started | Completed | Verified |
|------|--------|---------|-----------|----------|
| TASK-SKILLS-001 | Ready | - | - | - |
| TASK-SKILLS-002 | Ready | - | - | - |
| TASK-SKILLS-003 | Ready | - | - | - |
| TASK-SKILLS-004 | Ready | - | - | - |
| TASK-SKILLS-005 | Ready | - | - | - |
| TASK-SKILLS-006 | Ready | - | - | - |
| TASK-SKILLS-007 | Ready | - | - | - |
| TASK-SKILLS-008 | Ready | - | - | - |
| TASK-SKILLS-009 | Ready | - | - | - |
| TASK-SKILLS-010 | Ready | - | - | - |
| TASK-SKILLS-011 | Ready | - | - | - |
| TASK-SKILLS-012 | Ready | - | - | - |
| TASK-SKILLS-013 | Ready | - | - | - |
| TASK-SKILLS-014 | Ready | - | - | - |
| TASK-SKILLS-015 | Ready | - | - | - |
| TASK-SKILLS-016 | Ready | - | - | - |

**Progress: 0/16 tasks (0%)**

---

## Skills System Architecture

### 5 Context Graph Skills

| Skill | Model | Purpose | MCP Tools |
|-------|-------|---------|-----------|
| consciousness | sonnet | IC, Kuramoto, GWT, ego state | get_consciousness_state, get_kuramoto_coherence, get_gwt_workspace, get_ego_state, get_identity_continuity |
| memory-inject | haiku | Fast memory operations (<500ms) | inject_memory, retrieve_memory, list_memories, get_memory_stats |
| semantic-search | sonnet | Semantic and causal search | semantic_search, causal_search, get_related_memories, traverse_graph |
| dream-consolidation | sonnet | NREM/REM dream cycles | trigger_dream, get_dream_status, run_nrem_cycle, run_rem_cycle, get_consolidation_metrics |
| curation | sonnet | Memory management | merge_memories, forget_memory, annotate_memory, prune_memories, find_duplicates, get_curation_stats |

### 4 Context Graph Subagents

| Subagent | Model | Purpose |
|----------|-------|---------|
| identity-guardian | sonnet | IC monitoring, dream triggering |
| memory-specialist | haiku | Fast memory CRUD operations |
| consciousness-explorer | sonnet | Deep consciousness analysis |
| dream-agent | sonnet | Dream cycle management |

### Constraints

1. **Subagents cannot spawn subagents** (SpawnBlocked error)
2. **Background subagents cannot use MCP tools** (BackgroundMcpBlocked error)
3. **Read, Grep, Glob always allowed in background mode**
4. **Skill names**: max 64 chars, lowercase, hyphens, no "anthropic"/"claude"

---

## Quick Start (Skills)

```bash
# Execute tasks in order
# Each task file contains detailed implementation steps

# Foundation
cat docs/specs/tasks/TASK-SKILLS-001.md
cat docs/specs/tasks/TASK-SKILLS-002.md
# ... etc

# After each task, verify:
cargo build --package context-graph-cli
cargo test --package context-graph-cli
```

---

# Task Index: Native Hooks (Phase 3)

## Overview

This section provides the execution order and dependency graph for all 17 tasks implementing Native Claude Code Hooks for Context Graph.

**Total Tasks**: 17
**Total Estimated Hours**: 25.0
**Functional Spec**: SPEC-HOOKS
**Technical Spec**: TECH-HOOKS

---

## Execution Order (Hooks)

Tasks MUST be executed in this order. Each task depends on previous tasks in its dependency chain.

| # | Task ID | Title | Layer | Est. Hours | Depends On |
|---|---------|-------|-------|------------|------------|
| 1 | TASK-HOOKS-001 | HookEventType Enum | foundation | 1.5 | - | DONE |
| 2 | TASK-HOOKS-002 | HookInput/HookOutput Types | foundation | 1.5 | 001 | DONE |
| 3 | TASK-HOOKS-003 | HookPayload Enum with Typed Variants | foundation | 1.0 | 002 | DONE |
| 4 | TASK-HOOKS-004 | HookError Enum | foundation | 1.0 | - |
| 5 | TASK-HOOKS-005 | HookConfig Settings Struct | foundation | 0.5 | 004 |
| 6 | TASK-HOOKS-006 | SessionStart Shell Executor | logic | 2.0 | 001-005 |
| 7 | TASK-HOOKS-007 | SessionEnd Shell Executor | logic | 1.5 | 001-005, 006 |
| 8 | TASK-HOOKS-008 | PreToolUse Shell Executor | logic | 2.0 | 001-005 |
| 9 | TASK-HOOKS-009 | UserPromptSubmit Handler | logic | 1.5 | 001-005 |
| 10 | TASK-HOOKS-010 | CLI consciousness brief Command | logic | 2.0 | 001-003 |
| 11 | TASK-HOOKS-011 | CLI consciousness inject Command | logic | 2.5 | 001-003, 010 |
| 12 | TASK-HOOKS-012 | Session Identity Snapshot Persistence | logic | 3.0 | 001-002, 006 |
| 13 | TASK-HOOKS-013 | Session Identity Snapshot Restoration | logic | 3.0 | 001, 006, 012 |
| 14 | TASK-HOOKS-014 | Shell Scripts for Claude Code Hooks | surface | 2.0 | 006-011 |
| 15 | TASK-HOOKS-015 | .claude/settings.json Hook Registrations | surface | 1.0 | 014 |
| 16 | TASK-HOOKS-016 | Integration Tests for Hook Lifecycle | surface | 3.5 | 006-013 |
| 17 | TASK-HOOKS-017 | E2E Tests with Real MCP Calls | surface | 4.0 | 014-016 |

---

## Dependency Graph (Hooks)

```
Layer 1: Foundation (Must Complete First)
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ TASK-HOOKS-001  │    │ TASK-HOOKS-004  │    │ TASK-HOOKS-005  │
│ Snapshot Struct │    │ HookError Enum  │    │ HookConfig      │
│ (1.5h)          │    │ (1.0h)          │    │ (0.5h)          │
└────────┬────────┘    └────────┬────────┘    └─────────────────┘
         │                      │
         ▼                      │
┌─────────────────┐             │
│ TASK-HOOKS-002  │             │
│ HookInput/Output│◄────────────┘
│ (1.5h)          │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ TASK-HOOKS-003  │
│ HookPayload     │
│ (1.0h) DONE     │
└────────┬────────┘
         │
Layer 2: Logic (Depends on Foundation)
         │
    ┌────┴────┬─────────────┬──────────────┐
    │         │             │              │
    ▼         ▼             ▼              ▼
┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────┐
│ 006     │ │ 008     │ │ 009     │ │ 010         │
│ Session │ │ PreTool │ │ Prompt  │ │ brief CLI   │
│ Start   │ │ Use     │ │ Submit  │ │ (2.0h)      │
│ (2.0h)  │ │ (2.0h)  │ │ (1.5h)  │ └──────┬──────┘
└────┬────┘ └─────────┘ └─────────┘        │
     │                                     ▼
     ▼                           ┌─────────────────┐
┌─────────────────┐              │ TASK-HOOKS-011  │
│ TASK-HOOKS-007  │              │ inject CLI      │
│ SessionEnd      │              │ (2.5h)          │
│ (1.5h)          │              └─────────────────┘
└────────┬────────┘
         │
         ▼
┌─────────────────┐         ┌─────────────────┐
│ TASK-HOOKS-012  │────────►│ TASK-HOOKS-013  │
│ Persistence     │         │ Restoration     │
│ (3.0h)          │         │ (3.0h)          │
└─────────────────┘         └─────────────────┘

Layer 3: Surface (Shell Scripts & Tests)
┌─────────────────┐
│ TASK-HOOKS-014  │
│ Shell Scripts   │
│ (2.0h)          │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ TASK-HOOKS-015  │
│ settings.json   │
│ (1.0h)          │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ TASK-HOOKS-016  │
│ Integration     │
│ Tests (3.5h)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ TASK-HOOKS-017  │
│ E2E Tests       │
│ (4.0h)          │
└─────────────────┘
```

---

## Layer Summary (Hooks)

### Foundation Layer (5 tasks, 5.5 hours)

| Task | Description |
|------|-------------|
| 001 | SessionIdentitySnapshot struct for hook data |
| 002 | HookInput/HookOutput types for shell communication |
| 003 | CLI argument types for hook commands |
| 004 | HookError enum with all error variants |
| 005 | HookConfig settings struct |

### Logic Layer (8 tasks, 17.5 hours)

| Task | Description |
|------|-------------|
| 006 | session_start shell executor with brief output |
| 007 | session_end shell executor with snapshot |
| 008 | pre_tool_use shell executor with injection |
| 009 | UserPromptSubmit handler with analysis |
| 010 | consciousness brief CLI command |
| 011 | consciousness inject CLI command |
| 012 | Session identity snapshot persistence |
| 013 | Session identity snapshot restoration |

### Surface Layer (4 tasks, 10.5 hours)

| Task | Description |
|------|-------------|
| 014 | Shell scripts in .claude/hooks/ |
| 015 | .claude/settings.json hook registrations |
| 016 | Integration tests for hook lifecycle |
| 017 | E2E tests with real MCP calls |

---

## Status Tracking (Hooks)

| Task | Status | Started | Completed | Verified |
|------|--------|---------|-----------|----------|
| TASK-HOOKS-001 | DONE | 2026-01-15 | 2026-01-15 | HookEventType enum at types.rs:46 |
| TASK-HOOKS-002 | DONE | 2026-01-15 | 2026-01-15 | HookInput/HookOutput at types.rs:948,1001 |
| TASK-HOOKS-003 | DONE | 2026-01-15 | 2026-01-15 | HookPayload enum at types.rs:863 |
| TASK-HOOKS-004 | Ready | - | - | - |
| TASK-HOOKS-005 | Ready | - | - | - |
| TASK-HOOKS-006 | Ready | - | - | - |
| TASK-HOOKS-007 | Ready | - | - | - |
| TASK-HOOKS-008 | Ready | - | - | - |
| TASK-HOOKS-009 | Ready | - | - | - |
| TASK-HOOKS-010 | Ready | - | - | - |
| TASK-HOOKS-011 | Ready | - | - | - |
| TASK-HOOKS-012 | Ready | - | - | - |
| TASK-HOOKS-013 | Ready | - | - | - |
| TASK-HOOKS-014 | Ready | - | - | - |
| TASK-HOOKS-015 | Ready | - | - | - |
| TASK-HOOKS-016 | Ready | - | - | - |
| TASK-HOOKS-017 | Ready | - | - | - |

**Progress: 0/17 tasks (0%)**

---

## Hook System Architecture

### 5 Claude Code Hooks (Native)

| Hook | Timeout | Purpose | Shell Script |
|------|---------|---------|--------------|
| SessionStart | 5000ms | Init session, output brief, restore identity | session_start.sh |
| SessionEnd | 5000ms | Persist identity snapshot | session_end.sh |
| PreToolUse | 2000ms | Inject relevant context | pre_tool_use.sh |
| PostToolUse | 2000ms | Record tool outcome | post_tool_use.sh |
| UserPromptSubmit | 2000ms | Analyze prompt context | user_prompt_submit.sh |

### Constitution Compliance

| Requirement | Task(s) | Implementation |
|-------------|---------|----------------|
| AP-50 | All | Native hooks only (no internal hooks) |
| AP-51 | 014 | Shell scripts call context-graph-cli |
| AP-52 | 015 | .claude/settings.json configuration |
| AP-53 | All | Direct CLI commands, no wrapper |
| ARCH-07 | 015 | Native Claude Code hooks via settings |

---

## Quick Start (Hooks)

```bash
# Execute tasks in order
# Each task file contains detailed implementation steps

# Foundation
cat docs/specs/tasks/TASK-HOOKS-001.md
cat docs/specs/tasks/TASK-HOOKS-002.md
# ... etc

# After each task, verify:
cargo build --package context-graph-cli
cargo test --package context-graph-cli
```

---

# Combined Summary

## All Tasks by Phase

| Phase | Task Prefix | Count | Est. Hours | Status |
|-------|-------------|-------|------------|--------|
| Session Identity | TASK-SESSION | 17 | 21.0 | ✅ Complete |
| Native Hooks | TASK-HOOKS | 17 | 25.0 | Ready |
| Skills & Subagents | TASK-SKILLS | 16 | 23.5 | Ready |
| **Total** | | **50** | **69.5** | |

## Execution Order (All Phases)

1. **Session Identity** (17 tasks) - Foundation for identity persistence
2. **Native Hooks** (17 tasks) - Depends on Session Identity
3. **Skills & Subagents** (16 tasks) - Depends on Hooks

## Cross-Phase Dependencies

```
Session Identity ────► Native Hooks ────► Skills & Subagents
(TASK-SESSION-*)       (TASK-HOOKS-*)     (TASK-SKILLS-*)
     │                      │                   │
     └──────────────────────┴───────────────────┘
                   ▼
         Context Graph v5 Complete
```
