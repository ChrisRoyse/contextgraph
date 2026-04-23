# 19. CAMD-DPO Python Training Subsystem

## Source Files Covered

All under `scripts/v4_2/`.

| File | Role |
|---|---|
| `__init__.py` | Package marker (empty) |
| `camd_lib.py` | Core library — `ConstellationScorer`, `CAMDPairFilter`, `CAMDPair`, `ContrastivePair`, `ConstellationScore`, `STYLE_WEIGHTS`, `EMB`, `SONNETS_CONSTELLATION_ID`, prompt templates, `iter_contrastive_pairs`, `quick_score` |
| `mine_camd_pairs.py` | CLI — mine daemon-only (13-view) DPO pairs from CF_CONTRASTIVE_PAIRS |
| `validate_camd.py` | CLI — 3-part validation: reward sanity, pair discrimination, corpus statistics |
| `extended_ensemble.py` | `ExtendedEmbedderEnsemble` — loads 4 extra embedders (BGE-M3, Jina-v3, Qwen3) + BGE-reranker-v2-m3 |
| `extended_store.py` | `ExtendedStore` — LMDB sidecar for extended embeddings; `ExtendedMeta`, `content_hash` |
| `extended_constellation.py` | CLI — compile extended Sonnets centroids (.npz) from LMDB-cached vectors |
| `extended_camd.py` | `ExtendedConstellationScorer`, `ExtendedScoreResult` — combined 13+5 view reward |
| `backfill_extended.py` | CLI — populate LMDB sidecar from daemon content + FSV |
| `extended_fsv.py` | CLI — 6-check Full State Verification of the extended layer |
| `mine_camd_pairs_extended.py` | CLI — 18-view pair miner (daemon-13 + extended-5) |
| `mine_camd_from_lmdb.py` | CLI — alternative miner: score-and-sort all LMDB records, pair top-K x bottom-K |
| `analyze_correlations.py` | CLI — 13+4 cross-view correlation analysis (redundancy + orthogonality report) |
| `train_camd_dpo.py` | CLI — TRL DPO trainer consuming CAMD JSONL into LoRA adapter |
| `smoke_dpo_v4_2.py` | CLI — end-to-end 20-step DPO smoke test before committing 3-4h runs |
| `README.md` | Quickstart + reward-function explanation |

---

## 1. Purpose

Constellation-Anchored Multi-View DPO (CAMD-DPO) mines DPO training pairs from pre-computed contrastive pairs (CF_CONTRASTIVE_PAIRS) by ranking each side with a deterministic multi-embedder distance to a reference constellation (Sonnets, `18ef2460-9ab6-44fc-8d7e-8d7513174ac8`, 313 members, coherence 0.9036). No judge LLM. No on-policy sampling. The chosen/rejected assignment comes from which side is closer to the Shakespeare anchor across 5 style-discriminating embedder views. Mined pairs become TRL-compatible JSONL consumed by `train_camd_dpo.py` to fine-tune a LoRA adapter on a Gemma base.

Two parallel stacks exist:

- **Base CAMD** (`camd_lib.py` + `mine_camd_pairs.py`) — 13-view reward using daemon-resident embedders only (E1–E13 via `score_against_constellation` MCP tool).
- **Extended CAMD** (`extended_*.py` + `mine_camd_pairs_extended.py` / `mine_camd_from_lmdb.py`) — 13-view daemon reward blended with 5 new views (BGE-M3 dense/sparse/colbert, Jina-v3, Qwen3) cached in a local LMDB sidecar.

Both feed `train_camd_dpo.py`.

---

## 2. Data Flow

```
Context Graph daemon (127.0.0.1:3100)
        |
        |-- CF_CONTRASTIVE_PAIRS (13,465 pre-mined pairs) ------+
        |-- constellations (Sonnets 18ef2460..., coherence 0.9036) +
        |-- score_against_constellation MCP tool                   |
        |-- get_memory_fingerprint / forget_concept / store_memory |
        |                                                          |
        v                                                          v
  camd_lib.ConstellationScorer ------> CAMDPairFilter (base, 13-view)
        |                                     |
        |                                     +--> mine_camd_pairs.py
        |                                             -> v4_2_camd_pairs.jsonl
        |
        +-- extended_store.ExtendedStore (LMDB @ ~/.contextgraph/extended_embeddings.lmdb)
              |                                   ^
              |                                   | populated by backfill_extended.py
              |                                   | (uses ExtendedEmbedderEnsemble)
              |
              +-- extended_constellation.py -> sonnets_constellation_extended.npz
              |
              +-- extended_camd.ExtendedConstellationScorer (13+5 views)
                         |
                         +--> mine_camd_pairs_extended.py
                         +--> mine_camd_from_lmdb.py
                                    -> v4_2_camd_pairs_extended.jsonl
                                    -> v4_2_camd_lmdb.jsonl

  JSONL (any variant) --> train_camd_dpo.py (TRL DPOTrainer + Reg-DPO SFT)
                                    -> LoRA adapter (safetensors)
```

Every script connects to the daemon via `_gemma_shakespeare_lib.Rpc` (JSON-RPC over TCP at 127.0.0.1:3100).

---

## 3. Constants and Canonical Identifiers

Defined in `camd_lib.py` unless noted.

