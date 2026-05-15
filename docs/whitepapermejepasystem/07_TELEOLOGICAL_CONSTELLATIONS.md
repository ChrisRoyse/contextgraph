# 07 — Teleological Constellations & Reward Signal Density

This is the heart of the training data strategy. The corpus is large (4.9 M vector records) but the *labels* are sparse — only 2,400 (instance, mutation) cells have a direct Docker oracle outcome. The work of teleological constellations is to **extract reward signals from durable reality at maximum density** so the predictor has enough supervision to converge.

## Definition

A **teleological constellation** is an intent-linked chain through the durable-reality graph:

```
operator goal
   → agent claim
      → edit event (typed AST delta + SHA chain)
         → embedded chunk constellation (12 embedders × N chunks)
            → frozen instrument panel (15 slots)
               → oracle outcome (Pass / Fail / Exception type)
                  → operator feedback (Confirmed / Refuted / Inconclusive)
```

Each arrow is a deterministic transformation that can be witnessed and replayed. The chain is the unit of *intent grounding*: the predictor doesn't just learn "does this chunk pass tests" — it learns "does the *entire chain* from operator-goal to oracle-outcome move durable reality in the direction the operator wanted."

The "constellation" part: groups of chunks that participate in the same intent-linked chain are clustered into cells. The cell's centroid + variance is the per-cell signal the predictor calibrates against.

## Cells

A **cell** is a row of `CF_MEJEPA_TCT_CELLS`:

```rust
pub struct TctCell {
    pub cell_id: CellId,                  // sha256 of cell-defining keys
    pub mutation_category: MutationCategory,  // known_good, subtle_flip, off_by_one, ...
    pub language: Language,               // Python, Rust, TS, ...
    pub project: ProjectId,               // django, sympy, matplotlib, ...
    pub member_chunks: Vec<ChunkId>,
    pub centroid_per_embedder: HashMap<EmbedderId, Vec<f32>>,
    pub variance_per_embedder: HashMap<EmbedderId, f32>,
    pub oracle_outcome_distribution: OracleDistribution,
    pub last_refresh: Timestamp,
    pub gτ_threshold: f32,                // per-cell Gτ guard threshold
}
```

Cell keys are typically `(mutation_category, language, project, [optional: AST node kind])`. The 300 × 8 = 2,400 base cells in the SWE-bench Lite Python corpus are the seed. The system can refine cells over time (sub-divide based on observed reward density).

## The N² reward signal explosion

Per `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §1.1`, each chunk produces **N · (N + N²) ≈ 462 reward signals at N=21**. Even with our 12 active embedders, the math is:

| Component | Count at N=12 | Formula |
|---|---|---|
| Per-embedder cosine to cell centroid | 12 | N |
| Strict-upper-triangle pairwise cosine across embedders | 66 | N(N-1)/2 |
| Strict-upper-triangle pairwise MI across embedders | 66 | N(N-1)/2 |
| Blind-spot z-scores (primary vs enhancer pairs) | 11 | N-1 |
| **Total per chunk** | **155** | — |

At N=21 (full embedder slot count if we light up all 21 including learner-state) we'd hit 462. At N=12 (current Python corpus) we get **155 reward signals per chunk × 399,777 chunks = ~62 M reward signals in the seed corpus alone**.

That's the order-of-magnitude difference between "labeled corpus" (2,400 cells with oracle outcome) and "reward-dense corpus" (62 M derived signals per shard, summed across the chain).

## What each reward signal measures

### Per-embedder cosine to cell centroid
For each chunk, for each embedder, compute `cosine(chunk_vec_embedder, cell_centroid_embedder)`. This says: "how typical is this chunk for its cell, under this embedder's view?"

