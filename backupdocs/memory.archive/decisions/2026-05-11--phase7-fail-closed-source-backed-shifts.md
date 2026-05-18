---
namespace: decisions
created: 2026-05-11T02:09:55Z
updated: 2026-05-11T02:09:55Z
status: active
tags: phase7, shift-subscriber, mcp, fail-closed, source-of-truth
---

# Phase 7 Uses Fail-Closed Source-Backed Shifts

## Decision

Phase 7 code-edit inference must be built from real source-of-truth artifacts, not synthesized fallback data. Missing before text, missing after source, missing declared SHA, unreadable source files, or missing witness-chain evidence must produce a structured error. Non-code shifts are not coerced into fake code patches; they advance the durable watermark with `SkipReason::NoOracleSignal`.

## Rationale

The subscriber exists to convert durable reality shifts into ME-JEPA prediction/oracle/panel state. If it fabricates missing patch text or witness data, the database can contain predictions that cannot be tied back to the actual edit. Fail-closed behavior makes contract drift visible and gives operators an actionable error path.

## Related

- See also: ./memory/discoveries/2026-05-11--phase7-real-producer-shift-contract.md
- See also: ./memory/reports/session-2026-05-11--phase7-producer-source-backed-hardening.md
