# 03 — Signal-Yield Math and the Meaning-Compression Ratio

This document formalises the per-event and per-bundle signal-yield
identity for the combined runtime, and defines the Meaning-Compression
(MC) ratio that becomes a paper-table column in
`12_paper_tables_and_plots.md`.

## 1. Notation

| Symbol | Meaning |
| --- | --- |
| $D$ | A bundle's raw event corpus, $\lvert D \rvert = n$. |
| $\Phi^{(d)} = \{\phi_m^{(d)}\}_{m=1}^{N_d}$ | The frozen instrument panel for domain $d$. |
| $N_d$ | Number of declared instruments in domain $d$ (panel slots). |
| $\rho_{j,k}$ | Pairwise interaction feature between instruments $j$ and $k$. |
| $K_d$ | Number of candidate actions enumerated by the planner for domain $d$ (default 5 for career, fewer for sanity domains). |
| $M_d \subseteq \{1, \dots, N_d\}$ | Subset of instruments active in the runtime guard $G_\tau$ for domain $d$. |
| $|S_m|$ | Size of the threshold-calibration set for modality $m$. |
| $E$ | Number of bundle-level edge cases per career bundle (= 11). |
| $A$ | Number of ablation runs per bundle (= 6 for career, fewer for the sanity domains). |

## 2. Per-event signal yield, decomposed

Every raw event $x_i \in D$ contributes deterministic-from-source
signals (Slice 1), tiny-model-output signals paired with observed
targets (Slice 2), and verifier-readback signals (Slice 3). The split
is identical to the v1 paper §7 anatomy; the upgrade *adds rows* to
each slice.

### 2.1 Slice 1 — deterministic, source-anchored

| Source | Per-event yield |
| --- | ---: |
| Per-instrument readings on $s_t$ | $N_d$ |
| Per-instrument readings on $s_{t+1}$ (next-state target) | $N_d$ |
| Pairwise interaction features on $s_t$ (new) | $\binom{N_d}{2}$ |
| Pairwise interaction features on $s_{t+1}$ (new) | $\binom{N_d}{2}$ |
| Action vector | 1 |
| Outcome (mobility label / next cluster / etc.) | 1 |
| Trajectory link, transition link | 2 |

**Slice 1 per-event subtotal:**
$$
S_1(d) \;=\; 2N_d + 2\binom{N_d}{2} + 4 \;=\; N_d^2 + N_d + 4.
$$

### 2.2 Slice 2 — model-output paired with deterministic observed targets

| Source | Per-event yield |
| --- | ---: |
| Predicted next-panel for $K_d$ candidate actions | $K_d \cdot N_d$ |
| Per-candidate cosine-to-centroid scores (new) | $K_d \cdot \lvert M_d \rvert$ |
| Per-candidate utility scores | $K_d$ |
| Predicted-vs-observed delta when an actual outcome is recorded | $N_d$ |
| Surprise event (conditional, encoded as 0 / 1 plus delta) | $\le 2$ |

**Slice 2 per-event subtotal:** ranges with $K_d$ and $\lvert M_d \rvert$;
for career\_taxonomy with $K_d = 5$, $\lvert M_d \rvert = N_d = 10$:
$$
S_2(\text{career-A1}) \;\le\; 5\cdot 10 + 5\cdot 10 + 5 + 10 + 2 \;=\; 117 / \text{plan-step}.
$$
Note Slice 2 yields are per *plan*, not per event: per career bundle,
6 predictions and 1 plan trace today, expected $\le 12$ predictions and
$\le 2$ plan traces under combined runtime configurations
(`05_pairwise_features_design.md` §5).

### 2.3 Slice 3 — independent verifier readbacks

| Source | Per-bundle yield (not per-event) |
| --- | ---: |
| `dj_audit_log` rows | $\sim 3{,}019$ for 1K career bundle |
| `dj_verification_runs` (one per domain) | 3 in a research-smoke bundle |
| Edge-case rows (career bundle) | $E = 11$ |
| Ablation rows (career bundle) | $A = 6$ |
| FSV-9 / 10 / 11 / 12 outputs (new) | 4 evidence rows + their decoded contents |
| Constellation integrity row per constellation (new) | $\lvert C_d \rvert$ where $\lvert C_d \rvert$ is the set of constellations declared by domain $d$ |
| Threshold-calibration row per modality per subject per domain (new) | $\sum_{(d, \text{subject}) \in C_d} \lvert M_{d,\text{subject}} \rvert$ |

