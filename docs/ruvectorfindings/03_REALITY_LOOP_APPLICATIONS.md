# 03 — Reality-Loop Applications (ccreality)

Concrete transplants from the RuVector workspace to ccreality. Most of these are dramatically less work than the agentdb equivalents because RuVector's already in Rust. Effort is "path-dep" or "vendor" rather than "port."

References to ccreality docs use `docs/ccreality/` paths. Path discipline (CLAUDE.md §8) is honored throughout. Adoption decisions land in `./memory/decisions/` per CLAUDE.md §6.

---

## 1. Path-dep `ruvector-solver` for sublinear graph algorithms

**Source crate:** `ruvector/crates/ruvector-solver/` — 18-file Rust crate with `BMSSP`, `CG`, `Neumann`, `ForwardPush`, `BackwardPush`, `TRUE`, `RandomWalk`, plus router + audit + budget + SIMD.

**ccreality use cases:**

### 1a. Personalized PageRank over the failure-tag graph
When the optimizer asks "which past attempts most influenced today's failure mode?", PPR over the (attempt-tag) bipartite graph gives a principled influence ranking — better than cosine similarity over critique embeddings alone. Forward Push converges quickly even on sparse graphs.

```rust
use ruvector_solver::{forward_push::ForwardPushSolver, types::ComputeBudget, traits::Solver};

let solver = ForwardPushSolver::new(/* alpha */ 0.15, /* threshold */ 1e-4);
let importance = solver.solve(&attempt_graph, &seed_vec, &ComputeBudget::default())?;
// importance[i] = PPR mass on attempt i, given seed = current failing attempt
```

### 1b. BMSSP multigrid for Laplacian solve over the harness-transition history
Every `harness-transitions/round-NN.json` is an edge in a tracked graph. Computing Cheeger-cut style coherence ("how much has the harness drifted?") wants a Laplacian solve. BMSSP gives O(nnz·log n).

### 1c. Audit trail for every solver call
`ruvector-solver` writes its own audit log (`src/audit.rs`). When integrated into the optimizer's recommendation, the audit becomes a verifiable trace of what the optimizer asked the solver to compute.

**ccreality changes:**
- Add `ruvector-solver = { path = "../ruvector/crates/ruvector-solver" }` (or vendor) to `crates/context-graph-cli/Cargo.toml`.
- New module `crates/context-graph-cli/src/optimizer/solver.rs` — wraps `ruvector-solver` + provides ccreality-specific graph-builder helpers.
- New MCP tool `mcp__cgreality__optimizer_compute_influence(failure_tag, k)` — runs PPR, returns top-k most-influential past attempts.

**Effect on throughput:** when the diagnostician needs "which historical recommendations are most relevant to this failure?", the answer goes from cosine-only ranking to PPR-graph-aware ranking. Empirically PPR over (failure_tag, recommendation, attempt) gives more useful candidates than cosine.

**Effort:** 1 day (path-dep + wrapper module).

**Risk:** low. Solver is well-tested. The graph-construction is the only ccreality-specific code.

**CLAUDE.md §11.4 anti-overfit guard:** PPR is universal. The graph it operates on is what matters; build the graph from generic features (failure_tag, repo_family, recommendation_uplift) — never task-specific names.

---

## 2. Vendor `crates/rvf/rvf-crypto` for the witness chain over the optimizer's decision stream

**Source crate:** `ruvector/crates/rvf/rvf-crypto/` — ~700 LOC, no_std, ed25519 + ML-DSA-65 + SLH-DSA-128s + 73-byte SHAKE-256 witness chain. The agentdb findings doc proposed re-implementing this; here you can vendor it directly.

**ccreality changes:**

### 2a. Vendor the crate

Because `crates/rvf/*` is in workspace `exclude`, you can either:
- **Vendor** — copy `rvf-crypto/` and `rvf-types/` into `crates/context-graph-cli/vendor/` with attribution comments + the MIT LICENSE preserved. Single commit, no upstream version drift.
- **Path-dep** — `rvf-types = { path = "../ruvector/crates/rvf/rvf-types" }`, `rvf-crypto = { path = "../ruvector/crates/rvf/rvf-crypto" }`. Tracks upstream automatically; risks build breaks on upstream churn.

