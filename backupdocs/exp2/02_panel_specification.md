# EXP-2 Panel Specification

**The exact 13-embedder panel $\Phi = \{\phi_1, \dots, \phi_{13}\}$ that enters EXP-2.** Every row is the concrete tensor the MI estimators see, after all preprocessing.

Cross-reference: `docs2/contextgraphspecs/05_embedding_subsystem.md` (authoritative on architectures and dimensions).

---

## 1 · Panel table

Ordering follows `ModelId` repr-u8 slot. Each row specifies the tensor presented to the estimator.

| Slot | `ModelId` | Family | Native dim | Stored dim | Asymmetric | EXP-2 input tensor | Dim entering PCA | Notes |
|------|-----------|--------|------------|------------|-----------|--------------------|------------------|-------|
| E1 | `Semantic` | e5-large-v2 (BERT-large) | 1024 | 1024 | No | `fp.e1_semantic` | 1024 | Backbone shared with E8 |
| E2 | `TemporalRecent` | sinusoidal PE | 512 | 512 | No | `fp.e2_temporal_recent` | 512 | Deterministic function of Unix timestamp |
| E3 | `TemporalPeriodic` | Fourier basis | 512 | 512 | No | `fp.e3_temporal_periodic` | 512 | Deterministic |
| E4 | `TemporalPositional` | sinusoidal PE of session seq | 512 | 512 | No | `fp.e4_temporal_positional` | 512 | Deterministic |
| E5 | `Causal` | nomic-embed-v1.5 + LoRA + proj | 768 × 2 | 768 × 2 | Yes (cause, effect) | `concat(fp.e5_causal_as_cause, fp.e5_causal_as_effect)` then L2-renorm | 1536 | Concatenated; dual asymmetric |
| E6 | `Sparse` | SPLADE-cocondenser | 30522 | 1536 top-k | No | dense-ified 30522-dim value vector (zeros where inactive) | 30522 | Full dense form for EXP-2; top-k pruning OFF |
| E7 | `Code` | Qodo-Embed-1-1.5B (Qwen2) | 1536 | 1536 | No | `fp.e7_code` | 1536 | |
| E8 | `Graph` | e5-large-v2 + W_source/W_target | 1024 × 2 | 1024 × 2 | Yes (source, target) | `concat(fp.e8_graph_as_source, fp.e8_graph_as_target)` then L2-renorm | 2048 | **Shares backbone with E1** — pair with E1 is the known-high-MI anchor |
| E9 | `Hdc` | Hyperdimensional (trigram) | 10000 (binary) | 1024 f32 proj | No | `fp.e9_hdc` | 1024 | HDC projection to f32 |
| E10 | `Contextual` | e5-base-v2 + prefix asymmetry | 768 × 2 | 768 × 2 | Yes (paraphrase, context) | `concat(fp.e10_multimodal_paraphrase, fp.e10_multimodal_as_context)` then L2-renorm | 1536 | Shares e5 family with E1 / E8 but smaller base |
| E11 | `Kepler` | RoBERTa + TransE | 768 | 768 | No | `fp.e11_entity` | 768 | E11_ENTITY_ENABLED=false at runtime; still loaded and included in EXP-2 |
| E12 | `LateInteraction` | ColBERTv2 (BERT-base) | 128/token | variable | No (token-level) | mean-pool over stored token vectors, L2-renorm | 128 | Pooling loss reported as caveat |
| E13 | `Splade` | SPLADE-v3 | 30522 | 1536 top-k | No | dense-ified 30522-dim value vector | 30522 | Same dense treatment as E6 |

**Panel size: $N = 13$.** $\binom{N}{2} = 78$ pairs.

---

## 2 · Prior-expected redundancy map

Used only to interpret results, **not** to set thresholds. Column "Expected $I$" is an informal guess based on backbone sharing and family; actual MI comes from EXP-2.

