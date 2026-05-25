# Technical Architecture

This document specifies the implementation stack, the integration with the existing Royse stack (ContextGraph, ME-JEPA-Code, ClipCannon), the deployment topology, and the security/privacy substrate. It is opinionated: the platform is built on what already works, not on a clean-slate stack.

## 1. The reuse thesis

ContextGraph has shipped — over the past 18 months — a 22-crate Rust workspace with: 13 frozen embedders (51 RocksDB column families, ~ 400K LOC), an MCP server with 54 live tools, an audit-log + provenance substrate with operator attribution and lifecycle history, an HNSW + RRF + MMR + conformal predictor stack, a teleological-constellation guard (`G_τ`) implementation, a mistake-loop replay buffer, and a fail-closed durable-path resolver. ClipCannon has shipped a 7-modality 4,044-D multi-modal constellation pipeline (prosody, visual, OCR, transcript, audio, semantic, structural).

**The serendipity engine is a new application of this stack, not a new stack.** Building it is approximately 4-6 new Rust crates, ~ 40-60K LOC, plus a new MCP tool surface, a new mobile/web client, and a new venue-partnership operations layer — atop the existing substrate.

This is the same principle as ME-JEPA being built on ContextGraph rather than from scratch: solve the next problem by *adding* primitives to the proven substrate, never by *replacing* it.

## 2. The service architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                    Citizens (mobile / web client)                    │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  Minimal UI: next event • pre-read • reflection prompt       │    │
│  │  No feed, no profiles, no infinite scroll (per Invariants)   │    │
│  └─────────────────────────┬────────────────────────────────────┘    │
└──────────────────────────────┼─────────────────────────────────────────┘
                               │ HTTPS + JSON-RPC
┌──────────────────────────────▼─────────────────────────────────────────┐
│  context-graph-mcp (MCP server, existing crate, extended tool set)     │
│                                                                        │
│  New tool families:                                                    │
│   • polis_citizen_*                  (onboarding, profile, exit)       │
│   • polis_gathering_*                (invite, RSVP, attendance, drop)  │
│   • polis_reflection_*               (post-event + 30d feedback)       │
│   • polis_agora_*                    (proposition, registration)       │
│   • polis_chevruta_*                 (pair lifecycle)                  │
│   • polis_governance_*               (sortition draws, votes, audit)   │
│                                                                        │
└─┬──────────────────────────────────────────────────────────────────────┘
  │
  ├──────────────────────────────────────────────────────────────┐
  │                                                              │
  ▼                                                              ▼
┌──────────────────────────────────┐    ┌──────────────────────────────┐
│ polis-matcher (NEW crate)        │    │ context-graph-mejepa-*       │
│                                  │    │ (existing — frozen-target    │
│ • candidate generation (HNSW)    │◀──▶│  Constellation + G_τ guard + │
│ • MMR + bridging rerank          │    │  mistake-loop)                │
│ • G_τ guard                      │    │                              │
│ • capacity allocation (Gale-     │    │ • EmbedderForward trait      │
│   Shapley with fairness floor)   │    │ • PanelBuilder               │
│ • mistake-loop consumer          │    │ • TCT G_τ                    │
└─┬────────────────────────────────┘    └──────────────────────────────┘
  │
  ▼
┌──────────────────────────────────┐    ┌──────────────────────────────┐
│ polis-governance (NEW crate)     │    │ context-graph-witness        │
│                                  │◀──▶│ (existing — ed25519 + Merkle │
│ • VRF-based stratified sortition │    │  segment signing)             │
│ • SARB workflow                  │    │                              │
│ • Citizen-Assembly tally         │    │ • cert.rs (Stage 0/F certs)  │
│ • Tier-4 amendment state machine │    │ • Merkle compression         │
└──────────────────────────────────┘    └──────────────────────────────┘
                               │
                               ▼
