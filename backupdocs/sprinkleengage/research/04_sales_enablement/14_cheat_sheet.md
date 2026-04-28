---
namespace: research
agent_id: agent-13-conversation-enablement
timestamp: 2026-04-19T09:00:00Z
status: active
tags: cheat-sheet, live-conversation, enablement, phone-readable, 4-page
target_word_count: <=1200
---

# Cheat Sheet — Chris Royse / Teleox.ai
*Phone-readable. 4 printed pages. Memorize page 1.*

---

## PAGE 1 — CORE INTEL (primitive-first; products second)

### 10-second pitch (memorize — primitive-first)
"I discovered a data-layer primitive. Project a fixed corpus through N frozen approximately-independent embedders → up to `n·(N+C(N,2))` structured clean labeled supervisory signals per input, counting identity (§0.2). At N=13 that's up to 91 — **constructive upper bound, EXP-2 audit pending**. Every new embedder added to the pool multiplies correlations against every existing embedder. Public embedder inventory is the constraint; the primitive is unbounded. Three exploratory proofs (Shakespeare LoRA public, talking-head MEASURED operational, voice 0.961 within-WavLM-family MEASURED) — those aren't products, they're proofs of what becomes possible. This is the first step of the AI pipeline — the data."

### 3 key talking points (primitive → proof → scope)
1. **The primitive multiplies clean labeled data by N embedders** — counting identity §0.2; embedder pool is extensible; any meaning-representation can become an embedder.
2. **It sits structurally outside Shumailov** — non-recursive by construction (§3.3). **Scope argument, not refutation.**
3. **The three case studies are exploratory proofs** (Shakespeare public on HuggingFace; Santa MEASURED operational; voice within-WavLM-family MEASURED) — not products.

### Top-5 target table
| Target | Org / Role | Priority | Aligned construct |
|---|---|---|---|
| Elvis Dohmatob | Concordia — "Strong Model Collapse" | P0 Day-0 | §3.3 scope; EXP-2 co-author |
| MGX (Al Idrissi) | Abu Dhabi sovereign hub | P1 Track A | Sovereign signal-multiplier |
| Anthropic (Kaplan via Perez) | Frontier lab; scaling-laws author | P1 Track B | §3.1 vs. data wall; §5.4 `G_τ` |
| Humain (Tareq Amin) | Saudi PIF; token exporter | P1 Track A | DDA national-data-multiplier |
| Qatalyst (Quattrone) | Tier-1 banker; Figma-Adobe | P2 Track C | Case 1+3 MEASURED → mandate |

### Numbers to drop (always scoped)
- Case 1 SFT **1.12**, **98.9% tok-acc** (MEASURED 2026-04-17)
- Case 1 DPO **0.47**, **98.0% rewards-acc**, margin **43.62**
- Case 3 **0.961 mean WavLM SECS**, within-WavLM, N=10, 1 speaker, EN (MEASURED 2026-04-16)
- News Corp × OpenAI **~$250M** (industry anchor)
- Scale × Meta **$14.3B** / 49% (deal-shape precedent)
- **Up to 91** signals/input at N=13 — **constructive upper bound, EXP-2 pending**

---

## PAGE 2 — CONVERSATION STARTERS

### 1. Collapse-paper reply (Dohmatob / Feng / Kempe / Shumailov)
**Trigger**: "Model collapse is the ceiling on synthetic data."
**Open**: "The paper argues DDA sits *structurally outside* the generator-in-loop precondition your theorems require (§3.3). I'd value your read on the characterization — and I'm offering EXP-2 pairwise-MI co-author (§9)."

### 2. Data-wall reply (Kaplan / Villalobos / Longpre)
**Trigger**: "High-quality human text exhausts 2026–2032."
**Open**: "DDA is a third path: fixed corpus projected through N frozen embedders yields up to n·(N+C(N,2)) signals — constructive upper bound (§0.2, §3.1). Non-recursive; tightness pending EXP-2."

### 3. Multi-embedder-retrieval reply (Khattab / Zaharia)
**Trigger**: "This sounds like multi-vector or ColBERT."
**Open**: "Retrieval uses N embedders as retrievers. DDA uses identical N frozen instances in *three composed roles* — retriever + labeller + runtime guard (§5.1). That composition is the pool-relative novelty claim (§2.4)."

### 4. Licensing-cost reply (corp-dev / CFO / Casado)
**Trigger**: "News Corp got $250M; why not license more?"
**Open**: "Licensing is linear in corpus size. DDA multiplies signal yield on any corpus you already own — up to 91/input at N=13 (§0.2). Case 1 MEASURED; LoRA public. 48h PoC runs on your data, your environment."

### 5. Per-output-verifiability reply (Alignment / CAIO / GC)
**Trigger**: "How is this different from RLHF / Constitutional AI?"
**Open**: "Different property class (§7.3). `G_τ` (§5.4) is a deterministic `O(M·d_max)` cosine-trace test against a chosen centroid panel — per-output geometric verifiability. Not a benchmark contest."

---

## PAGE 3 — QUICK FACTS

