# CAMD-DPO Subsystem

**Source files covered:**
- `scripts/v4_2/camd_lib.py`
- `scripts/v4_2/extended_camd.py`, `extended_constellation.py`, `extended_ensemble.py`, `extended_fsv.py`, `extended_store.py`
- `scripts/v4_2/mine_camd_pairs.py`, `mine_camd_pairs_extended.py`, `mine_camd_from_lmdb.py`
- `scripts/v4_2/build_learner_dpo_pairs.py`
- `scripts/v4_2/train_camd_dpo.py`
- `scripts/v4_2/validate_camd.py`, `analyze_correlations.py`, `backfill_extended.py`
- `scripts/v4_2/shakespeare_*.py` (corpus FSV, empirical cycle, learner training FSV, learning event FSV, phase6 policy benchmark, state-conditioned FSV, embedding benchmark, constellation guard)
- `scripts/v4_2/conceptual_capabilities_fsv.py`, `agent_workflow_learning.py`, `phase6_existing_data_inventory.py`
- `scripts/v4_2/cuda13_blackwell.py`, `cuda13_wsl_fsv.py`, `unified_realdata_fsv.py`, `bigunlock_fsv_lib.py`, `learner_state_bigunlock_fsv.py`, `smoke_dpo_v4_2.py`
- `scripts/_gemma_shakespeare_lib.py`

**Cross-links:** [16_training_subsystem.md](16_training_subsystem.md), [17_constellation_subsystem.md](17_constellation_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md), [20_learning_subsystem.md](20_learning_subsystem.md), [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md).

## 1. Scope
The CAMD-DPO subsystem is **out-of-Rust Python tooling** that mines deterministic Direct Preference Optimization (DPO) pairs from RocksDB-resident contrastive pairs, learning events, retrieval logs, and constellation scores, then trains a base LLM (Gemma-4-E4B-it) with a Shakespeare-style LoRA adapter using `trl` DPO. CAMD = **Constellation-Aided Memetic Distillation**: chosen/rejected pairs are scored against a `Sonnets` constellation (E1-E14 multi-view) without a judge LLM. Phase 6 adds **learner-conditioned DPO** that joins `CF_LEARNING_EVENTS` and `CF_LEARNER_RETRIEVAL_LOG` so the same prompt's two real assistant responses with different learner outcomes become one preference pair.

Base CAMD remains 13-view. E14 access is via the `extended_*` sidecar scripts and `bigunlock_fsv_lib.py`; sidecar embeddings live in an LMDB cache at `~/.contextgraph/extended_embeddings.lmdb` and never enter the sacred RocksDB store.

## 2. Method Stack
| Method | Rationale |
| --- | --- |
| DPO (TRL) | Direct preference optimization; no reward model |
| Reg-DPO SFT regularization (`λ_sft = 0.1`, ArXiv 2511.01450) | Anchors LoRA close to the v2.1 supervised checkpoint |
| Length-normalized reward (LD-DPO) | Removes length bias from the chosen-vs-rejected log-prob delta |
| β = 0.1 KL regularization | Conservative for style transfer (style adapters tolerate 0.05-0.3) |
| Cosine LR schedule, 5% warmup, decay to 10% | Smooth convergence over a single epoch |
| bf16 compute, 4-bit nf4 base, rank-256 LoRA | Fits Gemma-4-E4B-it on RTX 5090 32 GB with gradient checkpointing |
| Per-stage LoRA checkpoints | Best stage's LoRA pairs with its own projection (lesson: catastrophic mismatch otherwise) |

## 3. Style Weights (base CAMD constellation scoring)
| Embedder | Weight | Role |
| --- | --- | --- |
| E7 | 0.15 | Code/structure |
| E9 | 0.15 | HDC robust style |
| E10 | 0.15 | Multimodal paraphrase |
| E12 | 0.30 | ColBERT token-level (highest weight) |
| E13 | 0.25 | SPLADE sparse style |

(See `camd_lib.py` `STYLE_WEIGHTS`.)

## 4. CAMD Filter (mine_camd_pairs.py)
A pair `{chosen, rejected}` is admitted only when ALL of the following hold:

| Threshold | Default | Meaning |
| --- | --- | --- |
| `min-e1` | 0.70 | Semantic content match (chosen ≈ rejected meaning) |
| `max-style-sim` | 0.60 | Style embedders disagree (different style) |
| `min-gap` | 0.15 | `style_score(chosen) - style_score(rejected) ≥ 0.15` |
| `min-chosen` | 0.50 | Chosen scores ≥ 0.50 against Sonnets constellation |
| `max-rejected` | 0.40 | Rejected scores ≤ 0.40 against Sonnets constellation |
| `min-text-len` | 40 | Length floor for both sides |
| `max-text-len` | 3000 | Length cap |
| `fail-budget` | 5 % | Abort run if scoring failures exceed this fraction |

