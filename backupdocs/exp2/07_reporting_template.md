# EXP-2 Reporting Template

**The exact shape of the final report.** Every EXP-2 deliverable follows this skeleton. No section is omitted. The format is the compliance gate — Steve reviews against this template before anything external.

---

## 1 · Report header (always first)

```
# EXP-2 Report — <date>

- Preregistration: v0.exp2.preregistered @ <sha>
- Corpus (primary): exp2_corpus_n1000_<date>.npz  sha256:<hash>
- PCA projections: exp2_pca_n1000_<date>.npz  sha256:<hash>
- Software: context-graph @ <commit sha>
- Compute: RTX 5090 (Blackwell), CUDA 13.x, driver <version>
- Wall time: <hrs>
- Protocol amendments: <count> (see 01_preregistration.md Appendix A)
- Sign-offs: Chris Royse <date>, Steve Abbey <date>
```

---

## 2 · Headline (always second)

A single paragraph. Two numbers. No hedges about which is "better." Constructive-ceiling-plus-realized-floor.

Template:

> EXP-2 measures pairwise mutual information across the 13-embedder Context Graph panel on a production corpus of $n$ inputs using MINE (Belghazi 2018), InfoNCE (Oord 2018), and KSG (Kraskov 2004) in parallel. The constructive upper bound on supervisory signal count per input at $N = 13$ is $N + \binom{N}{2} = 91$. The realized floor, measured as $N_\text{eff,pair}$, is $\hat{N}_\text{eff}$ (95 % CI [$L$, $U$]). The counting identity holds as a structural bound; the measured floor is the operating number.

**Three example headline sentences, one per outcome**:

- **Strong** ($\hat{N}_\text{eff} = 73$, CI [71, 78]): "Measured $N_\text{eff,pair} = 73$ (95 % CI [71, 78]); 80 % of the constructive 91-signal ceiling is realized on the production distribution. Axiom 2 of the counting identity is approximately supported; the largest pair redundancy (E1–E8, same backbone) accounts for 18 % of total redundancy."

- **Moderate** ($\hat{N}_\text{eff} = 34$, CI [28, 41]): "Measured $N_\text{eff,pair} = 34$ (95 % CI [28, 41]); the production panel emits 34 effective supervisory signals per input versus a constructive 91-signal ceiling. Panel delivers 2.6× supervisory multiplication over single-encoder baseline; extensibility to $N > 13$ preserves a super-linear growth curve."

- **Weak** ($\hat{N}_\text{eff} = 10$, CI [6, 14]): "Measured $N_\text{eff,pair} = 10$ (95 % CI [6, 14]); the production panel exhibits substantial pairwise redundancy consistent with the Platonic Representation Hypothesis on learned SSL encoders. The 91-signal ceiling is a structural bound, not a realized value; 14 pairs account for 73 % of total redundancy. The thesis's extensibility claim — each new embedder contributes $O(N)$ new pair signals — is unaffected. Headline pitch reframes from absolute 91 to three-role composition + growth curve."

---

## 3 · Methodology summary (one page)

