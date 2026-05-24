# AGENTS.md — ContextGraph

## §0 — THE NORTH STAR (read this every session)

**Every action you take in this repository must move at least one number on the project's target sheet, or preserve a doctrine invariant. If it does neither, it is worthless and must not ship.** This is not aspirational discipline — it is the locked rule per #383 that defines whether work belongs in this repository.

**Canonical specs (read both at session start, after the issue-queue queries below):**

- **[`docs/futurebuild/specs/NORTH-STAR.md`](docs/futurebuild/specs/NORTH-STAR.md)** — explicit north star + why this is the goal + the worthlessness test
- **[`docs/futurebuild/specs/TARGET-METRICS.md`](docs/futurebuild/specs/TARGET-METRICS.md)** — the perfect-state numerical predicate (every number with target + current value)

**Pinned GitHub context:** `[P0] CONTEXT — Numerical target sheet (the perfect-state predicate) — THE GOAL` (sister-pin to #712).

### The one-line predicate that defines "the system works"

```
THE_SYSTEM_WORKS ⇔
  prediction_oracle_correlation ≥ 0.95
  STABLE × 4 consecutive rolling windows
  AND cross_panel_oracle_correlation ≥ 0.95 on Panel B
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
  AND per-cell quality contract holds for every (mutation_category × language) cell
  AND online mistake-loop convergence holds
  AND every doctrine invariant equals its required value exactly
```

When this predicate evaluates `TRUE` for 4 consecutive windows on aiwonder against the 300×8 SWE-bench Lite Python corpus → **human review of AI-generated Python patches becomes statistically unjustified**. That is the moment the system has worked. Nothing before that moment is "done." Nothing else is the goal.

### Six numerical tiers (full detail in `TARGET-METRICS.md`)

| Tier | What it covers | Status today |
|---|---|---|
| **Tier 1** | The 4 ship-gate numbers (`prediction_oracle_correlation ≥ 0.95`, 4-window stability, Panel B agreement ≥ 0.95, panel drift ≤ 0.05) | raw Pearson `0.4843` → target `0.95` (gap `0.4657`) |
| **Tier 2** | Per-cell quality contract (#532): per-cell Pearson ≥ 0.95, Brier ≤ 0.05, ECE ≤ 0.05, recall ≥ 0.95 on degenerate cells, conformal coverage 88-92%, cell support ≥ 50 | aggregate ECE ✓; weak cells `delete_test_call`, `off_by_one`, `subtle_flip` |
| **Tier 3** | Online mistake-loop convergence: `mistake_repeat_rate` ≤ 0.05, unrelated-control delta ≤ 0.02 | LUT structurally inert (#689); fix prereq |
| **Tier 4** | Doctrine invariants — numbers that must equal exactly X (slot identity, anti-compensation, Q4 freeze, no-flat-vector, aiwonder durable root, inner-LLM compile guard) | enforced fail-closed |
| **Tier 5** | Substrate quality (BLINDSPOT audits): `I(panel; oracle) ≥ 0.95 bits` (#734), `oracle_self_consistency ≥ 0.97` (#723), per-embedder distinctness, asymmetric AUC ≥ 0.7 (#744) | ⭐ TWO CHEAPEST PROBES — run in parallel to #712 |
| **Tier 6** | Embedder gating (creativity engine + in-slot replacement): `τ_novelty < 0.7`, `δ_utility ≥ 0.005`, drop-ablation thresholds, asymmetric retrieval Spearman ≤ 0.5 | gates per-embedder promotion (#516/#518/#745/#746) |

### The worthlessness test (locked per #383)

Before you file an issue, write code, train an embedder, ingest a corpus, run an FSV harness, or propose any architectural change, answer all five questions explicitly:

1. **Which Tier-1/2/3/5/6 number does this move?** — Be specific. "Improves the predictor" is NOT an answer. "Increases per-cell Pearson on `subtle_flip::python` from 0.80 to 0.85" IS an answer.
2. **By how much (expected delta)?** — If < 0.005 on any target and the work cost is > 1 day, defer.
3. **What's the falsification condition?** — Under what measured outcome is this work classified as failed and rolled back?
4. **What's the doctrine cost?** — Does this preserve every Tier-4 invariant exactly? If not, it does not ship.
5. **Aiwonder execution path?** — Per `CLAUDE.md §6.7`, every production action runs on the aiwonder datacenter PC. Specify the `/zfs/archive/contextgraph/...` path, the SHA-pinning, and the manual SoT readback protocol per `06 §1.9`.

If the work cannot answer (1) cleanly with a specific number it moves OR (4) cleanly with a Tier-4 invariant it preserves — **it is worthless by definition** and should be reconsidered or rejected. Activity is not the goal. Movement of those specific numbers is the goal.

### The state is not the constraint — only Tier-4 invariants are

The current state of the project (300×8 SWE-bench Lite corpus, current 15-slot panel, current Docker oracle harness, current `0.95` threshold) is **starting state, not fixed constraint**. If reaching a target requires changing the state, that is the acceptable path. Only Tier-4 doctrine invariants are immovable.

State-change levers per stalled target (see `NORTH-STAR.md §9.5` + `TARGET-METRICS.md §7.5` for full matrix):

| If this target stalls | The right state-change lever is |
|---|---|
| Tier-1 `prediction_oracle_correlation` (statistical power) | **Corpus expansion** — SWE-bench Verified (~500 → +4,000 rows → ±0.036 Pearson CI at r=0.95), SWE-bench Full (2,294 → +18,352 rows → ±0.014 CI), BugsInPy, CommitPack |
| Tier-1 (4-window stability variance) | Window resampling refinement + corpus expansion for per-window power |
| Tier-1 (oracle ceiling) | Oracle refinement (replica voting, flakiness modeling); only upward threshold revision is doctrine-legitimate |
| Tier-2 per-cell weak cells | Per-cell corpus expansion (e.g., #615 delete_test_call regeneration) |
| Tier-2 Brier/ECE | Calibration refinement (per-cell conformal, isotonic/Platt per #695) |
| Tier-3 mistake-loop inertness | Architecture fix (#689 LUT) |
| Tier-5 `I(panel; oracle)` < 0.95 bits | **Panel expansion** (in-slot causal embedder replacement #745; new instrument slots if needed) — more data does NOT fix this |
| Tier-5 oracle self-consistency < 0.97 | Oracle harness refinement + per-instance flakiness modeling (BLINDSPOT-014) |
| Tier-5 inter-embedder distinctness | Per-embedder fix or replacement (#716/#720/#721/#719) + #715 v2 cache rebuild |
| Tier-6 embedder gating | Per-embedder audit + replacement (#516/#518/#746) |

**Forbidden state changes:** Lowering Tier-1 targets to match current corpus limitations is **not** a state change — it's a doctrine regression. The correct response to a corpus limitation is corpus expansion, not threshold relaxation. Only **upward** threshold revision is doctrine-legitimate (e.g., `0.95` → `0.97` if oracle self-consistency proves higher than expected).

**Decision rule** before invoking any state-change lever:
1. Which target is stalled? (Tier + variable + current measurement + gap)
2. Why does current state make that target unreachable? (FSV evidence, not speculation)
3. What is the minimum state change? (Don't over-shoot)
4. Are all Tier-4 invariants preserved?
5. What is the aiwonder execution cost? (Wall-clock GPU days, storage, operator review)
6. What is the falsification condition?

If (1)-(6) are answered, file as `type:decision` issue with operator authorization, execute on aiwonder, verify via manual SoT readback, re-measure the targeted variable.

### Why this matters (the operational framing)

AI coding agents produce code at rates humans cannot review at the same rate. The bottleneck is verification, not generation. ME-JEPA-Code exists to be **the deterministic mathematical verifier of code-against-reality** that closes that gap. When the target predicate evaluates `TRUE`, the verifier replaces human review at the statistical level — the verifier's `Pass` carries the same information content as a human's `Pass`, the verifier's `Fail` carries the same information content as a human's `Fail`, and disagreements feed back into the verifier's weights without human supervision.

This is super-intelligence in the operational sense: a system that replaces a human function (code review) at higher accuracy and at machine-scale throughput, achieved by predicting one specific number — `prediction_oracle_correlation` — on a representative corpus with calibrated per-cell confidence, Goodhart defenses, and online mistake-driven adaptation.

It is not a code generator. It is not an LLM-style reviewer. It is a calibrated binary predictor that runs in milliseconds on aiwonder's RTX 5090 and outputs `Pass | Fail` with bounded calibration error on every candidate patch.

### How every other artifact in this repo serves this goal

| Artifact | Role in service of the north star |
|---|---|
| **#712** critical path | Operator-authorized sequence of moves toward Tier-1 target #1 |
| **#739** BLINDSPOT-AUDIT (+ #723-#738 children) | Falsification audits checking whether the critical path is on track |
| **#744 / #745 / #746** causal-embedder track | Conditional architecture additions if BLINDSPOT-010 (#734) shows gap is causal-axis |
| **#683** (closed 2026-05-23) | Tier-1 prerequisite: trainer/checkpoint lineage countability |
| **#709 / #715** | Tier-5 substrate cleanup — clean per-embedder distinct vector spaces |
| **#615** | Tier-2 per-cell — `delete_test_call` cell corpus regeneration |
| **#618 / #619** | Tier-4 invariant restoration (no flat-buffer projection over the panel) |
| **#695** | Tier-1 measurement integrity — replace rank-lift Goodhart with raw Pearson |
| **#633 + #655 + #656 + #657** | Tier-1/4/5 evidence integrity gate |
| **#383** | The locked rule: gate-active issue priority must be evidence about progress toward the predicate |
| **#397** | The product-contract definition: what shape the predictor's output must take |

Anything that cannot trace to one of these (or a similar number-moving / invariant-preserving role) per #383 should not be in the backlog.

---

**Operating contract (universal):** [`docs2/AICodingAgentSuperPrompt.md`](docs2/AICodingAgentSuperPrompt.md) — the AI Coding Agent Doctrine. Master rules for any AI agent in this repo. Read once at session start.

**Project addendum:** [`CLAUDE.md`](CLAUDE.md) — ContextGraph / ME-JEPA-Code specifics: invariants, crates, paths, deny-list, build commands.

**Per-tick prompt:** [`.claude/loop.md`](.claude/loop.md) — what to do each `/loop` firing.

When the doctrine and `CLAUDE.md` agree, follow either. When they conflict, **the doctrine wins** for protocol detail (memory, claiming, FSV, RCA, hardening, communication, session lifecycle) and `CLAUDE.md` wins for project-specific invariants (paths, crates, deny-list, current phase). **The two specs at the top of this doc — `NORTH-STAR.md` + `TARGET-METRICS.md` — outrank both on the question "is this work worth doing"** (the worthlessness test).

---

## Memory = GitHub Issues

Per doctrine §3, all institutional memory lives in this repo's GitHub Issues. There is **no** `./memory/` directory. Old SAMP/SMAP content is archived at `backupdocs/memory.archive/` (read-only history; do not write new files there).

Read at the start of every turn (doctrine §3.3):

```bash
# 1. Pinned mission / current phase
gh issue list --state open --label "type:context" --json number,title,body

# 2. In-progress (avoid stepping on others)
gh issue list --state open --label "status:in-progress" --json number,title,assignees,updatedAt

# 3. Blocked (may be pickup-able if unblocked)
gh issue list --state open --label "status:blocked" --json number,title,assignees

# 4. Unclaimed queue (your candidates)
gh issue list --state open --label "source:agent" --search "no:assignee" --json number,title,labels,updatedAt

# 5. Active decisions binding you (closed = locked in)
gh issue list --state closed --label "type:decision" --limit 20

# 6. Discoveries / patterns touching your task area
gh issue list --state closed --label "type:discovery,type:pattern" --search "<keywords>" --limit 20
```

Then read `docs/futurebuild/README.md` for the current 8-doc map, plus `docs/futurebuild/01_overview.md`, `07_results_and_benchmarks.md`, and `08_roadmap_and_open_work.md` for current build state and open work.

---

## Two non-negotiable coordination rules (doctrine §3.2)

1. **File rule.** Defect / smell / anomaly / decision / discovery / pattern you are NOT capturing in code this turn → open a GitHub Issue before turn ends. If it isn't tracked, it dies with the session.
2. **Claim rule.** Before touching code tied to an Issue → assign self, flip `status:needs-triage` → `status:in-progress`, post a CLAIM comment with files-you-will-touch and ETA. Comment at every milestone. End the turn with RESOLVED / PAUSE / BLOCKED on every issue you worked.

No silent work. No `./memory/*.md` writes.

---

## Labels (bootstrap once)

Label set defined in [`.github/labels.yml`](.github/labels.yml). Apply with:

```bash
scripts/gh-labels-bootstrap.sh
```

Cap per issue: 1 `type:*` + 1 `priority:*` (default `priority:p2`) + 1 `status:*` + 1-2 `area:*` + `source:*` + `agent:*`.

---

## Project quick-reference

- **What:** ME-JEPA-Code — deterministic predictive truth system for AI-generated code changes.
- **The goal:** the full numerical predicate in [`docs/futurebuild/specs/TARGET-METRICS.md`](docs/futurebuild/specs/TARGET-METRICS.md). Headline: `prediction_oracle_correlation ≥ 0.95` stably × 4 rolling windows on 300×8 SWE-bench Lite Python corpus, with cross-panel agreement, per-cell quality contract, mistake-loop convergence, and all Tier-4 invariants holding exactly. Current honest gap: `0.4657` against raw Pearson.
- **The worthlessness test:** every action must move a Tier-1/2/3/5/6 number or preserve a Tier-4 invariant in `TARGET-METRICS.md`. Otherwise it does not ship (locked per #383).
- **Workspace:** Rust, 22 active crates under `crates/`.
- **Build:** `cargo build --release -p <crate>` → `cargo test -p <crate>` → `cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings`.
- **aiwonder production SoT:** post-migration durable corpus/runtime/project/FSV state lives on aiwonder under `/zfs/archive/contextgraph/`; hot work lives under `/zfs/hot/contextgraph/` or explicitly budgeted aiwonder scratch. **Every production action MUST execute on aiwonder per `CLAUDE.md §6.7`.** No `/mnt/d` / `/mnt/c`.
- **MCP server:** the only network surface for runtime work. No HTTP from action paths.
- **No inner LLM.** Retired 2026-05-09. Do not reanimate without explicit operator authorization. `feature = "llm"` triggers `compile_error!`.

Full detail in `docs/futurebuild/specs/NORTH-STAR.md`, `docs/futurebuild/specs/TARGET-METRICS.md`, `CLAUDE.md`, and `docs/futurebuild/README.md`.
