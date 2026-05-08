# 04 — ME-JEPA / DynamicJEPA / 5090jepa Applications

This doc maps agentdb capabilities onto the three JEPA-family workstreams in this repo:

- **DynamicJEPA** — `backupdocs/dynamicjepa/` — the universal world-model protocol (domain pack → instrument panel → predictor → planner/heads).
- **ME-JEPA** — `backupdocs/mejepa/` — career/recruiting corpus → 19-space panel → trajectory predictor → recruiting heads.
- **5090jepa demo** — `backupdocs/5090jepa/` — RTX 5090–native end-to-end PoC of DynamicJEPA on counter_world + gridworld_5x5.

JEPA is a *latent-dynamics* system, not a memory system. agentdb is the opposite — a memory system, not a dynamics system. So the transplants are at the **scaffolding** layer (audit, evaluation, retrieval, ranking) and the **policy** layer (action selection, candidate ranking), not at the core predictor.

References to JEPA docs use the structure under `backupdocs/`. Adoption decisions go through `./memory/decisions/` per CLAUDE.md §6.

---

## 1. SHAKE-256 witness chain → DynamicJEPA training-run + verification-run audit chain

**Borrowing:** `RvfSolver`'s `73-byte/entry` SHAKE-256 chain (`src/backends/rvf/RvfSolver.ts`, exposed via `SelfLearningRvfBackend.getWitnessChain()`).

**Why this is the most important JEPA transplant:** `backupdocs/5090jepa/00_TASK.md` rule R4 demands "every persisted behavior has a Source of Truth, executed-then-independently-inspected" — and rule R5 demands "trigger → outcome verification" with "find the database row, the file hash, the audit entry, the count delta. Look at it." A SHAKE-256 witness chain over the (run_id, op_type, content_hash, prev_hash) sequence is the cleanest tamper-evident layer over the existing CFs. It satisfies R4/R5 *cryptographically* rather than by review.

**Use cases inside DynamicJEPA:**

| Where | What gets witnessed | Doc reference |
|-------|---------------------|---------------|
| Adapter ingestion | `(adapter_run_id, source_record_hash, panel_hash)` per record | `08_adapter_and_panel_pipeline.md` |
| Training run lifecycle | `(run_id, dataset_manifest_hash, hparams_hash, model_artifact_hash)` per epoch | `11_training_runs_and_artifacts.md` |
| Verification run | `(verifier_run_id, instrument_panel_hash, prediction_hash, verdict)` per case | `14_full_state_verification_protocol.md` |
| ME-JEPA Stage 0 legal gate | `(consent_doc_hash, jurisdiction_tag, allowed_state)` per record | `04_data_contract_and_privacy_gate.md` |
| Constellation/anti-pole compile | `(constellation_id, anchor_hashes, anti_pole_hashes)` | ME-JEPA `05_latent_panel_and_schema.md` |

**Concrete shape (Rust, per training run):**
```rust
pub struct WitnessEntry {
    pub timestamp: u64,
    pub op_type: WitnessOpType,   // u8 — Ingest, Train, Verify, Promote, ...
    pub content_sha256: [u8; 32], // hash of the artifact written this step
    pub prev_chain_hash: [u8; 32],
}
// 73 bytes total, exactly matching agentdb's stride.
```

Append to a CF `CF_DJEPA_WITNESS_CHAIN`, keyed by `(run_id, offset)`. Provide a `dynamicjepa verify-witness-chain --run <run_id>` CLI that replays the chain and confirms `chain.len() % 73 == 0` plus `every prev_chain_hash matches`.

**Anti-tamper benefit:** if any CF row gets edited post-hoc, the witnessed hash chain will mismatch on replay. This is exactly the auditability requirement that the FSV protocol describes — it makes "look at the bytes" a single cryptographic check rather than a review.

**Effort:** 1 day for the chain code + Rust port (same as ccreality §5 — one implementation, two consumers).

**Risk:** low. Append-only.

---

## 2. SolverBandit → DynamicJEPA candidate-action ranking + ME-JEPA recruit-step ranking

**Borrowing:** `src/backends/rvf/SolverBandit.ts` (270 LOC) + the 18-bucket context model.