Slice 3 is bundle-level, not per-event.

## 3. Per-bundle aggregate

For a bundle with $n$ events on domain $d$:

$$
\boxed{\;
\lvert D'_{\text{combined}} \rvert
\;=\; n \cdot S_1(d) + S_2^{\text{plan}}(d) + S_3^{\text{bundle}}(d)
\;}
$$

The Slice-1 term grows linearly in $n$; Slices 2 and 3 are essentially
fixed per bundle (Slice 2 depends on plan / prediction call count,
Slice 3 depends on edge / ablation / FSV catalog size).

### 3.1 Career-taxonomy A1 paper-small (n = 1{,}000, $N_d = 10$)

$$
S_1(\text{career-A1}) = 10^2 + 10 + 4 = 114.
$$
Per-event Slice-1 yield = **114** (vs current $\sim 23$); per-1K-event
bundle = **114{,}000 Slice-1 signals**.

Including Slice 2 (12 plan steps, $\sim$117 / step ⇒ $\le 1{,}404$
signals) and Slice 3 (audit log + edges + ablations + FSV ⇒
$\sim 3{,}060$ signals), the combined per-bundle total is
$\approx \mathbf{118{,}500}$ structured signals.

### 3.2 Counter\_world ($n = 60$, $N_d = 2$)

$$
S_1(\text{counter}) = 2^2 + 2 + 4 = 10.
$$
Per-event Slice-1 yield = **10**; per-bundle = **600 Slice-1 signals**.
Total combined yield $\approx 1{,}100$.

### 3.3 Gridworld\_5x5 ($n = 8{,}000$, $N_d = 3$)

$$
S_1(\text{gridworld}) = 3^2 + 3 + 4 = 16.
$$
Per-event Slice-1 yield = **16**; per-bundle = **128{,}000 Slice-1
signals**. Total combined yield $\approx 130{,}000$.

## 4. The Meaning-Compression ratio

Following PAPER.md §3.2, define the per-input MC ratio as:

$$
\mathrm{MC}_d \;\triangleq\; \frac{S_1(d) + (S_2^{\text{plan}}(d) + S_3^{\text{bundle}}(d)) / n}{1}
$$

For large $n$, the MC ratio converges to $S_1(d)$ (Slice 1 dominates).
The reportable MC values per domain:

| Domain | $N_d$ | $S_1(d)$ | $\mathrm{MC}$ as $n \to \infty$ |
| --- | ---: | ---: | ---: |
| counter\_world | 2 | $4 + 2 + 4 = 10$ | 10 |
| gridworld\_5x5 | 3 | $9 + 3 + 4 = 16$ | 16 |
| career\_taxonomy A0 | 8 | $64 + 8 + 4 = 76$ | 76 |
| career\_taxonomy A1 | 10 | $100 + 10 + 4 = 114$ | 114 |
| career\_taxonomy A2 | 11 | $121 + 11 + 4 = 136$ | 136 |
| career\_taxonomy A4 | 12 | $144 + 12 + 4 = 160$ | 160 |

**This becomes the primary new paper-table column.** A new file
`paper_tables/table_mc_ratio.csv` reports per-domain $N_d$, $S_1$,
$\mathrm{MC}$, and the *measured* yield once Phase 4 lands (per-event
signal-yield audit-log accounting).

## 5. Comparison with the current bundle yield

| Domain | Current per-event Slice-1 | Combined per-event Slice-1 | Multiplier |
| --- | ---: | ---: | ---: |
| counter\_world | $\sim 6$ | 10 | 1.7$\times$ |
| gridworld\_5x5 | $\sim 8$ | 16 | 2.0$\times$ |
| career\_taxonomy A0 | $\sim 19$ | 76 | 4.0$\times$ |
| career\_taxonomy A1 | $\sim 23$ | 114 | 5.0$\times$ |
| career\_taxonomy A2 | $\sim 25$ | 136 | 5.4$\times$ |
| career\_taxonomy A4 | $\sim 27$ | 160 | 5.9$\times$ |

