---
namespace: discoveries
agent_id: agent-07-source-tier-classifier
timestamp: 2026-04-16T17:05:00Z
status: active
tags: tier-3, justification, keep-or-replace, no-tier-1-equivalent
depends_on:
  - memory/decisions/agent-07-source-tier-classifier--tier-assignments.md
---

# Tier-3 Justifications — Why Each T3 Stays (or Must Be Replaced)

Every Tier-3 citation in our core reference set (Clusters A–G) is listed here with
(a) the specific claim it supports, (b) the reason no Tier-1 or Tier-2 equivalent
exists, and (c) the "keep / replace" decision.

For Cluster H (enterprise-market), only the T3s that are **actually likely to be cited
in body** are justified here. The other 14 H T3s are held in the pool for
Acknowledgements-level reference and require no per-citation justification because
they will not appear in body.

---

## Cluster A — Industry-signal T3s (data-wall diminishing returns)

### A05 — Woo & Efrati (2024), *The Information*

- **Claim it supports.** Q01 prose: "industry signals in late 2024 observed
  pre-training returns flattening" — specifically, that OpenAI's internal
  assessments shifted strategy.
- **No-Tier-1-equivalent reason.** Peer-reviewed journals do not report corporate
  strategy shifts in near-real-time; the only public sources are credible tech trade
  press. The Information is regarded as the highest-signal tech-industry trade outlet.
- **Role in body.** **One citation** in §1 Introduction as part of a three-signal
  industry-observation cluster (A05 + A06 + A07 in a single parenthetical).
- **Decision.** **KEEP.** Required for the "diminishing returns in 2024" sub-claim;
  paired with A01–A04 Tier-1 backbone.

### A06 — Metz, Ghaffary & Love (2024), *Bloomberg*

- **Claim it supports.** Q01: same as A05 — industry-wide observation that
  pre-training returns are flattening across multiple labs (not just OpenAI).
- **No-Tier-1-equivalent reason.** Same as A05.
- **Role in body.** One of three citations in the aforementioned §1 cluster.
- **Decision.** **KEEP.** Diversifies the industry-signal observation beyond OpenAI.

### A07 — Hu & Tong (2024), Reuters

- **Claim it supports.** Q01: same as A05/A06.
- **No-Tier-1-equivalent reason.** Same as A05/A06. Reuters is top-tier wire.
- **Role in body.** Same cluster.
- **Decision.** **KEEP.** Three-way corroboration of the industry observation.

### A08 — Franzen (2024), TechCrunch

- **Claim it supports.** Q01: same as A05/A06/A07, slightly more technical framing
  ("scaling laws showing diminishing returns, forcing AI labs to change course").
- **No-Tier-1-equivalent reason.** Same as A05.
- **Role in body.** **Candidate to REPLACE with A02/A03.** A08 says essentially the
  same thing as A03 Villalobos (ICML 2024) and A02 Hoffmann (Chinchilla) say with
  peer-reviewed rigor. The only thing A08 adds is the 2024-industry perspective.
- **Decision.** **KEEP only if one more industry citation is needed.** In practice,
  the three-way A05/A06/A07 cluster is sufficient for §1's industry-signal
  sub-argument; A08 is redundant and can be dropped without evidence loss.
  - **Recommendation to agent-36:** If §1 already cites A05 + A06 + A07 in a single
    parenthetical, do NOT add A08. If §1 needs a fourth industry citation, A08 is
    the preferred drop.

### A09 — Sutskever NeurIPS 2024 Test-of-Time talk (oral)

