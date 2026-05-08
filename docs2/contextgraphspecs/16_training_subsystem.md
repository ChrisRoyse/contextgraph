# Training Subsystem

**Source files covered:**
- `crates/context-graph-core/src/training/{mod,computation,edge_signature,temporal,tests}.rs`
- `crates/context-graph-core/src/learner_training.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/{export,learner_training,learning,learner}.rs`
- `crates/context-graph-storage/src/teleological/column_families.rs` (`CF_TRAINING_RECORDS`, `CF_TYPED_EDGE_RECORDS`, `CF_LEARNER_TRAINING_DATASETS`, `CF_LEARNING_EVENTS`)
- `crates/context-graph-mcp/src/handlers/tools/training_tools.rs` (~945 lines)
- `crates/context-graph-mcp/src/tools/definitions/{training,typed_edges,learning}.rs`
- `crates/context-graph-cli/src/commands/{export_training,learning,utl}.rs`
- `crates/context-graph-cli/tests/{export_training_integration,learning_jsonl_export_integration}.rs`
- `crates/context-graph-storage/tests/{utl_learner_state_fsv,utl_learner_embedders_fsv,training_records_integration}.rs`
- `crates/context-graph-embeddings/src/training/{mod,data,loss,optimizer,trainer,pipeline,lora,multitask,evaluation,distillation}.rs`
- `crates/context-graph-embeddings/src/learner/mod.rs` (E15-E21 specs + calibration manifest)
- `crates/context-graph-embeddings/fixtures/utl_calibration/{dataset_manifest.json,phase0_synthetic_fixture.json}`
- `crates/context-graph-embeddings/src/bin/train_causal.rs`
- `crates/context-graph-benchmark/src/bin/*.rs`, `crates/context-graph-benchmark/src/{ablation,causal_bench,baseline,validation,realdata,scaling,tuning,runners,metrics,reports,datasets}/`

There are three distinct training surfaces in this repo:

1. **Training-data export** (`context-graph-core` + `-storage` + `-mcp` + `-cli`) — turns the live memory graph into bincode `TrainingRecord` rows persisted to RocksDB. Used to feed offline pipelines.
2. **Causal embedder fine-tuning** (`context-graph-embeddings/src/training/` + `bin/train_causal.rs`) — a 3-stage progressive LoRA + projection trainer for the E5 nomic-embed-text-v1.5 backbone. Outputs `models/causal/trained/{lora,projection_stage{1,2,3}_best}.safetensors`.
3. **Learner-as-UTL training datasets** (`context-graph-core/src/learner_training.rs`) — matrix-shard compilation of `CF_LEARNING_EVENTS` and learner-state CFs into row-major f32 tensors with SHA-256 manifest, persisted to `CF_LEARNER_TRAINING_DATASETS`.

Plus an online distillation loop (LLM → embedder buffer) that piggy-backs on the same trainer (`context-graph-embeddings/src/training/distillation.rs`).

Cross-references:
- DPO/CAMD downstream consumers of these outputs: [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md).
- Learner-state subsystem and 7-slot E15-E21 namespace: [20_learning_subsystem.md](20_learning_subsystem.md).
- Embedder topology consumed by training (E1-E14, E5 dual vectors): [05_embedding_subsystem.md](05_embedding_subsystem.md).
- Typed-edge training records and contrastive pair mining: [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).
- Memory-record fingerprint shape: [04_storage_layer.md](04_storage_layer.md).

## 1. TrainingRecord (v3) — Offline Export

`TRAINING_RECORD_VERSION = 3` (`crates/context-graph-core/src/training/mod.rs:67`). v3 adds `e14_bge_m3_dense`. v1/v2 records are unreadable by the v3 decoder; per the no-backwards-compat policy callers must wipe the column family and re-export.

### 1.1 Record shape (`TrainingRecord`)

