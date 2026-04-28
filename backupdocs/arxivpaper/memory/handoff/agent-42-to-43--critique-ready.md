---
namespace: handoff
agent_id: agent-42-adversarial-reviewer
timestamp: 2026-04-17T11:20:00Z
status: active
supersedes: none
tags: handoff, agent-43, reproducibility-checker, critique-ready, red-team-done
---

# Handoff: Agent-42 (adversarial-reviewer) → Agent-43 (reproducibility-checker)

## Read these files FIRST, in this order

1. `memory/discoveries/agent-42-adversarial-reviewer--critique.md` —
   main adversarial critique. Read sections 2 (section-by-section),
   3 (cross-cutting attacks), and 5 (stop-ship list).
2. `memory/patterns/agent-42-adversarial-reviewer--stop-ship-and-edits.md`
   — paste-ready edits for 3 CRITICAL + 14 HIGH findings.
3. `memory/discoveries/agent-32-contradiction-analyzer--contradiction-register.md`
   — IC-6 (asymmetric release) governs the entire reproducibility
   posture. Read IC-6 resolution carefully.
4. `memory/discoveries/agent-33-falsification-agent--counterarguments.md`
   — CA-3 (sample-complexity; INSUFFICIENT; must be softened) and
   CA-6 (commercial-COI; SUFFICIENT conditional on FM-01 pass) are
   the counterarguments that most directly drive reproducibility
   scrutiny.
5. `sections/06_method_tct.md` (§5 TCT method) and
   `sections/07_case_studies.md` (§6 case studies) — the sections
   where reproducibility claims and asymmetric-release posture
   live.

## What agent-42 found (executive summary for agent-43)

- **3 CRITICAL, 14 HIGH, 19 MEDIUM, 3 LOW findings.**
- **8 stop-ship items** (must-fix before arXiv v1): scope placement
  in predicate paragraph, bolded-headline asymmetry, temperature-
  sweep-vs-headline disambiguation, commercial-terms phrasing in §4,
  SMAP internal-identifier leakage, §8.L8 → §Competing Interests,
  missing per-measurement justifications in §6.3.5, Track-A/B +
  "camera-ready" language.
- **Worst cross-cutting attack**: CA-12 (SMAP identifier leakage —
  W1/VT-I3/IC-4/CA-N/C5/etc. tags across §1–§9 body). Makes the
  manuscript read as a draft rather than a finished artefact.
- **Second-worst**: CA-4 (commercial COI exposure); §Competing
  Interests not yet drafted; FM-01 regex not yet run; §4.3 carries
  commercial-terms phrasing that must be moved to Acknowledgements.
- **Paper is presentation-risk, not substance-risk**. Every CRITICAL
  or HIGH finding is a scope / typography / labeling / framing
  issue; the underlying claims hold. Agent-32 and agent-33 caught
  the substantive issues upstream.

## Reproducibility-specific handoff (your domain)

### Sections that need the closest reproducibility scrutiny

In priority order, the sections where reproducibility claims are
most vulnerable:

1. **§5.6 Case-specific protocols** (`sections/06_method_tct.md`).
   Three case instantiations with hyperparameters, reference-set
   sizes, thresholds, and rejection policies. Your job: verify each
   parameter is either (a) stated verbatim here, (b) pointed at a
   config file in the repository that is released, or (c) explicitly
   flagged as projected. Specific concerns:
   - Case 3 Full-ICL + 50-clip centroid + best-of-12 at $T = 0.3$ —
     this is the MEASURED case and is the reproducibility anchor.
     The $\tau = 0.95$ calibration needs a disjoint calibration set
     referenced by name / size / date. Finding 5.2 edit (§5.6 Case 3)
     strengthens this.
   - Case 2 seven-modality LoRA rank 256 / $\alpha = 512$ on 8 target
     modules with a specific per-modality $\lambda$ schedule
     (Finding 5.5 notes voice-modality $\lambda$ absent). Verify
     the schedule is complete or mark voice projected.
   - Case 1 SFT + DPO with $\lambda_{\mathrm{text\text{-}style}} =
     0.5$, DPO $\beta = 0.1$, three-layer stack, $K = 4$ regenerate.
     Finding 5.4 adds the fallback policy. Verify Layer 3 policy is
     now named.

