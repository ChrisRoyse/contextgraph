# Citation URL Verification — Agent 1 (A through M)

**Scope**: First-author surnames A through M in `/home/cabdru/contextgraph/docs/arxivpaper/sections/11_references.md`
**Verification date**: 2026-04-16
**Method**: WebFetch against each URL; compared page title/authors/year against cited work.

## Status legend
- **PASS**: URL resolves, content matches citation
- **REDIRECT-OK**: URL 30x redirects to a live page that matches the citation (e.g., DOI → publisher)
- **PAYWALL-OK**: URL resolves but content gated (Reuters, Bloomberg, ACM, IEEE, Nature). Redirect target is the expected publisher page.
- **MISMATCH**: URL works but cited metadata (author list, title) is wrong
- **BROKEN**: 404, DNS failure, or unreachable with no alternate

---

## Results table

| # | Entry | URL | Status | Notes |
|---|-------|-----|--------|-------|
| 1 | Alemohammad et al. (2023) — *Self-consuming generative models go MAD* | https://arxiv.org/abs/2307.01850 | PASS | Title and first authors match. |
| 2 | Anthropic (2024) — *Introducing the Model Context Protocol* | https://www.anthropic.com/news/model-context-protocol | PASS | Page title, date (Nov 25 2024), author (Anthropic) all match. |
| 3 | Bai et al. (2022) — *Constitutional AI: Harmlessness from AI feedback* | https://arxiv.org/abs/2212.08073 | PASS | Title, authors (Yuntao Bai, Saurav Kadavath, Sandipan Kundu…), date Dec 15 2022 all match. |
| 4 | Bordes et al. (2013) — Translating embeddings (TransE) | https://papers.nips.cc/paper_files/paper/2013/hash/1cecc7a77928ca8133fa24680a88d2f9-Abstract.html | PASS | NeurIPS 2013 abstract page, title+authors match exactly. |
| 5 | Casanova et al. (2024) — *XTTS* | https://arxiv.org/abs/2406.04904 | PASS | Title and full author list match. INTERSPEECH 2024 accepted. |
| 6 | Chen, S. et al. (2024) — *VALL-E 2* | https://arxiv.org/abs/2406.05370 | PASS | Title and authors (Sanyuan Chen, Shujie Liu, Long Zhou…) match. |
| 7 | Chen, S. et al. (2022) — WavLM (IEEE JSTSP) | https://doi.org/10.1109/JSTSP.2022.3188113 | PAYWALL-OK | DOI 302-redirects to `ieeexplore.ieee.org/document/9814838/`. IEEE returns 418 to bot but redirect target is correct. |
| 8 | Chen, Y. et al. (2024) — *F5-TTS* | https://arxiv.org/abs/2410.06885 | PASS | Title and authors match. |
| 9 | Christop et al. (2025) — *ClonEval* | https://arxiv.org/abs/2504.20581 | PASS | Title and authors (Christop, Kuczyński, Kubis) match. |
| 10 | Cormack, Clarke, Büttcher (2009) — Reciprocal Rank Fusion | https://doi.org/10.1145/1571941.1572114 | PAYWALL-OK | DOI 302-redirects to `dl.acm.org/doi/10.1145/1571941.1572114`. ACM returns 403 to bot — standard. |
| 11 | Cui et al. (2024) — *Hallo3* | https://arxiv.org/abs/2412.00733 | PASS | Title and 10-author list match. |
| 12 | Delétang et al. (2024) — *Language modeling is compression* | https://arxiv.org/abs/2309.10668 | PASS | Title and first author match. |
| 13 | Desplanques, Thienpondt, Demuynck (2020) — ECAPA-TDNN | https://doi.org/10.21437/Interspeech.2020-2650 | REDIRECT-OK | DOI 302-redirects to `isca-archive.org/interspeech_2020/desplanques20_interspeech.html`. Content verified to match citation. |
| 14 | Dohmatob et al. (2024) — *A tale of tails* | https://arxiv.org/abs/2402.07043 | PASS | Title and authors match, ICML 2024 accepted. |
| 15 | Ekman & Friesen (1978) — *FACS* (book) | (no URL in reference) | N/A | Print-only citation; no URL to verify. |
| 16 | Feng et al. (2024) — *Beyond model collapse* | https://arxiv.org/abs/2406.07515 | PASS | Title and authors match exactly. |
| 17 | Field (2024) — TechCrunch Reddit/OpenAI | https://techcrunch.com/2024/05/16/openai-inks-deal-to-train-ai-on-reddit-data/ | **MISMATCH** | URL loads; article exists on May 16 2024 about OpenAI/Reddit deal — but byline is **Kyle Wiggers**, not Hayden Field. Author in citation is wrong. |
| 18 | Formal, Piwowarski, Clinchant (2021) — SPLADE | https://doi.org/10.1145/3404835.3463098 | PAYWALL-OK | DOI 302-redirects to `dl.acm.org/doi/10.1145/3404835.3463098`. ACM 403 to bot — standard. |
| 19 | Franzen (2024) — TechCrunch AI scaling diminishing returns | https://techcrunch.com/2024/11/20/ai-scaling-laws-are-showing-diminishing-returns-forcing-ai-labs-to-change-course/ | **MISMATCH** | URL loads; article on Nov 20 2024 about AI scaling diminishing returns — but byline is **Maxwell Zeff** (Senior AI Reporter), not Carl Franzen. Author in citation is wrong. |
| 20 | Gerstgrasser et al. (2024) — *Is model collapse inevitable?* | https://arxiv.org/abs/2404.01413 | PASS | Title and 14-author list match. |
| 21 | Hoffmann et al. (2022) — Chinchilla / compute-optimal LLMs | https://arxiv.org/abs/2203.15556 | PASS | Title and first author (Jordan Hoffmann) match. |
| 22 | Hu, E. J. et al. (2022) — *LoRA* | https://arxiv.org/abs/2106.09685 | PASS | Title and authors match. arXiv submission 2021; ICLR 2022 publication year matches citation. |
| 23 | Hu, K., & Tong, A. (2024) — Reuters on AI scaling limits | https://www.reuters.com/technology/artificial-intelligence/openai-rivals-seek-new-path-smarter-ai-current-methods-hit-limitations-2024-11-11/ | PAYWALL-OK | Reuters blocks Claude Code's fetcher entirely (expected). URL slug matches cited article headline and date range; assume live. Recommend manual spot-check. |
| 24 | Huang et al. (2024) — *Compression represents intelligence linearly* | https://arxiv.org/abs/2404.09937 | **MISMATCH** | Title matches; COLM 2024 matches. But actual authors are **Yuzhen Huang, Jinghan Zhang, Zifei Shan, Junxian He** — 4 authors, not the 9 cited ("Huang, Y., Vincent, F., Ye, K., Xu, Z., Chang, J., Xu, D., Yin, X., Yang, D., & Xiao, C."). Cited author list is fabricated or confused with another paper. |
| 25 | Jiang, Z. et al. (2023) — Findings ACL 2023 gzip+kNN | https://aclanthology.org/2023.findings-acl.426/ | **MISMATCH** | Title and venue match. But actual authors are **Zhiying Jiang, Matthew Yang, Mikhail Tsirlin, Raphael Tang, Yiqin Dai, Jimmy Lin** — cited as "Jiang, Z., Yang, M., Cui, M., & Han, J." (last two authors wrong; missing Tsirlin, Tang, Dai, Lin). |
| 26 | Jiang, Z. et al. (2023) — *Mega-TTS 2* | https://arxiv.org/abs/2307.07218 | PASS (title note) | arXiv current title is "Mega-TTS 2: Boosting Prompting Mechanisms for Zero-Shot Speech Synthesis"; citation uses original v1 subtitle "Zero-shot text-to-speech with arbitrary-length speech prompts." Same paper; subtitle was revised. Authors unchanged. Consider updating subtitle in reference. |
| 27 | Kaplan et al. (2020) — *Scaling laws for neural language models* | https://arxiv.org/abs/2001.08361 | PASS | Title and first authors (Kaplan, McCandlish, Henighan, Brown) match. |
| 28 | Khattab & Zaharia (2020) — ColBERT | https://doi.org/10.1145/3397271.3401075 | PAYWALL-OK | DOI 302-redirects to `dl.acm.org/doi/10.1145/3397271.3401075`. ACM paywall. |
| 29 | Le et al. (2023) — *Voicebox* | https://arxiv.org/abs/2306.15687 | PASS | Title, authors, NeurIPS 2023 accepted — all match. |
| 30 | Li, C. et al. (2025) — Lossless compression (Nature MI) | https://doi.org/10.1038/s42256-025-01033-7 | **MISMATCH** | DOI resolves. Actual paper: "**Lossless data compression by large models**" (not "…by large language models"). Actual authors per Crossref: **Ziguang Li, Chao Huang, Xuliang Wang, Haibo Hu, Cole Wyeth, Dongbo Bu, Quan Yu, Wen Gao, Xingwu Liu, Ming Li** — completely different from cited "Li, C., Yang, L., Zhang, K., Wang, F., Liu, Y., Zhang, Z., & Sun, Z." Either wrong DOI or wrong author list. Nature Machine Intelligence 7(5), May 2025 is correct venue. |
| 31 | Li, C. et al. (2024) — *LatentSync* | https://arxiv.org/abs/2412.09262 | **MISMATCH** | Title matches core "LatentSync". Actual subtitle is "Taming Audio-Conditioned Latent Diffusion Models for Lip Sync with SyncNet Supervision" (not exact match). Actual authors: **Chunyu Li, Chao Zhang, Weikai Xu, Jingyu Lin, Jinghui Xie, Weiguo Feng, Bingyue Peng, Cunjian Chen, Weiwei Xing**. Cited authors differ beyond position 5: "…Feng, S., Zhou, X., Yan, M., & Huang, F." — wrong. |
| 32 | Li, T. et al. (2017) — FLAME (ACM TOG) | https://doi.org/10.1145/3130800.3130813 | PAYWALL-OK | DOI 302-redirects to `dl.acm.org/doi/10.1145/3130800.3130813`. ACM paywall. |
| 33 | Lipman et al. (2023) — *Flow matching for generative modeling* | https://arxiv.org/abs/2210.02747 | PASS | Title and authors (Lipman, Chen, Ben-Hamu, Nickel, Le) match. |
| 34 | Longpre et al. (2024) — *Consent in crisis* | https://arxiv.org/abs/2407.14933 | PASS | Title and first-author match. NeurIPS 2024 D&B Track. |
| 35 | Malkov & Yashunin (2018) — HNSW (IEEE TPAMI) | https://doi.org/10.1109/TPAMI.2018.2889473 | PAYWALL-OK | DOI 302-redirects to `ieeexplore.ieee.org/document/8594636/`. IEEE paywall. |
| 36 | Meng et al. (2025) — *EchoMimicV3* | https://arxiv.org/abs/2507.03905 | PASS | Title and first author (Rang Meng) match. |
| 37 | Metz, Ghaffary, Love (2024) — Bloomberg on AI scaling | https://www.bloomberg.com/news/articles/2024-11-13/openai-google-and-anthropic-are-struggling-to-build-more-advanced-ai | PAYWALL-OK | Bloomberg returned 403 to fetcher (expected behavior for bots). URL slug matches cited headline and date. |