- Panel composition — table from [02_panel_specification.md §1](02_panel_specification.md#1--panel-table)
- Sampling — stratification, $n$, splits, seeds
- Three estimators with architectures, sample sizes, hyperparameters
- Controls — names and pass / fail status of each

---

## 4 · Primary results

### 4.1 $N_\text{eff}$ table (all four aggregations)

| Metric | Point estimate | 95 % bootstrap CI | Interpretation |
|--------|----------------|-------------------|----------------|
| $N_\text{eff,pair}$ | | [L, U] | Pair-decomposed; headline |
| $N_\text{eff,joint}$ | | [L, U] | Block-entropy; tighter bound |
| $N_\text{eff,min}$ | | [L, U] | Single-worst-pair conservative |
| $N_\text{eff,sum}$ | | [L, U] | Additive; not a bound |

### 4.2 Pairwise MI matrix (heatmap)

$13 \times 13$ heatmap. Diagonal: per-embedder entropy $H(\phi_j)$. Off-diagonal: pairwise MI (symmetric across diagonal). Cell annotations: median of 5 seeds. Color scale: 0 to max(diagonal).

Published as `mi_matrix_heatmap_n1000.pdf` and as an ASCII table in the markdown version.

### 4.3 Per-pair numeric table

All 78 pairs with their full stats. Sorted descending by MINE median.

| Pair | MINE median (CI) | MINE seed std | InfoNCE median | KSG (when run) | Flags |
|------|------------------|---------------|----------------|---------------|-------|
| (E1, E8) | 6.9 (6.5, 7.3) | 0.18 | 5.55 (SAT) | 6.7 | NCE_SAT |
| (E2, E3) | 5.2 (4.9, 5.5) | 0.21 | 5.0 | 5.1 | OK |
| ... | | | | | |

Color convention (for PDF):
- Green border: `OK`, median $\ge$ some meaningful threshold
- Yellow border: `DISAGREE_MILD` or `NCE_SAT`
- Red border: `DISAGREE_STRONG` or `MINE_NONCONV`

---

## 5 · Secondary metrics

### 5.1 Cosine similarity matrix

Same 13 × 13 shape as the MI heatmap. Sanity-check panel; shows which pairs "feel" close without the information-theoretic framing.

### 5.2 Linear CKA matrix

Same shape. This is the metric Huh et al. 2024 use for PRH. Reported to allow direct comparison with the PRH literature.

### 5.3 Redundancy statistics

- Total redundancy $R$ (bits)
- Redundancy fraction $\rho \in [0, 1-1/N]$
- Effective spectral dimensionality (95 % variance)
- Near-reparameterization clusters (connected components at MI/H > 0.8 threshold)

---

## 6 · Controls section (mandatory)

Table of control results with pass / fail status:

| Control | Expected | Measured | Pass? |
|---------|----------|----------|-------|
| E1-copy | $\hat{I} \approx H(\phi_{E1})$ | | ✓ / ✗ |
| Random-proj | $\hat{I} \ge 0.95 H(\phi_{E1})$ | | ✓ / ✗ |
| Zero-MI | $\hat{I} \in [-0.1, 0.3]$ | | ✓ / ✗ |

If any control fails, the report front matter flags the run as **unvalidated** and the root-cause discussion follows in this section.

---

## 7 · Sensitivity analyses

Three rows of (corpus, $n$, $N_\text{eff,pair}$, CI):

| Corpus | $n$ | $N_\text{eff,pair}$ | 95 % CI | Notes |
|--------|-----|---------------------|---------|-------|
| Production | 1000 | | [L, U] | headline |
| Wikipedia-5k | 1000 | | [L, U] | external-distribution check |
| Code-5k | 1000 | | [L, U] | E7-heavy stress test |
| Shakespeare-2k | 1000 | | [L, U] | stylistic-narrow check |

If the sensitivity spread is > 15 $N_\text{eff}$ points, a brief discussion is required in this section.

---

## 8 · Discussion (bounded format)

Three sub-sections, each 1–2 paragraphs. No more.

### 8.1 What the result rules in

Which of H0 / H1 / H2 (from [01_preregistration.md](01_preregistration.md) §1) does the result best support? Cite the specific pair statistics that drive the inference.

### 8.2 What the result rules out

Which of H0 / H1 / H2 is falsified or made implausible? With what confidence?

### 8.3 Limitations

- E12 pooling loss (structural; mean-pool discards token-level signal)
- JL sketch bias on E6 / E13 (mitigable at cost of compute)
- Corpus size $M$ at freeze may be small
- PRH-like convergence could be an artifact of shared Wikipedia-scale pretraining
- Task-relevant MI is not measured (see [14_followup_experiments.md](14_followup_experiments.md))

---

## 9 · What the number means for the pitch

Explicit paragraph that translates the technical result into commercial language. Template for the three outcomes:

### Strong
"The 91-signal counting identity is empirically supported at $\hat{N}_\text{eff}$ effective signals per input. This is the realized floor under the pre-registered protocol; reproducibility artifacts are attached."

### Moderate
"The 13-embedder panel emits $\hat{N}_\text{eff}$ distinct supervisory signals per input, measured as effective pair-independence over the production distribution. For $N = 13$ this is a $\hat{N}_\text{eff} \times$ multiplier over single-encoder baselines. The extensibility of the architecture — $O(N)$ new pair signals per new embedder — preserves a super-linear growth curve."

### Weak
"The measured effective signal count is $\hat{N}_\text{eff}$, substantially below the constructive ceiling of 91. Pair redundancy is dominated by shared-backbone embedders (E1, E8, E10 in the e5 family) and twin-sparse embedders (E6, E13). The structural advantage of the panel is thus the three-role *composition* (retriever, labeller, $G_\tau$) operating over a lower-dim substrate than the constructive count suggests. The counting identity remains valid as a ceiling; the $O(N)$ growth curve is unchanged."

---

## 10 · Appendices

### A — Full seed sweep
Every pair × seed × estimator × $n$ combination. Machine-readable JSONL.

### B — Convergence curves
Per-pair MINE training-loss trajectories. Overplotted for all 5 seeds.

### C — Flag inventory
Pairs carrying each flag, with short per-pair commentary.

### D — Reproduction recipe
Minimal command sequence to reproduce the run from a clean checkout and the committed corpus / PCA artifacts.

### E — Amendment log
Copy of [01_preregistration.md](01_preregistration.md) Appendix A.

---

## 11 · Executive-facing appendix (separate one-pager)

A one-page brief for Chris / Steve / counsel / Perez memo inclusion. Only headline + decision + narrative paragraph. All technical detail lives in the main report.

---

## 12 · Style rules (compliance gate binding)

Language rules enforced by Steve's review:

**Avoid (prohibited or misleading)**:
- "solves the data wall" / "solved"
- "state of the art"
- "beats RLHF"
- "refutes collapse" / "solves collapse"
- "cross-encoder verified" (unless EXP-1 is also complete)
- "AGI-complete"
- "Axiom 2 is proven" (we do not prove axioms; we provide evidence bearing on them)
- "We have 91 independent signals" (without the realized-floor qualifier)
- "Up to 91" (without either the constructive-bound framing or the $N_\text{eff}$ number)

**Prefer (calibrated)**:
- "The constructive ceiling is 91; the measured realized floor is $\hat{N}_\text{eff}$"
- "Approximately supported" / "consistent with" rather than "proves"
- "Panel-level" rather than "embedder-level" when discussing composition
- "$O(N)$ growth curve preserved" rather than "scales forever"
- "On the production distribution" rather than "universally"
- "Measured under the pre-registered protocol" as a prefix to any cited number

**Required scope guards**:
- Every cited $N_\text{eff}$ is accompanied by (a) $n$, (b) CI, (c) corpus identifier.
- Every constructive "91" is accompanied by the realized-floor $N_\text{eff}$ in the same paragraph.
- The PRH limitation is acknowledged once in any discussion that lasts more than one paragraph.

---

## 13 · Distribution rules

| Audience | Version | Timing |
|----------|---------|--------|
| Chris, Steve (internal) | Full markdown + PDF | Day 20 (2026-05-13) |
| Perez (if memo revision) | Executive-facing one-pager + headline + one sentence linking to full report | Day 22+ post-internal-review |
| Patent counsel | Full markdown (for patent claim support) | Day 21 (2026-05-14) |
| NeurIPS supplementary (if accepted) | Full markdown + reproducibility artifacts | At acceptance |
| Public (HuggingFace / arXiv supplementary) | Full markdown + anonymized corpus | After patent priority-date lock + Chris decision |

**PDF on reply only.** First touches to any lab or external party include the executive-facing one-pager in the body, with a link to the full report. Full PDF is attached only in reply after the other party has acknowledged the first touch.
