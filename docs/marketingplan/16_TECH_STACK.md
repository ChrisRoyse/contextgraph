# 16 — Tech Stack

> The concrete build-vs-buy + integration map. Built on LangGraph for orchestration, RocksDB-backed persistence, Markifact / Hyper MCP / Windsor for marketing-platform integrations, Synapse for computer-use, **leapable.ai for queryable expert-knowledge vaults**, the Anthropic Claude SDK for primary inference, fine-tuned classifiers for the brand/compliance gates. The thesis: agents have **all the tools they need** so they can reason and execute.

---

## §1. The stack at a glance

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         OPERATOR INTERFACE                                │
│  Claude Code / Codex CLI / Web Dashboard / Slack / Email digest          │
└────────────────────────────────┬─────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                        ORCHESTRATION LAYER                                │
│  LangGraph (durable execution, checkpointing, HITL, state graph)         │
│  + custom OuterLoop (ASSESS → PLAN → BUILD → COMMUNICATE)                │
│  + ROMA-style recursive control (Atomizer / Planner / Executors / Aggr.)  │
└────────────────────────────────┬─────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                       AGENT FLEET (~26 classes)                           │
│  Each class is a LangGraph subgraph w/ its own prompt, tool surface,     │
│  evaluator, autonomy default, model routing.                              │
└────────────────────────────────┬─────────────────────────────────────────┘
                                 │
       ┌─────────────────────────┼──────────────────────────┐
       ▼                         ▼                          ▼
┌──────────────┐         ┌──────────────────┐      ┌──────────────────┐
│ LLM Inference │         │  Memory Layer    │      │  Tool Gateway    │
│ Anthropic     │         │  (LLM Wiki)      │      │  (MCP + custom)  │
│  Claude Opus  │         │  - skills        │      │                  │
│  Claude Sonnet│         │  - reflections   │      │                  │
│  Claude Haiku │         │  - channel cards │      │                  │
│ Fine-tuned    │         │  - brand voices  │      │                  │
│  Gemma 4B     │         │  - contact graph │      │                  │
│ Local 7B as   │         │                  │      │                  │
│  fallback     │         │  Backed by:      │      │                  │
└──────────────┘         │  - RocksDB CFs   │      │                  │
                         │  - filesystem MD │      │                  │
                         │  - leapable.ai   │      │                  │
                         │    expert vaults │      │                  │
                         └──────────────────┘      └──────────────────┘
                                                            │
            ┌───────────────────────────────────────────────┼───────────────┐
            ▼                                               ▼               ▼
┌──────────────────────┐                       ┌──────────────────┐  ┌────────────────┐
│ Marketing MCPs       │                       │ Computer-Use     │  │ Bespoke        │
│ - Markifact (300+)   │                       │ - Synapse        │  │ adapters       │
│ - Hyper MCP (200+)   │                       │ - Playwright     │  │ - GitHub gh CLI│
│ - Windsor (325+)     │                       │ - Browser-Use    │  │ - gh API       │
│ - 0nMCP (995+)       │                       │ - OpenClaw       │  │ - Anthropic SDK│
│ - SearchAtlas (112)  │                       │                  │  │ - OAuth flows  │
│ - AgentKits          │                       │                  │  │                │
└──────────────────────┘                       └──────────────────┘  └────────────────┘
       │                                              │                      │
       └─────────────────── External World ───────────┴──────────────────────┘
                   (ads, social, CRM, email, web, games, etc.)
