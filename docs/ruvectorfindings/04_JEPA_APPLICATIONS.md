# 04 — ME-JEPA / DynamicJEPA / 5090jepa Applications

> **🔒 Reference-only — clean-room reimplementation.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — every "Vendor `X`" / "Path-dep `X`" recommendation below is SUPERSEDED. Read each as **"Read RuVector's `X` as inspiration; reimplement the algorithm cleanly in our own crate."** Effort estimates roughly 2-3× the original numbers.

> **🎯 Master plan:** [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md) is the canonical specification for the **code-domain JEPA pack** — ME-JEPA-Code, a reality compiler with 11 instruments (`E_AST`, `E_CFG`, `E_DataFlow`, `E_TypeGraph`, `E_Test`, `E_Trace`, `E_Diff`, `E_Witness`, `E_Oracle`, `E_Problem`, `E_CommitMsg`), a 4,096-d panel, an EMA + VICReg predictor, conformal calibration, OOD detection, and self-healing via EWC++. The targets below describe primitives generically against the JEPA family — for the concrete code-domain pack, defer to doc 09's instruments, training corpus (mutation strategy), and 8-phase build plan. When this doc disagrees with doc 09, doc 09 wins.

Concrete reimplementation targets in the JEPA-family workstreams, with each target citing the RuVector source file to read as inspiration.

References to JEPA docs use `backupdocs/{mejepa,dynamicjepa,5090jepa}/` paths.

---

## Current JEPA implementation state (updated 2026-05-08)

Done:

- DynamicJEPA has a RocksDB audit witness chain (`dj_audit_witness_chain`) backed by `context-graph-witness`.
- `dynamicjepa verify-audit-witness` verifies audit rows against witness entries without mutating RocksDB counts.
- `context-graph-solver` provides reusable Forward-Push PPR and CG primitives.

Pending:

- JEPA trajectory-importance sampler is not wired to `context-graph-solver`.
- ME-JEPA-Code Phase 0 mutation corpus, code-domain instruments, 4,096-d panels, predictor, conformal calibration, OOD, and self-healing are not implemented.
- Retrieval diffusion, SONA planner integration, Stage 0/F certs, verified training certs, mincut, Granger, and tiered panel storage remain pending.

---

## 1. Clean-room reimplement a witness chain crate, inspired by `crates/rvf/rvf-crypto`

**Inspirational source crate (read-only reference):** `ruvector/crates/rvf/rvf-crypto/` — same crate as `docs/ruvectorfindings/03 §2` for ccreality. Single clean-room reimplementation, two consumers (the new `crates/context-graph-witness/` serves both).

**JEPA use cases (per `backupdocs/5090jepa/00_TASK.md` rules R4 and R5):**

| Where in DynamicJEPA | What gets witnessed | Doc reference |
|----------------------|---------------------|---------------|
| Adapter ingestion | `(adapter_run_id, source_record_hash, panel_hash)` per record | `08_adapter_and_panel_pipeline.md` |
| Training run lifecycle | `(run_id, dataset_manifest_hash, hparams_hash, model_artifact_hash)` per epoch | `11_training_runs_and_artifacts.md` |
| Verification run | `(verifier_run_id, instrument_panel_hash, prediction_hash, verdict)` per case | `14_full_state_verification_protocol.md` |
| ME-JEPA Stage 0 legal gate | `(consent_doc_hash, jurisdiction_tag, allowed_state)` per record | `04_data_contract_and_privacy_gate.md` |
| Constellation/anti-pole compile | `(constellation_id, anchor_hashes, anti_pole_hashes)` | ME-JEPA `05_latent_panel_and_schema.md` |
| Read-out head emission | `(query, head_id, trajectory_ids, output, accept/reject)` | ME-JEPA `08_product_readout_heads.md` |

**Concrete shape:**

```rust
use rvf_crypto::witness::{create_witness_chain, WitnessEntry};
use rvf_crypto::lineage::{compute_manifest_hash, lineage_witness_entry};

// Per training-run epoch
let manifest_hash = compute_manifest_hash(&training_dataset_manifest)?;
let entry = WitnessEntry {
    prev_hash: read_last_chain_hash(&run_chain_path)?,
    action_hash: shake256_256(&[
        run_id.as_bytes(),
        &epoch.to_le_bytes(),
        &manifest_hash,
        &hparams_hash,
        &model_artifact_hash,
    ].concat()),
    timestamp_ns: now_ns(),
    witness_type: WitnessType::TrainingEpoch as u8,
};
append_to_chain(&run_chain_path, &create_witness_chain(&[entry]))?;
```

**WitnessType enum (DynamicJEPA-specific):**

