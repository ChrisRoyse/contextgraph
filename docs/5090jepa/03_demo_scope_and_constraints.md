# 03 — Demo Scope & Constraints

The full DynamicJEPA contract is large. The 5090jepa demo intentionally trims it to the smallest set of capabilities that *still proves the whole loop end‑to‑end on real hardware*. Every trim is justified.

Current implementation state as of the Phase 3 re-audit on 2026-04-30:
Phases 0-3 are implemented and verified. Phase 4 is next. The full in-scope
list below is still the demo target, not a claim that Phase 4+ behavior exists
today.

## 1. In Scope (You Build This)

- Two domain packs: `counter_world.v1` (deterministic arithmetic) and `gridworld_5x5.v1` (deterministic 5×5 grid navigation).
- One adapter: `json_event` (declarative path mapping from JSONL fixture rows to normalized state/action/outcome/transition).
- Four instrument kinds: `scalar`, `categorical`, `onehot`, `time`. No text/embedding/sparse/graph_neighborhood instruments in v1.
- Full pipeline storage (≈25 DynamicJEPA CFs as listed in `05_storage_schema_v1.md`).
- Deterministic binding via `id_equality` and `explicit_mapping` only.
- Trajectory compiler with a single segmentation policy: `by_domain_session` (every event in a fixture file is one trajectory).
- Dataset compiler with one objective: `latent_delta` one‑step prediction. Splits: `by_domain_key_hash` for gridworld, `by_time` for counter_world. Negatives: `same_domain_different_outcome`.
- Tiny encoder + predictor (≈100K params total) + EMA target encoder + VICReg loss. See `10_tiny_jepa_model.md`.
- Single training run per domain → single artifact registration with on‑disk SHA‑256.
- Inference: `predict(artifact, panel, action) -> next_panel + uncertainty`.
- Planner: enumerate the small declared candidate action set, predict each, select the predicted‑best, persist `PlanTraceRecord`.
- Surprise events when `‖observed_panel − predicted_panel‖ > threshold` from domain pack policy.
- Verification harness: `verify-counter-world`, `verify-gridworld`. Each runs the entire loop, three edge cases, and writes a `VerificationRunRecord` plus a JSON evidence log.

## 2. Out Of Scope (You Do Not Build This For The Demo)

| Thing | Why excluded |
| --- | --- |
| ME‑JEPA career trajectory pack | Too domain‑specific; covered by Phase 12 of the full plan, not the demo |
| `context_graph_memory.v1` / `learner_state.v1` packs | Those reuse production data; demo only uses synthetic |
| Text/code/sparse/graph_neighborhood instruments | Adds embedder dependency; instruments listed in §1 are sufficient to prove the contract |
| LLM‑driven binding (`model_predicted_equivalence`) | Demo's domains have deterministic identity; LLM would obscure FSV |
| Any of the 13 existing embedders (E1–E14) | Reusing them would conflate "did our JEPA learn?" with "is E1 any good?" |
| Multi‑step rollout objectives, masked‑slot, action‑contrast | One objective is enough to prove training works; add more after demo lands |
| Multi‑GPU, multi‑process, multi‑node training | Single 5090 |
| Production phase config | Demo runs `phase = "development"` |
| Touching any of the 73 existing CFs | Additive only |
| Bumping candle, llama.cpp bindings, or rust toolchain | Pinned for the demo |

If you find yourself reaching into out‑of‑scope land, halt and ask.

## 3. Hard Performance Targets

These are **acceptance gates**, not aspirations. Failure to meet a target is a structured error.

| Stage | Target on 5090 + 9950X3D + 128GB |
| --- | --- |
| Cold workspace build (`cargo build --workspace --release`) | ≤ 15 min |
| Counter_world full FSV (register → ingest → train → predict → plan → verify) | ≤ 60 s wall, of which ≤ 15 s training |
| Gridworld full FSV (1000 trajectories × 8 events, train, predict, plan, verify) | ≤ 5 min wall, of which ≤ 90 s training |
| Tiny model VRAM peak | ≤ 1 GB |
| Tiny model file size | counter_world ≤ 200 KB, gridworld ≤ 2 MB |
| `cargo test -p context-graph-dynamicjepa --release` | ≤ 60 s |

If gridworld training takes longer than 5 min, you have either over‑sized the model (see `10_tiny_jepa_model.md` §5) or you accidentally sent CPU tensors to the loss (verify with `nvidia-smi --query-gpu=utilization.gpu` during training).

## 4. Convergence Targets

The whole point of the demo is that the model *learns*. Random‑weight baselines must fail clearly.

