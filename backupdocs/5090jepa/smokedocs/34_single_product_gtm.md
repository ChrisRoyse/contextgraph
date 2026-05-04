# 34 - Single-Product Go-To-Market

## Goal

Replace the previously fragmented product portfolio (voice cloning
SaaS + DDA-aaS + EU AI Act compliance + identity-locked agents +
open-core hosted) with a **single product positioned as one
integrated platform**. One brand, one pitch, one pricing structure.
Customers buy **the monolith**, not a product from a portfolio.

The previous five product docs (deleted 2026-04-30) are replaced by
this single doc and by the capabilities described in smokedoc 31.

## The Pitch (One Sentence)

> **The monolith is the only AI infrastructure that ingests any
> corpus, projects it through a fixed multi-modal embedder panel,
> trains identity-locked predictors on a single GPU, validates every
> output against frozen geometric centroids at runtime, and ships
> reproducible end-to-end provenance — in one binary, on one
> machine.**

That sentence is the elevator pitch, the abstract, the README first
line, the Twitter bio. Same words everywhere.

## Three Buyers, One Product

We have three distinct buyer personas. **They buy the same product.**
The conversation differs by what they're escaping.

### Buyer A — The Frontier AI Lab

What they're escaping: the data wall. They are paying $60M-$250M+
per data licensing deal (Reddit, News Corp, etc.) and Shumailov
collapse rules out synthetic-data recursion as a cheap alternative.

What we offer: 91 signals per input from a fixed corpus through a
frozen embedder panel. **Path 3 around the data wall.** PAPER.md
locks the naming.

How they use the monolith: deploy on their training cluster
(single-tenant install of the monolith), point at their existing
licensed corpus, get back FSV-bundled enriched dataset to feed
into their pre-training mix.

Pricing: $250K-$5M/year as a deployment + integration agreement.
Long sales cycle (6-12 months).

### Buyer B — The Regulated Enterprise

What they're escaping: EU AI Act + SR 11-7 + state-level AI laws
without a defensible audit story. Their existing ML stack (MLflow,
W&B, custom audit logs) does observability, not source-of-truth
verification.

What we offer: FSV bundles + Constellation Guard + tamper-evident
chain + per-output provenance + NIST AI RMF mapping (smokedoc 21).
**Compliance-by-construction**, not as overlay.

How they use the monolith: deploy alongside their model lifecycle
(single-tenant; on-prem available), wrap their existing models in
the monolith's FSV envelope, generate regulator-ready conformity
packages from bundle data.

Pricing: $250K-$1M/year ELA. 12-18 month sales cycle.

### Buyer C — The Builder

What they're escaping: stitching ElevenLabs + HeyGen + LangChain +
Pinecone + MLflow + their own audit code. Five vendors, five
contracts, five surfaces, no end-to-end provenance.

What we offer: one binary that does all of that and provides
verifiable identity-locking + provenance for free. **One purchase
order.**

How they use the monolith: self-serve install, integrate via the
unified MCP surface (smokedoc 33), build the product they were
going to build but with provenance and identity-locking by default.

Pricing: $99-$999/month per developer/team for hosted access OR
free OSS for self-hosted (per smokedoc 35 license terms). 1-4 week
sales cycle for the team tier.

## Pricing Structure

One product, three pricing tiers reflecting deployment mode rather
than feature gating. Every tier has the same capabilities; what
differs is **who runs it** and **what support you get**.

| Tier | Audience | Deployment | Pricing | Support |
| --- | --- | --- | --- | --- |
| **Self-Hosted (OSS Core)** | Buyer C builders, academics, evaluators | Customer's hardware; OSS core (BSL/Apache split per smokedoc 32) | Free | Community Discord; GitHub issues |
| **Hosted Team** | Buyer C teams ≤10 | Our cluster, multi-tenant | $499/mo team | Email; SLA-light |
| **Hosted Enterprise** | Buyer A, B, C-large; mid-market | Our cluster, single-tenant or dedicated | $250K-$1M/year ELA | Named CSM; 4h-response SLA; quarterly review |
| **Sovereign / On-Prem** | Buyer A, B with strict data residency | Customer hardware, our deployment + maintenance | $1M-$5M/year | Dedicated SRE; co-location of one engineer if requested |

No feature gates. No "voice clone tier" or "compliance tier". Every
tier ships every capability. What scales is **operational lift**.

## Differentiation Story (Same Stack, Three Lenses)