```

---

## §2. Orchestration — why LangGraph

Per the 2026 multi-agent orchestration framework comparison:

> *"LangGraph is the most production-ready orchestration framework. State persistence is native, error recovery is declarative, and the graph visualiser makes debugging tractable."*

| Capability | Why we need it |
|---|---|
| Durable execution + checkpointing | Goals run for months; daemon must survive restarts and API outages |
| Native human-in-the-loop interrupts | Approval queue patterns are first-class |
| State graph that's debuggable | Multi-month traces need a visualizer |
| Token overhead +9% (best in class) | Cost discipline |
| LangSmith integration | Observability + replay |

The OuterLoop is implemented as a LangGraph StateGraph with nodes for `assess`, `plan`, `atomize`, `dispatch`, `aggregate`, `evaluate`, `communicate`. Each Executor is a sub-graph that LangGraph dispatches in parallel.

**Alternative considered:** CrewAI (faster prototype but weaker production observability) and AutoGen / MS Agent Framework 1.0 (conversational debate model — wrong fit for our deterministic-pipeline use).

---

## §3. LLM model routing

Per `04_SYSTEM_ARCHITECTURE.md §10.3`:

| Task | Model | Why |
|---|---|---|
| Planner, Reflector, Aggregator, Evaluator (M3-M9) | Claude Opus 4.7 (or current frontier) | High-stakes reasoning |
| Content Writer (P1), Outreach Drafter (D4) | Claude Sonnet 4.6 (or current standard) | Quality + cost balance |
| Brand-voice classifier, compliance check | Fine-tuned Gemma 4B | <50ms; low cost; deterministic |
| Schema validation, simple extraction | Claude Haiku 4.5 | Cheapest tier |
| Hallucination check (reverse citation) | Sonnet + web tool | Verification |
| Influence Mapper graph queries | Sonnet | Graph traversal + structured output |
| Embeddings (LLM Wiki + leapable) | gte-multilingual-base | Vault-compatible |

Routing is a **mechanical wrapper** around the LLM call site, not an agent decision. Decisions like "send to Opus" are encoded per agent-class card (`05_AGENT_FLEET_DESIGN.md §2`).

---

## §4. Memory — the LLM Wiki + leapable.ai

The memory layer is the single biggest leverage point. Borrowed from Karpathy's LLM Wiki gist + the Yopedia pattern + extended with **leapable.ai vault-as-a-service**.

### 4.1 Three memory surfaces

| Surface | Backed by | Used for |
|---|---|---|
| **Project Wiki** (operational state) | RocksDB CFs + markdown files | Channel cards, brand voices, contact graph, reflections, skills, wins/losses |
| **Goal Memory** (per active goal) | RocksDB CFs (CF_GOALS, CF_GOAL_CYCLES, CF_EXPERIMENT_NODES, CF_REFLECTIONS, CF_AUDIT_LOG) | Per-goal state, durable across restarts |
| **Expert Vaults** (queryable knowledge) | **leapable.ai** | Externalized expert knowledge: Hormozi corpus, Karpathy corpus, Tier-3 contact corpus, competitor research |

### 4.2 leapable.ai integration

**Why leapable.ai matters here:** the operator already maintains domain-expert vaults on leapable.ai (e.g., `AlexHormoziTranscripts` with 1,150 documents / 6,428 chunks of Hormozi's complete recorded content). These vaults are **directly queryable by the agent fleet** via the leapable MCP server (`mcp__leapable__leapable_search` and friends).

**Operational pattern:**

The agent fleet queries the relevant expert vault before planning. Example:
- Before drafting a Grand Slam Offer for Leapable, P4 (Brief Generator) queries `AlexHormoziTranscripts` for Hormozi's offer-structure principles, then drafts the brief grounded in primary-source Hormozi material.
- Before launching a podcast tour for Polis, D8 (Podcast Booker) queries any operator-curated `PodcastingPlaybook` vault for tactical patterns and host-relationship etiquette.
- Before structuring a viral loop, O3 (Referral Engineer) queries the Hormozi vault for the Skool.com two-prize-giveaway worked example.

**Required vaults (operator builds these once; agents query forever):**

| Vault | Sources | Used by |
|---|---|---|
| `AlexHormoziTranscripts` (exists) | 1,150 docs — `$100M Offers`, `$100M Leads`, `$100M Money Models`, ~1000+ YouTube transcripts | All agents that touch positioning, copy, offers, referrals, scaling |
| `KarpathyCorpus` | All Andrej Karpathy talks, blog posts, AutoResearch repo, AgentHub repo, Software-3.0 keynote | Architecture decisions, agent-design discussions |
| `OperatorBrandVoices` | Every operator-approved piece of content, every operator-rejected piece, every operator correction | P1/P2 brand voice grounding; evaluator calibration |
| `CompetitiveIntel` | Public docs, blog posts, talks from named competitors | S5 Competitive Intelligence |
| `Tier3ContactProfiles` | Public-facing content from operator's Tier-3 target list — talks, papers, posts | Influence Network Mapper (S3), Pre-Call Briefer |
| `ProductTechSpecs` | Leapable, Synapse, Polis full documentation (CLAUDE.md, etc.) | P2 Technical Writer, accuracy grounding for every claim |
| `DevRelLiterature` | Open-source DevRel playbooks (FORKOFF, 5W, etc.) | D2, D3, D8 |
| `ComplianceCorpus` | CAN-SPAM text, GDPR text, CCPA text, CASL text, EU AI Act text, platform ToS docs | Compliance Check at the Guardrail Engine |
| `LeapableVaultMeta` | Internal vault about how to use leapable vaults effectively | Wiki Gardener M10 |

**Tool surface from the leapable MCP server** (already loaded — see the tools loaded in the session): `leapable_search`, `leapable_db_select`, `leapable_search_cross_db`, `leapable_document_get`, `leapable_document_page`, `leapable_chunk_context`, `leapable_image_search`, `leapable_marketplace_creators`, `leapable_prompt_search`, and ~120 more.

**Per-agent vault access policy:**

Each agent class card (`05_AGENT_FLEET_DESIGN.md §2`) lists which vaults that agent is allowed to query. The Tool Gateway enforces — an agent can't query a vault it isn't authorized for. Default: agents query only the vaults relevant to their specialty.

**Why not just RAG into our own vector store?** Two reasons:
1. Leapable's per-vault management UI, hybrid BM25+semantic+reranker pipeline, and provenance chain are operationally already-built and operator-curated.
2. The operator can publish vaults via the leapable marketplace if they want to monetize a domain-expert knowledge artifact — the same vault that powers the fleet can power a SaaS line.

---

## §5. Tool Gateway — the MCP-centric universe

Per `04_SYSTEM_ARCHITECTURE.md §9`, the Gateway is the single path to external systems. It exposes a unified MCP-style interface backed by:

### 5.1 Marketing-platform MCPs (preferred for any platform they cover)

| MCP server | Coverage | Primary use |
|---|---|---|
| **Markifact** | 300+ operations across Google Ads, Meta Ads, TikTok Ads, LinkedIn Ads, Microsoft Ads, Reddit Ads, Pinterest Ads, Snapchat Ads, Amazon Ads, DV360, GA4, BigQuery, Google Search Console, Google Business Profile, Google Merchant Center, Facebook, Instagram, LinkedIn, Shopify, HubSpot, Klaviyo, WhatsApp, Slack | Paid media + organic social + CRM + analytics from one MCP |
| **Hyper MCP** | 200+ integrations, 40+ built-in tools, 100+ skills covering paid media, SEO, content, social, research | Comprehensive — Reddit/X/Insta/YT/TikTok scrapers; Firecrawl; image+video gen |
| **Windsor MCP** | 325+ data sources (live data) — Meta Ads, Google Ads, TikTok Ads, GA4, HubSpot, Salesforce, Shopify, Stripe, BigQuery, Snowflake, QuickBooks, Xero | Data-warehouse-style query across marketing data |
| **SearchAtlas Agentic Omnichannel Marketing MCP** | 112 tools — SEO/GEO/Google Ads/local SEO/GMB/digital PR/website creation | Generative Engine Optimization specifically |
| **0nMCP** | 995+ tools, 55 services with one-server-every-API claim | Fallback for long-tail integrations |
| **leapable.ai MCP** | Vault search + management, marketplace | Memory layer (§4.2) |
| **GitHub MCP (gh CLI + native MCP)** | Repos, issues, PRs, releases, sponsors | D3 GitHub Maintainer |
| **Gmail / MS Graph MCP** | Email, Calendar | D4 Outreach Drafter, D7 Newsletter Pitcher, M9 Communicator |
| **Synapse MCP** | Computer-use on operator's desktop | D2 Community Operator on platforms without APIs |

### 5.2 Computer-use adapters

| Adapter | When used |
|---|---|
| **Synapse** | Operator-account flows on platforms where API is gated / nonexistent. See `06_COMPUTER_USE_PLAYBOOK.md`. |
| **Playwright** | Parallel read-only scraping of public surfaces (non-operator accounts; residential IP rotation). |
| **Browser-Use / OpenClaw** | Goal-driven browser automation for interactive flows (partnership inquiry forms, etc.) |

### 5.3 Bespoke adapters

Where neither MCP nor computer-use is the right tool, custom adapters:

- Anthropic SDK (direct LLM access; primary inference path)
- OpenAI SDK (fallback / model comparison)
- Direct PostgreSQL / RocksDB (product database readback for SoT)
- Email-deliverability vendor SDKs (Mailgun, SendGrid, Postmark) when sending at scale outside Gmail/MS Graph

---

## §6. Data layer

### 6.1 RocksDB schema (marketing-platform-specific CFs)

Per `04_SYSTEM_ARCHITECTURE.md §6`:

```
CF_GOALS                  — active and historical goal specs (JSON)
CF_GOAL_CYCLES            — every assess→plan→build→communicate cycle, with timestamps + diffs
CF_EXPERIMENT_NODES       — DAG of experiments per goal
CF_APPROVAL_QUEUE         — HITL items
CF_REFLECTIONS            — Reflexion memory
CF_WIKI_INDEX             — Index for filesystem-backed wiki pages
CF_CONTACTS               — Contact graph + tier classifications
CF_AUDIT_LOG              — Append-only event log
CF_CHANNEL_STATE          — Per-channel rate-limit buckets, posting history hashes
CF_PERSONA_HISTORY        — Per-persona publishing history (for dedup)
CF_ATTRIBUTION_GRAPH      — Multi-touch attribution edges
CF_KILL_SWITCHES          — Active flags, cached (≤2s TTL)
```

### 6.2 PostgreSQL (where SoT matters)

- Leapable product DB (CF_QUERIES readback for predicate)
- Synapse product DB (CF_SESSIONS readback for predicate)
- Polis product DB (event_attendances table readback for predicate)

The fleet **reads** product DBs directly for predicate evaluation. It does not write to product DBs. (Per `02_FIRST_PRINCIPLES.md §7` — product changes are operator-gated.)

### 6.3 Filesystem (markdown-backed wiki)

The LLM Wiki primary store is filesystem markdown. RocksDB indexes it. This is Karpathy's pattern: Obsidian-style live-editable knowledge that the agents read and the operator can browse in their normal editor.

---

## §7. Observability

| Capability | Tool |
|---|---|
| Per-agent traces | LangSmith (the LangGraph-native option) |
| Metrics + dashboards | Prometheus + Grafana (operator-hosted) |
| Logs | structured JSON to local + S3-compatible archive |
| Alerts | Slack + email via PagerDuty-style routing on circuit-breaker trips |
| Operator dashboard | Custom React app reading from RocksDB + LangSmith |

The operator dashboard (`15_METRICS_AND_OBSERVABILITY.md §6`) is the primary daily-use surface. It's built as a separate UI, not embedded in any agent flow.

---

## §8. Deployment topology

| Component | Where | Why |
|---|---|---|
| Orchestration (LangGraph) | Single host (operator's primary workstation or rented VM) | State must be durable, single-writer |
| Agent fleet (LLM calls) | API-only (Anthropic, OpenAI) | No GPU on the orchestration host |
| Computer-use (Synapse) | Operator's Windows desktop | Required for Win32 ops |
| Computer-use (Playwright pool) | Separate VM pool with residential IP rotation | Public read-only scraping |
| RocksDB | Same host as orchestration | Co-located for low latency |
| Wiki (filesystem) | Same host | Co-located + git-backed |
| Leapable.ai vaults | Operator's local leapable runtime | Owned + queryable |
| Marketing MCPs | Hosted by vendors (Markifact, Hyper, Windsor) | SaaS |
| Operator dashboard UI | Same orchestration host (port-exposed) | Local-only by default; cloud-exposable on operator-approved deploy |

**Single-host doctrine.** The orchestration host is the canonical state machine. Other components are stateless / queryable. If the orchestration host dies, the system is paused, not corrupted.

---

## §9. Build vs buy

| Component | Build or buy | Why |
|---|---|---|
| Orchestration | Buy (LangGraph) | Mature, production-grade, fits our model |
| LLM inference | Buy (Anthropic API) | Best frontier model |
| Marketing platform integrations | Buy (Markifact / Hyper / Windsor) | Already-built, well-maintained, $20-200/mo total |
| Memory expert vaults | Buy (leapable.ai) | Operator already runs |
| Brand voice classifier | Build (fine-tune Gemma 4B on operator's content) | Custom calibration required |
| Audience Sorter | Build | Custom signal weights |
| Influence Network Mapper | Build (atop leapable + LinkedIn data via Hyper MCP) | Custom graph weights |
| Operator dashboard | Build | UI is opinionated |
| Approval queue UI | Build | Tight integration with our LangGraph state |
| Computer-use | Buy (Synapse, which operator already owns) | Already shipped |

Buy where the integration is mature and standard. Build where the operator's specific knowledge is the value.

---

## §10. Costs envelope (monthly)

| Line item | Monthly cost (early) | Monthly cost (mature) |
|---|---|---|
| Anthropic API (Opus + Sonnet routed) | $400-1,200 | $2,000-8,000 |
| Marketing platform MCPs (Markifact, Hyper MCP, Windsor) | $200-500 | $500-1,500 |
| leapable.ai (existing operator subscription) | (operator) | (operator) |
| Hosting (orchestration VM, Playwright pool) | $100-300 | $400-1,000 |
| Observability (LangSmith + Grafana) | $50-150 | $200-600 |
| Email deliverability vendor (when scaling outbound) | $0 (Gmail) | $300-800 |
| Fine-tuning infra (one-time + maintenance) | $200-500 | $200-500 |
| **Total infrastructure** | **~$1,000-2,500/mo** | **~$3,600-12,400/mo** |
| Paid acquisition (Google/Meta/etc.) | per goal budget | per goal budget |

Infrastructure cost is roughly **5–10% of the paid acquisition budget** at maturity. Well below typical marketing-ops department overhead.

---

## §11. Security posture

- **Secrets management:** keychain on operator's host; never in agent prompts; rotated monthly.
- **Account credentials:** OAuth tokens where possible; least-privilege scopes.
- **Audit log:** append-only, cryptographically chained.
- **Per-agent scoped tokens:** an agent can only touch the tools its card authorizes.
- **Operator panic hotkey:** stops all activity in <50ms.
- **Network isolation:** orchestration host doesn't run servers exposed to the public internet without explicit operator approval.

Per `02_FIRST_PRINCIPLES.md §7`: no secrets in agent prompts. The Tool Gateway loads credentials at the call site from a secure store; agents never see them.

---

## §12. The Anthropic-first preference

We standardize on Anthropic Claude as the primary inference provider:
- Opus 4.7 (1M context) for the high-stakes reasoning roles
- Sonnet 4.6 for content + outreach drafting
- Haiku 4.5 for schema / classification / simple extraction

Why:
- Best frontier reasoning + best long-context handling for the multi-cycle goal-execution
- Native MCP support (we're MCP-native end-to-end)
- Prompt caching for the system-prompt-heavy meta-agents (90%+ cache hit rates on the Planner)
- Operator's existing infrastructure already targets Anthropic models

**Fallback:** OpenAI (Codex / GPT) for cross-validation when an experiment requires model-comparison evaluation. Never as the primary inference path.

---

## §13. Per-product technical alignment

Each product has its own native technology that the fleet needs to integrate with:

- **Leapable:** RocksDB CFs for SoT readback. The fleet queries `CF_QUERIES`, `CF_SUBSCRIPTIONS`, `CF_CREATOR_PUBLISH` for predicate evaluation. Leapable's own MCP surface (the 163 live `leapable_*` tools) is available to the fleet for the "test the product end-to-end" verification operations.

- **Synapse:** The Synapse MCP server runs on the operator's Win11 desktop and is the **primary computer-use surface for the fleet itself** (the fleet uses Synapse to operate other platforms!). Synapse is dogfooded.

- **Polis:** Postgres event_attendances table for SoT. Polis ambassador APIs (when built) for the ambassador-program tracking. Polis-native event-discovery tools for content generation.

---

## §14. Cross-references

- Agent fleet design (per-class tool authorization): `05_AGENT_FLEET_DESIGN.md`
- Computer-use specifics: `06_COMPUTER_USE_PLAYBOOK.md`
- Marketing MCPs (full inventory): `08_DISTRIBUTION_CHANNELS.md`
- Build roadmap (in what order to build): `21_BUILD_ROADMAP.md`
- Compliance enforcement at the Tool Gateway: `14_COMPLIANCE_AND_ETHICS.md §1`
- Metrics dashboard: `15_METRICS_AND_OBSERVABILITY.md §6`
