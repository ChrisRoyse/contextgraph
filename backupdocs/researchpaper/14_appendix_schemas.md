# 14. Appendix A — Schemas, Key Formats, and Constants

Reference material for reproduction. All information here is drawn from the authoritative source files in the Context Graph, ClipCannon, Phoenix, and Leapable codebases. Line-number citations point to the source files of record as of the manuscript writing date.

## A.1 Context Graph — the thirteen embedders

| # | ID | Backbone | Dense dim | Asymmetric | Sparse | Token-level | Distance metric | Index type | Quantisation |
|---|---|---|---|---|---|---|---|---|---|
| E1 | Semantic | e5-large-v2 | 1,024 | No | No | No | Cosine | HNSW | PQ-8 |
| E2 | Temporal Recent | Sinusoidal PE of Unix timestamp | 512 | No | No | No | Cosine | HNSW | Float8 |
| E3 | Temporal Periodic | Fourier basis (5 periods × 51 harmonics) | 512 | No | No | No | Cosine | HNSW | Float8 |
| E4 | Temporal Positional | Sinusoidal PE of session turn index | 512 | No | No | No | Cosine | HNSW | Float8 |
| E5 | Causal | nomic-embed-text-v1.5 fine-tuned | 768 × 2 | Yes (cause / effect) | No | No | Asymmetric Cosine | HNSW × 2 | PQ-8 |
| E6 | Sparse Lexical | BERT-vocab keyword frequency | 30,522 vocab, ≤1,500 active | No | Yes | No | Jaccard + dot | Inverted | SparseNative |
| E7 | Code | Qodo-Embed-1-1.5B (Qwen2 family) | 1,536 | No | No | No | Cosine | HNSW | PQ-8 |
| E8 | Graph | e5-large-v2 (structure-view) | 1,024 × 2 | Yes (source / target) | No | No | Cosine | HNSW × 2 | Float8 |
| E9 | HDC | 10,000-bit hypervector → 1,024 D projection | 1,024 | No | No | No | Cosine | HNSW | Binary |
| E10 | Contextual | e5-base-v2 (paraphrase / context) | 768 × 2 | Yes (paraphrase / context) | No | No | Cosine | HNSW × 2 | PQ-8 |
| E11 | Entity (KEPLER) | RoBERTa + TransE on Wikidata5M | 768 | No | No | No | Cosine | HNSW | Float8 |
| E12 | Late Interaction | ColBERTv2 | 128 per token | No | No | Yes | MaxSim | No HNSW | TokenPruning |
| E13 | SPLADE v3 | BERT-vocab with learned expansion | 30,522 vocab, ≤1,500 active | No | Yes | No | Jaccard + BM25 | Inverted | SparseNative |

Total: 11,008 effective dense dimensions (after counting all asymmetric dual vectors as separate) + 2 × 30,522 sparse vocabularies + 128-per-token late-interaction space.

## A.2 Context Graph — 59 RocksDB column families

| Group | Count | Column families |
|---|---|---|
| Base (8 + 3 graph-linking) | 11 | `nodes`, `edges`, `embeddings`, `metadata`, `temporal`, `tags`, `sources`, `system`, `embedder_edges`, `typed_edges`, `typed_edges_by_type` |
| Teleological (20 + 8 new) | 28 | `fingerprints`, `topic_profiles`, `e13_splade_inverted`, `e6_sparse_inverted`, `e1_matryoshka_128`, `content`, `source_metadata`, `file_index`, `topic_portfolio`, `e12_late_interaction`, `entity_provenance`, `audit_log`, `audit_by_target`, `merge_history`, `importance_history`, `tool_call_index`, `consolidation_recommendations`, `embedding_registry`, `custom_weight_profiles`, `hnsw_graphs`, `training_records`, `constellations`, `constellation_by_selector`, `contrastive_pairs`, `contrastive_by_kind`, `contrastive_by_anchor`, `typed_edge_records`, `typed_edge_validations` |
| Quantised embedder | 13 | `emb_0` (E1 PQ-8) through `emb_12` (E13 SparseNative) |
| Code | 5 | `code_entities`, `code_e7_embeddings`, `code_file_index`, `code_name_index`, `code_signature_index` |
| Causal | 2 | `causal_relationships`, `causal_by_source` |

