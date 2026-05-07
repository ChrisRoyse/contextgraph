---
namespace: discoveries
agent_id: agent-01-sherlock-sft
timestamp: 2026-04-17T18:10:00Z
status: active
tags: constellation, combined_score, semantic, decorative, not-a-filter
---

# Constellation Scoring Is Semantic, But Was Used Only as Metadata (Not a Filter)

## The Concern From the Brief
"If SFT training was built by filtering/ranking Shakespeare examples via E1
semantic similarity, the entire LoRA could be overfitted to TOPIC rather
than STYLE."

## What Constellation Scoring Actually Is
`score_against_constellation` returns `combined_score`, which is defined in
`crates/context-graph-core/src/constellation/types.rs:242-244` as:
> "Unweighted mean of `per_embedder_cosine` over entries where coverage > 0."

Implementation at `crates/context-graph-core/src/constellation/compiler.rs:374-391`:
```rust
for (i, stats) in constellation.per_embedder.iter().enumerate() {
    if stats.coverage <= 0.0 { continue; }
    let c = score_one(stats, fp);
    per[i] = c;
    summed += c;
    active_count += 1;
}
let combined = if active_count == 0 { 0.0 } else { summed / active_count as f32 };
```

So **`combined_score` averages across ALL 13 embedders including
E1_Semantic and E5_Causal** — the very semantic embedders the guard was
rewritten to exclude. This scoring IS topic-tilted: E1 centroid of the
Shakespeare corpus captures "love/death/kings/betrayal" topic centroid, and
modern text about neural networks will always score lower on E1 even if
stylistically identical.

## BUT — It Was Never a Filter
The suspect callers in SFT-building scripts:

### 1. `shakespeare_build_sft_dataset.py:212-218`
```python
work_name = "General"
work_score = 0.0
if cons_cache and not args.skip_constellation_scoring:
    try:
        work_name, work_score = find_closest_work(rpc, content, cons_cache)
    except Exception as e:
        print(...)
        errors += 1
...
example = build_sft_example(content, modern_prompt, work_name, work_score, mid)
f.write(json.dumps(example, ...) + "\n")   # WRITTEN UNCONDITIONALLY
written += 1
```
The `work_name` goes into the style tag of the system prompt, and
`work_score` is stored as metadata. There is no `if score >= X: keep`.

Moreover the production file `shakespeare_sft.jsonl` has ALL 994 entries
with `work: "General"`, `work_score: 0.0` — proving scoring was either
skipped with `--skip-constellation-scoring` or silently errored out.

### 2. `shakespeare_extract_all.py:276-305`
```python
content = get_content(rpc, mid)
if not content or len(content.strip()) < 30:
    skipped += 1; continue
work, score = "General", 0.0
if cons_cache:
    work, score = score_memory(rpc, content, cons_cache)
...
all_examples.append(sft_example(prompt, content, work, score, "memory_primary"))
```
Same pattern — score is decorative, not a filter.

### 3. `shakespeare_extract_all_signals.py` — never calls score_against at all
Grep confirmed no `score_against|score_text` usage. It uses `topic_profile`
(13 embedder topic scores already on each memory) and `group_alignments` to
classify *texture*, which becomes a system-prompt tag — not a selector.

### 4. `shakespeare_build_creative_dataset.py:171-183, 243-246`
This is the ONE path where `combined_score` could have acted as a selector:
```python
def score_candidate(rpc, text, cons_cache):
    fp, scores = score_text(rpc, text, cons_cache=cons_cache)
    ...
    return (scores[0].combined_score, short, scores[0].work)
...
candidates.sort(key=lambda c: -c["score"])
best = candidates[0]    # highest-score candidate
```
But the selection was **among N=4 Gemma candidates generated for the SAME
modern prompt at temperatures 0.7/0.8/0.9/1.0** — not a semantic filter over
the corpus. And the production file shows all 279 creative entries were
UNSCORED (`--skip-scoring` path).

## Does Any Script Use `search_graph` or `search_by_embedder` for SFT Training?
grep of `scripts/shakespeare_*.py` against `search_graph|search_by_embedder|customWeights|weightProfile|multi_space|e1_only`:

- **SFT-building scripts** (`shakespeare_build_sft_dataset.py`,
  `shakespeare_build_optimal_dataset.py`,
  `shakespeare_build_creative_dataset.py`,
  `shakespeare_build_full_dataset.py`,
  `shakespeare_build_dpo_dataset.py`,
  `shakespeare_enhance_prompts.py`,
  `shakespeare_extract_all.py`,
  `shakespeare_extract_all_signals.py`,
  `shakespeare_extract_full_signals.py`,
  `shakespeare_clean_training_data.py`,
  `shakespeare_rebuild_all_data.py`): **ZERO matches**.

- Only non-SFT scripts reference `search_graph`: `shakespeare_demo_queries.py`
  (demo), `shakespeare_genre_router.py` (inference-time router),
  `shakespeare_demo_summary.py` (demo). None feed training data.

## Conclusion
Constellation scoring *is* semantic (it averages E1/E5 cosine into
`combined_score`), BUT it was never wired up as a selector in the SFT
pipeline. It is pure metadata. The training data was a coverage-complete
sweep of the Shakespeare RocksDB + Gemma-generated creative additions.

## Related
- See also: ./memory/discoveries/agent-01-sherlock-sft--selection-mechanism-confirmed.md
- See also: ./memory/reports/agent-01-sherlock-sft--final-report.md
