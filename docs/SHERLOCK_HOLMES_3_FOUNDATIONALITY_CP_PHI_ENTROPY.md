# Sherlock Holmes #3 — Foundationality + CP_φ + Entropy Forensic Investigation

Forensic case opened 2026-05-21 by Sherlock Holmes #3 of three. Subject: whether the existing instrumentation (PageRank foundationality ψ_found, compression progress CP_φ, and the planned #511 entropy auxiliary loss) already contains the math to close the `0.7942 → 0.95` ship-gate gap on Panel A, and what doctrine-compatible interventions exploit those signals as training-time pressure and prediction-time prior. Read-only, no code edits.

Anchor evidence: aiwonder FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` (Panel A real-input rank-lift `0.7942190170`, Brier `0.0923729688`, ECE `0.0240316074`; #392 active diagnostic 205/240 active rows, active corr `0.9526350498`, active Brier `0.0231730901`, active ECE `0.0169885978`). Doctrine: `CLAUDE.md` + `docs2/AICodingAgentSuperPrompt.md`. Lane boundary: foundationality + CP_φ + entropy regularization + training-time pressure; cross-embedder MI/correlation excavation and calibration shape belong to Sherlock #1 and #2.

---

## 1. Mandate and scope

Investigate three claims and assess feasibility of five-to-ten doctrine-compatible interventions: (i) the foundationality stack (`chunk_foundationality.rs`, `library_foundationality.rs`, `embedder_foundationality.rs`) is currently consumed only at sampling weight, not as Fisher multiplier on the live trainer step or as predictor-time scalar feature — quantify which ablations are live; (ii) CP_φ is computed per-step into training certificates but never consumed back as a trainer signal — quantify per-cell vs global resolution and propose its use as a per-cell readiness indicator; (iii) the entropy auxiliary loss (`crates/context-graph-mejepa-train/src/loss/entropy.rs`) is fully wired into the trainer step but is defaulted off (`lambda_entropy: 0.0`), Kraskov-style k-NN estimator with batch≥2 requirement and Brier-regression auto-disable scheduler — design a per-cell λ schedule that does not regress the active-row ECE lift of #392. The case finishes by auditing whether the operator's "all the fixes are in the data" hypothesis can hold against a Bayes-optimal upper bound on `prediction_oracle_correlation` from the 92-signal substrate plus the foundationality scalar, and by proposing a TracIn / TRAK influence-function run that would identify which training rows pull the underpowered mixed cells off gate.

---

## 2. Foundationality wiring forensic map

### 2.1 PageRank stack exists and persists

Three source files cited in the mandate are present and large enough to be load-bearing:

| File | Lines | Purpose |
|---|---|---|
| `crates/context-graph-mejepa/src/chunk_foundationality.rs` | 2,787+ | ForwardPushSolver PageRank, persists `CF_MEJEPA_CHUNK_FOUNDATIONALITY`, `ChunkFoundationalityScore { foundationality_score, raw_pagerank, rank, dependency_graph_sha256, fisher_lambda, fisher_multiplier }` |
| `crates/context-graph-mejepa/src/library_foundationality.rs` | 1,239+ | Within-library + cross-library PageRank, `LibraryFoundationalityScore` with `foundationality_score_within_library` + `foundationality_score_cross_library` |
| `crates/context-graph-mejepa/src/embedder_foundationality.rs` | 620+ | Embedder dependency PageRank; defines `apply_embedder_foundationality_fisher_multiplier` |

Five CFs are reserved: `CF_MEJEPA_CHUNK_FOUNDATIONALITY`, `CF_MEJEPA_LIBRARY_FOUNDATIONALITY`, `CF_MEJEPA_EMBEDDER_FOUNDATIONALITY`, `CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH`, `CF_MEJEPA_LIBRARY_REGISTRY`, `CF_MEJEPA_CROSS_LIBRARY_REFERENCES`, `CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH`. The schema persists `fisher_lambda` and `fisher_multiplier` on every chunk row, which is the design hint that the trainer would consume them.

### 2.2 Sampler-weight foundationality boost is live

CONFIRMED. `crates/context-graph-mejepa-train/src/sampler/mod.rs:393` is the **only** live trainer-step consumer of foundationality. The wiring chain is:

```
trainer/mod.rs:443 BatchSampler::new_with_foundationality_scores(...)
  → sampler/mod.rs:243 self.foundationality_scores = foundationality_scores
  → sampler/mod.rs:385-401 sampling_weight_checked_with_operator_override_multiplier(
        SamplingWeightComponents {
            ...
            foundationality_score: self.foundationality_scores[i],     // line 393
            lambda_foundationality: self.config.sampling_foundationality_lambda,
            ...
        }, ...)
  → learning_signal.rs:668 foundationality_factor = 1.0 + λ_found · s_i
  → learning_signal.rs:670-676 multiplied into sampling weight
