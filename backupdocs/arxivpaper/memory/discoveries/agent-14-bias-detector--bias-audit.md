---
namespace: discoveries
agent_id: agent-14-bias-detector
timestamp: 2026-04-16T23:58:00Z
status: active
locked: false
supersedes: none
tags: bias-audit, publication-bias, selection-bias, affiliation-bias, single-subject, self-reporting, temporal-bias, structural-limitations
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md
  - memory/patterns/agent-11-systematic-reviewer--evidence-grading.md
  - memory/discoveries/agent-11-systematic-reviewer--cluster-biases.md
  - memory/discoveries/agent-13-quality-assessor--quality-flags.md
  - memory/handoff/agent-13-to-14--quality-done.md
downstream_consumers:
  - agent-15-evidence-synthesizer
  - agent-29-validity-guardian
  - agent-36-related-work-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
---

# Systematic Bias Audit — 10 Axes × (Citation Pool | Paper Claims)

## Method

For each of the 10 bias axes in the task brief, agent-14 records:

1. **What it is** — definition grounded in meta-science practice.
2. **Is it present?** — YES / PARTIAL / NO, with severity triage.
3. **Evidence** — quantified where possible (source counts, affiliations,
   temporal distribution, measurement protocol).
4. **Scope** — does the bias sit in (a) the 88-citation pool, (b) the paper's
   empirical claims, or (c) both?
5. **Mitigation** — concrete action assigned to §2 / §6 / §7 / §8 writers.

Statistical tests (Egger, trim-and-fill, fail-safe N) are **not applicable**
here. This pool is a narrative / position manuscript with a 3-case empirical
core, not a meta-analysis of comparable effect sizes. Egger-style asymmetry
tests require ≥ 10 independent studies measuring the same outcome with SEs; our
pool spans 9 clusters of heterogeneous tasks (scaling, collapse, compression,
retrieval, TTS, video, pipeline, market, alignment). The appropriate bias
apparatus is **qualitative / structural**, patterned on PRISMA /
AMSTAR-2 / ROBIS, not statistical-forest-plot methodology. This distinction is
itself recorded in §8 Limitations.

Severity scheme:

- **CRITICAL.** Triggers a §8 Limitations line + a §6 caveat + potentially a §7
  acknowledgement. Omitting it invites reviewer rejection.
- **HIGH.** Triggers a §8 Limitations line.
- **MODERATE.** Mentioned in §2 / §6 caveat text where the cluster is cited.
- **LOW.** Footnote-level; no mandatory prose.

---

## Bias-01 — Publication bias (positive-results-only in TTS / video citations)

### What it is
Tendency for successful / positive / SOTA-framed results to be published,
while null, failed, or below-SOTA approaches remain unpublished or are buried
in appendices. In a literature-driven §2, this biases the frontier upward.

### Is it present?
**YES — HIGH.** Concentrated in Clusters E (TTS) and F (identity-locked
video).

### Evidence
- Agent-11 cluster-bias audit flagged L-Bias-6 = **HIGH** for both Cluster E
  and Cluster F: "TTS papers almost universally report positive results on
  comparator benchmarks; failure analyses are rare in published venues"
  (agent-11 cluster E entry).
- VALL-E 2 claims "human parity" with CMOS ≈ 0 — a promotional frame;
  community has not independently replicated at method level (agent-13
  FLAG-1, RED, score 4/10).
- EchoMimicV3's flow-matching sign-convention bug (CL-21) was
  **author-discovered during Case 2 forensics** — the base paper does not
  document it. This is a concrete instance of a failed / under-specified
  aspect of a published paper being absent from the published record
  (agent-13 FLAG-2, AMBER, score 6/10).
- 9 of 13 Cluster-E refs are 2023–2025; 10 of 12 Cluster-F refs are 2024–2025.
  The "frontier" frame is recency-compressed (see Bias-10).
- No null-result TTS / video paper cited in our pool. Our §2.5 frontier
  narrative ("SIM-o ~0.64 / WavLM-cosine ~0.86–0.89") inherits publication
  bias from the cluster.

### Scope
**Citation pool (Clusters E + F)** and **paper's §2.5 framing**.

### Mitigation
- **§2.5 writer (agent-36).** Add one sentence: "TTS and identity-locked-video
  literature at the time of this writing reports predominantly positive
  results on internal benchmarks; we do not have access to independently
  replicated failure analyses and treat the 'SOTA frontier' numbers as
  cluster-typical upper bounds rather than verified comparisons."
