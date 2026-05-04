# 30 - Mutual Information Embedder Audit

## Goal

Empirically measure how tight the **91 signals per input** upper bound
of the DDA counting identity actually is on real corpora. PAPER.md
§3.1 explicitly flags this as an open problem ("tightness pending a
pairwise mutual-information audit"). This doc specifies the protocol
and the deliverable that closes the open problem and lets the paper
make a sharper claim than the constructive upper bound.

## Why This Matters

The DDA counting identity $|D'| = n \cdot (N + \binom{N}{2})$ is a
**structural upper bound**, not an information-theoretic lower bound.
If two embedders are linear reparameterizations of each other, their
pair contributes near-zero new information. Without measuring this, we
can only claim "up to 91 signals per input." With a measurement, we
can claim either:

- "**measured** k signals per input on our corpus, where k ∈ [k_low,
  k_high]" — a sharp empirical anchor, or
- "**effective signal count** k_eff via pairwise MI estimation,
  comparable to the 91 upper bound" — a dimensionality-reduction
  framing.

Either way the paper moves from constructive to measured.

## Protocol

### Step 1 — Corpus Selection

Three corpora to triangulate:

| Corpus | Size | Modality | Reason |
| --- | --- | --- | --- |
| O*NET 30.2 occupational rows | ~1,016 | text + categorical | already in pipeline; ME-JEPA-aligned |
| C4 sample (50K rows) | 50,000 | text only | external; broadly cited |
| Common Crawl Code (Qodo eval set) | ~10K | code | exercises E7 specifically |

Each corpus is projected through the production 13-embedder panel
(E1–E13) and the resulting per-embedder vectors are persisted in the
DDA bundle (doc 29, `dda/per_embedder_vectors.parquet`).

### Step 2 — Per-Embedder Marginal Entropy

For each embedder $\phi_m$, estimate the marginal entropy
$H(\phi_m(X))$ on the corpus using the **k-NN entropy estimator**
(Kozachenko-Leonenko) with $k=4$ in the L2-normalized vector space.

Recorded per (corpus, embedder) row in `mi_audit.json`:

```json
{
  "corpus": "onet_30_2",
  "embedder": "e1",
  "n": 1016,
  "dim": 1024,
  "marginal_entropy_nats": 4.27,
  "estimator": "kozachenko_leonenko_k4"
}
```

### Step 3 — Pairwise Mutual Information

For each pair $(j, k)$ with $j < k$ ($\binom{13}{2} = 78$ pairs),
estimate $I(\phi_j(X); \phi_k(X))$ using the **KSG estimator**
(Kraskov-Stögbauer-Grassberger) with $k=4$. Run two ways:

1. Direct on the full vector pair.
2. After PCA reduction of each vector to $d=32$ (cheaper, comparable).

Cross-checked against an MI-estimating neural network (MINE) for at
least 5 high-dimension pairs to validate KSG.

Persisted as `dda/pairwise_mi.parquet`:

| pair_j | pair_k | n | mi_nats_ksg_k4 | mi_nats_ksg_k4_pca32 | mi_nats_mine | redundancy_ratio |

`redundancy_ratio = mi / min(H_j, H_k)`. Values near 0 = independent;
near 1 = redundant.

### Step 4 — Effective Signal Count

Per corpus, compute:

$$
k_{\text{eff}}(D) = N + \sum_{j < k} (1 - \text{redundancy}_{jk})
$$

This is the **redundancy-discounted** signal count. Reported alongside
the constructive 91 upper bound. For the Context Graph production panel
this should give one number per corpus; the paper then reports
$k_{\text{eff}} \in [k_{\min}, k_{\max}]$ across corpora.

Hypothesis to test: $k_{\text{eff}}$ is large (e.g. > 50) and varies
across corpora because embedder independence varies with content type.

### Step 5 — Per-Embedder Contribution To Downstream Tasks

To corroborate the MI numbers with a behavioral test, run a paired
ablation: for each embedder, train a small classifier head on the
career-taxonomy `paper_med_5k` task using:

- (a) all 13 embedders,
- (b) all 13 embedders **except** the one being tested.

The drop in val accuracy is the embedder's marginal contribution. If
embedder X has high marginal MI with all others (high redundancy), the
ablation should show small drop. Pearson correlation between
`(1 − redundancy)` and `marginal contribution` is the validation
metric for the MI audit.

## CLI

```bash
target/debug/context-graph dynamicjepa mi-audit \
  --bundle-root tmp/dda_audit_v1 \
  --corpus-name onet_30_2 \
  --embedders all \
  --estimator ksg_k4 \
  --pca-dim 32 \
  --json
```