---

## Totals

- **URLs checked**: 36 (entry 15 Ekman has no URL)
- **PASS**: 21
- **REDIRECT-OK** (matched after following redirect): 1 (Desplanques → ISCA archive)
- **PAYWALL-OK** (DOI/publisher resolves; content gated): 9 (IEEE ×2, ACM ×4, Reuters ×1, Bloomberg ×1, Nature 303 ×0 — see MISMATCH note)
- **MISMATCH**: 6
  - Field (2024) TechCrunch — author is Wiggers, not Field
  - Franzen (2024) TechCrunch — author is Zeff, not Franzen
  - Huang et al. (2024) COLM — wrong author list (real paper has 4 authors: Huang, Zhang, Shan, He)
  - Jiang, Z. et al. (2023) Findings ACL — wrong author list
  - Li, C. et al. (2025) Nature MI — wrong title wording AND wrong author list
  - Li, C. et al. (2024) LatentSync — partial author-list mismatch + subtitle incomplete
- **BROKEN**: 0

---

## Remediation needed (MISMATCH entries)

All 6 MISMATCH entries need author-list and/or title correction in `11_references.md`. None of the URLs are broken — the pages resolve fine. The issue is metadata accuracy in the APA entries.

### 1. Field (2024) — TechCrunch Reddit/OpenAI
Fix: author is **Kyle Wiggers** (AI Editor), not Hayden Field. Full in-text change required wherever "(Field, 2024)" appears.

