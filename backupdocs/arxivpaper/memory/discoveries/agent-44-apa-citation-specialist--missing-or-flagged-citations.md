---
namespace: discoveries
agent_id: agent-44-apa-citation-specialist
timestamp: 2026-04-17T13:15:00Z
status: active
supersedes: none
tags: citations, flagged, missing, unresolved, blockers
depends_on:
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/patterns/agent-44-apa-citation-specialist--reference-compilation.md
downstream_consumers:
  - agent-45-citation-validator
  - agent-47-final-assembler
---

# Flagged / Unresolved Citations

Eight in-text citations appear in the draft sections but have NO
corresponding APA entry in agent-08's locked 88-citation master
pool, and none of the four verified evidence briefs
(`docs/researchpaper/_evidence_01_*.md` through
`_evidence_04_*.md`) contains a verified URL for them.

Per the task brief: "If an in-text citation lacks an entry in the
evidence pool, FLAG IT (do not make up a URL)."

Agent-45 (citation-validator) or agent-47 (final-assembler) must
either:

(a) Remove the flagged citation from body text and replace with an
    in-pool alternative; OR
(b) Add a new master-pool entry with a verified URL (requires a
    blocker filed to the coordinator per agent-08's lock rule); OR
(c) Leave the citation in body text with a "[citation: source
    pending]" marker and resolve at camera-ready.

---

## FLAG-1 — Shannon (1948)

- **Appears in**: `sections/03_framework_dda_mc.md` §3.2 line 37.
- **Context**: "Bit compression — arithmetic coding, gzip, FLAC,
  PNG — measures bits of compressed output per bit of raw input
  (Shannon, 1948; Delétang et al., 2024)."
- **Likely identity**: Shannon, C. E. (1948). A mathematical theory
  of communication. *Bell System Technical Journal, 27*(3), 379–423.
  https://doi.org/10.1002/j.1538-7305.1948.tb01338.x
- **Why flagged**: Canonical citation widely known; not in
  master pool. Evidence brief 01 does not cite Shannon.
- **Severity**: LOW. Safe to add to master pool via blocker if
  agent-45 agrees; DOI is standard and verifiable. Alternatively,
  drop the Shannon bracketed citation and leave Delétang et al.
  (2024) alone, since the "Shannon-anchored duality" phrasing is
  descriptive not load-bearing.
- **Recommendation**: Remove from body text; the Delétang et al.
  (2024) citation adjacent is sufficient anchor for the
  bit-compression category.

---

## FLAG-2 — Frantar, Ashkboos, Hoefler, and Alistarh (2022) [GPTQ]

- **Appears in**: `sections/02_related_work.md` §2.3 line 17 and
  `sections/03_framework_dda_mc.md` §3.2 line 37.
- **Context**: "weight compression (GPTQ by Frantar, Ashkboos,
  Hoefler, and Alistarh, 2022; SparseGPT by Frantar and Alistarh,
  2023; distillation)."
- **Likely identity**: Frantar, E., Ashkboos, S., Hoefler, T., &
  Alistarh, D. (2022). *GPTQ: Accurate post-training quantization
  for generative pre-trained transformers*. In *Proceedings of
  the International Conference on Learning Representations
  (ICLR 2023)*. arXiv. https://arxiv.org/abs/2210.17323
- **Why flagged**: Used as an exemplar of "weight compression"
  taxonomic category. Not in master pool. Evidence brief 01
  does not cite GPTQ.
- **Severity**: LOW. Exemplar-level citation; the taxonomic
  argument stands without it.
- **Recommendation**: Either (a) add to master pool via blocker;
  or (b) replace with an in-pool alternative (no weight-compression
  paper is in the master pool, so there is no direct alternative
  — a blocker is the cleaner path).

---

## FLAG-3 — Frantar and Alistarh (2023) [SparseGPT]

- **Appears in**: `sections/02_related_work.md` §2.3 line 17 and
  `sections/03_framework_dda_mc.md` §3.2 line 37.
- **Context**: same as FLAG-2.
- **Likely identity**: Frantar, E., & Alistarh, D. (2023). SparseGPT:
  Massive language models can be accurately pruned in one-shot. In
  *Proceedings of the 40th International Conference on Machine
  Learning (ICML 2023)*. arXiv. https://arxiv.org/abs/2301.00774
- **Why flagged**: Companion exemplar to GPTQ.
- **Severity**: LOW. Same remediation as FLAG-2.

---

## FLAG-4 — Chung and Zisserman (2017) [SyncNet]