| Pair | Backbone relationship | Informal prior on $I$ (bits) | Why |
|------|-----------------------|------------------------------|-----|
| E1–E8 | Same e5-large-v2 weights, differ only in learned linear projection | **High** (≥ 5) | E8 is a near-linear reparameterization of E1 — this is the structural ceiling case |
| E1–E10 | Same e5 family, different base (large vs base) | Moderate-high (2–5) | Same pretraining objective + data, different scale |
| E8–E10 | e5 family | Moderate-high (2–5) | Transitive via shared family |
| E1–E5 | e5-large vs nomic-embed (fine-tuned) | Moderate (1–3) | Both retrieval-trained but different pretraining corpus and objective |
| E1–E7 | e5-large vs Qwen2 | Moderate (1–3) | Different family, both learned semantics |
| E1–E11 | e5-large vs RoBERTa-KEPLER | Moderate (1–3) | Different pretraining, but PRH suggests convergence |
| E5–E10 | nomic vs e5-base | Moderate (1–3) | |
| E5–E7 | nomic vs Qwen2 | Low-moderate (0.5–2) | Different scales and domains |
| E7–E11 | Qwen2 vs RoBERTa | Low-moderate (0.5–2) | |
| E1/E8–E2/E3/E4 | Learned vs deterministic time | **Very low** (≤ 0.1) | Temporal embedders are pure functions of timestamp; content-free |
| E2–E3, E2–E4, E3–E4 | Same timestamp, different transforms | **High** (5+) | All three are invertible-ish functions of the same timestamp |
| E6–E13 | Both SPLADE variants over 30k vocab | **High** (3–7) | Same tokenizer, very similar training objective; "twin sparse" pair |
| E9 vs anything learned | HDC vs neural | Low (0–1) | HDC is non-learned; orthogonal paradigm |
| E12 vs anything | ColBERT pooled vs others | Low-moderate (0.5–2) | Pooling destroys token-level signal; E12 becomes a coarse BERT-base semantic |

**Predicted $N_\text{eff}$ from priors alone: ~18–35.** Likely in the Moderate band.

---

## 3 · Vector preprocessing — exact operations

For every stored memory $m$ and every embedder $\phi_j$, the EXP-2 input is constructed as follows:

```
vec = fetch_from_fingerprint(m.semantic_fingerprint, j)     # raw stored tensor
if j in {E5, E8, E10}:
    vec = concat(vec.first_half, vec.second_half)            # concatenate dual
    vec = vec / ||vec||_2                                    # L2 renorm
elif j in {E6, E13}:
    dense = zeros(30522)
    for (idx, val) in zip(vec.indices, vec.values):
        dense[idx] = val
    vec = dense                                              # full dense, no pruning
elif j == E12:
    # stored as Vec<Vec<f32>> of shape [num_tokens, 128]
    vec = mean(vec, axis=0)                                  # pool
    vec = vec / ||vec||_2                                    # L2 renorm
else:
    vec = vec                                                # already L2-normalized at storage
return vec.astype(np.float32)
```

**No quantized vectors are used.** EXP-2 reads from the pre-quantization full-precision fingerprint stored on the write path; if production storage quantizes before write, the corpus preparation step dequantizes (PQ-8, Float8, Binary) back to f32 using the stored codebooks. Dequantization error is reported as a caveat.

**No NaN / Inf permitted.** The corpus build step rejects any memory whose fingerprint contains NaN / Inf in any slot. Rejected memories are counted and disclosed.

---

## 4 · Failure modes specific to the panel

### 4.1 E6 / E13 dimensionality blowup