┌────────────────────────────────────────────────────────────────────────┐
│ context-graph-storage (existing) + RocksDB                              │
│                                                                         │
│ New CFs:                                                                │
│   CF_CITIZEN_CONSTELLATION       (per-citizen 13-embedder fingerprint)  │
│   CF_CITIZEN_PROVENANCE          (every embedder forward + operator)    │
│   CF_CITIZEN_ATTESTATIONS        (in-person PoP attestations)           │
│   CF_GATHERING_CANDIDATES        (matcher's candidate pools)            │
│   CF_GATHERING_INVITES           (issued invitations)                   │
│   CF_GATHERING_OUTCOMES          (attendance, reflections, tie signals) │
│   CF_MATCHING_WEIGHT_HISTORY     (every algorithm weight change + SARB) │
│   CF_PROPOSITION_QUEUE           (Agora propositions + drafting status) │
│   CF_AGORA_MIND_CHANGE           (aggregate per-Agora outputs)          │
│   CF_CHEVRUTA_PAIRS              (active + historical pairings)         │
│   CF_GOVERNANCE_SORTITION_LOG    (every sortition draw, VRF verifiable) │
│   CF_GOVERNANCE_VOTES            (Citizen Assembly + SARB tallies)      │
│   CF_VENUE_PARTNERSHIP           (Partner Venue registry + invoices)    │
│   CF_CIVIC_FUND_LEDGER           (catalyst grants, Solidarity-tier)     │
│                                                                         │
│ Reuses 51+ existing CFs, including all 13 embedder index spaces         │
└────────────────────────────────────────────────────────────────────────┘
```

## 3. The new Rust crates (4-6)

### 3.1 `polis-matcher`

Owns the serendipity algorithm of `07`. Consumes Constellations from `CF_CITIZEN_CONSTELLATION`, emits invitations into `CF_GATHERING_INVITES`. Implements:

- Coarse HNSW recall (reusing `context-graph-storage`'s 15 HNSW indexes).
- MMR diversification (reusing `context-graph-solver/mmr`).
- Bridging-based rerank (new module; ports Ovadya 2022 + Polis 2014 algorithms).
- `G_τ` guard (reusing `context-graph-mejepa-tct`).
- Capacity-aware Gale-Shapley with fairness floor (new module).
- Mistake-loop consumer (reusing pattern from ContextGraph #406/#523).

### 3.2 `polis-governance`

Owns sortition + voting + amendment workflows of `14`. Implements:

- Stratified VRF-based draws (BLS12-381 VRF, reusing `ed25519` + new BLS primitives).
- Citizen Assembly + SARB tally (transparent, auditable, reproducible).
- Tier-4 amendment state machine (90-day-publish → SARB → assembly-supermajority → 12-month-moratorium → re-vote → trustee-concurrence).
- Stratification validation (rejects unbalanced draws).

### 3.3 `polis-mcp-tools`

A thin extension crate that adds the `polis_*` tool families to the existing `context-graph-mcp` server. Same MCP transport, same audit-log discipline, same operator-attribution semantics. Adds:

- `polis_citizen_intake_start` / `polis_citizen_intake_complete`
- `polis_citizen_constellation_export` (citizen's own data only)
- `polis_citizen_request_deletion`
- `polis_gathering_invite_accept` / `polis_gathering_invite_decline`
- `polis_gathering_attendance_attest`
- `polis_reflection_24h` / `polis_reflection_30d`
- `polis_agora_propose` / `polis_agora_register` / `polis_agora_reregister`
- `polis_chevruta_express_interest` / `polis_chevruta_complete`
- `polis_governance_sortition_audit` (read-only)
- ... and ~ 30-50 more

### 3.4 `polis-client-api`

The HTTP layer the mobile / web client talks to. Thin shim over MCP for browser-compatible auth + session. Implements:

- WebAuthn / passkey-based citizen auth (post-PoP).
- Push notifications (only: next-event, RSVP-needed; never feed-style).
- Session-length tracking with hard cap (alerts citizen at 5min and forces a "take a break" after 10min).

### 3.5 `polis-venue-ops` (optional separate crate or part of `polis-governance`)

Partnership lifecycle, billing, group-rate negotiations, fee remittance, partner-member governance interface.

### 3.6 `polis-analytics` (read-only)

Differential-privacy aggregate analytics generation for SARB and Citizen Assembly reports. Zero per-citizen data exposure. Reuses ContextGraph aggregate facilities.

## 4. The mobile / web client

Web first (PWA), iOS + Android wrappers second. Stack:

- Web: TypeScript + SvelteKit (or React with strict server-component discipline) — chosen for *small* bundle size and zero infinite-scroll components.
- Mobile: thin wrapper around the web app (Capacitor) with native push for invitations.
- Auth: WebAuthn / passkeys + the platform's PoP credential.
- No analytics SDK (no Mixpanel, no Amplitude, no GA). Server-side aggregate only.
- No third-party tracking pixels.
- No advertising SDK, ever.

UI specification: ~ 4 screens total (home, next-event, pre-read, reflection), each ≤ 1 viewport tall, no scroll on home. UI palette deliberately understated — closer to a public-library card UI than to TikTok.

## 5. Privacy + security architecture

### 5.1 Encryption-at-rest

Per `05` §6 + `08` §7:

- Citizen Constellations encrypted with per-citizen keys derived from PoP credential.
- Server-side key wrapping uses HSM (production: AWS Nitro or GCP Confidential Computing).
- Match-computation runs in an isolated process with ephemeral key access (no long-lived plaintext on disk or in cross-request memory).

### 5.2 Encryption-in-transit

- All client-server traffic TLS 1.3, HPKE-protected per JSON-RPC payload.
- DM channel (per `04` §6 Path B): client-side end-to-end Signal protocol, server holds only ciphertext.

### 5.3 Differential privacy on aggregates

- Any public aggregate computed with ε ≤ 1.0 differential privacy and a minimum cell size of 30 citizens (per `08` §9).
- Per-cell aggregates intended for SARB use a higher ε budget (≤ 3.0) but with stronger downstream-disclosure controls.

### 5.4 Audit log

- Every state change writes to `CF_AUDIT_LOG` (existing ContextGraph CF) with operator attribution, timestamp, before/after hashes, and policy version.
- Append-only, RocksDB-secondary-indexed by (operator, time-window, entity).
- Citizen-pullable audit trail of their own data via `polis_citizen_audit_export`.

### 5.5 Right-to-be-forgotten

- One MCP call (`polis_citizen_request_deletion`) initiates a cascade:
  - All `CF_CITIZEN_*` rows for the citizen are flagged for deletion.
  - Within 24 hours: physical deletion of Constellation + provenance + attestations + DM ciphertext.
  - Aggregate event-outcome counts retain anonymized stub records (no PII) for algorithmic learning per `05` §6.
- Signed deletion receipt returned to citizen.

### 5.6 Threat model

Per `08` §1 (T1-T9) + standard infosec: assume nation-state adversary may target the social-graph data; assume insider threat. Defenses: zero-knowledge PoP, encrypted-at-rest with per-citizen keys, sortitioned-review-board oversight of staff access, mandatory two-person rule for any privileged data access, quarterly penetration tests by independent firm (Trail of Bits or equivalent).

## 6. Deployment topology

### 6.1 Per-polis deployment

Each polis runs:

- 1 primary `context-graph-mcp` instance + 2 standby (Raft consensus).
- RocksDB-on-aiwonder-equivalent (NVMe + zfs/snapshot retention) — production durable state per ContextGraph `/zfs/archive/contextgraph/...` convention.
- Stateless polis-matcher workers (autoscale per matching window).
- Stateless polis-client-api workers (autoscale per traffic).
- Per-polis isolation: a polis's RocksDB is independent; cross-polis queries (only SARB-level aggregates) go through `polis-analytics` differential-privacy layer.

### 6.2 Central cooperative deployment

- The Constellation embedders (which are large pretrained models) are deployed centrally (cost amortization) but expose only embedder-forward as MCP calls; raw model weights and citizen-decryptable Constellations never leave the polis's local storage.
- SARB workflow + Tier-4 amendment state machine: centrally hosted, replicated.
- Cross-polis affiliate billing + central cooperative ops: centrally hosted.

### 6.3 Disaster recovery

- Per-polis daily snapshots; weekly cross-polis backups to encrypted archive.
- Recovery time objective: 4 hours for any single polis.
- Recovery point objective: 1 hour.
- Annual disaster-recovery drill (full-rollover simulation), as per ContextGraph hardening discipline.

## 7. Operations + observability

- Reuse ContextGraph's existing telemetry (events, structured logs, metrics) discipline.
- Per-citizen observability is strictly local (latency, error rates) — never derived to behavioral insights.
- Aggregate dashboards (per-polis Tier-1/2/3 metric movement) are SARB- and Citizen-Assembly-visible; per-citizen dashboards never exposed.
- 14-axis hardening per `docs2/hardeningprompt.md` applied identically.

## 8. Open-source posture

The platform's code is fully open-source under a license aligned with the cooperative purpose (AGPL-v3 + a CommonsClause-equivalent supplementary license that prevents proprietary re-launch by others against the Tier-4 invariants). All algorithm code, all UI code, all governance state machines, all audit-log schemas are public, reproducibly built, and externally verifiable.

This is itself a defense against capture: even if the central cooperative were captured, any city's citizens could fork and run their own polis under the same code.

## 9. Standards & interop

- MCP (Model Context Protocol) for all programmatic surfaces — reusing existing ContextGraph adoption.
- ActivityPub-compatible federation for cross-polis governance messages (not for feed-style content — there is no feed).
- DIF / W3C decentralized identifier (DID) standards for PoP credential portability across polises.
- Standard calendar (iCal / WebDAV) export for citizens who want their gathering invitations in their existing calendar app.

## 10. What we don't build

- **No native mobile-only app** (web-first; mobile is a wrapper).
- **No bespoke ML training pipeline for the embedders.** All 13 embedders are pretrained models; we never retrain in the live loop (Invariant adjacent: the same frozen-target discipline as ContextGraph §6.12).
- **No blockchain / smart contract.** VRF for sortition is a primitive, not a chain.
- **No "AI assistant" for citizens.** No LLM-mediated DMs, no AI-generated bios. (Invariant I15.)
- **No social-graph-export API for third parties.** (Invariant I13 + I14.)
- **No A/B testing harness** that runs without sortitioned-review-board approval per `07` §8.

## 11. The build estimate

- New crates (4-6 above): ~ 40-60K LOC Rust.
- Client (PWA + mobile wrappers): ~ 30-50K LOC TypeScript.
- Operations + DevOps + venue-ops backend: ~ 10-20K LOC.
- Documentation, FSV harness, audit harness: ~ 5-10K LOC.
- Total: ~ 100-150K LOC of *new* code, atop the ~ 400K LOC of *existing* ContextGraph + ClipCannon substrate.

Engineering team estimate: 4-6 engineers, 12-18 months to v1, modeled on a comparable cooperative-platform stack (e.g., the Lemmy/Mastodon teams reached parity in similar windows).

## 12. Cross-references

| Topic | Doc |
|---|---|
| The 13-embedder Constellation it consumes | `05_THE_CONSTELLATION_DDA_LAYER.md` |
| The matching algorithm being implemented | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` |
| The governance state machine being implemented | `14_GOVERNANCE_AND_SORTITION.md` |
| The roadmap with technical milestones | `18_ROADMAP_FROM_ZERO_TO_POLIS.md` |
| Hardening discipline | `docs2/hardeningprompt.md` (project root) |
| ContextGraph reuse | `CLAUDE.md` (project root) |
| ClipCannon reuse | `docs/alldata.md` (project root) |

---

**Last updated:** 2026-05-24.
