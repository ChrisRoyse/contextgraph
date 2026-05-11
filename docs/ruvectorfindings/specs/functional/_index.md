# Functional Specs Index (Level 2)

> **Level 2 — what to build.** Each spec captures user stories, requirements (`[REQ-<DOMAIN>-NN]`), edge cases, error states, and test plan. Implementation details live in the matched Level 3 technical spec.

**Authority:** `../constitution.md` overrides. When this index conflicts with constitution, constitution wins.

---

## Spec inventory (13 functional specs) — **status refreshed 2026-05-11**

| ID | File | Phase | Status (2026-05-11) | REQ count |
|---|---|---|---|---|
| SPEC-CORPUS | [SPEC-CORPUS.md](./SPEC-CORPUS.md) | 0 | partial — Python operators+split+oracle bridge+CLI shipped; 300×8 batch ([#5](https://github.com/ChrisRoyse/contextgraph/issues/5)) + 10 non-Python langs ([#7](https://github.com/ChrisRoyse/contextgraph/issues/7)) pending | 16 |
| SPEC-AST-CHUNKING | [SPEC-AST-CHUNKING.md](./SPEC-AST-CHUNKING.md) | 1a | operational — 11-language `context_graph_core::memory::ast` API shipped; legacy `AstCodeChunker` convergence ([#6](https://github.com/ChrisRoyse/contextgraph/issues/6)) + reformat-invariant audit ([#12](https://github.com/ChrisRoyse/contextgraph/issues/12)) pending | 15 |
| SPEC-EMBEDDERS | [SPEC-EMBEDDERS.md](./SPEC-EMBEDDERS.md) | 1b | control surface hardened — 21-embedder enum + manifest + VRAM gate + routing wrapper shipped; neural forward wrappers pending ([#3](https://github.com/ChrisRoyse/contextgraph/issues/3)) | 12 |
| SPEC-INSTRUMENTS | [SPEC-INSTRUMENTS.md](./SPEC-INSTRUMENTS.md) | 1c | scaffold shipped — all 15 panel slots have deterministic encoders + Panel/PanelBuilder/PanelStore + linear-probe gate; embedder/analyzer-backed heads pending ([#4](https://github.com/ChrisRoyse/contextgraph/issues/4)) | 23 |
| SPEC-PREDICTOR | [SPEC-PREDICTOR.md](./SPEC-PREDICTOR.md) | 2 | **shipped** — `MeJepaPredictor` + VICReg + OracleHead + FrozenTargetAdapter + `predictor_fsv` | 18 |
| SPEC-TRAINING | [SPEC-TRAINING.md](./SPEC-TRAINING.md) | 3 | **shipped** — Trainer + AdamW + cosine LR + sampler + cert chain + holdout eval + LoRA + distill; 5 FSV examples | 12 |
| SPEC-UTML | [SPEC-UTML.md](./SPEC-UTML.md) | 3 (cross-cutting) | shipped / safety-hardened — pairwise MI + continual-backprop + state-transfer T shipped; formula-carrier reconciliation tracked in [#13](https://github.com/ChrisRoyse/contextgraph/issues/13) | 12 |
| SPEC-INFERENCE | [SPEC-INFERENCE.md](./SPEC-INFERENCE.md) | 4a | **shipped** — `MeJepaCompiler::{compile, verify, observe}` + conformal + OOD + Verdict enum + `mejepa_verify` MCP tool | 20 |
| SPEC-TCT | [SPEC-TCT.md](./SPEC-TCT.md) | 4b (cross-cutting) | **shipped** — full TctConstellation + Gτ + GuardRejected + shrinkage + freshness + calibrator + `mejepa_constellation_inspect` MCP tool | 15 |
| SPEC-SELF-HEALING | [SPEC-SELF-HEALING.md](./SPEC-SELF-HEALING.md) | 5 | **shipped** — 16-module `heal/` tree + `mejepa-heal-drill` binary + 6 RocksDB CFs + `mejepa_heal_status`/`mejepa_rollback_to` MCP tools | 20 |
| SPEC-HYGIENE | [SPEC-HYGIENE.md](./SPEC-HYGIENE.md) | 6 | shipped — hygiene crate + tier transitions + quota + Merkle compression + nightly GC + 3 MCP tools; production scheduling tracked in [#8](https://github.com/ChrisRoyse/contextgraph/issues/8) | 10 |
| SPEC-SHIFT-SUBSCRIBER | [SPEC-SHIFT-SUBSCRIBER.md](./SPEC-SHIFT-SUBSCRIBER.md) | 7 | shipped — restart-safe subscriber + watermark CF + lag/panic telemetry + 3 MCP tools; production p50/p99 evidence tracked in [#11](https://github.com/ChrisRoyse/contextgraph/issues/11) | 18 |
| SPEC-EVALUATION | [SPEC-EVALUATION.md](./SPEC-EVALUATION.md) | 8 | harness shipped — ship gate + active-learning + graph + weekly report + 4 MCP tools; full 300×8 rolling evidence tracked in [#5](https://github.com/ChrisRoyse/contextgraph/issues/5) | 20 |

**Total:** 211 numbered requirements across 13 domains.

---

## Cross-cutting concerns

The following requirements span multiple specs. The "owner" spec is the primary author; the "consumers" reference the requirement.

| Concern | Owner spec | Consumer specs |
|---|---|---|
| Per-chunk-type embedder routing table | SPEC-AST-CHUNKING + SPEC-EMBEDDERS | SPEC-INSTRUMENTS, SPEC-SHIFT-SUBSCRIBER |
| Frozen-target structural collapse-immunity | SPEC-INSTRUMENTS | SPEC-PREDICTOR, SPEC-TRAINING, SPEC-UTML |
| `L_constellation_centroid` loss term | SPEC-TCT | SPEC-TRAINING |
| Gτ guard at verify() | SPEC-TCT | SPEC-INFERENCE |
| `constellation_violation_rate` for ΔΞ | SPEC-TCT | SPEC-UTML |
| `L_step` per-step training certificate | SPEC-UTML | SPEC-TRAINING, SPEC-SHIFT-SUBSCRIBER |
| `mejepa.observe()` body | SPEC-SELF-HEALING | SPEC-INFERENCE, SPEC-SHIFT-SUBSCRIBER |
| Witness-chain integrity check | SPEC-HYGIENE + SPEC-SELF-HEALING | all phases |
| Tier compression integration | SPEC-HYGIENE | SPEC-CORPUS (panel persist), SPEC-INFERENCE (CF_MEJEPA_LIVE_PREDICTIONS) |
| MCP tool namespace `mcp__cgreality__mejepa_*` | constitution `<mcp_tool_namespace_lock>` | SPEC-INFERENCE, SPEC-SHIFT-SUBSCRIBER, SPEC-HYGIENE, SPEC-EVALUATION |
| Reality-boundary path discipline | constitution §8 `<anti_patterns>` | all specs |
| Per-language stratification | SPEC-EVALUATION | SPEC-CORPUS (split), SPEC-INFERENCE (calibration), SPEC-TCT (thresholds) |

---

## Reading guide

For an AI agent picking up a phase:

1. Read `../constitution.md` first.
2. Read the relevant SPEC-`<DOMAIN>`.md (functional — what to build).
3. Read the matched TECH-`<DOMAIN>`.md (technical — how to build).
4. Read the relevant `tasks/<phase>/TASK-<DOMAIN>-NNN.md` files in sequence order.
5. Implement task-by-task; never reference files from a future task; verify each task's `definition_of_done` before moving on.

For a human reviewer:

1. Skim `../_README.md` (master index) and `../implementation-plan.md` (roadmap).
2. Pick a domain; read the SPEC + TECH pair.
3. Cross-check requirements against `../tasks/_traceability.md` to confirm coverage.

---

## Cross-references

- Master index: `../_README.md`
- Implementation plan: `../implementation-plan.md`
- Constitution: `../constitution.md`
- Technical specs (matched pairs): `../technical/_index.md`
- Task graph: `../tasks/_index.md`
- Traceability matrix: `../tasks/_traceability.md`
- Spec-authoring framework: `/home/cabdru/contextgraph/docs2/prdtospec.md`