### Thesis in 3 lines
- **DDA**: n inputs × N frozen embedders → up to n·(N+C(N,2)) signals (up to 91 at N=13). Constructive upper bound; EXP-2 pending.
- **Meaning Compression**: Proposed fourth category — unit = signals/raw-data-unit at prep; distinct from bit/weight/activation at inference.
- **TCT**: Three-role composition (retriever + labeller + `G_τ` guard) over identical frozen instances; per-output verifiability.

### Operational substrate (what actually ships)
- **ClipCannon**: 67,585 LOC Python · 58 MCP tools · 23-stage DAG · 1,131 tests · BSL-1.1.
- **Phoenix**: GPU-native avatar · FLAME 82 fps · PhysicsFaceEngine <5ms · ConstellationGuard per-frame.
- **VoiceAgent**: real-time AI-clone on Google Meet + Zoom. Live today.
- **7-modality 4,044D** constellation (SigLIP 1152 + Nomic 768 + Wav2Vec2 1024 + WavLM 512 + prosody 12 + MiniLM 384 + ECAPA 192).
- **Provenance**: SHA-256 chain hash per operation; auditable. **HMAC billing**: production commercial infra.
- **750 FSV checks** · **17 failed architectures** documented before current stack landed.

### Paper structure map
**§0.2** identity · **§3.1** upper bound · **§3.2** meaning compression · **§3.3** collapse scope · **§3.4** pool-relative · **§5.1** role composition · **§5.4** `G_τ` · **§6.2** Case 2 (paper tag paper-internal; **operational as of 2026-04-19**) · **§6.3** Case 3 scope · **§7.3** property class · **§8.L1/L2** limits · **§9** EXP-1/2/3.

### Construct definitions
- **DDA identity**: `|signals| ≤ n·(N + C(N,2))` — individual views + unordered pairs. Constructive; EXP-2 pending.
- **Meaning compression**: signals/raw-data-unit at prep ≠ bit/weight/activation ratios at inference. Distinct unit pair.
- **TCT composition**: retriever + labeller + `G_τ` guard, all on identical frozen embedder instances.

### Case-status table
| Case | Status | Anchor |
|---|---|---|
| 1 Shakespeare | **MEASURED 2026-04-17** | SFT 1.12 / DPO 0.47 / 98.9% tok-acc; LoRA public |
| 2 Santa | **MEASURED & operational 2026-04-19** | ClipCannon + Phoenix pipeline live; 7-modality 4,044D constellation + EchoMimicV3 LoRA + runtime guard. Paper draft §6.2 tag is paper-internal. |
| 3 Voice | **MEASURED 2026-04-16** | 0.961 WavLM SECS, within-family, N=10, 1 speaker, EN |

---

## PAGE 4 — SUCCESS METRICS & NEXT STEPS

### Win ladders
| Archetype | Minimum | Strong | Exceptional |
|---|---|---|---|
| Researcher | Reply within 7d | Agrees §3.3 fair | Co-authors EXP-1/2/3 |
| Sovereign | Intro accepted | 30-min call | 48h PoC on sovereign corpus |
| Banker | 20-min call | Mandate discussion | Engagement letter review |
| Hyperscaler | Paper read | Technical deep-dive | License + acquihire lane |

### 5 ready next-steps
1. **48-hour PoC** — 1 corpus + 3–13 embedders → DDA count + `G_τ` trace. Data stays.
2. **HF LoRA demo** — live walk-through of Case 1 reproducibility.
3. **Co-author offer** — EXP-1 (ECAPA) / EXP-2 (pairwise-MI) / EXP-3 (nm sweep).
4. **Reference call** — introduce to collapse-cluster researcher who has read §3.3.
5. **Warm-intro request** — ask for named intro into aligned pool (MGX / Qatalyst / MATS).

### Emergency 5-minute prep
1. Re-read 10-second pitch (p.1).
2. Check target row: priority + construct.
3. Pick starter (p.2) matching likely opener.
4. Hold one number: Case 1 SFT 1.12 (research) **or** Case 3 0.961 *within-WavLM-family, N=10, EXP-1 pending* (technical).
5. Scope check: no "refutes", no "SOTA", no unscoped cross-encoder, "up to 91" with hedge.

### Post-conversation
- **2h**: follow-up email with paper + HF LoRA link + CI footer.
- **24h**: log to `memory/journal/`; update target row.
- **48h**: if PoC offered, begin corpus intake; else send concrete next-step.
- **7d**: if silent, send one cite-added nudge.

---

### Scope guards (abort send if broken)
- Case 3 = **within-WavLM-family, N=10, one speaker, English**. EXP-1 pending.
- "**Up to** 91 signals/input" — **constructive upper bound**, EXP-2 pending.
- DDA "**structurally outside**" Shumailov — **scope, not refutation**.
- TCT = **different property class** — **not a benchmark contest**.
- Case 2 = **MEASURED & operational 2026-04-19**. Paper draft §6.2 tag is paper-internal only; for outreach, Case 2 is measured and demonstrable. Subject artefacts withheld.
- Never say: "state of the art", "beats RLHF", "refutes collapse", "cross-encoder verified", Teleox TAM.

---

**Competing Interests**: Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). Methodology contribution from commercial engineering practice.
