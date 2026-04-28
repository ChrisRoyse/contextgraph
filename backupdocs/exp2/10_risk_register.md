# EXP-2 Risk Register

**What can go wrong. Every risk has a named owner, a probability-at-mitigation estimate, an impact, and a dated mitigation plan.**

Risk IDs: `R-<n>`. This register is frozen at preregistration time; risks added later are logged as amendments.

---

## R-1 · Estimator mis-calibration on control pair

**Description**: the E1-copy control returns MI substantially below $H(\phi_{E1})$, indicating MINE is under-estimating even on a trivial pair.

**Probability (w/ mitigation)**: 10 %
**Impact**: halt; every downstream MI estimate is suspect.
**Mitigation**:
1. Pre-register: expected $\hat{I}(\phi_{E1}, \phi_{E1}) \approx 0.95 H(\phi_{E1})$ after 2000 MINE steps.
2. Cross-check against KSG entropy estimate on $\phi_{E1}$ alone.
3. Add synthetic Gaussian test to the pipeline ([08_implementation_plan.md §6.1](08_implementation_plan.md)).

**On trigger**: halt the pipeline. Write `control_failure_<date>.md`. Diagnose: PCA dim, critic capacity, dtype, seed plumbing. Amend protocol (dated, signed). Re-run from controls.

---

## R-2 · Strong estimator disagreement (> 20 % pairs)

**Description**: MINE / InfoNCE / KSG disagree by > 2 bits on more than 20 % of the 78 pairs.

**Probability (w/ mitigation)**: 15 %
**Impact**: decision rule cannot be applied; headline $N_\text{eff}$ is unreliable.
**Mitigation**:
1. Pre-register the agreement criterion ([04_estimator_specifications.md §6](04_estimator_specifications.md)).
2. Run 5 seeds on each pair; flag high seed-variance pairs.
3. Dual reporting: if disagreement is real, report all three estimator values in the per-pair table.

**On trigger**: halt external surface. Chris + Steve review. If disagreement is consistent (e.g., InfoNCE systematically saturates but MINE and KSG agree), publish MINE + KSG with saturation caveats. If inconsistent, amend with deeper critic architecture (larger MINE, smaller tau for InfoNCE) and rerun.

---

## R-3 · Computation exceeds 36 GPU-hour budget

**Description**: $n = 1000$ MINE sweep takes > 48 hours instead of 24, blowing the timeline.

**Probability (w/ mitigation)**: 15 %
**Impact**: Day 11 KSG / Day 12 sensitivity phases slip.
**Mitigation**:
1. Staged execution: $n = 100$ first (1 hr), $n = 500$ next (8 hr), $n = 1000$ last (24 hr). Earliest ship wins.
2. `--resume` support in the sweep orchestrator ([08_implementation_plan.md §4.4](08_implementation_plan.md)).
3. Compute reserve via parallel GPU on the same node if VRAM allows (production daemon uses 12 GB of 32 GB).

**On trigger**: accept $n = 500$ as the headline and report $n = 1000$ partially. Add caveat to the report.

---

## R-4 · Corpus size $M$ at freeze is too small

**Description**: the production Context Graph at freeze has fewer than 5000 memories, making stratified sampling of 1000 representative inputs difficult.

**Probability (w/ mitigation)**: 20 % (depends on corpus state at Day 6)
**Impact**: bootstrap CIs are wider; strata may be thin.
**Mitigation**:
1. Count $M$ and per-stratum $M_s$ before the freeze; if $M < 5000$ or any stratum $< 250$, the sensitivity corpus (Wikipedia / Code / Shakespeare) becomes the primary alongside production.
2. Reduce $n$ if necessary (1000 $\to$ 500 $\to$ 200); re-report.

**On trigger**: treat Wikipedia-5k as primary corpus for the headline $N_\text{eff}$; production becomes a sensitivity check.

---

## R-5 · E6 / E13 dense-ification / JL sketch introduces artifact

**Description**: the 30522-dim dense-ification + JL sketch introduces structured noise that MINE interprets as MI.

**Probability (w/ mitigation)**: 10 %
**Impact**: E6 and E13 pair estimates are inflated (or deflated) by the sketch.
**Mitigation**:
1. Pre-register sketch seed (`sketch_seed = 2026`), matrix serialized.
2. Include a control: compute MI on the raw sparse representation via Jaccard-kernel approximation for 3 pairs (E6-E13, E6-E1, E13-E1); cross-check against dense-sketched MINE.
3. Report JL distortion bound: Johnson-Lindenstrauss with $d = 2048$ for $n = 1000$ points preserves pairwise distances within a factor of $(1 \pm \epsilon)$ where $\epsilon \approx 0.15$.

**On trigger**: if the Jaccard cross-check disagrees with MINE by > 2 bits, halt E6/E13 estimates and report them as "cannot be reliably estimated; see Appendix."

