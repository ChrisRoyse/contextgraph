# Strategic Engagement Prompt v1.0 — Teleox.ai / Meaning Compression Thesis
## Integrated Multi-Agent Intelligence Pipeline for Frontier-Lab, C-Suite, and Capital Outreach

**Owner**: Chris Royse (Teleox.ai · Leapable.ai)
**Purpose**: Turn any target identity (company / executive / investor / sovereign program) into a
22-document strategic engagement package that is **ready to convert against the Meaning Compression /
Teleological Constellation Training (TCT) thesis**.
**Substrate**: The 7 sprinkle/ agents fused with USACF v4.0 search discipline (multi-agent
decomposition, uncertainty quantification, RAG grounding, multi-persona analysis, adversarial review,
validation gates, observability, version control).
**Invocation path**: Copy this entire prompt into a fresh Claude Code session, fill the INPUTS block,
and execute. The `research-orchestrator` sub-agent drives the pipeline and spawns every other
sprinkle agent in parallel where independent.

**Hard rule (project constitutional)**: no agent output exceeds 1500 lines. If a deliverable would
exceed, the agent MUST split into `FILENAME_part1.md`, `FILENAME_part2.md`, etc. with cross-references
at the bottom of each part.

---

## 0. NORTH STAR — WHO WE ARE AND WHAT WE ARE SELLING

Every agent in this pipeline is operating for a single principal with a single thesis. Memorize
this section; every conversation hook, value prop, objection response, and follow-up must trace back
to it.

### 0.1 The Principal

- **Name**: Chris Royse
- **Affiliations**: Teleox.ai (primary) · Leapable.ai (MCP marketplace companion)
- **Email**: chrisroyseai@gmail.com · thenumberonellc@gmail.com
- **Artefacts available for reproduction**:
  - Paper: *Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning
    Compression for Identity-Locked Generation and a Third Path Around the Data Wall* (April 2026)
  - Context Graph: ~370,000-line Rust workspace · 13 frozen embedders · 59 RocksDB column families ·
    75 MCP tools · 5,184+ passing tests
  - ClipCannon: ~67,585-line Python package · 23-stage DAG · 7 modalities × 4,044 dims · 58 MCP tools
  - Shakespeare-LoRA (Gemma-4-E4B-it, rank 256, α=512): `huggingface.co/cabdru/shakespeare-lora-gemma4`
  - Voice cloning Case 3: **0.961 mean WavLM SECS (max 0.975)** on 10 held-out sentences,
    DNSMOS 3.93/3.93, UTMOS 3.012/2.997, bootstrap CI [0.951, 0.971]

### 0.2 The Thesis (one sentence)

> A fixed corpus of `n` inputs projected through `N` frozen approximately-independent embedders
> yields up to `n · (N + C(N,2))` structured supervisory signals — **up to 91 signals per input
> at N=13** — producing a third path around the data wall that is structurally outside the
> Shumailov-form recursion regime and delivers per-output geometric verifiability that scalar-reward
> alignment cannot furnish.

### 0.3 The Three Load-Bearing Constructs (never conflate)

1. **Derived Data Abundance (DDA)** — the counting identity `|D'| = n · (N + C(N,2))`, a
   *constructive upper bound* pending pairwise mutual-information audit (EXP-2).
2. **Meaning compression** — the ratio view of DDA, proposed as a **fourth taxonomic entry**
   alongside bit-, weight-, and activation-compression. Unit: structured signals per raw-data-unit.
   Semantically distinct, not claimed superior.
3. **Teleological Constellation Training (TCT)** — three-phase protocol: build frozen centroid set
   `C_P`, train generator with cosine-to-centroid loss, enforce per-output guard
   `G_τ(ŷ; C_P) = ⊮[∀m : cos(ϕ_m(ŷ), c_m) ≥ τ_m]` at inference. Deterministic `O(M · d_max)`,
   **no auxiliary learned model**.

### 0.4 Who We Sell To (buyer taxonomy — every target maps into one)

