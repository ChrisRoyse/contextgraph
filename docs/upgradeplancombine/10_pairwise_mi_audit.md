# 10 — Pairwise Mutual-Information Audit (KSG / MINE)

This document specifies the `audit-pairwise-mi` subcommand's
estimator design, sample-size discipline, output format, and
acceptance criteria for the empirical $N_{\text{eff}}$ that replaces
the upper-bound "up to $N_d + \binom{N_d}{2}$" headline of
`03_signal_yield_math.md`.

## 1. Why this audit matters

PAPER.md §3.1, Axiom 2 (approximate independence) and §8 L4 are
hedged. The $\binom{N_d}{2}$ pairwise term in the MC counting identity
is informative only to the extent that pairs of instruments carry
*non-redundant* information about the input. When two instruments
collapse to a near-linear reparameterisation, their pair contributes
near-zero additional signal. The audit is the empirical
falsification test PAPER.md leaves open.

## 2. Estimator selection

Two mutual-information estimators are in scope. The first is the
default; the second is gated behind `--estimator mine`.

### 2.1 KSG (Kraskov-Stögbauer-Grassberger), default

KSG is a non-parametric, non-trainable nearest-neighbour estimator.
Properties that fit this audit:

- **No model fitting** — no LoRA, no adapter, no extra parameters.
- **Deterministic** under fixed input ordering.
- **Bias decreases with $k$** (number of nearest neighbours).
- **Sample efficient** — operates on N=100..1000 events without need
  for thousands.

The implementation uses the standard KSG-1 formulation:

$$
\hat I_{\text{KSG}}(X; Y) = \psi(k) - \langle \psi(n_x + 1) +
\psi(n_y + 1) \rangle + \psi(N)
$$

where $\psi$ is the digamma function, $N$ is the sample size, and
$n_x, n_y$ are the marginal neighbour counts inside the joint
$\epsilon$-ball at the $k$-th nearest neighbour. We default $k = 5$.

Estimator output for a pair $(j, k)$: a scalar in nats.

### 2.2 MINE (Mutual Information Neural Estimation), opt-in

MINE trains a small neural net (Donsker-Varadhan-bound estimator) and
is optionally invoked via `--estimator mine`. Properties:

- **Higher variance** than KSG at the small sample sizes this audit
  uses.
- **Trainable** — adds a model to the audit chain. Requires a
  separate frozen-weights record once training completes (the
  estimator weights are NOT part of the bundle's frozen panel; they
  exist only to estimate MI).
- **Costs** several minutes per pair on the documented host.

MINE is included as a cross-check option for cases where KSG's
$k$-nearest-neighbour assumption is suspect (very high-$d$
embeddings, e.g.\ $d > 256$). For the in-scope domains
($d_m \le 64$ everywhere), KSG suffices.

## 3. Sample-size discipline

| Parameter | Default | Rationale |
| --- | ---: | --- |
| `sample-size` (events) | 1{,}000 | Matches paper-small bundle size; full-population audit at this scale. |
| `ksg_k` | 5 | Standard choice; tradeoff between bias (lower $k$) and variance (higher $k$). |
| `seed` (bootstrap subsampling) | 20260501 | Fixed for cross-run determinism. |
| `bootstrap_iters` | 1{,}000 | Enough for percentile CI on the per-pair MI. |

For smoke-bundle audits (120-event bundles), `sample-size` defaults
to all events ($n_{\text{audit}} = 120$). For paper-small bundles
($n = 1{,}000$), full-population is the default.

## 4. Output format

The audit emits four artefacts under `--output-dir`:

### 4.1 `pairwise_mi_matrix.csv`

```
instrument_j,instrument_k,instrument_j_name,instrument_k_name,d_j,d_k,n_audit,estimator,ksg_k,mi_nats,mi_bits,mi_normalised,bootstrap_low,bootstrap_high
1,2,occupation_cluster_onehot,job_zone_scalar,4,1,1000,ksg,5,0.4231,0.6105,0.1832,0.395,0.451
1,3,occupation_cluster_onehot,skill_vector,4,4,1000,ksg,5,1.0421,1.5034,0.4513,0.998,1.087
...
```