```rust
#[repr(u8)]
pub enum WitnessType {
    Provenance = 0x01,           // adapter ingestion
    Computation = 0x02,          // generic compute
    DatasetManifest = 0x03,      // training data lock-in
    TrainingEpoch = 0x04,        // per-epoch model state
    ModelArtifact = 0x05,        // model_artifact_hash recorded
    VerificationCase = 0x06,     // FSV case execution
    Stage0Legal = 0x07,          // ME-JEPA legal gate
    ConstellationCompile = 0x08, // constellation/anti-pole compile
    ReadoutEmission = 0x09,      // read-out head emission
    SkillPromotion = 0x0A,       // skill registry update
    Lineage = 0x0B,              // parent/child relationship
}
```

**Verifier CLI:**

```
context-graph dynamicjepa verify-witness-chain --run <run_id>
context-graph mejepa verify-stage0-chain --corpus <corpus_id>
```

Each replays the chain, recomputes hashes, returns `{valid, entries: u64, last_op: WitnessType, last_action_hash: hex}`.

### 1a. Lineage records for parent/child training-run derivation

`rvf-crypto::lineage` already provides:
- `compute_manifest_hash` for dataset manifest hashes.
- `lineage_record_to_bytes` / `lineage_record_from_bytes`.
- `verify_lineage_chain` for offline replay.

