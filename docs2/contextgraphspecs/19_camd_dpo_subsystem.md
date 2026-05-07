# CAMD-DPO Subsystem

**Source files covered:**
- `scripts/v4_2/README.md` (CAMD-DPO design rationale)
- `scripts/v4_2/camd_lib.py` (ConstellationScorer, CAMDPairFilter, data classes)
- `scripts/v4_2/extended_camd.py`, `extended_constellation.py`, `extended_ensemble.py`, `extended_fsv.py`, `extended_store.py` (E14 sidecar via LMDB)
- `scripts/v4_2/mine_camd_pairs.py`, `mine_camd_pairs_extended.py`, `mine_camd_from_lmdb.py` (pair mining CLIs)
- `scripts/v4_2/build_learner_dpo_pairs.py` (learner-conditioned DPO pairs)
- `scripts/v4_2/train_camd_dpo.py` (Qwen3.6-27B-it DPO trainer with TRL)
- `scripts/v4_2/validate_camd.py`, `analyze_correlations.py`, `backfill_extended.py`
- `scripts/v4_2/cuda13_blackwell.py`, `cuda13_wsl_fsv.py`, `unified_realdata_fsv.py`, `bigunlock_fsv_lib.py`, `learner_state_bigunlock_fsv.py`, `smoke_dpo_v4_2.py`
- `scripts/v4_2/shakespeare_*.py` (corpus FSV, empirical cycle, learner training FSV, learning event FSV, phase6 policy benchmark, state-conditioned FSV, embedding benchmark, constellation guard)
- `scripts/v4_2/conceptual_capabilities_fsv.py`, `agent_workflow_learning.py`, `phase6_existing_data_inventory.py`
- `scripts/_gemma_shakespeare_lib.py` (shared Rpc + probe helpers)
- Rust counterparts:
  - `crates/context-graph-benchmark/src/bin/causal_accuracy_bench.rs`, `causal_provenance_e2e_bench.rs`, `causal_e2e_bench.rs`, `causal_realdata_bench.rs`, `causal_provenance_llm_bench.rs`, `causal_prompt_experiment_bench.rs`, `causal_span_fix_bench.rs`, `causal_discovery_mcp_bench.rs`, `causal_embedding_bench.rs`, `causal_perf_bench.rs`, `causal_provenance_bench.rs`
  - `crates/context-graph-cli/src/commands/learning.rs` (`export-events-jsonl` — input source for `build_learner_dpo_pairs.py`)
  - `crates/context-graph-storage/src/teleological/` (CFs `CF_CONTRASTIVE_PAIRS`, `CF_CONSTELLATIONS`, `CF_LEARNING_EVENTS`, `CF_LEARNER_RETRIEVAL_LOG`)

**Cross-links:** [16_training_subsystem.md](16_training_subsystem.md), [17_constellation_subsystem.md](17_constellation_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md), [20_learning_subsystem.md](20_learning_subsystem.md), [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md).

## 1. Scope

The CAMD-DPO subsystem is **out-of-Rust Python tooling** that:

1. **Mines deterministic Direct Preference Optimization (DPO) pairs** from RocksDB-resident contrastive pairs, learning events, retrieval logs, and constellation scores.
2. **Trains a base LLM** (Qwen3.6-27B-it) with a Shakespeare-style LoRA adapter using `trl` DPO with Reg-DPO SFT regularization.

CAMD = **Constellation-Anchored Multi-View DPO** (per `scripts/v4_2/README.md`). Chosen/rejected pairs are scored against a `Sonnets` constellation (E1-E14 multi-view) without a judge LLM. The Sonnets constellation `18ef2460-9ab6-44fc-8d7e-8d7513174ac8` (313 members, coherence 0.9036) is the canonical Shakespeare anchor in the production database.

Phase 6 adds **learner-conditioned DPO** that joins `CF_LEARNING_EVENTS` and `CF_LEARNER_RETRIEVAL_LOG` so the same prompt's two real assistant responses with different learner outcomes become one preference pair (`build_learner_dpo_pairs.py`).

