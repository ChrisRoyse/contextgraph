# 03 — Reality-Loop Applications (ccreality)

> **🔒 Reference-only — clean-room reimplementation.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — every "Vendor `X`" / "Path-dep `X`" recommendation below is SUPERSEDED. Read each as **"Read RuVector's `X` as inspiration; reimplement the algorithm cleanly in our own crate."** Effort estimates are roughly 2-3× the original numbers — clean-room is more work than path-dep, but the result is fully ours.

> **🎯 Master plan:** [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md) is the canonical end-goal framing — **ME-JEPA-Code is the verifier**, a reality compiler that predicts oracle outcomes from `(code, context)`. Each section below describes a primitive as a *standalone tool for the ccreality optimizer*; in doc 09's framing those same primitives are *inputs/components of ME-JEPA-Code* (the witness chain becomes `E_Witness`, the solver suite powers importance-weighting in mini-batch sampling, sona's μLoRA + EWC++ implements online `observe()`/self-healing, etc.). The "verify_ai_output gates" framing in §3.6 below is SUPERSEDED — gates are input features to ME-JEPA, not standalone gates. **Doc 09 §3.5–§3.8** specifies the optimizer-side reality-shift consumer: every artifact the ccreality harness already produces (`harness-transitions/*.json`, `cgreality-shift-log/*.jsonl`, `attempt-summary.json`, `validation.json`, `inner-harness-response.json`, witness-chain entries, Pre/PostToolUse hook payloads) flows into the `mejepa-shift-subscriber` Tokio task in `context-graph-mcp` which extracts `E_Diff` / `E_Witness` / `E_Reasoning` / `E_Oracle` and calls `mejepa.observe(panel, oracle_outcome)` end-to-end in < 5 s p50 / < 30 s p99 — closing the loop *see edit → predict → observe outcome → update*. When this doc disagrees with doc 09, doc 09 wins.

Concrete reimplementation targets in the ccreality outer-optimizer harness, with each target citing the RuVector source file to read as inspiration.

References to ccreality docs use `docs/ccreality/` paths. Path discipline (CLAUDE.md §8) is honored throughout. Adoption decisions land in `./memory/decisions/` per CLAUDE.md §6.

---

## Current ccreality implementation state (updated 2026-05-08, post commits c18c694e → a9ca688e)

Done:

- `context-graph-witness` exists and is wired to the ccreality optimizer witness file.
- MCP tools `optimizer_witness_chain_verify` and `optimizer_witness_chain_diff` exist.
- Bandit/optimizer writes append witness entries and read back the source-of-truth file.
- `context-graph-solver` exists with weighted Forward-Push PPR + CG + CSR validation and manual FSV evidence.
- **`optimizer_compute_influence` MCP tool shipped** (c18c694e, `influence.rs`): Forward-Push PPR over attempt-influence graph; produces ranked influence list + persisted audit row; witness-chain preflight before write.
- **`optimizer_recall_recommendations` MMR rerank shipped** (f6ffeaeb, `diversity.rs`): MMR(λ, normalized(α·similarity + β·uplift − γ·latency_cost), token-Jaccard redundancy) over recall candidates; witnessed recall audits via `recommendation_certificate.rs` produce verifiable recall certs.
- **Witness-chain preflight on every write** (bfcc060e): bandit / influence / optimizer writes verify chain integrity *before* appending — the §5.6 nightly chain-integrity check has been promoted to a per-write fail-closed gate.
- **Witness-chain legacy reconciliation + canonical format manifest** (33daf95b): mixed-format chains coexist during the migration; `witness_chain_format.rs` documents the canonical encoding; `witness_chain_legacy.rs` reconciles older entries.
- **Witness-chain repair tooling + FSV** (`witness_chain_repair.rs`, `witness_chain_repair_fsv.rs`): when chain integrity fails (e.g., `CCREALITY_WITNESS_PREV_HASH_MISMATCH` at offset 0 from a corrupted first entry), the repair path can reconstruct a valid prefix; integrity-tested by FSV.
- **Atomic `write_text_checked`** (a9ca688e, `witness_chain_io.rs`): durable, no-torn-write semantics for chain appends; underpins ME-JEPA's planned `observe()` writes too.
- **`optimizer_preflight_fsv.rs`** — every preflight verification result is itself a signal (succeeded / failed / repaired) that ME-JEPA can consume as a pipeline-determinism feature.

