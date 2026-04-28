# EXP-2 Implementation Plan

**The code layout.** What files to create, what they do, and which language they go in. Chris drafts; Steve reviews before any long-running job.

---

## 1 · Split between Rust and Python

| Task | Language | Why |
|------|----------|-----|
| Corpus build (read daemon → panel tensors) | Rust (new `context-graph-exp2` bin) | Daemon client already in-tree; embedding pipeline is Rust; avoids Python FFI for the 13-embedder fetch |
| PCA fit + serialize | Python (`sklearn.decomposition.PCA`) | Drop-in, reproducible, battle-tested |
| MINE / InfoNCE training | Python + PyTorch | Neural critic, Adam, EMA — standard torch idioms |
| KSG estimator | Python (`scipy.spatial.cKDTree`) | Standard KSG library routines |
| Bootstrap + aggregation | Python (numpy) | |
| Reporting (matplotlib plots, markdown table generation) | Python | |
| Unit tests | Python `pytest` (+ Rust cargo test for the binary) | |

No new Rust crate; EXP-2 is a **single binary** in `crates/context-graph-exp2/` that reads fingerprints and writes `.npz`, plus Python scripts under `scripts/exp2/`.

---

## 2 · Directory layout (to be created)

```
/home/cabdru/contextgraph/
├── crates/
│   └── context-graph-exp2/                 # NEW
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs                     # CLI entry
│       │   ├── corpus_builder.rs           # sample + classify + embed
│       │   ├── strata.rs                   # 4-way stratification logic
│       │   ├── panel_vectors.rs            # dual-concat, JL sketch, pool
│       │   ├── jl_sketch.rs                # Johnson-Lindenstrauss matrix
│       │   ├── npz_writer.rs               # serialize to .npz format
│       │   └── daemon_client.rs            # fetch from :3100
│       └── tests/
│           └── corpus_builder_test.rs
├── scripts/
│   └── exp2/                               # NEW
│       ├── __init__.py
│       ├── prepare_pca.py                  # fit & save PCA matrices
│       ├── run_controls.py                 # E1-copy, random-proj, zero-MI
│       ├── run_mine_pair.py                # single-pair MINE worker
│       ├── run_infonce_pair.py             # single-pair InfoNCE worker
│       ├── run_ksg_pair.py                 # single-pair KSG worker
│       ├── run_all_pairs.py                # orchestrate 78-pair sweep
│       ├── bootstrap_ci.py                 # CI computation
│       ├── aggregate_n_eff.py              # compute all four N_eff
│       ├── generate_figures.py             # heatmap, convergence plots
│       ├── generate_report.py              # fill reporting template
│       ├── lib/
│       │   ├── mine.py                     # MINE critic + training loop
│       │   ├── infonce.py                  # InfoNCE loss + training
│       │   ├── ksg.py                      # Kraskov-Stögbauer-Grassberger
│       │   ├── block_entropy.py            # Paninski / Chao-Shen joint H
│       │   ├── npz_io.py                   # reader
│       │   ├── cka.py                      # linear CKA for secondary
│       │   └── seeds.py                    # pinned RNGs
│       └── tests/
│           ├── test_mine_on_synthetic.py   # synthetic Gaussian known-MI
│           ├── test_infonce_bound.py
│           ├── test_ksg_gaussian.py
│           └── test_controls.py
├── exp2_artifacts/                         # local, NOT git
│   ├── corpus/*.npz
│   ├── results/*.jsonl
│   ├── logs/*.csv
│   ├── figures/*.pdf
│   └── report/
├── docs/exp2/
│   ├── README.md
│   ├── 00-16_*.md
│   ├── .corpus_manifest                    # sha256 of every artifact
│   ├── .panel_dryrun_ok                    # sentinel after §5 dry-run
│   └── control_failure_<date>.md           # if any control fails
└── Cargo.toml                              # add context-graph-exp2 to members
```

---

## 3 · Rust binary — `context-graph-exp2`

### 3.1 CLI contract

