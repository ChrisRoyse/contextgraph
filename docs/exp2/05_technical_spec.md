# EXP-2 Technical Spec

How to build it. File layout, dependencies, I/O contracts, architecture.

## 1. Workspace Layout

```
contextgraph/
├── docs/
│   └── exp2/                       # This directory (specs)
├── scripts/
│   └── exp2/                       # Implementation
│       ├── __init__.py
│       ├── README.md               # Quick-start
│       ├── requirements.txt        # Python dependencies
│       ├── exp2.yml                # Hyperparameters (frozen, hashed)
│       ├── verify_environment.py   # T-001: substrate sanity
│       ├── build_corpus.py         # T-002 to T-004: corpus construction
│       ├── extract_embeddings.py   # T-005 to T-008: 14-embedder extraction
│       ├── fit_projections.py      # T-009: PCA fit on separate 1000 corpus
│       ├── estimators/
│       │   ├── __init__.py
│       │   ├── infonce.py          # T-010: InfoNCE estimator class
│       │   ├── ksg.py              # T-012: KSG estimator class
│       │   ├── synthetic.py        # T-011: synthetic ground-truth validation
│       │   └── bootstrap.py        # T-014: bootstrap CI
│       ├── run_estimators.py       # T-015: orchestrate 91 pairs both estimators
│       ├── effective_rank.py       # T-016: N_eff three methods
│       ├── visualize.py            # T-018: heatmap, plots
│       ├── fsv.py                  # T-019: full-state-verification harness
│       ├── replay.py               # T-019b: replay test
│       ├── audit_chain.py          # T-007 helpers: provenance
│       ├── run_exp2.py             # Top-level orchestrator
│       └── tests/
│           ├── __init__.py
│           ├── test_estimators.py
│           ├── test_corpus.py
│           ├── test_projections.py
│           ├── test_effective_rank.py
│           └── test_visualize.py
├── artefacts/
│   └── exp2/                       # gitignored; LFS or HuggingFace Hub
│       ├── corpus_manifest_v1.json
│       ├── pca_models.npz
│       ├── embeddings_per_input.npz   # [800 × 14 × 32]
│       ├── pair_results/              # 91 JSON files
│       │   ├── E1_E2.json
│       │   ├── E1_E3.json
│       │   └── ...
│       ├── n_eff_summary.json
│       ├── redundancy_heatmap.png
│       ├── redundancy_heatmap.pdf
│       ├── spectrum_plot.png
│       ├── spectrum_plot.pdf
│       ├── per_pair_table.csv
│       ├── audit_chain.json
│       ├── fsv_replay_log.json
│       └── publishable_summary.md
└── tasks/
    └── exp2/
        ├── todo.md                 # Per CLAUDE.md task tracking
        └── lessons.md              # Per CLAUDE.md lessons-learned
```

## 2. Why Python For The Harness (Not Rust)

The Context Graph stack is Rust; the EXP-2 harness is Python. Rationale:

- MI estimators have mature Python implementations (npeet, sklearn).
- Plotting + paper-figure generation is Python-native (matplotlib, seaborn).
- The harness is one-shot; doesn't need production-grade performance.
- Bridging to embedders is via the existing MCP server (Python client to localhost MCP server) or via a thin FFI to Rust embedders.

Code review for `run_exp2.py` orchestration: Python orchestration, Rust for embedder forward passes (already shipped), no new Rust required.

## 3. Dependencies (`requirements.txt`)

```
# Core
numpy>=1.26
scipy>=1.11
scikit-learn>=1.4
torch>=2.2
torchvision>=0.17

# MI estimators
npeet @ git+https://github.com/gregversteeg/NPEET.git@master  # KSG
# InfoNCE: implemented locally, no external dep

# Visualization
matplotlib>=3.8
seaborn>=0.13

# Configuration
pyyaml>=6.0
pydantic>=2.5

# Data
pandas>=2.1
pyarrow>=15.0
datasets>=2.16    # for FLORES, BEIR

# Testing & QA
pytest>=8.0
pytest-cov>=4.1
hypothesis>=6.95

# Auxiliary
click>=8.1
tqdm>=4.66
mcp>=1.0          # MCP client for Context Graph
```