Use case: when training-run `R2` is derived from `R1` (initialized from `R1`'s checkpoint, fine-tuned on different data), record the lineage so verifiers can prove the derivation chain. ME-JEPA `09_failure_policy_and_observability.md` requires this exact provenance.

### 1b. Attestation records for TEE-bound training

If you ever train on a TEE-bound machine (operator wants ME-JEPA training to happen in a confidential-compute environment for legal reasons), `rvf-crypto::attestation` has `QuoteVerifier` + `TeeBoundKeyRecord`. Drops in.

**Effect on FSV:** the `verify-counter-world` and `verify-gridworld` CLI commands (per `backupdocs/5090jepa/13_mcp_and_cli_surface.md`) get a single new step: `verify_witness_chain()`. If it returns `valid: true`, R4 + R5 are satisfied cryptographically. If `false`, you have a structured "chain broke at offset N" diagnostic.

**Effort:** ~3-4 days (3 days clean-room shared with §03 ccreality + 1 day JEPA-specific witness type enum + CLI integration).

**Risk:** low. Vendor already has tests; integration is purely additive.

---

## 2. Clean-room reimplement Forward-Push PPR for trajectory-importance ranking

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvector-solver/`.

**Current status:** the algorithmic primitive exists in `crates/context-graph-solver/` as weighted Forward-Push PPR with FSV evidence. The JEPA-specific panel-transition graph builder and sampler weighting are still pending.

**JEPA use case:**

When the predictor (per `backupdocs/dynamicjepa/07_dynamic_predictor_and_objectives.md`) trains on trajectories, naive uniform sampling over-weights long flat trajectories that teach little. PPR over the panel-transition graph gives importance scores; the training sampler uses these as weights.

```rust
use ruvector_solver::{forward_push::ForwardPushSolver, types::ComputeBudget};

// Build the panel-transition graph: nodes = unique panel signatures,
// edge weight = transition frequency
let graph = build_panel_transition_graph(&trajectories);

// PPR with α=0.15, threshold 1e-4 — classic Bahmani 2011 defaults
let solver = ForwardPushSolver::new(0.15, 1e-4);
let importance = solver.solve(&graph, &uniform_seed, &ComputeBudget::default())?;

// Per-trajectory importance = sum over transitions
let traj_importance: HashMap<TrajectoryId, f32> = trajectories.iter()
    .map(|t| (t.id, t.transitions.iter().map(|tr| importance[tr.panel_idx]).sum()))
    .collect();
```

The training sampler weights samples by `traj_importance`. Predictor focuses on transitions central to the dynamics, not over-fits to long flat trajectories.

**Effect on training quality:** lower variance in held-out loss; faster convergence on dynamics-relevant transitions. Standard PPR-importance sampling result.

**Remaining effort:** 1-2 days for the JEPA graph-builder and sampler integration. The Forward-Push primitive itself is implemented.

**Risk:** medium. PPR can over-weight high-degree centroid nodes; verify against uniform-sampling baseline before promoting.

**Cross-reference:** `docs/agentdbfindings/04 §6` proposed implementing PPR; here you path-dep a battle-tested Rust implementation.

---

## 3. Clean-room reimplement a SONA-style adaptive learner for the DynamicJEPA planner head

**Inspirational source crate (read-only reference):** `ruvector/crates/sona/`.

**JEPA use case:**

The DynamicJEPA planner head (per `backupdocs/dynamicjepa/12_prediction_planning_surprise.md`) ranks candidate actions. Today: predictor argmax. With SONA: open a trajectory per planning episode, record `(action, immediate_verifier_reward)` steps, end with `final_quality = task_success`. SONA's micro-LoRA influences the next planning step; EWC++ prevents forgetting across domain packs.

```rust
use sona::{SonaEngine, SonaConfig};

let engine = SonaEngine::new(SonaConfig {
    hidden_dim: panel_latent_dim,
    embedding_dim: panel_latent_dim,
    ..Default::default()
})?;

// Per planning episode
let mut traj = engine.begin_trajectory(initial_panel.to_vec());

for step in 0..max_steps {
    let candidate_actions = enumerate_candidate_actions(&current_panel);
    let predictor_scores = predictor.score_all(&current_panel, &candidate_actions)?;

    // Apply micro-LoRA influence
    let mut influenced_scores = vec![0.0; predictor_scores.len()];
    engine.apply_micro_lora(&predictor_scores, &mut influenced_scores);

    let chosen = argmax(&influenced_scores);
    let observed_panel_t1 = simulate_or_observe(&current_panel, chosen);
    let reward = 1.0 - verifier_distance(&predictor.predict(&current_panel, chosen)?,
                                         &observed_panel_t1);

    traj.add_step(predictor_scores.clone(), vec![], reward);
    current_panel = observed_panel_t1;
}

let final_quality = task_completion_score(...);
engine.end_trajectory(traj, final_quality);
```

### 3a. EWC++ across domain packs
`gridworld_5x5.v1` and `counter_world.v1` should each end up as a "task" in EWC++'s task memory. Switching between them at training time triggers SONA's automatic task-boundary detection. The Fisher information matrix protects gridworld weights from being overwritten by counter_world fine-tuning.

This is **the right primitive for cross-domain-pack non-interference** — exactly what `backupdocs/dynamicjepa/02_core_thesis.md` demands ("any domain can register a domain pack").

### 3b. ReasoningBank surfaces stable patterns across packs
After enough episodes, `sona::ReasoningBank::extract_patterns()` finds clusters like "high-reward trajectories often have property X" — which becomes a candidate skill in the DynamicJEPA skill registry.

### 3c. Federated coordinator for parallel domain packs
If ME-JEPA + counter_world + gridworld train concurrently (one RTX 5090, time-sliced), `sona::training::FederatedCoordinator` aggregates the per-pack adapter deltas into a consolidated cross-pack policy. Quality-weighted; high-quality packs dominate.

**JEPA changes:**
- Reuse the `crates/context-graph-sona/` clean-room crate from §03 §3.
- New module `crates/dynamicjepa-planner/src/sona_integration.rs` — wires our SONA into the planner head.
- Persist SONA state via `safetensors` (HuggingFace's public Rust crate) to `<runtime_root>/dynamicjepa/<pack_id>/sona.safetensors` per domain pack.
- New CLI: `dynamicjepa sona-stats --pack <pack_id>` — exposes our `SonaStats` for FSV.

**Effect:** the planner moves from "always exploit predictor argmax" to "learn online which kinds of actions succeed in this domain pack, generalizing across packs without forgetting." Real architectural improvement.

**Effort:** 5-7 days clean-room (shared with §03 §3 if shipped together) + 2 days JEPA-specific integration with the planner head.

**Risk:** medium-high. Reward shaping is critical (`1.0 - verifier_distance` is a starting point); micro-LoRA + EWC++ interact non-trivially with REINFORCE. Validate on counter_world (the canonical "does the plumbing work" pack) before gridworld.

**Cross-reference:** `docs/agentdbfindings/04 §2` proposed Thompson Sampling on the planner head. SONA is the *fuller* version. The bandit is the routing primitive on top of SONA's learning signal — they compose, they don't replace each other.

---

## 4. Clean-room reimplement 8/7/5/3-bit tier compression for ME-JEPA panel cold-store

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvector-temporal-tensor/`.

**JEPA use case:**

ME-JEPA's panel materialization (per `backupdocs/mejepa/05_latent_panel_and_schema.md`) writes high-dimensional latents per event per actor per role. Most are touched once at training and never again. The temporal-tensor compressor's 8/7/5/3-bit tier policy with access-pattern-driven tier selection gives 4-10× storage reduction.

```rust
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

// One compressor per (actor_role, panel_dim)
let mut comp = TemporalTensorCompressor::new(
    TierPolicy::access_pattern_driven(),
    /* tensor_dim */ panel_dim,
    /* segment_id */ actor_role_id,
);

for panel in event_stream {
    let mut segment = Vec::new();
    comp.push_frame(&panel.latent, panel.event_id, &mut segment);
    if !segment.is_empty() {
        store_segment(&segment)?;
    }
}
```

### 4a. Critically: never train against decompressed tiers
The decompression path is lossy. Training against decompressed cold-tier panels would inject noise into the gradient. **Compression is for retrieval, not training.** Keep the uncompressed training tensors out-of-band; use the compressed copy only for similarity search and aux memory.

The crate's `tier_policy.rs` lets you mark specific frames as "non-compressable" (always hot tier) — apply this to current-epoch training data.

### 4b. ME-JEPA cross-trajectory retrieval
When the predictor needs auxiliary memory ("retrieve similar past panels for attention"), search over the cold tier directly. 4-10× more panels fit in RAM.

**Effect:** for ME-JEPA's recruiting corpus (millions of events × dozens of panels each), this is a real storage cost reduction. For 5090jepa demo (small fixtures), it's overkill but free.

**Effort:** 4-5 days clean-room — each tier (8/7/5/3-bit) is a small groupwise symmetric quantizer (~100 LOC each) + tier-policy state machine (~200 LOC) + bit-exact dequant tests. Shared with §03 §5.

**Risk:** medium. JEPA latents can be sensitive to quantization. Test reconstruction error per tier before promoting cold-tier panels into retrieval results.

**Cross-reference:** `docs/agentdbfindings/04 §8` proposed the 5-tier policy from agentdb. RuVector's 8/7/5/3-bit tiers are finer-grained; prefer this implementation.

---

## 5. Clean-room reimplement Stage 0 + Stage F certificate machinery, inspired by `crates/rvf/rvf-crypto`

**Inspirational source paths (read-only references):** `ruvector/crates/rvf/rvf-crypto/{lineage.rs, attestation.rs, witness.rs}`.

ME-JEPA Stage 0 (`backupdocs/mejepa/04_data_contract_and_privacy_gate.md`) requires a legal gate that records:
- title document or sale/license document,
- privacy policy/consent review,
- approved data-use matrix,
- operating environment approval.

Each is an artifact whose presence + content needs to be cryptographically bound to a corpus. Build a `Stage0Certificate` from `rvf-crypto`'s primitives:

```rust
use rvf_crypto::{
    lineage::compute_manifest_hash,
    witness::{WitnessEntry, create_witness_chain},
    hash::shake256_256,
};

pub struct Stage0Certificate {
    pub corpus_id: String,
    pub artifacts: Vec<ArtifactRef>, // (kind, content_sha256)
    pub manifest_hash: [u8; 32],     // hash of the artifact set
    pub witness_chain_segment: Vec<u8>, // 73-byte entries
    pub policy_version: String,
    pub access_level: AccessLevel,
}

impl Stage0Certificate {
    pub fn issue(corpus_id: &str, artifacts: Vec<ArtifactRef>) -> Result<Self> {
        let manifest_hash = compute_manifest_hash(&serialize_artifacts(&artifacts))?;
        let witness = WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: manifest_hash,
            timestamp_ns: now_ns(),
            witness_type: WitnessType::Stage0Legal as u8,
        };
        let chain = create_witness_chain(&[witness]);
        Ok(Self { corpus_id: corpus_id.into(), artifacts, manifest_hash, witness_chain_segment: chain,
                   policy_version: "v1".into(), access_level: AccessLevel::Restricted })
    }

    pub fn verify(&self) -> Result<bool> {
        let recomputed = compute_manifest_hash(&serialize_artifacts(&self.artifacts))?;
        if recomputed != self.manifest_hash { return Ok(false); }
        verify_witness_chain(&self.witness_chain_segment).map(|_| true)
    }
}
```

### 5a. Stage F read-out head certificates

Per `backupdocs/mejepa/08_product_readout_heads.md`, every read-out emission (compensation band, match quality, hiring manager prediction) should ship with a certificate proving which trajectory IDs justified the answer:

```rust
pub struct ReadoutCertificate {
    pub query_id: String,
    pub head_id: String,                  // 'compensation_band', etc.
    pub trajectory_ids: Vec<String>,
    pub manifest_hash: [u8; 32],
    pub witness_chain_segment: Vec<u8>,
    pub policy_version: String,
    pub access_level: AccessLevel,
    pub latency_ms: u64,
}
```

Plus a **Merkle root** over `(trajectory_id, content_sha256)` pairs so any auditor can verify a specific trajectory's contribution to the answer without re-fetching the entire trajectory set.

`rvf-crypto` doesn't have a generic Merkle tree (it's witness-chain-shaped), but you can build one in ~150 LOC on top of `shake256_256`.

