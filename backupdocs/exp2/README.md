# EXP-2 — Pairwise Mutual Information Audit of the 13-Embedder Panel

**Status**: pre-registration phase (drafted 2026-04-23)
**Owner**: Chris Royse (primary), Steve Abbey (compliance reviewer)
**Critical path**: ACTION #3 in the 14-day action list — kickoff 2026-04-30 (Day 7), completion 2026-05-14 (Day 21)
**Pre-registration freeze date**: 2026-04-29 (one day before first embedding pass)

---

## What EXP-2 is

EXP-2 is the pairwise mutual-information audit on the 13-embedder Context Graph panel. For every pair of embedders $(\phi_j, \phi_k)$ in E1–E13, it computes an estimate of $I(\phi_j(X); \phi_k(X))$ on a sample of real inputs drawn from the production distribution. Three independent estimators are run in parallel: MINE (primary), InfoNCE lower bound (backup), and KSG (tertiary non-parametric cross-check).

The experiment converts the identity *n · (N + C(N,2)) = up to 91 supervisory signals per input at N = 13* from a **constructive upper bound** into a **measured realized floor**, reported as $N_\text{eff}$.

---

## Why it matters now

1. **Commercial**: in a diligence room, any competent lab research team can run EXP-2 in 72 hours. If they find $N_\text{eff} = 10$ and you never ran it, your price gets halved in the same meeting (red-team CR-2, probability-recalibration doc).
2. **Scientific**: Huh et al. 2024 (*Platonic Representation Hypothesis*, ICML 2024) directly predicts modern SSL encoders converge on overlapping representations. That prediction, if borne out on this panel, falsifies Axiom 2 of the counting identity as originally stated.
3. **Signaling**: running EXP-2 on the buyer's own compute during due diligence is the nuclear separating-signal that distinguishes "real Chris" from "charlatan." You cannot hold that card until you have first run it yourself.

---

## File index

| # | File | Purpose |
|---|------|---------|
| 00 | [00_executive_summary.md](00_executive_summary.md) | One-page summary for Chris, Steve, and any counsel reading |
| 01 | [01_preregistration.md](01_preregistration.md) | Locked protocol — freeze-before-run, anti-p-hacking |
| 02 | [02_panel_specification.md](02_panel_specification.md) | Exact 13-embedder panel, dims, dual-vector handling |
| 03 | [03_sampling_corpus.md](03_sampling_corpus.md) | Input selection, stratification, sample sizes |
| 04 | [04_estimator_specifications.md](04_estimator_specifications.md) | MINE, InfoNCE, KSG full specs with math |
| 05 | [05_methodology.md](05_methodology.md) | Step-by-step procedure, reproducibility checklist |
| 06 | [06_analysis_and_decision_rules.md](06_analysis_and_decision_rules.md) | MI matrix → $N_\text{eff}$, decision thresholds |
| 07 | [07_reporting_template.md](07_reporting_template.md) | Results format — constructive ceiling + realized floor |
| 08 | [08_implementation_plan.md](08_implementation_plan.md) | Code layout — Rust bin + Python scripts, file paths |
| 09 | [09_timeline_and_milestones.md](09_timeline_and_milestones.md) | Day 7 → Day 21 Gantt, gating criteria |
| 10 | [10_risk_register.md](10_risk_register.md) | What can go wrong, mitigations, kill-switches |
| 11 | [11_prior_art_and_citations.md](11_prior_art_and_citations.md) | Huh PRH, Belghazi MINE, KSG, Kaplan 2020 |
| 12 | [12_publication_strategy.md](12_publication_strategy.md) | Perez memo paragraph, patent claim, public comms |
| 13 | [13_failure_mode_playbook.md](13_failure_mode_playbook.md) | Strong / Moderate / Weak response playbook |
| 14 | [14_followup_experiments.md](14_followup_experiments.md) | Task-relevant MI, conditional MI, ablations |
| 15 | [15_code_stubs.md](15_code_stubs.md) | Pseudocode for each estimator + aggregation |
| 16 | [16_glossary_and_notation.md](16_glossary_and_notation.md) | Definitions: $\phi$, $I$, $N_\text{eff}$, $H$, KL, etc. |

---

## Quick runbook (once frozen)

```
# Day 7 (2026-04-30): Freeze panel + corpus + estimator hyperparameters
#   → commit to git, tag v0.exp2.preregistered
# Day 8–10: Build sampling corpus (01_preregistration.md §4)
# Day 11–14: Run MINE + InfoNCE on all 78 pairs × 3 sample sizes
# Day 15–17: Run KSG cross-check on representative subset (ceiling / floor / median pairs)
# Day 18: Bootstrap confidence intervals, effective-N aggregation
# Day 19: Internal review with Steve (compliance gate before any external surface)
# Day 20: Commit results, generate reporting artifacts
# Day 21: Freeze final report — insert into Perez memo revision + patent disclosure
```

---

## Three outcomes, three scripts

| Scenario | $N_\text{eff}$ range | What the pitch becomes |
|----------|------------|-------------------------|
| **Strong** | 70–91 | "Counting identity confirmed at the measured floor; panel achieves near-independent signals." |
| **Moderate** | 25–50 | "Measured 25–50× unique supervisory signals — substantial multiplicative advantage over single-encoder; extensibility claim dominant." |
| **Weak** | 8–15 | "91 is a structural ceiling; realized floor is 8–15. Thesis holds for growth curve (new embedder adds pairs); commercial pitch reframes from absolute signal count to substrate-level composition." |

Detailed response scripts per outcome in [13_failure_mode_playbook.md](13_failure_mode_playbook.md).

---

## Hard rules (see also 01_preregistration.md)

1. **The panel and the estimator hyperparameters are frozen before the first MI is computed.** No post-hoc tuning. No "we tried three temperatures and reported the highest." Freeze date 2026-04-29.
2. **All three estimators run on the same panel and sample.** Disagreement between estimators is reported, not resolved by selection.
3. **Bootstrap CIs and seed sweeps are mandatory**, not optional. 5 seeds, 1000 bootstrap samples, report the 95% CI not the point estimate.
4. **No data from the panel influences estimator architecture.** MINE critic network is fixed to the [Belghazi et al. 2018](https://arxiv.org/abs/1801.04062) 2-layer-MLP-256 default, not tuned.
5. **Publish realized floor alongside constructive ceiling.** 91 stays in the paper as a structural bound; $N_\text{eff}$ is the operating number.

---

## Cross-references

- Counting identity definition: position paper §9 (Conclusion), axioms §2 Axiom 2
- Commercial context: `/home/cabdru/teleoxwebsite/docs/gametheoryinvestigation/executive/14_DAY_ACTION_LIST.md` (ACTION #3)
- Red-team risk: `.../PROBABILITY_RECALIBRATION.md` CR-2
- Pitch framing: `.../WINNING_PLAYBOOK.md` §4.4 (Perez technical memo)
- Embedder subsystem reference: `docs2/contextgraphspecs/05_embedding_subsystem.md`
- Search/fusion: `docs2/contextgraphspecs/06_search_and_fusion.md`