Base CAMD remains 13-view (E1-E13). E14 access is via the `extended_*` sidecar scripts and `bigunlock_fsv_lib.py`; sidecar embeddings live in an LMDB cache at `~/.contextgraph/extended_embeddings.lmdb` and never enter the sacred RocksDB store.

There is **no Rust implementation of the DPO trainer itself** — DPO training runs entirely in Python via TRL. The Rust side provides:
- The constellation scoring tool `score_against_constellation` consumed by the Python `ConstellationScorer`.
- The contrastive pair store `CF_CONTRASTIVE_PAIRS` (~13,465 pairs as of 2026-04-21).
- The `learning export-events-jsonl` CLI command that supplies inputs to `build_learner_dpo_pairs.py`.
- 11+ causal benchmarks that validate fine-tuning impact end-to-end (see §6).

## 2. Why CAMD-DPO Is Novel

Public DPO pair construction uses one of three strategies, all with known failure modes:

| Public approach | Failure mode |
| --- | --- |
| Judge-LLM ranking (e.g., UltraFeedback, PairRM) | Expensive, noisy, biased toward judge's preferences, non-reproducible |
| Heuristic edit-distance | Produces pairs where rejected is a corrupted version of chosen, not a genuinely different record |
| On-policy sampling from the model being trained | Feedback loop reinforces model artifacts; can cause mode collapse |

**CAMD-DPO uses none of these.** It uses the Sonnets constellation — a pre-computed centroid + covariance in 13-embedder space — as an **absolute, deterministic reference anchor**. Pairs are mined from existing contrastive pairs where:

- **E1 (semantic) similarity is HIGH** → chosen and rejected mean the same thing
- **E12 (ColBERT) + E13 (SPLADE) similarity is LOW** → different style
- **Constellation distance gap is LARGE** → one side is substantially closer to Shakespeare than the other

The side with the higher style score becomes `chosen`; the other becomes `rejected`. No LLM judge required.

## 3. Reward Function — Style Weights

Combined weighted cosine-to-constellation-centroid across the 5 style-discriminating embedders (mirror of `shakespeare_chat_dashboard.py::STYLE_EMBEDDER_WEIGHTS`, so what is trained against matches what is enforced at inference time):

| Embedder | Weight | Role |
| --- | --- | --- |
| E7 (code structure / UniXCoder, 1536D) | 0.15 | Syntax pattern (low for prose, discriminative for verse/meter) |
| E9 (HDC, 1024D) | 0.15 | Holographic structural robustness |
| E10 (paraphrase asymmetric, 1024D) | 0.15 | Asymmetric paraphrase |
| E12 (ColBERT per-token, 128D/token) | 0.30 | Per-token late-interaction — strongest style signal |
| E13 (SPLADE sparse, 30K) | 0.25 | Sparse lexical expansion — archaic vocabulary detector |

`STYLE_WEIGHTS` sums to exactly 1.0 (asserted at module load in `camd_lib.py:74`).

The 13-embedder index map (`camd_lib.py:49-63`) follows the daemon's `similarity_profile` ordering from `list_contrastive_pairs`:

| Index | Slot | Model |
| --- | --- | --- |
| 0 | E1 | semantic (e5-large-v2, 1024D dense) |
| 1 | E2 | temporal/recency (sinusoidal PE, 512D) |
| 2 | E3 | temporal/periodic (512D) |
| 3 | E4 | temporal/sequential (512D) |
| 4 | E5 | causal asymmetric (768D) |
| 5 | E6 | sparse keyword (inverted, 30K) |
| 6 | E7 | code structure (UniXCoder, 1536D) |
| 7 | E8 | graph topology (1024D) |
| 8 | E9 | HDC (1024D) |
| 9 | E10 | paraphrase asymmetric (1024D) |
| 10 | E11 | entity/Kepler (768D) — often 0.0 (disabled/unlinked) |
| 11 | E12 | ColBERT per-token (128D/token) |
| 12 | E13 | SPLADE sparse (30K) |