Recommend vendor for a stable optimizer surface.

### 2b. Wire the witness chain

Append to `<runtime_root>/claude-code-optimizer/witness-chain.bin` on every `optimizer_record_*` MCP call:

```rust
use rvf_crypto::witness::{create_witness_chain, WitnessEntry};

fn append_to_witness_chain(
    chain_path: &Path,
    op_type: WitnessOpType,
    payload_hash: [u8; 32],
) -> Result<()> {
    let prev_hash = read_last_chain_hash(chain_path)?;
    let entry = WitnessEntry {
        prev_hash,
        action_hash: payload_hash,
        timestamp_ns: now_ns(),
        witness_type: op_type as u8,
    };
    let new_segment = create_witness_chain(&[entry]);
    append_bytes(chain_path, &new_segment)
}
```

`op_type`s:
- `0x01` PROVENANCE (recommendation cited a source-of-truth file)
- `0x02` COMPUTATION (optimizer ran a solver / computed a score)
- `0x03` MUTATION (harness was edited)
- `0x04` DECISION (optimizer recorded a structured decision)

### 2c. New MCP tools

- `mcp__cgreality__optimizer_witness_chain_verify` — replay chain, recompute hashes, return `{valid, entries: u64, last_op_type}`.
- `mcp__cgreality__optimizer_witness_chain_diff(since_offset)` — list new entries since a given offset.

### 2d. Optional: ed25519 segment signing

For higher-stakes audit, sign each emitted segment with the optimizer's ed25519 keypair. Public key in `<runtime_root>/claude-code-optimizer/optimizer.pub`; private key never written to disk (re-derived from operator-supplied seed).

**Effect on FSV:** the existing FSV protocol (`docs/ccreality/18_FSV_PLAN.md`) wants every persisted decision to be independently re-verifiable. Witness chain gives that *cryptographically* in one tool call.

**Effort:** 1 day vendor + 0.5 day witness append + 0.5 day verifier MCP. Total ~2 days.

**Risk:** low. Cryptographically sound implementation already in production at RuVector.

**Cross-reference:** `docs/agentdbfindings/03_REALITY_LOOP_APPLICATIONS.md §5` proposed re-implementing this; here it's a vendor.

---

## 3. Vendor `sona` for trajectory-based learning over the optimizer's decision stream

**Source crate:** `ruvector/crates/sona/` — Rust-native ReasoningBank + EWC++ + micro-LoRA + REINFORCE trajectory + 3 learning loops + federated coordinator + safetensors export. Production-grade.

**ccreality use case:**