- **§6.3 writer (agent-38).** When citing VALL-E 2 SIM-o 0.64, use
  agent-13 FLAG-1 caveat verbatim (different encoder, different test
  pool, different cohort composition).
- **§6.2 writer (agent-38).** Sign-convention forensic is **the** disclosure
  the field lacks — frame it as a contribution (per agent-13 FLAG-2),
  not as an embarrassing finding.
- **§8 Limitations (agent-40).** Include explicit statement: "Our frontier
  comparator numbers in §2.5 inherit publication bias of their source
  clusters; null / failure-mode TTS and video results are
  under-represented in the peer-reviewed record and thus in our pool."

---

## Bias-02 — Selection bias in citations (cherry-picking citations that support our claims)

### What it is
Systematic preference for citing sources aligned with the manuscript's thesis,
while downweighting or omitting sources that tension or contradict it. In a
position paper, the temptation is strong: pick the anchors that make the
contrast clean.

### Is it present?
**PARTIAL — MODERATE.** The pool is constructed to *support* the TCT / DDA
/ meaning-compression trio, but agent-08 + agent-11 did include critical /
tensioning refs (model-collapse lineage, RLHF-as-scalar-reward triad).

### Evidence
- Cluster B (model collapse) is **intentionally included to tension DDA**
  (agent-10 framework-map): if DDA were just synthetic-data recycling,
  model-collapse arguments would apply. The cluster is cited so that we
  can explicitly state why they don't (CL-06, MODERATE categorical).
  This is the opposite of cherry-picking — we cite what *could* contradict
  us. **Good methodological practice.**
- Cluster I (alignment) is cited as a 3-paper US-only sub-corpus to
  establish the *scalar-reward paradigm TCT departs from* (CL-14, STRONG
  for contrast). Agent-11 L-Bias-2 flagged this as "VERY HIGH" lab
  concentration, but also noted: "adequate for the Q06 BRK contrast (we
  *want* the scalar-reward paradigm to be tightly defined so TCT can
  depart from it)".
- Selection risk remains in Cluster E / F: we cite the *frontier* papers
  (VALL-E 2, NaturalSpeech 3, MaskGCT, EchoMimicV3) but **not** critical
  TTS / video replication work (e.g., third-party eval harnesses, eval
  audits, community reproducibility failures). Partially structural —
  such work is thin on the ground, per Bias-01 — but also reflects
  pool-construction choice.
- KTO, IPO, ORPO, RLAIF variants are **not** cited in Cluster I. Agent-11
  explicitly flags this as a scope boundary. The selection is defensible
  (Q06 needs tight paradigm), but a reviewer might reasonably call it
  cherry-picking if the paper claims to survey "alignment broadly".

### Scope
**Citation pool construction** — but partially mitigated by
cluster-construction itself (agent-06, agent-11).

### Mitigation
- **§2.6 writer (agent-36).** Scope the alignment discussion explicitly:
  "We cite the three canonical scalar-reward anchors (RLHF, DPO, CAI)
  because the TCT contrast is against their shared scalar-reward /
  preference-model primitive; we do not claim to survey the broader
  alignment landscape (e.g., RLAIF, KTO, IPO, ORPO)."
- **§2.5 writer (agent-36).** Do not claim "frontier review"; claim
  "representative frontier anchors". Preface with: "This section cites
  representative zero-shot TTS and identity-locked-video systems that
  share a common evaluation vocabulary (SIM-o, WER, DNSMOS, UTMOS) and
  anchor the numerical comparator space for §6.3. It is not a
  systematic review of the field."
- **§8 Limitations (agent-40).** "Our citation pool is thesis-aligned by
  design: clusters are selected to make DDA / TCT / meaning-compression
  contrasts legible. Model-collapse (Cluster B) is included as explicit
  tension; other tensioning literatures (e.g., reward-modeling
  extensions beyond RLHF / DPO / CAI) are out of scope."

---

## Bias-03 — Industry-lab / affiliation bias (Microsoft-, Alibaba-, OpenAI-heavy pool)

### What it is
Citation concentration around a small number of well-resourced corporate
labs. These labs publish frequently, get covered heavily, and become
default "frontier" references — crowding out academic or smaller-lab work
and systematically biasing toward *closed-artefact* narratives.

### Is it present?
**YES — CRITICAL.** This is the pool's strongest bias signal, and agents 11
and 13 both flagged it independently.