---

## R-6 · E12 pooling loss biases all E12 pair estimates

**Description**: mean-pooling ColBERT tokens destroys token-level information, making E12 look artificially low-MI with everything.

**Probability (w/ mitigation)**: 35 % (structural, not random)
**Impact**: E12 pair estimates are systematically biased downward.
**Mitigation**:
1. Report explicitly: "E12 is mean-pooled; raw MaxSim-based MI is expected to be 2–4× higher."
2. Follow-up experiment in [14_followup_experiments.md §3](14_followup_experiments.md) computes pseudo-MI via MaxSim for E12.
3. Do not include E12 in the headline $N_\text{eff}$ if its pooling bias dominates: compute $N_\text{eff}$ with and without E12, report both.

**On trigger**: structural; mitigated by disclosure, not intervention.

---

## R-7 · "Copy of E1" control over-estimates

**Description**: MINE on $\phi_{E1}$ vs $\phi_{copy} = \phi_{E1}$ reports MI far exceeding the PCA-truncated entropy estimate.

**Probability (w/ mitigation)**: 15 %
**Impact**: suggests MINE is finding spurious MI (likely from finite-batch or memorization).
**Mitigation**:
1. Pre-register PCA-truncation preserves finite entropy; pre-registered entropy estimate is the target.
2. Test across multiple seeds; over-estimation should be seed-independent if it's real.
3. Compare KSG entropy to MINE MI as a sanity check.

**On trigger**: diagnose (larger hold-out split, larger critic, different temperature); amend pre-registration with dated signature.

---

## R-8 · Memory-ID anonymization breaks reproducibility

**Description**: when we publish the anonymized corpus, a reader cannot regenerate the exact production sample because the memory IDs are hashed.

**Probability (w/ mitigation)**: LOW (by design; this is expected)
**Impact**: reproducibility is on the *tensors*, not the *texts*.
**Mitigation**:
1. Publish the embedded tensors + PCA matrices + seed list directly; the reader can re-run MI estimation bit-identically without needing to re-embed the original text.
2. Full text is retained locally for independent in-house audits.

**On trigger**: disclose clearly in the reproducibility appendix.

---

## R-9 · Preprocessing / seed drift between runs

**Description**: a re-run produces slightly different $N_\text{eff}$ due to RNG initialization order.

**Probability (w/ mitigation)**: 30 % without discipline, < 1 % with discipline
**Impact**: tests appear flaky; reviewers distrust the pipeline.
**Mitigation**:
1. All RNGs centralized in `scripts/exp2/lib/seeds.py` ([08_implementation_plan.md §5.5](08_implementation_plan.md)).
2. Every `torch.manual_seed`, `numpy.random.seed`, `rand::SeedableRng` call pinned to a named seed in `seeds.py`.
3. `torch.backends.cudnn.deterministic = True`, `benchmark = False`.
4. End-to-end regression test: same seeds → same output (tolerance $10^{-6}$ on bit-level, $0.01$ bits on the $N_\text{eff}$ rollup).

**On trigger**: run the regression test. If it fails, bisect with `git bisect` to find the introduction of non-determinism.

---

## R-10 · A lab runs EXP-2 first and publishes

**Description**: between Day 0 and Day 21 a lab (Anthropic, DeepMind, FAIR) publishes a PRH-style result on our panel composition or a similar 13-embedder substrate.

