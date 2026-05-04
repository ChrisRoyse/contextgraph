# Reference Link Audit — ME-JEPA Paper

**Date:** 2026-05-01
**Auditor:** Claude (playwright + curl + WebFetch)
**Scope:** Every URL in `references.bib` verified for (a) HTTP 200 reachability, (b) page-title match against the claimed paper in the bib entry.

**Result: 34 / 34 URLs verified. Zero broken links. Zero citation mismatches.**

---

## Method

1. Extracted 34 unique URLs from `references.bib`.
2. Curl HEAD sweep — all 34 returned HTTP 200.
3. Playwright navigation to each URL, capturing page title.
4. Compared page title against the title field in the corresponding bib entry.
5. PMC URL (Efron & Narasimhan) was captcha-gated for headless playwright; content verified via WebFetch.

---

## Arxiv URLs (19 — all verified)

| Bib key | Arxiv ID | Page title returned | Match |
|---|---|---|---|
| `assran2023ijepa` | 2301.08243 | Self-Supervised Learning from Images with a Joint-Embedding Predictive Architecture | ✓ |
| `bardes2024vjepa` | 2404.08471 | Revisiting Feature Prediction for Learning Visual Representations from Video | ✓ |
| `maes2026leworldmodel` *(new)* | 2603.19312 | LeWorldModel: Stable End-to-End Joint-Embedding Predictive Architecture from Pixels | ✓ |
| `chen2026vljepa` *(new)* | 2512.10942 | VL-JEPA: Joint Embedding Predictive Architecture for Vision-language | ✓ |
| `bardes2022vicreg` | 2105.04906 | VICReg: Variance-Invariance-Covariance Regularization for Self-Supervised Learning | ✓ |
| `grill2020byol` | 2006.07733 | Bootstrap your own latent: A new approach to self-supervised Learning | ✓ |
| `vandenoord2019cpc` | 1807.03748 | Representation Learning with Contrastive Predictive Coding | ✓ |
| `ha2018worldmodels` | 1803.10122 | World Models | ✓ |
| `hafner2023dreamerv3` | 2301.04104 | Mastering Diverse Domains through World Models | ✓ |
| `schrittwieser2019muzero` | 1911.08265 | Mastering Atari, Go, Chess and Shogi by Planning with a Learned Model | ✓ |
| `reed2022gato` | 2205.06175 | A Generalist Agent | ✓ |
| `jaegle2022perceiverio` | 2107.14795 | Perceiver IO: A General Architecture for Structured Inputs & Outputs | ✓ |
| `senger2024karrierewege` | 2412.14612 | KARRIEREWEGE: A Large Scale Career Path Prediction Dataset | ✓ |
| `johary2025jobhop` | 2505.07653 | JobHop: A Large-Scale Dataset of Career Trajectories | ✓ |
| `pineau2020reproducibility` | 2003.12206 | Improving Reproducibility in Machine Learning Research (NeurIPS 2019 Reproducibility Program Report) | ✓ |
| `shumailov2023curse` | 2305.17493 | The Curse of Recursion: Training on Generated Data Makes Models Forget | ✓ |
| `gerstgrasser2024accumulate` | 2404.01413 | Is Model Collapse Inevitable? Breaking the Curse of Recursion by Accumulating Real and Synthetic Data | ✓ |
| `lightman2023verify` | 2305.20050 | Let's Verify Step by Step | ✓ |
| `jung2025codeexec` | 2506.10343 | Code Execution as Grounded Supervision for LLM Reasoning | ✓ |

## Non-arxiv URLs (15 — all verified)

| Bib key | URL | Page title returned | Match |
|---|---|---|---|
| `metafair2024vjepa` | github.com/facebookresearch/jepa | facebookresearch/jepa: PyTorch code and models for V-JEPA self-supervised learning from video | ✓ |
| `onet2026` | onetcenter.org/database.html | O*NET Database at O*NET Resource Center | ✓ |
| `onet2026` (license) | onetcenter.org/license_db.html | O*NET® 30.2 Database Content License at O*NET Resource Center | ✓ |
| `onet2026` (ZIP) | onetcenter.org/dl_files/database/db_30_2_text.zip | (binary download, HTTP 200, no page title) | ✓ |
| `esco2025` | esco.ec.europa.eu/en/use-esco/download | Download \| European Skills, Competences, Qualifications and Occupations (ESCO) | ✓ |
| `mlflow2024` | mlflow.org/docs/latest/ml/tracking/ | ML Experiment Tracking \| MLflow AI Platform | ✓ |
| `pytorch2026reproducibility` | docs.pytorch.org/docs/2.11/notes/randomness.html | Reproducibility — PyTorch 2.11 documentation | ✓ |
| `docker2025reproducible` | docs.docker.com/build/ci/github-actions/reproducible-builds/ | Reproducible builds with GitHub Actions \| Docker Docs | ✓ |
| `docker2024bestpractices` | docs.docker.com/build/building/best-practices/ | Building best practices \| Docker Docs | ✓ |
| `apptainer2025buildsif` | apptainer.org/docs/user/main/build_a_container.html | Build a Container — Apptainer User Guide | ✓ |
| `apptainer2025buildsif` (appendix) | apptainer.org/docs/user/main/appendix.html | Appendix — Apptainer User Guide | ✓ |
| `efron2020automatic` | pmc.ncbi.nlm.nih.gov/articles/PMC7958418/ | The automatic construction of bootstrap confidence intervals (Efron & Narasimhan, J. Comp. Graph. Stat. 2020) — verified via WebFetch (page captcha-gated for headless playwright) | ✓ |
| `shumailov2024nature` | nature.com/articles/s41586-024-07566-y | AI models collapse when trained on recursively generated data \| Nature | ✓ |
| `primeintellect2025synthetic1` | primeintellect.ai/blog/synthetic-1 | SYNTHETIC-1: Scaling Distributed Synthetic Data Generation for Verified Reasoning | ✓ |
| `seed2025enigmata` | seed-enigmata.github.io | Enigmata: Scaling Logical Reasoning in Large Language Models with Synthetic Verifiable Puzzles | ✓ |

---

## Notes

- **PMC URL** (Efron & Narasimhan): pmc.ncbi.nlm.nih.gov serves a Cloudflare-style "Checking your browser" reCAPTCHA when accessed by headless Playwright. The underlying article was verified by WebFetch which uses a different request pipeline. Real human users with normal browsers will reach the article without issue. Optional improvement: add a DOI fallback in the bib entry (DOI 10.1080/10618600.2020.1714633) so reviewers have a non-PMC route.
- **Console-error pages** (mlflow, esco, apptainer, primeintellect, enigmata): pages render correctly despite client-side console errors (typical for modern JS-heavy sites). Title and content load.
- **arxiv 2603.19312 / 2512.10942**: the two citations I added in this edit pass — both confirmed against arxiv abstract pages with exact-match titles. LeCun is listed as co-author on both as expected for the JEPA-family citation context.

## Verdict

The reference list is **safe to send**. Every URL takes a reader to the cited paper. No URL maintenance needed before the LeCun outreach.