### Evidence
- **Cluster E (TTS).** Microsoft or Alibaba account for ~50% of cluster E
  papers (agent-11, L-Bias-2 HIGH). Four out of the TTS frontier citations
  (wang2023valle, chen2024valle2, tan2024naturalspeech3,
  shen2023naturalspeech2) are Microsoft.
- **Cluster F (video).** Chinese labs (Alibaba + Tencent + ByteDance +
  Fudan + Tsinghua) account for ~85% of cluster F papers; Alibaba alone
  accounts for meng2025echomimicv3, tian2024emo, tian2025emo2, wan2025video
  (agent-11, L-Bias-2 VERY HIGH).
- **Cluster I (alignment).** OpenAI, Stanford, Anthropic — all US, all
  dominant alignment players; 3 refs total (agent-11, L-Bias-2 VERY HIGH).
- **Hot-tier scored citations (agent-13).** Of 18 hot-tier refs, ~6 are
  Microsoft / Alibaba / Anthropic closed-artefact releases. All 4 RED
  / AMBER flags originate from this category (VALL-E 2, EchoMimicV3,
  Constitutional AI, NaturalSpeech 3).
- **Reproducibility correlation.** Industry-lab releases score 0/2 on
  reproducibility in the 5-dim rubric in 4/4 flagged cases — **affiliation
  bias and closed-artefact bias are systemically coupled** in this
  pool. Agent-13 explicitly calls this "a single systemic pattern".

### Scope
**Citation pool** AND **paper's framing of "the frontier"**. If we say
"the TTS frontier is ~0.64 SIM-o / ~0.86–0.89 WavLM-cosine", we are
citing Microsoft's internal numbers.

### Mitigation
- **§2.5 (agent-36, TTS).** Pair each Microsoft citation with at least
  one open-source comparator in the same paragraph: wang2024maskgct
  (CUHK-Shenzhen), chen2024f5tts (Shanghai Jiao Tong), casanova2024xtts
  (Coqui), le2023voicebox (Meta-partial-open).
- **§2.5 (agent-36, video).** Balance Alibaba-heavy citations with
  cui2024hallo3 (Fudan-Baidu), zheng2025hunyuanvideo (Tencent),
  yang2024cogvideox (Tsinghua-Zhipu), li2024latentsync (ByteDance).
- **§2.6 (agent-36, alignment).** State the 3-paper US-only scope
  explicitly.
- **§8 Limitations (agent-40).** Required paragraph:
  > "Our citation pool shows lab-concentration bias in three clusters:
  > TTS (Microsoft × 4), identity-locked video (Alibaba × 4), and
  > alignment (US-only × 3). We mitigate by pairing closed-artefact
  > industry citations with open-source comparators where possible, and
  > by reporting our Case 3 numbers on a fully open scorer (WavLM) over
  > releasable audio. We do not claim the cited frontier captures the
  > full field, only the most-referenced systems at the time of
  > writing."
- **§7 Discussion (agent-39).** When discussing comparability, explicitly
  note that closed-artefact industry baselines are not directly
  comparable to our open-scorer measurements.

---

## Bias-04 — English-only / cross-language bias

### What it is
Inclusion criterion restricted to English-language sources; non-English
(Chinese, Japanese, Korean, German, French, Arabic, etc.) work is excluded.
In TTS / video specifically, Chinese-language papers and evaluation corpora
are a substantial part of the field.

### Is it present?
**YES — MODERATE to HIGH.** Structural feature of PRISMA-lite scope
(agent-11 L-Bias-1 GLOBAL).

### Evidence
- All 88 pool citations are English-language (agent-11 L-Bias-1).
- Several Cluster E / F citations are *by* Chinese-lab authors writing in
  English (chen2024valle2, wang2023valle, chen2024f5tts,
  meng2025echomimicv3, etc.) — so author-language is not the same as
  *citation* language. But original Chinese-language TTS / video
  publications and Chinese-language evaluation benchmarks (e.g.,
  AISHELL variants, WenetSpeech) are not in the pool.
- Our Case 3 voice-cloning evaluation is conducted **in English only** on
  a single English-speaking subject (the author). Non-English speaker
  cloning, non-English prosody, tonal-language cloning (Mandarin,
  Cantonese) are out of scope of our evaluation.

### Scope
**Citation pool + Case 3 evaluation protocol.**