**Probability (w/ mitigation)**: 10 % in 21 days (arXiv + USPTO monitoring per action list #19)
**Impact**: the headline loses novelty; framing shifts from "first empirical measurement of pair-MI on a production 13-encoder panel" to "replication of X et al."
**Mitigation**:
1. arXiv + USPTO daily alerts live Day 5 (action list #19).
2. If a match surfaces, 48-hour triage: is it prior art or independent?
3. Accelerate EXP-2 reporting; publish realized floor alongside their result and note protocol differences.

**On trigger**: priority-accelerate the report; publish on arXiv within 48 hr of the lab's publication.

---

## R-11 · Compliance-gate failure on reporting language

**Description**: the report, Perez-memo revision, or external communication uses prohibited language from [07_reporting_template.md §12](07_reporting_template.md).

**Probability (w/ mitigation)**: 5 % with Steve's review gate, 40 % without
**Impact**: narrative poisoning; a single prohibited phrase (e.g., "solves the data wall," "proves Axiom 2") collapses credibility per WINNING_PLAYBOOK.
**Mitigation**:
1. Steve's compliance review is a mandatory gate before any external surface ([09_timeline_and_milestones.md §3](09_timeline_and_milestones.md)).
2. Laminated phrases-to-never-say card (ACTION #10 on the 14-day action list).
3. Template-based generation: report text is filled from [07_reporting_template.md](07_reporting_template.md), not free-written.

**On trigger**: halt external surface; same-day revision; re-submit for Steve's review.

---

## R-12 · Patent disclosure timing conflict

**Description**: counsel advises that publishing EXP-2 results before patent filing narrows claim scope.

**Probability (w/ mitigation)**: 15 %
**Impact**: either delay EXP-2 publication (acceptable) or lose patent claim scope (not acceptable).
**Mitigation**:
1. Counsel briefed on EXP-2 content by Day 5; publication timing known to counsel by Day 7.
2. Internal EXP-2 report (Day 20) is not external publication; Perez memo revision is controlled-disclosure-under-NDA.
3. Patent filed Day 14; EXP-2 external publication Day 22+.

**On trigger**: defer external publication; internal report still produced.

---

## R-13 · $N_\text{eff}$ comes out so low the playbook must pivot mid-flight

**Description**: EXP-2 returns $\hat{N}_\text{eff} \le 8$ with tight CI, ruling out even the Weak-outcome commercial narrative.

**Probability (w/ mitigation)**: 5 % (priors place most weight on Moderate)
**Impact**: the 91-signal framing becomes actively unhelpful in pitches; must reframe to three-role composition + $G_\tau$ + per-output cosine predicate.
**Mitigation**:
1. [13_failure_mode_playbook.md](13_failure_mode_playbook.md) provides the catastrophic-weak script.
2. Constraints-compliant pivot: do not claim pan-panel independence; lean on $G_\tau$ per-output predicate (which is a separate primitive, not dependent on pair-independence).
3. Leapable pricing tier as a fallback monetization track ([08_implementation_plan.md §9](08_implementation_plan.md) separate).

**On trigger**: per playbook, drop action items #16 (Stebbings), #18 (bridge), #20 (dean-advisor) to buy the week needed for pitch revision. Action #8 (Perez) memo revision deferred to Day 28; first write the reframed memo, not the EXP-2 addendum.

---

## R-14 · Dependencies break on production daemon state change

**Description**: the corpus-build binary depends on the daemon being healthy. A daemon restart mid-sweep, a RocksDB compaction spike, or a VRAM pressure event could corrupt fetches.

**Probability (w/ mitigation)**: 10 %
**Impact**: partial corpus with undefined sampling bias.
**Mitigation**:
1. Daemon health check before build; abort if unhealthy.
2. Fetch integrity check: every returned fingerprint validated (13 vectors, expected dims, finiteness).
3. Fetch retry policy: 3 retries on timeout, with exponential back-off.

**On trigger**: halt and restart from the last checkpoint (stable fingerprint count).

---

## R-15 · "Emergent" behavior from seed choice

**Description**: one of the 5 MINE seeds happens to produce an estimate that diverges from the other 4 by > 2 bits, dominating the median.

**Probability (w/ mitigation)**: 20 % on at least one pair
**Impact**: reported median is contaminated by the outlier.
**Mitigation**:
1. Use the **median** of 5 seeds, not the mean (already in the protocol).
2. Report the 5 individual seeds alongside the median.
3. If one seed consistently diverges across multiple pairs, flag the seed as defective and document.

**On trigger**: report median as primary; flag the outlier seed explicitly.

---

## R-16 · Huh et al. 2024 generalizes — i.e., even non-learned embedders converge

**Description**: the prediction of PRH extends beyond learned encoders to include non-learned embedders (E2/E3/E4 temporal, E9 HDC), producing surprisingly high MI on unexpected pairs.

**Probability (w/ mitigation)**: 5 %
**Impact**: panel achieves Strong $N_\text{eff}$ against prior expectation; minor re-interpretation needed.
**Mitigation**:
1. The decision rule is outcome-agnostic; Strong is Strong.
2. If Strong because of low learned-learned MI + high learned-non-learned MI (the PRH extension case), note the phenomenon in the discussion.

**On trigger**: report as-is; PRH-extension would be a novel finding worth a separate follow-up paper.

---

## 17 · Catastrophic halt conditions (no recovery within protocol)

- `halt-1`: Controls repeatedly fail even after amendments.
- `halt-2`: Estimator disagreement > 40 % of pairs.
- `halt-3`: Hardware failure (GPU crash, RocksDB corruption) that makes the corpus unreproducible.
- `halt-4`: A prior publication appears that makes EXP-2 redundant.
- `halt-5`: Legal instruction from counsel to not publish pre-patent.

In all halt conditions: **no external surface.** Chris + Steve + counsel decide next steps. EXP-2 enters a holding state with the existing (internal) artifacts preserved.

---

## 18 · Review cadence

- **Pre-freeze** (Day 0-6): each risk re-examined; mitigation updated if new information arises.
- **Mid-execution** (Day 7-21): any new risk observed is added as `R-<n+k>` in the amendment log.
- **Post-execution**: risk register reviewed as part of the final report (appendix). Retrospective notes on what actually fired, what didn't.