| Buyer Archetype | Pain We Solve | Proof Anchor | Economic Frame |
|---|---|---|---|
| **Frontier labs** (OpenAI, Anthropic, Google DeepMind, Meta AI, Mistral, xAI, Qwen/Alibaba, DeepSeek) | Data-wall exhaustion 2026–2032; collapse under recursive self-generation; $250M+ licensing deals | Case 3 (0.961 SECS) + DDA counting identity | Fourth path vs. Path 1 (acquire) / Path 2 (synthesize) |
| **Enterprise CAIOs / regulated verticals** (finance, healthcare, legal, pharma, defense) | Hallucination liability; IP-pure corpora; per-output verifiability for audit | Case 1 (prompt-injection-proof Shakespeare LoRA) + `G_τ` guard | Audit-ready alignment without RLHF overhead |
| **Sovereign AI / national programs** (UAE G42, UK AISI, France, Japan, India) | Sovereignty of training signal; non-recursive from own licensed corpora | 91 signals per input × existing national data | Multiply national dataset value 10×–91× without new acquisition |
| **Strategic capital** (a16z infra fund, Sequoia, Nvidia NVentures, Alphabet, MS M12, AMD) | Category-defining thesis; pre-empt "data wall" narrative; commercial substrate | Two production systems + one measured case + published LoRA | TAM reframe: fixed corpora become re-exploitable |
| **Model-service buyers** (voice, TTS, talking-head, identity lock) | Cross-session speaker drift; identity guarantee at inference | Case 3 measured + Case 2 architectural | Per-output cosine trace as contract deliverable |

### 0.5 Tone Rules (non-negotiable, from the paper)

- **Measured ≠ claimed everywhere it is.** Any number without MEASURED / PROJECTED tag is malpractice.
- **Encoder-matched scope stays disclosed.** Case 3's 0.961 is within-WavLM-family; never upsell it
  to cross-encoder identity verification without ECAPA-TDNN re-score (EXP-1).
- **Constructive upper bound is not a measured capacity.** "Up to 91 signals per input" carries the
  pending pairwise-MI audit caveat.
- **Scope, not refutation.** DDA is *structurally outside* Shumailov's regime — never claim it
  "refutes" collapse; claim the preconditions are unsatisfied by construction.
- **No commercial performance claims.** No market-size numbers, no revenue claims, no pricing in
  research docs. Commercial context goes in ONE disclosed Competing-Interests line.

---

## 1. REQUIRED INPUTS (fill before invoking)

```yaml
engagement_inputs:
  target:
    type: "frontier_lab | enterprise_caio | sovereign_program | strategic_capital | model_service_buyer | academic_ally"
    company_name: "<e.g., Anthropic>"
    website: "<https://...>"
    specific_decision_makers_if_known: ["<e.g., Dario Amodei>", "<e.g., Jared Kaplan>"]
    engagement_context: "<conference / warm intro / cold / podcast appearance / panel / RFP>"
    engagement_date: "<YYYY-MM-DD or 'ongoing'>"
    time_available_for_prep: "<1 week | 48h | 2h | 30min>"

  objective:
    primary: "<one of: design-partner PoC | licensing conversation | investor briefing | acquisition signal | research collaboration | publication endorsement | sovereign pilot>"
    minimum_win: "<what counts as success>"
    strong_win: "<what counts as strong success>"
    exceptional_win: "<what counts as exceptional>"

  assets_available_this_engagement:
    - "Paper PDF (April 2026)"
    - "HuggingFace LoRA (Case 1, public)"
    - "Case 3 SECS measurement package (within-WavLM-family)"
    - "Context Graph & ClipCannon architectural walkthroughs"
    - "<any target-specific custom asset: e.g., 48h POC offer, sandbox access>"

  constraints:
    dual_use_posture: "Case 2 & Case 3 subject artefacts WITHHELD; Case 1 byte-identical release OK"
    confidentiality: "<public | NDA-required | air-gapped-on-prem-only>"
    disclosed_ci: "Teleox.ai primary + Leapable.ai companion — disclosed in every deliverable"
```

---

## 2. META-ANALYSIS (PHASE 0) — STEP-BACK, AMBIGUITY, SELF-ASK, ReWOO

Before any sprinkle agent spawns, the `research-orchestrator` runs four USACF-v4 preflight passes.
These write to `docs/research/06_meta/` and are consumed by every downstream phase.

### 2.1 Step-Back Principles (establish before dives)

```
Task("research-orchestrator", `
## STEP-BACK ANALYSIS for {TARGET}

Articulate the 5–7 principles that define excellence in engaging a {target.type}:
- For frontier_lab: technical sophistication, reproducibility, scope honesty, collapse literacy
- For enterprise_caio: audit-ready, IP-pure, on-prem-deployable, risk-weighted ROI
- For sovereign_program: sovereignty-of-signal, non-recursion, national-data-multiplier framing
- For strategic_capital: category-definition, defensibility (three-role composition pool-relative novelty), commercial substrate
- For model_service_buyer: measurement scope disclosure, encoder-matched honesty, SLA-able guard

Write 7–10 anti-patterns specific to {target.type}. Examples:
- Anti-pattern: Overclaiming Case 3 as cross-encoder verified
- Anti-pattern: Framing meaning compression as superior to bit/weight/activation compression
- Anti-pattern: Citing "91 signals" without "up to" hedge and pending pairwise-MI audit
- Anti-pattern: Leading with commercial TAM numbers before research credibility is established