Pin versions exactly in `requirements-frozen.txt` after first successful run. Audit chain logs the frozen versions.

## 4. Architecture: Data Flow

```
[corpus selection] → corpus_manifest.json
   ↓
[per-input 14-embedder extraction] → embeddings_per_input.npz [800 × 14 × native_dim]
   ↓
[per-embedder PCA-32 projection] → projected.npz [800 × 14 × 32]
   ↓
[per-pair InfoNCE training] → pair_results/{j}_{k}.json  (91 files)
   ↓
[per-pair KSG estimation] → updated pair_results/{j}_{k}.json
   ↓
[bootstrap aggregation] → updated pair_results/{j}_{k}.json
   ↓
[entropy estimation per embedder] → entropy_per_embedder.json
   ↓
[N_eff computation] → n_eff_summary.json
   ↓
[visualization] → heatmap.{png,pdf}, spectrum.{png,pdf}
   ↓
[audit chain finalize] → audit_chain.json
   ↓
[FSV replay] → fsv_replay_log.json (matches reference)
   ↓
[publishable summary writeup] → publishable_summary.md
```

## 5. I/O Contracts

### 5a. Corpus Manifest
Format: JSON. Schema in [04_data_corpus.md](04_data_corpus.md) §5.
File: `artefacts/exp2/corpus_manifest_v1.json`.
Hash: sha256 of canonicalized JSON (sorted keys, no whitespace except in strings).

### 5b. Embeddings Per Input
Format: NumPy `.npz` archive.
File: `artefacts/exp2/embeddings_per_input.npz`.
Contents:
```
e1_semantic: [800, 1024]    fp32
e2_temporal_recent: [800, 512]
e3_temporal_periodic: [800, 512]
e4_temporal_positional: [800, 512]
e5_causal_cause: [800, 768]
e6_sparse_top_k: [800, 128]   (top-K weights)
e7_code: [800, 1536]
e8_graph_source: [800, 1024]
e9_hdc_projected: [800, 1024]
e10_paraphrase: [800, 768]
e11_entity: [800, 768]
e12_token_pooled: [800, 128]
e13_splade_top_k: [800, 128]
e14_bge_m3: [800, 1024]
input_ids: [800] str
```

### 5c. PCA Models
Format: NumPy `.npz` archive.
File: `artefacts/exp2/pca_models.npz`.
Contents:
```
e1_components: [32, 1024]
e1_mean: [1024]
e1_explained_variance: [32]
... (one set per embedder)
sha256: per-embedder PCA model hash
```

### 5d. Projected Vectors
Format: NumPy `.npz` archive.
File: `artefacts/exp2/projected.npz`.
Contents:
```
e1: [800, 32]    fp32, L2-normalized
e2: [800, 32]
... (14 keys)
projection_metadata: dict
```

### 5e. Per-Pair Result
Format: JSON.
File: `artefacts/exp2/pair_results/E{j}_E{k}.json`.
Schema: [03_methodology.md](03_methodology.md) §13 (the audit-chain row).

### 5f. N_eff Summary
Format: JSON.
File: `artefacts/exp2/n_eff_summary.json`.
Contents:
```json
{
  "n_eff_redundancy_adjusted": {"point": 47.3, "ci_low": 44.1, "ci_high": 50.6},
  "n_eff_spectrum_entropy": {"point": 11.8, "ci_low": 11.4, "ci_high": 12.2},
  "n_eff_schur_complement_int": {"point": 11, "ci_low": 10, "ci_high": 12},
  "headline_method": "redundancy_adjusted",
  "headline_value": 47.3,
  "n_total_pairs": 91,
  "n_marginal_embedders": 14,
  "structural_upper_bound": 91,
  "computed_at": "2026-04-XX UTC",
  "audit_chain_sha256": "..."
}
```

