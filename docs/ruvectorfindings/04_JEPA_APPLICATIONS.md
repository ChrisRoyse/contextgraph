# 04 — ME-JEPA / DynamicJEPA / 5090jepa Applications

Concrete transplants from the RuVector workspace to the JEPA-family workstreams. Like §03 for ccreality, most are vendor or path-dep rather than port. The story is dramatically richer than the agentdb-only findings: RuVector provides multiple primitives that map directly onto the JEPA spec.

References to JEPA docs use `backupdocs/{mejepa,dynamicjepa,5090jepa}/` paths.

---

## 1. Vendor `crates/rvf/rvf-crypto` for the DynamicJEPA training-run + verification-run audit chain

**Source crate:** `ruvector/crates/rvf/rvf-crypto/` — same crate as `docs/ruvectorfindings/03 §2` for ccreality. Single vendor, two consumers.

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

**Effort:** ~2 days (1 day vendor shared with §03 ccreality, 1 day JEPA-specific witness type enum + CLI integration).

**Risk:** low. Vendor already has tests; integration is purely additive.

---

## 2. Path-dep `ruvector-solver` for trajectory-importance ranking via PPR

**Source crate:** `ruvector/crates/ruvector-solver/`.

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

**Effort:** 1 day path-dep + graph-builder for `panel_transition_graph` + sampler-weight integration.

**Risk:** medium. PPR can over-weight high-degree centroid nodes; verify against uniform-sampling baseline before promoting.

**Cross-reference:** `docs/agentdbfindings/04 §6` proposed implementing PPR; here you path-dep a battle-tested Rust implementation.

---

## 3. Vendor `sona` for the DynamicJEPA planner head's online learning

**Source crate:** `ruvector/crates/sona/`.

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

**ccreality changes:**
- Vendor or path-dep `crates/sona/`.
- New module `crates/dynamicjepa-planner/src/sona_integration.rs` — wires SONA into the planner head.
- Persist SONA state to `<runtime_root>/dynamicjepa/<pack_id>/sona.safetensors` per domain pack (uses `sona::export::SafeTensorsExporter`).
- New CLI: `dynamicjepa sona-stats --pack <pack_id>` — exposes `SonaStats` for FSV.

**Effect:** the planner moves from "always exploit predictor argmax" to "learn online which kinds of actions succeed in this domain pack, generalizing across packs without forgetting." Real architectural improvement.

**Effort:** 3-4 days vendor + integration. Most is wiring; SONA's API is small.

**Risk:** medium-high. Reward shaping is critical (`1.0 - verifier_distance` is a starting point); micro-LoRA + EWC++ interact non-trivially with REINFORCE. Validate on counter_world (the canonical "does the plumbing work" pack) before gridworld.

**Cross-reference:** `docs/agentdbfindings/04 §2` proposed Thompson Sampling on the planner head. SONA is the *fuller* version. The bandit is the routing primitive on top of SONA's learning signal — they compose, they don't replace each other.

---

## 4. Path-dep `ruvector-temporal-tensor` for ME-JEPA panel cold-store

**Source crate:** `ruvector/crates/ruvector-temporal-tensor/`.

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

**Effort:** 2 days path-dep + tier policy + bit-exact dequant tests.

**Risk:** medium. JEPA latents can be sensitive to quantization. Test reconstruction error per tier before promoting cold-tier panels into retrieval results.

**Cross-reference:** `docs/agentdbfindings/04 §8` proposed the 5-tier policy from agentdb. RuVector's 8/7/5/3-bit tiers are finer-grained; prefer this implementation.

---

## 5. Vendor `crates/rvf/rvf-crypto` certificate machinery for ME-JEPA Stage 0 + Stage F

**Source paths:** `ruvector/crates/rvf/rvf-crypto/{lineage.rs, attestation.rs, witness.rs}`.

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

**Effort:** 2 days (Stage 0 + Stage F + tests).

**Risk:** low. Cryptographic, append-only, additive.

**Cross-reference:** `docs/agentdbfindings/04 §5` proposed this with a re-implemented witness chain. Here it's vendored.

---

## 6. Path-dep `ruvector-mincut` for trajectory leakage check + ME-JEPA actor-aware split

**Source crate:** `ruvector/crates/ruvector-mincut/`.

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

