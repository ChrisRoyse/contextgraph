---
namespace: handoff
agent_id: agent-35-introduction-writer
timestamp: 2026-04-17T09:50:00Z
status: active
tags: handoff, agent-36, literature-review-writer, introduction-ready
---

# Handoff: Agent-35 (introduction-writer) → Agent-36 (literature-review-writer)

## Read these files FIRST, in this order

1. `/home/cabdru/contextgraph/docs/arxivpaper/sections/01_introduction.md` — the §1 prose as it stands. Read this BEFORE writing §2 so the related-work section's scope matches what §1 has already teed up.
2. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-35-introduction-writer--writing-decisions.md` — the writing decisions, hedge catalogue, and claim-inclusion map for §1.
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md` — the paste-ready hedges and banned-phrase list (still load-bearing for §2).
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-21-theory-builder--theoretical-framework.md` — the spine §5 "Relation to existing frameworks"; §2 of the manuscript should be a superset of that section's framing, with full literature citations.
5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-18-gap-hunter--consolidated-gaps.md` — the 20-gap ledger; §2 must tee up the gaps §3/§5 will fill.

## What §1 accomplishes (summary)

Section 1 runs a funnel from (a) the data wall, model-collapse, and enterprise-licensing race; to (b) a narrowing question about projecting a fixed corpus through frozen embedders and aligning generation to a frozen geometric centroid set; to (c) the three locked constructs (DDA, meaning compression, TCT); to (d) two production systems and three case studies with Case 3 as the measured anchor (0.961 mean WavLM SECS, encoder-matched protocol, 10 held-out sentences, one speaker, English-only); to (e) the three-role composition (retriever / labeller / guard) with three consequences (counting identity; $O(M\cdot k)$ theorem; per-output verifiability); to (f) a competing-interests disclosure; to (g) a roadmap.

## Themes §1 tees up that §2 must pick up

### Theme A — Data-wall and model-collapse cluster (Cluster A of the pool)