**For DynamicJEPA's planner head** (`backupdocs/dynamicjepa/08_memory_planning_and_skills.md`, `12_prediction_planning_surprise.md`):

The planner enumerates candidate actions, the predictor scores each in latent space, the planner picks one. Today the picking is argmax over predictor score. With Thompson Sampling, the picking becomes:

- **Arms** = candidate-action templates (e.g. "increment_then_branch", "scan_left_first", "verify_before_act").
- **Context bucket** = `(domain_pack × instrument_panel_signature × goal_kind)` — analogous to agentdb's `(range, distractor, noise)` 18-bucket schema.
- **Reward** = `1.0 - verifier_distance(prediction, observed_state_t+1)`. Short-cycle reward — every step provides a signal.

This converts the planner from "always exploit the predictor's current best guess" to "occasionally explore lesser-known candidate actions when their posterior says they might be better." This is the standard exploration win for any plan-and-execute loop, made small + auditable.

**For ME-JEPA's recruiter-step ranking** (`backupdocs/mejepa/08_product_readout_heads.md`):

Read-out heads emit candidate next steps (e.g., "request reference call", "schedule technical screen", "send compensation band"). The bandit picks across them, reward is the eventual placement-quality score. The 18-bucket model maps onto `(seniority_band × industry_vertical × stage_of_funnel)`.

**Anti-tamper benefit:** the bandit's full state is JSON-serializable (~kilobytes). Snapshot at every training-run promotion → witness-chain entry. Regulator/operator can replay the policy state at any historical point.

**Effort:** the SolverBandit Rust port (0.5 day) is shared with ccreality §2. JEPA-specific work is wiring it into the planner and read-out heads (~2 days for DynamicJEPA, ~3 days for ME-JEPA because the read-out head schemas are more nuanced).

**Risk:** medium. Reward shaping for the planner step is critical — wrong reward means the bandit converges to garbage. Use `1.0 - verifier_distance` as a starting point, log raw verifier output for offline tuning.

**Read for context:**
- `backupdocs/dynamicjepa/12_prediction_planning_surprise.md` for the planner spec.
- ccreality `03_REALITY_LOOP_APPLICATIONS.md §2` for the same primitive in the optimizer context.

---

## 3. NightlyLearner doubly-robust uplift → DynamicJEPA "did this skill actually help?" verification

**Borrowing:** `src/controllers/NightlyLearner.ts:98-180` + the doubly-robust formula
```
τ̂ = E[reward|treatment] − E[reward|control]
   + a·(y − μ₁(x))/e(x) − (1−a)·(y − μ₀(x))/(1 − e(x))
```

**Use case:** DynamicJEPA's skill registry (`backupdocs/dynamicjepa/08_memory_planning_and_skills.md`) wants to promote successful trajectories into reusable skills. But "successful" is fragile — a skill that appears to help could be confounded by the trajectory it's typically used in. Doubly-robust estimation gives a propensity-corrected uplift estimate: "controlling for which trajectories tend to use skill X anyway, X improves outcome by τ̂."

**Concrete pipeline (run periodically per domain pack):**

