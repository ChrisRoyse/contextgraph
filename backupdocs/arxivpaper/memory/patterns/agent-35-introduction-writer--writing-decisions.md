---
namespace: patterns
agent_id: agent-35-introduction-writer
timestamp: 2026-04-17T09:45:00Z
status: active
locked: false
supersedes: none
tags: introduction, writing-decisions, funnel, hedging, contribution-list, section-1
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--meaning-compression-definition.md
  - memory/decisions/agent-21-theory-builder--theoretical-framework.md
  - memory/decisions/agent-23-hypothesis-generator--hypotheses.md
  - memory/patterns/agent-34-confidence-quantifier--per-claim-confidence.md
  - memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md
  - memory/handoff/agent-34-to-35--confidences-ready.md
downstream_consumers:
  - agent-36-relatedwork-writer
  - agent-37-methodology-writer
  - agent-38-systems-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
---

# §1 Introduction — Writing Decisions

## 0. Output artefact

- **Path**: `/home/cabdru/contextgraph/docs/arxivpaper/sections/01_introduction.md`
- **Header**: `# 1. Introduction` (no `Introduction` sub-heading — APA 7th discipline carried from the skill brief and consistent with arXiv typical practice).
- **Word count**: ~1,616 words of prose; inside the 1,500–2,000 target band.
- **Paragraph count**: 7 paragraphs — (1) hook / data-wall + collapse + licensing, (2) two-path accounting + the narrow question, (3) three constructs (DDA, meaning compression, TCT), (4) systems + case studies + Case-3 headline, (5) three-role composition + three consequences (theoretical contribution summary), (6) CI disclosure + paper-level claim recap, (7) roadmap ending with §2 hand-off.
- **Citations in body**: 15 distinct APA-style in-text citations plus the 2024 industry narrative cluster. Reference list is owned by agent-44.

## 1. Funnel structure (broad → narrow → specific)

| Paragraph | Funnel layer | Purpose |
|---|---|---|
| P1 | Broad opening | Data wall: Villalobos et al. (2022/2024) 300T tokens, 2026–2032 exhaustion; Longpre et al. (2024) ~25% C4 restricted; Shumailov et al. (2024) *Nature* collapse; Dohmatob et al. (2024), Alemohammad et al. (2023), Gerstgrasser et al. (2024), Feng et al. (2024); industry reporting (Efrati & Holmes, 2024; Metz et al., 2024; Hu & Tong, 2024); commercial licensing race (News Corp $250M, Reddit $60M, Field, 2024; Verma & De Vynck, 2023). |
| P2 | Narrow | Two-path accounting (acquire / synthesise); state the Shumailov recursion precondition verbatim $D_{t+1} \ni \tilde x \sim p_{\theta_t}$; pose the two narrowing questions (fixed-corpus projection through frozen embedders; frozen-geometric-centroid alignment target). |
| P3 | Specific constructs | DDA, meaning compression, TCT — defined verbatim from agent-04 locks with the agent-34 hedges applied (see §2 below). |
| P4 | Systems + cases | Context Graph (~370K lines, 13 embedders, 59 CFs, 75 MCP tools, 5,184 tests); ClipCannon (~67,585 lines, 23-stage DAG, 7 modalities); Case-3 headline as anchor; Cases 1 & 2 projected; Case-3 scope boundaries (encoder-matched within-WavLM, single speaker, English-only, $N=14$ naturalness). |
| P5 | Structural contribution | The three-role composition (retriever / labeller / guard) over identical frozen instances (P4 from agent-21); the three consequences (counting identity; $O(M \cdot k)$ theorem; per-output verifiability); explicit scalar-reward contrast (InstructGPT, DPO, CAI). |
| P6 | CI & paper-level claim recap | Teleox.ai / Leapable.ai disclosed; anti-overclaim sentences ("do not solve the data wall; reframe as three paths"). |
| P7 | Roadmap | §2–§9 outline; final sentence previews §2 (collapse literature + DDA scope exemption). |

## 2. Hedging decisions (which agent-34 cues were applied, why, and where)

### Applied verbatim

