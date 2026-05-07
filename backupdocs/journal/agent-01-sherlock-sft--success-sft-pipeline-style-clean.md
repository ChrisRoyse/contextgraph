---
namespace: journal
agent_id: agent-01-sherlock-sft
timestamp: 2026-04-17T18:15:00Z
status: success
tags: sft, forensic-investigation, lsu, verdict-clean
---

# Investigation Log — SFT Pipeline Semantic-Embedder Audit

## Trigger
Coordinator rewrote the Layer-3 style guard to remove E1_Semantic/E5_Causal
from verdict calculation because those embedders measure TOPIC not STYLE.
Concern: did the SFT training data also use those same semantic embedders
as a filter/ranker? If yes, the LoRA would be topic-overfit.

## Method
Linear Sequential Unmasking (LSU): examined code and actual training
artefacts BEFORE reading the coordinator's architectural decisions, then
cross-checked against the stated three-layer design.

## Key Finds (Chronological)
1. Read mission brief + guard architecture decisions — established the
   "semantic embedders are for topic, not style" thesis.
2. Read every SFT-building script in `scripts/shakespeare_*.py`.
3. grep for `search_graph|search_by_embedder|customWeights|weightProfile`
   across SFT scripts → **zero matches** in the training pipeline. Only demo
   / inference scripts reference those primitives.
4. Traced content origin: every SFT example's assistant text comes from
   `list_training_records` (paginated enumeration) or
   `get_training_record(memory_id)` (UUID lookup). Neither applies semantic
   ranking.
5. Identified `score_against_constellation` / `combined_score` as the ONLY
   semantic touchpoint. Its implementation averages ALL 13 embedder cosines
   — so it IS semantic-contaminated. BUT...
6. Read the actual _meta fields in production training files
   (`shakespeare_sft.jsonl`, `shakespeare_sft_enhanced.jsonl`,
   `shakespeare_sft_final_clean.jsonl`). All 994 entries in
   `shakespeare_sft.jsonl`: `work: "General"`, `work_score: 0.0`. All 279
   `curated_creative` entries: `verdict: "UNSCORED"`, `score: 0.5`. This
   means constellation scoring was skipped in production.
7. Read every `f.write(...)` in SFT-building scripts: all writes are
   unconditional. There is no `if score > threshold` gate anywhere. Even
   where constellation scoring runs, the score is stored as metadata and
   prepended to `system_extra`, never used to filter.

## Verdict
SFT training is **STYLE-CLEAN**: it was built by exhaustive coverage of the
Shakespeare RocksDB corpus plus Gemma-generated creative additions. No
semantic-similarity ranking filtered which examples entered the set. The
Layer-1 LoRA is not topic-overfit via the SFT pipeline.

## What I Could Not Verify
- Whether DPO contrastive pairs (`shakespeare_dpo.jsonl`) were ranked by
  E1 semantic similarity. **Out of scope** — Agent 2's territory.
- Whether inference-time prompt routing uses E1 search. **Out of scope** —
  Agent 3's territory.
- Whether the `gemma4_generated` prompt REWRITES (from
  `shakespeare_enhance_prompts.py`) accidentally inject topic bias. The
  prompt generator reads only the Shakespeare text snippet (first 400 chars)
  and asks Gemma to infer a modern prompt. No semantic search over the
  corpus is involved, but the prompt-Gemma interaction is a potential bias
  surface I did not quantify.

## Related
- See also: ./memory/reports/agent-01-sherlock-sft--final-report.md
