# Task Traceability Matrix

This document provides full traceability from functional requirements to task implementations for Phase 3 (Native Hooks) and Phase 4 (Skills & Subagents).

---

## Coverage Summary

| Phase | Spec | Requirements | Tasks | Coverage |
|-------|------|--------------|-------|----------|
| Phase 3 | SPEC-HOOKS | 47 | 17 | 100% |
| Phase 4 | SPEC-SKILLS | 45 | 16 | 100% |
| **Total** | | **92** | **33** | **100%** |

---

# Phase 3: Native Hooks Traceability

## Coverage: SPEC-HOOKS → TASK-HOOKS-*

### Foundation Requirements (REQ-HOOKS-01 to REQ-HOOKS-10)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-HOOKS-01 | SessionStart hook fires at session init | TASK-HOOKS-014 | ✓ |
| REQ-HOOKS-02 | SessionEnd hook fires at session termination | TASK-HOOKS-014 | ✓ |
| REQ-HOOKS-03 | PreToolUse hook fires before tool execution | TASK-HOOKS-014 | ✓ |
| REQ-HOOKS-04 | PostToolUse hook fires after tool execution | TASK-HOOKS-014 | ✓ |
| REQ-HOOKS-05 | Hooks configured via .claude/settings.json | TASK-HOOKS-015 | ✓ |
| REQ-HOOKS-06 | Hook timeout enforcement per spec | TASK-HOOKS-015 | ✓ |
| REQ-HOOKS-07 | Hook input received via stdin JSON | TASK-HOOKS-002 | ✓ |
| REQ-HOOKS-08 | Hook output returned via stdout | TASK-HOOKS-002 | ✓ |
| REQ-HOOKS-09 | Session identity persisted on SessionEnd | TASK-HOOKS-012 | ✓ |
| REQ-HOOKS-10 | Session identity restored on SessionStart | TASK-HOOKS-013 | ✓ |

### Data Model Requirements (REQ-HOOKS-11 to REQ-HOOKS-20)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-HOOKS-11 | SessionIdentitySnapshot struct defined | TASK-HOOKS-001 | ✓ |
| REQ-HOOKS-12 | Snapshot includes IC, Kuramoto phases | TASK-HOOKS-001 | ✓ |
| REQ-HOOKS-13 | Snapshot includes purpose vector | TASK-HOOKS-001 | ✓ |
| REQ-HOOKS-14 | Snapshot timestamp and session_id | TASK-HOOKS-001 | ✓ |
| REQ-HOOKS-15 | HookInput type with tool_name, tool_input | TASK-HOOKS-002 | ✓ |
| REQ-HOOKS-16 | HookOutput type with content, exit_code | TASK-HOOKS-002 | ✓ |
| REQ-HOOKS-17 | HookConfig struct with timeouts | TASK-HOOKS-005 | ✓ |
| REQ-HOOKS-18 | HookError enum variants | TASK-HOOKS-004 | ✓ |
| REQ-HOOKS-19 | CLI argument types | TASK-HOOKS-004 | ✓ |
| REQ-HOOKS-20 | Exit code mapping (AP-26 compliant) | TASK-HOOKS-004 | ✓ |

### CLI Command Requirements (REQ-HOOKS-21 to REQ-HOOKS-35)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-HOOKS-21 | session_start outputs brief | TASK-HOOKS-006 | ✓ |
| REQ-HOOKS-22 | session_end captures snapshot | TASK-HOOKS-007 | ✓ |
| REQ-HOOKS-23 | pre_tool_use injects context | TASK-HOOKS-008 | ✓ |
| REQ-HOOKS-24 | post_tool_use records outcome | TASK-HOOKS-008 | ✓ |
| REQ-HOOKS-25 | UserPromptSubmit analyzes context | TASK-HOOKS-009 | ✓ |
| REQ-HOOKS-26 | Identity markers detected in prompt | TASK-HOOKS-009 | ✓ |
| REQ-HOOKS-27 | consciousness brief CLI command | TASK-HOOKS-010 | ✓ |
| REQ-HOOKS-28 | Brief output under 200 tokens | TASK-HOOKS-010 | ✓ |
| REQ-HOOKS-29 | Brief includes r, IC, level | TASK-HOOKS-010 | ✓ |
| REQ-HOOKS-30 | Brief --format flag (json/text) | TASK-HOOKS-010 | ✓ |
| REQ-HOOKS-31 | consciousness inject CLI command | TASK-HOOKS-011 | ✓ |
| REQ-HOOKS-32 | Inject supports --query flag | TASK-HOOKS-011 | ✓ |
| REQ-HOOKS-33 | Inject supports --node-ids flag | TASK-HOOKS-011 | ✓ |
| REQ-HOOKS-34 | Inject respects --max-tokens | TASK-HOOKS-011 | ✓ |
| REQ-HOOKS-35 | Inject returns structured result | TASK-HOOKS-011 | ✓ |