- **C1 "up to 91 signals"** — paragraphs 3, 5. Never bare "91 signals". Always "up to 91 signals per input at $N=13$ as a constructive upper bound" with "tightness pending empirical mutual-information measurement" at least once. Matches agent-34 banned-phrase rule.
- **C5 0.961 mean WavLM SECS** — paragraph 4. Always paired with the **encoder-matched** scope modifier and the **within-WavLM-family identity-agreement** companion phrase. Cross-encoder triangulation (ECAPA-TDNN) is named future work inline. Matches agent-34 W1 / IC-3 / rec-2 mandatory companion sentence.
- **C6 DNSMOS parity** — paragraph 4. Framed as "parity (3.93 vs 3.93, $N=14$)" and "naturalness observations", never as "statistically equivalent". The $N=14$ annotation is load-bearing.
- **C12a vs C12b split** — paragraph 4 (and echoed in paragraph 5). Constructive theorem phrased as *holds by construction*; empirical anchor phrased as *one T1 operating point* with Cases 1 & 2 *projected to re-verify at camera-ready*. This is the agent-33 rec. 2 MUST softening.
- **C2 "semantically distinct"** — paragraph 3. Uses "measured in a semantically distinct unit (signals per raw-data-unit) from Shannon bits, accuracy per parameter, or memory per output quality". Never "dimensionally distinct" (agent-33 rec. 5 SHOULD + agent-34 banned-phrase table). "Proposed as a fourth taxonomic entry" carries the reviewer-acceptance hedge.
- **C3 per-output verifiability** — paragraph 5. Phrased as "per-output verifiability property" + "relative to the chosen centroid panel" + "pool-relative" + "in the same technical sense" as the comparator. Never bare "verifiable" (agent-34 C3 permitted/forbidden rules).
- **C4 collapse-silent scope** — paragraph 2. Rendered as "the collapse precondition is not satisfied" (categorical scope), not "immune to collapse" (immunity theorem). The Shumailov recursion form is stated explicitly so the scope is auditable. W9 boundary (curation feedback) is left to §3.3 / §8 per agent-34 cue.
- **C13 scalar-reward contrast** — paragraph 5. Phrased as "does not provide in the same technical sense" + "structural, not a benchmark competition" + pool-relative scope on the novelty. Never "outperforms" / "beats".
- **C14 commercial-interests acknowledgement** — paragraph 6. Single sentence on Teleox.ai / Leapable.ai; explicit "makes no claims about commercial performance or market size" to carry the FM-01 regex check.

### Cases 1 & 2 T3 tagging

- **Case 1** (paragraph 4): "a prior Shakespeare run passed 15/15 forensic stylometric validation, a retrain on forensically-cleaned data is in progress at the time of writing" — C11 rec-at-~60%, agent-34 permitted paraphrase.
- **Case 2** (paragraph 4): "Santa clone's flow-matching sign-bug has been diagnosed … with camera-ready metrics projected to re-verify" — C9, C10, H2d, H3b combined T3 framing, no "demonstrates", no "necessary" (C10 permitted paraphrase: forensic finding language only).

### Not in §1 (by design; routed downstream)

- **H2c, H2d threshold-calibration matrix** — out-of-scope for §1; §6 will carry. Mentioned here only via the general "projected" framing.
- **EXP-1, EXP-2, EXP-3** remediation experiments — named as future work inline (cross-encoder triangulation; mutual-information audit; $n_m$ sweep) but not listed individually. §8 Limitations owns the full list.
- **RLHF preference-score benchmarks** — explicitly NOT claimed to be outperformed (C13 scope).

## 3. Claims included in §1 and why

| Agent-34 claim | In §1? | How it lands |
|---|---|---|
| C1 up-to-91 signals | YES | Paragraphs 3 and 5 (headline + contribution summary). |
| C2 fourth compression category | YES | Paragraph 3 (definition) with "proposed" hedge. |
| C3 per-output verifiability | YES | Paragraph 5 (contribution summary) with "relative to chosen centroid panel" scope. |
| C4 Shumailov-exempt by construction | YES | Paragraph 2 (motivation — states the recursion precondition explicitly). |
| C5 0.961 WavLM SECS | YES | Paragraph 4 (headline anchor) with full encoder-matched scope paragraph-internally. |
| C6 DNSMOS parity | YES | Paragraph 4, one sentence, with $N=14$ annotation. |
| C7 $T=0.3$ optimal | NO (delegated) | Too narrow for §1; §6.3 owns it. §1 mentions "$T=0.3$" only as a protocol detail of the Case-3 result, not as an optimality claim. |
| C8 50-clip centroid +0.015 | NO (delegated) | §6.3 ablation table owns it. §1 mentions "50-clip centroid" as part of the protocol only. |
| C9 guard ON/OFF Case 2 | NO (delegated) | §6.2 owns it; §1 covers it only under the general T3 Case-2 flag. |
| C10 mouth-weighted loss | NO (delegated) | §6.2 owns it; §1 covers it only under Case-2 architectural note. |
| C11 15/15 FSV retrain | YES (weak) | Paragraph 4, single clause, with "prior run" + "retrain in progress" framing. |
| C12a constructive theorem | YES | Paragraph 4 closing + paragraph 5 (theorem list). |
| C12b empirical anchor | YES | Paragraph 4 closing; "one T1 operating point" exact phrasing. |
| C13 pool-relative vs scalar reward | YES | Paragraph 5 (contribution summary + explicit InstructGPT / DPO / CAI list). |
| C14 research not marketing | YES | Paragraph 6 (CI single sentence). |

