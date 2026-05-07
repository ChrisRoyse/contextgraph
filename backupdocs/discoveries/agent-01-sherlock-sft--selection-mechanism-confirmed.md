---
namespace: discoveries
agent_id: agent-01-sherlock-sft
timestamp: 2026-04-17T18:00:00Z
status: active
tags: sft, training-data, selection-mechanism, no-semantic-search, forensic
---

# SFT Example Selection Mechanism — Forensically Verified

## Claim
The SFT training pipeline selected examples by **direct memory-ID retrieval
through `list_training_records`**, NOT by semantic search or
embedder-weighted ranking. No E1/E5 semantic cosine filtering was applied to
decide which Shakespeare passages entered the training set.

## Evidence Path

### The production SFT file
`~/.contextgraph/training/shakespeare_sft_final_clean.jsonl` (7,925 examples,
used by `shakespeare_full_pipeline.py:phase2_sft` to train v2.1 on HuggingFace)

### `prompt_source` breakdown of that file
| Source | Count | Origin script | Selection mechanism |
|--------|-------|---------------|---------------------|
| `gemma4_generated` | 4,709 | `shakespeare_build_sft_dataset.py` → `shakespeare_enhance_prompts.py` | `list_training_records` paginated (no search) |
| `graph_entity` | 2,541 | `shakespeare_build_optimal_dataset.py` | typed-edge traversal via `get_training_record(id)` |
| `curated_creative` | 279 | `shakespeare_build_creative_dataset.py` | Gemma generation, **UNSCORED** (see below) |
| `composition` | 230 | `shakespeare_full_pipeline.py:phase1_compositions` | Gemma generation, no scoring |
| `intra_work` | 123 | `shakespeare_build_optimal_dataset.py` | random same-work pairs |
| `adversarial_style_break` | 30 | `shakespeare_rebuild_all_data.py:rewrite_adversarial_data` | hardcoded |
| `topic_coverage` | 8 | same | hardcoded |
| `foreign_language` | 5 | same | hardcoded |

### Forensic evidence the scoring paths were NEVER active
Verified from actual `_meta` fields in production files:

1. **`shakespeare_sft.jsonl` (994 entries)**: 100% have `work: "General"` and
   `work_score: 0.0`. Script `shakespeare_build_sft_dataset.py:211-218`
   defaults to these when `--skip-constellation-scoring` or scoring raises.
   The uniformity strongly indicates scoring was disabled or silently failed.

2. **`shakespeare_sft_enhanced.jsonl` (5,348 entries)**: same pattern, all
   `work: "General"`. Script `shakespeare_enhance_prompts.py` does no
   constellation scoring at all — it only rewrites the *user prompt* via
   Gemma. The assistant text is copied verbatim from the input file.

3. **`shakespeare_sft_final_clean.jsonl` curated_creative entries**: all 279
   have `"verdict": "UNSCORED"`, `"score": 0.5` (sentinel), `"work": "N/A"`.
   This is the explicit path in
   `shakespeare_build_creative_dataset.py:231`: `else: score, verdict, work =
   (0.5, "UNSCORED", "N/A")`. The `--skip-scoring` branch was taken — no
   daemon call, no constellation filtering.

### Where content comes from (assistant messages)
All Shakespeare text in the SFT file was pulled from RocksDB via one of:
- `rpc.call("list_training_records", {...})` — paginated enumeration of ALL
  memories
- `rpc.call("get_training_record", {"memoryId": mid, ...})` — retrieval by UUID

Neither of those APIs applies embedder-weighted ranking. They return records
by insertion order / UUID. The ONLY filters applied are:
- `len(content.strip()) >= 30` (minimum-length sanity)
- `content.strip() != ""`
- `strip_play_formatting(text) != ""` (cleaning step in rebuild script)

## Verifying constellation scoring was purely DECORATIVE where it ran
In `shakespeare_build_optimal_dataset.py` lines 242-254 and
`shakespeare_extract_all_signals.py` line 331-352, the constellation-score
and embedder-score fields are used to **annotate `system_extra` tags**
(e.g. `"Style: {work}. {embedder} signal: {score:.3f}"`) — never to filter
OR rank which memories enter the dataset. Every qualifying memory is
`all_examples.append(...)`. There is no `if score > threshold: keep` gate.

## The ONE place constellation scoring WAS a potential selector
`shakespeare_build_creative_dataset.py:243-244`:
```python
candidates.sort(key=lambda c: -c["score"])
best = candidates[0]
```
This would pick the highest-`combined_score` candidate from N=4 Gemma
generations per prompt. **BUT** the production runs used
`--skip-scoring` (evidence: all 279 curated_creative entries have
`verdict: UNSCORED`, score: 0.5). So in practice the first candidate at
temp=0.7 was kept regardless of constellation score.

Even if it HAD run, the selection was among candidates for the SAME prompt
(e.g. "Write a sonnet about coffee") at different temperatures — not a
semantic filter across the Shakespeare corpus.

## Implication for the Layer-1 LoRA training signal
The LoRA was trained on:
- 7,559 cleaned Shakespeare passages retrieved by UUID (pure corpus coverage)
- 230 Gemma-generated Shakespeare about modern topics (no filter)
- 279 Gemma creative candidates (no filter, picked by temperature order)
- 43 hand-crafted adversarial/topic-coverage/foreign examples (no search)
- 123 random intra-work pairings (no search)
- 2,541 typed-edge-derived continuation examples (edges come from causal
  discovery, NOT from E1 search)

None of these pathways filtered or ranked by E1_Semantic cosine similarity
or any semantic-embedder weight profile.

## Related
- See also: ./memory/discoveries/agent-01-sherlock-sft--constellation-scoring-semantic-but-decorative.md
- See also: ./memory/reports/agent-01-sherlock-sft--final-report.md