E14 (BgeM3Dense, 1024D) is not in the 13-view CAMD index; it is accessed via the `extended_*` sidecar scripts and `bigunlock_fsv_lib.py` against the LMDB cache.

## 4. CAMD Filter (`mine_camd_pairs.py` → `camd_lib.py::CAMDPairFilter`)

`CAMDCriteria` defaults (admitted only when ALL hold):

| Threshold | Default | Meaning |
| --- | --- | --- |
| `min_e1_similarity` | 0.70 | Semantic content match (chosen ≈ rejected meaning) |
| `max_style_similarity` | 0.60 | Style embedders disagree (different style) |
| `min_constellation_gap` | 0.15 | `style_score(chosen) - style_score(rejected) ≥ 0.15` |
| `min_chosen_style_score` | 0.50 | Chosen must score ≥ 0.50 against Sonnets constellation |
| `max_rejected_style_score` | 0.40 | Rejected must score ≤ 0.40 against Sonnets constellation |
| `min_text_len` | 40 | Length floor for both sides |
| `max_text_len` | 3000 | Length cap (CLI default may be 8000 for higher yield) |
| `--fail-budget` (CLI) | 5 % | Abort run if scoring failures exceed this fraction |

Filter pipeline per pair (`camd_lib.py:348-403`):

1. **Stage 1 — shape:** length floors/caps, non-empty texts.
2. **Stage 2 — cheap profile filter:** `e1_similarity ≥ 0.70` and `style_similarity ≤ 0.60` from the persisted similarity_profile, no daemon scoring yet.
3. **Stage 3 — constellation scoring:** two daemon `score_against_constellation` calls per pair via `ConstellationScorer` (probes are stored as transient memories, scored, then soft-deleted via `forget_concept`).
4. **Stage 4 — assign chosen/rejected:** whichever side has the higher style_score becomes `chosen`.
5. **Stage 5 — quality filters:** `chosen.style_score ≥ 0.50`, `rejected.style_score ≤ 0.40`, `gap ≥ 0.15`.

Atomic write pattern: output written to `<file>.partial` then renamed (`mine_camd_pairs.py:124`); sidecar `<file>.state.json` allows `--resume`.

`CAMDPairFilter.summary()` reports per-stage rejection counts (`rejected_shape`, `rejected_e1_too_low`, `rejected_style_too_similar`, `rejected_chosen_below_threshold`, `rejected_rejected_above_threshold`, `rejected_gap_too_small`).

## 5. Data Classes (`camd_lib.py`)

| Class | Fields | Purpose |
| --- | --- | --- |
| `ConstellationScore` | `text`, `constellation_id`, `per_embedder` (13-dim), `combined_score`, `in_spread_p95`, `fingerprint_id` | One candidate scored against one constellation. `style_score()` = weighted aggregate over `STYLE_WEIGHTS` slots. `semantic_score()` = E1 cosine |
| `ContrastivePair` | `anchor_id`, `anchor_text`, `negative_id`, `negative_text`, `similarity_profile` (13-dim), `anomaly_kind`, `high_embedders`, `low_embedders`, `disagreement_magnitude` | Loaded from daemon `list_contrastive_pairs` (CF_CONTRASTIVE_PAIRS) |
| `CAMDPair` | `prompt`, `chosen`, `rejected`, `chosen_score`, `rejected_score`, `source_pair` | Mined DPO triple. `to_dpo_jsonl()` emits TRL-compatible JSON with full `meta` provenance |
| `ConstellationScorer` | `rpc`, `constellation_id`, `session_tag`, `_probes_to_cleanup` | Wraps daemon `score_against_constellation` with probe lifecycle. Single-threaded; `flush()` cleans deferred probes |
| `CAMDPairFilter` | `scorer`, `criteria`, `stats` | Accept/reject by E1, style, gap, lengths |

Default prompt templates (`DEFAULT_PROMPT_TEMPLATES` at `camd_lib.py:428-440`) — 8 stock Shakespeare prompts rotated round-robin by `pick_prompt_template(idx)`. Topic-specific prompts (Method 9 in `V4_2_NOVEL_METHODS.md`) are deferred.

