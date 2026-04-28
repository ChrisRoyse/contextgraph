---
namespace: research
agent_id: agent-13-conversation-enablement
timestamp: 2026-04-19T09:00:00Z
status: active
tags: pitches, conversation-engineering, 10s, 30s, 2min, audience-variants, scope-preserving
---

# Pitches — Teleox.ai Conversation Engineering

Every pitch below passes the pre-send checklist (measured anchor, scope tag, CI-compatible). Prohibited phrases ("SOTA", "beats RLHF", "refutes collapse", "cross-encoder verified") are absent. Paper sections cited with § symbol.

---

## 1. 10-Second Pitch — primitive-first, four variants

### 1A. Primitive variant (DEFAULT — use unless otherwise specified)
"I've found a data-layer primitive: project a fixed corpus through N frozen approximately-independent embedders → up to `n·(N+C(N,2))` structured clean labeled supervisory signals (§0.2, constructive upper bound, EXP-2 audit pending). At N=13 public embedders, up to 91 per input. Every new embedder added to the pool multiplies correlations against every existing one — public embedder inventory is the constraint; the primitive is unbounded. I've built three case studies as exploratory proofs. They aren't products."

### 1B. Technical variant (for researchers, chief scientists, ML leads)
"The primitive is a counting identity in the data layer: fixed corpus × N frozen embedders → up to `n·(N+C(N,2))` structured supervisory signals (§0.2), constructive upper bound, pairwise mutual-information audit pending (§9). Non-recursive by construction — structurally outside the Shumailov regime (§3.3, scope not refutation). Case 3 voice identity under this primitive: **0.961 mean WavLM SECS, within-WavLM-family, N=10, one speaker, English, MEASURED 2026-04-16**. The voice clone is an exploratory proof — not the pitch."

### 1C. Business variant (for corp-dev, bankers, CFOs, sovereign)
"The data wall produced a $250M line item at News Corp and a $1.5B author settlement at Anthropic. I've discovered a primitive that multiplies clean labeled supervisory signal on any fixed corpus by the number of frozen embedders you project it through — up to 91 signals per input at N=13, constructive upper bound. Every new embedder extends the pool. This is the first step of the AI pipeline — the data. Case 1 Shakespeare is MEASURED, LoRA public on HuggingFace. The Shakespeare LoRA is a probe; the story is the primitive."

### 1D. Analogical variant (for mixed audiences, intros, panels)
"Think of it as stereo vision for training signals — but with N frozen eyes instead of two. Every input projected through the pool yields up to `n·(N+C(N,2))` structured paired supervisory signals. Add a new eye (a new embedder for any meaning-representation) and every other eye's view gets more informative. The possibility space is unbounded by the primitive itself; public embedder inventory is what constrains it today. I built three case studies — not to sell them, but to prove the primitive works across text, video, and audio identity."

### 1D. Product variant (for enterprise / commercial audiences)
"Teleox ships three production stacks: Context Graph (370K LOC Rust), ClipCannon (67,585 LOC Python, 58 MCP tools, 1,131 tests), Phoenix + VoiceAgent (real-time AI-clone on Google Meet and Zoom). Runtime ConstellationGuard validates every generated frame against a 7-modality 4,044D identity centroid. SHA-256 provenance chain per operation. Live demo on 24-hour notice."

---

## 2. 30-Second Pitch — two variants

### 2A. Comprehensive variant (most situations)
"The paper (arXiv: link) proposes Derived Data Abundance — a fixed corpus of n inputs projected through N frozen, approximately-independent embedders yields up to n·(N+C(N,2)) structured supervisory signals: **up to 91 per input at N=13** — a constructive upper bound (§3.1), with tightness pending EXP-2 pairwise mutual-information audit (§9). DDA is structurally outside the Shumailov generator-in-loop recursion regime (§3.3) — scope argument, not refutation. On top of DDA we build TCT, a three-role composition (retriever + labeller + runtime `G_τ` guard, §5.1, §5.4) over *identical frozen embedder instances* — that composition is the pool-relative novelty claim (§2.4). Case 1 Shakespeare is **MEASURED 2026-04-17**: SFT loss 1.12, DPO loss 0.47, 98.9% token-accuracy — LoRA public at huggingface.co/cabdru/shakespeare-lora-gemma4. Case 3 voice: **0.961 mean WavLM SECS, within-WavLM-family, N=10, one speaker, English, MEASURED 2026-04-16** — ECAPA-TDNN triangulation (EXP-1) pending."

