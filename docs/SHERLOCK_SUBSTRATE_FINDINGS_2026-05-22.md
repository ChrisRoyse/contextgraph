# Sherlock Holmes — Substrate + Data-Integrity Forensic Audit (2026-05-22)

Investigator: Sherlock Holmes (Opus 4.7)
Scope: data-substrate / corpus / embedders / instruments / Q4-freeze enforcement
Ship-gate context: `prediction_oracle_correlation = 0.7942190170` (raw 0.4843, rank-safe 0.6917). Goal ≥ 0.95 stable × 4 windows. Panel B unmeasured.

## Post-audit correction — 2026-05-23

The substrate findings remain valid for the legacy v1 forward cache: `e2/e3/e4` collapsed from non-semantic `chunk.time` injection, and legacy `e8` aliases `e1` before commit `ff4d4c03`. The trainer finding is also valid for the public Rust `Trainer`: it is certificate-only and not ship-gate-countable.

The open question about whether the `0.7942` run used a random-init checkpoint has since been narrowed by direct aiwonder readback. The latest ship-gate FSV loaded an out-of-band trained checkpoint with `optimizer_steps=64` and `weight_delta_observed=true`; #683 remains P0 because that path is not unified with the public `Trainer` contract and does not expose ship-gate countability fields in the training certificate lineage. Use #712/#683 as the current interpretation.

## Summary

