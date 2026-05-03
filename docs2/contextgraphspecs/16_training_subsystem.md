# Training Subsystem

**Source files covered:**
- `crates/context-graph-core/src/training/{mod,computation,edge_signature,temporal}.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs`
- `crates/context-graph-storage/src/teleological/column_families.rs` (`CF_TRAINING_RECORDS`, `CF_TYPED_EDGE_RECORDS`, `CF_LEARNER_TRAINING_DATASETS`)
- `crates/context-graph-mcp/src/tools/definitions/{training,typed_edges,learning}.rs`
- `crates/context-graph-cli/src/commands/export_training.rs`
- `crates/context-graph-embeddings/src/training/{mod,data,loss,optimizer,trainer,pipeline,lora,multitask,evaluation,distillation}.rs`
- `crates/context-graph-embeddings/src/bin/train_causal.rs`
- `crates/context-graph-benchmark/src/bin/*.rs`, `crates/context-graph-benchmark/src/{ablation,causal_bench,baseline,validation,realdata,scaling,tuning,runners,metrics,reports,datasets}/`

There are two distinct training surfaces in this repo:

1. **Training-data export** (in `context-graph-core` + `-storage` + `-mcp` + `-cli`) — turns the live memory graph into bincode `TrainingRecord` rows persisted to RocksDB column families. Used to feed offline pipelines.
2. **Causal embedder fine-tuning** (in `context-graph-embeddings/src/training/` + `bin/train_causal.rs`) — a 3-stage progressive LoRA + projection trainer for the E5 nomic-embed-text-v1.5 backbone. Outputs `models/causal/trained/{lora,projection_stage{1,2,3}_best}.safetensors`.

Plus an online distillation loop (LLM → embedder buffer) that piggy-backs on the same trainer.

## 1. TrainingRecord (v3) — Offline Export

`TRAINING_RECORD_VERSION = 3` (`crates/context-graph-core/src/training/mod.rs:67`). v1/v2 records are unreadable by the v3 decoder; per the no-backwards-compat policy, callers must wipe the column family and re-export.

### Record shape (`TrainingRecord`)

| Group | Field(s) | Notes |
| --- | --- | --- |
| Identity | `memory_id: Uuid`, `content: String`, `importance: f32`, `created_at: DateTime<Utc>`, `session_id: Option<String>` | |
| Source provenance | `source_type: Option<String>`, `source_path: Option<String>`, `content_hash: Option<[u8; 32]>` | SHA-256 of source content from `TeleologicalFingerprint` |
| Dense embeddings | `e1_semantic` (1024), `e2_temporal_recent` (512), `e3_temporal_periodic` (512), `e4_temporal_positional` (512), `e7_code` (1536), `e9_hdc` (1024), `e11_entity` (768), `e14_bge_m3_dense` (1024) | All L2-normalized; `Vec::new()` when `includeEmbeddings=false` |
| Asymmetric | `e5_causal_cause` (768), `e5_causal_effect` (768), `e8_graph_source` (1024), `e8_graph_target` (1024), `e10_paraphrase` (768), `e10_context` (768) | |
| Sparse | `e6_sparse_indices: Vec<u16>` + `e6_sparse_values: Vec<f32>`, `e13_splade_indices`, `e13_splade_values` | empty when `includeSparseVectors=false` |
| Token | `e12_token_embeddings: Vec<Vec<f32>>` (128D / token) | empty when `includeTokenEmbeddings=false` |
| Fusion | `topic_profile: [f32; 14]`, `cross_correlations: Vec<f32>` (length 91), `group_alignments: [f32; 6]` | Sourced from `CF_TOPIC_PROFILES` when present, else per-embedder vector-presence fallback |
| Graph | `outgoing_edges: Vec<TrainingEdge>`, `incoming_edges: Vec<TrainingEdge>`, `knn_neighbors: Vec<Vec<KnnNeighbor>>` (always 14 outer slots) | |
| Causal | `causal_effects: Vec<CausalLabel>`, `causal_causes: Vec<CausalLabel>` | LLM-discovered |
| Topics | `topic_memberships: Vec<TopicMembership>` | Each carries `topic_profile: [f32; 14]` |
| Temporal | `temporal_labels: Option<TemporalLabels>` | Phase-5 hour/day/month buckets, age, session position |
| Tucker | `tucker_core: Option<TuckerCore>` | Phase-4 hook; `None` until Agent D populates it |
| Typed-edge sig | `edge_type_distribution: [u32; 8]` | bincode positional, MUST stay last |

