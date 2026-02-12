# E5 Causal Embedder — Forensic Findings Report

**Date**: 2026-02-12
**Scope**: Components that appear to work but are broken, degraded, or silently failing
**Method**: Direct source code audit of all E5-touching code paths
**Branch**: casetrack

---

## Executive Summary

The E5 causal embedder subsystem has **3 confirmed bugs** and **2 silent failure modes** that allow the system to appear fully functional while critical components are ineffective or computing incorrect results. The most severe finding is that the **LoRA model silently falls back to a base model** that produces near-uniform scores, making the causal gate effectively a no-op — yet no health check, error, or MCP tool reveals this state.

| # | Severity | Finding | Status |
|---|----------|---------|--------|
| F-1 | CRITICAL | LoRA silent fallback — gate appears to work but is a no-op | Unpatched |
| F-2 | CRITICAL | `search_causes` gate uses wrong E5 direction | Unpatched |
| F-3 | HIGH | `e5_active_vector()` makes default E5 scoring symmetric | By design |
| F-4 | MEDIUM | Misleading doc comment propagates direction confusion | Unpatched |
| F-5 | LOW | `has_trained_weights()` is dead code — no health check uses it | Unpatched |

---

## F-1: LoRA Silent Fallback (CRITICAL)

### What appears to work
The causal gate (`apply_causal_gate`) runs on every search result. MCP tools like `search_causes`, `search_effects`, and `search_graph` all call it. The gate transparency output (`causalGate.e5Score`, `causalGate.action`) is populated in results. Everything looks operational.

### What is actually broken
When LoRA weights are not found on disk, the E5 model silently falls back to its base model (nomic-embed-text-v1.5 without fine-tuning). The base model produces **near-uniform E5 scores of 0.93–0.98 for all text**, regardless of causal content. This means:

- **Every result exceeds `CAUSAL_THRESHOLD` (0.30)** → gate always boosts
- **True Negative Rate = 0%** — non-causal content is never demoted
- The gate is effectively `similarity * 1.05` for all results — a uniform inflation
- Users see `causalGate.action: "boost"` on every result, believing the system is working

### Evidence

**`model.rs:379-390`** — The fallback path:
```rust
if !lora_path.exists() && !projection_path.exists() {
    tracing::info!(  // <-- info!, not warn! or error!
        "No trained weights found in {}, using base model",
        checkpoint_dir.display()
    );
    return Ok(false);  // <-- Returns success, not error
}
```

**`model.rs:338-343`** — Base model path when `trained` is `None`:
```rust
pub fn embed_dual_guided(&self, ...) -> EmbeddingResult<(Vec<f32>, Vec<f32>)> {
    // ...
    if let Some(ref t) = trained {
        // LoRA path: produces direction-sensitive scores
        self.gpu_forward_dual_trained(...)
    } else {
        // Base path: produces near-uniform scores (0.93-0.98)
        self.gpu_forward_dual(text, role, template)
    }
}
```

**`model.rs:452-458`** — Health check exists but is never called:
```rust
pub fn has_trained_weights(&self) -> bool {
    matches!(&*state, ModelState::Loaded { trained: Some(_), .. })
}
```
This method has **zero callers** outside the struct itself. No MCP handler, no health endpoint, no gate function checks whether LoRA is actually loaded.

### Impact
- **All causal gate decisions are meaningless** when LoRA is not loaded
- **No user-visible signal** distinguishes "gate with LoRA" from "gate without LoRA"
- **Benchmark Phase 5 (Gate) still PASSES** because the gate thresholds were calibrated against trained model scores — if running against base model, Phase 5 would fail with TNR=0%
- The system silently degrades from "causal-aware search" to "slightly inflated search scores"

### Recommended Fix
1. Promote the fallback log from `info!` to `warn!` and add a structured metric
2. Wire `has_trained_weights()` into the MCP `get_memetic_status` response
3. Add a `causal_model_health` field to search result metadata: `{ lora_loaded: bool, base_model_fallback: bool }`
4. Consider returning an error or `causalGate.warning: "base_model_fallback"` when LoRA is missing

---

## F-2: `search_causes` Gate Uses Wrong E5 Direction (CRITICAL)

### What appears to work
`search_causes` finds candidate causes for a given effect, ranks them with `rank_causes_by_abduction`, then applies the causal gate. The gate modifies scores. Results are returned. Everything looks functional.

### What is actually broken
The causal gate in `search_causes` computes E5 similarity with **reversed direction vectors**.

**`causal_tools.rs:135`** — `fp` is the query fingerprint, which represents the **effect** (user says "find causes for this effect"):
```rust
let fp = query_embedding.as_ref().unwrap();  // fp = effect fingerprint
```

**`causal_tools.rs:185-186`** — Gate uses `query_is_cause=true`, but fp IS the effect:
```rust
let e5_sim = compute_e5_asymmetric_fingerprint_similarity(
    fp, stored_fp, true,  // BUG: fp is the effect, not the cause
);
```

