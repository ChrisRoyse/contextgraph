# EXP-2 Code Stubs

**Pseudocode for every moving part.** Not production code — these are the reference patterns Chris implements from during Days 5–6. Every stub matches the spec in [08_implementation_plan.md](08_implementation_plan.md).

---

## 1 · Rust — Corpus build top-level

```rust
// crates/context-graph-exp2/src/main.rs
use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "context-graph-exp2")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    BuildCorpus {
        #[arg(long)]
        n: usize,
        #[arg(long, default_value_t = 42)]
        strata_seed: u64,
        #[arg(long, default_value_t = 2026)]
        sketch_seed: u64,
        #[arg(long, default_value_t = 137)]
        split_seed: u64,
        #[arg(long)]
        out: std::path::PathBuf,
        #[arg(long)]
        manifest: std::path::PathBuf,
    },
    BuildSensitivity {
        #[arg(long)]
        source: String,  // wikipedia5k | code5k | shakespeare2k
        #[arg(long)]
        n: usize,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    ValidateCorpus {
        #[arg(long)]
        input: std::path::PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::BuildCorpus { n, strata_seed, sketch_seed, split_seed, out, manifest } => {
            let builder = CorpusBuilder::new(n, strata_seed, sketch_seed, split_seed, out, manifest);
            builder.build().await?;
        }
        Commands::BuildSensitivity { source, n, out } => {
            let builder = SensitivityCorpusBuilder::new(&source, n, out);
            builder.build().await?;
        }
        Commands::ValidateCorpus { input } => {
            validate_corpus(&input)?;
        }
    }
    Ok(())
}
```

---

## 2 · Rust — Panel vector construction

```rust
// crates/context-graph-exp2/src/panel_vectors.rs
use context_graph_core::types::fingerprint::SemanticFingerprint;
use context_graph_embeddings::types::ModelId;
use ndarray::{Array1, concatenate, Axis};

pub fn prepare_panel_vectors(
    fp: &SemanticFingerprint,
    sketch_e6: &JlSketch,
    sketch_e13: &JlSketch,
) -> Result<PanelVectors> {
    Ok(PanelVectors {
        e1: as_array(&fp.e1_semantic),   // 1024
        e2: as_array(&fp.e2_temporal_recent),  // 512
        e3: as_array(&fp.e3_temporal_periodic),  // 512
        e4: as_array(&fp.e4_temporal_positional),  // 512
        e5: l2_normalize(concatenate(
            Axis(0),
            &[as_array(&fp.e5_causal_as_cause).view(),
              as_array(&fp.e5_causal_as_effect).view()]
        )?),  // 1536
        e6: l2_normalize(sketch_e6.apply(&densify_sparse(&fp.e6_sparse, 30522))),  // 2048
        e7: as_array(&fp.e7_code),  // 1536
        e8: l2_normalize(concatenate(
            Axis(0),
            &[as_array(&fp.e8_graph_as_source).view(),
              as_array(&fp.e8_graph_as_target).view()]
        )?),  // 2048
        e9: as_array(&fp.e9_hdc),  // 1024
        e10: l2_normalize(concatenate(
            Axis(0),
            &[as_array(&fp.e10_multimodal_paraphrase).view(),
              as_array(&fp.e10_multimodal_as_context).view()]
        )?),  // 1536
        e11: as_array(&fp.e11_entity),  // 768
        e12: l2_normalize(mean_pool_tokens(&fp.e12_late_interaction)),  // 128
        e13: l2_normalize(sketch_e13.apply(&densify_sparse(&fp.e13_splade, 30522))),  // 2048
    })
}

pub fn densify_sparse(sparse: &SparseVector, vocab_size: usize) -> Array1<f32> {
    let mut dense = Array1::zeros(vocab_size);
    for (idx, val) in sparse.indices.iter().zip(sparse.values.iter()) {
        dense[*idx as usize] = *val;
    }
    dense
}

pub fn mean_pool_tokens(tokens: &[Vec<f32>]) -> Array1<f32> {
    if tokens.is_empty() {
        return Array1::zeros(128);
    }
    let n = tokens.len() as f32;
    let mut sum = Array1::<f32>::zeros(128);
    for token in tokens {
        let arr = Array1::from_vec(token.clone());
        sum = sum + arr;
    }
    sum / n
}

pub fn l2_normalize(mut v: Array1<f32>) -> Array1<f32> {
    let norm = v.mapv(|x| x * x).sum().sqrt();
    if norm > 1e-8 {
        v = v / norm;
    }
    v
}
```

