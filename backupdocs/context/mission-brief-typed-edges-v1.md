---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-14T23:10:00Z
status: active
tags: mission, typed-edges, training-data, contrastive, llm-validation, relational-signature
---

# Mission: Turn Typed Edges Into First-Class Training-Data Products

## Thesis
`CF_TYPED_EDGES` holds millions of auto-derived `(source, target, edge_type, weight, embedder_scores[13], agreement_count, agreeing_embedders)` tuples. Today these edges are only used as a secondary lookup for graph traversal. They are in fact a **labeled relationship dataset** — exactly the artifact frontier labs pay Scale $50-$500 per example to curate. We are sitting on them and exporting zero.

This mission ships four coherent capabilities that turn typed edges into a first-class training-data asset.

## Four Features To Land

### Feature 1 — `export_typed_edges_corpus` MCP tool
Iterate `CF_TYPED_EDGES`, emit per-edge `TypedEdgeTrainingRecord` containing:
- `source_memory_id`, `target_memory_id`
- `edge_type` (u8 + string name)
- `weight` (f32)
- `direction` (Symmetric | Forward | Backward)
- `embedder_scores: [f32; 13]`
- `agreement_count: u8`
- `agreeing_embedders: u16` (bitset)
- `source_content`, `target_content` (optional, on flag)
- `source_source_metadata`, `target_source_metadata` (optional, on flag)
- `mechanism_type` (Option<String> — pulled from `CausalRelationship` if `edge_type == CausalChain`)

Persist to a NEW CF `CF_TYPED_EDGE_RECORDS` keyed by `[source:16][target:16][edge_type:u8]` (33 bytes; edge-type suffix lets us export multiple edge classifications per pair). Wire format: `[version_byte=1][bincode]`.

### Feature 2 — Derive AnomalyKind pairs from CF_TYPED_EDGES
New MCP tool `derive_anomalies_from_edges` runs a deterministic pass over the typed edge table and mines the five AnomalyKinds that are expressible in edge-type terms:

| Source edge pattern | AnomalyKind |
|---|---|
| High `SemanticSimilar` weight + absent or weak `CausalChain` | `semantic_but_not_causal` |
| High `KeywordOverlap` + absent `ParaphraseAligned` | `keyword_but_not_paraphrase` |
| High `CodeRelated` + divergent `SemanticSimilar` | `code_shape_but_different_intent` |
| High `EntityShared` + absent `GraphConnected` | `entity_shared_but_different_structure` |
| High `SemanticSimilar` (E9-driven) + divergent E1 in `embedder_scores` | `hdc_robust_but_semantic_different` |

Output rows go straight into `CF_CONTRASTIVE_PAIRS` (same shape as the existing miner, same atomic WriteBatch across primary + `by_kind` + `by_anchor`) so the unified contrastive export keeps working.

### Feature 3 — 8-dim relational signature on `TrainingRecord`
Add `edge_type_distribution: [u32; 8]` to `TrainingRecord` in `core/src/training/mod.rs`. Computed from `CF_TYPED_EDGES` outgoing-edges scan at export time (already walked for `outgoing_edges`, reuse the iterator). Indexes:

```
[0] SemanticSimilar, [1] CodeRelated, [2] EntityShared, [3] CausalChain,
[4] GraphConnected, [5] ParaphraseAligned, [6] KeywordOverlap, [7] MultiAgreement
```

Bump `TRAINING_RECORD_VERSION` to **2**. NO backwards compatibility — version 1 reads fail explicitly with `CoreError::SerializationError { message: "TrainingRecord v1 cannot be read by v2 decoder; re-export required" }`.

### Feature 4 — LLM validation feedback loop
New CF `CF_TYPED_EDGE_VALIDATIONS` keyed by `[source:16][target:16][edge_type:u8]`. Value: `[version:u8=1][bincode LLMEdgeValidation]` where:

```rust
pub struct LLMEdgeValidation {
    pub validated_at: DateTime<Utc>,
    pub llm_verdict: LLMVerdict,      // Valid | Invalid | Reclassify { new_type: u8 }
    pub llm_confidence: f32,          // [0, 1]
    pub llm_rationale: String,        // 1-3 sentences from grammar-constrained output
    pub auto_derived_weight: f32,     // copy of edge.weight at validation time
    pub validator_version: String,    // "Hermes-2-Pro-Mistral-7B-Q5_K_M@2026-04-14"
    pub prompt_hash: [u8; 32],        // SHA-256 of the validation prompt (reproducibility)
}
```

New MCP tool `validate_low_confidence_edges` accepts `{ weight_ceiling: f32 (default 0.60), max_edges: u32 (default 100), session_filter: Option<String> }`. For each edge with weight below ceiling that has no existing validation, calls `validate_graph_link` through the graph-agent LLM, writes the result, returns counts.

Export Feature 1 joins with validations: if a `LLMEdgeValidation` exists for the edge, it is included in the exported record. Exports now carry BOTH the embedder-ensemble verdict AND the LLM verdict — the exact "two training signals" claim from the product brief.

## Non-Negotiables (Reaffirmed)

1. **NO backwards compatibility.** Bump `TRAINING_RECORD_VERSION` to 2. Old records fail with a clear error, no auto-migration.
2. **NO mocks.** Tests use real RocksDB + real TypedEdge data generated from synthetic fingerprints.
3. **NO cover-up tests.** If a test's expected count depends on a hardcoded assumption, verify the assumption matches reality.
4. **Fail fast with structured errors.** No silent `unwrap_or_default()` paths.
5. **No writes to repo root.** New files go under `crates/*/src/`, `crates/*/tests/`, `memory/`, `tasks/`, `docs/`.
6. **Files < 500 lines.** If a module gets longer, split.

## Source of Truth
RocksDB at `{test_tempdir}/` in tests; `~/.contextgraph/data/` at runtime.
- Four CFs touched: `CF_TYPED_EDGES` (read), `CF_TYPED_EDGE_RECORDS` (new, write), `CF_TYPED_EDGE_VALIDATIONS` (new, write), `CF_CONTRASTIVE_PAIRS` (write — Feature 2).
- Three CFs grow in schema count: `TOTAL_COLUMN_FAMILIES` goes from 57 → 59.

## Success Criteria
1. `cargo check --workspace` passes with `CUDA_HOME=/usr/local/cuda`.
2. `cargo test --workspace` passes including new integration tests.
3. Four new MCP tools appear in `tools/list` output (71 → 75 total).
4. Synthetic FSV demo: store 20 memories, generate typed edges, export typed-edge corpus (expect ≥ 40 edge records), derive anomalies (expect ≥ 5 synthetic anomalies with predicted kinds), run LLM validation on 3 low-confidence edges (if LLM feature enabled, verify row written to `CF_TYPED_EDGE_VALIDATIONS`).
5. `TrainingRecord` v2 contains `edge_type_distribution` summing equal to total outgoing edges per memory on synthetic fixture.