## 6. DPO Training Pipeline (`train_camd_dpo.py`)

| Stage | Inputs | Outputs |
| --- | --- | --- |
| Pre-import CUDA env | `cuda13_blackwell.apply_preimport_cuda_env()` | `CUDA_VISIBLE_DEVICES`, `TORCH_CUDA_ARCH_LIST` (Blackwell 12.0) |
| Adapter load | `~/.contextgraph/models/shakespeare_lora/final` (v2.1, **NOT** v4.1) | PEFT model + base in 4-bit nf4 |
| Pair load | `--pairs` (CAMD JSONL), optional `--learner-pairs` | TRL-compatible dataset |
| Concat with `--learner-repeat ≥ 1` | Content pairs + repeated learner pairs | Single DPO loss surface |
| DPO training | `epochs=1`, `batch_size=4`, `grad_accum=4`, `lr=5e-6`, `β=0.1` | Updated LoRA + per-step JSONL telemetry |
| Acceptance gate | Browser FSV via Playwright (separate script) | 13/15 strict checks before HF push |

Key design constants baked into the trainer (per `train_camd_dpo.py:8-25`):

| Hyper-parameter | Value | Rationale |
| --- | --- | --- |
| β (DPO KL strength) | 0.1 | Conservative for style transfer; style adapters tolerate 0.05-0.3 |
| Learning rate | 5e-6 | Half of typical SFT LR (Cerebras DPO guidance) |
| `λ_sft` (Reg-DPO regularization) | 0.1 | ArXiv 2511.01450 anchors LoRA close to v2.1 supervised checkpoint |
| `loss_type` | `["sigmoid", "sft"]` | TRL 1.1.0 multi-loss expression of Reg-DPO |
| `loss_weights` | `[1.0, lambda_sft]` | Same |
| `length_normalize` | true (LD-DPO) | Removes length bias from chosen-vs-rejected log-prob delta |
| `max_length` | 1024 | Keep chosen+rejected each under ~500 tokens |
| `max_prompt_length` | 128 | |
| `lr_scheduler_type` | `cosine` | Smooth convergence over single epoch |
| `warmup_ratio` | 0.05 | 5% warmup |
| Optimizer | `paged_adamw_8bit` (bitsandbytes); fallback `adamw_torch` | |
| Quantization | 4-bit nf4 base, bf16 compute, double quant | Fits Qwen3.6-27B-it on RTX 5090 32 GB |
| LoRA rank | 256 | (Adapter-side; CAMD pair selection is unrelated to LoRA rank) |
| Gradient checkpointing | enabled (`use_reentrant=False`) | RTX 5090 32 GB constraint |
| `precompute_ref_log_probs` | optional | Avoids per-step ref forward pass; sidesteps Blackwell driver bugs |
| Seed | 42 | |

CLI flags surface every knob: `--lambda-sft`, `--max-length 1024`, `--max-prompt-length 128`, `--warmup-ratio 0.05`, `--seed 42`, `--dry-run`, `--max-steps`, `--limit-pairs`, `--learner-pairs`, `--learner-repeat`, `--optim`, `--precompute-ref`.

Failure handling: `write_failure_diagnostics(...)` writes `{output_dir}/failure_diagnostics.json` capturing args, paths (with SHA-256), `nvidia-smi` output, and `torch_cuda_state` (`device_index`, `device_name`, `compute_capability`, allocated/reserved/free VRAM in GiB) on any `trainer.train()` exception. The training manifest at `{output_dir}/training_manifest.json` is written atomically (`*.tmp` then rename) before, during, and after training.

Output directory: `~/.contextgraph/models/shakespeare_lora_v4_2/dpo`. Final adapter saved to `{output_dir}/final/` along with tokenizer and copied `chat_template.jinja` from v2.1.

## 7. Learner-Conditioned DPO Pair Filter (`build_learner_dpo_pairs.py`)