```
context-graph-exp2 build-corpus \
    --n 1000 \
    --strata-seed 42 \
    --sketch-seed 2026 \
    --split-seed 137 \
    --out /path/to/exp2_corpus_n1000_<date>.npz \
    --manifest /path/to/docs/exp2/.corpus_manifest

context-graph-exp2 build-sensitivity \
    --source wikipedia5k | code5k | shakespeare2k \
    --n 1000 \
    --out /path/to/...

context-graph-exp2 validate-corpus \
    --in /path/to/exp2_corpus_n1000_<date>.npz
    # prints dim, finite-check, L2-norm summaries per slot
```

### 3.2 Dependencies

```toml
[dependencies]
context-graph-core = { path = "../context-graph-core" }
context-graph-storage = { path = "../context-graph-storage" }
context-graph-embeddings = { path = "../context-graph-embeddings" }
tokio = { version = "1.35", features = ["rt-multi-thread", "macros"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
ndarray = "0.15"
ndarray-npy = "0.8"        # .npz writing
rand = "0.8"
rand_chacha = "0.3"        # deterministic seeding
sha2 = "0.10"
anyhow = "1.0"
clap = { version = "4.5", features = ["derive"] }
```

### 3.3 Key logic outline

```rust
// main.rs — CLI dispatch
// corpus_builder.rs — orchestrator
struct CorpusBuilder {
    daemon_port: u16,
    strata_seed: u64,
    sketch_seed: u64,
    split_seed: u64,
    n: usize,
    out_path: PathBuf,
}

impl CorpusBuilder {
    async fn build(&self) -> Result<()> {
        let all_ids = self.list_memories().await?;
        let strata = self.classify_strata(&all_ids).await?;
        let sampled = self.stratified_sample(&strata, self.n)?;
        let panel_tensors = self.embed_panel(&sampled).await?;
        let sketched = self.apply_jl_sketch(&panel_tensors)?;
        self.serialize_npz(&sketched)?;
        self.update_manifest()?;
        Ok(())
    }
}

// panel_vectors.rs — exact per-embedder preprocessing
fn prepare_vector(slot: ModelId, fp: &SemanticFingerprint) -> Array1<f32> {
    match slot {
        ModelId::Causal => {
            let cause = &fp.e5_causal_as_cause;
            let effect = &fp.e5_causal_as_effect;
            let concat = concat1d(cause, effect);
            l2_normalize(concat)
        }
        ModelId::Graph => {
            let src = &fp.e8_graph_as_source;
            let tgt = &fp.e8_graph_as_target;
            l2_normalize(concat1d(src, tgt))
        }
        ModelId::Contextual => {
            let p = &fp.e10_multimodal_paraphrase;
            let c = &fp.e10_multimodal_as_context;
            l2_normalize(concat1d(p, c))
        }
        ModelId::Sparse | ModelId::Splade => {
            densify_to_30522(&fp.e6_sparse_or_e13_splade)
        }
        ModelId::LateInteraction => {
            let tokens = &fp.e12_late_interaction;
            l2_normalize(mean_pool(tokens))
        }
        _ => fp.slot(slot).to_owned()
    }
}

// jl_sketch.rs — sketch 30522 -> 2048 for sparse slots
struct JlSketch {
    matrix: Array2<f32>,  // 2048 x 30522
    seed: u64,
}

impl JlSketch {
    fn new(seed: u64, target_dim: usize, source_dim: usize) -> Self {
        let mut rng = ChaChaRng::seed_from_u64(seed);
        // Gaussian(0, 1/sqrt(target_dim)) entries
        let entries = (0..target_dim * source_dim)
            .map(|_| rng.gen::<f32>().normal(0.0, 1.0 / (target_dim as f32).sqrt()))
            .collect();
        let matrix = Array2::from_shape_vec((target_dim, source_dim), entries).unwrap();
        Self { matrix, seed }
    }

    fn apply(&self, dense: &Array1<f32>) -> Array1<f32> {
        self.matrix.dot(dense)
    }
}
```

### 3.4 Testing

- Unit: panel_vectors with synthetic fingerprints; assert expected dims and finiteness
- Integration: end-to-end build on a 5-memory corpus; verify .npz round-trips through Python

---

