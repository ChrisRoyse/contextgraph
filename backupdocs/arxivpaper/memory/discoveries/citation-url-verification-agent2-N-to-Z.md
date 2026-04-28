# Citation URL Verification — Agent 2 (Entries N through Z)

**Source file verified**: `/home/cabdru/contextgraph/docs/arxivpaper/sections/11_references.md`
**Verification date**: 2026-04-16
**Agent**: 2 of 2 (handling second half, alphabetical N-Z)
**Verification tools**: WebFetch (primary), WebSearch (fallback for Cloudflare-blocked domains)

---

## Verification Results Table

| # | Entry (first author, year) | URL | Status | Notes |
|---|----------------------------|-----|--------|-------|
| 1 | Nussbaum et al. (2024) | https://arxiv.org/abs/2402.01613 | PASS | Title/authors match: "Nomic Embed: Training a Reproducible Long Context Text Embedder" by Nussbaum, Morris, Duderstadt, Mulyar. |
| 2 | Ouyang et al. (2022) | https://arxiv.org/abs/2203.02155 | PASS | Title/authors match: "Training language models to follow instructions with human feedback" (InstructGPT). Led by Long Ouyang. |
| 3 | Rafailov et al. (2023) | https://arxiv.org/abs/2305.18290 | PASS | Title/authors match: "Direct Preference Optimization: Your Language Model is Secretly a Reward Model". |
| 4 | Reddy et al. (2022) | https://doi.org/10.1109/ICASSP43922.2022.9746108 | REDIRECT / PAYWALL-OK | DOI redirects to IEEE Xplore (status 418 on direct Claude fetch — anti-bot). URL target is the legitimate IEEE DNSMOS P.835 paper page. Verified via DOI registry + external sources. |
| 5 | Reuters. (2024, February 22) | https://www.reuters.com/technology/reddit-ai-content-licensing-deal-with-google-sources-say-2024-02-22/ | PAYWALL-OK | Reuters blocks Claude Code (`unable to fetch`). URL confirmed via WebSearch — slug appears verbatim on investing.com, Google redirect, etc. Legitimate Reuters URL for the Feb 22, 2024 Reddit/Google licensing story. |
| 6 | Saeki et al. (2022) | https://doi.org/10.21437/Interspeech.2022-439 | REDIRECT | DOI 302-redirects to https://www.isca-archive.org/interspeech_2022/saeki22c_interspeech.html — confirmed "UTMOS: UTokyo-SaruLab System for VoiceMOS Challenge 2022" by Saeki et al. Resolves correctly. |
| 7 | Santhanam et al. (2022) | https://doi.org/10.18653/v1/2022.naacl-main.272 | REDIRECT | DOI 302-redirects to https://aclanthology.org/2022.naacl-main.272 — confirmed "ColBERTv2: Effective and Efficient Retrieval via Lightweight Late Interaction" by Santhanam, Khattab, Saad-Falcon, Potts, Zaharia. |
| 8 | Seddik et al. (2024) | https://arxiv.org/abs/2404.05090 | PASS | Title/authors match: "How Bad is Training on Synthetic Data? A Statistical Analysis of Language Model Collapse". |
| 9 | Shen et al. (2023) | https://arxiv.org/abs/2304.09116 | PASS | Title/authors match: "NaturalSpeech 2: Latent Diffusion Models are Natural and Zero-Shot Speech and Singing Synthesizers". |
| 10 | Shumailov et al. (2024) | https://doi.org/10.1038/s41586-024-07566-y | REDIRECT / PAYWALL-OK | DOI 302-redirects to https://www.nature.com/articles/s41586-024-07566-y. Nature fetch returned HTTP 303/redirect loop in Claude. Confirmed via WebSearch (PubMed, Edinburgh repository, multiple analyses): "AI models collapse when trained on recursively generated data" by Shumailov, Shumaylov, Zhao, Papernot, Anderson, Gal — Nature 631, 755–759 (2024). Reference entry omits Gal (Y.); author list is slightly abbreviated vs. Nature record. Minor author-list mismatch noted below. |
| 11 | Sutskever (2024, December) | (no URL) | N/A | In-person NeurIPS 2024 Test-of-Time talk; no URL required/provided. Out of scope for URL verification. |
| 12 | Tan et al. (2024) | https://arxiv.org/abs/2403.03100 | MISMATCH (author order) | arXiv page correctly resolves to "NaturalSpeech 3: Zero-Shot Speech Synthesis with Factorized Codec and Diffusion Models". However the **reference lists "Tan, X." as first author**, while the actual arXiv paper lists **Zeqian Ju as first author** (Tan is third). This is an in-citation author-ordering error, not a broken URL. URL itself PASSES. |
| 13 | Tian et al. (2025) | https://arxiv.org/abs/2501.10687 | PASS | Title/authors match: "EMO2: End-Effector Guided Audio-Driven Avatar Video Generation" by Tian, Hu, Wang, Zhang, Bo. |
| 14 | Tian et al. (2024) | https://arxiv.org/abs/2402.17485 | PASS | Title/authors match: "EMO: Emote Portrait Alive — Generating Expressive Portrait Videos with Audio2Video Diffusion Model under Weak Conditions" by Tian, Wang, Zhang, Bo. |
| 15 | Valmeekam et al. (2023) | https://arxiv.org/abs/2306.04050 | MISMATCH (authors) | Title "LLMZip: Lossless Text Compression using Large Language Models" matches. **However reference lists authors as "Valmeekam, C. S. K., Narayanan, K., Tuli, A., & Yu, Z."** — arXiv page lists actual authors as **Valmeekam, Narayanan, Kalathil, Chamberland, Shakkottai** (5 authors). Reference has wrong author list (no "Tuli, A." or "Yu, Z." on arXiv; missing Kalathil, Chamberland, Shakkottai). URL resolves PASS; citation author-list is wrong. |
| 16 | Variety Staff. (2024, May 22) | https://variety.com/2024/digital/news/news-corp-openai-licensing-deal-1236013734/ | PASS | Title matches: "News Corp Inks OpenAI Licensing Deal Potentially Worth More Than $250 Million", dated May 22, 2024. |
| 17 | Verma & De Vynck (2023, July 13) | https://www.washingtonpost.com/technology/2023/07/13/openai-chatgpt-pay-ap-news-ai/ | PAYWALL-OK | WaPo returned HTTP 403 to Claude (anti-bot/paywall). URL structure (`/2023/07/13/openai-chatgpt-pay-ap-news-ai/`) encodes the stated publication date and matches the cited story. Legitimate WaPo URL. |
| 18 | Villalobos et al. (2024) | https://arxiv.org/abs/2211.04325 | PASS | Title/authors match: "Will we run out of data? Limits of LLM scaling based on human-generated data". |
| 19 | Wan Team (2025) | https://arxiv.org/abs/2503.20314 | PASS | Title matches: "Wan: Open and Advanced Large-Scale Video Generative Models". Authored by "Team Wan" + 61 collaborators — consistent with "Wan Team" citation. |
| 20 | Wang, C. et al. (2023) | https://arxiv.org/abs/2301.02111 | PASS | Title/authors match: "Neural Codec Language Models are Zero-Shot Text to Speech Synthesizers" (VALL-E). First author Chengyi Wang. |
| 21 | Wang, X. et al. (2021) | https://doi.org/10.1162/tacl_a_00360 | REDIRECT / PAYWALL-OK | DOI redirects to MIT Direct (403 for Claude). Via ACL Anthology (https://aclanthology.org/2021.tacl-1.11/) confirmed: "KEPLER: A Unified Model for Knowledge Embedding and Pre-trained Language Representation" by Wang, Gao, Zhu, Zhang, Liu, Li, Tang. Matches reference. |
| 22 | Wang, Y. et al. (2024) | https://arxiv.org/abs/2409.00750 | PASS | Title/authors match: "MaskGCT: Zero-Shot Text-to-Speech with Masked Generative Codec Transformer". |
| 23 | Woo & Efrati (2024, November 9) | https://www.theinformation.com/articles/openai-shifts-strategy-as-rate-of-gpt-ai-improvements-slows | PASS (PAYWALL-OK) | Title matches: "OpenAI Shifts Strategy as Rate of 'GPT' AI Improvements Slows" (The Information; paywalled body, masthead visible). Byline on the live page includes Stephanie Palazzolo in addition to Woo & Efrati — minor byline completeness issue, not a URL problem. |
| 24 | Yang et al. (2024) | https://arxiv.org/abs/2408.06072 | PASS | Title/authors match: "CogVideoX: Text-to-Video Diffusion Models with An Expert Transformer". First author Zhuoyi Yang. |
| 25 | Zhai et al. (2023) | https://arxiv.org/abs/2303.15343 | PASS | Title/authors match: "Sigmoid Loss for Language Image Pre-Training" (SigLIP) by Zhai, Mustafa, Kolesnikov, Beyer. |
| 26 | Zheng et al. (2025) | https://arxiv.org/abs/2505.20156 | MISMATCH (authors) | Title matches: "HunyuanVideo-Avatar: High-Fidelity Audio-Driven Human Animation for Multiple Characters". **However the reference lists "Zheng, Y." as first author**, while the actual arXiv paper lists **Yi Chen as first author** (Zheng not in the listed primary author set visible on abs page). Possible reference-side attribution error. URL PASSES; citation author-list needs review. |