Pending:

- BMSSP/multigrid is not implemented (intentionally — no fake wrappers).
- ME-JEPA-Code itself: instruments + panels + predictor + mutation corpus + aux heads + counterfactual training + active-learning loop + `mejepa-shift-subscriber` are 0% started.
- SONA/EWC++ adaptive learner, temporal-tensor tiering, mincut, retrieval diffusion, Granger, verified-training certs are still pending.

---

## 1. Clean-room reimplement a sublinear-solver subset for graph influence ranking

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvector-solver/` — 18-file Rust crate with `BMSSP`, `CG`, `Neumann`, `ForwardPush`, `BackwardPush`, `TRUE`, `RandomWalk`, plus router + audit + budget + SIMD.

**ccreality use cases:**

### 1a. Personalized PageRank over the failure-tag graph
When the optimizer asks "which past attempts most influenced today's failure mode?", PPR over the (attempt-tag) bipartite graph gives a principled influence ranking — better than cosine similarity over critique embeddings alone. Forward Push converges quickly even on sparse graphs.

```rust
use context_graph_solver::{ForwardPushConfig, ForwardPushSolver};

let solver = ForwardPushSolver::new(ForwardPushConfig {
    alpha: 0.15,
    tolerance: 1e-4,
    max_pushes: 1_000_000,
})?;
let report = solver.solve_from_distribution(&attempt_graph, &seed_vec)?;
// importance[i] = PPR mass on attempt i, given seed = current failing attempt
let importance = report.estimate;
```

### 1b. BMSSP multigrid for Laplacian solve over the harness-transition history
Every `harness-transitions/round-NN.json` is an edge in a tracked graph. Computing Cheeger-cut style coherence ("how much has the harness drifted?") wants a Laplacian solve. BMSSP gives O(nnz·log n).

### 1c. Audit trail for every solver call
`ruvector-solver` writes its own audit log (`src/audit.rs`). When integrated into the optimizer's recommendation, the audit becomes a verifiable trace of what the optimizer asked the solver to compute.

**ccreality changes:**
- **Done:** new crate `crates/context-graph-solver/` with our own weighted Forward Push, CG, CSR validation, structured errors, and manual FSV evidence. Header comments cite `ruvnet/RuVector crates/ruvector-solver/src/{types,forward_push,cg}.rs at HEAD ef5274c2` as inspiration; no code copied.
- **Pending:** BMSSP/multigrid. Do not fake this with a CG wrapper.
- **Pending:** new module `crates/context-graph-cli/src/optimizer/influence.rs` — graph-builder helpers + thin wrapper over our `context-graph-solver`.
- **Pending:** new MCP tool `mcp__cgreality__optimizer_compute_influence(failure_tag, k)` — runs PPR, returns top-k most-influential past attempts.

**Effect on throughput:** when the diagnostician needs "which historical recommendations are most relevant to this failure?", the answer goes from cosine-only ranking to PPR-graph-aware ranking. Empirically PPR over (failure_tag, recommendation, attempt) gives more useful candidates than cosine.

**Remaining effort:** 2-4 days. Forward Push + CG foundation is done; remaining work is BMSSP/multigrid plus graph-builder, CLI/MCP wrapper, and persisted solver-call audit evidence.

**Risk:** low. Solver is well-tested. The graph-construction is the only ccreality-specific code.

**CLAUDE.md §11.4 anti-overfit guard:** PPR is universal. The graph it operates on is what matters; build the graph from generic features (failure_tag, repo_family, recommendation_uplift) — never task-specific names.

---

## 2. Clean-room reimplement a witness chain + signature crate, inspired by `crates/rvf/rvf-crypto`

**Inspirational source crate (read-only reference):** `ruvector/crates/rvf/rvf-crypto/` — ~700 LOC, no_std, ed25519 + ML-DSA-65 + SLH-DSA-128s + 73-byte SHAKE-256 witness chain. We read it; we reimplement.

**ccreality changes:**

### 2a. Clean-room reimplement

Per the locked rule, neither vendor nor path-dep is allowed. Build a new crate `crates/context-graph-witness/` with:
- Our own `WitnessEntry` struct (same 73-byte layout — `prev_hash 32 + action_hash 32 + timestamp_ns 8 + witness_type 1`).
- Our own `create_witness_chain` and `verify_witness_chain` functions.
- Our own SHAKE-256 wrapper (or use `sha3` crate directly — `cargo add sha3` is fine, that's an upstream-of-RuVector public Rust crate, not part of the locked rule).
- Optional `ed25519` feature using `ed25519-dalek` (again, public Rust crate).

Each file's header comment cites `ruvnet/RuVector crates/rvf/rvf-crypto/src/<file>.rs at HEAD ef5274c2 (read 2026-05-08)` as inspiration; no code copied.

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

**Current status:** chain core, append/readback behavior, and ccreality verifier/diff tooling exist through `crates/context-graph-witness/` and the optimizer witness-chain module. ed25519 segment signing is still pending.

**Remaining effort:** ~1-2 days for ed25519 segment signing, key handling, and FSV coverage around signed segments.

**Risk:** low. The cryptographic shape is additive, but any signing failure must fail closed and identify the broken segment/key material explicitly.

**Cross-reference:** `docs/agentdbfindings/03_REALITY_LOOP_APPLICATIONS.md §5` proposed re-implementing this; the chain core now exists as our own `context-graph-witness` crate.

---

## 3. Clean-room reimplement a SONA-style adaptive learner

**Inspirational source crate (read-only reference):** `ruvector/crates/sona/` — Rust-native ReasoningBank + EWC++ + micro-LoRA + REINFORCE trajectory + 3 learning loops + federated coordinator + safetensors export. Production-grade.

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
- New crate `crates/context-graph-sona/` containing our own clean-room implementation of: `SonaEngine`, `MicroLoRA`, `BaseLoRA`, `EwcPlusPlus`, `TrajectoryBuilder`, `ReasoningBank`, `LoopCoordinator`. Header comment cites `ruvnet/RuVector crates/sona/src/{engine,lora,ewc,trajectory,reasoning_bank,loops}.rs at HEAD ef5274c2` as inspiration; no code copied.
- New module `crates/context-graph-cli/src/optimizer/learning.rs` — wraps our `context-graph-sona`, exposes a turn-scoped trajectory API.
- Persist SONA state via `safetensors` (the public Rust crate from HuggingFace, not RuVector's wrapper) to `<runtime_root>/claude-code-optimizer/sona.safetensors` between turns.
- New MCP tool `mcp__cgreality__optimizer_apply_learning(decision_ctx, candidate_arms)` — returns a re-ranked list under our SONA's micro-LoRA influence.

**Effect on the bandit story:** this *deepens* the `SolverBandit` proposal. The bandit picks arms; SONA's trajectory recording + REINFORCE gradient estimation gives a richer reward signal that re-weights bandit priors over time. Bandit + SONA is strictly better than bandit alone.

**Effort:** 5-7 days clean-room reimplementation + integration. Major modules are LoRA forward pass (~200 LOC), EWC++ Fisher info update (~300 LOC), trajectory + REINFORCE (~250 LOC), ReasoningBank pattern clustering (~200 LOC), loop coordinator (~150 LOC). This is the clean-room estimate against the reference source; the math is well-described in their source comments.

**Risk:** medium. SONA is research-grade; verify EWC++ task-boundary detection actually fires on your decision-stream pattern. Run with `RUST_LOG=sona=debug` for the first 50 turns.

**Cross-reference:** `docs/agentdbfindings/03 §2` proposed a tiny SolverBandit. SONA is the *fuller* version; the bandit becomes the routing primitive on top of SONA's learning signal.

---

## 4. Clean-room reimplement `mmr`, `matryoshka`, `graph_rag`, `conformal_prediction`

**Inspirational source paths (read-only references):** `ruvector/crates/ruvector-core/src/advanced_features/{mmr.rs, matryoshka.rs, graph_rag.rs, conformal_prediction.rs}`.

You don't need *all* of `ruvector-core`. You need its pieces. The crate is organized so each `advanced_features/` module is self-contained.

### 4a. MMR diversity rerank for past-failure queries

Reimplement the ~50-line greedy in our own retrieval module. Read `ruvector/crates/ruvector-core/src/advanced_features/mmr.rs` for the formula + edge cases.

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
- New module `crates/context-graph-cli/src/optimizer/retrieval/{mmr,matryoshka,graph_rag,conformal}.rs` — our clean-room implementations. Header comments cite the inspirational RuVector files.
- New MCP tool: `mcp__cgreality__attempts_query_with_diversity(failure_summary, k, lambda)` — uses our `mmr.rs`.
- New MCP tool: `mcp__cgreality__attempts_global_patterns(repo_family)` — uses our `graph_rag.rs::Community`.
- `optimizer_record_recommendation` extension: optional `confidence_interval` field — uses our `conformal.rs`.

**Effect:** richer retrieval, calibrated confidence, principled global summarization. Each is a 1-day wire.

**Effort:** 2-3 days each, ~8-12 days total. MMR is the smallest; Graph RAG with Leiden community detection is the largest. Sequence by need.

**Risk:** low. All read-only or additive.

---

## 5. Clean-room reimplement `ruvector-temporal-tensor` 8/7/5/3-bit tier policy

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvector-temporal-tensor/`.

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