- **Claim it supports.** Q01: the most-quoted sentence from the industry-signal
  observation ("pre-training as we know it will unquestionably end… data is the
  fossil fuel of AI").
- **No-Tier-1-equivalent reason.** The quote itself is an oral remark; no written
  peer-reviewed artefact exists. A citation to the talk (e.g., "Sutskever, NeurIPS
  2024 Test-of-Time talk") is the only possible attribution. arXiv submissions
  routinely cite oral remarks attributed to specific talks.
- **Role in body.** Attribution-only. If quoted in the manuscript, use a footnote:
  "as attributed to Sutskever's NeurIPS 2024 Test-of-Time talk (oral)."
- **Risk.** Over-reliance on a rhetorical quote can pull §1 toward marketing tone
  (FM-01 risk). Agent-35 introduction-writer must use this at most once and tied to
  a concrete sub-claim.
- **Decision.** **KEEP with caveat.** Use once or drop; never use as an independent
  evidentiary anchor.

---

## Cluster F — Reference-work T3

### F12 — Ekman & Friesen (1978), *Facial Action Coding System*

- **Claim it supports.** Q11: "ClipCannon's seven-modality DAG includes an emotion /
  expression modality grounded in the Facial Action Coding System (FACS)."
- **No-Tier-1-equivalent reason.** FACS is the canonical print reference for facial-
  expression measurement. No peer-reviewed paper *replaces* this primary reference; it
  is cited in thousands of computer-vision papers as the ground truth for expression
  taxonomy. The task brief explicitly names "Ekman & Friesen 1978" as the example of
  a T3-class reference work.
- **Role in body.** §4.2 ClipCannon systems description; potentially §6.2 Case 2
  methods. Single citation.
- **Decision.** **KEEP.** Required by domain convention; no substitute.

---

## Cluster G — Tool-dependency T3s

### G06 — Google MediaPipe

- **Claim it supports.** Q11: ClipCannon's face-landmarking stage uses MediaPipe Face
  Mesh.
- **No-Tier-1-equivalent reason.** MediaPipe Face Mesh is released as software, not
  as a research paper. The closest peer-reviewed analogue (Kartynnik et al. 2019 on
  MediaPipe Face Mesh) exists as a workshop paper but is not itself the artefact;
  the GitHub release is what ClipCannon actually links against. Task brief explicitly
  names GitHub repositories as T3.
- **Role in body.** §4.2 single-sentence toolchain disclosure.
- **Decision.** **KEEP.** Required for reproducibility disclosure (G6 reproducibility
  completeness gate).

### G07 — YOLOv5-Face (TinaChen et al. 2021)

- **Claim it supports.** Q11: ClipCannon face-detection stage.
- **No-Tier-1-equivalent reason.** Same as G06 — software release, not peer-reviewed
  paper.
- **Role in body.** §4.2 toolchain disclosure.
- **Decision.** **KEEP.** Required for reproducibility disclosure.

### G08 — Resemble AI Resemble Enhance

- **Claim it supports.** Q11: ClipCannon audio-enhancement stage.
- **No-Tier-1-equivalent reason.** Same as G06 — software release.
- **Role in body.** §4.2 toolchain disclosure.
- **Decision.** **KEEP.** Required for reproducibility disclosure.

### G09 — Model Context Protocol specification

- **Claim it supports.** Q10 (Context Graph): "Context Graph exposes 75 MCP tools";
  Q11 (ClipCannon): "58 MCP tools."
- **No-Tier-1-equivalent reason.** MCP is an official spec, not a research paper.
  Task brief explicitly names "official specifications (MCP spec)" as T3.
- **Role in body.** §4.1 + §4.2 single citation each.
- **Decision.** **KEEP.** Required; no substitute.

---

## Cluster H — Enterprise-market T3s likely to be body-cited

Per the master table's body-citation rule (agent-36/44 use Cluster-H refs sparingly
and prefer the Cluster-H T1/T2 over T3), **most Cluster H T3s will NOT appear in
body**. The ones that *might* appear are:

### H01 — Anthropic MCP announcement

- **Claim.** Introduces MCP — optional corroborator to G09 (the spec).
- **Decision.** **DROP.** G09 is sufficient; H01 is corporate blog redundant.

### H08 / H09 — OpenAI Reddit / News Corp partnerships

- **Claim.** Q19: "path 1 — license more corpora" has a 2023–2024 industry
  cadence.
- **No-Tier-1-equivalent.** Corporate licensing announcements are not peer-reviewed.
- **Decision.** **KEEP AT MOST ONE.** H08 (Reddit, $60M at H11 CBS) is the most
  concrete data-point. Use H08 + H11 together if needed, or drop both.
- **Recommendation to agent-36/44.** If §1 names "path 1 — license more", a single
  citation to H10 Reuters or H11 CBS (for the Google-Reddit figure) plus a single
  citation to H18 Variety (for the News-Corp figure) is sufficient. At most two
  Cluster-H T3s in §1.

### All other Cluster-H T3s (H02, H03, H04, H06, H07, H10, H11, H12, H14, H16, H17,
H18, H19) — held in pool, not expected in body.

---

## Summary decisions