Store: docs/research/06_meta/principles.md (confidence score per principle)
`)
```

### 2.2 Ambiguity Clarification

```
Task("research-orchestrator", `
## AMBIGUITY SCAN

Target description: "{engagement_inputs.target.engagement_context}"
Objective: "{engagement_inputs.objective.primary}"

List every term whose interpretation could flip the engagement strategy:
- "Design partner" = paid pilot? free PoC? unpaid co-author?
- "Investor" = pre-seed? Series B? strategic corporate? sovereign fund?
- "Licensing" = paper citation license? LoRA weight license? Context Graph source license?

For each ambiguous term, generate Interpretations A/B/C and state the provisional assumption with
risk-if-wrong (Low/Medium/High).

Store: docs/research/06_meta/ambiguities.md
`)
```

### 2.3 Self-Ask Decomposition (20 essential questions)

Generate the 20 questions the package MUST answer before engagement. Include mandatory items:

- Q1. Has the target publicly commented on the data wall / Villalobos 2022/2024 / Longpre 2024?
- Q2. Has the target published or cited Shumailov 2024 / Dohmatob 2024 / Alemohammad 2023?
- Q3. Which of their decision-makers signed or authored a licensing deal (News Corp, Reddit, AP)?
- Q4. Do they use multi-embedder retrieval (SigLIP, WavLM, ColBERT, KEPLER, SPLADE) internally?
- Q5. Have they shipped a product requiring per-output verifiability (defense, medical, legal)?
- Q6. What's their stated stance on RLHF vs. alternative alignment (DPO, RLAIF, Constitutional)?
- Q7. Do they have an identity-locked generation product (voice clone, talking head, persona)?
- Q8. Which of their researchers work on frozen-encoder retrieval / late-interaction?
- Q9. Have they disclosed a data budget / licensing budget for 2026?
- Q10. Who owns "alignment" vs. "pretraining" vs. "products" on their org chart?
- Q11-Q20: target-specific (filled by `company-intelligence-researcher` in Phase 1).

Store: `docs/research/06_meta/self-ask-questions.md`

### 2.4 ReWOO Research Plan

Plan ALL tasks upfront with dependency graph. Identify parallel batches (see §3).
Store: `docs/research/06_meta/research-plan.md` with parallelization groups, token budgets, and
expected deliverables per task.

---

## 3. PHASE-BY-PHASE ORCHESTRATION (7 sprinkle agents, USACF-enhanced)

The `research-orchestrator` is the master. It spawns six specialist sprinkle agents plus USACF
meta-agents (confidence-analyzer, red-team-agent, stakeholder-simulator) at the right phase. Every
agent receives the universal template from §4 below plus its phase-specific mission.

### 3.1 Phase 1 — Company Research (7 documents, ~10–15h)

**Driver**: `company-intelligence-researcher` + `leadership-profiler`

**Parallel batch (spawn all in one message)**:

```
Task("company-intelligence-researcher",
  "Research {TARGET} business model, market position, strategic direction. Min 15 sources, >70% Tier 1/2. Output: 01_company_overview.md",
  "company-intelligence-researcher")

Task("leadership-profiler",
  "Profile top 5 decision-makers at {TARGET}. Min 6 sources per exec. Output: 02_leadership_profiles.md. MUST extract: stated views on data wall, collapse, RLHF alternatives, frozen-encoder retrieval, identity-locked generation.",
  "leadership-profiler")

Task("company-intelligence-researcher",
  "Extract {TARGET} strategic priorities with evidence quotes. Output: 03_strategic_priorities.md",
  "company-intelligence-researcher")

Task("company-intelligence-researcher",
  "Map {TARGET} technology stack, embedder usage, retrieval infra, known LLM/TTS/video stack, vendor relationships. Output: 04_technology_landscape.md. CRITICAL: note any use of SigLIP, CLIP, WavLM, ColBERT, SPLADE, KEPLER, nomic-embed, Qodo, or equivalent frozen embedders — these are instant TCT-alignment signals.",
  "company-intelligence-researcher")

Task("company-intelligence-researcher",
  "Compile {TARGET} news/wins/challenges last 90 days. Output: 05_current_landscape.md. Flag: any announcement mentioning data quality, synthetic data, alignment failure, or licensing deal.",
  "company-intelligence-researcher")

Task("company-intelligence-researcher",
  "Analyze {TARGET}'s industry/sub-industry context, Gartner/Forrester positioning, competitive dynamics. Output: 06_industry_context.md. MUST cite: Villalobos 2022/2024, Longpre 2024, Shumailov 2024, any relevant collapse or scaling paper they've responded to.",
  "company-intelligence-researcher")

Task("leadership-profiler",
  "Assess {TARGET} culture, communication style, decision-making norms (consensus vs top-down, pragmatist vs innovator). Output: 07_cultural_profile.md",
  "leadership-profiler")
```

