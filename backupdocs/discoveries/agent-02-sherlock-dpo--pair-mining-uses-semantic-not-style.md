---
namespace: discoveries
agent_id: agent-02-sherlock-dpo
timestamp: 2026-04-17T20:00:00Z
status: active
tags: dpo, contrastive-pairs, style-guard, semantic-vs-style, smoking-gun, forensic
---

# DPO Pair Mining Uses SEMANTIC Embedders, Not STYLE Embedders

## Bottom Line
The 12,567 DPO pairs used to train shakespeare-lora were mined using **cross-embedder ANOMALY** axes that put E1_Semantic (topic) front-and-center, not style-discriminative embedders. The actual shipped dataset is 100% `anomaly_kind="other"` with chosen/rejected pairs that have **nearly identical STYLE** (E12_ColBERT mean 0.901, E7_Code mean 0.810) and nearly identical TOPIC (E1 mean 0.928) — contrast emerging mostly from sparse keyword overlap (E6/E13 ~0.16).

In plain terms: DPO taught the model "prefer this Shakespeare scene over that Shakespeare scene" where both are written in flawless Shakespeare style. It did **not** teach "prefer Shakespeare-STYLE over non-Shakespeare-STYLE."

## Crime Scene Physical Evidence

### 1. Rust core — the mining rules (`crates/context-graph-core/src/contrastive/mining.rs:104-134`)
```
classify_anomaly priority order:
  1. high E7 + low E1  → CodeShapeButDifferentIntent
  2. high E11 + low E8 → EntitySharedButDifferentStructure
  3. high E9 + low E1  → HdcRobustButSemanticDifferent
  4. high E6 || E13 + low E10 → KeywordButNotParaphrase
  5. high E1 + low E5  → SemanticButNotCausal
  6. else              → Other
```
E1 (semantic/topic) is used as both the high criterion (rule 5) and the low criterion (rules 1, 3). There is no rule "high E1 semantic, low E7/E12/E13 style" that would surface pairs where topic matches but style differs. The six kinds partition STRUCTURAL/SEMANTIC anomalies — none of them partition on STYLE.

### 2. Rust core — candidate pool selection (`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:175-189`)
```rust
let pool = self.teleological_store
    .list_fingerprints_unbiased(candidate_pool_size)  // default 500
    .await
```
The miner samples 500 unbiased fingerprints from the entire daemon corpus, then scores each anchor against every other pool member. At Shakespeare-ingest time the daemon was ~100% Shakespeare chunks (see `shakespeare_ingest.py`, `memory/MEMORY.md`), so every sampled pair is Shakespeare-vs-Shakespeare.

### 3. Shipped DPO data — forensic audit of `~/.contextgraph/training/shakespeare_dpo.jsonl`
- **Total records**: 12,567 (unclean) / 11,724 (clean)
- **anomaly_kind distribution**: 100% `other`, 0% of any named kind
- **616 distinct CHOSEN passages → 11,724 pairs** (avg 19 negatives per anchor, max 57)
- **Similarity profile statistics** (across all 11,724 clean pairs):
  - E1 Semantic (TOPIC):     mean 0.928, stdev 0.015 — chosen/rejected are topically near-identical
  - E6 Sparse (KEYWORD):     mean 0.162 — low lexical overlap
  - E7 Code (STYLE):         mean 0.810 — high stylistic similarity
  - E12 ColBERT (STYLE):     mean 0.901 — very high stylistic similarity
  - E13 SPLADE (KEYWORD):    mean 0.158 — low lexical overlap
  - E11 Entity:              0.000 (zero-norm; E11 is disabled per MEMORY.md)
- Implication: the "contrastive" signal that drove the pairs into CF_CONTRASTIVE_PAIRS was **E6/E13 keyword disagreement**, not style contrast. Both chosen and rejected passages are stylistically indistinguishable.

### 4. First 10 pairs — all share chosen="SCENE I. Coventry (Warwick)"
Every rejected is a different Shakespeare scene (Westminster, Cyprus, London, Field of Battle near Barnet, the nunnery, etc.) — all also pure Shakespeare. The DPO model is being asked to prefer the Warwick-Coventry passage over all those other Shakespeare scenes purely because the **prompt** was synthesized as "Write about: SCENE I. Coventry" (see `shakespeare_build_dpo_dataset.py:50-64`, or PROMPT_TEMPLATE in `shakespeare_rlhf.py:28-32`). This is TOPIC preference, not STYLE preference.

### 5. Python DPO pipeline has NO style filter
- `shakespeare_build_dpo_dataset.py` — fetches pre-mined pairs via `list_contrastive_pairs({includeFull:true})` and writes them verbatim. No style check.
- `shakespeare_rlhf.py` — fetches pairs via `list_contrastive_pairs`+`get_contrastive_pair`, writes them verbatim. No style check.
- `shakespeare_rebuild_all_data.py:96-137` (`clean_dpo_data`) — strips stage directions, drops pairs where chosen/rejected become too short or identical after cleaning. **No embedder re-evaluation**. A pair can have E12 cosine 0.99 between chosen and rejected and still pass.
- No caller of `mine_contrastive_pairs` sets `primary_embedder` or `contrast_embedder` arguments — the MCP schema for that tool does not expose any such knobs (see `crates/context-graph-mcp/src/tools/definitions/contrastive.rs`). The tool has no notion of a "primary" / "contrast" embedder at all.

## What a Style-Correct DPO Would Look Like
Target: chosen = Shakespeare text; rejected = same topic/prompt but in a different literary style (modern English, academic prose, text-message slang, etc.). Contrastive signal should be:
- E1 Semantic: HIGH (same topic) for both chosen and rejected
- E12 ColBERT / E7 Code / E13 SPLADE: HIGH on chosen vs Shakespeare reference, LOW on rejected
- **Crucially**: rejected must be off-STYLE, not just off-TOPIC

Neither `mine_contrastive_pairs` (as currently implemented) nor any of the Shakespeare scripts produce this. The miner sees only Shakespeare and can only find differences *within* Shakespeare.

## Score Certainty
Confidence: HIGH.
- Rust source code directly inspected (mining.rs, types.rs, contrastive_tools.rs).
- Shipped DPO JSONL files directly inspected (12,567 records, anomaly_kind frequencies, similarity_profile statistics).
- Python DPO pipeline directly inspected (all three relevant scripts).
- No alternative hypothesis survives: the data's own metadata (`_meta.similarity_profile`) confirms chosen/rejected share style.

## Cross-references
- See also: `memory/reports/agent-02-sherlock-dpo--final-report.md`
- Mission: `memory/context/mission-brief-shakespeare-guard-hardening.md`
- Source of truth for style centroid: `memory/discoveries/coordinator--constellation-anchors-validated.md` (Sonnets constellation 0.418 combined_score vs modern 0.254)