**Effect on compliance:** ME-JEPA's recruiting product gains a strong audit story. Regulator can ask "show me the certificate for placement decision X" → `verify(cert)` returns boolean → if false, the system says so out loud rather than silently degrading.

**Effort:** 3-4 days clean-room (Stage 0 + Stage F + tests; reuses our `crates/context-graph-witness/` from §1).

**Risk:** low. Cryptographic, append-only, additive.

**Cross-reference:** `docs/agentdbfindings/04 §5` proposed this with a re-implemented witness chain. Here it's vendored.

---

## 6. Clean-room reimplement Stoer-Wagner mincut for trajectory leakage check + ME-JEPA actor-aware split

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvector-mincut/`.

### 6a. DynamicJEPA leakage check

`backupdocs/5090jepa/09_trajectory_and_dataset.md` mentions leakage constraints. When splitting trajectories into train/dev/test, you want minimum overlap of "panel state ancestors" across splits.

```rust
use ruvector_mincut::canonical::CanonicalMincut;

let panel_ancestor_graph = build_ancestor_graph(&trajectories);
let cut = CanonicalMincut::new();
let (train, holdout) = cut.bipartition(&panel_ancestor_graph)?;
```

The cut minimizes shared-ancestor edge weight. Cleaner than a random split; predictor's held-out loss reflects true generalization.

### 6b. ME-JEPA per-actor split

`backupdocs/mejepa/06_sequence_dataset_and_training_objective.md` constrains splits per actor role: no candidate's events appear in both train and test. Build the actor-trajectory bipartite graph; mincut respects the constraint with minimum data loss.

```rust
let actor_traj_graph = build_actor_trajectory_graph(&events);
let (train, test) = cut.bipartition(&actor_traj_graph)?;
// Both halves are actor-disjoint; |train| ~ 80%, |test| ~ 20%
```

### 6c. ruvector-mincut features over hand-rolled
- **Witness certificates** — `src/witness/`. Every cut comes with a proof; the FSV protocol can re-verify the cut.
- **Parallel implementation** — `src/parallel/` for large graphs.
- **Link-cut trees** — `src/linkcut/` for incremental updates as trajectories accumulate.
- **Subpolynomial-time** — `src/subpolynomial/` for very-large graphs.

**Effect:** correct, fast, audit-able train/test splits with formal cut certificates.

**Effort:** 4-5 days clean-room — Stoer-Wagner mincut implementation (~300 LOC) + graph builders for both use cases. Shared with §03 §6.

**Risk:** medium. The graph construction is the actual work — what edge weights mean (shared ancestor, shared actor, shared role-context) determines whether the cut is meaningful.

---

## 7. Clean-room reimplement cross-domain-pack skill transfer, inspired by `ruvector-domain-expansion`

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvector-domain-expansion/`.

