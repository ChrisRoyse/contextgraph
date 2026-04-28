---
namespace: discoveries
agent_id: agent-08-citation-extractor
timestamp: 2026-04-16T17:52:00Z
status: active
tags: unresolved, flagged, agent-45-attention, unverified, tier-deferred
depends_on:
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/decisions/agent-07-source-tier-classifier--tier-assignments.md
---

# Unresolved / Flagged Citations (for agent-45 citation-validator)

Every citation listed here requires attention at validation (agent-45) before the
camera-ready. Categories:

1. **UNVERIFIED** — not in the key index. Must be re-verified or excluded.
2. **TIER-DEFERRED** — in the key index at a conservative tier; candidate for promotion if downstream agent confirms the venue.
3. **METADATA-WATCH** — in the key index with complete metadata, but a specific field (author order, page range, DOI suffix) should be re-checked at camera-ready because the evidence brief was itself sparse.
4. **URL-STABILITY-RISK** — in the key index with a URL that may rot (trade-press article paths have changed once already; GitHub repos can rename).

---

## Category 1 — UNVERIFIED (NOT in master pool)

### U01 — Qwen3-TTS / Qwen3-Voice-Embedding

- **Raised by.** Brief 03 "Unverified / Excluded" + agent-07 flag.
- **Problem.** No resolvable arXiv identifier at verification time.
- **Mission-brief relevance.** **HIGH.** Qwen3-TTS is the **base model for Case 3** (mission-brief core claim 3). The manuscript MUST disclose this in §6.3 Methods.
- **Resolution path.** Cite as T3 via the Qwen GitHub org (https://github.com/QwenLM). **Proposed key: `qwen2024qwen3tts`**. Not yet entered in the master pool.
- **Action for agent-45.** Before camera-ready, re-check https://github.com/QwenLM for a pinned Qwen3-TTS repo URL. If still no arXiv exists, promote this to an entered T3 key and update the master pool's changelog.
- **Action for agent-11 (systematic reviewer).** If an arXiv technical report appears between now and camera-ready, re-verify and promote to T2.
- **Status.** **BLOCKS §6.3 Methods** until the citation is entered. Writing agent 37 / 38 must not silently leave Qwen3 uncited.

### U02 — Demucs v1/v2/v3 (Défossez 2019/2021)

- **Raised by.** Brief 03 "Unverified / Excluded".
- **Problem.** arXiv:1911.13254 exists but was not re-verified in briefs.
- **Mission-brief relevance.** **LOW.** v4 / HTDemucs is already cited as [G05] rouard2023htdemucs. v1–v3 are predecessor references only.
- **Resolution path.** Optional. If needed, re-verify arXiv:1911.13254 and enter as T2.
- **Action for agent-45.** No action required unless a writer adds a v1–v3 in-text citation.
- **Status.** Low priority; does NOT block anything.

### U03 — "Same-session verification band ≥ 0.95"

- **Raised by.** Brief 02 §8 + agent-07.
- **Problem.** **NOT a citation**; it is the authors' own empirical observation. The evidence brief explicitly framed this as "not an external benchmark."
- **Mission-brief relevance.** Feeds Q08 threshold calibration. The manuscript frames this as V12-qualified author observation, not a cited claim.
- **Resolution path.** **Do NOT add as a citation.** Writing agents must mark this in-text as "authors' own empirical observation on Case 3's speaker; not a published threshold."
- **Action for agent-45.** Verify §5.3 and §6.3 carry the V12 hedge. No citation required.
- **Status.** Resolved by design. Flagged here for transparency.

### U04 — Google "TurboQuant" (activation-compression specifics)

- **Raised by.** Prior manuscript §3.6 / `_evidence_01_data_wall.md` adjacency note.
- **Problem.** Descriptively referenced; no verifiable primary source for the specific 6× KV-cache compression claim.
- **Mission-brief relevance.** LOW. The four-category compression taxonomy in §3.1 does NOT depend on TurboQuant specifics; the paper's taxonomic argument stands even if TurboQuant is omitted.
- **Resolution path.** **Omit the "6× KV-cache" specific claim** unless a citable source is located. Reference activation-compression generically (KV-cache quantisation is the broader category) without the TurboQuant-specific number.
- **Action for agent-45.** If §3.5 cites "6× KV-cache", require a source; otherwise require that the number be removed.
- **Status.** Mitigated; the manuscript does not depend on this number.

### U05 — Bank / insurer custom-model pricing figures

- **Raised by.** Brief 04 omitted note.
- **Problem.** No citable primary source (peer-reviewed or top-tier analyst note) with concrete ranges.
- **Mission-brief relevance.** LOW. Leapable acknowledgement only; no pricing claim in body.
- **Resolution path.** Omit the figures from body. If any pricing claim leaks into §1 or §8, require a source.
- **Action for agent-45.** Grep the final PAPER.md for dollar figures / pricing ranges against H-cluster sources; reject any unsourced number.
- **Status.** Policy in place.

---

## Category 2 — TIER-DEFERRED (promotion candidates)

The following citations sit at T2 in the master pool because the evidence brief cited only an arXiv preprint, but the underlying paper was subsequently peer-reviewed at a T1 venue. Agent-07 kept them at T2 per the handoff step 8 rule (no silent tier changes). Agent-08 recommends **downstream verification** at the agent-45 pass.

| Key | Current tier | Venue evidence (if confirmed) | Recommendation |
|---|---|---|---|
| zhai2023siglip [D01] | T2 | ICCV 2023 | Promote T2 → T1 if ICCV 2023 publication re-confirmed |
| le2023voicebox [E06] | T2 | NeurIPS 2023 | Promote T2 → T1 if NeurIPS 2023 publication re-confirmed |
| shen2023naturalspeech2 [E07] | T2 | ICLR 2024 | Promote T2 → T1 if ICLR 2024 publication re-confirmed |
| tian2024emo [F05] | T2 | ECCV 2024 | Promote T2 → T1 if ECCV 2024 publication re-confirmed |

Promotion would move the Core (A–G + I) T1+T2 percentage from 85% to ~90%; it does not
change the pass/fail of QG1 G8 but strengthens the paper's tier-density margin.

**Action for agent-45.** At camera-ready, re-verify the venue pages (OpenReview / ACM DL
/ conference proceedings) for these four entries. If confirmed, issue a tier-change
blocker + update master-pool changelog.

---

## Category 3 — METADATA-WATCH

### M01 — longpre2024consent [A04] author list

- **Issue.** The arXiv abstract lists 20+ authors; agent-08 listed the first three plus common co-authors. APA 7th allows up to 20 authors, then `et al.`
- **Action for agent-45.** Compare the master-pool author list against the arXiv v2 author metadata; if >20 authors, insert `et al.` after the 19th author. Current entry is conservative.

### M02 — singhal2023nature [H13] and tu2025nature [H15] author lists

- **Issue.** Both are large Google / DeepMind consortia with 25+ authors. Our entries list up to 20 then cut off. Check the "..." marker ordering.
- **Action for agent-45.** Confirm the authors listed are the first 19 + the final senior author per APA 7th `…` convention.

### M03 — wu2023bloomberggpt [H20] preprint revision

- **Issue.** arXiv v3 updated in 2023; some co-authors order changes between versions.
- **Action for agent-45.** Confirm author order matches arXiv v3 (most recent).

### M04 — bai2022constitutional [I03] tier is a close call

- **Issue.** Anthropic technical report, arXiv-only. Agent-08 tiered T2 (T1-publishing group; high citation count). A strict reading could demote to T3 (treated as company tech report).
- **Action for agent-45.** If body T1+T2 density becomes tight, re-consider demotion. If kept at T2, no action. **Default: T2.**

### M05 — ekman1978facs [F12] bibliographic metadata

- **Issue.** Print reference, 1st edition; no ISBN captured in master pool.
- **Action for agent-45.** Add ISBN 0-931835-01-1 or 978-0-931835-01-4 at camera-ready if available. Not required for arXiv compliance.

### M06 — chen2022wavlm [D03] DOI

- **Issue.** Master pool entry now includes DOI 10.1109/JSTSP.2022.3188113 (cross-walked from IEEE). Brief 02 §6 cited only arXiv. Confirm the DOI resolves.
- **Action for agent-45.** Run a DOI resolution check on 10.1109/JSTSP.2022.3188113; confirm landing page.

---

## Category 4 — URL-STABILITY-RISK

Trade-press and company-blog URLs are prone to path changes. Pre-camera-ready, all of
these should be re-fetched.

| Key | URL | Risk level |
|---|---|---|
| woo2024information [A05] | https://www.theinformation.com/articles/openai-shifts-strategy-as-rate-of-gpt-ai-improvements-slows | MEDIUM (paywalled) |
| metz2024bloomberg [A06] | https://www.bloomberg.com/news/articles/2024-11-13/... | MEDIUM (paywalled) |
| hu2024reuters [A07] | https://www.reuters.com/technology/artificial-intelligence/... | LOW |
| franzen2024techcrunch [A08] | https://techcrunch.com/2024/11/20/ai-scaling-laws-are-showing-diminishing-returns-... | LOW (brief 01 vs brief 04 URL drift noted; canonical taken from brief 04) |
| field2024techcrunch [H02] | https://techcrunch.com/2024/05/16/openai-inks-deal-to-train-ai-on-reddit-data/ | LOW |
| anthropic2024mcp [H01] | https://www.anthropic.com/news/model-context-protocol | LOW |
| mcp2024spec [G09] | https://modelcontextprotocol.io | LOW |
| google2020mediapipe [G06] | https://github.com/google/mediapipe | LOW (well-maintained) |
| tinachen2021yolov5face [G07] | https://github.com/deepcam-cn/yolov5-face | MEDIUM (third-party repo) |
| resembleai2024enhance [G08] | https://github.com/resemble-ai/resemble-enhance | LOW |
| verma2023washpost [H19] | https://www.washingtonpost.com/technology/2023/07/13/openai-chatgpt-pay-ap-news-ai/ | MEDIUM (paywalled) |
| variety2024newscorp [H18] | https://variety.com/2024/digital/news/news-corp-openai-licensing-deal-1236013734/ | LOW |
| shutterstock2023openai [H12] | https://investor.shutterstock.com/news-releases/news-release-details/... | MEDIUM (investor-relations rewrites) |
| gartner2023trends [H03] | https://www.gartner.com/en/newsroom/press-releases/2023-08-01-gartner-identifies-top-trends-shaping-future-of-data-science-and-machine-learning | LOW |
| openai2024reddit [H08] | https://openai.com/index/openai-and-reddit-partnership/ | LOW |
| openai2024newscorp [H09] | https://openai.com/index/news-corp-and-openai-sign-landmark-multi-year-global-partnership/ | LOW |
| reuters2024reddit [H10] | https://www.reuters.com/technology/reddit-ai-content-licensing-deal-with-google-sources-say-2024-02-22/ | LOW |
| cerullo2024cbs [H11] | https://www.cbsnews.com/news/google-reddit-60-million-deal-ai-training/ | LOW |
| harvard2024caselaw [H04] | https://case.law/ | LOW (institutional) |
| nci2024thesaurus [H06] | https://ncithesaurus.nci.nih.gov/ | LOW (government) |
| nlm2024mesh [H07] | https://www.nlm.nih.gov/mesh/meshhome.html | LOW (government) |
| snomed2024ct [H14] | https://www.snomed.org/ | LOW (standards body) |
| sec2024edgar [H16] | https://www.sec.gov/submit-filings/about-edgar | LOW (government) |
| sec2024edgarsearch [H17] | https://www.sec.gov/edgar/search/ | LOW (government) |

**Action for agent-45.** Run a batch HTTP HEAD/GET on every URL at T-minus-2-days from
camera-ready. For any 404 / 410, search the Internet Archive for a snapshot and note
it in the final PAPER.md. The G7 gate tolerates ≤ 5% transient breakage.

---

## Summary

- **Total flagged:** 5 UNVERIFIED + 4 TIER-DEFERRED + 6 METADATA-WATCH + 24 URL-STABILITY-RISK = 39 items.
- **Blocks production:** 1 (U01 Qwen3-TTS — blocks §6.3 Methods; proposed T3 key `qwen2024qwen3tts` pending master-pool entry).
- **Mitigated by policy:** U03, U04, U05.
- **Promotion candidates:** 4 (zhai2023siglip, le2023voicebox, shen2023naturalspeech2, tian2024emo) — all would move T2→T1, tightening the body tier-density margin.

## Escalation

Any item NOT resolved by agent-45 at camera-ready escalates to the coordinator for a
decision (cite-and-flag, omit, or block submission). G7 (citation validity) is the
binding gate; G8 (empirical tier labelling) is a fixable-in-annotation-pass gate per
the master gate precedence in agent-01 / agent-05.

## Changelog

- 2026-04-16 — Initial issue by agent-08-citation-extractor.