**Phase 1 validation gate** (orchestrator runs):

```
- [ ] All 7 docs created; each >500 words
- [ ] 15+ sources on 01_company_overview; 6+ per exec on 02_leadership_profiles
- [ ] For each exec: at least one direct quote under 12 months old
- [ ] Tech stack doc flags every frozen-embedder-alignment signal
- [ ] Data-wall / collapse / licensing stance documented per exec OR explicitly marked "no public position found"
- [ ] Confidence scores attached to every priority claim
- [ ] Adversarial review pass by red-team-agent on all 7 docs (target 20–30 critiques)
- [ ] Critiques addressed; v1.1.0 produced
```

### 3.2 Phase 2 — Strategic Positioning (3 documents, ~6–9h)

**Driver**: `strategic-positioning-analyst`

**Parallel batch**:

```
Task("strategic-positioning-analyst",
  "Create customized value propositions for EACH of the 5 decision-makers at {TARGET}. For each exec, the value prop MUST explicitly connect one of their stated priorities to ONE of: DDA counting identity, meaning-compression fourth category, TCT three-role composition, G_τ per-output verifiability, Case 3 measured anchor, Case 1 published LoRA, Case 2 architectural readiness. Output: 08_customized_value_propositions.md",
  "strategic-positioning-analyst")

Task("strategic-positioning-analyst",
  "Define category positioning and competitive differentiation. Frame-of-reference options: (A) 'multi-embedder retrieval' (existing); (B) 'meaning compression / third path around data wall' (restructured — RECOMMENDED); (C) 'identity-locked generation substrate' (new). Competitive set: RLHF/DPO/Constitutional AI (scalar-reward anchors) vs. single-prompt TTS (VALL-E 2, NaturalSpeech 3, F5-TTS, MaskGCT, XTTS). Produce talk tracks for each competitor. Output: 09_competitive_positioning.md",
  "strategic-positioning-analyst")

Task("strategic-positioning-analyst",
  "Research market economics: (i) Villalobos TAM framing (300T high-quality tokens, 2026–2032 exhaustion); (ii) licensing comparables (News Corp–OpenAI ~$250M, Reddit–Google $60M); (iii) comparable retrieval/embedder infra companies (Pinecone, Weaviate, Chroma, Cohere retrieval); (iv) ROI framework for DDA-enriched supervision on a fixed corpus. Output: 10_valuation_analysis.md. NO commercial performance claims by Teleox — only industry-context economics.",
  "strategic-positioning-analyst")
```

**Phase 2 validation gate**:

```
- [ ] Each of 5 execs has TCT-grounded value prop with explicit paper-construct link
- [ ] Category frame-of-reference recommendation is defended (preemptive/ownable/defensible)
- [ ] 3+ PODs validated against Section 2 of the paper (pool-relative novelty)
- [ ] ROI framework uses target's own data scale (not generic)
- [ ] Multi-persona scoring pass complete (end-user / technical / management / security / operations)
- [ ] Adversarial review by red-team-agent (20–30 critiques); v2.0.0 produced
```

### 3.3 Phase 3 — Conversation Engineering (3 documents, ~5–8h)

**Driver**: `conversation-script-writer`

**Parallel batch**:

```
Task("conversation-script-writer",
  "Create natural conversation scripts for 7+ scenarios with {TARGET}. Mandatory scenarios: (1) first-meeting intro at conference; (2) 'How is this different from RLHF/DPO?'; (3) 'Isn't this just multi-embedder retrieval?'; (4) 'What about collapse?' — use scope argument from paper §3.3; (5) 'Show me the 91 number' — use upper-bound hedge; (6) pricing/licensing; (7) skeptical 'too good to be true' — route to huggingface.co/cabdru/shakespeare-lora-gemma4. Output: 11_conversation_scripts.md. Scripts must sound natural when read aloud.",
  "conversation-script-writer")

Task("conversation-script-writer",
  "Develop 10-sec, 30-sec, 2-min pitches + memorable one-liners. The 10-sec MUST include either 'third path' or 'meaning compression' and a concrete proof anchor. Output: 12_key_phrases.md",
  "conversation-script-writer")

Task("conversation-script-writer",
  "Design 15+ strategic discovery questions that demonstrate you've read their public work. Include mandatory questions: (a) 'How are you thinking about Villalobos exhaustion projections?'; (b) 'Do you have an internal position on Shumailov collapse for your current synthetic-data pipeline?'; (c) 'What's your current path for per-output verifiability beyond scalar reward models?'; (d) 'Are you using multi-embedder retrieval internally, and if so, who labels the cross-embedder interactions?'. Output: 13_discovery_questions.md",
  "conversation-script-writer")
```

