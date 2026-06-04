# References

## External works

Every entry below was verified against its live source (arXiv abstract page, publisher page, or PDF) during preparation, 2026-06-04. Author lists, titles, venues, and years are transcribed from the rendered citation metadata.

- **[Harnad1990]** Harnad, S. (1990). *The Symbol Grounding Problem.* Physica D: Nonlinear Phenomena, 42(1–3), 335–346. doi:10.1016/0167-2789(90)90087-6. — The Chinese/Chinese dictionary-go-round; symbols grounded outside the symbol system.

- **[VincentLamarre2016]** Vincent-Lamarre, P., Blondin Massé, A., Lopes, M., Lord, M., Marcotte, O., & Harnad, S. (2016). *The Latent Structure of Dictionaries.* Topics in Cognitive Science, 8(3), 625–659. doi:10.1111/tops.12211. — Grounding Kernel ≈10%; Minimal Grounding Set (minimum feedback vertex set) ≈1% of the dictionary / ≈15% of the Kernel; Core ≈75% of Kernel; NP-hard.

- **[Steyvers2005]** Steyvers, M., & Tenenbaum, J. B. (2005). *The Large-Scale Structure of Semantic Networks: Statistical Analyses and a Model of Semantic Growth.* Cognitive Science, 29(1), 41–78. doi:10.1207/s15516709cog2901_3. — Small-world, scale-free semantic networks grown by differentiation.

- **[Mollo2023]** Coelho Mollo, D., & Millière, R. (2023). *The Vector Grounding Problem.* arXiv:2304.01481. — Referential grounding of vector representations via causal-informational relations + selection history (teleosemantics).

- **[Belghazi2018]** Belghazi, M. I., Baratin, A., Rajeswar, S., Ozair, S., Bengio, Y., Courville, A., & Hjelm, R. D. (2018). *Mutual Information Neural Estimation (MINE).* ICML, PMLR 80:531–540. arXiv:1801.04062. — Neural MI estimator via the Donsker–Varadhan representation.

- **[vandenOord2018]** van den Oord, A., Li, Y., & Vinyals, O. (2018). *Representation Learning with Contrastive Predictive Coding.* arXiv:1807.03748. — InfoNCE; `I(x;c) ≥ log N − L_NCE`.

- **[Kraskov2004]** Kraskov, A., Stögbauer, H., & Grassberger, P. (2004). *Estimating Mutual Information.* Physical Review E, 69, 066138. arXiv:cond-mat/0305641. — k-nearest-neighbor MI estimator.

- **[Poole2019]** Poole, B., Ozair, S., van den Oord, A., Alemi, A. A., & Tucker, G. (2019). *On Variational Bounds of Mutual Information.* ICML, PMLR 97. arXiv:1905.06922. — Unified treatment of variational MI bounds; bias/variance tradeoffs.

- **[Tishby1999]** Tishby, N., Pereira, F. C., & Bialek, W. (1999). *The Information Bottleneck Method.* Proc. 37th Allerton Conf. arXiv:physics/0004057. — Relevant information `I(·;Y)` preserved through a compression bottleneck.

- **[CoverThomas2006]** Cover, T. M., & Thomas, J. A. (2006). *Elements of Information Theory* (2nd ed.). Wiley. — Data-processing inequality; Fano's inequality.

- **[Fano1961]** Fano, R. M. (1961). *Transmission of Information: A Statistical Theory of Communications.* MIT Press. — Fano's inequality relating error probability to conditional entropy.

- **[Karp1972]** Karp, R. M. (1972). *Reducibility Among Combinatorial Problems.* In Complexity of Computer Computations, 85–103. Plenum. — NP-hardness of the feedback vertex set problem.

- **[LeCun2022]** LeCun, Y. (2022). *A Path Towards Autonomous Machine Intelligence* (Version 0.9.2, 2022-06-27). OpenReview id BZ5a1r-kVsf. — Joint Embedding Predictive Architecture (JEPA): prediction in representation space, heterogeneous non-shared encoders, non-generative predictor.