| Metric | Counter_world acceptance | Gridworld acceptance |
| --- | --- | --- |
| Latent next‑panel cosine on validation | ≥ 0.99 | ≥ 0.95 |
| Latent next‑panel MSE on validation | ≤ 0.01 | ≤ 0.05 |
| Action‑contrast accuracy (positive vs negative panel) | ≥ 0.99 | ≥ 0.95 |
| Shuffled‑target sanity check (must FAIL with shuffled labels) | cosine ≤ 0.10 | cosine ≤ 0.30 |
| Random‑init baseline (no training) | cosine ≤ 0.10 | cosine ≤ 0.30 |
| VICReg variance per dim (non‑collapse check) | ≥ 0.5 | ≥ 0.5 |

If a randomly initialized predictor already meets the validation targets, you have a leakage bug — input features include the answer. Halt and audit `08_adapter_and_panel_pipeline.md`.

## 5. Test Strategy (How These Targets Get Enforced)

Current Phase 3 enforcement lives in `crates/context-graph-cli/tests/dynamicjepa_phase3_fsv.rs`: real CLI binary, real temp RocksDB, both checked-in domain TOMLs, decoded CF reads, and edge-case count invariants.

Phase 7+ model metrics get Rust tests asserting the bounds. Those tests will live in `crates/context-graph-dynamicjepa/tests/` once the service crate exists:

- `tests/counter_world_fsv.rs` — happy path + 3 edge cases + convergence asserts
- `tests/gridworld_fsv.rs` — happy path + 3 edge cases + convergence asserts
- `tests/collapse_audit.rs` — VICReg variance / covariance asserts on a frozen training run
- `tests/leakage_audit.rs` — shuffled‑targets asserts cosine drops below the random baseline
- `tests/artifact_hash_fsv.rs` — registers an artifact, recomputes hashes from disk, asserts equality, then corrupts a byte and asserts decode fails

These are **integration tests using real RocksDB** under `tempfile::TempDir`. No mocks. No synthetic embedders that bypass the real instruments. If a test passes with the system in a half‑broken state, the test is a bug.

## 6. Definition of Universality

The demo claims "universal by domain pack, not by special case". The proof: counter_world and gridworld

- share the same crate code,
- share the same storage CFs,
- share the same JSON adapter,
- share the same training loop,
- share the same FSV harness shape,

but use different instrument compositions, different state/action/outcome schemas, different segmentation, and different splits. Two domain packs is the minimum for "universal" — one would be a special case. If you find yourself hard‑coding `if domain == "counter_world"` anywhere outside fixture loaders, you have violated this rule.

## 7. What Counts As Done For Each Phase

The full Definition of Done is in `00_TASK.md` §4. Phase‑level acceptance:

- **Phase 0**: both TOMLs parse, all fixtures parse, expected‑count tables in `14_full_state_verification_protocol.md` agree with `06_…` and `07_…`.
- **Phase 1**: every record type round‑trips bincode in a real test; invalid records fail with typed errors carrying remediation hints.
- **Phase 2**: fresh `/tmp/` DB opens with all DynamicJEPA CFs and `dynamicjepa inspect-counts` returns zero across the board; corrupt version byte fails with codec error.
- **Phase 3**: registering both TOMLs writes the expected primary + name/version index + instrument registry rows in one batch; duplicate instrument id leaves counts unchanged.
- **Phase 4**: ingesting the happy‑path fixture (3 events for counter_world, 8 for one gridworld trajectory) produces the exact count deltas in `14_…`; invalid fixture writes a failed adapter run row but no normalized rows.
- **Phase 5**: trajectory compile produces the exact ordered transition list documented per domain.
- **Phase 6**: dataset shard row count = `transitions − 1` per trajectory; insufficient negatives fails before partial shard write.
- **Phase 7**: model trains and converges to the §4 targets; artifact hash recomputes to registered value; corrupt artifact load fails with a typed error.
- **Phase 8**: prediction row references the actual artifact + panel + action; plan trace references concrete predictions and guard decisions; surprise event links prediction → outcome → panel when threshold is crossed.
- **Phase 9**: every DynamicJEPA tool listed in `13_…` exists in `tools::names`, has a definition, has a handler, dispatches correctly, and has a corresponding capability‑matrix entry; tool count test bumped to the new total.
- **Phase 10**: `verify-counter-world` and `verify-gridworld` both pass on a fresh DB; verification run rows persist; JSON evidence under `tmp/5090jepa_evidence/` includes every required field per `14_…` §10.

## 8. Anti‑Patterns To Reject In Code Review

If you see any of these, the change does not merge:

- A `unwrap_or_default()` on a missing instrument reading.
- An empty `Vec` returned when a CF read fails.
- A `if cfg!(test)` branch that bypasses real RocksDB.
- A test that uses a `MockMemoryStore` or in‑memory stub when the production path is RocksDB.
- A handler that returns success without asserting the source‑of‑truth row exists.
- A `format!("...{}", err)` that swallows the structured error type.
- A new code path that decodes records without verifying the version byte.
- A new behavior with no FSV evidence in the PR description.

The user has been clear: "Do not cover up something not working correctly by making a test that passes when the project is in a broken state." Apply that ruthlessly.
