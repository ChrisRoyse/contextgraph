# 14 - Monolith Master Plan

This document is the master plan for the **single mega-system** we are
building. There is **one deliverable**: an integrated, source-of-truth-
verified, identity-locked, meaning-compressed AI infrastructure that
ships as one repository, one binary surface, one paper, one product.

Smokedocs `00-13` are the original ME-JEPA smoke program (still
canonical for scope, claims, evidence schema). Smokedocs `14-35`
specify what remains between current state (paper-small 1K passing,
ClipCannon vendored at `clipcannon/`, PAPER.md drafted) and the
v1.0 monolith release.

## What We Are Building (One Sentence)

> **One system that ingests any corpus, projects it through a fixed
> 13-text-embedder + 7-video-modality panel, trains identity-locked
> action-conditioned predictors with full source-of-truth provenance,
> validates every output against frozen geometric centroids at runtime,
> and ships as a single binary + MCP server + container — built on
> commodity high-end hardware (single RTX 5090, 32 GB VRAM).**

That one sentence covers Context Graph + ClipCannon + ME-JEPA + DDA +
TCT + Constellation Guard + FSV. It is the brand pitch and the
engineering scope statement at once.

## What "Done" Means For The Monolith

The v1.0 monolith ships when **all** of the following hold simultaneously:

1. **One repo** at `/home/cabdru/contextgraph/` containing every
   subsystem (Context Graph Rust workspace, ClipCannon Python at
   `clipcannon/`, ME-JEPA `dynamicjepa` crate, shared constellation
   crate, MCP server).
2. **One binary surface**: `context-graph` CLI subcommands cover every
   capability; one MCP server exposes every tool; one HTTP API mirrors
   it (smokedoc 33).
3. **One paper** (smokedoc 19) on arXiv with Zenodo DOI: covers DDA
   + Meaning Compression + ME-JEPA + FSV + TCT + Constellation Guard,
   measured end-to-end on the integrated system.
4. **One bundle schema** (smokedoc 29) carrying provenance from any
   raw corpus through any modality through any output.
5. **One product** (smokedoc 34) sold as the integrated platform — not
   a portfolio of unrelated SaaS products.
6. **All four FSV-grade workstreams** completed:
   - Multi-seed paper-small (smokedoc 15);
   - Decoder head + top-k metrics (smokedoc 16);
   - HP sweeps + collapse diagnostics (smokedoc 22);
   - Reproducibility artifact + ACM badging (smokedoc 20).
7. **Mutual-information audit** completed (smokedoc 30) so the
   91-signal upper bound has measured tightness.
8. **Constellation Guard** unified across ME-JEPA + ClipCannon
   (smokedoc 28) so every emitted output (text, voice, video, plan)
   passes the same per-output geometric test.
9. **Container** (smokedoc 20) builds reproducibly twice; reviewer
   recompute hash matches.
10. **CI smoke regression** (smokedoc 23) protects the whole monolith
    on every PR.

No fragmentation. No "voice product" separate from "compliance product"
separate from "agent platform". One thing.

## Anti-Goals (Explicit)

These are what the monolith is **not**:

- Not a portfolio of products that share infrastructure. The portfolio
  framing was rejected.
- Not three separate papers. One paper covers everything (smokedoc 19).
- Not a federation of microservices. The runtime is one process; the
  MCP server is one server; the CLI is one binary.
- Not a multi-repo with shared crates. One repo.
- Not "Context Graph powered by ClipCannon". Both names dissolve into
  one platform name (smokedoc 31 selects).
- Not a Phase 12 privacy-gated real-data pipeline at v1.0. Phase 12
  (smokedoc 25) is post-v1.0.
- Not a multi-step rollout (smokedoc 24) at v1.0. Optional appendix
  only.
- Not paper_full 250K (smokedoc 17). 50K is the v1.0 headline.

## Gap Matrix (Single Track)

One track. No "products" column. Every gap serves the v1.0 monolith.

| ID | Capability | Status | Plan Doc |
| --- | --- | --- | --- |
| M1 | ClipCannon vendored at `clipcannon/` | done (2026-04-30) | 32 |
| M2 | Repo structure: one workspace, one MCP, one CLI | not started | 32 |
| M3 | Cross-language build (Rust + Python in one repo) | not started | 32 |
| M4 | Unified MCP tool namespace across CG / CC / ME-JEPA | not started | 33 |
| M5 | One CLI binary covers every capability | not started | 33 |
| M6 | DDA → ME-JEPA panel bridge | not started | 27 |
| M7 | Constellation Guard for ME-JEPA outputs (shared crate w/ CC) | not started | 28 |
| M8 | Unified bundle schema across DDA / CG / CC / ME-JEPA | not started | 29 |
| M9 | Mutual-information embedder audit | not started | 30 |
| M10 | Multi-seed paper-small with BCa CI + paired tests | partial | 15 |
| M11 | Decoder head + top-k occupation + skill reconstruction | not started | 16 |
| M12 | Paper-full scaling ladder to 50K | not started | 17 |
| M13 | ESCO replication + crosswalk decoder | not started | 18 |
| M14 | One-paper drafting (covers DDA + ME-JEPA + TCT + FSV) | outline only | 19 |
| M15 | Reproducibility artifact (Apptainer + Docker + Zenodo + ACM) | not started | 20 |
| M16 | Compliance + ethics + broader impact | not started | 21 |
| M17 | HP sweeps + collapse diagnostics (rank, A+U) | not started | 22 |
| M18 | CI smoke regression + drift detection | not started | 23 |
| M19 | One-product GTM (replaces fragmented product portfolio) | not started | 34 |
| M20 | Roadmap to v1.0 release | not started | 35 |

