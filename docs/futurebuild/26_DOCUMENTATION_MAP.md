# 26 — Documentation Map (Meta)

> **Purpose.** A map of where each piece of documentation lives across the repo so a new contributor can navigate without spelunking.

---

## 1. Top-level layout

```
/home/cabdru/contextgraph/
├── CLAUDE.md                                # operator role + project rules (read first)
├── README.md                                # repo overview (if present)
├── docs/
│   ├── futurebuild/                         # forward-looking plan (this directory; the canon for future work)
│   │   ├── 00_INDEX.md ... 26_DOCUMENTATION_MAP.md
│   │   ├── specs/{functional,technical}/
│   │   └── tasks/<domain>/
│   ├── ruvectorfindings/                    # design + hardening evidence (current architectural canon)
│   │   ├── 00_INDEX.md
│   │   ├── 01..16_*.md                      # design docs + post-pivot statement + UTML formula + TCT + AST + hardening ledgers
│   │   └── specs/                           # current shipped substrate spec hierarchy
│   │       ├── constitution.md (Level 1)
│   │       ├── _README.md
│   │       ├── implementation-plan.md
│   │       ├── functional/, technical/, tasks/
│   ├── ccreality/                           # historical pre-pivot two-loop docs (read-only reference)
│   │   ├── 00_OVERVIEW.md
│   │   └── 01..21_*.md
│   ├── futurebuild/                         # (this directory)
│   ├── alldata.md                           # raw research notes
│   ├── PAPER.md                             # ContextGraph Reality Loop paper
│   └── ...
├── docs2/                                   # cross-project standards
│   ├── prdtospec.md                         # spec authoring framework (Level 1-5)
│   ├── SHARED-MIND-AGENT-PROTOCOL.md        # SMAP protocol
│   ├── SOLO-AGENT-MEMORY-PROTOCOL.md
│   ├── TheoryOfMachineLearning.md           # UTML v1.0 source theory
│   └── ...
├── crates/                                  # Rust workspace
│   └── context-graph-*/                     # per-crate src + tests + examples (FSV)
├── memory/                                  # SMAP shared mind (team narrative)
│   ├── context/, decisions/, discoveries/, handoff/, journal/, blockers/, patterns/, reports/
├── agents/                                  # SMAP agent roster + role definitions
│   └── README.md
├── .claude/                                 # Claude Code config
│   ├── settings.json                        # hooks + permissions
│   ├── loop.md                              # per-tick prompt (single source of truth)
│   ├── agents/                              # agent definitions
│   └── skills/                              # operator-driven skills
├── scripts/                                 # utility scripts
│   ├── cc/                                  # Claude Code hooks
│   ├── context-graph-mcp-wrapper.sh         # daemon launcher
│   ├── systemd/                             # systemd unit templates
│   ├── audit/                               # audit scripts
│   └── dr/                                  # disaster recovery scripts
├── ci/                                      # CI configs + lints
├── tasks/                                   # workstream-specific manifests
│   └── swebench-lite/*.json                 # 300 SWE-bench Lite task manifests
├── config/                                  # configuration files
└── target/                                  # build artifacts (gitignored)
```

---

## 2. Where to find specific topics