### Mitigation
- **§8 Limitations (agent-40).** Required line: "The 88-citation pool is
  English-language only (PRISMA-lite inclusion criterion I4). Case 3's
  voice-cloning evaluation is English-only on a single English-speaking
  subject. We make no cross-language generalisation claims; tonal
  languages in particular have distinct acoustic and prosodic
  requirements that our protocol does not test."
- **§6.3 writer (agent-38).** When stating the 0.961 SECS result, scope
  it: "on 10 unseen English sentences from a single English-speaking
  subject".
- **§2.5 writer (agent-36).** One-sentence acknowledgement: "Non-English
  TTS literature (including substantial work on Mandarin, Japanese, and
  Korean synthesis) is out of scope of this review."

---

## Bias-05 — Single-subject bias in Case 2 and Case 3 (n=1 subject)

### What it is
A case study with n = 1 subject cannot separate subject-specific
idiosyncrasies from method-general performance. This is a structural
(not remediable) feature of Case 2 (Santa, single interview subject) and
Case 3 (voice, the author as subject). It is **not** a flaw if scoped
correctly; it **is** a critical flaw if the paper presents the numbers as
generalising across subjects.

### Is it present?
**YES — CRITICAL. STRUCTURAL.** Cannot be mitigated empirically before
submission; can only be disclosed.

### Evidence
- **Case 2 (Santa).** 2,362 curated clips from 16 min of interview of a
  single subject. Training data diversity is clip-level, not
  subject-level.
- **Case 3 (voice).** The author is the subject. 50-clip reference
  centroid, 10 held-out sentences, the same speaker for reference and
  held-out (agent-01 FM-05, CRITICAL). Mission brief §3 Case 3 repeats
  this: "the author is also the subject".
- No confidence interval is computable across speakers because there is
  only one speaker. Within-subject variance across the 10 held-out
  sentences is computable and is the only statistic we have.
- **Case 1 (Shakespeare)** is text-style, not identity — the
  "single-subject" frame does not apply. Case 1 uses a corpus of
  Shakespearean works, not a single author persona in the identity-lock
  sense.

### Scope
**Paper's empirical claims — Cases 2 and 3.**

### Mitigation
- **§6.2 Case 2 opening (agent-38) — mandatory scope statement.**
  > "Case 2 trains on 2,362 curated clips from a single 16-minute
  > interview of one subject. The case establishes that the EchoMimicV3 +
  > LoRA pipeline runs end-to-end at this data scale and that the
  > flow-matching sign-convention issue can be forensically diagnosed and
  > fixed; it does not claim generalisation to other subjects."
- **§6.3 Case 3 opening (agent-38) — mandatory scope statement,
  verbatim from agent-01 FM-05.**
  > "One speaker (the author), 50-clip reference centroid, 10 held-out
  > sentences, WavLM-SECS-scored best-of-12 at T=0.3; DNSMOS / UTMOS
  > parity computed against the same speaker's real recordings. We do
  > not generalize beyond this protocol; see §8."
- **§8 Limitations (agent-40).** Required paragraph:
  > "Case 2 and Case 3 are n = 1 subject studies. Case 2's training
  > corpus is drawn from a single interview; Case 3's reference and
  > held-out sets share a single speaker (the author). Single-subject
  > evaluations are a structural feature of our current artefacts and
  > are not a substitute for multi-speaker evaluation; we report
  > numbers with this scope bounded and do not advance population-
  > level claims."
- **§7 Discussion (agent-39).** Do not make cross-speaker claims.

---

## Bias-06 — Single-encoder (WavLM-only) bias in Case 3

### What it is
Case 3's SECS (Speaker Encoder Cosine Similarity) metric uses a *single*
speaker encoder — WavLM-large (chen2022wavlm). Other speaker encoders
(ECAPA-TDNN, Resemblyzer, x-vector, d-vector) compute different cosine
similarities on the same audio pairs. A system optimised against WavLM
may score lower against another encoder; conversely, a weaker system may
score artificially high on WavLM alone.

### Is it present?
**YES — HIGH. STRUCTURAL (for the reported numbers), but methodologically
mitigable going forward.**

### Evidence
- The Case 3 protocol uses WavLM-SECS-scored best-of-12 at T=0.3 *both*
  as the training-time selection criterion *and* as the reported
  evaluation metric. This creates an **in-the-loop encoder** situation:
  the metric that decides which generation to keep is also the metric
  that is reported.
- WavLM-large is T1-rated as an encoder (chen2022wavlm, agent-07) — it
  is not a weak encoder. But SECS ranges and thresholds are
  encoder-specific; a 0.961 WavLM cosine does not translate to a 0.961
  ECAPA cosine.
