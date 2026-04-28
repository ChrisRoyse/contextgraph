# EXP-2 Follow-Up Experiments

**What comes after EXP-2.** EXP-2 measures raw embedder-embedder redundancy. These follow-ups measure the parts of the counting identity that EXP-2 does not touch.

Runtime priority: run EXP-2 first; these come later, in order of importance to the commercial + scientific narrative.

---

## 1 · Task-Relevant MI (EXP-2B)

**Question**: for each labeled task $T$ (retrieval, classification, etc.), how much of the embedder output is *task-relevant* vs *task-irrelevant*?

Formally: $I(\phi_j(X); Y_T)$ and $I(\phi_j(X); Y_T | Y_{T'})$ for a set of task labels $\{Y_T\}$.

### 1.1 Why it matters
EXP-2 measures $I(\phi_j; \phi_k)$ on raw inputs. A pair can have high shared-MI on *irrelevant* dimensions and still provide independent task-relevant information. Example: E1 and E8 share e5-large-v2 backbone so they are near-linearly related on semantic dimensions (high MI), but the learned source/target projection on E8 isolates a graph-direction subspace that E1 does not encode. On a downstream link-prediction task $T$, E1 and E8 may be near-orthogonal even though their raw MI is high.

### 1.2 Protocol sketch
- Define 3–5 downstream tasks from the production system: retrieval accuracy, entity-link score, code-similarity Kendall's $\tau$, causal-direction accuracy, session-turn ordering
- For each embedder $\phi_j$: learn a linear classifier $\hat{Y}_T = w^\top \phi_j(X) + b$
- Measure classifier accuracy + MI estimate $\hat{I}(\phi_j; Y_T)$
- Pair comparison: $\hat{I}(\phi_j(X); \phi_k(X)) - \hat{I}(\phi_j(X); \phi_k(X) | Y_T)$ — the task-conditional redundancy

### 1.3 Priority
HIGH. Run after EXP-2 if Moderate or Weak. The task-conditional redundancy almost always shrinks relative to raw MI, and reporting that shrinkage recovers much of the commercial narrative that EXP-2 loses.

### 1.4 Timeline
+2–4 weeks post EXP-2.

---

## 2 · Conditional Pair MI (EXP-2C)

**Question**: $I(\phi_j; \phi_k | Z)$ for some third variable $Z$. If $I(\phi_j; \phi_k)$ is large but $I(\phi_j; \phi_k | Z)$ is small, then the pair MI is "explained by" $Z$.

Candidate $Z$ variables:
- **Content length** — are learned embedders near-linearly related after conditioning on input length?
- **Timestamp** — how much E2/E3/E4 temporal MI is absorbed by conditioning on the timestamp?
- **Session ID** — does E4 (positional) become independent of E1 (semantic) when conditioning on session?
- **Task-labels $Y_T$** (from EXP-2B)

### 2.1 Why it matters
Conditional MI tells us whether the redundancy is "structural" (same info about X) or "contextual" (same info about some other property of X). Structural redundancy is what hurts the counting identity; contextual redundancy does not.

### 2.2 Priority
MEDIUM-HIGH. Run after EXP-2B.

### 2.3 Estimators
CMI is harder than MI — no good neural estimator. Use KSG-CMI (conditional version of KSG), or the I-projection approach (Mukherjee 2020).

---

## 3 · Token-Level E12 Pseudo-MI (EXP-2D)

**Question**: EXP-2 pools E12 to 128D. What is the MaxSim-based pseudo-MI if we operate on full token-level E12?

### 3.1 Why it matters
EXP-2 pairs involving E12 are biased downward. A proper E12 MI measurement requires either:
- An MI estimator over variable-length sequences (hard), OR
- A surrogate based on MaxSim similarity + linear model

### 3.2 Protocol
Define the E12-to-$\phi_k$ "compatibility" as $C(x, x') = \max_i \max_j \cos(q_i, d_j)$ for token vectors $q_i, d_j$, and train a classifier to predict $\phi_k(x) \approx \phi_k(x')$ given $C(x, x') > \tau$. Use the classifier's gained information as the MI surrogate.

### 3.3 Priority
LOW. Only if the E12 pooling-induced bias dominates the narrative (it probably does not).

---

## 4 · Panel-extension growth (EXP-2E)

**Question**: if we add a hypothetical 14th embedder $\phi_{14}$ with pairwise MI $\bar{I}$ to each existing embedder, what is $N_\text{eff,pair}$ at $N = 14$?

### 4.1 Why it matters
The extensibility claim ("every new embedder adds $O(N)$ new signals") depends on the growth rate. EXP-2E explicitly computes that growth rate on the measured panel, plus sensitivity to the average pair-MI of the new embedder.

### 4.2 Protocol
- From EXP-2 data, compute $\bar{I}_{14} = \bar{I}_\text{existing}$ (baseline).
- Simulate: for each existing embedder $\phi_j$, draw $I_{14,j}$ from a distribution around $\bar{I}_{14}$. Compute $N_\text{eff,pair}$ at $N = 14$.
- Repeat for $\bar{I}_{14}$ values 0.3×, 0.5×, 0.7×, 1.0×, 1.3× of the measured panel average.
- Plot $N_\text{eff}$ vs. new-embedder-redundancy-level.

### 4.3 Priority
HIGH for a Moderate/Weak outcome. This is the headline defense against "91 is meaningless" — the growth curve is what matters long-term.

### 4.4 Timeline
Immediate post-EXP-2. A few hours of compute.

---

## 5 · ECAPA-TDNN Cross-Encoder Voice MI (EXP-1, separate but related)

**Question**: validate the 0.961 WavLM-SECS result with an ECAPA-TDNN cross-encoder.

This is **EXP-1** per the action list — a separate experiment from EXP-2. Listed here because it is often mentioned in the same breath and because the ECAPA-TDNN cross-encoder is one of the two "validator" assets for the voice-cloning case study.

### 5.1 Timeline
EXP-1 is the Case 3 follow-up; not EXP-2's responsibility but coordinated through the same preregistration discipline.

---

## 6 · Cross-Panel Replication (EXP-2F)

**Question**: run the EXP-2 protocol on someone else's panel. What $N_\text{eff}$ does a Meta / Anthropic / DeepMind internal multi-encoder panel yield?

### 6.1 Why it matters
Establishes the protocol as a reusable audit methodology. If the result cross-replicates at similar $N_\text{eff}$ levels on independent panels, the framework is defensible across the field. If it varies widely, the framework still contains value but the panel composition becomes the primary determinant.

### 6.2 Protocol
Published reproducibility materials enable a lab to clone the pipeline, define their own 13+ embedder panel, and run. Co-author offer (ACTION #9) Dohmatob is the natural first partner.

### 6.3 Priority
MEDIUM. Depends on whether a lab picks up the baton. If Perez/Dohmatob/Hutter engage, this is the natural collaborative next step.

### 6.4 Timeline
+3–6 months.

---

## 7 · Full 3-Tuple / Higher-Order Interactions (EXP-2G)

**Question**: $I(\phi_i; \phi_j; \phi_k)$ — the interaction information across three embedders. Some 3-tuples may exhibit synergy (the joint reveals more than the pairwise), others redundancy.

### 7.1 Why it matters
Pairwise MI misses three-way-and-higher structure. For $N = 13$ there are $\binom{13}{3} = 286$ triples, more than the 78 pairs. Some of these may exhibit synergy (total information > sum of pairwise), which is the *positive* version of redundancy and actually *increases* $N_\text{eff}$.

### 7.2 Protocol
Interaction information $\text{II}(X, Y, Z) = I(X; Y; Z) = I(X; Y) - I(X; Y | Z)$. Estimate via conditional MI on representative triples.

### 7.3 Priority
LOW but high-intellectual-value. Mainly useful as an academic contribution in a longer paper; not a commercial must-have.

### 7.4 Timeline
+6–9 months.

---

## 8 · Alternative estimators for robustness (EXP-2H)

**Question**: run EXP-2 with CPC-MI, NWJ, SMILE, and JS-divergence-based estimators. Do they agree within the same bounds?

### 8.1 Why it matters
MINE / InfoNCE / KSG are the pre-registered estimators. Running 3–5 more estimators as a sensitivity study strengthens the agreement claim.

### 8.2 Priority
LOW. Only useful if reviewers or a diligence team question the estimator-choice specifically.

### 8.3 Timeline
+1–2 months.

---

## 9 · Distribution-shift sensitivity (EXP-2I)

**Question**: how does $N_\text{eff}$ change across sub-distributions of the production corpus? (Domain-specific slices, recent-memory vs old-memory, high-importance vs low-importance, etc.)

### 9.1 Why it matters
If $N_\text{eff}$ is highly distribution-dependent, the headline number is an average over a diverse corpus. A more honest presentation may be per-slice.

### 9.2 Protocol
Use EXP-2's stratification axis (code / prose / mixed / short) to compute per-stratum $N_\text{eff}$. If strata differ by > 10 in $N_\text{eff}$, break out the finding.

### 9.3 Priority
MEDIUM. EXP-2 already runs 3 sensitivity corpora; this extends with inner-distribution strata.

### 9.4 Timeline
+1–2 weeks.

---

## 10 · Panel-ablation $N_\text{eff}$ (EXP-2J)

**Question**: what is $N_\text{eff}$ when we drop each embedder one at a time?

### 10.1 Why it matters
Identifies which embedders are "most redundant" (dropping them barely changes $N_\text{eff}$) vs "most unique" (dropping them decreases $N_\text{eff}$ most). Informs both:
- Pruning strategy (drop most-redundant for compute savings)
- Composition claim (show which embedders are irreplaceable)

### 10.2 Protocol
13 leave-one-out runs of the aggregation formula. Trivial given EXP-2 data.

### 10.3 Priority
HIGH for a Moderate outcome. Shows the commercial story of "the panel has structurally distinct contributors" vs "the panel is dominated by one or two slots."

### 10.4 Timeline
Immediate post-EXP-2. Negligible compute.

---

## 11 · Cross-Corpus Meta-Analysis (EXP-2K)

**Question**: across all 4 corpora (production + 3 sensitivity) × all sample sizes × all estimators, what is the most robust quantitative statement about the panel?

### 11.1 Why it matters
Strengthens the protocol's claim-making. A statement like "$N_\text{eff}$ within [18, 45] across 4 distributions" is more defensible than a single point estimate.

### 11.2 Priority
HIGH. Included in the final report ([07_reporting_template.md](07_reporting_template.md) §7).

---

## 12 · Stay-the-course priorities (ranked)

Post-EXP-2, in strict priority order:

1. **EXP-2E (panel-extension growth)**: +1 day compute, recovers the extensibility narrative
2. **EXP-2J (embedder-ablation)**: +1 day compute, supports the composition story
3. **EXP-2B (task-relevant MI)**: +2–4 weeks, recovers much of commercial value if Moderate/Weak
4. **EXP-2F (cross-panel replication with Dohmatob et al.)**: 3–6 months, scientific credibility
5. **EXP-2C (conditional MI)**: 2–4 weeks, secondary deepening
6. Everything else as bandwidth allows

---

## 13 · What EXP-2 does NOT foreclose

EXP-2 measures redundancy on the *current* panel. It does not say anything about:
- Different panel compositions (different embedders)
- Different input distributions beyond what we test
- Different downstream tasks
- Whether the panel will remain relevant in 12 months

Each of these is addressable by a follow-up experiment with its own pre-registration. The protocol we build for EXP-2 is the template for all of them.