2. **§6.3 Case 3 protocol** (`sections/07_case_studies.md`). The one
   MEASURED case. Reproducibility scrutiny targets:
   - The 10 held-out sentences: how were they selected? The paper
     does NOT state the sentence-selection protocol explicitly.
     This is agent-29 VT-I1 flag. Verify §6.3.2 or §6.3.5 names
     the sentence-selection criterion; if not, add a limitation.
   - The 50-clip centroid is drawn from a 489-candidate pool by
     "ranking: WavLM similarity against the real-microphone
     reference". Verify this pre-ranking does not leak information
     from the evaluation set (the evaluator is WavLM; if the ranking
     used WavLM and the held-out evaluation uses WavLM on audio from
     the same session, there is a potential circularity).
   - The bootstrap CI [0.951, 0.971] at 10,000 resamples fixed seed
     20260416 percentile — verify the seed and method are stated;
     §6.3.2 does state them. Good.
   - The "pipeline version `qwen3-tts-foricl-v1`" is cited but not
     defined. Verify either (a) a pipeline manifest hash is in the
     artefact release, or (b) §5.6 Case 3 provides enough detail for
     reconstruction.

3. **§4 Production systems** (`sections/04_systems.md`). Context
   Graph ("≈370,000 lines") and ClipCannon ("≈67,585 lines") are
   cited as constructive evidence. Reproducibility concerns:
   - Are the repositories public? §4 does not state. §4.3 says
     Leapable.ai is named in Acknowledgements but does NOT say
     Context Graph and ClipCannon are open or closed. Verify the
     open-source status of each system is stated in §4 or §Ethics.
   - Line-count claims (370K, 67K) are not reproducibility items
     per se but invite verification. Agent-44 citation-validator
     may want to independently confirm; for your purposes, flag
     that the count is a claim not a citation.
   - The 13 embedders E1–E13 are listed by architecture and
     dimensionality. Verify each embedder's weight-pinning (which
     checkpoint hash) is either in §4.1 or in the companion
     reproducibility manifest. The current §4.1 does not list
     weight hashes per embedder.

4. **§6.2 Case 2 Santa pipeline** — PROJECTED. The forensic fixes
   (§6.2.2) are the one concrete contribution. Reproducibility
   concern: the sign-bug fix (`target = noise - latent`) is a
   commit-level change to training code that is described in prose
   but not cited to a commit hash in this version. Agent-31 ethics
   posture withholds Case 2 artefacts on dual-use grounds, so the
   code release is asymmetric. Verify the §Ethics statement (not
   yet drafted) names what is released vs. withheld at
   commit-hash granularity where possible.

5. **IC-6 asymmetric-release posture** (
   `memory/discoveries/agent-32-contradiction-analyzer--contradiction-register.md`
   §IC-6). This is the governing frame for reproducibility:
   - Case 1 = **byte-identical reproducibility** (Shakespeare LoRA
     training scripts + weights + cleaned corpus released).
   - Case 2 = **protocol-level reproducibility** (code for the
     pipeline, not the 2,362 clips or the LoRA weights on subject
     data).
   - Case 3 = **protocol-level reproducibility** (code for the
     pipeline, not the 50-clip centroid, the reference recordings,
     or the held-out evaluation set).
   Your scrutiny: does every §6 subsection carry the appropriate
   reproducibility-level label? Agent-42 finding: **No — the labels
   "byte-identical" and "protocol-level" do not appear in
   `sections/07_case_studies.md`**. This is a repro-disclosure gap
   that you may want to insert.

### Reproducibility items agent-42 did NOT check

Because they are explicitly downstream of your work:

- **Hyperparameter completeness check.** Does the paper give
  sufficient hyperparameters to reproduce Case 3 end-to-end? Agent
  -42 did not compare §5.6 Case 3 against standard reproducibility
  checklists (e.g., NeurIPS reproducibility checklist).
- **Seed reproducibility.** The paper names seeds for bootstrap
  (20260416) and logs per-candidate seeds (Finding 6.7 edit), but
  does not discuss whether bf16 / fp32 / CUDA version reproducibility
  can be achieved bit-identically across rebuilds.
- **Data-release manifest.** The paper does not contain a "what is
  released, where to find it" table. The §Ethics and §Data Access
  statements (not yet drafted) should contain this.