| Constant | Value | Description |
|---|---|---|
| `SONNETS_CONSTELLATION_ID` | `"18ef2460-9ab6-44fc-8d7e-8d7513174ac8"` | Canonical Shakespeare anchor; 313 members, coherence 0.9036 |
| `EMB` | 13-key dict | Embedder-name → 0-indexed slot (E1=0, E2=1, …, E13=12) |
| `STYLE_WEIGHTS` | see §4 | Weighted aggregate over 5 style-discriminating daemon embedders |
| `DEFAULT_PROMPT_TEMPLATES` | 8 strings | Rotated via `pick_prompt_template(idx)` |
| `DEFAULT_ALPHA_DAEMON` | `0.40` | Extended reward blend: `α·daemon_style + (1−α)·extended_style` |
| `EXTENDED_STYLE_WEIGHTS` | see §4 | 5-view aggregate for extended views |
| `QWEN3_PROMPT` | full sentence | Task prompt for Qwen3-Embedding (instruction-tuned) |
| `SONNETS_CONSTELLATION_ID` (extended) | same as daemon | Consistent anchor across layers |
| `DEFAULT_STORE_PATH` (LMDB) | `~/.contextgraph/extended_embeddings.lmdb` | Sidecar path |
| `DEFAULT_CENTROIDS_PATH` | `~/.contextgraph/models/extended_ensemble/sonnets_constellation_extended.npz` | Extended centroids artifact |
| `DEFAULT_MAP_SIZE` (LMDB) | `8 GiB` | LMDB map size, holds ~500k records |
| `SUBDB_NAMES` | 6 names | LMDB sub-DBs: `bge_m3_dense`, `bge_m3_sparse`, `bge_m3_colbert`, `jina_v3_classif`, `qwen3_instruct`, `meta` |
| `EXPECTED_DIMS` (extended) | 1024 for dense/colbert/jina/qwen | Enforced on put/get |
| `COV_RIDGE` | `1e-4` | Covariance regularization multiplier for Mahalanobis |
| `BGE_M3_MODEL_ID` | `"BAAI/bge-m3"` | |
| `JINA_V3_MODEL_ID` | `"jinaai/jina-embeddings-v3"` | |
| `QWEN3_MODEL_ID` | `"Qwen/Qwen3-Embedding-0.6B"` | |
| `BGE_RERANKER_MODEL_ID` | `"BAAI/bge-reranker-v2-m3"` | |

---

## 4. Reward Functions

### 4.1 Daemon Style Weights (base CAMD)

`camd_lib.STYLE_WEIGHTS` — sum to 1.0.

| Embedder | Slot | Weight | Role |
|---|---|---|---|
| E12 ColBERT | 11 | 0.30 | Per-token late-interaction, strongest style signal |
| E13 SPLADE | 12 | 0.25 | Sparse lexical expansion, archaic vocabulary detector |
| E7 code | 6 | 0.15 | Syntax/meter pattern |
| E9 HDC | 8 | 0.15 | Holographic structural robustness |
| E10 paraphrase | 9 | 0.15 | Asymmetric paraphrase |

`style_score = Σ w_i * per_embedder_cosine[slot_i]` over the 5 style embedders. `per_embedder_cosine` comes from the daemon's `score_against_constellation` tool.

These weights mirror `shakespeare_chat_dashboard.py::STYLE_EMBEDDER_WEIGHTS` so training and inference-time guards agree.

### 4.2 Extended Style Weights (extended CAMD)

`extended_camd.EXTENDED_STYLE_WEIGHTS` — sum to 1.0.

| View | Weight | Rationale |
|---|---|---|
| `jina_v3_classif` | 0.30 | Best discrimination gap in smoke tests (+0.1064) |
| `qwen3_instruct` | 0.30 | Instruction-conditioned, +0.0934 smoke gap |
| `bge_m3_dense` | 0.15 | Strong multilingual dense |
| `bge_m3_colbert` | 0.15 | Mean-pooled to 1024D, cosined against ColBERT centroid |
| `bge_m3_sparse` | 0.10 | Lexical overlap (informational more than stylistic) |

BGE reranker is NOT added to this sum; it is a separate logit signal used for gap-ordering filters.

### 4.3 Combined Extended Reward

```
combined_reward = α · daemon_style_score + (1 − α) · extended_combined_score
                  where α = DEFAULT_ALPHA_DAEMON = 0.40
```

Used by `ExtendedConstellationScorer.score_text` and `score_memory_id`.

### 4.4 Score Gap

`score_gap = chosen_style_score − rejected_style_score`. Selection requires `score_gap >= min_constellation_gap`.

---

## 5. Core Data Classes (`camd_lib.py`)

### 5.1 `ConstellationScore`

| Field | Type | Description |
|---|---|---|
| `text` | `str` | Probed text |
| `constellation_id` | `str` | Anchor UUID (Sonnets by default) |
| `per_embedder` | `list[float]` | 13-dim cosine-to-centroid vector from daemon |
| `combined_score` | `float` | Daemon's mean over covered embedders |
| `in_spread_p95` | `bool` | Whether inside 95th-percentile Mahalanobis spread |
| `fingerprint_id` | `str \| None` | Probe UUID before cleanup; `None` after `flush` |

Methods: `style_score()` (weighted over `STYLE_WEIGHTS`), `semantic_score()` (raw E1).

### 5.2 `ContrastivePair`

Mirrors CF_CONTRASTIVE_PAIRS rows via `list_contrastive_pairs` MCP tool.

| Field | Type |
|---|---|
| `anchor_id`, `negative_id` | `str` (UUIDs) |
| `anchor_text`, `negative_text` | `str` |
| `similarity_profile` | `list[float]` (13-dim) |
| `anomaly_kind` | `str` |
| `high_embedders`, `low_embedders` | `list[int]` |
| `disagreement_magnitude` | `float` |

Methods: `e1_similarity()`, `style_similarity()` (weighted over style slots).

### 5.3 `CAMDPair` (base, 13-view)

Holds prompt/chosen/rejected plus both `ConstellationScore`s and the source `ContrastivePair`. `to_dpo_jsonl()` returns the TRL-compatible dict.

### 5.4 `CAMDCriteria`

| Field | Default | Purpose |
|---|---|---|
| `min_e1_similarity` | `0.70` | Reject pairs whose semantic content differs |
| `max_style_similarity` | `0.60` | Reject pairs whose style is already the same |
| `min_constellation_gap` | `0.15` | Minimum `style_score(chosen) − style_score(rejected)` |
| `min_chosen_style_score` | `0.50` | Chosen must actually be Shakespeare-like |
| `max_rejected_style_score` | `0.40` | Rejected must not be Shakespeare-like |
| `min_text_len` | `40` | Drop trivially-short memories |
| `max_text_len` | `3000` | Drop huge memories (token cost) |