DynamicJEPA explicitly aims for cross-domain transfer (`backupdocs/dynamicjepa/02_core_thesis.md`). The interface mirrors agentdb's `learning_transfer` API (per `docs/agentdbfindings/04 §12`) but the implementation is Rust-native and ties into the rest of RuVector.

```rust
use ruvector_domain_expansion::{TransferEngine, TransferConfig, TransferKind};

let engine = TransferEngine::new(TransferConfig {
    min_similarity: 0.7,
    max_transfers: 10,
    transfer_kinds: vec![TransferKind::Skills, TransferKind::Trajectories,
                         TransferKind::CausalEdges],
});

let report = engine.transfer(&source_pack_state, &target_pack_state)?;
```

Implementation: project skills' panel exemplars from source pack's instrument space into target pack's instrument space (where instruments overlap), keep transfers above `min_similarity`, mark them with `transferred_from: <source_pack_id>` for provenance + lineage entry in the witness chain.

**Effect:** counter_world skills bootstrap gridworld learning. ME-JEPA recruiting skills bootstrap a hypothetical adjacent domain (e.g., academic placement) when both packs are in the system.

**Effort:** 3 days. The projection-across-instrument-spaces piece is the actual work; the API surface is small.

**Risk:** medium. Cross-pack semantics are subtle — verify with the counter_world ↔ gridworld pack pair as the smoke test before claiming cross-domain transfer works.

---

## 8. Clean-room reimplement Granger causality + retrocausal safety, inspired by `ruvector-graph-transformer/temporal.rs`

**Inspirational source path (read-only reference):** `ruvector/crates/ruvector-graph-transformer/src/temporal.rs`.