The career-A4 panel — the one with explicit pairwise-feature slots in
the existing system — sees the largest multiplier under the upgrade
because the *existing* A4 panel only encodes a small subset of pairs
explicitly (occupation-move-pair); the upgrade promotes *all*
$\binom{N}{2}$ pairs to first-class records.

## 6. The upper-bound caveat (Axiom 2)

The above counts assume *approximate* embedder independence
(PAPER.md §3.1, Axiom 2). When two instruments collapse to a near-
linear reparameterisation of each other, their pair contributes
near-zero additional information. The honest framing carried into the
paper update (`16_paper_v1_1_outline.md`):

- **Constructive upper bound:** $S_1(d) = N_d^2 + N_d + 4$ as written.
- **Effective signal count:** $S_1^{\text{eff}}(d) \le S_1(d)$ to be
  measured by the FSV-9 pairwise mutual-information audit
  (`10_pairwise_mi_audit.md`).
- **Reportable headline:** "*up to* $S_1(d)$ structured signals per
  event"; the measured $S_1^{\text{eff}}$ replaces the "up to" once
  Phase 5 lands.

## 7. The audit-log signal-yield instrumentation

Every persisted operation contributes a known quantum of signals.
`05_pairwise_features_design.md` §3 specifies the per-row yield. The
aggregator uses a fixed table (lookup keyed by audit-log
`operation_kind`) so that the MC ratio is reproducible from the audit
log alone, without re-decoding any other CF.

| Operation kind | Signal yield per row |
| --- | ---: |
| `ingest_event` | 1 |
| `run_adapter_success` | 1 (the normalised state) |
| `run_adapter_failure` | 0 (typed error; persisted but not a positive signal) |
| `materialize_panel` | $N_d$ |
| `materialize_pairwise` (new) | $\binom{N_d}{2}$ |
| `compile_transition` | 2 (link + delta) |
| `train_predictor` | 0 (training itself is not a Slice-1 signal; persisted as Slice-2 metadata via `dj_training_runs`) |
| `predict` | $N_d$ (predicted panel) + $\lvert M_d \rvert$ (cosine to centroid, new) |
| `plan` | $K_d \cdot (N_d + \lvert M_d \rvert + 1)$ |
| `record_surprise` | 1 |
| `verify_artifact_files` | 0 (Slice 3, contributes to FSV not MC) |

The `compute-mc-ratio` command sums these contributions weighted by the
audit log row counts and emits both per-stage and per-bundle totals.

## 8. Sanity check against existing bundles

The v1 paper-tables-reference for a 1{,}000-event career bundle
already records:

```
dj_raw_events               1004
dj_normalized_states        1000
dj_actions                  1005
dj_outcomes                 1000
dj_transitions              1000
dj_instrument_readings     10000
dj_latent_panels            1000
dj_dataset_shards              3
dj_predictions                 6
dj_plan_traces                 1
dj_guard_decisions             5
dj_audit_log                3019
```

The combined runtime adds, on the same bundle:

```
dj_pairwise_readings       45000   (1000 events × 45 pairs)
dj_constellations             10   (1 global + 4 cluster + 5 reserved)
dj_threshold_calibrations     10   (one per active modality per subject)
dj_guard_decisions             5   (extended schema, same row count)
dj_audit_log                3019   (extended schema with signal_yield, same row count)
```

Total CF row count rises from $\sim 19{,}050$ to $\sim 64{,}080$. Disk
footprint is reviewed in `07_storage_schema_changes.md`.

## 9. Why the MC ratio is the right paper-level metric

- It is *measurable* — every row of the audit log carries a known yield.
- It is *bundle-level* — comparable across domains, seeds, and releases.
- It is *forward-looking* — strengthens the Slice-1 share of the
  bundle, which is the lowest-collapse-hazard slice per the v1 paper §7
  collapse-hazard analysis.
- It is *honest* — the Axiom-2 hedge is preserved by the FSV-9
  audit instead of being papered over.

`12_paper_tables_and_plots.md` specifies the shape of the new
`table_mc_ratio.csv` and a plot of MC ratio vs.\ measured
$N_{\text{eff}}$ once the audit lands.