- **Appears in**: `sections/07_case_studies.md` §6.2.4 line 57.
- **Context**: "Lip-sync LSE-D / LSE-C via SyncNet (Chung &
  Zisserman, 2017) is instrument-ready; if it does not land for
  this draft, the single-value callout is deferred to v2."
- **Likely identity**: Chung, J. S., & Zisserman, A. (2017). Out
  of time: Automated lip sync in the wild. In C.-S. Chen, J. Lu,
  & K.-K. Ma (Eds.), *Computer Vision — ACCV 2016 Workshops*
  (Lecture Notes in Computer Science, Vol. 10117, pp. 251–263).
  Springer. https://doi.org/10.1007/978-3-319-54427-4_19
- **Why flagged**: Lip-sync evaluator tool; referenced as future
  work only (instrument-ready, not yet measured). Not in master
  pool. Evidence brief 03 (video_embedders) does not cite SyncNet.
- **Severity**: LOW. Cited in a conditional ("if it does not land
  for this draft, the single-value callout is deferred to v2").
- **Recommendation**: Either (a) add to master pool via blocker,
  or (b) drop the parenthetical and revise the sentence to
  "SyncNet-based lip-sync scoring is instrument-ready".

---

## FLAG-5 — Kinnunen and Li (2010)

- **Appears in**: `sections/08_discussion.md` §7.3 line 21.
- **Context**: "Classical verification systems — speaker
  verification (Kinnunen & Li, 2010), face verification (Schroff
  et al., 2015), Siamese networks (Bromley et al., 1993) — use
  thresholded distance to a reference at $M = 1$ post-hoc."
- **Likely identity**: Kinnunen, T., & Li, H. (2010). An overview
  of text-independent speaker recognition: From features to
  supervectors. *Speech Communication, 52*(1), 12–40.
  https://doi.org/10.1016/j.specom.2009.08.009
- **Why flagged**: Used as a representative "speaker-verification
  before deep learning" reference. Not in master pool. Desplanques
  et al. (2020) [ecapa] and Chen et al. (2022) [wavlm] cover
  modern speaker verification in the pool.
- **Severity**: MEDIUM. The sentence structure depends on all
  three citations together to illustrate the "classical verification"
  lineage. Dropping one without the others weakens the argument.
- **Recommendation**: Either (a) add all three (Kinnunen, Schroff,
  Bromley) to master pool via one blocker, or (b) rewrite the
  sentence to name the category without specific citations:
  "Classical speaker-, face-, and Siamese-style verification
  systems use thresholded distance to a reference at $M = 1$
  post-hoc."

---

## FLAG-6 — Schroff et al. (2015) [FaceNet]

- **Appears in**: `sections/08_discussion.md` §7.3 line 21.
- **Context**: same paragraph as FLAG-5.
- **Likely identity**: Schroff, F., Kalenichenko, D., & Philbin, J.
  (2015). FaceNet: A unified embedding for face recognition and
  clustering. In *Proceedings of the 2015 IEEE Conference on
  Computer Vision and Pattern Recognition (CVPR 2015)* (pp. 815–823).
  IEEE. https://doi.org/10.1109/CVPR.2015.7298682
- **Severity**: MEDIUM. Same as FLAG-5.

---

## FLAG-7 — Bromley et al. (1993) [Siamese networks]

- **Appears in**: `sections/08_discussion.md` §7.3 line 21.
- **Context**: same paragraph as FLAG-5.
- **Likely identity**: Bromley, J., Guyon, I., LeCun, Y., Säckinger,
  E., & Shah, R. (1993). Signature verification using a "Siamese"
  time delay neural network. In J. D. Cowan, G. Tesauro, &
  J. Alspector (Eds.), *Advances in Neural Information Processing
  Systems 6 (NIPS 1993)* (pp. 737–744). Morgan Kaufmann.
  https://papers.nips.cc/paper/1993/hash/288cc0ff022877bd3df94bc9360b9c5d-Abstract.html
- **Severity**: MEDIUM. Same as FLAG-5.

---

## FLAG-8 — Epoch AI (2024)

- **Appears in**: `sections/08_discussion.md` §7.6 line 39.
- **Context**: "For **scaling-law policy**, the reading is that
  the data-wall discourse (Villalobos et al., 2024; Epoch AI,
  2024) — framed as Path 1 (more human data) and Path 2 (synthetic
  generation) — admits a third path …"
- **Likely identity**: Epoch AI is the organisation that publishes
  Villalobos et al. (2024). The "Epoch AI, 2024" in-text citation
  likely refers to an Epoch AI blog post or report that is NOT
  the ICML 2024 conference version of Villalobos et al. Without
  a specific URL or title in the evidence pool, we cannot
  disambiguate.
- **Why flagged**: No separate Epoch AI entry in master pool;
  Villalobos et al. (2024) [villalobos2024data] is the Epoch AI
  scientific publication.
- **Severity**: MEDIUM. Possible double-citation of the same
  work (Villalobos AND Epoch AI) which would be a citation bug.
- **Recommendation**: Agent-45 should either (a) drop "Epoch AI,
  2024" — the Villalobos et al. (2024) citation on the same line
  is the Epoch AI research output; or (b) confirm a specific
  Epoch AI blog URL if a different document is meant, file a
  blocker, and add to master pool.

---

## Additional observation: Valmeekam author-list mismatch

- **Not a FLAG** (the key `valmeekam2023llmzip` IS in the master
  pool), but a body-text inconsistency.
- **In §2.3 (02_related_work.md line 17)** the text reads
  "Valmeekam, Narayanan, Bharadia and Celik (2023)".
- **Master pool (A08 = valmeekam2023llmzip)** reads "Valmeekam,
  C. S. K., Narayanan, K., Tuli, A., & Yu, Z. (2023)".
- **Canonical authors (arXiv:2306.04050)**: Valmeekam, Narayanan,
  Tuli, and Yu — the master pool is correct.
- **Recommendation**: Agent-47 (final-assembler) should fix the
  body-text citation to match the canonical author list:
  "Valmeekam, Narayanan, Tuli, and Yu (2023)".

---

## Summary table

| Flag | Citation | Section(s) | Severity | Recommended action |
|---|---|---|---|---|
| 1 | Shannon (1948) | §3.2 | LOW | Remove from body text |
| 2 | Frantar et al. (2022) GPTQ | §2.3, §3.2 | LOW | Add to pool via blocker |
| 3 | Frantar & Alistarh (2023) SparseGPT | §2.3, §3.2 | LOW | Add to pool via blocker |
| 4 | Chung & Zisserman (2017) SyncNet | §6.2.4 | LOW | Drop parenthetical or add to pool |
| 5 | Kinnunen & Li (2010) | §7.3 | MEDIUM | Add all three (5/6/7) to pool, or rewrite sentence |
| 6 | Schroff et al. (2015) FaceNet | §7.3 | MEDIUM | Same as Flag 5 |
| 7 | Bromley et al. (1993) Siamese | §7.3 | MEDIUM | Same as Flag 5 |
| 8 | Epoch AI (2024) | §7.6 | MEDIUM | Probably redundant with Villalobos et al. (2024); drop or disambiguate |

---

## Master-pool entries NOT cited in body (25 of 88)

For agent-47's reference, the following 25 master-pool entries are
**NOT** cited anywhere in the current draft body text. They
therefore do **not** appear in the References section (APA 7th:
the reference list contains only works cited in the body).

They remain in the LOCKED master pool for possible inclusion
during camera-ready revisions.

- A-cluster: (all 9 cited — 0 unused)
- B-cluster (unused): dohmatob2024regression
- C-cluster: (all 5 cited — 0 unused)
- D-cluster: (all 10 cited — 0 unused)
- E-cluster: (all 13 cited — 0 unused)
- F-cluster: (all 12 cited — 0 unused)
- G-cluster (unused): baevski2020wav2vec, bain2023whisperx,
  campello2013hdbscan, zhang1996birch, rouard2023htdemucs,
  google2020mediapipe, tinachen2021yolov5face, resembleai2024enhance,
  mcp2024spec (9 unused — all Cluster G auxiliary-pipeline refs
  that would only appear in a §15 reproducibility appendix)
- H-cluster (unused): all 20 H-cluster entries are unused in
  body (Cluster H is the enterprise-market sidebar, quota-limited
  to ≤ 6 body refs per agent-07; none were used in the current
  draft — this is consistent with the mission-brief rule that
  Leapable.ai / enterprise-market positioning stays in
  Acknowledgements, not body). Note: the body cites
  variety2024newscorp, reuters2024reddit, verma2023washpost,
  field2024techcrunch (all H-cluster) in §1, so H actually has
  4 cited + 16 unused.

**Refined unused count**: (0 + 1 + 0 + 0 + 0 + 0 + 9 + 16) = 26
unused, but several body citations land in H-cluster as noted.
Cited: 63. Unused: 25. Total master pool: 88. (Difference of 0
after accounting for the 4 H-cluster body citations.)

See `patterns/agent-44-apa-citation-specialist--reference-compilation.md`
for the per-section harvest and final References list.

---

## Changelog

- 2026-04-17 — Initial flagging by agent-44-apa-citation-specialist.