### 8a. Causal masking for the predictor

The predictor's attention should respect the causal direction of trajectories. `temporal.rs` provides:
- `MaskStrategy::Strict` — node at time t can only attend to nodes at t' < t.
- `MaskStrategy::TimeWindow { window_size }` — node at time t attends to t' in `[t - window_size, t]`.

The DynamicJEPA predictor uses a sliding window over panel sequences; `MaskStrategy::TimeWindow` with `window_size = 4` matches the 4-step trajectory format in `backupdocs/5090jepa/09_trajectory_and_dataset.md`.

### 8b. Granger causality for skill-promotion validation

When promoting a candidate skill, ask: "did this skill's use *Granger-cause* the subsequent reward improvement, controlling for prior trajectory state?" Granger is a frequentist time-series test — a few hundred LOC.

The full graph-transformer crate has the test embedded in an attention layer. For DynamicJEPA, port just the test to `crates/dynamicjepa-skill-registry/src/granger.rs` (~300 LOC).

### 8c. Continuous-time neural ODE on graphs

For sub-second event streams (where panel materialization happens at irregular intervals), the predictor wants continuous-time dynamics. `temporal.rs` integrates `neural-ode-rs`-style ODE solvers; for the 5090jepa demo this is overkill, but for ME-JEPA's real recruiting corpus (events at all kinds of intervals), it's the right primitive.

### 8d. Retrocausal-safety enforcement

`temporal.rs` includes guards that reject attention attempts crossing the causal boundary. For ME-JEPA Stage F (read-out heads), this prevents the predictor from "seeing" outcome events when computing attention — a real concern when training on placement-quality data where outcomes are recorded after the trajectory.

**Effort:** 5-7 days clean-room. Causal masking ~300 LOC; Granger test ~300 LOC; retrocausal-safety guards ~200 LOC. All inspired by `ruvector/crates/ruvector-graph-transformer/src/temporal.rs`.

**Risk:** medium. Granger is sensitive to time-window choice; verify against synthetic data before trusting.

---

## 9. Clean-room reimplement FSV-grade training-step audit, inspired by `ruvector-graph-transformer/verified_training.rs`

**Inspirational source path (read-only reference):** `ruvector/crates/ruvector-graph-transformer/src/verified_training.rs`.

This is the single highest-leverage transplant for `backupdocs/5090jepa/00_TASK.md` rules R3 (no mock data) and R4 (FSV). The crate's `verified_training` produces a `TrainingCertificate` per gradient step with:

- `LossStabilityBound` — Reflex tier, formally proven.
- `WeightNormBound` — Standard tier, formally proven.
- `LipschitzBound` — Standard tier, statistical estimate.
- `PermutationEquivariance` — Deep tier, statistical estimate.
- `EnergyGate` — Standard tier, formally proven.

Delta-apply pattern: gradients go to a scratch buffer, invariants are checked on the proposed state, the delta is committed only if all invariants pass. **Fail-closed by default.**

```rust
use ruvector_graph_transformer::verified_training::{
    VerifiedTrainingConfig, VerifiedTrainer, TrainingCertificate
};

let cfg = VerifiedTrainingConfig {
    invariants: vec![
        Invariant::LossStabilityBound { delta: 0.1 },
        Invariant::WeightNormBound { max: 100.0 },
        Invariant::EnergyGate { threshold: 1e-3 },
    ],
    fail_closed: true,
};

let trainer = VerifiedTrainer::new(cfg);
let cert: TrainingCertificate = trainer.step_with_proof(&mut model, &batch)?;
// cert is signed via rvf-crypto if signing is enabled
write_certificate_to_run_dir(&run_id, &cert)?;
```

Each certificate ties into `rvf-crypto::witness` so the per-step certificates *also* extend the witness chain. ME-JEPA `09_failure_policy_and_observability.md` requires exactly this kind of provenance.

**Effort:** 5-6 days clean-room implementation. Per-step certificate generation (~400 LOC) + invariant checks (~500 LOC) + integration with existing DynamicJEPA training pipeline.

**Risk:** medium. The "Lipschitz bound" estimate has hyperparameters; calibrate on counter_world before trusting on gridworld or ME-JEPA.

**Verdict:** worth doing for ME-JEPA's compliance posture. Less critical for 5090jepa demo (which can rely on the simpler witness chain alone).

---

## 10. Clean-room reimplement training-free retrieval LM + masked discrete diffusion for the DynamicJEPA gridworld planner head