- **[Huh2024]** Huh, M., Cheung, B., Wang, T., & Isola, P. (2024). *The Platonic Representation Hypothesis.* ICML 2024 (Position), PMLR 235:20617–20642. arXiv:2405.07987. — Representations converging to a shared statistical model of reality; converged kernel approaching PMI over latent generative events.

- **[Chollet2019]** Chollet, F. (2019). *On the Measure of Intelligence.* arXiv:1911.01547. — Intelligence as skill-acquisition efficiency; the ARC benchmark.

- **[Guo2017]** Guo, C., Pleiss, G., Sun, Y., & Weinberger, K. Q. (2017). *On Calibration of Modern Neural Networks.* ICML, PMLR 70:1321–1330. arXiv:1706.04599. — Expected Calibration Error; temperature scaling.

- **[Brier1950]** Brier, G. W. (1950). *Verification of Forecasts Expressed in Terms of Probability.* Monthly Weather Review, 78(1), 1–3. — The Brier score.

- **[Angelopoulos2021]** Angelopoulos, A. N., & Bates, S. (2021). *A Gentle Introduction to Conformal Prediction and Distribution-Free Uncertainty Quantification.* arXiv:2107.07511. — Distribution-free prediction sets; abstention.

- **[Kirkpatrick2017]** Kirkpatrick, J., Pascanu, R., Rabinowitz, N., Veness, J., Desjardins, G., Rusu, A. A., et al. (2017). *Overcoming Catastrophic Forgetting in Neural Networks.* PNAS, 114(13), 3521–3526. doi:10.1073/pnas.1611835114. — Elastic Weight Consolidation for continual learning.

- **[Manheim2018]** Manheim, D., & Garrabrant, S. (2018). *Categorizing Variants of Goodhart's Law.* arXiv:1803.04585. — Failure modes of optimizing a proxy for a target.

- **[Jimenez2024]** Jimenez, C. E., Yang, J., Wettig, A., Yao, S., Pei, K., Press, O., & Narasimhan, K. (2024). *SWE-bench: Can Language Models Resolve Real-World GitHub Issues?* ICLR 2024 (oral). arXiv:2310.06770. — 2,294 problems across 12 Python repositories; the test-harness oracle used here.

- **[Aleithan2024]** Aleithan, R., Xue, H., Mohajer, M. M., Nnorom, E., Uddin, G., & Wang, S. (2024). *SWE-Bench+: Enhanced Coding Benchmark for LLMs.* arXiv:2410.06992. — 32.67% solution leakage; 31.08% weak-test passes; same defects in Lite and Verified; >94% of issues predate common knowledge cutoffs.

- **[Wang2025]** Wang, et al. (2025). *Are "Solved Issues" in SWE-bench Really Solved Correctly? An Empirical Study.* arXiv:2503.15223. — PatchDiff differential testing; running all repository tests flips ~7.8% of plausible patches to incorrect (≈4.5% resolution-rate drop).

- **[OpenAI2026]** OpenAI (2026). *Why SWE-bench Verified No Longer Measures Frontier Coding Capabilities.* openai.com/index/why-we-no-longer-evaluate-swe-bench-verified/. — Audit of a failed-task subset: 59.4% with flawed test design / problem statements; gold-patch reproduction indicating training-set leakage.

### World-model generalization (§10)

- **[Bialek2001]** Bialek, W., Nemenman, I., & Tishby, N. (2001). *Predictability, Complexity, and Learning.* Neural Computation, 13(11), 2409–2463. arXiv:physics/0007070. — Predictive information $I(\text{past};\text{future})$; the predictable structure is sub-extensive and is the unique complexity measure.

- **[Shalizi2001]** Shalizi, C. R., & Crutchfield, J. P. (2001). *Computational Mechanics: Pattern and Prediction, Structure and Simplicity.* Journal of Statistical Physics, 104(3–4), 817–879. arXiv:cond-mat/9907176. — Causal states and the $\epsilon$-machine: the minimal sufficient statistic for prediction.

- **[Spelke2007]** Spelke, E. S., & Kinzler, K. D. (2007). *Core Knowledge.* Developmental Science, 10(1), 89–96. doi:10.1111/j.1467-7687.2007.00569.x. — Core knowledge systems: objects, agents/actions, number, space/geometry (and a possible social system).

