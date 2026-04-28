---
namespace: discoveries
agent_id: agent-27-instrument-developer
timestamp: 2026-04-16T16:20:00Z
status: active
tags: limitations, instruments, section-8, disclosure, paste-ready
depends_on:
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/discoveries/agent-14-bias-detector--bias-audit.md
downstream_consumers:
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
---

# Instrument Limitations — §8 Paste-Ready Disclosures

## 0. Purpose

This is the **paste-ready limitations register** for §8 of the
manuscript. Each entry gives (i) the limitation code, (ii) the exact
sentence agent-40 should paste, and (iii) the upstream instrument
citation. All codes are referenced from
`agent-27-instrument-developer--instrument-specifications.md`.

The register complements (does not replace) agent-14's Bias-05 /
Bias-06 / Bias-08 / Bias-09 disclosures, which cover single-subject,
single-encoder (cross-encoder gap), self-reporting, and benchmark-
monoculture framing. Agent-27's register covers the **instrument-
level** disclosures: what the scorer is trained on, what confounds
its score, and what category of artefact it cannot detect.

---

## 1. WavLM-SECS limitations (A.1)

### L-SECS-A — Encoder-matched self-consistency (not cross-encoder verification)

> *"The Case 3 headline of 0.961 mean WavLM SECS is an encoder-
> matched measurement: WavLM-base-plus-sv is both the guard used
> for best-of-12 selection and the evaluator used to report SECS.
> The headline therefore demonstrates that the guard behaves as
> specified (the generator produces audio that scores high in the
> same embedding space the guard selects on); it does not by itself
> constitute cross-encoder identity verification. Cross-encoder
> replication against ECAPA-TDNN (SpeechBrain / VoxCeleb) is named
> as L-01 future work."*

### L-SECS-B — Codec / vocoder artefact sensitivity

> *"WavLM-SECS is sensitive to codec artefacts and neural-vocoder
> residuals. Case 3 measurement is performed on decoded WAV from
> Qwen3-TTS without intermediate MP3 / Opus re-encoding; the
> reported 0.961 should not be compared with literature numbers
> that score on compressed codec output. On TTS audio with
> residual vocoder ringing (e.g., HiFi-GAN artefacts), SECS can
> be inflated relative to natural microphone recordings of the
> same speaker by 0.01–0.03."*

### L-SECS-C — Same-session band is an author observation, not a benchmark

> *"We observe empirically that within-session speaker pairs for
> the Case-3 subject score in the range [0.95, 0.99] on WavLM-SECS;
> we refer to this as the 'same-session band'. This is the authors'
> observation on the single Case-3 subject; it is not a published
> benchmark and it does not claim universal applicability. Different
> speakers / languages / conditions will exhibit different same-
> session bands, and calibration via the 10th-percentile rule of
> TCT §3.3 is required for any new subject."*

---

## 2. ECAPA-TDNN SECS limitations (A.2)

### L-ECAPA-A — Training-corpus mismatch with WavLM

> *"ECAPA-TDNN SECS and WavLM-base-plus-sv SECS are not directly
> comparable in absolute value: ECAPA-TDNN is trained on VoxCeleb 1+2
> (celebrity YouTube speech with compression, code-switching, and
> varied conditions), while WavLM-base-plus-sv is trained on WavLM's
> pre-training corpus plus VoxCeleb SV fine-tuning. On the same
> clip, ECAPA SECS systematically differs from WavLM SECS by a
> speaker- and condition-dependent offset (typically ±0.10 on
> clean 3 s clips). The audit signal for the cross-encoder check
> is monotone ranking agreement and the sign of the difference,
> not the absolute numerical agreement."*

### L-ECAPA-B — Embedding-dimension trade-off

> *"ECAPA-TDNN's 192-D embedding is finer-grained per dimension
> than WavLM's 512-D but noisier on short or single-clip scoring.
> Reporting the mean over 10 held-out sentences mitigates single-
> clip noise but does not eliminate the difference in per-clip
> variance."*

### L-ECAPA-C — Short-clip degradation

> *"ECAPA-TDNN pooling on clips below 3 s degrades more quickly
> than WavLM; Case 3's ≥ 3 s gate applies symmetrically to both
> scorers but shorter clips in future experiments will not yield
> comparable cross-encoder signal."*

---

## 3. Qwen3-TTS internal speaker encoder (A.3)

### L-Qwen3-A — Vendor-internal, not independently auditable

