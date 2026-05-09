# Functional Specs Index (Level 2)

> **Level 2 — what to build.** Each spec captures user stories, requirements (`[REQ-<DOMAIN>-NN]`), edge cases, error states, and test plan. Implementation details live in the matched Level 3 technical spec.

**Authority:** `../constitution.md` overrides. When this index conflicts with constitution, constitution wins.

---

## Spec inventory (13 functional specs)

| ID | File | Phase | Status | REQ count |
|---|---|---|---|---|
| SPEC-CORPUS | [SPEC-CORPUS.md](./SPEC-CORPUS.md) | 0 | partial | 16 |
| SPEC-AST-CHUNKING | [SPEC-AST-CHUNKING.md](./SPEC-AST-CHUNKING.md) | 1a | partial | 15 |
| SPEC-EMBEDDERS | [SPEC-EMBEDDERS.md](./SPEC-EMBEDDERS.md) | 1b | partial | 12 |
| SPEC-INSTRUMENTS | [SPEC-INSTRUMENTS.md](./SPEC-INSTRUMENTS.md) | 1c | partial | 23 |
| SPEC-PREDICTOR | [SPEC-PREDICTOR.md](./SPEC-PREDICTOR.md) | 2 | deferred | 18 |
| SPEC-TRAINING | [SPEC-TRAINING.md](./SPEC-TRAINING.md) | 3 | deferred | 12 |
| SPEC-UTML | [SPEC-UTML.md](./SPEC-UTML.md) | 3 (cross-cutting) | deferred | 12 |
| SPEC-INFERENCE | [SPEC-INFERENCE.md](./SPEC-INFERENCE.md) | 4a | deferred | 20 |
| SPEC-TCT | [SPEC-TCT.md](./SPEC-TCT.md) | 4b (cross-cutting) | deferred | 15 |
| SPEC-SELF-HEALING | [SPEC-SELF-HEALING.md](./SPEC-SELF-HEALING.md) | 5 | deferred | 20 |
| SPEC-HYGIENE | [SPEC-HYGIENE.md](./SPEC-HYGIENE.md) | 6 | partial | 10 |
| SPEC-SHIFT-SUBSCRIBER | [SPEC-SHIFT-SUBSCRIBER.md](./SPEC-SHIFT-SUBSCRIBER.md) | 7 | deferred | 18 |
| SPEC-EVALUATION | [SPEC-EVALUATION.md](./SPEC-EVALUATION.md) | 8 | deferred | 20 |

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