## 4. Claims explicitly EXCLUDED from §1

- **No "AGI", no "solves", no "revolutionary", no "best", no "breakthrough"** (mission-brief hard constraint + global rules).
- **No "outperforms RLHF"** (C13 forbidden; agent-34 banned-phrase table).
- **No "immune to collapse"** (C4 forbidden; use "structurally not subject to the recursion regime").
- **No URLs** (agent-44 owns references; hyperlinks defer to reference list).
- **No code blocks** (constraint file: no Python, no Rust in body).
- **No tables in §1** (one equation inline: $|C_P|$ — already modest; full counting identity appears in §3.4, full guard predicate in §5).
- **No multi-modality empirical claim** (Case 3 is $M=1$; §1 says so; $M>1$ empirical is projected + future work).

## 5. Figure plan

- §1 references Figure 1 implicitly through the three-role-composition paragraph (P5). It does NOT embed a figure; agent-43 (figure-designer / captioner) handles placement. §3 is expected to embed Figure 1 as the framework diagram.

## 6. Citation inventory for §1 (14 distinct authors/groups + 2024 industry cluster)

All citations drawn from the evidence pools `_evidence_01_data_wall.md` and `_evidence_04_enterprise_market.md`; none newly introduced.

- Villalobos et al. (2022/2024); Longpre et al. (2024); Shumailov et al. (2024); Dohmatob, Feng, Yang, Charton, & Kempe (2024); Dohmatob, Feng, & Kempe (2024) [used as "Dohmatob et al., 2024" per agent-23 style]; Feng, Dohmatob, Yang, Charton, & Kempe (2024); Alemohammad et al. (2023); Gerstgrasser et al. (2024); Efrati & Holmes (2024); Metz et al. (2024); Hu & Tong (2024); Variety Staff (2024); Reuters (2024); Field (2024); Verma & De Vynck (2023); Ouyang et al. (2022); Rafailov, Sharma, Mitchell, Ermon, Manning, & Finn (2023); Bai et al. (2022).

Agent-44 will verify all against the locked 88-pool at `agent-08-citation-extractor--master-reference-pool.md`.

## 7. Register rules honoured

- R1 composition-not-invention: paragraph 3 explicitly names DDA/meaning-compression/TCT as compositions; paragraph 5 describes the *three-role composition* move explicitly.
- R3 pool-relative scope: "to our knowledge in the locked 88-citation pool" appears twice (paragraphs 2 and 5); "pool-relative" on the scalar-reward contrast (paragraph 5).
- R4 measured / projected: Case 3 = measured (encoder-matched scope); Cases 1 & 2 = projected (with paste-ready phrasing); C12a (constructive) distinguished from C12b (one T1 anchor).
- V11 independence hedge: "approximately-independent" (paragraph 3) carries on every DDA mention.
- V18 "frozen" discipline: "frozen embedders" / "frozen parameters" / "frozen multi-modal centroid set" / "frozen geometric object" used consistently. No "pretrained" alone.
- FM-01 (no vendor-pitch prose): paragraph 6 CI sentence is minimal; no URLs, no pricing, no product positioning.

## 8. Confidence

**HIGH** that §1 prose reflects agent-34's calibrated language cue-sheet on every load-bearing claim (C1, C2, C3, C4, C5, C6, C11, C12a, C12b, C13, C14).

**HIGH** that the funnel structure lands the Case-3 0.961 SECS result as the single anchor while scoping it correctly (encoder-matched, within-WavLM, $N=10$ held-out sentences, one English-speaking subject) and that Case 1 / Case 2 are correctly flagged projected / camera-ready-verify.

**HIGH** that the banned-phrase list is honoured (audited sweep: no "91 signals" bare; no "dimensionally distinct"; no "outperforms"; no "immune"; no "revolutionary" / "breakthrough" / "game-changing" / "solves AGI" / "the best").

**MEDIUM** that the citation subset for §1 maps cleanly to the locked 88-pool; agent-44 will verify at the reference-list stage and any drift will be reconciled back.

## Changelog

- 2026-04-17 — Initial issue by agent-35-introduction-writer.