`NUM_CROSS_CORRELATIONS = 91` (= C(14, 2)); `NUM_GROUP_ALIGNMENTS = 6`; `NUM_EDGE_TYPE_DISTRIBUTION = 8`.

### `TrainingEdge`

| Field | Type | Notes |
| --- | --- | --- |
| `edge_type` | `u8` | `GraphLinkEdgeType::as_u8`: 0=SemanticSimilar, 1=CodeRelated, 2=EntityShared, 3=CausalChain, 4=GraphConnected, 5=ParaphraseAligned, 6=KeywordOverlap, 7=MultiAgreement |
| `peer_id` | `Uuid` | target for outgoing edges, source for incoming |
| `weight` | `f32` | |
| `direction` | `u8` | `Symmetric=0`, `Forward=1`, `Backward=2` |
| `agreement_count` | `u8` | |
| `embedder_scores` | `[f32; 14]` | Per-embedder similarity at edge creation time |

### `CausalLabel`

| Field | Notes |
| --- | --- |
| `related_memory_id: Uuid` | `Uuid::nil()` when the peer end isn't resolvable |
| `rel_id: Uuid` | Always the `CausalRelationship.id` (joinable against `CF_CAUSAL_RELATIONSHIPS`) |
| `description: String` | `"cause_statement -> effect_statement. explanation"` |
| `direction: String` | `"cause"` / `"effect"` |
| `confidence: f32` | |
| `mechanism_type: Option<String>` | `"direct"`, `"mediated"`, `"feedback"`, `"temporal"` |

### Persistence

Records are bincode-serialized with a single version byte prefix (no `skip_serializing_if` per constitution). Stored in `CF_TRAINING_RECORDS` (`crates/context-graph-storage/src/teleological/column_families.rs:399`). Iteration helpers and version handling live in `context_graph_storage::teleological::rocksdb_store::export`.

## 2. MCP Training Tools

`crates/context-graph-mcp/src/tools/definitions/training.rs` registers four tools:

| Tool | Inputs | Behavior |
| --- | --- | --- |
| `export_training_corpus` | `filter: "all"|"session"`, `filterId`, `maxMemories` (default 10000, max 1_000_000), `includeEmbeddings` (default true), `includeSparseVectors`, `includeTokenEmbeddings`, `includeEdges`, `includeCausalLabels`, `includeIncomingEdges`, `includeTemporalLabels`, `includeTuckerCore`, `clearExisting` | Iterates non-soft-deleted fingerprints, assembles `TrainingRecord`, writes to `CF_TRAINING_RECORDS`. Output is binary only (bincode + version byte) |
| `list_training_records` | `limit`, `offset`, `includeShape` | |
| `get_training_record` | `memoryId`, `includeVectors`, `includeTokenEmbeddings`, `includeEdges` | |
| `count_training_records` | — | |

`crates/context-graph-mcp/src/tools/definitions/typed_edges.rs` also exposes:

| Tool | Output CF |
| --- | --- |
| `export_typed_edges_corpus` | `CF_TYPED_EDGE_RECORDS` (one `TypedEdgeTrainingRecord` per typed edge — see [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md)) |
| `list_typed_edge_records`, `get_typed_edges`, `validate_low_confidence_edges` | |

`crates/context-graph-mcp/src/tools/definitions/learning.rs` (learner side):

| Tool | Output CF |
| --- | --- |
| `export_learner_training_dataset` | `CF_LEARNER_TRAINING_DATASETS` (per-learner E15-E21 signal datasets — see [20_learning_subsystem.md](20_learning_subsystem.md)) |
| `list_learner_training_datasets`, `get_learner_training_dataset`, `count_learner_training_datasets` | |

## 3. CLI Export

`crates/context-graph-cli/src/commands/export_training.rs`:

```text
context-graph-cli export training-corpus
  --batch_size <usize>
  --limit <usize>
  --out <path>
  --storage <path>
```

Same code path as `export_training_corpus` MCP tool; reads from the configured RocksDB store and writes records into `CF_TRAINING_RECORDS`.

## 4. Causal Embedder Fine-Tuning Pipeline

`crates/context-graph-embeddings/src/training/`:

```text
LLM (Hermes 2 Pro) ──> CausalTrainingPair ──> CausalDataLoader
                                                    │
                                              ┌─────┴─────┐
                                              │  CausalTrainer  │
                                              │  ┌───────┐ │
                                              │  │ Loss:  │ │
                                              │  │ InfoNCE│ │
                                              │  │ Dir.   │ │
                                              │  │ Sep.   │ │
                                              │  │ Soft   │ │
                                              │  │ Multi  │ │
                                              │  │  Task  │ │
                                              │  └───────┘ │
                                              │  ┌───────┐ │
                                              │  │AdamW  │ │
                                              │  └───────┘ │
                                              └─────┬─────┘
                                                    │
                                       LoRA (Q,V) + W_cause/W_effect/biases
                                       + DirectionHead + MechanismHead (optional)
```

### 4.1 `CausalTrainingPair` (`training/data.rs`)

| Field | Type | Notes |
| --- | --- | --- |
| `cause_text` | `String` | |
| `effect_text` | `String` | |
| `direction` | `TrainingDirection::{Forward,Backward,Bidirectional,None}` | LLM label |
| `confidence` | `f32` ∈ `[0,1]` | Soft label |
| `mechanism` | `String` | "biological", "economic", … |
| `hard_negative` | `String` | Semantically similar but non-causal |
| `rationale` | `Option<String>` | Optional LLM rationale |
| `domain` | `String` | Curriculum-learning category (default `"general"`) |

`CausalTrainingPair::is_causal()` = `direction != None && confidence >= 0.5`. `difficulty()` is used for stage-1 curriculum filtering.

### 4.2 LoRA Adapters (`training/lora.rs`)

`LoraConfig` defaults:

| Field | Default |
| --- | --- |
| `rank` | 16 |
| `alpha` | 16.0 (so scale = 1.0) |
| `dropout` | 0.1 |
| `hidden_size` | 768 (NomicBERT) |
| `num_layers` | 12 |
| `apply_query` | true |
| `apply_value` | true |
| `apply_key` | false |

`total_params` = `2 * hidden_size * rank * (apply_query + apply_value + apply_key) * num_layers` = 24,576 per adapter × (Q + V) × 12 layers = **589,824** trainable LoRA parameters.

`LoraAdapter`:
- `a: Var [hidden_size, rank]` initialized via deterministic golden-ratio low-discrepancy quasi-random with Kaiming scale.
- `b: Var [rank, hidden_size]` initialized to zero (LoRA starts as identity).
- `forward(x)` = `scale * (x @ A @ B)` (no dropout).
- `forward_train(x)` = `scale * dropout(x @ A @ B, p=dropout)`.

`LoraLayers` collection:

| Field | Type |
| --- | --- |
| `query_adapters` | `Vec<LoraAdapter>` (length = num_layers when `apply_query`) |
| `value_adapters` | `Vec<LoraAdapter>` |
| `key_adapters` | `Vec<LoraAdapter>` (empty unless `apply_key`) |
| `config` | `LoraConfig` |
| `training` | **`Cell<bool>`** |

The `Cell<bool>` lets `&self` methods toggle dropout on/off without `&mut self`. `set_training(false)` must be called before evaluation passes (`pipeline.rs:564`) and restored afterward (`pipeline.rs:618`). Stage entry points call `set_training(true)` then `set_training(false)` after the stage finishes (`pipeline.rs:933,948,1061,1076`).

**Lesson (load-bearing):** LoRA dropout adds dropout-mask tensors to the autograd graph — VRAM consumption rises. Default `batch_size=16` was reduced from `32` once dropout was enabled, and re-raising it can OOM RTX 5090. When dropout is disabled or removed, gate thresholds must be recalibrated because dropout regularization compresses absolute scores.

### 4.3 Loss Functions (`training/loss.rs`)

`LossConfig` defaults:

| Field | Default | Purpose |
| --- | --- | --- |
| `temperature` | 0.05 | InfoNCE (Causal2Vec) |
| `margin` | 0.2 | Directional triplet margin |
| `lambda_contrastive` | 1.0 | InfoNCE weight |
| `lambda_directional` | 0.3 | Directional triplet weight (Stage 3 multiplies by `stage3_direction_weight = 0.6`) |
| `lambda_separation` | 0.1 | Causal-vs-noncausal separation |
| `lambda_soft` | 0.2 | Soft-label distillation |
| `false_negative_threshold` | 0.8 | Min E1 cosine to exclude as potential false negative |
| `hard_negative_scale` | **1.0** (disabled) | Multiplier on hard-negative logits before softmax |
| `hard_negative_count` | 3 | Top-k hard negatives per sample |