## 4 · Python scripts — `scripts/exp2/`

### 4.1 `prepare_pca.py`

```
python scripts/exp2/prepare_pca.py \
    --corpus exp2_corpus_n1000_2026-04-30.npz \
    --split-seed 137 \
    --out exp2_pca_n1000_2026-04-30.npz
```

For each of the 13 panel slots:
- Take 20 % PCA partition per split_seed
- Fit `sklearn.decomposition.PCA(n_components=256)` (or 128 for E12)
- Also fit PCA to 16D for KSG
- Serialize all 13 × 2 = 26 PCA matrices to a single `.npz`

### 4.2 `run_controls.py`

```
python scripts/exp2/run_controls.py \
    --corpus exp2_corpus_n1000_2026-04-30.npz \
    --pca exp2_pca_n1000_2026-04-30.npz \
    --seeds 0 1 2 3 4 \
    --out exp2_artifacts/results/controls_<date>.json
```

Runs the three controls (E1-copy, random-proj, zero-MI) with MINE + InfoNCE + KSG. If any fails the pass criteria from [05_methodology.md](05_methodology.md) §2, exits non-zero and writes a `control_failure_<date>.md` file.

### 4.3 `run_mine_pair.py` / `run_infonce_pair.py` / `run_ksg_pair.py`

Single-pair workers. Each accepts `(j, k, seed, n)` and writes one JSONL row.

```
python scripts/exp2/run_mine_pair.py \
    --corpus exp2_corpus_n1000_<date>.npz \
    --pca exp2_pca_n1000_<date>.npz \
    --pair j=0,k=7 \
    --seed 0 \
    --steps 2000 \
    --batch 512 \
    --lr 1e-4 \
    --ema 0.01 \
    --out exp2_artifacts/results/exp2_results_mine_n1000.jsonl
```

### 4.4 `run_all_pairs.py`

Orchestrator. Sweeps all (j, k) × seed × n combinations.

```
python scripts/exp2/run_all_pairs.py \
    --corpus exp2_corpus_n1000_<date>.npz \
    --pca exp2_pca_n1000_<date>.npz \
    --estimator mine     # or infonce
    --parallel 4         # GPU parallelism; depends on VRAM
    --resume             # skip pairs already in the JSONL
    --out-dir exp2_artifacts/results/
```

Writes incremental JSONL. On re-run with `--resume`, skips completed rows. Can be killed and re-started safely.

### 4.5 `bootstrap_ci.py`

```
python scripts/exp2/bootstrap_ci.py \
    --results exp2_artifacts/results/exp2_results_mine_n1000.jsonl \
    --B 1000 \
    --out exp2_artifacts/results/exp2_mine_ci_n1000.json
```

### 4.6 `aggregate_n_eff.py`

```
python scripts/exp2/aggregate_n_eff.py \
    --results-mine exp2_artifacts/results/exp2_results_mine_n1000.jsonl \
    --results-infonce exp2_artifacts/results/exp2_results_infonce_n1000.jsonl \
    --results-ksg exp2_artifacts/results/exp2_results_ksg_n1000.jsonl \
    --corpus exp2_corpus_n1000_<date>.npz \
    --pca exp2_pca_n1000_<date>.npz \
    --n 1000 \
    --out exp2_artifacts/results/exp2_n_eff_n1000.json
```

Computes all four $N_\text{eff}$ aggregations per [06_analysis_and_decision_rules.md](06_analysis_and_decision_rules.md) §2, with bootstrap CIs propagated through the formula.

### 4.7 `generate_figures.py` and `generate_report.py`

Deterministic rendering of the heatmap, per-pair table, convergence curves, and report markdown / PDF per [07_reporting_template.md](07_reporting_template.md).

---

## 5 · Library internals (`scripts/exp2/lib/`)

### 5.1 `mine.py`

