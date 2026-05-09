# SITREP — ME-JEPA-Code Implementation State

> **🔁 PIVOT 2026-05-09 — single reality loop, Claude Code is the SWE-bench actor, ME-JEPA is the product.**
> Read [`../10_MEJEPA_PIVOT_2026_05_09.md`](../10_MEJEPA_PIVOT_2026_05_09.md) and
> [`../11_MEJEPA_LEARNING_FORMULA.md`](../11_MEJEPA_LEARNING_FORMULA.md) before this sitrep. The
> two-loop "outer Claude + inner Qwen" architecture is **superseded**. There is now ONE reality
> loop, around Claude Code, and its sole purpose is generating training data for ME-JEPA.
> Sitrep status entries that still mention "Qwen CoT" / "inner-actor CoT" / "qwen3.6-27b" are
> either historical (kept for audit) or queued for re-framing as "AI agent reasoning trace"
> (Claude Code, external agents, human reviewers).

> **Source of truth** for what's shipped, what's deferred, and what to build
> next across the ContextGraph workspace toward the ME-JEPA-Code reality
> predictor. Updated 2026-05-09.

## Master plan reference

The 12 ruvectorfindings docs (`00_INDEX.md` … `11_MEJEPA_LEARNING_FORMULA.md`)
together specify the system, in this priority order:

1. **`10_MEJEPA_PIVOT_2026_05_09.md`** — current operating contract (single loop, Claude Code
   is the actor, cross-language mandate, self-healing protocol, /loop.md refactor directive,
   UTML v1.0 learning formula reference; the central architectural claim is that ME-JEPA's
   frozen externally-defined instruments make `ΔΞ.target_collapse = 0` by construction —
   a property no other JEPA in the literature has).
2. **`11_MEJEPA_LEARNING_FORMULA.md`** — operational specification of UTML v1.0's four-factor
   `L_step = ΔP × ΔK × ΔΩ × ΔΞ` for per-example training weighting + sampling + consolidation.
   Includes the per-step training certificate format, edge-case audit, and implementation
   roadmap aligned with doc 09 phases.
3. **`09_END_GOAL_REVIEW_REPLACEMENT.md`** — build-ready master plan (instruments, panel,
   predictor, EMA, VICReg, oracle head, conformal calibration, OOD, hygiene, 8-phase plan).
   Sections that reference Qwen / inner-actor are re-framed by doc 10.
4. **Docs 00-08** — reference material for *how* primitives are clean-room reimplemented.

This `sitrep/` directory tracks the implementation state of doc 09's
8 phases plus doc 05's 3 tiers, file-by-file, post-pivot.

## The three operator questions ME-JEPA must answer (verbatim 2026-05-09)

1. Does what the AI said it did **EXIST**? (claim ↔ source-of-truth SHA chain)
2. Does it **WORK** as intended? (predicted oracle ↔ actual oracle)
3. How is it actually **IMPACTING REALITY**? (predicted side-effects ↔ observed shifts)

Every shipped primitive and every deferred plan in this sitrep ladders up to one of these three
questions for ANY supported language (Python, Rust, C, C++, Java, C#, JS, TS, Ruby).

## Directory layout

```
sitrep/
├── README.md                                 # this file
├── 01_status_overview.md                     # canonical status table (every shipped + deferred item)
├── 02_shipped/                               # detail per shipped primitive
│   ├── README.md
│   ├── tier1_foundations.md                  # MMR, conformal, ed25519, witness chain core, solver foundations
│   ├── tier2_foundations.md                  # Stoer-Wagner mincut, Granger F-test, Stage 0/F + Merkle certs
│   ├── tier3_causal_mask.md                  # MaskStrategy + retrocausal-safety guard + MCP root-cause fix
│   ├── tier1_3_tier_compression.md           # 8/7/5/3-bit groupwise quantization
│   ├── phase0_mutation_operators.md          # 8 mutation categories
│   ├── phase0_corpus_split.md                # repo-atomic train/cal/holdout split
│   └── phase1_scaffolding_and_e_oracle.md    # Instrument trait + Panel + E_Oracle
├── 03_deferred/                              # detail per deferred primitive
│   ├── README.md
│   ├── tier_remaining.md                     # doc 05 Tier 1.5, 1.2 BMSSP, 2.1, 2.2, 2.5, 3.1, 3.3-3.6
│   └── phases/                               # detail per doc-09 phase (0..8)
│       ├── phase0.md                         # Phase 0 remaining (Docker, RocksDB CFs, full corpus gen)
│       ├── phase1.md                         # Phase 1 remaining instruments (13 of 14)
│       ├── phase2.md                         # Phase 2 — panel + predictor + EMA + VICReg
│       ├── phase3.md                         # Phase 3 — training loop + AdamW + per-step certs
│       ├── phase4.md                         # Phase 4 — inference + conformal calibration + OOD
│       ├── phase5.md                         # Phase 5 — self-healing operations
│       ├── phase6.md                         # Phase 6 — disk and memory hygiene
│       ├── phase7.md                         # Phase 7 — reality-loop integration
│       └── phase8.md                         # Phase 8 — evaluation against end goal
└── 04_implementation_plans/                  # ready-to-execute plans for the next 5 primitives
    ├── README.md
    ├── plan_e_witness.md                     # next-priority instrument
    ├── plan_e_static_analysis.md
    ├── plan_phase0_docker_oracle.md
    ├── plan_phase2_predictor.md
    └── plan_phase6_storage_wiring.md
```

## Quick links

- **Canonical status table**: `01_status_overview.md` — every doc 05
  Tier and doc 09 Phase item with shipped/deferred status, crate path,
  test count, FSV evidence path.
- **Shipped primitives**: `02_shipped/` — per-primitive deep-dives.
- **Deferred phases**: `03_deferred/phases/` — full per-phase explanations
  including goal, deliverables, DoD, verification, dependencies, current
  status, critical path, and implementation order.
- **Next 5 implementation plans**: `04_implementation_plans/` —
  ready-to-execute plans with deliverables + DoD + design notes.

## High-level state (as of 2026-05-09)

- **207 tests pass** across 6 ME-JEPA-foundation crates.
- **Doc 05 Tiers**: Tier-1 mostly complete (1.1, 1.2 partial, 1.3, 1.4,
  1.6); Tier-2 partial (2.3, 2.4, 2.6 shipped; 2.1, 2.2, 2.5 deferred);
  Tier-3 partial (3.2 shipped; 3.1, 3.3-3.6 deferred); Tier-4 not in
  scope.
- **Doc 09 Phases**: Phase 0 partial (mutation operators + split shipped;
  Docker oracle + RocksDB + full corpus gen deferred); Phase 1 just
  started (trait + Panel + E_Oracle shipped; 13 instruments deferred);
  Phases 2-8 fully deferred.

## How to use this directory

- **If you're an operator** auditing what's been done: start with
  `01_status_overview.md`.
- **If you're a fresh AI agent** picking up implementation: start with
  `04_implementation_plans/README.md` and execute the highest-priority
  plan.
- **If you want full phase context**: read `03_deferred/phases/phaseN.md`
  for the phase you're picking up.
- **If you're investigating a shipped primitive**: read the relevant
  `02_shipped/` file for its location, test counts, FSV path, and
  design notes.