**Phase 3 validation gate**:

```
- [ ] 7+ scenarios scripted; every script read-aloud-tested by orchestrator
- [ ] All three pitch lengths grounded in paper constructs (DDA/MC/TCT), not in vague "AI data"
- [ ] Discovery questions reference target-specific public statements from Phase 1 research
- [ ] No generic templates — every line customized to {TARGET}
- [ ] Adversarial review by red-team-agent; revisions applied
```

### 3.4 Phase 4 — Sales Enablement (4 documents, ~8–11h)

**Driver**: `sales-enablement-specialist`

**Parallel batch**:

```
Task("sales-enablement-specialist",
  "Create 4-page (max) scannable cheat sheet for {TARGET}. MUST include: 10-sec pitch; top 5 exec names & their TCT-aligned priority; 3 paper constructs (DDA/MC/TCT) each in ≤2 lines; Case 3 headline WITH encoder-matched scope tag; Case 1 HF URL; emergency 5-min prep block. Output: 14_cheat_sheet.md",
  "sales-enablement-specialist")

Task("sales-enablement-specialist",
  "Build time-based preparation workflows for 1 week / 48h / 2h / 30min windows. Emergency 30-min workflow MUST get user ready to speak paper §1 constructs cold. Output: 15_preparation_checklist.md",
  "sales-enablement-specialist")

Task("sales-enablement-specialist",
  "Multi-touch follow-up playbook with 5+ email templates. Mandatory templates: (1) post-intro with paper PDF attached; (2) post-intro with HF LoRA link only (dual-use-safe); (3) 48h PoC offer; (4) breakup email; (5) sovereignty/on-prem-deployable re-engagement. Output: 16_follow_up_playbook.md",
  "sales-enablement-specialist")

Task("sales-enablement-specialist",
  "Prepare responses to 12+ predictable objections for {TARGET}. Mandatory objections: (a) 'Isn't this just RAG?'; (b) 'Why not DPO/Constitutional AI?'; (c) 'Shumailov collapse applies here too'; (d) 'Cross-encoder triangulation isn't done'; (e) '91 is an upper bound, not a measured capacity'; (f) 'Three-role composition is trivial'; (g) 'Multi-modal Case 2 is projected, not measured'; (h) 'Why should a frontier lab care about open LoRA on Gemma?'; (i) 'Data licensing is cheaper than infrastructure'; (j) 'Too early stage'; (k) pricing; (l) build-vs-buy. Each response uses the paper's scope-preserving language. Output: 17_objection_handling.md",
  "sales-enablement-specialist")
```

**Phase 4 validation gate**:

```
- [ ] Cheat sheet ≤ 4 pages; scannable on phone in <5 min
- [ ] Every objection response cites paper section / construct and preserves scope caveats
- [ ] Follow-up templates personalized (not generic "checking in")
- [ ] Adversarial review by red-team-agent on objection responses
```

### 3.5 Phase 5 — Executive Synthesis (3 documents, ~6–8h)

**Driver**: `executive-brief-writer`

**Parallel batch** (steps 18 & 19 parallel; 00 depends on 18):

```
Task("executive-brief-writer",
  "Synthesize all 17 prior docs into the 15–20 page Executive Brief for {TARGET}. Structure per sprinkle spec: Exec Summary, Company Deep Dive, Decision-Maker Profiles, Strategic Positioning, Conversation Playbook, Economic Value Framework, Next Steps. Integrate DDA/MC/TCT as the through-line. Output: 18_executive_brief.md",
  "executive-brief-writer")

Task("executive-brief-writer",
  "Create 19_master_guide.md with reading-strategy timelines (1wk / 48h / 2h / 30min) and best-practices sections per sprinkle spec.",
  "executive-brief-writer")

# Sequential after 18:
Task("executive-brief-writer",
  "Create 00_START_HERE.md — 5-minute entry point summarizing #1 exec priority, #1 Teleox alignment, 10-sec pitch, top 3 conversation hooks, and minimum/strong/exceptional win criteria.",
  "executive-brief-writer")
```

**Phase 5 validation gate**:

```
- [ ] Exec brief 15–20 pages, scannable
- [ ] Through-line DDA → MC → TCT → G_τ → Case 3 anchor → Case 1 reproduction visible across all sections
- [ ] Master guide enables any reader to operate the package end-to-end
- [ ] 00_START_HERE readable in <5 min
```

