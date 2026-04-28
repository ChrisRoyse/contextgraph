# 11. Companion System — Leapable as an AI-Agent-First Marketplace Exemplar

## 11.1 Why this section belongs in the paper

The argument of Sections 1–10 is methodological. A reasonable reader at this point asks: *is any of this actually deployable commercially, and what would a commercial distribution model look like for systems that expose DDA-enriched data to AI assistants as a two-sided marketplace?* Section 11 answers with a specific companion system we have built and are shipping under **leapable.ai**. Leapable is not itself a TCT instance — it is an AI-agent-first knowledge-infrastructure marketplace with different purposes — but it demonstrates a commercial and architectural pattern that we argue is the natural distribution model for the DDA-enriched tools described in this paper.

The commercial pattern has three distinguishing properties:

1. **AI agents are the primary paying customers, not humans.** Creators upload knowledge corpora; AI agents (Claude, Cursor, Zed, Cline, Continue, Windsurf, Roo, Gemini CLI, LM Studio, Amp) pay per-query to consult those corpora through 161 `leapable_` MCP tools.
2. **Every document is DDA-decomposed on ingest.** Upload triggers OCR, chunking, embedding (768-D nomic-embed-text or modernbert-legal), VLM image analysis, PII detection, and provenance-chain recording — producing a multi-dimensional labeled representation of the corpus before any query arrives.
3. **Revenue flows through a double-entry append-only ledger with deterministic Stripe idempotency.** Creators receive 70% (75% for founding creators); a 5% reserve is held 60 days against chargebacks; subscribers purchase one of four tiers (explorer, pro, builder, elite) gated by content class. Every financial event writes a balanced ledger transaction whose entries must sum to zero, verified at boot.

Leapable is an actively-maintained product at version 2.2.2 with 161 tools across 32 modules, 26 marketplace tables, 35 RAG tables, 6 registry tables, ~94,639 lines of source code, and 162 test files. The details here are condensed; the full system-level description lives under `docs2/systemdescription/` in the Leapable repository.

## 11.2 Architecture in one diagram

```
AI Agents (primary customers)            Human Creators (secondary)
Claude, Cursor, Zed, Cline, ...          Browser at leapable.ai
        │                                         │
        │ stdio / HTTP (MCP JSON-RPC)             │ HTTPS (Cloudflare Pages)
        │                                         │
        ▼                                         ▼
        ┌─────────────────────────────────────────────┐
        │       Tauri 2 One-Click Installer           │
        │       auto-configures 9 MCP clients         │
        └─────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│            Caddy Reverse Proxy (:80)                    │
│            config/Caddyfile                             │
└────┬──────────┬──────────────┬──────────────┬───────────┘
     │          │              │              │
     ▼          ▼              ▼              ▼
┌────────┐  ┌────────┐  ┌──────────┐  ┌────────────┐
│Market- │  │  MCP   │  │Dashboard │  │  Session   │
│place   │  │Server  │  │  :4200   │  │   Proxy    │
│:4000   │  │ :4100  │  │vanilla   │  │   :4300    │
│Hono 4.6│  │161 tls │  │HTML + JS │  │multi-agent │
│Stripe  │  │REST +  │  │          │  │   isolate  │
│Connect │  │JSON-RPC│  │          │  │            │
│60 routes│  │+ SSE  │  │          │  │            │
└───┬────┘  └────┬───┘  └──────────┘  └────────────┘
    │           │
    │           └─► Python workers (15):
    │                ocr / embedding / reranker / vlm / redaction
    │                clustering / extract / serverless_handler / …
    │
    │ X-License-Key (fan-out per file, no document data
    │  persisted in the marketplace)
    ▼
┌────────────────────────────────────────────────┐
│ RunPod Serverless GPU workers (on demand)      │
│ Dockerfile.serverless(.slim)                   │
│ python/serverless_handler.py                   │
│ Marker + chunk + embed in one call             │
│ replaces dedicated H100 server (commit e4cf6ca)│
└────────────────────────────────────────────────┘
```