| Group | Field(s) | Notes |
| --- | --- | --- |
| Identity | `memory_id: Uuid`, `content: String`, `importance: f32`, `created_at: DateTime<Utc>`, `session_id: Option<String>` | |
| Source provenance | `source_type: Option<String>`, `source_path: Option<String>`, `content_hash: Option<[u8; 32]>` | SHA-256 of source content from `TeleologicalFingerprint` |
| Dense embeddings | `e1_semantic` (1024), `e2_temporal_recent` (512), `e3_temporal_periodic` (512), `e4_temporal_positional` (512), `e7_code` (1536), `e9_hdc` (1024), `e11_entity` (768), `e14_bge_m3_dense` (1024) | All L2-normalized; `Vec::new()` when `includeEmbeddings=false` |
| Asymmetric | `e5_causal_cause` (768), `e5_causal_effect` (768), `e8_graph_source` (1024), `e8_graph_target` (1024), `e10_paraphrase` (768), `e10_context` (768) | |
| Sparse | `e6_sparse_indices: Vec<u16>` + `e6_sparse_values: Vec<f32>`, `e13_splade_indices`, `e13_splade_values` | empty when `includeSparseVectors=false` |
| Token | `e12_token_embeddings: Vec<Vec<f32>>` (128D / token) | empty when `includeTokenEmbeddings=false` |
| Fusion | `topic_profile: [f32; 14]`, `cross_correlations: Vec<f32>` (length 91), `group_alignments: [f32; 6]` | Sourced from `CF_TOPIC_PROFILES` when present (`computation.rs::topic_profile_or_fallback`); else per-embedder vector-presence fallback |
| Graph | `outgoing_edges: Vec<TrainingEdge>`, `incoming_edges: Vec<TrainingEdge>`, `knn_neighbors: Vec<Vec<KnnNeighbor>>` (always 14 outer slots) | |
| Causal | `causal_effects: Vec<CausalLabel>`, `causal_causes: Vec<CausalLabel>` | LLM-discovered |
| Topics | `topic_memberships: Vec<TopicMembership>` | Each carries `topic_profile: [f32; 14]` |
| Temporal | `temporal_labels: Option<TemporalLabels>` | Phase-5 hour/day/month buckets, age, session position |
| Tucker | `tucker_core: Option<TuckerCore>` | Phase-4 hook; `None` until Agent D populates it |
| Typed-edge sig | `edge_type_distribution: [u32; 8]` | bincode positional, MUST stay last (append-only field) |

Counts (`crates/context-graph-core/src/training/mod.rs:67-74`):

| Constant | Value | Meaning |
| --- | --- | --- |
| `TRAINING_RECORD_VERSION` | 3 | On-disk version byte |
| `NUM_EMBEDDERS` | 14 | Content embedder slot count (E1-E14) |
| `NUM_CROSS_CORRELATIONS` | 91 | C(14, 2) — unique embedder pairs |
| `NUM_GROUP_ALIGNMENTS` | 6 | Factual / Temporal / Causal / Relational / Qualitative / Implementation |
| `NUM_EDGE_TYPE_DISTRIBUTION` | 8 | Per-`GraphLinkEdgeType` counts |

### 1.2 `TrainingEdge` (`mod.rs:179-189`)

| Field | Type | Notes |
| --- | --- | --- |
| `edge_type` | `u8` | `GraphLinkEdgeType::as_u8`: 0=SemanticSimilar, 1=CodeRelated, 2=EntityShared, 3=CausalChain, 4=GraphConnected, 5=ParaphraseAligned, 6=KeywordOverlap, 7=MultiAgreement |
| `peer_id` | `Uuid` | target for outgoing edges, source for incoming |
| `weight` | `f32` | |
| `direction` | `u8` | `Symmetric=0`, `Forward=1`, `Backward=2` |
| `agreement_count` | `u8` | |
| `embedder_scores` | `[f32; 14]` | Per-embedder similarity at edge creation time |

### 1.3 `CausalLabel` (`mod.rs:206-222`)

| Field | Notes |
| --- | --- |
| `related_memory_id: Uuid` | `Uuid::nil()` when the peer end isn't resolvable |
| `rel_id: Uuid` | Always the `CausalRelationship.id` (joinable against `CF_CAUSAL_RELATIONSHIPS`) |
| `description: String` | `"cause_statement -> effect_statement. explanation"` |
| `direction: String` | `"cause"` / `"effect"` |
| `confidence: f32` | |
| `mechanism_type: Option<String>` | `"direct"` / `"mediated"` / `"feedback"` / `"temporal"` |

### 1.4 `TemporalLabels` (`temporal.rs`)

| Field | Notes |
| --- | --- |
| `stored_at`, `stored_hour_utc`, `stored_day_of_week`, `stored_month` | Derived from `fingerprint.created_at` |
| `age_seconds_at_export` | `export_now − stored_at` (i64) |
| `session_sequence`, `session_total`, `relative_position` | Caller-provided; `relative_position = seq/total` |
| `periodic_bucket: PeriodicBucket` | 8-class enum: WeekdayMorning..WeekendNight (`temporal.rs:24-50`); boundaries 5..12 / 12..17 / 17..21 / 21..5 |
| `e2_recency_norm`, `e3_periodic_norm`, `e4_positional_norm` | L2 norms of the three temporal vectors |

### 1.5 `compute_edge_type_distribution` (`edge_signature.rs`)