Source of truth for the count: `crates/context-graph-storage/src/column_families.rs:397` — constant `TOTAL_COLUMN_FAMILIES = 59`.

## A.3 Context Graph — 14 built-in weight profiles

Each profile is a 13-element array `[E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13]` summing to ≈ 1.0. E12 and E13 are 0.0 in all semantic scoring profiles per ARCH-13 — they are pipeline-stage-only.

| Profile | Primary embedder(s) | Use case |
|---|---|---|
| `semantic_search` | E1 (0.33) | General queries |
| `causal_reasoning` | E1 (0.40) + E5 (0.10) | "Why" questions |
| `code_search` | E7 (0.40) | Programming queries |
| `fact_checking` | E11 (0.40) | Entity / fact queries |
| `graph_reasoning` | E8 (0.40) | Structural / connectivity |
| `temporal_navigation` | E2 / E3 / E4 (0.23 each) | Time-based queries |
| `sequence_navigation` | E4 (0.55) | Sequence traversal |
| `conversation_history` | E4 (0.35) + E1 (0.30) | Contextual recall |
| `category_weighted` | Constitution-compliant | Semantic=1.0 / Relational=0.5 / Structural=0.5 weights |
| `typo_tolerant` | E9 (0.18) + E1 (0.30) | Noisy / misspelled queries |
| `pipeline_stage1_recall` | E13 (0.25) + E6 (0.25) | Sparse retrieval stage |
| `pipeline_stage2_scoring` | E1 (0.50) | Dense scoring stage |
| `pipeline_full` | E1 (0.40) | Complete pipeline |
| `balanced` | Equal (≈ 0.077 each) | Testing / comparison |

## A.4 ClipCannon — the seven video modalities

| # | Modality | Model | Dim | Captures |
|---|---|---|---|---|
| 1 | Visual | google/siglip-so400m-patch14-384 | 1,152 | Facial appearance, pose, expression, lighting, composition |
| 2 | Semantic | nomic-ai/nomic-embed-text-v1.5 | 768 | Spoken meaning — topics, vocabulary, argument structure |
| 3 | Emotion | facebook/wav2vec2-large-960h | 1,024 | Vocal arousal / valence |
| 4 | Speaker | microsoft/wavlm-base-plus-sv | 512 | Biometric voice identity (timbre, formant structure) |
| 5 | Prosody | Custom (F0, energy, rate, contour) | 12 | Speaking style — musical quality of speech |
| 6 | Sentiment | all-MiniLM-L6-v2 | 384 | Sentence-level sentiment / intent |
| 7 | Voice | marksverdhei/Qwen3-Voice-Embedding-12Hz-1.7B (ECAPA-TDNN) | 2,048 (L2-normalised) | Speaker verification signature |

Total: 5,900 dimensions (including the 2,048-D Qwen3 voice embedding used in Case 3). The 4,044 figure cited elsewhere in the paper is computed with the 192-D compact ECAPA-TDNN variant used by ClonEval's reference pipeline; both sum to a single-input multi-dimensional decomposition.

## A.5 ClipCannon — 23-stage DAG execution levels

```
Level 0: probe
Level 1: vfr_normalize
Level 2: audio_extract, frame_extract
Level 3: source_separation, visual_embed, ocr, quality, storyboard, transcribe, acoustic
Level 4: shot_type, scene_analysis, semantic_embed, narrative_llm, speaker_embed, emotion_embed, reactions, profanity
Level 4.5: prosody_analysis (depends on source_separation + transcribe)
Level 5: chronemic
Level 6: highlights
Level 7: finalize
```

Required: `probe`, `vfr_normalize`, `audio_extract`, `frame_extract`, `transcribe`, `finalize` — failure aborts.
Optional: all others — failure logged, downstream degrades gracefully.

## A.6 Santa constellation — per-modality sample counts (Case 2)

NPZ file: `~/.clipcannon/models/santa/embeddings/all_embeddings.npz`. 8.8 MB on disk. Counts after diarisation filtering (subject only):

| Modality | Samples | Dim | NPZ key |
|---|---|---|---|
| Visual | 1,725 | 1,152 | `vis_emb` |
| Semantic | 200 | 768 | `sem_emb` |
| Emotion | 343 | 1,024 | `emo_emb` |
| Speaker | 300 | 512 | `spk_emb` |
| Prosody | 188 | 12 | `pro_data` |
| Sentiment | 202 | 384 | `sent_emb` |
| Voice | 188 | 192 | `voice_emb` |
| **Aggregate** | **3,146 samples × 4,044 dim** | — | — |