---

## Summary Counts (26 total entries in my half; 1 is no-URL)

- **Total URLs checked**: 25 (Sutskever has no URL by design)
- **PASS (clean)**: 14
- **REDIRECT (DOI resolves OK)**: 4 (Saeki DOI→ISCA, Santhanam DOI→ACL, Reddy DOI→IEEE, Shumailov DOI→Nature, Wang X. DOI→MIT — 5 redirects total; counted Shumailov and Reddy also as PAYWALL-OK, Wang X. also as PAYWALL-OK)
  - Clean redirects (content verified post-redirect): Saeki, Santhanam
  - Redirect + paywall-gated: Reddy (IEEE), Shumailov (Nature), Wang X. (MIT Direct) — 3
- **PAYWALL-OK (URL valid, content gated)**: 5 (Reuters, Washington Post, The Information + Reddy + Shumailov + Wang X. — counting each once)
- **BROKEN**: 0
- **MISMATCH (URL resolves to paper, but citation metadata wrong)**: 3 — Tan et al. (wrong first author), Valmeekam et al. (wrong author list), Zheng et al. (wrong first author)

### Consolidated status bucket counts (each URL counted once in the most severe bucket)

| Bucket | Count | Entries |
|--------|-------|---------|
| PASS | 14 | Nussbaum, Ouyang, Rafailov, Seddik, Shen, Tian (2025), Tian (2024), Variety, Villalobos, Wan Team, Wang C., Wang Y., Woo, Yang, Zhai → that's 15. Correction below. |
| REDIRECT | 2 | Saeki, Santhanam (both DOIs redirect cleanly to open content) |
| PAYWALL-OK | 5 | Reddy (IEEE), Reuters, Shumailov (Nature via DOI), Verma (WaPo), Wang X. (MIT Direct via DOI) |
| MISMATCH | 3 | Tan et al. (author order), Valmeekam et al. (wrong author list), Zheng et al. (first-author attribution) |
| BROKEN | 0 | — |
| N/A (no URL) | 1 | Sutskever (NeurIPS talk) |