### 2. Franzen (2024) — TechCrunch scaling laws
Fix: author is **Maxwell Zeff**, not Carl Franzen. Full in-text change required.

### 3. Huang et al. (2024) — Compression Represents Intelligence Linearly
Fix: authors are **Huang, Y., Zhang, J., Shan, Z., & He, J.** — four authors only. Cited 9-author list is wrong (likely cross-contaminated with a different paper).

### 4. Jiang, Z. et al. (2023) — gzip + kNN (Findings ACL)
Fix: authors are **Jiang, Z., Yang, M., Tsirlin, M., Tang, R., Dai, Y., & Lin, J.** Cited "Cui, M., & Han, J." are incorrect; missing 3 other authors.

### 5. Li, C. et al. (2025) — Nature Machine Intelligence lossless compression
Fix: title is "**Lossless data compression by large models**" (not "…by large language models"). Authors are **Li, Z., Huang, C., Wang, X., Hu, H., Wyeth, C., Bu, D., Yu, Q., Gao, W., Liu, X., & Li, M.** First author is Ziguang Li, not "Li, C." Cited author list appears fabricated.

### 6. Li, C. et al. (2024) — LatentSync
Fix: full subtitle is "Taming Audio-Conditioned Latent Diffusion Models for Lip Sync with SyncNet Supervision". Authors are **Li, C., Zhang, C., Xu, W., Lin, J., Xie, J., Feng, W., Peng, B., Chen, C., & Xing, W.** Cited "Feng, S., Zhou, X., Yan, M., & Huang, F." are wrong.