| Buyer A frame | Buyer B frame | Buyer C frame |
| --- | --- | --- |
| 91 signals/input on fixed corpus | Article 10 data governance evidence | Replaces Hugging Face Datasets + your own labeling |
| ME-JEPA + DDA + frozen embedders escapes Shumailov regime | Constellation Guard satisfies Article 14 human oversight | Identity-locked voice/video clones with runtime verification |
| Per-output verifiability replaces RLHF with geometric guarantees | FSV bundle + tamper-evident chain = Article 12 logging | One MCP surface replaces ElevenLabs + HeyGen + your custom guard |
| Single-paper anchor (smokedoc 19) you can cite in your scaling-laws paper | Conformity package generator (smokedoc 21 mapping) | Apptainer + Docker; one binary; no microservice mesh |

Same primitives. Different framing per buyer. The sales engineer
flexes the deck; the underlying product is identical.

## Trust + Safety (Required Across Tiers)

Same policy applies to every tier:

- Voice / video clone consent attestation (smokedoc 31 §"What it does
  not do" enforcement layer).
- Public-figure refusal list.
- Inaudible watermark on Hosted Team, optional on others.
- Phase 12 privacy gate (smokedoc 25) is enabled only on Sovereign /
  on-prem with explicit subject-data approval.
- Every tier emits FSV-bundled provenance per output.

Trust is the differentiator vs all competitors. Do not soften the
floor by tier.

## Sales Channels

| Channel | Primary For | Effort |
| --- | --- | --- |
| Inbound from PAPER.md / arXiv / YouTube (Leapable AI channel) | Buyer C, academic Buyer A | Low-touch; ensure landing page + free trial |
| Direct outbound to frontier labs (5-15 named accounts) | Buyer A | High-touch; CEO/founder-led |
| Direct outbound to FS / regulated mid-cap CIOs | Buyer B | Sales engineer + compliance specialist |
| AWS / Azure / GCP Marketplace listings | Buyer C and small Buyer B | Medium-touch; platform partnerships |
| Conference presence: NeurIPS, MLSys, RSA, Money 20/20, ISACA | Buyer A + B credibility | Annual |

YouTube channel (Leapable AI, ~68 videos pre-existing per
`docs2/leapableai_youtube_transcripts.md`) is the standing
distribution. Every release shipped publicly with one demo video.

## Anti-Patterns To Avoid

- **Reverting to product fragmentation.** Even when a buyer says "I
  just want voice", the sale is the platform. Selling subsets erodes
  the moat.
- **Free voice tier with API quotas.** Voice generation has real
  marginal cost (GPU minutes); a generous free tier becomes a loss
  leader without the upgrade hook a horizontal platform provides.
- **Custom features per customer.** Every customer wants a tweak; if
  every tweak ships, the platform fragments. Hold the line: one
  product, configurable via domain packs and centroid sets.
- **Letting the brand fragment.** Don't sell "Teleox Voice" and
  "Teleox Compliance" as separate products. Sell **Teleox**.

## Annual Plan (Indicative, Not A Forecast)

Year 1 from v1.0 release:

| Tier | Target Logos | Effective ARR |
| --- | --- | --- |
| Self-Hosted (OSS) | 100+ active deployments | $0 direct; lead-gen for paid |
| Hosted Team | 50-200 paying teams | $300K-$1.2M |
| Hosted Enterprise | 3-10 logos at $250K-$1M | $750K-$10M |
| Sovereign / On-Prem | 1-3 logos | $1M-$15M |

Total Year-1 ambition: **$2M-$25M ARR**, biased to enterprise. The
distribution is right-skewed; one Sovereign deal alone could float
operations for 2 years.

## What Replaces The Deleted Product Docs

| Deleted product | Where its capability lives now |
| --- | --- |
| Voice cloning SaaS | Capability of every tier; sold as part of platform |
| DDA-aaS | Capability of every tier; sold as part of platform |
| EU AI Act compliance | Capability of every tier; foregrounded for Buyer B |
| Identity-locked agent platform | Capability of every tier; foregrounded for Buyer B internal-AI |
| Open-core hosted platform | Now the deployment-tier dimension; not a separate product |

Every capability survives. The product fragmentation framing does not.

## Acceptance

Single-product GTM is **launch-ready** when:

- one landing page exists with the elevator pitch and three buyer
  framings, all pointing at one deployment-tier matrix;
- one pricing page exists with no feature gates;
- one demo video covers DDA → ME-JEPA → voice generation → guard →
  bundle hash in under 8 minutes;
- sales engineering deck has 3 framings, 1 product;
- no external communication describes the monolith as multiple
  products.
