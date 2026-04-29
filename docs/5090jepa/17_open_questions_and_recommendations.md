# 17 — Open Questions & Recommendations

Decisions are locked where the demo's correctness depends on them. Where there is room to change strategy without breaking FSV, options + tradeoffs are recorded so the next agent can decide with context.

## 1. Locked Decisions (Do Not Change Without User Sign-Off)

| Decision | Locked Choice | Why |
| --- | --- | --- |
| New crate placement | `crates/context-graph-dynamicjepa` | Matches `02_target_module_and_crate_layout.md` |
| Storage integration | DynamicJEPA CFs added to the existing physical RocksDB | Atomic batches across DJ + existing CFs become possible later; one source of truth |
| Migration policy | Wipe DB on schema change, **no migrators** | Operator's R1 |
| Domain pack format | TOML only for v1 | Plan §15 recommendation; ambiguity-free typing |
| Adapter | `json_event` only, declarative path mapping | Smallest surface that handles both demo packs |
| Instruments | `scalar`, `categorical`, `onehot`, `time` only | No embedder dependency; covers both packs |
| Loss | latent MSE + VICReg variance + VICReg covariance | Empirically robust collapse prevention; simplest to monitor |
| Target encoder | EMA copy of online encoder, no grad | Standard JEPA |
| Demo packs | `counter_world.v1` + `gridworld_5x5.v1` | Two unrelated domains is the minimum for "universal" |
| Convergence acceptance | Asserted in tests, not in user docs | Test failure is the single source of truth |
| Tests use real RocksDB under `tempfile::TempDir` | Required by R3 |
| Hash algorithm | SHA-256 for files / payload hashes; BLAKE3-128 for short keys | SHA-256 for "audit me" hashes; BLAKE3 for performance-critical key derivation |

## 2. Open Questions (Demo Can Ship With Defaults; Revisit Later)

### Q1 — Should hypothetical actions live in `dj_actions` or a separate CF?

**Default**: same CF, `action_origin = Hypothetical`.

**Tradeoff**: Pros of one CF: simpler reads at planning time. Cons: every list operation must filter by `action_origin` to get observed actions only.

**Recommendation**: keep one CF for the demo. Add a secondary index `dj_actions_observed_by_domain` if read-heavy workloads emerge.

### Q2 — Should the demo include a `surprise → retrain` loop?

**Default**: no. Surprise events are persisted but the demo's verification harness never triggers retraining.

**Tradeoff**: Pros of including: proves the closed loop. Cons: doubles training time, complicates FSV evidence, requires deciding when to retrain.

**Recommendation**: ship without. Add as Phase 13 in a follow-up — easy to bolt on because the storage records are complete.

### Q3 — Should artifacts include the EMA target encoder?

**Default**: yes — `model.safetensors` packs `online.*`, `predictor.*`, `target.*`.

**Tradeoff**: target encoder is needed for resuming training and for re-encoding observed panels at surprise time. Including it doubles encoder param storage (still trivial in absolute terms).

**Recommendation**: keep included. Inference-only deployments could later add a `--lite` artifact format that drops the target.

### Q4 — Should the planner explore beyond the declared candidate list?

**Default**: no — `enumerate_declared_actions` only.

**Tradeoff**: Pros: deterministic, easy to audit. Cons: cannot discover novel actions.

**Recommendation**: keep declared-only for the demo. A `random_subspace` skill is worth adding once the surprise→retrain loop exists, because random exploration becomes useful only when the model can update.

### Q5 — Should we use `.safetensors` or candle's native `.npz`?

**Default**: `.safetensors` (binary tensor file with named tensors and metadata).

**Tradeoff**: safetensors is HuggingFace-standard and easy to inspect (`safetensors_inspect` CLI). Candle natively supports both.

**Recommendation**: keep safetensors.

### Q6 — Should the demo's evidence directory be committed?

