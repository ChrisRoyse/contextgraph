---
namespace: journal
agent_id: agent-02-sherlock-dpo
timestamp: 2026-04-17T20:05:00Z
status: active
tags: journal, success, dpo, forensic, verdict-guilty
---

# Journal: DPO Pair Mining Forensic Investigation Complete

## What I Was Asked
Determine whether the 12,567 DPO contrastive pairs used to train shakespeare-lora-gemma4 were constructed using STYLE-discriminative embedders or SEMANTIC embedders.

## What I Did
1. Read the mission brief and three coordinator discoveries (constellations validated, search schema confirmed, guard architecture locked).
2. Read every DPO-related Python script: `shakespeare_build_dpo_dataset.py`, `shakespeare_rlhf.py`, `shakespeare_adversarial_boost.py` (turned out to be SFT-only, not DPO), `shakespeare_full_pipeline.py`, `shakespeare_rebuild_all_data.py`, `shakespeare_clean_training_data.py`, `shakespeare_explore.py`.
3. Read the Rust core contrastive module (`mining.rs`, `types.rs`, `mod.rs`) and the MCP handler (`contrastive_tools.rs`, `contrastive.rs`).
4. Directly inspected the shipped DPO JSONL files on disk (12,567 unclean + 11,724 clean records) and computed statistics over `_meta.similarity_profile`.
5. Confirmed anomaly_kind distribution (100% "other"), embedder means (E1=0.928, E12=0.901), and chosen-passage dedup (616 distinct chosen in 11,724 pairs).

## What I Found
- The Rust core miner's classification rules put E1_Semantic at the center of high/low criteria. No rule captures "high E1 + low style embedders" — the signature of a topic-match-style-miss anomaly.
- The miner samples the entire daemon corpus which at Shakespeare-ingest time was ~100% Shakespeare. Every pair is Shakespeare-vs-Shakespeare.
- The Python DPO pipeline has zero style-discriminative filtering. Pairs are fetched and written verbatim.
- The actual data confirms: chosen/rejected have nearly identical style (E12=0.901, E7=0.810) and nearly identical topic (E1=0.928). The "contrast" the model sees is sparse-keyword disagreement (E6/E13≈0.16) — lexical, not stylistic.
- First 10 pairs all share the same chosen passage, rejected against 10 different Shakespeare scenes — DPO is teaching topic preference, not style preference.

## Verdict
GUILTY. DPO mining used SEMANTIC-oriented criteria. The model learned to prefer one Shakespeare-topic passage over another Shakespeare-topic passage. It did NOT learn to prefer Shakespeare-style over non-Shakespeare-style.

## Surprise Factor
Moderately surprising. Going in, I expected to find either explicit `primary_embedder="E1"` or explicit `contrast_embedder="E7"` knobs being misused. Instead the tool has no such knobs at all — the miner always computes all 13 embedders and classifies by fixed cross-embedder-anomaly rules. This is a different and subtler form of the same problem: the rules themselves don't surface style anomalies.

## What I Could Not Verify
- Whether `mine_contrastive_pairs` was invoked with a specific `sessionFilter` that narrowed to Shakespeare-only (it likely was, or at minimum the daemon held only Shakespeare at the time). Either way the conclusion is the same: all candidates were Shakespeare-style-uniform.
- The exact command line that produced the ~/.contextgraph/training/shakespeare_dpo.jsonl file — no audit log survives. The file was produced ~Apr 15 13:57 (mtime). I rely on the metadata embedded in the records.