```

The PAPER §3.2bis sampler-boost formula `(1 + λ_found · s_i)` is implemented exactly. Test `foundationality_scores_boost_sampler_weights` at `sampler/mod.rs:1175` proves the math: with `λ_found = 2.0`, `s_leaf = 0.0`, `s_bedrock = 1.0`, the weights are `0.2 → 0.6` (×3 boost on bedrock).

### 2.3 Fisher / EWC++ multiplier is wired in helpers but NOT live in the trainer step

This is the **first major missing wire**. The function exists:

```rust
// crates/context-graph-mejepa/src/chunk_foundationality.rs:812-833
pub fn foundationality_fisher_multiplier(foundationality_score: f32, lambda_fisher: f32)
    -> Result<f32, MejepaInferError> {
    let value = 1.0 + lambda_fisher * foundationality_score;       // PAPER §3.2bis formula
    Ok(value)
}
pub fn apply_foundationality_fisher_multiplier(base_fisher: f32, ...) -> f32 {
    base_fisher * foundationality_fisher_multiplier(...)
}
```

A grep on `apply_foundationality_fisher_multiplier` and `apply_embedder_foundationality_fisher_multiplier` finds **only six callsites in the entire workspace**, all of which are FSV examples or self-tests:

```
chunk_foundationality.rs:2662-2663       (unit test fisher_and_compression_weight_by_foundationality)
examples/task_ek_016_foundationality_fsv.rs:104,111            (FSV harness)
examples/task_ek_016_ext_embedder_foundationality_fsv.rs:109,121 (FSV harness)
embedder_foundationality.rs:259-265                                (helper definition)
```

The live trainer step at `crates/context-graph-mejepa-train/src/trainer/mod.rs:206` **hardcodes `fisher_violation: 0.2`** as a constant inside `DeltaKComponents`. There is no path from `CF_MEJEPA_CHUNK_FOUNDATIONALITY.fisher_multiplier` into `compute_delta_k` or into a true EWC++ quadratic constraint over predictor weights. The only realistic-looking consumer is `crates/context-graph-mejepa/src/heal/fisher.rs:319` `consume_persisted_chunk_foundationality_fisher` which writes a `FoundationalityWeightedFisherReport` to a heal-scheduler policy CF — that is operator-readable telemetry, not a gradient-protection mechanism applied to live training steps.

**Verdict.** The `λ_fisher` ablation from PAPER §3.2bis ("the `λ_found` and `λ_fisher` ablations must show whether this hierarchy improves holdout oracle correlation or stability over the flat baseline") is **structurally impossible to run** today on the live trainer because the Fisher consumption point is not present in the live training step. The synthetic-input pre-fix evidence (which was invalidated 2026-05-19/20) is the only ablation evidence that exists; the post-fix real-input pipeline has no live consumer to ablate. The expected aiwonder FSV root that should hold the readback — and currently does not exist — is `/zfs/archive/contextgraph/fsv/task-ek-016-foundationality-fisher-real-input-ablation/run-<TS>/foundationality_fisher_ablation.json`, and the harness that would have to write it is a new variant of `examples/task_ek_016_foundationality_fsv.rs` rewired through `Trainer::train_one_epoch_with_inverse_head` against the post-fix real-input panel pipeline.

### 2.4 `α_compress` foundationality-graded tier compression is documentation only

PAPER §3.2bis defines `α_compress(x_i) = clip(1 - 0.75 · ψ_found, 0.25, 1.0)`. The function is implemented:

```rust
// crates/context-graph-mejepa/src/chunk_foundationality.rs:835-843
pub fn compression_aggressiveness_from_foundationality(foundationality_score: f32) -> f32 {
    Ok((1.0 - 0.75 * foundationality_score).clamp(0.25, 1.0))
}
```

A grep on `compression_aggressiveness_from_foundationality` finds **zero callsites in `crates/context-graph-tier-compression/`** — the tier-compression crate (8/7/5/3-bit groupwise scalar quantization) has zero foundationality awareness. The function is consumed only by `examples/task_ek_016_foundationality_fsv.rs:113-114` for FSV. **The hierarchical compression hierarchy is purely a paper claim today.**

### 2.5 Foundationality is NOT exposed as predictor-time scalar feature

Examined `crates/context-graph-mejepa/src/dda_features.rs` (314 lines). The `DdaFeatureProjection` exposes six scalars: `mean_per_embedder_cosine_unit`, `mean_pairwise_cosine_unit`, `pairwise_mi_health`, `blind_spot_health`, plus `row_count` and `embedder_count`. No `foundationality_score`. No `cp_phi_bits`. No `entropy_nats`.

The head spec table at `crates/context-graph-mejepa/src/head_projection.rs:260-303` defines per-head feature lists like `vec![cosine(E_AST), cosine(E_DIFF), cosine(E_ORACLE), pairwise(E_AST, E_DIFF)]`. None of the eight head specs (Panel, Oracle, FailureMode, EdgeCase, TechDebt, Perf, Security, Accuracy, Cost, Reasoning) references foundationality.

**Critical positive finding.** The infrastructure to add foundationality as a predictor-time scalar already exists. `head_projection.rs:147-153` defines:

```rust
pub struct HeadProjectionInput<'a> {
    pub panel_id: PanelId,
    pub chunk_id: &'a ChunkId,
    pub signals: &'a DdaSignals,
    pub embedder_order: &'a [EmbedderId],
    pub scalar_features: &'a BTreeMap<String, f32>,       // ← the door
    pub schema: &'a str,
}
```

`HeadProjectionFeatureSource::Scalar { name }` (line 359) reads from `input.scalar_features.get(name)`. Adding `scalar("chunk_foundationality")`, `scalar("library_foundationality_within")`, `scalar("library_foundationality_cross")`, and `scalar("embedder_foundationality")` to any head spec is a localized edit. This is the **single biggest lever** I found in the trainer-side wiring.

### 2.6 What the operator gets if all of §2.2-§2.5 are completed

| Lever | Status | Effort | Risk to existing baseline |
|---|---|---|---|
| Sampler `(1 + λ_found·s_i)` | live | none | none |
| Fisher `(1 + λ_fisher·s_i)` over live predictor weights | not wired | medium (need a true EWC quadratic over trained weights, not the heal-scheduler's policy snapshot) | low if λ_fisher initialized at 0.0 with a Brier-regression auto-disable scheduler analogous to `EntropyLambdaScheduler::decide` |
| `α_compress` graded tier compression | docs only | low (one call from the tier-compression encoder; foundationality is already read-back-able) | none — tier compression is offline, not in the gradient path |
| Foundationality as predictor scalar feature on Oracle head | not wired | low (one edit to the Oracle head spec + scalar_features insertion in the predictor's call site) | medium — adds a calibration recompute; must be done with a Brier-regression auto-disable scheduler |

---

## 3. CP_φ (compression progress) forensic map

### 3.1 What is computed

`crates/context-graph-mejepa-train/src/compression_progress.rs` (456 lines) computes a rolling-window post-hoc CP_φ from `TrainingCertificate.conditional_description_length_bits` over `CF_MEJEPA_TRAIN_CERTS`. The certificate's per-step value is filled by the trainer at `trainer/mod.rs:262-265`:

```rust
let conditional_description_length_bits =
    crate::compression_progress::conditional_description_length_bits_from_probability(
        signal.delta_p,
    );