The first five services run inside a single Docker container via `scripts/docker-entrypoint.sh`. A Next.js 15 subscriber dashboard (`packages/subscriber-dashboard/`) runs separately on port 3367 with 7 pages and 29 React components. A Tauri 2 one-click installer (`packages/installer/`, v1.1.17) runs on each end-user desktop. The RunPod Serverless worker is invoked per file, on demand, and never persists data.

## 11.3 The 161-tool MCP surface

Leapable exposes 161 tools across 32 modules (all `leapable_`-prefixed), registered in `src/server/register-tools.ts`. The tool surface is the primary product: AI agents connect via MCP JSON-RPC and get a machine-readable decomposition of the system's capabilities without any product-specific training.

| Module | Count | Representative tools |
|---|---|---|
| Database management | 13 | create, list, select, stats, archive, search |
| Portability | 7 | backup, clone, import, merge, restore, snapshot |
| Sharing | 4 | share, import shared, transfer, receive |
| Ingestion | 7 | ingest files, directory, process pending, retry failed |
| Search | 7 | hybrid, cross-database, RAG context, BM25, semantic |
| Documents | 10 | get, list, delete, find similar, duplicates, structure |
| Chunks | 4 | get, list, context expansion, page reading |
| Embeddings | 4 | get, list, rebuild, stats |
| Images | 8 | get, list, search, pending, reanalyse |
| VLM | 3 | describe, process, status |
| Provenance | 6 | get chain, query, timeline, verify, export (W3C PROV) |
| Reports | 7 | overview, performance, cost summary, error analytics |
| Intelligence | 5 | guide, extras, tables, table export, recommendations |
| Comparison | 6 | diff, batch compare, similarity matrix |
| Clustering | 7 | HDBSCAN / agglomerative / k-means, merge, reassign |
| Tags | 6 | create, list, apply, remove, search, delete |
| Collaboration | 11 | annotations, document locks, search alerts |
| Workflow | 8 | submit, assign, review, approval chains |
| Contract / CLM | 9 | extract, summarise, obligations, playbooks |
| Compliance | 3 | reports, HIPAA, SOC 2 / SOX export |
| Events | 6 | webhooks, export annotations / audit / obligations |
| Redaction | 5 | analyse, apply, restore, status, configure |
| Dashboard | 2 | open, status |
| Creator | 4 | signup-open, me-by-key, paywall-by-key, onboarding |
| Extraction | 1 + structured 2 | raw content extraction + schema-typed extraction |
| Maintenance / Health / Users / Config | 1 + 1 + 2 + 2 | system-administration primitives |

**The creator module (4 tools, added in v2.1+)** is the AI-agent onboarding surface. Agents authenticate via `mk_`-prefixed keys (not session cookies), call `leapable_creator_signup_open` unauthenticated to get a key, then use the key to self-manage their own paywall — a full agent-driven creator lifecycle with no browser required.

## 11.4 Ingestion pipeline (DDA on documents)

Every document Leapable ingests is multi-embedded, much like Context Graph (Section 4) but with a narrower modality focus appropriate to document intelligence:

```
File (PDF, DOCX, PPTX, XLSX, HTML, EPUB, MD, TXT, CSV, images)
    │
    ▼
INGESTION — validate, SHA-256, dedup, document row created, provenance genesis
    │
    ▼
OCR PATH SELECTION (text-fast / PDF-fallback / Marker / serverless)
    ├─ .md/.txt            → src/services/ocr/text-fast.ts (Node, 20–50 ms warm)
    ├─ born-digital PDF    → python/pdf_text_extract.py (PyMuPDF)
    ├─ universal fallback  → python/doc_text_extract.py (18 formats)
    └─ scanned/complex     → Marker-pdf daemon OR RunPod Serverless
        ↓
    ┌ CHUNKING — section-aware, target 2000 chars, 10% overlap, atomic
    │   ↓
    │ EMBEDDING — 768-D via nomic-embed-text-v1.5 (general) or modernbert-legal-8192 (legal)
    │   ↓ L2-normalised, written to sqlite-vec HNSW index
    │
    └ IMAGE EXTRACTION → VLM (Chandra OCR 2 v0.2.0) → structured JSON description → VLM embedding
        ↓
INDEXING — sqlite-vec HNSW + SQLite FTS5 + 50+ B-tree indexes (schema v34)
    │
    ▼
SEARCH — BM25 + semantic + hybrid RRF (Cormack et al., 2009) + cross-encoder reranker
         (ms-marco-MiniLM-L-12-v2)
```

