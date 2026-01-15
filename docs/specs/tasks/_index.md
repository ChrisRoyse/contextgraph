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
| 015-TASK-SESSION-15 | ⏳ Pending | - | - | - |
| 016-TASK-SESSION-16 | ⏳ Pending | - | - | - |
| 017-TASK-SESSION-17 | ⏳ Pending | - | - | - |

**Progress: 14/17 tasks (82.4%)**

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
