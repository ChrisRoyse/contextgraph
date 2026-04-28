# EXP-2 Sampling and Corpus Construction

**What inputs the MI estimators see.** The experiment is only as honest as the distribution it samples.

---

## 1 · Production distribution $\mathcal{P}$

$\mathcal{P}$ is the current content of `CF_FINGERPRINTS` as of the freeze commit. At freeze time we record:

- `M` — total count of live (non-soft-deleted) fingerprints
- `M_strata` — count per stratum (code / prose / mixed / short)
- Content-length distribution (percentiles 10, 25, 50, 75, 90, 99)
- Session distribution (number of distinct session IDs)
- Temporal distribution (min / max / median `fingerprint.created_at`)

These are reported as-is in the final write-up. If $M < 5000$, a **caveat is added** that the corpus is small and effective-N estimates have wider CIs.

---

## 2 · Stratification

Four strata, target 25 % of the sample each.

### 2.1 Stratum definitions

| Stratum | Definition | Rationale |
|---------|------------|-----------|
| **Code** | `source_metadata.content_type == "code"` OR content matches `(fn|def|class|function|impl|const|let|var) ` anywhere in first 200 chars | E7 was pre-trained for this; exercises code-specific signal |
| **Prose** | content length ≥ 200 chars AND not code AND not mixed | General-purpose natural language; E1 / E10 dominant |
| **Mixed** | content length ≥ 200 chars AND contains both a code-identifier pattern AND prose-like structure (≥ 10 non-code words) | Exercises E7 / E1 / E10 interaction |
| **Short** | content length < 200 chars | Session notes, chat turns, labels; exercises E4 / E12 sparsity |

Classification is deterministic and one-pass over the corpus; the classifier code is frozen with the preregistration.

### 2.2 Balance constraints

- Each stratum contributes 25 % of the final $n$, ± 2 % tolerance.
- If a stratum is undersized ($< n/4$), the full stratum is taken and the residual is redistributed **proportionally** to the remaining three strata, up to each stratum's total size.
- If total $M < n$, the full corpus is used and $n$ is reduced. This is reported.

### 2.3 Sampling seed

Frozen: `stratify_seed = 42`. Re-running the corpus build with this seed reproduces the exact memory IDs.

### 2.4 Temporal diversity

Within each stratum, no more than 10 % of the sampled IDs may share a 1-hour window of `fingerprint.created_at`. This is enforced by rejection sampling: if a candidate would violate the 10 % rule, it is skipped and the next candidate is tried.

Rationale: E2 / E3 / E4 are deterministic functions of timestamp. Clustered timestamps inflate temporal-embedder correlation with each other and deflate it with everything else.

### 2.5 Session diversity

Within each stratum, no single session may contribute more than 15 % of the stratum sample. Rationale: session-clustered content inflates E4 positional correlation and content self-similarity.

---

## 3 · Sample sizes

Three sizes are run end-to-end. All three are reported.

| Size | $n$ | Expected wall-clock | Purpose |
|------|-----|---------------------|---------|
| Pilot | 100 | ~1 GPU-hour | Sanity check on pipeline; estimator convergence curves |
| Primary | 500 | ~8 GPU-hours | Main analysis; detects most effect sizes |
| High-resolution | 1000 | ~24 GPU-hours | Headline number; tight CIs |

**The headline $N_\text{eff}$ is reported from $n = 1000$.** $n = 100$ and $n = 500$ are used to:
1. Confirm estimator convergence (MINE estimate as a function of $n$ should plateau).
2. Track bootstrap CI shrinkage with $n$.
3. Detect sample-size instability (if $n=100$ and $n=1000$ disagree by > 3 bits on > 10 % of pairs, the estimator is unstable and the protocol is amended).

---

## 4 · Corpus splits

Each sample of size $n$ is split into three disjoint subsets:

| Split | Fraction | Use |
|-------|----------|-----|
| **PCA fit** | 20 % | Compute PCA projections (256D for MINE / InfoNCE, 16D for KSG) once, freeze |
| **Critic train** | 50 % | MINE / InfoNCE gradient steps |
| **Held-out test** | 30 % | Report MI on this split only; critic / PCA both frozen when touched |

Split seed: `split_seed = 137`. Frozen.

**The held-out test split is touched exactly once per pair per seed.** No "peeking" — critic selection is by training-loss convergence, not by test-split MI.

