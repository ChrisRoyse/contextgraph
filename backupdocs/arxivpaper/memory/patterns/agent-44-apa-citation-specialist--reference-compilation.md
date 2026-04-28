---
namespace: patterns
agent_id: agent-44-apa-citation-specialist
timestamp: 2026-04-17T13:10:00Z
status: active
supersedes: none
tags: citations, apa-7th, harvest-map, reference-compilation
depends_on:
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/handoff/agent-43-to-44--reproducibility-ready.md
  - sections/00_frontmatter.md
  - sections/01_introduction.md
  - sections/02_related_work.md
  - sections/03_framework_dda_mc.md
  - sections/04_systems.md
  - sections/06_method_tct.md
  - sections/07_case_studies.md
  - sections/08_discussion.md
  - sections/09_limitations.md
  - sections/10_conclusion.md
downstream_consumers:
  - agent-45-citation-validator
  - agent-46-coherence-checker
  - agent-47-final-assembler
---

# Reference Compilation — Harvest Map

Exhaustive map of every unique in-text citation harvested from the
ten draft sections, cross-referenced against agent-08's locked
88-citation master pool. Each row states (a) the canonical APA
reference-list key, (b) the in-text form that appears, (c) the
section(s) that cite it, (d) master-pool eligibility.

## Counts

- **Unique in-text citations harvested**: 63 (from master pool) + 8
  flagged not-in-pool = 71 total unique.
- **APA entries in References section**: 63 (all from master pool,
  every one cited in body).
- **Flagged not-in-pool (needs agent-45 resolution)**: 8 (see
  companion file
  `discoveries/agent-44-apa-citation-specialist--missing-or-flagged-citations.md`).
- **Master-pool entries NOT cited in body** (prunable): 25 = 88 − 63.
  These are retained in the locked master pool but not emitted into
  the References section per APA 7th (only cited entries appear in
  the reference list).

## Section-by-section harvest

### §0 Frontmatter (00_frontmatter.md)

No in-text citations in the abstract body. (Abstract is descriptive;
citations live in §1 onward.)

### §1 Introduction (01_introduction.md)

| Key | In-text form | Line(s) |
|---|---|---|
| villalobos2024data | Villalobos et al. (2022/2024) | 3 |
| longpre2024consent | Longpre et al. (2024) | 3 |
| woo2024information | Efrati & Holmes, 2024 [= Woo & Efrati, 2024; dedup per agent-08 master pool] | 3 |
| metz2024bloomberg | Metz et al., 2024 | 3 |
| hu2024reuters | Hu & Tong, 2024 | 3 |
| shumailov2024nature | Shumailov et al. (2024) | 3, 5 |
| dohmatob2024tails | Dohmatob, Feng, Yang, Charton, & Kempe, 2024 | 3, 5 |
| gerstgrasser2024colm | Gerstgrasser et al., 2024 | 3, 5 |
| feng2024verification | Feng, Dohmatob, Yang, Charton, & Kempe, 2024 | 3, 5 |
| variety2024newscorp | Variety Staff, 2024 | 3 |
| reuters2024reddit | Reuters, 2024 | 3 |
| verma2023washpost | Verma & De Vynck, 2023 | 3 |
| field2024techcrunch | Field, 2024 | 3 |
| alemohammad2023mad | Alemohammad et al. (2023) | 5 |
| ouyang2022instructgpt | Ouyang et al., 2022 | 11 |
| rafailov2023dpo | Rafailov, Sharma, Mitchell, Ermon, Manning, & Finn, 2023 | 11 |
| bai2022constitutional | Bai et al., 2022 | 11 |

### §2 Related Work (02_related_work.md)

All citations from §1 re-appear in §2; additionally:

| Key | In-text form | Line(s) |
|---|---|---|
| kaplan2020scaling | Kaplan et al. (2020) | 7 |
| hoffmann2022chinchilla | Hoffmann et al. (2022) | 7 |
| franzen2024techcrunch | Franzen, 2024 | 7 |
| sutskever2024neurips | Sutskever, 2024 | 7 |
| seddik2024collapse | Seddik, Chen, Hayou, Youssef and Debbah (2024) | 13 |
| deletang2024compression | Delétang et al. (2024) | 17 |
| valmeekam2023llmzip | Valmeekam, Narayanan, Bharadia and Celik (2023) | 17 *[authors differ slightly from master pool — pool has Tuli & Yu instead of Bharadia & Celik; agent-45 please verify and correct body text to match canonical authors from arXiv:2306.04050]* |
| jiang2023gzip | Jiang et al. (2023) | 17 |
| li2025lossless | Li et al. (2025) | 17 |
| huang2024compression | Huang et al. (2024) | 17 |
| zhai2023siglip | Zhai, Mustafa, Kolesnikov, & Beyer, 2023 | 21 |
| nussbaum2024nomic | Nussbaum, Morris, Duderstadt, & Mulyar, 2024 | 21 |
| chen2022wavlm | Chen et al., 2022 | 21, 29, 31 |
| khattab2020colbert | Khattab & Zaharia, 2020 | 21 |
| santhanam2022colbertv2 | Santhanam, Khattab, Saad-Falcon, Potts, & Zaharia, 2022 | 21 |
| formal2021splade | Formal, Piwowarski, & Clinchant, 2021 | 21 |
| wang2021kepler | Wang et al., 2021 | 21 |
| bordes2013transe | Bordes, Usunier, Garcia-Durán, Weston, & Yakhnenko, 2013 | 21 |
| malkov2018hnsw | Malkov & Yashunin, 2018 | 21 |
| cormack2009rrf | Cormack, Clarke, & Büttcher, 2009 | 21 |
| wang2023valle | Wang et al., 2023 | 29 |
| chen2024valle2 | Chen et al., 2024 (VALL-E 2) | 29 |
| tan2024naturalspeech3 | Tan et al., 2024 | 29 |
| wang2024maskgct | Wang et al., 2024 (MaskGCT) | 29 |
| chen2024f5tts | Chen et al., 2024 (F5-TTS) | 29 |
| le2023voicebox | Le et al., 2023 | 29 |
| shen2023naturalspeech2 | Shen et al., 2023 | 29 |
| jiang2023megatts2 | Jiang et al., 2023 (Mega-TTS 2) | 29 |
| casanova2024xtts | Casanova, Davis, Gölge, et al., 2024 | 29 |
| reddy2022dnsmos | Reddy, Gopal, & Cutler, 2022 | 29 |
| saeki2022utmos | Saeki et al., 2022 | 29 |
| desplanques2020ecapa | Desplanques, Thienpondt, & Demuynck, 2020 | 29 |
| christop2025cloneval | Christop, Kuczyński, & Kubis, 2025 | 29 |
| meng2025echomimicv3 | Meng et al., 2025 | 31 |
| cui2024hallo3 | Cui et al., 2024 | 31 |
| li2024latentsync | Li et al., 2024 (LatentSync) | 31 |
| zheng2025hunyuanvideo | Zheng et al., 2025 | 31 |
| tian2024emo | Tian, Wang, Zhang, & Bo, 2024 | 31 |
| tian2025emo2 | Tian, Hu, Wang, Zhang, & Bo, 2025 | 31 |
| yang2024cogvideox | Yang et al., 2024 | 31 |
| wan2025video | Wan Team, 2025 | 31 |
| hu2022lora | Hu et al., 2022 (LoRA) | 31 |
| lipman2023flowmatching | Lipman, Chen, Ben-Hamu, Nickel, & Le, 2023 | 31 |
| li2017flame | Li, Bolkart, Black, Li, & Romero, 2017 | 31 |
| ekman1978facs | Ekman & Friesen, 1978 | 31 |

**Flagged not-in-pool (§2.3)**: Frantar, Ashkboos, Hoefler, and
Alistarh (2022) [GPTQ] and Frantar and Alistarh (2023) [SparseGPT]
appear on line 17; see discoveries file.

### §3 Framework DDA & MC (03_framework_dda_mc.md)

| Key | In-text form | Line(s) |
|---|---|---|
| deletang2024compression | Delétang et al., 2024 | 37, 50 |
| kaplan2020scaling | Kaplan et al., 2020 | 50 |
| hoffmann2022chinchilla | Hoffmann et al., 2022 | 50 |
| huang2024compression | Huang et al., 2024 | 50 |
| shumailov2024nature | Shumailov et al. (2024) | 54, 58 |
| dohmatob2024tails | Dohmatob et al. (2024) | 54, 58 |
| alemohammad2023mad | Alemohammad et al. (2023) | 54, 58 |
| gerstgrasser2024colm | Gerstgrasser et al., 2024 | 58 |
| feng2024verification | Feng et al., 2024 | 58 |
| khattab2020colbert | Khattab & Zaharia, 2020 | 64 |
| malkov2018hnsw | Malkov & Yashunin, 2018 | 64 |
| cormack2009rrf | Cormack et al., 2009 | 64 |

**Flagged not-in-pool (§3.2)**: Shannon (1948), Frantar et al. (2022),
Frantar & Alistarh (2023) on line 37; see discoveries file.

### §4 Systems (04_systems.md)

