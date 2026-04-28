---
namespace: discoveries
agent_id: agent-09-methodology-scanner
timestamp: 2026-04-16T18:35:00Z
status: active
tags: methodological-gaps, novelty-delta, fills, evidence-based, related-work-source
depends_on:
  - memory/discoveries/agent-09-methodology-scanner--methodology-scan.md
  - memory/discoveries/agent-06-literature-mapper--structural-holes.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md
downstream_consumers:
  - agent-10-theoretical-framework-analyst
  - agent-11-systematic-reviewer
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-42-adversarial-reviewer
---

# Methodological Gaps Our Paper Fills

This file consolidates the **23 per-cluster methodological gaps** in
`methodology-scan.md` into **9 consolidated methodological gaps** that the arXiv
submission explicitly fills. Each consolidated gap is:

1. **Evidence-grounded** — cites the specific cluster + paper(s) whose methodology
   is absent of the feature, not a vague "nobody does this."
2. **Narrow** — expressed as a structural delta (per P07 / FM-03), not as an AGI
   claim or a superlative.
3. **Traced** — linked to at least one Tier-A construct (glossary #1–#4), at
   least one essential question (Q01–Q20), and at least one combined structural
   hole (HOLE-1 … HOLE-5 in structural-holes.md).

The nine consolidated gaps feed:
- **§5 Methods** (agent-37) — the method-level novelty claims.
- **§2 Related Work** (agent-36) — the §2 G3 novelty-delta table.
- **§8 Limitations / §10 Future Work** (agent-39) — where our gap-fills end and
  the next pieces of open work begin.

---

## METHOD-GAP-1. Signals-per-input is not a measurement convention.

**Source clusters & papers.**
- Cluster A: kaplan2020scaling, hoffmann2022chinchilla, villalobos2024data all
  count **raw tokens** / tokens-per-parameter / corpus-stock; none counts
  signals-per-input.
- Cluster C: deletang2024compression, huang2024compression, li2025lossless,
  valmeekam2023llmzip report compression ratio bits/bits on a *sequence*; none
  reports labelled signals per *input*.
- Cluster D: cormack2009rrf, khattab2020colbert, santhanam2022colbertv2 report
  retrieval quality; none reports signal yield from N jointly-applied embedders.

**What's missing methodologically.** There is no established protocol for
*counting* signals extractable from a fixed corpus projected through N frozen
independent embedders. The closest precedents (Cluster-D fusion lift; Cluster-C
compression ratio) are on different units and do not compose.

**What our paper adds.** **§3.4 DDA count** $n(N + \binom{N}{2})$. This is a
**TD** (theoretical-derivation) contribution. Instantiated at N=13 (Context
Graph → 91 signals/input) and N=7 (ClipCannon → 28). Future ablation across
varying N is open work (§10 / Q20).

**Tier-A constructs covered.** DDA (#1), meaning compression (#2), cross-embedder
interaction feature (glossary #15).

**Essential questions answered.** Q04, Q05.

**Combined structural hole filled.** HOLE-1, HOLE-2.

**Confidence.** HIGH (the absence is categorical, not a coverage claim; no
methodology in any cluster reports this unit).

---

## METHOD-GAP-2. No cross-embedder pairwise-interaction measurement.

**Source clusters & papers.**
- Cluster C (compression) papers report per-compressor ratios; no cross-compressor
  interaction protocol.
- Cluster D (retrieval) papers report fusion lift (cormack2009rrf) which is *one
  scalar* across rankers, not $\binom{N}{2}$ pairwise features per input.

**What's missing methodologically.** The per-input pairwise signal
$\rho_{jk}(\phi_j(x), \phi_k(x))$ has no reporting convention. Cluster D's fusion
work summarises across inputs; Cluster C's compression work operates per-compressor.

**What our paper adds.** **§3.4 cross-embedder interaction feature** (glossary
#15). The $\binom{N}{2}$ term is the pairwise-interaction count. Instantiation
options described: normalised cosine, MI estimate, concatenation-plus-projection.
The T4 count ($\binom{13}{2} = 78$ for Context Graph, $\binom{7}{2} = 21$ for
ClipCannon) is the explicit contribution; an empirical measurement of pairwise
orthogonality is open work.

**Tier-A constructs covered.** DDA (#1), cross-embedder interaction feature (#15).

**Essential questions answered.** Q04, Q05.

**Combined structural hole filled.** HOLE-1.

**Confidence.** HIGH.

---

## METHOD-GAP-3. No multi-clip centroid aggregation as an alignment target.

**Source clusters & papers.**
- Cluster E: wang2023valle, chen2024valle2, tan2024naturalspeech3,
  wang2024maskgct, chen2024f5tts, jiang2023megatts2, casanova2024xtts,
  shen2023naturalspeech2, le2023voicebox all condition on a **single reference
  utterance** or an arbitrary-length single prompt.
- Cluster F: meng2025echomimicv3, cui2024hallo3, li2024latentsync,
  zheng2025hunyuanvideo, tian2024emo, tian2025emo2, yang2024cogvideox condition
  on **reference images** (+ audio) through the generator's conditioning pathway,
  not on aggregated per-modality centroids.

**What's missing methodologically.** The "L2-normalised mean of L2-normalised
per-clip embeddings, held frozen" operation is a trivially available arithmetic
step on top of the cited embedders. Yet **no Cluster-E or Cluster-F paper applies
it as an alignment target**. Conditioning remains on single-prompt / single-image.

**What our paper adds.** **§5.1 centroid-construction phase.**

- Case 3: 50-clip reference set → WavLM speaker-centroid (T1).
- Case 2: per-modality centroids over the 2,362-clip corpus in 7 modalities (T2
  pipeline; T3 runtime cosines).
- Case 1: text-modality centroid from cleaned-Shakespeare references (T3
  projected).

**Tier-A constructs covered.** Centroid (#7), constellation (#8), reference set
(#9), frozen (#10), TCT (#3).

**Essential questions answered.** Q07, Q08, Q12, Q15.

**Combined structural hole filled.** HOLE-4.

**Confidence.** HIGH (explicit across 13 cited TTS/video papers).

---

## METHOD-GAP-4. No runtime per-output cosine-gate against a frozen centroid.

**Source clusters & papers.**
- Cluster D (retrieval): embedders are similarity scorers used for ranking, not
  runtime acceptance gates.
- Cluster E (TTS): SIM-o / WavLM-SECS are reported **after generation on the
  whole test set**. chen2024valle2, tan2024naturalspeech3, wang2024maskgct,
  chen2024f5tts — none resamples against a per-utterance cosine threshold at
  inference.
- Cluster F (video): CSIM / LSE-D are reported post-hoc; cui2024hallo3,
  meng2025echomimicv3, li2024latentsync, zheng2025hunyuanvideo — none applies
  a runtime per-modality cosine gate.
- Cluster I (alignment): ouyang2022instructgpt, rafailov2023dpo,
  bai2022constitutional use a scalar preference reward and have **no per-output
  inference-time predicate**.

**What's missing methodologically.** The full procedure (re-embed generated
output through the same frozen embedders used to build the alignment target;
compute per-modality cosine to centroid; accept iff ≥ τ_m for all modalities) is
absent across four clusters simultaneously.

**What our paper adds.** **§5.3 constellation guard** (glossary #12) — the
inference-time half of TCT. Instantiated in:
- Case 3: WavLM-scored best-of-12 at T=0.3 against the 50-clip speaker centroid
  (T1 — 0.961 mean SECS demonstrates the guard's operating regime).
- Case 2: projected five-modality guard (T3 pending).
- Case 1: projected text-modality guard (T3 pending).

This is **methodologically novel across D + E + F + I** (four clusters).

**Tier-A constructs covered.** TCT (#3), identity-locked generation (#4),
constellation guard (#12), per-modality threshold (#11).

**Essential questions answered.** Q06, Q07, Q09, Q15.

**Combined structural hole filled.** HOLE-3.

**Confidence.** HIGH.

---

## METHOD-GAP-5. No cross-encoder SECS protocol in TTS evaluation.

**Source clusters & papers.**
- Cluster E: chen2024valle2 reports SIM-o using a WavLM-based speaker encoder;
  VALL-E 2 was itself conditioned on neural-codec speaker tokens (not WavLM),
  so SIM-o partially qualifies as cross-encoder — but this is incidental, not
  methodologically named. tan2024naturalspeech3, wang2024maskgct rotate scorers
  within the same speaker-encoder family, risking correlated bias.
- No Cluster-E paper defines "cross-encoder SECS" as the paradigm where the
  scoring encoder is **architecturally independent** of any encoder used during
  generation.

**What's missing methodologically.** A named convention for choosing scorer
encoders so that the TTS system's internal speaker representation is **not** the
scorer. The SIM-o convention evolved organically; the cross-encoder guarantee is
not explicit.

**What our paper adds.** **§6.3 Case 3 Methods** explicitly names cross-encoder
SECS (glossary #20) and enforces it: Qwen3-TTS conditioning uses Qwen3's own
speaker-embedder; scoring uses microsoft/wavlm-base-plus-sv. Different
architectural families; independent training data. §8 Limitations carries the
V20 first-use disambiguation from the IR cross-encoder sense.

**Tier-A constructs covered.** SECS (#16), cross-encoder SECS (#20).

**Essential questions answered.** Q12, Q14, Q20.

**Combined structural hole filled.** HOLE-3 (adjacent: strengthens the guard
credibility).

**Confidence.** MEDIUM-HIGH (the structural absence is subtle; competent
Cluster-E reviewers may argue VALL-E 2's SIM-o already qualifies. Our §8 must
preempt this by stating the cross-encoder definition explicitly.)

---

## METHOD-GAP-6. No WavLM SECS same-session-band calibration precedent.

**Source clusters & papers.**
- Cluster E: speaker-encoder papers (chen2022wavlm, desplanques2020ecapa) report
  EER distributions on VoxCeleb1-O / NIST-SRE — aggregate discrimination across
  many speakers. **No published band for a single speaker's within-session
  pairs.**
- No Cluster-E paper uses a same-session band to calibrate a runtime τ_m.

**What's missing methodologically.** The 0.95–0.99 range we observe on Case 3's
speaker's own recordings is the **encoder's discrimination ceiling for that
speaker on that recording session**. No cited paper publishes this quantity for
any speaker.

**What our paper adds.** **§6.3** reports the 0.95–0.99 band as **authors' own
empirical observation** (glossary #17, V12 hedge mandated). Uses the lower edge
to calibrate τ_speaker for Case 3 only. **We do not claim this is universal.** §8
Limitations frames the band as per-speaker-per-session.

**Tier-A constructs covered.** Same-session verification band (#17).

**Essential questions answered.** Q08, Q14.

**Combined structural hole filled.** HOLE-5 (adjacent; forensic-transparency
lineage).

**Confidence.** HIGH (declared authors' observation; no citation required;
transparency-forward).

---

## METHOD-GAP-7. Flow-matching sign-convention forensic is undocumented.

**Source clusters & papers.**
- Cluster F: lipman2023flowmatching (the foundational paper) states the
  probability-path objective but does not highlight the sign as a reproducibility
  hazard. meng2025echomimicv3 and wan2025video inherit the convention
  (target = noise − latent) without explicit warning. Flipping the sign causes
  anti-denoising.
- No paper we cite explicitly documents the bug pattern, the symptom, or the
  diagnostic.

**What's missing methodologically.** A reproducibility-forensic paragraph stating
the convention, the symptom if flipped, the diagnostic method, the fix, and the
post-fix verification.

**What our paper adds.** **§6.2 + §8.4** disclose the flow-matching sign bug by
reference to `docs/inspect/08_training_audit.md` in the Santa project (the
forensic file; per P10). Forensic report is a distinct evidence category
(glossary #29). Glossary #30 defines the sign convention with the explicit
hedge.

**Tier-A constructs covered.** Forensic report (#29), flow-matching sign
convention (#30), identity-locked generation (#4, because the bug gated
identity-lock quality).

**Essential questions answered.** Q13, Q17, Q18.

**Combined structural hole filled.** HOLE-5.

**Confidence.** HIGH.

---

## METHOD-GAP-8. Shakespeare training-data echo-bug forensic is undocumented.

**Source clusters & papers.**
- Cluster I (alignment): ouyang2022instructgpt, rafailov2023dpo document SFT +
  DPO pipelines but do not document a per-pipeline forensic on
  training-data-as-output-echo.
- Cluster C (compression) papers do not frame training-data integrity as a
  forensic concern.

**What's missing methodologically.** The specific class of bug where a language
model's output exactly echoes a training sample when a certain prompt feature is
present — commonly a result of dataset leakage or prompt-format reuse — has no
reproducible forensic template in cited work.

**What our paper adds.** **§6.1 Case 1** discloses the Shakespeare echo-bug
forensic: diagnostic method (15/15 FSV test set on a canary prompt; post-clean
re-run), the fix, and the post-fix verification (15/15 passes on the cleaned
data). The file is cited in-text as a forensic disclosure per P10, not as a
Cluster-I citation.

**Tier-A constructs covered.** Forensic report (#29), identity-locked generation
(#4; at text-modality).

**Essential questions answered.** Q13, Q17, Q18.

**Combined structural hole filled.** HOLE-5.

**Confidence.** HIGH.

---

## METHOD-GAP-9. Sample-complexity collapse from "thousands of subject-specific examples" to "one reference set per modality + frozen centroids" is not measured in cited work.

**Source clusters & papers.**
- Cluster E: all cited systems assume a *single reference prompt* → full TTS
  capability; sample complexity is effectively 1-shot. But **no paper quantifies
  the delta between 1-shot and our 50-clip centroid** on the same subject with
  the same guard — there's no experiment to compare.
- Cluster F: EchoMimicV3 / Hallo3 / EMO condition on tens of reference images;
  no paper measures sample-complexity at the centroid level (clips-per-modality)
  under a runtime guard.
- Cluster I: scalar-reward alignment assumes tens of thousands of preference
  pairs; no paper evaluates sample complexity under a geometric-alignment
  alternative.

**What's missing methodologically.** A sample-complexity measurement *under the
TCT regime* — how many reference clips per modality, under what guard, to reach
which per-modality cosine.

**What our paper adds.** **§5.4 or §3.6** states the sample-complexity claim
(mission brief core claim 4): "identity-locked generation collapses from
thousands of subject-specific examples to one reference set per modality +
frozen centroids." Case 3 empirically instantiates this at the speaker modality
(50 clips → 0.961 mean WavLM SECS); Case 2 and Case 1 project similar collapses
(T3). **The full experimental measurement of sample-complexity vs. accept-rate
curves is open work (§10 / Q20).**

**Tier-A constructs covered.** DDA (#1), TCT (#3), identity-locked generation
(#4), reference set (#9), constellation (#8).

**Essential questions answered.** Q15.

**Combined structural hole filled.** HOLE-3, HOLE-4.

**Confidence.** MEDIUM (Case 3's 50-clip → 0.961 is a strong point datum, but
a complete sample-complexity curve is not in this paper).

---

## Consolidated gap-map table

| # | Gap | Tier-A construct(s) | Primary Q-IDs | HOLE | Primary clusters |
|---|---|---|---|---|---|
| METHOD-GAP-1 | Signals-per-input unmeasured | DDA, MC | Q04, Q05 | 1, 2 | A, C, D |
| METHOD-GAP-2 | No pairwise-interaction feature | DDA, #15 | Q04, Q05 | 1 | C, D |
| METHOD-GAP-3 | No multi-clip centroid aggregation | TCT, #7, #8, #9, #10 | Q07, Q08, Q12, Q15 | 4 | E, F |
| METHOD-GAP-4 | No runtime per-output cosine-gate | TCT, #12, #11 | Q06, Q07, Q09, Q15 | 3 | D, E, F, I |
| METHOD-GAP-5 | No cross-encoder SECS protocol | #16, #20 | Q12, Q14, Q20 | 3 | E |
| METHOD-GAP-6 | No same-session band precedent | #17 | Q08, Q14 | 5 | E |
| METHOD-GAP-7 | Flow-matching sign-bug forensic undocumented | #29, #30, IL | Q13, Q17, Q18 | 5 | F |
| METHOD-GAP-8 | Echo-bug forensic undocumented | #29, IL | Q13, Q17, Q18 | 5 | I |
| METHOD-GAP-9 | Sample-complexity under TCT unmeasured | DDA, TCT, IL | Q15 | 3, 4 | E, F, I |

## Coverage check

- **Every Tier-A construct** (DDA, meaning compression, TCT, identity-locked
  generation) is covered by at least one methodological gap.
- **Every Q-ID load-bearing for §5 Methods** (Q04–Q09, Q12–Q18) is covered.
- **Every combined structural hole** (HOLE-1 … HOLE-5) has at least one
  methodological gap filling it.
- **No construct-orphan; no question-orphan; no hole-orphan.**

## FM-03 (AGI overclaim) check

Every gap above is expressed as a **structural delta** ("the cited cluster does
not include X; we add X methodologically; here is where it manifests"). None
claims to "solve" the data wall or generation or alignment. **Pass.**

## FM-10 (citation padding) check

Every gap lists the specific cluster and papers whose methodology lacks the
feature. No generic "survey-wide" absence; every absence is enumerated. **Pass.**

## Confidence statement

**HIGH.** Nine methodological gaps identified, each evidence-grounded in the
master pool (88 citations), each traced to a Tier-A construct / Q-ID / structural
hole. The nine gaps align with the paper's four load-bearing claims (mission
brief §core claims 1–4) and with the five combined structural holes from
agent-06. No hole is left unanchored; no anchor is left orphaned.

## Changelog

- 2026-04-16 — Initial issue by agent-09-methodology-scanner.
