# 09 — The End Goal: A Reality Compiler (ME-JEPA)

> **Build-ready specification for an AI agent.** This is the master plan for the ContextGraph reality system. Read top-to-bottom; pick up at the Phase plan and execute. All earlier docs (01-08) are reference material that informs *how* the components are implemented; this doc is *what* gets built and *why*.

> **Operator framing, 2026-05-08:** *"the ultimate goal of this system and Main focus is to develop a system that makes it so that coders don't need to review code anymore since they are reviewing that what the ai said actually exists in reality... the me jepa is what predicts reality. this is what is going to be responsible for reviewing essentially and verifying/validating it... we need a truth system that can predict that code is good and working as intended... does what the ai says it did exist and does it work as the ai says it does or does it work as its intended to? how is it actually impacting reality itself."*

> **Operator's compiler analogy, 2026-05-08:** *"would this almost be like a compiler but it almost compiles reality itself?"* — yes; the structure of the system maps cleanly onto compiler stages. This doc is anchored on that framing.

---

## Section 0 — TL;DR

We are building **ME-JEPA-Code**: a Joint Embedding Predictive Architecture that, given a code change plus its context (AST, tests, traces, witness chain, environment), predicts the *latent of reality* — what the oracle would say if the change were run. When ME-JEPA-Code's prediction agrees with the actual oracle on held-out tests, the code passes through it without human review.

The structure is a **reality compiler**: source code goes in, a predicted reality state comes out, and the difference between predicted and actual is the loss signal. As the corpus grows, the compiler sharpens. The end state is a system where `me_jepa.compile(code, context) == oracle.run(code)` to within calibrated confidence — at which point human code review of AI-generated patches becomes statistically wasteful.

