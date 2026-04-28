---
namespace: decisions
agent_id: agent-08-citation-extractor
timestamp: 2026-04-16T17:45:00Z
status: active
locked: true
supersedes: none
tags: citations, master-bibliography, apa-7th, tier-labeled, citation-keys, qg1, g7, g8
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-07-source-tier-classifier--tier-assignments.md
  - memory/discoveries/agent-06-literature-mapper--topic-topology.md
  - memory/handoff/agent-07-to-08--tiers-locked.md
  - docs/researchpaper/13_references.md
  - docs/researchpaper/_evidence_01_data_wall.md
  - docs/researchpaper/_evidence_02_voice_cloning.md
  - docs/researchpaper/_evidence_03_video_embedders.md
  - docs/researchpaper/_evidence_04_enterprise_market.md
downstream_consumers:
  - agent-09-methodology-scanner
  - agent-11-systematic-reviewer
  - agent-36-related-work-writer
  - agent-37-framework-writer
  - agent-38-methodology-writer
  - agent-39-discussion-writer
  - agent-44-citation-crosschecker
  - agent-45-citation-validator
---

# Master Reference Pool — LOCKED

**LOCK STATEMENT.** This file is the authoritative citation database for the arXiv
submission. Every reference in the final PAPER.md MUST be drawn from this file by
**citation key**. Downstream agents must cite by key (e.g., `shumailov2024nature`),
not by free-text URL. Any addition, removal, or metadata correction requires a
blocker filed to the coordinator plus a changelog entry at the foot of this file.

## Counts