`mi_normalised = mi / min(H_j, H_k)` where $H$ is per-instrument
entropy estimated by KSG. A value near 1 indicates the pair is
highly redundant; near 0 indicates near-independence.

### 4.2 `table_pairwise_mi_summary.csv` (paper-table)

Aggregates the matrix per-domain:

```
domain,n_panel_slots,n_pairs,mean_mi_nats,median_mi_nats,p10_mi_nats,p90_mi_nats,n_redundant_pairs_loose,n_redundant_pairs_strict,n_eff_loose,n_eff_strict
career_taxonomy_a1,10,45,0.412,0.380,0.121,0.842,3,1,9.8,9.95
gridworld_5x5,3,3,0.331,0.330,0.290,0.371,0,0,3.0,3.0
counter_world,2,1,0.215,0.215,0.215,0.215,0,0,2.0,2.0
```

**$N_{\text{eff}}$ definitions:**
- `n_eff_loose`: $N_d - \sum_{(j,k)} \mathbb{1}[\text{mi\_normalised} \ge 0.7] \cdot 0.5$
- `n_eff_strict`: $N_d - \sum_{(j,k)} \mathbb{1}[\text{mi\_normalised} \ge 0.9] \cdot 0.5$

Two thresholds reported because the "redundancy" cutoff is a judgement
call — and we want both to be visible in the paper update.

### 4.3 `pairwise_mi_heatmap.svg`

$N_d \times N_d$ matrix, lower-triangular filled with normalised MI,
diagonal shaded. Includes a colour-scale legend and a title with the
domain id and audit timestamp. Used as the v1.1 paper's pairwise-MI
plot (see `12_paper_tables_and_plots.md`).

### 4.4 `pairwise_mi_audit.json` (machine-readable)

```json
{
  "audit_id": "...",
  "audit_uuid": "...",
  "domain_pack_id": "...",
  "n_audit_events": 1000,
  "estimator": "ksg",
  "ksg_k": 5,
  "seed": 20260501,
  "bootstrap_iters": 1000,
  "started_at_unix_ms": ...,
  "ended_at_unix_ms": ...,
  "wall_time_seconds": 92.3,
  "pairwise_mi_matrix_path": "pairwise_mi_matrix.csv",
  "summary_path": "table_pairwise_mi_summary.csv",
  "heatmap_path": "pairwise_mi_heatmap.svg",
  "n_eff_loose": 9.8,
  "n_eff_strict": 9.95,
  "audit_status": "passed"
}
```

## 5. Acceptance criteria for "approximately independent"

The empirical Axiom-2 verdict per domain is one of three:

- **STRONG (passes "approximate independence")**:
  `n_eff_strict / N_d ≥ 0.95`. The constructive upper bound
  $N_d + \binom{N_d}{2}$ is a tight reading of the panel's
  signal-yield capacity.
- **WEAK (passes "approximate independence" with hedge)**:
  `n_eff_strict / N_d ∈ [0.85, 0.95)`. The constructive bound is an
  *upper* bound; the effective signal yield is meaningfully lower
  than $N_d^2 + N_d + 4$ but still strictly exceeds $2N_d + 4$
  (the no-pairwise count).
- **FAILS (Axiom 2 violated for this panel)**:
  `n_eff_strict / N_d < 0.85`. The pairwise term carries little
  net information over per-instrument readings; the paper should
  report only the per-instrument count $2N_d + 4$ and explicitly
  retract the pairwise multiplier for this panel.

The headline paper number tracks the verdict. For domains where the
audit returns STRONG, the paper claims `up to S_1(d) signals per
event`; for WEAK, `at least 2N_d + 4 + ((1 - eff_loss) * C(N_d, 2))
signals per event`; for FAILS, `at most 2N_d + 4 signals per event`.

## 6. The first audit's expected outcome

For career\_taxonomy A1's 10-instrument panel:

- 4 of the 10 instruments are categorical one-hots
  (`occupation_cluster`, `wage_band`, `mobility_label`,
  `next_occupation_cluster`); pairs among these have low effective
  MI (one-hot codes are near-orthogonal by construction).