A pair is emitted only when **two real recorded assistant responses for the same `(task_id, query, retrieved_context)` group** exhibit different learner outcomes:

| Threshold | Default | Meaning |
| --- | --- | --- |
| `--min-gap` | 0.20 | `chosen.utility_delta - rejected.utility_delta ≥ 0.20` |
| `--min-chosen-utility` | 0.05 | Chosen response had positive utility |
| `--max-rejected-utility` | 0.00 | Rejected response had non-positive utility |
| `--include-feature-tensors` | true | Embed chosen/rejected feature vectors in pair metadata |
| `--overwrite` | false | Replace existing output file |

Algorithm (`build_pairs` at `build_learner_dpo_pairs.py:127`):

1. Load events JSONL produced by `context-graph-cli learning export-events-jsonl`.
2. Validate every row carries `record_kind=learning_event_tensor`, `event_id`, `query`, `retrieved_context`, `assistant_response`, `outcome`, `feature_tensor`, `feature_summary`, `provenance_sha256`. Schema length must equal feature_tensor length.
3. Group by `(task_id, query.strip(), retrieved_context.strip())`.
4. Within each group with ≥2 events: sort by `utility_delta`, pick lowest as `rejected` and highest as `chosen`.
5. Apply gap and threshold filters.
6. Emit prompt = `f"{query}\n\nContext:\n{context}"` (or just query when context empty).

If no group has a valid contrast, the script **fails closed** — it never synthesizes a rejected response (`build_learner_dpo_pairs.py:259-263`).

Output meta block carries `source = "learning_event_outcome"`, chosen/rejected `event_id`, `task_id`, session ids, outcome labels, utility deltas, the `learner_reward_gap`, and `combined_reward_components: { constellation_style: None, causal_validity: None, learner_well_being: gap }` (so future combined-reward training can fill the missing components without re-mining).

`train_camd_dpo.py::validate_pairs` enforces `meta.source == "learning_event_outcome"` for `--learner-pairs` to prevent accidental mixing.

## 8. Lessons Embedded in CAMD-DPO Code

| Pitfall | Mitigation in code |
| --- | --- |
| `hard_negative_scale ≥ 2.0` + `temperature 0.05` collapses logits | Use `hard_negative_scale = 1.0` (Rust E5 trainer default — see [16](16_training_subsystem.md) §4.3) |
| LoRA dropout adds VRAM (tracked grad tensors) | Drop `batch_size 16 → 8` when dropout enabled (Rust E5 trainer — see [16](16_training_subsystem.md) §4.2) |
| `Cell<bool>` inference-mode toggle (Rust embedder side) | LoRA train/inference paths share weights without re-instantiating |
| Train/inference prefix mismatch (nomic-embed `search_document:`) | Prefix asserted at load time |
| Best-stage selection by `directional_accuracy` is brittle | Select by `score_spread` (Rust trainer — see [16](16_training_subsystem.md) §4.7) |
| Per-stage LoRA checkpoints | Each stage keeps the projection it was trained against; CAMD-DPO inherits this discipline |
| Benchmark phases without config can't test the model | All bench phases take `&BenchConfig` |
| Judge-LLM bias | Replaced by deterministic constellation reward |
| Synthesizing rejected responses corrupts learning signal | `build_learner_dpo_pairs.py` fails closed when no real contrast exists |
| Single-anchor blind spot | Sonnets is the only anchor today; multi-anchor (Sonnets + Hamlet + Henry plays) is a deferred follow-up |

## 9. Phase 6 Additions (Learner-Conditioned DPO)

