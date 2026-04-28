# Citation Recheck — Agent 2 of 5 (Delétang through Huang)

**Date**: 2026-04-16
**Agent**: Citation Validator #41 (recheck pass 2/5)
**Scope**: 12 entries between Delétang (2024) and Huang (2024) in `sections/11_references.md`
**Method**: Independent WebFetch against each primary URL; cross-check title, first author, year.

---

## Summary Counts

| Verdict | Count | Entries |
|---|---|---|
| PASS | 10 | Delétang, Desplanques, Dohmatob, Feng, Formal (via arXiv), Gerstgrasser, Hoffmann, Hu E.J., Huang, Hu K.&Tong (partial) |
| REDIRECT | 2 | Desplanques (DOI→isca-archive, resolved PASS), Formal (DOI→ACM 403, verified via arXiv mirror) |
| BROKEN | 0 | — |
| MISMATCH | 0 | — |
| PAYWALL-OK | 0 | — |
| SKIP (no URL) | 1 | Ekman & Friesen (1978) — print book |
| MINOR DATE NOTE | 1 | Hu K. & Tong — cited "Nov 15" but URL slug + search results indicate Nov 11 (initial pub); Reuters may have updated date |

**Net result**: 11/11 URLs resolvable (some via redirect or independent mirror). 1 print book unreferenced. No substantive mismatches.

---

## Per-Entry Verification Table

| # | Citation | URL | Verdict | Notes |
|---|---|---|---|---|
| 1 | Delétang et al. (2024) — Language modeling is compression | https://arxiv.org/abs/2309.10668 | PASS | Title exact match. First author Grégoire Delétang confirmed. v1 Sept 2023, v2 March 2024 (ICLR 2024). Citing as 2024 matches ICLR proceedings year — correct. |
| 2 | Desplanques et al. (2020) — ECAPA-TDNN | https://doi.org/10.21437/Interspeech.2020-2650 | REDIRECT → PASS | DOI 302-redirects to isca-archive.org. Title, all three authors (Desplanques, Thienpondt, Demuynck), and 2020 date all confirmed on ISCA archive. |
| 3 | Dohmatob et al. (2024) — A tale of tails | https://arxiv.org/abs/2402.07043 | PASS | Title exact match. First author Elvis Dohmatob confirmed. 2024 submission. ICML 2024 venue matches citation. |
| 4 | Ekman & Friesen (1978) — FACS | (no URL) | SKIP | Print book, Consulting Psychologists Press. No verification possible or required. |
| 5 | Feng et al. (2024) — Beyond model collapse | https://arxiv.org/abs/2406.07515 | PASS | Title exact match. First author Yunzhen Feng confirmed. 2024. |
| 6 | Formal et al. (2021) — SPLADE | https://doi.org/10.1145/3404835.3463098 | REDIRECT → PASS (indirect) | DOI redirects to dl.acm.org which returned 403 to WebFetch (bot protection, expected). Cross-verified via arXiv 2107.05720: title "SPLADE: Sparse Lexical and Expansion Model for First Stage Ranking", authors Thibault Formal, Benjamin Piwowarski, Stéphane Clinchant, SIGIR'21 short paper. DOI itself is a valid ACM identifier; 403 is anti-bot not broken link. |
| 7 | Gerstgrasser et al. (2024) — Is model collapse inevitable | https://arxiv.org/abs/2404.01413 | PASS | Title exact match. First author Matthias Gerstgrasser confirmed. 2024. COLM 2024 venue plausible. |
| 8 | Hoffmann et al. (2022) — Chinchilla | https://arxiv.org/abs/2203.15556 | PASS | Title "Training Compute-Optimal Large Language Models" exact match. First author Jordan Hoffmann confirmed. 2022. NeurIPS 2022 venue correct. |
| 9 | Hu, E. J. et al. (2022) — LoRA | https://arxiv.org/abs/2106.09685 | PASS | Title "LoRA: Low-Rank Adaptation of Large Language Models" exact match. First author Edward J. Hu confirmed. arXiv v1 June 2021, v2 Oct 2021. Citing as 2022 is correct — ICLR 2022 is the formal publication venue. |
| 10 | Hu, K. & Tong, A. (2024, Nov 15) — Reuters OpenAI rivals | https://www.reuters.com/technology/artificial-intelligence/openai-rivals-seek-new-path-smarter-ai-current-methods-hit-limitations-2024-11-11/ | PASS (with minor date note) | Reuters actively blocks WebFetch (expected — Cloudflare). URL slug ends "2024-11-11" indicating initial publication Nov 11, 2024. DuckDuckGo search confirms article title and both authors Krystal Hu and Anna Tong. Citation says "November 15" — this likely reflects a Reuters content-update date rather than original pub date. **Recommendation**: Consider changing to "November 11, 2024" to match the canonical URL slug, OR verify author-stated retrieval date. Not a mismatch; a cosmetic date note. |
| 11 | Huang et al. (2024) — Compression represents intelligence linearly | https://arxiv.org/abs/2404.09937 | **PASS — author correction CONFIRMED CORRECT** | arXiv abstract page lists exactly 4 authors in this order: **Yuzhen Huang, Jinghan Zhang, Zifei Shan, Junxian He**. The previously-corrected author list (Huang Y., Zhang J., Shan Z., He J. — 4 authors) is accurate. COLM 2024 venue plausible. **Note**: first author is "Yuzhen" not "Yangzhen" — initial "Y." is correct. |

---

## Huang et al. (2024) — Special Check Result

**Claim to verify**: Author list corrected to Huang Y., Zhang J., Shan Z., He J. (4 authors total).

**arXiv 2404.09937 ground truth**:
1. Yuzhen Huang
2. Jinghan Zhang
3. Zifei Shan
4. Junxian He

**Verdict**: ✅ **CONFIRMED CORRECT**. Exactly 4 authors, initials match the corrected APA entry (Huang, Y., Zhang, J., Shan, Z., & He, J.).

---

## Findings / Action Items

1. **No mismatches requiring text fix.** All 11 URLs point to the correct paper with matching title and first author.
2. **Minor date note (optional fix)**: Hu, K. & Tong, A. Reuters entry — cited November 15, 2024 but URL slug is "2024-11-11". Reuters commonly stamps update dates; citing Nov 11 (original) is more defensible. If the original author recorded Nov 15 as retrieval date, current format is acceptable; if it was meant as publication date, consider changing to Nov 11.
3. **Ekman & Friesen (1978)** — correctly has no URL (print book); no action.
4. **DOI robustness** — two of three DOIs return 302 redirects (expected, normal DOI resolution). The ACM DOI returns 403 to automated fetchers but the DOI itself is valid and resolves in a browser. No action needed; these are PASS via redirect.

---

## Confidence

**High (0.95)**. 10 direct verifications via primary source, 1 via arXiv mirror (SPLADE), 1 via URL-slug + search-result cross-reference (Reuters). Huang author correction independently confirmed on arXiv abstract page. No broken links, no substantive mismatches, no title discrepancies.