---

## 3 · Rust — JL sketch

```rust
// crates/context-graph-exp2/src/jl_sketch.rs
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Normal};

pub struct JlSketch {
    pub matrix: ndarray::Array2<f32>,  // (target_dim, source_dim)
    pub seed: u64,
    pub target_dim: usize,
    pub source_dim: usize,
}

impl JlSketch {
    pub fn new(seed: u64, target_dim: usize, source_dim: usize) -> Self {
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let std = 1.0 / (target_dim as f32).sqrt();
        let dist = Normal::new(0.0, std).unwrap();

        let data: Vec<f32> = (0..target_dim * source_dim)
            .map(|_| dist.sample(&mut rng))
            .collect();
        let matrix = ndarray::Array2::from_shape_vec((target_dim, source_dim), data).unwrap();

        Self { matrix, seed, target_dim, source_dim }
    }

    pub fn apply(&self, dense: &ndarray::Array1<f32>) -> ndarray::Array1<f32> {
        assert_eq!(dense.len(), self.source_dim, "input dim mismatch");
        self.matrix.dot(dense)
    }

    pub fn save_to_npz(&self, path: &std::path::Path) -> anyhow::Result<()> {
        ndarray_npy::write_npy(path, &self.matrix)?;
        Ok(())
    }
}
```

---

## 4 · Python — MINE training / eval

```python
# scripts/exp2/lib/mine.py
import torch
import torch.nn as nn
import numpy as np


class MineCritic(nn.Module):
    def __init__(self, dim_x: int, dim_y: int, hidden: int = 256):
        super().__init__()
        self.dim_x = dim_x
        self.dim_y = dim_y
        self.net = nn.Sequential(
            nn.Linear(dim_x + dim_y, hidden),
            nn.ReLU(),
            nn.Linear(hidden, hidden),
            nn.ReLU(),
            nn.Linear(hidden, 1),
        )

    def forward(self, x, y):
        return self.net(torch.cat([x, y], dim=-1)).squeeze(-1)


def train_mine(
    x_train: torch.Tensor,
    y_train: torch.Tensor,
    seed: int,
    steps: int = 2000,
    batch: int = 512,
    lr: float = 1e-4,
    ema_alpha: float = 0.01,
    device: str = "cuda",
) -> tuple[MineCritic, list[float]]:
    """Returns (trained critic, per-step estimated MI in bits)."""
    torch.manual_seed(seed)
    torch.backends.cudnn.deterministic = True
    torch.backends.cudnn.benchmark = False

    dim_x = x_train.shape[1]
    dim_y = y_train.shape[1]
    critic = MineCritic(dim_x, dim_y).to(device)
    opt = torch.optim.Adam(critic.parameters(), lr=lr, betas=(0.5, 0.999))

    x_train = x_train.to(device)
    y_train = y_train.to(device)

    ema_logsumexp = None
    mi_trace = []

    for step in range(steps):
        # Positive: paired (x_i, y_i)
        idx_pos = torch.randint(len(x_train), (batch,), device=device)
        x_pos = x_train[idx_pos]
        y_pos = y_train[idx_pos]

        # Negative: shuffled y within batch
        idx_neg = torch.randint(len(y_train), (batch,), device=device)
        y_neg = y_train[idx_neg]

        t_pos = critic(x_pos, y_pos)
        t_neg = critic(x_pos, y_neg)

        # Log-sum-exp for the denominator
        logsumexp_neg = torch.logsumexp(t_neg, dim=0) - np.log(batch)

        # EMA on denominator for gradient bias correction (Belghazi eq 12)
        if ema_logsumexp is None:
            ema_logsumexp = logsumexp_neg.detach()
        else:
            ema_logsumexp = (1 - ema_alpha) * ema_logsumexp + ema_alpha * logsumexp_neg.detach()

        # Corrected gradient term
        corrected_denom = logsumexp_neg * (logsumexp_neg.detach() / (ema_logsumexp + 1e-8))
        loss = -(t_pos.mean() - corrected_denom)

        opt.zero_grad()
        loss.backward()
        opt.step()

        # Record bits (not the loss)
        mi_trace.append((t_pos.mean() - logsumexp_neg).item() / np.log(2))

    return critic, mi_trace


def eval_mine(
    critic: MineCritic,
    x_test: torch.Tensor,
    y_test: torch.Tensor,
    device: str = "cuda",
) -> float:
    """Returns held-out MI estimate in bits."""
    critic.eval()
    x_test = x_test.to(device)
    y_test = y_test.to(device)
    with torch.no_grad():
        # Positive pairs: diagonal
        t_pos = critic(x_test, y_test).mean()

        # Negative: all (n_test)^2 cross products
        n = len(x_test)
        # Build batch-compatible cross-product tensor
        x_rep = x_test.unsqueeze(1).expand(n, n, -1).reshape(n * n, -1)
        y_rep = y_test.unsqueeze(0).expand(n, n, -1).reshape(n * n, -1)
        t_neg = critic(x_rep, y_rep)
        logsumexp_neg = torch.logsumexp(t_neg, dim=0) - np.log(n * n)

        hat_I_nats = (t_pos - logsumexp_neg).item()
        return hat_I_nats / np.log(2)  # in bits
```