**Reference implementation in `chain.rs:346-348`** — Uses the **correct** direction:
```rust
// rank_causes_by_abduction correctly uses false
let e5_sim = compute_e5_asymmetric_fingerprint_similarity(
    effect_fingerprint, cause_fp, false  // CORRECT: effect is not the cause
);
```

### What `query_is_cause` controls (`asymmetric.rs:496-506`):
```rust
let (query_vec, doc_vec) = if query_is_cause {
    // query.cause_vector vs doc.effect_vector
    (query.get_e5_as_cause(), doc.get_e5_as_effect())
} else {
    // query.effect_vector vs doc.cause_vector
    (query.get_e5_as_effect(), doc.get_e5_as_cause())
};
```

With `true` (current bug): Compares **effect's cause-vector** vs **cause's effect-vector** — semantically inverted.
With `false` (correct): Compares **effect's effect-vector** vs **cause's cause-vector** — correct asymmetric pairing.

### Note: `search_effects` is CORRECT
`causal_tools.rs:543-544` uses `true` for `search_effects`, where `fp` IS the cause. This is correct.

### Impact
- The gate applies E5 similarity computed from the wrong vector pairing
- With trained LoRA, this would produce incorrect gate decisions (boost/demote reversed)
- Currently **masked by F-1**: base model produces near-uniform scores regardless of direction
- The abduction ranking itself (`rank_causes_by_abduction`) is **not affected** — it correctly uses `false`
- Only the gate overlay is wrong, so ranking order from abduction is preserved

### Recommended Fix
Change `causal_tools.rs:186` from `true` to `false`:
```rust
let e5_sim = compute_e5_asymmetric_fingerprint_similarity(
    fp, stored_fp, false,  // fp is the effect, not the cause
);
```

---

## F-3: `e5_active_vector()` Makes Default E5 Scoring Symmetric (HIGH)

### What appears to work
In multi-space search (the default strategy), E5 contributes a score alongside 5 other embedders via weighted RRF fusion. The E5 score is computed, weighted, and fused. Results appear to incorporate causal signal.

### What is actually broken
When `CausalDirection::Unknown` (the default for most queries), the E5 comparison is **symmetric**, defeating E5's asymmetric design.

**`fingerprint.rs:302-308`** — Always returns cause vector:
```rust
pub fn e5_active_vector(&self) -> &[f32] {
    if !self.e5_causal_as_cause.is_empty() {
        &self.e5_causal_as_cause  // Always returns cause vector
    } else {
        &self.e5_causal  // Legacy fallback, also symmetric
    }
}
```

**`search.rs:393`** — Default embedder score computation:
```rust
// Both query and stored return their cause vectors
cosine_similarity(query.e5_active_vector(), stored.e5_active_vector())
// Result: cause-vs-cause = symmetric comparison
```

The direction-aware path **does exist** (`search.rs:424-429`, `compute_similarity_for_space_with_direction_sync`), but it's only activated when `causal_direction` is explicitly set — which only happens for detected causal queries via the MCP tools that perform intent detection.

### Impact
- For queries where causal direction is not detected (most general searches), E5 contributes a **symmetric score** that measures "how structurally similar are these cause vectors" rather than "does a cause-effect relationship exist"
- This is technically **by design** — the system correctly upgrades to asymmetric mode when direction is detected
- But it means E5's default contribution to multi-space search is essentially a second E1 (topical similarity), not causal signal
- The E5 weight (typically 0.03–0.154 depending on profile) is partially wasted in default mode

### Recommended Fix
This is a design trade-off, not necessarily a bug. Options:
1. Accept it — symmetric E5 as fallback for unknown direction is reasonable
2. Add `e5_effect_vector()` method and use max(cause_sim, effect_sim) for unknown direction
3. Suppress E5 weight when direction is unknown (degenerate suppression already handles this partially)

---

## F-4: Misleading Doc Comment Propagates Direction Confusion (MEDIUM)

### The Problem
The doc comment on `compute_e5_asymmetric_fingerprint_similarity` contradicts its implementation:

**`asymmetric.rs:486`**:
```rust
/// For "why did X happen?" queries, query is looking for causes (query_is_cause=true)
```

"Why did X happen?" means: the query represents an **effect** (X happened), seeking causes.
The correct parameter should be `query_is_cause=false` — the query is NOT the cause.

The implementation at `asymmetric.rs:496-497` is unambiguous:
```rust
if query_is_cause {
    // Query represents a potential cause, looking for effects
```

This doc comment is likely the **root cause of F-2** — a developer reading the doc comment would use `true` for cause-seeking queries, but the implementation expects `true` only when the query **IS** a cause.

### Impact
- Directly caused the `search_causes` bug (F-2)
- Future developers reading the doc will make the same mistake
- The parameter name `query_is_cause` is clear but the doc comment contradicts it

### Recommended Fix
Correct the doc comment:
```rust
/// For "why did X happen?" queries, query represents the effect (query_is_cause=false)
/// For "what happens if X?" queries, query represents the cause (query_is_cause=true)
```

---

## F-5: `has_trained_weights()` is Dead Code (LOW)

### The Problem
`model.rs:452-458` provides a method to check whether LoRA weights are loaded, but **no code calls it**:

```rust
pub fn has_trained_weights(&self) -> bool {
    matches!(&*state, ModelState::Loaded { trained: Some(_), .. })
}
```

A grep for `has_trained_weights` across the entire codebase returns only the definition itself. No MCP handler, no health endpoint, no gate function, no status tool queries this method.

### Impact
- The system has the infrastructure to detect F-1 but doesn't use it
- Runtime LoRA state is completely opaque to operators and users

### Recommended Fix
Wire into `get_memetic_status` MCP tool and/or `system_health` endpoint.

---

## Components Confirmed Working Correctly

For completeness, these E5-related components were audited and found to be **correctly implemented**:

| Component | File | Evidence |
|-----------|------|----------|
| `rank_causes_by_abduction` | chain.rs:348 | Uses `query_is_cause=false` — correct |
| `rank_effects_by_prediction` | chain.rs:440 | Uses `query_is_cause=true` — correct |
| `memory_tools.rs` gate | memory_tools.rs:1718 | Derives from `query_direction` — correct |
| Merge safety | merge.rs:504-546 | Rejects opposing directions — correct |
| `get_causal_chain` | causal_tools.rs:873 | Uses `is_forward` flag — correct |
| HNSW direction routing | search.rs:552-556 | Routes to E5CausalCause/Effect — correct |
| `infer_direction_from_fingerprint` | asymmetric.rs:119-140 | 10% L2 norm threshold — correct |
| `suppress_degenerate_weights` | search.rs:1171-1212 | Variance < 0.001 → 0.25x — correct |
| `cosine_similarity_f32` | asymmetric.rs:923 | Handles empty vectors → 0.0 — correct |
| Intent detection | asymmetric.rs:585-906 | 130+ patterns with negation — correct |
| Causal gate transparency | memory_tools.rs:1394-1398 | Shows e5Score/action/delta — correct |
| `search_effects` gate | causal_tools.rs:543-544 | Uses `true` (fp IS cause) — correct |
| Direction-aware HNSW (pipeline) | search.rs:758-762 | Routes correctly — correct |
| Direction-aware HNSW (filtered) | search.rs:274-288 | Routes correctly — correct |

---

## Interaction Between Findings

The findings are not independent — they interact in ways that mask each other:

```
F-1 (LoRA fallback) ──masks──> F-2 (wrong direction in search_causes gate)
                      │
                      └──masks──> F-3 (symmetric e5_active_vector)
```

- **F-1 masks F-2**: When base model produces uniform scores (0.93-0.98), the direction of vector comparison doesn't matter — all results get the same score anyway. If F-1 is fixed (LoRA loaded), F-2 becomes observable.
- **F-1 masks F-3**: Symmetric vs asymmetric comparison doesn't matter when all scores are near-identical. If F-1 is fixed, F-3's impact on default search becomes measurable.
- **F-4 caused F-2**: The misleading doc comment is the likely source of the direction inversion.

### Recommended Fix Order
1. **F-4 first** (doc comment) — prevents future recurrence
2. **F-2 second** (search_causes direction) — correctness fix
3. **F-5 third** (wire health check) — enables detection of F-1
4. **F-1 fourth** (LoRA fallback severity) — needs operational decision on error vs warning
5. **F-3 last** (symmetric default) — design decision, lowest urgency

---

## Test Coverage Gaps

The following test gaps relate to the findings above:

1. **No test verifies LoRA fallback produces different scores than trained model** — tests either mock E5 or use artificial scores
2. **No test verifies `search_causes` gate uses the correct direction** — the integration test for search_causes doesn't check which E5 vectors were compared
3. **No test verifies `e5_active_vector()` returns the correct vector for each direction** — only basic non-empty checks exist
4. **All gate tests use artificial scores (0.4, 0.2, etc.)** — no test uses realistic E5 score distributions from either base or trained model
5. **No end-to-end test confirms the gate makes a different decision with vs without LoRA** — this is the most important missing test

---

## Appendix: Code Paths Audited

| Path | Files Examined | Verdict |
|------|---------------|---------|
| Embedding generation | model.rs (full) | F-1 found |
| Asymmetric similarity | asymmetric.rs:480-560 | F-4 found |
| Causal gate | asymmetric.rs:60-102 | Correct |
| search_causes MCP | causal_tools.rs:60-260 | F-2 found |
| search_effects MCP | causal_tools.rs:400-620 | Correct |
| search_graph MCP | memory_tools.rs:1710-1730 | Correct |
| Multi-space search | search.rs:380-430 | F-3 documented |
| HNSW routing | search.rs:270-290, 550-560, 755-765 | Correct |
| Merge safety | merge.rs:495-565 | Correct |
| Chain reasoning | chain.rs:339-462 | Correct |
| Causal chain MCP | causal_tools.rs:730-910 | Correct |
| Fingerprint accessors | fingerprint.rs:300-345 | F-3 documented |
| Direction inference | asymmetric.rs:104-140 | Correct |
| Weight suppression | search.rs:1170-1215 | Correct |
| RRF fusion | search.rs (multiple) | Correct |