> *"Qwen3-TTS's internal 2048-D speaker encoder conditions the
> generator but is not independently callable for cross-encoder
> scoring. Its weights are vendor-versioned and may change across
> releases; we record the model revision hash at run time for
> reproducibility, but we do not use this encoder as an identity
> scorer."*

### L-Qwen3-B — Category error to cite as cross-encoder

> *"The fact that Qwen3-TTS's internal speaker representation is
> architecturally distinct from WavLM is sometimes offered as
> evidence of cross-encoder independence in related work. We
> reject this framing for our setting: the generator's internal
> conditioning encoder is part of the generation pipeline, not an
> independent evaluator. Cross-encoder verification requires a
> scorer that has neither trained on the generator's output
> distribution nor participated in selection."*

---

## 4. DNSMOS limitations (B.1)

### L-DNSMOS-A — Trained on noise-suppression, extrapolated to TTS

> *"DNSMOS was trained to predict MOS on the output of noise-
> suppression algorithms, not on TTS-generated audio. We use it
> as a **parity** metric — comparing the Case-3 clone's P.808
> (3.93) to the same speaker's real-microphone recording (3.93
> on the same sentences in the same session) — so that the
> training-corpus mismatch cancels as a within-subject within-
> condition comparison. We do not claim DNSMOS 3.93 is the
> 'naturalness rating' in an absolute sense; we claim only that
> the clone and real-microphone scores are indistinguishable
> under DNSMOS."*

### L-DNSMOS-B — Sample-rate sensitivity

> *"DNSMOS is sensitive to bandwidth and sample rate: 8 kHz,
> 16 kHz, 22.05 kHz, and 44.1 kHz inputs yield systematically
> different scores. Case 3 reports at 16 kHz mono throughout; any
> comparison with literature numbers must match sample rate."*

### L-DNSMOS-C — Codec-artefact sensitivity

> *"DNSMOS can be depressed by codec artefacts (MP3, Opus, neural-
> vocoder residuals). Case 3 reports on WAV throughout. Future
> work that reports DNSMOS on compressed delivery should disclose
> the codec."*

---

## 5. UTMOS limitations (B.2)

### L-UTMOS-A — English-centric training

> *"UTMOS 'strong' is trained on BVCC and VCC data, which are
> English-centric with a mix of natural and synthetic (TTS / VC)
> samples. Case 3 reports on English clips by the English-speaking
> author, so this is aligned; extension to other languages would
> require re-validation."*

### L-UTMOS-B — Variant dependence

> *"Case 3 uses the UTMOS 'strong' ensemble variant (the VoiceMOS-
> Challenge-winning submission). The 'baseline' and 'light' variants
> produce systematically different values; any cross-paper
> comparison must match variant."*

### L-UTMOS-C — Parity interpretation

> *"As with DNSMOS, Case 3's UTMOS is reported as parity (3.012
> clone vs. 2.997 real) rather than as an absolute naturalness
> claim; the two scores are within the expected same-session
> noise floor, which is the operational statement."*

---

## 6. SigLIP visual-centroid cosine limitations (C.1)

### L-SigLIP-A — Demographic-bias inheritance from WebLI

> *"SigLIP-SO400M-patch14-384 is trained on WebLI, a web-scale
> image-text dataset that carries known demographic biases. Visual
> centroid cosine inherits whatever identity-related shortcuts the
> encoder has learned from the web distribution. Case 2's single-
> subject scope limits the implications in this manuscript but
> cross-subject generalisation would require bias auditing."*

### L-SigLIP-B — Centre-crop loss of peripheral context

> *"The canonical 384×384 centre crop drops peripheral subject
> context (hands, shoulders) that can carry identity cues. Case 2
> uses a mouth-weighted flow-matching loss specifically because
> the most identity-relevant per-frame visual content is in the
> face region; the centre-crop loss is therefore less severe than
> it would be for a full-body identity metric."*

### L-SigLIP-C — Does not measure lip-sync

> *"Visual centroid cosine is an identity / appearance metric; it
> does not measure whether the lip motion is correctly aligned
> with the audio. Lip-sync quality is the domain of LSE-D / LSE-C
> (E.1), which is pending in this manuscript."*

---

## 7. FLAME limitations (C.2)

### L-FLAME-A — Not FACS

> *"FLAME's 100 expression coefficients are learned basis
> components over 4D facial scans; they are not a canonical action-
> unit coding system (FACS). When we use FLAME coefficients as a
> 'Level-0 / Level-1 action-unit vocabulary' we mean that the
> coefficients parametrise expression intensity; we do not claim
> they are interpretable as discrete FACS action units without
> additional mapping."*