| Key | In-text form | Line(s) |
|---|---|---|
| anthropic2024mcp | Anthropic, 2024 | 9 |
| formal2021splade | Formal et al., 2021 | 11 |
| santhanam2022colbertv2 | Santhanam et al., 2022 | 11 |
| wang2021kepler | Wang et al., 2021 | 11 |
| cormack2009rrf | Cormack et al., 2009 | 15 |
| zhai2023siglip | Zhai et al., 2023 | 21 |
| chen2022wavlm | Chen et al., 2022 | 21 |
| desplanques2020ecapa | Desplanques et al., 2020 | 21 |

### §5 TCT Method (06_method_tct.md)

| Key | In-text form | Line(s) |
|---|---|---|
| rafailov2023dpo | Rafailov et al., 2023 | 39 |
| lipman2023flowmatching | Lipman et al., 2023 | 87 |
| chen2022wavlm | Chen et al., 2022 | 89 |
| desplanques2020ecapa | Desplanques et al., 2020 | 93 |

### §6 Case Studies (07_case_studies.md)

| Key | In-text form | Line(s) |
|---|---|---|
| meng2025echomimicv3 | Meng et al., 2025 | 37 |
| lipman2023flowmatching | Lipman et al., 2023 | 37 |
| reddy2022dnsmos | Reddy et al., 2022 | 75 |
| saeki2022utmos | Saeki et al., 2022 | 76 |
| desplanques2020ecapa | Desplanques et al., 2020 | 121 |

**Flagged not-in-pool (§6.2.4)**: Chung & Zisserman (2017) [SyncNet]
on line 57; see discoveries file.

### §7 Discussion (08_discussion.md)

| Key | In-text form | Line(s) |
|---|---|---|
| ouyang2022instructgpt | Ouyang et al., 2022 | 21 |
| rafailov2023dpo | Rafailov et al., 2023 | 21 |
| bai2022constitutional | Bai et al., 2022 | 21 |
| shumailov2024nature | Shumailov et al. (2024) | 27 |
| dohmatob2024tails | Dohmatob et al. (2024) | 27 |
| alemohammad2023mad | Alemohammad et al. (2023) | 27 |
| villalobos2024data | Villalobos et al., 2024 | 39 |
| deletang2024compression | Delétang et al., 2024 | 39 |
| huang2024compression | Huang et al., 2024 | 39 |

**Flagged not-in-pool (§7.3, §7.6)**: Kinnunen & Li (2010), Schroff
et al. (2015), Bromley et al. (1993), Epoch AI (2024); see
discoveries file.

### §8 Limitations (09_limitations.md)

No new in-text citations; only self-references to prior sections
(§6.3, §8.L1, etc.) and internal construct labels (EXP-1, EXP-2,
EXP-3, L1–L8).

### §9 Conclusion (10_conclusion.md)

No new in-text citations; only self-references.

## APA 7th formatting rules applied

1. **Up to 20 authors listed**; 21+ authors trigger the "… & [final]"
   ellipsis pattern. Applied to longpre2024consent, ouyang2022instructgpt,
   hoffmann2022chinchilla, bai2022constitutional.
2. **Journal titles and book titles italicised** via Markdown `*...*`.
3. **Volume numbers italicised** (`*42*`, `*9*`, etc.).
4. **DOI preferred** over URL where both exist; written as bare
   `https://doi.org/...` with no "DOI:" label (APA 7th rule).
5. **arXiv preprints** formatted as `arXiv. https://arxiv.org/abs/...`
   without a journal-style italicisation.
6. **Group / corporate authors** (Anthropic, Reuters, Variety Staff,
   Wan Team) written out as author surnames without "the" prefix.
7. **No "Retrieved from"** language (APA 7th dropped this for URLs
   with stable content).
8. **Hanging indent** is a typographic concern for the rendered PDF;
   in Markdown the entries are written as single paragraphs with
   blank lines between — LaTeX converter will apply hanging indent.
9. **Alphabetical by first author's surname**, with single-initial
   disambiguation only where surnames collide (e.g., three Chen
   entries ordered by given-initial letter: S., S., Y.).
10. **Dates**: year-only for scholarly works; year-month-day for
    news / press releases (Franzen, 2024, November 20).

## Narrative vs. parenthetical in-text style

Body text uses both forms:

- Narrative: `Smith (2020) argued that ...`
- Parenthetical: `... (Smith, 2020).`
- Multi-citation parenthetical (alphabetical within-parens):
  `(Jones, 2019; Smith, 2020; Williams, 2021)`.

Consistent with the 10 sections; no mixing errors observed.

## Changelog

- 2026-04-17 — Initial harvest + compilation by
  agent-44-apa-citation-specialist.