Saturating `u32` count per `GraphLinkEdgeType` slot. `debug_assert` that `edge.edge_type().as_u8() < 8`. The sum equals `outgoing_edges.len()` modulo `u32::MAX` saturation.

### 1.6 Fusion features (`computation.rs`)

- `topic_profile_or_fallback(stored_profile, fingerprint)`: prefers the `CF_TOPIC_PROFILES` row when present; otherwise derives a per-embedder presence indicator (dense L2 norm clamped, sparse `sqrt(sum_sq)` clamped, E12 mean of non-zero per-token alignments).
- `compute_cross_correlations(topic_profile, synergy)`: `cross_corr[(i,j)] = alignment_i · alignment_j · synergy(i, j)` for all `i < j` in `0..14`. Returns exactly 91 values in `[0, 1]`.
- `compute_group_alignments(topic_profile)`: 6D group aggregation — `Factual=E1/E12/E13/E14, Temporal=E2/E3, Causal=E4/E7, Relational=E5/E8/E9, Qualitative=E10/E11, Implementation=E6` (per `GroupAlignments::from_alignments`).

### 1.7 Persistence

Records are bincode-serialized with a single version byte prefix (no `skip_serializing_if` per constitution). Stored in `CF_TRAINING_RECORDS` (`column_families.rs:399`). Iteration helpers and version handling live in `context_graph_storage::teleological::rocksdb_store::export`. The encoder/decoder pair is `encode_training_record` / `decode_training_record`; tests roundtrip via real `RocksDbTeleologicalStore` in `tests/training_records_integration.rs`.

## 2. MCP Training Tools (4 tools)

`crates/context-graph-mcp/src/tools/definitions/training.rs` registers four tools (verified count from grep `ToolDefinition::new`):

| Tool | Inputs | Behavior |
| --- | --- | --- |
| `export_training_corpus` | `filter: "all"|"session"`, `filterId`, `maxMemories` (default 10000, max 1_000_000), `includeEmbeddings` (default true), `includeSparseVectors` (default true), `includeTokenEmbeddings` (default false), `includeEdges` (default true), `includeCausalLabels` (default true), `includeIncomingEdges` (default false), `includeTemporalLabels` (default true), `includeTuckerCore` (default false), `clearExisting` (default false) | Iterates non-soft-deleted fingerprints, assembles `TrainingRecord`, writes to `CF_TRAINING_RECORDS`. Output is binary only (bincode + version byte). `additionalProperties:false` enforced server-side |
| `list_training_records` | `limit` (default 50, max 1000), `offset`, `includeShape` (default true) | UUIDs + optional record-shape summary |
| `get_training_record` | `memoryId` (required), `includeVectors` (default false), `includeTokenEmbeddings` (default false), `includeEdges` (default true) | Full record by UUID |
| `count_training_records` | — | Returns row count |

Sister surfaces:
- `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs` exposes `export_typed_edges_corpus` (`CF_TYPED_EDGE_RECORDS`), `list_typed_edge_records`, `get_typed_edges`, `validate_low_confidence_edges` — see [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).
- `crates/context-graph-mcp/src/tools/definitions/learning.rs` exposes the learner-as-UTL dataset tools `export_learner_training_dataset`, `list_learner_training_datasets`, `get_learner_training_dataset`, `count_learner_training_datasets` (`CF_LEARNER_TRAINING_DATASETS`) — see §5 below and [20_learning_subsystem.md](20_learning_subsystem.md).

## 3. CLI Export

`crates/context-graph-cli/src/commands/export_training.rs` writes Parquet files using a minimal `(memory_id: Utf8, record_bytes: Binary)` schema; bytes are byte-for-byte identical to RocksDB (`encode_training_record` is reused, not re-serialized):

```text
context-graph-cli export training-corpus
  --kind training-corpus       # (only allowed value)
  --format parquet             # (only allowed value)
  --out <path>                 # output Parquet file (parent must exist)
  --storage <path>             # RocksDB data dir, env CONTEXT_GRAPH_STORAGE_PATH
  --batch-size <usize>         # default 500; controls row-group size
  --limit <usize>              # optional cap; default unlimited
```

Compression: ZSTD (default level via `parquet::file::properties::WriterProperties`). Failure modes:
- Missing parent directory for `--out` → `anyhow::Error`.
- Any RocksDB error from the store propagates unmodified.
- Any Parquet writer error aborts the export; partial files are removed.
- Version-byte mismatches on read are surfaced via `decode_training_record`, never silently dropped.

