# 00 — Combined ME-JEPA × Meaning-Compression Upgrade Plan: Master Index

This directory plans the integration of the *Meaning Compression / DDA / TCT*
framework from `docs/PAPER.md` into the existing Dynamic / ME-JEPA runtime
(`crates/context-graph-*` plus the verification ladder in
`docs/5090jepa/smokedocs/`). The integration is treated as a *single multi-
phase upgrade*, not as two independent tracks.

## Headline goal

> Convert each immutable ME-JEPA bundle into a **meaning-compressed audited
> execution trace** with cryptographic provenance: roughly 7× the supervision
> density per raw event, a frozen-constellation runtime guard
> $G_\tau$ replacing the current utility-only verdict, an explicit
> meaning-compression ratio in every bundle, and a pairwise mutual-
> information audit of the instrument panel as a first-class FSV harness.

## Why this matters in one sentence

ME-JEPA already provides the runtime, the audit log, and the deterministic
public-source corpus; PAPER.md provides the counting identity, the pairwise
$\binom{N}{2}$ term, and the per-output verifiability predicate. Composing
them costs no new primitives and yields a measurably stronger training-data
class.

## Status table

| Doc | Title | Status | Blocks |
| --- | --- | --- | --- |
| `00_master_index.md` | This file | drafted | — |
| `01_motivation.md` | Why combine, what's new, scope | drafted | — |
| `02_architecture.md` | Unified data flow + 3-role panel | drafted | 03–08 |
| `03_signal_yield_math.md` | Counting identity + MC ratio | drafted | 12 |
| `04_constellation_design.md` | $C_P$ object, Phase-1 protocol, persistence | drafted | 06, 09 |
| `05_pairwise_features_design.md` | $\binom{N}{2}$ feature catalog + computation | drafted | 07, 09, 10 |
| `06_guard_upgrade_g_tau.md` | Utility + $G_\tau$ combined guard | drafted | 04 |
| `07_storage_schema_changes.md` | New CFs, key formats, RocksDB descriptors | drafted | 13 |
| `08_cli_and_mcp_surface.md` | New subcommands + MCP tools | drafted | 13 |
| `09_fsv_extensions.md` | FSV-9 / 10 / 11 / 12 harnesses | drafted | 13 |
| `10_pairwise_mi_audit.md` | KSG / MINE estimator design + acceptance | drafted | 09 |
| `11_cross_domain_transfer.md` | counter\_world → gridworld zero-shot pilot | drafted | 13 (Phase 6) |
| `12_paper_tables_and_plots.md` | New tables / plots / aggregate columns | drafted | 13 |
| `13_implementation_phases.md` | 6-phase roadmap, deps, milestones | drafted | — |
| `14_test_plan.md` | Unit / integration / FSV test matrix | drafted | 13 |
| `15_risks_and_open_questions.md` | Risk register + decisions | drafted | — |
| `16_paper_v1_1_outline.md` | How the manuscript updates | drafted | — |
| `17_acceptance_criteria.md` | Per-phase + release-gate criteria | drafted | 13 |

## Read order

For implementers in a hurry:
1. `01_motivation.md` (why)
2. `02_architecture.md` (what)
3. `13_implementation_phases.md` (when)
4. `17_acceptance_criteria.md` (done)

For the deep dive, read in numeric order. `04`–`12` are the design specs;
`13`–`17` are the operational planning docs.

For reviewers / external readers: `01` + `02` + `03` + `16` give the
research-paper view in roughly 1{,}000 lines.

## Naming conventions

- **DDA** — Derived Data Abundance (PAPER.md §3).
- **TCT** — Teleological Constellation Training (PAPER.md §5).
- **MC ratio** — Meaning-compression ratio (signals per raw input).
- **$C_P$** — Constellation set; the frozen multi-modal centroid object.
- **$G_\tau$** — Per-output runtime guard predicate
  $\mathbb{1}[\forall m: \cos(\phi_m(\hat y), c_m) \geq \tau_m]$.
- **FSV** — Full-State Verification (existing ME-JEPA protocol).
- **ME-JEPA-DDA** — The combined runtime, this upgrade's product.

## Non-negotiables (carried from existing project rules)

These planning docs respect the project's existing rules:

- No backwards compatibility (CLAUDE.md): the bundle schema and RocksDB CF
  set are versioned; old DBs are wiped at the schema bump, not migrated.
- No fallback behaviour and no mock data (smokedoc 00): every new field is
  typed, every new CF is read back through a separate code path, every new
  edge case fails closed.
- No unauthorised destructive action: the schema bump is opt-in via
  `--schema-version 2`; the existing v1 bundles remain fully readable until
  the v2 release-D gate is cut.
- No claims of recruiting / hiring / fairness validity: §16 paper-update
  section preserves the scope discipline already in place.

## Glossary highlights

| Term | One-line definition |
| --- | --- |
| Slice 1 | Deterministic-from-public-source bytes of a bundle (largest, lowest-collapse-hazard slice). |
| Slice 2 | Tiny-model-output bytes paired with deterministic observed targets. |
| Slice 3 | Independent verifier-readback bytes (audit log + FSV reports). |
| Pairwise feature | Cross-instrument scalar / low-dim feature persisted per (event, instrument-pair). |
| Constellation $C_P$ | Frozen unit-norm centroid set $\{(m, c_m)\}_{m\in\mathcal{M}}$ for subject $P$. |
| MC ratio | $\mathrm{MC}(\Phi, D) = N + \binom{N}{2}$ structured signals per raw input. |
| $N_{\text{eff}}$ | Effective embedder count after pairwise mutual-information audit. |

## Cross-references

The plan inherits, modifies, or extends these existing docs:

- `docs/5090jepa/smokedocs/00_master_index.md` — overall ME-JEPA program.
- `docs/5090jepa/smokedocs/13_current_implementation_and_benchmark_status.md` —
  current per-CF row counts that this upgrade modifies.
- `docs/5090jepa/researchpaper/paper.tex` — the research manuscript that
  picks up the new contributions.
- `docs/PAPER.md` — the source for DDA / meaning compression / TCT.
- `backupdocs/dynamicjepa/06_latent_panel_and_binding.md` — already lists
  pairwise-feature *kinds* as ME-JEPA design intent; this upgrade lands
  them as first-class persisted records.
- `crates/context-graph-storage/src/dynamicjepa/column_families.rs` — the
  file that gains two new CFs in this upgrade.

## Out of scope for this plan

These are *not* covered by the documents in this directory and remain
deferred axes:

- Multi-step rollouts $H>1$ (smokedoc 24).
- Decoder head + top-k metrics (smokedoc 16).
- Real subject-level data (smokedoc 25).
- ESCO crosswalk (smokedoc 18).
- Anything in `docs/PAPER.md` related to Cases 2 (talking-head) and 3 (voice
  cloning); only Case-1-style text-style centroids and counter\_world /
  gridworld / career\_taxonomy domains are in scope.