### L-FLAME-B — Pose and lighting sensitivity

> *"DECA / EMOCA fitting of FLAME coefficients is sensitive to
> camera pose and lighting; out-of-distribution angles or harsh
> lighting degrade coefficient stability. Case 2's clips are
> drawn from a single session with consistent camera and lighting,
> which mitigates this within-case but limits cross-condition
> generalisation."*

### L-FLAME-C — Not the guard

> *"We do NOT use FLAME expression coefficients as a constellation
> guard scorer in this manuscript. FLAME is a parametric head
> representation used for action-unit vocabulary; any future use
> as a guard would require calibration via the TCT §3.3 10th-
> percentile rule."*

---

## 8. STYLE_CENTROID limitations (D.1)

### L-STYLE-A — Cross-encoder remediation pending

> *"The STYLE_CENTROID is a 9-embedder constellation (E1, E4, E6,
> E7, E8, E9, E10, E12, E13) covering dense retrieval, late-
> interaction, and sparse / inverted-index families. This is
> architecturally diverse but the cross-encoder W1 remediation for
> Case 1 specifically targets a held-out ColBERT / BGE-reranker
> trained outside the Context Graph panel — that remediation run
> is pending."*

### L-STYLE-B — No per-character / per-genre decomposition

> *"Shakespeare's stylistic range spans soliloquy, dialogue, letter,
> and sonnet; the reference set is stratified across these (70 /
> 20 / 10 for plays / sonnets / letters) but the centroid is a
> single aggregate. Character-type-specific centroids (e.g., a
> separate sonnet centroid) would give finer control; they are
> named future work."*

### L-STYLE-C — Deliberate exclusion of temporal / causal / entity embedders

> *"E2, E3, E4, E5, and E11 are excluded from STYLE_CENTROID
> because their signal (temporal decay, positional ordering,
> causal direction, entity identity) is not stylistic. Including
> them would degrade stylistic discrimination. E4 (V_ordering) is
> retained only as auxiliary — its inclusion is empirical, based
> on FSV observation that ordering weakly contributes to verse-
> rhythm signal."*

---

## 9. Vocabulary-density prefilter limitations (D.2)

### L-VOCAB-A — Dictionary-based, misses syntax and rhythm

> *"The vocabulary-density prefilter checks archaic-word ratio
> against an author-curated list of ≈200 early-modern-English
> markers. It catches obvious anachronisms (modern colloquialisms,
> contemporary slang) but misses syntax-level (iambic pentameter,
> inversions) and rhetorical-device signals; those are the domain
> of Layer 3 (STYLE_CENTROID)."*

### L-VOCAB-B — Can be fooled by archaic-free Shakespearean

> *"A well-crafted Shakespearean insult or pun built entirely from
> common English words (e.g., 'thou dost speak as a dog to its
> master') would fail the archaic-ratio prefilter despite being
> stylistically faithful. The prefilter is therefore a soft gate
> (regeneration is cheap) rather than a strict reject."*

### L-VOCAB-C — Non-standard dictionary

> *"The archaic-word list is author-defined; it is not a published
> standard. Replication requires the list (documented in
> `scripts/shakespeare_full_pipeline.py`)."*

---

## 10. LSE-D / LSE-C limitations (E.1)

### L-LSE-A — Training distribution mismatch

> *"SyncNet is trained on BBC-style talk-show clips with natural
> speaker-camera setups. Performance on stylised, non-frontal, or
> high-expression clips is less reliable. We note this as a future-
> work consideration when LSE-D is run on Case 2 held-out scenes."*

### L-LSE-B — Frame-rate assumption

> *"SyncNet assumes 25 fps video. Case 2 generation must be
> frame-rate-normalised before LSE-D scoring; mismatched rates
> give spurious confidence scores."*

### L-LSE-C — Not measured in this manuscript

> *"LSE-D / LSE-C are named as pending / future work (§6.2 and
> §8 L-02-Case2-equivalent). Case 2 therefore does not report a
> quantitative lip-sync number; the forensic fix of the mouth-
> bbox error and flow-matching sign-bug is the substantive
> integrity disclosure."*

---

## 11. SHA-256 manifest limitations (F.1)

### L-SHA-A — Byte-exact ≠ semantic identity

> *"Content SHA-256 hashes are byte-exact. Two audio clips that
> sound identical but differ by a single sample bit hash
> differently. This is the intended property for V18 frozen-
> encoder discipline (any silent change in the reference set is
> detectable) but it is not a semantic identity check."*

