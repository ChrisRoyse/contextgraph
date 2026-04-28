# Citation Recheck — Agent 5 of 5 (Sutskever through Zhai)

**Date**: 2026-04-16
**File audited**: `/home/cabdru/contextgraph/docs/arxivpaper/sections/11_references.md`
**Method**: Independent WebFetch retrieval of every URL; content, first-author, and year compared to cited entry.

## Result table

| # | Cited entry | URL / venue | WebFetch title match | First author / byline match | Year match | Verdict |
|---|-------------|-------------|----------------------|-----------------------------|------------|---------|
| 1 | Sutskever (2024). *Pre-training as we know it will end* [NeurIPS talk] | no URL | n/a (talk, not fetchable) | n/a | n/a | N/A (no URL to verify — entry correctly provides no link for the oral presentation) |
| 2 | Tian, L., Hu, S., Wang, Q., Zhang, B., & Bo, L. (2025). *EMO2: End-effector guided audio-driven avatar video generation* | https://arxiv.org/abs/2501.10687 | "EMO2: End-Effector Guided Audio-Driven Avatar Video Generation" — MATCH | Linrui Tian — MATCH | submitted Jan 18, 2025 — MATCH | PASS |
| 3 | Tian, L., Wang, Q., Zhang, B., & Bo, L. (2024). *EMO: Emote portrait alive — ...* | https://arxiv.org/abs/2402.17485 | "EMO: Emote Portrait Alive — Generating Expressive Portrait Videos with Audio2Video Diffusion Model under Weak Conditions" — MATCH | Linrui Tian — MATCH | Feb 27, 2024 — MATCH | PASS |
| 4 | Valmeekam, C. S. K., Narayanan, K., Kalathil, D., Chamberland, J.-F., & Shakkottai, S. (2023). *LLMZip: Lossless text compression using large language models* | https://arxiv.org/abs/2306.04050 | "LLMZip: Lossless Text Compression using Large Language Models" — MATCH | Valmeekam, Narayanan, Kalathil, Chamberland, Shakkottai — MATCH (order and names exact) | June 6, 2023 — MATCH | PASS |
| 5 | Variety Staff. (2024, May 22). *News Corp inks OpenAI licensing deal potentially worth more than $250 million* | https://variety.com/2024/digital/news/news-corp-openai-licensing-deal-1236013734/ | Title MATCH | Byline is **Todd Spangler** (not "Variety Staff") | May 22, 2024 — MATCH | MISMATCH (byline) — see note below |
| 6 | Verma, P., & De Vynck, G. (2023, July 13). *OpenAI strikes deal with AP to pay for using its news in training AI* | https://www.washingtonpost.com/technology/2023/07/13/openai-chatgpt-pay-ap-news-ai/ | Could not fetch (HTTP 403, Washington Post paywall/bot-block) | unverifiable via WebFetch | unverifiable via WebFetch | PAYWALL-OK (URL path matches date and slug exactly; prior pass verified) |
| 7 | Villalobos, P., Ho, A., Sevilla, J., Besiroglu, T., Heim, L., & Hobbhahn, M. (2024). *Will we run out of data?* | https://arxiv.org/abs/2211.04325 | "Will we run out of data? Limits of LLM scaling based on human-generated data" — MATCH | Villalobos, Ho, Sevilla, Besiroglu, Heim, Hobbhahn — MATCH | ICML 2024 latest rev June 4, 2024 — MATCH | PASS |
| 8 | Wan Team (2025). *Wan: Open and advanced large-scale video generative models* | https://arxiv.org/abs/2503.20314 | "Wan: Open and Advanced Large-Scale Video Generative Models" — MATCH | Team Wan + 61 co-authors — MATCH (consistent with collective "Wan Team" attribution) | March 26, 2025 — MATCH | PASS |
| 9 | Wang, C., Chen, S., Wu, Y., ... Wei, F. (2023). *Neural codec language models are zero-shot text to speech synthesizers* | https://arxiv.org/abs/2301.02111 | "Neural Codec Language Models are Zero-Shot Text to Speech Synthesizers" (VALL-E) — MATCH | Chengyi Wang first — MATCH | Jan 5, 2023 — MATCH | PASS |
| 10 | Wang, X., Gao, T., Zhu, Z., ... Tang, J. (2021). KEPLER. *TACL*, 9, 176–194. https://doi.org/10.1162/tacl_a_00360 | DOI → direct.mit.edu | HTTP 403 on publisher page (Cloudflare bot-block) | unverifiable via WebFetch | unverifiable via WebFetch | REDIRECT + PAYWALL-OK (DOI resolver redirect 302 is normal TACL behavior; DOI slug matches TACL article 00360) |
| 11 | Wang, Y., Zhan, H., Liu, L., ... Wu, Z. (2024). *MaskGCT* | https://arxiv.org/abs/2409.00750 | "MaskGCT: Zero-Shot Text-to-Speech with Masked Generative Codec Transformer" — MATCH | Yuancheng Wang first — MATCH | Sept 1, 2024 — MATCH | PASS |
| 12 | Wiggers, K. (2024, May 16). *OpenAI inks deal to train AI on Reddit data*. TechCrunch. | https://techcrunch.com/2024/05/16/openai-inks-deal-to-train-ai-on-reddit-data/ | Title MATCH | Byline Kyle Wiggers — MATCH | May 16, 2024 — MATCH | PASS |
| 13 | Woo, E., Efrati, A., & Palazzolo, S. (2024, November 9). *OpenAI shifts strategy as rate of 'GPT' AI improvements slows*. The Information. | https://www.theinformation.com/articles/openai-shifts-strategy-as-rate-of-gpt-ai-improvements-slows | Title MATCH | Byline confirms three authors: Stephanie Palazzolo, Erin Woo, Amir Efrati — MATCH (all three present) | Publication date not rendered on public snippet, but URL/slug matches Nov 9, 2024 report — ACCEPTABLE | PASS (byline) / PAYWALL-OK (date unverifiable in snippet) |
| 14 | Yang, Z., Teng, J., ... Tang, J. (2024). *CogVideoX* | https://arxiv.org/abs/2408.06072 | "CogVideoX: Text-to-Video Diffusion Models with An Expert Transformer" — MATCH | Zhuoyi Yang first — MATCH | August 12, 2024 — MATCH | PASS |
| 15 | Zeff, M. (2024, November 20). *Current AI scaling laws are showing diminishing returns, forcing AI labs to change course*. TechCrunch. | https://techcrunch.com/2024/11/20/ai-scaling-laws-are-showing-diminishing-returns-forcing-ai-labs-to-change-course/ | Title MATCH | Byline Maxwell Zeff — MATCH | November 20, 2024 — MATCH | PASS |
| 16 | Zhai, X., Mustafa, B., Kolesnikov, A., & Beyer, L. (2023). *Sigmoid loss for language image pre-training* | https://arxiv.org/abs/2303.15343 | "Sigmoid Loss for Language Image Pre-Training" (SigLIP) — MATCH | Xiaohua Zhai first — MATCH | March 27, 2023 — MATCH | PASS |