**Effort:** 2-3 days clean-room implementation. Each tier (8/7/5/3-bit) is a small groupwise symmetric quantizer (~100 LOC); the tier-policy state machine is another ~200 LOC. Tests for round-trip RMSE per tier are essential.

**Risk:** low. The compressor is documented as "zero deps."

---

## 6. Clean-room reimplement `ruvector-mincut` for "split tasks into independent training/eval shards"

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvector-mincut/`.

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

**Effort:** 3-4 days clean-room — implement Stoer-Wagner mincut (the canonical variant; ~300 LOC) + the graph builder for `task_failure_graph`. Read `ruvector/crates/ruvector-mincut/src/canonical/` for inspiration.

**Risk:** medium. The graph construction is the nontrivial part — what edge weights mean (shared tags, shared embeddings, shared repos, etc.) is your design choice and affects whether the cut is meaningful.

---

## 7. Clean-room reimplement a FastGRNN-style intent router

**Source crates:** `ruvector/crates/ruvector-router-core/`, `ruvector/crates/ruvector-tiny-dancer-core/`.

Currently ccreality routes via 4 search strategies × weight profiles, evaluated on every retrieval. That's flexible but slow (~10-50ms per retrieval).

For tools that don't need full strategy evaluation (e.g. `attempts_query_reflexion`), a FastGRNN-based intent router classifies the query in <10ms and dispatches. ContextGraph already has weight profiles; this just adds a faster *dispatcher*.

**Use cases:**
- "Is this a failure-mode query, a success-pattern query, or a global-summary query?" → 3-arm router → dispatch to the right sub-tool.
- "Is this an exact-recommendation-id lookup or a semantic search?" → 2-arm router → dispatch to direct lookup vs HNSW.

**Effort:** 5-6 days clean-room — FastGRNN forward pass + intent embedder + circuit breaker (~600 LOC) plus calibration dataset.

**Risk:** medium. If the router is wrong, queries route to the wrong sub-tool; circuit breaker is in `tiny-dancer-core/src/circuit_breaker.rs` — wires fall-back to the slow path on routing-error rate.

**Verdict:** quality-of-life, not load-bearing. Low priority.

---

## 8. Clean-room reimplement a coherence-driven SONA-tuning loop, inspired by `prime-radiant`

**Inspirational source crate (read-only reference):** `ruvector/crates/prime-radiant/src/sona_tuning/`.

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

**Effort:** 5-6 days clean-room. Specific to `prime-radiant`'s coherence definition; substitute "inner-actor correctness rate over the last 50 attempts" as the coherence proxy. Read `ruvector/crates/prime-radiant/src/sona_tuning/` as inspiration.

**Risk:** medium-high. Self-tuning can oscillate. Cap tune frequency (every 50 turns is probably the floor).

**Verdict:** medium-priority, after §3 (SONA) is stable.

---

## 9. Clean-room reimplement a Granger causality test, inspired by `ruvector-graph-transformer/temporal.rs`

**Source path:** `ruvector/crates/ruvector-graph-transformer/src/temporal.rs`.

Granger causality test: did harness-transition X cause subsequent score-improvement Y, controlling for prior trajectory? Standard frequentist time-series test. The crate has it as an attention-layer feature, but the test itself is a few hundred LOC of correlation + lagged regression.

**Use case:** "Did recommendation #34 actually cause the score uplift on tasks Y, Z, W, or did the uplift come from another concurrent change?"

**Verdict:** read; don't path-dep the whole graph-transformer crate just for this. Implement the test in `crates/context-graph-cli/src/optimizer/granger.rs` — you'll write ~300 LOC.

**Effort:** 2 days (port + tests).

**Risk:** medium. Granger is sensitive to time-window choice; verify against synthetic data before trusting on real harness-transition deltas.

---

## 10. Read `ruvllm_retrieval_diffusion` for the "training-free retrieval-LM" idea

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvllm_retrieval_diffusion/`.