---

## 5 · Python — InfoNCE

```python
# scripts/exp2/lib/infonce.py
import torch
import torch.nn as nn
import torch.nn.functional as F
import numpy as np


def train_and_eval_infonce(
    x_train: torch.Tensor,
    y_train: torch.Tensor,
    x_test: torch.Tensor,
    y_test: torch.Tensor,
    seed: int,
    steps: int = 2000,
    batch: int = 256,
    tau: float = 0.1,
    lr: float = 1e-4,
    device: str = "cuda",
) -> tuple[float, float]:
    """Returns (hat_I_bits, is_saturated)."""
    torch.manual_seed(seed)
    torch.backends.cudnn.deterministic = True

    W_x = nn.Linear(x_train.shape[1], 64, bias=False).to(device)
    W_y = nn.Linear(y_train.shape[1], 64, bias=False).to(device)
    opt = torch.optim.Adam(list(W_x.parameters()) + list(W_y.parameters()), lr=lr)

    x_train = x_train.to(device)
    y_train = y_train.to(device)

    for step in range(steps):
        idx = torch.randint(len(x_train), (batch,), device=device)
        x = W_x(x_train[idx])
        y = W_y(y_train[idx])
        logits = (x @ y.T) / tau
        labels = torch.arange(batch, device=device)
        loss = (F.cross_entropy(logits, labels) + F.cross_entropy(logits.T, labels)) / 2
        opt.zero_grad()
        loss.backward()
        opt.step()

    with torch.no_grad():
        x_t = W_x(x_test.to(device))
        y_t = W_y(y_test.to(device))
        K = len(x_t)
        logits = (x_t @ y_t.T) / tau
        log_prob_pos = torch.diag(logits) - torch.logsumexp(logits, dim=1)
        hat_I_nats = np.log(K) + log_prob_pos.mean().item()
        hat_I_bits = hat_I_nats / np.log(2)

    max_possible_bits = np.log(K) / np.log(2)
    is_saturated = hat_I_bits > 0.9 * max_possible_bits
    return hat_I_bits, is_saturated
```

---

## 6 · Python — KSG