| Feature | Source script | RocksDB CF used |
| --- | --- | --- |
| Pairable retrieval events | `phase6_existing_data_inventory.py`, `shakespeare_phase6_policy_benchmark.py` | `CF_LEARNER_RETRIEVAL_LOG` |
| State-conditioned retrieval FSV | `shakespeare_state_conditioned_fsv.py` | `CF_LEARNER_PROFILE`, `CF_LEARNER_STATE_HISTORY` |
| Conceptual capabilities FSV | `conceptual_capabilities_fsv.py` | `get_capability_matrix` MCP tool |
| Agent workflow learning loop | `agent_workflow_learning.py` | `CF_LEARNING_EVENTS` |
| Learner-conditioned DPO pairs | `build_learner_dpo_pairs.py` | `CF_LEARNING_EVENTS` (via `learning export-events-jsonl`) |
| DPO trainer telemetry | `train_camd_dpo.py` (no MCP) | Filesystem JSONL |
| Empirical closed-loop runner | `shakespeare_empirical_cycle.py` | All learning + content CFs |
| Evidence inventory | `phase6_existing_data_inventory.py` | Aggregates over all CFs |
| Constellation invariants | `shakespeare_constellation_guard.py` | `CF_CONSTELLATIONS` |

## 10. Rust-Side Causal Benchmarks (Validation Surface)

The Python DPO trainer has no direct Rust integration. Instead, the Rust benchmark crate provides the validation surface that confirms whether a fine-tuned E5 + DPO-adapted LoRA improved end-to-end behaviour:

| Binary | What it measures |
| --- | --- |
| `causal_accuracy_bench` | Phases 1.1, 1.2, 3.4, 4.1, 4.2 from the E5 plan: vector distinctness (cosine cause/effect per category), marker detection (precision/recall/F1 for `detect_causal_query_intent`), 3-class query intent accuracy on 100 labeled queries, asymmetric similarity sweep across 11 modifier pairs, optimal modifier per causal mechanism. GPU-gated Phase 1.1 needs `--features real-embeddings` |
| `causal_provenance_e2e_bench` | End-to-end provenance pipeline: span populated rate (>95% target), span text match (>85%), offset validity (>95%), storage success (100%), provenance chain complete (100%), search provenance display (100%) |
| `causal_e2e_bench`, `causal_perf_bench`, `causal_realdata_bench` | Latency/throughput and live-DB end-to-end |
| `causal_provenance_bench`, `causal_provenance_llm_bench` | Provenance coverage with and without LLM |
| `causal_prompt_experiment_bench`, `causal_span_fix_bench` | Prompt and span-extraction experiments |
| `causal_discovery_mcp_bench` | MCP `trigger_causal_discovery` throughput |
| `causal_embedding_bench` | E5 dual-vector quality |

These benchmarks consume the trained E5 weights (`models/causal/trained/lora_best.safetensors` + `projection_best.safetensors`) loaded by `ProductionMultiArrayProvider::new`. They never invoke Python.

## 11. Scripts Inventory (`scripts/v4_2/`)

| Script | Lines | Role |
| --- | --- | --- |
| `__init__.py` | — | Module init |
| `camd_lib.py` | ~492 | Base 13-view daemon client + filter + scorer |
| `extended_camd.py` | 428 | E14 sidecar CAMD wrapper |
| `extended_constellation.py` | 261 | E14 constellation extensions |
| `extended_ensemble.py` | 508 | Multi-embedder ensemble scoring |
| `extended_fsv.py` | 351 | Sidecar full-state verification |
| `extended_store.py` | 420 | LMDB sidecar persistence |
| `bigunlock_fsv_lib.py` | — | BigUnlock harness (sidecar) |
| `learner_state_bigunlock_fsv.py` | 379 | Learner-state FSV |
| `mine_camd_pairs.py` | 251 | Mine DPO pairs from `CF_CONTRASTIVE_PAIRS` |
| `mine_camd_pairs_extended.py` | 353 | Sidecar (E14) variant |
| `mine_camd_from_lmdb.py` | 300 | LMDB sidecar miner |
| `build_learner_dpo_pairs.py` | ~283 | Learner-conditioned DPO pairs from learning events |
| `train_camd_dpo.py` | 487 | TRL DPO training loop with telemetry |
| `validate_camd.py` | 307 | 3-part pair validation (reward sanity, pair discrimination, stats) |
| `analyze_correlations.py` | — | Cross-embedder correlation analysis |
| `backfill_extended.py` | — | Backfill sidecar views |
| `shakespeare_constellation_guard.py` | 551 | Constellation invariants for Shakespeare run |
| `shakespeare_corpus_fsv.py` | 1113 | Corpus FSV |
| `shakespeare_embedding_benchmark.py` | 926 | Embedding benchmark |
| `shakespeare_empirical_cycle.py` | 732 | Closed-loop runner |
| `shakespeare_learner_training_fsv.py` | 436 | Learner training FSV |
| `shakespeare_learning_event_fsv.py` | 432 | Learning event ingest FSV |
| `shakespeare_phase6_policy_benchmark.py` | 846 | State-conditioned policy benchmark |
| `shakespeare_state_conditioned_fsv.py` | 387 | State-conditioned retrieval FSV |
| `phase6_existing_data_inventory.py` | 426 | Phase-6 evidence inventory |
| `agent_workflow_learning.py` | — | Agent workflow learning loop |
| `conceptual_capabilities_fsv.py` | — | `get_capability_matrix` FSV |
| `cuda13_blackwell.py` | 131 | CUDA 13 / RTX 5090 (compute_cap 12.0) helpers |
| `cuda13_wsl_fsv.py` | 377 | WSL CUDA FSV |
| `unified_realdata_fsv.py` | 448 | Unified real-data FSV |
| `smoke_dpo_v4_2.py` | 252 | Smoke test |