For ccreality, this isn't a direct transplant — the optimizer is opus, not a retrieval-LM. But the *idea* — "given a corpus, retrieve next tokens via associative memory + masked diffusion, no training" — could apply to a sub-task: "given the corpus of past optimizer recommendations, generate a next-token candidate for the *body* of a new recommendation."

This is speculative. Worth keeping in mind as a future research direction, not a near-term transplant.

---

## Priority order — pick from the top

All efforts are **clean-room reimplementation** estimates (~2-3× the path-dep numbers in earlier drafts).

1. **§2 (witness chain crate)** — clean-room; ~3 days; load-bearing for FSV.
2. **§1 (sublinear solvers crate)** — clean-room; ~3-4 days; powers influence ranking and Laplacian solves.
3. **§3 (sona-style adaptive learner crate)** — clean-room; ~5-7 days; deepens the SolverBandit story.
4. **§4 (mmr + matryoshka + graph_rag + conformal modules)** — clean-room selectively; ~8-12 days total; richer retrieval + calibrated confidence.
5. **§5 (temporal-tensor 8/7/5/3-bit tier policy)** — clean-room; ~2-3 days; storage cost reduction.
6. **§6 (mincut for A/B splits)** — clean-room; ~3-4 days; cleaner uplift estimation.
7. **§9 (Granger causality test)** — clean-room; ~3-4 days; principled "did X cause Y?" attribution.
8. §7 (tiny-dancer-style intent router) — quality-of-life; defer.
9. §8 (coherence-driven SONA tuning) — defer until §3 is stable.
10. §10 (retrieval-diffusion) — research direction.