### Persistence Requirements (REQ-HOOKS-36 to REQ-HOOKS-42)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-HOOKS-36 | Snapshot stored in .claude/identity/ | TASK-HOOKS-012 | ✓ |
| REQ-HOOKS-37 | Atomic write (temp + rename) | TASK-HOOKS-012 | ✓ |
| REQ-HOOKS-38 | Index file tracks latest | TASK-HOOKS-012 | ✓ |
| REQ-HOOKS-39 | Snapshot rotation (max 50) | TASK-HOOKS-012 | ✓ |
| REQ-HOOKS-40 | Restore validates freshness | TASK-HOOKS-013 | ✓ |
| REQ-HOOKS-41 | Restore computes drift metrics | TASK-HOOKS-013 | ✓ |
| REQ-HOOKS-42 | Restore fails gracefully if stale | TASK-HOOKS-013 | ✓ |

### Test Requirements (REQ-HOOKS-43 to REQ-HOOKS-47)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-HOOKS-43 | Integration tests for lifecycle | TASK-HOOKS-016 | ✓ |
| REQ-HOOKS-44 | Integration tests for persistence | TASK-HOOKS-016 | ✓ |
| REQ-HOOKS-45 | E2E tests with real MCP | TASK-HOOKS-017 | ✓ |
| REQ-HOOKS-46 | E2E tests simulate Claude Code | TASK-HOOKS-017 | ✓ |
| REQ-HOOKS-47 | No mock data in any tests | TASK-HOOKS-016, 017 | ✓ |

---

# Phase 4: Skills & Subagents Traceability

## Coverage: SPEC-SKILLS → TASK-SKILLS-*

### Type Definition Requirements (REQ-SKILLS-01 to REQ-SKILLS-10)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-SKILLS-01 | SkillDefinition struct | TASK-SKILLS-001 | ✓ |
| REQ-SKILLS-02 | SkillFrontmatter YAML parsing | TASK-SKILLS-001 | ✓ |
| REQ-SKILLS-03 | SkillModel enum (haiku/sonnet/opus) | TASK-SKILLS-001 | ✓ |
| REQ-SKILLS-04 | SubagentDefinition struct | TASK-SKILLS-002 | ✓ |
| REQ-SKILLS-05 | TaskToolParams for spawning | TASK-SKILLS-002 | ✓ |
| REQ-SKILLS-06 | SkillLoadResult type | TASK-SKILLS-003 | ✓ |
| REQ-SKILLS-07 | SkillTrigger type | TASK-SKILLS-003 | ✓ |
| REQ-SKILLS-08 | SkillError enum variants | TASK-SKILLS-004 | ✓ |
| REQ-SKILLS-09 | SubagentError enum variants | TASK-SKILLS-004 | ✓ |
| REQ-SKILLS-10 | ProgressiveDisclosureLevel enum | TASK-SKILLS-005 | ✓ |

### Component Requirements (REQ-SKILLS-11 to REQ-SKILLS-20)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-SKILLS-11 | SkillLoader implementation | TASK-SKILLS-006 | ✓ |
| REQ-SKILLS-12 | Progressive disclosure (3 levels) | TASK-SKILLS-006 | ✓ |
| REQ-SKILLS-13 | YAML frontmatter parsing | TASK-SKILLS-006 | ✓ |
| REQ-SKILLS-14 | SkillRegistry implementation | TASK-SKILLS-007 | ✓ |
| REQ-SKILLS-15 | Discovery precedence (project>personal>plugin) | TASK-SKILLS-007 | ✓ |
| REQ-SKILLS-16 | TriggerMatcher implementation | TASK-SKILLS-008 | ✓ |
| REQ-SKILLS-17 | Keyword matching for auto-trigger | TASK-SKILLS-008 | ✓ |
| REQ-SKILLS-18 | ToolRestrictor implementation | TASK-SKILLS-009 | ✓ |
| REQ-SKILLS-19 | allowed-tools enforcement | TASK-SKILLS-009 | ✓ |
| REQ-SKILLS-20 | Background mode restrictions | TASK-SKILLS-009 | ✓ |