---

## 6. `ConstellationScorer` Lifecycle

Flow in `camd_lib.ConstellationScorer.score(text, cleanup=True)`:

1. `store_memory` MCP call — content, sessionId `"camd-scorer"`, importance 0.3. Returns `fingerprintId`.
2. `score_against_constellation` MCP call — `{constellationId, memoryId=fingerprintId}`, timeout 30s.
3. If `cleanup=True`: `forget_concept` MCP call; else append fingerprint to `_probes_to_cleanup` for later `flush()`.
4. Return `ConstellationScore`.

Errors during scoring still trigger cleanup (best-effort). `forget_concept` failures are swallowed.

Thread-unsafe: one scorer per worker.

---

## 7. `CAMDPairFilter` — 5-Stage Pipeline

`CAMDPairFilter.__call__(pair, prompt_template)` applies:

| Stage | Check | Reject Counter |
|---|---|---|
| 1. Shape | Both texts within `[min_text_len, max_text_len]` | `rejected_shape` |
| 2a. E1 similarity | `pair.e1_similarity() >= min_e1_similarity` | `rejected_e1_too_low` |
| 2b. Style similarity | `pair.style_similarity() <= max_style_similarity` | `rejected_style_too_similar` |
| 3. Scoring | Score both `anchor_text` + `negative_text` via `ConstellationScorer.score` (2 daemon round-trips per pair) | n/a |
| 4. Assign | Side with higher `style_score` → `chosen`; other → `rejected` | n/a |
| 5a. Chosen floor | `chosen.style_score >= min_chosen_style_score` | `rejected_chosen_below_threshold` |
| 5b. Rejected ceiling | `rejected.style_score <= max_rejected_style_score` | `rejected_rejected_above_threshold` |
| 5c. Gap floor | `chosen − rejected >= min_constellation_gap` | `rejected_gap_too_small` |

All counters are tracked in `self.stats`; `summary()` prints the full breakdown.

---

## 8. CLI Reference

### 8.1 `mine_camd_pairs.py` (base 13-view)

| Arg | Default | Description |
|---|---|---|
| `--output` | `~/.contextgraph/training/v4_2_camd_pairs.jsonl` | JSONL output path |
| `--target` | `5000` | Stop after N accepted pairs |
| `--max-input` | `13500` | Max pairs streamed from CF_CONTRASTIVE_PAIRS |
| `--page-size` | `200` | `list_contrastive_pairs` pagination |
| `--min-e1` | `0.70` | Forwarded to `CAMDCriteria` |
| `--max-style-sim` | `0.60` | idem |
| `--min-gap` | `0.15` | idem |
| `--min-chosen` | `0.50` | idem |
| `--max-rejected` | `0.40` | idem |
| `--min-text-len` / `--max-text-len` | `40` / `3000` | idem |
| `--kind-filter` | None | Optional AnomalyKind filter |
| `--heartbeat-every` | `25` | Progress log cadence |
| `--fail-budget` | `0.05` | Abort if > this fraction of scorings fail |
| `--resume` | off | Resume from sidecar `.state.json` |

Output atomicity: writes `<output>.partial` then renames. State checkpointed to `<output>.state.json` every heartbeat (offset, accepted, processed, scoring_failures, elapsed_sec, optional `complete`).

Exit: 0 on any accepted pairs, 2 on zero.

Throughput: ~2–3 seconds per candidate via RPC round-trip. 5000-pair production run: 5–6 hours on a warm daemon; worst case 15–25 hours at tighter thresholds (single-threaded).

### 8.2 Output JSONL Schema

One JSON object per line, TRL DPO-compatible:

```json
{
  "prompt": "Compose a passage in the voice of William Shakespeare.",
  "chosen":  "<higher style_score side>",
  "rejected": "<lower style_score side>",
  "meta": {
    "chosen_id": "...", "rejected_id": "...",
    "chosen_style_score": 0.62, "rejected_style_score": 0.38,
    "chosen_combined_score": 0.55, "rejected_combined_score": 0.34,
    "score_gap": 0.24,
    "chosen_per_embedder": [13 floats], "rejected_per_embedder": [13 floats],
    "anomaly_kind": "STYLE_INVERSION",
    "e1_similarity": 0.91, "style_similarity": 0.42,
    "disagreement_magnitude": 0.37,
    "constellation_id": "18ef2460-9ab6-44fc-8d7e-8d7513174ac8",
    "mined_by": "camd_lib.CAMDPairFilter"
  }
}
```

`prompt` is drawn from `DEFAULT_PROMPT_TEMPLATES` using `pick_prompt_template(idx)`; templates may interpolate `{first_line}` (up to 140 chars) or `{first_words}` (12 words).

### 8.3 `validate_camd.py`

Three sequential checks:

| Test | Source Data | Pass Condition |
|---|---|---|
| 1. Reward sanity | 6 curated texts (3 Shakespeare, 3 modern) in `REWARD_TESTS`; scored live via `quick_score` | Every SHAKE >= 0.45 style; every MODERN <= 0.40 style; `min_shake - max_modern >= 0.10` |
| 2. Pair discrimination | First `--sample` (default 50) pairs from JSONL | `chosen.style - rejected.style >= 0.10` in >= 95% of pairs |
| 3. Corpus statistics | Full JSONL | Informational; prints distribution of chosen/rejected/gap/E1/length/anomaly_kinds |

Args: `--pairs` (default `~/.contextgraph/training/v4_2_camd_pairs.jsonl`), `--sample 50`, `--skip-reward-test` (skips daemon-dependent test).

Exit 0 if all gates pass, 1 on any failure.

### 8.4 `extended_constellation.py`

Compiles `sonnets_constellation_extended.npz` from the LMDB sidecar.