Sister CLI surfaces:
- `context-graph-cli learning {count|list|get|export-events-jsonl|export-dataset-jsonl|record-events-jsonl|record-synthetic}` — dumps `CF_LEARNING_EVENTS` and `CF_LEARNER_TRAINING_DATASETS` to JSONL for downstream Python DPO miners (`build_learner_dpo_pairs.py`). See [20_learning_subsystem.md](20_learning_subsystem.md) §17 and [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md) §7.
- `context-graph-cli utl {list-embedders|fixture|preflight-assets|count|record-synthetic|get-delta|get-m}` — operational view of E15-E21 learner-state.

Integration tests:
- `crates/context-graph-cli/tests/export_training_integration.rs` — roundtrips synthetic `TrainingRecord`s through Parquet using a real `RocksDbTeleologicalStore` in a `TempDir`. Asserts byte-for-byte equality after bincode/Parquet/decoder roundtrip; tests empty CF, batch-size respect, `--limit` cap, and invalid output dir.
- `crates/context-graph-cli/tests/learning_jsonl_export_integration.rs` — exercises `learning export-events-jsonl` and `learning export-dataset-jsonl` against a real RocksDB store. Validates JSONL row schema, feature_tensor shape, SHA-256 provenance.

## 4. Causal Embedder Fine-Tuning Pipeline

`crates/context-graph-embeddings/src/training/`:

```text
LLM (Hermes 2 Pro) ──> CausalTrainingPair ──> CausalDataLoader
                                                    │
                                              ┌─────┴─────────┐
                                              │  CausalTrainer │
                                              │  ┌───────┐    │
                                              │  │ Loss:  │    │
                                              │  │ InfoNCE│    │
                                              │  │ Dir.   │    │
                                              │  │ Sep.   │    │
                                              │  │ Soft   │    │
                                              │  │ Multi  │    │
                                              │  │  Task  │    │
                                              │  └───────┘    │
                                              │  ┌───────┐    │
                                              │  │AdamW  │    │
                                              │  └───────┘    │
                                              └─────┬─────────┘
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
| `confidence` | `f32` ∈ `[0,1]` | Soft label; clamped on construction |
| `mechanism` | `String` | "biological", "economic", … |
| `hard_negative` | `String` | Semantically similar but non-causal |
| `rationale` | `Option<String>` | Optional LLM rationale |
| `domain` | `String` | Curriculum-learning category (default `"general"`) |

`CausalTrainingPair::is_causal()` = `direction != None && confidence >= 0.5`. `difficulty()` returns 0.2 (explicit markers like "because"/"therefore"), 0.5 (no markers, no hard negative), or 0.8 (hard negative present); 0.0 for non-causal pairs. Used for stage-1 curriculum filtering (`stage1_max_difficulty = 0.2`).

`seed_training_pairs()` ships ~250+ in-tree seed pairs spanning health, environment, economics, technology, social, physics, nutrition, cybersecurity, psychology, history, legal, engineering for smoke runs without external data. `expand_seed_pairs()` returns the expanded dataset (reversed + cross-domain negatives).

`CausalDataLoader::new(pairs, batch_size, seed)` uses `rand::rngs::StdRng::seed_from_u64(seed)`; `shuffle_epoch()` reshuffles indices each epoch.

### 4.2 LoRA Adapters (`training/lora.rs`)

`LoraConfig` defaults (lines 53-66):

| Field | Default |
| --- | --- |
| `rank` | 16 |
| `alpha` | 16.0 (so scale = alpha/rank = 1.0) |
| `dropout` | 0.1 |
| `hidden_size` | 768 (NomicBERT) |
| `num_layers` | 12 |
| `apply_query` | true |
| `apply_value` | true |
| `apply_key` | false |

`total_params` = `2 * hidden_size * rank * (apply_query + apply_value + apply_key) * num_layers` = 2·768·16 = 24,576 per adapter × 2 (Q + V) × 12 layers = **589,824** trainable LoRA parameters.

`LoraAdapter`:
- `a: Var [hidden_size, rank]` initialized via deterministic golden-ratio low-discrepancy quasi-random with Kaiming scale (`std_dev = sqrt(2/hidden_size)`).
- `b: Var [rank, hidden_size]` initialized to zero (LoRA starts as identity).
- `forward(x)` = `scale * (x @ A @ B)` (no dropout).
- `forward_train(x)` = `scale * dropout(x @ A @ B, p=dropout)`.
- `apply_dropout`: Bernoulli mask `floor(uniform[0,1) + (1-p))`, scaled by `1/(1-p)` (inverted-dropout).

`LoraLayers` collection:

| Field | Type |
| --- | --- |
| `query_adapters` | `Vec<LoraAdapter>` (length = num_layers when `apply_query`) |
| `value_adapters` | `Vec<LoraAdapter>` |
| `key_adapters` | `Vec<LoraAdapter>` (empty unless `apply_key`) |
| `config` | `LoraConfig` |
| `training` | **`Cell<bool>`** |

The `Cell<bool>` lets `&self` methods toggle dropout on/off without `&mut self` (`lora.rs:189,300`). Stage entry points call `set_training(true)` (`pipeline.rs:933,1061`) and `set_training(false)` after the stage finishes (`pipeline.rs:948,1076`); evaluation passes save/restore via `was_training = self.lora.training.get(); self.lora.set_training(false); ... self.lora.set_training(was_training);` (`pipeline.rs:563-618`).

LoRA can be loaded from safetensors (`LoraLayers::load_from_safetensors`) using tensor naming `lora.{query|value|key}.{layer}.{a|b}`, matching the `save_lora` format (`pipeline.rs:1196-1263`).

**Lesson (load-bearing):** LoRA dropout adds dropout-mask tensors to the autograd graph — VRAM consumption rises. Default `batch_size=16` was reduced from `32` once dropout was enabled, and re-raising it can OOM RTX 5090. When dropout is disabled or removed, gate thresholds must be recalibrated because dropout regularization compresses absolute scores.

### 4.3 Loss Functions (`training/loss.rs`)

`LossConfig` defaults (lines 42-51):

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

1. **InfoNCE contrastive** — `info_nce_loss(cause_vecs, effect_vecs)` over an N×N similarity matrix; uses `candle_nn::loss::cross_entropy` for differentiable gather-based indexing. When `n > 2` and `hard_negative_scale > 1.0`, `apply_hard_negative_mining` builds a mask scaling top-k semi-hard negatives (logits below the diagonal positive) by `hard_negative_scale`.
2. **Directional margin** — pulls forward similarity above mean off-diagonal similarity by `margin`. For N=1, pushes paired similarity above `margin`.
3. **Causal separation** — minimizes cosine similarity between paired cause/effect vectors (mean of batch cosine).
4. **Soft-label distillation** — MSE between paired cosine and `pair.confidence`.

`LossComponents` aggregates `{ contrastive, directional, separation, soft_label, total }` for logging.

**Lesson (load-bearing):** `hard_negative_scale >= 2.0` combined with `temperature = 0.05` causes catastrophic score collapse — cause/effect vectors become orthogonal. Keep `hard_negative_scale <= 1.3` if enabling. The default 1.0 disables the amplifier entirely.

### 4.4 AdamW Optimizer (`training/optimizer.rs`)

`AdamWConfig` defaults:

| Field | Default | Notes |
| --- | --- | --- |
| `lr_projection` | 1e-4 | W_cause / W_effect / biases |
| `lr_lora` | 1e-5 | LoRA params (10× smaller) |
| `lr_markers` | 5e-4 | Multi-task head params |
| `beta1` | 0.9 | |
| `beta2` | 0.999 | |
| `epsilon` | 1e-8 | |
| `weight_decay` | 0.01 | Decoupled |
| `max_grad_norm` | 1.0 | Gradient clipping (global L2 norm) |
| `total_steps` | 1000 (overridden by pipeline to data-derived value) | Schedule basis |
| `warmup_fraction` | 0.1 | 10% of total_steps |

`ParamGroup::{Projection,Lora,Markers}` lets each group ride its own learning rate. Schedule = linear warmup followed by cosine decay (`current_lr` in `optimizer.rs:108-128`).

Step semantics:
- Computes global gradient L2 norm across all tracked params, scales every grad by `max_grad_norm / (norm + ε)` if exceeded.
- AdamW updates: `m ← β1·m + (1-β1)·grad`; `v ← β2·v + (1-β2)·grad²`; bias-corrected `m_hat = m/(1-β1ᵗ)`, `v_hat = v/(1-β2ᵗ)`; `θ ← θ - lr·m_hat/(√v_hat + ε)`.
- Decoupled weight decay: `θ ← θ - lr·weight_decay·θ` applied as separate term (per AdamW paper).
- All updates `.detach()` to prevent the autograd graph from chaining across steps.
- `reset_moments()` zeros m/v on stage transitions.

### 4.5 Multi-task Heads (`training/multitask.rs`)

Optional auxiliary tasks; active in stages 2+3 when `PipelineConfig::multitask_config = Some(MultiTaskConfig::default())` (default).

`MultiTaskConfig` defaults:

| Field | Default |
| --- | --- |
| `input_dim` | 768 |
| `hidden_dim` | 256 |
| `lambda_direction` | 0.2 |
| `lambda_mechanism` | 0.1 |

| Head | Input | Output |
| --- | --- | --- |
| `direction_head: ClassificationHead` | concatenated cause+effect [N, 2·input_dim] | 3-class logits (`DirectionLabel::{Forward, Backward, None}`) |
| `mechanism_head: ClassificationHead` | concatenated cause+effect [N, 2·input_dim] | 7-class logits (`MechanismLabel::{Biological, Economic, Physical, Technical, Social, Ecological, Other}`) |

Each head is a 2-layer MLP (linear → ReLU → linear) with deterministic Xavier-flavoured init (`std = sqrt(2/(in+out))`, golden-ratio quasi-random fill). Cross-entropy loss against `LongTensor` labels, computed manually via shifted log-softmax + gather (no per-sample autograd loss because the loop accumulates `nll_sum` as f64 then constructs a fresh scalar tensor at the end).

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

`PipelineConfig` defaults (lines 58-72):

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
| 1 — warm-up | 25 | projection only (W_cause, W_effect, biases) | difficulty ≤ 0.2 (falls back to all pairs if < 4 easy ones) | no (saves VRAM; `gpu_forward_base_tensor`) |
| 2 — LoRA activation | 20 | projection + LoRA + multi-task heads | all pairs | yes (`gpu_forward_with_lora_tensor`) |
| 3 — directional | 20 | projection + LoRA + multi-task heads, with `lambda_directional` boosted to `stage3_direction_weight = 0.6` | all pairs | yes |

Forward path during training:
- `cause_prefixed = "search_document: " + pair.cause_text` (NOT `search_query: …` — instruction prefixes were collapsing all vectors to the same direction; the projection heads now learn cause/effect separation themselves).
- Same for effect.
- Encoder runs through `attention::create_strategy(&AttentionMode::Dense)` (sequences capped at 512 tokens).
- `l2_normalize` applied before loss computation.

Cross-stage continuity (`load_stage_projection`): each stage loads the previous stage's `projection_stage{N-1}_best.safetensors` if present, falling back to fresh `TrainableProjection::new`. The optimizer's `total_steps` is recomputed per stage as `(pairs/batch_size + 1) * stage_epochs` so the LR cosine schedule fits the actual data size — not the default 1000.

Between stages: `drop(trainer); drop(snapshot_eval); cuda_dev.synchronize();` to free Var tensors before the next stage allocates.

**Per-stage best-stage selection (load-bearing):**
- Inside a stage, the best checkpoint is selected by `metrics.directional_accuracy > best.directional_accuracy` and saved as `projection_stage{N}_best.safetensors` (`pipeline.rs:574-593`).
- A guard "always save final projection at end of stage" line writes the trainer's final projection regardless of eval-tie status (stages 2 & 3, lines 962-969 / 1090-1096) — protects against eval-metric ties that block the per-stage checkpoint from updating.
- The benchmark snapshot path (`log_benchmark_snapshot` / "best across stages" comparison) uses `metrics.score_spread` (`pipeline.rs:953,1081`) — `score_spread` (top-1 minus rank-5 cosine) is more discriminative than directional_accuracy for global "is this stage actually better than the previous" comparisons.
- Why per-stage: each stage learns a different projection, so the best stage's LoRA must pair with that stage's projection. Cross-stage mixing degrades inference quality. Stages 2 & 3 each save `lora_stage{2,3}_best.safetensors`, then the final selection step copies the best-stage LoRA to `lora_best.safetensors`.

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

`meets_targets()` checks the original 5; `meets_finetuning_targets()` checks the Option-B trio (`score_spread > 0.10`, `anisotropy < 0.30`, `standalone_accuracy >= 0.67`).

`Evaluator` exposes:
- `directional_accuracy(cause, effect)` — fraction where `forward_sim > reverse_sim`.
- `direction_ratio` — `mean(forward_sim) / mean(reverse_sim)`.
- `causal_auc(sims, labels)` — Wilcoxon-Mann-Whitney AUC.
- `topical_mrr(query, candidates, correct_indices)` — gather rank by counting candidates with higher similarity.
- `score_spread(sims)` — top-1 minus rank-5 (or last if < 5).
- `anisotropy_measure(vectors)` — average pairwise cosine sampled at most 100 pairs.
- `standalone_top1_accuracy(query, candidates, expected)` — top-1 hit rate.
- `evaluate(cause, effect, non_causal_sims)` — full bundle.

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

Behavior:
- `add_pair(pair)`: rejects pairs below `confidence_threshold`; FIFO-evicts when buffer hits `max_buffer_size`.
- `ready_for_training()` iff `len >= min_pairs`.
- `drain_for_training()` consumes the buffer via `mem::take` and increments `total_processed` + `rounds_completed`.
- `passes_quality_gate(metrics)`: when gate enabled, requires `new.directional_accuracy >= best.directional_accuracy`. EMA prevents catastrophic drift across rounds.

Returned `DistillationResult` carries `{ num_pairs, epochs_run, post_metrics, quality_gate_passed, rolled_back }`.

### 4.10 Training Binary

`crates/context-graph-embeddings/src/bin/train_causal.rs`:

```bash
train-causal --model-path models/causal --output models/causal/trained
train-causal --model-path models/causal --data pairs.jsonl --epochs 50
```

CLI flags: `--model-path`, `--data` (JSONL), `--output`, `--epochs`, `--batch-size` (default 16), `--lora-rank` (default 16), `--eval-every` (default 5), `--seed` (default 42), `--no-multitask` (saves VRAM by setting `multitask_config = None`).

When `--epochs N` is supplied, the binary distributes as `stage1 = N/5`, `stage2 = N·2/5`, `stage3 = N − stage1 − stage2` (each clamped to ≥ 1). Train/eval split is 80/20 with seeded shuffle. After training, the binary writes `lora_weights.safetensors` to `--output`.

Device selection: CUDA when `candle_core::utils::cuda_is_available()`, else CPU (with a warning).

### 4.11 Checkpoint Files

After a successful pipeline run, `models/causal/trained/` contains:

| File | Source |
| --- | --- |
| `projection_stage1_best.safetensors` | Best Stage-1 projection (warm-up) |
| `projection_stage2_best.safetensors` | Best Stage-2 projection (LoRA activation) |
| `projection_stage3_best.safetensors` | Best Stage-3 projection (directional emphasis) |
| `projection_best.safetensors` | Copy of best stage's projection (selected by `score_spread`) |
| `lora_stage2_best.safetensors` | Stage-2 LoRA (Q,V × 12 layers) |
| `lora_stage3_best.safetensors` | Stage-3 LoRA |
| `lora_best.safetensors` | Copy of best stage's LoRA (paired with `projection_best.safetensors`) — Stage 1 falls back to current pipeline LoRA state when best_stage=1 (no stage-1 LoRA) |
| `training_trajectory.jsonl` | Append-only per-stage benchmark snapshots: `{ timestamp, stage, score_spread, directional_accuracy, standalone_top1, direction_ratio, duration_ms }` |

`ProductionMultiArrayProvider::new` loads `lora.safetensors` + the best projection eagerly via `e5_causal_model.load_trained_weights(&trained_dir)` and FAILS FAST if these files are missing — the search gate cannot calibrate without them.

## 5. Learner-as-UTL Training Datasets (`learner_training.rs`)

Matrix shards compiled from learner + Learning-as-UTL CFs into row-major `f32` tensors. Six task families:

| Task variant (`LearnerTrainingTask`) | Wire string |
| --- | --- |
| `RewardModel` | `reward_model` (default) |
| `Reranker` | `reranker` |
| `EmbedderContrastive` | `embedder_contrastive` |
| `DiagnosticClassifier` | `diagnostic_classifier` |
| `Scheduler` | `scheduler` |
| `PersonalPhysiology` | `personal_physiology` |

Constants:

| Constant | Value | Source |
| --- | --- | --- |
| `LEARNER_TRAINING_DATASET_VERSION` | 1 | `learner_training.rs:20` |
| `MAX_LEARNER_TRAINING_ROWS` | 1_000_000 | `learner_training.rs:23` |
| `MAX_LEARNER_TRAINING_COLS` | 4096 | `learner_training.rs:27` |

`learning_event_feature_schema()` (`learner_training.rs:255`) returns a deterministic 149-column schema for `LearningEvent` rows:

| Block | Columns | Count |
| --- | --- | --- |
| `before_topic_e1..e14` | 14 | before topic profile |
| `before_cross_correlation_00..90` | 91 | before correlations |
| `before_embedder_score_e1..e14` | 14 | before embedder scores |
| `delta_topic_e1..e14` | 14 | after − before topic |
| Scalar block | 16 | `before_retrieval_rank_norm`, `after_retrieval_rank_norm`, `before_contradiction_pressure`, `after_contradiction_pressure`, `before_integration_confidence`, `after_integration_confidence`, `before_stability_score`, `after_stability_score`, `surprise_score`, `coherence_delta`, `contradiction_delta`, `consolidation_readiness`, `transfer_score`, `multi_utl_score`, `embedder_disagreement`, `retrieval_rank_shift` |
| **Total** | **149** | |

`learning_event_label_schema()` returns 4 labels: `utility_delta`, `outcome_label`, `correction_required`, `reuse_observed`.

`LearnerTrainingDataset::validate()` enforces:
- `feature_schema` non-empty + ≤ `MAX_LEARNER_TRAINING_COLS`
- `rows.len() ≤ MAX_LEARNER_TRAINING_ROWS`
- `rows_len`/`cols_len` consistent with payload
- `row_major.len() == rows × cols`
- All values finite
- Per-row `provenance_sha256` is 64-hex
- `row_major_sha256` recomputed at validate() time and compared (catches in-memory tampering)
- `provenance_manifest_sha256` recomputed from `(task, schemas, rows, source_counts)`

`learning_event_feature_vector(event)` validates the event then concatenates topic_profile + cross_correlations + embedder_scores + delta_topic + the 16 scalars; returns `DimensionMismatch` on any size error or `ValidationError` on non-finite values.

`rank_norm(rank: Option<u32>) -> f32 = 1.0 / (1.0 + rank as f32)` (0.0 when None).

## 6. Benchmark Suite (`crates/context-graph-benchmark/`)

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
| `causal_accuracy_bench` | Vector distinctness, marker detection, query-intent 3-class accuracy, modifier sweep, per-mechanism modifiers (Phases 1.1, 1.2, 3.4, 4.1, 4.2 of the E5 plan); GPU-gated Phase 1.1 needs `--features real-embeddings` |
| `causal_e2e_bench`, `causal_perf_bench`, `causal_realdata_bench`, `causal_provenance_bench`, `causal_provenance_e2e_bench`, `causal_provenance_llm_bench`, `causal_prompt_experiment_bench`, `causal_span_fix_bench`, `causal_discovery_mcp_bench`, `causal_embedding_bench` | Validate E5 fine-tuning impact end-to-end. `causal_provenance_e2e_bench` measures span-populated rate (>95%), span text match (>85%), offset validity (>95%), storage success (100%), provenance chain complete (100%), search provenance display (100%) |
| `e1_semantic_bench`, `e7_bench`, `e7_realdata_bench`, `e10_modifier_tuning`, `e11_entity_bench`, `e2_manual_verification`, `e4_hybrid_session_bench`, `embedder_impact_bench`, `embedder_impact_realdata_bench`, `embedder_stress` | Per-embedder regression / tuning |
| `gpu_bench` | GPU similarity / kernel benchmarks |
| `hf_bench` | HuggingFace embedder direct calls |
| `mcp_bench` | MCP server throughput |
| `realdata_bench`, `unified_realdata_bench` | Live-DB end-to-end |
| `model_injection_bench`, `model_injection_rigorous_bench` | Load-time injection |

**Lesson (load-bearing):** Benchmark phases that don't accept `&BenchConfig` cannot exercise the model under test — they're testing infrastructure or hardcoded constants only. If a metric never changes across runs, it is testing hardcoded behavior; flag and remove. Split scoring into model-dependent vs infrastructure-dependent for honest reporting. Provider failures in benchmarks must `panic` (fail-fast) — silent `0.0` corrupts results.

## 7. Versioning Policy

- `TRAINING_RECORD_VERSION = 3` rejects v1/v2 records on read; pre-production policy is **wipe and re-export**, not migrate.
- `LEARNING_EVENT_VERSION = 1`, `LEARNER_TRAINING_DATASET_VERSION = 1`, `LEARNER_RECORD_VERSION = 1` — all single-byte version prefixes; mismatches surface `CoreError::SerializationError`.
- Bincode positional serialization. Any new field MUST be appended at the end. `skip_serializing_if` is forbidden per constitution.
- `training_records` CF is in the `is_versioned_cf` set; legacy CFs intentionally not retrofitted with version bytes.

## 8. Cross-references

- DPO/CAMD downstream consumers of these outputs: [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md).
- Learner subsystem and 7-slot E15-E21 namespace: [20_learning_subsystem.md](20_learning_subsystem.md).
- Embedder topology (E1-E14, E5 dual vectors): [05_embedding_subsystem.md](05_embedding_subsystem.md).
- Memory-record fingerprint shape (input to training records): [04_storage_layer.md](04_storage_layer.md).
- GPU device singleton shared between training and inference: [11_cuda_acceleration.md](11_cuda_acceleration.md) §7-§8.
- Typed-edge training records and contrastive pair mining: [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).
- Causal discovery LLM that supplies `CausalTrainingPair`s: [07_graph_and_causal.md](07_graph_and_causal.md).
