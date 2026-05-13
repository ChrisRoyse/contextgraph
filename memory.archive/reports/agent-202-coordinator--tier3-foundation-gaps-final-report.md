---
namespace: reports
agent_id: agent-202-coordinator
timestamp: 2026-05-09T02:30:00Z
status: active
tags: tier3, ruvector-findings, causal-masking, root-cause-fix, final-report
---

# Tier-3 Foundation Gaps — Final Report (2026-05-09)

## Summary

Tier-3 partial: shipped the in-scope subset (root-cause fix for the recurring
`CCREALITY_WITNESS_OP_TYPE_UNKNOWN` hook errors + §3.2 Causal masking with
retrocausal-safety guards). Five Tier-3 items deferred per scope split in
`memory/context/tier3-foundation-gaps-initiative.md`.

| Deliverable | Status | Tests | FSV |
|-|-|-|-|
| Root-cause fix: stale debug MCP binary | ✓ shipped | 4 unit tests added | inline (binary timestamps verified) |
| §3.2 Causal masking + retrocausal safety | ✓ shipped | 13 new (58 solver total) | `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json` |
| `cert.rs` + `granger.rs` path-attr build fix | ✓ shipped | regression-pass on 30 + 28 tests | tier1 + granger + cert FSVs re-run green |
| §3.1 Cross-pack skill transfer | DEFERRED | — | — |
| §3.3 Coherence-driven SONA tuning | DEFERRED | — | — |
| §3.4 FastGRNN intent router | DEFERRED | — | — |
| §3.5 Prime-radiant sheaf-Laplacian | DEFERRED | — | — |
| §3.6 Manifold geometry | DEFERRED | — | — |

## What changed

### 1. Root cause: stale-debug-MCP — `crates/context-graph-cli/src/commands/ccreality/mod.rs:563-610`

The recurring hook error `CCREALITY_WITNESS_OP_TYPE_UNKNOWN` at offset 137
op_type=8 was caused by `mcp_binary()` defaulting to
`target/debug/context-graph-mcp` regardless of staleness. The May 8 16:15
debug binary did not know about `WitnessOperation::WitnessRepair = 8`,
which the May 8 21:03 release binary writes. Fix: replace the debug-default
resolver with `env override > .mcp.json command > hard error`. There is
intentionally no automatic profile fallback — the canonical source of
truth becomes `.mcp.json` (a checked-in file Claude Code itself spawns
from), guaranteeing the engine fork/exec uses the SAME binary.

The code is more conservative than my handoff brief described (the brief
said "env > release > debug" with auto-fallback; the actual code is
"env > .mcp.json > hard error"). The subagent caught the inconsistency
during review — see `memory/discoveries/agent-203-code-simplifier--mcp-resolver-handoff-mismatch.md`.

### 2. §3.2 Causal masking + retrocausal safety — clean-room

- **New module** `crates/context-graph-solver/src/causal_mask.rs` (~310 lines after R1+R2 doc refinements).
- **New tests** `crates/context-graph-solver/src/causal_mask_tests.rs` (13 tests, all pass).
- **New FSV** `crates/context-graph-solver/examples/causal_mask_fsv.rs` (writes `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json`).
- **Re-exports** `apply_mask_to_scores`, `build_causal_mask`,
  `retrocausal_safety_check`, `softmax_row`, `MaskStrategy` at the crate root.

Per `docs/ruvectorfindings/04 §8a, §8d`. Inspired by ruvnet/RuVector
`crates/ruvector-graph-transformer/src/temporal.rs` HEAD ef5274c2.
Clean-room, no code copied, no upstream tracking — see
`memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

`MaskStrategy` enum:
- `Strict`: t' < t (no self-attention; classic autoregressive)
- `TimeWindow { window_size }`: t' in [t - window, t] (sliding window over panel sequences; matches DynamicJEPA 4-step trajectory)
- `CausalWithSelf`: t' <= t (self-inclusive; default for Stage F read-out heads)

`apply_mask_to_scores` writes `f32::NEG_INFINITY` to forbidden positions
(softmax-of-NEG_INFINITY == 0.0 by definition; no numerical leakage).
`retrocausal_safety_check` is the per-cell fail-closed audit; bulk
callers should use `apply_mask_to_scores` (the doc steers them now).

Fail-closed everywhere:
- `CGSOLVER_INVALID_INPUT` on empty timestamps, length mismatch,
  index-out-of-bounds, retrocausal violations.
- `CGSOLVER_NUMERICAL_INVARIANT` on NaN, ±INFINITY in input scores,
  or fully-masked softmax row.
- Two-pass apply (validate → mutate) so scores are NOT mutated when
  validation fails (FSV asserts this).

### 3. Build fix: path-attribute on test/submodule decls

`cert.rs` and `granger.rs` had `mod cert_merkle;` and `mod granger_tests;`
declarations that rustc looked for under `cert/cert_merkle.rs` etc. The
files live at the same level as the parent (`cert_merkle.rs`,
`granger_tests.rs`). Fixed with `#[path = "cert_merkle.rs"]` /
`#[path = "granger_tests.rs"]` attributes. Caught the moment a test build
was attempted; release builds happened to succeed because of cached
artifacts.

## Verification (Full State Verification)

### Source of truth — physical disk evidence

