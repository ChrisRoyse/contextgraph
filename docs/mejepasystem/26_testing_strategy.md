# 26 — Testing Strategy

Source files covered: `docs/futurebuild/18_INTEGRATION_AND_E2E_TESTING.md`, `docs/ruvectorfindings/specs/constitution.md §testing_requirements`, `docs/futurebuild/specs/FSV-PROTOCOL.md`, `crates/*/examples/*_fsv.rs`, `crates/context-graph-test-utils/`.

> Testing is **fail-closed with real data**. No mocks. Every shipped primitive ships an FSV harness. Tests that pass while the project is broken are forbidden.

---

## 1. Coverage minimums

| Surface | Coverage minimum |
|---|---|
| Shipped crates | 80% line coverage |
| Safety-critical paths (`verify()`, `observe()`, `Gτ`, witness chain) | 90% |

---

## 2. Test categories

| Category | Purpose | Where |
|---|---|---|
| **Unit** | Per-instrument `encode()`, per-helper logic | `#[cfg(test)] mod tests` inline + `tests.rs` |
| **Integration** | Cross-module flows (panel materialization across 11 languages) | `tests/` directory + `cargo test --workspace` |
| **FSV (Forensic Source Verification)** | Read source-of-truth back; assert SHA matches expectation | `examples/<name>_fsv.rs` per crate |
| **Per-language oracle round-trip** | real source → AST chunker → instruments → panel → predictor → Gτ; oracle outcome compared against language toolchain | `examples/ast_chunker_multilang_fsv` |
| **UTML L_step validation** | 100 synthetic examples spanning ΔP/ΔK/ΔΩ/ΔΞ corner cases | `train_edge_cases_fsv` |
| **Constellation Gτ FP rate** | ≤ 5% on held-out KnownGood per language | `crates/context-graph-mejepa-tct/examples/` |
| **Drift drill** | Inject 70% empirical conformal coverage; detect within 1,000 obs; trigger A/B/C | `mejepa-heal-drill` binary + `phase5_self_healing_fsv` |
| **EWC++ non-degeneracy** | After 10 task boundaries, weight protection has full rank | `crates/context-graph-mejepa/heal/ewc_tests.rs` |
| **Witness chain replay** | End-to-end on 100,000-entry chain | `crates/context-graph-witness/cert_tests.rs` |
| **Shift subscriber latency** | p50 < 5s, p99 < 30s on synthetic shift stream | `crates/context-graph-mejepa-shift-subscriber/examples/shift_subscriber_fsv` |
| **Cross-cutting integration (futurebuild)** | provenance integrity + watermark restart-safety + scheduler+drift+heal end-to-end | `TASK-TEST-001` |
| **Chaos (futurebuild)** | random task kills, disk-full injection, embedder digest swap | `TASK-TEST-005` |
| **Load (futurebuild)** | 100k synthetic shifts/min sustained; p99 latency | `TASK-TEST-006` |
| **Soak (futurebuild)** | 72-hour continuous operation; memory + state stability | `TASK-TEST-007` (quarterly) |

---

## 3. FSV protocol (`docs/futurebuild/specs/FSV-PROTOCOL.md`)

Inherited contract for every TASK-* file:

| Step | Action |
|---|---|
| **§1 SoT readback** | Identify source-of-truth (file path / RocksDB CF / git SHA); read back after action and assert match |
| **§2 ≥3 boundary cases** | empty input, maximum input, malformed input — print state before + after |
| **§3 NO mocks** | Real Python / cargo / jest oracles; real Docker; real RocksDB; real tree-sitter |
| **§4 NO backwards compatibility** | Pre-production; wipe DB on schema change |
| **§5 Fail-closed errors** | Every fallible op returns `Result<T, ContextGraphError>` with structured error + `.code()` tag |
| **§6 Evidence artifacts** | Per-FSV evidence file under `/mnt/d/contextgraph/fsv/<crate>-fsv/` (mode 0600) |
| **§7 ASSUME THE TASK IS WRONG** | Audit every path, symbol, CF reference against the live tree before implementing |