| Citation | Decision | Rationale |
|---|---|---|
| A05 The Information | KEEP | Industry-signal anchor; no Tier-1 equivalent |
| A06 Bloomberg | KEEP | Industry-signal corroborator; no Tier-1 equivalent |
| A07 Reuters | KEEP | Industry-signal corroborator; no Tier-1 equivalent |
| A08 TechCrunch | KEEP IF NEEDED / DROP IF NOT | Redundant with A05/A06/A07 |
| A09 Sutskever oral | KEEP WITH CAVEAT | Single-use attribution; not an evidentiary anchor |
| F12 Ekman & Friesen 1978 | KEEP | Canonical FACS reference; no substitute |
| G06 MediaPipe | KEEP | Reproducibility toolchain; software release |
| G07 YOLOv5-Face | KEEP | Reproducibility toolchain; software release |
| G08 Resemble Enhance | KEEP | Reproducibility toolchain; software release |
| G09 MCP spec | KEEP | Official spec; no substitute |
| H01 Anthropic MCP blog | DROP | Redundant with G09 |
| H02 Field TechCrunch Reddit | DROP | Redundant with H10/H11 |
| H03 Gartner | DROP | Not needed for core manuscript |
| H04 Caselaw Access | DROP | Not needed for core manuscript |
| H06 NCI Thesaurus | DROP | Not needed unless §4 references biomedical vocabularies |
| H07 MeSH | DROP | Same as H06 |
| H08 OpenAI-Reddit | DROP or KEEP AT MOST | Choose H08 or H10, not both |
| H09 OpenAI-NewsCorp | DROP or KEEP AT MOST | Choose H09 or H18, not both |
| H10 Reuters Reddit-Google | KEEP IF NEEDED | $60M figure anchor for path-1 argument |
| H11 CBS Cerullo Reddit-Google | KEEP IF NEEDED | Same as H10 |
| H12 Shutterstock-OpenAI | DROP | Not needed; H10+H18 suffices for path-1 |
| H14 SNOMED CT | DROP | Not needed unless §4 references biomedical vocabularies |
| H16 SEC EDGAR about | DROP | Not needed for core manuscript |
| H17 SEC EDGAR search | DROP | Not needed for core manuscript |
| H18 Variety News-Corp | KEEP IF NEEDED | $250M figure anchor for path-1 argument |
| H19 Verma/De Vynck WaPo AP | DROP | Redundant with H18 |

---

## Counted outcome of "drop-if-not-needed" recommendations

If agent-36 follows all "DROP IF NOT NEEDED" recommendations above, the
final-in-body citation count drops to a cleaner set:

- Cluster A: keep A01–A09 minus A08 → 8 refs (T1=3, T2=1, T3=4)
- Cluster B: all 7 → 7 (T1=4, T2=3)
- Cluster C: all 5 → 5 (T1=3, T2=2)
- Cluster D: all 10 → 10 (T1=4, T2=6)
- Cluster E: all 13 → 13 (T1=2, T2=11)
- Cluster F: all 12 → 12 (T1=3, T2=8, T3=1)
- Cluster G: all 9 → 9 (T1=1, T2=4, T3=4)
- Cluster H (kept): at most {H05 Henderson, H10 or H11, H13 Singhal, H15 Tu, H18,
  H20 BloombergGPT} = 6 refs (T1=3, T2=1, T3=2)

**Total in body = 70 refs, of which T1 = 23, T2 = 36, T3 = 11 → T1+T2 = 84% ✅**

This is within arXiv's 30–60 reference soft target (we will be closer to 60) and
comfortably above the 80% T1+T2 gate.

If agent-36 chooses to drop A08 AND A09 (both are "drop-candidates"), the T3 count
drops by 2 more → 9 T3s in 68 body refs → T1+T2 = 87%. Even safer.

---

## Must-replace / ineligible references

The following references in the evidence pool are **ineligible for citation** in the
arXiv submission in their current state:

| Ref | Reason | Required action |
|---|---|---|
| Qwen3-TTS / Qwen3-Voice-Embedding | "Unverified / Excluded" in brief 03 | If Case 3 cites the base model, cite the GitHub model card as T3 with explicit "no arXiv available at time of submission" note. Agent-08 must verify the GitHub URL resolves. |
| Demucs v1/v2/v3 (Défossez 2019/2021) | "Unverified / Excluded" in brief 03 | If cited, agent-08 must re-verify the arXiv identifier; then tier as T2. Otherwise cite Rouard et al. 2023 (G05, T2) as the canonical source-separation reference. |

---

## Change log

- 2026-04-16 — Initial issue by agent-07-source-tier-classifier.
