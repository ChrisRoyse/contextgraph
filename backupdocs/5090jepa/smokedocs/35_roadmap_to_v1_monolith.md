# 35 - Roadmap To v1.0 Monolith

## Goal

Sequence the path from current state (2026-04-30: paper-small 1K
passing, ClipCannon vendored, PAPER.md drafted, fragmenting product
docs deleted) to **v1.0 monolith release**: one repo, one binary, one
paper, one DOI, one product, one container.

Replaces the prior 18-month fragmented "products + papers" roadmap.
This is a single critical path with explicit gates.

## Current State (2026-04-30)

What is **done**:

- Context Graph runtime: ~370K LOC Rust; 13 embedders; 75 MCP tools;
  5,184+ passing tests.
- ClipCannon vendored at `clipcannon/`: 23-stage DAG; Phoenix avatar;
  VoiceAgent; license server; chrome ext (~67K LOC).
- ME-JEPA Phase 10 complete; smoke ladder passing through paper-small
  1K (`20260430T_paper_small_1k_v1`).
- PAPER.md drafted with Case 3 voice measured (0.961 SECS), Case 1
  Shakespeare 5/5 FSV.
- Smokedocs 14-35 specifying remaining work.

What is **not done**:

- Repo merge mechanics (smokedoc 32: top-level Makefile / pyproject /
  Dockerfile / .gitignore for vendored ClipCannon).
- Cross-language CI (smokedoc 23 + smokedoc 32).
- Unified MCP surface (smokedoc 33).
- Constellation Guard shared crate (smokedoc 28).
- DDA → ME-JEPA panel bridge (smokedoc 27).
- Unified bundle schema (smokedoc 29).
- MI audit (smokedoc 30).
- Multi-seed paper-small with BCa CI (smokedoc 15).
- Decoder head + top-k metrics (smokedoc 16).
- Paper-full 50K (smokedoc 17).
- ESCO replication (smokedoc 18).
- HP sweeps + collapse diagnostics (smokedoc 22).
- One-paper drafting (smokedoc 19).
- Reproducibility artifact + Zenodo + ACM badging (smokedoc 20).
- Compliance + ethics (smokedoc 21).
- v1.0 release ceremony (this doc).

## v1.0 Release Definition

The v1.0 release is **one event** that ships:

1. A tagged repo (`v1.0.0`) with the merged structure (smokedoc 32).
2. Two container images, both reproducible from the tagged commit:
   - Apptainer SIF, primary;
   - Docker image, mirrored.
3. One Zenodo DOI for the artifact.
4. One Zenodo DOI for the supplementary materials archive.
5. One arXiv preprint linked to both DOIs.
6. One landing page (Teleox brand per smokedoc 31).
7. One reference bundle (paper_full_50k, multi-seed × 5) whose root
   hash matches the published reference.
8. One announcement: blog post + YouTube video + arXiv submission +
   GitHub release notes — same day.

## Critical Path (16 Weeks)

The work is sized for a single operator running flat-out, with
parallelizable items called out where possible. Multi-author scenario
shaves ~25%.

### Weeks 1-2: Foundation (Repo + CI)

- M2 / M3 Repo merge mechanics (smokedoc 32):
  - Top-level `Makefile`, `pyproject.toml`, `Dockerfile`,
    `.gitignore`, `LICENSING.md`.
  - First clean `make build && make test` from repo root.
- M18 CI smoke regression (smokedoc 23):
  - Tier-A workflow file with cargo + uv;
  - Reference hash files for current paper_small bundle;
  - Self-hosted runner registered.

Gate: clean per-PR CI green on a trivial test PR.

### Weeks 3-4: Surface + Constellation

- M4 / M5 Unified surface (smokedoc 33):
  - Supervisor MCP: spawn Rust + Python sub-servers;
  - Tool namespace consolidation (`cg.<subsystem>.<verb>_<object>`);
  - Backward-compat aliases for existing CG/CC tool names.
- M7 Constellation Guard shared crate (smokedoc 28):
  - `crates/context-graph-constellation/` with `GuardDecision`
    struct;
  - Wired into ME-JEPA `predict` / `plan` / `surprise`;
  - Wired into ClipCannon voice + video generation via supervisor.

Gate: a single MCP session drives one DDA enrich + one ME-JEPA train
+ one voice generation + one guard evaluation through the unified
surface.

### Weeks 5-6: Provenance + Bridge

- M8 Unified bundle schema (smokedoc 29):
  - v2 bundle layout + `bundle_root_hash`;
  - `reviewer_recompute.sh`;
  - Cross-stage consistency checks.
- M6 DDA → ME-JEPA bridge (smokedoc 27):
  - `dda_signals` adapter;
  - `career_taxonomy.v2.toml` with DDA panel slots;
  - FSV gate 11 passing on a paper-small bundle.

Gate: one paper-small bundle in v2 schema with DDA-enriched panel and
verified `bundle_root_hash`.

### Weeks 7-8: Empirical Anchors

- M9 MI audit (smokedoc 30):
  - `dynamicjepa mi-audit` CLI;
  - Run on 3 corpora; record $k_{\text{eff}}$ in bundle.
