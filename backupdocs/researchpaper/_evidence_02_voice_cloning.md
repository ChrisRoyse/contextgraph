# Evidence Brief 02: Zero-Shot Voice Cloning SOTA & Speaker-Similarity Evaluation

*Source: researcher agent af4cdfee6e0c0819b. APA 7th edition formatting applied. See "Verification notes" at bottom — several widely-cited headline numbers (0.881, 0.891, 0.877, 0.862) originate from secondary comparison tables and should be hedged in-paper.*

---

## 1. VALL-E (2023)
- Wang, C., Chen, S., Wu, Y., et al. (2023). *Neural codec language models are zero-shot text to speech synthesizers*. arXiv:2301.02111. https://arxiv.org/abs/2301.02111
- SECS: 0.580 (WavLM-TDNN continuation), 0.508 (cross-sentence).
- First neural-codec language model framing zero-shot TTS as in-context learning over EnCodec tokens.

## 2. VALL-E 2 (2024)
- Chen, S., Liu, S., Zhou, L., et al. (2024). *VALL-E 2: Neural codec language models are human parity zero-shot text to speech synthesizers*. arXiv:2406.05370. https://arxiv.org/abs/2406.05370
- Headline SIM-o ≈ 0.643; WavLM-large SV cosine ≈ 0.881 (secondary table). First to declare "human parity" on LibriSpeech via SIM/WER/DNSMOS parity with ground truth — **NOT a subjective Turing test**.
- Adds Repetition-Aware Sampling + Grouped Code Modeling.

## 3. NaturalSpeech 3 (2024)
- Tan, X., Ju, Z., Wang, Y., et al. (2024). *NaturalSpeech 3: Zero-shot speech synthesis with factorized codec and diffusion models*. ICML 2024. arXiv:2403.03100. https://arxiv.org/abs/2403.03100
- Headline SIM ≈ 0.67; WavLM-large SV cosine ≈ 0.891 (secondary table).
- FACodec factorizes speech into content / prosody / timbre / acoustic-detail.

## 4. MaskGCT (2024)
- Wang, Y., Zhan, H., Liu, L., et al. (2024). *MaskGCT: Zero-shot text-to-speech with masked generative codec transformer*. ICLR 2025. arXiv:2409.00750. https://arxiv.org/abs/2409.00750
- SIM ≈ 0.687; WavLM-large SV cosine ≈ 0.877 (secondary).

## 5. F5-TTS (2024)
- Chen, Y., Niu, Z., Ma, Z., et al. (2024). *F5-TTS: A fairytaler that fakes fluent and faithful speech with flow matching*. arXiv:2410.06885. https://arxiv.org/abs/2410.06885
- SIM-o ≈ 0.66; WavLM-large SV cosine ≈ 0.862 (secondary).
- Non-autoregressive flow-matching TTS with Sway Sampling.

## 6. WavLM (2022)
- Chen, S., Wang, C., Chen, Z., et al. (2022). WavLM: Large-scale self-supervised pre-training for full stack speech processing. *IEEE Journal of Selected Topics in Signal Processing*, 16(6), 1505–1518. arXiv:2110.13900. https://arxiv.org/abs/2110.13900
- microsoft/wavlm-base-plus-sv: WavLM-Base+ fine-tuned for speaker verification on VoxCeleb1 with X-Vector-style head.
- Canonical backbone for SECS (Speaker Encoder Cosine Similarity).

## 7. ClonEval (2025)
- Christop, I., Kuczyński, T., & Kubis, M. (2025). *ClonEval: An open voice cloning benchmark*. arXiv:2504.20581. https://arxiv.org/abs/2504.20581

## 8. "Same-Session Verification Band" ≥ 0.95
- **No canonical citation identified.** VoxCeleb/NIST-SRE literature reports EER distributions but does not formally define a 0.95 threshold as "same-session band." Frame as the authors' empirical observation from our own same-speaker/same-session reference measurements (0.999 same-session vs 0.718 different-session/different-mic), **not** an external benchmark.

## 9. DNSMOS
- Reddy, C. K. A., Gopal, V., & Cutler, R. (2022). DNSMOS P.835: A non-intrusive perceptual objective speech quality metric to evaluate noise suppressors. *ICASSP 2022*. arXiv:2110.01763. https://arxiv.org/abs/2110.01763
- P.808: single-score overall MOS. P.835 triad:
  - **SIG** = speech-signal MOS (distortion of speech).
  - **BAK** = background-noise MOS (higher = less intrusive).
  - **OVRL** = overall MOS combining SIG and BAK.

## 10. UTMOS (2022)
- Saeki, T., Xin, D., Nakata, W., et al. (2022). *UTMOS: UTokyo-SaruLab system for VoiceMOS Challenge 2022*. Interspeech 2022. arXiv:2204.02152. https://arxiv.org/abs/2204.02152
- Winner of VoiceMOS Challenge 2022 main track.

## 11. ECAPA-TDNN (2020)
- Desplanques, B., Thienpondt, J., & Demuynck, K. (2020). *ECAPA-TDNN: Emphasized channel attention, propagation and aggregation in TDNN based speaker verification*. Interspeech 2020. arXiv:2005.07143. https://arxiv.org/abs/2005.07143
- TDNN + Res2Net + SE + attentive statistics pooling, AAM-softmax loss.

## 12. In-Context Learning for TTS (precedents)
- **VoiceBox** — Le, M., Vyas, A., Shi, B., et al. (2023). *Voicebox: Text-guided multilingual universal speech generation at scale*. arXiv:2306.15687. https://arxiv.org/abs/2306.15687
- **NaturalSpeech 2** — Shen, K., Ju, Z., Tan, X., et al. (2023). *NaturalSpeech 2: Latent diffusion models are natural and zero-shot speech and singing synthesizers*. arXiv:2304.09116. https://arxiv.org/abs/2304.09116
- **Mega-TTS 2** — Jiang, Z., Ren, Y., Li, R., et al. (2023). *Mega-TTS 2: Zero-shot text-to-speech with arbitrary-length speech prompts*. arXiv:2307.07218. https://arxiv.org/abs/2307.07218
- **XTTS** — Casanova, E., Davis, K., Gölge, E., et al. (2024). *XTTS: a massively multilingual zero-shot text-to-speech model*. arXiv:2406.04904. https://arxiv.org/abs/2406.04904