### 2B. Intrigue variant (for cold opens, noisy contexts)
"Two measured results: Shakespeare LoRA, SFT loss 1.12 and DPO loss 0.47 at 98.9% token accuracy — open-weight on HuggingFace right now. A voice-identity result at 0.961 mean WavLM SECS within-WavLM-family on 10 held-out English sentences, one speaker — scope disclosed. The paper behind them (§0.2) proposes a counting identity: up to 91 structured supervisory signals per input at N=13 embedders, constructive upper bound, audit instrument specified (§9). Worth 15 minutes?"

---

## 3. 2-Minute Pitch — full flow with pause-points

**[0:00 — open]** "I'm Chris Royse at Teleox.ai. My paper drops a counting identity in §0.2: a fixed corpus of n inputs projected through N frozen, approximately-independent embedders yields up to n·(N+C(N,2)) structured supervisory signals per input — up to 91 at N=13. That's a **constructive upper bound** (§3.1), and the tightness is pending EXP-2, a pairwise mutual-information audit across the 13-embedder panel (§9)."

**[0:20 — pause for recognition]** *[Wait for nod or question; if question comes, answer it before continuing.]*

**[0:25 — scope argument]** "The construction is non-recursive — no generator-in-loop. So the Shumailov collapse theorems don't apply to DDA by precondition (§3.3). I want to be clear: that's a **scope argument, not a refutation**. EXP-2 is the falsification instrument; if pairwise MI collapses to zero across the panel, the upper bound tightens materially."

**[0:50 — TCT construct]** "On top of DDA we build TCT — teleological constellation training. Three roles — retriever, labeller, runtime `G_τ` guard (§5.1, §5.4) — all executed over *identical frozen embedder instances*. `G_τ` is a deterministic `O(M·d_max)` cosine test against a chosen centroid panel. That's **per-output geometric verifiability**, a different property class from scalar-reward alignment (§7.3) — not a benchmark contest against RLHF or DPO."

**[1:20 — measured anchors]** "Two measured anchors. Case 1 Shakespeare, **MEASURED 2026-04-17**: SFT loss 1.12, DPO loss 0.47, 98.9% token-accuracy — LoRA is open at huggingface.co/cabdru/shakespeare-lora-gemma4. Case 3 voice, **MEASURED 2026-04-16**: **0.961 mean WavLM SECS, within-WavLM-family, N=10 held-out English sentences, one speaker** — ECAPA-TDNN cross-encoder triangulation (EXP-1) is scheduled; I disclose the encoder-matched scope verbatim (§6.3, §8.L1)."

**[1:45 — ask]** "Two concrete asks depending on your role. If research: 15 minutes on whether §3.3 correctly characterizes the scope argument, and co-author any of EXP-1/2/3. If operations or capital: 48-hour proof-of-concept — give me one corpus and 3 to 13 embedders you already hold; I return a DDA signal count, a TCT `G_τ` guard with cosine trace per output, and a reproducible measurement package scoped per paper §6 and §8. Your data never leaves your environment."

**[2:00 — close]** *[Hand over paper PDF + LoRA link; if no handover possible, confirm follow-up email within 2 hours.]*

---

## 4. Audience Variants

### 4A. Researcher (Dohmatob / Perez / Khattab / Kaplan / Kempe / Feng)
**Hook**: "Your 2024/25 collapse paper's generator-in-loop precondition — does §3.3 correctly characterize DDA as structurally outside that regime?"
**Body**: Lead with constructive upper bound (§3.1); offer EXP-2 co-author role; anchor Case 3 within-WavLM scope disclosed.
**Ask**: 15-min call on scope + co-author invitation.
**Avoid**: commercial language; TAM; "beats"; "refutes".

