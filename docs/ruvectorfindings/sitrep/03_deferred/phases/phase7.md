# Doc 09 Phase 7 — Reality-Loop Integration + MCP Tools

> **Estimated effort (per doc 09 §7):** 4-6 days.
> **Status (2026-05-09):** DEFERRED — depends on Phase 4 verify() API.

## Goal

Phase 7 exposes the trained ME-JEPA verifier to the existing **ccreality
outer reality-loop optimizer** (CLAUDE.md §11) via the MCP tool
interface. After Phase 7 ships:

- The reality loop can call `mcp__cgreality__mejepa_verify` on any
  candidate patch and get a calibrated verdict in <100 ms (vs. the
  Docker oracle's multi-minute runtime).
- Witness-chain entries for each `verify()` call land alongside the
  existing `attempt-summary.json` artifacts.
- Trigger decisions (`mcp__cgreality__reality_trigger_decision`)
  consult ME-JEPA's verdict to decide whether to run the expensive
  Docker oracle or short-circuit on a high-confidence "Pass" /
  "OutOfDistribution" verdict.

Per doc 09 §1.1's compiler analogy, Phase 7 is the **runtime linker**:
it makes the trained verifier callable from the surrounding system at
production latency.

## Why this phase matters

The end goal in doc 09 §1 is to **replace human code review with reality
verification**. ME-JEPA must be reachable from the exact tools the outer
optimizer already uses; otherwise it's a research curiosity, not a
production verifier.

The latency gap is the killer:
- Docker oracle: ~2-5 minutes per attempt × 300 tasks × ≥10 attempts
  each = many days for one full sweep.
- ME-JEPA verify(): <100ms per attempt → full 300-task sweep in <30
  seconds.

ME-JEPA replaces 99%+ of Docker calls with sub-second predictions.
Docker is invoked only when ME-JEPA returns `OutOfDistribution`,
`Abstain`, or low-confidence Pass/Fail (interval width >0.30).

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mcp/handlers/tools/mejepa.rs`** — new MCP
   tool family:
   - `mcp__cgreality__mejepa_verify { task_id, patch_diff } -> Verdict`
   - `mcp__cgreality__mejepa_status -> { last_finetune_step,
     calibration_coverage, ood_threshold }`
   - `mcp__cgreality__mejepa_replay_witness { from_step, to_step }
     -> ReplayProof`
2. **Trigger-decision integration**: `reality_trigger_decision` consults
   ME-JEPA before scheduling Docker. The schema gains
   `mejepa_consultation: { verdict, interval_width, ood_score, decided_to_skip_docker }`.
3. **Per-attempt witness extension**: `attempt-summary.json` gains a
   `mejepa_verdict` field with the cert hash.
4. **Doc 09-compliant fail-closed**: if the ME-JEPA checkpoint is
   missing or stale, `mejepa_verify` returns `Verdict::Abstain { reason:
   "checkpoint_unavailable" }`, NOT a default Pass/Fail.

## Definition of Done (per doc 09 §7)

- All three MCP tools callable from Claude Code via the standard
  `mcp__cgreality__*` namespace.
- Trigger decision tested: synthetic high-confidence Pass verdict
  short-circuits Docker (`should_run_docker: false`); low-confidence or
  OOD verdict still runs Docker.
- Witness chain replay test: pick 100 attempts at random, replay from
  ME-JEPA cert chain, recover identical verdicts.
- End-to-end latency: `mejepa_verify` <100ms p95 on RTX 5090 cold-cache;
  <20ms warm-cache.

## Verification protocol (per doc 09 §7)

```bash
# Smoke test — assumes Phase 0..6 shipped
context-graph mcp-test --tools mejepa_verify,mejepa_status,mejepa_replay_witness
# expected:
#   mejepa_verify: 100/100 returned in <100ms p95
#   mejepa_status: { calibration_coverage: ~0.90, ood_threshold: <num> }
#   mejepa_replay_witness: chain integrity verified for 100 attempts
#   docker_short_circuit_rate: ~92% on a 1k-attempt sample
```

## Component breakdown

### A. MCP tool: mejepa_verify

```rust
async fn handle_mejepa_verify(
    server: &McpServer,
    params: Value,
) -> ToolResponse {
    let task_id: String = required_field(&params, "task_id")?;
    let patch_diff: String = required_field(&params, "patch_diff")?;
    let compiler = server.mejepa.read().await;
    let Some(c) = compiler.as_ref() else {
        return ToolResponse::abstain("checkpoint_unavailable");
    };
    let patch = parse_diff(&patch_diff)?;
    let verdict = c.verify(&patch);
    let cert = c.emit_witness_cert(&task_id, &patch_diff, &verdict)?;
    ToolResponse::ok(json!({
        "verdict": verdict,
        "cert_hash": hex::encode(cert.self_hash),
        "interval_width": verdict.interval_width(),
        "ood_score": verdict.ood_score(),
    }))
}
```

### B. Trigger-decision consultation

```rust
fn decide_trigger(
    state: &TriggerState,
    mejepa: Option<&Verdict>,
) -> TriggerDecision {
    // Existing trigger logic (cooldown, harness-changed-since-last, etc.)
    let base = state.base_decision();
    if !base.should_run {
        return base;  // base says no, ME-JEPA can't override "no"
    }
    let Some(verdict) = mejepa else {
        return base;  // ME-JEPA absent — fall through to Docker
    };
    match verdict {
        Verdict::Pass { confidence_interval, .. }
            if confidence_interval.1 - confidence_interval.0 < 0.30 =>
        {
            // High-confidence Pass — skip Docker
            TriggerDecision {
                should_run: false,
                policy: "mejepa_high_confidence_pass".into(),
                ..base
            }
        }
        Verdict::Fail { confidence_interval, .. }
            if confidence_interval.1 - confidence_interval.0 < 0.30 =>
        {
            // High-confidence Fail — also skip Docker (we trust the verdict)
            TriggerDecision {
                should_run: false,
                policy: "mejepa_high_confidence_fail".into(),
                ..base
            }
        }
        Verdict::OutOfDistribution { .. } => base,  // still run Docker
        Verdict::Abstain { .. } => base,             // still run Docker
        _ => base,                                    // low-confidence: run Docker
    }
}
```

### C. Witness extension

```rust
struct AttemptSummary {
    // ... existing fields ...
    mejepa_verdict: Option<MeJepaVerdictCert>,
}

struct MeJepaVerdictCert {
    cert_hash: [u8; 32],
    verdict_kind: VerdictKind,
    interval_lo: f32,
    interval_hi: f32,
    ood_score: f32,
    chain_parent: [u8; 32],
}
```

### D. Replay tool

`mcp__cgreality__mejepa_replay_witness { from_step, to_step }` walks the
chain, re-executes each `verify()` call against the persisted `(panel_t01,
panel_t2)` and confirms each cert's `self_hash` matches.

## Critical path within Phase 7

1. Phase 4 `MeJepaCompiler::verify(patch)` → callable as a library.
2. MCP tool registration + JSON-RPC schema.
3. Trigger-decision plumbing (consult ME-JEPA → emit decision).
4. Witness extension to `attempt-summary.json`.
5. Replay verifier.

## Dependencies

- ⏳ Phase 4 verify() API.
- ⏳ Phase 6 storage (so cert chain persists durably).
- ⏳ Phase 0 + Phase 2 (so panels exist for verify() inputs).
- ✅ Existing `mcp__cgreality__*` MCP server framework
  (`crates/context-graph-mcp`).
- ✅ Existing trigger-decision framework
  (`mcp__cgreality__reality_trigger_decision`).

## Implementation order (recommended)

**Sub-phase 7.A**: Wire `MeJepaCompiler` into `McpServer` state. ~0.5 day.

**Sub-phase 7.B**: Implement `mejepa_verify` tool with full schema. ~1 day.

**Sub-phase 7.C**: `mejepa_status` + `mejepa_replay_witness`. ~1 day.

**Sub-phase 7.D**: Trigger-decision consultation. ~1 day.

**Sub-phase 7.E**: Witness extension to attempt-summary.json. ~1 day.

**Sub-phase 7.F**: End-to-end latency + replay test. ~1 day.

## Risks (per doc 09 §8)

1. **MCP latency overhead** (JSON-RPC marshaling) eats the <100ms
   budget. Mitigation: in-process call path when verify() is invoked
   from same-process trigger-decision; only pay JSON-RPC cost for
   outer-optimizer-initiated calls.
2. **Trigger-decision integration introduces feedback loop**: if
   ME-JEPA says "skip Docker" and is wrong, we don't learn. Mitigation:
   periodically (5% of attempts) force Docker even when ME-JEPA
   high-confidence-passes, to keep the drift detector (Phase 5) calibrated.
3. **Schema drift**: doc 09 specifies `Verdict` shape; the MCP tool
   schema must match exactly. Pin schema version in MCP tool description.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.9,
  §7`
- Phase 4 dependency: `phase4.md`
- Phase 5 dependency: `phase5.md` (drift detector consumes verify() calls)
- Existing MCP framework: `crates/context-graph-mcp/src/handlers/tools/`
- Existing trigger framework: `crates/context-graph-mcp/src/handlers/tools/reality_loop/`