```python
# scripts/exp2/lib/ksg.py
import numpy as np
from scipy.spatial import cKDTree
from scipy.special import digamma


def ksg_mi(x: np.ndarray, y: np.ndarray, k: int = 3) -> float:
    """
    KSG Algorithm 1 (Kraskov, Stögbauer, Grassberger 2004).
    Returns MI estimate in bits.
    """
    assert len(x) == len(y), "Unequal sample sizes"
    n = len(x)

    # Joint space
    xy = np.hstack([x, y])
    tree_xy = cKDTree(xy, leafsize=16)
    tree_x = cKDTree(x, leafsize=16)
    tree_y = cKDTree(y, leafsize=16)

    # For each point, k-th nearest neighbor in joint space (Chebyshev norm)
    dists, _ = tree_xy.query(xy, k=k + 1, p=np.inf)
    eps = dists[:, -1]  # distance to k-th neighbor (0th is self)

    nx = np.array([len(tree_x.query_ball_point(xi, eps[i] - 1e-10, p=np.inf)) - 1
                   for i, xi in enumerate(x)])
    ny = np.array([len(tree_y.query_ball_point(yi, eps[i] - 1e-10, p=np.inf)) - 1
                   for i, yi in enumerate(y)])

    # Algorithm 1 estimator
    mi_nats = digamma(k) - np.mean(digamma(nx + 1) + digamma(ny + 1)) + digamma(n)

    return max(0.0, mi_nats / np.log(2))  # clamp at 0; in bits
```

---

## 7 · Python — bootstrap CI

```python
# scripts/exp2/bootstrap_ci.py
import numpy as np


def bootstrap_ci(seed_estimates: list[float], B: int = 1000, seed: int = 999) -> tuple[float, float, float]:
    """
    From a list of per-seed MI estimates (5 values), compute bootstrap 95% CI.
    Returns (lower, median, upper).
    """
    rng = np.random.default_rng(seed)
    estimates = np.array(seed_estimates)
    n = len(estimates)

    resampled_medians = []
    for _ in range(B):
        sample = rng.choice(estimates, size=n, replace=True)
        resampled_medians.append(np.median(sample))

    return (
        np.percentile(resampled_medians, 2.5),
        np.median(estimates),
        np.percentile(resampled_medians, 97.5),
    )
```

---

## 8 · Python — $N_\text{eff}$ aggregation

```python
# scripts/exp2/aggregate_n_eff.py
import numpy as np
import json


def compute_n_eff_pair(
    H_per_embedder: np.ndarray,  # shape (N,)
    I_pairwise: np.ndarray,      # shape (N, N) symmetric; diagonal = 0
) -> float:
    """
    Headline formula:
        N_eff = sum_H / (H_bar + (2/N) * sum_I / H_bar)
    where H_bar = mean(H_per_embedder) and sum_I = sum over j<k of I(phi_j; phi_k).
    """
    N = len(H_per_embedder)
    H_bar = np.mean(H_per_embedder)
    sum_H = np.sum(H_per_embedder)

    # Upper triangle excluding diagonal
    iu = np.triu_indices(N, k=1)
    sum_I = np.sum(I_pairwise[iu])

    denominator = H_bar + (2.0 / N) * (sum_I / H_bar)
    return sum_H / denominator


def compute_n_eff_min(H_per_embedder, I_pairwise):
    N = len(H_per_embedder)
    H_bar = np.mean(H_per_embedder)
    return N - np.max(I_pairwise) / H_bar


def compute_n_eff_sum(H_per_embedder, I_pairwise):
    N = len(H_per_embedder)
    H_bar = np.mean(H_per_embedder)
    iu = np.triu_indices(N, k=1)
    sum_I = np.sum(I_pairwise[iu])
    return N - sum_I / H_bar


def bootstrap_n_eff(
    H_per_embedder: np.ndarray,
    I_seeds_per_pair: dict[tuple[int, int], list[float]],
    B: int = 1000,
    seed: int = 999,
) -> dict[str, tuple[float, float, float]]:
    """
    Bootstrap CI on N_eff,pair by resampling the seed-level MI estimates per pair
    and propagating through the aggregation formula.
    """
    rng = np.random.default_rng(seed)
    N = len(H_per_embedder)

    n_eff_samples = []
    for _ in range(B):
        I_mat = np.zeros((N, N))
        for (j, k), estimates in I_seeds_per_pair.items():
            sample = rng.choice(estimates, size=len(estimates), replace=True)
            mi = np.median(sample)
            I_mat[j, k] = mi
            I_mat[k, j] = mi

        n_eff_samples.append(compute_n_eff_pair(H_per_embedder, I_mat))

    arr = np.array(n_eff_samples)
    return {
        "n_eff_pair": (np.percentile(arr, 2.5), np.median(arr), np.percentile(arr, 97.5)),
    }
```

