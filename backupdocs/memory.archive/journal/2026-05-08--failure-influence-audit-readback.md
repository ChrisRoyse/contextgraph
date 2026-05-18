---
namespace: journal
created: 2026-05-08T21:14:07Z
updated: 2026-05-08T21:20:03Z
status: active
tags: ccreality,influence,ppr,fsv,readback,test-failure
---

# Failure: Influence Audit Readback Mismatch

## What Happened

While adding `optimizer_compute_influence`, the happy-path FSV unit test failed after writing the influence audit file. The physical file was created, but the code compared the in-memory `serde_json::Value` to the parsed readback value and returned `CCREALITY_INFLUENCE_AUDIT_READBACK_MISMATCH`.

## Why It Matters

The source-of-truth discipline is correct, but float-bearing JSON should be canonicalized through serde before doing structural readback equality. Otherwise a valid durable write can be reported as a filesystem mismatch even when the text readback matched.

## Related

- Code path: `crates/context-graph-mcp/src/handlers/tools/reality_loop/influence.rs`
- Test: `influence_happy_path_writes_audit_and_witness_from_physical_state`

## Follow-Up Failure

After canonicalization, the same happy-path test reached physical audit and witness verification, then failed because it asserted `task-a` must outrank `task-c`. Both attempts were directly connected to the same `import-error` seed tag, so that ordering was not a valid invariant. The test now asserts the two expected import-error attempts are both present and both carry the seed tag.

## Registry Count Failure

The MCP protocol tools-list E2E test failed after the influence tests passed because its hard-coded default registry count still expected 146 tools, while the source-of-truth registry returned 149. The printed list showed this was the default registry count, not the reality-loop-only influence tool. The registry comments and count assertions were updated to 149 with LLM / 145 without LLM, and the DynamicJEPA submodule count was updated from 35 to 39.
