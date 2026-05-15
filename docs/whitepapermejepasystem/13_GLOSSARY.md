# 13 — Glossary

Every term defined once. Alphabetical.

---

**Active learning queue** — `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`. Chunks the daemon has flagged for operator review because they hit OOD thresholds, blind-spot z-scores, or low-density cells. Drainable via `mejepa active-learning list|label|dismiss` CLI.

**Adversarial corpus** — 30 hand-authored Python patches that exercise specific failure modes (`SubtleBugPassesTests`, `KnownGoodLookalike`, `EmbedderGaming`). Binds into the ship gate; predictor must classify ≥ 24/30 with `pass_at_high_confidence == 0`.

**Asymmetric embedder** — An embedder that produces different vectors depending on whether the input is "cause" vs "effect", "source" vs "target", or "document" vs "query". E5 (causal), E8 (graph), E10 (paraphrase) are asymmetric.

**Blast radius** — The set of files/symbols/tests/configs that a patch is predicted to affect, even if the patch doesn't directly edit them. Output of Q5 head.

**Blind spot** — A chunk where one embedder's view of the cell membership disagrees with another's by more than a calibrated z-score threshold. Blind spots get pulled into active learning at high priority.

**CBP (Continual Backprop)** — Plasticity-preservation technique for online retrain. Tracks per-unit utility; re-initializes low-utility units. Prevents the predictor's MLP from "dying" over months of training.

**Cell** — A row of `CF_MEJEPA_TCT_CELLS`. A group of chunks sharing the same teleological constellation key (typically `mutation_category × language × project`). Holds centroid, variance, oracle outcome distribution, Gτ threshold.

**Cell stratification** — The convergence requirement that *every cell* hits `prediction_oracle_correlation ≥ 0.95`, not just the aggregate. Prevents shipping on high overall correlation hiding poor adversarial-cell performance.