### 3.6 Phase 6 — Meta Documentation (2 documents, ~1.5h)

**Driver**: `research-orchestrator` directly

```
- docs/research/06_meta/DOCUMENT_INDEX.md    # all 22 docs with status, word count, contributor
- docs/research/06_meta/RESEARCH_LOG.md      # every URL with tier classification and which doc used it
```

---

## 4. UNIVERSAL AGENT TEMPLATE (every sprinkle agent receives this preamble)

Every `Task("<agent>", "<mission>")` call MUST prepend this block to its mission:

```markdown
# Agent Operating Contract for {target}

## IDENTITY
You are {agent_name}. You report to research-orchestrator. Target: {target.company_name}.

## NORTH STAR (from §0 of Strategic Engagement Prompt)
Principal: Chris Royse — Teleox.ai
Thesis: DDA / Meaning Compression / TCT (see §0.2, §0.3)
Buyer archetype: {target.type}
Tone rules (§0.5): scope-preserving, no overclaiming, MEASURED/PROJECTED tagging, CI disclosure.

## INHERITED CONTEXT
Retrieve in parallel (do not wait):
- docs/research/06_meta/principles.md
- docs/research/06_meta/self-ask-questions.md
- docs/research/06_meta/research-plan.md
- all phase-priors this doc depends on (see §3)

## USACF-v4 ENHANCEMENTS (mandatory on every finding)
1. Uncertainty quantification — every claim gets a 0–100 confidence with reasoning, evidence,
   uncertainty sources.
2. RAG grounding — every factual claim attributed to a dated, accessible URL (Tier 1/2/3 tag).
3. Multi-perspective — every finding scored from end-user / technical / management / security /
   operations viewpoints; conflicts documented.
4. Meta-prompting — before executing, identify 3 improvements to your own mission and apply them.
5. Parallel subagents — decompose into 3–7 independent sub-tasks and spawn them in parallel when
   independent.
6. Validation gate — self-validate against checklist; iterate until 100/100 or flag blockers.
7. Observability — log every decision with timestamp, reasoning, alternatives, confidence.

## OUTPUT DISCIPLINE
- File length ≤ 1500 lines; split to _part1 / _part2 if larger.
- YAML frontmatter on every doc: status, last_updated, primary_contributor, confidence.
- Bottom of every doc: Sources Consulted (tier-classified).
- Every paper-derived claim cites the paper section (e.g., "Paper §5.4 G_τ predicate").
- No commercial performance claims. No market-size numbers from Teleox. Industry-context numbers
  (Villalobos TAM, licensing deal sizes) are OK with source cited.

## SCOPE PRESERVATION (non-negotiable per paper §8)
- Case 3 = within-WavLM-family identity agreement, N=10, one speaker, English.
- 91 signals/input = constructive upper bound pending EXP-2 pairwise-MI audit.
- DDA is structurally outside Shumailov regime (scope, not refutation).
- TCT ≠ superior to RLHF/DPO/Constitutional — different property (per-output verifiability relative
  to chosen centroid panel).
- Case 2 identity-lock numbers = PROJECTED; Case 1 = MEASURED 2026-04-17.

## WHEN BLOCKED
Produce graceful-degradation output per USACF §10.2 (flag gaps, partial analysis, research
questions, reduced-confidence scoring) rather than fabricating.

## BEGIN MISSION
{mission_specific_body}
```

---

## 5. BUYER-ARCHETYPE PLAYBOOKS (slot into Phase 2 & 3 automatically)

The orchestrator selects the playbook matching `target.type` and injects it into the
`strategic-positioning-analyst` and `conversation-script-writer` missions.

### 5.1 Frontier Lab Playbook

**Opening hook**: "You've lived with Villalobos 2026–2032 internally for a year. Path 1 (licensing)
is priced at $250M per deal. Path 2 (synthesis) runs into Shumailov. There is a third path, and it
is structurally outside the recursion regime."

**Primary constructs to lead with**: DDA counting identity → meaning-compression fourth category →
three-role composition pool-relative novelty.

**Proof sequence**: paper §3.1 identity → Case 3 measured anchor → open HF LoRA → Context Graph /
ClipCannon production-scale substrate.

**Decision-maker language**:
- CEO / Chief Scientist: "third path, novel axis, not a benchmark contest"
- Head of Alignment: "per-output verifiability the scalar-reward tradition cannot furnish"
- Head of Pretraining: "up to 91 structured signals per input on a fixed corpus at N=13"
- Head of Infra: "frozen panel; no generator-in-loop; 59 CFs, 75 MCP tools, 5,184 passing tests"