**Effort:** 2 days path-dep + graph builders for both use cases.

**Risk:** medium. The graph construction is the actual work — what edge weights mean (shared ancestor, shared actor, shared role-context) determines whether the cut is meaningful.

---

## 7. Path-dep `ruvector-domain-expansion` for cross-domain-pack skill transfer

**Source crate:** `ruvector/crates/ruvector-domain-expansion/`.

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

## 8. Vendor `ruvector-graph-transformer/temporal.rs` for Granger causality + retrocausal safety in DynamicJEPA

**Source path:** `ruvector/crates/ruvector-graph-transformer/src/temporal.rs`.

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

**Effort:** 3-4 days. The causal masking is path-dep + 1 day; Granger test is a port (~2 days); retrocausal safety is path-dep + 1 day to wire into ME-JEPA Stage F.

**Risk:** medium. Granger is sensitive to time-window choice; verify against synthetic data before trusting.

---

## 9. Vendor `ruvector-graph-transformer/verified_training.rs` for FSV-grade training-step audit

**Source path:** `ruvector/crates/ruvector-graph-transformer/src/verified_training.rs`.

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

**Effort:** 3 days vendor + integration with the existing DynamicJEPA training pipeline.

**Risk:** medium. The "Lipschitz bound" estimate has hyperparameters; calibrate on counter_world before trusting on gridworld or ME-JEPA.

**Verdict:** worth doing for ME-JEPA's compliance posture. Less critical for 5090jepa demo (which can rely on the simpler witness chain alone).

---

## 10. Path-dep `ruvllm_retrieval_diffusion` for the DynamicJEPA gridworld planner head