**CF (Column Family)** — RocksDB partition. ME-JEPA uses 51+ CFs (`CF_MEJEPA_PANELS`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_TCT_CELLS`, etc.).

**Chunk** — An AST-aware unit of code (function, class, import, test block, fixture). Decomposition from `python_ast_chunks.py`. The atomic unit the embedders see.

**Claim** — Natural-language assertion the AI agent makes about its edit (e.g., *"I added input validation to parse_user_input"*). Parsed into atomic `(verb, target, qualifier)` tuples for Q1 reconciliation.

**Conformal calibration** — Statistical procedure that converts raw predictor scores to calibrated probabilities with coverage guarantees. Per-cell residual distribution fit on a held-out calibration set.

**Constellation** — See **Teleological constellation**.

**`context-graph-mejepa-*` crates** — The Rust crates that implement the system. Each is independently testable and FSV-verified.

**Corpus** — The training data. Currently: 300-task SWE-bench Lite × 8 mutation categories = 2,400 (instance, mutation) cells → 399,777 AST chunks → ~4.9 M (chunk, embedder) records.

**Daemon** — The continuous self-optimization process. Tails the shift log, materializes panels, ingests operator feedback, retrains predictor, recalibrates conformal, evicts drifting cells.

**DDA (Data-Driven Augmentation)** — Per-chunk reward signals computed from embedder agreement: cosines to centroid, pairwise cosines, pairwise MIs, blind-spot z-scores. Persisted to `CF_MEJEPA_DDA_SIGNALS`.

**Docker oracle** — The official SWE-bench evaluator container per task instance. Runs the test suite and exit-codes with FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL / PASS_TO_FAIL sets. Deterministic ground truth for the `E_Oracle` instrument.

**Drift detector** — Bayesian small-window detector that fires when a cell's oracle outcome distribution or its embedding distribution shifts significantly. Triggers emergency eviction + retraining.

**EWC (Elastic Weight Consolidation)** — Catastrophic-forgetting protection. Per-parameter Fisher information tracked online; high-Fisher parameters get strong penalty against updates.

**E1, E2, ..., E14** — The embedder slot identifiers. See [`06_EMBEDDER_STACK.md`](06_EMBEDDER_STACK.md) for the full list.

**E_Oracle, E_AST, E_DataFlow, etc.** — Frozen instrument slot names. 15 total. See [`05_FROZEN_INSTRUMENT_PANEL.md`](05_FROZEN_INSTRUMENT_PANEL.md).

**Embedder** — A function from `text → Vec<f32>` (or `text → Vec<Vec<f32>>` for late-interaction). 12 active content embedders in the current Python forward-cache run.

**Embedder slot** — A registered (id, dim) pair in the `EmbedderId` enum. Asymmetric embedders consume two slots.

**Existence prediction** — Q1 head's job. Does the bytes-on-disk delta match the agent's natural-language claim?

**ext4 journal** — The Linux journaling filesystem on the OS root disk. `jbd2_log_wait_commit` is the kernel state when a process is waiting for an ext4 journal commit. Observed as a bottleneck when the hot stage is on ext4 instead of ZFS.

**Fail-closed** — Design principle: when a check fails or input is invalid, refuse the operation with a structured error code rather than guess or proceed. Used throughout: `MEJEPA_PANEL_DIM_MISMATCH`, `REDACTION_NONUTF8`, etc.

**Forward-cache run** — The job that produces per-chunk per-embedder vector records across the corpus. Currently grinding on aiwonder.

**Frozen instrument** — A deterministic function with no trainable parameters that produces part of the target panel. Why ME-JEPA can't collapse.

**FSV (Forensic Source-Verified)** — Project verification protocol. Every primitive emits structured evidence (`before / after / readback`) to `/mnt/d/contextgraph/fsv/<task>-fsv/` or `/zfs/archive/contextgraph/fsv/...`. The truth contract for "is this implemented correctly?"

**Gold patch** — The actual fix in the SWE-bench Lite task. Should produce `OracleVerdict::Pass` on the Docker oracle. The `known_good` mutation category is identity-on-gold-patch.

**Gτ guard** (gamma-tau guard) — Per-cell threshold on chunk-to-centroid distance. If a chunk's distance exceeds Gτ, the predictor emits `Verdict::GuardRejected`. The cell-membership-conflict check.

**Hot root** — The fast-disk staging area for in-progress shard work. Currently on `/home/croyse/.cache/contextgraph/...` (ext4 OS root). Future optimization: move to ZFS hot pool.

**Instrument** — See **Frozen instrument**.

**Isolated embedder processes** — The `--isolate-embedder-processes` Rust runner flag that spawns one cargo subprocess per embedder per shard. Enables par=6 GPU concurrency on a single 32 GB RTX 5090.

**JEPA (Joint-Embedding Predictive Architecture)** — Class of architectures where a predictor maps input → some representation that matches a target representation produced from a related input. ME-JEPA-Code's innovation: the target representation has zero trainable parameters.

**LLM (Large Language Model)** — Not part of ME-JEPA. The untrusted edit generator. ME-JEPA verifies the LLM's output without consulting the LLM.

**MCP (Model Context Protocol)** — JSON-RPC stdio protocol for AI agent ↔ tool communication. ME-JEPA serves 28 `mejepa_*` tools over MCP.

**Mutation category** — One of 8 deterministic code mutations applied to gold patches: `known_good`, `subtle_flip`, `off_by_one`, `swap_variable`, `delete_test_call`, `wrong_file`, `over_engineer`, `compile_error`.

**Online retrain** — The daemon's continuous training cycle that ingests live predictions, operator feedback, and shift log events.

**OOD (Out of Distribution)** — Verdict emitted when the joint embedding falls outside the training distribution. Predictor refuses to commit to Pass/Fail.

**Operator** — The human running the system. Not the AI agent. Has authority to override predictions (`mejepa_record_agent_feedback`), approve promotions (`mejepa_promote_approval`), and edit forbidden paths.

**Operator override** — `mejepa_record_agent_feedback` invocation. Pulled into replay buffer at 6× weight.

**Panel** — The 5,120-d target representation produced by the 15 frozen instruments for a given (chunk, edit) event. Stored in `CF_MEJEPA_PANELS`.

**par (parallel)** — The `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS` env var. Number of cargo subprocesses run concurrently per shard. par=6 stable; par=12 OOMs.

**Pearson correlation** — The statistic the ship gate measures. ρ ≥ 0.95 per cell.

**Phase G** — The current convergence phase. Targets the 0.95 oracle correlation gate. Item-by-item plan in `docs/futurebuild/27_PHASE_G_ROADMAP.md`.

**Prediction ID** — `Vec<u8; 16>` (32 hex chars). Unique key per emitted `RealityPrediction`. Used in `mejepa_record_agent_feedback` to link feedback to prediction.

**Predictor** — The trained MLP head. Maps (joint embedding, panel_t1) to (predicted panel_t2). The only part of the system that has trainable parameters.

**Q1, Q2, Q3, Q4, Q5** — The five operator questions: EXIST, WORK, WHY, WHAT ELSE, IMPACT. See [`04_FIVE_OPERATOR_QUESTIONS.md`](04_FIVE_OPERATOR_QUESTIONS.md).

**Quantization tiers** — 8/7/5/3-bit per-vector linear scalar quantization. Hot/warm/cold/archive storage. Cosine retention 0.99999 / 0.99997 / 0.99960 / 0.99192 on 768-d Gaussian.

**Reality compiler** — The (planned) per-project full-rerun of the chunker + embedder + instrument pipeline against a customer's repo. Materializes the per-project cells + per-project Merkle cache. Foundation for repository-specific invariant learning.

**Reality-shift principle** — *"No alteration without current state. No alteration without a delta back into perception."* Pre/post-tool hooks enforce this.

**RealityPrediction** — The structured output of a prediction call. Contains Q1-Q5 fields + verdict + confidence + conformal interval + nearest exemplars + violating embedders.

**Replay buffer** — `CF_MEJEPA_REPLAY_BUFFER`. Prioritized sampling buffer for the daemon's online retrain. Operator overrides at 6× weight.

**Routing** — Per-cell weight profile lookup. `mejepa_routing_lookup` returns which embedders' opinions count most for a given cell.

**Shard** — A chunk of 1024 chunks the forward-cache run processes as a unit. 391 shards in the seed corpus.

**Ship gate** — `prediction_oracle_correlation ≥ 0.95 × 4 windows × per cell`. The single convergence metric.

**Shift log** — Append-only JSONL of typed reality events (Edit, Write, test outcomes, prediction emissions). Tailed by the shift subscriber.

**Shift subscriber** — Tokio task in `context-graph-mejepa-shift-subscriber` that tails the shift log and materializes panels.

**SoT (Source of Truth)** — The durable bytes on disk. The thing FSV evidence reads back from. Never a return value, never an LLM's claim. The `/zfs/archive/contextgraph/` paths on aiwonder, the RocksDB row, the JSONL row.

**SWE-bench Lite** — The 300-task Python benchmark from Princeton NLP. `princeton-nlp/SWE-bench_Lite` on HuggingFace.

**Target representation** — `f_target(panel_t2)`. The 5,120-d output of the frozen instruments. The predictor's training objective.

**TCT (Teleological Constellation)** — The intent-linked chain from operator goal → oracle outcome. Cell key includes mutation category, language, project.

**Teleological constellation** — See TCT.

**True batch** — The actual GPU forward batch size, distinct from the per-embedder batch size. `--true-batch-size 8` means each GPU call processes 8 chunks.

**UTML (Unified Training Markup Language)** — Per-cell parameter carrier system for loss weights, Fisher info, dormancy scores, dissipation thresholds. Implementation: TASK-UTML-005..008.

**Verdict** — The Q2 head's output: `Pass | Fail | OutOfDistribution | Abstain | GuardRejected`.

**Witness chain** — Merkle-rooted hash chain of chunk contents + parent commits. The `E_Witness` instrument's input. Implementation in `context-graph-witness` crate.

**ZFS** — The Zettabyte File System. aiwonder uses ZFS for the hot pool (1.8 TB NVMe) and archive pool (9.0 TB HDD mirror). Mount points: `/hotpool` and `/archive`. `/zfs/hot/contextgraph/` and `/zfs/archive/contextgraph/` are the project's working / durable roots.

---

**See also**: [`CLAUDE.md`](../../CLAUDE.md) for the project-level vocabulary, [`docs2/AICodingAgentSuperPrompt.md`](../../docs2/AICodingAgentSuperPrompt.md) for the doctrinal vocabulary.