- 4 are scalar instruments (`job_zone`, `experience_years`,
  `action_effort`, `step_index`); pairs are correlated only through
  the underlying state distribution.
- 2 are dense vectors (`skill_vector`, `next_skill_vector`); the
  pair `(skill_vector, next_skill_vector)` is highly correlated
  (state evolves smoothly), high MI; redundancy expected.

Predicted verdict: **WEAK** (1–2 redundant pairs out of 45). The
$n_{\text{eff,strict}} \approx 9.5$–$9.9$.

For gridworld\_5x5 ($N_d = 3$ — position one-hot, action one-hot,
step scalar): predicted **STRONG**.

For counter\_world ($N_d = 2$): predicted **STRONG**.

The audit *measures* these expectations; the paper update reports the
measurement, not the prediction.

## 7. Performance budget

| Step | Estimated wall time on RTX 5090 host |
| --- | ---: |
| Read 1{,}000 events × 10 instruments from RocksDB | 6 s |
| Compute 45 pairwise KSG with $k=5$ | 60 s |
| Bootstrap 1{,}000 iters per pair | 12 s |
| Write CSV + SVG heatmap | 2 s |
| **Total per career-A1 audit** | $\sim$80 s |

Career-A0 (8 instruments): 28 pairs, $\sim 50$ s.
Career-A4 (12 instruments): 66 pairs, $\sim 110$ s.

The full domain audit (career\_taxonomy across A0..A5) costs about
8 minutes total — comfortably inside the release-D wall time budget.

## 8. Edge cases (typed-error catalog)

| Edge | Trigger | Typed error |
| --- | --- | --- |
| Sample size below floor | `--sample-size < 50` | `MI_AUDIT_SAMPLE_SIZE_TOO_SMALL` |
| All events have identical instrument readings | KSG returns degenerate MI | `MI_AUDIT_DEGENERATE_INPUT` |
| Bootstrap CI empty (collapsed at point estimate) | All resamples agree | `MI_AUDIT_BOOTSTRAP_DEGENERATE` |
| MINE training did not converge | `--estimator mine` and loss did not decrease | `MI_AUDIT_MINE_NONCONVERGENT` |
| Output dir exists | re-run on same `--output-dir` | `MI_AUDIT_OUTPUT_DIR_EXISTS` |
| Instrument hash mid-audit drift | registry changed during audit | `MI_AUDIT_INSTRUMENT_HASH_DRIFT` |

## 9. The audit as part of the release ladder

The `release-d` gate (release-c successor) adds a mandatory
audit-pairwise-mi step after the multi-seed aggregate completes.
The release reference (`reference/reference_release_v2.0.0.json`)
gains:

```json
"pairwise_mi_audit": {
  "domain": "career_taxonomy",
  "n_audit_events": 1000,
  "estimator": "ksg",
  "ksg_k": 5,
  "seed": 20260501,
  "expected_n_eff_loose_min": 9.0,
  "expected_n_eff_strict_min": 8.5,
  "expected_verdict": "WEAK"
}
```

A release-D run that returns a verdict mismatching expected fails the
gate (typed error `RELEASE_REFERENCE_PAIRWISE_MI_VERDICT_DRIFT`).

## 10. What this audit does *not* do

- Does not measure higher-order MI (triplets, $N$-tuples).
- Does not measure cross-event temporal MI.
- Does not measure conditional MI given the action or the outcome.
- Does not propose an optimal panel selection algorithm.

These are deferred axes; the v2.0 release ships only the within-event
pairwise audit on the panel as declared in TOML. PAPER.md §9 names
these as future questions.

## 11. Linked output: the paper-update headline

When the audit lands, the v1.1 paper update (see
`16_paper_v1_1_outline.md`) replaces:

> "...up to 91 signals per event for the career\_taxonomy panel..."

with:

> "$n_{\text{eff,strict}} = X.X$ on a 1{,}000-event audit (verdict:
> WEAK / STRONG); the constructive upper bound is 91 signals per
> event and the measured effective count is approximately
> $S_1^{\text{eff}}$ signals per event."

The exact $X.X$ value is determined by the audit; it is the headline
new measurement.
