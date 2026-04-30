# 32 - DDA-as-a-Service Platform

## Goal

Productize the Context Graph 13-embedder DDA substrate as a hosted
**corpus enrichment service**: customers upload a corpus, the service
projects through 13 frozen embedders + 78 pairwise interactions, and
returns a **91-signal-per-input enriched dataset bundled with full FSV
provenance**. Sell to AI labs and enterprise data teams who hit the
data wall and want to extract more supervisory signal from their fixed
corpus without entering a Shumailov-form recursion.

## Why This Is A Real Product

The data wall is real (Villalobos 2026–2032 exhaustion projection;
Longpre 2024 robots.txt erosion). Frontier labs are paying $60M–$250M+
per data licensing deal (Reuters 2024 Reddit-Google; reporting on
News Corp–OpenAI). DDA is structurally outside the recursion regime
that bounds synthetic data; nobody else has named it as a third path.

A hosted version monetizes the substrate without requiring customers to
operate Context Graph's 370K-line Rust workspace themselves.

## Product Surface

### Tiers

| Tier | Audience | Pricing | Capabilities |
| --- | --- | --- | --- |
| **Lab** | Self-serve via API | $0.0005 per signal generated (~$0.046 per 91-signal input) | Standard 13-embedder panel; standard pairwise adapter; bundle delivery |
| **Enterprise** | Direct sales, SLA | $250K+ per corpus or $1M–$5M per year for sustained ingestion | Custom embedder panels; bring-your-own embedder; private deployment; FSV bundle DOI |
| **Frontier** | Top 5 model labs | Custom; equity-aware | On-prem Apptainer + dedicated engineer + IP-share clauses |

### Architecture

```text
customer corpus (S3 / GCS / private bucket)
   │
   ▼
[ Ingest worker ]   per-tenant isolation, content hashing
   │
   ▼
[ DDA substrate ]   13 frozen embedders × n inputs
                    78 pairwise interaction features
                    (parallelized across GPU pool)
   │
   ▼
[ FSV bundle assembly ]   matches doc 29 schema; per-input provenance
   │
   ▼
[ Mutual-info audit ] (optional)   doc 30 audit on customer corpus
   │
   ▼
[ Delivery ]   parquet bundle + tar.gz + recompute script
```

### What Customers Get

A signed bundle directory:

```text
dda_<customer>_<corpus>_<timestamp>/
  corpus_manifest.json                # source SHA-256, license, allowed-use
  embedder_panel_manifest.json        # 13 embedder versions + hashes
  per_embedder_vectors.parquet        # n × N rows, full vectors
  pairwise_interactions.parquet       # n × 78 rows
  mi_audit.json                       # optional, doc 30
  reviewer_recompute.sh
  bundle_root_hash.txt
```

The bundle is **the deliverable**, not access to a UI. Customers
integrate it into their training pipelines directly. Their training
team is the buyer, not their CMO.

## Differentiation vs DIY

A frontier lab could in principle stand up their own embedder panel.
The pitch:

| Aspect | DIY | Our Service |
| --- | --- | --- |
| Embedder pinning | Manual; drifts on every dependency bump | Hash-pinned; CI-verified (doc 23) |
| Pairwise adapter | Not standard; varies per team | Frozen; documented; PAPER.md citation |
| Mutual-info audit | One-off scripts | Reproducible (doc 30); part of bundle |
| FSV provenance | Logging at best | Hash-recomputable end-to-end; reviewer can recompute |
| Time-to-value | 3–6 months engineering | 1–4 weeks ingestion |
| Paper-citable | No | Yes; bundle DOI cross-cites PAPER.md |

The "paper-citable" line is the closer for academic / research-oriented
buyers. They want to publish; bundle DOIs help them.

## Embedder Panel Customization

The **standard** panel is the Context Graph 13-embedder set documented
in PAPER.md. Enterprise customers can request custom panels:

- **Add** an embedder: e.g. domain-specific E14 (legal-bert,
  bio-bert, fin-bert). Customer brings the model artifact + license.