Components implemented in `DirectionalContrastiveLoss`:

1. **InfoNCE contrastive** — `info_nce_loss(cause_vecs, effect_vecs)` over an N×N similarity matrix; uses `candle_nn::loss::cross_entropy` for differentiable gather-based indexing.
2. **Directional margin** — pulls forward similarity above reverse similarity by `margin`.
3. **Causal separation** — pushes causal embeddings away from non-causal embeddings.
4. **Soft-label distillation** — uses `pair.confidence` as soft target.

`LossComponents` aggregates `{ contrastive, directional, separation, soft_label, total }` for logging.

**Lesson (load-bearing):** `hard_negative_scale >= 2.0` combined with `temperature = 0.05` causes catastrophic score collapse — cause/effect vectors become orthogonal. Keep `hard_negative_scale <= 1.3` if enabling. The default 1.0 disables the amplifier entirely.

### 4.4 AdamW Optimizer (`training/optimizer.rs`)

`AdamWConfig` defaults:

| Field | Default | Notes |
| --- | --- | --- |
| `lr_projection` | 1e-4 | W_cause / W_effect / biases |
| `lr_lora` | 1e-5 | LoRA params (10× smaller) |
| `lr_markers` | 5e-4 | Marker MLP params |
| `beta1` | 0.9 | |
| `beta2` | 0.999 | |
| `epsilon` | 1e-8 | |
| `weight_decay` | 0.01 | Decoupled |
| `max_grad_norm` | 1.0 | Gradient clipping |
| `total_steps` | 1000 | Schedule basis |
| `warmup_fraction` | 0.1 | 10% of total_steps |

`ParamGroup::{Projection,Lora,Markers}` lets each group ride its own learning rate. Schedule = linear warmup followed by cosine decay (`current_lr` in `optimizer.rs:108-128`).

### 4.5 Multi-task Heads (`training/multitask.rs`)

Optional auxiliary tasks; active in stages 2+3 when `PipelineConfig::multitask_config = Some(MultiTaskConfig::default())` (default).

| Head | Input | Output |
| --- | --- | --- |
| `direction_head: ClassificationHead` | concatenated cause+effect [N, 2D] | 3-class logits (`DirectionLabel::{Forward, Backward, None}`) |
| `mechanism_head: ClassificationHead` | concatenated cause+effect [N, 2D] | 7-class logits (`MechanismLabel::{Biological, Economic, Physical, Technical, Social, Ecological, Other}`) |

Each head is a 2-layer MLP (linear → ReLU → linear) with Xavier-flavoured deterministic init. Cross-entropy loss against `LongTensor` labels.

### 4.6 Trainer (`training/trainer.rs`)

`TrainingConfig` defaults:

| Field | Default |
| --- | --- |
| `batch_size` | 32 |
| `epochs` | 50 |
| `eval_every` | 5 |
| `checkpoint_every` | 10 |
| `checkpoint_dir` | `models/causal/trained` |
| `early_stopping_patience` | 10 |
| `momentum_tau` | 0.999 |
| `seed` | 42 |
| `loss_config` | `LossConfig::default()` |
| `optimizer_config` | `AdamWConfig::default()` |

`MomentumProjection` — exponential moving average of `TrainableProjection` weights:
- `W_mom ← τ * W_mom + (1 - τ) * W_online` (Polyak / MoCo).
- All updates `.detach()` to prevent autograd-graph chains.
- Provides stable negative targets without requiring huge batch sizes.

`EpochResult` records `{ epoch, avg_loss: LossComponents, num_batches, eval_metrics, is_best }`. `TrainingHistory` accumulates per-epoch results, best directional accuracy, best epoch, early-stop flag, total steps.

### 4.7 3-Stage Progressive Pipeline (`training/pipeline.rs`)

`PipelineConfig` defaults:

| Field | Default | Purpose |
| --- | --- | --- |
| `lora_config` | `LoraConfig::default()` | |
| `stage1_epochs` | 25 | Projection-only warm-up |
| `stage2_epochs` | 20 | LoRA activation |
| `stage3_epochs` | 20 | Directional emphasis |
| `stage1_max_difficulty` | 0.2 | Curriculum filter |
| `stage3_direction_weight` | 0.6 | Multiplier on `lambda_directional` |
| `batch_size` | 16 | Reduced from 32 due to LoRA dropout VRAM cost |
| `eval_every` | 5 | |
| `early_stopping_patience` | 10 | |
| `output_dir` | `models/causal/trained` | |
| `seed` | 42 | |
| `multitask_config` | `Some(MultiTaskConfig::default())` | |