### Subagent Requirements (REQ-SKILLS-21 to REQ-SKILLS-28)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-SKILLS-21 | SubagentSpawner implementation | TASK-SKILLS-010 | ✓ |
| REQ-SKILLS-22 | Task tool invocation | TASK-SKILLS-010 | ✓ |
| REQ-SKILLS-23 | Subagent cannot spawn subagent | TASK-SKILLS-010 | ✓ |
| REQ-SKILLS-24 | Background MCP restriction | TASK-SKILLS-010 | ✓ |
| REQ-SKILLS-25 | Read/Grep/Glob always allowed | TASK-SKILLS-010 | ✓ |
| REQ-SKILLS-26 | identity-guardian subagent | TASK-SKILLS-014 | ✓ |
| REQ-SKILLS-27 | memory-specialist subagent | TASK-SKILLS-014 | ✓ |
| REQ-SKILLS-28 | consciousness-explorer subagent | TASK-SKILLS-014 | ✓ |

### Skill File Requirements (REQ-SKILLS-29 to REQ-SKILLS-38)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-SKILLS-29 | consciousness SKILL.md | TASK-SKILLS-011 | ✓ |
| REQ-SKILLS-30 | GWT tool mappings | TASK-SKILLS-011 | ✓ |
| REQ-SKILLS-31 | memory-inject SKILL.md | TASK-SKILLS-012 | ✓ |
| REQ-SKILLS-32 | haiku model for memory ops | TASK-SKILLS-012 | ✓ |
| REQ-SKILLS-33 | semantic-search SKILL.md | TASK-SKILLS-012 | ✓ |
| REQ-SKILLS-34 | Search tool mappings | TASK-SKILLS-012 | ✓ |
| REQ-SKILLS-35 | dream-consolidation SKILL.md | TASK-SKILLS-013 | ✓ |
| REQ-SKILLS-36 | Dream tool mappings | TASK-SKILLS-013 | ✓ |
| REQ-SKILLS-37 | curation SKILL.md | TASK-SKILLS-013 | ✓ |
| REQ-SKILLS-38 | Curation tool mappings | TASK-SKILLS-013 | ✓ |

### Test Requirements (REQ-SKILLS-39 to REQ-SKILLS-45)

| Requirement ID | Description | Task ID | Status |
|----------------|-------------|---------|--------|
| REQ-SKILLS-39 | SkillLoader unit tests | TASK-SKILLS-015 | ✓ |
| REQ-SKILLS-40 | SkillRegistry integration tests | TASK-SKILLS-015 | ✓ |
| REQ-SKILLS-41 | TriggerMatcher tests | TASK-SKILLS-015 | ✓ |
| REQ-SKILLS-42 | ToolRestrictor tests | TASK-SKILLS-015 | ✓ |
| REQ-SKILLS-43 | E2E skill invocation tests | TASK-SKILLS-016 | ✓ |
| REQ-SKILLS-44 | E2E subagent spawn tests | TASK-SKILLS-016 | ✓ |
| REQ-SKILLS-45 | No mock data in any tests | TASK-SKILLS-015, 016 | ✓ |

---

## Uncovered Items

### Phase 3 (Hooks)
**(none)** - All 47 requirements have task coverage.

### Phase 4 (Skills)
**(none)** - All 45 requirements have task coverage.

---

## Validation Checklist

### Phase 3: Native Hooks
- [x] All data models have tasks (REQ-HOOKS-11 to 20)
- [x] All CLI commands have tasks (REQ-HOOKS-21 to 35)
- [x] All persistence requirements have tasks (REQ-HOOKS-36 to 42)
- [x] All test requirements have tasks (REQ-HOOKS-43 to 47)
- [x] Task dependencies form valid DAG (no cycles)
- [x] Layer ordering correct (foundation → logic → surface)