```python
class MineCritic(nn.Module):
    def __init__(self, dim_x=256, dim_y=256, hidden=256):
        super().__init__()
        self.net = nn.Sequential(
            nn.Linear(dim_x + dim_y, hidden), nn.ReLU(),
            nn.Linear(hidden, hidden), nn.ReLU(),
            nn.Linear(hidden, 1)
        )

    def forward(self, x, y):
        return self.net(torch.cat([x, y], dim=-1))


def train_mine(x_train, y_train, seed, steps=2000, batch=512, lr=1e-4,
               ema_alpha=0.01, device='cuda'):
    torch.manual_seed(seed)
    critic = MineCritic(dim_x=x_train.shape[1], dim_y=y_train.shape[1]).to(device)
    opt = torch.optim.Adam(critic.parameters(), lr=lr, betas=(0.5, 0.999))

    ema_logsumexp = None  # gradient bias correction
    losses = []

    for step in range(steps):
        idx = torch.randperm(len(x_train))[:batch]
        x_pos = x_train[idx]
        y_pos = y_train[idx]
        y_neg = y_train[torch.randperm(len(y_train))[:batch]]

        t_pos = critic(x_pos, y_pos).mean()
        t_neg = critic(x_pos, y_neg)
        logsumexp_neg = torch.logsumexp(t_neg, dim=0) - np.log(batch)

        # EMA on denominator for gradient bias correction
        if ema_logsumexp is None:
            ema_logsumexp = logsumexp_neg.detach()
        else:
            ema_logsumexp = (1 - ema_alpha) * ema_logsumexp + ema_alpha * logsumexp_neg.detach()

        # Negative MI with EMA correction
        loss = -(t_pos - logsumexp_neg * (logsumexp_neg.detach() / ema_logsumexp))
        opt.zero_grad(); loss.backward(); opt.step()
        losses.append(-loss.item())  # record the MI estimate

    return critic, losses


def eval_mine(critic, x_test, y_test, device='cuda'):
    critic.eval()
    with torch.no_grad():
        t_pos = critic(x_test, y_test).mean()
        # sample all (n_test)^2 negatives
        n = len(x_test)
        x_rep = x_test.unsqueeze(1).expand(n, n, -1).reshape(n*n, -1)
        y_rep = y_test.unsqueeze(0).expand(n, n, -1).reshape(n*n, -1)
        t_neg = critic(x_rep, y_rep)
        logsumexp = torch.logsumexp(t_neg, dim=0) - np.log(n*n)
        return (t_pos - logsumexp).item() / np.log(2)  # in bits
```

### 5.2 `infonce.py`

```python
def train_infonce(x_train, y_train, seed, steps=2000, batch=256, tau=0.1, lr=1e-4, device='cuda'):
    torch.manual_seed(seed)
    W_x = nn.Linear(x_train.shape[1], 64, bias=False).to(device)
    W_y = nn.Linear(y_train.shape[1], 64, bias=False).to(device)
    opt = torch.optim.Adam(list(W_x.parameters()) + list(W_y.parameters()), lr=lr)

    for step in range(steps):
        idx = torch.randperm(len(x_train))[:batch]
        x = W_x(x_train[idx])
        y = W_y(y_train[idx])
        logits = (x @ y.T) / tau
        labels = torch.arange(batch, device=device)
        loss = (F.cross_entropy(logits, labels) + F.cross_entropy(logits.T, labels)) / 2
        opt.zero_grad(); loss.backward(); opt.step()

    return W_x, W_y


def eval_infonce(W_x, W_y, x_test, y_test, tau=0.1):
    with torch.no_grad():
        K = len(x_test)
        x = W_x(x_test); y = W_y(y_test)
        logits = (x @ y.T) / tau
        log_prob_pos = torch.diag(logits) - torch.logsumexp(logits, dim=1)
        hat_I = np.log(K) + log_prob_pos.mean().item()
        return hat_I / np.log(2)  # in bits
```

### 5.3 `ksg.py`

Standard KSG Algorithm 1 using `scipy.spatial.cKDTree`. Reference implementation from `scikit-learn`'s `mutual_info_regression` trusted; lightly wrapped to accept our pair input.

### 5.4 `block_entropy.py`

Paninski (2003) MLE with Chao-Shen corrector for the joint entropy $H(\Phi)$ used in $N_\text{eff,joint}$. 13 × 16 = 208-dim joint; uses a kNN entropy estimator since direct histogram is infeasible.