Used in:
- Q1 reconciliation (a typical chunk is more likely to match the claim).
- Q2 verdict confidence (a chunk far from its cell's centroid is more likely to fail oracle).
- OOD detection (a chunk with no centroid close enough to it is out-of-distribution).

### Pairwise cosine across embedders (66 signals)
For each unordered pair (E_i, E_j), `cosine(chunk_vec_i, chunk_vec_j)`. This says: "do these two embedders agree about how this chunk relates to its cell?"

Disagreement is the strong signal: when E1 (semantic) says high similarity to centroid but E7 (code) says low similarity, that's an indication the chunk is *semantically typical but structurally unusual* — exactly the kind of pattern that fails oracle.

### Pairwise mutual information across embedders (66 signals)
Computed on the per-cell distributions: `MI(E_i_distribution_in_cell, E_j_distribution_in_cell)`. This is at the cell level, not the chunk level, but each chunk inherits its cell's pairwise MI. High pairwise MI in a cell means the two embedders carry redundant signal there; low MI means independent signal.

Used to weight embedders: when the predictor is making a Q2 verdict, it weights more heavily the embedders that carry the most independent signal for that cell.

### Blind-spot z-scores (N-1 signals)
For each enhancer embedder relative to the primary embedder (E1, semantic), compute a z-score: `(chunk_distance_enhancer - mean) / stddev`. High |z| means the enhancer disagrees with the primary about whether this chunk belongs in this cell. A blind spot.

Blind spots are pulled into the active-learning queue at high priority. The predictor learns from them by routing them to operator review.

## Implementation status

| Component | Status |
|---|---|
| Per-embedder cosine | ✅ shipped (`python_pairwise_dda_mi.py` computes these for the Python corpus) |
| Pairwise cosine | ✅ shipped |
| Pairwise MI | ✅ shipped |
| Blind-spot z-scores | 🟡 partial; the embedder-contribution audit ships per-shard but the per-cell rollup is pending Phase G |
| Cell centroid materialization | 🟡 deferred to Phase G post-corpus |
| Gτ threshold calibration | 🟡 deferred to Phase G post-corpus |
| Live Gτ guard in predictor | 🟡 partial; struct exists in `context-graph-mejepa-tct` crate, full wiring is Phase G |

The forward-cache run on aiwonder is the input that lets all of this materialize. Once the 4.9 M vector records are in place, the constellation builder rolls them into the 2,400 cells, computes the centroids/variances/MIs, and persists to `CF_MEJEPA_TCT_CELLS`.

## Why "teleological"

The Greek root *telos* = end, purpose, intended outcome. A *teleological* chain isn't just "data points correlated with a label" — it's "events linked because they share the same intended outcome." The operator's goal, the agent's claim, the edit event, the oracle outcome — they're all bound by the same *purpose*. When the predictor learns from a constellation, it's learning the *causal* structure of intent → outcome, not just a statistical correlation.

This matters because:
1. It generalizes better. Patterns grounded in causal intent transfer across projects.
2. It's interpretable. The operator sees the chain, not a black-box score.
3. It surfaces hidden goals. When the operator's stated goal and the corpus's revealed reward field disagree, the system can flag that.

## How a cell gets built (concrete walkthrough)

Take cell `(subtle_flip, Python, django)`.

1. **Identify member chunks.** All AST chunks from any django task with mutation_category=`subtle_flip`. From the 300×8 SWE-bench Lite corpus: 114 django tasks × 1 mutation = 114 subtle_flip variants. Each variant has ~166 chunks (300×8 → 399,777 chunks total, mean 166 chunks/variant). So this cell has ~18,900 member chunks.

2. **Compute per-embedder centroids.** For each of 12 embedders, mean-pool the 18,900 chunk vectors → 12 centroid vectors.

3. **Compute per-embedder variances.** Sample variance around each centroid.

4. **Compute pairwise MI.** For each (E_i, E_j) of 66 pairs, compute MI on the empirical distribution of `(cosine(chunk_vec_i, centroid_i), cosine(chunk_vec_j, centroid_j))` across the cell's members.

5. **Compute oracle outcome distribution.** Of the 114 subtle_flip variants in django, how many had Docker oracle = Fail? Per CLAUDE.md / corpus design, subtle_flip should be 100% Fail; if it's not, the cell has off-cell members and gets quarantined.

6. **Compute Gτ threshold.** The Gτ guard says: "if a new chunk's distance from this cell's centroid exceeds threshold T, reject the cell membership." T is calibrated so that >95% of true cell members fall inside, with operator-tunable false-rejection rate.

7. **Persist.** Write the cell to `CF_MEJEPA_TCT_CELLS` keyed by cell_id.

## What the predictor does with cells

At inference time:

1. For a new chunk, compute its 12-embedder vectors.
2. Routing: find the K nearest cells in joint-embedding space. (`mejepa_routing_lookup` MCP tool.)
3. Inherit the nearest cells' Gτ thresholds.
4. Compute per-cell distance; apply Gτ.
5. If distance to nearest cell ≤ threshold: predict using that cell's centroid + variance + oracle outcome distribution.
6. If distance > threshold for all K cells: emit `Verdict::OutOfDistribution` or `Verdict::GuardRejected`.

This is how the system stays calibrated. Predictions are anchored in cells; cells are anchored in oracle outcomes; oracle outcomes are anchored in real Docker test runs.

## The "maximum reward signal" optimization

The user's directive: *"make sure we are optimally capturing all teleological constellations to extract maximum reward signals from durable reality."*

Operationally, this means:
1. **Maximize cell coverage** — every (mutation_category × language × project) combination has at least one cell.
2. **Maximize chunk coverage** — every chunk is a member of at least one cell.
3. **Maximize per-chunk reward density** — compute all 155 reward signals for every chunk, not just the cheap ones.
4. **Maximize MI/cosine reuse** — compute pairwise MI once per cell, reuse across all member chunks.
5. **Maximize Gτ calibration density** — re-calibrate thresholds whenever the cell's member set changes significantly.
6. **Maximize operator-override absorption** — every `mejepa_record_agent_feedback` becomes a 6×-weighted training example; that's a high-signal-density reward.

Phase G's per-cell stratification gate is what enforces this. Each `(mutation_category × language)` cell must reach `prediction_oracle_correlation ≥ 0.95` independently. The system isn't allowed to ship by being great on `known_good` and mediocre on `subtle_flip`; every cell pulls its weight.

## Future: cell sub-division

The 2,400 base cells are coarse. As reward density per cell saturates (i.e., predictor stops improving even with more samples), the system sub-divides the cell along the dimension with highest within-cell variance. Example: `(subtle_flip, Python, django)` might split into:
- `(subtle_flip, Python, django, AST_kind=FunctionDef)`
- `(subtle_flip, Python, django, AST_kind=Method)`
- `(subtle_flip, Python, django, AST_kind=Comprehension)`

Each sub-cell gets its own centroid, threshold, and predictor head. This is on the Phase G+ roadmap.

## TL;DR

Teleological constellations are the data-structure that turns 2,400 cell-level oracle labels into ~62 million chunk-level reward signals by:
- Mining embedder disagreement (66 pairwise cosines + 66 pairwise MIs per chunk).
- Anchoring each chunk to the centroid + variance + oracle distribution of its cell.
- Calibrating Gτ thresholds per cell for OOD detection.
- Replaying the full intent-linked chain so the predictor learns causal structure, not statistical correlation.

This is what makes ME-JEPA's training signal *dense enough* to converge on the 0.95 oracle correlation gate per cell.