**Inspirational source crate (read-only reference):** `ruvector/crates/ruvllm_retrieval_diffusion/`. Latest commit (PR #450, 2026-05-08).

**JEPA use case (gridworld_5x5.v1):**

Per `backupdocs/5090jepa/07_domain_pack_gridworld.md`, gridworld has a small action vocab (move-up/down/left/right, wait, etc.). This fits perfectly inside `vocab_size ≤ 254`. Use `Diffuser::diffuse` directly:

```rust
use ruvllm_retrieval_diffusion::{Diffuser, RetrievalConfig, SamplingConfig};
use ruvllm_sparse_attention::SparseAttentionConfig;

let cfg = RetrievalConfig {
    vocab_size: 8,                      // 8 gridworld actions
    head_dim: 64,
    pos_scale: 0.5,
    mask_sentinel: 255,
    ..RetrievalConfig::default()
};

// Build the corpus from successful gridworld trajectories
let corpus: Vec<u8> = encode_successful_trajectories(&trajectory_history);
let diffuser = Diffuser::new(corpus, cfg, /* seed */ 0xMARI_BEEF);

// Per planning episode
let masked_seq = mask_actions(&current_state_sequence);
let candidate_continuation = diffuser.diffuse(&masked_seq, &SamplingConfig::quality(), 0xC0FFEE);
```

The diffuser is **training-free** — no autograd, no learned weights, no Python toolchain. It's pure associative-memory retrieval with masked diffusion ranking.

Their measured number: **6.9× over AR baseline** on Mario benchmark. For a discrete-action gridworld, equivalent or better speedup is plausible.

### 10a. For counter_world.v1 (vocab_size = 4)

Even smaller vocab. Same drop-in.

### 10b. For ME-JEPA recruiting (large vocab)

The vocab ≤ 254 limit is too tight. Use `Retriever::generate_fast` (the AR path) with a domain-specific tokenizer. Slower but unrestricted.

**Effect:** the gridworld planner gains a fast bidirectional candidate-action generator without training. This is *the right primitive* for "given a partially-observed trajectory, what actions complete it best?" — exactly what the planner head needs.

**Effort:** 4-5 days clean-room — `Retriever` AR path (~250 LOC) + `Diffuser` masked-diffusion path (~400 LOC) + MaskGIT cosine schedule (~100 LOC) + corpus encoder. The training-free aspect makes this very tractable — no autograd, no backprop, just associative-memory retrieval.

**Risk:** medium. Quality on small-vocab domains is "as good as the corpus" — needs a sufficient set of successful trajectories before the diffuser is useful. Boostrap with random play.

**Cross-reference:** this is the most exciting transplant given the **2026-05-08 release**. Probably worth a week of operator attention.

---

## 11. Read `prime-radiant` + `ruvector-coherence` for ME-JEPA anti-pole / hallucination detection ideas

**Source crates:** `ruvector/crates/prime-radiant/`, `ruvector/crates/ruvector-coherence/`.

ME-JEPA's anti-pole compiler (per `backupdocs/mejepa/05_latent_panel_and_schema.md`) penalizes predictions that drift into forbidden regions. Prime-radiant's sheaf-Laplacian-based coherence engine is a more principled alternative to "anti-pole as a set of forbidden centroids":

- **Coherence ρ** — measure signal quality of an output.
- **Learned ρ threshold** — `prime-radiant/src/learned_rho/` learns the threshold from data.
- **Neural gate** — `prime-radiant/src/neural_gate/` refuses outputs below ρ.
- **SONA tuning** — `prime-radiant/src/sona_tuning/` tunes SONA's hyperparameters against coherence loss.

**Verdict:** read before committing. Prime-radiant is a substantial dependency (mincut + sheaf cohomology + GPU); the simpler anti-pole-set design might be enough for ME-JEPA v1.

**Effort:** read 1-2 days; if adopted, ~10-12 days clean-room (sheaf Laplacian math is non-trivial to reimplement faithfully).

**Risk:** high. Sheaf Laplacian math is non-trivial; verify by comparing against simple anti-pole-set baseline.

---

## 12. Read `ruvector-domain-expansion` + `ruvector-graph-transformer/manifold.rs` for cross-instrument transfer

**Inspirational source paths (read-only references):** `ruvector/crates/ruvector-domain-expansion/src/`, `ruvector/crates/ruvector-graph-transformer/src/manifold.rs`.

When ME-JEPA's 19-space panel needs to integrate a new instrument (`Erec6` for, say, geographic mobility), the question is: how do existing trajectories embed in the new joint space?

`ruvector-graph-transformer/manifold.rs` provides product manifold S^n × H^m × R^k operations. The new joint space is a product of the existing 19 instrument spaces × the new instrument space; trajectories project via Riemannian message passing.

Combined with `ruvector-domain-expansion` (§7), this gives a principled "absorb a new instrument without retraining from scratch" path.

**Effort:** 4-5 days. Substantial.

**Risk:** medium. Manifold math is correct but performance-tuning Riemannian operations is non-trivial.

---

## Priority order — pick from the top

All efforts are **clean-room reimplementation** estimates per the locked rule.

1. **§1 (witness chain crate)** — clean-room; ~3-4 days; load-bearing for FSV (R4/R5).
2. **§10 (training-free retrieval LM + diffuser for gridworld)** — clean-room; ~4-5 days; design-novelty win.
3. **§3 (sona-style adaptive learner for planner)** — clean-room; ~5-7 days (shared with §03 §3); cross-pack non-interference.
4. **§5 (Stage 0 + Stage F certificates)** — clean-room; ~3-4 days (reuses §1 crate); ME-JEPA compliance.
5. **§9 (verified_training certificates per gradient step)** — clean-room; ~5-6 days; FSV-grade per-step audit.
6. **§2 (PPR for trajectory importance)** — clean-room; ~3-4 days (shared with §03 §1); training quality.
7. **§4 (8/7/5/3-bit tier compression)** — clean-room; ~4-5 days; storage cost reduction.
8. **§6 (mincut for leakage + actor split)** — clean-room; ~4-5 days (shared with §03 §6).
9. **§8 (Granger + retrocausal safety)** — clean-room; ~5-7 days; "did skill cause uplift?" attribution.
10. **§7 (cross-pack skill transfer)** — clean-room; ~6-7 days.
11. **§12 (cross-instrument transfer via manifold)** — read; ~10+ days clean-room if adopted.
12. **§11 (prime-radiant coherence)** — read; ~10-12 days clean-room if adopted.

**Tier 1 status:** partially complete. §1 witness/audit foundation exists for DynamicJEPA audit rows, and §2's Forward-Push primitive exists in `context-graph-solver`. §10 retrieval diffusion and §3 SONA-style planner learning remain pending. Therefore, do not claim the gridworld planner or online REINFORCE/EWC++ path has shipped yet.

**Tier 2 (§5 + §9 + §2 + §4 + §6): ~19-24 days clean-room.** Compliance certs + FSV-grade per-step audit + better training quality + cold storage + cleaner splits.

**Tier 3 (§8 + §7 + §12 + §11): ~30-40+ days clean-room.** Deep integrations.

---

## Compared to the agentdb-only proposal

agentdb's findings recommended 8-15+ days of *Rust port* work for JEPA transplants. The locked-in clean-room-only path here recommends ~31-40 days of clean-room reimplementation total (Tier 1 + Tier 2) — for **substantially more capability** and zero upstream dependency:

| What agentdb proposes | What we do instead, inspired by RuVector |
|------------------------|------------------------------------------|
| Port 73-byte SHAKE-256 witness chain | Clean-room reimplement our own `context-graph-witness` (chain + ed25519 + lineage + attestation) — ~3-4 days |
| Port doubly-robust uplift estimator (NightlyLearner) | Clean-room: SONA REINFORCE + our mincut for A/B + Granger test — ~12-15 days |
| Port Merkle certificates for Stage 0/F | Clean-room: build certificates atop our witness crate in ~150 LOC of glue — ~3-4 days total |
| Implement PPR for trajectory importance | **Primitive done:** clean-room Forward Push in `context-graph-solver`; JEPA sampler integration remains |
| Implement Mincut for leakage check | Clean-room Stoer-Wagner — ~4-5 days |
| Implement TemporalCompressor 5-tier | Clean-room 8/7/5/3-bit groupwise symmetric quant — ~4-5 days |
| Use ContextSynthesizer narrative | Same — port the heuristic, no transplant needed |

**Net:** zero upstream tracking burden, no foreign crates in our build graph, full ownership of every line. The cost is roughly +15-20 days vs the path-dep alternative — accepted per the operator's locked decision.

---

## Cross-references

- `backupdocs/5090jepa/00_TASK.md` rules R4, R5 — directly satisfied by §1 + §9.
- `backupdocs/dynamicjepa/12_implementation_roadmap.md` — natural sequencing for §3 + §7 + §10.
- `backupdocs/mejepa/04_data_contract_and_privacy_gate.md` — directly satisfied by §5.
- `docs/agentdbfindings/04_JEPA_APPLICATIONS.md` — ideas at the *high level*; this doc is the *implementation level*.
- `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` — sprint-level effort triage.
- `docs/ruvectorfindings/06_RISKS_AND_NON_TRANSFERABLE.md` — clean-room provenance discipline, license-comment requirements.
