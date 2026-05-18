# Phase B Root Cause And Best Practices

## Root Cause
Phase B was documented but not implemented across the live boundary surfaces:
- `RealityPrediction` only had the Phase 4 minimal fields.
- The compiler did not assemble multi-surface prediction taxonomy output.
- The predictor only exposed the panel/oracle path, not Phase B auxiliary heads.
- Training had no deterministic Phase B label extraction and no per-head UTML accounting.
- MCP exposed verify/latest/feedback only, not what-if or explain surfaces.

## Web Research Applied
Exa research was used for:
- Multi-task auxiliary heads and negative-transfer risk.
- Structured Rust error handling using explicit `Result` errors and contextual logging.
- Serde/schema-evolution risk with bincode-like binary payloads.
- Deterministic weak-label extraction and abstention instead of hallucinated labels.

## Implementation Decisions
- Phase B heads are explicit typed outputs, not generic JSON bags.
- Deterministic prediction surfaces abstain by omission when evidence is unavailable; they do not synthesize unsupported claims.
- `what_if` is non-mutating and returns repository SHA before/after snapshots.
- `explain_prediction` reads persisted `CF_MEJEPA_LIVE_PREDICTIONS` rows and normalizes saliency from stored chunks.
- Per-head UTML requires all known `HeadId::ALL` heads and fails closed on missing head state.
