---
namespace: patterns
agent_id: agent-38-results-writer
timestamp: 2026-04-17T12:30:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
  - memory/discoveries/agent-29-validity-guardian--validity-threats.md
  - memory/patterns/agent-34-confidence-quantifier--per-claim-confidence.md
  - memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md
  - memory/handoff/agent-37-to-38--method-ready.md
downstream_consumers:
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
tags: writing-decisions, results, case-studies, measured-projected-split, hedges, banned-phrases
---

# Writing Decisions — Agent-38 Results Writer (§4 Systems, §6 Case Studies)

## 0. Scope

This file records the decisions taken while writing:

- `sections/04_systems.md` — §4 Production Systems (brief, ~980 words).
- `sections/07_case_studies.md` — §6 Case Studies (combined §6.1 Case 1,
  §6.2 Case 2, §6.3 Case 3, §6.4 summary; ~3,050 words total).

## 1. Header alignment

- §4 file: `# 4. Production Systems` per handoff instruction.
- §6 file: `# 6. Case Studies` per handoff instruction. Subsection
  numbering within the file: §6.1 Shakespeare, §6.2 Santa, §6.3 Voice,
  §6.4 summary.

## 2. Word-count discipline

- §4 target 700–1,000; delivered ~980 words across 4.1/4.2/4.3.
- §6 target 2,500–3,200; delivered ~3,050 words across 6.1/6.2/6.3/6.4.

## 3. Measured-versus-projected split

**MEASURED numbers (only from Case 3, all flagged in-line):**
- Mean WavLM SECS 0.961 (N=10)
- Max WavLM SECS 0.975 (N=10)
- Min WavLM SECS 0.914 (N=10)
- 9 of 10 sentences $\geq$ 0.95
- DNSMOS P.808 clone 3.93 vs real 3.93 (N=14)
- UTMOS clone 3.012 vs real 2.997 (N=14)
- Bootstrap 95% CI [0.951, 0.971] (N=10, seed 20260416)
- Temperature sweep T ∈ {0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8} values
  from alldata.md §5.4 reproduced exactly
- Centroid ablation: 1-clip 0.946 vs 50-clip 0.961 (+0.015)
- Prior-run 15/15 FSV for Case 1 (measured on that prior run; retrain
  status pending)
- Case 2 forensic integrity: 2,362 clips, 30 GB on-disk, six fixes in
  code, centroid sample counts (visual 1,725; semantic 200; emotion
  343; speaker 300; prosody 188; sentiment 202; voice 188)

**PROJECTED numbers (all flagged with "projected" or italicised):**
- Case 1: $\tau_{\text{text-style}} \in [0.80, 0.92]$ (H2c T3)
- Case 1: 15/15 pass on cleaned retrain
- Case 2: visual 0.70, emotion 0.60, speaker 0.80, voice 0.85 (T3)
- Case 2: visual severity tiers WARN 0.80, STRONG 0.90 (T3)
- Case 2: all guard-ON/OFF, mouth-weighted ablations, FID, LSE-D/LSE-C

Source-fidelity check: every exact MEASURED value was verified against
`tasks/newdata/alldata.md` lines 1450–1525 and agent-28 §4 before
insertion. The 0.961 / 0.975 / 0.914 / 9-of-10 / 3.93 / 3.012 / 2.997
numbers appear in alldata.md verbatim and are reproduced verbatim here.

## 4. Hedges carried

Per agent-34 §2 cue-sheet, every T1 MEASURED number in Case 3 is paired
with the mandatory companion sentence:

- C5 W1 disclosure ("encoder-matched; guard and evaluator both
  WavLM-base-plus-sv; does not constitute cross-encoder identity
  verification") — §6.3.2 and §6.3.5 item 1.
- C6 naturalness scope ("predictor-output parity; N=14 too small for
  statistical equivalence test; Bias-05 / Bias-06") — §6.3.2.
- C7 T=0.3 scope ("in this pipeline, this speaker, this corpus, this
  scorer") — §6.3.3.
- C8 50-clip centroid scope ("single operating point; $n_m$ curve is
  RO-10 future work") — §6.3.4.
- C9 Case-2 guard ON/OFF ("T3 projected; we predict; by analogy to
  Case 3") — §6.2.4.
- C10 mouth-weighted ("consistent with; VT-I4 future work; not
  necessary") — §6.2.2 and §6.2.4.
- C11 Case-1 retrain ("retrain in progress; projected; camera-ready
  will re-verify") — §6.1.1 and §6.1.4.
- V22 measured-vs-projected on every number.
- V18 frozen-with-LoRA on every embedder mention.

## 5. Banned-phrase audit

Cross-checked against agent-34 §4 banned-phrase list. None of the
following appear in either output file:

- "breakthrough"
- "revolutionary"
- "game-changing"
- "the best"
- "solves AGI"
- "91 signals per input" (bare — always qualified "up to")
- "0.961 mean SECS" (bare — always paired with "under the
  encoder-matched protocol" or equivalent scope tag)
- "indistinguishable from real speech" (bare — always paired with
  "predictor-output parity; N=14; Bias-05/06")
- "solves the data wall"
- "outperforms RLHF"
- "T=0.3 is optimal" (bare — always paired with pipeline-specific
  scope)
- "proven collapse-free"
- "mouth-weighted loss is necessary" (bare — always "consistent with")
- "we demonstrate the sample-complexity collapse" (bare — not used;
  C12a/C12b split respected)

## 6. Table discipline

Three tables total, per hard-rules budget:

- **Table 1** (§6.3.3): Temperature sweep. 7 rows × 3 columns.
- **Table 2** (§6.3.4): Centroid-size ablation. 2 rows × 3 columns.
- **Table 3** (§6.4): Summary across three cases. 3 rows × 5 columns.

Each table is minimal: no decorative columns, no derived statistics
beyond what the source data supports, numbers rounded to 3 sig-fig
per agent-28 §1 convention.

## 7. Forensic findings cited

- **Case 1 echo-bug**: 67 % (5,954 of 8,857) of SFT examples had
  play-script header prefixes; fix was training-data preparation;
  documented in `docs2/LORA_STYLE_TRANSFER_COMPLETE_REPORT.md`.
- **Case 2 flow-matching sign-bug**: `target = latent - noise` vs
  correct `target = noise - latent`; documented in the Santa project's
  `docs/inspect/08_training_audit.md`. Five companion blockers
  (cv2 fps mismatch, mouth-bbox on nose, dead q_audio LoRA target,
  22 GB RAM OOM, uniform viseme sampler) named in the same forensic
  pass.

## 8. Content boundary discipline

Explicitly did NOT re-narrate:

- §3.1 DDA counting identity (§3 owns it; §6 may reference "up to 91
  signals" in the Context Graph system description in §4, which I
  carried over via the upper-bound hedge).
- §3.4 three-role composition (§3 owns it; §4 invokes it in §4.3).
- §5 TCT phase specifications (§5 owns; §6 instantiates).
- §5.6 case-specific protocol table (§5 owns; §6 reports results
  against that table without re-stating it).

Checked §6 against §5 overlap: no duplicated equations, no duplicated
definitions, no duplicated threshold calibration procedure.

## 9. APA 7th in-text citations

Citations introduced in §4 and §6, all from the locked 88-pool per
agent-08:

- Anthropic (2024) — MCP protocol.
- Chen et al. (2022) — WavLM.
- Chung & Zisserman (2017) — SyncNet (named as pending LSE-D/LSE-C
  instrument).
- Cormack et al. (2009) — RRF.
- Desplanques et al. (2020) — ECAPA-TDNN (named as pending
  cross-encoder).
- Formal et al. (2021) — SPLADE.
- Hu et al. (2022) — LoRA (pre-approved).
- Lipman et al. (2023) — Flow matching (pre-approved).
- Meng et al. (2025) — EchoMimicV3.
- Reddy et al. (2022) — DNSMOS.
- Saeki et al. (2022) — UTMOS.
- Santhanam et al. (2022) — ColBERTv2.
- Wang et al. (2021) — KEPLER.
- Zhai et al. (2023) — SigLIP.

No new citations introduced beyond this set. No citation of the
scalar-reward alignment cluster (reserved for §7 Discussion per
agent-37 handoff).

## 10. What §6 hands to §7 (per agent-37 handoff §"What §6 should
produce for §7")

- **Case 1** T3 projected with 15/15 prior anchor; retrain in progress.
- **Case 2** T3 projected with forensic integrity complete.
- **Case 3** T1 measured with encoder-matched caveat.
- One-paragraph synthesis framing in §6.4 (Table 3 caption + trailing
  line): Case 3 is the one T1 anchor at M=1, n_m=50; Case 1 projects
  to M=1 text; Case 2 projects to M≥4 multi-modal.
- Three pending companion measurements for Case 3 named by identifier:
  cross-encoder SECS via ECAPA-TDNN; pre-selection cosine distribution
  over 120 candidates; $n_m$ curve at {10, 25, 50, 100}. §8 may quote
  verbatim.

## 11. Confidence

**HIGH** that §4 is under 1,000 words and covers Context Graph (370K
Rust, 13 embedders, 59 CFs, 75 MCP tools, 5,184 tests) and ClipCannon
(67,585 Python, 23-stage DAG, 7 modalities × 4,044 dim, 58 MCP tools)
at block-diagram level with Leapable only in the acknowledgements.

**HIGH** that Case 3 numbers (0.961 / 0.975 / 0.914 / 9-of-10 / 3.93 /
3.012 / 2.997 / [0.951, 0.971]) are reported exactly as in
`tasks/newdata/alldata.md` and agent-28 §4.3.

**HIGH** that every number in both files is tier-flagged MEASURED or
PROJECTED per the hard-rules discipline.

**HIGH** that the banned-phrase list is clean.

**MEDIUM** on reviewer-perception of the Case-2 T3 projection density:
§6.2 is primarily narrative and forensic because no T1 numbers exist
for it; §7 Discussion should avoid treating any §6.2 number as
measured.

## Changelog

- 2026-04-17 — Initial issue by agent-38-results-writer.