---

## 9 · Python — figure generation

```python
# scripts/exp2/generate_figures.py (excerpt)
import matplotlib.pyplot as plt
import numpy as np


def render_mi_heatmap(
    I_matrix: np.ndarray,  # (13, 13) symmetric
    H_diagonal: np.ndarray,  # (13,)
    embedder_names: list[str],
    out_path: str,
):
    fig, ax = plt.subplots(figsize=(10, 9))
    im = ax.imshow(I_matrix, cmap="viridis", aspect="equal")
    # annotate
    for i in range(13):
        for j in range(13):
            val = I_matrix[i, j] if i != j else H_diagonal[i]
            ax.text(j, i, f"{val:.1f}", ha="center", va="center",
                    color="white" if val > I_matrix.max() * 0.5 else "black",
                    fontsize=7)
    ax.set_xticks(range(13))
    ax.set_yticks(range(13))
    ax.set_xticklabels(embedder_names, rotation=45, ha="right")
    ax.set_yticklabels(embedder_names)
    ax.set_title("Pairwise MI (off-diag) and diagonal entropy (bits)")
    plt.colorbar(im, ax=ax, label="bits")
    plt.tight_layout()
    plt.savefig(out_path, dpi=200)
    plt.close()


def render_convergence_curve(
    per_step_mi: dict[int, list[float]],  # seed -> bits-per-step
    out_path: str,
):
    fig, ax = plt.subplots(figsize=(8, 4))
    for seed, trace in per_step_mi.items():
        ax.plot(trace, alpha=0.6, label=f"seed={seed}")
    ax.set_xlabel("step")
    ax.set_ylabel("estimated MI (bits)")
    ax.legend()
    ax.set_title("MINE convergence, 5 seeds")
    plt.tight_layout()
    plt.savefig(out_path, dpi=150)
    plt.close()
```

---

## 10 · End-to-end shell driver

