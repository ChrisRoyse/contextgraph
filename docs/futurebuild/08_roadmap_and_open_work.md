# 08 — Roadmap and Open Work

**Last refreshed:** 2026-05-18.

Source: `gh issue list --state open` (69 open issues), recent `type:decision` closes, git log, `MEMORY.md`, the 27-roadmap legacy doc.

---

## 1. Current Ship Gate (active until met)

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per (mutation_category × language)
  on the 300×8 Python SWE-bench Lite corpus
```

**Last holdout measurement:** `0.866667` (legacy `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`; #369 to revalidate post-migration on aiwonder).

**Gap to ship:** 0.083 correlation points. Bridging this is the entire job of Phase G's remaining open work (66 issues).

Once met → human review of AI-generated Python patches is statistically unjustified. Auto-trust policy gates engage (TASK-PY-G-058 / #277).

---

## 2. Open Issue Snapshot (69 open as of 2026-05-18)

By phase label:

| Label | Open count | Notes |
|---|---|---|
| `phase:g` | 66 | Ship-gate convergence work (this is the critical path) |
| `phase:migration` | (mostly closed) | Aiwonder migration — closing out; #366/#367/#368/#369 still open |
| `phase:f-followup` | 1 | #312 hot_root on ext4 throughput (perf followup) |
| `phase:f` | 0 (closed as of 2026-05-13) | — |
| `phase:h` | 0 (closed) | — |
| No phase (cross-cutting bugs, perf) | ~2 | #295 (resume scan perf), #307 (promote_tree_verified race), #320 (clippy bug) |

By priority:

| Priority | Open count | Examples |
|---|---|---|
| `priority:p0` | ~5 | #310 META issue refactor; #366, #369 aiwonder revalidation |
| `priority:p1` | ~30 | Most TASK-PY-DATA-* + TASK-PY-G-* shipped; remaining p1 are calibration + adversarial defense + Q4 head training |
| `priority:p2` | ~25 | EK series tail, post-ship policies, refined sub-tasks |
| `priority:p3` | ~9 | Deferred (SWE-bench Verified subset, sample-efficiency, v2 stretch tasks) |

---

## 3. Critical Path (the issues whose resolution moves the ship-gate needle)

### 3.1 Q4 head training (Phase G §2 producers — heads exist as schema fields but lack training data pipelines)

| Issue | Task | Producer for | Status |
|---|---|---|---|
| #266 / TASK-PY-G-047 | Q4 perf analyzer label pipeline | `predicted_perf_regressions` head | open |
| #267 / TASK-PY-G-048 | Q4 accuracy analyzer label pipeline | `predicted_accuracy_degradations` head | open |
| #268 / TASK-PY-G-049 | Q4 cost analyzer label pipeline | `predicted_cost_regressions` head | open |
| #269 / TASK-PY-G-050 | Reasoning-class head training | `predicted_reasoning_class` head | open |
| #118 / TASK-PY-G-003 | Per-head Q1-Q5 distillation training step | all heads | open |
| #270 / TASK-PY-G-051 | Q4 non-trivial-on-Pass calibration | always-on Q4 validation | open |
| #292 / TASK-PY-G-073 | Predicted-Works head + sub-heads | explicit positive assertions | ✅ closed 2026-05-18 |

### 3.2 Q5 shift-log replay (#264 / TASK-PY-G-045)

`reality_impact.rs` exists as a scaffold; the shift-log replay engine that drives Q5 `RealityImpact` for production traffic is the remaining work.

### 3.3 Phase G calibration sweep (Phase G §3)

| Issue | Task | Purpose |
|---|---|---|
| #122 | refined 66-cell pairwise MI matrix audit (redundancy/degeneracy) | open |
| #123 | per-cell TCT signal-saturation matrix (realized/theoretical) | open |
| #124 / TASK-PY-G-009 | frozen instrument coverage audit per Python cell | open |
| #125 / TASK-PY-G-010 | Q1 claim-existence calibration at corpus scale | open |
| #126 / TASK-PY-G-011 | Q3 failure-mode calibration | open |
| #127 / TASK-PY-G-012 | Q4 confidence calibration per class with Brier/ECE/τ | open |
| #128 / TASK-PY-G-013 | Q5 head calibration vs shift-log replay (consumes #264) | open |
| #263 / TASK-PY-G-044 | Multi-head contradiction calibration per cell | open |
| #121 / TASK-PY-G-006 | Conformal RCA classifier | open |
| #130 / TASK-PY-G-015 | Gτ guard audit | open |
| #152 / TASK-PY-G-021 | E_Trace gating | open |

**Gate for Phase G §3:** per-head Brier ≤ 0.05, ECE ≤ 0.05, conformal coverage 88–92%.

### 3.4 Phase 5 / Phase 6 closure (continuous self-improvement + agent feedback loop)

| Issue | Task | Status |
|---|---|---|
| #274 / TASK-PY-G-055 | UserPromptSubmit hook live RealityPrediction injection | ✅ closed |
| #154 / TASK-PY-G-023 | Active-learning consumer (multi-source queue: surprise / contradiction / OOD / agent / operator) | open |
| #275 / TASK-PY-G-056 | Live Claude Code session integration FSV (end-to-end) | open / in-progress |
| #276 / TASK-PY-G-057 | Live-session OOD corpus harvest | open |
| #131 / TASK-PY-G-016 | Feedback cooldown (prevent reward thrashing) | open |
| #163 / TASK-PY-G-024 | Feedback retrain consumer | open |
| #182 / TASK-PY-G-040 | Feedback replay-weight FSV | open |

### 3.5 Phase 7 adversarial defense + ship-gate closure (after calibration green)

| Issue | Task | Status |
|---|---|---|
| #178 | refined cell taxonomy with Active/Exempt/Uncovered tri-state | open |
| #129 / TASK-PY-G-014 | OOD detection gate calibrated on harvested + synthetic OOD | open |
| #120 / TASK-PY-G-005 | Catastrophic Python cell quarantine | open |
| #148 / TASK-PY-G-017 | Python leakage detector | open |
| #153 / TASK-PY-G-022 | Python oracle pass/fail audit | open |
| #165 / TASK-PY-G-027 | Python 0.95 confidence gate | open |
| #167 / TASK-PY-G-032 | Four-window Python burn-in | open |
| #172 / TASK-PY-G-034 | Python acceptance bundle | open |
| #180 / TASK-PY-G-038 | MCP RealityPrediction FSV | open |
| #171 / TASK-PY-G-033 | Fail-closed release switch | open |

### 3.6 Phase 8 post-ship operation (engages after `≥ 0.95 × 4 windows`)

| Issue | Task |
|---|---|
| #277 / TASK-PY-G-058 | Post-ship-gate auto-trust policy |
| #173 / TASK-PY-G-035 | Post-green drift guardrails |
| #170 / TASK-PY-G-031 | Model-quality RCA dashboard |
| #168 / TASK-PY-G-029 | Live Python patch backfill |
| #164 / TASK-PY-G-026 | Checkpoint provenance |
| #162 / TASK-PY-G-025 | Eval-after-promotion hook |
| #166 / TASK-PY-G-028 | Python error taxonomy |
| #169 / TASK-PY-G-030 | Unseen Python holdout quarantine |

---

## 4. EK Series (EPIC #321 explanatory-knowledge programme) — open tail

| Issue | Status |
|---|---|
| #325 TASK-EK-004 — Mechanism-conjecture surface on RealityPrediction | open |
| #326 TASK-EK-005 — `mejepa_counterfactual_under_invariant` MCP tool | open |
| #327 TASK-EK-006 — Curiosity reward | ✅ closed 2026-05-18 |
| #328 TASK-EK-007 — Held-out failure-mode class falsification | open |
| #329 TASK-EK-008 — AlphaEvolve-pattern outer loop using RealityPrediction as fitness | open |
| #330 TASK-EK-009 — Ontology-growth audit + telemetry (system-vs-operator attribution) | open |
| #335 TASK-EK-013 — `mejepa_reverse_prediction` MCP tool — desired-reality → patch-shape constraints | open |
| #336 TASK-EK-014 — Operator-contribution surface | in-progress |
| #339 TASK-EK-016 — Chunk foundationality | in-progress |
| #340 TASK-EK-017 — Library-level foundationality | in-progress |
| #349 TASK-EK-016-ext — Embedder-foundationality | in-progress |
| #352 TASK-EK-015 — Credibility-weighted operator override | in-progress |
| #354 / #355 / #356 / #357 | TASK-EK-018 v2 stretch (NAS embedder, cross-library, self-rewriting, status reallocation) — `priority:p3` |

---

## 5. Aiwonder migration (open tail)

| Issue | Task | Status |
|---|---|---|
| #310 | META — Issue refactor (2026-05-15) | in-progress p0 |
| #366 / TASK-AIWONDER-015 | Rerun TASK-PY-G-056 live-session FSV on aiwonder | open p0 |
| #367 / TASK-AIWONDER-016 | Project-ingest/runtime data locality audit for arbitrary repo folders | in-progress p1 |
| #368 / TASK-AIWONDER-017 | Host capability utilization audit + scheduler profile | open p1 |
| #369 / TASK-AIWONDER-018 | Revalidate post-migration Phase G FSV artifacts on aiwonder | open p0 (blocks transition of all Phase G FSV from `/mnt/d` to `/zfs/archive`) |
| #295 | PERF — #199 resume launch scans full D-drive FSV root | in-progress |
| #299 / #300 / #301 / #302 / #303 / #304 / #305 | AIWONDER-003..010 install / transfer / build / launch / calibration sub-tasks | in-progress p1 |
| #306 | AIWONDER-011 SWE-bench Lite Python Docker image plan (future Q3) | p3 |

---

## 6. Active type:context pins (must read at session start per AGENTS.md)

| # | Title | Why pinned |
|---|---|---|
| #310 | META Issue refactor (2026-05-15) | Tracks Phase G execution-order map; the live source of "what to do NOW" |
| #227 | Python ME-JEPA data explosion runtime policy | Defines runtime constraints during forward cache jobs |
| #296 | aiwonder migration EPIC (closed but still load-bearing context) | Establishes the migration plan |

---

## 7. Bugs (open, non-blocking)

| # | Title | Status |
|---|---|---|
| #295 | Resume launch scans full D-drive FSV root (perf) | in-progress |
| #307 | promote_tree_verified asserts 'previous destination archived' on second-subrun shard promotion (regression in 4cd003e1) | in-progress |
| #320 | phase_g_project_merkle_cache_fsv example fails clippy — `ProjectMerkleCache` doesn't impl `Debug` | needs-triage p1 |
| #338 | Capture agent response bodies in runtime/agent-history for Q1 claim FSV | needs-triage p2 |
| #312 | hot_root on ext4 is the throughput bottleneck — move to /hotpool ZFS | open `phase:f-followup` |

---

## 8. Deferred (not blocking ship gate)

| # | Why deferred |
|---|---|
| #149 / TASK-PY-G-018 | SWE-bench Verified subset — separate dataset; ship gate uses Lite 300×8 (priority dropped p2→p3 2026-05-15) |
| #150 / TASK-PY-G-019 | Python sample-efficiency analysis (post-ship-gate) |
| #354–#357 | TASK-EK-018 v2 stretch (NAS embedder from scratch / cross-library proposals / self-rewriting absence priors / status reallocation extension) |
| Non-Python ship-gate cells (Rust/JS/TS/Go/Java/C/Cpp/CSharp/Ruby/Php) | Active execution is Python-first (`CLAUDE.md §6.3`); non-Python deferred until Python truth system is green (`27_phase_g_roadmap` legacy doc) |

---

## 9. Post-ship roadmap (Phase J — only relevant after `≥ 0.95 × 4 windows` closes)

From legacy 31_COMPLETE_ROADMAP doc (now archived) + open `type:architecture` p3 issues:

- Non-Python language activation (one language at a time; each language re-engages the four-window stability cycle on its own corpus rows).
- Future modalities (`ME-JEPA-Voice`, `ME-JEPA-Image`) — template was 25_FUTURE_DOMAINS_MEJEPA_X in the archived suite; methodology to be revisited in a new GitHub `type:context` issue when ship gate green.
- Hardening axis maturation (see `docs2/hardeningprompt.md`) — supply chain, observability uplift, scalability tests, soak-test cadence.
- API stability commitments — currently pre-production; locks engage post-ship.

---

## 10. How to Pick Up Work (per AGENTS.md)

1. Run the six-issue queries from `AGENTS.md`:
   ```bash
   gh issue list --state open --label "type:context" --json number,title,body
   gh issue list --state open --label "status:in-progress" --json number,title,assignees
   gh issue list --state open --label "status:blocked" --json number,title
   gh issue list --state open --label "source:agent" --search "no:assignee" --sort updated
   gh issue list --state closed --label "type:decision" --limit 20
   gh issue list --state closed --label "type:discovery,type:pattern" --search "<kw>" --limit 20
   ```
2. Read `02_current_state` equivalent → `01_overview.md §1-2` + `07_results_and_benchmarks.md §1`.
3. Find the highest-priority open issue you can usefully unblock that does NOT have an in-progress assignee.
4. CLAIM (assign self + flip `status:needs-triage` → `status:in-progress` + post CLAIM comment with files-you-will-touch + ETA).
5. Audit-the-task before writing code (`06_operations_and_fsv.md §1.6`).
6. Work; comment at every milestone.
7. Produce FSV evidence per `06_operations_and_fsv.md §1.5`.
8. RESOLVED / PAUSE / BLOCKED issue closeout.
9. Code-simplifier pass (`docs2/hardeningprompt.md`).
10. File any observed defect / pattern / discovery as a fresh issue before ending the turn.

---

## 11. What This Doc Does NOT Cover

- Day-to-day operator decisions outside the issue tracker — those happen in conversation with the operator and become `type:decision` issues when locked in.
- Hardware procurement / aiwonder admin — `phase:migration` issues track that subset.
- Speculative future modalities or business outcomes — outside the WHAT-IS scope; capture as `type:context` issues if ever revisited.
- Time estimates — Phase G has no public ETA; TASK-G-021 (per-cell convergence ETA tracker) emits internal projections used by the heal scheduler.