- **e2/e3/e4 collapse mechanism identified and is structural, not a regeneration-fixable bug.** Old prod runner `python_forward_cache_fsv.rs` line 2308 (commit 2865672d, in effect 2026-05-15) injects `format!("timestamp:{}\n{}", chunk.time, chunk.source_text)` into the embedder input. `chunk.time` is the AST-chunker run-timestamp, NOT a per-chunk semantic time. Every one of 399,777 chunks carries the SAME `chunk.time` → sinusoidal-PE → SAME output vector. The fail-closed gate at `forward.rs:682` is bypassed by this constant-prefix injection.
- **e8 ≡ e1 byte-for-byte in the prod corpus.** Direct readback at `/mnt/d/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-c8117f15-20260515/shards/shard-000006-offset-000000768/packed_records/{e1,e8}.jsonl` confirmed the same 1024-d vector for chunk `sha256:5d368f24...`. Root cause: `embed_batch()` in `crates/context-graph-embeddings/src/models/pretrained/graph/model.rs` was missing the `project_source_vector` call before 2026-05-22 commit `ff4d4c03` ("Fix graph embedder source projection"). The prod corpus was generated 7 days BEFORE the fix landed.
- **The trainer is structurally DIAGNOSTIC-only.** `crates/context-graph-mejepa-train/src/trainer/mod.rs:31` constant `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE` plus hardcoded `predictor_parameter_update_count: 0` (line 417), `trained_predictor: false` (line 416), `ship_gate_countable_training: false` (line 419). **The predictor never receives a gradient update from the Rust-side trainer.** (Already filed as #683 / #596.)
- **`FrozenGuard::assert_no_gradient` runtime hook is never invoked from training.** Only callsite is `frozen_hook.rs:206` (its own test module). The doctrinal "MEJEPA_INSTRUMENT_GRADIENT_LEAK" gate exists as code but is wallpaper — no production training step calls it. (Already filed as #620.)
- **OOD gate fails OPEN under strict mode.** `compiler.rs:730` `ood_threshold_for_compilation` returns `1.0` when the calibrator is missing AND `require_ood_calibrator=true`. Because OOD scores are normally below 1.0, the gate is effectively DISABLED whenever production calibrator is unavailable. This is fail-open, not fail-closed — opposite of doctrine.
- **Doctrine §1.5ter (slot identity is sacred) is violated in 5+ production paths.** `latent_search.rs:374`, `heal/distill.rs:155`, `heal/pipeline.rs:714` & `:784`, `compiler.rs:1405` all read `panel.data()` as a flat 5,120-float buffer and aggregate across embedder boundaries. Two of these (`distill.rs`, `latent_search.rs`) are on the live heal-scheduler path.
- **`delete_test_call` mutation operator is semantically blind.** `crates/context-graph-mejepa-corpus/src/delete_test_call.rs:67-88` picks one assertion line uniformly at random from `ASSERTION_PREFIXES`. It does NOT distinguish test-file `assert`, production `assert`, `pytest.raises(...)` context-block, or `self.assertEqual`. Most deletions in test files leave the Docker oracle unchanged (3/300 failures, 0 calibration failures). Regeneration via #615 is necessary and correct; the operator is structurally underspecified, not random-bad-luck.
- **Q4 doctrine freeze IS structurally enforced.** `mejepa/src/q4_trust_gate.rs:33` `Q4_DOCTRINE_FREEZE_ACTIVE = true`, propagated via `freeze_q4_head_readiness()` → forces `trusted_in_decision=false` on every head, and `trusted_q4_consequences()` returns empty `Vec` for all Q4 heads. Reaches `verdict_assembly.rs:38`, `prediction_surfaces.rs:273/279`, `objective_safety.rs:285`. Trust verified.

## Embedder health audit

Direct readback from local legacy corpus `/mnt/d/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-c8117f15-20260515/shards/shard-000006-offset-000000768/packed_records/*.jsonl` (127 chunks for this shard, generated 2026-05-15 by the old `python_forward_cache_fsv.rs` runner):

| Slot | Status | Unique vectors / total | Verdict | Evidence |
|------|--------|------------------------|---------|----------|
| e1 (semantic e5-large-v2) | HEALTHY | 88/112 (78.6%) | Real signal | distinct vec per chunk, mean=0, L2=1.0 |
| e2 (TemporalRecent) | **COLLAPSED** | 1/127 | Constant vector | `[-0.022237888, -0.058409985, ...]` identical for every chunk |
| e3 (TemporalPeriodic) | **COLLAPSED** | 1/127 | Constant vector | identical for every chunk |
| e4 (TemporalPositional) | **COLLAPSED** | 1/127 | Constant vector | identical for every chunk |
| e6 (Sparse 30K) | HEALTHY | ~88 unique | Real signal (size ~2.5MB) |
| e7 (Code Qwen2) | HEALTHY | ~120 unique | Real signal (size ~2.7MB) |
| e8 (Graph) | **DUPLICATE-OF-E1** | 88/112 ≡ e1 set | Byte-identical to e1 | full vector equality, identical `vector_sha256` |
| e9 (HDC) | HEALTHY | ~120 unique | Real signal (CUDA HDC) |
| e10 (Contextual paraphrase) | HEALTHY | 88/112 (78.6%) | Real signal |
| e12 (ColBERT late-interaction) | HEALTHY | 88/112 | Real signal |
| e13 (SPLADE sparse) | HEALTHY | ~88 | Real signal (largest, 2.5MB) |
| e14 (BGE-M3 dense) | HEALTHY | 103/127 (81.1%) | Real signal |

**Real signal: 8 slots (e1, e6, e7, e9, e10, e12, e13, e14). Degenerate: 4 slots (e2, e3, e4, e8).** Matches #666/#670 closeout.

### Why e2/e3/e4 collapsed (the structural mechanism)

1. The OLD `crates/context-graph-mejepa-embedders/examples/python_forward_cache_fsv.rs:2308` (commit `2865672d`) wraps E2/E3/E4 inputs:
   ```rust
   let text = if matches!(embedder, EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4) {
       format!("timestamp:{}\n{}", chunk.time, chunk.source_text)
   } else {
       chunk.source_text.clone()
   };
   ```
2. `chunk.time` is set in `scripts/swebench_lite_ops/python_ast_chunks.py:294` to `now` — the AST chunker's wall-clock run time. Since the entire 300×8 corpus was chunked in a single run, **all 399,777 chunks share the same `chunk.time` ISO string** (per-second precision).
3. The temporal embedder forward path then computes `parse_timestamp("timestamp:<chunk.time>")` → same Unix timestamp → same sinusoidal positional encoding → same 512-d vector → same `vector_sha256`.
4. The fail-closed gate at `forward.rs:682` (`temporal_instruction()` returning `None`) is NOT triggered because `chunk.time` IS prefixed with `timestamp:`. The gate accepts the input as well-formed and produces a fully-valid but **uninformative** output.
5. The current runner `python_forward_cache_readback.rs` does NOT inject this prefix (it sends `text=chunk.source_text` raw, line 560). If re-run against E2/E3/E4 it would fail-closed loudly, which would surface the issue.

### Why e8 ≡ e1 (the structural mechanism)

1. `GraphModel::embed_batch()` in `crates/context-graph-embeddings/src/models/pretrained/graph/model.rs` was missing the `project_source_vector` call BEFORE commit `ff4d4c03` (2026-05-22 22:37).
2. Before the fix: `embed_batch` returned the raw e5-large-v2 forward output (after `passage:` prefix and mean pooling) — exactly the same path as `SemanticModel::embed_batch`. The "perturbed identity" projection matrix was constructed in memory but never applied.
3. Prod corpus generated 2026-05-15 → 7 days BEFORE the fix. Therefore e8 outputs are bit-equal to e1 outputs.
4. Direct evidence: `e1.jsonl[0]['output']['vector'] == e8.jsonl[0]['output']['vector']` (full 1024-float equality), same `vector_sha256` (`sha256:64d62bd0...`).
5. Single-`embed` path (used outside batching) DID apply `project_source_vector` even before `ff4d4c03`. The batch path was the leaking variant.

### Frozen-instrument freeze: structural OK, runtime check absent

- Static check: every active pretrained embedder slot in the `construct_model()` factory is a wrapped `EmbeddingModel` without any optimizer or `requires_grad` enablement. There is no `loss.backward()` chain reaching their parameters. The Rust trainer is `DIAGNOSTIC_CERTIFICATE_ONLY` so no gradient flows anywhere.
- The structural collapse-immunity guarantee (ΔΞ.target_collapse = 0) holds **because no training happens**.
- `FrozenGuard::assert_no_gradient` defensive hook exists at `crates/context-graph-mejepa-instruments/src/frozen_hook.rs:87` but is never invoked from production training (only from its own test). #620 already filed.
- The "online mistake-driven loop" (#406/#523/`mistake_log.rs`/`online_head_state.rs`) is **read/write CRUD only** — there is no `consume_mistake → gradient → optimizer.step → predictor weights updated` chain. It tracks `mistake_repeat_rate` as a passive metric. (Already filed as #683.)

## Label substrate honesty

The 491-row accepted-labels registry (#413), 128 named skills (#414), 379 Level-3 constellations (#404), and 36,773 chunk→skill memberships (#417 read-only slice) were all derived from the 12-slot forward cache that contains 4 degenerate slots (e2, e3, e4, e8) and 8 real-signal slots.

**Honest under admissibility overlay:**
- `python_geometry_label_atlas_materializer.py:234-260` loads `geometry_label_space_policy.json` and filters to `eligible_independent_embedder_ids = [e1, e6, e7, e9, e10, e12, e13, e14]`. Constellations and atlas-materialized labels DO restrict to these 8 valid spaces.
- `python_label_quality_signal.py:558-560` loads the `geometry_label_admissibility.jsonl` overlay before computing label quality.

**Partially honest at registry layer:**
- `python_auto_label_discovery.py` does NOT consume the admissibility overlay. It emits slot labels (`slot:e2:present`, `slot:e2:family:temporal_coordinate`, `slot:e2:top_abs:pos:bucket_05`, etc.) for ALL 12 embedders regardless of degeneracy.
- These slot labels are descriptors of the slot, not classifiers of chunks, so they don't inject false signal into prediction. But any downstream consumer that joins the 491-label registry without going through the admissibility overlay will count `slot:e2/e3/e4/e8:*` rows as live labels.
- Net assessment: the 491-row registry contains a non-trivial fraction of degenerate-slot labels. **Estimate**: with 4/12 slots degenerate and ~26 labels emitted per slot per chunk in `slot_labels()`, roughly 33% of the slot-family/top-abs label population is informationally empty. The semantic / AST-surface / mutation-mechanism labels (in the 491 set but NOT slot-prefixed) remain honest.

**Constellation honesty:** The 379 Level-3 constellations (#404) are derived from labels that flow through the admissibility filter. They are honest. The hierarchy holdout/exemplar FSV closed 2026-05-20 was on the post-admissibility surface.

**Skill honesty:** 128 named skills (#414) are derived from constellation IDs, not raw labels. Inherits constellation honesty.

**Mendeleev pair table (#583/#585):** Recently closed (2026-05-22 commit `fed6fdf3`). New code in `python_pair_surface_mining.py` and `python_mendeleev_pair_table.py` gates rankings by `valid_independent_embedder_ids` from the same policy. Honest.

## delete_test_call structural verdict

**Verdict: STRUCTURALLY BROKEN OPERATOR — fixable only by category split (#615 is correct).**

Evidence from `crates/context-graph-mejepa-corpus/src/delete_test_call.rs:40-65`:
```rust
const ASSERTION_PREFIXES: &[PrefixMatch] = &[
    PrefixMatch { prefix: "assert ", ... },
    PrefixMatch { prefix: "assert(", ... },
    PrefixMatch { prefix: "self.assert", ... },
    PrefixMatch { prefix: "pytest.raises", ... },
    PrefixMatch { prefix: "with pytest.raises", removes_context_block: true },
    PrefixMatch { prefix: "pytest.assume", ... },
];
```
And `apply()` at line 67-88 picks ONE candidate uniformly at random and replaces with `pass`.

The operator has no awareness of:
1. **File kind** — whether the source file is `test_*.py` (deleting `assert` is usually safe; the test was the one asserting) vs production `*.py` (deleting `assert` removes a runtime guard).
2. **Assertion semantic role** — whether the `assert` is a precondition guard, a postcondition check, a test fixture validation, or a debugging aid.
3. **pytest.raises context block** — removing it transforms expected-exception logic into pass-through; usually a definite Fail.
4. **Whether the assertion is on a hot path** — a deleted assertion in dead code has no oracle impact.

This explains the empirical 3/300 oracle-failure rate: when most assertions live in test files (typical Python project ratio: 60-80% of test-prefix assertions are in `test_*.py`), deleting them just no-ops the test assertion. The Docker oracle's PASS_TO_PASS set still passes because the test was the only thing that would have failed.

The operator IS deterministic and semantically correct (it does what it says: delete one assertion line). But the CATEGORY label `delete_test_call` then conflates semantically distinct subfamilies into one bucket. The predictor cannot learn a "Pass | Fail" decision boundary because the same category label maps to mostly-Pass and rarely-Fail with no learnable structure.

#615 (TASK-PY-G-135C, OPEN p0) prescribes the correct fix: split into `production_plain_assert_deleted`, `test_self_assert_deleted`, `pytest_raises_deleted`, `other_assertion_like_deleted`. Each subfamily then carries a learnable Pass/Fail distribution. **Regeneration is necessary and sufficient.**

## Slot-preserving doctrine violations

Beyond #618 (`compute_ood_score` over packed buffer) and #619 (`inverse_target_proj = linear(PANEL_DIM, hidden)`), the following additional violations exist in the live trunk:

| File:line | Code | Doctrine breach |
|-----------|------|------------------|
| `crates/context-graph-mejepa/src/latent_search.rs:374` | `panel_t0.data().iter().zip(panel_t1.data()).enumerate()` then accumulates `delta = after - before` into 32-d latent | Cross-embedder dim-wise delta; e.g. dim 0 of E_AST vs dim 0 of E_Code |
| `crates/context-graph-mejepa/src/heal/distill.rs:155` | `let panel_mean = panel.data().iter().take(256).sum::<f32>() / 256.0;` | First 256 dims span E_AST + part of E_CFG; averaging them is meaningless |
| `crates/context-graph-mejepa/src/heal/pipeline.rs:714` | `panel.data().iter().take(128).map(|v| v.abs()).sum::<f32>() / 128.0` | First 128 dims are E_AST only; arguable BUT the "128" magic number is brittle to dim changes |
| `crates/context-graph-mejepa/src/heal/pipeline.rs:784` | (similar pattern) | Same brittle slicing |
| `crates/context-graph-mejepa/src/compiler.rs:1405` | `for value in panel.data()` (full-flat iteration) | Treats panel as single 5,120-vector |
| `crates/context-graph-mejepa/src/panel_source.rs:58` | `for (col, base) in panel.data().iter().enumerate()` | Iterating across embedder boundaries as if homogeneous |

`distill.rs` is the most serious because the resulting `panel_mean` is added to per-embedder `student_score` (line 168) and the combined value feeds an InfoNCE loss against the oracle — meaning a cross-embedder aggregate IS reaching a production loss function. (See "Undocumented brokenness" UB-3 below for issue filing.)

## Undocumented brokenness

### UB-1 (HIGH): Old forward-cache runner injects constant `chunk.time` into temporal embedder input

- **File**: `crates/context-graph-mejepa-embedders/examples/python_forward_cache_fsv.rs:2308` (commit `2865672d` and earlier)
- **Expected**: per-chunk semantic timestamp injected; if absent, fail-closed
- **Actual**: `format!("timestamp:{}\n{}", chunk.time, chunk.source_text)` — `chunk.time` is the AST-chunker's wall-clock run time, same for every chunk in a 300×8 batch. Result: 399,150 records with 1 unique `vector_sha256` per embedder.
- **Severity**: HIGH. This is the root cause of #666/#670 e2/e3/e4 collapse. Any regeneration of E2/E3/E4 from the same AST-chunks substrate without a semantic per-chunk timestamp will collapse identically.
- **Readback**: 
  ```bash
  python3 -c "import json; rs=[json.loads(l) for l in open('/path/to/e2.jsonl')]; vs={r['output']['vector_sha256'] for r in rs}; print(f'{len(rs)} rows, {len(vs)} unique vectors')"
  ```
  Expected: `unique vectors / row count ≥ 0.7`. Actual on the corpus: `1 / 399,150`.
- **Filing as new issue.**

### UB-2 (P0): `e8` GraphModel `embed_batch` missing projection before commit `ff4d4c03`

- **File**: `crates/context-graph-embeddings/src/models/pretrained/graph/model.rs:234-260` (pre-`ff4d4c03`)
- **Expected**: `embed_batch` projects through `project_source_vector` and L2-normalizes (matches single `embed` at line 477-518)
- **Actual** (pre-fix): returns raw `gpu_forward_text_batch` output unchanged. Identical to `SemanticModel::embed_batch` output for the same input.
- **Severity**: P0 for the corpus generated 2026-05-15 to 2026-05-22. Already structurally fixed by `ff4d4c03`. Corpus regeneration required to recover e8 signal.
- **Readback**: 
  ```bash
  python3 -c "import json,sys; e1=json.loads(open('e1.jsonl').readline()); e8=json.loads(open('e8.jsonl').readline()); print('equal:', e1['output']['vector']==e8['output']['vector'])"
  ```
  Expected: `False` post-fix.
  Actual on prod corpus: `True`.
- **Already known** via #666/#670. New finding: the post-fix `python_forward_cache_readback.rs` runner needs an FSV harness that ASSERTS `vec_sha(e1) != vec_sha(e8)` for at least 50% of chunks, otherwise regression will silently re-introduce the alias.
- **Not filing new issue** (#670 covers this); will add a comment instead.

### UB-3 (P1): `heal/distill.rs:155` cross-embedder `panel_mean` reaches production InfoNCE loss

- **File**: `crates/context-graph-mejepa/src/heal/distill.rs:155-168`
- **Expected**: distillation signal stays within per-embedder space; doctrine §1.5ter forbids cross-embedder dim-wise aggregation
- **Actual**: `let panel_mean = panel.data().iter().take(256).sum::<f32>() / 256.0;` then `info_nce_loss(student_score + panel_mean, teacher_score, ...)`. The `panel.data()[0..256]` slice spans E_AST (384 dims) and part of E_CFG (256 dims) — a meaningless cross-embedder average. The result tilts the InfoNCE target by a corpus-mean-like scalar that has no slot semantics.
- **Severity**: P1 BLOCKER for any future ship-gate work that depends on heal/distill outputs. Impact-bounded today because heal-distill is not on the predictor's main forward-train path (the trainer is certificate-only), but it does run on the live heal scheduler.
- **Readback**: code search `grep -n 'panel.data().iter().take(256)' crates/context-graph-mejepa/src/heal/distill.rs` returns line 155.
- **Filing as new issue.**

### UB-4 (P1): `latent_search.rs:374` cross-embedder dim-wise delta in production latent-action computation

- **File**: `crates/context-graph-mejepa/src/latent_search.rs:373-394`
- **Expected**: latent action delta computed per-slot, then folded slot-preservingly
- **Actual**: `panel_t0.data().iter().zip(panel_t1.data()).enumerate()` then `delta = after - before` accumulated into a 32-d latent. Pairs E_AST[0] with E_AST[0] (OK) AND E_AST[383] - E_CFG[383] (cross-embedder), and so on. The 32-d latent is then re-used as a candidate-ranking primitive via `candidate_ranking.rs:8`.
- **Severity**: P1 — candidate ranking surface is exposed to the operator.
- **Readback**: 
  ```bash
  grep -n "panel_t0.data().iter().zip(panel_t1.data())" crates/context-graph-mejepa/src/latent_search.rs
  ```
- **Filing as new issue.**

### UB-5 (P0): `compiler.rs:730` `ood_threshold_for_compilation` returns `1.0` in strict mode → OOD gate disabled

- **File**: `crates/context-graph-mejepa/src/compiler.rs:730-739`
- **Expected**: when calibrator is missing AND `require_ood_calibrator=true`, fail-closed (refuse to compile / emit explicit `OOD_CALIBRATOR_MISSING` error)
- **Actual**: returns `Ok(1.0)`. Verdict assembly then compares `ood_score >= 1.0`, which is essentially `false` for any well-formed score in `[0,1]`. The OOD gate is silently disabled.
- **Severity**: P0 — fail-open of a safety gate is a doctrine violation (CLAUDE.md §6.4 "fail closed on unknowns").
- **Readback**: 
  ```bash
  grep -A4 "fn ood_threshold_for_compilation" crates/context-graph-mejepa/src/compiler.rs
  ```
- **Already noted in #629 parking lot ("ood_threshold_for_compilation returns 1.0 in strict mode when calibrator is missing"). Promoting to a real issue.**
- **Filing as new issue.**

### UB-6 (P2): AST chunker's `chunk.time` is the chunker run timestamp, not semantic

- **File**: `scripts/swebench_lite_ops/python_ast_chunks.py:294` writes `"time": now` where `now` is the start-of-chunker wall-clock time
- **Expected**: per-chunk semantic timestamp (e.g., git blame committer-date for the file, or task-instance committed-at, or source-file mtime)
- **Actual**: every chunk shares the same `time` field
- **Severity**: P2 because the OLD runner's injection of `chunk.time` into temporal embedder input is what triggered the collapse (UB-1). The chunker itself is logically consistent — `time` is the "when this chunk was emitted" — but downstream consumers misinterpret it as semantic time.
- **Readback**: 
  ```bash
  python3 -c "import json; ts={json.loads(l)['time'] for l in open('/path/to/python_ast_chunks.jsonl')}; print(f'unique time values: {len(ts)}')"
  ```
  Expected if chunker had per-chunk semantic timestamps: 399,777.
  Actual: 1.
- **Filing as new issue.**

### UB-7 (P1): No FSV gate asserts inter-embedder vector-space distinctness

- **File**: every embedder FSV harness in `crates/context-graph-mejepa-embedders/examples/`
- **Expected**: a per-cache-run gate that computes `vector_sha256_set(e_i) ∩ vector_sha256_set(e_j) / vector_sha256_set(e_i) ∪ vector_sha256_set(e_j)` (Jaccard) for every pair, and fails closed if any non-related-pair Jaccard exceeds ~0.5.
- **Actual**: no such gate. The label-discovery `vector_space_health_from_hash_counts()` function exists in `scripts/swebench_lite_ops/python_auto_label_discovery.py:588` but is only run at label-discovery time, AFTER 399,150 collapsed records have already been written to durable storage.
- **Severity**: P1 — without this gate, future regenerations will silently re-introduce the same collapse/alias pattern.
- **Readback**: 
  ```bash
  grep -rn "vector_space_health\|inter_embedder_jaccard\|cross_embedder_distinctness" crates --include="*.rs"
  ```
  Expected: at least one per-shard FSV gate.
  Actual: 0 results in `crates/`.
- **Filing as new issue.**

### UB-8 (P2): `Panel::filled_mask` accepts a constant slot vector as "filled"

- **File**: `crates/context-graph-mejepa-instruments/src/lib.rs:411-419` (PanelBuilder::set_slot)
- **Expected**: a degenerate / constant vector should be detected and either refused or marked as `is_degenerate(slot)=true`
- **Actual**: the only validity check is finite-values + correct dim. A panel filled with `[constant E2 vector × 512]` will have `is_filled(EReasoning_temporal)=true`. Downstream consumers (predictor, OOD score, holdout evaluation) treat this slot as informative.
- **Severity**: P2 — defense-in-depth for the substrate. Would have caught the e2/e3/e4 collapse at panel-build time even if upstream embedder cache failed.
- **Readback**: 
  ```rust
  // Synthetic test:
  let constant_vec = vec![0.1_f32; 512];
  let panel = PanelBuilder::new().set_slot(InstrumentSlot::EReasoningTemporal, &constant_vec)?.build()?;
  assert!(panel.is_filled(InstrumentSlot::EReasoningTemporal));  // Currently passes; should fail or warn
  ```
- **Filing as new issue.**

### UB-9 (P3): `python_auto_label_discovery.py` emits slot labels for all 12 embedders without admissibility filter

- **File**: `scripts/swebench_lite_ops/python_auto_label_discovery.py:576-585`
- **Expected**: emit slot labels ONLY for embedders that pass the admissibility overlay (`status != degenerate / duplicate`)
- **Actual**: `slot_labels()` is called unconditionally for every active embedder, including the degenerate `e2/e3/e4` and aliased `e8`. The labels are descriptive ("`slot:e2:present`", "`slot:e2:family:temporal_coordinate`") not classifiers, so they don't injure prediction directly — but any downstream raw-registry consumer counts them.
- **Severity**: P3 — recoverable by admissibility overlay if applied at every consumer.
- **Readback**: 
  ```bash
  grep -n "slot:e2:" docs/SHERLOCK_SUBSTRATE_FINDINGS_2026-05-22.md  # this file
  # OR in the registry:
  jq 'select(.label | startswith("slot:e2:"))' /path/to/accepted_labels.jsonl | wc -l
  ```
- **Filing as new issue.**

## Critical substrate jumps (ranked impact on predictor convergence)

1. **Regenerate the forward cache after retiring `chunk.time` injection** (highest impact). Replace the OLD `python_forward_cache_fsv.rs:2308` injection with either (a) per-chunk semantic timestamps from git blame committer-date / task instance committed-at, OR (b) drop E2/E3/E4 from active embedders for static AST corpora (declare them temporal-only-for-live-data slots). Expected uplift: 4 currently-dead embedder spaces become 0 to 3 real spaces, depending on (a) vs (b). Doubles real-slot count under (a).

2. **Regenerate the corpus with the post-`ff4d4c03` `embed_batch` projection fix** for e8. Expected uplift: 1 currently-aliased embedder space becomes 1 real space. Combined with #1, brings real-slot count from 8 to 12 (matching the declared SUPPORTED_FORWARD_EMBEDDERS).

3. **Complete the `delete_test_call` category split per #615** (TASK-PY-G-135C). Move 3/300 oracle-failure cell from "unlearnable cross-mixed semantic class" to 4 split-stable subfamily cells. Expected uplift: `delete_test_call::python` corr lifts from 0.6948 to ≥ 0.90 on per-cell, contributes to per-cell ship-gate.

4. **Wire `MEJEPA_INSTRUMENT_GRADIENT_LEAK` runtime hook into the training step** (per #620). Even though the trainer is certificate-only today, when real training resumes the hook must fire on every step. Expected uplift: safety/correctness; prevents silent target-encoder contamination.

5. **Replace `panel.data()` flat-buffer aggregations in `heal/distill.rs` and `latent_search.rs`** with slot-preserving per-slot aggregations. Expected uplift: removes cross-embedder noise from heal-distill InfoNCE and candidate-ranking latent. Likely small per-step effect but compounding over training.

## Web-research notes

- JEPA target-encoder collapse is normally prevented by **stop-gradient + EMA + architectural asymmetry** (V-JEPA literature). ME-JEPA's frozen-instrument design gives a stronger guarantee: ZERO trainable parameters on the target side. This protects against target-side collapse but **does NOT protect against input-side collapse** (the e2/e3/e4 issue is input collapse — degenerate input → degenerate output, independent of any training dynamics). [Rethinking JEPA: Compute-Efficient Video SSL with Frozen Teachers](https://arxiv.org/pdf/2509.24317) confirms frozen-teacher variants still require dim-wise variance regularizers (VICReg-style) as safety nets — ME-JEPA has none on the input side.
- Sinusoidal positional encoding gives distinct vectors for distinct positions, but is information-empty for a constant input. This is the standard PE failure mode when timestamp inputs degenerate. [Inside Sinusoidal Position Embeddings](https://learnopencv.com/sinusoidal-position-embeddings/) for canonical reference.

## Open questions

1. **Was the prod corpus generation timestamp-injection deliberate?** The author of `python_forward_cache_fsv.rs:2308` may have intended `chunk.time` to be a semantic time (e.g., taking the file's git-committed-at) but the AST chunker emits it as the chunker run-time. Needs an explicit `type:context` issue to clarify whether E2/E3/E4 are even meaningful for static AST corpora; the answer may be "no, retire them for code corpora and reactivate for live-session corpora only."
2. **Aiwonder verification.** I cannot SSH to `aiwonder` from this WSL host. All evidence here is from the LOCAL legacy corpus at `/mnt/d/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-c8117f15-20260515/`. Pattern is structurally identical to what #666/#670 report from aiwonder paths, but byte-level confirmation requires aiwonder access.
3. **Is the predictor that produced corr=0.7942 actually trained, or is it a random-init checkpoint?** Since the Rust trainer is certificate-only, the predictor weights at ship-gate evaluation time must come from somewhere else. Need to trace the checkpoint promotion path (`promote_trained_predictor_checkpoint` at `eval/regression.rs:45`) to find who actually writes the .safetensors that gets read by `predictor_checkpoint::load_safetensors`. Likely from a manual or out-of-band training run that the Rust crate is unaware of.
4. **Does any consumer of the 491-label registry skip the admissibility overlay?** Audit needed — every downstream that joins on `label` should derive admissibility per row. UB-9 lists the suspect.
5. **Are constellation hashes stable across reruns of the discovery pipeline?** The Mendeleev pair audit (#583) recently closed but the constellation derivation upstream of it relies on label set membership. If the 491-label registry shifts even by one row across reruns (e.g., due to non-deterministic Counter iteration order under `sorted()`), constellation hashes drift. Worth a separate forensic pass.
