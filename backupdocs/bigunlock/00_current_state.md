# 00 Current State

## Proven content substrate

The complete Shakespeare corpus has been processed into a verified content-side substrate.

Source corpus:

```text
/home/cabdru/contextgraph/shakespear/complete_works.txt
```

Full FSV report:

```text
/home/cabdru/contextgraph/tmp/shakespeare_full_fsv_report.json
```

Clean FSV RocksDB source of truth:

```text
/home/cabdru/contextgraph/tmp/shakespeare_full_fsv_db_20260428_timeout_044230
```

Verified physical CF counts from `ldb`:

| CF | Count |
| --- | ---: |
| `fingerprints` | 1552 |
| `content` | 1552 |
| `source_metadata` | 1552 |
| `embedding_registry` | 1552 |
| `e1_matryoshka_128` | 1552 |
| `e12_late_interaction` | 1552 |
| `training_records` | 1552 |
| `constellations` | 43 |
| `audit_log` | 1639 |
| `audit_by_target` | 1639 |

The verified content records include all 14 content embedders:

| Slot | Role |
| --- | --- |
| E1 | dense semantic |
| E2 | recency |
| E3 | periodicity |
| E4 | sequence position |
| E5 | causal asymmetric |
| E6 | sparse keyword |
| E7 | code/pattern |
| E8 | graph structure |
| E9 | HDC/noise robust |
| E10 | paraphrase/multimodal |
| E11 | entity |
| E12 | ColBERT/token precision |
| E13 | SPLADE expansion |
| E14 | BGE-M3 dense multilingual/long-context |

## Proven operational surfaces

The FSV run verified:

- batch real-data ingestion through `store_memories`
- structured MCP tool output
- E14 visible through `get_memory_fingerprint`
- `get_provenance_chain` readback for embedding versions and audit rows
- search against the persisted Shakespeare corpus
- 43 work-level constellations
- training export into `CF_TRAINING_RECORDS`
- no state mutation on invalid edge-case requests

Benchmarks from the clean full run:

| Operation | Result |
| --- | ---: |
| corpus chunks | 1552 |
| ingest elapsed | 12855.26 sec |
| ingest rate | 0.1207 chunks/sec |
| ingest p50 | 9945 ms |
| ingest p95 | 13726 ms |
| Hamlet search | 0.293 sec |
| Macbeth search | 0.347 sec |
| Romeo/Juliet search | 0.360 sec |
| constellation compile | 43 in 41.37 sec |
| training export | 1552 in 3.26 sec |

## Current 21-slot truth

The system now exposes a 21-slot capability matrix:

- E1-E14 are content-side slots.
- E15-E21 are learner-state slots.

The Shakespeare FSV populated E1-E14 only. That is correct: a text corpus cannot honestly populate physiological or learner-state vectors.

Current E15-E21 destination CFs and tools exist in the learner subsystem:

| Area | Source of truth |
| --- | --- |
| learner profile | `CF_LEARNER_PROFILE` |
| learner fingerprint | `CF_FINGERPRINTS_LEARNER` |
| learner state history | `CF_LEARNER_STATE_HISTORY` |
| delta logs | `CF_LEARNER_DELTA_LOG` |
| retrieval practice | `CF_LEARNER_RETRIEVAL_LOG` |
| sleep multiplier | `CF_LEARNER_K_SLEEP` |
| M traces | `CF_LEARNER_M_PER_TRACE` |
| skill goals | `CF_GOAL_CENTROIDS`, `CF_LEARNER_GOAL_STATES` |
| learner constellations | `CF_LEARNER_CONSTELLATIONS` |
| learner training datasets | `CF_LEARNER_TRAINING_DATASETS` |
| learner audit | `CF_LEARNER_AUDIT` |

## What is not proven yet

These are not failures; they are the next FSV targets.

| Capability | Current state | Required proof |
| --- | --- | --- |
| real E15-E21 ingestion | tools/storage exist | write real or instrument-derived learner observations and verify CFs |
| state-conditioned Shakespeare retrieval | policy exists | run same query under different persisted states and prove selected profile/ranking changes |
| learner outcome loop | learning events exist | retrieve, respond, log outcome, export dataset, verify row |
| learner-conditioned DPO | scripts exist | train/eval on real outcome events, compare before/after |
| real physiological channels | assets/tools exist | at least one real channel such as EDA/PPG flows through observation -> state -> policy |
| runtime generation guard | concept planned | fingerprint generated text, compare to constellation, log correction events |

## Design constraint

Do not append E15-E21 onto the `TeleologicalFingerprint` content wire format. Content fingerprints stay E1-E14. Learner-state data belongs in `LearnerFingerprint` and `LearnerStateVector`, keyed by learner/session.