```bash
target/debug/context-graph dynamicjepa mi-audit-aggregate \
  --bundle-roots tmp/dda_audit_v1,tmp/dda_audit_v2,tmp/dda_audit_v3 \
  --output tmp/dda_mi_aggregate \
  --json
```

The aggregate emits paper-table CSVs for the appendix:

- `paper_tables/table_dda_mi_per_corpus.csv`
- `paper_tables/table_dda_mi_per_pair.csv`
- `paper_tables/table_dda_keff.csv`

## Bundle Integration

`mi_audit.json` lives at `dda/mi_audit.json` (doc 29). It contains:

```json
{
  "schema_version": "1.0",
  "corpus": "onet_30_2",
  "n_inputs": 1016,
  "embedder_panel_version_hash": "...",
  "constructive_upper_bound": 91,
  "k_eff_redundancy_discounted": 67.4,
  "k_eff_method": "1 - redundancy_ratio_sum",
  "per_pair_mi_path": "dda/pairwise_mi.parquet",
  "marginal_entropies_path": "dda/marginal_entropies.parquet",
  "validation_against_ablation": {
    "pearson_r": 0.71,
    "p_value": 1.4e-3
  }
}
```

The `bundle_root_hash` (doc 29) covers `mi_audit.json`, so a reviewer
recomputes the audit deterministically from the persisted vectors.

## Paper Wording Update

PAPER.md §3.1 currently reads:

> identity (1) is a **structural upper bound** ... we therefore state
> the headline "**up to** $n \cdot (N + \binom{N}{2})$ signals" ...
> formal mutual-information measurement of redundancy is named future
> work.

After this audit:

> empirical measurement on three corpora yields $k_{\text{eff}} =
> [k_{\min}, k_{\max}]$ signals per input, with the highest density on
> code corpora and the lowest on narrowly-domained taxonomies.
> Pearson correlation between redundancy-discounted MI and downstream
> embedder ablation accuracy is $r = 0.71$ ($p < 0.01$), corroborating
> the redundancy weighting.

This converts the open problem to a measured result. Cite the bundle
DOI in the paper.

## Why KSG + PCA + MINE

KSG is the standard non-parametric MI estimator; widely cited; works on
continuous high-dim vectors. Known to underestimate at very high
dimensions, hence the PCA-32 cross-check. MINE (mutual information
neural estimator) provides a third orthogonal estimate that is
robust to high dimensionality but requires training; we use it on a
sample of pairs to validate KSG, not as the primary estimator.

The audit is therefore **estimator-triangulated**, not single-method.

## Cost Budget

For the production 13-embedder panel on the O*NET 30.2 corpus
(n=1,016, vectors averaging d=900):

- 78 pairwise KSG estimates × ~5 s each = ~6.5 min;
- 78 PCA-reduced KSG = ~1 min;
- 5 MINE validations × ~3 min = ~15 min;
- Per-embedder ablation: 13 × ~5 min training = ~65 min;

Total ~90 min on a single GPU. Repeat 3 times across the three corpora
= ~4.5 hours. Easily fits in a single workday.

## Edge Cases (Fail-Closed)

- Embedder panel version drifts mid-audit: `EMBEDDER_PANEL_DRIFT`
  error; audit aborts.
- One embedder vector is degenerate (zero-norm) for >5% of inputs:
  flag in `numerical_warnings`; do not silently exclude; record the
  fraction.
- KSG returns negative MI (small-sample bias): clamp at 0 with a
  flagged warning, **not** with silent suppression; numerical floor
  is a known KSG artifact.
- MINE training does not converge (loss flat): log; mark MINE result
  as `unconverged`; do not use as a validation source for that pair.
- Ablation classifier overfits (val drop unstable across seeds): run
  3 seeds per embedder; report mean ± std.

## Coordination With Other Docs

- Doc 27 (DDA bridge) supplies the per-embedder vectors that this
  audit consumes.
- Doc 29 (unified bundle schema) is where `mi_audit.json` lives.
- Doc 36 (three-paper roadmap) references this audit as the empirical
  anchor for the DDA paper's headline claim.
- Doc 23 (CI smoke regression) does **not** run the audit nightly; it
  runs only on tagged DDA-substrate releases.

## Acceptance

This work is done when:

- `dynamicjepa mi-audit` CLI exists;
- `mi_audit.json` is part of every bundle that includes a DDA stage;
- a numeric $k_{\text{eff}}$ value is published for at least three
  corpora;
- the corroboration ablation yields $r > 0.5$ (we expect ~0.7);
- PAPER.md §3.1 wording is updated to cite the audit.
