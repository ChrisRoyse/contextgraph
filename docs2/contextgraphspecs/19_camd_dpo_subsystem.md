# CAMD-DPO Subsystem

**Source files covered:**
- `scripts/v4_2/*.py`
- `scripts/_gemma_shakespeare_lib.py`

## 1. Scope
Python-side CAMD-DPO scripts mine deterministic DPO pairs from contrastive pairs and constellation scores, run learner-conditioned DPO, and execute Shakespeare empirical cycles. Base `camd_lib.py` is a 13-view daemon consumer; E14 access is via extended sidecar scripts and `bigunlock_fsv_lib.py`. Phase 6 adds pairable retrieval-event mining and policy benchmarks.

## 2. Style Weights (base CAMD)
| Embedder | Weight |
| --- | --- |
| E7 | 0.15 |
| E9 | 0.15 |
| E10 | 0.15 |
| E12 | 0.30 |
| E13 | 0.25 |

## 3. Data Classes (camd_lib.py)
| Class | Fields |
| --- | --- |
| ConstellationScore | per_embedder, combined_score, in_spread_p95, style_score |
| ContrastivePair | anchor/negative texts, 13D similarity profile, AnomalyKind, disagreement |
| CAMDPair | prompt, chosen, rejected, scores, score_gap |
| ConstellationScorer | Calls `score_against_constellation`; cleans up probes |
| CAMDPairFilter | Filters by semantic / style similarity and score gap |

## 4. Phase 6 Additions
| Feature | Source |
| --- | --- |
| Pairable retrieval events | `phase6_existing_data_inventory.py`, `shakespeare_phase6_policy_benchmark.py` |
| DPO trainer telemetry | `train_camd_dpo.py` |
| Learner-conditioned DPO pairs | `build_learner_dpo_pairs.py` |
| Shakespeare empirical closed-loop | `shakespeare_empirical_cycle.py` |
| State-conditioned retrieval FSV | `shakespeare_state_conditioned_fsv.py` |
| Conceptual capabilities FSV | `conceptual_capabilities_fsv.py` |
| Agent workflow learning | `agent_workflow_learning.py` |
| Evidence inventory | `phase6_existing_data_inventory.py` |

## 5. Scripts (`scripts/v4_2/`)
| Script | Role |
| --- | --- |
| `__init__.py` | Module init |
| `camd_lib.py` | Base 13-view daemon client |
| `extended_camd.py` | E14 sidecar CAMD wrapper |
| `extended_constellation.py` | E14 constellation extensions |
| `extended_ensemble.py` | Multi-embedder ensemble scoring |
| `extended_fsv.py` | Sidecar FSV |
| `extended_store.py` | LMDB sidecar persistence |
| `bigunlock_fsv_lib.py` | BigUnlock FSV harness |
| `learner_state_bigunlock_fsv.py` | Learner-state full state verification |
| `mine_camd_pairs.py` | Mine DPO pairs from contrastive pairs |
| `mine_camd_pairs_extended.py` | Extended sidecar version |
| `mine_camd_from_lmdb.py` | LMDB sidecar miner |
| `build_learner_dpo_pairs.py` | Learner-conditioned DPO pairs |
| `train_camd_dpo.py` | DPO training loop with telemetry |
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
| `phase6_existing_data_inventory.py` | Evidence inventory |
| `agent_workflow_learning.py` | Agent workflow learning loop |
| `conceptual_capabilities_fsv.py` | get_capability_matrix FSV |
| `cuda13_blackwell.py` | CUDA 13 / RTX 5090 helpers |
| `cuda13_wsl_fsv.py` | WSL CUDA FSV |
| `unified_realdata_fsv.py` | Unified real-data FSV |
| `smoke_dpo_v4_2.py` | Smoke test |

## 6. Cache Sidecar
`~/.contextgraph/extended_embeddings.lmdb` — LMDB sidecar for E14 / extended views; not part of sacred RocksDB store. Regenerable.

## 7. Source Notes
- Base CAMD remains 13-view; E14 always entered via `extended_*` sidecar scripts (no direct `camd_lib.py` change).
- `learner-conditioned DPO` joins `CF_LEARNER_RETRIEVAL_LOG` and `CF_LEARNING_EVENTS` to assemble pairs that account for affect state.
- DPO trainer telemetry writes per-step metrics for offline analysis (no MCP integration).