Total ≈ 15,447 lines of Python in `scripts/v4_2/`.

## 12. Cache Sidecar and Output Paths

| Path | Purpose |
| --- | --- |
| `~/.contextgraph/extended_embeddings.lmdb` | LMDB sidecar for E14 / extended views; not part of sacred RocksDB. Regenerable by `backfill_extended.py` |
| `~/.contextgraph/training/v4_2_camd_pairs.jsonl` | Production CAMD pair file (target ≈ 5,000 pairs from a 13,500-pair input pool) |
| `~/.contextgraph/models/shakespeare_lora/final` | v2.1 starting LoRA |
| `~/.contextgraph/models/shakespeare_lora_v4_2/dpo` | Output dir for DPO-trained LoRA |
| `~/.contextgraph/models/shakespeare_lora_v4_2/dpo/final/` | Final saved adapter + tokenizer + chat_template.jinja |
| `~/.contextgraph/models/shakespeare_lora_v4_2/dpo/training_manifest.json` | Atomic trainer manifest (status: prepared → ok) |
| `~/.contextgraph/models/shakespeare_lora_v4_2/dpo/failure_diagnostics.json` | Written on `trainer.train()` exception with full state capture |

## 13. Source Notes

- Base CAMD remains 13-view. E14 access is via `extended_*` sidecar scripts (no direct `camd_lib.py` change).
- Learner-conditioned DPO joins `CF_LEARNER_RETRIEVAL_LOG` and `CF_LEARNING_EVENTS` to assemble pairs that account for affect state. The Rust handler `record_learning_event` (see [20](20_learning_subsystem.md) §12) is the source of truth.
- DPO trainer telemetry writes per-step JSONL for offline analysis. There is **no MCP integration for DPO training itself**; the MCP surface is restricted to evidence ingest (`record_learning_event`, `record_graph_learning_event`) and policy resolution (`resolve_learner_retrieval_policy`, `resolve_graph_learning_policy`).
- The Rust DynamicJEPA subsystem ([21](21_dynamicjepa_subsystem.md)) is **separate** from CAMD-DPO and uses an entirely different training pipeline (Candle, CUDA, VICReg + EMA target encoder) that does **not** touch the LLM weights.
- TRL 1.1.0 introduced `processing_class=` (replacing `tokenizer=`) and the `loss_type=['sigmoid','sft']` + `loss_weights` pattern for Reg-DPO. Older TRL used `rpo_alpha` + single `loss_type='sigmoid'` (`train_camd_dpo.py:399-414`).
- The acceptance gate (Playwright browser FSV, 13/15 strict checks before HF push) is described in `docs/V4_2_NOVEL_METHODS.md` §5 and implemented outside this directory.