### 5.5 `seeds.py`

Single place where every deterministic RNG is named and seeded.

```python
SEEDS = {
    'stratify_seed': 42,
    'sketch_seed': 2026,
    'split_seed': 137,
    'mine_seeds': [0, 1, 2, 3, 4],
    'infonce_seeds': [0, 1, 2, 3, 4],
    'bootstrap_seed': 999,
    'random_proj_control_seeds': [9001, 9002],
}
```

Importing anywhere else is a lint error.

---

## 6 · Testing strategy

### 6.1 Synthetic-MI sanity tests

```python
# test_mine_on_synthetic.py
def test_gaussian_known_mi():
    # X, Y ~ N(0, Sigma) with known rho
    rho = 0.8
    n = 10000
    cov = [[1, rho], [rho, 1]]
    data = np.random.multivariate_normal([0, 0], cov, n)
    x, y = data[:, :1], data[:, 1:]
    # True MI = -0.5 * log(1 - rho^2)
    true_mi = -0.5 * np.log(1 - rho**2) / np.log(2)
    hat_mi = train_mine_and_eval(x, y, seed=0)
    assert abs(hat_mi - true_mi) < 0.5  # bits
```

Same test shape for InfoNCE and KSG with appropriate lower-bound / bias tolerances.

### 6.2 Regression tests

After any code change, the three controls from [05_methodology.md](05_methodology.md) §2 must re-pass on the frozen `n=100` pilot corpus before the change is merged.

### 6.3 Cross-check

`test_estimator_consistency.py`: on the synthetic Gaussian with $\rho = 0.7$, all three estimators must agree within 0.5 bits.

---

## 7 · Build integration

New Rust crate added to workspace:

```toml
# /home/cabdru/contextgraph/Cargo.toml (edit)
[workspace]
members = [
    # ... existing members ...
    "crates/context-graph-exp2",
]
```

`cargo build --release -p context-graph-exp2` is the build command. The binary is placed at `target/release/context-graph-exp2`.

Python environment:
```
# requirements-exp2.txt (new file at repo root)
numpy>=1.26
scipy>=1.12
scikit-learn>=1.4
torch>=2.2
tqdm
matplotlib
```

Create a dedicated venv: `python3 -m venv .venv-exp2 && source .venv-exp2/bin/activate && pip install -r requirements-exp2.txt`.

---

## 8 · Anti-pattern check

Things NOT to do in the implementation:
- Do not add a dependency not in the freeze list (even if it would be faster).
- Do not use `rand::thread_rng()` — always `ChaChaRng::seed_from_u64(<named seed>)`.
- Do not use the production daemon's search path — EXP-2 reads fingerprints directly via the memory list + fingerprint tools, not via retrieval.
- Do not mix precision — f32 throughout. No f16 / bf16 in the estimator path.
- Do not use `torch.cuda.manual_seed_all()` without also calling `torch.backends.cudnn.deterministic = True` + `benchmark = False`.
- Do not share a single torch tensor across processes — serialize via numpy and reload.

---

## 9 · Compute budgeting

| Phase | Wall-clock | GPU util | VRAM use |
|-------|-----------|----------|----------|
| Corpus build $n=1000$ | ~2 hr | sequential embedder calls | 12 GB (daemon) |
| PCA fit | ~5 min (CPU) | 0 | 0 |
| Controls | ~30 min | 40 % | 8 GB |
| MINE + InfoNCE sweep $n=1000$ | ~24 hr | 80–90 % | 14 GB |
| MINE + InfoNCE sweep $n=500$ | ~8 hr | " | " |
| MINE + InfoNCE sweep $n=100$ | ~1 hr | " | " |
| KSG 12 pairs × 3 $n$ | ~30 min (CPU) | 0 | 0 |
| Bootstrap | ~10 min (CPU) | 0 | 0 |
| Aggregation + figures | ~5 min | 0 | 0 |

**Total budget**: ~36 GPU-hours for one complete run. Plus daemon overhead that stays on throughout. One RTX 5090. Fits in the 14-day window with ~2× margin for re-runs.