§1 names Villalobos et al. (2022/2024), Longpre et al. (2024), Shumailov et al. (2024), Dohmatob et al. (2024) (both 2024 papers), Alemohammad et al. (2023), Gerstgrasser et al. (2024), Feng et al. (2024), plus the industry narrative cluster (Efrati & Holmes, 2024; Metz et al., 2024; Hu & Tong, 2024). **§2 Cluster A must**: (i) state the Shumailov recursion precondition in full (not just the prose form — with the $D_{t+1} \ni \tilde x \sim p_{\theta_t}$ equation); (ii) walk through Dohmatob's scaling-law modification and Alemohammad's MAD separately; (iii) pick up Kaplan et al. (2020) and Hoffmann et al. (2022) Chinchilla so §5.1 can extend them along the structured-signal axis; (iv) close with the explicit scope note that §3.3 picks up — DDA has no generator-in-loop recursion, so the collapse theorems do not apply by construction (this is C4's P2 proof-by-scope-argument).

### Theme B — Compression-as-intelligence (Cluster C of the pool)

§1 introduces meaning compression as a fourth taxonomic entry with a semantically distinct unit. **§2 Cluster C must** cover Delétang et al. (2024), Huang et al. (2024) (r≈0.95 within-model compression-capability correlation), Li et al. (2025), Valmeekam et al. (2023), Jiang et al. (2023), and the bit-/weight-/activation-compression families (SparseGPT, GPTQ, distillation, KV-cache quantisation, TurboQuant). §5.2 of the framework extends this cluster by the fourth-category proposal; §2 sets up the three existing categories without overclaiming the fourth (save the fourth-category argument for §3 / §5).

### Theme C — Multi-modal embedding and contrastive geometry (Cluster D of the pool)

§1 asserts the three-role composition (retriever / labeller / guard) over identical frozen instances and flags the pool-relative novelty. **§2 Cluster D must** cover the retriever role's literature baseline (contrastive encoders, dense retrieval, CLIP / SigLIP, late-interaction ColBERT, SPLADE), the labeller role's literature baseline (multi-modal pseudo-labelling, sensor-fusion-style data enrichment), and the guard role's literature baseline (reference-based speaker verification, FID-like geometric scoring). The novelty §1 promises (three roles over identical instances) lands only if §2 establishes that the three role literatures exist separately but are not composed over frozen identical instances in the pool.

### Theme D — Scalar-reward alignment (Cluster I of the pool)

§1 structurally contrasts TCT with scalar-reward alignment via InstructGPT (Ouyang et al., 2022), DPO (Rafailov et al., 2023), and Constitutional AI (Bai et al., 2022). **§2 Cluster I must** give the canonical framing (reward-model learning; PPO / DPO closed-form; CAI's natural-language rule-based preferences) without claiming TCT outperforms any of them on their benchmarks (C13 scope) and without dismissing their contributions. The axis-separation is key: scalar-reward aims at a learned scalar; TCT aims at a frozen geometric target. §5.3 formalises this break.

### Theme E — Identity-locked generation (Clusters E & F of the pool)

§1 names three case studies (Shakespeare LoRA, Santa talking-head clone, voice cloning beyond 0.95 SECS) and the constellation guard. **§2 must** cover the conventional subject-specific fine-tuning literature (many-shot LoRA, subject-specific DreamBooth / textual inversion families) and the single-prompt ICL / zero-shot cloning literature (voice-prompt conditioning, single-image avatar). The sample-complexity collapse claim (C12a constructive; C12b one T1 empirical anchor) is positioned against these two clusters as the comparator.

## Sections §2 should reserve for §3 (do NOT claim in §2)

- The formal DDA signal-count identity $|D'| = n \cdot (N + \binom{N}{2})$. §2 states the counting-gap (GAP-01) but does not claim the identity.
- The collapse-silence scope argument. §2 states the collapse literature; §3.3 argues the scope exemption.
- The meaning-compression fourth-category argument. §2 states the three existing categories; §3.1 / §5.2 argue the fourth.
- The constellation-guard predicate. §2 states the scalar-reward literature; §5 formalises $G_\tau$.
- The sample-complexity theorem. §2 states the cluster-E/F conventional-fine-tuning baseline; §5.4 states Theorem 1.

## Hedges that MUST carry into §2

- **"Up to $N + \binom{N}{2}$" / "up to 91 signals"** every time the DDA count is mentioned; never the bare number.
- **"Structurally not subject to the generator-in-loop recursion regime of Shumailov et al. (2024)"** is the exact phrasing for the scope exemption; never "immune to collapse".
- **"Semantically distinct unit"** for meaning compression vs. bit-/weight-/activation-compression; never "dimensionally distinct" (W8 / rec-5).
- **"Pool-relative novelty"** on every "no cited method" claim in §2; the 88-citation master pool is the comparator, not the global literature.
- **"Approximately independent"** on every Axiom-2 mention; V11 hedge carries.
- **"Frozen" / "frozen-with-LoRA"** discipline (V18); never "pretrained" alone when the frozenness is load-bearing.

## Banned phrases (agent-34 table, carried intact into §2)

- "91 signals per input" (bare)
- "dimensionally distinct"
- "0.961 mean SECS" (bare)
- "indistinguishable from real speech" (bare)
- "solves the data wall"
- "outperforms RLHF"
- "T=0.3 is optimal" (bare)
- "proven collapse-free"
- "mouth-weighted loss is necessary"
- "we demonstrate the sample-complexity collapse"

## What §2 produces for downstream agents

- A literature map organised by cluster (A, C, D, E, F, I) consistent with agent-06's topology and agent-17's thematic framework.
- A tee-up of GAP-01 (signal-count identity unformalised), GAP-02 (no third path on the data-wall menu), and GAP-03 / GAP-04 / GAP-05 / GAP-06 / GAP-07 as anchors for §3's framework.
- A conservative pool-relative framing that §5's relation-to-existing-frameworks section builds on.

## Files agent-35 produced

- `/home/cabdru/contextgraph/docs/arxivpaper/sections/01_introduction.md` (the §1 prose)
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-35-introduction-writer--writing-decisions.md`
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-35-to-36--introduction-ready.md` (this file)
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-35-introduction-writer--final-report.md`

## Carried hedges from upstream (unchanged)

- All agent-34 C1–C14 calibrated hedges; all agent-33 softening recommendations (rec-1 MUST, rec-2 MUST, rec-3 / rec-4 / rec-5 SHOULD).
- V11, V18, V22, R1, R3, R4, FM-01 from agent-01 and agent-17.
- Bias-05 / Bias-06 on Case-3 single-speaker English-only scope.

## One-paragraph summary for agent-36

> Agent-35 produced a 1,616-word §1 that runs the funnel from the data wall (Villalobos, Longpre, Shumailov, Dohmatob, Alemohammad, Gerstgrasser, Feng; Efrati/Metz/Hu 2024 industry cluster; News Corp / Reddit licensing deals) to the two existing paths (acquire / synthesise), to the narrowing question (fixed-corpus projection through frozen embedders + frozen geometric alignment target), to the three locked constructs (DDA, meaning compression, TCT), to Case 3 as the measured anchor (0.961 mean WavLM SECS, encoder-matched, $N=10$ held-out, one English speaker), to the three-role composition claim and its three consequences (counting identity, $O(M \cdot k)$ theorem, per-output verifiability) contrasted structurally but not competitively with scalar-reward alignment. All agent-34 hedges are applied; C12a / C12b is correctly split; Cases 1 and 2 are flagged projected; the CI sentence carries FM-01. §2 should now pick up the four clusters (A, C, D, I) plus E/F in full literature depth, tee up the gaps §3 will fill, and respect every hedge listed above.

## Quality-gate status delivered by agent-35

| Gate | Status |
|---|---|
| §1 prose file written at `sections/01_introduction.md` with `# 1. Introduction` header | **PASS** |
| Word count within 1,500–2,000 target (1,616 words) | **PASS** |
| Funnel structure (broad → narrow → specific) | **PASS** |
| Case-3 headline stated with encoder-matched scope | **PASS** |
| "up to 91 signals" used everywhere (not bare "91") | **PASS** |
| Constructive vs empirical split on sample-complexity claim | **PASS** |
| Scalar-reward contrast framed structurally, not competitively | **PASS** |
| CI disclosure + no vendor-pitch prose | **PASS** |
| Roadmap + §2 hand-off sentence | **PASS** |
| All banned phrases absent | **PASS** |
| No Python/Rust code blocks in body | **PASS** |
| All upstream hedges carried | **PASS** |

## Confidence

**HIGH** that the §1 prose accurately reflects the agent-34 calibrated confidence levels.

**HIGH** that §2 has a complete tee-up across Clusters A, C, D, E/F, and I.

**HIGH** that no banned phrase slipped through and the C14 CI disclosure is sufficient for FM-01.

## Changelog

- 2026-04-17 — Initial handoff by agent-35-introduction-writer.