## MISMATCH details

### Variety Staff (2024) — BYLINE MISMATCH
- Cited as: **Variety Staff.** (2024, May 22).
- Actual byline per Variety.com: **Todd Spangler**, NY Digital Editor.
- Severity: LOW/MEDIUM. The article IS real, the date, title, and outlet are correct, and the URL is live. Only the author attribution is wrong.
- Recommended fix: Change the reference entry to `Spangler, T. (2024, May 22).` and alphabetize accordingly (moving it from "Variety Staff" position to "Spangler" position in the reference list).

## Special-check summary (all four verified)

1. Valmeekam et al. (2023) at arXiv 2306.04050 — authors confirmed: Valmeekam, Narayanan, Kalathil, Chamberland, Shakkottai. PASS.
2. Wiggers (2024) at techcrunch.com/2024/05/16/... — byline Kyle Wiggers confirmed. PASS.
3. Woo, Efrati, Palazzolo (2024) at theinformation.com — all three authors present in byline (order per site: Palazzolo, Woo, Efrati). PASS on author list; cosmetic order note only.
4. Zeff (2024) at techcrunch.com/2024/11/20/... — byline Maxwell Zeff confirmed. PASS.

## Counts

- PASS: 11 (entries 2, 3, 4, 7, 8, 9, 11, 12, 13, 14, 15, 16 — minus #13's date being PAYWALL-OK → treat as 11 clean PASS + #13 PASS-with-note)
- Strict tally:
  - PASS: 12 (#2, #3, #4, #7, #8, #9, #11, #12, #13, #14, #15, #16)
  - PAYWALL-OK: 2 (#6 Washington Post, #10 TACL/MIT Press)
  - MISMATCH: 1 (#5 Variety — byline is Todd Spangler, not "Variety Staff")
  - N/A (no URL by design): 1 (#1 Sutskever NeurIPS talk)
- BROKEN: 0
- REDIRECT only (no content issue): 1 (#10 DOI → direct.mit.edu, normal)

## Confidence

**High** on all PASS and MISMATCH verdicts — titles, arXiv IDs, authors, and years all directly read from live pages. **Medium-high** on PAYWALL-OK entries (#6, #10) because URL slugs, DOI, dates, and publisher match perfectly and prior validator pass confirmed them; only live content verification was blocked by Cloudflare/paywall.
