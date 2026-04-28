---
namespace: discoveries
agent_id: agent-03-self-ask-decomposer
timestamp: 2026-04-16T15:45:00Z
status: active
depends_on:
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
tags: gaps, open-questions, gap-hunter-input, camera-ready-risks
---

# Knowledge Gaps — Questions We CANNOT Fully Answer With Current Evidence

This is agent-18 (gap-hunter)'s primary input. Each gap below is a question from
`agent-03--essential-questions.md` whose answer is partial or missing at submission time.
For each gap we state (a) severity, (b) what is known, (c) what is missing, (d) what a
camera-ready revision would need to close it, and (e) which agent should chase it.

Severity levels:

- **CRITICAL** — blocks submission; an adversarial reviewer will reject on this alone.
- **HIGH** — will be flagged; paper survives if explicitly hedged.
- **MEDIUM** — known limitation, not a blocker, should be in §8.
- **LOW** — minor clarification worth tracking.

---

## G01 — Q08 Threshold calibration for Cases 1 and 2 (HIGH)

- **Known.** Case 3 thresholds calibrated via WavLM same-session band (0.95–0.99) on the
  author's own recordings; this is a T1-grounded procedure for Case 3.
- **Missing.** Cases 1 (Shakespeare) and 2 (Santa) have no measured per-modality
  threshold $\tau_m$ yet. Without $\tau_m$, the runtime constellation guard in those cases
  can be described only architecturally (T3).
- **Camera-ready need.** Either (a) calibrate on held-out subject recordings / First Folio
  lines and report the resulting $\tau_m$, or (b) explicitly state the calibration
  protocol and flag the number as projected.
- **Chase.** agent-25 (method-designer), agent-29 (validity-guardian). Re-verification
  cadence: at each Case 1 / Case 2 re-run.

## G02 — Q13 Case 1 (Shakespeare) and Case 2 (Santa) metrics (HIGH)

- **Known.** Systems exist (T2: code, FSV logs, inspect docs). Case 1's 15/15 FSV
  history is documented; Case 2's 2,362-clip pipeline and flow-matching fix are
  documented. SFT+DPO retrain is in progress; Santa LoRA is architecturally complete.
- **Missing.** No T1 number for held-out constellation-guard accept-rate or held-out
  SECS / lip-sync. Paper-as-submitted will carry T3 flags.
- **Camera-ready need.** Run the held-out evaluations described in Q13 and Q18 and swap
  T3 for T1 in §6.1 / §6.2 headers.
- **Chase.** agent-38 (results-writer), agent-43 (reproducibility-checker). Projected
  numbers must carry the V22 "(projected; camera-ready will re-verify)" flag wherever
  they appear.

## G03 — Q16 Formal embedder-independence measurement (HIGH)

- **Known.** V11 / glossary #14 hedge "orthogonal ≈ approximately independent". No
  formal measurement of inter-embedder information overlap (e.g., MI across $\phi_j$ and
  $\phi_k$ for the 13 Context Graph embedders) has been performed.
- **Missing.** The $\binom{N}{2}$ pairwise-interaction count in DDA is upper-bounded by
  the joint MI structure; without measurement, the count is theoretical (T4) only.
- **Camera-ready need.** Either a pairwise MI / $R^2$ table across all $\binom{13}{2}=78$
  embedder pairs on a held-out corpus, OR a published citation we can invoke for a
  subset, OR an explicit reframing in §3.4 that the count is a theoretical upper bound
  absent a separate independence measurement.
- **Chase.** agent-21 (theory-builder), agent-09 (methodology-scanner). Future-work
  listed in Q20.

## G04 — Q12 / Q14 Single-subject, $n=10$ sample size on Case 3 (HIGH)

- **Known.** Case 3 measures WavLM SECS 0.961 on 10 unseen sentences for one speaker
  (the author).
- **Missing.** No confidence interval, no cross-speaker generalization, no cross-encoder
  SECS (only WavLM at this writing; glossary #20 flags that cross-encoder evaluation is
  preferred for bias-mitigation). FM-05 is the named failure mode.
- **Camera-ready need.** Either (a) extend to multiple speakers from a public dataset
  (LibriSpeech subset) to establish a range, or (b) keep the single-subject protocol
  but compute bootstrap / jackknife confidence intervals, or (c) add a cross-encoder SECS
  (SpeechBrain ECAPA-TDNN, Qwen3-speaker) so the number is not encoder-dependent.
- **Chase.** agent-29 (validity-guardian), agent-38.

## G05 — Q18 Proprietary vs. substitutable data for Cases 2 and 3 (MEDIUM)