The optimizer makes ~6 categorical decisions per turn (per `docs/agentdbfindings/03 §2`). Each decision has a context, an action, and an eventual reward (the inner-actor's `correctness` score). This is exactly the data flow SONA's `QueryTrajectory` captures.

```rust
use sona::{SonaEngine, SonaConfig, EwcConfig};

let engine = SonaEngine::new(SonaConfig {
    hidden_dim: 1024,
    embedding_dim: 1024,
    ..Default::default()
})?;

// At turn start: open a trajectory keyed by current failure embedding
let mut traj = engine.begin_trajectory(failure_embedding.to_vec());

// For each optimizer decision in this turn
for (decision_ctx, decision_arm) in turn_decisions {
    let activations = encode_decision(decision_arm);  // your encoder
    let immediate_reward = score_against_baseline(decision_arm);
    traj.add_step(activations, /* context_ids */ vec![], immediate_reward);
}

// At turn end: feedback
let final_quality = inner_actor_correctness;  // 0.0-1.0
engine.end_trajectory(traj, final_quality);

// Per-turn or every N turns: instant-loop apply
let mut output = vec![0.0; 1024];
engine.apply_micro_lora(&decision_logits, &mut output);
// output now contains learning-influenced decision logits
```

### 3a. EWC++ prevents catastrophic forgetting across turns
The optimizer's policy is meant to *generalize* across the 300 SWE-bench Lite tasks. Without EWC++, fine-tuning on later tasks would forget what worked on earlier tasks. SONA's automatic task-boundary detection + multi-task circular buffer is the right primitive.

### 3b. ReasoningBank surfaces stable patterns
After enough turns, `sona::ReasoningBank::extract_patterns()` returns clusters of "this trajectory shape consistently produces high quality" — which is exactly the (failure_pattern → harness_fix) hypothesis the optimizer wants surfaced.

### 3c. Background loop runs offline
The optimizer's foreground turn doesn't need to compute gradients. The background loop in `sona::loops::BackgroundLoop` runs on its own thread and consolidates trajectories into LoRA weight updates while the next turn happens.

**ccreality changes:**
- Vendor `crates/sona/` (or path-dep). Note: `sona` depends on `time_compat.rs` which has a no_std variant; you can build it with `std`.
- New module `crates/context-graph-cli/src/optimizer/learning.rs` — wraps SONA, exposes a turn-scoped trajectory API.
- Persist SONA state to `<runtime_root>/claude-code-optimizer/sona.safetensors` between turns (uses `sona::export::SafeTensorsExporter`).
- New MCP tool `mcp__cgreality__optimizer_apply_learning(decision_ctx, candidate_arms)` — returns a re-ranked list under SONA's micro-LoRA influence.

**Effect on the bandit story:** this *deepens* the `SolverBandit` proposal. The bandit picks arms; SONA's trajectory recording + REINFORCE gradient estimation gives a richer reward signal that re-weights bandit priors over time. Bandit + SONA is strictly better than bandit alone.

**Effort:** 2-3 days vendor + integration. Most of it is wiring; SONA's API is small.

**Risk:** medium. SONA is research-grade; verify EWC++ task-boundary detection actually fires on your decision-stream pattern. Run with `RUST_LOG=sona=debug` for the first 50 turns.

**Cross-reference:** `docs/agentdbfindings/03 §2` proposed a tiny SolverBandit. SONA is the *fuller* version; the bandit becomes the routing primitive on top of SONA's learning signal.

---

## 4. Path-dep `ruvector-core` for `mmr`, `matryoshka`, `graph_rag`, `conformal_prediction`

**Source paths:** `ruvector/crates/ruvector-core/src/advanced_features/{mmr.rs, matryoshka.rs, graph_rag.rs, conformal_prediction.rs}`.

You don't need *all* of `ruvector-core`. You need its pieces. The crate is organized so each `advanced_features/` module is self-contained.

### 4a. MMR diversity rerank for past-failure queries

Direct replacement for `docs/agentdbfindings/03 §7`'s 50-line port. Path-dep, no port.

### 4b. Matryoshka funnel for the embedder
ContextGraph already has `Matryoshka128` HNSW indexes. RuVector's `matryoshka.rs` adds the **funnel + cascade modes** — coarse-to-fine search where you start at 128D, prune to top-100, then re-rank with full 1024D. ~3-5× speedup at minimal recall loss.

```rust
use ruvector_core::advanced_features::matryoshka::{MatryoshkaSearch, FunnelConfig};

let funnel = MatryoshkaSearch::funnel(FunnelConfig {
    coarse_dim: 128,
    fine_dim: 1024,
    coarse_k: 100,
    fine_k: 10,
});
let results = funnel.search(&query, &index_128, &index_1024)?;
```

For the optimizer's "find top-10 most-similar past failures" tool, this is a strict win.

### 4c. Graph RAG (Leiden community detection) for cross-task pattern surfacing

The 300 SWE-bench Lite tasks form a graph (shared failure modes). Leiden community detection gives natural clusters. When the optimizer asks "show me a global summary of failure patterns across all astropy tasks," `graph_rag.rs::Community::summary` is exactly that.

The 30-60% improvement on multi-hop queries (their measured number on document corpora) translates roughly: when the optimizer asks "find tasks that share both an import-error and a fixture-issue," global Graph RAG out-performs naive co-occurrence ranking.

### 4d. Conformal prediction for "how confident is the recommender?"

Currently the optimizer's recommendations have implicit confidence (the prose says "I think this will help"). Conformal prediction lets you attach a **calibrated confidence interval**:

```rust
use ruvector_core::advanced_features::conformal_prediction::{
    ConformalPredictor, ConformalConfig, NonconformityMeasure
};

let mut cp = ConformalPredictor::new(ConformalConfig {
    alpha: 0.1,                                 // 90% coverage
    calibration_fraction: 0.2,
    nonconformity_measure: NonconformityMeasure::NormalizedDistance,
})?;

// Calibrate with past (recommendation, was-it-correct?) pairs
cp.calibrate(&calibration_set);

// At recommendation time, get a prediction set
let prediction_set = cp.predict_set(&candidate_recommendations, &query)?;
// prediction_set.confidence == 0.9
// prediction_set.results == those recommendations whose
//   non-conformity is below the calibrated threshold
```

The `optimizer_record_recommendation` MCP tool can return `{recommendation_id, confidence_interval: 0.9, alternatives_within_set: [...]}` instead of a single point estimate.

**ccreality changes:**
- `ruvector-core = { path = "../ruvector/crates/ruvector-core", default-features = false, features = ["mmr", "matryoshka", "graph-rag", "conformal"] }` (assuming the upstream gates these).
- New MCP tool: `mcp__cgreality__attempts_query_with_diversity(failure_summary, k, lambda)` — uses `mmr.rs`.
- New MCP tool: `mcp__cgreality__attempts_global_patterns(repo_family)` — uses `graph_rag.rs::Community`.
- `optimizer_record_recommendation` extension: optional `confidence_interval` field — uses `conformal_prediction.rs`.

**Effect:** richer retrieval, calibrated confidence, principled global summarization. Each is a 1-day wire.

**Effort:** 1 day each, ~4 days total.

**Risk:** low. All read-only or additive.

---

## 5. Path-dep `ruvector-temporal-tensor` for cold attempt-summary storage

**Source crate:** `ruvector/crates/ruvector-temporal-tensor/`.

The reality-loop runtime root accumulates attempt summaries indefinitely (every attempt × every task × every harness-transition = many KB of JSON + embeddings). Hot-tier raw f32 embeddings, cold-tier 3-bit quantized. The compressor is zero-deps and WASM-compatible.

```rust
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

let mut comp = TemporalTensorCompressor::new(
    TierPolicy::access_pattern_driven(),
    /* tensor_dim */ 1024,
    /* segment_id */ 0,
);

for embedding_frame in attempt_summary_embeddings {
    let mut segment = Vec::new();
    comp.push_frame(&embedding_frame, attempt_id, &mut segment);
    if !segment.is_empty() {
        write_segment_to_runtime_root(&segment)?;
    }
}
```

Tiers (per `crates/ruvector-temporal-tensor/src/lib.rs:13-24`):
- Hot 8-bit: ~4× saving — current attempts
- Warm 7-bit: ~4.57× saving — last week
- Warm 5-bit: ~6.4× saving — last month
- Cold 3-bit: ~10.67× saving — older

Access decoder:
```rust
let frame_back: Vec<f32> = comp.read_frame(segment, frame_idx)?;
```

**Effect:** 4-10× storage reduction for attempt embeddings. After 300 task suite × 5 attempts each × 1024D × 4B/value = ~6MB of raw embeddings. With tiering, ~1.5 MB. Trivial absolutely; meaningful if you ever scale to 10K attempt-runs.

**Effort:** 1 day path-dep + tier policy config.

**Risk:** low. The compressor is documented as "zero deps."

---

## 6. Path-dep `ruvector-mincut` for "split tasks into independent training/eval shards"

**Source crate:** `ruvector/crates/ruvector-mincut/`.

ccreality use case: when the NightlyLearner-style consolidation pipeline (`docs/agentdbfindings/03 §6`) wants to A/B test a candidate harness change, it needs to split tasks into treatment vs control without contamination.

A *random* split risks high variance — by chance, treatment gets all the easy tasks. A *mincut* split minimizes the cross-set edge weight (where edges = "these tasks share a failure mode") — both halves end up balanced.

```rust
use ruvector_mincut::canonical::CanonicalMincut;

let cut = CanonicalMincut::new();
let (treatment, control) = cut.bipartition(&task_failure_graph)?;
// treatment ∪ control == all tasks; |treatment| ≈ |control|;
// edge weight crossing the cut is minimized
```

**Effect:** cleaner A/B comparisons of harness changes. Lower variance in uplift estimates → fewer false-negative rejections of good changes.

**Effort:** 1 day path-dep + graph builder for `task_failure_graph`.

**Risk:** medium. The graph construction is the nontrivial part — what edge weights mean (shared tags, shared embeddings, shared repos, etc.) is your design choice and affects whether the cut is meaningful.

---

## 7. Path-dep `ruvector-router-core` + `ruvector-tiny-dancer-core` for sub-10ms intent routing

**Source crates:** `ruvector/crates/ruvector-router-core/`, `ruvector/crates/ruvector-tiny-dancer-core/`.

Currently ccreality routes via 4 search strategies × weight profiles, evaluated on every retrieval. That's flexible but slow (~10-50ms per retrieval).

For tools that don't need full strategy evaluation (e.g. `attempts_query_reflexion`), a FastGRNN-based intent router classifies the query in <10ms and dispatches. ContextGraph already has weight profiles; this just adds a faster *dispatcher*.

**Use cases:**
- "Is this a failure-mode query, a success-pattern query, or a global-summary query?" → 3-arm router → dispatch to the right sub-tool.
- "Is this an exact-recommendation-id lookup or a semantic search?" → 2-arm router → dispatch to direct lookup vs HNSW.

**Effort:** 2 days path-dep + train tiny-dancer on labeled examples (you'd hand-label ~100 example queries as a calibration set).

**Risk:** medium. If the router is wrong, queries route to the wrong sub-tool; circuit breaker is in `tiny-dancer-core/src/circuit_breaker.rs` — wires fall-back to the slow path on routing-error rate.

**Verdict:** quality-of-life, not load-bearing. Low priority.

---

## 8. Vendor `prime-radiant`'s SONA tuning loop for self-tuning the optimizer's hyperparameters

**Source crate:** `ruvector/crates/prime-radiant/src/sona_tuning/`.

SONA has knobs (μLoRA rank, EWC lambda, learning rate). Prime-radiant's `sona_tuning/` module has a meta-loop that tunes SONA's hyperparameters against a coherence loss. Drop SONA into ccreality (per §3 above) and prime-radiant's tuning loop becomes "the optimizer's optimizer."

```rust
// Pseudocode of what's in prime-radiant
loop_period(every_50_turns) {
    let coherence_before = measure_coherence(&decision_stream);
    let proposed_hparams = tune_step(&sona_engine, coherence_before);
    let coherence_after = simulate_with(&proposed_hparams);
    if coherence_after > coherence_before + min_delta {
        apply(proposed_hparams);
        record_witness(WitnessOpType::TuningCommit);
    }
}
```

**Effort:** 3 days. Specific to `prime-radiant`'s coherence definition; you may want to substitute "inner-actor correctness rate over the last 50 attempts" as your coherence proxy.

**Risk:** medium-high. Self-tuning can oscillate. Cap tune frequency (every 50 turns is probably the floor).

**Verdict:** medium-priority, after §3 (SONA) is stable.

---

## 9. Read `ruvector-graph-transformer/temporal.rs` for Granger causality ideas

**Source path:** `ruvector/crates/ruvector-graph-transformer/src/temporal.rs`.

Granger causality test: did harness-transition X cause subsequent score-improvement Y, controlling for prior trajectory? Standard frequentist time-series test. The crate has it as an attention-layer feature, but the test itself is a few hundred LOC of correlation + lagged regression.

**Use case:** "Did recommendation #34 actually cause the score uplift on tasks Y, Z, W, or did the uplift come from another concurrent change?"

**Verdict:** read; don't path-dep the whole graph-transformer crate just for this. Implement the test in `crates/context-graph-cli/src/optimizer/granger.rs` — you'll write ~300 LOC.

**Effort:** 2 days (port + tests).

**Risk:** medium. Granger is sensitive to time-window choice; verify against synthetic data before trusting on real harness-transition deltas.

---

## 10. Read `ruvllm_retrieval_diffusion` for "training-free retrieval-LM" idea

**Source crate:** `ruvector/crates/ruvllm_retrieval_diffusion/`.

For ccreality, this isn't a direct transplant — the optimizer is opus, not a retrieval-LM. But the *idea* — "given a corpus, retrieve next tokens via associative memory + masked diffusion, no training" — could apply to a sub-task: "given the corpus of past optimizer recommendations, generate a next-token candidate for the *body* of a new recommendation."

This is speculative. Worth keeping in mind as a future research direction, not a near-term transplant.

---

## Priority order — pick from the top

1. **§2 (rvf-crypto witness chain)** — vendor today; ~2 days; load-bearing for FSV.
2. **§1 (ruvector-solver PPR + BMSSP)** — path-dep; ~1 day; powers influence ranking and Laplacian solves.
3. **§3 (sona for trajectory learning)** — vendor; ~3 days; deepens the SolverBandit story.
4. **§4 (mmr + matryoshka + graph_rag + conformal_prediction)** — path-dep selectively; ~4 days; richer retrieval + calibrated confidence.
5. **§5 (temporal-tensor cold storage)** — path-dep; ~1 day; storage cost reduction.
6. **§6 (mincut for A/B splits)** — path-dep; ~1 day; cleaner uplift estimation.
7. **§9 (Granger causality)** — port the test; ~2 days; principled "did X cause Y?" attribution.
8. §7 (tiny-dancer routing) — quality-of-life; defer.
9. §8 (prime-radiant SONA tuning) — defer until §3 is stable.
10. §10 (retrieval-diffusion) — research direction.

**Total Tier 1 (highest priority, §1 + §2 + §3): ~6 days.** All vendor or path-dep. Adds: witness chain, sublinear solvers, trajectory-based learning. After this sprint the optimizer has cryptographic audit + sublinear-PPR-aware diagnosis + REINFORCE-trajectory learning + EWC++ memory.

**Tier 2 (§4 + §5 + §6 + §9): ~8 days.** Quality-of-life that adds up.

---

## Compared to the agentdb proposal

The agentdb findings recommended ~10-12 days of *Rust port* work for Tier 1+2. RuVector findings recommend ~14 days of *vendor + path-dep* work for the equivalent capability — **but the capability is dramatically richer**:

| What agentdb proposes | What RuVector lets you do instead |
|------------------------|-----------------------------------|
| Port 270-LOC SolverBandit (Thompson Sampling) | Vendor `sona` (full ReasoningBank + EWC++ + REINFORCE + 3 loops + federated) AND `ruvector-solver` (8 algos, witness audit, SIMD) |
| Port 250-LOC SHAKE-256 73-byte witness chain | Vendor `rvf-crypto` (no_std, includes ed25519 + ML-DSA-65 + SLH-DSA-128s + lineage + attestation) |
| Port 50-LOC MMR | Path-dep `ruvector-core::advanced_features::mmr` (production-tuned) |
| Port 300-LOC NightlyLearner doubly-robust uplift | Path-dep `ruvector-mincut` for A/B splits + port the Granger test from `graph-transformer/temporal.rs` |
| Port 250-LOC ContextSynthesizer narrative | Optional — agentdb's narrative was unique to TS; for ccreality reuse the synthesizer pattern unchanged in Rust |
| Add Reflexion `critique` field | Same — schema-level, no transplant needed |

**Net:** RuVector replaces "port 1500 LOC" with "vendor 1 sub-crate + 4 path-deps + 300 LOC of glue." Roughly half the effort for substantially more capability.

The trap: vendoring `sona` and `rvf-crypto` ties ContextGraph to RuVector's release cadence + license. See `06_RISKS_AND_NON_TRANSFERABLE.md`.

---

## Cross-references

- `docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md` — where new tools fit.
- `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md` — schema extensions for §2/§4d.
- `docs/agentdbfindings/03_REALITY_LOOP_APPLICATIONS.md` — same primitives at the *idea* level; this doc is the *implementation* level.
- `docs/ruvectorfindings/06_RISKS_AND_NON_TRANSFERABLE.md` — vendor-vs-path-dep tradeoffs.
- `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` — sprint-level effort triage.