A task is not done without all §1–§6. Regardless of `cargo test` results.

---

## 4. FSV evidence locations

Per `docs/ruvectorfindings/specs/constitution.md §testing_requirements`:

| Subsystem | FSV path |
|---|---|
| Phase 0 corpus | `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/` |
| Instruments + Panel | `/tmp/contextgraph-mejepa-instruments-fsv/` → `/mnt/d/contextgraph/fsv/` (migration in `TASK-OBS-006`) |
| Predictor | `/tmp/contextgraph-mejepa-predictor-fsv/` |
| Training | `/tmp/contextgraph-mejepa-train-fsv/` |
| TCT constellation | `/tmp/contextgraph-mejepa-tct-fsv/` |
| Shift subscriber | `/tmp/contextgraph-mejepa-shift-subscriber-fsv/` |
| Per-language AST chunkers | `/tmp/contextgraph-{python,rust,c,cpp,java,csharp,go,js,ts,ruby,php}-ast-chunker-fsv/` |
| ME-JEPA-cf CF round-trip | `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json` |
| Eval (Phase 8) | `/tmp/contextgraph-mejepa-phase8-eval-fsv/` (17 evidence files) |
| Hygiene GC stress | `/tmp/contextgraph-mejepa-hygiene-fsv/gc_stress_fsv/` |
| Tier compression | `/tmp/contextgraph-tier-compression-fsv/` |

All paths migrate to `/mnt/d/contextgraph/fsv/` per `TASK-OBS-006`.

---

## 5. How to run tests

```bash
# Full workspace
cargo test --workspace

# Single crate
cargo test -p context-graph-mejepa-corpus
cargo test -p context-graph-mejepa-tct chunk_gtau --lib

# FSV harness binaries
cargo run --release -p context-graph-mejepa --example predictor_fsv
cargo run --release -p context-graph-mejepa-train --example train_edge_cases_fsv
cargo run --release -p context-graph-mejepa --example phase5_self_healing_fsv
cargo run --release -p context-graph-mejepa --example phase8_eval_fsv
cargo run --release -p context-graph-core --example ast_chunker_multilang_fsv

# Heal drill (binary)
cargo run --release --bin mejepa-heal-drill -- --drift soft

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --check
```

---

## 6. Key fixtures + test utilities

Source: `crates/context-graph-test-utils/`. **NO MOCK DATA** — real synthetic only.

| Utility | Purpose |
|---|---|
| `fingerprints.rs` | Real fingerprint generators (E1–E14 vectors) |
| `stores.rs` | Store factory helpers (real RocksDB, real HNSW) |

Tests must NOT mock embedders. Use real model outputs OR real synthetic deterministic vectors.

---

## 7. Integration test surfaces (futurebuild Phase F)

Per `docs/futurebuild/18_INTEGRATION_AND_E2E_TESTING.md`:

| Test | Surface |
|---|---|
| `TASK-TEST-001` | Cross-cutting: provenance integrity end-to-end |
| `TASK-TEST-002` | Watermark restart-safety: kill mid-tail, restart, verify cursor resumes |
| `TASK-TEST-003` | Scheduler + drift + heal: inject drift, verify intervention + promote |
| `TASK-TEST-004` | Hook → shift-log → subscriber → predict round-trip |
| `TASK-TEST-005` | Chaos: random task kills + disk-full injection + embedder digest swap |
| `TASK-TEST-006` | Load: 100k synthetic shifts/min sustained; p99 latency report |
| `TASK-TEST-007` | Soak: 72-hour continuous; memory + state stability (quarterly) |
| `TASK-TEST-008` | Multi-language oracle round-trip (all 11 langs) |
| `TASK-TEST-009` | A/B/C promotion: simulate hold-out wins, ensure correct pointer flip |
| `TASK-TEST-010` | Agent-feedback path: synthetic SurpriseEvent → sampling weight bump verification |
| `TASK-TEST-011` | DR drill: backup + restore + verify witness chain |