Two embedding options, chosen at database creation and immutable thereafter because vectors from different models are not comparable:

| Model | Slug | Best for | VRAM |
|---|---|---|---|
| `nomic-embed-text-v1.5` (Nussbaum et al., 2024) | `general` | Business, research, general documents | ~1 GB |
| `modernbert-legal-8192` | `legal` | Legal contracts, court filings, compliance | ~1.5 GB |

Every mutation writes an immutable provenance record with a SHA-256 content hash linked by a chain formula analogous to ClipCannon's (Section 5.6). Chain verification (`leapable_provenance_verify`) walks leaf-to-root and identifies the exact tampered record on failure. Chain export (`leapable_provenance_export`) emits JSON, W3C PROV-JSON, or CSV.

## 11.5 The marketplace (two-sided economics)

Leapable's commercial innovation is the two-sided marketplace implemented by `packages/marketplace/` (a Hono 4.6 server on port 4000 with 19 route files and 60 handlers).

**Creators** sign up via Google OAuth one-click or magic-link email, complete Stripe Connect controller-properties onboarding (Express dashboard, platform pays fees and absorbs losses), publish paywalls, and manage:

- a public profile (bio, headline, expertise tags, sample questions, discoverable via FTS5 `creators_fts` virtual table);
- content classes (`free` / `pro` / `elite`) that gate documents by subscriber tier;
- quality gates (`POST /v1/creator/quality-check` runs ten predefined expert queries against the corpus; a failing corpus cannot be published);
- gap reports (category-specific missing-content analysis).

**Subscribers** (primarily AI agents) purchase one of four tiers (explorer / pro / builder / elite, commit `5fc218f`). Any subscriber with credits can query any creator's database — the join is LEFT, not INNER, so paywalls are optional. The query route (`routes/query.ts`) runs a four-layer access check:

| Layer | Check | Effect |
|---|---|---|
| 1 | Active subscription | Full access within the subscription's monthly cap |
| 2 | Ledger credit balance (`subscriber:{id}:credit`) | Access via purchased credit packs or owner adjustments |
| 3 | Free-tier monthly limit | Explorer tier: 50 free queries / month across all creators |
| 4 | Content-class gating | Subscriber tier → allowed content classes; filters documents returned |

The four allocation tiers default to 50 / 200 / 1,000 / 2,500 / 6,000 queries per month for explorer / starter / builder / pro / max respectively, configurable via `TOKEN_ALLOCATIONS_*` env vars.

## 11.6 Revenue split and payout engine

Every financial event writes a balanced transaction to an append-only `ledger_entries` table, verified at boot by `verifyLedgerIntegrity()` which throws `LedgerIntegrityError` if any transaction's entries do not sum to zero. The engine (`packages/marketplace/src/allocation.ts`) has three operations:

1. **Period close** — drains each creator's `platform:holding:{id}` into `platform:reserve:{id}` + `creator:{id}:payable` and inserts one `payouts` row per creator in `pending` status.
2. **Reserve sweep** — releases reserves older than `CREATOR_RESERVE_HOLD_DAYS` (default 60) into the creator's payable account as a new `payouts` row so it flows through the normal transfer pipeline.
3. **Stripe transfer** — calls `stripe.transfers.create` for each pending payout whose creator is fully onboarded, records the transfer id, flips status to `transferred`, writes the offsetting ledger entries.

**Revenue split defaults** (`env.ts`):

| Variable | Default | Purpose |
|---|---|---|
| `CREATOR_SHARE_PERCENT_STANDARD` | 70 | Standard creator share (%) |
| `CREATOR_SHARE_PERCENT_FOUNDING` | 75 | Founding creator share (%) |
| `CREATOR_RESERVE_PERCENT` | 5 | Held back at period close as chargeback/dispute reserve |
| `CREATOR_RESERVE_HOLD_DAYS` | 60 | Days to hold reserve before release |

