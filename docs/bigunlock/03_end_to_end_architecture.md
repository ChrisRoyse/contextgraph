# 03 End-To-End Architecture

## Core loop

```text
Shakespeare content substrate
  -> retrieval candidates
  -> learner-state-conditioned policy
  -> response/explanation
  -> learner action/outcome
  -> LearningEvent
  -> learner training dataset
  -> DPO/reranker/diagnostic training
  -> updated runtime policy/model
```

## Sources of truth

| Layer | Trigger | Source of truth | Verification read |
| --- | --- | --- | --- |
| content ingest | `store_memories` | `CF_FINGERPRINTS`, `CF_CONTENT`, `CF_SOURCE_METADATA`, `CF_EMBEDDING_REGISTRY` | `get_memory_fingerprint`, `get_provenance_chain`, `ldb` |
| content constellation | `compile_constellation` | `CF_CONSTELLATIONS`, `CF_CONSTELLATION_BY_SELECTOR` | `get_constellation`, `list_constellations`, `ldb` |
| learner profile | `register_learner` | `CF_LEARNER_PROFILE` | `get_learner_state`, `count_learner_state`, `ldb` |
| learner observation | `record_session_observation` | `CF_FINGERPRINTS_LEARNER`, `CF_LEARNER_STATE_HISTORY`, `CF_LEARNER_AUDIT` | `get_learner_state`, `count_learner_state`, `ldb` |
| retrieval policy | `resolve_learner_retrieval_policy` or `search_graph` | no mutation by default | returned selected profile + repeated search evidence |
| learning event | `record_learning_event` | `CF_LEARNING_EVENTS` | `get_learning_event`, `count_learning_events`, `ldb` |
| UTL L/M update | `compute_L` with `persist=true` | `CF_LEARNER_DELTA_LOG`, `CF_LEARNER_M_PER_TRACE` | `get_learner_state`, `get_learner_M`, `next_review_for_trace`, `ldb` |
| training export | `export_learner_training_dataset` | `CF_LEARNER_TRAINING_DATASETS` | `get_learner_training_dataset`, `count_learner_training_datasets`, `ldb` |

## Happy path sequence

1. Register learner.
2. Record baseline observations.
3. Compile learner constellation.
4. Run a Shakespeare query with state-conditioned retrieval.
5. Generate or select a response.
6. Ask learner for answer or feedback.
7. Record after-state observation.
8. Compute and persist L.
9. Record learning event.
10. Export learner training dataset.
11. Train/evaluate model or reranker.
12. Compare against prior baseline.

## Runtime path

For an interactive Shakespeare tutor:

```text
User question
  -> read latest learner state
  -> choose retrieval profile
  -> search Shakespeare corpus
  -> optionally score against play constellation
  -> build answer
  -> log retrieved memory IDs
  -> ask learner/observe outcome
  -> persist LearningEvent and UTL state
```

The retrieval step should fail if state-conditioned retrieval is requested but no learner state exists.

## Batch training path

For offline training:

```text
CF_LEARNING_EVENTS
  + CF_LEARNER_STATE_HISTORY
  + CF_LEARNER_DELTA_LOG
  + CF_FINGERPRINTS/CF_TRAINING_RECORDS
  -> export_learner_training_dataset
  -> scripts/v4_2/build_learner_dpo_pairs.py
  -> scripts/v4_2/train_camd_dpo.py
  -> evaluation suite
```

The training export must preserve IDs so every row can be traced back to physical RocksDB records.

## Failure policy

No silent fallback:

- Missing learner state: error.
- Missing content record: error.
- Missing training row: error.
- Missing consent for training export: error or excluded with an explicit count.
- Missing E15-E21 asset during required preflight: error.
- Unsupported modality: validation error.
- Shape mismatch: validation error.
- Partial write: rollback or error with exact failed CF.

## Observability

Every state-changing operation needs:

- request ID
- operator ID or learner ID
- target CF
- physical key count before/after in FSV
- readback proof
- audit/provenance row when applicable

The system should expose enough evidence through MCP for normal clients, but FSV must still inspect RocksDB directly with `ldb` or storage-level tests.