**Tier 1 status:** partially complete. §2 witness-chain core is implemented and wired. §1 solver foundation is implemented for Forward Push + CG, but BMSSP and optimizer integration remain pending. §3 SONA remains pending. The optimizer has cryptographic audit today; it does **not** yet have sublinear-PPR-aware diagnosis or REINFORCE/EWC++ memory.

**Tier 2 (§4 + §5 + §6 + §9): ~16-23 days clean-room.** Quality-of-life that adds up.

---

## Compared to the agentdb proposal

The agentdb findings recommended ~10-12 days of *Rust port* work for Tier 1+2. The locked-in clean-room-only path here recommends ~27-37 days of clean-room reimplementation for the equivalent capability — **with substantially richer primitives** and zero upstream dependency:

| What agentdb proposes | What we do instead, inspired by RuVector |
|------------------------|------------------------------------------|
| Port 270-LOC SolverBandit (Thompson Sampling) | **Partially done:** SolverBandit already exists; clean-room `context-graph-solver` has Forward Push + CG. SONA and BMSSP remain pending. |
| Port 250-LOC SHAKE-256 73-byte witness chain | **Partially done:** `context-graph-witness` SHAKE-256 chain core exists and is wired; ed25519 + lineage + attestation remain pending. |
| Port 50-LOC MMR | Clean-room implement ~150 LOC of MMR + tests — ~1 day |
| Port 300-LOC NightlyLearner doubly-robust uplift | Clean-room implement Granger test (~300 LOC) + mincut for A/B (~400 LOC) — ~7 days |
| Port 250-LOC ContextSynthesizer narrative | Same — port the heuristic, no upstream needed |
| Add Reflexion `critique` field | Same — schema-level, no transplant needed |

**Net:** zero upstream tracking burden, no foreign crates in our build graph, full ownership of every line. The cost is an extra ~15-20 days vs the path-dep alternative. Worth it per the operator's locked decision.

---

## Cross-references

- `docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md` — where new tools fit.
- `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md` — schema extensions for §2/§4d.
- `docs/agentdbfindings/03_REALITY_LOOP_APPLICATIONS.md` — same primitives at the *idea* level; this doc is the *implementation* level.
- `docs/ruvectorfindings/06_RISKS_AND_NON_TRANSFERABLE.md` — clean-room discipline, non-transferable risks, and license notes.
- `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` — sprint-level effort triage.