---

## 5 · Corpus build procedure (step-by-step)

```
1. Read freeze-commit git SHA
2. Connect to production daemon at 127.0.0.1:3100
3. Call list_memories() -> get all M UUIDs
4. For each UUID: load SourceMetadata + fingerprint summary
5. Classify into strata (§2.1)
6. Record per-stratum counts -> commit to report
7. For n in [100, 500, 1000]:
    a. Shuffle each stratum with stratify_seed
    b. Enforce temporal-diversity rejection filter (§2.4)
    c. Enforce session-diversity filter (§2.5)
    d. Take ceil(n/4) from each stratum; balance residuals (§2.2)
    e. For each sampled UUID: fetch SemanticFingerprint
    f. Construct 13-vector panel per [02_panel_specification.md §3]
    g. Apply JL sketch to E6 / E13 with sketch_seed=2026
    h. Save to exp2_corpus_n<n>_<date>.npz
    i. Hash the file (sha256); commit hash to preregistration appendix
8. For each n:
    a. Load exp2_corpus_n<n>_<date>.npz
    b. Apply split_seed=137 -> PCA / train / test partitions
    c. Fit PCA on the PCA partition only; save PCA matrices
    d. Serialize PCA matrices to exp2_pca_n<n>_<date>.npz
```

The build is **deterministic** given frozen seeds. Rebuilding from scratch on a clean checkout produces bit-identical corpora.

---

## 6 · Alternative / sensitivity corpora (reported but not headline)

Three alternative corpora are built in parallel and reported as **sensitivity analyses**. They do not change the decision rules; they stress-test the headline.

### 6.1 Wikipedia-5k

Random 5 000 Wikipedia intro paragraphs (English). Sampled from the standard Wikipedia dump of 2024-12 (committed in the corpus build script).

Purpose: provide an external-distribution $N_\text{eff}$ estimate. Useful for the Perez memo: "we also checked against Wikipedia 5k and got $N_\text{eff} \in [\dots]$."

### 6.2 Code-only-5k

Random 5 000 functions from The Stack v2 (MIT-licensed subset, sampled by language in proportion to the full Stack distribution). Purpose: test E7 / E1 correlation on in-distribution code.

### 6.3 Shakespeare-2k

The Shakespeare Sonnets + plays corpus already used by the Shakespeare LoRA subsystem (see SITREP 2026-04-21). Purpose: cross-check on a narrow stylistic distribution where we already have intuition.

**All three sensitivity corpora are pre-registered here.** Adding a new one post-run is a protocol amendment.

---

## 7 · Corpus artifacts at freeze

On freeze day, the following files are committed or noted in `docs/exp2/.corpus_manifest`:

```
exp2_corpus_n100_2026-04-30.npz    sha256:<hash>
exp2_corpus_n500_2026-04-30.npz    sha256:<hash>
exp2_corpus_n1000_2026-04-30.npz   sha256:<hash>
exp2_pca_n100_2026-04-30.npz       sha256:<hash>
exp2_pca_n500_2026-04-30.npz       sha256:<hash>
exp2_pca_n1000_2026-04-30.npz      sha256:<hash>
exp2_wikipedia5k_2026-04-30.npz    sha256:<hash>
exp2_code5k_2026-04-30.npz         sha256:<hash>
exp2_shakespeare2k_2026-04-30.npz  sha256:<hash>
exp2_sketch_matrices_2026-04-30.npz sha256:<hash>
```

The `.npz` bundles are large (the $n=1000$ production bundle is ~500 MB). They are **not** committed to git; they are stored under `/home/cabdru/contextgraph/exp2_artifacts/` with the hashes committed to the manifest file. The manifest is git-tracked.

For publication, a subset (typically $n=500$ with memory IDs anonymized) is released alongside the final write-up as the reproducibility artifact.

---

## 8 · Privacy and disclosure

The production corpus may contain sensitive user content. Before any corpus artifact leaves the local machine:

1. Memory IDs are replaced with a one-way hash keyed on a freeze-time secret.
2. Content text is **never** included in the published corpus; only the embedded tensors + stratum labels + token counts + hash IDs.
3. The raw content is retained locally for reproducibility audits.

This follows the same privacy model as the Constellation / Contrastive-pair public artifacts.