- Cluster E comparators (VALL-E 2, NaturalSpeech 3) report *different*
  encoders / metrics (SIM-o is often WavLM-TDNN or a speaker-encoder
  from X-vector). Agent-13 FLAG-1 already notes this: "different
  speaker embedding extractors".
- Our 0.961 number is reported without a second-encoder cross-check.
  DNSMOS / UTMOS parity is provided, but these are *naturalness*
  scorers, not speaker-similarity scorers.

### Scope
**Paper's empirical claim in Case 3 (CL-16, CL-17, CL-18).**

### Mitigation
- **§6.3 writer (agent-38).** Add explicit encoder-scope:
  > "SECS is computed on WavLM-large (Chen et al., 2022). We do not
  > report cross-encoder robustness at this time; the 0.961 mean is
  > specific to this encoder and should not be read as a universal
  > speaker-similarity guarantee. A second-encoder audit (ECAPA-TDNN
  > or Resemblyzer) is left to a follow-up."
- **§8 Limitations (agent-40).** Required line: "Case 3 SECS is
  reported on a single encoder (WavLM-large). Encoder-specific
  similarity scales are not directly comparable to other encoders'
  scales used elsewhere in the TTS literature."
- **§7 Discussion (agent-39).** When discussing the '0.95 barrier'
  framing, caveat: "encoder-specific; the figure reflects WavLM-large
  distance distribution and not an absolute speaker-identity
  guarantee."
- **Optional follow-up (not a §2–§9 blocker).** Cross-encoder audit is
  a clear next-step task; can be mentioned in §9 Conclusion /
  Future Work.

---

## Bias-07 — Confirmation bias in forensic narratives (post-hoc rationalising 17 failed approaches)

### What it is
When a paper documents bugs found and fixed, there is a temptation to
frame each fix as *the* correct move, obscuring that the fixes were
discovered after the fact and may be only one of several valid
resolutions. Post-hoc rationalisation can present engineering trial-and-
error as if it were principled derivation.

### Is it present?
**PARTIAL — MODERATE.** Forensic narratives (echo-bug, sign-convention
bug) are genuine author-discovered issues documented in inspect/
reports; however, the *framing* in §6.1 / §6.2 must avoid retcon.

### Evidence
- **Case 1 Shakespeare echo-bug.** Mission brief §3 Case 1:
  "forensically-diagnosed training-data echo-bug documented". 15/15 FSV
  passed *after* the echo-bug was fixed (agent-01 FM-08 counter-behavior:
  disclose the bug; don't present passing tests without the forensic
  context).
- **Case 2 Santa flow-matching sign-bug.** Mission brief §3 Case 2:
  "forensically-identified flow-matching sign-bug documented". CL-21
  GRADE is AUTHOR-EMPIRICAL (forensic). Agent-13 FLAG-2 notes the base
  paper (EchoMimicV3) does not document the sign convention — the
  author's discovery is the disclosure.
- The task brief references "17 failed approaches". These are real
  engineering dead-ends during development (per mission brief's
  reference to the prior researchpaper). Presenting them as 17 steps
  of *principled elimination* would be retcon; presenting them as
  "engineering trial-and-error that converged on the current stack" is
  honest.
- Risk concentrates in §6 prose: if agent-38 writes "we systematically
  eliminated 17 alternatives", that is retcon. If agent-38 writes
  "17 earlier approaches were attempted and discarded during
  development; the Forensics and corrections subsection describes the
  two representative diagnostic episodes we present as Case 1 and
  Case 2 empirical evidence", that is accurate.

### Scope
**Paper's §6 forensic narratives — framing risk, not evidentiary risk.**
The bugs *are* documented; the question is how they are described.