### 5g. Heatmap
Format: PNG (web) + PDF (paper).
File: `artefacts/exp2/redundancy_heatmap.{png,pdf}`.
Contents: 14 × 14 heatmap, log-color-scale on MI value, with embedder labels and color bar.

### 5h. Audit Chain
Format: JSON.
File: `artefacts/exp2/audit_chain.json`.
Contents: every artefact's sha256, every code commit hash, every hyperparameter hash, every weight hash, every timestamp, every seed.

## 6. Configuration File (`exp2.yml`)

```yaml
# EXP-2 Configuration
# Frozen at production-run start.

run_id: exp2_v1_$(date +%Y%m%d)
operator: chris_royse
output_dir: artefacts/exp2/

corpus:
  manifest: artefacts/exp2/corpus_manifest_v1.json
  expected_count: 800
  expected_strata: 8

embedders:
  use_e11: true   # Include E11 in the audit (per PRD §5)
  panel_size: 14
  asymmetric_use: ["E5_cause", "E8_source", "E10_paraphrase"]

pca:
  fit_corpus: artefacts/exp2/pca_training_corpus_v1.json
  fit_corpus_count: 1000
  target_dim: 32
  variance_threshold: 0.85

# Hyperparameters (per 03_methodology.md §6)
infonce:
  critic_hidden_dim: 64
  critic_depth: 2
  optimizer: adamw
  lr: 3e-4
  weight_decay: 0.01
  beta1: 0.9
  beta2: 0.98
  batch_size: 128
  train_steps: 5000
  warmup_steps: 200
  train_val_split: 0.8
  early_stopping_patience: 500
  overfit_gap_threshold_nats: 0.1
  device: cuda
  dtype: float32

ksg:
  k: 5
  k_sweep_pilot: [3, 5, 10]
  distance_metric: chebyshev
  implementation: npeet

bootstrap:
  n_resamples: 10000
  seed: 42
  method: percentile
  confidence: 0.95

cross_validation:
  agreement_threshold_nats: 0.15
  min_agreement_rate: 0.80
  on_failure: rerun_with_diagnostics

effective_rank:
  primary_method: redundancy_adjusted
  schur_threshold: 0.05
  spectrum_normalization: frobenius

run:
  pilot_pairs: ["E1_E1", "E1_random", "E1_E1_noisy", "E1_E10"]
  parallel_pair_workers: 8
  enable_replay_check: true
  replay_tolerance: 1e-5
  log_level: INFO
  random_seed: 42
```

## 7. Embedding Extraction Strategy

Two paths to extract embeddings for the 800 inputs:

### Path A: Use existing CF_FINGERPRINTS
For inputs already stored (S1 Shakespeare, S4 learning events), retrieve directly from RocksDB. Fastest. No re-embedding.

### Path B: Fresh extraction via MCP server
For new inputs (S2-S3, S5-S8), use the MCP `store_memory` tool to ingest, then retrieve. Or call the warm provider directly via Python FFI bindings.

Both paths produce identical embeddings (same frozen weights). The audit chain records which path was used per input.

For implementation: a single `extract_embeddings.py` script that:
1. Reads the corpus manifest.
2. For each input:
   a. Try CF_FINGERPRINTS lookup by content_hash.
   b. If miss, ingest via MCP `store_memory` (write to a temporary session).
   c. Retrieve all 14 embeddings.
3. Saves to `embeddings_per_input.npz`.

The temporary session is wiped after extraction (no contamination of the production DB).

## 8. PCA Fitting Strategy