Atomic write pattern: output written to `<file>.partial` then renamed; sidecar `<file>.state.json` allows `--resume`.

## 5. Data Classes (camd_lib.py)
| Class | Fields |
| --- | --- |
| `ConstellationScore` | `per_embedder` (dict), `combined_score`, `in_spread_p95`, `style_score` |
| `ContrastivePair` | `anchor_text`, `negative_text`, 13D similarity profile, `anomaly_kind`, `disagreement` |
| `CAMDPair` | `prompt`, `chosen`, `rejected`, `chosen_scores`, `rejected_scores`, `score_gap`, meta |
| `ConstellationScorer` | Wraps daemon `score_against_constellation`; cleans up probes |
| `CAMDPairFilter` | Accept/reject by E1, style, gap, lengths |

## 6. Training Pipeline (`train_camd_dpo.py`)
| Stage | Inputs | Outputs |
| --- | --- | --- |
| Pre-import CUDA env | None | `CUDA_VISIBLE_DEVICES`, `TORCH_CUDA_ARCH_LIST` (Blackwell 12.0) |
| Adapter load | `~/.contextgraph/models/shakespeare_lora/final` (v2.1, **NOT** v4.1) | PEFT model + base in 4-bit nf4 |
| Pair load | `--pairs` (CAMD JSONL), optional `--learner-pairs` | TRL-compatible dataset |
| Concat with `--learner-repeat ≥ 1` | Content pairs + repeated learner pairs | Single DPO loss surface |
| DPO training | `epochs=1`, `batch_size=4`, `grad_accum=4`, `lr=5e-6`, `β=0.1` | Updated LoRA + telemetry per step |
| Telemetry | Per-step JSONL | Offline analysis only (no MCP) |
| Acceptance gate | Browser FSV via Playwright | 13/15 strict checks before HF push (§5 acceptance criteria) |

CLI flags surface every knob: `--lambda-sft`, `--max-length 1024`, `--max-prompt-length 128`, `--warmup-ratio 0.05`, `--seed 42`, `--dry-run`, `--max-steps`, `--limit-pairs`.

## 7. Lessons Embedded in Training Code
| Pitfall | Mitigation in code |
| --- | --- |
| `hard_negative_scale ≥ 2.0` + `temperature 0.05` collapses logits | Use `hard_negative_scale = 1.0` |
| LoRA dropout adds VRAM (tracked grad tensors) | Drop `batch_size 16 → 8` when dropout enabled |
| `Cell<bool>` inference-mode toggle (Rust embedder side) | LoRA train/inference paths share weights without re-instantiating |
| Train/inference prefix mismatch (nomic-embed `search_document:`) | Prefix asserted at load time |
| Best-stage selection by `directional_accuracy` is brittle | Select by `score_spread` |
| Per-stage LoRA checkpoints | Each stage keeps the projection it was trained against |
| Benchmark phases without config can't test the model | All bench phases take `&BenchConfig` |

## 8. Phase 6 Additions (Learner-Conditioned DPO)
| Feature | Source | RocksDB CF used |
| --- | --- | --- |
| Pairable retrieval events | `phase6_existing_data_inventory.py`, `shakespeare_phase6_policy_benchmark.py` | `CF_LEARNER_RETRIEVAL_LOG` |
| State-conditioned retrieval FSV | `shakespeare_state_conditioned_fsv.py` | `CF_LEARNER_PROFILE`, `CF_LEARNER_STATE_HISTORY` |
| Conceptual capabilities FSV | `conceptual_capabilities_fsv.py` | `get_capability_matrix` |
| Agent workflow learning loop | `agent_workflow_learning.py` | `CF_LEARNING_EVENTS` |
| Learner-conditioned DPO pairs | `build_learner_dpo_pairs.py` | `CF_LEARNING_EVENTS` (via `learning export-events-jsonl`) |
| DPO trainer telemetry | `train_camd_dpo.py` (no MCP) | Filesystem JSONL |
| Empirical closed-loop runner | `shakespeare_empirical_cycle.py` | All learning + content CFs |
| Evidence inventory | `phase6_existing_data_inventory.py` | Aggregates over all CFs |
| Constellation invariants | `shakespeare_constellation_guard.py` | `CF_CONSTELLATIONS` |

## 9. Learner Pair Filter (`build_learner_dpo_pairs.py`)
A pair is emitted only when **two real recorded assistant responses for the same `(task_id, query, retrieved_context)` group** exhibit different learner outcomes:

| Threshold | Default | Meaning |
| --- | --- | --- |
| `min-gap` | 0.20 | `chosen.utility_delta - rejected.utility_delta ≥ 0.20` |
| `min-chosen-utility` | 0.05 | Chosen response had positive utility |
| `max-rejected-utility` | 0.00 | Rejected response had non-positive utility |
| `include-feature-tensors` | true | Embed chosen/rejected feature vectors in pair metadata |

If no group has a valid contrast, the script **fails closed** — it never synthesizes a rejected response.

## 10. Scripts Inventory (`scripts/v4_2/`)
| Script | Role |
| --- | --- |
| `__init__.py` | Module init |
| `camd_lib.py` | Base 13-view daemon client + filter + scorer |
| `extended_camd.py` | E14 sidecar CAMD wrapper |
| `extended_constellation.py` | E14 constellation extensions |
| `extended_ensemble.py` | Multi-embedder ensemble scoring |
| `extended_fsv.py` | Sidecar full-state verification |
| `extended_store.py` | LMDB sidecar persistence |
| `bigunlock_fsv_lib.py` | BigUnlock harness (sidecar) |
| `learner_state_bigunlock_fsv.py` | Learner-state FSV |
| `mine_camd_pairs.py` | Mine DPO pairs from `CF_CONTRASTIVE_PAIRS` |
| `mine_camd_pairs_extended.py` | Sidecar (E14) variant |
| `mine_camd_from_lmdb.py` | LMDB sidecar miner |
| `build_learner_dpo_pairs.py` | Learner-conditioned DPO pairs from learning events |
| `train_camd_dpo.py` | TRL DPO training loop with telemetry |
| `validate_camd.py` | Pair validation |
| `analyze_correlations.py` | Cross-embedder correlation analysis |
| `backfill_extended.py` | Backfill sidecar views |
| `shakespeare_constellation_guard.py` | Constellation invariants for Shakespeare run |
| `shakespeare_corpus_fsv.py` | Corpus FSV |
| `shakespeare_embedding_benchmark.py` | Embedding benchmark |
| `shakespeare_empirical_cycle.py` | Closed-loop runner |
| `shakespeare_learner_training_fsv.py` | Learner training FSV |
| `shakespeare_learning_event_fsv.py` | Learning event ingest FSV |
| `shakespeare_phase6_policy_benchmark.py` | State-conditioned policy benchmark |
| `shakespeare_state_conditioned_fsv.py` | State-conditioned retrieval FSV |
| `phase6_existing_data_inventory.py` | Phase-6 evidence inventory |
| `agent_workflow_learning.py` | Agent workflow learning loop |
| `conceptual_capabilities_fsv.py` | `get_capability_matrix` FSV |
| `cuda13_blackwell.py` | CUDA 13 / RTX 5090 (compute_cap 12.0) helpers |
| `cuda13_wsl_fsv.py` | WSL CUDA FSV |
| `unified_realdata_fsv.py` | Unified real-data FSV |
| `smoke_dpo_v4_2.py` | Smoke test |

## 11. Cache Sidecar
- `~/.contextgraph/extended_embeddings.lmdb` — LMDB sidecar for E14 / extended views; not part of sacred RocksDB. Regenerable by `backfill_extended.py`.
- `~/.contextgraph/training/v4_2_camd_pairs.jsonl` — production CAMD pair file (target ≈ 5 000 pairs from a 13 500-pair input pool).
- `~/.contextgraph/models/shakespeare_lora/final` — v2.1 starting LoRA.
- `~/.contextgraph/models/shakespeare_lora_v4_2/dpo` — output dir for DPO-trained LoRA.

## 12. Source Notes
- Base CAMD remains 13-view. E14 access is via `extended_*` sidecar scripts (no direct `camd_lib.py` change).
- `learner-conditioned DPO` joins `CF_LEARNER_RETRIEVAL_LOG` and `CF_LEARNING_EVENTS` to assemble pairs that account for affect state. The Rust handler `record_learning_event` (see [20](20_learning_subsystem.md)) is the source of truth.
- DPO trainer telemetry writes per-step JSONL for offline analysis. There is no MCP integration for DPO training itself; the MCP surface is restricted to evidence ingest (`record_learning_event`, `record_graph_learning_event`) and policy resolution (`resolve_learner_retrieval_policy`, `resolve_graph_learning_policy`).
- The Rust DynamicJEPA subsystem ([21](21_dynamicjepa_subsystem.md)) is **separate** from CAMD-DPO and uses an entirely different training pipeline (Candle, CUDA, VICReg + EMA target encoder) that does **not** touch the LLM weights.
