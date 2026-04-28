# Implementation Roadmap

## Immediate Current-State Follow-Ups

1. Decide whether to track `.cargo/config.toml`.
   - Current local file points CUDA builds to 13.2 and `compute_120a`.
   - If left untracked, another checkout may silently build against CUDA 13.1.

2. Refresh old specs under `docs2/contextgraphspecs/`.
   - They still contain 13-embedder and CUDA 13.1 language.
   - The verified runtime now has 14 production embedders and CUDA 13.2.

3. Add an automated guard for provider health.
   - `ProductionMultiArrayProvider::is_ready()` now includes E14.
   - Add a regression test that toggles an unhealthy E14 adapter and expects provider readiness to fail.

4. Add a constellation FSV integration test.
   - Core tests now pass and storage encoding is versioned.
   - A targeted test should write a constellation with 91 cross-correlation centroid values, close/reopen RocksDB, and verify `CONSTELLATION_VERSION=2`.

5. Add a real-data backfill test when production fingerprints exist.
   - Current live store has zero fingerprints.
   - Synthetic backfill tests pass, but a real-data migration remains unexercised because there is no live data.

## UTL Implementation Milestones

### Milestone 1 - Learning Event Storage

Deliver:

- `LearningEvent`
- `LearningOutcome`
- `LearningStateSnapshot`
- RocksDB column family `learning_events`
- CLI/MCP tool to inspect learning events.

FSV:

- Write event -> close/reopen -> read event.
- Verify 14D before/after profile and 91D correlations.
- Verify invalid vector length fails.

### Milestone 2 - Deterministic UTL Feature Computation

Deliver:

- `compute_delta_e`
- `compute_surprise`
- `compute_coherence_delta`
- `compute_consolidation_readiness`
- `compute_transfer_score`

FSV:

- Synthetic cases with known expected math.
- Print before/after state for each edge case.
- Persist computed features and re-read them.

### Milestone 3 - Learning Signal Embedders

Deliver:

- `LearningSignalId`
- `LearningSignal`
- `LearningSignalEmbedder` trait.
- Deterministic baseline embedders before any trained model.

FSV:

- Empty input rejected or yields explicit zero depending on signal contract.
- Valid synthetic event emits expected vector length.
- Bad dimensions fail fast.

### Milestone 4 - Training Corpus Export

Deliver:

- `LearningTrainingRecord` or versioned extension of `TrainingRecord`.
- Export path with include/exclude vector options.
- Shape metadata in MCP response.

FSV:

- Persist one complete record.
- Read from RocksDB.
- Export JSON.
- Verify shapes and exact values.

### Milestone 5 - Evaluation And Falsification

Deliver:

- Offline evaluation harness.
- Synthetic event generator.
- Falsification scenarios:
  - high surprise but low utility,
  - high coherence but no transfer,
  - consolidation that loses task-critical detail,
  - delta-E caused by irrelevant novelty.

FSV:

- Each falsification scenario must produce the expected failure/low-score outcome and persist an audit row.

## Done Criteria

The UTL pipeline is not done until:

- Every persisted field has a source-of-truth readback test.
- Every new vector has dimension tests and invalid-dimension tests.
- Every MCP/CLI output includes shape metadata.
- Every model-derived scalar can be traced back to persisted inputs.
- The live store can be audited without relying on function return values alone.