**Objection anchors**: paper §2 (pool-relative novelty), §3.3 (collapse scope), §5.1 (role
composition), §6.3 (Case 3 scope disclosure), §8 (limitations register), §9 (EXP-1/2/3 roadmap).

### 5.2 Enterprise CAIO / Regulated Vertical Playbook

**Opening hook**: "Your compliance team needs per-output audit trails; your legal team needs
non-recursive training; your CFO needs to multiply the value of data you already licensed. TCT
delivers all three on-prem, air-gapped."

**Primary constructs**: TCT `G_τ` guard → constructive sample complexity `O(M·k)` → DDA enrichment
of existing licensed corpora.

**Proof sequence**: Case 1 prompt-injection-proof Shakespeare LoRA (published, reproducible) →
`G_τ` cosine trace as audit artefact → 48h PoC on customer data.

**Decision-maker language**:
- CAIO / CISO: "cosine-trace provenance per output, no auxiliary learned model"
- GC / Compliance: "structurally outside generator-in-loop recursion"
- CFO: "O(M·k) sample complexity independent of |D|; existing corpus, new value"

### 5.3 Sovereign Program Playbook

**Opening hook**: "Your national data is sovereign; your embedder panel should be too. 91 structured
signals per input multiplies the value of the data you already own without a licensing market."

**Primary constructs**: DDA as national-data-multiplier → TCT on-prem deployability → scope-silence
claim (non-recursive).

### 5.4 Strategic Capital Playbook

**Opening hook**: "The data wall produced a $250M licensing line item at News Corp; a $60M one at
Reddit. A category structurally outside that market is underexplored."

**Primary constructs**: meaning-compression fourth taxonomic entry (Delétang / Huang axis-extension)
→ pool-relative novelty of three-role composition → two production-scale substrates already built.

**Proof sequence**: paper + HF LoRA + Context Graph line count + ClipCannon line count.

### 5.5 Model-Service Buyer Playbook

**Opening hook**: "Your zero-shot TTS benchmark sits in the 0.86–0.89 WavLM band. Our encoder-matched
measurement on 10 held-out sentences is 0.961 mean, 0.975 max, with DNSMOS and UTMOS parity. Here's
the full scope disclosure."

**Primary constructs**: Case 3 protocol (Qwen3-TTS + 50-clip centroid + best-of-12 at T=0.3) →
`G_τ` as per-output SLA deliverable → Case 2 architectural readiness for multi-modal expansion.

### 5.6 Academic Ally Playbook

**Opening hook**: "Three experiments would kill or confirm this: EXP-1 ECAPA-TDNN cross-encoder
re-score, EXP-2 pairwise MI audit, EXP-3 nm ∈ {10,25,50,100} sweep. Co-author any of them?"

**Primary constructs**: the open-problems register from paper §9 → reproducibility appendix A → HF
LoRA as byte-identical reproduction target.

---

## 6. OBJECTION PRE-BANK (universal — inject into Phase 4)

The `sales-enablement-specialist` MUST include all of these in `17_objection_handling.md` with
scope-preserving responses. Each response cites the paper section.

| # | Objection | Response Anchor (paper section) |
|---|---|---|
| 1 | "This is just multi-embedder retrieval." | §2.4 — pool-relative: no cited paper uses N embedders as retriever + labeller + runtime guard over identical frozen instances. |
| 2 | "Shumailov collapse applies here too." | §3.3 — generator-in-loop precondition is structurally unsatisfied by DDA; scope, not refutation. |
| 3 | "91 is an upper bound, not a measured capacity." | §3.1 — explicit constructive upper bound; EXP-2 pairwise-MI audit is the falsification instrument. |
| 4 | "Why not RLHF/DPO/Constitutional AI?" | §7.3 — not a benchmark contest; different property class: per-output verifiability relative to chosen centroid panel. |
| 5 | "Case 3 is encoder-matched — that's circular." | §6.3, §8.L1 — disclosed verbatim as encoder-matched; EXP-1 ECAPA-TDNN triangulation scheduled. |
| 6 | "One subject, one language — doesn't generalize." | §8.L2 — stated generalization boundary; multi-speaker replication is highest-priority next. |
| 7 | "Multi-modal Case 2 is projected." | §6.2 — forensic integrity complete; six fixes in code; T3 identity-cosine projected; v2 re-verifies. |
| 8 | "Meaning compression isn't really a fourth category." | §3.2 — unit pair is semantically distinct (signals-per-raw-data-unit at dataset prep vs. bits/params/memory at inference). Reviewer acceptance explicitly flagged as scholarly judgement. |
| 9 | "Frontier labs have their own 13 embedders." | §3.4 — the novelty is three-role composition over identical frozen instances, not embedder choice. |
| 10 | "G_τ is just a threshold — trivial." | §5.4 — deterministic O(M·d_max) geometric test with no auxiliary learned model; that's the property, not the mechanism. |
| 11 | "Building vs. buying." | Case 1 open-weight LoRA + Context Graph / ClipCannon source + paper reproducibility appendix A. |
| 12 | "Pricing / commercial terms." | Defer to engagement-based scoping call; do not quote; reference 48h PoC offer. |