- **Swap** an embedder: e.g. replace E1 e5-large-v2 with a newer or
  proprietary embedder. License must be compatible.
- **Subset** the panel: e.g. only E1, E7, E10, E11 for a code-only
  corpus. Reduces signal count to $4 + 6 = 10$ per input.

Every panel customization is a new versioned panel manifest, with its
own embedder hashes. The DDA counting identity scales with $N$, so
customers see the cost-per-signal directly.

## Mutual-Info Audit As Upsell

Standard tier ships per-embedder vectors and pairwise interactions
without a redundancy audit (just the constructive 91 upper bound).
Enterprise + Frontier tiers ship doc-30's MI audit, which gives the
customer:

- $k_{\text{eff}}$ on their actual corpus (typically 50–80 of the 91
  upper bound);
- per-pair redundancy ratios;
- per-embedder marginal contribution to a held-out task.

For a customer training a foundation model, $k_{\text{eff}}$ on their
domain tells them which embedders are pulling weight and which they
can drop, saving compute downstream.

## Operational Model

- Ingestion: Apptainer-deployed substrate; one cluster per tenant;
  RocksDB stores ephemeral, deleted after bundle delivery.
- Storage: customer corpus is **never persisted** beyond ingestion;
  bundle is the only retained artifact, and only for an agreed
  retention window.
- Privacy posture (coordinates with doc 25): standard tier requires
  customer attestation that corpus is permitted for derived-product
  generation; Enterprise tier offers Phase-12-grade pseudonymization
  if customer's corpus contains subject-level data.
- Compliance: SOC 2 Type II within 18 months of launch; ISO 27001 in
  parallel.

## Commercialization Risks

- **Customer concentration**: 5 frontier labs are 80% of the obvious
  TAM. Counterweight: position lower-tier (Lab) as a wedge for
  mid-market and academia.
- **Embedder licensing**: not every embedder in our 13-panel is
  freely usable for derivative-data generation. Audit licenses
  before launch; replace any non-permissive embedder in the public
  panel (Enterprise can use proprietary panels under their own
  licenses).
- **Customer can replicate after one purchase**: the bundle is a
  one-shot artifact. Counterweight: ongoing ingestion contracts,
  embedder version pinning + drift detection, mutual-info audit
  refresh, paper-citation lock-in.
- **Lab buyers are slow**: 6–12 month evaluation cycles. Plan for
  long sales cycles; do not let early conversations stall the
  voice-product (doc 31) revenue floor.

## Hard Engineering Items (Pre-Launch)

- Multi-tenant Context Graph deployment (Apptainer or Docker isolation);
- Customer-facing CLI / API to upload + monitor + download;
- Per-tenant audit log (which embedder versions, which pairwise
  adapter, when ingestion ran);
- Pricing meter on signals generated;
- License-compliance audit on the standard panel;
- Bundle DOI minting flow (doc 20);
- SOC 2 readiness pass.

## Coordination With Other Docs

- Doc 27 (DDA → ME-JEPA bridge): customers training their own
  ME-JEPA-style model can plug DDA bundles directly in.
- Doc 30 (MI audit): the audit is the upsell.
- Doc 33 (EU AI Act compliance): bundle satisfies Article 10 (data
  governance) for downstream regulated systems.
- Doc 36 (paper roadmap): the DDA paper is this product's marketing.
- Doc 37 (domain pack registry): customers can buy domain-specific
  embedder panels off-the-shelf.
- Doc 35 (open-core hosted platform): DDA-aaS is one of the platform's
  services.

## Acceptance

DDA-aaS is **launch-ready** when:

- self-serve API + bundle delivery works end-to-end on a 1M-row
  corpus in under 24 hours;
- one paying Lab-tier customer has run an ingestion and integrated
  bundle into a training pipeline;
- license audit is complete and the standard panel is provably
  free-for-derivative-data;
- pricing meter accurately bills to ±1%;
- customer-facing recompute script verifies their bundle in <10 min
  on a single GPU.