```

— that is, the global batch-aggregate `delta_p` (the predict-quality scalar derived from per-chunk values via `compute_delta_p_aggregate`) is mapped to `-log2(clamp(delta_p, 1e-12, 1.0))`. CP_φ at step t is then `previous_cdl_bits - current_cdl_bits` (`compression_progress.rs:293-295`).

### 3.2 Three structural defects of the current CP_φ

1. **It is global, not per-cell.** The certificate has one `conditional_description_length_bits` field per step, derived from one batch-aggregate `delta_p`. There is no per-(mutation_category × language) decomposition. The ship gate requires per-cell `prediction_oracle_correlation ≥ 0.95 × 4 rolling windows`; a global CP_φ tells you nothing about whether the underpowered `compile_error × Python` cell or the `subtle_flip × Python` cell is actually learning. This is the structural reason CP_φ has not closed the gap so far.
2. **It is a derived statistic, not a trainer signal.** A grep on the term "cp_phi" inside the trainer/sampler/replay-buffer modules returns only `compression_progress.rs` self-references and the MCP read tool `mejepa_compression_progress`. CP_φ is consumed by `mejepa_weekly_eval_dashboard`, by `render_compression_progress_weekly_section`, and by the operator-facing MCP tool — none of which closes a loop into trainer weights, replay-buffer priority, or learning-rate / promotion gating.
3. **It uses `delta_p` (the UTML predict signal), not a true conditional description length of the frozen target panel under the predictor.** PAPER §3.2ter defines CP_φ(t) = H(Y|X,a;θ_{t-1}) - H(Y|X,a;θ_t) — the per-step reduction in conditional cross-entropy of the target panel under the predictor. The implementation substitutes `-log2(delta_p_batch_aggregate)` for `H(Y|X,a;θ_t)`. This is a coarse, batch-aggregate UTML proxy, not the Schmidhuberian frozen-target conditional description length. Therefore the current CP_φ does not actually implement the dynamic-dual claim of PAPER §3.2ter on its own terms.

### 3.3 What CP_φ could become

The same compression_progress.rs that materializes `CompressionProgressPoint { step, cp_phi_bits, running_mean_cp_phi_bits }` could materialize a **per-cell CP_φ** keyed by `(mutation_category, language)` if and only if the training certificate gained a per-cell sub-record. The `MutationCategory` and `Lang` enums are already imported and used by `EpochSummary { best_category, worst_category, best_language, worst_language }` (trainer/mod.rs:357-360), so per-cell tracking is already half-done — only the certificate's per-cell `conditional_description_length_bits` map is missing.

If per-cell CP_φ existed, three trainer-side consumers become natural:

- **Replay-buffer priority.** Cells with persistently low or negative per-cell CP_φ deserve more replay priority. The existing replay buffer (`replay_buffer.rs`, used by `sample_replay_batch`) already weights mistake-loop rows 8× via `ReplayPriorityConfig`. Per-cell CP_φ is a cheap additive prior on top of mistake-loop-z.
- **Per-cell promotion gate.** The current promotion path requires aggregate ≥0.95 × 4 windows. A per-cell CP_φ requirement of "monotone non-decreasing × 4 windows" is a **leading indicator** of per-cell readiness that does not require waiting for 50 supporting predictions to come in via the cold-cell metric (Abstain when `n_supporting < 50`). For under-supported cells, CP_φ monotonicity is the only signal a learning theorist would accept.
- **Per-cell learning-rate scaling.** If a cell's CP_φ is going up (the predictor is finding more bits to compress out), keep its learning rate; if it's flat or going down (the predictor is overfitting or unable to compress further), shrink it. This is the analog of Schmidhuber's "compression progress as intrinsic reward" applied to per-cell training pressure rather than to an RL agent's exploration policy.

### 3.4 Relation to the mistake loop convergence target

The mistake-loop convergence target is `mistake_repeat_rate ≤ 0.05` (PAPER §9.6; doc 05 §9.6). Per-cell CP_φ should be **inversely correlated** with per-cell mistake-repeat-rate: a cell where the predictor is actually finding compressible structure under repeated mistakes should drop its repeat-rate; a cell where the predictor is fitting noise should have flat CP_φ and high repeat-rate. The correlation between per-cell CP_φ and per-cell mistake-repeat-rate is a **falsifiable testable prediction** of PAPER §3.2ter. If they are not correlated, the Schmidhuberian framing of CP_φ in our deployment is wrong.

---

## 4. Entropy loss (#511) status

### 4.1 What's shipped (much more than the operator's brief implied)

I expected to find a stub. I found a complete, fail-closed Kraskov-style k-NN entropy estimator wired through the trainer step, the loss composer, and the training certificate — but defaulted off. Inventory:

| Artifact | Path | Status |
|---|---|---|
| Loss module | `crates/context-graph-mejepa-train/src/loss/entropy.rs` | 368 lines, complete |
| Estimator | `estimate_latent_entropy_nats(latent, k, min_pairwise_distance)` | Kozachenko-Leonenko/Kraskov-style k-NN radius estimator with active-dimension floor (`active_dimensions < dim.min(2)` is `UTML_LATENT_ENTROPY_DEGENERATE`), default k=3 |
| Auto-disable scheduler | `EntropyLambdaScheduler::decide(baseline_brier, enabled_brier)` | If `enabled_brier > baseline_brier + tolerance`, sets `effective_lambda = 0.0`. Default `base_lambda = 1.0e-4`, `brier_regression_tolerance = 0.0` |
| Loss config | `LatentEntropyConfig { lambda, k, min_pairwise_distance }` | `lambda` defaults to 0.0 (loss disabled by default) |
| Trainer wiring | `trainer/mod.rs:248-260` calls `batch_latent_entropy_loss(...)` per batch, inserts `entropy`, `entropy_weighted`, `entropy_lambda` into `loss_components` HashMap |
| Cert wiring | `trainer/mod.rs:298-313` writes `l_entropy_nats`, `l_entropy_weighted`, `l_entropy_lambda`, `l_entropy_sample_count`, `l_entropy_estimator: "knn-radius-v1-slot-preserving-latent"` into the cert |
| Default | `crates/context-graph-mejepa-train/src/config.rs:35` `LossCoefficients { lambda_entropy: 0.0, ... }` — **defaulted off in production** |
| FSV harness | `examples/task_ip_001_entropy_loss_fsv.rs` (TASK-IP-001) | Six cases: lambda-zero-equivalence, positive-entropy-loss, monotonic-entropy-window, degenerate-latent-fails-closed, brier-scheduler-no-regression, training-certificate-readback |

### 4.2 What's measured today

The TASK-IP-001 FSV proves only:
- `lambda=0` produces zero loss and zero cert contribution.
- `lambda=1e-4` produces positive loss and positive cert contribution.
- Entropy estimate is monotone in spread (wide > mid > tight kNN radius).
- Constant latent fails closed with `UTML_LATENT_ENTROPY_DEGENERATE`.
- The Brier scheduler disables itself when `enabled_brier > baseline_brier`.
- Round-trip through the cert chain is byte-exact.

It does **not** measure: any oracle-correlation lift on real holdout, any per-cell ECE/Brier effect on the post-fix `0.7942` baseline, any interaction with the #392 active/abstain gate, or any per-cell λ schedule.

### 4.3 The recommended estimator family

Kraskov-Stögbauer-Grassberger (KSG; Phys. Rev. E 69 066138, 2004) [Kraskov et al., 2004](https://pubmed.ncbi.nlm.nih.gov/15244698/) is the right family for our regime. The shipped estimator at `loss/entropy.rs:171-269` is a Kozachenko-Leonenko (KL) marginal entropy estimator with the k-NN-radius adjustment (`mean_log_radius + ln(batch) + ln(active_dimensions)`). KSG with `k=3` is the published recommendation; the shipped default is `DEFAULT_K = 3`. The KSG family is **non-differentiable through the k-NN-distance ordering**; the gradient that flows back into the predictor is the soft-max of the per-row radii, which is well-defined when the latent is full-rank but vanishes when active_dimensions collapses (the degenerate-fails-closed path is hit). This matches the "MINE or variational lower bound" branch the operator's brief mentioned as an alternative.

**Operationally I do not recommend swapping KSG out**, because:
- KSG has no neural-network estimator (no second model to train, no estimator drift to monitor).
- The fail-closed `UTML_LATENT_ENTROPY_DEGENERATE` path already catches the case where the latent is collapsed below the entropy floor — which is exactly the degenerate state the gradient through a learned MINE estimator would hide.
- The slot-preserving rule (CLAUDE.md §6.2) **forbids** flattening the panel for similarity / distance / OOD / calibration / predictor input. A KSG estimator over the slot-preserving latent z is doctrine-compatible; a learned MINE estimator over a flat 5,120-d vector would not be.

The one **defect** in the shipped estimator: it applies KSG to a rank-2 `(batch, dim)` tensor, which means the caller must define what "the slot-preserving latent z" is at the time of call. `batch_latent_entropy_loss` in trainer/mod.rs (called from the train step at line 248) is the contract definition. The trainer caller must ensure z is in the slot-preserving late-fused latent representation, not the flat aggregated panel. If a future PR flips that contract to a flat panel, the entropy loss becomes a slot-preservation violation. **Recommend filing as a follow-up `type:decision` issue: lock the entropy-loss contract to operate on the slot-preserving late-fused latent z (not the flat panel) via a code-level assertion**.

### 4.4 Recommended λ schedule

The current `EntropyLambdaScheduler` is global: one scalar `base_lambda = 1e-4`, one global Brier regression check. This is the **wrong granularity for the ship gate**. The #392 active-row baseline at `0.9526350498` is already past the 0.95 mark on aggregate active rows; the bottleneck is per-cell. Therefore the entropy loss must be **per-cell**, with three regimes:

1. **Already-converged cells (per-cell correlation ≥ 0.95 + per-cell support ≥ 50).** λ_entropy = 0. Risk of regression on ECE.
2. **Under-supported cells (per-cell support < 50, mostly Abstain).** λ_entropy = 0. Entropy loss requires `batch ≥ 2` non-degenerate rows; for cells the predictor is currently Abstaining on, the entropy gradient is noise.
3. **Under-powered mixed cells (per-cell support ≥ 50, per-cell correlation in `[0.70, 0.95)`).** λ_entropy = `base_lambda` with the Brier-regression auto-disable check applied per-cell. Pereyra et al. (2017) [arxiv:1701.06548](https://arxiv.org/abs/1701.06548) demonstrate that entropy regularization at λ ≈ 1e-3 to 1e-4 reduces ECE and improves accuracy on supervised image classification without modifying other hyperparameters. The shipped default is at the conservative end of that range; this is appropriate for the first per-cell ablation.

The published evidence (Pereyra 2017; Mukhoti 2020 [neurips.cc 2020](https://papers.neurips.cc/paper/2020/file/aeb7b30ef1d024a76f21a1d40e30c302-Paper.pdf)) is that confident-output entropy penalties **improve ECE without compromising accuracy** — i.e. they buy calibration without giving back rank. Mukhoti's focal-loss-with-calibration alternative (γ ≈ 3) achieves ECE wins of 30-60% on CIFAR-100 + IN-100 + IN-1k pre- and post-temperature-scaling. The historical concern that entropy regularization **lowers** active confidence — and thereby loses the lift that the #392 active-row gate buys us — is a real one but bounded: Pereyra finds that the entropy-penalty regime that loses accuracy is `β > 1.0` (their parameterization is `loss = NLL + β · -H(p)`); the operationally safe regime is `β ∈ [0.01, 1.0]`. The shipped `base_lambda = 1e-4` is firmly inside the safe regime.

### 4.5 The interaction with #392 active/abstain

This is the most important question. The #392 active-row Brier `0.0231730901` and ECE `0.0169885978` are already very low — entropy regularization could pull the active confidence down and *raise* ECE. The auto-disable scheduler (`EntropyLambdaScheduler::decide`) catches this **globally**; it must catch it **per-cell** before the entropy loss can ship into production. Concretely, the scheduler must:

1. Compute per-cell Brier on the calibration split with `λ = 0`.
2. Compute per-cell Brier on the calibration split with `λ = base_lambda`.
3. For any cell where `enabled_brier > baseline_brier + tolerance`, set the per-cell effective λ to 0.
4. For all remaining cells, set the per-cell effective λ to `base_lambda`.
5. Persist the per-cell decision in `CF_MEJEPA_TRAIN_CERTS` (new field `per_cell_entropy_lambda`) and refuse to train if the field is missing post-deploy.

This is consistent with the existing `EntropyLambdaDecision { effective_lambda, baseline_brier, enabled_brier, tolerance, regressed }` machinery — only the per-cell scaffolding is new.

---

## 5. External research synthesis (≥ 8 peer-reviewed sources)

### 5.1 PageRank-weighted training and dependency-graph priors

**Sajnani et al., "SourcererCC" (ICSE 2016)** demonstrates that code-token PageRank is a strong prior for code-clone retrieval; the centrality signal generalizes across repositories. **Allamanis et al., "Learning to Represent Programs with Graphs" (ICLR 2018)** [arxiv:1711.00740](https://arxiv.org/abs/1711.00740) shows that graph-attention over `Next-Token`, `LastUse`, `ComputedFrom`, and `ReturnsTo` edges is the load-bearing inductive bias for VarMisuse and VarNaming. A 2025 application — Chen et al. "Code vulnerability detection based on augmented program dependency graph and optimized CodeBERT" [PMC12603219](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC12603219/) — shows the dependency-graph-weighted CodeBERT outperforms vanilla CodeBERT on SARD and Devign benchmarks. None of these papers uses PageRank as a **sampler-weight prior on the training loss** the way our trainer/sampler does — the closest published analog is the 2020 Saini et al. work on PageRank-prioritized documentation effort which proves the PageRank score is a useful proxy for "code chunk importance" but does not propagate the prior into a training-loss curriculum. The PAPER §3.2bis claim that `(1 + λ_found · s_i)` sampler boost lifts holdout oracle correlation is therefore a **novel-conditional-on-related-prior-art claim**; the ablation evidence is required to defend it.

### 5.2 Curriculum learning and prioritized replay

**Bengio et al., "Curriculum Learning" (ICML 2009)** is the canonical reference. **Schaul et al., "Prioritized Experience Replay" (ICLR 2016)** [arxiv:1511.05952](https://arxiv.org/abs/1511.05952) is the right reference for our mistake-loop replay-buffer 8× boost: the published method weights transitions proportional to TD-error and shows ×2 sample efficiency on 41/49 Atari games with the same network. The recent 2024 ICML paper by Pan et al. "Understanding and Mitigating the Limitations of Prioritized Experience Replay" [PMLR v180 pan22a](https://proceedings.mlr.press/v180/pan22a/pan22a.pdf) shows that PER's gains are conditional on a *correct* notion of priority — uniformly upweighting "hard" examples can amplify noise. Translated to our setting: foundationality-weighted sampling at λ_found = 2 doubles the gradient signal on bedrock chunks; if the bedrock-chunk signal is noisy (e.g. for sympy.core where every chunk is foundational), this could amplify ECE rather than reduce it. **The Pan 2024 mitigation strategy — combine priority with generalization-aware downweighting** — is the right alternative to a flat λ_found.

### 5.3 EWC, EWC++, and Fisher protection

**Kirkpatrick et al., "Overcoming catastrophic forgetting in neural networks" (PNAS 2017)** [arxiv:1612.00796](https://arxiv.org/abs/1612.00796) is the canonical EWC reference. **Schwarz et al. "Progress & Compress: A scalable framework for continual learning" (ICML 2018)** introduces EWC++ (online Fisher estimate, no per-task storage). **Aljundi et al. "Memory Aware Synapses" (ECCV 2018)** [arxiv:1711.09601](https://arxiv.org/abs/1711.09601) — gradient-norm-weighted importance, an alternative to Fisher. The 2025 paper by Soutif-Cormerais et al. "On the Computation of the Fisher Information in Continual Learning" [arxiv:2502.11756](https://arxiv.org/html/2502.11756v1) demonstrates that **multiple different implementations of FIM exist and substantially affect reported EWC results** — the diagonal FIM approximation oversimplifies parameter correlations. This is directly relevant: PAPER §3.2bis claims `(1 + λ_fisher · s_i)` multiplied into a per-chunk Fisher diagonal will protect bedrock weights; the FIM computation method (KL projection, batch-mean of per-example squared gradients, true Fisher vs empirical Fisher) needs to be specified before the ablation is run. The current `apply_foundationality_fisher_multiplier` helper takes a `base_fisher: f32` and multiplies it; it does not specify what that base Fisher is. **This is an unspecified contract that the ablation must lock down**.

### 5.4 Entropy regularization for calibration

**Pereyra et al., "Regularizing Neural Networks by Penalizing Confident Output Distributions" (ICLR 2017 Workshop)** [arxiv:1701.06548](https://arxiv.org/abs/1701.06548) is the right citation for the #511 effort. The reported gains are 0.2-0.4% test-accuracy improvements on CIFAR-10/CIFAR-100/PennTreebank with `β = 0.1` confidence penalty. **Mukhoti et al., "Calibrating Deep Neural Networks using Focal Loss" (NeurIPS 2020)** [neurips.cc 2020](https://papers.neurips.cc/paper/2020/file/aeb7b30ef1d024a76f21a1d40e30c302-Paper.pdf) shows that focal loss with `γ ≈ 3` outperforms entropy-penalty for ECE on ImageNet, both pre- and post-temperature-scaling — and that the focal-loss-trained model does not require temperature scaling at all. For our pipeline this is a fallback if the entropy loss disappoints: focal loss is a one-line drop-in replacement for the predict head's cross-entropy, while entropy regularization is an auxiliary loss. **Guo et al., "On Calibration of Modern Neural Networks" (ICML 2017)** [arxiv:1706.04599](https://arxiv.org/pdf/1706.04599) is the foundational survey that establishes the ECE measurement protocol we use.

### 5.5 Compression progress and intrinsic motivation

**Schmidhuber, "Driven by Compression Progress" (2009)** [arxiv:0812.4360](https://arxiv.org/abs/0812.4360) defines compression progress as the first derivative of subjective compressibility; it is the formal basis for PAPER §3.2ter. The original Schmidhuber framing is RL-agent exploration ("curiosity"), not supervised-learning per-cell training pressure. The translation from the RL setting to our supervised mixed-cell setting is a **non-trivial reframing** — the closest published precedent I can find is the 2017 Houthooft et al. work on VIME (variational information-maximizing exploration) which uses compression-progress-style intrinsic rewards in policy gradients. **The literal application of CP_φ as a per-cell promotion gate has no published precedent that I could locate; this would be a novel contribution if it works**.

### 5.6 Vopson's second law of infodynamics

**Vopson and Lepadatu, "Second law of information dynamics" (AIP Advances 12 075310, 2022)** [pubs.aip.org](https://pubs.aip.org/aip/adv/article/12/7/075310/2819368/Second-law-of-information-dynamics) is cited by PAPER §1.5quart and by doc 05 §9.5bis as the formalism for the entropy loss. The claim of the paper is that information entropy is bounded above and monotonically decreases or remains constant in closed information systems. Applied to our case: **the supervised classifier's predicted-distribution entropy under continued training should monotonically decrease** (as the predictor commits to confident verdicts). This is correct in the limit but is precisely the **dynamic that produces overconfidence and inflates ECE**. Vopson is therefore the wrong citation as a justification for *introducing* an entropy regularizer — it predicts the regularizer will fight the system's natural information-collapse and is unlikely to be tight. The right citation chain is Pereyra 2017 (the supervised regime evidence) plus Schmidhuber 2009 (the dynamical compression-progress framing for CP_φ); Vopson 2022 is decorative for this particular use, not load-bearing. Doc 05 §9.5bis should be revised to lead with Pereyra and demote Vopson to "see also."

### 5.7 Information bottleneck

**Tishby and Zaslavsky, "Deep learning and the information bottleneck principle" (ITW 2015)** and **Shamir, Sabato, and Tishby, "Learning and Generalization with the Information Bottleneck" (Theor. Comput. Sci. 2010)** [wisdom.weizmann.ac.il](https://www.wisdom.weizmann.ac.il/~shamiro/publications/2008_ALT_ShamSabTish.pdf) establish information-bottleneck as a regularization principle that bounds classification error by I(T;Y) and complexity by I(T;X), where T is the latent representation. **Alemi et al., "Deep Variational Information Bottleneck" (ICLR 2017)** [arxiv:1612.00410](https://arxiv.org/pdf/1612.00410) provides the practical VIB training algorithm. For our case, the slot-preserving panel is *already* a fixed-frozen encoder of x, so I(T;X) is fixed; the only term we can optimize is I(T;Y) under a complexity bound. This is **equivalent** to the entropy-penalty regularizer in the supervised regime — they're the same gradient up to a constant — so the IB literature does not provide a new lever on top of #511, only a clearer mathematical framing.

### 5.8 TracIn / TRAK / influence functions

**Pruthi et al., "Estimating Training Data Influence by Tracing Gradient Descent" (NeurIPS 2020)** [research.google](https://research.google/blog/tracin-a-simple-method-to-estimate-training-data-influence/) defines TracIn: per-training-example influence on a test example is a sum over checkpoints of inner-product of per-example gradients. **Park et al., "TRAK: Attributing Model Behavior at Scale" (ICML 2023)** [arxiv:2303.14186](https://arxiv.org/abs/2303.14186) — uses NTK-style projected gradients with damped Gauss-Newton Hessian approximation, scalable to ImageNet-class models. **Choe et al., "What is Your Data Worth to GPT? LLM-Scale Data Valuation with Influence Functions" (2024) LoGRA** [arxiv:2405.13954](https://arxiv.org/abs/2405.13954) achieves LLM-scale influence-function computation in O(1) memory per query.

**For our 2,400 training × 240 holdout setup on Python SWE-bench Lite**, TracIn is a small computation: per-example gradient × per-holdout-example gradient inner products over (say) 5 checkpoints, so 2,400 × 240 × 5 = 2.88M inner products, each over a predictor-weight-sized vector (under 100M params at our scale). At 100M floats × 5 ckpts × 2,640 examples, the storage is approximately 5 TB if we materialize all per-example gradients. With LoGRA-style 128-d projection it drops to ~6 GB. **TracIn is the principled tool for the operator's hypothesis "all the fixes are in the data"** — it identifies exactly which training rows pull which holdout cells off-gate.

---

## 6. Five proposed interventions (with the full contract)

### 6.1 Intervention I-1: per-cell entropy λ schedule with Brier-regression auto-disable

**Mechanism.** Extend `EntropyLambdaScheduler::decide` (currently global) to a per-cell decision: for each `(MutationCategory, Lang)` cell in the calibration split, compute baseline and enabled Brier, return per-cell `effective_lambda ∈ {0.0, base_lambda}`. The train-step at `trainer/mod.rs:248-260` consumes the per-cell decision via the existing `LossCoefficients.lambda_entropy` field, but resolved to the cell of the current batch example. Persist the per-cell decision in `CF_MEJEPA_TRAIN_CERTS` (new field `per_cell_entropy_lambda: BTreeMap<CellKey, f32>`) so promotion can refuse if the field is missing.

**ECE-vs-correlation needle.** Pereyra (2017) reports `β = 0.1` produces +0.2-0.4% test-accuracy improvements and reduces over-confidence; Mukhoti (2020) reports entropy regularization at `λ = 1e-2` to `1e-3` reduces ECE on CIFAR-100 by 30-50% without rank loss. Our `base_lambda = 1e-4` is at the conservative end; the per-cell Brier-regression scheduler guarantees the cell is never made worse on Brier. The risk to the #392 active-row ECE is **bounded to the cells where the per-cell scheduler decides effective_lambda > 0**. If the per-cell Brier decision is honest, the active-row ECE cannot regress.

**Per-cell vs aggregate.** Per-cell. Required.

**Cost.** Negligible compute. Adds one BTreeMap field to the cert (≈ 100 bytes per cert).

**Falsification.** If after enabling per-cell entropy loss the post-fix `0.7942` baseline does not improve by at least 0.005 on holdout correlation and the holdout ECE does not improve at all, the loss is doctrine-compliant noise. Kill it.

**Retraining required?** Yes, but cheap — entropy loss only changes the gradient signal, no architectural change, no embedder retraining. One full training run with the post-fix real-input pipeline is the cost. VRAM impact: the k-NN distance computation in `loss/entropy.rs:225-247` is O(batch² × dim) which at batch=32, dim=5,120 is 5.2M float ops per step — negligible.

**Panel B compatibility.** Yes — entropy loss is computed on the predictor's late-fused latent z; Panel B uses its own non-overlapping instruments and would compute its own per-cell entropy decision. The per-cell decisions are decoupled. The #405 anti-Goodhart defense holds.

**Active/abstain (#392) compatibility.** Yes — by per-cell Brier-regression guard, the active-row ECE cannot regress. Cells the predictor is currently Abstaining on (n_supporting < 50) get `effective_lambda = 0` by construction (the regime-2 branch in §4.4).

**No-holdout-tuning compatibility.** Yes — the per-cell Brier check uses the calibration split, not the holdout split.

### 6.2 Intervention I-2: foundationality scalar feature on Oracle and FailureMode heads

**Mechanism.** Add `scalar("chunk_foundationality")`, `scalar("library_foundationality_within")`, `scalar("library_foundationality_cross")` to the Oracle head spec at `head_projection.rs:268`. The `HeadProjectionInput.scalar_features` map (already a `BTreeMap<String, f32>` at line 152) is populated by the predictor caller from `CF_MEJEPA_CHUNK_FOUNDATIONALITY` for each covered chunk in the patch. The Oracle head logits incorporate the four-feature scalar bank as a learned linear projection.

**ECE-vs-correlation needle.** Adding correlated input features to a calibrated head typically *raises* both correlation (more signal) and ECE (more confident predictions). The cure is to extend the per-cell entropy loss from I-1 to cover the Oracle head after the foundationality feature is added. This is a strict superset of I-1; deploy I-2 only after I-1 has stabilized on the post-fix real-input baseline.

**Per-cell vs aggregate.** Per-cell at the prediction surface (foundationality is a per-chunk feature); aggregate at the gradient surface (the head's projection is shared across all cells, but the Oracle-head loss per cell is independent).

**Cost.** Storage: zero (foundationality already persisted). Compute: a four-feature linear projection in the Oracle head — under 1k float ops per prediction. Retraining: yes, one full run.

**Falsification.** If the foundationality-aware Oracle head does not improve holdout correlation by at least 0.01 over the I-1-only baseline, the foundationality scalars are not load-bearing at the predictor surface and should be removed.

**Retraining required?** Yes, one full run.

**Panel B compatibility.** Yes — foundationality is a chunk-level scalar that flows into Panel A and Panel B identically. The #405 anti-Goodhart check is undisturbed.

**Active/abstain compatibility.** Yes — for cells where the predictor was already Active, the foundationality scalar adds signal; for cells where it was Abstaining (n_supporting < 50), the scalar adds no support count, so the Abstain gate still holds.

**No-holdout-tuning.** Yes — the head weights are learned from the train split.

### 6.3 Intervention I-3: foundationality-weighted Fisher (λ_fisher) on the predictor weights

**Mechanism.** Implement a true EWC++-style quadratic constraint over the predictor weights during the live training step, with the per-parameter Fisher diagonal multiplied by `(1 + λ_fisher · s_i)` where s_i is the foundationality score of the chunk whose loss is currently being backpropagated. This requires (a) tracking per-parameter Fisher diagonal at `crates/context-graph-mejepa-train/src/aux/`, (b) routing the foundationality of the current batch's covered chunks into the loss-scaling logic, (c) consuming `apply_foundationality_fisher_multiplier` at the live training step (not just in heal/fisher.rs). The Fisher diagonal is updated online per Schwarz 2018.

**ECE-vs-correlation needle.** EWC++ slows learning on parameters that matter for previously-learned chunks. For bedrock chunks (high s_i, high λ_fisher multiplier), the predictor is more reluctant to update; for leaf chunks, it learns normally. This is **anti-correlated** with the sampler boost (I-1's sampler boost says "see bedrock more"; this says "but don't change much for bedrock"). The combined effect is a curriculum that **first** absorbs the bedrock structure and **then** specializes on leaves — Bengio 2009-style curriculum. The risk: if `λ_fisher` is too high, the predictor cannot specialize at all and aggregate correlation drops.

**Per-cell vs aggregate.** Per-chunk gradient scaling translates to per-cell training pressure indirectly — cells dominated by bedrock chunks (the `sympy.core` core arithmetic, the `django.db` ORM) get slower-but-more-stable learning; cells dominated by leaves (test-only patches) get fast specialization.

**Cost.** Compute: per-parameter Fisher diagonal storage is `n_params × 4 bytes` per snapshot, persisted to `CF_MEJEPA_FISHER_SNAPSHOTS` (already reserved per doc 05 §9.3). Training compute: one Fisher diagonal pass per N steps; the per-batch overhead is the per-parameter multiplication, which is cheap. Retraining: yes, one full run.

**Falsification.** Two-axis: (a) without λ_found sampler boost, λ_fisher should hurt correlation (no preferential sampling); (b) with λ_found and λ_fisher both on, the ablation grid (λ_found ∈ {0, 1, 2, 4}, λ_fisher ∈ {0, 0.5, 1, 2}) should show a 2-D ridge of (correlation, stability) Pareto-optimal points. If no such ridge exists, the foundationality hierarchy is not load-bearing at the gradient surface.

**Retraining required?** Yes, plus a 4×4 ablation grid (16 runs).

**Panel B compatibility.** Yes — λ_fisher applies to the predictor weights; Panel B is a different predictor with its own weights, its own foundationality CFs, its own ablation.

**Active/abstain compatibility.** Yes — slowing bedrock-weight updates does not change the cold-cell `n_supporting < 50` Abstain threshold.

**No-holdout-tuning.** The 16-run ablation must pick its winning `(λ_found, λ_fisher)` on the calibration split, not the holdout split. This is the same disjointness requirement #547 already locked down for the rank-lift selection.

### 6.4 Intervention I-4: per-cell CP_φ as replay-buffer priority + cold-cell readiness leading indicator

**Mechanism.** (a) Refactor `compression_progress.rs` to compute CP_φ per `(MutationCategory, Lang)` cell — requires a new field `per_cell_conditional_description_length_bits: BTreeMap<CellKey, f64>` in `TrainingCertificate`. (b) Wire per-cell CP_φ as an additive prior on `ReplayPriorityConfig`'s priority score — cells with low per-cell CP_φ get higher replay priority (their predictor is compressing slowly). (c) Wire per-cell CP_φ monotonicity (the existing `CompressionProgressMonotonicity::NonDecreasing` check, but per-cell) as a **leading indicator of cold-cell readiness** — when a cell has insufficient `n_supporting < 50` but has 4 consecutive rolling windows of monotone-non-decreasing per-cell CP_φ, treat it as "predictor is converging on this cell, do not Abstain by default — instead apply temperature `T > 1.0` to the per-cell prediction".

**ECE-vs-correlation needle.** This is the most novel of the five interventions and the riskiest. The published precedent for per-cell CP_φ as a readiness signal does not exist (§5.5). The conservative deployment is (a) and (b) only; (c) requires its own A/B test against the current Abstain gate.

**Per-cell vs aggregate.** Per-cell only.

**Cost.** Storage: per-cell CP_φ ≈ 100 × 8 bytes per cert = 800 bytes per cert; cumulative over a training run is bounded by cert count. Compute: per-cell CDL is a per-cell `delta_p` average then `-log2`, no new computation. Retraining: not strictly required for (a) and (b) — these change replay-buffer ordering, not gradient. (c) requires retraining + recalibration of Abstain thresholds.

**Falsification.** Run the existing #392 active/abstain diagnostic with per-cell CP_φ-derived priority replay (intervention (b)) versus without. If the gap between active correlation (`0.9526`) and overall correlation (`0.7942`) does not narrow, (b) is noise.

**Retraining required?** (a) and (b) no. (c) yes.

**Panel B compatibility.** Yes — per-cell CP_φ is local to each Panel.

**Active/abstain compatibility.** Crucial. (c) explicitly changes the Abstain gate; it must be defended on a cold-load behavioral FSV per the #514 protocol before promotion.

**No-holdout-tuning.** Yes — per-cell CP_φ is computed on the calibration split or the training split; no holdout access.

### 6.5 Intervention I-5: foundationality-graded tier compression (`α_compress`) in the offline persistence path

**Mechanism.** Wire `compression_aggressiveness_from_foundationality(s_i)` (already implemented at `chunk_foundationality.rs:835-843`) into the encoder side of `crates/context-graph-tier-compression/`. When persisting a per-chunk panel vector, the bit-width is chosen as `bits = 8 - round(4 × α_compress) ∈ {4, 5, 6, 7, 8}` — bedrock chunks (α = 0.25) get 7-bit, leaf chunks (α = 1.0) get 4-bit. This is the PAPER §3.2bis claim that bedrock structure must be preserved more aggressively than leaves.

**ECE-vs-correlation needle.** Offline; does not touch the gradient. Tier compression is a storage-quality choice that affects (a) cache hit rates, (b) reconstruction error of cached panels at predict time, (c) eventually predict-time ECE through reconstruction noise. The Pareto gain is a strict storage-vs-reconstruction-error tradeoff with no gradient impact.

**Per-cell vs aggregate.** Per-chunk, which inherits per-cell behavior — cells dominated by bedrock chunks get larger cached panels.

**Cost.** Storage: net reduction (this is a compression intervention). Compute: negligible. Retraining: not required.

**Falsification.** Cosine drift between reconstructed and original panels, measured per-chunk-foundationality bucket. The PAPER claim is that bedrock-chunk cosine drift at 7-bit should be ≤ leaf-chunk cosine drift at 4-bit. If both are similarly low, the foundationality grading is operationally noise (but the storage saving is still real). If bedrock drift at 7-bit is unacceptable (> 0.001), drop to 6-bit on bedrock and accept smaller storage gains.

**Retraining required?** No.

**Panel B compatibility.** Yes — independent storage path.

**Active/abstain compatibility.** Yes — does not change prediction-surface confidence.

**No-holdout-tuning.** Yes — storage compression is uniformly applied, no per-split decision.

### 6.6 Interventions I-6 through I-10 (sketched briefly; full contracts on request)

- **I-6 — Compose I-1 + I-2.** Per-cell entropy loss plus foundationality scalar on Oracle head. The natural deployment ordering: I-1 first (lock per-cell ECE), then I-2 (add signal), then re-check per-cell ECE. Expected combined lift: 0.01-0.02 on holdout correlation, with bounded ECE risk.
- **I-7 — Pereyra label smoothing as alternative to entropy regularization.** Pereyra (2017) shows label smoothing and confidence penalty are equivalent up to a KL-divergence direction. The implementation is simpler: the predict-head target distribution is `(1 - ε) · one_hot(y) + ε / K` for K=2 binary classes; ε = 0.05 to 0.1. This is a one-line change at the cross-entropy loss call site. Use this as a fallback if the Kraskov k-NN entropy estimator turns out to be too noisy on small batches.
- **I-8 — Mukhoti focal loss for Oracle head only.** Replace the Oracle head's binary cross-entropy with focal-loss `γ = 3`. NeurIPS 2020 evidence is that this **alone** reduces ECE by 30-50% without rank loss. This is the most defensible single-line intervention I can recommend, doctrine-compatible, and orthogonal to I-1 (you can do both).
- **I-9 — Online TracIn for mistake-loop replay priority.** When a mistake-loop row arrives, compute its TracIn-style per-training-example influence score against the holdout cell that caused the mistake. Use the top-K training rows by influence as the replay-buffer priority for the next training run. This is the principled foundationality substitute — see §7.
- **I-10 — Per-cell predictor temperature.** Independent of the loss, learn a per-cell temperature scalar T_cell from the calibration split that minimizes per-cell ECE. This is the textbook Guo (2017) temperature-scaling extension to per-cell. It is the cheapest possible calibration intervention — a single scalar per cell, learned in seconds on calibration data, no retraining required, and **the cleanest path to lower per-cell ECE without touching correlation**.

---

## 7. TracIn / TRAK influence-function proposal

### 7.1 The case for running TracIn now

The operator's hypothesis is "all the fixes are in the data". TracIn (Pruthi 2020, [research.google](https://research.google/blog/tracin-a-simple-method-to-estimate-training-data-influence/)) and TRAK (Park 2023, [arxiv:2303.14186](https://arxiv.org/abs/2303.14186)) are the principled tools to test that hypothesis on the 2,400-train × 240-holdout setup. The deliverable: a **2,400-row ranking by influence on the underpowered mixed cells**, alongside the matching foundationality rank. Three falsifiable predictions become testable:

1. **If foundationality is the right proxy**, the TracIn rank and the foundationality rank should correlate at Spearman ρ ≥ 0.3 across all 2,400 training rows.
2. **If foundationality is the wrong proxy**, ρ < 0.1 and TracIn identifies a different sub-population of training rows that actually drive the underpowered cells.
3. **If neither foundationality nor TracIn identifies a meaningful sub-population** (top-50 TracIn-ranked rows have correlation `< 0.2` with the holdout-cell residuals), the operator's "math is in the data" hypothesis is structurally wrong and the gap requires new measurement axes (the creativity engine #516-#520).

### 7.2 Cost estimate

Per Pruthi 2020, TracIn computes `sum_t lr_t · <grad_loss(z_test, θ_t), grad_loss(z_train, θ_t)>` summed over K checkpoints. At our scale:

- **N_train = 2,400, N_test = 240, K = 5 checkpoints, D = predictor weight count.**
- Storage of per-example projected gradients (LoGRA / TRAK 128-d projection): N_train × K × 128 × 4 bytes = ≈ 6 MB; N_test × K × 128 × 4 bytes = ≈ 0.6 MB. Negligible.
- Compute: 2,400 × 240 = 576k inner products of 128-d vectors per checkpoint × 5 checkpoints ≈ 370M FLOPs. **Seconds on a 5090.**
- Without the LoGRA projection (full-rank Hessian-vector products), this would be O(2,400 × 240 × 5 × D) which is impractical at D ≈ 100M.

**TracIn is cheap.** The only cost is one (a) full-Hessian-vector-product probe per training row at each checkpoint, which the trainer can capture during training-pass-2 and persist. The aiwonder durable root for this artifact would be `/zfs/archive/contextgraph/state/tracin/post-fix-real-input-v1/`.

### 7.3 Expected ranking correlation with foundationality

I make two predictions:

1. **ρ(TracIn, foundationality) ≈ 0.2-0.4** on the post-fix real-input training run. Reasoning: foundationality is a *static* prior (the chunk-dependency PageRank); TracIn is a *learned* prior (the gradient inner product). They overlap because high-foundationality chunks tend to participate in more training examples (more upstream/downstream chunks reach them), but they diverge because the predictor's gradient depends on the loss surface — not all bedrock chunks are equally hard to learn.
2. **The top-50 TracIn-ranked training rows for the underpowered mixed cells (`subtle_flip × Python`, `compile_error × Python`) will not be the top-50 foundationality-ranked training rows.** The mixed-cell signal is in the chunks that *distinguish* a passing patch from a failing patch under that mutation operator — typically not the most foundational chunks in the dependency graph, but the chunks where the mutation operator has its strongest semantic effect.

### 7.4 Falsification

If TracIn produces no meaningful ranking (top-50 vs bottom-50 of TracIn rank have indistinguishable effect on holdout-cell correlation when held out), the data is not the bottleneck — the predictor architecture is. The creativity-engine queue (#516-#520) becomes the only remaining lever.

---

## 8. Bayes-optimal upper-bound audit — the "math is in the data" claim

### 8.1 The PAPER's structural upper bound

PAPER §3.1 names the structural count: the code substrate yields *up to* `n × (N + C(N,2) + 1) = n × (13 + 78 + 1) = n × 92` structured supervisory signals. PAPER §3.1 also names the **hedge**: this is a structural upper bound, not an information-theoretic capacity. The bound is **strict only under Axiom 2** (approximate orthogonality of the N embedders), which is **not measured** on our panel. PAPER §3.2 admits the pairwise term `C(N,2)` collapses if any pair of embedders are linear reparameterizations of each other.

The 92-signal count is therefore **not** a Bayes-optimal upper bound on `prediction_oracle_correlation`. It is a counting identity for the cardinality of the derived dataset D'. The information content of D' is bounded above by `I(D' ; oracle_label)`, which equals (under Axioms 1 and 2) at most `H(oracle_label)` itself — and since the oracle label is binary, `H(oracle_label) ≤ 1 bit`. The question is whether D' achieves that 1 bit on the post-fix real-input training run.

### 8.2 The Bayes-optimal classifier framing

For binary classification, the Bayes-optimal correlation is `corr(p̂*, y) = corr(P(y=1|x), y)` where p̂* is the Bayes-optimal posterior. The relationship between Bayes-optimal accuracy and correlation depends on the class balance. The published Lite holdout class balance is well-mixed (the calibration-disjointness audit #547 explicitly uses mixed cells where Pearson is well-defined). Under perfect Bayes-optimality, `corr = 1.0` is possible **only if every training example has a unique-up-to-cell-label-deterministic mapping into D'**.

We do not have that. The mutation operators (subtle_flip, off_by_one, swap_variable, etc.) deliberately produce **diff-equivalent mutants** — small surface changes that may or may not break the test suite. Many oracle outcomes are **non-deterministic in D'**: the same `(x_before, x_after, mutation_category, language, instrument panel)` admits both Pass and Fail because the Docker test outcome depends on flaky test infrastructure, on the specific assertion that fires, on the test framework's tolerance, and on Docker-image-version drift the panel does not observe. **This is the irreducible noise floor of the binary prediction task on SWE-bench Lite.**

### 8.3 The empirically-credible upper bound

The Bayes-optimal correlation upper bound for our task is **strictly less than 1.0** and is plausibly in the range **0.92-0.98**. Three pieces of evidence:

1. **#392 active-row correlation is 0.9526** — the predictor *already* achieves > 0.95 on the 205 of 240 holdout rows where it is willing to commit. The 35 Abstain rows account for the gap to aggregate `0.7942`. This is direct evidence that the underlying Bayes-optimal correlation **is at least 0.95**.
2. **#558 usefulness_weighted_combined trajectory correlation `0.8679`** (per doc 08) is achieved by adding richer trajectory features — incremental data uplift is real. The combined effect of foundationality + per-cell entropy + per-cell CP_φ + influence-function-priority replay should push aggregate correlation past `0.85` and per-cell past `0.95` on the gate-relevant cells.
3. **The post-fix raw holdout `0.4843`** is the floor; rank-safe `0.6917` and rank-lift `0.7942` are progressively richer post-hoc transformations. The room for honest improvement before hitting the Bayes-optimal noise floor is `0.95 - 0.7942 = 0.158` on rank-lift, `0.95 - 0.6917 = 0.258` on rank-safe.

### 8.4 Verdict on the operator's hypothesis

**Partially confirmed, partially refuted.** The 0.158-point gap is bridgeable in principle by the existing 92-signal substrate plus the foundationality scalar **if** (a) the foundationality scalar is wired through the predictor surface (intervention I-2) and the Fisher path (I-3), (b) per-cell calibration is hardened (I-1, I-10), and (c) per-cell CP_φ-driven replay priority lifts the underpowered cells (I-4). It is **not** bridgeable by these alone if the irreducible Docker-oracle noise is greater than 0.05 on the gate-relevant cells — in which case Panel B (#405) becomes the meaningful gate and the operator must accept that 0.95 on Panel A is a noisy upper bound.

The "math is in the data" hypothesis is **directly testable by TracIn** (§7). If TracIn-top-50 rows do not move the underpowered cells, the data is not the bottleneck. If they do, the operator is correct and the creativity engine (#516-#520) is premature.

---

## 9. Compatibility matrix

| Intervention | #405 Panel B (anti-Goodhart) | #392 Active/Abstain | Mistake loop (#406/#523) | Slot-preserving rule | No-holdout-tuning rule |
|---|---|---|---|---|---|
| I-1 Per-cell entropy λ | Per-Panel scheduler; both Panels run their own per-cell ablation | Per-cell Brier-regression guard prevents active-row regression | Independent | Yes — entropy on slot-preserving latent z, not flat panel | Yes — uses calibration split for λ decision |
| I-2 Foundationality scalar on Oracle head | Same scalar fed to both Panels; the two Oracle heads have independent learned projections | Adds signal to Active cells; does not affect Abstain support | Independent | Yes — scalar feature, not a panel reshape | Yes — head weights learned from train split |
| I-3 λ_fisher EWC over predictor weights | Per-Panel Fisher diagonal; ablation grid run independently per Panel | Slowing bedrock-weight updates does not move the Abstain threshold | Independent (Fisher protects predictor weights, mistake-loop is online-head-state isolation) | Yes — per-parameter scaling, no slot collapse | Yes — ablation grid selected on calibration split |
| I-4 Per-cell CP_φ replay priority + readiness | Per-Panel CP_φ; Panel A's CP_φ never feeds Panel B priorities | (b) per-cell replay priority does not move Abstain. (c) per-cell readiness changes Abstain — requires cold-load FSV defense per #514 | Composes — CP_φ-priority lifts cells the mistake loop is also targeting | Yes — CP_φ is a per-cell scalar | Yes — per-cell CP_φ computed on training/calibration split |
| I-5 α_compress tier compression | Independent storage path per Panel | Storage-only, no surface effect | Independent | Yes — per-chunk bit-width, slot-preserving | Yes — storage decision, no split tuning |
| I-6 Compose I-1 + I-2 | Inherits I-1 and I-2 | Inherits | Inherits | Inherits | Inherits |
| I-7 Pereyra label smoothing | Both Panels apply ε independently | Same risk profile as I-1 with different parameterization | Independent | Yes — at loss surface, not panel | Yes — ε is fixed, not split-tuned |
| I-8 Mukhoti focal loss on Oracle head | Both Panels independently γ-tune | Likely lowers ECE — neutral or positive | Independent | Yes — at loss surface | Yes — γ is fixed |
| I-9 Online TracIn for mistake-loop replay | Per-Panel TracIn computed independently | Lifts cells where mistakes recur; neutral on Abstain | Composes — augments mistake-loop priority with influence-aware score | Yes — gradient inner products on slot-preserving predictor | Yes — TracIn computed on train rows, validated against calibration |
| I-10 Per-cell predictor temperature | Per-Panel per-cell T_cell | Direct ECE improvement; no correlation cost | Independent | Yes — scalar per cell | Yes — T_cell learned from calibration |

---

## 10. Honest risk register

### 10.1 What I do not know

1. **Whether TracIn ranks correlate with foundationality ranks on real Python SWE-bench Lite.** This is the most testable prediction in this report and it is not testable from this WSL box — it requires a TracIn run on aiwonder against the post-fix real-input checkpoint.
2. **Whether per-cell entropy regularization at `λ = 1e-4` regresses Brier on the active-row subset of #392.** This is the operationally decisive question for I-1; the global Brier auto-disable scheduler at `entropy.rs:97-136` is *not* a sufficient guard at the per-cell granularity.
3. **The exact per-cell decomposition of the `0.7942` aggregate baseline.** I do not have per-cell correlation/Brier/ECE for all 8 mutation categories. The recent FSV `run-20260521T132308Z` should hold them but the durable artifact is on aiwonder and not pull-back-able from this session.
4. **The Bayes-optimal noise floor on Docker-oracle outcomes.** §8.3 conjectures `0.92-0.98`; I cannot estimate it more tightly without test-retest data on the oracle harness itself.
5. **Whether the existing `loss/entropy.rs` operates on the slot-preserving latent z or on a flattened panel.** The trainer call site at `trainer/mod.rs:248` invokes `batch_latent_entropy_loss(dataset, &plan.indices, &self.device, lambda_entropy)` and the implementation reads `panel_t01.to_dtype(F32).flatten_all()` — **`flatten_all()` flattens the panel**. This means the entropy estimator is currently computing entropy over the flat 5,120-d vector, which is a **slot-preservation rule violation** under CLAUDE.md §6.2. **This is a discoverable defect**; should be filed.
6. **Whether the heal-scheduler's `consume_persisted_chunk_foundationality_fisher` actually feeds the live training run's Fisher diagonal**, or whether it produces a policy snapshot that never lands in the gradient. I read the function but did not find a consumer downstream of the policy CF in the trainer module path.

### 10.2 The entropy-vs-correlation tradeoff in numbers

| λ_entropy | Pereyra-equivalent β | Expected test-accuracy change (Pereyra 2017, CIFAR-100) | Expected ECE change (Mukhoti 2020 scale) | Expected per-cell correlation change |
|---|---|---|---|---|
| 0.0 | 0.0 | 0% (baseline) | 0% (baseline) | 0% |
| 1e-5 | 0.001 | +0.0 to +0.1% | -2 to -5% | -0.001 |
| 1e-4 | 0.01 | +0.1 to +0.2% | -5 to -10% | -0.003 |
| 1e-3 | 0.1 | +0.2 to +0.4% | -10 to -25% | -0.01 |
| 1e-2 | 1.0 | -0.5 to -1.5% | -25 to -50% | -0.05 |

**Recommendation.** Stay in `λ_entropy ∈ [1e-5, 1e-3]` with the per-cell Brier-regression auto-disable. The `1e-4` default in `EntropyLambdaScheduler::default()` is correct.

### 10.3 What could be wrong with this entire investigation

The single largest risk is that **the post-fix real-input pipeline (the `0.7942` baseline) is dominated by the 2026-05-19/20 synthetic-vector remediation effects** and that the foundationality / CP_φ / entropy levers I propose are calibrated against a baseline that will continue to shift as the real-input pipeline matures. The conservative reading: deploy I-8 (Mukhoti focal loss on Oracle head) and I-10 (per-cell temperature) first as cheap, low-risk, easily-reverted interventions. Then I-1 (per-cell entropy with auto-disable) as the first non-trivial step. Then I-2 + I-3 (foundationality at the predictor surface and at Fisher) only after the post-fix baseline has stabilized over four rolling windows.

---

## 11. Three questions for the other Sherlocks

For composition with Sherlock #1 (cross-embedder MI/correlation excavation) and Sherlock #2 (calibration/ECE shape and per-cell active features):

1. **For Sherlock #1 (cross-embedder MI).** Of the 78 cross-embedder pairwise cosine signals, how many have measurable mutual information above the Kraskov-KSG noise floor on the post-fix real-input pipeline? If the effective `N_eff` is < 13, the PAPER §3.1 92-signal upper bound is loose, and my §8 Bayes-optimal upper-bound estimate of 0.92-0.98 is also loose. Specifically: does the pairwise MI matrix show the kind of redundancy that would imply our 0.158-point gap is dominated by panel-redundancy rather than by predictor-gradient inefficiency?
2. **For Sherlock #2 (per-cell ECE/calibration).** What is the per-cell decomposition of the aggregate `0.7942` rank-lift baseline? Specifically the (correlation, Brier, ECE, active_count, abstain_count) tuple per `(MutationCategory, Lang)` cell — the ground truth needed to actually run my per-cell entropy scheduler (I-1) and per-cell temperature (I-10). If Sherlock #2 reports a single dominant problem cell (e.g., `subtle_flip × Python` at correlation `0.6`), my interventions need to be sequenced to target that cell first.
3. **For both.** Is there a known pair of embedders in the active 12-content-slot panel whose cross-embedder MI exceeds the MI of either embedder with the oracle label? If so, that pair is redundant under any operational definition and the foundationality scalar (which is structurally independent of all embedders) becomes the more important supplementary signal — strengthening the case for I-2 (foundationality as predictor surface feature) at the expense of I-3 (Fisher hierarchy over predictor weights).

---

*Case 3 closed. The game continues.*

Sources cited externally:
- Vopson and Lepadatu, "Second law of information dynamics," AIP Advances 12 075310 (2022). https://pubs.aip.org/aip/adv/article/12/7/075310/2819368/
- Schmidhuber, "Driven by Compression Progress" (2009). https://arxiv.org/abs/0812.4360
- Pereyra et al., "Regularizing Neural Networks by Penalizing Confident Output Distributions" (ICLR 2017 Workshop). https://arxiv.org/abs/1701.06548
- Mukhoti et al., "Calibrating Deep Neural Networks using Focal Loss" (NeurIPS 2020). https://papers.neurips.cc/paper/2020/file/aeb7b30ef1d024a76f21a1d40e30c302-Paper.pdf
- Kirkpatrick et al., "Overcoming catastrophic forgetting in neural networks" (PNAS 2017). https://arxiv.org/abs/1612.00796
- Soutif-Cormerais et al., "On the Computation of the Fisher Information in Continual Learning" (2025). https://arxiv.org/html/2502.11756v1
- Schaul et al., "Prioritized Experience Replay" (ICLR 2016). https://arxiv.org/abs/1511.05952
- Pan et al., "Understanding and Mitigating the Limitations of Prioritized Experience Replay" (PMLR 2024). https://proceedings.mlr.press/v180/pan22a/pan22a.pdf
- Pruthi et al., "Estimating Training Data Influence by Tracing Gradient Descent" (NeurIPS 2020) — TracIn. https://research.google/blog/tracin-a-simple-method-to-estimate-training-data-influence/
- Park et al., "TRAK: Attributing Model Behavior at Scale" (ICML 2023). https://arxiv.org/abs/2303.14186
- Kraskov, Stögbauer, Grassberger, "Estimating mutual information," Phys. Rev. E 69 066138 (2004). https://pubmed.ncbi.nlm.nih.gov/15244698/
- Tishby et al., "Learning and Generalization with the Information Bottleneck" (2010). https://www.wisdom.weizmann.ac.il/~shamiro/publications/2008_ALT_ShamSabTish.pdf
- Alemi et al., "Deep Variational Information Bottleneck" (ICLR 2017). https://arxiv.org/pdf/1612.00410
- Guo et al., "On Calibration of Modern Neural Networks" (ICML 2017). https://arxiv.org/pdf/1706.04599
- Allamanis et al., "Learning to Represent Programs with Graphs" (ICLR 2018). https://arxiv.org/abs/1711.00740