1. **Cluster trajectories** by panel signature (cluster centroid in latent space).
2. **For each candidate skill**, partition its uses across clusters → treatment (used skill) vs control (didn't).
3. **Fit μ₀, μ₁ outcome models** as linear ridge regressions over panel features.
4. **Fit propensity model e(x)** as a logistic regression: P(used skill | panel features).
5. **Compute τ̂** for the skill.
6. **Promote skills with τ̂ > upliftThreshold and confidence > 0.6**, exactly per agentdb's defaults (which are sensible).
7. **Demote / archive** skills with τ̂ < 0 (actively harming).

**Witness-chain integration:** every promotion/demotion appends an entry. The chain becomes an audit log of "skill X was promoted on date D with uplift τ̂ on dataset manifest H."

**Effect on training quality:** this is the missing piece between "trajectory finished successfully" and "we should reuse the skill." Without it, the skill registry accumulates lucky trajectories that happen to coincide with easy contexts. With it, only skills that *causally* lift outcomes survive.

**Effort:** 3 days (estimator implementation + integration with the existing skill registry).

**Risk:** medium. Doubly-robust is sensitive to extreme propensities (e(x) near 0 or 1 → unstable). Clip propensities to [0.05, 0.95] like agentdb does.

---

## 4. CausalRecall utility-reranker → DynamicJEPA trajectory retrieval

**Borrowing:** `src/controllers/CausalRecall.ts:90-140` (the α·sim + β·uplift − γ·cost formula).

**Use case:** when the predictor is asked "given this current panel, what past trajectories should I attend over?" — today this is k-NN over panel similarity. With CausalRecall:
- α-term: similarity of current panel to past panels.
- β-term: how much that past trajectory's continuation actually helped (verifier-distance reduction). This is the trajectory's *uplift*.
- γ-term: how stale the trajectory is (decay or freshness penalty), or how expensive it is to fetch.

The result: trajectories that *teach* dominate over trajectories that merely *resemble*.

**Where it slots in:**
- `backupdocs/dynamicjepa/06_latent_panel_and_binding.md` — the binding step retrieves panel-aligned context.
- `backupdocs/dynamicjepa/07_dynamic_predictor_and_objectives.md` — the predictor's auxiliary memory attention.

**Anti-overfit guard:** the uplift is per-trajectory, not per-domain. Trajectories from one domain pack can't accidentally pump uplift signal into a different domain pack's retrieval.

**Effort:** 1 day. α/β/γ default to agentdb's `(0.7, 0.2, 0.1)`; tune later via a SolverBandit (§2).

**Risk:** low. Read-only reranker over existing retrieval results.

---

## 5. ExplainableRecall Merkle certificates → ME-JEPA Stage 0 + Stage F audit

**Borrowing:** `src/controllers/ExplainableRecall.ts:37-90` (the `RecallCertificate` shape + `MerkleProof` chain).

**ME-JEPA Stage 0** (`backupdocs/mejepa/04_data_contract_and_privacy_gate.md`) requires the legal gate to record:
- title document or sale/license document,
- privacy policy/consent review,
- client confidentiality review,
- approved data-use matrix,
- operating environment approval.

These are *exactly* the artifacts a Merkle certificate is designed to bind together. A `Stage0Certificate` for a corpus would carry:
- `merkleRoot` over `(consent_doc_hash, jurisdiction_tag, allowed_state)` per record,
- `proofChain` so any audit can verify a specific record's allowed-state without re-reading every consent doc,
- `policyVersion` (which version of the privacy gate logic was applied),
- `accessLevel` (matches agentdb's `'public'|'internal'|'confidential'|'restricted'`).

**ME-JEPA Stage F** (`backupdocs/mejepa/08_product_readout_heads.md`) — every read-out-head answer should ship with a certificate explaining which trajectory IDs justified the answer (the `chunkIds` + `minimalWhy` fields). For a recruiting product, this is the difference between "the model says hire" and "the model says hire because trajectory A (similar successful placement), trajectory B (matching skills profile), and trajectory C (compensation alignment), Merkle-rooted."

**Concrete shape (Rust):**
```rust
pub struct ReadoutCertificate {
    pub id: Uuid,
    pub query_id: String,
    pub readout_head: String,         // 'compensation_band', 'match_quality', etc.
    pub trajectory_ids: Vec<String>,
    pub minimal_why: Vec<String>,     // minimum subset that justifies the answer
    pub redundancy_ratio: f32,
    pub completeness_score: f32,      // fraction of declared requirements met
    pub merkle_root: [u8; 32],
    pub source_hashes: Vec<[u8; 32]>,
    pub proof_chain: Vec<MerkleProof>,
    pub policy_proof: Option<String>, // legal/compliance attachment
    pub policy_version: String,
    pub access_level: AccessLevel,
    pub latency_ms: u64,
}
```

**Effect on auditability:** ME-JEPA's compliance posture goes from "we recorded the data we used" to "we cryptographically bound every read-out to its justifying corpus subset." This is a real differentiator for the recruiting domain.

**Effort:** 2 days (shared with ccreality §4 — same Merkle code, two consumer types).

**Risk:** low. Cryptographic, append-only.

---

## 6. SparsificationService PPR → DynamicJEPA trajectory-importance scoring

**Borrowing:** `src/controllers/SparsificationService.ts` (492 LOC) — Personalized PageRank, random walk sampling, spectral sparsification.

**Use case:** in DynamicJEPA, trajectories form a graph (nodes = panel states, edges = transitions). When training the predictor, you want to weight trajectories by how *informative* they are — long isolated trajectories that teach unique transitions matter more than redundant cluster-centroid trajectories.

PPR with restart probability α=0.15 (the agentdb default) gives a node-importance ranking that is sensitive to local graph structure — exactly what you want for "which transitions are central to the dynamics?"

**Concrete change:** add a `dynamicjepa::trajectory::importance::personalized_pagerank` module. Per training-run preprocessing:
1. Build the panel-transition graph from this run's trajectories.
2. Run PPR with α=0.15, 20 iterations, convergence threshold 1e-6 (matching agentdb defaults).
3. Persist per-trajectory importance scores into `CF_DJEPA_TRAJECTORY_IMPORTANCE`.
4. The training data sampler weights trajectories by importance.

**Effect on training quality:** the predictor focuses on transitions that are central to the dynamics, not over-fit to long flat trajectories.

**Effort:** 2 days (the PPR algorithm is straightforward; integration into the training sampler is the larger piece).

**Risk:** medium. PPR can over-weight high-degree nodes; verify against a uniform-sampling baseline before promoting.

---

## 7. MincutService → DynamicJEPA trajectory leakage check + ME-JEPA train/dev/test split

**Borrowing:** `src/controllers/MincutService.ts` (434 LOC).

**Use case 1 — DynamicJEPA leakage check** (`backupdocs/5090jepa/09_trajectory_and_dataset.md` mentions leakage constraints):

When you split trajectories into train/dev/test, you want minimum overlap of "panel state ancestors" across splits. Mincut on the panel-derivation graph gives a cut that minimizes shared ancestor states. This is what protects against the predictor seeing essentially the same state in train and test.

**Use case 2 — ME-JEPA actor split** (`backupdocs/mejepa/06_sequence_dataset_and_training_objective.md` constrains splits per actor role):

The constraint is "no candidate's events appear in both train and test." This is again a graph-cut problem — the actor-role bipartite graph between trajectories and persons. Mincut over that graph picks the split that respects the constraint with minimum data loss.

**Effort:** 2 days (shared between use cases — the algorithm is the same, the graph construction differs).

**Risk:** medium. Mincut is well-understood; the failure mode is constructing the wrong graph.

---

## 8. TemporalCompressor 5-tier policy → DynamicJEPA panel cold-store

**Borrowing:** `src/backends/rvf/AdaptiveIndexTuner.ts` (`TemporalCompressor` class) + the `(none, half, pq8, pq4, binary)` tier policy.

**Use case:** ME-JEPA / DynamicJEPA panel materialization (`backupdocs/dynamicjepa/06_latent_panel_and_binding.md`) writes a *lot* of panels — high-dim latents per event per actor per role. Most of those panels are touched once at training and never again. A 5-tier compression policy with bandit-picked tier per panel-id keeps recent training data uncompressed and ages cold panels into 96% savings.

**Why this is interesting beyond ContextGraph's existing PQ-8:** ContextGraph has binary "quantize? yes/no." The 5-tier policy lets you tune the tradeoff per row, including the Matryoshka truncation idea (truncate from 768D → 192D for cold tiers).

**Effort:** 2 days (Rust port of the 5-tier policy + bandit hook).

**Risk:** medium. Make sure the de-quantization path is bit-exact relative to the training-time embeddings — JEPA latents are sensitive.

---

## 9. ContextSynthesizer-style narrative summary → DynamicJEPA verify-run human-readable report

**Borrowing:** `src/controllers/ContextSynthesizer.ts` (deterministic, no model calls).

**Use case:** `dynamicjepa verify-counter-world` and `verify-gridworld` produce JSON evidence logs (per `backupdocs/5090jepa/14_full_state_verification_protocol.md`). For an operator to skim those quickly, a deterministic narrative summary `{summary, patterns, successRate, averageSurprise, recommendations}` is much faster than reading raw JSON.

**Concrete shape:** add a `dynamicjepa report --run <id>` CLI subcommand that emits a Markdown narrative built from the run's evidence log + counters. No LLM, no flake — same evidence in, same narrative out.

**Effort:** 1 day. Heuristics over typed records; trivially testable.

**Risk:** trivial.

---

## 10. MMR diversity reranker → DynamicJEPA constellation candidate selection

**Borrowing:** `src/controllers/MMRDiversityRanker.ts` (187 LOC).

**Use case:** ME-JEPA's constellation/anti-pole compiler (`backupdocs/mejepa/05_latent_panel_and_schema.md`) selects k anchor exemplars per constellation. Today, the most similar k. With MMR (λ=0.5), k *diverse* exemplars that span the constellation's range. This produces stronger anchors — ones that pin down the constellation manifold rather than its centroid.

Same for anti-pole selection (k diverse forbidden regions, not k near-identical ones).

**Effort:** 0.5 day (shared with ccreality §7).

**Risk:** trivial.

---

## 11. Reflexion-style "critique" field on every training-run record

**Borrowing:** `Episode.critique: string` (agentdb's free-text "what went wrong / what was insightful").

**Use case:** DynamicJEPA training runs have a `metrics.json` file (per `backupdocs/5090jepa/11_training_runs_and_artifacts.md`). Adding a free-text `critique` field — written by the operator OR by the verifier OR by an LLM agent — gives future training-run orchestrators something semantic to query against. "Find me past runs where the critique mentions VICReg variance collapse."

The critique field is also the right hook for ME-JEPA's anti-pole anchoring: when an evaluation flags a forbidden-region drift, the critique records *what kind* of drift, in human-readable form.

**Effort:** 0.5 day. Just a schema extension.

**Risk:** trivial.

---

## 12. LearningSystem `transfer` semantics → cross-domain-pack skill transfer

**Borrowing:** `src/controllers/LearningSystem.ts:622-637` — the `learning_transfer(source, target, min_similarity, transfer_type)` interface.

**Use case:** DynamicJEPA explicitly aims for cross-domain transfer (`backupdocs/dynamicjepa/02_core_thesis.md`: "any domain can register a domain pack" + skills are reusable across packs). The agentdb interface is exactly the right shape:

```rust
pub fn transfer_skills(
    source_pack: &DomainPackId,
    target_pack: &DomainPackId,
    options: TransferOptions {
        min_similarity: f32,            // 0.7 default — agentdb default
        transfer_kinds: Vec<TransferKind>, // [Skills, Trajectories, CausalEdges, All]
        max_transfers: usize,           // 10 default
    },
) -> TransferReport;
```

The implementation: project skills' panel exemplars from the source pack's instrument space into the target pack's instrument space (where instruments overlap), keep transfers above `min_similarity`, mark them with `transferred_from: <source_pack_id>` for provenance.

**Effort:** 3 days (the projection across instrument spaces is the actual work; the API is small).

**Risk:** medium. Cross-pack semantics are subtle — verify with the counter_world ↔ gridworld pack pair as the smoke test.

---

## Priority order for JEPA

If you ship one transplant, ship **§1 (SHAKE-256 witness chain)**. It's the strongest evidence story for FSV and works across all three JEPA variants identically.

If you ship two, add **§5 (ExplainableRecall Merkle certificates)**. ME-JEPA Stage 0 / Stage F are dramatically harder to defend without it.

If you ship three, add **§2 (SolverBandit for planner / read-out heads)**. Active exploration is the difference between "the predictor's best guess always wins" and a system that improves online.

If you ship four, add **§3 (NightlyLearner doubly-robust uplift)** — but only after §1 + §2 are deployed, because §3 piggybacks on the witness chain for audit and on the bandit for arm scoring.

The remainder (§4, §6, §7, §8, §9, §10, §11, §12) are independent quality wins; sequence them by your roadmap.

---

## Cross-references

- `backupdocs/5090jepa/00_TASK.md` — the demo definition of done that §1 directly supports.
- `backupdocs/dynamicjepa/12_implementation_roadmap.md` — the natural sequencing for §2/§3/§4.
- `backupdocs/mejepa/04_data_contract_and_privacy_gate.md` — the legal gate that §5 binds.
- `docs/agentdbfindings/03_REALITY_LOOP_APPLICATIONS.md` — same primitives, different consumer.
- `docs/agentdbfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` — effort triage for the JEPA proposals.
- `docs/agentdbfindings/06_RISKS_AND_NON_TRANSFERABLE.md` — JEPA-specific "do not steal" items.