**Deterministic Stripe idempotency**: transfer calls use `idempotencyKey: payout_{payout_id}_{last_8_chars_of_destination}`. Same payout_id + same destination → same key → Stripe replays the identical result. Same payout_id + corrected destination → different key → fresh Stripe request without "params don't match original" rejection.

**Subscriber-side billing**: per-page usage charges (OCR at 5 ¢/page, redaction at 3 ¢/page) flow through a centralised `chargeUsage()` function in `packages/marketplace/src/usage-charges.ts`. This is orthogonal to the payout engine — usage charges debit subscriber credit accounts; the creator payout path sees only subscription revenue.

## 11.7 Serverless OCR (replaces dedicated H100 server)

Heavy GPU OCR moved from a dedicated H100 pod to **RunPod Serverless** in commit `e4cf6ca`. The new pipeline is:

- `src/services/cloud-api/serverless.ts` submits jobs one HTTP request per file.
- `src/services/cloud-api/serverless-queue.ts` enforces client-side FIFO concurrency (up to 10 parallel jobs).
- `packages/marketplace/src/serverless-orchestrator.ts` handles marketplace-side fan-out, billing gate, and result merging.
- `packages/marketplace/src/routes/ocr.ts` is the `X-License-Key`-authenticated ingress (mounted before session middleware because it uses a header, not a cookie).
- `python/serverless_handler.py` is the RunPod Serverless worker entry, packaged in `Dockerfile.serverless` / `Dockerfile.serverless.slim`.

The RunPod API key lives **only on the marketplace server**. End users authenticate with their locally-issued license key (`lk_` + HMAC of `machine_id`) — they never see or need the RunPod credential. Logs go to `ocr_job_log` for audit. Per-page billing is applied on `status=COMPLETED` by `chargeUsage()`.

## 11.8 Five-caller authentication matrix

Leapable's security model handles five distinct caller types, each with its own auth path (`docs2/systemdescription/08_security_architecture.md`):

| Caller | Auth | Mount point | Purpose |
|---|---|---|---|
| Human creator (browser) | Session cookie (HS256 JWT) | `/v1/creator/*` session middleware | Dashboard, paywall management |
| AI agent (MCP) | `X-Marketplace-Key` (`mk_` key) | `/v1/creator-mcp/*`, mounted BEFORE session | Automated creator onboarding |
| Subscriber agent | Session cookie OR API key | `/v1/paywall/query*` | Query creator corpora |
| Local MCP container | `X-License-Key` (`lk_` key) | `/v1/ocr/*`, `/v1/redact/*`, mounted BEFORE session | Serverless OCR / redaction billing |
| Platform admin / owner | Session + `platform_admins` role | `/v1/admin/*`, `/v1/owner/*` middleware | Payout runs, balance adjustments, audit log |

Device auto-login (365-day tokens) silently mints fresh sessions for known browsers so users never see a login screen on returning visits. Owner immutability is protected by three SQLite triggers (no-delete, no-demote, no-rename) plus env-layer pinning (`PLATFORM_OWNER_EMAIL`) plus app-layer reconciliation at every boot (`reconcileAdmins()` throws `OwnerDriftError` on drift).

## 11.9 Tauri 2 one-click installer

The distribution model depends on frictionless local installation. `packages/installer/` is a Tauri 2 desktop app (v1.1.17, Rust backend in `src-tauri/`, WebView UI in `src/`) that:

- detects OS and Docker;
- pulls the subscriber image (`Dockerfile.app`, no GPU required);
- generates JWT secrets locally;
- persists user data across image updates via a `machine_id`-keyed named Docker volume;
- auto-configures 9 MCP clients (Claude Desktop, Claude Code, Cursor, Zed, Cline, Continue, Windsurf, Roo, Gemini CLI, LM Studio, Amp) by editing each client's settings file;
- supports autonomous install/update workflows with code signing (macOS notarised via Apple Developer; Windows Authenticode via Azure) and a built-in self-updater.

From cold start to fully operational: one command (`npx leapable install` or the installer executable). No cloud account creation, no API key management, no configuration files on the user side.

## 11.10 What the vendor sees vs. what the vendor does not see