```bash
# run_exp2.sh — end-to-end from corpus build to report
set -euo pipefail

DATE=$(date +%Y-%m-%d)
ARTIFACTS_DIR=/home/cabdru/contextgraph/exp2_artifacts
mkdir -p "$ARTIFACTS_DIR"/{corpus,results,logs,figures,report}

# 1. Build primary corpus
for N in 100 500 1000; do
  target/release/context-graph-exp2 build-corpus \
    --n $N \
    --strata-seed 42 \
    --sketch-seed 2026 \
    --split-seed 137 \
    --out "$ARTIFACTS_DIR/corpus/exp2_corpus_n${N}_${DATE}.npz" \
    --manifest "/home/cabdru/contextgraph/docs/exp2/.corpus_manifest"
done

# 2. Build sensitivity corpora
for SRC in wikipedia5k code5k shakespeare2k; do
  target/release/context-graph-exp2 build-sensitivity \
    --source $SRC --n 1000 \
    --out "$ARTIFACTS_DIR/corpus/exp2_${SRC}_${DATE}.npz"
done

# 3. Fit PCAs
source .venv-exp2/bin/activate
for N in 100 500 1000; do
  python scripts/exp2/prepare_pca.py \
    --corpus "$ARTIFACTS_DIR/corpus/exp2_corpus_n${N}_${DATE}.npz" \
    --split-seed 137 \
    --out "$ARTIFACTS_DIR/corpus/exp2_pca_n${N}_${DATE}.npz"
done

# 4. Controls
python scripts/exp2/run_controls.py \
  --corpus "$ARTIFACTS_DIR/corpus/exp2_corpus_n1000_${DATE}.npz" \
  --pca "$ARTIFACTS_DIR/corpus/exp2_pca_n1000_${DATE}.npz" \
  --out "$ARTIFACTS_DIR/results/controls_${DATE}.json"

if ! jq -e '.all_passed' "$ARTIFACTS_DIR/results/controls_${DATE}.json"; then
  echo "CONTROLS FAILED — halting. See $ARTIFACTS_DIR/results/controls_${DATE}.json"
  exit 1
fi

# 5. Sweep pairs (primary distribution)
for N in 100 500 1000; do
  python scripts/exp2/run_all_pairs.py \
    --corpus "$ARTIFACTS_DIR/corpus/exp2_corpus_n${N}_${DATE}.npz" \
    --pca "$ARTIFACTS_DIR/corpus/exp2_pca_n${N}_${DATE}.npz" \
    --estimator mine \
    --parallel 4 --resume \
    --out-dir "$ARTIFACTS_DIR/results/"

  python scripts/exp2/run_all_pairs.py \
    --corpus "$ARTIFACTS_DIR/corpus/exp2_corpus_n${N}_${DATE}.npz" \
    --pca "$ARTIFACTS_DIR/corpus/exp2_pca_n${N}_${DATE}.npz" \
    --estimator infonce \
    --parallel 4 --resume \
    --out-dir "$ARTIFACTS_DIR/results/"
done

# 6. KSG on 12 pre-registered pairs
for N in 100 500 1000; do
  python scripts/exp2/run_ksg_pair.py \
    --corpus "$ARTIFACTS_DIR/corpus/exp2_corpus_n${N}_${DATE}.npz" \
    --pca "$ARTIFACTS_DIR/corpus/exp2_pca_n${N}_${DATE}.npz" \
    --pairs-file docs/exp2/ksg_pairs.json \
    --out "$ARTIFACTS_DIR/results/exp2_results_ksg_n${N}.jsonl"
done

# 7. Bootstrap + aggregation
python scripts/exp2/bootstrap_ci.py \
  --results "$ARTIFACTS_DIR/results/exp2_results_mine_n1000.jsonl" \
  --B 1000 \
  --out "$ARTIFACTS_DIR/results/exp2_mine_ci_n1000.json"

python scripts/exp2/aggregate_n_eff.py \
  --results-mine "$ARTIFACTS_DIR/results/exp2_results_mine_n1000.jsonl" \
  --results-infonce "$ARTIFACTS_DIR/results/exp2_results_infonce_n1000.jsonl" \
  --results-ksg "$ARTIFACTS_DIR/results/exp2_results_ksg_n1000.jsonl" \
  --corpus "$ARTIFACTS_DIR/corpus/exp2_corpus_n1000_${DATE}.npz" \
  --pca "$ARTIFACTS_DIR/corpus/exp2_pca_n1000_${DATE}.npz" \
  --n 1000 \
  --out "$ARTIFACTS_DIR/results/exp2_n_eff_n1000.json"

# 8. Figures and report
python scripts/exp2/generate_figures.py \
  --results-dir "$ARTIFACTS_DIR/results/" \
  --n 1000 \
  --out-dir "$ARTIFACTS_DIR/figures/"

python scripts/exp2/generate_report.py \
  --n 1000 \
  --results-dir "$ARTIFACTS_DIR/results/" \
  --figures-dir "$ARTIFACTS_DIR/figures/" \
  --template docs/exp2/07_reporting_template.md \
  --out "$ARTIFACTS_DIR/report/exp2_report_n1000.md"

echo "=== EXP-2 COMPLETE ==="
echo "Report: $ARTIFACTS_DIR/report/exp2_report_n1000.md"
```

---

## 11 · Determinism regression test

```python
# scripts/exp2/tests/test_determinism.py
def test_full_pipeline_determinism():
    """Running the pipeline twice with the same seeds produces bit-identical N_eff."""
    result1 = run_pipeline_smoke_test(seed=42)  # smoke test on n=50 corpus
    result2 = run_pipeline_smoke_test(seed=42)
    assert abs(result1.n_eff_pair - result2.n_eff_pair) < 1e-6
    assert result1.per_pair_mine == result2.per_pair_mine
```

This test **must pass before preregistration freeze** and must be re-run after any code change.
