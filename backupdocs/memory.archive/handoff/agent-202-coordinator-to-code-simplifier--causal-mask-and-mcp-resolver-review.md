---
namespace: handoff
agent_id: agent-202-coordinator
timestamp: 2026-05-09T01:30:00Z
status: active
tags: tier3, causal-mask, code-simplifier, review-input
---

# Handoff to code-simplifier subagent — Tier-3 review batch (2026-05-09)

## What you are reviewing

**Two patches**, both in this turn, both in scope for Tier-3 of the foundation-gaps initiative
(see `memory/context/tier3-foundation-gaps-initiative.md`).

### Patch 1 — Root-cause fix for stale debug MCP binary

- **File**: `crates/context-graph-cli/src/commands/ccreality/mod.rs`
- **Function**: `mcp_binary(root: &Path) -> CCResult<PathBuf>`
- **Before**: defaulted to `target/debug/context-graph-mcp` regardless of staleness.
- **After**: prefer `CGREALITY_MCP_BIN` env override → `target/release/...` → `target/debug/...` →
  hard error `CCREALITY_CLI_MCP_BINARY_MISSING`.
- **Why**: the recurring `CCREALITY_WITNESS_OP_TYPE_UNKNOWN` hook errors were caused by
  the engine fork/execing a stale May 8 16:15 debug binary that didn't know about
  `WitnessOperation::WitnessRepair = 8`, while the fresh release binary at 21:03 had
  the correct enum.

### Patch 2 — §3.2 Causal masking + retrocausal safety (clean-room)

- **New module**: `crates/context-graph-solver/src/causal_mask.rs` (~272 lines)
- **New tests**: `crates/context-graph-solver/src/causal_mask_tests.rs` (~118 lines, 13 tests)
- **New FSV evidence-producer**: `crates/context-graph-solver/examples/causal_mask_fsv.rs`
  (~237 lines, writes `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json`)
- **lib.rs re-exports**: `apply_mask_to_scores`, `build_causal_mask`,
  `retrocausal_safety_check`, `softmax_row`, `MaskStrategy`.
- **Per spec**: `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §8a, §8d`. Inspired by
  ruvnet/RuVector `crates/ruvector-graph-transformer/src/temporal.rs` HEAD ef5274c2.
  Clean-room reimplementation per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### Two earlier-turn fixes also in this changeset (path-attribute repair)

- `crates/context-graph-witness/src/cert.rs`: added `#[path = "cert_merkle.rs"]` so the
  `mod cert_merkle;` declaration finds the sibling file.
- `crates/context-graph-solver/src/granger.rs`: same fix for `mod granger_tests;`.

## Verification status (all green)

- `cargo build --release -p context-graph-cli -p context-graph-mcp -p context-graph-witness -p context-graph-solver` → clean.
- `cargo test --release -p context-graph-solver` → 58 passed, 0 failed (was 45 pre-causal_mask).
- `cargo test --release -p context-graph-witness -p context-graph-mincut` → all green.
- FSV regression: tier1_fsv, granger_fsv, mincut_fsv, cert_fsv all re-run, all pass.
- Causal mask FSV evidence at `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json` —
  empty=CGSOLVER_INVALID_INPUT, shape=CGSOLVER_INVALID_INPUT,
  non-finite=CGSOLVER_NUMERICAL_INVARIANT, retrocausal=CGSOLVER_INVALID_INPUT,
  scores preserved verbatim on rejection.

## What I want you to look at, in priority order

1. **Causal mask boundary conditions** — `build_causal_mask` does an unguarded
   `n * n` allocation. If the caller passes a giant timestamp array, this can OOM.
   Should we cap, or should we trust the caller? Current stance: trust (fail-fast
   on OOM is fine; deny-listing a soft cap would just push the problem upstream).
   Confirm this is the right call or push back.

2. **`apply_mask_to_scores` non-finite handling** — input scores must be finite
   (NaN OR ±INFINITY rejected with `CGSOLVER_NUMERICAL_INVARIANT`). The mask
   then writes `f32::NEG_INFINITY` to forbidden positions. Downstream `softmax_row`
   tolerates the post-mask `NEG_INFINITY` but rejects `+inf` and `NaN`.
   Is the asymmetry between input validation (no infinities) and output (NEG_INF
   allowed) confusing? Recommend a comment if so.

3. **`retrocausal_safety_check` ergonomics** — the function takes a single
   (query_idx, key_idx) pair and returns `Result<()>`. Callers iterating over
   the score matrix would call this O(N²) times — fine for small N, but signals
   that the bulk path should be `apply_mask_to_scores`. Confirm the doc string
   makes that clear.

4. **MCP binary resolver** — does the order (env > release > debug) match
   the operator's mental model? The shell helper at `scripts/cc/_common.sh:106-125`
   uses release > debug only (no env override). Should the env override be
   added to the shell helper too for consistency?

5. **Anything else** — refactor opportunities, dead code, edge cases I missed.

## Subagent contract

- READ this handoff plus the cited source files.
- WORK: write `memory/discoveries/agent-{N}-code-simplifier--*.md` for each finding,
  AND `memory/reports/agent-{N}-code-simplifier--final-report.md` summarizing
  PASS / REFINEMENT / BLOCKER per file.
- Do NOT mutate code. Recommend only.
- Cite file:line in every finding.