20 gaps. One delivery date. One artifact.

## Dependency Graph

```text
M1 (CC vendored) ──> M2 (repo structure) ──> M3 (cross-lang build)
                                            │
                                            ▼
                          M4 (MCP) + M5 (CLI) ──┐
                                                │
M6 (DDA bridge) ──> M11 (decoder)               │
M7 (CG for ME-JEPA) ──┐                         │
M8 (unified bundle)  ─┤                         │
M9 (MI audit)        ─┤                         │
                      ├──> M10 (multi-seed) ────┤
                      │                         │
                      ▼                         │
                  M17 (HP sweeps + collapse) ───┤
                                                │
                                                ▼
                                         M14 (one paper)
                                                │
                                                ▼
                                         M15 (artifact + DOI)
                                                │
                                                ▼
                                         M19 (GTM)
                                                │
                                                ▼
                                         M20 (v1.0 release)

M18 (CI) ── protects all above ─ runs daily
M16 (compliance) ── unblocks v1.0 release
M12 (paper-full 50K) ── headline run for paper
M13 (ESCO) ── replication strength for paper
```

Critical path: **M1 → M2 → M3 → M4/M5 → M6 → M10 → M14 → M15 → M19 → M20.**

## Recommended Execution Order

| Order | Work | Why |
| ---: | --- | --- |
| 1 | M1 done; commit ClipCannon to repo | Already done; commit to lock state |
| 2 | M2 / M3 (repo structure + cross-lang build) | Nothing else can integrate without this |
| 3 | M18 (CI regression) | Defends every later step from drift |
| 4 | M4 / M5 (unified MCP + CLI) | Surface for everything else |
| 5 | M7 / M8 (Constellation Guard + bundle) | Required for any verifiable output |
| 6 | M6 / M9 (DDA bridge + MI audit) | Empirical anchor for paper claim |
| 7 | M10 (multi-seed) + M17 (HP sweeps) | Locks numerical floor |
| 8 | M11 (decoder head) | Adds top-k + skill reconstruction metrics |
| 9 | M12 / M13 (paper-full + ESCO) | Headline + replication for paper |
| 10 | M14 (one paper drafting) | Writes from finalized evidence |
| 11 | M15 (artifact + DOI + ACM badging) | Reviewer-ready release |
| 12 | M16 (compliance) | Required for any external buyer |
| 13 | M19 / M20 (GTM + release) | Ship |

## What Smokedocs 15-25 Become Under The Monolith

The earlier paper-readiness docs are still valid — each describes a
sub-task of the monolith. Their scopes are unchanged; only their
framing shifts from "paper" to "v1.0 monolith":

| Doc | Original Scope | Monolith Reframe |
| --- | --- | --- |
| 15 | Multi-seed for paper | Multi-seed for v1.0 evidence |
| 16 | Decoder head for paper | Decoder head for v1.0 capability |
| 17 | Paper-full scaling | Paper-full scaling for v1.0 headline |
| 18 | ESCO crosswalk | ESCO replication for v1.0 portability claim |
| 19 | Paper drafting | **One-paper** drafting (refactored) |
| 20 | Reproducibility artifact | Reproducibility artifact for v1.0 release |
| 21 | Compliance + ethics | Compliance baked into v1.0 |
| 22 | HP sweeps | HP sweeps for v1.0 evidence |
| 23 | CI smoke regression | CI for monolith |
| 24 | Multi-step rollout | Optional v1.1 |
| 25 | Privacy gate Phase 12 | Post-v1.0 |
| 26 | Closed-loop architecture | **Monolith architecture** (refactored) |
| 27 | DDA → ME-JEPA bridge | Same; in-monolith integration |
| 28 | Constellation Guard for ME-JEPA | Shared crate across CG / CC / ME-JEPA |
| 29 | Unified bundle schema | Single bundle for monolith |
| 30 | MI audit | MI audit for monolith |

## What Smokedocs 31-35 Are (New Monolith Docs)

| Doc | Scope |
| --- | --- |
| 31 | Monolith definition + brand + scope |
| 32 | Merged repo structure (Rust + Python + ClipCannon vendored) |
| 33 | Unified surface (one CLI, one MCP, one HTTP API) |
| 34 | Single-product GTM (replaces 5 fragmented product docs) |
| 35 | Roadmap to v1.0 release |

## Deletions

Six docs were written and then deleted because they fragmented the
work into separate products:

- `31_voice_cloning_saas_productization.md` — DELETED
- `32_dda_as_a_service_platform.md` — DELETED
- `33_eu_ai_act_compliance_product.md` — DELETED
- `34_identity_locked_agent_platform.md` — DELETED
- `35_open_core_hosted_platform.md` — DELETED
- `36_three_paper_joint_roadmap.md` — DELETED

Their themes are subsumed into the new 31-35. Capabilities are
preserved (voice cloning, agent runtime, compliance evidence are all
features of the monolith). What is gone is the **product fragmentation
framing**.

## Cross-References

- `docs2/PAPER.md` — DDA + Meaning Compression + TCT manuscript
- `docs2/alldata.md` — ClipCannon + Phoenix + VoiceAgent architecture
- `docs/5090jepa/00_TASK.md` — original ME-JEPA charter
- `clipcannon/` — vendored ClipCannon tree (post-2026-04-30)
- `crates/context-graph-dynamicjepa/` — ME-JEPA runtime
