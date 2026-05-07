---
namespace: discoveries
agent_id: coordinator
timestamp: 2026-04-17T15:28:00Z
status: active
tags: mcp, search_graph, store_memory, embedderScores, schema
---

# Search/Store MCP Response Schemas (Confirmed Live 2026-04-17)

## Why It Matters
The dashboard at `scripts/shakespeare_chat_dashboard.py:262-272` reads per-embedder scores via specific nested keys. Confirming the live schema prevents brittle parsing bugs. **The dashboard's existing key layout is correct** — no renames needed.

## `store_memory` Response
```json
{"embedderCount":13,"embeddingLatencyMs":717,"fingerprintId":"<uuid>"}
```
- REQUIRED keys in response: `fingerprintId`, `embedderCount`, `embeddingLatencyMs`
- Guard MUST assert `fingerprintId` is non-empty. `embedderCount == 13` is a secondary sanity check.
- Latency observed: ~700ms for the 140-char probe. Embedding 13 models is NOT free.

## `search_graph` Response (with `includeEmbedderBreakdown: true`)
Per-result shape:
```json
{
  "fingerprintId": "<uuid>",
  "similarity": 0.9211,
  "e1Score": 0.965,
  "agreementCount": 11,
  "agreementLevel": "high",
  "dominantEmbedder": "E1_Semantic",
  "embedderScores": {
    "semantic":   {"E1_Semantic", "E5_Causal", "E6_Sparse", "E7_Code", "E10_Multimodal", "E12_ColBERT", "E13_SPLADE"},
    "relational": {"E8_Graph", "E11_Entity"},
    "structural": {"E9_HDC"},
    "temporal":   {"E2_Recency", "E3_Periodic", "E4_Sequence"}
  },
  "embedderBreakdown": [...],   // flat array of {embedder, rank, score, weight, rrfContribution}
  "provenance": {...},
  "sequenceInfo": {...},
  "source": {...},
  "navigationHints": [...]
}
```

## Critical Edge Cases
1. **`E5_Causal` is `null`** when no causal direction is detected (non-causal Shakespeare prose). The memory_tools backend emits `Value::Null` for score < 0 (sentinel). Python `or 0` collapses null→0 and then the dashboard's `if actual <= 0: continue` skips E5. E5 isn't in STYLE_CENTROID, so this is harmless for Shakespeare.
2. **`E11_Entity`: 0.0** is possible (Entity model may emit zero-norm vector for low-entity text). Not in STYLE_CENTROID.
3. **Top result may not be the just-stored memory** if an older memory has higher similarity. In the probe, the just-stored memory (uuid cde6707...) WAS the top result with similarity 0.92. But for short / common phrases this could differ. Guard MUST verify by matching `result.fingerprintId == store.fingerprintId` — existing dashboard does NOT verify this, which is a bug.
4. **`searchTransparency.activeEmbedders`** lists which embedders actually got weighted into fusion. Under default `multi_space`, E2/E3/E4 are weight-gated (0) and E11 is disabled. But their SCORES are still returned in `embedderScores` — they just don't contribute to RRF ranking. STYLE_CENTROID uses E4 and E1/E6/E7/E8/E9/E10/E12/E13 all of which are present.

## Live Scores for Shakespeare Probe (reference values for tests)
Text: "Hark! Wherefore art thou skeptical of neural nets? Thou shalt see methinks they doth learn as we mortals dream. Verily, 'tis a goodly marvel of our age."

- E1_Semantic: 0.9650 (centroid 0.9019, p5 0.8923) ✓
- E4_Sequence: 0.5012 (centroid 0.5242, p5 0.4821) ✓
- E6_Sparse: 0.8600 (centroid 0.5701, p5 0.5260) ✓
- E7_Code: 0.9360 (centroid 0.8160, p5 0.7664) ✓
- E8_Graph: 0.8282 (centroid 0.7833, p5 0.7698) ✓
- E9_HDC: 0.7159 (centroid 0.5465, p5 0.4998) ✓
- E10_Multimodal: 0.9560 (centroid 0.8960, p5 0.8862) ✓
- E12_ColBERT: 0.8540 (centroid 0.6240, p5 0.5556) ✓
- E13_SPLADE: 0.8186 (centroid 0.5677, p5 0.5253) ✓

9/9 above p5×0.8 threshold → style_score = 1.00 → PASS. This is the HAPPY-PATH baseline.

## Related
- See also: ./memory/context/mission-brief-shakespeare-guard-hardening.md
- See also: ./memory/context/constraints-shakespeare-guard.md