Dense-ifying 30522-dim sparse vectors with ~1500 active values wastes 95 % of the dimension budget in PCA. Mitigation:
- Before PCA, the 30522 dense vectors are projected to a 2048-dim **random Gaussian sketch** $G \in \mathbb{R}^{2048 \times 30522}$ (seed `sketch_seed = 2026`, frozen), $L = G \cdot \text{sqrt}(\text{1/2048})$, then L2-renormed. PCA runs on the 2048-dim sketch.
- Johnson-Lindenstrauss guarantees $\ge 0.95$ preservation of pairwise distances at this sketch size for 1000 points.
- The sketch matrix is serialized with the preregistration.

### 4.2 E12 pooling loss

Mean-pool over token vectors loses the per-token structure that is the entire point of ColBERT. MI estimates on E12 are biased downward (pooled-E12 carries strictly less information than token-level E12). Reported as a **structural caveat** in the final write-up. Follow-up experiment ([14_followup_experiments.md](14_followup_experiments.md) §3) considers a MaxSim-based pseudo-MI for E12.

### 4.3 E2 / E3 / E4 near-determinism

Temporal embedders are deterministic functions of timestamp. If the corpus contains many memories with identical or near-identical timestamps, E2/E3/E4 degenerate to near-constant vectors and MI with anything else collapses to 0 (because $H(\phi_{E2}(X))$ collapses). Mitigation:
- Stratified sampling requires temporal diversity: no more than 10 % of the corpus from any 1-hour window. Enforced at corpus-build time.
- If post-filter corpus violates this, we subsample the over-represented window.

### 4.4 E5 / E8 / E10 dual-vector correlation

Within-embedder the two halves are highly correlated (same base model, different projection). Concatenation is the correct handling: MI between two dual-embedders compares the **joint** cause+effect space, which is the structural quantity of interest.

### 4.5 E11 disabled in production

`E11_ENTITY_ENABLED = false` means E11 is loaded into VRAM but never contributes weight in retrieval scoring. For EXP-2 this is irrelevant — we measure the substrate's information content, not its retrieval activation. Including E11 is the right call. This is explicitly disclosed in the report.

---

## 5 · Validation before freeze

Before the 2026-04-29 freeze, the following dry-run must pass:

1. Fetch 10 random memories.
2. For each, construct the 13-vector panel per §3.
3. Assert:
   - Every vector is finite (`np.all(np.isfinite(v))`).
   - Every vector is L2-normalized within $10^{-5}$ (except E6/E13 pre-sketch).
   - Every vector has the expected `dim_entering_pca` from §1.
4. Save the 10 × 13 tensor bundle to `/tmp/exp2_panel_dryrun.npz` and visual-inspect via `np.load` + `print(tensor.shape)`.
5. Commit `docs/exp2/.panel_dryrun_ok` as a sentinel file.

If any assertion fails, the corpus-build code is fixed and the dry-run is rerun. The fix is dated.

---

## 6 · Full tensor schema

Every panel tensor serialized for EXP-2 is saved as:

```
exp2_corpus_<date>.npz
├── ids: Uuid[n]                    # memory UUIDs
├── phi_E1: f32[n, 1024]
├── phi_E2: f32[n, 512]
├── phi_E3: f32[n, 512]
├── phi_E4: f32[n, 512]
├── phi_E5: f32[n, 1536]            # concatenated cause+effect
├── phi_E6: f32[n, 2048]            # after JL sketch
├── phi_E7: f32[n, 1536]
├── phi_E8: f32[n, 2048]            # concatenated source+target
├── phi_E9: f32[n, 1024]
├── phi_E10: f32[n, 1536]           # concatenated paraphrase+context
├── phi_E11: f32[n, 768]
├── phi_E12: f32[n, 128]            # mean-pooled
├── phi_E13: f32[n, 2048]           # after JL sketch
├── strata: u8[n]                   # 0=code, 1=prose, 2=mixed, 3=short
└── sketch_matrices: dict           # seeds + serialized G matrices for E6/E13
```

PCA projections (to 256 / 16 dims) are computed **after** load, from the frozen 20 % PCA split, and serialized to `exp2_pca_<date>.npz` alongside.