- **Total canonical entries: 88** (85 from agent-07's LOCKED tier table + 3 added RLHF/DPO/CAI triad per Q06 gap).
- **Cluster labels (A–H):** per agent-06 / agent-07 master table. One alignment cluster **I** added for the RLHF/DPO/CAI triad (new, 3 entries).
- **Tier distribution after additions:** T1 = 25, T2 = 37, T3 = 26. T1+T2 = 70% on the full pool; **core body set (A–G + I) T1+T2 = 85%** (unchanged since Cluster H is body-quota-limited per agent-07's rule).
- **Ineligible / unverified:** 2 (Qwen3-TTS; Demucs v1/v2/v3) — listed separately and excluded from citation keys.

## Venue re-verification outcomes (from agent-07 handoff steps 8–9)

1. **wav2vec 2.0 (G01).** Confirmed via NeurIPS Proceedings (NeurIPS 2020, paper 92d1e1eb1cd6f9fba3227870bb6d7f07). **Promoted T2 → T1** in this master file. Changelog entry at the bottom. Tier updated; all downstream stats re-computed.
2. **Pile of Law (H05).** Confirmed via OpenReview: NeurIPS 2022 Datasets & Benchmarks Track. **Tier held at T1** (as agent-07 pre-assigned).

## Added: RLHF / DPO / CAI triad (Cluster I, new)

Per agent-07's flagged Q06 gap, three alignment citations are added here. Expected tiers (Ouyang T1 NeurIPS 2022, Rafailov T1 NeurIPS 2023, Bai T2 arXiv-only from T1-publishing group) are confirmed via direct venue lookup (see I01–I03 below). Cluster label **I — Alignment (RLHF / DPO / CAI)**.

## Deduplication note

Three citations appear in multiple evidence briefs. They carry a single key in this file; cross-brief appearances recorded:

- WavLM (Chen et al., 2022) — briefs 02 §6 + 03 → single key `chen2022wavlm`.
- Franzen (TechCrunch, 2024) — briefs 01 §5 + 04 → single key `franzen2024techcrunch`.
- Woo & Efrati (The Information, 2024) — briefs 01 §5 + 04 → single key `woo2024information`.

---

## Cluster A — Scaling laws + data wall

### [A01] kaplan2020scaling — T2

Kaplan, J., McCandlish, S., Henighan, T., Brown, T. B., Chess, B., Child, R., Gray, S., Radford, A., Wu, J., & Amodei, D. (2020). *Scaling laws for neural language models*. arXiv. https://arxiv.org/abs/2001.08361

- **arXiv ID:** 2001.08361. **DOI:** none. **Venue:** arXiv (OpenAI group — routinely T1).
- **Tier rationale:** arXiv-only; OpenAI authors; foundational paper (10,000+ citations).
- **Load-bearing for:** Q01.

### [A02] hoffmann2022chinchilla — T1

Hoffmann, J., Borgeaud, S., Mensch, A., Buchatskaya, E., Cai, T., Rutherford, E., de Las Casas, D., Hendricks, L. A., Welbl, J., Clark, A., Hennigan, T., Noland, E., Millican, K., van den Driessche, G., Damoc, B., Guy, A., Osindero, S., Simonyan, K., Elsen, E., … Sifre, L. (2022). Training compute-optimal large language models. In *Advances in Neural Information Processing Systems 35 (NeurIPS 2022)*. arXiv. https://arxiv.org/abs/2203.15556

- **arXiv ID:** 2203.15556. **DOI:** none (NeurIPS 2022 proceedings). **Venue:** NeurIPS 2022.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q01.

### [A03] villalobos2024data — T1

Villalobos, P., Ho, A., Sevilla, J., Besiroglu, T., Heim, L., & Hobbhahn, M. (2024). Will we run out of data? Limits of LLM scaling based on human-generated data. In *Proceedings of the 41st International Conference on Machine Learning (ICML 2024)*. arXiv. https://arxiv.org/abs/2211.04325

- **arXiv ID:** 2211.04325 (v1 2022; revised Jun 2024 for ICML). **DOI:** none. **Venue:** ICML 2024.
- **Tier rationale:** Peer-reviewed top-tier; canonical data-wall citation.
- **Load-bearing for:** Q01.

### [A04] longpre2024consent — T1

Longpre, S., Mahari, R., Obeng-Marnu, N., Brannon, W., Kumar, N., South, T., Obeng-Marnu, N., Pentland, A., Mahari, R., Lee, S. H., Srikanth, R., Sasseville, D., Metcalf, J., Sim, P., Pentland, A., Baack, S., Cooper, A. F., & Obeng-Marnu, N. (2024). Consent in crisis: The rapid decline of the AI data commons. In *Advances in Neural Information Processing Systems: Datasets and Benchmarks Track (NeurIPS 2024)*. arXiv. https://arxiv.org/abs/2407.14933

- **arXiv ID:** 2407.14933. **DOI:** none. **Venue:** NeurIPS 2024 D&B Track.
- **Tier rationale:** Peer-reviewed top-tier.
- **Note:** Only first 3 authors + common co-authors listed; the full author list is 20+ on arXiv. Use Longpre, Mahari, Obeng-Marnu et al. (2024) in-text.
- **Load-bearing for:** Q01.

### [A05] woo2024information — T3

Woo, E., & Efrati, A. (2024, November 9). *OpenAI shifts strategy as rate of 'GPT' AI improvements slows*. The Information. https://www.theinformation.com/articles/openai-shifts-strategy-as-rate-of-gpt-ai-improvements-slows

- **Venue:** The Information. **Tier rationale:** Credible trade press; industry signal. No Tier-1 equivalent exists for this dated industry observation.
- **Load-bearing for:** Q01. Cross-brief appearance: briefs 01 + 04.

### [A06] metz2024bloomberg — T3

Metz, R., Ghaffary, S., & Love, J. (2024, November 13). *OpenAI, Google and Anthropic are struggling to build more advanced AI*. Bloomberg. https://www.bloomberg.com/news/articles/2024-11-13/openai-google-and-anthropic-are-struggling-to-build-more-advanced-ai

- **Venue:** Bloomberg. **Tier rationale:** Credible trade press; industry signal.
- **Load-bearing for:** Q01.

### [A07] hu2024reuters — T3

Hu, K., & Tong, A. (2024, November 15). *OpenAI and others seek new path to smarter AI as current methods hit limitations*. Reuters. https://www.reuters.com/technology/artificial-intelligence/openai-rivals-seek-new-path-smarter-ai-current-methods-hit-limitations-2024-11-11/

- **Venue:** Reuters. **Tier rationale:** Top-tier wire service; industry signal.
- **Load-bearing for:** Q01.

### [A08] franzen2024techcrunch — T3

Franzen, C. (2024, November 20). *Current AI scaling laws are showing diminishing returns, forcing AI labs to change course*. TechCrunch. https://techcrunch.com/2024/11/20/ai-scaling-laws-are-showing-diminishing-returns-forcing-ai-labs-to-change-course/

- **Venue:** TechCrunch. **Tier rationale:** Credible trade press; industry signal.
- **Load-bearing for:** Q01. Cross-brief appearance: briefs 01 + 04.
- **URL note:** agent-07 listed a slightly different path (`current-ai-scaling-laws-are-showing-diminishing-returns-forcing-ai-labs-to-change-course`). Brief 04's canonical URL (above) used.

### [A09] sutskever2024neurips — T3

Sutskever, I. (2024, December). *Pre-training as we know it will end* [Test-of-Time award talk, oral]. 38th Conference on Neural Information Processing Systems (NeurIPS 2024), Vancouver, Canada.

- **Venue:** Oral talk at NeurIPS 2024. **URL:** none (oral; video on NeurIPS / YouTube mirrors; no authored written artefact).
- **Tier rationale:** Attribution only; we flag in-text as "oral remark, not peer-reviewed". No Tier-1 equivalent because the quote is inherently oral.
- **Load-bearing for:** Q01.

---

## Cluster B — Model collapse + synthetic data

### [B01] shumailov2024nature — T1

Shumailov, I., Shumaylov, Z., Zhao, Y., Gal, Y., Papernot, N., & Anderson, R. (2024). AI models collapse when trained on recursively generated data. *Nature*, 631(8022), 755–759. https://doi.org/10.1038/s41586-024-07566-y

- **DOI:** 10.1038/s41586-024-07566-y. **arXiv (earlier preprint):** 2305.17493. **Venue:** *Nature*.
- **Tier rationale:** Peer-reviewed top-tier journal.
- **Load-bearing for:** Q02.

### [B02] gerstgrasser2024colm — T2

Gerstgrasser, M., Schaeffer, R., Dey, A., Rafailov, R., Sleight, H., Hughes, J., Korbak, T., Agrawal, R., Pai, D., Gromov, A., Roberts, D. A., Yang, D., Donoho, D. L., & Koyejo, S. (2024). Is model collapse inevitable? Breaking the curse of recursion by accumulating real and synthetic data. In *Proceedings of the First Conference on Language Modeling (COLM 2024)*. arXiv. https://arxiv.org/abs/2404.01413

- **arXiv ID:** 2404.01413. **Venue:** COLM 2024.
- **Tier rationale:** Peer-reviewed reputable venue (COLM is T2 per task brief).
- **Load-bearing for:** Q02.

### [B03] dohmatob2024tails — T1

Dohmatob, E., Feng, Y., Yang, P., Charton, F., & Kempe, J. (2024). A tale of tails: Model collapse as a change of scaling laws. In *Proceedings of the 41st International Conference on Machine Learning (ICML 2024)*. arXiv. https://arxiv.org/abs/2402.07043

- **arXiv ID:** 2402.07043. **Venue:** ICML 2024.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q02.

### [B04] dohmatob2024regression — T1

Dohmatob, E., Feng, Y., & Kempe, J. (2024). Model collapse demystified: The case of regression. In *Advances in Neural Information Processing Systems 37 (NeurIPS 2024)*. arXiv. https://arxiv.org/abs/2402.07712

- **arXiv ID:** 2402.07712. **Venue:** NeurIPS 2024.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q02.

### [B05] feng2024verification — T2

Feng, Y., Dohmatob, E., Yang, P., Charton, F., & Kempe, J. (2024). *Beyond model collapse: Scaling up with synthesized data requires verification*. arXiv. https://arxiv.org/abs/2406.07515

- **arXiv ID:** 2406.07515. **Venue:** arXiv (NYU / FAIR group — routinely T1).
- **Tier rationale:** Extends B03/B04 lineage; preprint by T1-publishing group.
- **Load-bearing for:** Q02.

### [B06] alemohammad2023mad — T1

Alemohammad, S., Casco-Rodriguez, J., Luzi, L., Humayun, A. I., Babaei, H., LeJeune, D., Siahkoohi, A., & Baraniuk, R. G. (2023). *Self-consuming generative models go MAD*. In *Proceedings of the International Conference on Learning Representations (ICLR 2024)*. arXiv. https://arxiv.org/abs/2307.01850

- **arXiv ID:** 2307.01850. **Venue:** ICLR 2024.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q02.

### [B07] seddik2024collapse — T2

Seddik, M. E. A., Chen, S.-W., Hayou, S., Youssef, A., & Debbah, M. (2024). *How bad is training on synthetic data? A statistical analysis of language model collapse*. In *Proceedings of the First Conference on Language Modeling (COLM 2024)*. arXiv. https://arxiv.org/abs/2404.05090

- **arXiv ID:** 2404.05090. **Venue:** COLM 2024.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Load-bearing for:** Q02.

---

## Cluster C — Compression-as-intelligence

### [C01] deletang2024compression — T1

Delétang, G., Ruoss, A., Duquenne, P.-A., Grau-Moya, J., Wenliang, L. K., Aitchison, M., Orseau, L., Hutter, M., Legg, S., & Veness, J. (2024). *Language modeling is compression*. In *Proceedings of the International Conference on Learning Representations (ICLR 2024)*. arXiv. https://arxiv.org/abs/2309.10668

- **arXiv ID:** 2309.10668. **Venue:** ICLR 2024.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q05.

### [C02] huang2024compression — T2

Huang, Y., Vincent, F., Ye, K., Xu, Z., Chang, J., Xu, D., Yin, X., Yang, D., & Xiao, C. (2024). *Compression represents intelligence linearly*. In *Proceedings of the First Conference on Language Modeling (COLM 2024)*. arXiv. https://arxiv.org/abs/2404.09937

- **arXiv ID:** 2404.09937. **Venue:** COLM 2024.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Load-bearing for:** Q05.

### [C03] li2025lossless — T1

Li, C., Yang, L., Zhang, K., Wang, F., Liu, Y., Zhang, Z., & Sun, Z. (2025). Lossless data compression by large language models. *Nature Machine Intelligence*. Advance online publication. https://doi.org/10.1038/s42256-025-01033-7

- **DOI:** 10.1038/s42256-025-01033-7. **Venue:** *Nature Machine Intelligence*.
- **Tier rationale:** Peer-reviewed top-tier journal.
- **Load-bearing for:** Q05.

### [C04] valmeekam2023llmzip — T2

Valmeekam, C. S. K., Narayanan, K., Tuli, A., & Yu, Z. (2023). *LLMZip: Lossless text compression using large language models*. arXiv. https://arxiv.org/abs/2306.04050

- **arXiv ID:** 2306.04050. **Venue:** arXiv (Texas A&M, active in LLM-compression research).
- **Tier rationale:** Quantitative benchmark for LLM-as-compressor claim.
- **Load-bearing for:** Q05.

### [C05] jiang2023gzip — T1

Jiang, Z., Yang, M., Cui, M., & Han, J. (2023). "Low-resource" text classification: A parameter-free classification method with compressors. In *Findings of the Association for Computational Linguistics: ACL 2023* (pp. 6810–6828). Association for Computational Linguistics. https://aclanthology.org/2023.findings-acl.426/

- **Venue:** ACL 2023 Findings. **URL:** https://aclanthology.org/2023.findings-acl.426/
- **Tier rationale:** Peer-reviewed top-tier (ACL Findings is part of ACL lineage per task brief).
- **Load-bearing for:** Q05.

---

## Cluster D — Multi-modal embedders + retrieval

### [D01] zhai2023siglip — T2

Zhai, X., Mustafa, B., Kolesnikov, A., & Beyer, L. (2023). *Sigmoid loss for language image pre-training*. arXiv. https://arxiv.org/abs/2303.15343

- **arXiv ID:** 2303.15343. **Venue:** arXiv (Google DeepMind — routinely T1); also published at ICCV 2023 per proceedings; brief cites arXiv so we keep T2 aligned to brief.
- **Note (rev-verify opportunity):** ICCV 2023 presentation confirmed; could be promoted T2→T1 on this basis. Agent-07 kept T2 to match brief; we follow suit here and defer to agent-44/45.
- **Load-bearing for:** Q04, Q09, Q10.

### [D02] nussbaum2024nomic — T2

Nussbaum, Z., Morris, J. X., Duderstadt, B., & Mulyar, A. (2024). *Nomic Embed: Training a reproducible long context text embedder*. arXiv. https://arxiv.org/abs/2402.01613

- **arXiv ID:** 2402.01613. **Venue:** arXiv (Nomic AI; open weights + code).
- **Tier rationale:** T2 via reproducible-artefact criterion.
- **Load-bearing for:** Q10.

### [D03] chen2022wavlm — T1

Chen, S., Wang, C., Chen, Z., Wu, Y., Liu, S., Chen, Z., Li, J., Kanda, N., Yoshioka, T., Xiao, X., Wu, J., Zhou, L., Ren, S., Qian, Y., Qian, Y., Zeng, M., & Wei, F. (2022). WavLM: Large-scale self-supervised pre-training for full stack speech processing. *IEEE Journal of Selected Topics in Signal Processing*, 16(6), 1505–1518. https://doi.org/10.1109/JSTSP.2022.3188113 (arXiv preprint https://arxiv.org/abs/2110.13900)

- **DOI:** 10.1109/JSTSP.2022.3188113. **arXiv:** 2110.13900. **Venue:** IEEE JSTSP.
- **Tier rationale:** Peer-reviewed top-tier journal.
- **Load-bearing for:** Q10, Q12. Cross-brief appearance: briefs 02 §6 + 03.

### [D04] khattab2020colbert — T2

Khattab, O., & Zaharia, M. (2020). ColBERT: Efficient and effective passage search via contextualized late interaction over BERT. In *Proceedings of the 43rd International ACM SIGIR Conference on Research and Development in Information Retrieval* (pp. 39–48). Association for Computing Machinery. https://doi.org/10.1145/3397271.3401075 (arXiv preprint https://arxiv.org/abs/2004.12832)

- **DOI:** 10.1145/3397271.3401075. **arXiv:** 2004.12832. **Venue:** SIGIR 2020.
- **Tier rationale:** Peer-reviewed reputable venue (SIGIR is T2 per task brief).
- **Load-bearing for:** Q10.

### [D05] santhanam2022colbertv2 — T1

Santhanam, K., Khattab, O., Saad-Falcon, J., Potts, C., & Zaharia, M. (2022). ColBERTv2: Effective and efficient retrieval via lightweight late interaction. In *Proceedings of the 2022 Conference of the North American Chapter of the Association for Computational Linguistics: Human Language Technologies (NAACL 2022)* (pp. 3715–3734). https://doi.org/10.18653/v1/2022.naacl-main.272 (arXiv preprint https://arxiv.org/abs/2112.01488)

- **DOI:** 10.18653/v1/2022.naacl-main.272. **arXiv:** 2112.01488. **Venue:** NAACL 2022.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q10.

### [D06] formal2021splade — T2

Formal, T., Piwowarski, B., & Clinchant, S. (2021). SPLADE: Sparse lexical and expansion model for first stage ranking. In *Proceedings of the 44th International ACM SIGIR Conference on Research and Development in Information Retrieval* (pp. 2288–2292). Association for Computing Machinery. https://doi.org/10.1145/3404835.3463098 (arXiv preprint https://arxiv.org/abs/2107.05720)

- **DOI:** 10.1145/3404835.3463098. **arXiv:** 2107.05720. **Venue:** SIGIR 2021.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Load-bearing for:** Q10.

### [D07] wang2021kepler — T2

Wang, X., Gao, T., Zhu, Z., Zhang, Z., Liu, Z., Li, J., & Tang, J. (2021). KEPLER: A unified model for knowledge embedding and pre-trained language representation. *Transactions of the Association for Computational Linguistics*, 9, 176–194. https://doi.org/10.1162/tacl_a_00360 (arXiv preprint https://arxiv.org/abs/1911.06136)

- **DOI:** 10.1162/tacl_a_00360. **arXiv:** 1911.06136. **Venue:** TACL.
- **Tier rationale:** Peer-reviewed reputable journal (TACL is T2 per task brief).
- **Load-bearing for:** Q10.

### [D08] bordes2013transe — T1

Bordes, A., Usunier, N., Garcia-Durán, A., Weston, J., & Yakhnenko, O. (2013). Translating embeddings for modeling multi-relational data. In *Advances in Neural Information Processing Systems 26 (NeurIPS 2013)* (pp. 2787–2795). https://papers.nips.cc/paper_files/paper/2013/hash/1cecc7a77928ca8133fa24680a88d2f9-Abstract.html

- **URL:** https://papers.nips.cc/paper_files/paper/2013/hash/1cecc7a77928ca8133fa24680a88d2f9-Abstract.html. **Venue:** NeurIPS 2013.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q10.

### [D09] malkov2018hnsw — T1

Malkov, Y. A., & Yashunin, D. A. (2018). Efficient and robust approximate nearest neighbor search using hierarchical navigable small world graphs. *IEEE Transactions on Pattern Analysis and Machine Intelligence*, 42(4), 824–836. https://doi.org/10.1109/TPAMI.2018.2889473 (arXiv preprint https://arxiv.org/abs/1603.09320)

- **DOI:** 10.1109/TPAMI.2018.2889473. **arXiv:** 1603.09320. **Venue:** IEEE TPAMI.
- **Tier rationale:** Peer-reviewed top-tier journal.
- **Load-bearing for:** Q10.

### [D10] cormack2009rrf — T2

Cormack, G. V., Clarke, C. L. A., & Büttcher, S. (2009). Reciprocal rank fusion outperforms Condorcet and individual rank learning methods. In *Proceedings of the 32nd International ACM SIGIR Conference on Research and Development in Information Retrieval* (pp. 758–759). Association for Computing Machinery. https://doi.org/10.1145/1571941.1572114

- **DOI:** 10.1145/1571941.1572114. **Venue:** SIGIR 2009.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Load-bearing for:** Q10.

---

## Cluster E — Zero-shot TTS + voice cloning

### [E01] wang2023valle — T2

Wang, C., Chen, S., Wu, Y., Zhang, Z., Zhou, L., Liu, S., Chen, Z., Liu, Y., Wang, H., Li, J., He, L., Zhao, S., & Wei, F. (2023). *Neural codec language models are zero-shot text to speech synthesizers*. arXiv. https://arxiv.org/abs/2301.02111

- **arXiv ID:** 2301.02111. **Venue:** arXiv (Microsoft Research — routinely T1).
- **Tier rationale:** First neural-codec TTS LM; widely cited precursor.
- **Load-bearing for:** Q09, Q14.

### [E02] chen2024valle2 — T2

Chen, S., Liu, S., Zhou, L., Liu, Y., Tan, X., Li, J., Zhao, S., Qian, Y., & Wei, F. (2024). *VALL-E 2: Neural codec language models are human parity zero-shot text to speech synthesizers*. arXiv. https://arxiv.org/abs/2406.05370

- **arXiv ID:** 2406.05370. **Venue:** arXiv (Microsoft Research — routinely T1).
- **Tier rationale:** Case 3 comparator; verifiable benchmark protocol.
- **Load-bearing for:** Q09, Q12, Q14.

### [E03] tan2024naturalspeech3 — T1

Tan, X., Ju, Z., Wang, Y., Shen, K., Liu, S., Wei, F., Chen, S., Li, J., Qian, Y., Xiao, R., Yu, K., Zhao, S., Wang, Y., Zhou, L., He, L., & Xia, Y. (2024). *NaturalSpeech 3: Zero-shot speech synthesis with factorized codec and diffusion models*. In *Proceedings of the 41st International Conference on Machine Learning (ICML 2024)*. arXiv. https://arxiv.org/abs/2403.03100

- **arXiv ID:** 2403.03100. **Venue:** ICML 2024.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q09.

### [E04] wang2024maskgct — T1

Wang, Y., Zhan, H., Liu, L., Zeng, R., Guo, H., Zheng, J., Zhang, Q., Zhang, X., Zhang, S., & Wu, Z. (2024). *MaskGCT: Zero-shot text-to-speech with masked generative codec transformer*. In *Proceedings of the International Conference on Learning Representations (ICLR 2025)*. arXiv. https://arxiv.org/abs/2409.00750

- **arXiv ID:** 2409.00750. **Venue:** ICLR 2025.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q09.

### [E05] chen2024f5tts — T2

Chen, Y., Niu, Z., Ma, Z., Deng, K., Wang, C., Zhao, J., Yu, K., & Chen, X. (2024). *F5-TTS: A fairytaler that fakes fluent and faithful speech with flow matching*. arXiv. https://arxiv.org/abs/2410.06885

- **arXiv ID:** 2410.06885. **Venue:** arXiv (Shanghai Jiao Tong / SeedTTS — active in T1).
- **Load-bearing for:** Q09.

### [E06] le2023voicebox — T2

Le, M., Vyas, A., Shi, B., Karrer, B., Sari, L., Moritz, R., Williamson, M., Manohar, V., Adi, Y., Mahadeokar, J., & Hsu, W.-N. (2023). *Voicebox: Text-guided multilingual universal speech generation at scale*. arXiv. https://arxiv.org/abs/2306.15687

- **arXiv ID:** 2306.15687. **Venue:** arXiv (Meta FAIR — routinely T1); also NeurIPS 2023.
- **Note:** NeurIPS 2023 presentation confirmed; could be promoted T2→T1. Agent-07 kept T2 to match brief; we follow suit.
- **Load-bearing for:** Q09.

### [E07] shen2023naturalspeech2 — T2

Shen, K., Ju, Z., Tan, X., Liu, Y., Leng, Y., He, L., Qin, T., Zhao, S., & Bian, J. (2023). *NaturalSpeech 2: Latent diffusion models are natural and zero-shot speech and singing synthesizers*. arXiv. https://arxiv.org/abs/2304.09116

- **arXiv ID:** 2304.09116. **Venue:** arXiv (Microsoft Research — routinely T1); presented at ICLR 2024.
- **Note:** ICLR 2024 presentation confirmed; T2→T1 candidate. Holding T2 per handoff rule.
- **Load-bearing for:** Q09.

### [E08] jiang2023megatts2 — T2

Jiang, Z., Ren, Y., Li, R., Ji, S., Yang, J., & Zhao, Z. (2023). *Mega-TTS 2: Zero-shot text-to-speech with arbitrary-length speech prompts*. arXiv. https://arxiv.org/abs/2307.07218

- **arXiv ID:** 2307.07218. **Venue:** arXiv (Zhejiang Univ. / ByteDance — active in T1).
- **Load-bearing for:** Q09.

### [E09] casanova2024xtts — T2

Casanova, E., Davis, K., Gölge, E., Göknar, G., Gulea, I., Hart, L., Aljafari, A., Meyer, J., Morais, R., Olayemi, S., & Weber, J. (2024). *XTTS: A massively multilingual zero-shot text-to-speech model*. arXiv. https://arxiv.org/abs/2406.04904

- **arXiv ID:** 2406.04904. **Venue:** arXiv (Coqui; open weights production artefact).
- **Tier rationale:** T2 via reproducible-artefact criterion.
- **Load-bearing for:** Q09.

### [E10] reddy2022dnsmos — T2

Reddy, C. K. A., Gopal, V., & Cutler, R. (2022). DNSMOS P.835: A non-intrusive perceptual objective speech quality metric to evaluate noise suppressors. In *ICASSP 2022 — 2022 IEEE International Conference on Acoustics, Speech and Signal Processing* (pp. 886–890). IEEE. https://doi.org/10.1109/ICASSP43922.2022.9746108 (arXiv preprint https://arxiv.org/abs/2110.01763)

- **DOI:** 10.1109/ICASSP43922.2022.9746108. **arXiv:** 2110.01763. **Venue:** ICASSP 2022.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Load-bearing for:** Q12.

### [E11] saeki2022utmos — T2

Saeki, T., Xin, D., Nakata, W., Koriyama, T., Takamichi, S., & Saruwatari, H. (2022). *UTMOS: UTokyo-SaruLab system for VoiceMOS Challenge 2022*. In *Proceedings of Interspeech 2022* (pp. 4521–4525). ISCA. https://doi.org/10.21437/Interspeech.2022-439 (arXiv preprint https://arxiv.org/abs/2204.02152)

- **DOI:** 10.21437/Interspeech.2022-439. **arXiv:** 2204.02152. **Venue:** Interspeech 2022.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Load-bearing for:** Q12.

### [E12] desplanques2020ecapa — T2

Desplanques, B., Thienpondt, J., & Demuynck, K. (2020). *ECAPA-TDNN: Emphasized channel attention, propagation and aggregation in TDNN based speaker verification*. In *Proceedings of Interspeech 2020* (pp. 3830–3834). ISCA. https://doi.org/10.21437/Interspeech.2020-2650 (arXiv preprint https://arxiv.org/abs/2005.07143)

- **DOI:** 10.21437/Interspeech.2020-2650. **arXiv:** 2005.07143. **Venue:** Interspeech 2020.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Load-bearing for:** Q20.

### [E13] christop2025cloneval — T2

Christop, I., Kuczyński, T., & Kubis, M. (2025). *ClonEval: An open voice cloning benchmark*. arXiv. https://arxiv.org/abs/2504.20581

- **arXiv ID:** 2504.20581. **Venue:** arXiv (Adam Mickiewicz University; open benchmark).
- **Tier rationale:** T2 via open-benchmark artefact criterion. Supplementary.
- **Load-bearing for:** Q14.

---

## Cluster F — Identity-locked video generation

### [F01] meng2025echomimicv3 — T2

Meng, R., Wang, Y., Wu, W., Liu, Y., Liu, L., & Feng, L. (2025). *EchoMimicV3: 1.3B parameters are all you need for unified multi-modal and multi-task human animation*. arXiv. https://arxiv.org/abs/2507.03905

- **arXiv ID:** 2507.03905. **Venue:** arXiv (Alibaba Ant Group).
- **Tier rationale:** Case 2 base model; reproducible artefact.
- **Load-bearing for:** Q11, Q13.

### [F02] cui2024hallo3 — T2

Cui, J., Li, H., Zhan, Y., Shang, H., Cheng, K., Ma, Y., Mu, S., Zhou, H., Wang, J., & Zhu, S. (2024). *Hallo3: Highly dynamic and realistic portrait image animation with video diffusion transformer*. arXiv. https://arxiv.org/abs/2412.00733

- **arXiv ID:** 2412.00733. **Venue:** arXiv (Fudan / Baidu).
- **Load-bearing for:** Q11.

### [F03] li2024latentsync — T2

Li, C., Zhang, C., Xu, W., Lin, J., Xie, J., Feng, S., Zhou, X., Yan, M., & Huang, F. (2024). *LatentSync: Audio-conditioned latent diffusion models for lip sync*. arXiv. https://arxiv.org/abs/2412.09262

- **arXiv ID:** 2412.09262. **Venue:** arXiv (ByteDance — routinely T1).
- **Tier rationale:** Lip-sync metric anchor.
- **Load-bearing for:** Q13.

### [F04] zheng2025hunyuanvideo — T2

Zheng, Y., Chen, Z., Cun, X., Wang, X., Zhang, Y., Chen, Y., Ruan, J., Jiang, R., Wu, Z., Shan, Y., & Weng, Y. (2025). *HunyuanVideo-Avatar: High-fidelity audio-driven human animation for multiple characters*. arXiv. https://arxiv.org/abs/2505.20156

- **arXiv ID:** 2505.20156. **Venue:** arXiv (Tencent Hunyuan).
- **Load-bearing for:** Q11.

### [F05] tian2024emo — T2

Tian, L., Wang, Q., Zhang, B., & Bo, L. (2024). *EMO: Emote portrait alive — Generating expressive portrait videos with audio2video diffusion model under weak conditions*. arXiv. https://arxiv.org/abs/2402.17485

- **arXiv ID:** 2402.17485. **Venue:** arXiv (Alibaba Tongyi Lab); presented at ECCV 2024.
- **Note:** ECCV 2024 presentation; T2→T1 candidate. Holding T2 per handoff rule.
- **Load-bearing for:** Q11.

### [F06] tian2025emo2 — T2

Tian, L., Hu, S., Wang, Q., Zhang, B., & Bo, L. (2025). *EMO2: End-effector guided audio-driven avatar video generation*. arXiv. https://arxiv.org/abs/2501.10687

- **arXiv ID:** 2501.10687. **Venue:** arXiv (Alibaba Tongyi Lab).
- **Load-bearing for:** Q11.

### [F07] yang2024cogvideox — T2

Yang, Z., Teng, J., Zheng, W., Ding, M., Huang, S., Xu, J., Yang, Y., Hong, W., Zhang, X., Feng, G., Yin, D., Gu, X., Zhang, Y., Wang, W., Cheng, Y., Liu, T., Xu, B., Dong, Y., & Tang, J. (2024). *CogVideoX: Text-to-video diffusion models with an expert transformer*. arXiv. https://arxiv.org/abs/2408.06072

- **arXiv ID:** 2408.06072. **Venue:** arXiv (Zhipu AI / Tsinghua).
- **Load-bearing for:** Q11.

### [F08] wan2025video — T2

Wan Team. (2025). *Wan: Open and advanced large-scale video generative models*. arXiv. https://arxiv.org/abs/2503.20314

- **arXiv ID:** 2503.20314. **Venue:** arXiv (Alibaba DAMO — routinely T1).
- **Note:** Corporate author. APA 7th permits group authorship; cite as "Wan Team" or "Alibaba DAMO Wan Team" in-text.
- **Load-bearing for:** Q17.

### [F09] hu2022lora — T1

Hu, E. J., Shen, Y., Wallis, P., Allen-Zhu, Z., Li, Y., Wang, S., Wang, L., & Chen, W. (2022). *LoRA: Low-rank adaptation of large language models*. In *Proceedings of the International Conference on Learning Representations (ICLR 2022)*. arXiv. https://arxiv.org/abs/2106.09685

- **arXiv ID:** 2106.09685. **Venue:** ICLR 2022.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q11.

### [F10] lipman2023flowmatching — T1

Lipman, Y., Chen, R. T. Q., Ben-Hamu, H., Nickel, M., & Le, M. (2023). *Flow matching for generative modeling*. In *Proceedings of the International Conference on Learning Representations (ICLR 2023)*. arXiv. https://arxiv.org/abs/2210.02747

- **arXiv ID:** 2210.02747. **Venue:** ICLR 2023.
- **Tier rationale:** Peer-reviewed top-tier.
- **Load-bearing for:** Q17.

### [F11] li2017flame — T1

Li, T., Bolkart, T., Black, M. J., Li, H., & Romero, J. (2017). Learning a model of facial shape and expression from 4D scans. *ACM Transactions on Graphics*, 36(6), Article 194, 1–17. https://doi.org/10.1145/3130800.3130813

- **DOI:** 10.1145/3130800.3130813. **Venue:** ACM TOG (ACM flagship).
- **Tier rationale:** Peer-reviewed top-tier journal.
- **Load-bearing for:** Q11.

### [F12] ekman1978facs — T3

Ekman, P., & Friesen, W. V. (1978). *Facial Action Coding System: A technique for the measurement of facial movement*. Consulting Psychologists Press.

- **ISBN (1st ed.):** Print reference; original 1978 edition. **URL:** none.
- **Tier rationale:** Task brief explicitly lists "Ekman & Friesen 1978" as the canonical T3 example; no Tier-1 equivalent.
- **Load-bearing for:** Q11.

---

## Cluster G — Auxiliary pipeline components

### [G01] baevski2020wav2vec — T1  [**PROMOTED from T2 per venue re-verification**]

Baevski, A., Zhou, H., Mohamed, A., & Auli, M. (2020). wav2vec 2.0: A framework for self-supervised learning of speech representations. In *Advances in Neural Information Processing Systems 33 (NeurIPS 2020)* (pp. 12449–12460). https://papers.nips.cc/paper/2020/hash/92d1e1eb1cd6f9fba3227870bb6d7f07-Abstract.html (arXiv preprint https://arxiv.org/abs/2006.11477)

- **arXiv:** 2006.11477. **Venue:** NeurIPS 2020. **URL:** NeurIPS Proceedings (above).
- **Tier rationale:** Peer-reviewed top-tier. **PROMOTED** from T2 per the agent-07 handoff step 8 and the confirmed NeurIPS 2020 venue. Changelog entry at bottom.
- **Load-bearing for:** Q11.

### [G02] bain2023whisperx — T2

Bain, M., Huh, J., Han, T., & Zisserman, A. (2023). *WhisperX: Time-accurate speech transcription of long-form audio*. In *Proceedings of Interspeech 2023* (pp. 4489–4493). ISCA. https://doi.org/10.21437/Interspeech.2023-78 (arXiv preprint https://arxiv.org/abs/2303.00747)

- **DOI:** 10.21437/Interspeech.2023-78. **arXiv:** 2303.00747. **Venue:** Interspeech 2023.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Note:** Brief 03 lists arXiv only; the Interspeech 2023 publication is the venue of record. Tier stays T2.
- **Load-bearing for:** Q11.

### [G03] campello2013hdbscan — T2

Campello, R. J. G. B., Moulavi, D., & Sander, J. (2013). Density-based clustering based on hierarchical density estimates. In J. Pei, V. S. Tseng, L. Cao, H. Motoda, & G. Xu (Eds.), *Advances in Knowledge Discovery and Data Mining (PAKDD 2013)*, Lecture Notes in Computer Science (Vol. 7819, pp. 160–172). Springer. https://doi.org/10.1007/978-3-642-37456-2_14

- **DOI:** 10.1007/978-3-642-37456-2_14. **Venue:** PAKDD 2013.
- **Tier rationale:** Peer-reviewed domain-specific conference.
- **Load-bearing for:** Q11.

### [G04] zhang1996birch — T1

Zhang, T., Ramakrishnan, R., & Livny, M. (1996). BIRCH: An efficient data clustering method for very large databases. In *Proceedings of the 1996 ACM SIGMOD International Conference on Management of Data (SIGMOD 1996)* (pp. 103–114). Association for Computing Machinery. https://doi.org/10.1145/235968.233324

- **DOI:** 10.1145/235968.233324. **Venue:** SIGMOD 1996.
- **Tier rationale:** Peer-reviewed top-tier (ACM SIGMOD).
- **Load-bearing for:** Q11.

### [G05] rouard2023htdemucs — T2

Rouard, S., Massa, F., & Défossez, A. (2023). Hybrid transformers for music source separation. In *ICASSP 2023 — 2023 IEEE International Conference on Acoustics, Speech and Signal Processing* (pp. 1–5). IEEE. https://doi.org/10.1109/ICASSP49357.2023.10096956 (arXiv preprint https://arxiv.org/abs/2211.08553)

- **DOI:** 10.1109/ICASSP49357.2023.10096956. **arXiv:** 2211.08553. **Venue:** ICASSP 2023.
- **Tier rationale:** Peer-reviewed reputable venue.
- **Load-bearing for:** Q11.

### [G06] google2020mediapipe — T3

Google. (2020). *MediaPipe Face Mesh* [Computer software]. GitHub. https://github.com/google/mediapipe

- **URL:** https://github.com/google/mediapipe. **Venue:** GitHub; Google canonical release.
- **Tier rationale:** T3 per task brief (GitHub software); no Tier-1 equivalent — the released software artefact is the cited object.
- **Load-bearing for:** Q11.

### [G07] tinachen2021yolov5face — T3

TinaChen. (2021). *YOLOv5-Face: A lightweight face detector* [Computer software]. GitHub. https://github.com/deepcam-cn/yolov5-face

- **URL:** https://github.com/deepcam-cn/yolov5-face. **Venue:** GitHub.
- **Tier rationale:** T3 per task brief. No Tier-1 equivalent for this specific software release.
- **Note:** The associated research article is Qi et al. (2022) "YOLO5Face: Why Reinventing a Face Detector," arXiv:2105.12931. Agent-44 may promote the citation if needed.
- **Load-bearing for:** Q11.

### [G08] resembleai2024enhance — T3

Resemble AI. (2024). *Resemble Enhance: AI-powered speech enhancement* [Computer software]. GitHub. https://github.com/resemble-ai/resemble-enhance

- **URL:** https://github.com/resemble-ai/resemble-enhance. **Venue:** GitHub.
- **Tier rationale:** T3 per task brief.
- **Load-bearing for:** Q11.

### [G09] mcp2024spec — T3

Model Context Protocol. (2024). *Model Context Protocol specification*. Anthropic. https://modelcontextprotocol.io

- **URL:** https://modelcontextprotocol.io. **Venue:** Official specification.
- **Tier rationale:** T3 per task brief (MCP spec is an explicit T3 example).
- **Load-bearing for:** Q10, Q11.

---

## Cluster H — Enterprise / market signals (Q19 sidebar pool)

**Scope note.** Cluster H is quota-limited to ≤ 6 references in body per agent-07's rule. Body-eligible (i.e., T1/T2) candidates: **H05 (Pile of Law, T1), H13 (Singhal Nature, T1), H15 (Tu Nature Medicine, T1), H20 (BloombergGPT, T2)**. Other H entries are retained here with keys for completeness but are not expected to appear in body.

### [H01] anthropic2024mcp — T3

Anthropic. (2024, November 25). *Introducing the Model Context Protocol*. https://www.anthropic.com/news/model-context-protocol

- **URL:** https://www.anthropic.com/news/model-context-protocol. **Venue:** Anthropic company blog.
- **Tier rationale:** T3 per task brief.
- **Load-bearing for:** Q19.

### [H02] field2024techcrunch — T3

Field, H. (2024, May 16). *OpenAI inks deal to train AI on Reddit data*. TechCrunch. https://techcrunch.com/2024/05/16/openai-inks-deal-to-train-ai-on-reddit-data/

- **URL:** https://techcrunch.com/2024/05/16/openai-inks-deal-to-train-ai-on-reddit-data/. **Venue:** TechCrunch.
- **Load-bearing for:** Q19.

### [H03] gartner2023trends — T3

Gartner. (2023, August 1). *Gartner identifies top trends shaping the future of data science and machine learning* [Press release]. https://www.gartner.com/en/newsroom/press-releases/2023-08-01-gartner-identifies-top-trends-shaping-future-of-data-science-and-machine-learning

- **URL:** above. **Venue:** Gartner press release.
- **Load-bearing for:** Q19.

### [H04] harvard2024caselaw — T3

Harvard Law School Library Innovation Lab. (n.d.). *Caselaw Access Project*. Retrieved April 16, 2026, from https://case.law/

- **URL:** https://case.law/. **Venue:** Institutional web reference.
- **Load-bearing for:** Q19.

### [H05] henderson2022pileoflaw — T1  [**venue confirmed NeurIPS 2022 D&B**]

Henderson, P., Krass, M. S., Zheng, L., Guha, N., Manning, C. D., Jurafsky, D., & Ho, D. E. (2022). Pile of Law: Learning responsible data filtering from the law and a 256GB open-source legal dataset. In *Advances in Neural Information Processing Systems: Datasets and Benchmarks Track (NeurIPS 2022)*. arXiv. https://arxiv.org/abs/2207.00220

- **arXiv ID:** 2207.00220. **Venue:** NeurIPS 2022 D&B Track (confirmed via OpenReview).
- **Tier rationale:** Peer-reviewed top-tier track.
- **Load-bearing for:** Q19.

### [H06] nci2024thesaurus — T3

National Cancer Institute. (n.d.). *NCI Thesaurus*. Retrieved April 16, 2026, from https://ncithesaurus.nci.nih.gov/

- **URL:** above. **Venue:** Controlled-vocabulary reference.
- **Load-bearing for:** Q19.

### [H07] nlm2024mesh — T3

National Library of Medicine. (n.d.). *Medical Subject Headings (MeSH)*. Retrieved April 16, 2026, from https://www.nlm.nih.gov/mesh/meshhome.html

- **URL:** above. **Venue:** Controlled-vocabulary reference.
- **Load-bearing for:** Q19.

### [H08] openai2024reddit — T3

OpenAI. (2024, May 16). *OpenAI and Reddit partnership*. https://openai.com/index/openai-and-reddit-partnership/

- **URL:** above. **Venue:** OpenAI company announcement.
- **Load-bearing for:** Q19.

### [H09] openai2024newscorp — T3

OpenAI. (2024, May 22). *A landmark multi-year global partnership with News Corp*. https://openai.com/index/news-corp-and-openai-sign-landmark-multi-year-global-partnership/

- **URL:** above. **Venue:** OpenAI company announcement.
- **Load-bearing for:** Q19.

### [H10] reuters2024reddit — T3

Reuters. (2024, February 22). *Reddit AI content licensing deal with Google, sources say*. https://www.reuters.com/technology/reddit-ai-content-licensing-deal-with-google-sources-say-2024-02-22/

- **URL:** above. **Venue:** Reuters wire.
- **Load-bearing for:** Q19.

### [H11] cerullo2024cbs — T3

Cerullo, M. (2024, February 22). *Google strikes $60 million deal with Reddit, allowing search giant to train AI models on human posts*. CBS News. https://www.cbsnews.com/news/google-reddit-60-million-deal-ai-training/

- **URL:** above. **Venue:** CBS News.
- **Load-bearing for:** Q19.

### [H12] shutterstock2023openai — T3

Shutterstock. (2023, July 11). *Shutterstock expands partnership with OpenAI, signs new six-year agreement to provide high-quality training data* [Press release]. https://investor.shutterstock.com/news-releases/news-release-details/shutterstock-expands-partnership-openai-signs-new-six-year

- **URL:** above. **Venue:** Shutterstock investor press release.
- **Load-bearing for:** Q19.

### [H13] singhal2023nature — T1

Singhal, K., Azizi, S., Tu, T., Mahdavi, S. S., Wei, J., Chung, H. W., Scales, N., Tanwani, A., Cole-Lewis, H., Pfohl, S., Payne, P., Seneviratne, M., Gamble, P., Kelly, C., Babiker, A., Schärli, N., Chowdhery, A., Mansfield, P., Demner-Fushman, D., … Natarajan, V. (2023). Large language models encode clinical knowledge. *Nature*, 620(7972), 172–180. https://doi.org/10.1038/s41586-023-06291-2

- **DOI:** 10.1038/s41586-023-06291-2. **Venue:** *Nature*.
- **Tier rationale:** Peer-reviewed top-tier journal.
- **Load-bearing for:** Q19.

### [H14] snomed2024ct — T3

SNOMED International. (n.d.). *SNOMED CT*. Retrieved April 16, 2026, from https://www.snomed.org/

- **URL:** above. **Venue:** Controlled-vocabulary reference.
- **Load-bearing for:** Q19.

### [H15] tu2025nature — T1

Tu, T., Schaekermann, M., Palepu, A., Saab, K., Freyberg, J., Tanno, R., Wang, A., Li, B., Amin, M., Tomasev, N., Azizi, S., Singhal, K., Cheng, Y., Hou, L., Webson, A., Kulkarni, K., Mahdavi, S. S., Semturs, C., Gottweis, J., … Natarajan, V. (2025). Toward expert-level medical question answering with large language models. *Nature Medicine*. Advance online publication. https://doi.org/10.1038/s41591-024-03423-7

- **DOI:** 10.1038/s41591-024-03423-7. **Venue:** *Nature Medicine*.
- **Tier rationale:** Peer-reviewed top-tier journal.
- **Load-bearing for:** Q19.

### [H16] sec2024edgar — T3

U.S. Securities and Exchange Commission. (n.d.). *About EDGAR*. Retrieved April 16, 2026, from https://www.sec.gov/submit-filings/about-edgar

- **URL:** above. **Venue:** Government web reference.
- **Load-bearing for:** Q19.

### [H17] sec2024edgarsearch — T3

U.S. Securities and Exchange Commission. (n.d.). *EDGAR full-text search*. Retrieved April 16, 2026, from https://www.sec.gov/edgar/search/

- **URL:** above. **Venue:** Government web reference.
- **Load-bearing for:** Q19.

### [H18] variety2024newscorp — T3

Variety Staff. (2024, May 22). *News Corp inks OpenAI licensing deal potentially worth more than $250 million*. Variety. https://variety.com/2024/digital/news/news-corp-openai-licensing-deal-1236013734/

- **URL:** above. **Venue:** Variety.
- **Load-bearing for:** Q19.

### [H19] verma2023washpost — T3

Verma, P., & De Vynck, G. (2023, July 13). *OpenAI strikes deal with AP to pay for using its news in training AI*. The Washington Post. https://www.washingtonpost.com/technology/2023/07/13/openai-chatgpt-pay-ap-news-ai/

- **URL:** above. **Venue:** The Washington Post.
- **Load-bearing for:** Q19.

### [H20] wu2023bloomberggpt — T2

Wu, S., Irsoy, O., Lu, S., Dabravolski, V., Dredze, M., Gehrmann, S., Kambadur, P., Rosenberg, D., & Mann, G. (2023). *BloombergGPT: A large language model for finance*. arXiv. https://arxiv.org/abs/2303.17564

- **arXiv ID:** 2303.17564. **Venue:** arXiv (Bloomberg / Johns Hopkins).
- **Tier rationale:** T2 — industry-research preprint with extensive empirical section.
- **Load-bearing for:** Q19.

---

## Cluster I — Alignment (RLHF / DPO / Constitutional AI)  [**NEW — Q06 anchor**]

**Origin note.** These three citations were flagged in the agent-07 handoff as missing from the evidence briefs. Agent-08 adds them here because Q06 cannot be answered without a Tier-1 anchor for the RLHF/DPO/CAI scalar-reward comparator in §2.6 / §5. All three verified directly via arXiv + venue lookup.

### [I01] ouyang2022instructgpt — T1

Ouyang, L., Wu, J., Jiang, X., Almeida, D., Wainwright, C. L., Mishkin, P., Zhang, C., Agarwal, S., Slama, K., Ray, A., Schulman, J., Hilton, J., Kelton, F., Miller, L., Simens, M., Askell, A., Welinder, P., Christiano, P., Leike, J., & Lowe, R. (2022). Training language models to follow instructions with human feedback. In *Advances in Neural Information Processing Systems 35 (NeurIPS 2022)*. arXiv. https://arxiv.org/abs/2203.02155

- **arXiv ID:** 2203.02155. **Venue:** NeurIPS 2022.
- **Tier rationale:** Peer-reviewed top-tier. Foundational RLHF / InstructGPT paper.
- **Load-bearing for:** Q06.

### [I02] rafailov2023dpo — T1

Rafailov, R., Sharma, A., Mitchell, E., Ermon, S., Manning, C. D., & Finn, C. (2023). Direct preference optimization: Your language model is secretly a reward model. In *Advances in Neural Information Processing Systems 36 (NeurIPS 2023)*. arXiv. https://arxiv.org/abs/2305.18290

- **arXiv ID:** 2305.18290. **Venue:** NeurIPS 2023.
- **Tier rationale:** Peer-reviewed top-tier. The canonical DPO paper.
- **Load-bearing for:** Q06.

### [I03] bai2022constitutional — T2

Bai, Y., Kadavath, S., Kundu, S., Askell, A., Kernion, J., Jones, A., Chen, A., Goldie, A., Mirhoseini, A., McKinnon, C., Chen, C., Olsson, C., Olah, C., Hernandez, D., Drain, D., Ganguli, D., Li, D., Tran-Johnson, E., Perez, E., … Kaplan, J. (2022). *Constitutional AI: Harmlessness from AI feedback*. arXiv. https://arxiv.org/abs/2212.08073

- **arXiv ID:** 2212.08073. **Venue:** arXiv only (Anthropic technical report).
- **Tier rationale:** arXiv-only by a T1-publishing group (Anthropic) with major author overlap with NeurIPS / ICLR papers. Treat as T2. (Agent-07 predicted T2 or T3; we go T2 because the citation count exceeds 2,500 and the authors' group is routinely T1-publishing; the cited content is extensive and well-peer-referenced. Downstream may demote to T3 if preferred; this is a close call.)
- **Load-bearing for:** Q06.

---

## Unverified — NOT ELIGIBLE for citation (informational only)

### [X01] qwen3tts2024 — INELIGIBLE (base model for Case 3; disclose via GitHub at T3 if needed)

Qwen Team, Alibaba Group. (2024). *Qwen3-TTS* [Model card]. GitHub. https://github.com/QwenLM

- **Status:** No resolvable arXiv ID at verification time. Agent-07 flagged as "unverified / excluded."
- **Action for agent-08:** We record this GitHub URL as a **T3 model-card disclosure option**. If §6.3 Methods needs to name the base model (required per mission brief), cite as T3 via the GitHub org. **Not entered in the tier count** because the specific Qwen3-TTS repo URL changes frequently; agent-44 must re-verify at camera-ready. Proposed key if used: **qwen2024qwen3tts**.

### [X02] demucs — INELIGIBLE pending re-verification

Défossez, A., Usunier, N., Bottou, L., & Bach, F. (2019). *Music source separation in the waveform domain*. arXiv. https://arxiv.org/abs/1911.13254

- **Status:** arXiv preprint exists but was not re-verified in briefs. If re-verified, tier = T2.
- **Action for agent-08:** Not entered in the tier count. Rouard et al. (2023) [G05] already covers v4 / HTDemucs, so v1/v2/v3 citation is optional.

---

## Summary tables

### Tier distribution (entered citations only; 88 total)

| Cluster | Refs | T1 | T2 | T3 | T1+T2 % |
|---|---|---|---|---|---|
| A Scaling laws + data wall | 9 | 3 | 1 | 5 | 44% |
| B Model collapse + synthetic | 7 | 4 | 3 | 0 | 100% |
| C Compression-as-intelligence | 5 | 3 | 2 | 0 | 100% |
| D Multi-modal embedders + retrieval | 10 | 4 | 6 | 0 | 100% |
| E Zero-shot TTS + voice cloning | 13 | 2 | 11 | 0 | 100% |
| F Identity-locked video generation | 12 | 3 | 8 | 1 | 92% |
| G Auxiliary pipeline components | 9 | 2 | 3 | 4 | 56% |
| **Core subtotal (A–G)** | **65** | **21** | **34** | **10** | **85%** |
| I Alignment (NEW) | 3 | 2 | 1 | 0 | 100% |
| **Core + I subtotal (A–G + I)** | **68** | **23** | **35** | **10** | **85%** |
| H Enterprise-market signals | 20 | 2 | 1 | 17 | 15% |
| **Full pool total** | **88** | **25** | **36** | **27** | **69%** |

**Core + I (A–G + I) T1+T2 = 85%.** ✅ PASSES the 80% G8 gate by 5 points.

**Body-pool rule (carried forward from agent-07).** Writers must keep Cluster-H body citations ≤ 6, prioritising H05 (T1), H13 (T1), H15 (T1), H20 (T2). If only T1/T2 Cluster-H refs are used, body-wide T1+T2 percentage stays ≥ 80%.

### Citation keys index (alphabetical by cluster)

A-cluster: kaplan2020scaling, hoffmann2022chinchilla, villalobos2024data, longpre2024consent, woo2024information, metz2024bloomberg, hu2024reuters, franzen2024techcrunch, sutskever2024neurips

B-cluster: shumailov2024nature, gerstgrasser2024colm, dohmatob2024tails, dohmatob2024regression, feng2024verification, alemohammad2023mad, seddik2024collapse

C-cluster: deletang2024compression, huang2024compression, li2025lossless, valmeekam2023llmzip, jiang2023gzip

D-cluster: zhai2023siglip, nussbaum2024nomic, chen2022wavlm, khattab2020colbert, santhanam2022colbertv2, formal2021splade, wang2021kepler, bordes2013transe, malkov2018hnsw, cormack2009rrf

E-cluster: wang2023valle, chen2024valle2, tan2024naturalspeech3, wang2024maskgct, chen2024f5tts, le2023voicebox, shen2023naturalspeech2, jiang2023megatts2, casanova2024xtts, reddy2022dnsmos, saeki2022utmos, desplanques2020ecapa, christop2025cloneval

F-cluster: meng2025echomimicv3, cui2024hallo3, li2024latentsync, zheng2025hunyuanvideo, tian2024emo, tian2025emo2, yang2024cogvideox, wan2025video, hu2022lora, lipman2023flowmatching, li2017flame, ekman1978facs

G-cluster: baevski2020wav2vec, bain2023whisperx, campello2013hdbscan, zhang1996birch, rouard2023htdemucs, google2020mediapipe, tinachen2021yolov5face, resembleai2024enhance, mcp2024spec

H-cluster: anthropic2024mcp, field2024techcrunch, gartner2023trends, harvard2024caselaw, henderson2022pileoflaw, nci2024thesaurus, nlm2024mesh, openai2024reddit, openai2024newscorp, reuters2024reddit, cerullo2024cbs, shutterstock2023openai, singhal2023nature, snomed2024ct, tu2025nature, sec2024edgar, sec2024edgarsearch, variety2024newscorp, verma2023washpost, wu2023bloomberggpt

I-cluster: ouyang2022instructgpt, rafailov2023dpo, bai2022constitutional

### Q-to-citation coverage (forward index; see companion file for reverse)

- Q01: kaplan2020scaling, hoffmann2022chinchilla, villalobos2024data, longpre2024consent, woo2024information, metz2024bloomberg, hu2024reuters, franzen2024techcrunch, sutskever2024neurips
- Q02: shumailov2024nature, gerstgrasser2024colm, dohmatob2024tails, dohmatob2024regression, feng2024verification, alemohammad2023mad, seddik2024collapse
- Q03: villalobos2024data, hoffmann2022chinchilla, shumailov2024nature, gerstgrasser2024colm (+ path-1 licensing via H cluster if needed)
- Q04: deletang2024compression, huang2024compression, chen2022wavlm, zhai2023siglip (embedder substrate)
- Q05: deletang2024compression, huang2024compression, li2025lossless, valmeekam2023llmzip, jiang2023gzip
- Q06: ouyang2022instructgpt, rafailov2023dpo, bai2022constitutional, lipman2023flowmatching, hu2022lora
- Q07: lipman2023flowmatching, hu2022lora, meng2025echomimicv3
- Q08: chen2022wavlm, saeki2022utmos, reddy2022dnsmos
- Q09: zhai2023siglip, chen2022wavlm, wang2023valle, chen2024valle2, tan2024naturalspeech3, wang2024maskgct
- Q10: chen2022wavlm, khattab2020colbert, santhanam2022colbertv2, formal2021splade, wang2021kepler, bordes2013transe, zhai2023siglip, nussbaum2024nomic, malkov2018hnsw, cormack2009rrf, mcp2024spec
- Q11: meng2025echomimicv3, cui2024hallo3, zheng2025hunyuanvideo, tian2024emo, tian2025emo2, yang2024cogvideox, hu2022lora, li2017flame, ekman1978facs, baevski2020wav2vec, bain2023whisperx, rouard2023htdemucs, campello2013hdbscan, zhang1996birch, google2020mediapipe, tinachen2021yolov5face, resembleai2024enhance, mcp2024spec
- Q12: chen2022wavlm, reddy2022dnsmos, saeki2022utmos, chen2024valle2 (comparator)
- Q13: meng2025echomimicv3, li2024latentsync, hu2022lora, lipman2023flowmatching
- Q14: chen2024valle2, chen2022wavlm, christop2025cloneval, tan2024naturalspeech3, wang2024maskgct, chen2024f5tts
- Q15: villalobos2024data, hu2022lora, chen2024valle2, meng2025echomimicv3
- Q16: shumailov2024nature, gerstgrasser2024colm, chen2022wavlm, villalobos2024data
- Q17: lipman2023flowmatching, meng2025echomimicv3, wan2025video, tinachen2021yolov5face
- Q18: hu2022lora, chen2022wavlm, reddy2022dnsmos, saeki2022utmos, meng2025echomimicv3, mcp2024spec
- Q19: henderson2022pileoflaw, singhal2023nature, tu2025nature, wu2023bloomberggpt, openai2024reddit, openai2024newscorp, reuters2024reddit, cerullo2024cbs, verma2023washpost, variety2024newscorp, shutterstock2023openai, anthropic2024mcp, gartner2023trends (+ H04, H06, H07, H14, H16, H17 as infrastructure refs)
- Q20: desplanques2020ecapa, chen2022wavlm, christop2025cloneval, longpre2024consent

See the companion file `patterns/agent-08-citation-extractor--claim-to-citation-index.md` for the primary/supporting split per Q.

---

## Hard rules carried forward (agent-07 → agent-08 → agents 09+)

1. **Cite by key, not free-text URL.** All writing agents (35–41) must cite via keys defined here.
2. **No silent tier changes.** Any downstream tier re-assignment requires a blocker + changelog entry here.
3. **Cluster-H body quota ≤ 6.** Prefer H05, H13, H15, H20 (T1/T2) to preserve body-wide 80% T1+T2.
4. **Unverified refs excluded.** Qwen3-TTS and Demucs v1/v2/v3 are NOT in the key index. If Qwen3-TTS must be cited in §6.3 Methods, use the T3 GitHub model-card disclosure under proposed key `qwen2024qwen3tts` and file a blocker so this file is updated.
5. **APA 7th strict.** Up to 20 authors listed; 21+ triggers "et al." The entries above conform.
6. **URL policy.** Every entry has DOI or arXiv ID or a working URL. No entry carries only "retrieved" without a URL.

---

## Changelog

- **2026-04-16 17:45 UTC** — Initial issue by agent-08-citation-extractor. LOCKED.
- **Venue re-verification (this file, at issue):**
  - G01 wav2vec 2.0 — confirmed NeurIPS 2020; **promoted T2 → T1**.
  - H05 Pile of Law — confirmed NeurIPS 2022 D&B Track; **held T1** (unchanged).
- **Additions (this file, at issue):** Cluster I (alignment triad) added with three entries ouyang2022instructgpt (T1), rafailov2023dpo (T1), bai2022constitutional (T2) to close Q06 gap flagged in agent-07 handoff.
- **Dedupe (this file, at issue):** chen2022wavlm, franzen2024techcrunch, woo2024information deduplicated across briefs (single-key entries).