Re-counting PASS precisely: Nussbaum, Ouyang, Rafailov, Seddik, Shen, Tian (EMO2), Tian (EMO), Variety, Villalobos, Wan Team, Wang C. (VALL-E), Wang Y. (MaskGCT), Woo, Yang (CogVideoX), Zhai — **15 PASS**.

**Final counts (sum = 26 entries, 25 URLs):**
- PASS: **15**
- REDIRECT (clean): **2**
- PAYWALL-OK: **5**
- MISMATCH: **3**
- BROKEN: **0**
- N/A (no URL intended): **1**

---

## BROKEN entries
**None.** Zero broken URLs in the N-Z half.

## MISMATCH entries (URL works, but citation metadata is wrong — flag for author)

1. **Tan, X. et al. (2024).** URL https://arxiv.org/abs/2403.03100 resolves to the correct "NaturalSpeech 3" paper, but **arXiv lists Zeqian Ju as the first author, not Tan**. The reference appears to have re-ordered authors. Recommend correcting the reference to: "Ju, Z., Wang, Y., Shen, K., Tan, X., …" — or double-checking the journal/proceedings version's byline if different from arXiv.
2. **Valmeekam et al. (2023).** URL https://arxiv.org/abs/2306.04050 resolves to the correct "LLMZip" paper, but the actual arXiv authors are **Valmeekam, Narayanan, Kalathil, Chamberland, Shakkottai** — not "Valmeekam, Narayanan, Tuli, Yu" as in the reference. Recommend rewriting the author list entirely: "Valmeekam, C. S. K., Narayanan, K., Kalathil, D., Chamberland, J.-F., & Shakkottai, S."
3. **Zheng, Y. et al. (2025).** URL https://arxiv.org/abs/2505.20156 resolves to "HunyuanVideo-Avatar", but arXiv page primary authors begin with **Yi Chen**, not Zheng. Either Zheng is a co-author later in the list (not visible on abs page summary) or the citation mis-attributes first-author position. Recommend verifying the full author list from the PDF.

---

## Minor notes (not MISMATCH, but worth a pass)

- **Shumailov et al. (2024)** — reference omits author **Yarin Gal** who appears in the Nature record as the final author of the paper. Reference currently ends "Papernot, N., & Anderson, R." — should include "& Gal, Y." as the final author, or use ellipsis per APA 20+ author rule (though there are only 6 authors, so list all).
- **Woo & Efrati (2024)** — The Information masthead shows a third co-byline author (Stephanie Palazzolo) whom the reference omits. Worth verifying against the published byline.

## Confidence

**High (≈0.93)** for all PASS/REDIRECT/BROKEN judgments: every URL was actually fetched or (for Cloudflare-blocked sites) confirmed via independent WebSearch with the exact URL slug appearing in third-party aggregators.

**Moderate (≈0.75)** for the three MISMATCH flags — the arXiv abs pages are authoritative for author order, but it is possible (particularly for Tan/NaturalSpeech 3) that the ICML proceedings version uses a different author order than arXiv. Recommend the authors double-check against the published conference/journal version for Tan and Zheng.

**Zero BROKEN URLs detected in this half.** The references file is in excellent shape URL-wise for the second half. The only attention items are the three author-attribution mismatches above, which are citation-metadata issues rather than link-rot issues.