- **Known.** Case 2 source is a 16-minute interview recording of the author. Case 3
  reference set is 50 audio clips of the author. Both are identity-sensitive and cannot
  be distributed.
- **Missing.** A clear substitutable public corpus statement — what a replicator should
  use instead.
- **Camera-ready need.** §8.5 reproducibility notes must list, per case, a specific
  public dataset a replicator can use (e.g., LibriSpeech-clean-100 for voice;
  Single-subject short-interview public dataset for Case 2). Where the public dataset
  has different statistics, state the substitution cost explicitly.
- **Chase.** agent-43 (reproducibility-checker), agent-37.

## G06 — Q14 VALL-E 2 comparability scope (MEDIUM)

- **Known.** VALL-E 2 (Chen et al., 2024) reports human-parity on LibriSpeech continuation
  with specific protocols. Our Case 3 uses Qwen3-TTS + Full ICL + 50-clip centroid + WavLM
  best-of-12 on 10 sentences from a different held-out set.
- **Missing.** A properly controlled head-to-head. Direct comparability is limited by
  encoder, held-out set, and speaker cohort.
- **Camera-ready need.** Either (a) run both systems on an identical held-out set scored
  by an identical cross-encoder, or (b) keep the current framing and explicitly state
  in §6.3 and §8 that the comparison is *directional, not head-to-head*. Option (b) is
  acceptable; option (a) is stronger.
- **Chase.** agent-38, agent-29, agent-42.

## G07 — Q08 Same-session-band universality (MEDIUM)

- **Known.** Glossary #17 / V12: the 0.95–0.99 WavLM same-session band is an empirical
  observation on Case 3's speaker.
- **Missing.** Published literature / public benchmark that would establish a same-session
  band across many speakers.
- **Camera-ready need.** Either cite a published WavLM same-session study (literature
  search required) or keep the explicit hedge. The hedge is already in V12 and is
  sufficient if applied rigorously by agent-38.
- **Chase.** agent-06 (literature-mapper), agent-08 (citation-extractor).

## G08 — Q17 Full bug-forensics citations (LOW)

- **Known.** Shakespeare echo-bug documented in FSV logs; Santa flow-matching sign-bug
  documented in `docs/inspect/08_training_audit.md`; cv2 frame-rate and YOLOv5-Face
  mouth-landmark bugs referenced in the 17-attempt history.
- **Missing.** Specific file paths for the cv2 and YOLOv5-Face bug records (the author
  has them but they are not yet routed through `./memory/`).
- **Camera-ready need.** Add the exact file paths to the §6 forensic paragraphs so each
  bug cite is traceable (P10).
- **Chase.** agent-38, agent-42. The Santa project path `/home/cabdru/santa/docs/inspect/`
  (or equivalent) should be walked to confirm file names before §6 is drafted.

## G09 — Q19 Watermarking / authentication posture for misuse mitigation (LOW)

- **Known.** Ethics question acknowledges identity-cloning misuse risk.
- **Missing.** A specific mitigation beyond "the constellation guard can verify" — does
  the paper commit to watermark embedding, cryptographic output signing, or public
  registry of reference centroids? These are design choices.
- **Camera-ready need.** §8.6 should either (a) commit to a specific mitigation posture
  (with citation) or (b) state explicitly that mitigation is future work and disclose
  the residual risk. Option (b) is acceptable for this submission.
- **Chase.** agent-31 (ethics-reviewer).

---

## Summary — gaps by severity

| Severity | Gaps                       | Submission impact                                       |
|----------|----------------------------|---------------------------------------------------------|
| CRITICAL | (none)                     | No gap blocks submission if we honor V22 and P01.       |
| HIGH     | G01, G02, G03, G04         | Must be hedged; camera-ready must close G02 at minimum. |
| MEDIUM   | G05, G06, G07              | §8 limitations must cite each.                          |
| LOW      | G08, G09                   | Cite-chase before §6 draft; disclose in §8.6.           |

---

## What the gap-hunter (agent-18) should do with this

1. Build a priority-ranked chase list keyed to these G-rows.
2. For each HIGH gap, confirm whether the "camera-ready need" is achievable on the
   current timeline; if not, ensure the V22 hedge is present in the writer's section.
3. Route G08 to agent-38 as a pre-draft cite-completion task (file paths only, no new
   prose).
4. Cross-check G03 against the §3.4 orthogonality hedge (V11) — the hedge must match
   the actual measurement state.

---

## Change log

- 2026-04-16 — Initial gap log by agent-03-self-ask-decomposer.