**Default**: no — `tmp/` is in `.gitignore`.

**Tradeoff**: committing would let reviewers see evidence without re-running. But the directory contains DB paths and run UUIDs that change every run; commits would always be noisy.

**Recommendation**: keep ignored. Provide a tarball-and-attach-to-PR convention via Section 9 of `15_runbook.md`.

### Q7 — Should we expose a `dynamicjepa export-onnx` for the trained model?

**Default**: no.

**Tradeoff**: ONNX export would let downstream consumers run the model without candle. Cost: another path to maintain, and ONNX of a 13K-param MLP is overkill.

**Recommendation**: skip for the demo. The trained model is for proving the loop, not for downstream consumption.

## 3. Things The Web Research Flagged As Worth Considering Post-Demo

Brief mentions only — implementing any of these is out of demo scope.

- **C-JEPA / VICReg-based JEPA papers** (2024) confirm that EMA alone does not prevent collapse and explicit variance/covariance constraints are required. The demo's loss already follows this. If we ever swap to BYOL/SimSiam-style asymmetry, re-add the VICReg term as a guard.
- **V-JEPA-2-AC** (Meta 2025) is a 300M-param action-conditioned video JEPA. Out of scope but a reference point — its action-conditioning pattern (concatenate action embedding into the predictor input) is the same as ours; we just have a much smaller predictor.
- **TD-JEPA** (NeurIPS 2025) trains policy-conditioned multi-step predictors directly in latent space. If/when the demo extends to multi-step rollouts (Phase 13), the TD-JEPA loss formulation is worth reading.
- **LeWorldModel (LeWM)** — end-to-end action-conditioned JEPA trained reward-free on offline trajectories. Architecturally close to what the demo does in miniature; consult its github (`lucas-maes/le-wm`) when scaling up.
- **CUDA 13 family-specific arch (`compute_120f`)** introduces "full" SM120 features. Not relevant for the demo (we use plain `compute_120`), but worth tracking for future kernel optimization.

## 4. Risks To Track

| Risk | Mitigation |
| --- | --- |
| Candle 0.9.2-alpha is alpha; could regress on Blackwell | Pin the version; record actual `cargo tree -p candle-core` output in evidence; surface any `Could not start backend: Runtime compute cap` in `16_…` §2.1 |
| WSL2 CUDA passthrough flake on first kernel launch | First-call warmup before training timer; documented in `16_…` §6 |
| Tiny model overshoots into capacity that memorizes both training rows | Counter_world only has 1 train row; assert `random_init_baseline` and `shuffled_target_baseline` per `10_…` §7–8 |
| Operator's `data/contextgraph_data` corruption from a misconfigured demo | Demo strictly uses `/tmp/5090jepa-*` paths; environment probes refuse to start if the demo path equals the production path |
| A future contributor adds an `if domain == ...` branch | `tests/universality_audit.rs` runs both packs against the same DB; CI would catch domain-specific code |
| FSV harness becomes flaky due to non-determinism in candle | Force `cudnn_benchmark = false`, fixed seeds; `10_…` §9 |

## 5. What "Done" Looks Like Beyond The Demo

If the operator says "this looks good, what's next?":

1. **Phase 11** of the full plan: `context_graph_memory.v1.toml` and `learner_state.v1.toml`. Ingest existing Context Graph data into DynamicJEPA without changing core types. Proves real production data flows through the same pipeline.
2. **Multi-step objectives**: extend dataset compiler to emit n-step prediction rows; extend predictor to emit a sequence; assert `n=4` rollouts maintain ≥ 0.85 cosine.
3. **Surprise→retrain loop**: implement the closed loop; add a `dynamicjepa retrain --domain ...` command that wakes when surprise count exceeds threshold.
4. **Real-world domain pack**: e.g. `code_repository.v1` reading git diffs / test outcomes. This is where the universality claim pays off in practice.

None of these block the demo's acceptance.