Key system properties:
- **Self-healing.** Online learning with EWC++ memory protection; drift detection via conformal-coverage rolling-window; background retraining + A/B/C promotion gating.
- **Bounded.** Hard disk and RAM quotas; tiered storage (8/7/5/3-bit); nightly GC; witness-chain Merkle compression.
- **Auditable.** Every training step has a certificate; every prediction has a confidence interval; every input file has a SHA chain back to its origin.
- **Clean-room owned.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`, every line is ours; no foreign Rust crates, no npm dependencies on the ruvnet ecosystem.

**Current implementation snapshot (2026-05-08):**

- The witness-chain substrate exists (`context-graph-witness`) and is wired into ccreality optimizer files and DynamicJEPA audit rows.
- The solver substrate exists in part (`context-graph-solver`): Forward-Push PPR + Conjugate Gradient + CSR validation + manual FSV evidence.
- The actual ME-JEPA-Code pipeline in this doc has **not** shipped yet: no mutation corpus, code-domain instruments, 4,096-d panels, predictor, conformal calibration, OOD gate, self-healing loop, or reality-loop verify integration.
- BMSSP/multigrid is deliberately still pending; no fallback wrapper is allowed.

---

## Section 1 — The Reality Compiler

### 1.1 Compiler-stage analogy

| Traditional compiler stage | Reality-compiler (ME-JEPA-Code) stage | Implementation |
|---|---|---|
| Lexer | Token stream from tree-sitter | already in `crates/context-graph-embedding` (E7 Qodo-Embed) |
| Parser → AST | AST instrument `E_AST` | encode AST nodes via tree-sitter + transformer over node-token sequence |
| Semantic analysis | Type-graph instrument `E_TypeGraph` + DataFlow `E_DataFlow` | static analysis (Pyright/mypy for Python; rust-analyzer for Rust) → graph attention |
| Intermediate representation | The **panel** — joint embedding of all instruments | concatenated projection of all `E_*` outputs into a single tensor |
| Optimization passes | The **predictor** moves panel-at-time-t → panel-at-time-t+1 in latent space | small transformer trained with VICReg loss against an EMA target encoder |
| Code generation | The **predicted reality state** — predicted oracle outcome, predicted execution trace, predicted side-effect set | a head on top of the predictor's output panel that maps to the oracle's signal-space |
| Linker | Cross-reference of claim-graph ↔ source-graph ↔ context-graph ↔ witness chain | provenance-binding lookups by content hash |
| Type-checker | Predictor-vs-target distance: "does predicted reality match claimed reality?" | conformal-prediction set + Granger attestation |

**The "compiled output" is a probability distribution over reality states**, not a binary verdict. Conformal prediction handles the calibration: with stated coverage `1 - α` (default 90%), the actual oracle outcome must land in the predicted set with empirical frequency at least `1 - α`.

### 1.2 Three differences from a real compiler

1. **Probabilistic, not deterministic.** Same code + fixed weights → same prediction (deterministic given weights), but the prediction is a *distribution*. Conformal prediction quantifies the spread.

2. **Learned semantics, not declared.** A C compiler's correctness derives from the C language standard. ME-JEPA-Code's correctness derives from the SWE-bench-oracle training corpus. Sharpness grows with corpus size; behavior on unseen libraries / patterns degrades gracefully (the model knows it's uncertain via conformal coverage).

3. **Failure mode reversed.** Compilers fail-closed (won't emit machine code if there's a type error). ME-JEPA-Code fails-open by default — it'll produce a confident-looking prediction on out-of-distribution code unless we wrap it with gates. The conformal-coverage check + witness-chain integrity check + Granger attestation collectively form the "fail-closed wrapper."

### 1.3 What "compilation success" means

```
me_jepa.compile(code, context) -> RealityPrediction {
    predicted_oracle_pass: f32,                  // ∈ [0, 1]
    predicted_failing_tests: Vec<TestId>,
    predicted_runtime_trace_summary: Vec<f32>,   // latent
    predicted_side_effect_set: Vec<SideEffect>,
    confidence: ConformalSet {                   // calibrated
        alpha: f32,
        outcome_set: Vec<OracleOutcome>,
    },
    out_of_distribution_score: f32,              // ∈ [0, 1]; >0.5 → escalate
    granger_attestations: HashMap<SkillId, f32>, // p-values per cited skill
    witness_chain_offset: u64,                   // entry pinning this prediction
}
```

A patch "compiles successfully" when:
1. `out_of_distribution_score < 0.5` (the predictor recognizes the input distribution).
2. `predicted_oracle_pass > 0.95` (high confidence the patch passes).
3. `confidence.outcome_set` has size ≤ 2 (low entropy = sharp prediction).
4. All cited skills' Granger p-values are below the rejection threshold.

When all four hold, no human review is needed. When any fails, the failure mode tells the human exactly which gate to investigate.

---

## Section 2 — ME-JEPA-Code Architecture

### 2.1 Joint Embedding Predictive Architecture (JEPA) recap

A JEPA learns to predict latent representations of unobserved targets from observed contexts. Three components:

1. **Context encoder** `f_θ`: encodes the observed context into a latent.
2. **Target encoder** `f_ξ`: encodes the masked target into a latent. Weights `ξ` are an exponential moving average (EMA) of `θ`. Frozen during loss computation.
3. **Predictor** `g_φ`: takes the context latent + a positional/conditional cue and predicts the target latent.

Loss: `||g_φ(f_θ(x_context)) - f_ξ(x_target)||²` plus VICReg regularization (variance + invariance + covariance) to prevent collapse.

For ME-JEPA-Code, the "context" is the patch + its surrounding state at time t; the "target" is the oracle-observable state at time t+1 (after running the patch).

### 2.2 The instrument set (front-end stages)

Each instrument is a function `(input_modality) -> latent_vector`. Pre-trained where possible; fine-tuned during ME-JEPA training.

| Instrument | Input | Output dim | Source |
|---|---|---|---|
| `E_AST` | tree-sitter AST of patched files | 384 | new — small transformer over node-token sequence |
| `E_CFG` | control-flow graph of patched functions | 256 | new — graph attention over basic blocks |
| `E_DataFlow` | def-use chains + dataflow edges | 256 | new — graph attention over variable lifetimes |
| `E_TypeGraph` | type signatures + inferred types (Pyright/mypy or rust-analyzer) | 256 | new — graph encoder over type lattice |
| `E_Test` | test-stack: which tests cover this code, what they assert | 384 | reuse `E1` (context-graph e5-large-v2) on test-fn natural-language descriptions |
| `E_Trace` | execution trace from running the patch (function entries, line hits, value snapshots) | 512 | new — sequence transformer over `(fn_id, line_no, val_hash)` events |
| `E_Diff` | AST-diff between pre-patch and post-patch | 256 | new — graph attention over diff-edge events (added/removed/modified nodes) |
| `E_Witness` | witness-chain events emitted during attempt | 256 | sequence transformer over 73-byte chain entries (as integer features) |
| `E_Oracle` | actual oracle outcome (pass/fail per test, runtime exception if any) | 128 | one-hot test outcomes + categorical exception classes |
| `E_Problem` | natural-language problem statement | 1024 | reuse `E1` (context-graph e5-large-v2) on the SWE-bench problem text |
| `E_CommitMsg` | commit message of the candidate fix | 384 | reuse `E1` on commit text |

Total panel dimension: `384 + 256 + 256 + 256 + 384 + 512 + 256 + 256 + 128 + 1024 + 384 = 4,096`.

The panel is a 4,096-dimensional joint embedding per `(patch_attempt, time_step)`.

### 2.3 The panel (the IR)

Per attempt, we materialize a sequence of panels:

- `panel[t=0]` — pre-patch state: `(E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Problem)` evaluated on the original code; `E_Diff = 0`; `E_Trace = 0`; `E_Witness = 0`; `E_Oracle = 0`; `E_CommitMsg = 0`.
- `panel[t=1]` — post-patch, pre-execution: `E_AST, E_CFG, E_DataFlow, E_TypeGraph` re-evaluated on patched code; `E_Diff` populated; `E_CommitMsg` populated; `E_Trace = 0`; `E_Witness` partial (just the patch-application entries); `E_Oracle = 0`.
- `panel[t=2]` — post-execution: all instruments populated, including `E_Trace` from the actual test run and `E_Oracle` with the verdict.

**Training task:** given `panel[t=0]` + `panel[t=1]` (context), predict `panel[t=2]` (target). The crucial feature being predicted is the `E_Oracle` slice — that's the supervision signal.

Storage: panels persist in RocksDB CF `CF_MEJEPA_PANELS`, keyed by `(attempt_id, time_step)`. With ~4,096 floats × 4 bytes = ~16 KB per panel × 3 panels per attempt × ~10 attempts per task × 300 tasks = **~144 MB** for the base SWE-bench Lite training set. Mutation corpus (Section 3) multiplies by ~8×. Total panel storage ~1.2 GB raw; ~150 MB on the cold tier (3-bit) per `ruvector-temporal-tensor`.

### 2.4 The predictor (the optimization passes)

Input: `(panel[t=0], panel[t=1])` concatenated → 8,192-d.
Architecture: 6-layer transformer, hidden dim 1,024, 8 heads, GeLU activations, residual + layer-norm. Output: predicted `panel[t=2]` — 4,096-d.

EMA target encoder: same architecture as the context encoder; weights updated via `ξ ← τ ξ + (1 - τ) θ` with `τ = 0.996` per V-JEPA standard.

Loss:
```
L_total = L_predict + λ_var · L_variance + λ_cov · L_covariance + λ_inv · L_invariance
```
where:
- `L_predict = ||g_φ(f_θ(context)) - f_ξ(target)||²` (Huber loss, δ=1)
- VICReg variance term: enforces per-dim std > γ on a batch
- VICReg covariance term: penalizes off-diagonal covariance
- VICReg invariance: redundancy term tying context and target latents

Defaults: `λ_var = 25`, `λ_cov = 1`, `λ_inv = 25`, `γ = 1`.

Optimizer: AdamW, lr 5e-4 with cosine decay, weight decay 0.04, batch size 64.

### 2.5 The reality-state head (the codegen)

On top of the predictor's 4,096-d output, a small head specifically predicts the `E_Oracle` slice:

```rust
struct OracleHead {
    project_to_logits: Linear<4096, num_tests + 1>,  // +1 for "exception"
}
```

At inference time, applying softmax to the head's logits gives the per-test pass probability. The conformal calibration set is built from holdout attempts where we know the actual oracle outcomes.

---

## Section 3 — The Training Corpus

### 3.1 Primary: SWE-bench Lite mutation corpus

The 300 SWE-bench Lite tasks each ship with a known-correct fix. We **deliberately mutate that fix** to generate a labeled training corpus where every example has a known oracle outcome. **8 mutation categories × 300 tasks = ~2,400 labeled examples.**

| # | Category | Mutation strategy | Expected oracle |
|---|---|---|---|
| 1 | **Known-good** | Apply the official fix unchanged | PASS |
| 2 | **Subtle-flip** | Invert one boolean in the fix (e.g., `>` → `<`, `and` → `or`, `True` → `False`) | FAIL on at least one test |
| 3 | **Off-by-one** | Add `± 1` to a numeric constant in the fix | FAIL on at least one test |
| 4 | **Swap-variable** | Rename one local variable to another in-scope | FAIL on type or runtime |
| 5 | **Delete-test-call** | Apply the fix but delete one assertion call | PASS the deleted test, FAIL the integration test |
| 6 | **Wrong-file** | Apply the fix to a *different* file in the repo with similar structure | FAIL on test discovery or runtime |
| 7 | **Over-engineer** | Apply the fix + add unrelated dead code | PASS (correct but messy — important for "predict pass without overfitting to one syntactic shape") |
| 8 | **Compile-error** | Introduce a syntax error in the fix | FAIL at parse time |

For each mutated example, we run the Docker oracle once (ground truth) and store:
- The mutated patch text.
- All instruments evaluated on (pre-patch, post-patch, post-execution).
- The actual oracle verdict (per-test pass/fail, exceptions, runtime trace).
- The mutation category label.

This is the **base training set**. It's KNOWN-LABELED, FIXED-SIZE (~2,400 examples), and REPRODUCIBLE (same task + same mutation → same example).

**Why this works:** ME-JEPA learns to predict reality from `(code, context)`. Category 1 teaches "this kind of input → PASS." Categories 2-6, 8 teach "this kind of input → FAIL with these specific failure modes." Category 7 teaches "structural variation around correct doesn't change PASS." The diversity is what makes the predictor generalize beyond one syntactic shape.

**Generation pipeline (Phase 0 deliverable):**

```rust
crate context-graph-mejepa-corpus {
    fn generate_swe_bench_mutation_corpus(
        swe_bench_lite_root: &Path,
        output_root: &Path,
        categories: &[MutationCategory],
    ) -> Result<CorpusReport> {
        for task_id in iter_swe_bench_lite_tasks(swe_bench_lite_root)? {
            let canonical_fix = task_id.official_fix()?;
            for category in categories {
                let mutated = apply_mutation(category, &canonical_fix)?;
                let panels = extract_panels(&task_id, &mutated)?;
                let oracle_verdict = run_docker_oracle(&task_id, &mutated)?;
                persist_corpus_entry(output_root, &task_id, &category, &panels, &oracle_verdict)?;
            }
        }
    }
}
```

### 3.2 Secondary: live reality-loop attempts

In addition to the static mutation corpus, **every reality-loop attempt** (whether the patch comes from Claude, Qwen, hand-written, or any future model) produces another labeled example: the harness extracts panels + the oracle verdicts, and they enter the corpus.

This is what makes ME-JEPA evolve: every Claude attempt, every Qwen attempt (if we keep the local model), every retry — they all become training data.

The corpus growth rate: ~5 attempts per task × 300 tasks × 1 reality-loop run per week = ~1,500 new examples per week beyond the static base set.

### 3.3 Tertiary: synthetic adversarial perturbations

Beyond the 8 base mutation categories, we generate synthetic adversarial examples in the *latent space* of the panel itself:

- Perturb `E_AST` slightly (10% of dims with N(0, 0.01) noise) → train ME-JEPA to be robust to small input variations.
- Inject "claim drift" in `E_Witness` (modify cited file SHAs to wrong values) → train ME-JEPA to predict FAIL when claims don't match reality.
- Replace `E_Trace` with traces from a different patch → train ME-JEPA to detect when claimed behavior doesn't match observed behavior.

These adversarials are generated on-the-fly during training; they don't need persistent storage.

### 3.4 Holdout split

Use **Stoer-Wagner mincut** (per `06_RISKS_AND_NON_TRANSFERABLE.md` clean-room reimplementation) on the (task → repo) bipartite graph to split:

- **Train:** 80% of tasks, all 8 mutation categories per task.
- **Calibration:** 10% of tasks, all 8 mutation categories. Used for conformal prediction calibration.
- **Holdout:** 10% of tasks, all 8 mutation categories. Used for evaluation only; never seen during training.

Mincut ensures no two tasks from the same repo (e.g., two astropy tasks) end up in different splits — that's the leakage failure mode.

---

## Section 4 — Inference

### 4.1 Call shape

```rust
impl MeJepaCompiler {
    /// Predict the reality state that would result from applying this patch.
    pub fn compile(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<RealityPrediction>;

    /// Run the static gates (witness chain, source-of-truth SHA, OOD score)
    /// and return the verdict suitable for "no human review needed" decision.
    pub fn verify(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<VerifyVerdict>;

    /// Update the model online with one new (panel, oracle_outcome) example.
    /// Uses EWC++ to protect older tasks. See Section 5.
    pub fn observe(
        &mut self,
        panel: &Panel,
        oracle_outcome: &OracleOutcome,
    ) -> Result<()>;
}
```

`PatchBundle` carries everything needed to evaluate the instruments: pre-patch source SHA, post-patch source SHA, AST diff, witness-chain segment, claim graph (what the AI said it did).

`TaskContext` carries the SWE-bench task ID (or a synthesized analog for non-benchmark code), problem statement, test stack, environment.

`VerifyVerdict`:

```rust
enum VerifyVerdict {
    Approve {
        reality_prediction: RealityPrediction,
        gates_passed: u8,
    },
    EscalateToHuman {
        failed_gate: FailedGate,
        reality_prediction: RealityPrediction,
    },
}

enum FailedGate {
    OutOfDistribution { score: f32 },
    LowConfidence { entropy: f32 },
    PredictedFailure { failing_tests: Vec<TestId> },
    GrangerRejection { skill: SkillId, p_value: f32 },
    WitnessChainBroken,
    SourceShaDrift { path: PathBuf },
}
```

### 4.2 Conformal calibration

After training, we calibrate on the calibration split (10% of tasks):

1. For each calibration example, compute predicted `E_Oracle` slice and actual `E_Oracle` slice.
2. Compute a non-conformity score: `max_test (|predicted_test_pass[t] - actual_test_pass[t]|)`.
3. Sort the calibration scores; pick the `(1 - α)` quantile as the conformal threshold.

At inference time, the prediction set is `{outcomes : non_conformity_score(outcome) ≤ threshold}`. Coverage guarantee: the actual oracle verdict lands in this set with probability `1 - α`.

### 4.3 Out-of-distribution detection

Compute the predictor's reconstruction error on the input panel:

```
ood_score(panel) = 1 - exp(-||g_φ(panel) - f_ξ(panel)||² / σ²)
```

where `σ²` is a bandwidth tuned on the calibration set such that calibration examples score `< 0.3`. If `ood_score > 0.5` at inference time, escalate to human regardless of other gates.

This is the "the predictor doesn't recognize this kind of input" signal. Critical for refusing to predict on novel libraries / language features the corpus didn't cover.

---

## Section 5 — Self-Healing and Evolution

### 5.1 Online learning with EWC++ protection

Every reality-loop attempt produces a new `(panel, oracle_outcome)` pair. The ME-JEPA serves predictions AND is updated online:

```rust
fn observe(&mut self, panel: &Panel, oracle_outcome: &OracleOutcome) -> Result<()> {
    // 1. Compute Fisher information for current (θ, ξ) on this example.
    let fisher = self.ewc.update_fisher_online(&panel, &oracle_outcome);

    // 2. Take a small SGD step on the new example, regularized by the EWC term:
    //    L_total = L_predict + λ_ewc · sum_over_old_tasks(F_i · (θ_i - θ_i*)²)
    let loss = self.predictor.step(panel, oracle_outcome, λ_ewc = self.ewc.current_lambda());

    // 3. Update the EMA target encoder.
    self.target_encoder.ema_update(&self.predictor, τ = 0.996);

    // 4. Detect if this is a new task boundary (per sona's automatic detection).
    if self.ewc.detect_task_boundary() {
        self.ewc.snapshot_current_task();
    }

    Ok(())
}
```

`EwcPlusPlus` protects weights that mattered for old tasks (per `crates/sona/src/ewc.rs` clean-room reimplementation). Catastrophic forgetting is prevented.

### 5.2 Drift detection

Maintain a rolling window (last 1,000 attempts) of `(predicted_set, actual_oracle)` pairs. Compute empirical conformal coverage:

```
empirical_coverage = #{actual_oracle ∈ predicted_set} / window_size
```

Compare to stated coverage `(1 - α)`. Drift severity:

| Empirical coverage range (when stated 90%) | Severity | Action |
|---|---|---|
| 88-92% | Healthy | Continue |
| 85-88% | Soft drift | Increase exploration; surface to operator |
| 80-85% | Hard drift | Pause inference; trigger background retraining |
| < 80% | Catastrophic | Refuse predictions; full retrain from corpus |

### 5.3 Background retraining + A/B/C promotion gating

When hard or catastrophic drift is detected:

1. **Mode A** = current production weights `θ_A`.
2. **Mode B** = retrain from corpus with current data: `θ_B`.
3. **Mode C** = ensemble of A + B (weighted by holdout performance): `θ_C`.

Evaluate all three on the holdout split. Promote whichever wins on:
- Conformal coverage (must be ≥ stated coverage).
- Predicted-vs-actual oracle agreement (must be ≥ A's score).
- Out-of-distribution score distribution (must not regress).

Promotion writes a witness-chain entry: `(timestamp, op_type=ModelPromote, weights_sha, mode_winner, evaluation_summary_sha)`. Old weights are retained for one promotion-cycle then GC'd (per Section 6).

### 5.4 Periodic full retraining

Every N attempts (default `N = 10,000`), trigger a full retrain regardless of drift:

```
schedule_full_retrain():
    snapshot_current_corpus()                # immutable snapshot for reproducibility
    train_from_scratch(snapshot)              # ignores online weights; starts fresh
    evaluate_on_holdout()
    if winner_score > current_score + 0.02:
        promote_weights(new_θ)
```

Full retraining catches drift that online updates accumulated (slow drift not detected by the rolling window) and re-balances the weighting across older vs newer tasks.

### 5.5 Continuous calibration

Every 1,000 attempts, re-compute the conformal threshold on the most recent 1,000 calibration examples (sliding window). This adapts the prediction set size to recent input distribution.

### 5.6 Witness-chain integrity check

Every 10,000 attempts (or on drift), replay the witness chain end-to-end:

```rust
fn verify_chain_integrity(&self) -> Result<ChainIntegrityReport> {
    let chain = self.read_witness_chain()?;
    if chain.len() % WITNESS_ENTRY_SIZE != 0 {
        return Err(ChainCorruption::WrongSize);
    }
    let mut prev_hash = [0u8; 32];
    for (i, entry_bytes) in chain.chunks(WITNESS_ENTRY_SIZE).enumerate() {
        let entry = decode_witness_entry(entry_bytes)?;
        if entry.prev_hash != prev_hash {
            return Err(ChainCorruption::BrokenAt(i));
        }
        prev_hash = shake256_256(entry_bytes);
    }
    Ok(ChainIntegrityReport::Valid { entry_count: chain.len() / WITNESS_ENTRY_SIZE })
}
```

If integrity fails, this is a serious issue (possible disk corruption or tampering); halt inference, surface to operator, restore from last verified checkpoint.

---

## Section 6 — Memory and Disk Hygiene

### 6.1 Storage budget (configurable; defaults below)

| Category | Budget | Tier policy |
|---|---|---|
| Mutation corpus (Phase 0 base) | 5 GB | static; never evicted |
| Live attempt corpus | 50 GB | tiered: hot 8-bit / warm 7 / cool 5 / cold 3 / frozen 1-bit |
| Model checkpoints | 10 GB | retain last 10 + tagged releases |
| Witness chains | 20 GB | append-only with Merkle compression of segments older than 90 days |
| Live panel cache (RAM) | 4 GB | LRU |
| Live panel disk overflow | 20 GB | LRU spill from RAM |
| Calibration snapshots | 5 GB | retain last 30 |
| Total disk | ~110 GB | hard quota |

Operator can override by setting `CG_MEJEPA_DISK_QUOTA_GB` env var.

### 6.2 Tiered storage policy (per `ruvector-temporal-tensor` clean-room reimplementation)

Per-entry access frequency (decayed at 0.99 per 24 hours):

| Frequency | Tier | Bits | Saving |
|---|---|---|---|
| ≥ 0.8 | Hot | 8 | 4× vs f32 |
| 0.6 - 0.8 | Warm | 7 | ~4.6× |
| 0.4 - 0.6 | Cool | 5 | 6.4× |
| 0.2 - 0.4 | Cold | 3 | ~10.7× |
| < 0.2 | Frozen | 1 | 32× |

Per attempt: nightly GC pass demotes entries by one tier when frequency drops below threshold. Entries can be promoted back to higher tiers on access (read = increment frequency).

### 6.3 Garbage collection passes

Nightly cron at 03:00 local:

1. **Demote** entries one tier colder if their access-frequency dropped below the next-tier threshold in the last 24 hours.
2. **Compact** RocksDB column families (run `compact_range_cf` per CF).
3. **Evict** corpus entries whose (uplift × access-frequency × recency) score is in the bottom 1% AND total corpus size exceeds the budget.
4. **Prune** old model checkpoints: keep last 10 + tagged releases; delete older.
5. **Prune** old calibration snapshots: keep last 30; delete older.
6. **Compact** witness chain: see 6.4.
7. **Verify** integrity: run `verify_chain_integrity` at the end; abort if broken.
8. **Report** to `<runtime_root>/mejepa/gc-report-<date>.json`.

### 6.4 Witness-chain Merkle compression

Witness chains are append-only and grow forever. To bound storage while preserving integrity:

For chains older than 90 days:

1. Group consecutive entries into 1,024-entry segments.
2. Compute a Merkle tree over the segment's entries.
3. Replace the segment with a single "compressed-segment" entry containing only the Merkle root + segment metadata (start/end timestamps, entry count).
4. The compressed-segment entry chains to the previous entry's hash and is followed by the next entry, preserving the overall chain structure.

This is git-pack-style compression. Verification of any specific old entry requires fetching the original segment from cold archive, recomputing its Merkle root, and confirming it matches the compressed-segment entry. Verification of the *chain itself* is unchanged.

Storage savings: 1,024 × 73 bytes = 74,752 bytes per segment compresses to 1 × ~100 bytes (Merkle root + metadata) = ~750× compression for old segments.

### 6.5 Hard quota enforcement

If any category exceeds budget, GC eviction triggers immediately (not just nightly). If GC can't bring the category under budget after one full pass, an alert is raised (logged + surfaced to operator). The system continues to serve predictions but stops accepting new data into the over-budget category until manual intervention.

### 6.6 Operations primitives

```rust
mod me_jepa::ops {
    /// Run the full nightly GC pass.
    pub fn gc_run_nightly() -> Result<GcReport>;

    /// Check quotas and trigger emergency eviction if over.
    pub fn quota_check_and_evict() -> Result<EvictReport>;

    /// Drift detection on the rolling window.
    pub fn heal_detect_drift() -> Result<DriftSeverity>;

    /// Trigger A/B/C retrain + promote.
    pub fn heal_retrain_if_needed() -> Result<HealReport>;

    /// Compress old witness-chain segments.
    pub fn witness_compress_old_segments(threshold_age: Duration) -> Result<CompressReport>;

    /// Verify witness chain integrity end-to-end.
    pub fn witness_verify_integrity() -> Result<ChainIntegrityReport>;

    /// Demote one entry one tier colder.
    pub fn tier_demote(entry_id: &EntryId) -> Result<()>;

    /// Promote one entry one tier hotter (called on read).
    pub fn tier_promote(entry_id: &EntryId) -> Result<()>;
}
```

All ops are MCP-callable for operator inspection.

---

## Section 7 — Phase Plan for an AI Agent

This section is the **direct hand-off**: an AI agent picks up here and executes phases sequentially. Each phase has a deliverable, a definition of done, and a verification protocol.

**Pre-phase foundations already present (2026-05-08):**

- `context-graph-witness` can be used for `E_Witness` and audit-chain integrity.
- `context-graph-solver` can be used for Forward-Push importance weighting and CG solves.
- These foundations do not replace Phase 0. The next blocking ME-JEPA-Code step is still the mutation corpus.

### Phase 0 — Workspace + corpus generation (~5-7 days)

**Deliverables:**

1. New crate `crates/context-graph-mejepa-corpus/` with `generate_swe_bench_mutation_corpus()` per Section 3.1.
2. Mutation engines for all 8 categories per Section 3.1, each in its own module.
3. Docker oracle wrapper that runs each mutation against the official SWE-bench evaluator and persists `(mutated_patch, oracle_verdict)` tuples.
4. RocksDB column families: `CF_MEJEPA_MUTATION_CORPUS`, `CF_MEJEPA_ORACLE_VERDICTS`.

**Definition of done:**

- `cargo run --release -p context-graph-mejepa-corpus -- generate --output ./data/mejepa-corpus` produces 2,400 entries across 300 tasks × 8 categories.
- All entries verifiable: `cargo run --release -p context-graph-mejepa-corpus -- verify --corpus ./data/mejepa-corpus` re-runs the oracle on a 5% sample and confirms matching verdicts.
- Stoer-Wagner mincut split (see Phase 1.4) produces train/calibration/holdout splits with no shared-task leakage.

**Verification:**

```bash
context-graph mejepa-corpus stats --corpus ./data/mejepa-corpus
# expected:
#   total_entries: 2400
#   by_category: { known_good: 300, subtle_flip: 300, ..., compile_error: 300 }
#   by_oracle: { pass: ~600, fail: ~1800 }
#   leakage_check: PASS
```

### Phase 1 — Instruments (~10-14 days)

**Deliverables:**

1. New crate `crates/context-graph-mejepa-instruments/` with each `E_*` instrument as a module.
2. Pre-trained encoders where applicable (E_Test, E_Problem, E_CommitMsg reuse `E1` from existing context-graph-embedding).
3. New encoders (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Trace, E_Diff, E_Witness, E_Oracle) as small networks trained against the mutation corpus' supervised signals.
4. Tree-sitter integration for AST, CFG, DataFlow extraction.
5. Pyright/mypy integration for type-graph extraction (Python tasks); rust-analyzer for Rust tasks.

**Definition of done:**

- For each instrument, `instrument.encode(input)` returns a fixed-dim latent vector.
- Round-trip test: encode 100 mutation-corpus examples; the latents differentiate categories at >70% top-1 accuracy on a simple linear probe.
- All instruments run on RTX 5090 sm_120a CUDA via candle.

**Verification:**

```bash
context-graph mejepa-instruments verify --corpus ./data/mejepa-corpus
# expected per-instrument:
#   E_AST: dim=384, latent_separation_score=0.78
#   E_CFG: dim=256, latent_separation_score=0.71
#   ...
```

### Phase 2 — Panel + predictor + target encoder (~7-10 days)

**Deliverables:**

1. New crate `crates/context-graph-mejepa/` with the JEPA architecture.
2. Panel materialization: given a task + a patch attempt, produce `panel[t=0], panel[t=1], panel[t=2]`.
3. Predictor (6-layer transformer, hidden 1024, 8 heads).
4. EMA target encoder (same architecture, frozen-with-EMA-update).
5. VICReg loss implementation.
6. Oracle head on top of predictor's output.

**Definition of done:**

- Forward pass: `panel[t=0..1] -> predictor -> predicted panel[t=2]`. Output dim 4,096.
- Loss is finite on a single-batch dry-run.
- Memory footprint <8 GB VRAM at batch 64 on RTX 5090.

**Verification:**

```bash
context-graph mejepa-train --dry-run --batches 1 --batch-size 64
# expected:
#   forward latency: <50ms/batch
#   loss: finite, ~order(10²) before training
#   no NaN/Inf
```

### Phase 3 — Training loop (~5-7 days)

**Deliverables:**

1. Training entry-point: `cargo run --release -p context-graph-mejepa -- train --corpus ./data/mejepa-corpus --output ./data/mejepa-checkpoints`.
2. Optimizer: AdamW with cosine decay.
3. Per-step training certificate emission (per `04_JEPA_APPLICATIONS.md §9` clean-room reimplementation).
4. Witness-chain entry per epoch.
5. Holdout evaluation every 1,000 steps.

**Definition of done:**

- Training runs to completion (10 epochs on the base 2,400-example corpus) on RTX 5090 in <12 hours.
- Holdout `predicted_oracle_pass`-vs-actual-oracle-pass agreement reaches ≥75% at the end of training.
- Every gradient step has a valid `TrainingCertificate`; chain integrity verifiable end-to-end.

**Verification:**

```bash
context-graph mejepa-eval --weights ./data/mejepa-checkpoints/best.safetensors --holdout ./data/mejepa-corpus/holdout
# expected:
#   prediction_oracle_agreement: ≥0.75
#   training_cert_chain_valid: true
#   ood_score_distribution: median <0.3
```

### Phase 4 — Inference + calibration (~3-5 days)

**Deliverables:**

1. `MeJepaCompiler::compile()` and `::verify()` per Section 4.
2. Conformal calibration on calibration split.
3. OOD detection per Section 4.3.
4. MCP server tool: `mcp__cgreality__mejepa_predict(patch, context) -> RealityPrediction`.

**Definition of done:**

- Single-prediction latency <100ms on RTX 5090 (excluding instrument re-evaluation; instruments cached).
- Calibration coverage on the calibration split: empirical coverage within ±2% of stated `1 - α`.
- OOD score on out-of-distribution synthetic inputs (random AST diffs over unrelated repos): >0.7.

**Verification:**

```bash
context-graph mejepa-infer-test --calibration ./data/mejepa-corpus/calibration --alpha 0.1
# expected:
#   stated_coverage: 0.90
#   empirical_coverage: 0.88-0.92 (within tolerance)
#   prediction_latency_p50: <100ms
#   ood_separation: AUC >0.85
```

### Phase 5 — Self-healing operations (~5-7 days)

**Deliverables:**

1. Online `observe()` API per Section 5.1 with EWC++ regularization (clean-room reimplementation of `crates/sona/src/ewc.rs`).
2. Drift detection per Section 5.2 (rolling 1,000-attempt window).
3. A/B/C retrain + promote pipeline per Section 5.3.
4. Periodic full retrain scheduler per Section 5.4.
5. Continuous calibration sliding window per Section 5.5.
6. Witness-chain integrity verifier per Section 5.6.

**Definition of done:**

- Inject synthetic drift (force 70% empirical coverage on the rolling window); detect within 1,000 observations.
- A/B/C pipeline runs end-to-end on synthetic drift; promotes Mode B only if it wins on holdout.
- EWC++ Fisher info accumulates correctly; after 10 task boundaries, weight protection is non-degenerate (rank-deficient Fisher matrix would be a bug).

**Verification:**

```bash
context-graph mejepa-heal-drill --inject-drift hard
# expected:
#   drift_detected: hard, after ~700 observations
#   retrain_triggered: yes
#   mode_winner: B (or C), never A unmodified
#   promotion_latency: <60min on RTX 5090
```

### Phase 6 — Disk and memory hygiene (~3-5 days)

**Deliverables:**

1. `me_jepa::ops::*` module per Section 6.6.
2. Nightly GC scheduler per Section 6.3.
3. Tiered storage backend per Section 6.2 (clean-room reimplementation of `ruvector-temporal-tensor`).
4. Witness-chain Merkle-compression per Section 6.4.
5. Quota enforcement per Section 6.5.
6. MCP server tools: `mcp__cgreality__mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`.

**Definition of done:**

- Run a 10,000-entry stress corpus through the system; nightly GC keeps disk under 110 GB total.
- Witness-chain Merkle compression on a 100,000-entry chain reduces storage from ~7 MB to ~10 KB (~700× compression).
- Quota enforcement triggers correctly on synthetic over-budget scenarios.

**Verification:**

```bash
context-graph mejepa-stress --entries 10000 --duration 7d
# expected after run:
#   total_disk_used_gb: ≤110
#   gc_passes_completed: 7
#   witness_chain_compressed: yes
#   quota_violations: 0 (after GC)
```

### Phase 7 — Reality-loop integration (~5-7 days)

**Deliverables:**

1. Wire the existing reality-loop's `mcp__cgreality__reality_run_attempt` to call `MeJepaCompiler::observe()` after every attempt, feeding the panel + oracle verdict back into the model.
2. Wire the `cc-reality-harness-editor` agent (or its successor) to call `MeJepaCompiler::verify()` before suggesting a patch acceptance.
3. New MCP tools: `mejepa_predict`, `mejepa_verify`, `mejepa_observe` (latter mostly internal; exposed for testing).
4. Update CLAUDE.md §11 to reflect the integration.

**Definition of done:**

- End-to-end: 1 fresh SWE-bench task runs through the inner loop; attempt produces a patch; ME-JEPA's verify() agrees with the actual Docker oracle outcome at ≥85% on the holdout slice.
- Observation pipeline back-feeds the corpus; one new corpus entry per attempt.
- Witness chain spans both the reality-loop's existing chain and the new ME-JEPA chain (one shared chain, segregated by witness_type).

**Verification:**

```bash
context-graph reality-loop end-to-end --task astropy__astropy-12907
# expected:
#   inner_actor_attempt: completed
#   me_jepa_verify_verdict: APPROVE or ESCALATE
#   me_jepa_predicts_match_oracle: true (or explicitly false with reason)
#   corpus_entry_added: 1
#   total_corpus_size: previous + 1
```

### Phase 8 — Evaluation against the end goal (~ongoing)

The **end-goal metric** is:

```
prediction_oracle_correlation = corr(
    me_jepa.compile(patch).predicted_oracle_pass,
    actual_docker_oracle_pass(patch)
)  over the rolling-window holdout
```

When this correlation crosses ~0.95, ME-JEPA-Code is more reliable than human reviewers at predicting oracle outcomes from patches. At that point, AI-generated patches that pass `me_jepa.verify()` don't need human review.

**Track it weekly. Surface to operator. The end goal is achieved when this metric stably exceeds 0.95 across all 8 mutation categories AND on live reality-loop attempts.**

---

## Section 8 — Open Research Questions

Honest list of bets, not solved problems:

1. **Will JEPA generalize across repositories?** V-JEPA generalized across video domains; I-JEPA across image domains. Whether ME-JEPA-Code generalizes from astropy to django to sympy is an open empirical question. Mitigation: train on a diverse subset, evaluate on held-out repos.

2. **Are 4,096-d panels expressive enough?** This is a hyperparameter; if test agreement plateaus below 0.95, increase to 8,192-d at the cost of doubled compute.

3. **Do we need per-language ME-JEPAs or one unified one?** SWE-bench Lite is Python-heavy. A unified ME-JEPA may underperform on Rust/TypeScript SWE-bench tasks. Phase 7's holdout evaluation surfaces this if it's a problem.

4. **Is the mutation corpus diverse enough?** 8 categories may miss important failure modes (race conditions, timeouts, OOM, network errors). Add categories iteratively as live attempts surface new failure modes.

5. **Can we trust the predictor on novel APIs?** New libraries the model didn't see during training will produce high OOD scores. Need a fallback: explicitly mark "OOD inputs ALWAYS escalate to human" until the corpus catches up.

6. **What's the right `λ_ewc` for online learning?** Too low → catastrophic forgetting. Too high → can't learn new tasks. Per `crates/sona/src/ewc.rs` defaults: `initial_lambda=2000`, `max_lambda=15000`. Tune empirically.

7. **How does this interact with non-SWE-bench code (e.g., the user's own private codebase)?** The mutation corpus is SWE-bench-specific. Generalizing requires either (a) running the user's own code through the mutation engine, or (b) accepting a higher OOD escalation rate.

8. **What do we do when ME-JEPA disagrees with itself across retraining cycles?** Specifically: a patch passed Mode A but fails Mode B's verify(). Resolution: ensemble (Mode C wins) AND surface the disagreement to the operator as a "model uncertainty" signal.

---

## Section 9 — Why Clean-Room (Recap)

Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`, every line of ME-JEPA-Code is implemented in our own crates. The reasons:

1. **The verifier IS the deliverable.** A verifier whose internals we don't fully own can be silently weakened by upstream commits.
2. **Provenance.** Every prediction must be traceable to specific lines of code; foreign crates make this fragile.
3. **Sovereignty.** The reality-loop must run unattended on the operator's hardware indefinitely. No external dependencies.

We READ RuVector's `crates/sona/`, `crates/rvf/rvf-crypto/`, `crates/ruvector-temporal-tensor/`, `crates/ruvector-mincut/`, `crates/ruvector-graph-transformer/`. We REIMPLEMENT them ourselves, with provenance comments citing the inspirational source. We never copy code, never path-dep, never npm-depend.

---

## Section 10 — Cross-References

- **Locked rule:** `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — clean-room only.
- **End-goal context:** `memory/context/end-goal-verifiable-ai-code.md` — operator's framing + design rationale.
- **Auto-memory:** `~/.claude/projects/-home-cabdru/contextgraph/memory/project_end_goal_verifiable_ai_code.md` — cross-session.
- **Sister doc (agentdb):** `docs/agentdbfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` — short mirror; treats agentdb-derived ideas as inspiration sources.
- **Findings:** `docs/ruvectorfindings/{02,03,04,05}.md` — primitives, applications, triage. **Read these for the *inspirations*; this doc 09 is the *plan*.**
- **ccreality two-loop architecture:** `docs/ccreality/01_ARCHITECTURE.md` — the existing Claude+Qwen reality loop. Phase 7 reuses this loop's outputs as ME-JEPA's training data (live attempts).
- **Existing JEPA design:** `backupdocs/dynamicjepa/` (universal world-model protocol), `backupdocs/mejepa/` (recruiting-domain ME-JEPA), `backupdocs/5090jepa/` (RTX 5090 demo plan). ME-JEPA-Code is a domain pack ("code_correctness.v1") within the dynamicjepa runtime — see `backupdocs/dynamicjepa/02_core_thesis.md` for the universal-runtime story; this doc specializes it for code.
- **5090jepa rules:** `backupdocs/5090jepa/00_TASK.md` R3 (no mock data), R4 (FSV mandatory), R5 (trigger-outcome verification mandatory) — directly satisfied by every Phase verification protocol above.

---

## Section 11 — One-Line Summary

**Build a reality compiler. Train it on a labeled mutation corpus of the 300 SWE-bench Lite tasks. Wire it into the reality loop so every attempt feeds back as training data. Self-heal on drift. Bound disk and memory. When prediction-oracle correlation crosses 0.95, code review of AI-generated patches is obsolete. That is the end goal.**
