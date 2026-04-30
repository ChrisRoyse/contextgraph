# 35 - Open-Core Hosted Platform

## Goal

Define the **open-core split** across Context Graph, ClipCannon,
ME-JEPA, and the runtime constellation crate, plus the **hosted
managed-service** that monetizes it. The OSS engine is the adoption
flywheel; the hosted platform is the recurring revenue with infra
moat.

## Why Open Core (Not Pure SaaS, Not Pure OSS)

- Pure SaaS: no developer adoption, no GitHub stars, no paper-citation
  flywheel. Hard to compete with ElevenLabs' brand.
- Pure OSS: no recurring revenue, no enterprise SLA, no compliance
  bundling, harder fundraising story.
- Open core: OSS engine drives credibility (paper + GitHub +
  reproducibility); hosted/managed/enterprise tier captures revenue
  from teams that don't want to operate the engine themselves.
  Pattern works (HashiCorp, MongoDB, Confluent, dbt Labs, Databricks).

## License Boundary

| Component | License | Rationale |
| --- | --- | --- |
| Context Graph runtime (370K LOC Rust) | **BSL-1.1 → Apache-2.0 after 4 years** | BSL allows commercial use except hosting a competing service; converts to Apache after 4 years per BSL norms |
| ClipCannon (67K LOC Python) | **BSL-1.1 → Apache-2.0 after 4 years** | Already BSL per `docs2/alldata.md`; keep |
| ME-JEPA crate | **Apache-2.0** dual-license MIT | Encourage academic + commercial adoption of the runtime |
| Constellation crate (shared, doc 28) | **Apache-2.0** dual-license MIT | Most strategic component for adoption |
| Voice clone product (doc 31) | **Closed source** | Direct revenue product |
| EU AI Act compliance product (doc 33) | **Closed source** | Direct revenue product |
| Identity-locked agent platform (doc 34) | **Closed source** | Direct revenue product |
| Domain packs (doc 37) | **CC-BY-4.0 community / commercial as opted in** | Dual: free community packs, paid premium packs |
| MCP tool surface (doc 39) | **MIT** | Maximize adoption; tools are coordination, not differentiation |

The BSL "additional use grant" allows non-competing commercial use.
The "competing use" definition is **"hosting Context Graph or
ClipCannon as a multi-tenant managed service"** — that is exactly the
hosted platform we sell.

## Hosted Platform Tiers

### Free / Community

- OSS download.
- Self-hosted, single tenant, single GPU.
- No SLA. Community Discord support.
- Used to build adoption + paper credibility.

### Developer (Hosted)

- $49/mo per developer.
- Multi-tenant hosted Context Graph + ClipCannon + ME-JEPA.
- 10K events / 100K embeddings / month included.
- Bundle DOI minting included.
- Use case: solo devs, academic labs, prototyping.

### Team (Hosted)

- $499/mo per team (10 seats).
- 100K events / 1M embeddings / month.
- Private workspace, RBAC, audit log.
- Use case: small product teams.

### Enterprise (Managed)

- $250K–$1M+/yr ELA.
- Single-tenant or on-prem deployment.
- Constellation custom centroid sets.
- Compliance product bundling (doc 33).
- Phase 12 privacy gate (doc 25).
- 24/7 support; named CSM; quarterly business review.

## Platform Surface

The platform exposes:

- **Web console**: workspace + bundle browser + drift dashboard.
- **API**: matches MCP tool surface (doc 39); REST + JSON-RPC
  alongside MCP for non-LLM clients.
- **CLI**: matches what the OSS user already runs locally; identical
  command names; just substitute `--remote=...` flag.
- **Bundle store**: hosted Zenodo-equivalent for customer bundles
  with DOI minting on demand (subset of Zenodo for non-academic
  customers).
- **Marketplace** (doc 37): browse / install domain packs and role
  packs.

## OSS Adoption Flywheel

The plan to drive GitHub stars and citation gravity:

1. PAPER.md submitted to arXiv with DOI cross-link to repo (doc 36).
2. ME-JEPA paper (smokedocs 14-25) submitted with reproducibility
   artifact (doc 20).
3. TCT paper (Cases 1, 2, 3 fully measured) submitted (doc 36).
4. Each paper's headline benchmark has a `cargo run --example
   reproduce_<benchmark>` that works on the operator's RTX 5090 and
   has been tested in CI (doc 23).
5. Tutorial blog series (one per major construct: DDA, Constellation
   Guard, FSV, ME-JEPA).
6. Hugging Face Spaces demo for voice cloning + a Spaces demo for
   DDA enrichment.
7. Conference talks: NeurIPS / MLSys / ICLR / KDD on different angles
   of the same stack.
8. Annual report: "1M signals enriched, X% drift caught, Y% rejected"
   — public stats from the hosted platform that drive credibility.

The OSS adoption is what gives the hosted platform its top-of-funnel.
Without it, enterprise sales is uphill.

## Hard Engineering Items

- Multi-tenant deployment architecture (Apptainer + Kubernetes
  orchestration);
- Per-tenant RocksDB isolation;
- Centralized billing + usage metering;
- Web console (React or SvelteKit, ~12 weeks);
- Bundle DOI minting flow (doc 20);
- Marketplace registry (doc 37 dependency);
- Single-sign-on for enterprise tier;
- SOC 2 Type II + ISO 27001 (~12 months parallel);
- Documentation site, API docs, tutorials.

## Risks

- **License blowback**: BSL is sometimes contentious in OSS
  communities. Mitigation: 4-year sunset to Apache-2.0 is published
  upfront; Apache for ME-JEPA / Constellation / MCP defuses
  most tension; keep BSL only where necessary to prevent direct
  competing services.
- **Hyperscaler service replication**: AWS / Azure / GCP could
  replicate. Mitigation: BSL prevents direct hosting; differentiation
  is the integrated stack, not just the runtime; partner with
  hyperscalers via AWS Marketplace / Azure Marketplace as our
  distribution.
- **Platform engineering cost**: hosting is expensive at small scale.
  Mitigation: launch Developer + Team tiers on a single autoscaling
  cluster; only build per-tenant infrastructure for Enterprise.

## Coordination With Other Docs

- Doc 31 / 32 / 33 / 34: each product is a service on the hosted
  platform.
- Doc 23 (CI): reference-bundle hash policy ensures hosted matches
  OSS.
- Doc 37 (marketplace): marketplace lives in the hosted console.
- Doc 39 (MCP surface): the API of the platform.
- Doc 41 (roadmap): platform launch is gated on at least one product
  (doc 31 voice) being live.

## Acceptance

Open-core platform is **launch-ready** when:

- license files are committed in every repo (BSL or Apache as listed);
- public OSS docs page is live with quickstart, tutorials, API ref;
- Developer-tier and Team-tier are live with billing;
- one paying Team-tier customer is using the hosted platform end-to-end
  (DDA enrich + ME-JEPA train + Constellation Guard at runtime);
- GitHub stars > 1K, repo on first page of "auditable ML"
  searches.