- M10 Multi-seed paper-small (smokedoc 15):
  - 5 seeds (42, 43, 44, 45, 46) on paper-small 1K;
  - BCa bootstrap + paired sign-flip permutation tests;
  - Aggregate bundle written.
- M11 Decoder head (smokedoc 16):
  - Occupation classifier + skill-vector regressor;
  - Top-k + ECE in `paper_tables/`.

Gate: paper_tables for prediction metrics, ablation, top-k all
auto-generated and CI-protected.

### Weeks 9-11: Scaling + Replication

- M12 Paper-full 50K (smokedoc 17):
  - Walk graph expanded to full O*NET 30.2;
  - Multi-seed × 5 if budget allows; single-seed disclosed otherwise;
  - Headline numbers locked.
- M13 ESCO replication (smokedoc 18):
  - ESCO-derived walk + `paper_med_5k` per taxonomy;
  - Crosswalk decoder head;
  - Parity tables.
- M17 HP sweeps + collapse diagnostics (smokedoc 22):
  - LR / latent / batch one-axis sweeps;
  - Effective rank + alignment+uniformity figures.

Gate: paper-full 50K bundle with all paper tables; ESCO bundle
parallel; collapse-diagnostic figure F5 ready.

### Weeks 12-14: Paper + Compliance

- M14 One-paper drafting (smokedoc 19):
  - Sections 1-9 drafted from finalized bundles;
  - Appendices populated;
  - Tables auto-generated, no hand-typed numbers.
- M16 Compliance + ethics + broader impact (smokedoc 21):
  - NIST AI RMF mapping;
  - Risk register with owners;
  - Dual-use disclosure;
  - `LICENSE_ATTRIBUTION.md` at repo root.

Gate: complete paper PDF builds from the paper repo against a frozen
bundle; compliance package PDF generated from bundle data.

### Weeks 15-16: Release

- M15 Reproducibility artifact (smokedoc 20):
  - Apptainer + Docker reproducible build (twice = same digest);
  - Zenodo DOI minted (artifact + supplementary);
  - ACM Available + Functional + Reusable packet drafted.
- v1.0 release event:
  - Tag `v1.0.0`;
  - Container images published;
  - arXiv submitted with DOIs;
  - Landing page live at the chosen domain;
  - Announcement video shot + uploaded;
  - GitHub release notes published.

Gate: external reviewer reproduces the headline bundle from the
published Apptainer SIF and `reproduce.sh` on a clean machine; root
hash matches.

## Dependencies (Summary)

```text
Week 1-2  ── Foundation ──┐
                          ▼
Week 3-4  ── Surface + CG ┐
                          ▼
Week 5-6  ── Bundle + DDA bridge ┐
                                  ▼
Week 7-8  ── MI + multi-seed + decoder ┐
                                        ▼
Week 9-11 ── Scaling + ESCO + sweeps ┐
                                      ▼
Week 12-14── Paper + compliance ┐
                                 ▼
Week 15-16── Artifact + release  → v1.0
```

Each block depends on the previous. The only parallelizable items are
ESCO (M13) vs HP sweeps (M17) within weeks 9-11. Compliance (M16)
runs in background through weeks 12-14.

## Risk-Adjusted Scenarios

### Best case (16 weeks)

Everything above lands on schedule. v1.0 ships ~2026-08-22.

### Realistic case (20-22 weeks)

One major slip: paper-full 50K reveals a collapse problem requiring
HP retuning, OR ESCO source license requires additional review, OR
ACM artifact-evaluation packet requires a revision. v1.0 ships
~2026-09-19.

### Pessimistic case (28+ weeks)

Two slips: a refactor breaks the Rust ↔ Python supervisor, AND
multi-seed reveals seed-sensitive ablation behavior requiring
investigation (echo of seed-43 failure analysis). v1.0 ships
~2026-11-14.

In all scenarios, the **scope does not change**. The release date
slips, but the v1.0 content is fixed (one paper, one binary, one DOI,
one product). Scope creep is the existential risk.

## What Triggers v1.1

After v1.0 ships, v1.1 candidates (post-v1.0; not in this roadmap):

- Multi-step rollout (smokedoc 24).
- Phase 12 privacy gate productized (smokedoc 25).
- Hosted/managed deployment tier.
- Domain pack marketplace.
- Multi-GPU training.
- 250K event tier.

Each is a separate decision after v1.0 and not blocking.

## Success Metrics (12 Months Post-v1.0)

- arXiv preprint cited ≥30 times within 12 months.
- ≥50 GitHub stars within 6 months.
- ≥3 enterprise pilot conversations from inbound.
- ≥1 paying enterprise customer.
- Reference bundle reproduced by ≥3 external parties.
- Paper accepted at NeurIPS 2026 D&B / MLSys 2027 / ICLR 2027.

These are floors, not ceilings. The brand thesis (Teleox, third path
around the data wall) compounds over multi-year horizons.

## Acceptance

The v1.0 roadmap is **closed** when:

- repo tag `v1.0.0` exists;
- both container images are published and reproducible;
- both Zenodo DOIs are minted and resolve;
- arXiv preprint cross-cites both DOIs;
- one external party has reproduced the headline bundle and
  reported root-hash match.