| Arg | Default |
|---|---|
| `--store-path` | `~/.contextgraph/extended_embeddings.lmdb` |
| `--out` | `~/.contextgraph/models/extended_ensemble/sonnets_constellation_extended.npz` |
| `--constellation-id` | `SONNETS_CONSTELLATION_ID` |
| `--sparse-min-coverage` | `0.1` (token must appear in >= 10% of members) |

NPZ output:

| Key | Shape | Description |
|---|---|---|
| `bge_m3_dense_centroid` | `f32[1024]` | Mean over dense vectors |
| `jina_v3_centroid` | `f32[1024]` | |
| `qwen3_centroid` | `f32[1024]` | |
| `bge_m3_colbert_centroid` | `f32[1024]` | Mean of per-member mean-pooled ColBERT |
| `bge_m3_dense_cov_inv` | `f32[1024,1024]` | Ridge-regularized inverse covariance (Mahalanobis) |
| `jina_v3_cov_inv`, `qwen3_cov_inv` | same | |
| `bge_m3_sparse_centroid_ids` | `i32[k]` | Token IDs meeting coverage |
| `bge_m3_sparse_centroid_weights` | `f32[k]` | Mean weight per token |
| `bge_m3_dense_std_l2`, `jina_v3_std_l2`, `qwen3_std_l2` | scalar `f32` | Per-member L2-norm std |
| `n_members` | `i32` scalar | Record count used |
| `member_ids` | `object[]` | UUIDs (pickled) |
| `constellation_id` | `str` | |
| `computed_at` | `f64` | Unix time |

Cov regularization: `Σ + (1e-4 · trace(Σ)/d) · I`. Sanity: `mahal(μ, μ) ≈ 0` is asserted after inversion.

### 8.5 `backfill_extended.py`

Populates the LMDB sidecar for a target set via the ensemble.

| Arg | Default | Description |
|---|---|---|
| `--target` | required | `sonnets` / `shakespeare` / `training` / `contrastive_pair_members` / `explicit` |
| `--memory-ids` | None | Used only with `--target=explicit` |
| `--store-path` | `DEFAULT_PATH` | LMDB |
| `--qwen3-prompt` | canonical classify prompt | Instruction for Qwen3 |
| `--reranker-query` | `"Shakespearean verse about summer"` | Query used for reranker score |
| `--force` | off | Re-compute existing records |
| `--limit` | None | Cap processed IDs (smoke testing) |
| `--skip-fsv` | off | Skip final FSV re-compute pass |
| `--fsv-sample-k` | 5 | Random sample for FSV |
| `--dtype` | `bfloat16` | `bfloat16` / `float16` / `float32` |

Per memory: `get_memory_fingerprint` → `ExtendedEmbedderEnsemble.embed_bge_m3` + `embed_jina_v3(task="classification")` + `embed_qwen3(task_prompt=...)` + `rerank(query, [content])` → `ExtendedMeta` build → `ExtendedStore.put` (atomic over 6 sub-DBs).

`EmptyContentError` (soft-deleted / stub memories) counts as `skipped_empty`, not error. Hard errors abort if > 3 and > 5% of processed count. Exit 0 on success, 2 on errors.

### 8.6 `extended_fsv.py`

Full State Verification — 6 checks against the LMDB + ensemble.

| Check | Summary |
|---|---|
| 1. DISK | `data.mdb` exists; all 6 sub-DBs have equal, non-zero counts |
| 2. SCHEMA | Random 10-sample: dense shape = `(1024,)` f32, non-pathological norm, colbert `(seq_len>0, 1024)` f32, sparse non-empty, `meta.content_sha256` 64 hex |
| 3. DETERMINISM | Random 5-sample: re-compute all 3 dense views; `cos(stored, recomputed) >= 0.9999` |
| 4. DISCRIMINATION | Held-out Shakespeare vs modern + anchor; gap >= 0.05 on each dense view; reranker logit gap > 0 |
| 5. EDGES | (a) empty text → raises; (b) missing memory_id → KeyError; (c) wrong dim in `_pack_dense` → ValueError; (d) empty Qwen3 prompt → RuntimeError |
| 6. PHYSICAL EVIDENCE | Dumps first-5 floats of 3 records + meta snapshot to log |

Args: `--store-path`, `--out` (JSON report), `--skip-ensemble` (runs only checks 1/2/6, no GPU). Exit 0 on all pass; 1 on any `FSVFailure`.

### 8.7 `mine_camd_pairs_extended.py` (18-view)

Analogous to `mine_camd_pairs.py` but uses `ExtendedConstellationScorer.score_memory_id` for both sides. Requires both anchor + negative present in LMDB (otherwise `rejected_lmdb_missing`).

