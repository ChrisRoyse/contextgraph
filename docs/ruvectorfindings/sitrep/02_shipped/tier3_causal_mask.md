# Tier-3 Causal Mask + Retrocausal Safety

> **Shipped:** 2026-05-08 / 2026-05-09 (MCP root-cause fix follow-up)
> **Crate:** `crates/context-graph-causal/src/mask.rs`
> **Tests:** 33 passing (causal mask) + 8 passing (MCP integration)
> **FSV evidence:** `/tmp/contextgraph-tier3-causal-mask-fsv/`

## What this primitive does

`MaskStrategy` enum + companion `apply_mask` function. Doc 05 §3.2
specifies that any temporal/causal attention pattern in ContextGraph
must reject "cheating" attention from t toward t+k (retrocausal flow)
or arbitrary cross-time leakage.

Three strategies:

1. **`Strict { current_t }`** — only allow attention from `t' ≤ t`
   where `t'` is each token's effective time index. Rejects every
   attention edge where source > target by even a single index.
   This is the safe default for predict-from-past tasks.

2. **`TimeWindow { current_t, window: usize }`** — like Strict but with
   a maximum back-window. Tokens older than `current_t - window` are
   masked out (irrelevant past).

3. **`CausalWithSelf { current_t }`** — Strict but allows attending to
   self (`t' = t`). Useful for residual attention.

The `apply_mask(scores, time_indices, strategy) -> Result<Tensor>`
function applies the chosen strategy to a 2D attention-score matrix,
zeroing out (or `-inf`-ing for softmax) the rejected pairs.

## Why it matters

ME-JEPA's predictor (Phase 2) maps `panel[t=0..1] → panel[t=2]`. The
context encoder's internal attention must NOT let `panel[t=1]` attend
to itself in a way that leaks `panel[t=2]` information through any
positional encoding shortcut. Doc 05 §3.2's masking guarantees the
predictor learns a genuine forward map, not a degenerate "copy through
embedding" map.

The retrocausal-safety guard is also load-bearing for **Phase 7
trigger-decision**: a verdict on a candidate patch must NOT use
information from "future" attempts on the same task to predict the
current attempt's pass rate.

## Implementation notes

### Strategy enum
- `MaskStrategy::Strict { current_t }`
- `MaskStrategy::TimeWindow { current_t, window }`
- `MaskStrategy::CausalWithSelf { current_t }`
- All variants carry the `current_t` so multiple sub-attentions can
  share a single instance for batch consistency.

### apply_mask
- `fn apply_mask(scores: &mut Tensor, time_indices: &[u32], strategy:
  MaskStrategy) -> CausalResult<()>`
- Validates `scores.shape() == [time_indices.len(); 2]`.
- Validates `time_indices.is_sorted()` for predictable masking.
- Returns explicit error `MASK_RETROCAUSAL_TIME_INDEX_OUT_OF_ORDER`
  when ordering invariant breaks.

### MCP integration root-cause fix
The Tier-3 §3.2 mask was initially silently ignored by the MCP
`reality_query` handler. Diagnosis traced this to a missing `strategy:
MaskStrategy` propagation in `query_executor::run_attention_pass`.
Root-cause fix: `QueryConfig` now carries `mask_strategy: Option<MaskStrategy>`
and the executor threads it through every attention pass; default of
`None` is rejected when the query topology declares a temporal axis.

### FSV
Physical FSV at `/tmp/contextgraph-tier3-causal-mask-fsv/`:
- `strict-3x3-evidence.json` — 3-token attention matrix; mask zeros
  out exactly the upper triangle, leaves lower triangle and diagonal
  intact. Round-trip byte-equal.
- `time-window-evidence.json` — 8-token sequence with window=2; only
  positions `[t-2, t-1, t]` survive per row.
- `retrocausal-rejection-evidence.json` — synthetic out-of-order
  `time_indices = [3, 1, 2]` produces explicit
  `MASK_RETROCAUSAL_TIME_INDEX_OUT_OF_ORDER` error rather than silent
  wrong masking.

## What this enables downstream

- **Phase 2 predictor** — context encoder applies `MaskStrategy::Strict`
  internally so the predictor's attention can't see future panel slots.
- **Phase 7 trigger decision** — `MaskStrategy::TimeWindow` filters
  out historical attempts older than 30 days when consulting ME-JEPA
  for the current attempt verdict.
- **Tier-3 §3.3..§3.6** (deferred) — all build on this masking primitive.

## See also

- Spec: `docs/ruvectorfindings/05_TIERED_PRIMITIVE_LADDER.md §3.2`
- MCP integration: `crates/context-graph-mcp/src/handlers/tools/causal/`
  (the executor patch from 2026-05-09)
- Phase 2 reuse: `03_deferred/phases/phase2.md` Component B
- Source: `crates/context-graph-causal/src/mask.rs`
