# Glossary — Defined Terms (used densely throughout)

Per `compressionprompt.md` §4.4 / §4.25 — every term defined here can appear unexplained in the body of any other doc.

## 1. Project terms

| Term | Definition |
|---|---|
| **Polis** | The system being built; also a single deployed city instance of it. The codename. |
| **Citizen** | An account-holder. Never called "user." Citizenship is per-polis. |
| **Newcomer / Established / Elder / Inactive / Departed** | Lifecycle stages (`04` §2). |
| **Constellation** | A citizen's frozen multi-embedder fingerprint (`03`). Type: `Vec<(Slot, Embedding)>`; never flattened (I10). |
| **Slot** | One embedder position within a Constellation. 21 slots in the slate (`03` §3). |
| **Format** | One of the Five (Symposium / Agora / Gymnasium / Theater / Festival) + Aleatory. (`04` §3.) |
| **Gathering** | An instance of a Format — specific date/time/venue/attendees. |
| **Convener** | The citizen sortitioned to host a specific gathering. Rotating role. |
| **Welcoming Citizen** | Sortitioned citizen who conducts a Newcomer intake interview. |
| **Chevruta** | 2-citizen sustained reading pair, 8–12 week commitment (`04` §3 Theater-adjacent). |
| **Citizen Assembly** | 1,000-citizen sortitioned polis-parliament (`06` §1.1). |
| **SARB** | Sortitioned Algorithmic Review Board, 50-citizen central panel (`06` §1.2). |
| **Trustee Council** | Founder + named long-term stewards, veto-only on Tier-4 violations (`06` §1.5). |
| **Partner Venue** | Third-place venue (cafe, library, bistro, park, gym) with platform agreement (`06` §3). |
| **G_τ (Constellation Guard)** | Runtime gate that rejects matches missing required slots (`05` §3). |
| **TCT** | Teleological Constellation Training — frozen-target alignment discipline. Inherited from `docs/PAPER.md`. |
| **DDA** | Derived Data Abundance — multi-embedder × pairwise-correlation signal density. Inherited from `docs/PAPER.md`. |
| **Mistake-loop** | Trainer pattern that replays predictions vs. outcomes with EWC++ Fisher protection. Inherited from ContextGraph #406/#523. |
| **PoP** | Proof of Personhood credential. ZK-attested, unique-per-human (`04` §1, `06` §2). |
| **Tier-1 / Tier-2 / Tier-3 / Tier-4** | Civic outcomes / anti-objectives / depth / invariants (`00 §7`, `01`). |
| **Eudaimonia index** | 5-Likert quarterly survey covering arete, philia, sophia, civic, eros/agape (`04`). |
| **Bridging rerank** | Ovadya 2022 cross-cluster boost in ranking. |
| **MMR** | Maximal Marginal Relevance — diversification rerank. |
| **Aleatory** | The sortitioned-random format for Newcomers. Pure lot, no algorithm. |
| **eknomōs** | The Athenian "what is said here stays here" rule. Applies to Agora absolutely. |
| **isēgoria** | Equal speech-time. Enforced by Agora wooden-token protocol. |
| **paideia** | Lifelong cultivation of mind, character, judgment. The knowledge layer (`04` Theater + Chevruta + Teaching Circles). |
| **koinōnia** | Fellowship under shared inquiry. The platform's operating soul-word. |

## 2. Engineering terms

| Term | Definition |
|---|---|
| **FSV** | Forensic Source-of-truth Verification per `docs/futurebuild/specs/FSV-PROTOCOL.md`. SoT readback is the contract. |
| **CF** | RocksDB Column Family. |
| **MCP** | Model Context Protocol. JSON-RPC over stdio or HTTP. |
| **VRF** | Verifiable Random Function. Used for sortition draws. |
| **DP** | Differential Privacy. ε ≤ 1.0 on public aggregates; minimum cell size 30. |
| **HNSW** | Hierarchical Navigable Small World. Vector index per slot. |
| **RRF** | Reciprocal Rank Fusion. Cross-slot ranking aggregation. |
| **Conformal predictor** | Distribution-free uncertainty wrapper. Split-conformal preferred. |
| **EWC++** | Elastic Weight Consolidation, regularized variant. Catastrophic-forgetting protection. |
| **Asset** | Any image / SVG / icon / illustration / brand artifact codex generates via image tooling. |
| **SoT** | Source of Truth. The byte-on-disk / CF row / API response that authoritatively answers a question. |
| **Tier-4 invariant** | Hard fence in `01_INVARIANTS.md`. Architectural, not aspirational. |

## 3. Modal words (RFC 2119)

| Word | Force |
|---|---|
| **MUST** / **MUST NOT** / **REQUIRED** / **SHALL** / **SHALL NOT** | Absolute requirement |
| **SHOULD** / **SHOULD NOT** / **RECOMMENDED** | Strong default; deviation requires written justification |
| **MAY** / **OPTIONAL** | Truly optional |

Every modal word in these docs is RFC-2119-strict. Never paraphrase a `MUST` to a `should` (`compressionprompt.md` §6.9).

## 4. Symbols (used in symbolic notation per `compressionprompt.md` §4.6)

| Symbol | Meaning |
|---|---|
| `∈ / ∉` | member of / not member of |
| `⊆ / ⊃` | subset / superset |
| `∩ / ∪` | intersection / union |
| `¬ / ∧ / ∨` | not / and / or |
| `⇒ / ⇔` | implies / iff |
| `∀ / ∃` | for all / exists |
| `≥ / ≤ / ≠` | comparison |
| `→` | maps to / leads to |

## 5. Cross-doc reference

Refer to sibling docs by short number prefix: `(see 03)` = `03_SUBSTRATE.md`. No need to write full filename.

---

**Last updated:** 2026-05-24.