Plus FLAME parametric head fits: 3,477 frames × 100 expression coefficients.

## A.7 Santa LoRA — training hyperparameters (Case 2)

- **Base model**: EchoMimicV3 (1.86 B DiT on Wan2.1-Fun; Meng et al., 2025; Wan Team, 2025)
- **Adapter**: LoRA rank 256, alpha 512 (Hu et al., 2022)
- **Target modules**: `q`, `k`, `v`, `o`, `k_audio`, `v_audio`, `k_img`, `v_img` (eight; `q_audio` excluded — see §8.4 for the typo-induced dead-code reason)
- **Steps**: 3,000
- **Learning rate**: 1e-4, cosine decay
- **Batch**: 1, grad_accum 32 (effective batch 32); batch 1 forced by RTX 5090 VRAM budget
- **Wall-clock target**: 90–120 min on RTX 5090; peak VRAM ~14 GB
- **Loss**: mouth-weighted flow-matching — 10× mouth, 2× face, 1× background
- **Flow-matching target**: `target = noise - latent` (CORRECT velocity convention; see §8.4 for the sign-bug discussion)
- **Sampler during training**: `WeightedRandomSampler` with viseme-rarity weights (OY, AW, UH oversampled 2–4×)

## A.8 Voice cloning — pipeline constants (Case 3)

- **Base TTS**: Qwen3-TTS-12Hz-1.7B-Base, unmodified
- **Generation parameters**: `temperature=0.3`, `top_p=0.85`, `repetition_penalty=1.05`, `max_new_tokens=2048`
- **Best-of-N**: N = 12, candidates scored by target encoder (WavLM)
- **Reference construction**: 50-clip centroid, L2-normalised mean of L2-normalised samples, selected from the subject's highest-quality clips ranked by WavLM similarity to real microphone recording
- **Evaluation encoder**: microsoft/wavlm-base-plus-sv (ClonEval standard)
- **Secondary encoder** (used in ablation): SpeechBrain ECAPA-TDNN `spkrec-ecapa-voxceleb` 192-D
- **Enhancement** (post-SECS-measurement, for production delivery only): Resemble Enhance denoise + 24 kHz → 44.1 kHz upsampling

## A.9 Constellation Guard thresholds (Case 2, defaults)

| Modality | Encoder | Threshold | Severity tiers | Rationale |
|---|---|---|---|---|
| Visual | SigLIP-SO400M (1,152 D) | 0.70 | WARN 0.80, STRONG 0.90 | Robust across angles and lighting |
| Emotion | Wav2Vec2-large (1,024 D) | 0.60 | — | Expressiveness variability tolerated |
| Speaker | WavLM-large-sv (512 D) | 0.80 | — | Voice identity is biometric |
| Voice | ECAPA-TDNN (192 D) | 0.85 | — | Strictest — explicit verification metric |

## A.10 Four-level expression hierarchy (Phoenix)

- **Level 0** — 17 FLAME expression-coefficient indices mapped to FACS Action Units (`FLAME_TO_AU` in `expression_skills.py`).
- **Level 1** — 40 K-Means clusters over FLAME coefficients (default `n_groups=40`); active-AU threshold deviation ≥ 0.5 from neutral mean.
- **Level 2** — 34 named expression skills with onset (0.2 s) / peak (0.5 s) / offset (0.3 s) phases and natural-language prompts (default `n_skills=25` temporal clusters).
- **Level 3** — 8 behavioural constellations with skill cycling at ~1 skill per 2 s. Full list in Section 6.3.

## A.11 Leapable — 32 tool modules, 161 tools (summary)

