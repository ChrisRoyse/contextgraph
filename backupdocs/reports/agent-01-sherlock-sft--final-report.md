---
namespace: reports
agent_id: agent-01-sherlock-sft
timestamp: 2026-04-17T18:20:00Z
status: complete
tags: sft, verdict-yes, style-clean, forensic-final
---

# Final Report — SFT Pipeline Semantic-Embedder Audit

## VERDICT: YES — SFT training was style-correct
(The pipeline did NOT use E1_Semantic, E5_Causal, or any semantic embedder
to filter, rank, or select training examples.)

Confidence: **HIGH** for the selection mechanism. **MEDIUM** for secondary
concerns noted below.

## Executive Summary
The Layer-1 LoRA training data in
`~/.contextgraph/training/shakespeare_sft_final_clean.jsonl` (7,925
examples, used in the production HuggingFace v2.1 model) was assembled by:

1. Exhaustive coverage of the Shakespeare RocksDB corpus via
   `list_training_records` (paginated enumeration) and
   `get_training_record(memory_id)` (UUID lookup) — both return records
   *by insertion order / UUID*, with no embedder-weighted ranking.
2. Gemma-generated creative additions — kept unconditionally (constellation
   scoring was skipped via `--skip-scoring`, evidenced by all 279 entries
   tagged `verdict: "UNSCORED"`).
3. Hardcoded adversarial / foreign-language / topic-coverage seeds.
4. Typed-edge traversal for graph-continuation examples (edges come from
   causal discovery, not E1 search).

No script in the SFT-building chain invokes `search_graph`,
`search_by_embedder`, `customWeights`, `weightProfile`, or any
embedder-specific similarity filter. Constellation scoring (the one
semantic-tainted scoring path, because `combined_score` averages E1/E5
cosine) is used PURELY as metadata when it runs at all — every
`f.write(example)` in every SFT builder is unconditional. And in the
production artefacts, constellation scoring was evidently skipped
(metadata uniformly shows `"work": "General", "work_score": 0.0`).

## Evidence (exact file:line references)

### Grep verification — no semantic-search primitives in SFT scripts
Pattern: `search_graph|search_by_embedder|customWeights|weightProfile|multi_space|e1_only`
Searched: all `scripts/shakespeare_*.py`
Matches in SFT-building scripts:

- `shakespeare_build_sft_dataset.py`: **0**
- `shakespeare_build_optimal_dataset.py`: **0**
- `shakespeare_build_creative_dataset.py`: **0**
- `shakespeare_build_full_dataset.py`: **0**
- `shakespeare_build_dpo_dataset.py`: **0**
- `shakespeare_enhance_prompts.py`: **0**
- `shakespeare_extract_all.py`: **0**
- `shakespeare_extract_all_signals.py`: **0**
- `shakespeare_extract_full_signals.py`: **0**
- `shakespeare_clean_training_data.py`: **0**
- `shakespeare_rebuild_all_data.py`: **0**

Matches only in non-training scripts: `shakespeare_demo_queries.py`,
`shakespeare_genre_router.py`, `shakespeare_demo_summary.py` — none feed
training data.

### Content retrieval is by ID, not by search
- `scripts/shakespeare_build_sft_dataset.py:72-89` — `get_all_memory_ids`
  calls `list_training_records` (paginated); `get_content_for_memory` calls
  `get_training_record(memoryId)`. No search.
- `scripts/shakespeare_build_optimal_dataset.py:141-196` — paginated
  `list_training_records` feeds a `content_cache` keyed by memory_id.
- `scripts/shakespeare_extract_all.py:81-108` — paginated
  `list_training_records` + `get_training_record`. No search.
- `scripts/shakespeare_extract_all_signals.py:174-208` — same pattern.
- `scripts/shakespeare_extract_full_signals.py:150-182` — same pattern.

### Writes are unconditional
- `scripts/shakespeare_build_sft_dataset.py:237-238` —
  `f.write(json.dumps(example, ...) + "\n"); written += 1` — no score gate.
- `scripts/shakespeare_build_optimal_dataset.py:316-319` — `for ex in
  all_sft: f.write(...)` — no filter.
- `scripts/shakespeare_extract_all.py:329-331` — same.
- `scripts/shakespeare_extract_all_signals.py:449-451` — same.
- `scripts/shakespeare_extract_full_signals.py:561-563` — same.

### Constellation scoring is decorative (when it runs at all)
- `scripts/shakespeare_build_sft_dataset.py:121-146` — `build_sft_example`
  inserts `work_name` into the system prompt tag and `work_score` into
  `_meta`. Returned unconditionally.
