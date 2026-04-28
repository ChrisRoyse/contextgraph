# 11 - Canonical Embedder Matrix

**Status:** Runtime-aligned matrix after resolving the E14 collision.

## Source of Truth

The executable source of truth is:

- Content embedders: `context_graph_embeddings::types::ModelId::production()`
- Learner-state embedders: `context_graph_embeddings::learner::LearnerEmbedderSlot::all()`
- Runtime inspection: `context-graph-cli utl list-embedders`

`ModelId::all()` still returns the legacy `Entity` variant for compatibility. It is not counted as a canonical slot because production E11 is `Kepler`.

## Counts

| Category | Slots | Count |
|---|---:|---:|
| Production content embedders | E1-E14 | 14 |
| Learner-state embedders | E15-E21 | 7 |
| Canonical total | E1-E21 | 21 |
| Legacy compatibility variants | `Entity` | 1, not counted |

## Matrix

| Slot | Runtime id | Model / implementation | Native dim | Storage dim | Role |
|---:|---|---|---:|---:|---|
| E1 | `semantic` | `intfloat/e5-large-v2` | 1024 | 1024 | Semantic content meaning |
| E2 | `temporal_recent` | Exponential recency basis | 512 | 512 | Recency signal |
| E3 | `temporal_periodic` | Fourier periodic basis | 512 | 512 | Periodic time signal |
| E4 | `temporal_positional` | Sinusoidal positional basis | 512 | 512 | Positional time signal |
| E5 | `causal` | `nomic-ai/nomic-embed-text-v1.5` | 768 | 768 | Causal / contradiction signal |
| E6 | `sparse` | SPLADE lexical encoder | 30522 | 1536 | Sparse lexical recall |
| E7 | `code` | `Qodo/Qodo-Embed-1-1.5B` | 1536 | 1536 | Code structure |
| E8 | `graph` | `intfloat/e5-large-v2` graph/sentence use | 1024 | 1024 | Graph text signal |
| E9 | `hdc` | Hyperdimensional computing encoder | 10000 | 1024 | Robust symbolic signature |
| E10 | `contextual` | `intfloat/e5-base-v2` | 768 | 768 | Contextual paraphrase |
| E11 | `kepler` | `THU-KEG/KEPLER-Wiki5M-KE` | 768 | 768 | Entity / knowledge graph signal |
| E12 | `late_interaction` | `colbert-ir/colbertv2.0` | 128/token | 128 pooled | Token-level late interaction |
| E13 | `splade` | SPLADE v3 | 30522 | 1536 | Stage-1 sparse retrieval |
| E14 | `bge_m3_dense` | `BAAI/bge-m3` dense head | 1024 | 1024 | Multilingual dense retrieval |
| E15 | `e15_affect_speech` | Wav2Vec2-large MSP-DIM | 1024 | 1024 | Speech arousal / dominance / valence |
| E16 | `e16_affect_face` | OpenFace 3.0 + MTL emotion/AU/gaze head | 512 | 512 | Face affect and AU state |
| E17 | `e17_affect_text` | MiniLM-L6-v2 + EmoBank regressor | 384 | 384 | Text valence / arousal |
| E18 | `e18_ppg` | PaPaGei PPG foundation model | 512 | 512 | HRV coherence |
| E19 | `e19_eda` | WESAD chest physiology stress head | 64 | 64 | Stress floor |
| E20 | `e20_eeg` | LaBraM EEG foundation model | 768 | 768 | Plasticity window |
| E21 | `e21_eeg_artifact_robust` | EEGPT artifact-robust EEG model | 768 | 768 | Artifact-robust plasticity check |

## Compatibility Rule

E14 is reserved for `BgeM3Dense` in the production content registry. Learner-state embedders start at E15. Any runtime, fixture, documentation, or storage record that names the learner-state namespace as E14-E20 is stale and should be updated to E15-E21.
