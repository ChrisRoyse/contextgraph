# CAMD DPO Subsystem

**Source files covered:**
- `scripts/v4_2/*.py`
- `scripts/_gemma_shakespeare_lib.py`

## 1. Scope
CAMD-DPO scripts mine deterministic DPO pairs from contrastive pairs and constellation scores. Base `camd_lib.py` is a 13-view daemon consumer: its `EMB` map covers E1-E13 and does not include E14.

## 2. Style Weights
| Embedder | Weight |
| --- | --- |
| E7 | 0.15 |
| E9 | 0.15 |
| E10 | 0.15 |
| E12 | 0.30 |
| E13 | 0.25 |

## 3. Data Classes
| Class | Behavior |
| --- | --- |
| ConstellationScore | per_embedder scores, combined_score, in_spread_p95, style_score |
| ContrastivePair | anchor/negative texts, 13D similarity profile, anomaly metadata |
| CAMDPair | prompt/chosen/rejected plus scores; score_gap = chosen style - rejected style |
| ConstellationScorer | calls score_against_constellation and cleans up probes |
| CAMDPairFilter | filters by semantic/style similarity and score gap |

## 4. Scripts
| Script | Role |
| --- | --- |
| scripts/v4_2/__init__.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/analyze_correlations.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/backfill_extended.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/camd_lib.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/extended_camd.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/extended_constellation.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/extended_ensemble.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/extended_fsv.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/extended_store.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/mine_camd_from_lmdb.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/mine_camd_pairs.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/mine_camd_pairs_extended.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/smoke_dpo_v4_2.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/train_camd_dpo.py | v4_2 CAMD or extended CAMD script |
| scripts/v4_2/validate_camd.py | v4_2 CAMD or extended CAMD script |

## 5. Source Gap
Rust core now has E14, but base CAMD remains 13-view. Extended scripts add sidecar views rather than updating base CAMD to E14.