- **[Lake2017]** Lake, B. M., Ullman, T. D., Tenenbaum, J. B., & Gershman, S. J. (2017). *Building Machines That Learn and Think Like People.* Behavioral and Brain Sciences, 40, e253. arXiv:1604.00289. — Developmental "start-up software": intuitive physics and intuitive psychology as priors enabling few-shot learning.

- **[LeCun2022]** LeCun, Y. (2022). *A Path Towards Autonomous Machine Intelligence* (Version 0.9.2). OpenReview id BZ5a1r-kVsf. — The Joint Embedding Predictive Architecture (JEPA): prediction in representation space; heterogeneous non-shared encoders.

- **[Anderson1972]** Anderson, P. W. (1972). *More Is Different.* Science, 177(4047), 393–396. doi:10.1126/science.177.4047.393. — Reductionism does not imply constructionism; new organizing laws emerge at each scale.

- **[Feynman1982]** Feynman, R. P. (1982). *Simulating Physics with Computers.* International Journal of Theoretical Physics, 21(6–7), 467–488. doi:10.1007/BF02650179. — Quantum systems cannot be simulated efficiently on a classical computer (exponential state space).

- **[Wolfram2002]** Wolfram, S. (2002). *A New Kind of Science.* Wolfram Media. — Computational irreducibility: for most non-trivial systems, the only way to find the outcome is to run the computation.

- **[Mehta2014]** Mehta, P., & Schwab, D. J. (2014). *An Exact Mapping Between the Variational Renormalization Group and Deep Learning.* arXiv:1410.3831. — Deep learning implements a generalized renormalization-group coarse-graining that extracts the relevant variables.

> Note: [LeCun2022] also supports §6 (panel architecture) and is listed once here. Core information-theoretic references shared with §5 ([CoverThomas2006], [Fano1961], [Tishby1999], [Belghazi2018], [Kraskov2004]) are reused by §10 without restatement.

## Internal evidence index (project source-of-truth)

Project-internal facts cited in §7–§8 trace to these artifacts and tracked issues. Production durable state lives on the aiwonder host.

| Tag | Fact | Source of truth |
|---|---|---|
| EV-MI | `I(panel; oracle)`: MINE 0.0960, classifier-LB 0.1254, KSG 0.4611 bits; `panel_sufficient=false`; per-cell + drop-one tables; 2,400 rows, `feature_dim=19209`, `target_bits=0.9978`, slot identity preserved, no flat-concat | Issue #759; FSV `/zfs/archive/contextgraph/fsv/inv-032-panel-sufficiency-mi-fsv/run-20260529T234900Z/full-candidate-map` |
| EV-ORACLE | `oracle_self_consistency = 1.0` over 30 instances × 10 replicas (300 invocations); 300/300 reports; all exit 0 | Issue #760; FSV `/zfs/archive/contextgraph/fsv/inv-035-oracle-noise-floor-fsv/run-20260529T233417Z` |
| EV-PEARSON | predictor raw Pearson `0.4843`; Brier `0.0924`; ECE `0.0240`; rank-lift `0.7942` (diagnostic only, Goodhart) | Ship-gate FSV `swebench-300x8-ship-gate-fsv/run-20260521T132308Z` |
| WORK-842 | Ship-gate reframe (anchor vs kernel; existence falsified; corrected ordering) | Issue #842 |
| WORK-843 | Kernel/MGS computation harness + MI existence gate | Issue #843 |
| WORK-745 | Panel sufficiency / in-slot causal embedder replacement (binding gate-mover) | Issue #745 |
| WORK-844 | Embedder candidate-screening + assembly loop (acquire→run→measure ΔI→assemble) | Issue #844 |
| WORK-841 | Constellation-as-definition derivation engine `Def(t)` | Issue #841 |

**Reproducibility note.** The MI estimator stack includes a synthetic self-test (strong-signal control → 0.977 bits; noise control → 0.0) validating the estimator before it is applied to real data; the runner fails closed on degenerate labels (`DEGENERATE_ORACLE_LABELS`). All artifacts are content-hashed and independently re-opened for readback (§8).