| Topic | Primary doc | Secondary references |
|---|---|---|
| Operator role + per-tick contract | `CLAUDE.md §11` | `.claude/loop.md` |
| Forward plan (the truth-system rollout) | `docs/futurebuild/00_INDEX.md` | `docs/futurebuild/12_IMPLEMENTATION_PHASES.md` |
| Vision (what we're building) | `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md` | `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` |
| Architecture (multi-head + closed loop) | `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md` | `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` |
| Prediction taxonomy (every prediction emitted) | `docs/futurebuild/15_PREDICTION_TAXONOMY.md` | — |
| Storage policy (D-drive only) | `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` | `memory/decisions/2026-05-10--d-backed-...md` |
| Learning formula (UTML v1.0 multi-head) | `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md` | `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`, `docs2/TheoryOfMachineLearning.md` |
| Multi-embedder enrichment (DDA/TCT) | `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md` | `docs/ruvectorfindings/12,13_*.md` |
| AST chunking (11 languages) | `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md` | `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` |
| Self-healing scheduler | `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md` | `docs/ruvectorfindings/specs/_README.md` Phase 5 |
| Live prediction agent API | `docs/futurebuild/08_LIVE_PREDICTION_AGENT_API.md` | `docs/futurebuild/15_PREDICTION_TAXONOMY.md` |
| Loop refactor / new loop.md content | `docs/futurebuild/06_REALITY_LOOP_REFACTOR.md`, `14_NEW_LOOP_MD.md` | `.claude/loop.md` |
| Inner-loop residue cleanup | `docs/futurebuild/11_DELETION_LIST_INNER_LOOP.md` | `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` |
| Integration / E2E / chaos / load testing | `docs/futurebuild/18_INTEGRATION_AND_E2E_TESTING.md` | per-task FSV examples |
| Operator runbook (daily ops) | `docs/futurebuild/19_OPERATOR_RUNBOOK.md` | `CLAUDE.md` |
| Disaster recovery + backup | `docs/futurebuild/20_DISASTER_RECOVERY_AND_BACKUP.md` | `19 §6` |
| Cold start + bootstrap | `docs/futurebuild/21_COLD_START_AND_BOOTSTRAP.md` | `09 §6` |
| Adversarial robustness | `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md` | `13 §6` (TCT collapse-immunity) |
| Multi-agent isolation | `docs/futurebuild/23_MULTI_AGENT_ISOLATION.md` | `08 §10` |
| API stability / versioning | `docs/futurebuild/24_API_STABILITY.md` | per-tool spec stability tier |
| Future domains (Voice / Image) | `docs/futurebuild/25_FUTURE_DOMAINS_MEJEPA_X.md` | `13 §6` |
| Blind spots audit | `docs/futurebuild/17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md` | this doc |
| Spec authoring framework | `docs2/prdtospec.md` | each spec |
| SMAP protocol | `docs2/SHARED-MIND-AGENT-PROTOCOL.md` | `memory/` |
| UTML source theory | `docs2/TheoryOfMachineLearning.md` | `docs/ruvectorfindings/11_*.md` |
| Reality-shift principle | `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md` | `CLAUDE.md §11.5` |
| Hardening ledgers | `docs/ruvectorfindings/15,16_PHASE_*_HARDENING_FSV_*.md` | per-task FSV evidence |

---

## 3. Where to find the spec for a domain

Each domain has functional + technical specs + atomic tasks. Cross-reference table:

| Domain | Functional | Technical | Tasks |
|---|---|---|---|
| FLYWHEEL | `specs/functional/SPEC-FLYWHEEL.md` | `specs/technical/TECH-FLYWHEEL.md` | `tasks/flywheel/` |
| PREDICT | `specs/functional/SPEC-PREDICT.md` | `specs/technical/TECH-PREDICT.md` | `tasks/predict/` |
| ENRICHMENT | `specs/functional/SPEC-ENRICHMENT.md` | `specs/technical/TECH-ENRICHMENT.md` | `tasks/enrichment/` |
| LOOP | `specs/functional/SPEC-LOOP.md` | `specs/technical/TECH-LOOP.md` | `tasks/loop/` |
| HEAL | `specs/functional/SPEC-HEAL.md` | `specs/technical/TECH-HEAL.md` | `tasks/heal/` |
| OBSERVABILITY | `specs/functional/SPEC-OBSERVABILITY.md` | `specs/technical/TECH-OBSERVABILITY.md` | `tasks/observability/` |
| TESTING | `specs/functional/SPEC-TESTING.md` | `specs/technical/TECH-TESTING.md` | `tasks/testing/` |
| RUNBOOK | `specs/functional/SPEC-RUNBOOK.md` | `specs/technical/TECH-RUNBOOK.md` | `tasks/runbook/` |
| DR | `specs/functional/SPEC-DR.md` | `specs/technical/TECH-DR.md` | `tasks/dr/` |
| SECURITY | `specs/functional/SPEC-SECURITY.md` | `specs/technical/TECH-SECURITY.md` | `tasks/security/` |
| CORPUS | (in `docs/ruvectorfindings/specs/functional/SPEC-CORPUS-functional.md`) | (in `docs/ruvectorfindings/specs/technical/`) | `tasks/corpus/` |
| BOOTSTRAP | (in `docs/futurebuild/21_*.md`) | (in `21 §3-7`) | `tasks/bootstrap/` |

---

## 4. Reading order for new contributors

1. `CLAUDE.md` — what the project is + the operator contract
2. `docs/futurebuild/00_INDEX.md` — forward plan navigation
3. `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md` — what we're building
4. `docs/futurebuild/02_CURRENT_STATE_HONEST.md` — what's shipped vs gaps
5. `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md` — reference architecture
6. `docs/futurebuild/12_IMPLEMENTATION_PHASES.md` — what's being built next
7. `docs/futurebuild/13_TASK_INDEX.md` — atomic task graph
8. `docs/futurebuild/15_PREDICTION_TAXONOMY.md` — what predictions look like
9. `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` — D-drive rules (must-read before adding any artifact)
10. `docs/futurebuild/19_OPERATOR_RUNBOOK.md` — how to operate
11. `docs/ruvectorfindings/specs/_README.md` — current shipped substrate

After that: deep-dive whichever domain you're working on (flywheel / predict / enrichment / loop / heal / observability / testing / runbook / DR / security).

---

## 5. Glossary

| Term | Definition |
|---|---|
| **ME-JEPA** | Joint-Embedding Predictive Architecture for code reality (this project's predictor) |
| **DDA** | Dynamic Discriminative Analysis — n × (N + N²) signal extraction per chunk |
| **TCT** | Teleological Constellation Training — per-cell centroids + Gτ guard |
| **UTML** | Unified Theory of Machine Learning v1.0 — the four-factor learning formula |
| **CF** | RocksDB Column Family |
| **FSV** | Full-State Verification — write evidence to disk + read back to verify |
| **SMAP** | Shared Mind Agent Protocol — the `./memory/` discipline |
| **PRD** | Product Requirements Document — Level 1 of the spec hierarchy |
| **Daemon** | the long-running supervised MCP server process per Phase A |
| **Verdict** | the headline of `RealityPrediction`: Pass / Fail / OOD / Abstain / GuardRejected |
| **Constellation** | per-(category × entity × language × embedder) centroid table |
| **Witness chain** | append-only SHAKE-256 hash chain binding every artifact |
| **Shift log** | per-session JSONL of file mutations |
| **Active-learning queue** | operator-facing list of uncertain predictions for label |
| **A/B/C** | promotion gate: A=current, B=fresh retrain, C=ensemble |
| **EWC++** | Elastic Weight Consolidation, online variant |
| **Conformal predictor** | calibrated confidence interval method |
| **Gτ guard** | TCT inference-time guard: cosine ≥ τ_m for every embedder |

---

## 6. Where each operator action is documented

| Action | Where |
|---|---|
| Bootstrap a fresh deploy | `19 §1` |
| Daily health check | `19 §2` |
| Weekly review | `19 §3` |
| Approve catastrophic promotion | `19 §5.3` |
| Restore from backup | `20 §3` |
| Force a rollback | `20 §3.4` |
| Label an active-learning entry | `19 §3.2` |
| Add a new prediction head | `25 §1` (template) + extend `SPEC-PREDICT.md` |
| Add a new domain | `25 §2-7` (template) |
| Investigate a wrong prediction | `19 §5.7` + `mejepa_inspect_prediction` |
| Cross-version replay | `24 §5` |
| Trigger DR drill | `20 §4` |
| Rotate models | `19 §5.3` (catastrophic path) or `25` (domain-specific) |
