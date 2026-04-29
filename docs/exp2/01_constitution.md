# EXP-2 Constitution

Immutable rules. Read before any implementation work. If a task or PR violates a rule here, the task is wrong, not the rule.

## C1. Embedder Frozen Forever

The 14 production embedders (E1 e5-large-v2, E2 sinusoidal-PE, E3 Fourier, E4 positional, E5 nomic-causal, E6 SPLADE-coCondenser, E7 Qodo-Embed, E8 e5-large-v2-graph, E9 HDC, E10 e5-base-v2, E11 KEPLER, E12 ColBERTv2, E13 SPLADE-PP, E14 BGE-M3-Dense) are **frozen at their pinned weight hashes** for the duration of EXP-2 and forever afterward. The audit chain records each embedder's SHA-256 weight hash; any deviation invalidates the run.

## C2. Single Corpus, Frozen Sample

The 800-input corpus selected per [04_data_corpus.md](04_data_corpus.md) is **frozen** at task T-004 completion. No additions, no removals, no re-sampling, no re-stratification. If a sanity check fails, the failure is investigated, not papered over. The corpus hash (SHA-256 of canonicalized JSON manifest) is logged in every downstream artefact.

## C3. Two MI Estimators, Both Reported

All 91 pairs receive **both** an InfoNCE/MINE estimate and a partitioned-KSG estimate with bootstrap 95% CIs. We report both. We do not silently pick the one that confirms the headline. If the two disagree by more than 0.15 nats, the disagreement is documented per pair with hypotheses for the cause.

## C4. Bootstrap Is Mandatory

Every reported MI scalar carries a 95% bootstrap CI computed from ≥10,000 resamples with replacement, **fixed seed** (default `42`), percentile method. Single-point estimates are not publishable.

## C5. No Hyperparameter Tuning On The Reported Numbers

InfoNCE network architecture, learning rate, batch size, training steps, KSG $k$ parameter, partition strategy are **all set before any production run** per [03_methodology.md](03_methodology.md) §6. They are not adjusted to make the headline number look better. Pre-registration: hyperparameters committed to git before the run; deviations require a separate documented run.

## C6. Replay Determinism

The pipeline is deterministic given a fixed seed and frozen corpus. The FSV harness ([07_validation_harness.md](07_validation_harness.md) §2) re-runs the pipeline twice and asserts numerical identity to ≤ $10^{-5}$ on all reported scalars. Any nondeterminism is investigated, eliminated, or explicitly logged with cause.

## C7. Audit Chain First

Every artefact (estimator output, plot, summary, scalar) carries a provenance record:
- Corpus manifest SHA-256.
- Embedder weight hashes (14).
- Code commit hash.
- Hyperparameter manifest SHA-256.
- Run timestamp.
- Run seed.

The audit chain is written *before* the data is consumed for visualization. No artefact ships without the chain.

## C8. Honest Numbers Or No Numbers

If $N_{\mathrm{eff}}$ comes in low (say, 25 instead of 91), **we publish 25.** The paper's defense is the *measurement* itself, not the headline number. A measured low $N_{\mathrm{eff}}$ that surprises us is more valuable than a fitted high $N_{\mathrm{eff}}$ that confirms us.

## C9. No Generator In The Loop

EXP-2 is purely an instrumentation pass. No fine-tuning, no LoRA, no generation, no DPO. The frozen embedders + the frozen corpus + the estimators. This is *measurement*, not training.

## C10. Defense In Depth Against Known Failure Modes

Known failure modes ([07_validation_harness.md](07_validation_harness.md) §4) are checked explicitly:
- Embedder degeneration (zero-norm vectors): rejected.
- Corpus duplicates: detected and excluded.
- KSG curse-of-dimensionality: addressed via partitioning before estimation.
- InfoNCE instability: detected via training curve monitoring; runs with non-converged loss are flagged and re-run with diagnostic logging.
- Numerical overflow in fp32 cosine: clamped, logged, never silently wrapped.

## C11. Open Source The Harness

The Python harness (`scripts/exp2/`) is released under Apache-2.0 alongside the paper revision. The corpus manifest, the seed, the hyperparameters, the embedder weight hashes are public. **Independent replication is the goal, not optional.**

## C12. The Paper Patch Is Drop-In

The §3.1 and §6.1 patches in [09_distribution_plan.md](09_distribution_plan.md) are written so that the paper text can be replaced atomically — no surrounding text edits required. If the patch breaks paper continuity, the patch is wrong, not the surrounding text.

## C13. The YouTube Video Includes The Heatmap At T+0:30

The video opens with the heatmap on screen no later than 30 seconds in. The audience does not need to wait through preamble for the visual payload. ([09_distribution_plan.md](09_distribution_plan.md) §C.)

## C14. ArXiv Revision Within 14 Days Of Last Run

Once T-021 (FSV pass) lands, the arXiv revision goes live within 14 calendar days. Delay erodes the moment when the result is novel.

## C15. Failure Mode Is Re-Run, Not Re-Interpret

If a pair MI estimate is implausible (e.g., $I(E1; E10) = 0.01$ nats when both are dense semantic embedders trained on overlapping corpora), the response is to **re-run with diagnostics**, not to re-interpret. A surprising number is a hypothesis to test, not a result to publish.

## C16. The Operator Stays In The Loop

This is your paper, your stack, your reputation. Subagents may execute tasks T-001 through T-026, but the operator personally signs off on:
- The corpus manifest before the embedding-extraction phase.
- The estimator hyperparameters before the production run.
- The headline $N_{\mathrm{eff}}$ number before publication.
- The paper patch before arXiv submission.

## C17. Pre-Registration On Open Science Framework

Before the production run begins, the hyperparameters + corpus manifest hash + analysis plan are pre-registered on OSF (https://osf.io). This makes the experimental record auditable to the broader research community and protects against accusations of post-hoc analysis.

## C18. No File At Repository Root

Per CLAUDE.md project rules, no working files, tests, or markdown go to the root folder. EXP-2 artefacts go under `scripts/exp2/` for code, `docs/exp2/` for docs, `artefacts/exp2/` for output bundles (with the `artefacts/` directory ignored by `.gitignore` and shipped via Git LFS or HuggingFace Hub for the published bundle).

---

These 18 constitutional rules are not negotiable. Subagents that violate them fail. Operator that overrides them owns the consequence with a written rationale.