| Stage | Epochs | Trainable | Pairs | Use LoRA at forward? |
| --- | --- | --- | --- | --- |
| 1 — warm-up | 25 | projection only (W_cause, W_effect, biases) | difficulty ≤ 0.2 | no (saves VRAM; skip the LoRA path entirely via `gpu_forward_base_tensor`) |
| 2 — LoRA activation | 20 | projection + LoRA + multi-task heads | all pairs | yes (`gpu_forward_with_lora_tensor`) |
| 3 — directional | 20 | projection + LoRA + multi-task heads, with `lambda_directional` boosted | all pairs | yes |

Forward path during training:
- `cause_prefixed = "search_document: " + pair.cause_text` (NOT `search_query: …` — instruction prefixes were collapsing all vectors to the same direction; the projection heads now learn cause/effect separation themselves).
- Same for effect.
- Encoder runs through `attention::create_strategy(&AttentionMode::Dense)` (sequences capped at 512 tokens).
- `l2_normalize` applied before loss computation.

**Per-stage best-stage selection (load-bearing):**
- Inside a stage, the best checkpoint selected by `metrics.directional_accuracy > best.directional_accuracy` and saved as `projection_stage{N}_best.safetensors`.
- The benchmark snapshot path uses `metrics.score_spread` (`pipeline.rs:668-683`) — `score_spread` (top-1 minus rank-5 cosine) is more discriminative than directional_accuracy for global "is this stage actually better than the previous" comparisons.
- Why per-stage: because each stage learns a different projection, the best stage's LoRA must pair with that stage's projection. Cross-stage mixing degrades inference quality.

### 4.8 Evaluation (`training/evaluation.rs`)

`EvaluationMetrics`:

| Field | Target |
| --- | --- |
| `directional_accuracy` | > 0.90 |
| `topical_mrr` | > 0.7 |
| `causal_auc` | > 0.85 |
| `direction_ratio` | > 2.0 |
| `cross_topic_rejection` | > 0.80 |
| `score_spread` | > 0.10 (top-1 cosine − rank-5 cosine) |
| `anisotropy` | < 0.30 (avg pairwise cosine of random pairs) |
| `standalone_accuracy` | ≥ 0.67 (E5-only top-1 across 6-pair benchmark) |
| `num_pairs` | (count) |

`meets_targets()` checks the original 5; `meets_finetuning_targets()` checks the Option B trio (`score_spread`, `anisotropy`, `standalone_accuracy`).

### 4.9 Online Distillation (`training/distillation.rs`)

`DistillationBuffer` accumulates LLM-validated `CausalTrainingPair`s and triggers incremental training rounds.

`DistillationConfig` defaults:

| Field | Default |
| --- | --- |
| `min_pairs` | 100 |
| `max_buffer_size` | 10000 |
| `confidence_threshold` | 0.8 |
| `incremental_epochs` | 10 |
| `ema_tau` | 0.999 |
| `quality_gate` | true |
| `checkpoint_dir` | `models/causal/trained` |

Quality gate: only accept new weights if metrics improve over `best_metrics`; otherwise roll back. EMA prevents catastrophic drift across rounds.

### 4.10 Training Binary

`crates/context-graph-embeddings/src/bin/train_causal.rs`:

```bash
train-causal --model-path models/causal --output models/causal/trained
train-causal --model-path models/causal --data pairs.jsonl --epochs 50
```

CLI flags: `--model-path`, `--data` (JSONL), `--output`, `--epochs`, `--batch-size`, `--lora-rank`, `--eval-every`, `--seed`, `--no-multitask` (saves VRAM).

`expand_seed_pairs()` and `seed_training_pairs()` (in `training/data.rs`) provide a built-in seed corpus for smoke runs without external data.

### 4.11 Checkpoint Files

After a successful pipeline run, `models/causal/trained/` contains:

| File | Source |
| --- | --- |
| `lora.safetensors` | LoRA `a`/`b` matrices for all (Q,V[,K]) adapters across 12 layers |
| `projection_stage1_best.safetensors` | Best Stage-1 projection (warm-up) |
| `projection_stage2_best.safetensors` | Best Stage-2 projection (LoRA activation) |
| `projection_stage3_best.safetensors` | Best Stage-3 projection (directional emphasis) |
| `training_trajectory.jsonl` | Append-only per-stage benchmark snapshots: `{ timestamp, stage, score_spread, directional_accuracy, standalone_top1, direction_ratio, duration_ms }` |

`ProductionMultiArrayProvider::new` loads `lora.safetensors` + the best projection eagerly via `e5_causal_model.load_trained_weights(&trained_dir)` and FAILS FAST if these files are missing — the search gate cannot calibrate without them.

## 5. Benchmark Suite (`crates/context-graph-benchmark/`)

The `bin/` directory holds 40+ standalone benchmark binaries. Library modules:

| Module | Purpose |
| --- | --- |
| `ablation.rs` | A/B ablation harness |
| `baseline/` | Baseline implementations for regression checks |
| `causal_bench/` | Causal-specific benchmarks |
| `config.rs` | `BenchConfig` shared parameters |
| `datasets/` | Synthetic + real dataset loaders |
| `metrics/` | Metric aggregation |
| `realdata/` | Real-data harnesses (live RocksDB / live HF embedders) |
| `reports/` | Markdown / JSON report writers |
| `runners/` | Phase orchestrators |
| `scaling/` | Throughput / latency sweeps |
| `stress_corpus.rs` | Embedder stress data |
| `tuning/` | Threshold sweepers |
| `util.rs`, `validation/` | Helpers |

Notable benchmark binaries relevant to training:

| Binary | Role |
| --- | --- |
| `causal_accuracy_bench`, `causal_e2e_bench`, `causal_perf_bench`, `causal_realdata_bench`, `causal_provenance_bench`, `causal_provenance_e2e_bench`, `causal_provenance_llm_bench`, `causal_prompt_experiment_bench`, `causal_span_fix_bench`, `causal_discovery_mcp_bench`, `causal_embedding_bench` | Validate E5 fine-tuning impact end-to-end |
| `e1_semantic_bench`, `e7_bench`, `e7_realdata_bench`, `e10_modifier_tuning`, `e11_entity_bench`, `e2_manual_verification`, `e4_hybrid_session_bench`, `embedder_impact_bench`, `embedder_impact_realdata_bench`, `embedder_stress` | Per-embedder regression / tuning |
| `gpu_bench` | GPU similarity / kernel benchmarks |
| `hf_bench` | HuggingFace embedder direct calls |
| `mcp_bench` | MCP server throughput |
| `realdata_bench`, `unified_realdata_bench` | Live-DB end-to-end |
| `model_injection_bench`, `model_injection_rigorous_bench` | Load-time injection |
| `report.rs` | Report writer |

**Lesson (load-bearing):** Benchmark phases that don't accept `&BenchConfig` cannot exercise the model under test — they're testing infrastructure or hardcoded constants only. If a metric never changes across runs, it is testing hardcoded behavior; flag and remove. Split scoring into model-dependent vs infrastructure-dependent for honest reporting. Provider failures in benchmarks must `panic` (fail-fast) — silent `0.0` corrupts results.

## 6. Versioning Policy

- `TRAINING_RECORD_VERSION = 3` rejects v1/v2 records on read; pre-production policy is **wipe and re-export**, not migrate.
- Bincode positional serialization. Any new field MUST be appended at the end. `skip_serializing_if` is forbidden per constitution.
- `training_records` CF in the `is_versioned_cf` set (`column_families.rs:29`); legacy CFs intentionally not retrofitted with version bytes.

## 7. Cross-references

- Embedder topology consumed by training (E1-E14, E5 dual vectors): [05_embedding_subsystem.md](05_embedding_subsystem.md).
- GPU device singleton shared between training and inference: [11_cuda_acceleration.md](11_cuda_acceleration.md) §7-§8.
- Typed-edge training records and contrastive pair mining: [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).
- DPO pair mining off the training corpus: [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md).
- Learner training datasets (E15-E21 namespace) — separate column family `CF_LEARNER_TRAINING_DATASETS`: [20_learning_subsystem.md](20_learning_subsystem.md).
- Causal discovery LLM that supplies `CausalTrainingPair`s: [07_graph_and_causal.md](07_graph_and_causal.md).