---

## 8. Negative tests (must fail closed)

| Test | Expected failure |
|---|---|
| Panel with dim ≠ 5,120 | `MEJEPA_INFER_DIM_MISMATCH` |
| Instrument output non-finite | `MEJEPA_PANEL_NON_FINITE` |
| Embedder digest mismatch | `MEJEPA_EMBEDDER_DIGEST_MISMATCH` — daemon refuses start |
| Constellation > 90 days stale | `MEJEPA_INFER_GTAU_STALE_CONSTELLATION` |
| Conformal calibration N=0 | `MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES` |
| Corpus output to `/tmp` | rejected by `context-graph-paths` |
| Tree-sitter parse error | `MEJEPA_CHUNKER_PARSE_FAILED` with byte-offset diagnostics |
| Mixed-language source | `MEJEPA_CHUNKER_MIXED_LANGUAGE` |
| Mutation site bounds invalid | `MEJEPA_CORPUS_OPERATOR_FAILED` |
| ΔΞ.target_collapse > 0 | `MEJEPA_INSTRUMENT_GRADIENT_LEAK` (critical bug) |
| Call to retired E5 | `MEJEPA_EMBEDDER_RETIRED` |
| Call to retired E11 | `MEJEPA_EMBEDDER_RETIRED` |
| Call to `reality_run_attempt` | `CCREALITY_ENGINE_RETIRED` |
| `mejepa_record_agent_feedback` > 100/hr | `MEJEPA_FEEDBACK_RATE_LIMITED` |
| Feedback against non-existent prediction | `MEJEPA_FEEDBACK_PREDICTION_NOT_FOUND` |
| Patch unparseable | `MEJEPA_VERIFY_INVALID_PATCH` |
| OOD score > hard threshold | `MEJEPA_VERIFY_OUT_OF_DISTRIBUTION_HARD` |
| Every chunk fails Gτ | `MEJEPA_VERIFY_GUARD_REJECTED_HARD` |
| D-drive missing | `MEJEPA_D_DRIVE_UNAVAILABLE` |
| > 5 scheduler restarts/5min | `MEJEPA_SCHEDULER_FLAPPING` |
| Subscriber lag > 500/min | `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT` |

---

## 9. Reproducibility tests

| Test | Verifies |
|---|---|
| Mutation operator determinism | Same input + same seed → byte-equal output |
| Witness chain replay | 100k-entry chain hash-verifies start-to-end |
| Training cert replay | step N's `self_hash` == `SHAKE-256(parent_witness_hash || cert_body)` |
| Embedder digest pinning | Model load failure on wrong SHA |
| Per-(predictor_version × calibration × constellation × theta_sha) | Same triple produces byte-equal `RealityPrediction` |

---

## 10. Quarterly soak test (futurebuild)

Per `TASK-TEST-007`. 72-hour continuous operation on synthetic shift stream:

```
- Inject 100 synthetic shifts/minute (Python + Rust files)
- Daemon runs end-to-end (subscriber + scheduler + GC + eval)
- Capture: RSS over time, VRAM over time, RocksDB SST size, lag p50/p99, A/B/C events
- Assertions:
  - RSS does not grow > 2× steady-state
  - VRAM does not exceed budget
  - Lag p99 < 30s
  - At least 1 A/B/C cycle completes
  - Witness chain valid at end
  - No MEJEPA_SCHEDULER_FLAPPING
```

---

## 11. Cross-references

[`docs/futurebuild/specs/FSV-PROTOCOL.md`](../futurebuild/specs/FSV-PROTOCOL.md), [`docs/ruvectorfindings/specs/constitution.md`](../ruvectorfindings/specs/constitution.md), [`29_verification_report.md`](29_verification_report.md).