- `scripts/shakespeare_build_optimal_dataset.py:242-254` — graph examples
  annotate `system_extra` with work / edge info. No filter.
- `scripts/shakespeare_extract_all_signals.py:303-308` — embedder scores
  become system-prompt tags (`"Style: {work}. {ename} signal: {score:.3f}"`).
  No filter.

### Production evidence scoring was SKIPPED
Empirical check of metadata in
`~/.contextgraph/training/shakespeare_sft_final_clean.jsonl`:
- `gemma4_generated` entries (4,709): all have `work: "General"`,
  `work_score: 0.0`. This is only possible via the
  `--skip-constellation-scoring` branch in
  `shakespeare_build_sft_dataset.py:213-215`, OR via the
  `shakespeare_enhance_prompts.py` path which never scores at all.
- `curated_creative` entries (279): all have
  `verdict: "UNSCORED"`, `score: 0.5`, `work: "N/A"`. This is only possible
  via `shakespeare_build_creative_dataset.py:230-231`:
  `else: score, verdict, work = (0.5, "UNSCORED", "N/A")` (the
  `--skip-scoring` branch).

### The one candidate-ranking path
`scripts/shakespeare_build_creative_dataset.py:243-246`:
```python
candidates.sort(key=lambda c: -c["score"])
best = candidates[0]
```
This picks the highest-`combined_score` Gemma candidate from N=4 generated
for the SAME prompt at temps [0.7, 0.8, 0.9, 1.0]. It is NOT a semantic
filter over the corpus — it is intra-prompt candidate selection. AND the
production run skipped it (`--skip-scoring`, see UNSCORED evidence above).

## Fix Needed If NO
Not applicable — verdict is YES. No fix required for the SFT pipeline.

## What I Could NOT Verify
1. **Prompt generation bias.** `shakespeare_enhance_prompts.py` pipes a
   400-char snippet of each Shakespeare passage into Gemma and asks Gemma
   to infer a modern prompt. The system prompt tells Gemma *"The prompt
   should describe the THEME, EMOTION, or REQUEST"* — Gemma's topic
   interpretation is therefore baked into 4,709 of the 7,925 SFT prompts.
   This is Gemma bias, not Context-Graph semantic-embedder bias, but it
   could still correlate prompts with topics present in the source
   passage. **Scope note**: this was not raised as a concern in the brief
   and is not a Context-Graph semantic-embedder issue.
2. **DPO contrastive pair selection.** Out of scope — Agent 2.
3. **Runtime inference routing.** Out of scope — Agent 3.
4. **`shakespeare_sft_optimal.jsonl` (8,857 entries) vs
   `shakespeare_sft_final_clean.jsonl` (7,925 entries)**: the cleaning step
   only removes by text length and regex (see
   `shakespeare_rebuild_all_data.py:71-93`). No semantic filter.

## Files I Wrote (for Agents 2 and 3)
- `./memory/discoveries/agent-01-sherlock-sft--selection-mechanism-confirmed.md`
- `./memory/discoveries/agent-01-sherlock-sft--constellation-scoring-semantic-but-decorative.md`
- `./memory/journal/agent-01-sherlock-sft--success-sft-pipeline-style-clean.md`
- `./memory/reports/agent-01-sherlock-sft--final-report.md` (this file)

## Relevant Source Files Examined
- `/home/cabdru/contextgraph/scripts/shakespeare_build_sft_dataset.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_build_optimal_dataset.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_build_creative_dataset.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_build_full_dataset.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_enhance_prompts.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_extract_all.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_extract_all_signals.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_extract_full_signals.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_clean_training_data.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_rebuild_all_data.py`
- `/home/cabdru/contextgraph/scripts/shakespeare_full_pipeline.py`
- `/home/cabdru/contextgraph/scripts/_gemma_shakespeare_lib.py`
- `/home/cabdru/contextgraph/crates/context-graph-core/src/constellation/compiler.rs` (combined_score math)
- `/home/cabdru/contextgraph/crates/context-graph-core/src/constellation/types.rs` (ConstellationScoringResult)
- `/home/cabdru/contextgraph/tmp/SITREP_2026-04-16_135543.md`
- `/home/cabdru/contextgraph/tmp/SITREP_2026-04-17_102816.md`

## Related
- See also: ./memory/context/mission-brief-shakespeare-guard-hardening.md
- See also: ./memory/decisions/coordinator--guard-architecture.md