### Mitigation
- **§6.1, §6.2 writer (agent-38).** Write forensics as narrative
  ("during development we observed X; diagnosis produced Y; fix was
  Z; subsequent FSV passed") rather than as systematic elimination
  ("we systematically tested 17 approaches and selected the optimal").
- **§6 opener (agent-38).** Single disclosure line: "The forensic
  subsections in §6.1 and §6.2 describe bugs discovered during
  development and the diagnostic methods that identified them. They
  are presented as engineering evidence, not as systematic ablation
  studies; alternative fixes may exist."
- **§8 Limitations (agent-40).** Optional line (if §6 prose trends
  retcon): "Our forensic descriptions in §6.1 and §6.2 are post-hoc
  accounts of engineering diagnosis and fix; they are not randomised
  ablation studies and do not enumerate all valid corrections."
- **Agent-42 adversarial review.** Regex / read-through for retcon
  tropes ("we systematically", "eliminated all alternatives",
  "principled derivation"). Flag for rewrite.

---

## Bias-08 — Self-reporting bias (all Case 1/2/3 numbers are author-reported, not third-party replicated)

### What it is
Empirical numbers reported by the system's author / developer are
susceptible to (i) inadvertent test-set leakage, (ii) selection of the
best run without reporting the distribution of runs, (iii) tuning the
evaluation protocol to the system, (iv) conflict of interest in framing.
Third-party replication is the standard correction but is often not
available at submission time for novel systems.

### Is it present?
**YES — CRITICAL. STRUCTURAL.** All Case 1 / 2 / 3 numbers are author-
reported; no independent replication exists because the systems are the
author's own.

### Evidence
- **Case 1.** Shakespeare SFT+DPO in progress at submission time
  (mission brief §3); 15/15 FSV passed — author-reported.
- **Case 2.** 2,362 curated clips, mouth-weighted flow-matching loss,
  forensic sign-bug — author-reported.
- **Case 3.** 0.961 mean SECS, 0.975 max SECS, 3.93 DNSMOS parity,
  3.012 UTMOS parity — author-measured on author's system on author's
  data. Mission brief labels this "the ONLY fully-measured case study"
  but that *measured* is still *author-measured*.
- Agent-11 GRADE for CL-16 / CL-17 / CL-18 is **AUTHOR-EMPIRICAL** —
  which *is* the self-reporting disclosure, but only if writers use the
  correct hedge text (V12 / V17 / V22, per agent-02 glossary).
- No third-party lab has reproduced 0.961 SECS using our pipeline as of
  submission time. The pipeline (50-clip centroid + WavLM
  best-of-12 + T=0.3) is specified; reproducibility is architecturally
  possible; empirical replication has not yet occurred.

### Scope
**Paper's empirical claims — all of Case 1, Case 2, Case 3.**

### Mitigation
- **All §6 subsections (agent-38).** Required attribution phrasing per
  agent-11 rule 4: "we measured / we observed / our Case 3 yields" —
  never "is" or "does" in unqualified voice. Every AUTHOR-EMPIRICAL
  number gets V12 ("observed in this study"), V17 ("not generalised"),
  V22 ("single-site") hedges.
- **§6.3 writer (agent-38).** Required sentence: "The SECS, DNSMOS, and
  UTMOS numbers in §6.3 are author-measured on the author's pipeline
  using open-source scorers; they have not been independently
  replicated at the time of submission. We release the evaluation
  audio to enable replication."
- **§8 Limitations (agent-40).** Required paragraph:
  > "All numerical results in §6 (Case 1 FSV pass rates, Case 2
  > forensic findings, Case 3 SECS / DNSMOS / UTMOS) are author-measured
  > on author-built pipelines and have not been independently
  > replicated at submission time. We mitigate the self-reporting
  > concern by (i) using open-source scorers where possible (WavLM,
  > DNSMOS, UTMOS), (ii) specifying the evaluation protocol in full
  > so the measurement is reproducible, and (iii) releasing held-out
  > evaluation audio. We acknowledge that third-party replication is
  > the next step and not yet in hand."
- **§9 Conclusion / Future Work (agent-39).** Signal that third-party
  replication is a named follow-up.

---

## Bias-09 — Benchmark bias (LibriSpeech, VoxCeleb may not reflect real-world speaker diversity)

### What it is
Standard TTS / speaker-verification benchmarks (LibriSpeech, VoxCeleb,
LibriTTS, AISHELL) are convenience corpora. LibriSpeech is audiobooks
read by volunteer readers — non-spontaneous, clean-audio, literary-
register. VoxCeleb is celebrity interview clips — noisy but
English-heavy and Western-celebrity-skewed. Real-world speaker
diversity (accents, dialects, recording conditions, emotional range,
code-switching) is under-represented.

### Is it present?
**YES — HIGH. Structural to the TTS comparator literature.**

### Evidence
- Cluster E comparators (VALL-E 2, NaturalSpeech 3, MaskGCT, Voicebox)
  all report on LibriSpeech-test-clean and LibriSpeech-dev-clean. This
  is audiobooks by trained volunteers.