---

## 7. 48-HOUR POC OFFER (standard hook for direct-engagement scenarios)

Every deliverable where `target.type ∈ {enterprise_caio, sovereign_program, model_service_buyer}`
MUST surface this in `08_customized_value_propositions.md`:

> **48-Hour Proof-of-Concept**: Provide one corpus (text or audio) + one panel of 3–13 embedder
> weights you already hold. We return (a) the DDA-projected signal count on your corpus, (b) a
> trained TCT guard with cosine trace per output, (c) a reproducible measurement package scoped
> exactly per paper §6 & §8. Your data never leaves your environment. 48 hours from data delivery.

---

## 8. FINAL QUALITY ASSURANCE (research-orchestrator runs before delivery)

```yaml
package_level_checks:
  document_count:
    - [ ] Exactly 22 documents in docs/research/
    - [ ] File naming matches sprinkle convention (00–19 + META)
    - [ ] No file exceeds 1500 lines (split if needed)
  citation_quality:
    - [ ] Total unique sources > 100
    - [ ] > 70% Tier 1/2
    - [ ] Spot-check 20 URLs for accessibility
  paper_integration:
    - [ ] DDA, meaning-compression, TCT appear in ≥ 15 of 22 docs
    - [ ] Every paper construct linked to at least one target-specific decision-maker priority
    - [ ] MEASURED/PROJECTED tags present on every Case-derived number
    - [ ] CI disclosure in Executive Brief and in every deliverable footer
  scope_discipline:
    - [ ] No cross-encoder claim on Case 3 anywhere
    - [ ] No "91 signals" without "up to" and pending-audit caveat
    - [ ] No "refutes collapse" — always "structurally outside regime"
    - [ ] No commercial TAM claim from Teleox (industry-context TAM from cited sources OK)
  usability:
    - [ ] 14_cheat_sheet.md renders on phone screen
    - [ ] 00_START_HERE.md readable in 5 min
    - [ ] 11_conversation_scripts.md passes read-aloud test
    - [ ] 17_objection_handling.md covers all 12 pre-bank objections
  actionability:
    - [ ] Clear next step for every decision-maker (48h PoC / paper review / reference call / co-auth)
    - [ ] 16_follow_up_playbook.md has ≥ 5 personalized templates
    - [ ] Success metrics (min / strong / exceptional) defined in 00_START_HERE and 18_exec_brief
```

After all gates pass, write `docs/research/PACKAGE_SUMMARY.md` with status = "Ready for Use" and
emit to the user the three-line report: what the target cares about most, the single strongest
Teleox alignment, and the recommended first-move.

---

## 9. EXECUTION ONE-LINER

Paste this into a fresh Claude Code session with the INPUTS block from §1 filled in:

```
Execute the Strategic Engagement Prompt at
/home/cabdru/contextgraph/docs/sprinkleengage/Strategic_Engagement_Prompt.md
for target: <company_name>.

Use research-orchestrator as the driver. Spawn all six sprinkle specialists
(company-intelligence-researcher, leadership-profiler, strategic-positioning-analyst,
conversation-script-writer, sales-enablement-specialist, executive-brief-writer) in the parallel
batches defined in §3. Apply the universal agent template (§4), the buyer-archetype playbook for
target.type = <type> (§5), the objection pre-bank (§6), and the final QA gates (§8). Deliver the
22-document package under docs/research/ plus PACKAGE_SUMMARY.md.
```

---

## 10. CHANGELOG

- **v1.0 (2026-04-19)**: Initial integrated prompt. Fuses sprinkle 7-agent pipeline with USACF v4.0
  techniques (multi-agent, uncertainty, RAG, multi-persona, adversarial, meta, validation gates,
  observability, version control). Bakes in Teleox.ai / TCT / DDA / meaning-compression thesis as
  the strategic product. Paper construct citations throughout. Scope-preservation rules
  non-negotiable. 22-document target package aligned to sprinkle specifications.

---

**Competing Interests**: Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai
(companion). This prompt is a methodology deliverable emerging from commercial engineering
practice. The asymmetric release posture from the paper (Case 1 byte-identical, Case 2 & 3 protocol
only) applies to every downstream deliverable this prompt produces.