### Phase 4: Skills & Subagents
- [x] All type definitions have tasks (REQ-SKILLS-01 to 10)
- [x] All components have tasks (REQ-SKILLS-11 to 20)
- [x] All subagent requirements have tasks (REQ-SKILLS-21 to 28)
- [x] All skill files have tasks (REQ-SKILLS-29 to 38)
- [x] All test requirements have tasks (REQ-SKILLS-39 to 45)
- [x] Task dependencies form valid DAG (no cycles)
- [x] Layer ordering correct (foundation → logic → surface)

---

## Constitution Compliance Matrix

| Anti-Pattern | Description | Compliance |
|--------------|-------------|------------|
| AP-50 | No internal hooks infrastructure | ✓ Native hooks only |
| AP-51 | Shell scripts call context-graph-cli | ✓ TASK-HOOKS-014 |
| AP-52 | Hooks via .claude/settings.json | ✓ TASK-HOOKS-015 |
| AP-53 | Direct CLI commands, no wrapper | ✓ All CLI tasks |

| Architecture Rule | Description | Compliance |
|-------------------|-------------|------------|
| ARCH-07 | Claude Code hooks via settings.json | ✓ TASK-HOOKS-015 |
| ARCH-08 | Skills in .claude/skills/ | ✓ TASK-SKILLS-011-013 |
| ARCH-09 | Subagents in .claude/agents/ | ✓ TASK-SKILLS-014 |

---

## Cross-Reference Index

### By Task ID

| Task ID | Requirements Covered |
|---------|---------------------|
| TASK-HOOKS-001 | REQ-HOOKS-11, 12, 13, 14 |
| TASK-HOOKS-002 | REQ-HOOKS-07, 08, 15, 16 |
| TASK-HOOKS-003 | TECH-HOOKS-2.2 (HookPayload types) |
| TASK-HOOKS-004 | REQ-HOOKS-18, 19, 20 |
| TASK-HOOKS-005 | REQ-HOOKS-17 |
| TASK-HOOKS-006 | REQ-HOOKS-21 |
| TASK-HOOKS-007 | REQ-HOOKS-22 |
| TASK-HOOKS-008 | REQ-HOOKS-23, 24 |
| TASK-HOOKS-009 | REQ-HOOKS-25, 26 |
| TASK-HOOKS-010 | REQ-HOOKS-27, 28, 29, 30 |
| TASK-HOOKS-011 | REQ-HOOKS-31, 32, 33, 34, 35 |
| TASK-HOOKS-012 | REQ-HOOKS-09, 36, 37, 38, 39 |
| TASK-HOOKS-013 | REQ-HOOKS-10, 40, 41, 42 |
| TASK-HOOKS-014 | REQ-HOOKS-01, 02, 03, 04 |
| TASK-HOOKS-015 | REQ-HOOKS-05, 06 |
| TASK-HOOKS-016 | REQ-HOOKS-43, 44, 47 |
| TASK-HOOKS-017 | REQ-HOOKS-45, 46, 47 |

| Task ID | Requirements Covered |
|---------|---------------------|
| TASK-SKILLS-001 | REQ-SKILLS-01, 02, 03 |
| TASK-SKILLS-002 | REQ-SKILLS-04, 05 |
| TASK-SKILLS-003 | REQ-SKILLS-06, 07 |
| TASK-SKILLS-004 | REQ-SKILLS-08, 09 |
| TASK-SKILLS-005 | REQ-SKILLS-10 |
| TASK-SKILLS-006 | REQ-SKILLS-11, 12, 13 |
| TASK-SKILLS-007 | REQ-SKILLS-14, 15 |
| TASK-SKILLS-008 | REQ-SKILLS-16, 17 |
| TASK-SKILLS-009 | REQ-SKILLS-18, 19, 20 |
| TASK-SKILLS-010 | REQ-SKILLS-21, 22, 23, 24, 25 |
| TASK-SKILLS-011 | REQ-SKILLS-29, 30 |
| TASK-SKILLS-012 | REQ-SKILLS-31, 32, 33, 34 |
| TASK-SKILLS-013 | REQ-SKILLS-35, 36, 37, 38 |
| TASK-SKILLS-014 | REQ-SKILLS-26, 27, 28 |
| TASK-SKILLS-015 | REQ-SKILLS-39, 40, 41, 42, 45 |
| TASK-SKILLS-016 | REQ-SKILLS-43, 44, 45 |

---

*Generated: 2026-01-15*
*Version: 1.0*