---

## Minor flags (no remediation required, but noted)

- **Jiang, Z. et al. (2023) — Mega-TTS 2**: arXiv subtitle has evolved from the v1 submission. Current: "Boosting Prompting Mechanisms for Zero-Shot Speech Synthesis". Cited subtitle "Zero-shot text-to-speech with arbitrary-length speech prompts" was the original. Both refer to the same paper/arXiv ID. Optional: update subtitle to current form.

- **Paywall-OK entries (ACM, IEEE, Nature, Reuters, Bloomberg)**: DOI/URL resolution is correct; content is gated. These were verified via DOI redirect target hostnames and structure. A future human reviewer with institutional access should spot-check content alignment for the three ACM and two IEEE DOIs.

---

## Confidence

**High** for PASS/REDIRECT-OK (23 entries) — content was directly read and compared.

**High** for the 6 MISMATCH findings — these are metadata errors in the reference list itself, confirmed by reading the page (2 TechCrunch bylines, 3 arXiv abstract pages, 1 Crossref record).

**Medium** for PAYWALL-OK (9 entries) — DOI/URL resolves to the expected hostname, but full content verification was blocked by publisher bot policies. Structural matches (correct DOI target; Reuters/Bloomberg slug matches headline) give high-to-medium confidence the live page is the cited work.