Local MCP containers process all document data locally. The marketplace server never sees document content — only:

| Marketplace sees | Marketplace does NOT see |
|---|---|
| Email addresses (for account identity) | Document content |
| `machine_id` (hashed) | OCR output text |
| License key (`lk_` hash) | Embeddings |
| Ledger balance (in cents) | VLM descriptions |
| Per-query credit debits | Search queries (beyond their audit row count) |
| Per-page OCR/redaction counts (for billing) | File names or paths |
| Stripe customer / subscription IDs | Database contents |
| `queries.ai_client` (Claude / Cursor / etc. identifier) | Document PII (redacted locally) |

For the subclass of deployments requiring mathematically-provable zero egress, air-gap mode (`docker run --network=none`) disables billing and replaces it with site licensing — the same pattern used in the security-hardened variants of Context Graph.

## 11.11 Economic structure

The local-first commercial pattern produces near-100% margins at scale because each new user brings their own CPU and storage for the local path, and cloud OCR fan-out is pay-as-you-go RunPod Serverless (marginal cost proportional to usage, not user count).

| Cost item | Traditional hosted SaaS | Leapable |
|---|---|---|
| GPU compute (routine) | US$2–8/hr per user session | US$0 (user's CPU; embedding + reranker only) |
| Storage | ~US$0.023/GB/month (S3) | US$0 (user's local SQLite) |
| Bandwidth | ~US$0.09/GB (egress) | US$0 for local path; RunPod pass-through for cloud OCR |
| Cloudflare Workers / D1 | N/A | ~US$0.50 / million requests; first 25M D1 reads free |
| Stripe fees | 2.9% + US$0.30 / txn | Same |
| Infrastructure team | US$200K+/yr | US$0 (serverless marketplace + RunPod) |
| Compliance overhead | US$50K–200K/yr | US$0 (document data never leaves user; HIPAA-compatible by architecture) |

Revenue per subscriber is near 100% margin minus Stripe fees and RunPod pass-through on the per-page cloud OCR path. Creator revenue share (70%/75%) is a cost but it is also a growth lever: creators bring their own audiences, which is how the two-sided marketplace bootstraps network effects.

## 11.12 What this demonstrates for the paper

1. **Commercial viability of AI-agent-first, local-first marketplaces is real and measurable.** Leapable is a shipping product with paying creators and subscribers, 4,000+ tests, a hardened security architecture with triple-layer owner immutability, and a mature provenance chain.
2. **DDA-enriched tool surfaces distribute naturally via MCP.** With 161 tools exposed via JSON-RPC 2.0, AI agents can exploit the system's full decomposition (document structure, embeddings, VLM descriptions, provenance chains, cluster memberships, quality metrics, content-class gating) without being trained on Leapable-specific documentation. This is a meta-DDA argument: the product itself is consumed by AI through a machine-readable decomposition of its capabilities.
3. **The user-provided-compute-plus-serverless-burst pattern is the natural commercial distribution for DDA systems.** Context Graph and ClipCannon are architecturally analogous: per-project local databases, MCP tool surfaces, per-query or per-page metered billing via Cloudflare Workers + Stripe. Deploying them as Leapable-pattern marketplaces (local processing for the cheap path, RunPod Serverless for the expensive GPU path, append-only ledger for the financial events) produces the same economic structure.
4. **The two-sided marketplace is the natural distribution for DDA's content-creator story.** Creators produce the training-value-dense content. AI agents pay to consume it. The marketplace is the coordination layer that matches them, with a deterministic revenue split and a 5% reserve hold. The same pattern can ship identity-specific constellations (Section 6), voice-cloning pipelines (Section 9), and full talking-head avatar clones (Section 8) as creator offerings — each creator's identity is a product that AI agents can rent by the query.
5. **Air-gap deployments are compatible with commercial licensing.** For organisations requiring mathematically-provable zero egress (legal, healthcare, classified government), air-gap mode is supported. Billing is disabled in air-gap mode and replaced with site licensing, preserving the commercial model while satisfying the strictest regulatory requirements.

Section 12 closes with the broader positioning of this work under Teleox.ai and Leapable.ai together.