- Our Case 3 does NOT use LibriSpeech — we use 10 author-read held-out
  sentences. This is arguably closer to real-world voice-cloning
  (one specific target speaker) but still far from population-level
  diversity (dialects, age range, health state, code-switching).
- DNSMOS (reddy2022dnsmos) and UTMOS (saeki2022utmos) are scorer
  models trained on specific noise + naturalness datasets with
  their own population skews.
- No cross-accent, cross-dialect, cross-age-group test reported in
  our pool for any TTS system — this is cluster-wide, not a defect of
  our evaluation specifically.

### Scope
**Citation pool (Cluster E benchmark monoculture)** AND **paper's
Case 3 evaluation protocol (single-subject, clean-audio,
American-English reads)**.

### Mitigation
- **§2.5 (agent-36).** Add: "TTS benchmarks in this literature are
  dominated by LibriSpeech and VoxCeleb derivatives, which are clean-
  audio convenience corpora with limited dialect, accent, and recording-
  condition diversity. Comparator numbers inherit these scope
  boundaries."
- **§6.3 (agent-38).** Add: "Our evaluation corpus (10 held-out
  sentences read by the author in clean recording conditions) is not
  a population-level speaker-diversity test. It tests whether the
  pipeline can exceed a published similarity barrier under its own
  specified protocol, not whether the method generalises across
  accents, dialects, or noise conditions."
- **§8 Limitations (agent-40).** Required line: "Case 3 uses clean-
  audio author-read sentences; we do not evaluate on LibriSpeech,
  VoxCeleb, or population-level diversity benchmarks. Cross-
  benchmark comparison is structurally limited for this reason."

---

## Bias-10 — Temporal bias (industry-signal citations 2024Q4–2025Q1, recency compression)

### What it is
Over-representation of a narrow time window can inflate a narrative
(the "diminishing returns" / "data wall" story) that was specifically
crystallised in industry discourse during that window. Subsequent
discourse (Q2 2025 onward) may have corrected, complicated, or walked
back the framing.

### Is it present?
**YES — MODERATE.** Concentrated in Cluster A (T3 industry signals) and
Cluster F (video, 10/12 refs from 2024–2025).

### Evidence
- **Cluster A T3 signals.** All 5 industry-reporting T3 sources
  (franzen, metz, hu, woo, sutskever) cluster around November 2024
  neurips sutskever oral → Dec 2024 coverage. Agent-11 L-Bias-5
  MODERATE for A; L-Bias-6 HIGH (inter-source citation loop — press
  cross-references itself). **This is one dated narrative moment, not
  five independent confirmations.**
- **Cluster F.** 10 of 12 refs from 2024–2025 (agent-11 L-Bias-5 VERY
  HIGH). The "identity-locked video" field as a named area
  crystallised post-Sora (2024); earlier foundational work
  (LoRA 2022, Flow Matching 2023, FLAME 2017, FACS 1978) grounds it,
  but the frontier crystallised in one year.
- **Cluster E.** 9 of 13 refs from 2023–2025; "very high" temporal
  concentration (agent-11 L-Bias-5). Similar post-VALL-E acceleration.
- Case 3 was **measured** in April 2026 (current date), using
  weights / tools / scorers available at that time. The measurement
  is properly dated; the *framing* around it (the "data wall",
  "diminishing returns") draws on the November 2024 moment.

### Scope
**Citation pool — Clusters A (T3 subset), E, F** AND **paper's
framing of the data-wall moment in §1 / §7**.

### Mitigation
- **§1 Introduction (agent-35 / agent-36).** Frame Cluster A T3
  signals as "a narrative moment" rather than "consensus":
  > "In late 2024, multiple industry observers reported diminishing
  > returns from pre-training-scale scaling (franzen 2024, metz 2024,
  > hu 2024, woo 2024, sutskever 2024). These reports cross-reference
  > each other and are best read as one narrative signal from that
  > period rather than as five independent confirmations. The T1 / T2
  > peer-reviewed literature on data-stock exhaustion (villalobos
  > 2024, longpre 2024) provides the evidential base."
- **§7 Discussion (agent-39).** Do not assert the data-wall framing
  as settled consensus; present it as the motivating context, hedged.
- **§8 Limitations (agent-40).** Required line: "Our Clusters E, F,
  and A-T3 are temporally compressed (most refs 2023–2025). The
  'frontier' and 'data-wall' framings reflect the discourse at the
  time of writing; subsequent literature may correct or complicate
  these framings."