| # | Module | Count | # | Module | Count |
|---|---|---|---|---|---|
| 1 | `database.ts` | 5 | 17 | `chunks.ts` | 4 |
| 2 | `database-management.ts` | 8 | 18 | `embeddings.ts` | 4 |
| 3 | `portability.ts` | 7 | 19 | `tags.ts` | 6 |
| 4 | `sharing.ts` | 4 | 20 | `intelligence.ts` | 5 |
| 5 | `ingestion.ts` | 7 | 21 | `health.ts` | 1 |
| 6 | `search.ts` | 7 | 22 | `maintenance.ts` | 1 |
| 7 | `documents.ts` | 10 | 23 | `users.ts` | 2 |
| 8 | `provenance.ts` | 6 | 24 | `collaboration.ts` | 11 |
| 9 | `config.ts` | 2 | 25 | `workflow.ts` | 8 |
| 10 | `vlm-local.ts` | 3 | 26 | `events.ts` | 6 |
| 11 | `images.ts` | 8 | 27 | `clm.ts` | 9 |
| 12 | `extraction.ts` | 1 | 28 | `compliance.ts` | 3 |
| 13 | `reports.ts` | 7 | 29 | `redaction.ts` | 5 |
| 14 | `extraction-structured.ts` | 2 | 30 | `dashboard.ts` | 2 |
| 15 | `comparison.ts` | 6 | 31 | `creator.ts` | 4 |
| 16 | `clustering.ts` | 7 | 32 | `_scope.ts` | 0 (helpers only) |

Total: **161 tools** registered across **31 modules that register tools** (module #32 is the per-email-scope helper file).

## A.12 Leapable — revenue split defaults

| Variable | Default | Source |
|---|---|---|
| `CREATOR_SHARE_PERCENT_STANDARD` | 70 | `env.ts:80` |
| `CREATOR_SHARE_PERCENT_FOUNDING` | 75 | `env.ts:81` |
| `CREATOR_RESERVE_PERCENT` | 5 | `env.ts:82` |
| `CREATOR_RESERVE_HOLD_DAYS` | 60 | `env.ts:83` |
| `OCR_COST_CENTS_PER_PAGE` | 5 | `env.ts` |
| `REDACTION_COST_CENTS_PER_PAGE` | 3 | `env.ts` |
| `MIN_BALANCE_CENTS_FOR_OCR_SUBMIT` | 5 | `env.ts` |

Subscriber tier allocations (`TOKEN_ALLOCATIONS_*` env vars): free=50, starter=200, builder=1,000, pro=2,500, max=6,000 queries/month.

## A.13 Provenance chain hash formula (shared)

Used by ClipCannon (Section 5.6) and Leapable (Section 11.4):

```
chain_hash = SHA256(
  "{parent_hash}|{input_sha256}|{output_sha256}|{operation}|{model_name}|{model_version}|{json.dumps(model_params, sort_keys=True)}"
)
```

First record uses `parent_hash = "GENESIS"`. Each subsequent record incorporates the parent's chain hash, so modification of any historical record invalidates every downstream hash. Leapable additionally enforces append-only invariants on `ledger_entries`, `admin_audit_log`, and `marketplace_events` via SQLite `BEFORE UPDATE` / `BEFORE DELETE` triggers that `RAISE(FAIL)`.

## A.14 Context Graph — training record wire format

Column family: `CF_TRAINING_RECORDS`. Encoding: `[version_byte: u8 = TRAINING_RECORD_VERSION (2)][bincode(TrainingRecord)]`. Positional bincode layout; `skip_serializing_if` is prohibited because it silently corrupts field-count-changing serialisations. New fields append only.

Fields (condensed):

```
memory_id, content, importance, created_at, session_id,
source_type, source_path, content_hash,
e1_semantic (1024), e2_temporal_recent (512), e3_temporal_periodic (512), e4_temporal_positional (512),
e5_causal_cause (768), e5_causal_effect (768),
e7_code (1536),
e8_graph_source (1024), e8_graph_target (1024),
e9_hdc (1024),
e10_paraphrase (768), e10_context (768),
e11_entity (768),
e6_sparse_indices / values, e13_splade_indices / values,
e12_token_embeddings (variable_token_count × 128),
topic_profile (13D), cross_correlations (78D), group_alignments (6D),
outgoing_edges, incoming_edges (both Vec<TrainingEdge>),
knn_neighbors (always 13 outer vecs),
causal_effects, causal_causes (Vec<CausalLabel>),
topic_memberships (Vec<TopicMembership>),
temporal_labels: Option<TemporalLabels>,
tucker_core: Option<TuckerCore> (Phase-4, at ranks (4,4,128)),
edge_type_distribution: [u32; 8]   # v2 field: per-GraphLinkEdgeType count of outgoing edges
```