`fit_projections.py`:
1. Reads PCA training corpus manifest (1000 inputs, distinct from the 800).
2. For each embedder, fits PCA-32 (sklearn `IncrementalPCA` if memory-bound; standard `PCA` otherwise).
3. Validates explained variance ≥ 0.85; if not, refits with target_dim=64 for that slot only.
4. Saves projections + metadata to `pca_models.npz`.

PCA is fit *once* and frozen. The same PCA models are applied to:
- The 800 audit inputs.
- All 91 pair MI estimations.
- Bootstrap resamples (no PCA refit per resample; this is critical — refitting per resample inflates variance).

## 9. InfoNCE Implementation

`estimators/infonce.py` provides a class:

```python
class InfoNCEEstimator:
    def __init__(self, hyperparams: dict, device: str = "cuda"):
        self.hp = hyperparams
        self.device = device
        self.critic = self._build_critic()
        self.optimizer = self._build_optimizer()

    def fit(self, u: torch.Tensor, v: torch.Tensor) -> dict:
        """Trains critic on (u, v) joint samples. Returns training metrics."""
        ...

    def estimate(self, u: torch.Tensor, v: torch.Tensor) -> float:
        """Returns InfoNCE estimate of MI in nats on the given (u, v)."""
        ...

    def state_dict(self) -> dict:
        """For serialization."""
        ...
```

The class is unit-tested on synthetic ground-truth data (per [03_methodology.md](03_methodology.md) §12) before any production run.

## 10. KSG Implementation

`estimators/ksg.py` wraps `npeet` (or implements directly using `sklearn.neighbors.NearestNeighbors` if `npeet` proves unstable):

```python
def ksg_mi(u: np.ndarray, v: np.ndarray, k: int = 5) -> float:
    """Returns KSG-1 MI estimate in nats. u, v are [N, d]."""
    ...

def ksg_entropy(x: np.ndarray, k: int = 5) -> float:
    """Returns KSG entropy estimate in nats."""
    ...
```

## 11. Bootstrap Strategy Implementation

`estimators/bootstrap.py`:

```python
def bootstrap_ci(
    point_fn: Callable[[np.ndarray], float],
    data: np.ndarray,
    n_resamples: int = 10000,
    seed: int = 42,
    confidence: float = 0.95,
) -> tuple[float, float]:
    """Returns (ci_low, ci_high) for the given point function."""
    ...
```

The function takes a point estimator (could be InfoNCE applied to a fixed critic, or a fresh KSG run) and the underlying data array, resamples, applies, returns percentiles.

## 12. Effective-Rank Computation

`effective_rank.py`:

```python
def n_eff_redundancy_adjusted(
    mi_matrix: np.ndarray,    # [N, N], symmetric, off-diagonal MI
    entropy_per_embedder: np.ndarray,    # [N]
) -> float:
    ...

def n_eff_spectrum_entropy(mi_matrix: np.ndarray) -> float:
    ...

def n_eff_schur_complement(mi_matrix: np.ndarray, threshold: float = 0.05) -> int:
    ...
```

Each function returns a scalar; bootstrap is wired in `run_exp2.py` orchestration.

## 13. Visualization

`visualize.py`:

```python
def render_redundancy_heatmap(
    mi_matrix: np.ndarray,
    embedder_labels: list[str],
    output_png: Path,
    output_pdf: Path,
) -> None:
    ...

def render_spectrum_plot(
    eigenvalues: np.ndarray,
    output_png: Path,
    output_pdf: Path,
) -> None:
    ...

def render_per_embedder_entropy_bars(
    entropy: np.ndarray,
    embedder_labels: list[str],
    output_png: Path,
    output_pdf: Path,
) -> None:
    ...

def render_redundancy_scatter(
    mi_matrix: np.ndarray,
    embedder_labels: list[str],
    output_png: Path,
    output_pdf: Path,
) -> None:
    ...
```

Style: matplotlib + seaborn. Color: viridis or magma (perceptually uniform). All saved at 300 DPI for print-quality PDF.