- **Dependency versions.** Qwen3-TTS, WavLM-base-plus-sv,
  ECAPA-TDNN, SigLIP-SO400M — version pinning not audited.
- **Benchmark-comparison repro.** The paper cites VALL-E 2 at 0.881
  WavLM-large-sv; agent-42 did not check whether the VALL-E 2
  number is reproducible from public artefacts. agent-32 EC-10
  notes VALL-E 2 is closed-artefact; flag consistent with that.

### Specific sub-sections you should question

**HIGH priority (most likely to have reproducibility issues):**
- §5.6 Case 2 — the absent $\lambda_{\mathrm{voice}}$ (Finding 5.5).
  If a reproducer applies the loss without the voice term and gets
  different results, the paper's projection is non-reproducible.
- §6.3.3 Table 1 temperature sweep — the T=0.3 sweep mean (0.932)
  vs. the headline mean (0.961). Finding 6.2 explains this as
  different seed chains; your scrutiny should verify that the seed
  chain archival is sufficient to reproduce either number.
- §6.3.4 Table 2 centroid-size ablation — the 1-clip baseline's
  missing max (Finding 6.3). If it is a single-draw number, the
  ablation is apples-to-oranges against best-of-12 at 50 clips.

**MEDIUM priority:**
- §3.1 Identity (1) and the signal definition — Finding 3.1 asks for
  a linear-reparameterisation exclusion. For reproducibility, the
  practical question is: given a production Context Graph instance,
  can a reproducer produce the same $D'$ deterministically? The
  frozen embedder hashes need to be in the released manifest.
- §5.3 Phase 2 loss equation (4) — the $\lambda_m$ schedule is a
  "constant" starting value with a halving policy on collapse. For
  reproducibility, any actual collapse event during a Case 1 or
  Case 2 run should be logged and the post-halve $\lambda_m$
  recorded.

**LOW priority:**
- §4 Production system descriptions — line counts, tool counts,
  column-family counts. Reproducibility-irrelevant; verify only
  that the counts are not misleading.

## Files agent-42 produced

1. `memory/discoveries/agent-42-adversarial-reviewer--critique.md`
   — section-by-section + 14 cross-cutting + severity tally +
   stop-ship list + worst cross-cutting identification.
2. `memory/patterns/agent-42-adversarial-reviewer--stop-ship-and-edits.md`
   — paste-ready edits for 3 CRITICAL + 14 HIGH findings.
3. `memory/handoff/agent-42-to-43--critique-ready.md` (this file).
4. `memory/reports/agent-42-adversarial-reviewer--final-report.md`
   — final report.

## Confidence

**HIGH** that the stop-ship list is complete for v1 presentation
risks.

**HIGH** that the reproducibility scrutiny list above points you at
the right sections. If you find reproducibility issues outside these
sections, they are likely lower-severity than the ones I flagged.

**MEDIUM** that I caught every presentation risk. A second
adversarial pass (e.g., agent-46 coherence-checker's scrutiny read)
will likely catch 2–3 items I missed. I did NOT run the FM-01 regex
pass myself — CA-11 is my by-eye read, not a deterministic scan.

**HIGH** that applying the eight stop-ship fixes moves the paper
from "targeted reviewer challenges expected" to "arXiv v1 will
stand on its own merits with the known limitations". Applying the
full HIGH set moves it to "strong submission".

## Suggested sequence for agent-43

1. Read `memory/discoveries/agent-42-adversarial-reviewer--critique.md`
   §5 (stop-ship list) first.
2. Read `memory/patterns/agent-42-adversarial-reviewer--stop-ship-and-edits.md`
   §2 (CRITICAL) and §3 (HIGH subset) to understand the concrete
   edits.
3. Focus your own scrutiny on the HIGH-priority items above (§5.6
   Case 2 missing $\lambda$, §6.3.3 seed chain reproducibility,
   §6.3.4 Table 2 pairing).
4. Explicitly check that the asymmetric-release posture (IC-6;
   byte-identical Case 1 vs. protocol-level Case 2/3) is labelled
   in §6.
5. Flag any reproducibility issue not already in the agent-42
   critique; your domain is reproducibility-strict and your
   findings should be additive, not duplicative.

## Changelog

- 2026-04-17 — Handoff written by agent-42-adversarial-reviewer.