| Arg | Default |
|---|---|
| `--output` | `~/.contextgraph/training/v4_2_camd_pairs_extended.jsonl` |
| `--target` / `--max-input` / `--page-size` | `5000` / `13500` / `200` |
| `--store-path` / `--centroids-path` | LMDB + NPZ defaults |
| `--alpha-daemon` | `0.40` |
| `--min-e1` | `0.70` |
| `--max-style-sim` | `0.75` (looser than base CAMD's 0.60) |
| `--min-gap` | `0.10` on combined reward |
| `--min-chosen` | `0.50` combined |
| `--max-rejected` | `0.50` combined |
| `--min-gap-reranker` | `0.0` (non-negative gap) |

Output schema extends §8.2 with `chosen_daemon_style`, `rejected_daemon_style`, `chosen_extended_combined`, `rejected_extended_combined`, `chosen_extended_per_view` (5-key dict), `rejected_extended_per_view`, `chosen_reranker_score`, `rejected_reranker_score`, `gap_daemon_style`, `gap_extended_style`, `gap_combined_reward`, `gap_reranker`, `alpha_daemon`, `from_cache`, `source: "mine_camd_pairs_extended"`.

Criteria class: `ExtendedCAMDCriteria`. Filter class: `ExtendedCAMDPairFilter` — 9 reject counters.

### 8.8 `mine_camd_from_lmdb.py` (top-K × bottom-K)

Alternative mining strategy that bypasses CF_CONTRASTIVE_PAIRS. For every memory in the LMDB sidecar: score against extended Sonnets centroid; sort descending; pair top-K × bottom-K.

| Arg | Default |
|---|---|
| `--top-k` / `--bottom-k` | `200` / `200` |
| `--min-gap` | `0.15` (on extended combined) |
| `--max-pairs` | `5000` |
| `--include-daemon-scores` | off (slow, 1 RPC per record) |
| `--constellation-id` | Sonnets |
| `--alpha-daemon` | `0.40` |

When `--include-daemon-scores`, each pair also receives `chosen_daemon_per_embedder`, `rejected_daemon_per_embedder`, `chosen_daemon_style`, `rejected_daemon_style`, `gap_daemon_style`, `chosen_combined_reward`, `rejected_combined_reward`, `gap_combined_reward`. Every pair gets `source: "mine_camd_from_lmdb"`, `top_k_rank_chosen`, `bottom_k_rank_rejected`.

### 8.9 `analyze_correlations.py`

| Arg | Default |
|---|---|
| `--store-path` | LMDB default |
| `--centroids-path` | NPZ default |
| `--out-dir` | `~/.contextgraph/models/extended_ensemble/` |

Builds an `n × 17` score matrix (13 daemon views + 4 extended — BGE-M3 sparse excluded, E11 noted as often zero-variance). Outputs `correlation_findings.json` with:

- `correlation_matrix` — `list[list[float|None]]`, None where zero-variance.
- `zero_variance_axes` — names.
- `redundant_pairs` — sorted by descending `|r|`, filtered `|r| > 0.9`.
- `orthogonal_daemon_vs_extended` — daemon×extended pairs with `|r| < 0.3`.
- `per_axis_stats_on_sonnets` — per-axis mean/std/min/max.

Also prints a text summary (daemon×extended cross-block matrix, redundant top-10, orthogonal top-10, per-axis stats).

### 8.10 `train_camd_dpo.py`

TRL DPO trainer over a CAMD JSONL dataset with Reg-DPO SFT regularization.

| Arg | Default | Description |
|---|---|---|
| `--base` | `google/gemma-4-E4B-it` | HF base model |
| `--adapter-in` | `~/.contextgraph/models/shakespeare_lora/final` | v2.1 starting LoRA (NOT v4.1) |
| `--pairs` | `~/.contextgraph/training/v4_2_camd_pairs.jsonl` | Dataset path |
| `--output-dir` | `~/.contextgraph/models/shakespeare_lora_v4_2/dpo` | Output |
| `--epochs` | `1` | Full passes |
| `--batch-size` | `4` | Per-device; DPO loads 2× for chosen+rejected |
| `--grad-accum` | `4` | Effective batch = batch × grad-accum = 16 |
| `--lr` | `5e-6` | Half of typical SFT LR (Cerebras DPO guidance) |
| `--beta` | `0.1` | DPO KL regularization strength |
| `--lambda-sft` | `0.1` | Reg-DPO SFT loss weight (ArXiv 2511.01450) |
| `--max-length` | `1024` | Total tokens |
| `--max-prompt-length` | `128` | |
| `--warmup-ratio` | `0.05` | Cosine warmup fraction |
| `--gradient-checkpointing` | on | Required for rank-256 LoRA on 32GB |
| `--seed` | `42` | |
| `--optim` | `paged_adamw_8bit` | Fallback `adamw_torch` on Blackwell driver issues |
| `--precompute-ref` | off | Precompute ref log-probs (memory win, sidesteps some driver bugs) |
| `--max-steps` | `-1` | Override: stop after N steps (smoke test) |
| `--limit-pairs` | None | Subsample dataset |
| `--dry-run` | off | Validate pairs, exit before training |

Base model loaded with BitsAndBytes 4-bit nf4 (`bnb_4bit_quant_type="nf4"`, `bnb_4bit_compute_dtype=torch.bfloat16`, `bnb_4bit_use_double_quant=True`). `AutoTokenizer.from_pretrained(adapter_in)` then `PeftModel.from_pretrained(base, adapter_in, is_trainable=True)`. `pad_token` falls back to `eos_token` when absent. Attention implementation: SDPA.

TRL 1.1.x config: `loss_type=["sigmoid", "sft"]` + `loss_weights=[1.0, lambda_sft]` (replaces older `rpo_alpha`). Trainer uses `processing_class=tok` (TRL renamed `tokenizer`). `ref_model=None` — PeftModel uses base weights as implicit reference. `save_strategy="epoch"`, `save_total_limit=2`, `logging_steps=5`, `report_to="none"`, `remove_unused_columns=False`.

Adapter saved to `<output-dir>/final`; `chat_template.jinja` copied from `adapter_in` when present. Tokenizer saved alongside.

`validate_pairs` pre-flight confirms `prompt`, `chosen`, `rejected` keys non-empty in every JSONL row.

### 8.11 `smoke_dpo_v4_2.py`

End-to-end smoke of `train_camd_dpo.py` (~5–8 min on RTX 5090). 4 phases:

| Phase | Check |
|---|---|
| 1. Training | Spawns `train_camd_dpo.py` with `--limit-pairs 50`, `--max-steps 20`, `--batch-size 2`, `--grad-accum 2`, `--max-length 512`. Subprocess exit 0 required. |
| 2. Adapter saved | `<output>/final/adapter_model.safetensors` present and > 1 MB; `adapter_config.json` present |
| 3. Reload + generate | Subprocess reloads adapter via PeftModel, generates 80 tokens from Shakespeare system+user prompt (temperature 0.8, top_p 0.92, repetition_penalty 1.2). Generation > 20 chars. No 4-gram repeats > 3 times over >= 8 words. |
| 4. Loss trend | `trainer_state.json` in last `checkpoint-*`: first vs last loss must differ (abs >= 1e-6). No strict monotonic decrease required. |

Defaults: `--pairs ~/.contextgraph/training/v4_2_camd_lmdb.jsonl`, `--output-dir ~/.contextgraph/models/shakespeare_lora_v4_2_smoke/dpo`, `--adapter-in ~/.contextgraph/models/shakespeare_lora/final`, `--base google/gemma-4-E4B-it`. Cleans `output-dir` before running.

---

## 9. `ExtendedEmbedderEnsemble` (`extended_ensemble.py`)

Loads 4 embedders + 1 reranker on a single CUDA device. Fail-fast on every load/encode error via `_die()` helper.

### 9.1 Models

| Name | HF ID | Framework | Dim | Ctx |
|---|---|---|---|---|
| BGE-M3 (triple-view: dense + sparse + colbert) | `BAAI/bge-m3` | `FlagEmbedding.BGEM3FlagModel` | 1024 dense + 1024 colbert | 8192 |
| Jina-v3 (task LoRA: classification/text-matching/retrieval.{query,passage}/separation) | `jinaai/jina-embeddings-v3` | `sentence-transformers` | 1024 | 8194 |
| Qwen3-Embedding-0.6B (instruction-tuned) | `Qwen/Qwen3-Embedding-0.6B` | `sentence-transformers` | 1024 | 32768 |
| BGE-reranker-v2-m3 (cross-encoder) | `BAAI/bge-reranker-v2-m3` | `transformers.AutoModelForSequenceClassification` | 1 (logit) | 8192 |

### 9.2 Compatibility Patches

- `transformers` 5.x: `transformers.utils.import_utils.is_torch_fx_available` monkeypatch if absent (top-of-module shim).
- Jina-v3 on transformers 5.x: `_patch_jina_v3_for_transformers5()` patches `XLMRobertaLoRA.__init__` to call `post_init()` so `all_tied_weights_keys` is populated.

### 9.3 Encoders

| Method | Input | Output |
|---|---|---|
| `embed_bge_m3(text)` | non-empty `str` | `BGEM3Output(dense: f32[1024], sparse: dict[int,float], colbert: f32[seq_len, 1024])` |
| `embed_jina_v3(text, task=…)` | non-empty `str` + task in `{classification, text-matching, retrieval.query, retrieval.passage, separation}` | `f32[1024]` (normalized) |
| `embed_qwen3(text, task_prompt)` | non-empty `str` + non-empty prompt | `f32[1024]` (prompt = `"Instruct: {task_prompt.strip()}\nQuery: "`, normalized) |
| `rerank(query, passages)` | non-empty query + non-empty list of non-empty passages | `list[float]` logits |
| `describe()` | — | Per-model: params_million, dim, context_length, framework, xor-hash `weights_hash` |

Every forward wraps exceptions into `_die(model_id, phase, exc)` which logs `cuda_free=… MiB` and raises a `RuntimeError` with full diagnostics.

`_assert_dim` asserts embedder output dimension matches `EXPECTED_DIMS`; refuses to proceed if HF ships a retrained variant.

### 9.4 Smoke Test (`python3 extended_ensemble.py`)

Encodes Sonnet 18, Sonnet 29 anchor, and a modern weather passage; requires `gap = sim(anchor, Shakespeare) - sim(anchor, modern) >= 0.05` on each of `bge_m3_dense`, `bge_m3_colbert` (MaxSim variant), `jina_v3_classification`, `qwen3_instruct`. Reranker gap must be > 0. `bge_m3_sparse` is reported INFO-only (lexical, not stylistic). VRAM budget: total usage <= 15 GB; prints per-model params / dim / context / weights_hash.

---

## 10. `ExtendedStore` (`extended_store.py`)

LMDB-backed sidecar. Single-writer / multi-reader, memory-mapped, ACID. One env with 6 named sub-DBs under `~/.contextgraph/extended_embeddings.lmdb/`.

### 10.1 Layout

| Sub-DB | Key | Value |
|---|---|---|
| `bge_m3_dense` | `memory_id.encode()` | `f32[1024].tobytes()` = 4096 B |
| `bge_m3_sparse` | same | msgpack pairs `[(token_id, weight), ...]`, threshold `abs(w) > 1e-6` |
| `bge_m3_colbert` | same | `<I`-packed `seq_len` (4 B) + `f32[seq_len, 1024].tobytes()` |
| `jina_v3_classif` | same | `f32[1024].tobytes()` |
| `qwen3_instruct` | same | `f32[1024].tobytes()` |
| `meta` | same | `msgpack(asdict(ExtendedMeta))` |

### 10.2 `ExtendedMeta`

Frozen dataclass.

| Field | Type | Purpose |
|---|---|---|
| `memory_id` | `str` | Keyed UUID (from daemon CF_FINGERPRINTS) |
| `content_sha256` | `str` (64 hex) | `sha256(content[:8192])` — drift detector |
| `content_length` | `int` | `len(content)` |
| `computed_at` | `float` | Unix time |
| `bge_m3_version`, `jina_v3_version`, `qwen3_version`, `reranker_version` | `str` | `"<model_id>@<weights_hash>"` |
| `reranker_score_vs_sonnets` | `float` | Logit vs `"Shakespearean verse about summer"` query |
| `task_prompt_qwen3` | `str` | Instruction used for Qwen3 embedding |

### 10.3 API

Methods all use encoded `memory_id.encode()` keys and raise `KeyError` when missing.

| Method | Description |
|---|---|
| `put(memory_id, *, bge_dense, bge_sparse, bge_colbert, jina_classif, qwen3_instruct, meta)` | Atomic write across 6 sub-DBs. Validates dtype/dim; ValueError on mismatch. Refuses when `readonly=True`. |
| `get_dense(memory_id, *, embedder)` | Returns `f32[1024]`; embedder ∈ `EXPECTED_DIMS` keys |
| `get_sparse(memory_id)` | Returns `dict[int,float]` |
| `get_colbert(memory_id)` | Returns `f32[seq_len, 1024]` |
| `get_meta(memory_id)` | Returns `ExtendedMeta` |
| `exists(memory_id)` | `True` if meta present |
| `iter_memory_ids()` | Yields decoded memory_ids that have a meta row |
| `count()` | `dict[str,int]` per sub-DB |
| `stats()` | `{path, map_size, last_pgno, psize, counts_per_subdb, total_records}` |

Readonly API gate: `readonly=True` refuses `put()`. LMDB env itself is opened writable (required for sub-DB registration); API-level gate is the safety.

Pack/unpack functions:

- `_pack_dense(arr, expected_dim)`: requires 1D, float32, exact dim.
- `_pack_colbert(arr)`: 2D, dim1=1024, float32; prefixed with `<I` `seq_len`.
- `_pack_sparse(dict)`: msgpack of non-negligible pairs (threshold `1e-6`).

---

## 11. `ExtendedConstellationScorer` (`extended_camd.py`)

Combined daemon + extended reward for one memory (by id) or fresh text.

### 11.1 Construction

```
ExtendedConstellationScorer(
    rpc, store_path=DEFAULT_STORE_PATH,
    centroids_path=DEFAULT_CENTROIDS_PATH,
    ensemble=None,                       # lazy; loaded on first fresh score
    constellation_id=SONNETS_CONSTELLATION_ID,
    alpha_daemon=0.40,
    qwen3_prompt=QWEN3_PROMPT,
    device="cuda", dtype=torch.bfloat16,
)
```

Opens the LMDB store readonly; loads centroids from NPZ. Ensemble is lazy — only instantiated when `score_text` is called or when `score_memory_id` misses the cache.

### 11.2 `score_text(text, include_reranker=True)`

1. Load ensemble if needed.
2. Compute `bge = embed_bge_m3(text)`, `jina = embed_jina_v3(text, "classification")`, `qwen = embed_qwen3(text, task_prompt=QWEN3_PROMPT)`.
3. Optionally `rerank("Shakespearean verse about summer", [text])` — a single logit.
4. Compute extended cosines via `_compute_extended_cosines` (cosines vs centroids; ColBERT mean-pooled vs centroid; sparse via dot/norm).
5. `_daemon_scores(text=...)` — stores a probe, calls `score_against_constellation`, cleans up.
6. `daemon_style = Σ w · daemon_cos[idx]` over `STYLE_WEIGHTS`.
7. `ext_combined = Σ w · ext_cos[k]` over `EXTENDED_STYLE_WEIGHTS`.
8. `reward = α · daemon_style + (1 − α) · ext_combined`.

Returns `ExtendedScoreResult` (text_prefix, content_hash, daemon_view_cosines, daemon_combined_score, daemon_style_score, extended_view_cosines, extended_combined_score, reranker_score, combined_reward, alpha_daemon, memory_id=None, from_cache=False).

### 11.3 `score_memory_id(memory_id)`

Uses LMDB cache when `store.exists(memory_id)`; otherwise recomputes with the ensemble. Daemon scoring is always live (via `score_against_constellation` by memoryId, no probe needed). `from_cache=True` only if extended came from LMDB.

---

## 12. Daemon Dependencies

All CLIs require the Context Graph daemon running on `127.0.0.1:3100`:

```
CONTEXT_GRAPH_STORAGE_PATH=/home/cabdru/contextgraph/contextgraph_data \
  ./target/release/context-graph-mcp --daemon-server-only --daemon-port 3100
```

MCP tools consumed (see [08_mcp_tools_reference.md](08_mcp_tools_reference.md)):

| Tool | Consumers | Purpose |
|---|---|---|
| `list_contrastive_pairs` | base miner, extended miner, `backfill_extended` | Stream CF_CONTRASTIVE_PAIRS |
| `store_memory` | `ConstellationScorer`, `ExtendedConstellationScorer` (text path) | Create transient probe memory |
| `score_against_constellation` | all scorers | 13-view per-embedder cosines vs constellation |
| `forget_concept` | scorers | Soft-delete probe memories |
| `get_memory_fingerprint` | `backfill_extended`, `mine_camd_from_lmdb`, `analyze_correlations`, `extended_fsv` (check 3) | Fetch content by memory_id |
| `get_constellation` | `backfill_extended` (sonnets / shakespeare targets) | Resolve member_ids |
| `list_constellations` | `backfill_extended` (shakespeare target) | Find all Shakespeare constellations |
| `list_training_records` | `backfill_extended` (training target) | CF_TRAINING_RECORDS member ids |

`list_contrastive_pairs` requires `includeFull=True` to return `anchor_text` / `negative_text`.

Prerequisites assumed:

- CF_CONTRASTIVE_PAIRS populated (13,465 pairs pre-mined as of 2026-04-21); otherwise run `mine_contrastive_pairs` via the daemon first.
- Sonnets constellation `18ef2460-9ab6-44fc-8d7e-8d7513174ac8` exists (313 members, coherence 0.9036).
- For extended pipeline: LMDB sidecar backfilled via `backfill_extended.py --target sonnets` (or broader set), and centroids compiled via `extended_constellation.py`.

---

## 13. Extended vs Base CAMD — Diff

| Dimension | Base CAMD | Extended CAMD |
|---|---|---|
| Views scored | 13 daemon embedders | 13 daemon + 5 extended (BGE-M3 dense/sparse/colbert, Jina-v3, Qwen3) |
| Reward formula | `Σ STYLE_WEIGHTS · daemon_cos` | `α · daemon_style + (1 − α) · Σ EXTENDED_STYLE_WEIGHTS · ext_cos`, α=0.40 |
| Extra signal | — | BGE-reranker logit for order check |
| Sonnets anchor | Daemon-side constellation (centroids in RocksDB) | Daemon + NPZ file with covariance-aware centroids |
| Storage | None beyond daemon | LMDB sidecar (~8 GB map) for 4 views + meta |
| Filter class | `CAMDCriteria` / `CAMDPairFilter` | `ExtendedCAMDCriteria` / `ExtendedCAMDPairFilter` |
| Reject stages | 7 counters | 9 counters (adds `rejected_lmdb_missing`, `rejected_reranker_wrong_order`) |
| Throughput | ~2–3s/pair (2 daemon RPCs + 2 probes) | ~0.5s/pair when both sides cached; ensemble inference otherwise |
| Cost to add candidate | 2 store + 2 score + 2 forget RPCs | 2 daemon score RPCs + LMDB lookups (no probe store for memory_id scoring) |

Base CAMD is self-contained in the daemon; extended CAMD trades a large initial backfill (10 min Sonnets / 2 k records Shakespeare / 2,770 training) for substantially stronger style discrimination in smoke tests (Jina-v3 classification gap +0.1064, Qwen3 +0.0934, reranker logit gap ~5.95 vs +0.1 for dense embedders) and orthogonal new signal (see `analyze_correlations.py`).

---

## 14. Output Locations

| Artifact | Path |
|---|---|
| Base CAMD pairs | `~/.contextgraph/training/v4_2_camd_pairs.jsonl` |
| Extended CAMD pairs (contrastive-pair source) | `~/.contextgraph/training/v4_2_camd_pairs_extended.jsonl` |
| LMDB top×bottom pairs | `~/.contextgraph/training/v4_2_camd_lmdb.jsonl` |
| Sidecar state | `<output>.state.json` (mining) |
| Partial writes | `<output>.partial` (renamed atomically on success) |
| LMDB sidecar | `~/.contextgraph/extended_embeddings.lmdb/` |
| Extended centroids | `~/.contextgraph/models/extended_ensemble/sonnets_constellation_extended.npz` |
| Correlation report | `~/.contextgraph/models/extended_ensemble/correlation_findings.json` |
| FSV report | `~/.contextgraph/models/extended_ensemble/fsv_report.json` |
| DPO starting adapter (v2.1) | `~/.contextgraph/models/shakespeare_lora/final` |
| DPO output adapter | `~/.contextgraph/models/shakespeare_lora_v4_2/dpo/final` |
| DPO smoke output | `~/.contextgraph/models/shakespeare_lora_v4_2_smoke/dpo` |

---

## 15. Failure Modes and Common Errors

| Error | Source | Cause / Fix |
|---|---|---|
| `fail_budget exceeded: X/Y scoring calls failed` | `mine_camd_pairs.py` | Daemon unhealthy or slow; inspect last error, restart daemon |
| `store_memory returned no fingerprintId` | `ConstellationScorer._store_probe` | Daemon rejected probe text (likely empty / policy) |
| `daemon returned N views, expected 13` | `_daemon_scores`, `daemon_score_vs_sonnets` | Daemon version skew — extended layer requires all 13 embedders enabled |
| `extended centroids not found: ...` | `ExtendedConstellationScorer` | Run `extended_constellation.py` after backfill |
| `FileNotFoundError: pairs not found` | `validate_pairs`, `smoke_dpo_v4_2` prereq | Miner didn't produce output; check filters and input pool |
| `FSVFailure: FSV1: data.mdb missing` | `extended_fsv.check_1_disk` | LMDB path not yet populated — run `backfill_extended.py` |
| `FSVFailure: FSV1: inconsistent counts across sub-dbs` | `check_1_disk` | Atomic write broken — likely a partial crash mid-`put`. Re-run backfill with `--force`. |
| `FSVFailure: FSV3: cos(stored, recomputed) < 0.9999` | `check_3_determinism` | Embedder weights changed between backfill and FSV. Re-backfill. |
| `dim mismatch for bge_m3_dense: expected 1024, got X` | `_assert_dim` via ensemble load | HF model version shift — refuses to proceed |
| `extended_ensemble: Jina-v3 compat patch failed` | `_patch_jina_v3_for_transformers5` | `XLMRobertaLoRA` not found in `sys.modules`; usually transformers version regressed before 5.x |
| `EmptyContentError` | `backfill_extended.fetch_content` | Legitimate — soft-deleted / stub memory, counted as `skipped_empty` not error |
| `backfill abort: X errors in first Y records exceeds 5% tolerance` | `backfill_extended.main` | Systemic failure; investigate first stack trace |
| `adapter_model.safetensors unexpectedly small` | `smoke_dpo_v4_2.verify_adapter_saved` | Training stopped before writing full adapter; inspect stdout tail in phase 1 |
| `generation has a 4-gram repeated >3 times` | `smoke_dpo_v4_2.verify_reload_and_generate` | Degenerate generation — repetition_penalty insufficient or training destabilized; do not proceed to full run |
| `training loss frozen: first=X, last=X` | `smoke_dpo_v4_2.check_training_loss_trend` | Optimizer not stepping — likely BitsAndBytes/driver issue; try `--optim adamw_torch` and `--precompute-ref` |
| `hard_negative_scale >= 2.0 with temperature=0.05 causes catastrophic collapse` | inherited from E5 training | DPO beta tuning is orthogonal; this is an E5 constraint documented in [16_training_subsystem.md](16_training_subsystem.md) |

---

## 16. Cross-References

| Topic | Document |
|---|---|
| Training data export (TrainingRecord, E5 offline LoRA) | [16_training_subsystem.md](16_training_subsystem.md) |
| Constellation subsystem (Sonnets anchor, centroid/covariance) | [17_constellation_subsystem.md](17_constellation_subsystem.md) |
| Contrastive pairs + typed edges (CF_CONTRASTIVE_PAIRS) | [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md) |
| Embedding subsystem (E1–E13, asymmetric vectors) | [05_embedding_subsystem.md](05_embedding_subsystem.md) |
| MCP tools reference (`score_against_constellation`, `list_contrastive_pairs`, etc.) | [08_mcp_tools_reference.md](08_mcp_tools_reference.md) |
| Storage layer (CF_CONTRASTIVE_PAIRS, constellation storage) | [09_storage_layer.md](09_storage_layer.md) |
