# AI Learning As UTL Embedder Implementation Plan

## Current Verified Base

The system now has 14 production embedders:

- E1 semantic dense.
- E2 temporal recency.
- E3 temporal periodicity.
- E4 temporal position/sequence.
- E5 causal asymmetric.
- E6 sparse lexical.
- E7 code.
- E8 graph asymmetric.
- E9 HDC / typo pattern.
- E10 contextual/paraphrase asymmetric.
- E11 entity.
- E12 late interaction / token-level.
- E13 SPLADE.
- E14 BGE-M3 Dense multilingual/long-context.

The UTL pipeline can safely assume:

- Topic profile shape: 14D.
- Per-embedder score arrays: 14D.
- Cross-correlations: 91D.
- Training records: version 3, includes E14.
- Constellations: version 2, includes 91D cross-correlation centroid.

## Learning-as-UTL Target

Use UTL as the organizing objective for both humans and AI:

```text
Learning utility = useful state transition under constraints
```

The system should learn from:

- Delta-E: change in human/agent epistemic state.
- Surprise: prediction error or novelty relative to context.
- Coherence: reduction of contradiction / increase in usable integration.
- Consolidation: durable reuse, abstraction, and compression.
- Transfer: whether learned structure helps in new tasks/domains.

## New Embedder Families Needed

### U1 Human State Delta Embedder

Purpose:

- Estimate before/after epistemic movement from interaction traces.

Inputs:

- User query.
- Retrieved context.
- Assistant response.
- Follow-up user behavior.
- Optional explicit ratings or corrections.

Output:

- `delta_e_vector`: dense state-change vector.
- `delta_e_scalar`: signed utility change.
- Attribution to contributing embedders.

Source of truth:

- Training record extension or separate `CF_LEARNING_EVENTS`.

### U2 Surprise Embedder

Purpose:

- Encode prediction error: what was unexpected but useful.

Inputs:

- Query/context pair.
- Retrieval results and rank positions.
- Model uncertainty or disagreement across embedders.

Output:

- Surprise vector.
- Novelty score.
- Productive-vs-noisy surprise label.

Source of truth:

- Per-memory surprise annotations, plus event-level surprise deltas.

### U3 Coherence Embedder

Purpose:

- Measure whether a memory/response integrates with existing graph structure.

Inputs:

- Existing graph neighbors.
- Typed edges.
- Contradiction/entailment signals.
- Constellation membership.

Output:

- Coherence vector.
- Contradiction pressure.
- Integration confidence.

Source of truth:

- Graph-link edge annotations and constellation deltas.

### U4 Consolidation Embedder

Purpose:

- Identify memories ready for abstraction, summarization, or chunk merge.

Inputs:

- Access/reuse history.
- Topic stability.
- Recurring typed-edge motifs.
- Cross-session recurrence.

Output:

- Consolidation readiness.
- Proposed abstraction target.
- Compression safety score.

Source of truth:

- Consolidation candidate table and post-merge audit records.

### U5 Transfer Embedder

Purpose:

- Detect whether knowledge learned in one context improves retrieval/reasoning in another.

Inputs:

- Source/target tasks.
- Search success deltas.
- Graph path reuse.
- Cross-domain topic movement.

Output:

- Transfer vector.
- Source-domain and target-domain tags.
- Transfer confidence.

Source of truth:

- Transfer event table linked to memory ids, task ids, and outcome metrics.

## Implementation Phases

### Phase 1 - Schema And Event Capture

Add:

- `LearningEvent` core type.
- `CF_LEARNING_EVENTS`.
- Event ids linked to memory ids, session ids, and response ids.
- Before/after state snapshots using current 14D topic profiles and 91D correlations.

FSV requirement:

- Write one synthetic learning event.
- Close/reopen RocksDB.
- Read the event and verify ids, 14D profile, 91D correlations, and outcome labels.

### Phase 2 - Derived UTL Features

Add deterministic feature computation:

- Delta-E from before/after topic profiles.
- Surprise from retrieval rank shifts and embedder disagreement.
- Coherence from graph/constellation consistency.
- Consolidation from recurrence and stability.
- Transfer from cross-domain reuse.

FSV requirement:

- Use synthetic cases with known expected outcomes:
  - no state change -> delta 0.
  - contradiction edge -> lower coherence.
  - repeated stable memory -> higher consolidation.
  - cross-domain successful reuse -> positive transfer.

### Phase 3 - Model/Embedder Interfaces

Add UTL embedder trait:

```rust
trait LearningSignalEmbedder {
    fn signal_id(&self) -> LearningSignalId;
    fn dimension(&self) -> usize;
    async fn embed_event(&self, event: &LearningEvent) -> Result<LearningSignal>;
}
```

Keep these separate from E1-E14 until the storage/query impact is proven. Do not increase `NUM_EMBEDDERS` for UTL signals prematurely.

### Phase 4 - Training Export

Extend `TrainingRecord` or create `LearningTrainingRecord`:

- 14D topic profile.
- 91D cross-correlations.
- UTL event signals.
- Before/after state.
- Outcome labels.

FSV requirement:

- Persist a record with all UTL fields.
- Read it back from RocksDB.
- Export JSON and verify exact shape.

### Phase 5 - Online Evaluation

Add metrics:

- Retrieval utility uplift.
- Correction reduction.
- Follow-up coherence.
- Cross-session reuse.
- Human-state movement proxy.

FSV requirement:

- Synthetic A/B event stream where one route has known higher utility.
- System must rank the higher utility route above baseline and persist the metric.

## Non-Negotiable Guardrails

- Never overload E1-E14 semantic embedders with UTL event semantics unless storage/search semantics are explicitly versioned.
- Every new persisted vector/table needs a version byte and a migration/re-export story.
- Every new source of truth must have:
  - happy path write/read/reopen test,
  - empty input test,
  - invalid dimension/format test,
  - maximum/large input test.
- Every UTL scalar must be explainable from persisted event fields.