### L-SHA-B — Not adversarial-robust

> *"SHA-256 is cryptographically collision-resistant but the
> integrity story assumes a non-adversarial pipeline operator.
> This manuscript makes that assumption; adversarial auditing
> (the operator themselves attacking provenance) is out of
> scope."*

---

## 12. Cross-register summary (one-sentence each)

| Code | One-sentence summary |
|---|---|
| L-SECS-A | WavLM is both guard and evaluator — self-consistency, not cross-encoder. |
| L-SECS-B | WavLM SECS is inflated by neural-vocoder artefacts. |
| L-SECS-C | Same-session band [0.95, 0.99] is author observation, not benchmark. |
| L-ECAPA-A | ECAPA trained on VoxCeleb, not LibriSpeech — systematic offset from WavLM. |
| L-ECAPA-B | 192-D vs. 512-D: ECAPA is noisier per clip. |
| L-ECAPA-C | ECAPA degrades faster on < 3 s clips. |
| L-Qwen3-A | Vendor-internal speaker encoder, not auditable. |
| L-Qwen3-B | Qwen3 internal encoder is NOT a cross-encoder check. |
| L-DNSMOS-A | DNSMOS trained on noise-suppression, not TTS — parity interpretation only. |
| L-DNSMOS-B | DNSMOS is sample-rate-sensitive. |
| L-DNSMOS-C | DNSMOS is codec-artefact-sensitive. |
| L-UTMOS-A | UTMOS English-centric. |
| L-UTMOS-B | UTMOS variant dependence (strong / baseline / light). |
| L-UTMOS-C | UTMOS parity interpretation only. |
| L-SigLIP-A | SigLIP carries WebLI demographic biases. |
| L-SigLIP-B | 384 centre crop drops peripheral context. |
| L-SigLIP-C | Visual cosine ≠ lip-sync. |
| L-FLAME-A | FLAME coefficients are not FACS. |
| L-FLAME-B | FLAME fitting is pose- and lighting-sensitive. |
| L-FLAME-C | FLAME is action-unit vocabulary, not a guard. |
| L-STYLE-A | STYLE_CENTROID cross-encoder remediation pending. |
| L-STYLE-B | No per-character / per-genre decomposition. |
| L-STYLE-C | E2 / E3 / E5 / E11 deliberately excluded. |
| L-VOCAB-A | Vocabulary prefilter misses syntax / rhythm. |
| L-VOCAB-B | Archaic-free Shakespearean fools the prefilter. |
| L-VOCAB-C | Dictionary is author-curated, not standard. |
| L-LSE-A | SyncNet trained on BBC-style clips. |
| L-LSE-B | SyncNet assumes 25 fps. |
| L-LSE-C | LSE-D not measured; pending. |
| L-SHA-A | SHA-256 is byte-exact, not semantic. |
| L-SHA-B | SHA integrity assumes non-adversarial operator. |

---

## 13. Top-priority disclosures for §8 (ordered)

The single most important §8 disclosure is **L-SECS-A** — the
encoder-matched self-consistency of the Case 3 headline. Agent-40
must paste L-SECS-A verbatim under the Case 3 limitations
paragraph, **before** the Bias-05 / Bias-06 single-subject /
single-encoder paragraph.

Next in priority:
1. **L-SECS-A** — encoder-matching (WavLM is guard AND evaluator).
2. **L-ECAPA-A** — if W1 remediation runs, disclose systematic offset.
3. **L-DNSMOS-A / L-UTMOS-C** — parity-not-absolute framing.
4. **L-STYLE-A** — Case 1 cross-encoder pending.
5. **L-LSE-C** — Case 2 lip-sync deferred.
6. **L-Qwen3-B** — reject Qwen3-internal-as-cross-encoder framing.
7. **L-SHA-A** — byte-exact hash is the integrity story.

## Confidence

**HIGH** that each limitation sentence is defensible against
adversarial review: every disclosure points at a specific training-
distribution / operating-condition / pipeline-role fact that is
verifiable from the cited source.

**HIGH** that the L-SECS-A disclosure is the single most important
instrument-level disclosure — the Case 3 headline's interpretability
hinges on it being stated plainly.

**MEDIUM** on the completeness of the Case-2 open-carries (emotion
and voice-identity encoders). Agent-37 should resolve these to
specific model IDs before §6.2 final writing; the placeholder
limitations entries for those encoders are not in this register yet.

## Changelog

- 2026-04-16 — Initial issue by agent-27-instrument-developer.