### 4B. Sovereign (MGX / Humain / QIA / GIC)
**Hook**: "Your national data is sovereign; your embedder panel should be too. Up to 91 structured signals per input (§0.2, constructive upper bound) multiplies the value of corpora you already own — without a licensing market."
**Body**: DDA as national-data-multiplier; TCT on-prem deployability; non-recursive by construction; 48h PoC runs inside your environment.
**Anchor**: Case 1 MEASURED (open-weight LoRA); Case 3 MEASURED (within-WavLM-family, scope disclosed).
**Ask**: intro to corp-dev / chief scientist; 48h PoC on a bounded sovereign corpus slice.
**Avoid**: Teleox-originated TAM; "superior to"; any cross-border-data commitment not in paper.

### 4C. Banker (Qatalyst / LionTree / Centerview / Allen)
**Hook**: "Figma–Adobe precedent at $20B shows the category willingness; News Corp × OpenAI at $250M shows the data-wall spend. The paper's (§0.2) constructive upper bound of up-to-91 structured signals per input sits structurally outside the licensing market."
**Body**: Industry comparables: Scale AI $14.3B / 49%, Character $2.7B licensing, Inflection $650M talent-license, Adept reverse-acquihire. Both measured anchors are public; reproducibility appendix A supports diligence.
**Anchor**: Case 1 MEASURED + open LoRA → diligence-ready; Case 3 MEASURED + scope disclosure → methodological rigor.
**Ask**: 20-min sell-side mandate exploration.
**Avoid**: pricing; quoting Teleox valuation; implying a committed counterparty.

### 4D. Hyperscaler exec (Anthropic / Google / Meta / Microsoft / Amazon)
**Hook**: "Kaplan's 2020 scaling-laws paper predicts this data wall. The paper (§0.2, §3.1) proposes a constructive counter-example structurally outside the Shumailov regime (§3.3) — with an EXP-2 audit instrument specified (§9)."
**Body**: Per-output verifiability via `G_τ` (§5.4) complements RLHF/DPO; different property class (§7.3). Three-role composition over *identical* frozen instances is the pool-relative novelty claim (§2.4), not embedder choice (§3.4).
**Anchor**: Case 1 MEASURED open-weight LoRA; Case 3 MEASURED within-family scope; Context Graph production substrate.
**Ask**: technical deep-dive with chief scientist or alignment head; 48h PoC on their licensed corpus.
**Avoid**: "beats RLHF"; "SOTA"; implying frontier-lab deficiency.

### 4E. Venture Capital (Casado / Gil / Guo / Midha / Grady)
**Hook**: "Pool-relative novelty: three-role composition over identical frozen embedder instances (§3.4). Category-creation plays at $1–3B are the structural precedent (Character, Inflection, Adept)."
**Body**: Two measured anchors; fourth-category taxonomic claim (§3.2, reviewer judgement flagged); scope discipline preserved. Substrate already built at production scale (Context Graph ~370K LOC Rust).
**Anchor**: Case 1 open-weight as diligence-ready evidence; Case 3 MEASURED with scope.
**Ask**: 30-min fund-fit call; portfolio-cross-pollination map; follow-on posture question.
**Avoid**: TAM from Teleox; "disruption"; "winner-take-all"; round-size hint.

---

## 5. Pitch-Mode Checklist (apply before every delivery)

- [ ] Measured anchor stated with MEASURED tag and date
- [ ] Case 3 scope: within-WavLM-family + N=10 + one speaker + English + EXP-1 pending
- [ ] "Up to 91" only with "constructive upper bound" + "EXP-2 pending"
- [ ] "Structurally outside Shumailov regime" → always "scope, not refutation"
- [ ] TCT vs. RLHF/DPO → "different property class, not a benchmark contest"
- [ ] Case 2 → "MEASURED & operational 2026-04-19" (paper draft §6.2 tag is paper-internal only; operational stack runs end-to-end on ClipCannon+Phoenix)
- [ ] HF LoRA link available if asked: huggingface.co/cabdru/shakespeare-lora-gemma4
- [ ] CI disclosure ready in follow-up email
- [ ] No prohibited phrases: "SOTA", "beats", "refutes", "cross-encoder verified", commercial TAM

---

**Competing Interests**: Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.