```bash
$ ls -la /home/cabdru/contextgraph/target/release/context-graph-mcp /home/cabdru/contextgraph/target/release/reality-loop
-rwxr-xr-x ... May  8 21:03 /home/cabdru/contextgraph/target/release/context-graph-mcp
-rwxr-xr-x ... May  8 21:05 /home/cabdru/contextgraph/target/release/reality-loop

$ jq '.mcpServers.cgreality.command' /home/cabdru/contextgraph/.mcp.json
"/home/cabdru/contextgraph/target/release/context-graph-mcp"

$ ls -la /tmp/contextgraph-tier2-fsv/causal-mask-evidence.json
-rw-r--r-- ... May  8 21:12 /tmp/contextgraph-tier2-fsv/causal-mask-evidence.json
```

### Test results

- `cargo build --release -p context-graph-cli -p context-graph-mcp -p context-graph-witness -p context-graph-solver` → clean (3m 43s).
- `cargo test --release -p context-graph-solver` → 58 passed, 0 failed (was 45 pre-§3.2; +13 new).
- `cargo test --release -p context-graph-witness` → 30 passed.
- `cargo test --release -p context-graph-mincut` → all green.
- Tier-1 FSV (tier1_fsv): re-run, passes.
- Granger FSV: re-run, passes (F(2,30)@3.32=0.0498, F(1,100)@3.94 within tolerance).
- Mincut FSV (Stoer-Wagner Fig 1): re-run, deterministic across runs.
- Cert FSV (Stage 0/F + Merkle): re-run, witness chain verifies.

### Causal mask FSV evidence (boundary cases on disk)

```json
{
  "empty":      "CGSOLVER_INVALID_INPUT",
  "shape":      "CGSOLVER_INVALID_INPUT",
  "nonfinite":  "CGSOLVER_NUMERICAL_INVARIANT",
  "retro":      "CGSOLVER_INVALID_INPUT",
  "mask_passes": true,
  "scores_preserved_on_inf_rejection": ["0.1", "+inf", "0.3", "0.4"]
}
```

Forbidden-position softmax weight is exactly 0.0 (verified for both row 0
and row 2 of the masked 4x4 matrix). Time-window mask matches
hand-derived expected mask bit-for-bit for irregular timestamps.

## Code-simplifier review

Subagent agent-203 (synchronous, read-only). 6 files reviewed; 5 PASS,
1 REFINEMENT-RECOMMENDED (`causal_mask.rs`, both refinements doc-only),
0 BLOCKER.

- **R1** applied at `causal_mask.rs:29-43` — added preamble note explaining
  the deliberate input/output asymmetry of non-finite handling.
- **R2** applied at `causal_mask.rs:182-189` — added "Use case" note
  steering bulk callers from `retrocausal_safety_check` to
  `apply_mask_to_scores`.
- **R3 (deferred)** — env-var naming convention `CGREALITY_MCP_BIN` vs.
  `CGREALITY_CONTEXT_GRAPH_CLI`. Pre-production, not a bug; pick a
  convention before a third overridable binary lands.

Subagent confirmations of "no change needed":
- Unguarded n×n allocation in `build_causal_mask` — `checked_mul` is the
  only guard that matters; soft caps would push the problem upstream.
- Two-pass validate-then-mutate in `apply_mask_to_scores` — keep it; the
  FSV depends on no-mutation-on-rejection.
- MCP resolver: actual code is BETTER than handoff described.

Discoveries:
- `memory/discoveries/agent-203-code-simplifier--causal-mask-doc-asymmetry.md`
- `memory/discoveries/agent-203-code-simplifier--causal-mask-double-pass-validation.md`
- `memory/discoveries/agent-203-code-simplifier--mcp-resolver-handoff-mismatch.md`
- `memory/discoveries/agent-203-code-simplifier--retrocausal-bulk-path-doc.md`
- `memory/discoveries/agent-203-code-simplifier--unguarded-nxn-allocation-is-fine.md`

Final report: `memory/reports/agent-203-code-simplifier--tier3-causal-mask-review--final-report.md`.

## Cross-references

- Tier-1 final report: `memory/reports/agent-200-coordinator--tier1-foundation-gaps-final-report.md`
- Tier-2 final report: `memory/reports/agent-201-coordinator--tier2-foundation-gaps-final-report.md`
- Tier-3 initiative: `memory/context/tier3-foundation-gaps-initiative.md`
- Causal-masking spec: `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §8a, §8d`
- Master ME-JEPA-Code plan: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
- Upstream policy: `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`

## Next-turn handoff (deferred Tier-3 + post-Tier-3)

Five Tier-3 items remain deferred:
- §3.1 cross-pack skill transfer — depends on SONA (Tier-2 deferred).
- §3.3 coherence-driven SONA tuning — depends on SONA + sheaf coherence.
- §3.4 FastGRNN intent router — bounded ~600 LOC + calibration dataset.
- §3.5 prime-radiant sheaf-Laplacian — XL (10-12 days, sheaf cohomology).
- §3.6 manifold geometry — XL (10+ days, Riemannian on product manifolds).

Tier-2 deferred items (SONA, verified-training, Matryoshka+Graph RAG)
remain on the same long-tail. Recommend tackling §3.4 FastGRNN next as
a quality-of-life standalone; everything else either depends on Tier-2
deferred work or crosses the research-grade-math threshold.

The witness-chain hook errors should now be silent on subsequent
PostToolUse hooks. If they recur, the next-turn diagnostician should
verify `.mcp.json` still points at `target/release/context-graph-mcp`
and the release binary is fresh; the resolver will hard-error rather
than silently fall back to debug.