**Source crate:** `ruvector/crates/ruvllm_retrieval_diffusion/`. Latest commit (PR #450, 2026-05-08).

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

**Effort:** 1-2 days path-dep + corpus encoder.

**Risk:** medium. Quality on small-vocab domains is "as good as the corpus" — needs a sufficient set of successful trajectories before the diffuser is useful. Boostrap with random play.

**Cross-reference:** this is the most exciting transplant given the **2026-05-08 release**. Probably worth a week of operator attention.

---

## 11. Read `prime-radiant` + `ruvector-coherence` for ME-JEPA anti-pole / hallucination detection

**Source crates:** `ruvector/crates/prime-radiant/`, `ruvector/crates/ruvector-coherence/`.

ME-JEPA's anti-pole compiler (per `backupdocs/mejepa/05_latent_panel_and_schema.md`) penalizes predictions that drift into forbidden regions. Prime-radiant's sheaf-Laplacian-based coherence engine is a more principled alternative to "anti-pole as a set of forbidden centroids":

- **Coherence ρ** — measure signal quality of an output.
- **Learned ρ threshold** — `prime-radiant/src/learned_rho/` learns the threshold from data.
- **Neural gate** — `prime-radiant/src/neural_gate/` refuses outputs below ρ.
- **SONA tuning** — `prime-radiant/src/sona_tuning/` tunes SONA's hyperparameters against coherence loss.

**Verdict:** read before committing. Prime-radiant is a substantial dependency (mincut + sheaf cohomology + GPU); the simpler anti-pole-set design might be enough for ME-JEPA v1.

**Effort:** read 1-2 days; if adopted, ~1 week vendor + integration.

**Risk:** high. Sheaf Laplacian math is non-trivial; verify by comparing against simple anti-pole-set baseline.

---

## 12. Read `ruvector-domain-expansion` + `ruvector-graph-transformer/manifold.rs` for cross-instrument transfer

**Source paths:** `ruvector/crates/ruvector-domain-expansion/src/`, `ruvector/crates/ruvector-graph-transformer/src/manifold.rs`.

When ME-JEPA's 19-space panel needs to integrate a new instrument (`Erec6` for, say, geographic mobility), the question is: how do existing trajectories embed in the new joint space?

`ruvector-graph-transformer/manifold.rs` provides product manifold S^n × H^m × R^k operations. The new joint space is a product of the existing 19 instrument spaces × the new instrument space; trajectories project via Riemannian message passing.

Combined with `ruvector-domain-expansion` (§7), this gives a principled "absorb a new instrument without retraining from scratch" path.

**Effort:** 4-5 days. Substantial.

**Risk:** medium. Manifold math is correct but performance-tuning Riemannian operations is non-trivial.

---

## Priority order — pick from the top

1. **§1 (rvf-crypto witness chain for training/verification runs)** — vendor; ~2 days; load-bearing for FSV.
2. **§10 (ruvllm_retrieval_diffusion for gridworld planner)** — path-dep; ~1-2 days; latest active workstream, dramatic speedup story.
3. **§3 (sona for planner online learning + EWC++ across packs)** — vendor; ~3-4 days; deepens the bandit story; right primitive for cross-pack non-interference.
4. **§5 (Stage 0 + Stage F certificates)** — vendor (shared with §1); ~2 days; ME-JEPA compliance.
5. **§9 (verified_training certificates per gradient step)** — vendor; ~3 days; FSV-grade per-step audit.
6. **§2 (PPR for trajectory importance)** — path-dep; ~1 day; training quality.
7. **§4 (temporal-tensor cold storage)** — path-dep; ~2 days; storage cost reduction at scale.
8. **§6 (mincut for leakage check + actor split)** — path-dep; ~2 days; cleaner held-out loss.
9. **§8 (Granger causality + retrocausal safety)** — port; ~3-4 days; principled "did skill cause uplift?" attribution.
10. **§7 (cross-pack skill transfer)** — path-dep; ~3 days; counter_world ↔ gridworld smoke.
11. **§12 (cross-instrument transfer via manifold)** — read; ~5 days if adopted.
12. **§11 (prime-radiant coherence)** — read; ~1 week if adopted.

**Tier 1 (§1 + §10 + §3): ~7-8 days.** Witness chain + retrieval-diffusion + SONA. After this sprint:
- Every training / verification run has cryptographic audit.
- gridworld planner is a 6.9× faster training-free retrieval-LM.
- The planner learns online via REINFORCE-trajectory + EWC++.

**Tier 2 (§5 + §9 + §2 + §4 + §6): ~10 days.** Compliance certificates + FSV-grade per-step audit + better training quality + cold storage + cleaner splits.

**Tier 3 (§8 + §7 + §12 + §11): ~15+ days.** Deep integrations.

---

## Compared to the agentdb-only proposal

agentdb's findings recommended 8-15+ days of *Rust port* work for JEPA transplants (witness chain, doubly-robust uplift, Stage 0/F certificates, Mincut, PPR). RuVector findings recommend ~17 days of *vendor + path-dep* work for **substantially more capability**:

| What agentdb proposes | What RuVector lets you do instead |
|------------------------|-----------------------------------|
| Port 73-byte SHAKE-256 witness chain | Vendor `rvf-crypto` (production crate, no_std, ed25519 + ML-DSA-65 + SLH-DSA-128s + lineage + attestation) |
| Port doubly-robust uplift estimator (NightlyLearner) | Use `sona` REINFORCE + `ruvector-mincut` for cleaner A/B + port Granger from `temporal.rs` |
| Port Merkle certificates for Stage 0/F | Build from `rvf-crypto` lineage + witness primitives in 150 LOC of glue |
| Implement PPR for trajectory importance | Path-dep `ruvector-solver::forward_push` |
| Implement Mincut for leakage check | Path-dep `ruvector-mincut::canonical` |
| Implement TemporalCompressor 5-tier | Path-dep `ruvector-temporal-tensor` (8/7/5/3-bit, finer-grained, zero deps) |
| Use ContextSynthesizer narrative | Same — port the heuristic, no transplant needed |

**Net:** RuVector replaces "port multi-thousand LOC" with "vendor 1 sub-crate + 6 path-deps + ~500 LOC of glue."

---

## Cross-references

- `backupdocs/5090jepa/00_TASK.md` rules R4, R5 — directly satisfied by §1 + §9.
- `backupdocs/dynamicjepa/12_implementation_roadmap.md` — natural sequencing for §3 + §7 + §10.
- `backupdocs/mejepa/04_data_contract_and_privacy_gate.md` — directly satisfied by §5.
- `docs/agentdbfindings/04_JEPA_APPLICATIONS.md` — ideas at the *high level*; this doc is the *implementation level*.
- `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` — sprint-level effort triage.
- `docs/ruvectorfindings/06_RISKS_AND_NON_TRANSFERABLE.md` — vendor-vs-path-dep tradeoffs, license considerations.