- Agent-42 regex: flag "industry observers agree" or "consensus
  view" when the supporting citations are 2024 press.

---

## Summary table — 10 biases × severity × scope × mitigation site

| # | Bias | Present? | Severity | Scope | Primary mitigation site |
|---|---|---|---|---|---|
| 01 | Publication bias (positive TTS / video) | YES | HIGH | Pool + §2.5 framing | §2.5 caveat + §8 Limitations |
| 02 | Citation selection bias | PARTIAL | MODERATE | Pool construction | §2.5 / §2.6 scope + §8 |
| 03 | **Industry-lab / affiliation bias** | YES | **CRITICAL** | Pool + frontier framing | §2.5 pairing + §8 required ¶ + §7 |
| 04 | English-only bias | YES | MODERATE-HIGH | Pool + Case 3 protocol | §8 Limitations line |
| 05 | **Single-subject bias (Cases 2, 3)** | YES | **CRITICAL** STRUCTURAL | Empirical claims | §6.2 / §6.3 scope + §8 required ¶ + §7 |
| 06 | Single-encoder bias (WavLM-only, Case 3) | YES | HIGH STRUCTURAL | Case 3 claim | §6.3 scope + §8 + §7 |
| 07 | Confirmation bias (forensic retcon) | PARTIAL | MODERATE | §6 framing | §6 framing discipline + agent-42 regex |
| 08 | **Self-reporting bias (all cases)** | YES | **CRITICAL** STRUCTURAL | All empirical claims | §6 attribution + §8 required ¶ + §9 |
| 09 | Benchmark bias (LibriSpeech / VoxCeleb) | YES | HIGH | Cluster E + Case 3 scope | §2.5 + §6.3 + §8 |
| 10 | Temporal bias (2024Q4 narrative moment) | YES | MODERATE | Clusters A, E, F | §1 framing + §8 |

## Cross-axis pattern — the three structural features

**Bias-03 (industry-lab / affiliation) + Bias-05 (single-subject) +
Bias-08 (self-reporting) are the three biases that every reviewer will
raise first.** They are not remediable by re-citation or re-framing:
- Bias-03 is a field-wide problem that our pool mirrors honestly.
- Bias-05 is a design feature of our empirical artefacts: Case 2 is a
  single-subject video LoRA, Case 3 is the author as subject.
- Bias-08 is inherent to "this is a paper about systems I built" — the
  only cure is third-party replication, which is a follow-up.

The manuscript must **own** these three explicitly in §8 Limitations
with dedicated paragraphs, **repeat** the scope constraints in §6,
and **not oversell** in §7 Discussion. Biases 01, 02, 04, 06, 07, 09,
and 10 can be mitigated by careful writing and citation balance.

## Three biases that will have explicit Limitations text

1. **Bias-03 (industry-lab / affiliation).** Paragraph: "Our citation
   pool shows lab-concentration bias in three clusters ..."
2. **Bias-05 (single-subject Cases 2, 3).** Paragraph: "Case 2 and Case
   3 are n = 1 subject studies ..."
3. **Bias-08 (self-reporting).** Paragraph: "All numerical results in
   §6 are author-measured on author-built pipelines ..."

## Three biases that are structural features of our empirical claims

1. **Bias-05 single-subject.** By design — Case 2 is a single-interview
   subject; Case 3 is the author-as-subject. Cannot be fixed within
   this submission.
2. **Bias-06 single-encoder (WavLM).** SECS depends on WavLM-large;
   cross-encoder audit is a named follow-up.
3. **Bias-08 self-reporting.** Inherent to first-party measurement of
   a novel system; third-party replication is a post-submission task.

## What downstream writers must copy verbatim

Mitigation phrasings above are designated *required* in the table
(§6.2 Case 2 opener, §6.3 Case 3 opener, §8 Limitations composite
paragraphs). The writer-facing lift is in
`patterns/agent-14-bias-detector--bias-mitigations.md`.

## Confidence

**HIGH.** Every bias finding is quantified from the LOCKED pool counts
agent-11 and agent-13 produced, or from the mission brief's explicit
statement of empirical protocol. No statistical bias test (Egger / trim-
and-fill / fail-safe N) is applicable to this heterogeneous narrative
pool; the appropriate apparatus is qualitative structural audit per
PRISMA / AMSTAR-2 / ROBIS conventions, which is what this file records.

## Changelog

- 2026-04-16 — Initial issue by agent-14-bias-detector.