## 14. Audit Chain

`audit_chain.py`:

```python
class AuditChain:
    def __init__(self, run_id: str, output_path: Path):
        ...

    def add_artefact(
        self, artefact_path: Path, label: str, parent_artefact_ids: list[str]
    ) -> str:
        """Computes sha256, logs to chain, returns artefact_id."""
        ...

    def add_event(self, event_type: str, payload: dict) -> None:
        """Logs a runtime event."""
        ...

    def finalize(self) -> Path:
        """Writes the audit chain JSON, returns its sha256."""
        ...
```

Every artefact passes through `add_artefact`. Every config load, every model load, every bootstrap pass is an event.

## 15. Replay Determinism

`replay.py`:

```python
def replay(
    config_path: Path,
    reference_artefacts: Path,
    tolerance: float = 1e-5,
) -> ReplayResult:
    """Re-runs the entire pipeline from scratch and compares to reference."""
    ...
```

The replay produces an alternate set of artefacts; pair-by-pair, replay artefacts are compared to reference within tolerance. Pass criterion: zero numerical mismatches outside $10^{-5}$ across all reported scalars.

## 16. Top-Level Orchestrator

`run_exp2.py` is a click CLI:

```python
@click.command()
@click.option("--config", required=True)
@click.option("--phase", default="all")
def main(config: str, phase: str):
    """Run EXP-2 pipeline. --phase ∈ {all, corpus, embeddings, projections,
    estimators, n_eff, visualize, fsv}."""
    ...
```

Each phase reads its inputs from artefacts/, writes its outputs to artefacts/. Phases can be run independently for incremental debugging.

## 17. Determinism Knobs

To enforce determinism per Constitution C6:

```python
torch.manual_seed(42)
torch.cuda.manual_seed_all(42)
np.random.seed(42)
torch.backends.cudnn.deterministic = True
torch.backends.cudnn.benchmark = False
torch.use_deterministic_algorithms(True, warn_only=True)
```

The InfoNCE training is the main source of nondeterminism (cuDNN convolution stochasticity); we use deterministic mode at the cost of ~10% throughput.

## 18. Logging

Structured logging via standard `logging` module:
- Each phase logs at INFO start/end with timing.
- Each pair logs at DEBUG with intermediate values.
- Errors at ERROR with full context.
- Audit chain logs are *never* at TRACE; they are first-class artefacts.

JSON-formatted logs written to `artefacts/exp2/run.log.jsonl` for grep-able analysis.

## 19. Test Harness

`scripts/exp2/tests/`:

- `test_estimators.py` — synthetic ground-truth + edge cases (zero MI, full MI, unit-vectors).
- `test_corpus.py` — corpus manifest validation, deduplication, length bounds.
- `test_projections.py` — PCA fit + apply + L2 norm + variance check.
- `test_effective_rank.py` — three N_eff methods on known matrices.
- `test_visualize.py` — output exists, valid PNG/PDF.

Run via `pytest scripts/exp2/tests/`. Coverage target ≥ 80%.

## 20. Open Source Release Bundle

After T-022 (paper patch) lands, release on GitHub:

```
contextgraph-exp2-v1/
├── README.md                       # Quick-start
├── LICENSE                         # Apache-2.0
├── docs/
│   └── (copy of this docs/exp2/ tree)
├── scripts/
│   └── (copy of this scripts/exp2/ tree)
├── artefacts/
│   ├── corpus_manifest_v1.json     # full text included if license permits
│   ├── corpus_manifest_v1.json.sha256
│   ├── pca_models.npz              # release as Git LFS or HuggingFace Hub
│   └── per_pair_table.csv
├── PAPER_PATCH.md                  # Drop-in §3.1 + §6.1 update
└── CITATION.cff
```

CITATION.cff lets users cite the EXP-2 release directly. Bundle DOI'd via Zenodo at v1.0.0 release.
